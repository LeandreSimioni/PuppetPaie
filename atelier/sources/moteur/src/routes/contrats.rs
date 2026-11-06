//! Les routes des contrats : ce que le contrat a signer et l'avenant montrent, les envoyer a la signature ou les
//! imprimer, les relancer, les noter revenus signes, et les pieces parties d'un engagement.
//!
//! Chaque charge et chaque reponse est une forme declaree ici, lue une fois (`routes::lire_charge`) ; l'horloge d'une
//! compagnie fictive et la releve des signatures appellent les memes fonctions, sur les memes formes.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::compagnie::signature::{Choisis, Cible, ContratNomme, ContratSigneDit, ContratsEnvoyes, Impression, UnAvenant};
use crate::routes::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Place, Refus, Route, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::routes::documents::{client_courriel, refus, Ouvrir};
use crate::routes::engagement::{en_echec, maintenant};
use crate::routes::etapes::{ChargeMois, Moteur};
use crate::routes::{en_json, lire_charge, Encart, Pastille, INTERFACE};
use droit::contexte::Contexte;
use engagement::numeros::NumeroContrat;
use envois::etats;
use envois::journal::{self, AvenantParti, ArticleModifie, ContratParti};
use envois::releve_signatures::ReleveDite;
use socle::date::{jour_iso, Jour};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::repr;
use socle::mois::Mois;


const CONTRATS: Champ = Champ { nom: "contrats", libelle: Affiche::new("Contrats"), forme: Forme::Objet, sens: "les contrats choisis : [{salarie, debut, numero}]", avance: false };
const DEBUT: Champ = Champ { nom: "debut", libelle: Affiche::new("Début de l'engagement"), forme: Forme::Date, sens: "le premier jour de l'engagement, AAAA-MM-JJ", avance: false };
const NUMERO: Champ = Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, quand deux commencent le même jour", avance: true };
/// Un contrat ecrit refuse de se faire quand une case qu'il appelle est vide : elle se dit dans la saisie de l'engagement.
const CONTRAT_ECRIT: Refus = Refus {
    classe: "ErreurOperation",
    corriger: "ce que la phrase dit ; une case du contrat écrit vide (`contrat.ecrits` de la route apercu) se remplit dans la saisie de l'engagement, par modifier-engagement ; rien n'est écrit",
};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::SignatureApercu,
        geste: "Montrer les contrats du mois : à envoyer, en attente de signature, en avenant.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats (chacun : son état, et ses gestes recu et relancer, route et charge), boite_liee, signature (l'encart de la signature d'un clic), rien_a_envoyer, rien_en_attente",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DemanderSignature,
        geste: "Faire signer par courriel les contrats et les avenants cochés.",
        charge: &[
            MOIS,
            CONTRATS,
            Champ { nom: "avenants", libelle: Affiche::new("Avenants"), forme: Forme::Objet, sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false },
            Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "tous les contrats de ces salariés", avance: true },
            Champ { nom: "relance", libelle: Affiche::new("Relancer"), forme: Forme::OuiNon, sens: "vrai : relancer ceux qui attendent leur signature", avance: true },
            Champ { nom: "remplace", libelle: Affiche::new("Annule et remplace"), forme: Forme::OuiNon, sens: "vrai : un contrat refait qui annule et remplace le précédent", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"FULLER","debut":"2026-11-27"}]}"#,
        rend: "phrase, partis, contrats (les contrats et les avenants partis, tels que le journal les note), avertissements (ceux sans courriel), signatures (compagnie fictive : revenues aussitôt)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est parti" },
            Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe), ou imprimer" },
            CONTRAT_ECRIT,
            MANQUE,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 600,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PiecesParties,
        geste: "Lister ce qui est parti pour un contrat : DPAE, contrat envoyé, signé, preuve.",
        charge: &[
            Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, tel que signature-apercu ou etat-par-engagement le rend", avance: false },
            Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé de la fiche du salarié ; le numéro suffit", avance: true },
        ],
        exemple: r#"{"salarie":"FULLER","numero":"FU1.26.01"}"#,
        rend: "phrase (chaque pièce partie et son jour), dpae, contrat : les fichiers partis, lus au journal",
        refus: &[MANQUE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AvenantApercu,
        geste: "Montrer l'avenant d'un contrat : ce qui change, par article ou en clair.",
        charge: &[
            SALARIE,
            DEBUT,
            Champ { nom: "numero", libelle: Affiche::new("Numéro du contrat"), forme: Forme::Texte, sens: "le numéro du contrat, tel que signature-apercu le rend", avance: false },
            Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois de l'écran, où se lit un contrat signé hors du logiciel (le mois présent sinon)", avance: true },
        ],
        exemple: r#"{"salarie":"FULLER","debut":"2026-11-27","numero":"FU1.26.01"}"#,
        rend: "le contrat (contrat, date_contrat, signe ; ailleurs : signé hors du logiciel, et contrat_signe : chemin du dossier de son PDF, deposes, phrase), numero de l'avenant, effet, modifications (numero et article, avant, apres ; phrase en clair sans numero), avenants (et la phrase de chacun), destinataire, titre (le contrat comme l'avenant le nomme), situation, encarts",
        refus: &[Refus { classe: "ErreurOperation", corriger: "le salarié, le début et le numéro d'un contrat parti à la signature ou signé hors du logiciel" }, MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsImprimer,
        geste: "Réunir en un PDF les contrats et avenants cochés, à remettre en main propre.",
        charge: &[MOIS, CONTRATS, Champ { nom: "avenants", libelle: Affiche::new("Avenants"), forme: Forme::Objet, sens: "les avenants cochés : [{salarie, debut, numero, mois, effet}]", avance: false }],
        exemple: r#"{"mois":"2026-11","contrats":[{"salarie":"SVOBODA","debut":"2026-11-02"}]}"#,
        rend: "impression (le PDF) et son chemin, fichiers (chaque pièce partie), contrats et avenants partis",
        refus: &[CONTRAT_ECRIT, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratRecu,
        geste: "Noter qu'un contrat (ou un avenant) est revenu signé, ou retirer la note.",
        charge: &[
            MOIS,
            SALARIE,
            DEBUT,
            NUMERO,
            Champ { nom: "recu", libelle: Affiche::new("Reçu"), forme: Forme::OuiNon, sens: "faux : retirer la note", avance: true },
            Champ { nom: "avenant", libelle: Affiche::new("Avenant signé"), forme: Forme::Nombre, sens: "le numéro de l'avenant revenu signé", avance: true },
        ],
        exemple: r#"{"mois":"2026-11","salarie":"FULLER","debut":"2026-11-27"}"#,
        rend: "phrase, recu (quand), annuler (la route et la charge qui défont la note)",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PoserCourriel,
        geste: "Poser le courriel d'un salarié, là où il manque.",
        charge: &[SALARIE, Champ { nom: "email", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "son adresse de courriel", avance: false }],
        exemple: r#"{"salarie":"FULLER","email":"loie.fuller@exemple.invalid"}"#,
        rend: "salarie, email",
        refus: &[crate::routes::fiches::REFUSEE, OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les charges

/// Un contrat que la page vise : son salarie, son premier jour, et son numero, qui distingue deux contrats du meme jour
/// (vide : tous ceux de ce salarie a ce jour).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContratVise {
    pub(crate) salarie: String,
    pub(crate) debut: Jour,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) numero: Option<String>,
}

impl ContratVise {
    fn cible(&self) -> Cible {
        (self.salarie.clone(), self.debut.iso(), self.numero.clone().filter(|n| !n.is_empty()))
    }
}

/// Un avenant coche : le contrat qu'il modifie (son salarie, son premier jour, son numero, tel que `signature-apercu` le
/// rend), le mois de l'ecran (ou se lit un contrat signe hors du logiciel), sa date d'effet (celle de l'apercu sinon) ; une
/// relance (`relance`) reprend l'avenant parti et pas revenu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AvenantVoulu {
    pub(crate) salarie: String,
    pub(crate) debut: Jour,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) numero: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mois: Option<Mois>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) effet: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) relance: bool,
}

/// La charge de `demander-signature` : le mois, les contrats choisis (`contrats` ; `salaries` : tous les contrats de ces
/// salaries ; rien : tous ceux du mois), les avenants coches, une relance ou un contrat refait (`remplace`), et le mot de
/// passe du logiciel tape pour l'envoi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeSignature {
    pub(crate) mois: Mois,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) contrats: Option<Vec<ContratVise>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) avenants: Vec<AvenantVoulu>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) salaries: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) relance: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) remplace: bool,
    #[serde(default, skip_serializing)]
    pub(crate) motdepasse: Option<String>,
}

impl ChargeSignature {
    /// Les contrats de `mois` choisis par `contrats`, sans relance ni mot de passe.
    pub(crate) fn des_contrats(mois: Mois, contrats: Vec<ContratVise>) -> ChargeSignature {
        ChargeSignature { mois, contrats: Some(contrats), avenants: Vec::new(), salaries: None, relance: false, remplace: false, motdepasse: None }
    }

