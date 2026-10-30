//! CE QUI EST PARTI, tel que la couche des envois le dit a l'engagement. L'engagement ne lit pas le journal des envois :
//! il est du cote des envois, au-dessus de lui. Le moteur lit le journal (`envois::journal::departs`) et passe a chaque
//! fonction de l'engagement qui en a besoin ce qui est parti de chaque engagement (la cle de la fiche de son salarie et
//! son numero de contrat), et sur quelle version (`Depart`). L'engagement s'en sert pour savoir ce qui est fige (les
//! documents d'un mois partis), quand garder une version datee, et ce qui est a refaire (`etats::a_refaire`).

use std::collections::BTreeMap;

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
    par_engagement: std::sync::Arc<BTreeMap<(String, String), Vec<Depart>>>,
}

impl Departs {
    /// Note un depart de l'engagement du salarie `cle` au numero `numero`.
    pub fn noter(&mut self, cle: &str, numero: &str, d: Depart) {
        std::sync::Arc::make_mut(&mut self.par_engagement).entry((cle.to_string(), numero.to_string())).or_default().push(d);
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

