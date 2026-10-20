//! La veille de la fenetre (`web.py`, classe `Veille`) : relever les retours, les signatures et les fiches, puis
//! sauvegarder, a l'ouverture puis toutes les `minutes`. En arriere-plan, jamais dans une requete ; un tour a la fois.
//! Elle ne depose rien : elle appelle la route `ouverture`, qui ne connait que la releve et la sauvegarde.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::api::route;
use crate::etat_garde::FileDesEtats;

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
            etats,
        })
    }

    fn derniere(&self) -> Value {
        route(&self.racine, "ouverture-derniere", &json!({})).unwrap_or_else(|_| json!({}))
    }

    /// Ou en est le passage, sans rien lancer (`veille-etape`) : une lecture de drapeaux et d'un petit fichier, que la page
    /// fait toutes les secondes le temps d'un passage. L'etape est celle que l'ouverture ecrit en y entrant
    /// (`ouverture::etape_en_cours`) ; aucun passage en cours, aucune etape.
    pub fn etape(&self) -> Value {
        let en_cours = self.en_cours.load(Ordering::SeqCst);
        let sauvegarde = self.sauvegarde_en_cours.load(Ordering::SeqCst);
        let etape = if en_cours || sauvegarde { crate::ouverture::etape_en_cours(&self.racine) } else { None };
        json!({"en_cours": en_cours, "sauvegarde_en_cours": sauvegarde, "etape": etape})
    }

    pub fn etat(&self) -> Value {
        let coffre = route(&self.racine, "coffre-etat", &json!({})).unwrap_or_else(|_| json!({}));
        let dernier = self.dernier.lock().ok().and_then(|d| d.clone()).unwrap_or_else(|| self.derniere());
        json!({
            "en_cours": self.en_cours.load(Ordering::SeqCst),
            "sauvegarde_en_cours": self.sauvegarde_en_cours.load(Ordering::SeqCst),
            "etape": self.etape()["etape"],
            "minutes": self.minutes,
            "dernier": dernier,
            "erreur": self.erreur.lock().map(|e| e.clone()).unwrap_or_default(),
            "sauvegarde": route(&self.racine, "sauvegarde-derniere", &json!({})).unwrap_or(Value::Null),
            "coffre": coffre.get("services").cloned().unwrap_or_else(|| json!([])),
            "mot_de_passe_pose": coffre.get("mot_de_passe_pose").cloned().unwrap_or(Value::Bool(false)),
        })
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
        match route(&self.racine, "ouverture", &charge) {
            Ok(rep) => {
                *self.dernier.lock().unwrap_or_else(|p| p.into_inner()) = Some(self.derniere());
                self.en_cours.store(false, Ordering::SeqCst);
                if sauvegarder && rep.get("retenue").and_then(Value::as_bool) != Some(true) {
                    self.sauvegarde_en_cours.store(true, Ordering::SeqCst);
                    let mut s = base;
                    s["sauvegarder"] = Value::Bool(true);
                    s["sauvegarde_seule"] = Value::Bool(true);
                    s["incidents_editeur"] = Value::Bool(false);
                    if let Err(r) = route(&self.racine, "ouverture", &s) {
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
        std::thread::spawn(move || {
            // La mise a jour ne se cherche plus ici, a l'ouverture d'une compagnie : c'est le logiciel qui la cherche pour
            // lui-meme, au lancement (`maj::chercher_en_fond`, lot U1).
            v.tour(false, true);
            while v.minutes > 0 {
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
}
