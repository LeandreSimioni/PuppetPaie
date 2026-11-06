//! `moteur route` : les routes de l'interface. La fenetre les sert dans son processus (`dans_la_fenetre`) ; `moteur
//! route`, une par processus, sert la ligne de commande et les tests.
//!
//! Demande sur l'entree standard : `{"route": "...", "charge": {...}}`, avec `--racine` et `--date`. Reponse :
//! `{"ok": true, "resultat": ...}` ; un refus rend `{"ok": false, "erreur": "...", "classe": "..."}` avec sa classe
//! (`ErreurOperation`, `FileNotFoundError`, `KeyError`...), que la page releve.
//!
//! UN MODULE PAR PLACE DE L'ECRAN (`catalogue::Place`) : la Paie (`mois`, `etapes`, `engagement`, `contrats`,
//! `documents`, `depots`, `imprevus`, `rappels`), la Configuration (`fiches`, `metiers`, `identite`, `bulletin`,
//! `modeles_contrat`, `coffre`, `import`), l'Historique (`historique`), les Exports (`archives`, `dossier_salarie`),
//! « Un probleme, une idee ? » (`filet`), l'accueil des compagnies (`accueil` : les routes de l'installation), et les lectures que les ecrans partagent (`ouverture`). Chacun dit ses routes et
//! leurs fiches (`ROUTES`, `catalogue::FICHES`), comme les modules des obligations,
//! des conseils et de l'ouverture ; l'aiguillage ne fait rien d'autre que les servir (et l'horloge
//! du banc, hors catalogue). Ce que les routes partagent (lire un fichier, le salarie d'une charge, `INTERFACE`) est ici.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use socle::date::repr_texte;
use socle::echec::{erreur, Echec, R};
use ::documents::valeur::vrai;
use droit::contexte::Contexte;
use socle::num::Resultat;
use ::documents::valeur::{valeur_ecrite, valeur_en_texte};
use crate::service::contexte;
use catalogue::Route;


pub mod ouverture;
pub mod depots;
pub mod archives;
pub mod libelles_courts;
pub mod historique;
pub mod documents;
pub mod dossier_salarie;
pub mod coffre;
pub mod engagement;
pub mod contrats;
pub mod modeles_contrat;
pub mod fiches;
pub mod identite;
pub mod metiers;
pub mod bulletin;
pub mod etablissements;
pub mod atmp;
pub mod import;
pub mod imprevus;
pub mod filet;
pub mod etapes;
pub mod rappels;
pub mod mois;
pub mod accueil;
pub mod catalogue;
pub mod affiche;

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

/// Le fil remis propre apres une route servie dans la fenetre (`dans_la_fenetre`, `etat_garde::FileDesEtats`) : plus
/// de mot de passe tape, les nombres a leur ecriture normale, aucun registre de lectures ouvert.
pub(crate) fn remettre_le_fil() {
    crate::compagnie::coffre::oublier_mot_de_passe_tape();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    socle::fichiers::fermer_tout();
}

/// UNE ROUTE SERVIE DANS LA FENETRE, ET CE QU'ELLE LAISSE SUR SON FIL. La fenetre sert ses routes dans son
/// processus (`interface::api::route`, la file des etats), et le fil d'une connexion en sert d'autres ensuite. Cette
/// fonction fait la route (`servir_la_route`), gardee d'une panique (une route qui tombe rend un refus `DefautDuLogiciel`
/// et le fil continue), puis remet le fil propre, quoi qu'il soit arrive : le mot de passe tape oublie (`coffre::TAPE` :
/// il ne survit jamais a sa route), les nombres a leur ecriture normale (`num::ECHELLE_GARDEE`), le registre des lectures
/// qu'un calcul aurait laisse ouvert en tombant ferme (`fichiers::suivre`).
///
/// `moteur route` et la ligne de commande (`commandes::Cadre`) appellent `commande` directement : le processus meurt avec
/// sa commande, et le mot de passe qu'elle a fait taper (`demande::autoriser_acte`) sert a toute la commande.
pub fn dans_la_fenetre(o: &HashMap<String, String>, route: Route, charge: &Value) -> Value {
    let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| servir_la_route(o, route, charge)));
    remettre_le_fil();
    match rendu {
        Ok(rep) => rep,
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

/// `moteur route [--racine R] [--date D]` : la demande `{"route", "charge"}` lue (la route une fois, par son nom :
/// `Route::lire`), puis servie.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    Ok(match lire_la_demande(entree) {
        Ok((route, charge)) => servir_la_route(o, route, &charge),
        Err(e) => reponse(crate::service::compagnie_nommee(o).then(|| crate::service::racine_de(o)).as_deref(), "", Err(e)),
    })
}

