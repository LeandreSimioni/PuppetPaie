//! `moteur guet` : le guet de l'orchestrateur, combien de contexte le sous-agent a-t-il consomme ?
//! Un outil d'atelier.
//!
//! Lit toutes les 30 s le dernier `usage` du transcript du sous-agent (et de celui de l'orchestrateur), jamais un
//! transcript entier : la fin du fichier seulement. Il SORT quand il faut reveiller l'orchestrateur, qui le lance en
//! arriere-plan avec le sous-agent :
//!
//! - le sous-agent passe `--seuil` (300 000 par defaut) ;
//! - avec `--commits`, un nouveau commit apparait sur HEAD (le moment propre pour un arret) ;
//! - l'orchestrateur passe 300 000 ;
//! - le transcript du sous-agent n'a plus bouge depuis 20 minutes (fini ou bloque).
//!
//! OU SONT LES TRANSCRIPTS : sous `~/.claude/projects/<dossier ou la seance a ete lancee, encode>/`, le fichier
//! `<seance>.jsonl` pour l'orchestrateur, `<seance>/subagents/*.jsonl` pour ses sous-agents. L'encodage remplace `/`
//! par `-`, et aussi tout autre caractere hors `[A-Za-z0-9]` (un `\`, un `:`, un point), par inference : c'est ce
//! qu'on voit sur les dossiers deja crees, pas un texte lu. La seance peut etre lancee depuis le depot ou depuis un de
//! ses parents : le guet essaie le depot puis chacun de ses parents, et garde celui des
//! dossiers dont le dernier `.jsonl` est le plus recent. `--transcript` force le fichier du sous-agent.
//!
//! ```text
//! moteur guet                              le dernier sous-agent lance, seuil 300 000
//! moteur guet --seuil 330000 --commits
//! moteur guet --transcript CHEMIN.jsonl
//! ```
//!
//! Ne l'arreter jamais par `pkill -f guet` : le motif tue le shell qui le lance.

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Le seuil de l'orchestrateur, fixe, et celui du sous-agent par defaut.
const SEUIL: u64 = 300_000;
/// Ce qu'on lit de la fin d'un transcript : assez pour y trouver le dernier tour.
const FIN_LUE: u64 = 400_000;
/// Le pas de la veille.
const PAS: Duration = Duration::from_secs(30);
/// Un sous-agent dont le transcript n'a pas bouge depuis ce temps est fini ou bloque.
const IMMOBILE: Duration = Duration::from_secs(1200);

const AIDE: &str = "usage : moteur guet [--seuil N] [--commits] [--transcript CHEMIN.jsonl]\n\
    \x20 sort quand le sous-agent passe le seuil (300 000 par defaut), quand l'orchestrateur passe 300 000,\n\
    \x20 au prochain commit sur HEAD (--commits), ou apres 20 minutes sans que le transcript du sous-agent bouge.\n\
    \x20 --transcript : le transcript du sous-agent (par defaut : le plus recent).";

struct Options {
    seuil: u64,
    commits: bool,
    transcript: Option<PathBuf>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options { seuil: SEUIL, commits: false, transcript: None };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--commits" => o.commits = true,
            "--seuil" | "--transcript" => {
                let v = args.get(i + 1).ok_or_else(|| format!("{} attend une valeur", args[i]))?;
                if args[i] == "--seuil" {
                    o.seuil = v.replace('_', "").parse().map_err(|_| format!("--seuil : nombre attendu, pas {:?}", v))?;
                } else {
                    o.transcript = Some(PathBuf::from(v));
                }
                i += 1;
            }
            a => return Err(format!("option inconnue : {}", a)),
        }
        i += 1;
    }
    Ok(o)
}

