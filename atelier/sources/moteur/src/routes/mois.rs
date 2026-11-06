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
//! saisie se calcule en memoire, tel que son brouillon est enregistre (`paies_de_l_ecran`) ; une modification pas encore
//! ecrite d'un engagement valide se calcule de ses changements (`mois_designe`) ; les numeros se reservent sans rien
//! ecrire.
//!
//! Chaque charge est une structure (`ChargeCase`, `ChargeApercu`, `ChargeAjuster`, `ChargeContratsPasses`,
//! `ChargeEmploi`, et `etapes::ChargeMois`), lue une fois (`routes::lire_charge`) ; chaque reponse en est une aussi.
//!
//! La carte d'un engagement et la vue du mois sont `engagement::saisie::{Carte, VueDuMois}`. TRAVAIL A FAIRE (chaine A,
//! le domaine de l'engagement) : les lignes d'une carte et le spectacle d'une paie se lisent encore en
//! `serde_json::Value`, par leurs cles ; de meme l'engagement importe (`engagement::importe` : son contrat, ses paies), les cartes importees
//! (`import::mois::cartes`), le conseil de saisie (`compagnie::conseils`), la ligne « Pour démarrer »
//! (`declarations::demarrage::conges_a_dire`) et la fiche d'un spectacle (`Contexte::spectacles`). Ils se declareront avec
//! leur domaine ; ici, ils passent tels quels.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::routes::fiches::{maintenant, refus};
use crate::routes::{en_json, lire_charge};
use crate::routes::etapes::ChargeMois;
use engagement::calcul::Paie;
use engagement::carte::{Brouillon, Changements, NatureDeContrat};
use engagement::saisie::{Carte as SaisieCarte, VueDuMois};
use engagement::stock::Identite;
use engagement::viser::Cible;
use droit::catalogue;
use droit::contexte::{Contexte, IdSalarie};
use droit::emplois;
use engagement::{self, LignePaie};
use socle::date::Jour;
use documents::valeur::en_texte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::repr;
use socle::mois::Mois;
use socle::num::Decimal;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::engagement::SAISIE_INCOMPLETE;
use crate::routes::catalogue::{Ecran, Place};

const MODIFICATIONS: Champ = Champ {
    nom: "modifications", libelle: Affiche::new("Modifications en cours"), forme: Forme::Objet,
    sens: "les engagements validés ouverts pour être modifiés, pas encore écrits : [{engagement (son identité), changements (comme modifier-engagement)}] ; [] sans aucun. Une carte en saisie se lit telle qu'elle est enregistrée",
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
        geste: "Calculer sans rien écrire les paies du mois, avec les modifications en cours.",
        charge: &[MOIS, MODIFICATIONS],
        exemple: r#"{"mois":"2026-11","modifications":[]}"#,
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
        charge: &[MOIS, Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé d'un salarié, pour ne lire que les siens ; null : tous", avance: true }],
        exemple: r#"{"mois":"2026-11","salarie":null}"#,
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
            Champ { nom: "cadre", libelle: Affiche::new("Cadre"), forme: Forme::OuiNon, sens: "cadre ou non ; null : le défaut du métier au catalogue", avance: true },
            Champ { nom: "feminin", libelle: Affiche::new("Au féminin"), forme: Forme::OuiNon, sens: "le libellé au féminin (vrai), ou selon la fiche (faux)", avance: true },
            Champ { nom: "salarie", libelle: Affiche::new("Fiche"), forme: Forme::Objet, sens: "ce que la fiche remplie dit du sexe : {nir, sexe} ; null : rien", avance: true },
        ],
        exemple: r#"{"metier":"COM010","cadre":null,"feminin":false,"salarie":null}"#,
        rend: "les champs de la fiche que le métier remplit (emploi, statut, catégorie…)",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::EnregistrerSaisie,
        geste: "Changer des cases du brouillon d'une carte, et l'écrire.",
        charge: &[
            MOIS,
            Champ { nom: "carte", libelle: Affiche::new("Carte"), forme: Forme::Objet, sens: "{\"neuve\": sa nature (`nouvelle_carte` de la route etat)} pour une carte neuve, ou {\"ecrite\": son identité, celle que le moteur a rendue}", avance: false },
            super::engagement::CHANGEMENTS,
        ],
        exemple: r#"{"mois":"2026-11","carte":{"neuve":"cddu_spectacle"},"changements":[{"case":"salarie","valeur":"FULLER"},{"case":"spectacle","valeur":"le-banc-des-menteurs"},{"case":"lignes","valeur":[{"rubrique":"cachet","quantite":1,"taux":150,"lieu":null,"dates":["2026-11-20"],"planning":[]}]}]}"#,
        rend: "engagement (l'identité écrite, à passer à valider-engagement), brouillon (chacune des cases de sa nature, null tant qu'elle n'est pas remplie), a_completer (ce qui lui manque pour être validé, ou null), enregistre_a",
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
            MODIFICATIONS,
            super::engagement::IDENTITE,
            Champ { nom: "cible", libelle: Affiche::new("Net, brut ou coût"), forme: Forme::Texte, sens: "net, brut ou cout", avance: false },
            Champ { nom: "montant", libelle: Affiche::new("Montant voulu"), forme: Forme::Nombre, sens: "le montant voulu, en euros", avance: false },
        ],
        exemple: r#"{"mois":"2026-11","modifications":[],"engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"cible":"brut","montant":200}"#,
        rend: "phrase (ce qui est atteint, et le taux qu'il faudrait), vise, atteint, ecart, engagements (les taux qui changent), paies ; rien n'est écrit",
        refus: &[OPERATION, FORME, SAISIE_INCOMPLETE],
        acte: Acte::Lit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les charges

/// UNE MODIFICATION PAS ENCORE ECRITE D'UN ENGAGEMENT VALIDE (`apercu`, `ajuster`) : la carte ouverte de l'engagement
/// `engagement` (son identite), et ce qu'on y a change, comme `modifier-engagement` l'ecrira (`changements`). Elle ne
/// s'ecrit qu'a « Modifier l'engagement » : l'apercu et le montant vise la calculent en memoire.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModificationEnCours {
    pub(crate) engagement: Identite,
    pub(crate) changements: Changements,
}

/// LA CARTE QU'UN CHANGEMENT VISE : une carte neuve, nee dans sa nature (`nouvelle_carte` de la route etat), ou celle
/// d'un engagement ecrit, par son identite.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CarteVisee {
    Neuve(NatureDeContrat),
    Ecrite(Identite),
}

