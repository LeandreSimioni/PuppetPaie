//! `moteur/journal.py`, la part qui LIT le journal des depots : les etats du
//! mois, les resumes, la repetition, le rattachement des retours. Rien n'y
//! calcule de paie.
//!
//! Les lignes du journal restent des `serde_json::Value`, lues comme Python
//! lit des dictionnaires : une cle absente vaut `None`, une valeur fausse
//! (`None`, `0`, `""`, `[]`, `{}`) cede sa place a un `or`, et ce que Python
//! refuserait (un `.get` sur autre chose qu'un dictionnaire, une boucle sur
//! `None`) est refuse sous le meme nom d'exception (`AttributeError`,
//! `TypeError`, `KeyError`). Les cles de dictionnaire Python (`(salarie,
//! debut)`) se rendent en listes `[salarie, debut, valeur]`, dans l'ordre
//! d'insertion de Python.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

use socle::ecriture::ecrire_json;
use socle::fichiers::vrai;
use crate::journal::{chemin, ACTES, REEL};
use crate::retours;
use socle::echec::{erreur, Echec, R};
use socle::valeur::{valeurs_egales, nom_type, nombre, valeur_ecrite, valeur_en_texte};
use crate::tables_journal::ALNUM;

pub(crate) const DSN: &str = "dsn";
pub(crate) const DPAE: &str = "dpae";
pub(crate) const DOCUMENTS: &str = "documents";
pub(crate) const SIGNATURE: &str = "contrat-signature";
pub(crate) const CONTRAT_RECU: &str = "contrat-recu";
const CONTRESIGNE: &str = "contrat-contresigne";
const CASC: &str = "regularisation-casc";
const TEST: &str = "test";
const NATURE_CERTIFICAT_DPAE: &str = "41";

/// `dpae_api.NATURES` : les retours du canal DPAE.
pub const NATURES_DPAE: [(&str, &str); 4] = [
    ("10", "avis de rejet du depot"),
    ("11", "certificat de conformite / bilan d'anomalies"),
    ("20", "bilan d'identification des salaries"),
    ("41", "certificat de conformite des DPAE"),
];

static NUL: Value = Value::Null;
pub(crate) static LISTE_VIDE: Value = Value::Array(Vec::new());

pub(crate) fn objet_vide() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| Value::Object(Map::new()))
}

pub(crate) fn texte_vide() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| Value::String(String::new()))
}

/// `v.get(k)`.
pub(crate) fn get<'a>(v: &'a Value, k: &str) -> R<&'a Value> {
    getd(v, k, &NUL)
}

/// `v.get(k, defaut)`.
pub(crate) fn getd<'a>(v: &'a Value, k: &str, defaut: &'a Value) -> R<&'a Value> {
    match v {
        Value::Object(o) => Ok(o.get(k).unwrap_or(defaut)),
        _ => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v)))),
    }
}

/// `v or defaut`.
pub(crate) fn ou<'a>(v: &'a Value, defaut: &'a Value) -> &'a Value {
    if vrai(v) {
        v
    } else {
        defaut
    }
}

pub(crate) fn est(v: &Value, s: &str) -> bool {
    v.as_str() == Some(s)
}

/// `for x in v` : une liste, les cles d'un dictionnaire, les caracteres d'un texte.
pub(crate) fn items(v: &Value) -> R<Vec<Cow<'_, Value>>> {
    match v {
        Value::Array(a) => Ok(a.iter().map(Cow::Borrowed).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Cow::Owned(Value::String(k.clone()))).collect()),
        Value::String(s) => Ok(s.chars().map(|c| Cow::Owned(Value::String(c.to_string()))).collect()),
        _ => Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(v)))),
    }
}

pub(crate) fn s(t: &str) -> Value {
    Value::String(t.to_string())
}

// ---------------------------------------------------- les dictionnaires Python

/// La cle de hachage d'une valeur : `1`, `1.0` et `True` ne font qu'une ;
/// une liste ou un dictionnaire ne se hache pas.
fn cle_py(v: &Value) -> R<String> {
    match v {
        Value::Null => Ok("N".into()),
        Value::String(t) => Ok(format!("s{}:{}", t.len(), t)),
        Value::Array(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        autre => {
            let x = nombre(autre).unwrap_or(0.0);
            Ok(format!("n{:?}", if x == 0.0 { 0.0 } else { x }))
        }
    }
}

/// Un `dict` (ou un `set`) a cle composee, dans l'ordre d'insertion : reposer
/// une cle change la valeur et garde la place.
pub(crate) struct Dico<V> {
    pub(crate) index: HashMap<String, usize>,
    pub(crate) items: Vec<(Vec<Value>, V)>,
}

impl<V> Dico<V> {
    pub(crate) fn nouveau() -> Self {
        Dico { index: HashMap::new(), items: Vec::new() }
    }

    pub(crate) fn cle(k: &[Value]) -> R<String> {
        let mut t = String::new();
        for x in k {
            t.push_str(&cle_py(x)?);
            t.push('\u{1f}');
        }
        Ok(t)
    }

    pub(crate) fn get(&self, k: &[Value]) -> R<Option<&V>> {
        Ok(self.index.get(&Self::cle(k)?).map(|&i| &self.items[i].1))
    }

    pub(crate) fn get_mut(&mut self, k: &[Value]) -> R<Option<&mut V>> {
        let i = self.index.get(&Self::cle(k)?).copied();
        Ok(i.map(move |i| &mut self.items[i].1))
    }

    pub(crate) fn contient(&self, k: &[Value]) -> R<bool> {
        Ok(self.index.contains_key(&Self::cle(k)?))
    }

    pub(crate) fn poser(&mut self, k: Vec<Value>, v: V) -> R<()> {
        let c = Self::cle(&k)?;
        match self.index.get(&c) {
            Some(&i) => self.items[i].1 = v,
            None => {
                self.index.insert(c, self.items.len());
                self.items.push((k, v));
            }
        }
        Ok(())
    }

    pub(crate) fn sortie(&self, f: impl Fn(&V) -> Value) -> Value {
        Value::Array(
            self.items
                .iter()
                .map(|(k, v)| {
                    let mut l = k.clone();
                    l.push(f(v));
                    Value::Array(l)
                })
                .collect(),
        )
    }

    pub(crate) fn cles(&self) -> Value {
        Value::Array(self.items.iter().map(|(k, _)| Value::Array(k.clone())).collect())
    }
}

fn paire(l: &Value, a: &str, b: &str) -> R<Vec<Value>> {
    Ok(vec![get(l, a)?.clone(), get(l, b)?.clone()])
}

/// Sous quelle cle le journal range un contrat. `Paire` : (salarie, premier jour), la maille historique, que les
/// operations offertes au Python gardent. `Contrat` : (salarie, premier jour, numero), la maille des ecrans (Retours Windows,
/// B, suite) : deux contrats du meme salarie qui commencent le meme jour ont chacun leur etat, leur relance, leur
/// avenant. Le numero vaut `""` quand la ligne n'en porte pas (une ligne d'avant les numeros).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Maille {
    Paire,
    Contrat,
}

/// (salarie, premier jour, numero) : la cle d'un contrat, hors du journal.
pub(crate) type Cle3 = (String, String, String);

/// Le numero d'une ligne de contrat, tel qu'il sert de cle : son texte, ou `""`.
pub(crate) fn numero_de_cle(v: &Value) -> Value {
    Value::String(if vrai(v) { valeur_en_texte(v) } else { String::new() })
}

/// La cle d'une ligne de contrat (une entree de `detail.contrats`, ou le `detail` d'un `contrat-recu`).
pub(crate) fn cle_contrat(l: &Value, m: Maille) -> R<Vec<Value>> {
    let mut k = paire(l, "salarie", "debut")?;
    if m == Maille::Contrat {
        k.push(numero_de_cle(get(l, "numero")?));
    }
    Ok(k)
}

/// Les numeros distincts, dans l'ordre du journal, sous lesquels le journal connait un contrat de ce salarie et de ce debut : parti
/// a la signature (avenants compris : un avenant porte le numero de son contrat) ou note recu.
pub(crate) fn numeros_demandes(lignes: &[Value], salarie: &str, debut: &str) -> R<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for l in lignes {
        // Un contrat note « recu » a la main sans avoir ete demande par courriel est connu du journal sous son numero aussi.
        if est(get(l, "quoi")?, CONTRAT_RECU) && accepte(l)? {
            let d = ou(get(l, "detail")?, objet_vide());
            if est(get(d, "salarie")?, salarie) && est(get(d, "debut")?, debut) {
                let n = valeur_en_texte(&numero_de_cle(get(d, "numero")?));
                if !out.contains(&n) {
                    out.push(n);
                }
            }
            continue;
        }
        if !est(get(l, "quoi")?, SIGNATURE) || !accepte(l)? {
            continue;
        }
        for c in detail_ou_liste(l, "contrats")? {
            if est(get(&c, "salarie")?, salarie) && est(get(&c, "debut")?, debut) {
                let n = valeur_en_texte(&numero_de_cle(get(&c, "numero")?));
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }
    Ok(out)
}

/// Pour les paies d'UN meme (salarie, debut), dans l'ordre de la saisie, le numero SOUS LEQUEL LE JOURNAL parle de chacune :
/// `propres` = les numeros que la saisie leur donne, `demandes` = ceux sous lesquels des contrats sont partis.
/// 1. une paie dont le numero est parmi les demandes garde le sien ;
/// 2. les paies restantes et les numeros demandes que personne ne reclame se jumellent dans l'ordre, SI elles sont aussi
///    nombreuses (un contrat renumerote depuis son envoi retrouve ainsi sa demande) ;
/// 3. sinon une paie reste sans demande : le contrat jumeau qui n'est pas parti ne prend pas la place de celui qui est parti.
pub(crate) fn apparier(propres: &[String], demandes: &[String]) -> Vec<Option<String>> {
    let mut sortie: Vec<Option<String>> = propres.iter().map(|n| demandes.contains(n).then(|| n.clone())).collect();
    let libres: Vec<&String> = demandes.iter().filter(|d| !propres.contains(d)).collect();
    let restantes: Vec<usize> = (0..propres.len()).filter(|&i| sortie[i].is_none()).collect();
    if !libres.is_empty() && libres.len() == restantes.len() {
        for (i, d) in restantes.iter().zip(libres) {
            sortie[*i] = Some(d.clone());
        }
    }
    sortie
}

/// Les cles sous lesquelles chaque paie (salarie, debut, numero propre) d'un mois retrouve son contrat dans le journal :
/// d'abord celle du contrat demande qui lui correspond (`apparier`), puis la sienne, puis celle d'une ligne sans numero.
pub(crate) fn cles_des_paies(lignes: &[Value], paies: &[Cle3]) -> R<Vec<Vec<Cle3>>> {
    let mut sortie: Vec<Vec<Cle3>> = vec![Vec::new(); paies.len()];
    let mut vus: Vec<(String, String)> = Vec::new();
    for (s, d, _) in paies {
        let sd = (s.clone(), d.clone());
        if vus.contains(&sd) {
            continue;
        }
        vus.push(sd);
        let rangs: Vec<usize> = (0..paies.len()).filter(|&i| paies[i].0 == *s && paies[i].1 == *d).collect();
        let propres: Vec<String> = rangs.iter().map(|&i| paies[i].2.clone()).collect();
        let demandes = numeros_demandes(lignes, s, d)?;
        let appariees = apparier(&propres, &demandes);
        for (j, &i) in rangs.iter().enumerate() {
            let mut cles: Vec<Cle3> = Vec::new();
            let mut ajouter = |n: &str| {
                let k = (s.clone(), d.clone(), n.to_string());
                if !cles.contains(&k) {
                    cles.push(k);
                }
            };
            if let Some(n) = &appariees[j] {
                ajouter(n);
            }
            ajouter(&propres[j]);
            ajouter("");
            sortie[i] = cles;
        }
    }
    Ok(sortie)
}

// ------------------------------------------------------------ lire le journal

fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, format!("{} : {}", p.display(), e))
}

