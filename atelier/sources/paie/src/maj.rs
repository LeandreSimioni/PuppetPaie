//! La mise a jour dans l'ombre (`moteur/maj.py`, lot J4) : verifier, appliquer, revenir ; et, chez l'editeur, publier.
//!
//! UNE MISE A JOUR PAR INSTALLATION (lot U1, 14/10/2026). Il n'y a qu'un logiciel par installation, et c'est lui qui se
//! met a jour, pas une compagnie : tout se passe dans `<installation>/maj/` (a cote de `logiciel/`, que la bascule ne
//! touche pas), sans qu'aucune compagnie soit ouverte. Toute installation recoit, celle de l'editeur comprise (Leandre,
//! 14/10/2026 : « mon logiciel c'est le meme que les autres […] quand je fais une mise a jour, il faut que la mise a jour
//! arrive aussi sur ma machine ») ; l'editeur, en plus, publie. Le bridage n'y joue aucun role.
//!
//!   verifier   au lancement, en fond (`chercher_en_fond`). Lit `derniere.json` au depot ; une version plus recente est
//!              prise, son condensat et la SIGNATURE de l'editeur (Ed25519, `cles_de_l_editeur`) verifies, deballee dans
//!              `maj/<version>/`, CONTROLEE (son programme lit la configuration de chaque compagnie de l'installation),
//!              puis `maj/prete.json` est pose.
//!   appliquer  au demarrage suivant. Ce qui est dans `logiciel/` va dans `logiciel/.avant/`, la version prete prend sa
//!              place ; si ca casse en route, tout revient.
//!   publier    chez l'editeur : l'archive signee et `derniere.json` dans le depot. Depuis le lot P1 (14/10/2026), la
//!              publication se fait sur GitHub : le flux « Publier » du depot public (`.github/workflows/publier.yml`)
//!              fabrique `logiciel/`, attend l'approbation de Leandre, signe par `publier_depuis_l_environnement` et
//!              pose l'archive, `derniere.json` et l'installeur dans une Release, que `DEPOT_PAR_DEFAUT` lit.
//!
//! LE RELAIS, AU LANCEMENT (09/10/2026 : « que personne ne voit »). Sous Windows, un programme ne se remplace pas pendant
//! qu'il tourne. Le programme qu'on lance regarde donc, AVANT TOUTE FENETRE, si l'installation a une version prete et
//! applicable (`au_lancement`) : il copie le programme NEUF hors de `logiciel/` (`maj/relais-<version>`), le lance avec
//! `maj-relais`, et s'arrete sans rien avoir montre. Le relais attend la fin de ce processus (par son numero, pas par un
//! port), bascule, lance la fenetre neuve, et s'arrete : on voit s'ouvrir directement la neuve.
//!
//! Deux garde-fous, comme le Python : une version qui change un CALCUL attend que chaque mois saisi d'une compagnie reelle
//! de l'installation ait sa DSN (sauf
//! `malgre_calcul` ou une version EXIGEE) ; une version exigee bloque tout depot reel tant qu'elle n'est pas appliquee
//! (`depot_acte::exigee_en_attente`).
//!
//! LA CLE DE SIGNATURE (`config/maj-cle-privee.bin`, lot K1, 14/10/2026 ; Leandre : « si mon ordi tombe, on pourra plus
//! faire de mise a jour, ça me va pas »). Sa graine est scellee par la CLE MAITRESSE DU COFFRE
//! (`Coffre::proteger_par_le_coffre`), dans un petit JSON (`FORMAT_CLE`) : elle s'ouvre partout ou le coffre s'ouvre, par
//! la cle du poste, ou ailleurs par le mot de passe du logiciel (tape, ou la seance). Elle depend donc du coffre qui l'a
//! scellee : sans lui, rien ne l'ouvre, et un coffre neuf ne l'ouvrira pas. Avant, elle etait scellee par la cle du poste
//! seule (`Coffre::proteger`) : perdue avec le poste, les compagnies auraient refuse toute version suivante.
//! UNE CLE SCELLEE A L'ANCIENNE s'ouvre encore par la cle du poste qui l'a faite, et s'y RESCELLE par le coffre a cette
//! ouverture (`cle_privee`) : c'est la seule facon de la sauver sans changer la paire, donc sans faire parvenir une
//! nouvelle cle publique a chaque compagnie. `paie maj --cle` sur une cle a l'ancienne fait cette ouverture sans publier.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::coffre::Coffre;


/// La cle publique de l'editeur (Ed25519, 32 octets en base64) : toute archive non signee par elle est refusee.
/// Refaite le 09/10/2026 : l'ancienne (16/09, scellee par le moteur Python) n'existait plus sur le poste ; aucune
/// compagnie n'avait encore recu de version.
pub const CLE_PUBLIQUE: &str = "sDj5+46oBt7Yy1x00VJ1ThJNOwf5jT5+bttXEN/yAIU=";
/// LA CLE PUBLIQUE DE GITHUB (lot P1, 14/10/2026) : le flux « Publier » du depot public signe l'archive avec une cle
/// neuve, dediee, dont la graine est le secret `PUPPETPAIE_CLE_SIGNATURE` de l'environnement `publication` (approuve
/// par Leandre). Fabriquee sur le poste de l'editeur par `paie maj --cle-github` (`generer_cle_github`), qui ecrit la
/// cle publique dans un fichier a reporter ICI. Reportee le 14/10/2026 (fabriquee par Leandre sur son poste). Vide,
/// aucune archive ne serait acceptee par elle et le flux refuserait de publier (`publier_depuis_l_environnement`).
pub const CLE_PUBLIQUE_GITHUB: &str = "48dC1Gb82e2Zh7z7NiUH14EbXX7IvXTp83Xaui/31wg=";
/// Le depot des mises a jour QUAND LE REGLAGE DE L'INSTALLATION NE DIT RIEN : les fichiers de la DERNIERE RELEASE du
/// depot public (lot P1, 14/10/2026 ; avant, le serveur du filet, qui ne servait rien). GitHub repond a
/// `releases/latest/download/<fichier>` par une redirection vers la Release la plus recente, puis vers son stockage :
/// `lire_depot` les suit (ureq, dix au plus). `derniere.json` y nomme l'archive, servie a cote. Un `depot` pose dans le
/// reglage le remplace toujours. Il ne sert qu'a RECEVOIR (`verifier`) : `publier` ecrit dans un dossier.
pub const DEPOT_PAR_DEFAUT: &str = "https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download";
/// La variable d'environnement ou le flux « Publier » pose la graine (base64) : lue par `publier_depuis_l_environnement`,
/// jamais affichee.
pub const VARIABLE_GRAINE: &str = "PUPPETPAIE_CLE_SIGNATURE";
const DERNIERE: &str = "derniere.json";
const AGENT: &str = "PuppetPaie (mise a jour)";
const FICHIER_CLE_PRIVEE: &str = "maj-cle-privee.bin";
const DESCRIPTION_CLE: &str = "PuppetPaie : cle de signature des mises a jour";
/// Ce que porte `maj-cle-privee.bin` depuis le lot K1 : `{"format": FORMAT_CLE, "scelle": <base64>}`, la graine scellee par
/// la cle maitresse du coffre. Tout autre contenu est une cle scellee a l'ancienne, par la cle du poste seule.
const FORMAT_CLE: &str = "puppetpaie/maj-cle/scellee-par-le-coffre";
const SANS_CLE: &str = "pas de cle privee : les compagnies refuseraient une archive non signee (paie maj --cle pour la fabriquer)";
/// Ce qui ne part jamais dans une archive, ni ne bouge a la bascule.
const EXCLUS: [&str; 6] = [".git", ".avant", ".apres", "__pycache__", ".pytest_cache", "target"];
const NE_PAS_TOUCHER: [&str; 3] = [".git", ".avant", ".apres"];
/// Ce qui, dans `logiciel/`, dit l'INSTALLATION et non la version : il ne part jamais dans une archive (`archiver`), et la
/// bascule le laisse EXACTEMENT comme il etait -- remis s'il etait la (copie depuis `.avant/`, qui garde l'image entiere de
/// la version d'avant pour `revenir`), absent s'il n'y etait pas, meme si une archive d'avant le lot V1 en portait un.
/// Leandre, 14/10/2026 : « le petit curseur ecrit en JSON lui il reste comme il etait. Si les gens ont debloque, elle reste
/// debloquee » ; bridee, elle reste bridee. (L'autre moitie du curseur, `.edition.json`, vit a cote
/// de `logiciel/` : la bascule n'y touche pas.) Avant, l'`EDITION.json` de l'installeur partait a la premiere mise a jour
/// et le poste se debloquait sans geste (`PASSATION-I1.md`, ouvert 1).
const PROPRES_A_L_INSTALLATION: [&str; 1] = ["EDITION.json"];
/// Les programmes d'un systeme Unix dans `logiciel/` (la fenetre, et le moteur pour un Linux sans WebKitGTK, qui ouvre la
/// meme page dans le navigateur) : le flux « Publier » les pose a cote de `puppetpaie.exe` (Leandre, 15/10/2026 : « rajouter
/// a Publier le Linux »). Un zip ne garde pas le bit d'execution : `archiver` le marque, `deballer` le remet.
const PROGRAMMES_UNIX: [&str; 2] = ["puppetpaie", "paie"];

