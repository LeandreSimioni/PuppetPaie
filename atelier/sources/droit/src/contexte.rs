//! Les donnees de la compagnie, lues une fois a une date : les fiches (entreprise, etablissements, salaries,
//! spectacles), l'effectif, et le bareme de l'annee empile (le socle livre, la convention, la surcouche de la
//! compagnie) puis lu a la date.
//!
//! Les fiches et le bareme se lisent par leurs formes (`fiche`, `forme_bareme`). Un refus est une `String`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::fiche::{Case, ChampsEntreprise, ChampsSpectacle, Datee, Etablissement, FicheEntrepriseDatee, FicheSalarie, FicheSalarieDatee, FicheSpectacleDatee, Salaries, Spectacle, Spectacles};
use crate::forme_bareme::{CotisationDsn, CotisationModifiee, CoucheConvention, CoucheNature, NatureModifiee, NonPortee, Pile, ReductionGenerale, Rubrique, RubriqueModifiee, Surcouche, TauxAtmpEcrit, ValeurAtmp, VersementMobiliteEcrit};
use socle::echec::APart;
use socle::num::Decimal;

use socle::date::{repr_texte, Jour};
use socle::fichiers::dossier_du_code;
use socle::mois::Mois;
use socle::num::Resultat;
use socle::texte::cle_recherche;

// ------------------------------------------------------------ les petits outils

/// Le texte d'une case de fiche, exigee : son absence se refuse en la nommant.
fn exige<T: AsRef<str>>(c: &Case<T>, cle: &str) -> Resultat<String> {
    match c {
        Case::Absente => Err(socle::fichiers::absent(cle)),
        _ => Ok(c.texte().to_string()),
    }
}

/// `"%g" % x`.
pub fn format_g(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return if x.is_nan() { "nan" } else if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let t = format!("{:.5e}", x);
    let (m, e) = t.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    let rogner = |s: String| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if !(-4..6).contains(&e) {
        format!("{}e{}{:02}", rogner(m.to_string()), if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        rogner(format!("{:.*}", (5 - e) as usize, x))
    }
}

/// Un nombre a virgule, sans zeros inutiles.
fn nombre_fr(x: f64) -> String {
    format_g(x).replace('.', ",")
}

// ------------------------------------------------- la recherche floue

/// La distance de Levenshtein, sur les caracteres.
fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prec: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cour = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cour.push((prec[j + 1] + 1).min(cour[j] + 1).min(prec[j] + usize::from(ca != cb)));
        }
        prec = cour;
    }
    prec[b.len()]
}

// ------------------------------------------------------------ les fiches

// ------------------------------------------------- la configuration datee (`fiche::dater`)

/// LES AUTRES ETABLISSEMENTS OUVERTS UN JOUR AU MOINS entre `debut` et `fin` (la fiche entreprise datee : `etablissements`
/// est la liste de ceux qui sont ouverts, une version par ouverture, modification ou fermeture), chacun dans sa derniere
/// valeur de la periode. Un etablissement ferme en cours de mois a encore sa DSN de ce mois. `None` : la fiche n'en dit
/// rien.
pub fn etablissements_entre(liste: &[Datee<Vec<Etablissement>>], debut: &Jour, fin: &Jour) -> Option<Vec<Etablissement>> {
    if liste.is_empty() {
        return None;
    }
    let versions = crate::fiche::valeurs_datees(liste);
    let mut ordre: Vec<String> = Vec::new();
    let mut vus: HashMap<String, Etablissement> = HashMap::new();
    for (i, v) in versions.iter().enumerate() {
        let jusqu_a = versions.get(i + 1).and_then(|w| w.depuis);
        let commence_avant_la_fin = v.depuis.map_or(true, |j| j <= *fin);
        let finit_apres_le_debut = jusqu_a.map_or(true, |j| j > *debut);
        if !(commence_avant_la_fin && finit_apres_le_debut) {
            continue;
        }
        for e in v.valeur.dite().into_iter().flatten() {
            let nic = e.nic.texte().to_string();
            if !vus.contains_key(&nic) {
                ordre.push(nic.clone());
            }
            vus.insert(nic, e.clone());
        }
    }
    Some(ordre.into_iter().filter_map(|n| vus.remove(&n)).collect())
}

/// L'HISTOIRE DES AUTRES ETABLISSEMENTS : chacun dans sa derniere valeur, et le dernier jour ou il etait ouvert s'il est
/// ferme (la veille de la premiere version qui ne le porte plus).
pub fn histoire_des_etablissements(liste: &[Datee<Vec<Etablissement>>]) -> Resultat<Vec<(Etablissement, Option<Jour>)>> {
    let versions = crate::fiche::valeurs_datees(liste);
    let mut ordre: Vec<String> = Vec::new();
    let mut vus: HashMap<String, (Etablissement, Option<Jour>)> = HashMap::new();
    for (i, v) in versions.iter().enumerate() {
        let ouverts: &[Etablissement] = v.valeur.dite().map_or(&[], Vec::as_slice);
        let presents: Vec<&str> = ouverts.iter().map(|e| e.nic.texte()).collect();
        for e in ouverts {
            let nic = e.nic.texte().to_string();
            if !vus.contains_key(&nic) {
                ordre.push(nic.clone());
            }
            vus.insert(nic, (e.clone(), None));
        }
        if i > 0 {
            if let Some(j) = v.depuis {
                for (nic, (_, ferme)) in vus.iter_mut() {
                    if !presents.contains(&nic.as_str()) && ferme.is_none() {
                        *ferme = Some(j.plus_jours(-1)?);
                    }
                }
            }
        }
    }
    Ok(ordre.into_iter().filter_map(|n| vus.remove(&n)).collect())
}

// ------------------------------------------------ l'effectif

const CODE_SECURITE_SOCIALE: &str = "A01";
const ANNEES_CONSECUTIVES: i32 = 5;
/// Le plus petit seuil d'effectif dont un taux depend (la formation des permanents) : une annee qui ne le departage pas
/// ne departage aucun des autres.
const PLUS_PETIT_SEUIL: i64 = 11;

/// Les effectifs A01 connus, par annee civile (`config/effectifs.json`) : ceux que l'Urssaf renvoie en compte rendu de
/// nature 118, que le releve des retours y ecrit, et ceux que la compagnie dit pour une annee sans 118
/// (`envois::releve_retours::poser_effectif`), qu'un 118 de la meme annee remplace.
#[derive(Debug, Clone, Default)]
pub struct Effectif {
    pub annees: BTreeMap<i32, f64>,
    /// L'annee de la premiere embauche (fiche entreprise) : avant elle, zero.
    pub premiere_embauche: Option<i32>,
    /// La fiche dit la compagnie d'au moins cinquante salaries (`cinquante_salaries_et_plus`, une compagnie qui emploie des
    /// permanents) : sans elle, aucun seuil de cinquante et plus n'est atteint, et l'effectif ne se lit pas pour eux.
    pub cinquante_salaries: bool,
}

/// Le plus petit des grands seuils : le FNAL a 0,50 % et le `tdelta` de la reduction generale (bareme,
/// `seuils_effectif`), l'echeance du 5 (R. 243-6 CSS), la PEEC (CCH L. 313-1), puis la contribution supplementaire a
/// l'apprentissage a 250 (C. trav. L. 6242-1, I).
pub const GRAND_SEUIL: i64 = 50;

/// La case est-elle cochee ? Elle ne vaut qu'avec des permanents, la seule voie vers cinquante salaries : une compagnie
/// d'intermittents ne la voit pas, et un seuil de cinquante et plus ne l'atteint pas.
pub fn cinquante_salaries_et_plus(fiche: &ChampsEntreprise) -> bool {
    emploie_des_permanents(fiche) && fiche.cinquante_salaries_et_plus.oui()
}

/// `config/effectifs.json` : les effectifs par annee civile et par code, ceux que l'Urssaf renvoie en 118 (`calcule_le`,
/// `source`) et ceux que la compagnie a dits (`dit_le`) ; ecrit par `envois::releve_retours`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FichierEffectifs {
    #[serde(rename = "quoi", default)]
    _quoi: Option<String>,
    #[serde(default)]
    annees: BTreeMap<String, BTreeMap<String, EffectifEcrit>>,
}

/// L'effectif d'un code pour une annee.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectifEcrit {
    valeur: f64,
    #[serde(rename = "calcule_le", default)]
    _calcule_le: Option<String>,
    #[serde(rename = "source", default)]
    _source: Option<String>,
    #[serde(rename = "dit_le", default)]
    _dit_le: Option<String>,
}

/// Lit `config/effectifs.json` (absent : vide).
pub fn effectif_de(racine: &Path, fiche: &ChampsEntreprise) -> Resultat<Effectif> {
    let p = racine.join(EFFECTIFS);
    let mut annees = BTreeMap::new();
    if socle::fichiers::existe(&p) {
        let lu: FichierEffectifs = socle::forme::lire_fichier(&p, "Les effectifs de la compagnie ne se lisent pas : relever les retours les récrit.")?;
        for (an, codes) in lu.annees {
            if an.is_empty() || !an.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if let Some(a01) = codes.get(CODE_SECURITE_SOCIALE) {
                annees.insert(an.parse::<i32>().map_err(|e| e.to_string())?, a01.valeur);
            }
        }
    }
    Ok(Effectif { annees, premiere_embauche: fiche.premiere_embauche.dite().map(|j| j.an), cinquante_salaries: cinquante_salaries_et_plus(fiche) })
}

impl Effectif {
    pub fn annee(&self, a: i32) -> Option<f64> {
        if let Some(v) = self.annees.get(&a) {
            return Some(*v);
        }
        match self.premiere_embauche {
            Some(p) if a < p => Some(0.0),
            _ => None,
        }
    }

    /// L'effectif le plus recent connu pour payer en `annee_paie`, ou `None`.
    pub fn valeur(&self, annee_paie: i32) -> Option<f64> {
        match self.annees.range(..annee_paie).next_back() {
            Some((_, v)) => Some(*v),
            None => self.annee(annee_paie - 1),
        }
    }

    /// LES ANNEES DONT L'EFFECTIF MANQUE POUR LES TAUX DE LA PAIE EN `annee_paie` : celles des cinq dernieres que le plus
    /// petit seuil ne departage pas (`atteint`, L. 130-1, II, CSS) ; ce sont elles qui font les taux a seuil
    /// (`controler_effectif`). Vide : les seuils de la paie sont connus. Ce qui lit l'effectif de l'annee d'avant
    /// (`valeur` : le FNAS de la CCNEAC sans CSE dit) le demande lui-meme.
    pub fn a_dire(&self, annee_paie: i32) -> Vec<i32> {
        if self.atteint(PLUS_PETIT_SEUIL, annee_paie).is_ok() {
            return Vec::new();
        }
        (annee_paie - ANNEES_CONSECUTIVES..annee_paie).filter(|a| self.annee(*a).is_none()).collect()
    }

