//! Les routes du coffre et des mots de passe : ce qui est pose (`coffre-etat`, `google-etat`), poser ou oublier un
//! secret (`coffre-poser`, avec le compte DPAE pour l'Urssaf, `coffre-oublier`), lier ou delier le compte Google
//! (`google-autoriser`, `google-oublier`), changer ou retirer le mot de
//! passe du logiciel (`mot-de-passe-changer`, `mot-de-passe-retirer`), et ce que l'administrateur est pour la compagnie
//! (`administrateur-etat`, `administrateur-rattacher`, `administrateur-detacher` ; `compagnie/administrateur.rs`).
//!
//! Aucune valeur de secret ne sort : la reponse est l'etat du coffre, des presences et des dates. Ces routes passent
//! par la porte du mot de passe du logiciel s'il est pose (`exiger_mot_de_passe`) ; le coffre lui-meme est
//! `coffre.rs`.
//!
//! UN SECRET NE SE LIT JAMAIS SUR L'ENTREE STANDARD. La page tape les siens dans ses cases et les met dans la charge ; par
//! `moteur route`, ils en sont retires (`SECRETS`, `service::commande`) et le moteur les demande lui-meme a un humain :
//! le mot de passe du logiciel par `demande::autoriser_acte`, un secret a poser par `demande::saisir_deux_fois` (sa
//! fenetre, sinon le terminal sans echo, sinon un refus).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::compagnie::coffre::{Coffre, Echec as EchecCoffre};
use crate::compagnie::administrateur::Poste;
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, OPERATION};
use droit::contexte::Contexte;
use super::{ou_texte, vers_route};
use envois::etats::{get, joindre, objet_vide, ou};
use envois::google::{champ, en_texte, jeton_par_rafraichissement, DELAI, PERMISSIONS};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{vrai, MARQUEUR_COPIE};
use socle::valeur::{nom_type, valeur_en_texte};
use crate::routes::catalogue::{Ecran, Place};

const SERVICE: Champ = Champ { nom: "service", libelle: Affiche::new("Service"), forme: Forme::Texte, sens: "net-entreprises, urssaf ou courriel", avance: false };
const MOT_DE_PASSE: Champ = Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain", avance: false };
const ADMINISTRATEUR: Champ = Champ { nom: "administrateur", libelle: Affiche::new("Mot de passe de l'administrateur"), forme: Forme::Texte, sens: "le mot de passe de l'administrateur, tapé dans la page ; par moteur route, le moteur le demande à un humain", avance: false };
const PREUVE: Refus = Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est modifié" };
const ETAT: &str = "l'état du coffre : ce qui est posé, depuis quand, jamais les valeurs";

/// Les secrets qu'une charge de ce module peut porter, retires de celle qui arrive par `moteur route` (le mot de passe
/// du logiciel, `motdepasse`, l'est de toutes).
pub const SECRETS: [&str; 4] = ["valeur", "nouveau", "confirmation", "administrateur"];

/// Un secret a poser : celui de la charge (la page), sinon tape deux fois par un humain (`demande::saisir_deux_fois`).
fn secret_a_poser(charge: &Value, cle: &str, titre: &str, lignes: &[String]) -> R<String> {
    match charge.get(cle) {
        Some(v) => Ok(if vrai(v) { valeur_en_texte(v) } else { String::new() }),
        None => crate::compagnie::demande::saisir_deux_fois(titre, lignes, "mot de passe (rien ne s'affiche) : ").map_err(refus),
    }
}

