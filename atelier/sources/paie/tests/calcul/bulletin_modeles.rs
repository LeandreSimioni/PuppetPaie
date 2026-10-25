//! Le bulletin suit le modele de l'arrete du 25/02/2016, art. 1, I (redaction du 25/06/2024, seul modele depuis le
//! 01/01/2026 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml) : ses
//! colonnes, ses rubriques dans l'ordre, les « Autres charges dues par l'employeur » agregees (C. trav. R. 3243-2, al. 2 ;
//! arrete, art. 3, LEGIARTI000047098507.xml), les exonerations non deduites de chaque cotisation (art. 4,
//! LEGIARTI000047098456.xml), la ligne du prelevement a la source ; ses sommes refont la paie ; un permanent et un
//! intermittent suivent le meme modele (regle 0).
//!
//! Deux modeles, *simplifie* et *complet* (PLAN-CONTRATS, C, points 9 et 11) : le complet imprime le meme bulletin, a
//! l'octet pres, suivi d'une page annexe qui detaille chaque cotisation (FAQ du BOSS sur le bulletin, Q 10 : pas plus de
//! detail dans la rubrique des obligatoires ; le detail « au verso du bulletin de paie ou bien sur un document annexe » ;
//! atelier/reference/sources/montant-net-social/boss-regles-generales-bulletin-de-paie-20250207-archive-web.txt,
//! l. 170-172) ; la compagnie choisit pour tous, la case du salarie l'emporte. La caisse des Conges Spectacles se range
//! dans « Autres charges dues par l'employeur » (Q 11 et 12, l. 173-178).
//!
//! Sur la copie figee des compagnies fictives (`tests/donnees/fictives/`), lue sans y rien ecrire.

use paie::bulletin::{fmt_eur, fmt_taux, jeux_et_description};
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use engagement::{charger, Saisie};
use socle::num::{dec_f64, Decimal};
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::outils::fictive;
use std::path::Path;

fn saisie(ctx: &Contexte, s: Value) -> Saisie {
    let chemin = crate::outils::chemin_unique("bulletin-modeles.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).unwrap()
}

/// Montansier (cadre permanent : mutuelle, prevoyance conventionnelle, Apec, CET) en octobre 2026.
fn permanent() -> (Contexte, Paie, &'static str) {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 4520.0, "heures": 151.67}]}
        ]}),
    );
    (ctx, s.paies[0].clone(), "2026-10")
}

/// Biancolelli (artiste intermittente, Conges Spectacles, prevoyance et Fonds sante, defraiements), juillet 2026.
fn intermittent() -> (Contexte, Paie, &'static str) {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, &fictive("fictive-3090-grande").join("paies/2026-07.json")).unwrap();
    let p = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("BI4.26.06")).unwrap().clone();
    (ctx, p, "2026-07")
}

/// Chaminade (technicien intermittent : la reduction generale s'applique), octobre 2026.
fn technicien() -> (Contexte, Paie, &'static str) {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-10-28").unwrap())).unwrap();
    let s = saisie(
        &ctx,
        json!({"mois": "2026-10", "date_paiement": "2026-10-31", "engagements": [
            {"salarie": "Chaminade", "spectacle": "266Z00910022", "debut": "2026-10-20", "fin": "2026-10-21", "numero": "CH1.26.10",
             "date_paiement": "2026-10-21",
             "lignes": [{"rubrique": "heure", "quantite": 16, "taux": 14.0, "lieu": "Theatre fictif", "dates": ["2026-10-20", "2026-10-21"]}]}
        ]}),
    );
    (ctx, s.paies[0].clone(), "2026-10")
}

/// La fiche entreprise du mois, la case `modele_bulletin` posee a `modele` (`None` : retiree).
fn entreprise(ctx: &Contexte, mois: &str, modele: Option<&str>) -> Value {
    let mut ent = ctx.entreprise_du_mois(mois).unwrap();
    let o = ent.as_object_mut().unwrap();
    o.remove("modele_bulletin");
    if let Some(m) = modele {
        o.insert("modele_bulletin".into(), json!(m));
    }
    ent
}

