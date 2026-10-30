//! Les routes des fiches : l'identite de la compagnie (`identite`, `poser-identite`, `retirer-identite`), les fiches des
//! salaries, des spectacles et des permanents (`permanents`, `verifier-fiche`, `activer-fiche`, `enregistrer-fiche`,
//! `changements-declares`), leur demande par courriel (`fiche-demander`, `fiches-relever`, `fiche-oublier-demande`),
//! les metiers (`metiers-catalogue`, `metier-retenu`, `metiers-ajoutes`), le taux AT/MP (`taux-atmp`), le regime de la
//! taxe sur les salaires, et le modele du bulletin (`bulletin-modeles`, `bulletin-modele-apercu`,
//! `bulletin-modele-choisir`).
//!
//! Les refus portent leur classe (`KeyError`, `TypeError`, `ErreurOperation`...), que la page releve.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::documents::{client_courriel, preuve_de, vrai_ou};
use crate::routes::mois::{lire_saisie, sauver_avant};
use crate::routes::ouverture::{metier_visible, preferences_metiers};
use declarations::champs;
use documents::bulletin::{jeux_et_description, Modele, CASE_MODELE};
use documents::fiche_questionnaire as fq;
use droit::contexte::Contexte;
use droit::emplois;
use droit::jumeaux::{numero_pose, Ident};
use envois::courriel::Courriel;
use envois::courriel_lecture as lecture;
use envois::envoi::{self, Message};
use envois::etats::{self};
use envois::journal::{self, Note};
use envois::ligne_d_envoi::{echec_d_envoi, noter};
use envois::simulation_courriels::BoiteQuiRepond;
use socle::date::Jour;
use socle::echec::{erreur, Echec, R};
use socle::ecriture::ecrire_json;
use socle::fichiers::{dossier_du_code, json_en_cache, lire_json, repr, vrai};
use socle::html::{html_echapper, mailto};
use socle::texte::{capitaliser, strip};
use socle::valeur::{nom_type, valeur_ecrite, valeur_en_texte, valeurs_egales};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "identite", "permanents", "taxe-sur-les-salaires-regime", "verifier-fiche", "changements-declares", "taux-atmp",
    "metiers-catalogue", "bulletin-modeles", "bulletin-modele-apercu", "bulletin-modele-choisir",
    "activer-fiche", "enregistrer-fiche", "fiche-oublier-demande", "poser-identite", "retirer-identite", "metier-retenu",
    "metiers-ajoutes", "fiche-demander", "fiches-relever", "poser-courriel",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &[
    "activer-fiche", "enregistrer-fiche", "fiche-oublier-demande", "poser-identite", "retirer-identite", "metier-retenu",
    "metiers-ajoutes", "fiche-demander", "fiches-relever", "poser-courriel",
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let texte = |cle: &str| charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    match nom {
        "identite" => identite(ctx),
        "permanents" => permanents(ctx, charge),
        "taxe-sur-les-salaires-regime" => taxe_sur_les_salaires_regime(ctx, charge),
        "verifier-fiche" => verifier_fiche(charge),
        "changements-declares" => changements_declares(ctx, charge),
        "taux-atmp" => crate::routes::atmp::route(ctx, charge),
        "metiers-catalogue" => metiers_catalogue(ctx),
        "bulletin-modeles" => modeles(ctx),
        "bulletin-modele-apercu" => {
            let modele = modele_demande(charge)?;
            let pdf = apercu_pdf(modele).map_err(refus)?;
            Ok(json!({"modele": modele.cle(), "pdf": socle::base64::ecrire(&pdf)}))
        }
        "bulletin-modele-choisir" => choisir(ctx, charge),
        "activer-fiche" => activer_fiche_route(o, ctx, charge),
        "enregistrer-fiche" => enregistrer_fiche(o, ctx, charge),
        "poser-courriel" => poser_courriel(ctx, charge),
        "fiche-oublier-demande" => oublier_demande(o, ctx, charge),
        "poser-identite" => poser_identite(ctx, charge),
        "retirer-identite" => retirer_identite(ctx, charge),
        "metier-retenu" => metier_retenu(ctx, charge),
        "metiers-ajoutes" => metiers_ajoutes(ctx, charge),
        "fiche-demander" => {
            let d = demander_fiche(ctx, &texte("adresse"), &texte("prenom"), &preuve_de(charge), "interface", None)?;
            Ok(json!({"code": d.code, "adresse": d.adresse}))
        }
        "fiches-relever" => {
            let r = relever_fiches(ctx, None, "interface")?;
            Ok(json!({"attendues": r.attendues, "recues": r.recues, "incidents": r.incidents}))
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ permanents

/// `permanents` : `{"entreprise": {...}}`, la fiche Entreprise en cours de saisie -- sans elle, celle de la compagnie.
/// Rend le curseur (`contexte::emploie_des_permanents`) : la page montre ou replie ce qui ne vaut que pour les
/// permanents selon cette reponse, et ne le decide pas elle-meme quand on coche la case.
fn permanents(ctx: &Contexte, charge: &Value) -> R<Value> {
    let oui = match charge.get("entreprise") {
        Some(e @ Value::Object(_)) => droit::contexte::emploie_des_permanents(e),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
        None => ctx.emploie_des_permanents(),
    };
    Ok(json!({ "permanents": oui }))
}

// ------------------------------------------------------- taxe-sur-les-salaires-regime

/// `taxe-sur-les-salaires-regime` : `{"taxe_sur_les_salaires": {...}}`, ce que la fiche en cours de saisie dit -- sans
/// lui, celle de la compagnie. Rend la conclusion du moteur (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`, CGI art.
/// 231, 1) : les cases a montrer, la consequence en clair, et si la reponse est complete. La page n'en decide rien.
fn taxe_sur_les_salaires_regime(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = documents::taxe_sur_les_salaires::CHAMP_TAXE_SUR_LES_SALAIRES;
    let ent = match charge.get(cle) {
        Some(v @ (Value::Object(_) | Value::Null)) => json!({ cle: v }),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
        None => ctx.entreprise.clone(),
    };
    // LES PLAFONDS DE LA FRANCHISE, CHIFFRES : ceux du bareme de l'annee des salaires -- `annee` dans la
    // charge, sinon l'annee en cours. Un bareme absent ne fait pas refuser : la question se pose sans montants.
    let annee = match charge.get("annee") {
        None | Some(Value::Null) => ctx.aujourdhui.an,
        Some(v) => match v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(a) if (1900..=2999).contains(&a) => a as i32,
            _ => return Err(erreur("ValueError", socle::date::annee_impossible(valeur_ecrite(v)))),
        },
    };
    let bar = socle::date::Jour::nouveau(annee, 1, 1).and_then(|j| ctx.bareme(&j).ok());
    let q = documents::taxe_sur_les_salaires::question_franchise(bar.as_ref(), annee);
    // La conclusion nomme les deux plafonds.
    let r = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ent, q.plafonds);
    Ok(json!({
        "franchise": {
            "aide": q.aide,
            "choix": { "true": q.sous, "false": q.dessus },
            "plafonds": q.plafonds.map(|(s, t)| json!({
                "prestations_de_services": s.normalize().to_string(),
                "total": t.normalize().to_string(),
            })),
            "annee": annee,
        },
        "champs": r.champs,
        "consequence": r.consequence,
        "complet": r.decision.is_ok(),
        "due": matches!(r.decision, Ok(Some(_))),
        "part_taxee": match r.decision { Ok(Some(p)) => json!(p.normalize().to_string()), _ => Value::Null },
    }))
}

// ------------------------------------------------------------------ outils

/// `charge["cle"]`.
pub(crate) fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    match charge {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
    }
}