/// La route et la charge d'une demande ; un refus si elle ne se lit pas, ne nomme pas sa route, ou en nomme une qui
/// n'existe pas.
fn lire_la_demande(entree: &str) -> Result<(Route, Value), Echec> {
    let d: Value = serde_json::from_str(entree)
        .map_err(|e| Echec::from(socle::echec::avec("La demande ne se lit pas : ce n'est pas du JSON.", json!({"detail": e.to_string()}))))?;
    let nom = d.get("route").and_then(Value::as_str).unwrap_or_default();
    if nom.is_empty() {
        return Err(socle::echec::avec("La demande ne nomme pas sa route.", json!({"cle": "route"})).into());
    }
    let route = Route::lire(nom).ok_or_else(|| Echec::from(socle::echec::avec(catalogue::ROUTE_INCONNUE, json!({"route": nom}))))?;
    Ok((route, d.get("charge").cloned().unwrap_or_else(|| json!({}))))
}

/// La route `route` servie sur `charge`, et ce que l'ecran et le LLM en lisent (`reponse`).
pub(crate) fn servir_la_route(o: &HashMap<String, String>, route: Route, charge: &Value) -> Value {
    // Le code livre ne bouge pas le temps de la route : ses signatures se lisent une fois (`fichiers::code_fige`).
    let _code = socle::fichiers::code_fige();
    // Ce que la compagnie porte (les engagements, leurs paies, les imprevus) se lit une fois par geste (`socle::forme::geste`).
    let _geste = socle::forme::geste();
    crate::compagnie::incidents::brancher_l_engagement();
    let rendu = servir(o, route, charge);
    // Ce que la route rend, reussite ou refus, est ce que l'ecran lit : ses phrases passent ici (`phrase_a_sa_place`).
    let racine = crate::service::compagnie_nommee(o).then(|| crate::service::racine_de(o));
    if let (Ok(v), Some(r)) = (&rendu, &racine) {
        if catalogue::dans(accueil::ROUTES, route).is_none() && catalogue::dans(catalogue::ROUTES, route).is_none() {
            phrase_a_sa_place(r, route, v);
        }
    }
    reponse(racine.as_deref(), route.nom(), rendu)
}

