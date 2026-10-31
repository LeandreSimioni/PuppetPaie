//! `/api/<route>` : ce que la page demande, rendu en `{"ok", "resultat"}` (ou
//! `{"ok": false, "erreur": {"type", "message", ...}}`).
//!
//! LES ROUTES TOURNENT DANS LE PROCESSUS DE LA FENETRE :
//! `route`, et pourquoi rien ne reste a part. Rien d'autre ne passe ici que les routes du catalogue : les gestes du poste
//! (ouvrir, choisir un dossier) et la veille sont ceux de la fenetre (`poste.rs`, `Fenetre::lanceur`).

use std::path::Path;

use serde_json::{json, Map, Value};

use super::veille::Veille;
use crate::etat_garde::FileDesEtats;

/// Les refus qui ne sont pas des defauts du logiciel : une saisie en cours, un mot de passe mal tape, un mois sans
/// paie, et ceux que la page traite elle-meme (une fiche refusee, un depot deja fait...). On ne les compte pas en incidents.
const PAS_DES_INCIDENTS: [&str; 7] = [
    "SaisieIncomplete",
    "PreuveInsuffisante",
    "AucunDocument",
    "FicheRefusee",
    "DsnDejaDeposee",
    "RepetitionManquante",
    "ModificationNonDeclarable",
];

/// Un refus de route : sa classe (le `type` que la page lit), sa phrase, et ce qu'il porte pour la page.
pub struct Refus {
    pub classe: String,
    pub message: String,
    pub plus: Map<String, Value>,
}

impl Refus {
    pub fn simple(classe: &str, message: impl Into<String>) -> Refus {
        Refus { classe: classe.into(), message: message.into(), plus: Map::new() }
    }
}

/// Une route du moteur, servie ICI, dans le processus de la fenetre :
/// sous Windows, un processus par route coutait 120 a 260 ms de plus a chaque appel (l'antivirus inspecte chaque
/// lancement et chaque lecture d'un processus neuf), et les baremes, referentiels et catalogues se relisaient a chaque
/// fois ; ici ils restent en memoire tant que leur fichier ne change pas (`fichiers::json_en_cache` et les caches qui
/// s'en servent). Rend son `resultat`, ou son refus.
///
/// Rien ne reste dans un processus a part. Ce que le processus separe protegeait est tenu ici : une panique rend un
/// refus et la fenetre reste (`routes::dans_la_fenetre`, `panic = "unwind"` en `release`) ; le mot de passe tape est
/// oublie a la fin de sa route, quoi qu'il arrive ; la compagnie vient de `racine`, jamais d'une variable du processus
/// (deux compagnies peuvent etre servies en meme temps par deux fils) ; les sorties reseau passent par `garde_reseau`
/// aussi. Seul `maj::controler` lance encore un processus : il eprouve le NOUVEAU programme, qui n'est pas celui-ci.
pub fn route(racine: &Path, nom: &str, charge: &Value) -> Result<Value, Refus> {
    // LE FIL (`diagnostic`) : une route que la page n'a pas demandee (la veille, le filet, le fond) s'y note ici ; celle de
    // la page se note dans `repondre`, avec ce que la page a fait avant.
    if DANS_LA_PAGE.with(std::cell::Cell::get) {
        return route_seule(racine, nom, charge);
    }
    let debut = std::time::Instant::now();
    let r = route_seule(racine, nom, charge);
    let issue = r.as_ref().map(|_| ()).map_err(|e| (e.classe.as_str(), e.message.as_str()));
    crate::compagnie::diagnostic::noter_route(racine, "fond", nom, charge, debut.elapsed(), issue);
    r
}

thread_local! {
    /// Vrai le temps d'une requete de la page (`repondre`) : ses routes internes ne se notent pas une a une dans le fil.
    static DANS_LA_PAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn route_seule(racine: &Path, nom: &str, charge: &Value) -> Result<Value, Refus> {
    let mut o = std::collections::HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    // LE BANC D'ESSAI (l'atelier seulement) : chaque route que la fenetre sert -- la page, la veille, le fil de fond --
    // entre dans le scenario, avec sa charge et son instant (`scenario::Enregistreur`).
    #[cfg(feature = "atelier")]
    if let Some(e) = crate::atelier::scenario::enregistreur_actif() {
        return depuis_reponse(e.route(racine, nom, charge, || crate::routes::dans_la_fenetre(&o, nom, charge)));
    }
    depuis_reponse(crate::routes::dans_la_fenetre(&o, nom, charge))
}

/// Une reponse de `moteur route` (`{"ok", "resultat"}` ou `{"ok": false, "erreur", "classe", ...}`) : son resultat, ou son refus.
pub fn depuis_reponse(rep: Value) -> Result<Value, Refus> {
    if rep.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(rep.get("resultat").cloned().unwrap_or(Value::Null));
    }
    let message = match rep.get("erreur") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "refus sans phrase".into(),
    };
    let classe = rep.get("classe").and_then(Value::as_str).unwrap_or("CalculRefuse").to_string();
    let mut plus = Map::new();
    if let Value::Object(m) = &rep {
        for (k, v) in m {
            if !["ok", "erreur", "classe"].contains(&k.as_str()) {
                plus.insert(k.clone(), v.clone());
            }
        }
    }
    Err(Refus { classe, message, plus })
}

