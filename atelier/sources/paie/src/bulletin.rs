//! La description du bulletin de paie (`moteur/bulletin.py`, FORMAT.md de
//! `fabrique-pdf`) : des blocs JSON que la fabrique met en page. Ce module
//! s'arrete a la `Value` de la description ; l'ecriture du PDF est
//! `fabrique::ecrire`.
//!
//! Hors de ce module : la relecture d'un PDF ecrit (`apercu_dsn::numeros_archives`).

use engagement::calcul::Paie;
use engagement::conges::{compteur_au_bulletin, CompteurConges};
use droit::contexte::Contexte;
use droit::cumul::{tableau_cumul, Colonne};
use engagement::cumul::lire_engagements;
use socle::date::{d, dernier_du_mois, en_texte, forme_ecrite, Jour};
use droit::emplois;
use socle::fichiers::vrai;
use socle::num::{dec, dec_f64, div, sub, Decimal, Resultat};
use droit::pas::Versement;
use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `fabrique.FORMAT`.
pub const FORMAT: i64 = 3;

pub const ROUGE: &str = "#B22222";
pub const ARDOISE: &str = "#2C3E50";
const GRIS: &str = "#555555";
const GRIS_CLAIR: &str = "#777777";
const FOND_FAMILLE: &str = "#f4f0f0";
const FOND_BRUT: &str = "#fbf7f7";
const NBSP: &str = "\u{a0}";
const BLANC: &str = "#FFFFFF";

/// Emplacement du logo sur la page, en points PDF (A4 = 595 x 842).
pub const LOGO_LARGEUR: i64 = 74;
pub const LOGO_MARGE_DROITE: i64 = 34;
pub const LOGO_MARGE_HAUT: i64 = 28;

/// Dans l'ordre de preference : le vectoriel d'abord.
pub const IDENTITE_EXTENSIONS: [&str; 3] = [".svg", ".png", ".jpg"];

// ------------------------------------------------------------ les formats (util)

/// Un `Decimal` arrondi a `dp` decimales au pair sur le demi (le contexte de
/// `format(Decimal, ".2f")`), ecrit avec ses decimales ; le signe est celui de l'original.
fn fixe(x: Decimal, dp: u32) -> (bool, String) {
    let neg = x.is_sign_negative();
    let mut r = x.abs().round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
    r.rescale(dp);
    (neg, r.to_string())
}

/// `util.fmt_eur` : 1234.5 -> « 1 234,50 ».
pub fn fmt_eur(x: Decimal) -> String {
    let (neg, s) = fixe(x, 2);
    let (entier, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut groupes = String::new();
    for (i, c) in entier.chars().enumerate() {
        if i > 0 && (entier.len() - i) % 3 == 0 {
            groupes.push(' ');
        }
        groupes.push(c);
    }
    format!("{}{},{}", if neg { "-" } else { "" }, groupes, frac)
}

/// `util.fmt_taux` : 9.1 -> « 9,100 ».
pub fn fmt_taux(x: Decimal) -> String {
    let (neg, s) = fixe(x, 3);
    format!("{}{}", if neg { "-" } else { "" }, s.replace('.', ","))
}

/// `util.fmt_heures` : 24 -> « 24 », 13.5 -> « 13,5 ».
pub fn fmt_heures(x: Decimal) -> String {
    x.normalize().to_string().replace('.', ",")
}

/// `util.jj_mm_aaaa` d'une valeur.
pub fn jj_mm_aaaa(v: &Value) -> Resultat<String> {
    Ok(d(v)?.jj_mm_aaaa())
}

/// `_jours` : « 2,5 », et « 2 » quand c'est rond (`quantize` au pair, comme le contexte).
pub fn jours(x: Decimal) -> String {
    let q = x.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven).normalize();
    q.to_string().replace('.', ",")
}

/// `jours` d'un nombre de jours facultatif : refuse quand il manque.
fn jours_option(x: Option<f64>) -> Resultat<String> {
    match x {
        Some(x) => Ok(jours(dec_f64(x)?)),
        None => Err("nombre de jours absent".to_string()),
    }
}

// ------------------------------------------------------------ les briques

fn num(x: f64) -> Value {
    json!(x)
}

fn requis<'a>(o: &'a Value, cle: &str) -> Resultat<&'a Value> {
    o.get(cle).ok_or_else(|| socle::fichiers::absent(cle))
}

/// `str(o[cle])`.
fn texte(o: &Value, cle: &str) -> Resultat<String> {
    Ok(en_texte(requis(o, cle)?))
}

/// `str(o.get(cle) or defaut)`.
fn ou(o: &Value, cle: &str, defaut: &str) -> String {
    match o.get(cle) {
        Some(v) if vrai(v) => en_texte(v),
        _ => defaut.to_string(),
    }
}

/// `_m` : un morceau de texte d'un paragraphe ou d'une cellule.
fn m(texte: impl Into<String>, gras: bool, couleur: Option<&str>) -> Value {
    let mut o = Map::new();
    o.insert("texte".into(), Value::String(texte.into()));
    if gras {
        o.insert("gras".into(), Value::Bool(true));
    }
    if let Some(c) = couleur {
        o.insert("couleur".into(), c.into());
    }
    Value::Object(o)
}

fn mt(texte: impl Into<String>) -> Value {
    m(texte, false, None)
}

/// `_para`.
fn para(morceaux: Vec<Value>, taille: f64, couleur: Option<&str>, avant: f64, apres: f64) -> Value {
    let mut o = Map::new();
    o.insert("type".into(), "paragraphe".into());
    o.insert("morceaux".into(), Value::Array(morceaux));
    if taille != 0.0 {
        o.insert("taille".into(), num(taille));
    }
    if let Some(c) = couleur {
        o.insert("couleur".into(), c.into());
    }
    if avant != 0.0 {
        o.insert("espace_avant".into(), num(avant));
    }
    if apres != 0.0 {
        o.insert("espace_apres".into(), num(apres));
    }
    Value::Object(o)
}

/// `_cell`.
fn cell(texte: impl Into<String>, etendue: i64, gras: bool, couleur: Option<&str>) -> Value {
    let mut o = Map::new();
    o.insert("texte".into(), Value::String(texte.into()));
    if etendue != 1 {
        o.insert("etendue".into(), json!(etendue));
    }
    if gras {
        o.insert("gras".into(), Value::Bool(true));
    }
    if let Some(c) = couleur {
        o.insert("couleur".into(), c.into());
    }
    Value::Object(o)
}

fn cel(texte: impl Into<String>, etendue: i64) -> Value {
    cell(texte, etendue, false, None)
}

fn s(texte: impl Into<String>) -> Value {
    Value::String(texte.into())
}

/// `_ligne_tableau` : les options absentes (le faux Python) ne s'ecrivent pas.
fn ligne(cellules: Vec<Value>, options: Vec<(&str, Value)>) -> Value {
    let mut o = Map::new();
    o.insert("cellules".into(), Value::Array(cellules));
    for (k, v) in options {
        o.insert(k.into(), v);
    }
    Value::Object(o)
}

fn gras() -> (&'static str, Value) {
    ("gras", Value::Bool(true))
}

fn filet_haut(couleur: &str, epaisseur: f64) -> (&'static str, Value) {
    ("filet_haut", json!({"couleur": couleur, "epaisseur": epaisseur}))
}

/// Les cellules des tableaux : 7 pt, 1,4 pt au-dessus et au-dessous, 3 pt de chaque cote.
fn marge_cellule() -> Value {
    json!({"haut": 1.4, "droite": 3, "bas": 1.4, "gauche": 3})
}

/// `_tableau` : premiere colonne a gauche, les autres a droite, un filet fin sous chaque ligne.
fn tableau(largeurs: &[f64], entete: Vec<Value>, lignes: Vec<Value>, filet: &str) -> Value {
    let colonnes: Vec<Value> = largeurs
        .iter()
        .enumerate()
        .map(|(i, l)| json!({"largeur": l, "alignement": if i == 0 { "gauche" } else { "droite" }}))
        .collect();
    json!({
        "type": "tableau", "taille": 7, "interligne": 1.27,
        "colonnes": colonnes, "marge_cellule": marge_cellule(),
        "filet_lignes": {"couleur": filet, "epaisseur": 0.3},
        "entete": entete, "lignes": lignes
    })
}

