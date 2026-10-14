//! `caviardage` (lot E1 du plan d'import des bulletins) : rien de personnel ne reste d'un texte de
//! bulletin, et ce dont l'editeur a besoin (montants, taux, dates de periode, libelles, numero
//! d'objet, Siret, NAF) reste.
//!
//! TOUT EST FICTIF : les deux bulletins sont ecrits ici, aux gabarits decrits par les consignes du
//! lot B (A, CoolPaie ; B, logiciel non identifie), avec des valeurs inventees. Les NIR sont de forme
//! fabriquee (departement 99, commune 999), les Siret commencent par 900 000, les IBAN sont faits de
//! zeros. Aucun PDF de `import/` n'est lu.

use paie::caviardage::{caviarder, Caviarde, Connus};
use regex::Regex;

// ------------------------------------------------------------------ les bulletins

/// Gabarit A (CoolPaie) : deux colonnes fusionnees sur une ligne (l'entreprise a gauche, la salariee
/// a droite, separees par un grand blanc), comme la fabrique rend une page.
const BULLETIN_A: &str = "\
Bulletin de paie n° 12/2026
numéro d'objet : 266Z00000001
Spectacle : Le Grand Nulle Part
Compagnie des Fausses Pistes    Lucienne VERGNAUD-BRASSAC
5 impasse des Tilleuls    8 rue des Glycines
44000 NANTES    69003 LYON
Siret : 900 000 000 00018    Date de naissance : 23/07/1985
NAF : 9001Z    N° Sécurité Sociale : 2 85 07 99 999 123 45
Urssaf : Pays de la Loire    Code Salarié : VERG01
Conv. collective : 3090    Emploi : Comédienne
Licence : PLATESV-R-2026-000001    Statut : Artiste
Abattement : non    N° de Congés Spectacles : X 012 3456
Périodes : du 14/01/2026 au 24/01/2026
Dates travaillées : du 14/01/2026 au 16/01/2026, du 22/01/2026 au 24/01/2026
LIBELLE Base Part Employeur Taux Montant Part Salarié Taux Montant
6 Service(s) répét. 4h (contrat<5j) 60,00 360,00
3 Cachets isolés 150,00 450,00
SALAIRE BRUT 810,00
SANTE
Maladie 810,00 7,000 % 56,70
ACCIDENTS DU TRAVAIL - MALADIES PROFESSIONNELLES
Accident du travail 810,00 1,250 % 10,13
RETRAITE
Vieillesse plafonnée 810,00 8,550 % 69,26 6,900 % 55,89
Retraite complémentaire AGIRC-ARRCO T1 810,00 6,010 % 48,68 4,010 % 32,48
FAMILLE
Allocations familiales 810,00 3,450 % 27,95
ASSURANCE CHOMAGE
Chômage 810,00 4,050 % 32,81
CONGES SPECTACLES
Congés Spectacles 810,00 15,500 % 125,55
CSG ET CRDS
CSG déductible 795,83 6,800 % 54,12
Total des Cotisations et Contributions 314,32 142,49
NET SOCIAL 667,51
NET A PAYER AVANT IMPOT 667,51
Impôt sur le revenu prélevé à la source 682,17 4,830 % 32,95
Net Imposable 682,17
Cachets 3 9    Salaire brut 810,00 2 430,00
Réglement le : 24/01/2026    mode de règlement : Virement
NET A PAYER AU SALARIE 634,56
Total versé par l'employeur 1 124,32
Lucienne VERGNAUD-BRASSAC - courriel lucienne.vergnaud@exemple.fr - tél. 06 12 34 56 78
IBAN FR76 3000 1000 0000 0000 0000 185
coolpaie";

