//! La veille de la fenetre : relever les retours, les signatures et les fiches, puis
//! sauvegarder, a l'ouverture puis toutes les `minutes`. En arriere-plan, jamais dans une requete ; un tour a la fois.
//! Elle ne depose rien : elle appelle la route `ouverture`, qui ne connait que la releve et la sauvegarde.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::api::route;
use crate::etat_garde::FileDesEtats;
use crate::routes::catalogue::Route;

pub struct Veille {
    racine: PathBuf,
    pub minutes: u64,
    jours: u32,
    dernier: Mutex<Option<Value>>,
    erreur: Mutex<String>,
    en_cours: AtomicBool,
    sauvegarde_en_cours: AtomicBool,
    tour_unique: Mutex<()>,
    arretee: AtomicBool,
    /// Quand partira le prochain tour de la boucle (« prochain relevé dans … minutes »).
    prochain: Mutex<Option<std::time::SystemTime>>,
    /// Les etats des mois, a verifier apres chaque tour : un retour releve, une signature notee changent un mois.
    etats: Option<Arc<FileDesEtats>>,
}

impl Veille {
    pub fn nouvelle(racine: &Path, minutes: u64, etats: Option<Arc<FileDesEtats>>) -> Arc<Veille> {
        Arc::new(Veille {
            racine: racine.to_path_buf(),
            minutes,
            jours: 30,
            dernier: Mutex::new(None),
            erreur: Mutex::new(String::new()),
            en_cours: AtomicBool::new(false),
            sauvegarde_en_cours: AtomicBool::new(false),
            tour_unique: Mutex::new(()),
            arretee: AtomicBool::new(false),
            prochain: Mutex::new(None),
            etats,
        })
    }

    fn derniere(&self) -> Value {
        crate::compagnie::ouverture::derniere_ouverture(&self.racine)
    }

    /// Ou en est le passage, sans rien lancer (`veille-etape`) : une lecture de drapeaux et d'un petit fichier, que la page
    /// fait toutes les secondes le temps d'un passage. L'etape est celle que l'ouverture ecrit en y entrant
    /// (`ouverture::etape_en_cours`) ; aucun passage en cours, aucune etape.
    pub fn etape(&self) -> Value {
        let en_cours = self.en_cours.load(Ordering::SeqCst);
        let sauvegarde = self.sauvegarde_en_cours.load(Ordering::SeqCst);
        let etape = if en_cours || sauvegarde { crate::compagnie::ouverture::etape_en_cours(&self.racine) } else { None };
        json!({"en_cours": en_cours, "sauvegarde_en_cours": sauvegarde, "etape": etape})
    }

    pub fn etat(&self) -> Value {
        let coffre = route(&self.racine, Route::CoffreEtat, &json!({})).unwrap_or_else(|_| json!({}));
        let dernier = self.dernier.lock().ok().and_then(|d| d.clone()).unwrap_or_else(|| self.derniere());
        let prochain = self.prochain_dans();
        let fictive = crate::compagnie::bridage::branchement(&self.racine).joue();
        json!({
            "dit": dit_du_bandeau(&dernier, &prochain, fictive),
            "en_cours": self.en_cours.load(Ordering::SeqCst),
            "sauvegarde_en_cours": self.sauvegarde_en_cours.load(Ordering::SeqCst),
            "etape": self.etape()["etape"],
            "minutes": self.minutes,
            "prochain_dans": prochain,
            "dernier": dernier,
            "erreur": self.erreur.lock().map(|e| e.clone()).unwrap_or_default(),
            "sauvegarde": crate::compagnie::sauvegarde::derniere(&self.racine).en_json(),
            "coffre": coffre.get("services").cloned().unwrap_or_else(|| json!([])),
            "mot_de_passe_pose": coffre.get("mot_de_passe_pose").cloned().unwrap_or(Value::Bool(false)),
        })
    }

