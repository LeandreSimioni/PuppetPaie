//! La machinerie commune des etats exportes : un tableau, six facons de l'ecrire --
//! CSV, texte tabule, XLSX, PDF, HTML, JSON --, et les ecrivains du livre de paie, qui a les siens
//! (`operations_exports.csv_livre`, `xlsx_livre`, `pdf_livre`).
//!
//! UN ETAT DECLARE, LA MACHINERIE ECRIT. Un etat rend un `Tableau` : ses colonnes (et lesquelles portent un
//! montant), ses sections (un groupe de lignes et son sous-total), un total general, ses avertissements et les
//! sources qui le fondent. Chaque ligne est un dictionnaire ordonne cle -> valeur : une ligne qui oublie une
//! colonne laisse une case vide. Une cle PRESENTE a `None` (la source ne dit pas ce montant) se lit « — » au PDF
//! et a la page HTML ; une cle ABSENTE reste vide.
//!
//! LES TOTAUX se calculent ici, avec la regle du livre de paie : un montant `None` ne compte pas pour zero, et
//! une colonne qu'aucune ligne ne declare a un total VIDE, pas 0,00.
//!
//! Le `.xlsx` s'ecrit avec `rust_xlsxwriter` : montants en nombres, formats `# ##0.00 €`, largeurs 14 / 26,
//! volet fige sous l'en-tete. Il se prouve par son contenu (openpyxl), pas a l'octet. Le PDF est celui de la
//! fabrique (`fabrique_pdf`), decrit comme `tableaux.description_pdf`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Format, FormatAlign, Workbook, Worksheet};
use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::forme_ecrite;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::Decimal;
use socle::echec::Echec;

pub type R<T> = Result<T, Echec>;

/// Une `ErreurOperation`.
pub fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// `FORMATS_TABLEAU` : les six formats de la machinerie, dans l'ordre du format.
pub const FORMATS_TABLEAU: [&str; 6] = ["csv", "tsv", "xlsx", "pdf", "html", "json"];

// ------------------------------------------------------------------ valeurs

/// Une valeur de cellule : un `Decimal`, un nombre JSON (entier, flottant), un texte.
#[derive(Clone, Debug)]
pub enum Val {
    Dec(Decimal),
    Num(Value),
    Txt(String),
}

impl Val {
    pub fn txt(s: impl Into<String>) -> Val {
        Val::Txt(s.into())
    }

    pub fn entier(n: i64) -> Val {
        Val::Num(Value::from(n))
    }

    /// `str(v)`.
    pub fn str_py(&self) -> String {
        match self {
            Val::Dec(d) => d.to_string(),
            Val::Num(v) => forme_ecrite(v),
            Val::Txt(s) => s.clone(),
        }
    }

    /// `Decimal(str(v))`.
    pub fn decimal(&self) -> R<Decimal> {
        let t = match self {
            Val::Dec(d) => return Ok(*d),
            Val::Num(v) => return socle::num::dec(v).map_err(|m| erreur("InvalidOperation", m)),
            Val::Txt(s) => s,
        };
        socle::num::decimal_py(t).ok_or_else(|| erreur("InvalidOperation", socle::num::illisible(t)))
    }

    /// `dict_tableau._v` / `sortie_json.brut` : un `Decimal` devient son texte.
    pub fn json(&self) -> Value {
        match self {
            Val::Dec(d) => Value::String(d.to_string()),
            Val::Num(v) => v.clone(),
            Val::Txt(s) => Value::String(s.clone()),
        }
    }
}

/// Une ligne : un dictionnaire ordonne (l'ordre d'insertion compte pour le JSON).
#[derive(Clone, Debug, Default)]
pub struct Ligne {
    pub champs: Vec<(String, Option<Val>)>,
}

impl Ligne {
    pub fn nouvelle() -> Ligne {
        Ligne::default()
    }

    /// `d[k] = v` : remplace en gardant la place, ou ajoute a la fin.
    pub fn poser(&mut self, k: &str, v: Option<Val>) -> &mut Ligne {
        match self.champs.iter_mut().find(|(c, _)| c == k) {
            Some(x) => x.1 = v,
            None => self.champs.push((k.to_string(), v)),
        }
        self
    }

    pub fn t(&mut self, k: &str, s: impl Into<String>) -> &mut Ligne {
        self.poser(k, Some(Val::Txt(s.into())))
    }

    pub fn d(&mut self, k: &str, v: Decimal) -> &mut Ligne {
        self.poser(k, Some(Val::Dec(v)))
    }

