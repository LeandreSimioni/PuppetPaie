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
//! Ecart VOULU (refuser plutot que formater autrement) : un modele de compte
//! individuel (`net.par_salarie`) qui emploie une forme de `str.format` que Python
//! formate mais que ce module ne rend pas -- groupement des milliers avec zeros
//! (`{code:010,}`), types flottants (`{code:e}`, `f`, `g`, `%`), methodes ou
//! attributs de `str`/`int` autres que `real`, `imag`, `numerator`, `denominator` --
//! est refuse en `ErreurOperation` qui le dit. Ce que Python refuse (TypeError,
//! AttributeError, ValueError...) l'est ici sous la meme classe.
//!
//! Ecarts assumes : le texte d'un `JSONDecodeError` est celui de serde ; un
//! `mois` de `historique/paies.json` qui n'est pas un texte (fichier abime a la
//! main) ne fait refuser que par un `TypeError` de la meme classe, pas toujours de
//! la meme phrase.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::{d, repr_texte};
use crate::etats::{getd, objet_vide, ou, texte_vide, Dico};
use socle::valeur::valeur_ecrite;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::echec::Echec;
use crate::routes_listes::lire_json;

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

/// Les tables fixes de `operations_exports` et `operations_ecritures`.
pub(crate) fn tables() -> &'static Value {
    static T: OnceLock<Value> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(include_str!("exports_tables.json")).expect("exports_tables.json"))
}

/// Les logiciels du comptable que `operations_ecritures.LOGICIELS` connait.
pub(crate) fn logiciels_connus() -> Vec<&'static str> {
    tables()["logiciels"].as_array().expect("logiciels").iter().filter_map(|l| l["logiciel"].as_str()).collect()
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

/// `operations_exercice.plan_comptable` : le plan livre, recouvert cle par cle par
/// `config/plan-comptable.json`.
pub(crate) fn plan_comptable(ctx: &Contexte) -> R<Map<String, Value>> {
    let base = lire_json(&ctx.code.join("plan-comptable-base.json"))?;
    let Value::Object(mut plan) = base else { unreachable!("plan-comptable-base.json est un objet") };
    let propre = ctx.racine.join("config").join("plan-comptable.json");
    if propre.exists() {
        let perso = lire_json(&propre)?;
        let Value::Object(perso) = perso else {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&perso))));
        };
        for (k, v) in &perso {
            if k.starts_with('_') {
                continue;
            }
            if k == "lignes" || k == "tiers" || k == "retenues" {
                let cible = match plan.entry(k.clone()).or_insert_with(|| Value::Object(Map::new())) {
                    Value::Object(o) => o,
                    _ => unreachable!("lignes, tiers et retenues sont des objets"),
                };
                mettre_a_jour(cible, v)?;
            } else {
                plan.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(plan)
}

/// `dict.update(v)`.
fn mettre_a_jour(cible: &mut Map<String, Value>, v: &Value) -> R<()> {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                cible.insert(k.clone(), x.clone());
            }
            Ok(())
        }
        Value::Array(a) => {
            for (i, el) in a.iter().enumerate() {
                match el {
                    Value::Array(paire) if paire.len() == 2 => {
                        cible.insert(valeur_en_texte(&paire[0]), paire[1].clone());
                    }
                    Value::Array(paire) => {
                        return Err(erreur(
                            "ValueError",
                            socle::fichiers::paire_attendue(i, paire.len()),
                        ))
                    }
                    Value::String(t) if t.chars().count() == 2 => {
                        let c: Vec<String> = t.chars().map(|c| c.to_string()).collect();
                        cible.insert(c[0].clone(), Value::String(c[1].clone()));
                    }
                    Value::String(t) => {
                        return Err(erreur(
                            "ValueError",
                            socle::fichiers::paire_attendue(i, t.chars().count()),
                        ))
                    }
                    Value::Object(o) if o.len() == 2 => {
                        let c: Vec<&String> = o.keys().collect();
                        cible.insert(c[0].clone(), Value::String(c[1].clone()));
                    }
                    Value::Object(o) => {
                        return Err(erreur(
                            "ValueError",
                            socle::fichiers::paire_attendue(i, o.len()),
                        ))
                    }
                    _ => return Err(erreur("TypeError", socle::fichiers::pas_une_paire(i, nom_type(el)))),
                }
            }
            Ok(())
        }
        Value::String(t) => match t.chars().next() {
            None => Ok(()),
            Some(_) => Err(erreur("ValueError", socle::fichiers::paire_attendue(0, 1))),
        },
        autre => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    }
}

