//! `paie route` : les routes de l'interface, une par processus.
//!
//! Demande sur l'entree standard : `{"route": "...", "charge": {...}}`, avec
//! `--racine` et `--date` comme `paie interne`. Reponse : `{"ok": true,
//! "resultat": ...}`, le JSON que la route Python rendait ; un refus rend
//! `{"ok": false, "erreur": "...", "classe": "..."}` avec la classe
//! d'exception que Python levait (`ErreurOperation`, `FileNotFoundError`,
//! `KeyError`...), que `paie_rust.route` releve sous le meme nom.
//!
//! L'aiguillage ne fait rien d'autre : chaque lot de routes vit dans son
//! module et s'ajoute ici d'une ligne. Les routes de l'ouverture
//! (`routes_ouverture`, `rappels` et `apercu`) font leur propre Contexte, et
//! leur refus classe voyage dans le texte de l'erreur
//! (`routes_ouverture::decoder`).

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::num::Resultat;
use crate::retours::Echec;
use crate::service::contexte;

/// `paie route [--racine R] [--date D]`.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let route = d.get("route").and_then(Value::as_str).ok_or("demande illisible : il faut `route`")?;
    let charge = d.get("charge").cloned().unwrap_or_else(|| json!({}));
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`).
    if let Some(m) = charge.get("motdepasse").and_then(Value::as_str) {
        crate::coffre::retenir_mot_de_passe_tape(m);
    }
    if let Some(r) = crate::routes_ouverture::traiter(o, route, &charge) {
        return match r {
            Ok(v) => Ok(json!({"ok": true, "resultat": v})),
            Err(e) => match crate::routes_ouverture::decoder(&e) {
                (Some(c), message) => Ok(json!({"ok": false, "erreur": message, "classe": c})),
                (None, message) => Err(message.to_string()),
            },
        };
    }
    let ctx = contexte(o)?;
    let rendu: Result<Value, Echec> = match route {
        // Ce qu'il manque a la compagnie pour ses premiers actes, d'un coup (`demarrage`).
        crate::demarrage::ROUTE => crate::demarrage::route(&ctx),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        crate::organismes::ROUTE_LIRE => crate::organismes::lire(&ctx),
        crate::organismes::ROUTE_ECRIRE => crate::organismes::enregistrer(&ctx, &charge),
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        r if crate::etat_garde::ROUTES.contains(&r) => crate::etat_garde::route(o, &ctx, route, &charge),
        // Le mois, sans calcul de paie.
        "embauches-a-declarer" | "mois-a-ouvrir" | "bilan-du-mois" | "fichiers-deposables" => {
            crate::routes_mois::route(o, &ctx, route, &charge)
        }
        // L'accueil : la page lit tout en s'ouvrant.
        "etat" | "configuration" => crate::routes_accueil::route(o, &ctx, route, &charge),
        // La saisie d'un mois et l'echeancier : la page les lit en s'ouvrant.
        "saisie" | "echeancier" => crate::routes_saisie::route(o, &ctx, route, &charge),
        // Les petites lectures de l'ouverture : identite, exercices, coffre, filet, importation, mise a jour, signalements, pieces parties.
        "identite" | "exercices" | "coffre-etat" | "filet-etat" | "importation-etat" | "maj-etat" | "signalements" | "pieces-parties" => {
            crate::routes_etats::route(o, &ctx, route, &charge)
        }
        // Les listes de l'ouverture : arrets, fins de contrat a signaler, mois a regulariser.
        "arrets" | "fins-de-contrat" | "regularisations" => crate::routes_listes::route(o, &ctx, route, &charge),
        // Les contrats en cours et passes, l'emploi, les rubriques cochees, le catalogue des metiers.
        "contrats-en-cours" | "contrats-passes" | "emploi" | "rubriques-actives" | "metiers-catalogue" => {
            crate::routes_contrats::route(o, &ctx, route, &charge)
        }
        // Les exports et les dossiers : les contrats qu un arret suspendrait, les formes du dossier salarie.
        "arret-contrats" | "dossier-formes" | "exports-periodes" => crate::routes_dossiers::route(o, &ctx, route, &charge),
        // Les apercus : ce que la page montre avant d'agir (lecture seule).
        "documents-liste" | "depot-apercu" => crate::routes_apercus::route(o, &ctx, route, &charge),
        // Les apercus des documents du mois : le contrat a signer, l'avenant, l'envoi.
        "signature-apercu" | "avenant-apercu" | "envoi-apercu" => crate::routes_documents::route(o, &ctx, route, &charge),
        // L'import d'un export CoolPaie : ce qu'il ferait ; le tri de la boite aux lettres `import/` (rien n'est ecrit).
        "importation-apercu" | "importation-tri" => crate::routes_importation::route(o, &ctx, route, &charge),
        // Les fichiers que l'import n'a pas repris : le message caviarde qui partirait chez l'editeur (rien n'est ecrit),
        // et son envoi, sur le seul clic de la personne (`importation_incident`).
        r if crate::importation_incident::ROUTES.contains(&r) => crate::importation_incident::route(&ctx, route, &charge),
        // La regularisation : ce que la correction d'un mois depose changerait.
        "regularisation-apercu" => crate::routes_regularisation::route(o, &ctx, route, &charge),
        // Le dossier d'un salarie : ses documents, recrees ou pris tels quels, en un PDF ou un ZIP.
        "dossier-salarie" => crate::routes_dossier_salarie::route(o, &ctx, route, &charge),
        // Les routes qui ecrivent : la revue des spectacles, la correction d'un mois depose.
        "revue-spectacles" | "regularisation-mois" => crate::routes_ecrites::route(o, &ctx, route, &charge),
        // Les ecritures de l'ecran : fiches, identite, metiers, saisie d'un mois, total vise.
        r if crate::routes_ecritures::ROUTES.contains(&r) => crate::routes_ecritures::route(o, &ctx, route, &charge),
        // Les ecritures du lot G2 : corrections d'un mois depose, arrets, echeances cochees ; et le cout de l'annee.
        r if crate::routes_ecritures_g2::ROUTES.contains(&r) => crate::routes_ecritures_g2::route(o, &ctx, route, &charge),
        // Les imprevus d'un salarie : la liste, le maintien d'un arret, la suspension du bloc 65 saisie une fois.
        r if crate::routes_imprevus::ROUTES.contains(&r) => crate::routes_imprevus::route(o, &ctx, route, &charge),
        // Les ecritures du lot G3 : coffre, mot de passe du logiciel, jeton Google, mise a jour, cloture, export compta, import.
        r if crate::routes_ecritures_g3::ROUTES.contains(&r) => crate::routes_ecritures_g3::route(o, &ctx, route, &charge),
        // Les ecritures du lot G4 : valider une paie, le contrat recu, les reglages et reprises locales.
        r if crate::routes_ecritures_g4::ROUTES.contains(&r) => crate::routes_ecritures_g4::route(o, &ctx, route, &charge),
        // Les productions du lot H1 : la DPAE, le signalement d'arret, la fin de contrat et ses papiers (rien ne part).
        r if crate::routes_actes_h1::ROUTES.contains(&r) => crate::routes_actes_h1::route(o, &ctx, route, &charge),
        // Les actes du lot H2 : deposer, suivre un depot, le bilan d'une repetition, le filet. Un refus qui porte une
        // donnee pour la page (`etat`, `manques`) la rend a cote de `erreur` et `classe`.
        r if crate::routes_actes_h2::ROUTES.contains(&r) => match crate::routes_actes_h2::route(o, &ctx, route, &charge) {
            Err(Echec { classe: Some(c), message }) if matches!(c, "RepetitionManquante" | "DsnDejaDeposee" | "MoisPasPret") => {
                let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
                let cle = if c == "MoisPasPret" { "manques" } else { "etat" };
                return Ok(json!({"ok": false, "erreur": d["message"], "classe": c, cle: d[cle]}));
            }
            autre => autre,
        },
        // Les envois du lot H3b : les documents du mois, un contrat ou un avenant a la signature, un dossier, les courriels
        // de la regularisation CASC. Le mot de passe du logiciel, retape ; la boite simulee dans une compagnie fictive. Le filet DPAE
        // en fond de `demander-signature` n'est pas ici : `web.py` le lance apres l'appel.
        r if crate::routes_envois::ROUTES.contains(&r) => crate::routes_envois::route(o, &ctx, route, &charge),
        // Les fiches par courriel et la verification des comptes du lot H4a : la fiche remplie par le salarie lui-meme
        // (la demande part avec le mot de passe du logiciel ; la releve ne fait rien partir), et le secret pose ouvre-t-il
        // une session chez l'organisme (UNE seule tentative).
        r if crate::routes_fiches_courriel::ROUTES.contains(&r) => crate::routes_fiches_courriel::route(o, &ctx, route, &charge),
        // La releve des reponses « Je signe » du lot I2a : cherche les codes attendus dans la boite (la vraie, ou celle d'une
        // compagnie fictive), note les signatures, archive la preuve, contresigne et renvoie le contrat. N'a de sens, hors d'une
        // compagnie fictive, que pour relever pour de bon : une vraie boite est lue.
        r if crate::releve_signatures::ROUTES.contains(&r) => crate::releve_signatures::route(o, &ctx, route, &charge),
        // L'ouverture du logiciel et la veille (lot I2b) : sauvegarder, relever les retours, les signatures et les fiches, sans
        // jamais echouer. ATTENTION : hors d'une compagnie fictive, cette route va au reseau (net-entreprises, Urssaf,
        // boite courriel, Drive, serveur des incidents).
        r if crate::ouverture::ROUTES.contains(&r) => crate::ouverture::route(o, &ctx, route, &charge),
        crate::coffre_verifier::ROUTE => crate::coffre_verifier::route(&ctx, &charge),
        // La veille lit la sauvegarde sans reseau : ce que l'index dit du dernier passage, et si le disque n'a pas bouge (lot H4b).
        r if crate::routes_sauvegarde::ROUTES.contains(&r) => crate::routes_sauvegarde::route(o, &ctx, route, &charge),
        // L'horloge de la compagnie fictive (lot I1) : rattraper ce qui etait du, et ce que le bandeau « pendant votre absence »
        // lit. Elle ne joue que dans une compagnie fictive (`horloge::HorlogeFictive`).
        r if crate::horloge::ROUTES.contains(&r) => crate::horloge::route(o, &ctx, route, &charge),
        // Le compte Google : le jeton se rafraichit reellement aupres de Google.
        "google-etat" => crate::routes_google::route(o, &ctx, route, &charge),
        // Le modele du bulletin (lot R11) : les modeles, l'apercu d'une paie fictive sous chacun, le choix de la compagnie.
        r if crate::routes_bulletin_modele::ROUTES.contains(&r) => crate::routes_bulletin_modele::route(&ctx, route, &charge),
        // La fiche : ce qu elle dirait, ce qu elle changerait, la forme d'une case, le taux AT/MP.
        "normaliser" | "taux-atmp" | "verifier-fiche" | "changements-declares" => crate::routes_fiches::route(o, &ctx, route, &charge),
        // Les incidents et le canal vers l'editeur : le journal, « Un probleme, une idee ? », le filet DPAE.
        r if crate::routes_incidents::ROUTES.contains(&r) || r == crate::routes_incidents::ROUTE_OUBLIER => {
            crate::routes_incidents::route(o, &ctx, route, &charge)
        }
        autre => return Err(format!("route inconnue : {}", autre)),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    let rendu = match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && !message.trim_start().starts_with('{') => {
            let suite = crate::demarrage::a_la_suite(&ctx).unwrap_or_default();
            Err(Echec { classe, message: message + &suite })
        }
        autre => autre,
    };
    // UNE ECRITURE QUI NOMME UN MOIS REFAIT SON ETAT, en arriere-plan : l'ecran le trouvera pret.
    if rendu.is_ok() && est_une_ecriture(route) {
        if let Some(mois) = charge.get("mois").and_then(Value::as_str).filter(|m| m.len() >= 7) {
            crate::etat_garde::apres_ecriture(o, &mois[..7]);
        }
    }
    match rendu {
        Ok(v) => Ok(json!({"ok": true, "resultat": v})),
        // Une fiche refusee porte ses anomalies : la page les montre.
        Err(Echec { classe: Some("FicheRefusee"), message }) => {
            let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
            Ok(json!({"ok": false, "erreur": d["message"], "classe": "FicheRefusee", "anomalies": d["anomalies"]}))
        }
        Err(Echec { classe: None, message }) => Err(message),
        Err(Echec { classe: Some(c), message }) => Ok(json!({"ok": false, "erreur": message, "classe": c})),
    }
}

/// Les routes qui ecrivent (les lots d'ecritures, d'actes, d'envois et de releves) : apres elles, l'etat du mois se refait.
fn est_une_ecriture(route: &str) -> bool {
    matches!(route, "revue-spectacles" | "regularisation-mois")
        || crate::routes_ecritures::ROUTES.contains(&route)
        || crate::routes_ecritures_g2::ROUTES.contains(&route)
        || (crate::routes_imprevus::ROUTES.contains(&route) && route != "imprevus")
        || crate::routes_ecritures_g3::ROUTES.contains(&route)
        || crate::routes_ecritures_g4::ROUTES.contains(&route)
        || crate::routes_actes_h1::ROUTES.contains(&route)
        || crate::routes_actes_h2::ROUTES.contains(&route)
        || crate::routes_envois::ROUTES.contains(&route)
        || crate::routes_fiches_courriel::ROUTES.contains(&route)
        || crate::releve_signatures::ROUTES.contains(&route)
}
