//! Le verrou des données réelles (règle de fond 9a) : ce qui sera public — `logiciel/`, `fictives/`, `atelier/sources/`, et les
//! textes de référence d'`atelier/reference/` — n'écrit pas un NIR qui ne soit pas inventé (sauf, dans un texte de référence,
//! l'exemple que son émetteur y a imprimé : `EXEMPLES_DES_TEXTES`) ; aucun test ne construit un chemin vers `prive/` ; la
//! suite ne prend pas `prive/` pour racine. Tout ce qui part du dépôt hors de `prive/` (`atelier/` entier, `.claude/`, la racine : le dépôt public part de
//! là, `atelier/PLAN-DEPOT-PUBLIC.md`, « La forme du dépôt public ») ne porte ni le SIREN ni le nom de la compagnie du poste,
//! ni un chemin du poste (un chemin absolu sous le dossier des utilisateurs de Windows), sauf ce qui attend une décision
//! (`EN_SUSPENS`). Il ne voit ni les adresses ni les noms de personnes : les écrire ici pour les chercher, ce serait les
//! publier.
//!
//! Un NIR inventé porte `999`, `123456` ou `000` dans ses treize chiffres (une commune ou un rang que personne n'a) ; un
//! numéro qui ne suit pas la convention est tenu pour réel, et refusé.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

// La marque d'un NIR inventé (`INVENTE`) est celle du garde avant commit (`paie verifier-avant-commit`), qui la tient.
use paie::verifier_avant_commit::INVENTE;

/// En deux morceaux, pour que ce fichier ne porte pas lui-même ce qu'il refuse.
const SIREN_REEL: &str = concat!("8783", "66194");
const NOM_REEL: &str = concat!("ueu", "eue");
/// Ce qui ne s'écrit pas à la main : ce que construit cargo (ses empreintes ressemblent à des NIR), les caches.
const HORS_CODE: [&str; 4] = ["target", ".git", "__pycache__", ".pytest_cache"];
const PUBLIC: [&str; 4] = ["logiciel", "fictives", "atelier/sources", "atelier/reference"];

/// Les textes de référence (`atelier/reference/`) sont des copies, octet pour octet, de ce que d'autres ont publié : un numéro
/// à la forme d'un NIR qui y figure est celui de l'émetteur -- l'exemple d'une documentation officielle, ou un nombre qui
/// n'est pas un NIR --, et le retirer, ce serait changer la copie (son empreinte est vérifiée, `paie sources`). Chacun est
/// nommé ici, fichier par fichier (relatif à la racine du dépôt), avec ce qu'il est ; un motif qui ne couvre plus rien se
/// retire (`chaque_exemple_des_textes_couvre_encore_un_numero`). Ce qui n'est pas listé est refusé, nos notes comprises.
const EXEMPLES_DES_TEXTES: [(&str, &str); 6] = [
    (
        r"^atelier/reference/sources/gip-mds-api-dsn/ctip-dsn-crm-oc-phases-2-3-20160601\.txt$",
        "les salariés d'exemple des comptes rendus métier OC imprimés par le CTIP (présentation des phases 2 et 3, juin 2016)",
    ),
    (
        r"^atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6\.8-20260706\.txt$",
        "les exemples de flux et de réponses du guide d'implémentation de l'API DSN (GIP-MDS)",
    ),
    (
        r"^atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2955-mutation-transfert-contrat-20250818\.(html|txt)$",
        "le salarié d'exemple de la fiche net-entreprises 2955",
    ),
    (
        r"^atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2956-bloc-41-changement-modalites-contrat-20250429\.(html|txt)$",
        "le salarié d'exemple de la fiche net-entreprises 2956",
    ),
    (
        r"^atelier/reference/sources/urssaf-liste-des-sites-et-des-activites\.csv$",
        "une longitude de la table de l'Urssaf (des décimales de coordonnée), pas un NIR",
    ),
    (
        r"^atelier/reference/sources/python/python-cpython-3\.11\.17-pyexpat-20261015\.txt$",
        "une empreinte hexadécimale d'Argument Clinic dans la source de CPython (`[clinic end generated code: … input=…]`), pas un NIR",
    ),
];

