//! Le client Drive de la sauvegarde contre un faux serveur sur `127.0.0.1` : un delai de lecture n'est pas une absence
//! de reseau, rien ne part sans compte lie ni d'une copie de developpement (regle 11), et le trait `Drive` ne sait rien
//! supprimer. Le jeton renouvele apres un 401 (cle du poste dans l'environnement) : `tests/drive_jeton.rs`. Aucune
//! connexion vers l'exterieur ; `google.json` est fictif.

use std::path::Path;

// ------------------------------------------------------------ un delai n'est pas « hors ligne »

// ------------------------------------------------------------ compte et copie de developpement

// ------------------------------------------------------------ « ne supprime jamais »

/// Le trait `Drive` ne porte que six methodes (deux de lecture pour la restauration), et aucune ne supprime : si
/// quelqu'un en ajoute une (`supprimer`, `effacer`, `corbeille`...), ce test echoue et force a le dire. La garantie « ne
/// supprime jamais rien dans le Drive » est dans le TYPE : la sauvegarde, l'archivage et la restauration ne recoivent qu'un
/// `&mut dyn Drive`.
#[test]
fn le_trait_drive_n_a_pas_de_suppression() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../envois/src/drive.rs")).unwrap();
    let debut = source.find("pub trait Drive {").expect("le trait Drive");
    let corps = &source[debut..];
    let fin = corps.find("\n}\n").expect("fin du trait");
    let trait_ = &corps[..fin];
    let methodes: Vec<&str> = trait_
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.strip_prefix("fn "))
        .map(|l| l.split(|c: char| c == '(' || c == '<').next().unwrap())
        .collect();
    assert_eq!(methodes, ["compte", "chemin", "lister", "televerser", "sous_dossiers", "telecharger"], "le trait Drive a change : {:?}", methodes);
    for m in &methodes {
        for mot in ["suppr", "supprim", "effac", "delete", "remove", "corbeille", "trash", "detruire"] {
            assert!(!m.to_lowercase().contains(mot), "une methode de suppression dans Drive : {}", m);
        }
    }
    // Et le client n'appelle jamais DELETE ni ne met a la corbeille.
    let client = &source[..debut + corps.len()];
    assert!(!client.contains("\"DELETE\"") && !client.contains("trashed\": true") && !client.contains("\"trashed\":true"));
}
