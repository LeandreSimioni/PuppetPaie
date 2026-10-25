//! La fiche recapitulative du dossier d'un salarie (`operations_dossier._recapitulatif`) : « ses
//! contrats (heures, cachets, brut) et ses paies (brut, net, PAS) sur la periode », les deux etats
//! de l'onglet Exports reduits a lui (`operations_etats.heures_et_cachets` et
//! `recapitulatif_annuel`), et leur mise en page PDF (`tableaux.description_pdf`).
//!
//! Ce qui se porte ici est ce que ces deux etats lisent : la periode et les mois disponibles
//! (`operations_exports.mois_disponibles`), les lignes du livre de paie d'un mois saisi (recalculees)
//! ou d'un mois declare avant PuppetPaie (`historique/paies.json`), l'AEM attestee
//! (`historique/engagements.json`). Un etat qui refuse en `ErreurOperation` (rien sur la periode,
//! un mois qui ne se calcule pas) est LAISSE DE COTE ; tout autre refus remonte, comme en Python.
//!
//! Ecart voulu : un mois saisi est un fichier `paies/AAAA-MM.json`, rien d'autre (le Python prenait
//! aussi `2026-09.calcul.json` pour un mois « 2026-09.calcul »).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use crate::bulletin::FORMAT;
use crate::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use crate::engagement;
use socle::valeur::valeur_en_texte;
use socle::fichiers::vrai;
use socle::num::{eur2, Decimal};
use crate::routes_ouverture::{refus_du_calcul, Refus};

type R<T> = Result<T, Refus>;

const SAISIE: &str = "saisie";
const DSN: &str = "DSN déposée";
const AEM: &str = "AEM attestée";

const PAGE_LARGEUR: f64 = 841.89;
const PAGE_HAUTEUR: f64 = 595.28;
const ROUGE: &str = "#B22222";
const ENCRE: &str = "#2C3E50";
const CHASSE: f64 = 3.9;
const CHASSE_ENTETE: f64 = 4.2;
const MARGE_CELLULE: f64 = 3.0;
const PLAFOND_TEXTE: f64 = 40.0;

fn erreur_operation(message: impl Into<String>) -> Refus {
    Refus { classe: Some("ErreurOperation"), message: message.into() }
}

fn avec_classe(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

/// Un refus qu'un `except ErreurOperation` de Python attrape.
fn est_erreur_operation(r: &Refus) -> bool {
    matches!(r.classe, None | Some("ErreurOperation") | Some("SaisieIncomplete"))
}

fn copie(r: &Refus) -> Refus {
    Refus { classe: r.classe, message: r.message.clone() }
}

// ------------------------------------------------------------------- le tableau

#[derive(Clone, Copy, PartialEq)]
enum Genre {
    Montant,
    Nombre,
    Taux,
    Texte,
}

struct Colonne {
    cle: String,
    libelle: String,
    genre: Genre,
}

fn colonne(cle: &str, libelle: &str, genre: Genre) -> Colonne {
    Colonne { cle: cle.to_string(), libelle: libelle.to_string(), genre }
}

impl Colonne {
    fn totalisee(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre)
    }
    fn a_droite(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre | Genre::Taux)
    }
}

/// Une valeur de cellule : un `Decimal`, un nombre JSON tel que Python l'ecrit, un texte.
#[derive(Clone, Debug)]
enum Val {
    Dec(Decimal),
    Num(String),
    Txt(String),
}

type Ligne = HashMap<String, Option<Val>>;

struct Section {
    libelle: String,
    lignes: Vec<Ligne>,
    total: Option<Ligne>,
}

struct Tableau {
    titre: String,
    periode: String,
    colonnes: Vec<Colonne>,
    sections: Vec<Section>,
    total: Option<Ligne>,
    sources: Vec<String>,
}

fn decimal_de(v: &Val) -> R<Decimal> {
    let t = match v {
        Val::Dec(d) => return Ok(*d),
        Val::Num(s) | Val::Txt(s) => s,
    };
    crate::apercu_dsn::decimal_py(t).ok_or_else(|| avec_classe("InvalidOperation", socle::num::illisible(t)))
}

/// `_somme` : la somme de ce qui est DECLARE, ou `None` quand aucune ligne ne l'est.
fn somme(lignes: &[&Ligne], cle: &str) -> R<Option<Val>> {
    let mut vus: Vec<&Val> = Vec::new();
    for l in lignes {
        match l.get(cle) {
            Some(Some(Val::Txt(t))) if t.is_empty() => {}
            Some(Some(v)) => vus.push(v),
            _ => {}
        }
    }
    if vus.is_empty() {
        return Ok(None);
    }
    let mut total = Decimal::ZERO;
    for v in vus {
        total += decimal_de(v)?;
    }
    Ok(Some(Val::Dec(total)))
}

/// `tableaux.totaliser` : le sous-total de chaque section et le total general.
fn totaliser(t: &mut Tableau) -> R<()> {
    let porte = t.colonnes.iter().find(|c| c.genre == Genre::Texte).map(|c| c.cle.clone());
    let ligne = |colonnes: &[Colonne], lignes: &[&Ligne], libelle: &str| -> R<Ligne> {
        let mut d: Ligne = HashMap::new();
        for c in colonnes.iter().filter(|c| c.totalisee()) {
            d.insert(c.cle.clone(), somme(lignes, &c.cle)?);
        }
        if let Some(p) = &porte {
            d.insert(p.clone(), Some(Val::Txt(libelle.to_string())));
        }
        Ok(d)
    };
    for s in t.sections.iter_mut() {
        s.total = if s.libelle.is_empty() {
            None
        } else {
            let l: Vec<&Ligne> = s.lignes.iter().collect();
            Some(ligne(&t.colonnes, &l, &format!("Total {}", s.libelle))?)
        };
    }
    let toutes: Vec<&Ligne> = t.sections.iter().flat_map(|s| s.lignes.iter()).collect();
    t.total = Some(ligne(&t.colonnes, &toutes, "TOTAL GÉNÉRAL")?);
    Ok(())
}

