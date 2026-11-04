//! Ce qui est du, et quand : `echeances` est la seule fonction qui compose les echeances de la compagnie (les DPAE, les
//! obligations de `obligations.rs`, la DSN de chaque mois, les echeances annuelles de `donnees/echeancier-base.json` --
//! la cloture de l'exercice apres la DSN de decembre, l'export compta, la revue des spectacles, le taux AT/MP, les
//! contributions de branche). Chacune porte sa date et dit si elle est due maintenant (`due`). Les deux routes la lisent :
//! `echeancier` (tout, date, ce mois-ci et l'annee qui vient) et `rappels` (ce qui est du maintenant, avec son geste).
//! Le panneau et l'horloge lisent les mois dont la DSN est due (`mois_a_declarer`, `mois_a_parcourir`), qu'elle compose.
//! Aucun calcul de paie : les engagements se lisent sans chiffrer, et leur etat (`engagement::etats`), le journal par
//! `etats`.
//!
//! Chaque charge, chaque reponse, l'echeancier livre (`donnees/echeancier-base.json`) et ce que la compagnie a coche
//! (`config/echeancier.json`) sont une forme declaree ici, lue une fois (`socle::forme::lire`). Un refus porte sa `classe`.

use socle::date::jj_mm_aaaa;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use droit::contexte::{archivee, Contexte};
use socle::date::{en_texte, Jour};
use socle::mois::{Graphie, Mois};
use engagement;
use envois::etats;
use crate::compagnie::etat_dsn;
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus as RefusFiche, FORME, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::{lire_charge, Pastille};
use socle::echec::{erreur, Echec};
use std::collections::HashMap;
use socle::fichiers::{lire_json, vrai};
use crate::routes::catalogue::{Ecran, Place};

// ------------------------------------------------------------------ les refus

pub(crate) use socle::echec::Refus;
pub(crate) type R<T> = Result<T, Refus>;

fn avec_classe(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Rappels,
        geste: "Dire ce que la compagnie a à faire ce jour : DSN, fins de contrat, DPAE…",
        charge: &[],
        exemple: r#"{}"#,
        rend: "rappels : les échéances dues maintenant (chacune : quoi, titre, question, mois ou année, état et sa pastille, sa date ; action : la route que son geste ouvre, sa charge, puis la route de l'acte, le mot du bouton)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Echeancier,
        geste: "Lire les échéances du mois et de l'année.",
        charge: &[],
        exemple: "{}",
        rend: "ce_mois et a_venir : les échéances, chacune avec sa date, son état et si un rappel la dit",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RevueSpectacles,
        geste: "Archiver les spectacles qui ne continuent pas, et noter la revue faite.",
        charge: &[ANNEE, Champ { nom: "continuent", libelle: Affiche::new("Spectacles qui continuent"), forme: Forme::Liste, sens: "les spectacles qui continuent (leurs clés) ; les autres s'archivent", avance: false }],
        exemple: r#"{"annee":2026,"continuent":["le-banc-des-menteurs"]}"#,
        rend: "phrase, archives, continuent, configuration",
        refus: &[RefusFiche { classe: "ErreurOperation", corriger: "les spectacles qui continuent : des spectacles actifs, par leur clé" }, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        route: Route::PourDemarrer,
        geste: "Dire ce que la compagnie peut remplir d'avance pour ses premiers actes.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "titre, note, manques (chacun sa case, sa phrase, où il vit ; une ligne d'un salarié dit qui), poses (ce que le moteur a posé, en information)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        route: Route::EcheanceFaite,
        geste: "Cocher une échéance annuelle que seule la compagnie peut dire faite.",
        charge: &[
            Champ { nom: "cle", libelle: Affiche::new("Échéance"), forme: Forme::Texte, sens: "l'échéance et son année, « quoi:AAAA » (celle que rappels ou echeancier rendent)", avance: false },
            Champ { nom: "fait", libelle: Affiche::new("Faite"), forme: Forme::OuiNon, sens: "faux : la décocher", avance: false },
        ],
        exemple: r#"{"cle":"revue-spectacles:2026"}"#,
        rend: "cle, fait",
        refus: &[RefusFiche { classe: "ErreurOperation", corriger: "une échéance qui se coche, avec son année" }, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
];

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

// ------------------------------------------------------------------ les charges

/// La charge d'une route qui ne prend rien : `{}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SansCharge {}

/// La charge de `revue-spectacles` : l'annee de la revue, et les spectacles qui continuent (leurs cles ; les autres
/// s'archivent). Le geste qu'offre son rappel l'ecrit aussi.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChargeRevue {
    annee: i32,
    #[serde(default)]
    continuent: Vec<String>,
}

/// La charge de `echeance-faite` : l'echeance « quoi:AAAA », cochee sauf `fait: false`, au nom de `qui` (`interface`
/// s'il ne se dit pas).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFaite {
    cle: String,
    #[serde(default)]
    fait: Option<bool>,
    #[serde(default)]
    qui: Option<String>,
}

// ------------------------------------------------------------------ les reponses

/// Ce que rend `rappels`.
#[derive(Debug, Serialize)]
pub(crate) struct Rappels {
    phrase: String,
    pub(crate) rappels: Vec<EcheanceDite>,
    /// La boite des rappels se ferme « plus tard » quand il y a quelque chose a faire.
    fermer_dit: &'static str,
    phrases: Vec<String>,
}

/// Ce que rend `echeancier`.
#[derive(Debug, Serialize)]
struct Echeancier {
    phrase: String,
    aujourdhui: String,
    date: String,
    date_dit: String,
    bienvenue: &'static str,
    compagnie: String,
    /// La convention de la fiche de l'entreprise, telle que la fiche la porte.
    idcc: Value,
    ce_mois: Vec<EcheanceDite>,
    a_venir: Vec<EcheanceDite>,
    a_faire: usize,
    phrases: Vec<String>,
}

/// Ce que rend `revue-spectacles`.
#[derive(Debug, Serialize)]
struct Revue {
    phrase: String,
    annee: i32,
    archives: Vec<String>,
    continuent: Vec<String>,
    /// La configuration relue (`ouverture`, route `configuration`).
    configuration: Value,
}

/// Ce que rend `echeance-faite`.
#[derive(Debug, Serialize)]
struct Faite {
    phrase: &'static str,
    cle: String,
    fait: bool,
}

/// Une reponse en JSON : chacune est une structure de ce module, que `serde_json` ecrit toujours.
fn en_json(v: impl Serialize) -> socle::echec::R<Value> {
    serde_json::to_value(v).map_err(|e| Echec::from(e.to_string()))
}

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> socle::echec::R<Value> {
    match nom {
        Route::Rappels => {
            let SansCharge {} = lire_charge(charge)?;
            en_json(rappels(ctx)?)
        }
        Route::Echeancier => {
            let SansCharge {} = lire_charge(charge)?;
            en_json(echeancier(ctx)?)
        }
        Route::RevueSpectacles => en_json(revue_spectacles(o, ctx, lire_charge(charge)?)?),
        Route::EcheanceFaite => en_json(echeance_faite(ctx, lire_charge(charge)?)?),
        // Ce que la compagnie peut remplir d'avance, d'un coup ; ce que le moteur a pose de lui-meme se dit en information
        // (gris) ; les acces aux organismes et le Drive se proposent (`acces_proposes`).
        Route::PourDemarrer => {
            let SansCharge {} = lire_charge(charge)?;
            let (proposes, simules) = acces_proposes(ctx)?;
            let proposes = proposes.iter().map(en_json).collect::<Result<Vec<_>, _>>()?;
            let simules = simules.as_ref().map(en_json).transpose()?;
            declarations::demarrage::route(ctx, proposes, simules).map(|mut v| {
                if let Some(Value::Array(p)) = v.get_mut("poses") {
                    p.iter_mut().filter(|x| x.get("pastille").is_none()).for_each(|x| x["pastille"] = super::pastille("information", "posé pour vous"));
                }
                v
            })
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// Archive les spectacles actifs qui ne sont pas dans
/// `continuent`, note la revue de l'annee faite, rend la configuration.
fn revue_spectacles(o: &HashMap<String, String>, ctx: &Contexte, charge: ChargeRevue) -> socle::echec::R<Revue> {
    let ChargeRevue { annee, continuent } = charge;
    // L'ordre ne compte pas, les doublons non plus.
    let garder: BTreeSet<String> = continuent.into_iter().collect();
    let actifs = ctx.spectacles_actifs();
    let inconnus: Vec<&str> = garder.iter().map(String::as_str).filter(|g| !actifs.contains(g)).collect();
    if !inconnus.is_empty() {
        return Err(erreur("ErreurOperation", format!("Spectacle actif inconnu : {}.", inconnus.join(", "))));
    }
    let archives: Vec<String> = actifs.iter().filter(|c| !garder.contains(**c)).map(|c| c.to_string()).collect();
    for cle in &archives {
        crate::routes::fiches::activer_fiche(ctx, "spectacle", cle, false)?;
    }
    marquer_echeance(ctx, &format!("{}:{}", REVUE_SPECTACLES, annee), true, "interface")?;
    let continuent: Vec<String> = garder.into_iter().collect();
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(|m| socle::echec::noter_et(None, m))?;
    let configuration = crate::routes::ouverture::route(o, &neuf, Route::Configuration, &json!({}))?;
    let phrase = format!("Spectacles qui continuent : {} ; archivés : {}.", continuent.len(), archives.len());
    Ok(Revue { phrase, annee, archives, continuent, configuration })
}

/// `echeance-faite` : coche (`fait`) ou decoche l'echeance `cle`, au nom de `qui`.
fn echeance_faite(ctx: &Contexte, charge: ChargeFaite) -> socle::echec::R<Faite> {
    let ChargeFaite { cle, fait, qui } = charge;
    let fait = fait != Some(false);
    let qui = qui.filter(|q| !q.trim().is_empty()).unwrap_or_else(|| "interface".to_string());
    marquer_echeance(ctx, &cle, fait, &qui)?;
    Ok(Faite { phrase: if fait { "Échéance cochée : faite." } else { "Échéance décochée." }, cle, fait })
}

// ------------------------------------------------------------------ outils

/// Une valeur facultative en texte (`None` si absente).
pub(super) fn texte(v: Option<&Value>) -> String {
    v.map_or("None".to_string(), en_texte)
}

/// La valeur de `cle` en texte, vide si elle manque ou est vide.
fn texte_ou_vide(v: &Value, cle: &str) -> String {
    match v.get(cle) {
        Some(x) if vrai(x) => en_texte(x),
        _ => String::new(),
    }
}

/// Ce que `journal.lire(ctx, quoi, reel_seulement=...)` rend, sur des lignes deja lues.
fn lire_journal<'a>(lignes: &'a [Value], quoi: &str, reel: bool) -> R<Vec<&'a Value>> {
    Ok(etats::filtrer(lignes, Some(quoi), None, reel)?)
}

// ============================================================ les echeances

const DSN: &str = "dsn";
const CLOTURE: &str = "cloture";
const EXPORT_COMPTA: &str = "export-compta";
const REVUE_SPECTACLES: &str = "revue-spectacles";
/// Une obligation due (`obligations.rs`) : la fin d'un contrat qui se signale, ce qu'un arret fait naitre.
const OBLIGATION: &str = "obligation";
const DPAE: &str = "dpae";

/// L'etat d'une echeance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EtatEcheance {
    AFaire,
    Fait,
    EnRetard,
    Information,
}
use EtatEcheance::{AFaire, EnRetard, Fait, Information};

impl EtatEcheance {
    /// Son nom, celui que la couleur lit (`super::couleur_de_l_etat`).
    fn nom(self) -> &'static str {
        match self {
            AFaire => "a_faire",
            Fait => "fait",
            EnRetard => "en_retard",
            Information => "information",
        }
    }

    /// Ses mots, sur sa pastille.
    fn mots(self) -> &'static str {
        match self {
            EnRetard => "en retard",
            AFaire => "à faire",
            Fait => "fait",
            Information => "pour info",
        }
    }
}

