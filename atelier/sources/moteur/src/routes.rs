//! `moteur route` : les routes de l'interface. La fenetre les sert dans son processus (`dans_la_fenetre`, ticket #244) ;
//! `moteur route`, une par processus, reste pour la ligne de commande et les tests.
//!
//! Demande sur l'entree standard : `{"route": "...", "charge": {...}}`, avec `--racine` et `--date`. Reponse :
//! `{"ok": true, "resultat": ...}` ; un refus rend `{"ok": false, "erreur": "...", "classe": "..."}` avec sa classe
//! (`ErreurOperation`, `FileNotFoundError`, `KeyError`...), que la page releve.
//!
//! L'aiguillage ne fait rien d'autre : chaque lot de routes vit dans son
//! module et s'ajoute ici d'une ligne. Les routes de l'ouverture
//! (`routes_ouverture`, `rappels` et `apercu`) font leur propre Contexte, et
//! leur refus classe voyage dans le texte de l'erreur
//! (`routes_ouverture::decoder`).

use std::collections::HashMap;

use serde_json::{json, Value};

use socle::num::Resultat;
use socle::echec::Echec;
use crate::service::contexte;

/// La pile d'un fil qui sert des routes dans la fenetre : 8 Mo, celle du fil principal d'un `moteur route` sous Linux (un
/// fil neuf n'en a que 2). Un calcul qui remonte les mois d'avant s'y empile ; un debordement de pile ne se rattrape pas
/// (`catch_unwind` n'y peut rien), il fermerait la fenetre.
pub const PILE_D_UNE_ROUTE: usize = 8 * 1024 * 1024;

/// Un fil qui sert des routes dans la fenetre (une connexion, la file des etats, la veille, un passage en fond), avec
/// la pile qu'il leur faut (`PILE_D_UNE_ROUTE`).
pub fn fil_de_routes<F: FnOnce() + Send + 'static>(f: F) {
    if let Err(e) = std::thread::Builder::new().stack_size(PILE_D_UNE_ROUTE).spawn(f) {
        eprintln!("Un fil du moteur ne demarre pas : {}.", e);
    }
}

/// La route d'essai qui tombe (`commande`, en essai seulement).
pub const ROUTE_QUI_TOMBE: &str = "essai-defaut-du-logiciel";

/// Le fil remis propre apres une route servie dans la fenetre (`dans_la_fenetre`, `etat_garde::FileDesEtats`) : plus
/// de mot de passe tape, les nombres a leur ecriture normale, aucun registre de lectures ouvert.
pub(crate) fn remettre_le_fil() {
    crate::coffre::oublier_mot_de_passe_tape();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    let _ = socle::fichiers::rendre();
}

/// UNE ROUTE SERVIE DANS LA FENETRE, ET CE QU'ELLE LAISSE SUR SON FIL (ticket #244). La fenetre sert ses routes dans son
/// processus (`interface::api::route`, la file des etats), et le fil d'une connexion en sert d'autres ensuite. Cette
/// fonction fait la route (`commande`), gardee d'une panique (une route qui tombe rend un refus `DefautDuLogiciel` et le
/// fil continue), puis remet le fil propre, quoi qu'il soit arrive : le mot de passe tape oublie (`coffre::TAPE` : il ne
/// survit jamais a sa route), les nombres a leur ecriture normale (`num::ECHELLE_GARDEE`), le registre des lectures
/// qu'un calcul aurait laisse ouvert en tombant ferme (`fichiers::suivre`).
///
/// `moteur route` et la ligne de commande (`commandes::Cadre`) appellent `commande` directement : le processus meurt avec
/// sa commande, et le mot de passe qu'elle a fait taper (`demande::autoriser_acte`) sert a toute la commande.
pub fn dans_la_fenetre(o: &HashMap<String, String>, route: &str, charge: &Value) -> Value {
    let entree = json!({"route": route, "charge": charge}).to_string();
    let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| commande(o, &entree)));
    remettre_le_fil();
    match rendu {
        Ok(Ok(rep)) => rep,
        Ok(Err(m)) => json!({"ok": false, "erreur": m}),
        Err(panique) => {
            let pourquoi = panique
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panique.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "sans phrase".into());
            json!({"ok": false, "classe": "DefautDuLogiciel", "erreur": format!("Defaut du logiciel : la route {} s'est arretee ({}).", route, pourquoi)})
        }
    }
}