/// Toutes les lignes de `journal/depots.json` (aucune si le fichier n'existe pas).
pub(crate) fn charger(racine: &Path) -> R<Vec<Value>> {
    Ok((*charger_partage(racine)?).clone())
}

/// Les lignes de `journal/depots.json`, PARTAGEES : lues une fois tant que le fichier ne change pas (chemin, date,
/// taille ; il s'ecrit toujours d'un coup, `ecriture::ecrire_json`), et servies a chaque route qui les relit -- la
/// fenetre garde le moteur charge (ticket #244). Un refus ne se garde pas.
pub(crate) fn charger_partage(racine: &Path) -> R<std::sync::Arc<Vec<Value>>> {
    type Cache = std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (Option<(u128, u64)>, std::sync::Arc<Vec<Value>>)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let p = chemin(racine);
    let sig = socle::fichiers::signature(&p);
    if let Some((s, l)) = sig.and_then(|_| cache.lock().unwrap_or_else(|e| e.into_inner()).get(&p).cloned()) {
        if s == sig {
            return Ok(l);
        }
    }
    let l = std::sync::Arc::new(charger_du_disque(&p)?);
    if sig.is_some() {
        socle::fichiers::ranger(cache, p, (sig, l.clone()));
    }
    Ok(l)
}

fn charger_du_disque(p: &Path) -> R<Vec<Value>> {
    if !p.exists() {
        return Ok(Vec::new());
    }
    let octets = std::fs::read(&p).map_err(|e| erreur_io(&e, &p))?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))?;
    let v: Value = serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))?;
    Ok(items(&v)?.into_iter().map(Cow::into_owned).collect())
}

/// `journal.lire` sur des lignes deja lues.
pub(crate) fn filtrer<'a>(lignes: &'a [Value], quoi: Option<&str>, mois: Option<&str>, reel: bool) -> R<Vec<&'a Value>> {
    let mut v: Vec<&Value> = lignes.iter().collect();
    if let Some(q) = quoi.filter(|q| !q.is_empty()) {
        let mut k = Vec::new();
        for l in v {
            if est(get(l, "quoi")?, q) {
                k.push(l);
            }
        }
        v = k;
    }
    if let Some(m) = mois.filter(|m| !m.is_empty()) {
        let mut k = Vec::new();
        for l in v {
            if est(get(l, "mois")?, m) {
                k.push(l);
            }
        }
        v = k;
    }
    if reel {
        let mut k = Vec::new();
        for l in v {
            if est(get(l, "mode")?, REEL) {
                k.push(l);
            }
        }
        v = k;
    }
    Ok(v)
}

pub(crate) fn accepte(l: &Value) -> R<bool> {
    Ok(vrai(get(l, "accepte")?))
}

/// `l.get("detail", {}).get(k, [])`, parcouru.
fn detail_liste<'a>(l: &'a Value, k: &str) -> R<Vec<Cow<'a, Value>>> {
    items(getd(getd(l, "detail", objet_vide())?, k, &LISTE_VIDE)?)
}

/// `(l.get("detail") or {}).get(k, [])`, parcouru.
fn detail_ou_liste<'a>(l: &'a Value, k: &str) -> R<Vec<Cow<'a, Value>>> {
    items(getd(ou(get(l, "detail")?, objet_vide()), k, &LISTE_VIDE)?)
}

// ------------------------------------------------------------ les embauches

pub(crate) fn embauches_par_depot(lignes: &[Value]) -> R<Dico<Value>> {
    let mut d = Dico::nouveau();
    for l in filtrer(lignes, Some(DPAE), None, true)? {
        if !accepte(l)? {
            continue;
        }
        for e in detail_liste(l, "embauches")? {
            d.poser(paire(&e, "salarie", "debut")?, get(l, "quand")?.clone())?;
        }
    }
    Ok(d)
}

/// `journal.de_l_etablissement` : `principal` est le NIC de la fiche entreprise.
pub(crate) fn de_l_etablissement(lignes: &[Value], nic: &Value, principal: &str) -> R<Vec<Value>> {
    let voulu = if vrai(nic) { valeur_en_texte(nic) } else { principal.to_string() };
    let mut out = Vec::new();
    for l in lignes {
        let e = get(ou(get(l, "detail")?, objet_vide()), "etablissement")?;
        let de_la_ligne = if vrai(e) { valeur_en_texte(e) } else { principal.to_string() };
        if de_la_ligne == voulu {
            out.push(l.clone());
        }
    }
    Ok(out)
}

pub(crate) fn etat_dsn_du_mois(lignes: &[Value], mois: &str, nic: &Value, principal: &str) -> R<Value> {
    let du_mois: Vec<Value> = filtrer(lignes, Some(DSN), Some(mois), true)?.into_iter().cloned().collect();
    let mut faites = Vec::new();
    for l in de_l_etablissement(&du_mois, nic, principal)? {
        if accepte(&l)? {
            faites.push(l);
        }
    }
    let Some(l) = faites.last() else {
        return Ok(Value::Null);
    };
    let retours = items(ou(get(l, "retours")?, &LISTE_VIDE))?;
    let mut verdicts = Vec::new();
    let mut refus = Vec::new();
    let mut aer = false;
    for r in &retours {
        let nature = valeur_en_texte(get(r, "nature")?);
        if nature == "11" {
            verdicts.push(ou(get(r, "verdict")?, texte_vide()));
        }
    }
    for r in &retours {
        if est(ou(get(r, "verdict")?, texte_vide()), "KO") {
            refus.push(r);
        }
    }
    for r in &retours {
        if valeur_en_texte(get(r, "nature")?) == "46" {
            aer = true;
        }
    }
    let mut anomalies = Vec::new();
    for r in &refus {
        for a in items(ou(get(r, "anomalies")?, &LISTE_VIDE))? {
            anomalies.push(a.into_owned());
        }
    }
    // UN DEPOT REPRIS DES RETOURS (`reprise_journal`, ticket #215) : sa piece EST le bilan de la plate-forme, et la ligne
    // en garde le verdict (`detail.etat_declaration`) -- tant qu'aucun retour n'est range en face, c'est lui qui dit.
    let detail = ou(get(l, "detail")?, objet_vide());
    let repris = if verdicts.is_empty() && vrai(getd(detail, "reconstruit", &Value::Null)?) {
        Some(valeur_en_texte(getd(detail, "etat_declaration", texte_vide())?).to_uppercase())
    } else {
        None
    };
    let refus_repris = repris.as_deref() == Some("KO");
    Ok(json!({
        "quand": get(l, "quand")?, "fichier": get(l, "fichier")?, "flux": get(l, "flux")?,
        "passee": (verdicts.iter().any(|v| est(v, "OK")) || repris.as_deref() == Some("OK")) && refus.is_empty(),
        "refusee": !refus.is_empty() || refus_repris,
        "anomalies": anomalies,
        "aer_recue": aer,
    }))
}

/// `journal._meme_nom`.
fn meme_nom(cle: &Value, nom: &Value) -> R<bool> {
    fn plat(v: &Value) -> R<String> {
        let t = match v {
            Value::String(t) => t.as_str(),
            autre if !vrai(autre) => "",
            autre => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
        };
        Ok(t.to_uppercase().chars().filter(|&c| alnum_py(c)).collect())
    }
    let a = plat(cle)?;
    if a.is_empty() {
        return Ok(false);
    }
    Ok(a == plat(nom)?)
}

