//! Le verrou des données réelles (règle de fond 9a), sur ce que Git emporterait hors de `prive/`, en texte : la liste
//! du garde avant commit (`verifier_avant_commit::ce_qui_part`, suivis et non suivis, les `.gitignore` appliqués par Git
//! lui-même), lue une fois pour deux verrous. Nulle part un NIR qui ne soit pas inventé (`nirs_non_inventes`, sauf
//! l'exemple qu'un texte de référence porte de son émetteur : `EXEMPLES_DES_TEXTES`) ; dans le code et ce que reçoit une
//! compagnie (`DU_CODE`), pas le nom de la compagnie du poste (règle 0 : le logiciel est pour toute compagnie), hors de
//! la mention de l'auteur (`SIGNATURE`). Et aucun test ne construit un chemin vers `prive/`. Il ne voit ni les adresses
//! ni les noms de personnes : les écrire ici pour les chercher, ce serait les publier.
//!
//! Un NIR inventé porte `999`, `123456` ou `000` dans ses treize chiffres (une commune ou un rang que personne n'a) ; un
//! numéro qui ne suit pas la convention est tenu pour réel, et refusé. L'échec dit le fichier et la ligne, jamais le
//! numéro : la sortie des tests se lit dans les journaux publics des flux.

use std::path::{Path, PathBuf};

use regex::Regex;

use moteur::atelier::verifier_avant_commit::{ce_qui_part, nirs_non_inventes, EXEMPLES_DES_TEXTES};

/// En deux morceaux, pour que ce fichier ne porte pas lui-même ce qu'il refuse.
const NOM_REEL: &str = concat!("ueu", "eue");

/// Le code et ce que reçoit une compagnie, relatifs à la racine du dépôt : là où un cas propre à la compagnie du poste
/// n'a pas sa place.
const DU_CODE: [&str; 10] = [
    "logiciel/",
    "fictives/",
    "atelier/fictives/",
    "atelier/sources/",
    "atelier/outils/",
    "atelier/installeur/",
    ".claude/",
    ".github/",
    "PuppetPaie.cmd",
    "atelier/PuppetPaie.",
];

/// LA SIGNATURE DE L'EDITEUR : l'editeur et la compagnie du poste portent le meme nom, et le logiciel le signe en un seul
/// fichier (la mention de l'auteur et ses sites). Le verrou du nom le laisse passer, lui seul.
const SIGNATURE: &str = "atelier/sources/moteur/src/interface/auteur.rs";

fn depot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn part() -> Vec<String> {
    ce_qui_part(&depot(), false).unwrap_or_else(|m| panic!("ce verrou lit ce que Git emporterait : {}", m))
}

fn ligne(o: &[u8], i: usize) -> usize {
    o[..i].iter().filter(|&&b| b == b'\n').count() + 1
}

#[test]
fn rien_de_la_compagnie_reelle_n_est_ecrit_dans_ce_qui_part() {
    let (mut nirs, mut noms) = (Vec::new(), Vec::new());
    for rel in part() {
        let Ok(o) = std::fs::read(depot().join(&rel)) else { continue };
        if !EXEMPLES_DES_TEXTES.iter().any(|(f, _)| *f == rel) {
            nirs.extend(nirs_non_inventes(&o).into_iter().map(|(i, _)| format!("{}:{}", rel, ligne(&o, i))));
        }
        if rel != SIGNATURE && DU_CODE.iter().any(|d| rel.starts_with(d)) {
            if rel.to_lowercase().contains(NOM_REEL) {
                noms.push(rel.clone());
            }
            let position = o.to_ascii_lowercase().windows(NOM_REEL.len()).position(|w| w == NOM_REEL.as_bytes());
            noms.extend(position.map(|i| format!("{}:{}", rel, ligne(&o, i))));
        }
    }
    let mut echec = String::new();
    if !nirs.is_empty() {
        echec += &format!("NIR sans la marque d'un numéro inventé (999, 123456 ou 000) : {:#?}\n", nirs);
    }
    if !noms.is_empty() {
        echec += &format!("le logiciel est pour toute compagnie, il ne porte pas le nom de son banc d'essai : {:#?}\n", noms);
    }
    assert!(echec.is_empty(), "{}", echec);
}

/// Un chemin vers les données du poste construit depuis le code : le parent d'un chemin suivi de `prive`, ou un relatif
/// qui remonte. Un dossier de test qui s'appelle `prive` (`monde().join("prive")`) n'est pas visé.
#[test]
fn aucun_test_ne_construit_un_chemin_vers_prive() {
    let re = Regex::new(r#"(?:parent\(\)|"\.\.")[^;\n]*?\.join\(\s*"prive"|\.\./prive"#).unwrap();
    let mut trouves = Vec::new();
    for rel in part() {
        let dans_les_tests = rel.strip_prefix("atelier/sources/").is_some_and(|r| r.split('/').any(|c| c == "tests"));
        if !dans_les_tests || !rel.ends_with(".rs") {
            continue;
        }
        let t = std::fs::read_to_string(depot().join(&rel)).unwrap();
        trouves.extend(re.find_iter(&t).map(|m| format!("{}:{}", rel, ligne(t.as_bytes(), m.start()))));
    }
    assert!(trouves.is_empty(), "{:#?}", trouves);
}
