//! La mise a jour dans l'ombre (`moteur/maj.py`, lot J4) : verifier, appliquer, revenir ; et, chez l'editeur, publier.
//!
//! UNE MISE A JOUR PAR INSTALLATION (lot U1, 14/10/2026). Il n'y a qu'un logiciel par installation, et c'est lui qui se
//! met a jour, pas une compagnie : tout se passe dans `<installation>/maj/` (a cote de `logiciel/`, que la bascule ne
//! touche pas), sans qu'aucune compagnie soit ouverte. Toute installation recoit, celle de l'editeur comprise (Leandre,
//! 14/10/2026 : « mon logiciel c'est le meme que les autres […] quand je fais une mise a jour, il faut que la mise a jour
//! arrive aussi sur ma machine ») ; l'editeur, en plus, publie. Le bridage n'y joue aucun role.
//!
//!   verifier   au lancement, en fond (`chercher_en_fond` ; lance sans reseau, il reprend de lui-meme). Lit
//!              `derniere.json` au depot ; une version plus recente est
//!              prise, son condensat et la SIGNATURE de l'editeur (Ed25519, `cles_de_l_editeur`) verifies, deballee dans
//!              `maj/<version>/`, CONTROLEE (son programme lit la configuration de chaque compagnie de l'installation),
//!              puis `maj/prete.json` est pose.
//!   appliquer  au demarrage suivant. Ce qui est dans `logiciel/` va dans `logiciel/.avant/`, la version prete prend sa
//!              place ; si ca casse en route, tout revient.
//!   publier    chez l'editeur : les archives signees et `derniere.json` dans le depot. Depuis le lot P1 (14/10/2026), la
//!              publication se fait sur GitHub : le flux « Publier » du depot public (`.github/workflows/publier.yml`)
//!              fabrique `logiciel/`, attend l'approbation de Leandre, signe par `publier_depuis_l_environnement` et
//!              pose les archives, `derniere.json` et l'installeur dans une Release, que `DEPOT_PAR_DEFAUT` lit.
//!
//! QUATRE ARCHIVES PAR VERSION (Leandre, 15/10/2026 : « Il y a les quatre : une par systeme et une tout-systeme,
//! ensemble »). `logiciel/` porte les programmes des trois systemes : `puppetpaie.exe` et `WebView2Loader.dll` a la
//! racine (Windows, le seul suivi par Git), `linux/puppetpaie` et `linux/moteur`, `mac/puppetpaie` et `mac/moteur` (poses
//! par le flux, universels Intel et Apple pour le Mac) : Linux et Mac ont tous deux un `puppetpaie`, chacun son dossier
//! (`dossier_programmes`). La publication fait une archive par systeme (les fichiers communs et ceux de ce systeme,
//! `systeme_du_fichier`) et une qui porte tout (`TOUT_SYSTEME` : un dossier portable pour plusieurs machines, et le repli),
//! chacune signee ; `derniere.json` les nomme sous `archives`. Une installation prend celle de son systeme (`SYSTEME`), ou
//! la tout-systeme si la sienne manque (`telecharger`). « Publier » choisit ce qu'il fabrique (Windows seul, Windows et
//! Linux, ou les trois ; Leandre, 15/10/2026) : seules les archives fabriquees sont nommees, la tout-systeme dit sous
//! `systemes` ceux qu'elle porte, et une installation dont le systeme n'est pas publie garde sa version et le dit.
//!
//! ====================================================================================================================
//! LA FORME DE L'ARCHIVE DE MISE A JOUR EST UN CONTRAT AVEC TOUTES LES INSTALLATIONS POSEES. ELLE NE CHANGE PLUS.
//! ====================================================================================================================
//! L'archive que `derniere.json` nomme sous `archives` (`archiver`, `puppetpaie-<version>-<systeme>.zip`) porte LE CONTENU
//! DE `logiciel/` A SA RACINE (`VERSION`, `donnees/`, `interface/`, les programmes), et rien d'autre : ni `logiciel/` en
//! tete, ni `fictives/`. C'est le code DEJA INSTALLE chez chaque compagnie qui la lit, pas celui qu'elle apporte : il la
//! deballe telle quelle dans `maj/<version>/logiciel/`, y lit `VERSION`, et refuse si ce n'est pas la version annoncee.
//! Changer cette forme, c'est couper toutes les installations posees de leurs mises a jour, sans retour possible : leur
//! code ne se change que par une mise a jour. Le ticket #246 l'a fait (7ea89f6, `logiciel/` et `fictives/` dans chaque
//! archive : les installations posees lisaient « 0.0.0 » et refusaient) ; on est revenu a la forme d'avant le jour meme.
//! De meme pour `derniere.json` : `version`, et sous `archives` une entree par systeme (`archive`, `sha256`,
//! `signature` ; `systemes` pour la tout-systeme) ; on peut AJOUTER une cle a sa racine (`fictives`, le 22/10/2026 : le
//! code d'avant ne lit que `version` et `archives`), jamais en retirer ni en changer une, et rien sous `archives` qui ne
//! soit un systeme (le code d'avant refuse alors tout `derniere.json`). Les installations d'avant le 15/10/2026 (une seule
//! archive) lisent, elles, `archive`, `sha256` et `signature` a la racine : ce sont ceux de l'archive de Windows
//! (`publier_signe`).
//! `tests/fichiers/mise_a_jour.rs` deballe l'archive comme le code d'avant (`deballer` de 8fc646b, recopie tel quel).
//!
//! L'ARCHIVE D'INSTALLATION, a cote (ticket #246, 22/10/2026 : `archiver_installation`,
//! `puppetpaie-<version>-<systeme>-installation.zip`) : ce que l'installeur Windows pose, pour Linux, Mac et le dossier
//! portable (la tout-systeme) : `logiciel/` et, a cote, `fictives/` (les compagnies fictives livrees). Decompressee dans
//! un dossier neuf, elle s'ouvre bridee sur ses quatre fictives. Elle ne sert qu'a installer : aucune installation ne la
//! telecharge, `derniere.json` ne la nomme pas, et elle n'est pas signee (l'installeur Windows non plus) ; la Release
//! la porte a cote des archives de mise a jour.
//!
//! LES FICTIVES VOYAGENT A PART (Leandre, 22/10/2026) : `archiver_fictives`, `puppetpaie-<version>-fictives.zip`, signee,
//! nommee dans `derniere.json` sous `fictives` (une cle ajoutee, qu'un code d'avant ignore) avec l'EMPREINTE de leur
//! contenu. Une installation ne la telecharge que si l'empreinte n'est pas celle qu'elle retient (`fictives/.livraison.json`)
//! : une Release aux memes fictives ne coute rien. Quand elle a change, la livraison REMPLACE les fictives du poste au
//! lancement suivant, jouees ou non, photos d'origine comprises (`preparer_les_fictives`, `poser_les_fictives_pretes`).
//! La mise a jour du logiciel, elle, ne touche jamais `fictives/`.
//!
//! LE RELAIS, AU LANCEMENT (09/10/2026 : « que personne ne voit »). Sous Windows, un programme ne se remplace pas pendant
//! qu'il tourne. Le programme qu'on lance regarde donc, AVANT TOUTE FENETRE, si l'installation a une version prete
//! (`au_lancement`) : il copie le programme NEUF hors de `logiciel/` (`maj/relais-<version>`), le lance avec
//! `maj-relais`, et s'arrete sans rien avoir montre. Le relais attend la fin de ce processus (par son numero, pas par un
//! port), bascule, lance la fenetre neuve, et s'arrete : on voit s'ouvrir directement la neuve.
//!
//! UNE VERSION S'INSTALLE DES QU'ELLE EST PRETE : au lancement suivant, ou au clic « Redemarrer pour mettre a jour ».
//! Rien ne la retient. Sa phrase `calcul` (la case de « Publier ») dit a la compagnie ce qu'elle change d'un calcul de
//! paie : l'indicateur la montre avec la version prete, puis, une fois, au lancement qui suit son installation
//! (`dire_l_installation`) ; `moteur version` aussi. Une version EXIGEE bloque tout depot reel tant qu'elle n'est pas
//! installee (`depot_acte::exigee_en_attente`) ; l'indicateur le dit, sans survol (`Recherche::etat`, `exigee`).
//!
//! LA CLE DE SIGNATURE (`config/maj-cle-privee.bin`, lot K1, 14/10/2026 ; Leandre : « si mon ordi tombe, on pourra plus
//! faire de mise a jour, ça me va pas »). Sa graine est scellee par la CLE MAITRESSE DU COFFRE
//! (`Coffre::proteger_par_le_coffre`), dans un petit JSON (`FORMAT_CLE`) : elle s'ouvre partout ou le coffre s'ouvre, par
//! la cle du poste, ou ailleurs par le mot de passe du logiciel (tape, ou la seance). Elle depend donc du coffre qui l'a
//! scellee : sans lui, rien ne l'ouvre, et un coffre neuf ne l'ouvrira pas. Avant, elle etait scellee par la cle du poste
//! seule (`Coffre::proteger`) : perdue avec le poste, les compagnies auraient refuse toute version suivante.
//! UNE CLE SCELLEE A L'ANCIENNE s'ouvre encore par la cle du poste qui l'a faite, et s'y RESCELLE par le coffre a cette
//! ouverture (`cle_privee`) : c'est la seule facon de la sauver sans changer la paire, donc sans faire parvenir une
//! nouvelle cle publique a chaque compagnie. `moteur maj --cle` sur une cle a l'ancienne fait cette ouverture sans publier.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::compagnie::coffre::Coffre;


