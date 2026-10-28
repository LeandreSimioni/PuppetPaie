//! Le bordereau Urssaf se calcule : le montant du bloc 22 (et de l'ordre 20) est la somme des lignes 23, chacune
//! valorisee au taux de la table des CTP de l'Urssaf et arrondie a l'euro. Contre les 75 bordereaux que CoolPaie a
//! deposes et que l'Urssaf a preleves (`bordereaux-reels.json` : les blocs 22 et 23 tels qu'ecrits, rien d'autre).

use crate::outils::{dec, lire_oracle};
use documents::bordereau::{montant, LigneBordereau};
use socle::date::Jour;
use serde_json::Value;

fn jour(s: &str) -> Jour {
    Jour::lire(s).unwrap()
}

fn l(code: &str, qualifiant: &str, assiette: &str, taux_at: &str) -> LigneBordereau {
    LigneBordereau {
        code: code.into(),
        qualifiant: qualifiant.into(),
        taux_at: taux_at.into(),
        assiette: Some(dec(assiette)),
        montant: None,
        commune: None,
    }
}

/// CSS L. 133-10 (`code-de-la-securite-sociale/articles-20261013.md`) : « arrondi a l'euro le plus proche. La fraction
/// d'euro egale a 0,50 est comptee pour 1 » -- et non au pair : 0,50 donne 1, 1,50 donne 2, 2,50 donne 3.
#[test]
fn la_fraction_d_euro_egale_a_cinquante_centimes_est_comptee_pour_un() {
    for (avant, apres) in [("0.50", "1"), ("1.50", "2"), ("2.50", "3"), ("2.49", "2"), ("2.51", "3"), ("1000.50", "1001"), ("0.49", "0")] {
        assert_eq!(documents::bordereau::euro(dec(avant)), dec(apres), "{} EUR", avant);
    }
}

#[test]
fn un_montant_negatif_s_arrondit_comme_son_oppose_par_inference() {
    // INFERENCE (`atelier/reference/sources/arrondi-euro/LISEZ-MOI.md`) : L. 133-10 dit « la fraction d'euro egale a 0,50
    // est comptee pour 1 » sans dire le signe, et aucun texte depose ne le dit. Le moteur arrondit -2,50 comme 2,50, au
    // signe pres : une regularisation qui retire ce qu'un mois a declare retire le meme euro.
    for (avant, apres) in [("-0.50", "-1"), ("-2.50", "-3"), ("-2.49", "-2"), ("-2.51", "-3")] {
        assert_eq!(documents::bordereau::euro(dec(avant)), dec(apres), "{} EUR", avant);
        assert_eq!(documents::bordereau::euro(dec(avant)), -documents::bordereau::euro(-dec(avant)), "{} EUR", avant);
    }
}

#[test]
fn un_ctp_inconnu_fait_refuser() {
    let e = montant(&[l("000", "920", "100.00", "")], &jour("2026-09-01")).unwrap_err();
    assert!(e.contains("000") && e.contains("Rien n'est ecrit"), "{}", e);
}

/// Une ligne d'oracle : code, qualifiant, taux AT, assiette, montant (vides s'ils manquent).
fn ligne(c: &Value) -> LigneBordereau {
    let champ = |i: usize| c[i].as_str().unwrap();
    LigneBordereau {
        code: champ(0).into(),
        qualifiant: champ(1).into(),
        taux_at: champ(2).into(),
        assiette: Some(champ(3)).filter(|s| !s.is_empty()).map(dec),
        montant: Some(champ(4)).filter(|s| !s.is_empty()).map(dec),
        commune: None,
    }
}

/// Octobre 2025 : 393 declares et preleves, 394 calcules. Les lignes arrondies une a une font 394, leur somme brute
/// 393,23 : un euro que CoolPaie a arrondi autrement ce mois-la, et qu'aucune variante n'explique sans casser d'autres
/// bordereaux. L'exception connue, et la seule : (jour, declare, calcule).
const OCTOBRE_2025: (&str, &str, &str) = ("2025-10-01", "393.00", "394.00");

#[test]
fn les_75_bordereaux_preleves_se_recalculent_au_centime_sauf_octobre_2025() {
    let v = lire_oracle("bordereaux-reels.json");
    let bordereaux = v["bordereaux"].as_array().unwrap();
    assert_eq!(bordereaux.len(), 75);
    let mut ecarts = Vec::new();
    for b in bordereaux {
        let le_jour = b["jour"].as_str().unwrap();
        let declare = b["declare"].as_str().unwrap();
        let lignes: Vec<LigneBordereau> = b["lignes"].as_array().unwrap().iter().map(ligne).collect();
        let calcule = montant(&lignes, &jour(le_jour)).unwrap_or_else(|e| panic!("{} : {}", le_jour, e));
        if calcule != dec(declare) {
            ecarts.push((le_jour, declare, calcule.to_string()));
        }
    }
    let attendus = [(OCTOBRE_2025.0, OCTOBRE_2025.1, OCTOBRE_2025.2.to_string())];
    assert_eq!(ecarts, attendus);
}