/// La route `route` servie : ce qu'elle rend, ou son refus.
fn servir(o: &HashMap<String, String>, route: Route, charge: &Value) -> Result<Value, Echec> {
    let charge = charge.clone();
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`) ;
    // dans la fenetre, le temps de cette route seulement (`dans_la_fenetre`).
    if let Some(m) = charge.get("motdepasse").and_then(Value::as_str) {
        crate::compagnie::coffre::retenir_mot_de_passe_tape(m);
    }
    // UNE ROUTE QUI TOMBE, pour les essais seulement (`garde_reseau::en_essai`, que Cargo pose) : la fenetre doit rendre
    // un refus et servir la suivante (`dans_la_fenetre`, `tests/routes/`). Le fil est laisse sale
    // expres, comme un calcul qui tombe le laisserait. Hors des essais, elle n'existe pas.
    if route == Route::EssaiDefautDuLogiciel {
        if !socle::garde_reseau::en_essai() {
            return Err(socle::echec::avec(catalogue::ROUTE_INCONNUE, json!({"route": route})).into());
        }
        socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
        panic!("la route d'essai tombe expres");
    }
    // UNE ROUTE DE L'INSTALLATION (l'accueil des compagnies) ne porte sur aucune compagnie : elle se sert sans contexte.
    if catalogue::dans(accueil::ROUTES, route).is_some() {
        return accueil::route(o, route, &charge);
    }
    // ECRIRE AU DEVELOPPEUR, ET CE QUI S'Y RATTACHE, SE SERT AUSSI SANS COMPAGNIE : depuis l'accueil, ou en ligne de
    // commande sans `--racine` (`filet::sans_compagnie`).
    if catalogue::dans(filet::ROUTES, route).is_some() && !crate::service::compagnie_nommee(o) {
        return filet::route_sans_compagnie(o, route, &charge);
    }
    // Le catalogue lui-meme, ce que `moteur routes --json` imprime : il ne porte sur aucune compagnie non plus.
    if route == Route::Routes {
        let mut v = catalogue::en_json(charge.get("avance").is_some_and(vrai));
        let lignes: Vec<Value> = v["routes"].as_array().into_iter().flatten().map(|f| json!(format!("{} : {}", mot(&f["nom"]), mot(&f["geste"])))).collect();
        let p = format!("Routes : {} (la fiche de chacune : moteur routes NOM).", lignes.len());
        v["phrases"] = Value::Array(lignes);
        return Ok(phrase_en_tete(v, p));
    }
    // UNE ROUTE QUI PORTE UN MOIS LIT LA CONFIGURATION DE CE MOIS (`Contexte::du_mois`) : sa paie, sa DSN et ses documents
    // se font sous les valeurs qui valaient alors, jamais sous celles d'aujourd'hui. Le mois de la charge se lit ici, une
    // fois : un mois qui ne se lit pas refuse la route, rien ne s'ecrit.
    let mois = match charge.get("mois").and_then(Value::as_str).filter(|m| !m.is_empty()) {
        Some(m) => Some(socle::mois::Mois::lire(m).map_err(|e| erreur("ValueError", socle::echec::a_la_cle("Le mois de la demande ne se lit pas : il s'écrit AAAA-MM, de 01 à 12.", "mois", e)))?),
        None => None,
    };
    let ctx = match mois {
        Some(m) => contexte(o)?.du_mois(&m.to_string())?,
        None => contexte(o)?,
    };
    let dans = |l: &[catalogue::Fiche]| catalogue::dans(l, route).is_some();
    let rendu: Result<Value, Echec> = match route {
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        _ if dans(crate::etat_garde::ROUTES) => crate::etat_garde::route(o, &ctx, route, &charge),
        // Un geste de l'ecran, une route : un module par geste.
        _ if dans(mois::ROUTES) => mois::route(o, &ctx, route, &charge),
        _ if dans(etapes::ROUTES) => etapes::route(o, &ctx, route, &charge),
        _ if dans(engagement::ROUTES) => engagement::route(o, &ctx, route, &charge),
        _ if dans(contrats::ROUTES) => contrats::route(o, &ctx, route, &charge),
        _ if dans(modeles_contrat::ROUTES) => modeles_contrat::route(o, &ctx, route, &charge),
        _ if dans(documents::ROUTES) => documents::route(o, &ctx, route, &charge),
        _ if dans(depots::ROUTES) => depots::route(o, &ctx, route, &charge),
        _ if dans(imprevus::ROUTES) => imprevus::route(o, &ctx, route, &charge),
        // Les obligations d'un arret : le jour ou la compagnie l'a su, la declaration d'accident faite.
        _ if dans(crate::obligations::ROUTES) => crate::obligations::route(&ctx, route, &charge),
        _ if dans(fiches::ROUTES) => fiches::route(o, &ctx, route, &charge),
        _ if dans(identite::ROUTES) => identite::route(o, &ctx, route, &charge),
        _ if dans(metiers::ROUTES) => metiers::route(o, &ctx, route, &charge),
        _ if dans(bulletin::ROUTES) => bulletin::route(o, &ctx, route, &charge),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        _ if dans(crate::compagnie::organismes::ROUTES) => crate::compagnie::organismes::route(&ctx, route, &charge),
        _ if dans(coffre::ROUTES) => coffre::route(o, &ctx, route, &charge),
        _ if dans(filet::ROUTES) => filet::route(o, &ctx, route, &charge),
        _ if dans(import::ROUTES) => import::route(o, &ctx, route, &charge),
        _ if dans(archives::ROUTES) => archives::route(o, &ctx, route, &charge),
        _ if dans(libelles_courts::ROUTES) => libelles_courts::route(o, &ctx, route, &charge),
        _ if dans(historique::ROUTES) => historique::route(o, &ctx, route, &charge),
        // L'Historique d'un mois : ses documents, ses envois et leurs retours, lus (lecture seule).
        Route::HistoriqueMois => crate::historique_mois::route(&ctx, &charge),
        _ if dans(dossier_salarie::ROUTES) => dossier_salarie::route(o, &ctx, route, &charge),
        _ if dans(ouverture::ROUTES) => ouverture::route(o, &ctx, route, &charge),
        _ if dans(rappels::ROUTES) => rappels::route(o, &ctx, route, &charge),
        // Les conseils « Le saviez-vous ? » : le conseil du jour, ceux du dossier d'un salarie, le resume de l'accueil.
        _ if dans(crate::compagnie::conseils::ROUTES) => crate::compagnie::conseils::route(&ctx, route, &charge),
        // L'ouverture du logiciel et la veille : sauvegarder, relever les retours, les signatures et les fiches, sans
        // jamais echouer. ATTENTION : hors d'une compagnie fictive, cette route va au reseau (net-entreprises, Urssaf,
        // boite courriel, Drive, serveur des incidents).
        _ if dans(crate::compagnie::ouverture::ROUTES) => crate::compagnie::ouverture::route(o, &ctx, route, &charge),
        // Le banc : l'horloge de la compagnie fictive (rattraper ce qui etait du, ce que le bandeau « pendant votre
        // absence » lit ; `horloge::HorlogeFictive`).
        Route::HorlogeRattraper => crate::horloge::route(o, &ctx, &charge),
        autre => return Err(socle::echec::avec(catalogue::ROUTE_INCONNUE, json!({"route": autre})).into()),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    // Les mots de passe et les comptes (`routes::coffre`), le filet et les messages au developpeur (`routes::filet`) ne
    // dependent d'aucun de ces manques : leur refus se dit seul.
    match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && !dans(coffre::ROUTES) && !dans(filet::ROUTES) && classe != Some(crate::compagnie::bridage::CLASSE_REFUS_D_ENVOI) => {
            // Un refus qui porte une donnee a part (l'etat d'un mois, les anomalies d'une fiche, un chemin) se dit seul.
            let message = if ::documents::valeur::separer(&message).1.is_empty() { let suite = declarations::demarrage::a_la_suite(&ctx, &message).unwrap_or_default(); message + &suite } else { message };
            Err(Echec { classe, message })
        }
        autre => autre,
    }
}

/// LA REPONSE DE `moteur route`, le seul endroit ou ce qu'une route a rendu devient ce que l'ecran et le LLM lisent :
/// `{"ok": true, "resultat"}`, ou `{"ok": false, "erreur", "classe"}` et ce que le refus porte a part
/// (`socle::echec::avec` : un chemin, une cle, l'etat d'un mois, les anomalies d'une fiche). La phrase d'un refus se lit
/// a l'ecran : elle passe la verification de ce que l'ecran affiche (`refus_a_sa_place`).
fn reponse(racine: Option<&Path>, route: &str, rendu: Result<Value, Echec>) -> Value {
    match rendu {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(Echec { classe, message }) => {
            let (phrase, plus) = ::documents::valeur::separer(&message);
            if let Some(r) = racine {
                refus_a_sa_place(r, route, &phrase);
            }
            let mut m = Map::new();
            m.insert("ok".into(), Value::Bool(false));
            m.insert("erreur".into(), Value::String(phrase));
            if let Some(c) = classe {
                m.insert("classe".into(), json!(c));
            }
            for (k, v) in plus {
                m.entry(k).or_insert(v);
            }
            Value::Object(m)
        }
    }
}

/// UNE PHRASE DE REFUS QUI N'EST PAS FAITE POUR L'ECRAN (un chemin, un nom de fichier, un terme de code :
/// `affiche::defaut`) se lit quand meme, et note un incident silencieux au developpeur : la route et le defaut, jamais la
/// phrase. On la corrige a la cause : la phrase en mots, ce qui n'est pas pour l'ecran a part (`socle::echec::avec`).
fn refus_a_sa_place(racine: &Path, route: &str, phrase: &str) {
    let Some(d) = affiche::defaut(phrase, usize::MAX, false) else { return };
    let resume = format!("Un refus de la route {} n'est pas fait pour l'écran : {}.", route, d.corriger());
    let _ = crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
        nature: crate::compagnie::incidents::LOGICIEL,
        resume: &resume,
        ou: route,
        quoi: "PhrasePasFaitePourLEcran",
        detail: json!({"route": route, "defaut": format!("{:?}", d), "refus": true}),
        sans_contexte: true,
        ..Default::default()
    });
}

/// LA PLUS LONGUE DES PHRASES QUE REND UNE ROUTE, en caracteres : la sienne, et celle de chaque ligne qu'elle porte (la
/// `phrase` d'une carte, d'un carre, de la frise d'un engagement), a toute profondeur. Chacune s'affiche dans la place
/// de la route (`Fiche::phrase`).
pub fn plus_longue_phrase(v: &Value) -> usize {
    match v {
        Value::Object(o) => o
            .iter()
            .map(|(k, x)| match (k.as_str(), x) {
                ("phrase", Value::String(p)) => p.chars().count(),
                _ => plus_longue_phrase(x),
            })
            .max()
            .unwrap_or(0),
        Value::Array(a) => a.iter().map(plus_longue_phrase).max().unwrap_or(0),
        _ => 0,
    }
}

/// LES PHRASES QUE REND UNE ROUTE, celles que l'ecran lit : la `phrase`, les `phrases` et les `aides` de la reponse et de chaque ligne
/// qu'elle porte, a toute profondeur.
pub fn phrases_rendues(v: &Value) -> Vec<&str> {
    let mut out = Vec::new();
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                match (k.as_str(), x) {
                    ("phrase", Value::String(p)) => out.push(p.as_str()),
                    ("phrases", Value::Array(l)) => out.extend(l.iter().filter_map(Value::as_str)),
                    ("aides", Value::Object(a)) => out.extend(a.values().filter_map(Value::as_str)),
                    _ => out.extend(phrases_rendues(x)),
                }
            }
        }
        Value::Array(a) => out.extend(a.iter().flat_map(phrases_rendues)),
        _ => {}
    }
    out
}

/// UNE PHRASE PLUS LONGUE QUE SA PLACE (la `phrase` de la fiche ; `plus_longue_phrase`) se lit entiere, et note un incident silencieux au
/// developpeur : la route, la place, la longueur ; jamais la phrase, qui peut nommer un salarie. On la corrige a la cause,
/// en l'ecrivant plus breve.
fn phrase_a_sa_place(racine: &Path, route: Route, rendu: &Value) {
    // UNE REPONSE SANS PHRASE d'une route du catalogue (un avertissement, une variante que l'exemple de sa fiche ne joue
    // pas) : ni l'ecran ni `moteur route --texte` n'ont rien a lire. Le meme incident silencieux, pour la reponse qui l'a
    // oubliee. (L'horloge du banc, hors catalogue, rend ses phrases en liste.)
    let au_catalogue = route.au_catalogue();
    if au_catalogue && !rendu.get("phrase").and_then(Value::as_str).is_some_and(|p| !p.trim().is_empty()) {
        let resume = format!("La route {} a rendu une réponse sans phrase.", route);
        let cles: Vec<&str> = rendu.as_object().map(|o| o.keys().map(String::as_str).collect()).unwrap_or_default();
        let _ = crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
            nature: crate::compagnie::incidents::LOGICIEL,
            resume: &resume,
            ou: route.nom(),
            quoi: "PhraseManquante",
            detail: json!({"route": route, "cles": cles}),
            sans_contexte: true,
            ..Default::default()
        });
        return;
    }
    // UNE PHRASE QUI N'EST PAS FAITE POUR L'ECRAN (un nom de fichier, un terme de code : `affiche::defaut`) : le meme
    // incident silencieux, sans la phrase.
    if let Some(d) = phrases_rendues(rendu).into_iter().find_map(|p| affiche::defaut(p, usize::MAX, false)) {
        let resume = format!("Une phrase de la route {} n'est pas faite pour l'écran : {}.", route, d.corriger());
        let _ = crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
            nature: crate::compagnie::incidents::LOGICIEL,
            resume: &resume,
            ou: route.nom(),
            quoi: "PhrasePasFaitePourLEcran",
            detail: json!({"route": route, "defaut": format!("{:?}", d)}),
            sans_contexte: true,
            ..Default::default()
        });
    }
    let Some(place) = catalogue::catalogue().iter().find(|f| f.route == route).map(|f| f.phrase).filter(|p| *p > 0) else { return };
    let longueur = plus_longue_phrase(rendu);
    if longueur <= place {
        return;
    }
    let resume = format!("La phrase de la route {} passe sa place : {} caractères pour {}.", route, longueur, place);
    let _ = crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
        nature: crate::compagnie::incidents::LOGICIEL,
        resume: &resume,
        ou: route.nom(),
        quoi: "PhraseTropLongue",
        detail: json!({"route": route, "place": place, "longueur": longueur}),
        sans_contexte: true,
        ..Default::default()
    });
}

/// Les routes qui ecrivent ou engagent, d'apres l'acte de leur fiche : apres elles, la fenetre verifie les etats des mois
/// (`interface/api.rs`).
pub fn est_une_ecriture(route: Route) -> bool {
    catalogue::FICHES.iter().any(|l| catalogue::dans(l, route).is_some_and(|f| f.acte != catalogue::Acte::Lit))
}

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// QUI ECRIT : ce que les routes inscrivent, la fenetre comme la ligne de commande (`moteur route`, qui sert les memes).
pub(crate) const INTERFACE: &str = "interface";

/// LA CHARGE D'UNE ROUTE, lue une fois a son entree, en sa forme (`socle::forme::lire`) : une cle que la forme ne connait
/// pas, une cle qui manque, une valeur d'une autre forme la refusent (`TypeError`, `catalogue::FORME`), la cle fautive a
/// part ; rien ne s'ecrit.
pub(crate) fn lire_charge<T: serde::de::DeserializeOwned>(charge: &Value) -> R<T> {
    socle::forme::lire(charge, "La demande est mal formée.").map_err(|m| erreur("TypeError", m))
}

/// Une reponse en JSON : chacune est une structure de son module, que `serde_json` ecrit toujours.
pub(crate) fn en_json(v: impl serde::Serialize) -> R<Value> {
    serde_json::to_value(v).map_err(|e| Echec::from(e.to_string()))
}

/// La charge d'une route qui ne prend rien : `{}`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SansCharge {}

/// Le salarié que `nom` désigne (`Contexte::salarie`) ; refus (`KeyError`) avec la phrase de `Contexte::salarie`.
pub(crate) fn salarie(ctx: &Contexte, nom: &Value) -> R<droit::fiche::FicheSalarie> {
    ctx.salarie(&cle_du_salarie(ctx, nom)?).map_err(|m| erreur("KeyError", m))
}

/// La cle de la fiche que `nom` designe (`Contexte::cle_du_salarie`) ; refus (`KeyError`) avec sa phrase, qui cite `nom`
/// tel que la charge l'ecrit.
pub(crate) fn cle_du_salarie(ctx: &Contexte, nom: &Value) -> R<droit::contexte::IdSalarie> {
    let texte = valeur_en_texte(nom);
    ctx.cle_du_salarie(&texte).map_err(|m| {
        let m = match nom {
            Value::String(_) => m,
            autre if m.starts_with("Salarie ") => m.replacen(&repr_texte(&texte), &valeur_ecrite(autre), 1),
            _ => m,
        };
        erreur("KeyError", m)
    })
}

/// LA COULEUR D'UN ETAT, la meme sur tout l'ecran : `vert` ce qui est fait, `orange` ce qui reste a faire et que la
/// compagnie peut faire (une signature attendue se relance), `rouge` ce qui est refuse, `gris` ce qui n'est qu'une
/// information (rien a faire). La page dessine la couleur qu'on lui donne ; elle n'en choisit aucune. Sa pastille :
/// `Pastille`.
pub(crate) use socle::ecran::{couleur_de_l_etat, Pastille};

/// UN ENCART, tel que la page le dessine (`encart` de js/tuyau.js) : un titre en gras, sa phrase, la couleur de l'etat.
/// La page n'ecrit aucune phrase : ce qu'une boite dit d'un cas, la route le rend ainsi.
pub(crate) fn encart(etat: &str, titre: impl Into<String>, phrase: impl Into<String>) -> Value {
    json!(Encart::de(etat, titre, phrase))
}

/// UN ENCART, tel que la page le dessine (`encart` de js/tuyau.js) : un titre en gras, sa phrase, la couleur de l'etat.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Encart {
    pub couleur: &'static str,
    pub titre: String,
    pub phrase: String,
}

impl Encart {
    pub(crate) fn de(etat: &str, titre: impl Into<String>, phrase: impl Into<String>) -> Encart {
        Encart { couleur: couleur_de_l_etat(etat), titre: titre.into(), phrase: phrase.into() }
    }

    /// L'encart en une ligne pour `moteur route --texte` : « titre : phrase ».
    pub(crate) fn en_ligne(&self) -> String {
        [self.titre.as_str(), self.phrase.as_str()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" : ")
    }
}

/// `Pastille::de`, en JSON.
pub(crate) fn pastille(etat: &str, mots: &str) -> Value {
    json!(Pastille::de(etat, mots))
}

/// « 12/09/2026 à 18:18 » : un horodatage ISO tel que l'ecran le dit ; ce qui n'en est pas un, tel quel.
pub(crate) fn quand(iso: &str) -> String {
    let b = iso.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return iso.to_string();
    }
    let jour = format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4]);
    match iso.get(11..16) {
        Some(h) => format!("{} à {}", jour, h),
        None => jour,
    }
}

/// LE NOM D'UN SALARIE TEL QU'UN ECRAN LE MONTRE : le nom de famille en capitales, sous sa forme lisible (`nom_affichage`,
/// accents compris : « SÉRAPHIN »), puis le prenom lisible. La forme declaree (`nom`, sans accents) reste celle de la DSN
/// et de la DPAE ; une fiche qui n'a que celle-la la montre.
pub(crate) fn nom_a_l_ecran(sal: &droit::fiche::ChampsSalarie) -> String {
    sal.nomme().unwrap_or_default()
}

/// `sortie_json.brut` d'une valeur lue d'un fichier : un dictionnaire perd ses
/// cles qui commencent par `_`.
pub(crate) fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, socle::echec::du_disque(p, e))
}

/// Un fichier lu en UTF-8.
fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", socle::echec::du_disque(&p, &e)))
}

pub(crate) fn vers_route(x: crate::compagnie::coffre::Echec) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// LA PHRASE D'UNE ROUTE, en tete de ce qu'elle rend : celle que l'ecran lit, et que `moteur route --texte` imprime
/// (`service::en_texte`). Chaque route en rend une ; une phrase deja posee par la route est remplacee.
pub(crate) fn phrase_en_tete(v: Value, phrase: impl Into<String>) -> Value {
    let mut m = Map::new();
    m.insert("phrase".into(), Value::String(phrase.into()));
    if let Value::Object(o) = v {
        m.extend(o.into_iter().filter(|(k, _)| k != "phrase"));
    }
    Value::Object(m)
}

/// « 1 salarié », « 3 salariés » : `n` et le nom, au pluriel quand il le faut (un `s` de plus, ou `pluriel`).
pub(crate) fn compte(n: usize, un: &str, pluriel: &str) -> String {
    format!("{} {}", n, if n > 1 { pluriel } else { un })
}

/// Le nombre d'elements d'une liste rendue (0 : pas une liste).
pub(crate) fn nombre(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

/// Une valeur rendue, en mots dans une phrase : rien pour une valeur absente (jamais « None »).
pub(crate) fn mot(v: &Value) -> String {
    if v.is_null() { String::new() } else { valeur_en_texte(v) }
}

/// Un jour rendu (`2026-11-02`), tel qu'une phrase le dit : « 02/11/2026 » ; ce qui n'en est pas un, tel quel.
pub(crate) fn jour_en_clair(v: &Value) -> String {
    let t = mot(v);
    socle::date::Jour::lire(&t).map(|j| j.jj_mm_aaaa()).unwrap_or(t)
}

/// « du 02/11/2026 au 30/11/2026 », « depuis le 02/11/2026 » sans fin : une periode telle qu'une phrase la dit.
pub(crate) fn periode_en_clair(debut: &Value, fin: &Value) -> String {
    if vrai(fin) {
        format!("du {} au {}", jour_en_clair(debut), jour_en_clair(fin))
    } else {
        format!("depuis le {}", jour_en_clair(debut))
    }
}