fn texte(charge: &Value, cle: &str) -> Result<String, Refus> {
    charge.get(cle).and_then(Value::as_str).map(String::from).ok_or_else(|| Refus::simple("KeyError", socle::fichiers::absent(cle)))
}

/// « Un probleme, une idee ? » part en fond : `signalement-envoyer` a enregistre le message et
/// rendu la main ; `signalements-partir` le fait partir, le temps du serveur de l'editeur, sans que la page attende.
fn signalements_en_fond(racine: &Path) {
    let r = racine.to_path_buf();
    crate::routes::fil_de_routes(move || {
        let _ = route(&r, "signalements-partir", &json!({}));
    });
}

fn traiter(racine: &Path, nom: &str, charge: &Value, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Result<Value, Refus> {
    match nom {
        // `xlsx` et `pdf` : deux faits du poste, que le moteur constate lui-meme.
        "exports-periodes" => {
            let mut c = charge.as_object().cloned().unwrap_or_default();
            c.insert("xlsx".into(), Value::Bool(true));
            c.insert("pdf".into(), Value::Bool(true));
            route(racine, nom, &Value::Object(c))
        }
        "signalement-envoyer" => {
            let r = route(racine, nom, charge);
            if r.is_ok() {
                signalements_en_fond(racine);
            }
            r
        }
        "rappels" => route(racine, nom, &json!({})),
        // L'ECRAN D'UN MOIS SE LIT ICI, SANS PROCESSUS : le dernier etat connu du
        // mois, tout de suite, meme perime (marque `perime`, et refait en fond) ; `attendre` : l'etat frais.
        "mois" => {
            let mois = texte(charge, "mois")?;
            match etats {
                Some(f) => f
                    .ecran(&mois, charge.get("attendre").and_then(Value::as_bool) == Some(true))
                    .map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message)),
                None => route(racine, nom, charge),
            }
        }
        // L'OUVERTURE D'UNE COMPAGNIE, D'UN COUP ET SANS PROCESSUS : le dernier etat connu de ce que la page lit
        // en s'ouvrant (`etat_garde::FileDesEtats::ouverture`), et, lus a chaque fois a cote, ce qui vit dans la memoire de
        // la fenetre (la veille, la mise a jour) et les signalements (que l'etat garde ne suit pas).
        "ouverture" => {
            let Some(f) = etats else {
                return Err(Refus::simple("ErreurOperation", "Pas d'ouverture sans la fenetre de PuppetPaie."));
            };
            let frais = charge.get("frais").and_then(Value::as_bool) == Some(true);
            let patienter = std::time::Duration::from_millis(charge.get("patienter").and_then(Value::as_u64).unwrap_or(400).min(5000));
            // La veille se lit pendant que l'etat garde se lit (elle demande au coffre ce qui est pose).
            let (rendu, etat_veille) = std::thread::scope(|s| {
                let v = s.spawn(|| (!frais).then(|| veille.map(Veille::etat)).flatten());
                let r = f.ouverture(patienter, frais);
                (r, v.join().ok().flatten())
            });
            let mut v = rendu.map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message))?;
            if v.get("pret").and_then(Value::as_bool) == Some(true) && !frais {
                v["veille"] = etat_veille.unwrap_or_else(|| json!({"minutes": 0, "dernier": {}, "en_cours": false}));
                v["maj"] = crate::compagnie::maj::RECHERCHE.etat();
                v["signalements"] = en_page(
                    crate::routes::filet::signalements_route(racine).map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message)),
                );
            }
            Ok(v)
        }
        _ => route(racine, nom, charge),
    }
}

