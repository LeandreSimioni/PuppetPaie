//! Les PDF FICTIFS de bulletins, communs aux tests (`bulletin_lu`, `routes::import`). La fabrique elle-meme (les
//! gabarits CoolPaie (A) et sPAIEctacle (B), le PDF) est celle de l'outil d'atelier `moteur-atelier fabriquer-bulletins` :
//! `src/atelier/bulletins_fictifs.rs`, inclus ici tel quel (un seul code). Ici, les exemples des tests, et les pages que seuls
//! les tests fabriquent (le certificat Congés Spectacles, l'AEM, un PDF en traces) : noms, adresses,
//! NIR, Siret, numeros d'objet, titres et montants inventes ; rien de `prive/`, aucun des PDF reels de `import/`.

#![allow(dead_code)]

#[path = "../../src/atelier/bulletins_fictifs.rs"]
mod fabrique;
pub use fabrique::*;
use fabrique_pdf::forme::Bloc;

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

// ------------------------------------------------------------------------------------------- le certificat Congés Spectacles

/// UN CERTIFICAT D'EMPLOI CONGES SPECTACLES, a la mise en page qu'Audiens impose (le titre, la Caisse, les rubriques
/// « N° SIRET », « COMPTE », « N° CE », « NIR », « NOM DE NAISSANCE »..., le bloc d'adresse du salarie au-dessus), les
/// dates ecrites `JJ MM AAAA` ou `JJ/MM/AAAA` selon le logiciel (`dates_a_barres`).
pub struct Certificat<S = &'static str> {
    pub siret: S,
    pub compte: S,
    pub numero: S,
    pub employeur: S,
    /// Quinze caracteres, sans espace : il s'imprime en treize, puis sa cle.
    pub nir: S,
    pub nom: S,
    pub prenom: S,
    /// La rue, « code postal commune ».
    pub adresse: [S; 2],
    pub emploi: S,
    pub cadre: bool,
    /// JJ/MM/AAAA.
    pub debut: S,
    pub fin: S,
    pub nombre: S,
    pub base_conge: S,
    pub brut: S,
    pub dates_a_barres: bool,
}