/// Le mot de passe actuel du logiciel : celui de la charge (la page), sinon celui que l'humain vient de taper pour cette
/// route (`exiger_mot_de_passe`).
fn actuel(charge: &Value) -> Option<String> {
    chaine(charge, "motdepasse").filter(|m| !m.is_empty()).map(String::from).or_else(crate::compagnie::coffre::mot_de_passe_retenu)
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Coffre);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "coffre-etat",
        geste: "Dire quels mots de passe sont posés, et depuis quand ; jamais leur valeur.",
        charge: &[],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "google-etat",
        geste: "Dire si le compte Google de la sauvegarde est lié.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "lie, compte, et ce que la page en montre",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "coffre-poser",
        geste: "Poser le mot de passe d'un organisme ; pour l'Urssaf, avec le compte des DPAE.",
        charge: &[
            SERVICE,
            Champ { nom: "valeur", libelle: Affiche::new("Mot de passe"), forme: Forme::Texte, sens: "le mot de passe à poser, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "compte", libelle: Affiche::new("Identifiant du compte"), forme: Forme::Texte, sens: "l'identifiant du compte (l'adresse, pour la boîte courriel)", avance: false },
            Champ { nom: "nom", libelle: Affiche::new("Nom"), forme: Forme::Texte, sens: "Urssaf : le nom du compte qui dépose les DPAE ; s'il change, la route ne pose que le compte quand `valeur` manque", avance: false },
            Champ { nom: "prenom", libelle: Affiche::new("Prénom"), forme: Forme::Texte, sens: "Urssaf : son prénom", avance: false },
            MOT_DE_PASSE,
        ],
        exemple: r#"{"service":"urssaf"}"#,
        rend: "l'état du coffre, et ce qu'une tentative de connexion en dit : phrase, couleur",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 400,
        ecran: ECRAN,
    },
    Fiche {
        nom: "coffre-verifier",
        geste: "Essayer une fois le mot de passe posé d'un organisme.",
        charge: &[SERVICE],
        exemple: r#"{"service":"urssaf"}"#,
        rend: "service, ok, message (la première ligne de l'erreur), conseil, pastille",
        refus: &[Refus { classe: "ErreurOperation", corriger: "poser d'abord le mot de passe de ce service" }],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "coffre-oublier",
        geste: "Retirer du coffre le mot de passe d'un organisme.",
        charge: &[SERVICE, MOT_DE_PASSE],
        exemple: r#"{"service":"urssaf"}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "mot-de-passe-changer",
        geste: "Poser ou changer le mot de passe du logiciel.",
        charge: &[
            MOT_DE_PASSE,
            Champ { nom: "nouveau", libelle: Affiche::new("Nouveau mot de passe"), forme: Forme::Texte, sens: "le nouveau mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "confirmation", libelle: Affiche::new("Confirmation"), forme: Forme::Texte, sens: "le même, retapé dans la page", avance: false },
        ],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "mot-de-passe-retirer",
        geste: "Retirer le mot de passe du logiciel.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "google-autoriser",
        geste: "Lier le compte Google de la sauvegarde : Google s'ouvre, la personne accepte.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: "compte : l'adresse du compte Google lié ; la fenêtre de Google s'ouvre dans le navigateur, son adresse est dite sur la sortie d'erreur",
        refus: &[PREUVE, Refus { classe: "ErreurOperation", corriger: "ce que la phrase dit (Google a refusé, aucune réponse en cinq minutes, l'application absente) ; rien n'est enregistré" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "google-oublier",
        geste: "Délier le compte Google de la sauvegarde : le jeton du poste s'efface.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: "l'état du compte Google",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "administrateur-etat",
        geste: "Dire ce que l'administrateur est pour cette compagnie.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), rattachee, autres, peut_rattacher, question (« même administrateur ? »), mot_de_passe_pose, pastille",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        nom: "administrateur-rattacher",
        geste: "Rattacher cette compagnie à l'administrateur : son mot de passe l'ouvre aussi.",
        charge: &[
            ADMINISTRATEUR,
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe actuel"), forme: Forme::Texte, sens: "le mot de passe actuel de cette compagnie, s'il est posé ; par moteur route, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), etat, phrase",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        nom: "administrateur-detacher",
        geste: "Détacher cette compagnie : elle reçoit son propre mot de passe.",
        charge: &[
            ADMINISTRATEUR,
            Champ { nom: "nouveau", libelle: Affiche::new("Nouveau mot de passe"), forme: Forme::Texte, sens: "son propre mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "confirmation", libelle: Affiche::new("Confirmation"), forme: Forme::Texte, sens: "le même, retapé dans la page", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "etat, phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 120,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let mots = |v: &Value| valeur_en_texte(&v["pastille"]["mots"]);
    // L'etat du coffre en une phrase : chaque service et ce que l'ecran en dit.
    let coffre = |fait: &str, v: Value| {
        let services: Vec<String> = v["services"].as_array().into_iter().flatten().map(|s| format!("{} : {}", valeur_en_texte(&s["libelle"]), mots(s))).collect();
        let p = format!("{}{}.", fait, services.join(" ; "));
        super::phrase_en_tete(v, p)
    };
    match nom {
        "coffre-etat" => coffre_etat(ctx).map(|mut v| {
            v["ecran"] = ecran_du_coffre(ctx, &mut v);
            coffre("", v)
        }),
        "google-etat" => google_etat(ctx).map(|v| {
            let p = format!("Google Drive : {}.", mots(&v));
            super::phrase_en_tete(v, p)
        }),
        "coffre-poser" => coffre_poser(ctx, charge),
        "coffre-verifier" => crate::compagnie::coffre_verifier::route(ctx, charge).map(|v| {
            let service = valeur_en_texte(&v["service"]);
            // Un compte incomplet se dit avec ce qui lui manque et ou le donner (`identifiant_du_compte`).
            let manque = etat_du_coffre(ctx).ok().and_then(|e| {
                e["services"].as_array().into_iter().flatten().filter(|s| valeur_en_texte(&s["service"]) == service).map(|s| manque_du_compte(&s["identifiant"])).find(|m| !m.is_empty())
            });
            let conseil = v["conseil"].as_str().filter(|c| !c.is_empty()).map(|c| format!(" {}", c)).unwrap_or_default();
            let p = match manque {
                Some(m) if v["ok"] != true => format!("{} : {}, {}.", libelle_du_service(&service), mots(&v), m),
                _ => format!("{} : {}. {}{}", libelle_du_service(&service), mots(&v), valeur_en_texte(&v["message"]), conseil).trim().to_string(),
            };
            super::phrase_en_tete(v, p)
        }),
        "coffre-oublier" => coffre_oublier(ctx, charge).map(|v| coffre("Mot de passe retiré du coffre. ", v)),
        "mot-de-passe-changer" => mot_de_passe_changer(ctx, charge).map(|v| coffre("Mot de passe du logiciel posé. ", v)),
        "mot-de-passe-retirer" => mot_de_passe_retirer(ctx, charge).map(|v| coffre("Mot de passe du logiciel retiré. ", v)),
        "google-oublier" => google_oublier(ctx, charge).map(|v| super::phrase_en_tete(v, "Compte Google délié : la sauvegarde ne va plus au Drive. L'accès accordé côté Google se retire sur myaccount.google.com.")),
        "google-autoriser" => google_autoriser(ctx, charge).map(|v| {
            let p = format!("Drive lié : {}. La prochaine ouverture sauvegarde.", valeur_en_texte(&v["compte"]));
            super::phrase_en_tete(v, p)
        }),
        "administrateur-etat" => {
            let v = administrateur_etat(&ctx.racine);
            let p = format!("Mot de passe de la compagnie : {}.", mots(&v));
            Ok(super::phrase_en_tete(v, p))
        }
        "administrateur-rattacher" | "administrateur-detacher" => administrateur(&ctx.racine, nom, charge, &Poste::de_ce_poste()),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'administrateur

/// Ce que l'administrateur de l'installation est pour la compagnie `racine` (`administrateur::etat`).
pub fn administrateur_etat(racine: &Path) -> Value {
    let inst = crate::compagnie::administrateur::installation_de(racine);
    crate::compagnie::administrateur::etat(&inst, Some(racine), &Poste::de_ce_poste())
}

/// `administrateur-rattacher` et `administrateur-detacher`, sur le coffre de `poste` : tout se decide dans
/// `administrateur.rs`. Le mot de passe de l'administrateur vient de la charge (la page), sinon il est tape par un humain.
pub fn administrateur(racine: &Path, nom: &str, charge: &Value, poste: &Poste) -> R<Value> {
    use crate::compagnie::administrateur as adm;
    let inst = adm::installation_de(racine);
    let admin = match chaine(charge, "administrateur") {
        Some(m) => m.to_string(),
        None => crate::compagnie::demande::saisir_secret(
            "PuppetPaie : le mot de passe de l'administrateur",
            &["Celui qui ouvre les compagnies de l'administrateur.".to_string(), "La frappe ne s'affiche pas.".to_string()],
            "mot de passe (rien ne s'affiche) : ",
        )
        .map_err(refus)?,
    };
    let en_liste = |n: &[String]| match n.len() {
        0 => String::new(),
        1 => n[0].clone(),
        k => format!("{} et {}", n[..k - 1].join(", "), n[k - 1]),
    };
    if nom == "administrateur-rattacher" {
        // Une compagnie qui a deja son mot de passe le donne aussi : c'est lui que celui de l'administrateur remplace.
        let actuel = if charge.get("motdepasse").is_some() {
            chaine(charge, "motdepasse").map(String::from)
        } else if poste.coffre(racine).ouverture_exigee().map_err(en_operation)? {
            exiger_mot_de_passe(racine, optionnel(charge, "motdepasse"), "rattacher cette compagnie à l'administrateur")?;
            actuel(charge)
        } else {
            None
        };
        let noms = adm::rattacher(&inst, racine, &admin, actuel.as_deref(), poste).map_err(refus)?;
        let phrase = format!("Compagnie rattachée à l'administrateur : {} s'ouvrent avec le même mot de passe.", en_liste(&noms));
        return Ok(json!({"compagnies": noms, "etat": adm::etat(&inst, Some(racine), poste), "phrase": phrase}));
    }
    let lignes = ["Le NOUVEAU mot de passe de cette compagnie, à elle seule.".to_string(), "La frappe ne s'affiche pas.".to_string()];
    let nouveau = secret_a_poser(charge, "nouveau", "PuppetPaie : son propre mot de passe", &lignes)?;
    let confirmation = match charge.get("confirmation") {
        Some(_) => chaine(charge, "confirmation").unwrap_or("").to_string(),
        None => nouveau.clone(),
    };
    adm::detacher(&inst, racine, &admin, &nouveau, &confirmation, poste).map_err(refus)?;
    Ok(json!({"etat": adm::etat(&inst, Some(racine), poste), "phrase": "Compagnie détachée : elle s'ouvre désormais avec son propre mot de passe."}))
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// `exiger_mot_de_passe` : le mot de passe du logiciel, s'il est pose, pour un
/// acte qui ENGAGE sans rien envoyer. Decide ICI, jamais par la page.
pub(crate) fn exiger_mot_de_passe(racine: &Path, motdepasse: &Value, quoi: &str) -> R<()> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(racine, "");
    let vers = crate::routes::vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Ok(());
    }
    if !vrai(motdepasse) {
        // Rien n'a ete tape pour cette charge : le moteur demande lui-meme a un humain (fenetre, terminal), ou il refuse.
        // Un « oui » ne laisse que l'ouverture du coffre pour la suite de la commande ; la seance ne vaut pas ici.
        let acte = crate::compagnie::demande::Acte::nouveau(quoi, "", "");
        return match crate::compagnie::demande::autoriser_acte(racine, &acte) {
            crate::compagnie::demande::Verdict::Accepte => Ok(()),
            crate::compagnie::demande::Verdict::Refuse(phrase) => Err(erreur(
                "PreuveInsuffisante",
                format!("Un mot de passe du logiciel est pose : il faut le taper pour {}. {}", quoi, phrase.replace("Rien n'est parti", "Rien n'a ete modifie")),
            )),
        };
    }
    let mdp = valeur_en_texte(motdepasse);
    if !coffre.verifier_ouverture(&mdp).map_err(vers)? {
        return Err(erreur("PreuveInsuffisante", "Mot de passe du logiciel refuse. Rien n'a ete modifie."));
    }
    Ok(())
}

/// `charge.get(cle)`.
fn optionnel<'a>(charge: &'a Value, cle: &str) -> &'a Value {
    charge.get(cle).unwrap_or(&Value::Null)
}

/// Le refus du coffre : `ErreurCoffre` se rend `ErreurOperation` (la page le montre tel quel), tout le reste
/// (`OSError`...) garde sa classe.
fn en_operation(x: EchecCoffre) -> Echec {
    Echec { classe: Some(if x.classe == "ErreurCoffre" { "ErreurOperation" } else { x.classe }), message: x.message }
}

/// Le refus du coffre tel quel (`coffre.oublier_ouverture` ne l'attrapait pas).
fn en_coffre(x: EchecCoffre) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// L'heure de Paris, a la minute.
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

fn coffre(ctx: &Contexte) -> Coffre {
    Coffre::du_poste(&ctx.racine, &maintenant())
}

/// `coffre_etat(ctx)` : ce qui est pose, depuis quand. Jamais les valeurs.
fn etat_du_coffre(ctx: &Contexte) -> R<Value> {
    coffre_etat_de(&ctx.racine, &ctx.entreprise)
}

/// Une chaine de la charge, quand c'en est une (`texte` du moteur : le reste n'est rien).
fn chaine<'a>(charge: &'a Value, cle: &str) -> Option<&'a str> {
    charge.get(cle).and_then(Value::as_str)
}

// ------------------------------------------------------------------ le coffre

/// Le libelle d'un service du coffre, tel que l'ecran le dit (« Urssaf (DPAE) ») ; son nom s'il n'est pas connu.
fn libelle_du_service(nom: &str) -> String {
    crate::compagnie::coffre::SERVICES.iter().find(|s| s.nom == nom).map_or_else(|| nom.to_string(), |s| s.libelle.to_string())
}

/// Ce qui manque au compte d'un service (`identifiant_du_compte`), en mots, et ou le donner ; vide s'il est complet.
fn manque_du_compte(identifiant: &Value) -> String {
    let manque: Vec<&str> = identifiant["manque"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|c| match c {
            "siret" => "le SIRET",
            "nom" => "le nom",
            "prenom" => "le prénom",
            autre => autre,
        })
        .collect();
    if manque.is_empty() {
        return String::new();
    }
    let ou = match identifiant["ou"].as_str() {
        Some(ou) => format!(" ({})", ou),
        None if identifiant["ici"] == true => " (à donner avec le mot de passe)".to_string(),
        None => String::new(),
    };
    format!("il manque au compte {}{}", manque.join(", "), ou)
}

/// Range le mot de passe d'un organisme, ou de la boite courriel avec son adresse (`compte`) ; pour l'Urssaf, avec le
/// nom et le prenom du compte qui depose les DPAE (`nom`, `prenom`), ecrits dans la fiche entreprise s'ils changent. Un
/// seul geste, une seule preuve : tout se verifie avant d'ecrire. Rend l'etat du coffre, sans valeur. Rien a poser se
/// refuse avant la porte du mot de passe du logiciel : il n'y a rien a autoriser.
fn coffre_poser(ctx: &Contexte, charge: &Value) -> R<Value> {
    let service = crate::compagnie::coffre::service_connu(optionnel(charge, "service")).map_err(en_operation)?;
    let compte = if charge.get("nom").is_some() || charge.get("prenom").is_some() { compte_dpae_change(ctx, charge)? } else { None };
    // Sans `valeur` (`moteur route`) : le compte seul s'il change, sinon le secret, tape par un humain apres la porte.
    let tape = charge.get("valeur").is_none() && compte.is_none();
    if !tape && socle::texte::strip(chaine(charge, "valeur").unwrap_or("")).is_empty() && compte.is_none() {
        return Err(refus(format!("Tapez le mot de passe {} à poser : rien n'a été posé.", service.nom)));
    }
    let pose_un_secret = tape || !socle::texte::strip(chaine(charge, "valeur").unwrap_or("")).is_empty();
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), if pose_un_secret { "poser un mot de passe" } else { "changer le compte qui depose les DPAE" })?;
    let lignes = [format!("Service : {}", service.libelle), "La frappe ne s'affiche pas : elle est scellée dans le coffre, nulle part ailleurs.".to_string()];
    let valeur = if pose_un_secret { secret_a_poser(charge, "valeur", "PuppetPaie : poser un mot de passe", &lignes)? } else { String::new() };
    let a_poser = !socle::texte::strip(&valeur).is_empty();
    let mut entreprise = ctx.entreprise.clone();
    if let Some(propres) = compte {
        entreprise = ecrire_compte_dpae(ctx, propres)?;
    }
    if a_poser {
        coffre(ctx).poser(service.nom, &valeur, chaine(charge, "compte")).map_err(en_operation)?;
    }
    let mut etat = coffre_etat_de(&ctx.racine, &entreprise)?;
    // PUIS UNE TENTATIVE, UNE SEULE (`coffre_verifier`) : un mot de passe faux se voit maintenant, pas au depot. Pas si
    // l'identifiant est incomplet : l'echec serait certain, et chaque echec rapproche du blocage du compte.
    let quoi = format!("{} {} enregistré", if a_poser { "Mot de passe" } else { "Compte" }, libelle_du_service(service.nom));
    let manque = etat["services"].as_array().into_iter().flatten().filter(|s| s["service"] == service.nom).map(|s| manque_du_compte(&s["identifiant"])).find(|m| !m.is_empty());
    let (phrase, couleur) = if let Some(manque) = manque {
        (format!("{} ; pas essayé : {}.", quoi, manque), "gris")
    } else {
        match crate::compagnie::coffre_verifier::route(ctx, &json!({"service": service.nom})) {
            Ok(v) if v["ok"] == true => (format!("{}, et vérifié : la connexion passe.", quoi), "vert"),
            Ok(v) => {
                let conseil = v["conseil"].as_str().filter(|c| !c.is_empty()).map(|c| format!(" — {}", c)).unwrap_or_default();
                (format!("{} ; connexion refusée : {}{}", quoi, v["message"].as_str().unwrap_or(""), conseil), "rouge")
            }
            Err(e) => (format!("{} ; vérification impossible : {}", quoi, e.message), "rouge"),
        }
    };
    etat["phrase"] = json!(phrase);
    etat["couleur"] = json!(couleur);
    Ok(etat)
}

/// `coffre-oublier` : retire un secret du coffre.
fn coffre_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "retirer un mot de passe")?;
    let service = crate::compagnie::coffre::service_connu(optionnel(charge, "service")).map_err(en_operation)?;
    coffre(ctx).oublier(service.nom).map_err(en_operation)?;
    etat_du_coffre(ctx)
}