/// LA CHARGE DES CHANGEMENTS D'UNE CARTE (`enregistrer-saisie`) : le mois de l'ecran, la carte visee, et ses changements,
/// dans l'ordre.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeCase {
    pub(crate) mois: Mois,
    pub(crate) carte: CarteVisee,
    pub(crate) changements: Changements,
}

/// La charge de l'apercu : le mois, et les modifications pas encore ecrites d'engagements valides (`mois_designe`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeApercu {
    mois: Mois,
    modifications: Vec<ModificationEnCours>,
}

/// La charge du montant vise : le mois et ses modifications pas encore ecrites, l'engagement a ajuster (son identite),
/// ce qui se vise et combien.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAjuster {
    mois: Mois,
    modifications: Vec<ModificationEnCours>,
    engagement: Identite,
    cible: Cible,
    montant: f64,
}

/// La charge des contrats d'un mois passe : le mois, et le salarie dont ne lire que les siens (absent : tous).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeContratsPasses {
    mois: Mois,
    salarie: Option<IdSalarie>,
}

/// La charge du metier : son code, et ce qui choisit sa forme (cadre, ou non ; au feminin, dit tel quel ou lu de la
/// fiche que l'ecran remplit).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEmploi {
    metier: String,
    cadre: Option<bool>,
    feminin: bool,
    salarie: Option<SexeDeLaFiche>,
}

/// Ce qui dit le sexe, sur la fiche que l'ecran remplit : le NIR, sinon la case `sexe` (`ntt::sexe_de`), chacune telle
/// que la fiche l'ecrit (vide quand elle n'est pas remplie).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SexeDeLaFiche {
    nir: String,
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
    /// La vue du mois (`engagement::stock::vue_du_mois`), les noms resolus (`resoudre_noms`).
    saisie: VueDuMois,
    /// L'identite de chaque engagement de `saisie.engagements`, dans son ordre : ce par quoi sa carte le designe.
    identites: Vec<Identite>,
}

/// Une carte ecrite (`enregistrer-saisie`).
#[derive(Debug, Serialize)]
struct CarteEcrite {
    phrase: String,
    engagement: Identite,
    /// Le brouillon tel qu'il est ecrit, chacune de ses cases (`null` : pas encore remplie).
    brouillon: Brouillon,
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
    taux_cachet: Option<f64>,
    deja_repris: bool,
    engagement: Reprise,
}

/// L'ENGAGEMENT D'UN CONTRAT EN COURS, pour le fichier du mois : le contrat tel qu'il a ete signe, et une ligne de
/// cachets a dater.
#[derive(Debug, Serialize)]
struct Reprise {
    nature: &'static str,
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
    rubrique: String,
    taux_avant: Option<Decimal>,
    taux: Option<Decimal>,
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
        Route::Emploi => en_json(emploi(ctx, lire_charge(charge)?)?),
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


// ============================================================ la saisie

/// Un mois neuf s'ouvre VIDE, avec le
/// dernier taux saisi -- `null` quand le dossier n'a encore aucun cachet (R12 :
/// aucun texte ne fixe un taux de cachet, la compagnie le saisit).
pub(crate) fn squelette(ctx: &Contexte, mois: &str) -> R<VueDuMois> {
    let mois = Mois::lire(mois).map_err(|m| erreur("ValueError", m))?;
    Ok(VueDuMois { taux_cachet: dernier_taux(ctx)?.and_then(socle::forme::NombreLu::de_f64), ..VueDuMois::vide(mois) })
}


/// Le taux du cachet le plus recemment saisi : une ligne de cachet d'abord, le `taux_cachet` du mois a defaut ; `None` si
/// aucun mois n'en dit rien -- aucun texte ne fixe un taux de cachet (regle 1, R12), la compagnie saisit le sien.
pub(crate) fn dernier_taux(ctx: &Contexte) -> R<Option<f64>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?;
    mois.reverse();
    for m in mois {
        let Ok(Some(data)) = engagement::stock::vue_du_mois(&ctx.racine, &m.to_string()) else { continue };
        if let Some(t) = taux_du_mois(ctx, &data) {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

/// Le taux du dernier cachet de la saisie `data`, sinon son `taux_cachet` (un taux nul ne dit rien).
fn taux_du_mois(ctx: &Contexte, data: &VueDuMois) -> Option<f64> {
    for e in data.engagements.iter().rev() {
        for l in e.lignes().iter().rev() {
            if let Some(t) = taux_d_une_ligne(ctx, l) {
                return Some(t);
            }
        }
    }
    data.taux_cachet.as_ref().and_then(socle::forme::NombreLu::as_f64).filter(|t| *t != 0.0)
}

/// Le taux d'une ligne de cachet (le `try` de `dernier_taux`) : tout ce qui
/// leve est une ligne qui ne dit rien.
fn taux_d_une_ligne(ctx: &Contexte, l: &LignePaie) -> Option<f64> {
    if droit::catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au).ok()?.unite != "cachet" {
        return None;
    }
    l.taux().filter(|t| !t.is_zero()).map(socle::num::flottant)
}

/// Les lignes d'un permanent, calculees. Une saisie qui ne se charge pas se rend telle quelle.
fn resoudre_noms(ctx: &Contexte, saisie: VueDuMois) -> R<VueDuMois> {
    let Ok(paies) = engagement::lue_sans_paie(ctx, &saisie) else { return Ok(saisie) };
    let mut engagements = Vec::new();
    for (e, p) in saisie.engagements.iter().zip(&paies) {
        let mut resolu = e.clone();
        // Les lignes d'un permanent sont CALCULEES, pas saisies.
        if !p.engagement.nat()?.mensuelle() {
            resolu.lignes = Some(lignes_affichees(p, e.lignes())?);
        }
        engagements.push(resolu);
    }
    Ok(VueDuMois { engagements, ..saisie })
}

/// Ce que dit, a l'ecran, la case d'une ligne de frais (`engagement::ligne::Marques::cases`).
fn libelle_de_case(case: engagement::CaseDeFrais) -> &'static str {
    match case {
        engagement::CaseDeFrais::Paris => "Paris ou petite couronne",
        engagement::CaseDeFrais::Restaurant => "au restaurant",
        engagement::CaseDeFrais::Loge => "hôtel payé par la compagnie",
        engagement::CaseDeFrais::Justificatif => "justificatif conservé",
    }
}

/// LES LIGNES DE PAIE TELLES QUE LE MOTEUR LES A COMPRISES. `remuneration` suit les lignes du fichier une a une
/// (`calcul::lignes_brut`) ; la part d'un frais au-dela de sa limite, que le moteur pose apres elles
/// (`engagement::frais_au_dela`), ne se montre pas en ligne de saisie, et la ligne garde le taux saisi
/// (`ligne::Marques::taux_saisi`). Une ligne en unites ou de frais prend la quantite et le taux que le calcul a lus (le
/// tarif de la convention d'un repas qui ne dit pas le sien) ; le reste de la ligne du fichier se garde (ses heures, ses
/// cases de frais, ses jours et son lieu ; une ligne en pourcentage, au montant ou en avantage en nature telle quelle) :
/// la page le relit pour le remettre dans ses champs.
fn lignes_affichees(p: &engagement::calcul::Paie, brutes: &[LignePaie]) -> R<Vec<LignePaie>> {
    let mut out = Vec::new();
    for (i, (l, src)) in p.remuneration.iter().zip(brutes).enumerate() {
        let saisi = p.engagement.lignes.get(i).and_then(|x| x.marques()).and_then(|m| m.taux_saisi);
        // Le nombre que l'ecran montre : sa forme flottante la plus courte.
        let nombre = |d: socle::num::Decimal| socle::num::dec_f64(socle::num::flottant(d)).map_err(Echec::from);
        let (quantite, taux) = (Some(nombre(l.quantite)?), Some(match saisi {
            Some(t) => t,
            None => nombre(l.taux)?,
        }));
        out.push(match src.clone() {
            LignePaie::Unites(x) => LignePaie::Unites(engagement::ligne::Unites { quantite, taux, ..x }),
            LignePaie::UnitesEtHeures(x) => LignePaie::UnitesEtHeures(engagement::ligne::UnitesEtHeures { quantite, taux, ..x }),
            LignePaie::Frais(x) => LignePaie::Frais(engagement::ligne::Frais { quantite, taux, ..x }),
            autre => autre,
        });
    }
    Ok(out)
}

/// Ce que la page propose quand la saisie ne le dit pas : `taux_cachet`, le taux d'une ligne de cachet neuve, celui
/// du dernier cachet de CE mois, sinon son `taux_cachet`, sinon le dernier taux saisi du dossier (`dernier_taux`),
/// sinon rien (`null`).
fn defauts_de_la_saisie(ctx: &Contexte, saisie: &VueDuMois) -> Defauts {
    // Un dossier qu'on ne sait pas relire ne propose rien (`null`) : la route rend la saisie telle quelle, et
    // c'est l'apercu qui dira pourquoi.
    Defauts { taux_cachet: taux_du_mois(ctx, saisie).or_else(|| dernier_taux(ctx).ok().flatten()) }
}

/// La route `saisie`.
fn saisie(ctx: &Contexte, mois: Mois) -> R<SaisieDuMois> {
    let texte_mois = mois.to_string();
    let existe = engagement::stock::a_une_saisie(&ctx.racine, &texte_mois).map_err(Echec::from)?;
    // Un mois neuf s'ouvre VIDE, avec le dernier taux saisi (`squelette`) ; un mois que des engagements touchent, tel qu'ils
    // le montrent.
    let saisie = match engagement::stock::vue_du_mois(&ctx.racine, &texte_mois).map_err(Echec::from)? {
        Some(vue) => vue,
        None => squelette(ctx, &texte_mois)?,
    };
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
        saisie: resoudre_noms(ctx, saisie)?,
        identites: engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, &texte_mois).map_err(Echec::from)?),
    })
}
// ------------------------------------------------------- les jours d'un engagement

