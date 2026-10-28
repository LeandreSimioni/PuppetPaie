//! Les nets du bloc 50 et le montant net social du bloc 58, sur la paie et le bulletin.
//!
//! - La remuneration nette fiscale (50.002) part du brut soumis, deduit les cotisations salariales et reintegre la
//!   CSG/CRDS non deductible et la part patronale sante (norme DSN 2026.1, § 2.3.2, p. 60-61) ; des lignes hors
//!   assiette, elle ne prend que les imposables (ni defraiement, ni acompte, ni indemnite exoneree : CGI 80 duodecies).
//!   50.004 = 50.002 moins ce qu'elle reintegre (p. 244). 50.012 est reservee aux assistants maternels (p. 245).
//! - Le montant net social (58.004, type 03) : decret n° 2023-1378 (CSS R. 844-1 II ; C. trav. R. 3243-1, 9° bis) et FAQ
//!   du BOSS : les sommes brutes, exonerees ou non (indemnites de rupture comprises), moins les cotisations salariales ;
//!   ni les frais professionnels, ni les mouvements de tresorerie, ni le financement patronal des garanties. Le moteur
//!   lit ce qu'est une ligne hors assiette dans la `nature` de sa rubrique ; une autre nature se refuse (regle 1).
//!
//! Sur la copie figee des compagnies fictives (`tests/donnees/fictives/`) ; saisies et compagnie retouchee dans le dossier des tests.

use paie::bulletin::{fmt_eur, jeux_et_description};
use engagement::calcul::Paie;
use droit::catalogue::{catalogue, HorsAssiette};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use engagement::{charger, Saisie};
use socle::num::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::outils::{dec, fictive, versement};
use droit::pas::{du_versement, ZONE_DEFAUT};
use std::path::PathBuf;

/// Ecrit une saisie dans le dossier temporaire des tests (jamais dans la copie figee).
fn ecrire_saisie(s: &Value) -> PathBuf {
    let chemin = crate::outils::chemin_unique("net-social.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    chemin
}

fn saisie(ctx: &Contexte, s: Value) -> Saisie {
    crate::outils::charger_fichier(ctx, &ecrire_saisie(&s)).unwrap()
}

fn rubrique(v: &[(String, String)], r: &str) -> Option<Decimal> {
    crate::outils::valeur(v, r).map(|x| dec(&x))
}

/// Ce que la norme reintegre au net des cotisations : CSG/CRDS non deductible, part patronale sante.
fn reintegre(p: &Paie) -> Decimal {
    somme(p, "csg_crds_nd", false) + somme(p, "mutuelle", true)
}

/// Les 58.004 du versement, avec le type (58.003) qui les precede.
fn nets_sociaux(v: &[(String, String)]) -> Vec<(String, Decimal)> {
    let mut type_58 = String::new();
    let mut out = Vec::new();
    for (k, x) in v {
        if k == "S21.G00.58.003" {
            type_58 = x.clone();
        } else if k == "S21.G00.58.004" {
            out.push((type_58.clone(), dec(x)));
        }
    }
    out
}

fn textes(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| textes(x, out)),
        Value::Object(m) => m.values().for_each(|x| textes(x, out)),
        _ => {}
    }
}

/// Le premier montant que le bulletin imprime apres le libelle `libelle`.
fn imprime_apres(desc: &Value, libelle: &str) -> String {
    let mut t = Vec::new();
    textes(desc, &mut t);
    let i = t.iter().position(|x| x == libelle).unwrap_or_else(|| panic!("le bulletin n'imprime pas {:?}", libelle));
    t[i + 1..].iter().find(|x| x.contains(',') && x.chars().any(|c| c.is_ascii_digit())).cloned().unwrap()
}

fn imprime(desc: &Value, libelle: &str) -> bool {
    let mut t = Vec::new();
    textes(desc, &mut t);
    t.iter().any(|x| x == libelle)
}

fn somme(p: &Paie, id: &str, patronale: bool) -> Decimal {
    p.lignes.iter().filter(|l| l.id == id).fold(Decimal::ZERO, |a, l| a + if patronale { l.montant_pat } else { l.montant_sal })
}

fn bulletin(ctx: &Contexte, p: &Paie, mois: &str) -> Value {
    let ent = ctx.entreprise_du_mois(mois).unwrap();
    jeux_et_description(ctx, std::slice::from_ref(p), &ent, mois, None, false, &HashMap::new()).unwrap()
}