/// `tableaux.texte` : une cellule en texte francais.
fn texte_cellule(v: &Option<Val>, c: &Colonne) -> String {
    match v {
        None => String::new(),
        Some(Val::Dec(d)) if c.a_droite() => d.to_string().replace('.', ","),
        Some(Val::Dec(d)) => d.to_string(),
        Some(Val::Num(s)) if c.a_droite() => s.replace('.', ","),
        Some(Val::Num(s)) | Some(Val::Txt(s)) => s.clone(),
    }
}

/// (cellules, genre) dans l'ordre d'ecriture : lignes, sous-totaux, total.
fn rangees(t: &Tableau) -> Vec<(&Ligne, &'static str)> {
    let plusieurs = t.sections.len() > 1;
    let mut out: Vec<(&Ligne, &'static str)> = Vec::new();
    for s in &t.sections {
        for l in &s.lignes {
            out.push((l, ""));
        }
        if plusieurs {
            if let Some(tot) = &s.total {
                out.push((tot, "total"));
            }
        }
    }
    if let Some(tot) = &t.total {
        out.push((tot, "general"));
    }
    out
}

// ------------------------------------------------------------------ le PDF

/// `tableaux._fractions` : la largeur de chaque colonne, en fraction de la largeur utile.
pub(crate) fn fractions(entetes: &[(String, bool)], rangees: &[(Vec<String>, &str)], utile: f64) -> Vec<f64> {
    let n = entetes.len();
    let mut plus_long = vec![0.0f64; n];
    let mut plus_long_mot = vec![0.0f64; n];
    let mut mot_entete = vec![0.0f64; n];
    for (i, (libelle, _)) in entetes.iter().enumerate() {
        mot_entete[i] = libelle.split(socle::texte::blanc).filter(|m| !m.is_empty()).map(|m| m.chars().count()).max().unwrap_or(0) as f64;
    }
    let gonfle = |genre: &str| match genre {
        "total" => 1.1,
        "general" => 1.3,
        _ => 1.0,
    };
    for (cellules, genre) in rangees {
        for (i, c) in cellules.iter().take(n).enumerate() {
            plus_long[i] = plus_long[i].max(c.chars().count() as f64 * gonfle(genre));
            for m in c.split(socle::texte::blanc).filter(|m| !m.is_empty()) {
                plus_long_mot[i] = plus_long_mot[i].max(m.chars().count() as f64 * gonfle(genre));
            }
        }
    }
    let texte: Vec<bool> = entetes.iter().map(|(_, a_droite)| !a_droite).collect();
    let besoins = |plafond: f64| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let lg = if texte[i] { plus_long[i].min(plafond).max(plus_long_mot[i]) } else { plus_long[i] };
                (lg * CHASSE).max(mot_entete[i] * CHASSE_ENTETE) + 2.0 * MARGE_CELLULE
            })
            .collect()
    };
    let somme_de = |b: &[f64]| b.iter().fold(0.0, |a, x| a + x);
    let mut plafond = PLAFOND_TEXTE;
    let mut b = besoins(plafond);
    while somme_de(&b) > utile && plafond > 8.0 {
        plafond -= 2.0;
        b = besoins(plafond);
    }
    let total = somme_de(&b);
    b.iter().map(|x| x / total).collect()
}

fn filets(couleur: &str, epaisseur: f64) -> Value {
    json!({"couleur": couleur, "epaisseur": epaisseur})
}

/// `tableaux.description_pdf` + `_description_tableau`.
fn description(t: &Tableau, nom: &str) -> Value {
    let rangs = rangees(t);
    let mut rows: Vec<(Vec<String>, &str)> = Vec::new();
    for (d, genre) in rangs {
        let mut cel = Vec::new();
        for c in &t.colonnes {
            let v = d.get(&c.cle);
            let manque = matches!(v, Some(None)) && c.genre == Genre::Montant && genre.is_empty();
            cel.push(if manque { "—".to_string() } else { texte_cellule(v.unwrap_or(&None), c) });
        }
        rows.push((cel, genre));
    }
    let entetes: Vec<(String, bool)> = t.colonnes.iter().map(|c| (c.libelle.clone(), c.a_droite())).collect();
    let marges = json!({"haut": 40, "droite": 34, "bas": 40, "gauche": 34});
    let fr = fractions(&entetes, &rows, PAGE_LARGEUR - 68.0);
    let colonnes: Vec<Value> = fr
        .iter()
        .zip(&entetes)
        .map(|(f, e)| if e.1 { json!({"largeur": f, "alignement": "droite"}) } else { json!({"largeur": f}) })
        .collect();
    let lignes: Vec<Value> = rows
        .iter()
        .map(|(cellules, genre)| {
            let mut l = Map::new();
            l.insert("cellules".into(), json!(cellules));
            if *genre == "total" {
                l.insert("gras".into(), json!(true));
                l.insert("filet_haut".into(), filets("#999999", 0.5));
                l.insert("filet_bas".into(), filets("#999999", 0.5));
            } else if *genre == "general" {
                l.insert("gras".into(), json!(true));
                l.insert("taille".into(), json!(8));
                l.insert("couleur".into(), json!(ROUGE));
                l.insert("filet_haut".into(), filets("#999999", 0.5));
                l.insert("filet_bas".into(), filets("#999999", 0.5));
            }
            Value::Object(l)
        })
        .collect();
    let mut blocs: Vec<Value> = Vec::new();
    if !nom.is_empty() {
        blocs.push(json!({"type": "paragraphe", "taille": 8.5, "espace_apres": 1, "morceaux": [{"texte": nom, "gras": true}]}));
    }
    blocs.push(json!({"type": "paragraphe", "taille": 11, "couleur": ROUGE, "majuscules": true, "espace_apres": 1,
                      "morceaux": [{"texte": t.titre, "gras": true}]}));
    blocs.push(json!({"type": "paragraphe", "taille": 6.8, "couleur": "#555555", "espace_apres": 6,
                      "morceaux": [{"texte": t.periode}]}));
    blocs.push(json!({
        "type": "tableau", "colonnes": colonnes, "taille": 7, "couleur": ENCRE,
        "marge_cellule": {"haut": 1.6, "droite": MARGE_CELLULE, "bas": 1.6, "gauche": MARGE_CELLULE},
        "filet_lignes": {"couleur": "#EEEEEE", "epaisseur": 0.3},
        "entete": [{"cellules": entetes.iter().map(|e| e.0.clone()).collect::<Vec<_>>(), "gras": true, "taille": 6.3,
                    "couleur": ROUGE, "filet_bas": {"couleur": ROUGE, "epaisseur": 0.7}}],
        "lignes": lignes}));
    for src in &t.sources {
        blocs.push(json!({"type": "paragraphe", "taille": 6, "couleur": "#999999", "espace_avant": 2,
                          "morceaux": [{"texte": format!("Source : {}", src)}]}));
    }
    let pied = format!("{} — {}", t.titre, t.periode);
    json!({
        "format": FORMAT,
        "page": {"largeur": PAGE_LARGEUR, "hauteur": PAGE_HAUTEUR, "marges": marges},
        "police": {"taille": 7, "interligne": 1.25, "couleur": ENCRE},
        "titre": format!("{} — {}", t.titre, t.periode),
        "entete_page": {"gauche": format!("{} — {} (suite)", t.titre, t.periode), "taille": 8.5, "gras": true,
                        "couleur": ENCRE, "distance": 24, "premiere_page": false},
        "pied_page": {"gauche": pied, "droite": "page {page} sur {pages}", "taille": 6.5, "couleur": "#777777", "distance": 20},
        "documents": [{"blocs": blocs}]
    })
}