/// `_ligne_net` : une ligne « net », blanc sur rouge, 9,5 pt, en gras.
fn ligne_net(libelle: &str, montant: Decimal, colonnes: i64) -> Value {
    ligne(
        vec![cel(libelle, colonnes - 1), s(fmt_eur(montant))],
        vec![gras(), ("fond", ROUGE.into()), ("couleur", BLANC.into()), ("taille", num(9.5))],
    )
}

// ------------------------------------------------------------ ce que le bulletin dit

/// `_smic_de_la_reduction`, apres `html.unescape` : le SMIC de la reduction
/// generale, celui du CALCUL (gele a 12,02 en 2026), pas le SMIC du jour.
pub fn smic_de_la_reduction(p: &Paie) -> Resultat<String> {
    if let Some(rg) = &p.reduction_generale {
        if !rg.heures.is_zero() {
            return Ok(format!(
                "{} € ({} h à {})",
                fmt_eur(rg.smic_reference),
                fmt_heures(rg.heures),
                fmt_eur(div(rg.smic_reference, rg.heures))
            ));
        }
    }
    Ok(format!("{} € ({} h à {})", fmt_eur(p.plafond), fmt_heures(p.heures), fmt_eur(dec_f64(p.smic)?)))
}

/// `entreprise.convention_du_contrat` : (IDCC, libelle) d'un contrat.
pub fn convention_du_contrat(ent: &Value, sp: &Value, sal: &Value) -> Resultat<(String, String)> {
    let idcc = ou(ent, "idcc", "");
    let libelle = ou(ent, "convention_collective", "");
    let tv = sp.get("emission_tv_1734").map_or(false, vrai);
    if (idcc != "2642" && idcc != "3241") || !tv {
        return Ok((idcc, libelle));
    }
    if ou(sal, "statut", "artiste") != "artiste" {
        return Ok((idcc, libelle));
    }
    let code = sal.get("code_emploi_ft").filter(|c| vrai(c)).map(en_texte);
    let metier = match &code {
        Some(c) => emplois::metier(c)?,
        None => None,
    };
    let famille = metier.as_ref().and_then(|m| m.get("famille")).filter(|f| !f.is_null());
    match famille {
        None => Err(format!(
            "Spectacle {}, émission de télévision (convention 1734) : le métier {} de {} n'est pas au catalogue, et \
             le logiciel ne sait pas s'il est artiste-interprète (1734) ou musicien, metteur en scène, artiste de \
             complément (convention de l'entreprise). Choisir un métier du catalogue.",
            ou(sp, "titre", ""),
            code.unwrap_or_else(|| "(aucun)".into()),
            ou(sal, "nom", "ce salarié")
        )),
        Some(f) if matches!(f.as_str(), Some("Artistes") | Some("Cirque et rue")) => {
            Ok(("1734".into(), "Artistes-interprètes engagés pour des émissions de télévision".into()))
        }
        Some(_) => Ok((idcc, libelle)),
    }
}

/// Un bulletin qui remplace en entier celui qui est parti (`{"remplace": true, "rectifie", "mois_origine"}`) : la paie du
/// mois refaite avant que sa DSN ne soit close, imprimee comme un bulletin du mois, sous un nouveau numero. Sans
/// `remplace`, c'est la regularisation d'un mois clos, versee avec une paie suivante (`rectificatif_lignes`).
pub fn remplace_en_entier(r: &Value) -> bool {
    r.get("remplace").is_some_and(vrai)
}

/// `_rectificatif_lignes` : le bas d'un bulletin RECTIFICATIF.
fn rectificatif_lignes(r: &Value, colonnes: i64) -> Resultat<Vec<Value>> {
    let lignes = [
        ("Net à payer avant impôt du bulletin rectifié", "net_avant"),
        ("Net à payer avant impôt rectifié", "net_apres"),
        ("Différence de net à payer", "difference_net"),
        ("Différence de net imposable", "difference_net_imposable"),
    ];
    let mut lignes_dec = Vec::new();
    for (libelle, cle) in lignes {
        lignes_dec.push((libelle, dec(requis(r, cle)?)?));
    }
    let mut out = vec![ligne(
        vec![cel(format!("RÉGULARISATION VERSÉE AVEC LA PAIE DE {}", texte(r, "mois_versement")?.to_uppercase()), colonnes)],
        vec![gras(), filet_haut(ROUGE, 0.6)],
    )];
    for (libelle, montant) in lignes_dec {
        out.push(ligne(vec![cel(libelle, colonnes - 1), s(fmt_eur(montant))], vec![]));
    }
    let prelevement = match r.get("prelevement") {
        None | Some(Value::Null) => {
            out.push(ligne(vec![cel("Impôt prélevé à la source sur la différence", colonnes - 1), s("non calculé")], vec![]));
            return Ok(out);
        }
        Some(p) => dec(p)?,
    };
    let libelle = if vrai(requis(r, "indu")?) {
        "Régularisation du prélèvement à la source (taux du mois de l'indu)"
    } else {
        "Impôt sur le revenu prélevé à la source sur la différence"
    };
    out.push(ligne(
        vec![cel(format!("{} (taux {} %)", libelle, fmt_taux(dec(requis(r, "taux")?)?)), colonnes - 1), s(fmt_eur(prelevement))],
        vec![],
    ));
    let a_verser = dec(requis(r, "a_verser")?)?;
    out.push(ligne_net(if a_verser >= Decimal::ZERO { "NET À VERSER" } else { "NET À RETENIR" }, a_verser.abs(), colonnes));
    Ok(out)
}

/// `_lignes_pas_tableau` : l'impot a la source, et le net paye. La ligne du prelevement suit le modele de l'arrete du
/// 25/02/2016 (art. 1, I : « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE », colonnes Base, Taux, Salarié) : son assiette
/// (le montant soumis, apres l'abattement d'un contrat court ; C. trav. R. 3243-1, 9°), son taux, son montant. Le
/// « Cumul annuel » du modele (art. 1, V) n'est pas imprime : le moteur ne le calcule pas.
fn lignes_pas_tableau(p: &Paie, pas: Option<&Versement>, colonnes: i64) -> Resultat<Vec<Value>> {
    let taux = match pas.map(|v| &v.taux) {
        Some(t) if vrai(t) => dec(t)?,
        _ => dec(&json!("0.00"))?,
    };
    let montant = pas.map_or(Decimal::new(0, 2), |v| v.prelevement);
    let assiette = pas.map_or(String::new(), |v| fmt_eur(v.montant_soumis));
    Ok(vec![
        ligne(
            vec![s("IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE"), s(assiette), s(fmt_taux(taux)), s(fmt_eur(montant))],
            vec![],
        ),
        ligne_net("MONTANT NET A PAYER (en Euros)", sub(p.net_a_payer, montant), colonnes),
    ])
}

/// `_tableau_cumul` : une colonne de libelles, une par fenetre.
fn tableau_des_cumuls(colonnes: &[Colonne]) -> Value {
    let n = colonnes.len() as f64;
    let mut cellules = vec![cell("CUMULS", 1, true, Some(ARDOISE))];
    for c in colonnes {
        cellules.push(json!({"morceaux": [m(c.titre.clone(), true, None), mt("\n"), mt(c.sous_titre.clone())]}));
    }
    let entete = vec![ligne(
        cellules,
        vec![("couleur", ROUGE.into()), ("taille", num(6.8)), ("fond", FOND_FAMILLE.into()), filet_haut(ROUGE, 0.8)],
    )];
    let enseigne = colonnes.iter().any(|c| c.enseignement != 0);
    // (libelle, valeur de chaque colonne, suffixe)
    type Rang = (&'static str, fn(&Colonne) -> String, &'static str);
    let mut rangs: Vec<Rang> = vec![
        ("Cachets", |c| c.cachets.to_string(), ""),
        ("Heures déclarées", |c| c.heures.to_string(), " h"),
        ("Total heures (cachets × 12 + heures)", |c| c.heures_equivalentes.to_string(), " h"),
    ];
    if enseigne {
        rangs.push(("dont enseignement (droits seuls, pas le montant)", |c| c.enseignement.to_string(), " h"));
        rangs.push((
            "Heures retenues pour les 507 h",
            |c| c.heures_ouverture.map_or("None".to_string(), |h| h.to_string()),
            " h",
        ));
    }
    rangs.push(("Salaire brut", |c| fmt_eur(c.brut), " €"));
    let lignes = rangs
        .iter()
        .map(|(libelle, valeur, suffixe)| {
            let mut cel = vec![s(*libelle)];
            cel.extend(colonnes.iter().map(|c| s(format!("{}{}", valeur(c), suffixe))));
            ligne(cel, vec![])
        })
        .collect();
    let mut largeurs = vec![0.40];
    largeurs.extend(std::iter::repeat(0.60 / n).take(colonnes.len()));
    tableau(&largeurs, entete, lignes, "#eeeeee")
}