/// Ou en est le mois dont on rappelle la DSN (`etape`) : un mois sans saisie se cree, un
/// mois saisi se produit, un mois produit se depose, une DSN refusee se corrige.
const A_SAISIR: &str = "a_saisir";
const A_PRODUIRE: &str = "a_produire";
const A_DEPOSER: &str = "a_deposer";
const A_CORRIGER: &str = "a_corriger";

/// L'echeancier va jusqu'a douze mois devant.
const MOIS_A_VENIR: i32 = 12;

/// UNE ECHEANCE est une DONNEE : quoi, quand, ou en est-on, et ce qu'il y a a faire. Elle ne fait rien : « verifier et
/// envoyer » passe par les operations existantes, avec leurs garde-fous et leur mot de passe.
pub(crate) struct Echeance {
    quoi: String,
    cle: String,
    titre: String,
    question: String,
    detail: String,
    source: String,
    /// Le mois ou elle tombe : l'echeancier la range sous ce mois (aucun : une obligation sans date butoir).
    tombe: Option<Mois>,
    /// Son dernier jour, quand elle en a un.
    date: Option<Jour>,
    /// Quand, en clair (« vendredi 15/01/2027 à midi », « après la DSN de décembre… »).
    quand: String,
    etat: EtatEcheance,
    /// DUE MAINTENANT : son moment est venu et elle n'est pas faite. Les rappels sont exactement celles-ci.
    due: bool,
    /// Ou en est la chose : le mois d'une DSN (`a_saisir`, `a_produire`, `a_deposer`, `a_corriger`), l'etat d'une obligation.
    etape: String,
    /// Seule la compagnie peut la dire faite (`echeance-faite`).
    cochable: bool,
    /// Le mois sur lequel elle porte (la DSN de ce mois, la DPAE d'un engagement de ce mois).
    mois: Option<Mois>,
    annee: Option<i32>,
    fichier: Option<String>,
    engagements: i64,
    validees: i64,
    spectacles: Vec<SpectacleARevoir>,
    salarie: Option<String>,
    a_saisir: Vec<String>,
    /// Une DSN : son echeance de depot (`echeance_dsn::Echeance::en_json`).
    echeance_depot: Option<Value>,
    /// Une obligation due : l'obligation telle que `obligations.rs` l'a calculee.
    obligation: Option<Value>,
    /// Ce sur quoi elle porte (la fin de contrat, l'arret resume), que la page repasse au geste qui la fait.
    objet: Option<Value>,
}

impl Echeance {
    fn nouveau(quoi: &str, cle: String, titre: String, question: String, tombe: Option<Mois>) -> Echeance {
        Echeance {
            quoi: quoi.to_string(),
            cle,
            titre,
            question,
            detail: String::new(),
            source: String::new(),
            tombe,
            date: None,
            quand: String::new(),
            etat: AFaire,
            due: true,
            etape: String::new(),
            cochable: false,
            mois: None,
            annee: None,
            fichier: None,
            engagements: 0,
            validees: 0,
            spectacles: Vec::new(),
            salarie: None,
            a_saisir: Vec::new(),
            echeance_depot: None,
            obligation: None,
            objet: None,
        }
    }

    /// Son dernier jour, et ce jour en clair.
    fn le(&mut self, jour: &Jour) {
        self.date = Some(*jour);
        self.quand = format!("le {}", jour.jj_mm_aaaa());
    }

    /// Ce sur quoi l'echeancier la range : son dernier jour, ou le mois ou elle tombe (en texte : « 2026-11-05 » se range
    /// apres « 2026-11 »).
    fn rang(&self) -> String {
        self.date.map(|j| j.iso()).unwrap_or_else(|| self.tombe_texte())
    }

    fn tombe_texte(&self) -> String {
        self.tombe.map(String::from).unwrap_or_default()
    }

