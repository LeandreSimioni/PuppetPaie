//! `moteur-atelier depot-dsn (FICHIER | AAAA-MM) [--ordre N] [--environnement production|editeur]` : un depot de TEST d'une
//! DSN, a un ordre donne (la base d'essai a deja retenu des essais de ce SIRET pour ce mois que le journal ignore : un
//! autre poste, une compagnie d'essai) ou sur la plate-forme d'essai de l'editeur. Le depot est celui du moteur
//! (`depot_acte::deposer_dsn_dans`), qui refuse un ordre hors d'un essai de mensuelle et tout depot reel ailleurs qu'en
//! production ; le depot ordinaire, reel ou de test, est la route `deposer`. Un mois `AAAA-MM` : la DSN que la route
//! designerait (`depots::designee`).


use crate::commandes::actes::{refuser_si_pas_redirige, Organisme};
use crate::commandes::analyse::{Args, Opt, Pos, Spec};
use crate::commandes::Cadre;
use crate::compagnie::depot_acte::{self, Preuve};
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "depot-dsn",
    positionnels: &[Pos::facultatif("fichier", None)],
    options: &[Opt::valeur("ordre"), Opt::choix("environnement", &["production", "editeur"], Some("production"))],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    faire(cadre, args).unwrap_or_else(|e| super::refus(&e))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let Some(fichier) = args.texte("fichier").filter(|f| !f.is_empty()) else {
        dire!("depot-dsn FICHIER|AAAA-MM [--ordre N] [--environnement editeur] : un depot de TEST.");
        return Ok(2);
    };
    let ordre = match args.texte("ordre").filter(|t| !t.is_empty()) {
        None => None,
        Some(t) => match t.trim().parse::<i64>() {
            Ok(n) => Some(n),
            Err(_) => {
                dire!("--ordre N : un entier positif.");
                return Ok(2);
            }
        },
    };
    refuser_si_pas_redirige(cadre, Organisme::Dsn, "le depot de test")?;
    let designee = match socle::mois::Mois::lire(fichier) {
        Ok(m) => crate::routes::depots::designee(&cadre.ctx, &crate::routes::depots::Designation::du_mois(crate::routes::depots::Quoi::Dsn, m))?.0,
        Err(_) => fichier.to_string(),
    };
    // Un depot de test n'engage rien : il lui faut seulement le secret de l'organisme, donc le coffre ouvert.
    cadre.ouvrir_le_coffre();
    let environnement = args.texte("environnement").unwrap_or("production");
    let fait = depot_acte::deposer_dsn_dans(&cadre.ctx, &designee, false, Some(&Preuve::default()), "atelier", environnement, false, "", ordre)?;
    dire!("Depot de TEST {} ({}) : rien n'est conserve.", fait.mois.map(|m| m.to_string()).unwrap_or_default(), environnement);
    dire!("Reponse du point de depot :\n{}", if fait.reponse.is_empty() { "(vide)" } else { fait.reponse.as_str() });
    if !fait.flux.is_empty() {
        dire!("Flux : {} -- les retours s'y rattacheront (outil retours, ou la route ouverture).", fait.flux);
    }
    Ok(0)
}