/// `str.isalnum()` de Python.
pub(crate) fn alnum_py(c: char) -> bool {
    let c = c as u32;
    ALNUM
        .binary_search_by(|&(a, b)| {
            if c < a {
                std::cmp::Ordering::Greater
            } else if c > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn dossiers_urssaf(retours: &[Cow<'_, Value>]) -> R<Dico<Value>> {
    let mut out = Dico::nouveau();
    for r in retours {
        if valeur_en_texte(get(r, "nature")?) != NATURE_CERTIFICAT_DPAE {
            continue;
        }
        for l in items(ou(get(r, "dpae")?, &LISTE_VIDE))? {
            out.poser(paire(&l, "nom", "embauche")?, l.into_owned())?;
        }
    }
    Ok(out)
}

pub(crate) fn etat_des_embauches(lignes: &[Value]) -> R<Value> {
    let mut etat: Dico<Value> = Dico::nouveau();
    for l in filtrer(lignes, Some(DPAE), None, true)? {
        if !accepte(l)? {
            continue;
        }
        let retours = items(ou(get(l, "retours")?, &LISTE_VIDE))?;
        let mut passee = false;
        for r in &retours {
            let nature = valeur_en_texte(get(r, "nature")?);
            if nature == NATURE_CERTIFICAT_DPAE || (nature == "11" && est(ou(get(r, "verdict")?, texte_vide()), "OK")) {
                passee = true;
            }
        }
        let mut refus = Vec::new();
        for r in &retours {
            if est(ou(get(r, "verdict")?, texte_vide()), "KO") {
                refus.push(r);
            }
        }
        let refusee = !refus.is_empty() && !passee;
        let mut anomalies = Vec::new();
        for r in &refus {
            for a in items(ou(get(r, "anomalies")?, &LISTE_VIDE))? {
                anomalies.push(a.into_owned());
            }
        }
        let dossiers = dossiers_urssaf(&retours)?;
        let retours_json: Vec<Value> = retours.iter().map(|r| (**r).clone()).collect();
        for e in detail_liste(l, "embauches")? {
            let mut cert: Option<&Value> = None;
            for ((nom, emb), v) in dossiers.items.iter().map(|(k, v)| ((&k[0], &k[1]), v)) {
                if valeurs_egales(emb, get(&e, "debut")?) && meme_nom(get(&e, "salarie")?, nom)? {
                    cert = Some(v);
                    break;
                }
            }
            let (dossier, existante) = match cert {
                Some(c) if vrai(c) => (ou(get(c, "dossier")?, texte_vide()).clone(), vrai(get(c, "deja_existante")?)),
                _ => (texte_vide().clone(), false),
            };
            etat.poser(
                paire(&e, "salarie", "debut")?,
                json!({
                    "quand": get(l, "quand")?, "fichier": get(l, "fichier")?, "fin": get(&e, "fin")?,
                    "passee": passee, "refusee": refusee, "anomalies": anomalies,
                    "retours": retours_json, "dossier": dossier, "deja_existante": existante,
                }),
            )?;
        }
    }
    Ok(etat.sortie(Clone::clone))
}

// -------------------------------------------------------------- les contrats

/// Les lignes telles que la maille `Contrat` les lit : une ligne `contrat-recu` SANS numero (d'avant les numeros, ou posee sans que
/// la saisie en porte un) vaut pour le dernier contrat que le journal a vu partir pour ce salarie a ce debut. En maille `Paire`, rien
/// ne change.
fn normalisees(lignes: &[Value], m: Maille) -> Cow<'_, [Value]> {
    if m == Maille::Paire {
        return Cow::Borrowed(lignes);
    }
    let mut dernier: HashMap<(String, String), String> = HashMap::new();
    let mut out: Vec<Value> = Vec::with_capacity(lignes.len());
    for l in lignes {
        let mut l = l.clone();
        if est(l.get("quoi").unwrap_or(&NUL), SIGNATURE) {
            if let Some(Value::Array(cs)) = l.get("detail").and_then(|d| d.get("contrats")) {
                for c in cs {
                    if let (Some(s), Some(d)) = (c.get("salarie").and_then(Value::as_str), c.get("debut").and_then(Value::as_str)) {
                        if est(c.get("acte").unwrap_or(&NUL), "avenant") {
                            continue;
                        }
                        dernier.insert((s.to_string(), d.to_string()), valeur_en_texte(&numero_de_cle(c.get("numero").unwrap_or(&NUL))));
                    }
                }
            }
        } else if est(l.get("quoi").unwrap_or(&NUL), CONTRAT_RECU) {
            if let Some(Value::Object(d)) = l.get_mut("detail") {
                if !vrai(d.get("numero").unwrap_or(&NUL)) {
                    if let (Some(s), Some(db)) = (d.get("salarie").and_then(Value::as_str), d.get("debut").and_then(Value::as_str)) {
                        if let Some(n) = dernier.get(&(s.to_string(), db.to_string())) {
                            let n = n.clone();
                            d.insert("numero".into(), Value::String(n));
                        }
                    }
                }
            }
        }
        out.push(l);
    }
    Cow::Owned(out)
}

/// `d[cle]`, ou, a defaut, la ligne du meme contrat sans numero (une ligne d'avant les numeros).
pub(crate) fn avec_repli<'a, V>(d: &'a Dico<V>, k: &[Value]) -> R<Option<&'a V>> {
    if let Some(v) = d.get(k)? {
        return Ok(Some(v));
    }
    if k.len() == 3 && vrai(&k[2]) {
        return d.get(&[k[0].clone(), k[1].clone(), Value::String(String::new())]);
    }
    Ok(None)
}

pub(crate) fn contrats_demandes(lignes: &[Value], m: Maille) -> R<Value> {
    let lignes = &*normalisees(lignes, m);
    let mut d = Dico::nouveau();
    for l in filtrer(lignes, Some(SIGNATURE), None, false)? {
        if !accepte(l)? {
            continue;
        }
        for c in detail_liste(l, "contrats")? {
            d.poser(cle_contrat(&c, m)?, get(l, "quand")?.clone())?;
        }
    }
    Ok(d.sortie(Clone::clone))
}

pub(crate) fn numeros_contrats_partis(lignes: &[Value], m: Maille) -> R<Value> {
    let lignes = &*normalisees(lignes, m);
    let mut d = Dico::nouveau();
    for l in filtrer(lignes, Some(SIGNATURE), None, false)? {
        if !accepte(l)? {
            continue;
        }
        for c in detail_ou_liste(l, "contrats")? {
            if vrai(get(&c, "numero")?) {
                d.poser(cle_contrat(&c, m)?, get(&c, "numero")?.clone())?;
            }
        }
    }
    Ok(d.sortie(Clone::clone))
}

/// Les contrats repartis apres leur signature sur une autre version de l'engagement que celle signee : ils attendent une
/// nouvelle signature. Un avenant n'en fait pas un.
pub(crate) fn renvoyes_depuis_la_signature(lignes: &[Value], m: Maille) -> R<Dico<()>> {
    let lignes = &*normalisees(lignes, m);
    let mut envoye: Dico<Value> = Dico::nouveau();
    let mut signe: Dico<Value> = Dico::nouveau();
    let mut renvoye: Dico<()> = Dico::nouveau();
    for l in lignes {
        if !accepte(l)? {
            continue;
        }
        if est(get(l, "quoi")?, SIGNATURE) {
            for c in detail_ou_liste(l, "contrats")? {
                let k = cle_contrat(&c, m)?;
                // La version de l'engagement sur laquelle ce contrat part (`journal::versions_des_contrats`).
                let e = get(l, "versions")?
                    .as_array()
                    .and_then(|v| v.iter().find(|x| est(get(x, "salarie").unwrap_or(&Value::Null), &valeur_en_texte(get(&c, "salarie").unwrap_or(&Value::Null))) && valeurs_egales(get(x, "numero").unwrap_or(&Value::Null), get(&c, "numero").unwrap_or(&Value::Null))))
                    .and_then(|x| x.get("version").cloned())
                    .unwrap_or(Value::Null);
                if est(get(&c, "acte")?, "avenant") {
                    envoye.poser(k, e)?;
                    continue;
                }
                if let Some(deja) = signe.get(&k)? {
                    if vrai(&e) && vrai(deja) && !valeurs_egales(&e, deja) {
                        renvoye.poser(k.clone(), ())?;
                    }
                }
                envoye.poser(k, e)?;
            }
        } else if est(get(l, "quoi")?, CONTRAT_RECU) {
            let d = ou(get(l, "detail")?, objet_vide());
            if vrai(get(d, "recu")?) {
                let k = cle_contrat(d, m)?;
                let v = envoye.get(&k)?.cloned().unwrap_or(Value::Null);
                signe.poser(k.clone(), v)?;
                if let Some(i) = renvoye.index.remove(&Dico::<()>::cle(&k)?) {
                    renvoye.items.remove(i);
                    for x in renvoye.index.values_mut() {
                        if *x > i {
                            *x -= 1;
                        }
                    }
                }
            }
        }
    }
    Ok(renvoye)
}

/// `contrats_recus` : (salarie, debut) -> quand, pour ceux dont le dernier mot est « recu ».
pub(crate) fn contrats_recus(lignes: &[Value], m: Maille) -> R<Dico<Value>> {
    let lignes = &*normalisees(lignes, m);
    let mut etat: Dico<Value> = Dico::nouveau();
    for l in filtrer(lignes, Some(CONTRAT_RECU), None, false)? {
        if !accepte(l)? {
            continue;
        }
        let d = getd(l, "detail", objet_vide())?;
        let k = cle_contrat(d, m)?;
        let v = if vrai(get(d, "recu")?) { get(l, "quand")?.clone() } else { Value::Null };
        etat.poser(k, v)?;
    }
    garder_vrais(etat)
}

fn garder_vrais(d: Dico<Value>) -> R<Dico<Value>> {
    let mut out = Dico::nouveau();
    for (k, v) in d.items {
        if vrai(&v) {
            out.poser(k, v)?;
        }
    }
    Ok(out)
}

pub(crate) fn contrats_recus_comment(lignes: &[Value], m: Maille) -> R<Value> {
    let lignes = &*normalisees(lignes, m);
    let mut etat: Dico<Value> = Dico::nouveau();
    for l in filtrer(lignes, Some(CONTRAT_RECU), None, false)? {
        if !accepte(l)? {
            continue;
        }
        let d = getd(l, "detail", objet_vide())?;
        let k = cle_contrat(d, m)?;
        let main = s("main");
        let v = if vrai(get(d, "recu")?) { ou(get(d, "origine")?, &main).clone() } else { Value::Null };
        etat.poser(k, v)?;
    }
    Ok(garder_vrais(etat)?.sortie(Clone::clone))
}

pub(crate) fn signatures_attendues(lignes: &[Value], mois: Option<&str>, salaries: &Value) -> R<Value> {
    // Par CONTRAT (salarie, debut, numero) : une reponse ne releve que le contrat qu'elle signe (Retours Windows, B, suite).
    let lignes = &*normalisees(lignes, Maille::Contrat);
    let recus = contrats_recus(lignes, Maille::Contrat)?;
    let renvoyes = renvoyes_depuis_la_signature(lignes, Maille::Contrat)?;
    let mut derniere_recue: Dico<usize> = Dico::nouveau();
    for (i, l) in lignes.iter().enumerate() {
        let d = ou(get(l, "detail")?, objet_vide());
        if est(get(l, "quoi")?, CONTRAT_RECU) && accepte(l)? && vrai(get(d, "recu")?) {
            derniere_recue.poser(cle_contrat(d, Maille::Contrat)?, i)?;
        }
    }
    struct Demande {
        code: Value,
        mois: Value,
        quand: Value,
        salarie: Value,
        adresse: Value,
        fichier: Value,
        acte: Value,
        avenant: Value,
        rang: usize,
        apres_signature: bool,
        envoyes: Dico<()>,
        prenom: Value,
        nom: String,
        contrats: Vec<Value>,
    }
    let mut par_code: Dico<usize> = Dico::nouveau();
    let mut demandes: Vec<Demande> = Vec::new();
    for (i, l) in lignes.iter().enumerate() {
        if !est(get(l, "quoi")?, SIGNATURE) || !accepte(l)? {
            continue;
        }
        if let Some(m) = mois.filter(|m| !m.is_empty()) {
            if !est(get(l, "mois")?, m) {
                continue;
            }
        }
        for c in detail_liste(l, "contrats")? {
            let code = get(&c, "code")?;
            if !vrai(code) {
                continue;
            }
            let k = cle_contrat(&c, Maille::Contrat)?;
            let acte = get(&c, "acte")?;
            let apres_signature = est(acte, "avenant") || est(acte, "remplace") || avec_repli(&renvoyes, &k)?.is_some();
            let existant = par_code.get(std::slice::from_ref(code))?.copied();
            let n = match existant {
                Some(n) => n,
                None => {
                    let salarie = get(&c, "salarie")?;
                    let vide = objet_vide();
                    let fiche = match salarie {
                        Value::String(nom) => salaries.get(nom).unwrap_or(vide),
                        autre => {
                            cle_py(autre)?;
                            vide
                        }
                    };
                    let prenom = ou(get(fiche, "prenom_affichage")?, getd(fiche, "prenom", texte_vide())?).clone();
                    let nom_aff = ou(get(fiche, "nom_affichage")?, getd(fiche, "nom", salarie)?);
                    demandes.push(Demande {
                        code: code.clone(),
                        mois: get(l, "mois")?.clone(),
                        quand: get(l, "quand")?.clone(),
                        salarie: salarie.clone(),
                        adresse: getd(&c, "adresse", texte_vide())?.clone(),
                        fichier: getd(&c, "fichier", texte_vide())?.clone(),
                        acte: ou(acte, &s("contrat")).clone(),
                        avenant: get(&c, "avenant")?.clone(),
                        rang: i,
                        apres_signature: false,
                        envoyes: Dico::nouveau(),
                        nom: format!("{} {}", valeur_en_texte(&prenom), valeur_en_texte(nom_aff)),
                        prenom,
                        contrats: Vec::new(),
                    });
                    par_code.poser(vec![code.clone()], demandes.len() - 1)?;
                    demandes.len() - 1
                }
            };
            let e = &mut demandes[n];
            if vrai(get(&c, "message")?) {
                e.envoyes.poser(vec![get(&c, "message")?.clone()], ())?;
            }
            e.apres_signature = e.apres_signature || apres_signature;
            e.contrats.push(json!({
                "salarie": get(&c, "salarie")?, "debut": get(&c, "debut")?,
                "fin": get(&c, "fin")?, "numero": get(&c, "numero")?,
                "spectacle": getd(&c, "spectacle", texte_vide())?,
                "fichier": getd(&c, "fichier", texte_vide())?, "avenant": get(&c, "avenant")?,
                "mois": ou(get(&c, "mois")?, get(l, "mois")?),
            }));
        }
    }
    let mut out = Vec::new();
    for e in &demandes {
        let mut attendue = false;
        for c in &e.contrats {
            let k = cle_contrat(c, Maille::Contrat)?;
            let revenu = if e.apres_signature {
                avec_repli(&derniere_recue, &k)?.is_some_and(|&rang| rang > e.rang)
            } else {
                avec_repli(&recus, &k)?.is_some()
            };
            if !revenu {
                attendue = true;
                break;
            }
        }
        if attendue {
            out.push(json!({
                "code": e.code, "mois": e.mois, "quand": e.quand, "salarie": e.salarie,
                "adresse": e.adresse, "fichier": e.fichier, "acte": e.acte, "avenant": e.avenant,
                "rang": e.rang, "apres_signature": e.apres_signature,
                "envoyes": e.envoyes.items.iter().map(|(k, _)| k[0].clone()).collect::<Vec<_>>(),
                "prenom": e.prenom, "nom": e.nom, "contrats": e.contrats,
            }));
        }
    }
    Ok(Value::Array(out))
}

pub(crate) fn avenants(lignes: &[Value], m: Maille) -> R<Value> {
    let lignes = &*normalisees(lignes, m);
    let mut out: Dico<Vec<Value>> = Dico::nouveau();
    for l in lignes {
        if !accepte(l)? {
            continue;
        }
        let d = ou(get(l, "detail")?, objet_vide());
        if est(get(l, "quoi")?, SIGNATURE) {
            for c in items(getd(d, "contrats", &LISTE_VIDE)?)? {
                if !est(get(&c, "acte")?, "avenant") {
                    continue;
                }
                let k = cle_contrat(&c, m)?;
                if !out.contient(&k)? {
                    out.poser(k.clone(), Vec::new())?;
                }
                let liste = out.get_mut(&k)?.unwrap();
                let numero = get(&c, "avenant")?;
                liste.retain(|a| !valeurs_egales(&a["numero"], numero));
                liste.push(json!({
                    "numero": numero, "quand": get(l, "quand")?,
                    "mois": ou(get(&c, "mois")?, get(l, "mois")?),
                    "fichier": get(&c, "fichier")?, "code": get(&c, "code")?,
                    "modifications": ou(get(&c, "modifications")?, &LISTE_VIDE),
                    "signe": null,
                }));
            }
        } else if est(get(l, "quoi")?, CONTRAT_RECU) && vrai(get(d, "recu")?) {
            let k = cle_contrat(d, m)?;
            // Une ligne sans numero (d'avant les numeros) vaut pour tous les contrats du salarie a ce debut.
            let cibles: Vec<Vec<Value>> = if m == Maille::Contrat && !vrai(&k[2]) {
                out.items.iter().map(|(c, _)| c.clone()).filter(|c| c[..2] == k[..2]).collect()
            } else {
                vec![k]
            };
            for k in cibles {
                if let Some(liste) = out.get_mut(&k)? {
                    for a in liste.iter_mut() {
                        if a["signe"].is_null() {
                            a["signe"] = get(l, "quand")?.clone();
                        }
                    }
                }
            }
        }
    }
    Ok(out.sortie(|v| Value::Array(v.clone())))
}

pub(crate) fn documents_envoyes(lignes: &[Value], mois: &str) -> R<Value> {
    let mut quand: Dico<Value> = Dico::nouveau();
    for l in filtrer(lignes, Some(DOCUMENTS), Some(mois), false)? {
        if !accepte(l)? {
            continue;
        }
        for sal in detail_liste(l, "salaries")? {
            quand.poser(vec![get(&sal, "cle")?.clone()], get(l, "quand")?.clone())?;
        }
    }
    Ok(quand.sortie(Clone::clone))
}

/// CE QUI EST PARTI A CHAQUE SALARIE avec ses documents du mois (ticket #240) : cle -> `{quand, empreinte, resume}` du
/// dernier envoi accepte. `empreinte` : la paie du salarie quand ses documents sont partis
/// (`routes_mois::empreintes_documents`) ; `resume` : ce que ses documents disaient (`envoi::resume_de`). Un envoi
/// d'avant le 21/10/2026 n'a ni l'un ni l'autre : `null`.
pub(crate) fn documents_partis(lignes: &[Value], mois: &str) -> R<std::collections::HashMap<String, Value>> {
    let mut out = std::collections::HashMap::new();
    for l in filtrer(lignes, Some(DOCUMENTS), Some(mois), false)? {
        if !accepte(l)? {
            continue;
        }
        for sal in detail_liste(l, "salaries")? {
            out.insert(
                valeur_en_texte(get(&sal, "cle")?),
                json!({"quand": get(l, "quand")?, "empreinte": get(&sal, "empreinte")?, "resume": get(&sal, "resume")?}),
            );
        }
    }
    Ok(out)
}

pub(crate) fn etat_du_mois(lignes: &[Value], mois: &str) -> R<Value> {
    let mut etat = Map::new();
    for acte in ACTES {
        let mut derniere = Value::Null;
        for l in filtrer(lignes, Some(acte), Some(mois), true)? {
            if accepte(l)? {
                derniere = l.clone();
            }
        }
        etat.insert(acte.to_string(), derniere);
    }
    Ok(Value::Object(etat))
}

pub(crate) fn flux_de_test(lignes: &[Value]) -> R<Value> {
    let mut d: Dico<()> = Dico::nouveau();
    for l in lignes {
        if est(get(l, "mode")?, TEST) && vrai(get(l, "flux")?) {
            d.poser(vec![get(l, "flux")?.clone()], ())?;
        }
    }
    Ok(Value::Array(d.items.iter().map(|(k, _)| k[0].clone()).collect()))
}

// ------------------------------------------------------------------- resume

/// `util.quand_lisible` avec sa liaison par defaut.
pub(crate) fn quand_lisible(iso: &Value) -> String {
    let t = if vrai(iso) { valeur_en_texte(iso) } else { String::new() };
    let c: Vec<char> = t.chars().collect();
    if c.len() < 10 || c[4] != '-' || c[7] != '-' {
        return t;
    }
    let tranche = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    let jour = format!("{}/{}/{}", tranche(8, 10), tranche(5, 7), tranche(0, 4));
    if c.len() >= 16 {
        format!("{} à {}", jour, tranche(11, 16))
    } else {
        jour
    }
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// `", ".join(liste)` : chaque element doit etre un texte.
pub(crate) fn joindre(sep: &str, v: &[Value]) -> R<String> {
    let mut out = Vec::new();
    for (i, x) in v.iter().enumerate() {
        match x {
            Value::String(t) => out.push(t.as_str()),
            autre => {
                return Err(erreur("TypeError", socle::fichiers::element(i, &socle::fichiers::attendu("un texte", nom_type(autre)))))
            }
        }
    }
    Ok(out.join(sep))
}

pub(crate) fn resume(ligne: &Value, recus: &Dico<()>, m: Maille) -> R<String> {
    if !vrai(ligne) {
        return Ok("pas encore envoyé".into());
    }
    let mut quand = quand_lisible(get(ligne, "quand")?);
    if est(get(ligne, "mode")?, TEST) {
        quand = format!("en TEST le {}", quand);
    }
    let retours = ou(get(ligne, "retours")?, &LISTE_VIDE);
    if !accepte(ligne)? {
        let rep = ou(get(ligne, "reponse")?, texte_vide());
        let debut = match rep {
            Value::String(t) => t.chars().take(120).collect::<String>(),
            Value::Array(a) => valeur_ecrite(&Value::Array(a.iter().take(120).cloned().collect())),
            autre => return Err(erreur("TypeError", format!("'reponse' : {}", socle::fichiers::attendu("un texte", nom_type(autre))))),
        };
        return Ok(format!("envoi du {} refusé au dépôt — {}", quand, debut));
    }
    let prefixe = format!("envoyé {}", if quand.starts_with("en TEST") { quand.clone() } else { format!("le {}", quand) });
    let quoi = get(ligne, "quoi")?;
    if est(quoi, SIGNATURE) {
        let contrats = items(ou(get(getd(ligne, "detail", objet_vide())?, "contrats")?, &LISTE_VIDE))?;
        let mut revenus = 0;
        for c in &contrats {
            if avec_repli(recus, &cle_contrat(c, m)?)?.is_some() {
                revenus += 1;
            }
        }
        let n = contrats.len();
        if n == 0 {
            return Ok(prefixe);
        }
        if revenus == n {
            return Ok(format!(
                "{} — {}",
                prefixe,
                if n == 1 { "revenu signé".to_string() } else { format!("tous revenus signés ({})", n) }
            ));
        }
        if revenus > 0 {
            return Ok(format!(
                "{} — {} contrat{} sur {} revenu{} signé{}",
                prefixe,
                revenus,
                pluriel(revenus),
                n,
                pluriel(revenus),
                pluriel(revenus)
            ));
        }
        return Ok(format!("{} — pas encore revenu signé", prefixe));
    }
    if est(quoi, DOCUMENTS) || est(quoi, CONTRESIGNE) || est(quoi, CASC) {
        return Ok(prefixe);
    }
    if !vrai(retours) {
        return Ok(format!("{} — pas encore de retour", prefixe));
    }
    let retours = items(retours)?;
    let mut refus = Vec::new();
    for r in &retours {
        if est(ou(get(r, "verdict")?, texte_vide()), "KO") {
            refus.push(r);
        }
    }
    let mut a_certificat = false;
    for r in &retours {
        if valeur_en_texte(get(r, "nature")?) == NATURE_CERTIFICAT_DPAE {
            a_certificat = true;
        }
    }
    if !refus.is_empty() && !a_certificat {
        let mut motifs: Dico<()> = Dico::nouveau();
        for r in &refus {
            for a in items(ou(get(r, "anomalies")?, &LISTE_VIDE))? {
                motifs.poser(vec![a.into_owned()], ())?;
            }
        }
        let mut tous = Vec::new();
        for r in &refus {
            for _ in items(ou(get(r, "anomalies")?, &LISTE_VIDE))? {
                tous.push(());
            }
        }
        let suite = if tous.is_empty() {
            String::new()
        } else {
            let textes: Vec<Value> = motifs.items.iter().map(|(k, _)| k[0].clone()).collect();
            format!(" : {}", joindre(" ; ", &textes)?)
        };
        return Ok(format!(
            "envoyé {} — REFUSÉ{}",
            if quand.starts_with("en TEST") { quand.clone() } else { format!("le {}", quand) },
            suite
        ));
    }
    let mut certificat = None;
    for r in &retours {
        if valeur_en_texte(get(r, "nature")?) == NATURE_CERTIFICAT_DPAE {
            certificat = Some(r);
        }
    }
    if let Some(c) = certificat {
        let lignes = items(ou(get(c, "dpae")?, &LISTE_VIDE))?;
        let mut dossiers = Vec::new();
        for l in &lignes {
            if vrai(get(l, "enregistree")?) && vrai(get(l, "dossier")?) {
                dossiers.push(get(l, "dossier")?.clone());
            }
        }
        let mut existantes = Vec::new();
        for l in &lignes {
            if vrai(get(l, "deja_existante")?) {
                match l.get("nom") {
                    Some(n) => existantes.push(n.clone()),
                    None => return Err(erreur("KeyError", socle::fichiers::absent("nom"))),
                }
            }
        }
        let mut fin = " — certificat de conformité reçu : passée".to_string();
        if !dossiers.is_empty() {
            fin.push_str(&format!(" (dossier{} {})", pluriel(dossiers.len()), joindre(", ", &dossiers)?));
        }
        if !existantes.is_empty() {
            fin.push_str(&format!(" ; déjà existante à l'Urssaf, rien de créé : {}", joindre(", ", &existantes)?));
        }
        return Ok(format!("{}{}", prefixe, fin));
    }
    let mut bilan_ok = false;
    for r in &retours {
        if valeur_en_texte(get(r, "nature")?) == "11" && est(ou(get(r, "verdict")?, texte_vide()), "OK") {
            bilan_ok = true;
        }
    }
    if bilan_ok {
        let mut apres = 0;
        for r in &retours {
            let n = valeur_en_texte(get(r, "nature")?);
            if n != "10" && n != "11" {
                apres += 1;
            }
        }
        let suite = if apres > 0 {
            format!(" ; {} compte{}-rendu{} d'organismes reçu{}", apres, pluriel(apres), pluriel(apres), pluriel(apres))
        } else {
            String::new()
        };
        return Ok(format!("{} — bilan OK, aucune anomalie{}", prefixe, suite));
    }
    let dernier = retours.last().expect("retours non vide");
    let retour = s("retour reçu");
    let choix = ou(get(dernier, "libelle")?, ou(get(dernier, "nature")?, &retour));
    Ok(format!("{} — {}", prefixe, valeur_en_texte(choix)))
}

// ------------------------------------------------------ la repetition avant l'acte

pub(crate) fn refuse_par_un_retour(ligne: &Value) -> R<bool> {
    for r in items(ou(get(ligne, "retours")?, &LISTE_VIDE))? {
        if est(ou(get(&r, "verdict")?, texte_vide()), "KO") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Le compte rendu d'un depot : `ko` des qu'un retour refuse ; `ok` seulement quand LE BILAN le dit -- la nature 11
/// (certificat de conformite ou bilan d'anomalies) pour une DSN, le certificat 41 pour une DPAE ; `attendu` sinon.
/// L'ACCUSE D'ENREGISTREMENT (nature 10) NE COMPTE PAS : il dit « envoi recu », `etat_envoi` OK, pas « declaration
/// conforme » (regle 5 ; `retours::NATURES` : « 10 - Accuse d'enregistrement electronique »). Le 19/10/2026 sur le
/// poste, un test dont seul l'accuse etait la s'affichait « testee — conforme » alors que l'essai precedent du meme
/// contenu venait de prendre B1-208-11 (tickets #221, #222 du depot prive).
fn compte_rendu_du_depot(ligne: &Value) -> R<&'static str> {
    let mut ok = false;
    for r in items(ou(get(ligne, "retours")?, &LISTE_VIDE))? {
        let v = ou(get(&r, "verdict")?, texte_vide());
        if est(v, "KO") {
            return Ok("ko");
        }
        let nature = valeur_en_texte(get(&r, "nature")?);
        ok = ok || ((nature == "11" || nature == NATURE_CERTIFICAT_DPAE) && est(v, "OK"));
    }
    Ok(if ok { "ok" } else { "attendu" })
}

pub(crate) fn deja_depose(lignes: &[Value], quoi: &str, empreinte: &str) -> R<Value> {
    for l in filtrer(lignes, Some(quoi), None, true)?.into_iter().rev() {
        if accepte(l)? && est(get(l, "empreinte")?, empreinte) {
            return Ok(l.clone());
        }
    }
    Ok(Value::Null)
}

pub(crate) fn repetition(lignes: &[Value], quoi: &str, empreinte: &str, nom: &str) -> R<Value> {
    let mut conforme: Option<&Value> = None;
    let (mut ratees, mut perimees) = (Vec::new(), Vec::new());
    for l in filtrer(lignes, Some(quoi), None, false)? {
        if !est(get(l, "mode")?, TEST) {
            continue;
        }
        if est(get(l, "empreinte")?, empreinte) {
            if accepte(l)? && !refuse_par_un_retour(l)? {
                conforme = Some(l);
            } else {
                ratees.push(l.clone());
            }
        } else if est(get(l, "fichier")?, nom) {
            perimees.push(l.clone());
        }
    }
    let compte_rendu = match conforme {
        Some(c) if vrai(c) => compte_rendu_du_depot(c)?,
        _ => "",
    };
    Ok(json!({
        "empreinte": empreinte, "fichier": nom, "conforme": conforme, "compte_rendu": compte_rendu,
        "ratees": ratees, "perimees": perimees,
    }))
}

// -------------------------------------------------------------------- flux_de

fn egal_sans_casse(c: char, voulu: char) -> bool {
    c.to_ascii_lowercase() == voulu || (voulu == 'i' && (c == '\u{131}' || c == '\u{130}'))
}

fn mot(c: &[char], mut j: usize, mot: &str) -> Option<usize> {
    for v in mot.chars() {
        if j < c.len() && egal_sans_casse(c[j], v) {
            j += 1;
        } else {
            return None;
        }
    }
    Some(j)
}

fn blancs(c: &[char], mut j: usize) -> usize {
    while j < c.len() && socle::texte::blanc(c[j]) {
        j += 1;
    }
    j
}

/// `<\s*idflux\s*>\s*([^<\s]+)\s*<`, insensible a la casse.
fn flux_xml(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '<' {
            continue;
        }
        let Some(j) = mot(c, blancs(c, i + 1), "idflux") else { continue };
        let j = blancs(c, j);
        if j >= c.len() || c[j] != '>' {
            continue;
        }
        let d = blancs(c, j + 1);
        let mut f = d;
        while f < c.len() && c[f] != '<' && !socle::texte::blanc(c[f]) {
            f += 1;
        }
        if f == d {
            continue;
        }
        let k = blancs(c, f);
        if k < c.len() && c[k] == '<' {
            return Some(c[d..f].iter().collect());
        }
    }
    None
}

/// `"idflux"\s*:\s*"([^"]+)"`, insensible a la casse.
fn flux_json(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '"' {
            continue;
        }
        let Some(j) = mot(c, i + 1, "idflux") else { continue };
        if j >= c.len() || c[j] != '"' {
            continue;
        }
        let j = blancs(c, j + 1);
        if j >= c.len() || c[j] != ':' {
            continue;
        }
        let d = blancs(c, j + 1);
        if d >= c.len() || c[d] != '"' {
            continue;
        }
        let mut f = d + 1;
        while f < c.len() && c[f] != '"' {
            f += 1;
        }
        if f < c.len() && f > d + 1 {
            return Some(c[d + 1..f].iter().collect());
        }
    }
    None
}

pub(crate) fn flux_de(reponse: &str) -> String {
    if reponse.is_empty() {
        return String::new();
    }
    let c: Vec<char> = reponse.chars().collect();
    if let Some(m) = flux_xml(&c) {
        return m;
    }
    if let Some(m) = flux_json(&c) {
        return m;
    }
    let nu = reponse.trim_matches(socle::texte::blanc);
    let n = nu.chars().count();
    if (8..=64).contains(&n) && nu.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        nu.to_string()
    } else {
        String::new()
    }
}

