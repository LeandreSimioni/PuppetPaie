//! LES SUITES D'UN ENVOI : ce qu'un envoi releve en chemin et que `moteur` fait ensuite, parce que cela touche a la
//! compagnie et non au transport. Un envoi les remplit au fil de l'eau, meme quand il echoue ; l'appelant les fait
//! toutes, que l'envoi ait abouti ou non.

use std::path::PathBuf;

use serde_json::Value;

/// Un depot refuse, a noter au journal des incidents (test comme reel).
#[derive(Clone, Debug)]
pub struct DepotRefuse {
    /// `dsn` ou `dpae`.
    pub quoi: String,
    pub reel: bool,
    pub classe: String,
    pub message: String,
    pub repetition: Value,
}

/// Ce qu'un depot reel accepte laisse derriere lui hors du journal des depots.
#[derive(Clone, Debug)]
pub struct DepotReel {
    /// `dsn` ou `dpae`.
    pub quoi: String,
    pub mois: String,
    /// Le fichier depose.
    pub chemin: PathBuf,
    /// L'heure de la ligne du journal.
    pub quand: String,
    /// Le depot a-t-il ete joue par le simulateur ?
    pub joue: bool,
    /// Un signalement (DSN de nature 04, 05, 07, 08) : sa nature, son ordre, son type, son evenement.
    pub signalement: Option<Signalement>,
    /// Les embauches d'une DPAE (`salarie`, `debut`, `fin`).
    pub embauches: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct Signalement {
    pub nature: String,
    pub ordre: String,
    pub type_declaration: String,
    pub evenement: String,
}

/// Les suites d'un envoi.
#[derive(Default, Debug)]
pub struct Suites {
    /// Les incidents que le rattachement des retours a releves (`etats::AvecIncidents`).
    pub incidents_des_retours: Vec<Value>,
    /// Les attestations France Travail archivees, a ranger.
    pub aer: Vec<PathBuf>,
    /// Un depot refuse.
    pub depot_refuse: Option<DepotRefuse>,
    /// Un depot reel accepte.
    pub depot_reel: Option<DepotReel>,
}

impl Suites {
    /// Le resultat d'un `AvecIncidents`, ses incidents gardes pour l'appelant.
    pub fn garder<T>(&mut self, r: crate::etats::AvecIncidents<T>) -> socle::echec::R<T> {
        self.incidents_des_retours.extend(r.incidents);
        r.resultat
    }
}