/// `_tableau_conges` : le compteur de conges payes d'un permanent.
fn tableau_conges(cpt: &CompteurConges) -> Value {
    let mut lignes = vec![("Acquis sur la période", cpt.acquis), ("Pris", cpt.pris)];
    if !cpt.reports.is_zero() {
        lignes.insert(1, ("Report de la période précédente", cpt.reports));
    }
    let entete = vec![ligne(
        vec![
            json!({"morceaux": [
                m("CONGÉS PAYÉS", true, None), mt("\n"),
                mt(format!("période du {} au {}", cpt.periode_debut.jj_mm_aaaa(), cpt.periode_fin.jj_mm_aaaa()))],
                "couleur": ARDOISE}),
            cell("Jours ouvrables", 1, true, None),
        ],
        vec![("couleur", ROUGE.into()), ("taille", num(6.8)), ("fond", FOND_FAMILLE.into()), filet_haut(ROUGE, 0.8)],
    )];
    let mut corps: Vec<Value> =
        lignes.iter().map(|(l, v)| ligne(vec![s(*l), s(format!("{} j", jours(*v)))], vec![])).collect();
    corps.push(ligne(vec![s("Solde"), s(format!("{} j", jours(cpt.solde())))], vec![gras()]));
    tableau(&[0.40, 0.60], entete, corps, "#eeeeee")
}

// ------------------------------------------------------------ le modele de l'arrete

/// Les colonnes du tableau du bulletin : le libelle, puis celles du modele de l'arrete du 25/02/2016, art. 1, I
/// (redaction du 25/06/2024 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/
/// LEGIARTI000049854119.xml) : « Base », « Taux », « Salarié », « Employeur ». Un seul taux : celui du salarie, le seul
/// que le code du travail exige (R. 3243-1, 8°, a : « pour les cotisations et contributions [...] a la charge du
/// salarie, leurs taux ») ; le modele met « - » au taux des lignes que l'employeur paie seul (accidents du travail,
/// famille). Jusqu'a la vague V2g : six colonnes, taux et montant employeur avant taux et montant salarie. Les deux
/// modeles impriment ce tableau-la (lot D2 : voir `Modele`).
const COLONNES: i64 = 5;
const LARGEURS: [f64; 5] = [0.55, 0.12, 0.09, 0.12, 0.12];
/// L'annexe du modele *complet* : le libelle, la base, le taux et le montant de chaque part.
const COLONNES_ANNEXE: i64 = 6;
const LARGEURS_ANNEXE: [f64; 6] = [0.46, 0.11, 0.09, 0.11, 0.11, 0.12];

/// LES DEUX MODELES DE BULLETIN (PLAN-CONTRATS, C, points 9 et 11, decides le 30/09/2026 ; lot R11, 11/10/2026 ; lot
/// D2, 13/10/2026).
///
/// - *Simplifie* : le modele de l'arrete du 25/02/2016, art. 1, I.
/// - *Complet* : le MEME bulletin, ligne pour ligne, suivi d'une page annexe qui detaille chaque cotisation et
///   contribution sous la ligne du modele qui la porte (sa base, le taux et le montant de chaque part, le taux employeur
///   compris). Le BOSS, FAQ « Règles générales relatives au bulletin de paie », Q 10
///   (atelier/reference/sources/montant-net-social/boss-regles-generales-bulletin-de-paie-20250207-archive-web.txt,
///   l. 170-172, copie de l'archive du web, a redeposer depuis le BOSS) : « Il n'est pas possible d'indiquer plus de
///   détails pour la rubrique « Cotisations et contributions sociales obligatoires » [...] son uniformisation permet
///   d'assurer la lisibilité entre les bulletins des différents employeurs » ; « il est autorisé de fournir le détail de
///   certaines rubriques (cotisations statutaires, autres charges dues, complémentaire garantie frais de santé
///   obligatoire, etc.) au verso du bulletin de paie ou bien sur un document annexe ». Jusqu'au lot D2, le complet
///   ajoutait une colonne « Taux employeur » et des lignes « dont ... » DANS la rubrique : c'est ce que Q 10 refuse.
///   L'annexe ne porte pas la marque « Bulletin de paie n° » : le lecteur des archives (`apercu_dsn::numeros_archives`)
///   la range dans les pages du bulletin qu'elle suit.
///
/// QUI CHOISIT : la compagnie, pour tous (`modele_bulletin` de la fiche entreprise, pose par le selecteur de
/// Configuration) ; un salarie qui demande l'autre modele l'obtient par la case `modele_bulletin` de sa fiche (Avance),
/// qui l'emporte. Sans choix : le simplifie. Une valeur inconnue refuse (regle 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modele {
    Simplifie,
    Complet,
}

/// La case qui porte le choix, dans la fiche entreprise comme dans la fiche salarie.
pub const CASE_MODELE: &str = "modele_bulletin";

impl Modele {
    pub const TOUS: [Modele; 2] = [Modele::Simplifie, Modele::Complet];

    /// La valeur de la case (`champs-base.json`, forme « choix »).
    pub fn cle(self) -> &'static str {
        match self {
            Modele::Simplifie => "simplifié",
            Modele::Complet => "complet",
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Modele::Simplifie => "Simplifié",
            Modele::Complet => "Complet",
        }
    }

    /// Ce que la page en dit, sous l'apercu.
    pub fn description(self) -> &'static str {
        match self {
            Modele::Simplifie => {
                "Le bulletin clarifié du modèle réglementaire (arrêté du 25 février 2016) : les cotisations regroupées \
                 sous les lignes du modèle, le taux de la part salariale, les contributions de l'employeur en une ligne."
            }
            Modele::Complet => {
                "Le même bulletin, ligne pour ligne, suivi d'une page annexe qui détaille chaque cotisation et \
                 contribution sous la ligne du bulletin qui la regroupe : sa base, le taux et le montant de la part \
                 salariale et de la part employeur."
            }
        }
    }

    /// Le modele que dit la case `modele_bulletin` d'une fiche : vide ou absente, aucun ; inconnue, refus.
    pub fn de_la_fiche(fiche: &Value, quelle: &str) -> Resultat<Option<Modele>> {
        let v = match fiche.get(CASE_MODELE) {
            None | Some(Value::Null) => return Ok(None),
            Some(Value::String(v)) => v.trim(),
            Some(autre) => {
                return Err(format!("La fiche {} dit un modèle de bulletin illisible : {} (un texte est attendu).", quelle, autre))
            }
        };
        if v.is_empty() {
            return Ok(None);
        }
        Modele::TOUS.iter().copied().find(|m| m.cle() == v).map(Some).ok_or_else(|| {
            format!(
                "La fiche {} demande le modèle de bulletin « {} », que le logiciel ne connaît pas : « simplifié » ou \
                 « complet ».",
                quelle, v
            )
        })
    }
}

/// Le modele du bulletin d'une paie : la case de la fiche du salarie l'emporte ; sinon celle de la compagnie ; sinon le
/// simplifie (PLAN-CONTRATS, C, point 11).
pub fn modele_du_bulletin(ent: &Value, sal: &Value) -> Resultat<Modele> {
    let nom = sal.get("_cle").map(en_texte).unwrap_or_default();
    match Modele::de_la_fiche(sal, &format!("du salarié {}", nom).trim().to_string())? {
        Some(m) => Ok(m),
        None => Ok(Modele::de_la_fiche(ent, "entreprise")?.unwrap_or(Modele::Simplifie)),
    }
}