/// Le nom et le prenom du compte Urssaf qui depose les DPAE, tels que la charge les dit, s'ils changent ceux de la fiche ;
/// `None` s'ils sont ceux de la fiche (vides compris). SANS ACCENTS, DANS LA CASSE TAPEE : la DUE MtoM refuse les accents.
fn compte_dpae_change(ctx: &Contexte, charge: &Value) -> R<Option<Vec<(&'static str, String)>>> {
    let actuel = ctx.entreprise.get("api_dpae").cloned().unwrap_or(Value::Null);
    let propres: Vec<(&'static str, String)> = ["nom", "prenom"]
        .into_iter()
        .map(|cle| {
            let v = optionnel(charge, cle);
            let brut = if vrai(v) { valeur_en_texte(v) } else { String::new() };
            let sans = declarations::champs::sans_accents(&brut);
            (cle, sans.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" "))
        })
        .collect();
    if propres.iter().all(|(k, v)| actuel.get(*k).and_then(Value::as_str).unwrap_or("") == v) {
        return Ok(None);
    }
    if let Some((cle, _)) = propres.iter().find(|(_, v)| v.is_empty()) {
        return Err(refus(format!(
            "Le {} du compte Urssaf manque. Il est celui du compte ouvert sur urssaf.fr pour la DPAE -- pas forcement celui de la personne qui signe. Rien n'a ete modifie.",
            cle
        )));
    }
    Ok(Some(propres))
}

