//! L'AVENANT D'UN CONTRAT PARTI : ce que le contrat dirait aujourd'hui, compare a ce qu'il disait sur la version de
//! l'engagement sur laquelle il est parti (`engagement::etats::version_au_depart`). L'ancien texte n'est pas relu dans le
//! PDF signe : il s'ecrit du modele avec les valeurs de la version d'avant, telles qu'elles etaient ecrites (regle 4), et
//! chaque paragraphe des articles du modele qui differe d'une version a l'autre est une modification (`modifications`).
//!
//! Ici, les valeurs que le modele de contrat lit dans une paie (`valeurs_de_la_paie`, `contrat.marqueurs`), le contrat
//! parti que le journal connait (`contrat_parti`) et ses avenants. Le texte de l'avenant vit dans le modele de sa nature
//! (`contrat_ecrit.avenant` de `contrats-base.json` : `avenant-cddu.modele`, `avenant-permanent.modele`), une donnee,
//! comme celui du contrat.


use std::collections::BTreeMap;
use std::path::Path;

use fabrique_pdf::modele::{self, Modele, Valeurs};
use serde_json::Value;

use crate::bulletin::{convention_du_contrat, fmt_eur};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::mois::Mois;
use droit::emplois::{emploi_de, EmploiEngagement};
use socle::fichiers::vrai;
use socle::nfkc::nfkc;
use socle::num::{dec, Resultat};


/// Ce qui remplace une valeur qui n'est pas un texte : elle ne se refuse que si le modele s'en
/// sert. Une phrase qui la porte est un refus.
const INVALIDE: &str = "\u{0}\u{0}VALEUR-SANS-TEXTE\u{0}\u{0}";


// ------------------------------------------------------ le texte qu'on compare

use socle::texte::blanc as est_blanc;

/// Ce qui se compare. Ligatures defaites (NFKC : « ﬀ » -> « ff », espaces
/// insecables -> espaces), une coupure apres un trait d'union recollee, les blancs ramenes a un seul.
pub fn normaliser(texte: &str) -> String {
    let t: Vec<char> = nfkc(texte).chars().collect();
    let mut recolle = String::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if t[i] == '-' && i + 2 < t.len() && t[i + 1] == '\n' && !est_blanc(t[i + 2]) {
            recolle.push('-');
            i += 2;
        } else {
            recolle.push(t[i]);
            i += 1;
        }
    }
    let mut sortie = String::with_capacity(recolle.len());
    let mut dans_blanc = false;
    for c in recolle.chars() {
        if est_blanc(c) {
            if !dans_blanc {
                sortie.push(' ');
            }
            dans_blanc = true;
        } else {
            sortie.push(c);
            dans_blanc = false;
        }
    }
    sortie.trim_matches(' ').to_string()
}

// ------------------------------------------------------ les valeurs du modele

pub(crate) fn requis<'a>(v: &'a Value, cle: &str) -> Resultat<&'a Value> {
    v.get(cle).ok_or_else(|| format!("le modele n'a pas de valeur {}", cle))
}

/// Une valeur posee telle quelle dans les marqueurs : un texte, ou de quoi refuser si le modele s'en sert.
pub(crate) fn texte_ou_invalide(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => INVALIDE.to_string(),
    }
}

/// Une case texte de la fiche du salarie, que le modele exige : absente, elle se refuse ; nulle, elle ne se lit pas.
fn case(c: &droit::fiche::Case<String>, cle: &str) -> Resultat<String> {
    match c {
        droit::fiche::Case::Nulle => Ok(INVALIDE.to_string()),
        c => c.ecrite(cle).map(str::to_string),
    }
}

/// La forme lisible d'une case de la fiche du salarie si elle est dite, la forme declaree sinon (`case`).
fn lisible(affichage: &droit::fiche::Case<String>, c: &droit::fiche::Case<String>, cle: &str) -> Resultat<String> {
    match affichage.texte() {
        "" => case(c, cle),
        a => Ok(a.to_string()),
    }
}