/// Le style d'un titre de rubrique du modele (« COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES », « IMPOT SUR LE
/// REVENU »...) : en gras, rouge, sur fond rose.
fn style_section() -> Vec<(&'static str, Value)> {
    vec![gras(), ("couleur", ROUGE.into()), ("taille", num(6.8)), ("fond", FOND_FAMILLE.into())]
}

/// Ou une ligne de cotisation se range dans le modele : son rang (l'ordre du modele), le titre du groupe qui la porte
/// (« Santé », « Retraite »), et le libelle du modele quand elle est seule sur sa ligne du modele.
#[derive(Clone, PartialEq)]
enum Rang {
    /// « Santé » : ses lignes sous le titre du groupe.
    Sante,
    /// Une ligne du modele (accidents du travail, famille, chomage, Apec, cotisations conventionnelles, CSG,
    /// prevoyance) : seule, elle prend le libelle du modele ; a plusieurs (deux tranches d'Apec, FNAS et FCAP), le
    /// libelle du modele les titre et chacune garde le sien.
    Ligne(u8, &'static str),
    /// « Retraite » : ses lignes sous le titre du groupe.
    Retraite,
    /// « Autres charges dues par l'employeur » : UNE ligne, le seul montant employeur, toutes ses contributions
    /// agregees (C. trav. R. 3243-2, al. 2 ; arrete, art. 3), la caisse des Conges Spectacles comprise (lot D2).
    Autres,
    /// Une famille que le moteur ne range pas : titree par sa famille, comme avant la vague V2g.
    Hors(String),
    /// « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES ».
    Facultative(&'static str),
    /// « EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS ».
    Allegement,
}

impl Rang {
    /// Le libelle qui titre le rang dans l'annexe du modele complet : la ligne (ou le groupe) du bulletin qui le porte.
    fn titre(&self) -> &str {
        match self {
            Rang::Sante => "Santé",
            Rang::Retraite => "Retraite",
            Rang::Ligne(_, nom) | Rang::Facultative(nom) => nom,
            Rang::Autres => AUTRES,
            Rang::Hors(famille) => famille,
            Rang::Allegement => EXONERATIONS,
        }
    }
}

const SANTE_SECU: &str = "Sécurité Sociale Maladie Maternité Invalidité Décès";
const SANTE_COMPLEMENTAIRE: &str = "Complémentaire garanties frais de santé obligatoire";
const ATMP: &str = "Accidents du travail & maladies professionnelles";
const FAMILLE: &str = "Famille";
const CHOMAGE: &str = "Assurance chômage";
const APEC: &str = "Apec";
const AUTRES: &str = "Autres charges dues par l'employeur";
const CONVENTION: &str = "Cotisations statutaires ou prévues par la convention collective";
const CSG_DEDUCTIBLE: &str = "CSG déductible de l'impôt sur le revenu";
const CSG_NON_DEDUCTIBLE: &str = "CSG/CRDS non déductible de l'impôt sur le revenu";
const PREVOYANCE: &str = "Prévoyance, Incapacité, Invalidité, Décès, Autres";
const OBLIGATOIRES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES";
const TOTAL_OBLIGATOIRES: &str = "TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES";
const EXONERATIONS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";
const FACULTATIVES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES";

/// Le rang d'une ligne de cotisation, par son identifiant (le libelle et la famille sont ceux du bareme, que le
/// bulletin ne lit plus que pour une ligne qu'il ne connait pas). Le modele range la prevoyance, obligatoire ou non,
/// sous « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES », sa seule ligne « Prévoyance, Incapacité, Invalidité,
/// Décès, Autres » (art. 1, I) ; la FAQ du BOSS, Q 18 (meme fichier que `Modele`, l. 192-196) : une cotisation rendue
/// obligatoire par un accord de branche, et non par la loi, est facultative.
///
/// La caisse des Conges Spectacles : dans « Autres charges dues par l'employeur » (lot D2 ; jusque-la, une ligne a
/// elle, que le modele ne nomme pas). FAQ du BOSS, Q 11 (l. 173-174) : « Est-il possible d'afficher des lignes
/// supplémentaires pour certaines contributions propres à un secteur d'activité [...] (ex : BTP [...]) ? Non, il convient
/// de regrouper certaines contributions particulières dans les rubriques « autres » prévues à cet effet » ; Q 12
/// (l. 175-178) : « totaliser les contributions légales à la charge exclusive de l'employeur dans la rubrique « Autres
/// charges dues par l'employeur » ». La caisse est propre au secteur, legale (C. trav. L. 3141-32 et D. 7121-40,
/// atelier/reference/sources/code-du-travail/articles-20261013.md : l'employeur du spectacle « s'affilie ») et a la seule
/// charge de l'employeur (le bareme n'a pas de taux salarial).
fn rang(l: &engagement::calcul::LigneCotisation) -> Rang {
    let seule_employeur = l.taux_sal.is_none() && l.montant_sal.is_zero();
    match l.id.as_str() {
        "maladie" | "regime_local" | "mutuelle" => Rang::Sante,
        "atmp" => Rang::Ligne(1, ATMP),
        "vieil_plaf" | "vieil_deplaf" | "retraite_t1" | "retraite_t2" | "cet" => Rang::Retraite,
        "famille" => Rang::Ligne(3, FAMILLE),
        "chomage" => Rang::Ligne(4, CHOMAGE),
        "apec_t1" | "apec_t2" => Rang::Ligne(5, APEC),
        "autres_1" | "autres_2" | "autres_3" | "autres_4" | "versement_mobilite" | "conges_spec" if seule_employeur => {
            Rang::Autres
        }
        "ccn" | "fcap" | "paritarisme" => Rang::Ligne(8, CONVENTION),
        "csg_ded" => Rang::Ligne(9, CSG_DEDUCTIBLE),
        "csg_crds_nd" => Rang::Ligne(10, CSG_NON_DEDUCTIBLE),
        "reduction_generale" => Rang::Allegement,
        id if id == "prevoyance" || id.starts_with("prevoyance_") => Rang::Facultative(PREVOYANCE),
        _ => Rang::Hors(l.famille.clone()),
    }
}

/// L'ordre du modele : santé, accidents du travail, retraite, famille, chomage, Apec, autres charges, cotisations
/// conventionnelles, CSG deductible, CSG/CRDS non deductible, (ce que le moteur ne range pas).
fn ordre(r: &Rang) -> u8 {
    match r {
        Rang::Sante => 0,
        Rang::Retraite => 2,
        Rang::Ligne(n, _) => *n,
        Rang::Autres => 6,
        Rang::Hors(_) => 11,
        Rang::Facultative(_) => 20,
        Rang::Allegement => 30,
    }
}

fn obligatoire(r: &Rang) -> bool {
    !matches!(r, Rang::Facultative(_) | Rang::Allegement)
}

/// Les lignes de cotisation d'une paie, groupees par rang dans l'ordre du modele (un tri stable : les familles que le
/// moteur ne range pas gardent leur ordre entre elles, et les lignes d'un rang celui du moteur).
fn rangees(p: &Paie) -> Vec<(Rang, Vec<&engagement::calcul::LigneCotisation>)> {
    let mut rangees: Vec<(Rang, Vec<&engagement::calcul::LigneCotisation>)> = Vec::new();
    for l in &p.lignes {
        let r = rang(l);
        match rangees.iter_mut().find(|(x, _)| *x == r) {
            Some((_, v)) => v.push(l),
            None => rangees.push((r, vec![l])),
        }
    }
    rangees.sort_by_key(|(r, _)| ordre(r));
    rangees
}

/// Le libelle qu'une ligne de « Santé » ou de « Retraite » prend du modele ; celles qu'il ne nomme pas (le regime
/// local d'Alsace-Moselle, la CET d'un permanent, que le moteur calcule sur T1 + T2 sans la partager entre les deux
/// tranches) gardent le leur.
fn libelle_du_modele(l: &engagement::calcul::LigneCotisation) -> String {
    match l.id.as_str() {
        "maladie" => SANTE_SECU.into(),
        "mutuelle" => SANTE_COMPLEMENTAIRE.into(),
        "vieil_plaf" => "Sécurité Sociale Vieillesse plafonnée".into(),
        "vieil_deplaf" => "Sécurité Sociale Vieillesse déplafonnée".into(),
        "retraite_t1" => "Retraite complémentaire, CEG et CET T1".into(),
        "retraite_t2" => "Retraite complémentaire, CEG et CET T2".into(),
        _ => l.libelle.clone(),
    }
}

fn montant_ou_rien(x: Decimal) -> String {
    if x.is_zero() {
        String::new()
    } else {
        fmt_eur(x)
    }
}

/// La base d'une ligne, et son taux salarial et son taux employeur ; rien la ou le bareme masque base et taux.
fn base_et_taux(l: &engagement::calcul::LigneCotisation) -> Resultat<(String, String, String)> {
    if l.masquer_base_et_taux {
        return Ok((String::new(), String::new(), String::new()));
    }
    let taux = |t: Option<f64>| -> Resultat<String> {
        match t {
            Some(t) => Ok(fmt_taux(dec_f64(t)?)),
            None => Ok(String::new()),
        }
    };
    Ok((fmt_eur(l.base), taux(l.taux_sal)?, taux(l.taux_pat)?))
}

/// Une ligne de cotisation du bulletin : libelle, base (sauf `masquer_base_et_taux`), taux salarial, montant salarial,
/// montant employeur ; un montant nul ne s'ecrit pas.
fn ligne_cotisation(libelle: String, l: &engagement::calcul::LigneCotisation) -> Resultat<Value> {
    let ts = match l.taux_sal {
        Some(t) => fmt_taux(dec_f64(t)?),
        None => String::new(),
    };
    let base = if l.masquer_base_et_taux { String::new() } else { fmt_eur(l.base) };
    Ok(ligne(vec![s(libelle), s(base), s(ts), s(montant_ou_rien(l.montant_sal)), s(montant_ou_rien(l.montant_pat))], vec![]))
}

fn titre_groupe(titre: &str, colonnes: i64) -> Value {
    ligne(vec![cel(titre, colonnes)], vec![gras()])
}

/// Les lignes d'un groupe de rangs egaux, dans l'ordre du moteur.
fn lignes_du_rang(r: &Rang, lignes: &[&engagement::calcul::LigneCotisation]) -> Resultat<Vec<Value>> {
    let mut out = Vec::new();
    match r {
        Rang::Sante | Rang::Retraite => {
            out.push(titre_groupe(r.titre(), COLONNES));
            for l in lignes {
                out.push(ligne_cotisation(libelle_du_modele(l), l)?);
            }
        }
        Rang::Ligne(_, nom) | Rang::Facultative(nom) => {
            if let [seule] = lignes {
                out.push(ligne_cotisation((*nom).to_string(), seule)?);
            } else {
                out.push(titre_groupe(nom, COLONNES));
                for l in lignes {
                    out.push(ligne_cotisation(l.libelle.clone(), l)?);
                }
            }
        }
        Rang::Autres => {
            let total = lignes.iter().fold(Decimal::new(0, 2), |a, l| a + l.montant_pat);
            out.push(ligne(vec![s(AUTRES), s(""), s(""), s(""), s(fmt_eur(total))], vec![]));
        }
        Rang::Hors(famille) => {
            out.push(titre_groupe(famille, COLONNES));
            for l in lignes {
                out.push(ligne_cotisation(l.libelle.clone(), l)?);
            }
        }
        Rang::Allegement => {}
    }
    Ok(out)
}

/// Les sommes salariale et employeur des lignes des rangs que `filtre` retient.
fn somme_des_rangs(
    rangees: &[(Rang, Vec<&engagement::calcul::LigneCotisation>)],
    filtre: &dyn Fn(&Rang) -> bool,
) -> (Decimal, Decimal) {
    let zero = Decimal::new(0, 2);
    rangees.iter().filter(|(r, _)| filtre(r)).flat_map(|(_, v)| v.iter()).fold((zero, zero), |(s, e), l| {
        (s + l.montant_sal, e + l.montant_pat)
    })
}

/// Le milieu du bulletin, du brut au montant net social : le modele de l'arrete du 25/02/2016, art. 1, I.
/// - « COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES », ses lignes dans l'ordre et sous les libelles du modele ;
///   « Autres charges dues par l'employeur » agrege les contributions que l'employeur paie seul (C. trav. R. 3243-2,
///   al. 2 ; arrete, art. 3, `legi-arrete-20160225-bulletin/LEGIARTI000047098507.xml`).
/// - « TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES » : salarie et employeur, AVANT les exonerations (R. 3243-1,
///   8°, a : « avant deduction des exonerations et exemptions mentionnees au 13° »).
/// - « EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS » : les reductions qui ne sont pas deduites de chaque cotisation
///   (arrete, art. 4, `LEGIARTI000047098456.xml` : la reduction generale de L. 241-13 pour l'employeur ; la reduction
///   salariale des heures supplementaires de L. 241-17, que le moteur ne calcule pas).
/// - « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES » : la prevoyance.
/// Aucun montant ne change : chaque ligne de la paie s'imprime une fois, et les trois sommes refont les totaux de la
/// paie (sinon refus).
fn tableau_modele(p: &Paie) -> Resultat<Vec<Value>> {
    let rangees = rangees(p);
    let (oblig_sal, oblig_pat) = somme_des_rangs(&rangees, &obligatoire);
    let (fac_sal, fac_pat) = somme_des_rangs(&rangees, &|r| matches!(r, Rang::Facultative(_)));
    let (all_sal, all_pat) = somme_des_rangs(&rangees, &|r| *r == Rang::Allegement);
    if oblig_sal + fac_sal + all_sal != p.total_sal || oblig_pat + fac_pat + all_pat != p.total_pat {
        return Err(format!(
            "Le bulletin ne retrouve pas les totaux de la paie : salarie {} + {} + {} pour {}, employeur {} + {} + {} \
             pour {}. Une ligne de cotisation n'a pas trouve sa place dans le modele.",
            oblig_sal, fac_sal, all_sal, p.total_sal, oblig_pat, fac_pat, all_pat, p.total_pat
        ));
    }
    let mut out = vec![ligne(vec![cel(OBLIGATOIRES, COLONNES)], style_section())];
    for (r, lignes) in rangees.iter().filter(|(r, _)| obligatoire(r)) {
        out.extend(lignes_du_rang(r, lignes)?);
    }
    out.push(ligne(
        vec![cel(TOTAL_OBLIGATOIRES, 3), s(fmt_eur(oblig_sal)), s(fmt_eur(oblig_pat))],
        vec![gras(), filet_haut(ROUGE, 0.6)],
    ));
    if rangees.iter().any(|(r, _)| *r == Rang::Allegement) {
        out.push(ligne(vec![cel(EXONERATIONS, 3), s(montant_ou_rien(all_sal)), s(fmt_eur(all_pat))], vec![gras()]));
    }
    if rangees.iter().any(|(r, _)| matches!(r, Rang::Facultative(_))) {
        out.push(ligne(vec![cel(FACULTATIVES, COLONNES)], style_section()));
        for (r, lignes) in rangees.iter().filter(|(r, _)| matches!(r, Rang::Facultative(_))) {
            out.extend(lignes_du_rang(r, lignes)?);
        }
    }
    Ok(out)
}

/// L'annexe du modele complet (voir `Modele`) : une page qui suit le bulletin et detaille chacune de ses lignes de
/// cotisation -- chaque ligne du bareme sous la ligne (ou le groupe) du bulletin qui la porte, avec sa base, le taux et
/// le montant de la part salariale et de la part employeur --, puis les trois totaux du bulletin, aux memes montants.
/// Elle ne dit rien que la paie ne dise : chaque ligne de la paie s'y imprime une fois. Ni « Bulletin de paie n° », ni
/// « Contrat n° », ni « Période : » : ce sont les marques que lit `apercu_dsn::numeros_archives`.
fn blocs_annexe(p: &Paie) -> Resultat<Vec<Value>> {
    let sal = &p.salarie;
    let mut blocs = vec![
        para(vec![m("Détail des cotisations et contributions", true, None)], 11.0, Some(ROUGE), 0.0, 1.0),
        para(
            vec![mt(format!(
                "Document annexe au bulletin n° {} — {} {} — du {} au {}.\nIl détaille, sous chaque ligne du bulletin \
                 qui les regroupe, les cotisations et contributions de la paie : leur base, le taux et le montant de la \
                 part salariale et de la part employeur. Il ne remplace aucune ligne du bulletin ; les totaux sont les \
                 siens.",
                p.numero_bulletin.clone().unwrap_or_default(),
                texte(sal, "nom")?,
                texte(sal, "prenom")?,
                p.engagement.debut_periode()?.jj_mm_aaaa(),
                p.engagement.fin_periode()?.jj_mm_aaaa(),
            ))],
            7.0,
            Some(GRIS),
            0.0,
            6.0,
        ),
    ];
    let th = vec![("couleur", Value::from(BLANC)), gras(), ("taille", num(6.6)), ("fond", Value::from(ROUGE))];
    let entete = vec![ligne(
        vec![
            cell("LIBELLÉ", 1, true, None),
            s("Base"),
            s("Taux salarié"),
            s("Salarié"),
            s("Taux employeur"),
            s("Employeur"),
        ],
        th,
    )];
    let rangees = rangees(p);
    let detail = |r: &Rang, lignes: &[&engagement::calcul::LigneCotisation], corps: &mut Vec<Value>| -> Resultat<()> {
        // Les exonerations n'ont pas de titre : la ligne du bulletin qui les porte est leur total, qui les suit.
        if *r != Rang::Allegement {
            corps.push(titre_groupe(r.titre(), COLONNES_ANNEXE));
        }
        for l in lignes {
            let (base, ts, tp) = base_et_taux(l)?;
            corps.push(ligne(
                vec![
                    s(format!("{}{}{}", NBSP, NBSP, l.libelle)),
                    s(base),
                    s(ts),
                    s(montant_ou_rien(l.montant_sal)),
                    s(tp),
                    s(montant_ou_rien(l.montant_pat)),
                ],
                vec![],
            ));
        }
        Ok(())
    };
    let total = |libelle: &str, (sal, pat): (Decimal, Decimal)| {
        ligne(
            vec![cel(libelle, 3), s(montant_ou_rien(sal)), s(""), s(fmt_eur(pat))],
            vec![gras(), filet_haut(ROUGE, 0.6)],
        )
    };
    let mut corps = vec![ligne(vec![cel(OBLIGATOIRES, COLONNES_ANNEXE)], style_section())];
    for (r, lignes) in rangees.iter().filter(|(r, _)| obligatoire(r)) {
        detail(r, lignes, &mut corps)?;
    }
    corps.push(total(TOTAL_OBLIGATOIRES, somme_des_rangs(&rangees, &obligatoire)));
    for (r, lignes) in rangees.iter().filter(|(r, _)| *r == Rang::Allegement) {
        detail(r, lignes, &mut corps)?;
        corps.push(total(EXONERATIONS, somme_des_rangs(&rangees, &|r| *r == Rang::Allegement)));
    }
    if rangees.iter().any(|(r, _)| matches!(r, Rang::Facultative(_))) {
        corps.push(ligne(vec![cel(FACULTATIVES, COLONNES_ANNEXE)], style_section()));
        for (r, lignes) in rangees.iter().filter(|(r, _)| matches!(r, Rang::Facultative(_))) {
            detail(r, lignes, &mut corps)?;
        }
    }
    blocs.push(tableau(&LARGEURS_ANNEXE, entete, corps, "#e4e4e4"));
    Ok(blocs)
}

// ------------------------------------------------------------ un bulletin

/// Les arguments de `_blocs_un` pour une paie : ce que le bulletin dit en plus d'elle.
pub struct Jeu<'a> {
    pub paie: &'a Paie,
    pub colonnes: Option<&'a [Colonne]>,
    pub plafond: Option<Decimal>,
    pub conges: Option<&'a CompteurConges>,
    pub pas: Option<&'a Versement>,
    pub rectificatif: Option<&'a Value>,
    /// Les indices, dans `paie.remuneration`, des lignes hors assiette qui ne sont pas un revenu (remboursements de
    /// frais, acompte : `HorsAssiette::Frais` et `Mouvement`) : elles s'impriment sous « REMBOURSEMENTS ET DEDUCTIONS
    /// DIVERSES », entre le montant net social et le net a payer avant impot (arrete du 25/02/2016, art. 1, I ; BOSS,
    /// montant net social, Q 39). Vide : toutes les lignes sont au-dessus du brut.
    pub remboursements: Vec<usize>,
}

/// `_blocs_un` : les blocs d'UN bulletin.
fn blocs_un(j: &Jeu, ent: &Value) -> Resultat<Vec<Value>> {
    let p = j.paie;
    let (sal, e, sp) = (&p.salarie, &p.engagement, &p.spectacle);
    let conv = convention_du_contrat(ent, sp, sal)?;
    let n = COLONNES;
    let nat = e.nat()?;
    let permanent = nat.mensuelle();
    let numero = e.numero.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "-".to_string());
    let tiret = format!("{}—{}", NBSP, NBSP);
    let barre = format!(" {}|{} ", NBSP, NBSP);