/// Gabarit B (logiciel non identifie) : reperes en colonne de droite, le salarie (prenom NOM,
/// adresse) sous « Emploi / Statut / NIR / N° Congés Sp. ».
const BULLETIN_B: &str = "\
Les Tréteaux du Coin                         Bulletin de paie
3 place du Marché                            Période du 01/04/2026 au 30/04/2026
13001 MARSEILLE                              Règlement 30/04/2026 virement
Siret 900 000 001 00025                      Numéro d'objet 266Z00000002
Code APE 9001Z                               Analytique TOURNEE PRINTEMPS
Convention Collective 3090
Emploi Statut NIR N° Congés Sp.
Musicien Artiste 1 90 11 99 999 456 78 E0000001
Matthieu BOISSERIN
2-4 rue du Général Imaginaire
75014 PARIS
libellé base Taux salarial Part salarié Part employeur
2 Cachets isolés 200,00 400,00
MONTANT BRUT 400,00
SANTE
Maladie 400,00 7,000 % 28,00
RETRAITE
Vieillesse déplafonnée 400,00 0,400 % 1,60 2,020 % 8,08
TOTAL COTISATIONS ET CONTRIB. OBLIGATOIRES 98,40 180,22
MONTANT NET SOCIAL 301,60
Défraiement non soumis 45,00
NET A PAYER AVANT IMPOT 346,60
IMPOT SUR LE REVENU PRELEVE A LA SOURCE 301,60 0,000 % 0,00
BASE Période Cumul
Cachets 2 14
Salaire brut 400,00 2 800,00";

// ----------------------------------------------------------------- les verifications

/// Sans casse ni accent.
fn plier(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            x => x,
        })
        .collect()
}

/// Le mot (ou la suite de mots) est-il dans la sortie, en entier, sans casse ni accent ?
fn present(sortie: &str, mot: &str) -> bool {
    let re = Regex::new(&format!(r"(?:^|[^\p{{L}}\p{{N}}]){}(?:$|[^\p{{L}}\p{{N}}])", regex::escape(&plier(mot)))).unwrap();
    re.is_match(&plier(sortie))
}

/// Les chiffres d'un numero se retrouvent-ils, a la suite, sur une ligne dont on a ote les blancs,
/// points et tirets ?
fn chiffres_presents(sortie: &str, valeur: &str) -> bool {
    let cible: String = valeur.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    sortie.lines().any(|l| {
        let compacte: String = l.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        compacte.contains(&cible)
    })
}

/// Mot a mot : aucune valeur personnelle ne reste.
fn rien_ne_reste(c: &Caviarde, mots: &[&str], numeros: &[&str]) {
    for m in mots {
        assert!(!present(&c.texte, m), "« {} » est reste :\n{}", m, c.texte);
    }
    for n in numeros {
        assert!(!chiffres_presents(&c.texte, n), "le numero « {} » est reste :\n{}", n, c.texte);
    }
}

/// Ce dont l'editeur a besoin est la, tel quel.
fn tout_reste(c: &Caviarde, gardes: &[&str]) {
    for g in gardes {
        assert!(c.texte.contains(g), "« {} » a disparu :\n{}", g, c.texte);
    }
}

const PERSONNEL_A: [&str; 12] = [
    "Lucienne",
    "VERGNAUD",
    "BRASSAC",
    "Glycines",
    "8 rue des Glycines",
    "69003",
    "LYON",
    "23/07/1985",
    "X 012 3456",
    "VERG01",
    "lucienne.vergnaud@exemple.fr",
    "06 12 34 56 78",
];
const NUMEROS_A: [&str; 4] = ["2 85 07 99 999 123 45", "0612345678", "FR76 3000 1000 0000 0000 0000 185", "X0123456"];

const GARDE_A: [&str; 26] = [
    "Bulletin de paie n° 12/2026",
    "266Z00000001",
    "900 000 000 00018",
    "9001Z",
    "3090",
    "PLATESV-R-2026-000001",
    "du 14/01/2026 au 24/01/2026",
    "du 14/01/2026 au 16/01/2026, du 22/01/2026 au 24/01/2026",
    "LIBELLE Base Part Employeur Taux Montant Part Salarié Taux Montant",
    "6 Service(s) répét. 4h (contrat<5j) 60,00 360,00",
    "3 Cachets isolés 150,00 450,00",
    "SALAIRE BRUT 810,00",
    "Maladie 810,00 7,000 % 56,70",
    "ACCIDENTS DU TRAVAIL - MALADIES PROFESSIONNELLES",
    "Vieillesse plafonnée 810,00 8,550 % 69,26 6,900 % 55,89",
    "Retraite complémentaire AGIRC-ARRCO T1 810,00 6,010 % 48,68 4,010 % 32,48",
    "Congés Spectacles 810,00 15,500 % 125,55",
    "CSG déductible 795,83 6,800 % 54,12",
    "Total des Cotisations et Contributions 314,32 142,49",
    "NET SOCIAL 667,51",
    "Impôt sur le revenu prélevé à la source 682,17 4,830 % 32,95",
    "Cachets 3 9    Salaire brut 810,00 2 430,00",
    "Réglement le : 24/01/2026",
    "NET A PAYER AU SALARIE 634,56",
    "Total versé par l'employeur 1 124,32",
    "Spectacle : Le Grand Nulle Part",
];

