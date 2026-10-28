//! Les petites routes de lecture que la page appelle en s'ouvrant (`moteur route`) :
//! `identite`, `exercices`, `coffre-etat`, `filet-etat`, `importation-etat`,
//! `maj-etat`, `signalements`, `pieces-parties`.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et ce qu'il appelait :
//! `operations_fiches.identite`, `operations_exercice.etat_exercices`,
//! `operations_ouverture.coffre_etat` (+ `identifiant_du_compte`, `version`),
//! `operations_filet.etat_filet` (+ `lire_etat`), `operations_importation.
//! etat_importation`, `operations_filet.signalements` (+ `reponses_non_lues`),
//! `operations_documents.pieces_parties` -- portes a l'identique : meme JSON
//! (l'enveloppe de `web.py` comprise), memes refus sous la meme classe.
//!
//! Aucun sous-processus, AUCUN RESEAU : le journal, le coffre et les fichiers se
//! lisent ici. Le coffre ne rend que des PRESENCES (`coffre::Coffre::etat`), comme
//! le Python : aucune valeur de secret ne sort.
//!
//! Restent en Python, parce que leur reponse depend d'autre chose que du disque :
//! `google-etat` (il rafraichit reellement le jeton aupres de Google) et
//! `diagnostic-etat` (chaque groupe porte la machine -- systeme, architecture,
//! version de Python --, que le moteur ne peut pas dire).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::etats::{self, est, get, getd, items, joindre, objet_vide, ou, DPAE, SIGNATURE};
use socle::valeur::valeurs_egales;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, vrai};
use socle::echec::Echec;
use crate::routes::accueil::{identite, mois_connus};
use crate::routes::mois::Moteur;

type R<T> = Result<T, Echec>;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 8] =
    ["identite", "exercices", "coffre-etat", "filet-etat", "importation-etat", "maj-etat", "signalements", "pieces-parties"];

/// `journal.CONTRAT_RECU`.
const CONTRAT_RECU: &str = "contrat-recu";

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "identite" => Ok(json!({"identite": identite(ctx, &ctx.gabarits())?})),
        "exercices" => exercices_route(ctx),
        "coffre-etat" => coffre_etat(ctx),
        "filet-etat" => etat_filet(&ctx.racine),
        "importation-etat" => etat_importation(&ctx.racine),
        "maj-etat" => version(ctx),
        "signalements" => signalements_route(&ctx.racine),
        "pieces-parties" => pieces_parties(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        _ => "OSError",
    };
    erreur(classe, format!("{} : {}", p.display(), e))
}

/// `p.read_text(encoding="utf-8")`.
fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))
}

/// `json.loads(p.read_text(encoding="utf-8"))`.
fn lire_json(p: &Path) -> R<Value> {
    serde_json::from_str(&lire_texte(p)?).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

/// `_lire_json(p)` : `{}` si le fichier n'existe pas.
pub(crate) fn lire_json_ou_vide(p: &Path) -> R<Value> {
    if p.exists() {
        lire_json(p)
    } else {
        Ok(Value::Object(Map::new()))
    }
}

/// `try: json.loads(p.read_text(...)) except ValueError: ...` : `None` quand le
/// fichier est absent, illisible comme texte ou comme JSON ; un autre refus du
/// disque (`OSError`) se leve.
fn lire_json_tolerant(p: &Path) -> R<Option<Value>> {
    if !p.exists() {
        return Ok(None);
    }
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(String::from_utf8(octets).ok().and_then(|t| serde_json::from_str(&t).ok()))
}

/// `len(v)`.
fn longueur(v: &Value) -> R<usize> {
    match v {
        Value::Array(a) => Ok(a.len()),
        Value::Object(o) => Ok(o.len()),
        Value::String(s) => Ok(s.chars().count()),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un texte ou une liste", nom_type(autre)))),
    }
}

/// `v.get(...)` sur un dictionnaire ; sinon `AttributeError`.
fn objet<'a>(v: &'a Value) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v))))
}

fn valeur<'a>(m: &'a Map<String, Value>, cle: &str) -> &'a Value {
    static NUL: Value = Value::Null;
    m.get(cle).unwrap_or(&NUL)
}

