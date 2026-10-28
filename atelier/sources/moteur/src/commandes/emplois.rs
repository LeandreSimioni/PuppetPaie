//! `moteur emplois [--tous] [--chercher MOT] [--autres] [--ajouter CODE ...] [--retirer CODE ...] [--json]` : le catalogue des metiers
//! (`moteur/cli.py`, `cmd_emplois`). `--ajouter` / `--retirer` mettent au deroulant (ou en otent) des metiers d'une autre
//! convention : route `metiers-ajoutes` (ecrit `config/metiers-ajoutes.json`, une preference d'affichage, rien ne part).
//!
//! `--tous` (la nomenclature France Travail entiere, 1 236 codes) n'est PAS porte : `emplois::catalogue` n'a pas de nomenclature.
//! La commande le dit en une phrase (code 2) au lieu de rendre une liste incomplete.
//!
//! Le Python ne distingue pas `--ajouter` et `--retirer` donnes ensemble : les codes de `--ajouter`, retires. Idem ici.

use serde_json::{json, Value};

use super::actes::{est_operation, reponse_metier};
use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, debut, enveloppe, rendre};
use super::Cadre;
use socle::fichiers::vrai;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "emplois",
    positionnels: &[],
    options: &[
        Opt::drapeau("tous"),
        Opt::valeur("chercher"),
        Opt::drapeau("autres"),
        Opt::plusieurs("ajouter"),
        Opt::plusieurs("retirer"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_operation(e.classe) => reponse_metier("emplois", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    if args.drapeau("tous") {
        let e = Echec {
            classe: Some("ErreurOperation"),
            message: "`moteur emplois --tous` (la nomenclature France Travail entiere) n'est pas encore porte ; `moteur emplois` donne le catalogue du spectacle vivant.".into(),
        };
        return Ok(reponse_metier("emplois", json, &e, &[]));
    }
    let retirer = args.drapeau("retirer");
    let source = if args.drapeau("ajouter") { args.liste("ajouter") } else { args.liste("retirer") };
    if args.drapeau("ajouter") || retirer {
        let codes: Vec<Value> = source.iter().map(|c| json!(c.trim_matches(socle::texte::blanc).to_uppercase())).collect();
        cadre.route("metiers-ajoutes", json!({"codes": codes, "retirer": retirer}))?;
    }
    let liste = if args.drapeau("autres") {
        crate::routes::accueil::metiers_autres_conventions(&cadre.ctx)?
    } else {
        crate::routes::accueil::metiers(&cadre.ctx)?
    };
    let mut liste: Vec<Value> = liste.as_array().cloned().unwrap_or_default();
    if let Some(q) = args.texte("chercher").filter(|q| !q.is_empty()) {
        let q = q.to_lowercase();
        liste.retain(|m| champ(m, "libelle").to_lowercase().contains(&q) || champ(m, "code").to_lowercase().contains(&q));
    }
    if json {
        return Ok(rendre(&enveloppe("emplois", json!({"total": liste.len(), "emplois": liste}))));
    }
    dire!();
    dire!("{} metier(s)", liste.len());
    dire!("{}", "-".repeat(78));
    let mut famille: Option<String> = None;
    for m in &liste {
        let groupe = if m.get("de").is_some_and(vrai) { champ(m, "de") } else { champ(m, "famille") };
        if famille.as_deref() != Some(groupe.as_str()) {
            dire!();
            dire!("  {}", groupe.to_uppercase());
            famille = Some(groupe);
        }
        let pcs = if m.get("pcs_ese").is_some_and(vrai) { champ(m, "pcs_ese") } else { "-".to_string() };
        let cadre_txt = if m.get("cadre_par_defaut").is_some_and(vrai) { " cadre" } else { "" };
        dire!("   {:<7} {:<44} {:<10} {:<5}{}", champ(m, "code"), debut(&champ(m, "libelle"), 44), champ(m, "statut"), pcs, cadre_txt);
    }
    dire!("{}", "-".repeat(78));
    dire!(
        "\ncode = code emploi France Travail (DSN S21.G00.40.005) ; PCS-ESE (40.004) ;\nstatut = artiste (annexe 10, taux abattus) ou technicien (annexe 8).\nDans une paie : \"metier\": \"REG120\", et au besoin \"cadre\": true.\nSources : `moteur sources`."
    );
    Ok(0)
}
