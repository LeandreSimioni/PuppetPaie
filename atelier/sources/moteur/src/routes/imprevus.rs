//! Les routes des IMPREVUS d'un salarie : la liste (`imprevus`, `arrets`, `fins-de-contrat`, `arret-contrats`), l'arret
//! saisi, repris, prolonge, signale ou oublie (`arret-enregistrer`, `arret-reprise`, `arret-prolonger`,
//! `arret-signaler`, `arret-oublier`), ses indemnites journalieres (`arret-indemnites`), la suspension du bloc 65
//! (`suspension-enregistrer`, `-modifier`, `-oublier`), et la fin d'un contrat signalee, avec ses documents
//! (`fin-contrat-signaler`, `fin-contrat-documents`).
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN ; un signalement s'ecrit en fichier, il ne s'envoie pas. Les refus sont des
//! `ErreurOperation` (la page les montre tels quels) ; un refus n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (rien de remplissable s'il n'y a pas a le remplir). Le moteur dit, sur chaque
//! imprevu resume (`resume`), ce que ses faits ouvrent : `ij_attendues` (l'arret ouvre le maintien de salaire, par la loi
//! ou la convention : ses indemnites journalieres se disent, `arret-indemnites`), `ij_majoree_attendue` et
//! `jours_deja_indemnises_attendus`. La page n'ouvre que cela.
//!
//! LES FORMES. Chaque charge est une structure lue une fois a l'entree (`routes::lire_charge`) ; chaque reponse, une
//! structure. Ce que ce module lit d'un autre domaine se lit tel qu'il est : l'imprevu ecrit (`engagement::imprevus`, qui
//! en declare les champs), la saisie des engagements (`engagement::stock`), le journal des envois (`envois::journal`), les
//! obligations d'un arret (`crate::obligations`), les nomenclatures livrees du bloc 60 (`droit::arrets::Bloc60`, que lisent
//! aussi `declarations` et `engagement`) et des suspensions (`droit::permanents`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::routes::{brut, cle_du_salarie, en_json, lire_charge, maintenant, Pastille, SansCharge, INTERFACE};
use declarations::signalement;
use droit::contexte::{Contexte, IdSalarie};
use engagement::calcul::Paie;
use droit::arrets::{Bloc60, CodeDuBloc60};
use engagement::imprevus::{self, Suspend, ARRET, SUSPENSION};
use engagement::numeros::NumeroContrat;
use envois::etats::{get, getd, items, objet_vide, ou, Dico};
use socle::date::Jour;
use documents::valeur::{d, en_texte, vrai};
use socle::echec::{erreur, Echec, R};
use socle::mois::Mois;
use socle::num::Decimal;
use documents::valeur::{nom_type, valeur_ecrite, valeur_en_texte};


use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

const DERNIER_JOUR: Champ = Champ { nom: "dernier_jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé avant l'arrêt, qui le désigne", avance: false };
const DEBUT: Champ = Champ { nom: "debut", libelle: Affiche::new("Premier jour d'absence"), forme: Forme::Date, sens: "le premier jour de l'absence, qui la désigne", avance: false };
const UN_ARRET: &str = r#"{"salarie":"FULLER","dernier_jour":"2026-11-27"}"#;

/// Une fiche de ce module : `rend` et les refus que toutes partagent.
const fn fiche(route: Route, geste: &'static str, charge: &'static [Champ], exemple: &'static str, rend: &'static str, acte: Acte, avance: bool) -> Fiche {
    Fiche { route, geste, charge, exemple, rend, refus: &[OPERATION, MANQUE, ILLISIBLE, FORME], acte, avance, phrase: 0, ecran: ECRAN }
}

const ORDRE: Champ = Champ { nom: "ordre", libelle: Affiche::new("Numéro d'ordre"), forme: Forme::Nombre, sens: "son numéro d'ordre ; sinon le suivant de ceux déjà partis", avance: true };
const REMPLACE: Champ = Champ { nom: "remplace", libelle: Affiche::new("Remplace"), forme: Forme::Texte, sens: "le signalement qu'il remplace", avance: true };
const TEST: Champ = Champ { nom: "test", libelle: Affiche::new("Fichier d'essai"), forme: Forme::OuiNon, sens: "vrai : un fichier d'essai, marqué comme tel", avance: true };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches, dans l'ordre d'un arrêt : il s'enregistre, se suit, se signale, s'oublie.
pub const ROUTES: &[Fiche] = &[
    fiche(Route::Imprevus, "Lire les imprévus d'un mois ou d'un salarié, et ce qu'on peut en saisir.",
        &[MOIS, Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "à la place du mois : les imprévus de ce salarié", avance: true }],
        r#"{"mois":"2026-11"}"#, "imprevus (chacun résumé, un arrêt avec ses obligations), nomenclatures", Acte::Lit, false),
    Fiche { ecran: Ecran::Auto(Place::Exports, Affiche::new("Lire tous les arrêts")), ..fiche(Route::Arrets, "Lire tous les arrêts de la compagnie.", &[], "{}", "phrase, phrases (qui, quand, le motif, ce qui reste à signaler), arrets : chacun résumé", Acte::Lit, true) },
    fiche(Route::FinsDeContrat, "Lire les fins de contrat dues ce mois, et ce qui manque à chacune.", &[MOIS],
        r#"{"mois":"2026-11"}"#, "fins : chacune avec ce qui est fait et ce qui manque", Acte::Lit, false),
    fiche(Route::ArretContrats, "Lire les contrats qu'un arrêt suspendrait à ce jour.",
        &[SALARIE, Champ { nom: "jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé", avance: false }],
        r#"{"salarie":"FULLER","jour":"2026-11-27"}"#, "contrats : ceux du salarié qui couvrent ce jour", Acte::Lit, false),
    Fiche { phrase: 400, ..fiche(Route::ArretEnregistrer, "Enregistrer un arrêt de travail.",
        &[
            SALARIE, DERNIER_JOUR,
            Champ { nom: "debut", libelle: Affiche::new("Début de l'arrêt"), forme: Forme::Date, sens: "le premier jour que l'avis prescrit ; le dernier jour travaillé s'il a été prescrit ce jour-là", avance: false },
            Champ { nom: "fin", libelle: Affiche::new("Fin prescrite"), forme: Forme::Date, sens: "la fin prescrite", avance: false },
            Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif d'arrêt (nomenclatures de imprevus)", avance: false },
            Champ { nom: "date_accident", libelle: Affiche::new("Jour de l'accident"), forme: Forme::Date, sens: "le jour de l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "connu", libelle: Affiche::new("Su le"), forme: Forme::Objet, sens: "{jour, heure} où la compagnie a su l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat suspendu, quand plusieurs couvrent le jour", avance: true },
            Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "pour le maintien d'un permanent", avance: true },
            Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true },
        ],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","debut":"2026-11-30","fin":"2026-11-30","motif":"01"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 700, ..fiche(Route::ArretProlonger, "Prolonger un arrêt : sa nouvelle fin.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin prescrite"), forme: Forme::Date, sens: "la nouvelle fin prescrite", avance: false }],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","fin":"2026-12-04"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 400, ..fiche(Route::ArretReprise, "Poser ou effacer la reprise d'un arrêt.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "reprise", libelle: Affiche::new("Reprise le"), forme: Forme::Date, sens: "le jour de la reprise ; nul l'efface", avance: false }, Champ { nom: "motif_reprise", libelle: Affiche::new("Motif de la reprise"), forme: Forme::Texte, sens: "le motif de la reprise", avance: true }],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","reprise":"2026-12-03"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 400, ..fiche(Route::ArretIndemnites, "Dire les indemnités journalières d'un arrêt, par jour.",
        &[SALARIE, DERNIER_JOUR,
            Champ { nom: "ij_brute", libelle: Affiche::new("Indemnité brute d'un jour"), forme: Forme::Texte, sens: "l'indemnité d'un jour, avant CSG et CRDS (relevé de la CPAM), en texte (« 40.89 ») ; absente, elle ne change pas", avance: false },
            Champ { nom: "ij_nette", libelle: Affiche::new("Indemnité nette d'un jour"), forme: Forme::Texte, sens: "la même, nette de CSG et CRDS", avance: false },
            Champ { nom: "ij_brute_majoree", libelle: Affiche::new("Indemnité brute majorée"), forme: Forme::Texte, sens: "accident du travail, dès le 29e jour : brute", avance: true },
            Champ { nom: "ij_nette_majoree", libelle: Affiche::new("Indemnité nette majorée"), forme: Forme::Texte, sens: "la même, nette", avance: true },
            Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "les jours indemnisés dans les douze mois qu'aucun arrêt saisi ne porte", avance: true }],
        r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","ij_brute":"40.89","ij_nette":"38.15"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 600, ..fiche(Route::ArretSignaler, "Fabriquer le signalement d'un arrêt, sans le déposer.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "nature", libelle: Affiche::new("Nature"), forme: Forme::Texte, sens: "04 arrêt (par défaut), ou 05 reprise", avance: true }, REMPLACE, ORDRE, TEST,
            Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        UN_ARRET, "fichier (écrit dans sorties/AAAA-MM/), phrase, nature, type, ordre, avertissements", Acte::Ecrit, false) },
    Fiche { phrase: 400, ..fiche(Route::ArretOublier, "Retirer un arrêt qui n'a pas été déclaré.", &[SALARIE, DERNIER_JOUR],
        UN_ARRET, "arret (l'arrêt retiré, résumé), phrase", Acte::Ecrit, false) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionEnregistrer, "Enregistrer une absence qui suspend un contrat.",
        &[SALARIE, DEBUT, Champ { nom: "fin", libelle: Affiche::new("Dernier jour"), forme: Forme::Date, sens: "son dernier jour", avance: false }, Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif (nomenclatures de imprevus)", avance: false }, Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat, quand plusieurs couvrent le jour", avance: true }, Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27","motif":"501"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionModifier, "Changer la fin ou le motif d'une absence.",
        &[SALARIE, DEBUT, Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin"), forme: Forme::Date, sens: "sa nouvelle fin", avance: false }, Champ { nom: "motif", libelle: Affiche::new("Nouveau motif"), forme: Forme::Texte, sens: "son nouveau motif", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionOublier, "Retirer une absence qui n'a pas été déclarée.", &[SALARIE, DEBUT],
        r#"{"salarie":"FULLER","debut":"2026-11-27"}"#, "suspension (l'absence retirée, résumée), phrase", Acte::Ecrit, true) },
    Fiche { phrase: 600, ..fiche(Route::FinContratSignaler, "Fabriquer le signalement d'une fin de contrat, sans le déposer.",
        &[MOIS, SALARIE, REMPLACE, ORDRE, TEST, Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, type, ordre, preavis, avertissements", Acte::Ecrit, true) },
    Fiche { phrase: 160, ..fiche(Route::FinContratDocuments, "Fabriquer le certificat de travail et le reçu pour solde de tout compte.",
        &[MOIS, SALARIE, Champ { nom: "le", libelle: Affiche::new("Établis le"), forme: Forme::Date, sens: "le jour où ils s'établissent ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "phrase, certificat et recu (leurs chemins), dossier, avertissements (ce qu'il faut savoir en les remettant)", Acte::Ecrit, false) },
];

// ------------------------------------------------------------------ les charges
//
// `salarie` y est toujours ce que la personne en dit : la cle de la fiche, ou son nom (`Contexte::cle_du_salarie`), resolu
// a l'entree de la route en `IdSalarie`. Ce qu'elles ecrivent, la fenetre l'ecrit (`routes::INTERFACE`).

/// La charge de `imprevus` : les imprevus d'un mois, ou d'un salarie ; tous, sans l'un ni l'autre.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeImprevus {
    #[serde(default)]
    mois: Option<Mois>,
    #[serde(default)]
    salarie: Option<String>,
}

/// La charge de `fins-de-contrat` : le mois.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeMois {
    mois: Mois,
}

/// La charge de `arret-contrats` : le salarie et son dernier jour travaille.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretContrats {
    salarie: String,
    jour: Jour,
}

/// La charge de `arret-enregistrer`. `connu` est la connaissance de l'accident, en la forme que lit
/// `obligations::connaissance_saisie` (`{jour, heure}`, celle de la route `arret-connaissance`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretEnregistrer {
    salarie: String,
    dernier_jour: Jour,
    debut: Jour,
    fin: Jour,
    motif: String,
    #[serde(default)]
    date_accident: Option<Jour>,
    #[serde(default)]
    connu: Option<crate::obligations::ConnaissanceDite>,
    #[serde(default)]
    contrat: Option<NumeroContrat>,
    #[serde(default)]
    jours_deja_indemnises: Option<u32>,
    #[serde(default)]
    note: Option<String>,
}

/// La charge de `arret-prolonger` : l'arret (son salarie, son dernier jour travaille) et sa nouvelle fin prescrite.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretProlonger {
    salarie: String,
    dernier_jour: Jour,
    fin: Jour,
}

/// La charge de `arret-reprise` : la reprise, exigee meme nulle (nulle, elle s'efface).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretReprise {
    salarie: String,
    dernier_jour: Jour,
    reprise: Option<Jour>,
    #[serde(default)]
    motif_reprise: Option<String>,
}

/// La charge de `arret-indemnites` : chaque montant, un decimal en texte (« 40.89 », tel que le releve l'ecrit et que
/// l'arret le garde), et les jours deja indemnises ; absent ou nul, rien ne change.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretIndemnites {
    salarie: String,
    dernier_jour: Jour,
    ij_brute: Option<String>,
    ij_nette: Option<String>,
    ij_brute_majoree: Option<String>,
    ij_nette_majoree: Option<String>,
    jours_deja_indemnises: Option<u32>,
}

/// La nature d'un signalement d'arret (S20.G00.05.001) : 04, l'arret ; 05, la reprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub(crate) enum NatureSignalement {
    #[default]
    #[serde(rename = "04")]
    Arret,
    #[serde(rename = "05")]
    Reprise,
}

impl NatureSignalement {
    pub(crate) fn code(self) -> &'static str {
        match self {
            NatureSignalement::Arret => signalement::ARRET,
            NatureSignalement::Reprise => signalement::REPRISE,
        }
    }
}

/// La charge de `arret-signaler`. `remplace` : l'identifiant du signalement qu'il annule et remplace (vide : aucun) ;
/// `ordre` : son numero d'ordre (0 ou absent : le suivant) ; `le` : son jour de constitution. Le geste qu'offre le rappel
/// d'un signalement l'ecrit aussi (`ChargeArretSignaler::de`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeArretSignaler {
    salarie: String,
    dernier_jour: Jour,
    #[serde(default)]
    nature: NatureSignalement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remplace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ordre: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    le: Option<Jour>,
}

impl ChargeArretSignaler {
    /// Le signalement de l'arret de `salarie` qui suit `dernier_jour`, de sa `nature` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(salarie: String, dernier_jour: Jour, nature: NatureSignalement) -> ChargeArretSignaler {
        ChargeArretSignaler { salarie, dernier_jour, nature, remplace: None, ordre: None, test: None, le: None }
    }
}

/// La charge de `arret-oublier`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretOublier {
    salarie: String,
    dernier_jour: Jour,
}

/// La charge de `suspension-enregistrer`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionEnregistrer {
    salarie: String,
    debut: Jour,
    fin: Jour,
    motif: String,
    #[serde(default)]
    contrat: Option<NumeroContrat>,
    #[serde(default)]
    note: Option<String>,
}