/// Les jours travailles d'un engagement brut, AAAA-MM-JJ, tries : ceux de ses lignes de travail
/// (`engagement::dates_engagement`).
fn jours_joues(e: &SaisieCarte) -> Vec<String> {
    let mut jours: Vec<String> = e.dates().iter().map(|j| j.jour().to_string()).collect();
    jours.sort();
    jours
}

/// (debut, fin) d'une carte, AAAA-MM-JJ, comme `Engagement` les deduirait : son debut (`Carte::debut_lu`), sa fin
/// ecrite, sinon le dernier de ses jours travailles et de son planning ; vides sans aucun jour.
pub(crate) fn bornes_brutes(e: &SaisieCarte) -> (String, String) {
    let fin = match e.fin.as_deref().filter(|x| !x.is_empty()) {
        Some(f) => f.to_string(),
        None => e.dates().iter().chain(e.planning().iter()).map(|j| j.jour().to_string()).max().unwrap_or_default(),
    };
    (e.debut_lu(), fin)
}

// ------------------------------------------------------------ les fichiers de mois

/// Les engagements que le mois montre (`engagement::stock::vue_du_mois`), avec la cle du salarie ; vide si aucun
/// engagement ne le touche. Une carte qui ne nomme pas son salarie est sautee.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: &str) -> R<Vec<(IdSalarie, SaisieCarte)>> {
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Ok(Vec::new());
    };
    Ok(data.engagements.into_iter().filter_map(|e| Some((e.salarie.clone()?, e))).collect())
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
fn contrats_en_cours(ctx: &Contexte, mois: &str) -> R<Vec<EnCours>> {
    let mut out: Vec<EnCours> = Vec::new();
    let mut vus: HashSet<(String, String, String)> = HashSet::new();
    for i in engagement::importe::qui_continuent(&ctx.racine, mois).map_err(Echec::from)? {
        let b = i.objet().carte();
        let t = |x: &Option<String>| x.clone().unwrap_or_default();
        let Some(salarie) = b.salarie.as_ref().map(ToString::to_string) else { continue };
        let (debut, fin, numero) = (t(&b.debut), t(&b.fin), t(&b.numero));
        if !vus.insert((salarie.clone(), debut.clone(), numero.clone())) {
            continue;
        }
        let Ok(fiche) = ctx.salarie(&salarie) else { continue };
        let derniere = i.paies().values().last();
        let plein = |o: Option<&String>| o.map(|o| o.trim().to_string()).filter(|o| !o.is_empty());
        let numero_objet = plein(i.contrat().and_then(|c| c.numero_objet.as_ref())).or_else(|| plein(derniere.and_then(|p| p.numero_objet.as_ref()))).unwrap_or_default();
        let spectacle = ctx
            .spectacles
            .iter()
            .find(|(_, s)| !numero_objet.is_empty() && s.numero_objet.portee().is_some_and(|n| n.eq_ignore_ascii_case(&numero_objet)))
            .map(|(k, _)| k.to_string());
        let taux_cachet = derniere
            .into_iter()
            .flat_map(|p| p.lignes.iter())
            .find(|l| l.rubrique.as_deref() == Some("cachet"))
            .and_then(|l| l.taux)
            .and_then(|x| x.to_string().parse::<f64>().ok());
        let taux = match taux_cachet.filter(|t| *t != 0.0) {
            Some(t) => Some(t),
            None => dernier_taux(ctx).ok().flatten(),
        };
        let engagement = Reprise {
            // Un contrat qui continue ici est un contrat d'usage (`importe::Importe::continue_apres` : jamais un permanent).
            nature: engagement::carte::CDDU,
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
            taux_cachet,
            deja_repris: false,
            engagement,
        });
    }
    let mut deja: HashSet<(IdSalarie, String, String)> = HashSet::new();
    for (c, e) in engagements_du_mois(ctx, mois)? {
        deja.insert((c, bornes_brutes(&e).0, e.numero.clone().unwrap_or_default()));
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
/// Les spectacles retrouves par leur cle ou par leur numero d'objet.
struct Spectacles<'a> {
    ctx: &'a Contexte,
    par_objet: HashMap<String, &'a droit::fiche::ChampsSpectacle>,
}