/// `x or ""`.
fn ou_texte(v: &Value) -> Value {
    if vrai(v) {
        v.clone()
    } else {
        json!("")
    }
}

// ------------------------------------------------------------------ exercices

/// `operations_socle.exercices` : les clotures, sans les cles `_...`.
fn exercices_clos(racine: &Path) -> R<Map<String, Value>> {
    let v = lire_json_ou_vide(&racine.join("config").join("exercices.json"))?;
    match v {
        Value::Object(o) => Ok(o.into_iter().filter(|(k, _)| !k.starts_with('_')).collect()),
        autre => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&autre)))),
    }
}

/// La route `exercices` : `{"exercices": etat_exercices, "dossier": ...}`.
fn exercices_route(ctx: &Contexte) -> R<Value> {
    let liste = etat_exercices(ctx)?;
    Ok(json!({"exercices": liste, "dossier": ctx.racine.join("sorties").join("exercices").display().to_string()}))
}

/// `etat_exercices` : chaque annee connue du dossier -- ses mois, sa cloture,
/// son export.
fn etat_exercices(ctx: &Contexte) -> R<Value> {
    let clos = exercices_clos(&ctx.racine)?;
    let mois_tous: Vec<String> = mois_connus(&ctx.racine).into_iter().map(|(m, _)| m).collect();
    let mut annees: BTreeSet<String> = mois_tous.iter().map(|m| m.chars().take(4).collect()).collect();
    annees.extend(clos.keys().cloned());
    let mut m = Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in annees.iter().rev() {
        let mut mois: Vec<&String> = mois_tous.iter().filter(|x| x.starts_with(a.as_str())).collect();
        mois.sort();
        let export = ctx.racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", a));
        let export = export.exists().then_some(export);
        let mut acceptees = Vec::new();
        for x in &mois {
            if vrai(&m.dsn_du_mois(x)?) {
                acceptees.push(json!(x));
            }
        }
        let export_json = match &export {
            Some(p) => {
                json!({
                    "fichier": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    "ecrit": crate::depart::ecrit_le(&ctx.racine, p),
                })
            }
            None => Value::Null,
        };
        out.push(json!({
            "annee": a, "mois": mois, "dsn_acceptees": acceptees,
            "clos": clos.get(a).cloned().unwrap_or(Value::Null),
            "export": export_json,
        }));
    }
    Ok(Value::Array(out))
}

// ------------------------------------------------------------------ le coffre

pub(crate) fn vers_route(x: crate::coffre::Echec) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// `identifiant_du_compte` : le compte que ce mot de passe ouvre, en clair ; ce
/// qui en manque ; et s'il se saisit sur cette ligne (`ici`).
fn identifiant_du_compte(ent: &Value, service: &str) -> R<Value> {
    match service {
        "net-entreprises" => {
            let d = envois::transport::declarant(ent).map_err(|v| Echec {
                classe: match v.get("classe").and_then(Value::as_str) {
                    Some("AttributeError") => Some("AttributeError"),
                    _ => None,
                },
                message: v.get("erreur").and_then(Value::as_str).unwrap_or("").to_string(),
            })?;
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(&d, c)?) {
                    manque.push(json!(c));
                }
            }
            if !vrai(get(&d, "siret")?) {
                manque.insert(0, json!("siret"));
            }
            let noms: Vec<Value> = [get(&d, "nom")?, get(&d, "prenom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(get(&d, "siret")?)) } else { String::new() };
            Ok(json!({"texte": phrase, "manque": manque, "ici": false, "ou": "fiche Entreprise › Qui les organismes contactent"}))
        }
        "urssaf" => {
            let cfg = ou(get(ent, "api_dpae")?, objet_vide());
            let siret = ou(get(cfg, "siret")?, get(ent, "siret")?);
            let mut manque: Vec<Value> = Vec::new();
            for c in ["nom", "prenom"] {
                if !vrai(get(cfg, c)?) {
                    manque.push(json!(c));
                }
            }
            let noms: Vec<Value> = [get(cfg, "prenom")?, get(cfg, "nom")?].into_iter().filter(|x| vrai(x)).cloned().collect();
            let texte = joindre(" ", &noms)?;
            let phrase = if manque.is_empty() { format!("compte de {}, SIRET {}", texte, valeur_en_texte(siret)) } else { String::new() };
            Ok(json!({
                "texte": phrase, "manque": manque, "ici": true,
                "nom": ou_texte(get(cfg, "nom")?), "prenom": ou_texte(get(cfg, "prenom")?),
            }))
        }
        _ => Ok(json!({"texte": "", "manque": [], "ici": false})),
    }
}