    /// Le seuil est-il franchi pour payer en `annee_paie` ? (L. 130-1, II, CSS :
    /// cinq annees civiles consecutives.)
    /// Un grand seuil (`GRAND_SEUIL` et plus) n'est atteint que si la fiche dit la compagnie d'au moins cinquante salaries.
    pub fn atteint(&self, seuil: i64, annee_paie: i32) -> Resultat<bool> {
        let grand = seuil >= GRAND_SEUIL;
        if grand && !self.cinquante_salaries {
            return Ok(false);
        }
        let fenetre = annee_paie - ANNEES_CONSECUTIVES..annee_paie;
        let connues: BTreeMap<i32, f64> = fenetre.clone().filter_map(|a| self.annee(a).map(|v| (a, v))).collect();
        if connues.values().any(|v| *v < seuil as f64) {
            return Ok(false);
        }
        if connues.len() == ANNEES_CONSECUTIVES as usize {
            return Ok(true);
        }
        let manquent: Vec<String> = fenetre.filter(|a| !connues.contains_key(a)).map(|a| a.to_string()).collect();
        Err(format!(
            "Effectif de l'entreprise pour payer en {} : {}. Le seuil de {} salariés ne se franchit qu'après cinq années civiles \
consécutives au-dessus (CSS, art. L. 130-1, II) : il manque l'effectif de {}. Une compagnie de moins de cinq ans dit la date \
de sa première embauche (Configuration › Entreprise) ; une plus ancienne dit l'effectif de chacune de ces années, tel que \
l'Urssaf l'a calculé (Configuration › Entreprise, Avancé, « Dire ou poser l'effectif », ou la route effectif). Le compte \
rendu de nature 118 de l'Urssaf le remplace à son arrivée.{}",
            annee_paie,
            if connues.is_empty() {
                "rien de connu".to_string()
            } else {
                format!(
                    "connu pour {}",
                    connues.iter().map(|(a, v)| format!("{} ({})", a, nombre_fr(*v))).collect::<Vec<_>>().join(", ")
                )
            },
            seuil,
            manquent.join(", "),
            if grand {
                " Une compagnie de moins de cinquante salariés décoche « Au moins 50 salariés » (Configuration › Entreprise, Avancé)."
            } else {
                ""
            }
        ))
    }
}

// ----------------------------------------- les baremes : socle, convention, fusion

/// Une erreur de bareme, dite pour l'utilisateur : ce qui manque, et ou le dire.
struct Refus {
    message: String,
}

impl From<String> for Refus {
    fn from(message: String) -> Refus {
        Refus { message }
    }
}

/// Le bareme d'une annee, ou ce que la compagnie y dit, n'existe pas.
fn introuvable(message: String) -> Refus {
    Refus { message }
}

/// Les annees dont le socle est livre.
fn annees_du_socle(dossier: &Path) -> Resultat<Vec<i32>> {
    let fichiers = match socle::fichiers::lister(dossier)? {
        Some(f) => f,
        None => Vec::new(),
    };
    let mut v: Vec<i32> = fichiers
        .into_iter()
        .filter_map(|p| {
            if p.extension()? != "json" {
                return None;
            }
            let stem = p.file_stem()?.to_str()?;
            if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            stem.parse().ok()
        })
        .collect();
    v.sort();
    Ok(v)
}

/// Les annees disponibles, ecrites comme `", ".join(...) or "aucune"` ; le refus du disque, s'il refuse.
fn disponibles(dossier: &Path) -> String {
    match annees_du_socle(dossier) {
        Ok(a) if a.is_empty() => "aucune".to_string(),
        Ok(a) => a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "),
        Err(e) => e,
    }
}


/// Le fichier du socle livre de l'annee, s'il est livre.
fn fichier_du_socle(dossier: &Path, an: i32) -> Option<PathBuf> {
    let p = dossier.join(format!("{}.json", an));
    socle::fichiers::existe(&p).then_some(p)
}

/// LE SOCLE LIVRE DE L'ANNEE `an`, seul (sans convention ni compagnie), lu au 1er janvier : ce que la tarification AT/MP
/// de l'annee en dit (`bareme::risques_de`, `bareme::arrete_atmp`). `None` : il n'est pas livre.
pub fn socle_livre_lu(an: i32) -> Resultat<Option<Pile>> {
    let Some(fichier) = fichier_du_socle(&dossier_du_code().join("baremes"), an) else { return Ok(None) };
    let jour = Jour::nouveau(an, 1, 1).ok_or_else(|| format!("Année {} hors du calendrier.", an))?;
    Ok(Some(crate::forme_bareme::lire_le_socle(&fichier, &jour, None)?.0))
}

/// LA COUCHE DE LA CONVENTION COLLECTIVE `idcc` (`par_convention` du socle), ce qu'elle fond dans le socle ; `None` : un
/// socle qui ne couvre aucune convention, ou une convention qui n'a rien a fondre.
fn convention<'a>(pile: &'a Pile, idcc: &str) -> Result<Option<&'a CoucheConvention>, String> {
    if pile.par_convention.is_empty() {
        return Ok(None);
    }
    let annee = pile.annee;
    let connues: Vec<&str> = pile.par_convention.keys().map(String::as_str).collect();
    let idcc = idcc.trim();
    if idcc.is_empty() {
        return Err(socle::echec::avec(
            "La fiche entreprise ne dit pas sa convention collective : la choisir dans Configuration › Entreprise.",
            APart {
                cle: Some("idcc".into()),
                detail: Some(format!(
                    "Bareme {} : la fiche entreprise ne dit pas son `idcc`, et les \
cotisations conventionnelles en dependent.\n\
\x20 Conventions couvertes : {}.\n\
\x20 A ecrire dans config/entreprise.json : \"idcc\": \"3090\".",
                    annee,
                    connues.join(", ")
                )),
                ..APart::default()
            },
        ));
    }
    let Some(couche) = pile.par_convention.get(idcc) else {
        return Err(socle::echec::avec(
            "La convention collective de la compagnie n'est pas couverte par le barème : le signaler au développeur.",
            APart {
                detail: Some(format!(
                    "Bareme {} : la convention collective {} n'a pas de couche dans le \
socle, et le moteur refuse de payer avec celle d'une autre \
branche.\n\
\x20 Couvertes a ce jour : {}.\n\
\x20 Ce qui manque n'est pas du code : les cotisations propres a \
cette convention (celle qui remplace la CASC-SVP, la prevoyance \
de ses intermittents cadres), a relever puis a ajouter dans \
`par_convention` de donnees/baremes/{}.json, avec leur source.",
                    annee,
                    idcc,
                    connues.join(", "),
                    annee
                )),
                ..APart::default()
            },
        ));
    };
    if !couche.applicable {
        return Err(format!(
            "Bareme {}, convention {} ({}) : la couche est RELEVEE mais pas \
confirmee, le moteur ne paie donc pas.\n  {}",
            annee,
            idcc,
            couche.libelle,
            couche.a_confirmer.as_deref().unwrap_or("")
        ));
    }
    Ok(couche.bareme.as_ref())
}

/// Une couche change ce que le bareme n'a pas : elle se corrige.
fn inconnue(ou: &str, quoi: &str) -> String {
    socle::echec::cle(format!("{} change « {} », que le barème de l'année n'a pas : la corriger.", ou, quoi), quoi)
}

/// Le code d'une cotisation de la DSN dans une couche : `code81`, ou `code81:affectation`.
fn code_de(c: &CotisationDsn) -> String {
    let code = c.code81.clone().unwrap_or_default();
    match c.affectation.as_deref().filter(|a| !a.is_empty()) {
        Some(a) => format!("{}:{}", code, a),
        None => code,
    }
}

/// Ce qu'une couche ecrit des rubriques `liste`, par leur `id`.
fn modifier_rubriques(liste: &mut [Rubrique], mods: &BTreeMap<String, RubriqueModifiee>, ou: &str) -> Resultat<()> {
    for (id, m) in mods {
        let r = liste.iter_mut().find(|r| r.id == *id).ok_or_else(|| inconnue(ou, id))?;
        crate::bareme::modifier(r, m);
    }
    Ok(())
}

/// Ce qu'une couche ecrit des cotisations `liste`, par leur code (`code_de`).
fn modifier_cotisations(liste: &mut [CotisationDsn], mods: &BTreeMap<String, CotisationModifiee>, ou: &str) -> Resultat<()> {
    for (code, m) in mods {
        let mut vue = false;
        for c in liste.iter_mut().filter(|c| code_de(c) == *code) {
            crate::bareme::modifier_cotisation(c, m);
            vue = true;
        }
        if !vue {
            return Err(inconnue(ou, code));
        }
    }
    Ok(())
}

/// Ce qu'une couche ecrit des modifications de rubriques d'une section s'ajoute a ce qu'elles portaient.
fn par_dessus_rubriques(section: &mut BTreeMap<String, RubriqueModifiee>, mods: &BTreeMap<String, RubriqueModifiee>) {
    for (id, m) in mods {
        crate::bareme::par_dessus(section.entry(id.clone()).or_default(), m);
    }
}

/// Ce qu'une couche ecrit de la reduction generale d'une section s'ajoute a ce qu'elle portait.
fn reduction_par_dessus(r: &mut ReductionGenerale, m: &ReductionGenerale) {
    macro_rules! si {
        ($($c:ident),*) => { $( if m.$c.is_some() { r.$c = m.$c.clone(); } )* };
    }
    si!(verifie, formule, tmin, tdelta, puissance, t, plafond_smic, smic_horaire_reference, imputation, dsn, majoration_caisse_conges, source, note);
    if !m.statuts.is_empty() {
        r.statuts = m.statuts.clone();
    }
}

/// Ce qu'une couche ecrit de la section d'une nature.
fn modifier_la_nature(nat: &mut CoucheNature, m: &NatureModifiee, ou: &str) -> Resultat<()> {
    par_dessus_rubriques(&mut nat.rubriques, &m.rubriques);
    modifier_rubriques(&mut nat.rubriques_ajoutees, &m.rubriques_ajoutees, ou)?;
    if !m.cotisations_dsn.is_empty() {
        let l = nat.cotisations_dsn.as_mut().ok_or_else(|| inconnue(ou, "cotisations_dsn"))?;
        modifier_cotisations(l, &m.cotisations_dsn, ou)?;
    }
    Ok(())
}