fn bulletin(ctx: &Contexte, p: &Paie, mois: &str, ent: &Value) -> Result<Value, String> {
    jeux_et_description(ctx, std::slice::from_ref(p), ent, mois, None, false, &HashMap::new())
}

fn texte(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("texte").and_then(Value::as_str).unwrap_or_default().to_string(),
        _ => String::new(),
    }
}

/// L'indice du tableau des cotisations dans les blocs, puis ses rangs (l'en-tete d'abord), chaque rang en ses cellules.
fn tableau(desc: &Value) -> (usize, Vec<Vec<String>>) {
    let blocs = desc["documents"][0]["blocs"].as_array().unwrap();
    let i = blocs
        .iter()
        .position(|b| b["type"] == json!("tableau") && b.to_string().contains("MONTANT NET SOCIAL"))
        .expect("le tableau des cotisations");
    let t = &blocs[i];
    let rangs = t["entete"].as_array().unwrap().iter().chain(t["lignes"].as_array().unwrap()).map(|r| r["cellules"].as_array().unwrap().iter().map(texte).collect()).collect();
    (i, rangs)
}

/// Le tableau des cotisations, l'en-tete d'abord.
fn rangs(desc: &Value) -> Vec<Vec<String>> {
    tableau(desc).1
}

/// Le bulletin du mois, sous le modele que la fiche entreprise du mois choisit.
fn du_mois(ctx: &Contexte, p: &Paie, mois: &str) -> Value {
    bulletin(ctx, p, mois, &ctx.entreprise_du_mois(mois).unwrap()).unwrap()
}

fn position(r: &[Vec<String>], libelle: &str) -> usize {
    r.iter().position(|x| x[0] == libelle).unwrap_or_else(|| panic!("le bulletin n'imprime pas {:?} : {:#?}", libelle, r.iter().map(|x| &x[0]).collect::<Vec<_>>()))
}

fn rang<'a>(r: &'a [Vec<String>], libelle: &str) -> &'a Vec<String> {
    &r[position(r, libelle)]
}

const AUTRES: &str = "Autres charges dues par l'employeur";
const TOTAL: &str = "TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES";
const EXONERATIONS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";
const RETRAIT: &str = "\u{a0}\u{a0}";

fn montant(t: &str) -> Decimal {
    t.replace([' ', '\u{a0}'], "").replace(',', ".").parse().unwrap()
}

fn ou_rien(x: Decimal) -> String {
    if x.is_zero() {
        String::new()
    } else {
        fmt_eur(x)
    }
}

/// Les rangs du tableau de l'annexe (le second document), l'en-tete d'abord.
fn annexe(desc: &Value) -> Vec<Vec<String>> {
    let blocs = desc["documents"][1]["blocs"].as_array().expect("l'annexe du complet");
    let t = blocs.iter().find(|b| b["type"] == json!("tableau")).expect("le tableau de l'annexe");
    t["entete"].as_array().unwrap().iter().chain(t["lignes"].as_array().unwrap()).map(|r| r["cellules"].as_array().unwrap().iter().map(texte).collect()).collect()
}

/// Le texte d'un PDF ecrit depuis la description, page par page.
fn pages_du_pdf(d: &Value, p: &Paie, nom: &str) -> Vec<String> {
    let f = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("bulletin-modeles-{}-{}.pdf", p.engagement.numero.clone().unwrap_or_default(), nom));
    paie::fabrique::ecrire(d, &f).unwrap();
    fabrique_pdf::lecture::texte(&f.to_string_lossy()).unwrap().0
}