/// `coffre_etat` : ce qui est pose, pour quel usage, depuis quand. Jamais les
/// valeurs.
fn coffre_etat(ctx: &Contexte) -> R<Value> {
    coffre_etat_de(&ctx.racine, &ctx.entreprise)
}

/// `coffre_etat` sur une fiche entreprise donnee : `poser_compte_dpae` le relit sur la fiche qu'il vient d'ecrire.
pub(crate) fn coffre_etat_de(racine: &Path, entreprise: &Value) -> R<Value> {
    let coffre = crate::coffre::Coffre::du_poste(racine, "");
    let secrets = coffre.etat().map_err(vers_route)?;
    let mut services = Vec::new();
    for s in &secrets {
        let nom = s["service"].as_str().unwrap_or("");
        services.push(json!({
            "service": s["service"], "libelle": s["libelle"], "sert_a": s["sert_a"],
            "pose": s["pose"], "pose_le": s["pose_le"], "environnement": s["dans_environnement"],
            "compte": s["compte"], "avec_compte": s["avec_compte"],
            "identifiant": identifiant_du_compte(entreprise, nom)?,
        }));
    }
    let mot_de_passe_pose = coffre.ouverture_exigee().map_err(vers_route)?;
    let cle_poste = coffre.cle_posee().map_err(vers_route)?;
    Ok(json!({
        "services": services,
        "mot_de_passe_pose": mot_de_passe_pose,
        "cle_poste": cle_poste,
        "chemin_cle": coffre.chemin_cle().display().to_string(),
    }))
}

// ------------------------------------------------------------------ le filet

/// `etat_filet` : le compte (sans sa cle), et le nombre de reveils. Lit
/// `config/filet-dpae.json` : rien ne part vers le serveur.
pub(crate) fn etat_filet(racine: &Path) -> R<Value> {
    let mut e = lire_json_tolerant(&racine.join("config").join("filet-dpae.json"))?.unwrap_or_else(|| Value::Object(Map::new()));
    let Value::Object(m) = &mut e else {
        // Le Python plantait (`AttributeError: ... 'setdefault'`, trace) : un refus avec sa phrase (decision de Leandre : un defaut du Python ne
        // se reproduit pas). `commandes/filet.rs` le dit en « ! ... », code 1.
        return Err(erreur("ErreurOperation", format!("{} est illisible : ce n'est pas un objet JSON.\nLe deplacer suffit a repartir de zero (le filet sera a reactiver).", racine.join("config").join("filet-dpae.json").display())));
    };
    for (cle, defaut) in [("compte", Value::Null), ("tests", json!({})), ("reveils", json!({}))] {
        m.entry(cle).or_insert(defaut);
    }
    let compte = valeur(m, "compte");
    let c = if vrai(compte) { compte } else { objet_vide() };
    let c = objet(c)?;
    let actif = !c.is_empty();
    let adresse = c.get("adresse").cloned().unwrap_or_else(|| json!(""));
    let confirme = vrai(valeur(c, "confirme"));
    // Le serveur auquel `filet-activer` s'inscrirait : celui du compte, de la racine, sinon celui du moteur (lot S1).
    let serveur = json!(crate::filet::serveur(&Value::Object(m.clone())));
    let inscrit_le = c.get("inscrit_le").cloned().unwrap_or_else(|| json!(""));
    let erreur_c = c.get("erreur").cloned().unwrap_or_else(|| json!(""));
    let reveils = longueur(valeur(m, "reveils"))?;
    Ok(json!({
        "actif": actif, "adresse": adresse, "confirme": confirme, "serveur": serveur,
        "inscrit_le": inscrit_le, "erreur": erreur_c, "reveils": reveils,
    }))
}

