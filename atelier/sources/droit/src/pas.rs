//! Le prelevement a la source : le taux et son identifiant, tels que la DGFiP les donne (les taux recus, dans la fiche du
//! salarie, `CHAMP`), et la grille de l'article 204 H du CGI quand aucun ne vaut. Ce que le calcul d'un engagement et la
//! DSN lisent.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::contexte::Contexte;
use socle::date::{en_texte, jours_du_mois, Jour};
use socle::num::nombre;
use socle::fichiers::{repr, vrai};
use socle::num::{dec_avec_echelle, eur, pct, sub, Decimal, Resultat};

/// LE TAUX RECU APPARTIENT AU SALARIE : chaque taux que la DGFiP transmet (un compte rendu de nature 94) s'ecrit dans
/// sa fiche, valeur datee du jour ou il commence de valoir (`date_d_effet`) : `{"taux", "type", "identifiant",
/// "mis_a_disposition", "en_retour_de"?}`, `mis_a_disposition` en AAAAMMJJHHMMSS, l'instant du compte rendu, et
/// `en_retour_de` (`EN_RETOUR_DE`) le mois de la DSN a laquelle il repond.
pub const CHAMP: &str = "taux_pas";

/// Un refus du module. `SansGrille` est `BaremeSansGrille` (un bareme sans la grille de l'article 204 H : le prelevement
/// ne se devine pas, le calcul refuse) ; `Inconnu` est `PasInconnu`.
#[derive(Debug, Clone, PartialEq)]
pub enum ErreurPas {
    SansGrille(String),
    Inconnu(String),
    Autre(String),
}

impl ErreurPas {
    pub fn message(&self) -> &str {
        match self {
            ErreurPas::SansGrille(m) | ErreurPas::Inconnu(m) | ErreurPas::Autre(m) => m,
        }
    }
}

impl From<String> for ErreurPas {
    fn from(m: String) -> ErreurPas {
        ErreurPas::Autre(m)
    }
}

type R<T> = Result<T, ErreurPas>;

/// Un taux qui vaut : (taux, type, identifiant), tels que le compte rendu les donne.
pub type Triplet = (Value, Value, Value);

// ------------------------------------------------------------- peremption

/// Un taux transmis par la DGFiP vaut jusqu'a la fin du deuxieme mois suivant
/// celui de sa mise a disposition : le debiteur « applique le taux calculé par
/// l'administration fiscale, au plus tard le deuxième mois suivant sa
/// transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) ; « le taux de prélèvement est
/// applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition » :
/// mis a disposition le 10 fevrier, il est « valide jusqu'au 30 avril »
/// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/
/// BOI-IR-PAS-30-10-20-20181203.html`).
pub const VALIDITE_MOIS: u32 = 2;

/// La valeur de `cle` en texte, sans blancs autour, vide si elle manque ou est vide.
fn texte_ou_vide(e: &Value, cle: &str) -> String {
    match e.get(cle) {
        Some(v) if vrai(v) => en_texte(v).trim().to_string(),
        _ => String::new(),
    }
}

/// Les `n` premiers caracteres, s'ils sont tous des chiffres.
fn chiffres(s: &str, n: usize) -> Option<Vec<u32>> {
    let c: Vec<char> = s.chars().take(n).collect();
    if c.len() == n && c.iter().all(|x| x.is_ascii_digit()) {
        Some(c.iter().map(|x| x.to_digit(10).unwrap()).collect())
    } else {
        None
    }
}

fn nombre_de(c: &[u32]) -> u32 {
    c.iter().fold(0, |a, x| a * 10 + x)
}

/// Le jour de la mise a disposition d'un taux recu (`mis_a_disposition`, AAAAMMJJHHMMSS) : d'ou court sa validite.
pub fn mise_a_disposition(e: &Value) -> Option<Jour> {
    let c = chiffres(&texte_ou_vide(e, "mis_a_disposition"), 8)?;
    Jour::nouveau(nombre_de(&c[..4]) as i32, nombre_de(&c[4..6]), nombre_de(&c[6..8]))
}