    let nom_affichage = texte(ent, "nom_affichage")?;
    let adresse_defaut = texte(ent, "adresse")?;
    let adresse = ent.get("adresse_affichage").map_or(adresse_defaut, en_texte);
    let code_postal = texte(ent, "code_postal")?;
    let commune_defaut = texte(ent, "commune")?;
    let commune = ent.get("commune_affichage").map_or(commune_defaut, en_texte);
    let (siret, naf, licence) = (texte(ent, "siret")?, texte(ent, "naf")?, texte(ent, "licence_spectacle")?);
    let mut blocs = vec![
        para(vec![m(nom_affichage, true, None)], 13.0, Some(ARDOISE), 0.0, 0.0),
        para(
            vec![mt(format!(
                "{} — {} {}\nSiret {} {}·{} APE {} {}·{} Licence {}",
                adresse, code_postal, commune, siret, NBSP, NBSP, naf, NBSP, NBSP, licence
            ))],
            6.6,
            Some(GRIS),
            0.0,
            0.0,
        ),
    ];
    let titre_rectif = if j.rectificatif.is_some() { " — RECTIFICATIF" } else { "" };
    blocs.push(para(
        vec![m(
            format!("Bulletin de paie n° {}{}", p.numero_bulletin.clone().unwrap_or_default(), titre_rectif),
            true,
            None,
        )],
        11.0,
        Some(ROUGE),
        8.0,
        1.0,
    ));
    if let Some(r) = j.rectificatif {
        let rectifie = match r.get("rectifie") {
            Some(v) if vrai(v) => format!("n° {}", en_texte(v)),
            _ => format!("du contrat {}", numero),
        };
        let suite = match r.get("motif").filter(|v| vrai(v)) {
            Some(motif) => format!(", qu'il remplace. Motif : {}.", en_texte(motif)),
            None => ", qu'il remplace.".to_string(),
        };
        blocs.push(para(
            vec![m(format!("Rectifie le bulletin {} de {}", rectifie, texte(r, "mois_origine")?), true, None), mt(suite)],
            7.0,
            Some(GRIS),
            0.0,
            5.0,
        ));
    }
    let k = |t: &str| m(t, false, Some(GRIS_CLAIR));
    let sous_titre;
    let ligne_caisse;
    let travail;
    if permanent {
        sous_titre = format!(
            "Durée contractuelle : {} h par mois {} durée de référence de l'entreprise : {} h",
            jours(e.quotite()?),
            tiret,
            jours_option(e.quotite_reference()?)?
        );
        ligne_caisse = Vec::new();
        travail = vec![m("Durée du travail :", true, None), mt(format!(" {} h", jours(e.quotite()?)))];
    } else {
        sous_titre =
            format!("Numéro d'objet : {} {} Spectacle : {}", texte(sp, "numero_objet")?, tiret, texte(sp, "titre")?);
        ligne_caisse = vec![
            k("N° Congés Spectacles :"),
            mt(format!(" {}\n", ou(sal, "num_conges_spectacles", "-"))),
        ];
        let dates: Vec<String> = e.dates_travail.iter().map(Jour::jj_mm_aaaa).collect();
        travail = vec![m("Dates travaillées :", true, None), mt(format!(" {}", dates.join(", ")))];
    }
    blocs.push(para(vec![mt(sous_titre)], 7.0, Some(GRIS), 0.0, 5.0));

