//! Ce qui est du, et quand : `echeances` est la seule fonction qui compose les echeances de la compagnie (les DPAE, les
//! obligations de `obligations.rs`, la DSN de chaque mois, les echeances annuelles de `donnees/echeancier-base.json` --
//! la cloture de l'exercice apres la DSN de decembre, l'export compta, la revue des spectacles, le taux AT/MP, les
//! contributions de branche). Chacune porte sa date et dit si elle est due maintenant (`due`). Les deux routes la lisent :
//! `echeancier` (tout, date, ce mois-ci et l'annee qui vient) et `rappels` (ce qui est du maintenant, avec son geste).
//! Le panneau et l'horloge lisent les mois dont la DSN est due (`mois_a_declarer`, `mois_a_parcourir`), qu'elle compose.
//! Aucun calcul de paie : les engagements se lisent sans chiffrer, et leur etat (`engagement::etats`), le journal par
//! `etats`.
//!
//! Un refus porte sa `classe`.

use socle::date::jj_mm_aaaa;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::{archivee, Contexte};
use socle::date::{en_texte, Jour};
use engagement;
use envois::etats;
use crate::compagnie::etat_dsn;
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus as RefusFiche, FORME, MANQUE, OPERATION};
use crate::routes::qui_de;
use crate::routes::fiches::{entier, exiger};
use crate::routes::ouverture::marquer_echeance;
use envois::etats::{getd, items};
use socle::echec::{erreur, Echec};
use socle::valeur::{nom_type, valeurs_egales};
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
        nom: "rappels",
        geste: "Dire ce que la compagnie a à faire ce jour : DSN, fins de contrat, DPAE…",
        charge: &[],
        exemple: r#"{}"#,
        rend: "rappels : les échéances dues maintenant (chacune : quoi, titre, question, mois ou année, état et sa pastille, sa date ; action : la route que son geste ouvre, sa charge, puis la route de l'acte, le mot du bouton)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "echeancier",
        geste: "Lire les échéances du mois et de l'année.",
        charge: &[],
        exemple: "{}",
        rend: "ce_mois et a_venir : les échéances, chacune avec sa date, son état et si un rappel la dit",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "revue-spectacles",
        geste: "Archiver les spectacles qui ne continuent pas, et noter la revue faite.",
        charge: &[ANNEE, Champ { nom: "continuent", libelle: Affiche::new("Spectacles qui continuent"), forme: Forme::Liste, sens: "les spectacles qui continuent (leurs clés) ; les autres s'archivent", avance: false }],
        exemple: r#"{"annee":2026,"continuent":["le-banc-des-menteurs"]}"#,
        rend: "phrase, archives, continuent, configuration",
        refus: &[MANQUE, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        nom: "pour-demarrer",
        geste: "Dire ce que la compagnie peut remplir d'avance pour ses premiers actes.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "titre, note, manques (chacun sa case, sa phrase, où il vit ; une ligne d'un salarié dit qui), poses (ce que le moteur a posé, en information)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        nom: "echeance-faite",
        geste: "Cocher une échéance annuelle que seule la compagnie peut dire faite.",
        charge: &[
            Champ { nom: "cle", libelle: Affiche::new("Échéance"), forme: Forme::Texte, sens: "l'échéance et son année, « quoi:AAAA » (celle que rappels ou echeancier rendent)", avance: false },
            Champ { nom: "fait", libelle: Affiche::new("Faite"), forme: Forme::OuiNon, sens: "faux : la décocher", avance: false },
        ],
        exemple: r#"{"cle":"revue-spectacles:2026"}"#,
        rend: "cle, fait",
        refus: &[RefusFiche { classe: "ErreurOperation", corriger: "une échéance qui se coche, avec son année" }, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
];

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> socle::echec::R<Value> {
    match nom {
        "rappels" => rappels(ctx).map(|mut v| {
            let lignes: Vec<Value> = v["rappels"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| {
                    let quand = r["quand"].as_str().filter(|q| !q.is_empty()).map(|q| format!(" Échéance : {}.", q)).unwrap_or_default();
                    json!(format!("{} ({}) : {} {}{}", super::mot(&r["titre"]), super::mot(&r["pastille"]["mots"]), super::mot(&r["question"]), super::mot(&r["detail"]), quand).replace("  ", " "))
                })
                .collect();
            let p = format!("Rappels : {}.", lignes.len());
            v["phrases"] = Value::Array(lignes);
            super::phrase_en_tete(v, p)
        }),
        "echeancier" => echeancier(ctx).map(|mut v| {
            let ligne = |x: &Value| {
                let quand = super::mot(&x["quand"]);
                json!(format!("{} ({}{})", super::mot(&x["titre"]), super::mot(&x["pastille"]["mots"]), if quand.is_empty() { String::new() } else { format!(", {}", quand) }))
            };
            let ce_mois: Vec<Value> = v["ce_mois"].as_array().into_iter().flatten().map(ligne).collect();
            let n = ce_mois.len();
            let mut lignes = ce_mois;
            lignes.extend(v["a_venir"].as_array().into_iter().flatten().map(ligne));
            let p = format!("{} : {} ce mois-ci, {} à venir.", socle::valeur::valeur_en_texte(&v["compagnie"]), super::compte(n, "échéance", "échéances"), super::nombre(&v["a_venir"]));
            v["phrases"] = Value::Array(lignes);
            super::phrase_en_tete(v, p)
        }),
        "revue-spectacles" => revue_spectacles(o, ctx, charge).map(|v| {
            let p = format!("Spectacles qui continuent : {} ; archivés : {}.", super::nombre(&v["continuent"]), super::nombre(&v["archives"]));
            super::phrase_en_tete(v, p)
        }),
        "echeance-faite" => echeance_faite(ctx, charge).map(|v| {
            let p = if v["fait"] == json!(true) { "Échéance cochée : faite." } else { "Échéance décochée." };
            super::phrase_en_tete(v, p)
        }),
        // Ce que la compagnie peut remplir d'avance, d'un coup ; ce que le moteur a pose de lui-meme se dit en information
        // (gris) ; les acces aux organismes et le Drive se proposent (`acces_proposes`).
        "pour-demarrer" => {
            let (proposes, simules) = acces_proposes(ctx)?;
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
fn revue_spectacles(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> socle::echec::R<Value> {
    let annee = entier(exiger(charge, "annee")?)?;
    let continuent = etats::ou(getd(charge, "continuent", &etats::LISTE_VIDE)?, &etats::LISTE_VIDE);
    // `set(continuent)` : l'ordre ne compte pas, les doublons non plus.
    let mut garder: Vec<Value> = Vec::new();
    for c in items(continuent)? {
        if matches!(c.as_ref(), Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(&c))));
        }
        if !garder.iter().any(|g| valeurs_egales(g, &c)) {
            garder.push(c.into_owned());
        }
    }
    let actifs = ctx.spectacles_actifs();
    let est_actif = |v: &Value| v.as_str().is_some_and(|s| actifs.contains(&s));
    let mut inconnus: Vec<&Value> = garder.iter().filter(|g| !est_actif(g)).collect();
    if !inconnus.is_empty() {
        if let Some((i, autre)) = inconnus.iter().enumerate().find(|(_, c)| !c.is_string()) {
            return Err(erreur("TypeError", socle::fichiers::element(i, &socle::fichiers::attendu("un texte", nom_type(autre)))));
        }
        inconnus.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        let noms: Vec<&str> = inconnus.iter().filter_map(|c| c.as_str()).collect();
        return Err(erreur("ErreurOperation", format!("Spectacle actif inconnu : {}.", noms.join(", "))));
    }
    let archives: Vec<String> = actifs.iter().filter(|c| !garder.iter().any(|g| g.as_str() == Some(c))).map(|c| c.to_string()).collect();
    for cle in &archives {
        crate::routes::fiches::activer_fiche(ctx, "spectacle", cle, false)?;
    }
    marquer_echeance(ctx, &format!("revue-spectacles:{}", annee), true, "interface")?;
    let mut continuent: Vec<String> = garder.iter().filter_map(|g| g.as_str().map(String::from)).collect();
    continuent.sort();
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(|m| socle::echec::noter_et(None, m))?;
    let configuration = crate::routes::ouverture::route(o, &neuf, "configuration", &json!({}))?;
    Ok(json!({"annee": annee, "archives": archives, "continuent": continuent, "configuration": configuration}))
}

/// `echeance-faite` : coche (`fait`) ou decoche l'echeance `charge["cle"]`, au nom de `qui`.
fn echeance_faite(ctx: &Contexte, charge: &Value) -> socle::echec::R<Value> {
    let cle = exiger(charge, "cle")?;
    let fait = charge.get("fait") != Some(&Value::Bool(false));
    let Some(texte) = cle.as_str() else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(cle))));
    };
    marquer_echeance(ctx, texte, fait, &qui_de(charge))?;
    Ok(json!({"cle": texte, "fait": fait}))
}