/// La meme paie sous les deux modeles : le bulletin du complet est celui du simplifie, a l'octet pres (Q 10 : rien de
/// plus dans la rubrique des obligatoires) ; l'annexe qui le suit dit chaque ligne de la paie une fois, sous le titre de
/// la ligne du bulletin qui la porte, avec ses deux taux, et refait les totaux du bulletin ; elle ne porte aucune des
/// marques que lit le lecteur des archives.
fn memes_totaux(ctx: &Contexte, p: &Paie, mois: &str) {
    let simple = bulletin(ctx, p, mois, &entreprise(ctx, mois, Some("simplifié"))).unwrap();
    let complet = bulletin(ctx, p, mois, &entreprise(ctx, mois, Some("complet"))).unwrap();
    assert_eq!(simple["documents"].as_array().unwrap().len(), 1);
    assert_eq!(complet["documents"].as_array().unwrap().len(), 2, "le complet : le bulletin, puis son annexe");
    assert_eq!(complet["documents"][0], simple["documents"][0], "le bulletin du complet est le simplifie");
    let s = rangs(&simple);
    assert_eq!(s[0], ["LIBELLE", "Base", "Taux", "Salarié", "Employeur"]);
    assert!(s.iter().all(|r| !r[0].starts_with(RETRAIT)), "aucun detail dans le bulletin");
    // L'annexe : chaque ligne de la paie, une fois, sous le titre de la ligne du bulletin qui la porte.
    let a = annexe(&complet);
    assert_eq!(a[0], ["LIBELLÉ", "Base", "Taux salarié", "Salarié", "Taux employeur", "Employeur"]);
    let mut details: Vec<Vec<String>> = a.iter().filter(|r| r[0].starts_with(RETRAIT)).cloned().collect();
    let taux = |t: Option<f64>| t.map(|t| fmt_taux(dec_f64(t).unwrap())).unwrap_or_default();
    let mut attendus: Vec<Vec<String>> = p
        .lignes
        .iter()
        .map(|l| {
            let (base, ts, tp) = if l.masquer_base_et_taux { (String::new(), String::new(), String::new()) } else { (fmt_eur(l.base), taux(l.taux_sal), taux(l.taux_pat)) };
            vec![format!("{}{}", RETRAIT, l.libelle), base, ts, ou_rien(l.montant_sal), tp, ou_rien(l.montant_pat)]
        })
        .collect();
    details.sort();
    attendus.sort();
    assert_eq!(details, attendus, "chaque ligne de la paie, une fois, dans l'annexe");
    // Les autres charges : leur titre, puis chaque contribution ; leur somme est la ligne du bulletin.
    let titre = a.iter().position(|r| r[0] == AUTRES).expect("le titre des autres charges dans l'annexe");
    let regroupees: Vec<&Vec<String>> = a[titre + 1..].iter().take_while(|r| r[0].starts_with(RETRAIT)).collect();
    let somme: Decimal = regroupees.iter().map(|r| montant(&r[5])).sum();
    assert_eq!(fmt_eur(somme), rang(&s, AUTRES)[4], "le detail refait la ligne regroupee");
    // Les totaux de l'annexe sont ceux du bulletin.
    assert_eq!(rang(&a, TOTAL)[1..], [rang(&s, TOTAL)[1].clone(), String::new(), rang(&s, TOTAL)[2].clone()]);
    if s.iter().any(|r| r[0] == EXONERATIONS) {
        assert_eq!(rang(&a, EXONERATIONS)[1..], [rang(&s, EXONERATIONS)[1].clone(), String::new(), rang(&s, EXONERATIONS)[2].clone()]);
    }
    // Le taux employeur des accidents du travail, que le modele laisse a « - », se lit dans l'annexe.
    let atmp = p.lignes.iter().find(|l| l.id == "atmp").unwrap();
    assert_eq!(rang(&s, "Accidents du travail & maladies professionnelles")[2], "", "le taux du bulletin reste celui du salarie");
    assert!(a.iter().any(|r| r[0] == format!("{}{}", RETRAIT, atmp.libelle) && r[4] == taux(atmp.taux_pat)));
    // Aucune marque du lecteur des archives dans l'annexe : elle se range dans les pages du bulletin qu'elle suit.
    let texte_annexe = complet["documents"][1].to_string();
    for marque in ["Bulletin de paie n", "Contrat n°", "Période :", "NET A PAYER", "NET À PAYER"] {
        assert!(!texte_annexe.contains(marque), "l'annexe porte {:?}", marque);
    }
    // Le PDF : la premiere page du complet est celle du simplifie, l'annexe sur la suivante.
    let (ps, pc) = (pages_du_pdf(&simple, p, "simplifie"), pages_du_pdf(&complet, p, "complet"));
    assert_eq!(pc.len(), ps.len() + 1);
    assert_eq!(pc[..ps.len()], ps[..]);
    assert!(pc[ps.len()].contains("Détail des cotisations et contributions"));
}

