//! `paie` : la ligne de commande du moteur Rust. Se parse a la main.

fn usage() -> ! {
    eprintln!(
        "usage : paie [--dev] COMMANDE [options]   (la ligne de commande du produit : paie aide)\n\
         \x20       paie route [--racine R] [--date D]   (route et charge en JSON sur l entree standard : les routes de l interface)\n\
         \x20       paie pdf ...   (la ligne de commande de la fabrique : paie pdf --aide)\n\
         \x20       paie interne NOM ...   (les commandes que le Python appelle avec du JSON : paie interne --aide)\n\
         \x20       paie oracle ... | paie fabrique DESCRIPTION.json SORTIE.pdf   (outils d'atelier)\n\
         \x20       paie extraire naf|usages|formats|urssaf SOURCES... SORTIE.json [--remplacer]   (releves de l'atelier : paie extraire --aide)"
    );
    std::process::exit(2)
}

/// `paie interne --aide` : les commandes que le Python appelle (`moteur/paie_rust.py`), avec leur demande JSON sur
/// l'entree standard. Avant le lot J2a elles s'appelaient `paie NOM` ; les noms reviennent aux commandes du produit.
fn usage_interne() -> ! {
    eprintln!(
        "usage : paie interne bulletin [--racine R] [--date D]   (saisies, jeux, ent en JSON sur l'entree standard)\n\
         \x20       paie interne bareme-applique   (bareme, salarie, nature, fin_periode en JSON sur l'entree standard)\n\
         \x20       paie interne bordereau   (bordereaux Urssaf : jour et lignes 23 en JSON sur l'entree standard)\n\
         \x20       paie interne droits|provision-conges|preavis|lignes|dsn|montant-urssaf|fctu|amorcage [--racine R] [--date D]   (demande JSON sur l'entree standard)\n\
         \x20       paie interne signalement [--racine R] [--date D]   (arret, nature, options, sortie en JSON sur l'entree standard)\n\
         \x20       paie interne signalement-verifier   (arret, nature, constitution en JSON sur l'entree standard)\n\
         \x20       paie interne dpae   (embauches, horodatage, ent, operation en JSON sur l'entree standard)\n\
         \x20       paie interne dpae-lire   (contenu d'un flux DPAE et operation en JSON sur l'entree standard)\n\
         \x20       paie interne retours   (lecture d'un retour DSN : operation et contenu en JSON sur l'entree standard)\n\
         \x20       paie interne transport   (net-entreprises et Urssaf : operation et ses donnees en JSON sur l'entree standard)\n\
         \x20       paie interne ecrire-json|verrou   (ecriture d'un JSON, verrou des compteurs : demande JSON sur l'entree standard)\n\
         \x20       paie interne aer|aer-verifier|aer-quantites|aer-cachet   (attestation AER Cinema Spectacle : demande JSON sur l'entree standard)"
    );
    std::process::exit(2)
}

/// `paie interne NOM ...` : le service du Python (`paie_rust._appeler`). Les noms d'avant le lot J2a, sous `interne`.
fn interne(args: &[String]) -> std::process::ExitCode {
    match args.first().map(String::as_str) {
        Some(q @ ("bulletin" | "bareme-applique" | "droits" | "provision-conges" | "preavis" | "lignes" | "dsn" | "montant-urssaf" | "bordereau" | "fctu" | "amorcage" | "signalement" | "signalement-verifier" | "dpae" | "dpae-lire" | "retours" | "ecrire-json" | "verrou")) => match paie::service::commande(q, &args[1..]) {
            Some(code) => std::process::exit(code),
            None => usage_interne(),
        },
        Some(q @ ("aer" | "aer-verifier" | "aer-quantites" | "aer-cachet")) => match paie::service::commande(q, &args[1..]) {
            Some(code) => std::process::exit(code),
            None => usage_interne(),
        },
        Some("transport") => std::process::ExitCode::from(paie::transport::commande_ligne() as u8),
        _ => usage_interne(),
    }
}

fn main() -> std::process::ExitCode {
    // Le binaire de l'atelier (`--features atelier`) : une redirection vers autre chose qu'un serveur local arrete tout.
    #[cfg(feature = "atelier")]
    if let Err(m) = paie::atelier::verifier_environnement() {
        eprintln!("{}", m);
        return std::process::ExitCode::from(2);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Les commandes de la fabrique, servies par sa bibliotheque.
        Some("pdf") => return fabrique_pdf::ligne_de_commande::executer(&args[1..]),
        // Les routes de l'interface, une par processus (la demande JSON sur l'entree standard).
        Some("route") => match paie::service::commande("route", &args[1..]) {
            Some(code) => std::process::exit(code),
            None => usage(),
        },
        // La fenetre, dans le navigateur du poste (lot J3) : `paie interface`.
        // La mise a jour (lot J4) : verifier, appliquer, revenir ; publier et la cle, chez l'editeur.
        Some("maj") => return std::process::ExitCode::from(paie::maj::commande(&args[1..]).clamp(0, 255) as u8),
        // La compagnie d'essai (lot E1) : un vrai depot de TEST sans `prive/`, l'identite lue dans `PUPPETPAIE_COMPAGNIE`.
        Some("essai") => return std::process::ExitCode::from(paie::essai::commande(&args[1..]).clamp(0, 255) as u8),
        Some("interface") => return std::process::ExitCode::from(paie::interface::commande(&args[1..]).clamp(0, 255) as u8),
        // Les commandes que le Python appelle avec du JSON : `paie interne NOM`.
        Some("interne") => return interne(&args[1..]),
        Some("extraire") => return std::process::ExitCode::from(paie::extraire::commande(&args[1..]).clamp(0, 255) as u8),
        Some("oracle") => match paie::oracle::commande(&args[1..]) {
            Some(code) => std::process::exit(code),
            None => usage(),
        },
        // Outil d'atelier : une description JSON -> un PDF, par la bibliotheque.
        Some("fabrique") if args.len() == 3 => {
            let r = std::fs::read_to_string(&args[1])
                .map_err(|e| format!("{} illisible ({})", args[1], e))
                .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
                .and_then(|d| paie::fabrique::ecrire(&d, std::path::Path::new(&args[2])));
            if let Err(m) = r {
                eprintln!("{}", m);
                std::process::exit(1);
            }
        }
        // Le reste est la ligne de commande du produit (`paie [--dev] COMMANDE ...`).
        Some(_) => return std::process::ExitCode::from(paie::commandes::executer(&args).clamp(0, 255) as u8),
        None => usage(),
    }
    std::process::ExitCode::SUCCESS
}