type R<T> = Result<T, String>;

/// Le dossier du logiciel (`logiciel/`), resolu.
pub fn code() -> PathBuf {
    crate::interface::compagnies::logiciel()
}

/// Le dossier des programmes : `logiciel/` lui-meme, ou l'exe est pose a cote de `donnees/` et `interface/` (rangement
/// du 09/10/2026 : `logiciel/` est exactement ce que les gens recoivent).
pub fn dossier_programmes(code: &Path) -> PathBuf {
    code.to_path_buf()
}

pub fn programme(code: &Path, nom: &str) -> PathBuf {
    dossier_programmes(code).join(if cfg!(windows) { format!("{}.exe", nom) } else { nom.to_string() })
}

pub fn courante(code: &Path) -> String {
    std::fs::read_to_string(code.join("VERSION")).map(|v| v.trim().to_string()).unwrap_or_else(|_| "0.0.0".into())
}

fn cle_de_version(v: &str) -> Vec<u64> {
    v.trim().split('.').map(|x| x.parse().unwrap_or(0)).collect()
}

pub fn plus_recente(candidate: &str, reference: &str) -> bool {
    cle_de_version(candidate) > cle_de_version(reference)
}

/// LA VERSION TELLE QU'UN HUMAIN LA LIT : `JJ.MM.AAAA` (Leandre, 14/10/2026 : « mettre la version jour.mois.annee plutot
/// que comme maintenant en commencant par annee, ca embrouille » ; `prive/journal/ETAT.md`, « DECIDE LE 14/10/2026 »).
/// L'IDENTIFIANT, lui, reste `AAAA.MM.JJ` (`VERSION`, `derniere.json`, les archives, `prive/maj/<version>/`, ce que signe
/// l'editeur) : `plus_recente` compare les trois nombres de gauche a droite, et `01.11.2026` passerait pour plus ancienne
/// que `14.10.2026`. Seul l'affichage change : tout ce qui montre une version passe par ici. Ce qui n'a pas la forme d'une
/// date (`0.0.0`, un texte libre) est rendu tel quel.
pub fn version_lue(v: &str) -> String {
    let t = v.trim();
    let p: Vec<&str> = t.split('.').collect();
    let chiffres = |x: &str, n: usize| x.len() == n && x.bytes().all(|c| c.is_ascii_digit());
    if p.len() == 3 && chiffres(p[0], 4) && chiffres(p[1], 2) && chiffres(p[2], 2) {
        format!("{}.{}.{}", p[2], p[1], p[0])
    } else if p.len() == 4 && chiffres(p[0], 4) && chiffres(p[1], 2) && chiffres(p[2], 2) && p[3].parse::<u32>().is_ok() {
        // La deuxieme version d'un meme jour (`AAAA.MM.JJ.N`, posee par le flux « Publier ») : « JJ.MM.AAAA (N) ».
        format!("{}.{}.{} ({})", p[2], p[1], p[0], p[3])
    } else {
        t.to_string()
    }
}

fn lire_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn ecrire_json(p: &Path, v: &Value) -> R<()> {
    crate::ecriture::ecrire_json(p, v, 1)
}

/// L'INSTALLATION : le dossier qui porte `logiciel/`, et a cote les compagnies (`prive/`, `compagnies/`, les fictives) et
/// `maj/`. Il n'y a qu'un logiciel par installation, et c'est lui qui se met a jour (lot U1, 14/10/2026 ; Leandre : « il
/// n'y a pas de logiciels differents, il n'y en a qu'un »), pas une compagnie : la mise a jour se cherche, se range et
/// s'applique ici, sans qu'aucune compagnie soit ouverte. Un dossier portable est une installation comme une autre.
pub fn installation() -> PathBuf {
    crate::interface::compagnies::base()
}

/// `<installation>/maj/` : le reglage, la version prete, ce qui a ete applique, le journal. A cote de `logiciel/`, que la
/// bascule ne touche pas.
pub fn dossier_maj(inst: &Path) -> PathBuf {
    inst.join("maj")
}

/// Le logiciel de l'installation.
fn logiciel_de(inst: &Path) -> PathBuf {
    inst.join("logiciel")
}

const REGLAGE: &str = "reglage.json";
/// Le reglage d'avant le lot U1, lu EN REPLI quand l'installation n'a pas le sien : la compagnie du poste le portait
/// (`prive/config/maj.json`, `editeur` et `depot`), et c'est ainsi qu'est regle le poste de l'editeur. On le lit, on ne le
/// migre pas : rien a ecrire sur un poste qui marche, et un seul fichier fait foi a la fois (le neuf, s'il est pose).
const REGLAGE_D_AVANT: &str = "prive/config/maj.json";

/// Le reglage de l'installation : (depot pose, editeur). `maj/reglage.json`, sinon `prive/config/maj.json` (le reglage
/// d'avant le lot U1), sinon rien de pose. `editeur` ne sert qu'a autoriser la publication (Leandre, 14/10/2026 : « mon
/// logiciel c'est le meme que les autres. C'est juste que je peux travailler dessus, mais quand je fais une mise a jour, il
/// faut que la mise a jour arrive aussi sur ma machine ») : le poste de l'editeur recoit comme les autres.
pub fn reglage(inst: &Path) -> R<(Option<String>, bool)> {
    let neuf = dossier_maj(inst).join(REGLAGE);
    let (p, nom) = if neuf.exists() { (neuf, "maj/reglage.json") } else { (inst.join(REGLAGE_D_AVANT), REGLAGE_D_AVANT) };
    if !p.exists() {
        return Ok((None, false));
    }
    let d = lire_json(&p).filter(Value::is_object).ok_or_else(|| format!("{} illisible", nom))?;
    let depot = d.get("depot").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
    Ok((depot, d.get("editeur").and_then(Value::as_bool).unwrap_or(false)))
}

/// Le depot d'ou l'installation recoit : celui qui est pose, sinon `DEPOT_PAR_DEFAUT`.
pub fn depot(pose: Option<String>) -> String {
    pose.unwrap_or_else(|| DEPOT_PAR_DEFAUT.to_string())
}

/// La version telechargee, controlee, a appliquer.
pub fn prete(inst: &Path) -> Option<Value> {
    let d = lire_json(&dossier_maj(inst).join("prete.json"))?;
    let v = d.get("version")?.as_str()?;
    dossier_maj(inst).join(v).join("logiciel").join("donnees").is_dir().then_some(d)
}

fn lire_depot(depot: &str, nom: &str) -> R<Vec<u8>> {
    if depot.starts_with("http://") || depot.starts_with("https://") {
        let url = format!("{}/{}", depot.trim_end_matches('/'), nom);
        let mut c = ureq::Agent::config_builder().user_agent(AGENT).timeout_global(Some(std::time::Duration::from_secs(120)));
        if url.starts_with("https:") {
            c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
        }
        let agent: ureq::Agent = c.build().into();
        let mut r = agent.get(&url).call().map_err(|e| format!("{} : {}", url, e))?;
        return r.body_mut().with_config().limit(512 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string());
    }
    std::fs::read(Path::new(depot).join(nom)).map_err(|e| format!("{} : {}", Path::new(depot).join(nom).display(), e))
}

pub fn derniere(depot: &str) -> R<Value> {
    let d: Value = serde_json::from_slice(&lire_depot(depot, DERNIERE)?).map_err(|e| format!("derniere.json illisible : {}", e))?;
    for k in ["version", "archive", "sha256"] {
        if d.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).is_none() {
            return Err(format!("derniere.json sans « {} »", k));
        }
    }
    Ok(d)
}

fn b64(t: &str) -> R<Vec<u8>> {
    crate::retours::base64_lire(t)
}

/// Les cles publiques dont une archive est acceptee : celle du poste de l'editeur (`CLE_PUBLIQUE`) et celle du flux de
/// GitHub (`CLE_PUBLIQUE_GITHUB`), quand elle est reportee. Une signature de toute autre cle est refusee.
pub fn cles_de_l_editeur() -> Vec<&'static str> {
    [CLE_PUBLIQUE, CLE_PUBLIQUE_GITHUB].into_iter().filter(|c| !c.is_empty()).collect()
}