/// La page d'un certificat.
pub fn page_certificat<S: AsRef<str>>(c: &Certificat<S>) -> Vec<Bloc> {
    let mut b = Vec::new();
    let date = |d: &str| if c.dates_a_barres { d.to_string() } else { d.replace('/', " ") };
    let siret = c.siret.as_ref();
    let nir = c.nir.as_ref();
    grand(&mut b, 322.8, 150.0, &format!("{} {}", c.prenom.as_ref(), c.nom.as_ref()), 11.0);
    grand(&mut b, 322.8, 166.0, c.adresse[0].as_ref(), 11.0);
    grand(&mut b, 322.8, 182.0, c.adresse[1].as_ref(), 11.0);
    grand(&mut b, 49.0, 432.0, "CERTIFICAT D'EMPLOI", 11.0);
    grand(&mut b, 212.0, 430.0, "LES CONGES SPECTACLES", 14.0);
    grand(&mut b, 49.0, 444.0, "DESTINÉ AU SALARIÉ", 11.0);
    grand(&mut b, 48.0, 568.0, "N° SIRET", 14.0);
    grand(&mut b, 123.6, 568.0, &format!("{} {}", &siret[..siret.len().min(9)], &siret[siret.len().min(9)..]), 14.0);
    grand(&mut b, 253.9, 568.0, "COMPTE", 14.0);
    grand(&mut b, 338.4, 568.0, c.compte.as_ref(), 14.0);
    grand(&mut b, 431.0, 568.0, "N° CE", 14.0);
    grand(&mut b, 503.5, 568.0, c.numero.as_ref(), 14.0);
    grand(&mut b, 55.0, 603.0, "NIR", 14.0);
    grand(&mut b, 112.8, 603.0, &nir[..nir.len().min(13)], 14.0);
    grand(&mut b, 229.7, 603.0, &nir[nir.len().min(13)..], 14.0);
    grand(&mut b, 319.0, 603.0, c.employeur.as_ref(), 14.0);
    grand(&mut b, 58.1, 626.3, "NOM DE NAISSANCE", 9.0);
    grand(&mut b, 174.0, 626.3, c.nom.as_ref(), 9.0);
    grand(&mut b, 319.0, 626.3, "PRENOM", 9.0);
    grand(&mut b, 390.0, 626.3, c.prenom.as_ref(), 9.0);
    grand(&mut b, 58.1, 636.4, "NOM D'USAGE", 9.0);
    grand(&mut b, 319.0, 636.4, "PSEUDONYME", 9.0);
    grand(&mut b, 57.8, 646.2, "EMPLOI", 9.0);
    grand(&mut b, 174.0, 646.2, c.emploi.as_ref(), 9.0);
    grand(&mut b, 57.8, 656.3, "CADRE", 9.0);
    grand(&mut b, 174.0, 656.3, if c.cadre { "OUI" } else { "NON" }, 9.0);
    grand(&mut b, 160.0, 674.3, "DATES DE TRAVAIL", 11.0);
    grand(&mut b, 407.0, 674.3, "NB JOURS OU CACHETS", 11.0);
    grand(&mut b, 51.8, 693.0, "DEBUT", 14.0);
    grand(&mut b, 126.0, 693.0, &date(c.debut.as_ref()), 14.0);
    grand(&mut b, 235.0, 693.0, "FIN", 14.0);
    grand(&mut b, 266.9, 693.0, &date(c.fin.as_ref()), 14.0);
    grand(&mut b, 457.4, 693.0, c.nombre.as_ref(), 14.0);
    grand(&mut b, 70.8, 725.4, "BASE CONGE", 14.0);
    grand(&mut b, 248.6, 725.4, c.base_conge.as_ref(), 14.0);
    grand(&mut b, 319.0, 725.4, "SALAIRE BRUT", 14.0);
    grand(&mut b, 470.6, 725.4, c.brut.as_ref(), 14.0);
    grand(&mut b, 72.0, 740.3, "Arrondie à l'euro le plus proche", 8.0);
    grand(&mut b, 319.0, 740.3, "Arrondi à l'euro le plus proche", 8.0);
    grand(&mut b, 355.0, 752.0, "Date :", 9.0);
    grand(&mut b, 441.8, 752.0, &date(c.fin.as_ref()), 9.0);
    grand(&mut b, 354.0, 764.0, "Nom du signataire", 9.0);
    grand(&mut b, 105.1, 774.0, "N° d'agrément :", 9.0);
    grand(&mut b, 355.0, 774.0, "Signature", 9.0);
    b
}

// ------------------------------------------------------------------------------------------- l'AEM en PDF

/// UNE ATTESTATION D'EMPLOYEUR MENSUELLE (annexes 8 et 10) telle qu'un logiciel l'edite sur le formulaire d'Unédic :
/// les rubriques sur leurs lignes (« N° SIRET », « Raison sociale », « Nom de famille », « NIR »...), les cases cochees
/// d'un « Ñ » a droite de leur libelle, les heures, cachets et jours en colonnes sous leurs en-tetes sur deux rangs, le
/// brut sous « SALAIRES BRUTS ». Les nombres vides (`""`) ne s'impriment pas.
pub struct Aem<S = &'static str> {
    /// MM/AAAA.
    pub mois: S,
    pub numero: S,
    pub siret: S,
    pub ape: S,
    pub idcc: S,
    pub employeur: S,
    pub employeur_code_postal: S,
    pub employeur_commune: S,
    pub adherent: S,
    pub nom: S,
    pub prenom: S,
    /// Quinze caracteres, sans espace.
    pub nir: S,
    /// JJ/MM/AAAA.
    pub naissance: S,
    pub rue: S,
    pub code_postal: S,
    pub commune: S,
    pub emploi: S,
    pub objet: S,
    pub cadre: bool,
    pub artiste: bool,
    /// JJ/MM/AAAA.
    pub embauche: S,
    pub fin: S,
    pub heures: S,
    pub isoles: S,
    pub groupes: S,
    pub jours: S,
    /// « 590,00 ».
    pub brut: S,
}