/// LA COUCHE DE LA CONVENTION FONDUE DANS LE SOCLE : ses tarifs, ses contributions, son champ s'ajoutent ; ce qu'elle
/// ecrit des rubriques et des sections du socle les change ; la section d'un statut qu'elle donne est la sienne entiere.
fn fondre_la_convention(pile: &mut Pile, c: &CoucheConvention, idcc: &str) -> Resultat<()> {
    let ou = format!("La couche de la convention {} du barème {}", idcc, pile.annee);
    if let Some(t) = &c.tarifs_conventionnels {
        pile.tarifs_conventionnels = Some(t.clone());
    }
    for x in &c.contributions_conventionnelles {
        match pile.contributions_conventionnelles.iter_mut().find(|y| y.id == x.id) {
            Some(y) => *y = x.clone(),
            None => pile.contributions_conventionnelles.push(x.clone()),
        }
    }
    if !c.contributions_dsn.is_empty() {
        pile.contributions_dsn = c.contributions_dsn.clone();
    }
    if let Some(s) = &c.secteurs {
        pile.secteurs = Some(s.clone());
    }
    modifier_rubriques(&mut pile.rubriques_bulletin, &c.rubriques, &ou)?;
    for (n, m) in &c.par_nature {
        modifier_la_nature(pile.par_nature.entry(n.clone()).or_default(), m, &ou)?;
    }
    for (k, m) in &c.par_categorie {
        let cat = pile.par_categorie.entry(k.clone()).or_default();
        par_dessus_rubriques(&mut cat.rubriques, &m.rubriques);
        for (s, sec) in &m.par_statut {
            cat.par_statut.insert(s.clone(), sec.clone());
        }
    }
    Ok(())
}

/// LA SURCOUCHE DE LA COMPAGNIE PAR-DESSUS : son annee, son taux AT/MP, son versement mobilite, et ce qu'elle decide
/// des rubriques et des cotisations au-dela d'un seuil d'effectif (`controler_effectif`). Rend le `depuis` du taux AT/MP
/// de `jour`, quand une notification le date.
fn fondre_la_surcouche(pile: &mut Pile, s: Surcouche, jour: &Jour, ou: &str) -> Resultat<Option<Jour>> {
    let ou = format!("La surcouche de la compagnie ({})", ou);
    pile.annee = s.annee;
    let mut depuis = None;
    if let Some(t) = &s.taux_atmp {
        let (t, d) = t.au_jour(jour)?;
        pile.taux_atmp = Some(t);
        depuis = d;
    }
    if s.versement_mobilite.is_some() {
        pile.versement_mobilite = s.versement_mobilite;
    }
    modifier_rubriques(&mut pile.rubriques_bulletin, &s.rubriques, &ou)?;
    modifier_cotisations(&mut pile.cotisations_dsn, &s.cotisations_dsn, &ou)?;
    for (k, m) in &s.par_statut {
        let st = pile.par_statut.get_mut(k).ok_or_else(|| inconnue(&ou, &format!("par_statut.{}", k)))?;
        if !m.rubriques.is_empty() {
            modifier_rubriques(st.rubriques_bulletin.as_mut().ok_or_else(|| inconnue(&ou, "rubriques_bulletin"))?, &m.rubriques, &ou)?;
        }
        if !m.cotisations_dsn.is_empty() {
            modifier_cotisations(st.cotisations_dsn.as_mut().ok_or_else(|| inconnue(&ou, "cotisations_dsn"))?, &m.cotisations_dsn, &ou)?;
        }
        if let Some(r) = &m.reduction_generale {
            reduction_par_dessus(st.reduction_generale.get_or_insert_with(ReductionGenerale::default), r);
        }
    }
    for (n, m) in &s.par_nature {
        modifier_la_nature(pile.par_nature.get_mut(n).ok_or_else(|| inconnue(&ou, &format!("par_nature.{}", n)))?, m, &ou)?;
    }
    Ok(depuis)
}

// ----------------------------------------- l'entreprise et ses etablissements

/// La cle de Luhn d'un SIRET (14 chiffres).
fn cle_siret(siret: &str) -> bool {
    if siret.len() != 14 || !siret.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let total: u32 = siret
        .chars()
        .rev()
        .enumerate()
        .map(|(i, c)| {
            let n = c.to_digit(10).unwrap() * if i % 2 == 1 { 2 } else { 1 };
            if n > 9 { n - 9 } else { n }
        })
        .sum();
    total % 10 == 0
}

/// La pile porte-t-elle un versement mobilite (une reponse, ou un taux) ?
fn versement_mobilite_dit(pile: &Pile) -> bool {
    match &pile.versement_mobilite {
        Some(VersementMobiliteEcrit::Dit(s)) => !s.is_empty(),
        Some(VersementMobiliteEcrit::Ecrit(t)) => t.taux.is_some() || t.code_insee.is_some() || t.autorite.is_some() || t.source.is_some() || t.note.is_some(),
        None => false,
    }
}

/// Les etablissements de la fiche se tiennent-ils (SIRET, NIC, une fois chacun) ?
pub fn controler_etablissements(fiche: &ChampsEntreprise, pile: &Pile) -> Resultat<()> {
    let siren = fiche.siren.texte();
    let an = pile.annee.to_string();
    let risques = crate::bareme::risques_de(pile);
    let mut vus = vec![fiche.nic_principal()];
    let mut griefs: Vec<String> = Vec::new();
    for e in fiche.secondaires() {
        let nic = e.nic.texte().to_string();
        let nom = match e.libelle.texte() {
            "" if !nic.is_empty() => nic.clone(),
            "" => "?".to_string(),
            l => l.to_string(),
        };
        if nic.len() != 5 || !nic.chars().all(|c| c.is_ascii_digit()) {
            griefs.push(format!("{} : NIC {} (cinq chiffres attendus)", nom, repr_texte(&nic)));
            continue;
        }
        if vus.contains(&nic) {
            griefs.push(format!("{} : NIC {} en double", nom, nic));
        }
        vus.push(nic.clone());
        if !siren.is_empty() && !cle_siret(&format!("{}{}", siren, nic)) {
            griefs.push(format!("{} : le SIRET {}{} n'a pas une cle juste", nom, siren, nic));
        }
        for (cle, c) in [("adresse", &e.adresse.en_texte()), ("code_postal", &e.code_postal.en_texte()), ("commune", &e.commune.en_texte())] {
            if c.texte().trim().is_empty() {
                griefs.push(format!("{} : `{}` manque", nom, cle));
            }
        }
        // Le code risque se dit toujours ; le taux, sauf quand l'arrete de l'annee le fixe (`bareme::taux_atmp_applique`).
        // Le taux d'une autre annee que celle du bareme ne se controle pas ici : la paie de l'annee le demande
        // (`bareme::surcharger`, `bareme::taux_atmp_d_un_etablissement`).
        let d_une_autre_annee = e.taux_atmp.dite().and_then(|t| t.annee.dite()).is_some_and(|a| *a != 0 && a.to_string() != an);
        let atmp = crate::bareme::taux_atmp_d_un_etablissement(e, &an);
        let atmp_juste = atmp.as_ref().is_some_and(|t| t.code_risque.as_deref().is_some_and(|c| !c.is_empty())
            && (d_une_autre_annee || crate::bareme::taux_atmp_applique(&risques, t, crate::bareme::en_alsace_moselle(e.code_postal.texte())).is_some()));
        if !atmp_juste {
            griefs.push(format!(
                "{} : son code risque AT/MP et son taux, tels que la CARSAT les notifie a CET etablissement (Configuration › \
Entreprise › Avancé, « Établissements »)",
                nom
            ));
        }
        if versement_mobilite_dit(pile) && matches!(e.versement_mobilite, Case::Absente | Case::Nulle) {
            griefs.push(format!(
                "{} : le siège paie un versement mobilité ; celui de cet établissement dépend de sa commune et de \
l'effectif qu'il y emploie (CGCT, L. 2333-64, I) : son taux et le code INSEE de sa commune, ou 0 s'il n'en paie pas \
(Configuration › Entreprise › Avancé, « Autres établissements »)",
                nom
            ));
        }
    }
    if griefs.is_empty() {
        return Ok(());
    }
    Err(socle::echec::avec(
        "Les établissements de la fiche entreprise se mêlent : chacun se déclare dans sa propre DSN, avec ce qui est à lui.",
        APart {
            detail: Some(format!(
                "config/entreprise.json, etablissements : chaque etablissement se \
declare dans sa propre DSN, avec ce qui est a lui.\n  - {}",
                griefs.join("\n  - ")
            )),
            ..APart::default()
        },
    ))
}

/// Le secteur de la fiche est-il un de ceux du bareme ?
fn controler_secteur(pile: &Pile, fiche: &ChampsEntreprise) -> Resultat<()> {
    let Some(sec) = &pile.secteurs else { return Ok(()) };
    let dit = sec.champ.dit(fiche);
    if sec.portes.iter().any(|p| *p == dit) {
        return Ok(());
    }
    Err(format!(
        "Convention {} : {}\n  Fiche Entreprise, « {} » : {}. Portés : {}.",
        fiche.idcc.portee().unwrap_or("None"),
        sec.refus,
        sec.libelle,
        if dit.is_empty() { "non renseigné".to_string() } else { dit },
        sec.portes.join(", ")
    ))
}

/// L'organisation patronale de la fiche est-elle une de celles du bareme ?
fn controler_organisation_patronale(pile: &Pile, fiche: &ChampsEntreprise) -> Resultat<()> {
    let nom = fiche.organisation_patronale.texte().trim().to_string();
    if nom.is_empty() {
        return Ok(());
    }
    let annee = pile.annee;
    let connues: Vec<&str> = pile.par_organisation_patronale.keys().map(String::as_str).collect();
    let Some(couche) = crate::bareme::couche_organisation(pile, &nom) else {
        return Err(socle::echec::avec(
            "L'organisation patronale de la compagnie n'est pas couverte par le barème : le signaler au développeur.",
            APart {
                detail: Some(format!(
                    "Bareme {} : la fiche entreprise declare une adhesion a \
l'organisation patronale {}, et le socle n'a pas de couche pour \
elle.\n\
\x20 Couvertes a ce jour : {}.\n\
\x20 Ce qui manque n'est pas du code : les taux que cette adhesion \
change, a relever a la source (l'accord lui-meme, la fiche de \
parametrage de l'organisme), puis a ecrire dans \
`par_organisation_patronale` de donnees/baremes/{}.json.\n\
\x20 Une compagnie qui n'adhere a rien laisse ce champ vide, et rien \
ne change pour elle.",
                    annee,
                    repr_texte(&nom),
                    if connues.is_empty() { "aucune".to_string() } else { connues.join(", ") },
                    annee
                )),
                ..APart::default()
            },
        ));
    };
    if !couche.applicable {
        return Err(format!(
            "Bareme {}, adhesion a l'organisation patronale {} ({}) : la \
couche existe mais n'est pas applicable, le moteur ne paie donc \
pas.\n  {}",
            annee,
            nom,
            couche.libelle,
            couche.a_confirmer.as_deref().unwrap_or("")
        ));
    }
    Ok(())
}

