//! Le contrat refait depuis sa description : le modèle donné en texte (`modele.texte`) donne le même PDF que son
//! fichier, au bit près ; l'exemplaire contresigné est ce PDF, plus la mention de l'artiste (`marquage::contresigner`).

use serde_json::{json, Value};

const MODELE: &str = "TITRE: Contrat n° {{contrat.numero}}\n\
PREAMBULE:\nEntre la compagnie et {{salarie.nom_complet}}.\n\
ARTICLE: Objet\nL'artiste joue.\n\
SIGNATURES:\nFait à Paris.\n";

const MENTION: &str = "Signé électroniquement le 02/10/2026 à 09:41\npar Paul Dupont\npaul@example.org\ncode de signature SIG-ABC123";

fn description(modele: Value) -> Value {
    let mut m = json!({"valeurs": {
        "contrat.numero": "C-1", "employeur.signataire": "Jeanne Martin",
        "employeur.signataire_fonction": "Présidente", "salarie.nom_complet": "Paul Dupont"}});
    for (k, v) in modele.as_object().unwrap() {
        m[k] = v.clone();
    }
    json!({"format": 2, "style": "contrat", "documents": [{"modele": m}]})
}

fn fabriquer(v: &Value) -> Result<Vec<u8>, String> {
    fabrique_pdf::Description::lire(&v.to_string()).and_then(|d| fabrique_pdf::fabriquer(&d)).map_err(|r| {
        assert!(!r.0.contains('\n'), "une seule ligne : {:?}", r.0);
        r.0
    })
}

#[test]
fn le_modele_en_texte_donne_le_meme_pdf_que_le_fichier() {
    let f = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("regeneration.modele");
    std::fs::write(&f, MODELE).unwrap();
    let par_fichier = fabriquer(&description(json!({"chemin": f.to_str().unwrap().replace('\\', "/")}))).unwrap();
    let par_texte = fabriquer(&description(json!({"texte": MODELE}))).unwrap();
    assert_eq!(par_fichier, par_texte);
    // Et c'est déterministe : le texte gardé refait le même PDF.
    assert_eq!(par_texte, fabriquer(&description(json!({"texte": MODELE}))).unwrap());
}

#[test]
fn le_chemin_et_le_texte_s_excluent_et_l_un_des_deux_est_requis() {
    let e = fabriquer(&description(json!({"chemin": "/x.modele", "texte": MODELE}))).unwrap_err();
    assert!(e.contains("\"chemin\" et \"texte\" s'excluent"), "{}", e);
    let e = fabriquer(&description(json!({}))).unwrap_err();
    assert!(e.contains("demande \"chemin\"") && e.contains("\"texte\""), "{}", e);
    // Un texte sans article est refusé comme un fichier, avec un nom qui dit d'où il vient.
    let e = fabriquer(&description(json!({"texte": "TITRE: T\n"}))).unwrap_err();
    assert_eq!(e, "le modèle donné en texte ne contient aucun article");
}

/// L'exemplaire contresigné est le contrat parti, plus la mention : ses octets commencent par ceux du PDF
/// parti (une mise à jour incrémentale, ISO 32000-2, 7.5.6), et sans ce qui les suit c'est le contrat parti.
#[test]
fn l_exemplaire_contresigne_est_le_contrat_parti_plus_la_mention() {
    let parti = fabriquer(&description(json!({"texte": MODELE}))).unwrap();
    let signe = fabrique_pdf::marquage::contresigner(&parti, MENTION).unwrap();
    assert_eq!(&signe[..parti.len()], &parti[..]);
    let doc = fabrique_pdf::lecture::charger(&signe).unwrap();
    assert!(doc.trailer.get(b"Encrypt").is_err());
    let (textes, _) = fabrique_pdf::texte::texte_des_pages(&doc);
    for l in MENTION.lines() {
        assert!(textes[0].lines().any(|t| t == l), "la ligne {:?} manque : {:?}", l, textes[0]);
    }
    // Une mention qui ne tient pas dans la case, ou un PDF sans case de l'artiste : refus d'une ligne.
    let e = fabrique_pdf::marquage::contresigner(&parti, "a\nb\nc\nd\ne\nf").unwrap_err().0;
    assert!(e.contains("ne tient pas dans la case") && !e.contains('\n'), "{}", e);
    let mut attestation = description(json!({"texte": MODELE}));
    attestation["style"] = json!("attestation");
    let sans_case = fabriquer(&attestation).unwrap();
    assert!(fabrique_pdf::marquage::contresigner(&sans_case, MENTION).unwrap_err().0.contains("case de l'artiste"));
}