/// Ecrit le compte DPAE dans la fiche entreprise, par l'ecrivain de la fiche (`fiches::poser_entreprise`), date du jour ;
/// les autres cles d'`api_dpae` sont gardees. Rend la fiche telle qu'elle vaut aujourd'hui.
fn ecrire_compte_dpae(ctx: &Contexte, propres: Vec<(&'static str, String)>) -> R<Value> {
    let mut cfg = match ctx.entreprise.get("api_dpae").filter(|v| vrai(v)) {
        Some(Value::Object(o)) => o.clone(),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
        None => serde_json::Map::new(),
    };
    for (cle, v) in propres {
        cfg.insert(cle.to_string(), Value::String(v));
    }
    crate::routes::fiches::poser_entreprise(ctx, "api_dpae", Value::Object(cfg), Some(ctx.aujourdhui))?;
    let contenu = crate::routes::lire_json_ou_vide(&ctx.racine.join("config").join("entreprise.json"))?;
    droit::contexte::fiche_au(&contenu, &ctx.aujourdhui, "config/entreprise.json").map_err(Echec::from)
}

// ------------------------------------------------------------------ le mot de passe du logiciel

/// Pose ou change le mot de passe du logiciel. L'actuel
/// est exige s'il existe, et il ouvre aussi le coffre quand la cle de ce poste ne l'ouvre plus.
fn mot_de_passe_changer(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "changer le mot de passe du logiciel")?;
    // La page tape le nouveau et sa confirmation dans ses cases ; par `moteur route`, l'humain le tape deux fois.
    let (nouveau, confirmation) = if charge.get("nouveau").is_some() {
        // Une saisie qui n'est pas un texte n'est pas un mot de passe : le coffre la lit comme vide.
        let texte = |cle: &str| match optionnel(charge, cle) {
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        (texte("nouveau"), texte("confirmation"))
    } else {
        let lignes = ["Le NOUVEAU mot de passe du logiciel (pas celui d'un organisme).".to_string(), "La frappe ne s'affiche pas.".to_string()];
        let m = secret_a_poser(charge, "nouveau", "PuppetPaie : mot de passe du logiciel", &lignes)?;
        (m.clone(), m)
    };
    if nouveau != confirmation {
        return Err(refus("Les deux saisies du nouveau mot de passe different. Rien n'a ete modifie."));
    }
    let actuel = actuel(charge);
    // UNE COMPAGNIE DE L'ADMINISTRATEUR : son mot de passe est celui de toutes ses compagnies, et il se change
    // pour toutes a la fois (`administrateur::changer`) ; l'actuel les ouvre chacune, tout se prepare puis tout se remplace.
    let inst = crate::compagnie::administrateur::installation_de(&ctx.racine);
    if crate::compagnie::administrateur::est_rattachee(&inst, &ctx.racine) {
        let Some(actuel) = actuel else {
            return Err(refus("Tapez le mot de passe actuel de l'administrateur : le nouveau le remplace dans toutes ses compagnies. Rien n'a ete modifie."));
        };
        crate::compagnie::administrateur::changer(&inst, &actuel, &nouveau, &confirmation, &crate::compagnie::administrateur::Poste::de_ce_poste()).map_err(refus)?;
        return etat_du_coffre(ctx);
    }
    coffre(ctx).poser_ouverture(&nouveau, actuel.as_deref()).map_err(en_operation)?;
    etat_du_coffre(ctx)
}

/// Plus de mot de passe du logiciel. L'actuel est exige.
fn mot_de_passe_retirer(ctx: &Contexte, charge: &Value) -> R<Value> {
    if crate::compagnie::administrateur::est_rattachee(&crate::compagnie::administrateur::installation_de(&ctx.racine), &ctx.racine) {
        return Err(refus("Cette compagnie est geree par l'administrateur : son mot de passe ouvre aussi les autres. La detacher d'abord (elle recoit alors son propre mot de passe), puis le retirer. Rien n'a ete modifie."));
    }
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "retirer le mot de passe du logiciel")?;
    coffre(ctx).oublier_ouverture(actuel(charge).as_deref()).map_err(en_coffre)?;
    etat_du_coffre(ctx)
}

