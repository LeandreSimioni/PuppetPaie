//! Les routes des exports et des dossiers (`paie route`) : `arret-contrats` et
//! `dossier-formes`.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et ce qu'il appelait --
//! `operations_arrets.contrats_au`, `operations_dossier.formes_dossier` (et, par
//! lui, `operations_exports.periodes_exportables`, `operations_ecritures.
//! etat_logiciel`) --, porte a l'identique : meme JSON (l'enveloppe de `web.py`
//! comprise), memes refus sous la meme classe.
//!
//! LECTURE SEULE, AUCUN CALCUL DE PAIE, AUCUN RESEAU, AUCUN SOUS-PROCESSUS.
//!
//! Restent en Python : `exports-periodes` (il dit `xlsx` et `pdf` : `import
//! openpyxl` reussit-il, la fabrique est-elle installee ? deux faits de
//! l'interprete et du poste, pas du disque), `dossier-salarie` (il compose des
//! PDF dans un dossier temporaire par la fabrique, meme sans rien ecrire dans le
//! dossier de la compagnie).
//!
//! `formes_dossier` ne garde de `periodes_exportables` que les mois et les
//! annees, mais Python calcule aussi `etat_logiciel` (le logiciel du comptable
//! retenu, le controle du plan comptable) : un `compta.json` ou un
//! `plan-comptable.json` abime fait donc refuser `dossier-formes`. Le refus est
//! porte (`verifier_compta`), pas la sortie, que la route jetait.
//!
//! Ecarts assumes : le texte d'un `JSONDecodeError` est celui de serde ; un
//! `mois` de `historique/paies.json` qui n'est pas un texte (fichier abime a la
//! main) ne fait refuser que par un `TypeError` de la meme classe, pas toujours de
//! la meme phrase.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::{d, repr_texte};
use envois::etats::{getd, ou, texte_vide, Dico};
use documents::plan_comptable::{controler_plan, tables};
use socle::valeur::valeur_ecrite;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use socle::fichiers::lire_json_echec as lire_json;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["arret-contrats", "dossier-formes", "exports-periodes"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "arret-contrats" => arret_contrats(ctx, charge),
        "dossier-formes" => dossier_formes(ctx),
        "exports-periodes" => exports_periodes(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}
/// `charge["cle"]`.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

// -------------------------------------------------------------- arret-contrats

/// `Contexte.salarie(nom)` : le refus est un `KeyError`. Rust rend le texte
/// du refus tel quel pour une phrase (`Salarie ...`), et deja entre guillemets
/// pour une cle manquante d'une fiche ; `str(KeyError(phrase))` est la phrase
/// entre guillemets. Un `nom` qui n'est pas un texte se lit `str(nom)` et se
/// montre `repr(nom)` dans la phrase.
pub(crate) fn salarie(ctx: &Contexte, nom: &Value) -> R<Value> {
    let texte = valeur_en_texte(nom);
    ctx.salarie(&texte).map_err(|m| {
        let m = if m.starts_with("Salarie ") {
            let m = match nom {
                Value::String(_) => m,
                autre => m.replacen(&repr_texte(&texte), &valeur_ecrite(autre), 1),
            };
            m
        } else {
            m
        };
        erreur("KeyError", m)
    })
}

/// `sortie_json.brut` d'une valeur lue d'un fichier : un dictionnaire perd ses
/// cles qui commencent par `_`.
pub(crate) fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

/// `{"contrats": brut(operations_arrets.contrats_au(ctx, salarie, jour))}` : les
/// contrats d'un salarie qui couvrent un jour, lus dans les saisies.
fn arret_contrats(ctx: &Contexte, charge: &Value) -> R<Value> {
    let sal = exiger(charge, "salarie")?;
    let jour = exiger(charge, "jour")?;
    let cle = match salarie(ctx, sal)?.get("_cle") {
        Some(c) => c.clone(),
        None => return Err(erreur("KeyError", socle::fichiers::absent("_cle"))),
    };
    let jour = d(jour).map_err(|m| erreur("ValueError", m))?.iso();
    Ok(json!({"contrats": contrats_au(ctx, &cle, &jour)?}))
}

/// `operations_arrets.contrats_au` : les contrats de la fiche `cle` qui couvrent `jour` (AAAA-MM-JJ),
/// lus dans les saisies, du plus ancien debut au plus recent. Chacun : salarie, nom, numero, debut,
/// fin, mois, spectacle.
pub(crate) fn contrats_au(ctx: &Contexte, cle: &Value, jour: &str) -> R<Vec<Value>> {
    let cle_texte = valeur_en_texte(cle);
    let mut sorties: Vec<(String, Value)> = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    let mut mois = crate::routes_contrats::mois_saisis(ctx)?;
    mois.reverse();
    for m in mois {
        for (c, e) in crate::routes_contrats::engagements_du_mois(ctx, &m)? {
            if c != cle_texte {
                continue;
            }
            let (debut, fin) = crate::routes_contrats::bornes_brutes(&e)?;
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Le mois le plus recent qui le saisit fait foi (une fin ecrite plus
            // tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map(|n| n.mensuelle()).unwrap_or(false);
            let numero = e.get("numero").cloned().unwrap_or(Value::Null);
            if debut.is_empty() || (fin.is_empty() && !mensuel) {
                continue;
            }
            if vus.contient(&[json!(c), json!(debut), numero.clone()])? {
                continue;
            }
            vus.poser(vec![json!(c), json!(debut), numero.clone()], ())?;
            if !(debut.as_str() <= jour && (fin.is_empty() || jour <= fin.as_str())) {
                continue;
            }
            let s = salarie(ctx, cle)?;
            let champ = |k: &str| -> R<Value> { s.get(k).cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k))) };
            let prenom = match s.get("prenom_affichage").filter(|x| vrai(x)) {
                Some(p) => p.clone(),
                None => champ("prenom")?,
            };
            let nom = format!("{} {}", valeur_en_texte(&champ("nom")?), valeur_en_texte(&prenom));
            let ou_vide = |k: &str| e.get(k).filter(|x| vrai(x)).cloned().unwrap_or_else(|| json!(""));
            let mut o = Map::new();
            o.insert("salarie".into(), cle.clone());
            o.insert("nom".into(), json!(nom));
            o.insert("numero".into(), brut(&ou_vide("numero")));
            o.insert("debut".into(), json!(debut));
            o.insert("fin".into(), json!(fin));
            o.insert("mois".into(), json!(m));
            o.insert("spectacle".into(), brut(&ou_vide("spectacle")));
            // Paye au mois (`Nature::mensuelle`) : son mois de paie se tire du moteur, et un arret y appelle le maintien de
            // salaire (L1226-1 : pas pour un intermittent).
            o.insert("mensuel".into(), json!(mensuel));
            sorties.push((debut, Value::Object(o)));
        }
    }
    sorties.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sorties.into_iter().map(|(_, v)| v).collect())
}

