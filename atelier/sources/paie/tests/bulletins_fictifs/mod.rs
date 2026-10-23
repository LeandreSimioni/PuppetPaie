//! Les PDF FICTIFS de bulletins, communs aux tests (`bulletin_lu`, `routes_importation`). La fabrique elle-meme (les
//! gabarits CoolPaie (A) et sPAIEctacle (B), le PDF) est celle de l'outil d'atelier `paie-atelier fabriquer-bulletins` :
//! `src/atelier/bulletins_fictifs.rs`, inclus ici tel quel (un seul code). Ici, les exemples des tests : noms, adresses,
//! NIR, Siret, numeros d'objet, titres et montants inventes ; rien de `prive/`, aucun des PDF reels de `import/`.

#![allow(dead_code)]

#[path = "../../src/atelier/bulletins_fictifs.rs"]
mod fabrique;
pub use fabrique::*;

// ------------------------------------------------------------------------------------------- gabarit A

/// L'identite des exemples CoolPaie.
pub fn identite_a() -> Identite {
    Identite {
        entreprise: ["Compagnie des Essais", "12 rue Imaginaire", "99998 Nullepart"],
        siret: "99999999900017",
        naf: "9001Z",
        urssaf: "999999999999",
        convention: ["IDCC n° 3090 - Spectacle vivant", "du secteur privé"],
        licence: "L-R-99-9",
        nom: "BRIOCHE",
        prenom: "FANCHON",
        adresse: ["3 IMPASSE DES ESSAIS", "99999 VILLE FICTIVE"],
        naissance: "04/11/1988",
        nir: "288117599900146",
        code: "7",
        conges_spectacles: Some("Z0000001"),
    }
}

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

/// Le total des cotisations de `COTISATIONS_A` : (employeur, salarie).
pub const TOTAL_A: [&str; 2] = ["255,53", "112,18"];

/// Le libelle tel que le lecteur le rend (un titre sur deux rangs se recolle).
pub fn libelle_lu(k: usize) -> String {
    match COTISATIONS_A[k].1 {
        "PROFESSIONNELLES" => "ACCIDENTS DU TRAVAIL - MALADIES PROFESSIONNELLES".into(),
        "CONVENTION COLLECTIVE" => "COTISATIONS STATUTAIRES OU PREVUES PAR LA CONVENTION COLLECTIVE".into(),
        l => l.into(),
    }
}

/// A : cachets isoles + services de repetition, 2026.
pub fn a_repetitions() -> A {
    A {
        id: identite_a(),
        cotisations: COTISATIONS_A.to_vec(),
        total: TOTAL_A,
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
        id: identite_a(),
        cotisations: COTISATIONS_A.to_vec(),
        total: TOTAL_A,
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
        id: identite_a(),
        cotisations: COTISATIONS_A.to_vec(),
        total: TOTAL_A,
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
        id: identite_a(),
        cotisations: COTISATIONS_A.to_vec(),
        total: TOTAL_A,
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
        id: identite_a(),
        cotisations: COTISATIONS_A.to_vec(),
        total: TOTAL_A,
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

/// L'identite des exemples sPAIEctacle : l'entreprise, la convention et le numero Conges Spectacles donnes.
pub fn identite_b(entreprise: [&'static str; 3], convention: [&'static str; 2], conges: Option<&'static str>) -> Identite {
    Identite {
        entreprise,
        siret: "99999999900025",
        naf: "9001Z",
        urssaf: "",
        convention,
        licence: "",
        nom: "BRIOCHE",
        prenom: "Fanchon",
        adresse: ["3 impasse des Essais", "99999 FICTIVE"],
        naissance: "",
        nir: "288117599900146",
        code: "99",
        conges_spectacles: conges,
    }
}

/// B : cachets isoles, une cotisation facultative apres le total, un defraiement non soumis (2026).
pub fn b_cachets() -> B {
    let s = "";
    B {
        id: identite_b(["Les Ateliers Fictifs", "Maison des Essais", "99998 Fictiveville"], ["Entreprises du secteur privé du", "spectacle vivant"], Some("Z 000 0001")),
        periode: "du 02/04/26 au 03/04/26",
        periode_courte: "du 2 au 3/04/26",
        reglement: "29/04/26 par virement",
        objet: "18 6Z 999999 01",
        analytique: "FICTION",
        emploi: "Artiste dramatique",
        statut: "Artiste",
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
        id: identite_b(["CIE DES ESSAIS", "2-4 rue du Général Fictif", "99997 FICTIVILLE"], ["Entreprises Artistiques et", "Culturelles"], None),
        periode: "du 08/06/20 au 12/06/20",
        periode_courte: "du 8 au 12/06/20",
        reglement: "12/06/20",
        objet: "17 7Z 999999 02",
        analytique: "ESSAI",
        emploi: "Artiste Musicien",
        statut: "Artiste",
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
