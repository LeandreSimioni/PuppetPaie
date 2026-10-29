//! `moteur` : la ligne de commande du moteur Rust. Se parse a la main.

fn usage() -> ! {
    eprintln!(
        "usage : moteur [--dev] COMMANDE [options]   (la ligne de commande du produit : moteur aide)\n\
         \x20       moteur route [--racine R] [--date D]   (route et charge en JSON sur l entree standard : les routes de l interface)\n\
         \x20       moteur pdf ...   (la ligne de commande de la fabrique : moteur pdf --aide)\n\
         \x20       moteur fabrique DESCRIPTION.json SORTIE.pdf   (outil d'atelier)\n\
         \x20       moteur guet [--seuil N] [--commits] [--transcript F]   (outil d'atelier : la veille du contexte d'un sous-agent)\n\
         \x20       moteur verifier-avant-commit [--indexes]   (le garde avant commit : rien de reel dans ce que git emporterait)\n\
         \x20       moteur generer-tables SOURCES ESPACE   (les tables Unicode du moteur, depuis atelier/reference/sources, dans atelier/sources)\n\
         \x20       moteur extraire naf|usages|formats|urssaf SOURCES... SORTIE.json [--remplacer]   (releves de l'atelier : moteur extraire --aide)\n\
         \x20       moteur releve-codes ... | moteur releve-conventions ...   (les textes LEGI et KALI de la DILA, outils d'atelier : --aide)"
    );
    std::process::exit(2)
}

fn main() -> std::process::ExitCode {
    // Le binaire de l'atelier (`--features atelier`) : une redirection vers autre chose qu'un serveur local arrete tout.
    #[cfg(feature = "atelier")]
    if let Err(m) = moteur::atelier::verifier_environnement() {
        eprintln!("{}", m);
        return std::process::ExitCode::from(2);
    }
    moteur::incidents::brancher_l_engagement();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Les commandes de la fabrique, servies par sa bibliotheque.
        Some("pdf") => return fabrique_pdf::ligne_de_commande::executer(&args[1..]),
        // Les routes de l'interface, une par processus (la demande JSON sur l'entree standard).
        Some("route") => match moteur::service::commande("route", &args[1..]) {
            Some(code) => std::process::exit(code),
            None => usage(),
        },
        // La fenetre, dans le navigateur du poste (lot J3) : `moteur interface`.
        // La mise a jour (lot J4) : verifier, appliquer, revenir ; publier et la cle, chez l'editeur.
        Some("maj") => return std::process::ExitCode::from(moteur::maj::commande(&args[1..]).clamp(0, 255) as u8),
        // La compagnie d'essai (lot E1) : un vrai depot de TEST sans `prive/`, l'identite lue dans `PUPPETPAIE_COMPAGNIE`.
        Some("essai") => return std::process::ExitCode::from(moteur::essai::commande(&args[1..]).clamp(0, 255) as u8),
        // Les scenarios du banc d'essai (#236) : rejouer, verifier un banc, elaguer. L'atelier seulement.
        #[cfg(feature = "atelier")]
        Some("scenario") => return std::process::ExitCode::from(moteur::scenario::commande(&args[1..]).clamp(0, 255) as u8),
        // Les bulletins fictifs d'un logiciel precedent, pour une compagnie qui s'importe (#236). L'atelier seulement.
        #[cfg(feature = "atelier")]
        Some("fabriquer-bulletins") => {
            return std::process::ExitCode::from(moteur::atelier::fabriquer_bulletins::commande(&args[1..]).clamp(0, 255) as u8)
        }
        Some("interface") => return std::process::ExitCode::from(moteur::interface::commande(&args[1..]).clamp(0, 255) as u8),
        Some("generer-tables") => return std::process::ExitCode::from(moteur::generer_tables::commande(&args[1..]).clamp(0, 255) as u8),
        Some("extraire") => return std::process::ExitCode::from(moteur::extraire::commande(&args[1..]).clamp(0, 255) as u8),
        // Les codes (LEGI) et les conventions collectives (KALI) releves chez la DILA, dans `atelier/reference/sources`.
        Some(q @ ("releve-codes" | "releve-conventions")) => {
            let commande = if q == "releve-codes" { moteur::releve_codes::commande } else { moteur::releve_conventions::commande };
            return std::process::ExitCode::from(commande(&args[1..]).clamp(0, 255) as u8);
        }
        // Outil d'atelier : la veille du contexte d'un sous-agent (`guet.rs`).
        Some("guet") => return std::process::ExitCode::from(moteur::guet::commande(&args[1..]).clamp(0, 255) as u8),
        // Le garde avant commit (le crochet `atelier/.githooks/pre-commit`) : les donnees reelles dans ce que git emporterait.
        Some("verifier-avant-commit") => return std::process::ExitCode::from(moteur::verifier_avant_commit::commande(&args[1..]).clamp(0, 255) as u8),
        // Outil d'atelier : une description JSON -> un PDF, par la bibliotheque.
        Some("fabrique") if args.len() == 3 => {
            let r = std::fs::read_to_string(&args[1])
                .map_err(|e| format!("{} illisible ({})", args[1], e))
                .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
                .and_then(|d| documents::fabrique::ecrire(&d, std::path::Path::new(&args[2])));
            if let Err(m) = r {
                eprintln!("{}", m);
                std::process::exit(1);
            }
        }
        // Le reste est la ligne de commande du produit (`moteur [--dev] COMMANDE ...`).
        Some(_) => return std::process::ExitCode::from(moteur::commandes::executer(&args).clamp(0, 255) as u8),
        None => usage(),
    }
    std::process::ExitCode::SUCCESS
}
