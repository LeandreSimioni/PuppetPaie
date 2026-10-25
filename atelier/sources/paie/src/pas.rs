//! Prelevement a la source : le taux et son identifiant, tels que la DGFiP
//! les donne, et la grille de l'article 204 H du CGI quand elle n'en donne
//! pas (`pas.py`). Ce que le calcul d'un bulletin et la DSN lisent ; ce qui
//! ecrit le magasin (`ecrire`, `enregistrer`, `amorcer`) et l'import de
//! l'historique ne sont pas portes.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::calcul::Paie;
use droit::contexte::Contexte;
use droit::natures::contrat_court;
use socle::date::{en_texte, jours_du_mois, Jour};
use crate::engagement::nombre;
use socle::fichiers::{lire_json, repr, vrai};
use socle::num::{dec_py, eur, pct, sub, Decimal, Resultat};

const FICHIER: &str = "pas.json";

/// Un refus du module. `SansGrille` est `BaremeSansGrille`, que `des_paies`
/// sait taire ; `Inconnu` est `PasInconnu`.
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

/// Un taux qui vaut : (taux, type, identifiant), tels que le magasin les porte.
pub type Triplet = (Value, Value, Value);

// ------------------------------------------------------------- peremption

/// Un taux transmis par la DGFiP vaut jusqu'a la fin du deuxieme mois suivant
/// celui de sa mise a disposition.
const VALIDITE_MOIS: u32 = 2;

/// `str(e.get(cle) or "").strip()`.
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

/// D'ou court la validite, de la source la plus sure a la moins sure :
/// `mis_a_disposition` (AAAAMMJJHHMMSS), puis `periode` (AAAAMM ou AAAA-MM),
/// puis `releve_le`.
pub fn mise_a_disposition(e: &Value) -> Option<Jour> {
    let v = texte_ou_vide(e, "mis_a_disposition");
    if let Some(c) = chiffres(&v, 8) {
        if let Some(j) = Jour::nouveau(nombre_de(&c[..4]) as i32, nombre_de(&c[4..6]), nombre_de(&c[6..8])) {
            return Some(j);
        }
    }
    let p = texte_ou_vide(e, "periode").replace('-', "");
    if let Some(c) = chiffres(&p, 6) {
        if let Some(j) = Jour::nouveau(nombre_de(&c[..4]) as i32, nombre_de(&c[4..6]), 1) {
            return Some(j);
        }
    }
    let r = texte_ou_vide(e, "releve_le");
    if r.chars().count() >= 10 {
        let t: String = r.chars().take(10).collect();
        if let Ok(j) = depuis_iso(&t) {
            return Some(j);
        }
    }
    None
}

/// `date.fromisoformat` sur `AAAA-MM-JJ`.
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

/// `str(e.get("type"))`.
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

/// `e` avec `valable_jusqu_a` et `perime`.
pub fn avec_validite(e: &Value, le: Jour) -> Value {
    let mut m = e.as_object().cloned().unwrap_or_default();
    m.insert("valable_jusqu_a".into(), fin_de_validite(e).map_or(Value::Null, |f| Value::String(f.iso())));
    m.insert("perime".into(), Value::Bool(est_perime(e, le)));
    Value::Object(m)
}

// ------------------------------------------------------------------ lecture

/// `pas.charger` : le magasin (`config/pas.json`), ou un magasin vide.
pub fn charger(ctx: &Contexte) -> Resultat<Value> {
    let p = ctx.racine.join("config").join(FICHIER);
    if !socle::fichiers::existe(&p) {
        let mut m = Map::new();
        m.insert("salaries".into(), Value::Object(Map::new()));
        m.insert("journal".into(), Value::Array(Vec::new()));
        m.insert("demandes".into(), Value::Object(Map::new()));
        return Ok(Value::Object(m));
    }
    let mut d = lire_json(&p)?;
    if let Some(o) = d.as_object_mut() {
        o.entry("salaries").or_insert_with(|| Value::Object(Map::new()));
        o.entry("journal").or_insert_with(|| Value::Array(Vec::new()));
        o.entry("demandes").or_insert_with(|| Value::Object(Map::new()));
    }
    Ok(d)
}

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