/// `plan[k]["compte"]`.
fn compte_de(plan: &Map<String, Value>, k: &str) -> R<Value> {
    let section = plan.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))?;
    match section {
        Value::Object(o) => o.get("compte").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("compte"))),
        autre => Err(erreur("TypeError", format!("{} : {}", repr(k), socle::fichiers::attendu("un objet", nom_type(autre))))),
    }
}

/// `operations_ecritures.controler_plan` : ce qui, dans le plan comptable, ne
/// passerait pas dans ce format -- avant l'export.
fn controler_plan(ctx: &Contexte, format: &str) -> R<Vec<String>> {
    let plan = plan_comptable(ctx)?;
    let Some(lim) = tables()["limites"].get(format) else { return Ok(Vec::new()) };
    let (lim_compte, lim_journal) = (lim["compte"].as_u64().expect("compte") as usize, lim["journal"].as_u64().expect("journal") as usize);
    let nom_format = lim["nom"].as_str().expect("nom");
    let mut comptes: HashSet<String> = HashSet::new();
    for k in ["brut", "net", "pas"] {
        comptes.insert(valeur_en_texte(&compte_de(&plan, k)?));
    }
    if let Some(lignes) = plan.get("lignes") {
        let vide = Map::new();
        let lignes = lignes.as_object().unwrap_or(&vide);
        for regle in lignes.values() {
            for k in ["charge", "tiers"] {
                let v = getd(regle, k, &Value::Null)?;
                if vrai(v) {
                    comptes.insert(valeur_en_texte(&regle[k]));
                }
            }
        }
    }
    if let Some(Value::Object(tiers)) = plan.get("tiers") {
        comptes.extend(tiers.keys().cloned());
    }
    if let Some(Value::Object(retenues)) = plan.get("retenues") {
        for regle in retenues.values() {
            let v = getd(regle, "compte", &Value::Null)?;
            if vrai(v) {
                comptes.insert(valeur_en_texte(v));
            }
        }
    }
    let mut msgs: Vec<String> = Vec::new();
    let mut fiches: Vec<(&String, &Value)> = ctx.salaries.iter().collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    for (cle, s) in fiches {
        if cle.starts_with('_') || !s.is_object() {
            continue;
        }
        match compte_du_salarie(&plan, s, cle) {
            Ok(c) if !c.is_empty() => {
                comptes.insert(c);
            }
            Ok(_) => {}
            Err(Refus::Operation(m)) => msgs.push(m),
            Err(Refus::Autre(e)) => return Err(e),
        }
    }
    let journal = valeur_en_texte(plan.get("journal").unwrap_or(&json!("OD")));
    let mut tries: Vec<&String> = comptes.iter().collect();
    tries.sort();
    for c in tries {
        if c.chars().count() > lim_compte {
            msgs.push(format!("{} : le compte « {} » a {} caractères, la zone en prend {}.", nom_format, c, c.chars().count(), lim_compte));
        } else if format == "quadra" && !(!c.is_empty() && c.chars().all(|x| x.is_ascii_digit())) {
            msgs.push(format!(
                "Quadra : le compte « {} » n'est pas numérique ; Quadra range les comptes de lettres parmi les tiers (clients, fournisseurs).",
                c
            ));
        }
    }
    if journal.chars().count() > lim_journal {
        msgs.push(format!("{} : le journal « {} » a {} caractères, la zone en prend {}.", nom_format, journal, journal.chars().count(), lim_journal));
    }
    if format == "pennylane" && !(!journal.is_empty() && journal.chars().all(char::is_alphabetic)) {
        msgs.push(format!("Pennylane : le code journal « {} » ne doit porter que des lettres.", journal));
    }
    Ok(msgs)
}

