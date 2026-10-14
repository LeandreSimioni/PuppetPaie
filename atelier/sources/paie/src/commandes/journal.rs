//! `paie journal [--quoi ACTE] [--mois AAAA-MM]` : le journal des depots, en lecture (`moteur/commandes_retours.py`,
//! `cmd_journal`). `--reprendre` / `--ecrire` (remettre au journal les depots DSN attestes par `retours/`, lot J2b) : voir `reprendre`.
//! Le Python ignore `--json` ici ; le Rust aussi.

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, quand_court, texte};
use super::Cadre;
use crate::retours::Echec;

pub static SPEC: Spec = Spec {
    commande: "journal",
    positionnels: &[],
    options: &[
        Opt::choix("quoi", &["dsn", "dpae", "documents", "contrat-signature"], None),
        Opt::valeur("mois"),
        Opt::drapeau("reprendre"),
        Opt::drapeau("ecrire"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if args.drapeau("reprendre") {
        return match reprendre(cadre, args) {
            Ok(c) => c,
            Err(e) if super::actes::est_operation(e.classe) => super::actes::reponse_metier("journal", false, &e, &[]),
            Err(e) => cadre.plantage(e),
        };
    }
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let (quoi, mois) = (args.texte("quoi"), args.texte("mois"));
    let toutes = crate::etats::charger(&cadre.racine)?;
    let lignes = crate::etats::filtrer(&toutes, quoi, mois, false)?;
    if lignes.is_empty() {
        if quoi.is_some() || mois.is_some() {
            dire!("Le journal est vide pour ce filtre.");
        } else {
            dire!("Le journal est vide.\nRien n'est encore parti par ce logiciel -- ou le journal n'a pas ete repris :\n  paie journal --reprendre");
        }
        return Ok(0);
    }
    for l in lignes {
        let vrai = |k: &str| l.get(k).is_some_and(crate::fichiers::vrai);
        let cible = if vrai("fichier") { champ(l, "fichier") } else if vrai("flux") { champ(l, "flux") } else { String::new() };
        dire!(
            "{}  {:<16} {:<4} {:<7} {:<8} {}",
            quand_court(l.get("quand")),
            champ(l, "quoi"),
            champ(l, "mode"),
            if vrai("accepte") { "accepte" } else { "REFUSE" },
            if vrai("mois") { champ(l, "mois") } else { "-".to_string() },
            cible
        );
        if vrai("origine") {
            // La ligne n'est pas un acte de ce logiciel : elle a ete reconstituee. Le dire a chaque affichage.
            dire!("        repris de : {}", champ(l, "origine"));
        }
        for r in l.get("retours").and_then(Value::as_array).cloned().unwrap_or_default() {
            let libelle = if r.get("libelle").is_some_and(crate::fichiers::vrai) { champ(&r, "libelle") } else { String::new() };
            let publication = if r.get("publication").is_some_and(crate::fichiers::vrai) { champ(&r, "publication") } else { String::new() };
            dire!("        retour {} {} ({})", texte(r.get("nature").unwrap_or(&Value::Null)), libelle, publication);
        }
    }
    Ok(0)
}

/// La liste d'une cle d'un objet JSON (vide si elle manque).
fn liste_de(v: &Value, cle: &str) -> Vec<Value> {
    v.get(cle).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// `journal --reprendre [--ecrire]` : remettre au journal les depots que `retours/` atteste -- les DPAE d'abord (certificats de nature 41 de
/// `retours/dpae/`, `reprise_journal::reprise_dpae`), puis les DSN (route `journal-reprise`) --, `qui` = `cli`. Rien ne part, aucun reseau,
/// pas de mot de passe. Sans `--ecrire`, la commande DIT ce qu'elle ferait. Les DEUX CANAUX se reprennent ensemble : la DSN et la DPAE ne
/// repondent pas au meme service, et deux commandes pour une seule question (« qu'est-ce qui est deja parti ? ») seraient une de trop.
fn reprendre(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let ecrire = args.drapeau("ecrire");
    let maintenant = crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif());
    let d = crate::reprise_journal::reprise_dpae(&cadre.ctx, ecrire, "cli", &maintenant)?;
    let (d_trouves, d_ajoutes, d_inconnus) = (liste_de(&d, "trouves"), liste_de(&d, "ajoutes"), liste_de(&d, "inconnus"));
    if !d_trouves.is_empty() {
        dire!("DPAE attestees par les certificats de l'Urssaf : {}", d_trouves.len());
        dire!();
        for v in &d_trouves {
            let quand = if v.get("quand").is_some_and(crate::fichiers::vrai) { quand_court(v.get("quand")) } else { "?".to_string() };
            dire!(
                "  {}  {:<4}  {:<8}  {}{}",
                quand,
                if champ(v, "mode") == "reel" { "REEL" } else { "test" },
                if v.get("mois").is_some_and(crate::fichiers::vrai) { champ(v, "mois") } else { "-".to_string() },
                champ(v, "flux"),
                if v.get("deja").is_some_and(crate::fichiers::vrai) { "   [deja au journal]" } else { "" }
            );
            for e in liste_de(v, "embauches") {
                dire!("        {} a partir du {}", champ(&e, "salarie"), champ(&e, "debut"));
            }
        }
        dire!();
    }
    for x in &d_inconnus {
        dire!("Non rattache : {}", texte(x));
    }
    let r = cadre.route("journal-reprise", json!({"ecrire": ecrire, "qui": "cli"}))?;
    let liste = |k: &str| -> Vec<Value> { r.get(k).and_then(Value::as_array).cloned().unwrap_or_default() };
    let (trouves, ajoutes, sans_mois, illisibles) = (liste("trouves"), liste("ajoutes"), liste("sans_mois"), liste("illisibles"));
    if !illisibles.is_empty() {
        dire!("Pieces illisibles, sautees : {}\n", illisibles.iter().map(texte).collect::<Vec<_>>().join(", "));
    }
    if trouves.is_empty() && d_trouves.is_empty() {
        dire!("Aucune piece de depot dans retours/. Rien a reprendre.\nLes accuses (nature 10) et les bilans (nature 11) s'archivent avec `paie retours`.\n");
        return Ok(0);
    }
    if !trouves.is_empty() {
        dire!("Depots DSN attestes par les retours : {}\n", trouves.len());
    }
    for v in &trouves {
        let vrai_de = |k: &str| v.get(k).is_some_and(crate::fichiers::vrai);
        let quand = if vrai_de("quand") { quand_court(v.get("quand")) } else { "?".to_string() };
        dire!(
            "  {}  {:<4}  {:<7}  {:<8}  {:<11}  {}{}",
            quand,
            if champ(v, "mode") == "reel" { "REEL" } else { "test" },
            if vrai_de("accepte") { "accepte" } else { "REJETE" },
            if vrai_de("mois") { champ(v, "mois") } else { "-".to_string() },
            if vrai_de("logiciel") { champ(v, "logiciel") } else { "?".to_string() },
            champ(v, "flux"),
            if vrai_de("deja") { "   [deja au journal]" } else { "" }
        );
    }
    dire!();
    if !sans_mois.is_empty() {
        dire!("{} piece(s) sans date de reception : non reprises.\n", sans_mois.len());
    }
    if ajoutes.is_empty() && d_ajoutes.is_empty() {
        dire!("Rien de nouveau : le journal connait deja tout cela.");
        return Ok(0);
    }
    let total = ajoutes.len() + d_ajoutes.len();
    if !ecrire {
        dire!("{} ligne(s) SERAIENT ajoutees (dont {} DPAE). Rien n'a ete ecrit.\nRelancer avec --ecrire pour les poser.", total, d_ajoutes.len());
        return Ok(0);
    }
    dire!("{} ligne(s) ajoutees au journal (dont {} DPAE).", total, d_ajoutes.len());
    let rattaches = r.get("rattaches").and_then(Value::as_i64).unwrap_or(0);
    if rattaches != 0 {
        dire!("{} retour(s) rattaches a ces depots.", rattaches);
    }
    Ok(0)
}