// ------------------------------------------------------- les verdicts des retours

enum Jeton {
    Etoile,
    Un,
    Car(char),
    Classe(Vec<(char, char)>, bool),
}

/// Le motif d'un `glob` de `fnmatch` : `*`, `?`, `[...]`.
fn jetons(motif: &str) -> Vec<Jeton> {
    let p: Vec<char> = motif.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let c = p[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Jeton::Etoile)) {
                    out.push(Jeton::Etoile);
                }
            }
            '?' => out.push(Jeton::Un),
            '[' => {
                let mut j = i;
                if j < p.len() && p[j] == '!' {
                    j += 1;
                }
                if j < p.len() && p[j] == ']' {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    out.push(Jeton::Car('['));
                } else {
                    let mut corps: &[char] = &p[i..j];
                    i = j + 1;
                    let nie = corps.first() == Some(&'!');
                    if nie {
                        corps = &corps[1..];
                    }
                    let mut plages = Vec::new();
                    let mut k = 0;
                    while k < corps.len() {
                        if k + 2 < corps.len() && corps[k + 1] == '-' {
                            plages.push((corps[k], corps[k + 2]));
                            k += 3;
                        } else {
                            plages.push((corps[k], corps[k]));
                            k += 1;
                        }
                    }
                    out.push(Jeton::Classe(plages, nie));
                }
            }
            c => out.push(Jeton::Car(c)),
        }
    }
    out
}

