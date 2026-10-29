//! `moteur importer [--dossier D] [--ecrire]` : reprendre l'historique depuis la boite aux lettres `import/`, pour le diagnostic et le poste.
//!
//! Sans `--ecrire`, l'APERCU (route `importation-apercu`, `{"dossier": D}`) : rien n'est ecrit. Avec `--ecrire`, la
//! reprise (route `importer`, meme charge) : la reponse de l'apercu, completee de ce qui a ete ecrit. Sans `--dossier`,
//! `import/` a la racine de la compagnie. La sortie est le JSON du moteur, dans l'enveloppe de `--json` (`commande`,
//! `ok`, `resultat`) : la meme, que `--json` soit donne ou non -- un apercu de cette taille ne se lit pas en lignes.
//!
//! Un refus du moteur (dossier absent, table des gabarits illisible...) sort dans la meme enveloppe, `ok: false`, code
//! 2 ; une panne passe par `Cadre::plantage`. Ecrire l'historique n'est pas un acte reel (rien ne part) : aucun mot de
//! passe n'est demande. L'incident des fichiers non repris ne part pas d'ici : c'est l'ecran qui le montre et
//! l'envoie, au clic de la personne.

use serde_json::json;

use super::actes::est_operation;
use super::analyse::{Args, Opt, Spec};
use super::rendu::{enveloppe, erreur, rendre};
use super::Cadre;

pub static SPEC: Spec = Spec {
    commande: "importer",
    positionnels: &[],
    options: &[Opt::valeur("dossier"), Opt::drapeau("ecrire"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let charge = json!({"dossier": args.texte("dossier").unwrap_or("")});
    let route = if args.drapeau("ecrire") { "importer" } else { "importation-apercu" };
    match cadre.route(route, charge) {
        Ok(r) => rendre(&enveloppe("importer", r)),
        Err(e) if est_operation(e.classe) || matches!(e.classe, Some("FileNotFoundError" | "TypeError")) => {
            rendre(&erreur("importer", e.classe.unwrap_or("ErreurOperation"), &e.message, &[]))
        }
        Err(e) => cadre.plantage(e),
    }
}