    /// Les avenants coches de `mois`.
    pub(crate) fn des_avenants(mois: Mois, avenants: Vec<AvenantVoulu>) -> ChargeSignature {
        ChargeSignature { avenants, contrats: None, ..ChargeSignature::des_contrats(mois, Vec::new()) }
    }

    /// Ce que la charge choisit parmi les contrats (`Choisis`).
    fn choisis(&self) -> Choisis {
        Choisis { salaries: self.salaries.clone(), contrats: self.contrats.as_ref().map(|l| l.iter().map(ContratVise::cible).collect()) }
    }

    /// Des contrats sont expressement choisis (une liste non vide).
    fn contrats_choisis(&self) -> bool {
        self.contrats.as_ref().is_some_and(|l| !l.is_empty()) || self.salaries.as_ref().is_some_and(|l| !l.is_empty())
    }

    /// UNE CHARGE QUI COCHE UNE LISTE VIDE (`contrats` ou `salaries` donnes vides, sans avenant) : rien n'est choisi.
    fn aucun_coche(&self) -> R<()> {
        let donne = self.contrats.is_some() || self.salaries.is_some();
        if donne && !self.contrats_choisis() && self.avenants.is_empty() {
            return Err(refus("Aucun contrat coché. Rien n'est parti."));
        }
        Ok(())
    }
}

/// La charge de `contrats-imprimer` : le mois, les contrats et les avenants coches.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeImprimer {
    pub(crate) mois: Mois,
    #[serde(default)]
    pub(crate) contrats: Option<Vec<ContratVise>>,
    #[serde(default)]
    pub(crate) avenants: Vec<AvenantVoulu>,
}

impl ChargeImprimer {
    fn signature(self) -> ChargeSignature {
        ChargeSignature { contrats: self.contrats, avenants: self.avenants, ..ChargeSignature::des_contrats(self.mois, Vec::new()) }
    }
}

/// La charge de `contrat-recu` : le contrat (son mois, son salarie, son premier jour, son numero), l'avenant revenu
/// signe s'il s'agit de lui, et le mot : recu (par defaut) ou la note retiree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeRecu {
    pub(crate) mois: Mois,
    pub(crate) salarie: String,
    pub(crate) debut: Jour,
    #[serde(default)]
    pub(crate) numero: Option<String>,
    #[serde(default)]
    pub(crate) avenant: Option<i64>,
    #[serde(default = "recu_par_defaut")]
    pub(crate) recu: bool,
}

fn recu_par_defaut() -> bool {
    true
}

/// La charge de `avenant-apercu` : le contrat parti (son salarie, son premier jour, son numero) et le mois de l'ecran, ou
/// se lit un contrat signe hors du logiciel (le mois present sinon).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeAvenant {
    pub(crate) salarie: String,
    pub(crate) debut: Jour,
    #[serde(default)]
    pub(crate) numero: Option<String>,
    #[serde(default)]
    pub(crate) mois: Option<Mois>,
}

impl ChargeAvenant {
    fn numero(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }
}

/// La charge de `pieces-parties` : le numero du contrat, et son salarie (le numero suffit).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePieces {
    #[serde(default)]
    numero: Option<String>,
    #[serde(default)]
    salarie: Option<String>,
}

/// La charge de `poser-courriel` : le salarie et son courriel.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCourriel {
    salarie: String,
    email: String,
}

// ------------------------------------------------------------------ les reponses

/// Le geste « Reçu signé » d'un contrat (ou de son avenant), et celui qui defait la note : la route et sa charge.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GesteRecu {
    route: Route,
    charge: ChargeRecu,
}

/// Le geste « Relancer » : la route, ce que la demande du mot de passe dit, et sa charge.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GesteRelancer {
    route: Route,
    pourquoi: String,
    charge: ChargeSignature,
}

/// UN CONTRAT DU MOIS QUI SE FAIT SIGNER ICI (`signature-apercu`) : qui, quoi, ou en est sa signature (parti, revenu,
/// caduc, son avenant), ce que la boite en fait (`a_envoyer`, `en_avenant`, `en_attente`), sa pastille et ses gestes.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContratASigner {
    pub(crate) salarie: String,
    pub(crate) nom: String,
    pub(crate) destinataire: String,
    spectacle: String,
    pub(crate) debut: Jour,
    fin: Option<Jour>,
    pub(crate) numero: Option<NumeroContrat>,
    pub(crate) demande_le: Option<String>,
    pastille: Option<Pastille>,
    pub(crate) par: Option<&'static str>,
    pub(crate) a_envoyer: bool,
    pub(crate) en_avenant: bool,
    pub(crate) en_attente: bool,
    recu: Option<GesteRecu>,
    relancer: Option<GesteRelancer>,
    pub(crate) recu_le: Option<String>,
    pub(crate) caduc: bool,
    periode_dit: String,
    numero_dit: Option<String>,
    pourquoi: &'static str,
    pub(crate) avenant: Option<AvenantParti>,
    pub(crate) avenant_attendu: bool,
}

impl ContratASigner {
    /// Le numero du contrat, ou `""`.
    pub(crate) fn numero_dit(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }
}

/// Ce que rend `signature-apercu`.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuSignature {
    phrase: String,
    mois: Mois,
    pub(crate) contrats: Vec<ContratASigner>,
    boite_liee: bool,
    avertissements: Vec<String>,
    /// L'encart de la signature d'un clic, quand une boite courriel est liee.
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<Encart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rien_a_envoyer: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rien_en_attente: Option<&'static str>,
    phrases: Vec<String>,
}

/// Ce que rend `contrat-recu` : la phrase, la note (quand, ou rien quand elle est retiree) et de quoi la defaire.
#[derive(Debug, Serialize)]
pub(crate) struct NoteRecue {
    phrase: String,
    salarie: String,
    debut: Jour,
    recu: Option<String>,
    annuler: GesteRecu,
}

/// Ce que rend `contrats-imprimer` : sa phrase, et ce que l'impression a ecrit.
#[derive(Debug, Serialize)]
pub(crate) struct Imprimes {
    phrase: String,
    #[serde(flatten)]
    impression: Impression,
}

/// Ce que rend `demander-signature` : sa phrase, les adresses servies, les contrats et les avenants partis, ce qui n'est
/// pas parti (pas de courriel), et, dans une compagnie fictive, les signatures revenues aussitot.
#[derive(Debug, Serialize)]
pub(crate) struct SignaturesDemandees {
    phrase: String,
    partis: Vec<String>,
    contrats: Vec<ContratParti>,
    avertissements: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signatures: Option<ReleveDite>,
}

/// Une modification que l'avenant porte, et le titre que la boite lui donne (« Article 3 — Durée ») ; un terme dit en
/// clair n'en a pas.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModificationDite {
    #[serde(flatten)]
    modification: ArticleModifie,
    #[serde(skip_serializing_if = "Option::is_none")]
    titre_dit: Option<String>,
}

/// Un avenant deja parti, et ce que la boite en dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AvenantDit {
    #[serde(flatten)]
    parti: AvenantParti,
    phrase: String,
}

/// Ce que rend `avenant-apercu` : le contrat parti (son salarie, ses bornes, son spectacle, comment il se nomme dans le
/// courriel, le mois ou il est parti, son numero, sa date, sa signature ; `ailleurs` : signe hors du logiciel, et son
/// dossier), l'avenant (son numero, son effet, ce qu'il change), les avenants deja partis, et ce que la boite dit.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuAvenant {
    phrase: String,
    salarie: String,
    debut: Jour,
    fin: Option<Jour>,
    nom: String,
    spectacle: String,
    nom_du_contrat: String,
    pour: String,
    periode: String,
    mois: Mois,
    contrat: Option<NumeroContrat>,
    date_contrat: String,
    signe: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    ailleurs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    contrat_signe: Option<ContratSigneDit>,
    numero: i64,
    effet: String,
    modifications: Vec<ModificationDite>,
    avenants: Vec<AvenantDit>,
    remplacable: bool,
    boite_liee: bool,
    destinataire: String,
    situation: String,
    encarts: Vec<Encart>,
    titre: String,
    phrases: Vec<String>,
}

/// Une piece partie pour un contrat : ce qu'elle est, quand, le mois et le fichier ou elle se range, si le fichier existe.
#[derive(Debug, Serialize)]
struct PiecePartie {
    quoi: String,
    quand: String,
    quand_dit: String,
    mois: Option<Mois>,
    fichier: String,
    existe: bool,
}

/// Ce que rend `pieces-parties`.
#[derive(Debug, Serialize)]
struct PiecesParties {
    phrase: String,
    dpae: Vec<PiecePartie>,
    contrat: Vec<PiecePartie>,
}

