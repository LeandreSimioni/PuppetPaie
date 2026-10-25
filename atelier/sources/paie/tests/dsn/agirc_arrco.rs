//! Corrections DSN, vague 1g (point 19 de la relecture) : Agirc-Arrco se declare en 131, l'Apec en 132.
//!
//! Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`) : la modalite Agirc-Arrco de
//! S21.G00.81.001 reserve « 105 - Montant de cotisation Regime Unifie Agirc-Arrco, y compris Apec » a la
//! « regularisation des periodes anterieures a 2025 » et attend « 131 - Cotisation regime unifie Agirc-Arrco » et
//! « 132 - Cotisation Apec » (p. 302, lignes 79-81 ; liste p. 305) ; CCH-12 de 81.004 (p. 309) : 131 et 132 portent
//! chacune leur montant. Agirc-Arrco ne renseigne 81.003 que « pour une reduction, exoneration » (p. 307).
//! Le rejeu des mois deposes des compagnies fictives : `compagnies fictives.rs`.

use crate::outils::{self, compagnie, dec, fictive, paires as lignes, versement};
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use engagement::{charger, Saisie};
use socle::num::Decimal;
use serde_json::{json, Value};

pub(crate) const AUDIENS_RETRAITE: &str = "44832375800038";

fn saisie(ctx: &Contexte, s: Value) -> Saisie {
    let chemin = outils::chemin_unique("agirc-arrco.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).unwrap()
}

/// Une ligne 81 : son code, son bloc 78 (rang dans le versement), son 81.002, son 81.003 s'il y en a un, son 81.004, son
/// 81.007.
#[derive(Debug, Clone, PartialEq)]
struct L81 {
    code: String,
    bloc78: usize,
    org: Option<String>,
    assiette: Option<String>,
    montant: Decimal,
    taux: Option<String>,
}

fn lignes_81(v: &[(String, String)]) -> Vec<L81> {
    let (mut b78, mut out) = (0usize, Vec::<L81>::new());
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.78.001" => b78 += 1,
            "S21.G00.81.001" => out.push(L81 { code: x.clone(), bloc78: b78, org: None, assiette: None, montant: Decimal::ZERO, taux: None }),
            "S21.G00.81.002" => out.last_mut().unwrap().org = Some(x.clone()),
            "S21.G00.81.003" => out.last_mut().unwrap().assiette = Some(x.clone()),
            "S21.G00.81.004" => out.last_mut().unwrap().montant = dec(x),
            "S21.G00.81.007" => out.last_mut().unwrap().taux = Some(x.clone()),
            _ => {}
        }
    }
    out
}

fn agirc_arrco(v: &[(String, String)]) -> Vec<L81> {
    lignes_81(v).into_iter().filter(|l| ["105", "131", "132"].contains(&l.code.as_str())).collect()
}

/// La somme, parts patronale et salariale, des lignes `ids` du bulletin.
fn bulletin(p: &Paie, ids: &[&str]) -> Decimal {
    p.lignes.iter().filter(|l| ids.contains(&l.id.as_str())).fold(Decimal::ZERO, |a, l| a + l.montant_pat + l.montant_sal)
}

fn paie<'a>(s: &'a Saisie, numero: &str) -> &'a Paie {
    s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some(numero)).unwrap()
}

fn sans_105(dsn: &str) {
    let codes: Vec<String> = lignes(dsn).into_iter().filter(|(k, _)| k == "S21.G00.81.001").map(|(_, v)| v).collect();
    assert!(!codes.iter().any(|c| c == "105"), "plus aucun 105 sur une periode 2025 ou 2026");
    assert!(codes.iter().any(|c| c == "131"), "Agirc-Arrco se declare en 131");
}

/// Le versement 20 de l'organisme `siret` (S21.G00.20.005, zero sans versement), et la somme des 81.004 de `codes` dans tout le fichier.
pub(crate) fn versement_20_et_lignes(dsn: &str, siret: &str, codes: &[&str]) -> (Decimal, Decimal) {
    let l = lignes(dsn);
    let (mut s, mut verse) = (String::new(), None);
    for (k, v) in &l {
        match k.as_str() {
            "S21.G00.20.001" => s = v.clone(),
            "S21.G00.20.005" if s == siret => verse = Some(dec(v)),
            _ => {}
        }
    }
    let (mut code, mut total) = (String::new(), Decimal::ZERO);
    for (k, v) in &l {
        match k.as_str() {
            "S21.G00.81.001" => code = v.clone(),
            "S21.G00.81.004" if codes.contains(&code.as_str()) => total += dec(v),
            _ => {}
        }
    }
    (verse.unwrap_or(Decimal::ZERO), total)
}

