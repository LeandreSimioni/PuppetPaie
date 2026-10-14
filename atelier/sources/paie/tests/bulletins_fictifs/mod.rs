//! Les PDF FICTIFS de bulletins, communs aux tests (`bulletin_lu`, `routes_importation`) : fabriques par la fabrique
//! (`texte_fixe`, positions du haut-gauche de la page) aux positions des deux gabarits de la table livree, CoolPaie
//! (A) et sPAIEctacle (B). Noms, adresses, NIR, Siret, numeros d'objet, titres et montants sont inventes ; rien de
//! `prive/`, aucun des PDF reels de `import/`.

#![allow(dead_code)]

use serde_json::{json, Value};
use std::path::Path;

// ------------------------------------------------------------------------------------------- la fabrique

pub fn t(b: &mut Vec<Value>, x: f64, y: f64, texte: &str) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": 9.0}));
}

/// Un nombre, aligne a droite sur `x`.
pub fn r(b: &mut Vec<Value>, x: f64, y: f64, texte: &str) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": 9.0, "alignement": "droite"}));
}

pub fn grand(b: &mut Vec<Value>, x: f64, y: f64, texte: &str, taille: f64) {
    b.push(json!({"type": "texte_fixe", "texte": texte, "x": x, "y": y, "taille": taille}));
}