/// `champs.FICHES[nom]` : le nom d'une fiche connue, `KeyError` sinon,
/// `TypeError` quand ce qui est donne ne se hache pas.
pub(crate) fn nom_de_fiche(v: &Value) -> R<&str> {
    match v {
        Value::String(s) if matches!(s.as_str(), "entreprise" | "salarie" | "spectacle") => Ok(s),
        Value::Array(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        autre => Err(erreur("KeyError", format!("fiche inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ taux-atmp

/// Un entier, de ce qu'un JSON porte.
pub(crate) fn entier(v: &Value) -> R<i64> {
    let trop_grand = || Echec::from("annee hors de la plage d'un entier de 64 bits".to_string());
    match v {
        Value::Bool(b) => Ok(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => {
                let x = n.as_f64().unwrap_or(0.0).trunc();
                if x.abs() < 9.0e18 {
                    Ok(x as i64)
                } else {
                    Err(trop_grand())
                }
            }
        },
        Value::String(t) => {
            let illisible = || erreur("ValueError", format!("nombre entier illisible : {}", repr(t)));
            let u = t.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c));
            let (neg, chiffres) = match u.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, u.strip_prefix('+').unwrap_or(u)),
            };
            let c: Vec<char> = chiffres.chars().collect();
            let bien_forme = !c.is_empty()
                && crate::routes::fiches::chiffre_lu(c[0]).is_some()
                && crate::routes::fiches::chiffre_lu(c[c.len() - 1]).is_some()
                && c.iter().all(|x| crate::routes::fiches::chiffre_lu(*x).is_some() || *x == '_')
                && !chiffres.contains("__");
            if !bien_forme {
                return Err(illisible());
            }
            let ascii: String = c.iter().filter(|x| **x != '_').filter_map(|x| crate::routes::fiches::chiffre_lu(*x)).map(|d| char::from(b'0' + d as u8)).collect();
            let n: i64 = ascii.parse().map_err(|_| trop_grand())?;
            Ok(if neg { -n } else { n })
        }
        autre => Err(erreur(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}

/// `p.read_text` puis `json.loads`, comme `_lire_json` sous un `except ValueError` :
/// `None` quand le fichier est absent, illisible comme texte ou comme JSON ;
/// un refus du disque se leve.
pub(crate) fn lire_tolerant(p: &Path) -> R<Option<Value>> {
    if !p.exists() {
        return Ok(None);
    }
    let octets = std::fs::read(p).map_err(|e| {
        let classe = match e.kind() {
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        };
        erreur(classe, format!("{} : {}", p.display(), e))
    })?;
    Ok(String::from_utf8(octets).ok().and_then(|t| serde_json::from_str(&t).ok()))
}

// ------------------------------------------------------------ verifier-fiche

/// Un objet, de ce qu'un JSON porte : un objet se copie ; une liste
/// de paires aussi ; le reste se refuse sous `TypeError`.
pub(crate) fn copie_en_objet(v: &Value) -> R<Map<String, Value>> {
    let paires: Vec<Value> = match v {
        Value::Object(o) => return Ok(o.clone()),
        Value::Array(a) => a.clone(),
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        autre => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    };
    let mut sortie = Map::new();
    for (i, p) in paires.iter().enumerate() {
        let couple: Vec<Value> = match p {
            Value::Array(a) => a.clone(),
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            Value::Object(o) => o.keys().map(|k| Value::String(k.clone())).collect(),
            _ => return Err(erreur("TypeError", socle::fichiers::pas_une_paire(i, nom_type(p)))),
        };
        if couple.len() != 2 {
            return Err(erreur("ValueError", socle::fichiers::paire_attendue(i, couple.len())));
        }
        match &couple[0] {
            Value::String(k) => {
                sortie.insert(k.clone(), couple[1].clone());
            }
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            _ => {}
        }
    }
    Ok(sortie)
}

/// `str(v or "")`.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// `faute_de_norme(champ, actuel)` pour une valeur deja la, vraie : un texte
/// passe par le controle de la norme ; ce qui n'en est pas un se refuse sous les
/// memes `TypeError` que la table des formats.
fn fautif(c: &champs::Champ, actuel: &Value) -> R<bool> {
    if let Value::String(s) = actuel {
        return Ok(champs::faute_de_norme(c, s)?.is_some());
    }
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(false) };
    let format = declarations::formats::format_de(rubrique)?;
    if format.as_object().map_or(true, |o| o.is_empty()) {
        return Ok(false);
    }
    let motifs = format.get("csl").and_then(Value::as_array).map_or(false, |l| !l.is_empty());
    let pas_un_texte = || erreur("TypeError", socle::fichiers::attendu("un texte", nom_type(actuel)));
    let mut faute = false;
    if let Some(lon) = format.get("longueur").and_then(Value::as_array).filter(|l| !l.is_empty()) {
        let n = match actuel {
            Value::Array(a) => a.len() as i64,
            Value::Object(o) => o.len() as i64,
            autre => return Err(erreur("TypeError", socle::fichiers::attendu("un texte ou une liste", nom_type(autre)))),
        };
        let (mini, maxi) = (lon[0].as_i64().unwrap_or(0), lon[1].as_i64().unwrap_or(0));
        faute = !(mini <= n && n <= maxi);
    }
    if !faute && motifs {
        return Err(pas_un_texte());
    }
    if faute && motifs {
        return Err(pas_un_texte());
    }
    Ok(faute)
}

/// Sans accents, en capitales, espaces resserrees.
fn capitales(v: &Value) -> String {
    let t: String = champs::sans_accents(&valeur_en_texte(v)).chars().flat_map(char::to_uppercase).collect();
    t.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Les formes declarees du signataire et du contact DSN ;
/// rend `{cle: [avant, apres]}` pour les structures modifiees.
fn deriver_personnes(donnees: &mut Map<String, Value>, forcer: bool) -> Map<String, Value> {
    let mut faits = Map::new();
    // LE CONTACT DSN ET CELUI DE LA MEDECINE DU TRAVAIL INTERNE : la norme les ecrit en « nom et prenom » en une seule
    // rubrique (S20.G00.07.001, `norme-dsn/texte/bloc-S20.G00.07.txt` l. 63-71) ; la fiche les saisit en une case
    // (`nom_prenom_affichage`), dont la forme declaree est la meme en capitales sans accents.
    let personnes: [(&str, &[&str], Option<(&str, &[&str])>); 3] = [
        ("signataire", &["nom", "prenom"], None),
        ("declarant_dsn", &[], Some(("nom_prenom", &["nom_prenom"]))),
        ("contact_sante_travail", &[], Some(("nom_prenom", &["nom_prenom"]))),
    ];
    for (cle, officiels, compose) in personnes {
        let Some(Value::Object(p)) = donnees.get(cle) else { continue };
        let mut neuf = p.clone();
        let faux = |m: &Map<String, Value>, k: &str| m.get(k).map_or(true, |v| !vrai(v));
        for k in officiels {
            let lisible = neuf.get(&format!("{}_affichage", k)).cloned().unwrap_or(Value::Null);
            if vrai(&lisible) && (forcer || faux(&neuf, k)) {
                neuf.insert(k.to_string(), Value::String(capitales(&lisible)));
            }
        }
        if let Some((cible, morceaux)) = compose {
            let lisibles: Vec<Value> = morceaux.iter().map(|m| neuf.get(&format!("{}_affichage", m)).cloned().unwrap_or(Value::Null)).collect();
            if lisibles.iter().any(vrai) && (forcer || faux(&neuf, cible)) {
                let mots: Vec<String> = lisibles.iter().filter(|x| vrai(x)).map(valeur_en_texte).collect();
                neuf.insert(cible.to_string(), Value::String(capitales(&Value::String(mots.join(" ")))));
            }
        }
        if !valeurs_egales(&Value::Object(neuf.clone()), &Value::Object(p.clone())) {
            faits.insert(cle.to_string(), json!([Value::Object(p.clone()), Value::Object(neuf.clone())]));
            donnees.insert(cle.to_string(), Value::Object(neuf));
        }
    }
    faits
}

/// LE SIREN ET LE NIC SE DEDUISENT DU SIRET (regle 2 : la page ne les pose pas) : le SIRET
/// est le SIREN (9 chiffres) suivi du NIC (5 chiffres) -- c'est sa definition (INSEE), aucune valeur ne se devine. Un
/// SIRET qui n'a pas 14 chiffres ne deduit rien : sa case le dit (`champs::verifier_fiche`). Rend `{cle: [avant, apres]}`.
fn deriver_siren_nic(donnees: &mut Map<String, Value>) -> Map<String, Value> {
    let mut faits = Map::new();
    let siret = texte_ou_vide(donnees.get("siret")).trim().to_string();
    if siret.chars().count() != 14 || !siret.chars().all(|c| c.is_ascii_digit()) {
        return faits;
    }
    for (cle, valeur) in [("siren", &siret[..9]), ("nic", &siret[9..])] {
        let attendu = Value::String(valeur.to_string());
        let actuel = donnees.get(cle).cloned();
        if actuel.as_ref() != Some(&attendu) {
            faits.insert(cle.into(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
            donnees.insert(cle.into(), attendu);
        }
    }
    faits
}

/// Fabrique les formes declarees depuis les formes lisibles.
/// Sans `forcer`, on ne remplit que ce qui est vide ou indeclarable. Rend
/// `{cle: [avant, apres]}` : ce que le programme a fabrique tout seul.
pub(crate) fn deriver(nom: &str, donnees: &mut Map<String, Value>, forcer: bool) -> R<Map<String, Value>> {
    let mut faits = Map::new();
    for c in champs::fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        let Some(Value::String(texte)) = donnees.get(source).cloned() else { continue };
        if texte.is_empty() {
            continue;
        }
        let attendu = champs::normaliser(&c, &Value::String(texte))?;
        let actuel = donnees.get(&c.cle).cloned();
        if actuel.as_ref() == Some(&attendu) {
            continue;
        }
        if let Some(a) = actuel.as_ref().filter(|a| vrai(a)) {
            if !forcer && !fautif(&c, a)? {
                continue;
            }
        }
        faits.insert(c.cle.clone(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
        donnees.insert(c.cle.clone(), attendu);
    }
    if nom == "entreprise" {
        // Le numero d'adherent Conges Spectacles s'ecrit dans sa forme (`conges_spectacles::forme_adherent`), avant d'etre
        // controle et enregistre : « 0123 450001-f » est la meme valeur que « 0123450001F ».
        let cle = documents::conges_spectacles::CLE_ADHERENT;
        if let Some(Value::String(brut)) = donnees.get(cle).cloned() {
            let forme = documents::conges_spectacles::forme_adherent(&brut);
            if forme != brut {
                faits.insert(cle.into(), json!([brut, forme.clone()]));
                donnees.insert(cle.into(), Value::String(forme));
            }
        }
        faits.extend(deriver_personnes(donnees, forcer));
        faits.extend(deriver_siren_nic(donnees));
    }
    // LE COMPTE BANCAIRE S'ECRIT COMPACT : l'IBAN et le BIC sans blancs, en majuscules -- « FR76 3000 6000 … » est le
    // meme compte que « FR763000 6000… » (`socle::iban::iban_compact`, communique IBAN de la Banque centrale de Turquie,
    // annexe 3 : `reference/sources/iban/`).
    if let Some(Value::Object(b)) = donnees.get_mut("banque") {
        for k in ["iban", "bic"] {
            if let Some(Value::String(brut)) = b.get(k).cloned() {
                let forme = socle::iban::iban_compact(&brut);
                if forme != brut {
                    b.insert(k.into(), Value::String(forme));
                }
            }
        }
    }
    // LE DEPARTEMENT DE NAISSANCE SE LIT DANS LE NIR (regle 2 : la page le montre, le moteur l'ecrit) : ses 6e et 7e
    // caracteres sont le departement de naissance, 99 pour une personne nee a l'etranger (S21.G00.30.001 et .014,
    // `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Il ne s'ecrit que dans une case vide : une
    // valeur saisie qui le contredit se dit (`champs::controles_croises`), elle ne se remplace pas.
    if nom == "salarie" && socle::texte::strip(&texte_ou_vide(donnees.get("departement_naissance"))).is_empty() {
        if let Some(dep) = champs::departement_du_nir(&texte_ou_vide(donnees.get("nir"))) {
            faits.insert("departement_naissance".into(), json!([donnees.get("departement_naissance").cloned().unwrap_or(Value::Null), dep]));
            donnees.insert("departement_naissance".into(), Value::String(dep));
        }
    }
    if nom == "salarie" && socle::texte::strip(&texte_ou_vide(donnees.get("initiales_contrat"))).is_empty() {
        let lettres: String = texte_ou_vide(donnees.get("nom")).chars().filter(|c| c.is_ascii_uppercase()).collect();
        if !lettres.is_empty() {
            let deux: String = lettres.chars().take(2).collect();
            faits.insert("initiales_contrat".into(), json!(["", deux.clone()]));
            donnees.insert("initiales_contrat".into(), Value::String(deux));
        }
    }
    // LA NATIONALITE DSN SE DEDUIT DU PAYS : avec un pays de nationalite de la liste, la case 30.013 ne se
    // choisit plus, le moteur l'ecrit (`pays::codification`). Sans pays, ou avec un pays inconnu (qui refuse sur sa
    // case), la case reste ce qu'elle est.
    if nom == "salarie" {
        if let Some(Value::String(code)) = donnees.get("pays_nationalite").filter(|v| vrai(v)).cloned() {
            if let Some(p) = socle::pays::du_code(&code).map_err(Echec::from)? {
                let attendu = Value::String(socle::pays::codification(&p).to_string());
                let actuel = donnees.get("code_nationalite_dsn").cloned();
                if actuel.as_ref() != Some(&attendu) {
                    faits.insert("code_nationalite_dsn".into(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
                    donnees.insert("code_nationalite_dsn".into(), attendu);
                }
            }
        }
    }
    Ok(faits)
}

/// L'organisme du territoire, ou "".
fn urssaf_du_departement(dep: &str) -> R<String> {
    let table = json_en_cache(&dossier_du_code().join("urssaf-dpae-base.json")).map_err(|m| Echec::from(m))?;
    let d = socle::texte::strip(dep).to_uppercase();
    let par = table.get("par_departement");
    let lire = |k: &str| par.and_then(|p| p.get(k)).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    Ok(lire(&d).or_else(|| if d == "2A" || d == "2B" { lire("20") } else { None }).unwrap_or_default())
}

/// `urssaf.pour_code_postal`.
pub(crate) fn urssaf_du_code_postal(cp: Option<&Value>) -> R<String> {
    let v: Vec<char> = texte_ou_vide(cp).chars().filter(|c| champs::est_chiffre(*c)).collect();
    if v.len() < 2 {
        return Ok(String::new());
    }
    let debut: String = v.iter().collect::<String>();
    let n = if debut.starts_with("97") || debut.starts_with("98") { 3 } else { 2 };
    urssaf_du_departement(&v.iter().take(n).collect::<String>())
}

/// Ce que le moteur PROPOSERAIT pour une case encore vide.
fn suggestions(nom: &str, donnees: &Map<String, Value>) -> R<Value> {
    let mut propose = Map::new();
    if nom == "salarie" {
        if let Some(dep) = champs::departement_du_nir(&texte_ou_vide(donnees.get("nir"))) {
            let mut trouvees = vec![("departement_naissance", dep.clone())];
            if dep != "99" {
                trouvees.push(("pays_naissance", "FR".to_string()));
            }
            for (cle, valeur) in trouvees {
                if socle::texte::strip(&texte_ou_vide(donnees.get(cle))).is_empty() {
                    propose.insert(cle.into(), json!({"valeur": valeur, "pourquoi": "lu dans le numéro de sécurité sociale"}));
                }
            }
        }
    } else if nom == "entreprise" && socle::texte::strip(&texte_ou_vide(donnees.get("code_urssaf"))).is_empty() {
        let code = urssaf_du_code_postal(donnees.get("code_postal"))?;
        if !code.is_empty() {
            propose.insert(
                "code_urssaf".into(),
                json!({
                    "valeur": code,
                    "pourquoi": "l'organisme du département, d'après votre code postal — à corriger si le vôtre diffère (versement en lieu unique, pôle grandes entreprises, GUSO)",
                }),
            );
        }
    }
    Ok(Value::Object(propose))
}

/// Les anomalies de la fiche APRES derivation, sur une copie.
pub(crate) fn verifier(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut copie = donnees.clone();
    deriver(nom, &mut copie, false)?;
    champs::verifier_fiche(nom, &copie)
}

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme `ecrire_fiche` la
/// derive avant d'ecrire (`deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu
/// de l'import les montre telles : avant, il verifiait la fiche avant derivation, et chaque champ que le bulletin ou
/// la DSN imprime en minuscules (« 3 quai de la Fosse ») criait « doit etre en MAJUSCULES » sous sa forme lisible,
/// alors que l'ecriture le declare bien en majuscules.
///
/// Le matricule d'un salarie neuf que la piece ne donne pas, l'ecriture le pose (le suivant, `ecrire_fiche`) : son
/// absence n'est pas une anomalie a montrer. Chaque anomalie porte le libelle de sa case (`libelle`), celui que la
/// fiche montre : la page le dit a l'utilisateur, pas la cle.
pub(crate) fn verifier_comme_a_l_ecriture(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut copie = donnees.clone();
    if nom == "salarie" && copie.get("code_salarie").is_none_or(|v| !vrai(v)) {
        copie.insert("code_salarie".into(), json!("1"));
    }
    deriver(nom, &mut copie, true)?;
    let mut anomalies = champs::verifier_fiche(nom, &copie)?;
    avec_libelles(nom, &mut anomalies)?;
    Ok(anomalies)
}

/// Pose sur chaque anomalie le libelle de sa case (`libelle`), celui que la fiche montre.
pub(crate) fn avec_libelles(nom: &str, anomalies: &mut [Value]) -> R<()> {
    let champs = champs::fiche(nom)?;
    for a in anomalies.iter_mut() {
        let cle = a.get("champ").and_then(Value::as_str).unwrap_or("").to_string();
        // Le code PCS-ESE ne se tape pas : la fiche le remplit du metier choisi (`emplois.rs`).
        if cle == "pcs_ese" {
            a["libelle"] = json!("Métier (il donne le code PCS-ESE)");
        } else if let Some(c) = champs.iter().find(|c| c.cle == cle) {
            a["libelle"] = json!(c.libelle);
        }
    }
    Ok(())
}

/// `verifier-fiche` : les anomalies de la fiche telle qu'elle SERAIT derivee, ce qui
/// serait declare, et ce que le moteur proposerait pour les cases vides.
fn verifier_fiche(charge: &Value) -> R<Value> {
    let nom_brut = exiger(charge, "nom")?;
    let donnees = exiger(charge, "donnees")?;
    let mut copie = copie_en_objet(donnees)?;
    let nom = nom_de_fiche(nom_brut)?;
    deriver(nom, &mut copie, false)?;
    let anomalies = champs::verifier_fiche(nom, &copie)?;
    let objet = donnees.as_object();
    let declare = match objet {
        Some(o) => champs::apercu_derive(nom, o)?,
        None if champs::fiche(nom)?.iter().any(champs::Champ::calcule) => {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(donnees))));
        }
        None => json!({}),
    };
    let propose = match objet {
        Some(o) => suggestions(nom, o)?,
        None if nom == "spectacle" => json!({}),
        None => return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(donnees)))),
    };
    Ok(json!({"anomalies": anomalies, "declare": declare, "suggestions": propose}))
}

// ------------------------------------------------------ changements-declares

/// Ou l'import copie les pieces qu'il reprend de l'ancien logiciel, les DSN deposees comprises (`historique/bulletins/`).
pub(crate) fn pieces_importees(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// Les fichiers (et dossiers) dont le nom finit par
/// `.dsn`, sans suivre les liens vers des dossiers.
pub(crate) fn fichiers_dsn(dossier: &Path, sortie: &mut Vec<std::path::PathBuf>) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    for e in entrees.flatten() {
        let chemin = e.path();
        if e.file_name().to_string_lossy().ends_with(".dsn") {
            sortie.push(chemin.clone());
        }
        if e.file_type().map_or(false, |t| t.is_dir()) {
            fichiers_dsn(&chemin, sortie);
        }
    }
}

/// `usages_declares` : combien de DSN deja deposees portent cette valeur (telle
/// qu'une DSN l'ecrit, `,'VALEUR'`), et sur quelle periode.
fn usages_declares(ctx: &Contexte, valeur: &Value) -> Value {
    let jeton = format!(",'{}'", valeur_en_texte(valeur));
    let prefixe = "S20.G00.05.005,";
    let (mut fichiers, mut mois) = (0, Vec::<String>::new());
    for dossier in [pieces_importees(ctx), ctx.racine.join("sorties")] {
        if !dossier.exists() {
            continue;
        }
        let mut trouves = Vec::new();
        fichiers_dsn(&dossier, &mut trouves);
        for f in trouves {
            let Ok(octets) = std::fs::read(&f) else { continue };
            // latin-1, puis les fins de ligne universelles de `read_text`.
            let brut: String = octets.iter().map(|b| *b as char).collect();
            let texte = brut.replace("\r\n", "\n").replace('\r', "\n");
            if !texte.contains(&jeton) {
                continue;
            }
            fichiers += 1;
            for l in socle::texte::lignes_du_texte(&texte) {
                if let Some(reste) = l.strip_prefix(prefixe) {
                    let mp: Vec<char> = reste.trim_matches('\'').chars().collect();
                    if mp.len() == 8 {
                        mois.push(format!("{}-{}", mp[4..8].iter().collect::<String>(), mp[2..4].iter().collect::<String>()));
                    }
                    break;
                }
            }
        }
    }
    json!({"declarations": fichiers, "du": mois.iter().min(), "au": mois.iter().max()})
}

/// `garder_notes_internes` : `neuve`, augmentee des cles internes (`_...`) de
/// `ancienne`. Les listes se recollent par rang, a longueur egale seulement.
pub(crate) fn garder_notes_internes(ancienne: &Value, neuve: &Value) -> Value {
    match (ancienne, neuve) {
        (Value::Object(a), Value::Object(n)) => {
            let mut sortie = n.clone();
            for (k, v) in a {
                if k.starts_with('_') && !sortie.contains_key(k) {
                    sortie.insert(k.clone(), v.clone());
                } else if let Some(dedans) = sortie.get(k).cloned() {
                    sortie.insert(k.clone(), garder_notes_internes(v, &dedans));
                }
            }
            Value::Object(sortie)
        }
        (Value::Array(a), Value::Array(n)) if a.len() == n.len() => {
            Value::Array(a.iter().zip(n).map(|(x, y)| garder_notes_internes(x, y)).collect())
        }
        _ => neuve.clone(),
    }
}

/// `changements-declares` : les valeurs existantes que cet enregistrement ferait
/// changer (chacune demande sa date d'effet), avec ce que la valeur sortante a deja servi a declarer. Regarde, n'ecrit rien.
fn changements_declares(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom_brut = exiger(charge, "nom")?;
    let cle = charge.get("cle").unwrap_or(&Value::Null);
    let donnees = exiger(charge, "donnees")?;
    let nom = match nom_brut {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(s) if matches!(s.as_str(), "entreprise" | "salarie" | "spectacle") => s.as_str(),
        autre => return Err(erreur("ErreurOperation", format!("Fiche inconnue : {}", valeur_ecrite(autre)))),
    };
    let vide = Value::Object(Map::new());
    let ancienne: &Value = if nom == "entreprise" {
        &ctx.entreprise
    } else {
        let magasin = if nom == "salarie" { &ctx.salaries } else { &ctx.spectacles };
        match cle {
            Value::String(k) => magasin.get(k).unwrap_or(&vide),
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            _ => &vide,
        }
    };
    let copie = copie_en_objet(donnees)?;
    Ok(json!({"changements": changements(ctx, nom, ancienne, copie)?}))
}

/// Le coeur de `changements_declares` : la fiche `ancienne` (telle que l'ecran l'a montree) et les `donnees` qui la
/// remplaceraient ; chaque valeur existante qui changerait (`crate::routes::fiches::changements_a_dater`), celles-la memes
/// dont l'enregistrement exigera la date d'effet. Un champ declare dit aussi ce que sa valeur sortante a deja servi a
/// declarer.
pub(crate) fn changements(ctx: &Contexte, nom: &str, ancienne: &Value, copie: Map<String, Value>) -> R<Vec<Value>> {
    let Value::Object(mut donnees) = garder_notes_internes(ancienne, &Value::Object(copie)) else { unreachable!() };
    deriver(nom, &mut donnees, true)?;
    let Value::Object(anc) = ancienne else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(ancienne))));
    };
    let champs_fiche = champs::fiche(nom)?;
    let mut sortie = Vec::new();
    let a_dater = crate::routes::fiches::changements_a_dater(ancienne, &donnees);
    for cle in &a_dater {
        let cle = cle.clone();
        let avant = anc.get(&cle).cloned().unwrap_or(Value::Null);
        let apres = donnees[&cle].clone();
        // La forme lisible d'une personne ne se declare pas, mais elle change : elle se date comme le reste.
        let c = champs_fiche.iter().find(|c| c.cle == cle);
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        if c.filter(|c| c.calcule()).and_then(|c| c.derive_de.as_ref()).is_some_and(|s| a_dater.contains(s)) {
            continue;
        }
        let mut entree = json!({
            "champ": cle, "libelle": c.map_or(cle.as_str(), |c| c.libelle.as_str()), "avant": avant, "apres": apres,
        });
        let o = entree.as_object_mut().unwrap();
        if c.is_some_and(|c| c.declare) {
            o.insert("deja_declare".into(), usages_declares(ctx, &avant));
        }
        if let Some(c) = c.filter(|c| c.calcule()) {
            let source = c.derive_de.as_deref().unwrap_or("");
            let lisible = champs_fiche.iter().find(|x| x.cle == source);
            let vue = champs::combler_affichage(nom, anc)?;
            o.insert("libelle".into(), json!(lisible.map_or(&c.libelle, |l| &l.libelle)));
            o.insert("avant_lisible".into(), vue.get(source).cloned().unwrap_or(Value::Null));
            o.insert("apres_lisible".into(), donnees.get(source).cloned().unwrap_or(Value::Null));
        }
        sortie.push(entree);
    }
    Ok(sortie)
}

