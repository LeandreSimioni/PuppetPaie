//! Les listes que la page lit en s'ouvrant (`paie route`) : `arrets`,
//! `fins-de-contrat`, `regularisations`.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et ce qu'il appelait :
//! `operations_arrets` (`tous_les_arrets`, `a_signaler`, `resume`),
//! `signalement` (`motifs_arret`, `motifs_reprise`, `nomenclatures`),
//! `operations_permanents.fins_a_signaler` (+ `_ce_qui_manque_au_signalement`),
//! `operations_regularisation` (`mois_a_corriger`, `etat_mois`,
//! `corrections_par_mois`) et `coffre.ouverture_exigee` -- portes a
//! l'identique : meme JSON (l'enveloppe de `web.py` comprise), memes refus sous
//! la meme classe.
//!
//! LECTURE SEULE, AUCUN CALCUL DE PAIE, AUCUN RESEAU, AUCUN SOUS-PROCESSUS.
//! `fins-de-contrat` demandait au Python un sous-processus `paie preavis` par
//! contrat sans motif de preavis : ici c'est `solde::duree_du_preavis`, appelee
//! directement. Le journal des depots n'est lu qu'une fois par mois de saisie
//! (`regularisations`), par les etats de `etats.rs`.
//!
//! Ecarts assumes : le texte d'un `JSONDecodeError` est celui de serde, pas celui
//! de Python (la classe est la meme) ; un `mois` de correction qui n'est pas un
//! texte (fichier abime a la main) n'est range que pour les cas simples.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::date::{d, Jour};
use crate::etats::{erreur, get, getd, items, nom_type, objet_vide, ou, py_repr, py_str, LISTE_VIDE, R};
use crate::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use crate::num::Decimal;
use crate::retours::Echec;
use crate::routes_mois::Moteur;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["arrets", "fins-de-contrat", "regularisations"];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "arrets" => arrets(ctx),
        "fins-de-contrat" => fins_de_contrat(ctx, charge),
        "regularisations" => regularisations(o, ctx),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

fn plain(m: impl Into<String>) -> Echec {
    Echec { classe: None, message: m.into() }
}

/// Un refus du coffre, sous la classe que Python levait : une `OSError` se
/// releve sous la sous-classe que son errno designe (`IsADirectoryError`...).
pub(crate) fn vers_route(x: crate::coffre::Echec) -> Echec {
    let classe = match (x.classe, x.os.as_ref().map(|o| o.0)) {
        ("OSError", Some(1)) | ("OSError", Some(13)) => "PermissionError",
        ("OSError", Some(2)) => "FileNotFoundError",
        ("OSError", Some(17)) => "FileExistsError",
        ("OSError", Some(20)) => "NotADirectoryError",
        ("OSError", Some(21)) => "IsADirectoryError",
        (c, _) => c,
    };
    Echec { classe: Some(classe), message: x.message }
}

/// `p.read_text(encoding="utf-8")`.
pub(crate) fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| {
        let classe = if p.is_dir() {
            "IsADirectoryError"
        } else {
            match e.kind() {
                std::io::ErrorKind::NotFound => "FileNotFoundError",
                std::io::ErrorKind::PermissionDenied => "PermissionError",
                _ => "OSError",
            }
        };
        erreur(classe, crate::fichiers::panne_fichier(&e, p))
    })?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))
}

/// `json.loads(p.read_text(encoding="utf-8"))`.
pub(crate) fn lire_json(p: &Path) -> R<Value> {
    serde_json::from_str(&lire_texte(p)?).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

/// `sorted(dossier.glob("????-??.json"))` : les (radical, chemin) des fichiers
/// d'un mois, dans l'ordre des noms. Un dossier absent, ou qui n'en est pas
/// un, ne donne rien.
fn fichiers_de_mois(dossier: &Path) -> Vec<(String, PathBuf)> {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut out = Vec::new();
    for e in lecture.flatten() {
        let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
        let c: Vec<char> = nom.chars().collect();
        if c.len() == 12 && c[4] == '-' && nom.ends_with(".json") {
            out.push((c[..7].iter().collect::<String>(), e.path()));
        }
    }
    out.sort();
    out
}

/// `int(texte)` pour un morceau de nom de mois.
fn entier(t: &str) -> R<i64> {
    t.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(t))))
}