    pub fn od(&mut self, k: &str, v: Option<Decimal>) -> &mut Ligne {
        self.poser(k, v.map(Val::Dec))
    }

    pub fn n(&mut self, k: &str, v: i64) -> &mut Ligne {
        self.poser(k, Some(Val::entier(v)))
    }

    pub fn rien(&mut self, k: &str) -> &mut Ligne {
        self.poser(k, None)
    }

    /// `d.get(k)` distinguant l'absente (`None`) de la presente a `None` (`Some(None)`).
    pub fn get(&self, k: &str) -> Option<&Option<Val>> {
        self.champs.iter().find(|(c, _)| c == k).map(|(_, v)| v)
    }

    /// `d.get(k)` : la valeur, ou rien.
    pub fn val(&self, k: &str) -> Option<&Val> {
        self.get(k).and_then(|v| v.as_ref())
    }

    /// Le texte d'une cle (vide si absente ou `None`).
    pub fn texte(&self, k: &str) -> String {
        self.val(k).map(Val::str_py).unwrap_or_default()
    }

    pub fn decimal(&self, k: &str) -> Option<Decimal> {
        match self.val(k) {
            Some(Val::Dec(d)) => Some(*d),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ le tableau

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Genre {
    Montant,
    Nombre,
    Taux,
    Texte,
}

impl Genre {
    pub fn nom(self) -> &'static str {
        match self {
            Genre::Montant => "montant",
            Genre::Nombre => "nombre",
            Genre::Taux => "taux",
            Genre::Texte => "texte",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Colonne {
    pub cle: String,
    pub libelle: String,
    pub genre: Genre,
}

impl Colonne {
    pub fn totalisee(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre)
    }

    pub fn a_droite(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre | Genre::Taux)
    }
}

/// Une colonne de texte.
pub fn col_t(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Texte }
}

/// Une colonne de montant.
pub fn col_m(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Montant }
}

/// Une colonne de quantite.
pub fn col_n(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Nombre }
}

/// Une colonne de taux (a droite, jamais totalisee).
pub fn col_x(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Taux }
}

#[derive(Clone, Debug)]
pub struct Section {
    pub libelle: String,
    pub lignes: Vec<Ligne>,
    pub total: Option<Ligne>,
}

impl Section {
    pub fn nouvelle(libelle: impl Into<String>, lignes: Vec<Ligne>) -> Section {
        Section { libelle: libelle.into(), lignes, total: None }
    }
}

#[derive(Clone, Debug)]
pub struct Tableau {
    pub etat: String,
    pub titre: String,
    pub periode: String,
    pub colonnes: Vec<Colonne>,
    pub sections: Vec<Section>,
    pub total: Option<Ligne>,
    pub avertissements: Vec<String>,
    pub sources: Vec<String>,
    pub mois: Vec<String>,
    pub format: String,
    pub fichier: Option<PathBuf>,
    pub sous_totaux: bool,
    pub avec_total: bool,
    pub donnees_personnelles: bool,
}

impl Tableau {
    pub fn nouveau(etat: &str, titre: impl Into<String>, periode: impl Into<String>) -> Tableau {
        Tableau {
            etat: etat.to_string(),
            titre: titre.into(),
            periode: periode.into(),
            colonnes: Vec::new(),
            sections: Vec::new(),
            total: None,
            avertissements: Vec::new(),
            sources: Vec::new(),
            mois: Vec::new(),
            format: "csv".to_string(),
            fichier: None,
            sous_totaux: true,
            avec_total: true,
            donnees_personnelles: false,
        }
    }

}

/// `_sections(lignes, cle, libelle, tri)` : les lignes en sections, une par cle, les cles triees
/// (`sorted(paquets.items())`), les lignes de chaque section triees par `tri` s'il y en a un (tri stable).
pub fn sections_de<K: Ord + Clone>(
    lignes: Vec<Ligne>, cle: impl Fn(&Ligne) -> K, libelle: impl Fn(&K) -> String, tri: Option<&dyn Fn(&Ligne) -> Vec<String>>,
) -> Vec<Section> {
    let mut paquets: BTreeMap<K, Vec<Ligne>> = BTreeMap::new();
    for l in lignes {
        paquets.entry(cle(&l)).or_default().push(l);
    }
    paquets
        .into_iter()
        .map(|(k, mut v)| {
            if let Some(t) = tri {
                v.sort_by_cached_key(|l| t(l));
            }
            Section { libelle: libelle(&k), lignes: v, total: None }
        })
        .collect()
}