    /// Dans combien de minutes part le prochain tour, arrondi a la minute superieure (au moins 1) ; `null` sans boucle.
    fn prochain_dans(&self) -> Value {
        let p = *self.prochain.lock().unwrap_or_else(|p| p.into_inner());
        match p {
            Some(t) if self.minutes > 0 => {
                let s = t.duration_since(std::time::SystemTime::now()).map(|d| d.as_secs()).unwrap_or(0);
                json!(s.div_ceil(60).max(1))
            }
            _ => Value::Null,
        }
    }

    /// Un passage : la releve d'abord (qui ecrit la trace, et envoie les incidents a l'editeur), puis la sauvegarde seule.
    pub fn tour(&self, veille: bool, sauvegarder: bool) -> Value {
        let Ok(_un_seul) = self.tour_unique.try_lock() else { return self.etat() };
        self.en_cours.store(true, Ordering::SeqCst);
        if let Ok(mut e) = self.erreur.lock() {
            e.clear();
        }
        let base = json!({"jours": self.jours, "veille": veille, "incidents_editeur": true});
        let mut charge = base.clone();
        charge["sauvegarder"] = Value::Bool(false);
        match route(&self.racine, Route::Ouverture, &charge) {
            Ok(rep) => {
                *self.dernier.lock().unwrap_or_else(|p| p.into_inner()) = Some(self.derniere());
                self.en_cours.store(false, Ordering::SeqCst);
                if sauvegarder && rep.get("retenue").and_then(Value::as_bool) != Some(true) {
                    self.sauvegarde_en_cours.store(true, Ordering::SeqCst);
                    let mut s = base;
                    s["sauvegarder"] = Value::Bool(true);
                    s["sauvegarde_seule"] = Value::Bool(true);
                    s["incidents_editeur"] = Value::Bool(false);
                    if let Err(r) = route(&self.racine, Route::Ouverture, &s) {
                        *self.erreur.lock().unwrap_or_else(|p| p.into_inner()) = r.message;
                    }
                }
                *self.dernier.lock().unwrap_or_else(|p| p.into_inner()) = Some(self.derniere());
            }
            Err(r) => *self.erreur.lock().unwrap_or_else(|p| p.into_inner()) = r.message,
        }
        self.en_cours.store(false, Ordering::SeqCst);
        self.sauvegarde_en_cours.store(false, Ordering::SeqCst);
        if let Some(f) = &self.etats {
            f.tout_verifier();
        }
        self.etat()
    }