    /// LE GESTE QUI LA FAIT, en routes du catalogue (`etapes::action` : la route qu'il ouvre, sa charge, la route de
    /// l'acte qui suit, le mot du bouton), celles que l'ecran et le LLM appellent. Une obligation, lue sur ses champs : la
    /// fin d'un contrat se signale (ou, s'il manque une case, se complete dans sa carte : la saisie du mois, puis
    /// l'engagement modifie) ; un signalement DSN d'arret ou de reprise se signale ; une obligation hors DSN qu'une route
    /// dit faite (`accident-declare`, `attestation-declare`) se dit faite ; le reste s'ouvre dans son mois. Une DSN, une
    /// DPAE : l'apercu de son depot, puis le depot. La cloture, l'export compta, la revue des spectacles : leur route.
    fn action(&self) -> Value {
        use crate::routes::etapes::{action, deposer_dsn};
        let mois = self.mois.map(String::from).unwrap_or_default();
        let annee_dite = self.annee.map(|a| a.to_string()).unwrap_or_default();
        let annee = || json!({"annee": annee_dite});
        let objet = self.objet.as_ref().unwrap_or(&Value::Null);
        match self.quoi.as_str() {
            OBLIGATION => {
                let Some(o) = self.obligation.as_ref() else { return Value::Null };
                let arret = || json!({"salarie": objet["salarie"], "dernier_jour": objet["dernier_jour_travaille"]});
                match o.get("cle").and_then(Value::as_str).unwrap_or("") {
                    "signalement_fin_contrat" if !self.a_saisir.is_empty() => {
                        action(Route::Saisie, json!({"mois": mois}), Some(Route::ModifierEngagement), "Compléter le contrat".into())
                    }
                    "signalement_fin_contrat" => {
                        action(Route::FinContratSignaler, json!({"mois": mois, "salarie": objet["salarie"]}), Some(Route::Deposer), "Signaler…".into())
                    }
                    cle @ ("signalement_arret" | "signalement_arret_rectificatif" | "signalement_reprise") => {
                        let mut charge = arret();
                        charge["nature"] = json!(if cle == "signalement_reprise" { "05" } else { "04" });
                        action(Route::ArretSignaler, charge, Some(Route::Deposer), "Signaler…".into())
                    }
                    "declaration_accident" if o.get("ou").is_some_and(Value::is_array) => action(Route::AccidentDeclare, arret(), None, "Faite le".into()),
                    "attestation_salaire" if o.get("ou").is_some_and(Value::is_array) => action(Route::AttestationDeclare, arret(), None, "Faite le".into()),
                    _ => action(Route::EtatDuMois, json!({"mois": mois}), None, "Ouvrir le mois".into()),
                }
            }
            DSN => deposer_dsn(&mois, &Value::Null, "Vérifier et envoyer".into()),
            DPAE => action(Route::DepotApercu, json!({"quoi": DPAE, "mois": mois}), Some(Route::Deposer), "Préparer la DPAE".into()),
            CLOTURE => action(Route::Cloture, annee(), None, format!("Clôturer {}", annee_dite).trim().to_string()),
            EXPORT_COMPTA => action(Route::ExportCompta, annee(), None, "Exporter pour la compta".into()),
            // Tous continuent, tant qu'on n'en a pas décoché un : la boîte s'ouvre ainsi.
            REVUE_SPECTACLES => {
                let charge = ChargeRevue { annee: self.annee.unwrap_or_default(), continuent: self.spectacles.iter().map(|s| s.cle.clone()).collect() };
                action(Route::RevueSpectacles, json!(charge), None, "Choisir les spectacles".into())
            }
            _ => Value::Null,
        }
    }

    /// L'échéance telle que les routes la rendent.
    fn dite(&self) -> EcheanceDite {
        EcheanceDite {
            quoi: self.quoi.clone(),
            cle: self.cle.clone(),
            titre: self.titre.clone(),
            question: self.question.clone(),
            detail: self.detail.clone(),
            source: self.source.clone(),
            tombe: self.tombe_texte(),
            date: self.date.map(|j| j.iso()),
            quand: self.quand.clone(),
            etat: self.etat,
            en_retard: self.etat == EnRetard,
            rappel: self.due,
            etape: self.etape.clone(),
            cochable: self.cochable,
            pastille: Pastille::de(self.etat.nom(), self.etat.mots()),
            mois: self.mois,
            annee: self.annee.map(|a| a.to_string()),
            fichier: self.fichier.clone(),
            engagements: self.engagements,
            validees: self.validees,
            spectacles: self.spectacles.clone(),
            salarie: self.salarie.clone(),
            a_saisir: self.a_saisir.clone(),
            echeance_depot: self.echeance_depot.clone(),
            obligation: self.obligation.clone(),
            objet: self.objet.as_ref().map(brut_valeur),
            action: self.action(),
            bouton_coche: if self.etat == Fait { "Pas encore fait" } else { "C'est fait" },
            titre_boite: self.annee.map(|a| format!("Les spectacles de {}", a)),
        }
    }
}

/// UNE ECHEANCE TELLE QUE LES ROUTES LA RENDENT (`rappels`, `echeancier`), champ par champ.
#[derive(Debug, Serialize)]
pub(crate) struct EcheanceDite {
    pub(crate) quoi: String,
    pub(crate) cle: String,
    pub(crate) titre: String,
    pub(crate) question: String,
    pub(crate) detail: String,
    pub(crate) source: String,
    /// Le mois ou elle tombe, vide sans date butoir.
    pub(crate) tombe: String,
    pub(crate) date: Option<String>,
    pub(crate) quand: String,
    pub(crate) etat: EtatEcheance,
    pub(crate) en_retard: bool,
    /// Due maintenant : un rappel la dit.
    pub(crate) rappel: bool,
    pub(crate) etape: String,
    pub(crate) cochable: bool,
    /// Sa pastille : la couleur et les mots de son etat.
    pub(crate) pastille: Pastille,
    pub(crate) mois: Option<Mois>,
    pub(crate) annee: Option<String>,
    pub(crate) fichier: Option<String>,
    pub(crate) engagements: i64,
    pub(crate) validees: i64,
    pub(crate) spectacles: Vec<SpectacleARevoir>,
    pub(crate) salarie: Option<String>,
    pub(crate) a_saisir: Vec<String>,
    /// Une DSN : son echeance de depot, telle que `declarations::echeance_dsn` l'ecrit.
    pub(crate) echeance_depot: Option<Value>,
    /// Une obligation due, telle que `obligations.rs` l'a calculee.
    pub(crate) obligation: Option<Value>,
    /// Ce sur quoi elle porte (la fin de contrat de `imprevus`, l'arret resume), sans ses cles `_`.
    pub(crate) objet: Option<Value>,
    /// Le geste qui la fait (`etapes::action` : une route du catalogue et son bouton).
    pub(crate) action: Value,
    /// La case « C'est fait ».
    pub(crate) bouton_coche: &'static str,
    /// Le titre de sa boite.
    pub(crate) titre_boite: Option<String>,
}

/// Un spectacle actif que la revue de l'annee demande : sa cle, son titre et son numero d'objet tels que sa fiche les
/// porte.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SpectacleARevoir {
    cle: String,
    titre: Value,
    numero_objet: Value,
}

/// `sortie_json.brut` d'un JSON : les cles qui commencent par `_` s'en vont.
pub(super) fn brut_valeur(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut_valeur(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut_valeur).collect()),
        autre => autre.clone(),
    }
}

/// Ce qu'une echeance lit : la compagnie, le jour, et le journal des depots (lu une fois).
pub(crate) struct Etat<'a> {
    pub(crate) ctx: &'a Contexte,
    pub(crate) aujourdhui: Jour,
    pub(crate) journal: std::sync::Arc<Vec<Value>>,
}

impl Etat<'_> {
    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    fn de(ctx: &Contexte) -> socle::echec::R<Etat<'_>> {
        Ok(Etat { ctx, aujourdhui: ctx.aujourdhui, journal: envois::journal::lu(&ctx.racine).map(|j| j.lignes)? })
    }
}

/// `rappels` : les echeances dues maintenant (`echeances`, `due`), dans l'ordre ou elles se font, au jour de la compagnie.
pub(crate) fn rappels(ctx: &Contexte) -> socle::echec::R<Rappels> {
    let st = Etat::de(ctx)?;
    let rappels: Vec<EcheanceDite> = echeances(&st)?.iter().filter(|e| e.due).map(Echeance::dite).collect();
    let phrases: Vec<String> = rappels
        .iter()
        .map(|r| {
            let quand = if r.quand.is_empty() { String::new() } else { format!(" Échéance : {}.", r.quand) };
            format!("{} ({}) : {} {}{}", r.titre, r.pastille.mots, r.question, r.detail, quand).replace("  ", " ")
        })
        .collect();
    Ok(Rappels {
        phrase: format!("Rappels : {}.", rappels.len()),
        fermer_dit: if rappels.is_empty() { "Fermer" } else { "Plus tard" },
        rappels,
        phrases,
    })
}

/// `echeancier` : les memes echeances, rangees. « Ce mois-ci » : celles qui sont dues (un rappel les dit, avec leur
/// geste) et celles qui tombent ce mois ; « a venir » : les autres, sur douze mois.
fn echeancier(ctx: &Contexte) -> socle::echec::R<Echeancier> {
    let st = Etat::de(ctx)?;
    let courant = st.aujourdhui.mois_civil();
    let fin = courant.plus(MOIS_A_VENIR)?;
    let (mut ce_mois, mut a_venir) = (Vec::new(), Vec::new());
    for e in echeances(&st)? {
        if e.due || e.tombe == Some(courant) {
            ce_mois.push(e);
        } else if e.tombe.is_some_and(|t| courant < t && t <= fin) {
            a_venir.push(e);
        }
    }
    let ordre = |etat: EtatEcheance| match etat {
        EnRetard => 0,
        AFaire => 1,
        Information => 2,
        Fait => 3,
    };
    ce_mois.sort_by_cached_key(|e| (ordre(e.etat), e.rang()));
    a_venir.sort_by_key(|e| (e.tombe, e.date));
    let compagnie = ctx.entreprise.nommee();
    let a_faire = ce_mois.iter().filter(|x| !x.due && matches!(x.etat, AFaire | EnRetard)).count();
    let ce_mois: Vec<EcheanceDite> = ce_mois.iter().map(Echeance::dite).collect();
    let a_venir: Vec<EcheanceDite> = a_venir.iter().map(Echeance::dite).collect();
    let ligne = |x: &EcheanceDite| format!("{} ({}{})", x.titre, x.pastille.mots, if x.quand.is_empty() { String::new() } else { format!(", {}", x.quand) });
    let phrases: Vec<String> = ce_mois.iter().chain(&a_venir).map(ligne).collect();
    Ok(Echeancier {
        phrase: format!("{} : {} ce mois-ci, {} à venir.", compagnie, super::compte(ce_mois.len(), "échéance", "échéances"), a_venir.len()),
        aujourdhui: st.aujourdhui.iso(),
        date: date_en_clair(&st.aujourdhui),
        date_dit: format!("Nous sommes le {}", date_en_clair(&st.aujourdhui)),
        bienvenue: "Bienvenue sur PuppetPaie",
        compagnie,
        idcc: ctx.entreprise.idcc.en_valeur(),
        ce_mois,
        a_venir,
        a_faire,
        phrases,
    })
}

