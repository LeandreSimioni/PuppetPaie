//! `paie depots [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--environnement production|editeur]` : la liste des depots que
//! net-entreprises a vus, telle quelle (`moteur/commandes_retours.py`, `cmd_depots`). CACHEE : elle marche, elle
//! n'apparait pas dans l'aide. C'est une commande d'enquete pour l'editeur, sur le poste d'une compagnie -- retrouver
//! quelque chose sur un depot passe, par exemple la date de constitution d'un fichier qu'on n'a plus. Volontairement
//! BRUTE : tous les champs que le service renvoie, sans en choisir.
//!
//! ATTENTION : elle va au RESEAU (net-entreprises). Lecture seule.

use super::analyse::{Args, Opt, Spec};
use super::retours::{fenetre, jour_de, ko};
use super::rendu::texte;
use super::Cadre;
use crate::releve_retours::{Client, Panne};
use crate::retours::Echec;

pub static SPEC: Spec = Spec {
    commande: "depots",
    positionnels: &[],
    options: &[
        Opt::valeur("depuis"),
        Opt::valeur("jusqu-a"),
        Opt::choix("environnement", &["production", "editeur"], Some("production")),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => super::retours::refus_reseau_ou_plantage(cadre, e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    // Le Python ne passe pas par la seance ici (`coffre.lire` sans ouverture) ; le moteur, lui, ouvre le coffre comme pour
    // `retours` s'il est ferme.
    cadre.ouvrir_le_coffre();
    let Some(mdp) = crate::releve_retours::mot_de_passe_de_consultation(&cadre.ctx, "net-entreprises")? else {
        dire!("Mot de passe absent. Le poser une fois : `paie coffre poser net-entreprises`.");
        return Ok(2);
    };
    let environnement = args.texte("environnement").unwrap_or("production");
    let client = Client::dsn(&cadre.ctx, environnement, Some(mdp)).map_err(|p| ko(p.message))?;
    derouler(cadre, args, client)
}

/// Le compte rendu, avec un client donne (un test le pointe sur un faux serveur local).
pub fn derouler(cadre: &Cadre, args: &Args, mut client: Client) -> Result<i32, Echec> {
    let environnement = args.texte("environnement").unwrap_or("production");
    let (debut, fin) = fenetre(args)?;
    dire!("Depots {}, du {} au {}.\n", environnement, jour_de(debut), jour_de(fin));
    let _ = cadre;
    let mut depots = Vec::new();
    let resultat: Result<(), Panne> = (|| {
        client.authentifier()?;
        let mut t = debut;
        while t < fin {
            let t1 = (t + 86_400).min(fin);
            depots.extend(client.lister_depots(t, t1)?);
            t += 86_400;
        }
        Ok(())
    })();
    match resultat {
        Ok(()) => {}
        Err(p) if p.est_api() => {
            dire!("Echec : {}", p.message);
            return Ok(1);
        }
        Err(p) => return Err(p.en_echec()),
    }
    if depots.is_empty() {
        dire!("Aucun depot sur la periode. Elargir avec --depuis JJ/MM/AAAA.");
        return Ok(0);
    }
    for (i, dep) in depots.iter().enumerate() {
        dire!("  depot {}", i + 1);
        if let Some(o) = dep.as_object() {
            for (k, v) in o {
                dire!("     {:<24} {}", k, texte(v));
            }
        }
        dire!();
    }
    dire!("{} depot(s).", depots.len());
    Ok(0)
}