// ------------------------------------------------------------------ outils

/// Une valeur facultative en texte (`None` si absente).
pub(super) fn texte(v: Option<&Value>) -> String {
    v.map_or("None".to_string(), en_texte)
}

fn vrai_opt(v: Option<&Value>) -> bool {
    v.map_or(false, vrai)
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
const A_FAIRE: &str = "a_faire";
const FAIT: &str = "fait";
const EN_RETARD: &str = "en_retard";
const INFORMATION: &str = "information";

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
    /// Le mois ou elle tombe (AAAA-MM) : l'echeancier la range sous ce mois.
    tombe: String,
    /// Son dernier jour (ISO), quand elle en a un.
    date: Option<String>,
    /// Quand, en clair (« vendredi 15/01/2027 à midi », « après la DSN de décembre… »).
    quand: String,
    /// `a_faire`, `en_retard`, `fait`, `information`.
    etat: &'static str,
    /// DUE MAINTENANT : son moment est venu et elle n'est pas faite. Les rappels sont exactement celles-ci.
    due: bool,
    /// Ou en est la chose : le mois d'une DSN (`a_saisir`, `a_produire`, `a_deposer`, `a_corriger`), l'etat d'une obligation.
    etape: String,
    /// Seule la compagnie peut la dire faite (`echeance-faite`).
    cochable: bool,
    /// Le mois sur lequel elle porte (la DSN de ce mois, la DPAE d'un engagement de ce mois).
    mois: Option<String>,
    annee: Option<String>,
    fichier: Option<String>,
    engagements: i64,
    validees: i64,
    spectacles: Vec<Value>,
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
    fn nouveau(quoi: &str, cle: String, titre: String, question: String, tombe: String) -> Echeance {
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
            etat: A_FAIRE,
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
        self.date = Some(jour.iso());
        self.quand = format!("le {}", jour.jj_mm_aaaa());
    }

    /// LE GESTE QUI LA FAIT, en routes du catalogue (`etapes::action` : la route qu'il ouvre, sa charge, la route de
    /// l'acte qui suit, le mot du bouton), celles que l'ecran et le LLM appellent. Une obligation, lue sur ses champs : la
    /// fin d'un contrat se signale (ou, s'il manque une case, se complete dans sa carte : la saisie du mois, puis
    /// l'engagement modifie) ; un signalement DSN d'arret ou de reprise se signale ; une obligation hors DSN qu'une route
    /// dit faite (`accident-declare`, `attestation-declare`) se dit faite ; le reste s'ouvre dans son mois. Une DSN, une
    /// DPAE : l'apercu de son depot, puis le depot. La cloture, l'export compta, la revue des spectacles : leur route.
    fn action(&self) -> Value {
        use crate::routes::etapes::{action, deposer_dsn};
        let mois = self.mois.clone().unwrap_or_default();
        let annee = || json!({"annee": self.annee.clone().unwrap_or_default()});
        let objet = self.objet.as_ref().unwrap_or(&Value::Null);
        match self.quoi.as_str() {
            OBLIGATION => {
                let Some(o) = self.obligation.as_ref() else { return Value::Null };
                let arret = || json!({"salarie": objet["salarie"], "dernier_jour": objet["dernier_jour_travaille"]});
                match o.get("cle").and_then(Value::as_str).unwrap_or("") {
                    "signalement_fin_contrat" if !self.a_saisir.is_empty() => {
                        action("saisie", json!({"mois": mois}), Some("modifier-engagement"), "Compléter le contrat".into())
                    }
                    "signalement_fin_contrat" => {
                        action("fin-contrat-signaler", json!({"mois": mois, "salarie": objet["salarie"]}), Some("deposer"), "Signaler…".into())
                    }
                    cle @ ("signalement_arret" | "signalement_arret_rectificatif" | "signalement_reprise") => {
                        let mut charge = arret();
                        charge["nature"] = json!(if cle == "signalement_reprise" { "05" } else { "04" });
                        action("arret-signaler", charge, Some("deposer"), "Signaler…".into())
                    }
                    "declaration_accident" if o.get("ou").is_some_and(Value::is_array) => action("accident-declare", arret(), None, "Faite le".into()),
                    "attestation_salaire" if o.get("ou").is_some_and(Value::is_array) => action("attestation-declare", arret(), None, "Faite le".into()),
                    _ => action("etat-du-mois", json!({"mois": mois}), None, "Ouvrir le mois".into()),
                }
            }
            DSN => deposer_dsn(&mois, &Value::Null, "Vérifier et envoyer".into()),
            DPAE => action("depot-apercu", json!({"quoi": DPAE, "mois": mois}), Some("deposer"), "Préparer la DPAE".into()),
            CLOTURE => action("cloture", annee(), None, format!("Clôturer {}", self.annee.clone().unwrap_or_default()).trim().to_string()),
            EXPORT_COMPTA => action("export-compta", annee(), None, "Exporter pour la compta".into()),
            // Tous continuent, tant qu'on n'en a pas décoché un : la boîte s'ouvre ainsi.
            REVUE_SPECTACLES => {
                let mut charge = annee();
                charge["continuent"] = self.spectacles.iter().map(|s| s["cle"].clone()).collect();
                action("revue-spectacles", charge, None, "Choisir les spectacles".into())
            }
            _ => Value::Null,
        }
    }

    /// L'échéance en objet JSON, champ par champ.
    fn json(&self) -> Value {
        let opt = |o: &Option<String>| o.clone().map_or(Value::Null, Value::String);
        json!({
            "quoi": self.quoi, "cle": self.cle, "titre": self.titre, "question": self.question, "detail": self.detail,
            "source": self.source, "tombe": self.tombe, "date": opt(&self.date), "quand": self.quand, "etat": self.etat,
            "en_retard": self.etat == EN_RETARD, "rappel": self.due, "etape": self.etape, "cochable": self.cochable,
            // Sa pastille : la couleur et les mots de son etat (`super::pastille`).
            "pastille": super::pastille(self.etat, match self.etat { EN_RETARD => "en retard", A_FAIRE => "à faire", FAIT => "fait", _ => "pour info" }),
            "mois": opt(&self.mois), "annee": opt(&self.annee), "fichier": opt(&self.fichier),
            "engagements": self.engagements, "validees": self.validees,
            "spectacles": self.spectacles.iter().map(brut_valeur).collect::<Vec<_>>(),
            "salarie": opt(&self.salarie), "a_saisir": self.a_saisir,
            "echeance_depot": self.echeance_depot,
            "obligation": self.obligation, "objet": self.objet.as_ref().map(brut_valeur),
            // Le geste qui la fait (une route du catalogue et son bouton), la case « C'est fait », le titre de sa boite.
            "action": self.action(),
            "bouton_coche": if self.etat == FAIT { "Pas encore fait" } else { "C'est fait" },
            "titre_boite": self.annee.as_ref().map(|a| format!("Les spectacles de {}", a)),
        })
    }
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
pub(crate) fn rappels(ctx: &Contexte) -> socle::echec::R<Value> {
    let st = Etat::de(ctx)?;
    let out = echeances(&st)?;
    let rappels: Vec<Value> = out.iter().filter(|e| e.due).map(Echeance::json).collect();
    // La boite des rappels se ferme « plus tard » quand il y a quelque chose à faire.
    let fermer = if rappels.is_empty() { "Fermer" } else { "Plus tard" };
    Ok(json!({ "rappels": rappels, "fermer_dit": fermer }))
}

/// `echeancier` : les memes echeances, rangees. « Ce mois-ci » : celles qui sont dues (un rappel les dit, avec leur
/// geste) et celles qui tombent ce mois ; « a venir » : les autres, sur douze mois.
fn echeancier(ctx: &Contexte) -> socle::echec::R<Value> {
    let st = Etat::de(ctx)?;
    let courant = st.aujourdhui.mois_civil().to_string();
    let fin = st.aujourdhui.mois_civil().plus(MOIS_A_VENIR)?.to_string();
    let (mut ce_mois, mut a_venir) = (Vec::new(), Vec::new());
    for e in echeances(&st)? {
        if e.due || e.tombe == courant {
            ce_mois.push(e);
        } else if courant < e.tombe && e.tombe <= fin {
            a_venir.push(e);
        }
    }
    let ordre = |etat: &str| match etat {
        EN_RETARD => 0,
        A_FAIRE => 1,
        INFORMATION => 2,
        _ => 3,
    };
    ce_mois.sort_by(|a, b| (ordre(a.etat), a.date.clone().unwrap_or_else(|| a.tombe.clone())).cmp(&(ordre(b.etat), b.date.clone().unwrap_or_else(|| b.tombe.clone()))));
    a_venir.sort_by(|a, b| (a.tombe.clone(), a.date.clone().unwrap_or_default()).cmp(&(b.tombe.clone(), b.date.clone().unwrap_or_default())));
    let texte = |c: &str| ctx.entreprise.get(c).filter(|v| vrai(v)).map(en_texte);
    let nom = texte("nom_affichage").or_else(|| texte("raison_sociale")).unwrap_or_default();
    Ok(json!({
        "aujourdhui": st.aujourdhui.iso(),
        "date": date_en_clair(&st.aujourdhui),
        "date_dit": format!("Nous sommes le {}", date_en_clair(&st.aujourdhui)),
        "bienvenue": "Bienvenue sur PuppetPaie",
        "compagnie": nom,
        "idcc": ctx.entreprise.get("idcc").cloned().unwrap_or(Value::Null),
        "ce_mois": ce_mois.iter().map(Echeance::json).collect::<Vec<_>>(),
        "a_venir": a_venir.iter().map(Echeance::json).collect::<Vec<_>>(),
        "a_faire": ce_mois.iter().filter(|x| !x.due && (x.etat == A_FAIRE || x.etat == EN_RETARD)).count(),
    }))
}

/// TOUT CE QUI EST DU, ET QUAND : la seule fonction qui compose les echeances, dans l'ordre ou elles se font. D'abord
/// les DPAE (elles se comptent en heures, avant la premiere heure de travail), puis les obligations nees d'un evenement
/// (quarante-huit heures pour un accident, cinq jours ouvres pour un signalement), la DSN des mois passes qui n'en ont
/// pas, les echeances annuelles (la cloture apres la DSN de decembre, l'export compta, la revue des spectacles, le taux
/// AT/MP, les contributions de branche), et la DSN des douze mois qui viennent. Une echeance se tait des qu'elle est
/// faite ; aucune ne se tait parce qu'on a clique « plus tard ».
pub(crate) fn echeances(st: &Etat) -> R<Vec<Echeance>> {
    let base: Value = crate::routes::mois::lire_saisie(&socle::fichiers::dossier_du_code().join("echeancier-base.json")).map_err(|e| Refus { classe: e.classe, message: e.message })?;
    let modele = base.get("dsn").ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent("dsn")))?;
    // Un rappel qui ne se calcule pas n'empeche pas les autres.
    let mut out = rappels_dpae(st).unwrap_or_default();
    out.extend(rappels_obligations(st)?);
    let reclames = mois_a_declarer(st)?;
    for m in &reclames {
        out.push(rappel_dsn(st, m, modele)?);
    }
    out.extend(annuelles(st, &base)?);
    // La DSN du mois passe (quand elle n'est plus reclamee) et des douze qui viennent ; rien avant la premiere embauche.
    let premier = etat_dsn::premier_mois_du(st.ctx)?;
    for n in -1..MOIS_A_VENIR {
        let m = st.aujourdhui.mois_civil().plus(n)?.to_string();
        if reclames.contains(&m) || premier.as_ref().is_some_and(|p| !p.is_empty() && m < *p) {
            continue;
        }
        out.push(dsn_a_venir(st, &m, modele)?);
    }
    Ok(out)
}

