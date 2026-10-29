//! LA PUBLICATION D'UNE VERSION, chez l'editeur : les archives signees et `derniere.json`. Le flux « Publier » du depot
//! public (`.github/workflows/publier.yml`) fabrique `logiciel/`, attend l'approbation de Leandre, signe par
//! `publier_depuis_l_environnement` et pose les archives, `derniere.json` et l'installeur dans une Release, que
//! `maj::DEPOT_PAR_DEFAUT` lit. Ce que les installations recoivent, verifient et appliquent est dans `compagnie::maj` ;
//! la forme des archives y est ecrite en tete : c'est un contrat avec toutes les installations posees.
//!
//! LA CLE DE SIGNATURE DU POSTE DE L'EDITEUR (`config/maj-cle-privee.bin`). Sa graine est scellee par la CLE MAITRESSE
//! DU COFFRE (`Coffre::proteger_par_le_coffre`), dans un petit JSON (`FORMAT_CLE`) : elle s'ouvre partout ou le coffre
//! s'ouvre, par la cle du poste, ou ailleurs par le mot de passe du logiciel (tape, ou la seance). Elle depend donc du
//! coffre qui l'a scellee : sans lui, rien ne l'ouvre, et un coffre neuf ne l'ouvrira pas. UNE CLE SCELLEE PAR LA CLE DU
//! POSTE SEULE s'ouvre encore sur le poste qui l'a faite, et s'y RESCELLE par le coffre a cette ouverture (`cle_privee`) :
//! c'est la seule facon de la sauver sans changer la paire, donc sans faire parvenir une nouvelle cle publique a chaque
//! compagnie. `moteur maj --cle` fait cette ouverture sans publier.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::compagnie::coffre::Coffre;
use crate::compagnie::maj::{
    b64, code, condensat, courante, ecrire_json, installation, lire_depot, nom_d_archive, nom_d_installation, plus_recente, programme_de, reglage,
    signature_valide, systeme_du_fichier, version_lue, CLE_PUBLIQUE, CLE_PUBLIQUE_GITHUB, DERNIERE, LIVRAISON, LIVREE, LOGICIEL,
    PROPRES_A_L_INSTALLATION, SYSTEMES, TOUT_SYSTEME,
};

type R<T> = Result<T, String>;

/// La variable d'environnement ou le flux « Publier » pose la graine (base64) : lue par `publier_depuis_l_environnement`,
/// jamais affichee.
pub const VARIABLE_GRAINE: &str = "PUPPETPAIE_CLE_SIGNATURE";
const FICHIER_CLE_PRIVEE: &str = "maj-cle-privee.bin";
const DESCRIPTION_CLE: &str = "PuppetPaie : cle de signature des mises a jour";
/// Ce que porte `maj-cle-privee.bin` depuis le lot K1 : `{"format": FORMAT_CLE, "scelle": <base64>}`, la graine scellee par
/// la cle maitresse du coffre. Tout autre contenu est une cle scellee a l'ancienne, par la cle du poste seule.
const FORMAT_CLE: &str = "puppetpaie/maj-cle/scellee-par-le-coffre";
const SANS_CLE: &str = "pas de cle privee : les compagnies refuseraient une archive non signee (moteur maj --cle pour la fabriquer)";
/// Ce qui ne part jamais dans une archive, ni ne bouge a la bascule.
const EXCLUS: [&str; 6] = [".git", ".avant", ".apres", "__pycache__", ".pytest_cache", "target"];
/// Les programmes d'un systeme Unix, dans son dossier de `logiciel/` (`linux/`, `mac/`) : la fenetre, et le moteur (pour un
/// Linux sans WebKitGTK, qui ouvre la meme page dans le navigateur ; et le flux « Publier » signe avec celui de Linux).
/// Un zip ne garde le bit d'execution que si on l'y marque : `archiver` le marque sur eux, `deballer` le remet.
const PROGRAMMES_UNIX: [&str; 2] = ["puppetpaie", "moteur"];