/// Ce que rend `poser-courriel`.
#[derive(Debug, Serialize)]
struct CourrielPose {
    phrase: String,
    salarie: String,
    email: String,
}

// ------------------------------------------------------------------ l'aiguillage

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::PoserCourriel => en_json(poser_courriel(ctx, lire_charge(charge)?)?),
        Route::SignatureApercu => en_json(signature_apercu(ctx, lire_charge::<ChargeMois>(charge)?.mois)?),
        Route::AvenantApercu => en_json(avenant_apercu(ctx, &lire_charge(charge)?)?),
        Route::PiecesParties => en_json(pieces_parties(ctx, lire_charge(charge)?)?),
        Route::DemanderSignature => en_json(demander_signature(ctx, lire_charge(charge)?, &mut || client_courriel(ctx))?),
        Route::ContratsImprimer => en_json(contrats_imprimer(ctx, lire_charge::<ChargeImprimer>(charge)?.signature(), INTERFACE)?),
        Route::ContratRecu => en_json(contrat_recu(ctx, &lire_charge(charge)?, INTERFACE, None)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `poser-courriel` : le courriel du salarie, pose dans sa fiche (`fiches::poser_courriel`).
fn poser_courriel(ctx: &Contexte, c: ChargeCourriel) -> R<CourrielPose> {
    let email = c.email.trim().to_string();
    crate::routes::fiches::poser_courriel(ctx, &c.salarie, &email)?;
    Ok(CourrielPose { phrase: format!("Courriel de {} : {}.", c.salarie, email), salarie: c.salarie, email })
}

// ------------------------------------------------------------------ contrat-recu

/// LA REPONSE « JE SIGNE » QUE LA RELEVE A TROUVEE (`compagnie::releve_signatures`) : son code, l'adresse d'ou elle vient,
/// sa date, l'identifiant du message, la preuve archivee, l'exemplaire contresigne, la mention posee, ce qu'elle signe
/// (un contrat ou un avenant, et son numero).
pub(crate) struct ParCourriel {
    pub(crate) code: String,
    pub(crate) adresse: String,
    pub(crate) quand: String,
    pub(crate) message: String,
    pub(crate) preuve: String,
    pub(crate) fichier_signe: String,
    pub(crate) mention: String,
    pub(crate) acte: String,
    pub(crate) avenant: Option<i64>,
}

/// UN CONTRAT (OU UN AVENANT) REVENU SIGNE, ou la note retiree (`c.recu`). Une ligne de journal, pas un etat de la saisie :
/// elle date un fait du monde, sur la ligne de l'envoi. Rien ne part, aucun compteur ne bouge. `qui` : l'ecran, l'horloge
/// d'une compagnie fictive, la releve. `courriel` : la reponse « Je signe » que la releve a trouvee ; la note est alors
/// datee de la SIGNATURE, pas du releve, et porte ce qui la prouve.
pub(crate) fn contrat_recu(ctx: &Contexte, c: &ChargeRecu, qui: &str, courriel: Option<&ParCourriel>) -> R<NoteRecue> {
    let mois = c.mois.to_string();
    let (salarie, debut) = (c.salarie.as_str(), c.debut.iso());
    // Les paies telles que la DSN les a declarees, sans rien ecrire.
    let paies = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(en_echec)?.1;
    // LE CONTRAT, pas le salarie : son numero, celui que l'ecran affiche (ou que la releve a lu) ; sans numero, l'engagement
    // de ce salarie qui commence ce jour-la.
    let numero = c.numero.as_deref().filter(|n| !n.is_empty());
    let trouvee = paies.iter().position(|p| {
        p.salarie.id == salarie
            && match numero {
                Some(n) => p.engagement.numero.as_deref() == Some(n),
                None => p.engagement.debut == c.debut,
            }
    });
    let Some(rang) = trouvee else {
        return Err(refus(format!(
            "Aucun engagement de {} commençant le {}{} dans la saisie enregistrée de {}.",
            salarie,
            debut,
            numero.map_or(String::new(), |n| format!(" (n° {})", n)),
            mois
        )));
    };
    let p = &paies[rang];
    let numero_note = p.engagement.numero.clone();
    let nom = p.salarie.nomme().unwrap_or_else(|| salarie.to_string());
    let avenant = c.avenant.filter(|a| *a != 0);
    let ce = match avenant {
        Some(a) => format!("avenant n° {}", a),
        None => "contrat".to_string(),
    };
    let recu = c.recu;
    let (reponse, quand) = match courriel {
        Some(m) => (format!("contrat signe par courriel ({}, {})", m.code, m.adresse), m.quand.clone()),
        None => (
            if recu { format!("{} signe recu, range a la main", ce) } else { format!("mention retiree : le {} signe n'est pas la", ce) },
            String::new(),
        ),
    };
    // CE QUI REVIENT SE RANGE SUR LA LIGNE DE L'ENVOI : la derniere qui a fait partir ce contrat (ou cet avenant).
    let av = courriel.and_then(|m| m.avenant).or(avenant).filter(|a| *a != 0);
    let retour = journal::RetourContrat {
        salarie: droit::contexte::IdSalarie::lire(salarie)?,
        debut: p.engagement.debut.iso(),
        recu,
        numero: numero_note.clone(),
        nom: nom.clone(),
        avenant: av,
        origine: courriel.map(|_| "courriel".to_string()),
        code: courriel.map(|m| m.code.clone()),
        adresse: courriel.map(|m| m.adresse.clone()),
        quand: if quand.is_empty() { maintenant() } else { quand },
        message: courriel.map(|m| m.message.clone()),
        preuve: courriel.map(|m| m.preuve.clone()),
        fichier_signe: courriel.map(|m| m.fichier_signe.clone()),
        mention: courriel.map(|m| m.mention.clone()),
        acte: courriel.map(|m| m.acte.clone()),
        qui: qui.to_string(),
        reponse,
    };
    let porte = |l: &journal::Ligne| -> bool {
        l.signature().is_some_and(|e| {
            e.accepte
                && e.detail.contrats.iter().any(|x| {
                    x.salarie.as_str() == salarie
                        && x.numero == numero_note
                        && match av {
                            Some(a) => x.est_avenant() && x.avenant == Some(a),
                            None => !x.est_avenant(),
                        }
                })
        })
    };
    let quand_note = retour.quand.clone();
    if journal::rattacher(&ctx.racine, porte, journal::Revenu::Contrat(retour)).map_err(Echec::from)?.is_none() {
        return Err(refus(format!("Le {} de {} n'est jamais parti : rien ne peut en revenir. Rien n'est écrit.", ce, nom)));
    }
    let phrase = format!("{} de {} {}.", socle::texte::capitaliser(&ce), nom, if recu { "noté reçu signé" } else { "de nouveau en attente" });
    // De quoi revenir sur la note : la meme route, le mot contraire.
    let annuler = ChargeRecu { mois: c.mois, salarie: salarie.to_string(), debut: c.debut, numero: numero_note.map(|n| n.to_string()), avenant, recu: !recu };
    Ok(NoteRecue { phrase, salarie: salarie.to_string(), debut: c.debut, recu: recu.then_some(quand_note), annuler: GesteRecu { route: Route::ContratRecu, charge: annuler } })
}

// ------------------------------------------------------------------ contrats-imprimer et demander-signature

/// UN CONTRAT PARTI PUIS CHANGE NE REPART PAS EN CONTRAT : signe ou non, commence ou non, il se modifie par un avenant
/// (`signature-apercu` le dit, `en_avenant`). Refus quand un contrat vise expressement (`contrats`, `salaries`) en est un ;
/// rien n'est ecrit.
fn refuser_les_contrats_partis(ctx: &Contexte, mois: Mois, choisis: &Choisis) -> R<()> {
    if choisis.contrats.is_none() && choisis.salaries.is_none() {
        return Ok(());
    }
    let apercu = signature_apercu(ctx, mois)?;
    for c in apercu.contrats.iter().filter(|c| c.en_avenant) {
        let (salarie, debut, numero) = (&c.salarie, c.debut.iso(), c.numero_dit());
        let vise = choisis.salaries.as_ref().is_some_and(|l| l.contains(salarie))
            || choisis.contrats.as_ref().is_some_and(|l| l.iter().any(|(s, d, n)| s == salarie && *d == debut && n.as_deref().is_none_or(|n| n == numero)));
        if vise {
            return Err(refus(format!(
                "Le contrat de {} est déjà parti et l'engagement a changé depuis : il se modifie par un avenant, que la boîte « Faire signer les contrats » propose. Rien n'est parti.",
                c.nom
            )));
        }
    }
    Ok(())
}

/// `contrats-imprimer` (`signature::contrats_imprimer`) : les contrats choisis et les avenants coches, imprimes ensemble ;
/// sans contrat choisi, seuls les avenants s'impriment. `qui` : la ligne du journal (l'horloge d'une compagnie fictive
/// passe le sien).
pub(crate) fn contrats_imprimer(ctx: &Contexte, c: ChargeSignature, qui: &str) -> R<Imprimes> {
    c.aucun_coche()?;
    let avenants = avenants_voulus(ctx, &c.avenants)?;
    if avenants.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) {
        return Err(refus("Une relance ne s'imprime pas : l'avenant imprimé se remet en main propre. Rien n'est parti."));
    }
    let choisis = c.choisis();
    refuser_les_contrats_partis(ctx, c.mois, &choisis)?;
    let choisis = (c.contrats_choisis() || avenants.is_empty()).then_some(&choisis);
    let impression = crate::compagnie::signature::contrats_imprimer(ctx, &c.mois.to_string(), choisis, &avenants, qui)?;
    // Chaque piece imprimee par son numero, comme l'ecran la coche.
    let quoi: Vec<String> = impression
        .contrats
        .iter()
        .map(|x| format!("contrat n° {}", x.numero_dit()))
        .chain(impression.avenants.iter().map(|x| format!("avenant n° {} au contrat n° {}", x.avenant.map(|a| a.to_string()).unwrap_or_default(), x.numero_dit())))
        .collect();
    Ok(Imprimes { phrase: format!("À faire signer en main propre : {} ; imprimés.", quoi.join(", ")), impression })
}