pub(crate) enum Refus {
    /// Une `ErreurOperation` : `controler_plan` la rend en phrase.
    Operation(String),
    Autre(Echec),
}

/// `operations_exercice.compte_du_salarie` : le sous-compte 421 d'un salarie, ou
/// "" si le plan n'en veut pas.
pub(crate) fn compte_du_salarie(plan: &Map<String, Value>, salarie: &Value, cle: &str) -> Result<String, Refus> {
    let net = ou(plan.get("net").unwrap_or(&Value::Null), objet_vide());
    let modele = getd(net, "par_salarie", &Value::Null).map_err(Refus::Autre)?;
    if !vrai(modele) {
        return Ok(String::new());
    }
    let code_brut = ou(salarie.get("code_salarie").unwrap_or(&Value::Null), texte_vide());
    let code = entier_py(valeur_en_texte(code_brut).trim());
    let modele_texte = valeur_en_texte(modele);
    formater(&modele_texte, code, cle, 0).map_err(|(classe, message)| {
        // `compte_du_salarie` ne rattrape que ValueError, KeyError et IndexError ;
        // le reste (TypeError, AttributeError...) traverse comme en Python.
        match classe {
            "ValueError" | "KeyError" | "IndexError" => {}
            "NonPorte" => {
                return Refus::Autre(erreur(
                    "ErreurOperation",
                    format!(
                        "Le modèle du compte individuel « {} » (net.par_salarie) emploie une forme de str.format que `paie route` ne prend pas en charge ({}) : refusé plutôt que formaté autrement que Python.",
                        valeur_en_texte(modele),
                        message
                    ),
                ))
            }
            autre => return Refus::Autre(erreur(autre, message)),
        }
        Refus::Operation(format!(
            "Le modèle du compte individuel « {} » (net.par_salarie du plan comptable) ne se remplit pas pour {} : {}. Modèles possibles : « 421{{code:03}} », « 421{{cle}} ».",
            valeur_en_texte(modele),
            cle,
            message
        ))
    })
}

/// `int(texte)` : le signe, les chiffres, un tiret bas entre deux chiffres.
fn entier_py(t: &str) -> Option<i64> {
    let (neg, reste) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if reste.is_empty() || reste.starts_with('_') || reste.ends_with('_') || reste.contains("__") {
        return None;
    }
    let chiffres: String = reste.chars().filter(|c| *c != '_').collect();
    if !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    chiffres.parse::<i64>().ok().map(|n| if neg { -n } else { n })
}

// ------------------------------------------------------------- str.format

