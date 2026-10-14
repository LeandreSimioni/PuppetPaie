//! La contresignature par régénération : le modèle donné en texte (`modele.texte`) et la mention de
//! l'artiste (`modele.mention_artiste`). Le contrat signé se refait depuis la description gardée à
//! l'envoi : sans mention c'est le même PDF au bit près, avec elle seule la case de l'artiste change.

use lopdf::content::Content;
use lopdf::{Document, Object};
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
    fabrique_pdf::fabriquer(&v.to_string()).map_err(|r| {
        assert!(!r.0.contains('\n'), "une seule ligne : {:?}", r.0);
        r.0
    })
}

/// Le texte de la première page, chaînes des opérateurs Tj / TJ mises bout à bout, une par ligne.
fn texte(octets: &[u8]) -> Vec<String> {
    let doc = Document::load_mem(octets).unwrap();
    let id = *doc.get_pages().values().next().unwrap();
    let contenu = Content::decode(&doc.get_page_content(id).unwrap()).unwrap();
    let mut lignes = Vec::new();
    for op in contenu.operations {
        if op.operator == "Tj" || op.operator == "TJ" {
            let mut t = String::new();
            for o in &op.operands {
                match o {
                    Object::String(s, _) => t.extend(caracteres(s)),
                    Object::Array(a) => {
                        for x in a {
                            if let Object::String(s, _) = x {
                                t.extend(caracteres(s));
                            }
                        }
                    }
                    _ => {}
                }
            }
            lignes.push(t);
        }
    }
    lignes
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

#[test]
fn la_mention_ne_change_que_la_case_de_l_artiste() {
    let sans = fabriquer(&description(json!({"texte": MODELE}))).unwrap();
    let avec = fabriquer(&description(json!({"texte": MODELE, "mention_artiste": MENTION}))).unwrap();
    assert_ne!(sans, avec);
    let (t_sans, t_avec) = (texte(&sans), texte(&avec));
    for l in MENTION.lines() {
        assert!(t_avec.iter().any(|t| t == l), "la ligne {:?} manque : {:?}", l, t_avec);
    }
    // Retirez les lignes de la mention : c'est le texte du PDF sans mention, dans le même ordre.
    let reste: Vec<String> = t_avec.into_iter().filter(|t| !MENTION.lines().any(|l| l == t)).collect();
    assert_eq!(reste, t_sans);
    // Rien n'a bougé ailleurs : même nombre de pages.
    assert_eq!(Document::load_mem(&sans).unwrap().get_pages().len(), Document::load_mem(&avec).unwrap().get_pages().len());
}

#[test]
fn la_mention_est_refusee_quand_elle_ne_tient_pas() {
    let trop_de_lignes = "a\nb\nc\nd\ne\nf";
    let e = fabriquer(&description(json!({"texte": MODELE, "mention_artiste": trop_de_lignes}))).unwrap_err();
    assert!(e.contains("la mention de signature ne tient pas dans la case"), "{}", e);
    let longue = "x".repeat(200);
    let e = fabriquer(&description(json!({"texte": MODELE, "mention_artiste": longue}))).unwrap_err();
    assert!(e.contains("trop longue"), "{}", e);
    // Un caractère que la police elle-même n'a pas (un idéogramme) : refus d'une ligne.
    let e = fabriquer(&description(json!({"texte": MODELE, "mention_artiste": "Signé 日本"}))).unwrap_err();
    assert!(e.contains("« 日 »") && e.contains("Liberation"), "{}", e);
}

#[test]
fn la_mention_ne_va_pas_avec_l_attestation() {
    let mut d = description(json!({"texte": MODELE, "mention_artiste": MENTION}));
    d["style"] = json!("attestation");
    let e = fabriquer(&d).unwrap_err();
    assert!(e.contains("n'a pas de case de l'artiste"), "{}", e);
}

/// Le texte d'une chaîne du PDF : deux octets par caractère (le point de code Unicode), la police
/// étant embarquée en sous-ensemble.
fn caracteres(octets: &[u8]) -> Vec<char> {
    octets
        .chunks(2)
        .map(|c| char::from_u32(((c[0] as u32) << 8) | c[1] as u32).unwrap())
        .collect()
}
