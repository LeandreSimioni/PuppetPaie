//! LA RUBRIQUE DU BULLETIN : une ligne de cotisation de la pile du bareme applique (`rubriques_bulletin`), lue une fois
//! apres l'empilement (`bareme::bareme_applique`) et rendue typee. Le calcul de la paie et la DSN la lisent ici, et pas
//! dans le JSON.
//!
//! La lecture refuse une cle qu'elle ne connait pas (sauf un commentaire, `_…`) et une valeur du mauvais type : une faute
//! de frappe dans un fichier de bareme se voit au chargement, pas en cotisation absente.

use serde_json::{Map, Value};
use socle::fichiers::vrai;
use socle::num::{dec, flottant, Decimal, Resultat};

/// Une ligne de cotisation du bulletin.
#[derive(Debug, Clone, PartialEq)]
pub struct Rubrique {
    pub id: String,
    pub famille: Option<String>,
    pub libelle: Option<String>,
    /// La cle de son assiette dans les bases du calcul (`brut`, `brut_x115`, `t1_journaliere`…).
    pub base: Option<String>,
    /// Les deux parts, en pourcent ; `None` : pas de part de ce cote.
    pub taux_pat: Option<f64>,
    pub taux_sal: Option<f64>,
    /// Une part patronale qui se compose de plusieurs taux, chacun arrondi au centime (`composants`, ou `composants_pat`) ;
    /// la part salariale de meme (`composants_sal`).
    pub composants: Option<Vec<f64>>,
    pub composants_pat: Option<Vec<f64>>,
    pub composants_sal: Option<Vec<f64>>,
    /// Le complement de taux patronal au-dela d'un seuil de SMIC.
    pub complement: Option<Complement>,
    /// La CET des cadres, fondue dans la ligne quand elle est due.
    pub cet_si_depasse: Option<Cet>,
    pub masquer_base_et_taux: bool,
    /// Le type de composant de la base 31 de la DSN (S21.G00.79.001) que la sante pose (`bareme::bareme_sante`).
    pub composant_79: Option<String>,
}

/// Le taux qui s'ajoute au taux patronal quand la remuneration depasse `seuil_smic` fois le SMIC (sans seuil : toujours).
#[derive(Debug, Clone, PartialEq)]
pub struct Complement {
    pub taux: f64,
    pub seuil_smic: Option<Decimal>,
}

/// Les taux patronal et salarial de la CET.
#[derive(Debug, Clone, PartialEq)]
pub struct Cet {
    pub pat: f64,
    pub sal: f64,
}

/// Ce que l'empilement lit et laisse sur la rubrique (`bareme_taxe_apprentissage`, l'ordre d'une rubrique ajoutee, le
/// taux d'une contribution selon la representation du personnel) : connu, sans rien pour le calcul.
const DE_L_EMPILEMENT: [&str; 3] = ["taxe_apprentissage", "apres", "par_representation"];

/// LA LISTE DES RUBRIQUES de la pile appliquee, typee, dans l'ordre du bareme.
pub fn rubriques(pile: &Value) -> Resultat<Vec<Rubrique>> {
    let liste = pile
        .get("rubriques_bulletin")
        .and_then(Value::as_array)
        .ok_or_else(|| socle::echec::cle("Le barème de l'année est incomplet : le signaler au développeur.", "rubriques_bulletin"))?;
    liste.iter().map(rubrique).collect()
}