// ------------------------------------------------------------------ Google

/// Efface le jeton local, sans reseau. L'acces reste accorde cote
/// Google, et se retire sur myaccount.google.com.
fn google_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "oublier le compte Google")?;
    let p = ctx.racine.join("config").join("google-jeton.bin");
    let efface = p.exists();
    if efface {
        std::fs::remove_file(&p).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
    }
    Ok(json!({ "efface": efface }))
}

// ------------------------------------------------------------------ le compte Google

/// L'application Google du logiciel, livree avec lui
/// dans `google-application.json` (a cote du code), `client_id` et
/// `client_secret`. Le secret ne s'ecrit pas dans le source Rust.
const FICHIER_APPLICATION: &str = "google-application.json";

/// `google.FICHIER_IDENTIFIANTS`, `google.FICHIER_JETON`.
const FICHIER_IDENTIFIANTS: &str = "google.json";
pub(crate) const FICHIER_JETON: &str = "google-jeton.bin";

/// La description scellee avec le jeton (`google._dpapi`).
pub(crate) const DESCRIPTION: &str = "moteur de paie - jeton Google";

/// `compagnies.MARQUEUR`.

fn config(racine: &Path) -> PathBuf {
    racine.join("config")
}

/// `ErreurGoogle` : la phrase est ecrite pour etre lue.
#[track_caller]
fn erreur_google(message: impl Into<String>) -> Echec {
    erreur("ErreurGoogle", message)
}

/// (client_id, client_secret, origine).
pub(crate) fn identifiants(racine: &Path) -> R<(Value, Value, String)> {
    let p = config(racine).join(FICHIER_IDENTIFIANTS);
    if p.exists() {
        // Un `google.json` illisible est un refus, pas un plantage (`JSONDecodeError`, `UnicodeDecodeError`, `AttributeError`).
        let illisible = |raison: String| erreur_google(format!("{} est illisible : {}.\nLe corriger, ou le supprimer pour revenir a l'application du logiciel.", p.display(), raison));
        let octets = std::fs::read(&p).map_err(|e| illisible(e.to_string()))?;
        let texte = String::from_utf8(octets).map_err(|e| illisible(e.to_string()))?;
        let donnees: Value = serde_json::from_str(&texte).map_err(|e| illisible(e.to_string()))?;
        let Value::Object(d) = &donnees else { return Err(illisible("ce n'est pas un objet JSON".into())) };
        let manque: Vec<&str> = ["client_id", "client_secret"]
            .into_iter()
            .filter(|c| !d.get(*c).map(vrai).unwrap_or(false))
            .collect();
        if !manque.is_empty() {
            return Err(erreur_google(format!(
                "{} incomplet, il manque : {}.\nLe supprimer suffit a revenir a l'application du logiciel.",
                p.display(),
                manque.join(", ")
            )));
        }
        return Ok((d["client_id"].clone(), d["client_secret"].clone(), p.display().to_string()));
    }
    let livre = socle::fichiers::dossier_du_code().join(FICHIER_APPLICATION);
    let application = std::fs::read(&livre)
        .ok()
        .and_then(|o| serde_json::from_slice::<Value>(&o).ok())
        .filter(|a| a.get("client_id").map(vrai).unwrap_or(false));
    let Some(a) = application else {
        return Err(erreur_google(format!(
            "L'application Google du logiciel n'est pas renseignee.\n\n\
             Elle est livree avec chaque version publiee ({}, posee par\n\
             le flux « Publier » depuis le secret PUPPETPAIE_GOOGLE_APPLICATION) ;\n\
             une copie de travail du depot ne l'a pas.\n\n\
             Sinon, on peut utiliser sa propre application en creant\n\
             {} :\n\n\
             \x20 {{\n\
             \x20   \"client_id\": \"....apps.googleusercontent.com\",\n\
             \x20   \"client_secret\": \"....\"\n\
             \x20 }}\n",
            livre.display(),
            p.display()
        )));
    };
    Ok((a["client_id"].clone(), a.get("client_secret").cloned().unwrap_or(Value::Null), "le logiciel".into()))
}

/// Y a-t-il un jeton utilisable, sans rien demander au reseau. Un jeton illisible, un coffre qui
/// refuse, un jeton sans `rafraichissement` : non. Ne leve jamais (l'ouverture l'appelle).
pub(crate) fn autorise(racine: &Path) -> bool {
    match lire_jeton(racine) {
        Ok(j) => champ(&j, "rafraichissement").map(|v| vrai(&v)).unwrap_or(false),
        Err(()) => false,
    }
}

/// Le contenu du jeton, `Null` s'il n'y en a pas.
/// `Err` : ce que `autorise` et ses voisins rattrapent (`ErreurGoogle`,
/// `ValueError`) -- le coffre qui refuse, un JSON illisible.
fn lire_jeton(racine: &Path) -> Result<Value, ()> {
    lire_jeton_dit(racine).map_err(|_| ())
}