/// `_nom_compagnie` : le nom de la compagnie, ou rien.
pub(crate) fn nom_compagnie(ctx: &Contexte) -> String {
    // `fiche(ctx)` lit aussi les compteurs : un fichier abime fait rendre « ».
    let compteurs = ctx.racine.join("config").join("compteurs.json");
    if compteurs.exists() && socle::fichiers::lire_json(&compteurs).is_err() {
        return String::new();
    }
    for c in ["nom_affichage", "raison_sociale", "nom"] {
        if let Some(v) = ctx.entreprise.get(c).filter(|v| vrai(v)) {
            return valeur_en_texte(v);
        }
    }
    String::new()
}

// ---------------------------------------------------------------- la periode

/// `operations_etats.Periode` : ce qu'on a demande, les mois qu'on a, et d'ou ils viennent.
struct Periode {
    dispo: BTreeMap<String, &'static str>,
    un_mois: Option<String>,
    annee: Option<String>,
    toutes: bool,
    mois: Vec<String>,
    libelle: String,
}

impl Periode {
    fn saisis(&self) -> Vec<String> {
        self.mois.iter().filter(|m| self.dispo.get(*m) == Some(&SAISIE)).cloned().collect()
    }

    fn contient(&self, m: &str) -> bool {
        if self.toutes {
            return true;
        }
        if let Some(u) = &self.un_mois {
            return m == u;
        }
        m.starts_with(self.annee.as_deref().unwrap_or(""))
    }
}

fn libelle_periode(mois: &[String], un_mois: &Option<String>, annee: &Option<String>, toutes: bool) -> R<String> {
    if toutes {
        let annees: BTreeSet<String> = mois.iter().map(|m| m.chars().take(4).collect()).collect();
        let premiere = annees.iter().next().cloned().unwrap_or_default();
        let derniere = annees.iter().next_back().cloned().unwrap_or_default();
        return Ok(format!(
            "toutes les années ({})",
            if annees.len() > 1 { format!("{} à {}", premiere, derniere) } else { premiere }
        ));
    }
    if let Some(u) = un_mois {
        return Ok(crate::tableaux_export::libelle_mois(u)?);
    }
    Ok(annee.clone().unwrap_or_default())
}

// --------------------------------------------------------------- les lectures

/// Une ligne du livre de paie : ce que le recapitulatif annuel en lit.
struct LigneLivre {
    mois: String,
    salarie: String,
    cle_salarie: String,
    spectacle: String,
    brut: Option<Decimal>,
    cotisations_patronales: Option<Decimal>,
    net_imposable: Option<Decimal>,
    net_a_payer: Option<Decimal>,
    pas: Option<Decimal>,
    cout_employeur: Option<Decimal>,
    source: &'static str,
}

fn requis<'a>(v: &'a Value, k: &str) -> R<&'a Value> {
    v.get(k).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent(k)))
}

/// `_qui` : « NOM Prenom ».
fn qui(fiche: &Value) -> R<String> {
    let prenom = match fiche.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => fiche.get("prenom").map(valeur_en_texte).unwrap_or_default(),
    };
    Ok(format!("{} {}", valeur_en_texte(requis(fiche, "nom")?), prenom))
}

/// `_montant` : un montant declare, ou `None` quand il ne l'a pas ete.
fn montant(v: Option<&Value>) -> R<Option<Decimal>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => {
            let t = valeur_en_texte(x);
            if t.trim_matches(socle::texte::blanc).is_empty() {
                return Ok(None);
            }
            let d = crate::apercu_dsn::decimal_py(&t).ok_or_else(|| avec_classe("InvalidOperation", socle::num::illisible(&t)))?;
            Ok(Some(eur2(d)))
        }
    }
}

/// `un mois saisi` : `paies/AAAA-MM.json`, rien d'autre.
pub(crate) fn mois_saisis(ctx: &Contexte) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(lecture) = std::fs::read_dir(ctx.racine.join("paies")) {
        for e in lecture.flatten() {
            let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
            let Some(stem) = nom.strip_suffix(".json") else { continue };
            let c: Vec<char> = stem.chars().collect();
            if c.len() == 7 && c[4] == '-' && c.iter().enumerate().all(|(i, x)| i == 4 || x.is_ascii_digit()) && e.path().is_file() {
                out.push(stem.to_string());
            }
        }
    }
    out.sort();
    out
}