#[test]
fn le_complet_imprime_la_meme_paie_que_le_simplifie_d_un_permanent() {
    let (ctx, p, mois) = permanent();
    memes_totaux(&ctx, &p, mois);
}

#[test]
fn le_complet_imprime_la_meme_paie_que_le_simplifie_d_un_intermittent() {
    let (ctx, p, mois) = intermittent();
    memes_totaux(&ctx, &p, mois);
}

#[test]
fn le_complet_imprime_la_meme_paie_que_le_simplifie_d_un_technicien_avec_reduction_generale() {
    let (ctx, p, mois) = technicien();
    assert!(p.lignes.iter().any(|l| l.id == "reduction_generale"), "le cas porte une exoneration");
    memes_totaux(&ctx, &p, mois);
}

#[test]
fn sans_choix_le_bulletin_est_le_simplifie_d_avant() {
    // La compagnie qui n'a rien choisi garde le bulletin de la vague V2g, a l'octet pres.
    let (ctx, p, mois) = intermittent();
    let rien = bulletin(&ctx, &p, mois, &entreprise(&ctx, mois, None)).unwrap();
    assert_eq!(rien, bulletin(&ctx, &p, mois, &entreprise(&ctx, mois, Some(""))).unwrap());
    assert_eq!(rien, bulletin(&ctx, &p, mois, &entreprise(&ctx, mois, Some("simplifié"))).unwrap());
    assert_eq!(rien["documents"].as_array().unwrap().len(), 1, "pas d'annexe");
}

#[test]
fn la_case_du_salarie_l_emporte_sur_le_choix_de_la_compagnie() {
    let (ctx, p, mois) = technicien();
    let documents = |ent: &Value, sal: Option<&str>| {
        let mut q = p.clone();
        if let Some(m) = sal {
            q.salarie["modele_bulletin"] = json!(m);
        }
        bulletin(&ctx, &q, mois, ent).unwrap()["documents"].as_array().unwrap().len()
    };
    let (simple, complet) = (entreprise(&ctx, mois, Some("simplifié")), entreprise(&ctx, mois, Some("complet")));
    assert_eq!(documents(&complet, None), 2, "la compagnie choisit pour tous");
    assert_eq!(documents(&simple, Some("complet")), 2, "le salarie qui demande le complet l'obtient");
    assert_eq!(documents(&complet, Some("simplifié")), 1, "le salarie qui demande le simplifie l'obtient");
    assert_eq!(documents(&complet, Some("")), 2, "une case vide laisse le choix de la compagnie");
    assert_eq!(documents(&entreprise(&ctx, mois, None), None), 1, "sans choix : le simplifie");
}

#[test]
fn un_modele_inconnu_refuse() {
    // Regle 1 : un modele que le logiciel ne connait pas ne se devine pas.
    let (ctx, p, mois) = technicien();
    let e = bulletin(&ctx, &p, mois, &entreprise(&ctx, mois, Some("détaillé"))).unwrap_err();
    assert!(e.contains("modèle de bulletin « détaillé »"), "{}", e);
    let mut q = p.clone();
    q.salarie["modele_bulletin"] = json!(true);
    let e = bulletin(&ctx, &q, mois, &entreprise(&ctx, mois, None)).unwrap_err();
    assert!(e.contains("modèle de bulletin illisible"), "{}", e);
}