/// La pile dit-elle ce qu'il faut pour l'effectif de l'entreprise (`seuils_effectif`) ? Un bareme sans seuils ne se
/// controle pas.
fn controler_effectif(pile: &Pile, eff: &Effectif, ou: &str) -> Resultat<()> {
    let Some(decl) = &pile.seuils_effectif else { return Ok(()) };
    let an = pile.annee;
    let mut atteints: BTreeMap<i64, bool> = BTreeMap::new();
    let mut atteint = |s: i64| -> Resultat<bool> {
        if let Some(b) = atteints.get(&s) {
            return Ok(*b);
        }
        let b = eff.atteint(s, an)?;
        atteints.insert(s, b);
        Ok(b)
    };
    let ou = if ou.is_empty() { "config/baremes/<annee>.json" } else { ou };
    let (mut griefs, mut noms): (Vec<String>, Vec<String>) = (vec![], vec![]);

    for v in &decl.valeurs {
        if !atteint(v.seuil)? {
            // SOUS LE SEUIL : quand le socle dit que sa valeur est alors la seule due (`sous_le_seuil`, et le
            // texte qu'il cite : pour la formation, C. trav. L. 6331-1 et L. 6331-1 A, CSS L. 130-1, II), une surcouche
            // qui en ecrit une autre -- restee d'avant un franchissement a la baisse -- ferait payer ce qui n'est pas du.
            if let Some(texte) = v.sous_le_seuil.as_deref().filter(|t| !t.is_empty()) {
                let mut autres: Vec<String> = v
                    .effets
                    .iter()
                    .filter_map(|e| {
                        let ecrit = e.lu(pile)?;
                        (!ecrit.meme(&e.sous_le_seuil())).then(|| format!("{} = {} (le socle : {})", e, ecrit, e.sous_le_seuil()))
                    })
                    .collect();
                if !autres.is_empty() {
                    autres.sort();
                    noms.push(format!("{} (sous le seuil)", v.cle));
                    griefs.push(format!(
                        "  - {} : l'effectif n'atteint pas le seuil de {} salaries, et la surcouche ecrit une autre valeur que celle du socle
    {}
    Ecrit : {}
    A FAIRE : effacer ces valeurs de la surcouche ({}) ; le socle porte celle qui est due.
    Source : {}",
                        v.cle,
                        v.seuil,
                        v.quoi,
                        autres.join(" ; "),
                        ou,
                        texte
                    ));
                }
            }
            continue;
        }
        let mut restees: Vec<String> =
            v.effets.iter().filter(|e| e.lu(pile).is_some_and(|l| l.meme(&e.sous_le_seuil()))).map(|e| e.to_string()).collect();
        if restees.is_empty() {
            continue;
        }
        restees.sort();
        noms.push(v.cle.clone());
        griefs.push(format!(
            "  - {} (a partir de {} salaries)\n    {}\n    A ECRIRE : {}\n    Encore au taux du socle : {}\n    Source : {}",
            v.cle,
            v.seuil,
            v.quoi,
            v.au_dela,
            restees.join(", "),
            v.texte
        ));
    }

    for v in &decl.non_portees {
        let cle = v.cle.to_string();
        let NonPortee::VersementMobilite = v.cle;
        let rep = &pile.versement_mobilite;
        let taux_ecrit = matches!(rep, Some(VersementMobiliteEcrit::Ecrit(_)));
        if !atteint(v.seuil)? {
            if taux_ecrit {
                // Un taux ecrit sous le seuil : la compagnie paierait ce qu'elle ne doit pas.
                noms.push(format!("{} (sous le seuil)", cle));
                griefs.push(format!(
                    "  - {} : un taux est ecrit dans la surcouche, et il n'est \
du qu'a partir de {} salaries (CGCT L2333-64, L2531-2). \
L'effacer, ou ecrire \"aucun\".",
                    cle, v.seuil
                ));
            }
            continue;
        }
        let mut attendues: Vec<&str> = v.reponses.keys().map(String::as_str).collect();
        match rep {
            Some(VersementMobiliteEcrit::Dit(r)) if attendues.contains(&r.as_str()) => continue,
            Some(VersementMobiliteEcrit::Ecrit(t)) if t.taux.is_some() && t.code_insee.as_deref().unwrap_or("").trim().chars().count() == 5 => continue,
            _ => {}
        }
        noms.push(format!("{} (non porte)", cle));
        let constat = match rep {
            None => "Rien n'est ecrit sur ce point.".to_string(),
            Some(VersementMobiliteEcrit::Ecrit(t)) => format!(
                "La valeur ecrite (taux {}, code INSEE {}) n'a pas son taux ET le code INSEE (cinq \
caracteres) de la commune de l'etablissement : les deux se \
lisent sur la deliberation de l'autorite ou sur le bordereau \
Urssaf.",
                t.taux.map_or("absent".to_string(), |x| x.to_string()),
                t.code_insee.as_deref().map_or("absent".to_string(), |c| format!("« {} »", c))
            ),
            Some(VersementMobiliteEcrit::Dit(r)) => format!(
                "La valeur ecrite (« {} ») ne le debloque pas : il n'y a ni \
rubrique, ni ligne DSN, ni CTP pour la porter -- ecrire un \
taux ne le ferait pas cotiser.",
                r
            ),
        };
        attendues.sort();
        griefs.push(format!(
            "  - {} (a partir de {} salaries) -- LE MOTEUR NE LE PORTE PAS\n    {}\n    {}\n    A ECRIRE dans {} : \"{}\": {}, et rien d'autre ne le debloque.\n    Source : {}",
            cle,
            v.seuil,
            v.quoi,
            constat,
            ou,
            cle,
            attendues.iter().map(|r| format!("\"{}\"", r)).collect::<Vec<_>>().join(" ou "),
            v.texte.as_deref().unwrap_or("")
        ));
    }

    if griefs.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Bareme {}, entreprise de {} salaries : le moteur REFUSE de payer \
-- {}.\n\
\x20 Le socle livre avec le logiciel est ecrit pour le plus petit \
palier -- moins de onze salaries --, et ce qui change au-dessus n'est \
pas ecrit ici. Produire quand meme donnerait des bulletins et une DSN \
faux, sans un mot : c'est ce que le moteur faisait avant le \
21/09/2026.\n\
\x20 Ce qui manque, et ou :\n{}\n\
\x20 Une valeur SURCHARGEE est tenue pour decidee : le moteur verifie \
qu'une decision a ete prise, pas laquelle -- comme pour le taux \
AT/MP, que lui seul ne peut pas connaitre.",
        an,
        eff.valeur(an).map_or("?".to_string(), format_g),
        noms.join(", "),
        griefs.join("\n")
    ))
}

/// Les quatre, dans l'ordre du plus structurel au plus chiffre.
fn controler(pile: &Pile, fiche: &ChampsEntreprise, eff: &Effectif, ou: &str) -> Resultat<()> {
    controler_secteur(pile, fiche)?;
    controler_etablissements(fiche, pile)?;
    controler_organisation_patronale(pile, fiche)?;
    controler_effectif(pile, eff, ou)
}

/// LE `depuis` DU SMIC EN VIGUEUR a `jour` (la liste reste datee dans la pile : `Contexte::smic`), et sa valeur.
fn smic_en_vigueur(liste: &[crate::forme_bareme::Datee], jour: &Jour) -> Resultat<Option<(Jour, Decimal)>> {
    let mut entrees: Vec<(Jour, Decimal)> = Vec::new();
    for e in liste {
        let depuis = e.depuis.as_deref().ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", "smic_horaire.depuis", ""))?;
        entrees.push((Jour::lire(depuis)?, e.valeur));
    }
    entrees.sort_by_key(|(j, _)| *j);
    Ok(entrees.into_iter().filter(|(j, _)| j <= jour).last())
}



// --------------------------------------------------------------- le Contexte

/// Le jour courant (UTC : sans fuseau, un jour local differe
/// entre minuit et l'heure locale). Au temps du moteur (`temps`) : la montre, ou le temps simule du banc et du rejeu.
pub fn jour_reel() -> Jour {
    socle::temps::jour_utc()
}

/// `PUPPETPAIE_AUJOURDHUI` (AAAA-MM-JJ) remplace l'horloge.
pub fn aujourdhui() -> Resultat<Jour> {
    let jour = aujourdhui_brut()?;
    socle::fichiers::noter_valeur("aujourdhui", jour.iso());
    Ok(jour)
}

/// Sans le noter : `Contexte::nouveau` le garde, le calcul ne le lit pas.
pub fn aujourdhui_sans_note() -> Resultat<Jour> {
    aujourdhui_brut()
}

fn aujourdhui_brut() -> Resultat<Jour> {
    // LE TEMPS SIMULE (le banc d'essai, le rejeu d'un scenario : `temps`) fait foi : c'est lui qui dit quel jour on est.
    if socle::temps::simule() {
        return Ok(socle::temps::jour_de_paris());
    }
    match std::env::var("PUPPETPAIE_AUJOURDHUI") {
        Ok(v) if !v.is_empty() => Jour::lire(&v),
        _ => Ok(jour_reel()),
    }
}

/// LE CURSEUR DES PERMANENTS : une fiche Entreprise dit-elle que la compagnie emploie des
/// salaries payes au mois (`emploie_des_permanents`) ? Absent vaut NON. C'est la seule question qui ouvre ce qui ne vaut
/// que pour eux : les natures payees au mois, les champs `si_permanents` des fiches, les contrats de prevoyance et de
/// sante des permanents (`organismes`), les etats qui ne concernent qu'eux. La page ne le deduit jamais : elle le lit
/// (`configuration.permanents`) ou, la case a peine cochee, le redemande (route `permanents`).
pub fn emploie_des_permanents(entreprise: &ChampsEntreprise) -> bool {
    entreprise.emploie_des_permanents.oui()
}