/// La DSN d'un mois que les rappels ne reclament pas : faite, ou a venir.
fn dsn_a_venir(st: &Etat, mois: &str, modele: &Value) -> R<Echeance> {
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    let mut e = Echeance::nouveau(DSN, format!("dsn:{}", mois), dsn_formee(modele, "titre", &[("de_mois", socle::texte::de(&socle::mois::Mois::lire(mois)?.libelle(socle::mois::Graphie::Accentuee)))])?, String::new(), socle::mois::Mois::lire(mois)?.suivant().to_string());
    e.etat = if !etat_dsn::due(st.ctx, mois, &Value::Null)? { FAIT } else if echeance.passee_au(&st.aujourdhui) { EN_RETARD } else { A_FAIRE };
    e.due = false;
    e.mois = Some(mois.to_string());
    e.detail = dsn_detail(modele, &echeance)?;
    e.source = dsn_formee(modele, "source", &[])?;
    e.date = Some(echeance.jour.iso());
    e.quand = echeance_phrase(&echeance);
    e.echeance_depot = Some(echeance.en_json());
    Ok(e)
}

fn echeance_phrase(e: &declarations::echeance_dsn::Echeance) -> String {
    e.en_json()["phrase"].as_str().map(String::from).unwrap_or_else(|| e.jour.jj_mm_aaaa())
}