/// Une rubrique de la pile.
fn rubrique(v: &Value) -> Resultat<Rubrique> {
    let m = v.as_object().ok_or_else(|| refus("?", "", "un objet est attendu"))?;
    let id = match m.get("id") {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(refus("?", "id", "un texte est attendu")),
    };
    let lu = Lu { id: &id, m };
    lu.connues(&[
        "id", "famille", "libelle", "base", "taux_pat", "taux_sal", "composants", "composants_pat", "composants_sal",
        "complement", "cet_si_depasse", "masquer_base_et_taux",
    ])?;
    let complement = match m.get("complement").filter(|c| vrai(c)) {
        None => None,
        Some(c) => {
            let c = lu.sous(c, "complement", &["taux", "seuil_smic"])?;
            Some(Complement {
                taux: c.taux("taux")?.ok_or_else(|| refus(&id, "complement/taux", "un nombre est attendu"))?,
                seuil_smic: match c.m.get("seuil_smic") {
                    None | Some(Value::Null) => None,
                    Some(s) => Some(dec(s).map_err(|e| refus(&id, "complement/seuil_smic", e))?),
                },
            })
        }
    };
    let cet_si_depasse = match m.get("cet_si_depasse").filter(|c| vrai(c)) {
        None => None,
        Some(c) => {
            let c = lu.sous(c, "cet_si_depasse", &["pat", "sal"])?;
            let part = |k: &str| c.taux(k)?.ok_or_else(|| refus(&id, &format!("cet_si_depasse/{}", k), "un nombre est attendu"));
            Some(Cet { pat: part("pat")?, sal: part("sal")? })
        }
    };
    Ok(Rubrique {
        famille: lu.texte("famille")?,
        libelle: lu.texte("libelle")?,
        base: lu.texte("base")?,
        taux_pat: lu.part("taux_pat")?,
        taux_sal: lu.part("taux_sal")?,
        composants: lu.taux_liste("composants")?,
        composants_pat: lu.taux_liste("composants_pat")?,
        composants_sal: lu.taux_liste("composants_sal")?,
        complement,
        cet_si_depasse,
        masquer_base_et_taux: match m.get("masquer_base_et_taux") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(refus(&id, "masquer_base_et_taux", "vrai ou faux est attendu")),
        },
        composant_79: lu.texte("_composant_79")?.filter(|t| !t.is_empty()),
        id,
    })
}

/// Le refus d'une rubrique illisible : la phrase pour l'ecran, la rubrique, la cle et ce qui ne va pas a part.
fn refus(id: &str, cle: &str, detail: impl std::fmt::Display) -> String {
    socle::echec::a_la_cle(
        "Le barème porte une rubrique du bulletin que le moteur ne sait pas lire : le signaler au développeur.",
        &format!("rubriques_bulletin/{}/{}", id, cle),
        detail,
    )
}

/// Un objet de la rubrique `id` en cours de lecture.
struct Lu<'a> {
    id: &'a str,
    m: &'a Map<String, Value>,
}

impl<'a> Lu<'a> {
    /// Refuse une cle hors de `connues`, de l'empilement ou d'un commentaire.
    fn connues(&self, connues: &[&str]) -> Resultat<()> {
        match self.m.keys().find(|k| !k.starts_with('_') && !connues.contains(&k.as_str()) && !DE_L_EMPILEMENT.contains(&k.as_str())) {
            Some(k) => Err(refus(self.id, k, "cette clé n'est pas une clé de rubrique")),
            None => Ok(()),
        }
    }

    /// L'objet `v` a la cle `cle`, dont les cles sont `connues`.
    fn sous(&self, v: &'a Value, cle: &str, connues: &[&str]) -> Resultat<Lu<'a>> {
        let m = v.as_object().ok_or_else(|| refus(self.id, cle, "un objet est attendu"))?;
        if let Some(k) = m.keys().find(|k| !k.starts_with('_') && !connues.contains(&k.as_str())) {
            return Err(refus(self.id, &format!("{}/{}", cle, k), "cette clé n'est pas attendue ici"));
        }
        Ok(Lu { id: self.id, m })
    }

    fn texte(&self, cle: &str) -> Resultat<Option<String>> {
        match self.m.get(cle) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(refus(self.id, cle, "un texte est attendu")),
        }
    }

    /// Un taux : un nombre (ou un nombre ecrit en texte), `None` s'il est nul ou absent.
    fn taux(&self, cle: &str) -> Resultat<Option<f64>> {
        match self.m.get(cle) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => flottant(v).map(Some).map_err(|e| refus(self.id, cle, e)),
        }
    }

    /// Une part de la ligne : la cle est exigee (un taux qui manque ne se devine pas), nulle quand il n'y a pas de part.
    fn part(&self, cle: &str) -> Resultat<Option<f64>> {
        if !self.m.contains_key(cle) {
            return Err(refus(self.id, cle, "la part manque : elle s'écrit, nulle s'il n'y en a pas"));
        }
        self.taux(cle)
    }

    /// Les taux d'une liste, `None` si elle est absente, nulle ou vide.
    fn taux_liste(&self, cle: &str) -> Resultat<Option<Vec<f64>>> {
        match self.m.get(cle) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Array(a)) if a.is_empty() => Ok(None),
            Some(Value::Array(a)) => a.iter().map(flottant).collect::<Resultat<_>>().map(Some).map_err(|e| refus(self.id, cle, e)),
            Some(_) => Err(refus(self.id, cle, "une liste de taux est attendue")),
        }
    }
}
