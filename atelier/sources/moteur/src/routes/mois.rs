//! Les routes du mois a l'ecran : `saisie`, `apercu`, `enregistrer-saisie`, `ajuster`, et ce que la saisie
//! consulte (`contrats-en-cours`, `contrats-passes`, `emploi`).
//!
//! `saisie` : le mois tel que l'ecran l'ouvre -- la saisie que ses engagements montrent (`engagement::stock::vue_du_mois`)
//! ou, pour un mois neuf, un squelette vide au dernier taux saisi --, avec les noms resolus en cles de fiche et les lignes
//! de paie telles que le moteur les comprend. Aucun calcul pour l'ecran : la saisie se charge jusqu'au contrat qui
//! deborde, sans chiffrer. Les engagements valides que l'ecran montre se lisent par `engagement::etats::du_mois`, qui les
//! recalcule s'ils ne sont plus justes.
//!
//! `apercu` : le mois pour l'ecran, rien d'ecrit. Un engagement valide montre la paie qu'il a ecrite ; un engagement en
//! saisie se calcule en memoire (`paies_de_l_ecran`) ; les numeros se reservent sans rien ecrire.
//!
//! Chaque charge est une structure (`ChargeCarte`, `ChargeApercu`, `ChargeAjuster`, `ChargeContratsPasses`,
//! `ChargeEmploi`, et `etapes::ChargeMois`), lue une fois (`routes::lire_charge`) ; chaque reponse en est une aussi.
//!
//! TRAVAIL A FAIRE (chaine A, le domaine de l'engagement) : la saisie d'un engagement (`Carte::saisie`, la vue du mois,
//! les lignes, le spectacle d'une paie) se lit encore comme son fichier l'ecrit, en `serde_json::Value`, par ses cles ;
//! de meme l'engagement importe (`engagement::importe` : son contrat, ses paies), les cartes importees
//! (`import::mois::cartes`), le conseil de saisie (`compagnie::conseils`), la ligne « Pour démarrer »
//! (`declarations::demarrage::conges_a_dire`) et la fiche d'un spectacle (`Contexte::spectacles`). Ils se declareront avec
//! leur domaine ; ici, ils passent tels quels.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::routes::fiches::{maintenant, refus};
use crate::routes::rappels::texte;
use crate::routes::{en_json, lire_charge};
use crate::routes::etapes::ChargeMois;
use engagement::calcul::Paie;
use engagement::stock::Identite;
use engagement::viser::Cible;
use droit::catalogue;
use droit::contexte::{archivee, Contexte, IdSalarie};
use droit::emplois;
use engagement::{self, Engagement};
use socle::date::{en_texte, Jour};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{repr, vrai};
use socle::mois::Mois;
use socle::num::Decimal;
use socle::valeur::{nom_type, valeur_en_texte};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::engagement::SAISIE_INCOMPLETE;
use crate::routes::catalogue::{Ecran, Place};

const CARTES: Champ = Champ {
    nom: "cartes", libelle: Affiche::new("Cartes"), forme: Forme::Objet,
    sens: "les cartes non écrites de l'écran : [{engagement (son identité, ou null pour une neuve), saisie}]",
    avance: false,
};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Saisie,
        geste: "Lire les engagements d'un mois, leurs identités et le cachet par défaut.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "saisie (les engagements du mois), identites, importes, ferme_aux_neufs, defauts, phrase (ce que l'écran dit de la saisie)",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Apercu,
        geste: "Calculer sans rien écrire les paies du mois, avec les cartes en cours.",
        charge: &[MOIS, CARTES],
        exemple: r#"{"mois":"2026-11","cartes":[]}"#,
        rend: "paies (une par engagement, ses bornes debut et fin, ou ce qui lui manque : a_completer ; contrat.ecrits : les cases que son contrat écrit appelle, à remplir avant de le faire signer ; a_dire : la case de fiche qu'il appelle, comme « Pour démarrer » ; faits_avances : les cases « avancé » de la carte d'un contrat au mois qu'il appelle, `cases` de valider-engagement ; cases : celles qu'une ligne de frais appelle, avec leur aide), les trois nombres de chaque carte, le mois",
        refus: &[SAISIE_INCOMPLETE, MANQUE, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsEnCours,
        geste: "Lire les contrats d'un mois passé qui courent encore ce mois.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats : chacun avec l'engagement qui le reprend",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsPasses,
        geste: "Lire les contrats déclarés d'un mois importé.",
        charge: &[MOIS, Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé d'un salarié, pour ne lire que les siens", avance: true }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats : ceux que les engagements importés attestent",
        refus: &[MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Emploi,
        geste: "Traduire un métier du catalogue en champs de la fiche salarié.",
        charge: &[
            Champ { nom: "metier", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier au catalogue (COM010…)", avance: false },
            Champ { nom: "cadre", libelle: Affiche::new("Cadre"), forme: Forme::OuiNon, sens: "cadre ou non ; absent : le défaut du métier au catalogue", avance: true },
            Champ { nom: "feminin", libelle: Affiche::new("Au féminin"), forme: Forme::OuiNon, sens: "le libellé au féminin", avance: true },
            Champ { nom: "salarie", libelle: Affiche::new("Fiche"), forme: Forme::Objet, sens: "ce que la fiche remplie dit du sexe : {nir, sexe}", avance: true },
        ],
        exemple: r#"{"metier":"COM010"}"#,
        rend: "les champs de la fiche que le métier remplit (emploi, statut, catégorie…)",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::EnregistrerSaisie,
        geste: "Écrire la carte d'un engagement en saisie, et elle seule.",
        charge: &[
            MOIS,
            Champ { nom: "engagement", libelle: Affiche::new("Carte"), forme: Forme::Objet, sens: "l'identité de la carte ; absente pour une carte neuve", avance: false },
            super::engagement::SAISIE,
        ],
        exemple: r#"{"mois":"2026-11","saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":150,"dates":["2026-11-20"]}]}}"#,
        rend: "engagement (l'identité écrite), a_completer (ce qui lui manque, ou null), enregistre_a",
        refus: &[OPERATION, FORME, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Ajuster,
        geste: "Trouver le taux qui donne un net, un brut ou un coût voulu.",
        charge: &[
            MOIS,
            CARTES,
            Champ { nom: "carte", libelle: Affiche::new("Rang de la carte"), forme: Forme::Nombre, sens: "le rang, dans cartes, de la carte à ajuster", avance: false },
            Champ { nom: "cible", libelle: Affiche::new("Net, brut ou coût"), forme: Forme::Texte, sens: "net, brut ou cout", avance: false },
            Champ { nom: "montant", libelle: Affiche::new("Montant voulu"), forme: Forme::Nombre, sens: "le montant voulu, en euros", avance: false },
        ],
        exemple: r#"{"mois":"2026-11","cartes":[{"engagement":null,"saisie":{"salarie":"FULLER","spectacle":"le-banc-des-menteurs","lignes":[{"rubrique":"cachet","quantite":1,"taux":150,"dates":["2026-11-21"]}]}}],"carte":0,"cible":"brut","montant":200}"#,
        rend: "phrase (ce qui est atteint, et le taux qu'il faudrait), vise, atteint, ecart, engagements (les taux qui changent), paies ; rien n'est écrit",
        refus: &[OPERATION, FORME, SAISIE_INCOMPLETE],
        acte: Acte::Lit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les charges

/// UNE CARTE QUE L'ECRAN MONTRE AUTREMENT QU'ELLE N'EST ECRITE (`apercu`, `ajuster`) : son identite (`None` : pas encore
/// ecrite) et ce qu'elle porte.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Carte {
    #[serde(default)]
    pub(crate) engagement: Option<Identite>,
    /// TRAVAIL A FAIRE (chaine A) : la saisie d'un engagement, telle que son fichier l'ecrit.
    pub(crate) saisie: Map<String, Value>,
}

/// LA CHARGE D'UNE CARTE QUI S'ECRIT (`enregistrer-saisie`, `valider-engagement`) : le mois de l'ecran, l'identite de la
/// carte (absente : un engagement neuf) et ce qu'elle ecrit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeCarte {
    pub(crate) mois: Mois,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) engagement: Option<Identite>,
    /// TRAVAIL A FAIRE (chaine A) : la saisie d'un engagement, telle que son fichier l'ecrit.
    pub(crate) saisie: Map<String, Value>,
}

/// La charge de l'apercu : le mois, et les cartes que l'ecran montre autrement (`mois_designe`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeApercu {
    mois: Mois,
    #[serde(default)]
    cartes: Vec<Carte>,
}

/// La charge du montant vise : le mois et ses cartes, le rang de la carte a ajuster, ce qui se vise et combien.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAjuster {
    mois: Mois,
    #[serde(default)]
    cartes: Vec<Carte>,
    carte: usize,
    cible: Cible,
    montant: f64,
}

/// La charge des contrats d'un mois passe : le mois, et le salarie dont ne lire que les siens (absent : tous).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeContratsPasses {
    mois: Mois,
    #[serde(default)]
    salarie: Option<IdSalarie>,
}

/// La charge du metier : son code, et ce qui choisit sa forme (cadre, ou non ; au feminin, dit tel quel ou lu de la
/// fiche que l'ecran remplit).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEmploi {
    metier: String,
    #[serde(default)]
    cadre: Option<bool>,
    #[serde(default)]
    feminin: bool,
    #[serde(default)]
    salarie: Option<SexeDeLaFiche>,
}

/// Ce qui dit le sexe, sur la fiche que l'ecran remplit : le NIR, sinon la case `sexe` (`ntt::sexe_de`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SexeDeLaFiche {
    #[serde(default)]
    nir: String,
    #[serde(default)]
    sexe: String,
}

// ------------------------------------------------------------------ les reponses

/// Ce que la page propose quand la saisie ne le dit pas : le taux d'une ligne de cachet neuve.
#[derive(Debug, Serialize)]
struct Defauts {
    taux_cachet: Option<f64>,
}

/// LE MOIS TEL QUE L'ECRAN L'OUVRE (`saisie`).
#[derive(Debug, Serialize)]
struct SaisieDuMois {
    mois: Mois,
    phrase: Option<&'static str>,
    /// TRAVAIL A FAIRE : les cartes importees (`import::mois::cartes`).
    importes: Vec<Value>,
    importes_titre: &'static str,
    importes_dit: String,
    ferme_aux_neufs: Option<String>,
    defauts: Defauts,
    /// TRAVAIL A FAIRE (chaine A) : la vue du mois (`engagement::stock::vue_du_mois`), les noms resolus.
    saisie: Value,
    /// L'identite de chaque engagement de `saisie.engagements`, dans son ordre : ce par quoi sa carte le designe.
    identites: Vec<Identite>,
}

/// Une carte ecrite (`enregistrer-saisie`).
#[derive(Debug, Serialize)]
struct CarteEcrite {
    phrase: String,
    engagement: Identite,
    a_completer: Option<String>,
    enregistre_a: String,
    enregistre_dit: String,
}

/// UN CONTRAT D'USAGE REPRIS D'UN AUTRE LOGICIEL QUI COURT SUR LE MOIS, et l'engagement qui le reprend.
#[derive(Debug, Serialize)]
struct EnCours {
    salarie: IdSalarie,
    nom: String,
    spectacle: Option<String>,
    debut: String,
    fin: String,
    numero: String,
    depuis: String,
    metier: Option<String>,
    cadre: Option<bool>,
    taux_cachet: Option<f64>,
    lieu: &'static str,
    deja_repris: bool,
    engagement: Reprise,
}

/// L'ENGAGEMENT D'UN CONTRAT EN COURS, pour le fichier du mois : le contrat tel qu'il a ete signe, et une ligne de
/// cachets a dater.
#[derive(Debug, Serialize)]
struct Reprise {
    salarie: IdSalarie,
    #[serde(skip_serializing_if = "Option::is_none")]
    spectacle: Option<String>,
    debut: String,
    fin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    numero: Option<String>,
    lignes: [LigneAReprendre; 1],
}

/// La ligne de cachets d'un contrat repris : aucun cachet encore, au taux du contrat ou au dernier saisi.
#[derive(Debug, Serialize)]
struct LigneAReprendre {
    rubrique: &'static str,
    quantite: u8,
    taux: Option<f64>,
    dates: [Jour; 0],
}

#[derive(Debug, Serialize)]
struct ContratsEnCours {
    phrase: String,
    mois: Mois,
    contrats: Vec<EnCours>,
    reprise: Option<String>,
}

/// UN CONTRAT QUI TOUCHE UN MOIS PASSE, tel que le calendrier le montre. LECTURE SEULE, ET UN PLANCHER : c'est ce que la
/// compagnie a declare, pas la position du salarie.
#[derive(Debug, Serialize)]
struct Passe {
    dit: String,
    salarie: String,
    nom: String,
    debut: String,
    fin: String,
    cachets: i64,
    heures: i64,
    jours: i64,
    titre: String,
    numero_objet: String,
    dates: Vec<String>,
    source: &'static str,
}

#[derive(Debug, Serialize)]
struct ContratsPasses {
    phrase: String,
    mois: Mois,
    contrats: Vec<Passe>,
    rien_dit: &'static str,
}

#[derive(Debug, Serialize)]
struct Emploi {
    phrase: String,
    #[serde(flatten)]
    champs: emplois::ChampsDuMetier,
}

/// Une ligne de la carte dont le taux change, pour le montant vise. TRAVAIL A FAIRE (chaine A) : les taux et la rubrique
/// se lisent de la saisie, telle que son fichier l'ecrit.
#[derive(Debug, Serialize)]
struct TauxQuiChange {
    rang: usize,
    rubrique: Value,
    taux_avant: Value,
    taux: Value,
}

/// Ce qui a change, pour l'engagement ajuste : son salaire du mois s'il change, ses lignes dont le taux change.
#[derive(Debug, Serialize)]
struct CeQuiChange {
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    salaire_mensuel: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lignes: Vec<TauxQuiChange>,
}

