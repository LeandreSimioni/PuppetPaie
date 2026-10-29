//! `moteur cloture [ANNEE] [--rouvrir] [--forcer] [--json]` : cloturer un exercice (le zip de l'annee), le rouvrir, ou dire l'etat de
//! chaque exercice (`moteur/commandes_exercice.py`, `cmd_cloture`). Routes `exercices` (l'etat), `cloture` (cloturer, rouvrir ;
//! `exercice_ecrit.rs`). `qui` : `cli`, comme le Python.
//!
//! Sans annee : l'etat de chaque exercice (lecture seule). Avec une annee : fige l'exercice (`config/exercices.json`) et ecrit
//! `sorties/exercices/Exercice_AAAA.zip` ; `--forcer` cloture meme si un mois n'a pas de DSN acceptee ; `--rouvrir` retire la cloture
//! (l'archive reste). Rien ne part, rien n'est lu au coffre : pas de mot de passe.
//!
//! LE JSON d'une cloture est celui du Python (`Cloture` : `annee, le, archive` -- le NOM du zip --, `octets, empreinte, contenu, manques`) ;
//! la route rend le chemin entier et `fichiers` : la commande les remet en forme. Le texte garde la forme du Python.
//!
//! DEFAUT DU PYTHON, QUI NE SE REPRODUIT PAS : une annee illisible (`abc`), un fichier d'exercices abime, un disque plein sortaient en
//! trace d'appels ; ici, `! phrase` (code 2) pour ce que la route nomme un refus, un plantage propre sinon.
//!
//! POINT DOUTEUX : l'empreinte et la taille du zip dependent du compresseur (le Rust n'est pas zlib) ; le contenu, lui, est identique.

use serde_json::{json, Value};

use super::actes::{est_refus, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, debut, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "cloture",
    positionnels: &[Pos::facultatif("annee", None)],
    options: &[Opt::drapeau("rouvrir"), Opt::drapeau("forcer"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) || e.classe == Some("CalculRefuse") => reponse_metier("cloture", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

/// Le nom d'un chemin (`Path.name`).
fn nom_de(chemin: &str) -> String {
    chemin.rsplit(['/', '\\']).next().unwrap_or(chemin).to_string()
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json_ = args.drapeau("json");
    let Some(annee) = args.texte("annee").filter(|t| !t.is_empty()) else {
        return etat(cadre, json_);
    };
    if args.drapeau("rouvrir") {
        let r = cadre.route("cloture", json!({"annee": annee, "rouvrir": true, "qui": "cli"}))?;
        if json_ {
            return Ok(rendre(&enveloppe("cloture", r)));
        }
        let depuis = r.get("etait_clos_depuis").filter(|v| socle::fichiers::vrai(v)).map(super::rendu::texte).unwrap_or_else(|| "?".to_string());
        dire!("Exercice {} rouvert (il etait clos depuis le {} ; l'archive {} reste).", champ(&r, "annee"), debut(&depuis, 10), champ(&r, "archive"));
        return Ok(0);
    }
    let c = cadre.route("cloture", json!({"annee": annee, "forcer": args.drapeau("forcer"), "qui": "cli"}))?;
    if json_ {
        // `sortie_json.brut(Cloture)` : le chemin de l'archive ne rend que son nom ; `contenu` est le nombre de fichiers.
        let archive = nom_de(&champ(&c, "archive"));
        let v = json!({
            "annee": c["annee"], "le": c["le"], "archive": archive, "octets": c["octets"], "empreinte": c["empreinte"],
            "contenu": c["fichiers"], "manques": c["manques"],
        });
        return Ok(rendre(&enveloppe("cloture", v)));
    }
    let octets = c.get("octets").and_then(Value::as_u64).unwrap_or(0);
    dire!(
        "Exercice {} clos le {} : {} ({} fichiers, {} Ko, sha256 {}...)",
        champ(&c, "annee"),
        debut(&champ(&c, "le"), 10),
        champ(&c, "archive"),
        champ(&c, "fichiers"),
        octets / 1024,
        debut(&champ(&c, "empreinte"), 12)
    );
    for m in super::actes::textes_de(&c, "manques") {
        dire!("  ! {}", m);
    }
    Ok(0)
}

/// Sans annee : l'etat de chaque exercice.
fn etat(cadre: &Cadre, json_: bool) -> Result<i32, Echec> {
    let r = cadre.route("exercices", json!({}))?;
    let etats: Vec<Value> = r.get("exercices").and_then(Value::as_array).cloned().unwrap_or_default();
    if json_ {
        return Ok(rendre(&enveloppe("cloture", Value::Array(etats))));
    }
    if etats.is_empty() {
        dire!("Aucun exercice connu.");
    }
    for e in &etats {
        let nb = |cle: &str| e.get(cle).and_then(Value::as_array).map_or(0, Vec::len);
        let clos = e.get("clos").filter(|v| socle::fichiers::vrai(v));
        let statut = match clos {
            Some(c) => format!("CLOS le {} ({})", debut(&champ(c, "le"), 10), champ(c, "archive")),
            None => "ouvert".to_string(),
        };
        let export = match e.get("export").filter(|v| socle::fichiers::vrai(v)) {
            Some(x) => format!(", export compta du {}", debut(&champ(x, "ecrit"), 10)),
            None => String::new(),
        };
        dire!("{} : {} mois, {} DSN acceptees -- {}{}", champ(e, "annee"), nb("mois"), nb("dsn_acceptees"), statut, export);
    }
    Ok(0)
}