/// La cle publique de l'editeur (Ed25519, 32 octets en base64) : toute archive non signee par elle est refusee.
/// Refaite le 09/10/2026 : l'ancienne (16/09, scellee par le moteur Python) n'existait plus sur le poste ; aucune
/// compagnie n'avait encore recu de version.
pub const CLE_PUBLIQUE: &str = "sDj5+46oBt7Yy1x00VJ1ThJNOwf5jT5+bttXEN/yAIU=";
/// LA CLE PUBLIQUE DE GITHUB (lot P1, 14/10/2026) : le flux « Publier » du depot public signe l'archive avec une cle
/// neuve, dediee, dont la graine est le secret `PUPPETPAIE_CLE_SIGNATURE` de l'environnement `publication` (approuve
/// par Leandre). Fabriquee sur le poste de l'editeur par `moteur maj --cle-github` (`generer_cle_github`), qui ecrit la
/// cle publique dans un fichier a reporter ICI. Reportee le 14/10/2026 (fabriquee par Leandre sur son poste). Vide,
/// aucune archive ne serait acceptee par elle et le flux refuserait de publier (`publier_depuis_l_environnement`).
pub const CLE_PUBLIQUE_GITHUB: &str = "48dC1Gb82e2Zh7z7NiUH14EbXX7IvXTp83Xaui/31wg=";
/// Le depot des mises a jour QUAND LE REGLAGE DE L'INSTALLATION NE DIT RIEN : les fichiers de la DERNIERE RELEASE du
/// depot public (lot P1, 14/10/2026 ; avant, le serveur du filet, qui ne servait rien). GitHub repond a
/// `releases/latest/download/<fichier>` par une redirection vers la Release la plus recente, puis vers son stockage :
/// `lire_depot` les suit (ureq, dix au plus). `derniere.json` y nomme les archives, servies a cote. Un `depot` pose dans le
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
const SANS_CLE: &str = "pas de cle privee : les compagnies refuseraient une archive non signee (moteur maj --cle pour la fabriquer)";
/// Ce qui ne part jamais dans une archive, ni ne bouge a la bascule.
const EXCLUS: [&str; 6] = [".git", ".avant", ".apres", "__pycache__", ".pytest_cache", "target"];
const NE_PAS_TOUCHER: [&str; 3] = [".git", ".avant", ".apres"];
/// Ce qui, dans `logiciel/`, dit l'INSTALLATION et non la version : il ne part jamais dans une archive (`archiver`), et la
/// bascule le laisse EXACTEMENT comme il etait -- remis s'il etait la (copie depuis `.avant/`, qui garde l'image entiere de
/// la version d'avant pour `revenir`), absent s'il n'y etait pas, meme si une archive d'avant le lot V1 en portait un.
/// Leandre, 14/10/2026 : « le petit curseur ecrit en JSON lui il reste comme il etait. Si les gens ont debloque, elle reste
/// debloquee » ; bridee, elle reste bridee. Depuis le ticket #246, ce fichier n'est plus que celui du PAQUET SOUDE
/// (`"edition": "bride"`, `bridage.rs`) ; le geste, `.edition.json`, vit a cote de `logiciel/` : la bascule n'y touche pas.
const PROPRES_A_L_INSTALLATION: [&str; 1] = ["EDITION.json"];
/// Les programmes d'un systeme Unix, dans son dossier de `logiciel/` (`linux/`, `mac/`) : la fenetre, et le moteur (pour un
/// Linux sans WebKitGTK, qui ouvre la meme page dans le navigateur ; et le flux « Publier » signe avec celui de Linux).
/// Un zip ne garde le bit d'execution que si on l'y marque : `archiver` le marque sur eux, `deballer` le remet.
const PROGRAMMES_UNIX: [&str; 2] = ["puppetpaie", "moteur"];

/// LES TROIS SYSTEMES, chacun son archive (Leandre, 15/10/2026). Le nom est celui du dossier des programmes de ce systeme
/// dans `logiciel/` (sauf Windows, dont les programmes sont a la racine : `dossier_programmes_de`) et la cle de son
/// archive dans `derniere.json`.
pub const SYSTEMES: [&str; 3] = ["windows", "linux", "mac"];
/// L'archive qui porte les trois systemes a la fois : un dossier portable qui passe d'une machine a l'autre, et le repli
/// d'une installation dont le systeme n'a pas d'archive a lui dans `derniere.json`.
pub const TOUT_SYSTEME: &str = "tout-systeme";
/// Le systeme de ce programme, tel qu'il a ete compile. Ce qui n'est ni Windows ni macOS est lu comme Linux : le flux ne
/// compile pour aucun autre, et un programme Linux qui ne se lance pas ailleurs est refuse par `controler`.
pub const SYSTEME: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "mac"
} else {
    "linux"
};
/// Ce qui, a la racine de `logiciel/`, n'est que de Windows : la fenetre (suivie par Git), le chargeur de WebView2 qu'elle
/// charge au demarrage, et le moteur s'il y est pose.
const PROPRES_A_WINDOWS: [&str; 3] = ["puppetpaie.exe", "WebView2Loader.dll", "moteur.exe"];

type R<T> = Result<T, String>;

/// Le dossier du logiciel (`logiciel/`), resolu.
pub fn code() -> PathBuf {
    crate::interface::compagnies::logiciel()
}

/// Le dossier des programmes de CE systeme dans `logiciel/` (`dossier_programmes_de`).
pub fn dossier_programmes(code: &Path) -> PathBuf {
    dossier_programmes_de(code, SYSTEME)
}

/// Le dossier des programmes d'un systeme : `logiciel/` lui-meme pour Windows, ou l'exe est pose a cote de `donnees/` et
/// `interface/` (rangement du 09/10/2026 : `logiciel/` est exactement ce que les gens recoivent ; le raccourci de
/// l'installeur et `PuppetPaie.cmd` le lancent la) ; `logiciel/linux/` et `logiciel/mac/` pour les deux autres, qui ont
/// chacun un `puppetpaie` (15/10/2026 : l'archive de tous les systemes les porte ensemble). Un programme pose dans un
/// sous-dossier trouve ses `donnees/` en remontant (`commandes::compagnie::fixer_le_code`).
pub fn dossier_programmes_de(code: &Path, systeme: &str) -> PathBuf {
    if systeme == "windows" {
        code.to_path_buf()
    } else {
        code.join(systeme)
    }
}

/// Le programme `nom` de CE systeme dans `logiciel/`.
pub fn programme(code: &Path, nom: &str) -> PathBuf {
    programme_de(code, SYSTEME, nom)
}

/// Le programme `nom` d'un systeme dans `logiciel/` : `puppetpaie.exe`, `linux/puppetpaie`, `mac/puppetpaie`.
pub fn programme_de(code: &Path, systeme: &str, nom: &str) -> PathBuf {
    dossier_programmes_de(code, systeme).join(if systeme == "windows" { format!("{}.exe", nom) } else { nom.to_string() })
}

/// Le systeme d'un fichier de `logiciel/` (chemin relatif, `/` comme separateur), ou `None` s'il est commun a tous
/// (`VERSION`, `donnees/`, `interface/`) : ce qui decide de ce que porte l'archive d'un systeme.
pub fn systeme_du_fichier(rel: &str) -> Option<&'static str> {
    match rel.split_once('/') {
        Some((tete, _)) => SYSTEMES.into_iter().find(|s| *s != "windows" && *s == tete),
        None => PROPRES_A_WINDOWS.contains(&rel).then_some("windows"),
    }
}

/// Le nom de l'archive d'une version pour un systeme (ou `TOUT_SYSTEME`) : `puppetpaie-<version>-<systeme>.zip`.
pub fn nom_d_archive(version: &str, systeme: &str) -> String {
    format!("puppetpaie-{}-{}.zip", version, systeme)
}

/// Le nom de l'archive D'INSTALLATION d'une version (`archiver_installation`) : `puppetpaie-<version>-<systeme>-installation.zip`.
/// Il n'y en a que pour Linux, Mac et tous les systemes : Windows a son installeur.
pub fn nom_d_installation(version: &str, systeme: &str) -> String {
    format!("puppetpaie-{}-{}-installation.zip", version, systeme)
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
    socle::ecriture::ecrire_json(p, v, 1)
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

/// Le dossier du logiciel dans une installation, et dans une archive.
const LOGICIEL: &str = "logiciel";

/// Le logiciel de l'installation.
fn logiciel_de(inst: &Path) -> PathBuf {
    inst.join(LOGICIEL)
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
    lire_depot_suivi(depot, nom, &|_, _| {}).map_err(|(_, e)| e)
}

/// Ce qu'on dit d'un echec, en un mot, a l'ecran (ticket #210 : « les erreurs se disent en un mot, sans alarme ») ; la
/// phrase entiere va au journal de la mise a jour.
const HORS_LIGNE: &str = "hors ligne";
const INTROUVABLE: &str = "introuvable";
const ILLISIBLE: &str = "dépôt illisible";
const SIGNATURE_REFUSEE: &str = "signature refusée";
const ALTEREE: &str = "archive altérée";
const REFUSEE_AU_CONTROLE: &str = "refusée au contrôle";
const REGLAGE_ILLISIBLE: &str = "réglage illisible";

type Echec = (&'static str, String);

/// `lire_depot`, en disant a `suivre` ce qui est recu (octets recus, octets attendus si le depot le dit) : la barre de
/// l'indicateur (`Recherche`). Un echec porte son mot : injoignable (`HORS_LIGNE`), ou absent (`INTROUVABLE`).
fn lire_depot_suivi(depot: &str, nom: &str, suivre: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, Echec> {
    const LIMITE: u64 = 512 * 1024 * 1024;
    let lire_tout = |lecteur: &mut dyn Read, attendus: Option<u64>| -> std::io::Result<Vec<u8>> {
        let mut o = Vec::with_capacity(attendus.unwrap_or(0).min(LIMITE) as usize);
        let mut tampon = vec![0u8; 64 * 1024];
        suivre(0, attendus);
        loop {
            let n = lecteur.read(&mut tampon)?;
            if n == 0 {
                return Ok(o);
            }
            o.extend_from_slice(&tampon[..n]);
            suivre(o.len() as u64, attendus);
        }
    };
    if depot.starts_with("http://") || depot.starts_with("https://") {
        let url = format!("{}/{}", depot.trim_end_matches('/'), nom);
        let mut c = ureq::Agent::config_builder().user_agent(AGENT).timeout_global(Some(std::time::Duration::from_secs(120)));
        if url.starts_with("https:") {
            c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
        }
        // En essai, le garde-fou reseau : rien ne sort de la machine (une panne de reseau, comme hors ligne).
        let agent: ureq::Agent = socle::garde_reseau::regler(c, &url).map_err(|e| (HORS_LIGNE, format!("{} : {}", url, e)))?.build().into();
        let mut r = agent.get(&url).call().map_err(|e| {
            let mot = if matches!(e, ureq::Error::StatusCode(_)) { INTROUVABLE } else { HORS_LIGNE };
            (mot, format!("{} : {}", url, e))
        })?;
        let attendus = r.body().content_length();
        let mut lecteur = r.body_mut().with_config().limit(LIMITE).reader();
        return lire_tout(&mut lecteur, attendus).map_err(|e| (HORS_LIGNE, format!("{} : {}", url, e)));
    }
    let p = Path::new(depot).join(nom);
    let dit = |e: std::io::Error| (INTROUVABLE, format!("{} : {}", p.display(), e));
    let mut f = std::fs::File::open(&p).map_err(dit)?;
    let attendus = f.metadata().ok().map(|m| m.len());
    lire_tout(&mut f, attendus).map_err(dit)
}

/// `derniere.json` du depot : la `version`, et sous `archives` une entree par systeme publie (`SYSTEMES`, `TOUT_SYSTEME`),
/// chacune avec son `archive` (le nom du fichier, a cote), son `sha256` et sa `signature`. Toute autre forme est refusee.
pub fn derniere(depot: &str) -> R<Value> {
    derniere_dite(depot).map_err(|(_, e)| e)
}

/// `derniere`, l'echec avec son mot : le depot ne repond pas, ou il repond quelque chose qui n'est pas un `derniere.json`.
fn derniere_dite(depot: &str) -> Result<Value, Echec> {
    let octets = lire_depot_suivi(depot, DERNIERE, &|_, _| {})?;
    derniere_lue(&octets).map_err(|e| (ILLISIBLE, e))
}

fn derniere_lue(octets: &[u8]) -> R<Value> {
    let d: Value = serde_json::from_slice(octets).map_err(|e| format!("derniere.json illisible : {}", e))?;
    if d.get("version").and_then(Value::as_str).filter(|s| !s.is_empty()).is_none() {
        return Err("derniere.json sans « version »".into());
    }
    let archives = d.get("archives").and_then(Value::as_object).filter(|a| !a.is_empty()).ok_or("derniere.json sans « archives »")?;
    for (systeme, a) in archives {
        if systeme != TOUT_SYSTEME && !SYSTEMES.contains(&systeme.as_str()) {
            return Err(format!("derniere.json : « {} » n'est pas un systeme", systeme));
        }
        for k in ["archive", "sha256", "signature"] {
            if a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).is_none() {
                return Err(format!("derniere.json : l'archive « {} » sans « {} »", systeme, k));
            }
        }
    }
    // Celle de tous les systemes dit lesquels elle porte : ceux qui ont ete fabriques.
    if let Some(t) = archives.get(TOUT_SYSTEME) {
        let portes = t.get("systemes").and_then(Value::as_array).filter(|l| !l.is_empty()).ok_or("derniere.json : l'archive « tout-systeme » sans « systemes »")?;
        if let Some(x) = portes.iter().find(|x| !x.as_str().is_some_and(|x| SYSTEMES.contains(&x))) {
            return Err(format!("derniere.json : « {} » n'est pas un systeme", x));
        }
    }
    Ok(d)
}

fn condensat(octets: &[u8]) -> String {
    Sha256::digest(octets).iter().map(|b| format!("{:02x}", b)).collect()
}

fn b64(t: &str) -> R<Vec<u8>> {
    socle::base64::lire(t)
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
    Ok(socle::base64::ecrire(&ed25519_dalek::SigningKey::from_bytes(&k).sign(octets).to_bytes()))
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
    v.retain(|r| !crate::interface::compagnies::est_cache(r) && r.join("config").join("entreprise.json").is_file() && (!crate::interface::compagnies::est_copie(r) || crate::interface::compagnies::est_fictive(r)));
    v
}

/// L'ARCHIVE DE MISE A JOUR (`archiver` : le contenu de `logiciel/` a sa racine, le contrat de la tete de ce fichier),
/// deballee telle quelle dans `neuf` (`maj/<version>/logiciel/`). Rien n'en sort de `neuf` (`enclosed_name`) : les
/// compagnies du poste, les fictives jouees et leur photo d'origine (`fictives/.origine/`) ne sont jamais touchees, et
/// l'archive n'en porte aucune. Une archive D'INSTALLATION (`archiver_installation` : `logiciel/` et `fictives/` en tete)
/// n'est pas une mise a jour : refusee avant d'ecrire quoi que ce soit.
fn deballer(octets: &[u8], neuf: &Path) -> R<()> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| format!("archive illisible : {}", e))?;
    if z.file_names().any(|n| n == format!("{}/VERSION", LOGICIEL) || n.starts_with(&format!("{}/", crate::interface::compagnies::DOSSIER_FICTIVES))) {
        return Err("c'est une archive d'installation (logiciel/ et fictives/), pas une archive de mise a jour".into());
    }
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { return Err(format!("archive suspecte : {}", f.name())) };
        let chemin = neuf.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&chemin).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        // Un programme d'un systeme Unix : `archiver` l'a marque executable, il le redevient ici.
        let executable = f.unix_mode().is_some_and(|m| m & 0o111 != 0);
        let mut o = Vec::new();
        f.read_to_end(&mut o).map_err(|e| e.to_string())?;
        std::fs::write(&chemin, o).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("{} : {}", chemin.display(), e))?;
        }
        #[cfg(not(unix))]
        let _ = executable;
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