#[test]
fn un_cadre_et_un_non_cadre_permanents_declarent_131_et_132_aux_montants_du_bulletin() {
    // Montansier (cadre, CDI) a 5 000 EUR : tranche 2, CET et Apec non nulles. Nicolet (non cadre, CDI).
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 5000.0, "heures": 151.67}]},
            {"salarie": "Nicolet", "nature": "cdi", "debut": "2024-03-01", "numero": "NI.24.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2310.0, "heures": 151.67}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    sans_105(&dsn);

    // Le cadre : 131 de la tranche 1 (retraite T1 et CET), 132 (Apec) sous le meme bloc 78, puis le 131 de la tranche 2.
    let p = paie(&s, "MO.23.01");
    let a = agirc_arrco(&versement(&dsn, "MO.23.01"));
    let codes: Vec<&str> = a.iter().map(|l| l.code.as_str()).collect();
    assert_eq!(codes, ["131", "132", "131"], "{:?}", a);
    for (l, ids) in a.iter().zip([&["retraite_t1", "cet"][..], &["apec_t1"][..], &["retraite_t2"][..]]) {
        let attendu = bulletin(p, ids);
        assert!(attendu > Decimal::ZERO, "{:?} : le bulletin porte {:?}", l, ids);
        assert_eq!(l.montant, attendu, "{} : la somme des lignes {:?} du bulletin", l.code, ids);
        assert_eq!(l.assiette, None, "Agirc-Arrco : pas de 81.003 pour une cotisation (p. 307)");
    }
    assert_eq!(a[0].bloc78, a[1].bloc78, "l'Apec sous le bloc 78 du 131 de la tranche 1");
    assert_ne!(a[1].bloc78, a[2].bloc78);
    // Ensemble, ce que le 105 « y compris Apec » portait : la retraite, la CET et l'Apec du bulletin.
    let total: Decimal = a.iter().map(|l| l.montant).sum();
    assert_eq!(total, bulletin(p, &["retraite_t1", "retraite_t2", "cet", "apec_t1"]));

    // Le non cadre : deux 131, aucun 132.
    let q = paie(&s, "NI.24.01");
    let b = agirc_arrco(&versement(&dsn, "NI.24.01"));
    assert_eq!(b.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(), ["131", "131"], "{:?}", b);
    assert_eq!(b[0].montant, bulletin(q, &["retraite_t1"]));
    assert_eq!(bulletin(q, &["apec_t1", "cet"]), Decimal::ZERO, "un non cadre n'a ni Apec ni CET");

    // Le versement Agirc-Arrco : la copie figee nomme encore le seul 105 dans `config/organismes.json` (un fichier
    // compose avant ce lot) ; il porte quand meme les 131 et 132, et la reduction generale (106).
    let orgs: Value = serde_json::from_slice(&std::fs::read(fictive("fictive-3090-grande").join("config/organismes.json")).unwrap()).unwrap();
    let audiens = orgs["versements"].as_array().unwrap().iter().find(|v| v["siret"] == json!(AUDIENS_RETRAITE)).unwrap();
    assert_eq!(audiens["codes81"], json!(["105", "106"]));
    let (verse, lignes) = versement_20_et_lignes(&dsn, AUDIENS_RETRAITE, &["131", "132", "106"]);
    assert_eq!(verse, lignes, "le versement Agirc-Arrco porte les 131, 132 et 106 du fichier");
    assert!(verse > dec("500"));
}