/// Le chiffre (0 a 9) que `c` represente : les chiffres ASCII, et
/// ceux des ecritures les plus courantes (arabe, devanagari, pleine chasse).
pub(crate) fn chiffre_lu(c: char) -> Option<u32> {
    const DEBUTS: [u32; 4] = [0x0660, 0x06F0, 0x0966, 0xFF10];
    let u = c as u32;
    if c.is_ascii_digit() {
        return Some(u - 0x30);
    }
    DEBUTS.iter().find(|d| u >= **d && u < **d + 10).map(|d| u - d)
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
        // `_de_la_convention` : sans IDCC (une compagnie qui s'installe), rien ne se cache.
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

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

#[track_caller]
fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

pub(super) fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

fn creer_dossier(d: &Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))
}

/// Un fichier de configuration : son objet, vide s'il n'existe pas.
fn objet_du_fichier(chemin: &Path) -> R<Map<String, Value>> {
    if !chemin.exists() {
        return Ok(Map::new());
    }
    match lire_saisie(chemin)? {
        Value::Object(m) => Ok(m),
        _ => Err(erreur("AttributeError", format!("{} : un objet JSON est attendu", chemin.display()))),
    }
}

fn ecrire(chemin: &Path, v: Value) -> R<()> {
    ecrire_json(chemin, &v, 2).map_err(en_os)
}