/// LE MONTANT VISE (`ajuster`) : ce qui est atteint, le taux qu'il faudrait ; rien n'est ecrit.
#[derive(Debug, Serialize)]
struct Ajuste {
    phrase: String,
    cible: Cible,
    vise: Decimal,
    atteint: Decimal,
    depart: Decimal,
    ecart: Decimal,
    facteur: Decimal,
    tours: usize,
    engagements: [CeQuiChange; 1],
    paies: [VuePaie; 1],
}

// ------------------------------------------------------------------ les routes

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::Saisie => en_json(saisie(ctx, lire_charge::<ChargeMois>(charge)?.mois)?),
        Route::Apercu => en_json(apercu(ctx, lire_charge(charge)?)?),
        Route::ContratsEnCours => en_json(contrats_en_cours_route(ctx, lire_charge::<ChargeMois>(charge)?.mois)?),
        Route::ContratsPasses => en_json(contrats_passes_route(ctx, lire_charge(charge)?)?),
        Route::Emploi => en_json(emploi(lire_charge(charge)?)?),
        Route::EnregistrerSaisie => en_json(enregistrer_saisie(ctx, lire_charge(charge)?)?),
        Route::Ajuster => en_json(ajuster(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

/// Le fichier d'une saisie, lu en JSON ; un fichier illisible refuse en le nommant. TRAVAIL A FAIRE : ses lecteurs
/// (`rappels` : l'echeancier livre et celui de la compagnie ; `atmp` ; `fiches`) lisent chacun un fichier de leur domaine,
/// qui se declarera avec lui.
pub(crate) fn lire_saisie(chemin: &Path) -> R<Value> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", socle::echec::du_disque(&chemin, &e)))?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", socle::echec::du_disque(&chemin, &e)))?;
    serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
}

fn nombre_f(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

// ============================================================ la saisie

/// Un mois neuf s'ouvre VIDE, avec le
/// dernier taux saisi -- `null` quand le dossier n'a encore aucun cachet (R12 :
/// aucun texte ne fixe un taux de cachet, la compagnie le saisit).
pub(crate) fn squelette(ctx: &Contexte, mois: &str) -> R<Value> {
    socle::mois::Mois::lire(mois).map(socle::mois::Mois::dernier_jour).map_err(|m| erreur("ValueError", m))?;
    let taux = dernier_taux(ctx)?.map_or(Value::Null, nombre_f);
    Ok(json!({ "mois": mois, "taux_cachet": taux, "engagements": [] }))
}

/// Le flottant d'une valeur de saisie, quand elle est vraie.
fn flottant_de(v: &Value) -> R<f64> {
    match v {
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| erreur("ValueError", format!("nombre illisible : {}", socle::fichiers::repr(s)))),
        autre => Err(erreur("TypeError", format!("float() argument must be a string or a real number, not '{}'", nom_type(autre)))),
    }
}

/// Les elements de `valeur` (vide ou absente : aucun) : une liste, ou
/// les caracteres d'une chaine, les cles d'un dictionnaire (des textes sans
/// `.get`) ; autre chose refuse.
fn elements(v: Option<&Value>) -> R<Vec<Value>> {
    match v {
        None => Ok(Vec::new()),
        Some(x) if !vrai(x) => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(Value::String(s)) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Some(Value::Object(o)) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        Some(autre) => Err(erreur("TypeError", format!("'{}' object is not reversible", nom_type(autre)))),
    }
}

#[track_caller]
fn pas_de_get(v: &Value) -> Echec {
    erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v)))
}

/// Le taux du cachet le plus recemment saisi : une ligne de cachet d'abord, le `taux_cachet` du mois a defaut ; `None` si
/// aucun mois n'en dit rien -- aucun texte ne fixe un taux de cachet (regle 1, R12), la compagnie saisit le sien.
pub(crate) fn dernier_taux(ctx: &Contexte) -> R<Option<f64>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
    mois.reverse();
    for m in mois {
        let Ok(Some(data)) = engagement::stock::vue_du_mois(&ctx.racine, &m) else { continue };
        let Some(objet) = data.as_object() else { return Err(pas_de_get(&data)) };
        for e in elements(objet.get("engagements"))?.iter().rev() {
            let Some(eng) = e.as_object() else { return Err(pas_de_get(e)) };
            for l in elements(eng.get("lignes"))?.iter().rev() {
                if let Some(t) = taux_d_une_ligne(ctx, l) {
                    return Ok(Some(t));
                }
            }
        }
        if vrai_opt(objet.get("taux_cachet")) {
            return flottant_de(&objet["taux_cachet"]).map(Some);
        }
    }
    Ok(None)
}

/// Le taux d'une ligne de cachet (le `try` de `dernier_taux`) : tout ce qui
/// leve est une ligne qui ne dit rien.
fn taux_d_une_ligne(ctx: &Contexte, l: &Value) -> Option<f64> {
    let code = l.get("rubrique")?.as_str()?;
    if droit::catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).ok()?.unite != "cachet" {
        return None;
    }
    let taux = l.get("taux").filter(|t| vrai(t))?;
    flottant_de(taux).ok()
}


/// Le décimal en flottant : la valeur la plus proche, par sa représentation.
fn decimal_en_f64(d: socle::num::Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

/// « Celi » et « aaah zerbinette » deviennent les cles
/// du fichier. Une saisie qui ne se charge pas se rend telle quelle.
fn resoudre_noms(ctx: &Contexte, saisie: &Value) -> R<Value> {
    let Some(objet) = saisie.as_object() else { return Ok(saisie.clone()) };
    let Ok(paies) = engagement::lue_sans_paie(ctx, saisie) else { return Ok(saisie.clone()) };
    let mut sortie = objet.clone();
    let tous = elements(objet.get("engagements"))?;
    let mut engagements = Vec::new();
    for (e, p) in tous.iter().zip(&paies) {
        let Some(brut) = e.as_object() else { return Ok(saisie.clone()) };
        let mut resolu = brut.clone();
        resolu.insert("salarie".into(), Value::from(p.salarie.id.clone()));
        // UN PERMANENT N'A PAS DE SPECTACLE : le numero d'objet lui est interdit.
        if vrai(&p.spectacle) {
            resolu.insert("spectacle".into(), p.spectacle.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        }
        // Les lignes d'un permanent sont CALCULEES, pas saisies.
        if !p.engagement.nat()?.mensuelle() {
            let brutes = elements(brut.get("lignes"))?;
            resolu.insert("lignes".into(), Value::Array(lignes_affichees(ctx, p, &brutes)?));
        }
        engagements.push(Value::Object(resolu));
    }
    sortie.insert("engagements".into(), Value::Array(engagements));
    Ok(Value::Object(sortie))
}

/// Ce qu'une ligne du fichier dit et que le moteur ne recalcule pas : la page
/// les relit pour les remettre dans ses champs, et les reecrit ensuite.
/// `base` et `taux_majoration` d'une ligne en pourcentage, surtout : sans eux,
/// la page lirait le `taux` (0.1) comme un pourcentage (0,1 %).
const CHAMPS_DE_LIGNE_GARDES: [&str; 9] = ["heures", "genre", "base", "taux_majoration", engagement::JUSTIFICATIF, engagement::PARIS, engagement::RESTAURANT, engagement::TERRITOIRE, engagement::LOGE];

/// Ce que dit, a l'ecran, la case d'une ligne de frais (`engagement::CASES`).
fn libelle_de_case(case: &str) -> &'static str {
    match case {
        engagement::PARIS => "Paris ou petite couronne",
        engagement::RESTAURANT => "au restaurant",
        engagement::LOGE => "hôtel payé par la compagnie",
        _ => "justificatif conservé",
    }
}

/// Les lignes de paie telles que le MOTEUR les a comprises.
/// `remuneration` suit les lignes du fichier une a une (`calcul::lignes_brut`) ; la part d'un frais au-dela de sa
/// limite, que le moteur pose apres elles (`engagement::frais_au_dela`), ne se montre pas en ligne de saisie, et la
/// ligne garde le taux saisi (`engagement::TAUX_SAISI`).
fn lignes_affichees(ctx: &Contexte, p: &engagement::calcul::Paie, brutes: &[Value]) -> R<Vec<Value>> {
    let lignes: Vec<Map<String, Value>> = p
        .remuneration
        .iter()
        .take(brutes.len())
        .enumerate()
        .map(|(i, l)| {
            let mut m = Map::new();
            m.insert("rubrique".into(), Value::String(l.code.clone()));
            m.insert("quantite".into(), nombre_f(decimal_en_f64(l.quantite)));
            let saisi = p.engagement.lignes.get(i).and_then(|x| x.get(engagement::TAUX_SAISI)).cloned();
            m.insert("taux".into(), saisi.unwrap_or_else(|| nombre_f(decimal_en_f64(l.taux))));
            if let Some(src) = brutes.get(i).and_then(Value::as_object) {
                for k in CHAMPS_DE_LIGNE_GARDES {
                    match src.get(k) {
                        None | Some(Value::Null) => {}
                        Some(v) => {
                            m.insert(k.into(), v.clone());
                        }
                    }
                }
            }
            m
        })
        .collect();
    Ok(jours_sur_les_lignes(ctx, &p.engagement, lignes, brutes)?.into_iter().map(Value::Object).collect())
}

/// Une valeur de saisie en liste.
fn liste_lue(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// Pose sur chaque ligne les jours qu'elle couvre, et ou : une saisie qui porte les jours sur
/// l'engagement les redescend sur ses lignes.
fn jours_sur_les_lignes(
    ctx: &Contexte,
    eng: &Engagement,
    mut lignes: Vec<Map<String, Value>>,
    brutes: &[Value],
) -> R<Vec<Map<String, Value>>> {
    // Ce que le fichier disait deja ligne par ligne : on le reprend tel quel.
    let mut porte = false;
    for (i, l) in lignes.iter_mut().enumerate() {
        let vide = Value::Object(Map::new());
        let src = brutes.get(i).unwrap_or(&vide);
        let Some(src) = src.as_object() else { return Err(pas_de_get(src)) };
        if vrai_opt(src.get("dates")) {
            l.insert("dates".into(), Value::Array(liste_lue(&src["dates"])?));
            porte = true;
        }
        if vrai_opt(src.get("planning")) {
            l.insert("planning".into(), Value::Array(liste_lue(&src["planning"])?));
        }
        if vrai_opt(src.get("lieu")) {
            l.insert("lieu".into(), src["lieu"].clone());
        }
    }
    if porte || lignes.is_empty() {
        return Ok(lignes);
    }
    let cachet = |l: &Map<String, Value>| {
        l.get("rubrique")
            .and_then(Value::as_str)
            .map_or(false, |c| droit::catalogue::rubrique(&ctx.racine, c, &ctx.lue_au).map_or(false, |r| r.unite == "cachet"))
    };
    let jours: Vec<Value> = eng.dates_travail.iter().map(|j| Value::String(j.iso())).collect();
    let Some(i) = lignes.iter().position(cachet) else {
        // Pas de cachet : une journee, une masterclass. Les jours vont a la
        // premiere ligne, faute de mieux.
        lignes[0].insert("dates".into(), Value::Array(jours));
        if !eng.lieu.is_empty() {
            lignes[0].insert("lieu".into(), Value::String(eng.lieu.clone()));
        }
        return Ok(lignes);
    };
    let groupes = eng.dates_par_lieu(true);
    if groupes.len() <= 1 {
        lignes[i].insert("dates".into(), Value::Array(jours));
        let ou = groupes.first().map_or(eng.lieu.clone(), |g| g.0.clone());
        if !ou.is_empty() {
            lignes[i].insert("lieu".into(), Value::String(ou));
        }
        return Ok(lignes);
    }
    // Plusieurs lieux : la ligne de cachets se SCINDE, une par lieu.
    let mut eclatees = Vec::new();
    for (ou, jours) in groupes {
        let mut neuve = lignes[i].clone();
        neuve.insert("quantite".into(), nombre_f(jours.len() as f64));
        neuve.insert("dates".into(), Value::Array(jours.iter().map(|j| Value::String(j.iso())).collect()));
        if !ou.is_empty() {
            neuve.insert("lieu".into(), Value::String(ou));
        }
        eclatees.push(neuve);
    }
    let apres = lignes.split_off(i + 1);
    lignes.truncate(i);
    lignes.extend(eclatees);
    lignes.extend(apres);
    Ok(lignes)
}

/// Ce que la page propose quand la saisie ne le dit pas : `taux_cachet`, le taux d'une ligne de cachet neuve, celui
/// du dernier cachet de CE mois, sinon son `taux_cachet`, sinon le dernier taux saisi du dossier (`dernier_taux`),
/// sinon rien (`null`).
/// Ce que la page propose quand la saisie ne le dit pas : `taux_cachet`, le taux d'une ligne de cachet neuve, celui
/// du dernier cachet de CE mois, sinon son `taux_cachet`, sinon le dernier taux saisi du dossier (`dernier_taux`),
/// sinon rien (`null`).
fn defauts_de_la_saisie(ctx: &Contexte, saisie: &Value) -> Defauts {
    let du_mois = || -> Option<f64> {
        let objet = saisie.as_object()?;
        for e in elements(objet.get("engagements")).ok()?.iter().rev() {
            for l in elements(e.get("lignes")).ok()?.iter().rev() {
                if let Some(t) = taux_d_une_ligne(ctx, l) {
                    return Some(t);
                }
            }
        }
        objet.get("taux_cachet").filter(|t| vrai(t)).and_then(|t| flottant_de(t).ok())
    };
    // Un dossier qu'on ne sait pas relire ne propose rien (`null`) : la route rend la saisie telle quelle, et
    // c'est l'apercu qui dira pourquoi.
    Defauts { taux_cachet: du_mois().or_else(|| dernier_taux(ctx).ok().flatten()) }
}

/// La route `saisie`.
fn saisie(ctx: &Contexte, mois: Mois) -> R<SaisieDuMois> {
    let texte_mois = mois.to_string();
    let existe = engagement::stock::a_une_saisie(&ctx.racine, &texte_mois).map_err(Echec::from)?;
    // Un mois neuf s'ouvre VIDE, avec le dernier taux saisi ; un mois que des engagements touchent prend du squelette ce
    // qu'il ne dit pas de lui.
    let mut saisie = squelette(ctx, &texte_mois)?;
    if let Some(Value::Object(vue)) = engagement::stock::vue_du_mois(&ctx.racine, &texte_mois).map_err(Echec::from)? {
        for (k, v) in vue {
            saisie[k] = v;
        }
    }
    let defauts = defauts_de_la_saisie(ctx, &saisie);
    // LES ENGAGEMENTS IMPORTES QUI TOUCHENT LE MOIS, en cartes fermees : ils se lisent, rien ne les calcule
    // (`import::mois`). Un mois qui en porte ne recoit pas d'engagement neuf : `ferme_aux_neufs` est la phrase qui le
    // dit (`null` : le mois en recoit).
    let importes = crate::import::mois::cartes(ctx, &texte_mois)?;
    let ferme = ferme_aux_neufs(ctx, &texte_mois)?;
    // CE QUE L'ECRAN DIT DE LA SAISIE : rien pour un mois importe (sa phrase est `ferme_aux_neufs`).
    let phrase = match (ferme.is_none(), existe) {
        (false, _) => None,
        (true, true) => Some("Saisie chargée. L'enregistrement est automatique."),
        (true, false) => Some("Nouveau mois — l'enregistrement est automatique dès la première saisie."),
    };
    // L'en-tete des cartes importees : son titre, et combien d'engagements viennent de l'ancien logiciel.
    let n_importes = importes.len();
    Ok(SaisieDuMois {
        mois,
        phrase,
        importes_titre: if n_importes > 1 { "Importés" } else { "Importé" },
        importes_dit: format!("{} de l'ancien logiciel", super::compte(n_importes, "engagement", "engagements")),
        importes,
        ferme_aux_neufs: ferme,
        defauts,
        saisie: resoudre_noms(ctx, &saisie)?,
        identites: engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, &texte_mois).map_err(Echec::from)?),
    })
}
// ------------------------------------------------------- les jours d'un engagement