#[test]
fn un_cadre_intermittent_declare_sa_retraite_en_131_et_son_apec_en_132() {
    // Biancolelli (artiste) et Servandoni (technicien) passes cadres dans une copie de la compagnie ; Clairon reste non
    // cadre. Juillet 2026 de la compagnie fictive.
    // Cadre aussi au classement conventionnel (04) : la CCH-11 du bloc 40 (p. 182) refuse un cadre (40.003 = 01)
    // reste classe 05, et le moteur la verifie avant de produire (vague V2b).
    let cadre = json!({"statut_categoriel_rc": "01", "statut_conventionnel": "04"});
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("agirc_arrco", "cadres"));
    outils::retoucher_fiches(&racine, &[("BIANCOLELLI", cadre.clone()), ("SERVANDONI", cadre)]);
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    sans_105(&dsn);

    for numero in ["BI4.26.06", "SE9.26.05"] {
        let p = paie(&s, numero);
        let a = agirc_arrco(&versement(&dsn, numero));
        assert_eq!(a.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(), ["131", "132"], "{} : {:?}", numero, a);
        assert_eq!(a[0].montant, bulletin(p, &["retraite_t1", "retraite_t2"]), "{} : 131 = la retraite du bulletin", numero);
        assert_eq!(a[1].montant, bulletin(p, &["apec_t1", "apec_t2"]), "{} : 132 = l'Apec du bulletin", numero);
        assert!(a[1].montant > Decimal::ZERO);
        assert_eq!(a[0].bloc78, a[1].bloc78, "{} : un seul bloc 78 pour la retraite et l'Apec", numero);
        assert!(a.iter().all(|l| l.assiette.is_none()));
    }
    // Le non cadre : un 131, au taux du bareme sur le brut, aucun 132.
    let c = agirc_arrco(&versement(&dsn, "CL6.26.07"));
    assert_eq!(c.iter().map(|l| l.code.as_str()).collect::<Vec<_>>(), ["131"], "{:?}", c);

    let (verse, lignes) = versement_20_et_lignes(&dsn, AUDIENS_RETRAITE, &["131", "132", "106"]);
    assert_eq!(verse, lignes);
}

#[test]
fn le_taux_d_une_organisation_patronale_se_pose_sur_le_131() {
    // `bareme::poser_t1` remplace le taux de la retraite T1 d'un intermittent non cadre quand l'organisation patronale
    // de la compagnie l'impose (SYNDEAC : 10,16 % pour un artiste) : sur la ligne du bulletin ET sur la cotisation DSN de
    // meme assiette, qui s'ecrit desormais en 131.
    use droit::bareme::bareme_applique;
    use droit::contexte::telle_qu_au;
    use droit::natures::nature;
    use droit::emplois::{emploi_de, EmploiEngagement};
    let jour = Jour::lire("2026-07-15").unwrap();
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(jour)).unwrap();
    let mut bar = ctx.bareme(&jour).unwrap();
    bar["_entreprise"]["organisation_patronale"] = json!("SYNDEAC");
    bar["_entreprise"]["organisation_patronale_depuis"] = json!("2020-01-01");
    let tqa = |f: &Value, j: &str| telle_qu_au(f, &Jour::lire(j)?);
    let sal = emploi_de(&ctx.salarie("BIANCOLELLI").unwrap(), Some("2026-07-15"), &EmploiEngagement::default(), &tqa).unwrap();
    let b = bareme_applique(bar, &sal, &nature(None).unwrap(), Some(&jour)).unwrap();
    let t1 = b["rubriques_bulletin"].as_array().unwrap().iter().find(|r| r["id"] == json!("retraite_t1")).unwrap();
    let total = dec(&t1["taux_pat"].to_string()) + dec(&t1["taux_sal"].to_string());
    let cot = b["cotisations_dsn"].as_array().unwrap().iter().find(|c| c["code81"] == json!("131")).unwrap();
    assert_eq!(dec(&cot["taux"].to_string()), total, "le 131 porte le taux de la ligne du bulletin");
    let ceg = &b["retraite_t1_repartition"]["ceg"];
    let ceg = dec(&ceg["pat"].to_string()) + dec(&ceg["sal"].to_string());
    assert_eq!(total - ceg, dec("10.16"), "le taux SYNDEAC d'un artiste, CEG en plus");
    assert!(!b["cotisations_dsn"].as_array().unwrap().iter().any(|c| c["code81"] == json!("105")));
}