/// La charge de `suspension-modifier` : le debut la designe ; la fin et le motif, s'ils se disent, changent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionModifier {
    salarie: String,
    debut: Jour,
    #[serde(default)]
    fin: Option<Jour>,
    #[serde(default)]
    motif: Option<String>,
}

/// La charge de `suspension-oublier`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionOublier {
    salarie: String,
    debut: Jour,
}

/// La charge de `fin-contrat-signaler` (`remplace`, `ordre`, `test`, `le` : comme `arret-signaler`). Le geste qu'offre
/// le rappel d'une fin de contrat l'ecrit aussi (`ChargeFinContratSignaler::de`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeFinContratSignaler {
    mois: Mois,
    salarie: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remplace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ordre: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    le: Option<Jour>,
}

impl ChargeFinContratSignaler {
    /// Le signalement de la fin du contrat de `salarie` en `mois` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(mois: Mois, salarie: String) -> ChargeFinContratSignaler {
        ChargeFinContratSignaler { mois, salarie, remplace: None, ordre: None, test: None, le: None }
    }
}

/// La charge de `fin-contrat-documents`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFinContratDocuments {
    mois: Mois,
    salarie: String,
    #[serde(default)]
    le: Option<Jour>,
}

/// L'identifiant remplace d'un signalement : vide, aucun.
fn remplace_dit(r: &Option<String>) -> Option<String> {
    r.as_deref().filter(|t| !t.is_empty()).map(str::to_string)
}

// ------------------------------------------------------------------ les reponses

/// `imprevus` : les imprevus resumes, les nomenclatures pour en saisir, et ce que l'ecran lit de chacun.
#[derive(Debug, Serialize)]
struct ReponseImprevus {
    phrase: String,
    imprevus: Vec<Resume>,
    motifs_arret: Vec<CodeDuBloc60>,
    motifs_reprise: Vec<CodeDuBloc60>,
    motifs_suspension: Vec<MotifSuspension>,
    phrases: Vec<String>,
}

/// `arrets` : tous les arrets resumes, ceux qui restent a signaler, et les nomenclatures du bloc 60.
#[derive(Debug, Serialize)]
struct ReponseArrets {
    phrase: String,
    phrases: Vec<String>,
    arrets: Vec<Resume>,
    a_signaler: Vec<Resume>,
    motifs: Vec<CodeDuBloc60>,
    motifs_reprise: Vec<CodeDuBloc60>,
    subrogation: Vec<CodeDuBloc60>,
}

/// `fins-de-contrat`.
#[derive(Debug, Serialize)]
struct ReponseFins {
    phrase: String,
    mois: Mois,
    fins: Vec<FinASignaler>,
}

/// `arret-contrats`.
#[derive(Debug, Serialize)]
struct ReponseContrats {
    phrase: String,
    contrats: Vec<ContratAuJour>,
}

/// Ce que rend un geste sur un arret : l'arret resume, et ce qui se lit du geste.
#[derive(Debug, Serialize)]
struct ReponseArret {
    arret: Resume,
    phrase: String,
}

/// Ce que rend un geste sur une absence.
#[derive(Debug, Serialize)]
struct ReponseSuspension {
    suspension: Resume,
    phrase: String,
}

/// `arret-signaler` : le fichier ecrit.
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalement {
    fichier: String,
    phrase: String,
    nature: NatureSignalement,
    #[serde(rename = "type")]
    type_declaration: &'static str,
    ordre: i64,
    avertissements: Vec<String>,
}

/// `fin-contrat-signaler` : le fichier ecrit.
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalementFin {
    fichier: String,
    phrase: String,
    #[serde(rename = "type")]
    type_declaration: &'static str,
    ordre: i64,
    preavis: String,
    avertissements: Vec<String>,
}