fn entree<'a>(magasin: &'a Value, cle: &str) -> Option<&'a Value> {
    magasin.get("salaries").and_then(|s| s.get(cle))
}

/// Une entree vide vaut une entree absente (`if not e`).
fn presente(e: Option<&Value>) -> Option<&Value> {
    e.filter(|v| vrai(v))
}

/// Taux de PAS courant d'un salarie, avec sa fin de validite. Refuse
/// (`PasInconnu`) plutot que de renvoyer une valeur par defaut. Un taux
/// perime est rendu, marque `perime`.
pub fn etat(ctx: &Contexte, cle: &str, le: Option<Jour>) -> R<Value> {
    let le = le_jour(ctx, le);
    let magasin = charger(ctx)?;
    let Some(e) = presente(entree(&magasin, cle)) else {
        if sans_nir(ctx, cle) {
            return Err(ErreurPas::Inconnu(format!(
                "{} n'a pas de NIR : la DGFiP ne peut pas lui transmettre de \
                 taux de prelevement a la source, et aucun compte-rendu \
                 metier n'en portera.\n\
                 C'est la grille de l'article 204 H du code general des \
                 impots -- le taux par defaut, type 13 -- qui s'applique dans \
                 ce cas, et la DSN la calcule (`pas.rubriques_par_defaut`). \
                 Il n'y a donc rien a attendre ici, et rien a demander : le \
                 signalement d'amorcage exige un NIR.\n\
                 Des que la caisse notifie le NIR, le saisir dans la fiche : \
                 le CRM suivra, et le taux du salarie remplacera la grille.",
                cle
            )));
        }
        return Err(ErreurPas::Inconnu(format!(
            "Aucun taux de prelevement a la source connu pour {}.\n\
             Il vient du compte-rendu metier DGFiP (retour de nature 94), pas\n\
             d'un calcul. Le recuperer avec :\n\
             \x20   paie retours\n\
             ou, a defaut, amorcer le magasin depuis les DSN deja deposees :\n\
             \x20   paie pas --amorcer\n\
             En attendant, la DSN ne s'arrete pas : elle declare la grille de\n\
             l'article 204 H, qui preleve le plus souvent PLUS que le taux du\n\
             salarie.",
            cle
        )));
    };
    Ok(avec_validite(e, le))
}

/// `(taux, type, identifiant)` prets pour S21.G00.50.006/.007/.008.
pub fn rubriques(ctx: &Contexte, cle: &str) -> R<Triplet> {
    let e = etat(ctx, cle, None)?;
    let champ = |c: &str| e.get(c).cloned().ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent(c)));
    Ok((champ("taux")?, champ("type")?, champ("identifiant")?))
}