    // Le cadre d'identite : un tableau d'une ligne, deux cellules a filet.
    let nir_affiche = match sal.get("nir") {
        Some(n) if vrai(n) => format!("{}{}", en_texte(n), texte(sal, "nir_cle")?),
        _ => "en cours d'immatriculation".to_string(),
    };
    let mut salarie = vec![
        m(format!("{} {}", texte(sal, "nom")?, texte(sal, "prenom")?), true, None),
        mt(format!("\n{}\n{} {}\n\n", texte(sal, "adresse")?, texte(sal, "code_postal")?, texte(sal, "commune")?)),
        k("N° Sécurité sociale :"),
        mt(format!(" {}\n", nir_affiche)),
        k("Date de naissance :"),
        mt(format!(" {}\n", jj_mm_aaaa(requis(sal, "date_naissance")?)?)),
        k("Emploi :"),
        mt(format!(" {} {} ", texte(sal, "emploi")?, NBSP)),
        k("Statut :"),
        mt(format!(" {}\n", texte(sal, "statut")?)),
    ];
    salarie.extend(ligne_caisse);
    salarie.push(k("Abattement :"));
    salarie.push(mt(" 0 %"));
    let employeur = vec![
        m("EMPLOYEUR", true, None),
        mt("\n\n"),
        k("N° Siret :"),
        mt(format!(" {}\n", siret)),
        k("NAF (APE) :"),
        mt(format!(" {}\n", naf)),
        k("Urssaf :"),
        mt(format!(
            " {}\n",
            ent.get("urssaf_compte").filter(|v| vrai(v)).map(en_texte).ok_or(
                "La fiche entreprise ne dit pas le numéro du compte Urssaf (`urssaf_compte`) : il figure sur chaque bulletin \
                 (art. R. 3243-1 du code du travail, le numéro sous lequel les cotisations sont versées). Il se lit sur \
                 l'avis d'immatriculation de l'Urssaf."
            )?
        )),
        k("Conv. collective :"),
        mt(format!(" IDCC {} - {}\n", conv.0, conv.1)),
        k("Licence spec. :"),
        mt(format!(" {}", licence)),
    ];
    blocs.push(json!({
        "type": "tableau", "taille": 7, "interligne": 1.27,
        "colonnes": [{"largeur": 0.5}, {"largeur": 0.5}],
        "marge_cellule": marge_cellule(),
        "filets_cellules": {"couleur": "#dddddd", "epaisseur": 0.5},
        "lignes": [{"cellules": [{"morceaux": salarie}, {"morceaux": employeur}]}]
    }));