/// `google._lire_jeton`, avec la phrase du refus : le coffre qui refuse dit
/// quoi faire (`google._dpapi`), un JSON illisible dit seulement qu'il l'est.
/// Le client Drive (`drive.rs`) la rend telle quelle.
pub(crate) fn lire_jeton_dit(racine: &Path) -> Result<Value, String> {
    let p = config(racine).join(FICHIER_JETON);
    if !p.exists() {
        return Ok(Value::Null);
    }
    // Une lecture impossible (droits) levait une OSError que rien ne
    // rattrapait ; elle se range ici avec les refus du coffre : l'etat se lit.
    let brut = std::fs::read(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    let clair = crate::compagnie::coffre::Coffre::du_poste(racine, "").deproteger(&brut, DESCRIPTION).map_err(|e| {
        format!("{} Pour le jeton Google, le remede est : relier de nouveau le Drive (Configuration > Mots de passe).", e.message)
    })?;
    let texte = String::from_utf8(clair).map_err(|e| e.to_string())?;
    serde_json::from_str(&texte).map_err(|e| e.to_string())
}

/// L'adresse du compte, sans reseau ; `None` si pas autorise.
pub fn compte_autorise(racine: &Path) -> Option<String> {
    let j = lire_jeton_dit(racine).ok()?;
    match champ(&j, "compte").ok()? {
        Value::Null => None,
        Value::String(s) => Some(s),
        autre => Some(en_texte(&autre)),
    }
}

/// Les adresses de Google ; dans l'atelier, un faux Google local (`atelier::redirection`).
pub fn adresses_du_poste() -> envois::drive::Adresses {
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::redirection::base(crate::atelier::redirection::VARIABLE_GOOGLE) {
        return envois::drive::Adresses::de_la_base(&b);
    }
    envois::drive::Adresses::default()
}

/// LE CLIENT DU DRIVE DE LA COMPAGNIE : refuse une copie de developpement (qui ecrirait dans le Drive de la VRAIE
/// compagnie), lit l'application (`google.json` ou celle du logiciel), le jeton scelle par la cle du poste et l'adresse
/// du compte, et les passe au client (`drive::Client::nouveau`). Aucun appel reseau.
pub fn client_drive(racine: &Path) -> Result<envois::drive::Client, envois::drive::ErreurGoogle> {
    if let Some(refus) = refus_de_copie(racine) {
        return Err(refus.into());
    }
    let (id, secret, _origine) = identifiants(racine)?;
    let autorisation = envois::drive::Autorisation { identifiants: (id, secret), jeton: lire_jeton_dit(racine), compte: compte_autorise(racine) };
    Ok(envois::drive::Client::nouveau(autorisation, adresses_du_poste()))
}

/// La variable d'un faux Google des essais : sa base locale (`http://127.0.0.1:<port>`), lue en essai seulement
/// (`garde_reseau::en_essai`, que Cargo pose) ; les jetons et le compte y sont demandes (`google_autorisation::Adresses`).
pub const VARIABLE_GOOGLE_ESSAIS: &str = "PUPPETPAIE_ESSAIS_GOOGLE";

/// Les adresses de l'autorisation : celles de Google ; dans l'atelier, son faux Google ; en essai, celui que l'essai
/// nomme (`VARIABLE_GOOGLE_ESSAIS`, une adresse locale seulement).
fn adresses_de_l_autorisation() -> envois::google_autorisation::Adresses {
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::atelier::redirection::base(crate::atelier::redirection::VARIABLE_GOOGLE) {
        return envois::google_autorisation::Adresses::de_la_base(&b);
    }
    let essai = std::env::var(VARIABLE_GOOGLE_ESSAIS).ok().filter(|b| socle::garde_reseau::en_essai() && envois::redirection::est_locale(b));
    match essai {
        Some(b) => envois::google_autorisation::Adresses::de_la_base(b.trim_end_matches('/')),
        None => envois::google_autorisation::Adresses::default(),
    }
}

/// LIER LE COMPTE GOOGLE (`google-autoriser`) : le mot de passe du logiciel s'il est pose, puis l'autorisation
/// (`google_autorisation::autoriser`) : le moteur ouvre la fenetre de consentement de Google dans le navigateur du poste
/// et dit son adresse sur la sortie d'erreur (un terminal sans navigateur la copie), la personne accepte, Google renvoie
/// le code au moteur, qui scelle le jeton. Rend l'adresse du compte lie. La page et `moteur route` l'appellent pareil.
fn google_autoriser(ctx: &Contexte, charge: &Value) -> R<Value> {
    exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "lier le compte Google")?;
    let (id, secret, _) = identifiants(&ctx.racine).map_err(|e| Echec { classe: Some("ErreurOperation"), message: e.message })?;
    let dire_url = |u: &str| {
        eprintln!("La fenetre de Google, a ouvrir dans le navigateur si elle ne s'ouvre pas seule : {}", u);
        let _ = crate::interface::poste::ouvrir_adresse(u);
    };
    let jeton = envois::google_autorisation::autoriser((id, secret), &adresses_de_l_autorisation(), envois::google_autorisation::DELAI_AUTORISATION, &dire_url, &|m| eprintln!("{}", m))?;
    Ok(json!({"compte": sceller_le_jeton(&ctx.racine, &jeton)?}))
}

/// Le jeton que l'autorisation a obtenu (`google_autorisation::autoriser`) et l'adresse du compte, dans un seul fichier
/// scelle par la cle du poste. Rend l'adresse du compte lie.
fn sceller_le_jeton(racine: &Path, jeton: &envois::google_autorisation::JetonGoogle) -> R<String> {
    let brut = socle::json_ascii::dumps(
        &serde_json::json!({"rafraichissement": jeton.rafraichissement, "compte": jeton.compte, "permissions": jeton.permissions}),
        None,
        false,
        false,
    );
    let scelle = crate::compagnie::coffre::Coffre::du_poste(racine, "")
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| erreur_google(format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", e.message)))?;
    let dossier = config(racine);
    std::fs::create_dir_all(&dossier).map_err(|e| erreur_google(socle::echec::du_disque(&dossier, &e)))?;
    let chemin = dossier.join(FICHIER_JETON);
    std::fs::write(&chemin, scelle).map_err(|e| erreur_google(socle::echec::du_disque(&chemin, &e)))?;
    Ok(jeton.compte.clone())
}

/// `p.rsplit("/", 1)[-1]` sur chaque permission.
fn courtes<'a>(ps: impl IntoIterator<Item = &'a Value>) -> R<Vec<Value>> {
    ps.into_iter()
        .map(|p| match p {
            Value::String(s) => Ok(json!(s.rsplit('/').next().unwrap_or(s))),
            autre => Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
        })
        .collect()
}

/// La liste de ce que le jeton enregistre comme permissions.
fn liste(v: &Value) -> R<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| json!(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| json!(k)).collect()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// Une copie de developpement n'ecrit jamais
/// dans le Drive de la vraie compagnie. Le refus a sa classe, `CopieDeDeveloppement`.
pub(crate) fn refus_de_copie(racine: &Path) -> Option<Echec> {
    Path::new(racine).join(MARQUEUR_COPIE).is_file().then(|| {
        erreur(
            "CopieDeDeveloppement",
            "Copie de developpement : la sauvegarde ou l'archivage Drive ne part jamais d'ici. Les essais se font en \
             test ; pour le faire en vrai, ouvrir la vraie compagnie.",
        )
    })
}

/// `Client(ctx).jeton_acces()` : `Ok(None)` si Google rend un jeton d'acces,
/// `Ok(Some(probleme))` pour une `ErreurGoogle`, `Err` pour tout autre
/// defaut.
fn rafraichir(ctx: &Contexte, client_id: &Value, client_secret: &Value) -> R<Option<String>> {
    // `compagnies.refuser_si_copie`, a la construction du client.
    if let Some(refus) = refus_de_copie(&ctx.racine) {
        return Err(refus);
    }
    let j = match lire_jeton(&ctx.racine) {
        Ok(j) => j,
        Err(()) => return Ok(Some("le jeton Google ne s'ouvre plus".into())),
    };
    let rafraichissement = champ(&j, "rafraichissement")?;
    if !vrai(&rafraichissement) {
        return Ok(Some(
            "Compte Google pas encore lie : Configuration > Mots de passe > Lier le Drive, ou la route google-autoriser."
                .into(),
        ));
    }
    Ok(jeton_par_rafraichissement(&adresses_du_poste().jetons, client_id, client_secret, &rafraichissement, DELAI)?.err())
}