impl<'a> Spectacles<'a> {
    fn nouveau(ctx: &'a Contexte) -> Spectacles<'a> {
        let mut par_objet = HashMap::new();
        for v in ctx.spectacles.values() {
            if let Some(n) = v.numero_objet.remplie() {
                par_objet.entry(n.to_string()).or_insert(v);
            }
        }
        Spectacles { ctx, par_objet }
    }

    /// Le spectacle de cle `x`, sinon de numero d'objet `x`.
    fn trouver(&self, x: &str) -> Option<&'a droit::fiche::ChampsSpectacle> {
        self.ctx.spectacles.get(x).or_else(|| self.par_objet.get(x).copied())
    }
}

/// Le titre de la fiche, ou le numero d'objet a defaut.
fn titre_de(spec: Option<&droit::fiche::ChampsSpectacle>, numero: &str) -> String {
    let t = spec.and_then(|s| s.titre.remplie()).unwrap_or_default().trim();
    if !t.is_empty() && t != TITRE_A_COMPLETER { t.to_string() } else { numero.to_string() }
}

/// « NOM Prénom » d'une clé de salarié (la clé elle-même si elle est inconnue).
fn nom_de(ctx: &Contexte, cle: &str) -> String {
    ctx.salaries.get(cle).and_then(|s| s.nomme()).unwrap_or_else(|| cle.to_string())
}

/// Le nombre de cachets d'un engagement : la somme des quantites de ses lignes dont la rubrique est un cachet (une case
/// vide n'en compte aucun).
fn cachets_de(ctx: &Contexte, e: &SaisieCarte) -> R<i64> {
    let mut cachets = 0;
    for l in e.lignes() {
        if catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au).map_err(Echec::from)?.unite != "cachet" {
            continue;
        }
        if let Some(q) = l.quantite() {
            cachets += i64::try_from(q.trunc()).map_err(|_| erreur("OverflowError", socle::fichiers::entier_hors_limites(&q.to_string())))?;
        }
    }
    Ok(cachets)
}

/// Ce qu'une ligne de la liste dit d'un contrat passe : ses bornes, ses cachets et ses heures.
fn dit_du_contrat(debut: &str, fin: &str, cachets: i64, heures: i64) -> String {
    let court = |iso: &str| Jour::lire(iso).map(|j| j.jj_mm()).unwrap_or_else(|_| iso.to_string());
    let h = if heures > 0 { format!(" + {} h", heures) } else { String::new() };
    format!("{}→{} · {}{}", court(debut), court(fin), super::compte(cachets.max(0) as usize, "cachet", "cachets"), h)
}

/// UN CONTRAT QU'UN ENGAGEMENT IMPORTE ATTESTE (`engagement::importe::Importe::contrat_lu`) ; celui d'un permanent, mois
/// par mois : chaque paie, aux jours qu'elle paie et avec ce que son bulletin compte.
struct Atteste {
    salarie: String,
    debut: String,
    fin: String,
    numero_objet: String,
    cachets: i64,
    heures: i64,
    jours: i64,
}