/// `demander-signature` : faire signer, en un geste, les contrats choisis et les avenants coches. Tous les refus
/// d'abord, puis la boite et UNE preuve (le mot de passe tape, `c.motdepasse`), puis les contrats
/// (`signature::demander_signature`) et les avenants (`envoyer_avenants`, un courriel par salarie). `relance` : l'etat des
/// contrats du mois (`signature-apercu`) dit lesquels peuvent se relancer.
pub(crate) fn demander_signature(ctx: &Contexte, c: ChargeSignature, ouvrir: Ouvrir) -> R<SignaturesDemandees> {
    c.aucun_coche()?;
    let mois = c.mois.to_string();
    let des_contrats = c.avenants.is_empty() || c.contrats_choisis();
    // Un avenant sans objet ou une date d'effet illisible refuse le tout, avant que rien ne parte.
    avenants_voulus(ctx, &c.avenants)?;
    let mut preuve = Preuve::du_mot_de_passe(c.motdepasse.clone());
    let mut r = ContratsEnvoyes::default();
    if des_contrats {
        let choisis = c.choisis();
        let a_qui = match &choisis.salaries {
            Some(l) if !l.is_empty() => format!("{} salarie(s) : {}", l.len(), l.join(", ")),
            _ => "les salaries du mois qui ont un contrat a signer".to_string(),
        };
        let etats_des_contrats = if c.relance { signature_apercu(ctx, c.mois)?.contrats } else { Vec::new() };
        if !c.relance && !c.remplace {
            refuser_les_contrats_partis(ctx, c.mois, &choisis)?;
        }
        let demande = crate::compagnie::signature::DemandeDeSignature { mois: &mois, relance: c.relance, remplace: c.remplace, choisis, etats_des_contrats };
        // Un refus se dit d'abord : la boite et la preuve ne se demandent que pour ce qui part.
        crate::compagnie::signature::verifier_demande(ctx, &demande)?;
        let mut client = ouvrir()?;
        let quoi = if c.relance { "relance de demande de signature de contrat" } else if c.remplace { "contrat refait, a signer (il annule et remplace le precedent)" } else { "demande de signature de contrat" };
        preuve = Preuve { donnee: Some(verifier_preuve(ctx, Some(&preuve), &mois, quoi, &a_qui)?), ..Preuve::default() };
        r = crate::compagnie::signature::demander_signature(ctx, &demande, client.as_mut())?;
    }
    let n_contrats = r.contrats.len();
    let mut n_avenants = 0;
    if !c.avenants.is_empty() {
        let (av, avertissements) = envoyer_avenants(ctx, &c.avenants, &preuve, ouvrir)?;
        n_avenants = av.avenants.len();
        r.partis.extend(av.partis);
        r.contrats.extend(av.avenants);
        r.avertissements.extend(avertissements);
    }
    let quoi = [(n_contrats, "contrat", "contrats"), (n_avenants, "avenant", "avenants")].iter().filter(|(n, _, _)| *n > 0).map(|(n, un, des)| super::compte(*n, un, des)).collect::<Vec<_>>().join(" et ");
    let avertissements = r.avertissements.join(" ");
    let relancee = c.relance || (!des_contrats && c.avenants.iter().all(|a| a.relance));
    let s = if n_contrats + n_avenants > 1 { "s" } else { "" };
    let fait = if relancee { format!("relancé{}", s) } else { format!("envoyé{} à la signature", s) };
    let mut phrase = format!("{} {}.{}{}", quoi, fait, if avertissements.is_empty() { "" } else { " " }, avertissements);
    // LE SIMULATEUR REPOND DANS LE GESTE : les salaries d'une compagnie fictive signent aussitot, et la vraie releve lit
    // leur reponse (`releve_signatures::boite_des_signatures`).
    let mut signatures = None;
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        let releve = crate::compagnie::releve_signatures::relever_signatures(ctx, None, None, INTERFACE)?;
        let resume = releve.resume();
        if !resume.is_empty() {
            phrase.push_str(&format!(" {}.", resume.trim_end_matches('.')));
        }
        signatures = Some(releve.dite());
    }
    let ContratsEnvoyes { partis, contrats, avertissements } = r;
    Ok(SignaturesDemandees { phrase, partis, contrats, avertissements, signatures })
}

/// LES AVENANTS VOULUS, verifies avant que rien ne parte : chacun a son apercu (`avenant-apercu`), qui dit ce qui change
/// (un avenant sans objet ou une date d'effet illisible refuse le tout) ; une relance (`relance`) reprend l'avenant parti
/// et pas revenu, tel quel. Regroupes par salarie.
fn avenants_voulus(ctx: &Contexte, voulus: &[AvenantVoulu]) -> R<Vec<(String, Vec<UnAvenant>)>> {
    let mut par_salarie: Vec<(String, Vec<UnAvenant>)> = Vec::new();
    for v in voulus {
        let un = if v.relance {
            avenant_a_relancer(ctx, v)?
        } else {
            // `numero` (celui que `signature-apercu` rend) et `mois` (celui de l'ecran) disent QUEL contrat du jour : facultatifs.
            let apercu = avenant_apercu(ctx, &ChargeAvenant { salarie: v.salarie.clone(), debut: v.debut, numero: v.numero.clone(), mois: v.mois })?;
            if apercu.modifications.is_empty() {
                return Err(refus(format!(
                    "{}, contrat du {} : le contrat parti dit déjà ce que dit l'engagement, pas d'avenant à faire. Rien n'est parti.",
                    apercu.nom,
                    v.debut.jj_mm_aaaa()
                )));
            }
            let effet = v.effet.clone().filter(|e| !e.is_empty()).unwrap_or_else(|| apercu.effet.clone());
            if jour_iso(&effet).is_none() {
                return Err(refus(format!("Date d'effet illisible : {}.", repr(&effet))));
            }
            UnAvenant {
                salarie: apercu.salarie,
                debut: apercu.debut,
                mois: apercu.mois,
                contrat: apercu.contrat,
                numero: apercu.numero,
                date_contrat: apercu.date_contrat,
                effet,
                modifications: apercu.modifications.into_iter().map(|m| m.modification).collect(),
                destinataire: apercu.destinataire,
                relance: None,
            }
        };
        match par_salarie.iter_mut().find(|(k, _)| *k == v.salarie) {
            Some((_, l)) => l.push(un),
            None => par_salarie.push((v.salarie.clone(), vec![un])),
        }
    }
    Ok(par_salarie)
}