/// La configuration, relue apres l'ecriture (`web._configuration(Contexte(racine))`).
pub(crate) fn configuration(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    crate::routes::ouverture::route(o, &neuf, "configuration", &json!({}))
}

/// L'heure de Paris, a la seconde (`datetime.now().isoformat(timespec="seconds")`).
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// Aujourd'hui, a Paris (`date.today()`).
fn aujourd_hui() -> Jour {
    Jour::lire(&maintenant()[..10]).expect("date de Paris")
}

// --------------------------------------------------------------- activer-fiche

fn fichier_de(nom: &str) -> Option<&'static str> {
    match nom {
        "entreprise" => Some("entreprise.json"),
        "salarie" => Some("salaries.json"),
        "spectacle" => Some("spectacles.json"),
        _ => None,
    }
}

/// Seul `actif` change, a partir d.aujourd.hui (archiver, c.est ne plus proposer a
/// partir de ce jour ; ce qui precede reste ce qu.il etait) ; la copie horodatee du fichier est faite avant.
pub(crate) fn activer_fiche(ctx: &Contexte, nom: &str, cle: &str, actif: bool) -> R<()> {
    let fichier = match nom {
        "salarie" | "spectacle" => fichier_de(nom).unwrap(),
        _ => return Err(refus(format!("Seuls un salarie ou un spectacle s'activent : {}.", socle::fichiers::repr(nom)))),
    };
    let chemin = ctx.racine.join("config").join(fichier);
    let mut contenu = objet_du_fichier(&chemin)?;
    match contenu.get_mut(cle) {
        Some(Value::Object(f)) => {
            // Une fiche sans `actif` etait active de tout temps.
            if !f.contains_key("actif") {
                droit::contexte::dater(f, "actif", Value::Bool(true), None, None);
            }
            droit::contexte::dater(f, "actif", Value::Bool(actif), Some(&ctx.aujourdhui), None);
        }
        Some(_) => return Err(erreur("TypeError", format!("la fiche {} n'est pas un objet", cle))),
        None => return Err(refus(format!("Fiche introuvable : {}.", cle))),
    }
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))
}

fn activer_fiche_route(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let (nom, cle) = (texte_requis(charge, "nom")?, texte_requis(charge, "cle")?);
    activer_fiche(ctx, nom, cle, charge.get("actif").is_some_and(vrai))?;
    Ok(json!({"configuration": configuration(o, ctx)?}))
}

// ----------------------------------------------------- fiche-oublier-demande

fn oublier_demande(_o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let code = charge.get("code").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    let lignes = envois::etats::charger(&ctx.racine)?;
    let attendues = crate::routes::ouverture::attendues(&lignes)?;
    if !attendues.iter().any(|a| a.get("code").and_then(Value::as_str) == Some(code.as_str())) {
        return Err(refus(format!("Aucune demande de fiche en attente sous le code {}.", code)));
    }
    let detail = json!({"code": code, "abandon": true});
    let note = journal::Note {
        quoi: "fiche-recue",
        mois: None,
        fichier: None,
        mode: journal::REEL,
        qui: "interface",
        accepte: true,
        destinataire: "",
        flux: "",
        reponse: "demande abandonnee, sans reponse",
        trace: None,
        detail: Some(&detail),
        versions: None,
        quand: "",
        origine: "",
    };
    journal::noter(&ctx.racine, &note, &maintenant()).map_err(plain)?;
    // Ni NIR ni IBAN dans la reponse : la page relit `configuration` (comme apres `fiche-demander`).
    Ok(json!({"code": code}))
}

// ------------------------------------------------------------------ l'identite

const IDENTITES: [&str; 3] = ["signature", "cachet", "logo"];
const OCTETS_MAX: usize = 2 * 1024 * 1024;

#[track_caller]
fn identite_inconnue(quoi: &str) -> Echec {
    refus(format!("Identite inconnue : {}. Connues : cachet, logo, signature.", socle::fichiers::repr(quoi)))
}

fn dossier_identite(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("identite")
}

/// Le fichier pose pour `quoi`, quelle que soit son extension.
fn chemin_identite(dossier: &Path, quoi: &str) -> Option<PathBuf> {
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("{}{}", quoi, e))).find(|f| f.exists())
}

/// Le format de ces octets, reconnu sur l'en-tete et non sur l'extension.
fn format_de(o: &[u8]) -> Option<&'static str> {
    if o.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("png");
    }
    if o.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpeg");
    }
    let tete = &o[..o.len().min(2048)];
    let debut = tete.iter().position(|c| !c.is_ascii_whitespace() && *c != 0x0b).unwrap_or(tete.len());
    let tete = &tete[debut..];
    if tete.starts_with(b"<?xml") || tete.starts_with(b"<svg") || tete.starts_with(b"<!--") {
        let long = o[..o.len().min(8192)].to_ascii_lowercase();
        if long.windows(4).any(|w| w == b"<svg") {
            return Some("svg");
        }
    }
    None
}

fn avant(f: &Path) -> R<()> {
    let mut nom = f.as_os_str().to_owned();
    nom.push(".avant");
    std::fs::rename(f, PathBuf::from(nom)).map_err(|e| en_os(format!("{} : {}", f.display(), e)))
}

fn poser_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    let contenu = match exiger(charge, "contenu")? {
        Value::Null => "",
        Value::String(s) => s.as_str(),
        _ => return Err(erreur("TypeError", "`contenu` : un texte est attendu")),
    };
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    let utile = contenu.rsplit(',').next().unwrap_or("");
    let octets = crate::compagnie::problemes::base64_strict(utile).ok_or_else(|| refus("Le fichier n'est pas lisible : base64 invalide."))?;
    if octets.is_empty() {
        return Err(refus("Le fichier est vide."));
    }
    if octets.len() > OCTETS_MAX {
        return Err(refus(format!("Le fichier fait {:.1} Mo ; la limite est de 2 Mo.", octets.len() as f64 / 1024.0 / 1024.0)));
    }
    let format = format_de(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois). Rien n'a ete ecrit.")
    })?;
    let dossier = dossier_identite(ctx);
    creer_dossier(&dossier)?;
    // Les autres extensions s'en vont : une identite, un fichier.
    if let Some(ancien) = chemin_identite(&dossier, quoi) {
        avant(&ancien)?;
    }
    let extension = match format {
        "png" => ".png",
        "jpeg" => ".jpg",
        _ => ".svg",
    };
    let cible = dossier.join(format!("{}{}", quoi, extension));
    std::fs::write(&cible, &octets).map_err(|e| en_os(format!("{} : {}", cible.display(), e)))?;
    if quoi == "signature" {
        // Pour qui elle est posee : la personne qui signe CE JOUR-LA.
        let registre = dossier.join("identite.json");
        let mut tenu = objet_du_fichier(&registre)?;
        let signataire = crate::routes::ouverture::signataire_de(&ctx.entreprise)?;
        tenu.insert("signature".into(), json!({"de": signataire["nom"], "le": aujourd_hui().iso()}));
        ecrire(&registre, Value::Object(tenu))?;
    }
    identite(ctx)
}

fn retirer_identite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = texte_requis(charge, "quoi")?;
    if !IDENTITES.contains(&quoi) {
        return Err(identite_inconnue(quoi));
    }
    if let Some(f) = chemin_identite(&dossier_identite(ctx), quoi) {
        avant(&f)?;
    }
    identite(ctx)
}

fn identite(ctx: &Contexte) -> R<Value> {
    let mut v = crate::routes::ouverture::identite(ctx, &ctx.gabarits())?;
    // La pastille de chaque image : la sienne, posee, ou celle que le logiciel dessine, faites ; rien de pose, a faire.
    for e in v.as_object_mut().into_iter().flat_map(|o| o.values_mut()) {
        let Some(o) = e.as_object_mut() else { continue };
        let p = if o.get("pose").is_some_and(vrai) {
            super::pastille("fait", "le vôtre")
        } else if o.get("automatique").is_some_and(vrai) {
            super::pastille("fait", "celui du logiciel")
        } else {
            super::pastille("a_faire", "rien de posé")
        };
        o.insert("pastille".into(), p);
    }
    Ok(json!({"identite": v}))
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

fn metiers_ajoutes(ctx: &Contexte, charge: &Value) -> R<Value> {
    let codes: Vec<Value> = match exiger(charge, "codes")? {
        Value::Array(a) => a.clone(),
        _ => return Err(erreur("TypeError", "`codes` : une liste est attendue")),
    };
    let retirer = charge.get("retirer").is_some_and(vrai);
    let connus: HashSet<String> = emplois::catalogue()
        .map_err(plain)?
        .iter()
        .filter_map(|m| m.get("code").and_then(Value::as_str).map(str::to_string))
        .collect();
    let inconnus: Vec<String> = codes.iter().filter(|c| !c.as_str().is_some_and(|s| connus.contains(s))).map(valeur_en_texte).collect();
    if !inconnus.is_empty() {
        return Err(refus(format!("Metier(s) hors catalogue : {}.", inconnus.join(", "))));
    }
    let codes: Vec<String> = codes.iter().filter_map(|c| c.as_str().map(str::to_string)).collect();
    let (avant, retires) = crate::routes::ouverture::preferences_metiers(&ctx.racine)?;
    let apres: Vec<String> = if retirer {
        avant.into_iter().filter(|c| !codes.contains(c)).collect()
    } else {
        let mut v = avant;
        for c in codes {
            if !v.contains(&c) {
                v.push(c);
            }
        }
        v
    };
    ecrire_preferences(ctx, &apres, &retires)?;
    emplois_rendus(ctx)
}

// ------------------------------------------------------------- enregistrer-fiche

/// Le prochain matricule (S21.G00.30.019, que l'employeur choisit : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`) : le
/// plus grand code salarie que le fichier ait jamais porte, plus un. Le fichier est date : chaque fiche porte la suite
/// de ses valeurs (`[{"valeur"}, {"depuis", "valeur"}]`), et chacune compte.
fn matricule_suivant(salaries: &Map<String, Value>) -> i64 {
    let lire = |v: &Value| entier(&Value::String(valeur_en_texte(v).trim().to_string())).ok();
    let codes: Vec<i64> = salaries
        .iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .filter_map(|(_, s)| s.as_object()?.get("code_salarie"))
        .flat_map(|c| match c {
            Value::Array(versions) => versions.iter().filter_map(|x| x.get("valeur").and_then(lire)).collect::<Vec<_>>(),
            autre => lire(autre).into_iter().collect(),
        })
        .collect();
    codes.into_iter().max().map_or(1, |m| m + 1)
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier.
fn cle_neuve(nom: &str, donnees: &Map<String, Value>, existants: &Map<String, Value>) -> R<String> {
    let propre = |texte: &str, bas: bool| -> String {
        let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
        let t: String = t.chars().map(|c| if envois::etats::lettre_ou_chiffre(c) { c } else { '-' }).collect();
        let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
        if bas {
            t.to_lowercase()
        } else {
            t.to_uppercase()
        }
    };
    let texte = |k: &str| donnees.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let ou = |a: &str, b: &str| {
        let x = texte(a);
        if x.is_empty() {
            texte(b)
        } else {
            x
        }
    };
    let candidats: Vec<String> = if nom == "salarie" {
        let base = propre(&ou("nom", "nom_affichage"), false);
        let prenom = propre(&ou("prenom", "prenom_affichage"), false);
        vec![base.clone(), format!("{}-{}", base, prenom)]
    } else {
        let titre = texte("titre");
        let titre = titre.trim();
        let base = if (titre.is_empty() || titre.to_uppercase() == "A COMPLETER") && donnees.get("numero_objet").is_some_and(vrai) {
            format!("objet-{}", valeur_en_texte(&donnees["numero_objet"]).trim())
        } else {
            propre(titre, true)
        };
        vec![base]
    };
    let candidats: Vec<String> = candidats.into_iter().filter(|c| !c.is_empty() && !c.starts_with('-')).collect();
    if candidats.is_empty() {
        return Err(refus(format!(
            "Impossible de nommer cette fiche {} : il lui faut au moins un {}.",
            nom,
            if nom == "salarie" { "nom" } else { "titre" }
        )));
    }
    if let Some(c) = candidats.iter().find(|c| !existants.contains_key(*c)) {
        return Ok(c.clone());
    }
    let dernier = candidats.last().unwrap();
    let mut n = 2;
    while existants.contains_key(&format!("{}-{}", dernier, n)) {
        n += 1;
    }
    Ok(format!("{}-{}", dernier, n))
}

/// Les cases a cocher arrivees en texte redeviennent booleennes.
fn coercer_booleens(nom: &str, donnees: &mut Map<String, Value>) -> R<()> {
    for c in champs::fiche(nom)? {
        if c.forme != "booleen" {
            continue;
        }
        let Some(v) = donnees.get(&c.cle) else { continue };
        let neuf = match v {
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "true" | "vrai" | "oui" | "1" | "on" | "coche" => Some(Value::Bool(true)),
                "false" | "faux" | "non" | "0" | "off" | "" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Null => None,
            autre => Some(Value::Bool(vrai(autre))),
        };
        if let Some(n) = neuf {
            donnees.insert(c.cle.clone(), n);
        }
    }
    Ok(())
}

// ------------------------------------------------ ecrire une valeur datee

/// Une case vide : absente, nulle, un texte vide, ou le titre « A COMPLETER » que l'import pose sur un spectacle dont il
/// ne connait que le numero d'objet (`import::completer::A_COMPLETER`) -- le remplir n'est pas un changement a dater.
fn vide(v: Option<&Value>) -> bool {
    v.map_or(true, |v| v.is_null() || v.as_str() == Some("") || v.as_str().is_some_and(|s| s.trim().eq_ignore_ascii_case(crate::import::completer::A_COMPLETER)))
}

/// La meme valeur, a la forme pres : un nombre saisi en texte (100 et « 100 »), une personne et sa forme lisible (les
/// cles `_affichage` que l'ecran comble, `champs::combler_affichage`, ne se declarent pas).
fn meme(a: &Value, b: &Value) -> bool {
    let sans_affichage = |v: &Value| match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.ends_with("_affichage")).map(|(k, v)| (k.clone(), v.clone())).collect()),
        autre => autre.clone(),
    };
    valeurs_egales(a, b)
        || (a.is_object() && valeurs_egales(&sans_affichage(a), &sans_affichage(b)))
        || (!a.is_object() && !a.is_array() && valeur_en_texte(a) == valeur_en_texte(b))
}