/// LA CLE D'UN SALARIE : celle de sa fiche dans `config/salaries.json`, interne au logiciel (aucun texte n'en fixe la
/// forme). Elle se retrouve d'un nom par `Contexte::cle_du_salarie`, ou se lit d'un fichier, d'une charge (`lire`, ou
/// `serde` : la meme chaine) ; un type a part, pour qu'elle ne se confonde pas avec un autre texte (un nom, un numero).
/// INFERENCE du code : une cle n'est jamais vide (`moteur::routes::fiches::cle_neuve` refuse de nommer une fiche sans
/// nom) ; rien de plus ne vaut pour toutes les cles qui existent (celles qu'un import ou un jeu d'essai a ecrites).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdSalarie(String);

impl IdSalarie {
    /// La cle ecrite `texte` ; vide, elle se refuse.
    pub fn lire(texte: &str) -> Resultat<IdSalarie> {
        if texte.is_empty() {
            return Err("cle de salarie vide".to_string());
        }
        Ok(IdSalarie(texte.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for IdSalarie {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for IdSalarie {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for IdSalarie {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IdSalarie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for IdSalarie {
    type Error = String;
    fn try_from(s: String) -> Resultat<IdSalarie> {
        IdSalarie::lire(&s)
    }
}

impl From<IdSalarie> for String {
    fn from(c: IdSalarie) -> String {
        c.0
    }
}


impl PartialEq<str> for IdSalarie {
    fn eq(&self, autre: &str) -> bool {
        self.0 == autre
    }
}

impl PartialEq<&str> for IdSalarie {
    fn eq(&self, autre: &&str) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<String> for IdSalarie {
    fn eq(&self, autre: &String) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<IdSalarie> for &str {
    fn eq(&self, autre: &IdSalarie) -> bool {
        *self == autre.0
    }
}

impl PartialEq<IdSalarie> for String {
    fn eq(&self, autre: &IdSalarie) -> bool {
        *self == autre.0
    }
}

/// LA FICHE ENTREPRISE QU'UN DOCUMENT LIT : ses cases, et le dossier `gabarits/` de CETTE compagnie, ou il trouve son
/// modele, la signature, le logo ou le cachet. La compagnie vient du contexte, jamais d'une variable du processus : la
/// fenetre sert plusieurs compagnies dans le meme processus.
#[derive(Debug, Clone)]
pub struct EntrepriseDuDocument {
    pub fiche: ChampsEntreprise,
    pub gabarits: PathBuf,
}

impl std::ops::Deref for EntrepriseDuDocument {
    type Target = ChampsEntreprise;
    fn deref(&self) -> &ChampsEntreprise {
        &self.fiche
    }
}

/// Les donnees de la compagnie, lues une fois, et resolues a une date (`lue_au`).
#[derive(Debug, Clone)]
pub struct Contexte {
    pub racine: PathBuf,
    /// `logiciel/donnees` : le socle des baremes est dans `code/baremes/`.
    pub code: PathBuf,
    /// Le jour de la declaration (celui ou l'on demande si un taux de PAS vaut encore).
    pub date: Jour,
    /// La date a laquelle la configuration est lue : `date`, ou la periode payee (`au`).
    pub lue_au: Jour,
    /// Quel jour on est, pour les gardes du calendrier (distinct de `date`).
    pub aujourdhui: Jour,
    /// La fiche entreprise a `lue_au`.
    pub entreprise: ChampsEntreprise,
    /// Les fiches telles qu'ecrites, datees, lues par leur forme : une periode se lit a sa date (`salarie_au`, `au`).
    fiches: Arc<Fiches>,
    /// Les fiches salaries a `lue_au` (celles qui existaient ce jour-la), dans l'ordre du fichier.
    pub salaries: IndexMap<IdSalarie, FicheSalarie>,
    /// Les fiches spectacles a `lue_au` (celles qui existaient ce jour-la), dans l'ordre du fichier.
    pub spectacles: IndexMap<String, ChampsSpectacle>,
    /// Le taux AT/MP d'une annee qu'une piece atteste, pour une compagnie qui n'en a pas encore pose
    /// (`avec_le_taux_atmp_lu`) ; en memoire seulement.
    atmp_lu: Option<(i32, Decimal)>,
}

/// LA PILE D'UN JOUR, et le `depuis` de chaque valeur datee qui s'y applique (`Contexte::regles_datees`).
#[derive(Debug)]
struct PileLue {
    pile: Arc<Pile>,
    regles: Vec<Jour>,
}

/// Les fiches d'une compagnie telles qu'ecrites, datees, lues par leur forme (`fiche::lire_fichier`)...
#[derive(Debug)]
struct Fiches {
    entreprise: FicheEntrepriseDatee,
    salaries: Salaries,
    spectacles: Spectacles,
}

/// Les fiches de `cfg`, lues une fois tant que leurs fichiers ne changent pas.
fn lire_fiches(cfg: &Path) -> Resultat<Arc<Fiches>> {
    type Signature = Option<(u128, u64)>;
    type Cache = Mutex<HashMap<PathBuf, ([Signature; 3], Arc<Fiches>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let chemins = [cfg.join("entreprise.json"), cfg.join("salaries.json"), cfg.join("spectacles.json")];
    let sig = chemins.clone().map(|p| socle::fichiers::signature(&p));
    let connue = sig.iter().all(Option::is_some);
    if connue {
        if let Some((vue, f)) = cache.lock().unwrap().get(cfg) {
            if *vue == sig {
                chemins.iter().for_each(|p| socle::fichiers::noter_si_besoin(p));
                return Ok(f.clone());
            }
        }
    }
    let entreprise = crate::fiche::lire_fichier(&chemins[0], ENTREPRISE)?;
    let salaries = crate::fiche::lire_fichier(&chemins[1], SALARIES)?;
    let spectacles = crate::fiche::lire_fichier(&chemins[2], SPECTACLES)?;
    let f = Arc::new(Fiches { entreprise, salaries, spectacles });
    if connue {
        cache.lock().unwrap().insert(cfg.to_path_buf(), (sig, f.clone()));
    }
    Ok(f)
}

/// Les fiches spectacles a `jour` : celles qui existaient ce jour-la.
fn spectacles_au(s: &Spectacles, jour: &Jour) -> IndexMap<String, ChampsSpectacle> {
    s.fiches.iter().map(|(k, f)| (k.clone(), f.au(jour))).filter(|(_, c)| !c.est_vide()).collect()
}

/// Les fiches salaries a `jour` : celles qui existaient ce jour-la.
fn salaries_au(s: &Salaries, jour: &Jour) -> IndexMap<IdSalarie, FicheSalarie> {
    s.fiches
        .iter()
        .map(|(id, f)| (id, f.au(jour)))
        .filter(|(_, c)| !c.est_vide())
        .map(|(id, champs)| (id.clone(), FicheSalarie { id: id.clone(), champs }))
        .collect()
}

/// Les fiches d'une compagnie et ce qu'elle a dit d'elle-meme, sous sa racine.
pub const ENTREPRISE: &str = "config/entreprise.json";
pub const SALARIES: &str = "config/salaries.json";
pub const SPECTACLES: &str = "config/spectacles.json";
pub const EFFECTIFS: &str = "config/effectifs.json";
/// Les baremes propres a la compagnie (`<annee>.json`), sous sa racine.
pub const BAREMES: &str = "config/baremes";

/// Combien d'annees en arriere le taux AT/MP d'une annee que la compagnie n'a pas dite se cherche.
const ANNEES_DU_TAUX_ATMP: i32 = 5;
/// Le fichier des organismes et contrats de la DSN, sous `config/`.
pub const ORGANISMES: &str = "organismes.json";

impl Contexte {
    /// Le contexte de `racine` a la date `date`. Sans `date`, celle de l'horloge du moteur (`jour_reel`).
    pub fn nouveau(racine: &Path, date: Option<Jour>) -> Resultat<Contexte> {
        let date = date.unwrap_or_else(jour_reel);
        let aujourdhui = aujourdhui_brut()?;
        let cfg = racine.join(socle::fichiers::CONFIG);
        let fiches = lire_fiches(&cfg)?;
        Ok(Contexte {
            racine: racine.to_path_buf(),
            code: dossier_du_code(),
            date,
            lue_au: date,
            aujourdhui,
            entreprise: fiches.entreprise.au(&date),
            salaries: salaries_au(&fiches.salaries, &date),
            spectacles: spectacles_au(&fiches.spectacles, &date),
            fiches,
            atmp_lu: None,
        })
    }

    /// LE CONTEXTE D'UNE PERIODE : la meme compagnie, sa configuration lue a `jour` (la fin de la periode payee). Le
    /// calcul d'une paie, la DSN d'un mois et leurs documents lisent tout par lui, jamais « tel qu'aujourd'hui ».
    pub fn au(&self, jour: &Jour) -> Resultat<Contexte> {
        if *jour == self.lue_au {
            return Ok(self.clone());
        }
        Ok(Contexte {
            racine: self.racine.clone(),
            code: self.code.clone(),
            date: self.date,
            lue_au: *jour,
            aujourdhui: self.aujourdhui,
            entreprise: self.fiches.entreprise.au(jour),
            salaries: salaries_au(&self.fiches.salaries, jour),
            spectacles: spectacles_au(&self.fiches.spectacles, jour),
            fiches: self.fiches.clone(),
            atmp_lu: self.atmp_lu.clone(),
        })
    }

    /// LE TAUX AT/MP QU'UNE PIECE ATTESTE : le meme contexte, qui lit pour l'annee `an` le taux AT/MP `valeur` (en
    /// pourcent) quand la compagnie n'a pas encore pose le sien (`config/baremes/<an>.json`). Ce taux est propre a
    /// l'etablissement, notifie par l'Assurance Maladie : il ne se devine pas, il se lit, ici du bulletin d'un autre
    /// logiciel qui l'a applique. Rien ne s'ecrit ; une compagnie qui a pose le sien garde le sien.
    pub fn avec_le_taux_atmp_lu(mut self, an: i32, valeur: Decimal) -> Contexte {
        // Le taux tel qu'un bareme l'ecrit : sa forme la plus courte (`socle::forme::decimal`).
        let ecrit = valeur.to_string().parse::<f64>().ok().and_then(|f| socle::num::dec_f64(f).ok()).unwrap_or(valeur);
        self.atmp_lu = Some((an, ecrit));
        self
    }

    /// Le contexte du mois `AAAA-MM` : sa configuration lue au dernier jour du mois.
    pub fn du_mois(&self, mois: &str) -> Resultat<Contexte> {
        self.au(&Mois::lire(mois)?.dernier_jour())
    }

    /// La fiche entreprise a `jour`.
    pub fn entreprise_au(&self, jour: &Jour) -> ChampsEntreprise {
        if *jour == self.lue_au {
            return self.entreprise.clone();
        }
        self.fiches.entreprise.au(jour)
    }

    /// La fiche salarie `cle` a `jour`. Refus : la fiche ne dit rien ce jour-la.
    pub fn salarie_au(&self, cle: &str, jour: &Jour) -> Resultat<FicheSalarie> {
        let (id, brute) = self.fiches.salaries.fiches.get_key_value(cle).ok_or_else(|| format!("Salarie inconnu : {}.", repr_texte(cle)))?;
        let champs = brute.au(jour);
        if champs.est_vide() {
            return Err(format!(
                "La fiche de {} ne dit rien au {} : sa premiere valeur est posterieure. Dater sa fiche a partir de ce jour \
(Configuration, « a partir du »).",
                cle,
                jour.jj_mm_aaaa()
            ));
        }
        Ok(FicheSalarie { id: id.clone(), champs })
    }

    /// Les cles des salaries qu'on peut encore embaucher (les archives restent dans `salaries`).
    pub fn salaries_actifs(&self) -> Vec<&str> {
        self.salaries.iter().filter(|(_, v)| !v.archivee()).map(|(k, _)| k.as_str()).collect()
    }

    /// La compagnie emploie-t-elle des permanents (des salaries payes au mois, contrats `cdi` ou `cdd`) ? Une decision de
    /// la fiche Entreprise (`emploie_des_permanents`), pas une deduction : absent vaut NON. C'est le fait qui ouvre tout ce
    /// qui ne vaut que pour eux, a l'ecran comme dans le calcul ; un engagement paye au mois est refuse sans lui
    /// (`engagement::charger`) : il n'y a donc pas de permanent sans cette case.
    pub fn emploie_des_permanents(&self) -> bool {
        emploie_des_permanents(&self.entreprise)
    }

    /// Idem pour les spectacles.
    pub fn spectacles_actifs(&self) -> Vec<&str> {
        self.spectacles.iter().filter(|(_, v)| !v.archivee()).map(|(k, _)| k.as_str()).collect()
    }

    /// L'effectif de la compagnie (`config/effectifs.json`).
    pub fn effectif(&self) -> Resultat<Effectif> {
        effectif_de(&self.racine, &self.entreprise)
    }

    /// Le 5 a partir de cinquante salaries, le 15 sinon
    /// (un effectif inconnu donne le 15).
    pub fn echeance_dsn(&self) -> Resultat<u32> {
        socle::fichiers::noter_valeur("aujourdhui", self.aujourdhui.iso());
        Ok(match self.effectif()?.atteint(50, self.aujourdhui.an) {
            Ok(true) => 5,
            _ => 15,
        })
    }

    /// LES CONVENTIONS QUE LE SOCLE SERT (`par_convention`), lues par leur forme : celles du socle livre de l'annee du
    /// jour, a defaut du plus recent livre. Elles ne dependent d'aucune compagnie : la fiche qui n'a pas encore choisi la
    /// sienne les propose.
    pub fn conventions_servies(&self) -> Resultat<BTreeMap<String, crate::forme_bareme::Convention>> {
        let dossier = self.code.join("baremes");
        let jour = self.aujourdhui;
        let an = match fichier_du_socle(&dossier, jour.an) {
            Some(_) => jour.an,
            None => annees_du_socle(&dossier)?.into_iter().max().ok_or_else(|| socle::echec::livree("baremes", "par_convention"))?,
        };
        let fichier = fichier_du_socle(&dossier, an).ok_or_else(|| socle::echec::livree("baremes", "par_convention"))?;
        // Le dernier jour de l'annee du socle quand il est d'une annee passee : ses valeurs datees y valent toutes.
        let a = if an == jour.an { jour } else { Jour::lire(&format!("{}-12-31", an))? };
        Ok(crate::forme_bareme::lire_le_socle(&fichier, &a, None)?.0.par_convention)
    }

    /// Le dossier des gabarits de la compagnie.
    pub fn gabarits(&self) -> PathBuf {
        self.racine.join("gabarits")
    }

    /// La fiche entreprise de ce contexte, telle qu'un document la lit.
    pub fn entreprise_et_gabarits(&self) -> EntrepriseDuDocument {
        self.pour_un_modele(&self.entreprise)
    }

    /// Ce dont depend le bareme lu a `jour` : la compagnie, sa fiche entreprise a ce jour, et la signature (date, taille)
    /// des fichiers qui le composent.
    pub fn empreinte_bareme(&self, jour: &Jour) -> Resultat<String> {
        Ok(self.empreinte_bareme_de(jour.an, &self.entreprise_au(jour)))
    }

    fn empreinte_bareme_de(&self, an: i32, ent: &ChampsEntreprise) -> String {
        let cfg = self.racine.join(socle::fichiers::CONFIG);
        // Les surcouches des annees d'avant : le taux AT/MP que l'annee ne dit pas s'en reprend (`taux_atmp_reporte`).
        let sig: Vec<_> = (an - ANNEES_DU_TAUX_ATMP..=an)
            .flat_map(|y| [socle::fichiers::signature(&cfg.join("baremes").join(format!("{}.json", y))), socle::fichiers::signature(&self.code.join("baremes").join(format!("{}.json", y)))])
            .chain([socle::fichiers::signature(&cfg.join("effectifs.json"))])
            .collect();
        format!("{}|{}|{}|{:?}|{:?}|{:?}", self.racine.display(), self.code.display(), an, sig, ent, self.atmp_lu)
    }

    /// LA PILE DE `jour` : le socle livre + la couche de la convention que la fiche `ent` nomme + la surcouche de la
    /// compagnie, lus a ce jour et confrontes aux faits d'entreprise de cette fiche. `ent` est la fiche entreprise au jour
    /// lu : une compagnie qui change de convention en cours d'annee lit chaque jour sous la sienne. Garde tant que les
    /// fichiers et la fiche sont les memes, avec ce qu'il a lu (`garde_notee`) ; un refus ne se garde pas.
    fn pile_lue(&self, jour: &Jour, ent: &ChampsEntreprise) -> Result<Arc<PileLue>, Refus> {
        type Cache = Mutex<HashMap<String, socle::fichiers::Notee<Arc<PileLue>>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let cle = format!("{}|{}", self.empreinte_bareme_de(jour.an, ent), jour.iso());
        socle::fichiers::garde_notee(cache, cle, || {
            let (pile, regles) = self.empiler(jour, ent)?;
            // Le controle est ici, le seul endroit qui compose un bareme.
            controler(&pile, ent, &effectif_de(&self.racine, ent)?, &format!("config/baremes/{}.json", jour.an))?;
            Ok(Arc::new(PileLue { pile: Arc::new(pile), regles }))
        })
    }

    /// LES TROIS ETAGES EMPILES A `jour`, avant leur controle : le socle de l'annee que la surcouche reprend, la couche de
    /// la convention que la fiche `ent` nomme, la surcouche de la compagnie ; et le `depuis` de chaque valeur datee qui s'y
    /// applique (le SMIC compris).
    fn empiler(&self, jour: &Jour, ent: &ChampsEntreprise) -> Result<(Pile, Vec<Jour>), Refus> {
        let an = jour.an;
        let ou = format!("config/baremes/{}.json", an);
        let dossier = self.code.join("baremes");
        let organisation = ent.organisation_patronale.dite().map(String::as_str);
        let p = self.racine.join(BAREMES).join(format!("{}.json", an));
        let mut regles = Vec::new();
        let mut surcouche = if socle::fichiers::existe(&p) {
            Some(crate::forme_bareme::lire_la_surcouche(&p)?)
        } else {
            match &self.atmp_lu {
                Some((a, v)) if *a == an => Some(Surcouche {
                    annee: an,
                    socle: an,
                    taux_atmp: Some(TauxAtmpEcrit { valeur: Some(ValeurAtmp::Seule(socle::forme::Nombre(*v))), ..TauxAtmpEcrit::default() }),
                    ..Surcouche::default()
                }),
                _ => None,
            }
        };
        // Le taux AT/MP que la compagnie n'a pas dit pour l'annee se lit de celui qu'elle a dit avant
        // (`bareme::taux_atmp_reporte`) ; ce qui ne se reprend pas dit pourquoi, et ce qu'il faut saisir.
        if !surcouche.as_ref().is_some_and(|s| s.taux_atmp.is_some()) {
            match self.taux_atmp_reporte_de(an, ent) {
                Some(Ok(t)) => surcouche.get_or_insert_with(|| Surcouche { annee: an, socle: an, ..Surcouche::default() }).taux_atmp = Some(t),
                Some(Err(pourquoi)) => match surcouche.as_mut() {
                    Some(s) => s.taux_atmp = Some(TauxAtmpEcrit { manque: Some(pourquoi), ..TauxAtmpEcrit::default() }),
                    None if fichier_du_socle(&dossier, an).is_some() => return Err(introuvable(pourquoi)),
                    None => {}
                },
                None => {}
            }
        }
        let Some(surcouche) = surcouche else {
            if fichier_du_socle(&dossier, an).is_none() {
                return Err(introuvable(format!(
                    "Le logiciel n'a pas les taux de {} (SMIC, plafond de la Sécurité sociale, cotisations) : il a ceux de {}. Une paie de {} ne se calcule pas avec cette version.",
                    an,
                    disponibles(&dossier),
                    an
                )));
            }
            return Err(introuvable(format!(
                "Le code risque et le taux AT/MP de {} ne sont pas dits. Ils se lisent sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                an
            )));
        };
        let Some(fichier) = fichier_du_socle(&dossier, surcouche.socle) else {
            return Err(introuvable(format!(
                "Bareme {} : {} demande le socle {}, qui n'est pas livre.\n\
\x20 Socles disponibles : {}. Un socle neuf s'ecrit en janvier, \
depuis le plus recent.",
                an,
                ou,
                surcouche.socle,
                disponibles(&dossier)
            )));
        };
        // Les trois etages, dans l'ordre : droit commun, convention, compagnie.
        let (mut pile, r) = crate::forme_bareme::lire_le_socle(&fichier, jour, organisation)?;
        regles.extend(r);
        regles.extend(smic_en_vigueur(&pile.smic_horaire, jour)?.map(|(j, _)| j));
        if let Some(idcc) = ent.idcc.portee() {
            if let Some(c) = convention(&pile, idcc)?.cloned() {
                fondre_la_convention(&mut pile, &c, idcc)?;
            }
        }
        regles.extend(fondre_la_surcouche(&mut pile, surcouche, jour, &ou)?);
        regles.sort();
        regles.dedup();
        Ok((pile, regles))
    }

    /// LE TAUX AT/MP DE L'ANNEE `an` QUE LA COMPAGNIE N'A PAS DIT, d'apres le dernier qu'elle a dit dans les annees d'avant
    /// (sa surcouche `config/baremes/<annee>.json`, `ANNEES_DU_TAUX_ATMP` au plus) : `bareme::taux_atmp_reporte`. `None`
    /// : aucun code dit avant.
    pub fn taux_atmp_reporte(&self, an: i32) -> Option<Result<TauxAtmpEcrit, String>> {
        self.taux_atmp_reporte_de(an, &self.entreprise)
    }

    /// LE TAUX AT/MP DE L'ANNEE `an` TEL QUE LE BAREME LE LIT (`bareme_brut_sans_cache`) : celui que la surcouche de
    /// l'annee dit (`config/baremes/<an>.json`, `taux_atmp`), sinon celui qui se reprend d'une annee d'avant
    /// (`taux_atmp_reporte`, qui porte `_reporte_de`). `Some(Err)` : pourquoi il ne se reprend pas, ou la surcouche
    /// illisible ; `None` : rien de dit ni d'avant. Tout ce qui demande « le taux de l'annee est-il pose ? » lit ici.
    pub fn taux_atmp_de(&self, an: i32) -> Option<Result<TauxAtmpEcrit, String>> {
        let p = self.racine.join(BAREMES).join(format!("{}.json", an));
        if socle::fichiers::existe(&p) {
            match crate::forme_bareme::lire_la_surcouche(&p) {
                Ok(Surcouche { taux_atmp: Some(t), .. }) => return Some(Ok(t)),
                Ok(_) => {}
                Err(e) => return Some(Err(e)),
            }
        }
        self.taux_atmp_reporte(an)
    }

    fn taux_atmp_reporte_de(&self, an: i32, ent: &ChampsEntreprise) -> Option<Result<TauxAtmpEcrit, String>> {
        let dossier = self.racine.join(BAREMES);
        let (pa, pose) = (an - ANNEES_DU_TAUX_ATMP..an).rev().find_map(|y| {
            let p = dossier.join(format!("{}.json", y));
            let t = socle::fichiers::existe(&p).then(|| crate::forme_bareme::lire_la_surcouche(&p).ok()).flatten()?.taux_atmp?;
            t.code_risque.as_deref().is_some_and(|c| !c.trim().is_empty()).then_some((y, t))
        })?;
        let socle_an = socle_livre_lu(an).ok().flatten();
        let socle_avant = socle_livre_lu(pa).ok().flatten();
        let alsace = crate::bareme::en_alsace_moselle(ent.code_postal.texte());
        crate::bareme::taux_atmp_reporte(an, socle_an.as_ref(), (pa, &pose), socle_avant.as_ref(), alsace)
    }

    /// LE BAREME DE `jour` AVANT SON CONTROLE (le socle, la convention, la surcouche de la compagnie) : ce contre quoi une
    /// fiche qu'on enregistre se controle avant d'etre ecrite (`controler_etablissements`).
    pub fn bareme_fondu(&self, jour: &Jour) -> Resultat<Pile> {
        self.empiler(jour, &self.entreprise).map(|(p, _)| p).map_err(|r| r.message)
    }

    /// LES REGLES DATEES QUI S'APPLIQUENT A CE JOUR : le `depuis` de chaque valeur datee en vigueur du bareme de
    /// l'annee (le socle, la convention, la compagnie), du plus ancien au plus recent, sans doublon. Une paie qui s'est
    /// calculee sous d'autres regles le dit a sa date (« regle du JJ/MM/AAAA »).
    pub fn regles_datees(&self, jour: &Jour) -> Resultat<Vec<Jour>> {
        Ok(self.pile_lue(jour, &self.entreprise_au(jour)).map_err(|r| r.message)?.regles.clone())
    }

    /// LES VALEURS DATEES DE LA COMPAGNIE QUI S'APPLIQUENT A CE JOUR a l'engagement du salarie `salarie` sur le spectacle
    /// `spectacle` : le `depuis` de la valeur en vigueur de chaque liste datee de leurs fiches et de celle de l'entreprise,
    /// du plus ancien au plus recent, sans doublon. Une paie qui s'est calculee avant qu'une de ces valeurs s'applique le
    /// dit a sa date (« règle du JJ/MM/AAAA »).
    pub fn configuration_datee(&self, jour: &Jour, salarie: &str, spectacle: Option<&str>) -> Resultat<Vec<Jour>> {
        let mut out = self.fiches.entreprise.dates_en_vigueur(jour);
        out.extend(self.fiches.salaries.fiches.get(salarie).map_or_else(Vec::new, |f| f.dates_en_vigueur(jour)));
        out.extend(spectacle.and_then(|s| self.fiches.spectacles.fiches.get(s)).map_or_else(Vec::new, |f| f.dates_en_vigueur(jour)));
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// LES CONTRIBUTIONS DE BRANCHE QUE LA DSN DECLARE A CE JOUR : `contributions_dsn` de la couche de la convention que
    /// la fiche entreprise nomme ce jour-la, dans le socle de l'annee (la meme liste que le bareme fondu porte). Elles ne
    /// dependent que de la convention : un bareme que l'effectif ou le taux AT/MP rendent illisible les a quand meme.
    /// Une annee sans socle, ou une fiche sans convention couverte, n'en a aucune (le refus se dit au bareme).
    pub fn contributions_dsn(&self, jour: &Jour) -> Resultat<Vec<crate::forme_bareme::ContributionDsn>> {
        let Some(fichier) = fichier_du_socle(&self.code.join("baremes"), jour.an) else { return Ok(Vec::new()) };
        let ent = self.entreprise_au(jour);
        let Some(idcc) = ent.idcc.portee() else { return Ok(Vec::new()) };
        let (socle_an, _) = crate::forme_bareme::lire_le_socle(&fichier, jour, ent.organisation_patronale.dite().map(String::as_str))?;
        let Ok(Some(couche)) = convention(&socle_an, idcc) else { return Ok(Vec::new()) };
        Ok(couche.contributions_dsn.clone())
    }

    /// Le bareme resolu a ce jour, garde et partage : il se lit sans se copier ; ce qui le change (la pile d'un contrat,
    /// `bareme::bareme_applique` ; la surcharge d'un etablissement) en part.
    pub fn bareme(&self, jour: &Jour) -> Resultat<Arc<Pile>> {
        Ok(self.pile_lue(jour, &self.entreprise_au(jour)).map_err(|r| r.message)?.pile.clone())
    }

    /// LA COMPAGNIE TELLE QUE LA PILE DU BAREME LA LIT a `jour` (`bareme::bareme_applique`) : sa fiche a ce jour, et son
    /// effectif de l'annee.
    pub fn entreprise_lue(&self, jour: &Jour) -> Resultat<crate::bareme::EntrepriseLue> {
        let fiche = self.entreprise_au(jour);
        let effectif = effectif_de(&self.racine, &fiche)?.valeur(jour.an);
        Ok(crate::bareme::EntrepriseLue { fiche, effectif, taux_lus: false })
    }

    /// (PSS mensuel, PSS horaire) de l'annee, d'apres la fiche entreprise de ce contexte.
    pub fn plafond_securite_sociale(&self, annee: i32) -> Resultat<(i64, i64)> {
        let jour = Jour::nouveau(annee, 1, 1).ok_or_else(|| format!("Année {} hors du calendrier.", annee))?;
        let lue = self.pile_lue(&jour, &self.entreprise).map_err(|r| r.message)?;
        let entier = |x: Decimal| x.trunc().to_string().parse::<i64>().ok().filter(|n| *n != 0);
        match lue.pile.plafond_securite_sociale.as_ref().and_then(|p| entier(p.mensuel).zip(entier(p.horaire))) {
            Some(mh) => Ok(mh),
            None => Err(socle::echec::avec(
                "Le barème de l'année n'a pas le plafond de la Sécurité sociale : le signaler au développeur.",
                APart {
                    detail: Some(format!(
                        "Pas de plafond de Securite sociale pour {} dans \
config/baremes/{}.json : ajouter la section \
plafond_securite_sociale ({{\"mensuel\": ..., \"horaire\": ...}}), \
relevee sur la table des parametres du BOSS (PSS_M_METROP, \
PSS_H_METROP).",
                        annee, annee
                    )),
                    ..APart::default()
                },
            )),
        }
    }

    /// Le SMIC horaire applicable a la date (le bareme liste les revalorisations ; la derniere dont `depuis <= date`
    /// gagne).
    pub fn smic(&self, jour: &Jour) -> Resultat<f64> {
        let lue = self.pile_lue(jour, &self.entreprise_au(jour)).map_err(|r| r.message)?;
        match smic_en_vigueur(&lue.pile.smic_horaire, jour)? {
            Some((_, v)) => v.to_string().parse::<f64>().map_err(|e| e.to_string()),
            None => Err(format!("Aucun SMIC applicable au {} dans le bareme.", jour.iso())),
        }
    }

    /// Les autres etablissements que la fiche dit en dernier (ouverts ou fermes depuis), hors le siege : ce que l'ecran en
    /// montre.
    pub fn etablissements_dits(&self) -> Vec<crate::fiche::Etablissement> {
        let tout = Jour::nouveau(9999, 12, 31).map(|j| self.fiches.entreprise.au(&j)).unwrap_or_default();
        let principal = self.entreprise.nic_principal();
        tout.etablissements.dite().into_iter().flatten().filter(|e| e.nic.texte() != principal).cloned().collect()
    }

    /// La fiche entreprise d'un mois `AAAA-MM` : lue au dernier jour du mois, avec chaque autre etablissement ouvert un
    /// jour au moins du mois (`etablissements_entre`).
    pub fn entreprise_du_mois(&self, mois: &str) -> Resultat<ChampsEntreprise> {
        let mois = Mois::lire(mois)?;
        let dernier = mois.dernier_jour();
        let mut ent = self.entreprise_au(&dernier);
        if let Some(l) = etablissements_entre(&self.fiches.entreprise.etablissements, &mois.premier_jour(), &dernier) {
            ent.etablissements = Case::Dite(l);
        }
        Ok(ent)
    }

    /// La fiche entreprise `ent` telle qu'un document de cette compagnie la lit (`EntrepriseDuDocument`).
    pub fn pour_un_modele(&self, ent: &ChampsEntreprise) -> EntrepriseDuDocument {
        EntrepriseDuDocument { fiche: ent.clone(), gabarits: self.gabarits() }
    }

    /// Le numero de licence de la fiche entreprise a la date (vide : pas de licence).
    pub fn licence(&self, jour: &Jour) -> String {
        self.entreprise_au(jour).licence_spectacle.texte().to_string()
    }

    /// La fiche entreprise telle qu'ecrite, datee.
    pub fn entreprise_brute(&self) -> &FicheEntrepriseDatee {
        &self.fiches.entreprise
    }

    /// La fiche salarie `cle` telle qu'ecrite, datee, `None` si la cle est inconnue.
    pub fn salarie_brut(&self, cle: &str) -> Option<&FicheSalarieDatee> {
        self.fiches.salaries.fiches.get(cle)
    }

    /// La fiche spectacle `cle` telle qu'ecrite, datee, `None` si la cle est inconnue.
    pub fn spectacle_brut(&self, cle: &str) -> Option<&FicheSpectacleDatee> {
        self.fiches.spectacles.fiches.get(cle)
    }

    /// LES ORGANISMES ET CONTRATS DE LA DSN a `lue_au`, composes depuis les textes livres et la configuration de cette
    /// date (convention, bareme, `config/organismes.json` pour ce que la compagnie a saisi) : `organismes::composer`.
    /// Ce qu'un texte donne entier vaut sans que la compagnie ait rien enregistre.
    pub fn organismes(&self) -> Resultat<crate::organismes::Organismes> {
        let (saisi, prises) = crate::organismes::saisis(self)?;
        crate::organismes::composer(self, &saisi, prises, &std::collections::BTreeMap::new(), None)
    }

    // -------------------------------------------------------------- recherche

    /// Le salarie de cette cle tel que l'ecran le nomme (`socle::texte::salarie_nomme` : « NOM Prénom ») ; la cle sans
    /// fiche.
    pub fn nomme(&self, cle: &str) -> String {
        self.salaries.get(cle).and_then(|f| f.nomme()).unwrap_or_else(|| cle.to_string())
    }

    /// Retrouve un salarie par cle, NIR, nom, prenom ou
    /// faute de frappe. Le nom EXACT retrouve n'importe qui, archive compris ;
    /// la recherche APPROCHANTE (prefixe, Levenshtein <= 3) ne va que parmi les actifs.
    /// Rend la fiche (qui porte sa cle), ou le refus.
    pub fn salarie(&self, nom: &str) -> Resultat<FicheSalarie> {
        let cle = self.cle_du_salarie(nom)?;
        Ok(self.salaries[cle.as_str()].clone())
    }

    /// La cle de la fiche que `salarie` retrouve, sans la recopier ; la cle meme se retrouve d'abord.
    pub fn cle_du_salarie(&self, nom: &str) -> Resultat<IdSalarie> {
        IdSalarie::lire(&self.cle_trouvee(nom)?)
    }

    fn cle_trouvee(&self, nom: &str) -> Resultat<String> {
        if self.salaries.contains_key(nom) {
            return Ok(nom.to_string());
        }
        let q = cle_recherche(nom);
        for cle in self.salaries.keys() {
            if cle_recherche(cle) == q {
                return Ok(cle.to_string());
            }
        }
        // Le NIR, quand c'est lui qu'on donne : 13 caracteres, ou 15 avec la cle.
        let brut: String = nom.replace(' ', "").to_uppercase();
        if forme_de_nir(&brut) {
            for (cle, s) in &self.salaries {
                if s.nir.dite().is_some_and(|n| n.as_ref().to_uppercase() == brut[..13]) {
                    return Ok(cle.to_string());
                }
            }
        }
        let mut trouves: Vec<&IdSalarie> = Vec::new();
        for (cle, s) in &self.salaries {
            let (n, p) = (exige(&s.nom, "nom")?, exige(&s.prenom, "prenom")?);
            let ensemble = [cle_recherche(&n), cle_recherche(&p), cle_recherche(&format!("{} {}", n, p)), cle_recherche(&format!("{} {}", p, n))];
            if ensemble.contains(&q) {
                trouves.push(cle);
            }
        }
        if trouves.len() > 1 {
            return Err(format!(
                "Salarie ambigu : {} designe {} fiches ({}). L'appeler par son nom, ou par sa cle.",
                repr_texte(nom),
                trouves.len(),
                trouves.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(cle) = trouves.first() {
            return Ok(cle.to_string());
        }
        // Tolerance : prefixe (Celi -> CELIMENE) puis distance d'edition, sur les seuls actifs.
        let actifs = self.salaries_actifs();
        let noms_de = |cle: &str| -> Resultat<[String; 2]> {
            let s = &self.salaries[cle];
            Ok([cle_recherche(&exige(&s.prenom, "prenom")?), cle_recherche(&exige(&s.nom, "nom")?)])
        };
        let mut prefixes: Vec<&str> = Vec::new();
        for cle in &actifs {
            if q.chars().count() >= 3 && noms_de(cle)?.iter().any(|c| c.starts_with(&q)) {
                prefixes.push(cle);
            }
        }
        if prefixes.len() > 1 {
            return Err(format!(
                "Salarie ambigu : {} commence {} fiches ({}). Ecrire le nom en entier.",
                repr_texte(nom),
                prefixes.len(),
                prefixes.join(", ")
            ));
        }
        if let Some(cle) = prefixes.first() {
            return Ok(cle.to_string());
        }
        let (mut meilleur, mut score) = (None, 99);
        for cle in &actifs {
            for c in noms_de(cle)? {
                let dist = levenshtein(&q, &c);
                if dist < score {
                    meilleur = Some(*cle);
                    score = dist;
                }
            }
        }
        if let Some(cle) = meilleur.filter(|c| !c.is_empty() && score <= 3) {
            return Ok(cle.to_string());
        }
        // Rien parmi les actifs : si une archive ressemble, le dire plutot que « inconnu ».
        for (cle, s) in &self.salaries {
            if !s.archivee() {
                continue;
            }
            let (n, p) = (exige(&s.nom, "nom")?, exige(&s.prenom, "prenom")?);
            let ressemble = [p.as_str(), n.as_str(), cle.as_str()].iter().any(|x| {
                let c = cle_recherche(x);
                c.starts_with(&q) || levenshtein(&q, &c) <= 3
            });
            if ressemble {
                return Err(socle::echec::avec(format!("{} ressemble à {} {}, dont la fiche est archivée : pour l'embaucher à nouveau, rendre sa fiche active dans Configuration › Salariés.", repr_texte(nom), n, p), APart { cle: Some(cle.to_string()), detail: Some(format!(
                    "Salarie archive : {} ressemble a {} {} (cle {}), dont la fiche porte \
`actif: false`. L'appeler par sa cle exacte pour rejouer \
une paie passee ; pour l'embaucher a nouveau, remettre \
`actif: true` dans config/salaries.json.",
                    repr_texte(nom),
                    n,
                    p,
                    repr_texte(cle)
                )), ..APart::default() }));
            }
        }
        Err(format!("Salarie inconnu : {}. Connus : {}.", repr_texte(nom), actifs.join(", ")))
    }

    /// Par cle, titre, alias ou numero d'objet (les fiches actives d'abord), puis par inclusion dans un sens ou l'autre.
    /// Rend la fiche et sa cle, ou le refus.
    /// Un nom vide (ou blanc) ne designe AUCUN spectacle : la recherche par inclusion le trouverait
    /// dans tous (« » est dans tout titre) et rendrait le premier actif, au hasard -- c'est ainsi
    /// qu'un permanent repris se rattachait a un spectacle. Il se refuse.
    pub fn spectacle(&self, nom: &str) -> Resultat<Spectacle> {
        let q = cle_recherche(nom);
        if q.is_empty() && !self.spectacles.contains_key(nom.trim()) {
            return Err(format!(
                "Aucun spectacle n'est nomme ({}). Le spectacle porte le numero d'objet, qui part en DSN, \
                 en AEM et sur le bulletin : il se choisit, il ne se devine pas. Un contrat paye au mois \
                 (CDI, CDD de droit commun) n'en a pas.",
                repr_texte(nom)
            ));
        }
        let trouve = |cle: &str| Spectacle { cle: cle.to_string(), champs: self.spectacles[cle].clone() };
        let mut ordre: Vec<(&String, &ChampsSpectacle)> = self.spectacles.iter().collect();
        ordre.sort_by_key(|(_, v)| v.archivee()); // stable : les actives d'abord
        let net = nom.trim();
        if self.spectacles.contains_key(net) {
            return Ok(trouve(net));
        }
        let noms_de = |cle: &str, sp: &ChampsSpectacle| -> Resultat<Vec<String>> {
            let mut noms = vec![cle.to_string(), sp.titre.ecrite("titre")?.to_string()];
            noms.extend(sp.alias.dite().into_iter().flatten().cloned());
            Ok(noms)
        };
        for (cle, sp) in &ordre {
            if noms_de(cle, sp)?.iter().any(|n| cle_recherche(n) == q) {
                return Ok(trouve(cle));
            }
            if sp.numero_objet.texte().to_uppercase() == net.to_uppercase() {
                return Ok(trouve(cle));
            }
        }
        for (cle, sp) in &ordre {
            let ressemble = noms_de(cle, sp)?.iter().filter(|n| !n.is_empty()).any(|n| {
                let c = cle_recherche(n);
                c.contains(&q) || q.contains(&c)
            });
            if ressemble {
                return Ok(trouve(cle));
            }
        }
        let mut connus = Vec::new();
        for (_, v) in &ordre {
            connus.push(format!("« {} »", v.titre.ecrite("titre")?));
        }
        Err(format!(
            "Spectacle inconnu : {}. Connus : {}.\nUn spectacle se crée dans Configuration › Spectacles, \
avec son numéro d'objet France Travail.",
            repr_texte(nom),
            if connus.is_empty() { "aucun".to_string() } else { connus.join(", ") }
        ))
    }
}

/// Un NIR : 1 ou 2, quatre chiffres, le département (deux chiffres, 2A ou 2B), six chiffres, et la clé facultative.
fn forme_de_nir(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    if !(c.len() == 13 || c.len() == 15) || !matches!(c[0], '1' | '2') {
        return false;
    }
    let chiffres = |de: usize, a: usize| c[de..a].iter().all(char::is_ascii_digit);
    let dep = chiffres(5, 7) || (c[5] == '2' && matches!(c[6], 'A' | 'B'));
    chiffres(1, 5) && dep && chiffres(7, 13) && (c.len() == 13 || chiffres(13, 15))
}