/// Refuse `c` quand ce n'est pas un objet, qu'il porte une cle inconnue ou qu'il oublie un champ obligatoire.
fn construire(classe: &str, c: &Value, champs: &[&str], requis: usize) -> R<()> {
    let Value::Object(m) = c else {
        return Err(erreur("TypeError", format!("{} : un objet est attendu, pas {}", classe, nom_type(c))));
    };
    if let Some(k) = m.keys().find(|k| !champs.contains(&k.as_str())) {
        return Err(erreur("TypeError", format!("{} : champ inconnu {}", classe, repr(k))));
    }
    let manque: Vec<String> = champs[..requis].iter().filter(|k| !m.contains_key(**k)).map(|k| repr(k)).collect();
    if !manque.is_empty() {
        return Err(erreur("TypeError", format!("{} : champ manquant {}", classe, manque.join(", "))));
    }
    Ok(())
}

/// `dict.get(cle)` d'une dataclass construite : `None` pour un champ absent.
fn champ(v: &Value, k: &str, defaut: Value) -> Value {
    v.get(k).cloned().unwrap_or(defaut)
}

// -------------------------------------------------------------------- arrets

/// Un refus du magasin des imprevus (`imprevus`) : un fichier abime, un genre inconnu, le dossier `arrets/` d'avant.
pub(crate) fn refus_magasin(m: String) -> Echec {
    erreur("ErreurOperation", m)
}

/// Les arrets de travail du magasin des imprevus (`imprevus/AAAA-MM.json`, genre `arret`), dans l'ordre des mois du
/// magasin, chacun complet.
pub(crate) fn tous_les_arrets(ctx: &Contexte) -> R<Vec<Value>> {
    Ok(crate::imprevus::tous(&ctx.racine).map_err(refus_magasin)?.into_iter().filter(crate::imprevus::est_un_arret).collect())
}

/// `signalement.nomenclatures()`.
pub(crate) fn nomenclatures(ctx: &Contexte) -> R<std::sync::Arc<Value>> {
    let _ = ctx;
    json_en_cache(&dossier_du_code().join("arrets-base.json")).map_err(plain)
}

/// Les codes de `nomenclatures()[quoi]["codes"]`.
fn codes(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let section = n.get(quoi).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(quoi)))?;
    let liste = section.get("codes").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("codes")))?;
    Ok(items(liste)?.into_iter().map(|c| c.into_owned()).collect())
}

/// `signalement.motifs_arret()` et `motifs_reprise()` : les codes saisissables.
pub(crate) fn saisissables(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for c in codes(n, quoi)? {
        if vrai(get(&c, "saisissable")?) {
            out.push(c);
        }
    }
    Ok(out)
}

/// `Arret.libelle_motif` : `_codes("motifs_arret").get(motif, {}).get("libelle", "?")`.
fn libelle_motif(n: &Value, motif: &Value) -> R<Value> {
    if matches!(motif, Value::Array(_) | Value::Object(_)) {
        return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(motif))));
    }
    let Some(m) = motif.as_str() else { return Ok(json!("?")) };
    let mut trouve: Option<Value> = None;
    for c in codes(n, "motifs_arret")? {
        if get(&c, "code")?.as_str() == Some(m) {
            trouve = Some(c);
        }
    }
    match trouve {
        Some(c) => Ok(getd(&c, "libelle", &json!("?"))?.clone()),
        None => Ok(json!("?")),
    }
}