/// L'archive de CE systeme, sinon celle de tous les systemes SI ELLE LE PORTE (sa liste `systemes`) : leur cle dans
/// `derniere.json` et leur entree, dans l'ordre ou les essayer. Vide : la version est publiee sans ce systeme (Leandre,
/// 15/10/2026 : « Publier » choisit Windows seul, Windows et Linux, ou les trois), l'installation ne telecharge rien.
fn archives_a_essayer(d: &Value) -> Vec<(&'static str, &Value)> {
    let porte = |a: &Value| a["systemes"].as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(SYSTEME)));
    let mut v: Vec<(&'static str, &Value)> = d["archives"].get(SYSTEME).map(|a| (SYSTEME, a)).into_iter().collect();
    v.extend(d["archives"].get(TOUT_SYSTEME).filter(|a| porte(a)).map(|a| (TOUT_SYSTEME, a)));
    v
}

/// La version est-elle publiee pour ce systeme (son archive, ou celle de tous qui le porte) ?
fn publiee_pour_ce_systeme(d: &Value) -> bool {
    !archives_a_essayer(d).is_empty()
}

/// Telecharge l'archive de ce systeme (ou, si `derniere.json` n'en a pas pour lui ou qu'elle ne se telecharge pas, celle
/// de tous les systemes, si elle le porte), la verifie, la deballe, la controle et la pose prete. Rend la cle de l'archive prise. Une
/// archive telechargee dont le condensat ou la signature ne vaut pas est refusee sans essayer l'autre : ce n'est pas
/// qu'elle manque, c'est qu'elle n'est pas la notre.
fn telecharger(inst: &Path, depot: &str, d: &Value, cles: &[&str], suivi: &Recherche) -> Result<&'static str, Echec> {
    let version = d["version"].as_str().unwrap_or_default();
    let candidates = archives_a_essayer(d);
    if candidates.is_empty() {
        return Err((INTROUVABLE, format!("derniere.json n'a pas d'archive qui porte ce systeme ({})", SYSTEME)));
    }
    let mut manquees = Vec::new();
    let mut mot = INTROUVABLE;
    let mut prise = None;
    for (k, a) in candidates {
        let suivre = |recus: u64, attendus: Option<u64>| suivi.poser(Etape::Telechargement, Some(version), |p| (p.recus, p.attendus) = (recus, attendus));
        match lire_depot_suivi(depot, a["archive"].as_str().unwrap_or_default(), &suivre) {
            Ok(o) => {
                prise = Some((k, a, o));
                break;
            }
            Err((m, e)) => {
                mot = m;
                manquees.push(e)
            }
        }
    }
    let (systeme, a, octets) = prise.ok_or_else(|| (mot, manquees.join(" ; ")))?;
    suivi.poser(Etape::Preparation, Some(version), |_| {});
    let sha = condensat(&octets);
    let attendu = a["sha256"].as_str().unwrap_or_default();
    if sha != attendu {
        return Err((ALTEREE, format!("archive alteree (condensat {}… attendu {}…)", &sha[..12], attendu.chars().take(12).collect::<String>())));
    }
    let signature = a.get("signature").and_then(Value::as_str).ok_or((SIGNATURE_REFUSEE, "archive non signee : derniere.json n'a pas de « signature »".to_string()))?;
    if !signee_par_l_une(&octets, signature, cles) {
        return Err((SIGNATURE_REFUSEE, "la signature de l'archive n'est pas celle de l'editeur".into()));
    }
    let au_controle = |e: String| (REFUSEE_AU_CONTROLE, e);
    let cible = dossier_maj(inst).join(version);
    let _ = std::fs::remove_dir_all(&cible);
    let neuf = logiciel_de(&cible);
    std::fs::create_dir_all(&neuf).map_err(|e| au_controle(e.to_string()))?;
    deballer(&octets, &neuf).map_err(au_controle)?;
    if courante(&neuf) != version {
        return Err(au_controle(format!("l'archive dit {}, derniere.json dit {}", version_lue(&courante(&neuf)), version_lue(version))));
    }
    // Le programme de ce systeme doit y etre, meme sans compagnie a controler.
    if !programme(&neuf, "puppetpaie").is_file() {
        let _ = std::fs::remove_dir_all(&cible);
        return Err(au_controle(format!("le programme de ce systeme manque dans l'archive ({})", programme(Path::new(""), "puppetpaie").display())));
    }
    // Le programme neuf lit la configuration de chaque compagnie de l'installation (sans compagnie, il n'y a rien a lire).
    for racine in compagnies_de(inst) {
        if let Err(p) = controler(&neuf, &racine) {
            let _ = std::fs::remove_dir_all(&cible);
            return Err(au_controle(format!("le nouveau code ne demarre pas sur {} : {}", racine.display(), p)));
        }
    }
    ecrire_json(
        &dossier_maj(inst).join("prete.json"),
        &json!({
            "version": version, "quand": socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].to_string(),
            "archive": a["archive"].clone(),
            "notes": d.get("notes").cloned().unwrap_or(json!("")), "calcul": d.get("calcul").cloned().unwrap_or(json!("")),
            "exigee": d.get("exigee").cloned().unwrap_or(json!("")),
        }),
    )
    .map_err(au_controle)?;
    Ok(systeme)
}

/// LES FICTIVES PRETES : la livraison telechargee et verifiee (`preparer_les_fictives`), qui attend le prochain lancement
/// dans `maj/fictives-pretes/fictives/` ; `poser_les_fictives_pretes` la met en place.
const FICTIVES_PRETES: &str = "fictives-pretes";

fn lire_valeur(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Ce qu'un dossier `fictives/` retient de sa livraison (`LIVRAISON`), ou `None` (une installation d'avant ce mecanisme,
/// ou posee par l'installeur Windows, qui ne la porte pas).
fn livraison_retenue(fictives: &Path) -> Option<Value> {
    lire_valeur(&fictives.join(LIVRAISON)).filter(|v| v.get("empreinte").and_then(Value::as_str).is_some_and(|e| !e.is_empty()))
}

/// L'empreinte de la livraison de fictives que retient un dossier `fictives/`, ou `None`.
pub fn empreinte_retenue(fictives: &Path) -> Option<String> {
    livraison_retenue(fictives).and_then(|v| v["empreinte"].as_str().map(String::from))
}

fn dossier_des_fictives(inst: &Path) -> PathBuf {
    inst.join(crate::interface::compagnies::DOSSIER_FICTIVES)
}

/// L'archive des fictives deballee dans `cible` (`cible/fictives/...`). Tout ce qu'elle porte est sous `fictives/` et y
/// reste (`enclosed_name`) ; autre chose, et elle est refusee.
fn deballer_les_fictives(octets: &[u8], cible: &Path) -> R<()> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| format!("archive des fictives illisible : {}", e))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let rel = f
            .enclosed_name()
            .filter(|r| r.starts_with(crate::interface::compagnies::DOSSIER_FICTIVES))
            .ok_or_else(|| format!("archive des fictives suspecte : {}", f.name()))?;
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
    Ok(())
}

