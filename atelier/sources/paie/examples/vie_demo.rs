//! LA VIE D'UNE COMPAGNIE DE DEMONSTRATION, JOUEE PAR LES GESTES DE L'ECRAN (Leandre, 08/10/2026 : « une vie de demo neuve
//! et bien ecrite », « une technique d'ecriture fiable »).
//!
//! L'atelier seulement, jamais livre. Un scenario (`logiciel/outils/demos/*.json`) est une suite DATEE de gestes : chacun
//! est une route du moteur, appelee comme la fenetre l'appelle (`paie route --racine R`, la demande sur l'entree
//! standard), le jour du geste pose dans `PUPPETPAIE_AUJOURDHUI`. Rien ne s'ecrit a cote : la compagnie n'a que ce que
//! le moteur a ecrit. Entre deux gestes, l'horloge de la demonstration (`horloge-rattraper`) joue ce qu'une compagnie
//! bien tenue aurait fait entre-temps (DPAE, contrats signes, bulletins, DSN, documents).
//!
//! Seul le DECOR de depart s'ecrit ici, comme « Ajouter une compagnie » l'ecrit (`interface::compagnies::ajouter` : les
//! trois fiches vides), plus le marqueur qui fait de ce dossier une demonstration (rien ne part en reel).
//!
//!     cargo run --release --example vie_demo -- <scenario.json> <dossier de la compagnie> [paie]
//!
//! Un geste refuse arrete tout, sauf s'il porte `"refus_attendu": true` (on montre alors le refus). Chaque geste et sa
//! reponse sont notes dans `<dossier>/VIE.log`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn ecrire(p: &Path, v: &Value) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
}

/// Une route, comme la fenetre l'appelle.
fn route(paie: &Path, racine: &Path, jour: &str, nom: &str, charge: &Value) -> Value {
    let mut c = Command::new(paie);
    c.arg("route").arg("--racine").arg(racine).env("PUPPETPAIE_AUJOURDHUI", jour);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut p = c.spawn().expect("paie ne se lance pas");
    p.stdin.take().unwrap().write_all(json!({"route": nom, "charge": charge}).to_string().as_bytes()).unwrap();
    let sortie = p.wait_with_output().unwrap();
    serde_json::from_slice(&sortie.stdout).unwrap_or_else(|_| {
        json!({"ok": false, "erreur": format!("{}{}", String::from_utf8_lossy(&sortie.stdout), String::from_utf8_lossy(&sortie.stderr))})
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("vie_demo <scenario.json> <dossier> [paie]");
        std::process::exit(2);
    }
    let scenario: Value = serde_json::from_str(&std::fs::read_to_string(&args[0]).expect("scenario illisible")).expect("scenario : JSON");
    let racine = PathBuf::from(&args[1]);
    let paie = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/paie"));
    let paie = std::fs::canonicalize(&paie).expect("paie introuvable : cargo build --release");

    // Le decor : ce qu'« Ajouter une compagnie » ecrit, et le marqueur de la demonstration.
    if racine.exists() {
        std::fs::remove_dir_all(&racine).unwrap();
    }
    let nom = scenario["nom"].as_str().expect("scenario : il faut `nom`");
    ecrire(&racine.join("config/entreprise.json"), &json!({"nom_affichage": nom, "raison_sociale": nom.to_uppercase()}));
    ecrire(&racine.join("config/salaries.json"), &json!({}));
    ecrire(&racine.join("config/spectacles.json"), &json!({}));
    ecrire(
        &racine.join("COPIE-DE-DEVELOPPEMENT.json"),
        &json!({"copie_de": format!("compagnie de demonstration {}", nom),
                "_quoi": "Compagnie de DEMONSTRATION (paie/examples/vie_demo.rs). Ce marqueur fait refuser tout acte reel. Ne pas le retirer."}),
    );
    let racine = std::fs::canonicalize(&racine).unwrap();
    let mut log = std::fs::File::create(racine.join("VIE.log")).unwrap();

    for (i, geste) in scenario["gestes"].as_array().expect("scenario : il faut `gestes`").iter().enumerate() {
        let jour = geste["jour"].as_str().expect("un geste sans `jour`");
        let (nom, charge) = match geste.get("horloge") {
            // Jusqu'a ce jour, l'horloge joue ce qui etait du.
            Some(_) => ("horloge-rattraper", json!({"aujourdhui": jour, "retenir": false})),
            None => (geste["route"].as_str().expect("un geste sans `route`"), geste.get("charge").cloned().unwrap_or(json!({}))),
        };
        let r = route(&paie, &racine, jour, nom, &charge);
        let ok = r["ok"] == json!(true);
        let quoi = geste.get("quoi").and_then(Value::as_str).unwrap_or(nom);
        writeln!(log, "{} {} — {}\n{}\n", jour, quoi, if ok { "ok" } else { "REFUS" }, serde_json::to_string_pretty(&r).unwrap()).unwrap();
        if nom == "horloge-rattraper" && ok {
            for phrase in r["resultat"].as_array().into_iter().flatten() {
                println!("      {}", phrase.as_str().unwrap_or(""));
            }
        }
        println!("{:>3}  {}  {}  {}", i + 1, jour, if ok { "ok   " } else { "REFUS" }, quoi);
        if !ok {
            println!("       {}", r["erreur"].as_str().unwrap_or("").replace('\n', "\n       "));
            if geste.get("refus_attendu") != Some(&json!(true)) {
                std::process::exit(1);
            }
        }
    }
}