/// Les envois d'un arret (`arret.envois`), a parcourir.
fn envois(a: &Value) -> R<Vec<std::borrow::Cow<'_, Value>>> {
    match a.get("envois") {
        None => Ok(Vec::new()),
        Some(v) => items(v),
    }
}

/// `operations_arrets.resume`.
pub(crate) fn resume_arret(n: &Value, a: &Value) -> R<Value> {
    let mut signalements = Vec::new();
    for e in envois(a)? {
        if matches!(get(&e, "nature")?.as_str(), Some("04") | Some("05")) {
            signalements.push(e.into_owned());
        }
    }
    let mut reports = Vec::new();
    for e in envois(a)? {
        if get(&e, "nature")?.as_str() == Some("01") {
            reports.push(e.into_owned());
        }
    }
    let motif = champ(a, "motif", Value::Null);
    let libelle = libelle_motif(n, &motif)?;
    let mut mois = Vec::new();
    for r in &reports {
        mois.push(get(r, "mois")?.clone());
    }
    let signale = !signalements.is_empty();
    // UN SIGNALEMENT EST-IL DU ? Pas pour un motif que la DSN ne couvre pas (le deuil d'enfant : `obligations-base.json`,
    // `signalement_arret.sauf_motifs`) ; son obligation est alors l'attestation de salaire hors DSN.
    let signalement_du = crate::obligations::signalement_du(&crate::date::en_texte(&motif))?;
    Ok(json!({
        "salarie": champ(a, "salarie", Value::Null),
        "contrat": champ(a, "contrat_numero", Value::Null),
        "contrat_debut": champ(a, "contrat_debut", Value::Null),
        "motif": motif,
        "motif_libelle": libelle,
        "dernier_jour_travaille": champ(a, "dernier_jour_travaille", Value::Null),
        "fin_previsionnelle": champ(a, "fin_previsionnelle", Value::Null),
        "subrogation": champ(a, "subrogation", Value::Null),
        "subrogation_debut": champ(a, "subrogation_debut", Value::Null),
        "subrogation_fin": champ(a, "subrogation_fin", Value::Null),
        "date_accident": champ(a, "date_accident", Value::Null),
        "reprise": champ(a, "reprise", Value::Null),
        "reprise_motif": champ(a, "reprise_motif", Value::Null),
        "note": champ(a, "note", json!("")),
        "saisi_le": champ(a, "saisi_le", Value::Null),
        "signale": signale,
        "signalement_du": signalement_du,
        "signalements": signalements,
        "reporte_par": mois,
        "etat": if signale { "signale" } else if signalement_du { "a signaler" } else { "sans signalement" },
    }))
}

/// La route `arrets` : les arrets de travail du magasin des imprevus, chacun resume comme `imprevus` le resume
/// (`routes_imprevus::resume`), ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<Value> {
    let tous = tous_les_arrets(ctx)?;
    let n = nomenclatures(ctx)?;
    // LES OBLIGATIONS de chaque arret (`obligations::de_l_arret`, 15/10/2026) : un champ de plus a son resume.
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut a_signaler = Vec::new();
    for a in &tous {
        let mut r = crate::routes_imprevus::resume(ctx, &n, a)?;
        r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?);
        if !r["signale"].as_bool().unwrap_or(false) && r["signalement_du"].as_bool().unwrap_or(true) {
            a_signaler.push(r.clone());
        }
        resumes.push(r);
    }
    let subrogation = n.get("subrogation").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("subrogation")))?;
    Ok(json!({
        "arrets": resumes,
        "a_signaler": a_signaler,
        "motifs": saisissables(&n, "motifs_arret")?,
        "motifs_reprise": saisissables(&n, "motifs_reprise")?,
        "subrogation": subrogation.get("codes").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("codes")))?,
    }))
}

// ----------------------------------------------------------- fins de contrat

