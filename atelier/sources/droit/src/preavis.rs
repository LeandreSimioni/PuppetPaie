//! Le type de preavis d'une fin de contrat (S21.G00.63.001) et l'indemnite compensatrice que la paie verse : deux
//! choses que la norme veut d'accord. Les types et leurs drapeaux sont ceux de `fctu-base.json` (cahier technique DSN
//! 2026.1, p. 253).

use serde_json::Value;

use socle::date::en_texte;
use socle::fichiers::{dossier_du_code, json_en_cache, repr};
use socle::num::Resultat;

fn types_preavis() -> Resultat<Value> {
    let n = json_en_cache(&dossier_du_code().join("fctu-base.json"))?;
    Ok(n.get("types_preavis").cloned().unwrap_or(Value::Null))
}

/// Les codes S21.G00.52.001 d'une indemnite compensatrice de preavis (`types_preavis.codes_52_de_l_indemnite_de_preavis` :
/// 023, 032 ; cahier technique 2026.1, p. 253).
pub fn codes_52_de_l_indemnite_de_preavis() -> Resultat<Vec<String>> {
    let t = types_preavis()?;
    Ok(t.get("codes_52_de_l_indemnite_de_preavis").and_then(Value::as_array).map_or(&[][..], |a| &a[..]).iter().map(en_texte).collect())
}

/// Le type de preavis (S21.G00.63.001) et l'indemnite compensatrice de preavis que la paie verse disent-ils la meme
/// chose ? `versees` : les lignes de la paie declarees sous un code de `codes_52_de_l_indemnite_de_preavis`, en (code,
/// libelle). `None` s'ils s'accordent, sinon pourquoi non. Le drapeau `indemnite_de_preavis` du type (`fctu-base.json`) :
/// `versee` (02) veut une ligne, `aucune` (01, 03, 10, 60, 61, 90) n'en veut pas, un type sans
/// drapeau (50, 51) n'est pas confronte. Un type inconnu refuse.
pub fn desaccord_du_preavis(type_preavis: &str, versees: &[(String, String)]) -> Resultat<Option<String>> {
    let types = types_preavis()?;
    let tous = types.get("codes").and_then(Value::as_array).cloned().unwrap_or_default();
    let t = tous.iter().find(|c| en_texte(&c["code"]) == type_preavis).ok_or_else(|| {
        format!("Type de preavis {} inconnu de la norme (S21.G00.63.001). Les types sont dans fctu-base.json.", repr(type_preavis))
    })?;
    let nom = format!("« {} - {} »", type_preavis, en_texte(&t["libelle"]));
    let codes = codes_52_de_l_indemnite_de_preavis()?.join(" ou ");
    Ok(match t.get("indemnite_de_preavis").and_then(Value::as_str) {
        Some("versee") if versees.is_empty() => Some(format!(
            "le preavis se declare {} (S21.G00.63.001), et la paie de ce contrat ne verse aucune indemnite compensatrice \
de preavis (S21.G00.52.001 = {}) : le signalement dirait paye un preavis qui ne l'est pas. Si aucune indemnite n'est due \
pour ce motif (le solde de tout compte le dit), le type est « 03 - preavis non effectue et non paye » ; si elle est due, \
c'est la saisie qui le dit (`preavis_non_effectue`), et le solde la paie.",
            nom, codes
        )),
        Some("aucune") if !versees.is_empty() => {
            let lignes: Vec<String> = versees.iter().map(|(c, l)| format!("« {} » ({})", l, c)).collect();
            Some(format!(
                "le preavis se declare {} (S21.G00.63.001), et la paie de ce contrat verse {} : l'indemnite d'un preavis \
non effectue. Un preavis non effectue dont l'indemnite est versee se declare « 02 - preavis non effectue et paye ».",
                nom,
                lignes.join(", ")
            ))
        }
        _ => None,
    })
}