/// TOUT CE QUI EST DU, ET QUAND : la seule fonction qui compose les echeances, dans l'ordre ou elles se font. D'abord
/// les DPAE (elles se comptent en heures, avant la premiere heure de travail), puis les obligations nees d'un evenement
/// (quarante-huit heures pour un accident, cinq jours ouvres pour un signalement), la DSN des mois passes qui n'en ont
/// pas, les echeances annuelles (la cloture apres la DSN de decembre, l'export compta, la revue des spectacles, le taux
/// AT/MP, les contributions de branche), et la DSN des douze mois qui viennent. Une echeance se tait des qu'elle est
/// faite ; aucune ne se tait parce qu'on a clique « plus tard ».
pub(crate) fn echeances(st: &Etat) -> R<Vec<Echeance>> {
    let base = echeancier_base()?;
    // Un rappel qui ne se calcule pas n'empeche pas les autres.
    let mut out = rappels_dpae(st).unwrap_or_default();
    out.extend(rappels_obligations(st)?);
    let reclames = mois_a_declarer(st)?;
    for m in &reclames {
        out.push(rappel_dsn(st, *m, &base.dsn)?);
    }
    out.extend(annuelles(st, &base)?);
    // La DSN du mois passe (quand elle n'est plus reclamee) et des douze qui viennent ; rien avant la premiere embauche.
    let premier = premier_mois(st.ctx)?;
    for n in -1..MOIS_A_VENIR {
        let m = st.aujourdhui.mois_civil().plus(n)?;
        if reclames.contains(&m) || premier.is_some_and(|p| m < p) {
            continue;
        }
        out.push(dsn_a_venir(st, m, &base.dsn)?);
    }
    Ok(out)
}

/// Le premier mois de la compagnie (`etat_dsn::premier_mois_du`), s'il y en a un.
fn premier_mois(ctx: &Contexte) -> R<Option<Mois>> {
    Ok(etat_dsn::premier_mois_du(ctx)?.filter(|p| !p.is_empty()).map(|p| Mois::lire(&p)).transpose()?)
}

/// La DSN d'un mois que les rappels ne reclament pas : faite, ou a venir.
fn dsn_a_venir(st: &Etat, mois: Mois, modele: &ModeleDsn) -> R<Echeance> {
    let texte = mois.to_string();
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, &texte)?;
    let titre = formater(&modele.titre, &[("de_mois", socle::texte::de(&mois.libelle(Graphie::Accentuee)))])?;
    let mut e = Echeance::nouveau(DSN, format!("dsn:{}", mois), titre, String::new(), Some(mois.suivant()));
    e.etat = if !etat_dsn::due(st.ctx, &texte, &Value::Null)? { Fait } else if echeance.passee_au(&st.aujourdhui) { EnRetard } else { AFaire };
    e.due = false;
    e.mois = Some(mois);
    e.detail = dsn_detail(modele, &echeance)?;
    e.source = formater(&modele.source, &[])?;
    e.date = Some(echeance.jour);
    e.quand = echeance_phrase(&echeance);
    e.echeance_depot = Some(echeance.en_json());
    Ok(e)
}

fn echeance_phrase(e: &declarations::echeance_dsn::Echeance) -> String {
    e.en_json()["phrase"].as_str().map(String::from).unwrap_or_else(|| e.jour.jj_mm_aaaa())
}

fn dsn_detail(modele: &ModeleDsn, echeance: &declarations::echeance_dsn::Echeance) -> R<String> {
    formater(
        &modele.detail,
        &[("jour", echeance.jour.jour.to_string()), ("mois_suivant", echeance.jour.mois_civil().libelle(Graphie::Accentuee)), ("report", echeance.pourquoi().map(|p| format!(" {}", p)).unwrap_or_default())],
    )
}

/// LES ECHEANCES ANNUELLES (`echeancier-base.json`, celles de la convention de la fiche), de l'annee en cours et de la
/// suivante. La cloture de l'annee passee vient APRES sa DSN de decembre, et l'export compta avec elle.
fn annuelles(st: &Etat, base: &EcheancierBase) -> R<Vec<Echeance>> {
    let ctx = st.ctx;
    let faites = cochees(ctx)?;
    let derniere = st.aujourdhui.mois_civil().plus(MOIS_A_VENIR)?.annee();
    let premier = premier_mois(ctx)?;
    let mut out = Vec::new();
    for annee in st.aujourdhui.an..=derniere {
        for e in annuelles_de(base, ctx.entreprise.idcc.portee()) {
            if let Some(x) = annuelle(st, e, annee, &faites, premier)? {
                let export = x.quoi == CLOTURE && x.etat != Fait;
                out.push(x);
                if export {
                    out.extend(export_compta(st, annee - 1));
                }
            }
        }
    }
    Ok(out)
}

/// L'export compta de l'annee passee : du des janvier, tant que l'annee n'est pas close et que l'export n'est pas a jour.
fn export_compta(st: &Etat, annee: i32) -> Option<Echeance> {
    if annee != st.aujourdhui.an - 1 || crate::compagnie::exercice_ecrit::export_compta_a_jour(&st.ctx.racine, &annee.to_string()) {
        return None;
    }
    let mut r = Echeance::nouveau(
        EXPORT_COMPTA,
        format!("export-compta:{}", annee),
        format!("Exporter les chiffres de {} pour la comptabilité", annee),
        "Le journal de paie de l'année, pour le logiciel de compta.".into(),
        Mois::nouveau(annee + 1, 1).ok(),
    );
    r.annee = Some(annee);
    r.detail = "Une écriture par mois, équilibrée : salaires bruts et charges au débit, organismes et net à payer au crédit.".into();
    Some(r)
}

/// Une echeance annuelle de l'annee `annee` ; `None` quand elle ne concerne pas ce dossier.
fn annuelle(st: &Etat, e: &Annuelle, annee: i32, faites: &Cochees, premier: Option<Mois>) -> R<Option<Echeance>> {
    let ctx = st.ctx;
    let cle = e.cle.as_str();
    let precedente = annee - 1;
    // La cloture d'une annee dont aucun mois n'est dans PuppetPaie (tenue ailleurs) ne concerne pas ce dossier.
    if e.fait_par == FaitPar::ExerciceClos && mois_de_l_annee(ctx, &precedente.to_string()).is_empty() {
        return Ok(None);
    }
    // Une revue des spectacles sans spectacle n'a rien a revoir.
    if cle == REVUE_SPECTACLES && ctx.spectacles.is_empty() {
        return Ok(None);
    }
    let trop_courte = || avec_classe("IndexError", socle::fichiers::trop_courte("les mois de l'échéance"));
    let debut = Mois::nouveau(annee, *e.mois.first().ok_or_else(trop_courte)?)?;
    let dernier = Mois::nouveau(annee, *e.mois.last().ok_or_else(trop_courte)?)?;
    // Ce qui tombait avant l'arrivee de la compagnie ne la concerne pas -- sauf le taux AT/MP de l'annee, sans lequel
    // rien ne se calcule.
    let arrivee_apres = premier.is_some_and(|b| dernier < b);
    if arrivee_apres && cle != "at-mp" {
        return Ok(None);
    }
    // `{jour_dsn}` : le jour de janvier ou la DSN de decembre de l'annee d'avant est due, reporte (`echeance_dsn`).
    let decembre = Mois::nouveau(precedente, 12)?.to_string();
    let dsn_decembre = declarations::echeance_dsn::de_la_compagnie(ctx, &decembre)?;
    let valeurs = [("annee", annee.to_string()), ("annee_precedente", precedente.to_string()), ("jour_dsn", dsn_decembre.jour.jour.to_string())];
    let courant = st.aujourdhui.mois_civil();
    let mut x = Echeance::nouveau(cle, format!("{}:{}", cle, annee), formater(&e.titre, &valeurs)?, String::new(), Some(debut));
    x.detail = formater(&e.detail, &valeurs)?;
    x.source = e.source.clone();
    x.cochable = e.fait_par == FaitPar::Coche;
    x.annee = Some(annee);
    x.etat = if e.fait_par == FaitPar::Information {
        Information
    } else {
        match fait_annuel(ctx, e, annee, faites)? {
            Some(true) => Fait,
            // Une compagnie arrivee apres l'echeance n'est pas « en retard » : elle a a le faire.
            Some(false) if courant > dernier && !arrivee_apres => EnRetard,
            _ => AFaire,
        }
    };
    // Due : son premier mois est venu, et elle n'est pas faite.
    x.due = matches!(x.etat, AFaire | EnRetard) && debut <= courant;
    if cle == CLOTURE {
        x.annee = Some(precedente);
        x.cle = format!("{}:{}", CLOTURE, precedente);
        // LA CLOTURE VIENT APRES LA DSN DE DECEMBRE : tant qu'elle n'est pas partie, c'est son rappel qui parle.
        x.quand = format!("après la DSN de décembre, due {}", echeance_phrase(&dsn_decembre));
        let partie = etat_dsn::du_mois(ctx, &decembre, &Value::Null)?;
        x.due = x.due && (partie.partie() || partie.refusee());
        x.etat = if x.etat == EnRetard && !x.due { AFaire } else { x.etat };
        if x.due {
            x.titre = format!("Clôturer l'exercice {}", precedente);
            x.question = format!("La DSN de décembre est passée. Figer l'année {} ?", precedente);
        }
    }
    if cle == REVUE_SPECTACLES && x.due {
        x.spectacles = spectacles_a_revoir(ctx, annee, faites);
        x.question = format!("Lesquels de vos spectacles se jouent encore en {} ?", annee);
    }
    Ok(Some(x))
}