/// `calendar.monthrange(an, mo)[1]` pour `_fin_mois`.
fn fin_du_mois(mois: &str) -> R<Jour> {
    let an = entier(&mois.chars().take(4).collect::<String>())?;
    let mo = entier(&mois.chars().skip(5).take(2).collect::<String>())?;
    if !(1..=12).contains(&mo) {
        return Err(erreur("ValueError", crate::date::mois_impossible(mo)));
    }
    if !(1..=9999).contains(&an) {
        return Err(erreur("ValueError", crate::date::annee_impossible(an)));
    }
    let dernier = crate::date::jours_du_mois(an as i32, mo as u32);
    Jour::nouveau(an as i32, mo as u32, dernier).ok_or_else(|| erreur("ValueError", crate::date::jour_impossible(an as i64, mo as i64, dernier)))
}

/// `contrats.nature(cle)` : `KeyError` quand elle est inconnue.
fn nature_de(cle: &Value) -> R<std::sync::Arc<crate::contrats::Nature>> {
    let texte = match cle {
        Value::String(s) => Some(s.clone()),
        v if !vrai(v) => None,
        Value::Array(_) | Value::Object(_) => {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(cle))));
        }
        autre => {
            // Une cle qui n'est pas un texte ne peut etre aucune nature.
            let mut connues: Vec<String> = crate::contrats::natures().map_err(plain)?.iter().map(|n| n.cle.clone()).collect();
            connues.sort();
            return Err(erreur("KeyError", format!("Nature de contrat inconnue : {}. Connues : {}.", py_repr(autre), connues.join(", "))));
        }
    };
    crate::contrats::nature(texte.as_deref()).map_err(|m| erreur("KeyError", m))
}

/// `_ce_qui_manque_au_signalement` : ce qu'un signalement exige et que la
/// saisie ne dit pas encore.
pub(crate) fn ce_qui_manque(ctx: &Contexte, e: &Value) -> R<Vec<Value>> {
    let mut manque = Vec::new();
    let motif = get(e, "motif_rupture")?;
    if !vrai(motif) {
        manque.push(json!(
            "`motif_rupture` : le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par defaut."
        ));
    }
    // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`fctu::notification_exigee`).
    if vrai(motif) && !vrai(get(e, "notification_rupture")?) && crate::fctu::notification_exigee(&py_str(motif)).map_err(plain)? {
        manque.push(json!(format!(
            "`notification_rupture` : la date de notification de la rupture (S21.G00.62.003) est obligatoire pour le motif {} \
(CCH-12, cahier technique DSN p. 275) : l'envoi de la lettre de licenciement ou de rupture, la remise de la demission, la \
notification de la fin de la periode d'essai. Le signalement ne s'ecrit pas sans elle.",
            py_str(motif)
        )));
    }
    let preavis = ou(get(e, "preavis")?, objet_vide());
    if !vrai(get(preavis, "type")?) {
        let requis = |k: &str| e.get(k).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(k)));
        let (debut, fin) = (d(requis("debut")?).map_err(|m| erreur("ValueError", m))?, d(requis("fin")?).map_err(|m| erreur("ValueError", m))?);
        let anciennete = crate::permanents::anciennete_mois(&debut, &fin);
        let motif_texte = if vrai(motif) { Some(py_str(motif)) } else { None };
        let cdd = !nature_de(get(e, "nature")?)?.duree_indeterminee;
        let duree = crate::permanents::preavis_mois(anciennete, motif_texte.as_deref(), cdd).map_err(plain)?;
        let regle = crate::permanents::regle_du_preavis(motif_texte.as_deref(), cdd).map_err(plain)?;
        let mut dit = match (duree, &regle) {
            (Some(m), _) if m != Decimal::ZERO => format!("La loi en donne {} mois (L1234-1).", m),
            (Some(_), _) => format!(
                "La loi ne donne aucun preavis pour ce motif : {}",
                crate::permanents::raison_sans_preavis(motif_texte.as_deref(), cdd).map_err(plain)?.unwrap_or_default()
            ),
            (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
            (None, None) => "Sous six mois d'anciennete, la loi renvoie a la convention ou aux usages (L1234-1) : elle ne dit pas de duree.".to_string(),
        };
        // Ce que dit la convention, a cote, par le moteur (le calcul de `paie preavis`) ; pas pour un motif sans preavis
        // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
        if duree != Some(Decimal::ZERO) && regle.is_none() {
            let tout = crate::solde::duree_du_preavis(ctx, e, anciennete, e.get("motif_rupture").and_then(Value::as_str)).map_err(plain)?;
            if let (Some(conv), true) = (tout.convention, tout.qui != "la loi (L1234-1)") {
                dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", tout.qui, conv));
            } else if tout.mois.is_none() {
                if let Some(pourquoi) = tout.pourquoi.as_ref().filter(|p| !p.is_empty()) {
                    dit.push_str(&format!(" Et {}.", pourquoi));
                }
            }
        }
        manque.push(json!(format!(
            "`preavis` : {{\"type\": \"...\", \"debut\": \"...\", \"fin\": \"...\"}}. Le bloc S21.G00.63 est OBLIGATOIRE dans un \
             signalement de fin de contrat (cardinalite 1,*) : meme sans preavis, il se declare, avec le type << 90 - pas de clause de \
             preavis applicable >>. {}",
            dit
        )));
    } else if crate::fctu::preavis_non_effectue(&py_str(get(preavis, "type")?)).map_err(plain)? && !vrai(get(e, "dernier_jour_paye")?) {
        manque.push(json!(
            "`dernier_jour_paye` : sur un preavis non effectue, le dernier jour travaille et paye au salaire habituel \
(S21.G00.62.006) n'est pas la fin du contrat, il precede le debut du preavis (CCH-12, cahier technique DSN p. 275). La \
mensuelle et le signalement le declarent tous deux."
        ));
    }
    Ok(manque)
}

