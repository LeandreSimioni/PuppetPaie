//! Les apercus de `paie route` : ce que la page montre AVANT qu'on agisse.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et ce qu'il appelait, porte a
//! l'identique : meme JSON (l'enveloppe de `web.py` comprise), memes refus sous
//! la meme classe d'exception. Aucun sous-processus, aucun reseau, AUCUNE ecriture.
//!
//! - `documents-liste` : `operations_documents.documents_du_mois`.
//! - `bilan-repetition` : `operations_declarations.bilan_de_la_repetition`, SANS le
//!   releve des retours : quand le compte rendu du test est « attendu », la fonction
//!   Python va le chercher chez l'organisme (reseau) et l'archive (ecriture) avant
//!   de relire le journal. Ce cas reste en Python (`web.py` le reprend a la main) ;
//!   cette route rend ce que la fonction relirait dans tous les autres cas, et, pour
//!   « attendu », ce qu'elle lit AVANT d'aller chercher (`incident` vide).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use crate::etats;
use socle::valeur::nom_type;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["documents-liste", "bilan-repetition", "depot-apercu"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "documents-liste" => documents_liste(ctx, charge),
        "bilan-repetition" => bilan_repetition(ctx, charge),
        "depot-apercu" => depot_apercu(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

/// `charge["cle"]` : `KeyError` si elle manque.
fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

/// L'erreur du disque, sous le nom que Python lui donne.
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match (e.kind(), e.raw_os_error()) {
        (std::io::ErrorKind::NotFound, _) => "FileNotFoundError",
        (std::io::ErrorKind::PermissionDenied, _) => "PermissionError",
        (_, Some(20)) => "NotADirectoryError",
        (_, Some(21)) => "IsADirectoryError",
        _ => "OSError",
    };
    erreur(classe, socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------------------------ documents-liste

/// `ctx.racine / "sorties" / mois` : `mois` doit etre un texte (`TypeError` sinon,
/// avec la phrase de `PosixPath.__truediv__`).
fn sous_sorties(ctx: &Contexte, mois: &Value) -> R<std::path::PathBuf> {
    match mois {
        Value::String(m) => Ok(ctx.racine.join("sorties").join(m)),
        autre => Err(erreur(
            "TypeError",
            format!("'mois' : {}", socle::fichiers::attendu("un texte", nom_type(autre))),
        )),
    }
}

/// `sorted(d.glob("*.pdf"))` : les noms (sensible a la casse, fichiers caches compris),
/// en ordre de points de code.
fn pdf_de(d: &Path) -> R<Vec<String>> {
    let mut noms = Vec::new();
    // `Path.glob` ne leve rien pour ce qui n'est pas un dossier ni pour un dossier
    // interdit : elle ne rend rien.
    let lecture = match std::fs::read_dir(d) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(20) => return Ok(noms),
        Err(e) => return Err(erreur_io(&e, d)),
    };
    for e in lecture {
        let e = e.map_err(|e| erreur_io(&e, d))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".pdf") {
            noms.push(n);
        }
    }
    noms.sort();
    Ok(noms)
}

/// `documents_du_mois` : les fichiers du mois a ouvrir ou a remettre a la main.
pub(crate) fn documents_du_mois(ctx: &Contexte, mois: &Value, quoi: &Value) -> R<Value> {
    let base = sous_sorties(ctx, mois)?;
    let envoi = base.join("envoi");
    // `{...}.get(quoi)` : un texte seulement se trouve ; une liste ou un dictionnaire ne se hache pas.
    let prefixes: Option<&[&str]> = match quoi {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(q) if q == "contrats" => Some(&["Contrat_", "Contrats_"]),
        Value::String(q) if q == "documents" => Some(&["Bulletin_", "CertificatCS_", "AER_"]),
        _ => None,
    };
    let mut out = Vec::new();
    for (d, est_base) in [(&envoi, false), (&base, true)] {
        if !d.exists() {
            continue;
        }
        for nom in pdf_de(d)? {
            if let Some(p) = prefixes {
                if !p.iter().any(|x| nom.starts_with(x)) {
                    continue;
                }
            }
            if est_base && vrai(quoi) {
                continue; // les fichiers groupes du mois : pas ici
            }
            let chemin = d.join(&nom);
            let st = std::fs::metadata(&chemin).map_err(|e| erreur_io(&e, &chemin))?;
            let stem = nom.strip_suffix(".pdf").filter(|s| !s.is_empty()).unwrap_or(&nom);
            let morceaux: Vec<&str> = stem.split('_').collect();
            let salarie = if morceaux.len() > 2 { morceaux[1] } else { "" };
            out.push(json!({"nom": nom, "octets": st.len(), "salarie": salarie}));
        }
    }
    Ok(Value::Array(out))
}

fn documents_liste(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = cle(charge, "mois")?;
    let quoi = charge.get("quoi").unwrap_or(&Value::Null);
    Ok(json!({"mois": mois, "fichiers": documents_du_mois(ctx, mois, quoi)?}))
}

// ------------------------------------------------------------------ les chemins de Python

/// `str(PurePosixPath(t))` : les `//` et les `.` intermediaires s'en vont, un `/`
/// de tete reste (deux, s'il y en a exactement deux).
pub(crate) fn normaliser(t: &str) -> String {
    let tete = if t.starts_with("//") && !t.starts_with("///") {
        "//"
    } else if t.starts_with('/') {
        "/"
    } else {
        ""
    };
    let morceaux: Vec<&str> = t.split('/').filter(|m| !m.is_empty() && *m != ".").collect();
    let corps = morceaux.join("/");
    if tete.is_empty() && corps.is_empty() {
        return ".".to_string();
    }
    format!("{}{}", tete, corps)
}

/// `PurePosixPath(t).name`.
pub(crate) fn nom_du_chemin(t: &str) -> String {
    let n = normaliser(t);
    if n == "." || n.chars().all(|c| c == '/') {
        return String::new();
    }
    n.rsplit('/').next().unwrap_or("").to_string()
}

fn existe(t: &str) -> bool {
    !t.contains('\0') && Path::new(t).exists()
}

/// `contexte.resoudre(chemin, racine)` : un chemin tape a la main, tel quel
/// s'il existe, sinon sous les donnees, un mois nu designant sa saisie. Rend
/// le texte du chemin, ecrit comme `str(Path)`.
pub(crate) fn resoudre(racine: &Path, chemin: &Value) -> R<String> {
    let vide = match chemin {
        Value::Null => true,
        Value::String(t) => t.chars().all(socle::texte::blanc),
        _ => false,
    };
    if vide {
        return Err(erreur("FileNotFoundError", "Aucun fichier indique."));
    }
    let Value::String(t) = chemin else {
        return Err(erreur("TypeError", format!("expected str, bytes or os.PathLike object, not {}", nom_type(chemin))));
    };
    let p = normaliser(t);
    if existe(&p) || p.starts_with('/') {
        return Ok(p);
    }
    let base = normaliser(&racine.display().to_string());
    let candidat = normaliser(&format!("{}/{}", base, p));
    if existe(&candidat) {
        return Ok(candidat);
    }
    // Un mois nu (`\d{4}-\d{2}`) designe sa saisie.
    let c: Vec<char> = t.chars().collect();
    if c.len() == 7
        && c[..4].iter().all(|x| crate::retours::est_chiffre(*x))
        && c[4] == '-'
        && c[5..].iter().all(|x| crate::retours::est_chiffre(*x))
    {
        let saisie = normaliser(&format!("{}/{}/{}.json", base, engagement::stock::DOSSIER, t));
        if existe(&saisie) {
            return Ok(saisie);
        }
    }
    Ok(p)
}

/// `journal.empreinte(Path)` : `read_bytes`, sous le nom d'erreur de Python.
fn empreinte_du_fichier(chemin: &str) -> R<String> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(crate::journal::empreinte(&octets))
}