/// Ce que les deux etats lisent, une fois chacun.
pub(crate) struct Recapitulatifs<'a> {
    ctx: &'a Contexte,
    mois: Option<String>,
    annee: Option<String>,
    toutes: bool,
    saisies: HashMap<String, Result<Rc<Vec<Paie>>, Refus>>,
    declarees: Option<Result<Rc<Vec<Value>>, Refus>>,
    historique: Option<Result<Rc<Vec<Value>>, Refus>>,
    etats: [Option<Result<Rc<Tableau>, Refus>>; 2],
}

impl<'a> Recapitulatifs<'a> {
    pub(crate) fn nouveau(ctx: &'a Contexte, mois: Option<String>, annee: Option<String>, toutes: bool) -> Self {
        Recapitulatifs { ctx, mois, annee, toutes, saisies: HashMap::new(), declarees: None, historique: None, etats: [None, None] }
    }

    /// `charger(ctx, paies/<mois>.json, sans_numeros=True).paies`.
    fn paies(&mut self, mois: &str) -> R<Rc<Vec<Paie>>> {
        if !self.saisies.contains_key(mois) {
            let ctx = self.ctx;
            let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
            let r = (|| -> R<Rc<Vec<Paie>>> {
                let data = engagement::lire_saisie(&chemin)?;
                let s = engagement::charger_donnees(ctx, &data).map_err(|m| refus_du_calcul(ctx, &data, m))?;
                Ok(Rc::new(s.paies))
            })();
            self.saisies.insert(mois.to_string(), r);
        }
        match &self.saisies[mois] {
            Ok(p) => Ok(Rc::clone(p)),
            Err(e) => Err(copie(e)),
        }
    }