// ------------------------------------------------------------------ totaux

/// `_somme` : la somme de ce qui est DECLARE, ou `None` quand aucune ligne ne l'est.
fn somme(lignes: &[&Ligne], cle: &str) -> R<Option<Val>> {
    let mut total: Option<Decimal> = None;
    for l in lignes {
        let v = match l.get(cle) {
            Some(Some(Val::Txt(t))) if t.is_empty() => continue,
            Some(Some(v)) => v,
            _ => continue,
        };
        total = Some(total.unwrap_or(Decimal::ZERO) + v.decimal()?);
    }
    Ok(total.map(Val::Dec))
}

/// `tableaux.totaliser` : pose le sous-total de chaque section et le total general. La premiere colonne de
/// texte porte le libelle (« Total ... »), les colonnes totalisees leur somme, les autres restent vides.
pub fn totaliser(t: &mut Tableau, libelle_total: &str) -> R<()> {
    let porte = t.colonnes.iter().find(|c| c.genre == Genre::Texte).map(|c| c.cle.clone());
    let ligne = |colonnes: &[Colonne], lignes: &[&Ligne], libelle: &str| -> R<Ligne> {
        let mut d = Ligne::nouvelle();
        for c in colonnes.iter().filter(|c| c.totalisee()) {
            d.poser(&c.cle, somme(lignes, &c.cle)?);
        }
        if let Some(p) = &porte {
            d.t(p, libelle);
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
    t.total = if t.avec_total {
        let toutes: Vec<&Ligne> = t.sections.iter().flat_map(|s| s.lignes.iter()).collect();
        Some(ligne(&t.colonnes, &toutes, libelle_total)?)
    } else {
        None
    };
    if !t.avec_total {
        for s in t.sections.iter_mut() {
            s.total = None;
        }
    }
    Ok(())
}

/// Le libelle du total general : « TOTAL GÉNÉRAL ».
pub const TOTAL_GENERAL: &str = "TOTAL GÉNÉRAL";

// ------------------------------------------------------------ mise en forme

/// `tableaux.texte` : une cellule en texte francais, virgule decimale, rien pour `None`.
pub fn texte(v: Option<&Val>, c: &Colonne) -> String {
    match v {
        None => String::new(),
        Some(Val::Dec(d)) if c.a_droite() => d.to_string().replace('.', ","),
        Some(Val::Num(n)) if c.a_droite() => forme_ecrite(n).replace('.', ","),
        Some(autre) => autre.str_py(),
    }
}

/// `tableaux._rangees` : (cellules, genre) dans l'ordre d'ecriture : lignes, sous-totaux, total.
fn rangees(t: &Tableau) -> Vec<(&Ligne, &'static str)> {
    let plusieurs = t.sections.len() > 1 && t.sous_totaux;
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

/// `(texte, genre)` de chaque cellule d'une ligne, `—` pour un montant de ligne que la source ne dit pas.
fn cellule_lisible(l: &Ligne, c: &Colonne, genre: &str) -> String {
    let v = l.get(&c.cle);
    if matches!(v, Some(None)) && c.genre == Genre::Montant && genre.is_empty() {
        "—".to_string()
    } else {
        texte(v.and_then(|x| x.as_ref()), c)
    }
}

/// `tableaux._nom_compagnie` : le nom de la compagnie, ou rien.
pub fn nom_compagnie(ctx: &Contexte) -> String {
    crate::fiche_recapitulative::nom_compagnie(ctx)
}

fn entete_texte(t: &Tableau, nom: &str) -> Vec<String> {
    let mut out = vec![format!("{} — {}", t.titre, t.periode)];
    if !nom.is_empty() {
        out.insert(0, nom.to_string());
    }
    out
}

fn lire_nom(ctx: Option<&Contexte>) -> String {
    ctx.map(nom_compagnie).unwrap_or_default()
}

// ---------------------------------------------------------------------- csv

/// `csv_tableau` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub fn csv_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut s = String::from("\u{feff}");
    let ligne = |s: &mut String, c: &[String]| s.push_str(&socle::csv::ligne(c, ';'));
    for x in entete_texte(t, &lire_nom(ctx)) {
        ligne(&mut s, &[x]);
    }
    ligne(&mut s, &[]);
    ligne(&mut s, &t.colonnes.iter().map(|c| c.libelle.clone()).collect::<Vec<_>>());
    let mut precedent = "";
    for (d, genre) in rangees(t) {
        if genre == "general" && precedent == "total" {
            ligne(&mut s, &[]);
        }
        ligne(&mut s, &t.colonnes.iter().map(|c| texte(d.val(&c.cle), c)).collect::<Vec<_>>());
        if genre == "total" {
            ligne(&mut s, &[]);
        }
        precedent = genre;
    }
    for a in &t.avertissements {
        ligne(&mut s, &[]);
        ligne(&mut s, &[format!("! {}", a)]);
    }
    if !t.sources.is_empty() {
        ligne(&mut s, &[]);
        for src in &t.sources {
            ligne(&mut s, &[format!("Source : {}", src)]);
        }
    }
    Ok(s.into_bytes())
}

// ---------------------------------------------------------------------- tsv

/// `tsv_tableau` : les donnees seules. Une colonne « Groupe » en tete quand il y a plusieurs sections.
pub fn tsv_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let groupe = t.sections.len() > 1;
    let mut s = String::new();
    let mut entete: Vec<String> = if groupe { vec!["Groupe".to_string()] } else { Vec::new() };
    entete.extend(t.colonnes.iter().map(|c| c.libelle.clone()));
    s.push_str(&socle::csv::ligne(&entete, '\t'));
    for sec in &t.sections {
        for l in &sec.lignes {
            let mut cel: Vec<String> = if groupe { vec![sec.libelle.clone()] } else { Vec::new() };
            cel.extend(t.colonnes.iter().map(|c| texte(l.val(&c.cle), c)));
            s.push_str(&socle::csv::ligne(&cel, '\t'));
        }
    }
    Ok(s.into_bytes())
}

// --------------------------------------------------------------------- json

fn ligne_json(l: &Ligne) -> Value {
    Value::Object(l.champs.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.as_ref().map_or(Value::Null, Val::json))).collect())
}