/// Les cases d'un metier : quatre ecritures d'un seul fait, choisies ensemble dans le catalogue.
const METIER: [&str; 4] = ["emploi_affichage", "emploi", "pcs_ese", "code_emploi_ft"];

/// Les champs de `donnees` qui CHANGENT une valeur existante de `ancienne` (la fiche telle que l'ecran l'a montree) :
/// ceux-la demandent leur date d'effet. Remplir un champ vide n'en est pas un : c'est sa premiere valeur. Sauf le NIR qui
/// arrive sur une fiche qui existait : la premiere DSN qui le porte porte aussi le NTT sous lequel le salarie etait
/// declare (`ntt::depuis_quand_le_nir`), et le jour ou il arrive ne se devine pas.
pub(crate) fn changements_a_dater(ancienne: &Value, donnees: &Map<String, Value>) -> Vec<String> {
    let existait = ancienne.as_object().is_some_and(|o| o.keys().any(|k| !k.starts_with('_')));
    let nir_qui_arrive = |k: &str, v: &Value| existait && k == "nir" && vide(ancienne.get(k)) && !vide(Some(v));
    // UN METIER QUI N'ETAIT PAS ENCORE CHOISI SE COMPLETE, IL NE CHANGE PAS : sans code PCS-ESE, l'intitule lu sur un
    // ancien bulletin n'a rien pu declarer (la DSN l'exige, S21.G00.40.004), et choisir le metier n'a pas de date d'effet.
    let metier_a_completer = vide(ancienne.get("pcs_ese"));
    donnees
        .iter()
        .filter(|(k, v)| {
            !k.starts_with('_')
                && !(metier_a_completer && METIER.contains(&k.as_str()))
                && (nir_qui_arrive(k, v) || (!vide(ancienne.get(k.as_str())) && !ancienne.get(k.as_str()).is_some_and(|a| meme(a, v))))
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// La date d'effet que la charge d'une route d'ecriture donne (`depuis`, AAAA-MM-JJ), ou `None`.
pub(crate) fn depuis_de(depuis: &Value) -> R<Option<Jour>> {
    if vrai(depuis) {
        socle::date::d(depuis).map(Some).map_err(|m| erreur("ValueError", m))
    } else {
        Ok(None)
    }
}

/// POSER UN CHAMP DE LA FICHE ENTREPRISE (une route qui ne touche qu'une case : le modele de bulletin, la case des
/// permanents) : comme une fiche de l'ecran (`ecrire_datee`), l'ancien fichier sauvegarde.
pub(crate) fn poser_entreprise(ctx: &Contexte, champ: &str, valeur: Value, depuis: Option<Jour>) -> R<()> {
    let chemin = ctx.racine.join("config").join("entreprise.json");
    let mut contenu = objet_du_fichier(&chemin)?;
    let mut donnees = Map::new();
    donnees.insert(champ.to_string(), valeur);
    ecrire_datee(&mut contenu, "entreprise", &ctx.entreprise, &donnees, depuis, None, &ctx.lue_au)?;
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))
}

/// L'ANCIENNETE D'UN SALARIE PART DE SON EMBAUCHE (S21.G00.86.003, l'anciennete dans l'entreprise :
/// `norme-dsn/texte/bloc-S21.G00.86.txt`) : la validation de son premier engagement la pose sur sa fiche, au premier jour
/// de ce contrat (`debut`), quand la fiche ne la dit pas ; elle se change ensuite dans « Avancé » de la fiche. Une fiche
/// qui la dit la garde. Rend vrai si elle a ete posee. INFERENCE : l'anciennete commence a l'entree dans l'entreprise.
pub(crate) fn poser_anciennete_a_l_embauche(ctx: &Contexte, salarie: &str, debut: &str) -> R<bool> {
    let chemin = ctx.racine.join("config").join("salaries.json");
    let mut contenu = objet_du_fichier(&chemin)?;
    let Some(Value::Object(fiche)) = contenu.get_mut(salarie) else { return Ok(false) };
    let dite = fiche.get("date_anciennete").and_then(Value::as_array).is_some_and(|v| v.iter().any(|x| x.get("valeur").is_some_and(vrai)));
    if dite {
        return Ok(false);
    }
    fiche.insert("date_anciennete".into(), json!([{"valeur": debut}]));
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))?;
    Ok(true)
}

/// L'ANCIENNETE D'UN SALARIE REPRIS QUE SON CONTRAT FAIT REMONTER : l'import la pose au premier bulletin lu
/// (`import::bulletins`) ; un contrat repris qui a commence avant (`debut`, celui que la DSN de l'ancien logiciel
/// declare, S21.G00.40.001) dit qu'il etait dans l'entreprise des ce jour-la (S21.G00.86.003, l'anciennete dans
/// l'entreprise : `norme-dsn/texte/bloc-S21.G00.86.txt` ; inference : un contrat en cours dans l'entreprise y compte).
/// Seule la valeur que l'import a posee, `pose_le`, se remplace, de tout temps : elle n'a jamais ete declaree. Une date
/// saisie depuis reste la sienne. Rend vrai si elle a ete remplacee.
pub(crate) fn reprendre_anciennete(ctx: &Contexte, salarie: &str, pose_le: &str, debut: &str) -> R<bool> {
    let chemin = ctx.racine.join("config").join("salaries.json");
    let mut contenu = objet_du_fichier(&chemin)?;
    let Some(Value::Array(versions)) = contenu.get_mut(salarie).and_then(|f| f.get_mut("date_anciennete")) else { return Ok(false) };
    let [seule] = versions.as_mut_slice() else { return Ok(false) };
    if seule.get("valeur").map(valeur_en_texte).as_deref() != Some(pose_le) || debut >= pose_le {
        return Ok(false);
    }
    seule["valeur"] = json!(debut);
    sauver_avant(&chemin)?;
    ecrire(&chemin, Value::Object(contenu))?;
    Ok(true)
}

/// ECRIRE UNE FICHE DATEE : chaque valeur de `donnees` qui differe de `ancienne` (la fiche lue a `lue_au`, celle que
/// l'ecran a montree) s'ecrit dans la fiche datee `brute` (`contexte::dater`). Ce qui change une valeur existante vaut a
/// partir de `depuis`, et sans lui se refuse : ce qui precede garde l'ancienne valeur. La premiere valeur d'un champ
/// (absent de la fiche) vaut de tout temps ; celle qui remplit un champ vide vaut depuis `depuis`, a defaut depuis la
/// valeur vide qu'elle remplit. Le NIR qui arrive : la fiche disait « sans NIR » de tout temps, le NIR vaut depuis
/// `depuis`. Les notes `_...` s'ecrivent telles quelles.
pub(crate) fn ecrire_datee(
    brute: &mut Map<String, Value>,
    nom: &str,
    ancienne: &Value,
    donnees: &Map<String, Value>,
    depuis: Option<Jour>,
    note: Option<&str>,
    lue_au: &Jour,
) -> R<()> {
    let a_dater = changements_a_dater(ancienne, donnees);
    if depuis.is_none() && !a_dater.is_empty() {
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        let fiche = champs::fiche(nom)?;
        let derive_d_un_change = |k: &str| {
            fiche.iter().find(|c| c.cle == k).and_then(|c| c.derive_de.as_ref()).is_some_and(|s| a_dater.contains(s))
        };
        let libelles: HashMap<&str, &str> = fiche.iter().map(|c| (c.cle.as_str(), c.libelle.as_str())).collect();
        let noms: Vec<String> = a_dater
            .iter()
            .filter(|k| !derive_d_un_change(k))
            .map(|k| format!("« {} »", libelles.get(k.as_str()).copied().unwrap_or(k)))
            .collect();
        return Err(refus(format!(
            "Changer {} demande sa date d'effet (« a partir du ») : ce qui precede garde l'ancienne valeur, et la date ne \
             se devine pas. Rien n'a ete enregistre.",
            noms.join(", ")
        )));
    }
    for (k, v) in donnees {
        if k.starts_with('_') {
            brute.insert(k.clone(), v.clone());
            continue;
        }
        let avant = ancienne.get(k.as_str());
        // « A COMPLETER » est une case vide a remplir, pas rien : il s'ecrit (une fiche importee le porte, et la fiche
        // d'un spectacle sans titre ne se lit pas).
        let rien = v.is_null() || v.as_str() == Some("");
        if (vide(avant) && rien) || avant.is_some_and(|a| meme(a, v)) {
            continue;
        }
        let nir_qui_arrive = k == "nir" && !brute.contains_key("nir") && depuis.is_some();
        let jour = if depuis.is_some() {
            depuis
        } else {
            droit::contexte::valeurs_datees(&Value::Object(brute.clone()), k)
                .map_err(plain)?
                .into_iter()
                .filter(|(j, _)| j.map_or(true, |j| j <= *lue_au))
                .last()
                .and_then(|(j, _)| j)
        };
        if nir_qui_arrive {
            droit::contexte::dater(brute, k, Value::String(String::new()), None, None);
        }
        droit::contexte::dater(brute, k, v.clone(), jour.as_ref(), note);
    }
    Ok(())
}