/// Forme lisible si elle existe, forme administrative sinon.
pub(crate) fn aff<'a>(fiche: &'a Value, cle: &str) -> Resultat<&'a Value> {
    match fiche.get(format!("{}_affichage", cle)) {
        Some(v) if vrai(v) => Ok(v),
        _ => requis(fiche, cle),
    }
}

/// « d' » devant une voyelle ou un h, « de » sinon.
pub(crate) fn de_devant(mot: &str) -> &'static str {
    let t: String = mot.chars().take(1).flat_map(char::to_lowercase).collect();
    if t.is_empty() || "aeiouyhâàéèêîôûœ".contains(&t) {
        "d'"
    } else {
        "de "
    }
}

/// « a, b et c ».
pub(crate) fn et(morceaux: &[String]) -> String {
    match morceaux.len() {
        0 => String::new(),
        1 => morceaux[0].clone(),
        n => format!("{} et {}", morceaux[..n - 1].join(", "), morceaux[n - 1]),
    }
}

/// Ce que `contrat.marqueurs` lit de l'engagement : la forme canonique du moteur
/// (`Engagement::canon`, dans la paie calculee).
pub(crate) struct Eng {
    debut: String,
    fin: Option<String>,
    mois: String,
    numero: Option<String>,
    dernier_mois: bool,
    dates_travail: Vec<String>,
    planning: Vec<String>,
    lieux: BTreeMap<String, String>,
    planning_lieux: BTreeMap<String, String>,
    lieu: String,
    date_paiement: Option<String>,
    cachets: String,
    /// `None` : un contrat sans cachet dont ni l'engagement ni le mois n'ecrivent de taux.
    taux_cachet: Option<String>,
}

impl Eng {
    pub(crate) fn lire(e: &Value) -> Resultat<Eng> {
        let texte = |c: &str| e.get(c).and_then(Value::as_str).map(str::to_string);
        let requis_texte = |c: &str| texte(c).ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", c));
        let liste = |c: &str| -> Resultat<Vec<String>> {
            e.get(c)
                .and_then(Value::as_array)
                .ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", c))?
                .iter()
                .map(|x| x.as_str().map(str::to_string).ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", c)))
                .collect()
        };
        let carte = |c: &str| -> Resultat<BTreeMap<String, String>> {
            Ok(e.get(c)
                .and_then(Value::as_object)
                .ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", c))?
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect())
        };
        Ok(Eng {
            debut: requis_texte("debut")?,
            fin: texte("fin"),
            mois: requis_texte("mois")?,
            numero: texte("numero"),
            dernier_mois: e.get("dernier_mois").and_then(Value::as_bool).ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", "dernier_mois"))?,
            dates_travail: liste("dates_travail")?,
            planning: liste("planning")?,
            lieux: carte("lieux")?,
            planning_lieux: carte("planning_lieux")?,
            lieu: requis_texte("lieu")?,
            date_paiement: texte("date_paiement"),
            cachets: requis_texte("cachets")?,
            taux_cachet: texte("taux_cachet"),
        })
    }

    /// `Engagement.dates_du_contrat` : les jours du mois, puis le planning.
    fn dates_du_contrat(&self) -> Vec<&String> {
        self.dates_travail.iter().chain(self.planning.iter()).collect()
    }

    /// `Engagement.lieu_de` : la date l'emporte, l'engagement supplee.
    fn lieu_de(&self, jour: &str) -> &str {
        [self.lieux.get(jour), self.planning_lieux.get(jour)]
            .into_iter()
            .flatten()
            .find(|l| !l.is_empty())
            .map_or(self.lieu.as_str(), String::as_str)
    }

    /// `Engagement.lieux_joues` : dans l'ordre des dates, sans repetition.
    fn lieux_joues(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            if !ou.is_empty() && !out.contains(&ou) {
                out.push(ou);
            }
        }
        out
    }