/// La signature est-elle celle de l'une des `cles` ?
pub fn signee_par_l_une(octets: &[u8], signature: &str, cles: &[&str]) -> bool {
    cles.iter().any(|c| signature_valide(octets, signature, c))
}

pub fn signature_valide(octets: &[u8], signature: &str, cle_publique: &str) -> bool {
    let (Ok(cle), Ok(sig)) = (b64(cle_publique), b64(signature)) else { return false };
    let (Ok(cle), Ok(sig)) = (<[u8; 32]>::try_from(cle.as_slice()), <[u8; 64]>::try_from(sig.as_slice())) else { return false };
    let Ok(v) = ed25519_dalek::VerifyingKey::from_bytes(&cle) else { return false };
    v.verify_strict(octets, &ed25519_dalek::Signature::from_bytes(&sig)).is_ok()
}

fn signer(octets: &[u8], privee: &[u8]) -> R<String> {
    use ed25519_dalek::Signer;
    let k = <[u8; 32]>::try_from(privee).map_err(|_| "cle privee illisible (32 octets attendus)")?;
    Ok(crate::retours::base64_ecrire(&ed25519_dalek::SigningKey::from_bytes(&k).sign(octets).to_bytes()))
}

/// Les compagnies de l'installation : la compagnie du poste (`prive/`), les autres (`compagnies/*`), les fictives ; pas
/// une copie de developpement.
pub fn compagnies_de(inst: &Path) -> Vec<PathBuf> {
    let mut v = vec![inst.join("prive")];
    for groupe in ["compagnies", crate::interface::compagnies::DOSSIER_FICTIVES] {
        if let Ok(it) = std::fs::read_dir(inst.join(groupe)) {
            let mut autres: Vec<PathBuf> = it.flatten().map(|e| e.path()).collect();
            autres.sort();
            v.extend(autres);
        }
    }
    v.retain(|r| r.join("config").join("entreprise.json").is_file() && (!crate::interface::compagnies::est_copie(r) || crate::interface::compagnies::est_fictive(r)));
    v
}

/// LE GARDE-FOU « UNE VERSION QUI CHANGE UN CALCUL ATTEND LA DSN DU MOIS », pour l'installation : les mois saisis sans DSN
/// acceptee de ses compagnies REELLES (`prive/`, `compagnies/*`), chacun suivi du nom de sa compagnie. Les fictives ne
/// retiennent rien : elles ne deposent jamais, leurs mois retiendraient la version pour toujours. Le garde-fou reste ce
/// qu'il etait (la DSN doit dire ce que disent les bulletins deja faits) ; il regarde seulement toutes les compagnies
/// qui declarent, puisque la version s'applique a toutes a la fois.
fn mois_qui_retiennent(inst: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for racine in compagnies_de(inst).into_iter().filter(|r| !crate::interface::compagnies::est_copie(r)) {
        let o = std::collections::HashMap::from([("racine".to_string(), racine.display().to_string())]);
        let nom = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Une compagnie qu'on ne lit pas retient la version : on ne sait pas si ses mois ont leur DSN.
        let lus = crate::service::contexte(&o).and_then(|ctx| crate::routes_etats::mois_saisis_non_deposes(&o, &ctx).map_err(|e| e.message));
        match lus {
            Ok(mois) => out.extend(mois.iter().map(|m| format!("{} ({})", m.as_str().unwrap_or_default(), nom))),
            Err(e) => out.push(format!("? ({} illisible : {})", nom, e)),
        }
    }
    out
}

fn deballer(octets: &[u8], cible: &Path) -> R<()> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| format!("archive illisible : {}", e))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { return Err(format!("archive suspecte : {}", f.name())) };
        let chemin = cible.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&chemin).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut o = Vec::new();
        f.read_to_end(&mut o).map_err(|e| e.to_string())?;
        std::fs::write(&chemin, o).map_err(|e| e.to_string())?;
    }
    // Le programme d'un systeme Unix : executable (le zip ne le garde pas).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for nom in PROGRAMMES_UNIX {
            let _ = std::fs::set_permissions(cible.join(nom), std::fs::Permissions::from_mode(0o755));
        }
    }
    Ok(())
}