const PERSONNEL_B: [&str; 8] =
    ["Matthieu", "BOISSERIN", "Imaginaire", "2-4 rue du Général Imaginaire", "75014", "PARIS", "E0000001", "1 90 11 99 999 456 78"];
const NUMEROS_B: [&str; 2] = ["1 90 11 99 999 456 78", "E0000001"];

const GARDE_B: [&str; 17] = [
    "Les Tréteaux du Coin",
    "Période du 01/04/2026 au 30/04/2026",
    "Règlement 30/04/2026 virement",
    "Siret 900 000 001 00025",
    "Numéro d'objet 266Z00000002",
    "Code APE 9001Z",
    "Convention Collective 3090",
    "Emploi Statut NIR N° Congés Sp.",
    "2 Cachets isolés 200,00 400,00",
    "MONTANT BRUT 400,00",
    "Vieillesse déplafonnée 400,00 0,400 % 1,60 2,020 % 8,08",
    "TOTAL COTISATIONS ET CONTRIB. OBLIGATOIRES 98,40 180,22",
    "MONTANT NET SOCIAL 301,60",
    "Défraiement non soumis 45,00",
    "IMPOT SUR LE REVENU PRELEVE A LA SOURCE 301,60 0,000 % 0,00",
    "BASE Période Cumul",
    "Salaire brut 400,00 2 800,00",
];

fn connus_a() -> Connus {
    Connus {
        noms: vec!["Vergnaud-Brassac".into()],
        prenoms: vec!["Lucienne".into()],
        nirs: vec!["285079999912345".into()],
        naissances: vec!["1985-07-23".into()],
        adresses: vec!["8 rue des Glycines".into()],
        codes_postaux: vec!["69003".into()],
        communes: vec!["Lyon".into()],
        conges_spectacles: vec!["X0123456".into()],
        courriels: vec!["lucienne.vergnaud@exemple.fr".into()],
        telephones: vec!["+33 6 12 34 56 78".into()],
        ibans: vec!["FR7630001000000000000000185".into()],
        entreprise: vec!["Compagnie des Fausses Pistes".into(), "5 impasse des Tilleuls".into(), "44000 NANTES".into()],
    }
}

fn connus_b() -> Connus {
    Connus {
        noms: vec!["Boisserin".into()],
        prenoms: vec!["Matthieu".into()],
        nirs: vec!["1 90 11 99 999 456 78".into()],
        adresses: vec!["2-4 rue du Général Imaginaire".into()],
        codes_postaux: vec!["75014".into()],
        communes: vec!["Paris".into()],
        conges_spectacles: vec!["E 000 0001".into()],
        entreprise: vec!["Les Tréteaux du Coin".into(), "3 place du Marché".into(), "13001 MARSEILLE".into()],
        ..Connus::default()
    }
}

// ---------------------------------------------------------------------- les tests

#[test]
fn gabarit_a_sans_rien_connaitre() {
    let c = caviarder(BULLETIN_A, &Connus::default());
    rien_ne_reste(&c, &PERSONNEL_A, &NUMEROS_A);
    tout_reste(&c, &GARDE_A);
    // La compagnie, en colonne de gauche, n'est pas prise pour un nom.
    tout_reste(&c, &["Compagnie des Fausses Pistes"]);
    for e in ["NOM", "PRENOM", "NIR", "NAISSANCE", "ADRESSE", "CS", "COURRIEL", "TEL", "IBAN", "CODE"] {
        assert!(c.enleves.get(e).copied().unwrap_or(0) > 0, "rien d'enleve pour {} : {:?}\n{}", e, c.enleves, c.texte);
    }
    assert!(c.texte.contains("<NOM>") && c.texte.contains("<PRENOM>") && c.texte.contains("<NIR>"));
}

