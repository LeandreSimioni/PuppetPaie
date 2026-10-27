//! Le bordereau Urssaf se calcule : le montant du bloc 22 (et de l'ordre 20) est la somme des lignes 23, chacune
//! valorisee au taux de la table des CTP de l'Urssaf et arrondie a l'euro. Contre les 75 bordereaux que CoolPaie a
//! deposes et que l'Urssaf a preleves (`bordereaux-reels.json` : les blocs 22 et 23 tels qu'ecrits, rien d'autre) ; et
//! contre `bordereau.json`, un releve du moteur sur la table livree (chaque CTP, un refus par motif, des totaux), que
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit.

use crate::outils::{dec, egal, lire_oracle, oracle_selon, reecrire_oracles};
use paie::bordereau::{montant, montant_ligne, LigneBordereau};
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
        assert_eq!(paie::bordereau::euro(dec(avant)), dec(apres), "{} EUR", avant);
    }
}

#[test]
fn un_montant_negatif_s_arrondit_comme_son_oppose_par_inference() {
    // INFERENCE (`atelier/reference/sources/arrondi-euro/LISEZ-MOI.md`) : L. 133-10 dit « la fraction d'euro egale a 0,50
    // est comptee pour 1 » sans dire le signe, et aucun texte depose ne le dit. Le moteur arrondit -2,50 comme 2,50, au
    // signe pres : une regularisation qui retire ce qu'un mois a declare retire le meme euro.
    for (avant, apres) in [("-0.50", "-1"), ("-2.50", "-3"), ("-2.49", "-2"), ("-2.51", "-3")] {
        assert_eq!(paie::bordereau::euro(dec(avant)), dec(apres), "{} EUR", avant);
        assert_eq!(paie::bordereau::euro(dec(avant)), -paie::bordereau::euro(-dec(avant)), "{} EUR", avant);
    }
}

/// Le << mystere d'avril >> (+10 a +14 EUR chaque annee) : c'est la ligne 995,
/// le solde de taxe d'apprentissage, 0,09 % de 17 156 = 15 EUR.
#[test]
fn avril_2026_au_centime_avec_le_solde_de_taxe_d_apprentissage() {
    let lignes = [
        l("027", "920", "520.00", ""), l("260", "920", "517.00", ""),
        l("312", "920", "520.00", "01.05"), l("312", "921", "520.00", ""),
        l("334", "921", "580.00", ""), l("432", "920", "520.00", ""),
        l("636", "920", "520.00", ""), l("983", "920", "520.00", ""),
        l("992", "920", "572.00", ""), l("995", "920", "17156.00", ""),
    ];
    assert_eq!(montant(&lignes, &jour("2026-04-01")).unwrap().to_string(), "217.00");
}

/// 2654 x 9,31 % = 247,09 -> 247 ; 2654 x 1,16 % = 30,79 -> 31 : 278. En
/// additionnant les taux, 2654 x 10,47 % = 277,87 -> 278 ici, mais la regle
/// faisait 60 bordereaux sur 75 au lieu de 74.
#[test]
fn le_taux_at_s_arrondit_a_part() {
    let m = montant_ligne(&l("312", "920", "2654.00", "01.16"), &jour("2025-12-01")).unwrap();
    assert_eq!(m, dec("278"));
}

/// Format F (671, reduction generale) : un montant, en deduction.
#[test]
fn une_reduction_se_deduit() {
    let ligne = LigneBordereau { assiette: None, montant: Some(dec("15.00")), ..l("671", "921", "0", "") };
    assert_eq!(montant_ligne(&ligne, &jour("2026-05-01")).unwrap(), dec("-15"));
}

/// L'AGS passe a 0,25 le << 2024-01-07 >> de la table : le 1er juillet 2024.
/// Deux lignes restent ouvertes (0,20 depuis le 01/01, 0,25 depuis le 01/07) :
/// la plus recente l'emporte.
#[test]
fn les_dates_de_la_table_sont_en_aaaa_jj_mm() {
    let ags = l("937", "920", "10000.00", "");
    assert_eq!(montant_ligne(&ags, &jour("2024-06-30")).unwrap(), dec("20"));
    assert_eq!(montant_ligne(&ags, &jour("2024-07-01")).unwrap(), dec("25"));
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

/// Ce que dit un refus, sans le CTP qu'il nomme : « absent de la table », « pas de taux plafonne », « format R »...
fn motif(refus: &str) -> String {
    refus.split_once(" : ").map_or(refus, |(_, m)| m).chars().filter(|c| !c.is_ascii_digit()).collect()
}

fn resultat<T: ToString>(r: Result<T, String>) -> Value {
    Value::String(r.map_or_else(|e| format!("ERR:{}", e), |x| x.to_string()))
}

#[test]
fn chaque_ctp_de_la_table_et_des_totaux() {
    let v = lire_oracle("bordereau.json");
    let (mut lignes, mut motifs) = (Vec::new(), std::collections::BTreeSet::new());
    for c in v["lignes"].as_array().unwrap() {
        let vu = montant_ligne(&ligne(c), &jour(c[5].as_str().unwrap()));
        if let Err(e) = &vu {
            if !motifs.insert(motif(e)) && reecrire_oracles() {
                continue;
            }
        }
        let mut c = c.clone();
        let vu = resultat(vu);
        if !egal(&c[6], &vu) {
            c[6] = vu; // « -0 » et « 0 » sont le meme montant : l'oracle garde sa forme
        }
        lignes.push(c);
    }
    let mut totaux = v["totaux"].clone();
    for g in totaux.as_array_mut().unwrap() {
        let l: Vec<LigneBordereau> = g["lignes"].as_array().unwrap().iter().map(ligne).collect();
        g["attendu"] = resultat(montant(&l, &jour(g["jour"].as_str().unwrap())));
    }
    assert!(lignes.len() > 3000 && motifs.len() >= 5 && totaux.as_array().unwrap().len() > 100, "{} {:?}", lignes.len(), motifs);
    oracle_selon("bordereau.json", "/lignes", &Value::Array(lignes), egal);
    oracle_selon("bordereau.json", "/totaux", &totaux, egal);
}