/// `google-etat` : ce que la page dit du compte Google (lie, a quelle date), jamais le jeton.
fn google_etat(ctx: &Contexte) -> R<Value> {
    // L'application ne sert qu'a rafraichir un jeton : sans Drive lie, son absence (une copie de travail du depot) n'est
    // pas un refus, et l'etat dit seulement « pas lie ». Un `config/google.json` pose mais illisible ou
    // incomplet en reste un : c'est un fichier de l'utilisateur, a corriger.
    let application = match identifiants(&ctx.racine) {
        Err(e) if config(&ctx.racine).join(FICHIER_IDENTIFIANTS).exists() => return Err(e),
        a => a,
    };
    let origine = application.as_ref().map(|a| json!(a.2)).unwrap_or(Value::Null);
    let permissions = courtes(&PERMISSIONS.map(|p| json!(p)))?;
    let etat = |compte: Value, verifie: bool, probleme: Option<String>, accordees: Vec<Value>| -> Value {
        let mut o = Map::new();
        o.insert("application".into(), origine.clone());
        o.insert("permissions".into(), json!(permissions));
        o.insert("accordees".into(), json!(accordees));
        o.insert("autorise".into(), json!(vrai(&compte)));
        o.insert("compte".into(), compte);
        o.insert("verifie".into(), json!(verifie));
        o.insert("probleme".into(), json!(probleme));
        // Sa pastille : aucun Drive lie, a faire (la sauvegarde attend) ; lie et l'autorisation tient, fait ; lie mais
        // l'autorisation ne repond pas, a faire.
        o.insert("pastille".into(), if !vrai(&o["compte"]) {
            super::pastille("a_faire", "aucun Drive lié")
        } else if verifie {
            super::pastille("fait", "lié")
        } else {
            super::pastille("a_faire", "lié, mais")
        });
        Value::Object(o)
    };
    // `autorise` : un jeton illisible ou sans rafraichissement, c'est « pas autorise ».
    let jeton = lire_jeton(&ctx.racine);
    let autorise = match &jeton {
        Ok(j) => vrai(&champ(j, "rafraichissement")?),
        Err(()) => false,
    };
    if !autorise {
        return Ok(etat(Value::Null, false, None, Vec::new()));
    }
    let j = jeton.unwrap_or(Value::Null);
    let compte = champ(&j, "compte")?;
    let accordees = match champ(&j, "permissions")? {
        p if vrai(&p) => courtes(&liste(&p)?)?,
        _ => permissions.clone(),
    };
    let (client_id, client_secret, _) = application?;
    match rafraichir(ctx, &client_id, &client_secret)? {
        None => Ok(etat(compte, true, None, accordees)),
        Some(probleme) => Ok(etat(compte, false, Some(probleme), accordees)),
    }
}

/// `identifiant_du_compte` : le compte que ce mot de passe ouvre, en clair ; ce
/// qui en manque ; et s'il se saisit sur cette ligne (`ici`).
fn identifiant_du_compte(ent: &Value, service: &str) -> R<Value> {
    match service {
        "net-entreprises" => {
            let d = envois::transport::declarant(ent).map_err(|v| Echec {
                classe: match v.get("classe").and_then(Value::as_str) {
                    Some("AttributeError") => Some("AttributeError"),
                    _ => None,
                },
                message: v.get("erreur").and_then(Value::as_str).unwrap_or("").to_string(),
            })?;
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(&d, c)?) {
                    manque.push(json!(c));
                }
            }
            if !vrai(get(&d, "siret")?) {
                manque.insert(0, json!("siret"));
            }
            let noms: Vec<Value> = [get(&d, "nom")?, get(&d, "prenom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(get(&d, "siret")?)) } else { String::new() };
            Ok(json!({"texte": phrase, "manque": manque, "ici": false, "ou": "fiche Entreprise › Qui les organismes contactent"}))
        }
        "urssaf" => {
            let cfg = ou(get(ent, "api_dpae")?, objet_vide());
            let siret = ou(get(cfg, "siret")?, get(ent, "siret")?);
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(cfg, c)?) {
                    manque.push(json!(c));
                }
            }
            let noms: Vec<Value> = [get(cfg, "prenom")?, get(cfg, "nom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            // Ce qui manque, dit : le compte de la DPAE ne part pas sans lui.
            let noms_manquants: Vec<&str> = manque.iter().map(|m| match m.as_str().unwrap_or("") { "nom" => "le nom", "prenom" => "le prénom", "siret" => "le SIRET", autre => autre }).collect();
            let manque_dit = if manque.is_empty() { String::new() } else { format!("le compte Urssaf n'est pas dit ({}) : la DPAE ne partira pas", noms_manquants.join(", ")) };
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(siret)) } else { String::new() };
            Ok(json!({
                "texte": phrase, "manque": manque, "ici": true, "manque_dit": manque_dit,
                "nom": ou_texte(get(cfg, "nom")?), "prenom": ou_texte(get(cfg, "prenom")?),
            }))
        }
        _ => Ok(json!({"texte": "", "manque": [], "ici": false})),
    }
}

/// `coffre_etat` : ce qui est pose, pour quel usage, depuis quand. Jamais les
/// valeurs.
fn coffre_etat(ctx: &Contexte) -> R<Value> {
    coffre_etat_de(&ctx.racine, &ctx.entreprise)
}