pub(crate) fn signer(octets: &[u8], privee: &[u8]) -> R<String> {
    use ed25519_dalek::Signer;
    let k = <[u8; 32]>::try_from(privee).map_err(|_| "cle privee illisible (32 octets attendus)")?;
    Ok(socle::base64::ecrire(&ed25519_dalek::SigningKey::from_bytes(&k).sign(octets).to_bytes()))
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

/// Ce qu'une compagnie fictive porte sur un poste sans que ce soit la compagnie livree : ses secrets et son jeton (une
/// fictive ouverte pose elle aussi un coffre), le journal de la fenetre. Ni l'archive ni le depot ne les portent (les memes
/// exclusions que le `.gitignore` du depot public, « LES COMPAGNIES FICTIVES »).
/// La marque d'une livraison (`LIVREE`) non plus : la livraison l'ajoute elle-meme.
const HORS_D_UNE_FICTIVE_LIVREE: [&str; 6] =
    ["config/coffre.bin", "config/google-jeton.bin", "config/maj-cle-privee.bin", "config/.jeton-interface", "journal/interface.log", LIVREE];

/// LES COMPAGNIES FICTIVES LIVREES, a cote du logiciel (`<code>/../fictives/`) : les fichiers que porte l'archive
/// d'installation (`archiver_installation`), chemins relatifs a `fictives/`, dans l'ordre des noms. Une fictive est un dossier que son marqueur dit fictif
/// (`compagnies::est_fictive`) ; ce qui commence par un point (`.origine/`, les photos d'origine du poste, et les dossiers
/// d'une remise a zero en cours) n'en est pas. Sans aucune fictive, une archive decompressee ouvrirait un accueil vide
/// (le logiciel y est bride, ticket #246) : refus.
fn fictives_livrees(code: &Path) -> R<Vec<PathBuf>> {
    let dossier = code.parent().unwrap_or(code).join(crate::interface::compagnies::DOSSIER_FICTIVES);
    let mut v: Vec<_> = std::fs::read_dir(&dossier).map(|it| it.flatten().collect()).unwrap_or_default();
    v.sort_by_key(|e| e.file_name());
    let mut out = Vec::new();
    let mut n = 0;
    for e in v {
        let nom = e.file_name().to_string_lossy().into_owned();
        if nom.starts_with('.') || EXCLUS.contains(&nom.as_str()) {
            continue;
        }
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            out.push(PathBuf::from(&nom));
            continue;
        }
        // Une fictive AJOUTEE sur le poste (ticket #236) porte les donnees qu'on y a mises : elle ne se livre jamais.
        if !crate::interface::compagnies::est_fictive_livree(&e.path()) {
            continue;
        }
        n += 1;
        let mut dans = Vec::new();
        fichiers(&e.path(), Path::new(""), &mut dans);
        for r in dans {
            let rs = r.to_string_lossy().replace('\\', "/");
            if !HORS_D_UNE_FICTIVE_LIVREE.contains(&rs.as_str()) {
                out.push(Path::new(&nom).join(r));
            }
        }
    }
    if n == 0 {
        return Err(format!(
            "aucune compagnie fictive dans {} : une archive sans elles s'ouvrirait bridee sur un accueil vide",
            dossier.display()
        ));
    }
    Ok(out)
}

/// Une entree d'archive : son nom dans le zip, le fichier qu'elle porte, et si c'est un programme Unix (executable).
type Entree = (String, PathBuf, bool);

