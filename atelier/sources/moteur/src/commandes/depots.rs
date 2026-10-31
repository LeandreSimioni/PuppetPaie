//! `moteur depots [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--environnement production|editeur]` : la liste des depots que
//! net-entreprises a vus, telle quelle. CACHEE : elle marche, elle
//! n'apparait pas dans l'aide. C'est une commande d'enquete pour l'editeur, sur le poste d'une compagnie -- retrouver
//! quelque chose sur un depot passe, par exemple la date de constitution d'un fichier qu'on n'a plus. Volontairement
//! BRUTE : tous les champs que le service renvoie, sans en choisir.
//!
//! ATTENTION : elle va au RESEAU (net-entreprises). Lecture seule.

use super::analyse::{Args, Opt, Spec};
use socle::date::Jour;
use super::rendu::texte;
use super::Cadre;
use envois::releve_retours::{Client, Panne};
use socle::echec::Echec;

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
        Err(e) => refus_reseau_ou_plantage(cadre, e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    // Le moteur ouvre le coffre comme pour `retours` s'il est ferme.
    cadre.ouvrir_le_coffre();
    let Some(mdp) = crate::compagnie::releve::mot_de_passe_de_consultation(&cadre.ctx, "net-entreprises")? else {
        dire!("Mot de passe absent. Le poser une fois : `moteur coffre poser net-entreprises`.");
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

/// Le reseau injoignable ou coupe, ou le coffre qui ne s'ouvre pas : une phrase et un code, pas un plantage.
fn refus_reseau_ou_plantage(cadre: &Cadre, e: Echec) -> i32 {
    if matches!(e.classe, Some("URLError" | "TimeoutError" | "RemoteDisconnected" | "IncompleteRead" | "HTTPException")) {
        dire!("Consultation interrompue : le reseau est injoignable ou a ete coupe ({}).\nRien n'est perdu : relancer la commande quand la connexion est revenue.", e.message);
        return 1;
    }
    // Le coffre ferme et la seance refusee : « rien ici ne l'ouvre » est une phrase pour l'utilisateur, pas un defaut du logiciel.
    // Code 2, comme « le mot de passe n'est nulle part ».
    if e.classe == Some("ErreurCoffre") {
        dire!("{}", e.message);
        return 2;
    }
    cadre.plantage(e)
}

fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

const JOUR: i64 = 86_400;

fn secondes(j: &Jour) -> i64 {
    (j.ordinal() - 719_163) * JOUR
}

/// La plage de recherche, en heures locales naives. Par defaut, les 30 derniers jours.
fn fenetre(args: &Args) -> Result<(i64, i64), Echec> {
    let fin = match args.texte("jusqu-a") {
        Some(j) => secondes(&Jour::lire(j).map_err(|m| Echec { classe: Some("ValueError"), message: m })?) + 23 * 3600 + 59 * 60 + 59,
        None => socle::paris::maintenant_naif(),
    };
    let debut = match args.texte("depuis") {
        Some(j) => secondes(&Jour::lire(j).map_err(|m| Echec { classe: Some("ValueError"), message: m })?),
        None => fin - 30 * JOUR,
    };
    Ok((debut, fin))
}

/// Le jour d'une heure locale naive, en `JJ/MM/AAAA`.
fn jour_de(t: i64) -> String {
    Jour::depuis_jours_unix(t.div_euclid(JOUR)).map(|j| j.jj_mm_aaaa()).unwrap_or_default()
}