fn ligne_json_brute(l: &Ligne) -> Value {
    // `_v` du fichier JSON ne retire pas les cles en `_` ; il n'y en a pas a ce stade.
    Value::Object(l.champs.iter().map(|(k, v)| (k.clone(), v.as_ref().map_or(Value::Null, Val::json))).collect())
}

/// `tableaux.dict_tableau` : ce que le fichier JSON porte.
pub fn dict_tableau(t: &Tableau, fichier_ecrit: bool) -> Value {
    let sections: Vec<Value> = t
        .sections
        .iter()
        .map(|s| json!({"libelle": s.libelle, "lignes": s.lignes.iter().map(ligne_json_brute).collect::<Vec<_>>(), "total": s.total.as_ref().map(ligne_json_brute)}))
        .collect();
    let fichier = if fichier_ecrit { t.fichier.as_ref().and_then(|p| p.file_name()).map(|n| json!(n.to_string_lossy())) } else { None };
    json!({
        "etat": t.etat, "titre": t.titre, "periode": t.periode, "mois": t.mois,
        "colonnes": t.colonnes.iter().map(|c| json!({"cle": c.cle, "libelle": c.libelle, "type": c.genre.nom()})).collect::<Vec<_>>(),
        "sections": sections,
        "total": t.total.as_ref().map(ligne_json_brute),
        "avertissements": t.avertissements,
        "sources": t.sources,
        "donnees_personnelles": t.donnees_personnelles,
        "format": t.format,
        "fichier": fichier,
    })
}

/// `json_tableau` : `json.dumps(dict_tableau(t), ensure_ascii=False, indent=1)`. Le fichier n'est pas encore nomme quand il s'ecrit.
pub fn json_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    Ok(socle::ecriture::json_indente(&dict_tableau(t, false), 1).into_bytes())
}

/// `sortie_json.brut(Tableau)` : la reponse de la route.
pub fn brut_tableau(t: &Tableau) -> Value {
    let mut m = Map::new();
    m.insert("etat".into(), json!(t.etat));
    m.insert("titre".into(), json!(t.titre));
    m.insert("periode".into(), json!(t.periode));
    m.insert("colonnes".into(), json!(t.colonnes.iter().map(|c| json!({"cle": c.cle, "libelle": c.libelle, "type": c.genre.nom()})).collect::<Vec<_>>()));
    m.insert(
        "sections".into(),
        Value::Array(t.sections.iter().map(|s| json!({"libelle": s.libelle, "lignes": s.lignes.iter().map(ligne_json).collect::<Vec<_>>(), "total": s.total.as_ref().map(ligne_json)})).collect()),
    );
    m.insert("total".into(), t.total.as_ref().map_or(Value::Null, ligne_json));
    m.insert("avertissements".into(), json!(t.avertissements));
    m.insert("sources".into(), json!(t.sources));
    m.insert("mois".into(), json!(t.mois));
    m.insert("format".into(), json!(t.format));
    m.insert("fichier".into(), t.fichier.as_ref().and_then(|p| p.file_name()).map_or(Value::Null, |n| json!(n.to_string_lossy())));
    m.insert("sous_totaux".into(), json!(t.sous_totaux));
    m.insert("avec_total".into(), json!(t.avec_total));
    m.insert("donnees_personnelles".into(), json!(t.donnees_personnelles));
    Value::Object(m)
}