// ------------------------------------------------------------------ l'importation

/// Le nombre de valeurs distinctes (`len({...})`) : `1`, `1.0` et `True` ne font
/// qu'une ; une liste ou un dictionnaire ne se hache pas.
fn distincts(engagements: &[&Value], cle: &str) -> R<usize> {
    let mut vus: HashSet<String> = HashSet::new();
    for e in engagements {
        let v = get(e, cle)?;
        if !vrai(v) {
            continue;
        }
        vus.insert(match v {
            Value::String(t) => format!("s{}", t),
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            autre => format!("n{:?}", socle::valeur::nombre(autre).unwrap_or(0.0)),
        });
    }
    Ok(vus.len())
}

/// `etat_importation` : ce qui a deja ete repris, pour que la page le dise.
///
/// Les cles d'avant (`importe_le`, `source`, `contrats`, `salaries`, `objets`, `du`, `au` : les contrats attestes de
/// `historique/engagements.json`) gardent leur sens et leurs refus. S'y ajoutent (lot F du plan d'import) :
/// - `dossier` : la boite aux lettres `import/` de la compagnie, que la page preremplit ;
/// - `reprises` : chaque source reprise -- `DSN` (les versements de `historique/paies.json` declares par une DSN ;
///   `declarations` : une DSN par mois declare),
///   `AEM` (les contrats attestes par une AEM), `bulletins` (les paies qu'un bulletin a ecrites ou completees, les
///   contrats qu'il atteste, les PDF copies dans `historique/bulletins/`) --, avec combien, de quand a quand, et le
///   jour de la reprise (`importe_le` de la ligne, sinon celui du fichier) ;
/// - `editeur` : ce qui attend l'editeur -- les incidents et les signalements partis (ou a partir) de ce poste qui
///   parlent de l'import, sans valeur personnelle (ils ont ete caviardes avant d'etre ecrits) ; `attend_editeur` en
///   compte ceux qui n'ont pas de reponse.
///
/// Ces ajouts ne levent jamais : une ligne qui n'a pas la forme attendue n'est pas comptee.
fn etat_importation(racine: &Path) -> R<Value> {
    let p = racine.join("historique").join("engagements.json");
    let mut h = lire_json_ou_vide(&p)?;
    let Value::Object(m) = &mut h else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&h))));
    };
    m.entry("engagements").or_insert_with(|| json!([]));
    let brut = ou(valeur(m, "engagements"), &etats::LISTE_VIDE);
    let liste = items(brut)?;
    let mut engagements: Vec<&Value> = Vec::new();
    for e in &liste {
        engagements.push(e.as_ref());
    }
    let dates = |cle: &str| -> R<Vec<String>> {
        let mut v = Vec::new();
        for e in &engagements {
            let x = get(e, cle)?;
            if vrai(x) {
                v.push(valeur_en_texte(x));
            }
        }
        v.sort();
        Ok(v)
    };
    let debuts = dates("debut")?;
    let fins = dates("fin")?;
    let editeur = attend_l_editeur(racine);
    let attend = editeur.iter().filter(|x| !x.get("repondu").is_some_and(vrai)).count();
    Ok(json!({
        "importe_le": ou_texte(valeur(m, "importe_le")),
        "source": ou_texte(valeur(m, "source")),
        "contrats": engagements.len(),
        "salaries": distincts(&engagements, "salarie")?,
        "objets": distincts(&engagements, "numero_objet")?,
        "du": debuts.first().cloned().unwrap_or_default(),
        "au": fins.last().cloned().unwrap_or_default(),
        "dossier": racine.join("import").display().to_string(),
        "reprises": reprises_faites(racine, m, &engagements),
        "editeur": editeur,
        "attend_editeur": attend,
        // Les contrats repris dont la nature reste a dire, par salarie (#236, `importation_nature`).
        "a_dire": crate::importation_nature::a_dire(racine),
    }))
}