    /// `Engagement.dates_par_lieu()` : [(lieu, [dates])] dans l'ordre des dates, sans retrier.
    fn dates_par_lieu(&self) -> Vec<(&str, Vec<&String>)> {
        let mut out: Vec<(&str, Vec<&String>)> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            match out.last_mut() {
                Some((dernier, dates)) if *dernier == ou => dates.push(j),
                _ => out.push((ou, vec![j])),
            }
        }
        out
    }
}

/// « 2 cachets à 130,00 € » : une ligne de remuneration telle que le contrat la cite.
pub(crate) fn ligne_citee(l: &Value) -> Resultat<String> {
    let designation = requis(l, "designation")?.as_str().ok_or("designation : un texte est attendu")?;
    // La designation ouvre une phrase : « 2 Cachets » y devient « 2 cachets ». On ne touche
    // qu'a la premiere majuscule.
    let mut texte = String::new();
    let mut fait = false;
    for c in designation.chars() {
        if !fait && c.is_uppercase() {
            texte.extend(c.to_lowercase());
            fait = true;
        } else {
            texte.push(c);
        }
    }
    Ok(format!("{} à {}", texte, socle::texte::euros(dec(requis(l, "taux")?)?)))
}

/// La ligne fait-elle le brut ? (`soumis` de la rubrique ; une paie qui ne le dit pas, si.)
pub(crate) fn fait_le_brut(l: &Value) -> bool {
    l.get("soumis").and_then(Value::as_bool).unwrap_or(true)
}

/// « 2 cachets à 130,00 € », ou « 1 journée de travail à
/// 200,00 € et 6 h de masterclass à 40,00 € ». Le modele la dit « brut » : elle ne cite que les
/// lignes qui font le brut ; les autres (les defraiements...) sont dans `remuneration_hors_brut`.
fn remuneration_detaillee(remuneration: &[Value], e: &Eng) -> Resultat<String> {
    let mut morceaux = Vec::new();
    for l in remuneration.iter().filter(|l| fait_le_brut(l)) {
        morceaux.push(ligne_citee(l)?);
    }
    let mut phrase = et(&morceaux);
    if !e.dernier_mois {
        let quand = Mois::lire(&e.mois)?.libelle(socle::mois::Graphie::Accentuee);
        if !e.planning.is_empty() {
            let n = e.planning.len();
            phrase += &format!(
                " pour {} et, pour {} du planning, payée{} dans {} mois, le même montant",
                quand,
                if n > 1 { format!("les {} dates suivantes", n) } else { "la date suivante".to_string() },
                if n > 1 { "s" } else { "" },
                if n > 1 { "leur" } else { "son" }
            );
        } else {
            phrase += &format!(" pour {} et, pour les dates suivantes du planning, payées dans leur mois, le même montant", quand);
        }
    }
    Ok(phrase)
}

/// Ce que le contrat verse EN PLUS du brut -- les lignes non soumises qui s'ajoutent (defraiements,
/// indemnites hors assiette), convenues comme le reste : « , auxquels s'ajoutent 3 défraiements au
/// forfait à 20,20 € hors brut ». Vide sans elles : la phrase du modele reste alors mot pour mot
/// celle d'avant. Une ligne qui retranche (un acompte) n'est pas une clause du contrat.
pub(crate) fn remuneration_hors_brut(remuneration: &[Value]) -> Resultat<String> {
    let mut morceaux = Vec::new();
    for l in remuneration.iter().filter(|l| !fait_le_brut(l)) {
        if dec(requis(l, "montant")?)? > socle::num::Decimal::ZERO {
            morceaux.push(ligne_citee(l)?);
        }
    }
    Ok(if morceaux.is_empty() { String::new() } else { format!(", auxquels s'ajoutent {} hors brut", et(&morceaux)) })
}