    /// Le fichier tel qu'il est, ou un squelette vide : `data.setdefault(cle, [])`.
    fn liste_de(&self, fichier: &str, cle: &str) -> R<Rc<Vec<Value>>> {
        let p = self.ctx.racine.join("historique").join(fichier);
        if !p.exists() {
            return Ok(Rc::new(Vec::new()));
        }
        let data = crate::routes_listes::lire_json(&p)?;
        let Value::Object(o) = data else {
            return Err(avec_classe("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&data))));
        };
        match o.get(cle) {
            None => Ok(Rc::new(Vec::new())),
            Some(Value::Array(a)) => Ok(Rc::new(a.clone())),
            // Un texte ou un dictionnaire se parcourt (ses caracteres, ses cles : des textes) et c'est
            // leur `.get` qui refuse ; vide, rien ne se parcourt.
            Some(Value::String(t)) if t.is_empty() => Ok(Rc::new(Vec::new())),
            Some(Value::Object(m)) if m.is_empty() => Ok(Rc::new(Vec::new())),
            Some(autre @ (Value::String(_) | Value::Object(_))) => Err(avec_classe("AttributeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
            Some(autre) => Err(avec_classe("TypeError", socle::fichiers::attendu("une liste", socle::valeur::nom_type(autre)))),
        }
    }

    /// `lire_paies_declarees(ctx)["paies"]`.
    fn paies_declarees(&mut self) -> R<Rc<Vec<Value>>> {
        if self.declarees.is_none() {
            self.declarees = Some(self.liste_de("paies.json", "paies"));
        }
        match self.declarees.as_ref().unwrap() {
            Ok(v) => Ok(Rc::clone(v)),
            Err(e) => Err(copie(e)),
        }
    }

    /// `lire_historique(ctx)["engagements"]`.
    fn engagements_attestes(&mut self) -> R<Rc<Vec<Value>>> {
        if self.historique.is_none() {
            self.historique = Some(self.liste_de("engagements.json", "engagements"));
        }
        match self.historique.as_ref().unwrap() {
            Ok(v) => Ok(Rc::clone(v)),
            Err(e) => Err(copie(e)),
        }
    }

    /// `mois_disponibles` : tous les mois qu'il y a matiere a exporter, et d'ou ils viennent.
    fn mois_disponibles(&mut self) -> R<BTreeMap<String, &'static str>> {
        let mut out: BTreeMap<String, &'static str> = BTreeMap::new();
        for x in self.paies_declarees()?.iter() {
            let m = x.get("mois").map_or(Value::Null, Clone::clone);
            if vrai(&m) {
                out.insert(valeur_en_texte(&m), DSN);
            }
        }
        for m in mois_saisis(self.ctx) {
            out.insert(m, SAISIE);
        }
        Ok(out)
    }

    /// `Periode(ctx, mois, annee, toutes, obligatoire=...)`.
    fn periode(&mut self, obligatoire: bool) -> R<Periode> {
        let dispo = self.mois_disponibles()?;
        let (un_mois, annee, toutes) = (self.mois.clone(), self.annee.clone(), self.toutes);
        let mois: Vec<String> = if obligatoire {
            // `_mois_pris` : les mois demandes qu'on sait exporter.
            if toutes {
                if dispo.is_empty() {
                    return Err(erreur_operation("Aucune paie, ni saisie ni déclarée."));
                }
                dispo.keys().cloned().collect()
            } else if let Some(m) = &un_mois {
                if !dispo.contains_key(m) {
                    return Err(erreur_operation(format!("Aucune paie en {}.", m)));
                }
                vec![m.clone()]
            } else {
                let a = annee.clone().unwrap_or_default();
                let pris: Vec<String> = dispo.keys().filter(|m| m.starts_with(&a)).cloned().collect();
                if pris.is_empty() {
                    return Err(erreur_operation(format!("Aucune paie en {}, ni saisie ni déclarée.", a)));
                }
                pris
            }
        } else if let Some(m) = &un_mois {
            vec![m.clone()]
        } else if toutes {
            dispo.keys().cloned().collect()
        } else {
            let a = annee.clone().unwrap_or_default();
            dispo.keys().filter(|m| m.starts_with(&a)).cloned().collect()
        };
        let libelle = if !mois.is_empty() {
            libelle_periode(&mois, &un_mois, &annee, toutes)?
        } else if let Some(m) = &un_mois {
            crate::tableaux_export::libelle_mois(m)?
        } else if let Some(a) = &annee {
            a.clone()
        } else {
            "toutes les années".to_string()
        };
        Ok(Periode { dispo, un_mois, annee, toutes, mois, libelle })
    }

    // ----------------------------------------------------------- le livre de paie

    /// `_lignes_du_mois` : les bulletins d'un mois saisi, recalcules.
    fn lignes_du_mois(&mut self, mois: &str) -> R<Vec<LigneLivre>> {
        let paies = self.paies(mois)?;
        let pas = crate::prelevement::des_paies(self.ctx, &paies).map_err(Refus::from)?;
        let mut out = Vec::new();
        for (p, r) in paies.iter().zip(pas) {
            if p.engagement.sans_paie() {
                continue;
            }
            out.push(LigneLivre {
                mois: mois.to_string(),
                salarie: qui(&p.salarie)?,
                cle_salarie: valeur_en_texte(requis(&p.salarie, "_cle")?),
                spectacle: p.spectacle.get("titre").filter(|t| vrai(t)).map(en_texte).unwrap_or_default(),
                brut: Some(eur2(p.brut)),
                cotisations_patronales: Some(eur2(p.total_pat)),
                net_imposable: Some(eur2(p.net_imposable)),
                net_a_payer: Some(eur2(p.net_a_payer)),
                pas: r.map(|v| eur2(v.prelevement)),
                cout_employeur: Some(eur2(p.cout_employeur)),
                source: SAISIE,
            });
        }
        Ok(out)
    }

    /// `_lignes_declarees` : un mois ANTERIEUR a PuppetPaie, tel que la DSN deposee l'a declare.
    fn lignes_declarees(&mut self, mois: &str) -> R<Vec<LigneLivre>> {
        let mut out = Vec::new();
        for x in self.paies_declarees()?.iter() {
            if x.get("mois").map_or(true, |m| valeur_en_texte(m) != mois) {
                continue;
            }
            let cle = match x.get("salarie").filter(|v| vrai(v)) {
                Some(v) => valeur_en_texte(v),
                None => String::new(),
            };
            let fiche = if cle.is_empty() { None } else { self.ctx.salaries.get(&cle).filter(|f| vrai(f)) };
            out.push(LigneLivre {
                mois: mois.to_string(),
                salarie: match fiche {
                    Some(f) => qui(f)?,
                    None => cle.clone(),
                },
                cle_salarie: cle.clone(),
                spectacle: self
                    .ctx
                    .spectacles
                    .values()
                    .find(|s| s.get("numero_objet").is_some_and(vrai) && s.get("numero_objet") == x.get("numero_objet"))
                    .and_then(|s| s.get("titre").filter(|t| vrai(t)).map(en_texte))
                    // A defaut de fiche, le titre que le bulletin de paie imprimait (lot E de l'import).
                    .or_else(|| x.get("spectacle").filter(|t| vrai(t)).map(en_texte))
                    .unwrap_or_default(),
                brut: montant(x.get("brut"))?,
                cotisations_patronales: montant(x.get("cotisations_patronales"))?,
                net_imposable: montant(x.get("net_imposable"))?,
                net_a_payer: montant(x.get("net_a_payer"))?,
                pas: montant(x.get("pas"))?,
                cout_employeur: montant(x.get("cout_employeur"))?,
                source: crate::etats_paie::source_declaree(x),
            });
        }
        Ok(out)
    }

    /// `livre_de_paie(...).lignes`, ce que le recapitulatif annuel en lit.
    fn lignes_du_livre(&mut self, p: &Periode) -> R<Vec<LigneLivre>> {
        let mut lignes = Vec::new();
        for m in &p.mois {
            if p.dispo.get(m) == Some(&DSN) {
                lignes.extend(self.lignes_declarees(m)?);
            } else {
                lignes.extend(self.lignes_du_mois(m)?);
            }
        }
        if lignes.is_empty() {
            return Err(erreur_operation("Aucun bulletin sur la période : rien à porter au livre de paie."));
        }
        Ok(lignes)
    }

    // ------------------------------------------------ le cout de l'annee

    /// `operations_etats.couts_de_l_annee` : ce qu'un spectacle, ou un salarie, a coute dans
    /// l'annee (`self.annee`), jusqu'au mois de `ctx.aujourdhui` ; ce qui est seulement saisi pour
    /// plus tard se dit a part (`prevu`). Sans annee, ou sans mois cette annee-la : rien.
    pub(crate) fn couts_de_l_annee(&mut self) -> R<Value> {
        let annee = self.annee.clone().unwrap_or_default();
        let courant: String = self.ctx.aujourdhui.iso().chars().take(7).collect();
        let mut out = Map::new();
        out.insert("annee".into(), json!(annee));
        out.insert("jusqu_a".into(), json!(courant));
        out.insert("spectacles".into(), json!({}));
        out.insert("salaries".into(), json!({}));
        // `Periode(ctx, None, annee, False)` : une annee vide ne se demande pas (`ErreurOperation`).
        if annee.is_empty() {
            return Ok(Value::Object(out));
        }
        let p = self.periode(false)?;
        if p.mois.is_empty() {
            return Ok(Value::Object(out));
        }
        struct Cumul {
            bulletins: i64,
            brut: Decimal,
            charges: Decimal,
            cout: Decimal,
            prevu: Decimal,
            incomplet: bool,
        }
        let zero = socle::num::zero2();
        let mut cotes: [Vec<(String, Cumul)>; 2] = [Vec::new(), Vec::new()];
        for l in self.lignes_du_livre(&p)? {
            for (i, cle) in [&l.spectacle, &l.cle_salarie].into_iter().enumerate() {
                if cle.is_empty() {
                    continue;
                }
                let pos = match cotes[i].iter().position(|(k, _)| k == cle) {
                    Some(n) => n,
                    None => {
                        cotes[i].push((
                            cle.clone(),
                            Cumul { bulletins: 0, brut: zero, charges: zero, cout: zero, prevu: zero, incomplet: false },
                        ));
                        cotes[i].len() - 1
                    }
                };
                let d = &mut cotes[i][pos].1;
                if l.mois > courant {
                    d.prevu += l.cout_employeur.unwrap_or(zero);
                    continue;
                }
                d.bulletins += 1;
                d.brut += l.brut.unwrap_or(zero);
                d.charges += l.cotisations_patronales.unwrap_or(zero);
                if l.cout_employeur.is_none() {
                    d.incomplet = true;
                }
                d.cout += l.cout_employeur.unwrap_or(zero);
            }
        }
        for (i, nom) in ["spectacles", "salaries"].into_iter().enumerate() {
            let mut m = Map::new();
            for (cle, d) in std::mem::take(&mut cotes[i]) {
                m.insert(
                    cle,
                    json!({
                        "bulletins": d.bulletins,
                        "brut": d.brut.to_string(),
                        "charges": d.charges.to_string(),
                        "cout": d.cout.to_string(),
                        "prevu": d.prevu.to_string(),
                        "incomplet": d.incomplet,
                    }),
                );
            }
            out.insert(nom.into(), Value::Object(m));
        }
        Ok(Value::Object(out))
    }

    // ------------------------------------------------ 2. le recapitulatif annuel

    fn recapitulatif_annuel(&mut self) -> R<Tableau> {
        if self.mois.is_some() {
            return Err(erreur_operation("Le récapitulatif annuel se demande pour une année, ou pour toutes les années — pas pour un mois."));
        }
        let p = self.periode(true)?;
        let lignes = self.lignes_du_livre(&p)?;
        // (annee, salarie) -> la ligne de synthese, dans l'ordre ou elle est apparue.
        let mut ordre: Vec<(String, String)> = Vec::new();
        let mut par: HashMap<(String, String), (Ligne, BTreeSet<&'static str>, i64)> = HashMap::new();
        let zero = Decimal::from_str("0.00").unwrap_or(Decimal::ZERO);
        for l in &lignes {
            let cle = (l.mois.chars().take(4).collect::<String>(), l.salarie.clone());
            if !par.contains_key(&cle) {
                ordre.push(cle.clone());
                par.insert(cle.clone(), (Ligne::new(), BTreeSet::new(), 0));
            }
            let (d, sources, n) = par.get_mut(&cle).expect("insere");
            *n += 1;
            let k = format!("m{}", l.mois.chars().skip(5).take(2).collect::<String>());
            let precedent = match d.get(&k) {
                Some(Some(Val::Dec(x))) if !x.is_zero() => *x,
                _ => zero,
            };
            d.insert(k, Some(Val::Dec(precedent + l.brut.filter(|b| !b.is_zero()).unwrap_or(zero))));
            for (champ, v) in [
                ("brut", l.brut),
                ("net_imposable", l.net_imposable),
                ("net_a_payer", l.net_a_payer),
                ("pas", l.pas),
                ("cotisations_patronales", l.cotisations_patronales),
                ("cout_employeur", l.cout_employeur),
            ] {
                if let Some(v) = v {
                    let avant = match d.get(champ) {
                        Some(Some(Val::Dec(x))) if !x.is_zero() => *x,
                        _ => zero,
                    };
                    d.insert(champ.to_string(), Some(Val::Dec(avant + v)));
                }
            }
            sources.insert(l.source);
        }
        let mut cols = vec![colonne("salarie", "Salarié", Genre::Texte)];
        for i in 1..=12usize {
            let abrege = crate::etats_paie::mois_abrege(i);
            cols.push(colonne(&format!("m{:02}", i), &abrege, Genre::Montant));
        }
        cols.push(colonne("brut", "Brut annuel", Genre::Montant));
        cols.push(colonne("net_imposable", "Net imposable", Genre::Montant));
        cols.push(colonne("pas", "PAS", Genre::Montant));
        cols.push(colonne("net_a_payer", "Net payé", Genre::Montant));
        cols.push(colonne("cout_employeur", "Coût employeur", Genre::Montant));
        cols.push(colonne("bulletins", "Bulletins", Genre::Nombre));
        cols.push(colonne("sources", "Source", Genre::Texte));
        // `_sections` : une section par annee, ses lignes triees par salarie.
        let mut paquets: BTreeMap<String, Vec<(String, Ligne)>> = BTreeMap::new();
        for cle in ordre {
            let (mut d, sources, n) = par.remove(&cle).expect("present");
            d.insert("salarie".into(), Some(Val::Txt(cle.1.clone())));
            d.insert("bulletins".into(), Some(Val::Num(n.to_string())));
            d.insert("sources".into(), Some(Val::Txt(sources.into_iter().collect::<Vec<_>>().join(" + "))));
            paquets.entry(cle.0.clone()).or_default().push((cle.1, d));
        }
        let sections = paquets
            .into_iter()
            .map(|(an, mut v)| {
                v.sort_by(|a, b| a.0.cmp(&b.0));
                Section { libelle: an, lignes: v.into_iter().map(|x| x.1).collect(), total: None }
            })
            .collect();
        let mut t = Tableau {
            titre: "Récapitulatif annuel par salarié".to_string(),
            periode: p.libelle,
            colonnes: cols,
            sections,
            total: None,
            sources: vec![
                "cumuls des bulletins de l'année ; le net imposable est celui que la DSN transmet à la DGFiP pour la déclaration de revenus"
                    .to_string(),
            ],
        };
        totaliser(&mut t)?;
        Ok(t)
    }

    // -------------------------------------------------- 11. les heures et les cachets

    fn heures_et_cachets(&mut self) -> R<Tableau> {
        let p = self.periode(false)?;
        let mut lignes: Vec<Ligne> = Vec::new();
        let saisis: BTreeSet<String> = p.saisis().into_iter().collect();
        let jj = |iso: &str| -> String {
            let c: Vec<char> = iso.chars().collect();
            if c.len() >= 10 && c[4] == '-' {
                format!("{}/{}/{}", c[8..10].iter().collect::<String>(), c[5..7].iter().collect::<String>(), c[..4].iter().collect::<String>())
            } else {
                iso.to_string()
            }
        };
        let txt = |s: String| Some(Val::Txt(s));
        for m in p.saisis() {
            let paies = self.paies(&m)?;
            for paie in paies.iter().filter(|x| !x.engagement.sans_paie()) {
                let e = &paie.engagement;
                let cachets = paie.remuneration.iter().filter(|r| r.unite == "cachet").fold(Decimal::ZERO, |a, r| a + r.quantite);
                let heures = paie.remuneration.iter().fold(Decimal::ZERO, |a, r| a + r.heures_annexes);
                let premiere = e.dates_travail.first().map(|j| j.iso()).unwrap_or_default();
                let derniere = e.dates_travail.last().map(|j| j.iso()).unwrap_or_default();
                let mut l = Ligne::new();
                l.insert("mois".into(), txt(m.clone()));
                l.insert("salarie".into(), txt(qui(&paie.salarie)?));
                l.insert("contrat".into(), txt(e.numero.clone().unwrap_or_default()));
                l.insert(
                    "spectacle".into(),
                    txt(paie.spectacle.get("titre").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()),
                );
                let emploi = match e.emploi.clone().filter(|x| !x.is_empty()) {
                    Some(x) => x,
                    None => paie.salarie.get("emploi").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default(),
                };
                l.insert("emploi".into(), txt(emploi));
                l.insert("debut".into(), txt(jj(&e.debut.iso().to_string())));
                let fin = match e.fin {
                    Some(f) => f.iso(),
                    None => derniere,
                };
                let _ = premiere;
                l.insert("fin".into(), txt(jj(&fin)));
                l.insert("cachets".into(), Some(Val::Dec(cachets)));
                l.insert("heures".into(), Some(Val::Dec(heures)));
                l.insert("jours".into(), Some(Val::Num(e.dates_travail.len().to_string())));
                l.insert("brut".into(), Some(Val::Dec(eur2(paie.brut))));
                l.insert("source".into(), txt(SAISIE.to_string()));
                lignes.push(l);
            }
        }
        // L'AEM attestee, pour les mois d'avant PuppetPaie.
        let mut objets: HashMap<String, Value> = HashMap::new();
        for s in self.ctx.spectacles.values() {
            let Value::Object(o) = s else {
                return Err(avec_classe("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(s))));
            };
            objets.insert(o.get("numero_objet").map_or("None".to_string(), valeur_en_texte), o.get("titre").cloned().unwrap_or(Value::Null));
        }
        for x in self.engagements_attestes()?.iter() {
            let debut = x.get("debut").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
            let m: String = debut.chars().take(7).collect();
            if m.is_empty() || saisis.contains(&m) || !p.contient(&m) {
                continue;
            }
            let cle = x.get("salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
            let fiche = if cle.is_empty() { None } else { self.ctx.salaries.get(&cle).filter(|f| vrai(f)) };
            let objet = x.get("numero_objet").cloned().unwrap_or(Value::Null);
            let spectacle = match objets.get(&valeur_en_texte(&objet)).filter(|v| vrai(v)) {
                Some(t) => valeur_en_texte(t),
                None => if vrai(&objet) { valeur_en_texte(&objet) } else { String::new() },
            };
            let mut l = Ligne::new();
            l.insert("mois".into(), txt(m));
            l.insert(
                "salarie".into(),
                txt(match fiche {
                    Some(f) => qui(f)?,
                    None => x.get("salarie").map_or(String::new(), valeur_en_texte),
                }),
            );
            l.insert("contrat".into(), txt(x.get("numero_attestation").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()));
            l.insert("spectacle".into(), txt(spectacle));
            l.insert("emploi".into(), txt(fiche.and_then(|f| f.get("emploi")).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()));
            l.insert("debut".into(), txt(jj(&valeur_en_texte(x.get("debut").unwrap_or(&Value::Null)))));
            l.insert("fin".into(), txt(jj(&valeur_en_texte(x.get("fin").unwrap_or(&Value::Null)))));
            l.insert("cachets".into(), Some(Val::Dec(montant_brut(x.get("cachets"))?)));
            l.insert("heures".into(), Some(Val::Dec(montant_brut(x.get("heures"))?)));
            l.insert(
                "jours".into(),
                match x.get("jours") {
                    None | Some(Value::Null) => None,
                    Some(Value::Number(n)) => Some(Val::Num(socle::date::forme_ecrite(&Value::Number(n.clone())))),
                    Some(autre) => Some(Val::Txt(valeur_en_texte(autre))),
                },
            );
            l.insert("brut".into(), montant(x.get("brut"))?.map(Val::Dec));
            l.insert("source".into(), txt(AEM.to_string()));
            lignes.push(l);
        }
        // UNE DONNEE IMPOSSIBLE SE DIT, elle ne se corrige pas ici : seul un `jours` qui ne se lit pas leve.
        for l in &lignes {
            if let (Some(Some(Val::Txt(debut))), Some(Some(Val::Txt(fin)))) = (l.get("debut"), l.get("fin")) {
                let _ = (debut, fin);
            }
            if let Some(Some(v)) = l.get("jours") {
                controle_jours(v)?;
            }
        }
        if lignes.is_empty() {
            return Err(erreur_operation(format!("Aucun contrat sur la période ({}), ni saisi ni attesté.", p.libelle)));
        }
        // Une section par salarie, ses lignes triees par (mois, debut, contrat).
        let mut paquets: BTreeMap<String, Vec<Ligne>> = BTreeMap::new();
        for l in lignes {
            let k = match l.get("salarie") {
                Some(Some(Val::Txt(s))) => s.clone(),
                _ => String::new(),
            };
            paquets.entry(k).or_default().push(l);
        }
        let cle_tri = |l: &Ligne| -> (String, String, String) {
            let t = |k: &str| match l.get(k) {
                Some(Some(Val::Txt(s))) => s.clone(),
                _ => String::new(),
            };
            (t("mois"), t("debut"), t("contrat"))
        };
        let sections = paquets
            .into_iter()
            .map(|(k, mut v)| {
                v.sort_by_key(|l| cle_tri(l));
                Section { libelle: k, lignes: v, total: None }
            })
            .collect();
        let cols = vec![
            colonne("mois", "Mois", Genre::Texte),
            colonne("salarie", "Salarié", Genre::Texte),
            colonne("contrat", "Contrat", Genre::Texte),
            colonne("spectacle", "Spectacle", Genre::Texte),
            colonne("emploi", "Emploi", Genre::Texte),
            colonne("debut", "Début", Genre::Texte),
            colonne("fin", "Fin", Genre::Texte),
            colonne("cachets", "Cachets", Genre::Nombre),
            colonne("heures", "Heures (annexes)", Genre::Nombre),
            colonne("jours", "Jours", Genre::Nombre),
            colonne("brut", "Brut", Genre::Montant),
            colonne("source", "Source", Genre::Texte),
        ];
        let mut t = Tableau {
            titre: "Heures et cachets (annexes 8 et 10)".to_string(),
            periode: p.libelle,
            colonnes: cols,
            sections,
            total: None,
            sources: vec![
                "règlement d'assurance chômage, annexes VIII et X (décret n° 2019-797 modifié) : un cachet compte pour les heures que la rubrique déclare (heures_annexes) ; AEM : attestation employeur mensuelle déposée à France Travail"
                    .to_string(),
            ],
        };
        totaliser(&mut t)?;
        Ok(t)
    }

    // ------------------------------------------------------------ la fiche

    fn etat(&mut self, i: usize) -> R<Rc<Tableau>> {
        if self.etats[i].is_none() {
            let r = if i == 0 { self.heures_et_cachets() } else { self.recapitulatif_annuel() };
            self.etats[i] = Some(r.map(Rc::new));
        }
        match self.etats[i].as_ref().unwrap() {
            Ok(t) => Ok(Rc::clone(t)),
            Err(e) => Err(copie(e)),
        }
    }

    /// `_recapitulatif` : les deux etats reduits au salarie `cle`, en un PDF dans `tmp`. `None` quand
    /// aucun des deux n'a rien a dire de lui.
    pub(crate) fn pdf(&mut self, cle: &str, tmp: &Path) -> R<Option<std::path::PathBuf>> {
        let fiche = self.ctx.salaries.get(cle).ok_or_else(|| avec_classe("KeyError", socle::fichiers::absent(cle)))?;
        let lui = format!(
            "{} {}",
            valeur_en_texte(requis(fiche, "nom")?),
            match fiche.get("prenom_affichage").filter(|v| vrai(v)) {
                Some(v) => valeur_en_texte(v),
                None => fiche.get("prenom").map(valeur_en_texte).unwrap_or_default(),
            }
        );
        let mut morceaux: Vec<Vec<u8>> = Vec::new();
        for i in 0..2 {
            let t = match self.etat(i) {
                Ok(t) => t,
                Err(e) if est_erreur_operation(&e) => continue,
                Err(e) => return Err(e),
            };
            let mut sections: Vec<Section> = Vec::new();
            for s in &t.sections {
                let lignes: Vec<Ligne> = s
                    .lignes
                    .iter()
                    .filter(|l| matches!(l.get("salarie"), Some(Some(Val::Txt(x))) if *x == lui))
                    .cloned()
                    .collect();
                if !lignes.is_empty() {
                    sections.push(Section { libelle: s.libelle.clone(), lignes, total: None });
                }
            }
            if sections.is_empty() {
                continue;
            }
            let mut reduit = Tableau {
                titre: format!("{} — {}", t.titre, lui),
                periode: t.periode.clone(),
                colonnes: t.colonnes.iter().map(|c| Colonne { cle: c.cle.clone(), libelle: c.libelle.clone(), genre: c.genre }).collect(),
                sections,
                total: None,
                sources: t.sources.clone(),
            };
            totaliser(&mut reduit)?;
            let d = description(&reduit, &nom_compagnie(self.ctx));
            let texte = serde_json::to_string(&d).map_err(|e| e.to_string())?;
            // `pdf_fabrique` : un refus de la fabrique est une `ErreurOperation`.
            let octets = fabrique_pdf::fabriquer(&texte).map_err(|r| {
                erreur_operation(format!(
                    "PDF impossible : la fabrique de PDF refuse cet état ({}). Aucun PDF n'est fait autrement.",
                    r.0.replace('\n', " ")
                ))
            });
            match octets {
                Ok(o) => morceaux.push(o),
                Err(e) => return Err(e),
            }
        }
        if morceaux.is_empty() {
            return Ok(None);
        }
        let chemin = tmp.join(format!("R_{}.pdf", cle));
        let octets = crate::routes_dossier_salarie::fusion_octets(&morceaux, tmp)?;
        std::fs::write(&chemin, octets).map_err(|e| Refus::from(format!("ecriture impossible de {} : {}", chemin.display(), e)))?;
        Ok(Some(chemin))
    }
}

/// `_montant(x.get(...))` pour les cachets et les heures attestes : `Decimal(str(x.get("cachets") or 0))`.
fn montant_brut(v: Option<&Value>) -> R<Decimal> {
    let t = match v.filter(|v| vrai(v)) {
        Some(x) => valeur_en_texte(x),
        None => "0".to_string(),
    };
    crate::apercu_dsn::decimal_py(&t).ok_or_else(|| avec_classe("InvalidOperation", socle::num::illisible(&t)))
}

/// `int(l["jours"])` : un `jours` qui ne se lit pas leve `ValueError`.
fn controle_jours(v: &Val) -> R<()> {
    match v {
        Val::Txt(t) => {
            if t.is_empty() {
                return Ok(());
            }
            if crate::apercu_dsn::int_py(t).is_none() {
                return Err(avec_classe("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(t))));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[allow(dead_code)]
fn jour_ou_vide(v: &Value) -> String {
    match socle::date::d(v) {
        Ok(j) => j.iso(),
        Err(_) => en_texte(v),
    }
}

#[allow(dead_code)]
fn _jour(_: Jour) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_fractions_donnent_une_largeur_utile_pleine() {
        let entetes = vec![("Mois".to_string(), false), ("Brut".to_string(), true)];
        let rangs = vec![(vec!["2026-10".to_string(), "260.00".to_string()], "")];
        let f = fractions(&entetes, &rangs, 700.0);
        assert!((f.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }
}
