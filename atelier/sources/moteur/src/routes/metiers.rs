//! Configuration › Métiers : les metiers que la compagnie coche, ceux de sa convention et ceux qu'elle prend aux autres.

use std::collections::HashMap;


use serde_json::{json, Map, Value};

use crate::routes::catalogue::{Acte, Champ, Ecran, Fiche, Place, MANQUE, OPERATION};
use crate::routes::fiches::{creer_dossier, ecrire, plain, refus, texte_requis};
use droit::contexte::Contexte;
use droit::emplois;
use envois::etats;
use crate::routes::mois::sauver_avant;
use crate::routes::ouverture::{metier_visible, preferences_metiers};
use socle::echec::{Echec, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

const METIERS_REND: &str = "emplois (les métiers proposés) et emplois_autres (ceux des autres conventions)";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Metiers);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "metiers-catalogue",
        geste: "Dire les métiers du catalogue, ceux de la convention et les autres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "metiers (le catalogue, chacun dit retenu ou non)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "metier-retenu",
        geste: "Proposer un métier du catalogue dans les listes, ou ne plus le proposer.",
        charge: &[Champ { nom: "code", sens: "le code du métier au catalogue", avance: false }, Champ { nom: "retenu", sens: "vrai : le proposer", avance: false }],
        exemple: r#"{"code":"CHE350","retenu":true}"#,
        rend: METIERS_REND,
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "metiers-catalogue" => metiers_catalogue(ctx),
        "metier-retenu" => metier_retenu(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// L'onglet
/// Metiers, tout le catalogue, chaque metier coche ou non.
fn metiers_catalogue(ctx: &Contexte) -> R<Value> {
    let familles = emplois::familles().map_err(plain)?;
    let ordre: HashMap<&str, usize> = familles.iter().enumerate().map(|(i, f)| (f.as_str(), i)).collect();
    let noms = emplois::noms_des_conventions().map_err(plain)?;
    let (ajoutes, retires) = preferences_metiers(&ctx.racine)?;
    let idcc = ctx.entreprise.get("idcc").filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
    let mut liste: Vec<Map<String, Value>> = Vec::new();
    for m in emplois::catalogue().map_err(plain)? {
        let conventions: Vec<Value> = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
        // Sans IDCC (une compagnie qui s'installe), rien ne se cache.
        let conv = conventions.is_empty() || idcc.is_empty() || conventions.iter().any(|c| c.as_str() == Some(idcc.as_str()));
        let de = if conv || conventions.is_empty() {
            String::new()
        } else {
            conventions.iter().map(|c| noms.get(&valeur_en_texte(c)).map(valeur_en_texte).unwrap_or_else(|| valeur_en_texte(c))).collect::<Vec<_>>().join(", ")
        };
        let mut e = m.as_object().cloned().unwrap_or_default();
        e.insert("retenu".into(), json!(metier_visible(&m, &idcc, &ajoutes, &retires)));
        e.insert("de_la_convention".into(), json!(conv));
        e.insert("de".into(), json!(de));
        liste.push(e);
    }
    let texte = |m: &Map<String, Value>, k: &str| m.get(k).map(valeur_en_texte).unwrap_or_default();
    liste.sort_by_cached_key(|m| (ordre.get(texte(m, "famille").as_str()).copied().unwrap_or(ordre.len()), texte(m, "libelle").to_lowercase()));
    let _ = etats::s("");
    Ok(json!({"metiers": liste, "familles": familles}))
}

// ---------------------------------------------------------------- les metiers

const QUOI_METIERS: &str = "Les metiers que cette compagnie a coches ou decoches dans Configuration > Metiers : `codes`, pris a une autre convention collective ; `retires`, decoches dans la sienne. Preference d'affichage : aucun calcul ne change. Le catalogue est livre avec le logiciel (donnees/emplois-base.json).";

fn ecrire_preferences(ctx: &Contexte, ajoutes: &[String], retires: &[String]) -> R<()> {
    let p = ctx.racine.join("config").join("metiers-ajoutes.json");
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    if p.exists() {
        sauver_avant(&p)?;
    }
    ecrire(&p, json!({"_quoi": QUOI_METIERS, "codes": ajoutes, "retires": retires}))
}

fn emplois_rendus(ctx: &Contexte) -> R<Value> {
    Ok(json!({"emplois": crate::routes::ouverture::metiers(ctx)?, "emplois_autres": crate::routes::ouverture::metiers_autres_conventions(ctx)?}))
}

fn metier_retenu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let code = texte_requis(charge, "code")?;
    let retenu = charge.get("retenu").is_some_and(vrai);
    let catalogue = emplois::catalogue().map_err(plain)?;
    let Some(m) = catalogue.iter().find(|m| m.get("code").and_then(Value::as_str) == Some(code)) else {
        return Err(refus(format!("Metier hors catalogue : {}.", code)));
    };
    // Ceux de sa convention le sont d'office ; sans IDCC, rien ne se cache.
    let idcc = ctx.entreprise.get("idcc").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let conventions = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
    let de_la_convention = conventions.is_empty() || idcc.is_empty() || conventions.iter().any(|c| c.as_str() == Some(idcc.as_str()));
    let (mut ajoutes, mut retires) = crate::routes::ouverture::preferences_metiers(&ctx.racine)?;
    ajoutes.retain(|c| c != code);
    retires.retain(|c| c != code);
    if retenu && !de_la_convention {
        ajoutes.push(code.to_string());
    }
    if !retenu && de_la_convention {
        retires.push(code.to_string());
    }
    ecrire_preferences(ctx, &ajoutes, &retires)?;
    emplois_rendus(ctx)
}

