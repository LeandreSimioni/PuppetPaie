//! `paie nouveau AAAA-MM [--force] [--json]` : ouvrir un mois, `engagements/AAAA-MM.json`, avec son taux de cachet et sa
//! date de paiement. Rien ne part. Les contrats des mois d'avant qui courent encore y sont deja : un engagement est un
//! seul objet, du mois ou il commence a celui ou il finit.
//!
//! Un mois passe (`MoisHorsDelai`), un mois mal ecrit (`2026-13`, `foo`) : un refus d'une phrase (`! ...`, code 2).

use serde_json::{json, Value};

use super::actes::{mois_valide, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "nouveau",
    positionnels: &[Pos::requis("mois")],
    options: &[Opt::drapeau("force"), Opt::drapeau("json")],
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
    let force = args.drapeau("force");
    if !mois_valide(mois) {
        return Ok(reponse_metier("nouveau", json, &refus(format!("Le mois « {} » ne se lit pas : AAAA-MM attendu (par exemple 2026-10).", mois)), &[]));
    }
    let chemin = engagement::stock::chemin(&cadre.racine, mois);
    if chemin.exists() && !force {
        let message = format!("{} existe deja.", chemin.display());
        if json {
            return Ok(rendre(&erreur("nouveau", "FichierExistant", &message, &[("chemin", json!(chemin.display().to_string()))])));
        }
        dire!("{}  (--force pour en reposer le taux et la date de paiement)", message);
        return Ok(1);
    }
    // Un mois d.avant la premiere embauche ne se cree pas : aucune DSN n.y est due.
    if let Err(e) = crate::routes_ecritures::refuser_si_avant_la_premiere_embauche(&cadre.ctx, mois) {
        if super::actes::est_operation(e.classe) {
            return Ok(reponse_metier("nouveau", json, &e, &[]));
        }
        return Err(e);
    }
    // Le mois dit son taux et sa date de paiement ; ses engagements restent ceux qu'il montre (ceux des mois d'avant qui
    // courent encore y sont deja).
    let mut data = crate::routes_saisie::squelette(&cadre.ctx, mois)?;
    if let Some(vue) = engagement::stock::vue_du_mois(&cadre.racine, mois).map_err(Echec::from)? {
        data["engagements"] = vue.get("engagements").cloned().unwrap_or_else(|| json!([]));
    }
    engagement::stock::ecrire_vue_du_mois(&cadre.ctx, mois, &data).map_err(|m| Echec { classe: Some("ErreurOperation"), message: m })?;
    for cle in ["taux_cachet", "date_paiement"] {
        engagement::stock::poser_au_mois(&cadre.racine, mois, cle, data[cle].clone()).map_err(|m| Echec { classe: Some("OSError"), message: m })?;
    }

    let courants = en_cours(cadre, mois)?;
    if json {
        let saisie = engagement::saisie_du_mois(&cadre.racine, mois).map_err(Echec::from)?;
        let contrats: Vec<Value> = courants.into_iter().map(|(c, _)| c).collect();
        let r = enveloppe("nouveau", json!({"mois": mois, "chemin": chemin.display().to_string(), "saisie": saisie, "contrats_en_cours": contrats}));
        return Ok(rendre(&r));
    }
    dire!("Cree : {}", chemin.display());
    if !courants.is_empty() {
        dire!();
        dire!("{} contrat(s) en cours depuis un mois precedent, deja dans le mois :", courants.len());
        for (c, _) in &courants {
            let numero = match c.get("numero") {
                Some(n) if socle::fichiers::vrai(n) => super::rendu::texte(n),
                _ => "?".to_string(),
            };
            dire!(
                "  {:<24} {} -> {}  n° {}  (depuis {})",
                champ(c, "nom"),
                champ(c, "debut"),
                champ(c, "fin"),
                numero,
                engagement::libelle_mois(&champ(c, "depuis"))
            );
        }
    }
    dire!("Saisir les engagements, les valider, puis :  paie depot-dsn {} --test", mois);
    Ok(0)
}