/// `coffre_etat` sur une fiche entreprise donnee : `poser_compte_dpae` le relit sur la fiche qu'il vient d'ecrire.
pub(crate) fn coffre_etat_de(racine: &Path, entreprise: &Value) -> R<Value> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(racine, "");
    let secrets = coffre.etat().map_err(vers_route)?;
    // UNE COMPAGNIE DONT LES ENVOIS SONT JOUES n'a rien a poser : chaque pastille le dit, en gris.
    let simule = crate::compagnie::bridage::branchement(racine).joue();
    let mut services = Vec::new();
    for s in &secrets {
        let nom = s["service"].as_str().unwrap_or("");
        // Sa pastille : simule, une information ; fourni par l'environnement (il passe devant le coffre), a voir ; pose,
        // fait ; absent, a poser.
        let pastille = if simule {
            super::pastille("information", "simulé")
        } else if vrai(&s["dans_environnement"]) {
            super::pastille("a_faire", "fourni par l'environnement")
        } else if vrai(&s["pose"]) {
            let le = s["pose_le"].as_str().and_then(|t| t.get(..10)).and_then(|t| socle::date::Jour::lire(t).ok()).map(|j| j.jj_mm_aaaa());
            super::pastille("fait", &le.map_or_else(|| "posé".to_string(), |l| format!("posé le {}", l)))
        } else {
            super::pastille("a_faire", "absent")
        };
        services.push(json!({
            "service": s["service"], "libelle": s["libelle"], "sert_a": s["sert_a"],
            "pose": s["pose"], "pose_le": s["pose_le"], "environnement": s["dans_environnement"],
            "compte": s["compte"], "avec_compte": s["avec_compte"],
            "identifiant": identifiant_du_compte(entreprise, nom)?,
            "pastille": pastille,
        }));
    }
    let mot_de_passe_pose = coffre.ouverture_exigee().map_err(vers_route)?;
    let cle_poste = coffre.cle_posee().map_err(vers_route)?;
    Ok(json!({
        "services": services,
        "mot_de_passe_pose": mot_de_passe_pose,
        "cle_poste": cle_poste,
        "chemin_cle": coffre.chemin_cle().display().to_string(),
        // Ce que montrent le filet et le compte Google d'une compagnie fictive, ou rien ne se branche : une information.
        "fictive": {"google": super::pastille("information", "non lié")},
        // La pastille du mot de passe du logiciel (rattache a l'administrateur, la page dit a combien de compagnies).
        "pastille_mot_de_passe": if simule {
            super::pastille("information", "simulé")
        } else if mot_de_passe_pose {
            super::pastille("fait", "posé")
        } else {
            super::pastille("a_faire", "aucun")
        },
    }))
}

/// CE QUE L'ECRAN DES MOTS DE PASSE DIT (`ecran` de `coffre-etat`) : son introduction, l'encart d'une compagnie fictive ou
/// d'un poste sans cle, la ligne du mot de passe du logiciel et celle de l'administrateur, les notes des comptes Urssaf
/// et courriel, a quoi servent le compte Google et le filet, et ce que demandent les confirmations. La page n'en ecrit
/// rien ; chaque service porte aussi ce que sa confirmation d'oubli demande (`oublier`).
fn ecran_du_coffre(ctx: &Contexte, v: &mut Value) -> Value {
    let fictive = crate::compagnie::bridage::branchement(&ctx.racine).joue();
    let adm = if fictive { json!({}) } else { administrateur_etat(&ctx.racine) };
    let autres: Vec<String> = adm["autres"].as_array().into_iter().flatten().map(valeur_en_texte).collect();
    let en_liste = match autres.len() {
        0 => String::new(),
        1 => autres[0].clone(),
        k => format!("{} et {}", autres[..k - 1].join(", "), autres[k - 1]),
    };
    let rattachee = vrai(&adm["rattachee"]);
    let pose = vrai(&v["mot_de_passe_pose"]);
    for s in v["services"].as_array_mut().into_iter().flatten() {
        s["oublier"] = json!(format!("Oublier le mot de passe {} ? Les retours et les dépôts ne seront plus possibles jusqu'à ce qu'il soit reposé.", valeur_en_texte(&s["libelle"])));
    }
    let encart = if fictive {
        super::encart("information", "Compagnie fictive : les champs sont grisés", "Les dépôts de DPAE et de DSN, leurs retours, les courriels et les signatures sont simulés : aucun mot de passe n'est demandé, et rien ne quitte le poste. Dans une compagnie réelle, c'est ici que se posent les accès à net-entreprises, à l'Urssaf et à la boîte courriel.")
    } else if !vrai(&v["cle_poste"]) {
        super::encart("information", "Cet ordinateur n'a pas encore sa clé du coffre", "Rien n'est perdu : elle se refait toute seule la prochaine fois que le mot de passe du logiciel est tapé (ou posé, ci-dessous). Elle vit à part des données, pour qu'une sauvegarde n'emporte jamais de quoi ouvrir le coffre.")
    } else {
        Value::Null
    };
    let demande = "demandé à l'ouverture et à chaque dépôt réel, envoi ou signature";
    json!({
        "intro": "Les mots de passe dont le logiciel a besoin pour déposer, relever et envoyer. La page ne voit jamais un mot de passe, seulement s'il est posé.",
        "encart": encart,
        "logiciel": {
            "nom": if rattachee { "Le mot de passe de l'administrateur" } else { "Le mot de passe du logiciel" },
            "dit": if rattachee { format!("il ouvre aussi {} ; {}", en_liste, demande) } else if pose || fictive { demande.to_string() } else { "rien ne peut partir en réel tant qu'il n'est pas posé".to_string() },
            "nouveau": if rattachee { "six caractères au moins ; il remplacera l'actuel dans toutes ses compagnies" } else { "six caractères au moins" },
            "retirer": "Retirer le mot de passe du logiciel ? La fenêtre s'ouvrira sans rien demander et le coffre ne sera plus protégé.",
        },
        "administrateur": if rattachee {
            json!({"dit": format!("cette compagnie s'ouvre avec {}, d'un seul mot de passe", en_liste), "note": "Détachée, elle a son propre mot de passe : celui de l'administrateur ne l'ouvre plus, et elle dépose seule."})
        } else if vrai(&adm["peut_rattacher"]) {
            json!({"dit": format!("le mot de passe de {} peut ouvrir aussi cette compagnie", en_liste), "note": "Rattachée, elle s'ouvre avec le mot de passe de l'administrateur, et ses déclarations prêtes partent avec celles des autres compagnies, d'un seul mot de passe."})
        } else {
            Value::Null
        },
        "urssaf": "Le compte ouvert sur urssaf.fr pour la DPAE (service « DPAE Service plus ») : ce n'est pas celui de net-entreprises. Les accents sont retirés, la DPAE les refuse. Un mot de passe vide garde celui qui est posé.",
        "courriel": "Pour Gmail : validation en deux étapes activée, puis myaccount.google.com › Sécurité › Mots de passe des applications. C'est ce mot de passe-là, pas celui du compte.",
        "verifier": "Une tentative de connexion chez l'organisme, une seule.",
        "google": "la sauvegarde du dossier privé et l'archivage des mois, dans son Drive ; rien d'autre",
        "google_ouvert": "La fenêtre Google est ouverte dans votre navigateur : choisissez le compte, acceptez, puis revenez ici ; cette ligne se met à jour toute seule. Si Google dit que l'application n'est pas validée : Paramètres avancés › Accéder à PuppetPaie.",
        "google_oublier": "Oublier le compte Google ? Le jeton local est effacé : plus de sauvegarde ni d'archivage jusqu'à ce qu'un Drive soit relié. L'accès accordé côté Google, lui, se retire sur myaccount.google.com › Sécurité › Vos connexions à des applications tierces.",
        "diagnostic": "ce qui casse part chez l'éditeur, avec de quoi le reproduire (ses données, les 3 minutes d'avant, cet ordinateur), et une fois par an le nombre de paies faites ; les noms, NIR, adresses et coordonnées des salariés n'en partent pas",
    })
}
