//! Les routes de la fiche (`moteur/web.py`, `operations_fiches`, `champs`) :
//! `verifier-fiche`, `changements-declares`, `normaliser`, `taux-atmp`.
//!
//! Aucune ecriture. `revue-spectacles` n'est pas ici : elle archive des fiches
//! et note l'echeance, elle reste en Python. `taux-atmp` ne LIT que : quand la
//! page donne une `valeur`, c'est `web.py` qui la pose (Python) avant d'appeler
//! cette lecture.
//!
//! Les refus gardent la classe d'exception du Python (`KeyError`, `TypeError`,
//! `ErreurOperation`...), que `paie_rust.route` releve sous le meme nom.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::champs;
use droit::contexte::Contexte;
use socle::valeur::valeurs_egales;
use socle::valeur::valeur_ecrite;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::echec::Echec;

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "normaliser" => normaliser(charge),
        "taux-atmp" => taux_atmp(ctx, charge),
        "verifier-fiche" => verifier_fiche(charge),
        "changements-declares" => changements_declares(ctx, charge),
        "taxe-sur-les-salaires-regime" => taxe_sur_les_salaires_regime(ctx, charge),
        "permanents" => permanents(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ permanents

/// `permanents` : `{"entreprise": {...}}`, la fiche Entreprise en cours de saisie -- sans elle, celle de la compagnie.
/// Rend le curseur (`contexte::emploie_des_permanents`) : la page montre ou replie ce qui ne vaut que pour les
/// permanents selon cette reponse, et ne le decide pas elle-meme quand on coche la case (ticket #214).
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
/// lui, celle de la compagnie. Rend la conclusion du moteur (`assujettissement::regime_taxe_sur_les_salaires`, CGI art.
/// 231, 1) : les cases a montrer, la consequence en clair, et si la reponse est complete. La page n'en decide rien
/// (ticket #216).
fn taxe_sur_les_salaires_regime(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = crate::assujettissement::CHAMP_TAXE_SUR_LES_SALAIRES;
    let ent = match charge.get(cle) {
        Some(v @ (Value::Object(_) | Value::Null)) => json!({ cle: v }),
        Some(autre) => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
        None => ctx.entreprise.clone(),
    };
    // LES PLAFONDS DE LA FRANCHISE, CHIFFRES (ticket #239) : ceux du bareme de l'annee des salaires -- `annee` dans la
    // charge, sinon l'annee en cours. Un bareme absent ne fait pas refuser : la question se pose sans montants.
    let annee = match charge.get("annee") {
        None | Some(Value::Null) => ctx.aujourdhui.an,
        Some(v) => match v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(a) if (1900..=2999).contains(&a) => a as i32,
            _ => return Err(erreur("ValueError", socle::date::annee_impossible(valeur_ecrite(v)))),
        },
    };
    let bar = socle::date::Jour::nouveau(annee, 1, 1).and_then(|j| ctx.bareme(&j).ok());
    let q = crate::assujettissement::question_franchise(bar.as_ref(), annee);
    // La conclusion nomme les deux plafonds (ticket #245).
    let r = crate::assujettissement::regime_taxe_sur_les_salaires(&ent, q.plafonds);
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

// --------------------------------------------------------------- normaliser

/// `normaliser` : la forme que la case devrait avoir ; une case inconnue de la
/// fiche rend la valeur telle quelle.
fn normaliser(charge: &Value) -> R<Value> {
    let nom = nom_de_fiche(exiger(charge, "nom")?)?;
    let cle = exiger(charge, "champ")?;
    let champ = champs::fiche(nom)?.into_iter().find(|c| cle.as_str() == Some(c.cle.as_str()));
    let valeur = exiger(charge, "valeur")?;
    let rendu = match champ {
        Some(c) => champs::normaliser(&c, valeur)?,
        None => valeur.clone(),
    };
    Ok(json!({ "valeur": rendu }))
}

// ------------------------------------------------------------------ taux-atmp

/// `int(v)` de Python, pour ce qu'un JSON porte.
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
                && crate::routes_ecrites::chiffre_py(c[0]).is_some()
                && crate::routes_ecrites::chiffre_py(c[c.len() - 1]).is_some()
                && c.iter().all(|x| crate::routes_ecrites::chiffre_py(*x).is_some() || *x == '_')
                && !chiffres.contains("__");
            if !bien_forme {
                return Err(illisible());
            }
            let ascii: String = c.iter().filter(|x| **x != '_').filter_map(|x| crate::routes_ecrites::chiffre_py(*x)).map(|d| char::from(b'0' + d as u8)).collect();
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

/// `taux_atmp` : le taux AT/MP d'une annee, lu ; pose d'abord si `valeur` est donnee.
fn taux_atmp(ctx: &Contexte, charge: &Value) -> R<Value> {
    let an = entier(exiger(charge, "annee")?)?;
    // `valeur` donnee : on POSE d'abord (`routes_ecrites`), puis on relit.
    if let Some(v) = charge.get("valeur").filter(|v| !v.is_null() && v.as_str() != Some("")) {
        let vide = json!("");
        crate::routes_ecrites::poser_taux_atmp(ctx, an, v, charge.get("code_risque").unwrap_or(&vide))?;
    }
    let p = ctx.racine.join("config").join("baremes").join(format!("{}.json", an));
    let taux = match lire_tolerant(&p)? {
        None => Value::Null,
        Some(Value::Object(o)) => match o.get("taux_atmp") {
            Some(Value::Object(t)) if t.get("valeur").map_or(false, |v| !v.is_null()) => {
                let code = t.get("code_risque").filter(|c| vrai(c)).cloned().unwrap_or_else(|| json!(""));
                json!({"valeur": t["valeur"], "code_risque": code})
            }
            _ => Value::Null,
        },
        Some(autre) => {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&autre))));
        }
    };
    Ok(json!({"annee": an, "taux": taux}))
}

