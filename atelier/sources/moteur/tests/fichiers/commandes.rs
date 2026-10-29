//! La ligne de commande, rangée par commande : ce que chacune fait sans réseau, et ses refus (une phrase et un code,
//! jamais une trace ni un nom d'exception). Chaque `moteur` part de `env_clear` (ni mot de passe, ni mandataire), sur une
//! copie sous `CARGO_TARGET_TMPDIR`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::outils::{copier, dossier_de_test, dossier_du_processus};
use serde_json::json;

const AUJOURDHUI: &str = "2026-10-08";

/// `logiciel/donnees` et `VERSION`, copiés une fois par processus : `prive/` voisin de `logiciel/` est la compagnie par
/// défaut, et `--dev` cherche son marqueur dans `cle/`. Jamais le dépôt.
fn monde() -> &'static Path {
    static MONDE: OnceLock<PathBuf> = OnceLock::new();
    MONDE.get_or_init(|| {
        let m = dossier_du_processus("commandes");
        let livre = crate::outils::code_livre();
        copier(&livre, &m.join("logiciel/donnees"));
        std::fs::copy(livre.join("../VERSION"), m.join("logiciel/VERSION")).unwrap();
        std::fs::create_dir_all(m.join("cle")).unwrap();
        std::fs::create_dir_all(m.join("home")).unwrap();
        m
    })
}



struct Sortie {
    code: i32,
    out: String,
    err: String,
}

fn lancer(binaire: &str, donnees: Option<&Path>, args: &[&str], env: &[(&str, &str)]) -> Sortie {
    let m = monde();
    let mut c = Command::new(binaire);
    c.env_clear()
        .env(socle::garde_reseau::VARIABLE, "1") // le garde-fou reseau des essais : rien ne sort de la machine
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", m.join("home"))
        .env("PUPPETPAIE_CLE", m.join("cle"))
        .env("PUPPETPAIE_CODE", m.join("logiciel/donnees"))
        .env("PUPPETPAIE_AUJOURDHUI", AUJOURDHUI)
        .envs(env.iter().copied())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = donnees {
        c.env("PUPPETPAIE_DONNEES", d);
    }
    let p = c.output().unwrap();
    Sortie { code: p.status.code().unwrap_or(-1), out: String::from_utf8_lossy(&p.stdout).into_owned(), err: String::from_utf8_lossy(&p.stderr).into_owned() }
}


fn ecrire(racine: &Path, chemin: &str, contenu: &str) {
    let p = racine.join(chemin);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

// ------------------------------------------------------------------ les dates des options


// ------------------------------------------------------------------ echeancier


// ------------------------------------------------------------------ droits


// ------------------------------------------------------------------ incidents


// ------------------------------------------------------------------ version


/// Le depot des mises a jour d'une installation qui n'en pose pas : celui du moteur (`maj::DEPOT_PAR_DEFAUT`, lot P1),
/// la derniere Release du depot public -- un faux mandataire local voit la demande et refuse, rien ne sort. Un depot pose
/// dans le reglage de l'installation le remplace : c'est lui qu'on interroge. `moteur maj` n'a besoin d'aucune compagnie.
#[test]
fn maj_cherche_au_depot_du_moteur_tant_qu_aucun_n_est_pose() {
    use crate::outils::{http_json, mandataire, serveur};
    let inst = crate::outils::installation(&dossier_de_test("commandes", "maj-depot"), "2026.10.09");
    let code = crate::outils::code_de(&inst);
    let (proxy, demandes) = mandataire();
    let r = lancer(env!("CARGO_BIN_EXE_moteur"), None, &["maj", "--sans-telecharger"], &[(socle::garde_reseau::VARIABLE, &proxy), ("PUPPETPAIE_CODE", &code)]);
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    assert!(r.out.contains("depot injoignable") && r.out.contains("https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download/derniere.json"), "{}", r.out);
    assert_eq!(*demandes.lock().unwrap(), ["CONNECT github.com:443 HTTP/1.1"]);
    // Un depot pose : le moteur ne parle qu'a lui.
    let derniere = json!({"version": "2000.01.01", "archives": {"tout-systeme": {"archive": "a.zip", "sha256": "x", "signature": "s", "systemes": ["windows", "linux", "mac"]}}});
    let (base, recu) = serveur(move |_| Some(http_json(200, &derniere)));
    ecrire(&inst, "maj/reglage.json", &json!({"depot": base}).to_string());
    let r = lancer(env!("CARGO_BIN_EXE_moteur"), None, &["maj", "--sans-telecharger"], &[("PUPPETPAIE_CODE", &code)]);
    assert!(r.out.contains("a jour"), "{}{}", r.out, r.err);
    assert_eq!(recu.lock().unwrap().iter().map(|q| q.chemin.clone()).collect::<Vec<_>>(), ["/derniere.json"]);
}