    /// Le premier tour, complet, puis un tour toutes les `minutes` tant que la veille n'est pas arretee.
    pub fn demarrer(self: &Arc<Self>) {
        let v = self.clone();
        crate::routes::fil_de_routes(move || {
            // La mise a jour ne se cherche pas ici, a l'ouverture d'une compagnie : c'est le logiciel qui la cherche pour
            // lui-meme, au lancement (`maj::chercher_en_fond`).
            v.tour(false, true);
            while v.minutes > 0 {
                *v.prochain.lock().unwrap_or_else(|p| p.into_inner()) = Some(std::time::SystemTime::now() + Duration::from_secs(v.minutes * 60));
                for _ in 0..v.minutes * 60 {
                    if v.arretee.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                v.tour(true, true);
            }
        });
    }

    pub fn arreter(&self) {
        self.arretee.store(true, Ordering::SeqCst);
    }

    /// Un passage tourne-t-il (la releve, ou la sauvegarde) ?
    pub fn occupee(&self) -> bool {
        self.en_cours.load(Ordering::SeqCst) || self.sauvegarde_en_cours.load(Ordering::SeqCst)
    }

    /// LA COMPAGNIE S'EN VA (`interface::suppression`) : plus de tour, et celui qui aurait commence se finit avant qu'on
    /// rende la main -- rien ne s'ecrit plus dans son dossier ensuite.
    pub fn arreter_et_attendre(&self) {
        self.arreter();
        let _fini = self.tour_unique.lock().unwrap_or_else(|p| p.into_inner());
    }
}

/// « à 16:28 » le jour du passage, « le 23/09 à 16:28 » un autre jour : l'heure d'un passage (`JJ/MM/AAAA HH:MM`, ou
/// ISO), au jour de l'horloge de Paris (celle d'une fictive au banc comprise).
fn heure_du_passage(quand: &str) -> String {
    let c: Vec<&str> = quand.split(|x: char| !x.is_ascii_digit()).filter(|x| !x.is_empty()).collect();
    let (an, mo, jo, h, mi) = match c.as_slice() {
        [a, m, j, h, mi, ..] if a.len() == 4 => (*a, *m, *j, *h, *mi),
        [j, m, a, h, mi, ..] if a.len() == 4 => (*a, *m, *j, *h, *mi),
        _ => return if quand.is_empty() { String::new() } else { format!("le {}", quand) },
    };
    let aujourdhui = socle::paris::maintenant();
    if aujourdhui.get(..10) == Some(&format!("{}-{}-{}", an, mo, jo)) {
        format!("à {}:{}", h, mi)
    } else {
        format!("le {}/{} à {}:{}", jo, mo, h, mi)
    }
}

/// CE QUE LE BANDEAU DIT (`dit` de l'etat de la veille) : l'etat en une phrase, un service muet, ce qu'il y a a poser et
/// ou, les retours nouveaux, ce que l'horloge d'une fictive a fait, et ce que dit un passage en cours ou fini. La page
/// n'en ecrit rien.
fn dit_du_bandeau(d: &Value, prochain: &Value, fictive: bool) -> Value {
    let mot = crate::routes::mot;
    let quand = heure_du_passage(&mot(&d["quand"]));
    let hors_ligne = d["hors_ligne"].as_bool() == Some(true);
    let retours = !mot(&d["retours"]).is_empty();
    let dans = prochain.as_u64();
    let etat = if hors_ligne && !retours {
        format!("Pas de réponse au relevé {} : vérifier la connexion{}", quand, dans.map(|n| format!(" ; nouvel essai dans {} min", n)).unwrap_or_default())
    } else {
        format!("À jour, relevé {}{}", quand, dans.map(|n| format!(" · prochain relevé dans {} min", n)).unwrap_or_default())
    };
    let n_muets = d["injoignables"].as_array().map_or(0, Vec::len);
    let muets = if hors_ligne && retours {
        json!(format!("{} (détail au survol) : nouvel essai au prochain relevé", if n_muets > 1 { format!("{} services n'ont pas répondu", n_muets) } else { "un service n'a pas répondu".to_string() }))
    } else {
        Value::Null
    };
    let nouveaux = d["nouveaux"].as_u64().unwrap_or(0);
    let horloge = d["horloge"].as_array().map_or(0, Vec::len);
    json!({
        "etat": etat,
        "muets": muets,
        "a_poser": {
            "net-entreprises": ["Les retours de DSN ne se relèvent pas seuls : le mot de passe net-entreprises n'est pas posé.", "Poser le mot de passe"],
            "urssaf": ["Les retours de DPAE ne se relèvent pas seuls : le mot de passe Urssaf (DPAE) n'est pas posé.", "Poser le mot de passe"],
            "google": ["La sauvegarde ne part pas : aucun Drive n'est lié.", "Lier le Drive"],
        },
        "retours": if nouveaux > 0 { json!(format!("{} retour{} nouveau{} : les lire dans l'Historique", nouveaux, if nouveaux > 1 { "s" } else { "" }, if nouveaux > 1 { "x" } else { "" })) } else { Value::Null },
        "fictive": if fictive { json!("compagnie fictive : pas de sauvegarde, pourquoi ?") } else { Value::Null },
        "horloge": if fictive && horloge > 0 { json!(format!("pendant votre absence, la compagnie a travaillé ({}) : voir", horloge)) } else { Value::Null },
        "horloge_titre": "Pendant votre absence",
        "horloge_intro": "L'administrateur fictif a fait ce qui tombait, chacun à sa date limite : le 1er du mois, le mois s'écrit ; le jour d'une embauche, sa DPAE part ; à l'échéance, la DSN. Tout est joué, rien n'a quitté le poste, et le journal de chaque mois le montre.",
        "en_cours": "Relevé des retours…",
        "sauvegarde": "Sauvegarde…",
        "termine": "Relevé terminé.",
    })
}