/// Une date `AAAA-MM-JJ` stricte.
fn depuis_iso(s: &str) -> Resultat<Jour> {
    let faux = || format!("date illisible : {}", repr(s));
    let c: Vec<char> = s.chars().collect();
    if c.len() != 10 || c[4] != '-' || c[7] != '-' {
        return Err(faux());
    }
    let tranche = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    match (chiffres(&tranche(0, 4), 4), chiffres(&tranche(5, 7), 2), chiffres(&tranche(8, 10), 2)) {
        (Some(a), Some(m), Some(j)) => Jour::nouveau(nombre_de(&a) as i32, nombre_de(&m), nombre_de(&j)).ok_or_else(faux),
        _ => Err(faux()),
    }
}

/// Dernier jour ou ce taux est celui du salarie ; `None` si on ne sait pas.
pub fn fin_de_validite(e: &Value) -> Option<Jour> {
    let debut = mise_a_disposition(e)?;
    let mois = debut.mois + VALIDITE_MOIS;
    let (an, m) = (debut.an + ((mois - 1) / 12) as i32, (mois - 1) % 12 + 1);
    Jour::nouveau(an, m, jours_du_mois(an, m))
}

/// Le type, en texte.
fn type_texte(e: &Value) -> String {
    en_texte(e.get("type").unwrap_or(&Value::Null))
}

/// Un taux qu'on ne sait pas dater est traite comme perime ; le bareme (tout
/// type autre que 01) ne se perime pas.
pub fn est_perime(e: &Value, le: Jour) -> bool {
    if type_texte(e) != "01" {
        return false;
    }
    match fin_de_validite(e) {
        None => true,
        Some(fin) => le > fin,
    }
}

// ------------------------------------------------------------------ lecture

/// Vrai seulement si la fiche est la ET qu'elle n'a pas de NIR.
pub fn sans_nir(ctx: &Contexte, cle: &str) -> bool {
    match ctx.salaries.get(cle) {
        Some(f) => !f.get("nir").filter(|n| vrai(n)).map_or(false, |n| !en_texte(n).trim().is_empty()),
        None => false,
    }
}

/// Le jour auquel on demande si un taux vaut encore : celui du contexte.
pub fn le_jour(ctx: &Contexte, le: Option<Jour>) -> Jour {
    le.unwrap_or_else(|| {
        socle::fichiers::noter_valeur("date", ctx.date.iso());
        ctx.date
    })
}

/// LES TAUX RECUS POUR LE SALARIE `cle`, du premier mis a disposition au dernier (a instant egal, dans l'ordre ou ils
/// se sont ecrits). Une valeur de la fiche qui n'est pas un taux (la case vide d'avant le premier) n'en est pas un.
pub fn recus(ctx: &Contexte, cle: &str) -> Resultat<Vec<Value>> {
    let Some(fiche) = ctx.salarie_brut(cle) else { return Ok(Vec::new()) };
    let mut r: Vec<Value> = crate::contexte::valeurs_datees(fiche, CHAMP)?.into_iter().map(|(_, v)| v).filter(|v| v.is_object()).cloned().collect();
    r.sort_by_key(|v| texte_ou_vide(v, "mis_a_disposition"));
    Ok(r)
}

/// Dans un taux recu : le mois (AAAA-MM) de la DSN mensuelle a laquelle repond le compte rendu qui l'a transmis ; absent
/// pour la reponse a une demande de taux (signalement d'amorcage) et pour un taux repris d'une DSN deposee.
pub const EN_RETOUR_DE: &str = "en_retour_de";

/// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT, JAMAIS POUR CELUI QU'ELLE DECLARE. Le compte
/// rendu est « établi par l'administration fiscale en retour de chaque déclaration sociale nominative » (BOI-IR-PAS-30-10-20,
/// § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`) ; le taux de celui qui repond a la DSN de
/// janvier sert a « précompter la retenue à la source sur les revenus versés au titre des mois de février, mars et avril »
/// (§ 70, exemple 1). La DSN d'un mois a deja declare le taux « précompté appliqué » a chacun de ses versements
/// (S21.G00.50.006, `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`) ; le compte rendu qui lui repond porte « un
/// identifiant propre » (CSS, art. R. 133-13, V, 1°, `atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261030.md`) : s'il valait pour les versements qu'elle declare, chaque DSN deposee avant eux serait a
/// refaire par sa propre reponse, et son annule-et-remplace par la sienne. INFERENCE (les textes supposent la DSN deposee
/// apres le versement) : un taux en retour de la DSN du mois `M` ne vaut pas pour la paie de `M`, ni d'un mois d'avant.
fn en_retour_d_un_mois_qui_n_est_pas_avant(e: &Value, mois: Option<&str>) -> bool {
    let retour = texte_ou_vide(e, EN_RETOUR_DE);
    mois.is_some_and(|m| !retour.is_empty() && retour.as_str() >= m)
}