/// CE QUE L'ENREGISTREMENT D'UNE FICHE PASSE OUTRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forcer {
    /// Rien : une erreur refuse la fiche.
    Rien,
    /// Ce que l'ecran peut forcer (« Enregistrer quand meme ») : une erreur de forme (`forcable`), jamais une valeur
    /// obligatoire absente d'un salarie ou d'un spectacle, sans laquelle la DPAE et la DSN ne se font pas (
    /// `champs::verifier_fiche`).
    Forme,
    /// Tout : l'import et le questionnaire du salarie posent une fiche a completer, qui le dit (« A COMPLETER »).
    Tout,
}

impl Forcer {
    /// La charge `forcer` de la route `enregistrer-fiche` : l'ecran ne force que la forme.
    pub(crate) fn depuis_l_ecran(forcer: bool) -> Forcer {
        if forcer {
            Forcer::Forme
        } else {
            Forcer::Rien
        }
    }

    /// Ces anomalies refusent-elles la fiche ?
    fn refuse(self, anomalies: &[Value]) -> bool {
        let erreurs = || anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur"));
        match self {
            Forcer::Rien => erreurs().next().is_some(),
            Forcer::Forme => erreurs().any(|a| a.get("forcable") == Some(&Value::Bool(false))),
            Forcer::Tout => false,
        }
    }
}

/// Ce que `echoue` dit : la fiche porte au moins une erreur, elle n'a pas ete ecrite.
fn fiche_refusee(anomalies: &[Value]) -> Echec {
    let erreurs: Vec<&Value> = anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).collect();
    let phrases: Vec<String> = erreurs
        .iter()
        .map(|a| format!("{} : {}", a.get("champ").map_or_else(String::new, valeur_en_texte), a.get("message").map_or_else(String::new, valeur_en_texte)))
        .collect();
    let message = format!("{} erreur(s) : {}", erreurs.len(), phrases.join(" ; "));
    Echec { classe: Some("FicheRefusee"), message: json!({"message": message, "anomalies": anomalies}).to_string() }
}

/// Ce que l'ecriture d'une fiche a fait : sa cle (celle d'une fiche neuve est fabriquee ici), ce
/// qui a ete ecrit apres derivation, les anomalies, et les valeurs derivees.
pub(crate) struct FicheEcrite {
    pub(crate) cle: Option<String>,
    pub(crate) donnees: Map<String, Value>,
    pub(crate) anomalies: Vec<Value>,
    pub(crate) derives: Map<String, Value>,
}

fn enregistrer_fiche(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = texte_requis(charge, "nom")?;
    let cle: Option<String> = match charge.get("cle") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => return Err(erreur("TypeError", "`cle` : un texte est attendu")),
    };
    let donnees = exiger(charge, "donnees")?;
    let forcer = charge.get("forcer").is_some_and(vrai);
    let depuis = charge.get("depuis").unwrap_or(&Value::Null);
    let note = charge.get("note").unwrap_or(&Value::Null);
    let e = ecrire_fiche(ctx, nom, cle, donnees, Forcer::depuis_l_ecran(forcer), depuis, note)?;
    let fiche = crate::routes::ouverture::vue_fiche(nom, e.cle.as_deref(), &e.donnees, e.anomalies, &e.derives)?;
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    Ok(json!({"fiche": fiche, "configuration": configuration(o, ctx)?, "enregistre_a": maintenant()}))
}

/// `poser-courriel` : `{"salarie", "email"}`, le courriel d'un salarie, saisi la ou il manque (la boite des contrats a
/// faire signer). La fiche entiere s'ecrit comme depuis l'ecran (`ecrire_fiche`), seul le courriel y change : il se
/// verifie avec elle, et une fiche qui refuse le dit.
fn poser_courriel(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = texte_requis(charge, "salarie")?;
    let email = texte_requis(charge, "email")?;
    let Some(Value::Object(fiche)) = ctx.salaries.get(cle) else {
        return Err(refus(format!("Salarié {} inconnu.", socle::fichiers::repr(cle))));
    };
    let mut donnees: Map<String, Value> = fiche.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.clone())).collect();
    donnees.insert("email".into(), Value::String(email.trim().to_string()));
    ecrire_fiche(ctx, "salarie", Some(cle.to_string()), &Value::Object(donnees), Forcer::Rien, &Value::Null, &Value::Null)?;
    Ok(json!({"salarie": cle, "email": email.trim()}))
}

/// Verifie, date ce qui change, sauvegarde l'ancien fichier et
/// ecrit. Rend ce qui a ete ecrit, sans la page : l'import s'en sert fiche par fiche.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, cle: Option<String>, donnees: &Value, forcer: Forcer, depuis: &Value, note: &Value) -> R<FicheEcrite> {
    let mut cle = cle;
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
    };
    if !donnees.is_object() {
        return Err(erreur("TypeError", "`donnees` : un objet est attendu"));
    }
    let vide = Value::Object(Map::new());
    let ancienne: &Value = if nom == "entreprise" {
        &ctx.entreprise
    } else {
        let magasin = if nom == "salarie" { &ctx.salaries } else { &ctx.spectacles };
        cle.as_deref().and_then(|k| magasin.get(k)).unwrap_or(&vide)
    };
    // Les notes internes que l'ecran n'a pas vues sont rendues a la fiche AVANT tout le reste.
    let Value::Object(mut donnees) = crate::routes::fiches::garder_notes_internes(ancienne, donnees) else { unreachable!() };
    coercer_booleens(nom, &mut donnees)?;

    // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
    if nom == "salarie" && cle.as_deref().is_some_and(|k| !k.is_empty()) {
        let (avant, apres) = (ancienne.get("code_salarie"), donnees.get("code_salarie"));
        if let (Some(a), Some(b)) = (avant.filter(|v| vrai(v)), apres.filter(|v| vrai(v))) {
            let nir = donnees.get("nir").is_some_and(vrai) || ancienne.get("nir").is_some_and(vrai);
            if valeur_en_texte(a) != valeur_en_texte(b) && !nir {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    valeur_en_texte(a),
                    valeur_en_texte(b)
                )));
            }
        }
    }
    // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let nir = donnees.get("nir").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let nir = nir.trim();
        if !nir.is_empty() {
            let autres: Vec<&str> = ctx
                .salaries
                .iter()
                .filter(|(k, s)| Some(k.as_str()) != cle.as_deref() && s.get("nir").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim() == nir)
                .map(|(k, _)| k.as_str())
                .collect();
            if !autres.is_empty() {
                return Err(refus(format!(
                    "Le NIR {} est deja celui de la fiche {}. Un NIR designe une personne et une seule : completer ou reactiver cette \
                     fiche-la plutot que d'en creer une seconde.",
                    nir,
                    autres.join(", ")
                )));
            }
        }
    }
    // UN MATRICULE, UN SALARIE : la DSN rapproche chaque salarie de ses declarations par son matricule (S21.G00.30.019, dont
    // « le choix de la valeur revient a l'employeur », `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 327-329) ; deux fiches au
    // meme matricule s'y confondraient. `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let matricule = donnees.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let matricule = matricule.trim();
        if !matricule.is_empty() {
            let porteur = ctx.salaries.iter().find(|(k, s)| {
                Some(k.as_str()) != cle.as_deref()
                    && s.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim() == matricule
            });
            if let Some((k, s)) = porteur {
                let prenom = s.get("prenom_affichage").filter(|p| vrai(p)).or(s.get("prenom")).map(valeur_en_texte).unwrap_or_default();
                let qui = format!("{} {}", s.get("nom").map(valeur_en_texte).unwrap_or_default(), prenom).trim().to_string();
                return Err(refus(format!(
                    "Le matricule {} est déjà celui de {} (fiche {}). Un matricule désigne un salarié et un seul : la DSN \
rapproche chacun de ses déclarations par lui. En choisir un autre, ou laisser la case vide pour prendre le suivant.",
                    matricule,
                    if qui.is_empty() { k.as_str() } else { qui.as_str() },
                    k
                )));
            }
        }
    }
    let chemin = ctx.racine.join("config").join(fichier);
    if cle.as_deref().map_or(true, str::is_empty) && nom == "salarie" {
        // Le matricule d'une fiche neuve : le suivant. Un matricule saisi est garde tel quel.
        if !donnees.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
        } else {
            let existants = objet_du_fichier(&chemin)?;
            donnees.insert("code_salarie".into(), Value::String(matricule_suivant(&existants).to_string()));
        }
        // L'ANCIENNETE PART DE L'EMBAUCHE, pas du jour de la fiche : la validation du premier engagement la pose
        // (`poser_anciennete_a_l_embauche`) ; une date saisie est gardee telle quelle.
    }
    let derives = crate::routes::fiches::deriver(nom, &mut donnees, true)?;

    let anomalies = crate::routes::fiches::verifier(nom, &donnees)?;
    if forcer.refuse(&anomalies) {
        return Err(fiche_refusee(&anomalies));
    }

    let (depuis, note) = (depuis_de(depuis)?, Some(valeur_en_texte(note)).filter(|n| vrai(note) && !n.trim().is_empty()));
    let mut contenu = objet_du_fichier(&chemin)?;
    if nom == "entreprise" {
        // La fiche entreprise est le fichier entier.
        ecrire_datee(&mut contenu, nom, ancienne, &donnees, depuis, note.as_deref(), &ctx.lue_au)?;
        return finir_fiche(None, &chemin, contenu, donnees, anomalies, derives);
    }
    if cle.as_deref().map_or(true, str::is_empty) {
        // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
        cle = Some(cle_neuve(nom, &donnees, &contenu)?);
    }
    let k = cle.clone().unwrap();
    let mut brute = match contenu.get(&k) {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    ecrire_datee(&mut brute, nom, ancienne, &donnees, depuis, note.as_deref(), &ctx.lue_au)?;
    contenu.insert(k, Value::Object(brute));
    finir_fiche(cle, &chemin, contenu, donnees, anomalies, derives)
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (`import::completer`) : chaque champ de `champs` va dans la fiche
/// telle qu'elle est sur le disque, s'il y est vide ou « A COMPLETER » -- et seulement alors : un champ rempli depuis
/// l'apercu reste ce qu'il est. Une case vide n'avait rien declare : la valeur qui la remplit vaut depuis elle (un champ
/// absent : de tout temps) ; rien d'autre ne bouge ; l'ancien fichier est sauvegarde comme pour une fiche
/// de l'ecran. Rend les champs ecrits.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
    };
    let chemin = ctx.racine.join("config").join(fichier);
    let mut contenu = objet_du_fichier(&chemin)?;
    let cible: &mut Map<String, Value> = if nom == "entreprise" {
        &mut contenu
    } else {
        match cle.and_then(|k| contenu.get_mut(k)) {
            Some(Value::Object(m)) => m,
            _ => return Err(refus(format!("La fiche {} {} n'existe plus : rien n'est complete.", nom, cle.unwrap_or("?")))),
        }
    };
    let mut ecrits = Map::new();
    for (k, v) in champs {
        let en_vigueur = droit::contexte::valeurs_datees(&Value::Object(cible.clone()), k)
            .map_err(plain)?
            .into_iter()
            .filter(|(j, _)| j.map_or(true, |j| j <= ctx.lue_au))
            .last()
            .map(|(j, v)| (j, v.clone()));
        if crate::import::completer::vide(en_vigueur.as_ref().map(|(_, v)| v)) {
            droit::contexte::dater(cible, k, v.clone(), en_vigueur.and_then(|(j, _)| j).as_ref(), None);
            ecrits.insert(k.clone(), v.clone());
        }
    }
    if !ecrits.is_empty() {
        sauver_avant(&chemin)?;
        ecrire(&chemin, Value::Object(contenu))?;
    }
    Ok(ecrits)
}

