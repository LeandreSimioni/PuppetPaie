//! Agirc-Arrco se declare en 131, l'Apec en 132.
//!
//! Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`) : la modalite Agirc-Arrco de
//! S21.G00.81.001 reserve « 105 - Montant de cotisation Regime Unifie Agirc-Arrco, y compris Apec » a la
//! « regularisation des periodes anterieures a 2025 » et attend « 131 - Cotisation regime unifie Agirc-Arrco » et
//! « 132 - Cotisation Apec » (p. 302, lignes 79-81 ; liste p. 305) ; CCH-12 de 81.004 (p. 309) : 131 et 132 portent
//! chacune leur montant. Agirc-Arrco ne renseigne 81.003 que « pour une reduction, exoneration » (p. 307).

use crate::outils::paires as lignes;

const URSSAF: &str = "78861779300013";

/// SIG-18 (cahier technique 2026.1, p. 303, `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, l. 158-163) : un 131
/// sous une base 78 dont les deux dates de rattachement (78.002, 78.003) sont incluses dans le mois principal declare
/// (S20.G00.05.005) veut au moins un 142 sous le meme versement (50). Un 131 sous une base d'un autre mois -- un
/// versement de regularisation -- n'en veut pas. Et chaque 142 et 146 est a l'Urssaf.
fn sig_18(dsn: &str, quoi: &str) {
    struct Versement {
        un_131_du_mois: bool,
        un_142: bool,
    }
    let mois = |v: &str| v.get(2..).unwrap_or("").to_string();
    let (mut principal, mut debut, mut fin) = (String::new(), String::new(), String::new());
    let mut versements: Vec<Versement> = Vec::new();
    let mut code = String::new();
    for (k, v) in lignes(dsn) {
        match k.as_str() {
            "S20.G00.05.005" => principal = mois(&v),
            "S21.G00.50.001" => versements.push(Versement { un_131_du_mois: false, un_142: false }),
            k if k.starts_with("S21.G00.30.") => code.clear(),
            "S21.G00.78.001" => {
                debut.clear();
                fin.clear();
            }
            "S21.G00.78.002" => debut = mois(&v),
            "S21.G00.78.003" => fin = mois(&v),
            "S21.G00.81.001" => {
                code = v.clone();
                if let Some(x) = versements.last_mut() {
                    x.un_131_du_mois |= v == "131" && !principal.is_empty() && debut == principal && fin == principal;
                    x.un_142 |= v == "142";
                }
            }
            "S21.G00.81.002" if code == "142" || code == "146" => assert_eq!(v, URSSAF, "{} : {} a l'Urssaf", quoi, code),
            _ => {}
        }
    }
    for (i, v) in versements.iter().enumerate() {
        assert!(!v.un_131_du_mois || v.un_142, "{} : le versement {} porte un 131 du mois principal sans 142", quoi, i + 1);
    }
}

#[test]
fn sig_18_n_exige_le_142_que_sous_une_base_datee_dans_le_mois_principal() {
    let versement = |debut: &str, fin: &str, avec_142: bool| {
        format!(
            "S21.G00.50.001,'01'\nS21.G00.78.001,'03'\nS21.G00.78.002,'{}'\nS21.G00.78.003,'{}'\nS21.G00.81.001,'131'\n{}",
            debut,
            fin,
            if avec_142 { format!("S21.G00.81.001,'142'\nS21.G00.81.002,'{}'\n", URSSAF) } else { String::new() }
        )
    };
    let dsn = |v: String| format!("S20.G00.05.005,'01102026'\n{}", v);
    // Un 131 du mois principal avec son 142 ; un 131 de regularisation (septembre) sans 142 : la norme ne l'exige pas.
    sig_18(&dsn(versement("01102026", "31102026", true)), "du mois, avec 142");
    sig_18(&dsn(versement("01092026", "30092026", false)), "regularisation, sans 142");
    // Un 131 du mois principal sans 142 : SIG-18.
    let refuse = std::panic::catch_unwind(|| sig_18(&dsn(versement("01102026", "31102026", false)), "du mois, sans 142"));
    assert!(refuse.is_err());
}