/// Le jour ou un taux recu commence de valoir dans la fiche (`depuis` de sa valeur datee) : le jour de sa mise a
/// disposition `mad`, et, en retour de la DSN du mois `en_retour_de`, pas avant le premier jour du mois qui suit
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Deux comptes rendus du meme jour, l'un pour le mois, l'autre en retour
/// de sa DSN, ne s'ecrasent donc pas.
pub fn date_d_effet(mad: Jour, en_retour_de: Option<&str>) -> Jour {
    let premier_d_apres = en_retour_de.and_then(|m| socle::mois::Mois::lire(m).ok()).map(|m| m.suivant().premier_jour());
    premier_d_apres.map_or(mad, |p| p.max(mad))
}

/// LE TAUX DU SALARIE AU JOUR `le`, pour la paie du mois `mois` : de ceux qu'il a recus, le dernier mis a disposition au
/// plus tard ce jour-la, hors ceux qui repondent a la DSN de ce mois ou d'un mois d'apres
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Le debiteur « applique le taux calculé par l'administration fiscale, au
/// plus tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) : un taux vaut des sa mise a disposition (BOI-IR-PAS-30-10-20, § 70 et
/// 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`), pour toute retenue faite a partir de ce
/// jour ; la retenue est « effectuée par le débiteur lors du paiement » (CGI, art. 1671, 1, meme fichier) : un taux
/// transmis apres le paiement d'un salaire ne vaut pas pour lui. Aucun taux recu avant ce jour : aucun (la grille de
/// l'article 204 H s'applique).
fn en_vigueur<'a>(recus: &'a [Value], le: Jour, mois: Option<&str>) -> Option<&'a Value> {
    recus.iter().filter(|e| mise_a_disposition(e).is_some_and(|d| d <= le) && !en_retour_d_un_mois_qui_n_est_pas_avant(e, mois)).last()
}

// ------------------------------------------ la grille de l'article 204 H du CGI

pub const ZONE_DEFAUT: &str = "metropole";

fn section(bareme: &Value) -> R<&Value> {
    match bareme.get("pas_taux_par_defaut") {
        Some(s) if vrai(s) => Ok(s),
        _ => Err(ErreurPas::SansGrille(
            "Le bareme ne porte pas de section `pas_taux_par_defaut` : sans \
             elle, le moteur ne sait pas quel taux appliquer a qui n'a pas de \
             taux DGFiP.\n\
             Elle se releve sur BOI-BAREME-000037 (grilles des taux par \
             defaut et abattement pour les contrats courts), et le socle \
             livre de l'annee en cours la porte."
                .to_string(),
        )),
    }
}

fn str_iso(v: &Value) -> Resultat<Jour> {
    depuis_iso(&en_texte(v))
}

/// Une date en AAAA-MM-JJ, `None` s'il n'y en a pas.
fn le_texte(le: Option<Jour>) -> String {
    le.map_or("None".to_string(), |j| j.iso())
}

/// Une valeur du bareme, deja resolue ou encore datee (`[{depuis, valeur}]`).
pub fn valeur_datee(v: &Value, le: Option<Jour>) -> R<Value> {
    let Value::Array(entrees) = v else { return Ok(v.clone()) };
    let cle = |e: &Value| -> Resultat<String> { Ok(en_texte(e.get("depuis").ok_or_else(|| socle::fichiers::absent("depuis"))?)) };
    let mut tries: Vec<(String, &Value)> = Vec::new();
    for e in entrees {
        tries.push((cle(e)?, e));
    }
    tries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut retenu = None;
    for (_, e) in tries {
        let dans = match le {
            None => true,
            Some(l) => str_iso(&e["depuis"])? <= l,
        };
        if dans {
            retenu = Some(e.get("valeur").cloned().ok_or_else(|| socle::fichiers::absent("valeur"))?);
        }
    }
    retenu.ok_or_else(|| {
        ErreurPas::SansGrille(format!(
            "Aucun abattement contrats courts applicable au {} : la premiere \
             valeur connue est posterieure.",
            le_texte(le)
        ))
    })
}

