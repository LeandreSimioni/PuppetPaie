//! `moteur fin-contrat [liste|signaler|documents] AAAA-MM [SALARIE] [--remplace ID] [--ordre N] [--le AAAA-MM-JJ] [--test] [--json]` :
//! le signalement de fin de contrat de travail unique (nature 07) et les deux papiers dus au salarie qui part
//! (`moteur/commandes_fctu.py`).
//!
//! - `liste` dit quels contrats du mois se terminent et se signalent, et ce qui manque a la saisie (route `fins-de-contrat`) ;
//! - `signaler` ECRIT le fichier du signalement dans `sorties/AAAA-MM/` (route `fin-contrat-signaler`) -- rien ne part, le depot
//!   est `moteur depot-dsn` ;
//! - `documents` ecrit le certificat de travail et le recu pour solde de tout compte (route `fin-contrat-documents`) : des
//!   papiers, rien ne part.
//!
//! `--le` est la date de constitution du fichier (`signaler`) ou la date portee par les papiers (`documents`) ; defaut :
//! aujourd'hui. Les routes l'ignoraient : elles la lisent maintenant, dans la charge.
//!
//! DEFAUT DU PYTHON, NE SE REPRODUIT PAS : un mois mal ecrit, une date `--le` illisible ou un contrat sans date de fin donnaient une
//! trace d'appels. Ici : `! phrase`, code 2.

use serde_json::{json, Value};

use super::actes::{est_refus, jour_option, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::champ;
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "fin-contrat",
    positionnels: &[Pos::parmi("quoi", &["liste", "signaler", "documents"], Some("liste")), Pos::requis("mois"), Pos::facultatif("salarie", None)],
    options: &[Opt::valeur("remplace"), Opt::entier("ordre"), Opt::valeur("le"), Opt::drapeau("test"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// Un refus d'une phrase, au format de la commande (`! phrase` ou `sortie_json.erreur`).
fn refuse(json: bool, e: &Echec) -> i32 {
    reponse_metier("fin-contrat", json, e, &[])
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let mois = args.texte("mois").unwrap_or("");
    let quoi = args.texte("quoi").unwrap_or("liste");
    let salarie = args.texte("salarie").filter(|s| !s.is_empty());
    let le = match args.texte("le").filter(|t| !t.is_empty()) {
        Some(t) => match jour_option("le", t) {
            Ok(j) => Some(j.iso()),
            Err(e) => return Ok(refuse(json, &e)),
        },
        None => None,
    };

    if quoi == "liste" {
        let r = match cadre.route("fins-de-contrat", json!({"mois": mois})) {
            Ok(r) => r,
            Err(e) if est_refus(e.classe) => return Ok(refuse(json, &e)),
            Err(e) => return Err(e),
        };
        let fins: Vec<Value> = r.get("fins").and_then(Value::as_array).cloned().unwrap_or_default();
        if json {
            // Le Python imprime la liste telle quelle, sans enveloppe.
            dire!("{}", socle::ecriture::json_indente(&Value::Array(fins), 2));
            return Ok(0);
        }
        if fins.is_empty() {
            dire!("Aucun contrat ne se termine en {} dont la fin se signale.", mois);
            dire!("  Un CDD d'usage du spectacle arrive a son terme (031) n'en fait pas");
            dire!("  partie : sa fin part dans la DSN mensuelle, par le circuit derogatoire.");
            dire!("  Rompu avant son terme (036, 037, 084...), il se signale (fiche 2062).");
            return Ok(0);
        }
        for f in &fins {
            dire!("{}  {}  fin le {}  ({})", champ(f, "salarie"), champ(f, "numero"), champ(f, "fin"), champ(f, "nature"));
            // Le delai du signalement et ou on en est, tels que le moteur les rend (`obligations.rs`).
            if let Some(o) = f.get("obligation").filter(|o| o.is_object()) {
                dire!("    {}", champ(o, "phrase"));
            }
            let a_saisir: Vec<Value> = f.get("a_saisir").and_then(Value::as_array).cloned().unwrap_or_default();
            for m in &a_saisir {
                dire!("    A SAISIR : {}", super::rendu::texte(m));
            }
            if a_saisir.is_empty() {
                dire!("    pret a signaler : moteur fin-contrat signaler {} {}", mois, champ(f, "salarie"));
            }
        }
        return Ok(0);
    }

    let Some(salarie) = salarie else {
        // Le Python le dit en texte, meme avec --json.
        dire!("! Quel salarie ? `moteur fin-contrat liste {}` les montre.", mois);
        return Ok(2);
    };

    if quoi == "signaler" {
        let mut charge = json!({"mois": mois, "salarie": salarie});
        if let Some(r) = args.texte("remplace").filter(|t| !t.is_empty()) {
            charge["remplace"] = json!(r);
        }
        // 0 ou absent : le suivant de tous les signalements deja partis (le moteur le lit dans le journal).
        if let Some(o) = args.entier("ordre").filter(|o| *o != 0) {
            charge["ordre"] = json!(o);
        }
        if args.drapeau("test") {
            charge["test"] = json!(true);
        }
        if let Some(l) = &le {
            charge["le"] = json!(l);
        }
        let r = match cadre.route("fin-contrat-signaler", charge) {
            Ok(r) => r,
            Err(e) if est_refus(e.classe) => return Ok(refuse(json, &e)),
            Err(e) => return Err(e),
        };
        let avertissements: Vec<Value> = r.get("avertissements").and_then(Value::as_array).cloned().unwrap_or_default();
        if json {
            let sortie = json!({"fichier": r["fichier"], "ordre": r["ordre"], "type": r["type"], "avertissements": avertissements});
            dire!("{}", socle::ecriture::json_indente(&sortie, 2));
            return Ok(0);
        }
        dire!("Signalement de fin de contrat ecrit : {}", champ(&r, "fichier"));
        dire!("  type {}   ordre {}   preavis {}", champ(&r, "type"), champ(&r, "ordre"), champ(&r, "preavis"));
        for a in &avertissements {
            dire!("  - {}", super::rendu::texte(a));
        }
        dire!("\nRien n'est depose. Le depot est un acte a part :");
        dire!("  moteur depot-dsn {} --test", champ(&r, "fichier"));
        return Ok(0);
    }

    // documents
    let mut charge = json!({"mois": mois, "salarie": salarie});
    if let Some(l) = &le {
        charge["le"] = json!(l);
    }
    let r = match cadre.route("fin-contrat-documents", charge) {
        Ok(r) => r,
        Err(e) if est_refus(e.classe) => return Ok(refuse(json, &e)),
        Err(e) => return Err(e),
    };
    let avertissements: Vec<Value> = r.get("avertissements").and_then(Value::as_array).cloned().unwrap_or_default();
    if json {
        let sortie = json!({"certificat": r["certificat"], "recu": r["recu"], "avertissements": avertissements});
        dire!("{}", socle::ecriture::json_indente(&sortie, 2));
        return Ok(0);
    }
    dire!("Certificat de travail        : {}", champ(&r, "certificat"));
    dire!("Recu pour solde de tout compte : {}", champ(&r, "recu"));
    for a in &avertissements {
        dire!("  - {}", super::rendu::texte(a));
    }
    Ok(0)
}

