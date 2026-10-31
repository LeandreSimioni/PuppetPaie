//! Le magasin `config/pas.json` : les taux de prelevement a la source que la DGFiP a fait connaitre (un compte rendu de
//! nature 94, `suites`) ou que les DSN deposees declarent (`moteur pas --amorcer`, l'import). `droit::pas` les lit.

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::Echec;

pub(crate) fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

fn maintenant_court() -> String {
    // A l'heure de Paris, a la minute.
    socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].replace('T', " ")
}

/// Le magasin, indente de deux espaces, sans saut de ligne final.
fn ecrire(ctx: &Contexte, data: &Value) -> Result<(), Echec> {
    let p = ctx.racine.join("config").join("pas.json");
    std::fs::write(&p, socle::ecriture::json_indente(data, 2)).map_err(|e| Echec { classe: Some("OSError"), message: format!("{} : {}", p.display(), e) })
}

/// Applique des releves de CRM ; rend les changements lisibles. Un releve :
/// `{"salarie", "taux", "type", "identifiant", "periode", "mis_a_disposition", "applique_le"}`.
pub fn enregistrer(ctx: &Contexte, releves: &[Value], source: &str) -> Result<Vec<String>, Echec> {
    let mut data = droit::pas::charger(ctx).map_err(ko)?;
    let mut changements = Vec::new();
    let mut soldees = false;
    let horodatage = maintenant_court();
    for r in releves {
        let cle = r.get("salarie").and_then(Value::as_str).unwrap_or("").to_string();
        // LA REPONSE EST LA : la demande qui l'attendait n'a plus lieu d'etre. Elle se solde AVANT le test d'egalite.
        let demandes = data["demandes"].as_object_mut().ok_or_else(|| ko("demandes illisibles".into()))?;
        soldees = demandes.shift_remove(&cle).is_some() || soldees;
        let avant = data["salaries"].get(&cle).cloned();
        let get = |k: &str| r.get(k).cloned().unwrap_or(Value::Null);
        let mut neuf = json!({
            "taux": get("taux"), "type": get("type"), "identifiant": get("identifiant"),
            "periode": get("periode"), "mis_a_disposition": get("mis_a_disposition"),
            "source": source, "releve_le": horodatage,
        });
        if let Some(a) = r.get("applique_le").filter(|a| socle::fichiers::vrai(a)) {
            neuf["applique_le"] = a.clone();
        }
        if let Some(a) = avant.as_ref().filter(|a| socle::fichiers::vrai(a)) {
            if ["taux", "type", "identifiant"].iter().all(|k| a.get(*k) == neuf.get(*k)) {
                continue;
            }
        }
        let t = |v: &Value, k: &str| crate::commandes::rendu::champ(v, k);
        data["salaries"].as_object_mut().ok_or_else(|| ko("salaries illisibles".into()))?.insert(cle.clone(), neuf.clone());
        match avant.filter(|a| socle::fichiers::vrai(a)) {
            Some(a) => changements.push(format!(
                "{} : taux {} -> {}, type {} -> {}, identifiant {} -> {}",
                cle, t(&a, "taux"), t(&neuf, "taux"), t(&a, "type"), t(&neuf, "type"), t(&a, "identifiant"), t(&neuf, "identifiant")
            )),
            None => changements.push(format!("{} : premier releve - taux {}, type {}, identifiant {}", cle, t(&neuf, "taux"), t(&neuf, "type"), t(&neuf, "identifiant"))),
        }
        let mut ligne = neuf.as_object().cloned().unwrap_or_default();
        ligne.insert("salarie".into(), json!(cle));
        data["journal"].as_array_mut().ok_or_else(|| ko("journal illisible".into()))?.push(Value::Object(ligne));
    }
    if !changements.is_empty() || soldees {
        ecrire(ctx, &data)?;
    }
    if !changements.is_empty() {
        refaire_les_paies(ctx)?;
    }
    Ok(changements)
}

/// UN TAUX RECU REFAIT, DANS LE MEME GESTE, LES PAIES QU'IL TOUCHE (le recalcul de lui-meme, « taux de PAS reçu ») :
/// chaque mois saisi se relit (`engagement::etats::du_mois`), et ses paies validees dont l'empreinte ne tient plus se
/// reecrivent. Un mois dont les documents sont partis ne bouge pas. Aucune lecture ulterieure n'a donc a le decouvrir.
fn refaire_les_paies(ctx: &Contexte) -> Result<(), Echec> {
    let departs = envois::journal::lu(&ctx.racine).map_err(ko)?.departs;
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(ko)? {
        engagement::etats::du_mois(ctx, &mois, &departs).map_err(ko)?;
    }
    Ok(())
}