const OBLIGATOIRES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES";
const FACULTATIVES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES";
const VERSE: &str = "TOTAL VERSÉ PAR L'EMPLOYEUR";

/// Les libelles d'avant la vague V2g, qu'aucun bulletin neuf n'imprime plus.
const ANCIENS: [&str; 10] = [
    "SALAIRE BRUT",
    "Total des cotisations et contributions",
    "Part Employeur",
    "Part Salarié",
    "SANTÉ",
    "RETRAITE",
    "CSG / CRDS",
    "AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR",
    "Autres contributions dues par l'employeur",
    "Coût total employeur",
];

fn sans_les_anciens(desc: &Value) {
    let tout = desc.to_string();
    for a in ANCIENS {
        assert!(!tout.contains(a), "{:?} n'est pas un libelle du modele", a);
    }
}

fn somme(p: &Paie, ids: &[&str], patronale: bool) -> Decimal {
    p.lignes.iter().filter(|l| ids.contains(&l.id.as_str())).fold(Decimal::new(0, 2), |a, l| a + if patronale { l.montant_pat } else { l.montant_sal })
}

#[test]
fn le_bulletin_d_un_permanent_suit_le_modele_de_l_arrete() {
    let (ctx, p, mois) = permanent();
    let desc = du_mois(&ctx, &p, mois);
    let r = rangs(&desc);
    // Les colonnes du modele.
    assert_eq!(r[0], vec!["LIBELLE", "Base", "Taux", "Salarié", "Employeur"]);
    let ordre = [
        position(&r, "1 Salaire mensuel de base à 4 520,00 — MONTANT BRUT"),
        position(&r, OBLIGATOIRES),
        position(&r, "Santé"),
        position(&r, "Sécurité Sociale Maladie Maternité Invalidité Décès"),
        position(&r, "Complémentaire garanties frais de santé obligatoire"),
        position(&r, "Accidents du travail & maladies professionnelles"),
        position(&r, "Retraite"),
        position(&r, "Sécurité Sociale Vieillesse plafonnée"),
        position(&r, "Sécurité Sociale Vieillesse déplafonnée"),
        position(&r, "Retraite complémentaire, CEG et CET T1"),
        position(&r, "Famille"),
        position(&r, "Assurance chômage"),
        position(&r, "Apec"),
        position(&r, "Autres charges dues par l'employeur"),
        position(&r, "CSG déductible de l'impôt sur le revenu"),
        position(&r, "CSG/CRDS non déductible de l'impôt sur le revenu"),
        position(&r, TOTAL),
        position(&r, FACULTATIVES),
        position(&r, "MONTANT NET SOCIAL"),
        position(&r, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"),
        position(&r, "IMPOT SUR LE REVENU"),
        position(&r, "Montant net imposable"),
        position(&r, "IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE"),
        position(&r, "MONTANT NET A PAYER (en Euros)"),
        position(&r, VERSE),
    ];
    assert!(ordre.windows(2).all(|w| w[0] < w[1]), "l'ordre du modele : {:?}", ordre);
    sans_les_anciens(&desc);
    // Le rang d'une cotisation : base, taux SALARIAL, montant salarial, montant employeur.
    let t1 = p.lignes.iter().find(|l| l.id == "retraite_t1").unwrap();
    assert_eq!(
        rang(&r, "Retraite complémentaire, CEG et CET T1")[1..],
        [fmt_eur(t1.base), fmt_taux(dec_f64(t1.taux_sal.unwrap()).unwrap()), fmt_eur(t1.montant_sal), fmt_eur(t1.montant_pat)]
    );
    // La CET d'un permanent (sur T1 + T2, non partagee entre les tranches) garde sa ligne, sous « Retraite ».
    let cet = position(&r, "Contribution d'équilibre technique (CET)");
    assert!(position(&r, "Retraite") < cet && cet < position(&r, "Famille"));
}

#[test]
fn le_bulletin_d_un_intermittent_suit_le_meme_modele() {
    // Regle 0 : les memes rubriques du modele, dans le meme ordre ; la caisse des Conges Spectacles est dans les autres
    // charges (voir `la_caisse_des_conges_spectacles_est_une_autre_charge_due_par_l_employeur`).
    let (ctx, p, mois) = intermittent();
    let desc = du_mois(&ctx, &p, mois);
    let r = rangs(&desc);
    assert_eq!(r[0], vec!["LIBELLE", "Base", "Taux", "Salarié", "Employeur"]);
    let ordre = [
        position(&r, "MONTANT BRUT"),
        position(&r, OBLIGATOIRES),
        position(&r, "Santé"),
        position(&r, "Sécurité Sociale Maladie Maternité Invalidité Décès"),
        position(&r, "Accidents du travail & maladies professionnelles"),
        position(&r, "Retraite"),
        position(&r, "Sécurité Sociale Vieillesse plafonnée"),
        position(&r, "Retraite complémentaire, CEG et CET T1"),
        position(&r, "Famille"),
        position(&r, "Assurance chômage"),
        position(&r, "Autres charges dues par l'employeur"),
        position(&r, "Cotisations statutaires ou prévues par la convention collective"),
        position(&r, "CSG déductible de l'impôt sur le revenu"),
        position(&r, TOTAL),
        position(&r, FACULTATIVES),
        position(&r, "Prévoyance, Incapacité, Invalidité, Décès, Autres"),
        position(&r, "MONTANT NET SOCIAL"),
        position(&r, "REMBOURSEMENTS ET DÉDUCTIONS DIVERSES"),
        position(&r, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"),
        position(&r, "MONTANT NET A PAYER (en Euros)"),
        position(&r, VERSE),
    ];
    assert!(ordre.windows(2).all(|w| w[0] < w[1]), "l'ordre du modele : {:?}", ordre);
    sans_les_anciens(&desc);
}

#[test]
fn aucun_montant_ne_bouge_les_sommes_du_bulletin_refont_la_paie() {
    for (nom, (ctx, p, mois)) in [("permanent", permanent()), ("intermittent", intermittent()), ("technicien", technicien())] {
        let r = rangs(&du_mois(&ctx, &p, mois));
        let fac = ["prevoyance", "prevoyance_t2", "prevoyance_tb"];
        let (fac_sal, fac_pat) = (somme(&p, &fac, false), somme(&p, &fac, true));
        let rg = somme(&p, &["reduction_generale"], true);
        // TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES : avant les exonerations, sans les facultatives.
        assert_eq!(rang(&r, TOTAL)[1..], [fmt_eur(p.total_sal - fac_sal), fmt_eur(p.total_pat - fac_pat - rg)], "{}", nom);
        if rg.is_zero() {
            assert!(!r.iter().any(|x| x[0] == EXONERATIONS), "{} : pas d'exoneration, pas de ligne", nom);
        } else {
            assert_eq!(rang(&r, EXONERATIONS)[1..], [String::new(), fmt_eur(rg)], "{}", nom);
        }
        // Les autres charges : UNE ligne, le seul montant employeur.
        let autres = somme(&p, &["autres_1", "autres_2", "autres_3", "autres_4", "versement_mobilite", "conges_spec"], true);
        assert_eq!(rang(&r, "Autres charges dues par l'employeur")[1..], ["", "", "", &fmt_eur(autres)], "{}", nom);
        assert_eq!(r.iter().filter(|x| x[0] == "Autres charges dues par l'employeur").count(), 1, "{}", nom);
        // TOTAL VERSÉ PAR L'EMPLOYEUR : le cout employeur de la paie (C. trav. R. 3243-1, 14°).
        assert_eq!(rang(&r, VERSE)[1..], [fmt_eur(p.cout_employeur)], "{}", nom);
        assert_eq!(p.cout_employeur, p.brut + p.total_pat, "{}", nom);
        // Les nets ne bougent pas.
        assert_eq!(rang(&r, "MONTANT NET SOCIAL")[1..], [fmt_eur(p.net_social)], "{}", nom);
        assert_eq!(rang(&r, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU")[1..], [fmt_eur(p.net_a_payer)], "{}", nom);
        assert_eq!(rang(&r, "Montant net imposable")[1..], [fmt_eur(p.net_imposable)], "{}", nom);
        // Chaque montant salarial et employeur d'une ligne hors des autres charges s'imprime une fois.
        for l in p.lignes.iter().filter(|l| !l.id.starts_with("autres_") && !["versement_mobilite", "conges_spec", "reduction_generale"].contains(&l.id.as_str())) {
            let trouve = r.iter().any(|x| {
                x.len() == 5
                    && x[3] == if l.montant_sal.is_zero() { String::new() } else { fmt_eur(l.montant_sal) }
                    && x[4] == if l.montant_pat.is_zero() { String::new() } else { fmt_eur(l.montant_pat) }
            });
            assert!(trouve, "{} : la ligne {} ({} / {}) s'imprime", nom, l.id, l.montant_sal, l.montant_pat);
        }
    }
}

#[test]
fn la_caisse_des_conges_spectacles_est_une_autre_charge_due_par_l_employeur() {
    // FAQ du BOSS sur le bulletin, Q 11 (l. 173-174) : pas de ligne en plus pour une contribution propre a un secteur,
    // elle se regroupe dans les rubriques « autres » ; Q 12 (l. 175-178) : les contributions legales a la charge
    // exclusive de l'employeur se totalisent dans « Autres charges dues par l'employeur ». La caisse : C. trav.
    // L. 3141-32, D. 7121-40 (l'employeur du spectacle s'y affilie), taux patronal seul. Jusqu'au lot D2 : une ligne a
    // elle, « Caisse des Congés Spectacles », entre les autres charges et les cotisations conventionnelles.
    let (ctx, p, mois) = intermittent();
    let cs = p.lignes.iter().find(|l| l.id == "conges_spec").expect("le cas porte la caisse");
    assert!(cs.taux_sal.is_none() && !cs.montant_pat.is_zero());
    let r = rangs(&du_mois(&ctx, &p, mois));
    assert!(!r.iter().any(|x| x[0] == cs.libelle), "la caisse n'a plus de ligne a elle : {:?}", r.iter().map(|x| &x[0]).collect::<Vec<_>>());
    let autres = somme(&p, &["autres_1", "autres_2", "autres_3", "autres_4", "versement_mobilite"], true);
    assert_eq!(rang(&r, AUTRES)[4], fmt_eur(autres + cs.montant_pat), "les autres charges comptent la caisse");
}

#[test]
fn la_ligne_du_prelevement_dit_son_assiette_son_taux_et_son_montant() {
    // Arrete, art. 1, I : « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE » sous les colonnes Base, Taux, Salarié ; C. trav.
    // R. 3243-1, 9° : l'assiette, le taux et le montant. Avant : le taux dans le libelle, ni assiette ni colonne.
    let (ctx, p, mois) = intermittent();
    let r = rangs(&du_mois(&ctx, &p, mois));
    assert_eq!(rang(&r, "IMPOT SUR LE REVENU")[1..], ["Base", "Taux", "Salarié"]);
    let pas = &rang(&r, "IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE")[1..];
    assert_eq!(pas.len(), 3);
    let (assiette, taux, montant) = (&pas[0], &pas[1], &pas[2]);
    let lu = |t: &str| t.replace([' ', '\u{a0}'], "").replace(',', ".").parse::<f64>().unwrap();
    // Le mois est depose : l'assiette, le taux et le montant sont ceux que sa DSN a declares (lot D2, `prelevement::des_paies`),
    // pas recalcules ; l'assiette n'est donc pas comparee au net imposable que le moteur du jour recalcule.
    assert!(lu(assiette) > 0.0, "l'assiette : {}", assiette);
    assert!(((lu(assiette) * lu(taux) / 100.0) - lu(montant)).abs() < 0.011, "{} x {} % = {}", assiette, taux, montant);
}