/// « pour le mois d'octobre 2026 » quand le contrat se poursuit.
fn periode_remuneration(e: &Eng) -> Resultat<String> {
    if e.dernier_mois {
        return Ok(String::new());
    }
    let mois = Mois::lire(&e.mois)?;
    let nom = mois.nom(socle::mois::Graphie::Accentuee);
    let de = if nom.chars().next().map_or(false, |c| "aeiouyâàéèêîôû".contains(c)) { "d'" } else { "de " };
    Ok(format!(" pour le mois {}{} {}", de, nom, mois.annee()))
}

/// Les jours joues, et OU chacun se joue quand ils different.
fn dates_travail(e: &Eng) -> Resultat<String> {
    let mut texte = if e.lieux_joues().len() < 2 {
        let v: Resultat<Vec<String>> = e.dates_du_contrat().into_iter().map(|j| Jour::lire(j).map(|x| x.jj_mm_aaaa())).collect();
        v?.join(", ")
    } else {
        let mut groupes = Vec::new();
        for (ou, jours) in e.dates_par_lieu() {
            let v: Resultat<Vec<String>> = jours.into_iter().map(|j| Jour::lire(j).map(|x| x.jj_mm_aaaa())).collect();
            let dates = et(&v?);
            groupes.push(if ou.is_empty() { dates } else { format!("{} à {}", dates, ou) });
        }
        groupes.join(", ")
    };
    if !e.dernier_mois && e.planning.is_empty() {
        let fin = e.fin.as_deref().ok_or("contrat sans fin")?;
        texte += &format!(", puis les dates du planning jusqu'au {}", Jour::lire(fin)?.jj_mm_aaaa());
    }
    Ok(texte)
}

/// L'article « Lieu de travail », qui suit ce que les dates ont dit.
fn lieu_du_contrat(e: &Eng) -> String {
    let lieux = e.lieux_joues();
    match lieux.len() {
        0 => "au lieu de la représentation".to_string(),
        1 => format!("à {}", lieux[0]),
        _ => "aux lieux indiqués ci-dessus".to_string(),
    }
}

/// Les valeurs offertes au modele. `remuneration`, `brut` et `net` sont ceux
/// de la paie calculee ; `fiche` celle du salarie telle qu'elle vaut pour cet engagement.
fn marqueurs(ent: &Value, fiche: &droit::fiche::ChampsSalarie, sp: &Value, e: &Eng, remuneration: &[Value], brut: &Value, net: &Value) -> Resultat<Valeurs> {
    let licence = match requis(ent, "licence_spectacle")? {
        Value::String(s) => s.clone(),
        autre => return Err(socle::echec::a_la_cle("Le numéro de licence de la compagnie ne se lit pas pour ce mois : le vérifier dans Configuration › Entreprise.", "licence_spectacle", en_texte(autre))),
    };
    let mut m = parties(ent, fiche, sp, e)?;
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };
    poser("employeur.licence", licence);
    // Le numero sous lequel la compagnie est affiliee a la Caisse : son numero d'adherent (`conges_spectacles::adherent`).
    poser("employeur.congesspectacles", crate::conges_spectacles::adherent(ent)?);
    poser("contrat.fin", Jour::lire(e.fin.as_deref().ok_or("contrat sans fin")?)?.jj_mm_aaaa());
    poser("contrat.dates_travail", dates_travail(e)?);
    poser("contrat.lieu", lieu_du_contrat(e));
    // Le jour du versement de la paie (`engagement::Engagement::date_du_versement`, que la paie ecrite porte).
    let paiement = e.date_paiement.as_deref().filter(|p| !p.is_empty()).ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", "date_paiement"))?;
    poser("contrat.date_paiement", Jour::lire(paiement)?.jj_mm_aaaa());
    poser("remuneration.detail", remuneration_detaillee(remuneration, e)?);
    poser("remuneration.periode", periode_remuneration(e)?);
    poser("remuneration.hors_brut", remuneration_hors_brut(remuneration)?);
    poser("remuneration.cachets", e.cachets.clone());
    // Sans taux de cachet, pas de marqueur : un modele qui le cite se refuse (`contrat::refuser_ce_que_le_modele_tairait`).
    if let Some(t) = &e.taux_cachet {
        poser("remuneration.taux_cachet", fmt_eur(dec(&Value::String(t.clone()))?));
    }
    poser("remuneration.brut", fmt_eur(dec(brut)?));
    poser("remuneration.net_estime", fmt_eur(dec(net)?));
    poser("spectacle.titre", texte_ou_invalide(requis(sp, "titre")?));
    // LE NUMERO D'OBJET se reporte sur le contrat « a chaque fois que cela est possible » (annexes VIII et X, art. 47 § 3 :
    // atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt l. 1183-1184, et l'annexe X,
    // l. 1161-1162) : un spectacle qui n'en a pas encore fait son contrat, qui n'en dit rien. Le bulletin, lui, l'exige
    // (`exige_par` de `champs-base.json`).
    let objet = sp.get("numero_objet").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
    poser("spectacle.objet", objet.map(|n| format!(" (numéro d'objet France Travail {})", n)).unwrap_or_default());
    Ok(m)
}

