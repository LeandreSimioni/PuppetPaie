//! La contribution CPF-CDD (code du travail L. 6331-6) : un contrat l'ouvre-t-il ? La meme regle pour le bulletin, le
//! nominatif (cotisation 129), le bordereau (CTP 987) et le bloc S21.G00.44 (013/014). Le droit la juge sur ce que la
//! DSN ecrit d'un contrat : nature (40.007), dispositif (40.008), motif de recours (40.021).

use socle::date::en_texte;
use socle::num::Resultat;

use crate::natures::Nature;

/// Un contrat ouvre-t-il la contribution CPF-CDD ? `qui` nomme le contrat dans un refus.
pub fn ouvre_cpf_cdd(nature: &str, dispositif: &str, motif: &str, qui: &str) -> Resultat<bool> {
    // « titulaires d'un contrat à durée déterminée » : la nature 02 (CDD de droit prive) seule.
    if nature != "02" {
        return Ok(false);
    }
    match motif {
        // L. 6331-55 : la participation unique des employeurs d'intermittents du spectacle, « par dérogation [...] à
        // l'article L. 6331-6 ». INFERENCE : le CDD d'usage (05) d'une compagnie est celui d'un intermittent.
        "05" => return Ok(false),
        // L. 6331-6 : « ceux visant les salariés occupant un emploi à caractère saisonnier défini au 3° de l'article
        // L. 1242-2 ». Le contrat vendanges (04) est un emploi saisonnier (INFERENCE).
        "03" | "04" => return Ok(false),
        "" => {
            return Err(format!(
                "{} : un CDD sans motif de recours (S21.G00.40.021). Le motif dit si le contrat ouvre la contribution \
CPF-CDD (code du travail L. 6331-6 : pas un CDD saisonnier) ; il se saisit sur l'engagement (`motif_recours`).",
                qui
            ))
        }
        _ => {}
    }
    // D. 6331-72 : contrats d'accompagnement dans l'emploi, d'apprentissage, de professionnalisation, et ceux de
    // L. 6321-9 -- tous portent un dispositif de politique publique (40.008), et le moteur n'en ecrit aucun.
    if !dispositif.is_empty() && dispositif != "99" {
        return Err(format!(
            "{} : un CDD sous le dispositif de politique publique {} (S21.G00.40.008). D. 6331-72 du code du travail en \
exclut certains de la contribution CPF-CDD (contrats d'accompagnement dans l'emploi, d'apprentissage, de \
professionnalisation, ceux de L. 6321-9) : le moteur ne sait pas lequel c'est, et ne declare pas le bloc \
S21.G00.44 sur une supposition.",
            qui, dispositif
        ));
    }
    Ok(true)
}

/// Ce que la DSN ecrit d'un contrat de nature `nat` : nature, dispositif, motif (celui de la nature, sinon celui de
/// l'engagement).
pub fn codes_de(nat: &Nature, motif_recours: Option<&str>) -> (String, String, String) {
    let code = |r: &str| -> String {
        nat.codes.get(r).map(|v| match v.get("valeur") {
            Some(x) => en_texte(x),
            None => en_texte(v),
        }).unwrap_or_default()
    };
    let motif = match code("S21.G00.40.021") {
        m if !m.is_empty() => m,
        _ => motif_recours.unwrap_or_default().to_string(),
    };
    (code("S21.G00.40.007"), code("S21.G00.40.008"), motif)
}

/// Le contrat de nature `nat` paie-t-il la contribution CPF-CDD ? Un CDD sans motif de recours refuse (le motif dit
/// s'il est saisonnier).
pub fn contrat_ouvre_cpf_cdd(nat: &Nature, motif_recours: Option<&str>, qui: &str) -> Resultat<bool> {
    let (n, d, m) = codes_de(nat, motif_recours);
    ouvre_cpf_cdd(&n, &d, &m, qui)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_cpf_cdd_suit_la_nature_et_le_motif() {
        assert!(!ouvre_cpf_cdd("01", "99", "", "x").unwrap(), "un CDI");
        assert!(!ouvre_cpf_cdd("02", "99", "05", "x").unwrap(), "un CDD d'usage : L. 6331-55");
        assert!(!ouvre_cpf_cdd("02", "99", "03", "x").unwrap(), "un saisonnier : L. 6331-6");
        assert!(ouvre_cpf_cdd("02", "99", "02", "x").unwrap(), "un accroissement d'activite");
        assert!(ouvre_cpf_cdd("02", "99", "", "x").is_err(), "sans motif, on ne sait pas");
        assert!(ouvre_cpf_cdd("02", "64", "11", "x").is_err(), "un apprenti : D. 6331-72");
    }
}
