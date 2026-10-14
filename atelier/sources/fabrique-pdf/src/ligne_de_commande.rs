//! `fabrique-pdf <sortie.pdf>` : lit la description JSON sur l'entrée standard, écrit le PDF.
//! `fabrique-pdf --assembler <sortie.pdf>` : fusionne des pages de PDF existants (JSON sur l'entrée).
//! `fabrique-pdf --pages a.pdf [b.pdf …]` : le nombre de pages de chaque PDF, `[n1, n2, …]`.
//! `fabrique-pdf --texte a.pdf` : le texte de chaque page, un tableau JSON de chaînes.
//! `fabrique-pdf --inspecter a.pdf` : page.rect, mots et leurs boîtes, images, traits, polices,
//! métadonnées, chiffrement : une ligne de JSON (ce que les tests demandaient à PyMuPDF).
//! `fabrique-pdf --marquer entree.pdf sortie.pdf "texte"` : un texte en pied de chaque page.
//! `fabrique-pdf --verrouiller entree.pdf sortie.pdf` : chiffre le PDF (AES-256, sans mot de passe).
//! `fabrique-pdf --champs a.pdf` : les champs du formulaire (nom → valeur, page, rectangle), en JSON.
//! `fabrique-pdf --remplir entree.pdf sortie.pdf` : remplit des champs texte et pose des images
//! (JSON sur l'entrée standard).
//! `fabrique-pdf --version` : « fabrique-pdf 3 », le numéro du format.
//!
//! Code de sortie 0 : le PDF est écrit (ou le JSON est sur la sortie standard). Sinon rien n'est
//! écrit, et une ligne de français dit pourquoi sur la sortie d'erreur (code 1 : refus ; code 2 :
//! mauvais arguments).

use std::io::Read;
use std::process::ExitCode;

use crate::{fabriquer, FORMAT};

fn echec(message: &str, code: u8) -> ExitCode {
    eprintln!("{}", message.replace('\n', " "));
    ExitCode::from(code)
}

/// L'entrée standard en UTF-8, sans marque d'ordre des octets.
fn lire_entree() -> Result<String, String> {
    let mut entree = Vec::new();
    std::io::stdin()
        .read_to_end(&mut entree)
        .map_err(|e| format!("lecture de l'entrée standard impossible : {}", e))?;
    let texte = String::from_utf8(entree)
        .map_err(|_| "description illisible : l'entrée n'est pas de l'UTF-8".to_string())?;
    Ok(texte.strip_prefix('\u{feff}').unwrap_or(&texte).to_string())
}

/// Écrit `octets` dans `sortie`. Un échec efface le fichier partiel — sauf si `sortie` est aussi
/// le fichier d'entrée (`--marquer a.pdf a.pdf …`) : celui-là n'est jamais effacé.
fn ecrire(sortie: &str, octets: &[u8], entree: Option<&str>) -> Result<(), ExitCode> {
    if let Err(e) = std::fs::write(sortie, octets) {
        if entree != Some(sortie) {
            let _ = std::fs::remove_file(sortie);
        }
        return Err(echec(&format!("écriture impossible de {} : {}", sortie, e), 1));
    }
    Ok(())
}

fn lire_fichier(chemin: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(chemin).map_err(|e| echec(&format!("{} illisible ({})", chemin, e), 1))
}