// --------------------------------------------------------------------- xlsx

/// Une cellule du classeur.
enum Cel {
    Vide,
    Nombre(f64),
    Texte(String),
}

fn en_f64(d: &Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    d.to_f64().unwrap_or(0.0)
}

/// `_montant_classeur` : le nombre d'une colonne de chiffres, le texte sinon.
fn cellule_classeur(v: Option<&Val>, c: &Colonne) -> Cel {
    let Some(v) = v else { return Cel::Vide };
    if matches!(v, Val::Txt(s) if s.is_empty()) {
        return Cel::Vide;
    }
    if c.a_droite() {
        return match v.decimal() {
            Ok(d) => Cel::Nombre(en_f64(&d)),
            Err(_) => Cel::Texte(v.str_py()),
        };
    }
    match v {
        Val::Dec(d) => Cel::Nombre(en_f64(d)),
        Val::Num(n) => match n.as_f64() {
            Some(x) => Cel::Nombre(x),
            None => Cel::Texte(forme_ecrite(n)),
        },
        Val::Txt(s) => Cel::Texte(s.clone()),
    }
}

fn xl(e: rust_xlsxwriter::XlsxError) -> Echec {
    erreur("OSError", format!("classeur : {}", e))
}

/// Un format : gras ou non, une taille, un format de nombre.
fn format_de(gras: bool, taille: Option<f64>, nombre: Option<&str>) -> Format {
    let mut f = Format::new();
    if gras {
        f = f.set_bold();
    }
    if let Some(t) = taille {
        f = f.set_font_size(t);
    }
    if let Some(n) = nombre {
        f = f.set_num_format(n);
    }
    f
}

fn ecrire_cellule(ws: &mut Worksheet, ligne: u32, col: u16, cel: &Cel, fmt: &Format) -> R<()> {
    match cel {
        Cel::Vide => Ok(()),
        Cel::Nombre(x) => ws.write_number_with_format(ligne, col, *x, fmt).map(|_| ()).map_err(xl),
        Cel::Texte(t) => ws.write_string_with_format(ligne, col, t, fmt).map(|_| ()).map_err(xl),
    }
}

/// Le nom de la feuille : au plus 31 caracteres, sans ceux qu'Excel refuse.
fn nom_de_feuille(t: &str) -> String {
    let n: String = t.chars().take(31).collect();
    n.chars().map(|c| if "[]:*?/\\".contains(c) { '_' } else { c }).collect()
}

/// `xlsx_tableau`.
pub fn xlsx_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(nom_de_feuille(&t.titre)).map_err(xl)?;
    let mut ligne: u32 = 0;
    for x in entete_texte(t, &lire_nom(ctx)) {
        ws.write_string_with_format(ligne, 0, &x, &format_de(true, Some(13.0), None)).map_err(xl)?;
        ligne += 1;
    }
    ligne += 1;
    let entete = Format::new().set_bold().set_text_wrap().set_align(FormatAlign::VerticalCenter);
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.write_string_with_format(ligne, i as u16, &c.libelle, &entete).map_err(xl)?;
    }
    let ligne_entete = ligne;
    ligne += 1;
    let nombre_de = |c: &Colonne| -> Option<&'static str> {
        match c.genre {
            Genre::Montant => Some("# ##0.00 \u{20ac}"),
            Genre::Nombre => Some("# ##0.##"),
            Genre::Taux => Some("0.000##"),
            Genre::Texte => None,
        }
    };
    for (d, genre) in rangees(t) {
        let (gras, taille) = match genre {
            "general" => (true, Some(12.0)),
            "total" => (true, Some(11.0)),
            _ => (false, None),
        };
        for (i, c) in t.colonnes.iter().enumerate() {
            let cel = cellule_classeur(d.val(&c.cle), c);
            let fmt = format_de(gras, taille, if matches!(cel, Cel::Nombre(_)) { nombre_de(c) } else { None });
            ecrire_cellule(ws, ligne, i as u16, &cel, &fmt)?;
        }
        ligne += 1;
    }
    let simple = Format::new();
    for a in &t.avertissements {
        ligne += 1;
        ws.write_string_with_format(ligne, 0, format!("! {}", a), &simple).map_err(xl)?;
        ligne += 1;
    }
    for s in &t.sources {
        ws.write_string_with_format(ligne, 0, format!("Source : {}", s), &simple).map_err(xl)?;
        ligne += 1;
    }
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.set_column_width(i as u16, if c.a_droite() { 14.0 } else { 26.0 }).map_err(xl)?;
    }
    ws.set_freeze_panes(ligne_entete + 1, 0).map_err(xl)?;
    wb.save_to_buffer().map_err(xl)
}