/// La reponse a `/api/<nom>`, au format que la page attend.
pub fn repondre(racine: &Path, nom: &str, charge: &Value, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Value {
    let debut = std::time::Instant::now();
    let debut_ms = socle::temps::maintenant_ms();
    let avant = DANS_LA_PAGE.with(|d| d.replace(true));
    let rendu = traiter(racine, nom, charge, veille, etats);
    DANS_LA_PAGE.with(|d| d.set(avant));
    let duree = debut.elapsed();
    // LE FIL : la route de la page, sa charge mise en forme, sa duree, son issue. Une erreur de la page (`incident-navigateur`)
    // s'y dit comme telle.
    if nom == "incident-navigateur" {
        let c = |k: &str| charge.get(k).map(socle::valeur::valeur_en_texte).unwrap_or_default();
        crate::compagnie::diagnostic::noter_moteur(&format!(
            "erreur JS {} : {} ({}:{})",
            c("genre"),
            crate::compagnie::diagnostic::texte_en_forme(Some(racine), &c("message"), 200),
            crate::compagnie::diagnostic::texte_en_forme(Some(racine), &c("fichier"), 80),
            c("ligne")
        ));
    } else {
        let issue = rendu.as_ref().map(|_| ()).map_err(|e| (e.classe.as_str(), e.message.as_str()));
        crate::compagnie::diagnostic::noter_route(racine, "page", nom, charge, duree, issue);
    }
    // APRES UNE ECRITURE, les etats de tous les mois se verifient en fond, le mois affiche
    // d'abord : une ecriture d'un mois peut changer les suivants (un contrat en cours, une numerotation, un cumul). Meme
    // refusee, une ecriture a pu toucher un fichier ; un mois deja juste ne coute que son empreinte. La reponse le dit
    // (`ecrit`) : la page oublie alors ce qu'elle a lu.
    let ecrit = crate::routes::est_une_ecriture(nom);
    if let Some(f) = etats {
        if ecrit {
            f.tout_verifier();
        }
    }
    let mut reponse = match rendu {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(r) => {
            if !PAS_DES_INCIDENTS.contains(&r.classe.as_str()) && nom != "incident-navigateur" {
                // DE QUOI REJOUER LA ROUTE : sa charge mise en forme,
                // sa duree, l'etat du mois qu'elle touche, ce que le refus portait pour la page ; et la trace -- l'endroit du
                // code et la chaine des echecs. Le fil et la machine s'y ajoutent dans `incidents::noter`.
                let mut detail = Map::new();
                detail.insert("route".into(), json!(nom));
                detail.insert("charge".into(), crate::compagnie::diagnostic::en_forme(Some(racine), charge));
                detail.insert("duree_ms".into(), json!(duree.as_millis() as u64));
                if let Some(m) = charge.get("mois").and_then(Value::as_str) {
                    detail.insert("mois".into(), crate::compagnie::diagnostic::etat_du_mois(racine, m));
                }
                if !r.plus.is_empty() {
                    detail.insert("refus".into(), crate::compagnie::diagnostic::en_forme(Some(racine), &Value::Object(r.plus.clone())));
                }
                let trace = crate::compagnie::diagnostic::trace_du_refus(racine, &r.message, debut_ms);
                let _ = crate::compagnie::incidents::depuis_exception(racine, &r.classe, &r.message, None, nom, Value::Object(detail), trace.as_deref());
            }
            en_page(Err(r))
        }
    };
    if ecrit {
        reponse["ecrit"] = json!(true);
    }
    reponse
}

/// UNE ROUTE DE L'INSTALLATION (`routes::accueil` : l'accueil des compagnies), servie sans compagnie : l'installation est
/// celle du programme. La reponse au format que la page attend.
pub fn route_de_l_installation(nom: &str, charge: &Value) -> Value {
    let o = std::collections::HashMap::new();
    // LE BANC D'ESSAI (l'atelier seulement) : une compagnie ajoutee entre dans le scenario comme le decor de depart que le
    // rejeu refait (`scenario::rejouer`, le geste `ajouter`).
    #[cfg(feature = "atelier")]
    if nom == "compagnie-ajouter" {
        if let Some(e) = crate::atelier::scenario::enregistreur_actif() {
            let rendu = e.geste("ajouter", || {
                let v = depuis_reponse(crate::routes::dans_la_fenetre(&o, nom, charge)).map_err(|r| r.message)?;
                let mut m = Map::new();
                m.insert("nom".into(), charge.get("nom").cloned().unwrap_or(Value::Null));
                if let Some(venue) = charge.get("venue").filter(|v| v.as_str() != Some("neuve")) {
                    m.insert("venue".into(), venue.clone());
                }
                let racine = std::path::PathBuf::from(v.get("chemin").and_then(Value::as_str).unwrap_or_default());
                m.insert("compagnie".into(), json!(e.compagnie(&racine)));
                Ok((v, m))
            });
            return en_page(rendu.map_err(|m| Refus::simple("ErreurOperation", m)));
        }
    }
    en_page(depuis_reponse(crate::routes::dans_la_fenetre(&o, nom, charge)))
}

/// Un resultat ou un refus, au format que la page attend (`{"ok", "resultat"}` ou `{"ok": false, "erreur": {"type", "message", ...}}`).
pub fn en_page(r: Result<Value, Refus>) -> Value {
    match r {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(r) => {
            let mut e = Map::new();
            e.insert("type".into(), Value::String(r.classe));
            e.insert("message".into(), Value::String(r.message));
            for (k, v) in r.plus {
                e.insert(k, v);
            }
            json!({"ok": false, "erreur": e})
        }
    }
}

