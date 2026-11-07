//! La contribution CPF-CDD (code du travail L. 6331-6) : un contrat l'ouvre-t-il ? La meme regle pour le bulletin, le
//! nominatif (cotisation 129), le bordereau (CTP 987) et le bloc S21.G00.44 (013/014). Le droit la juge sur ce que la
//! DSN ecrit d'un contrat : nature (40.007), dispositif (40.008), motif de recours (40.021).

use socle::num::Resultat;

use crate::natures::Nature;

/// Le contrat d'un salarie dans un message : « le contrat VA5.26.01 de VARENNE », « le contrat de VARENNE » tant qu'il n'a
/// pas de numero.
pub fn le_contrat(numero: Option<&str>, salarie: &str) -> String {
    match numero.filter(|n| !n.is_empty()) {
        Some(n) => format!("le contrat {} de {}", n, salarie),
        None => format!("le contrat de {}", salarie),
    }
}

/// Un contrat ouvre-t-il la contribution CPF-CDD ? `qui` nomme le contrat dans un refus (`le_contrat`).
pub fn ouvre_cpf_cdd(nature: Option<&str>, dispositif: Option<&str>, motif: Option<&str>, qui: &str) -> Resultat<bool> {
    let Some(nature) = nature else {
        return Err(format!("La nature de {} ne dit pas son code (S21.G00.40.007) : le signaler au développeur.", qui));
    };
    // « titulaires d'un contrat à durée déterminée » : la nature 02 (CDD de droit prive) seule.
    if nature != "02" {
        return Ok(false);
    }
    match motif {
        // L. 6331-55 : la participation unique des employeurs d'intermittents du spectacle, « par dérogation [...] à
        // l'article L. 6331-6 ». INFERENCE : le CDD d'usage (05) d'une compagnie est celui d'un intermittent.
        Some("05") => return Ok(false),
        // L. 6331-6 : « ceux visant les salariés occupant un emploi à caractère saisonnier défini au 3° de l'article
        // L. 1242-2 ». Le contrat vendanges (04) est un emploi saisonnier (INFERENCE).
        Some("03" | "04") => return Ok(false),
        None => {
            return Err(format!(
                "Dites pourquoi ce CDD est conclu (« Motif du CDD », sur la carte de l'engagement) : {} n'en dit pas le motif.\n\
Le motif dit si le contrat ouvre la contribution CPF-CDD (code du travail, L. 6331-6 : pas un CDD saisonnier) ; la DSN \
le déclare (S21.G00.40.021).",
                qui
            ))
        }
        _ => {}
    }
    // D. 6331-72 : contrats d'accompagnement dans l'emploi, d'apprentissage, de professionnalisation, et ceux de
    // L. 6321-9 -- tous portent un dispositif de politique publique (40.008), et le moteur n'en ecrit aucun.
    if let Some(dispositif) = dispositif.filter(|d| *d != "99") {
        return Err(format!(
            "Un CDD sous le dispositif de politique publique {1} (S21.G00.40.008), {0}. D. 6331-72 du code du travail en \
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
/// Un code que la nature ni l'engagement ne disent est `None`.
pub fn codes_de(nat: &Nature, motif_recours: Option<&str>) -> (Option<String>, Option<String>, Option<String>) {
    let motif = nat.code("S21.G00.40.021").or_else(|| motif_recours.filter(|m| !m.is_empty()).map(str::to_string));
    (nat.code("S21.G00.40.007"), nat.code("S21.G00.40.008"), motif)
}

/// Le contrat de nature `nat` paie-t-il la contribution CPF-CDD ? C'est la ou le calcul lit le motif de recours saisi :
/// un CDD sans motif refuse (le motif dit s'il est saisonnier), un motif que la nature n'a pas aussi
/// (`Nature::motif_recours_refuse`).
pub fn contrat_ouvre_cpf_cdd(nat: &Nature, motif_recours: Option<&str>, qui: &str) -> Resultat<bool> {
    if let Some(refus) = nat.code("S21.G00.40.021").is_none().then(|| nat.motif_recours_refuse(motif_recours.unwrap_or_default(), qui)).flatten() {
        return Err(refus);
    }
    let (n, d, m) = codes_de(nat, motif_recours);
    ouvre_cpf_cdd(n.as_deref(), d.as_deref(), m.as_deref(), qui)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_cpf_cdd_suit_la_nature_et_le_motif() {
        assert!(!ouvre_cpf_cdd(Some("01"), Some("99"), None, "x").unwrap(), "un CDI");
        assert!(!ouvre_cpf_cdd(Some("02"), Some("99"), Some("05"), "x").unwrap(), "un CDD d'usage : L. 6331-55");
        assert!(!ouvre_cpf_cdd(Some("02"), Some("99"), Some("03"), "x").unwrap(), "un saisonnier : L. 6331-6");
        assert!(ouvre_cpf_cdd(Some("02"), Some("99"), Some("02"), "x").unwrap(), "un accroissement d'activite");
        assert!(ouvre_cpf_cdd(Some("02"), Some("99"), None, "x").is_err(), "sans motif, on ne sait pas");
        assert!(ouvre_cpf_cdd(Some("02"), Some("64"), Some("11"), "x").is_err(), "un apprenti : D. 6331-72");
        assert!(ouvre_cpf_cdd(None, None, None, "x").is_err(), "sans nature, on ne sait pas");
    }
}