fn dsn_formee(modele: &Value, champ: &str, valeurs: &[(&str, String)]) -> R<String> {
    formater(modele.get(champ).and_then(Value::as_str).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent(champ)))?, valeurs)
}

fn dsn_detail(modele: &Value, echeance: &declarations::echeance_dsn::Echeance) -> R<String> {
    dsn_formee(
        modele,
        "detail",
        &[("jour", echeance.jour.jour.to_string()), ("mois_suivant", echeance.jour.mois_civil().libelle(socle::mois::Graphie::Accentuee)), ("report", echeance.pourquoi().map(|p| format!(" {}", p)).unwrap_or_default())],
    )
}

/// LES ECHEANCES ANNUELLES (`echeancier-base.json`, celles de la convention de la fiche), de l'annee en cours et de la
/// suivante. La cloture de l'annee passee vient APRES sa DSN de decembre, et l'export compta avec elle.
fn annuelles(st: &Etat, base: &Value) -> R<Vec<Echeance>> {
    let ctx = st.ctx;
    let faites = cochees(ctx).map_err(|e| Refus { classe: e.classe, message: e.message })?;
    let derniere = st.aujourdhui.mois_civil().plus(MOIS_A_VENIR)?.annee();
    let premier = etat_dsn::premier_mois_du(ctx)?;
    let mut out = Vec::new();
    for annee in st.aujourdhui.an..=derniere {
        for e in annuelles_de(base, ctx.entreprise.get("idcc"))? {
            if let Some(x) = annuelle(st, e, annee, &faites, &premier)? {
                let export = x.quoi == CLOTURE && x.etat != FAIT;
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
        format!("{}-01", annee + 1),
    );
    r.annee = Some(annee.to_string());
    r.detail = "Une écriture par mois, équilibrée : salaires bruts et charges au débit, organismes et net à payer au crédit.".into();
    Some(r)
}

fn mois_de_l_echeance(e: &Value, rang: Option<usize>) -> R<i64> {
    let mois = e.get("mois").and_then(Value::as_array).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent("mois")))?;
    let x = match rang {
        Some(i) => mois.get(i),
        None => mois.last(),
    };
    x.and_then(Value::as_i64).ok_or_else(|| avec_classe("IndexError", socle::fichiers::trop_courte("les mois de l'échéance")))
}

