//! Tests du lecteur de `.modele` (`src/modele.rs`) et du style « contrat » (`src/style_contrat.rs`) :
//! la grammaire, les marqueurs, les refus, et la preuve qu'un document « modèle » donne exactement le PDF de la description à blocs équivalente.

use std::path::{Path, PathBuf};

use fabrique_pdf::modele::{self, Valeurs};
use serde_json::{json, Value};

fn valeurs(paires: &[(&str, &str)]) -> Valeurs {
    paires.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn dossier_temp(nom: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(nom);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ecrire_modele(nom: &str, texte: &str) -> String {
    let f = dossier_temp("modeles").join(nom);
    // Plusieurs tests écrivent le même fichier en parallèle : on l'écrit à côté puis on le
    // renomme, pour qu'un lecteur ne le voie jamais à moitié écrit.
    let provisoire = f.with_extension(format!("{:?}.tmp", std::thread::current().id()).replace(['(', ')'], ""));
    std::fs::write(&provisoire, texte).unwrap();
    std::fs::rename(&provisoire, &f).unwrap();
    f.to_str().unwrap().replace('\\', "/")
}

fn fabriquer(v: &Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::fabriquer(&v.to_string()).map_err(|r| r.0)
}

fn refus(v: &Value) -> String {
    let e = fabriquer(v).expect_err("la description aurait dû être refusée");
    assert!(!e.contains('\n'), "une seule ligne : {:?}", e);
    e
}

const PETIT: &str = "# un commentaire\n\
TITRE: Contrat n° {{contrat.numero}}\n\
SOUSTITRE: Pour **{{salarie.nom}}**\n\
ANNULE:\nAnnule le {{remplace.numero}} du {{remplace.date}}.\n\
PREAMBULE:\n**Entre** nous,\n  et vous.\n\nDeuxième paragraphe.\n\
ARTICLE: Objet\nL'artiste {{ salarie.nom }} joue.\n# commentaire au milieu\nSuite.\n\n\
ARTICLE: Durée\nDu {{contrat.debut}}.\n\
SIGNATURES:\nFait à Paris.\n";

fn vals() -> Valeurs {
    valeurs(&[
        ("contrat.numero", "C-1"),
        ("contrat.debut", "01/01/2025"),
        ("salarie.nom", "DUPONT"),
        ("employeur.signataire", "Jeanne Martin"),
        ("employeur.signataire_fonction", "Présidente"),
        ("salarie.nom_complet", "Paul Dupont"),
    ])
}

// ----------------------------------------------------------------- la grammaire

#[test]
fn la_grammaire_decoupe_comme_le_python() {
    let m = modele::lire_modele(PETIT, "petit.modele").unwrap();
    assert_eq!(m.titre, "Contrat n° {{contrat.numero}}");
    assert_eq!(m.soustitre, "Pour **{{salarie.nom}}**");
    assert_eq!(m.annule, vec!["Annule le {{remplace.numero}} du {{remplace.date}}."]);
    // Les lignes se recollent par un espace, une ligne vide sépare deux paragraphes.
    assert_eq!(m.preambule, vec!["**Entre** nous, et vous.", "Deuxième paragraphe."]);
    assert_eq!(m.articles.len(), 2);
    assert_eq!(m.articles[0].0, "Objet");
    // Le commentaire du milieu disparaît, ses voisines se recollent.
    assert_eq!(m.articles[0].1, vec!["L'artiste {{ salarie.nom }} joue. Suite."]);
    assert_eq!(m.articles[1].0, "Durée");
    assert_eq!(m.signatures, vec!["Fait à Paris."]);
    assert!(m.modification.is_empty());
}

#[test]
fn la_cle_modification_et_les_lignes_avant_toute_cle() {
    let t = "du texte avant toute clé\nMODIFICATION:\nChangé.\nARTICLE: A\nx\n";
    let m = modele::lire_modele(t, "m").unwrap();
    assert_eq!(m.modification, vec!["Changé."]);
    assert_eq!(m.articles.len(), 1);
}

#[test]
fn fins_de_ligne_crlf_et_commentaire_indente() {
    let t = "TITRE: T\r\n   # commentaire indenté\r\nARTICLE: A\r\nUn\r\nDeux\r\n";
    let m = modele::lire_modele(t, "m").unwrap();
    assert_eq!(m.titre, "T");
    assert_eq!(m.articles[0].1, vec!["Un Deux"]);
}

#[test]
fn un_modele_sans_article_est_refuse() {
    let e = modele::lire_modele("TITRE: T\nPREAMBULE:\nx\n", "mon.modele").unwrap_err();
    assert_eq!(e.0, "mon.modele ne contient aucun article");
}

// ------------------------------------------------------------- les marqueurs

#[test]
fn les_marqueurs_se_substituent_avec_ou_sans_blancs() {
    let v = vals();
    let t = modele::substituer("{{contrat.numero}}|{{ contrat.numero }}|{{\tcontrat.debut\n}}", &v).unwrap();
    assert_eq!(t, "C-1|C-1|01/01/2025");
    // Ce qui n'est pas un marqueur reste tel quel (majuscules, un seul mot, accolades seules).
    let t = modele::substituer("{{Contrat.numero}} {{numero}} { {contrat.numero} } {{", &v).unwrap();
    assert_eq!(t, "{{Contrat.numero}} {{numero}} { {contrat.numero} } {{");
    // La valeur substituée n'est pas relue.
    let v2 = valeurs(&[("a.b", "{{c.d}}")]);
    assert_eq!(modele::substituer("{{a.b}}", &v2).unwrap(), "{{c.d}}");
}

#[test]
fn un_marqueur_inconnu_les_nomme_tous_et_liste_les_disponibles() {
    let v = valeurs(&[("b.y", "1"), ("a.x", "2")]);
    let e = modele::substituer("{{a.x}} {{contrat.inconnu}} {{z.z}} {{contrat.inconnu}}", &v).unwrap_err();
    assert_eq!(
        e.0,
        "marqueur inconnu dans le modèle de contrat : {{contrat.inconnu}}, {{z.z}}, {{contrat.inconnu}}. \
         Marqueurs disponibles : a.x, b.y"
    );
}

#[test]
fn le_gras_comme_le_regex_du_python() {
    let v = valeurs(&[("a.b", "X")]);
    let m = |t: &str, gras: bool| modele::morceaux(t, &v, gras).unwrap();
    assert_eq!(
        m("un **gras {{a.b}}** et **deux** fin", false),
        vec![
            ("un ".to_string(), false),
            ("gras X".to_string(), true),
            (" et ".to_string(), false),
            ("deux".to_string(), true),
            (" fin".to_string(), false)
        ]
    );
    // Tout en gras : les morceaux simples le deviennent aussi.
    assert_eq!(m("a **b** c", true), vec![("a ".into(), true), ("b".into(), true), (" c".into(), true)]);
    // Un ** sans fermeture reste du texte ; `.+?` veut au moins un caractère ; paresseux.
    assert_eq!(m("a ** b", false), vec![("a ** b".to_string(), false)]);
    assert_eq!(m("a **** b", false), vec![("a **** b".to_string(), false)]);
    assert_eq!(m("***x** y", false), vec![("*x".to_string(), true), (" y".to_string(), false)]);
    // Les blancs se réduisent comme en HTML, sauf l'espace insécable ; les bords sont rognés.
    assert_eq!(m("  un \n\t deux\u{a0}\u{a0}trois  ", false), vec![("un deux\u{a0}\u{a0}trois".to_string(), false)]);
    assert!(m("", false).is_empty());
}

#[test]
fn le_texte_brut_n_a_ni_gras_ni_marqueur() {
    let v = vals();
    assert_eq!(modele::texte_brut("Pour **{{salarie.nom}}** !", &v).unwrap(), "Pour DUPONT !");
}

// ------------------------------------------------------ le document « modèle »

fn description_modele(chemin: &str) -> Value {
    json!({
        "format": 2, "style": "contrat",
        "documents": [{"modele": {"chemin": chemin, "valeurs": vals(),
                                  "remplace": {"numero": "X-42", "date": "01/01/2025"}}}]
    })
}

fn para_json(morceaux: Value, taille: Option<f64>, couleur: Option<&str>, align: &str, avant: f64, apres: f64) -> Value {
    let mut b = json!({"type": "paragraphe", "morceaux": morceaux});
    if let Some(t) = taille {
        b["taille"] = json!(t);
    }
    if let Some(c) = couleur {
        b["couleur"] = json!(c);
    }
    if align != "gauche" {
        b["alignement"] = json!(align);
    }
    if avant != 0.0 {
        b["espace_avant"] = json!(avant);
    }
    if apres != 0.0 {
        b["espace_apres"] = json!(apres);
    }
    b
}

/// Le même document, écrit à la main en blocs.
fn description_a_blocs() -> Value {
    let g = |t: &str| json!({"texte": t, "gras": true});
    let n = |t: &str| json!({"texte": t});
    let rouge = Some("#B22222");
    let case = |libelle: &str, nom: &str, sous: &str| {
        vec![
            para_json(json!([g(libelle)]), Some(6.6), rouge, "gauche", 0.0, 0.0),
            para_json(json!([n(nom)]), None, None, "gauche", 0.0, 0.0),
            para_json(json!([n(sous)]), Some(7.0), Some("#777777"), "gauche", 0.0, 0.0),
        ]
    };
    let mut employeur = case("L'employeur", "Jeanne Martin", "Présidente");
    employeur.push(json!({"type": "espace", "hauteur": 3.0 * 7.5 * 1.25}));
    let mut artiste = case("L'artiste", "Paul Dupont", "signature");
    artiste.push(json!({"type": "espace", "hauteur": 3.0 * 7.5 * 1.25}));
    let blocs = vec![
        para_json(json!([g("Contrat n° C-1")]), Some(11.0), rouge, "gauche", 10.0, 1.0),
        para_json(json!([n("Pour "), g("DUPONT")]), Some(6.8), Some("#555555"), "justifie", 0.0, 7.0),
        para_json(json!([g("Annule le X-42 du 01/01/2025.")]), None, None, "justifie", 0.0, 6.0),
        para_json(json!([g("Entre"), n(" nous, et vous.")]), None, None, "justifie", 0.0, 2.6),
        para_json(json!([n("Deuxième paragraphe.")]), None, None, "justifie", 0.0, 2.6),
        para_json(json!([g("Article 1 \u{2014} Objet")]), Some(7.7), rouge, "gauche", 4.5, 1.0),
        para_json(json!([n("L'artiste DUPONT joue. Suite.")]), None, None, "justifie", 0.0, 2.6),
        para_json(json!([g("Article 2 \u{2014} Durée")]), Some(7.7), rouge, "gauche", 4.5, 1.0),
        para_json(json!([n("Du 01/01/2025.")]), None, None, "justifie", 0.0, 2.6),
        para_json(json!([n("Fait à Paris.")]), None, None, "justifie", 6.0, 2.6),
        json!({"type": "colonnes", "filet_haut": {"couleur": "#CCCCCC", "epaisseur": 0.5},
               "espace_avant": 4.0, "marge_interne_haut": 5.0,
               "colonnes": [{"largeur": 0.5, "blocs": employeur}, {"largeur": 0.5, "blocs": artiste}]}),
    ];
    json!({
        "format": 1,
        "page": {"largeur": 595.0, "hauteur": 842.0,
                 "marges": {"haut": 106.0, "droite": 59.5, "bas": 46.0, "gauche": 59.5}},
        "police": {"taille": 7.5, "interligne": 1.25, "couleur": "#2C3E50"},
        "titre": "Contrat n° C-1",
        "documents": [{"blocs": blocs}]
    })
}

#[test]
fn un_document_modele_donne_le_pdf_des_blocs_equivalents() {
    let chemin = ecrire_modele("petit.modele", PETIT);
    let a = fabriquer(&description_modele(&chemin)).unwrap();
    let b = fabriquer(&description_a_blocs()).unwrap();
    assert!(a == b, "le document modèle et ses blocs équivalents doivent donner le même PDF");
}

#[test]
fn sans_phrase_annule_le_defaut_du_contrat_sert() {
    let sans = PETIT.replace("ANNULE:\nAnnule le {{remplace.numero}} du {{remplace.date}}.\n", "");
    let chemin = ecrire_modele("sans-annule.modele", &sans);
    let mut d = description_modele(&chemin);
    let a = fabriquer(&d).unwrap();
    // La phrase par défaut est plus longue : le PDF diffère de celui d'un contrat qui ne remplace rien.
    d["documents"][0]["modele"].as_object_mut().unwrap().remove("remplace");
    assert_ne!(a, fabriquer(&d).unwrap());
}

#[test]
fn le_vrai_contrat_cddu_passe_avec_tous_ses_marqueurs() {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees/modeles/contrat-cddu.modele");
    let texte = std::fs::read_to_string(&chemin).unwrap();
    let m = modele::lire_modele(&texte, "contrat-cddu.modele").unwrap();
    assert_eq!(m.articles.len(), 12);
    // Chaque marqueur qui n'est pas dans un commentaire reçoit une valeur.
    let mut v = Valeurs::new();
    for ligne in texte.lines().filter(|l| !l.trim_start().starts_with('#')) {
        for morceau in ligne.split("{{").skip(1) {
            if let Some((cle, _)) = morceau.split_once("}}") {
                v.insert(cle.trim().to_string(), "valeur".to_string());
            }
        }
    }
    for k in ["employeur.signataire", "employeur.signataire_fonction", "salarie.nom_complet"] {
        v.insert(k.to_string(), "valeur".to_string());
    }
    let d = json!({"format": 2, "style": "contrat",
        "documents": [{"modele": {"chemin": chemin.to_str().unwrap(), "valeurs": v}},
                      {"modele": {"chemin": chemin.to_str().unwrap(), "valeurs": v}}]});
    let pdf = fabriquer(&d).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // Un marqueur retiré des valeurs, et le contrat n'est pas produit.
    v.remove("employeur.siret");
    let d = json!({"format": 2, "style": "contrat",
        "documents": [{"modele": {"chemin": chemin.to_str().unwrap(), "valeurs": v}}]});
    let e = refus(&d);
    assert!(e.starts_with("marqueur inconnu dans le modèle de contrat : {{employeur.siret}}"), "{}", e);
}

// ------------------------------------------------------------------- les refus

#[test]
fn refus_du_document_modele() {
    let chemin = ecrire_modele("petit.modele", PETIT);
    let bon = description_modele(&chemin);

    let mut d = bon.clone();
    d["format"] = json!(1);
    assert!(refus(&d).contains("\"modele\" demande \"format\": 2"));

    let mut d = bon.clone();
    d.as_object_mut().unwrap().remove("style");
    assert!(refus(&d).contains("demande un \"style\""));

    let mut d = bon.clone();
    d["style"] = json!("facture");
    let e = refus(&d);
    assert!(
        e.contains("style inconnu \"facture\"") && e.contains("styles connus : contrat, avenant, attestation"),
        "{}",
        e
    );

    let mut d = bon.clone();
    d["documents"][0]["blocs"] = json!([{"type": "espace", "hauteur": 1}]);
    assert!(refus(&d).contains("s'excluent"));

    let mut d = bon.clone();
    d["documents"][0]["modele"]["inconnu"] = json!(1);
    assert!(refus(&d).contains("champ inconnu \"inconnu\""));

    let mut d = bon.clone();
    d["documents"][0]["modele"]["valeurs"]["salarie.nom"] = json!(12);
    assert!(refus(&d).contains("mauvais type"));

    let mut d = bon.clone();
    d["documents"][0]["modele"]["remplace"] = json!({"numero": "X"});
    assert!(refus(&d).contains("champ absent \"date\""));

    // Un fichier absent, un modèle sans article, une valeur de signature manquante.
    let d = description_modele("/rien/du/tout.modele");
    assert!(refus(&d).starts_with("modèle illisible : /rien/du/tout.modele"));
    let vide = ecrire_modele("vide.modele", "TITRE: T\n");
    assert_eq!(refus(&description_modele(&vide)), format!("{} ne contient aucun article", vide));
    let mut d = bon.clone();
    d["documents"][0]["modele"]["valeurs"].as_object_mut().unwrap().remove("salarie.nom_complet");
    assert!(refus(&d).contains("{{salarie.nom_complet}}"));
}

#[test]
fn un_marqueur_inconnu_ne_produit_aucun_pdf() {
    let chemin = ecrire_modele("faute.modele", &format!("{}ARTICLE: Faute\n{{{{contrat.inconnu}}}}\n", PETIT));
    let e = refus(&description_modele(&chemin));
    assert!(e.contains("{{contrat.inconnu}}") && e.contains("Marqueurs disponibles"), "{}", e);
}

// ------------------------------------------- l'avenant et les papiers de fin de contrat

const AVENANT: &str = "TITRE: Avenant n° {{avenant.numero}}\n\
SOUSTITRE: Contrat {{contrat.numero}}\n\
PREAMBULE:\nLes parties conviennent de ce qui suit.\n\
MODIFICATION:\nL'article {{modification.numero}} « {{modification.article}} » disait :\n\n\
« {{modification.avant}} »\n\nIl dit désormais :\n\n« {{modification.apres}} »\n\
ARTICLE: Autres clauses\nLe reste ne change pas.\n\
SIGNATURES:\nFait à Paris.\n";

fn vals_avenant() -> Valeurs {
    let mut v = vals();
    v.insert("avenant.numero".into(), "2".into());
    v
}

fn description_avenant(chemin: &str) -> Value {
    json!({
        "format": 2, "style": "avenant",
        "documents": [{"modele": {"chemin": chemin, "valeurs": vals_avenant(),
            "modifications": [
                {"numero": 4, "article": "Rémunération", "avant": ["Cachet de 130 €.", "Frais."], "apres": ["Cachet de 140 €.", "Frais."]},
                {"numero": 7, "article": "Lieu", "avant": ["Paris."], "apres": ["Lyon."]}]}}]
    })
}

/// L'avenant lu du modèle est, au bit près, celui que les blocs décrivent : un intitulé par article
/// modifié, l'ancien et le nouveau texte cités en italique et en retrait de 12 pt, les articles du
/// modèle numérotés à la suite, puis les deux cases.
#[test]
fn l_avenant_donne_le_pdf_des_blocs_equivalents() {
    let chemin = ecrire_modele("avenant.modele", AVENANT);
    let a = fabriquer(&description_avenant(&chemin)).unwrap();
    let g = |t: &str| json!({"texte": t, "gras": true});
    let n = |t: &str| json!({"texte": t});
    let rouge = Some("#B22222");
    let cite = |t: &str| {
        let mut b = para_json(json!([{"texte": t, "italique": true}]), None, None, "justifie", 0.0, 2.6);
        b["retrait"] = json!(12.0);
        b
    };
    let case = |libelle: &str, nom: &str, sous: &str| {
        vec![
            para_json(json!([g(libelle)]), Some(6.6), rouge, "gauche", 0.0, 0.0),
            para_json(json!([n(nom)]), None, None, "gauche", 0.0, 0.0),
            para_json(json!([n(sous)]), Some(7.0), Some("#777777"), "gauche", 0.0, 0.0),
            json!({"type": "espace", "hauteur": 3.0 * 7.5 * 1.25}),
        ]
    };
    let j = |t: &str| para_json(json!([n(t)]), None, None, "justifie", 0.0, 2.6);
    let h = |t: &str| para_json(json!([g(t)]), Some(7.7), rouge, "gauche", 4.5, 1.0);
    let blocs = vec![
        para_json(json!([g("Avenant n° 2")]), Some(11.0), rouge, "gauche", 10.0, 1.0),
        para_json(json!([n("Contrat C-1")]), Some(6.8), Some("#555555"), "justifie", 0.0, 7.0),
        j("Les parties conviennent de ce qui suit."),
        h("Article 1 \u{2014} Modification de l'article 4 « Rémunération »"),
        j("L'article 4 « Rémunération » disait :"),
        cite("« Cachet de 130 €. »"),
        cite("« Frais. »"),
        j("Il dit désormais :"),
        cite("« Cachet de 140 €. »"),
        cite("« Frais. »"),
        h("Article 2 \u{2014} Modification de l'article 7 « Lieu »"),
        j("L'article 7 « Lieu » disait :"),
        cite("« Paris. »"),
        j("Il dit désormais :"),
        cite("« Lyon. »"),
        h("Article 3 \u{2014} Autres clauses"),
        j("Le reste ne change pas."),
        para_json(json!([n("Fait à Paris.")]), None, None, "justifie", 6.0, 2.6),
        json!({"type": "colonnes", "filet_haut": {"couleur": "#CCCCCC", "epaisseur": 0.5},
               "espace_avant": 4.0, "marge_interne_haut": 5.0,
               "colonnes": [{"largeur": 0.5, "blocs": case("L'employeur", "Jeanne Martin", "Présidente")},
                            {"largeur": 0.5, "blocs": case("L'artiste", "Paul Dupont", "signature")}]}),
    ];
    let b = fabriquer(&json!({
        "format": 2,
        "page": {"largeur": 595.0, "hauteur": 842.0,
                 "marges": {"haut": 106.0, "droite": 59.5, "bas": 46.0, "gauche": 59.5}},
        "police": {"taille": 7.5, "interligne": 1.25, "couleur": "#2C3E50"},
        "titre": "Avenant n° 2",
        "documents": [{"blocs": blocs}]
    }))
    .unwrap();
    assert!(a == b, "l'avenant modèle et ses blocs équivalents doivent donner le même PDF");
}

#[test]
fn le_vrai_avenant_passe_avec_tous_ses_marqueurs() {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees/modeles/avenant-cddu.modele");
    let texte = std::fs::read_to_string(&chemin).unwrap();
    let m = modele::lire_modele(&texte, "avenant-cddu.modele").unwrap();
    assert!(!m.modification.is_empty());
    let mut v = Valeurs::new();
    for ligne in texte.lines().filter(|l| !l.trim_start().starts_with('#')) {
        for morceau in ligne.split("{{").skip(1) {
            if let Some((cle, _)) = morceau.split_once("}}") {
                if !cle.trim().starts_with("modification.") {
                    v.insert(cle.trim().to_string(), "valeur".to_string());
                }
            }
        }
    }
    for k in ["employeur.signataire", "employeur.signataire_fonction", "salarie.nom_complet"] {
        v.insert(k.to_string(), "valeur".to_string());
    }
    let d = json!({"format": 2, "style": "avenant",
        "documents": [{"modele": {"chemin": chemin.to_str().unwrap(), "valeurs": v,
            "modifications": [{"numero": 4, "article": "Rémunération", "avant": ["a"], "apres": ["b"]}]}}]});
    assert!(fabriquer(&d).unwrap().starts_with(b"%PDF"));
}

#[test]
fn refus_de_l_avenant() {
    let chemin = ecrire_modele("avenant.modele", AVENANT);
    let bon = description_avenant(&chemin);

    let mut d = bon.clone();
    d["documents"][0]["modele"]["modifications"] = json!([]);
    assert!(refus(&d).contains("rien n'a changé"), "{}", refus(&d));

    let mut d = bon.clone();
    d["documents"][0]["modele"]["modifications"][0]["apres"] = json!(["un seul"]);
    assert!(refus(&d).contains("pas le même nombre de paragraphes"));

    let sans = ecrire_modele("avenant-sans.modele", &AVENANT.replace("MODIFICATION:", "# MODIFICATION:"));
    let e = refus(&description_avenant(&sans));
    assert_eq!(e, format!("{} n'a pas de bloc MODIFICATION:.", sans));

    // Un marqueur de modification hors du bloc MODIFICATION: n'existe pas.
    let hors = ecrire_modele("avenant-hors.modele", &format!("{}ARTICLE: Z\n{{{{modification.numero}}}}\n", AVENANT));
    let e = refus(&description_avenant(&hors));
    assert!(e.contains("{{modification.numero}}"), "{}", e);

    // Les clés d'un autre style.
    let mut d = bon.clone();
    d["documents"][0]["modele"]["remplace"] = json!({"numero": "X", "date": "Y"});
    assert!(refus(&d).contains("\"remplace\" ne va qu'avec le style \"contrat\""));
    let mut d = description_modele(&ecrire_modele("petit.modele", PETIT));
    d["documents"][0]["modele"]["modifications"] =
        json!([{"numero": 1, "article": "A", "avant": [], "apres": []}]);
    assert!(refus(&d).contains("\"modifications\" ne va qu'avec le style \"avenant\""));
}

const ATTESTATION: &str = "TITRE: Reçu\n\
PREAMBULE:\nJe soussigné **{{salarie.nom}}**.\n\
ARTICLE: Inventaire\n{{solde.inventaire}}\n\n**Net : {{solde.net}}.**\n\
SIGNATURES:\nFait à Paris.\n";

fn description_attestation(chemin: &str, inventaire: &str) -> Value {
    let mut v = vals();
    v.insert("solde.inventaire".into(), inventaire.into());
    v.insert("solde.net".into(), "2 649,06".into());
    json!({"format": 2, "style": "attestation",
           "documents": [{"modele": {"chemin": chemin, "valeurs": v}}]})
}

/// Une valeur de plusieurs lignes vaut plusieurs paragraphes, les intitulés n'ont pas de numéro et
/// il n'y a pas de cases de signature.
#[test]
fn l_attestation_coupe_les_valeurs_multilignes_sans_numeroter_ni_signer() {
    let chemin = ecrire_modele("attestation.modele", ATTESTATION);
    let a = fabriquer(&description_attestation(&chemin, "Salaire : 2 200,00\nPrime : 449,06")).unwrap();
    let n = |t: &str| json!({"texte": t});
    let g = |t: &str| json!({"texte": t, "gras": true});
    let j = |t: Value, avant: f64| para_json(t, None, None, "justifie", avant, 2.6);
    let blocs = vec![
        para_json(json!([g("Reçu")]), Some(11.0), Some("#B22222"), "gauche", 10.0, 1.0),
        j(json!([n("Je soussigné "), g("DUPONT"), n(".")]), 0.0),
        para_json(json!([g("Inventaire")]), Some(7.7), Some("#B22222"), "gauche", 4.5, 1.0),
        j(json!([n("Salaire : 2 200,00")]), 0.0),
        j(json!([n("Prime : 449,06")]), 0.0),
        j(json!([g("Net : 2 649,06.")]), 0.0),
        j(json!([n("Fait à Paris.")]), 6.0),
    ];
    let b = fabriquer(&json!({
        "format": 2,
        "page": {"largeur": 595.0, "hauteur": 842.0,
                 "marges": {"haut": 106.0, "droite": 59.5, "bas": 46.0, "gauche": 59.5}},
        "police": {"taille": 7.5, "interligne": 1.25, "couleur": "#2C3E50"},
        "titre": "Reçu",
        "documents": [{"blocs": blocs}]
    }))
    .unwrap();
    assert!(a == b, "l'attestation et ses blocs équivalents doivent donner le même PDF");
    // Une valeur substituée n'est pas relue : « {{ » et « ** » dans l'inventaire restent du texte.
    let c = fabriquer(&description_attestation(&chemin, "A {{b.c}} ** d")).unwrap();
    assert!(c.starts_with(b"%PDF"));
}

#[test]
fn refus_de_l_attestation() {
    let chemin = ecrire_modele("attestation.modele", ATTESTATION);
    let mut d = description_attestation(&chemin, "x");
    d["documents"][0]["modele"]["signature"] = json!("/une/image.svg");
    assert!(refus(&d).contains("pas de case de signature"));
    let mut d = description_attestation(&chemin, "x");
    d["documents"][0]["modele"]["valeurs"].as_object_mut().unwrap().remove("solde.net");
    assert!(refus(&d).contains("{{solde.net}}"));
}

/// Le certificat de travail s'établit signé : nom, fonction et image en fin de document, à droite ;
/// le reçu, qui ne demande rien, reste sans.
#[test]
fn l_attestation_pose_la_signature_de_l_employeur_en_fin_de_document() {
    let chemin = ecrire_modele("attestation-signee.modele", ATTESTATION);
    let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("exemples/signature.png");
    let sans = fabriquer(&description_attestation(&chemin, "x")).unwrap();
    let mut d = description_attestation(&chemin, "x");
    d["documents"][0]["modele"]["signature_employeur"] = json!(image.to_str().unwrap());
    let avec = fabriquer(&d).unwrap();
    assert!(avec.len() > sans.len(), "l'image de signature doit être dans le PDF");
    // Hors du style attestation, la clé est refusée.
    let mut d = description_modele(&ecrire_modele("petit.modele", PETIT));
    d["documents"][0]["modele"]["signature_employeur"] = json!(image.to_str().unwrap());
    assert!(refus(&d).contains("\"signature_employeur\" ne va qu'avec le style \"attestation\""));
}
