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
    /// Ce qui a ouvert le depot reel (`None` pour un test).
    pub repetition: Option<crate::journal::Ouverture>,
}

/// Les suites d'un envoi.
#[derive(Default, Debug)]
pub struct Suites {
    /// Les incidents que le rattachement des retours a releves (`etats::AvecIncidents`).
    pub incidents_des_retours: Vec<Value>,
    /// Les attestations France Travail archivees, a ranger.
    pub aer: Vec<PathBuf>,
    /// Les comptes rendus DGFiP de nature 94 archives (le retour tel que la liste le decrit, l'archive) : leurs taux
    /// s'ecrivent dans la fiche de leur salarie.
    pub taux_pas: Vec<(Value, PathBuf)>,
    /// Les notifications du taux AT/MP de nature 34 archivees (le retour, l'archive) : leur code et leur taux se posent
    /// dans la surcouche de l'annee.
    pub taux_atmp: Vec<(Value, PathBuf)>,
    /// Un depot refuse.
    pub depot_refuse: Option<DepotRefuse>,
}

impl Suites {
    /// Le resultat d'un `AvecIncidents`, ses incidents gardes pour l'appelant.
    pub fn garder<T>(&mut self, r: crate::etats::AvecIncidents<T>) -> socle::echec::R<T> {
        self.incidents_des_retours.extend(r.incidents);
        r.resultat
    }
}