/// `moteur guet ...` : le code de sortie (0 quand il sonne, 1 sans transcript, 2 sur une option fausse).
pub fn commande(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--aide" || a == "-h" || a == "--help") {
        println!("{}", AIDE);
        return 0;
    }
    let o = match options(args) {
        Ok(o) => o,
        Err(m) => {
            eprintln!("{}\n{}", m, AIDE);
            return 2;
        }
    };
    let depot = depot();
    let projets = match dossier_maison() {
        Some(m) => m.join(".claude").join("projects"),
        None => {
            eprintln!("ni HOME ni USERPROFILE : pas de dossier ~/.claude/projects");
            return 1;
        }
    };
    let projet = dossier_des_transcripts(&projets, &depot);
    let moi = projet.as_deref().and_then(|p| plus_recent(jsonl_de(p)));
    let sous_agent = match o.transcript.clone().or_else(|| projet.as_deref().and_then(|p| plus_recent(jsonl_des_sous_agents(p)))) {
        Some(s) => s,
        None => {
            match &projet {
                Some(p) => eprintln!("aucun transcript de sous-agent sous {}", p.display()),
                None => eprintln!("aucun transcript sous {} pour {} ni ses parents", projets.display(), depot.display()),
            }
            return 1;
        }
    };
    println!(
        "guet : {}, seuil {}{}",
        sous_agent.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        o.seuil,
        if o.commits { ", et au prochain commit" } else { "" }
    );

    let depart = head(&depot);
    loop {
        let (s, m) = (dernier_usage(Some(&sous_agent)), dernier_usage(moi.as_deref()));
        if s >= o.seuil {
            println!("SEUIL : sous-agent a {} (>= {})", s, o.seuil);
            return 0;
        }
        if m >= SEUIL {
            println!("SEUIL : orchestrateur a {}", m);
            return 0;
        }
        if o.commits {
            let h = head(&depot);
            if h != depart {
                println!("COMMIT {} : sous-agent a {}", h.get(..8).unwrap_or(&h), s);
                return 0;
            }
        }
        if let Ok(t) = std::fs::metadata(&sous_agent).and_then(|m| m.modified()) {
            if SystemTime::now().duration_since(t).is_ok_and(|d| d > IMMOBILE) {
                println!("IMMOBILE depuis 20 minutes : sous-agent a {}", s);
                return 0;
            }
        }
        std::thread::sleep(PAS);
    }
}

/// Le depot ou l'on est (`git rev-parse --show-toplevel`), a defaut le dossier courant.
fn depot() -> PathBuf {
    let ici = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&ici)
        .output()
        .ok()
        .filter(|s| s.status.success())
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or(ici)
}

fn head(depot: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(depot)
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .unwrap_or_default()
}

fn dossier_maison() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Le nom du dossier de transcripts d'un dossier de lancement : tout caractere hors `[A-Za-z0-9]` devient `-`
/// (`/home/user/PuppetPaie` donne `-home-user-PuppetPaie`).
pub fn nom_du_dossier(chemin: &Path) -> String {
    chemin.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Le dossier de transcripts de la seance : celui du depot ou d'un de ses parents, le plus recent qui porte des `.jsonl`.
pub fn dossier_des_transcripts(projets: &Path, depot: &Path) -> Option<PathBuf> {
    depot
        .ancestors()
        .map(|d| projets.join(nom_du_dossier(d)))
        .filter_map(|p| {
            let t = jsonl_de(&p).iter().filter_map(|f| date(f)).max()?;
            Some((t, p))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

fn date(f: &Path) -> Option<SystemTime> {
    std::fs::metadata(f).and_then(|m| m.modified()).ok()
}

fn plus_recent(fichiers: Vec<PathBuf>) -> Option<PathBuf> {
    fichiers.into_iter().filter_map(|f| Some((date(&f)?, f))).max_by_key(|(t, _)| *t).map(|(_, f)| f)
}

/// Les `*.jsonl` d'un dossier (pas ses sous-dossiers).
fn jsonl_de(dossier: &Path) -> Vec<PathBuf> {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    lecture
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file())
        .collect()
}

/// Les `*/subagents/*.jsonl` d'un dossier de transcripts.
fn jsonl_des_sous_agents(dossier: &Path) -> Vec<PathBuf> {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    lecture.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).flat_map(|p| jsonl_de(&p.join("subagents"))).collect()
}

/// Le contexte du dernier tour d'un transcript (0 s'il n'y en a pas, ou si le fichier ne se lit pas) : lit la fin du
/// fichier seulement.
pub fn dernier_usage(chemin: Option<&Path>) -> u64 {
    let Some(chemin) = chemin else { return 0 };
    let lire = || -> std::io::Result<Vec<u8>> {
        let mut f = std::fs::File::open(chemin)?;
        let taille = f.seek(SeekFrom::End(0))?;
        f.seek(SeekFrom::Start(taille.saturating_sub(FIN_LUE)))?;
        let mut octets = Vec::new();
        f.read_to_end(&mut octets)?;
        Ok(octets)
    };
    lire().map(|o| dernier_usage_d_octets(&o)).unwrap_or(0)
}

/// Le dernier `message.usage` lisible de lignes JSON (la premiere peut etre coupee : elle ne se lit pas, on passe) :
/// la somme de `input_tokens`, `cache_creation_input_tokens` et `cache_read_input_tokens`, un champ absent valant 0.
pub fn dernier_usage_d_octets(octets: &[u8]) -> u64 {
    for ligne in octets.split(|&b| b == b'\n' || b == b'\r').rev() {
        if !ligne.windows(7).any(|w| w == b"\"usage\"") {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(ligne) else { continue };
        let Some(u) = v.get("message").and_then(|m| m.get("usage")).filter(|u| u.is_object()) else { continue };
        let champ = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
        return champ("input_tokens") + champ("cache_creation_input_tokens") + champ("cache_read_input_tokens");
    }
    0
}