#[test]
fn defraiement_acompte_et_indemnite_dans_les_nets_fiscal_et_social() {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [
                {"rubrique": "mois", "quantite": 1, "taux": 3120.0, "heures": 151.67},
                {"rubrique": "defraiement_forfait", "quantite": 3, "taux": 20.7},
                {"rubrique": "acompte", "quantite": 1, "taux": 500.0},
                {"rubrique": "indemnite_licenciement", "quantite": 1, "taux": 1000.0}
             ]},
            {"salarie": "Nicolet", "nature": "cdi", "debut": "2024-03-01", "numero": "NI.24.01", "quotite_contrat": 151.67,
             "lignes": [
                {"rubrique": "mois", "quantite": 1, "taux": 2310.0, "heures": 151.67},
                {"rubrique": "indemnite_rupture_conventionnelle_imposable", "quantite": 1, "taux": 800.0}
             ]}
        ]}),
    );
    let dsn = generer(&ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();

    // Montansier : defraiement 62,10 (frais), acompte 500 (mouvement), indemnite de licenciement 1 000 (exoneree, mais
    // un revenu : Q 2.2).
    let p = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("MO.23.01")).unwrap();
    assert!(p.total_sal > Decimal::ZERO);
    assert!(somme(p, "mutuelle", true) > Decimal::ZERO && somme(p, "prevoyance", true) > Decimal::ZERO, "le financement patronal des garanties existe, et n'entre pas");
    let net_social = p.brut - p.total_sal + dec("1000.00");
    let net_a_payer = p.brut - p.total_sal + dec("62.10") - dec("500.00") + dec("1000.00");
    let v = versement(&dsn, "MO.23.01");
    assert_eq!(nets_sociaux(&v), vec![("03".to_string(), net_social)], "58.004 : brut - cotisations salariales + l'indemnite, sans defraiement ni acompte");
    assert_eq!(p.net_a_payer, net_a_payer, "le net a payer garde le defraiement et deduit l'acompte");
    assert_eq!(p.brut, dec("3120.00"), "le brut ne compte que les lignes soumises");
    let rnf = p.brut - p.total_sal + reintegre(p);
    assert_eq!(rubrique(&v, "S21.G00.50.002"), Some(rnf), "50.002 : brut - cotisations salariales + CSG/CRDS nd + sante patronale");
    assert_eq!(rubrique(&v, "S21.G00.50.004"), Some(p.brut - p.total_sal), "50.004 : la RNF moins ce qu'elle reintegre");
    assert_eq!(rubrique(&v, "S21.G00.50.013"), Some(rnf), "taux DGFiP : 50.013 = 50.002");
    assert_eq!(p.net_imposable, rnf);
    assert_eq!(p.net_verse(), p.brut - p.total_sal);

    // Le bulletin imprime le meme montant net social, puis le net a payer (libelles du modele de l'arrete du 25/02/2016
    // depuis la vague 1o ; « NET À PAYER AVANT IMPÔT » avant).
    let desc = bulletin(&ctx, p, "2026-10");
    assert_eq!(imprime_apres(&desc, "MONTANT NET SOCIAL"), fmt_eur(net_social));
    assert_eq!(imprime_apres(&desc, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"), fmt_eur(net_a_payer));
    assert_eq!(imprime_apres(&desc, "Montant net imposable"), fmt_eur(rnf));
    assert!(!imprime(&desc, "NET SOCIAL / NET A PAYER AVANT IMPÔT"), "les deux nets different : ils ne partagent plus une ligne");

    // Nicolet : l'indemnite imposable est un revenu comme l'autre ; sans frais ni mouvement, les deux nets sont egaux. Le
    // modele de l'arrete les imprime chacun sur sa ligne (vague 1o ; avant : une ligne « NET SOCIAL / NET A PAYER AVANT
    // IMPÔT »).
    let q = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("NI.24.01")).unwrap();
    let w = versement(&dsn, "NI.24.01");
    assert_eq!(nets_sociaux(&w), vec![("03".to_string(), q.brut - q.total_sal + dec("800.00"))]);
    // Une indemnite hors assiette mais IMPOSABLE entre dans la RNF (« AJOUTER les indemnites imposables », p. 61).
    assert_eq!(rubrique(&w, "S21.G00.50.002"), Some(q.brut - q.total_sal + dec("800.00") + reintegre(q)));
    assert_eq!(rubrique(&w, "S21.G00.50.004"), Some(q.brut - q.total_sal + dec("800.00")));
    assert_eq!(q.net_a_payer, q.brut - q.total_sal + dec("800.00"));
    let desc = bulletin(&ctx, q, "2026-10");
    assert_eq!(imprime_apres(&desc, "MONTANT NET SOCIAL"), fmt_eur(q.net_a_payer));
    assert_eq!(imprime_apres(&desc, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"), fmt_eur(q.net_a_payer));
    assert!(!imprime(&desc, "NET SOCIAL / NET A PAYER AVANT IMPÔT"));
}

#[test]
fn le_contrat_bi4_26_06_de_la_fictive_declare_les_nets_de_la_fiche_n2() {
    // Fiche N2 : brut 4 077,00, defraiements 434,70 ; la vieillesse plafonnee salariale sur le plafond des 23 jours
    // remuneres (BOSS § 1090), 4 005 x 23 / 31 = 2 971,45 : cotisations salariales 866,71. Montant net social et 50.004
    // 3 210,29 (les defraiements n'y sont pas), 50.002 3 327,77, net a payer 3 644,99.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, "2026-07").unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let v = versement(&dsn, "BI4.26.06");
    assert_eq!(nets_sociaux(&v), vec![("03".to_string(), dec("3210.29"))]);
    assert_eq!(rubrique(&v, "S21.G00.50.002"), Some(dec("3327.77")), "+ CSG/CRDS non deductible 117,48");
    assert_eq!(rubrique(&v, "S21.G00.50.004"), Some(dec("3210.29")));
    let p = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("BI4.26.06")).unwrap();
    assert_eq!(p.net_a_payer, dec("3644.99"));
    let desc = bulletin(&ctx, p, "2026-07");
    assert_eq!(imprime_apres(&desc, "MONTANT NET SOCIAL"), "3 210,29");
    assert_eq!(imprime_apres(&desc, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"), "3 644,99");
}

#[test]
fn chaque_rubrique_livree_hors_assiette_dit_ce_qu_elle_est_pour_le_net_social() {
    let cat = catalogue(&fictive("fictive-3090-grande")).unwrap();
    let mut vus: Vec<(String, HorsAssiette)> = Vec::new();
    for r in cat.iter().filter(|r| !r.soumis && r.origine == "base") {
        vus.push((r.code.clone(), r.hors_assiette().unwrap_or_else(|e| panic!("{} : {}", r.code, e))));
    }
    vus.sort_by(|a, b| a.0.cmp(&b.0));
    let attendu: Vec<(String, HorsAssiette)> = [
        ("acompte", HorsAssiette::Mouvement),
        ("defraiement_forfait", HorsAssiette::Frais),
        ("defraiement_reel", HorsAssiette::Frais),
        ("indemnite_licenciement", HorsAssiette::Revenu),
        ("indemnite_licenciement_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_mise_a_la_retraite", HorsAssiette::Revenu),
        ("indemnite_mise_a_la_retraite_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_imposable", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_supplement", HorsAssiette::Revenu),
        ("indemnite_rupture_conventionnelle_supplement_imposable", HorsAssiette::Revenu),
    ]
    .iter()
    .map(|(c, h)| (c.to_string(), *h))
    .collect();
    assert_eq!(vus, attendu);
}

#[test]
fn une_ligne_hors_assiette_dont_la_nature_ne_dit_pas_ce_qu_elle_est_se_refuse() {
    // Une compagnie ajoute une prime hors assiette : revenu, remboursement ou mouvement, sa nature (`prime`) ne le dit
    // pas d'une ligne hors assiette. Le montant net social ne se devine pas : la paie se refuse, et dit pourquoi.
    let racine = crate::outils::compagnie::config_et_mois("fictive-3090-grande", &[], &crate::outils::dossier_de_test("net_social", "refus"));
    std::fs::write(
        racine.join("config/rubriques-compagnie.json"),
        json!({"rubriques": {"prime_maison": {
            "libelle": "Prime maison", "unite": "forfait", "nature": "prime", "saisie": ["quantite", "taux"],
            "soumis": false, "imposable": false, "conges_spectacles": false, "source": "Accord d'entreprise du test."
        }}})
        .to_string(),
    )
    .unwrap();
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let chemin = ecrire_saisie(&json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
        {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
         "lignes": [
            {"rubrique": "mois", "quantite": 1, "taux": 3120.0, "heures": 151.67},
            {"rubrique": "prime_maison", "quantite": 1, "taux": 100.0}
         ]}
    ]}));
    let e = match crate::outils::charger_fichier(&ctx, &chemin) {
        Ok(_) => panic!("une ligne hors assiette de nature `prime` doit se refuser"),
        Err(e) => e,
    };
    assert!(e.contains("prime_maison") && e.contains("montant net social"), "{}", e);
    assert!(e.contains("frais") && e.contains("indemnite") && e.contains("retenue"), "{}", e);
}

/// Vague 1m : la page du BOSS « Montant net social » (version du 01/10/2026, II.A.1, Q 13, Q 14, Q 29) range la part
/// patronale de sante et de prevoyance DANS le montant net social ; le CSS R. 844-1, II, 1° (auquel renvoie le 9° bis
/// de R. 3243-1 du code du travail) l'en EXCLUT, dans les memes mots depuis le 31/12/2023 (decret n° 2023-1378), le
/// decret n° 2026-535 n'ayant touche que le 5° du I. Le moteur suit le decret : textes dans
/// atelier/reference/sources/montant-net-social/LISEZ-MOI.md. Un intermittent dont l'employeur finance la prevoyance et
/// le Fonds collectif du spectacle pour la sante (interbranche) : son montant net social est son brut moins ses
/// cotisations salariales, sans un centime de cette part patronale. Le permanent (sante et prevoyance patronales) est
/// le test `defraiement_acompte_et_indemnite_dans_les_nets_fiscal_et_social`.
#[test]
fn la_part_patronale_de_prevoyance_et_du_fonds_sante_d_un_intermittent_reste_hors_du_montant_net_social() {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, "2026-07").unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    let p = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("BI4.26.06")).unwrap();
    let patronale = somme(p, "prevoyance", true);
    assert!(patronale > Decimal::ZERO, "la prevoyance patronale (Fonds sante compris) existe : {:?}",
        p.lignes.iter().map(|l| (&l.id, l.montant_pat)).collect::<Vec<_>>());
    assert!(somme(p, "prevoyance", false) > Decimal::ZERO, "la part salariale de prevoyance existe, et se deduit");
    let attendu = p.brut - p.total_sal;
    assert_eq!(p.net_social, attendu, "brut - cotisations salariales, sans la part patronale ({})", patronale);
    assert_eq!(nets_sociaux(&versement(&dsn, "BI4.26.06")), vec![("03".to_string(), attendu)]);
}