/// Les parties au contrat, quelle que soit sa nature : l'employeur, sa convention et son signataire, le salarie, le
/// numero et le premier jour du contrat. `sp` : le spectacle (`Null` pour un contrat qui n'en a pas). La convention se
/// nomme comme le bareme du mois de la paie qui ecrit le document la nomme (`bareme::libelle_convention`) : le premier
/// mois d'un contrat pour le contrat, le mois de l'avenant pour un avenant -- un contrat commence avant le logiciel n'a
/// pas de bareme de son annee.
pub(crate) fn parties(ent: &Value, fiche: &droit::fiche::ChampsSalarie, sp: &Value, e: &Eng) -> Resultat<Valeurs> {
    let (idcc, convention) = convention_du_contrat(ent, sp, fiche, &Mois::lire(&e.mois)?.premier_jour())?;
    let sig = requis(ent, "signataire")?;
    let mut m = Valeurs::new();
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };
    poser("employeur.raison_sociale", texte_ou_invalide(requis(ent, "raison_sociale")?));
    poser("employeur.adresse", texte_ou_invalide(aff(ent, "adresse")?));
    poser("employeur.code_postal", texte_ou_invalide(requis(ent, "code_postal")?));
    poser("employeur.commune", texte_ou_invalide(aff(ent, "commune")?));
    poser("employeur.siret", texte_ou_invalide(requis(ent, "siret")?));
    poser("employeur.naf", texte_ou_invalide(requis(ent, "naf")?));
    poser("employeur.idcc", idcc);
    poser("employeur.convention", convention);
    poser(
        "employeur.signataire",
        format!("{} {}", en_texte(aff(sig, "prenom")?), en_texte(aff(sig, "nom")?)),
    );
    let fonction = texte_ou_invalide(requis(sig, "fonction")?);
    // « en qualité d'administratrice », « de président » : l'élision devant la fonction de qui signe.
    poser("employeur.qualite_de", de_devant(&fonction).to_string());
    poser("employeur.signataire_fonction", fonction);
    let s = fiche;
    poser("salarie.nom", case(&s.nom, "nom")?);
    poser("salarie.prenom", lisible(&s.prenom_affichage, &s.prenom, "prenom")?);
    poser("salarie.nom_complet", format!("{} {}", lisible(&s.prenom_affichage, &s.prenom, "prenom")?, case(&s.nom, "nom")?));
    poser("salarie.date_naissance", s.date_naissance.exigee("date_naissance")?.jj_mm_aaaa());
    poser("salarie.lieu_naissance", lisible(&s.lieu_naissance_affichage, &s.lieu_naissance, "lieu_naissance")?);
    poser("salarie.adresse", lisible(&s.adresse_affichage, &s.adresse, "adresse")?);
    poser("salarie.code_postal", case(&s.code_postal, "code_postal")?);
    poser("salarie.commune", lisible(&s.commune_affichage, &s.commune, "commune")?);
    // Le numero technique temporaire n'a rien a faire ici : un salarie pas encore immatricule lit
    // ce qui est vrai.
    poser(
        "salarie.nir",
        if !s.nir.texte().is_empty() {
            format!("{}{}", s.nir.texte(), s.nir_cle.ecrite("nir_cle")?)
        } else {
            "en cours d'immatriculation".to_string()
        },
    );
    let emploi_texte = lisible(&s.emploi_affichage, &s.emploi, "emploi")?;
    if emploi_texte == INVALIDE {
        return Err("l'emploi n'est pas un texte".to_string());
    }
    poser("salarie.emploi", emploi_texte.to_lowercase());
    poser("salarie.qualite_de", de_devant(&emploi_texte).to_string());
    // Un numero que le moteur ne porte pas (contrat pas encore numerote) n'est pas devine ni pris au compteur.
    poser(
        "contrat.numero",
        match e.numero.as_deref() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => INVALIDE.to_string(),
        },
    );
    poser("contrat.debut", Jour::lire(&e.debut)?.jj_mm_aaaa());
    Ok(m)
}