// ------------------------------------------------------------------------------------------------------------------
// Vague V2d : la part patronale Agirc-Arrco en 142 et 146, a l'Urssaf.
//
// Cahier technique 2026.1 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`) : 142 et 146 sont des codes de la
// modalite URSSAF de 81.001 (p. 302, lignes 106-112 ; libelles p. 305, lignes 292 et 296) ; SIG-18 (p. 303, lignes
// 158-163) : un 131 sous une base 78 datee dans le mois principal veut au moins un 142 sous le meme versement ; 81.002 de
// l'Urssaf : « SIRET de l'Urssaf » (p. 307). Guide Urssaf du 04/06/2026 (`atelier/reference/sources/editeur-dsn/
// guide-urssaf-regularisation.pdf`, p. 34 et 37-38) : 142 = parts patronales retraite T1 et CEG T1, avec le taux ; 146 =
// parts patronales retraite T2, CEG T2 et CET (T1 et T2), plusieurs blocs 146 toleres ; p. 36 : la retraite salariale de
// tranche 1 se relit « 131 - 142 », le 131 garde donc les deux parts.

const URSSAF: &str = "78861779300013";
const LIGNES_RETRAITE: [&str; 3] = ["retraite_t1", "retraite_t2", "cet"];

/// La part patronale des lignes de retraite et de CET du bulletin, contrat par contrat (sans les contrats a zero).
pub(crate) fn patronale_par_contrat(paies: &[Paie]) -> std::collections::BTreeMap<String, Decimal> {
    let mut out = std::collections::BTreeMap::new();
    for p in paies {
        let m: Decimal = p.lignes.iter().filter(|l| LIGNES_RETRAITE.contains(&l.id.as_str())).map(|l| l.montant_pat).sum();
        if !m.is_zero() {
            *out.entry(p.engagement.numero.clone().unwrap_or_default()).or_insert(Decimal::ZERO) += m;
        }
    }
    out
}

/// SIG-18 (cahier technique 2026.1, p. 303, `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, l. 158-163) : un 131
/// sous une base 78 dont les deux dates de rattachement (78.002, 78.003) sont incluses dans le mois principal declare
/// (S20.G00.05.005) veut au moins un 142 sous le meme versement (50). Un 131 sous une base d'un autre mois -- un
/// versement de regularisation -- n'en veut pas. Et chaque 142 et 146 est a l'Urssaf.
pub(crate) fn sig_18(dsn: &str, quoi: &str) {
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

fn ligne<'a>(p: &'a Paie, id: &str) -> &'a engagement::calcul::LigneCotisation {
    p.lignes.iter().find(|l| l.id == id).unwrap_or_else(|| panic!("pas de ligne {} au bulletin", id))
}

fn taux3(x: f64) -> String {
    format!("{:.3}", x)
}

#[test]
fn un_intermittent_non_cadre_declare_sa_part_patronale_t1_en_142_a_l_urssaf() {
    // Juillet 2026 de la compagnie fictive : Clairon (artiste), Chaminade et Servandoni (techniciens), non cadres. Le 131
    // porte un taux sur le brut ; le 142, sous le meme bloc 78, la part patronale de la ligne de retraite T1 du bulletin,
    // son assiette et son taux patronal. Aucun 146 : un non cadre intermittent n'a pas de tranche 2.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &fictive("fictive-3090-grande").join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    sig_18(&dsn, "3090-grande 2026-07");
    for numero in ["CL6.26.07", "CH8.26.04", "SE9.26.05"] {
        let p = paie(&s, numero);
        let t1 = ligne(p, "retraite_t1");
        let l = lignes_81(&versement(&dsn, numero));
        let i = l.iter().position(|x| x.code == "131").unwrap();
        let x = &l[i + 1];
        assert_eq!(x.code, "142", "{} : le 142 suit le 131 : {:?}", numero, l);
        assert_eq!(x.bloc78, l[i].bloc78, "{} : sous le bloc 78 du 131", numero);
        assert_eq!(x.org.as_deref(), Some(URSSAF), "{} : 81.002 = l'Urssaf", numero);
        assert_eq!(x.assiette.as_deref(), Some(paie::dsn::mt(t1.base).as_str()), "{} : l'assiette de la ligne", numero);
        assert_eq!(x.taux, Some(taux3(t1.taux_pat.unwrap())), "{} : le taux patronal", numero);
        assert_eq!(x.montant, t1.montant_pat, "{} : la part patronale du bulletin", numero);
        assert!(x.montant > Decimal::ZERO);
        assert!(!l.iter().any(|x| x.code == "146"), "{} : pas de 146", numero);
    }
    // La somme des 81 du versement Agirc-Arrco ne bouge pas : 142 et 146 n'y vont pas.
    let (verse, lignes) = versement_20_et_lignes(&dsn, AUDIENS_RETRAITE, &["131", "132", "106"]);
    assert_eq!(verse, lignes);
}