    let contrat_long = if permanent {
        let fin = match e.fin {
            Some(f) => format!(", jusqu'au {}", f.jj_mm_aaaa()),
            None => String::new(),
        };
        format!(" ({} depuis le {}{})", nat.libelle, e.debut.jj_mm_aaaa(), fin)
    } else if e.a_cheval()? {
        let fin = match e.fin {
            Some(f) => f,
            None => Jour::lire("None")?,
        };
        format!(" (contrat du {} au {})", e.debut.jj_mm_aaaa(), fin.jj_mm_aaaa())
    } else {
        String::new()
    };
    let mut periode = vec![
        m("Période :", true, None),
        mt(format!(
            " du {} au {}{} {} ",
            e.debut_periode()?.jj_mm_aaaa(),
            e.fin_periode()?.jj_mm_aaaa(),
            contrat_long,
            tiret
        )),
    ];
    periode.extend(travail);
    blocs.push(para(periode, 7.0, Some(GRIS), 5.0, 5.0));

    // Le tableau : le modele de l'arrete du 25/02/2016, art. 1, I (redaction du 25/06/2024, seul modele depuis le
    // 01/01/2026 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml),
    // ses colonnes (Base, Taux, Salarié, Employeur), ses libelles, son ordre et ses regroupements : voir `tableau_modele`.
    let th = || {
        vec![("couleur", Value::from(BLANC)), gras(), ("taille", num(6.6)), ("fond", Value::from(ROUGE))]
    };
    let entete =
        vec![ligne(vec![cell("LIBELLE", 1, true, None), s("Base"), s("Taux"), s("Salarié"), s("Employeur")], th())];
    let mut corps = Vec::new();
    let rem: Vec<&engagement::calcul::LigneBrut> =
        p.remuneration.iter().enumerate().filter(|(i, _)| !j.remboursements.contains(i)).map(|(_, l)| l).collect();
    // « MONTANT BRUT » (art. 1, I ; VI : mis en valeur). Jusqu'a la vague V2g : « SALAIRE BRUT ».
    if rem.len() <= 1 {
        let titre = match rem.first() {
            Some(l) => format!("{} à {}", l.designation(), fmt_eur(l.taux)),
            None => "SALAIRE".to_string(),
        };
        corps.push(ligne(
            vec![s(format!("{} — MONTANT BRUT", titre)), s(fmt_eur(p.brut))],
            vec![gras(), ("fond", FOND_BRUT.into())],
        ));
    } else {
        for l in &rem {
            corps.push(ligne(
                vec![s(format!("{} à {}", l.designation(), fmt_eur(l.taux))), s(fmt_eur(l.montant))],
                vec![],
            ));
        }
        corps.push(ligne(vec![s("MONTANT BRUT"), s(fmt_eur(p.brut))], vec![gras(), ("fond", FOND_BRUT.into())]));
    }
    corps.extend(tableau_modele(p)?);
    // Le bas du tableau (vague 1o) : « MONTANT NET SOCIAL », puis « REMBOURSEMENTS ET DEDUCTIONS DIVERSES », puis
    // « MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU », puis « IMPOT SUR LE REVENU » (« Montant net imposable »,
    // « IMPOT SUR LE REVENU PRELEVE A LA SOURCE ») et « MONTANT NET A PAYER (en Euros) », enfin « TOTAL VERSE PAR
    // L'EMPLOYEUR » (vague V2g). Jusqu'a la vague 1o, deux nets egaux partageaient une ligne « NET SOCIAL / NET A PAYER
    // AVANT IMPOT » (la derogation de l'arrete du 31/01/2023, finie au 01/01/2026).
    corps.push(ligne_net("MONTANT NET SOCIAL", p.net_social, n));
    if !j.remboursements.is_empty() {
        corps.push(ligne(vec![cel("REMBOURSEMENTS ET DÉDUCTIONS DIVERSES", n)], style_section()));
        for i in &j.remboursements {
            let l = &p.remuneration[*i];
            corps.push(ligne(vec![s(format!("{} à {}", l.designation(), fmt_eur(l.taux))), s(fmt_eur(l.montant))], vec![]));
        }
    }
    corps.push(ligne_net("MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU", p.net_a_payer, n));
    corps.push(ligne(vec![s("IMPOT SUR LE REVENU"), s("Base"), s("Taux"), s("Salarié")], style_section()));
    corps.push(ligne(vec![cel("Montant net imposable", 3), s(fmt_eur(p.net_imposable))], vec![]));
    match j.rectificatif.filter(|r| !remplace_en_entier(r)) {
        Some(r) => corps.extend(rectificatif_lignes(r, n)?),
        None => corps.extend(lignes_pas_tableau(p, j.pas, n)?),
    }
    // « TOTAL VERSÉ PAR L'EMPLOYEUR » (art. 1, I ; C. trav. R. 3243-1, 14° : le brut et les cotisations patronales,
    // exonerations deduites) : le cout employeur de la paie, imprime jusqu'a la vague V2g au pied du bulletin
    // (« Coût total employeur »).
    corps.push(ligne(
        vec![cel("TOTAL VERSÉ PAR L'EMPLOYEUR", n - 1), s(fmt_eur(p.cout_employeur))],
        vec![gras(), filet_haut(ROUGE, 0.6)],
    ));
    // Les montants ne se coupent pas : voir les largeurs de colonnes du Python.
    blocs.push(tableau(&LARGEURS, entete, corps, "#e4e4e4"));