/// La ligne de commande de la fabrique, sans le nom du programme : `fabrique-pdf` et `paie pdf`
/// la servent toutes deux.
pub fn executer(args: &[String]) -> ExitCode {
    match args {
        [a] if a == "--version" => {
            println!("fabrique-pdf {}", FORMAT);
            ExitCode::SUCCESS
        }
        [a] if a == "--aide" || a == "--help" || a == "-h" => {
            println!(
                "usage : fabrique-pdf <sortie.pdf>   (la description JSON arrive sur l'entrée standard)\n        \
                 fabrique-pdf --assembler <sortie.pdf>   (fusion de PDF existants, JSON sur l'entrée standard)\n        \
                 fabrique-pdf --pages <a.pdf> [<b.pdf> ...]   (nombre de pages de chaque PDF, tableau JSON)\n        \
                 fabrique-pdf --texte <a.pdf>   (texte de chaque page, tableau JSON)\n        \
                 fabrique-pdf --inspecter <a.pdf>   (taille, mots et boîtes, images, traits, polices, métadonnées : JSON)\n        \
                 fabrique-pdf --marquer <entree.pdf> <sortie.pdf> <texte>   (texte en pied de chaque page)\n        \
                 fabrique-pdf --verrouiller <entree.pdf> <sortie.pdf>   (chiffrement AES-256, sans mot de passe)\n        \
                 fabrique-pdf --champs <a.pdf>   (les champs du formulaire, objet JSON)\n        \
                 fabrique-pdf --remplir <entree.pdf> <sortie.pdf>   (champs et images, JSON sur l'entrée standard)\n        \
                 fabrique-pdf --version"
            );
            ExitCode::SUCCESS
        }
        // L'assemblage de PDF existants (FORMAT.md, « L'assemblage »).
        [a, sortie] if a == "--assembler" => {
            let texte = match lire_entree() {
                Ok(t) => t,
                Err(m) => return echec(&m, 1),
            };
            let octets = match crate::assembler(&texte) {
                Ok(o) => o,
                Err(r) => return echec(&r.0, 1),
            };
            match ecrire(sortie, &octets, None) {
                Ok(()) => ExitCode::SUCCESS,
                Err(c) => c,
            }
        }
        // Le nombre de pages de chaque PDF : un tableau JSON (FORMAT.md, « Lire un PDF »).
        [a, fichiers @ ..] if a == "--pages" && !fichiers.is_empty() => match crate::lecture::pages(fichiers) {
            Ok(n) => {
                println!("{}", serde_json::to_string(&n).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(r) => echec(&r.0, 1),
        },
        // Le texte de chaque page d'un PDF : un tableau JSON de chaînes.
        [a, fichier] if a == "--texte" => match crate::lecture::texte(fichier) {
            Ok((textes, avertissements)) => {
                for m in avertissements {
                    eprintln!("{} : {}", fichier, m);
                }
                println!("{}", serde_json::to_string(&textes).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(r) => echec(&r.0, 1),
        },
        // L'inspection d'un PDF : une ligne de JSON (FORMAT.md, « Inspecter un PDF »).
        [a, fichier] if a == "--inspecter" => match crate::inspection::inspecter(fichier) {
            Ok((json, avertissements)) => {
                for m in avertissements {
                    eprintln!("{} : {}", fichier, m);
                }
                println!("{}", json);
                ExitCode::SUCCESS
            }
            Err(r) => echec(&r.0, 1),
        },
        // Un texte en pied de chaque page d'un PDF existant (FORMAT.md, « Marquer un PDF »).
        [a, entree, sortie, texte] if a == "--marquer" => {
            let octets = match lire_fichier(entree) {
                Ok(o) => o,
                Err(c) => return c,
            };
            let marque = match crate::marquage::marquer(&octets, texte) {
                Ok(o) => o,
                Err(r) => return echec(&format!("{} : {}", entree, r.0), 1),
            };
            match ecrire(sortie, &marque, Some(entree)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(c) => c,
            }
        }
        // Verrouille un PDF : AES-256, ouvert sans mot de passe, impression et copie seulement.
        [a, entree, sortie] if a == "--verrouiller" => {
            let octets = match lire_fichier(entree) {
                Ok(o) => o,
                Err(c) => return c,
            };
            let chiffre = match crate::chiffrement::verrouiller(&octets) {
                Ok(o) => o,
                Err(r) => return echec(&format!("{} : {}", entree, r.0), 1),
            };
            match ecrire(sortie, &chiffre, Some(entree)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(c) => c,
            }
        }
        // Les champs d'un formulaire AcroForm : un objet JSON (FORMAT.md, « Remplir un formulaire »).
        [a, fichier] if a == "--champs" => {
            let octets = match lire_fichier(fichier) {
                Ok(o) => o,
                Err(c) => return c,
            };
            match crate::formulaire::champs_json(&octets) {
                Ok(v) => {
                    println!("{}", v);
                    ExitCode::SUCCESS
                }
                Err(r) => echec(&format!("{} : {}", fichier, r.0), 1),
            }
        }
        // Remplit des champs texte et pose des images ; la description JSON arrive sur l'entrée.
        [a, entree, sortie] if a == "--remplir" => {
            let description = match lire_entree() {
                Ok(t) => t,
                Err(m) => return echec(&m, 1),
            };
            let octets = match lire_fichier(entree) {
                Ok(o) => o,
                Err(c) => return c,
            };
            let rempli = match crate::formulaire::remplir(&octets, &description) {
                Ok(o) => o,
                Err(r) => return echec(&format!("{} : {}", entree, r.0), 1),
            };
            match ecrire(sortie, &rempli, Some(entree)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(c) => c,
            }
        }
        [sortie] if !sortie.starts_with("--") => {
            let mut entree = Vec::new();
            if let Err(e) = std::io::stdin().read_to_end(&mut entree) {
                return echec(&format!("lecture de l'entrée standard impossible : {}", e), 1);
            }
            let texte = match String::from_utf8(entree) {
                Ok(t) => t,
                Err(_) => return echec("description illisible : l'entrée n'est pas de l'UTF-8", 1),
            };
            let texte = texte.strip_prefix('\u{feff}').unwrap_or(&texte);
            let octets = match fabriquer(texte) {
                Ok(o) => o,
                Err(r) => return echec(&r.0, 1),
            };
            match ecrire(sortie, &octets, None) {
                Ok(()) => ExitCode::SUCCESS,
                Err(c) => c,
            }
        }
        _ => echec(
            "usage : fabrique-pdf <sortie.pdf> (description JSON sur l'entrée standard), --assembler, --pages, --texte, --inspecter, --marquer, --verrouiller, --champs, --remplir ou --version",
            2,
        ),
    }
}
