//! L'ENGAGEMENT IMPORTE : un engagement que l'import a tire des pieces d'un autre logiciel (les DSN deposees, les AEM,
//! les bulletins de paie). Il vit avec les autres, dans `engagements/AAAA-MM.json`, le fichier du mois ou il commence,
//! et porte `importe` :
//!
//! ```text
//! "importe": {"le", "source", "contrat"?: {ce que l'AEM ou le bulletin atteste}, "paies": {"AAAA-MM": {ce qui a ete verse}}}
//! ```
//!
//! Il est valide et ses documents sont partis : ce qui en est parti, c'est l'import lui-meme (les pieces de l'autre
//! logiciel). IL SE LIT, IL NE SE MODIFIE PAS ET NE SE RECALCULE JAMAIS : il ne touche aucun mois de la saisie
//! (`stock::Objet::couvre`), donc aucun calcul, aucune declaration ni aucun document ne le prend ; l'ecran ne peut ni le
//! reecrire ni en poser un double (`stock::ecrire_vue_du_mois`). Son numero de contrat est celui que la DSN deposee lui
//! a donne (S21.G00.40.009), jamais un numero tire : il compte parmi les numeros pris (`numeros`).
//!
//! Ce qui le lit : les cumuls des annexes 8 et 10 (`cumul`, par ce que le contrat atteste), le mois tenu avant
//! PuppetPaie (`contrats_lus`, `paies_lues` : une ligne par contrat atteste, une ligne par paie versee).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::stock::{self, Objet};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Dans un engagement importe : ce que l'import a lu de lui.
pub const IMPORTE: &str = "importe";

/// Un engagement importe.
#[derive(Debug, Clone)]
pub struct Importe {
    objet: Objet,
}

