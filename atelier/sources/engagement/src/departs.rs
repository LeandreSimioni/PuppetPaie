//! CE QUI EST PARTI, tel que la couche des envois le dit a l'engagement. L'engagement ne lit pas le journal des envois :
//! il est du cote des envois, au-dessus de lui. La couche du dessus pose une fois son lecteur (`poser_lecteur`), qui rend,
//! pour chaque engagement (la cle de la fiche de son salarie et son numero de contrat), ce qui en est parti et sur quelle
//! version (`Depart`). L'engagement s'en sert pour savoir ce qui est fige (les documents d'un mois partis), quand garder
//! une version datee, et ce qui est a refaire (`etats::a_refaire`).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use socle::num::Resultat;

/// Ce qui part d'un engagement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envoi {
    /// Le contrat (a la signature ou imprime pour une remise en main propre), ou un avenant.
    Contrat,
    /// La declaration prealable a l'embauche.
    Dpae,
    /// Les documents d'un mois au salarie (bulletin, certificat Conges Spectacles, AER).
    Documents,
    /// La DSN mensuelle d'un mois.
    Dsn,
    /// Un signalement d'evenement (DSN de nature 04, 05, 07 ou 08 : arret, reprise, fin de contrat) qui cite son contrat.
    Signalement,
}

/// Un depart : quoi, pour quel mois (documents et DSN), sur quelle version de l'engagement, et quand.
#[derive(Debug, Clone)]
pub struct Depart {
    pub envoi: Envoi,
    pub mois: Option<String>,
    /// Le mois de la DSN qui a declare `mois` quand ce n'est pas la sienne : la correction d'un mois passe, portee par la
    /// DSN d'un mois suivant. `None` : la DSN du mois lui-meme.
    pub par: Option<String>,
    /// Le numero de l'avenant quand ce qui est parti est un avenant au contrat, pas le contrat.
    pub avenant: Option<String>,
    pub version: u64,
    pub le: String,
}

/// Ce qui est parti, engagement par engagement : `(cle du salarie, numero du contrat)`.
#[derive(Debug, Clone, Default)]
pub struct Departs {
    par_engagement: BTreeMap<(String, String), Vec<Depart>>,
}

impl Departs {
    /// Note un depart de l'engagement du salarie `cle` au numero `numero`.
    pub fn noter(&mut self, cle: &str, numero: &str, d: Depart) {
        self.par_engagement.entry((cle.to_string(), numero.to_string())).or_default().push(d);
    }

    /// Les departs de cet engagement, dans l'ordre ou ils sont partis.
    pub fn de(&self, cle: &str, numero: &str) -> &[Depart] {
        self.par_engagement.get(&(cle.to_string(), numero.to_string())).map_or(&[], Vec::as_slice)
    }

    /// Les engagements dont quelque chose est parti : `(cle du salarie, numero)`.
    pub fn engagements(&self) -> impl Iterator<Item = (&str, &str)> {
        self.par_engagement.keys().map(|(c, n)| (c.as_str(), n.as_str()))
    }
}

/// Qui lit ce qui est parti pour une compagnie (sa racine).
pub type Lecteur = fn(&Path) -> Resultat<Departs>;

static LECTEUR: OnceLock<Lecteur> = OnceLock::new();

/// Pose le lecteur de ce qui est parti, une fois pour le processus.
pub fn poser_lecteur(l: Lecteur) {
    let _ = LECTEUR.set(l);
}

/// Ce qui est parti pour la compagnie `racine`. Refus si aucun lecteur n'est pose : sans lui, rien ne dirait ce qui est
/// fige, et un document parti se recalculerait.
pub fn lire(racine: &Path) -> Resultat<Departs> {
    let l = LECTEUR.get().ok_or("Ce qui est parti ne se lit pas : la couche des envois n'a pas pose son lecteur (`departs::poser_lecteur`).")?;
    l(racine)
}