/// Les echeances annuelles de la convention de la fiche (`idcc`) : celles qui n'en nomment aucune, et celles qui la
/// nomment.
fn annuelles_de<'a>(base: &'a EcheancierBase, idcc: Option<&str>) -> impl Iterator<Item = &'a Annuelle> {
    let voulu = idcc.map_or("None".to_string(), String::from);
    base.annuelles.iter().filter(move |e| e.idcc.is_empty() || e.idcc.contains(&voulu))
}

/// Vrai, faux, ou `None` quand on ne peut pas le savoir.
fn fait_annuel(ctx: &Contexte, e: &Annuelle, annee: i32, faites: &Cochees) -> R<Option<bool>> {
    if e.fait_par == FaitPar::ExerciceClos {
        return Ok(Some(exercice_clos(ctx, &(annee - 1).to_string())?.as_ref().is_some_and(vrai)));
    }
    // Le taux AT/MP se saisit dans la page : pose pour l'annee (ou un taux collectif que l'annee reprend d'elle-meme,
    // `Contexte::taux_atmp_de`), il vaut releve.
    if e.cle == "at-mp" && matches!(ctx.taux_atmp_de(annee), Some(Ok(t)) if t.get("valeur").is_some_and(|x| !x.is_null())) {
        return Ok(Some(true));
    }
    if e.fait_par == FaitPar::Coche {
        return Ok(Some(faites.fait.contains_key(&format!("{}:{}", e.cle, annee))));
    }
    Ok(None)
}

// ------------------------------------------------------ l'echeancier livre

/// L'ECHEANCIER LIVRE (`donnees/echeancier-base.json`) : les phrases de la DSN mensuelle, et les echeances de
/// calendrier, chacune avec son texte. La seule lecture du fichier.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EcheancierBase {
    /// Ce qu'est le fichier.
    #[allow(dead_code)]
    quoi: String,
    dsn: ModeleDsn,
    annuelles: Vec<Annuelle>,
}

/// La DSN mensuelle : son titre (`{de_mois}`), son detail (`{jour}`, `{mois_suivant}`, `{report}`), ses textes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeleDsn {
    titre: String,
    detail: String,
    source: String,
}

/// Une echeance de calendrier : ses mois (le premier ou elle est due, le dernier apres lequel elle est en retard), son
/// titre et son detail (`{annee}`, `{annee_precedente}`, `{jour_dsn}`), son texte, qui la dit faite ; les conventions
/// qu'elle concerne (aucune : toutes).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Annuelle {
    cle: String,
    #[serde(default)]
    idcc: Vec<String>,
    mois: Vec<u32>,
    titre: String,
    detail: String,
    source: String,
    fait_par: FaitPar,
}

/// Qui dit une echeance annuelle faite : la cloture de l'exercice, la compagnie qui la coche, ou personne (une
/// information).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FaitPar {
    ExerciceClos,
    Coche,
    Information,
}

fn echeancier_base() -> socle::echec::R<EcheancierBase> {
    let v = crate::routes::mois::lire_saisie(&socle::fichiers::dossier_du_code().join("echeancier-base.json"))?;
    socle::forme::lire(&v, "L'échéancier livré avec le logiciel ne se lit pas : le signaler au développeur.").map_err(|m| erreur("ValueError", m))
}

// ------------------------------------------------------ ce que la compagnie a coche

/// Ce que l'utilisateur a coche dans l'echeancier, sous la racine de la compagnie.
pub(crate) const ECHEANCIER: &str = "config/echeancier.json";

/// `config/echeancier.json` : les echeances que la compagnie a dites faites (« quoi:AAAA »), quand et par qui.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cochees {
    #[serde(rename = "_origine")]
    origine: String,
    #[serde(default)]
    fait: BTreeMap<String, Coche>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Coche {
    le: String,
    qui: String,
}

/// `config/echeancier.json`, lu : rien de coche s'il n'existe pas. La seule lecture du fichier ; `marquer_echeance` est
/// la seule ecriture.
fn cochees(ctx: &Contexte) -> socle::echec::R<Cochees> {
    let p = ctx.racine.join(ECHEANCIER);
    if !p.exists() {
        return Ok(Cochees::default());
    }
    let v = crate::routes::mois::lire_saisie(&p)?;
    socle::forme::lire(&v, "Les échéances cochées de la compagnie ne se lisent pas.").map_err(|m| erreur("ValueError", m))
}

/// Coche (ou decoche) une echeance que seule la compagnie peut dire faite : « quoi:AAAA », `quoi` une echeance annuelle
/// cochable de l'echeancier livre.
fn marquer_echeance(ctx: &Contexte, cle: &str, fait: bool, qui: &str) -> socle::echec::R<()> {
    let (quoi, annee) = cle.split_once(':').unwrap_or((cle, ""));
    let base = echeancier_base()?;
    let chiffres = !annee.is_empty() && annee.chars().all(|c| c.is_ascii_digit());
    let Some(e) = base.annuelles.iter().find(|x| x.cle == quoi).filter(|_| chiffres) else {
        return Err(erreur("ErreurOperation", format!("Echeance inconnue : {}.", socle::fichiers::repr(cle))));
    };
    if e.fait_par != FaitPar::Coche {
        let n: i64 = annee.parse().unwrap_or(0);
        let titre = formater(&e.titre, &[("annee", annee.to_string()), ("annee_precedente", (n - 1).to_string()), ("jour_dsn", String::new())])?;
        return Err(erreur("ErreurOperation", format!("« {} » ne se coche pas : le logiciel le voit fait quand c'est fait.", titre)));
    }
    let mut c = cochees(ctx)?;
    if fait {
        c.fait.insert(cle.to_string(), Coche { le: ctx.aujourdhui.iso(), qui: qui.to_string() });
    } else {
        c.fait.remove(cle);
    }
    c.origine = "Les echeances que l'utilisateur a dites faites.".to_string();
    let p = ctx.racine.join(ECHEANCIER);
    if let Some(parent) = p.parent() {
        crate::routes::fiches::creer_dossier(parent)?;
    }
    socle::ecriture::ecrire_json(&p, &en_json(&c)?, 2).map_err(crate::routes::fiches::en_os)
}

/// `{nom}` remplace, `{{` et `}}` s'ecrivent `{` et `}`.
fn formater(modele: &str, valeurs: &[(&str, String)]) -> R<String> {
    let mut out = String::new();
    let mut it = modele.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut nom = String::new();
                loop {
                    match it.next() {
                        Some('}') => break,
                        Some(x) => nom.push(x),
                        None => return Err(avec_classe("ValueError", "une accolade ouverte n'est pas fermée avant la fin du texte")),
                    }
                }
                let v = valeurs.iter().find(|(k, _)| *k == nom).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent(&nom)))?;
                out.push_str(&v.1);
            }
            '}' => return Err(avec_classe("ValueError", "une accolade fermée n'a pas été ouverte")),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// « mercredi 23 septembre 2026 ».