/// L'abattement contrats courts en vigueur : la moitie du SMIC net imposable.
pub fn abattement(bareme: &Value, le: Option<Jour>) -> R<Decimal> {
    let v = section(bareme)?.get("abattement_contrats_courts").ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent("abattement_contrats_courts")))?;
    Ok(dec_avec_echelle(&valeur_datee(v, le)?)?)
}

/// La grille en vigueur a cette date pour cette zone (elles changent au 1er mai).
pub fn grille<'a>(bareme: &'a Value, le: Option<Jour>, zone: &str) -> R<&'a Value> {
    let grilles = section(bareme)?.get("grilles").and_then(Value::as_array).ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent("grilles")))?;
    let mut meilleure: Option<(&Value, String)> = None;
    for g in grilles {
        if en_texte(g.get("zone").unwrap_or(&Value::Null)) != zone {
            continue;
        }
        let depuis = en_texte(g.get("depuis").ok_or_else(|| socle::fichiers::absent("depuis"))?);
        if let Some(l) = le {
            if depuis_iso(&depuis)? > l {
                continue;
            }
        }
        // `max` garde la premiere des egales.
        if meilleure.as_ref().map_or(true, |(_, m)| depuis > *m) {
            meilleure = Some((g, depuis));
        }
    }
    match meilleure {
        Some((g, _)) => Ok(g),
        None => {
            let connues: BTreeSet<String> = grilles.iter().map(|g| en_texte(&g["zone"])).collect();
            Err(ErreurPas::SansGrille(format!(
                "Aucune grille de taux par defaut pour la zone {} au {}. Zones \
                 connues du bareme : {}.",
                repr(zone),
                le.map_or("?".to_string(), |j| j.iso()),
                connues.into_iter().collect::<Vec<_>>().join(", ")
            )))
        }
    }
}

/// `"%.2f" % taux`.
fn deux_decimales(v: &Value) -> Resultat<String> {
    match v {
        Value::Number(n) => Ok(format!("{:.2}", n.as_f64().unwrap_or(0.0))),
        Value::Bool(b) => Ok(format!("{:.2}", f64::from(u8::from(*b)))),
        autre => Err(socle::fichiers::attendu("un nombre", socle::valeur::nom_type(autre))),
    }
}

/// `(taux, type de taux)` pour un montant DEJA abattu.
pub fn taux_par_defaut(bareme: &Value, montant: Decimal, le: Option<Jour>, zone: &str) -> R<(String, String)> {
    let g = grille(bareme, le, zone)?;
    let type_taux = en_texte(g.get("type").ok_or_else(|| socle::fichiers::absent("type"))?);
    let tranches = g.get("tranches").and_then(Value::as_array).ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent("tranches")))?;
    for t in tranches {
        let (bas, haut, taux) = match t.as_array().map(Vec::as_slice) {
            Some([b, h, x]) => (b, h, x),
            _ => return Err(ErreurPas::Autre(format!("tranche de la grille du PAS illisible (bas, haut, taux attendus) : {}", t))),
        };
        if montant >= dec_avec_echelle(bas)? && (haut.is_null() || montant < dec_avec_echelle(haut)?) {
            return Ok((deux_decimales(taux)?, type_taux));
        }
    }
    // Seul un montant negatif tombe ici : taux zero.
    Ok(("0.00".to_string(), type_taux))
}

/// Le prelevement d'un versement, par l'un des deux chemins.
#[derive(Debug, Clone, PartialEq)]
pub struct Versement {
    pub taux: Value,
    pub type_taux: Value,
    pub identifiant: Value,
    pub abattement: Decimal,
    pub montant_soumis: Decimal,
    pub prelevement: Decimal,
}