/// Le nouveau code demarre-t-il, et lit-il NOS donnees ? Son programme, sur la racine de cette compagnie.
fn controler(neuf: &Path, racine: &Path) -> R<()> {
    // La fenetre sert aussi les routes (`puppetpaie route`) : c'est elle qu'on livre, pas la ligne de commande.
    let exe = programme(neuf, "puppetpaie");
    if !exe.is_file() {
        return Err(format!("le programme de ce systeme manque dans l'archive ({})", exe.display()));
    }
    let mut c = std::process::Command::new(&exe);
    c.args(["route", "--racine"])
        .arg(racine)
        .env("PUPPETPAIE_CODE", neuf.join("donnees"))
        .env("PUPPETPAIE_DONNEES", racine)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let mut p = c.spawn().map_err(|e| format!("le nouveau programme ne se lance pas : {}", e))?;
    if let Some(mut e) = p.stdin.take() {
        let _ = e.write_all(br#"{"route": "configuration", "charge": {}}"#);
    }
    let s = p.wait_with_output().map_err(|e| e.to_string())?;
    let rep: Option<Value> = String::from_utf8_lossy(&s.stdout).lines().rev().find_map(|l| serde_json::from_str(l).ok());
    match rep {
        Some(r) if r.get("ok").and_then(Value::as_bool) == Some(true) => Ok(()),
        Some(r) => Err(format!("il ne lit pas la configuration : {}", r.get("erreur").map(|e| e.to_string()).unwrap_or_default())),
        None => Err(String::from_utf8_lossy(&s.stderr).trim().chars().rev().take(800).collect::<String>().chars().rev().collect()),
    }
}

fn telecharger(inst: &Path, depot: &str, d: &Value, cles: &[&str]) -> R<()> {
    let version = d["version"].as_str().unwrap_or_default();
    let octets = lire_depot(depot, d["archive"].as_str().unwrap_or_default())?;
    let sha: String = Sha256::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect();
    let attendu = d["sha256"].as_str().unwrap_or_default();
    if sha != attendu {
        return Err(format!("archive alteree (condensat {}… attendu {}…)", &sha[..12], attendu.chars().take(12).collect::<String>()));
    }
    let signature = d.get("signature").and_then(Value::as_str).ok_or("archive non signee : derniere.json n'a pas de « signature »")?;
    if !signee_par_l_une(&octets, signature, cles) {
        return Err("la signature de l'archive n'est pas celle de l'editeur".into());
    }
    let cible = dossier_maj(inst).join(version);
    let _ = std::fs::remove_dir_all(&cible);
    let neuf = cible.join("logiciel");
    std::fs::create_dir_all(&neuf).map_err(|e| e.to_string())?;
    deballer(&octets, &neuf)?;
    if courante(&neuf) != version {
        return Err(format!("l'archive dit {}, derniere.json dit {}", version_lue(&courante(&neuf)), version_lue(version)));
    }
    // Le programme neuf lit la configuration de chaque compagnie de l'installation (sans compagnie, il n'y a rien a lire).
    for racine in compagnies_de(inst) {
        if let Err(p) = controler(&neuf, &racine) {
            let _ = std::fs::remove_dir_all(&cible);
            return Err(format!("le nouveau code ne demarre pas sur {} : {}", racine.display(), p));
        }
    }
    ecrire_json(
        &dossier_maj(inst).join("prete.json"),
        &json!({
            "version": version, "quand": crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string(),
            "notes": d.get("notes").cloned().unwrap_or(json!("")), "calcul": d.get("calcul").cloned().unwrap_or(json!("")),
            "exigee": d.get("exigee").cloned().unwrap_or(json!("")),
        }),
    )
}

/// Au lancement, en fond. Ne leve jamais : rend une phrase (ce qui s'est passe), pour le journal de la mise a jour.
pub fn verifier(telecharger_aussi: bool) -> String {
    verifier_avec(&installation(), telecharger_aussi, &cles_de_l_editeur())
}

/// `verifier`, l'installation et les cles publiques acceptees donnees (le moteur livre passe toujours `installation()` et
/// `cles_de_l_editeur()` ; les essais de bout en bout, une installation fictive et des cles d'essai :
/// `tests/fichiers/mise_a_jour.rs`).
/// Toute installation recoit, celle de l'editeur comprise : elle a deja la version qu'elle vient de publier depuis son
/// propre `logiciel/`, et recoit celle qu'on aurait publiee d'ailleurs.
pub fn verifier_avec(inst: &Path, telecharger_aussi: bool, cles: &[&str]) -> String {
    let c = logiciel_de(inst);
    let cour = version_lue(&courante(&c));
    let depot = match reglage(inst) {
        Ok((d, _)) => self::depot(d),
        Err(e) => return format!("version {} ({})", cour, e),
    };
    let d = match derniere(&depot) {
        Ok(d) => d,
        Err(e) => return format!("version {} (depot injoignable : {})", cour, e),
    };
    let v = d["version"].as_str().unwrap_or_default().to_string();
    if !plus_recente(&v, &courante(&c)) {
        return format!("version {}, a jour", cour);
    }
    let lue = version_lue(&v);
    if prete(inst).and_then(|p| p["version"].as_str().map(String::from)).as_deref() == Some(v.as_str()) || !telecharger_aussi {
        return format!("version {} -- la {} est prete", cour, lue);
    }
    match telecharger(inst, &depot, &d, cles) {
        Ok(()) => format!("version {} -- la {} est telechargee, controlee, prete : elle s'appliquera au prochain demarrage", cour, lue),
        Err(e) => format!("version {} -- la {} existe mais n'a pas pu etre preparee : {}", cour, lue, e),
    }
}

/// La version prete et applicable maintenant (rien ne la retient), ou `None`.
pub fn applicable(inst: &Path) -> Option<String> {
    let p = prete(inst)?;
    let v = p["version"].as_str()?.to_string();
    if !plus_recente(&v, &courante(&logiciel_de(inst))) {
        return None;
    }
    let force = p.get("malgre_calcul").and_then(Value::as_bool) == Some(true) || p.get("exigee").and_then(Value::as_str).is_some_and(|e| !e.is_empty());
    let calcul = p.get("calcul").and_then(Value::as_str).is_some_and(|c| !c.is_empty());
    if calcul && !force && !mois_qui_retiennent(inst).is_empty() {
        return None;
    }
    Some(v)
}

fn deplacer(de: &Path, vers: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if tests::echouer_sur(de) {
        return Err(std::io::Error::other("echec provoque par l'essai"));
    }
    std::fs::rename(de, vers)
}

fn copier(de: &Path, vers: &Path) -> std::io::Result<u64> {
    #[cfg(test)]
    if tests::echouer_sur(de) {
        return Err(std::io::Error::other("echec provoque par l'essai"));
    }
    std::fs::copy(de, vers)
}

/// La bascule : `logiciel/` -> `logiciel/.avant/`, la version prete a sa place ; tout revient si ca casse.
pub fn appliquer(inst: &Path, malgre_calcul: bool) -> R<Value> {
    appliquer_depuis(&dossier_maj(inst), &logiciel_de(inst), malgre_calcul)
}

/// `appliquer`, le dossier de la version prete et le logiciel donnes : `<installation>/maj/`, ou, pour le relais lance par
/// une version d'avant le lot U1, le `maj/` de la compagnie ou elle l'avait rangee.
fn appliquer_depuis(maj: &Path, code: &Path, malgre_calcul: bool) -> R<Value> {
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let d = (|| {
        let d = lire_json(&maj.join("prete.json"))?;
        maj.join(d.get("version")?.as_str()?).join("logiciel").join("donnees").is_dir().then_some(d)
    })()
    .ok_or("aucune version prete a appliquer")?;
    let version = d["version"].as_str().unwrap_or_default().to_string();
    let force = malgre_calcul || d.get("malgre_calcul").and_then(Value::as_bool) == Some(true) || d.get("exigee").and_then(Value::as_str).is_some_and(|e| !e.is_empty());
    if d.get("calcul").and_then(Value::as_str).is_some_and(|c| !c.is_empty()) && !force {
        let attend = mois_qui_retiennent(&inst);
        if !attend.is_empty() {
            return Err(format!(
                "la {} change un calcul ({}) et le mois {} est saisi sans DSN acceptee : elle attend le depot, pour que la DSN dise ce que disent les bulletins. Pour l'appliquer quand meme : paie maj --appliquer --malgre-calcul",
                version_lue(&version),
                d["calcul"].as_str().unwrap_or_default(),
                attend.join(", ")
            ));
        }
    }
    let neuf = maj.join(&version).join("logiciel");
    let avant = code.join(".avant");
    let _ = std::fs::remove_dir_all(&avant);
    std::fs::create_dir_all(&avant).map_err(|e| e.to_string())?;
    let entrees = |d: &Path| -> R<Vec<PathBuf>> {
        Ok(std::fs::read_dir(d)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| !NE_PAS_TOUCHER.iter().any(|n| p.file_name().is_some_and(|f| f == *n)))
            .collect())
    };
    let mut deplaces: Vec<std::ffi::OsString> = Vec::new();
    let mut arrives: Vec<std::ffi::OsString> = Vec::new();
    let mut faire = || -> R<()> {
        for p in entrees(code)? {
            let nom = p.file_name().unwrap_or_default().to_os_string();
            deplacer(&p, &avant.join(&nom)).map_err(|e| format!("{} : {}", p.display(), e))?;
            deplaces.push(nom);
        }
        for p in entrees(&neuf)? {
            let nom = p.file_name().unwrap_or_default().to_os_string();
            deplacer(&p, &code.join(&nom)).map_err(|e| format!("{} : {}", p.display(), e))?;
            arrives.push(nom);
        }
        for nom in PROPRES_A_L_INSTALLATION {
            if code.join(nom).is_file() {
                std::fs::remove_file(code.join(nom)).map_err(|e| format!("{} : {}", code.join(nom).display(), e))?;
            }
            if avant.join(nom).is_file() {
                copier(&avant.join(nom), &code.join(nom)).map_err(|e| format!("{} : {}", code.join(nom).display(), e))?;
            }
        }
        Ok(())
    };
    if let Err(e) = faire() {
        // Tout revient : ce qui est arrive repart, ce qui est parti revient. Ce qui est arrive se suit par son nom, pas par
        // difference avec ce qui est parti (avant le lot V1, un fichier de la version neuve qui portait le nom d'un fichier
        // de l'ancienne -- VERSION, donnees -- restait en place, et l'ancien ne revenait pas : un melange des deux).
        for nom in &arrives {
            let _ = deplacer(&code.join(nom), &neuf.join(nom));
        }
        for nom in &deplaces {
            if avant.join(nom).exists() && !code.join(nom).exists() {
                let _ = deplacer(&avant.join(nom), &code.join(nom));
            }
        }
        return Err(format!("la bascule a echoue, l'ancienne version reste en place : {}", e));
    }
    let quand = crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string();
    std::fs::create_dir_all(dossier_maj(&inst)).map_err(|e| e.to_string())?;
    ecrire_json(
        &dossier_maj(&inst).join("appliquee.json"),
        &json!({"version": version, "quand": quand, "notes": d.get("notes").cloned().unwrap_or(json!("")), "calcul": d.get("calcul").cloned().unwrap_or(json!("")), "vue": false}),
    )?;
    let _ = std::fs::remove_file(maj.join("prete.json"));
    let _ = std::fs::remove_dir_all(maj.join(&version));
    Ok(json!({"version": version, "avant": avant.display().to_string(), "quand": quand}))
}

/// Reprendre la version d'avant, gardee dans `.avant/`. Une seule marche arriere.
pub fn revenir(inst: &Path) -> R<Value> {
    let code = &logiciel_de(inst);
    let avant = code.join(".avant");
    if !avant.join("donnees").is_dir() {
        return Err(format!("pas de version d'avant dans {}", avant.display()));
    }
    let version_avant = courante(&avant);
    // Ce qui part est DEPLACE dans `.apres/`, puis efface si possible : sous Windows, le programme qui lance `maj
    // --revenir` (`puppetpaie.exe`) ne peut pas s'effacer pendant qu'il tourne, mais il peut se deplacer.
    let apres = code.join(".apres");
    let _ = std::fs::remove_dir_all(&apres);
    std::fs::create_dir_all(&apres).map_err(|e| e.to_string())?;
    for e in std::fs::read_dir(code).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        if NE_PAS_TOUCHER.iter().any(|n| p.file_name().is_some_and(|f| f == *n)) {
            continue;
        }
        deplacer(&p, &apres.join(e.file_name())).map_err(|e| format!("{} : {}", p.display(), e))?;
    }
    for e in std::fs::read_dir(&avant).map_err(|e| e.to_string())?.flatten() {
        deplacer(&e.path(), &code.join(e.file_name())).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_dir(&avant);
    let _ = std::fs::remove_dir_all(&apres);
    let _ = std::fs::remove_file(dossier_maj(inst).join("appliquee.json"));
    Ok(json!({"version": version_avant}))
}