/// Les contrats que les engagements importes attestent.
fn historique(ctx: &Contexte) -> R<Vec<Atteste>> {
    let mut out = Vec::new();
    for i in engagement::importe::tous(&ctx.racine).map_err(Echec::from)? {
        let Some(c) = i.contrat_lu().map_err(Echec::from)? else { continue };
        let atteste =|debut: String, fin: String, (cachets, heures, jours): (Option<i64>, Option<i64>, Option<i64>)| Atteste {
            salarie: c.salarie.clone(),
            debut,
            fin,
            numero_objet: c.contrat.numero_objet.clone().unwrap_or_default(),
            cachets: cachets.unwrap_or(0),
            heures: heures.unwrap_or(0),
            jours: jours.unwrap_or(0),
        };
        if !i.permanent() {
            out.push(atteste(c.debut.clone().unwrap_or_default(), c.fin.clone().unwrap_or_default(), (c.contrat.cachets, c.contrat.heures, c.contrat.jours)));
            continue;
        }
        for (m, p) in i.paies() {
            let lu = |j: Option<Jour>| j.map(|j| j.iso()).filter(|j| j.starts_with(m.as_str()));
            let debut = lu(p.debut).unwrap_or_else(|| format!("{}-01", m));
            let fin = match lu(p.fin) {
                Some(f) => f,
                None => Mois::lire(m).map(Mois::dernier_jour).map(|j| j.iso()).map_err(plain)?,
            };
            out.push(atteste(debut, fin, (p.cachets, p.heures, p.jours)));
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
    let mut connus = engagement::stock::mois_saisis(&ctx.racine).map_err(Echec::from)?.into_iter().map(String::from).collect::<Vec<_>>();
    connus.reverse();
    for mc in &connus {
        couverts.insert(mc.clone());
        for (cle, e) in engagements_du_mois(ctx, mc)? {
            let (debut, fin) = bornes_brutes(&e);
            if debut.is_empty() || debut > dernier || (if fin.is_empty() { &debut } else { &fin }).as_str() < premier.as_str() {
                continue;
            }
            if !voulu(&cle) {
                continue;
            }
            let ref_spec = e.spectacle.as_ref().map(ToString::to_string).unwrap_or_default();
            let spec = spectacles.trouver(&ref_spec);
            let numero = spec.and_then(|s| s.numero_objet.remplie()).map_or_else(|| ref_spec.clone(), str::to_string);
            let cachets = cachets_de(ctx, &e)?;
            let dates = jours_joues(&e);
            let fin = if fin.is_empty() { debut.clone() } else { fin };
            let ident = (cle.to_string(), debut.clone(), fin.clone());
            let deja = vus.get(&ident).copied();
            if deja.is_some() && mc.as_str() != mois {
                continue;
            }
            let jours = match e.jours.as_ref().and_then(socle::forme::NombreLu::as_f64).filter(|j| *j != 0.0) {
                Some(j) => j.trunc() as i64,
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
fn emploi(ctx: &Contexte, c: ChargeEmploi) -> R<Emploi> {
    let feminin = c.feminin || c.salarie.is_some_and(|s| declarations::ntt::sexe_de(&s.nir, &s.sexe) == "2");
    if emplois::metier(&c.metier).map_err(plain)?.is_none() {
        return Err(erreur(
            "ValueError",
            format!("Métier {} inconnu : ni dans le catalogue des métiers, ni dans la nomenclature France Travail.", repr(&c.metier)),
        ));
    }
    let champs = emplois::resoudre(&c.metier, c.cadre, feminin, ctx.entreprise.idcc.texte()).map_err(plain)?;
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
    if let Some(clos) = crate::routes::rappels::exercice_clos(ctx, &annee)? {
        let le: String = clos.le.chars().take(10).collect();
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


/// LES CHANGEMENTS D'UNE CARTE, ECRITS : le brouillon de la carte visee (neuf, ou celui que son engagement ecrit pour ce
/// mois, `engagement::stock::Objet::vue`) prend chaque changement dans l'ordre, tout ou rien (`Changements::appliquer`),
/// puis s'ecrit entier (`ecrire_une_carte`). Rend l'identite de l'engagement ecrit, le brouillon tel qu'il est ecrit, et
/// ce qui lui manque pour etre valide.
fn enregistrer_saisie(ctx: &Contexte, c: ChargeCase) -> R<CarteEcrite> {
    let mois = c.mois.to_string();
    let _verrou = avant_d_ecrire(ctx, &mois)?;
    let (mut brouillon, id) = match c.carte {
        CarteVisee::Neuve(n) => (Brouillon::neuf(n), None),
        CarteVisee::Ecrite(id) => (brouillon_ecrit(ctx, &mois, &id)?, Some(id)),
    };
    c.changements.appliquer(ctx, c.mois, &mut brouillon, |b| qui_de_la_carte(ctx, &b.en_carte())).map_err(refus)?;
    let id = ecrire_une_carte(ctx, &mois, id.as_ref(), &brouillon.en_carte())?;
    let brouillon = brouillon_ecrit(ctx, &mois, &id)?;
    let (k, _) = engagement::stock::designe(&ctx.racine, &mois, &id).map_err(Echec::from)?;
    let vue = engagement::stock::vue_du_mois(&ctx.racine, &mois).map_err(Echec::from)?.ok_or_else(|| refus(format!("{} : la carte écrite ne s'y lit pas.", Mois::en_clair(&mois))))?;
    let a_completer = manque_de(ctx, &vue, k);
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let a = maintenant();
    Ok(CarteEcrite {
        phrase: match &a_completer {
            Some(m) => format!("Carte enregistrée ; à compléter avant de valider : {}", m),
            None => "Carte enregistrée.".to_string(),
        },
        engagement: id,
        brouillon,
        a_completer,
        enregistre_dit: format!("Enregistré à {}", a.chars().skip(11).take(5).collect::<String>()),
        enregistre_a: a,
    })
}

/// LE BROUILLON QU'ECRIT L'ENGAGEMENT `id` de l'ecran de `mois` : sa vue de ce mois (`engagement::stock::Objet::vue`),
/// lue en brouillon.
pub(crate) fn brouillon_ecrit(ctx: &Contexte, mois: &str, id: &Identite) -> R<Brouillon> {
    let (_, o) = engagement::stock::designe(&ctx.racine, mois, id).map_err(refus)?;
    let qui = engagement::etats::qui_de(ctx, &o);
    let vue = o.vue(mois).map_err(Echec::from)?.ok_or_else(|| refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", qui)))?;
    Brouillon::de_la_carte(&vue, &qui).map_err(refus)
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
pub(crate) fn ecrire_une_carte(ctx: &Contexte, mois: &str, id: Option<&Identite>, saisie: &SaisieCarte) -> R<Identite> {
    // UN ENGAGEMENT NEUF, OU LE BROUILLON QUI DEMENAGE, S'ECRIT DANS SON PREMIER MOIS (`engagement::stock::ou_s_ecrit`) : ce
    // mois-la passe les memes refus que celui de la carte, et donne ce qu'il dit par defaut.
    let fichier = engagement::stock::ou_s_ecrit(&ctx.racine, mois, id, saisie).map_err(refus)?;
    let ecrit_dans = match id {
        Some(id) => Some(engagement::stock::designe(&ctx.racine, mois, id).map_err(refus)?.1.fichier),
        None => None,
    };
    if fichier != mois && ecrit_dans.as_deref() != Some(fichier.as_str()) {
        refuser_si_importe(ctx, &fichier)?;
        refuser_si_clos(ctx, &fichier)?;
    }
    let id = engagement::stock::ecrire_la_carte(ctx, mois, id, saisie, &squelette(ctx, &fichier)?).map_err(|m| erreur("ErreurOperation", m))?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(id)
}

/// CE QUI MANQUE A CHAQUE ENGAGEMENT DE LA SAISIE `data` pour se calculer, dans son ordre (`None` : rien). Une saisie
/// qui se calcule entiere n'a rien qui manque. Sinon, chaque engagement se calcule a la suite de ceux d'avant qui se
/// calculent (le calcul d'un engagement lit ceux qui le precedent) : celui qui refuse porte le refus, et les suivants se
/// calculent sans lui. Un engagement valide ne manque de rien, sa validation l'a calcule, sauf les indemnites journalieres
/// d'un arret venu apres elle (`engagement::maintien::indemnites_a_dire`).
pub(crate) fn manques_de_la_saisie(ctx: &Contexte, data: &VueDuMois) -> Vec<Option<String>> {
    let tous = &data.engagements;
    let mois = data.mois.to_string();
    let a_dire = |e: &SaisieCarte| -> Option<String> {
        if e.est_validee() { engagement::maintien::indemnites_a_dire(ctx, e, &mois).ok().flatten() } else { None }
    };
    let valides: Vec<Option<String>> = tous.iter().map(a_dire).collect();
    if valides.iter().all(Option::is_none) && (tous.iter().all(SaisieCarte::est_validee) || engagement::pour_l_ecran(ctx, data).is_ok()) {
        return vec![None; tous.len()];
    }
    let mut gardes: Vec<SaisieCarte> = Vec::new();
    let mut manques = Vec::new();
    for (e, manque) in tous.iter().zip(valides) {
        if e.est_validee() {
            if manque.is_none() {
                gardes.push(e.clone());
            }
            manques.push(manque);
            continue;
        }
        let essai = VueDuMois { engagements: gardes.iter().cloned().chain(std::iter::once(e.clone())).collect(), ..data.clone() };
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
pub(crate) fn manque_de(ctx: &Contexte, data: &VueDuMois, i: usize) -> Option<String> {
    let e = data.engagements.get(i)?;
    let cle = &e.salarie;
    let numero = e.numero_dit();
    let mut rang = 0;
    let mut siens = Vec::new();
    for (j, x) in data.engagements.iter().enumerate() {
        let meme_salarie = cle.is_some() && x.salarie == *cle;
        let meme_numero = numero.is_some() && x.numero_dit() == numero;
        if j == i {
            rang = siens.len();
        }
        if j == i || meme_salarie || meme_numero {
            siens.push(x.clone());
        }
    }
    manques_de_la_saisie(ctx, &VueDuMois { engagements: siens, ..data.clone() }).swap_remove(rang)
}


// ------------------------------------------------------------------ le montant vise


/// Ce qui a change, de l'engagement `avant` a l'engagement `apres` (la saisie que la recherche a ajustee).
fn ce_qui_a_change(avant: &SaisieCarte, apres: &SaisieCarte, index: usize) -> CeQuiChange {
    let mut lignes = Vec::new();
    for (rang, (a, b)) in avant.lignes().iter().zip(apres.lignes().iter()).enumerate() {
        let (ta, tb) = (a.taux(), b.taux());
        if ta.is_none() && tb.is_none_or(|t| t.is_zero()) {
            continue; // restee sans taux : rien a ecrire
        }
        if ta != tb {
            lignes.push(TauxQuiChange { rang, rubrique: b.rubrique().to_string(), taux_avant: ta, taux: tb });
        }
    }
    let salaire = |e: &SaisieCarte| socle::ecriture::en_valeur(&e.salaire_mensuel);
    let (sa, sb) = (salaire(avant), salaire(apres));
    CeQuiChange { index, salaire_mensuel: (!documents::valeur::valeurs_egales(&sb, &sa)).then_some(sb), lignes }
}

/// Ce que la phrase du montant vise dit de la cible.
fn cible_dite(c: Cible) -> &'static str {
    match c {
        Cible::Net => "Net salarié",
        Cible::Brut => "Brut",
        Cible::Cout => "Coût entreprise",
    }
}

/// Le taux qu'il faudrait, pour que l'engagement `engagement` (son identite ; son brouillon enregistre, ou sa modification
/// pas encore ecrite, `mois_designe`) coute CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, c: ChargeAjuster) -> R<Ajuste> {
    let vise = q2(socle::num::dec_f64(c.montant).map_err(|m| erreur("InvalidOperation", m))?);
    if vise <= Decimal::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", vise)));
    }
    let texte = c.mois.to_string();
    let (data, ids) = mois_designe(ctx, c.mois, c.modifications)?;
    let i = ids.iter().position(|x| *x == c.engagement).ok_or_else(|| {
        refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois.", Mois::en_clair(&texte)))
    })?;
    let v = engagement::viser::viser(ctx, &data, &[i], c.cible, vise).map_err(|r| match r {
        engagement::viser::RefusVisee::Refus(m) => refus(m),
        engagement::viser::RefusVisee::Calcul(m) => refus_du_calcul(ctx, &data, m),
    })?;
    let (avant, apres) = match (data.engagements.get(i), v.saisie.engagements.get(i)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(Echec::from(format!("{} : l'engagement visé n'a pas de paie calculée.", Mois::en_clair(&texte)))),
    };
    let vue = vue_paie(ctx, &v.paies[i], apres, &v.saisie).map_err(Echec::from)?;
    let atteint = q2(v.atteint);
    let mut facteur = v.facteur.round_dp(6);
    facteur.rescale(6);
    let change = ce_qui_a_change(avant, apres, i);
    // Le taux qu'il faudrait : chaque ligne qui change, puis le salaire du mois s'il change.
    let mut faut: Vec<String> = change
        .lignes
        .iter()
        .map(|l| {
            let euros = |c: &Option<Decimal>| c.map(socle::texte::euros).unwrap_or_default();
            let avant = if l.taux_avant.is_none() { String::new() } else { format!(" au lieu de {}", euros(&l.taux_avant)) };
            format!("{} à {}{}", l.rubrique, euros(&l.taux), avant)
        })
        .collect();
    if let Some(s) = change.salaire_mensuel.as_ref().filter(|s| !s.is_null()) {
        faut.push(format!("salaire du mois à {}", documents::valeur::euros_de(s)));
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
pub(crate) fn refus_du_calcul(ctx: &Contexte, data: &VueDuMois, message: String) -> Echec {
    if engagement::saisie_incomplete(&message) {
        return erreur("SaisieIncomplete", message);
    }
    if let Some(m) = engagement::refus_de_saisie_incomplete(ctx, data) {
        return erreur("SaisieIncomplete", m);
    }
    Echec::from(message)
}

/// LE MOIS QUE L'APERCU ET LE MONTANT VISE CALCULENT : ses engagements tels qu'ils sont ecrits (le brouillon enregistre
/// de chaque carte en saisie, la validation de chaque engagement valide), dans l'ordre de l'ecran (`vue_du_mois`), et
/// chaque modification pas encore ecrite d'un engagement valide (`modifications`) : son brouillon (`Brouillon::de_la_carte`
/// de sa vue) prend ses changements comme `modifier-engagement` les prendra, et se calcule a sa place, en saisie. Rend
/// ce mois et l'identite de chacun de ses engagements, dans son ordre. Rien ne s'ecrit.
fn mois_designe(ctx: &Contexte, mois: Mois, modifications: Vec<ModificationEnCours>) -> R<(VueDuMois, Vec<Identite>)> {
    let texte = mois.to_string();
    let mut v = engagement::stock::vue_du_mois(&ctx.racine, &texte).map_err(Echec::from)?.unwrap_or_else(|| VueDuMois::vide(mois));
    let ids = engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, &texte).map_err(Echec::from)?);
    if ids.len() != v.engagements.len() {
        return Err(Echec::from(format!("{} : {} identités pour {} engagements.", Mois::en_clair(&texte), ids.len(), v.engagements.len())));
    }
    for m in modifications {
        let i = ids.iter().position(|x| *x == m.engagement).ok_or_else(|| {
            refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois.", Mois::en_clair(&texte)))
        })?;
        let ecrit = v.engagements[i].clone();
        if !ecrit.est_validee() {
            return Err(refus(format!(
                "{} : cet engagement n'est pas validé ; ses changements s'enregistrent (enregistrer-saisie), l'aperçu lit son brouillon.",
                qui_de_la_carte(ctx, &ecrit)
            )));
        }
        let mut b = Brouillon::de_la_carte(&ecrit, &qui_de_la_carte(ctx, &ecrit)).map_err(refus)?;
        m.changements.appliquer(ctx, mois, &mut b, |b| qui_de_la_carte(ctx, &b.en_carte())).map_err(refus)?;
        let saisie = engagement::stock::en_forme(&b.en_carte()).map_err(refus)?;
        // L'avenant du mois se deduit des termes dates de l'engagement ecrit (`stock::Objet::vue`) : le brouillon ne le
        // porte pas.
        v.engagements[i] = SaisieCarte { date_modification: ecrit.date_modification.clone(), avant: ecrit.avant.clone(), ..saisie };
    }
    Ok((v, ids))
}

/// Le salarie qu'une carte nomme, pour un refus (« Cette carte » sans lui).
fn qui_de_la_carte(ctx: &Contexte, c: &SaisieCarte) -> String {
    match &c.salarie {
        Some(s) => ctx.nomme(s),
        None => "Cette carte".to_string(),
    }
}

/// Une paie que l'ecran montre, et pourquoi elle s'est recalculee d'elle-meme (`{"le", "raison"}`), s'il y a lieu.
pub(crate) struct PaieMontree {
    pub(crate) paie: Paie,
    pub(crate) recalcul: Option<engagement::paie_ecrite::Recalcul>,
}


/// LES PAIES QUE L'ECRAN MONTRE POUR LA SAISIE `data` : un engagement valide se lit (sa paie ecrite, `etats::Valide::paie`),
/// un engagement en saisie se calcule, sans rien ecrire, comme un engagement valide d'un mois d'apres le present, qui n'a
/// pas encore de paie. Un mois tout valide et present ne calcule rien.
///
/// L'ecran rend chaque engagement valide tel que le moteur le lui a montre (`valide` pose) : il retrouve le sien, sur le
/// disque, par son salarie et son numero de contrat (ecrit a la validation).
pub(crate) fn paies_de_l_ecran(ctx: &Contexte, data: &VueDuMois, mois: &str) -> R<Vec<PaieMontree>> {
    let disque = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let etats = if disque.is_some() { engagement::etats::du_mois(ctx, mois, &departs)? } else { Vec::new() };
    // TRAVAIL A FAIRE (chaine A) : l'engagement ecrit (`stock::Objet::champs`) se lit par ses cles.
    let qui = |b: &engagement::saisie::Carte| (b.salarie.clone(), b.numero_dit().map(String::from));
    let mut lues: Vec<Option<PaieMontree>> = Vec::new();
    for e in &data.engagements {
        let mut lue = None;
        if e.est_validee() {
            let sien = (e.salarie.clone(), e.numero_dit().map(String::from));
            // Un mois d'apres le present n'a pas encore de paie ecrite : l'engagement valide s'y calcule pour l'ecran.
            let apres_le_present = mois > engagement::etats::mois_present(ctx).as_str();
            let mut trouve = None;
            for (j, x) in etats.iter().enumerate() {
                let a_venir = apres_le_present && x.paie_ecrite(mois)?.is_none();
                if x.valide().is_some() && !a_venir && sien.1.is_some() && qui(x.objet().carte()) == sien {
                    trouve = Some((j, x));
                    break;
                }
            }
            if let (Some((j, x)), Some(d)) = (trouve, disque.as_ref()) {
                let v = x.valide().ok_or("engagement en saisie")?;
                lue = Some(PaieMontree { paie: v.paie(ctx, d, j)?, recalcul: v.recalcul(mois)? });
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

/// LES CASES DU CONTRAT ECRIT QUE CET ENGAGEMENT APPELLE (`engagement::ContratEcrit`), et elles seules : la
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
fn champs_du_contrat_ecrit_en_saisie(ctx: &Contexte, e: &SaisieCarte, mois: &str) -> Vec<&'static str> {
    let debut = e.debut_lu();
    if engagement::stock::commence_avant_le_logiciel(&ctx.racine, &debut, mois).unwrap_or(false) {
        return numero_a_dire(ctx, e.salarie.as_deref().unwrap_or_default(), &debut);
    }
    let Ok(nat) = e.nature_lue() else { return Vec::new() };
    let Some(cle) = &e.salarie else { return Vec::new() };
    if nat.modele_contrat().is_none() || engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, cle, &debut, e.numero.as_deref()).is_ok_and(|v| v) {
        return Vec::new();
    }
    let mut v = vec!["periode_essai_jours"];
    if e.motif_recours.as_deref().is_some_and(|m| nat.motif_de_remplacement(m)) {
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
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CaseDeFrais {
    ligne: usize,
    case: String,
    libelle: &'static str,
    aide: Option<String>,
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
    /// Le taux de la premiere ligne de cachet ; sans cachet, celui que la carte ou son mois ecrit.
    taux_cachet: Option<socle::forme::NombreLu>,
    heures: Option<f64>,
    faits_avances: Vec<&'static str>,
    cases: Vec<CaseDeFrais>,
    remuneration: Vec<LigneVue>,
    /// Les jours que la carte declare, sinon ceux du calcul ; le lieu qu'elle ecrit.
    jours: socle::forme::NombreLu,
    lieu: String,
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

/// Une paie pour l'ecran. `brut` : la carte de l'engagement ; `data` : la saisie du mois (son `taux_cachet`).
pub(crate) fn vue_paie(ctx: &Contexte, p: &Paie, brut: &SaisieCarte, data: &VueDuMois) -> R<VuePaie> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let s = &p.salarie;
    let exige = |c: &droit::fiche::Case<String>, k: &str| if c.est_absente() { Err(erreur("KeyError", socle::fichiers::absent(k))) } else { Ok(c.clone()) };
    let salarie = SalarieVu {
        cle: s.id.clone(),
        nom: exige(&s.nom.en_texte(), "nom")?,
        prenom: if s.prenom_affichage.texte().is_empty() { exige(&s.prenom.en_texte(), "prenom")? } else { s.prenom_affichage.clone() },
    };
    let spectacle = match &p.spectacle {
        Some(s) if s.titre.est_absente() => return Err(erreur("KeyError", socle::fichiers::absent("titre"))),
        Some(s) => Some(SpectacleVu {
            titre: socle::ecriture::en_valeur(&s.titre),
            numero_objet: if s.numero_objet.est_absente() { Value::Null } else { socle::ecriture::en_valeur(&s.numero_objet) },
        }),
        None => None,
    };
    let mois_du_contrat = e.mois_du_contrat()?;
    // Le taux du cachet, tel que le contrat l'imprime : celui de la premiere ligne a l'unite « cachet » ; sans cachet, la
    // valeur ecrite, telle quelle, et rien quand rien n'est ecrit (regle 1 : plus de 130 EUR).
    let premier_cachet = e
        .lignes
        .iter()
        .find(|l| droit::catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au).ok().map_or(false, |r| r.unite == "cachet"));
    let taux_cachet = match premier_cachet {
        Some(l) => socle::forme::NombreLu::de_f64(socle::num::flottant(l.taux().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("taux")))?)),
        None => brut.taux_cachet.clone().or_else(|| data.taux_cachet.clone()),
    };
    let jours = brut.jours.clone().unwrap_or_else(|| socle::forme::NombreLu::from(e.jours));
    let lieu = brut.lieu.clone().unwrap_or_default();
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
            .filter_map(|l| l.marques())
            .filter_map(|m| Some((m.rang?, m)))
            .flat_map(|(i, m)| {
                m.cases.iter().map(move |k| CaseDeFrais { ligne: i, case: k.cle().to_string(), libelle: libelle_de_case(*k), aide: m.aides.get(k).cloned() })
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
        // (CCH-11) -- donc rien a verifier de ce cote.
        if let Some(sp) = p.spectacle.as_ref().filter(|s| s.actif.dite() == Some(&false)) {
            if sp.titre.est_absente() {
                return Err(erreur("KeyError", socle::fichiers::absent("titre")));
            }
            out.push(format!(
                "Le spectacle « {} » (objet {}) est archive : pour le jouer a nouveau, le reactiver (Configuration › Spectacles).",
                sp.titre.texte(),
                sp.numero_objet.texte()
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

/// Le salarie tel que la carte le nomme.
#[derive(Debug, Serialize)]
struct CleEcrite {
    cle: Option<String>,
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

/// Une paie de l'apercu, et l'identite de son engagement : la carte retrouve la sienne.
#[derive(Debug, Serialize)]
struct PaieDeLApercu {
    #[serde(flatten)]
    paie: PaieVue,
    engagement: Identite,
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
    let (data, identites) = mois_designe(ctx, c.mois, c.modifications)?;
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
                Some(t) if taux.is_some_and(|x| socle::num::flottant(t) == x) => {
                    tarif = Some(t);
                    break;
                }
                _ => {}
            }
        }
        if let Some(tarif) = tarif {
            let dit = format!("{} à {}", l.designation.to_lowercase(), socle::texte::euros(tarif));
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
fn dire_l_entree(ctx: &Contexte, mois: &str, brut: &SaisieCarte, v: &mut VuePaie) -> R<()> {
    let debut = v.debut.iso();
    if brut.est_validee() || !engagement::stock::commence_avant_le_logiciel(&ctx.racine, &debut, mois).map_err(Echec::from)? {
        return Ok(());
    }
    let qui = format!("{} {}", v.salarie.nom.texte(), v.salarie.prenom.texte()).trim().to_string();
    let le = v.debut.jj_mm_aaaa();
    let numero = v.numero_contrat.as_ref().map_or("", |n| n.as_str());
    let phrase = if brut.numero_dit().is_some() {
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
fn apercu_des_paies(ctx: &Contexte, data: &VueDuMois) -> R<Calcul> {
    let refus = match apercu_de(ctx, data) {
        Ok(c) => return Ok(c),
        Err(refus) => refus,
    };
    let manques = manques_de_la_saisie(ctx, data);
    if manques.iter().all(Option::is_none) {
        return Err(refus);
    }
    let tous = &data.engagements;
    let complete = VueDuMois { engagements: tous.iter().zip(&manques).filter(|(_, m)| m.is_none()).map(|(e, _)| e.clone()).collect(), ..data.clone() };
    let mut r = apercu_de(ctx, &complete)?;
    let mut calculees = std::mem::take(&mut r.paies).into_iter();
    let mut calcules = std::mem::take(&mut r.blocs).into_iter();
    let mois = data.mois.to_string();
    for (e, m) in tous.iter().zip(&manques) {
        match m {
            None => {
                r.paies.extend(calculees.next());
                r.blocs.push(calcules.next().flatten());
            }
            Some(m) => {
                let (debut, fin) = bornes_brutes(e);
                r.paies.push(PaieVue::ACompleter(PaieACompleter {
                    a_completer: socle::echec::phrase_de(m),
                    salarie: CleEcrite { cle: e.salarie.as_ref().map(ToString::to_string) },
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

/// L'apercu de la saisie `data` : chaque engagement se calcule.
fn apercu_de(ctx: &Contexte, data: &VueDuMois) -> R<Calcul> {
    let mois = data.mois.to_string();
    let lu = data.mois;
    let montrees = paies_de_l_ecran(ctx, data, &mois)?;
    let mut paies: Vec<Paie> = montrees.iter().map(|m| m.paie.clone()).collect();
    documents::ecrire_documents::numeroter(ctx, &mut paies, lu.annee(), &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let tous = &data.engagements;
    let vide = SaisieCarte::default();
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(vue_paie(ctx, p, tous.get(i).unwrap_or(&vide), data)?);
    }
    // LES CONSEILS DE SAISIE : au plus un par carte, quand l'option de la compagnie est cochee, pour un
    // engagement en saisie seulement. Ils ne vivent qu'ici, dans la vue de l'ecran : ni dans les avertissements de la
    // paie, ni au bulletin, ni a `produire`.
    for (i, (v, c)) in vues.iter_mut().zip(crate::compagnie::conseils::de_saisie(ctx, &paies)?).enumerate() {
        if !tous.get(i).is_some_and(SaisieCarte::est_validee) {
            v.conseil = c;
        }
    }
    for (i, v) in vues.iter_mut().enumerate() {
        dire_l_entree(ctx, &mois, tous.get(i).unwrap_or(&vide), v)?;
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
        let cle = format!("{}|{}", en_texte(&socle::ecriture::en_valeur(&v.salarie.nom)), en_texte(&socle::ecriture::en_valeur(&v.salarie.prenom)));
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