fn correspond(j: &[Jeton], nom: &[char]) -> bool {
    let sans_casse = cfg!(windows);
    let egal = |a: char, b: char| if sans_casse { a.to_lowercase().eq(b.to_lowercase()) } else { a == b };
    let (mut ji, mut ni) = (0, 0);
    let mut reprise: Option<(usize, usize)> = None;
    while ni < nom.len() {
        let ok = match j.get(ji) {
            Some(Jeton::Etoile) => {
                reprise = Some((ji, ni));
                ji += 1;
                continue;
            }
            Some(Jeton::Un) => true,
            Some(Jeton::Car(c)) => egal(*c, nom[ni]),
            Some(Jeton::Classe(plages, nie)) => plages.iter().any(|&(a, b)| a <= nom[ni] && nom[ni] <= b) != *nie,
            None => false,
        };
        if ok {
            ji += 1;
            ni += 1;
        } else if let Some((rj, rn)) = reprise {
            reprise = Some((rj, rn + 1));
            ji = rj + 1;
            ni = rn + 1;
        } else {
            return false;
        }
    }
    while matches!(j.get(ji), Some(Jeton::Etoile)) {
        ji += 1;
    }
    ji == j.len()
}

fn echec_de_valeur(v: Value) -> Echec {
    let classe = match v.get("classe").and_then(Value::as_str) {
        Some("FileNotFoundError") => "FileNotFoundError",
        Some("PermissionError") => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, v.get("erreur").and_then(Value::as_str).unwrap_or(""))
}