#[test]
fn le_bulletin_imprime_les_libelles_et_l_ordre_du_modele_de_l_arrete() {
    // Vague 1o. Arrete du 25/02/2016, art. 1, I, redaction du 25/06/2024, seul modele depuis le 01/01/2026
    // (atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml) :
    // « MONTANT NET SOCIAL », « REMBOURSEMENTS ET DÉDUCTIONS DIVERSES », « MONTANT NET A PAYER AVANT IMPOT SUR LE
    // REVENU », « IMPOT SUR LE REVENU » (« Montant net imposable », « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE »),
    // « MONTANT NET A PAYER (en Euros) ». Le defraiement et l'acompte (hors assiette, pas un revenu ; BOSS, Q 39)
    // s'impriment sous les remboursements et deductions, plus au-dessus du brut. Avant : « NET SOCIAL / NET A PAYER
    // AVANT IMPÔT » quand les nets etaient egaux, « NET À PAYER AVANT IMPÔT », « Net imposable », « NET PAYÉ » seulement
    // avec un prelevement, et le defraiement au-dessus de « SALAIRE BRUT ».
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [
                {"rubrique": "mois", "quantite": 1, "taux": 3120.0, "heures": 151.67},
                {"rubrique": "defraiement_forfait", "quantite": 3, "taux": 20.7},
                {"rubrique": "acompte", "quantite": 1, "taux": 500.0}
             ]}
        ]}),
    );
    let p = &s.paies[0];
    let desc = bulletin(&ctx, p, "2026-10");
    let mut t = Vec::new();
    textes(&desc, &mut t);
    let ou = |libelle: &str| t.iter().position(|x| x == libelle).unwrap_or_else(|| panic!("le bulletin n'imprime pas {:?}", libelle));
    let ou_contient = |motif: &str| t.iter().position(|x| x.contains(motif)).unwrap_or_else(|| panic!("rien ne contient {:?}", motif));
    let modele = [
        ou("MONTANT NET SOCIAL"),
        ou("REMBOURSEMENTS ET DÉDUCTIONS DIVERSES"),
        ou_contient("à 20,70"),
        ou_contient("à 500,00"),
        ou("MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"),
        ou("IMPOT SUR LE REVENU"),
        ou("Montant net imposable"),
        ou_contient("IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE"),
        ou("MONTANT NET A PAYER (en Euros)"),
    ];
    assert!(modele.windows(2).all(|w| w[0] < w[1]), "l'ordre du modele : {:?}", modele);
    // Vague V2g : « MONTANT BRUT » (le modele de l'arrete) au lieu de « SALAIRE BRUT », toujours au-dessus.
    assert!(ou_contient("MONTANT BRUT") < modele[0]);
    assert_eq!(imprime_apres(&desc, "MONTANT NET SOCIAL"), fmt_eur(p.net_social));
    assert_eq!(imprime_apres(&desc, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"), fmt_eur(p.net_a_payer));
    assert_eq!(p.net_a_payer, p.net_social + dec("62.10") - dec("500.00"));
    for ancien in ["NET SOCIAL / NET A PAYER AVANT IMPÔT", "NET À PAYER AVANT IMPÔT", "Net imposable", "NET PAYÉ"] {
        assert!(!imprime(&desc, ancien), "{:?} n'est pas un libelle du modele", ancien);
    }
}

// ---------------------------------- vague 1o : les dates du montant net social d'une correction (BOSS Q 35 et Q 41)

/// Les blocs 58 d'un versement : (58.001, 58.002, 58.004).
fn blocs_58(v: &[(String, String)]) -> Vec<(String, String, Decimal)> {
    let (mut d, mut f, mut out) = (String::new(), String::new(), Vec::new());
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.58.001" => d = x.clone(),
            "S21.G00.58.002" => f = x.clone(),
            "S21.G00.58.004" => out.push((d.clone(), f.clone(), dec(x))),
            _ => {}
        }
    }
    out
}