fn date_en_clair(j: &Jour) -> String {
    const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
    let mois = j.mois_civil().nom(socle::mois::Graphie::Accentuee);
    format!("{} {} {} {}", JOURS[j.jour_de_la_semaine() as usize], j.jour, mois, j.an)
}

// -------------------------------------------------------------- l'exercice

/// La cloture de l'annee (ou `None`).
pub(crate) fn exercice_clos(ctx: &Contexte, annee: &str) -> R<Option<Value>> {
    let p = ctx.racine.join(crate::compagnie::exercice_ecrit::EXERCICES);
    if !socle::fichiers::existe(&p) {
        return Ok(None);
    }
    // Lu une fois tant qu'il ne change pas, le refus d'un fichier illisible inchange (`lire_json`).
    match &*socle::fichiers::json_en_cache_ou(&p, lire_json)? {
        Value::Object(o) => Ok(o.iter().find(|(k, _)| !k.starts_with('_') && k.as_str() == &annee[..4.min(annee.len())]).map(|(_, v)| v.clone())),
        autre => Err(avec_classe("AttributeError", format!("config/exercices.json : {}", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&autre))))),
    }
}

/// Les mois de l'annee connus du dossier.
pub(crate) fn mois_de_l_annee(ctx: &Contexte, annee: &str) -> Vec<String> {
    let mut v: Vec<String> = super::ouverture::mois_connus(&ctx.racine).into_iter().map(|m| m.0).filter(|m| m.starts_with(annee)).collect();
    v.sort();
    v
}

/// `spectacles_a_revoir` : les spectacles actifs, tant que la revue de l'annee
/// n'est pas faite.
fn spectacles_a_revoir(ctx: &Contexte, annee: i32, faites: &Cochees) -> Vec<SpectacleARevoir> {
    if faites.fait.contains_key(&format!("{}:{}", REVUE_SPECTACLES, annee)) {
        return Vec::new();
    }
    ctx.spectacles
        .iter()
        .filter(|(_, s)| !archivee(s))
        .map(|(cle, s)| SpectacleARevoir {
            cle: cle.clone(),
            titre: brut_valeur(&s.get("titre").filter(|t| vrai(t)).cloned().unwrap_or_else(|| Value::String(cle.clone()))),
            numero_objet: brut_valeur(&s.get("numero_objet").filter(|t| vrai(t)).cloned().unwrap_or(Value::Null)),
        })
        .collect()
}

// ------------------------------------------------------- la DSN des mois

/// Le mois le plus recent dont la DSN reelle est partie et n'a pas ete
/// refusee -- passee, ou en attente de son bilan.
fn derniere_dsn_passee(st: &Etat) -> R<Option<String>> {
    let mut mois = BTreeSet::new();
    for l in lire_journal(&st.journal, DSN, true)? {
        if l.get("accepte").is_some_and(vrai) && l.get("mois").is_some_and(vrai) {
            mois.insert(en_texte(&l["mois"]));
        }
    }
    let mut passes = Vec::new();
    for m in mois {
        let e = etat_dsn::du_mois(st.ctx, &m, &Value::Null)?;
        if !e.a_faire() {
            passes.push(m);
        }
    }
    Ok(passes.into_iter().max())
}

/// Les mois dont la DSN manque -- ou a ete refusee --, du plus ancien au plus
/// recent : depuis le mois qui suit la derniere DSN passee jusqu'au mois d'avant
/// aujourd'hui. Sans aucune DSN au journal : le seul mois d'avant -- on ne va pas
/// reclamer cinq ans d'histoire a une compagnie qui commence.
fn mois_a_declarer(st: &Etat) -> R<Vec<Mois>> {
    let dernier = st.aujourdhui.mois_civil().precedent();
    let depuis = derniere_dsn_passee(st)?;
    let mut premier = match &depuis {
        // Un signalement se date par le jour de son evenement (`AAAA-MM-JJ`) : son mois en est le debut.
        Some(d) => Mois::lire(d.get(..7).unwrap_or(d))?.suivant(),
        None => dernier,
    };
    if let (Some(borne), None) = (premier_mois(st.ctx)?, &depuis) {
        premier = premier.max(borne);
    }
    let mut out = Vec::new();
    for m in premier.jusqu_a(dernier) {
        if etat_dsn::due(st.ctx, &m.to_string(), &Value::Null)? {
            out.push(m);
        }
    }
    Ok(out)
}

/// LES MOIS QUI ONT QUELQUE CHOSE A FAIRE au jour `aujourdhui`, du plus ancien au plus recent : ceux que la saisie
/// touche (`stock::mois_saisis`), et ceux dont la DSN est reclamee sans engagement (`mois_a_declarer` : une DSN sans
/// individu, cahier technique DSN 2026.1, § 2.4). Le mois a ouvrir et l'horloge de la compagnie fictive les parcourent.
pub(crate) fn mois_a_parcourir(ctx: &Contexte, aujourdhui: Jour) -> socle::echec::R<Vec<String>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default();
    let st = Etat { ctx, aujourdhui, journal: envois::journal::lu(&ctx.racine)?.lignes };
    mois.extend(mois_a_declarer(&st)?.into_iter().map(String::from));
    mois.sort();
    mois.dedup();
    Ok(mois)
}

/// `fichiers_deposables(ctx, mois)["dsn"]` : `(nom,
/// chemin)` de la DSN mensuelle du dossier du mois, dans l'ordre de `crate::routes::etapes::ordre_de_depot`.
fn dsn_deposable(st: &Etat, mois: &str) -> Option<(String, String)> {
    // Le mois n'est ecrit que si le moteur a note sa DSN mensuelle (`depart::mois_ecrit`, le critere de l'horloge
    // et de l'accueil) : un fichier DSN qu'il n'a pas note ne fait pas « le mois est ecrit ».
    if !crate::compagnie::depart::mois_ecrit(st.racine(), mois) {
        return None;
    }
    let dossier = st.racine().join("sorties").join(mois);
    let mut fichiers: Vec<(String, PathBuf)> = entrees_dans_l_ordre(&dossier)
        .into_iter()
        .filter(|(nom, c)| c.is_file() && Path::new(nom).extension().map_or(false, |e| e.to_string_lossy().to_lowercase() == "dsn"))
        // LA MENSUELLE, PAS UN SIGNALEMENT : un signalement d'arret ou de fin ecrit apres la DSN du mois, dans
        // le meme dossier, devenait « la DSN a deposer » du rappel.
        .filter(|(_, c)| std::fs::read(c).map(|o| !envois::depot::lire(&o).est_signalement()).unwrap_or(false))
        .collect();
    fichiers.sort_by_cached_key(|(nom, c)| crate::routes::etapes::ordre_de_depot(nom, c));
    fichiers.into_iter().next().map(|(nom, c)| (nom, c.to_string_lossy().into_owned()))
}

/// Les entrees d'un dossier dans l'ordre du systeme (`iterdir`), pour que deux
/// fichiers de meme date se departagent toujours de la meme facon.
fn entrees_dans_l_ordre(dossier: &Path) -> Vec<(String, PathBuf)> {
    match std::fs::read_dir(dossier) {
        Ok(l) => l.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())).collect(),
        Err(_) => Vec::new(),
    }
}

/// Combien d'engagements le mois montre, et combien sont valides. Muette en cas de pepin : un mois illisible rend
/// `(0, 0)`.
fn engagements_valides(ctx: &Contexte, mois: &str) -> (i64, i64) {
    let Ok(valides) = engagement::etats::valides_du_mois(ctx, mois) else { return (0, 0) };
    (valides.len() as i64, valides.iter().filter(|v| **v).count() as i64)
}