/// Les entrees du logiciel (`code`) pour un systeme (`SYSTEMES`) ou tous (`TOUT_SYSTEME`), chacune nommee `<tete><chemin>` :
/// le contenu de `logiciel/`, sans caches ni depot git ni version d'avant ; pour un systeme, les fichiers communs et les
/// siens seulement (`systeme_du_fichier`). Les programmes Unix (`linux/puppetpaie`, `mac/moteur`...) sont marques
/// executables, et eux seuls.
fn entrees_du_logiciel(code: &Path, systeme: &str, tete: &str) -> R<Vec<Entree>> {
    if systeme != TOUT_SYSTEME && !SYSTEMES.contains(&systeme) {
        return Err(format!("« {} » n'est pas un systeme", systeme));
    }
    let mut liste = Vec::new();
    fichiers(code, Path::new(""), &mut liste);
    let mut out = Vec::new();
    for rel in liste {
        let nom = rel.to_string_lossy().replace('\\', "/");
        let du_systeme = systeme_du_fichier(&nom);
        if systeme != TOUT_SYSTEME && du_systeme.is_some_and(|s| s != systeme) {
            continue;
        }
        let programme_unix = du_systeme.is_some_and(|s| s != "windows") && nom.rsplit('/').next().is_some_and(|n| PROGRAMMES_UNIX.contains(&n));
        out.push((format!("{}{}", tete, nom), code.join(&rel), programme_unix));
    }
    Ok(out)
}

/// Ce qui se livre des compagnies fictives (`fictives_livrees`) : les entrees nommees `fictives/<chemin>` ; les fichiers
/// que la livraison ajoute (la marque `.livree.json` de chaque fictive, et `.livraison.json`) ; et L'EMPREINTE de toute
/// la livraison. Une empreinte est le SHA-256 des lignes `<nom>\t<SHA-256 du fichier>`, dans l'ordre des entrees (pour
/// une fictive, ses seuls fichiers, nommes depuis son dossier) : elle ne depend que des fichiers livres et de leurs noms,
/// ni de la compression ni des dates, et deux publications aux memes fictives ont la meme.
fn entrees_des_fictives(code: &Path) -> R<(Vec<Entree>, Vec<(String, Vec<u8>)>, String)> {
    let racine = crate::interface::compagnies::DOSSIER_FICTIVES;
    let dossier = code.parent().unwrap_or(code).join(racine);
    let hex = |h: Sha256| h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let mut entrees = Vec::new();
    let mut toute = Sha256::new();
    let mut par_fictive: std::collections::BTreeMap<String, Sha256> = std::collections::BTreeMap::new();
    for rel in fictives_livrees(code)? {
        let r = rel.to_string_lossy().replace('\\', "/");
        let chemin = dossier.join(&rel);
        let sha = condensat(&std::fs::read(&chemin).map_err(|e| format!("{} : {}", chemin.display(), e))?);
        toute.update(format!("{}/{}\t{}\n", racine, r, sha).as_bytes());
        if let Some((fictive, dedans)) = r.split_once('/') {
            par_fictive.entry(fictive.to_string()).or_default().update(format!("{}\t{}\n", dedans, sha).as_bytes());
        }
        entrees.push((format!("{}/{}", racine, r), chemin, false));
    }
    let empreinte = hex(toute);
    let mut en_plus = Vec::new();
    let mut posees = serde_json::Map::new();
    for (nom, h) in par_fictive {
        let e = hex(h);
        en_plus.push((format!("{}/{}/{}", racine, nom, LIVREE), format!("{{\"empreinte\": \"{}\"}}\n", e).into_bytes()));
        posees.insert(nom, json!(e));
    }
    let l = serde_json::to_vec_pretty(&json!({"empreinte": empreinte, "posees": posees})).map_err(|e| e.to_string())?;
    en_plus.push((format!("{}/{}", racine, LIVRAISON), l));
    Ok((entrees, en_plus, empreinte))
}