#[test]
fn gabarit_a_avec_les_connus() {
    // Une ligne de plus, que la forme seule ne prendrait pas (un nom sans prenom) : les connus la
    // prennent, dans toutes ses ecritures.
    let texte = format!("{}\nAttestation pour Mme Vergnaud, née le 23 juillet 1985 (Lucienne)\nBRASSAC L.", BULLETIN_A);
    let c = caviarder(&texte, &connus_a());
    rien_ne_reste(&c, &PERSONNEL_A, &NUMEROS_A);
    rien_ne_reste(&c, &["juillet 1985"], &[]);
    tout_reste(&c, &GARDE_A);
    // L'entreprise est connue : protegee, elle reste en entier.
    tout_reste(&c, &["Compagnie des Fausses Pistes", "5 impasse des Tilleuls", "44000 NANTES"]);
}

#[test]
fn gabarit_b_sans_rien_connaitre() {
    let c = caviarder(BULLETIN_B, &Connus::default());
    rien_ne_reste(&c, &PERSONNEL_B, &NUMEROS_B);
    tout_reste(&c, &GARDE_B);
    // L'analytique en majuscules seules n'est pas un nom.
    tout_reste(&c, &["Analytique TOURNEE PRINTEMPS"]);
}

#[test]
fn gabarit_b_avec_les_connus() {
    let texte = format!("{}\nboisserin matthieu\nM. Boisserin", BULLETIN_B);
    let c = caviarder(&texte, &connus_b());
    rien_ne_reste(&c, &PERSONNEL_B, &NUMEROS_B);
    tout_reste(&c, &GARDE_B);
    tout_reste(&c, &["Les Tréteaux du Coin", "3 place du Marché", "13001 MARSEILLE"]);
}

#[test]
fn les_connus_sans_casse_ni_accent_en_entier_et_en_parties() {
    let connus = Connus {
        noms: vec!["Le Goff-Martineau".into()],
        prenoms: vec!["Élodie".into(), "Jean-Noël".into()],
        ..Connus::default()
    };
    let c = caviarder("ELODIE LE GOFF-MARTINEAU\nelodie martineau\nJEAN NOEL\nMme Le Goff\nTotal 12,00", &connus);
    rien_ne_reste(&c, &["Elodie", "Goff", "Martineau", "Jean", "Noël"], &[]);
    tout_reste(&c, &["Total 12,00"]);
    assert!(c.enleves["NOM"] >= 3 && c.enleves["PRENOM"] >= 3, "{:?}", c.enleves);
}

#[test]
fn chaque_forme() {
    let cas: [(&str, &str, &str); 14] = [
        ("NIR avec espaces", "N° SS : 1 88 02 99 999 363 10 fin", "NIR"),
        ("NIR colle", "188029999936310", "NIR"),
        ("NIR corse", "2 75 03 2A 999 001 22", "NIR"),
        ("date apres naissance", "Date de naissance : 02/03/1990", "NAISSANCE"),
        ("nee le", "née le 5 mars 1991 à Nulle-Part", "NAISSANCE"),
        ("Conges Spectacles espaces", "N° CS X 012 3456", "CS"),
        ("Conges Spectacles colle", "E0000001", "CS"),
        ("courriel", "ecrire a jean.personne@exemple.org svp", "COURRIEL"),
        ("telephone", "tel 01.23.45.67.89", "TEL"),
        ("telephone international", "+33 6 12 34 56 78", "TEL"),
        ("IBAN etranger", "BE68 5390 0754 7034", "IBAN"),
        ("adresse", "2-4 rue du Général Imaginaire", "ADRESSE"),
        ("code postal et commune", "75014 PARIS", "ADRESSE"),
        ("NOM Prenom", "PERSONNE Jean-Michel", "NOM"),
    ];
    for (quoi, texte, etiquette) in cas {
        let c = caviarder(texte, &Connus::default());
        assert!(c.enleves.get(etiquette).copied().unwrap_or(0) >= 1, "{} : {:?} -> {}", quoi, c.enleves, c.texte);
        assert!(c.texte.contains(&format!("<{}>", etiquette)), "{} : {}", quoi, c.texte);
    }
    // La valeur part, ce qui l'entoure reste.
    let c = caviarder("N° SS : 1 88 02 99 999 363 10 fin", &Connus::default());
    assert_eq!(c.texte, "N° SS : <NIR> fin");
    let c = caviarder("Date de naissance : 02/03/1990", &Connus::default());
    assert_eq!(c.texte, "Date de naissance : <NAISSANCE>");
    let c = caviarder("Jean de LA FONTAINE", &Connus::default());
    assert_eq!(c.texte, "<PRENOM> <NOM>");
    let c = caviarder("8 rue des Glycines    Emploi : Comédienne", &Connus::default());
    assert_eq!(c.texte, "<ADRESSE>    Emploi : Comédienne");
}