fn finir_fiche(
    cle: Option<String>,
    chemin: &Path,
    contenu: Map<String, Value>,
    donnees: Map<String, Value>,
    anomalies: Vec<Value>,
    derives: Map<String, Value>,
) -> R<FicheEcrite> {
    sauver_avant(chemin)?;
    ecrire(chemin, Value::Object(contenu))?;
    Ok(FicheEcrite { cle, donnees, anomalies, derives })
}

// ------------------------------------------------------------ enregistrer-saisie




/// Le contrat de chaque engagement : (cle du salarie, premier jour, numero POSE dans la saisie) -- l'identite de
/// `jumeaux` : deux contrats du meme salarie qui commencent le meme jour ne se confondent pas.
pub(crate) fn identifiants(ctx: &Contexte, data: &Value) -> Result<Vec<Ident>, String> {
    let engs = engagement::engagements_de_la_saisie(ctx, data)?;
    let bruts = data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut out = Vec::new();
    for (i, e) in engs.iter().enumerate() {
        let fiche = ctx.salarie(&e.salarie)?;
        out.push((fiche.get("_cle").map_or_else(String::new, valeur_en_texte), e.debut.iso(), bruts.get(i).and_then(numero_pose)));
    }
    Ok(out)
}




// ----------------------------------------------------------------------- ajuster






// ------------------------------------------------------------------ la fiche demandee par courriel

/// Secondes entre une demande de fiche et la reponse de la personne fictive de la compagnie fictive (`DELAI_SIGNATURE`).
pub const DELAI_FICTIVE: i64 = 20;

/// `re.fullmatch(r"[^@\s]+@[^@\s]+\.[^@\s]+", adresse)`.
fn adresse_valide(adresse: &str) -> bool {
    let blanc = socle::texte::blanc;
    if adresse.chars().any(blanc) || adresse.matches('@').count() != 1 {
        return false;
    }
    let (local, domaine) = adresse.split_once('@').unwrap_or(("", ""));
    let d: Vec<char> = domaine.chars().collect();
    !local.is_empty() && d.len() >= 3 && (1..d.len() - 1).any(|i| d[i] == '.')
}

/// Ce que `demander_fiche` rend : le code, l'adresse (en minuscules), la ligne du journal.
pub struct FicheDemandee {
    pub code: String,
    pub adresse: String,
    pub ligne: Value,
}

/// Envoie a `adresse` le courriel « Remplir ma fiche ». Le journal garde
/// le code : c'est par lui que la releve reconnaitra la reponse. `client` : la boite, ouverte ici (boite simulee dans une
/// compagnie fictive, sinon refus « boite absente ») quand l'appelant n'en donne pas.
pub fn demander_fiche(ctx: &Contexte, adresse: &str, prenom: &str, preuve: &Preuve, qui: &str, client: Option<&mut dyn Courriel>) -> R<FicheDemandee> {
    let adresse = strip(adresse).to_lowercase();
    if !adresse_valide(&adresse) {
        return Err(refus(format!("« {} » n'est pas une adresse de courriel.", adresse)));
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => ouverte.insert(client_courriel(ctx)?).as_mut(),
    };
    verifier_preuve(ctx, Some(preuve), &socle::paris::maintenant()[..7], "demande de fiche (« Remplir ma fiche »)", &adresse)?;

    let ent = &ctx.entreprise;
    let compagnie = match vrai_ou(ent, "nom_affichage") {
        Some(c) => c,
        None => ent.get("raison_sociale").map(valeur_en_texte).unwrap_or_default(),
    };
    let sg = ent.get("signataire").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
    let signataire = match vrai_ou(&sg, "prenom_affichage") {
        Some(p) => p,
        None => capitaliser(&sg.get("prenom").map(valeur_en_texte).unwrap_or_default()),
    };
    let expediteur = client.compte().to_lowercase();
    let code = fq::code_fiche();
    let q = fq::questionnaire(&code);
    let sujet = envoi::formater(fq::SUJET, &[("compagnie", compagnie.clone())]);
    let prenom = strip(prenom).to_string();
    let virgule_prenom = if prenom.is_empty() { String::new() } else { format!(" {}", prenom) };
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![("virgule_prenom", e(&virgule_prenom)), ("compagnie", e(&compagnie)), ("signataire", e(&signataire))]
    };
    let lien = mailto(&expediteur, &format!("Re: {}", sujet), &q);
    let mut texte = champs(false);
    texte.push(("questionnaire", q.clone()));
    let mut html = champs(true);
    html.push(("questionnaire", html_echapper(&q)));
    html.push(("mailto", html_echapper(&lien)));
    let message = Message {
        cle: format!("fiche-{}", code),
        nom: if prenom.is_empty() { adresse.clone() } else { prenom.clone() },
        destinataire: adresse.clone(),
        sujet,
        corps: envoi::formater(fq::CORPS, &texte),
        html: envoi::formater(fq::HTML, &html),
        pieces: Vec::new(),
    };
    let mut identifiants: Vec<String> = Vec::new();
    if let Err(e) = envoi::envoyer(&[message], &expediteur, client, Some(&mut identifiants), None) {
        let detail = json!({"code": code, "adresse": adresse, "prenom": prenom});
        noter(ctx, "fiche-demande", None, qui, false, &adresse, &e, Some(&detail))?;
        return Err(echec_d_envoi(e));
    }
    let detail = json!({
        "code": code,
        "adresse": adresse,
        "prenom": prenom,
        "message": identifiants.first().cloned().unwrap_or_default(),
        "expediteur": expediteur,
    });
    let ligne = noter(ctx, "fiche-demande", None, qui, true, &adresse, &format!("fiche demandee a {}", adresse), Some(&detail))?;
    Ok(FicheDemandee { code, adresse, ligne })
}

// ------------------------------------------------------------------ les demandes attendues

/// Une demande partie et pas encore revenue.
#[derive(Debug, Clone)]
pub struct Attendue {
    pub code: String,
    pub adresse: String,
    pub prenom: String,
    /// L'heure de la demande (ISO), celle de la ligne du journal.
    pub quand: String,
    /// Les identifiants de NOS envois (le `Message-ID` gardé avec la demande) : jamais pris pour une reponse.
    pub envoyes: Vec<String>,
}

/// `fiches_attendues` : les demandes parties et pas encore revenues, les plus anciennes d'abord. Une demande a
/// laquelle on renonce se retire (`fiche-oublier-demande`, `fiches`).
pub fn fiches_attendues(ctx: &Contexte) -> R<Vec<Attendue>> {
    let lignes = envois::etats::charger(&ctx.racine)?;
    let detail = |l: &Value| -> Value { l.get("detail").filter(|d| vrai(d)).cloned().unwrap_or_else(|| json!({})) };
    let closes: Vec<Value> = envois::etats::filtrer(&lignes, Some("fiche-recue"), None, false)?.iter().map(|l| detail(l).get("code").cloned().unwrap_or(Value::Null)).collect();
    let mut sortie = Vec::new();
    for l in envois::etats::filtrer(&lignes, Some("fiche-demande"), None, false)? {
        let d = detail(l);
        let code = d.get("code").cloned().unwrap_or(Value::Null);
        if !l.get("accepte").is_some_and(vrai) || !vrai(&code) || closes.contains(&code) {
            continue;
        }
        let texte = |cle: &str| d.get(cle).map(valeur_en_texte).unwrap_or_default();
        sortie.push(Attendue {
            code: valeur_en_texte(&code),
            adresse: texte("adresse"),
            prenom: texte("prenom"),
            quand: l.get("quand").and_then(Value::as_str).unwrap_or("").to_string(),
            envoyes: d.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()).map(|m| vec![m.to_string()]).unwrap_or_default(),
        });
    }
    Ok(sortie)
}

// ------------------------------------------------------------------ la releve

/// Ce que la releve a fait : combien de demandes attendaient, les fiches creees, ce qui l'a empechee.
#[derive(Debug, Default)]
pub struct ReleveFiches {
    pub attendues: usize,
    /// `{code, cle, nom, illisible, erreurs}` par fiche creee ou completee.
    pub recues: Vec<Value>,
    pub incidents: Vec<String>,
}

impl ReleveFiches {
    /// `ReleveFiches.resume` : la phrase de la veille, vide quand rien n'est arrive.
    pub fn resume(&self) -> String {
        if self.recues.is_empty() {
            return String::new();
        }
        let noms: Vec<String> = self.recues.iter().map(|r| r.get("nom").map(valeur_en_texte).unwrap_or_default()).collect();
        format!("{} fiche(s) salarié reçue(s) : {}", self.recues.len(), noms.join(", "))
    }
}

fn os(m: String) -> Echec {
    erreur("OSError", m)
}

/// Cherche les reponses aux demandes de fiche et cree chaque fiche.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- la personne fictive de la compagnie fictive, ou la
/// vraie boite ; sans boite, un incident et rien d'autre. `qui` : `interface`, ou `veille`.
pub fn relever_fiches(ctx: &Contexte, client: Option<&mut dyn Courriel>, qui: &str) -> R<ReleveFiches> {
    let mut r = ReleveFiches::default();
    let attendues = fiches_attendues(ctx)?;
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None if branchement.joue() => {
            // LA COMPAGNIE FICTIVE : la personne fictive repond, et c'est la vraie lecture qui suit.
            ouverte.insert(Box::new(boite_des_fiches(ctx, &attendues, DELAI_FICTIVE)?)).as_mut()
        }
        None => {
            // Le message : rien n'est parti, rien ne revient.
            if let Some(refus) = branchement.refus() {
                r.incidents.push(refus.phrase.to_string());
                return Ok(r);
            }
            if !crate::compagnie::boite::disponible(&ctx.racine) {
                r.incidents.push("Aucune boite courriel : les fiches demandees ne peuvent pas etre relevees.".to_string());
                return Ok(r);
            }
            let vrai_client = crate::compagnie::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = &a.code;
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e));
                break;
            }
        };
        let mut retenu: Option<(String, Vec<u8>, String, String, String)> = None;
        for ident in identifiants {
            if a.envoyes.contains(&ident) {
                continue;
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e));
                    continue;
                }
            };
            let Ok(m) = mailparse::parse_mail(&brut) else {
                r.incidents.push(format!("Lecture du message {} : message illisible.", ident));
                continue;
            };
            if a.envoyes.contains(&lecture::identifiant_de(&m)) {
                continue;
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.as_str()) || texte.contains("Remplir ma fiche") {
                continue;
            }
            let de = lecture::adresse_de(&m);
            if de == expediteur && de != a.adresse {
                continue;
            }
            if de != a.adresse {
                r.incidents.push(format!("Fiche {} : une reponse vient de {} et non de {} ; elle n'est pas retenue.", code, de, a.adresse));
                continue;
            }
            let quand = lecture::date_de(&m, &socle::paris::maintenant());
            retenu = Some((ident, brut.clone(), texte, de, quand));
            break;
        }
        let Some((ident, brut, texte, de, quand)) = retenu else { continue };
        let (mut donnees, illisible) = fq::fiche_depuis_reponses(&fq::lire_reponses(&texte));
        donnees.insert("email".into(), json!(de));
        if !(donnees.get("nom_affichage").is_some_and(vrai) || donnees.get("prenom_affichage").is_some_and(vrai)) {
            r.incidents.push(format!("Fiche {} : la reponse de {} ne porte ni nom ni prenom lisible ; rien n'est cree.", code, de));
            continue;
        }
        let dossier = ctx.racine.join("sorties").join("fiches-salaries");
        std::fs::create_dir_all(&dossier).map_err(|e| os(format!("{} : {}", dossier.display(), e)))?;
        let eml = dossier.join(format!("Fiche_{}.eml", code));
        std::fs::write(&eml, &brut).map_err(|e| os(format!("{} : {}", eml.display(), e)))?;
        let jour: Vec<&str> = quand.get(..10).unwrap_or("").split('-').rev().collect();
        let mut note = format!("Fiche remplie par la personne, par courriel, le {}. Reste à choisir l'emploi.", jour.join("/"));
        if !illisible.is_empty() {
            note.push_str(&format!(" Non lu, à compléter : {}.", illisible.join(" ; ")));
        }
        let existante: Option<String> = ctx
            .salaries
            .iter()
            .find(|(k, s)| s.is_object() && !k.starts_with('_') && strip(&s.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).to_lowercase() == de)
            .map(|(k, _)| k.clone());
        let donnees: Map<String, Value> = match &existante {
            Some(k) => {
                // Une fiche qui existe deja : on ne complete que ses cases VIDES. Ce qu'elle porte a peut-etre deja
                // ete declare.
                let ancienne = ctx.salaries.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
                let mut fusion = ancienne.clone();
                for (cle, valeur) in &donnees {
                    if strip(&ancienne.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).is_empty() {
                        fusion.insert(cle.clone(), valeur.clone());
                    }
                }
                fusion.insert("fiche_recue".into(), json!(note));
                fusion
            }
            None => {
                let mut d = donnees;
                d.insert("fiche_recue".into(), json!(note));
                d.insert("actif".into(), json!(true));
                d
            }
        };
        let f = crate::routes::fiches::ecrire_fiche(ctx, "salarie", existante.clone(), &Value::Object(donnees.clone()), crate::routes::fiches::Forcer::Tout, &Value::Null, &Value::Null)?;
        let texte_de = |cle: &str| donnees.get(cle).map(valeur_en_texte).unwrap_or_default();
        let nom = format!("{} {}", texte_de("prenom_affichage"), texte_de("nom_affichage"));
        let erreurs = f.anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).count();
        let detail = json!({"code": code, "salarie": f.cle, "illisible": illisible, "erreurs": erreurs});
        let reponse = format!("fiche {} par courriel", if existante.is_some() { "completee" } else { "creee" });
        let origine = format!("courriel {}", ident);
        let note_du_journal = Note {
            quoi: "fiche-recue",
            mois: None,
            fichier: None,
            mode: journal::REEL,
            qui,
            accepte: true,
            destinataire: &de,
            flux: "",
            reponse: &reponse,
            trace: None,
            detail: Some(&detail),
            versions: None,
            quand: &quand,
            origine: &origine,
        };
        journal::noter(&ctx.racine, &note_du_journal, &socle::paris::maintenant()).map_err(os)?;
        r.recues.push(json!({"code": code, "cle": f.cle, "nom": strip(&nom), "illisible": illisible, "erreurs": erreurs}));
    }
    client.fermer();
    Ok(r)
}