// --------------------------------------------------------------- html / pdf

const CSS: &str = "\n* { font-family: Arial, sans-serif; }\nbody { font-size: 7pt; color: #2C3E50; }\nh1 { font-size: 11pt; color: #B22222; margin: 0 0 1pt 0;\n     text-transform: uppercase; letter-spacing: 0.5pt; }\n.sub { font-size: 6.8pt; color: #555; margin: 0 0 6pt 0; }\n.rs { font-size: 8.5pt; font-weight: bold; color: #2C3E50; margin: 0 0 1pt 0; }\ntable { width: 100%; border-collapse: collapse; }\nth { font-size: 6pt; color: #B22222; text-transform: uppercase;\n     letter-spacing: 0.3pt; text-align: left; padding: 0 3pt 2pt 0;\n     border-bottom: 0.7pt solid #B22222; }\ntd { font-size: 7pt; padding: 1.6pt 3pt 1.6pt 0; border-bottom: 0.3pt solid #eee; }\nth.n, td.n { text-align: right; }\ntr.total td { font-weight: bold; border-top: 0.5pt solid #999;\n              border-bottom: 0.5pt solid #999; }\ntr.general td { font-weight: bold; font-size: 8pt; color: #B22222; }\n.note { font-size: 6.3pt; color: #777; margin: 6pt 0 0 0; }\n.source { font-size: 6pt; color: #999; margin: 2pt 0 0 0; }\n";

/// `tableaux._e`.
fn echapper(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `html_tableau`.
pub fn html_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut corps = String::new();
    for (d, genre) in rangees(t) {
        let mut cel = String::new();
        for c in &t.colonnes {
            let v = d.get(&c.cle);
            let s = if matches!(v, Some(None)) && c.genre == Genre::Montant && genre.is_empty() {
                "&mdash;".to_string()
            } else {
                echapper(&texte(v.and_then(|x| x.as_ref()), c))
            };
            cel.push_str(&format!("<td{}>{}</td>", if c.a_droite() { " class='n'" } else { "" }, s));
        }
        corps.push_str(&format!("<tr{}>{}</tr>", if genre.is_empty() { String::new() } else { format!(" class='{}'", genre) }, cel));
    }
    let th: String = t.colonnes.iter().map(|c| format!("<th{}>{}</th>", if c.a_droite() { " class='n'" } else { "" }, echapper(&c.libelle))).collect();
    let nom = lire_nom(ctx);
    let tete = format!(
        "{}<h1>{}</h1><p class='sub'>{}{}</p>",
        if nom.is_empty() { String::new() } else { format!("<p class='rs'>{}</p>", echapper(&nom)) },
        echapper(&t.titre),
        echapper(&t.periode),
        if t.donnees_personnelles { " &mdash; porte des données personnelles" } else { "" }
    );
    let pied: String = t.avertissements.iter().map(|a| format!("<p class='note'>! {}</p>", echapper(a))).collect::<String>()
        + &t.sources.iter().map(|s| format!("<p class='source'>Source : {}</p>", echapper(s))).collect::<String>();
    let page = format!(
        "<!DOCTYPE html><html lang='fr'><head><meta charset='utf-8'><title>{} — {}</title><style>{} body{{font-size:10pt;margin:24px}}td,th{{font-size:9pt}}</style></head><body>{}<table><tr>{}</tr>{}</table>{}</body></html>",
        echapper(&t.titre),
        echapper(&t.periode),
        CSS,
        tete,
        th,
        corps,
        pied
    );
    Ok(page.into_bytes())
}

const PAGE_LARGEUR: f64 = 841.89;
const PAGE_HAUTEUR: f64 = 595.28;
const ROUGE: &str = "#B22222";
const ENCRE: &str = "#2C3E50";