/// `j["jour"] if isinstance(j, dict) else j`.
fn jour_de(j: &Value) -> Value {
    match j {
        Value::Object(m) => m.get("jour").cloned().unwrap_or(Value::Null),
        autre => autre.clone(),
    }
}

/// Les jours travailles d'un engagement brut, AAAA-MM-JJ, tries : ceux de ses lignes de travail
/// (`engagement::dates_engagement`).
fn jours_joues(ctx: &Contexte, e: &Map<String, Value>) -> Vec<String> {
    let mut jours: Vec<String> = engagement::dates_engagement(&ctx.racine, &Value::Object(e.clone())).iter().map(|j| valeur_en_texte(&jour_de(j))).collect();
    jours.sort();
    jours
}

/// (debut, fin) d'un engagement brut, AAAA-MM-JJ, comme `Engagement` les deduirait : son debut (`engagement::debut_brut`),
/// sa fin ecrite, sinon le dernier de ses jours travailles et de son planning ; vides sans aucun jour.
pub(crate) fn bornes_brutes(ctx: &Contexte, e: &Map<String, Value>) -> (String, String) {
    let brut = Value::Object(e.clone());
    let fin = match e.get("fin").filter(|x| vrai(x)) {
        Some(f) => valeur_en_texte(f),
        None => engagement::dates_engagement(&ctx.racine, &brut)
            .iter()
            .chain(engagement::planning_engagement(&ctx.racine, &brut).iter())
            .map(|j| valeur_en_texte(&jour_de(j)))
            .max()
            .unwrap_or_default(),
    };
    (engagement::debut_brut(&ctx.racine, &brut), fin)
}

// ------------------------------------------------------------ les fichiers de mois

/// Les engagements bruts que le mois montre (`engagement::stock::vue_du_mois`), avec la cle du salarie ; vide si aucun
/// engagement ne le touche. Un engagement dont le salarie est inconnu (ou qui n'est pas un objet) est saute.
/// TRAVAIL A FAIRE (chaine A) : l'engagement brut, tel que son fichier l'ecrit.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: &str) -> R<Vec<(IdSalarie, Map<String, Value>)>> {
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for e in data.get("engagements").and_then(Value::as_array).into_iter().flatten() {
        let (Some(m), Some(cle)) = (e.as_object(), engagement::cle_de_l_engagement(ctx, e)) else { continue };
        out.push((cle, m.clone()));
    }
    Ok(out)
}

// ------------------------------------------------------------- contrats en cours

/// LES CONTRATS D'USAGE REPRIS D'UN AUTRE LOGICIEL QUI COURENT SUR `mois`. Un engagement d'ici commence un mois precedent
/// est deja a l'ecran de `mois` (un seul objet, lu dans le fichier du mois ou il commence : `engagement::stock`), comme
/// le contrat repris d'un permanent, qui continue dans son engagement (`engagement::importe`) ; ne restent que les
/// contrats d'usage que l'historique importe porte. Chacun se dit repris (`deja_repris`) quand un engagement de l'ecran
/// a son salarie, son debut et son numero. Jamais lu dans la DSN.
///
/// LE CONTRAT D'USAGE COMMENCE DANS L'ANCIEN LOGICIEL ET CONTINUE ICI (`engagement::importe::Importe::continue_apres`) :
/// ses mois verses sont importes, les suivants se paient ici, sous le numero et les dates que la DSN deposee lui a
/// donnes (regle 4). Le spectacle est celui de son numero d'objet, le cachet celui que l'ancien logiciel a paye ; son
/// engagement, pour le fichier du mois, est le contrat tel qu'il a ete signe, et une ligne de cachets a dater, au taux du
/// contrat, sinon au dernier saisi (la page ne choisit pas de taux ; un dossier qu'on ne sait pas relire, ou qui n'a
/// encore aucun cachet, R12, n'en propose aucun : la ligne reste a completer).
/// TRAVAIL A FAIRE (chaine A) : l'engagement importe se lit encore par ses cles.
fn contrats_en_cours(ctx: &Contexte, mois: &str) -> R<Vec<EnCours>> {
    let mut out: Vec<EnCours> = Vec::new();
    let mut vus: HashSet<(String, String, String)> = HashSet::new();
    let t = |x: &Map<String, Value>, k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    for i in engagement::importe::qui_continuent(&ctx.racine, mois).map_err(Echec::from)? {
        let b = i.objet().champs();
        let (salarie, debut, fin, numero) = (t(b, "salarie"), t(b, "debut"), t(b, "fin"), t(b, "numero"));
        if !vus.insert((salarie.clone(), debut.clone(), numero.clone())) {
            continue;
        }
        let Ok(fiche) = ctx.salarie(&salarie) else { continue };
        let paies = i.paies();
        let derniere = paies.values().last().copied().cloned().unwrap_or_default();
        let objet_du = |x: &Map<String, Value>| ["numero_objet", "objet"].iter().map(|k| t(x, k)).find(|o| !o.is_empty()).unwrap_or_default();
        let numero_objet = i.contrat().map(objet_du).filter(|o| !o.is_empty()).unwrap_or_else(|| objet_du(&derniere));
        let spectacle = ctx
            .spectacles
            .iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .find(|(_, s)| !numero_objet.is_empty() && s.get("numero_objet").map(valeur_en_texte).is_some_and(|n| n.eq_ignore_ascii_case(&numero_objet)))
            .map(|(k, _)| k.clone());
        let taux_cachet = derniere
            .get("lignes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|l| l.get("rubrique").and_then(Value::as_str) == Some("cachet"))
            .and_then(|l| l.get("taux").and_then(|x| valeur_en_texte(x).parse::<f64>().ok()));
        let taux = match taux_cachet.filter(|t| *t != 0.0) {
            Some(t) => Some(t),
            None => dernier_taux(ctx).ok().flatten(),
        };
        let engagement = Reprise {
            salarie: fiche.id.clone(),
            spectacle: spectacle.clone(),
            debut: debut.clone(),
            fin: fin.clone(),
            numero: Some(numero.clone()).filter(|n| !n.is_empty()),
            lignes: [LigneAReprendre { rubrique: "cachet", quantite: 0, taux, dates: [] }],
        };
        out.push(EnCours {
            nom: ctx.nomme(&salarie),
            salarie: fiche.id,
            spectacle,
            debut,
            fin,
            numero,
            depuis: i.continue_apres().unwrap_or_default(),
            metier: None,
            cadre: None,
            taux_cachet,
            lieu: "",
            deja_repris: false,
            engagement,
        });
    }
    let mut deja: HashSet<(IdSalarie, String, String)> = HashSet::new();
    for (c, e) in engagements_du_mois(ctx, mois)? {
        deja.insert((c, bornes_brutes(ctx, &e).0, e.get("numero").map(valeur_en_texte).unwrap_or_default()));
    }
    for c in out.iter_mut() {
        c.deja_repris = deja.contains(&(c.salarie.clone(), c.debut.clone(), c.numero.clone()));
    }
    Ok(out)
}

fn contrats_en_cours_route(ctx: &Contexte, mois: Mois) -> R<ContratsEnCours> {
    let contrats = contrats_en_cours(ctx, &mois.to_string())?;
    // Ce que l'ecran dit en posant les contrats qui ne sont pas encore repris : qui, et que ses jours restent a poser.
    let de_mois = socle::texte::de(&Mois::en_clair(&mois.to_string()));
    let noms: Vec<&str> = contrats.iter().filter(|c| !c.deja_repris).map(|c| c.nom.as_str()).collect();
    let reprise = match noms.len() {
        0 => None,
        1 => Some(format!("Contrat en cours repris : {} — poser ses jours {}.", noms[0], de_mois)),
        n => Some(format!("{} contrats en cours repris ({}) — poser leurs jours {}.", n, noms.join(", "), de_mois)),
    };
    Ok(ContratsEnCours { phrase: format!("Contrats d'un mois passé qui courent encore ce mois : {}.", contrats.len()), mois, contrats, reprise })
}

// ------------------------------------------------------------- contrats passes

const TITRE_A_COMPLETER: &str = "A COMPLETER";

/// Les spectacles retrouves par leur cle ou par leur numero d'objet. TRAVAIL A FAIRE : la fiche d'un spectacle se lit
/// par ses cles (`Contexte::spectacles`).
struct Spectacles<'a> {
    par_cle: &'a Map<String, Value>,
    par_objet: HashMap<String, &'a Value>,
}

impl<'a> Spectacles<'a> {
    fn nouveau(ctx: &'a Contexte) -> Spectacles<'a> {
        let mut par_objet = HashMap::new();
        for (k, v) in &ctx.spectacles {
            if k.starts_with('_') {
                continue;
            }
            if let Some(n) = v.get("numero_objet").filter(|x| vrai(x)) {
                par_objet.entry(valeur_en_texte(n)).or_insert(v);
            }
        }
        Spectacles { par_cle: &ctx.spectacles, par_objet }
    }

    /// Le spectacle de cle `x`, sinon de numero d'objet `x`.
    fn trouver(&self, x: &str) -> Option<&'a Value> {
        self.par_cle.get(x).filter(|v| vrai(v)).or_else(|| self.par_objet.get(x).copied())
    }
}

/// Le titre de la fiche, ou le numero d'objet a defaut.
fn titre_de(spec: Option<&Value>, numero: &str) -> String {
    let t = spec.and_then(|s| s.get("titre")).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
    let t = t.trim();
    if !t.is_empty() && t != TITRE_A_COMPLETER { t.to_string() } else { numero.to_string() }
}

/// « NOM Prénom » d'une clé de salarié (la clé elle-même si elle est inconnue).
fn nom_de(ctx: &Contexte, cle: &str) -> String {
    ctx.salaries.get(cle).and_then(|s| s.nomme()).unwrap_or_else(|| cle.to_string())
}

/// Un entier, de ce qu'un fichier porte (un nombre, ou son texte). TRAVAIL A FAIRE (chaine A) : l'engagement brut et
/// l'engagement importe le declareront.
fn entier_de(v: &Value) -> R<i64> {
    match v {
        Value::Bool(b) => Ok(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => {
                let x = n.as_f64().unwrap_or(0.0);
                if x.is_nan() {
                    Err(erreur("ValueError", "un nombre indefini (NaN) n'est pas un entier"))
                } else if x.is_infinite() {
                    Err(erreur("OverflowError", socle::fichiers::entier_hors_limites("infini")))
                } else {
                    Ok(x.trunc() as i64)
                }
            }
        },
        Value::String(s) => s.trim().replace('_', "").parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(s)))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre)))),
    }
}

/// Le nombre de cachets d'un engagement : la somme des quantites de ses lignes dont la rubrique est un cachet. Une ligne
/// qui ne dit rien de lisible (rubrique absente ou inconnue, quantite illisible) ne compte pas.
fn cachets_de(ctx: &Contexte, e: &Map<String, Value>) -> R<i64> {
    let mut cachets = 0;
    for l in e.get("lignes").and_then(Value::as_array).into_iter().flatten() {
        let Some(code) = l.get("rubrique").and_then(Value::as_str) else { continue };
        let Ok(r) = catalogue::rubrique(&ctx.racine, code, &ctx.lue_au) else { continue };
        if r.unite != "cachet" {
            continue;
        }
        let q = match l.get("quantite").filter(|x| vrai(x)) {
            Some(Value::Bool(b)) => *b as i64 as f64,
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
            Some(Value::String(s)) => match s.trim().replace('_', "").parse::<f64>() {
                Ok(x) => x,
                Err(_) => continue,
            },
            Some(_) | None => 0.0,
        };
        if q.is_nan() {
            continue;
        }
        if q.is_infinite() {
            return Err(erreur("OverflowError", socle::fichiers::entier_hors_limites("infini")));
        }
        cachets += q.trunc() as i64;
    }
    Ok(cachets)
}