/// Chaminade, technicien, les 20 et 21/10/2026 : 16 h a `taux`, deux defraiements de 20,70 et un acompte de 100 (le net
/// a payer n'est pas le montant net social), declare par un moteur d'avant la vague 1b : son 58.004 porte le net a
/// payer. Rend la paie d'octobre, et ce que la DSN deposee d'octobre a declare, relu comme une correction le relit.
fn octobre_d_avant_1b(ctx: &Contexte, taux: f64) -> (Paie, paie::versement_declare::VersementDeclare) {
    let s = saisie(
        ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-21", "numero": "CH1.26.10",
             "date_paiement": "2026-10-21",
             "lignes": [
                {"rubrique": "heure", "quantite": 16, "taux": taux, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-21"]},
                {"rubrique": "defraiement_forfait", "quantite": 2, "taux": 20.7},
                {"rubrique": "acompte", "quantite": 1, "taux": 100.0}
             ]}
        ]}),
    );
    let p = s.paies[0].clone();
    let dsn = generer(ctx, &s.paies, "2026-10", &[], &[], &Options::default()).unwrap();
    let ancien: String = dsn
        .lines()
        .map(|l| if l.starts_with("S21.G00.58.004,") { format!("S21.G00.58.004,'{}'", p.net_a_payer) } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    let nir = p.salarie["nir"].as_str().unwrap().to_string();
    let declare = paie::versement_declare::lire(&ancien, "la DSN d'octobre", "2026-10", &nir, "CH1.26.10").unwrap();
    assert_eq!(declare.net_social, p.net_a_payer);
    (p, declare)
}