/// Un PDF d'une page par liste de blocs, A4.
pub fn pdf(pages: Vec<Vec<Value>>, chemin: &Path) {
    let d = json!({
        "format": 3,
        "page": {"largeur": 595.22, "hauteur": 842.0, "marges": {"haut": 0.0, "droite": 0.0, "bas": 0.0, "gauche": 0.0}},
        "police": {"taille": 9.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": pages.into_iter().map(|b| json!({"blocs": b})).collect::<Vec<_>>()
    });
    let octets = fabrique_pdf::fabriquer(&d.to_string()).unwrap_or_else(|r| panic!("la fabrique refuse : {}", r.0));
    std::fs::write(chemin, octets).unwrap();
}

// ------------------------------------------------------------------------------------------- gabarit A

/// Une ligne du tableau des cotisations : un titre de famille (`famille` sans nombres), ou une cotisation.
/// (x du libelle, libelle, famille attendue, base, taux employeur, montant employeur, taux salarie, montant salarie)
pub type RangA = (f64, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str);

/// Les cotisations fictives de CoolPaie (montants inventes) ; `B` : la base (le brut), `C` : la base de la CSG.
pub const COTISATIONS_A: [RangA; 20] = [
    (33.9, "SANTE", "", "", "", "", "", ""),
    (33.9, "Sécurité sociale - Maladie Maternité Invalidité Décès", "sante", "B", "7,000", "24,50", "", ""),
    (33.9, "Complémentaire Incapacité Invalidité Décès (Audiens)", "sante", "B", "1,040", "5,20", "0,120", "0,60"),
    (33.9, "ACCIDENTS DU TRAVAIL - MALADIES", "", "", "", "", "", ""),
    (40.0, "PROFESSIONNELLES", "accidents_du_travail", "B", "1,050", "5,25", "", ""),
    (33.9, "RETRAITE", "", "", "", "", "", ""),
    (33.9, "Sécurité sociale plafonnée", "retraite", "B", "8,550", "29,95", "6,900", "24,15"),
    (33.9, "Sécurité sociale déplafonnée", "retraite", "B", "2,020", "7,40", "0,400", "1,40"),
    (33.9, "Complémentaire Tranche 1", "retraite", "B", "5,290", "28,70", "4,010", "26,50"),
    (33.9, "FAMILLE", "famille", "B", "3,450", "12,10", "", ""),
    (33.9, "ASSURANCE CHOMAGE", "", "", "", "", "", ""),
    (33.9, "Chômage", "assurance_chomage", "B", "4,550", "48,75", "2,400", "12,00"),
    (33.9, "AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR", "", "", "", "", "", ""),
    (33.9, "autres contributions dues par l'employeur", "autres_contributions", "B", "2,836", "14,18", "", ""),
    (33.9, "CONGES SPECTACLES", "conges_spectacles", "B", "15,500", "77,50", "", ""),
    (33.9, "COTISATIONS STATUTAIRES OU PREVUES PAR LA", "", "", "", "", "", ""),
    (40.0, "CONVENTION COLLECTIVE", "cotisations_conventionnelles", "", "", "2,00", "", ""),
    (33.9, "CSG déductible à l'impôt sur le revenu", "csg", "C", "", "", "6,800", "33,32"),
    (33.9, "CSG/CRDS non déductible à l'impôt sur le revenu", "csg", "C", "", "", "2,900", "14,21"),
    (33.9, "EXONERATIONS ET ALLEGEMENTS DE COTISATIONS", "exonerations", "", "100,000", "-27,30", "", "0,00"),
];

/// Le libelle tel que le lecteur le rend (un titre sur deux rangs se recolle).
pub fn libelle_lu(k: usize) -> String {
    match COTISATIONS_A[k].1 {
        "PROFESSIONNELLES" => "ACCIDENTS DU TRAVAIL - MALADIES PROFESSIONNELLES".into(),
        "CONVENTION COLLECTIVE" => "COTISATIONS STATUTAIRES OU PREVUES PAR LA CONVENTION COLLECTIVE".into(),
        l => l.into(),
    }
}

pub struct A {
    pub numero: &'static str,
    /// Numero d'objet et titre du spectacle.
    pub objet: Option<(&'static str, &'static str)>,
    pub emploi: &'static str,
    pub statut: &'static str,
    pub periode: &'static str,
    /// Les rangs des dates travaillees.
    pub dates: Vec<&'static str>,
    /// (quantite, libelle, taux, montant)
    pub lignes: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    pub brut: &'static str,
    pub csg: &'static str,
    pub net: &'static str,
    /// (base, taux, montant, cumul)
    pub pas: [&'static str; 4],
    /// (periode, cumul)
    pub net_imposable: [&'static str; 2],
    pub net_a_payer: &'static str,
    pub regle: &'static str,
    /// « mode de reglement » (absent avant 2023).
    pub mode: bool,
    /// « Cachets: » ou « Heures: »
    pub unite: &'static str,
    /// (unite, brut, plafond, cout employeur) x (periode, cumul)
    pub cumuls: [[&'static str; 2]; 4],
    pub total_verse: &'static str,
}

pub fn page_a(a: &A) -> Vec<Value> {
    let mut b = Vec::new();
    let b_ = &mut b;
    grand(b_, 320.0, 45.0, &format!("Bulletin de paie n° {}", a.numero), 11.0);
    if let Some((objet, titre)) = a.objet {
        t(b_, 354.3, 65.0, &format!("numéro d'objet : {}", objet));
        t(b_, 354.3, 76.0, &format!("Spectacle : {}", titre));
    }
    grand(b_, 28.4, 120.0, "Compagnie des Essais", 11.0);
    t(b_, 283.5, 120.0, "BRIOCHE FANCHON");
    t(b_, 28.4, 132.0, "12 rue Imaginaire");
    t(b_, 283.5, 132.0, "3 IMPASSE DES ESSAIS");
    t(b_, 28.4, 143.0, "99998 Nullepart");
    t(b_, 283.5, 143.0, "99999 VILLE FICTIVE");
    for (y, gauche, valeur_g, droite, valeur_d) in [
        (172.0, "N° Siret :", "99999999900017", "Date de naissance :", "04/11/1988"),
        (183.0, "NAF(APE) :", "9001Z", "N° Sécurité Sociale :", "288117599900146"),
        (194.0, "Urssaf :", "999999999999", "Code Salarié :", "7"),
        (205.0, "Conv. collective :", "IDCC n° 3090 - Spectacle vivant", "Emploi :", a.emploi),
        (227.0, "Licence spec. N° :", "L-R-99-9", "Abattement :", "0 %"),
    ] {
        t(b_, 28.4, y, gauche);
        t(b_, 127.5, y, valeur_g);
        t(b_, 283.5, y, droite);
        t(b_, 436.4, y, valeur_d);
    }
    t(b_, 127.5, 216.0, "du secteur privé");
    t(b_, 283.5, 217.5, "Statut :");
    t(b_, 436.4, 217.5, a.statut);
    t(b_, 283.5, 238.0, "N° de Congés Spectacles :");
    t(b_, 436.4, 238.0, "Z0000001");
    t(b_, 79.5, 262.0, "Périodes :");
    t(b_, 135.9, 262.0, a.periode);
    t(b_, 42.8, 273.0, "Dates travaillées :");
    for (k, d) in a.dates.iter().enumerate() {
        t(b_, 135.9, 273.0 + 11.0 * k as f64, d);
    }
    let mut y = 308.0;
    t(b_, 104.9, y, "LIBELLE");
    t(b_, 324.3, y, "Base");
    t(b_, 382.6, y, "Part Employeur");
    t(b_, 504.6, y, "Part Salarié");
    for (q, libelle, taux, montant) in &a.lignes {
        y += 11.2;
        t(b_, 56.7, y, &format!("{} {}", q, libelle));
        r(b_, 346.0, y, taux);
        r(b_, 453.7, y, montant);
    }
    y += 11.2;
    t(b_, 56.7, y, "SALAIRE BRUT");
    r(b_, 453.7, y, a.brut);
    y += 14.2;
    for (x, texte) in [(394.1, "Taux"), (453.9, "Montant"), (504.8, "Taux"), (561.6, "Montant")] {
        r(b_, x, y, texte);
    }
    for (x, libelle, _, base, te, me, ts, ms) in COTISATIONS_A.iter() {
        y += 11.2;
        t(b_, *x, y, libelle);
        let base = match *base {
            "B" => a.brut,
            "C" => a.csg,
            autre => autre,
        };
        for (xd, v) in [(345.9, base), (394.2, *te), (453.7, *me), (504.9, *ts), (561.5, *ms)] {
            if !v.is_empty() {
                r(b_, xd, y, v);
            }
        }
    }
    y += 22.0;
    t(b_, 36.8, y, "Total des Cotisations et Contributions");
    r(b_, 434.0, y, "255,53");
    r(b_, 561.4, y, "112,18");
    y += 20.3;
    t(b_, 33.9, y, "NET SOCIAL");
    r(b_, 561.6, y, a.net);
    y += 11.0;
    t(b_, 33.9, y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, 561.5, y, a.net);
    // Le pied.
    y += 11.5;
    t(b_, 33.9, y, "Impôt sur le revenu");
    t(b_, 310.9, y, "base");
    t(b_, 396.3, y, "taux personnalisé");
    t(b_, 486.8, y, "montant");
    t(b_, 543.0, y, "cumul");
    y += 14.0;
    t(b_, 33.9, y, "Impôt sur le revenu prélevé à la source");
    r(b_, 331.7, y, a.pas[0]);
    r(b_, 466.8, y, a.pas[1]);
    t(b_, 471.6, y, "%");
    r(b_, 516.0, y, a.pas[2]);
    r(b_, 567.2, y, a.pas[3]);
    y += 8.5;
    t(b_, 33.9, y, "Net Imposable");
    r(b_, 518.9, y, a.net_imposable[0]);
    r(b_, 567.2, y, a.net_imposable[1]);
    y += 8.5;
    t(b_, 33.9, y, "Montant net des heures compl / suppl exonérées");
    r(b_, 518.9, y, "0,00");
    r(b_, 567.2, y, "0,00");
    y += 13.0;
    t(b_, 131.8, y, "Période");
    t(b_, 203.6, y, "Cumul");
    t(b_, 435.7, y, "NET A PAYER AU SALARIE");
    t(b_, 255.2, y + 2.0, &format!("Réglement le : {}", a.regle));
    t(b_, 460.3, y + 10.0, "(EN EUROS)");
    for (k, libelle) in [a.unite, "Salaire brut:", "Plafond Urssaf:", "Cout employeur:"].iter().enumerate() {
        let yk = y + 14.0 + 11.2 * k as f64;
        t(b_, 36.8, yk, libelle);
        r(b_, 170.2, yk, a.cumuls[k][0]);
        r(b_, 235.5, yk, a.cumuls[k][1]);
    }
    if a.mode {
        t(b_, 255.2, y + 14.0, "mode de règlement : Virement");
    }
    grand(b_, 469.5, y + 32.0, &format!("{} €", a.net_a_payer), 14.0);
    t(b_, 439.7, y + 51.0, "Total versé par l'employeur");
    grand(b_, 478.9, y + 72.0, &format!("{} €", a.total_verse), 11.0);
    grand(b_, 31.0, 810.0, "coolpaie 4.0.0.8.312", 7.0);
    b
}

/// A : cachets isoles + services de repetition, 2026.
pub fn a_repetitions() -> A {
    A {
        numero: "7/2026",
        objet: Some(("999Z99999901", "Les Fictifs en Tournée")),
        emploi: "ARTISTE DRAMATIQUE",
        statut: "artiste",
        periode: "du 14/01/2026 au 24/01/2026",
        dates: vec!["du 14/01/2026 au 14/01/2026, du 18/01/2026 au 18/01/2026,", "du 21/01/2026 au 21/01/2026, du 23/01/2026 au 24/01/2026"],
        lignes: vec![("5", "Service(s) répét. 4h (contrat<5j)", "62,00", "310,00"), ("2", "Cachets isolés", "140,00", "280,00")],
        brut: "590,00",
        csg: "579,68",
        net: "477,82",
        pas: ["491,90", "0.000", "0.00", "0,00"],
        net_imposable: ["491,90", "491,90"],
        net_a_payer: "477,82",
        regle: "24/01/2026",
        mode: true,
        unite: "Cachets:",
        cumuls: [["2,00", "2,00"], ["590,00", "590,00"], ["1 512,40", "1 512,40"], ["845,53", "845,53"]],
        total_verse: "845,53",
    }
}

/// A : cachets groupes, 2021, un impot preleve, pas de mode de reglement.
pub fn a_groupes() -> A {
    A {
        numero: "4/2021",
        objet: Some(("999Z99999902", "Le Grand Essai")),
        emploi: "MUSICIEN",
        statut: "artiste",
        periode: "du 02/12/2021 au 20/12/2021",
        dates: vec!["du 02/12/2021 au 05/12/2021, du 18/12/2021 au 20/12/2021"],
        lignes: vec![("15", "Cachets groupés", "160,00", "2 400,00")],
        brut: "2 400,00",
        csg: "2 358,00",
        net: "2 287,82",
        pas: ["2 360,10", "1.500", "35.40", "35,40"],
        net_imposable: ["2 360,10", "3 120,10"],
        net_a_payer: "2 252,42",
        regle: "24/12/2021",
        mode: false,
        unite: "Cachets:",
        cumuls: [["15,00", "22,00"], ["2 400,00", "3 600,00"], ["2 900,00", "7 000,00"], ["2 655,53", "4 100,00"]],
        total_verse: "2 655,53",
    }
}

/// A : cachets et masterclass.
pub fn a_masterclass() -> A {
    A {
        numero: "12/2026",
        objet: Some(("999Z99999903", "Atelier Imaginaire")),
        emploi: "ARTISTE DRAMATIQUE",
        statut: "artiste",
        periode: "du 03/03/2026 au 05/03/2026",
        dates: vec!["du 03/03/2026 au 05/03/2026"],
        lignes: vec![("3", "Cachets isolés", "135,00", "405,00"), ("3", "Masterclass / Action Culturelle", "85,00", "255,00")],
        brut: "660,00",
        csg: "648,45",
        net: "547,82",
        pas: ["560,00", "0.000", "0.00", "0,00"],
        net_imposable: ["560,00", "1 051,90"],
        net_a_payer: "547,82",
        regle: "05/03/2026",
        mode: true,
        unite: "Cachets:",
        cumuls: [["3,00", "5,00"], ["660,00", "1 250,00"], ["1 600,00", "3 112,40"], ["915,53", "1 761,06"]],
        total_verse: "915,53",
    }
}

/// A : les heures d'un « non cadre », sans spectacle.
pub fn a_heures() -> A {
    A {
        numero: "15/2025",
        objet: None,
        emploi: "CHARGE DE PRODUCTION",
        statut: "non cadre",
        periode: "du 12/05/2025 au 16/05/2025",
        dates: vec!["du 12/05/2025 au 16/05/2025"],
        lignes: vec![("40", "Heures", "16,00", "640,00")],
        brut: "640,00",
        csg: "628,80",
        net: "527,82",
        pas: ["540,00", "0.000", "0.00", "0,00"],
        net_imposable: ["540,00", "1 540,00"],
        net_a_payer: "527,82",
        regle: "16/05/2025",
        mode: true,
        unite: "Heures:",
        cumuls: [["40,00", "112,00"], ["640,00", "1 906,00"], ["1 200,00", "3 575,52"], ["895,53", "2 700,00"]],
        total_verse: "895,53",
    }
}

/// A : 2021, sans numero d'objet ni spectacle.
pub fn a_sans_objet() -> A {
    A {
        numero: "1/2021",
        objet: None,
        emploi: "ARTISTE DRAMATIQUE",
        statut: "artiste",
        periode: "du 09/01/2021 au 10/01/2021",
        dates: vec!["du 09/01/2021 au 10/01/2021"],
        lignes: vec![("2", "Cachets isolés", "150,00", "300,00")],
        brut: "300,00",
        csg: "294,75",
        net: "187,82",
        pas: ["200,00", "0.000", "0.00", "0,00"],
        net_imposable: ["200,00", "200,00"],
        net_a_payer: "187,82",
        regle: "10/01/2021",
        mode: false,
        unite: "Cachets:",
        cumuls: [["2,00", "2,00"], ["300,00", "300,00"], ["850,00", "850,00"], ["555,53", "555,53"]],
        total_verse: "555,53",
    }
}

// ------------------------------------------------------------------------------------------- gabarit B

/// (x, libelle, famille attendue, base, taux salarial, part salarie, part employeur)
pub type RangB = (f64, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str);

pub struct B {
    pub entreprise: [&'static str; 3],
    pub periode: &'static str,
    pub periode_courte: &'static str,
    pub reglement: &'static str,
    pub objet: &'static str,
    pub convention: [&'static str; 2],
    pub analytique: &'static str,
    pub emploi: &'static str,
    pub conges: Option<&'static str>,
    /// (quantite, libelle, base, montant)
    pub lignes: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    pub brut: (&'static str, &'static str),
    pub cotisations: Vec<RangB>,
    pub total: (&'static str, &'static str, &'static str),
    pub apres_total: Vec<RangB>,
    pub net_social: Option<&'static str>,
    pub non_soumis: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    pub net_avant_impot: &'static str,
    /// L'en-tete du PAS : (x, mot) ; puis la ligne du PAS (libelle, [(x droite, valeur)]).
    pub entete_pas: Vec<(f64, &'static str)>,
    pub pas: (&'static str, Vec<(f64, &'static str)>),
    pub net_imposable: Option<(&'static str, &'static str)>,
    /// Les cumuls : (libelle, periode, cumul).
    pub cumuls: Vec<(&'static str, &'static str, &'static str)>,
    pub net_paye: &'static str,
    /// « Total verse par » : avec le cadre de l'allegement (2020-2022), la valeur dessous ; sinon a droite.
    pub allegement: bool,
    pub total_verse: &'static str,
}

pub fn page_b(p: &B) -> Vec<Value> {
    let mut b = Vec::new();
    let b_ = &mut b;
    grand(b_, 39.0, 36.0, p.entreprise[0], 10.0);
    grand(b_, 375.0, 38.0, "Bulletin de paie", 14.0);
    t(b_, 545.6, 36.0, "99");
    t(b_, 39.0, 54.0, p.entreprise[1]);
    t(b_, 375.0, 55.0, "Période");
    t(b_, 443.0, 55.0, p.periode);
    t(b_, 39.0, 76.0, p.entreprise[2]);
    t(b_, 375.0, 67.5, p.periode_courte);
    t(b_, 39.0, 93.0, "Siret");
    t(b_, 87.0, 93.0, "999 999 999 00025");
    t(b_, 39.0, 102.0, "Code APE");
    t(b_, 87.0, 102.0, "9001Z");
    t(b_, 375.0, 107.0, "Règlement");
    t(b_, 443.0, 107.0, p.reglement);
    t(b_, 375.0, 121.0, "Numéro d'objet");
    t(b_, 443.0, 121.0, p.objet);
    t(b_, 39.0, 126.5, &format!("Convention Collective : {}", p.convention[0]));
    t(b_, 375.0, 133.0, "Analytique");
    t(b_, 443.0, 133.0, p.analytique);
    t(b_, 39.0, 135.7, p.convention[1]);
    t(b_, 39.0, 145.0, "Durée congés payés : code du travail, art L3141-3 à 11");
    grand(b_, 314.0, 175.0, "Fanchon BRIOCHE", 10.0);
    t(b_, 39.0, 176.5, "Emploi");
    t(b_, 108.0, 176.5, p.emploi);
    t(b_, 39.0, 187.5, "Statut");
    t(b_, 108.0, 187.5, "Artiste");
    t(b_, 314.0, 200.0, "3 impasse des Essais");
    t(b_, 39.0, 209.5, "NIR");
    t(b_, 108.0, 209.5, "2 88 11 75 999 001 46");
    if let Some(c) = p.conges {
        t(b_, 39.0, 220.5, "N° Congés Sp.");
        t(b_, 108.0, 220.5, c);
    }
    t(b_, 314.0, 222.0, "99999");
    t(b_, 369.0, 222.0, "FICTIVE");
    t(b_, 314.0, 233.0, "FRANCE");
    t(b_, 499.0, 264.0, "XYZ");
    t(b_, 399.8, 279.5, "Taux");
    t(b_, 458.2, 279.5, "Part");
    t(b_, 523.2, 279.5, "Part");
    t(b_, 126.2, 286.5, "libellé");
    t(b_, 339.7, 286.5, "base");
    t(b_, 394.5, 290.0, "salarial");
    t(b_, 452.7, 290.0, "salarié");
    t(b_, 509.2, 290.0, "employeur");
    let mut y = 308.5;
    let ligne = |b_: &mut Vec<Value>, y: f64, (q, libelle, base, montant): (&str, &str, &str, &str)| {
        t(b_, 59.5, y, &format!("{} ", q));
        t(b_, 74.0, y, libelle);
        r(b_, 374.4, y, base);
        r(b_, 494.4, y, montant);
    };
    for l in &p.lignes {
        ligne(b_, y, *l);
        y += 11.0;
    }
    t(b_, 74.0, y, p.brut.0);
    r(b_, 494.4, y, p.brut.1);
    let cotisation = |b_: &mut Vec<Value>, y: f64, c: &RangB| {
        t(b_, c.0, y, c.1);
        if !c.3.is_empty() {
            r(b_, 374.4, y, c.3);
        }
        if !c.4.is_empty() {
            r(b_, 419.7, y, c.4);
            t(b_, 421.9, y, "%");
        }
        if !c.5.is_empty() {
            r(b_, 495.0, y, c.5);
        }
        if !c.6.is_empty() {
            r(b_, 559.0, y, c.6);
        }
    };
    y += 9.0;
    for c in &p.cotisations {
        y += if c.0 < 50.0 { 13.0 } else { 10.0 };
        cotisation(b_, y, c);
    }
    y += 13.0;
    t(b_, 74.0, y, p.total.0);
    r(b_, 494.4, y, p.total.1);
    r(b_, 558.3, y, p.total.2);
    for c in &p.apres_total {
        y += 12.0;
        cotisation(b_, y, c);
    }
    if let Some(n) = p.net_social {
        y += 18.0;
        t(b_, 74.0, y, "MONTANT NET SOCIAL");
        r(b_, 494.2, y, n);
    }
    for l in &p.non_soumis {
        y += 18.0;
        ligne(b_, y, *l);
    }
    let mut y = 655.0;
    t(b_, 39.0, y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, 494.2, y, p.net_avant_impot);
    y += 19.0;
    t(b_, 99.6, y, "Impôt sur le revenu");
    for (x, mot) in &p.entete_pas {
        t(b_, *x, y, mot);
    }
    if let Some((n, c)) = p.net_imposable {
        y += 17.0;
        t(b_, 39.0, y, "Montant net imposable");
        r(b_, 486.2, y, n);
        r(b_, 563.0, y, c);
        y += 13.0;
        t(b_, 39.0, y, "Montant net des HC/HS/RTT exonérées");
        r(b_, 563.0, y, "0,00");
    }
    y += 16.0;
    t(b_, 39.0, y, p.pas.0);
    for (x, v) in &p.pas.1 {
        r(b_, *x, y + 0.5, v);
    }
    let mut y = 737.0;
    t(b_, 39.0, y, "BASE");
    t(b_, 131.0, y, "Période");
    t(b_, 189.0, y, "Cumul");
    t(b_, 475.0, y + 3.0, "Net payé");
    for (libelle, periode, cumul) in &p.cumuls {
        y += 11.0;
        t(b_, 39.0, y, libelle);
        if !periode.is_empty() {
            r(b_, 165.0, y, periode);
            r(b_, 220.0, y, cumul);
        }
    }
    grand(b_, 493.3, 765.0, &format!("{} €", p.net_paye), 14.0);
    t(b_, 445.0, 787.0, "Total versé par");
    t(b_, 450.2, 795.0, "l'employeur");
    if p.allegement {
        t(b_, 510.0, 787.0, "Allègement de");
        t(b_, 506.0, 795.0, "cotisations empl.");
        grand(b_, 460.0, 812.0, &format!("{} €", p.total_verse), 11.0);
        grand(b_, 532.0, 812.0, "0,00 €", 11.0);
    } else {
        grand(b_, 524.7, 792.0, &format!("{} €", p.total_verse), 11.0);
    }
    grand(b_, 504.7, 828.0, "GHS - sPAIEctacle ©", 6.5);
    b
}

/// B : cachets isoles, une cotisation facultative apres le total, un defraiement non soumis (2026).
pub fn b_cachets() -> B {
    let s = "";
    B {
        entreprise: ["Les Ateliers Fictifs", "Maison des Essais", "99998 Fictiveville"],
        periode: "du 02/04/26 au 03/04/26",
        periode_courte: "du 2 au 3/04/26",
        reglement: "29/04/26 par virement",
        objet: "18 6Z 999999 01",
        convention: ["Entreprises du secteur privé du", "spectacle vivant"],
        analytique: "FICTION",
        emploi: "Artiste dramatique",
        conges: Some("Z 000 0001"),
        lignes: vec![("3", "Cachets isolés", "110,00", "330,00")],
        brut: ("MONTANT BRUT", "330,00"),
        cotisations: vec![
            (39.0, "Santé", s, s, s, s, s),
            (74.0, "Sécurité Sociale - Maladie Maternité Invalidité Décès", "sante", s, s, s, "23,10"),
            (74.0, "Prévoyance, Incapacité, Invalidité, Décès, Autres", "sante", "330,00", "0,120", "0,40", "3,66"),
            (39.0, "Accidents du travail & maladies professionnelles", "accidents_du_travail", s, s, s, "3,47"),
            (39.0, "Retraite", s, s, s, s, s),
            (74.0, "Sécurité Sociale Vieillesse plafonnée", "retraite", "330,00", "4,830", "15,94", "19,77"),
            (74.0, "Sécurité Sociale Vieillesse déplafonnée", "retraite", "330,00", "0,280", "0,92", "4,88"),
            (74.0, "Retraite complémentaire, CEG et CET T1", "retraite", "330,00", "5,300", "17,49", "18,94"),
            (39.0, "Famille", "famille", s, s, s, "11,62"),
            (39.0, "Assurance chômage", "assurance_chomage", "330,00", "2,400", "7,92", "32,18"),
            (39.0, "Congés Spectacles", "conges_spectacles", s, s, s, "51,15"),
            (39.0, "Autres charges dues par l\u{2019}employeur", "autres_contributions", s, s, s, "10,37"),
            (39.0, "Cotisations statutaires ou prévues par la convention collective", "cotisations_conventionnelles", s, s, s, "3,37"),
            (39.0, "CSG déductible de l'impôt sur le revenu", "csg", "324,23", "6,800", "22,05", s),
            (39.0, "CSG/CRDS non déductible de l'impôt sur le revenu", "csg", "324,23", "2,900", "9,40", s),
        ],
        total: ("TOTAL COTISATIONS ET CONTRIB. OBLIGATOIRES", "74,12", "182,51"),
        apres_total: vec![
            (39.0, "Cotisations et contributions sociales facultatives", s, s, s, s, s),
            (74.0, "Mutuelle fictive facultative", "cotisations_facultatives", "330,00", "0,500", "1,65", "1,65"),
        ],
        net_social: Some("255,88"),
        non_soumis: vec![("2", "Défraiement non soumis", "18,50", "37,00")],
        net_avant_impot: "291,23",
        entete_pas: vec![(280.9, "base"), (339.0, "taux personnalisé"), (442.2, "montant"), (502.7, "cumul annuel")],
        pas: ("IMPOT SUR LE REVENU PRELEVE A LA SOURCE", vec![(316.1, "263,90"), (412.2, "1,10"), (487.0, "2,90"), (563.0, "2,90")]),
        net_imposable: Some(("263,90", "1 263,90")),
        cumuls: vec![("Cachets", "3,00", "7,00"), ("Jours", "2,00", "6,00"), ("Salaire brut", "330,00", "1 120,00")],
        net_paye: "288,33",
        allegement: false,
        total_verse: "514,16",
    }
}

/// B : 2020, l'activite partielle (libelles absents de la table : signales, gardes), le brut a zero.
pub fn b_activite_partielle() -> B {
    let s = "";
    B {
        entreprise: ["CIE DES ESSAIS", "2-4 rue du Général Fictif", "99997 FICTIVILLE"],
        periode: "du 08/06/20 au 12/06/20",
        periode_courte: "du 8 au 12/06/20",
        reglement: "12/06/20",
        objet: "17 7Z 999999 02",
        convention: ["Entreprises Artistiques et", "Culturelles"],
        analytique: "ESSAI",
        emploi: "Artiste Musicien",
        conges: None,
        lignes: vec![
            ("3", "Cachet en activité partielle", "160,00", "480,00"),
            ("3", "Absence cachet en activité partielle", "-160,00", "-480,00"),
        ],
        brut: ("Salaire brut", "0,00"),
        cotisations: vec![
            (38.0, "SANTE", s, s, s, s, s),
            (73.0, "Complémentaire Incapacité Invalidité Décès", "sante", s, s, s, "1,50"),
            (38.0, "CONGES SPECTACLES", "conges_spectacles", s, s, s, "52,00"),
            (38.0, "CSG déductible de l'impôt sur le revenu", s, s, s, s, s),
            (73.0, "CSG déductible de l'impôt sur le revenu", "csg", "330,12", "3,800", "12,54", s),
            (73.0, "CSG - écrêtement sur ret. déductible", "csg", "-12,54", "100,000", "-12,54", s),
            (38.0, "CSG/CRDS non déductible de l'impôt sur le revenu", "csg", "331,50", "2,900", "9,61", s),
            (73.0, "CSG - écrêtement sur ret. non déductible", "csg", "-9,51", "100,000", "-9,51", s),
        ],
        total: ("Total des Cotisations et Contributions", "0,10", "53,50"),
        apres_total: vec![],
        net_social: None,
        non_soumis: vec![("3", "Indemnisation cachet en activité partielle", "112,00", "336,00")],
        net_avant_impot: "335,90",
        entete_pas: vec![(341.0, "base"), (394.0, "taux non personnalisé"), (515.0, "montant")],
        pas: ("Impôt sur le revenu prélevé à la source", vec![(374.0, "0,00"), (452.0, "0,00"), (558.0, "0,00")]),
        net_imposable: None,
        cumuls: vec![("Jours", "3,00", "4,00"), ("Salaire brut", "", ""), ("Net imposable", "335,98", "410,00"), ("Montant PAS", "0,00", "0,00")],
        net_paye: "335,90",
        allegement: true,
        total_verse: "53,50",
    }
}

/// Une page sans un mot : un trait seul, aucune police (comme une impression en traces).
pub fn pdf_en_traces() -> Vec<u8> {
    let flux = "10 10 m 190 190 l S";
    let objets = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", flux.len(), flux),
    ];
    let mut o: Vec<u8> = b"%PDF-1.7\n\n".to_vec();
    let mut positions = Vec::new();
    for (k, corps) in objets.iter().enumerate() {
        positions.push(o.len());
        o.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", k + 1, corps).as_bytes());
    }
    let debut = o.len();
    o.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objets.len() + 1).as_bytes());
    for p in positions {
        o.extend_from_slice(format!("{:010} 00000 n \n", p).as_bytes());
    }
    o.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objets.len() + 1, debut).as_bytes());
    o
}