// -------------------------------------------------------------- dossier-formes

/// Les mois qu'il y a matiere a exporter : ceux que des engagements touchent, du plus ancien au plus recent, chacun
/// marque `saisie` (ses paies se lisent dans ses engagements valides).
pub(crate) fn mois_disponibles(ctx: &Contexte) -> R<Vec<(String, &'static str)>> {
    let mut mois: Vec<(String, &'static str)> = crate::routes_contrats::mois_saisis(ctx)?.into_iter().map(|m| (m, "saisie")).collect();
    mois.sort();
    mois.dedup();
    Ok(mois)
}

/// `sorted({m[:4] for m in mois}, reverse=True)`.
fn annees_de(dispo: &[(String, &'static str)]) -> Vec<String> {
    let mut annees: Vec<String> = dispo.iter().map(|(m, _)| m.chars().take(4).collect::<String>()).collect::<HashSet<_>>().into_iter().collect();
    annees.sort();
    annees.reverse();
    annees
}

/// Les salaries que la page propose.
fn salaries_de_la_page(ctx: &Contexte) -> Vec<Value> {
    let mut fiches: Vec<(&String, &Value)> = ctx.salaries.iter().filter(|(k, v)| !k.starts_with('_') && v.is_object()).collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    fiches
        .into_iter()
        .map(|(k, v)| {
            let nom = format!(
                "{} {}",
                valeur_en_texte(v.get("nom").unwrap_or(texte_vide())),
                valeur_en_texte(ou(v.get("prenom_affichage").unwrap_or(&Value::Null), v.get("prenom").unwrap_or(texte_vide())))
            );
            json!({"cle": k, "nom": nom, "actif": v.get("actif").map_or(true, vrai), "email": v.get("email").is_some_and(vrai)})
        })
        .collect()
}

/// `operations_dossier.formes_dossier` : ce que la page propose -- les salaries,
/// les categories a cocher, les periodes, et si l'envoi est possible.
fn dossier_formes(ctx: &Contexte) -> R<Value> {
    let salaries = salaries_de_la_page(ctx);
    // `periodes_exportables` : les mois, puis (sans rien en dire) le logiciel du comptable.
    let dispo = mois_disponibles(ctx)?;
    let mois: Vec<&String> = dispo.iter().map(|(m, _)| m).collect();
    let annees = annees_de(&dispo);
    etat_logiciel(ctx)?;
    let boite = crate::routes_accueil::boite_liee(ctx)?;
    Ok(json!({
        "salaries": salaries,
        "categories": [
            {"categorie": "bulletins", "quoi": "Bulletins de paie"},
            {"categorie": "contrats", "quoi": "Contrats"},
            {"categorie": "contrats-signes", "quoi": "Contrats signés"},
            {"categorie": "certificats-cs", "quoi": "Certificats Congés Spectacles"},
            {"categorie": "attestations", "quoi": "Attestations employeur (AER, AEM)"},
            {"categorie": "documents-fin", "quoi": "Documents de fin de contrat"},
            {"categorie": "recapitulatif", "quoi": "Fiche récapitulative"},
        ],
        "mois": mois,
        "annees": annees,
        "boite_liee": boite,
        "dossier": ctx.racine.join("exports").join("dossiers").display().to_string(),
    }))
}

// ------------------------------------------------- le logiciel du comptable

/// `operations_ecritures.etat_logiciel` : ce que la page montre -- les logiciels,
/// celui retenu (`config/compta.json`), et ce que le plan comptable ne tiendrait
/// pas dans son format.
pub(crate) fn etat_logiciel(ctx: &Contexte) -> R<Value> {
    let chemin = ctx.racine.join("config").join("compta.json");
    let choix = if chemin.exists() { lire_json(&chemin)? } else { json!({}) };
    let retenu = getd(&choix, "logiciel", &Value::Null)?;
    let retenu = ou(retenu, texte_vide());
    let analytique = vrai(getd(&choix, "analytique", &Value::Null)?);
    let controle = match retenu {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(nom) => {
            let logiciels = tables()["logiciels"].as_array().expect("logiciels");
            match logiciels.iter().find(|l| l["logiciel"].as_str() == Some(nom)) {
                Some(l) => controler_plan(ctx, l["format"].as_str().expect("format"))?,
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    Ok(json!({"logiciels": tables()["logiciels"], "logiciel": retenu, "analytique": analytique, "controle": controle}))
}

/// `operations_exports._etat_concerne` : la page ne propose pas la CASC-SVP de la
/// 3090 a une compagnie de la 1285, ni la dette de conges a qui n'emploie pas de
/// permanents.
fn etat_concerne(ctx: &Contexte, etat: &str) -> bool {
    let idcc = valeur_en_texte(ou(ctx.entreprise.get("idcc").unwrap_or(&Value::Null), texte_vide()));
    let t = tables();
    if let Some(liste) = t["si_convention"].get(etat) {
        if !idcc.is_empty() && !liste.as_array().expect("liste").iter().any(|c| c.as_str() == Some(idcc.as_str())) {
            return false;
        }
    }
    if t["si_permanents"].as_array().expect("permanents").iter().any(|e| e.as_str() == Some(etat)) {
        return ctx.emploie_des_permanents();
    }
    true
}

/// `operations_exports.periodes_exportables` : les mois et les annees qu'il y a
/// matiere a exporter, les etats, les formats, et le logiciel du comptable. `xlsx`
/// (`import openpyxl` reussit-il ?) et `pdf` (la fabrique est-elle installee ?)
/// sont deux faits de l'interprete et du poste, que `web.py` dit dans la charge.
fn exports_periodes(ctx: &Contexte, charge: &Value) -> R<Value> {
    let dispo = mois_disponibles(ctx)?;
    let t = tables();
    let etats: Vec<&Value> = t["etats"].as_array().expect("etats").iter().filter(|e| etat_concerne(ctx, e["etat"].as_str().expect("etat"))).collect();
    let annees = annees_de(&dispo);
    let compta = etat_logiciel(ctx)?;
    let fait = |k: &str| charge.get(k).is_some_and(vrai);
    Ok(json!({
        "mois": dispo.iter().map(|(m, _)| m).collect::<Vec<_>>(),
        "annees": annees,
        "etats": etats,
        "formats": t["formats"],
        "regroupements": t["regroupements"],
        "dossier": ctx.racine.join("exports").display().to_string(),
        "xlsx": fait("xlsx"),
        "pdf": fait("pdf"),
        "compta": compta,
    }))
}