pub fn connu(ctx: &Contexte, cle: &str) -> Resultat<bool> {
    Ok(entree(&charger(ctx)?, cle).is_some())
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

/// `str(le)` d'un `Optional[date]`.
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
    Ok(dec_py(&valeur_datee(v, le)?)?)
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
        if montant >= dec_py(bas)? && (haut.is_null() || montant < dec_py(haut)?) {
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

fn plancher(x: Decimal, zero: Decimal) -> Decimal {
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
    let prelevement = pct(soumis, dec_py(&Value::String(taux.clone()))?);
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

/// Le triplet a declarer, ou `None` : pas de taux DGFiP applicable, c'est la
/// grille. Un taux perime compte pour rien.
pub fn taux_dgfip(ctx: &Contexte, cle: &str, le: Option<Jour>) -> Resultat<Option<Triplet>> {
    let s = situation(ctx, cle, le)?;
    if s["situation"] != Value::String(VALIDE.into()) {
        return Ok(None);
    }
    Ok(Some((s["taux"].clone(), s["type"].clone(), s["identifiant"].clone())))
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
        prelevement: pct(soumis, dec_py(t)?),
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

/// Ou en est le taux d'un salarie. Ne refuse que si le magasin est illisible.
pub fn situation(ctx: &Contexte, cle: &str, le: Option<Jour>) -> Resultat<Value> {
    let le = le_jour(ctx, le);
    let magasin = charger(ctx)?;
    let e = presente(entree(&magasin, cle));
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
    m.insert(
        "demande".into(),
        magasin.get("demandes").and_then(|d| d.get(cle)).cloned().unwrap_or(Value::Null),
    );
    Ok(Value::Object(m))
}

/// La demande en cours pour ce salarie, ou `None`.
pub fn demande(ctx: &Contexte, cle: &str) -> Resultat<Option<Value>> {
    Ok(charger(ctx)?.get("demandes").and_then(|d| d.get(cle)).cloned())
}

fn archive(ctx: &Contexte, cle: &str) -> bool {
    ctx.salaries.get(cle).map_or(false, |f| vrai(f) && f.get("actif") == Some(&Value::Bool(false)))
}

/// Le meme tableau pour plusieurs salaries, dans l'ordre alphabetique. Sans
/// liste : ceux que le magasin ou les fiches connaissent, moins les archives.
pub fn diagnostic(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>) -> Resultat<Vec<Value>> {
    let cles: BTreeSet<String> = match cles {
        Some(c) => c.iter().cloned().collect(),
        None => {
            let magasin = charger(ctx)?;
            let mut s: BTreeSet<String> = magasin.get("salaries").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default();
            s.extend(ctx.salaries.keys().cloned());
            s.into_iter().filter(|c| !archive(ctx, c)).collect()
        }
    };
    cles.iter().map(|c| situation(ctx, c, le)).collect()
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

/// Les phrases a dire avant de declarer. Vide si tout va bien.
pub fn avertissements(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>) -> Resultat<Vec<String>> {
    let mut lignes = Vec::new();
    for s in diagnostic(ctx, cles, le)? {
        if s["situation"] == Value::String(VALIDE.into()) {
            continue;
        }
        let mut ligne = format!("{} : {}", en_texte(&s["salarie"]), en_texte(&s["libelle"]));
        if s["situation"] == Value::String(PERIME.into()) {
            ligne += &format!(" (il l'etait jusqu'au {})", fr(&s["valable_jusqu_a"]));
        }
        let dem = &s["demande"];
        if vrai(dem) {
            ligne += &format!(
                " -- demande partie le {}, reponse attendue avant le {} : attendre si le calendrier le permet",
                fr(dem.get("quand").unwrap_or(&Value::Null)),
                fr(dem.get("reponse_attendue_avant").unwrap_or(&Value::Null))
            );
        }
        lignes.push(ligne);
    }
    Ok(lignes)
}

// ------------------------------------------------------------ les paies

/// Le prelevement d'une paie qu'une DSN deposee a deja declare, ou `None` : le mois n'est pas depose (ni au journal, ni
/// repris de l'historique), la paie n'a pas de NIR ou pas de numero de contrat (la DSN ne se lit pas par eux), ou la DSN
/// deposee ne porte pas son versement (une paie ajoutee apres le depot, que la correction du mois suivant declarera).
///
/// LE PAS D'UN MOIS DEPOSE SE LIT, IL NE SE RECALCULE PAS (lot D2 ; regles 1 et 4). CGI art. 1671, 1 et 2
/// (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`, l. 99 et 102) : la retenue « est effectuée
/// par le débiteur lors du paiement », au taux que l'administration a transmis et que l'employeur applique « au plus
/// tard le deuxième mois suivant sa transmission ». Ce qui a ete retenu l'a ete au paiement, au taux de ce jour-la, et
/// la DSN deposee le dit (S21.G00.50.006 a 50.013) ; le recalculer au taux du jour (celui que le magasin porte quand on
/// relit le mois : `taux_dgfip(ctx, cle, None)`) imprimait sur le bulletin relu, le livre de paie, l'etat du
/// prelevement, la fiche recapitulative et les ecritures un impot qui n'a pas ete preleve (PASSATION-R3, « Resté
/// ouvert »). Le mois de la DSN est celui de la fin de la periode de la paie, comme le declare `dsn::bloc_versement`
/// (50.001) ; l'etablissement, celui du salarie.
fn deja_declare(ctx: &Contexte, pa: &Paie) -> Result<Option<Versement>, String> {
    let e = &pa.engagement;
    let nir = pa.salarie.get("nir").filter(|n| vrai(n)).map(en_texte).unwrap_or_default();
    let numero = e.numero.clone().unwrap_or_default();
    if nir.is_empty() || numero.is_empty() {
        return Ok(None);
    }
    let fin = e.fin_periode()?;
    let mois = format!("{:04}-{:02}", fin.an, fin.mois);
    let nic = droit::bareme::etablissement_du_salarie(&ctx.entreprise, &pa.salarie)?;
    let Some(chemin) = crate::casc::dsn_deposee(ctx, &mois, Some(&nic))? else { return Ok(None) };
    let lu = crate::versement_declare::pas_declare(&chemin, &mois, &nir, &numero)
        .map_err(|m| format!("Le prelevement a la source de {} (contrat {}) se lit dans la DSN deposee : {}. Rien n'est ecrit.", mois, numero, m))?;
    let Some(d) = lu else { return Ok(None) };
    let zero = Decimal::new(0, 2);
    let abattement = d.abattement.unwrap_or_else(|| eur(plancher(sub(d.net_imposable, d.soumis), zero)));
    Ok(Some(Versement {
        taux: Value::String(d.taux),
        type_taux: Value::String(d.type_taux),
        identifiant: d.identifiant.map_or(Value::Null, Value::String),
        abattement,
        montant_soumis: d.soumis,
        prelevement: d.prelevement,
    }))
}

/// Ce que `du_versement` dit de chaque paie, par POSITION dans `paies` (`None` :
/// paie sans bulletin, ou bareme sans grille -- on n'imprime pas un impot qu'on
/// ne sait pas calculer). Le taux DGFiP se lit une fois par salarie ;
/// l'abattement se calcule une fois par versement. Une paie qu'une DSN deposee a
/// deja declaree garde le prelevement que la DSN dit (`deja_declare`).
pub fn des_paies(ctx: &Contexte, paies: &[Paie]) -> Result<Vec<Option<Versement>>, String> {
    let mut taux: Vec<(String, Option<Triplet>)> = Vec::new();
    let mut out = Vec::new();
    for pa in paies {
        let e = &pa.engagement;
        if e.sans_paie() {
            out.push(None);
            continue;
        }
        if let Some(v) = deja_declare(ctx, pa)? {
            out.push(Some(v));
            continue;
        }
        let cle = en_texte(pa.salarie.get("_cle").ok_or_else(|| socle::fichiers::absent("_cle"))?);
        let i = match taux.iter().position(|(c, _)| *c == cle) {
            Some(i) => i,
            None => {
                taux.push((cle.clone(), taux_dgfip(ctx, &cle, None)?));
                taux.len() - 1
            }
        };
        let fin = e.fin_periode()?;
        let bareme = ctx.bareme(&Jour { an: fin.an, mois: fin.mois, jour: 1 })?;
        let t = |j: Jour| (j.an, j.mois, j.jour);
        let court = contrat_court(&*e.nat()?, t(e.debut), e.fin.map(t), t(fin));
        match du_versement(taux[i].1.as_ref(), &bareme, pa.net_imposable, Some(fin), court, ZONE_DEFAUT) {
            Ok(v) => out.push(Some(v)),
            Err(ErreurPas::SansGrille(_)) => out.push(None),
            Err(autre) => return Err(autre.message().to_string()),
        }
    }
    Ok(out)
}
