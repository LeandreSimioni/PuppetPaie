//! Les compteurs des documents : les numeros de contrat et de bulletin que portent les paies d'un mois
//! (`numeroter`, `paies_aux_numeros_declares`), ceux qu'un envoi reserve avant de les ecrire (`Reserves`), la sequence
//! continue des N CE (`prochain_continu`) et le verrou sous lequel un numero se reserve puis s'ecrit (`prendre_le_verrou`).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use crate::valeur::lire_json;
use socle::verrou::Verrou;
use socle::echec::Refus;
use socle::ecriture::json_indente;
use engagement::calcul::Paie;

fn os(chemin: &std::path::Path, e: std::io::Error) -> Echec {
    erreur("OSError", socle::echec::du_disque(&chemin, &e))
}

fn valeur(m: String) -> Echec {
    erreur("ValueError", m)
}

/// La suite des numeros de chaque annee, sous la racine de la compagnie.
const COMPTEURS: &str = "config/compteurs.json";

/// Une suite de numeros que les compteurs tiennent : les bulletins (`17/2026`), les certificats Congés Spectacles (N CE,
/// une suite qui ne repart pas a 1 chaque an).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suite {
    Bulletin,
    CertificatCs,
}

/// `config/compteurs.json` : par annee (`"2026"`), le dernier numero de chaque suite.
pub type Compteurs = BTreeMap<String, CompteursDeLAnnee>;

/// Les derniers numeros d'une annee, suite par suite.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompteursDeLAnnee {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bulletin: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificat_cs: Option<i64>,
}

impl CompteursDeLAnnee {
    /// Le dernier numero de la suite, s'il y en a un.
    pub fn de(&self, suite: Suite) -> Option<i64> {
        match suite {
            Suite::Bulletin => self.bulletin,
            Suite::CertificatCs => self.certificat_cs,
        }
    }

    fn poser(&mut self, suite: Suite, n: i64) {
        match suite {
            Suite::Bulletin => self.bulletin = Some(n),
            Suite::CertificatCs => self.certificat_cs = Some(n),
        }
    }
}

/// `config/compteurs.json`, lu par sa forme, ou vide s'il n'existe pas. La seule lecture du fichier.
pub fn compteurs(racine: &std::path::Path) -> R<Compteurs> {
    let p = racine.join(COMPTEURS);
    if !p.exists() {
        return Ok(Compteurs::new());
    }
    let v = lire_json(&p).map_err(valeur)?;
    socle::forme::lire_en_phrase(&v, |cle, _| {
        format!("Les compteurs de numéros de la compagnie (config/compteurs.json) ne se lisent pas{} : le fichier se corrige à la main.", if cle.is_empty() { String::new() } else { format!(" ({})", cle) })
    })
    .map_err(valeur)
}

/// `Contexte.prochain_continu` : une sequence qui ne repart pas a 1 chaque an. La LECTURE balaie toutes les
/// annees de `compteurs.json` et retient le maximum.
pub fn prochain_continu(ctx: &Contexte, suite: Suite, defaut: i64) -> R<i64> {
    let vu = compteurs(&ctx.racine)?.values().filter_map(|c| c.de(suite)).fold(defaut - 1, i64::max);
    Ok(vu + 1)
}

/// `verrou_compteurs(ctx, qui=...)` : refuse si un autre tient les compteurs.
pub fn prendre_le_verrou(ctx: &Contexte, qui: &str) -> R<Verrou> {
    let chemin = socle::verrou::chemin_compteurs(&ctx.racine);
    let maintenant = socle::paris::iso_secondes(socle::temps::secondes()).replace('T', " ");
    let marque = socle::verrou::marque(qui, std::process::id(), &maintenant);
    match Verrou::prendre(&chemin, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
        Ok(v) => Ok(v),
        Err(v) => Err(erreur("CompteursVerrouilles", v.message())),
    }
}


// ------------------------------------------------------------------ les numeros des contrats et des bulletins

/// Les numeros de contrat (FA1.26.13) et de bulletin (17/2026) que les paies porteraient (`engagement::numeros::numeroter`), en
/// memoire : l'apercu n'ecrit rien. Un engagement qui PORTE DEJA son numero de contrat le garde (`engagement::numeros`),
/// une paie partie son numero de bulletin (`engagement::etats::Valide::bulletin`).
pub fn numeroter(ctx: &Contexte, paies: &mut [Paie], annee: i32, departs: &engagement::departs::Departs) -> Result<(), Refus> {
    numeroter_en_attente(ctx, paies, annee, &mut HashMap::new(), departs)
}