/// LES FICTIVES SE METTENT A JOUR A PART DU LOGICIEL (Leandre, 22/10/2026). `derniere.json` les nomme sous `fictives`
/// (`archive`, `sha256`, `signature`, `empreinte`) -- une cle qu'un code d'avant ignore : il ne lit que `version` et
/// `archives`. Une installation ne les telecharge QUE si leur empreinte n'est pas celle qu'elle retient
/// (`fictives/.livraison.json`) ni celle qui l'attend deja prete : une Release aux memes fictives ne coute rien. Recues,
/// condensat et signature verifies comme une archive du logiciel, et leur `.livraison.json` disant l'empreinte annoncee,
/// elles attendent dans `maj/fictives-pretes/` le prochain lancement (`poser_les_fictives_pretes`). Un DEPOT DE TRAVAIL
/// (l'installation porte `.git` : le poste de l'editeur, ou `fictives/` est suivi par Git et d'ou partent les
/// livraisons) ne les recoit pas. Rend ce qui s'est passe, ou `None` s'il n'y avait rien a faire.
pub fn preparer_les_fictives(inst: &Path, depot: &str, d: &Value, cles: &[&str]) -> Option<String> {
    let f = d.get("fictives")?;
    let empreinte = f.get("empreinte").and_then(Value::as_str).filter(|e| !e.is_empty())?;
    let maj = dossier_maj(inst);
    let pretes = maj.join(FICTIVES_PRETES);
    let vise = pretes.join(crate::interface::compagnies::DOSSIER_FICTIVES);
    if empreinte_retenue(&dossier_des_fictives(inst)).as_deref() == Some(empreinte) || empreinte_retenue(&vise).as_deref() == Some(empreinte) {
        return None;
    }
    if inst.join(".git").exists() {
        return None;
    }
    let court = &empreinte[..empreinte.len().min(12)];
    let essai = || -> R<()> {
        let archive = f.get("archive").and_then(Value::as_str).filter(|a| !a.is_empty()).ok_or("derniere.json : les fictives sans « archive »")?;
        let octets = lire_depot_suivi(depot, archive, &|_, _| {}).map_err(|(_, e)| e)?;
        if Some(condensat(&octets).as_str()) != f.get("sha256").and_then(Value::as_str) {
            return Err("archive des fictives alteree (condensat)".into());
        }
        if !f.get("signature").and_then(Value::as_str).is_some_and(|s| signee_par_l_une(&octets, s, cles)) {
            return Err("la signature de l'archive des fictives n'est pas celle de l'editeur".into());
        }
        let part = maj.join(format!("{}.part", FICTIVES_PRETES));
        let _ = std::fs::remove_dir_all(&part);
        deballer_les_fictives(&octets, &part)?;
        if empreinte_retenue(&part.join(crate::interface::compagnies::DOSSIER_FICTIVES)).as_deref() != Some(empreinte) {
            let _ = std::fs::remove_dir_all(&part);
            return Err("l'archive des fictives ne dit pas l'empreinte annoncee".into());
        }
        let _ = std::fs::remove_dir_all(&pretes);
        std::fs::rename(&part, &pretes).map_err(|e| e.to_string())
    };
    Some(match essai() {
        Ok(()) => format!("Compagnies fictives {} telechargees, controlees, pretes : elles se poseront au prochain demarrage.", court),
        Err(e) => format!("Compagnies fictives {} non preparees : {}", court, e),
    })
}

/// LE MARQUEUR D'UNE FICTIVE LIVREE, pour une installation qui ne retient aucune livraison (d'avant ce mecanisme, ou
/// posee par l'installeur Windows) : son `COPIE-DE-DEVELOPPEMENT.json` dit `copie_de` = « compagnie fictive <son nom> »,
/// exactement ce que porte chaque fictive livree (`fictives/<nom>/COPIE-DE-DEVELOPPEMENT.json`). Une fictive copiee et
/// renommee sur le poste dit encore le nom de celle qu'elle copie : ce n'est pas le sien.
fn marquee_comme_livree(dossier: &Path, nom: &str) -> bool {
    lire_valeur(&dossier.join("COPIE-DE-DEVELOPPEMENT.json")).and_then(|v| v.get("copie_de").and_then(Value::as_str).map(String::from))
        == Some(format!("{} {}", crate::interface::compagnies::MARQUE_FICTIVE, nom))
}

/// AU LANCEMENT, avant toute fenetre (`interface::preparer_l_installation`) : la livraison de fictives prete se pose.
/// Elle ne touche QUE CE QU'ELLE A POSE ELLE-MEME (Leandre, 22/10/2026 : « les gens pourront creer des fictives avec leurs
/// donnees, il ne faut pas les supprimer par megarde ») : un dossier de `fictives/` est a elle si son nom est dans
/// `posees` de la livraison retenue ET que sa marque (`.livree.json`) dit l'empreinte qu'on y a retenue ; sans livraison
/// retenue, si son nom est celui d'une fictive livree et qu'il porte le marqueur d'une fictive livree
/// (`marquee_comme_livree`). Ce qui est a elle est REMPLACE, joue ou non, photo d'origine (`fictives/.origine/<nom>`)
/// comprise -- elle se reprend a la prochaine ouverture, sur la fictive neuve (Leandre, 22/10/2026 : « on remplace ») --
/// ou RETIRE si la nouvelle livraison ne le porte plus. Une fictive nouvelle apparait. Une fictive livree dont le nom est
/// pris par un dossier qui n'est pas a la livraison n'est PAS posee : le dossier reste intact, le journal de la mise a
/// jour le dit, et elle n'entre pas dans `posees`. Tout autre dossier (une copie renommee, une fictive faite sur le
/// poste) et sa photo d'origine ne sont jamais touches, ni rien hors de `fictives/`. Un dossier qui ne se deplace pas (un
/// fichier ouvert) reste tel quel et a la livraison ; l'empreinte retenue reste alors l'ancienne, et le lancement
/// suivant reessaie. Rend ce qui s'est passe, ou `None` s'il n'y avait rien de pret.
pub fn poser_les_fictives_pretes(inst: &Path) -> Option<String> {
    let pretes = dossier_maj(inst).join(FICTIVES_PRETES);
    let neuves = pretes.join(crate::interface::compagnies::DOSSIER_FICTIVES);
    let nouvelle = livraison_retenue(&neuves)?;
    let poste = dossier_des_fictives(inst);
    let ancienne = livraison_retenue(&poste);
    let origines = poste.join(".origine");
    let avant = dossier_maj(inst).join("fictives-avant");
    let _ = std::fs::remove_dir_all(&avant);
    let _ = std::fs::create_dir_all(&poste);
    let empreinte_de = |v: &Value, nom: &str| v["posees"].get(nom).and_then(Value::as_str).map(String::from);
    let a_la_livraison = |nom: &str| -> bool {
        let d = poste.join(nom);
        match &ancienne {
            Some(a) => empreinte_de(a, nom).is_some_and(|e| lire_valeur(&d.join(LIVREE)).and_then(|m| m["empreinte"].as_str().map(String::from)) == Some(e)),
            None => marquee_comme_livree(&d, nom),
        }
    };
    let ecarter = |nom: &str| -> R<()> {
        let d = poste.join(nom);
        if d.exists() {
            std::fs::create_dir_all(&avant).map_err(|e| e.to_string())?;
            std::fs::rename(&d, avant.join(nom)).map_err(|e| format!("{} : {}", d.display(), e))?;
        }
        let _ = std::fs::remove_dir_all(origines.join(nom));
        Ok(())
    };
    let mut posees = serde_json::Map::new();
    let mut dit = Vec::new();
    let mut manque = false;
    let noms_neufs: Vec<String> = nouvelle["posees"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    for nom in &noms_neufs {
        let d = poste.join(nom);
        if d.exists() && !a_la_livraison(nom) {
            dit.push(format!("{} non posee : un dossier de ce nom, que la livraison n'a pas pose, reste tel quel", nom));
            continue;
        }
        let pose = ecarter(nom).and_then(|_| std::fs::rename(neuves.join(nom), &d).map_err(|e| format!("{} : {}", d.display(), e)));
        match pose {
            Ok(()) => {
                posees.insert(nom.clone(), nouvelle["posees"][nom].clone());
            }
            Err(e) => {
                manque = true;
                dit.push(format!("{} non remplacee ({})", nom, e));
                if let Some(e) = ancienne.as_ref().and_then(|a| empreinte_de(a, nom)) {
                    posees.insert(nom.clone(), json!(e));
                }
            }
        }
    }
    // Celles que l'ancienne livraison avait posees et que la nouvelle ne porte plus : retirees, si elles sont a elle.
    let anciennes: Vec<String> = ancienne.as_ref().and_then(|a| a["posees"].as_object()).map(|o| o.keys().cloned().collect()).unwrap_or_default();
    for nom in anciennes.iter().filter(|n| !noms_neufs.contains(n)) {
        if !poste.join(nom).exists() || !a_la_livraison(nom) {
            continue;
        }
        if let Err(e) = ecarter(nom) {
            manque = true;
            dit.push(format!("{} non retiree ({})", nom, e));
            if let Some(e) = ancienne.as_ref().and_then(|a| empreinte_de(a, nom)) {
                posees.insert(nom.clone(), json!(e));
            }
        } else {
            dit.push(format!("{} retiree", nom));
        }
    }
    // Les fichiers de `fictives/` livres a la racine (`LISEZ-MOI.md`).
    for e in std::fs::read_dir(&neuves).into_iter().flatten().flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        if e.file_type().is_ok_and(|t| t.is_file()) && nom != LIVRAISON {
            let _ = std::fs::copy(e.path(), poste.join(&nom));
        }
    }
    let empreinte = nouvelle["empreinte"].as_str().unwrap_or_default().to_string();
    let retenue = if manque { ancienne.as_ref().and_then(|a| a["empreinte"].as_str().map(String::from)).unwrap_or_default() } else { empreinte.clone() };
    let _ = ecrire_json(&poste.join(LIVRAISON), &json!({"empreinte": retenue, "posees": posees}));
    let _ = std::fs::remove_dir_all(&avant);
    let _ = std::fs::remove_dir_all(&pretes);
    Some(format!(
        "Compagnies fictives : livraison {} posee ({}){}.",
        &empreinte[..empreinte.len().min(12)],
        posees.keys().cloned().collect::<Vec<_>>().join(", "),
        if dit.is_empty() { String::new() } else { format!(" ; {}", dit.join(" ; ")) }
    ))
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
    verifier_suivi(inst, telecharger_aussi, cles, &Recherche::nouvelle())
}