/// `journal._archive_du_retour`.
fn archive_du_retour(racine: &Path, r: &Value, quoi: &Value) -> R<Option<PathBuf>> {
    let canal = if est(quoi, DPAE) { "dpae" } else { "" };
    let dossier = crate::transport::dossier_archive(racine, canal).map_err(echec_de_valeur)?;
    let sans_flux = s("sansflux");
    let flux = match ou(get(r, "flux")?, &sans_flux) {
        Value::String(t) => t.chars().take(24).collect::<String>(),
        autre => return Err(erreur("TypeError", format!("'flux' : {}", socle::fichiers::attendu("un texte", nom_type(autre))))),
    };
    let motif: Vec<char> = format!("*_nature{}_{}*", valeur_en_texte(get(r, "nature")?), flux).chars().collect();
    let motif = jetons(&motif.iter().collect::<String>());
    let mut noms = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(|e| erreur_io(&e, &dossier))? {
        let e = e.map_err(|e| erreur_io(&e, &dossier))?;
        let nom = e.file_name().to_string_lossy().into_owned();
        if correspond(&motif, &nom.chars().collect::<Vec<_>>()) {
            noms.push(nom);
        }
    }
    noms.sort();
    Ok(noms.into_iter().find(|n| n != "journal.json").map(|n| dossier.join(n)))
}

fn lire_archive(f: &Path) -> R<Vec<u8>> {
    std::fs::read(f).map_err(|e| erreur_io(&e, f))
}

/// `journal._verdict_du_certificat`.
fn verdict_du_certificat(contenu: &[u8]) -> Value {
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let lignes = crate::dpae::lire_certificat(&texte);
    if lignes.is_empty() {
        return json!({});
    }
    let mut anomalies: Vec<String> = Vec::new();
    let texte_de = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    for l in &lignes {
        let lot = texte_de(l, "lot");
        if lot != "00" && !lot.is_empty() {
            if let Some((_, nom)) = crate::dpae::CODES_RET_AR_LOT.iter().find(|(c, _)| *c == lot) {
                anomalies.push(format!("{} (lot)", nom));
            }
        }
        if !vrai(&l["enregistree"]) && !vrai(&l["deja_existante"]) {
            anomalies.push(format!("{} -- {}", texte_de(l, "libelle"), texte_de(l, "nom")));
        }
    }
    let mut vus: Vec<String> = Vec::new();
    for a in anomalies {
        if !vus.contains(&a) {
            vus.push(a);
        }
    }
    let cles = ["nom", "prenom", "embauche", "dossier", "code", "libelle", "enregistree", "deja_existante", "formalites"];
    let dpae: Vec<Value> = lignes
        .iter()
        .map(|l| Value::Object(cles.iter().map(|k| (k.to_string(), l[*k].clone())).collect()))
        .collect();
    json!({"verdict": if vus.is_empty() { "OK" } else { "KO" }, "anomalies": vus, "dpae": dpae})
}

/// `journal.verdict_du_retour`.
fn verdict_du_retour(racine: &Path, r: &Value, quoi: &Value, contenu: Option<&[u8]>) -> R<Value> {
    let nature = valeur_en_texte(get(r, "nature")?);
    if nature != "10" && nature != "11" && !(est(quoi, DPAE) && nature == NATURE_CERTIFICAT_DPAE) {
        return Ok(json!({}));
    }
    let octets = match contenu {
        Some(c) => c.to_vec(),
        None => match archive_du_retour(racine, r, quoi)? {
            None => return Ok(json!({})),
            Some(f) => lire_archive(&f)?,
        },
    };
    if nature == NATURE_CERTIFICAT_DPAE {
        return Ok(verdict_du_certificat(&octets));
    }
    let b = retours::bilan_d_octets(&octets);
    if b.is_null() {
        return Ok(json!({}));
    }
    let etat = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let verdict = if !etat("etat_declaration").is_empty() { etat("etat_declaration") } else { etat("etat_envoi") };
    Ok(json!({"verdict": verdict.to_uppercase(), "anomalies": retours::anomalies_lisibles_d_octets(&octets)?}))
}

/// `journal._libelle_nature`.
fn libelle_nature(code: &Value, acte: &Value) -> String {
    if !vrai(code) {
        return String::new();
    }
    if est(acte, DPAE) {
        let c = valeur_en_texte(code);
        return match NATURES_DPAE.iter().find(|(k, _)| *k == c) {
            Some((_, l)) => l.to_string(),
            None => format!("nature {} (inconnue du canal DPAE)", c),
        };
    }
    retours::libelle_nature(&valeur_en_texte(code))
}

/// L'incident que `journal._noter_rejet` ecrirait : rien quand le retour ne
/// rejette pas, ou quand son calcul echoue (il ne leve jamais).
fn incident(racine: &Path, ligne: &Value, r: &Value, verdict: &Value, contenus: &Dico<Vec<u8>>) -> Option<Value> {
    let calcul = || -> R<Option<Value>> {
        if !est(get(verdict, "verdict")?, "KO") {
            return Ok(None);
        }
        let dsn = s(DSN);
        let quoi = ou(get(ligne, "quoi")?, &dsn);
        let nature = valeur_en_texte(ou(get(r, "nature")?, texte_vide()));
        let mut anomalies: Vec<Value> = Vec::new();
        if est(quoi, DPAE) && nature == NATURE_CERTIFICAT_DPAE {
            for d in items(ou(get(verdict, "dpae")?, &LISTE_VIDE))? {
                if !vrai(get(&d, "enregistree")?) && !vrai(get(&d, "deja_existante")?) {
                    anomalies.push(json!({"code": getd(&d, "code", texte_vide())?, "message": getd(&d, "libelle", texte_vide())?}));
                }
            }
        } else {
            let cle = [get(r, "flux")?.clone(), get(r, "nature")?.clone(), get(r, "publication")?.clone()];
            let contenu = match contenus.get(&cle)? {
                Some(c) => c.clone(),
                None => match archive_du_retour(racine, r, quoi)? {
                    Some(f) => lire_archive(&f)?,
                    None => Vec::new(),
                },
            };
            anomalies = retours::anomalies_codees_d_octets(&contenu).map_err(|e| Echec { classe: e.classe, message: e.message })?;
        }
        // `{tuple(sorted(a.items())) for a in anomalies}` : sans doublon, puis
        // les cles par ordre alphabetique.
        let mut vus: Vec<String> = Vec::new();
        let mut uniques: Vec<Value> = Vec::new();
        for a in &anomalies {
            let Value::Object(o) = a else {
                return Err(erreur("AttributeError", "anomalie illisible"));
            };
            let mut p: Vec<(&String, &Value)> = o.iter().collect();
            p.sort_by(|x, y| x.0.cmp(y.0));
            for (_, v) in &p {
                cle_py(v)?;
            }
            let k = p.iter().map(|(k, v)| format!("{}={}", k, cle_py(v).unwrap_or_default())).collect::<Vec<_>>().join("\u{1e}");
            if !vus.contains(&k) {
                vus.push(k);
                uniques.push(Value::Object(p.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()));
            }
        }
        let mut cles: Vec<(bool, String, Value)> = Vec::new();
        for a in uniques {
            let bloquant = !est(get(&a, "categorie")?, "bloquant");
            let code = match getd(&a, "code", texte_vide())? {
                Value::String(t) => t.clone(),
                autre => return Err(erreur("TypeError", format!("'code' : {}", socle::fichiers::incomparables(nom_type(autre), "str")))),
            };
            cles.push((bloquant, code, a));
        }
        cles.sort_by(|x, y| (x.0, &x.1).cmp(&(y.0, &y.1)));
        let anomalies: Vec<Value> = cles.into_iter().map(|c| c.2).collect();
        let mut codes = Vec::new();
        for a in &anomalies {
            if vrai(get(a, "code")?) {
                codes.push(get(a, "code")?.clone());
            }
        }
        let mut phrases = Vec::new();
        for a in anomalies.iter().take(10) {
            let t = format!("{} : {}", valeur_en_texte(get(a, "code")?), valeur_en_texte(get(a, "message")?));
            phrases.push(t.trim_matches(|c| c == ' ' || c == ':').to_string());
        }
        let resume = if phrases.join(" ; ").is_empty() { "Rejet sans motif lisible".to_string() } else { phrases.join(" ; ") };
        let canal = if est(quoi, DPAE) { "dpae" } else { "dsn" };
        Ok(Some(json!({
            "canal": canal,
            "ou": format!("retour-{}", valeur_en_texte(quoi)),
            "quoi": codes.first().cloned().unwrap_or_else(|| s("KO")),
            "resume": resume,
            "detail": {
                "reel": est(get(ligne, "mode")?, REEL),
                "nature_retour": nature,
                "type_declaration": getd(ligne, "type_declaration", texte_vide())?,
                "anomalies": anomalies.iter().take(30).collect::<Vec<_>>(),
            },
        })))
    };
    calcul().ok().flatten()
}

/// `journal.rattacher_retours` : le nombre ajoute ; les incidents a noter (dans
/// l'ordre) s'ajoutent a `incidents`, que l'appelant ecrit dans le journal des
/// incidents, meme quand le rattachement s'arrete sur une erreur.
fn rattacher_retours(racine: &Path, retours: &Value, contenus: &Dico<Vec<u8>>, incidents: &mut Vec<Value>) -> R<i64> {
    let p = chemin(racine);
    if !p.exists() {
        return Ok(0);
    }
    let mut lignes = charger(racine)?;
    let mut par_flux: Dico<Vec<usize>> = Dico::nouveau();
    for (i, l) in lignes.iter().enumerate() {
        let f = get(l, "flux")?;
        if vrai(f) {
            let k = std::slice::from_ref(f);
            if !par_flux.contient(k)? {
                par_flux.poser(k.to_vec(), Vec::new())?;
            }
            par_flux.get_mut(k)?.unwrap().push(i);
        }
    }
    let mut ajoutes = 0;
    for r in items(retours)? {
        let r: &Value = &r;
        let vide = texte_vide();
        let indices = par_flux.get(&[ou(get(r, "flux")?, vide).clone()])?.cloned().unwrap_or_default();
        for li in indices {
            {
                let Value::Object(o) = &mut lignes[li] else {
                    return Err(erreur("AttributeError", "ligne illisible"));
                };
                if !o.contains_key("retours") {
                    o.insert("retours".into(), json!([]));
                }
            }
            let ligne = lignes[li].clone();
            let connus = items(&ligne["retours"])?;
            let cle = [get(r, "nature")?.clone(), get(r, "publication")?.clone()];
            let cle_contenu = [get(r, "flux")?.clone(), get(r, "nature")?.clone(), get(r, "publication")?.clone()];
            let verdict = verdict_du_retour(racine, r, get(&ligne, "quoi")?, contenus.get(&cle_contenu)?.map(Vec::as_slice))?;
            let mut existant = None;
            for (xi, x) in connus.iter().enumerate() {
                if valeurs_egales(get(x, "nature")?, &cle[0]) && valeurs_egales(get(x, "publication")?, &cle[1]) {
                    existant = Some(xi);
                    break;
                }
            }
            fn nouvelles_de(l: &mut Value) -> R<&mut Vec<Value>> {
                match l.get_mut("retours") {
                    Some(Value::Array(a)) => Ok(a),
                    _ => Err(erreur("AttributeError", "'retours' n'est pas une liste")),
                }
            }
            if let Some(xi) = existant {
                if vrai(&verdict) && !connus[xi].as_object().is_some_and(|o| o.contains_key("verdict")) {
                    if let (Some(o), Value::Object(v)) = (nouvelles_de(&mut lignes[li])?[xi].as_object_mut(), &verdict) {
                        for (k, x) in v {
                            o.insert(k.clone(), x.clone());
                        }
                    }
                    ajoutes += 1;
                    incidents.extend(incident(racine, &ligne, r, &verdict, contenus));
                }
                continue;
            }
            incidents.extend(incident(racine, &ligne, r, &verdict, contenus));
            let mut m = Map::new();
            m.insert("nature".into(), get(r, "nature")?.clone());
            if let Value::Object(v) = &verdict {
                for (k, x) in v {
                    m.insert(k.clone(), x.clone());
                }
            }
            m.insert("libelle".into(), Value::String(libelle_nature(get(r, "nature")?, get(&ligne, "quoi")?)));
            m.insert("statut".into(), get(r, "statut")?.clone());
            m.insert("publication".into(), get(r, "publication")?.clone());
            nouvelles_de(&mut lignes[li])?.push(Value::Object(m));
            ajoutes += 1;
        }
    }
    if ajoutes > 0 {
        ecrire_json(&p, &Value::Array(lignes), 2).map_err(|m| erreur("OSError", m))?;
    }
    Ok(ajoutes)
}