    let cumul = if permanent {
        j.conges.map(tableau_conges)
    } else {
        j.colonnes.filter(|c| !c.is_empty()).map(tableau_des_cumuls)
    };
    blocs.extend(cumul);
    let plafond = j.plafond.map_or("—".to_string(), fmt_eur);
    let date_paiement = match &e.date_paiement {
        Some(dp) if !dp.is_empty() => d(&Value::String(dp.clone()))?,
        _ => e.fin_periode()?,
    };
    blocs.push(para(
        vec![mt(format!(
            "Plafond Urssaf de la période : {} €{}SMIC retenu pour la réduction générale : {}{}Règlement le {} \
             par virement.\nContrat n° {} — {}, convention collective IDCC {}. Conservez ce bulletin sans limitation \
             de durée.",
            plafond,
            barre,
            smic_de_la_reduction(p)?,
            barre,
            date_paiement.jj_mm_aaaa(),
            numero,
            nat.libelle,
            conv.0
        ))],
        6.2,
        Some(GRIS_CLAIR),
        5.0,
        0.0,
    ));
    Ok(blocs)
}

// ------------------------------------------------------------ la description

/// `contexte.gabarits_de` : le dossier `gabarits/` de LA COMPAGNIE dont `ent` est la fiche (`_gabarits`,
/// `Contexte::entreprise_et_gabarits`). Sans `_gabarits`, aucun : la compagnie ne se devine pas d'une variable du
/// processus (la fenetre en sert plusieurs, ticket #244) ; un document sans dossier n'a ni logo ni signature posee.
pub fn gabarits_de(ent: &Value) -> Option<PathBuf> {
    match ent.get("_gabarits") {
        Some(g) if vrai(g) => Some(PathBuf::from(en_texte(g))),
        _ => None,
    }
}

/// Un chemin ecrit comme `str(Path(...))` : sans `.` ni separateur double.
fn chemin_texte(p: &Path) -> String {
    p.components().collect::<PathBuf>().to_string_lossy().into_owned()
}

/// `identite_de` : le fichier d'identite (`logo`, `signature`) de la compagnie, ou `None`.
pub fn identite_de(ent: &Value, nom: &str) -> Option<PathBuf> {
    let dossier = gabarits_de(ent)?.join("identite");
    IDENTITE_EXTENSIONS.iter().map(|ext| dossier.join(format!("{}{}", nom, ext))).find(|f| f.exists())
}

/// `description` : un document par jeu, chacun sur sa page ; au modele complet (la case du salarie, sinon celle de la
/// compagnie : `modele_du_bulletin`), l'annexe suit le bulletin, document a part (une page neuve).
pub fn description(jeux: &[Jeu], ent: &Value) -> Resultat<Value> {
    let mut d = Map::new();
    d.insert("format".into(), json!(FORMAT));
    d.insert(
        "page".into(),
        json!({"largeur": 595.28, "hauteur": 841.89, "marges": {"haut": 42, "droite": 42, "bas": 42, "gauche": 42}}),
    );
    d.insert("police".into(), json!({"taille": 7.4, "interligne": 1.2, "couleur": ARDOISE}));
    if let Some(logo) = identite_de(ent, "logo") {
        d.insert(
            "logo".into(),
            json!({"chemin": chemin_texte(&logo), "largeur": LOGO_LARGEUR,
                   "marge_haut": LOGO_MARGE_HAUT, "marge_droite": LOGO_MARGE_DROITE}),
        );
    }
    let mut documents = Vec::new();
    for j in jeux {
        let modele = modele_du_bulletin(ent, &j.paie.salarie)?;
        documents.push(json!({"blocs": blocs_un(j, ent)?}));
        if modele == Modele::Complet {
            documents.push(json!({"blocs": blocs_annexe(j.paie)?}));
        }
    }
    d.insert("documents".into(), Value::Array(documents));
    Ok(Value::Object(d))
}

/// Les lignes de remuneration d'une paie qui s'impriment sous « REMBOURSEMENTS ET DEDUCTIONS DIVERSES » : hors assiette
/// et pas un revenu (la nature de la rubrique, `Rubrique::hors_assiette`, celle que le montant net social lit).
pub fn remboursements(ctx: &Contexte, p: &Paie) -> Resultat<Vec<usize>> {
    let mut out = Vec::new();
    for (i, l) in p.remuneration.iter().enumerate() {
        if !l.soumis && droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.hors_assiette()? != droit::catalogue::HorsAssiette::Revenu {
            out.push(i);
        }
    }
    Ok(out)
}

/// Ce que `bulletin.ecrire` prepare avant d'appeler la fabrique : la licence
/// resolue, le prelevement a la source, les cumuls et le compteur de conges, le
/// plafond ; les mois sans paie sont ecartes. `paies_du_mois` : TOUTES les
/// paies du mois quand on n'en imprime qu'une ; `borne_au_mois` : les cumuls
/// ne lisent que les contrats commences avant la fin du mois ; `rectificatifs` :
/// indice dans `paies` -> ce que `_bloc_rectificatif` imprime.
pub fn jeux_et_description(
    ctx: &Contexte,
    paies: &[Paie],
    ent: &Value,
    mois: &str,
    paies_du_mois: Option<&[Paie]>,
    borne_au_mois: bool,
    rectificatifs: &HashMap<usize, Value>,
) -> Resultat<Value> {
    // La licence est DATEE : l'appelant la resout au mois du bulletin.
    if let Some(l) = ent.get("licence_spectacle") {
        if !l.is_null() && !l.is_string() {
            return Err(format!(
                "licence_spectacle non resolue : elle vaut {}. Le numero change de date en date ; le resoudre au \
                 mois du bulletin avec Contexte.licence(), comme le fait la fabrique des documents (`depart`).",
                forme_ecrite(l)
            ));
        }
    }
    let pas_par_paie = crate::prelevement::des_paies(ctx, paies)?;
    let mut colonnes_par_salarie: HashMap<String, Vec<Colonne>> = HashMap::new();
    let mut conges_par_paie: HashMap<usize, CompteurConges> = HashMap::new();
    if !mois.is_empty() {
        // Tout ce que la compagnie a declare, ce mois-ci compris : les paies
        // qu'on imprime remplacent le fichier de leur mois.
        let courantes = paies_du_mois.filter(|p| !p.is_empty()).unwrap_or(paies);
        let mut engagements = lire_engagements(ctx, Some(courantes))?;
        if borne_au_mois {
            let fin = dernier_du_mois(mois)?;
            engagements.retain(|e| e.debut <= fin);
        }
        for (i, pa) in paies.iter().enumerate() {
            let cle = texte(&pa.salarie, "_cle")?;
            // Le compteur de conges est par contrat, le cumul des 507 h par salarie. Un contrat paye au mois et
            // declare a la caisse des Conges Spectacles (`calcul::declaree_a_la_caisse_des_conges`, D. 7121-41) n'a pas de compteur
            // de l'employeur : c'est la caisse qui lui paie ses conges (#132).
            if pa.engagement.nat()?.mensuelle() {
                if !engagement::calcul::declaree_a_la_caisse_des_conges(pa)? {
                    conges_par_paie.insert(i, compteur_au_bulletin(ctx, pa, mois)?);
                }
            } else if !colonnes_par_salarie.contains_key(&cle) {
                colonnes_par_salarie.insert(cle.clone(), tableau_cumul(ctx, &cle, mois, &engagements)?);
            }
        }
    }
    let mut jeux = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        if p.engagement.sans_paie() {
            continue;
        }
        let cle = texte(&p.salarie, "_cle")?;
        jeux.push(Jeu {
            paie: p,
            colonnes: colonnes_par_salarie.get(&cle).map(Vec::as_slice),
            plafond: Some(p.plafond_urssaf(ctx)?),
            conges: conges_par_paie.get(&i),
            pas: pas_par_paie[i].as_ref(),
            rectificatif: rectificatifs.get(&i),
            remboursements: remboursements(ctx, p)?,
        });
    }
    if jeux.is_empty() {
        return Err("Aucun bulletin a ecrire : toutes les paies sont des mois sans paie.".to_string());
    }
    description(&jeux, ent)
}
