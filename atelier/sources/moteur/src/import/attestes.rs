//! UN CONTRAT NE S'ATTESTE QU'UNE FOIS. Une AEM deposee, une AEM en PDF, un bulletin, un certificat Congés Spectacles
//! disent chacun d'un contrat : ce salarie, ces dates, ces heures, cachets et jours, ce brut. Le registre tient ce qui
//! est deja atteste (l'historique, puis ce que l'import en cours retient) ; chaque piece lui demande si son contrat
//! l'est (`pareils`), et s'il ne l'est pas, le lui ajoute (`ajouter`). Ce qu'il retient est la seule liste que
//! l'ecriture prend (`import::ecrite`).
//!
//! LA REGLE : un contrat du meme salarie (par son NIR) dont les dates chevauchent celles de la piece l'atteste deja ;
//! la piece le confirme, n'ajoute rien, et ses ecarts se notent. Une exception, une seule : le contrat qu'un certificat
//! seul atteste ne porte ni les heures, ni les cachets et les jours a part, et son brut est arrondi a l'euro
//! (`import::certificat_lu`) ; il ne confirme pas le contrat d'un bulletin ou d'une AEM, qui le remplace (l'ecriture
//! retire alors le sien).

use std::collections::HashMap;

use serde_json::Value;

use droit::contexte::Contexte;
use socle::echec::{Echec, R};
use socle::num::Decimal;
use socle::valeur::valeur_en_texte;

/// Un contrat atteste : de qui (NIR), quand, par quoi, et ce qu'il compte.
#[derive(Clone, Debug)]
pub(crate) struct Atteste {
    pub(crate) nir: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    /// « AEM », « bulletin », « certificat », ou « historique » (avec `de_certificat` pour celui d'un certificat).
    pub(crate) source: &'static str,
    pub(crate) de_certificat: bool,
    pub(crate) heures: i128,
    pub(crate) cachets: i128,
    pub(crate) jours: i128,
    pub(crate) brut: Option<Decimal>,
}

impl Atteste {
    /// Son nom dans le compte rendu.
    pub(crate) fn d_ou(&self) -> &'static str {
        match (self.source, self.de_certificat) {
            ("historique", true) => "historique, d'un certificat",
            (s, _) => s,
        }
    }
}

/// Ce qu'une ligne de contrat (la forme qu'un engagement importe porte) dit, pour le registre.
fn lu(nir: &str, ligne: &Value, source: &'static str) -> Atteste {
    let t = |k: &str| ligne.get(k).map(valeur_en_texte).unwrap_or_default();
    let n = |k: &str| ligne.get(k).and_then(Value::as_i64).unwrap_or(0) as i128;
    Atteste {
        nir: nir.to_string(),
        debut: t("debut"),
        fin: t("fin"),
        source,
        de_certificat: t("atteste_par") == "certificat",
        heures: n("heures"),
        cachets: n("cachets"),
        jours: n("jours"),
        brut: socle::num::decimal_lu(&t("brut")),
    }
}

/// Les dates `[d1, f1]` et `[d, f]` (ISO) se chevauchent ; une fin vide court toujours.
pub(crate) fn chevauche(d1: &str, f1: &str, d: &str, f: &str) -> bool {
    !d1.is_empty() && d1 <= f && (f1.is_empty() || d <= f1)
}

/// Ce que l'import retient : (NIR, la ligne du contrat, le bloc du compte rendu qui le compte).
pub(crate) type Retenu = (String, Value, &'static str);

pub(crate) struct Registre {
    tous: Vec<Atteste>,
    pub(crate) retenus: Vec<Retenu>,
}

impl Registre {
    /// Le registre de l'historique : les contrats des engagements importes, rattaches a leur NIR par la fiche
    /// (`par_cle` : cle de fiche -> NIR).
    pub(crate) fn de_l_historique(ctx: &Contexte, par_cle: &HashMap<String, String>) -> R<Registre> {
        let mut tous = Vec::new();
        for e in engagement::importe::contrats_lus(&ctx.racine).map_err(Echec::from)? {
            let cle = e.get("salarie").map(valeur_en_texte).unwrap_or_default();
            if let Some(nir) = par_cle.get(&cle) {
                tous.push(lu(nir, &e, "historique"));
            }
        }
        Ok(Registre { tous, retenus: Vec::new() })
    }

    /// Les contrats deja attestes qui attestent celui-ci (`certificat` : la piece est un certificat). Vide : il ne
    /// l'est pas.
    pub(crate) fn pareils(&self, nir: &str, debut: &str, fin: &str, certificat: bool) -> Vec<Atteste> {
        self.tous.iter().filter(|a| a.nir == nir && chevauche(&a.debut, &a.fin, debut, fin) && (certificat || !a.de_certificat)).cloned().collect()
    }

    /// Retient le contrat `ligne` de la piece `source` (une fois `pareils` vide), compte au bloc `bloc` ; `true` : il
    /// remplace le contrat d'un certificat. Un contrat de certificat que cet import retenait s'efface ici.
    pub(crate) fn ajouter(&mut self, nir: &str, ligne: Value, source: &'static str, bloc: &'static str) -> bool {
        let a = lu(nir, &ligne, source);
        let remplace = !a.de_certificat && self.tous.iter().any(|x| x.nir == nir && x.de_certificat && chevauche(&x.debut, &x.fin, &a.debut, &a.fin));
        if remplace {
            self.tous.retain(|x| !(x.nir == nir && x.de_certificat && x.source != "historique" && chevauche(&x.debut, &x.fin, &a.debut, &a.fin)));
            let t = |l: &Value, k: &str| l.get(k).map(valeur_en_texte).unwrap_or_default();
            self.retenus.retain(|(n, l, _)| !(n == nir && t(l, "atteste_par") == "certificat" && chevauche(&t(l, "debut"), &t(l, "fin"), &a.debut, &a.fin)));
        }
        self.tous.push(a);
        self.retenus.push((nir.to_string(), ligne, bloc));
        remplace
    }

    /// Combien ce bloc retient.
    pub(crate) fn nombre(&self, bloc: &str) -> usize {
        self.retenus.iter().filter(|r| r.2 == bloc).count()
    }

    /// Les lignes que ce bloc retient.
    pub(crate) fn lignes(&self, bloc: &str) -> Vec<Value> {
        self.retenus.iter().filter(|r| r.2 == bloc).map(|r| r.1.clone()).collect()
    }
}