/// Ce qu'un verrou cherche hors de `prive/`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Verrou {
    Nom,
    Siren,
    /// Un chemin absolu sous le dossier des utilisateurs de Windows : il nomme le poste et son utilisateur.
    Chemin,
}

/// Ce qui attend la décision de Léandre (lot N1, 14/10/2026) : le verrou qu'il suspend, les fichiers (relatifs à la racine
/// du dépôt), et pourquoi. Un motif qui ne couvre plus rien se retire (`chaque_suspens_couvre_encore_un_fichier`) : la
/// liste ne fait que rétrécir. `prive/journal/finitions/PASSATION-N1.md` dit ce qui est proposé pour chacun.
const EN_SUSPENS: [(Verrou, &str, &str); 8] = [
    (Verrou::Nom, "^atelier/prive-dev/", "la copie de développement : des données de la compagnie du poste"),
    (Verrou::Siren, "^atelier/prive-dev/", "la copie de développement : des données de la compagnie du poste"),
    (Verrou::Chemin, "^atelier/prive-dev/", "la copie de développement : des données de la compagnie du poste"),
    (Verrou::Nom, "^atelier/site/", "les pages du site de l'éditeur, qui nomment l'éditeur : ici ou dans le dépôt du site"),
    (Verrou::Chemin, "^atelier/bac-a-sable/", "le bac à sable de la mise à jour : son maj.json et ses sorties portent le poste"),
    (Verrou::Chemin, r"^fictives/[^/]+/journal/depots\.json$", "le moteur écrit la trace d'un dépôt par son chemin absolu (depot_acte.rs)"),
    (Verrou::Chemin, r"^fictives/[^/]+/journal/incidents\.json$", "des traces Python d'avant le portage ; un fichier propre au poste (PROPRES_AU_POSTE)"),
    (Verrou::Chemin, r"^fictives/[^/]+/sorties/.+\.description\.json$", "le moteur écrit le logo et la signature par leur chemin absolu (contrat.rs, bulletin.rs)"),
];

/// Un chemin absolu sous le dossier des utilisateurs de Windows, dans toutes ses écritures (`\`, `/`, `\\` d'un JSON).
fn chemin_du_poste() -> regex::bytes::Regex {
    regex::bytes::Regex::new(r"(?i)(?-u:\b)[a-z]:[\\/]+users[\\/]").unwrap()
}