fn rappel_dsn(st: &Etat, le_mois: Mois, modele: &ModeleDsn) -> R<Echeance> {
    let mois = le_mois.to_string();
    let mois = mois.as_str();
    let lib = le_mois.libelle(Graphie::Accentuee);
    let mut r = Echeance::nouveau(
        DSN,
        format!("dsn:{}", mois),
        format!("Il est temps d'envoyer la DSN {}", socle::texte::de(&lib)),
        format!("Avez-vous bien fini les paies {} ?", socle::texte::de(&lib)),
        Some(le_mois.suivant()),
    );
    r.mois = Some(le_mois);
    // L'echeance : `echeance_dsn`, le 5 ou le 15 du mois suivant selon l'effectif, reporte au jour ouvre suivant un
    // samedi, un dimanche ou un jour ferie (R. 133-14, I), a midi. Passee, ce n'est plus un rappel, c'est un retard --
    // et on le dit.
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    r.etat = if echeance.passee_au(&st.aujourdhui) { EnRetard } else { AFaire };
    r.date = Some(echeance.jour);
    r.quand = echeance_phrase(&echeance);
    r.source = formater(&modele.source, &[])?;
    r.echeance_depot = Some(echeance.en_json());

    let (engagements, validees) = engagements_valides(st.ctx, mois);
    if !engagement::stock::a_une_saisie(&st.ctx.racine, mois).unwrap_or(false) {
        r.etape = A_SAISIR.into();
        r.detail = format!(
            "Aucun engagement saisi pour {}. S'il n'y a eu aucun cachet, une DSN « néant » se dépose quand même : le mois s'écrit vide, puis se dépose.",
            lib
        );
        return Ok(r);
    }
    r.engagements = engagements;
    r.validees = validees;

    let etat = etat_dsn::du_mois(st.ctx, mois, &Value::Null)?;
    if etat.refusee() {
        r.etape = A_CORRIGER.into();
        let anomalies: Vec<String> = etat.0.get("anomalies").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default();
        r.detail = format!(
            "La DSN de {} a été REFUSÉE : {}. À corriger et à renvoyer.",
            lib,
            if anomalies.is_empty() { "motif non lu".to_string() } else { anomalies.join(" ; ") }
        );
        return Ok(r);
    }

    let s = |n: i64| if n == 1 { "" } else { "s" };
    if let Some((nom, chemin)) = dsn_deposable(st, mois) {
        r.etape = A_DEPOSER.into();
        r.detail = format!(
            "{} engagement{}, {} validé{}. Le mois est écrit ({}) ; la DSN n'est pas partie.",
            r.engagements,
            s(r.engagements),
            r.validees,
            s(r.validees),
            nom
        );
        r.fichier = Some(chemin);
    } else {
        r.etape = A_PRODUIRE.into();
        r.detail = format!(
            "{} engagement{} saisi{}, {} validé{}. Le mois n'est pas encore écrit : bulletins, DSN et le reste s'écrivent d'abord, puis la DSN se dépose.",
            r.engagements,
            s(r.engagements),
            s(r.engagements),
            r.validees,
            s(r.validees)
        );
    }
    if r.engagements != 0 && r.validees < r.engagements {
        let reste = r.engagements - r.validees;
        r.detail.push_str(&format!(" Il reste {} engagement{} non validé{}.", reste, s(reste), s(reste)));
    }
    ajouter_les_mois_de_l_ancien_logiciel(st, mois, &mut r)?;
    Ok(r)
}

/// Ce que la DSN de `mois` declare en plus pour les mois dont l'ancien logiciel a depose la DSN, dit comme la page Paie
/// le dit (`import::mois::dsn_de_l_ancien_logiciel`).
fn ajouter_les_mois_de_l_ancien_logiciel(st: &Etat, mois: &str, r: &mut Echeance) -> R<()> {
    for m in engagement::stock::mois_saisis(st.racine()).unwrap_or_default().into_iter().filter(|m| m.as_str() < mois) {
        let Some(d) = crate::import::mois::dsn_de_l_ancien_logiciel(st.ctx, &m)? else { continue };
        if d["porteur"] == json!(mois) && d["engagements"].as_u64().is_some_and(|n| n > 0) {
            let lib = socle::mois::Mois::lire(&m).map(|m| m.libelle(socle::mois::Graphie::Accentuee)).map_err(Refus::from)?;
            r.detail.push_str(&format!(" {} : {}", socle::texte::capitaliser(&lib), en_texte(&d["phrase"])));
        }
    }
    Ok(())
}

// ------------------------------------------------------ les obligations dues

/// CE QUI EST DU, A L'OUVERTURE : toute obligation que `obligations.rs` dit a faire ou en retard -- la
/// fin d'un contrat qui se signale, et ce qu'un arret fait naitre (le signalement d'arret ou de reprise anticipee, la
/// declaration d'accident du travail ou de trajet sous quarante-huit heures, l'attestation de salaire du deuil d'enfant).
/// UN SEUL MECANISME : chaque obligation devient un rappel par `rappel_d_obligation`, qui porte l'obligation telle que le
/// moteur l'a calculee (sa date butoir, son etat, sa phrase, ses textes) et ce sur quoi elle porte (`objet` : la fin de
/// contrat de la route `fins-de-contrat`, ou l'arret resume comme la route `imprevus` le resume) -- la page en tire le meme
/// geste que dans la colonne du mois. Le plus presse d'abord : en retard, puis par date butoir.
///
/// Ils se taisent des que la chose est faite -- le depot REEL du signalement note au journal, la declaration ou
/// l'attestation dite faite (`accident-declare`, `attestation-declare`) --, jamais parce qu'on a clique « plus tard ».
/// Ce qui n'est pas encore du (`a_venir`), ce qui est a la charge du salarie (la maladie professionnelle) et ce dont le
/// delai ne se compte pas (`indetermine` : il n'a pas de date butoir ; la colonne du mois le montre) ne s'y rappellent pas.
fn rappels_obligations(st: &Etat) -> R<Vec<Echeance>> {
    let mut out = Vec::new();

    // LES FINS DE CONTRAT : celles du mois passe et du mois en cours, deja finies (`crate::routes::imprevus::fins_a_signaler`, la
    // meme liste que la colonne du mois), et celles d'avant dont le signalement est en retard (`fins_en_retard_avant`). Un CDD
    // d'usage du spectacle arrive a son terme (031) n'en fait pas partie.
    let mois = st.aujourdhui.mois_civil().to_string();
    let precedent = st.aujourdhui.mois_civil().precedent().to_string();
    let mut fins = crate::routes::imprevus::fins_en_retard_avant(st.ctx, &precedent, st.aujourdhui)?;
    for m in [precedent, mois.clone()] {
        fins.extend(crate::routes::imprevus::fins_a_signaler(st.ctx, &m, st.aujourdhui)?);
    }
    for f in fins {
        let m = Mois::lire(&en_texte(&f["mois"]))?;
        let fin = Jour::lire(&en_texte(&f["fin"]))?;
        if fin > st.aujourdhui {
            continue; // il n'a pas encore pris fin
        }
        let o = &f["obligation"];
        if !du(o) {
            continue;
        }
        let salarie = en_texte(&f["salarie"]);
        let a_saisir: Vec<String> = f["a_saisir"].as_array().map(|l| l.iter().map(crate::routes::imprevus::phrase_du_manque).collect()).unwrap_or_default();
        let mut r = rappel_d_obligation(
            o,
            format!("fin-contrat:{}:{}", salarie, en_texte(&f["fin"])),
            format!("Signaler la fin du contrat de {}", salarie),
            format!("Le contrat {} s'est terminé le {}.", texte(f.get("numero")), fin.jj_mm_aaaa()),
            f.clone(),
        );
        r.mois = Some(m);
        r.salarie = Some(salarie.clone());
        r.etape = if a_saisir.is_empty() { A_PRODUIRE } else { A_SAISIR }.into();
        if !a_saisir.is_empty() {
            r.detail.push_str(&format!(" À saisir d'abord, dans la carte du contrat : {}", a_saisir.join(" ")));
        }
        r.a_saisir = a_saisir;
        out.push(r);
    }

    // CE QU'UN ARRET FAIT NAITRE : tous les arrets du magasin des imprevus (`crate::routes::imprevus::tous_les_arrets`), leurs
    // obligations comptees par `obligations::de_l_arret` au jour du rappel.
    let arrets = crate::routes::imprevus::tous_les_arrets(st.ctx)?;
    if !arrets.is_empty() {
        let n = crate::routes::imprevus::nomenclatures(st.ctx)?;
        let suivi = crate::obligations::Suivi::lire(st.ctx)?;
        for a in &arrets {
            let obligations = crate::obligations::de_l_arret(st.ctx, a, &suivi, st.aujourdhui)?;
            if !obligations.iter().any(du) {
                continue;
            }
            let mut resume = crate::routes::imprevus::resume(st.ctx, &n, a)?;
            resume["obligations"] = Value::Array(obligations.clone());
            let salarie = st.ctx.nomme(&en_texte(&a["salarie"]));
            for o in obligations.iter().filter(|o| du(o)) {
                let mut r = rappel_d_obligation(
                    o,
                    format!("{}:{}:{}", en_texte(&o["cle"]), en_texte(&a["salarie"]), en_texte(&a["dernier_jour_travaille"])),
                    format!("{} — {}", en_texte(&o["libelle"]), salarie),
                    format!("{}, {}.", arret_en_clair(&resume), periode_de_l_arret(&resume)),
                    resume.clone(),
                );
                r.mois = Some(Jour::lire(&en_texte(&a["dernier_jour_travaille"]))?.mois_civil());
                r.salarie = Some(en_texte(&a["salarie"]));
                out.push(r);
            }
        }
    }

    // Le plus presse d'abord : ce qui est en retard, puis par date butoir (le tri est stable : a butoir egale, l'ordre
    // de lecture reste).
    out.sort_by_key(|r| (r.etat != EnRetard, r.date));
    Ok(out)
}