/// La page d'une AEM.
pub fn page_aem<S: AsRef<str>>(a: &Aem<S>) -> Vec<Bloc> {
    let mut b = Vec::new();
    // Une ligne : chaque texte a son abscisse, repoussee au-dela du precedent (un blanc de 10 points entre deux).
    let ligne = |b: &mut Vec<Bloc>, y: f64, textes: &[(f64, &str)]| {
        let mut fin = 0.0f64;
        for (x, texte) in textes {
            if texte.is_empty() {
                continue;
            }
            let x = x.max(fin + 10.0);
            t(b, x, y, texte);
            fin = x + 6.5 * texte.chars().count() as f64;
        }
    };
    let jma = |d: &S| -> Vec<String> { d.as_ref().split('/').map(String::from).collect() };
    let coche = |l: &str, oui: bool| if oui { format!("{} Ñ", l) } else { l.to_string() };
    let (mm, an) = a.mois.as_ref().split_once('/').unwrap_or(("00", "0000"));
    let (n, e, f) = (jma(&a.naissance), jma(&a.embauche), jma(&a.fin));
    ligne(&mut b, 35.0, &[(40.0, "ACTIVITÉS RELEVANT DES ANNEXES 8 et 10 au règlement de l'Assurance Chômage")]);
    ligne(&mut b, 47.0, &[(40.0, "À REMPLIR PAR L'EMPLOYEUR ET À REMETTRE AU SALARIÉ")]);
    ligne(
        &mut b,
        78.0,
        &[(30.0, "1 ATTESTATION (AEM)"), (0.0, "MOIS"), (0.0, &mm[..1]), (0.0, &mm[1..]), (0.0, "20"), (0.0, &an[2..3]), (0.0, &an[3..]), (0.0, "ATTESTATION N°"), (0.0, a.numero.as_ref())],
    );
    ligne(&mut b, 104.0, &[(30.0, "AEM INITIALE Ñ"), (0.0, "COMPLÉMENTAIRE"), (0.0, "RECTIFICATIVE POSITIVE"), (0.0, "OU NÉGATIVE")]);
    ligne(&mut b, 114.0, &[(300.0, "N° DE L'ATTESTATION INITIALE")]);
    ligne(&mut b, 130.0, &[(30.0, "2 EMPLOYEUR")]);
    ligne(&mut b, 146.0, &[(30.0, "N° SIRET"), (0.0, a.siret.as_ref()), (0.0, "Code APE/NAF"), (0.0, a.ape.as_ref()), (0.0, "N° d'affiliation"), (0.0, "99999999999")]);
    ligne(&mut b, 158.0, &[(30.0, "N° IDCC"), (0.0, a.idcc.as_ref()), (0.0, "Si différent, N° IDCC de la prestation de travail")]);
    ligne(&mut b, 172.0, &[(30.0, "Raison sociale"), (0.0, a.employeur.as_ref())]);
    ligne(&mut b, 184.0, &[(30.0, "Code Postal"), (0.0, a.employeur_code_postal.as_ref()), (0.0, "Commune"), (0.0, a.employeur_commune.as_ref()), (0.0, "Courriel")]);
    ligne(&mut b, 198.0, &[(30.0, "N° d'affiliation à la caisse OUI Ñ"), (0.0, a.adherent.as_ref()), (0.0, "NON")]);
    ligne(&mut b, 207.0, &[(30.0, "de congés spectacles")]);
    ligne(&mut b, 230.0, &[(30.0, "3 SALARIE")]);
    ligne(&mut b, 246.0, &[(30.0, "Nom de famille"), (0.0, a.nom.as_ref()), (0.0, "Prénom"), (0.0, a.prenom.as_ref())]);
    ligne(&mut b, 256.0, &[(30.0, "(Nom de naissance)")]);
    ligne(&mut b, 270.0, &[(30.0, "Nom d'usage")]);
    ligne(&mut b, 286.0, &[(30.0, "NIR"), (0.0, a.nir.as_ref()), (0.0, "Date de naissance"), (0.0, &n[0]), (0.0, &n[1]), (0.0, &n[2])]);
    ligne(&mut b, 302.0, &[(30.0, "Adresse"), (0.0, a.rue.as_ref())]);
    ligne(&mut b, 318.0, &[(30.0, "Code Postal"), (0.0, a.code_postal.as_ref()), (0.0, "Commune"), (0.0, a.commune.as_ref())]);
    ligne(&mut b, 340.0, &[(30.0, "4 PRESTATION DE TRAVAIL")]);
    ligne(&mut b, 356.0, &[(30.0, "Emploi occupé"), (0.0, a.emploi.as_ref()), (0.0, "Numéro Objet"), (0.0, a.objet.as_ref())]);
    // « Non » au-dessus de « cadre » : « Non cadre » sur deux rangs, comme le formulaire.
    ligne(&mut b, 366.0, &[(150.0, "Non")]);
    ligne(
        &mut b,
        374.0,
        &[(30.0, &coche("Cadre", a.cadre)), (150.0, &coche("cadre", !a.cadre)), (0.0, "Réalisateur"), (0.0, &coche("Artiste", a.artiste)), (0.0, &coche("Technicien", !a.artiste)), (0.0, "Ouvrier")],
    );
    ligne(&mut b, 392.0, &[(30.0, "Date d'embauche"), (0.0, &e[0]), (0.0, &e[1]), (0.0, &e[2]), (0.0, "à Contrat en cours")]);
    ligne(&mut b, 404.0, &[(30.0, "Sinon à Date de fin du contrat de travail"), (0.0, &f[0]), (0.0, &f[1]), (0.0, &f[2])]);
    ligne(&mut b, 414.0, &[(30.0, "(Date de début du contrat)")]);
    // Les en-tetes des nombres, sur deux rangs, et les nombres a leur aplomb.
    ligne(&mut b, 432.0, &[(40.0, "Nombre d'"), (110.0, "HEURES"), (200.0, "Nombre de"), (330.0, "Nombre de")]);
    ligne(&mut b, 442.0, &[(40.0, "effectuées"), (200.0, "CACHETS*"), (330.0, "JOURS")]);
    ligne(&mut b, 452.0, &[(180.0, "isolés"), (240.0, "groupés"), (330.0, "travaillés")]);
    ligne(&mut b, 470.0, &[(122.0, a.heures.as_ref()), (186.0, a.isoles.as_ref()), (250.0, a.groupes.as_ref()), (345.0, a.jours.as_ref())]);
    ligne(&mut b, 484.0, &[(100.0, "* Uniquement pour les artistes (voir notice)")]);
    ligne(&mut b, 500.0, &[(150.0, "RÉMUNERATION VERSÉES AU COURS DU MOIS")]);
    ligne(&mut b, 512.0, &[(40.0, "SALAIRES BRUTS"), (220.0, "SALAIRES BRUTS")]);
    ligne(&mut b, 521.0, &[(220.0, "soumis à contributions d'assurance chômage *")]);
    ligne(&mut b, 536.0, &[(55.0, a.brut.as_ref()), (235.0, a.brut.as_ref())]);
    ligne(&mut b, 600.0, &[(30.0, "5 AUTHENTIFICATION PAR L'EMPLOYEUR")]);
    b
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