/// Le paragraphe tel qu'il se LIT sur le contrat imprime.
fn texte_brut(texte: &str, vals: &Valeurs) -> Resultat<String> {
    let t = modele::texte_brut(texte, vals).map_err(|r| r.0)?;
    if t.contains(INVALIDE) {
        return Err("une valeur du modele n'est pas un texte".to_string());
    }
    Ok(t)
}

// ------------------------------------------------- ce que l'avenant modifie

/// Un article du contrat qui a change depuis la version sur laquelle il est parti : son numero, son titre, et pour chaque
/// paragraphe change l'ancien texte, le nouveau et son rang dans l'article.
pub struct Modification {
    pub numero: usize,
    pub article: String,
    pub avant: Vec<Value>,
    pub apres: Vec<String>,
    pub blocs: Vec<usize>,
}

/// Les articles du modele `m` dont un paragraphe s'ecrit autrement avec les valeurs d'aujourd'hui (`apres`) qu'avec celles
/// de la version sur laquelle le contrat est parti (`avant`), dans l'ordre du modele.
pub fn modifications(m: &Modele, avant: &Valeurs, apres: &Valeurs) -> Resultat<Vec<Modification>> {
    let mut out = Vec::new();
    for (n, (titre, corps)) in m.articles.iter().enumerate() {
        let n = n + 1;
        let (mut anciens, mut neufs, mut blocs): (Vec<Value>, Vec<String>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
        for (i, bloc) in corps.iter().enumerate() {
            let (ancien, neuf) = (normaliser(&texte_brut(bloc, avant)?), normaliser(&texte_brut(bloc, apres)?));
            if ancien != neuf {
                anciens.push(Value::String(ancien));
                neufs.push(neuf);
                blocs.push(i);
            }
        }
        if !anciens.is_empty() {
            out.push(Modification { numero: n, article: texte_brut(titre, apres)?, avant: anciens, apres: neufs, blocs });
        }
    }
    Ok(out)
}

/// La fiche entreprise, avec la licence du mois et les gabarits de la compagnie.
pub fn ent_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Value> {
    let licence = ctx.licence(&Mois::lire(mois.get(..7).unwrap_or(mois))?.premier_jour())?;
    let mut ent = ctx.entreprise.clone();
    let o = ent.as_object_mut().ok_or("fiche entreprise : un objet est attendu")?;
    o.insert("licence_spectacle".into(), Value::String(licence));
    o.insert("_gabarits".into(), Value::String(ctx.gabarits().to_string_lossy().into_owned()));
    Ok(ent)
}

/// `contrat.marqueurs(p, ent)` pour une paie calculee (`paie`, sa forme JSON). `numero` : le numero
/// de contrat que la paie porte VRAIMENT (lu dans l'archive, ou celui de l'engagement), quand il
/// differe de celui de la reponse du calcul. Le contrat d'un spectacle lit ses dates, ses cachets et son spectacle ;
/// celui d'un permanent son salaire au mois et sa duree du travail (`contrat_ecrit::marqueurs_au_mois`) ; tous deux
/// les mentions que la loi exige de tout contrat ecrit (`contrat_ecrit::poser_les_mentions`).
pub fn valeurs_de_la_paie(ctx: &Contexte, ent: &Value, paie: &Value, numero: Option<&Option<String>>) -> Resultat<Valeurs> {
    let nul = Value::Null;
    let eng = requis(paie, "engagement")?;
    let nat = crate::contrat_ecrit::nature_de_la_paie(paie)?;
    let nom_spectacle = eng.get("spectacle").and_then(Value::as_str).filter(|s| !s.is_empty());
    let mut e = Eng::lire(eng)?;
    if let Some(n) = numero {
        e.numero = n.clone();
    }
    let texte = |c: &str| eng.get(c).and_then(Value::as_str).map(str::to_string).filter(|s| !s.is_empty());
    let fiche = ctx.salarie(eng.get("salarie").and_then(Value::as_str).ok_or("engagement sans salarie")?)?;
    let emploi = EmploiEngagement {
        emploi: texte("emploi"),
        emploi_affichage: texte("emploi_affichage"),
        pcs_ese: texte("pcs_ese"),
        code_emploi_ft: texte("code_emploi_ft"),
        statut: texte("statut"),
        statut_conventionnel: texte("statut_conventionnel"),
        statut_categoriel_rc: texte("statut_categoriel_rc"),
    };
    let lue = emploi_de(&ctx.salarie_au(fiche.id.as_str(), &Jour::lire(&e.debut)?)?, &emploi)?;
    let remuneration = requis(paie, "remuneration")?.as_array().ok_or("remuneration : une liste est attendue")?;
    let (brut, net) = (paie.get("brut").unwrap_or(&nul), paie.get("net_a_payer").unwrap_or(&nul));
    let mut m = match nom_spectacle {
        Some(s) => {
            let sp = ctx.spectacle(s)?;
            marqueurs(ent, &lue, &sp, &e, remuneration, brut, net)?
        }
        None => {
            let mut m = parties(ent, &lue, &nul, &e)?;
            crate::contrat_ecrit::marqueurs_au_mois(eng, &nat, remuneration, brut, net, &mut m)?;
            m
        }
    };
    crate::contrat_ecrit::poser_les_mentions(ctx, ent, &lue, eng, &nat, &mut m)?;
    Ok(m)
}

/// Le modele du contrat ecrit de la nature `nat`, celui de la compagnie s'il est dans ses gabarits, lu.
pub fn modele_du_contrat(ent: &Value, gabarits: &Path, nat: &droit::natures::Nature) -> Resultat<Modele> {
    let nom = nat.modele_contrat().ok_or_else(|| format!("La nature {} n'a pas de contrat écrit.", nat.libelle))?;
    modele::lire_fichier(&socle::fichiers::modele_de(&nom, ent, gabarits).to_string_lossy()).map_err(|r| r.0)
}

// ------------------------------------------- l'avenant d'un contrat signe ailleurs

/// Un terme du contrat, tel que l'avenant d'un contrat signe hors du logiciel le nomme : son titre, et son nom dans une
/// phrase.
fn terme_nomme(terme: &str) -> (&'static str, &'static str) {
    match terme {
        "salaire_mensuel" => ("Rémunération mensuelle brute", "la rémunération mensuelle brute"),
        "quotite_contrat" => ("Durée du travail", "la durée du travail"),
        "quotite_reference" => ("Durée de référence", "la durée de référence"),
        "modalite_temps_travail" => ("Temps de travail", "la modalité du temps de travail"),
        "emploi" => ("Emploi", "l'emploi"),
        "metier" => ("Métier", "le métier"),
        "cadre" => ("Statut de cadre", "le statut de cadre"),
        "statut" => ("Statut", "le statut"),
        "nature" => ("Nature du contrat", "la nature du contrat"),
        "motif_recours" => ("Motif de recours", "le motif de recours"),
        "lieu" => ("Lieu de travail", "le lieu de travail"),
        "spectacle" => ("Spectacle", "le spectacle"),
        _ => ("Clause du contrat", "une clause du contrat"),
    }
}

/// La valeur d'un terme en clair : un montant en euros, une duree en heures par mois (S21.G00.40.013), une nature, un
/// motif ou un spectacle par son nom.
fn valeur_en_clair(ctx: &Contexte, nat: &droit::natures::Nature, terme: &str, v: &Value) -> String {
    let texte = en_texte(v);
    match terme {
        "salaire_mensuel" => socle::texte::euros_de(v),
        "quotite_contrat" | "quotite_reference" => dec(v).map(|x| format!("{} heures par mois", socle::texte::virgule(x))).unwrap_or(texte),
        "cadre" => if vrai(v) { "cadre".into() } else { "non cadre".into() },
        "nature" => droit::natures::nature(Some(&texte)).map(|n| n.libelle.clone()).unwrap_or(texte),
        "motif_recours" => nat.libelle_motif_recours(&texte).unwrap_or(texte),
        "spectacle" => ctx.spectacle(&texte).ok().and_then(|s| s.get("titre").map(en_texte)).unwrap_or(texte),
        "emploi" => format!("« {} »", texte),
        _ => texte,
    }
}

/// CE QUE DIT L'AVENANT D'UN CONTRAT SIGNE HORS DU LOGICIEL : il ne connait pas le texte du contrat, il n'en cite aucun
/// article ; il dit en clair, pour chaque terme qui change (`engagement::versions::termes_changes`), a partir de quand,
/// l'ancienne valeur et la nouvelle (code du travail R. 1221-40 : « un document indiquant ces modifications »,
/// `atelier/reference/sources/code-du-travail/xml/R1221-40.xml`). Chaque modification : `article` (son titre), `avant` et
/// `apres` (les valeurs), `phrase` (ce que l'avenant ecrit), `jour` ; `numero` nul (aucun article cite).
pub fn changements_en_clair(ctx: &Contexte, nat: &droit::natures::Nature, changes: &[engagement::versions::TermeChange]) -> Vec<Value> {
    changes
        .iter()
        .map(|c| {
            let (titre, nom) = terme_nomme(&c.terme);
            let dit = |v: &Option<Value>| v.as_ref().map(|x| valeur_en_clair(ctx, nat, &c.terme, x));
            let (avant, apres) = (dit(&c.avant), dit(&c.apres));
            let quand = format!("À compter du {}", c.jour.jj_mm_aaaa());
            let phrase = match (&avant, &apres) {
                (Some(a), Some(b)) => format!("{}, {} passe de {} à {}.", quand, nom, a, b),
                (None, Some(b)) => format!("{}, {} est : {}.", quand, nom, b),
                (Some(a), None) => format!("{}, {} n'est plus {}.", quand, nom, a),
                (None, None) => format!("{}, {} change.", quand, nom),
            };
            serde_json::json!({
                "numero": Value::Null, "article": titre, "phrase": phrase, "jour": c.jour.iso(),
                "avant": [avant.unwrap_or_default()], "apres": [apres.unwrap_or_default()],
            })
        })
        .collect()
}