/// Une source reprise : combien, de quand a quand, quel jour.
#[derive(Default)]
struct Reprise {
    nombre: usize,
    du: Option<String>,
    au: Option<String>,
    le: Option<String>,
}

impl Reprise {
    /// Une ligne de plus : ses dates (un texte non vide), et le jour ou elle a ete reprise.
    fn voir(&mut self, debut: Option<&Value>, fin: Option<&Value>, le: Option<&Value>) {
        let texte = |v: Option<&Value>| v.and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
        self.nombre += 1;
        if let Some(d) = texte(debut) {
            if self.du.as_ref().is_none_or(|x| &d < x) {
                self.du = Some(d);
            }
        }
        if let Some(f) = texte(fin) {
            if self.au.as_ref().is_none_or(|x| &f > x) {
                self.au = Some(f);
            }
        }
        if let Some(l) = texte(le) {
            if self.le.as_ref().is_none_or(|x| &l > x) {
                self.le = Some(l);
            }
        }
    }
}

/// `reprises` de `importation-etat` : DSN, AEM, bulletins, dans cet ordre, celles qui ont repris quelque chose.
fn reprises_faites(racine: &Path, engagements_json: &Map<String, Value>, engagements: &[&Value]) -> Vec<Value> {
    let le_fichier = |m: &Map<String, Value>| m.get("importe_le").filter(|v| v.is_string()).cloned();
    let (mut aem, mut contrats_bulletins) = (Reprise::default(), Reprise::default());
    let le_engagements = le_fichier(engagements_json);
    for e in engagements {
        let Some(o) = e.as_object() else { continue };
        let le = o.get("importe_le").filter(|v| v.as_str().is_some_and(|s| !s.is_empty())).or(le_engagements.as_ref());
        let cible = if o.get("atteste_par").and_then(Value::as_str) == Some("bulletin") { &mut contrats_bulletins } else { &mut aem };
        cible.voir(o.get("debut"), o.get("fin"), le);
    }
    let (mut dsn, mut bulletins) = (Reprise::default(), Reprise::default());
    // Les DSN reprises : une par mois declare (la phrase de l'etat dit des declarations, pas des versements).
    let mut mois_dsn: BTreeSet<String> = BTreeSet::new();
    if let Ok(Value::Object(f)) = lire_json_ou_vide(&racine.join("historique").join("paies.json")) {
        let le_paies = le_fichier(&f);
        for x in f.get("paies").and_then(Value::as_array).into_iter().flatten() {
            let Some(o) = x.as_object() else { continue };
            let le = o.get("importe_le").filter(|v| v.as_str().is_some_and(|s| !s.is_empty())).or(le_paies.as_ref());
            let par = o.get("declare_par").and_then(Value::as_str).unwrap_or("");
            // Une paie declaree par une DSN et completee par un bulletin compte des deux cotes : elle a les deux sources.
            if par != "bulletin" {
                dsn.voir(o.get("mois"), o.get("mois"), le);
                if let Some(m) = o.get("mois").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                    mois_dsn.insert(m.to_string());
                }
            }
            if par == "bulletin" || o.get("bulletin").is_some_and(vrai) {
                bulletins.voir(o.get("mois"), o.get("mois"), le);
            }
        }
    }
    let copies = std::fs::read_dir(racine.join("historique").join("bulletins"))
        .map(|d| d.flatten().filter(|e| e.path().is_file()).count())
        .unwrap_or(0);
    let mut out = Vec::new();
    let base = |source: &str, quoi: &str, r: &Reprise| {
        json!({"source": source, "quoi": quoi, "nombre": r.nombre, "du": r.du.clone().unwrap_or_default(),
               "au": r.au.clone().unwrap_or_default(), "le": r.le.clone().unwrap_or_default()})
    };
    if dsn.nombre > 0 {
        let mut d = base("DSN", "versements déclarés", &dsn);
        d["declarations"] = json!(mois_dsn.len());
        out.push(d);
    }
    if aem.nombre > 0 {
        out.push(base("AEM", "contrats attestés", &aem));
    }
    if bulletins.nombre > 0 || contrats_bulletins.nombre > 0 || copies > 0 {
        let mut b = base("bulletins", "bulletins de paie", &bulletins);
        if b["le"] == json!("") {
            b["le"] = json!(contrats_bulletins.le.clone().unwrap_or_default());
        }
        b["contrats"] = json!(contrats_bulletins.nombre);
        b["copies"] = json!(copies);
        out.push(b);
    }
    out
}

