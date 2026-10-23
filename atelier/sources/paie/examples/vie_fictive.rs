//! LA VIE D'UNE COMPAGNIE FICTIVE, REJOUEE SANS NAVIGATEUR (#236, phase 2).
//!
//! L'atelier seulement, jamais livre. Un SCENARIO (`paie/src/scenario.rs`) est ce que l'enregistreur du banc d'essai a
//! consigne pendant qu'on jouait dans l'interface (`atelier/outils/banc/`) : le « + » de l'accueil, chaque route que la
//! page a appelee avec sa charge et son instant, le geste qui rend les envois simules, les jours ou le calendrier a
//! avance. Ce programme le rejoue dans une installation neuve (`scenario::rejouer`), dans son processus : le decor du
//! « + », chaque geste a son instant fige, et la compagnie qui en sort est celle du banc.
//!
//!     cargo run --profile essais --example vie_fictive -- <dossier du scenario> <installation neuve> [--lectures]
//!
//! `--lectures` rejoue aussi les routes qui n'avaient rien ecrit. Rend 1 si un geste ne s'est pas passe comme a
//! l'enregistrement (une route reussie qui refuse, ou l'inverse), et le dit.

use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let lectures = args.iter().any(|a| a == "--lectures");
    args.retain(|a| a != "--lectures");
    if args.len() != 2 {
        eprintln!("vie_fictive <dossier du scenario> <installation neuve> [--lectures]");
        std::process::exit(2);
    }
    // Rien ne sort de la machine (`garde_reseau`) : `cargo run` la pose deja ; lance a la main, l'exemple la pose lui-meme,
    // avant tout fil.
    if std::env::var_os(paie::garde_reseau::VARIABLE).is_none() {
        std::env::set_var(paie::garde_reseau::VARIABLE, "1");
    }
    let (scenario, inst) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    if inst.exists() && std::fs::read_dir(&inst).map(|mut d| d.next().is_some()).unwrap_or(false) {
        eprintln!("{} n'est pas vide : le rejeu part d'une installation neuve.", inst.display());
        std::process::exit(2);
    }
    let debut = std::time::Instant::now();
    let options = paie::scenario::OptionsRejeu { lectures, ..Default::default() };
    match paie::scenario::rejouer(&scenario, &inst, &options) {
        Ok(r) => {
            for p in &r.horloge {
                println!("      {}", p);
            }
            for (nom, racine) in &r.compagnies {
                println!("{} -> {}", nom, racine.display());
            }
            println!(
                "{} route(s) rejouee(s), {} lecture(s) ecartee(s), {} geste(s) sur une autre compagnie, en {:.1} s.",
                r.routes_rejouees,
                r.lectures_ecartees,
                r.autres_compagnies,
                debut.elapsed().as_secs_f64()
            );
            for e in &r.ecarts {
                println!("ECART  {}", e);
            }
            if !r.ecarts.is_empty() {
                std::process::exit(1);
            }
        }
        Err(m) => {
            eprintln!("{}", m);
            std::process::exit(1);
        }
    }
}