/// Une echeance annuelle de l'annee `annee` ; `None` quand elle ne concerne pas ce dossier.
fn annuelle(st: &Etat, e: &Value, annee: i32, faites: &serde_json::Map<String, Value>, premier: &Option<String>) -> R<Option<Echeance>> {
    let ctx = st.ctx;
    let cle = e.get("cle").and_then(Value::as_str).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent("cle")))?;
    let comment = e.get("fait_par").and_then(Value::as_str);
    let precedente = (annee - 1).to_string();
    // La cloture d'une annee dont aucun mois n'est dans PuppetPaie (tenue ailleurs) ne concerne pas ce dossier.
    if comment == Some("exercice_clos") && mois_de_l_annee(ctx, &precedente).is_empty() {
        return Ok(None);
    }
    // Une revue des spectacles sans spectacle n'a rien a revoir.
    if cle == REVUE_SPECTACLES && ctx.spectacles.is_empty() {
        return Ok(None);
    }
    let debut = format!("{}-{:02}", annee, mois_de_l_echeance(e, Some(0))?);
    let dernier = format!("{}-{:02}", annee, mois_de_l_echeance(e, None)?);
    // Ce qui tombait avant l'arrivee de la compagnie ne la concerne pas -- sauf le taux AT/MP de l'annee, sans lequel
    // rien ne se calcule.
    let arrivee_apres = premier.as_ref().is_some_and(|b| !b.is_empty() && dernier < *b);
    if arrivee_apres && cle != "at-mp" {
        return Ok(None);
    }
    // `{jour_dsn}` : le jour de janvier ou la DSN de decembre de l'annee d'avant est due, reporte (`echeance_dsn`).
    let dsn_decembre = declarations::echeance_dsn::de_la_compagnie(ctx, &format!("{}-12", precedente))?;
    let valeurs = [("annee", annee.to_string()), ("annee_precedente", precedente.clone()), ("jour_dsn", dsn_decembre.jour.jour.to_string())];
    let courant = st.aujourdhui.mois_civil().to_string();
    let champ = |c: &str| -> R<String> { formater(e.get(c).and_then(Value::as_str).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent(c)))?, &valeurs) };
    let mut x = Echeance::nouveau(cle, format!("{}:{}", cle, annee), champ("titre")?, String::new(), debut.clone());
    x.detail = champ("detail")?;
    x.source = e.get("source").and_then(Value::as_str).unwrap_or("").to_string();
    x.cochable = comment == Some("coche");
    x.annee = Some(annee.to_string());
    x.etat = if comment == Some("information") {
        INFORMATION
    } else {
        match fait_annuel(ctx, e, annee, faites)? {
            Some(true) => FAIT,
            // Une compagnie arrivee apres l'echeance n'est pas « en retard » : elle a a le faire.
            Some(false) if courant > dernier && !arrivee_apres => EN_RETARD,
            _ => A_FAIRE,
        }
    };
    // Due : son premier mois est venu, et elle n'est pas faite.
    x.due = matches!(x.etat, A_FAIRE | EN_RETARD) && debut <= courant;
    if cle == CLOTURE {
        x.annee = Some(precedente.clone());
        x.cle = format!("{}:{}", CLOTURE, precedente);
        // LA CLOTURE VIENT APRES LA DSN DE DECEMBRE : tant qu'elle n'est pas partie, c'est son rappel qui parle.
        x.quand = format!("après la DSN de décembre, due {}", echeance_phrase(&dsn_decembre));
        let partie = etat_dsn::du_mois(ctx, &format!("{}-12", precedente), &Value::Null)?;
        x.due = x.due && (partie.partie() || partie.refusee());
        x.etat = if x.etat == EN_RETARD && !x.due { A_FAIRE } else { x.etat };
        if x.due {
            x.titre = format!("Clôturer l'exercice {}", precedente);
            x.question = format!("La DSN de décembre est passée. Figer l'année {} ?", precedente);
        }
    }
    if cle == REVUE_SPECTACLES && x.due {
        x.spectacles = spectacles_a_revoir(ctx, annee)?;
        x.question = format!("Lesquels de vos spectacles se jouent encore en {} ?", annee);
    }
    Ok(Some(x))
}