/// `editeur` de `importation-etat` : les incidents (`journal/incidents.json`) et les signalements
/// (`journal/signalements.json`) de ce poste qui parlent de l'import -- un incident de nature `refus` n'en est pas : la
/// phrase a ete dite a l'ecran. Rien que ce qu'ils portent deja (caviarde).
fn attend_l_editeur(racine: &Path) -> Vec<Value> {
    let parle_d_import = |t: &str| {
        let t = t.to_lowercase();
        t.contains("import") || t.contains("repris")
    };
    let texte = |v: &Value, cle: &str| v.get(cle).and_then(Value::as_str).unwrap_or("").to_string();
    let mut out = Vec::new();
    for l in crate::incidents::lire(racine, None, false).unwrap_or_default() {
        let (ou_, genre, resume) = (texte(&l, "ou"), texte(&l, "genre"), texte(&l, "resume"));
        // Un refus (un dossier mal tape, un export absent) se dit a l'ecran ; il n'attend pas l'editeur.
        if l.get("quoi").and_then(Value::as_str) == Some(crate::incidents::REFUS) || !parle_d_import(&format!("{} {}", ou_, genre)) {
            continue;
        }
        let etat = l.get("envoi").map_or(String::new(), |e| texte(e, "etat"));
        if etat == crate::incidents::SANS_OBJET {
            continue;
        }
        out.push(json!({
            "canal": "incident", "quand": texte(&l, "quand"),
            "resume": if resume.is_empty() { genre } else { resume },
            "parti": etat == crate::incidents::DEPOSE, "repondu": false,
        }));
    }
    for s in crate::problemes::signalements(racine) {
        let debut = texte(&s, "debut");
        if !parle_d_import(&debut) {
            continue;
        }
        out.push(json!({
            "canal": "signalement", "quand": texte(&s, "quand"), "resume": debut,
            "numero": s.get("numero").cloned().unwrap_or(Value::Null),
            "parti": s.get("etat").and_then(Value::as_str) == Some("arrive"), "repondu": s.get("reponse").is_some_and(vrai),
        }));
    }
    out
}

// ------------------------------------------------------------------ la mise a jour

/// `maj.courante(code)` : le contenu du fichier `VERSION`, ou `0.0.0`.
pub(crate) fn courante(code: &Path) -> R<String> {
    let p = code.join("VERSION");
    if p.exists() {
        Ok(lire_texte(&p)?.trim().to_string())
    } else {
        Ok("0.0.0".to_string())
    }
}

/// `maj.prete` : la version telechargee et controlee de l'installation (`maj::dossier_maj`), ou `None`.
pub(crate) fn prete(inst: &Path) -> R<Option<Value>> {
    let dossier = crate::maj::dossier_maj(inst);
    let Some(d) = lire_json_tolerant(&dossier.join("prete.json"))? else {
        return Ok(None);
    };
    let o = objet(&d)?;
    let version = o.get("version").cloned().unwrap_or_else(|| json!(""));
    let Value::String(v) = &version else {
        return Err(erreur("TypeError", format!("prete.json : 'version' : {}", socle::fichiers::attendu("un texte", nom_type(&version)))));
    };
    if !dossier.join(v).join("logiciel").join("donnees").is_dir() {
        return Ok(None);
    }
    Ok(Some(d))
}

/// `maj.mois_saisis_non_deposes` : les mois qui montrent au
/// moins un engagement et dont aucune DSN reelle n'est acceptee (ou dont la
/// derniere est refusee).
pub(crate) fn mois_saisis_non_deposes(ctx: &Contexte) -> R<Vec<Value>> {
    let mut m = Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for mois in crate::routes::mois::mois_saisis(&ctx.racine) {
        let Some(d) = engagement::stock::vue_du_mois(&ctx.racine, &mois).map_err(Echec::from)? else {
            continue;
        };
        if !d.is_object() || !vrai(get(&d, "engagements")?) {
            continue;
        }
        let e = m.dsn_du_mois(&mois)?;
        if !vrai(&e) || vrai(get(&e, "refusee")?) {
            out.push(json!(mois));
        }
    }
    Ok(out)
}