/// Les fichiers de `logiciel/` pour l'archive, dans l'ordre des noms (meme contenu, meme condensat).
fn fichiers(code: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    let mut v: Vec<_> = std::fs::read_dir(code.join(rel)).map(|it| it.flatten().collect()).unwrap_or_default();
    v.sort_by_key(|e| e.file_name());
    for e in v {
        let nom = e.file_name().to_string_lossy().into_owned();
        let r = rel.join(&nom);
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if !EXCLUS.contains(&nom.as_str()) {
                fichiers(code, &r, out);
            }
        } else if !nom.ends_with(".pyc") && !nom.ends_with(".pyo") && !(rel == Path::new("") && PROPRES_A_L_INSTALLATION.contains(&nom.as_str())) {
            out.push(r);
        }
    }
}

/// Le contenu de `logiciel/`, sans caches ni depot git ni version d'avant, dans un zip deterministe.
pub fn archiver(code: &Path) -> R<Vec<u8>> {
    let mut liste = Vec::new();
    fichiers(code, Path::new(""), &mut liste);
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let date = zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0).map_err(|e| format!("{:?}", e))?;
    for rel in liste {
        let nom = rel.to_string_lossy().replace('\\', "/");
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(date).unix_permissions(
            if PROGRAMMES_UNIX.contains(&nom.as_str()) { 0o755 } else { 0o644 },
        );
        z.start_file(nom, o).map_err(|e| e.to_string())?;
        z.write_all(&std::fs::read(code.join(&rel)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

/// Le scelle d'une cle au format du lot K1, ou None (une cle scellee a l'ancienne, ou autre chose).
fn scelle_par_le_coffre(octets: &[u8]) -> Option<Vec<u8>> {
    let v: Value = serde_json::from_slice(octets).ok()?;
    if v.get("format").and_then(Value::as_str) != Some(FORMAT_CLE) {
        return None;
    }
    b64(v.get("scelle")?.as_str()?).ok()
}

/// La cle de signature de la compagnie de `racine` : None s'il n'y en a pas, `Some(true)` si elle est scellee par le
/// coffre (elle s'ouvre par le mot de passe du logiciel), `Some(false)` si elle l'est a l'ancienne (par la cle du poste
/// qui l'a faite, seule). Ne lit que la forme du fichier, n'ouvre rien.
pub fn cle_scellee_par_le_coffre(racine: &Path) -> Option<bool> {
    let o = std::fs::read(racine.join("config").join(FICHIER_CLE_PRIVEE)).ok()?;
    Some(scelle_par_le_coffre(&o).is_some())
}

/// Ecrit la cle scellee par le coffre : un `.part` renomme, jamais un fichier coupe en route sous le vrai nom.
fn ecrire_cle(chemin: &Path, scelle: &[u8]) -> R<()> {
    let texte = serde_json::to_string_pretty(&json!({"format": FORMAT_CLE, "scelle": crate::retours::base64_ecrire(scelle)})).map_err(|e| e.to_string())?;
    let part = chemin.with_extension("bin.part");
    std::fs::write(&part, texte + "\n").map_err(|e| e.to_string())?;
    std::fs::rename(&part, chemin).map_err(|e| e.to_string())
}

/// La graine de la cle privee, ouverte par ce qui ouvre le coffre. Une cle scellee a l'ancienne s'ouvre par la cle de son
/// poste et, si le coffre a un mot de passe du logiciel, s'y rescelle par le coffre (verifiee avant d'etre ecrite) ; sinon
/// elle reste comme elle est.
fn cle_privee(coffre: &Coffre) -> R<Option<Vec<u8>>> {
    let p = coffre.racine.join("config").join(FICHIER_CLE_PRIVEE);
    if !p.exists() {
        return Ok(None);
    }
    let o = std::fs::read(&p).map_err(|e| e.to_string())?;
    if let Some(scelle) = scelle_par_le_coffre(&o) {
        return coffre.deproteger_par_le_coffre(&scelle, DESCRIPTION_CLE).map(Some).map_err(|e| e.message);
    }
    let graine = coffre.deproteger(&o, DESCRIPTION_CLE).map_err(|e| {
        format!(
            "{}\nCette cle de signature est scellee a l'ancienne, par la cle du poste qui l'a faite seule : elle ne s'ouvre que la-bas.",
            e.message
        )
    })?;
    if let Ok(neuf) = coffre.proteger_par_le_coffre(&graine, DESCRIPTION_CLE) {
        if coffre.deproteger_par_le_coffre(&neuf, DESCRIPTION_CLE).ok().as_deref() == Some(graine.as_slice()) {
            ecrire_cle(&p, &neuf)?;
        }
    }
    Ok(Some(graine))
}

/// Ce que `paie maj` dit d'une cle encore scellee a l'ancienne apres l'avoir ouverte, ou None.
fn reste_a_l_ancienne(racine: &Path) -> Option<String> {
    (cle_scellee_par_le_coffre(racine) == Some(false)).then(|| {
        "La cle de signature n'est scellee que par la cle de ce poste : perdue avec lui. Poser le mot de passe du logiciel \
         (`paie coffre ouverture`) ; elle se rescellera par le coffre a sa prochaine ouverture (`paie maj --cle`)."
            .to_string()
    })
}

/// La signature de `octets` par la cle privee de la compagnie de `coffre` (le poste, sa cle, ce qui l'ouvre).
pub fn signer_par_la_cle(coffre: &Coffre, octets: &[u8]) -> R<String> {
    let privee = cle_privee(coffre)?.ok_or(SANS_CLE)?;
    signer(octets, &privee)
}

pub(crate) fn exiger_editeur(racine: &Path, quoi: &str) -> R<()> {
    if crate::interface::compagnies::est_copie(racine) {
        return Err(format!("Copie de developpement : {} ne part jamais d'ici.", quoi));
    }
    if !reglage(&installation())?.1 {
        return Err("Ce poste n'est pas celui de l'editeur (le reglage de l'installation, maj/reglage.json -- ou, avant lui, prive/config/maj.json -- dit \"editeur\": true).".into());
    }
    Ok(())
}

/// Chez l'editeur : la paire de cles qui signe les archives. La privee, scellee par la cle maitresse du coffre (refuse
/// sans mot de passe du logiciel : elle serait perdue avec le poste) ; la publique, rendue, a reporter dans `CLE_PUBLIQUE`.
/// Une cle qui existe deja n'est jamais remplacee sans `remplacer` ; scellee a l'ancienne, elle est ouverte (et donc
/// rescellee par le coffre, `cle_privee`), et sa cle publique rendue.
pub fn generer_cle(racine: &Path, remplacer: bool) -> R<Value> {
    exiger_editeur(racine, "la cle de signature")?;
    generer_cle_au_poste(&Coffre::du_poste(racine, ""), remplacer)
}

/// `generer_cle`, le poste donne (`coffre` : la compagnie et la cle de ce poste). Le reglage de l'installation (« ce poste
/// est celui de l'editeur ») se regarde dans `generer_cle` ; ici, seule une copie est refusee.
pub fn generer_cle_au_poste(coffre: &Coffre, remplacer: bool) -> R<Value> {
    let racine = coffre.racine.as_path();
    if crate::interface::compagnies::est_copie(racine) {
        return Err("Copie de developpement : la cle de signature ne part jamais d'ici.".into());
    }
    let chemin = racine.join("config").join(FICHIER_CLE_PRIVEE);
    if chemin.exists() && !remplacer && cle_scellee_par_le_coffre(racine) == Some(false) {
        let graine = cle_privee(coffre)?.ok_or(SANS_CLE)?;
        let k = ed25519_dalek::SigningKey::from_bytes(&<[u8; 32]>::try_from(graine.as_slice()).map_err(|_| "cle privee illisible (32 octets attendus)")?);
        let publique = crate::retours::base64_ecrire(k.verifying_key().as_bytes());
        let mut r = json!({"chemin": chemin.display().to_string(), "cle_publique": publique, "correspond": publique == CLE_PUBLIQUE, "existante": true});
        if let Some(t) = reste_a_l_ancienne(racine) {
            r["a_l_ancienne"] = json!(t);
        }
        return Ok(r);
    }
    if chemin.exists() && !remplacer {
        return Err(format!(
            "Une cle privee existe deja ({}). --remplacer pour en faire une autre : les compagnies devront alors recevoir la nouvelle cle publique par une autre voie.",
            chemin.display()
        ));
    }
    let mut graine = [0u8; 32];
    getrandom::fill(&mut graine).map_err(|e| e.to_string())?;
    let k = ed25519_dalek::SigningKey::from_bytes(&graine);
    let publique = crate::retours::base64_ecrire(k.verifying_key().as_bytes());
    let scelle = coffre.proteger_par_le_coffre(&graine, DESCRIPTION_CLE).map_err(|e| e.message)?;
    ecrire_cle(&chemin, &scelle)?;
    Ok(json!({"chemin": chemin.display().to_string(), "cle_publique": publique, "correspond": publique == CLE_PUBLIQUE}))
}

/// Chez l'editeur : l'archive signee et `derniere.json` dans le depot (un dossier).
pub fn publier(racine: &Path, depot: Option<&str>, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    exiger_editeur(racine, "la publication d'une version")?;
    let depot = match depot {
        Some(d) => d.to_string(),
        None => reglage(&installation())?.0.ok_or("Aucun depot : le donner en argument ou dans le reglage de l'installation (maj/reglage.json).")?,
    };
    if depot.starts_with("http://") || depot.starts_with("https://") {
        return Err("publier ecrit dans un dossier ; pour un depot en ligne, y deposer ensuite les deux fichiers".into());
    }
    let privee = cle_privee(&Coffre::du_poste(racine, ""))?.ok_or(SANS_CLE)?;
    let mut r = publier_signe(&code(), Path::new(&depot), &privee, CLE_PUBLIQUE, notes, calcul, exigee)?;
    if let Some(t) = reste_a_l_ancienne(racine) {
        r["a_l_ancienne"] = json!(t);
    }
    Ok(r)
}

/// Le coeur de `publier`, le logiciel, la graine et la cle publique donnes : l'archive de `c`, signee et verifiee, et
/// `derniere.json` dans le dossier `d`. Le moteur livre n'y passe que par `publier` (la cle du poste, `CLE_PUBLIQUE`) ;
/// les essais de bout en bout y passent une cle d'essai (`tests/fichiers/mise_a_jour.rs`).
pub fn publier_signe(c: &Path, d: &Path, privee: &[u8], cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    let version = courante(c);
    if version == "0.0.0" {
        return Err(format!("pas de fichier VERSION dans {}", c.display()));
    }
    let depot = d.display().to_string();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    if d.join(DERNIERE).exists() {
        let avant = derniere(&depot)?;
        let av = avant["version"].as_str().unwrap_or_default();
        if !plus_recente(&version, av) {
            return Err(format!(
                "le depot est deja en {} ; augmenter VERSION (aujourd'hui {}, le fichier s'ecrit AAAA.MM.JJ : {}) avant de publier",
                version_lue(av),
                version_lue(&version),
                version
            ));
        }
    }
    let octets = archiver(c)?;
    let signature = signer(&octets, privee)?;
    if !signature_valide(&octets, &signature, cle_publique) {
        return Err("la cle privee de ce poste ne correspond pas a la cle publique du code : refaire `paie maj --cle` et reporter la cle publique".into());
    }
    let nom = format!("puppetpaie-{}.zip", version);
    std::fs::write(d.join(&nom), &octets).map_err(|e| e.to_string())?;
    let manifeste = json!({
        "version": version, "archive": nom,
        "sha256": Sha256::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect::<String>(),
        "publiee_le": crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..10].to_string(),
        "notes": notes, "calcul": calcul, "exigee": exigee, "signature": signature,
    });
    ecrire_json(&d.join(DERNIERE), &manifeste)?;
    let mut r = manifeste;
    r["octets"] = json!(octets.len());
    r["dossier"] = json!(d.display().to_string());
    Ok(r)
}

/// LA CLE DE GITHUB, fabriquee sur le poste de l'editeur (`paie maj --cle-github [DOSSIER]`, lot P1). Une cle neuve,
/// dediee : la GRAINE (base64) va dans un fichier, `cle-github-graine.txt`, que Leandre ouvre pour la coller dans le
/// secret `PUPPETPAIE_CLE_SIGNATURE` de l'environnement `publication`, puis efface ; elle ne passe jamais a l'ecran ni
/// dans un journal. La cle PUBLIQUE (pas secrete) va dans `cle-github-publique.txt` et dans ce qui est rendu : elle se
/// reporte dans `CLE_PUBLIQUE_GITHUB`. Refuse si la graine d'une cle precedente est encore la (rien n'est ecrase).
pub fn generer_cle_github(dossier: &Path) -> R<Value> {
    let fichier = dossier.join("cle-github-graine.txt");
    let fichier_publique = dossier.join("cle-github-publique.txt");
    if fichier.exists() {
        return Err(format!(
            "{} existe deja : le coller dans le secret GitHub s'il n'y est pas, puis l'effacer, avant d'en faire une autre",
            fichier.display()
        ));
    }
    std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    let mut graine = [0u8; 32];
    getrandom::fill(&mut graine).map_err(|e| e.to_string())?;
    let publique = crate::retours::base64_ecrire(ed25519_dalek::SigningKey::from_bytes(&graine).verifying_key().as_bytes());
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(&fichier).map_err(|e| format!("{} : {}", fichier.display(), e))?;
    f.write_all(crate::retours::base64_ecrire(&graine).as_bytes()).map_err(|e| e.to_string())?;
    graine.fill(0);
    std::fs::write(&fichier_publique, format!("{}\n", publique)).map_err(|e| e.to_string())?;
    Ok(json!({"graine": fichier.display().to_string(), "publique": fichier_publique.display().to_string(), "cle_publique": publique}))
}

/// `publier_signe` depuis une graine en base64 (celle du secret de GitHub), verifiee contre `cle_publique` : refuse si
/// la cle publique n'est pas reportee (vide) ou si la graine n'est pas la sienne. Aucun message ne porte la graine.
pub fn publier_par_graine(logiciel: &Path, sortie: &Path, graine_b64: &str, cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    if cle_publique.is_empty() {
        return Err("la cle publique de GitHub n'est pas reportee dans le code (paie/src/maj.rs, CLE_PUBLIQUE_GITHUB) : rien n'est publie".into());
    }
    let graine = b64(graine_b64.trim()).ok().filter(|g| g.len() == 32).ok_or_else(|| format!("{} illisible (32 octets en base64 attendus)", VARIABLE_GRAINE))?;
    publier_signe(logiciel, sortie, &graine, cle_publique, notes, calcul, exigee).map_err(|e| {
        if e.starts_with("la cle privee de ce poste") {
            format!("la graine de {} n'est pas celle de CLE_PUBLIQUE_GITHUB : rien n'est publie", VARIABLE_GRAINE)
        } else {
            e
        }
    })
}

/// LA PUBLICATION PAR LE FLUX DE GITHUB (`paie maj --publier-github --logiciel L --sortie S`, lot P1) : l'archive de `L`
/// signee par la graine de `PUPPETPAIE_CLE_SIGNATURE`, verifiee contre `CLE_PUBLIQUE_GITHUB`, et `derniere.json`, dans
/// `S`. Si `S` porte deja le `derniere.json` de la derniere Release (le flux l'y telecharge), une version qui n'est pas
/// plus recente est refusee (`publier_signe`). Ni le reglage « editeur » ni le coffre n'y jouent : le secret n'existe
/// que dans le travail approuve par Leandre.
pub fn publier_depuis_l_environnement(logiciel: &Path, sortie: &Path, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    let g = std::env::var(VARIABLE_GRAINE).ok().filter(|g| !g.trim().is_empty()).ok_or_else(|| format!("{} absente : rien n'est publie", VARIABLE_GRAINE))?;
    publier_par_graine(logiciel, sortie, &g, CLE_PUBLIQUE_GITHUB, notes, calcul, exigee)
}

/// AU LANCEMENT, avant toute fenetre : une version prete et que rien ne retient part au relais. Rend `true` si le relais
/// est parti (le programme doit alors s'arreter sans rien ouvrir). Ce qui echoue s'ecrit dans le journal de la mise a
/// jour, et l'ancienne version s'ouvre : une mise a jour ne doit jamais empecher une paie.
pub fn au_lancement() -> bool {
    let inst = installation();
    let Some(v) = applicable(&inst) else { return false };
    match lancer_relais(&inst, &v) {
        Ok(_) => true,
        Err(e) => {
            noter(&inst, &format!("Mise a jour {} non appliquee : {}", version_lue(&v), e));
            false
        }
    }
}

/// AU LANCEMENT, apres `au_lancement` : le logiciel cherche pour lui-meme, en fond, sans attendre qu'une compagnie
/// s'ouvre ; ce qu'il trouve s'appliquera au lancement suivant. Ce qui s'est passe va dans le journal de la mise a jour.
pub fn chercher_en_fond() {
    let inst = installation();
    std::thread::spawn(move || {
        let dit = verifier_avec(&inst, true, &cles_de_l_editeur());
        noter(&inst, &format!("Recherche : {}", dit));
    });
}

/// Le journal de la mise a jour (et de ce que le lancement range dans l'installation) : `<installation>/maj/journal.log`,
/// une ligne datee par evenement.
pub fn noter(inst: &Path, t: &str) {
    let d = dossier_maj(inst);
    let _ = std::fs::create_dir_all(&d);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("journal.log")) {
        let quand = crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif());
        let _ = writeln!(f, "{} {}", &quand[..quand.len().min(16)], t);
    }
}