// ------------------------------------------------------------------ bilan-repetition

pub(crate) fn bilan_repetition(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let fichier = cle(charge, "fichier")?;
    let chemin = resoudre(&ctx.racine, fichier)?;
    let empreinte = empreinte_du_fichier(&chemin)?;
    let nom = nom_du_chemin(&chemin);
    let mut m = crate::routes_mois::Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let Some(quoi) = quoi.as_str() else {
        return Err(Echec { classe: None, message: "demande illisible : il faut `quoi`".to_string() });
    };
    let mut r = etats::repetition(&lignes, quoi, &empreinte, &nom)?;
    r["incident"] = json!("");
    Ok(r)
}

// ------------------------------------------------------------------ depot-apercu, la DPAE

/// `Path.read_bytes()`.
pub(crate) fn lire_octets(chemin: &str) -> R<Vec<u8>> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    std::fs::read(p).map_err(|e| erreur_io(&e, p))
}

/// `str.isdigit()` sur un caractere du latin-1 : les chiffres, et `²`, `³`, `¹`.
fn chiffre_py(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
}

/// `operations_declarations._iso_depuis_jjmmaaaa`.
fn iso_depuis_jjmmaaaa(v: &str) -> String {
    let t: Vec<char> = v.trim_matches(socle::texte::blanc).chars().collect();
    if t.len() != 8 || !t.iter().all(|c| chiffre_py(*c)) {
        return String::new();
    }
    let s = |a: usize, b: usize| t[a..b].iter().collect::<String>();
    format!("{}-{}-{}", s(4, 8), s(2, 4), s(0, 2))
}