/// `verifier_avec`, en tenant `suivi` a jour a chaque etape (ce que l'indicateur montre, ticket #210). Ce que la recherche
/// rend (la phrase du journal) et ce qu'elle laisse dans `suivi` disent la meme chose.
pub fn verifier_suivi(inst: &Path, telecharger_aussi: bool, cles: &[&str], suivi: &Recherche) -> String {
    verifier_en(inst, telecharger_aussi, cles, suivi, false)
}

/// `verifier_suivi` ; `calme` : un tour de fond (`chercher_tant_qu_il_tourne`), qui ne change ce que l'indicateur montre
/// que si quelque chose change. Il ne passe pas par « Recherche de mise a jour… », et un depot qui ne repond pas, le temps
/// d'un tour, laisse l'etat tel qu'il etait (« Logiciel a jour ») : le tour suivant redemandera.
/// Une version plus recente trouvee se dit comme au lancement (« Mise a jour en cours », puis prete ou l'echec).
fn verifier_en(inst: &Path, telecharger_aussi: bool, cles: &[&str], suivi: &Recherche, calme: bool) -> String {
    if !calme {
        suivi.poser(Etape::Recherche, None, |_| {});
    }
    let c = logiciel_de(inst);
    let cour = version_lue(&courante(&c));
    let echec = |mot: &'static str, version: Option<&str>, phrase: String| {
        suivi.poser(Etape::Echec, version, |p| p.mot = mot.to_string());
        phrase
    };
    let depot = match reglage(inst) {
        Ok((d, _)) => self::depot(d),
        Err(e) if calme => return format!("version {} ({} ; nouvel essai au tour suivant)", cour, e),
        Err(e) => return echec(REGLAGE_ILLISIBLE, None, format!("version {} ({})", cour, e)),
    };
    let d = match derniere_dite(&depot) {
        Ok(d) => d,
        Err((_, e)) if calme => return format!("version {} (depot injoignable : {} ; nouvel essai au tour suivant)", cour, e),
        Err((mot, e)) => return echec(mot, None, format!("version {} (depot injoignable : {})", cour, e)),
    };
    // Les fictives, a part du logiciel : seulement si leur livraison a change, a jour ou non (`preparer_les_fictives`).
    if telecharger_aussi {
        if let Some(t) = preparer_les_fictives(inst, &depot, &d, cles) {
            noter(inst, &t);
        }
    }
    let v = d["version"].as_str().unwrap_or_default().to_string();
    if !plus_recente(&v, &courante(&c)) {
        suivi.poser(Etape::AJour, Some(&courante(&c)), |p| p.sans_ce_systeme = None);
        return format!("version {}, a jour", cour);
    }
    let lue = version_lue(&v);
    // Publiee sans ce systeme : rien a telecharger, la version en place reste (une tout-systeme qui ne le porte pas ne
    // se lancerait pas ici). Pour cette installation, il n'y a pas de mise a jour.
    if !publiee_pour_ce_systeme(&d) {
        suivi.poser(Etape::AJour, Some(&courante(&c)), |p| p.sans_ce_systeme = Some(v.clone()));
        return format!("version {} -- la {} est publiee sans ce systeme ({}) : cette installation garde sa version", cour, lue, SYSTEME);
    }
    let deja = prete(inst).and_then(|p| p["version"].as_str().map(String::from)).as_deref() == Some(v.as_str());
    if deja {
        suivi.prete(inst, &v);
        return format!("version {} -- la {} est prete", cour, lue);
    }
    // `moteur maj --sans-telecharger` (le seul appel sans telechargement, sur un suivi que personne ne lit) : la version
    // existe, rien n'est recu. On le dit tel quel, sans pretendre « a jour » ni « prete ».
    if !telecharger_aussi {
        return format!("version {} -- la {} est publiee, pas encore telechargee (`moteur maj` la prepare)", cour, lue);
    }
    match telecharger(inst, &depot, &d, cles, suivi) {
        Ok(prise) => {
            suivi.prete(inst, &v);
            format!(
                "version {} -- la {} est telechargee, controlee, prete : elle s'appliquera au prochain demarrage{}",
                cour,
                lue,
                if prise == SYSTEME { String::new() } else { format!(" (archive {} : celle de ce systeme manque)", prise) }
            )
        }
        Err((mot, e)) => echec(mot, Some(&v), format!("version {} -- la {} existe mais n'a pas pu etre preparee : {}", cour, lue, e)),
    }
}

/// LA RECHERCHE EN FOND, DITE A L'ECRAN (ticket #210, Leandre, 16/10/2026 : « quand il charge la mise a jour en
/// arriere-plan, j'aimerais qu'il y ait un petit truc en bas a gauche, des l'ecran d'accueil, avec une barre de
/// progression : Recherche de mise a jour / Aucune mise a jour / Mise a jour en cours (avec la barre) / Redemarrer pour
/// mettre a jour »). Le moteur tient ou en est la recherche de CE processus (`chercher_en_fond` pose `RECHERCHE`) ;
/// l'accueil et la page d'une compagnie la lisent par le geste `maj` du lanceur (`interface::Fenetre::lanceur`) et
/// n'affichent que ce que `etat` leur dit : la phrase, la barre, et s'il faut relire (`en_cours`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Etape {
    /// Aucune recherche dans ce processus (un `moteur route`, un essai) : rien a montrer.
    Aucune,
    Recherche,
    /// Rien de plus recent pour cette installation.
    AJour,
    /// L'archive arrive : `recus` sur `attendus`.
    Telechargement,
    /// Recue : condensat, signature, deballage, controle sur chaque compagnie.
    Preparation,
    /// Prete : elle s'applique au redemarrage.
    Prete,
    /// Quelque chose n'a pas marche ; `mot` le dit.
    Echec,
}

#[derive(Debug)]
struct Point {
    etape: Etape,
    version: Option<String>,
    recus: u64,
    attendus: Option<u64>,
    mot: String,
    /// La phrase `calcul` de la version prete : ce qu'elle change d'un calcul de paie (vide : rien).
    calcul: String,
    /// La raison `exigee` de la version prete : elle est indispensable avant de declarer (vide : elle ne l'est pas).
    exigee: String,
    /// Ce que la version en place, installee juste avant ce lancement, change d'un calcul de paie
    /// (`dire_l_installation`) : vide, rien a dire. Il tient tout le lancement, quelle que soit l'etape.
    installee: String,
    sans_ce_systeme: Option<String>,
    /// Un echec `HORS_LIGNE` que la recherche reprendra d'elle-meme : quand (`chercher_tant_qu_il_tourne`).
    nouvel_essai: Option<std::time::Instant>,
}

impl Point {
    const fn vide() -> Point {
        Point {
            etape: Etape::Aucune,
            version: None,
            recus: 0,
            attendus: None,
            mot: String::new(),
            calcul: String::new(),
            exigee: String::new(),
            installee: String::new(),
            sans_ce_systeme: None,
            nouvel_essai: None,
        }
    }
}

/// Ce que la page attend entre deux lectures de l'indicateur : pendant une recherche, pendant qu'un nouvel essai est
/// prevu (le reseau peut revenir), et pendant que la recherche de fond tourne (« a jour » : une version
/// trouvee entre deux tours doit se voir sans recharger la page, `RYTHME_A_JOUR`). Une lecture ne fait que lire `RECHERCHE` :
/// aucune recherche, aucun fil. Une version prete, ou un echec definitif : plus rien ne change, elle ne relit pas.
const RELIRE_EN_COURS_MS: u64 = 700;
const RELIRE_EN_ATTENTE_MS: u64 = 10_000;
const RELIRE_EN_FOND_MS: u64 = 30_000;

/// LE RYTHME DE LA RECHERCHE quand le logiciel est a jour (Leandre, 21/10/2026 : la 2026.10.21, publiee a 11 h 40, ne
/// se voyait pas dans un logiciel lance avant, qui restait « Logiciel a jour » ; « ca coute rien » : toutes les 5 minutes).
/// Un tour ne coute qu'une demande de `derniere.json` ; le fil dort entre deux (`chercher_tant_qu_il_tourne`).
pub const RYTHME_A_JOUR: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// LE RYTHME DES NOUVEAUX ESSAIS quand le depot ne repond pas (ticket #217, Leandre, 20/10/2026 : le logiciel demarre
/// sans reseau et reste « hors ligne » alors que le poste est connecte). Apres le n-ieme echec, la recherche attend la
/// n-ieme duree, puis la derniere a chaque fois : un essai par 5 minutes au plus lent, jamais plus d'un par 30 secondes.
/// Un essai hors ligne ne coute qu'une demande de `derniere.json` qui echoue ; le fil dort entre deux.
pub const ATTENTES_HORS_LIGNE: [std::time::Duration; 4] = [
    std::time::Duration::from_secs(30),
    std::time::Duration::from_secs(60),
    std::time::Duration::from_secs(120),
    std::time::Duration::from_secs(300),
];

/// L'etat d'une recherche (celle du lancement : `RECHERCHE`).
#[derive(Debug)]
pub struct Recherche(std::sync::Mutex<Point>);

/// La recherche du lancement, celle que l'indicateur montre.
pub static RECHERCHE: Recherche = Recherche::nouvelle();

impl Default for Recherche {
    fn default() -> Self {
        Recherche::nouvelle()
    }
}

impl Etape {
    /// Le nom que l'etat donne a l'etape.
    pub fn nom(self) -> &'static str {
        match self {
            Etape::Aucune => "aucune",
            Etape::Recherche => "recherche",
            Etape::AJour => "a_jour",
            Etape::Telechargement => "telechargement",
            Etape::Preparation => "preparation",
            Etape::Prete => "prete",
            Etape::Echec => "echec",
        }
    }
}

impl Recherche {
    pub const fn nouvelle() -> Recherche {
        Recherche(std::sync::Mutex::new(Point::vide()))
    }

    /// Passe a `etape` (ce qui tient a l'etape d'avant s'efface, sauf la version si `version` n'en dit pas), puis `plus`.
    fn poser(&self, etape: Etape, version: Option<&str>, plus: impl FnOnce(&mut Point)) {
        let mut p = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if p.etape != etape {
            let garde = version.map(String::from).or_else(|| if etape == Etape::AJour { None } else { p.version.take() });
            let installee = std::mem::take(&mut p.installee);
            *p = Point { etape, version: garde, installee, ..Point::vide() };
        } else if let Some(v) = version {
            p.version = Some(v.to_string());
        }
        plus(&mut p);
    }