/// Les echeances annuelles de la convention de la fiche (`idcc`).
pub(crate) fn annuelles_de<'a>(base: &'a Value, idcc: Option<&Value>) -> R<Vec<&'a Value>> {
    let toutes = base.get("annuelles").and_then(Value::as_array).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent("annuelles")))?;
    let voulu = idcc.map_or("None".to_string(), en_texte);
    Ok(toutes
        .iter()
        .filter(|e| match e.get("idcc") {
            Some(l) if vrai(l) => l.as_array().is_some_and(|a| a.iter().any(|x| en_texte(x) == voulu)),
            _ => true,
        })
        .collect())
}

/// Ce que l'utilisateur a coche dans l'echeancier, sous la racine de la compagnie.
pub(crate) const ECHEANCIER: &str = "config/echeancier.json";

/// `config/echeancier.json` : ce que l'utilisateur a coche (`fait`). La seule lecture du fichier ; `ouverture::marquer_echeance` l'ecrit.
pub(crate) fn cochees(ctx: &Contexte) -> socle::echec::R<serde_json::Map<String, Value>> {
    let p = ctx.racine.join(ECHEANCIER);
    if !p.exists() {
        return Ok(serde_json::Map::new());
    }
    let v = crate::routes::mois::lire_saisie(&p)?;
    let Some(o) = v.as_object() else { return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&v)))) };
    match o.get("fait") {
        None => Ok(serde_json::Map::new()),
        Some(Value::Object(f)) => Ok(f.clone()),
        Some(autre) => Err(erreur("TypeError", format!("'fait' : {}", socle::fichiers::attendu("un objet", nom_type(autre))))),
    }
}

