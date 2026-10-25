//! `paie nouveau AAAA-MM [--force] [--reprendre] [--json]` : creer le fichier de saisie d'un mois
//! (`moteur/cli.py`, `cmd_nouveau` ; `operations_paies.nouvelle_saisie`). Rien ne part : un fichier `paies/AAAA-MM.json`.
//!
//! `--reprendre` y ecrit, a la place des deux exemples, les contrats en cours des mois precedents (bornes et numero) : la
//! route `contrats-en-cours` du moteur les dit, avec l'engagement de chacun, tel que la fenetre le pose d'office.
//!
//! DEFAUT DU PYTHON, NE SE REPRODUIT PAS : un mois passe (`MoisHorsDelai`), un mois mal ecrit (`2026-13`, `foo`) donnaient une
//! trace d'appels au lieu d'une phrase. Ici : un refus d'une phrase (`! ...`, code 2).

use serde_json::{json, Value};

use super::actes::{mois_valide, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "nouveau",
    positionnels: &[Pos::requis("mois")],
    options: &[Opt::drapeau("force"), Opt::drapeau("reprendre"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn refus(m: String) -> Echec {
    Echec { classe: Some("ErreurOperation"), message: m }
}

/// Les contrats en cours du mois, sans leur engagement (`sortie_json.contrat_en_cours`), et l'engagement de chacun.
fn en_cours(cadre: &Cadre, mois: &str) -> Result<Vec<(Value, Value)>, Echec> {
    let r = cadre.route("contrats-en-cours", json!({"mois": mois}))?;
    let mut out = Vec::new();
    for c in r.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default() {
        let mut vue = c.as_object().cloned().unwrap_or_default();
        let engagement = vue.remove("engagement").unwrap_or(Value::Null);
        out.push((Value::Object(vue), engagement));
    }
    Ok(out)
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let mois = args.texte("mois").unwrap_or("");
    let (force, reprendre) = (args.drapeau("force"), args.drapeau("reprendre"));
    if !mois_valide(mois) {
        return Ok(reponse_metier("nouveau", json, &refus(format!("Le mois « {} » ne se lit pas : AAAA-MM attendu (par exemple 2026-10).", mois)), &[]));
    }
    let chemin = cadre.racine.join("paies").join(format!("{}.json", mois));
    if chemin.exists() && !force {
        let message = format!("{} existe deja.", chemin.display());
        if json {
            return Ok(rendre(&erreur("nouveau", "FichierExistant", &message, &[("chemin", json!(chemin.display().to_string()))])));
        }
        dire!("{}  (--force pour ecraser)", message);
        return Ok(1);
    }
    // Un mois passe ne se cree plus : sa DSN etait due.
    if let Err(e) = crate::routes_ecritures::refuser_si_hors_delai(&cadre.ctx, mois, false) {
        if super::actes::est_operation(e.classe) {
            return Ok(reponse_metier("nouveau", json, &e, &[]));
        }
        return Err(e);
    }
    let mut data = crate::routes_saisie::squelette(&cadre.ctx, mois)?;
    if reprendre {
        let reprises = en_cours(cadre, mois)?;
        if !reprises.is_empty() {
            if let Some(o) = data.as_object_mut() {
                o.remove("spectacle");
                o.insert("engagements".into(), Value::Array(reprises.into_iter().map(|(_, e)| e).collect()));
            }
        }
    }
    socle::ecriture::ecrire_json(&chemin, &data, 2).map_err(|m| Echec { classe: Some("OSError"), message: m })?;

    // Ce qui court encore, relu apres l'ecriture (`deja_repris` dit ce que le fichier vient de recevoir).
    let courants = en_cours(cadre, mois)?;
    if json {
        let saisie = crate::routes_saisie::lire_saisie(&chemin)?;
        let contrats: Vec<Value> = courants.into_iter().map(|(c, _)| c).collect();
        let r = enveloppe("nouveau", json!({"mois": mois, "chemin": chemin.display().to_string(), "saisie": saisie, "contrats_en_cours": contrats}));
        return Ok(rendre(&r));
    }
    dire!("Cree : {}", chemin.display());
    if !courants.is_empty() {
        dire!();
        dire!("{} contrat(s) en cours depuis un mois precedent :", courants.len());
        for (c, _) in &courants {
            let numero = match c.get("numero") {
                Some(n) if socle::fichiers::vrai(n) => super::rendu::texte(n),
                _ => "?".to_string(),
            };
            dire!(
                "  {:<24} {} -> {}  n° {}  (depuis {}){}",
                champ(c, "nom"),
                champ(c, "debut"),
                champ(c, "fin"),
                numero,
                crate::engagement::libelle_mois(&champ(c, "depuis")),
                if reprendre { "  -- ecrit dans le fichier, poser les jours" } else { "" }
            );
        }
        if !reprendre {
            dire!(
                "  Pour les reprendre : recopier `debut`, `fin` et `numero` dans un engagement du fichier (ou `paie nouveau {} --force --reprendre`). Ni contrat ni DPAE ne seront refaits : ils datent du premier mois.",
                mois
            );
        }
    }
    dire!("Modifier les dates et les noms, puis :  paie mois {}", chemin.display());
    Ok(0)
}