/// `fin-contrat-documents` : les deux papiers ecrits.
#[derive(Debug, Serialize)]
struct ReponsePapiers {
    phrase: String,
    certificat: String,
    recu: String,
    dossier: String,
    avertissements: [&'static str; 3],
}

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::Imprevus => en_json(imprevus(ctx, lire_charge(charge)?)?),
        Route::Arrets => {
            let SansCharge {} = lire_charge(charge)?;
            en_json(arrets(ctx)?)
        }
        Route::FinsDeContrat => en_json(fins_de_contrat(ctx, lire_charge(charge)?)?),
        Route::ArretContrats => en_json(arret_contrats(ctx, lire_charge(charge)?)?),
        Route::ArretEnregistrer => {
            let c: ChargeArretEnregistrer = lire_charge(charge)?;
            let mut r = arret_enregistrer(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretReprise => {
            let c: ChargeArretReprise = lire_charge(charge)?;
            let mut r = arret_reprise(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretOublier => {
            let c: ChargeArretOublier = lire_charge(charge)?;
            let mut r = arret_oublier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretIndemnites => {
            let c: ChargeArretIndemnites = lire_charge(charge)?;
            let mut r = arret_indemnites(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretProlonger => {
            let c: ChargeArretProlonger = lire_charge(charge)?;
            let mut r = arret_prolonger(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretSignaler => en_json(signaler_l_arret(ctx, &lire_charge(charge)?, None)?),
        Route::SuspensionEnregistrer => {
            let c: ChargeSuspensionEnregistrer = lire_charge(charge)?;
            let mut r = suspension_enregistrer(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::SuspensionModifier => {
            let c: ChargeSuspensionModifier = lire_charge(charge)?;
            let mut r = suspension_modifier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::SuspensionOublier => {
            let c: ChargeSuspensionOublier = lire_charge(charge)?;
            let mut r = suspension_oublier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::FinContratSignaler => en_json(signaler_la_fin(ctx, &lire_charge(charge)?, false)?),
        Route::FinContratDocuments => en_json(fin_contrat_documents(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// APRES L'ECRITURE D'UN IMPREVU : les engagements du salarie dont des documents sont partis se revalident
/// (`engagement::etats::apres_un_imprevu`) ; une paie partie qui change suit le chemin de toute correction d'un mois parti,
/// et la phrase dit ce qu'il entraine, avec les mots de « Modifier l'engagement »
/// (`crate::routes::engagement::ce_que_le_changement_entraine`).
fn regulariser(ctx: &Contexte, nom: &str, phrase: &mut String) -> R<()> {
    use engagement::etats::Revalidation;
    let cle = cle_du_nom(ctx, nom)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let mut dit = Vec::new();
    for x in engagement::etats::apres_un_imprevu(ctx, &cle, &departs).map_err(Echec::from)? {
        match x {
            Revalidation::Changee(v) => {
                let lignes = crate::routes::engagement::ce_que_le_changement_entraine(ctx, &v, &departs)?;
                if !lignes.is_empty() {
                    dit.push(format!("La paie partie change : {}.", lignes.join(", ")));
                }
            }
            Revalidation::Attend(_, refus) => dit.push(format!("La paie partie attend : {}", refus)),
        }
    }
    if !dit.is_empty() {
        *phrase = format!("{} {}", phrase, dit.join(" "));
    }
    Ok(())
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn jour(v: &Value) -> R<Jour> {
    d(v).map_err(|m| erreur("ValueError", m))
}

fn magasin<T>(r: Result<T, String>) -> R<T> {
    r.map_err(refus_magasin)
}

/// UN IMPREVU EN SA FORME (`engagement::imprevus::Imprevu`), pour les regles du magasin : cette route le tient encore en
/// valeur.
pub(crate) fn forme(v: &Value) -> R<imprevus::Imprevu> {
    magasin(socle::forme::lire(v, imprevus::ILLISIBLE))
}

/// Un arret en sa forme (`engagement::imprevus::ArretSaisi`).
pub(crate) fn arret_de(v: &Value) -> R<imprevus::ArretSaisi> {
    magasin(socle::forme::lire(v, imprevus::ILLISIBLE))
}

/// Les imprevus du magasin, en valeurs telles que cette route les tient.
pub(crate) fn en_valeurs(l: Result<Vec<imprevus::Imprevu>, String>) -> R<Vec<Value>> {
    Ok(magasin(l)?.iter().map(socle::ecriture::en_valeur).collect())
}

/// La cle de la fiche que `nom` designe, comme partout dans le moteur (`Contexte::salarie` : la casse, les accents, le
/// prenom, le NIR ne comptent pas). Un nom qui ne designe personne se refuse avec la phrase de `Contexte::salarie`.
fn cle_du_nom(ctx: &Contexte, nom: &str) -> R<IdSalarie> {
    cle_du_salarie(ctx, &json!(nom))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`contrats_au`), celui que la charge
/// nomme (`voulu`, son numero), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: Option<&str>) -> R<ContratAuJour> {
    let candidats = contrats_au(ctx, cle, &jour.iso())?;
    let texte = |v: &Value| valeur_en_texte(v);
    if let Some(voulu) = voulu.filter(|v| !v.is_empty()) {
        return match candidats.iter().find(|c| c.numero == json!(voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| texte(&c.numero)).collect();
                Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    voulu,
                    cle,
                    jour.jj_mm_aaaa(),
                    if liste.is_empty() { "aucun".to_string() } else { liste.join(", ") }
                )))
            }
        };
    }
    match candidats.len() {
        1 => Ok(candidats[0].clone()),
        0 => Err(refus(format!(
            "Aucun contrat de {} ne couvre le {}. Un arret ou une suspension designe le contrat suspendu par son numero et sa \
date de debut (S21.G00.40.009 et .001) : sans contrat declare, il n'y a rien a suspendre. Verifier la date, ou saisir le \
contrat.",
            cle,
            jour.jj_mm_aaaa()
        ))),
        _ => {
            let liste: Vec<String> = candidats.iter().map(|c| format!("{} du {} au {}", texte(&c.numero), c.debut, c.fin)).collect();
            Err(refus(format!(
                "Deux contrats de {} couvrent le {} ({}). Le cahier technique laisse le choix au declarant : dire lequel.",
                cle,
                jour.jj_mm_aaaa(),
                liste.join(", ")
            )))
        }
    }
}

/// UN IMPREVU NE CHEVAUCHE PAS UN AUTRE IMPREVU DU MEME CONTRAT : un jour d'absence ne se retire qu'une fois du salaire,
/// et deux periodes qui se recouvrent sur le meme contrat sont deux declarations qui se contredisent (un salarie ne
/// travaille pas le dernier jour travaille d'un arret qui court deja). `sauf` : l'imprevu lui-meme, quand il change.
fn sans_chevauchement(ctx: &Contexte, nouveau: &Value, sauf: Option<&Value>) -> R<()> {
    let lu = forme(nouveau)?;
    let (p, dj) = (magasin(imprevus::premier_jour(&lu))?, magasin(imprevus::dernier_jour(&lu))?);
    let cle = valeur_en_texte(&nouveau["salarie"]);
    let debut = jour(&nouveau["contrat_debut"])?;
    let numero = valeur_en_texte(&nouveau["contrat_numero"]);
    for autre in magasin(imprevus::tous(&ctx.racine))? {
        let v = socle::ecriture::en_valeur(&autre);
        if sauf.is_some_and(|s| meme_imprevu(s, &v)) || meme_imprevu(nouveau, &v) {
            continue;
        }
        if !magasin(imprevus::du_contrat(&autre, &cle, Some(numero.as_str()), &debut))? {
            continue;
        }
        let (ap, ad) = (magasin(imprevus::premier_jour(&autre))?, magasin(imprevus::dernier_jour(&autre))?);
        if ap <= dj && ad >= p {
            return Err(refus(format!(
                "{} : du {} au {}, cet imprevu chevauche {} du {} au {} sur le meme contrat ({}). Un jour d'absence ne se \
declare et ne se retire qu'une fois : corriger les dates, ou reprendre l'autre imprevu.",
                cle,
                p.jj_mm_aaaa(),
                dj.jj_mm_aaaa(),
                if imprevus::est_un_arret(&autre) { "l'arret de travail" } else { "la suspension" },
                ap.jj_mm_aaaa(),
                ad.jj_mm_aaaa(),
                if numero.is_empty() { debut.jj_mm_aaaa() } else { numero.clone() }
            )));
        }
    }
    Ok(())
}

/// Le meme imprevu : meme genre, meme salarie, meme jour de rangement.
fn meme_imprevu(a: &Value, b: &Value) -> bool {
    let rangement = |v: &Value| forme(v).ok().map(|i| (i.genre(), imprevus::jour_de_rangement(&i).ok()));
    a["salarie"] == b["salarie"] && rangement(a) == rangement(b)
}

/// L'imprevu de ce genre, de ce salarie, a ce jour de rangement (le dernier jour travaille d'un arret, le debut d'une
/// suspension) : son indice et le fichier de son mois, tous genres compris.
fn trouver(ctx: &Contexte, genre: &str, cle: &str, jour: &Jour) -> R<Option<(usize, Vec<Value>)>> {
    let lus = magasin(imprevus::du_fichier(&ctx.racine, &jour.mois_civil().to_string()))?;
    let i = lus.iter().position(|x| x.genre() == genre && x.salarie() == cle && imprevus::jour_de_rangement(x).ok() == Some(*jour));
    Ok(i.map(|i| (i, lus.iter().map(socle::ecriture::en_valeur).collect())))
}

/// L'arret de ce salarie a ce dernier jour travaille : (son indice, le fichier de son mois, tous imprevus compris).
fn l_arret(ctx: &Contexte, cle: &str, dernier: &Jour) -> R<(usize, Vec<Value>)> {
    trouver(ctx, ARRET, cle, dernier)?.ok_or_else(|| {
        refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}. La route `arrets` les liste.", cle, dernier.jj_mm_aaaa()))
    })
}

/// Ecrit le fichier d'un mois du magasin des imprevus (`imprevus/AAAA-MM.json`) : les arrets ET les suspensions qu'il
/// range, tels que la liste les donne.
fn ecrire(ctx: &Contexte, mois: &str, liste: Vec<Value>) -> R<()> {
    let liste = liste.iter().map(forme).collect::<R<Vec<_>>>()?;
    imprevus::ecrire(&ctx.racine, mois, liste).map_err(|m| erreur("OSError", m))
}

/// Ce qui est parti d'un imprevu (`obligations::ce_qui_est_parti` : les envois du journal, tels qu'il les lit).
fn ses_envois(ctx: &Contexte, imp: &Value) -> R<Vec<crate::obligations::EnvoiDeLArret>> {
    crate::obligations::ce_qui_est_parti(ctx, imp)
}

/// Les mois des DSN mensuelles qui ont reporte l'imprevu (nature 01).
fn reporte_par(ctx: &Contexte, imp: &Value) -> R<Vec<String>> {
    Ok(ses_envois(ctx, imp)?.into_iter().filter(|x| x.nature == "01").filter_map(|x| x.mois).collect())
}

/// Un signalement de cette nature (04, 05) est-il parti pour cet arret ?
fn envoye(ctx: &Contexte, imp: &Value, nature: &str) -> R<bool> {
    Ok(ses_envois(ctx, imp)?.iter().any(|x| x.nature == nature))
}

/// Le jour ou le signalement NORMAL (type 01) de cette nature (04, 05) est parti pour cet arret, s'il l'est
/// (`obligations::ce_qui_est_parti` : un depot reel accepte, que le bilan n'a pas refuse).
fn parti_le(ctx: &Contexte, imp: &Value, nature: &str) -> R<Option<Jour>> {
    let envois = ses_envois(ctx, imp)?;
    let premier = envois.iter().filter(|x| x.nature == nature && x.type_declaration == signalement::NORMALE).map(|x| x.quand.as_str()).min().map(str::to_string);
    premier.map(|q| Jour::lire(&q.chars().take(10).collect::<String>()).map_err(|m| erreur("ValueError", m))).transpose()
}

/// Le nom d'un signalement : `<base>.dsn` ; un annule-et-remplace, `<base>_AR<ordre>.dsn`, a cote du signalement parti
/// qu'il remplace, qui reste tel qu'il est parti (regle 4).
fn nom_du_signalement(base: &str, annule_et_remplace: bool, ordre: i64) -> String {
    if annule_et_remplace {
        format!("{}_AR{}.dsn", base, ordre)
    } else {
        format!("{}.dsn", base)
    }
}

/// UN ARRET EN COURS : celui dont on peut encore apprendre qu'il se prolonge ou que le salarie a repris plus
/// tot. Aucune reprise n'est saisie, et :
///
/// - sa fin previsionnelle n'est pas passee : le salarie est arrete aujourd'hui ;
/// - ou elle est passee, mais la DSN mensuelle du mois ou elle tombe n'est pas encore partie : l'avis de prolongation peut
///   arriver apres la fin prescrite, et une reprise anticipee se signale encore. Une fois cette DSN partie, ou la DSN
///   suivante qui reporte un arret saisi apres elle (`reporte_par`),
///   l'arret y est declare fini a sa fin previsionnelle -- « la reprise du travail de votre salarie a la date prevue doit
///   etre signalee dans la DSN mensuelle » (ameli, `atelier/reference/sources/ameli-arret-de-travail/`) --, et il n'est
///   plus en cours.
///
/// Les routes elles-memes ne s'en servent pas pour refuser : une prolongation se saisit tant qu'aucune reprise ne l'est
/// (`arret_prolonger`). C'est ce que la page offre, pas ce que le moteur admet.
fn en_cours(ctx: &Contexte, imp: &Value) -> R<bool> {
    if vrai(&imp["reprise"]) {
        return Ok(false);
    }
    let fin = jour(&imp["fin_previsionnelle"])?;
    if fin >= ctx.aujourdhui {
        return Ok(true);
    }
    Ok(!reporte_par(ctx, imp)?.iter().any(|m| *m >= fin.mois_civil().to_string()))
}

// ------------------------------------------------------------------ le resume

/// UN IMPREVU TEL QUE LA PAGE LE LIT : un arret (`ResumeDArret`) ou une absence (`ResumeDeSuspension`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Resume {
    Arret(Box<ResumeDArret>),
    Suspension(Box<ResumeDeSuspension>),
}

/// UN ARRET RESUME, tel que le lisent la liste des imprevus et la DSN qui le reporte (`compagnie::depart`) : ses champs
/// tels que l'arret les ecrit, ce qui en est parti (`signalements` : 04 et 05 ; `reporte_par` : les mois des DSN
/// mensuelles), et l'etat de son signalement.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArretResume {
    salarie: Value,
    contrat: Value,
    contrat_debut: Value,
    motif: Value,
    pub motif_libelle: String,
    dernier_jour_travaille: Value,
    debut: Value,
    fin_previsionnelle: Value,
    subrogation: Value,
    subrogation_debut: Value,
    subrogation_fin: Value,
    date_accident: Value,
    reprise: Value,
    reprise_motif: Value,
    note: Value,
    saisi_le: Value,
    pub signale: bool,
    signalement_du: bool,
    signalements: Vec<crate::obligations::EnvoiDeLArret>,
    reporte_par_dit: Vec<String>,
    reporte_par: Vec<String>,
    etat: EtatDuSignalement,
}

/// Ou en est le signalement d'un arret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum EtatDuSignalement {
    #[serde(rename = "signale")]
    Signale,
    #[serde(rename = "a signaler")]
    ASignaler,
    #[serde(rename = "sans signalement")]
    SansSignalement,
}

impl EtatDuSignalement {
    /// Tel que l'ecran le dit.
    fn dit(self) -> &'static str {
        match self {
            EtatDuSignalement::Signale => "signalé",
            EtatDuSignalement::ASignaler => "à signaler",
            EtatDuSignalement::SansSignalement => "sans signalement",
        }
    }
}

/// UN ARRET TEL QUE LA PAGE LE LIT : le resume de toujours (`ArretResume`), plus `genre`, `premier_jour`, `dernier_jour`,
/// `jours_deja_indemnises`, les indemnites par jour (`ij_brute`, `ij_nette`, `ij_brute_majoree`, `ij_nette_majoree`),
/// `origine`, `ij_attendues` (l'arret ouvre le maintien), `ij_proposees` (le montant de principe, majore pour un accident,
/// et sa phrase, tant que l'arret ne dit pas le sien), `ij_majoree_attendue` (un accident du travail au-dela du
/// vingt-huitieme jour, CSS R433-3), `jours_deja_indemnises_attendus`, `cachets`, ce qu'il ouvre a la page, ses phrases
/// (`Dit`), puis, dans les listes, ses `obligations` et ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDArret {
    #[serde(flatten)]
    base: ArretResume,
    genre: &'static str,
    premier_jour: String,
    dernier_jour: String,
    jours_deja_indemnises: Value,
    ij_brute: Value,
    ij_nette: Value,
    ij_brute_majoree: Value,
    ij_nette_majoree: Value,
    origine: Option<droit::maintien::Origine>,
    ij_attendues: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_proposees: Option<IjProposees>,
    ij_majoree_attendue: bool,
    jours_deja_indemnises_attendus: bool,
    cachets: Vec<Cachet>,
    prolongations: Value,
    en_cours: bool,
    peut_prolonger: bool,
    peut_reprendre_plus_tot: bool,
    reprise_corrigeable: bool,
    #[serde(flatten)]
    dit: Dit,
    #[serde(skip_serializing_if = "Option::is_none")]
    obligations: Option<Vec<crate::obligations::Obligation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parti: Option<Vec<Parti>>,
}

/// UNE ABSENCE TELLE QUE LA PAGE LA LIT : `genre`, `salarie`, `contrat`, `contrat_debut`, `motif`, `motif_libelle`,
/// `remunere`, `debut`, `fin`, `premier_jour`, `dernier_jour`, `note`, `saisi_le`, `reporte_par`, `cachets`, ses phrases
/// (`Dit`), puis, dans la liste, ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDeSuspension {
    genre: &'static str,
    salarie: Value,
    contrat: Value,
    contrat_debut: Value,
    motif: Value,
    motif_libelle: String,
    remunere: bool,
    debut: Value,
    fin: Value,
    premier_jour: String,
    dernier_jour: String,
    note: Value,
    saisi_le: Value,
    reporte_par_dit: Vec<String>,
    reporte_par: Vec<String>,
    cachets: Vec<Cachet>,
    #[serde(flatten)]
    dit: Dit,
    #[serde(skip_serializing_if = "Option::is_none")]
    parti: Option<Vec<Parti>>,
}

/// LE MONTANT DE PRINCIPE des indemnites d'un arret (`engagement::maintien::ij_de_principe`), ou la phrase qui dit ou le
/// lire.
#[derive(Debug, Clone, Serialize)]
struct IjProposees {
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_brute: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_nette: Option<String>,
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_brute_majoree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_nette_majoree: Option<String>,
}

/// Un jour paye pendant l'imprevu (`imprevus::cachets_dans`, tel qu'il le rend), avec son mois et chacun de ses jours en
/// phrases.
#[derive(Debug, Clone, Serialize)]
struct Cachet {
    #[serde(flatten)]
    lu: Map<String, Value>,
    mois_dit: String,
    jours: Vec<JourPaye>,
}

#[derive(Debug, Clone, Serialize)]
struct JourPaye {
    iso: Value,
    phrase: String,
    phrase_du_mois: String,
}

/// CE QUE L'ECRAN LIT D'UN IMPREVU, en phrases que la page ne compose pas : ce qu'il est (`quoi`), sa periode
/// (`periode` ; un arret sans reprise dit sa fin prescrite), son contrat (`contrat_dit`), sa reprise (`reprise_dit`), le
/// debut d'une absence (`depuis_dit`), ce qui est paye pendant (`paye_pendant`), et les signalements deja partis
/// (`deja_signale`, par nature).
#[derive(Debug, Clone, Serialize)]
struct Dit {
    quoi: String,
    periode: String,
    contrat_dit: Option<String>,
    reprise_dit: Option<String>,
    depuis_dit: Option<String>,
    paye_pendant: &'static str,
    reprise_titre: &'static str,
    signaler_titre: SignalerTitre,
    deja_signale: DejaSignale,
}

#[derive(Debug, Clone, Serialize)]
struct SignalerTitre {
    #[serde(rename = "04")]
    arret: &'static str,
    #[serde(rename = "05")]
    reprise: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct DejaSignale {
    #[serde(rename = "04", skip_serializing_if = "Option::is_none")]
    arret: Option<String>,
    #[serde(rename = "05", skip_serializing_if = "Option::is_none")]
    reprise: Option<String>,
}

/// Un morceau de ce qui est parti d'un imprevu, ou de ce qui en reste du (`pastille`).
#[derive(Debug, Clone, Serialize)]
struct Parti {
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pastille: Option<crate::routes::Pastille>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discret: Option<bool>,
}

/// Un jour ISO, tel qu'une phrase le dit.
fn jour_dit(iso: &str) -> String {
    crate::routes::jour_en_clair(&json!(iso))
}

impl Dit {
    fn de(arret: bool, libelle: &str, reprise: &Value, premier: &str, dernier: &str, contrat: &Value, debut: &Value, signalements: &[crate::obligations::EnvoiDeLArret]) -> Dit {
        let jour = |v: &Value| crate::routes::jour_en_clair(v);
        let deja = |nature: &str| {
            let n = signalements.iter().filter(|s| s.nature == nature).count();
            (n > 0).then(|| format!("Déjà signalé {} fois", n))
        };
        Dit {
            quoi: if arret { format!("Arrêt de travail ({})", libelle) } else { libelle.to_string() },
            periode: if arret && !vrai(reprise) {
                format!("depuis le {}, prescrit jusqu'au {}", jour_dit(premier), jour_dit(dernier))
            } else {
                format!("du {} au {}", jour_dit(premier), jour_dit(dernier))
            },
            contrat_dit: vrai(contrat).then(|| format!("contrat {}", valeur_en_texte(contrat))),
            reprise_dit: vrai(reprise).then(|| format!("reprise le {}", jour(reprise))),
            depuis_dit: vrai(debut).then(|| format!("Depuis le {}", jour(debut))),
            paye_pendant: if arret { "Payé pendant l'arrêt" } else { "Payé pendant l'absence" },
            reprise_titre: if vrai(reprise) { "La reprise" } else { "Reprise plus tôt" },
            signaler_titre: SignalerTitre { arret: "Signaler l'arrêt", reprise: "Signaler la reprise" },
            deja_signale: DejaSignale { arret: deja("04"), reprise: deja("05") },
        }
    }
}

/// Un cachet paye pendant l'imprevu : son mois tel que l'ecran le lit (« de novembre 2026 »), et chacun de ses jours.
fn cachet_dit(c: Value) -> Cachet {
    let mois_dit = crate::regularisation::de_mois_en_clair(&valeur_en_texte(&c["mois"]));
    let libelle = valeur_en_texte(&c["libelle"]);
    let jours = c["dates"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|iso| {
            let le = format!("{} le {}", libelle, crate::routes::jour_en_clair(iso));
            JourPaye { iso: iso.clone(), phrase_du_mois: format!("{}, {}", le, mois_dit), phrase: le }
        })
        .collect();
    let lu = match c {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    Cachet { lu, mois_dit, jours }
}

/// CE QUI EST PARTI D'UN IMPREVU, ET CE QUI EN RESTE DU : une ligne courte par morceau, dans l'ordre ou l'ecran les lit --
/// les prolongations, les signalements, la DSN qui l'a declare, puis chaque obligation, faite ou due (avec sa pastille) ;
/// « rien n'est encore parti » sinon.
fn ce_qui_est_parti(prolongations: &Value, signalements: &[crate::obligations::EnvoiDeLArret], reporte_par_dit: &[String], obligations: &[crate::obligations::Obligation]) -> Vec<Parti> {
    let jour = |v: &Value| crate::routes::jour_en_clair(v);
    let minuscule = |t: String| -> String { t.chars().take(1).flat_map(char::to_lowercase).chain(t.chars().skip(1)).collect() };
    let dite = |phrase: String| Parti { phrase, pastille: None, discret: None };
    let mut parti = Vec::new();
    for p in prolongations.as_array().into_iter().flatten().filter(|p| p["fin"] != p["fin_avant"]) {
        parti.push(dite(format!("prolongé jusqu'au {} (avis du {})", jour(&p["fin"]), jour(&p["le"]))));
    }
    for s in signalements {
        let quoi = if s.nature == "05" { "reprise signalée" } else if s.type_declaration == "03" { "signalement rectifié" } else { "signalé" };
        parti.push(dite(format!("{} le {}", quoi, crate::routes::quand(&s.quand))));
    }
    if !reporte_par_dit.is_empty() {
        parti.push(dite(format!("déclaré par la DSN {}", reporte_par_dit.join(", "))));
    }
    for o in obligations {
        let libelle = minuscule(o.libelle.clone());
        match &o.fait {
            Some(f) if !f.le().is_empty() && matches!(o.ou, Some(crate::obligations::Ou::Lieux(_))) => {
                parti.push(dite(format!("{}, faite le {}", libelle, crate::routes::quand(f.le()))));
            }
            // Une obligation due sans date butoir (son delai ne se compte pas) dit son etat, pas une butoir absente.
            _ if o.du => parti.push(Parti { pastille: Some(o.pastille.clone()), ..dite(format!("{}, {}", libelle, o.butoir_dit.clone().unwrap_or_else(|| o.pastille.mots.clone()))) }),
            _ => {}
        }
    }
    if parti.is_empty() {
        parti.push(Parti { discret: Some(true), ..dite("rien n'est encore parti".to_string()) });
    }
    parti
}

impl Resume {
    /// Les obligations d'un arret (`obligations::de_l_arret`), posees sur lui ; une absence n'en a pas.
    pub(crate) fn poser_obligations(&mut self, obligations: Vec<crate::obligations::Obligation>) {
        if let Resume::Arret(a) = self {
            a.obligations = Some(obligations);
        }
    }

    /// Ce qui est parti de l'imprevu (`ce_qui_est_parti`), une fois ses obligations posees.
    fn dire_ce_qui_est_parti(&mut self) {
        match self {
            Resume::Arret(a) => {
                a.parti = Some(ce_qui_est_parti(&a.prolongations, &a.base.signalements, &a.base.reporte_par_dit, a.obligations.as_deref().unwrap_or(&[])));
            }
            Resume::Suspension(s) => s.parti = Some(ce_qui_est_parti(&Value::Null, &[], &s.reporte_par_dit, &[])),
        }
    }

    /// L'imprevu en lignes, tel que `imprevus` le dit : qui, quel contrat, quoi, quand, ou en est son signalement, puis
    /// chacune de ses obligations.
    fn en_lignes(&self) -> Vec<String> {
        let mot = crate::routes::mot;
        let quand = crate::routes::quand;
        let (salarie, contrat, libelle, premier, dernier, etat, obligations) = match self {
            Resume::Arret(a) => (&a.base.salarie, &a.base.contrat, &a.base.motif_libelle, &a.premier_jour, &a.dernier_jour, a.base.etat.dit(), a.obligations.as_deref()),
            Resume::Suspension(s) => (&s.salarie, &s.contrat, &s.motif_libelle, &s.premier_jour, &s.dernier_jour, "", None),
        };
        let mut l = vec![format!("{} ({}) : {}, du {} au {} — {}", mot(salarie), mot(contrat), libelle, quand(premier), quand(dernier), etat)];
        if let Some(o) = obligations {
            l.extend(crate::obligations::en_phrases(o).into_iter().map(|o| format!("  {}", o)));
        }
        l
    }
}

/// UN IMPREVU TEL QUE LA PAGE LE LIT (`Resume`), avec les cachets qui tombent dedans.
pub(crate) fn resume(ctx: &Contexte, n: &Bloc60, imp: &Value) -> R<Resume> {
    let lu = forme(imp)?;
    let cachets: Vec<Cachet> = magasin(imprevus::cachets_dans(ctx, &lu))?.iter().map(|c| cachet_dit(socle::ecriture::en_valeur(c))).collect();
    let premier = magasin(imprevus::premier_jour(&lu))?.iso();
    let dernier = magasin(imprevus::dernier_jour(&lu))?.iso();
    if imprevus::est_un_arret(&lu) {
        let base = resume_arret(ctx, n, imp)?;
        let motif = magasin(droit::arrets::motif_d_arret(&valeur_en_texte(&imp["motif"])))?;
        let ouvert = ouvre_le_maintien(ctx, imp)?;
        let majoree = ouvert
            && motif.as_ref().and_then(|m| m.origine).is_some_and(|o| o.professionnelle() || o == droit::maintien::Origine::Trajet)
            && magasin(imprevus::dernier_jour(&lu))?.moins(&magasin(imprevus::premier_jour(&lu))?) >= 28;
        // LE MONTANT DE PRINCIPE, propose tant que l'arret ne dit pas le sien (`engagement::maintien::ij_de_principe`) : la
        // page en remplit les cases, la personne le garde ou le corrige ; sinon, la phrase dit ou le lire.
        let ij_proposees = if ouvert && !vrai(&imp["ij_brute"]) {
            Some(match magasin(engagement::maintien::ij_de_principe(ctx, &arret_de(imp)?))? {
                Ok(ij) => IjProposees {
                    ij_brute: Some(ij.brute.to_string()),
                    ij_nette: Some(ij.nette.to_string()),
                    phrase: ij.phrase,
                    ij_brute_majoree: ij.majorees.map(|(b, _)| b.to_string()),
                    ij_nette_majoree: ij.majorees.map(|(_, n)| n.to_string()),
                },
                Err(phrase) => IjProposees { ij_brute: None, ij_nette: None, phrase, ij_brute_majoree: None, ij_nette_majoree: None },
            })
        } else {
            None
        };
        // CE QUE L'ARRET OUVRE A LA PAGE (rien de remplissable s'il n'y a pas a le remplir) :
        // un arret en cours se prolonge ou se reprend plus tot ; une reprise saisie se corrige tant que son signalement
        // n'est pas parti.
        let cours = en_cours(ctx, imp)?;
        let reprise_corrigeable = vrai(&imp["reprise"]) && !envoye(ctx, imp, signalement::REPRISE)?;
        let dit = Dit::de(true, &base.motif_libelle, &base.reprise, &premier, &dernier, &base.contrat, &base.debut, &base.signalements);
        return Ok(Resume::Arret(Box::new(ResumeDArret {
            base,
            genre: ARRET,
            premier_jour: premier,
            dernier_jour: dernier,
            jours_deja_indemnises: imp["jours_deja_indemnises"].clone(),
            ij_brute: imp["ij_brute"].clone(),
            ij_nette: imp["ij_nette"].clone(),
            ij_brute_majoree: imp["ij_brute_majoree"].clone(),
            ij_nette_majoree: imp["ij_nette_majoree"].clone(),
            origine: motif.and_then(|m| m.origine),
            ij_attendues: ouvert,
            ij_proposees,
            ij_majoree_attendue: majoree,
            jours_deja_indemnises_attendus: ouvert,
            cachets,
            prolongations: imp["prolongations"].clone(),
            en_cours: cours,
            peut_prolonger: cours,
            peut_reprendre_plus_tot: cours,
            reprise_corrigeable,
            dit,
            obligations: None,
            parti: None,
        })));
    }
    let reportee = reporte_par(ctx, imp)?;
    let motif = magasin(imprevus::motif_de_suspension(&valeur_en_texte(&imp["motif"])))?;
    let libelle = motif.as_ref().map_or_else(|| "?".to_string(), |m| m.libelle.clone());
    let dit = Dit::de(false, &libelle, &Value::Null, &premier, &dernier, &imp["contrat_numero"], &imp["debut"], &[]);
    Ok(Resume::Suspension(Box::new(ResumeDeSuspension {
        genre: SUSPENSION,
        salarie: imp["salarie"].clone(),
        contrat: imp["contrat_numero"].clone(),
        contrat_debut: imp["contrat_debut"].clone(),
        motif: imp["motif"].clone(),
        motif_libelle: libelle,
        remunere: motif.as_ref().is_some_and(|m| m.remunere),
        debut: imp["debut"].clone(),
        fin: imp["fin"].clone(),
        premier_jour: premier,
        dernier_jour: dernier,
        note: imp["note"].clone(),
        saisi_le: imp["saisi_le"].clone(),
        reporte_par_dit: mois_de_la_dsn(&reportee),
        reporte_par: reportee,
        cachets,
        dit,
        parti: None,
    })))
}

/// Un motif d'absence qui se declare, tel que la page le propose.
#[derive(Debug, Serialize)]
struct MotifSuspension {
    code: String,
    libelle: String,
    libelle_dit: String,
    remunere: bool,
}

/// Les motifs d'une suspension qui se DECLARENT (`permanents-base.json`, section `suspensions`, sans le 200).
fn motifs_de_suspension() -> R<Vec<MotifSuspension>> {
    let regles = droit::permanents::regles().map_err(refus_magasin)?;
    Ok(regles
        .suspensions
        .motifs
        .iter()
        .filter(|m| m.ne_se_declare_pas.is_none())
        .map(|m| {
            let libelle_dit = format!("{}{}", m.libelle, if m.remunere { " (payé)" } else { "" });
            MotifSuspension { code: m.code.clone(), libelle: m.libelle.clone(), libelle_dit, remunere: m.remunere }
        })
        .collect())
}

// ------------------------------------------------------------------ imprevus

/// La route `imprevus` : les imprevus (d'un salarie, qui croisent un mois), resumes -- un arret avec ses `obligations` --,
/// et les nomenclatures pour en saisir.
fn imprevus(ctx: &Contexte, c: ChargeImprevus) -> R<ReponseImprevus> {
    let cle = c.salarie.as_deref().filter(|s| !s.is_empty()).map(|nom| cle_du_nom(ctx, nom)).transpose()?;
    let bornes = c.mois.map(|m| (m.premier_jour(), m.dernier_jour()));
    let n = nomenclatures()?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for lu in magasin(imprevus::tous(&ctx.racine))? {
        let imp = socle::ecriture::en_valeur(&lu);
        if cle.as_deref().is_some_and(|c| valeur_en_texte(&imp["salarie"]) != c) {
            continue;
        }
        if let Some((debut, fin)) = &bornes {
            if !magasin(imprevus::croise(&lu, debut, fin))? {
                continue;
            }
        }
        let mut r = resume(ctx, &n, &imp)?;
        // CE QU'UN ARRET FAIT NAITRE, SUR L'ARRET LUI-MEME : la page des imprevus (la carte d'un salarie, la
        // colonne du mois, son dossier) lit une seule liste, et chaque arret y porte ses obligations, calculees au meme
        // endroit que pour la route `arrets` (`obligations::de_l_arret`).
        if imprevus::est_un_arret(&lu) {
            r.poser_obligations(crate::obligations::de_l_arret(ctx, &imp, &suivi, ctx.aujourdhui)?);
        }
        r.dire_ce_qui_est_parti();
        out.push(r);
    }
    Ok(ReponseImprevus {
        phrase: format!("Imprévus : {}.", out.len()),
        phrases: out.iter().flat_map(Resume::en_lignes).collect(),
        imprevus: out,
        motifs_arret: n.motifs_arret.saisissables(),
        motifs_reprise: n.motifs_reprise.saisissables(),
        motifs_suspension: motifs_de_suspension()?,
    })
}

// ------------------------------------------------------------------ le maintien

/// `arret-indemnites`. Les indemnites journalieres d'un arret, par jour, telles que les ecrit le releve de la caisse ou
/// son bordereau de paiement (BPIJ : le prix unitaire de l'« I.J. normale » ou « majoree », puis les retenues CSG et RDS,
/// `atelier/reference/sources/subrogation-indemnites-journalieres/bpij-edi/`) : brutes, et nettes de CSG et CRDS. Le
/// maintien de salaire s'en deduit (`engagement::maintien`) ; avant le releve, le resume en propose le montant de principe
/// (`ij_proposees`), que la page met dans ses cases. Un champ absent ou nul ne change pas. La page ne les demande que
/// pour un arret qui ouvre le maintien (`ij_attendues` du resume) ; ailleurs, elles ne se deduisent de rien.
fn arret_indemnites(ctx: &Contexte, c: &ChargeArretIndemnites) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let montants = [("ij_brute", &c.ij_brute), ("ij_nette", &c.ij_nette), ("ij_brute_majoree", &c.ij_brute_majoree), ("ij_nette_majoree", &c.ij_nette_majoree)];
    if montants.iter().all(|(_, v)| v.is_none()) && c.jours_deja_indemnises.is_none() {
        return Err(refus("Dire les indemnités journalières (brute et nette, par jour) ou les jours déjà indemnisés : rien n'est changé."));
    }
    let mut nouveau = liste[i].clone();
    for (k, v) in montants {
        let Some(t) = v else { continue };
        let x = documents::valeur::dec_avec_echelle(&json!(t.trim())).map_err(|e| refus(format!("Indemnité journalière ({}) : {}", k, e)))?;
        if x <= Decimal::ZERO {
            return Err(refus("Une indemnité journalière est un montant par jour, positif : rien n'est changé."));
        }
        nouveau[k] = json!(socle::num::eur2(x).to_string());
    }
    if let Some(j) = c.jours_deja_indemnises {
        nouveau["jours_deja_indemnises"] = json!(j);
    }
    imprevus::verifier(&socle::forme::lire(&nouveau, imprevus::ILLISIBLE).map_err(refus)?).map_err(refus)?;
    liste[i] = nouveau.clone();
    ecrire(ctx, &dernier.mois_civil().to_string(), liste)?;
    reponse_arret(ctx, &nouveau, "Indemnités journalières notées.")
}

/// Le contrat qu'un imprevu suspend, lu dans la saisie qui le porte ; `None` s'il n'y est plus.
fn contrat_de(ctx: &Contexte, imp: &Value) -> R<Option<ContratAuJour>> {
    let debut = valeur_en_texte(&imp["contrat_debut"]);
    let contrats = contrats_au(ctx, &valeur_en_texte(&imp["salarie"]), &debut)?;
    Ok(contrats.into_iter().find(|c| c.debut == debut))
}

/// La nature du contrat qu'un imprevu suspend ; `None` s'il n'y est plus.
fn nature_du_contrat(ctx: &Contexte, imp: &Value) -> R<Option<std::sync::Arc<droit::natures::Nature>>> {
    let Some(c) = contrat_de(ctx, imp)? else { return Ok(None) };
    // Une nature se dit, elle ne se devine pas (regle 6) : celle d'un contrat repris se dit apres l'import (`import::nature`).
    let Some(cle) = c.nature.as_str().filter(|n| !n.is_empty()) else {
        return Err(refus(format!("{} : la nature de son contrat du {} n'est pas encore dite.", c.nom, c.debut)));
    };
    Ok(Some(droit::natures::nature(cle).map_err(refus)?))
}

/// L'arret ouvre-t-il le maintien de salaire, par la loi ou la convention (`engagement::maintien::droits`) ?
fn ouvre_le_maintien(ctx: &Contexte, imp: &Value) -> R<bool> {
    let Some(nat) = nature_du_contrat(ctx, imp)? else { return Ok(false) };
    Ok(magasin(engagement::maintien::droits(ctx, &arret_de(imp)?, &nat))?.is_some_and(|d| d.du()))
}

/// LA SUBROGATION D'UN ARRET, telle que le moteur la tire de la convention et du choix de la compagnie
/// (`engagement::maintien::subrogation`) : `01` et ses dates, ou `02`.
pub(crate) fn subrogation_de(ctx: &Contexte, imp: &Value) -> R<(Value, Value, Value)> {
    let sub = match nature_du_contrat(ctx, imp)? {
        Some(nat) => magasin(engagement::maintien::subrogation(ctx, &arret_de(imp)?, &nat))?,
        None => None,
    };
    Ok(match sub {
        Some((d, f)) => (json!("01"), json!(d.iso()), json!(f.iso())),
        None => (json!("02"), Value::Null, Value::Null),
    })
}

// ------------------------------------------------------------------ la prolongation

/// `arret-prolonger` : L'AVIS DE PROLONGATION D'UN ARRET EN COURS.
///
/// CE QUI PART, ET CE QUI NE PART PAS. « En cas de prolongation d'arret, il ne faut pas envoyer de nouveau signalement de
/// nature " arret de travail ". Il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle, au niveau de la
/// rubrique " Date de fin previsionnelle - S21.G00.60.003 " » (fiche net-entreprises 1983, `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l.
/// 13) ; ameli dit de meme a l'employeur en DSN : « modifier la date de fin previsionnelle de l'arret de travail dans votre
/// logiciel de paie » (`atelier/reference/sources/ameli-arret-de-travail/`). La route ne change donc que
/// `fin_previsionnelle` : le bloc S21.G00.60 de chaque DSN mensuelle que l'arret traverse la porte (`dsn::bloc_60`,
/// `imprevus::du_mois`, qui suit la nouvelle fin), la paie d'un permanent retire les jours ajoutes et le maintien s'ouvre
/// aux mois ajoutes (`imprevus::lignes_du_mois`, `maintien`), les cachets payes dans la prolongation s'avertissent
/// (`imprevus::cachets_dans`) -- par le mecanisme de toujours, sans second. Un signalement d'arret pas encore parti partira
/// avec la nouvelle fin.
///
/// LA SUBROGATION. Elle n'est portee que par le signalement d'arret (60.004 a 60.008 : C en 01, mais le moteur ne l'ecrit
/// qu'en 04 -- `dsn::bloc_60`, tableau des usages) ; sa fin « maximale prevue dans le cadre de la convention collective ou
/// de l'accord de branche » se declare d'emblee (fiche 1983, l. 25). Si elle est « insuffisante ou erronee », un nouveau
/// signalement d'arret de type annule et remplace la porte (l. 37), « le plus rapidement possible des connaissance de la
/// prolongation » (l. 66) ; si elle « court toujours », rien (l. 110). Le moteur tire ce maximum du droit
/// (`engagement::maintien::subrogation`) : il ne bouge qu'avec une subrogation qui dure tant que la caisse indemnise (1285,
/// X.4.7), et suit alors la nouvelle fin. Une fin de subrogation changee apres le
/// depot du signalement d'arret fait naitre le signalement rectificatif (`obligations::de_l_arret`,
/// `signalement_arret_rectificatif`), d'ou la trace de chaque avis dans `prolongations`.
///
/// CE QUI REFUSE, sans rien ecrire : un arret deja repris (« se prolonge sans interruption », ameli : un arret apres la
/// reprise est un nouvel arret) ; une fin qui n'est pas apres la fin prescrite (une fin plus tot est une reprise
/// anticipee, qui se signale : `arret-reprise`) ; ce que la norme refuse au signalement d'arret (`verifier`) ; un
/// chevauchement.
fn arret_prolonger(ctx: &Contexte, c: &ChargeArretProlonger) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    if vrai(&avant["reprise"]) {
        return Err(refus(format!(
            "L'arrêt de {} a pris fin : le salarié a repris le {}. Une prolongation continue un arrêt en cours, sans \
interruption ; un arrêt qui commence après la reprise est un nouvel arrêt, qui se saisit et se signale comme tel. Rien \
n'est changé.",
            cle,
            jour(&avant["reprise"])?.jj_mm_aaaa()
        )));
    }
    let f = c.fin;
    let fin_avant = jour(&avant["fin_previsionnelle"])?;
    if f < fin_avant {
        return Err(refus(format!(
            "Le {} est avant la fin prescrite de l'arrêt (le {}) : ce n'est pas une prolongation. Si le salarié a repris \
plus tôt, c'est une reprise anticipée, qui se saisit comme telle et se signale (nature 05). Rien n'est changé.",
            f.jj_mm_aaaa(),
            fin_avant.jj_mm_aaaa()
        )));
    }
    if f == fin_avant {
        return Err(refus(format!("L'arrêt est déjà prescrit jusqu'au {} : rien n'est changé.", f.jj_mm_aaaa())));
    }
    let mut a = avant.clone();
    a["fin_previsionnelle"] = json!(f.iso());
    // Une subrogation qui dure tant que la caisse indemnise (1285, X.4.7) suit la fin de l'arret ; les autres vont deja
    // jusqu'au maximum du maintien (`engagement::maintien::subrogation`).
    if signalement_subroge(&a)? {
        let (sub, _, sub_fin) = subrogation_de(ctx, &a)?;
        if sub == json!("01") {
            a["subrogation_fin"] = sub_fin;
        }
    }
    verifier(&a)?;
    sans_chevauchement(ctx, &a, Some(&avant))?;

    // La trace de l'avis : le jour ou la compagnie l'a su (le jour du moteur), l'heure de la saisie, et ce qui a change.
    let trace = json!({
        "le": ctx.aujourdhui.iso(),
        "saisi_le": maintenant(),
        "qui": INTERFACE,
        "fin_avant": avant["fin_previsionnelle"], "fin": a["fin_previsionnelle"],
        "subrogation_fin_avant": avant["subrogation_fin"], "subrogation_fin": a["subrogation_fin"],
    });
    match a["prolongations"].as_array_mut() {
        Some(l) => l.push(trace),
        None => a["prolongations"] = json!([trace]),
    }
    liste[i] = a.clone();
    ecrire(ctx, &dernier.mois_civil().to_string(), liste)?;

    let nouvelle_fin = f;
    let mut avertissements: Vec<String> = Vec::new();
    avertissements.push(format!(
        "Prolongé jusqu'au {}. Rien ne part : pas de nouveau signalement d'arrêt ; la DSN mensuelle de chaque mois que \
l'arrêt traverse porte la nouvelle fin prévisionnelle (S21.G00.60.003, fiche net-entreprises 1983).",
        nouvelle_fin.jj_mm_aaaa()
    ));
    // Les jours ajoutes qui tombent dans un mois dont la DSN reelle est deja partie : elle portait l'ancienne fin.
    let declares = reporte_par(ctx, &avant)?;
    for m in Mois::entre(&fin_avant.plus_jours(1).map_err(|e| erreur("ValueError", e))?, &nouvelle_fin).iter().map(Mois::to_string) {
        if declares.iter().any(|x| *x == m) {
            avertissements.push(format!(
                "La DSN de {} est déjà partie avec l'ancienne fin (le {}) : la paie de ce mois, où les jours ajoutés \
tombent, est à corriger.",
                Mois::en_clair(&m),
                fin_avant.jj_mm_aaaa()
            ));
        }
    }
    if a["subrogation_fin"] != avant["subrogation_fin"] && envoye(ctx, &avant, signalement::ARRET)? {
        avertissements.push(
            "Le signalement d'arrêt est déjà parti avec l'ancienne fin de subrogation : un signalement d'arrêt annule et \
remplace porte la nouvelle, le plus vite possible (fiche net-entreprises 1983). Il paraît dans ce qui est dû."
                .to_string(),
        );
    }
    reponse_arret(ctx, &a, &avertissements.join("  ·  "))
}

/// L'arret est-il subroge (S21.G00.60.004 = oui) ?
fn signalement_subroge(a: &Value) -> R<bool> {
    declarations::signalement::avec_subrogation(&declarations::dsn::Arret { v: a.clone() }).map_err(|m| erreur("ValueError", m))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &Value, contrat_fin: &str) -> R<()> {
    imprevus::verifier_motif_de_suspension(&valeur_en_texte(&s["motif"]), &valeur_en_texte(&s["salarie"])).map_err(refus)?;
    let (debut, fin) = (jour(&s["debut"])?, jour(&s["fin"])?);
    if fin < debut {
        return Err(refus(format!(
            "Suspension du {} au {} : elle finit avant de commencer (S21.G00.65.002 et .003).",
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        )));
    }
    if !contrat_fin.is_empty() && fin.iso().as_str() > contrat_fin {
        return Err(refus(format!(
            "Suspension jusqu'au {}, mais le contrat {} finit le {} : une suspension ne survit pas a son contrat.",
            fin.jj_mm_aaaa(),
            valeur_en_texte(&s["contrat_numero"]),
            jour(&json!(contrat_fin))?.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// `suspension-enregistrer`.
fn suspension_enregistrer(ctx: &Contexte, c: &ChargeSuspensionEnregistrer) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let contrat = contrat_suspendu(ctx, &cle, &debut, c.contrat.as_ref().map(NumeroContrat::as_str))?;
    let mut s = Map::new();
    s.insert("genre".into(), json!(SUSPENSION));
    s.insert("salarie".into(), json!(cle));
    s.insert("contrat_debut".into(), json!(contrat.debut));
    s.insert("contrat_numero".into(), contrat.numero.clone());
    s.insert("motif".into(), json!(c.motif));
    s.insert("debut".into(), json!(debut.iso()));
    s.insert("fin".into(), json!(c.fin.iso()));
    s.insert("saisi_le".into(), json!(maintenant()));
    s.insert("qui".into(), json!(INTERFACE));
    s.insert("note".into(), json!(c.note.clone().unwrap_or_default()));
    let s = Value::Object(s);
    verifier_suspension(&s, &contrat.fin)?;
    if trouver(ctx, SUSPENSION, &cle, &debut)?.is_some() {
        return Err(refus(format!(
            "Une suspension de {} au {} est deja enregistree : la modifier plutot que d'en saisir une seconde.",
            cle,
            debut.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &s, None)?;
    let mois = debut.mois_civil().to_string();
    let mut liste = en_valeurs(imprevus::du_fichier(&ctx.racine, &mois))?;
    liste.push(s.clone());
    ecrire(ctx, &mois, liste)?;
    reponse_suspension(ctx, &s, "Absence enregistrée. Chaque mois qu'elle traverse la déclarera.")
}

/// `suspension-modifier`. Le debut designe la suspension et ne change pas : une suspension qui commence un autre jour
/// s'oublie et se saisit de nouveau.
fn suspension_modifier(ctx: &Contexte, c: &ChargeSuspensionModifier) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    let mut s = avant.clone();
    if let Some(f) = c.fin {
        s["fin"] = json!(f.iso());
    }
    if let Some(m) = &c.motif {
        s["motif"] = json!(m);
    }
    let fin_du_contrat = contrat_de(ctx, &s)?.map(|c| c.fin).unwrap_or_default();
    verifier_suspension(&s, &fin_du_contrat)?;
    sans_chevauchement(ctx, &s, Some(&avant))?;
    liste[i] = s.clone();
    ecrire(ctx, &debut.mois_civil().to_string(), liste)?;
    reponse_suspension(ctx, &s, "Absence changée.")
}

/// `suspension-oublier`. Une suspension qu'une DSN reelle a deja declaree ne se retire pas : le magasin garderait moins
/// que ce qui est parti.
fn suspension_oublier(ctx: &Contexte, c: &ChargeSuspensionOublier) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let envois = ses_envois(ctx, &liste[i])?.len();
    if envois > 0 {
        return Err(refus(format!(
            "Cette suspension a deja ete declaree ({} DSN). La retirer du magasin n'annule rien aupres des organismes : la \
reprendre (une fin plus tot), ou corriger les DSN deposees.",
            envois
        )));
    }
    let s = liste.remove(i);
    ecrire(ctx, &debut.mois_civil().to_string(), liste)?;
    reponse_suspension(ctx, &s, "Absence retirée : aucune déclaration ne l'avait portée.")
}

/// Ce que la norme refuse (`SignalementImpossible`, une
/// `ValueError`) ; les avertissements ne servent pas ici.
fn verifier(arret: &Value) -> R<()> {
    match signalement::verifier(&declarations::dsn::Arret { v: arret.clone() }, signalement::ARRET, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(erreur("ValueError", m)),
        Err(signalement::Echec::Autre(m)) => Err(Echec { classe: None, message: m }),
    }
}

/// Le dernier jour travaille saisi de l'arret de ce salarie dont un signalement a declare `declare` (S21.G00.60.002,
/// `imprevus::dernier_jour_declare`) : ce qui designe l'arret aux routes ; `declare` lui-meme si aucun arret ne le porte.
pub(crate) fn dernier_jour_saisi(ctx: &Contexte, salarie: &str, declare: &str) -> R<Jour> {
    for a in tous_les_arrets(ctx)? {
        if valeur_en_texte(&a["salarie"]) == salarie && magasin(imprevus::dernier_jour_declare(&arret_de(&a)?))?.iso() == declare {
            return jour(&a["dernier_jour_travaille"]);
        }
    }
    jour(&json!(declare))
}

/// `{"arret": resume, "phrase"}` : l'arret tel que la page le lit (`resume`, avec les cachets qui tombent dedans), et ce
/// qui se lit du geste.
fn reponse_arret(ctx: &Contexte, arret: &Value, phrase: &str) -> R<ReponseArret> {
    Ok(ReponseArret { arret: resume(ctx, &nomenclatures()?, arret)?, phrase: phrase.to_string() })
}

/// `{"suspension": resume, "phrase"}`.
fn reponse_suspension(ctx: &Contexte, s: &Value, phrase: &str) -> R<ReponseSuspension> {
    Ok(ReponseSuspension { suspension: resume(ctx, &nomenclatures()?, s)?, phrase: phrase.to_string() })
}

/// UN ARRET TEL QUE LE MAGASIN L'ECRIT (`engagement::imprevus::ArretSaisi`) : chaque cle y est, nulle quand l'arret n'a pas
/// encore de valeur (la subrogation, que le droit tire ensuite ; la reprise ; les indemnites journalieres).
pub(crate) fn arret_a_ecrire(
    cle: &str,
    contrat: &ContratAuJour,
    motif: &str,
    dernier_jour: &Jour,
    debut: &Jour,
    fin: &Jour,
    date_accident: Option<Jour>,
    jours_deja_indemnises: Option<u32>,
    note: &str,
) -> Value {
    json!({
        "genre": ARRET, "salarie": cle, "contrat_debut": contrat.debut, "contrat_numero": contrat.numero, "motif": motif,
        "dernier_jour_travaille": dernier_jour.iso(), "debut": debut.iso(), "fin_previsionnelle": fin.iso(),
        "subrogation": null, "subrogation_debut": null, "subrogation_fin": null, "reprise": null, "reprise_motif": null,
        "date_accident": date_accident.map(|j| j.iso()), "jours_deja_indemnises": jours_deja_indemnises,
        "ij_brute": null, "ij_nette": null, "ij_brute_majoree": null, "ij_nette_majoree": null,
        "prolongations": [], "reprises": [], "saisi_le": maintenant(), "qui": INTERFACE, "note": note,
    })
}

/// `arret-enregistrer`. Les controles de la norme sont ceux du signalement (`signalement::verifier`, nature 04), avant
/// toute ecriture ; un arret ne chevauche pas un autre imprevu du meme contrat (`sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, c: &ChargeArretEnregistrer) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let choisi = contrat_suspendu(ctx, &cle, &dernier, c.contrat.as_ref().map(NumeroContrat::as_str))?;
    let mut arret = arret_a_ecrire(&cle, &choisi, &c.motif, &dernier, &c.debut, &c.fin, c.date_accident, c.jours_deja_indemnises, c.note.as_deref().unwrap_or_default());
    // Les jours deja indemnises (D1226-4) ne se disent que pour un arret qui ouvre le maintien de salaire.
    if c.jours_deja_indemnises.is_some() && !ouvre_le_maintien(ctx, &arret)? {
        return Err(refus(
            "Les jours deja indemnises (D1226-4) ne servent qu'au maintien de salaire, que cet arret n'ouvre pas. Rien n'est ecrit.",
        ));
    }
    // LA SUBROGATION SE TIRE DU DROIT : la convention qui la donne, ou le choix de la compagnie, et la duree maximale du
    // maintien pour ses dates (`engagement::maintien::subrogation`).
    let (sub, sub_debut, sub_fin) = subrogation_de(ctx, &arret)?;
    arret["subrogation"] = sub;
    arret["subrogation_debut"] = sub_debut;
    arret["subrogation_fin"] = sub_fin;
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;
    // Le jour où la compagnie a su l'accident, point de départ des 48 heures de la déclaration (R441-3) : contrôlé ici,
    // écrit avec l'arrêt.
    let connu = match &c.connu {
        Some(dite) => Some(crate::obligations::connaissance_saisie(ctx, &arret, crate::obligations::DeQuoi::Accident, dite)?),
        None => None,
    };

    let mois = dernier.mois_civil().to_string();
    if trouver(ctx, ARRET, &cle, &dernier)?.is_some() {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            cle,
            dernier.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &arret, None)?;
    let mut liste = en_valeurs(imprevus::du_fichier(&ctx.racine, &mois))?;
    liste.push(arret.clone());
    ecrire(ctx, &mois, liste)?;
    if let Some(saisie) = connu {
        crate::obligations::connaissance_ecrire(ctx, &arret, crate::obligations::DeQuoi::Accident, saisie)?;
    }
    reponse_arret(ctx, &arret, "Arrêt enregistré.")
}

/// `arret-reprise` : une reprise nulle l'efface.
fn arret_reprise(ctx: &Contexte, c: &ChargeArretReprise) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (i, mut liste) = l_arret(ctx, &cle, &c.dernier_jour)?;
    let avant = liste[i].clone();
    let reprise = json!(c.reprise.map(|j| j.iso()));
    let motif = json!(c.motif_reprise.as_deref().filter(|m| !m.is_empty()));
    // La trace de la saisie : quand, et la reprise d'avant et d'apres (`etats_d_alors` la defait).
    let trace = json!({
        "le": ctx.aujourdhui.iso(),
        "saisi_le": maintenant(),
        "qui": INTERFACE,
        "reprise_avant": avant["reprise"], "reprise_motif_avant": avant["reprise_motif"],
        "reprise": reprise, "reprise_motif": motif,
    });
    if let Value::Object(a) = &mut liste[i] {
        a.insert("reprise".into(), reprise);
        a.insert("reprise_motif".into(), motif);
        match a.get_mut("reprises").and_then(Value::as_array_mut) {
            Some(l) => l.push(trace),
            None => {
                a.insert("reprises".into(), json!([trace]));
            }
        }
    }
    let arret = liste[i].clone();
    verifier(&arret)?;
    ecrire(ctx, &c.dernier_jour.mois_civil().to_string(), liste)?;
    reponse_arret(ctx, &arret, if c.reprise.is_some() { "Reprise notée." } else { "Reprise effacée." })
}

/// `arret-oublier`. Un arret deja declare ne se retire pas.
fn arret_oublier(ctx: &Contexte, c: &ChargeArretOublier) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (i, mut liste) = l_arret(ctx, &cle, &c.dernier_jour)?;
    let envois = ses_envois(ctx, &liste[i])?.len();
    if envois > 0 {
        return Err(refus(format!(
            "Cet arret a deja ete declare ({} envoi(s)). Le retirer du magasin n'annule rien aupres des organismes et ferait disparaitre la trace de ce qui est parti : emettre un annule-et-remplace.",
            envois
        )));
    }
    let arret = liste.remove(i);
    ecrire(ctx, &c.dernier_jour.mois_civil().to_string(), liste)?;
    reponse_arret(ctx, &arret, "Arrêt retiré : aucune déclaration ne l'avait porté.")
}

// ------------------------------------------------------------------ la fin d'un contrat

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn validees(ctx: &Contexte, mois: &str) -> R<(Value, Vec<Paie>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    engagement::etats::validees_du_mois(ctx, mois, &departs).map_err(plain)?.map(|(d, p)| (socle::ecriture::en_valeur(&d), p)).ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
}

/// La carte d'un engagement de la saisie que cette route lit encore en valeur (`engagement::saisie::Carte`) ; une carte qui
/// ne se lit pas refuse.
fn carte_de(e: &Value) -> R<engagement::saisie::Carte> {
    socle::forme::lire(e, engagement::saisie::CARTE_ILLISIBLE).map_err(plain)
}

/// L'engagement brut avec ses bornes : un contrat aux cachets ne les ecrit pas, il commence son premier jour travaille et
/// finit son dernier (`engagement::debut_brut`, `engagement::dates_engagement`).
fn avec_ses_bornes(e: &Value) -> R<Value> {
    let mut e = e.clone();
    let jour = |j: &engagement::DateDeLigne| j.jour().to_string();
    if !e.get("debut").is_some_and(vrai) {
        let debut = carte_de(&e)?.debut_lu();
        if !debut.is_empty() {
            e["debut"] = json!(debut);
        }
    }
    if !e.get("fin").is_some_and(vrai) {
        if let Some(fin) = carte_de(&e)?.dates().iter().map(jour).max() {
            e["fin"] = json!(fin);
        }
    }
    Ok(e)
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, avec ses bornes (`avec_ses_bornes`), et son rang dans la
/// saisie. Un engagement se rattache a sa fiche comme sa paie (`engagement::cle_de_l_engagement`), quelle que soit la
/// casse du nom saisi. `nom` : comme on l'a dit, pour la phrase. Un contrat qui a un motif de rupture passe avant les
/// autres. Un salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit(saisie: &Value, cle: &str, nom: &str, mois: &str) -> R<(usize, Value)> {
    let eng = saisie.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut siens: Vec<(usize, Value)> = Vec::new();
    for (i, x) in eng.iter().enumerate() {
        if carte_de(x)?.salarie.as_deref() == Some(cle) {
            siens.push((i, avec_ses_bornes(x)?));
        }
    }
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucun engagement en {} : aucun contrat ne s'y termine.", nom, Mois::en_clair(mois))));
    }
    let finit = |x: &Value| x.get("fin").and_then(Value::as_str).is_some_and(|f| f.starts_with(mois));
    let rompu = siens.iter().position(|(_, x)| finit(x) && x.get("motif_rupture").is_some_and(vrai));
    let i = rompu.or_else(|| siens.iter().position(|(_, x)| finit(x))).ok_or_else(|| {
        refus(format!("{} n'a aucun contrat qui se termine en {}.", nom, Mois::en_clair(mois)))
    })?;
    Ok(siens.into_iter().nth(i).expect("rang lu dans la meme liste"))
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `validees`, au meme rang que sa saisie).
fn paie_de_l_engagement(paies: &[Paie], rang: usize, cle: &str, nom: &str, mois: &str) -> R<Paie> {
    paies
        .get(rang)
        .filter(|p| p.salarie.id == cle)
        .cloned()
        .ok_or_else(|| refus(format!("{} n'a aucune paie validee en {} pour ce contrat.", nom, Mois::en_clair(mois))))
}

/// `fin-contrat-documents` : le certificat de travail et le recu pour solde de tout compte (`documents::fin_de_contrat`)
/// de l'engagement du salarie qui finit ce mois.
fn fin_contrat_documents(ctx: &Contexte, c: ChargeFinContratDocuments) -> R<ReponsePapiers> {
    let mois = c.mois.to_string();
    let (saisie, paies) = validees(ctx, &mois)?;
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (rang, _) = engagement_qui_finit(&saisie, &cle, &c.salarie, &mois)?;
    let p = paie_de_l_engagement(&paies, rang, &cle, &c.salarie, &mois)?;
    let date = c.le.unwrap_or(ctx.aujourdhui);
    let dossier = ctx.racine.join("sorties").join(&mois);
    let [certificat, recu] = documents::fin_de_contrat::ecrire(ctx, &p, date, &dossier)?;
    Ok(ReponsePapiers {
        phrase: format!("Écrits pour {} : le certificat de travail et le reçu pour solde de tout compte.", crate::routes::nom_a_l_ecran(&p.salarie)),
        certificat: certificat.to_string_lossy().into_owned(),
        recu: recu.to_string_lossy().into_owned(),
        dossier: dossier.to_string_lossy().into_owned(),
        avertissements: documents::fin_de_contrat::AVIS,
    })
}

/// `fctu.FinDeContrat` que le signalement declarera, batie sur la saisie (`fin-contrat-signaler`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Value, mois: &str) -> R<declarations::fctu::FinDeContrat> {
    let manque = ce_qui_manque(ctx, e)?;
    let e = &fin_au_terme(e)?;
    if !manque.is_empty() {
        let numero = e.get("numero").filter(|n| vrai(n)).map(en_texte).unwrap_or_else(|| "?".to_string());
        let liste: Vec<String> = manque.iter().map(phrase_du_manque).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne s'écrit pas encore ; il manque, dans la carte de l'engagement, sous « Contrat » :\n  {}",
            numero,
            liste.join("\n  ")
        )));
    }
    let preavis = e.get("preavis").filter(|p| vrai(p)).cloned().unwrap_or_else(|| json!({}));
    let cle = |k: &str| e.get(k).cloned().unwrap_or(Value::Null);
    let fin = e.get("fin").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("fin")))?.clone();
    let motif = e.get("motif_rupture").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("motif_rupture")))?.clone();
    let type_preavis = preavis.get("type").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("type")))?.clone();
    let v = json!({
        "fin": fin, "motif": motif, "mois_rattachement": mois,
        "type_preavis": type_preavis,
        "preavis_debut": preavis.get("debut").cloned().unwrap_or(Value::Null),
        "preavis_fin": preavis.get("fin").cloned().unwrap_or(Value::Null),
        "notification": cle("notification_rupture"),
        "signature_convention": cle("signature_convention"),
        "engagement_licenciement": cle("engagement_licenciement"),
        "transaction": cle("transaction"),
    });
    declarations::fctu::FinDeContrat::lire(&v).map_err(plain)
}