/// Ce qu'une ligne de la liste dit d'un contrat passe : ses bornes, ses cachets et ses heures.
fn dit_du_contrat(debut: &str, fin: &str, cachets: i64, heures: i64) -> String {
    let court = |iso: &str| Jour::lire(iso).map(|j| j.jj_mm()).unwrap_or_else(|_| iso.to_string());
    let h = if heures > 0 { format!(" + {} h", heures) } else { String::new() };
    format!("{}→{} · {}{}", court(debut), court(fin), super::compte(cachets.max(0) as usize, "cachet", "cachets"), h)
}

/// UN CONTRAT QU'UN ENGAGEMENT IMPORTE ATTESTE (`engagement::importe::Importe::ligne_du_contrat`) ; celui d'un
/// permanent, mois par mois : chaque paie, aux jours qu'elle paie et avec ce que son bulletin compte.
struct Atteste {
    salarie: String,
    debut: String,
    fin: String,
    numero_objet: String,
    cachets: i64,
    heures: i64,
    jours: i64,
}

/// Les contrats que les engagements importes attestent. TRAVAIL A FAIRE (chaine A) : l'engagement importe se lit encore
/// par ses cles.
fn historique(ctx: &Contexte) -> R<Vec<Atteste>> {
    let texte_de = |x: &Map<String, Value>, k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let compte = |x: &Map<String, Value>, k: &str| x.get(k).filter(|v| vrai(v)).map_or(Ok(0), entier_de);
    let mut out = Vec::new();
    for i in engagement::importe::tous(&ctx.racine).map_err(Echec::from)? {
        let Some(Value::Object(contrat)) = i.ligne_du_contrat() else { continue };
        let atteste = |debut: String, fin: String, chiffres: &Map<String, Value>| -> R<Atteste> {
            Ok(Atteste {
                salarie: texte_de(&contrat, "salarie"),
                debut,
                fin,
                numero_objet: texte_de(&contrat, "numero_objet"),
                cachets: compte(chiffres, "cachets")?,
                heures: compte(chiffres, "heures")?,
                jours: compte(chiffres, "jours")?,
            })
        };
        if !i.permanent() {
            out.push(atteste(texte_de(&contrat, "debut"), texte_de(&contrat, "fin"), &contrat)?);
            continue;
        }
        for (m, p) in i.paies() {
            let lu = |k: &str| p.get(k).filter(|x| vrai(x)).map(valeur_en_texte).filter(|j| j.starts_with(&m));
            let debut = lu("debut").unwrap_or_else(|| format!("{}-01", m));
            let fin = match lu("fin") {
                Some(f) => f,
                None => Mois::lire(&m).map(Mois::dernier_jour).map(|j| j.iso()).map_err(plain)?,
            };
            out.push(atteste(debut, fin, p)?);
        }
    }
    Ok(out)
}

/// LES CONTRATS QUI TOUCHENT `mois`, pour le calendrier des mois passes. Deux sources : un mois qui a son fichier de paie
/// est lu la (avec ses jours exacts), l'historique importe parle pour les autres. Trie par salarie puis debut.
/// `salarie` : ses seuls contrats (absent : ceux de tous).
fn contrats_passes(ctx: &Contexte, mois: &str, salarie: Option<&IdSalarie>) -> R<Vec<Passe>> {
    let (premier, dernier) = (format!("{}-01", mois), format!("{}-31", mois));
    let voulu = |cle: &str| salarie.is_none_or(|s| s.as_str() == cle);
    let spectacles = Spectacles::nouveau(ctx);

    let mut couverts: HashSet<String> = HashSet::new();
    // Un contrat sur plusieurs mois est dans chacun de ses fichiers de mois, avec les memes bornes : on le garde une
    // fois, de preference tel que le fichier du mois regarde l'ecrit (c'est lui qui a les jours joues).
    let mut vus: HashMap<(String, String, String), usize> = HashMap::new();
    let mut out: Vec<Passe> = Vec::new();
    let mut connus = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
    connus.reverse();
    for mc in &connus {
        couverts.insert(mc.clone());
        for (cle, e) in engagements_du_mois(ctx, mc)? {
            let (debut, fin) = bornes_brutes(ctx, &e);
            if debut.is_empty() || debut > dernier || (if fin.is_empty() { &debut } else { &fin }).as_str() < premier.as_str() {
                continue;
            }
            if !voulu(&cle) {
                continue;
            }
            let ref_spec = e.get("spectacle").filter(|s| vrai(s)).map(valeur_en_texte).unwrap_or_default();
            let spec = spectacles.trouver(&ref_spec);
            let numero = spec.and_then(|s| s.get("numero_objet")).filter(|x| vrai(x)).map_or_else(|| ref_spec.clone(), valeur_en_texte);
            let cachets = cachets_de(ctx, &e)?;
            let dates = jours_joues(ctx, &e);
            let fin = if fin.is_empty() { debut.clone() } else { fin };
            let ident = (cle.to_string(), debut.clone(), fin.clone());
            let deja = vus.get(&ident).copied();
            if deja.is_some() && mc.as_str() != mois {
                continue;
            }
            let jours = match e.get("jours").filter(|x| vrai(x)) {
                Some(j) => entier_de(j)?,
                None => dates.len() as i64,
            };
            let contrat = Passe {
                dit: dit_du_contrat(&debut, &fin, cachets, 0),
                nom: nom_de(ctx, &cle),
                salarie: cle.to_string(),
                debut,
                fin,
                cachets,
                heures: 0,
                jours,
                titre: titre_de(spec, &numero),
                numero_objet: numero,
                dates: dates.into_iter().filter(|j| j.chars().take(7).eq(mois.chars())).collect(),
                source: "paie",
            };
            match deja {
                Some(i) => out[i] = contrat,
                None => {
                    vus.insert(ident, out.len());
                    out.push(contrat);
                }
            }
        }
    }
    for a in historique(ctx)? {
        if a.debut.is_empty() || a.debut > dernier || a.fin < premier {
            continue;
        }
        // Un mois produit ici se tait dans l'historique : ce sont les memes contrats, declares deux fois pendant le
        // double run.
        if couverts.contains(&a.fin.chars().take(7).collect::<String>()) || !voulu(&a.salarie) {
            continue;
        }
        let spec = spectacles.par_objet.get(&a.numero_objet).copied();
        out.push(Passe {
            dit: dit_du_contrat(&a.debut, &a.fin, a.cachets, a.heures),
            nom: nom_de(ctx, &a.salarie),
            titre: titre_de(spec, &a.numero_objet),
            salarie: a.salarie,
            debut: a.debut,
            fin: a.fin,
            cachets: a.cachets,
            heures: a.heures,
            jours: a.jours,
            numero_objet: a.numero_objet,
            dates: Vec::new(),
            source: "historique",
        });
    }
    out.sort_by(|a, b| (&a.salarie, &a.debut, &a.fin).cmp(&(&b.salarie, &b.debut, &b.fin)));
    Ok(out)
}

fn contrats_passes_route(ctx: &Contexte, c: ChargeContratsPasses) -> R<ContratsPasses> {
    let contrats = contrats_passes(ctx, &c.mois.to_string(), c.salarie.as_ref())?;
    Ok(ContratsPasses {
        phrase: format!("Contrats déclarés dans ce mois importé : {}.", contrats.len()),
        mois: c.mois,
        contrats,
        // Ce que la liste dit quand elle est vide : rien de declare pour ce salarie, ou aucun salarie choisi.
        rien_dit: if c.salarie.is_some() { "aucun contrat déclaré" } else { "aucun salarié coché" },
    })
}

// ------------------------------------------------------------------------ emploi

/// Un metier du catalogue traduit en champs de fiche, pour le formulaire salarie. LE METIER AU FEMININ pour une
/// salariee : le catalogue porte les deux formes (`feminin`), la fiche dit le sexe (le premier chiffre du NIR, sinon la
/// case `sexe` : `ntt::sexe_de`). `feminin` donne seul suffit aussi.
fn emploi(c: ChargeEmploi) -> R<Emploi> {
    let feminin = c.feminin || c.salarie.is_some_and(|s| declarations::ntt::sexe_de(&s.nir, &s.sexe) == "2");
    if emplois::metier(&c.metier).map_err(plain)?.is_none() {
        return Err(erreur(
            "ValueError",
            format!("Métier {} inconnu : ni dans le catalogue des métiers, ni dans la nomenclature France Travail.", repr(&c.metier)),
        ));
    }
    let champs = emplois::resoudre(&c.metier, c.cadre, feminin).map_err(plain)?;
    Ok(Emploi { phrase: format!("Métier : {} ({}).", champs.emploi_affichage, champs.code_emploi_ft.as_deref().unwrap_or_default()), champs })
}
// ---------------------------------------------------------------- rubriques cochees

const GARDER_COPIES: usize = 20;

/// La copie horodatee du fichier, avant de le
/// remplacer (l'heure de Paris), et seulement les `GARDER_COPIES` dernieres.
pub(crate) fn sauver_avant(chemin: &Path) -> R<()> {
    if !chemin.exists() {
        return Ok(());
    }
    let io = |e: std::io::Error| erreur("OSError", e.to_string());
    let parent = chemin.parent().unwrap_or(Path::new("."));
    let dossier = parent.join("avant");
    std::fs::create_dir_all(&dossier).map_err(io)?;
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let maintenant = socle::temps::secondes();
    let horodatage = socle::paris::iso_secondes(maintenant).replace(['-', ':'], "").replace('T', "-");
    std::fs::copy(chemin, dossier.join(format!("{}.{}", nom, horodatage))).map_err(io)?;
    let prefixe = format!("{}.", nom);
    let mut anciennes: Vec<String> = std::fs::read_dir(&dossier)
        .map_err(io)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefixe))
        .collect();
    anciennes.sort();
    if anciennes.len() > GARDER_COPIES {
        for vieille in &anciennes[..anciennes.len() - GARDER_COPIES] {
            let _ = std::fs::remove_file(dossier.join(vieille));
        }
    }
    Ok(())
}


/// `refuser_si_clos` : un exercice cloture ne bouge plus.
pub(crate) fn refuser_si_clos(ctx: &Contexte, mois: &str) -> R<()> {
    let annee: String = mois.chars().take(4).collect();
    if let Some(clos) = crate::routes::rappels::exercice_clos(ctx, &annee)?.filter(vrai) {
        let le: String = clos.get("le").filter(|v| vrai(v)).map_or_else(|| "?".to_string(), valeur_en_texte).chars().take(10).collect();
        return Err(refus(format!(
            "L'exercice {} est cloture depuis le {} : {} ne se modifie plus. Rouvrir l'exercice (Historique > L'exercice, ou \
             la route `cloture` avec `rouvrir`) avant de toucher a ce mois.",
            annee, le, mois
        )));
    }
    Ok(())
}

/// UN MOIS QUI PORTE DES PAIES IMPORTEES NE RECOIT PAS D'ENGAGEMENT NEUF : ses paies ont ete calculees et declarees par
/// l'ancien logiciel (`engagement::importe`), et sa DSN avec elles ; un engagement de plus la changerait, et une
/// declaration deja faite ne se regenere pas (regle 4). Ce mois se finit dans l'ancien logiciel. Rien ne passe outre.
/// Rend la phrase qui le dit (`None` : le mois recoit des engagements) ; la route `saisie` la rend a la page, les
/// ecritures et l'apercu d'une carte neuve refusent avec elle.
fn ferme_aux_neufs(ctx: &Contexte, mois: &str) -> R<Option<String>> {
    let importes = engagement::importe::du_mois(&ctx.racine, mois).map_err(Echec::from)?;
    Ok(importes.iter().any(|i| i.paies().contains_key(mois)).then(|| {
        format!(
            "Le mois de {} porte des paies importées de votre ancien logiciel : il ne reçoit pas d'engagement neuf. Finissez ce mois \
             dans l'ancien logiciel.",
            socle::mois::Mois::en_clair(mois)
        )
    }))
}

fn refuser_si_importe(ctx: &Contexte, mois: &str) -> R<()> {
    match ferme_aux_neufs(ctx, mois)? {
        Some(phrase) => Err(refus(format!("{} Rien n'est écrit.", phrase))),
        None => Ok(()),
    }
}

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE NE SE SAISIT PAS : la DSN est due par l'employeur pour les salaries qu'il
/// emploie, et le premier mois du est celui de la premiere embauche (`premiere_embauche` de la fiche Entreprise, la
/// borne que l'echeancier, les rappels et le panneau lisent : `compagnie::etat_dsn::premier_mois_du`), une fois l'echeance
/// de sa DSN passee (`echeance_dsn`). Avant elle, une compagnie qui commence saisit ce mois, et sa DSN se depose a temps.
/// Un mois qui a deja des engagements se modifie.
pub(crate) fn refuser_si_avant_la_premiere_embauche(ctx: &Contexte, mois: &str) -> R<()> {
    let embauche = ctx.entreprise.premiere_embauche.dite().map(|j| j.iso()).unwrap_or_default();
    if embauche.len() >= 7
        && mois < &embauche[..7]
        && declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(Echec::from)?.passee_au(&ctx.aujourdhui)
        && !engagement::stock::a_une_saisie(&ctx.racine, mois).map_err(Echec::from)?
    {
        return Err(refus(format!(
            "{} precede la premiere embauche de la compagnie ({}) : aucune DSN n'y est due, rien ne s'y saisit. Si la \
             compagnie employait deja, corriger la date de premiere embauche (Configuration › Entreprise). Rien n'est ecrit.",
            mois,
            Jour::lire(&embauche).map(|j| j.jj_mm_aaaa()).unwrap_or(embauche)
        )));
    }
    Ok(())
}