/// Un zip deterministe (dates fixes, ordre donne) : les `entrees`, puis les fichiers `en_plus` (nom, octets).
fn zipper(entrees: &[Entree], en_plus: &[(String, Vec<u8>)]) -> R<Vec<u8>> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let date = zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0).map_err(|e| format!("{:?}", e))?;
    let options = |executable: bool| {
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(date)
            .unix_permissions(if executable { 0o755 } else { 0o644 })
    };
    for (nom, chemin, executable) in entrees {
        z.start_file(nom.as_str(), options(*executable)).map_err(|e| e.to_string())?;
        z.write_all(&std::fs::read(chemin).map_err(|e| format!("{} : {}", chemin.display(), e))?).map_err(|e| e.to_string())?;
    }
    for (nom, octets) in en_plus {
        z.start_file(nom.as_str(), options(false)).map_err(|e| e.to_string())?;
        z.write_all(octets).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

/// L'ARCHIVE DE MISE A JOUR d'un systeme (`SYSTEMES`) ou de tous (`TOUT_SYSTEME`) : LE CONTENU DE `logiciel/` A SA RACINE,
/// sans `fictives/`. Sa forme est un contrat avec toutes les installations posees (la tete de ce fichier) : elle ne change
/// plus. C'est elle que `derniere.json` nomme sous `archives`, que l'editeur signe, que `telecharger` prend.
pub fn archiver(code: &Path, systeme: &str) -> R<Vec<u8>> {
    zipper(&entrees_du_logiciel(code, systeme, "")?, &[])
}

/// L'ARCHIVE DES FICTIVES (`puppetpaie-<version>-fictives.zip`) : `fictives/`, ses compagnies fictives livrees, la marque
/// de chacune et `.livraison.json` (`entrees_des_fictives`), et l'empreinte de la livraison. Signee comme les archives de
/// mise a jour, nommee dans `derniere.json` sous `fictives` ; une installation ne la telecharge que si l'empreinte n'est
/// pas celle qu'elle retient (`preparer_les_fictives`).
pub fn archiver_fictives(code: &Path) -> R<(Vec<u8>, String)> {
    let (entrees, en_plus, empreinte) = entrees_des_fictives(code)?;
    Ok((zipper(&entrees, &en_plus)?, empreinte))
}

/// L'ARCHIVE D'INSTALLATION d'un systeme ou de tous (ticket #246) : `logiciel/` (ce que porte l'archive de mise a jour,
/// sous `logiciel/`) et, a cote, `fictives/` (exactement ce que porte l'archive des fictives : une installation neuve
/// retient la livraison qu'elle a, et ne la retelecharge pas). Decompressee dans un dossier neuf, elle donne ce que pose
/// l'installeur Windows : une installation bridee, avec ses quatre fictives. Elle ne sert qu'a installer : aucune
/// installation ne la telecharge, et `deballer` la refuse comme mise a jour. Elle pourrait se composer des deux autres
/// (les decompresser l'une dans `logiciel/`, l'autre a cote) ; elle reste une archive, pour qu'installer soit telecharger
/// un fichier et le decompresser.
pub fn archiver_installation(code: &Path, systeme: &str) -> R<Vec<u8>> {
    let mut entrees = entrees_du_logiciel(code, systeme, &format!("{}/", LOGICIEL))?;
    let (fictives, en_plus, _) = entrees_des_fictives(code)?;
    entrees.extend(fictives);
    zipper(&entrees, &en_plus)
}

/// Le nom de l'archive des fictives d'une version : `puppetpaie-<version>-fictives.zip`.
pub fn nom_des_fictives(version: &str) -> String {
    format!("puppetpaie-{}-fictives.zip", version)
}

/// LES MARQUES DE LA LIVRAISON, posees dans les fictives livrees a cote de `code` (`moteur maj --marquer-fictives --logiciel
/// L`) : `fictives/.livraison.json` et `fictives/<nom>/.livree.json`, exactement ce que porte l'archive des fictives. Le
/// flux « Publier » les pose avant de fabriquer l'installeur Windows, qui les pose a son tour : une installation de
/// l'installeur retient sa livraison comme une installation de l'archive, et ne retelecharge ni ne remplace des fictives
/// qui n'ont pas change. Rien de ce que porte une livraison n'en depend (`fictives_livrees` ne les livre pas). Rend
/// l'empreinte.
pub fn marquer_les_fictives(code: &Path) -> R<String> {
    let (_, en_plus, empreinte) = entrees_des_fictives(code)?;
    let base = code.parent().unwrap_or(code);
    for (nom, octets) in en_plus {
        std::fs::write(base.join(&nom), octets).map_err(|e| format!("{} : {}", nom, e))?;
    }
    Ok(empreinte)
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
    let texte = serde_json::to_string_pretty(&json!({"format": FORMAT_CLE, "scelle": socle::base64::ecrire(scelle)})).map_err(|e| e.to_string())?;
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

/// Ce que `moteur maj` dit d'une cle encore scellee a l'ancienne apres l'avoir ouverte, ou None.
fn reste_a_l_ancienne(racine: &Path) -> Option<String> {
    (cle_scellee_par_le_coffre(racine) == Some(false)).then(|| {
        "La cle de signature n'est scellee que par la cle de ce poste : perdue avec lui. Poser le mot de passe du logiciel \
         (`moteur coffre ouverture`) ; elle se rescellera par le coffre a sa prochaine ouverture (`moteur maj --cle`)."
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
        let publique = socle::base64::ecrire(k.verifying_key().as_bytes());
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
    let publique = socle::base64::ecrire(k.verifying_key().as_bytes());
    let scelle = coffre.proteger_par_le_coffre(&graine, DESCRIPTION_CLE).map_err(|e| e.message)?;
    ecrire_cle(&chemin, &scelle)?;
    Ok(json!({"chemin": chemin.display().to_string(), "cle_publique": publique, "correspond": publique == CLE_PUBLIQUE}))
}

/// Chez l'editeur : les archives signees et `derniere.json` dans le depot (un dossier).
pub fn publier(racine: &Path, depot: Option<&str>, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    exiger_editeur(racine, "la publication d'une version")?;
    let depot = match depot {
        Some(d) => d.to_string(),
        None => reglage(&installation())?.0.ok_or("Aucun depot : le donner en argument ou dans le reglage de l'installation (maj/reglage.json).")?,
    };
    if depot.starts_with("http://") || depot.starts_with("https://") {
        return Err("publier ecrit dans un dossier ; pour un depot en ligne, y deposer ensuite les archives et derniere.json".into());
    }
    let privee = cle_privee(&Coffre::du_poste(racine, ""))?.ok_or(SANS_CLE)?;
    let mut r = publier_signe(&code(), Path::new(&depot), &privee, CLE_PUBLIQUE, notes, calcul, exigee)?;
    if let Some(t) = reste_a_l_ancienne(racine) {
        r["a_l_ancienne"] = json!(t);
    }
    Ok(r)
}

/// Le coeur de `publier`, le logiciel, la graine et la cle publique donnes : les archives de `c` (une par systeme dont la
/// fenetre y est, et celle de tous les systemes, qui porte ceux-la et le dit sous `systemes`), chacune signee et verifiee,
/// et `derniere.json` qui les nomme, dans le dossier `d` ; a cote, les archives d'installation (Linux, Mac, tous), ni
/// signees ni nommees, rendues sous `installations`. Le moteur livre n'y passe que par `publier` (la cle du poste, `CLE_PUBLIQUE`) ;
/// les essais de bout en bout y passent une cle d'essai (`tests/fichiers/mise_a_jour.rs`).
pub fn publier_signe(c: &Path, d: &Path, privee: &[u8], cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    let version = courante(c);
    if version == "0.0.0" {
        return Err(format!("pas de fichier VERSION dans {}", c.display()));
    }
    let depot = d.display().to_string();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    if d.join(DERNIERE).exists() {
        // Seule sa version compte ici : le `derniere.json` de la Release d'avant peut etre d'une forme plus ancienne (une
        // seule archive, avant le 15/10/2026), que `derniere` refuserait a juste titre chez celui qui recoit.
        let avant: Value = serde_json::from_slice(&lire_depot(&depot, DERNIERE)?).map_err(|e| format!("derniere.json illisible : {}", e))?;
        let av = avant["version"].as_str().filter(|v| !v.is_empty()).ok_or("le derniere.json deja la n'a pas de « version »")?;
        if !plus_recente(&version, av) {
            return Err(format!(
                "le depot est deja en {} ; augmenter VERSION (aujourd'hui {}, le fichier s'ecrit AAAA.MM.JJ : {}) avant de publier",
                version_lue(av),
                version_lue(&version),
                version
            ));
        }
    }
    // Une archive par systeme dont la fenetre est dans `logiciel/` (le flux les pose toutes trois ; un poste qui publie
    // depuis son propre `logiciel/` n'a que la sienne), et celle de tous les systemes. Sans aucune fenetre, aucune
    // installation ne pourrait lancer la version : rien n'est publie.
    let presents: Vec<&str> = SYSTEMES.into_iter().filter(|s| programme_de(c, s, "puppetpaie").is_file()).collect();
    if presents.is_empty() {
        return Err(format!(
            "aucune fenetre dans {} (ni {}) : aucune installation ne pourrait lancer cette version",
            c.display(),
            SYSTEMES.map(|s| programme_de(Path::new(""), s, "puppetpaie").to_string_lossy().replace('\\', "/")).join(", ni ")
        ));
    }
    let mut archives = serde_json::Map::new();
    let mut octets_de = serde_json::Map::new();
    let mut faites: Vec<(String, Vec<u8>)> = Vec::new();
    for systeme in presents.iter().copied().chain([TOUT_SYSTEME]) {
        let octets = archiver(c, systeme)?;
        let signature = signer(&octets, privee)?;
        if !signature_valide(&octets, &signature, cle_publique) {
            return Err("la cle privee de ce poste ne correspond pas a la cle publique du code : refaire `moteur maj --cle` et reporter la cle publique".into());
        }
        let nom = nom_d_archive(&version, systeme);
        let mut a = json!({"archive": nom, "sha256": condensat(&octets), "signature": signature});
        if systeme == TOUT_SYSTEME {
            a["systemes"] = json!(presents);
        }
        archives.insert(systeme.to_string(), a);
        octets_de.insert(systeme.to_string(), json!(octets.len()));
        faites.push((nom, octets));
    }
    // LES ARCHIVES D'INSTALLATION, a cote (`archiver_installation`) : Linux et Mac s'ils sont publies, et tous les
    // systemes (le dossier portable) ; Windows a son installeur. Ni signees ni nommees dans `derniere.json` : aucune
    // installation ne les telecharge.
    let mut installations = serde_json::Map::new();
    for systeme in presents.iter().copied().filter(|s| *s != "windows").chain([TOUT_SYSTEME]) {
        let octets = archiver_installation(c, systeme)?;
        let nom = nom_d_installation(&version, systeme);
        installations.insert(systeme.to_string(), json!({"archive": nom, "sha256": condensat(&octets), "octets": octets.len()}));
        faites.push((nom, octets));
    }
    // LES FICTIVES, a part (`archiver_fictives`) : signees comme les archives, nommees sous `fictives` avec leur empreinte.
    let (octets, empreinte) = archiver_fictives(c)?;
    let signature = signer(&octets, privee)?;
    if !signature_valide(&octets, &signature, cle_publique) {
        return Err("la cle privee de ce poste ne correspond pas a la cle publique du code : refaire `moteur maj --cle` et reporter la cle publique".into());
    }
    let nom = nom_des_fictives(&version);
    let fictives = json!({"archive": nom, "sha256": condensat(&octets), "signature": signature, "empreinte": empreinte});
    octets_de.insert("fictives".into(), json!(octets.len()));
    faites.push((nom, octets));
    // Rien n'est ecrit avant que toutes soient signees et verifiees.
    for (nom, octets) in &faites {
        std::fs::write(d.join(nom), octets).map_err(|e| e.to_string())?;
    }
    let mut manifeste = json!({
        "version": version,
        "publiee_le": socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..10].to_string(),
        "notes": notes, "calcul": calcul, "exigee": exigee,
        "archives": archives,
        "fictives": fictives,
    });
    // LES INSTALLATIONS D'AVANT LE 15/10/2026 (une seule archive, celle de Windows : v2026.10.14.2, v2026.10.15) lisent a
    // la racine `archive`, `sha256` et `signature` (le code d'avant 6a5c83f) : l'archive de Windows y est nommee, et elles
    // se mettent a jour. Le code d'aujourd'hui ne lit pas ces cles.
    if let Some(w) = manifeste["archives"].get("windows").cloned() {
        for k in ["archive", "sha256", "signature"] {
            manifeste[k] = w[k].clone();
        }
    }
    ecrire_json(&d.join(DERNIERE), &manifeste)?;
    let mut r = manifeste;
    r["installations"] = Value::Object(installations);
    r["octets"] = Value::Object(octets_de);
    r["dossier"] = json!(d.display().to_string());
    Ok(r)
}

/// Ce que `moteur maj --publier` et `--publier-github` disent des archives publiees : leur nom et leur taille, celles de
/// mise a jour, celle des fictives, puis celles d'installation.
pub(crate) fn dire_les_archives(r: &Value) -> String {
    let maj = r["archives"].as_object().into_iter().flatten().map(|(k, v)| (v["archive"].as_str().unwrap_or(""), r["octets"][k].as_u64().unwrap_or(0)));
    let installations = r["installations"].as_object().into_iter().flatten().map(|(_, v)| (v["archive"].as_str().unwrap_or(""), v["octets"].as_u64().unwrap_or(0)));
    let fictives = r["fictives"]["archive"].as_str().map(|a| (a, r["octets"]["fictives"].as_u64().unwrap_or(0)));
    maj.chain(fictives).chain(installations).map(|(nom, n)| format!("{} ({} Ko)", nom, n / 1024)).collect::<Vec<_>>().join(", ")
}

/// LA CLE DE GITHUB, fabriquee sur le poste de l'editeur (`moteur maj --cle-github [DOSSIER]`, lot P1). Une cle neuve,
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
    let publique = socle::base64::ecrire(ed25519_dalek::SigningKey::from_bytes(&graine).verifying_key().as_bytes());
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(&fichier).map_err(|e| format!("{} : {}", fichier.display(), e))?;
    f.write_all(socle::base64::ecrire(&graine).as_bytes()).map_err(|e| e.to_string())?;
    graine.fill(0);
    std::fs::write(&fichier_publique, format!("{}\n", publique)).map_err(|e| e.to_string())?;
    Ok(json!({"graine": fichier.display().to_string(), "publique": fichier_publique.display().to_string(), "cle_publique": publique}))
}

/// `publier_signe` depuis une graine en base64 (celle du secret de GitHub), verifiee contre `cle_publique` : refuse si
/// la cle publique n'est pas reportee (vide) ou si la graine n'est pas la sienne. Aucun message ne porte la graine.
pub fn publier_par_graine(logiciel: &Path, sortie: &Path, graine_b64: &str, cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    if cle_publique.is_empty() {
        return Err("la cle publique de GitHub n'est pas reportee dans le code (moteur/src/compagnie/maj.rs, CLE_PUBLIQUE_GITHUB) : rien n'est publie".into());
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

/// LA PUBLICATION PAR LE FLUX DE GITHUB (`moteur maj --publier-github --logiciel L --sortie S`, lot P1) : les archives de `L`
/// signees par la graine de `PUPPETPAIE_CLE_SIGNATURE`, verifiee contre `CLE_PUBLIQUE_GITHUB`, et `derniere.json`, dans
/// `S`. Si `S` porte deja le `derniere.json` de la derniere Release (le flux l'y telecharge), une version qui n'est pas
/// plus recente est refusee (`publier_signe`). Ni le reglage « editeur » ni le coffre n'y jouent : le secret n'existe
/// que dans le travail approuve par Leandre.
pub fn publier_depuis_l_environnement(logiciel: &Path, sortie: &Path, notes: &str, calcul: &str, exigee: &str) -> R<Value> {
    let g = std::env::var(VARIABLE_GRAINE).ok().filter(|g| !g.trim().is_empty()).ok_or_else(|| format!("{} absente : rien n'est publie", VARIABLE_GRAINE))?;
    publier_par_graine(logiciel, sortie, &g, CLE_PUBLIQUE_GITHUB, notes, calcul, exigee)
}