/// `operations_ouverture.version` : la version du logiciel, ce qui est pret, ce
/// qui est configure. Ne va voir aucun depot.
pub(crate) fn version(ctx: &Contexte) -> R<Value> {
    let code = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    // La mise a jour est celle de l'INSTALLATION (lot U1) : son reglage (`maj::reglage`), sa version prete.
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let (depot, editeur) = crate::maj::reglage(&inst).map_err(|e| erreur("ValueError", e))?;
    // Le depot d'ou l'on recoit : celui qui est pose, sinon celui du moteur (`maj::DEPOT_PAR_DEFAUT`, lot S1).
    let depot = json!(crate::maj::depot(depot));
    let prete = prete(&inst)?;
    let courante_v = courante(&code)?;
    // Ce que CETTE compagnie retient (ses mois saisis sans DSN acceptee) ; une fictive ou une copie ne retient rien
    // (`maj::mois_qui_retiennent`, qui regarde toutes les compagnies reelles de l'installation).
    let attend: Vec<Value> = match &prete {
        _ if crate::interface::compagnies::est_copie(&ctx.racine) => Vec::new(),
        Some(pr) if vrai(get(pr, "malgre_calcul")?) => Vec::new(),
        Some(pr) if vrai(get(pr, "calcul")?) => crate::maj::mois_qui_retiennent_la_compagnie(ctx)?,
        _ => Vec::new(),
    };
    let avant = if code.join(".avant").join("donnees").is_dir() { json!(courante(&code.join(".avant"))?) } else { Value::Null };
    let (version_prete, calcul, exigee) = match &prete {
        Some(pr) => (pr["version"].clone(), ou_texte(get(pr, "calcul")?), ou_texte(get(pr, "exigee")?)),
        None => (Value::Null, json!(""), json!("")),
    };
    Ok(json!({
        "courante": courante_v, "editeur": editeur, "depot": depot,
        "prete": version_prete, "calcul": calcul, "exigee": exigee,
        "attend": attend,
        // La cle publique de l'editeur est ecrite dans `maj.py` : toujours posee.
        "signee": true,
        "avant": avant,
    }))
}

// ------------------------------------------------------------------ les signalements

/// La route `signalements` : `{"signalements", "non_lues"}`.
pub(crate) fn signalements_route(racine: &Path) -> R<Value> {
    let liste = crate::problemes::signalements(racine);
    // `reponses_non_lues` : les reponses que la boite n'a pas encore montrees.
    let mut non_lues = 0;
    for l in &liste {
        if vrai(get(l, "reponse")?) && !vrai(get(l, "lu")?) {
            non_lues += 1;
        }
    }
    Ok(json!({"signalements": liste, "non_lues": non_lues}))
}

// ------------------------------------------------------------------ les pieces parties

/// `_chemin_piece` : le fichier d'une piece partie existe-t-il, dans
/// `sorties/<mois>/` ou son `envoi/` ? Un NOM, jamais un chemin.
fn piece_existe(racine: &Path, mois: &Value, fichier: &Value) -> R<bool> {
    if !vrai(mois) || !vrai(fichier) {
        return Ok(false);
    }
    let nom = valeur_en_texte(fichier);
    if ["/", "\\", ".."].iter().any(|x| nom.contains(x)) {
        return Ok(false);
    }
    let Value::String(f) = fichier else {
        return Err(erreur("TypeError", format!("'fichier' : {}", socle::fichiers::attendu("un texte", nom_type(fichier)))));
    };
    let base = racine.join("sorties").join(valeur_en_texte(mois));
    Ok([base.join("envoi").join(f), base.join(f)].iter().any(|c| c.is_file()))
}