/// « ENREGISTRE » : la carte d'un engagement en saisie s'ecrit, et elle seule (`ecrire_une_carte`). Rend l'identite de
/// l'engagement ecrit, ce qui lui manque encore (`None` : rien) et l'heure de l'ecriture.
fn enregistrer_saisie(ctx: &Contexte, c: ChargeCarte) -> R<CarteEcrite> {
    let mois = c.mois.to_string();
    let _verrou = avant_d_ecrire(ctx, &mois)?;
    let id = ecrire_une_carte(ctx, &mois, c.engagement.as_ref(), &c.saisie)?;
    let (k, _) = engagement::stock::designe(&ctx.racine, &mois, &id).map_err(Echec::from)?;
    let vue = engagement::stock::vue_du_mois(&ctx.racine, &mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    let a_completer = manque_de(ctx, &vue, k);
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let a = maintenant();
    Ok(CarteEcrite {
        phrase: match &a_completer {
            Some(m) => format!("Carte enregistrée ; à compléter avant de valider : {}", m),
            None => "Carte enregistrée.".to_string(),
        },
        engagement: id,
        a_completer,
        enregistre_dit: format!("Enregistré à {}", a.chars().skip(11).take(5).collect::<String>()),
        enregistre_a: a,
    })
}

/// AVANT TOUTE ECRITURE D'UN ENGAGEMENT (saisir, valider, modifier, retirer) : une ecriture a la fois (la fenetre sert
/// ses routes en parallele, et la page enregistre une seconde apres la derniere frappe, meme si l'enregistrement d'avant
/// n'a pas fini), puis les trois refus du mois : des paies importees (`refuser_si_importe`), un exercice clos
/// (`refuser_si_clos`), un mois d'avant la premiere embauche (`refuser_si_avant_la_premiere_embauche`). Le verrou rendu
/// se garde jusqu'a la fin de l'ecriture.
pub(crate) fn avant_d_ecrire(ctx: &Contexte, mois: &str) -> R<std::sync::MutexGuard<'static, ()>> {
    static ECRITURE_EN_COURS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let verrou = ECRITURE_EN_COURS.lock().unwrap_or_else(|e| e.into_inner());
    refuser_si_importe(ctx, mois)?;
    refuser_si_clos(ctx, mois)?;
    refuser_si_avant_la_premiere_embauche(ctx, mois)?;
    Ok(verrou)
}

/// LA CARTE D'UN ENGAGEMENT EN SAISIE, ECRITE (`engagement::stock::ecrire_la_carte`) : `saisie`, et `id`, son identite
/// (absente : un engagement neuf). Un mois sans fichier le prend avec ce qu'il dit par defaut (`squelette` : le cachet et
/// la date de paiement du mois). Une carte a laquelle il manque un fait s'ecrit aussi : seule la validation refuse. Rend
/// l'identite de l'engagement ecrit. L'appelant tient le verrou (`avant_d_ecrire`).
pub(crate) fn ecrire_une_carte(ctx: &Contexte, mois: &str, id: Option<&Identite>, saisie: &Map<String, Value>) -> R<Identite> {
    // UN ENGAGEMENT NEUF S'ECRIT DANS SON PREMIER MOIS (`engagement::stock::mois_d_ecriture`) : ce mois-la passe les memes
    // refus que celui de la carte, et donne ce qu'il dit par defaut.
    let fichier = match id {
        Some(_) => mois.to_string(),
        None => engagement::stock::mois_d_ecriture(&ctx.racine, mois, &Value::Object(saisie.clone())).map_err(Echec::from)?,
    };
    if fichier != mois {
        refuser_si_importe(ctx, &fichier)?;
        refuser_si_clos(ctx, &fichier)?;
    }
    let mut defauts = match squelette(ctx, &fichier)? {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    defauts.remove("engagements");
    let id = engagement::stock::ecrire_la_carte(ctx, mois, id, saisie, &defauts).map_err(|m| erreur("ErreurOperation", m))?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(id)
}

/// CE QUI MANQUE A CHAQUE ENGAGEMENT DE LA SAISIE `data` pour se calculer, dans son ordre (`None` : rien). Une saisie
/// qui se calcule entiere n'a rien qui manque. Sinon, chaque engagement se calcule a la suite de ceux d'avant qui se
/// calculent (le calcul d'un engagement lit ceux qui le precedent) : celui qui refuse porte le refus, et les suivants se
/// calculent sans lui. Un engagement valide ne manque de rien, sa validation l'a calcule, sauf les indemnites journalieres
/// d'un arret venu apres elle (`engagement::maintien::indemnites_a_dire`).
pub(crate) fn manques_de_la_saisie(ctx: &Contexte, data: &Value) -> Vec<Option<String>> {
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mois = valeur_en_texte(&data["mois"]);
    let a_dire = |e: &Value| -> Option<String> {
        if engagement::etats::est_validee(e) { engagement::maintien::indemnites_a_dire(ctx, e, &mois).ok().flatten() } else { None }
    };
    let valides: Vec<Option<String>> = tous.iter().map(a_dire).collect();
    if valides.iter().all(Option::is_none) && (tous.iter().all(engagement::etats::est_validee) || engagement::pour_l_ecran(ctx, data).is_ok()) {
        return vec![None; tous.len()];
    }
    let mut gardes: Vec<Value> = Vec::new();
    let mut manques = Vec::new();
    for (e, manque) in tous.iter().zip(valides) {
        if engagement::etats::est_validee(e) {
            if manque.is_none() {
                gardes.push(e.clone());
            }
            manques.push(manque);
            continue;
        }
        let mut essai = data.clone();
        essai["engagements"] = Value::Array(gardes.iter().cloned().chain(std::iter::once(e.clone())).collect());
        match engagement::pour_l_ecran(ctx, &essai) {
            Ok(_) => {
                gardes.push(e.clone());
                manques.push(None);
            }
            Err(m) => manques.push(Some(m)),
        }
    }
    manques
}

/// CE QUI MANQUE A L'ENGAGEMENT `i` DE LA SAISIE `data` (`manques_de_la_saisie`), sur ce que son calcul lit : les
/// engagements de son salarie (une paie ne lit que les siens, `engagement::etats`) et ceux qui portent son numero de
/// contrat (un numero est celui d'un seul salarie). Les autres salaries du mois ne se calculent pas pour lui.
pub(crate) fn manque_de(ctx: &Contexte, data: &Value, i: usize) -> Option<String> {
    let tous = data.get("engagements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]);
    let e = tous.get(i)?;
    let cle = engagement::cle_de_l_engagement(ctx, e);
    let numero = e.get("numero").filter(|n| vrai(n)).map(valeur_en_texte);
    let mut rang = 0;
    let mut siens = Vec::new();
    for (j, x) in tous.iter().enumerate() {
        let meme_salarie = cle.is_some() && engagement::cle_de_l_engagement(ctx, x) == cle;
        let meme_numero = numero.is_some() && x.get("numero").filter(|n| vrai(n)).map(valeur_en_texte) == numero;
        if j == i {
            rang = siens.len();
        }
        if j == i || meme_salarie || meme_numero {
            siens.push(x.clone());
        }
    }
    let mut d = data.clone();
    d["engagements"] = Value::Array(siens);
    manques_de_la_saisie(ctx, &d).swap_remove(rang)
}


// ------------------------------------------------------------------ le montant vise

/// Une valeur saisie : ni absente, ni `null`, ni vide.
fn rempli(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Null)) && v.and_then(Value::as_str) != Some("")
}

/// Ce qui a change, de l'engagement `avant` a l'engagement `apres` (la saisie que la recherche a ajustee). TRAVAIL A
/// FAIRE (chaine A) : la saisie d'un engagement se lit par ses cles.
fn ce_qui_a_change(avant: &Value, apres: &Value, index: usize) -> CeQuiChange {
    let lire = |v: &Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let lignes_de = |e: &Value| e.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut lignes = Vec::new();
    for (rang, (a, b)) in lignes_de(avant).iter().zip(lignes_de(apres).iter()).enumerate() {
        let (ta, tb) = (lire(a, "taux"), lire(b, "taux"));
        if !rempli(a.get("taux")) && !vrai(&tb) {
            continue; // restee sans taux : rien a ecrire
        }
        if !socle::valeur::valeurs_egales(&ta, &tb) {
            lignes.push(TauxQuiChange { rang, rubrique: b.get("rubrique").cloned().unwrap_or_else(|| json!("cachet")), taux_avant: ta, taux: tb });
        }
    }
    let (sa, sb) = (lire(avant, "salaire_mensuel"), lire(apres, "salaire_mensuel"));
    CeQuiChange { index, salaire_mensuel: (!socle::valeur::valeurs_egales(&sb, &sa)).then_some(sb), lignes }
}

/// Ce que la phrase du montant vise dit de la cible.
fn cible_dite(c: Cible) -> &'static str {
    match c {
        Cible::Net => "Net salarié",
        Cible::Brut => "Brut",
        Cible::Cout => "Coût entreprise",
    }
}

/// Le taux qu'il faudrait, pour que l'engagement d'une carte (`carte` : son rang dans `cartes`, `mois_designe`) coute
/// CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, c: ChargeAjuster) -> R<Ajuste> {
    let vise = q2(socle::num::dec_f64(c.montant).map_err(|m| erreur("InvalidOperation", m))?);
    if vise <= Decimal::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", vise)));
    }
    let (data, places, _) = mois_designe(ctx, &c.mois.to_string(), &c.cartes)?;
    let i = *places.get(c.carte).ok_or_else(|| refus(format!("La carte {} n'est pas dans `cartes`.", c.carte + 1)))?;
    let v = engagement::viser::viser(ctx, &data, &[i], c.cible, vise).map_err(|r| match r {
        engagement::viser::RefusVisee::Refus(m) => refus(m),
        engagement::viser::RefusVisee::Calcul(m) => refus_du_calcul(ctx, &data, m),
    })?;
    let vue = vue_paie(ctx, &v.paies[i], &v.saisie["engagements"][i], &v.saisie).map_err(Echec::from)?;
    let atteint = q2(v.atteint);
    let mut facteur = v.facteur.round_dp(6);
    facteur.rescale(6);
    let change = ce_qui_a_change(&data["engagements"][i], &v.saisie["engagements"][i], i);
    // Le taux qu'il faudrait : chaque ligne qui change, puis le salaire du mois s'il change.
    let mut faut: Vec<String> = change
        .lignes
        .iter()
        .map(|l| {
            let avant = if l.taux_avant.is_null() { String::new() } else { format!(" au lieu de {}", socle::texte::euros_de(&l.taux_avant)) };
            format!("{} à {}{}", valeur_en_texte(&l.rubrique), socle::texte::euros_de(&l.taux), avant)
        })
        .collect();
    if let Some(s) = change.salaire_mensuel.as_ref().filter(|s| !s.is_null()) {
        faut.push(format!("salaire du mois à {}", socle::texte::euros_de(s)));
    }
    let avec = if faut.is_empty() { String::new() } else { format!(" ; {}", faut.join(", ")) };
    let ecart = atteint - vise;
    let pres = if ecart.is_zero() {
        " au centime près".to_string()
    } else {
        format!(" ({}{} : un taux se paie au centime)", if ecart > Decimal::ZERO { "+" } else { "" }, socle::texte::euros(ecart))
    };
    Ok(Ajuste {
        phrase: format!("{} visé {} → {}{}{}.", cible_dite(c.cible), socle::texte::euros(vise), socle::texte::euros(atteint), pres, avec),
        cible: c.cible,
        vise,
        atteint,
        depart: q2(v.depart),
        ecart,
        facteur,
        tours: v.tours,
        engagements: [change],
        paies: [vue],
    })
}

// ============================================================== l'apercu

/// Le refus du calcul d'une saisie : le moteur refuse avec sa phrase, mais
/// quand la saisie elle-meme est encore incomplete (un spectacle pas encore
/// choisi, des jours ou des heures pas encore saisis :
/// `engagement::saisie_incomplete`) c'est cette phrase-la, sous son nom
/// (`SaisieIncomplete`), qui monte. La page les
/// dit sobrement ; l'ecran enregistre a chaque frappe, donc ce ne sont pas des
/// defauts du logiciel.
pub(crate) fn refus_du_calcul(ctx: &Contexte, data: &Value, message: String) -> Echec {
    if engagement::saisie_incomplete(&message) {
        return erreur("SaisieIncomplete", message);
    }
    if let Some(m) = engagement::refus_de_saisie_incomplete(ctx, data) {
        return erreur("SaisieIncomplete", m);
    }
    Echec::from(message)
}

/// LE MOIS QUE L'APERCU ET LE MONTANT VISE CALCULENT : ses engagements tels qu'ils sont ecrits, dans l'ordre de l'ecran
/// (`vue_du_mois`), et chaque carte que l'ecran montre autrement (`cartes`) : a sa place, ou a la suite. Rend ce mois,
/// la place de chaque carte dans ses engagements, et l'identite de chacun de ses engagements (`None` pour une carte pas
/// encore ecrite). Rien ne s'ecrit. TRAVAIL A FAIRE (chaine A) : la vue du mois se lit et se compose par ses cles.
fn mois_designe(ctx: &Contexte, mois: &str, cartes: &[Carte]) -> R<(Value, Vec<usize>, Vec<Option<Identite>>)> {
    let mut v = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or_else(|| json!({"engagements": []}));
    v["mois"] = Value::String(mois.to_string());
    let mut tous: Vec<Value> = v.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let ids = engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, mois).map_err(Echec::from)?);
    let mut places = Vec::new();
    for c in cartes {
        let mut saisie = engagement::stock::en_forme(&c.saisie).map_err(Echec::from)?;
        let i = match &c.engagement {
            None => {
                tous.push(Value::Object(saisie));
                tous.len() - 1
            }
            Some(id) => {
                let i = ids.iter().position(|x| x == id).filter(|i| *i < tous.len()).ok_or_else(|| {
                    refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois.", Mois::en_clair(mois)))
                })?;
                // L'avenant du mois se deduit des termes dates de l'engagement ecrit (`stock::Objet::vue`) : la carte
                // ne le porte pas.
                for k in ["date_modification", "avant"] {
                    if let Some(x) = tous[i].get(k).cloned() {
                        saisie.insert(k.to_string(), x);
                    }
                }
                tous[i] = Value::Object(saisie);
                i
            }
        };
        places.push(i);
    }
    let identites = (0..tous.len()).map(|i| ids.get(i).cloned()).collect();
    v["engagements"] = Value::Array(tous);
    Ok((v, places, identites))
}

