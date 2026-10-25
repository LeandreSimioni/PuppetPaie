//! Le garde-fou reseau des essais (`socle/src/garde_reseau.rs`) tient pour TOUTE sortie du moteur, et pour tout
//! `paie` qu'un essai lance : un essai ne joint jamais un serveur reel (celui de l'editeur, net-entreprises, l'Urssaf,
//! Google, la DILA, le depot des versions, une boite courriel). Cargo pose la variable (`.cargo/config.toml`, `[env]`) ;
//! ces essais verifient, dans les sources, que rien ne la contourne :
//! - chaque fichier du moteur qui ouvre une connexion vers dehors passe par `garde_reseau` (sauf ceux qui ne parlent
//!   qu'a la machine elle-meme, nommes ici avec leur raison) ;
//! - chaque essai qui lance un programme dans un environnement vide (`env_clear`) y repose la variable.
//!
//! La preuve par le comportement est dans `incidents_filet` (`sans_serveur_pose_rien_ne_sort_vers_le_serveur_de_l_editeur`)
//! et dans le moteur (`garde_reseau::tests`).

use std::path::{Path, PathBuf};

fn fichiers_rs(dossier: &Path, o: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dossier).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            fichiers_rs(&p, o);
        } else if p.extension().is_some_and(|x| x == "rs") {
            o.push(p);
        }
    }
}

/// L'espace `atelier/sources/` : le socle, le droit et le moteur.
fn espace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Ce qui ouvre une connexion (ou un navigateur) vers une adresse.
const SORTIES: [&str; 8] = ["ureq::Agent", "ureq::get(", "ureq::post(", "TcpStream::connect", "SmtpTransport::", "UdpSocket", "\"xdg-open\"", "rundll32"];

/// Les fichiers qui n'ouvrent de connexion que vers la machine elle-meme.
const LOCAUX: [(&str, &str); 2] = [
    ("paie/src/interface/instance.rs", "le verrou d'une installation : la fenetre deja ouverte, sur 127.0.0.1"),
    ("paie/src/google_autorisation.rs", "reveille son propre ecouteur local (127.0.0.1) ; l'echange de jetons passe par `transport`"),
];

#[test]
fn chaque_sortie_du_moteur_passe_par_le_garde_fou() {
    let racine = espace();
    let mut fichiers = Vec::new();
    for c in ["socle", "droit", "paie"] {
        fichiers_rs(&racine.join(c).join("src"), &mut fichiers);
    }
    let mut fautifs = Vec::new();
    for f in fichiers {
        let rel = f.strip_prefix(&racine).unwrap().to_string_lossy().replace('\\', "/");
        if rel == "socle/src/garde_reseau.rs" || LOCAUX.iter().any(|(l, _)| *l == rel) {
            continue;
        }
        let texte = std::fs::read_to_string(&f).unwrap();
        let ouvre = texte.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| SORTIES.iter().any(|s| l.contains(s)));
        if ouvre && !texte.contains("garde_reseau::") {
            fautifs.push(rel);
        }
    }
    assert!(fautifs.is_empty(), "ces fichiers ouvrent une connexion sans passer par `garde_reseau` : {:?}", fautifs);
    // Les fichiers locaux le restent : aucune autre sortie n'y est apparue.
    for (l, _) in LOCAUX {
        let texte = std::fs::read_to_string(racine.join(l)).unwrap();
        assert!(!texte.contains("ureq::") && !texte.contains("SmtpTransport"), "{} : une sortie HTTP ou SMTP, hors du garde-fou", l);
    }
}

#[test]
fn chaque_environnement_vide_d_un_essai_repose_le_garde_fou() {
    let racine = espace();
    let mut fichiers = Vec::new();
    for c in ["socle", "droit", "paie"] {
        if racine.join(c).join("tests").is_dir() {
            fichiers_rs(&racine.join(c).join("tests"), &mut fichiers);
        }
    }
    let mut fautifs = Vec::new();
    for f in fichiers {
        if f.ends_with("fichiers/garde_reseau.rs") {
            continue;
        }
        let texte = std::fs::read_to_string(&f).unwrap();
        let lignes: Vec<&str> = texte.lines().collect();
        for (i, l) in lignes.iter().enumerate() {
            if l.trim_start().starts_with("//") || !l.contains("env_clear()") {
                continue;
            }
            if !lignes[i..(i + 3).min(lignes.len())].iter().any(|x| x.contains("garde_reseau::VARIABLE")) {
                fautifs.push(format!("{}:{}", f.strip_prefix(&racine).unwrap().display(), i + 1));
            }
        }
    }
    assert!(fautifs.is_empty(), "un `env_clear` qui ne repose pas `garde_reseau::VARIABLE` juste apres : {:?}", fautifs);
}

/// Cargo pose la variable pour cet essai, et donc pour tout `paie` qu'il lance sans vider l'environnement.
#[test]
fn cargo_pose_le_garde_fou() {
    assert!(std::env::var_os(socle::garde_reseau::VARIABLE).is_some(), "`.cargo/config.toml` du moteur doit poser {}", socle::garde_reseau::VARIABLE);
}