/// Un chemin vers les données du poste construit depuis le code : le parent d'un chemin suivi de `prive`, ou un relatif
/// qui remonte. Un dossier de test qui s'appelle `prive` (`monde().join("prive")`) n'est pas visé.
fn vers_prive() -> Regex {
    Regex::new(r#"(?:parent\(\)|"\.\.")[^;\n]*?\.join\(\s*"prive"|\.\./prive"#).unwrap()
}

fn depot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn prive() -> PathBuf {
    depot().join(concat!("pri", "ve"))
}

fn fichiers(racine: &Path, sortie: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(racine).unwrap().flatten() {
        let p = e.path();
        if HORS_CODE.contains(&e.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        if e.file_type().unwrap().is_dir() {
            fichiers(&p, sortie);
        }
        sortie.push(p);
    }
}

/// Chaque fichier et dossier de ce qui sera public.
fn publics() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in PUBLIC {
        fichiers(&depot().join(d), &mut v);
    }
    v.sort();
    v
}

/// Ce que Git emporterait, hors de `prive/` : les fichiers suivis et ceux qu'un `git add -A` prendrait. Les `.gitignore`
/// (négations comprises) sont appliqués par Git lui-même : ce qu'ils tiennent dehors ne part pas.
fn ce_qui_part() -> Vec<String> {
    let sortie = Command::new("git")
        .arg("-C")
        .arg(depot())
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .output()
        .expect("git introuvable : ce verrou lit ce que Git emporterait");
    assert!(sortie.status.success(), "{}", String::from_utf8_lossy(&sortie.stderr));
    let mut v: Vec<String> = sortie
        .stdout
        .split(|&b| b == 0)
        .map(|r| String::from_utf8_lossy(r).into_owned())
        .filter(|r| !r.is_empty() && !r.starts_with("prive/") && depot().join(r).is_file())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Le motif d'`EN_SUSPENS` qui couvre `rel` pour ce verrou.
fn suspendu(verrou: Verrou, rel: &str) -> Option<&'static str> {
    EN_SUSPENS.iter().find(|(v, motif, _)| *v == verrou && Regex::new(motif).unwrap().is_match(rel)).map(|s| s.1)
}

/// Où `verrou` trouve ce qu'il refuse dans `rel` (le nom du fichier compris) : `rel:ligne`, ou `rel` seul pour un nom.
fn trouvailles(verrou: Verrou, rel: &str) -> Vec<String> {
    let mut trouves = Vec::new();
    if verrou == Verrou::Nom && rel.to_lowercase().contains(NOM_REEL) {
        trouves.push(rel.to_string());
    }
    let Some(o) = texte(&depot().join(rel)) else { return trouves };
    let position = match verrou {
        Verrou::Nom => o.to_ascii_lowercase().windows(NOM_REEL.len()).position(|w| w == NOM_REEL.as_bytes()),
        Verrou::Siren => o.windows(SIREN_REEL.len()).position(|w| w == SIREN_REEL.as_bytes()),
        Verrou::Chemin => chemin_du_poste().find(&o).map(|m| m.start()),
    };
    trouves.extend(position.map(|i| format!("{}:{}", rel, ligne(&o, i))));
    trouves
}

/// Ce que `verrou` trouve dans ce qui part, hors de ce qui est en suspens.
fn hors_suspens(verrou: Verrou) -> Vec<String> {
    ce_qui_part().iter().filter(|r| suspendu(verrou, r).is_none()).flat_map(|r| trouvailles(verrou, r)).collect()
}

/// Les octets d'un fichier écrit à la main : de l'UTF-8, ou un texte à un octet par caractère (un `.dsn`, un gabarit à
/// largeur fixe) ; `None` pour un binaire (un exe, un PDF, une image).
fn texte(p: &Path) -> Option<Vec<u8>> {
    if !p.is_file() {
        return None;
    }
    let o = std::fs::read(p).unwrap();
    let texte = std::str::from_utf8(&o).is_ok() || !o.iter().any(|&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r'));
    texte.then_some(o)
}

fn ligne(o: &[u8], i: usize) -> usize {
    o[..i].iter().filter(|&&b| b == b'\n').count() + 1
}

fn relatif(p: &Path) -> String {
    p.strip_prefix(depot()).unwrap_or(p).display().to_string()
}

/// Sexe, année, mois (01-12, ou 20-99 pour les cas spéciaux), département (deux chiffres, 2A, 2B), six chiffres, puis la
/// clé si elle suit : `(début, fin, les treize sans la clé)` de chaque numéro qui n'est ni précédé d'une lettre ou d'un
/// chiffre, ni suivi d'un chiffre.
fn nirs(o: &[u8]) -> Vec<(usize, usize, String)> {
    let chiffre = |i: usize| o.get(i).is_some_and(u8::is_ascii_digit);
    let mut trouves = Vec::new();
    for i in 0..o.len().saturating_sub(12) {
        if !matches!(o[i], b'1' | b'2') || (i > 0 && o[i - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let mois = match (o[i + 3], o[i + 4]) {
            (b'0', b'1'..=b'9') | (b'1', b'0'..=b'2') | (b'2'..=b'9', b'0'..=b'9') => true,
            _ => false,
        };
        let departement = (chiffre(i + 5) && chiffre(i + 6)) || (o[i + 5] == b'2' && matches!(o[i + 6], b'A' | b'B'));
        if !(chiffre(i + 1) && chiffre(i + 2) && mois && departement && (i + 7..i + 13).all(chiffre)) {
            continue;
        }
        let suite = (i + 13..).take_while(|&j| chiffre(j)).count();
        if suite == 0 || suite == 2 {
            trouves.push((i, i + 13 + suite, String::from_utf8_lossy(&o[i..i + 13]).into_owned()));
        }
    }
    trouves
}

/// Un numéro de document dans une adresse web (l'Unédic en a de treize chiffres) n'est pas un NIR.
fn dans_une_url(o: &[u8], i: usize) -> bool {
    let debut = o[..i].iter().rposition(|b| b" \t\n\"'(<".contains(b)).map_or(0, |p| p + 1);
    o[debut..].starts_with(b"http://") || o[debut..].starts_with(b"https://")
}

fn invente(nir: &str) -> bool {
    INVENTE.iter().any(|x| nir.contains(x))
}

/// Les numéros à la forme d'un NIR, sans la marque d'invention, hors d'une adresse web : `(fichier relatif, ligne, numéro)`.
fn nirs_reels(p: &Path) -> Vec<(String, usize, String)> {
    let Some(o) = texte(p) else { return Vec::new() };
    nirs(&o)
        .into_iter()
        .filter(|(debut, _, nir)| !invente(nir) && !dans_une_url(&o, *debut))
        .map(|(debut, fin, _)| (relatif(p), ligne(&o, debut), String::from_utf8_lossy(&o[debut..fin]).into_owned()))
        .collect()
}

/// Le motif d'`EXEMPLES_DES_TEXTES` qui couvre `rel`.
fn exemple_des_textes(rel: &str) -> Option<&'static str> {
    EXEMPLES_DES_TEXTES.iter().find(|(motif, _)| Regex::new(motif).unwrap().is_match(rel)).map(|e| e.0)
}

#[test]
fn aucun_nir_reel_n_est_ecrit_dans_ce_qui_sera_public() {
    let trouves: Vec<String> = publics()
        .iter()
        .flat_map(|p| nirs_reels(p))
        .filter(|(rel, _, _)| exemple_des_textes(rel).is_none())
        .map(|(rel, l, n)| format!("{}:{} {}", rel, l, n))
        .collect();
    assert!(trouves.is_empty(), "NIR sans la marque d'un numéro inventé (999, 123456 ou 000) : {:#?}", trouves);
}

#[test]
fn chaque_exemple_des_textes_couvre_encore_un_numero() {
    let trouves: Vec<String> = publics().iter().flat_map(|p| nirs_reels(p)).map(|(rel, _, _)| rel).collect();
    let vides: Vec<&str> =
        EXEMPLES_DES_TEXTES.iter().filter(|(motif, _)| !trouves.iter().any(|r| exemple_des_textes(r) == Some(*motif))).map(|e| e.1).collect();
    assert!(vides.is_empty(), "ces exemples ne couvrent plus rien, ils se retirent d'EXEMPLES_DES_TEXTES : {:#?}", vides);
}

#[test]
fn le_siren_de_la_compagnie_reelle_n_est_ecrit_nulle_part_hors_de_prive() {
    let trouves = hors_suspens(Verrou::Siren);
    assert!(trouves.is_empty(), "{:#?}", trouves);
}

#[test]
fn le_nom_de_la_compagnie_reelle_n_est_ecrit_nulle_part_hors_de_prive() {
    let trouves = hors_suspens(Verrou::Nom);
    assert!(trouves.is_empty(), "le logiciel est pour toute compagnie, il ne porte pas le nom de son banc d'essai : {:#?}", trouves);
}

#[test]
fn aucun_chemin_du_poste_n_est_ecrit_hors_de_prive() {
    let trouves = hors_suspens(Verrou::Chemin);
    assert!(trouves.is_empty(), "un chemin du poste dit qui l'utilise et où ; le dépôt public n'en porte pas : {:#?}", trouves);
}

/// La copie de développement ne part pas au dépôt public (lot N1, 14/10/2026) : là où elle n'est pas, ses suspens ne
/// couvrent rien sans que la liste ait à changer. Partout ailleurs, un suspens qui ne couvre plus rien se retire.
fn hors_du_depot(motif: &str) -> bool {
    motif == "^atelier/prive-dev/" && !depot().join("atelier/prive-dev").is_dir()
}

#[test]
fn chaque_suspens_couvre_encore_un_fichier() {
    let part = ce_qui_part();
    let vides: Vec<String> = EN_SUSPENS
        .iter()
        .filter(|(_, motif, _)| !hors_du_depot(motif))
        .filter(|(v, motif, _)| !part.iter().any(|r| suspendu(*v, r) == Some(*motif) && !trouvailles(*v, r).is_empty()))
        .map(|(v, motif, raison)| format!("{:?} {} ({})", v, motif, raison))
        .collect();
    assert!(vides.is_empty(), "ces suspens ne couvrent plus rien, ils se retirent d'EN_SUSPENS : {:#?}", vides);
}

#[test]
fn aucun_test_ne_construit_un_chemin_vers_prive() {
    let re = vers_prive();
    let mut trouves = Vec::new();
    for p in publics() {
        let dans_les_tests = p.strip_prefix(depot().join("atelier/sources")).is_ok_and(|r| r.components().any(|c| c.as_os_str() == "tests"));
        if !dans_les_tests || p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let t = std::fs::read_to_string(&p).unwrap();
        trouves.extend(re.find_iter(&t).map(|m| format!("{}:{}", relatif(&p), ligne(t.as_bytes(), m.start()))));
    }
    assert!(trouves.is_empty(), "{:#?}", trouves);
}

#[test]
fn les_dossiers_de_la_suite_ne_sont_pas_dans_prive() {
    let prive = prive();
    // `target/tmp` se vide avant une suite (`atelier/CLAUDE.md`, « Les tests ») : Cargo ne le recrée qu'à la compilation,
    // et ce test peut passer avant qu'un autre l'ait recréé ; `canonicalize` exige qu'il existe.
    std::fs::create_dir_all(env!("CARGO_TARGET_TMPDIR")).unwrap();
    for d in [Path::new(env!("CARGO_TARGET_TMPDIR")).to_path_buf(), Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees")] {
        let d = d.canonicalize().unwrap();
        assert!(!d.starts_with(&prive), "{}", d.display());
    }
}

#[test]
fn le_verrou_reconnait_ce_qu_il_refuse() {
    // Un numéro à la forme d'un NIR, sans la marque d'invention, composé à la volée : écrit tel quel, il tomberait sous
    // le premier test.
    let sans_marque = format!("{}{}", "18503754", "12387");
    let dsn = format!("S21.G00.30.001,'{}'", sans_marque);
    assert_eq!(nirs(dsn.as_bytes()).iter().map(|n| n.2.as_str()).collect::<Vec<_>>(), [sans_marque.as_str()]);
    assert!(!invente(&sans_marque));
    let avec_cle = format!("{}97 ", sans_marque);
    assert_eq!(nirs(avec_cle.as_bytes()).len(), 1);
    for trop_long in [format!("{}9", sans_marque), format!("{}979", sans_marque), format!("x{}", sans_marque)] {
        assert!(nirs(trop_long.as_bytes()).is_empty(), "{}", trop_long);
    }
    let corse = format!("{}{}", "185032A", "123457");
    assert_eq!(nirs(corse.as_bytes()).len(), 1);
    let url = format!("https://www.unedic.org/doc/{}", sans_marque);
    assert!(dans_une_url(url.as_bytes(), url.len() - 13));
    assert!(["1850375999001", "1850675123456", "1850675000456"].iter().all(|n| invente(n)));
    let re = vers_prive();
    for construit in [
        format!("{}{}", r#"Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("pri"#, r#"ve")"#),
        format!("{}{}", r#"code.join("..").join("pri"#, r#"ve")"#),
        format!("{}{}", r#"concat!(env!("CARGO_MANIFEST_DIR"), "/../../../pri"#, r#"ve")"#),
    ] {
        assert!(re.is_match(&construit), "{}", construit);
    }
    assert!(!re.is_match(r#"monde().join("prive").join("config")"#));
    // Les trois écritures d'un chemin du poste, composées à la volée pour que ce fichier n'en porte aucun.
    let poste = chemin_du_poste();
    for ecrit in [format!("{}{}", "C:", r"\Users\nom\Documents"), format!("{}{}", "c:", "/users/nom"), format!("{}{}", r#""D:"#, r#"\\Users\\nom""#)] {
        assert!(poste.is_match(ecrit.as_bytes()), "{}", ecrit);
    }
    for ailleurs in ["%LOCALAPPDATA%\\puppetpaie", "/home/user/depot", "C:/Program Files/PuppetPaie"] {
        assert!(!poste.is_match(ailleurs.as_bytes()), "{}", ailleurs);
    }
    assert_eq!(suspendu(Verrou::Chemin, "fictives/fictive-x/sorties/2026-10/Contrats.description.json"), Some(EN_SUSPENS[7].1));
    assert_eq!(suspendu(Verrou::Chemin, "fictives/fictive-x/config/entreprise.json"), None);
    assert_eq!(suspendu(Verrou::Nom, "atelier/bac-a-sable/LISEZ-MOI.md"), None);
}