/// La meme, avec les numeros de bulletin deja RESERVES par un appel precedent du meme processus : `annee -> dernier
/// numero`, sinon a la suite du compteur de l'annee (`config/compteurs.json`) et des bulletins importes de l'annee
/// (`engagement::numeros::dernier_bulletin_importe`).
pub fn numeroter_en_attente(ctx: &Contexte, paies: &mut [Paie], annee: i32, attente: &mut HashMap<i32, i64>, departs: &engagement::departs::Departs) -> Result<(), Refus> {
    let n_bull = match attente.get(&annee) {
        Some(v) => v + 1,
        None => {
            let lu = compteurs(&ctx.racine)?.get(&annee.to_string()).and_then(|c| c.bulletin).unwrap_or(0);
            lu.max(engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee.into())?) + 1
        }
    };
    let n_bull = engagement::numeros::numeroter(ctx, paies, annee, n_bull, departs)?;
    attente.insert(annee, n_bull - 1);
    Ok(())
}

// ------------------------------------------------------------- les numeros reserves

/// Les numeros reserves par ce processus, dans leur ordre d'arrivee. Rien n'est ecrit avant `valider`.
#[derive(Default)]
pub struct Reserves {
    valeurs: HashMap<(i64, Suite), i64>,
    ordre: Vec<(i64, Suite)>,
}

impl Reserves {
    /// Le suivant de ce qui est reserve, sinon de ce que `compteurs.json` porte ; un bulletin, apres les bulletins importes
    /// de l'annee (`engagement::numeros::dernier_bulletin_importe`).
    pub fn prochain(&self, ctx: &Contexte, suite: Suite, annee: i64) -> R<i64> {
        if let Some(v) = self.valeurs.get(&(annee, suite)) {
            return Ok(v + 1);
        }
        let lu = compteurs(&ctx.racine)?.get(&annee.to_string()).and_then(|c| c.de(suite)).unwrap_or(0);
        let importe = if suite == Suite::Bulletin { engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee).map_err(Echec::from)? } else { 0 };
        Ok(lu.max(importe) + 1)
    }

    /// Reserve, sans ecrire.
    pub fn enregistrer(&mut self, annee: i64, suite: Suite, v: i64) {
        let k = (annee, suite);
        if !self.valeurs.contains_key(&k) {
            self.ordre.push(k);
        }
        self.valeurs.insert(k, v);
    }

    /// Les numeros de bulletin des paies, reserves ; leurs numeros de contrat, ceux qu'elles porteraient.
    pub fn numeroter(&mut self, ctx: &Contexte, paies: &mut [Paie], annee: i32, departs: &engagement::departs::Departs) -> R<()> {
        let mut attente: HashMap<i32, i64> = self.valeurs.get(&(annee.into(), Suite::Bulletin)).map(|v| (annee, *v)).into_iter().collect();
        numeroter_en_attente(ctx, paies, annee, &mut attente, departs)?;
        if let Some(n) = attente.get(&annee) {
            self.enregistrer(annee.into(), Suite::Bulletin, *n);
        }
        Ok(())
    }

    /// Ecrit les numeros reserves dans `config/compteurs.json`.
    pub fn valider(&self, ctx: &Contexte) -> R<()> {
        if self.ordre.is_empty() {
            return Ok(());
        }
        let p = ctx.racine.join(COMPTEURS);
        let mut c = compteurs(&ctx.racine)?;
        for k in &self.ordre {
            c.entry(k.0.to_string()).or_default().poser(k.1, self.valeurs[k]);
        }
        let v = serde_json::to_value(&c).map_err(|e| valeur(e.to_string()))?;
        std::fs::write(&p, json_indente(&v, 2)).map_err(|e| os(&p, e))
    }
}

// ------------------------------------------------------------------ les paies d'un mois aux numeros declares

/// LA SAISIE DU MOIS AUX NUMEROS DECLARES : ce qui peut partir de `mois` (`etats::validees_du_mois` : la saisie reduite
/// aux engagements valides, et leurs paies ecrites), chaque contrat sous le numero que son engagement porte depuis sa
/// validation (`engagement::numeros`), chaque bulletin sous le sien (ecrit dans la paie a son depart, ou celui que le
/// compteur propose). `reserves` : les numeros de bulletin qu'un envoi propose, que la route d'envoi ecrit plus loin. Un
/// mois sans engagement valide n'a rien.
pub fn paies_aux_numeros_declares(ctx: &Contexte, mois: &str, reserves: Option<&mut Reserves>, departs: &engagement::departs::Departs) -> Result<(engagement::saisie::VueDuMois, Vec<Paie>), Refus> {
    let Some((data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois, departs)? else {
        return Ok((engagement::saisie::VueDuMois::vide(socle::mois::Mois::lire(mois)?), Vec::new()));
    };
    let annee = data.mois.annee();
    match reserves {
        Some(r) => r.numeroter(ctx, &mut paies, annee, departs)?,
        None => numeroter(ctx, &mut paies, annee, departs)?,
    }
    Ok((data, paies))
}

