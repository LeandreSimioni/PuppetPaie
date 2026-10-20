//! `journal-reprise` : la reprise du journal des depots depuis les retours archives. Sans `ecrire`, elle dit ce qu'elle
//! ferait et le journal ne bouge pas ; une piece illisible est nommee, les autres passent. Sur `fictive-3090-petite` entiere.

use crate::outils::{compagnie, dossier_de_test, reussi, route};
use serde_json::json;
use std::path::PathBuf;

fn compagnie(nom: &str) -> PathBuf {
    compagnie::entiere("fictive-3090-petite", &dossier_de_test("routes_reprise", nom).join("compagnie"))
}

#[test]
fn la_reprise_du_journal_sans_ecrire_ne_touche_pas_au_journal() {
    let racine = compagnie("apercu");
    let avant = std::fs::read(racine.join("journal/depots.json")).unwrap();
    let r = route(&racine, "journal-reprise", json!({}), "2026-10-03");
    assert_eq!(reussi(&r)["ecrit"], json!(false), "{}", r);
    assert_eq!(std::fs::read(racine.join("journal/depots.json")).unwrap(), avant);
}

/// Une piece de `retours/` dont le XML ne se lit pas est NOMMEE dans `illisibles`, et les autres passent ; un XML bien
/// forme qui n'est pas un bilan de depot reste saute sans bruit.
#[test]
fn la_reprise_du_journal_nomme_la_piece_illisible() {
    let racine = compagnie("illisible");
    let retours = racine.join("retours");
    std::fs::write(retours.join("abime.xml"), b"<rapport type=\"10\"><idflux>WS1</idfl").unwrap();
    std::fs::write(retours.join("autre.xml"), b"<?xml version=\"1.0\"?><autre>rien</autre>").unwrap();
    let r = reussi(&route(&racine, "journal-reprise", json!({}), "2026-10-03")).clone();
    let illisibles = r["illisibles"].as_array().unwrap();
    assert_eq!(illisibles.len(), 1, "{}", r);
    assert!(illisibles[0].as_str().unwrap().starts_with("abime.xml"), "{}", r);
}

/// Un bilan de mensuelle comme la plate-forme le rend, pour le SIREN de `fictive-3090-petite`.
fn bilan(flux: &str, essai_reel: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gipmds:rapport xmlns:gipmds=\"http://www.gip-mds.fr/\" profil=\"DSN\" type=\"CCO\" version=\"v01r08\">\
<envoi><envoi_identification><idflux>{f}</idflux><date_reception>2026-09-27</date_reception><heure_reception>19:38:00</heure_reception>\
<nom_logiciel>ATELIER</nom_logiciel><essai_reel>{e}</essai_reel></envoi_identification><envoi_bilan><envoi_etat>OK</envoi_etat></envoi_bilan></envoi>\
<declaration><declaration_identification><nature>01</nature><type>01</type><fraction>11</fraction><ordre>4</ordre><mois>01102026</mois>\
<SIREN>123456923</SIREN></declaration_identification><declaration_bilan><etat>OK</etat></declaration_bilan></declaration></gipmds:rapport>",
        f = flux, e = essai_reel
    )
}

/// Un TEST absent du journal vient de l'atelier (meme compte net-entreprises : ligne de commande, `paie --dev`, seances
/// du nuage) : il ne s'affiche pas et ne se reprend pas. Un REEL absent du journal est un depot de la compagnie (l'atelier
/// n'en fait jamais) : il se reprend (Leandre, 20/10/2026 ; tickets #221, #222 : « 19 depots retrouves dans les retours,
/// absents du journal », dont un test du 27/09 « octobre 2026 »).
#[test]
fn un_test_absent_du_journal_ne_se_montre_pas_un_reel_absent_se_reprend() {
    let racine = compagnie("atelier");
    let retours = racine.join("retours");
    std::fs::write(retours.join("20260927-193800_nature11_FLUX-ESSAI-ATELIER.xml"), bilan("FLUX-ESSAI-ATELIER", "01")).unwrap();
    std::fs::write(retours.join("20260927-193900_nature11_FLUX-REEL-AILLEURS.xml"), bilan("FLUX-REEL-AILLEURS", "02")).unwrap();
    let r = reussi(&route(&racine, "journal-reprise", json!({}), "2026-10-03")).clone();
    assert!(!r.to_string().contains("FLUX-ESSAI-ATELIER"), "{}", r);
    let ajoutes: Vec<&serde_json::Value> = r["ajoutes"].as_array().unwrap().iter().filter(|v| v["flux"] == json!("FLUX-REEL-AILLEURS")).collect();
    assert_eq!(ajoutes.len(), 1, "{}", r);
    assert_eq!(ajoutes[0]["mode"], json!("reel"));
    reussi(&route(&racine, "journal-reprise", json!({"ecrire": true}), "2026-10-03"));
    let j = std::fs::read_to_string(racine.join("journal/depots.json")).unwrap();
    assert!(j.contains("FLUX-REEL-AILLEURS") && !j.contains("FLUX-ESSAI-ATELIER"));
}