#[test]
fn une_correction_du_seul_montant_net_social_declare_garde_la_periode_d_origine() {
    // BOSS, montant net social, Q 35 (atelier/reference/sources/montant-net-social/boss-montant-net-social-20261001.txt,
    // l. 482-486) : « Si seul le calcul du montant net social affiche et declare est errone, mais que le montant
    // effectivement verse au salarie etait correct », la correction se rattache « a la periode d'emploi concernee ».
    // Octobre est corrige en novembre, sans rien changer de ce qui a ete paye : l'annulation du 58.004 depose (le net a
    // payer) et le montant net social juste, aux dates d'octobre ; aucun 58 aux dates de novembre.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-11-28").unwrap())).unwrap();
    let (p, declare) = octobre_d_avant_1b(&ctx, 19.0);
    let r = paie::dsn::Regularisation {
        mois: "2026-10".into(), avant: p.clone(), declare, apres: p.clone(), taux_pas_origine: None, net_deja_verse: p.net_a_payer, decidee_le: None,
    };
    let dsn = generer(&ctx, &[], "2026-11", &[r], &[], &Options::default()).unwrap();
    let b = blocs_58(&versement(&dsn, "CH1.26.10"));
    assert_eq!(
        b,
        vec![
            ("20102026".to_string(), "21102026".to_string(), -p.net_a_payer),
            ("20102026".to_string(), "21102026".to_string(), p.net_social),
        ]
    );
}

