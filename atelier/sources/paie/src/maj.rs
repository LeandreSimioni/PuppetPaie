//! La mise a jour dans l'ombre (`moteur/maj.py`, lot J4) : verifier, appliquer, revenir ; et, chez l'editeur, publier.
//!
//!   verifier   a l'ouverture, en fond. Lit `derniere.json` au depot ; une version plus recente est prise, son condensat
//!              et la SIGNATURE de l'editeur (Ed25519, `CLE_PUBLIQUE`) verifies, deballee dans `prive/maj/<version>/`,
//!              CONTROLEE (son programme lit la configuration de la compagnie), puis `prete.json` est pose.
//!   appliquer  au demarrage suivant. Ce qui est dans `logiciel/` va dans `logiciel/.avant/`, la version prete prend sa
//!              place ; si ca casse en route, tout revient.
//!   publier    chez l'editeur : l'archive signee et `derniere.json` dans le depot.
//!
//! LE RELAIS, AU LANCEMENT (09/10/2026 : « que personne ne voit »). Sous Windows, un programme ne se remplace pas pendant
//! qu'il tourne. Le programme qu'on lance regarde donc, AVANT TOUTE FENETRE, si une compagnie du poste a une version
//! prete et applicable (`au_lancement`) : il copie le programme NEUF hors de `logiciel/` (`prive/maj/relais-<version>`),
//! le lance avec `maj-relais`, et s'arrete sans rien avoir montre. Le relais attend la fin de ce processus (par son
//! numero, pas par un port), bascule, lance la fenetre neuve, et s'arrete : on voit s'ouvrir directement la neuve.
//!
//! Deux garde-fous, comme le Python : une version qui change un CALCUL attend que chaque mois saisi ait sa DSN (sauf
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
/// Le depot des mises a jour QUAND `config/maj.json` NE DIT RIEN (`moteur/maj.py`, `DEPOT_PAR_DEFAUT`, vide jusqu'ici ;
/// lot S1, 13/10/2026) : `derniere.json` et l'archive signee, servis par le serveur du filet sous `/maj/`
/// (`prive/journal/DIAGNOSTIC-A-DISTANCE.md`, etape 6). Un `depot` pose le remplace toujours. Il ne sert qu'a RECEVOIR
/// (`verifier`) : `publier` ecrit dans un dossier, celui qu'on lui donne ou celui de `config/maj.json`. Le poste de
/// l'editeur ne recoit rien, et une copie (les demonstrations) ne cherche pas de mise a jour (`interface/veille.rs`).
pub const DEPOT_PAR_DEFAUT: &str = "https://filet.puppetpaie.fr/maj";
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

fn lire_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn ecrire_json(p: &Path, v: &Value) -> R<()> {
    crate::ecriture::ecrire_json(p, v, 1)
}

/// `prive/config/maj.json` : (depot pose, editeur). Le depot n'y est que s'il est pose ; `depot` dit celui qui sert.
pub fn configuration(racine: &Path) -> R<(Option<String>, bool)> {
    let p = racine.join("config").join("maj.json");
    if !p.exists() {
        return Ok((None, false));
    }
    let d = lire_json(&p).ok_or("config/maj.json illisible")?;
    let depot = d.get("depot").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
    Ok((depot, d.get("editeur").and_then(Value::as_bool).unwrap_or(false)))
}

/// Le depot d'ou cette compagnie recoit : celui qu'elle a pose, sinon `DEPOT_PAR_DEFAUT`.
pub fn depot(pose: Option<String>) -> String {
    pose.unwrap_or_else(|| DEPOT_PAR_DEFAUT.to_string())
}

fn dossier_maj(racine: &Path) -> PathBuf {
    racine.join("maj")
}