/// Le programme neuf, copie hors de `logiciel/`, prend le relais ; il attendra la fin de CE processus. Rend `true` si le
/// relais est parti.
pub fn lancer_relais(inst: &Path, version: &str) -> R<bool> {
    let maj = dossier_maj(inst);
    let neuf = maj.join(version).join("logiciel");
    let source = programme(&neuf, "puppetpaie");
    if !source.is_file() {
        return Err(format!("le programme de la fenetre manque dans la version {} ({})", version, source.display()));
    }
    let relais = maj.join(format!("relais-{}{}", version, if cfg!(windows) { ".exe" } else { "" }));
    std::fs::copy(&source, &relais).map_err(|e| e.to_string())?;
    // La fenetre Windows compilee hors MSVC charge WebView2Loader.dll au demarrage : elle doit suivre le relais.
    let chargeur = dossier_programmes(&neuf).join("WebView2Loader.dll");
    if cfg!(windows) && chargeur.is_file() {
        std::fs::copy(&chargeur, maj.join("WebView2Loader.dll")).map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&relais, std::fs::Permissions::from_mode(0o755));
    }
    let mut c = std::process::Command::new(&relais);
    c.arg("maj-relais").arg("--installation").arg(inst).arg("--code").arg(logiciel_de(inst)).arg("--attendre").arg(std::process::id().to_string());
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map_err(|e| format!("le relais ne se lance pas : {}", e))?;
    Ok(true)
}