/// Une paie que l'ecran montre, et pourquoi elle s'est recalculee d'elle-meme (`{"le", "raison"}`), s'il y a lieu.
pub(crate) struct PaieMontree {
    pub(crate) paie: Paie,
    pub(crate) recalcul: Option<Value>,
}


/// LES PAIES QUE L'ECRAN MONTRE POUR LA SAISIE `data` : un engagement valide se lit (sa paie ecrite, `etats::Valide::paie`),
/// un engagement en saisie se calcule, sans rien ecrire, comme un engagement valide d'un mois d'apres le present, qui n'a
/// pas encore de paie. Un mois tout valide et present ne calcule rien.
///
/// L'ecran rend chaque engagement valide tel que le moteur le lui a montre (`valide` pose) : il retrouve le sien, sur le
/// disque, par son salarie et son numero de contrat (ecrit a la validation).
pub(crate) fn paies_de_l_ecran(ctx: &Contexte, data: &Value, mois: &str) -> R<Vec<PaieMontree>> {
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let disque = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let etats = if disque.is_some() { engagement::etats::du_mois(ctx, mois, &departs)? } else { Vec::new() };
    let qui = |b: &Map<String, Value>| (b.get("salarie").map(en_texte), b.get("numero").filter(|n| vrai(n)).map(valeur_en_texte));
    let mut lues: Vec<Option<PaieMontree>> = Vec::new();
    for e in &tous {
        let mut lue = None;
        if let (true, Some(b)) = (engagement::etats::est_validee(e), e.as_object()) {
            let sien = qui(b);
            // Un mois d'apres le present n'a pas encore de paie ecrite : l'engagement valide s'y calcule pour l'ecran.
            let a_venir = |x: &engagement::etats::Etat| mois > engagement::etats::mois_present(ctx).as_str() && x.valide().is_some_and(|v| v.paie_ecrite(mois).is_none());
            let trouve = etats.iter().enumerate().find(|(_, x)| x.valide().is_some() && !a_venir(x) && sien.1.is_some() && qui(x.objet().champs()) == sien);
            if let (Some((j, x)), Some(d)) = (trouve, disque.as_ref()) {
                let v = x.valide().ok_or("engagement en saisie")?;
                lue = Some(PaieMontree { paie: v.paie(ctx, d, j)?, recalcul: v.recalcul(mois) });
            }
        }
        lues.push(lue);
    }
    let calculees = if lues.iter().any(Option::is_none) {
        engagement::pour_l_ecran(ctx, data).map_err(|m| refus_du_calcul(ctx, data, m))?.paies
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for (i, l) in lues.into_iter().enumerate() {
        match l {
            Some(l) => out.push(l),
            None => out.push(PaieMontree { paie: calculees.get(i).cloned().ok_or("le calcul du mois n'a pas cette paie")?, recalcul: None }),
        }
    }
    Ok(out)
}

/// Deux décimales, la moitié au pair.
pub(crate) fn q2(x: Decimal) -> Decimal {
    let mut r = x.round_dp(2);
    r.rescale(2);
    r
}

/// LA RUPTURE d'un engagement, telle que la page la propose (« Un imprévu… ») : le libelle de la porte et la fin du
/// contrat (celle qu'il dit, ou son dernier jour travaille quand il est aux cachets).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Rupture {
    porte: &'static str,
    fin: Option<Jour>,
}

/// LA RUPTURE de cet engagement : `None` quand elle n'a pas de sens ici -- une nature sans motif de rupture
/// (S21.G00.62.002, CCH-11 : `Nature::motifs_rupture`), ou un contrat a terme qui ne finit pas ce mois-ci. Les motifs et
/// ce que chacun demande viennent de `natures`.
fn rupture_de(e: &engagement::Engagement, nat: &droit::natures::Nature) -> R<Option<Rupture>> {
    if nat.motifs_rupture().is_empty() {
        return Ok(None);
    }
    let porte = if nat.duree_indeterminee { "La fin du contrat (licenciement, démission…)" } else { "Une rupture avant terme" };
    // Un contrat paye au mois (CDI, CDD) dit sa fin dans sa case : elle se tape, le mois ou elle a lieu.
    if nat.duree_indeterminee || nat.mensuelle() {
        return Ok(Some(Rupture { porte, fin: e.fin }));
    }
    // Un contrat aux cachets finit son dernier jour travaille ; le bloc 62 s'ecrit ce mois-la et lui seul (fiche n° 3344,
    // `fin_de_contrat` de `contrats-base.json`) : sa rupture se dit ce mois-la.
    if !e.dernier_mois()? {
        return Ok(None);
    }
    Ok(Some(Rupture { porte, fin: Some(match e.fin {
        Some(f) => f,
        None => e.fin_periode()?,
    }) }))
}


/// LA CASE DU NUMERO DEJA DECLARE EN DSN d'un contrat commence avant le logiciel : un numero ne se recalcule pas, il se lit
/// (regle 4) ; celui que l'ancien logiciel a declare (S21.G00.40.009) reste le sien, la saisie le porte en `numero`
/// (`engagement::numeros::a_la_validation` le garde). Vide, le logiciel en donne un.
const NUMERO_DIT: &str = "numero";

/// La case du numero, sauf pour un contrat dont une DSN de l'ancien logiciel a declare le numero : il est lu
/// (`engagement::importe::numero_declare_du_contrat`), il ne se dit pas.
fn numero_a_dire(ctx: &Contexte, salarie: &str, debut: &str) -> Vec<&'static str> {
    match engagement::importe::numero_declare_du_contrat(&ctx.racine, salarie, debut) {
        Ok(true) => Vec::new(),
        _ => vec![NUMERO_DIT],
    }
}

/// LES CASES DU CONTRAT ECRIT QUE CET ENGAGEMENT APPELLE (`engagement::CHAMPS_DU_CONTRAT_ECRIT`), et elles seules : la
/// periode d'essai et les clauses particulieres de tout contrat ecrit ; la personne remplacee d'un CDD de remplacement
/// (L1242-12, 1° ; `Nature::motif_de_remplacement`) ; la repartition des horaires d'un temps partiel (L3123-6, 1° : la
/// modalite 20 de S21.G00.40.014, `Engagement::modalite_temps_travail`). Un engagement dont le contrat ne se fait pas
/// signer ici (`engagement::contrat_a_signer`) n'en a aucune.
/// Un contrat commence avant le logiciel (`engagement::stock::commence_avant_le_logiciel`) a ete signe ailleurs : il
/// n'appelle que la case de son numero deja declare en DSN (`NUMERO_DIT`).
fn champs_du_contrat_ecrit(ctx: &Contexte, e: &engagement::Engagement, nat: &droit::natures::Nature) -> R<Vec<&'static str>> {
    if engagement::stock::commence_avant_le_logiciel(&ctx.racine, &e.debut.iso(), &e.mois)? {
        return Ok(numero_a_dire(ctx, &e.salarie, &e.debut.iso()));
    }
    if !engagement::contrat_a_signer(&ctx.racine, e)? {
        return Ok(Vec::new());
    }
    let mut v = vec!["periode_essai_jours"];
    if e.motif_recours.as_deref().is_some_and(|m| nat.motif_de_remplacement(m)) {
        v.push("personne_remplacee");
    }
    if nat.mensuelle() && e.modalite_temps_travail()? == "20" {
        v.push("repartition_horaire");
    }
    v.push("clauses_particulieres");
    Ok(v)
}

/// `champs_du_contrat_ecrit` d'un engagement qui ne se calcule pas encore (`a_completer`), lu sur sa saisie `e` : la
/// periode d'essai et les clauses se disent avant que le reste soit complet. La repartition des horaires attend la
/// duree du travail calculee.
fn champs_du_contrat_ecrit_en_saisie(ctx: &Contexte, e: &Value, mois: &str) -> Vec<&'static str> {
    if engagement::stock::commence_avant_le_logiciel(&ctx.racine, &engagement::debut_brut(&ctx.racine, e), mois).unwrap_or(false) {
        return numero_a_dire(ctx, &e.get("salarie").map(en_texte).unwrap_or_default(), &engagement::debut_brut(&ctx.racine, e));
    }
    let Ok(nat) = droit::natures::nature(e.get("nature").and_then(Value::as_str)) else { return Vec::new() };
    let Some(cle) = engagement::cle_de_l_engagement(ctx, e) else { return Vec::new() };
    let numero = e.get("numero").filter(|n| vrai(n)).map(en_texte).unwrap_or_default();
    if nat.modele_contrat().is_none() || engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, &cle, &engagement::debut_brut(&ctx.racine, e), &numero) {
        return Vec::new();
    }
    let mut v = vec!["periode_essai_jours"];
    if e.get("motif_recours").and_then(Value::as_str).is_some_and(|m| nat.motif_de_remplacement(m)) {
        v.push("personne_remplacee");
    }
    v.push("clauses_particulieres");
    v
}

/// Le salarie d'une paie a l'ecran : sa cle, son nom, son prenom (celui d'usage s'il en a un).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SalarieVu {
    cle: IdSalarie,
    nom: droit::fiche::Case<String>,
    prenom: droit::fiche::Case<String>,
}

/// Le spectacle d'une paie : son titre et son numero d'objet. TRAVAIL A FAIRE : la fiche d'un spectacle
/// (`Contexte::spectacles`) se lit par ses cles.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SpectacleVu {
    titre: Value,
    numero_objet: Value,
}

/// La nature du contrat : sa cle, son libelle, et s'il se paie au mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct NatureVue {
    cle: String,
    libelle: String,
    mensuelle: bool,
}

/// LES CHEVRONS D'UN CONTRAT SUR PLUSIEURS MOIS, tels qu'on les lit au survol, de chaque cote ou il deborde du mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Chevrons {
    avant: Option<String>,
    apres: Option<String>,
}

/// La periode de paie d'un contrat dans le mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Periode {
    debut: Jour,
    fin: Jour,
}

/// Le contrat par rapport au mois : un contrat qui deborde n'a pas de DPAE ni de contrat imprime hors de son premier mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContratVu {
    premier_mois: bool,
    dernier_mois: bool,
    a_cheval: bool,
    chevrons: Chevrons,
    sans_paie: bool,
    depuis: String,
    jusqu_a: String,
    periode: Periode,
    ecrits: Vec<&'static str>,
    rupture: Option<Rupture>,
}

/// Une case qu'une ligne de frais appelle : le rang de la ligne dans la saisie, la case, ce qu'elle dit, son aide.
/// TRAVAIL A FAIRE (chaine A) : la case et son aide se lisent de la ligne, par ses cles.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CaseDeFrais {
    ligne: u64,
    case: String,
    libelle: &'static str,
    aide: Value,
}

/// Une ligne de la remuneration, telle que la paie la calcule.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LigneVue {
    rubrique: String,
    designation: String,
    quantite: Decimal,
    taux: Decimal,
    montant: Decimal,
}

/// L'emploi du salarie, tel que sa fiche le dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmploiVu {
    libelle: droit::fiche::Case<String>,
    code_emploi_ft: droit::fiche::Case<String>,
    statut: droit::fiche::Case<droit::fiche::Statut>,
    cadre: bool,
}

/// UNE PAIE POUR L'ECRAN (`apercu`, `ajuster`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct VuePaie {
    resolu_dit: String,
    contrat_dit: String,
    /// Ce que la carte validee dit de la paie : ce qu'elle paye, en bref, son bulletin.
    paye_dit: String,
    bulletin_dit: Option<String>,
    numero_bulletin: Option<String>,
    numero_contrat: Option<engagement::numeros::NumeroContrat>,
    salarie: SalarieVu,
    /// UN PERMANENT N'A PAS DE SPECTACLE, et `None` est la reponse juste.
    spectacle: Option<SpectacleVu>,
    nature: NatureVue,
    debut: Jour,
    fin: Option<Jour>,
    contrat: ContratVu,
    dates_travail: Vec<Jour>,
    cachets: i64,
    /// TRAVAIL A FAIRE (chaine A) : sans cachet, le taux tel que la saisie l'ecrit.
    taux_cachet: Value,
    heures: Option<f64>,
    faits_avances: Vec<&'static str>,
    cases: Vec<CaseDeFrais>,
    remuneration: Vec<LigneVue>,
    /// TRAVAIL A FAIRE (chaine A) : les jours et le lieu tels que la saisie les ecrit.
    jours: Value,
    lieu: Value,
    brut: Decimal,
    net_a_payer: Decimal,
    net_paye: Decimal,
    net_imposable: Decimal,
    cout_employeur: Decimal,
    avertissements: Vec<String>,
    informations: Vec<String>,
    emploi: EmploiVu,
    /// TRAVAIL A FAIRE : le conseil de saisie (`compagnie::conseils::de_saisie`) se compose en JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    conseil: Option<Value>,
    /// TRAVAIL A FAIRE : la ligne de « Pour démarrer » (`declarations::demarrage::conges_a_dire`) se compose en JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    a_dire: Option<[Value; 1]>,
}

