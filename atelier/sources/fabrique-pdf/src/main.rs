//! `fabrique-pdf` : la ligne de commande de la fabrique (`src/ligne_de_commande.rs`).

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    fabrique_pdf::ligne_de_commande::executer(&args)
}