#[test]
fn ce_qui_ne_part_pas() {
    let texte = "\
du 14/01/2026 au 24/01/2026
Réglement le : 24/01/2026
1 234,56 4,830 %
Numéro d'objet : 266Z00000001
Siret : 900 000 000 00018
Siret 90000000000018
NAF 9001Z
2 Cours de danse 50,00 100,00
Prévoyance AUDIENS non cadre 810,00 0,750 % 6,08
Contribution FNAS 810,00 0,500 % 4,05
Taux PAS 4,830 %
Spectacle : Faites Encore
Conv. collective IDCC 3090";
    let c = caviarder(texte, &Connus::default());
    assert_eq!(c.texte, texte, "rien ne devait partir : {:?}", c.enleves);
    assert_eq!(c.total(), 0);
    assert_eq!(c.resume(), "rien d'enlevé");
}

#[test]
fn le_resume() {
    let c = caviarder("Jean DUPLAIN\nPaul DUPLAIN\nN° SS 1 88 02 99 999 363 10\n8 rue des Glycines\n2 avenue des Lilas", &Connus::default());
    assert_eq!(c.enleves["NOM"], 2);
    assert_eq!(c.enleves["PRENOM"], 2);
    assert_eq!(c.enleves["NIR"], 1);
    assert_eq!(c.enleves["ADRESSE"], 2);
    assert_eq!(c.resume(), "2 noms, 2 prénoms, 1 NIR, 2 adresses enlevés");
}

#[test]
fn une_etiquette_n_est_pas_reprise_et_rien_ne_fuit_des_marques() {
    // Une personne qui s'appelle « Nom » : l'etiquette posee n'est pas reprise par la suite.
    let connus = Connus { noms: vec!["Nom".into()], prenoms: vec!["Prenom".into()], ..Connus::default() };
    let c = caviarder("Prenom NOM", &connus);
    assert_eq!(c.texte, "<PRENOM> <NOM>");
    // Un caractere d'usage prive a l'entree ne derange rien.
    let c = caviarder("a\u{F0000}b Jean DUPLAIN", &Connus::default());
    assert_eq!(c.texte, "a\u{FFFD}b <PRENOM> <NOM>");
}

/// #149 : le code salarie tout en chiffres, seul a la ligne qui suit « Code salarié : » (deux colonnes que la fabrique
/// rend sur deux lignes), part comme celui de la meme ligne ; une date ou un montant de la ligne suivante restent.
#[test]
fn un_code_salarie_en_chiffres_a_la_ligne_suivante_part() {
    let c = caviarder("Code salarié :\n00123\nEmploi : Comédienne", &Connus::default());
    assert_eq!(c.texte, "Code salarié :\n<CODE>\nEmploi : Comédienne", "{:?}", c.enleves);
    let c = caviarder("Matricule\n  4512   Statut : non cadre", &Connus::default());
    assert_eq!(c.texte, "Matricule\n  <CODE>   Statut : non cadre", "{:?}", c.enleves);
    for garde in ["Code salarié :\n14/01/2026", "Code salarié :\n1 234,56"] {
        assert_eq!(caviarder(garde, &Connus::default()).texte, garde);
    }
}