// ------------------------------------------------------------ verifier-fiche

/// `dict(v)` de Python, pour ce qu'un JSON porte : un objet se copie ; une liste
/// de paires aussi ; le reste se refuse sous la classe et la phrase de Python.
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
/// passe par le controle de la norme ; ce qui n'en est pas un fait lever a
/// Python les memes `TypeError` que la table des formats lui donne a lire.
fn fautif(c: &champs::Champ, actuel: &Value) -> R<bool> {
    if let Value::String(s) = actuel {
        return Ok(champs::faute_de_norme(c, s)?.is_some());
    }
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(false) };
    let format = crate::formats::format_de(rubrique)?;
    if format.as_object().map_or(true, |o| o.is_empty()) {
        return Ok(false);
    }
    let motifs = format.get("csl").and_then(Value::as_array).map_or(false, |l| !l.is_empty());
    let pas_un_texte = || erreur("TypeError", format!("expected string or bytes-like object, got '{}'", nom_type(actuel)));
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

/// `champs._capitales` : sans accents, en capitales, espaces resserrees.
fn capitales(v: &Value) -> String {
    let t: String = champs::sans_accents(&valeur_en_texte(v)).chars().flat_map(char::to_uppercase).collect();
    t.split(champs::espace).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// `champs.deriver_personnes` : les formes declarees du signataire et du contact DSN ;
/// rend `{cle: [avant, apres]}` pour les structures modifiees.
fn deriver_personnes(donnees: &mut Map<String, Value>, forcer: bool) -> Map<String, Value> {
    let mut faits = Map::new();
    // Le contact de la medecine du travail interne (S20.G00.07 de type 17, `dsn::contact_sante_travail`) s'ecrit comme
    // celui de la DSN : « NOM PRENOM ».
    let personnes: [(&str, &[&str], Option<(&str, [&str; 2])>); 3] = [
        ("signataire", &["nom", "prenom"], None),
        ("declarant_dsn", &[], Some(("nom_prenom", ["nom", "prenom"]))),
        ("contact_sante_travail", &[], Some(("nom_prenom", ["nom", "prenom"]))),
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

/// LE SIREN ET LE NIC SE DEDUISENT DU SIRET (#236 ; la page les posait elle-meme, `brancherDeduits`, regle 2) : le SIRET
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

/// `champs.deriver` : fabrique les formes declarees depuis les formes lisibles.
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
        let cle = crate::conges_spectacles::CLE_ADHERENT;
        if let Some(Value::String(brut)) = donnees.get(cle).cloned() {
            let forme = crate::conges_spectacles::forme_adherent(&brut);
            if forme != brut {
                faits.insert(cle.into(), json!([brut, forme.clone()]));
                donnees.insert(cle.into(), Value::String(forme));
            }
        }
        faits.extend(deriver_personnes(donnees, forcer));
        faits.extend(deriver_siren_nic(donnees));
    }
    if nom == "salarie" && champs::strip(&texte_ou_vide(donnees.get("initiales_contrat"))).is_empty() {
        let lettres: String = texte_ou_vide(donnees.get("nom")).chars().filter(|c| c.is_ascii_uppercase()).collect();
        if !lettres.is_empty() {
            let deux: String = lettres.chars().take(2).collect();
            faits.insert("initiales_contrat".into(), json!(["", deux.clone()]));
            donnees.insert("initiales_contrat".into(), Value::String(deux));
        }
    }
    // LA NATIONALITE DSN SE DEDUIT DU PAYS (vague 1k) : avec un pays de nationalite de la liste, la case 30.013 ne se
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

/// `urssaf.pour_departement` : l'organisme du territoire, ou "".
fn urssaf_du_departement(dep: &str) -> R<String> {
    let table = json_en_cache(&dossier_du_code().join("urssaf-dpae-base.json")).map_err(|m| Echec::from(m))?;
    let d = champs::strip(dep).to_uppercase();
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

/// `ops.suggestions_fiche` : ce que le moteur PROPOSERAIT pour une case encore vide.
fn suggestions(nom: &str, donnees: &Map<String, Value>) -> R<Value> {
    let mut propose = Map::new();
    if nom == "salarie" {
        let nir = champs::strip(&texte_ou_vide(donnees.get("nir"))).to_string();
        let c: Vec<char> = nir.chars().collect();
        if c.len() == 13 && champs::tous_chiffres(&nir) && (c[0] == '1' || c[0] == '2') {
            let dep: String = c[5..7].iter().collect();
            if dep != "20" {
                let mut trouvees = vec![("departement_naissance", dep.clone())];
                if dep != "99" {
                    trouvees.push(("pays_naissance", "FR".to_string()));
                }
                for (cle, valeur) in trouvees {
                    if champs::strip(&texte_ou_vide(donnees.get(cle))).is_empty() {
                        propose.insert(cle.into(), json!({"valeur": valeur, "pourquoi": "lu dans le numéro de sécurité sociale"}));
                    }
                }
            }
        }
    } else if nom == "entreprise" && champs::strip(&texte_ou_vide(donnees.get("code_urssaf"))).is_empty() {
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

/// `operations_fiches.verifier_fiche` : les anomalies de la fiche APRES derivation, sur une copie.
pub(crate) fn verifier(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut copie = donnees.clone();
    deriver(nom, &mut copie, false)?;
    champs::verifier_fiche(nom, &copie)
}

/// LES ANOMALIES QUE L'ECRITURE TROUVERA (#236) : celles d'une fiche que l'import cree, derivee comme `ecrire_fiche` la
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

/// `Path.parent` de Python : `.` pour un nom seul.
fn parent_py(p: &Path) -> std::path::PathBuf {
    match p.parent() {
        Some(x) if x.as_os_str().is_empty() => std::path::PathBuf::from("."),
        Some(x) => x.to_path_buf(),
        None => p.to_path_buf(),
    }
}

/// `contexte.trouver_export` : le dossier de CoolPaie, remonte de dossier en
/// dossier, puis la copie `export-coolpaie`, puis `export` (absent).
pub(crate) fn trouver_export(depart: &Path) -> std::path::PathBuf {
    let p1 = parent_py(depart);
    let p2 = parent_py(&p1);
    for base in [depart, p1.as_path(), p2.as_path()] {
        let p = base.join("export");
        if p.is_dir() {
            return p;
        }
    }
    let copie = depart.join("export-coolpaie");
    if copie.is_dir() {
        return copie;
    }
    depart.join("export")
}

/// `declarations_coolpaie` : la copie de l'import quand elle porte des DSN ou des
/// AEM, l'`export/` de CoolPaie sinon.
pub(crate) fn declarations_coolpaie(ctx: &Contexte) -> std::path::PathBuf {
    let copie = ctx.racine.join("historique").join("coolpaie");
    if copie.join("DSN").is_dir() || copie.join("AEM_EDI").is_dir() {
        return copie;
    }
    trouver_export(&ctx.racine)
}

/// `dossier.rglob("*.dsn")` : les fichiers (et dossiers) dont le nom finit par
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
    for dossier in [declarations_coolpaie(ctx).join("DSN"), ctx.racine.join("sorties")] {
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
            for l in crate::usages::lignes(&texte) {
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
/// remplaceraient ; chaque valeur existante qui changerait (`routes_ecritures::changements_a_dater`), celles-la memes
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
    let a_dater = crate::routes_ecritures::changements_a_dater(ancienne, &donnees);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// La forme declaree d'une case : majuscules sans accent, civilite retiree, apostrophe typographique, chiffres seuls
    /// du SIREN ; ce qui n'est pas un texte, ou une case inconnue, revient tel quel.
    #[test]
    fn normaliser_rend_la_forme_declaree_d_une_case() {
        let v = |nom: &str, champ: &str, valeur: Value| normaliser(&json!({"nom": nom, "champ": champ, "valeur": valeur})).unwrap()["valeur"].clone();
        assert_eq!(v("salarie", "nom", json!(" Béjart Duparc ")), json!("BEJART DUPARC"));
        assert_eq!(v("salarie", "nom", json!("Madame Dupont")), json!("DUPONT"));
        assert_eq!(v("salarie", "adresse", json!("2 rue d’Arvey")), json!("2 RUE D'ARVEY"));
        assert_eq!(v("entreprise", "siren", json!(" 732 829 320 ")), json!("732829320"));
        assert_eq!(v("salarie", "email", json!(" a@b.fr ")), json!("a@b.fr"));
        assert_eq!(v("salarie", "nom", json!(12)), json!(12));
        assert_eq!(v("salarie", "inconnue", json!(" x ")), json!(" x "));
        for charge in [json!({}), json!({"nom": "salarie"}), json!({"nom": "salarie", "champ": "nom"}), json!({"nom": "autre", "champ": "nom", "valeur": "x"})] {
            assert!(normaliser(&charge).is_err(), "{}", charge);
        }
    }
}