/// L'AVENANT A RELANCER : celui que `signature-apercu` montre en attente pour ce contrat, parti par courriel. Il repart
/// tel qu'il est parti (son PDF, ses modifications). Un contrat dont aucun avenant n'attend, ou dont l'avenant est parti
/// imprime, se refuse : rien n'est ecrit.
fn avenant_a_relancer(ctx: &Contexte, v: &AvenantVoulu) -> R<UnAvenant> {
    let qui = ctx.nomme(&v.salarie);
    let numero = v.numero.as_deref().unwrap_or("");
    let mois = v.mois.ok_or_else(|| erreur("KeyError", socle::echec::cle("L'avenant à relancer dit son mois, celui de l'écran. Rien n'est parti.", "mois")))?;
    let c = signature_apercu(ctx, mois)?
        .contrats
        .into_iter()
        .find(|c| c.salarie == v.salarie && c.debut == v.debut && (numero.is_empty() || c.numero_dit() == numero))
        .ok_or_else(|| refus(format!("Aucun contrat de {} commençant le {} dans {}. Rien n'est parti.", qui, v.debut.iso(), mois)))?;
    let a = match &c.avenant {
        Some(a) if c.avenant_attendu && c.en_attente => a,
        _ => return Err(refus(format!("Le contrat de {} du {} n'a aucun avenant en attente de signature : il n'y a rien à relancer. Rien n'est parti.", qui, v.debut.jj_mm_aaaa()))),
    };
    if c.par != Some("courriel") {
        return Err(refus(format!(
            "L'avenant n° {} de {} est parti imprimé, à remettre en main propre : il ne se relance pas. Rien n'est parti.",
            a.numero.map(|n| n.to_string()).unwrap_or_default(),
            qui
        )));
    }
    Ok(UnAvenant {
        salarie: c.salarie.clone(),
        debut: c.debut,
        mois: a.mois,
        contrat: a.contrat.clone(),
        numero: a.numero.unwrap_or(0),
        date_contrat: String::new(),
        effet: a.effet.clone().unwrap_or_default(),
        modifications: a.modifications.clone(),
        destinataire: c.destinataire.clone(),
        relance: Some(a.fichier.clone()),
    })
}

/// Les avenants voulus, verifies, puis la boite, la preuve, et `signature::envoyer_avenants`. UN SALARIE SANS
/// COURRIEL n'empeche pas les autres : ses avenants ne partent pas, ils se nomment dans le second terme (la phrase de
/// `signature::avenants_sans_courriel_dit`) ; refus si aucun avenant coche n'a de courriel.
pub(crate) fn envoyer_avenants(ctx: &Contexte, voulus: &[AvenantVoulu], preuve: &Preuve, ouvrir: Ouvrir) -> R<(crate::compagnie::signature::Avenants, Vec<String>)> {
    if voulus.is_empty() {
        return Err(refus("Aucun avenant coché. Rien n'est parti."));
    }
    // La porte du branchement, avant de rien lire.
    crate::compagnie::bridage::branchement(&ctx.racine).ouvrir()?;
    let (par_salarie, sans): (Vec<_>, Vec<_>) = avenants_voulus(ctx, voulus)?.into_iter().partition(|(_, aa)| aa.iter().all(|a| !a.destinataire.trim().is_empty()));
    let noms_sans: Vec<String> = sans.iter().map(|(k, _)| ctx.nomme(k)).collect();
    if par_salarie.is_empty() {
        return Err(refus(format!("{} Rien n'est parti.", crate::compagnie::signature::avenants_sans_courriel_dit(&noms_sans))));
    }
    let avertissements = if noms_sans.is_empty() { Vec::new() } else { vec![crate::compagnie::signature::avenants_sans_courriel_dit(&noms_sans)] };
    crate::compagnie::signature::controler_avenants(ctx, &par_salarie)?;
    let mut client = ouvrir()?;
    let premier_mois = par_salarie.iter().flat_map(|(_, aa)| aa.iter().map(|a| a.mois)).min().map(|m| m.to_string()).unwrap_or_default();
    let a_qui = format!("{} salarie(s) : {}", par_salarie.len(), par_salarie.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "));
    let quoi = if par_salarie.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) { "relance d'avenant(s) a signer" } else { "envoi d'avenant(s) a signer" };
    verifier_preuve(ctx, Some(preuve), &premier_mois, quoi, &a_qui)?;
    Ok((crate::compagnie::signature::envoyer_avenants(ctx, &par_salarie, client.as_mut())?, avertissements))
}

// ------------------------------------------------------------------ signature-apercu

/// `signature-apercu` : les contrats du mois et l'etat de leur demande de signature, ce que ses deux boites en font et
/// disent (la signature d'un clic quand une boite courriel est liee, leurs listes vides), et ce que l'envoi dira des
/// contrats dont le salarie n'a pas de courriel.
pub(crate) fn signature_apercu(ctx: &Contexte, mois: Mois) -> R<ApercuSignature> {
    let mois_t = mois.to_string();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_t, None, &departs)?;
    let mut m = Moteur::nouveau(ctx);
    let g = m.globales()?;
    let avenants = crate::routes::etapes::avenants_a_faire(ctx, &mois_t)?;
    let boite_liee = crate::routes::ouverture::boite_liee(ctx)?;
    let mut contrats = Vec::new();
    for p in &paies {
        // SEUL UN CONTRAT ECRIT SE FAIT SIGNER ICI (`engagement::contrat_a_signer`) ; un contrat signe hors du logiciel
        // (`engagement::signe_ailleurs`) n'y vient que pour son avenant, a faire ou attendu.
        let ailleurs = engagement::signe_ailleurs(&ctx.racine, &p.engagement).map_err(Echec::from)?;
        if !ailleurs && !engagement::contrat_a_signer(&ctx.racine, &p.engagement).map_err(Echec::from)? {
            continue;
        }
        let cle_s = p.salarie.id.to_string();
        let numero_t = p.engagement.numero.as_deref().unwrap_or_default().to_string();
        let k = (cle_s.clone(), numero_t.clone());
        let etat = crate::routes::etapes::etat_signature(
            crate::routes::etapes::contrat_parti_le(&departs, &cle_s, p.engagement.numero.as_deref()),
            g.par.get(&k).copied(),
            g.recus.get(&k),
            g.renvoyes.contains(&k),
            g.avenants.get(&k),
            avenants.contains(&k),
            ailleurs,
        );
        if ailleurs && !etat.caduc && !etat.avenant_attendu {
            continue;
        }
        let nom = p.salarie.nomme().unwrap_or_else(|| cle_s.clone());
        let (debut, fin) = (p.engagement.debut, p.engagement.fin);
        // CE QUE LA BOITE EN FAIT : un contrat a envoyer (jamais parti, ou caduc) se coche, et part en avenant s'il est
        // deja parti, signe ou non (`en_avenant`) ; un contrat ou un avenant parti et pas revenu attend : il se note reçu signe
        // (`recu`), et seul celui parti par courriel se relance (`relancer`), quand la boite est posee. Les deux gestes
        // sont des routes et leurs charges : la page n'en compose aucun.
        let parti = etat.fait.clone();
        let (recu, caduc) = (etat.recu.is_some(), etat.caduc);
        let avenant_en_attente = !caduc && etat.avenant_attendu;
        let en_attente = !caduc && ((parti.is_some() && !recu) || avenant_en_attente);
        let a_envoyer = caduc || (!recu && parti.is_none());
        let par = etat.par;
        let numero_avenant = etat.avenant.as_ref().and_then(|a| a.numero).filter(|_| avenant_en_attente);
        let (ce, depuis) = if avenant_en_attente {
            (format!("avenant n° {} ", numero_avenant.map(|n| n.to_string()).unwrap_or_default()), etat.avenant.as_ref().map(|a| a.quand.clone()))
        } else {
            (String::new(), parti.clone())
        };
        let jour = depuis.unwrap_or_default();
        let jj_mm = if jour.len() >= 10 { format!("{}/{}", &jour[8..10], &jour[5..7]) } else { jour };
        let attente = format!("{}{} le {}", ce, if par == Some("imprime") { "imprimé" } else { "envoyé" }, jj_mm);
        let numero = p.engagement.numero.clone().filter(|n| !n.is_empty());
        let (recu_g, relancer) = if en_attente {
            let charge = ChargeRecu { mois, salarie: cle_s.clone(), debut, numero: numero.as_ref().map(|n| n.to_string()), avenant: numero_avenant, recu: true };
            let relancer = (par == Some("courriel") && boite_liee).then(|| {
                let vise = ContratVise { salarie: cle_s.clone(), debut, numero: numero.as_ref().map(|n| n.to_string()) };
                let charge = if avenant_en_attente {
                    ChargeSignature::des_avenants(mois, vec![AvenantVoulu { salarie: cle_s.clone(), debut, numero: vise.numero.clone(), mois: Some(mois), effet: None, relance: true }])
                } else {
                    ChargeSignature { relance: true, ..ChargeSignature::des_contrats(mois, vec![vise]) }
                };
                // Ce que la demande du mot de passe dit avant une relance.
                GesteRelancer { route: Route::DemanderSignature, pourquoi: format!("Relancer la signature : {}, {}.", nom, attente), charge }
            });
            (Some(GesteRecu { route: Route::ContratRecu, charge }), relancer)
        } else {
            (None, None)
        };
        // Sa pastille : un contrat parti puis change part en avenant, signe ou non, commence ou non ; parti et pas revenu,
        // il attend sa signature. Toutes a faire : la compagnie le peut.
        let pastille = if caduc {
            let quand = if recu { " (signé)".to_string() } else { parti.as_deref().map_or(String::new(), |q| format!(" (parti le {})", super::quand(q))) };
            let pourquoi = if etat.pourquoi.is_empty() { crate::routes::etapes::CONTRAT_A_REFAIRE } else { etat.pourquoi };
            Some(Pastille::de("a_faire", &format!("{}{}", pourquoi, quand)))
        } else if en_attente {
            Some(Pastille::de("en_attente", &attente))
        } else {
            None
        };
        contrats.push(ContratASigner {
            salarie: cle_s.clone(),
            destinataire: p.salarie.email.dite().map(String::from).unwrap_or_default(),
            spectacle: p.titre()?,
            debut,
            fin,
            numero_dit: numero.as_ref().map(|n| format!("contrat n° {}", n)),
            numero,
            demande_le: parti,
            pastille,
            par,
            a_envoyer,
            en_avenant: caduc,
            en_attente,
            recu: recu_g,
            relancer,
            recu_le: etat.recu.clone(),
            caduc: etat.caduc,
            periode_dit: periode_dite(debut, fin),
            pourquoi: etat.pourquoi,
            avenant: etat.avenant.clone(),
            avenant_attendu: etat.avenant_attendu,
            nom,
        });
    }
    // Les contrats et les avenants a faire partir dont le salarie n'a pas de courriel : ce que l'envoi en dira.
    let (mut sans, mut sans_avenant): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for c in contrats.iter().filter(|c| boite_liee && c.a_envoyer && c.destinataire.trim().is_empty()) {
        let l = if c.en_avenant { &mut sans_avenant } else { &mut sans };
        if !l.contains(&c.nom) {
            l.push(c.nom.clone());
        }
    }
    let mut avertissements: Vec<String> = Vec::new();
    if !sans.is_empty() {
        avertissements.push(crate::compagnie::signature::sans_courriel_dit(&sans));
    }
    if !sans_avenant.is_empty() {
        avertissements.push(crate::compagnie::signature::avenants_sans_courriel_dit(&sans_avenant));
    }
    let phrases: Vec<String> = contrats
        .iter()
        .map(|c| format!("{} ({}) : {}", c.nom, c.numero_dit(), if c.destinataire.is_empty() { "pas de courriel : il s'imprime" } else { c.destinataire.as_str() }))
        .chain(avertissements.iter().cloned())
        .collect();
    Ok(ApercuSignature {
        phrase: format!("Contrats à faire signer : {}.", contrats.len()),
        mois,
        signature: boite_liee.then(|| {
            Encart::de("information", "Signature d'un clic", "Le contrat part par courriel avec un bouton « Je signe » ; la réponse du salarié est relevée toute seule, et il reçoit son contrat contresigné.")
        }),
        rien_a_envoyer: (!contrats.iter().any(|c| c.a_envoyer)).then_some("Aucun contrat à faire partir ce mois-ci."),
        rien_en_attente: (!contrats.iter().any(|c| c.en_attente)).then_some("Aucun contrat n'attend sa signature."),
        contrats,
        boite_liee,
        avertissements,
        phrases,
    })
}