/// L'echec d'un signalement de fin de contrat ou d'amorcage : une regle de la norme qui s'oppose
/// (`FctuImpossible`, `AmorcageImpossible`, des `ValueError`) ou autre chose.
fn echec_fctu(e: declarations::fctu::Echec) -> Echec {
    match e {
        declarations::fctu::Echec::Impossible(m) | declarations::fctu::Echec::Autre(m) => erreur("ValueError", m),
    }
}

/// LE SIGNALEMENT DE FIN DE CONTRAT, refait pour se lire (`compagnie::depart::refaire_le_signalement`) : sa charge, telle
/// que `fin-contrat-signaler` la lit.
pub(crate) fn signaler_fin(ctx: &Contexte, charge: &Value, refaire: bool) -> R<ReponseSignalementFin> {
    signaler_la_fin(ctx, &lire_charge(charge)?, refaire)
}

/// LE SIGNALEMENT DE FIN DE CONTRAT, ecrit dans `sorties/AAAA-MM/`. Rien n'est depose : le depot est un acte a part. Un
/// signalement normal deja parti pour ce contrat ne se refait pas : toute modification posterieure « doit faire l'objet
/// d'un signalement « Annule et remplace » ou « Annule » » (cahier technique DSN, `norme-dsn/texte/chapitre-5-modeles.txt`,
/// 5.2) ; la route le dit et n'ecrit rien. `refaire` : la piece partie, refaite pour se lire
/// (`compagnie::depart::refaire_le_signalement`). L'annule-et-remplace a son nom (`_AR<ordre>`) : le fichier parti reste
/// tel qu'il est parti.
pub(crate) fn signaler_la_fin(ctx: &Contexte, c: &ChargeFinContratSignaler, refaire: bool) -> R<ReponseSignalementFin> {
    let mois = c.mois.to_string();
    let salarie = c.salarie.as_str();
    let remplace = remplace_dit(&c.remplace);
    let envoi_test = c.test == Some(true);

    let (saisie, paies) = validees(ctx, &mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(&saisie, &cle, salarie, &mois)?;
    let fin = fin_de_contrat_declaree(ctx, &e, &mois)?;
    if remplace.is_none() && !refaire {
        let journal = envois::journal::lu(&ctx.racine).map_err(plain)?;
        if let Some(le) = crate::obligations::fin_signalee_le(&journal.lignes, &cle, &jour(&json!(fin.fin))?) {
            return Err(refus(format!(
                "Le signalement de fin de contrat de {} est parti le {} : il ne se refait pas. Le corriger : un annule et remplace (champ « remplace »). Rien n'est écrit.",
                cle,
                le.jj_mm_aaaa()
            )));
        }
    }
    let paie = paie_de_l_engagement(&paies, rang, &cle, salarie, &mois)?;
    let type_declaration = if remplace.is_some() { declarations::fctu::ANNULE_ET_REMPLACE } else { declarations::fctu::NORMALE };
    let mut avertissements = declarations::fctu::verifier(&fin, &paie.engagement, type_declaration).map_err(echec_fctu)?;
    // LE NUMERO D'ORDRE : celui qu'on donne, sinon (0) le suivant de tous les signalements deja partis (p. 136), lu une
    // fois la saisie verifiee : la plate-forme ne se consulte pas pour un refus.
    let (ordre, plate_forme) = match c.ordre.unwrap_or(0) {
        0 => prochain_ordre(ctx)?,
        n => (n, Vec::new()),
    };
    avertissements.extend(plate_forme);
    let nom = nom_du_signalement(&format!("SignalementFinContrat_{}_{}", cle, fin.fin.replace('-', "")), remplace.is_some(), ordre);
    let cible = ctx.racine.join("sorties").join(&mois).join(&nom);
    crate::compagnie::depart::refuser_si_partie(&ctx.racine, &cible)?;
    let options = declarations::fctu::Options {
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace: remplace.clone(),
        envoi_test,
        date_constitution: c.le,
    };
    // Les arrets du contrat qui croisent la periode du signalement : le FCTU les reporte (bloc 60), comme la mensuelle,
    // tires du magasin des imprevus ; ses suspensions (bloc 65) en viennent deja (`imprevus::suspensions_du_mois`).
    let (debut_p, fin_p) = (paie.engagement.debut_periode().map_err(plain)?, paie.engagement.fin_periode().map_err(plain)?);
    let arrets: Vec<declarations::dsn::Arret> = imprevus::arrets_du_contrat(ctx, &paie, &debut_p, &fin_p)
        .map_err(refus_magasin)?
        .into_iter()
        .map(|a| declarations::dsn::Arret { v: socle::ecriture::en_valeur(&a) })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets, &envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
    let preavis = declarations::fctu::libelle_preavis(&fin.type_preavis).map_err(plain)?;
    Ok(ReponseSignalementFin {
        fichier: cible.to_string_lossy().into_owned(),
        phrase: phrase_du_signalement(&format!("Signalement de fin de contrat écrit (préavis : {}). Rien n'est parti.", preavis), &avertissements),
        type_declaration,
        ordre,
        preavis,
        avertissements,
    })
}

/// Ce qui se lit d'un signalement écrit : ce qui est fait, puis ses avis, sur la même ligne.
fn phrase_du_signalement(fait: &str, avertissements: &[String]) -> String {
    std::iter::once(fait.to_string()).chain(avertissements.iter().cloned()).collect::<Vec<_>>().join("  ·  ")
}

/// CE QU'ETAIT L'ARRET A L'INSTANT `instant` (« AAAA-MM-JJ HH:MM », `routes::maintenant`) : chaque saisie d'apres se
/// defait par la trace qu'elle a laissee, de la plus recente a la plus ancienne -- une prolongation (`prolongations`) rend
/// la fin prescrite et la fin de subrogation d'avant, une reprise (`reprises`) la reprise et son motif d'avant. Une saisie
/// de la minute meme a pu preceder l'instant ou le suivre : chacun de ces etats est rendu, du plus recent au plus ancien ;
/// l'empreinte de ce qui est parti dit lequel c'etait (`compagnie::depart::refaire_le_signalement`).
pub(crate) fn etats_d_alors(ctx: &Contexte, salarie: &str, dernier_jour: &Jour, instant: &str) -> R<Vec<Value>> {
    let (i, liste) = l_arret(ctx, &cle_du_nom(ctx, salarie)?, dernier_jour)?;
    let mut a = liste[i].clone();
    let quand = |t: &Value| t.get("saisi_le").and_then(Value::as_str).unwrap_or("").to_string();
    let mut traces: Vec<(&str, Value)> =
        ["prolongations", "reprises"].iter().flat_map(|k| a[*k].as_array().into_iter().flatten().map(move |t| (*k, t.clone()))).collect();
    traces.sort_by_key(|(_, t)| quand(t));
    let apres = traces.iter().filter(|(_, t)| quand(t).as_str() > instant).count();
    let memes = traces.iter().filter(|(_, t)| quand(t) == instant).count();
    let mut etats = Vec::new();
    if apres == 0 {
        etats.push(a.clone());
    }
    for (n, (k, t)) in traces.iter().rev().take(apres + memes).enumerate() {
        let defaits: &[(&str, &str)] = if *k == "prolongations" {
            &[("fin_previsionnelle", "fin_avant"), ("subrogation_fin", "subrogation_fin_avant")]
        } else {
            &[("reprise", "reprise_avant"), ("reprise_motif", "reprise_motif_avant")]
        };
        for (champ, avant) in defaits {
            a[*champ] = t[*avant].clone();
        }
        if n + 1 >= apres {
            etats.push(a.clone());
        }
    }
    Ok(etats)
}

/// Le numero d'ordre (`S20.G00.05.004`) du prochain signalement : un de plus que le plus grand de tous ceux que
/// la compagnie a deja envoyes, arrets, reprises, fins de contrat et amorcages confondus. Cahier technique
/// DSN, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit unique quel que
/// soit le mois d'emission » (et non plus le rang parmi les envois de CET arret, qui repartait a 1). LA PLATE-FORME
/// FAIT FOI : `crate::compagnie::releve::dernier_ordre_connu` la consulte ; ce qu'il faut en dire (elle n'a pas
/// repondu, elle connaissait plus que le journal) part avec les avertissements.
fn prochain_ordre(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let connu = crate::compagnie::releve::dernier_ordre_connu(ctx, None)?;
    Ok((connu.dernier + 1, connu.avertissements))
}

/// LE SIGNALEMENT D'ARRET, refait pour se lire (`compagnie::depart::refaire_le_signalement`) depuis l'arret tel qu'il etait
/// a son ecriture (`alors`, l'un de `etats_d_alors`) : sa charge, telle que `arret-signaler` la lit.
pub(crate) fn signaler_arret(ctx: &Contexte, charge: &Value, alors: &Value) -> R<ReponseSignalement> {
    signaler_l_arret(ctx, &lire_charge(charge)?, Some(alors))
}

/// LE SIGNALEMENT D'ARRET (04) OU DE REPRISE (05), ecrit dans `sorties/AAAA-MM/`. Rien n'est depose : le depot est un
/// acte a part. Un signalement normal de cette nature deja parti pour cet arret ne se refait pas : une prolongation n'en
/// appelle pas de nouveau, « il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle » (fiche
/// net-entreprises 1983, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l. 13) ; ce qui le corrige est un
/// annule et remplace (l. 37). La route le dit et n'ecrit rien. `alors` : la piece partie, refaite pour se lire
/// (`compagnie::depart::refaire_le_signalement`) depuis l'arret tel qu'il etait a son ecriture (`etats_d_alors`) : une
/// prolongation ou une reprise saisie depuis ne la change pas. L'annule-et-remplace a son nom (`_AR<ordre>`) : le
/// fichier parti reste tel qu'il est parti.
pub(crate) fn signaler_l_arret(ctx: &Contexte, c: &ChargeArretSignaler, alors: Option<&Value>) -> R<ReponseSignalement> {
    use declarations::signalement::Echec as Ech;
    let nature = c.nature.code();
    let remplace = remplace_dit(&c.remplace);
    let arret = match alors {
        Some(a) => a.clone(),
        None => {
            let (i, liste) = l_arret(ctx, &cle_du_nom(ctx, &c.salarie)?, &c.dernier_jour)?;
            liste[i].clone()
        }
    };
    if remplace.is_none() && alors.is_none() {
        if let Some(le) = parti_le(ctx, &arret, nature)? {
            let quoi = match c.nature {
                NatureSignalement::Arret => ("d'arrêt", " ; une prolongation ne se signale pas, la DSN mensuelle porte la nouvelle fin"),
                NatureSignalement::Reprise => ("de reprise", ""),
            };
            return Err(refus(format!(
                "Le signalement {} de {} est parti le {} : il ne se refait pas{}. Le corriger : un annule et remplace (champ « remplace »). Rien n'est écrit.",
                quoi.0,
                en_texte(&arret["salarie"]),
                le.jj_mm_aaaa(),
                quoi.1
            )));
        }
    }
    let type_declaration = if remplace.is_some() { signalement::ANNULE_ET_REMPLACE } else { signalement::NORMALE };
    // `ordre` (`--ordre N`) : le rang voulu ; sinon le suivant. `le` (`--le`) : la date de constitution ; sinon le jour du contexte.
    let constitution = c.le.unwrap_or(ctx.aujourdhui);
    let dsn_arret = declarations::dsn::Arret { v: arret.clone() };
    let echec = |e: Ech| match e {
        Ech::Impossible(m) | Ech::Autre(m) => erreur("ValueError", m),
    };
    let mut avertissements = signalement::verifier(&dsn_arret, nature, Some(&constitution)).map_err(echec)?;
    let (ordre, plate_forme) = match c.ordre.unwrap_or(0) {
        0 => prochain_ordre(ctx)?,
        n => (n, Vec::new()),
    };
    avertissements.extend(plate_forme);

    let jour = en_texte(&arret["dernier_jour_travaille"]);
    let mois: String = jour.chars().take(7).collect();
    let nom = nom_du_signalement(
        &format!("Signalement{}_{}_{}", if c.nature == NatureSignalement::Arret { "Arret" } else { "Reprise" }, en_texte(&arret["salarie"]), jour.replace('-', "")),
        type_declaration == signalement::ANNULE_ET_REMPLACE,
        ordre,
    );
    let cible = ctx.racine.join("sorties").join(&mois).join(&nom);
    crate::compagnie::depart::refuser_si_partie(&ctx.racine, &cible)?;
    let options = signalement::Options {
        nature: nature.to_string(),
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace,
        envoi_test: c.test == Some(true),
        date_constitution: Some(constitution),
    };
    let texte = signalement::generer(ctx, &dsn_arret, &options, &envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?).map_err(echec)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
    let fait = match c.nature {
        NatureSignalement::Arret => "Signalement d'arrêt écrit. Rien n'est parti.",
        NatureSignalement::Reprise => "Signalement de reprise écrit. Rien n'est parti.",
    };
    Ok(ReponseSignalement {
        fichier: cible.to_string_lossy().into_owned(),
        phrase: phrase_du_signalement(fait, &avertissements),
        nature: c.nature,
        type_declaration,
        ordre,
        avertissements,
    })
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

// -------------------------------------------------------------------- arrets

/// Un refus du magasin des imprevus (`imprevus`) : un fichier abime, un genre inconnu, le dossier `arrets/` d'avant.
#[track_caller]
pub(crate) fn refus_magasin(m: String) -> Echec {
    erreur("ErreurOperation", m)
}

/// Les arrets de travail du magasin des imprevus (`imprevus/AAAA-MM.json`, genre `arret`), dans l'ordre des mois du
/// magasin, chacun ecrit entier (`engagement::imprevus::ArretSaisi`).
pub(crate) fn tous_les_arrets(ctx: &Contexte) -> R<Vec<Value>> {
    en_valeurs(imprevus::tous(&ctx.racine).map(|l| l.into_iter().filter(imprevus::est_un_arret).collect()))
}

/// Les nomenclatures du bloc 60 (`arrets-base.json`), lues par leur forme (`droit::arrets::bloc_60`).
pub(crate) fn nomenclatures() -> R<Bloc60> {
    droit::arrets::bloc_60().map_err(plain)
}

/// Le libelle du motif d'arret dans les codes de la norme, `?` s'il n'y est pas.
fn libelle_motif(n: &Bloc60, motif: &Value) -> R<String> {
    if matches!(motif, Value::Array(_) | Value::Object(_)) {
        return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(motif))));
    }
    Ok(n.motifs_arret.code(motif.as_str().unwrap_or_default()).map_or_else(|| "?".to_string(), |c| c.libelle.clone()))
}