/// `journal.rattacher_retours(ctx, retours)` sans contenus : le nombre ajoute. Les incidents que le
/// rattachement releve s'ecrivent au journal des incidents (`_ecrire_incident` : ne leve jamais), meme si le
/// rattachement s'arrete sur une erreur, qui remonte ensuite.
pub(crate) fn rattacher_retours_ecrits(racine: &Path, retours: &Value) -> R<i64> {
    rattacher_retours_avec_contenus(racine, retours, &[])
}

/// `journal.rattacher_retours(ctx, retours, contenus)` : `contenus` donne, pour chaque `(flux, nature,
/// publication)` telecharge, les octets du retour (le verdict se lit dedans).
pub(crate) fn rattacher_retours_avec_contenus(racine: &Path, retours: &Value, contenus: &[([Value; 3], Vec<u8>)]) -> R<i64> {
    let mut incidents: Vec<Value> = Vec::new();
    let mut dico: Dico<Vec<u8>> = Dico::nouveau();
    for (k, o) in contenus {
        dico.poser(k.to_vec(), o.clone())?;
    }
    let r = rattacher_retours(racine, retours, &dico, &mut incidents);
    for i in &incidents {
        let canal = if i.get("canal").and_then(Value::as_str) == Some("dpae") { crate::incidents::DPAE } else { crate::incidents::DSN };
        let _ = crate::incidents::noter(
            racine,
            &crate::incidents::Note {
                nature: canal,
                ou: i.get("ou").and_then(Value::as_str).unwrap_or(""),
                quoi: i.get("quoi").and_then(Value::as_str).unwrap_or(""),
                resume: i.get("resume").and_then(Value::as_str).unwrap_or(""),
                detail: i.get("detail").cloned().unwrap_or(Value::Null),
                ..Default::default()
            },
        );
    }
    r
}

// ----------------------------------------------------- le mois est-il parti ?

/// `journal._nic_principal` : le NIC de la fiche entreprise, ou `""`.
pub(crate) fn nic_principal(entreprise: &Value) -> String {
    match entreprise.get("nic") {
        Some(v) if vrai(v) => valeur_en_texte(v),
        _ => String::new(),
    }
}

/// Le dernier depot REEL accepte de DSN du mois (`etat_dsn_du_mois`), `Null`
/// si rien de reel n'est parti : le mois est « parti » des qu'il existe.
pub(crate) fn dsn_du_mois(lignes: &[Value], entreprise: &Value, mois: &str) -> R<Value> {
    etat_dsn_du_mois(lignes, mois, &Value::Null, &nic_principal(entreprise))
}

// Les etats et resumes sur un journal invente (noms et numeros fictifs).
#[cfg(test)]
mod tests {
    use super::*;