/// Une paie pour l'ecran. `brut` : l'engagement tel qu'il est ecrit dans la saisie ; `data` : la saisie du mois (pour
/// `lieu` et `taux_cachet`, gardes tels qu'ecrits). TRAVAIL A FAIRE (chaine A) : ils se lisent par leurs cles.
pub(crate) fn vue_paie(ctx: &Contexte, p: &Paie, brut: &Value, data: &Value) -> R<VuePaie> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let s = &p.salarie;
    let exige = |c: &droit::fiche::Case<String>, k: &str| if c.est_absente() { Err(erreur("KeyError", socle::fichiers::absent(k))) } else { Ok(c.clone()) };
    let salarie = SalarieVu {
        cle: s.id.clone(),
        nom: exige(&s.nom, "nom")?,
        prenom: if s.prenom_affichage.texte().is_empty() { exige(&s.prenom, "prenom")? } else { s.prenom_affichage.clone() },
    };
    let spectacle = match vrai(&p.spectacle) {
        true => Some(SpectacleVu {
            titre: p.spectacle.get("titre").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("titre")))?,
            numero_objet: p.spectacle.get("numero_objet").cloned().unwrap_or(Value::Null),
        }),
        false => None,
    };
    let mois_du_contrat = e.mois_du_contrat()?;
    // Le taux du cachet, tel que le contrat l'imprime : celui de la premiere ligne a l'unite « cachet » ; sans cachet, la
    // valeur ecrite, telle quelle, et rien quand rien n'est ecrit (regle 1 : plus de 130 EUR).
    let premier_cachet = e
        .lignes
        .iter()
        .find(|l| l.get("rubrique").and_then(Value::as_str).and_then(|c| droit::catalogue::rubrique(&ctx.racine, c, &ctx.lue_au).ok()).map_or(false, |r| r.unite == "cachet"));
    let taux_cachet = match premier_cachet {
        Some(l) => nombre_f(socle::num::flottant(l.get("taux").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("taux")))?)?),
        None => brut.get("taux_cachet").or_else(|| data.get("taux_cachet")).cloned().unwrap_or(Value::Null),
    };
    let jours = match brut.get("jours") {
        Some(j) if !j.is_null() => j.clone(),
        _ => Value::from(e.jours),
    };
    let lieu = brut.get("lieu").cloned().unwrap_or_else(|| Value::String(String::new()));
    let prelevement = p.pas.as_ref().map_or(Decimal::new(0, 2), |v| v.prelevement);
    // CE QUE LE MOTEUR A COMPRIS DE LA CARTE, en une ligne : son bulletin (ou qu'il n'y en a pas), son contrat, ses
    // bornes (et la periode de paie d'un contrat a cheval), ce qu'elle paye, le metier, le brut.
    let resolu_dit = {
        let mut m = vec![if e.sans_paie() { "aucun cachet ce mois-ci".to_string() } else { format!("bulletin {}", p.numero_bulletin.clone().unwrap_or_default()) }];
        m.push(format!("contrat {}", e.numero.as_deref().unwrap_or_default()));
        let mut bornes = format!("{} → {}", e.debut.jj_mm_aaaa(), e.fin.as_ref().map(|f| f.jj_mm_aaaa()).unwrap_or_default());
        if e.a_cheval()? {
            bornes.push_str(&format!(" (paie du {} au {})", e.debut_periode()?.jj_mm_aaaa(), e.fin_periode()?.jj_mm_aaaa()));
        }
        m.push(bornes);
        let paye = p.paye_en_bref();
        m.push(if paye.is_empty() { "—".to_string() } else { paye });
        if let Some(emploi) = s.emploi.dite().filter(|x| !x.is_empty()) {
            let cadre = if droit::bareme::est_cadre(s) { " (cadre)" } else { "" };
            m.push(format!("{}{}", emploi.to_lowercase(), cadre));
        }
        m.push(format!("brut {}", socle::texte::euros(p.brut)));
        m.join(" · ")
    };
    // LA LIGNE REPLIEE DU CONTRAT : ses bornes, son numero, et ce qu'il fait par rapport au mois (en cours depuis un
    // mois passe, se poursuit au mois suivant) ; un contrat sans terme se dit par son debut.
    let contrat_dit = {
        let mut t = match &e.fin {
            Some(f) => format!("Contrat du {} au {}", e.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
            None if nat.mensuelle() => format!("Contrat du {}, sans terme", e.debut.jj_mm_aaaa()),
            None => format!("Contrat du {}", e.debut.jj_mm_aaaa()),
        };
        if let Some(n) = &e.numero {
            t.push_str(&format!(" · n° {}", n));
        }
        if e.fin.is_some() {
            if !e.premier_mois()? {
                t.push_str(&format!(" — en cours depuis {}", Mois::en_clair(&e.debut.iso()[..7])));
            }
            if !e.dernier_mois()? {
                t.push_str(&format!(" — se poursuit en {}", Mois::en_clair(&e.debut_mois()?.mois_civil().suivant().to_string())));
            }
        }
        t
    };
    let trop_courte = || socle::fichiers::trop_courte("les mois du contrat");
    Ok(VuePaie {
        resolu_dit,
        contrat_dit,
        paye_dit: p.paye_en_bref(),
        bulletin_dit: p.numero_bulletin.as_ref().map(|n| format!("bulletin {}", n)),
        numero_bulletin: p.numero_bulletin.clone(),
        numero_contrat: e.numero.clone(),
        salarie,
        spectacle,
        nature: NatureVue { cle: nat.cle.clone(), libelle: nat.libelle.clone(), mensuelle: nat.mensuelle() },
        debut: e.debut,
        fin: e.fin,
        contrat: ContratVu {
            premier_mois: e.premier_mois()?,
            dernier_mois: e.dernier_mois()?,
            a_cheval: e.a_cheval()?,
            chevrons: chevrons_dits(e)?,
            sans_paie: e.sans_paie(),
            depuis: mois_du_contrat.first().cloned().ok_or_else(trop_courte)?,
            jusqu_a: mois_du_contrat.last().cloned().ok_or_else(trop_courte)?,
            periode: Periode { debut: e.debut_periode()?, fin: e.fin_periode()? },
            ecrits: champs_du_contrat_ecrit(ctx, e, &nat)?,
            rupture: rupture_de(e, &nat)?,
        },
        dates_travail: e.dates_travail.clone(),
        cachets: e.cachets,
        taux_cachet,
        heures: e.heures,
        // LES CASES « AVANCÉ » QUE CE CONTRAT PAYE AU MOIS APPELLE (`catalogue::CASES_D_UN_MOIS_PAYE`) : ses heures
        // supplementaires a temps plein, complementaires a temps partiel (`engagement::heures_en_plus::faits_appeles`).
        faits_avances: if nat.mensuelle() { engagement::heures_en_plus::faits_appeles(ctx, e.modalite_temps_travail()? == "20")? } else { Vec::new() },
        // LES CASES QU'UNE LIGNE DE FRAIS APPELLE : le lieu d'une nuitée, le restaurant d'un repas, le justificatif d'un
        // frais au-delà de sa limite (`engagement::frais_au_dela`).
        cases: e
            .lignes
            .iter()
            .filter_map(|l| Some((l.get(engagement::RANG)?.as_u64()?, l)))
            .flat_map(|(i, l)| {
                l.get(engagement::CASES).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(move |k| CaseDeFrais {
                    ligne: i,
                    case: k.to_string(),
                    libelle: libelle_de_case(k),
                    aide: l.get(engagement::AIDES).and_then(|a| a.get(k)).cloned().unwrap_or(Value::Null),
                })
            })
            .collect(),
        remuneration: p
            .remuneration
            .iter()
            .map(|l| LigneVue { rubrique: l.code.clone(), designation: l.designation(), quantite: l.quantite, taux: l.taux, montant: q2(l.montant) })
            .collect(),
        jours,
        lieu,
        brut: q2(p.brut),
        net_a_payer: q2(p.net_a_payer),
        // CE QUE LA COMPAGNIE VIRE : la somme effectivement recue par le salarie (C. trav. R. 3243-1, 10°,
        // `atelier/reference/sources/code-du-travail/articles-20261029.md`), le « MONTANT NET À PAYER » du bulletin :
        // le net a payer moins l'impot preleve a la source.
        net_paye: q2(p.net_a_payer - prelevement),
        net_imposable: q2(p.net_imposable),
        cout_employeur: q2(p.cout_employeur),
        avertissements: p.avertissements.iter().filter(|a| !a.info).map(|a| a.texte.clone()).collect(),
        informations: p.avertissements.iter().filter(|a| a.info).map(|a| a.texte.clone()).collect(),
        emploi: EmploiVu { libelle: s.emploi.clone(), code_emploi_ft: s.code_emploi_ft.clone(), statut: s.statut.clone(), cadre: droit::bareme::est_cadre(s) },
        conseil: None,
        a_dire: None,
    })
}

/// LES CHEVRONS D'UN CONTRAT SUR PLUSIEURS MOIS : ses bornes, ce qu'il fait du cote ou il deborde du mois (commence
/// avant, se poursuit apres), et le mois voisin ou mene le clic. `None` du cote ou il ne deborde pas.
pub(crate) fn chevrons_dits(e: &engagement::Engagement) -> R<Chevrons> {
    let clair = |m: &str| Mois::en_clair(m);
    let bornes = match &e.fin {
        Some(f) => format!("Contrat du {} au {}", e.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("Contrat du {}, sans date de fin", e.debut.jj_mm_aaaa()),
    };
    let mois = e.mois_du_contrat()?;
    let premier = e.debut_mois()?;
    let avant = match e.premier_mois()? {
        true => None,
        false => {
            let voisin = premier.plus_jours(-1)?.iso()[..7].to_string();
            Some(format!("{} — commencé en {} · voir {}", bornes, clair(mois.first().map_or("", String::as_str)), clair(&voisin)))
        }
    };
    let apres = (!e.dernier_mois()?).then(|| {
        let suite = match (&e.fin, mois.last()) {
            (Some(_), Some(j)) => format!("se poursuit jusqu'en {}", clair(j)),
            _ => "se poursuit".to_string(),
        };
        format!("{} — {} · voir {}", bornes, suite, clair(&premier.mois_civil().suivant().to_string()))
    });
    Ok(Chevrons { avant, apres })
}

/// Ce qui, dans ce mois, porte sur une
/// fiche archivee. On n'INTERDIT pas : rejouer une paie passee, reimprimer un
/// bulletin perdu, cela doit marcher. Mais une paie NEUVE sur une fiche mise de
/// cote est presque toujours une erreur, et elle ne se verrait nulle part ailleurs.
pub(crate) fn archives_employees(paies: &[Paie]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for p in paies {
        if p.salarie.archivee() {
            out.push(format!(
                "{} : sa fiche est archivee. Pour refaire une paie passee, rien a faire ; pour une nouvelle embauche, la \
                 reactiver (Configuration › Salaries, « Reactiver »).",
                p.salarie.nomme().unwrap_or_else(|| ("Ce salarié").to_string())
            ));
        }
        // Un permanent n'a pas de spectacle -- le numero d'objet lui est interdit
        // (CCH-11) -- donc rien a verifier de ce cote. TRAVAIL A FAIRE : la fiche d'un spectacle se lit par ses cles.
        if vrai(&p.spectacle) && archivee(&p.spectacle) {
            let titre = p.spectacle.get("titre").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("titre")))?;
            out.push(format!(
                "Le spectacle « {} » (objet {}) est archive : pour le jouer a nouveau, le reactiver (Configuration › Spectacles).",
                en_texte(titre),
                texte(p.spectacle.get("numero_objet"))
            ));
        }
    }
    Ok(out)
}

/// Les trois nombres d'une carte (un engagement) ou d'un salarie (la colonne « Le mois »).
#[derive(Debug, Clone, Serialize)]
struct Trois {
    net_paye: Decimal,
    brut: Decimal,
    cout_employeur: Decimal,
    engagements: usize,
}

impl Trois {
    fn de(vues: &[&VuePaie]) -> Trois {
        let somme = |f: fn(&VuePaie) -> Decimal| q2(vues.iter().map(|v| f(v)).fold(Decimal::new(0, 2), |t, x| t + x));
        Trois { net_paye: somme(|v| v.net_paye), brut: somme(|v| v.brut), cout_employeur: somme(|v| v.cout_employeur), engagements: vues.len() }
    }
}

/// La ligne d'un salarie dans la colonne « Le mois » : ses trois nombres, son nom, ses engagements, et au survol le cout
/// et le brut.
#[derive(Debug, Serialize)]
struct ParSalarie {
    #[serde(flatten)]
    trois: Trois,
    salarie: SalarieVu,
    nom: String,
    engagements_dit: String,
    survol_dit: String,
}

/// Un engagement de la saisie qui ne se calcule pas encore : ce qui lui manque, son salarie tel que la saisie l'ecrit,
/// ses bornes comme le moteur les lit de ses lignes de travail (la carte les montre avant tout calcul), les cases du
/// contrat ecrit qu'il appelle deja.
#[derive(Debug, Serialize)]
struct PaieACompleter {
    a_completer: String,
    salarie: CleEcrite,
    debut: String,
    fin: String,
    avertissements: [String; 0],
    informations: [String; 0],
    contrat: EcritsSeuls,
}

/// TRAVAIL A FAIRE (chaine A) : le salarie tel que la saisie l'ecrit.
#[derive(Debug, Serialize)]
struct CleEcrite {
    #[serde(rename = "_cle")]
    cle: Value,
}

#[derive(Debug, Serialize)]
struct EcritsSeuls {
    ecrits: Vec<&'static str>,
}

/// Une paie de l'apercu : calculee, ou a completer.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PaieVue {
    Calculee(Box<VuePaie>),
    ACompleter(PaieACompleter),
}

impl PaieVue {
    fn textes(&self) -> impl Iterator<Item = &String> {
        match self {
            PaieVue::Calculee(v) => Some(v.avertissements.iter().chain(v.informations.iter())),
            PaieVue::ACompleter(_) => None,
        }
        .into_iter()
        .flatten()
    }
}

/// Une paie de l'apercu, et l'identite de son engagement (`None` : une carte pas encore ecrite) : la carte retrouve la
/// sienne.
#[derive(Debug, Serialize)]
struct PaieDeLApercu {
    #[serde(flatten)]
    paie: PaieVue,
    engagement: Option<Identite>,
}

/// Les paies de la saisie, et ce que le mois en compte.
struct Calcul {
    phrase_du_mois: Option<String>,
    mois: Mois,
    libelle: String,
    paies: Vec<PaieVue>,
    total_cout: Decimal,
    total_net: Decimal,
    blocs: Vec<Option<Trois>>,
    par_salarie: Vec<ParSalarie>,
    avertissements: Vec<String>,
    informations: Vec<String>,
}