impl Versement {
    /// Le versement tel que `canon` l'ecrit.
    pub fn lire(v: &Value) -> Resultat<Versement> {
        let n = |k: &str| v.get(k).ok_or_else(|| socle::fichiers::absent(k)).and_then(dec_avec_echelle);
        Ok(Versement {
            taux: v.get("taux").cloned().unwrap_or(Value::Null),
            type_taux: v.get("type").cloned().unwrap_or(Value::Null),
            identifiant: v.get("identifiant").cloned().unwrap_or(Value::Null),
            abattement: n("abattement")?,
            montant_soumis: n("montant_soumis")?,
            prelevement: n("prelevement")?,
        })
    }

    pub fn canon(&self) -> Value {
        let mut m = Map::new();
        m.insert("taux".into(), self.taux.clone());
        m.insert("type".into(), self.type_taux.clone());
        m.insert("identifiant".into(), self.identifiant.clone());
        m.insert("abattement".into(), Value::String(nombre(self.abattement)));
        m.insert("montant_soumis".into(), Value::String(nombre(self.montant_soumis)));
        m.insert("prelevement".into(), Value::String(nombre(self.prelevement)));
        Value::Object(m)
    }
}

pub fn plancher(x: Decimal, zero: Decimal) -> Decimal {
    if zero > x {
        zero
    } else {
        x
    }
}

