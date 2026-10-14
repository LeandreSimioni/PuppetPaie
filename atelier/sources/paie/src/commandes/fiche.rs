//! `paie fiche [--archives] [--json]` : rappeler les donnees de reference (`moteur/cli.py`, `cmd_fiche`).
//! Elle lit le `Contexte` ; rien ne s'ecrit.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{brut, enveloppe, exiger, json_ligne, rendre};
use super::Cadre;
use crate::contexte::archivee;
use crate::retours::Echec;

pub static SPEC: Spec = Spec { commande: "fiche", positionnels: &[], options: &[Opt::drapeau("archives"), Opt::drapeau("json")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(code) => code,
        Err(e) => cadre.plantage(e),
    }
}

/// `compteurs.json`, ou `{}` : `Contexte.compteurs`.
fn compteurs(cadre: &Cadre) -> Result<Value, Echec> {
    let p = cadre.racine.join("config").join("compteurs.json");
    if !p.exists() {
        return Ok(json!({}));
    }
    let t = std::fs::read_to_string(&p).map_err(|e| Echec { classe: Some("OSError"), message: format!("{} : {}", p.display(), e) })?;
    serde_json::from_str(&t).map_err(|e| Echec { classe: Some("JSONDecodeError"), message: e.to_string() })
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let archives = args.drapeau("archives");
    let ctx = &cadre.ctx;
    let a_part = |m: &Map<String, Value>| -> BTreeSet<String> { m.iter().filter(|(_, v)| archivee(v)).map(|(k, _)| k.clone()).collect() };
    let (salaries_archives, spectacles_archives) = (a_part(&ctx.salaries), a_part(&ctx.spectacles));
    let choisir = |m: &Map<String, Value>| -> Map<String, Value> { m.iter().filter(|(_, v)| archives || !archivee(v)).map(|(k, v)| (k.clone(), v.clone())).collect() };
    let (salaries, spectacles) = (choisir(&ctx.salaries), choisir(&ctx.spectacles));
    let compteurs = compteurs(cadre)?;

    if args.drapeau("json") {
        let brutes = |m: &Map<String, Value>| Value::Object(m.iter().map(|(k, v)| (k.clone(), brut(v))).collect());
        let entreprise: Map<String, Value> =
            ctx.entreprise.as_object().map(|o| o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), brut(v))).collect()).unwrap_or_default();
        return Ok(rendre(&enveloppe(
            "fiche",
            json!({
                "entreprise": Value::Object(entreprise),
                "salaries": brutes(&salaries),
                "spectacles": brutes(&spectacles),
                "salaries_archives": salaries_archives,
                "spectacles_archives": spectacles_archives,
                "compteurs": compteurs,
            }),
        )));
    }

    dire!("Entreprise : {} - SIRET {} - IDCC {}", exiger(&ctx.entreprise, "raison_sociale")?, exiger(&ctx.entreprise, "siret")?, exiger(&ctx.entreprise, "idcc")?);
    dire!("\nSalaries :");
    for (k, s) in &salaries {
        let nir = if s.get("nir").is_some_and(crate::fichiers::vrai) {
            format!("NIR {}{}", exiger(s, "nir")?, exiger(s, "nir_cle")?)
        } else {
            "NIR aucun (declare sous NTT)".to_string()
        };
        dire!(
            "  {:<10} {} {}  {}  code {}{}",
            k,
            exiger(s, "nom")?,
            exiger(s, "prenom")?,
            nir,
            exiger(s, "code_salarie")?,
            if archivee(s) { "   [archive]" } else { "" }
        );
    }
    dire!("\nSpectacles :");
    for (k, sp) in &spectacles {
        dire!("  {:<22} {:<24} objet {}{}", k, exiger(sp, "titre")?, exiger(sp, "numero_objet")?, if archivee(sp) { "   [archive]" } else { "" });
    }
    if !archives && (!salaries_archives.is_empty() || !spectacles_archives.is_empty()) {
        // Le dire, et ne pas les lister : une fiche archivee dont personne ne parle passe pour perdue, et on la ressaisit.
        dire!("");
        dire!("Archives : {} salarie(s), {} spectacle(s). `--archives` les montre.", salaries_archives.len(), spectacles_archives.len());
    }
    dire!("\nCompteurs :");
    dire!("  {}", json_ligne(&compteurs));
    Ok(0)
}