/// `operations_permanents.fins_a_signaler`.
fn fins_a_signaler(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !chemin.exists() {
        return Ok(Vec::new());
    }
    let saisie = lire_json(&chemin)?;
    let fin_mois = fin_du_mois(mois)?;
    let engagements = getd(&saisie, "engagements", &Value::Null)?;
    let liste = if vrai(engagements) { items(engagements)? } else { Vec::new() };
    let mut out = Vec::new();
    let journal = crate::etats::charger(&ctx.racine)?;
    for e in liste {
        let e: &Value = &e;
        let nat = nature_de(get(e, "nature")?)?;
        // La regle est celle de la nature et du motif (`Nature::signalement_fctu`) : un CDDU rompu avant son terme se signale.
        let motif = e.get("motif_rupture").filter(|m| vrai(m)).map(py_str);
        if !nat.signalement_fctu(motif.as_deref()) || !vrai(get(e, "fin")?) {
            continue;
        }
        let debut_du_mois = d(&json!(format!("{}-01", mois))).map_err(|m| erreur("ValueError", m))?;
        let fin = d(&e["fin"]).map_err(|m| erreur("ValueError", m))?;
        if !(debut_du_mois <= fin && fin <= fin_mois) {
            continue;
        }
        let salarie = e.get("salarie").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("salarie")))?;
        // La cle de la fiche : celle que porte le nom du fichier du signalement (`fin-contrat-signaler`).
        let cle = ctx.salarie(&py_str(salarie)).ok().and_then(|f| f.get("_cle").map(py_str));
        out.push(json!({
            "salarie": salarie,
            "numero": get(e, "numero")?,
            "fin": e["fin"],
            "nature": nat.libelle,
            "motif_rupture": get(e, "motif_rupture")?,
            "preavis": get(e, "preavis")?,
            "a_saisir": ce_qui_manque(ctx, e)?,
            // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`, 15/10/2026) : un champ de plus.
            "obligation": crate::obligations::de_la_fin(ctx, cle.as_deref(), fin, &journal, ctx.aujourdhui)?,
        }));
    }
    Ok(out)
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = charge.get("mois").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?;
    let fins = fins_a_signaler(ctx, &py_str(brut))?;
    Ok(json!({"mois": brut, "fins": fins}))
}