/// L'APERCU (`apercu`) : le mois tel que l'ecran le montre, rien d'ecrit.
#[derive(Debug, Serialize)]
struct Apercu {
    phrase: String,
    phrase_du_mois: Option<String>,
    mois: Mois,
    libelle: String,
    paies: Vec<PaieDeLApercu>,
    total_cout: Decimal,
    total_net: Decimal,
    blocs: Vec<Option<Trois>>,
    par_salarie: Vec<ParSalarie>,
    avertissements: Vec<String>,
    informations: Vec<String>,
    /// Ce que le moteur dit de chaque engagement (les encarts de sa carte), en phrases : le mois, puis ses paies.
    phrases: Vec<String>,
}

/// `apercu` : le mois tel que l'ecran le montre, rien d'ecrit -- ni fichier, ni compteur.
///
/// Un engagement valide montre la paie qu'il a ecrite, avec ses numeros du moment ; s'il s'est recalcule de lui-meme,
/// ses informations disent quand et pourquoi (`etats::phrase_du_recalcul`). Un engagement en saisie se calcule, et
/// porte ses conseils de saisie.
/// L'APERCU D'UNE SAISIE DONT DES ENGAGEMENTS NE SE CALCULENT PAS ENCORE (`manques_de_la_saisie`) : les autres se
/// calculent, et chacun de ceux-la tient sa place dans `paies` avec ce qui lui manque (`a_completer`), sans montant ;
/// la carte qui en porte un n'a pas de total. Une saisie dont aucun refus ne se rapporte a un engagement refuse.
fn apercu(ctx: &Contexte, c: ChargeApercu) -> R<Apercu> {
    let mois = c.mois.to_string();
    let (data, _, identites) = mois_designe(ctx, &mois, &c.cartes)?;
    // Une carte neuve dans un mois ferme aux engagements neufs ne se compte pas : son ecriture serait refusee.
    if identites.iter().any(Option::is_none) {
        refuser_si_importe(ctx, &mois)?;
    }
    let r = apercu_des_paies(ctx, &data)?;
    let mut paies = Vec::new();
    for (mut paie, engagement) in r.paies.into_iter().zip(identites) {
        if let PaieVue::Calculee(v) = &mut paie {
            dire_les_tarifs(ctx, &mois, v)?;
        }
        paies.push(PaieDeLApercu { paie, engagement });
    }
    let phrase = r.phrase_du_mois.clone().unwrap_or_else(|| format!("{} : {}.", r.libelle, super::compte(paies.len(), "paie", "paies")));
    let mut phrases: Vec<String> = Vec::new();
    for t in r.avertissements.iter().chain(r.informations.iter()).chain(paies.iter().flat_map(|p| p.paie.textes())) {
        if !phrases.contains(t) {
            phrases.push(t.clone());
        }
    }
    Ok(Apercu {
        phrase,
        phrase_du_mois: r.phrase_du_mois,
        mois: r.mois,
        libelle: r.libelle,
        paies,
        total_cout: r.total_cout,
        total_net: r.total_net,
        blocs: r.blocs,
        par_salarie: r.par_salarie,
        avertissements: r.avertissements,
        informations: r.informations,
        phrases,
    })
}

/// LE MONTANT D'UN REPAS OU D'UNE NUITEE DIT D'OU IL VIENT : une ligne au tarif de la convention (`engagement::tarif_conventionnel`,
/// le barème de l'année, couche de la convention de la compagnie) le dit dans les informations de sa paie. Le tarif
/// est daté : il se lit au premier jour du mois, au premier et au dernier jour de la paie dans le mois.
fn dire_les_tarifs(ctx: &Contexte, mois: &str, p: &mut VuePaie) -> R<()> {
    let Ok(premier) = Jour::lire(&format!("{}-01", mois)) else { return Ok(()) };
    let mut jours = vec![premier];
    for j in std::iter::once(p.debut).chain(p.fin) {
        if j.iso().starts_with(mois) && !jours.contains(&j) {
            jours.push(j);
        }
    }
    let mut dits: Vec<String> = Vec::new();
    for l in &p.remuneration {
        let taux = l.taux.to_string().parse::<f64>().ok();
        let mut tarif = None;
        for j in &jours {
            match engagement::tarif_conventionnel(ctx, &l.rubrique, j).map_err(Echec::from)? {
                Some(t) if taux.is_some_and(|x| Some(x) == t.as_f64()) => {
                    tarif = Some(t);
                    break;
                }
                _ => {}
            }
        }
        if let Some(tarif) = tarif {
            let dit = format!("{} à {}", l.designation.to_lowercase(), socle::texte::euros_de(&tarif));
            if !dits.contains(&dit) {
                dits.push(dit);
            }
        }
    }
    if dits.is_empty() {
        return Ok(());
    }
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let qui = format!("{} {}", p.salarie.nom.texte(), p.salarie.prenom.texte()).trim().to_string();
    let quoi = dits.join(", ");
    let quoi = if qui.is_empty() { quoi[..1].to_uppercase() + &quoi[1..] } else { format!("{} : {}", qui, quoi) };
    p.informations.push(format!("{}, le tarif de la convention collective {}.", quoi, idcc));
    Ok(())
}

/// UN CONTRAT COMMENCE AVANT LE LOGICIEL, SUR SA CARTE PAS ENCORE VALIDEE (`engagement::stock::commence_avant_le_logiciel`) :
/// la phrase qui dit ce qui se passe (ni DPAE ni contrat ici ; le numero sous lequel la DSN le declare, et qu'un numero
/// deja declare par l'ancien logiciel se dit dans sa case, `NUMERO_DIT`), et, pour un contrat paye au mois dont la fiche
/// ne dit pas ses conges a la reprise, la ligne de « Pour démarrer » qui y mene (`a_dire`,
/// `declarations::demarrage::conges_a_dire`) : le compteur de conges part du solde de l'ancien bulletin (L3141-3,
/// `engagement::conges::reprise_conges`). `brut` : l'engagement tel que la saisie l'ecrit ; `v` : sa vue (`vue_paie`).
fn dire_l_entree(ctx: &Contexte, mois: &str, brut: &Value, v: &mut VuePaie) -> R<()> {
    let debut = v.debut.iso();
    if engagement::etats::est_validee(brut) || !engagement::stock::commence_avant_le_logiciel(&ctx.racine, &debut, mois).map_err(Echec::from)? {
        return Ok(());
    }
    let qui = format!("{} {}", v.salarie.nom.texte(), v.salarie.prenom.texte()).trim().to_string();
    let le = v.debut.jj_mm_aaaa();
    let numero = v.numero_contrat.as_ref().map_or("", |n| n.as_str());
    let phrase = if brut.get("numero").is_some_and(vrai) {
        format!("{} : contrat commencé le {}, avant PuppetPaie : ni DPAE ni contrat à faire ici. La DSN le déclare sous le n° {}, celui que votre ancien logiciel a déclaré.", qui, le, numero)
    } else {
        format!(
            "{} : contrat commencé le {}, avant PuppetPaie : ni DPAE ni contrat à faire ici. La DSN le déclare sous le n° {} ; si votre ancien logiciel l'a déjà déclaré en DSN, dites son numéro (« Numéro en DSN »), sinon il compterait pour un second contrat.",
            qui, le, numero
        )
    };
    v.informations.insert(0, phrase);
    if v.nature.mensuelle && !declarations::demarrage::conges_repris(ctx, &v.salarie.cle) {
        v.a_dire = Some([declarations::demarrage::conges_a_dire(&v.salarie.cle, &qui, &format!("{}-01", mois))]);
    }
    Ok(())
}

/// Les paies de l'apercu de `data`, une par engagement, dans son ordre.
fn apercu_des_paies(ctx: &Contexte, data: &Value) -> R<Calcul> {
    let refus = match apercu_de(ctx, data) {
        Ok(c) => return Ok(c),
        Err(refus) => refus,
    };
    let manques = manques_de_la_saisie(ctx, data);
    if manques.iter().all(Option::is_none) {
        return Err(refus);
    }
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut complete = data.clone();
    complete["engagements"] = Value::Array(tous.iter().zip(&manques).filter(|(_, m)| m.is_none()).map(|(e, _)| e.clone()).collect());
    let mut r = apercu_de(ctx, &complete)?;
    let mut calculees = std::mem::take(&mut r.paies).into_iter();
    let mut calcules = std::mem::take(&mut r.blocs).into_iter();
    let mois = valeur_en_texte(&data["mois"]);
    for (e, m) in tous.iter().zip(&manques) {
        match m {
            None => {
                r.paies.extend(calculees.next());
                r.blocs.push(calcules.next().flatten());
            }
            Some(m) => {
                let (debut, fin) = e.as_object().map(|o| bornes_brutes(ctx, o)).unwrap_or_default();
                r.paies.push(PaieVue::ACompleter(PaieACompleter {
                    a_completer: m.clone(),
                    salarie: CleEcrite { cle: e.get("salarie").cloned().unwrap_or(Value::Null) },
                    debut,
                    fin,
                    avertissements: [],
                    informations: [],
                    contrat: EcritsSeuls { ecrits: champs_du_contrat_ecrit_en_saisie(ctx, e, &mois) },
                }));
                // Une carte a completer n'a pas de total.
                r.blocs.push(None);
            }
        }
    }
    r.phrase_du_mois = phrase_du_mois(&r.paies, r.par_salarie.is_empty());
    Ok(r)
}

/// CE QUE LES CHIFFRES DU MOIS NE COMPTENT PAS, en une phrase sous eux (`None` : rien) : un engagement a completer ne
/// se calcule pas, mais il est la ; un mois sans aucun engagement se dit aussi.
fn phrase_du_mois(paies: &[PaieVue], aucun_chiffre: bool) -> Option<String> {
    let attendent = paies.iter().filter(|v| matches!(v, PaieVue::ACompleter(_))).count();
    match (attendent, aucun_chiffre) {
        (0, true) => Some("Aucun engagement ce mois-ci.".to_string()),
        (0, false) => None,
        (1, vide) => Some(format!("1 engagement à compléter{} : il compte quand il est complet.", if vide { "" } else { ", pas encore compté" })),
        (k, vide) => Some(format!("{} engagements à compléter{} : ils comptent quand ils sont complets.", k, if vide { "" } else { ", pas encore comptés" })),
    }
}

/// L'apercu de la saisie `data` : chaque engagement se calcule. TRAVAIL A FAIRE (chaine A) : la saisie se lit par ses
/// cles.
fn apercu_de(ctx: &Contexte, data: &Value) -> R<Calcul> {
    let mois = data.get("mois").map(en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
    let lu = Mois::lire(&mois)?;
    let montrees = paies_de_l_ecran(ctx, data, &mois)?;
    let mut paies: Vec<Paie> = montrees.iter().map(|m| m.paie.clone()).collect();
    documents::ecrire_documents::numeroter(ctx, &mut paies, lu.annee(), &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let tous = data.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(vue_paie(ctx, p, tous.get(i).unwrap_or(&Value::Null), data)?);
    }
    // LES CONSEILS DE SAISIE : au plus un par carte, quand l'option de la compagnie est cochee, pour un
    // engagement en saisie seulement. Ils ne vivent qu'ici, dans la vue de l'ecran : ni dans les avertissements de la
    // paie, ni au bulletin, ni a `produire`.
    for (i, (v, c)) in vues.iter_mut().zip(crate::compagnie::conseils::de_saisie(ctx, &paies)?).enumerate() {
        if !tous.get(i).is_some_and(engagement::etats::est_validee) {
            v.conseil = c;
        }
    }
    for (i, v) in vues.iter_mut().enumerate() {
        dire_l_entree(ctx, &mois, tous.get(i).unwrap_or(&Value::Null), v)?;
    }
    for (v, m) in vues.iter_mut().zip(&montrees) {
        if let Some(phrase) = m.recalcul.as_ref().and_then(engagement::etats::phrase_du_recalcul) {
            v.informations.insert(0, phrase);
        }
    }
    let total_cout = q2(vues.iter().fold(Decimal::new(0, 2), |t, v| t + v.cout_employeur));
    let total_net = q2(vues.iter().fold(Decimal::new(0, 2), |t, v| t + v.net_paye));
    // LES TROIS NOMBRES, sommes ici : par carte (une carte, un engagement) et par salarie (la colonne « Le mois »).
    let blocs = vues.iter().map(|v| Some(Trois::de(&[v]))).collect();
    let mut salaries: Vec<(String, String, Vec<&VuePaie>)> = Vec::new();
    for (v, p) in vues.iter().zip(&paies) {
        let cle = format!("{}|{}", en_texte(&v.salarie.nom.en_valeur()), en_texte(&v.salarie.prenom.en_valeur()));
        match salaries.iter_mut().find(|(c, _, _)| *c == cle) {
            Some((_, _, vs)) => vs.push(v),
            None => salaries.push((cle, super::nom_a_l_ecran(&p.salarie), vec![v])),
        }
    }
    let par_salarie = salaries
        .iter()
        .map(|(_, nom, vs)| {
            let trois = Trois::de(vs);
            ParSalarie {
                engagements_dit: super::compte(vs.len(), "engagement", "engagements"),
                survol_dit: format!("coût employeur {} · brut {}", socle::texte::euros(trois.cout_employeur), socle::texte::euros(trois.brut)),
                salarie: vs[0].salarie.clone(),
                nom: nom.clone(),
                trois,
            }
        })
        .collect::<Vec<_>>();
    let mut avertissements = archives_employees(&paies)?;
    let mut informations = Vec::new();
    for v in &vues {
        avertissements.extend(v.avertissements.iter().cloned());
        informations.extend(v.informations.iter().cloned());
    }
    let paies: Vec<PaieVue> = vues.into_iter().map(|v| PaieVue::Calculee(Box::new(v))).collect();
    Ok(Calcul {
        phrase_du_mois: phrase_du_mois(&paies, par_salarie.is_empty()),
        mois: lu,
        libelle: lu.libelle(socle::mois::Graphie::Titre),
        paies,
        total_cout,
        total_net,
        blocs,
        par_salarie,
        avertissements,
        informations,
    })
}