#[test]
fn un_cadre_intermittent_au_dela_de_la_tranche_1_declare_142_et_146() {
    // Biancolelli passe cadre, un cachet de 250,00 (le bulletin CoolPaie 31/2026 de reference, `par_categorie.cadre`) :
    // T1 journaliere 220,00, T2 30,00, CET due (250 > 220) et FONDUE dans les deux lignes (5,44 et 14,78 patronaux).
    // 142 = retraite et CEG de T1 seules : 220 x 3,94 % = 8,67 + 220 x 1,29 % = 2,84 -> 11,51 (la ligne : 8,67 + 3,30 =
    // 11,97). 146 de la T2 : 30 x 12,95 % = 3,89 + 30 x 1,62 % = 0,49 -> 4,38 (la ligne : 3,89 + 0,55 = 4,44). 146 de la
    // CET : le reste, 0,46 + 0,06 = 0,52, sur 220 + 30, au taux de 0,21.
    // Un cadre se classe 04 (CCH-11 du bloc 40, lot V2b) : la fiche le dit, sinon la DSN refuse.
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("agirc_arrco", "142-146"));
    outils::retoucher_fiches(&racine, &[("BIANCOLELLI", json!({"statut_categoriel_rc": "01", "statut_conventionnel": "04"}))]);
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Biancolelli", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-20", "numero": "BI4.26.10",
             "date_paiement": "2026-10-20",
             "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 250.0, "lieu": "Theatre fictif", "dates": ["2026-10-20"]}]}
        ]}),
    );
    let p = paie(&s, "BI4.26.10");
    let (t1, t2) = (ligne(p, "retraite_t1"), ligne(p, "retraite_t2"));
    assert_eq!((t1.base, t2.base), (dec("220.00"), dec("30.00")));
    assert_eq!((t1.montant_pat, t2.montant_pat), (dec("11.97"), dec("4.44")), "la CET fondue dans les lignes");
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    sig_18(&dsn, "cadre intermittent");
    let l = lignes_81(&versement(&dsn, "BI4.26.10"));
    let a: Vec<&L81> = l.iter().filter(|x| ["131", "142", "146", "132"].contains(&x.code.as_str())).collect();
    let vu: Vec<(&str, Option<&str>, Decimal, Option<&str>)> =
        a.iter().map(|x| (x.code.as_str(), x.assiette.as_deref(), x.montant, x.taux.as_deref())).collect();
    assert_eq!(
        vu,
        vec![
            ("131", None, bulletin(p, &["retraite_t1", "retraite_t2"]), None),
            ("142", Some("220.00"), dec("11.51"), Some("5.230")),
            ("146", Some("30.00"), dec("4.38"), Some("14.570")),
            ("146", Some("250.00"), dec("0.52"), Some("0.210")),
            ("132", None, bulletin(p, &["apec_t1", "apec_t2"]), None),
        ],
        "{:?}",
        a
    );
    assert!(a.iter().all(|x| x.bloc78 == a[0].bloc78), "un seul bloc 78 (base 23) : {:?}", a);
    assert!(a[1..4].iter().all(|x| x.org.as_deref() == Some(URSSAF)));
    // Ensemble, la part patronale du bulletin, ni plus ni moins.
    assert_eq!(a[1].montant + a[2].montant + a[3].montant, t1.montant_pat + t2.montant_pat);
}