fn piece(racine: &Path, quoi: &str, quand: &Value, mois: &Value, fichier: &Value) -> R<Value> {
    Ok(json!({"quoi": quoi, "quand": quand, "mois": mois, "fichier": fichier, "existe": piece_existe(racine, mois, fichier)?}))
}

/// `(c.get("salarie"), c.get("debut")) == (salarie, debut)`.
fn est_cette_paie(c: &Value, salarie: &Value, debut: &Value) -> R<bool> {
    Ok(valeurs_egales(get(c, "salarie")?, salarie) && valeurs_egales(get(c, "debut")?, debut))
}

/// `pieces_parties` : les FICHIERS partis pour une paie -- la DPAE deposee, le
/// contrat envoye a la signature, le contrat revenu signe et la preuve de la
/// signature. Lu au journal ; rien ne part.
fn pieces_parties(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = |k: &str| charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
    let salarie = cle("salarie")?;
    let debut = cle("debut")?;
    let racine = ctx.racine.as_path();
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;

    let mut dpae = Vec::new();
    for l in etats::filtrer(&lignes, Some(DPAE), None, true)? {
        if !vrai(get(l, "accepte")?) || !vrai(get(l, "fichier")?) {
            continue;
        }
        let embauches = items(getd(ou(get(l, "detail")?, objet_vide()), "embauches", &etats::LISTE_VIDE)?)?;
        let mut trouve = false;
        for e in &embauches {
            if est_cette_paie(e, salarie, debut)? {
                trouve = true;
                break;
            }
        }
        if trouve {
            dpae.push(piece(racine, "DPAE déposée", get(l, "quand")?, get(l, "mois")?, get(l, "fichier")?)?);
        }
    }

    let mut contrat: Vec<Value> = Vec::new();
    for l in etats::filtrer(&lignes, Some(SIGNATURE), None, false)? {
        if !vrai(get(l, "accepte")?) {
            continue;
        }
        let parti = if l.pointer("/detail/main_propre").is_some_and(vrai) { "imprimé" } else { "envoyé à la signature" };
        for c in items(getd(ou(get(l, "detail")?, objet_vide()), "contrats", &etats::LISTE_VIDE)?)? {
            if !(est_cette_paie(&c, salarie, debut)? && vrai(get(&c, "fichier")?)) {
                continue;
            }
            let quoi = if est(get(&c, "acte")?, "avenant") {
                format!("Avenant n° {} {}", valeur_en_texte(get(&c, "avenant")?), parti)
            } else if est(get(&c, "acte")?, "remplace") {
                format!("Contrat qui annule et remplace le n° {}, {}", valeur_en_texte(get(&c, "remplace")?), parti)
            } else {
                format!("Contrat {}", parti)
            };
            let mois = ou(get(&c, "mois")?, get(l, "mois")?);
            contrat.push(piece(racine, &quoi, get(l, "quand")?, mois, get(&c, "fichier")?)?);
        }
    }
    for l in etats::filtrer(&lignes, Some(CONTRAT_RECU), None, false)? {
        let d = ou(get(l, "detail")?, objet_vide());
        if !vrai(get(l, "accepte")?) || !est_cette_paie(d, salarie, debut)? {
            continue;
        }
        if vrai(get(d, "fichier_signe")?) {
            let quoi = if est(get(d, "acte")?, "avenant") {
                format!("Avenant n° {} signé, contresigné", valeur_en_texte(get(d, "avenant")?))
            } else {
                "Contrat signé, contresigné".to_string()
            };
            contrat.push(piece(racine, &quoi, get(l, "quand")?, get(l, "mois")?, get(d, "fichier_signe")?)?);
        }
        if vrai(get(d, "preuve")?) {
            contrat.push(piece(racine, "Réponse « Je signe » du salarié (preuve)", get(l, "quand")?, get(l, "mois")?, get(d, "preuve")?)?);
        }
    }
    // `contrat.sort(key=lambda x: x["quand"] or "")` : stable.
    contrat.sort_by_key(|x| if vrai(&x["quand"]) { valeur_en_texte(&x["quand"]) } else { String::new() });
    Ok(json!({"dpae": dpae, "contrat": contrat}))
}