/// Un arret resume (`ArretResume`).
pub(crate) fn resume_arret(ctx: &Contexte, n: &Bloc60, a: &Value) -> R<ArretResume> {
    let envois = ses_envois(ctx, a)?;
    let (signalements, mois): (Vec<_>, Vec<_>) = envois.into_iter().filter(|e| matches!(e.nature.as_str(), "04" | "05" | "01")).partition(|e| e.nature != "01");
    let mois: Vec<String> = mois.into_iter().filter_map(|e| e.mois).collect();
    let champ = |k: &str| a.get(k).cloned().unwrap_or(Value::Null);
    let motif = champ("motif");
    let motif_libelle = libelle_motif(n, &motif)?;
    let signale = !signalements.is_empty();
    // UN SIGNALEMENT EST-IL DU ? Pas pour un motif que la DSN ne couvre pas (le deuil d'enfant : `obligations-base.json`,
    // `signalement_arret.sauf_motifs`) ; son obligation est alors l'attestation de salaire hors DSN.
    let signalement_du = crate::obligations::signalement_du(&en_texte(&motif))?;
    Ok(ArretResume {
        salarie: champ("salarie"),
        contrat: champ("contrat_numero"),
        contrat_debut: champ("contrat_debut"),
        motif,
        motif_libelle,
        dernier_jour_travaille: champ("dernier_jour_travaille"),
        debut: champ("debut"),
        fin_previsionnelle: champ("fin_previsionnelle"),
        subrogation: champ("subrogation"),
        subrogation_debut: champ("subrogation_debut"),
        subrogation_fin: champ("subrogation_fin"),
        date_accident: champ("date_accident"),
        reprise: champ("reprise"),
        reprise_motif: champ("reprise_motif"),
        note: a.get("note").cloned().unwrap_or_else(|| json!("")),
        saisi_le: champ("saisi_le"),
        signale,
        signalement_du,
        signalements,
        reporte_par_dit: mois_de_la_dsn(&mois),
        reporte_par: mois,
        etat: if signale {
            EtatDuSignalement::Signale
        } else if signalement_du {
            EtatDuSignalement::ASignaler
        } else {
            EtatDuSignalement::SansSignalement
        },
    })
}

