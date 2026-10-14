//! `journal-reprise` : la reprise du journal des depots depuis les retours archives. Sans `ecrire`, elle dit ce qu'elle
//! ferait et le journal ne bouge pas ; une piece illisible est nommee, les autres passent. Sur `demo-3090-petite` entiere.

use crate::outils::{compagnie, dossier_de_test, reussi, route};
use serde_json::json;
use std::path::PathBuf;

fn compagnie(nom: &str) -> PathBuf {
    compagnie::entiere("demo-3090-petite", &dossier_de_test("routes_reprise", nom).join("compagnie"))
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