    /// La version `v` est prete (`maj/prete.json`) : elle s'applique au redemarrage ; sa phrase `calcul` et sa raison
    /// `exigee` se gardent pour l'indicateur.
    fn prete(&self, inst: &Path, v: &str) {
        let d = prete(inst);
        let texte = |cle: &str| d.as_ref().and_then(|p| p.get(cle).and_then(Value::as_str).map(str::trim).map(String::from)).unwrap_or_default();
        let (calcul, exigee) = (texte("calcul"), texte("exigee"));
        self.poser(Etape::Prete, Some(v), |p| {
            p.calcul = calcul;
            p.exigee = exigee;
        });
    }

    /// Ce que la version en place, installee juste avant ce lancement, change d'un calcul de paie (`dire_l_installation`).
    fn installee(&self, phrase: String) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).installee = phrase;
    }

    pub fn etape(&self) -> Etape {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).etape
    }

    /// La recherche a echoue faute de reseau (`HORS_LIGNE`) : c'est le seul echec qu'un nouvel essai peut lever.
    fn hors_ligne(&self) -> bool {
        let p = self.0.lock().unwrap_or_else(|e| e.into_inner());
        p.etape == Etape::Echec && p.mot == HORS_LIGNE
    }

    /// Ce que l'indicateur montre, tout decide ici : `etape`, `texte` (vide : rien a montrer), `detail` (la bulle),
    /// `progression` (0 a 1, ou rien : pas de barre ; une barre sans fin connue est `null` avec `barre`), `en_cours`
    /// (une recherche se fait), `relire_ms` (dans combien la page relit : pendant une recherche, pendant qu'un nouvel essai
    /// est prevu, et tant que la recherche de fond tourne ; `null`, elle se tait), `redemarrer` (le clic est offert),
    /// `exigee` (la version prete est indispensable avant de declarer : la phrase, ou `null`), `installee` (ce que la
    /// version en place, installee juste avant ce lancement, change d'un calcul de paie : la phrase, ou `null`).
    /// Tout se dit positivement, par ce qui est vrai (« Logiciel a jour » plutot qu'« Aucune mise a jour »).
    pub fn etat(&self) -> Value {
        let p = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let lue = p.version.as_deref().map(version_lue);
        let nommee = |t: &str| match &lue {
            Some(v) => format!("PuppetPaie {} {}", v, t),
            None => String::new(),
        };
        let mo = |o: u64| format!("{:.1}", o as f64 / 1_048_576.0).replace('.', ",");
        let essai = p.nouvel_essai.map(|t| {
            let s = t.saturating_duration_since(std::time::Instant::now()).as_secs();
            match s {
                0 => "nouvel essai maintenant".to_string(),
                1..=59 => "nouvel essai dans moins d'une minute".to_string(),
                _ => format!("nouvel essai dans {} min", s.div_ceil(60)),
            }
        });
        let (texte, detail, barre, progression): (&str, String, bool, Option<f64>) = match p.etape {
            Etape::Aucune => ("", String::new(), false, None),
            Etape::Recherche => ("Recherche de mise à jour…", String::new(), false, None),
            Etape::AJour => (
                "Logiciel à jour",
                match (&lue, &p.sans_ce_systeme) {
                    (Some(c), Some(v)) => format!("PuppetPaie {} est la dernière version pour ce système ; la {} est publiée pour les autres.", c, version_lue(v)),
                    (Some(c), None) => format!("PuppetPaie {} est la dernière version.", c),
                    (None, _) => "Cette version est la dernière.".into(),
                },
                false,
                None,
            ),
            Etape::Telechargement => (
                "Mise à jour en cours",
                match p.attendus.filter(|a| *a > 0) {
                    Some(a) => nommee(&format!(": {} Mo sur {} Mo", mo(p.recus), mo(a))),
                    None => nommee(&format!(": {} Mo reçus", mo(p.recus))),
                },
                true,
                p.attendus.filter(|a| *a > 0).map(|a| (p.recus as f64 / a as f64).min(1.0)),
            ),
            Etape::Preparation => ("Mise à jour en cours", nommee(": vérification"), true, Some(1.0)),
            Etape::Prete => (
                "Redémarrer pour mettre à jour",
                if p.calcul.is_empty() {
                    nommee("est prête : elle s'installe au redémarrage.")
                } else {
                    nommee(&format!("est prête : elle s'installe au redémarrage. {}", phrase_du_calcul(&p.calcul)))
                },
                false,
                None,
            ),
            Etape::Echec => (
                "",
                match (&lue, &essai) {
                    (Some(v), Some(e)) => format!("Cette version reste en service ; PuppetPaie {} se prépare dès que le réseau revient : {}.", v, e),
                    (None, Some(e)) => format!("Cette version reste en service ; la recherche reprend dès que le réseau revient : {}.", e),
                    (Some(v), None) => format!("Cette version reste en service ; PuppetPaie {} se prépare de nouveau au prochain lancement.", v),
                    (None, None) => "Cette version reste en service ; la recherche reprend au prochain lancement.".into(),
                },
                false,
                None,
            ),
        };
        let texte = if p.etape == Etape::Echec { format!("Mise à jour : {}", p.mot) } else { texte.to_string() };
        let en_cours = matches!(p.etape, Etape::Recherche | Etape::Telechargement | Etape::Preparation);
        let relire_ms = if en_cours {
            Some(RELIRE_EN_COURS_MS)
        } else if p.etape == Etape::Echec && p.nouvel_essai.is_some() {
            Some(RELIRE_EN_ATTENTE_MS)
        } else if p.etape == Etape::AJour {
            Some(RELIRE_EN_FOND_MS)
        } else {
            None
        };
        json!({
            "etape": p.etape.nom(),
            "texte": texte,
            "detail": detail,
            "barre": barre,
            "progression": progression,
            "en_cours": en_cours,
            "relire_ms": relire_ms,
            "redemarrer": p.etape == Etape::Prete,
            "version": lue,
            // Une version prete EXIGEE : les depots reels la refusent tant qu'elle n'est pas installee
            // (`depot_acte::exigee_en_attente`, qui lit le meme `prete.json`). La phrase se lit sans survol.
            "exigee": (p.etape == Etape::Prete && !p.exigee.is_empty())
                .then(|| format!("Elle est indispensable avant de déclarer ({}) : aucun dépôt réel ne part tant qu'elle n'est pas installée.", p.exigee)),
            "installee": (!p.installee.is_empty()).then(|| p.installee.clone()),
        })
    }
}

/// « Elle change un calcul de paie : … », la phrase `calcul` de « Publier » terminee par un point.
fn phrase_du_calcul(calcul: &str) -> String {
    let c = calcul.trim();
    let point = if c.ends_with(['.', '!', '?', '…']) { "" } else { "." };
    format!("Elle change un calcul de paie : {}{}", c, point)
}

/// LA VERSION QUI VIENT DE S'INSTALLER se dit au lancement qui suit, une fois. La bascule (`appliquer_depuis`) ecrit
/// `maj/appliquee.json` avec la phrase `calcul` et `vue: false` ; lu ici, il passe a `vue: true` et ne se redit plus.
/// Une version qui change un calcul de paie se dit pour tout ce lancement (`Point::installee`) : « PuppetPaie X est en
/// place. Elle change un calcul de paie : … » ; une version sans phrase `calcul` n'a rien de plus a dire que « Logiciel
/// a jour ». Si `vue` ne s'ecrit pas, la phrase se dit quand meme : mieux vaut la relire que ne jamais la lire.
fn dire_l_installation(inst: &Path, suivi: &Recherche) {
    let chemin = dossier_maj(inst).join("appliquee.json");
    let Some(mut a) = lire_json(&chemin) else { return };
    if a.get("vue").and_then(Value::as_bool) != Some(false) {
        return;
    }
    a["vue"] = json!(true);
    if let Err(e) = ecrire_json(&chemin, &a) {
        noter(inst, &format!("{} : {}", chemin.display(), e));
    }
    let calcul = a.get("calcul").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    if calcul.is_empty() {
        return;
    }
    let version = version_lue(a.get("version").and_then(Value::as_str).unwrap_or_default());
    suivi.installee(format!("PuppetPaie {} est en place. {}", version, phrase_du_calcul(calcul)));
}

/// Le geste « Redemarrer pour mettre a jour » : la version prete part au relais (comme au lancement, `au_lancement`) ; a
/// l'appelant de s'arreter aussitot, sans rien ouvrir (le relais attend sa fin, bascule et ouvre la neuve). Refuse si la
/// recherche de ce processus n'a rien de pret : rien n'est lance.
pub fn redemarrer_pour_mettre_a_jour(inst: &Path, suivi: &Recherche) -> R<String> {
    let v = match a_installer(inst) {
        Some(v) if suivi.etape() == Etape::Prete => v,
        _ => return Err("Cette version reste en service jusqu'à ce qu'une nouvelle soit prête.".into()),
    };
    lancer_relais(inst, &v).map_err(|e| {
        noter(inst, &format!("Mise a jour {} non appliquee : {}", version_lue(&v), e));
        format!("La mise à jour ne s'installe pas : {}", e)
    })?;
    noter(inst, &format!("Redemarrage demande pour la mise a jour {}.", version_lue(&v)));
    Ok(v)
}