/// La route `arrets` : les arrets de travail du magasin des imprevus, chacun resume comme `imprevus` le resume
/// (`resume`), avec ses obligations, ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<ReponseArrets> {
    let tous = tous_les_arrets(ctx)?;
    let n = nomenclatures()?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut phrases = Vec::new();
    for a in &tous {
        let mut r = resume(ctx, &n, a)?;
        let obligations = crate::obligations::de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?;
        // Ce que l'ecran lit (Exports › Avancé) : qui, quand, le motif, ce qui reste a signaler, puis ses obligations, une
        // ligne chacune.
        if let Resume::Arret(x) = &r {
            let b = &x.base;
            let t = |v: &Value| if vrai(v) { valeur_en_texte(v) } else { String::new() };
            let fin = if vrai(&b.fin_previsionnelle) { format!(", fin prévue le {}", crate::routes::quand(&t(&b.fin_previsionnelle))) } else { String::new() };
            phrases.push(format!("{} : dernier jour travaillé le {}{} — {} — {}", t(&b.salarie), crate::routes::quand(&t(&b.dernier_jour_travaille)), fin, b.motif_libelle, b.etat.dit()));
            phrases.extend(crate::obligations::en_phrases(&obligations).into_iter().map(|o| format!("  {}", o)));
        }
        r.poser_obligations(obligations);
        r.dire_ce_qui_est_parti();
        resumes.push(r);
    }
    let a_signaler: Vec<Resume> = resumes.iter().filter(|r| matches!(r, Resume::Arret(x) if !x.base.signale && x.base.signalement_du)).cloned().collect();
    Ok(ReponseArrets {
        phrase: format!("{}, {} à signaler.", crate::routes::compte(resumes.len(), "arrêt de travail", "arrêts de travail"), a_signaler.len()),
        phrases,
        arrets: resumes,
        a_signaler,
        motifs: n.motifs_arret.saisissables(),
        motifs_reprise: n.motifs_reprise.saisissables(),
        subrogation: n.subrogation.codes,
    })
}