/// « du 02/11/2026 au 30/11/2026 », « depuis le 02/11/2026 » sans fin.
fn periode_dite(debut: Jour, fin: Option<Jour>) -> String {
    match fin {
        Some(f) => format!("du {} au {}", debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("depuis le {}", debut.jj_mm_aaaa()),
    }
}

// ------------------------------------------------------------------ avenant-apercu

/// L'AVENANT D'UN CONTRAT, tel que la boite le montre : ce qu'il modifierait, article par article, l'ancien texte (celui de
/// la version sur laquelle le contrat est parti) et le nouveau (la paie validee) ; d'un contrat signe hors du logiciel,
/// chaque terme change dit en clair (`avenant_d_un_contrat_signe_ailleurs`). Rien n'est ecrit.
pub(crate) fn avenant_apercu(ctx: &Contexte, c: &ChargeAvenant) -> R<ApercuAvenant> {
    let brut = match avenant_d_un_contrat_parti(ctx, c)? {
        Some(a) => a,
        None => match avenant_d_un_contrat_signe_ailleurs(ctx, c)? {
            Some(a) => a,
            None if c.numero().is_empty() => {
                return Err(erreur("KeyError", socle::echec::cle("Le numéro du contrat désigne le contrat parti dont l'avenant se fait, et la demande ne le dit pas. Rien n'est écrit.", "numero")));
            }
            None => {
                return Err(erreur(
                    "ErreurOperation",
                    format!("Aucun contrat n'est parti à la signature pour {} du {} : il n'y a rien à modifier.", c.salarie, c.debut.jj_mm_aaaa()),
                ))
            }
        },
    };
    Ok(brut.dit())
}

/// Ce que l'apercu d'un avenant sait avant de le dire (`AvenantLu::dit`).
struct AvenantLu {
    salarie: String,
    debut: Jour,
    fin: Option<Jour>,
    nom: String,
    spectacle: String,
    nom_du_contrat: String,
    pour: String,
    periode: String,
    mois: Mois,
    contrat: Option<NumeroContrat>,
    date_contrat: String,
    signe: Option<String>,
    contrat_signe: Option<ContratSigneDit>,
    numero: i64,
    effet: String,
    modifications: Vec<ArticleModifie>,
    avenants: Vec<AvenantParti>,
    remplacable: bool,
    boite_liee: bool,
    destinataire: String,
}

impl AvenantLu {
    /// L'apercu dit : la situation du contrat, les encarts, le titre, la phrase.
    fn dit(self) -> ApercuAvenant {
        let n = self.modifications.len();
        let ailleurs = self.contrat_signe.is_some();
        let contrat = self.contrat.as_deref().unwrap_or("").to_string();
        let situation = format!(
            "{} — {}, {} ; {}",
            self.nom,
            self.spectacle,
            self.periode,
            if ailleurs {
                "signé hors de PuppetPaie".to_string()
            } else if let Some(s) = &self.signe {
                format!("signé le {}", super::quand(s))
            } else {
                format!("envoyé le {}", jour_dit(&self.date_contrat))
            }
        );
        let avenants: Vec<AvenantDit> = self
            .avenants
            .into_iter()
            .map(|a| {
                let signe = a.signe.as_deref().map_or_else(|| "pas encore revenu signé".to_string(), |s| format!("signé le {}", super::quand(s)));
                let phrase = format!("Avenant n° {} envoyé le {} — {}", a.numero.map(|x| x.to_string()).unwrap_or_default(), super::quand(&a.quand), signe);
                AvenantDit { parti: a, phrase }
            })
            .collect();
        let mut encarts = vec![if ailleurs && n > 0 {
            Encart::de("information", "", format!(
                "Le contrat a été signé hors de PuppetPaie : l'avenant n° {} ne cite aucun de ses articles, il dit en clair ce qui change, l'ancienne valeur et la nouvelle ; le reste du contrat ne change pas.",
                self.numero
            ))
        } else if ailleurs {
            Encart::de("information", "", "Aucun terme du contrat signé hors de PuppetPaie n'a changé : il n'y a rien à modifier.")
        } else if n > 0 {
            Encart::de("information", "", format!(
                "Le contrat parti se compare à l'engagement d'aujourd'hui : {}. L'avenant n° {} cite l'ancien texte et le nouveau ; le reste du contrat ne change pas.",
                if n > 1 { format!("{} articles ont changé", n) } else { "un article a changé".to_string() },
                self.numero
            ))
        } else {
            Encart::de("information", "", "Le contrat parti dit déjà tout ce que dit l'engagement : il n'y a rien à modifier.")
        }];
        if n > 0 && self.remplacable && self.boite_liee {
            encarts.push(Encart::de("information", "", format!(
                "Le contrat n'a pas commencé : il peut aussi être refait en entier. Le nouveau porte en tête « annule et remplace le contrat n° {} du {} ».",
                contrat,
                jour_dit(&self.date_contrat)
            )));
        }
        let dits = self.modifications.iter().filter_map(|m| m.phrase.clone().filter(|p| !p.is_empty()));
        let dossier = self.contrat_signe.as_ref().map(|d| format!("{} Son dossier : {}", d.phrase, d.chemin));
        let phrases: Vec<String> = std::iter::once(situation.clone()).chain(encarts.iter().map(Encart::en_ligne)).chain(dits).chain(dossier).collect();
        // Le contrat comme l'avenant le nomme (`{{avenant.reference}}`) : signe hors du logiciel, le papier ne porte
        // pas le numero que le logiciel lui a donne.
        let reference = if ailleurs { format!("commencé le {}", self.debut.jj_mm_aaaa()) } else { format!("n° {}", contrat) };
        let phrase = format!(
            "Avenant {} au contrat {} de {}, à effet du {} : {}.",
            self.numero,
            reference,
            self.nom,
            jour_dit(&self.effet),
            super::compte(n, "modification", "modifications")
        );
        let modifications = self
            .modifications
            .into_iter()
            .map(|m| ModificationDite { titre_dit: m.numero.filter(|_| !m.en_clair()).map(|x| format!("Article {} — {}", x, m.article)), modification: m })
            .collect();
        ApercuAvenant {
            phrase,
            salarie: self.salarie,
            debut: self.debut,
            fin: self.fin,
            nom: self.nom,
            spectacle: self.spectacle,
            nom_du_contrat: self.nom_du_contrat,
            pour: self.pour,
            periode: self.periode,
            mois: self.mois,
            contrat: self.contrat,
            date_contrat: self.date_contrat,
            signe: self.signe,
            ailleurs,
            contrat_signe: self.contrat_signe,
            numero: self.numero,
            effet: self.effet,
            modifications,
            avenants,
            remplacable: self.remplacable,
            boite_liee: self.boite_liee,
            destinataire: self.destinataire,
            situation,
            encarts,
            titre: format!("Avenant au contrat {}", reference),
            phrases,
        }
    }
}

/// « 02/11/2026 » d'un jour AAAA-MM-JJ ; ce qui n'en est pas un, tel quel.
fn jour_dit(iso: &str) -> String {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string())
}