/// `NonPorte` : une forme que Python formate et que ce module refuse.
type Faute = (&'static str, String);

/// `modele.format(code=code, cle=cle)` : `code` est l'entier de la fiche, ou "" s'il
/// n'en a pas. Refuse ou Python leve, sous la classe de Python (`ValueError`, `KeyError`,
/// `IndexError`...) et une phrase en francais, en suivant `MarkupIterator_next` et
/// `parse_field` de CPython.
fn formater(modele: &str, code: Option<i64>, cle: &str, profondeur: u32) -> Result<String, Faute> {
    if profondeur >= 2 {
        return Err(("ValueError", "mise en forme imbriquée trop profondément".into()));
    }
    let c: Vec<char> = modele.chars().collect();
    let mut sortie = String::new();
    let mut i = 0;
    let mut auto: Option<bool> = None; // Some(true) : numerotation automatique, Some(false) : manuelle
    let mut prochain = 0usize;
    while i < c.len() {
        let x = c[i];
        i += 1;
        match x {
            '{' | '}' => {
                let au_bout = i >= c.len();
                if x == '}' && (au_bout || c[i] != '}') {
                    return Err(("ValueError", "« } » seule".into()));
                }
                if au_bout && x == '{' {
                    return Err(("ValueError", "« { » seule".into()));
                }
                if c[i] == x {
                    sortie.push(x);
                    i += 1;
                    continue;
                }
                let (nom, spec, conversion, a_developper) = lire_champ(&c, &mut i)?;
                sortie.push_str(&remplir(&nom, &spec, conversion, a_developper, code, cle, profondeur, &mut auto, &mut prochain)?);
            }
            x => sortie.push(x),
        }
    }
    Ok(sortie)
}

/// `parse_field` : le nom, la specification, la conversion, et si la specification
/// porte des champs a developper. `i` est apres l'accolade ouvrante.
fn lire_champ(c: &[char], i: &mut usize) -> Result<(String, String, Option<char>, bool), Faute> {
    let debut = *i;
    let mut dernier = '\0';
    while *i < c.len() {
        dernier = c[*i];
        *i += 1;
        match dernier {
            '{' => return Err(("ValueError", "« { » inattendue dans un nom de champ".into())),
            '[' => {
                while *i < c.len() && c[*i] != ']' {
                    *i += 1;
                }
                continue;
            }
            '}' | ':' | '!' => break,
            _ => continue,
        }
    }
    let nom: String = c[debut..(*i).saturating_sub(1).max(debut)].iter().collect();
    if dernier == '!' || dernier == ':' {
        let mut conversion = None;
        if dernier == '!' {
            if *i >= c.len() {
                return Err(("ValueError", "conversion inachevée après « ! »".into()));
            }
            conversion = Some(c[*i]);
            *i += 1;
            if *i < c.len() {
                let x = c[*i];
                *i += 1;
                if x == '}' {
                    return Ok((nom, String::new(), conversion, false));
                }
                if x != ':' {
                    return Err(("ValueError", "« : » attendu après la conversion".into()));
                }
            }
        }
        let debut_spec = *i;
        let (mut niveau, mut a_developper) = (1, false);
        while *i < c.len() {
            let x = c[*i];
            *i += 1;
            match x {
                '{' => {
                    a_developper = true;
                    niveau += 1;
                }
                '}' => {
                    niveau -= 1;
                    if niveau == 0 {
                        return Ok((nom, c[debut_spec..*i - 1].iter().collect(), conversion, a_developper));
                    }
                }
                _ => {}
            }
        }
        return Err(("ValueError", "« { » sans « } » dans la mise en forme".into()));
    }
    if dernier != '}' {
        return Err(("ValueError", "« } » attendue avant la fin".into()));
    }
    Ok((nom, String::new(), None, false))
}

enum Objet {
    Entier(i64),
    Texte(String),
}

/// Les attributs que l'entier et le texte portent en Python et que ce modele ne sait
/// pas rendre (une methode se montre par son adresse en memoire).
const METHODES_ENTIER: [&str; 9] = ["as_integer_ratio", "bit_count", "bit_length", "conjugate", "from_bytes", "to_bytes", "is_integer", "__class__", "__doc__"];
const METHODES_TEXTE: [&str; 47] = [
    "capitalize", "casefold", "center", "count", "encode", "endswith", "expandtabs", "find", "format", "format_map", "index", "isalnum", "isalpha", "isascii",
    "isdecimal", "isdigit", "isidentifier", "islower", "isnumeric", "isprintable", "isspace", "istitle", "isupper", "join", "ljust", "lower", "lstrip",
    "maketrans", "partition", "removeprefix", "removesuffix", "replace", "rfind", "rindex", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines",
    "startswith", "strip", "swapcase", "title", "translate", "upper", "zfill",
];

/// `get_field_object` : le champ `nom` (`code`, `cle`, un numero, avec ses `.attribut`
/// et ses `[indice]`).
fn resoudre(nom: &str, code: Option<i64>, cle: &str, auto: &mut Option<bool>, prochain: &mut usize) -> Result<Objet, Faute> {
    let c: Vec<char> = nom.chars().collect();
    let fin_premier = c.iter().position(|x| *x == '.' || *x == '[').unwrap_or(c.len());
    let premier: String = c[..fin_premier].iter().collect();
    let mut objet = if premier.is_empty() || premier.chars().all(|x| x.is_ascii_digit()) {
        let n = if premier.is_empty() {
            if *auto == Some(false) {
                return Err(("ValueError", "champs numérotés et non numérotés mêlés".into()));
            }
            *auto = Some(true);
            *prochain += 1;
            *prochain - 1
        } else {
            if *auto == Some(true) {
                return Err(("ValueError", "champs numérotés et non numérotés mêlés".into()));
            }
            *auto = Some(false);
            premier.parse::<usize>().unwrap_or(usize::MAX)
        };
        return Err(("IndexError", format!("champ {{{}}} sans valeur (seuls {{code}} et {{cle}} en ont une)", n)));
    } else if premier == "code" {
        match code {
            Some(n) => Objet::Entier(n),
            None => Objet::Texte(String::new()),
        }
    } else if premier == "cle" {
        Objet::Texte(cle.to_string())
    } else {
        return Err(("KeyError", format!("champ {} inconnu (seuls code et cle en ont un)", repr(&premier))));
    };
    let mut i = fin_premier;
    while i < c.len() {
        if c[i] == '.' {
            i += 1;
            let debut = i;
            while i < c.len() && c[i] != '.' && c[i] != '[' {
                i += 1;
            }
            let attribut: String = c[debut..i].iter().collect();
            if attribut.is_empty() {
                return Err(("ValueError", "attribut vide".into()));
            }
            objet = match (&objet, attribut.as_str()) {
                (Objet::Entier(n), "real") | (Objet::Entier(n), "numerator") => Objet::Entier(*n),
                (Objet::Entier(_), "imag") => Objet::Entier(0),
                (Objet::Entier(_), "denominator") => Objet::Entier(1),
                (Objet::Entier(_), a) if METHODES_ENTIER.contains(&a) => return Err(("NonPorte", format!("l'attribut « {} » d'un entier", a))),
                (Objet::Texte(_), a) if METHODES_TEXTE.contains(&a) || a.starts_with("__") => {
                    return Err(("NonPorte", format!("l'attribut « {} » d'un texte", a)))
                }
                (Objet::Entier(_), a) => return Err(("AttributeError", format!("un nombre entier n'a pas d'attribut « {} »", a))),
                (Objet::Texte(_), a) => return Err(("AttributeError", format!("un texte n'a pas d'attribut « {} »", a))),
            };
        } else {
            // c[i] == '[' : `[indice]`
            i += 1;
            let debut = i;
            while i < c.len() && c[i] != ']' {
                i += 1;
            }
            if i >= c.len() {
                return Err(("ValueError", "« ] » manquant".into()));
            }
            let indice: String = c[debut..i].iter().collect();
            i += 1;
            if indice.is_empty() {
                return Err(("ValueError", "attribut vide".into()));
            }
            if i < c.len() && c[i] != '.' && c[i] != '[' {
                return Err(("ValueError", "seuls « . » ou « [ » suivent « ] »".into()));
            }
            objet = match &objet {
                Objet::Entier(_) => return Err(("TypeError", "un nombre entier ne s'indexe pas".into())),
                Objet::Texte(t) => {
                    if !indice.chars().all(|x| x.is_ascii_digit()) {
                        return Err(("TypeError", "un texte ne s'indexe que par un nombre".into()));
                    }
                    match indice.parse::<usize>().ok().and_then(|n| t.chars().nth(n)) {
                        Some(x) => Objet::Texte(x.to_string()),
                        None => return Err(("IndexError", format!("indice {} hors du texte", indice))),
                    }
                }
            };
        }
    }
    Ok(objet)
}

/// `output_markup` : le champ resolu, converti, mis en forme.
#[allow(clippy::too_many_arguments)]
fn remplir(
    nom: &str,
    spec: &str,
    conversion: Option<char>,
    a_developper: bool,
    code: Option<i64>,
    cle: &str,
    profondeur: u32,
    auto: &mut Option<bool>,
    prochain: &mut usize,
) -> Result<String, Faute> {
    let objet = resoudre(nom, code, cle, auto, prochain)?;
    let objet = match conversion {
        None => objet,
        Some('s') => Objet::Texte(match objet {
            Objet::Entier(n) => n.to_string(),
            Objet::Texte(t) => t,
        }),
        Some('r') | Some('a') => Objet::Texte(match objet {
            Objet::Entier(n) => n.to_string(),
            Objet::Texte(t) => {
                let r = repr(&t);
                if conversion == Some('a') {
                    r.chars()
                        .map(|x| match x as u32 {
                            0..=0x7f => x.to_string(),
                            0x80..=0xff => format!("\\x{:02x}", x as u32),
                            0x100..=0xffff => format!("\\u{:04x}", x as u32),
                            n => format!("\\U{:08x}", n),
                        })
                        .collect()
                } else {
                    r
                }
            }
        }),
        Some(x) => return Err(("ValueError", format!("conversion « !{} » inconnue", x))),
    };
    let spec = if a_developper { formater(spec, code, cle, profondeur + 1)? } else { spec.to_string() };
    mettre_en_forme(&objet, &spec)
}

struct Specification {
    remplissage: char,
    alignement: Option<char>,
    signe: Option<char>,
    diese: bool,
    zero: bool,
    largeur: Option<usize>,
    groupe: Option<char>,
    precision: Option<usize>,
    genre: Option<char>,
}

fn lire_specification(spec: &str, type_py: &str) -> Result<Specification, Faute> {
    let c: Vec<char> = spec.chars().collect();
    let invalide = || ("ValueError", format!("mise en forme « {} » invalide pour {}", spec, socle::fichiers::nature(type_py)));
    let est_align = |x: char| matches!(x, '<' | '>' | '=' | '^');
    let mut s = Specification { remplissage: ' ', alignement: None, signe: None, diese: false, zero: false, largeur: None, groupe: None, precision: None, genre: None };
    let mut i = 0;
    if c.len() >= 2 && est_align(c[1]) {
        s.remplissage = c[0];
        s.alignement = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && est_align(c[0]) {
        s.alignement = Some(c[0]);
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        s.signe = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        s.diese = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        s.zero = true;
        i += 1;
    }
    let debut = i;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i > debut {
        s.largeur = Some(c[debut..i].iter().collect::<String>().parse().map_err(|_| ("ValueError", "nombre trop grand dans la mise en forme".to_string()))?);
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        s.groupe = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let debut = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == debut {
            return Err(("ValueError", "précision manquante après « . »".into()));
        }
        s.precision = Some(c[debut..i].iter().collect::<String>().parse().map_err(|_| ("ValueError", "nombre trop grand dans la mise en forme".to_string()))?);
    }
    if i < c.len() {
        s.genre = Some(c[i]);
        i += 1;
    }
    if i < c.len() {
        return Err(invalide());
    }
    Ok(s)
}

/// `format(objet, spec)`.
fn mettre_en_forme(objet: &Objet, spec: &str) -> Result<String, Faute> {
    match objet {
        Objet::Texte(t) => {
            if spec.is_empty() {
                return Ok(t.clone());
            }
            let s = lire_specification(spec, "str")?;
            if let Some(g) = s.genre.filter(|g| *g != 's') {
                return Err(("ValueError", format!("type « {} » inconnu pour un texte", g)));
            }
            if s.signe.is_some() {
                return Err(("ValueError", "pas de signe pour un texte".into()));
            }
            if s.diese {
                return Err(("ValueError", "pas de forme « # » pour un texte".into()));
            }
            if s.alignement == Some('=') {
                return Err(("ValueError", "pas d'alignement « = » pour un texte".into()));
            }
            if let Some(g) = s.groupe {
                return Err(("ValueError", format!("pas de « {} » avec « s »", g)));
            }
            let mut texte: String = t.clone();
            if let Some(p) = s.precision {
                texte = texte.chars().take(p).collect();
            }
            let remplissage = if s.zero && s.alignement.is_none() { '0' } else { s.remplissage };
            Ok(remplir_largeur(&texte, s.largeur, remplissage, s.alignement.unwrap_or('<')))
        }
        Objet::Entier(n) => {
            if spec.is_empty() {
                return Ok(n.to_string());
            }
            let s = lire_specification(spec, "int")?;
            let genre = s.genre.unwrap_or('d');
            if matches!(genre, 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%') {
                return Err(("NonPorte", format!("le type « {} » appliqué à un entier", genre)));
            }
            if !matches!(genre, 'd' | 'n' | 'x' | 'X' | 'o' | 'b' | 'c') {
                return Err(("ValueError", format!("type « {} » inconnu pour un nombre entier", genre)));
            }
            if s.precision.is_some() {
                return Err(("ValueError", "pas de précision pour un nombre entier".into()));
            }
            if s.groupe.is_some() && (s.zero || s.alignement == Some('=')) {
                return Err(("NonPorte", "le groupement des milliers avec remplissage de zeros".into()));
            }
            let negatif = *n < 0;
            let valeur = n.unsigned_abs();
            let mut corps = match genre {
                'x' => format!("{:x}", valeur),
                'X' => format!("{:X}", valeur),
                'o' => format!("{:o}", valeur),
                'b' => format!("{:b}", valeur),
                'c' => return Ok(char::from_u32(*n as u32).map(|x| x.to_string()).unwrap_or_default()),
                _ => valeur.to_string(),
            };
            if let (Some(g), 'd') = (s.groupe, genre) {
                let chiffres: Vec<char> = corps.chars().collect();
                let mut groupes = String::new();
                for (k, ch) in chiffres.iter().enumerate() {
                    if k > 0 && (chiffres.len() - k) % 3 == 0 {
                        groupes.push(g);
                    }
                    groupes.push(*ch);
                }
                corps = groupes;
            }
            let prefixe = if s.diese {
                match genre {
                    'x' => "0x",
                    'X' => "0X",
                    'o' => "0o",
                    'b' => "0b",
                    _ => "",
                }
            } else {
                ""
            };
            let signe = if negatif {
                "-"
            } else {
                match s.signe {
                    Some('+') => "+",
                    Some(' ') => " ",
                    _ => "",
                }
            };
            let entete = format!("{}{}", signe, prefixe);
            let alignement = s.alignement.unwrap_or(if s.zero { '=' } else { '>' });
            let remplissage = if s.zero && s.alignement.is_none() { '0' } else { s.remplissage };
            if alignement == '=' {
                let largeur = s.largeur.unwrap_or(0);
                let total = entete.chars().count() + corps.chars().count();
                let manque = largeur.saturating_sub(total);
                return Ok(format!("{}{}{}", entete, remplissage.to_string().repeat(manque), corps));
            }
            Ok(remplir_largeur(&format!("{}{}", entete, corps), s.largeur, remplissage, alignement))
        }
    }
}

fn remplir_largeur(t: &str, largeur: Option<usize>, remplissage: char, alignement: char) -> String {
    let n = t.chars().count();
    let manque = largeur.unwrap_or(0).saturating_sub(n);
    let bourre = |k: usize| remplissage.to_string().repeat(k);
    match alignement {
        '>' => format!("{}{}", bourre(manque), t),
        '^' => format!("{}{}{}", bourre(manque / 2), t, bourre(manque - manque / 2)),
        _ => format!("{}{}", t, bourre(manque)),
    }
}
