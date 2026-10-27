//! Les contrats d'organismes (les adhesions de prevoyance et de sante, bloc S21.G00.15 de la DSN) sont dates : un
//! contrat porte `depuis` (le premier jour ou il vaut) et, s'il y a lieu, `jusqu_a` (le dernier). Le contrat de
//! Thalie Sante des intermittents, par exemple, n'existe qu'a partir du 01/01/2026 : la fiche de parametrage Audiens
//! version 139 le dit (`ParametresContrats DateDebutValidite="01012026"`, contrat `TSINTERMITTENTS` ;
//! `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`, l. 305-307), et
//! `logiciel/donnees/organismes-base.json` le porte (`depuis`).
//!
//! UN SEUL CRITERE, partout : un contrat vaut pour une periode qu'il couvre, au moins un jour. Le calcul et la DSN
//! d'un mois ne lisent que ceux-la, une affiliation (bloc S21.G00.70) ne rattache qu'a eux, et la Configuration ne
//! propose un contrat que pour la periode qu'il couvre. Un contrat qui commence dans la periode la couvre : son
//! affiliation se date de son premier jour (S21.G00.70.014, `dsn::rattachements`).

use serde_json::Value;

use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::Resultat;

/// La date de `cle` du contrat, ou `None` quand il n'en dit pas ; refus quand elle ne se lit pas.
fn borne(contrat: &Value, cle: &str) -> Resultat<Option<Jour>> {
    match contrat.get(cle).filter(|v| vrai(v)) {
        None => Ok(None),
        Some(v) => Jour::lire(en_texte(v).trim())
            .map(Some)
            .map_err(|e| format!("Le contrat d'organisme {} : `{}` ne se lit pas ({}).", en_texte(contrat.get("libelle").unwrap_or(&Value::Null)), cle, e)),
    }
}

/// Le contrat couvre-t-il au moins un jour de la periode `debut`..=`fin` ?
pub fn en_vigueur(contrat: &Value, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    let depuis = borne(contrat, "depuis")?;
    let jusqu_a = borne(contrat, "jusqu_a")?;
    Ok(depuis.map_or(true, |d| d <= *fin) && jusqu_a.map_or(true, |j| j >= *debut))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn j(s: &str) -> Jour {
        Jour::lire(s).unwrap()
    }

    /// Thalie Sante des intermittents n'existe qu'a partir du 01/01/2026 (`DateDebutValidite="01012026"`,
    /// `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`, l. 305) : decembre 2025 ne la
    /// connait pas, janvier 2026 si ; un contrat clos le 30/06 ne couvre plus juillet ; un contrat ouvert le 15 couvre
    /// le mois ou il s'ouvre.
    #[test]
    fn un_contrat_ne_vaut_que_pour_la_periode_qu_il_couvre() {
        let thalie = json!({"libelle": "TSINTERMITTENTS", "depuis": "2026-01-01"});
        assert!(!en_vigueur(&thalie, &j("2025-12-01"), &j("2025-12-31")).unwrap());
        assert!(en_vigueur(&thalie, &j("2026-01-01"), &j("2026-01-31")).unwrap());
        let clos = json!({"libelle": "X", "depuis": "2025-01-01", "jusqu_a": "2026-06-30"});
        assert!(en_vigueur(&clos, &j("2026-06-01"), &j("2026-06-30")).unwrap());
        assert!(!en_vigueur(&clos, &j("2026-07-01"), &j("2026-07-31")).unwrap());
        assert!(en_vigueur(&json!({"depuis": "2026-03-15"}), &j("2026-03-01"), &j("2026-03-31")).unwrap());
        assert!(en_vigueur(&json!({"libelle": "INTERBRANCHE"}), &j("2025-01-01"), &j("2025-01-31")).unwrap());
        assert!(en_vigueur(&json!({"depuis": "2026-13-01"}), &j("2026-01-01"), &j("2026-01-31")).is_err());
    }
}