/// Tout le bloc 50 cote PAS quand aucun taux DGFiP ne s'applique ; pas
/// d'identifiant (S21.G00.50.008 est interdite hors taux DGFiP).
pub fn rubriques_par_defaut(
    bareme: &Value,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> R<Versement> {
    let rnf = eur(remuneration_nette_fiscale);
    let zero = Decimal::new(0, 2);
    let abat = if contrat_court { abattement(bareme, le)? } else { zero };
    // L'abattement ne creuse pas : il se limite a ce qu'il y a a abattre.
    let abat = eur(abat.min(plancher(rnf, zero)));
    let soumis = eur(plancher(sub(rnf, abat), zero));
    let (taux, type_taux) = taux_par_defaut(bareme, soumis, le, zone)?;
    let prelevement = pct(soumis, dec_avec_echelle(&Value::String(taux.clone()))?);
    Ok(Versement {
        taux: Value::String(taux),
        type_taux: Value::String(type_taux),
        identifiant: Value::Null,
        abattement: abat,
        montant_soumis: soumis,
        prelevement,
    })
}

// ------------------------------------------- lequel des deux chemins, et pour
//                                              quel versement

/// Le triplet a declarer pour un versement du jour `le` de la paie du mois `mois`, ou `None` : pas de taux DGFiP
/// applicable, c'est la grille. Un taux perime compte pour rien.
/// UN CALCUL DEPEND DU TAUX DE SON SALARIE A SON JOUR : il le note (`pas <cle> <jour> <mois>`, `empreinte`), et un taux
/// recu qui le change le fait recalculer.
pub fn taux_dgfip(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Option<Triplet>> {
    let t = triplet(ctx, cle, le, mois)?;
    if let Some(j) = le {
        socle::fichiers::noter_valeur(&cle_d_empreinte(cle, j, mois), empreinte_du_triplet(&t));
    }
    Ok(t)
}

/// La cle sous laquelle `taux_dgfip` note le taux du salarie `cle` au jour `le` pour la paie de `mois`.
fn cle_d_empreinte(cle: &str, le: Jour, mois: Option<&str>) -> String {
    match mois {
        Some(m) => format!("pas {} {} {}", cle, le.iso(), m),
        None => format!("pas {} {}", cle, le.iso()),
    }
}

fn triplet(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Option<Triplet>> {
    let s = situation(ctx, cle, le, mois)?;
    if s["situation"] != Value::String(VALIDE.into()) {
        return Ok(None);
    }
    Ok(Some((s["taux"].clone(), s["type"].clone(), s["identifiant"].clone())))
}

fn empreinte_du_triplet(t: &Option<Triplet>) -> String {
    match t {
        Some((a, b, c)) => Value::Array(vec![a.clone(), b.clone(), c.clone()]).to_string(),
        None => "aucun".into(),
    }
}

/// Ce que `taux_dgfip` a note sous `cle_notee` (`pas <cle> <jour> [<mois>]`), relu maintenant : l'empreinte tient tant
/// qu'il est le meme. `None` : la cle ne se lit pas.
pub fn empreinte(ctx: &Contexte, cle_notee: &str) -> Option<String> {
    let mut parts = cle_notee.strip_prefix("pas ")?.split(' ');
    let (cle, jour, mois) = (parts.next()?, Jour::lire(parts.next()?).ok()?, parts.next());
    triplet(ctx, cle, Some(jour), mois).ok().map(|t| empreinte_du_triplet(&t))
}

/// La fiche d'un salarie sans ses taux recus : ce qu'une paie lit de la fiche hors du taux, qu'elle lit au jour de son
/// versement (`taux_dgfip`). Un taux recu ne fait recalculer que la paie dont il change le taux.
pub fn sans_taux_recus(fiche: &Value) -> Value {
    let mut f = fiche.clone();
    if let Some(o) = f.as_object_mut() {
        o.remove(CHAMP);
    }
    f
}

/// Le prelevement d'UN versement. Un triplet s'applique tel quel, sur la
/// remuneration nette fiscale entiere, sans abattement.
pub fn du_versement(
    taux: Option<&Triplet>,
    bareme: &Value,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> R<Versement> {
    let Some((t, type_taux, identifiant)) = taux else {
        return rubriques_par_defaut(bareme, remuneration_nette_fiscale, le, contrat_court, zone);
    };
    // UNE REMUNERATION NETTE FISCALE NEGATIVE N'EST PAS UNE ASSIETTE NEGATIVE. Fiche DSN 1810 « Remuneration nette fiscale
    // negative » (`atelier/reference/sources/suspension-non-remuneree/net-entreprises-fiche1810-rnf-negative-pas-20250322.txt`) :
    // « La rubrique « Montant soumis au PAS - S21.G00.50.013 » ne peut pas etre renseignee avec une valeur negative. Il
    // convient alors de renseigner [...] a 0.00 » (l'exemple : 50.002 = -180.00, 50.009 = 0.00, 50.013 = 0.00) ; le cahier
    // technique refuse le signe moins en 50.009 et 50.013 et l'accepte en 50.002 (`norme-dsn/texte/bloc-S21.G00.50.txt`).
    // La grille (`rubriques_par_defaut`) plancher deja a zero ; le taux DGFiP aussi. Le cas : un mois sans salaire ou la
    // part patronale de sante reste due et se reintegre (CGI 83, 1° quater, meme dossier, `legi-extrait-code-general-des-
    // impots-20261015.md`).
    let soumis = plancher(eur(remuneration_nette_fiscale), Decimal::new(0, 2));
    Ok(Versement {
        taux: t.clone(),
        type_taux: type_taux.clone(),
        identifiant: identifiant.clone(),
        abattement: Decimal::new(0, 2),
        montant_soumis: soumis,
        prelevement: pct(soumis, dec_avec_echelle(t)?),
    })
}

// ------------------------------------------------------------- diagnostic

pub const VALIDE: &str = "valide";
pub const PERIME: &str = "perime";
pub const BAREME: &str = "bareme";
pub const INCONNU: &str = "inconnu";
pub const SANS_NIR: &str = "sans_nir";

fn libelle(situation: &str) -> &'static str {
    match situation {
        VALIDE => "taux DGFiP en cours de validite",
        PERIME => "taux DGFiP perime -- ce n'est plus le taux du salarie",
        BAREME => "aucun taux personnalise : c'est le bareme qui s'applique",
        INCONNU => "aucun taux connu",
        _ => "pas de NIR : la DGFiP n'a personne a qui repondre",
    }
}

/// Ou en est le taux d'un salarie au jour `le`, pour la paie du mois `mois` (`en_vigueur`). Ne refuse que si sa fiche est
/// illisible.
pub fn situation(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Value> {
    let le = le_jour(ctx, le);
    let recus = recus(ctx, cle)?;
    let e = en_vigueur(&recus, le, mois);
    let fin = e.and_then(fin_de_validite);
    let quoi = match e {
        None if sans_nir(ctx, cle) => SANS_NIR,
        None => INCONNU,
        Some(e) if type_texte(e) != "01" => {
            if sans_nir(ctx, cle) {
                SANS_NIR
            } else {
                BAREME
            }
        }
        Some(e) => {
            if est_perime(e, le) {
                PERIME
            } else {
                VALIDE
            }
        }
    };
    let champ = |c: &str| e.and_then(|e| e.get(c)).cloned().unwrap_or(Value::Null);
    let mut m = Map::new();
    m.insert("salarie".into(), Value::String(cle.to_string()));
    m.insert("situation".into(), Value::String(quoi.to_string()));
    m.insert("libelle".into(), Value::String(libelle(quoi).to_string()));
    m.insert("taux".into(), champ("taux"));
    m.insert("type".into(), champ("type"));
    m.insert("identifiant".into(), champ("identifiant"));
    m.insert("valable_jusqu_a".into(), fin.map_or(Value::Null, |f| Value::String(f.iso())));
    m.insert("a_demander".into(), Value::Bool(matches!(quoi, PERIME | BAREME | INCONNU)));
    m.insert("dernier_recu".into(), recus.last().map_or(Value::Null, |r| r["mis_a_disposition"].clone()));
    Ok(Value::Object(m))
}

fn archive(ctx: &Contexte, cle: &str) -> bool {
    ctx.salaries.get(cle).map_or(false, |f| vrai(f) && f.get("actif") == Some(&Value::Bool(false)))
}

/// Le meme tableau pour plusieurs salaries, dans l'ordre alphabetique. Sans liste : ceux des fiches, moins les archives.
pub fn diagnostic(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>, mois: Option<&str>) -> Resultat<Vec<Value>> {
    let cles: BTreeSet<String> = match cles {
        Some(c) => c.iter().cloned().collect(),
        None => ctx.salaries.keys().filter(|c| !archive(ctx, c)).cloned().collect(),
    };
    cles.iter().map(|c| situation(ctx, c, le, mois)).collect()
}

/// `jj/mm/aaaa` d'une date ISO, `?` si elle manque.
fn fr(iso: &Value) -> String {
    match iso {
        Value::String(s) if !s.is_empty() => {
            let c: Vec<char> = s.chars().collect();
            let t = |a: usize, b: usize| c.iter().skip(a).take(b - a).collect::<String>();
            format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
        }
        _ => "?".to_string(),
    }
}

/// Les phrases a dire au depot de la DSN et a la production des bulletins (`depart::sans_taux_recu`), une par salarie
/// dont la paie prendrait la grille de l'article 204 H, III (CGI, art. 1671, 2 : « A défaut de taux transmis par
/// l'administration ») : ce que sa fiche sait des taux recus. Vide si tout va bien.
pub fn avertissements(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>, mois: Option<&str>) -> Resultat<Vec<String>> {
    let mut lignes = Vec::new();
    for s in diagnostic(ctx, cles, le, mois)? {
        let quoi = s["situation"].as_str().unwrap_or_default();
        let mut ligne = format!("{} : ", ctx.nomme(&en_texte(&s["salarie"])));
        ligne += &match quoi {
            VALIDE => continue,
            PERIME => format!(
                "le dernier taux transmis par la DGFiP ({} %) valait jusqu'au {}",
                en_texte(&s["taux"]).replace('.', ","),
                fr(&s["valable_jusqu_a"])
            ),
            SANS_NIR => "sans NIR dans sa fiche, la DGFiP ne peut transmettre aucun taux".into(),
            _ => "aucun taux transmis par la DGFiP".into(),
        };
        lignes.push(ligne);
    }
    Ok(lignes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// LA DATE D'EFFET D'UN TAUX TRANSMIS. Le debiteur « applique le taux calculé par l'administration fiscale, au plus
    /// tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/code-general-des-
    /// impots/articles-20261021.md`, l. 135) ; la retenue « est effectuée par le débiteur lors du paiement » (meme
    /// article, 1, l. 132) ; le taux est « applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition »
    /// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`, l. 23-24). Un
    /// taux vaut donc pour tout paiement fait a partir du jour de sa mise a disposition, et pour aucun avant : mis a
    /// disposition le 13/03, il vaut pour le salaire paye le 14/03 ; celui du 15/04 ne vaut pas pour lui, il vaut pour
    /// celui paye le 15/04. Sans taux recu avant le paiement : aucun, la grille s'applique.
    #[test]
    fn un_taux_vaut_pour_tout_paiement_a_partir_de_sa_mise_a_disposition() {
        let taux = |id: &str, le: &str| json!({"taux": "9.40", "type": "01", "identifiant": id, "mis_a_disposition": le});
        let recus = vec![taux("480747943", "20260313070000"), taux("985249073", "20260415100000")];
        let id = |le: &str| en_vigueur(&recus, Jour::lire(le).unwrap(), None).map(|e| e["identifiant"].clone());
        assert_eq!(id("2026-03-12"), None);
        assert_eq!(id("2026-03-13"), Some(json!("480747943")));
        assert_eq!(id("2026-03-14"), Some(json!("480747943")));
        assert_eq!(id("2026-04-14"), Some(json!("480747943")));
        assert_eq!(id("2026-04-15"), Some(json!("985249073")));
        assert_eq!(id("2024-12-31"), None);
    }

    /// BOI-IR-PAS-30-10-20, § 70, exemple 1 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) :
    /// un taux mis a disposition le 10 fevrier « est valide jusqu'au 30 avril ».
    #[test]
    fn un_taux_vaut_jusqu_a_la_fin_du_deuxieme_mois_qui_suit_sa_mise_a_disposition() {
        let e = json!({"taux": "5.00", "type": "01", "identifiant": "1", "mis_a_disposition": "20260210120000"});
        assert_eq!(fin_de_validite(&e), Jour::nouveau(2026, 4, 30));
        assert!(!est_perime(&e, Jour::nouveau(2026, 4, 30).unwrap()));
        assert!(est_perime(&e, Jour::nouveau(2026, 5, 1).unwrap()));
    }

    /// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT (BOI-IR-PAS-30-10-20, § 20 et § 70, exemple 1 :
    /// le compte rendu de la DSN de janvier sert aux revenus de « février, mars et avril », `atelier/reference/sources/
    /// bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`). La DSN de decembre, deposee le 21/12 pour un versement du 31/12 au
    /// taux du compte rendu du 05/10 (valide jusqu'au 31/12) : son compte rendu du 21/12 ne vaut pas pour la paie de
    /// decembre, il vaut pour celle de janvier, et s'ecrit dans la fiche a partir du 01/01. La reponse a une demande de
    /// taux (sans DSN mensuelle) vaut des sa mise a disposition.
    #[test]
    fn le_taux_en_retour_de_la_dsn_d_un_mois_vaut_pour_les_mois_qui_suivent() {
        let taux = |id: &str, le: &str, retour: Option<&str>| {
            let mut v = json!({"taux": "2.30", "type": "01", "identifiant": id, "mis_a_disposition": le});
            if let Some(m) = retour {
                v[EN_RETOUR_DE] = json!(m);
            }
            v
        };
        let recus = vec![taux("157352475", "20261005194311", Some("2026-09")), taux("713288315", "20261221184328", Some("2026-12"))];
        let id = |le: &str, mois: &str| en_vigueur(&recus, Jour::lire(le).unwrap(), Some(mois)).map(|e| e["identifiant"].clone());
        assert_eq!(id("2026-12-31", "2026-12"), Some(json!("157352475")));
        assert_eq!(id("2027-01-05", "2026-12"), Some(json!("157352475")));
        assert_eq!(id("2027-01-31", "2027-01"), Some(json!("713288315")));
        let demande = vec![taux("480747943", "20261221184328", None)];
        assert!(en_vigueur(&demande, Jour::lire("2026-12-31").unwrap(), Some("2026-12")).is_some());
        let mad = Jour::lire("2026-12-21").unwrap();
        assert_eq!(date_d_effet(mad, Some("2026-12")), Jour::lire("2027-01-01").unwrap());
        assert_eq!(date_d_effet(mad, Some("2026-11")), mad);
        assert_eq!(date_d_effet(mad, None), mad);
    }
}
