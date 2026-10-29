//! LA VIE D'UNE COMPAGNIE FICTIVE, REJOUEE SANS NAVIGATEUR.
//!
//! L'atelier seulement, jamais livre. Un SCENARIO (`moteur/src/atelier/scenario.rs`) est ce que l'enregistreur du banc d'essai a
//! consigne pendant qu'on jouait dans l'interface (`atelier/outils/banc/`) : le « + » de l'accueil, chaque route que la
//! page a appelee avec sa charge et son instant, le geste qui rend les envois simules, les jours ou le calendrier a
//! avance. Ce programme sert la ligne de commande des scenarios (`scenario::commande`) sans le binaire de l'atelier :
//!
//!     cargo run --profile essais --example vie_fictive -- rejouer <scenario> <installation neuve> [--lectures]
//!     cargo run --profile essais --example vie_fictive -- verifier <scenario> <installation du banc> <dossier de travail>
//!     cargo run --profile essais --example vie_fictive -- elaguer <scenario> <scenario sans lectures>
//!
//! `moteur-atelier scenario ...` est la meme commande.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(moteur::atelier::scenario::commande(&args));
}