/// La version telechargee, controlee, a appliquer.
pub fn prete(racine: &Path) -> Option<Value> {
    let d = lire_json(&dossier_maj(racine).join("prete.json"))?;
    let v = d.get("version")?.as_str()?;
    dossier_maj(racine).join(v).join("logiciel").join("donnees").is_dir().then_some(d)
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

/// Les mois saisis sans DSN acceptee qui retiennent une version qui change un calcul (la route `maj-etat` les dit).
fn mois_qui_retiennent(racine: &Path) -> Vec<String> {
    let o = std::collections::HashMap::from([("racine".to_string(), racine.display().to_string())]);
    let Ok(ctx) = crate::service::contexte(&o) else { return Vec::new() };
    crate::routes_etats::route(&o, &ctx, "maj-etat", &Value::Null)
        .ok()
        .and_then(|v| v.get("attend").and_then(Value::as_array).cloned())
        .map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect())
        .unwrap_or_default()
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
        let _ = std::fs::set_permissions(cible.join("puppetpaie"), std::fs::Permissions::from_mode(0o755));
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

fn telecharger(racine: &Path, depot: &str, d: &Value) -> R<()> {
    let version = d["version"].as_str().unwrap_or_default();
    let octets = lire_depot(depot, d["archive"].as_str().unwrap_or_default())?;
    let sha: String = Sha256::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect();
    let attendu = d["sha256"].as_str().unwrap_or_default();
    if sha != attendu {
        return Err(format!("archive alteree (condensat {}… attendu {}…)", &sha[..12], attendu.chars().take(12).collect::<String>()));
    }
    let signature = d.get("signature").and_then(Value::as_str).ok_or("archive non signee : derniere.json n'a pas de « signature »")?;
    if !signature_valide(&octets, signature, CLE_PUBLIQUE) {
        return Err("la signature de l'archive n'est pas celle de l'editeur".into());
    }
    let cible = dossier_maj(racine).join(version);
    let _ = std::fs::remove_dir_all(&cible);
    let neuf = cible.join("logiciel");
    std::fs::create_dir_all(&neuf).map_err(|e| e.to_string())?;
    deballer(&octets, &neuf)?;
    if courante(&neuf) != version {
        return Err(format!("l'archive dit {}, derniere.json dit {}", courante(&neuf), version));
    }
    if let Err(p) = controler(&neuf, racine) {
        let _ = std::fs::remove_dir_all(&cible);
        return Err(format!("le nouveau code ne demarre pas : {}", p));
    }
    ecrire_json(
        &dossier_maj(racine).join("prete.json"),
        &json!({
            "version": version, "quand": crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string(),
            "notes": d.get("notes").cloned().unwrap_or(json!("")), "calcul": d.get("calcul").cloned().unwrap_or(json!("")),
            "exigee": d.get("exigee").cloned().unwrap_or(json!("")),
        }),
    )
}

/// A l'ouverture, en fond. Ne leve jamais : rend une phrase (ce qui s'est passe), pour le journal de la fenetre.
pub fn verifier(racine: &Path, telecharger_aussi: bool) -> String {
    let c = code();
    let cour = courante(&c);
    let (depot, editeur) = match configuration(racine) {
        Ok(x) => x,
        Err(e) => return format!("version {} ({})", cour, e),
    };
    if editeur {
        return format!("version {} (poste de l'editeur : pas de mise a jour automatique)", cour);
    }
    let depot = self::depot(depot);
    let d = match derniere(&depot) {
        Ok(d) => d,
        Err(e) => return format!("version {} (depot injoignable : {})", cour, e),
    };
    let v = d["version"].as_str().unwrap_or_default().to_string();
    if !plus_recente(&v, &cour) {
        return format!("version {}, a jour", cour);
    }
    if prete(racine).and_then(|p| p["version"].as_str().map(String::from)).as_deref() == Some(v.as_str()) || !telecharger_aussi {
        return format!("version {} -- la {} est prete", cour, v);
    }
    match telecharger(racine, &depot, &d) {
        Ok(()) => format!("version {} -- la {} est telechargee, controlee, prete : elle s'appliquera au prochain demarrage", cour, v),
        Err(e) => format!("version {} -- la {} existe mais n'a pas pu etre preparee : {}", cour, v, e),
    }
}

/// La version prete et applicable maintenant (rien ne la retient), ou `None`.
pub fn applicable(racine: &Path) -> Option<String> {
    let p = prete(racine)?;
    let v = p["version"].as_str()?.to_string();
    if !plus_recente(&v, &courante(&code())) || configuration(racine).map(|c| c.1).unwrap_or(true) {
        return None;
    }
    let force = p.get("malgre_calcul").and_then(Value::as_bool) == Some(true) || p.get("exigee").and_then(Value::as_str).is_some_and(|e| !e.is_empty());
    let calcul = p.get("calcul").and_then(Value::as_str).is_some_and(|c| !c.is_empty());
    if calcul && !force && !mois_qui_retiennent(racine).is_empty() {
        return None;
    }
    Some(v)
}

fn deplacer(de: &Path, vers: &Path) -> std::io::Result<()> {
    std::fs::rename(de, vers)
}

/// La bascule : `logiciel/` -> `logiciel/.avant/`, la version prete a sa place ; tout revient si ca casse.
pub fn appliquer(racine: &Path, code: &Path, malgre_calcul: bool) -> R<Value> {
    let d = prete(racine).ok_or("aucune version prete a appliquer")?;
    let version = d["version"].as_str().unwrap_or_default().to_string();
    let force = malgre_calcul || d.get("malgre_calcul").and_then(Value::as_bool) == Some(true) || d.get("exigee").and_then(Value::as_str).is_some_and(|e| !e.is_empty());
    if d.get("calcul").and_then(Value::as_str).is_some_and(|c| !c.is_empty()) && !force {
        let attend = mois_qui_retiennent(racine);
        if !attend.is_empty() {
            return Err(format!(
                "la {} change un calcul ({}) et le mois {} est saisi sans DSN acceptee : elle attend le depot, pour que la DSN dise ce que disent les bulletins. Pour l'appliquer quand meme : paie maj --appliquer --malgre-calcul",
                version,
                d["calcul"].as_str().unwrap_or_default(),
                attend.join(", ")
            ));
        }
    }
    let neuf = dossier_maj(racine).join(&version).join("logiciel");
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
    let mut faire = || -> R<()> {
        for p in entrees(code)? {
            let nom = p.file_name().unwrap_or_default().to_os_string();
            deplacer(&p, &avant.join(&nom)).map_err(|e| format!("{} : {}", p.display(), e))?;
            deplaces.push(nom);
        }
        for p in entrees(&neuf)? {
            let nom = p.file_name().unwrap_or_default().to_os_string();
            deplacer(&p, &code.join(&nom)).map_err(|e| format!("{} : {}", p.display(), e))?;
        }
        Ok(())
    };
    if let Err(e) = faire() {
        // Tout revient : ce qui est arrive repart, ce qui est parti revient.
        if let Ok(arrives) = entrees(code) {
            for p in arrives {
                let nom = p.file_name().unwrap_or_default().to_os_string();
                if !deplaces.contains(&nom) {
                    let _ = deplacer(&p, &neuf.join(&nom));
                }
            }
        }
        for nom in &deplaces {
            if avant.join(nom).exists() && !code.join(nom).exists() {
                let _ = deplacer(&avant.join(nom), &code.join(nom));
            }
        }
        return Err(format!("la bascule a echoue, l'ancienne version reste en place : {}", e));
    }
    let quand = crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string();
    ecrire_json(
        &dossier_maj(racine).join("appliquee.json"),
        &json!({"version": version, "quand": quand, "notes": d.get("notes").cloned().unwrap_or(json!("")), "calcul": d.get("calcul").cloned().unwrap_or(json!("")), "vue": false}),
    )?;
    let _ = std::fs::remove_file(dossier_maj(racine).join("prete.json"));
    let _ = std::fs::remove_dir_all(dossier_maj(racine).join(&version));
    Ok(json!({"version": version, "avant": avant.display().to_string(), "quand": quand}))
}

/// Reprendre la version d'avant, gardee dans `.avant/`. Une seule marche arriere.
pub fn revenir(racine: &Path, code: &Path) -> R<Value> {
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
    let _ = std::fs::remove_file(dossier_maj(racine).join("appliquee.json"));
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
        } else if !nom.ends_with(".pyc") && !nom.ends_with(".pyo") {
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
            if nom == "puppetpaie" { 0o755 } else { 0o644 },
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
    if !configuration(racine)?.1 {
        return Err("Ce poste n'est pas celui de l'editeur (config/maj.json, \"editeur\": true).".into());
    }
    Ok(())
}

/// Chez l'editeur : la paire de cles qui signe les archives. La privee, scellee par la cle maitresse du coffre (refuse
/// sans mot de passe du logiciel : elle serait perdue avec le poste) ; la publique, rendue, a reporter dans `CLE_PUBLIQUE`.
/// Une cle qui existe deja n'est jamais remplacee sans `remplacer` ; scellee a l'ancienne, elle est ouverte (et donc
/// rescellee par le coffre, `cle_privee`), et sa cle publique rendue.
pub fn generer_cle(racine: &Path, remplacer: bool) -> R<Value> {
    generer_cle_au_poste(&Coffre::du_poste(racine, ""), remplacer)
}

/// `generer_cle`, le poste donne (`coffre` : la compagnie et la cle de ce poste).
pub fn generer_cle_au_poste(coffre: &Coffre, remplacer: bool) -> R<Value> {
    let racine = coffre.racine.as_path();
    exiger_editeur(racine, "la cle de signature")?;
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
        None => configuration(racine)?.0.ok_or("Aucun depot : le donner en argument ou dans config/maj.json.")?,
    };
    if depot.starts_with("http://") || depot.starts_with("https://") {
        return Err("publier ecrit dans un dossier ; pour un depot en ligne, y deposer ensuite les deux fichiers".into());
    }
    let privee = cle_privee(&Coffre::du_poste(racine, ""))?.ok_or(SANS_CLE)?;
    let c = code();
    let version = courante(&c);
    if version == "0.0.0" {
        return Err(format!("pas de fichier VERSION dans {}", c.display()));
    }
    let d = PathBuf::from(&depot);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    if d.join(DERNIERE).exists() {
        let avant = derniere(&depot)?;
        let av = avant["version"].as_str().unwrap_or_default();
        if !plus_recente(&version, av) {
            return Err(format!("le depot est deja en {} ; augmenter VERSION ({}) avant de publier", av, version));
        }
    }
    let octets = archiver(&c)?;
    let signature = signer(&octets, &privee)?;
    if !signature_valide(&octets, &signature, CLE_PUBLIQUE) {
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
    if let Some(t) = reste_a_l_ancienne(racine) {
        r["a_l_ancienne"] = json!(t);
    }
    r["octets"] = json!(octets.len());
    r["dossier"] = json!(d.display().to_string());
    Ok(r)
}

/// Les compagnies du poste dont une version prete peut venir : les vraies (une fictive n'a pas de depot de mises a jour).
fn compagnies_du_poste() -> Vec<PathBuf> {
    let base = crate::interface::compagnies::base();
    let mut v = vec![crate::interface::compagnies::dossier_reel()];
    if let Ok(it) = std::fs::read_dir(base.join("compagnies")) {
        let mut autres: Vec<PathBuf> = it.flatten().map(|e| e.path()).filter(|d| d.join("config").is_dir()).collect();
        autres.sort();
        v.extend(autres);
    }
    v.retain(|r| r.join("config").is_dir() && !crate::interface::compagnies::est_copie(r));
    v
}

/// AU LANCEMENT, avant toute fenetre : une version prete et que rien ne retient part au relais. Rend `true` si le relais
/// est parti (le programme doit alors s'arreter sans rien ouvrir). Ce qui echoue s'ecrit dans le journal de la compagnie,
/// et l'ancienne version s'ouvre : une mise a jour ne doit jamais empecher une paie.
pub fn au_lancement() -> bool {
    for racine in compagnies_du_poste() {
        let Some(v) = applicable(&racine) else { continue };
        match lancer_relais(&racine, &v) {
            Ok(_) => return true,
            Err(e) => journal(&racine, &format!("Mise a jour {} non appliquee : {}", v, e)),
        }
    }
    false
}

fn journal(racine: &Path, t: &str) {
    let _ = std::fs::create_dir_all(racine.join("journal"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(racine.join("journal").join("interface.log")) {
        let _ = writeln!(f, "{}", t);
    }
}

/// Le programme neuf, copie hors de `logiciel/`, prend le relais ; il attendra la fin de CE processus. Rend `true` si le
/// relais est parti.
pub fn lancer_relais(racine: &Path, version: &str) -> R<bool> {
    let neuf = dossier_maj(racine).join(version).join("logiciel");
    let source = programme(&neuf, "puppetpaie");
    if !source.is_file() {
        return Err(format!("le programme de la fenetre manque dans la version {} ({})", version, source.display()));
    }
    let relais = dossier_maj(racine).join(format!("relais-{}{}", version, if cfg!(windows) { ".exe" } else { "" }));
    std::fs::copy(&source, &relais).map_err(|e| e.to_string())?;
    // La fenetre Windows compilee hors MSVC charge WebView2Loader.dll au demarrage : elle doit suivre le relais.
    let chargeur = dossier_programmes(&neuf).join("WebView2Loader.dll");
    if cfg!(windows) && chargeur.is_file() {
        std::fs::copy(&chargeur, dossier_maj(racine).join("WebView2Loader.dll")).map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&relais, std::fs::Permissions::from_mode(0o755));
    }
    let mut c = std::process::Command::new(&relais);
    c.arg("maj-relais").arg("--racine").arg(racine).arg("--code").arg(code()).arg("--attendre").arg(std::process::id().to_string());
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

/// Le relais (`puppetpaie maj-relais --racine R --code C --attendre PID`) : attend la fin du programme qui l'a lance,
/// bascule, et lance la fenetre (la neuve, ou l'ancienne si la bascule a echoue). Ecrit ce qui s'est passe dans le journal.
pub fn relais(args: &[String]) -> i32 {
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let (Some(racine), Some(code)) = (valeur("--racine"), valeur("--code")) else { return 2 };
    let pid = valeur("--attendre").and_then(|p| p.to_str().and_then(|p| p.parse::<u32>().ok())).unwrap_or(0);
    if !attendre_fin(pid) {
        journal(&racine, "Mise a jour non appliquee : le programme qui l'a lancee ne s'est pas arrete.");
    } else {
        let journal = |t: &str| journal(&racine, t);
        match appliquer(&racine, &code, false) {
            Ok(r) => journal(&format!("Mise a jour appliquee : PuppetPaie {} (la precedente est gardee dans .avant/).", r["version"].as_str().unwrap_or("?"))),
            Err(e) => journal(&format!("Mise a jour non appliquee : {}", e)),
        }
    }
    let _ = std::process::Command::new(programme(&code, "puppetpaie")).spawn();
    0
}

/// `paie maj [--sans-telecharger] [--appliquer [--malgre-calcul]] [--revenir] [--publier [DEPOT] --notes T --calcul T
/// --exigee T] [--cle [--remplacer]]`.
pub fn commande(args: &[String]) -> i32 {
    crate::commandes::compagnie::fixer_le_code();
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned();
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
                let t = format!("Publiee : {} ({}, {} Ko) dans {}", r["version"].as_str().unwrap_or(""), r["archive"].as_str().unwrap_or(""), r["octets"].as_u64().unwrap_or(0) / 1024, r["dossier"].as_str().unwrap_or(""));
                match r["a_l_ancienne"].as_str() {
                    Some(a) => format!("{}\n{}", t, a),
                    None => t,
                }
            })
    } else if drapeau("--appliquer") {
        if std::net::TcpListener::bind(("127.0.0.1", crate::interface::PORT_INSTANCE)).is_err() {
            Err("PuppetPaie tourne : fermer la fenetre, puis appliquer".into())
        } else {
            appliquer(&racine, &code(), drapeau("--malgre-calcul"))
                .map(|r| format!("Version {} en place ; la precedente est dans {}", r["version"].as_str().unwrap_or(""), r["avant"].as_str().unwrap_or("")))
        }
    } else if drapeau("--revenir") {
        if std::net::TcpListener::bind(("127.0.0.1", crate::interface::PORT_INSTANCE)).is_err() {
            Err("PuppetPaie tourne : fermer la fenetre, puis revenir".into())
        } else {
            revenir(&racine, &code()).map(|r| format!("Retour a la version {}", r["version"].as_str().unwrap_or("")))
        }
    } else {
        Ok(verifier(&racine, !drapeau("--sans-telecharger")))
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

    #[test]
    fn versions_comparees_en_entiers() {
        assert!(plus_recente("2026.10.10", "2026.10.9"));
        assert!(!plus_recente("2026.10.02", "2026.10.02"));
    }
}