// ------------------------------------------------------------ regularisations

const CHAMPS_CORRECTION: [&str; 11] =
    ["mois", "numero", "salarie", "debut", "motif", "engagement", "decidee_le", "qui", "preparee_par", "portee_par", "annulee_le"];

/// `Correction.etat`.
fn etat_de_la_correction(c: &Value) -> &'static str {
    if vrai(&c["annulee_le"]) {
        "annulee"
    } else if vrai(&c["portee_par"]) {
        "portee"
    } else if vrai(&c["preparee_par"]) {
        "preparee"
    } else {
        "en attente"
    }
}

/// `a == b` pour deux cles de dictionnaire Python : `1 == 1.0 == True`.
fn memes_cles(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (x, y) => match (nombre_py(x), nombre_py(y)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

fn nombre_py(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(*b as i64 as f64),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn nom_python(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        autre => nom_type(autre),
    }
}

/// `a < b` pour deux cles : deux textes, ou deux nombres ; le reste est le
/// `TypeError` de Python.
fn avant(a: &Value, b: &Value) -> R<bool> {
    match (a, b) {
        (Value::String(x), Value::String(y)) => Ok(x < y),
        (x, y) => match (nombre_py(x), nombre_py(y)) {
            (Some(x), Some(y)) => Ok(x < y),
            _ => Err(erreur("TypeError", crate::fichiers::incomparables(nom_python(a), nom_python(b)))),
        },
    }
}

/// `sorted(out.items())` : le tri de Python (une course montante ou
/// descendante, puis l'insertion dichotomique des suivants), pour que le
/// premier couple de cles qui ne se compare pas soit celui que Python nomme
/// dans son `TypeError`.
fn trier_comme_python(g: &mut Vec<(Value, Vec<Value>)>) -> R<()> {
    let n = g.len();
    if n < 2 {
        return Ok(());
    }
    let mut k = 2;
    if avant(&g[1].0, &g[0].0)? {
        while k < n && avant(&g[k].0, &g[k - 1].0)? {
            k += 1;
        }
        g[..k].reverse();
    } else {
        while k < n && !avant(&g[k].0, &g[k - 1].0)? {
            k += 1;
        }
    }
    for debut in k..n {
        let (mut l, mut r) = (0, debut);
        while l < r {
            let p = l + ((r - l) >> 1);
            if avant(&g[debut].0, &g[p].0)? {
                r = p;
            } else {
                l = p + 1;
            }
        }
        let pivot = g.remove(debut);
        g.insert(l, pivot);
    }
    Ok(())
}

/// `operations_regularisation.corrections_par_mois` : par mois d'origine, les
/// corrections et leur etat.
fn corrections_par_mois(ctx: &Contexte) -> R<Vec<Value>> {
    let dossier = ctx.racine.join("regularisations");
    let mut toutes: Vec<Value> = Vec::new();
    if dossier.exists() {
        for (_, chemin) in fichiers_de_mois(&dossier) {
            let contenu = lire_json(&chemin)?;
            let liste = getd(&contenu, "corrections", &LISTE_VIDE)?;
            for c in items(liste)? {
                construire("Correction", &c, &CHAMPS_CORRECTION, 8)?;
                let mut m = Map::new();
                for k in CHAMPS_CORRECTION {
                    m.insert(k.to_string(), champ(&c, k, Value::Null));
                }
                let etat = etat_de_la_correction(&Value::Object(m.clone()));
                m.insert("etat".into(), json!(etat));
                toutes.push(Value::Object(m));
            }
        }
    }
    // `out.setdefault(c.mois, []).append(...)`, puis `sorted(out.items())`.
    let mut groupes: Vec<(Value, Vec<Value>)> = Vec::new();
    for c in toutes {
        let mois = c["mois"].clone();
        if matches!(mois, Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", crate::fichiers::pas_une_cle(nom_type(&mois))));
        }
        match groupes.iter_mut().find(|(m, _)| memes_cles(m, &mois)) {
            Some((_, cs)) => cs.push(c),
            None => groupes.push((mois, vec![c])),
        }
    }
    trier_comme_python(&mut groupes)?;
    Ok(groupes.into_iter().map(|(m, cs)| json!({"mois": m, "corrections": cs})).collect())
}

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Value> {
    let e = m.dsn_du_mois(mois)?;
    let echeance = crate::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let passee = ctx.aujourdhui > limite;
    let deposee = vrai(&e);
    let drapeau = |k: &str| -> R<bool> { Ok(deposee && vrai(get(&e, k)?)) };
    let (partie_passee, refusee) = (drapeau("passee")?, drapeau("refusee")?);
    // `_remplacement.limite` : la veille de l'echeance (reportee), a minuit.
    let veille = echeance.veille().map_err(plain)?;
    let mut regularisable = false;
    let raison = if !deposee {
        let retard = if passee { format!(" (en retard : `paie mois paies/{}.json --forcer`)", mois) } else { String::new() };
        format!("Aucune DSN reelle n'est partie pour {} : ce mois se saisit et se depose normalement{}.", mois, retard)
    } else if refusee {
        format!("La DSN de {} a ete REFUSEE : elle se corrige et se redepose, il n'y a rien a regulariser.", mois)
    } else if ctx.aujourdhui <= veille {
        format!(
            "La DSN de {} est partie et l'annule-et-remplace est admis jusqu'au {} a minuit : modifier le mois et refaire la DSN, \
             elle partira en annule-et-remplace.",
            mois,
            veille.jj_mm_aaaa()
        )
    } else {
        regularisable = true;
        format!(
            "La DSN de {} est deposee et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance du {}) : \
             une correction part dans la DSN du mois courant, datee de {}.",
            mois,
            veille.jj_mm_aaaa(),
            limite.jj_mm_aaaa(),
            mois
        )
    };
    Ok(json!({
        "mois": mois, "deposee": deposee, "passee": partie_passee, "refusee": refusee,
        "echeance": limite.iso(), "echeance_passee": passee, "echeance_depot": echeance.en_json(),
        "mois_courant": ctx.aujourdhui.mois_iso(),
        "regularisable": regularisable, "raison": raison,
    }))
}

/// `mois_a_corriger` : les mois qui appellent une decision.
fn mois_a_corriger(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let mut m = Moteur::nouveau(o, ctx);
    let mut etats = Vec::new();
    for (mois, _) in fichiers_de_mois(&ctx.racine.join("paies")) {
        etats.push(etat_mois(&mut m, ctx, &mois)?);
    }
    let choisis = |f: &dyn Fn(&Value) -> bool| -> Vec<Value> { etats.iter().filter(|e| f(e)).cloned().collect() };
    Ok(json!({
        "regularisables": choisis(&|e| e["regularisable"] == json!(true)),
        "en_retard": choisis(&|e| e["echeance_passee"] == json!(true) && e["deposee"] == json!(false)),
    }))
}

/// La route `regularisations`.
fn regularisations(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let mois = mois_a_corriger(o, ctx)?;
    // `coffre.ouverture_exigee` : le refus du coffre garde sa classe et sa phrase.
    let pose = crate::coffre::Coffre::du_poste(&ctx.racine, "").ouverture_exigee().map_err(vers_route)?;
    Ok(json!({"mois": mois, "mot_de_passe_pose": pose, "corrections": corrections_par_mois(ctx)?}))
}