// ----------------------------------------------------------- fins de contrat

/// `KeyError` quand elle est inconnue.
fn nature_de(cle: &Value) -> R<std::sync::Arc<droit::natures::Nature>> {
    let texte = match cle {
        Value::String(s) if !s.is_empty() => s.clone(),
        v if !vrai(v) => {
            // Une nature se dit, elle ne se devine pas (regle 6).
            return Err(erreur("KeyError", socle::fichiers::absent("nature")));
        }
        Value::Array(_) | Value::Object(_) => {
            return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(cle))));
        }
        autre => {
            // Une cle qui n'est pas un texte ne peut etre aucune nature.
            let mut connues: Vec<String> = droit::natures::natures().map_err(plain)?.iter().map(|n| n.cle.clone()).collect();
            connues.sort();
            return Err(erreur("KeyError", format!("Nature de contrat inconnue : {}. Connues : {}.", valeur_ecrite(autre), connues.join(", "))));
        }
    };
    droit::natures::nature(&texte).map_err(|m| erreur("KeyError", m))
}

/// CE QUI MANQUE A UN SIGNALEMENT, tel que l'ecran le dit : `champ` (la donnee de l'engagement, `data-fait` de sa carte),
/// `case` (le nom de la case, sous « Contrat »), `dire` (ce qu'on y met, en mots d'utilisateur, visible sans rien
/// deplier) et `pourquoi` (la regle de la norme).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Manque {
    champ: &'static str,
    case: &'static str,
    dire: String,
    pourquoi: String,
}

fn manque(champ: &'static str, case: &'static str, dire: impl Into<String>, pourquoi: impl Into<String>) -> Manque {
    Manque { champ, case, dire: dire.into(), pourquoi: pourquoi.into() }
}

/// Le type de preavis « 90 - pas de clause de preavis applicable » (`fctu-base.json`, S21.G00.63.001).
const SANS_PREAVIS: &str = "90";

/// UN CONTRAT A DUREE DETERMINEE QUI VA A SON TERME n'a rien a dire de sa fin : son motif est celui de sa nature
/// (`contrats-base.json`, `motif_par_defaut` : 031, « la fin normale d'un CDD arrive a son terme »), et il n'a pas de
/// preavis -- il « cesse de plein droit a l'echeance du terme » (L1243-5, `permanents-base.json`, `motifs_sans_preavis`,
/// 031) --, ce que le bloc S21.G00.63 declare en type 90. L'engagement tel que le signalement le lit : sans motif de
/// rupture saisi, ces deux faits ; avec un motif saisi, tel quel.
fn fin_au_terme(e: &Value) -> R<Value> {
    let mut e = e.clone();
    if vrai(get(&e, "motif_rupture")?) {
        return Ok(e);
    }
    if let Some(m) = nature_de(get(&e, "nature")?)?.motif_rupture_par_defaut() {
        e["motif_rupture"] = json!(m);
        if !vrai(get(ou(get(&e, "preavis")?, objet_vide()), "type")?) {
            e["preavis"] = json!({"type": SANS_PREAVIS});
        }
    }
    Ok(e)
}

/// La phrase d'un manque (`ce_qui_manque`) : sa case, son nom dans la saisie de la carte, et ce qu'on y met.
pub(crate) fn phrase_du_manque(m: &Manque) -> String {
    format!("« {} » (`{}`) : {}", m.case, m.champ, m.dire)
}