#[test]
fn une_correction_qui_verse_un_rappel_le_date_du_mois_du_versement() {
    // Norme DSN 2026.1, S21.G00.58.003 (atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt, l. 14-18, p. 265) : les
    // dates du montant net social « sont a renseigner en periode de versement » ; BOSS Q 35 et Q 41 : le rappel se prend
    // dans le montant net social du mois ou il est verse. L'heure passe de 19 a 20 EUR, le rappel se verse en
    // novembre ; octobre etait declare avec le net a payer (avant 1b) : il se corrige au montant net social de ce qui a
    // ete paye (Q 41, « en precisant quel etait le montant net social effectivement verse »), aux dates d'octobre ; la
    // difference, aux dates de novembre. Avant (vagues 0 a 1n) : l'annulation et le montant net social corrige entier,
    // aux dates d'octobre.
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-11-28").unwrap())).unwrap();
    let (p, declare) = octobre_d_avant_1b(&ctx, 19.0);
    let (q, _) = octobre_d_avant_1b(&ctx, 20.0);
    assert!(q.net_social > p.net_social);
    let r = paie::dsn::Regularisation {
        mois: "2026-10".into(), avant: p.clone(), declare, apres: q.clone(), taux_pas_origine: None, net_deja_verse: p.net_a_payer, decidee_le: None,
    };
    let dsn = generer(&ctx, &[], "2026-11", &[r], &[], &Options::default()).unwrap();
    let b = blocs_58(&versement(&dsn, "CH1.26.10"));
    assert_eq!(
        b,
        vec![
            ("20102026".to_string(), "21102026".to_string(), -p.net_a_payer),
            ("20102026".to_string(), "21102026".to_string(), p.net_social),
            ("01112026".to_string(), "30112026".to_string(), q.net_social - p.net_social),
        ]
    );
}

#[test]
fn l_abattement_contrats_courts_ne_s_ecrit_plus_en_50_012() {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, "2026-07").unwrap();
    let p = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("BI4.26.06")).unwrap();
    // Sans taux DGFiP : la grille, et l'abattement d'un contrat court.
    let mut o = Options::default();
    for q in &s.paies {
        o.pas_par_salarie.insert(q.salarie["_cle"].as_str().unwrap().to_string(), None);
    }
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &o).unwrap();
    assert!(!dsn.contains("S21.G00.50.012"), "50.012 est reservee aux assistants maternels et familiaux (p. 245)");
    let v = versement(&dsn, "BI4.26.06");
    assert_eq!(rubrique(&v, "S21.G00.50.007"), Some(dec("13")));
    let fin = p.engagement.fin_periode().unwrap();
    let bareme = ctx.bareme(&fin).unwrap();
    let attendu = du_versement(None, &bareme, p.net_imposable, Some(fin), true, ZONE_DEFAUT).unwrap();
    assert!(attendu.abattement > Decimal::ZERO && attendu.montant_soumis < p.net_imposable, "un contrat court a un abattement");
    assert_eq!(rubrique(&v, "S21.G00.50.002"), Some(p.net_imposable));
    assert_eq!(rubrique(&v, "S21.G00.50.013"), Some(attendu.montant_soumis), "50.013 : la RNF moins l'abattement");
}