fn filets(couleur: &str, epaisseur: f64) -> Value {
    json!({"couleur": couleur, "epaisseur": epaisseur})
}

/// `tableaux.description_pdf` : la description (FORMAT.md) d'un etat -- A4 paysage, son titre, son tableau, ses
/// notes. `entetes` : (libelle, a droite) ; `rangees` : (cellules en texte, genre) avec `genre` dans « », « total »
/// ou « general ».
#[allow(clippy::too_many_arguments)]
pub fn description_pdf(
    titre: &str, periode: &str, nom: &str, sous_titre: &str, pied: &str, entetes: &[(String, bool)], rangees: &[(Vec<String>, &str)], notes: &[String],
    sources: &[String],
) -> Value {
    let fr = crate::fiche_recapitulative::fractions(entetes, rangees, PAGE_LARGEUR - 68.0);
    let colonnes: Vec<Value> =
        fr.iter().zip(entetes).map(|(f, e)| if e.1 { json!({"largeur": f, "alignement": "droite"}) } else { json!({"largeur": f}) }).collect();
    let lignes: Vec<Value> = rangees
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
    blocs.push(json!({"type": "paragraphe", "taille": 11, "couleur": ROUGE, "majuscules": true, "espace_apres": 1, "morceaux": [{"texte": titre, "gras": true}]}));
    blocs.push(json!({"type": "paragraphe", "taille": 6.8, "couleur": "#555555", "espace_apres": 6, "morceaux": [{"texte": sous_titre}]}));
    blocs.push(json!({
        "type": "tableau", "colonnes": colonnes, "taille": 7, "couleur": ENCRE,
        "marge_cellule": {"haut": 1.6, "droite": 3.0, "bas": 1.6, "gauche": 3.0},
        "filet_lignes": {"couleur": "#EEEEEE", "epaisseur": 0.3},
        "entete": [{"cellules": entetes.iter().map(|e| e.0.clone()).collect::<Vec<_>>(), "gras": true, "taille": 6.3,
                    "couleur": ROUGE, "filet_bas": {"couleur": ROUGE, "epaisseur": 0.7}}],
        "lignes": lignes}));
    for a in notes {
        blocs.push(json!({"type": "paragraphe", "taille": 6.3, "couleur": "#777777", "espace_avant": 6, "morceaux": [{"texte": format!("! {}", a)}]}));
    }
    for src in sources {
        blocs.push(json!({"type": "paragraphe", "taille": 6, "couleur": "#999999", "espace_avant": 2, "morceaux": [{"texte": format!("Source : {}", src)}]}));
    }
    json!({
        "format": crate::bulletin::FORMAT,
        "page": {"largeur": PAGE_LARGEUR, "hauteur": PAGE_HAUTEUR, "marges": {"haut": 40, "droite": 34, "bas": 40, "gauche": 34}},
        "police": {"taille": 7, "interligne": 1.25, "couleur": ENCRE},
        "titre": format!("{} — {}", titre, periode),
        "entete_page": {"gauche": format!("{} — {} (suite)", titre, periode), "taille": 8.5, "gras": true, "couleur": ENCRE, "distance": 24, "premiere_page": false},
        "pied_page": {"gauche": pied, "droite": "page {page} sur {pages}", "taille": 6.5, "couleur": "#777777", "distance": 20},
        "documents": [{"blocs": blocs}]
    })
}

/// `pdf_fabrique` : les octets du PDF mis en page par la fabrique. Un refus de la fabrique est une `ErreurOperation`.
pub fn pdf_fabrique(description: &Value) -> R<Vec<u8>> {
    let texte = serde_json::to_string(description).map_err(|e| refus(e.to_string()))?;
    fabrique_pdf::fabriquer(&texte).map_err(|r| refus(format!("PDF impossible : la fabrique de PDF refuse cet état ({}). Aucun PDF n'est fait autrement.", r.0.replace('\n', " "))))
}

/// `pdf_tableau` : A4 paysage, l'en-tete des colonnes a chaque page, un pied numerote.
pub fn pdf_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let sous = format!("{}{}", t.periode, if t.donnees_personnelles { " — porte des données personnelles" } else { "" });
    let rangs: Vec<(Vec<String>, &str)> = rangees(t).into_iter().map(|(d, genre)| (t.colonnes.iter().map(|c| cellule_lisible(d, c, genre)).collect(), genre)).collect();
    let entetes: Vec<(String, bool)> = t.colonnes.iter().map(|c| (c.libelle.clone(), c.a_droite())).collect();
    let d = description_pdf(&t.titre, &t.periode, &lire_nom(ctx), &sous, &format!("{} — {}", t.titre, t.periode), &entetes, &rangs, &t.avertissements, &t.sources);
    pdf_fabrique(&d)
}