/// Ce qu'un signalement exige et que la saisie ne dit pas encore, un manque par case de la carte (`Manque`).
fn ce_qui_manque(ctx: &Contexte, e: &Value) -> R<Vec<Manque>> {
    let e = &fin_au_terme(e)?;
    let mut manques = Vec::new();
    let motif = get(e, "motif_rupture")?;
    if !vrai(motif) {
        manques.push(manque(
            "motif_rupture",
            "Motif de la rupture",
            "pourquoi le contrat a pris fin.",
            "Le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par défaut.",
        ));
    }
    // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`fctu::notification_exigee`).
    if vrai(motif) && !vrai(get(e, "notification_rupture")?) && declarations::fctu::notification_exigee(&valeur_en_texte(motif)).map_err(plain)? {
        manques.push(manque(
            "notification_rupture",
            "Notifiée le",
            "le jour où la rupture a été notifiée : la démission remise, la lettre de rupture ou de licenciement envoyée, la \
fin de la période d'essai notifiée.",
            format!(
                "La date de notification de la rupture (S21.G00.62.003) est obligatoire pour le motif {} (CCH-12, cahier \
technique DSN p. 275). Le signalement ne s'écrit pas sans elle.",
                valeur_en_texte(motif)
            ),
        ));
    }
    let preavis = ou(get(e, "preavis")?, objet_vide());
    if !vrai(get(preavis, "type")?) {
        let requis = |k: &str| e.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
        let (debut, fin) = (d(requis("debut")?).map_err(|m| erreur("ValueError", m))?, d(requis("fin")?).map_err(|m| erreur("ValueError", m))?);
        let anciennete = droit::permanents::anciennete_mois(&debut, &fin);
        let motif_texte = if vrai(motif) { Some(valeur_en_texte(motif)) } else { None };
        let cdd = !nature_de(get(e, "nature")?)?.duree_indeterminee;
        let duree = droit::permanents::preavis_mois(anciennete, motif_texte.as_deref(), cdd).map_err(plain)?;
        let regle = droit::permanents::regle_du_preavis(motif_texte.as_deref(), cdd).map_err(plain)?;
        let mut dit = match (duree, &regle) {
            (Some(m), _) if m != Decimal::ZERO => format!("La loi en donne {} mois (L1234-1).", m),
            (Some(_), _) => format!(
                "La loi ne donne aucun préavis pour ce motif : {}",
                droit::permanents::raison_sans_preavis(motif_texte.as_deref(), cdd).map_err(plain)?.unwrap_or_default()
            ),
            (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
            (None, None) => "Sous six mois d'ancienneté, la loi renvoie à la convention ou aux usages (L1234-1) : elle ne dit pas de durée.".to_string(),
        };
        // Ce que dit la convention, a cote, par le moteur (le calcul de `moteur preavis`) ; pas pour un motif sans preavis
        // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
        if duree != Some(Decimal::ZERO) && regle.is_none() {
            let carte = socle::forme::lire(e, engagement::saisie::CARTE_ILLISIBLE).map_err(plain)?;
            let tout = engagement::preavis(ctx, &carte, anciennete, e.get("motif_rupture").and_then(Value::as_str)).map_err(plain)?;
            if let (Some(conv), true) = (tout.convention, tout.qui != "la loi (L1234-1)") {
                dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", tout.qui, conv));
            } else if tout.mois.is_none() {
                if let Some(pourquoi) = tout.pourquoi.as_ref().filter(|p| !p.is_empty()) {
                    dit.push_str(&format!(" Et {}.", pourquoi.trim_end_matches('.')));
                }
            }
        }
        manques.push(manque(
            "preavis",
            "Préavis",
            format!("le préavis de la rupture, ses dates s'il y en a un, ou « pas de clause de préavis ». {}", dit),
            "Le bloc S21.G00.63 est obligatoire dans un signalement de fin de contrat (cardinalité 1,*) : même sans préavis, il \
se déclare, avec le type « 90 - pas de clause de préavis applicable ».",
        ));
    } else if declarations::fctu::preavis_non_effectue(&valeur_en_texte(get(preavis, "type")?)).map_err(plain)? && !vrai(get(e, "dernier_jour_paye")?) {
        manques.push(manque(
            "dernier_jour_paye",
            "Dernier jour payé",
            "le dernier jour travaillé et payé au salaire habituel, avant le préavis qui n'est pas effectué.",
            "Sur un préavis non effectué, le dernier jour travaillé et payé au salaire habituel (S21.G00.62.006) n'est pas la fin \
du contrat, il précède le début du préavis (CCH-12, cahier technique DSN p. 275). La mensuelle et le signalement le \
déclarent tous deux.",
        ));
    } else if let Some(dernier) = e.get("dernier_jour_paye").filter(|j| vrai(j)).map(valeur_en_texte) {
        // Hors preavis non effectue, aucun jour de la paie ne vient apres le dernier jour paye.
        let jour = |j: &engagement::DateDeLigne| j.jour().to_string();
        let apres: Vec<String> = carte_de(e)?.dates().iter().map(jour).filter(|j| *j > dernier).collect();
        if !apres.is_empty() {
            let dire = |j: &str| socle::date::jour_iso(j).map_or_else(|| j.to_string(), |x| x.jj_mm());
            manques.push(manque(
                "dernier_jour_paye",
                "Dernier jour payé",
                format!(
                    "{} {} après le dernier jour payé, le {} : retirez-le de la paie, ou dites un autre dernier jour payé.",
                    apres.iter().map(|j| format!("le {}", dire(j))).collect::<Vec<_>>().join(", "),
                    if apres.len() > 1 { "sont" } else { "est" },
                    dire(&dernier)
                ),
                "Le dernier jour travaillé et payé (S21.G00.62.006) clôt la paie du contrat rompu : un jour payé après lui \
contredit le signalement.",
            ));
        }
    }
    Ok(manques)
}

/// UNE FIN DE CONTRAT QUI SE SIGNALE, telle que la colonne du mois et les rappels la lisent : l'engagement (`salarie`,
/// `debut`, `numero`, `fin`, `motif_rupture`, `preavis`, tels que la saisie les ecrit), sa nature, ce qui manque pour la
/// signaler (`a_saisir`), son obligation (`obligations::de_la_fin`) et sa pastille ; la route `fins-de-contrat` y ajoute
/// ses phrases (`qui_dit`, `contrat_dit`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinASignaler {
    pub salarie: Value,
    pub mois: String,
    debut: Value,
    pub en_retard: bool,
    pub numero: Value,
    pub fin: Value,
    nature: String,
    motif_rupture: Value,
    preavis: Value,
    pub a_saisir: Vec<Manque>,
    pub du: bool,
    pastille: crate::routes::Pastille,
    pub obligation: crate::obligations::Obligation,
    #[serde(skip_serializing_if = "Option::is_none")]
    qui_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contrat_dit: Option<String>,
}

/// Les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`rappels`) la lisent.
pub(crate) fn fins_a_signaler(ctx: &Contexte, mois: &str, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    // Un signalement de fin ne se doit que pour un engagement valide, lu tel qu'il est ecrit (`tels_qu_ecrits`) : un
    // brouillon ne part pas, et ce qui ne fait que lire ne recalcule rien.
    let Some(saisie) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.map(|v| socle::ecriture::en_valeur(&v)) else {
        return Ok(Vec::new());
    };
    let journal = envois::journal::lu(&ctx.racine)?;
    let etats = engagement::etats::tels_qu_ecrits(ctx, mois, &journal.departs).map_err(Echec::from)?;
    let le_mois = Mois::lire(mois).map_err(|e| erreur("ValueError", e))?;
    let engagements = getd(&saisie, "engagements", &Value::Null)?;
    let liste = if vrai(engagements) { items(engagements)? } else { Vec::new() };
    let mut out = Vec::new();
    for (i, e) in liste.into_iter().enumerate() {
        if etats.get(i).and_then(|x| x.valide()).is_none() {
            continue;
        }
        let e: &Value = &avec_ses_bornes(&e)?;
        let nat = nature_de(get(e, "nature")?)?;
        // La regle est celle de la nature et du motif (`Nature::signalement_fctu`) : un CDDU rompu avant son terme se signale.
        let motif = e.get("motif_rupture").filter(|m| vrai(m)).map(valeur_en_texte);
        if !nat.signalement_fctu(motif.as_deref()) || !vrai(get(e, "fin")?) {
            continue;
        }
        let fin = d(&e["fin"]).map_err(|m| erreur("ValueError", m))?;
        if !(le_mois.premier_jour() <= fin && fin <= le_mois.dernier_jour()) {
            continue;
        }
        let salarie = e.get("salarie").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("salarie")))?;
        // La cle de la fiche : celle que porte le nom du fichier du signalement (`fin-contrat-signaler`).
        let cle = ctx.cle_du_salarie(&valeur_en_texte(salarie)).ok();
        let a_saisir = ce_qui_manque(ctx, e)?;
        // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`).
        let obligation = crate::obligations::de_la_fin(ctx, cle.as_deref(), fin, &journal.lignes, aujourdhui)?;
        // Due : il manque de quoi signaler, ou le delai court. Sa pastille : celle du delai passe
        // (« en retard » l'emporte : la date butoir ne se tait pas parce qu'une case manque), sinon ce qui manque, sinon
        // celle du delai.
        let incomplet = !a_saisir.is_empty();
        let du = incomplet || obligation.du;
        let en_retard = obligation.etat == crate::obligations::EN_RETARD;
        let pastille = if incomplet && !en_retard { Pastille::de("a_faire", "à compléter") } else { obligation.pastille.clone() };
        out.push(FinASignaler {
            salarie: salarie.clone(),
            mois: mois.to_string(),
            debut: e["debut"].clone(),
            en_retard,
            numero: get(e, "numero")?.clone(),
            fin: e["fin"].clone(),
            nature: nat.libelle.clone(),
            motif_rupture: get(e, "motif_rupture")?.clone(),
            preavis: get(e, "preavis")?.clone(),
            a_saisir,
            du,
            pastille,
            obligation,
            qui_dit: None,
            contrat_dit: None,
        });
    }
    Ok(out)
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, c: ChargeMois) -> R<ReponseFins> {
    let mois = c.mois.to_string();
    let mut fins = fins_en_retard_avant(ctx, &mois, ctx.aujourdhui)?;
    fins.extend(fins_a_signaler(ctx, &mois, ctx.aujourdhui)?);
    // Ce que l'ecran lit de chaque fin : qui, quelle nature, quel jour (et le mois d'une fin d'un mois passe), et son
    // contrat.
    for f in &mut fins {
        let fin = crate::routes::jour_en_clair(&f.fin);
        let nom = ctx.salarie(&valeur_en_texte(&f.salarie)).map(|s| crate::routes::nom_a_l_ecran(&s)).unwrap_or_else(|_| valeur_en_texte(&f.salarie));
        let ailleurs = if !f.mois.is_empty() && f.mois != mois { format!(" ({})", Mois::en_clair(&f.mois)) } else { String::new() };
        f.qui_dit = Some(format!("{}, {}, fin le {}{}", nom, f.nature, fin, ailleurs));
        let numero = if vrai(&f.numero) { valeur_en_texte(&f.numero) } else { "?".to_string() };
        f.contrat_dit = Some(format!("contrat {}, fin le {}", numero, fin));
    }
    Ok(ReponseFins { phrase: format!("Fins de contrat du mois : {}.", fins.len()), mois: c.mois, fins })
}

/// Combien de mois en arriere un signalement de fin en retard se rappelle : une annee de paies.
const MOIS_EN_ARRIERE: u32 = 12;

/// LES FINS D'AVANT, EN RETARD : celles des mois qui precedent `mois` (sur `MOIS_EN_ARRIERE`) dont le signalement est du
/// et sa date butoir passee. Une echeance manquee reste due -- le signalement se fait « dans les cinq jours ouvres qui
/// suivent la survenance de la fin du contrat » (cahier technique DSN 2026.1, p. 107, `fctu-base.json`, sa `note`) -- et
/// elle se dit sur le mois qu'on regarde, pas seulement sur celui ou le contrat a fini.
pub(crate) fn fins_en_retard_avant(ctx: &Contexte, mois: &str, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    let mut out = Vec::new();
    let mut m = Mois::lire(mois).map_err(|e| erreur("ValueError", e))?;
    for _ in 0..MOIS_EN_ARRIERE {
        m = m.precedent();
        let mut celles: Vec<FinASignaler> = fins_a_signaler(ctx, &m.to_string(), aujourdhui)?.into_iter().filter(|f| f.du && f.en_retard).collect();
        celles.extend(out);
        out = celles;
    }
    Ok(out)
}

// ------------------------------------------------------------ les contrats d'un jour

/// UN CONTRAT QUI COUVRE UN JOUR, lu dans les saisies : le salarie, son nom, le numero, le debut et la fin, le mois qui le
/// saisit, le spectacle (tels que l'engagement les ecrit), paye au mois ou non, sa nature ; `arret-contrats` le dit
/// (`dit`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContratAuJour {
    salarie: IdSalarie,
    nom: String,
    pub numero: Value,
    pub debut: String,
    fin: String,
    mois: String,
    spectacle: Value,
    mensuel: bool,
    nature: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    dit: Option<String>,
}

/// `arret-contrats` : les contrats d'un salarie qui couvrent un jour.
fn arret_contrats(ctx: &Contexte, c: ChargeArretContrats) -> R<ReponseContrats> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let mut contrats = contrats_au(ctx, &cle, &c.jour.iso())?;
    let phrase = match contrats.len() {
        0 => {
            let nom = ctx.salaries.get(c.salarie.as_str()).and_then(|s| s.nomme()).unwrap_or_else(|| c.salarie.clone());
            format!("Aucun contrat de {} ne couvre le {} : il n'y a rien à suspendre.", nom, c.jour.jj_mm_aaaa())
        }
        n => format!("Contrats qu'un arrêt peut toucher : {}.", n),
    };
    // Chaque contrat se dit en une ligne : son numero et sa periode.
    for k in &mut contrats {
        let numero = if vrai(&k.numero) { valeur_en_texte(&k.numero) } else { "sans numéro".to_string() };
        k.dit = Some(format!("{}, {}", numero, crate::routes::periode_en_clair(&json!(k.debut), &json!(k.fin))));
    }
    Ok(ReponseContrats { phrase, contrats })
}

/// Les contrats de la fiche `cle` qui couvrent `jour` (AAAA-MM-JJ), lus dans les saisies, du plus ancien debut au plus
/// recent.
fn contrats_au(ctx: &Contexte, cle: &str, jour: &str) -> R<Vec<ContratAuJour>> {
    let mut sorties: Vec<ContratAuJour> = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(socle::echec::Echec::from)?.into_iter().map(String::from).collect::<Vec<_>>();
    mois.reverse();
    for m in mois {
        for (c, e) in crate::routes::mois::engagements_du_mois(ctx, &m)? {
            if c.as_str() != cle {
                continue;
            }
            let (debut, fin) = crate::routes::mois::bornes_brutes(&e);
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Le mois le plus recent qui le saisit fait foi (une fin ecrite plus
            // tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = e.nature_lue().map(|n| n.mensuelle()).unwrap_or(false);
            let numero = e.numero.clone().map_or(Value::Null, Value::String);
            if debut.is_empty() || (fin.is_empty() && !mensuel) {
                continue;
            }
            if vus.contient(&[json!(c), json!(debut), numero.clone()])? {
                continue;
            }
            vus.poser(vec![json!(c), json!(debut), numero.clone()], ())?;
            if !(debut.as_str() <= jour && (fin.is_empty() || jour <= fin.as_str())) {
                continue;
            }
            let salarie = cle_du_nom(ctx, cle)?;
            let ou_vide = |x: &Option<String>| json!(x.clone().unwrap_or_default());
            sorties.push(ContratAuJour {
                nom: ctx.nomme(cle),
                salarie,
                numero: brut(&ou_vide(&e.numero)),
                debut,
                fin,
                mois: m.to_string(),
                spectacle: brut(&json!(e.spectacle.as_ref().map(ToString::to_string).unwrap_or_default())),
                // Paye au mois (`Nature::mensuelle`) : son mois de paie se tire du moteur, et un arret y appelle le maintien
                // de salaire (L1226-1 : pas pour un intermittent).
                mensuel,
                nature: e.nature.clone().map_or(Value::Null, Value::String),
                dit: None,
            });
        }
    }
    sorties.sort_by(|a, b| a.debut.cmp(&b.debut));
    Ok(sorties)
}

/// Les mois des DSN qui ont porte un imprevu, tels que l'ecran les lit (« de novembre 2026 »).
fn mois_de_la_dsn(mois: &[String]) -> Vec<String> {
    mois.iter().map(|m| crate::regularisation::de_mois_en_clair(m)).collect()
}