/// Attend la fin du processus `pid` (60 s au plus). Vrai s'il est parti.
#[cfg(windows)]
fn attendre_fin(pid: u32) -> bool {
    extern "system" {
        fn OpenProcess(acces: u32, heriter: i32, pid: u32) -> isize;
        fn WaitForSingleObject(objet: isize, millisecondes: u32) -> u32;
        fn CloseHandle(objet: isize) -> i32;
    }
    const SYNCHRONIZE: u32 = 0x0010_0000;
    // SAFETY : appels du noyau sur un numero de processus ; la poignee ouverte est refermee.
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if h == 0 {
            return true; // deja parti (ou jamais vu : rien a attendre)
        }
        let r = WaitForSingleObject(h, 60_000);
        CloseHandle(h);
        r == 0
    }
}

/// Hors Windows, la prise du verrou n'est jamais heritee (CLOEXEC) : quand elle se libere, la fenetre est partie.
#[cfg(not(windows))]
fn attendre_fin(_pid: u32) -> bool {
    for _ in 0..120 {
        if std::net::TcpListener::bind(("127.0.0.1", crate::interface::PORT_INSTANCE)).is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    false
}

/// Le relais (`puppetpaie maj-relais --installation I --code C --attendre PID`) : attend la fin du programme qui l'a
/// lance, bascule, et lance la fenetre (la neuve, ou l'ancienne si la bascule a echoue). Ecrit ce qui s'est passe dans le
/// journal de la mise a jour. Lance par une version d'avant le lot U1, il recoit `--racine R` (la compagnie dont `maj/`
/// porte la version prete) au lieu de `--installation` : il l'applique de la.
pub fn relais(args: &[String]) -> i32 {
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let Some(code) = valeur("--code") else { return 2 };
    let maj = match (valeur("--installation"), valeur("--racine")) {
        (Some(i), _) => dossier_maj(&i),
        (None, Some(r)) => dossier_maj(&r),
        _ => return 2,
    };
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let pid = valeur("--attendre").and_then(|p| p.to_str().and_then(|p| p.parse::<u32>().ok())).unwrap_or(0);
    if !attendre_fin(pid) {
        noter(&inst, "Mise a jour non appliquee : le programme qui l'a lancee ne s'est pas arrete.");
    } else {
        let journal = |t: &str| noter(&inst, t);
        match appliquer_depuis(&maj, &code, false) {
            Ok(r) => journal(&format!("Mise a jour appliquee : PuppetPaie {} (la precedente est gardee dans .avant/).", version_lue(r["version"].as_str().unwrap_or("?")))),
            Err(e) => journal(&format!("Mise a jour non appliquee : {}", e)),
        }
    }
    let _ = std::process::Command::new(programme(&code, "puppetpaie")).spawn();
    0
}

/// `paie maj [--sans-telecharger] [--appliquer [--malgre-calcul]] [--revenir] [--publier [DEPOT] --notes T --calcul T
/// --exigee T] [--cle [--remplacer]] [--cle-github [DOSSIER]] [--publier-github --logiciel L --sortie S --notes T
/// --calcul T --exigee T]`.
pub fn commande(args: &[String]) -> i32 {
    // D'abord savoir ou l'on est installe : sans cela, `installation()` retombe sur le dossier ou le programme a ete
    // compile (un chemin du conteneur, que Windows lit sur le lecteur courant : le 14/10/2026, la graine de `--cle-github`
    // est allee dans `D:\home\user\puppetpaieprivate\maj\`).
    crate::commandes::compagnie::fixer_le_code();
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned();
    // Les deux gestes de la publication par GitHub (lot P1) : ni compagnie ni coffre.
    let github = if drapeau("--cle-github") {
        let dossier = valeur("--cle-github").map(PathBuf::from).unwrap_or_else(|| dossier_maj(&installation()));
        Some(generer_cle_github(&dossier).map(|r| {
            format!(
                "Cle de GitHub fabriquee.\nLa GRAINE (secrete) est dans : {}\n  l'ouvrir, copier tout son contenu dans le secret PUPPETPAIE_CLE_SIGNATURE de l'environnement \
                 « publication » du depot public, puis EFFACER ce fichier.\nLa cle publique (pas secrete), aussi dans {} :\n  {}\n  a reporter dans paie/src/maj.rs, CLE_PUBLIQUE_GITHUB.",
                r["graine"].as_str().unwrap_or(""),
                r["publique"].as_str().unwrap_or(""),
                r["cle_publique"].as_str().unwrap_or("")
            )
        }))
    } else if drapeau("--publier-github") {
        match (valeur("--logiciel"), valeur("--sortie")) {
            (Some(l), Some(s)) => Some(
                publier_depuis_l_environnement(Path::new(&l), Path::new(&s), &valeur("--notes").unwrap_or_default(), &valeur("--calcul").unwrap_or_default(), &valeur("--exigee").unwrap_or_default())
                    .map(|r| format!("Signee : {} ({}, {} Ko) dans {}", r["version"].as_str().unwrap_or(""), r["archive"].as_str().unwrap_or(""), r["octets"].as_u64().unwrap_or(0) / 1024, r["dossier"].as_str().unwrap_or(""))),
            ),
            _ => Some(Err("--publier-github demande --logiciel DOSSIER et --sortie DOSSIER".into())),
        }
    } else {
        None
    };
    if let Some(r) = github {
        return match r {
            Ok(t) => {
                println!("{}", t);
                0
            }
            Err(e) => {
                println!("! {}", e);
                2
            }
        };
    }
    let racine = crate::routes_accueil::racine_du_poste();
    let r = if drapeau("--cle") {
        generer_cle(&racine, drapeau("--remplacer")).map(|r| {
            let ecrite = if r["existante"] == json!(true) {
                match r["a_l_ancienne"].as_str() {
                    Some(t) => format!("Cle privee existante, ouverte : {}\n{}", r["chemin"].as_str().unwrap_or(""), t),
                    None => format!(
                        "Cle privee existante, rescellee par le coffre (le mot de passe du logiciel l'ouvre sur un autre poste) : {}",
                        r["chemin"].as_str().unwrap_or("")
                    ),
                }
            } else {
                format!("Cle privee ecrite (scellee par le coffre : le mot de passe du logiciel l'ouvre sur un autre poste) : {}", r["chemin"].as_str().unwrap_or(""))
            };
            format!(
                "{}\nCle publique, a reporter dans paie/src/maj.rs, CLE_PUBLIQUE :\n  {}{}",
                ecrite,
                r["cle_publique"].as_str().unwrap_or(""),
                if r["correspond"] == json!(true) { "" } else { "\n  (le code porte une autre cle publique : tant qu'elle n'est pas reportee, `publier` refuse)" }
            )
        })
    } else if drapeau("--publier") {
        publier(&racine, valeur("--publier").as_deref(), &valeur("--notes").unwrap_or_default(), &valeur("--calcul").unwrap_or_default(), &valeur("--exigee").unwrap_or_default())
            .map(|r| {
                let t = format!("Publiee : {} ({}, {} Ko) dans {}", version_lue(r["version"].as_str().unwrap_or("")), r["archive"].as_str().unwrap_or(""), r["octets"].as_u64().unwrap_or(0) / 1024, r["dossier"].as_str().unwrap_or(""));
                match r["a_l_ancienne"].as_str() {
                    Some(a) => format!("{}\n{}", t, a),
                    None => t,
                }
            })
    } else if drapeau("--appliquer") {
        if std::net::TcpListener::bind(("127.0.0.1", crate::interface::PORT_INSTANCE)).is_err() {
            Err("PuppetPaie tourne : fermer la fenetre, puis appliquer".into())
        } else {
            appliquer(&installation(), drapeau("--malgre-calcul"))
                .map(|r| format!("Version {} en place ; la precedente est dans {}", version_lue(r["version"].as_str().unwrap_or("")), r["avant"].as_str().unwrap_or("")))
        }
    } else if drapeau("--revenir") {
        if std::net::TcpListener::bind(("127.0.0.1", crate::interface::PORT_INSTANCE)).is_err() {
            Err("PuppetPaie tourne : fermer la fenetre, puis revenir".into())
        } else {
            revenir(&installation()).map(|r| format!("Retour a la version {}", version_lue(r["version"].as_str().unwrap_or(""))))
        }
    } else {
        Ok(verifier(!drapeau("--sans-telecharger")))
    };
    match r {
        Ok(t) => {
            println!("{}", t);
            0
        }
        Err(e) => {
            println!("! {}", e);
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        /// Le chemin dont `deplacer` ou `copier` echoue UNE fois, pour l'essai de la bascule qui casse en route.
        pub(super) static ECHOUER_SUR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    }

    /// Vrai une seule fois pour le chemin pose dans `ECHOUER_SUR` (le retour en arriere, lui, doit pouvoir le deplacer).
    pub(super) fn echouer_sur(de: &Path) -> bool {
        ECHOUER_SUR.with(|e| {
            let oui = e.borrow().as_deref() == Some(de);
            if oui {
                e.borrow_mut().take();
            }
            oui
        })
    }

    fn arbre(d: &Path, version: &str) {
        std::fs::create_dir_all(d.join("donnees")).unwrap();
        std::fs::create_dir_all(d.join("interface")).unwrap();
        std::fs::write(d.join("VERSION"), format!("{}\n", version)).unwrap();
        std::fs::write(d.join("donnees").join("a.json"), version).unwrap();
        std::fs::write(d.join("interface").join("index.html"), version).unwrap();
    }

    /// Une archive qui porterait un `EDITION.json` (publiee avant le lot V1) ne change pas le bridage de l'installation :
    /// debloquee ou bridee, elle reste comme elle etait ; sans `EDITION.json`, elle n'en recoit pas.
    #[test]
    fn la_bascule_laisse_l_edition_de_l_installation_comme_elle_etait() {
        for avant in [Some("{\"defaut\": \"bride\"}"), Some("{\"defaut\": \"debloque\"}"), None] {
            let base = std::env::temp_dir().join(format!("puppetpaie-maj-edition-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let (code, racine) = (base.join("logiciel"), base.clone());
            arbre(&code, "2026.10.09");
            if let Some(t) = avant {
                std::fs::write(code.join("EDITION.json"), t).unwrap();
            }
            let neuf = racine.join("maj").join("2026.11.01").join("logiciel");
            arbre(&neuf, "2026.11.01");
            std::fs::write(neuf.join("EDITION.json"), "{\"edition\": \"bride\"}").unwrap();
            ecrire_json(&racine.join("maj").join("prete.json"), &json!({"version": "2026.11.01", "calcul": ""})).unwrap();
            appliquer(&racine, false).unwrap();
            assert_eq!(courante(&code), "2026.11.01");
            assert_eq!(std::fs::read_to_string(code.join("EDITION.json")).ok().as_deref(), avant);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    /// L'archive ne porte jamais l'`EDITION.json` du logiciel de l'editeur.
    #[test]
    fn l_archive_ne_porte_pas_l_edition() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-archive-edition-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        arbre(&base, "2026.11.01");
        std::fs::write(base.join("EDITION.json"), "{}").unwrap();
        std::fs::write(base.join("donnees").join("EDITION.json"), "{}").unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(archiver(&base).unwrap())).unwrap();
        let noms: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(!noms.contains(&"EDITION.json".to_string()) && noms.contains(&"VERSION".to_string()), "{:?}", noms);
        assert!(noms.contains(&"donnees/EDITION.json".to_string()), "seul celui de la racine du logiciel est ecarte");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// La bascule casse en route, au dernier pas (toute la version neuve est arrivee, l'`EDITION.json` de l'installation ne
    /// se remet pas) : TOUT revient. L'ancienne version est entiere a sa
    /// place, la neuve entiere dans `prive/maj/<version>/` (rien de melange), et l'`EDITION.json` de l'installation est la.
    #[test]
    fn une_bascule_qui_casse_en_route_remet_l_ancienne_entiere() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-bascule-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (code, racine) = (base.join("logiciel"), base.clone());
        arbre(&code, "2026.10.09");
        std::fs::write(code.join("EDITION.json"), "{\"defaut\": \"bride\"}").unwrap();
        let neuf = racine.join("maj").join("2026.11.01").join("logiciel");
        arbre(&neuf, "2026.11.01");
        ecrire_json(&racine.join("maj").join("prete.json"), &json!({"version": "2026.11.01", "calcul": ""})).unwrap();
        ECHOUER_SUR.with(|e| *e.borrow_mut() = Some(code.join(".avant").join("EDITION.json")));
        let r = appliquer(&racine, false);
        ECHOUER_SUR.with(|e| *e.borrow_mut() = None);
        assert!(r.unwrap_err().starts_with("la bascule a echoue"));
        assert_eq!(courante(&code), "2026.10.09");
        assert_eq!(std::fs::read_to_string(code.join("donnees").join("a.json")).unwrap(), "2026.10.09");
        assert_eq!(std::fs::read_to_string(code.join("interface").join("index.html")).unwrap(), "2026.10.09");
        assert!(code.join("EDITION.json").is_file());
        assert_eq!(courante(&neuf), "2026.11.01");
        assert_eq!(std::fs::read_to_string(neuf.join("donnees").join("a.json")).unwrap(), "2026.11.01");
        assert!(neuf.join("interface").join("index.html").is_file());
        assert!(prete(&racine).is_some(), "la version reste prete, pour le prochain demarrage");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// L'archive d'un logiciel, signee par une cle neuve, se verifie avec sa cle publique et se deballe a l'identique.
    #[test]
    fn archive_signee_deballee() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-{}", std::process::id()));
        let code = base.join("logiciel");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(code.join("donnees")).unwrap();
        std::fs::create_dir_all(code.join("target")).unwrap();
        std::fs::write(code.join("VERSION"), "2026.10.09\n").unwrap();
        std::fs::write(code.join("donnees").join("a.json"), "{}").unwrap();
        std::fs::write(code.join("target").join("gros"), "a ne pas livrer").unwrap();
        let octets = archiver(&code).unwrap();
        assert_eq!(octets, archiver(&code).unwrap(), "le zip est deterministe");
        let graine = [7u8; 32];
        let k = ed25519_dalek::SigningKey::from_bytes(&graine);
        let publique = crate::retours::base64_ecrire(k.verifying_key().as_bytes());
        let sig = signer(&octets, &graine).unwrap();
        assert!(signature_valide(&octets, &sig, &publique));
        assert!(!signature_valide(&octets, &sig, CLE_PUBLIQUE));
        let mut altere = octets.clone();
        let n = altere.len() / 2;
        altere[n] ^= 1;
        assert!(!signature_valide(&altere, &sig, &publique));
        let neuf = base.join("neuf");
        deballer(&octets, &neuf).unwrap();
        assert_eq!(courante(&neuf), "2026.10.09");
        assert!(neuf.join("donnees").join("a.json").is_file());
        assert!(!neuf.join("target").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// La version se lit JJ.MM.AAAA (Leandre, 14/10/2026) ; ce qui n'est pas une date passe tel quel.
    #[test]
    fn la_version_lue_commence_par_le_jour() {
        assert_eq!(version_lue("2026.10.09"), "09.10.2026");
        assert_eq!(version_lue(" 2026.11.01\n"), "01.11.2026");
        assert_eq!(version_lue("0.0.0"), "0.0.0");
        assert_eq!(version_lue("2026.10"), "2026.10");
        assert_eq!(version_lue("2026.10.14.2"), "14.10.2026 (2)");
        assert!(plus_recente("2026.10.14.2", "2026.10.14") && plus_recente("2026.10.15", "2026.10.14.2"));
        // L'identifiant garde l'ordre : la lecture ne sert jamais a comparer.
        assert!(plus_recente("2026.11.01", "2026.10.14") && version_lue("2026.11.01") < version_lue("2026.10.14"));
    }

    #[test]
    fn versions_comparees_en_entiers() {
        assert!(plus_recente("2026.10.10", "2026.10.9"));
        assert!(!plus_recente("2026.10.02", "2026.10.02"));
    }
}