/// L'AVENANT D'UN CONTRAT PARTI D'ICI : ce qu'il modifierait, article par article. `None` : aucun contrat de ce numero
/// n'est parti.
fn avenant_d_un_contrat_parti(ctx: &Contexte, c: &ChargeAvenant) -> R<Option<AvenantLu>> {
    let (salarie, numero_voulu) = (c.salarie.as_str(), c.numero());
    if numero_voulu.is_empty() {
        return Ok(None);
    }
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let Some(parti) = envois::etats::contrat_parti(&lignes, salarie, numero_voulu) else { return Ok(None) };
    let mois_parti = parti.mois.to_string();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_parti, None, &departs)?;
    // La paie de CE contrat : celle qui porte son numero.
    let numero_parti = parti.numero.as_ref().map(|n| n.to_string()).unwrap_or_default();
    let Some(i) = etats::paie_du_contrat(&paies, salarie, &numero_parti)? else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat de {} commençant le {} dans les engagements de {}.", salarie, c.debut.jj_mm_aaaa(), mois_parti),
        ));
    };
    // L'ANCIEN TEXTE : celui que le contrat disait sur la version de l'engagement sur laquelle il (ou son dernier avenant)
    // est parti. Ce qui change se compare au mois ou le contrat change (`engagement::etats::mois_du_changement`) : un
    // contrat au mois qui change a partir d'un mois d'apres celui ou il est parti se compare ce mois-la, et l'avenant
    // prend effet le premier jour de ce mois (ou a la date de l'avenant que la saisie de ce mois dit).
    let siens = version_validee(ctx, &mois_parti, salarie, paies[i].engagement.numero.as_deref(), &departs)?;
    let passee = siens.as_ref().and_then(|v| engagement::etats::version_au_depart(v, &departs, engagement::departs::Envoi::Contrat));
    let mois_effet = match (siens.as_ref(), passee) {
        (Some(v), Some(x)) => engagement::etats::mois_du_changement(v, x).map_err(Echec::from)?.filter(|m| *m > mois_parti),
        _ => None,
    };
    let (paies, i, mois_compare) = match &mois_effet {
        Some(me) => {
            let (_d, ps) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, me, None, &departs)?;
            let j = etats::paie_du_contrat(&ps, salarie, &numero_parti)?
                .ok_or_else(|| erreur("ErreurOperation", format!("Aucune paie de {} en {} pour le contrat n° {}.", salarie, socle::mois::Mois::en_clair(me), numero_parti)))?;
            (ps, j, me.clone())
        }
        None => (paies, i, mois_parti.clone()),
    };
    let effet = match (&mois_effet, siens.as_ref()) {
        (Some(me), Some(v)) => {
            let dite = v.objet().vue(me).map_err(Echec::from)?.and_then(|x| x.date_modification.filter(|d| !d.is_empty()));
            match dite {
                Some(d) => d,
                None => engagement::stock::premier_jour(me).map_err(Echec::from)?.iso(),
            }
        }
        _ => c.debut.iso(),
    };
    let p = &paies[i];
    let g = m.globales()?;
    let deja = envois::etats::avenants_du_contrat(g.avenants.get(&(salarie.to_string(), numero_parti.clone())), &parti);
    let ent = documents::avenant::ent_du_mois(ctx, &mois_parti)?;
    let modele = documents::avenant::modele_du_contrat(&ent.gabarits, &*p.engagement.nat().map_err(Echec::from)?)?;
    let paie_json = socle::ecriture::en_valeur(&engagement::relire::ecrire(ctx, p)?);
    let numero_porte = Some(p.engagement.numero.as_deref().unwrap_or("None").to_string());
    let vals = documents::avenant::valeurs_de_la_paie(ctx, &ent, &paie_json, Some(&numero_porte))?;
    // L'ancien texte s'ecrit du modele avec les valeurs de la version partie, au mois compare (ou, pour un contrat sans
    // terme dont cette version n'avait pas encore ecrit ce mois, a son dernier mois d'avant : ses termes s'y reconduisent).
    let ancienne = match (siens.as_ref(), passee) {
        (Some(v), Some(x)) => {
            let o = v.objet();
            let mut mois = engagement::paies::mois_ecrits(o).map_err(Echec::from)?;
            mois.retain(|m| *m <= mois_compare);
            let mut trouvee = None;
            for m in mois.iter().rev() {
                if let Some(p) = engagement::etats::paie_ecrite_de_la_version(o, x, m).map_err(Echec::from)? {
                    trouvee = Some(p.paie.clone());
                    break;
                }
            }
            trouvee
        }
        _ => None,
    };
    let ancienne = ancienne.ok_or_else(|| {
        erreur("ErreurOperation", format!("Le journal ne dit pas sur quelle version de l'engagement le contrat n° {} est parti : l'avenant ne peut pas citer son ancien texte.", numero_parti))
    })?;
    let avant = documents::avenant::valeurs_de_la_paie(ctx, &ent, &socle::ecriture::en_valeur(&ancienne), Some(&numero_porte))?;
    let modifs = documents::avenant::modifications(&modele, &avant, &vals)?;
    let mut signes: Vec<i64> = Vec::new();
    for a in deja.iter().filter(|a| a.signe.is_some()) {
        signes.push(a.numero.ok_or_else(|| erreur("TypeError", "un numero d'avenant n'est pas un entier"))?);
    }
    let modifications = modifs
        .into_iter()
        .map(|x| ArticleModifie { numero: Some(x.numero), article: x.article, avant: x.avant.iter().map(documents::valeur::valeur_en_texte).collect(), apres: x.apres, blocs: x.blocs, phrase: None, jour: None })
        .collect();
    let nomme = ContratNomme::de(p)?;
    Ok(Some(AvenantLu {
        salarie: salarie.to_string(),
        debut: c.debut,
        fin: p.engagement.fin,
        nom: p.salarie.nomme().unwrap_or_else(|| salarie.to_string()),
        spectacle: nomme.spectacle,
        nom_du_contrat: nomme.nom_du_contrat,
        pour: nomme.pour,
        periode: nomme.periode,
        mois: parti.mois,
        contrat: parti.numero.clone(),
        date_contrat: parti.date(),
        signe: parti.signe.clone(),
        contrat_signe: None,
        numero: signes.iter().max().copied().unwrap_or(0) + 1,
        effet,
        modifications,
        avenants: deja,
        remplacable: c.debut > ctx.aujourdhui,
        // Sans boite courriel, l'avenant s'imprime avec les contrats (`contrats-imprimer`) : rien ne part d'ici.
        boite_liee: crate::routes::ouverture::boite_liee(ctx)?,
        destinataire: p.salarie.email.dite().map(String::from).unwrap_or_default(),
    }))
}

/// La version validee de l'engagement de `salarie` qui porte le numero `numero`, parmi ceux de `mois`.
fn version_validee(ctx: &Contexte, mois: &str, salarie: &str, numero: Option<&str>, departs: &engagement::departs::Departs) -> R<Option<engagement::etats::Valide>> {
    Ok(engagement::etats::du_mois(ctx, mois, departs)?.into_iter().find_map(|e| {
        let v = e.valide()?.clone();
        let b = &*v.objet().brut;
        (b.saisie.carte.salarie.as_deref() == Some(salarie) && b.saisie.carte.numero.as_deref() == numero).then_some(v)
    }))
}