/// Une obligation due : a faire, ou en retard.
fn du(o: &Value) -> bool {
    matches!(o.get("etat").and_then(Value::as_str), Some(crate::obligations::A_FAIRE) | Some(crate::obligations::EN_RETARD))
}

/// Le rappel d'une obligation : son titre, sa question, et ce que le moteur en a calcule -- la date butoir (`jour`), le
/// retard, la phrase -- ; `objet` : ce sur quoi elle porte, que la page repasse au geste qui la fait.
fn rappel_d_obligation(o: &Value, cle: String, titre: String, question: String, objet: Value) -> Echeance {
    let butoir = o.get("butoir").and_then(Value::as_str).map(str::to_string);
    let tombe = butoir.as_deref().and_then(|b| Mois::lire(b.get(..7)?).ok());
    let mut r = Echeance::nouveau(OBLIGATION, cle, titre, question, tombe);
    r.etat = if o.get("etat").and_then(Value::as_str) == Some(crate::obligations::EN_RETARD) { EnRetard } else { AFaire };
    r.etape = en_texte(&o["etat"]);
    if let Some(j) = butoir.as_deref().and_then(|b| Jour::lire(b).ok()) {
        r.le(&j);
    }
    let delai = o.get("delai").and_then(Value::as_str).unwrap_or("");
    let phrase = o.get("phrase").and_then(Value::as_str).unwrap_or("");
    r.detail = if delai.is_empty() { phrase.to_string() } else { format!("Délai : {}. {}", delai, phrase) };
    r.obligation = Some(o.clone());
    r.objet = Some(objet);
    r
}

/// « Arrêt de travail (accident du travail) » : ce qu'est l'arret, avec le libelle de son motif.
fn arret_en_clair(resume: &Value) -> String {
    format!("Arrêt de travail ({})", en_texte(&resume["motif_libelle"]).to_lowercase())
}

/// Sa periode : depuis son premier jour ; jusqu'a la reprise si elle est dite, sinon la fin prescrite.
fn periode_de_l_arret(resume: &Value) -> String {
    let jj = |k: &str| Jour::lire(&en_texte(&resume[k])).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| en_texte(&resume[k]));
    if vrai(&resume["reprise"]) {
        format!("du {} au {}", jj("premier_jour"), jj("dernier_jour"))
    } else {
        format!("depuis le {}, prescrit jusqu'au {}", jj("premier_jour"), jj("dernier_jour"))
    }
}

// ------------------------------------------------------------- les DPAE


/// Les DPAE a envoyer : celles des engagements valides (`etapes::Moteur::embauches_a_declarer`), a toute distance de
/// l'embauche, celles en retard comprises. Une par jour d'embauche, avec la charge de
/// son depot si elle est testee -- le bouton ouvre alors directement ce depot. Se tait
/// des que la DPAE reelle est au journal.
fn rappels_dpae(st: &Etat) -> R<Vec<Echeance>> {
    let aujourdhui = st.aujourdhui;
    let mut jours: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    let a_faire = crate::routes::etapes::Moteur::nouveau(st.ctx).embauches_a_declarer(None)?;
    for e in a_faire.as_array().into_iter().flatten() {
        jours.entry((texte_ou_vide(e, "mois"), texte_ou_vide(e, "debut"))).or_default().push(e.clone());
    }

    let mut out = Vec::new();
    for ((mois, debut), groupe) in jours {
        let jour = Jour::lire(&debut)?;
        let retard = jour < aujourdhui;
        let noms = groupe.iter().map(|e| format!("{} ({})", texte_ou_vide(e, "nom"), texte_ou_vide(e, "spectacle"))).collect::<Vec<_>>().join(", ");
        let quand = if retard {
            format!("L'embauche a commencé le {} : la DPAE est EN RETARD, à envoyer tout de suite.", jj_mm_aaaa(&debut))
        } else if jour == aujourdhui {
            "L'embauche est aujourd'hui : la DPAE doit partir avant la première heure de travail.".to_string()
        } else {
            let reste = jour.moins(&aujourdhui);
            format!(
                "Elle doit partir avant la première heure de travail — il reste {} jour{}.",
                reste,
                if reste == 1 { "" } else { "s" }
            )
        };
        let mut r = Echeance::nouveau(
            DPAE,
            format!("dpae:{}", debut),
            format!("DPAE à envoyer — embauche du {}", jj_mm_aaaa(&debut)),
            format!("Pas de trace d'envoi pour : {}.", noms),
            Some(jour.mois_civil()),
        );
        r.mois = Some(Mois::lire(&mois)?);
        r.le(&jour);
        r.quand = format!("avant la première heure de travail, le {}", jour.jj_mm_aaaa());
        r.detail = quand;
        r.etat = if retard { EnRetard } else { AFaire };
        r.engagements = groupe.len() as i64;
        out.push(r);
    }
    Ok(out)
}

/// LES ACCES AUX ORGANISMES (les mots de passe et les comptes du coffre, `coffre::coffre_etat_de`) : aucun ne bloque.
/// On configure, on saisit, on fabrique la DPAE et la DSN sans eux ; ils servent a envoyer depuis le logiciel, et chacun
/// est un choix. « Pour demarrer » propose ceux qui ne sont pas poses, avec ce qu'ils permettent (`sinon` : ce qui se
/// fait sans eux) ; une compagnie dont rien ne part (envois simules, ou la demo : `bridage::branchement`) n'en propose
/// aucun et le dit, une fois.
fn acces_proposes(ctx: &Contexte) -> socle::echec::R<(Vec<AccesPropose>, Option<AccesSansObjet>)> {
    use envois::branchement::Branchement;
    let (texte, mots) = match crate::compagnie::bridage::branchement(&ctx.racine) {
        Branchement::Monde => ("", ""),
        Branchement::Simulateur => ("Les envois de cette compagnie sont simulés : rien ne part, ses mots de passe et ses comptes ne se demandent pas.", "simulé"),
        Branchement::Message(_) => ("Dans la démo, rien ne part de cette compagnie : ses mots de passe et ses comptes ne se demandent pas.", "démo"),
    };
    if !mots.is_empty() {
        let sans_objet = AccesSansObjet { cle: "acces", libelle: "Les accès aux organismes", ou: OU_DES_ACCES, onglet: "coffre", champ: None, texte, pastille: Pastille::de("information", mots) };
        return Ok((Vec::new(), Some(sans_objet)));
    }
    let etat = super::coffre::coffre_etat_de(&ctx.racine, &ctx.entreprise)?;
    let mut out = Vec::new();
    for s in etat["services"].as_array().into_iter().flatten() {
        let pose = vrai(&s["pose"]) || vrai(&s["environnement"]);
        let compte_dit = s["identifiant"]["manque"].as_array().is_none_or(Vec::is_empty);
        if pose && compte_dit {
            continue;
        }
        let dit = |k: &str| s[k].as_str().unwrap_or("").to_string();
        out.push(AccesPropose::nouveau(format!("acces.{}", dit("service")), dit("libelle"), format!("{} depuis le logiciel", dit("sert_a")), "vous le faites vous-même, hors du logiciel"));
    }
    // Le Drive de la sauvegarde : se lie au meme endroit (« Lier le Drive »), sans reseau pour le savoir.
    if super::coffre::compte_autorise(&ctx.racine).is_none() {
        out.push(AccesPropose::nouveau("acces.google".into(), "Google Drive".into(), "sauvegarder la compagnie dans votre Drive".into(), "rien ne se sauvegarde hors de ce poste"));
    }
    Ok((out, None))
}

const OU_DES_ACCES: &str = "Configuration › Mots de passe";

/// UN ACCES PROPOSE, une ligne de « Pour demarrer » (`declarations::demarrage::route`, qui y ajoute sa phrase) : ce qu'il
/// permet (`utile`), ce qui se fait sans lui (`sinon`), ou il se pose.
#[derive(Debug, Serialize)]
struct AccesPropose {
    cle: String,
    libelle: String,
    utile: String,
    sinon: &'static str,
    ou: &'static str,
    onglet: &'static str,
    /// Aucune case : l'onglet du coffre.
    champ: Option<&'static str>,
    trouver: &'static str,
}

impl AccesPropose {
    fn nouveau(cle: String, libelle: String, utile: String, sinon: &'static str) -> AccesPropose {
        AccesPropose { cle, libelle, utile, sinon, ou: OU_DES_ACCES, onglet: "coffre", champ: None, trouver: "" }
    }
}

/// Une compagnie dont rien ne part : les acces ne se demandent pas, ce que « Pour demarrer » dit en information.
#[derive(Debug, Serialize)]
struct AccesSansObjet {
    cle: &'static str,
    libelle: &'static str,
    ou: &'static str,
    onglet: &'static str,
    champ: Option<&'static str>,
    texte: &'static str,
    pastille: Pastille,
}