/// `moteur route [--racine R] [--date D]`.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    // Le code livre ne bouge pas le temps de la route : ses signatures se lisent une fois (`fichiers::code_fige`).
    let _code = socle::fichiers::code_fige();
    crate::incidents::brancher_l_engagement();
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let route = d.get("route").and_then(Value::as_str).ok_or("demande illisible : il faut `route`")?;
    let charge = d.get("charge").cloned().unwrap_or_else(|| json!({}));
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`) ;
    // dans la fenetre, le temps de cette route seulement (`dans_la_fenetre`).
    if let Some(m) = charge.get("motdepasse").and_then(Value::as_str) {
        crate::coffre::retenir_mot_de_passe_tape(m);
    }
    // UNE ROUTE QUI TOMBE, pour les essais seulement (`garde_reseau::en_essai`, que Cargo pose) : la fenetre doit rendre
    // un refus et servir la suivante (`dans_la_fenetre`, `tests/routes_dans_la_fenetre.rs`). Le fil est laisse sale
    // expres, comme un calcul qui tombe le laisserait.
    if route == ROUTE_QUI_TOMBE && socle::garde_reseau::en_essai() {
        socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
        panic!("la route d'essai tombe expres");
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
    // UNE ROUTE QUI PORTE UN MOIS LIT LA CONFIGURATION DE CE MOIS (`Contexte::du_mois`) : sa paie, sa DSN et ses documents
    // se font sous les valeurs qui valaient alors, jamais sous celles d'aujourd'hui.
    let ctx = match charge.get("mois").and_then(Value::as_str).filter(|m| socle::date::premier_du_mois(m).is_ok()) {
        Some(m) => contexte(o)?.du_mois(m)?,
        None => contexte(o)?,
    };
    let rendu: Result<Value, Echec> = match route {
        // Ce qu'il manque a la compagnie pour ses premiers actes, d'un coup (`demarrage`).
        declarations::demarrage::ROUTE => declarations::demarrage::route(&ctx),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        crate::organismes::ROUTE_LIRE => crate::organismes::lire(&ctx, &charge),
        crate::organismes::ROUTE_ECRIRE => crate::organismes::enregistrer(&ctx, &charge),
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        r if crate::etat_garde::ROUTES.contains(&r) => crate::etat_garde::route(o, &ctx, route, &charge),
        // Le mois, sans calcul de paie.
        "embauches-a-declarer" | "mois-a-ouvrir" | "bilan-du-mois" | "fichiers-deposables" => {
            crate::routes_mois::route(o, &ctx, route, &charge)
        }
        // L'accueil : la page lit tout en s'ouvrant.
        "etat" | "configuration" => crate::routes_accueil::route(o, &ctx, route, &charge),
        // Supprimer la compagnie : ce que la boite demande (`interface::suppression::question`) ; la suppression elle-meme
        // est un geste de la fenetre, qui lache d'abord ce qu'elle tient de la compagnie.
        "compagnie-suppression" => crate::interface::suppression::question(&ctx.racine).map_err(|m| Echec { classe: Some("ErreurOperation"), message: m }),
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
        // L'Historique d'un mois : ses documents, ses envois et leurs retours, lus (lecture seule).
        crate::historique_mois::ROUTE => crate::historique_mois::route(&ctx, &charge),
        // Les apercus des documents du mois : le contrat a signer, l'avenant, l'envoi.
        "signature-apercu" | "avenant-apercu" | "envoi-apercu" => crate::routes_documents::route(o, &ctx, route, &charge),
        // L'import d'un export CoolPaie : ce qu'il ferait ; le tri de la boite aux lettres `import/` (rien n'est ecrit).
        "importation-apercu" | "importation-tri" => crate::routes_importation::route(o, &ctx, route, &charge),
        // Les fichiers que l'import n'a pas repris : le message caviarde qui partirait chez l'editeur (rien n'est ecrit),
        // et son envoi, sur le seul clic de la personne (`importation_incident`).
        r if crate::importation_incident::ROUTES.contains(&r) => crate::importation_incident::route(&ctx, route, &charge),
        // Les corrections d'un mois depose : ce que la page en montre.
        "regularisation-apercu" | "regularisation-mois" => crate::routes_regularisation::route(o, &ctx, route, &charge),
        // Le dossier d'un salarie : ses documents, recrees ou pris tels quels, en un PDF ou un ZIP.
        "dossier-salarie" => crate::routes_dossier_salarie::route(o, &ctx, route, &charge),
        // Les conseils « Le saviez-vous ? » (ticket 224) : le conseil du jour, ceux du dossier d'un salarie, le resume de l'accueil.
        r if crate::conseils::ROUTES.contains(&r) => crate::conseils::route(&ctx, route, &charge),
        // Les routes qui ecrivent : la revue des spectacles, la correction d'un mois depose.
        "revue-spectacles" => crate::routes_ecrites::route(o, &ctx, route, &charge),
        // Les ecritures de l'ecran : fiches, identite, metiers, saisie d'un mois, total vise.
        r if crate::routes_ecritures::ROUTES.contains(&r) => crate::routes_ecritures::route(o, &ctx, route, &charge),
        // Les ecritures du lot G2 : corrections d'un mois depose, arrets, echeances cochees ; et le cout de l'annee.
        r if crate::routes_ecritures_g2::ROUTES.contains(&r) => crate::routes_ecritures_g2::route(o, &ctx, route, &charge),
        // Les imprevus d'un salarie : la liste, le maintien d'un arret, la suspension du bloc 65 saisie une fois.
        r if crate::routes_imprevus::ROUTES.contains(&r) => crate::routes_imprevus::route(o, &ctx, route, &charge),
        // Les ecritures du lot G3 : coffre, mot de passe du logiciel, jeton Google, mise a jour, cloture, export compta, import.
        r if crate::routes_ecritures_g3::ROUTES.contains(&r) => crate::routes_ecritures_g3::route(o, &ctx, route, &charge),
        // Les ecritures du lot G4 : valider ou modifier un engagement, le contrat recu, les reglages et reprises locales.
        r if crate::routes_ecritures_g4::ROUTES.contains(&r) => crate::routes_ecritures_g4::route(&ctx, route, &charge),
        // Les obligations d'un arret (15/10/2026) : le jour ou la compagnie l'a su, la declaration d'accident faite.
        r if crate::obligations::ROUTES.contains(&r) || crate::obligations::ROUTES_LECTURE.contains(&r) => crate::obligations::route(&ctx, route, &charge),
        // Les productions du lot H1 : la DPAE, le signalement d'arret, la fin de contrat et ses papiers (rien ne part).
        r if crate::routes_actes_h1::ROUTES.contains(&r) => crate::routes_actes_h1::route(&ctx, route, &charge),
        // Les actes du lot H2 : deposer, suivre un depot, le bilan d'une repetition, le filet. Un refus qui porte une
        // donnee pour la page (`etat`) la rend a cote de `erreur` et `classe`.
        r if crate::routes_actes_h2::ROUTES.contains(&r) => match crate::routes_actes_h2::route(o, &ctx, route, &charge) {
            Err(Echec { classe: Some(c), message }) if matches!(c, "RepetitionManquante" | "DsnDejaDeposee") => {
                let d: Value = serde_json::from_str(&message).unwrap_or_else(|_| json!({"message": message}));
                return Ok(json!({"ok": false, "erreur": d["message"], "classe": c, "etat": d["etat"]}));
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
        r if crate::releve_signatures::ROUTES.contains(&r) => crate::releve_signatures::route(&ctx, route, &charge),
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
        "normaliser" | "taux-atmp" | "verifier-fiche" | "changements-declares" | "taxe-sur-les-salaires-regime" | "permanents" => crate::routes_fiches::route(o, &ctx, route, &charge),
        // Les incidents et le canal vers l'editeur : le journal, « Un probleme, une idee ? », le filet DPAE.
        r if crate::routes_incidents::ROUTES.contains(&r) || r == crate::routes_incidents::ROUTE_OUBLIER => {
            crate::routes_incidents::route(o, &ctx, route, &charge)
        }
        autre => return Err(format!("route inconnue : {}", autre)),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    let rendu = match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && classe != Some(crate::bridage::CLASSE_REFUS_D_ENVOI) && classe != Some(crate::routes_ecritures_g4::CLASSE_DOCUMENTS_PARTIS) && classe != Some(crate::routes_ecritures_g4::CLASSE_ENGAGEMENT_PARTI) && !message.trim_start().starts_with('{') => {
            let suite = declarations::demarrage::a_la_suite(&ctx).unwrap_or_default();
            Err(Echec { classe, message: message + &suite })
        }
        autre => autre,
    };
    // Les etats des mois ne se refont plus ici : c'est la fenetre qui les refait, dans son processus, apres chaque
    // ecriture (`etat_garde::FileDesEtats`, ticket #212). Hors de la fenetre, un etat perime se refait a sa lecture.
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

/// Les routes qui ecrivent (les lots d'ecritures, d'actes, d'envois et de releves) : apres elles, la fenetre verifie les
/// etats des mois (`interface/api.rs`).
pub fn est_une_ecriture(route: &str) -> bool {
    matches!(route, "revue-spectacles")
        || crate::routes_ecritures::ROUTES.contains(&route)
        || crate::routes_ecritures_g2::ROUTES.contains(&route)
        || (crate::routes_imprevus::ROUTES.contains(&route) && route != "imprevus")
        || crate::routes_ecritures_g3::ROUTES.contains(&route)
        || crate::routes_ecritures_g4::ROUTES.contains(&route)
        || crate::routes_actes_h1::ROUTES.contains(&route)
        || crate::obligations::ROUTES.contains(&route)
        || crate::routes_actes_h2::ROUTES.contains(&route)
        || crate::routes_envois::ROUTES.contains(&route)
        || crate::routes_fiches_courriel::ROUTES.contains(&route)
        || crate::releve_signatures::ROUTES.contains(&route)
}