/// `apercu_depot_dpae` : ce que le fichier DPAE declare, enregistrement par enregistrement.
pub(crate) fn apercu_depot_dpae(ctx: &Contexte, fichier: &Value) -> R<Value> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let enrs = declarations::dpae::relire(&texte);
    let mut m = crate::routes_mois::Moteur::nouveau(ctx);
    let lignes_journal = m.lignes()?;
    let faites = etats::embauches_par_depot(&lignes_journal)?;
    let mut par_nir: HashMap<String, (&String, &Value)> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        let nir = etats::get(s, "nir")?;
        if !vrai(nir) {
            continue;
        }
        match nir {
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            Value::String(t) => {
                par_nir.insert(t.clone(), (cle, s));
            }
            _ => {}
        }
    }
    let vide = json!({});
    let mut lignes = Vec::new();
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let trouve = par_nir.get(champ("nir"));
        let (cle, s) = match trouve {
            Some((c, s)) => (Some(*c), *s),
            None => (None, &vide),
        };
        let debut = iso_depuis_jjmmaaaa(champ("date_embauche"));
        let nom_s = etats::getd(s, "nom", etats::texte_vide())?;
        let prenom = etats::ou(etats::get(s, "prenom_affichage")?, etats::getd(s, "prenom", etats::texte_vide())?);
        let nom = format!("{} {}", socle::valeur::valeur_en_texte(nom_s), socle::valeur::valeur_en_texte(prenom));
        let nom = nom.trim_matches(socle::texte::blanc);
        let nom = if nom.is_empty() { "salarie inconnu de la configuration" } else { nom };
        let type_brut = champ("type_contrat").trim_matches(socle::texte::blanc);
        let type_contrat = match type_brut {
            "1" => "CDD".to_string(),
            "2" => "CDI".to_string(),
            "3" => "CTT".to_string(),
            _ => format!("type {}", champ("type_contrat")),
        };
        let cle_json = cle.map_or(Value::Null, |c| json!(c));
        let deja = faites.contient(&[cle_json, json!(debut)])?;
        lignes.push(json!({
            "salarie": cle.map_or("?", |c| c.as_str()),
            "nom": nom,
            "debut": debut,
            "fin": iso_depuis_jjmmaaaa(champ("date_fin")),
            "type_contrat": type_contrat,
            "heure": champ("heure_embauche"),
            "deja_declaree": deja,
        }));
    }
    let debuts: Vec<String> = lignes.iter().map(|l| l["debut"].as_str().unwrap_or("").to_string()).collect();
    let mois: std::collections::BTreeSet<String> =
        debuts.iter().filter(|d| !d.is_empty()).map(|d| d.chars().take(7).collect()).collect();
    let mois: Vec<String> = mois.into_iter().collect();
    let empreinte = crate::journal::empreinte(&contenu);
    let nom_fichier = nom_du_chemin(&chemin);
    let deja_depose = etats::deja_depose(&lignes_journal, "dpae", &empreinte)?;
    let repetition = etats::repetition(&lignes_journal, "dpae", &empreinte, &nom_fichier)?;
    Ok(json!({
        "fichier": chemin, "nom": nom_fichier,
        "enregistrements": enrs.len(), "octets": contenu.len(),
        "reel": crate::depot::est_reel(&contenu),
        "indicateur": crate::depot::indicateur(&contenu),
        "mois": mois.first().cloned().unwrap_or_default(),
        "tous_les_mois": mois,
        "embauches": lignes,
        "deja_depose": deja_depose,
        "repetition": repetition,
    }))
}

fn depot_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let quoi = cle(charge, "quoi")?;
    let fichier = cle(charge, "fichier")?;
    if etats::est(quoi, "dsn") {
        return crate::apercu_dsn::apercu_depot_dsn(ctx, fichier);
    }
    apercu_depot_dpae(ctx, fichier)
}