#[test]
fn un_cadre_permanent_declare_142_sous_la_tranche_1_et_146_sous_la_tranche_2_et_pour_la_cet() {
    // Montansier (cadre, CDI) a 5 000 EUR : tranche 1 (le plafond), tranche 2, CET sur le brut entier. Nicolet (non cadre)
    // a 2 310 EUR : sous le plafond, sa tranche 2 est nulle, pas de 146.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 5000.0, "heures": 151.67}]},
            {"salarie": "Nicolet", "nature": "cdi", "debut": "2024-03-01", "numero": "NI.24.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2310.0, "heures": 151.67}]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    sig_18(&dsn, "permanents");

    let p = paie(&s, "MO.23.01");
    let (t1, t2, cet) = (ligne(p, "retraite_t1"), ligne(p, "retraite_t2"), ligne(p, "cet"));
    let l = lignes_81(&versement(&dsn, "MO.23.01"));
    let a: Vec<&L81> = l.iter().filter(|x| ["131", "142", "146", "132"].contains(&x.code.as_str())).collect();
    let vu: Vec<(&str, Option<String>, Decimal, Option<String>)> =
        a.iter().map(|x| (x.code.as_str(), x.assiette.clone(), x.montant, x.taux.clone())).collect();
    let s3 = |l: &engagement::calcul::LigneCotisation| (Some(paie::dsn::mt(l.base)), l.montant_pat, Some(taux3(l.taux_pat.unwrap())));
    let (b1, m1, x1) = s3(t1);
    let (bc, mc, xc) = s3(cet);
    let (b2, m2, x2) = s3(t2);
    assert_eq!(
        vu,
        vec![
            ("131", None, bulletin(p, &["retraite_t1", "cet"]), None),
            ("142", b1, m1, x1),
            ("146", bc, mc, xc),
            ("132", None, bulletin(p, &["apec_t1"]), None),
            ("131", None, bulletin(p, &["retraite_t2"]), None),
            ("146", b2, m2, x2),
        ],
        "{:?}",
        a
    );
    assert_eq!(x_taux(&a[1]), "6.010", "T1 : 4,72 + 1,29");
    assert_eq!(x_taux(&a[2]), "0.210", "CET");
    assert_eq!(x_taux(&a[5]), "14.570", "T2 : 12,95 + 1,62");
    assert_eq!((a[0].bloc78, a[1].bloc78, a[2].bloc78, a[3].bloc78), (a[0].bloc78, a[0].bloc78, a[0].bloc78, a[0].bloc78));
    assert_eq!(a[5].bloc78, a[4].bloc78, "le 146 de la T2 sous le bloc 78 de la T2");
    assert_ne!(a[4].bloc78, a[0].bloc78);
    assert!([&a[1], &a[2], &a[5]].iter().all(|x| x.org.as_deref() == Some(URSSAF)));

    let q = paie(&s, "NI.24.01");
    let b = lignes_81(&versement(&dsn, "NI.24.01"));
    let codes: Vec<&str> = b.iter().filter(|x| ["131", "142", "146"].contains(&x.code.as_str())).map(|x| x.code.as_str()).collect();
    assert_eq!(codes, ["131", "142", "131"], "{:?}", b);
    assert_eq!(b.iter().find(|x| x.code == "142").unwrap().montant, ligne(q, "retraite_t1").montant_pat);

    let (verse, lignes) = versement_20_et_lignes(&dsn, AUDIENS_RETRAITE, &["131", "132", "106"]);
    assert_eq!(verse, lignes, "le versement Agirc-Arrco ne porte ni 142 ni 146");
}

fn x_taux(x: &L81) -> &str {
    x.taux.as_deref().unwrap_or("")
}

#[test]
fn le_142_porte_le_taux_patronal_d_une_organisation_patronale() {
    // SYNDEAC : `bareme::poser_t1` pose 10,16 % (CEG en plus) sur la retraite T1 d'un artiste ; le 142 porte la part et le
    // taux patronaux de cette ligne-la.
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("agirc_arrco", "syndeac"));
    let mut e = outils::lire(&racine, "config/entreprise.json");
    e["organisation_patronale"] = json!("SYNDEAC");
    e["organisation_patronale_depuis"] = json!("2020-01-01");
    outils::ecrire(&racine, "config/entreprise.json", &e);
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &racine.join("paies/2026-07.json")).unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let p = paie(&s, "CL6.26.07");
    let t1 = ligne(p, "retraite_t1");
    let l = lignes_81(&versement(&dsn, "CL6.26.07"));
    let x = l.iter().find(|x| x.code == "142").unwrap();
    assert_eq!((x.montant, x.taux.clone()), (t1.montant_pat, Some(taux3(t1.taux_pat.unwrap()))));
    assert_ne!(x.taux.as_deref(), Some("5.740"), "le taux SYNDEAC, pas celui du socle");
}