/// Vrai, faux, ou `None` quand on ne peut pas le savoir.
fn fait_annuel(ctx: &Contexte, e: &Value, annee: i32, faites: &serde_json::Map<String, Value>) -> R<Option<bool>> {
    let comment = e.get("fait_par").and_then(Value::as_str);
    let cle = e.get("cle").and_then(Value::as_str).unwrap_or("");
    if comment == Some("exercice_clos") {
        return Ok(Some(vrai_opt(exercice_clos(ctx, &(annee - 1).to_string())?.as_ref())));
    }
    // Le taux AT/MP se saisit dans la page : pose pour l'annee (ou un taux collectif que l'annee reprend d'elle-meme,
    // `Contexte::taux_atmp_de`), il vaut releve.
    if cle == "at-mp" && matches!(ctx.taux_atmp_de(annee), Some(Ok(t)) if t.get("valeur").is_some_and(|x| !x.is_null())) {
        return Ok(Some(true));
    }
    if comment == Some("coche") {
        return Ok(Some(faites.contains_key(&format!("{}:{}", cle, annee))));
    }
    Ok(None)
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
fn spectacles_a_revoir(ctx: &Contexte, annee: i32) -> R<Vec<Value>> {
    if cochees(ctx).map_err(|e| Refus { classe: e.classe, message: e.message })?.contains_key(&format!("{}:{}", REVUE_SPECTACLES, annee)) {
        return Ok(Vec::new());
    }
    Ok(ctx
        .spectacles
        .iter()
        .filter(|(_, s)| !archivee(s))
        .map(|(cle, s)| {
            let titre = s.get("titre").filter(|t| vrai(t)).cloned().unwrap_or_else(|| Value::String(cle.clone()));
            let objet = s.get("numero_objet").filter(|t| vrai(t)).cloned().unwrap_or(Value::Null);
            json!({ "cle": cle, "titre": titre, "numero_objet": objet })
        })
        .collect())
}

// ------------------------------------------------------- la DSN des mois

/// Le mois le plus recent dont la DSN reelle est partie et n'a pas ete
/// refusee -- passee, ou en attente de son bilan.
fn derniere_dsn_passee(st: &Etat) -> R<Option<String>> {
    let mut mois = BTreeSet::new();
    for l in lire_journal(&st.journal, DSN, true)? {
        if vrai_opt(l.get("accepte")) && vrai_opt(l.get("mois")) {
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
pub(crate) fn mois_a_declarer(st: &Etat) -> R<Vec<String>> {
    let dernier = st.aujourdhui.mois_civil().precedent().to_string();
    let depuis = derniere_dsn_passee(st)?;
    let mut premier = match &depuis {
        // Un signalement se date par le jour de son evenement (`AAAA-MM-JJ`) : son mois en est le debut.
        Some(d) => socle::mois::Mois::lire(d.get(..7).unwrap_or(d))?.suivant().to_string(),
        None => dernier.clone(),
    };
    if let (Some(borne), None) = (etat_dsn::premier_mois_du(st.ctx)?, &depuis) {
        premier = premier.max(borne);
    }
    if premier > dernier {
        return Ok(Vec::new());
    }
    let (mut out, mut m) = (Vec::new(), premier);
    while m <= dernier {
        if etat_dsn::due(st.ctx, &m, &Value::Null)? {
            out.push(m.clone());
        }
        m = socle::mois::Mois::lire(&m)?.suivant().to_string();
    }
    Ok(out)
}

/// LES MOIS QUI ONT QUELQUE CHOSE A FAIRE au jour `aujourdhui`, du plus ancien au plus recent : ceux que la saisie
/// touche (`stock::mois_saisis`), et ceux dont la DSN est reclamee sans engagement (`mois_a_declarer` : une DSN sans
/// individu, cahier technique DSN 2026.1, § 2.4). Le mois a ouvrir et l'horloge de la compagnie fictive les parcourent.
pub(crate) fn mois_a_parcourir(ctx: &Contexte, aujourdhui: Jour) -> socle::echec::R<Vec<String>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default();
    let st = Etat { ctx, aujourdhui, journal: envois::journal::lu(&ctx.racine)?.lignes };
    mois.extend(mois_a_declarer(&st).map_err(|r| Echec { classe: r.classe, message: r.message })?);
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

fn rappel_dsn(st: &Etat, mois: &str, modele: &Value) -> R<Echeance> {
    let lib = socle::mois::Mois::lire(mois).map(|m| m.libelle(socle::mois::Graphie::Accentuee)).map_err(Refus::from)?;
    let mut r = Echeance::nouveau(
        DSN,
        format!("dsn:{}", mois),
        format!("Il est temps d'envoyer la DSN {}", socle::texte::de(&lib)),
        format!("Avez-vous bien fini les paies {} ?", socle::texte::de(&lib)),
        socle::mois::Mois::lire(mois)?.suivant().to_string(),
    );
    r.mois = Some(mois.to_string());
    // L'echeance : `echeance_dsn`, le 5 ou le 15 du mois suivant selon l'effectif, reporte au jour ouvre suivant un
    // samedi, un dimanche ou un jour ferie (R. 133-14, I), a midi. Passee, ce n'est plus un rappel, c'est un retard --
    // et on le dit.
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    r.etat = if echeance.passee_au(&st.aujourdhui) { EN_RETARD } else { A_FAIRE };
    r.date = Some(echeance.jour.iso());
    r.quand = echeance_phrase(&echeance);
    r.source = dsn_formee(modele, "source", &[])?;
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
        let m = en_texte(&f["mois"]);
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
        r.mois = Some(m.clone());
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
                r.mois = Some(en_texte(&a["dernier_jour_travaille"]).chars().take(7).collect());
                r.salarie = Some(en_texte(&a["salarie"]));
                out.push(r);
            }
        }
    }

    // Le plus presse d'abord : ce qui est en retard, puis par date butoir (le tri est stable : a butoir egale, l'ordre
    // de lecture reste).
    out.sort_by_key(|r| (r.etat != EN_RETARD, r.date.clone().unwrap_or_default()));
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
    let mut r = Echeance::nouveau(OBLIGATION, cle, titre, question, butoir.as_deref().map(|b| b.chars().take(7).collect()).unwrap_or_default());
    r.etat = if o.get("etat").and_then(Value::as_str) == Some(crate::obligations::EN_RETARD) { EN_RETARD } else { A_FAIRE };
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
            jour.mois_civil().to_string(),
        );
        r.mois = Some(mois);
        r.le(&jour);
        r.quand = format!("avant la première heure de travail, le {}", jour.jj_mm_aaaa());
        r.detail = quand;
        r.etat = if retard { EN_RETARD } else { A_FAIRE };
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
fn acces_proposes(ctx: &Contexte) -> socle::echec::R<(Vec<Value>, Option<Value>)> {
    const OU: &str = "Configuration › Mots de passe";
    use envois::branchement::Branchement;
    let (texte, mots) = match crate::compagnie::bridage::branchement(&ctx.racine) {
        Branchement::Monde => ("", ""),
        Branchement::Simulateur => ("Les envois de cette compagnie sont simulés : rien ne part, ses mots de passe et ses comptes ne se demandent pas.", "simulé"),
        Branchement::Message(_) => ("Dans la démo, rien ne part de cette compagnie : ses mots de passe et ses comptes ne se demandent pas.", "démo"),
    };
    if !mots.is_empty() {
        return Ok((Vec::new(), Some(json!({
            "cle": "acces", "libelle": "Les accès aux organismes", "ou": OU, "onglet": "coffre", "champ": null, "texte": texte,
            "pastille": super::pastille("information", mots),
        }))));
    }
    let etat = super::coffre::coffre_etat_de(&ctx.racine, &ctx.entreprise)?;
    let mut out = Vec::new();
    for s in etat["services"].as_array().into_iter().flatten() {
        let pose = vrai(&s["pose"]) || vrai(&s["environnement"]);
        let compte_dit = s["identifiant"]["manque"].as_array().is_none_or(Vec::is_empty);
        if pose && compte_dit {
            continue;
        }
        out.push(json!({
            "cle": format!("acces.{}", s["service"].as_str().unwrap_or("")),
            "libelle": s["libelle"].as_str().unwrap_or(""),
            "utile": format!("{} depuis le logiciel", s["sert_a"].as_str().unwrap_or("")),
            "sinon": "vous le faites vous-même, hors du logiciel",
            "ou": OU, "onglet": "coffre", "champ": null, "trouver": "",
        }));
    }
    // Le Drive de la sauvegarde : se lie au meme endroit (« Lier le Drive »), sans reseau pour le savoir.
    if super::coffre::compte_autorise(&ctx.racine).is_none() {
        out.push(json!({
            "cle": "acces.google", "libelle": "Google Drive", "utile": "sauvegarder la compagnie dans votre Drive",
            "sinon": "rien ne se sauvegarde hors de ce poste",
            "ou": OU, "onglet": "coffre", "champ": null, "trouver": "",
        }));
    }
    Ok((out, None))
}
