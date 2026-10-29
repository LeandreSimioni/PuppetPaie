//! CE QUE L'IMPORT MET DANS UNE FICHE QUI EXISTE DEJA (lot I2, Leandre, 14/10/2026 : « est-ce qu'il prend bien TOUT ce
//! qu'il peut tirer des imports ? » ; « pourquoi la fiche fait loi alors que le bulletin a les bonnes infos ? »).
//!
//! Une fiche que l'import CREE recoit tout ce que la DSN et les bulletins disent. Une fiche qui EXISTE recoit ce qui y
//! est vide ou « A COMPLETER » (sans casse : le titre que l'import des DSN donne a un spectacle dont la DSN ne porte que
//! le numero d'objet, `crate::routes::import::apercu_lu`), et garde tout champ rempli : la fiche fait foi, l'ecart reste
//! seulement signale (`ecarts`). L'apercu dit, fiche par fiche, ce qui serait complete et d'ou ca vient (`completer`) ;
//! l'import l'ecrit (`crate::routes::fiches::completer_fiche`), sans rien dater : une case vide n'avait rien declare.
//!
//! Un champ que la piece ne porte pas ne s'invente pas (regle 1) : on ne complete que de ce que la DSN ou le bulletin
//! imprime, par le meme chemin que la fiche qu'il creerait.

use serde_json::{json, Map, Value};

use socle::echec::R;

/// Le titre que l'import donne a un spectacle dont il ne connait que le numero d'objet.
pub(crate) const A_COMPLETER: &str = "A COMPLETER";

/// Une case qui ne dit rien : absente, nulle, un texte blanc ou « A COMPLETER » (sans casse), une liste ou un objet vides.
/// Un booleen ou un nombre disent quelque chose.
pub(crate) fn vide(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => {
            let t = s.trim();
            t.is_empty() || t.eq_ignore_ascii_case(A_COMPLETER)
        }
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        Some(_) => false,
    }
}

/// Ce que l'import ne met jamais dans une fiche qui existe : son etat (`actif` : une fiche archivee l'est exprès), son
/// historique, et `taux_pas`, que personne ne lit plus (le taux vit dans `config/pas.json`, pose par les comptes rendus
/// de la DGFiP : `champs-base.json`, la note du champ).
const JAMAIS: [&str; 3] = ["actif", "historique", "taux_pas"];

/// Les champs de `proposees` (la fiche que l'import creerait de cette piece) qu'une fiche existante recoit : un champ de
/// la fiche, vide dans `fiche`, plein dans `proposees`. Un champ officiel dont la forme lisible est remplie se derive
/// d'elle a l'ecriture : il n'est pas complete. La forme lisible (`*_affichage`) d'un champ complete suit, quand elle
/// est vide : celle que la piece donne, sinon celle que l'ecran en ferait (`champs::combler_affichage`).
pub(crate) fn champs_a_completer(nom: &str, fiche: &Value, proposees: &Map<String, Value>) -> R<Map<String, Value>> {
    let champs = declarations::champs::fiche(nom)?;
    let mut out = Map::new();
    for c in &champs {
        if c.cle.ends_with("_affichage") || JAMAIS.contains(&c.cle.as_str()) {
            continue;
        }
        let Some(v) = proposees.get(&c.cle) else { continue };
        if vide(Some(v)) || !vide(fiche.get(&c.cle)) {
            continue;
        }
        if c.derive_de.as_deref().is_some_and(|src| !src.is_empty() && !vide(fiche.get(src))) {
            continue;
        }
        out.insert(c.cle.clone(), v.clone());
    }
    if out.is_empty() {
        return Ok(out);
    }
    let mut fusion = fiche.as_object().cloned().unwrap_or_default();
    for (k, v) in &out {
        fusion.insert(k.clone(), v.clone());
    }
    let comble = declarations::champs::combler_affichage(nom, &fusion)?;
    for c in &champs {
        let Some(src) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        if !out.contains_key(&c.cle) || !vide(fiche.get(src)) {
            continue;
        }
        let lisible = proposees.get(src).filter(|v| !vide(Some(v))).or_else(|| comble.get(src).filter(|v| !vide(Some(v))));
        if let Some(v) = lisible {
            out.insert(src.to_string(), v.clone());
        }
    }
    Ok(out)
}

/// Une fiche existante que l'import completerait : laquelle (`cle` ; rien pour l'entreprise), et les champs.
pub(crate) struct ACompleter {
    pub(crate) nom: &'static str,
    pub(crate) cle: Option<String>,
    pub(crate) champs: Map<String, Value>,
}

/// Ajoute des champs a completer. LA PREMIERE PIECE QUI DONNE UN CHAMP LE GARDE : les DSN passent avant les bulletins,
/// comme pour une fiche que l'import cree (le bulletin complete la fiche que la DSN cree, il ne la reecrit pas).
pub(crate) fn ajouter(liste: &mut Vec<ACompleter>, nom: &'static str, cle: Option<&str>, champs: &Map<String, Value>) {
    if champs.is_empty() {
        return;
    }
    match liste.iter_mut().find(|a| a.nom == nom && a.cle.as_deref() == cle) {
        Some(a) => {
            for (k, v) in champs {
                a.champs.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        None => liste.push(ACompleter { nom, cle: cle.map(String::from), champs: champs.clone() }),
    }
}

/// Note dans l'entree de la reponse ce qui serait complete et d'ou ca vient (`origine` : `{"source": "dsn", "mois"}` ou
/// `{"source": "bulletin", "fichier", "page"}`). Un champ deja note garde sa premiere origine (`ajouter`).
pub(crate) fn noter(entree: &mut Value, champs: &Map<String, Value>, origine: Value) {
    if champs.is_empty() || !entree.is_object() {
        return;
    }
    if !entree.get("completer").is_some_and(Value::is_object) {
        entree["completer"] = json!({});
    }
    let Some(Value::Object(c)) = entree.get_mut("completer") else { return };
    for (k, v) in champs {
        c.entry(k.clone()).or_insert_with(|| {
            let mut m = Map::new();
            m.insert("valeur".into(), crate::routes::import::brut(v));
            if let Value::Object(o) = &origine {
                m.extend(o.clone());
            }
            Value::Object(m)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// « A COMPLETER » ne dit rien, quelle qu'en soit la casse ; un texte, un booleen faux, un zero disent quelque chose.
    #[test]
    fn a_completer_est_une_case_vide_sans_casse() {
        for v in [json!(null), json!(""), json!("  "), json!("A COMPLETER"), json!("a completer"), json!(" A Completer "),json!([]), json!({})] {
            assert!(vide(Some(&v)), "{}", v);
        }
        assert!(vide(None));
        for v in [json!("Ubu roi"), json!(false), json!(0), json!(["x"])] {
            assert!(!vide(Some(&v)), "{}", v);
        }
    }
}
