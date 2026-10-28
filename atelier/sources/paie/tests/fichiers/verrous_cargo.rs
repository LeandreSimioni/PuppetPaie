//! Les trois verrous Cargo de l'atelier (celui de l'espace `Cargo.lock`, `fabrique-pdf/Cargo.lock`, `fenetre/Cargo.lock`) disent la
//! meme chose des paquets de l'atelier. « Publier » (`.github/workflows/publier.yml`) compile la fenetre avec `--locked` :
//! une dependance ajoutee au moteur sans que le verrou de la fenetre la connaisse fait echouer la publication, et rien
//! d'autre ne le dit avant (le 15/10/2026 au soir, `icu_normalizer` ajoute a `paie` : Windows et Mac refuses). Pour
//! remettre la fenetre a jour sans toucher aux autres versions epinglees : `cargo update --workspace` dans `fenetre/`.
//!
//! On ne lit pas le TOML entier : seulement, dans chaque verrou, le bloc `[[package]]` du paquet et sa liste
//! `dependencies` (les noms, sans la version qui les distingue quand deux versions cohabitent).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn sources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Les noms des dependances du paquet `nom` dans le verrou `verrou`, ou `None` s'il n'y figure pas.
fn dependances(verrou: &Path, nom: &str) -> Option<BTreeSet<String>> {
    let texte = std::fs::read_to_string(verrou).unwrap_or_else(|e| panic!("{} : {}", verrou.display(), e));
    let mut dans_le_bloc = false;
    let mut dans_la_liste = false;
    let mut deps = BTreeSet::new();
    for ligne in texte.lines() {
        let l = ligne.trim();
        if l == "[[package]]" {
            if dans_le_bloc {
                return Some(deps);
            }
            continue;
        }
        if l == format!("name = \"{}\"", nom) {
            dans_le_bloc = true;
            continue;
        }
        if !dans_le_bloc {
            continue;
        }
        if l.starts_with("dependencies = [") {
            dans_la_liste = !l.ends_with(']');
            continue;
        }
        if dans_la_liste {
            if l == "]" {
                dans_la_liste = false;
            } else {
                let d = l.trim_matches(|c| c == '"' || c == ',');
                deps.insert(d.split(' ').next().unwrap().to_string());
            }
        }
    }
    dans_le_bloc.then_some(deps)
}

#[test]
fn le_verrou_de_la_fenetre_connait_les_dependances_du_moteur_et_de_la_fabrique() {
    let fenetre = sources().join("fenetre/Cargo.lock");
    for (paquet, verrou) in [("socle", "Cargo.lock"), ("droit", "Cargo.lock"), ("engagement", "Cargo.lock"), ("documents", "Cargo.lock"), ("paie", "Cargo.lock"), ("fabrique-pdf", "fabrique-pdf/Cargo.lock")] {
        let sien = sources().join(verrou);
        let attendu = dependances(&sien, paquet).unwrap_or_else(|| panic!("{} absent de son propre verrou", paquet));
        let vu = dependances(&fenetre, paquet).unwrap_or_else(|| panic!("{} absent du verrou de la fenetre", paquet));
        let manquantes: Vec<_> = attendu.difference(&vu).collect();
        let en_trop: Vec<_> = vu.difference(&attendu).collect();
        assert!(
            manquantes.is_empty() && en_trop.is_empty(),
            "fenetre/Cargo.lock ne dit pas la meme chose que {} sur `{}` (manque {:?}, en trop {:?}) : \
             `cargo update --workspace` dans atelier/sources/fenetre/, sinon « Publier » refuse (--locked)",
            verrou, paquet, manquantes, en_trop
        );
    }
}

#[test]
fn un_verrou_en_retard_se_voit() {
    // Le verrou de la fenetre tel qu'il etait le 15/10/2026 au soir : `paie` sans `icu_normalizer`.
    let d = crate::outils::dossier_de_test("verrous_cargo", "en_retard");
    let texte = std::fs::read_to_string(sources().join("fenetre/Cargo.lock")).unwrap();
    // L'occurrence a retirer est celle du bloc de `paie` (idna, plus haut, en a une aussi).
    let debut = texte.find("name = \"paie\"\n").expect("le bloc de paie");
    let (avant, bloc) = texte.split_at(debut);
    let en_retard = format!("{}{}", avant, bloc.replacen(" \"icu_normalizer\",\n", "", 1));
    assert_ne!(texte, en_retard, "le verrou d'aujourd'hui connait icu_normalizer");
    let chemin = d.join("Cargo.lock");
    std::fs::write(&chemin, en_retard).unwrap();
    let attendu = dependances(&sources().join("Cargo.lock"), "paie").unwrap();
    let vu = dependances(&chemin, "paie").unwrap();
    assert_eq!(attendu.difference(&vu).collect::<Vec<_>>(), vec!["icu_normalizer"]);
    assert_eq!(dependances(&chemin, "paquet-inconnu"), None);
}
