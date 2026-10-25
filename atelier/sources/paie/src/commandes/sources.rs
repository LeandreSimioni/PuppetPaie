//! `paie sources [--verifier] [--relever] [--json]` : d'ou viennent les donnees livrees, de quand elles datent, et -- avec
//! `--verifier` -- si la source en ligne a bouge depuis qu'on l'a lue (`moteur/cli.py`, `cmd_sources`, retire au lot K ;
//! le travail est dans `sources.rs` et `kali.rs`).
//!
//! UN OUTIL DE L'EDITEUR (Leandre, 11/10/2026 : lance avant chaque publication ; rien pour une compagnie, qui ne fait que
//! recevoir la mise a jour signee). Il est dans le `paie` livre, comme `paie maj --publier`, parce que c'est ce
//! programme-la que le poste de l'editeur fait tourner (il ne compile pas le Rust ; `paie-atelier` n'est jamais livre),
//! et il est refuse ailleurs que sur ce poste, de la meme facon que la publication (`maj::exiger_editeur` :
//! le reglage de l'installation, `"editeur": true`, et jamais depuis une copie). CACHEE : elle n'apparait pas dans l'aide.
//!
//! ATTENTION : `--verifier` va au RESEAU (chaque source « telechargement », et l'index KALI de la DILA). Lecture seule,
//! hors `atelier/reference/sources/` de l'installation (le depot, sur le poste de l'editeur ; `sources::COPIES`) : le
//! resume des livraisons KALI (`veille-kali.json`), et avec `--relever`, la nouvelle version d'une source a cote de
//! l'ancienne et le XML des textes KALI touches (`kali/`).

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{enveloppe, rendre};
use super::Cadre;

pub static SPEC: Spec = Spec {
    commande: "sources",
    positionnels: &[],
    options: &[Opt::drapeau("verifier"), Opt::drapeau("relever"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if let Err(m) = crate::maj::exiger_editeur(&cadre.racine, "la verification des sources") {
        eprintln!("{} La verification des sources est un outil de l'editeur, lance avant chaque publication.", m);
        return 1;
    }
    let code = cadre.ctx.code.clone();
    let manifeste = match crate::sources::manifeste(&code) {
        Ok(m) => m,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    // `--relever` ne se fait qu'en retournant voir : il emporte `--verifier`.
    let relever = args.drapeau("relever");
    let verifier = args.drapeau("verifier") || relever;
    // Les copies sont les textes du logiciel, pas ceux d'une compagnie : elles vivent dans l'installation.
    let installation = crate::maj::installation();
    let liste = if verifier {
        let mut tele = |u: &str| crate::sources::telecharger(u, 30);
        let mut kali = |u: &str| crate::sources::telecharger(u, 60).map(|(o, _)| o);
        crate::sources::verifier(&installation, &manifeste, &cadre.ctx.aujourdhui, &[], relever, &mut tele, &mut kali)
    } else {
        crate::sources::etats(&installation, &manifeste, &cadre.ctx.aujourdhui)
    };
    let liste = match liste {
        Ok(l) => l,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    if args.drapeau("json") {
        rendre(&enveloppe("sources", json!({"total": liste.len(), "sources": liste})));
        return 0;
    }
    let s = |e: &Value, k: &str| match &e[k] {
        Value::Null => "?".to_string(),
        v => socle::valeur::valeur_en_texte(v),
    };
    dire!();
    for e in &liste {
        dire!("{}", s(e, "ident"));
        dire!("   {}", s(e, "titre"));
        dire!("   {} -- version {}, relevee le {} (il y a {} j)", s(e, "editeur"), s(e, "version"), s(e, "releve_le"), s(e, "age_jours"));
        dire!("   {}", s(e, "contenu"));
        let copie = if e["copie_conforme"] == json!(true) {
            "copie conforme".to_string()
        } else if e["copie_presente"] == json!(true) {
            "COPIE DIFFERENTE du manifeste".to_string()
        } else if e["transcription_presente"] == json!(true) {
            format!("original hors du depot public, transcription : {}/{}", crate::sources::COPIES, s(e, "transcription"))
        } else {
            format!("copie absente ({}/{})", crate::sources::COPIES, s(e, "fichier"))
        };
        let nourrit: Vec<String> = e["nourrit"].as_array().into_iter().flatten().map(socle::valeur::valeur_en_texte).collect();
        dire!("   {} ; nourrit : {}", copie, if nourrit.is_empty() { "rien (verification)".to_string() } else { nourrit.join(", ") });
        if let Some(en_ligne) = e["en_ligne"].as_str() {
            let marque = match en_ligne {
                "inchangee" => "OK",
                "changee" => "A RELIRE",
                "injoignable" => "??",
                _ => "A LA MAIN",
            };
            dire!("   en ligne : {} {}", marque, s(e, "detail"));
            if matches!(en_ligne, "manuelle" | "changee") && e["page_de_veille"].is_string() {
                dire!("   ouvrir : {}", s(e, "page_de_veille"));
            }
        }
        dire!();
    }
    if !verifier {
        dire!("`--verifier` retelecharge chaque source et compare ; `--relever` pose la nouvelle version a cote de l'ancienne quand elle a change.");
    }
    0
}