impl Importe {
    /// L'engagement `o` s'il est importe.
    pub fn de(o: Objet) -> Option<Importe> {
        o.est_importe().then_some(Importe { objet: o })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    fn lu(&self) -> &Map<String, Value> {
        static VIDE: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
        self.objet.brut.get(IMPORTE).and_then(Value::as_object).unwrap_or_else(|| VIDE.get_or_init(Map::new))
    }

    fn texte(&self, k: &str) -> String {
        self.objet.brut.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default()
    }

    /// Quand il a ete importe.
    pub fn le(&self) -> String {
        self.lu().get("le").map(en_texte).unwrap_or_default()
    }

    /// Ce que l'AEM ou le bulletin atteste du contrat (heures, cachets, jours, brut, numero d'objet...), s'il l'est.
    pub fn contrat(&self) -> Option<&Map<String, Value>> {
        self.lu().get("contrat").and_then(Value::as_object)
    }

    /// Ce qui a ete verse, mois par mois.
    pub fn paies(&self) -> BTreeMap<String, &Map<String, Value>> {
        self.lu().get("paies").and_then(Value::as_object).into_iter().flatten().filter_map(|(m, p)| Some((m.clone(), p.as_object()?))).collect()
    }

    /// Les mois qu'il touche : de son debut a sa fin, et ceux ou il a ete verse.
    pub fn mois(&self) -> Vec<String> {
        let (debut, fin) = (self.texte("debut"), self.texte("fin"));
        let mut v: Vec<String> = self.paies().into_keys().collect();
        if debut.len() >= 7 {
            let fin = if fin.len() >= 7 { fin } else { debut.clone() };
            let mut m = debut[..7].to_string();
            while m.as_str() <= &fin[..7] {
                v.push(m.clone());
                match stock::mois_apres(&m) {
                    Ok(n) => m = n,
                    Err(_) => break,
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// Le contrat atteste, en une ligne : `{"salarie", "debut", "fin", <ce qu'il atteste>}`.
    pub fn ligne_du_contrat(&self) -> Option<Value> {
        let c = self.contrat()?;
        let mut l = self.bornes();
        l.extend(c.iter().map(|(k, v)| (k.clone(), v.clone())));
        Some(Value::Object(l))
    }

    /// Les paies versees, une ligne par mois : `{"mois", "salarie", "debut", "fin", "contrat"?, <ce qui a ete verse>}`.
    pub fn lignes_des_paies(&self) -> Vec<Value> {
        self.paies()
            .into_iter()
            .map(|(m, p)| {
                let mut l = self.bornes();
                l.insert("mois".into(), Value::String(m));
                l.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                Value::Object(l)
            })
            .collect()
    }

    fn bornes(&self) -> Map<String, Value> {
        let mut l = Map::new();
        for k in ["salarie", "debut", "fin"] {
            l.insert(k.into(), Value::String(self.texte(k)));
        }
        if let Some(n) = self.objet.brut.get("numero").filter(|n| vrai(n)) {
            l.insert("contrat".into(), n.clone());
        }
        if let Some(s) = self.lu().get("source").filter(|s| vrai(s)) {
            l.insert("source".into(), s.clone());
        }
        l.insert("importe_le".into(), Value::String(self.le()));
        l
    }
}

/// Tous les engagements importes, fichier par fichier.
pub fn tous(racine: &Path) -> Resultat<Vec<Importe>> {
    Ok(stock::objets(racine)?.into_iter().filter_map(Importe::de).collect())
}

/// Les engagements importes qui touchent `mois`.
pub fn du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Importe>> {
    Ok(tous(racine)?.into_iter().filter(|i| i.mois().iter().any(|m| m == mois)).collect())
}

/// Les contrats attestes de tous les engagements importes, une ligne chacun (`Importe::ligne_du_contrat`).
pub fn contrats_lus(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().filter_map(Importe::ligne_du_contrat).collect())
}

/// Les paies versees de tous les engagements importes, une ligne par mois (`Importe::lignes_des_paies`).
pub fn paies_lues(racine: &Path) -> Resultat<Vec<Value>> {
    Ok(tous(racine)?.iter().flat_map(Importe::lignes_des_paies).collect())
}

/// L'engagement importe du salarie `cle` (la cle de sa fiche) qui commence le `debut` et touche `mois`, s'il y en a un.
pub fn celui_de(racine: &Path, mois: &str, cle: &str, debut: &str) -> Resultat<Option<Importe>> {
    Ok(du_mois(racine, mois)?.into_iter().find(|i| i.texte("salarie") == cle && i.texte("debut") == debut))
}

/// Le refus d'un geste sur un engagement importe : il se lit, il ne se modifie pas.
pub fn refus(qui: &str, debut: &str) -> String {
    let jour = socle::date::Jour::lire(debut).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| debut.to_string());
    format!(
        "L'engagement de {} du {} est importé : il se lit tel que l'ancien logiciel l'a déclaré, il ne se modifie pas, ne se \
         recalcule pas et ne se supprime pas. Rien n'est écrit.",
        qui, jour
    )
}

/// ECRIT LES ENGAGEMENTS IMPORTES, tous : chacun dans le fichier du mois ou il commence (son `debut`, sinon le premier
/// mois ou il a ete verse), apres les engagements de la saisie. Ceux qui y etaient s'en vont ; rien d'autre ne change.
/// Rend les fichiers ecrits.
pub fn ecrire_tous(racine: &Path, importes: Vec<Map<String, Value>>) -> Resultat<Vec<PathBuf>> {
    let mut par_mois: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for e in importes {
        let debut = e.get("debut").map(en_texte).unwrap_or_default();
        let premier = e.get(IMPORTE).and_then(|i| i.get("paies")).and_then(Value::as_object).and_then(|p| p.keys().next().cloned());
        let mois = if debut.len() >= 7 { debut[..7].to_string() } else { premier.ok_or("un engagement importe sans debut ni paie")? };
        par_mois.entry(mois).or_default().push(Value::Object(e));
    }
    let mut touches: Vec<String> = tous(racine)?.iter().map(|i| i.objet.fichier.clone()).collect();
    touches.extend(par_mois.keys().cloned());
    touches.sort();
    touches.dedup();
    let mut ecrits = Vec::new();
    for f in touches {
        let existe = stock::lire_fichier(racine, &f)?;
        let neufs = par_mois.remove(&f).unwrap_or_default();
        let mut contenu = existe.clone().unwrap_or_default();
        contenu.insert("mois".into(), Value::String(f.clone()));
        let mut es: Vec<Value> =
            contenu.get("engagements").and_then(Value::as_array).into_iter().flatten().filter(|e| !e.get(IMPORTE).is_some_and(Value::is_object)).cloned().collect();
        es.extend(neufs);
        contenu.insert("engagements".into(), Value::Array(es));
        if existe.as_ref() == Some(&contenu) {
            continue;
        }
        let p = stock::chemin(racine, &f);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| socle::fichiers::panne_fichier(&e, d))?;
        }
        stock::sauver_avant(&p)?;
        socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)?;
        ecrits.push(p);
    }
    Ok(ecrits)
}

/// CE QUE L'IMPORT COMPLETE DE LUI-MEME : ce que les pieces ne disaient pas et que la personne dit apres l'import (la
/// nature d'un contrat de permanent, son numero). `completer` recoit chaque engagement importe tel qu'il est ecrit et
/// dit s'il l'a change ; ceux-la se reecrivent. Rend combien ont change.
pub fn completer(racine: &Path, mut completer: impl FnMut(&mut Map<String, Value>) -> bool) -> Resultat<usize> {
    let mut tous: Vec<Map<String, Value>> = tous(racine)?.into_iter().map(|i| i.objet.brut).collect();
    let mut changes = 0usize;
    for e in tous.iter_mut() {
        if completer(e) {
            changes += 1;
        }
    }
    if changes > 0 {
        ecrire_tous(racine, tous)?;
    }
    Ok(changes)
}