// ------------------------------------------------------------------ la compagnie fictive

/// Des personnes inventees, comme celles des compagnies fictives : (prenom, nom, sexe INSEE). Le NIR se
/// fabrique a la demande, cle comprise.
const PERSONNES: [(&str, &str, &str); 4] = [("Marie", "Dorval", "2"), ("Pierre", "Bocage", "1"), ("Rose", "Cheri", "2"), ("Frederick", "Lemaitre", "1")];

/// `_reponse_de_fiche` : le questionnaire que la personne fictive renvoie, rempli ligne a ligne dans les termes memes
/// des questions, le code en dernier. `bouton` : (objet, corps) du lien du courriel de la demande, quand on l'a.
fn reponse_de_fiche(demande: &Attendue, compte: &str, bouton: Option<&(String, String)>) -> Result<Vec<u8>, String> {
    let code = &demande.code;
    let somme: u32 = code.chars().map(|c| c as u32).sum();
    let (prenom, nom, sexe) = PERSONNES[somme as usize % PERSONNES.len()];
    let prenom = if demande.prenom.is_empty() { prenom.to_string() } else { demande.prenom.clone() };
    let an = 80 + code.chars().count() % 15;
    let mois = 1 + somme % 12;
    let treize = format!("{}{:02}{:02}75056{:03}", sexe, an, mois, 1 + somme % 900);
    let cle = 97 - treize.parse::<u64>().unwrap_or(0) % 97;
    let reponses: HashMap<&str, String> = HashMap::from([
        ("prenom", prenom),
        ("nom", nom.to_string()),
        ("naissance", format!("12/{:02}/19{:02}", mois, an)),
        ("lieu", "Paris".to_string()),
        ("pays", "France".to_string()),
        ("nationalite", "française".to_string()),
        ("nir", format!("{}{:02}", treize, cle)),
        ("adresse", "12 rue du Temple".to_string()),
        ("code_postal", "75003".to_string()),
        ("commune", "Paris".to_string()),
        // Le compte fabrique des fictives, a la cle juste (`socle::iban::faute_iban`).
        ("iban", "FR7630001007941234567890185".to_string()),
    ]);
    let mut lignes: Vec<String> =
        fq::QUESTIONS.iter().map(|(cle, lib, fac)| format!("{}{} : {}", lib, if *fac { " (facultatif)" } else { "" }, reponses.get(cle).map(String::as_str).unwrap_or(""))).collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", code));
    let sujet = bouton.map(|b| b.0.clone()).unwrap_or_else(|| "Re: fiche".to_string());
    envois::simulation_courriels::reponse_jouee(&demande.adresse, compte, &sujet, &lignes.join("\n"), &format!("fiche-{}", code))
}

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de fiche partie
/// depuis `delai` secondes y trouve sa reponse remplie ; la vraie releve la lit et cree la fiche. Ce qu'on y envoie
/// s'ecrit comme tout courriel joue (`BoiteQuiRepond`, partagee avec `BoiteDesSignatures`).
pub fn boite_des_fiches(ctx: &Contexte, attendues: &[Attendue], delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx);
    let maintenant_naif = socle::paris::maintenant_naif();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine);
    for a in attendues {
        let age = socle::paris::naif_de_iso(&a.quand).map_or(0, |t| maintenant_naif - t);
        if age < delai {
            continue;
        }
        let bouton = envois::simulation_courriels::bouton_de_la_demande(&envoyes, &a.envoyes);
        let brut = reponse_de_fiche(a, Courriel::compte(&boite), bouton.as_ref()).map_err(|m| erreur("ValueError", m))?;
        boite.repondre(&a.code, brut);
    }
    Ok(boite)
}

// ------------------------------------------------------------------ le modele du bulletin

/// Le modele que la charge nomme (`modele`) : un modele connu, rien d'autre.
fn modele_demande(charge: &Value) -> R<Modele> {
    let v = charge.get("modele").cloned().unwrap_or(Value::Null);
    let cle = v.as_str().map(str::trim).unwrap_or_default();
    Modele::TOUS.iter().copied().find(|m| m.cle() == cle).ok_or_else(|| {
        refus(format!("Modèle de bulletin inconnu : {}. Connus : « simplifié », « complet ».", v))
    })
}

/// L'adresse ou la fenetre sert l'apercu d'un modele (`interface::Fenetre::apercu_bulletin`).
pub fn adresse_apercu(m: Modele) -> String {
    format!("/apercu/bulletin?modele={}", if m == Modele::Simplifie { "simplifie" } else { "complet" })
}

/// Le modele d'une adresse d'apercu (`simplifie`, `complet` : sans accent dans l'adresse).
pub fn modele_de_l_adresse(v: &str) -> Option<Modele> {
    match v {
        "simplifie" | "simplifié" => Some(Modele::Simplifie),
        "complet" => Some(Modele::Complet),
        _ => None,
    }
}

// ------------------------------------------------------------------ bulletin-modeles

fn modeles(ctx: &Contexte) -> R<Value> {
    let choisi = Modele::de_la_fiche(&ctx.entreprise, "entreprise").map_err(refus)?;
    let modele = choisi.unwrap_or(Modele::Simplifie);
    // Les salaries dont la case dit un autre modele que la compagnie : la page les nomme, la decision reste au moteur.
    let mut autrement = Vec::new();
    for (cle, sal) in &ctx.salaries {
        if let Some(m) = Modele::de_la_fiche(sal, &format!("du salarié {}", cle)).map_err(refus)? {
            if m != modele {
                let champ = |k: &str| sal.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let nom = format!("{} {}", champ("prenom_affichage"), champ("nom_affichage")).trim().to_string();
                let nom = if nom.is_empty() { cle.clone() } else { nom };
                autrement.push(json!({"cle": cle, "nom": nom, "modele": m.cle()}));
            }
        }
    }
    let liste: Vec<Value> = Modele::TOUS
        .iter()
        .map(|m| json!({"cle": m.cle(), "nom": m.nom(), "description": m.description(), "apercu": adresse_apercu(*m),
            // Le modele de la compagnie : sa pastille, celle d'un choix fait.
            "pastille": if *m == modele { super::pastille("fait", if choisi.is_none() { "celui de la compagnie (par défaut)" } else { "celui de la compagnie" }) } else { Value::Null }}))
        .collect();
    Ok(json!({
        "choisi": modele.cle(),
        "par_defaut": choisi.is_none(),
        "modeles": liste,
        "salaries_autrement": autrement,
    }))
}

// ------------------------------------------------------------------ l'apercu

/// La compagnie fictive de l'apercu, livree avec le logiciel.
pub fn dossier_apercu() -> PathBuf {
    dossier_du_code().join("apercu-bulletin")
}

/// La description du bulletin de la paie fictive sous `modele` : le moteur du jour, la compagnie fictive, rien d'ecrit.
pub fn apercu_description(modele: Modele) -> Result<Value, String> {
    let racine = dossier_apercu();
    let saisie = lire_json(&racine.join("saisie.json"))?;
    let mois = saisie.get("mois").and_then(Value::as_str).ok_or("La paie de l'aperçu ne dit pas son mois.")?.to_string();
    let jour = saisie.get("date_paiement").and_then(Value::as_str).ok_or("La paie de l'aperçu ne dit pas sa date de paiement.")?;
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire(jour)?))?;
    let s = engagement::pour_l_ecran(&ctx, &saisie)?;
    let mut ent = ctx.entreprise.clone();
    let o: &mut Map<String, Value> = ent.as_object_mut().ok_or("La fiche de la compagnie de l'aperçu n'est pas un objet.")?;
    o.insert(CASE_MODELE.into(), json!(modele.cle()));
    // Ni le logo ni la signature du poste sur une paie fictive : ses gabarits sont un dossier qui n'existe pas.
    o.insert("_gabarits".into(), json!(racine.join("gabarits").to_string_lossy()));
    // Une paie de l'ecran n'a pas de numero de bulletin (il se reserve au depart) : l'apercu montre celui d'un premier
    // bulletin de l'annee.
    let numeros: Vec<Option<String>> = (1..=s.paies.len()).map(|n| Some(format!("{}/{}", n, &mois[..4]))).collect();
    jeux_et_description(&ctx, &s.paies, &numeros, &ent, &mois, None, false, &HashMap::new())
}

/// Le PDF de la paie fictive sous `modele`.
pub fn apercu_pdf(modele: Modele) -> Result<Vec<u8>, String> {
    documents::fabrique::fabriquer_octets(&apercu_description(modele)?)
}

// ------------------------------------------------------------------ bulletin-modele-choisir

fn choisir(ctx: &Contexte, charge: &Value) -> R<Value> {
    let modele = modele_demande(charge)?;
    // Le modele vaut pour les documents faits a partir d'aujourd'hui ; ceux d'avant gardent le leur.
    crate::routes::fiches::poser_entreprise(ctx, CASE_MODELE, json!(modele.cle()), Some(ctx.aujourdhui))?;
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(refus)?;
    modeles(&neuf)
}