// ------------------------------------------------------------------ l'ecriture

/// Les octets d'un tableau dans son format.
pub fn octets(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    match t.format.as_str() {
        "csv" => csv_tableau(t, ctx),
        "tsv" => tsv_tableau(t, ctx),
        "xlsx" => xlsx_tableau(t, ctx),
        "pdf" => pdf_tableau(t, ctx),
        "html" => html_tableau(t, ctx),
        "json" => json_tableau(t, ctx),
        autre => Err(refus(format!("Format inconnu pour cet état : « {} ».", autre))),
    }
}

/// `tableaux.format_possible` : le format demande, s'il est l'un de `formats`. (Le XLSX et le PDF sont toujours
/// disponibles : ni openpyxl ni la fabrique externe ne sont plus en jeu.)
pub fn format_possible(fmt: &str, formats: &[&str]) -> R<String> {
    if !formats.contains(&fmt) {
        return Err(refus(format!("Format inconnu pour cet état : « {} ». Les formats possibles sont {}.", fmt, formats.join(", "))));
    }
    Ok(fmt.to_string())
}

/// `tableaux.ecrire` : ecrit le tableau dans `dossier/nom.<ext>` (`tsv` s'ecrit `.txt`) et rend le chemin.
pub fn ecrire(t: &mut Tableau, dossier: &Path, nom: &str, ctx: &Contexte) -> R<PathBuf> {
    std::fs::create_dir_all(dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    let ext = if t.format == "tsv" { "txt" } else { t.format.as_str() };
    let chemin = dossier.join(format!("{}.{}", nom, ext));
    let contenu = octets(t, Some(ctx))?;
    std::fs::write(&chemin, contenu).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    t.fichier = Some(chemin.clone());
    Ok(chemin)
}

// ------------------------------------------------------------------ petits outils

/// Le nom du mois `AAAA-MM` : « Septembre » (`date::nom_du_mois`, graphie `Titre`). Un mois hors 01-12 se refuse.
pub fn nom_du_mois(m: &str) -> R<&'static str> {
    let tranche: String = m.chars().skip(5).take(2).collect();
    let n = socle::num::int_py(&tranche).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(&tranche))))?;
    socle::date::nom_du_mois(i64::try_from(n).unwrap_or(0), socle::date::Graphie::Titre).map_err(|e| erreur("IndexError", e))
}

/// `_libelle_mois` : « Septembre 2026 ».
pub fn libelle_mois(m: &str) -> R<String> {
    Ok(format!("{} {}", nom_du_mois(m)?, m.chars().take(4).collect::<String>()))
}

/// `_jj` : AAAA-MM-JJ -> JJ/MM/AAAA ; le reste tel quel.
pub fn jj(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() >= 10 && c[4] == '-' {
        format!("{}/{}/{}", c[8..10].iter().collect::<String>(), c[5..7].iter().collect::<String>(), c[..4].iter().collect::<String>())
    } else {
        iso.to_string()
    }
}

/// `_qui` : « NOM Prenom ».
pub fn qui(fiche: &Value) -> R<String> {
    let prenom = match fiche.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => fiche.get("prenom").map(valeur_en_texte).unwrap_or_default(),
    };
    let nom = fiche.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
    Ok(format!("{} {}", valeur_en_texte(nom), prenom))
}

/// `_qui` d'une fiche qui n'a peut-etre pas de nom (`.get("nom", "")`).
pub fn qui_souple(fiche: &Value) -> String {
    let prenom = match fiche.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => fiche.get("prenom").map(valeur_en_texte).unwrap_or_default(),
    };
    format!("{} {}", fiche.get("nom").map(valeur_en_texte).unwrap_or_default(), prenom)
}

/// `_montant` : un montant declare, ou `None` quand il ne l'a pas ete (une chaine vide n'est pas un zero).
pub fn montant(v: Option<&Value>) -> R<Option<Decimal>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(x) => {
            let t = valeur_en_texte(x);
            if t.trim_matches(socle::texte::blanc).is_empty() {
                return Ok(None);
            }
            let d = socle::num::decimal_py(&t).ok_or_else(|| erreur("InvalidOperation", socle::num::illisible(&t)))?;
            Ok(Some(socle::num::eur2(d)))
        }
    }
}