/// La version prete et plus recente que celle en place : celle qui s'installe au lancement suivant ; ou `None`.
pub fn a_installer(inst: &Path) -> Option<String> {
    let p = prete(inst)?;
    let v = p["version"].as_str()?.to_string();
    plus_recente(&v, &courante(&logiciel_de(inst))).then_some(v)
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
pub fn appliquer(inst: &Path) -> R<Value> {
    appliquer_depuis(&dossier_maj(inst), &logiciel_de(inst))
}

/// `appliquer`, le dossier de la version prete et le logiciel donnes : `<installation>/maj/`, ou, pour le relais lance par
/// une version d'avant le lot U1, le `maj/` de la compagnie ou elle l'avait rangee.
fn appliquer_depuis(maj: &Path, code: &Path) -> R<Value> {
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let d = (|| {
        let d = lire_json(&maj.join("prete.json"))?;
        maj.join(d.get("version")?.as_str()?).join("logiciel").join("donnees").is_dir().then_some(d)
    })()
    .ok_or("aucune version prete a appliquer")?;
    let version = d["version"].as_str().unwrap_or_default().to_string();
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
    let quand = socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].to_string();
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

/// Ce qu'une installation retient de la livraison de fictives qu'elle a : `fictives/.livraison.json`,
/// `{"empreinte": <celle de toute la livraison>, "posees": {<nom>: <empreinte de cette fictive livree>}}`. Un nom qui
/// commence par un point : ni une fictive, ni livre par `fictives_livrees`.
pub const LIVRAISON: &str = ".livraison.json";
/// LA MARQUE D'UNE FICTIVE POSEE PAR UNE LIVRAISON : `fictives/<nom>/.livree.json`, `{"empreinte": …}`, l'empreinte de
/// cette fictive telle que livree. Elle voyage avec la fictive (photo d'origine, remise a zero) ; une copie renommee la
/// porte aussi, mais sous un autre nom. Une livraison ne reconnait comme sien que le dossier dont le NOM est dans
/// `posees` ET dont la marque dit L'EMPREINTE qu'elle y a retenue (`poser_les_fictives_pretes`).
pub const LIVREE: &str = ".livree.json";

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
fn dire_les_archives(r: &Value) -> String {
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

/// AU LANCEMENT, avant toute fenetre : une version prete part au relais. Rend `true` si le relais
/// est parti (le programme doit alors s'arreter sans rien ouvrir). Ce qui echoue s'ecrit dans le journal de la mise a
/// jour, et l'ancienne version s'ouvre : une mise a jour ne doit jamais empecher une paie.
pub fn au_lancement() -> bool {
    let inst = installation();
    let Some(v) = a_installer(&inst) else { return false };
    match lancer_relais(&inst, &v) {
        Ok(_) => true,
        Err(e) => {
            noter(&inst, &format!("Mise a jour {} non appliquee : {}", version_lue(&v), e));
            false
        }
    }
}

/// AU LANCEMENT, apres `au_lancement` : la version qui vient de s'installer se dit (`dire_l_installation`), puis le
/// logiciel cherche pour lui-meme, en fond, sans attendre qu'une compagnie s'ouvre ; ce qu'il trouve s'installe au
/// lancement suivant (ou au clic « Redemarrer pour mettre a jour »). Ce qui s'est passe va dans le journal de la mise a
/// jour ; ou elle en est, dans `RECHERCHE` (l'indicateur). Elle continue tant que le logiciel tourne
/// (`chercher_tant_qu_il_tourne`). Un seul fil par processus : un second appel ne lance rien.
pub fn chercher_en_fond() {
    static LANCEE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if LANCEE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let inst = installation();
    dire_l_installation(&inst, &RECHERCHE);
    RECHERCHE.poser(Etape::Recherche, None, |_| {});
    std::thread::spawn(move || chercher_tant_qu_il_tourne(&inst, &cles_de_l_editeur(), &RECHERCHE, &ATTENTES_HORS_LIGNE, RYTHME_A_JOUR));
}

/// LA RECHERCHE CONTINUE TANT QUE LE LOGICIEL TOURNE. `verifier_suivi`, puis, selon ce qu'elle laisse dans `suivi` :
/// - un echec `HORS_LIGNE` (le depot ne repond pas, ou la reception a ete coupee ; ticket #217, Leandre, 20/10/2026 : le
///   logiciel lance sans reseau restait « hors ligne » alors que le poste etait connecte) : un nouvel essai apres
///   `attentes[n]` (la derniere se repete : `ATTENTES_HORS_LIGNE`). Le moment du prochain essai est dans `suivi` : la
///   bulle le dit, et la page relit tant qu'il est prevu (`relire_ms`) ;
/// - « a jour » : un tour calme (`verifier_en`) toutes les `rythme` (`RYTHME_A_JOUR`). Une version publiee
///   entre-temps est trouvee, telechargee et preparee ; l'indicateur passe a « Redemarrer pour mettre a jour ». Rien ne
///   s'installe : elle attend le redemarrage ;
/// - « prete », ou un echec qu'un nouvel essai ne leverait pas (signature refusee, archive alteree, depot introuvable) :
///   la boucle s'arrete. Une version prete ne se remplace pas sous le bouton « Redemarrer » : le clic installe celle que
///   l'indicateur nomme, et le lancement d'apres recherche de nouveau (une plus recente s'y trouve en un tour).
///
/// Le journal de la mise a jour recoit chaque tour dont la phrase change (pas une ligne par tour). Bloquante : a lancer
/// dans un fil.
pub fn chercher_tant_qu_il_tourne(inst: &Path, cles: &[&str], suivi: &Recherche, attentes: &[std::time::Duration], rythme: std::time::Duration) {
    let mut n = 0usize;
    let mut calme = false;
    let mut notee = String::new();
    loop {
        let dit = verifier_en(inst, true, cles, suivi, calme);
        if dit != notee {
            noter(inst, &format!("Recherche : {}", dit));
            notee = dit;
        }
        let attente = match suivi.etape() {
            Etape::AJour => {
                n = 0;
                calme = true;
                rythme
            }
            _ if suivi.hors_ligne() => {
                let Some(a) = attentes.get(n).or(attentes.last()).copied() else { return };
                n += 1;
                calme = false;
                suivi.0.lock().unwrap_or_else(|e| e.into_inner()).nouvel_essai = Some(std::time::Instant::now() + a);
                a
            }
            _ => return,
        };
        std::thread::sleep(attente);
    }
}

/// Le journal de la mise a jour (et de ce que le lancement range dans l'installation) : `<installation>/maj/journal.log`,
/// une ligne datee par evenement.
pub fn noter(inst: &Path, t: &str) {
    let d = dossier_maj(inst);
    let _ = std::fs::create_dir_all(&d);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("journal.log")) {
        let quand = socle::paris::iso_de_naif(socle::paris::maintenant_naif());
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

/// Attend la fin du processus `pid` (60 s au plus). Vrai s'il est parti. Sous Windows, le processus lui-meme (son numero,
/// qui ne se confond avec aucun autre) ; `_inst` n'y sert pas.
#[cfg(windows)]
fn attendre_fin(pid: u32, _inst: &Path) -> bool {
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

/// Hors Windows, la prise du verrou n'est jamais heritee (CLOEXEC) : quand LA FENETRE DE CETTE INSTALLATION ne repond plus
/// a son creneau (`interface::instance::ouverte`, qui compare le dossier de l'installation), elle est partie. Une autre
/// installation ouverte sur le poste (ticket #244) a son propre creneau, et ne se confond jamais avec elle.
#[cfg(not(windows))]
fn attendre_fin(_pid: u32, inst: &Path) -> bool {
    for _ in 0..120 {
        if !crate::interface::instance::ouverte(inst, crate::interface::instance::Plage::DU_POSTE) {
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
    if !attendre_fin(pid, &inst) {
        noter(&inst, "Mise a jour non appliquee : le programme qui l'a lancee ne s'est pas arrete.");
    } else {
        let journal = |t: &str| noter(&inst, t);
        match appliquer_depuis(&maj, &code) {
            Ok(r) => journal(&format!("Mise a jour appliquee : PuppetPaie {} (la precedente est gardee dans .avant/).", version_lue(r["version"].as_str().unwrap_or("?")))),
            Err(e) => journal(&format!("Mise a jour non appliquee : {}", e)),
        }
    }
    let _ = std::process::Command::new(programme(&code, "puppetpaie")).spawn();
    0
}

/// `moteur maj [--sans-telecharger] [--appliquer] [--revenir] [--publier [DEPOT] --notes T --calcul T
/// --exigee T] [--cle [--remplacer]] [--cle-github [DOSSIER]] [--publier-github --logiciel L --sortie S --notes T
/// --calcul T --exigee T] [--marquer-fictives --logiciel L]`.
pub fn commande(args: &[String]) -> i32 {
    // D'abord savoir ou l'on est installe : sans cela, `installation()` retombe sur le dossier ou le programme a ete
    // compile (un chemin du conteneur, que Windows lit sur le lecteur courant : le 14/10/2026, la graine de `--cle-github`
    // est allee dans `D:\home\user\puppetpaieprivate\maj\`).
    crate::commandes::compagnie::fixer_le_code();
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned();
    // Les gestes de la publication par GitHub (lot P1) : ni compagnie ni coffre.
    let github = if drapeau("--cle-github") {
        let dossier = valeur("--cle-github").map(PathBuf::from).unwrap_or_else(|| dossier_maj(&installation()));
        Some(generer_cle_github(&dossier).map(|r| {
            format!(
                "Cle de GitHub fabriquee.\nLa GRAINE (secrete) est dans : {}\n  l'ouvrir, copier tout son contenu dans le secret PUPPETPAIE_CLE_SIGNATURE de l'environnement \
                 « publication » du depot public, puis EFFACER ce fichier.\nLa cle publique (pas secrete), aussi dans {} :\n  {}\n  a reporter dans moteur/src/compagnie/maj.rs, CLE_PUBLIQUE_GITHUB.",
                r["graine"].as_str().unwrap_or(""),
                r["publique"].as_str().unwrap_or(""),
                r["cle_publique"].as_str().unwrap_or("")
            )
        }))
    } else if drapeau("--publier-github") {
        match (valeur("--logiciel"), valeur("--sortie")) {
            (Some(l), Some(s)) => Some(
                publier_depuis_l_environnement(Path::new(&l), Path::new(&s), &valeur("--notes").unwrap_or_default(), &valeur("--calcul").unwrap_or_default(), &valeur("--exigee").unwrap_or_default())
                    .map(|r| format!("Signee : {} ({}) dans {}", r["version"].as_str().unwrap_or(""), dire_les_archives(&r), r["dossier"].as_str().unwrap_or(""))),
            ),
            _ => Some(Err("--publier-github demande --logiciel DOSSIER et --sortie DOSSIER".into())),
        }
    } else if drapeau("--marquer-fictives") {
        Some(match valeur("--logiciel") {
            Some(l) => marquer_les_fictives(Path::new(&l)).map(|e| format!("Fictives marquees : livraison {}.", e)),
            None => Err("--marquer-fictives demande --logiciel DOSSIER".into()),
        })
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
    let racine = crate::routes::ouverture::racine_du_poste();
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
                "{}\nCle publique, a reporter dans moteur/src/compagnie/maj.rs, CLE_PUBLIQUE :\n  {}{}",
                ecrite,
                r["cle_publique"].as_str().unwrap_or(""),
                if r["correspond"] == json!(true) { "" } else { "\n  (le code porte une autre cle publique : tant qu'elle n'est pas reportee, `publier` refuse)" }
            )
        })
    } else if drapeau("--publier") {
        publier(&racine, valeur("--publier").as_deref(), &valeur("--notes").unwrap_or_default(), &valeur("--calcul").unwrap_or_default(), &valeur("--exigee").unwrap_or_default())
            .map(|r| {
                let t = format!("Publiee : {} ({}) dans {}", version_lue(r["version"].as_str().unwrap_or("")), dire_les_archives(&r), r["dossier"].as_str().unwrap_or(""));
                match r["a_l_ancienne"].as_str() {
                    Some(a) => format!("{}\n{}", t, a),
                    None => t,
                }
            })
    } else if drapeau("--appliquer") {
        if crate::interface::instance::ouverte(&installation(), crate::interface::instance::Plage::DU_POSTE) {
            Err("PuppetPaie tourne : fermer la fenetre, puis appliquer".into())
        } else {
            appliquer(&installation())
                .map(|r| format!("Version {} en place ; la precedente est dans {}", version_lue(r["version"].as_str().unwrap_or("")), r["avant"].as_str().unwrap_or("")))
        }
    } else if drapeau("--revenir") {
        if crate::interface::instance::ouverte(&installation(), crate::interface::instance::Plage::DU_POSTE) {
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

    /// Une compagnie fictive livree a cote du logiciel : `<base>/fictives/fictive-essai/` (son marqueur et sa fiche), et le
    /// LISEZ-MOI du dossier.
    fn une_fictive(base: &Path) {
        let f = base.join("fictives").join("fictive-essai");
        std::fs::create_dir_all(f.join("config")).unwrap();
        std::fs::write(f.join("COPIE-DE-DEVELOPPEMENT.json"), "{\"copie_de\": \"compagnie fictive (essai)\"}").unwrap();
        std::fs::write(f.join("config").join("entreprise.json"), "{\"nom_affichage\": \"Essai\"}").unwrap();
        std::fs::write(base.join("fictives").join("LISEZ-MOI.md"), "Les fictives.\n").unwrap();
    }

    fn noms_de(octets: &[u8]) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).unwrap();
        (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect()
    }

    /// Une archive qui porterait un `EDITION.json` (publiee avant le lot V1) ne change pas l'installation : un paquet soude
    /// garde son `EDITION.json` ; une installation sans `EDITION.json` n'en recoit pas (elle reste bridee ou debloquee par son
    /// `.edition.json`, a cote de `logiciel/`, que la bascule ne touche pas).
    #[test]
    fn la_bascule_laisse_l_edition_de_l_installation_comme_elle_etait() {
        for avant in [Some("{\"edition\": \"bride\"}"), None] {
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
            appliquer(&racine).unwrap();
            assert_eq!(courante(&code), "2026.11.01");
            assert_eq!(std::fs::read_to_string(code.join("EDITION.json")).ok().as_deref(), avant);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    /// Aucune des deux archives ne porte l'`EDITION.json` du logiciel de l'editeur.
    #[test]
    fn l_archive_ne_porte_pas_l_edition() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-archive-edition-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let code = base.join("logiciel");
        arbre(&code, "2026.11.01");
        une_fictive(&base);
        std::fs::write(code.join("EDITION.json"), "{}").unwrap();
        std::fs::write(code.join("donnees").join("EDITION.json"), "{}").unwrap();
        let noms = noms_de(&archiver(&code, TOUT_SYSTEME).unwrap());
        assert!(!noms.contains(&"EDITION.json".to_string()) && noms.contains(&"VERSION".to_string()), "{:?}", noms);
        assert!(noms.contains(&"donnees/EDITION.json".to_string()), "seul celui de la racine du logiciel est ecarte");
        let noms = noms_de(&archiver_installation(&code, TOUT_SYSTEME).unwrap());
        assert!(!noms.contains(&"logiciel/EDITION.json".to_string()) && noms.contains(&"logiciel/VERSION".to_string()), "{:?}", noms);
        assert!(noms.contains(&"logiciel/donnees/EDITION.json".to_string()), "seul celui de la racine du logiciel est ecarte");
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
        std::fs::write(code.join("EDITION.json"), "{\"edition\": \"bride\"}").unwrap();
        let neuf = racine.join("maj").join("2026.11.01").join("logiciel");
        arbre(&neuf, "2026.11.01");
        ecrire_json(&racine.join("maj").join("prete.json"), &json!({"version": "2026.11.01", "calcul": ""})).unwrap();
        ECHOUER_SUR.with(|e| *e.borrow_mut() = Some(code.join(".avant").join("EDITION.json")));
        let r = appliquer(&racine);
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
        // L'archive de mise a jour ne demande aucune fictive ; celle d'installation, si : decompressee, elle s'ouvrirait
        // bridee sur un accueil vide.
        let octets = archiver(&code, TOUT_SYSTEME).unwrap();
        assert!(archiver_installation(&code, TOUT_SYSTEME).unwrap_err().contains("aucune compagnie fictive"));
        une_fictive(&base);
        // Ce qui est du poste et pas de la fictive livree : sa photo d'origine, ses secrets, le journal
        // de la fenetre ; et un dossier qui n'est pas une fictive.
        let f = base.join("fictives");
        for (rel, t) in [
            (".origine/fictive-essai/config/entreprise.json", "{}"),
            ("fictive-essai/config/coffre.bin", "scelle"),
            ("fictive-essai/journal/interface.log", "chemin du poste"),
            ("pas-une-fictive/config/entreprise.json", "{}"),
        ] {
            std::fs::create_dir_all(f.join(rel).parent().unwrap()).unwrap();
            std::fs::write(f.join(rel), t).unwrap();
        }
        // LE CONTRAT : le contenu de `logiciel/` a la racine, rien d'autre, et les fictives n'y changent rien.
        assert_eq!(octets, archiver(&code, TOUT_SYSTEME).unwrap(), "le zip est deterministe, et sans fictives");
        let mut noms = noms_de(&octets);
        noms.sort();
        assert_eq!(noms, ["VERSION", "donnees/a.json"]);
        let installation = archiver_installation(&code, TOUT_SYSTEME).unwrap();
        let noms = noms_de(&installation);
        assert!(noms.contains(&"logiciel/VERSION".to_string()) && noms.contains(&"logiciel/donnees/a.json".to_string()), "{:?}", noms);
        assert!(noms.contains(&"fictives/LISEZ-MOI.md".to_string()) && noms.contains(&"fictives/fictive-essai/config/entreprise.json".to_string()), "{:?}", noms);
        assert!(noms.contains(&"fictives/fictive-essai/COPIE-DE-DEVELOPPEMENT.json".to_string()), "{:?}", noms);
        // Les trois fichiers livres, la marque de la fictive et ce que retient la livraison : rien du poste.
        assert!(noms.contains(&"fictives/fictive-essai/.livree.json".to_string()) && noms.contains(&"fictives/.livraison.json".to_string()), "{:?}", noms);
        assert_eq!(noms.iter().filter(|n| n.starts_with("fictives/")).count(), 5, "rien du poste : {:?}", noms);
        assert_eq!(installation, archiver_installation(&code, TOUT_SYSTEME).unwrap(), "le zip est deterministe");
        // L'archive des fictives porte exactement le `fictives/` de l'archive d'installation ; son empreinte ne depend que
        // du contenu livre.
        let (fictives, empreinte) = archiver_fictives(&code).unwrap();
        assert_eq!(noms_de(&fictives), noms.iter().filter(|n| n.starts_with("fictives/")).cloned().collect::<Vec<_>>());
        std::fs::write(f.join("fictive-essai/config/coffre.bin"), "autre scelle").unwrap();
        assert_eq!(archiver_fictives(&code).unwrap().1, empreinte, "ce que la livraison ne porte pas ne change pas l'empreinte");
        std::fs::write(f.join("fictive-essai/config/entreprise.json"), "{\"nom_affichage\": \"Autre\"}").unwrap();
        assert_ne!(archiver_fictives(&code).unwrap().1, empreinte, "ce qu'elle porte, si");
        let graine = [7u8; 32];
        let k = ed25519_dalek::SigningKey::from_bytes(&graine);
        let publique = socle::base64::ecrire(k.verifying_key().as_bytes());
        let sig = signer(&octets, &graine).unwrap();
        assert!(signature_valide(&octets, &sig, &publique));
        assert!(!signature_valide(&octets, &sig, CLE_PUBLIQUE));
        let mut altere = octets.clone();
        let n = altere.len() / 2;
        altere[n] ^= 1;
        assert!(!signature_valide(&altere, &sig, &publique));
        let neuf = base.join("maj-neuve").join("logiciel");
        deballer(&octets, &neuf).unwrap();
        assert_eq!(courante(&neuf), "2026.10.09");
        assert!(neuf.join("donnees").join("a.json").is_file());
        assert!(!neuf.join("target").exists());
        // Une archive d'installation n'est pas une mise a jour : refusee, rien n'est ecrit.
        let ailleurs = base.join("maj-installation").join("logiciel");
        assert!(deballer(&installation, &ailleurs).unwrap_err().contains("archive d'installation"));
        assert!(!ailleurs.exists());
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

    /// Ce que recoit `suivre` en lisant le depot (la barre de l'indicateur, ticket #210) : des octets recus qui ne
    /// font que croitre, jusqu'a la taille entiere, et la taille attendue telle que le depot la dit (un dossier : celle
    /// du fichier ; un serveur : son `Content-Length`). Un fichier absent est `introuvable`, un serveur muet `hors ligne`.
    fn suivi_de(depot: &str, nom: &str) -> (Result<Vec<u8>, Echec>, Vec<(u64, Option<u64>)>) {
        let vus = std::sync::Mutex::new(Vec::new());
        let r = lire_depot_suivi(depot, nom, &|recus, attendus| vus.lock().unwrap().push((recus, attendus)));
        (r, vus.into_inner().unwrap())
    }

    fn croissants_jusqu_a(vus: &[(u64, Option<u64>)], n: u64) {
        assert!(vus.windows(2).all(|w| w[0].0 <= w[1].0), "les octets recus ne reculent pas");
        assert_eq!(vus.first().map(|v| v.0), Some(0));
        assert_eq!(vus.last().copied(), Some((n, Some(n))), "la barre finit pleine");
        assert!(vus.iter().all(|v| v.1 == Some(n)), "la taille attendue est celle que dit le depot");
    }

    #[test]
    fn la_lecture_du_depot_dit_ce_qu_elle_a_recu() {
        let octets: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let d = std::env::temp_dir().join(format!("paie-maj-suivi-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.zip"), &octets).unwrap();
        let (r, vus) = suivi_de(&d.display().to_string(), "a.zip");
        assert_eq!(r.unwrap(), octets);
        croissants_jusqu_a(&vus, octets.len() as u64);
        assert_eq!(suivi_de(&d.display().to_string(), "absente.zip").0.unwrap_err().0, INTROUVABLE);
        let _ = std::fs::remove_dir_all(&d);

        let ecoute = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let servis = octets.clone();
        let serveur = std::thread::spawn(move || {
            let (mut c, _) = ecoute.accept().unwrap();
            let mut tete = Vec::new();
            let mut b = [0u8; 1];
            while !tete.ends_with(b"\r\n\r\n") && c.read(&mut b).unwrap() == 1 {
                tete.push(b[0]);
            }
            write!(c, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", servis.len()).unwrap();
            c.write_all(&servis).unwrap();
        });
        let (r, vus) = suivi_de(&format!("http://127.0.0.1:{}", port), "a.zip");
        serveur.join().unwrap();
        assert_eq!(r.unwrap(), octets);
        croissants_jusqu_a(&vus, octets.len() as u64);

        let libre = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap().local_addr().unwrap().port();
        assert_eq!(suivi_de(&format!("http://127.0.0.1:{}", libre), "a.zip").0.unwrap_err().0, HORS_LIGNE);
    }
}