/// L'AVENANT D'UN CONTRAT SIGNE HORS DU LOGICIEL (`engagement::signe_ailleurs`) : le contrat n'est pas parti d'ici, le
/// logiciel ne connait pas son texte. Ce qu'il disait est ce que disait l'engagement sur la version de son dernier avenant
/// parti, ou, sans avenant, son premier mois ; ce qui change, chaque terme date depuis (`engagement::versions::termes_changes`),
/// se dit en clair, sans article (`documents::avenant::changements_en_clair`). `None` : ce n'est pas un contrat signe
/// ailleurs (le contrat du mois de la charge, ou du mois present, qui porte ce numero).
fn avenant_d_un_contrat_signe_ailleurs(ctx: &Contexte, c: &ChargeAvenant) -> R<Option<AvenantLu>> {
    let (salarie, numero) = (c.salarie.as_str(), c.numero());
    if numero.is_empty() {
        return Ok(None);
    }
    let mois = c.mois.unwrap_or_else(|| ctx.aujourdhui.mois_civil());
    let mois_t = mois.to_string();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_t, None, &departs)?;
    let Some(i) = etats::paie_du_contrat(&paies, salarie, numero)? else { return Ok(None) };
    let p = &paies[i];
    if !engagement::signe_ailleurs(&ctx.racine, &p.engagement).map_err(Echec::from)? {
        return Ok(None);
    }
    let debut = p.engagement.debut;
    let Some(v) = version_validee(ctx, &mois_t, salarie, Some(numero), &departs)? else { return Ok(None) };
    let presente = v.objet().brut.valide.as_ref().and_then(|x| x.saisie.clone()).unwrap_or_default();
    let convenue = match engagement::etats::version_au_depart(&v, &departs, engagement::departs::Envoi::Contrat) {
        Some(x) => x.saisie().cloned().unwrap_or_default(),
        None => engagement::stock::Saisie { avenants: Vec::new(), ..presente.clone() },
    };
    let nat = p.engagement.nat().map_err(Echec::from)?;
    let changes = engagement::versions::termes_changes(&convenue, &presente);
    let modifications = documents::avenant::changements_en_clair(ctx, &nat, &changes)
        .map_err(Echec::from)?
        .into_iter()
        .map(|m| serde_json::from_value::<ArticleModifie>(m).map_err(|e| Echec::from(e.to_string())))
        .collect::<R<Vec<_>>>()?;
    let effet = changes.first().map_or_else(|| debut.iso(), |x| x.jour.iso());
    let g = Moteur::nouveau(ctx).globales()?;
    let deja: Vec<AvenantParti> = g.avenants.get(&(salarie.to_string(), numero.to_string())).cloned().unwrap_or_default();
    let signes = deja.iter().filter(|a| a.signe.is_some()).filter_map(|a| a.numero).max().unwrap_or(0);
    let nomme = ContratNomme::de(p)?;
    Ok(Some(AvenantLu {
        salarie: salarie.to_string(),
        debut,
        fin: p.engagement.fin,
        nom: p.salarie.nomme().unwrap_or_else(|| salarie.to_string()),
        spectacle: nomme.spectacle,
        nom_du_contrat: nomme.nom_du_contrat,
        pour: nomme.pour,
        periode: nomme.periode,
        mois,
        contrat: NumeroContrat::lire(numero).ok(),
        date_contrat: debut.iso(),
        signe: None,
        contrat_signe: Some(crate::compagnie::signature::contrat_signe_dit(ctx, salarie, &debut.iso())),
        numero: signes + 1,
        effet,
        modifications,
        avenants: deja,
        remplacable: false,
        boite_liee: crate::routes::ouverture::boite_liee(ctx)?,
        destinataire: p.salarie.email.dite().map(String::from).unwrap_or_default(),
    }))
}

// ------------------------------------------------------------------ pieces-parties

/// Le fichier d'une piece partie existe-t-il, dans `sorties/<mois>/` ou son `envoi/` ? Un NOM, jamais un chemin.
fn piece_existe(racine: &Path, mois: Option<Mois>, fichier: &str) -> bool {
    let Some(mois) = mois else { return false };
    if fichier.is_empty() || ["/", "\\", ".."].iter().any(|x| fichier.contains(x)) {
        return false;
    }
    let base = racine.join("sorties").join(mois.to_string());
    [base.join("envoi").join(fichier), base.join(fichier)].iter().any(|c| c.is_file())
}

fn piece(racine: &Path, quoi: String, quand: &str, mois: Option<Mois>, fichier: &str) -> PiecePartie {
    PiecePartie { quoi, quand: quand.to_string(), quand_dit: super::quand(quand), mois, fichier: fichier.to_string(), existe: piece_existe(racine, mois, fichier) }
}

/// `pieces-parties` : les FICHIERS partis pour le contrat n° `numero` -- la DPAE deposee, le contrat envoye a la
/// signature, le contrat revenu signe et la preuve de la signature. Lu au journal ; rien ne part. La phrase dit chaque
/// piece et son jour, comme la boite de l'ecran.
fn pieces_parties(ctx: &Contexte, c: ChargePieces) -> R<PiecesParties> {
    let numero = c.numero.filter(|n| !n.is_empty()).ok_or_else(|| {
        erreur("KeyError", "Quel contrat ? Donnez son numéro (« numero »), tel que signature-apercu ou etat-par-engagement le rend. Rien n'est écrit.")
    })?;
    let le_salarie = c.salarie.filter(|s| !s.is_empty());
    let mut qui: Option<String> = le_salarie.clone();
    let racine = ctx.racine.as_path();
    let lignes = Moteur::nouveau(ctx).lignes()?;

    // La ligne d'un contrat (une embauche d'une DPAE, un contrat parti, un contrat revenu) est-elle celle du contrat n°
    // `numero` (de ce salarie, quand il est dit) ?
    let ce_contrat = |s: &str, n: &str| n == numero && le_salarie.as_deref().is_none_or(|x| x == s);
    let mut dpae = Vec::new();
    for e in journal::filtrer(&lignes, Some(journal::Acte::Dpae), None, true).filter_map(journal::Ligne::dpae) {
        let Some(fichier) = e.fichier.as_deref().filter(|f| !f.is_empty()) else { continue };
        if !e.accepte {
            continue;
        }
        if let Some(x) = e.detail.embauches.iter().find(|x| ce_contrat(&x.salarie, x.numero.as_deref().unwrap_or(""))) {
            qui.get_or_insert_with(|| x.salarie.to_string());
            dpae.push(piece(racine, "DPAE déposée".into(), &e.quand, e.mois, fichier));
        }
    }

    let mut contrat: Vec<PiecePartie> = Vec::new();
    for l in lignes.iter().filter(|l| l.accepte()) {
        let Some(e) = l.signature() else { continue };
        let parti = if e.detail.main_propre { "imprimé" } else { "envoyé à la signature" };
        for x in e.detail.contrats.iter().filter(|x| ce_contrat(&x.salarie, x.numero_dit()) && !x.fichier.is_empty()) {
            qui.get_or_insert_with(|| x.salarie.to_string());
            let quoi = match x.acte.as_deref() {
                Some("avenant") => format!("Avenant n° {} {}", x.avenant.map(|a| a.to_string()).unwrap_or_default(), parti),
                Some("remplace") => format!("Contrat qui annule et remplace le n° {}, {}", x.remplace.as_deref().unwrap_or(""), parti),
                _ => format!("Contrat {}", parti),
            };
            contrat.push(piece(racine, quoi, &e.quand, Some(x.mois), &x.fichier));
        }
        // CE QUI EN EST REVENU, sur la ligne de chaque envoi.
        for d in e.retours.iter().filter(|d| ce_contrat(&d.salarie, d.numero_dit())) {
            if let Some(f) = d.fichier_signe.as_deref().filter(|f| !f.is_empty()) {
                let quoi = match d.avenant {
                    Some(a) if d.acte.as_deref() == Some("avenant") => format!("Avenant n° {} signé, contresigné", a),
                    _ => "Contrat signé, contresigné".to_string(),
                };
                contrat.push(piece(racine, quoi, &d.quand, l.mois(), f));
            }
            if let Some(p) = d.preuve.as_deref().filter(|p| !p.is_empty()) {
                contrat.push(piece(racine, "Réponse « Je signe » du salarié (preuve)".into(), &d.quand, l.mois(), p));
            }
        }
    }
    contrat.sort_by(|a, b| a.quand.cmp(&b.quand));
    let de = match &qui {
        Some(cle) => format!(" de {}", ctx.salarie(cle).map(|s| super::nom_a_l_ecran(&s)).unwrap_or_else(|_| cle.clone())),
        None => String::new(),
    };
    let dites: Vec<String> = dpae
        .iter()
        .chain(contrat.iter())
        .map(|p| {
            let quoi = if p.quoi.starts_with("DPAE") { p.quoi.clone() } else { p.quoi[..1].to_lowercase() + &p.quoi[1..] };
            if p.quand.is_empty() { quoi } else { format!("{} le {}", quoi, p.quand_dit) }
        })
        .collect();
    let phrase = if dites.is_empty() { format!("Contrat {}{} : rien n'est parti.", numero, de) } else { format!("Contrat {}{} : {}.", numero, de, dites.join(" ; ")) };
    Ok(PiecesParties { phrase, dpae, contrat })
}