    /// Un journal sur le disque, pour ce qui l'ecrit ou le lit.
    fn journal(nom: &str, lignes: Value) -> PathBuf {
        let p = std::env::temp_dir().join(format!("paie-etats-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("journal")).unwrap();
        std::fs::write(p.join("journal").join("depots.json"), serde_json::to_string_pretty(&lignes).unwrap()).unwrap();
        p
    }

    fn ligne(quoi: &str, mois: &str, mode: &str, quand: &str, detail: Value, retours: Value) -> Value {
        json!({"quoi": quoi, "quand": quand, "origine": "", "mois": mois, "fichier": "f.dsn", "empreinte": "e1",
               "mode": mode, "qui": "?", "accepte": true, "destinataire": "", "flux": "", "retours": retours,
               "reponse": "", "trace": null, "detail": detail})
    }

    fn recus(cles: Value) -> Dico<()> {
        let mut d = Dico::nouveau();
        for k in cles.as_array().unwrap() {
            d.poser(k.as_array().unwrap().clone(), ()).unwrap();
        }
        d
    }

    #[test]
    fn les_embauches_declarees_ne_comptent_que_le_reel_accepte() {
        let test = ligne("dpae", "2026-06", "test", "2026-06-01T10:00:00", json!({"embauches": [{"salarie": "ALPHA", "debut": "2026-06-02"}]}), json!([]));
        let mut refuse = ligne("dpae", "2026-06", "reel", "2026-06-01T11:00:00", json!({"embauches": [{"salarie": "BETA", "debut": "2026-06-03"}]}), json!([]));
        refuse["accepte"] = json!(false);
        let bon = ligne("dpae", "2026-06", "reel", "2026-06-01T12:00:00", json!({"embauches": [{"salarie": "GAMMA", "debut": "2026-06-04"}, {"salarie": "GAMMA", "debut": "2026-06-04"}]}), json!([]));
        let l = vec![test, refuse, bon];
        let e = embauches_par_depot(&l).unwrap();
        assert_eq!(e.cles(), json!([["GAMMA", "2026-06-04"]]));
        assert_eq!(e.sortie(Clone::clone), json!([["GAMMA", "2026-06-04", "2026-06-01T12:00:00"]]));
    }

    #[test]
    fn l_etat_de_la_dsn_par_etablissement() {
        let ok = json!([{"nature": "10"}, {"nature": "11", "verdict": "OK"}, {"nature": 46}]);
        let ko = json!([{"nature": "11", "verdict": "KO", "anomalies": ["motif a"]}]);
        let mut autre = ligne("dsn", "2026-06", "reel", "2026-06-20T10:00:00", json!({"etablissement": "00022"}), ko);
        autre["flux"] = json!("FLUX2");
        let mut premier = ligne("dsn", "2026-06", "reel", "2026-06-10T10:00:00", json!({}), ok);
        premier["flux"] = json!("FLUX1");
        let l = vec![premier, autre];
        let principal = etat_dsn_du_mois(&l, "2026-06", &Value::Null, "00011").unwrap();
        assert_eq!((&principal["flux"], &principal["passee"], &principal["refusee"], &principal["aer_recue"]), (&json!("FLUX1"), &json!(true), &json!(false), &json!(true)));
        let second = etat_dsn_du_mois(&l, "2026-06", &json!("00022"), "00011").unwrap();
        assert_eq!((&second["flux"], &second["passee"], &second["refusee"]), (&json!("FLUX2"), &json!(false), &json!(true)));
        assert_eq!(second["anomalies"], json!(["motif a"]));
        assert_eq!(etat_dsn_du_mois(&l, "2026-07", &Value::Null, "").unwrap(), Value::Null);
    }

    #[test]
    fn l_etat_des_embauches_lit_le_certificat() {
        let cert = json!([{"nature": "41", "dpae": [
            {"nom": "Beta-Gamma", "embauche": "2026-06-04", "dossier": "D-1", "enregistree": true, "deja_existante": false}]}]);
        let l = ligne("dpae", "2026-06", "reel", "2026-06-01T12:00:00",
                      json!({"embauches": [{"salarie": "BETA GAMMA", "debut": "2026-06-04", "fin": "2026-06-30"}]}), cert);
        let l = vec![l];
        let e = etat_des_embauches(&l).unwrap();
        assert_eq!(e[0][0], "BETA GAMMA");
        let v = &e[0][2];
        assert_eq!((&v["passee"], &v["dossier"], &v["deja_existante"], &v["fin"]), (&json!(true), &json!("D-1"), &json!(false), &json!("2026-06-30")));
    }

    fn contrat(sal: &str, code: &str, empreinte: &str, acte: Value) -> Value {
        json!({"salarie": sal, "debut": "2026-06-04", "fin": "2026-06-30", "numero": "N1", "code": code,
               "empreinte": empreinte, "acte": acte, "avenant": null, "message": "m1", "mois": "2026-06"})
    }

    fn recu(sal: &str, origine: &str) -> Value {
        ligne("contrat-recu", "2026-06", "reel", "2026-06-12T10:00:00", json!({"salarie": sal, "debut": "2026-06-04", "recu": true, "origine": origine}), json!([]))
    }

    fn signature(quand: &str, c: Value) -> Value {
        ligne("contrat-signature", "2026-06", "reel", quand, json!({"contrats": [c]}), json!([]))
    }

    #[test]
    fn les_signatures_attendues_et_les_contrats_revenus() {
        let envoi1 = signature("2026-06-05T10:00:00", contrat("ALPHA", "C1", "e1", Value::Null));
        let mut envoi2 = signature("2026-06-06T10:00:00", contrat("BETA", "C2", "e2", Value::Null));
        envoi2["detail"]["contrats"][0]["adresse"] = json!("beta@exemple.fr");
        let l = vec![envoi1, envoi2, recu("ALPHA", "courriel")];
        let a = signatures_attendues(&l, None, &json!({"BETA": {"prenom": "Beta", "nom": "Bêta"}})).unwrap();
        assert_eq!(a.as_array().unwrap().len(), 1);
        assert_eq!((&a[0]["code"], &a[0]["nom"], &a[0]["envoyes"], &a[0]["rang"]), (&json!("C2"), &json!("Beta Bêta"), &json!(["m1"]), &json!(1)));
        assert_eq!(contrats_recus(&l, Maille::Paire).unwrap().sortie(Clone::clone), json!([["ALPHA", "2026-06-04", "2026-06-12T10:00:00"]]));
        assert_eq!(contrats_recus_comment(&l, Maille::Paire).unwrap(), json!([["ALPHA", "2026-06-04", "courriel"]]));
        assert_eq!(contrats_demandes(&l, Maille::Paire).unwrap().as_array().unwrap().len(), 2);
        assert_eq!(numeros_contrats_partis(&l, Maille::Paire).unwrap()[0][2], "N1");
    }

    #[test]
    fn les_avenants_attendent_leur_signature() {
        let mut c = contrat("ALPHA", "C5", "e5", json!("avenant"));
        c["avenant"] = json!(1);
        c["modifications"] = json!(["horaire"]);
        let l = vec![signature("2026-06-08T10:00:00", c), recu("ALPHA", "main")];
        let a = avenants(&l, Maille::Paire).unwrap();
        assert_eq!((&a[0][2][0]["numero"], &a[0][2][0]["signe"], &a[0][2][0]["modifications"]), (&json!(1), &json!("2026-06-12T10:00:00"), &json!(["horaire"])));
    }

    #[test]
    fn l_etat_et_les_dernieres_lignes_du_mois() {
        let mut t = ligne("dsn", "2026-06", "test", "2026-06-01T10:00:00", json!({}), json!([]));
        t["fichier"] = json!("test.dsn");
        let r1 = ligne("dsn", "2026-06", "reel", "2026-06-02T10:00:00", json!({}), json!([]));
        let mut r2 = ligne("dsn", "2026-06", "reel", "2026-06-03T10:00:00", json!({}), json!([]));
        r2["accepte"] = json!(false);
        let d = ligne("documents", "2026-06", "reel", "2026-06-04T10:00:00", json!({"salaries": [{"cle": "ALPHA"}]}), json!([]));
        let l = vec![t, r1, r2, d];
        let e = etat_du_mois(&l, "2026-06").unwrap();
        assert_eq!(e["dsn"]["quand"], "2026-06-02T10:00:00");
        assert_eq!(e["dpae"], Value::Null);
        assert_eq!(documents_envoyes(&l, "2026-06").unwrap(), json!([["ALPHA", "2026-06-04T10:00:00"]]));
    }

    #[test]
    fn les_resumes() {
        let r = |l: Value, k: Value| resume(&l, &recus(k), Maille::Paire).unwrap();
        // Chaque resume dit l'etat du geste : envoye ou non, en test, refuse et pourquoi, accepte, signe.
        assert!(r(Value::Null, json!([])).contains("pas encore envoyé"));
        let l = ligne("dsn", "2026-06", "test", "2026-06-02T10:05:00", json!({}), json!([]));
        let s = r(l, json!([]));
        assert!(s.contains("TEST") && s.contains("02/06/2026") && s.contains("pas encore de retour"), "{}", s);
        let mut refus = ligne("dpae", "2026-06", "reel", "2026-06-02", json!({}), json!([]));
        refus["accepte"] = json!(false);
        refus["reponse"] = json!("Code URSSAF invalide");
        let s = r(refus, json!([]));
        assert!(s.contains("refusé") && s.contains("Code URSSAF invalide"), "{}", s);
        let ko = ligne("dsn", "2026-06", "reel", "2026-06-02T10:05:00", json!({}), json!([{"nature": "11", "verdict": "KO", "anomalies": ["a", "b", "a"]}]));
        let s = r(ko, json!([]));
        assert!(s.contains("REFUSÉ") && s.ends_with(": a ; b"), "chaque anomalie une fois : {}", s);
        let ok = ligne("dsn", "2026-06", "reel", "2026-06-02T10:05:00", json!({}),
                       json!([{"nature": "10"}, {"nature": "11", "verdict": "OK"}, {"nature": "45"}, {"nature": "46"}]));
        let s = r(ok, json!([]));
        assert!(s.contains("bilan OK") && s.contains("2 comptes-rendus"), "{}", s);
        let cert = ligne("dpae", "2026-06", "reel", "2026-06-02T10:05:00", json!({}),
                         json!([{"nature": "41", "dpae": [{"nom": "X", "enregistree": true, "dossier": "D1"}, {"nom": "Y", "deja_existante": true}]}]));
        let s = r(cert, json!([]));
        assert!(s.contains("(dossier D1)") && s.ends_with(": Y"), "{}", s);
        let sig = ligne("contrat-signature", "2026-06", "reel", "2026-06-02T10:05:00", json!({"contrats": [{"salarie": "A", "debut": "d"}, {"salarie": "B", "debut": "d"}]}), json!([]));
        assert!(r(sig.clone(), json!([])).contains("pas encore revenu signé"));
        assert!(r(sig.clone(), json!([["A", "d"]])).contains("1 contrat sur 2"));
        assert!(r(sig, json!([["A", "d"], ["B", "d"]])).contains("tous revenus signés (2)"));
        let dernier = ligne("dsn", "2026-06", "reel", "2026-06-02T10:05:00", json!({}), json!([{"nature": 99, "libelle": ""}]));
        assert!(r(dernier, json!([])).ends_with("99"));
    }

    #[test]
    fn le_flux_se_lit_dans_la_reponse() {
        assert_eq!(flux_de("<ns:IdFlux > WSabc123XYZ </ns:IdFlux>"), "");
        assert_eq!(flux_de("x <  IDFLUX >WSabc123XYZ</idflux>"), "WSabc123XYZ");
        assert_eq!(flux_de(r#"{"idflux" : "WSabc123XYZ", "a": 1}"#), "WSabc123XYZ");
        assert_eq!(flux_de("  WSabc123XYZ\n"), "WSabc123XYZ");
        for rien in ["court", "deux mots ici", "", "<idflux>AB CD</idflux>"] {
            assert_eq!(flux_de(rien), "", "{:?}", rien);
        }
    }

    #[test]
    fn la_repetition_et_le_deja_depose() {
        let mut bon = ligne("dsn", "2026-06", "test", "2026-06-01T10:00:00", json!({}), json!([{"nature": "11", "verdict": "OK"}]));
        bon["empreinte"] = json!("eA");
        let mut rate = ligne("dsn", "2026-06", "test", "2026-06-01T11:00:00", json!({}), json!([]));
        rate["empreinte"] = json!("eA");
        rate["accepte"] = json!(false);
        let mut perimee = ligne("dsn", "2026-06", "test", "2026-06-01T09:00:00", json!({}), json!([]));
        perimee["empreinte"] = json!("eB");
        perimee["fichier"] = json!("DSN.dsn");
        let mut reel = ligne("dsn", "2026-06", "reel", "2026-06-02T09:00:00", json!({}), json!([]));
        reel["empreinte"] = json!("eA");
        let l = vec![perimee, bon, rate, reel];
        let rep = repetition(&l, "dsn", "eA", "DSN.dsn").unwrap();
        assert_eq!((&rep["compte_rendu"], &rep["conforme"]["quand"]), (&json!("ok"), &json!("2026-06-01T10:00:00")));
        assert_eq!((rep["ratees"].as_array().unwrap().len(), rep["perimees"].as_array().unwrap().len()), (1, 1));
        let vide = repetition(&l, "dpae", "eA", "x").unwrap();
        assert_eq!((&vide["conforme"], &vide["compte_rendu"]), (&Value::Null, &json!("")));
        assert_eq!(deja_depose(&l, "dsn", "eA").unwrap()["quand"], "2026-06-02T09:00:00");
        assert_eq!(deja_depose(&l, "dsn", "eB").unwrap(), Value::Null);
        assert_eq!(compte_rendu_du_depot(&json!({"retours": [{"verdict": "OK"}, {"verdict": "KO"}]})).unwrap(), "ko");
        assert_eq!(compte_rendu_du_depot(&json!({"retours": []})).unwrap(), "attendu");
    }

    const BAN_KO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"BAN\">\
         <envoi><idflux>WSBBBB</idflux></envoi><declaration><declaration_bilan><etat>KO</etat>\
         <declaration_anomalie><code>CST-04</code><categorie>bloquant</categorie><message>Code URSSAF invalide</message>\
         <identifiant>S21.G00.20.001</identifiant><nom_famille>FAVART</nom_famille></declaration_anomalie>\
         </declaration_bilan></declaration></ns2:rapport>";

    #[test]
    fn le_rattachement_pose_le_verdict_et_signale_l_incident() {
        let mut l = ligne("dsn", "2026-06", "test", "2026-06-01T10:00:00", json!({}), json!([]));
        l["flux"] = json!("WSBBBB");
        let mut autre = ligne("dpae", "2026-06", "reel", "2026-06-01T10:00:00", json!({}), json!([]));
        autre["flux"] = json!("WSCCCC");
        let r = journal("rattachement", json!([l, autre]));
        let retours = json!([
            {"flux": "WSBBBB", "nature": "11", "publication": "p1", "statut": "recu"},
            {"flux": "WSINCONNU", "nature": "11", "publication": "p2", "statut": "recu"},
            {"flux": "WSCCCC", "nature": "20", "publication": "p3", "statut": "recu"}]);
        let mut contenus = Dico::nouveau();
        contenus.poser(vec![json!("WSBBBB"), json!("11"), json!("p1")], BAN_KO.as_bytes().to_vec()).unwrap();
        let mut inc = Vec::new();
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut inc).unwrap(), 2);
        assert_eq!(inc.len(), 1);
        assert_eq!((&inc[0]["canal"], &inc[0]["ou"], &inc[0]["quoi"]), (&json!("dsn"), &json!("retour-dsn"), &json!("CST-04")));
        assert_eq!(inc[0]["resume"], "CST-04 : Code URSSAF invalide");
        assert_eq!(inc[0]["detail"]["reel"], false);
        assert_eq!(inc[0]["detail"]["anomalies"][0]["rubrique"], "S21.G00.20.001");
        // ni le nom du salarie ni la valeur ne partent chez l'editeur
        assert!(!inc[0].to_string().contains("FAVART"));
        let fichier = charger(&r).unwrap();
        let pose = &fichier[0]["retours"][0];
        assert_eq!(pose["verdict"], "KO");
        assert_eq!(pose["anomalies"], json!(["Code URSSAF invalide (bloquant) -- FAVART"]));
        assert_eq!(pose["libelle"], "Certificat de conformite / bilan d'anomalies");
        assert_eq!(fichier[1]["retours"][0]["libelle"], "bilan d'identification des salaries");
        // un second passage ne rajoute rien
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut Vec::new()).unwrap(), 0);
        std::fs::remove_dir_all(&r).unwrap();
    }

    #[test]
    fn les_noms_se_rapprochent_sans_accent_de_ponctuation() {
        assert!(meme_nom(&json!("DU-MONT-LAVAL"), &json!("du mont laval")).unwrap());
        assert!(!meme_nom(&json!("ALPHA"), &json!("BETA")).unwrap());
        assert!(!meme_nom(&json!(""), &json!("")).unwrap());
        assert!(!meme_nom(&Value::Null, &json!("X")).unwrap());
    }

    #[test]
    fn le_journal_illisible_est_refuse() {
        let r = journal("illisible", json!([]));
        std::fs::write(r.join("journal").join("depots.json"), "[{").unwrap();
        assert!(charger(&r).is_err());
        // une ligne qui n'est pas un objet
        std::fs::write(r.join("journal").join("depots.json"), "[3]").unwrap();
        assert!(charger(&r).and_then(|l| etat_du_mois(&l, "2026-06")).is_err());
        std::fs::remove_dir_all(&r).unwrap();
    }
}
