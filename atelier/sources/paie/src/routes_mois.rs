//! Les routes du mois : `etat-par-engagement`, `etat-du-mois`, `embauches-a-declarer`, `mois-a-ouvrir`, et les lectures
//! qu'elles partagent (`bilan-du-mois`, `fichiers-deposables`) : l'etat de chaque engagement (sa validation, sa DPAE,
//! son contrat, ses documents, la DSN du mois), l'etat du mois, et l'annule-et-remplace.
//!
//! UN PROCESSUS PAR ROUTE, ET CHAQUE FICHIER LU UNE FOIS : le journal des
//! depots, la saisie de chaque mois et le Contexte ne se relisent pas d'un
//! etat a l'autre. Les etats du journal sont ceux de `etats.rs`, appeles
//! directement sur les lignes deja lues.
//!
//! AUCUN CALCUL D'UN ENGAGEMENT VALIDE. La saisie se charge sans calculer (`charger_saisie` : les engagements, le
//! contrat qui deborde, les numeros attribues en memoire). Le seul montant affiche, le `brut` de `etat-par-engagement`,
//! se lit dans la paie ecrite d'un engagement valide ; il ne se calcule que pour un engagement en saisie, et seulement si
//! la route le demande (`bilan-du-mois` et `mois-a-ouvrir` ne le demandent jamais).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use engagement::{self, LignesPermanent};
use crate::etats::{self, est, get, getd, joindre, objet_vide, ou, quand_lisible, texte_vide, Dico, DOCUMENTS, DPAE, DSN, LISTE_VIDE, SIGNATURE};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{sha256_hex, vrai};
use socle::num::Decimal;
use socle::echec::Echec;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 6] = ["etat-par-engagement", "etat-du-mois", "embauches-a-declarer", "mois-a-ouvrir", "bilan-du-mois", "fichiers-deposables"];

pub(crate) type Paire = (String, String);
pub(crate) use crate::etats::Cle3;

/// `ETAPES_DEPOT` : le chemin d'un depot, dans l'ordre ou il se fait.
const ETAPES_DEPOT: [(&str, &str, &str); 5] = [
    ("ecrite", "écrite — à tester", "Le fichier est écrit : le dépôt de test vient d'abord."),
    ("en_test", "en test — compte rendu attendu", "Déposée en test, compte rendu attendu : le dépôt réel se fait une fois le test revenu conforme."),
    ("testee", "testée — prête à envoyer", "Testée, conforme : il ne reste qu'à l'envoyer pour de vrai."),
    ("test_refuse", "test refusé — à corriger", "Le test a été refusé : le même refus attendrait le dépôt réel."),
    (
        "paie_changee",
        "la paie a changé — à réécrire et tester",
        "Le fichier ne déclare plus la paie : réécrire le mois, le tester, puis l'envoyer.",
    ),
];

/// `RIEN_CE_MOIS` : un mois sans paie n'a ni DPAE, ni contrat, ni document ;
/// l'etape est « faite » au sens ou il n'y a rien a y faire.
fn rien_ce_mois() -> Value {
    json!({"couleur": "bien", "courte": "rien ce mois-ci", "vide": true,
           "phrase": "aucune paie ce mois-ci : rien à faire à cette étape"})
}

// ------------------------------------------------------------ petits outils

fn dupliquer(e: &Echec) -> Echec {
    Echec { classe: e.classe, message: e.message.clone() }
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `un if k == 1 else des` : le pluriel de `_pluriel`, qui compte zero au pluriel.
fn plur(k: usize, un: &'static str, des: &'static str) -> &'static str {
    if k == 1 {
        un
    } else {
        des
    }
}

/// Un refus du chargement d'une saisie. Les refus que l'ecran ne tient pas
/// pour un defaut du logiciel -- le spectacle pas encore choisi, les jours ou
/// les heures pas encore saisis (`SaisieIncomplete`, leur liste est
/// `engagement::saisie_incomplete`) -- gardent leur classe ; tout autre est
/// celui du moteur (`CalculRefuse` cote Python).
pub fn classer_saisie(message: String) -> Echec {
    let incomplete = engagement::saisie_incomplete(&message);
    Echec { classe: if incomplete { Some("SaisieIncomplete") } else { None }, message }
}

/// `d["cle"]` sur un dictionnaire de fiche : `KeyError` si elle manque.
fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    d.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn paire(cle: &Value, debut: &Value) -> Option<Paire> {
    Some((cle.as_str()?.to_string(), debut.as_str()?.to_string()))
}

/// Les `[a, b, valeur]` rendus par `etats.rs` pour un dictionnaire a cle
/// (salarie, premier jour), en table. Une cle qui n'est pas faite de deux
/// textes ne peut egaler aucune cle (salarie, debut) de la saisie.
fn en_table(v: Value) -> HashMap<Paire, Value> {
    let mut out = HashMap::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b, x] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(k) = paire(a, b) {
                out.insert(k, x.clone());
            }
        }
    }
    out
}

/// Les `[salarie, debut, numero, valeur]` rendus par `etats.rs` pour un dictionnaire a cle de CONTRAT.
fn en_table3(v: Value) -> HashMap<Cle3, Value> {
    let mut out = HashMap::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b, c, x] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let (Some(a), Some(b), Some(c)) = (a.as_str(), b.as_str(), c.as_str()) {
                out.insert((a.to_string(), b.to_string(), c.to_string()), x.clone());
            }
        }
    }
    out
}

fn cles_en_ensemble3(v: Value) -> HashSet<Cle3> {
    let mut out = HashSet::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b, c] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let (Some(a), Some(b), Some(c)) = (a.as_str(), b.as_str(), c.as_str()) {
                out.insert((a.to_string(), b.to_string(), c.to_string()));
            }
        }
    }
    out
}

/// Le premier etat que le journal donne a l'une de ces cles (celles d'une paie : `etats::cles_des_paies`).
pub(crate) fn premier<'a, V>(table: &'a HashMap<Cle3, V>, cles: &[Cle3]) -> Option<&'a V> {
    cles.iter().find_map(|k| table.get(k))
}

pub(crate) fn dans_l_ensemble(ensemble: &HashSet<Cle3>, cles: &[Cle3]) -> bool {
    cles.iter().any(|k| ensemble.contains(k))
}

/// Le (salarie, debut, numero propre) de chaque paie de la saisie, dans l'ordre.
pub(crate) fn contrats_des_paies(paies: &[Paie]) -> Vec<Cle3> {
    paies
        .iter()
        .map(|p| (p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default(), p.engagement.debut.iso(), p.engagement.numero.clone().unwrap_or_default()))
        .collect()
}

fn cles_en_ensemble(v: Value) -> HashSet<Paire> {
    let mut out = HashSet::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(k) = paire(a, b) {
                out.insert(k);
            }
        }
    }
    out
}

/// `str(Decimal(str(x)).quantize(Decimal("0.01")))` : un montant a deux decimales.
fn brut_en_texte(v: &Value) -> R<String> {
    let t = valeur_en_texte(v);
    let mut d = Decimal::from_str(t.trim())
        .or_else(|_| Decimal::from_scientific(t.trim()))
        .map_err(|_| erreur("InvalidOperation", socle::num::illisible(&t)))?;
    d = d.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven);
    d.rescale(2);
    Ok(d.to_string())
}

/// `json.dumps(_sans_bruit(x))` de `operations_remplacement` : sans les cles
/// `_...`, ni `valide` ni `rouverte` -- ce qui change dans une saisie sans
/// changer la paie.
pub(crate) fn sans_bruit(x: &Value) -> Value {
    match x {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !k.starts_with('_') && *k != "valide" && *k != "rouverte")
                .map(|(k, v)| (k.clone(), sans_bruit(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sans_bruit).collect()),
        autre => autre.clone(),
    }
}

pub(crate) fn empreinte_de(v: &Value) -> String {
    sha256_hex(socle::ecriture::json_trie(&sans_bruit(v)).as_bytes())[..16].to_string()
}

/// Les mois que la saisie a touches (`engagement::stock::mois_saisis`), du plus ancien au plus recent.
pub fn mois_saisis(racine: &Path) -> Vec<String> {
    engagement::stock::mois_saisis(racine).unwrap_or_default()
}

/// L'ORDRE DES FICHIERS DEPOSABLES D'UN MOIS (`fichiers-deposables`), le meme partout (#236) : plus jamais la date de
/// modification du fichier, que deux machines -- ou deux rejeux -- ne donnent pas pareille.
/// - Les DSN : D'ABORD LA MENSUELLE (nature 01 ; un etablissement par fichier, le principal `DSN_<Mois>.dsn` avant les
///   `DSN_<Mois>_<NIC>.dsn`, par le nom) : c'est « la DSN du mois » que l'ecran et l'echeance attendent. D'un meme
///   etablissement, LA DERNIERE ECRITE D'ABORD (#253, 23/10/2026) : un annule-et-remplace s'ecrit a cote de la DSN deja
///   partie, sous `DSN_<Mois>_AR<ordre>.dsn` (`produire_mois`), et c'est lui qui part ; il se reconnait a son numero
///   d'ordre (S20.G00.05.004, ou `_AR<n>` du nom), le plus grand d'abord. Rangees par le seul nom, la DSN deja deposee
///   (`DSN_Janvier2025.dsn`) passait avant son annule-et-remplace (`DSN_Janvier2025_AR2.dsn`) : la boite « Deposer » et
///   l'etape DSN montraient, testaient et auraient redepose l'ancienne. PUIS LES SIGNALEMENTS (arret, fin de contrat,
///   amorcage...), dans l'ordre de leur numero d'ordre (S20.G00.05.004) : unique et jamais remis a zero, il dit l'ordre ou
///   le moteur les a produits (`ordre_signalements`, cahier technique DSN 2026.1 p. 136), et chacun part a son tour.
/// - Les DPAE : la plus recemment produite d'abord, lue dans son nom (`DPAE_<Mois>_<AAAAMMJJ-HHMM>.txt` apres
///   `DPAE_<Mois>.txt` : l'ordre inverse des noms).
pub(crate) fn ordre_de_depot(nom: &str, chemin: &Path) -> (u8, i64, String, std::cmp::Reverse<String>) {
    if suffixe(nom).to_lowercase() == ".dsn" {
        let apercu = std::fs::read(chemin).map(|o| crate::depot::lire(&o)).ok();
        let signalement = apercu.as_ref().is_some_and(|a| a.est_signalement());
        let lu = apercu.as_ref().and_then(|a| a.champ("ordre").trim().parse::<i64>().ok());
        if signalement {
            return (1, lu.unwrap_or(0), nom.to_string(), std::cmp::Reverse(String::new()));
        }
        let (etablissement, du_nom) = sans_annule_et_remplace(nom);
        return (0, 0, etablissement, std::cmp::Reverse(format!("{:020}", lu.or(du_nom).unwrap_or(1).max(0))));
    }
    (0, 0, String::new(), std::cmp::Reverse(nom.to_string()))
}

/// `DSN_<Mois>[_<NIC>]_AR<n>.dsn` -> (`DSN_<Mois>[_<NIC>].dsn`, Some(n)) ; un autre nom reste tel quel.
fn sans_annule_et_remplace(nom: &str) -> (String, Option<i64>) {
    if let Some(corps) = nom.strip_suffix(".dsn").or_else(|| nom.strip_suffix(".DSN")) {
        if let Some(i) = corps.rfind("_AR") {
            if let Ok(n) = corps[i + 3..].parse::<i64>() {
                return (format!("{}.dsn", &corps[..i]), Some(n));
            }
        }
    }
    (nom.to_string(), None)
}

/// `Path.suffix`.
fn suffixe(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[i..],
        _ => "",
    }
}

/// `Path.stem`.
fn radical(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[..i],
        _ => nom,
    }
}

/// Ce qu'une saisie chargee sans calcul porte : le fichier lu, et une
/// « embauche » par engagement (le salarie, le spectacle, les dates, la nature
/// et le numero du contrat ; aucun montant).
pub(crate) struct SaisieMois {
    pub(crate) data: Value,
    pub(crate) paies: Vec<Paie>,
}

impl SaisieMois {
    /// `(cle du salarie, premier jour)` de chaque paie, dans l'ordre du fichier.
    pub(crate) fn rangs(&self) -> R<Vec<Paire>> {
        self.paies.iter().map(|p| Ok((texte_de(requis(&p.salarie, "_cle")?), p.engagement.debut.iso()))).collect()
    }

    /// `data.get("engagements", [])`.
    pub(crate) fn engagements(&self) -> &[Value] {
        self.data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn texte_de(v: &Value) -> String {
    valeur_en_texte(v)
}

/// Les etats du journal et de la saisie, calcules une fois par processus.
pub(crate) struct Globales {
    pub(crate) dpae: HashMap<Paire, Value>,
    // Les etats du contrat, rangés par CONTRAT (salarie, debut, numero) : deux contrats du meme jour ne se partagent rien.
    pub(crate) contrats: HashMap<Cle3, Value>,
    pub(crate) recus: HashMap<Cle3, Value>,
    pub(crate) recus_par: HashMap<Cle3, Value>,
    pub(crate) partis: HashMap<Cle3, Value>,
    pub(crate) renvoyes: HashSet<Cle3>,
    pub(crate) numeros_partis: HashMap<Cle3, Value>,
    pub(crate) avenants: HashMap<Cle3, Value>,
}

/// Tout ce qu'une route lit, lu une fois.
pub struct Moteur<'a> {
    o: &'a HashMap<String, String>,
    ctx: &'a Contexte,
    journal: Option<Result<std::sync::Arc<Vec<Value>>, Echec>>,
    globales: Option<Rc<Globales>>,
    fichiers: HashMap<String, Value>,
    compteurs: Option<Value>,
}

impl<'a> Moteur<'a> {
    pub fn nouveau(o: &'a HashMap<String, String>, ctx: &'a Contexte) -> Moteur<'a> {
        Moteur { o, ctx, journal: None, globales: None, fichiers: HashMap::new(), compteurs: None }
    }

    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    /// Les lignes de `journal/depots.json`, lues une fois (et le refus aussi).
    pub(crate) fn lignes(&mut self) -> R<std::sync::Arc<Vec<Value>>> {
        if self.journal.is_none() {
            self.journal = Some(etats::charger_partage(self.racine()));
        }
        match self.journal.as_ref().unwrap() {
            Ok(l) => Ok(l.clone()),
            Err(e) => Err(dupliquer(e)),
        }
    }

    fn nic(&self) -> String {
        etats::nic_principal(&self.ctx.entreprise)
    }

    /// `journal.etat_dsn_du_mois` : le dernier depot reel accepte, ou `Null`.
    pub(crate) fn dsn_du_mois(&mut self, mois: &str) -> R<Value> {
        let lignes = self.lignes()?;
        etats::dsn_du_mois(&lignes, &self.ctx.entreprise, mois)
    }

    // ------------------------------------------------------ la saisie du mois

    /// `operations_paies.charger(ctx, fichier)` sans calculer la paie.
    fn charger_saisie(&mut self, mois: &str) -> R<SaisieMois> {
        let data = engagement::saisie_du_mois(self.racine(), mois).map_err(classer_saisie)?;
        let _derivation = engagement::conges::Derivation::ouvrir();
        let lignes: LignesPermanent = &engagement::conges::lignes_du_mois;
        let (engs, _) = engagement::engagements(self.ctx, &data, lignes).map_err(classer_saisie)?;
        let mois_dit = data.get("mois").map(en_texte).ok_or_else(|| plain(socle::fichiers::absent("mois")))?;
        // Le salarie, son emploi et son spectacle d'abord, pour tous les
        // engagements (ce que le calcul lit avant de suivre les contrats) ; puis
        // le contrat qui deborde du mois reprend le numero de son premier mois.
        let mut paies = Vec::new();
        for e in &engs {
            paies.push(crate::amorcage::embauche(self.ctx, e).map_err(plain)?);
        }
        for p in paies.iter_mut() {
            engagement::suivre_contrat(self.ctx, &mut p.engagement).map_err(classer_saisie)?;
        }
        let annee: i32 = mois_dit.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| {
            erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(mois_dit.get(..4).unwrap_or(&mois_dit))))
        })?;
        self.numeroter(&mut paies, annee)?;
        Ok(SaisieMois { data, paies })
    }

    /// `ctx.prochain(cle, annee)` : le numero que le compteur proposerait.
    fn prochain(&mut self, cle: &str, annee: i32) -> R<i64> {
        if self.compteurs.is_none() {
            let p = self.racine().join("config").join("compteurs.json");
            let v = if p.exists() {
                let t = std::fs::read_to_string(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
                serde_json::from_str(&t).map_err(|e| erreur("JSONDecodeError", e.to_string()))?
            } else {
                Value::Object(Map::new())
            };
            self.compteurs = Some(v);
        }
        let valeur = self.compteurs.as_ref().unwrap().get(annee.to_string()).and_then(|a| a.get(cle));
        Ok(match valeur {
            None => 1,
            Some(Value::Number(n)) => n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64) + 1,
            Some(Value::Bool(b)) => i64::from(*b) + 1,
            Some(Value::String(s)) => {
                s.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(s))))? + 1
            }
            Some(autre) => {
                return Err(erreur(
                    "TypeError",
                    socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre)),
                ))
            }
        })
    }

    /// `engagement.numeroter`, la part des numeros de CONTRAT : un engagement
    /// qui porte deja son numero le garde ; les autres prennent la suite du
    /// compteur, en memoire (rien n'est reserve ni ecrit). Un mois qui n'est pas le premier du contrat n'en recoit pas :
    /// son numero est celui de l'engagement, pose quand le contrat part (`engagement::refus_sans_numero`).
    fn numeroter(&mut self, paies: &mut [Paie], annee: i32) -> R<()> {
        let mut seqs: HashMap<String, i64> = HashMap::new();
        for p in paies.iter_mut() {
            if p.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) || !p.engagement.premier_mois().map_err(plain)? {
                continue;
            }
            let s = &p.salarie;
            let cle = format!("contrat_{}", texte_de(requis(s, "_cle")?));
            let seq = match seqs.get(&cle) {
                Some(x) => x + 1,
                None => self.prochain(&cle, annee)?,
            };
            seqs.insert(cle, seq);
            let initiales = texte_de(requis(s, "initiales_contrat")?);
            let code = texte_de(requis(s, "code_salarie")?);
            let an2: String = annee.to_string().chars().skip(2).collect();
            p.engagement.numero = Some(format!("{}{}.{}.{:02}", initiales, code, an2, seq));
        }
        Ok(())
    }

    // --------------------------------------------- ce que le journal et la saisie disent

    pub(crate) fn globales(&mut self) -> R<Rc<Globales>> {
        if let Some(g) = &self.globales {
            return Ok(g.clone());
        }
        let lignes = self.lignes()?;
        let dpae = en_table(etats::etat_des_embauches(&lignes)?);
        let c = etats::Maille::Contrat;
        let contrats = en_table3(etats::contrats_demandes(&lignes, c)?);
        let recus = etats::contrats_recus(&lignes, c)?;
        let recus = en_table3(recus.sortie(Clone::clone));
        let recus_par = en_table3(etats::contrats_recus_comment(&lignes, c)?);
        let partis = en_table3(etats::empreintes_contrats_partis(&lignes, c)?);
        let renvoyes = cles_en_ensemble3(etats::renvoyes_depuis_la_signature(&lignes, c)?.cles());
        let numeros_partis = en_table3(etats::numeros_contrats_partis(&lignes, c)?);
        let avenants = en_table3(etats::avenants(&lignes, c)?);
        let g = Rc::new(Globales { dpae, contrats, recus, recus_par, partis, renvoyes, numeros_partis, avenants });
        self.globales = Some(g.clone());
        Ok(g)
    }

    /// `empreintes_engagements` : l'engagement resume en 16 caracteres, sans son numero.
    pub(crate) fn empreintes_engagements(s: &SaisieMois, rangs: &[Paire]) -> HashMap<Paire, String> {
        let eng = s.engagements();
        let mut out = HashMap::new();
        for (i, cle) in rangs.iter().enumerate() {
            if let Some(e) = eng.get(i) {
                let sans_numero: Value = match e {
                    Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| *k != "numero").map(|(k, v)| (k.clone(), v.clone())).collect()),
                    autre => autre.clone(),
                };
                out.insert(cle.clone(), empreinte_de(&sans_numero));
            }
        }
        out
    }

    /// L'EMPREINTE DES DOCUMENTS DE CHAQUE SALARIE (ticket #240) : ses engagements du mois, dans l'ordre de la saisie,
    /// resumes comme ceux d'un contrat (`empreintes_engagements` : sans numero, sans validation). Notee au journal a
    /// l'envoi des documents ; une empreinte qui a change depuis dit que ses documents ne sont plus les bons.
    /// `cles` : la cle du salarie de chaque engagement, dans le meme ordre.
    pub(crate) fn empreintes_documents(engagements: &[Value], cles: &[String]) -> HashMap<String, String> {
        let mut par_salarie: Vec<(String, Vec<Value>)> = Vec::new();
        for (e, cle) in engagements.iter().zip(cles) {
            let sans_numero: Value = match e {
                Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| *k != "numero").map(|(k, v)| (k.clone(), v.clone())).collect()),
                autre => autre.clone(),
            };
            match par_salarie.iter_mut().find(|(c, _)| c == cle) {
                Some((_, v)) => v.push(sans_numero),
                None => par_salarie.push((cle.clone(), vec![sans_numero])),
            }
        }
        par_salarie.into_iter().map(|(c, v)| (c, empreinte_de(&Value::Array(v)))).collect()
    }

    /// `numeros_saisis` : le numero de contrat ECRIT dans la saisie, et lui seul.
    pub(crate) fn numeros_saisis(s: &SaisieMois, rangs: &[Paire]) -> HashMap<Paire, Value> {
        let eng = s.engagements();
        let mut out = HashMap::new();
        for (i, cle) in rangs.iter().enumerate() {
            if let Some(n) = eng.get(i).and_then(|e| e.get("numero")).filter(|n| vrai(n)) {
                out.insert(cle.clone(), n.clone());
            }
        }
        out
    }

    /// Le `brut` de chaque engagement du mois, dans l'ordre de l'ecran : celui de sa paie ecrite s'il est valide, celui
    /// du calcul de la saisie sinon (un calcul seulement si un engagement est en saisie).
    fn brut_du_mois(&mut self, mois: &str, s: &SaisieMois) -> R<Vec<String>> {
        let etats = engagement::etats::du_mois(self.ctx, mois).map_err(plain)?;
        let ecrits: Vec<Option<Value>> = etats
            .iter()
            .map(|e| e.valide().and_then(|v| v.paie_ecrite(mois)).and_then(|p| p.pointer("/paie/brut")).cloned())
            .collect();
        let calcules = if ecrits.iter().any(Option::is_none) {
            let c = engagement::charger_donnees(self.ctx, &s.data).map_err(classer_saisie)?;
            c.paies.iter().map(|p| Some(Value::String(p.brut.to_string()))).collect()
        } else {
            vec![None; ecrits.len()]
        };
        ecrits.into_iter().zip(calcules.into_iter().chain(std::iter::repeat(None))).map(|(e, c)| brut_en_texte(&e.or(c).unwrap_or(Value::Null))).collect()
    }

    // ------------------------------------------------------------ etat_par_engagement

    /// `etat_par_engagement` : ou en est CHAQUE engagement du mois. `avec_brut` : le
    /// montant brut de chaque paie y figure ; le bilan, qui ne l'affiche pas,
    /// ne le demande jamais.
    pub fn etat_par_engagement(&mut self, mois: &str, avec_brut: bool) -> R<Vec<Value>> {
        // UN MOIS SANS ENGAGEMENT N'EST PAS UNE ERREUR (ticket #98).
        if !engagement::stock::a_une_saisie(self.racine(), mois).map_err(plain)? {
            return Ok(Vec::new());
        }
        let s = self.charger_saisie(mois)?;
        let bruts = if avec_brut { Some(self.brut_du_mois(mois, &s)?) } else { None };
        let rangs = s.rangs()?;
        // Valide, ou parti : la date de la validation (`engagement::etats`), engagement par engagement.
        let etats_eng = engagement::etats::du_mois(self.ctx, mois).map_err(plain)?;
        let g = self.globales()?;
        // Par RANG dans la saisie : deux contrats du meme salarie et du meme jour ont chacun leur empreinte et leur numero.
        let uniques: Vec<Paire> = rangs.iter().enumerate().map(|(i, (c, d))| (c.clone(), format!("{}#{}", d, i))).collect();
        let actuelles = Self::empreintes_engagements(&s, &uniques);
        let saisis = Self::numeros_saisis(&s, &uniques);
        let lignes = self.lignes()?;
        let cles_paies = etats::cles_des_paies(&lignes, &contrats_des_paies(&s.paies))?;
        let docs = etats::documents_partis(&lignes, mois)?;
        let cles_saisies: Vec<String> = rangs.iter().map(|(c, _)| c.clone()).collect();
        let empreintes_docs = Self::empreintes_documents(s.engagements(), &cles_saisies);
        let actes = etats::etat_du_mois(&lignes, mois)?;
        let depot_dsn = actes.get(DSN).cloned().unwrap_or(Value::Null);

        let mut out = Vec::new();
        for (i, p) in s.paies.iter().enumerate() {
            let (cle, debut) = rangs[i].clone();
            let k = (cle.clone(), debut.clone());
            let sal = &p.salarie;
            let nat = p.engagement.nat().map_err(plain)?;
            let nom_famille = texte_de(requis(sal, "nom")?);
            let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
                Some(v) => texte_de(v),
                None => texte_de(requis(sal, "prenom")?),
            };
            let spectacle = if vrai(&p.spectacle) { requis(&p.spectacle, "titre")?.clone() } else { Value::Null };
            let mois_du_contrat = p.engagement.mois_du_contrat().map_err(plain)?;
            if mois_du_contrat.is_empty() {
                return Err(erreur("IndexError", socle::fichiers::trop_courte("les mois du contrat")));
            }
            let premier_mois = p.engagement.premier_mois().map_err(plain)?;
            let dernier_mois = p.engagement.dernier_mois().map_err(plain)?;
            let valide = etats_eng.get(i).and_then(|e| e.valide()).map_or(Value::Null, |v| Value::String(v.le().to_string()));
            // Un CDI n'a pas de fin : `null`, comme `Engagement::canon`.
            let fin = p.engagement.fin.map_or(Value::Null, |f| Value::String(f.iso()));
            let mut ligne = Map::new();
            ligne.insert("salarie".into(), Value::String(cle.clone()));
            ligne.insert("nom".into(), Value::String(format!("{} {}", nom_famille, prenom)));
            ligne.insert("spectacle".into(), spectacle);
            ligne.insert("nature".into(), Value::String(nat.libelle.clone()));
            ligne.insert("numero".into(), p.engagement.numero.clone().map_or(Value::Null, Value::String));
            ligne.insert("debut".into(), Value::String(debut.clone()));
            ligne.insert("fin".into(), fin);
            ligne.insert("emploi".into(), sal.get("emploi").cloned().unwrap_or(Value::Null));
            ligne.insert("statut".into(), sal.get("statut").cloned().unwrap_or(Value::Null));
            if let Some(b) = &bruts {
                // Une paie par engagement de la saisie : qu'il en manque une est un defaut du logiciel, jamais la saisie de
                // l'utilisateur, et la phrase le lui dit.
                let brut = b.get(i).cloned().ok_or_else(|| Echec {
                    classe: Some("DefautDuLogiciel"),
                    message: format!(
                        "Le montant de la paie de {} {} ne se lit pas : le calcul de {} ne correspond pas à la saisie \
                         enregistrée. Ce n'est pas votre saisie qui est en cause : c'est un défaut du logiciel, \
                         noté dans ses incidents.",
                        nom_famille, prenom, mois
                    ),
                })?;
                ligne.insert("brut".into(), Value::String(brut));
            }
            ligne.insert("valide".into(), valide);
            // RECALCULEE D'ELLE-MEME (`engagement::etats::du_mois`) : pourquoi, et quand.
            let recalcul = etats_eng.get(i).and_then(|e| e.valide()).and_then(|v| v.recalcul(mois)).and_then(engagement::etats::phrase_du_recalcul);
            ligne.insert("recalcul".into(), recalcul.map_or(Value::Null, Value::String));
            ligne.insert(
                "contrat".into(),
                json!({"premier_mois": premier_mois, "dernier_mois": dernier_mois,
                       "depuis": mois_du_contrat.first(), "jusqu_a": mois_du_contrat.last()}),
            );
            // CE QUE CE SALARIE RECOIT POUR CETTE PAIE (#236) : le bulletin s'il y a une paie, le certificat Conges Spectacles
            // du contrat d'usage qui finit (`routes_dossier_salarie::certificats_dus`), l'AER Cinema Spectacle qui lui est due
            // (`aer::due`). L'etape « Documents » du mois les nomme d'apres ceci, et rien d'autre.
            let mut pieces: Vec<&str> = Vec::new();
            if !p.engagement.sans_paie() {
                pieces.push("bulletin");
            }
            if !nat.mensuelle() && dernier_mois && engagement::calcul::declaree_a_la_caisse_des_conges(p).map_err(plain)? {
                pieces.push("certificat_cs");
            }
            if crate::aer::due(&p.engagement).map_err(plain)? {
                pieces.push("aer");
            }
            ligne.insert("pieces".into(), json!(pieces));
            let mut dpae_v = g.dpae.get(&k).filter(|v| vrai(v)).unwrap_or(objet_vide());
            // UNE DATE D'EMBAUCHE CHANGEE apres la limite (21/10/2026) : la DPAE de l'ancienne date vaut-elle
            // (`routes_regularisation::dpae_d_une_date_changee`) ?
            let mut date_changee = Value::Null;
            if !vrai(dpae_v) {
                let ancienne = p.engagement.numero.as_deref().and_then(|n| crate::routes_regularisation::debut_declare_avant(self.ctx, &cle, n, &debut));
                if let Some((avant, deposee)) = ancienne.and_then(|a| g.dpae.get(&(cle.clone(), a.clone())).filter(|v| vrai(v)).map(|v| (a, v))) {
                    let (vaut, phrase) = crate::routes_regularisation::dpae_d_une_date_changee(&avant, &debut);
                    if vaut {
                        dpae_v = deposee;
                    }
                    date_changee = json!({"avant": avant, "vaut": vaut, "phrase": phrase});
                }
            }
            let dpae = json!({
                "date_changee": date_changee,
                "fait": get(dpae_v, "quand")?,
                "passee": vrai(get(dpae_v, "passee")?),
                "refusee": vrai(get(dpae_v, "refusee")?),
                "anomalies": ou(get(dpae_v, "anomalies")?, &LISTE_VIDE),
                "dossier": ou(get(dpae_v, "dossier")?, texte_vide()),
                "deja_existante": vrai(get(dpae_v, "deja_existante")?),
                "fin_declaree": if nat.date_fin_dpae() { get(dpae_v, "fin")?.clone() } else { Value::Null },
                "maille": "paie",
            });
            let cles = &cles_paies[i];
            let rang_du_jour = rangs[..i].iter().filter(|r| **r == rangs[i]).count();
            // Son rang parmi les contrats du meme salarie et du meme jour : ce que l'ecran rend a `valider-engagement` quand
            // le contrat n'a pas encore de numero (`jumeaux`).
            ligne.insert("rang".into(), json!(rang_du_jour));
            let ecart = if vrai(&p.spectacle) {
                crate::avenant::ecart_du_contrat(self.ctx, mois, &cle, &debut, Some(cles[0].2.as_str()), rang_du_jour)
            } else {
                String::new()
            };
            let nul = Value::Null;
            let mut signature = etat_signature(
                premier(&g.contrats, cles).unwrap_or(&nul),
                premier(&g.recus, cles).unwrap_or(&nul),
                premier(&g.recus_par, cles).unwrap_or(&nul),
                premier(&g.partis, cles).unwrap_or(&nul),
                actuelles.get(&uniques[i]).map(String::as_str),
                mois,
                dans_l_ensemble(&g.renvoyes, cles),
                premier(&g.numeros_partis, cles).unwrap_or(&nul),
                saisis.get(&uniques[i]).unwrap_or(&nul),
                premier(&g.avenants, cles).unwrap_or(&nul),
                &ecart,
            );
            // UN CDI EN COURS N'A PAS D'ETAT DE CONTRAT MENSUEL (30/09/2026).
            if nat.mensuelle() && nat.duree_indeterminee && !premier_mois {
                let recu = premier(&g.recus, cles).filter(|r| vrai(r));
                let depuis = mois_du_contrat.first().cloned().unwrap_or_default();
                let signe_en = match recu {
                    Some(r) => valeur_en_texte(r).chars().take(7).collect::<String>(),
                    None => depuis.clone(),
                };
                signature["cdi_en_cours"] = json!({"depuis": depuis, "signe_en": signe_en});
            }
            let documents = etat_documents(docs.get(&cle), empreintes_docs.get(&cle).map(String::as_str));
            let dsn = json!({"fait": if vrai(&depot_dsn) { get(&depot_dsn, "quand")?.clone() } else { Value::Null }, "maille": "mois"});
            ligne.insert("etats".into(), json!({"dpae": dpae, "signature": signature, "documents": documents, "dsn": dsn}));
            out.push(Value::Object(ligne));
        }
        Ok(out)
    }

    // ------------------------------------------------------ les fichiers deposables

    /// `fichiers_deposables` : les fichiers d'un mois qu'on peut deposer, par
    /// acte, le dernier ecrit en tete.
    pub fn fichiers_deposables(&mut self, mois: &str) -> R<Value> {
        if let Some(v) = self.fichiers.get(mois) {
            return Ok(v.clone());
        }
        let dossier = self.racine().join("sorties").join(mois);
        let rendu = if !dossier.is_dir() {
            json!({"dsn": [], "dpae": []})
        } else {
            let mut tous: Vec<(String, PathBuf, u64)> = Vec::new();
            let rd = std::fs::read_dir(&dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
            for e in rd.flatten() {
                let chemin = e.path();
                let Ok(m) = std::fs::metadata(&chemin) else { continue };
                if m.is_file() {
                    tous.push((e.file_name().to_string_lossy().into_owned(), chemin, m.len()));
                }
            }
            // L'ORDRE NE DEPEND PAS DE L'HEURE D'UN FICHIER (#236 : deux rejeux, deux machines, le meme ordre) : il se lit
            // dans ce que le moteur sait de chacun (`ordre_de_depot`).
            let racine = self.racine().to_path_buf();
            let ranger = |mut v: Vec<&(String, PathBuf, u64)>| -> Value {
                v.sort_by_cached_key(|f| ordre_de_depot(&f.0, &f.1));
                Value::Array(
                    v.into_iter()
                        .map(|(nom, chemin, octets)| {
                            // « ecrit le ... » : ce que le moteur a consigne en l'ecrivant, pas la date du fichier (#236).
                            json!({"nom": nom, "chemin": chemin.to_string_lossy(), "octets": octets, "ecrit": crate::produire_mois::ecrit_le(&racine, chemin)})
                        })
                        .collect(),
                )
            };
            let dsn: Vec<_> = tous.iter().filter(|f| suffixe(&f.0).to_lowercase() == ".dsn").collect();
            let dpae: Vec<_> = tous
                .iter()
                .filter(|f| f.0.starts_with("DPAE_") && suffixe(&f.0).to_lowercase() == ".txt" && !radical(&f.0).ends_with("_recapitulatif"))
                .collect();
            json!({"dsn": ranger(dsn), "dpae": ranger(dpae)})
        };
        self.fichiers.insert(mois.to_string(), rendu.clone());
        Ok(rendu)
    }

    // ----------------------------------------------------- l'annule-et-remplace

    /// `empreinte_saisie` : la paie du mois, resumee en 16 caracteres ; `None`
    /// sans saisie ou si elle ne se lit pas.
    pub(crate) fn empreinte_saisie(&self, mois: &str) -> R<Option<String>> {
        Ok(engagement::stock::vue_du_mois(self.racine(), mois).map_err(plain)?.map(|v| empreinte_de(&v)))
    }

    /// `limite` : le dernier jour ou un annule-et-remplace est admis, jusqu'a minuit : la veille de l'echeance de depot
    /// (`echeance_dsn::Echeance::veille`, reportee comme elle).
    pub(crate) fn limite(&self, mois: &str) -> R<Jour> {
        let e = crate::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        e.veille().map_err(plain)
    }

    /// `_ordre_de_la_ligne` : le rang d'un depot reel du journal.
    pub(crate) fn ordre_de_la_ligne(&self, mois: &str, l: &Value) -> R<i64> {
        let d = ou(get(l, "detail")?, objet_vide());
        let ordre = get(d, "ordre")?;
        let t = if vrai(ordre) { valeur_en_texte(ordre) } else { String::new() };
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = t.parse::<i64>() {
                return Ok(n);
            }
        }
        let nom = get(l, "fichier")?;
        if vrai(nom) {
            let p = self.racine().join("sorties").join(mois).join(valeur_en_texte(nom));
            if p.exists() {
                let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
                if let Some(n) = rang_dans_le_fichier(&octets) {
                    return Ok(n);
                }
            }
        }
        // Une ligne reconstituee sans fichier : une normale porte l'ordre 1.
        Ok(1)
    }

    /// `depots_reels` : les depots REELS de DSN mensuelle du mois que la
    /// plate-forme a pris et qu'aucun bilan n'a refuses.
    pub(crate) fn depots_reels(&mut self, mois: &str, nic: &Value) -> R<Vec<Value>> {
        let lignes = self.lignes()?;
        let reels: Vec<Value> = etats::filtrer(&lignes, Some(DSN), Some(mois), true)?.into_iter().cloned().collect();
        let mut out = Vec::new();
        for l in etats::de_l_etablissement(&reels, nic, &self.nic())? {
            if !vrai(get(&l, "accepte")?) || etats::refuse_par_un_retour(&l)? {
                continue;
            }
            let defaut = Value::String("01".into());
            let t = getd(ou(get(&l, "detail")?, objet_vide()), "type", &defaut)?;
            if ["01", "02", "03", "05"].iter().any(|x| est(t, x)) {
                out.push(l);
            }
        }
        Ok(out)
    }

    /// `operations_remplacement.etat` : ou en est le mois vis-a-vis de
    /// l'annule-et-remplace.
    /// `nic` : l'etablissement dont on parle (`Null` : le principal).
    pub(crate) fn etat_remplacement(&mut self, mois: &str, nic: &Value) -> R<Value> {
        let faits = self.depots_reels(mois, nic)?;
        let lim = self.limite(mois)?;
        let admis = self.ctx.aujourdhui <= lim;
        let mut out = Map::new();
        out.insert("mois".into(), json!(mois));
        out.insert("deposee".into(), json!(!faits.is_empty()));
        out.insert("dernier_ordre".into(), Value::Null);
        out.insert("prochain_ordre".into(), json!(1));
        out.insert("limite".into(), json!(lim.iso()));
        out.insert("admis".into(), json!(admis));
        out.insert("saisie_modifiee".into(), Value::Null);
        out.insert("a_envoyer".into(), json!(false));
        out.insert("phrase".into(), json!(""));
        // Apres la limite : la correction de chaque contrat modifie est ecrite pour la DSN suivante.
        out.insert("correction_ecrite".into(), json!(false));
        let Some(derniere) = faits.last() else {
            return Ok(Value::Object(out));
        };
        let mut dernier = i64::MIN;
        for l in &faits {
            dernier = dernier.max(self.ordre_de_la_ligne(mois, l)?);
        }
        out.insert("dernier_ordre".into(), json!(dernier));
        out.insert("prochain_ordre".into(), json!(dernier + 1));
        let notee = get(ou(get(derniere, "detail")?, objet_vide()), "empreinte_saisie")?;
        let mut modifiee = false;
        if vrai(notee) {
            let maintenant = self.empreinte_saisie(mois)?;
            modifiee = maintenant.map_or(true, |m| !est(notee, &m));
            out.insert("saisie_modifiee".into(), json!(modifiee));
        }
        // Modifiee apres la limite, en le confirmant (21/10/2026) : la saisie telle que declaree est gardee, et dit ce qui a
        // change meme quand le depot n'a pas note d'empreinte.
        let couverture = if admis { None } else { crate::routes_regularisation::couverture(self.o, self.ctx, mois)? };
        if couverture.is_some() {
            let changes = crate::routes_regularisation::ce_qui_a_change(self.ctx, mois)?;
            if changes.is_some_and(|c| !c.contrats.is_empty() || c.retires > 0) {
                modifiee = true;
                out.insert("saisie_modifiee".into(), json!(true));
            }
        }
        let quand = quand_lisible(get(derniere, "quand")?);
        let jour = lim.jj_mm_aaaa();
        if let (true, Some(c)) = (modifiee && !admis, &couverture) {
            // CE QUI EST ECRIT POUR LA DSN SUIVANTE : une correction (« Corriger ce mois ») par contrat dont la paie a
            // change ou qui est ajoute, et qui declare la paie d'AUJOURD'HUI (`routes_regularisation::couverture`).
            let pluriel = |n: usize| if n > 1 { "s" } else { "" };
            let mut phrase = format!("Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit)", quand, jour);
            if c.a_ecrire == 0 && c.a_refaire == 0 && c.retires == 0 {
                out.insert("correction_ecrite".into(), json!(true));
                phrase.push_str(" — la correction est écrite (Corriger ce mois), elle part dans la DSN suivante");
            } else {
                if c.a_ecrire > 0 {
                    phrase.push_str(&format!(" — {} contrat{} à corriger dans la DSN suivante (Corriger ce mois)", c.a_ecrire, pluriel(c.a_ecrire)));
                }
                if c.a_refaire > 0 {
                    phrase.push_str(&format!(
                        " — la paie a encore changé depuis la correction écrite — à corriger à nouveau (Corriger ce mois, {} contrat{})",
                        c.a_refaire,
                        pluriel(c.a_refaire)
                    ));
                }
                if c.retires > 0 {
                    phrase.push_str(&format!(
                        " — {} contrat{} déclaré{} retiré{} de la saisie : un contrat déclaré ne se retire pas : il se remet, ses quantités à zéro",
                        c.retires,
                        pluriel(c.retires),
                        pluriel(c.retires),
                        pluriel(c.retires)
                    ));
                }
            }
            out.insert("phrase".into(), json!(phrase));
            return Ok(Value::Object(out));
        }
        if modifiee {
            if admis {
                out.insert("a_envoyer".into(), json!(true));
                out.insert(
                    "phrase".into(),
                    json!(format!("Paie modifiée depuis la DSN du {} — annule-et-remplace à envoyer, jusqu'au {} à minuit", quand, jour)),
                );
            } else {
                out.insert(
                    "phrase".into(),
                    json!(format!(
                        "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — la correction part dans la DSN suivante (Corriger ce mois)",
                        quand, jour
                    )),
                );
            }
        } else if admis {
            out.insert(
                "phrase".into(),
                json!(format!("Une modification reste possible : la DSN refaite partira en annule-et-remplace (ordre {}), jusqu'au {} à minuit", dernier + 1, jour)),
            );
        }
        Ok(Value::Object(out))
    }

    /// `paie_changee_depuis_l_ecriture` : `None` si le fichier declare la paie
    /// d'aujourd'hui, sinon POURQUOI.
    pub(crate) fn paie_changee_depuis_l_ecriture(&self, mois: &str, fichier: &Path) -> R<Option<String>> {
        let nom = fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let p = self.racine().join("journal").join("dsn-ecrites.json");
        let d: Value = if p.exists() {
            let t = std::fs::read_to_string(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
            serde_json::from_str(&t).unwrap_or_else(|_| Value::Object(Map::new()))
        } else {
            Value::Object(Map::new())
        };
        let note = get(&d, &format!("{}/{}", mois, nom))?;
        if vrai(note) && vrai(get(note, "empreinte_saisie")?) {
            let notee = get(note, "empreinte_saisie")?;
            let maintenant = self.empreinte_saisie(mois)?;
            if maintenant.map_or(true, |m| !est(notee, &m)) {
                return Ok(Some(format!("la paie a change depuis que ce fichier a ete ecrit (le {})", quand_lisible(get(note, "quand")?))));
            }
            return Ok(None);
        }
        // SANS EMPREINTE NOTEE, LE MOTEUR NE SAIT PAS (#236) : il ne compare plus les dates des fichiers. Une fictive
        // livree, decompressee chez un visiteur, a des dates de fichier qui ne disent rien de l'ordre ou elle a ete ecrite,
        // et chacun de ses mois se disait « paie changee ».
        Ok(None)
    }

    // ------------------------------------------------------------- le bilan

    /// `_etape_de_depot` : pose sur `bilan` ou en est le fichier a deposer.
    fn etape_de_depot(&mut self, mois: &str, quoi: &str, bilan: &mut Map<String, Value>) -> R<()> {
        // « Ecrite » pour une DSN : le meme critere que l'horloge, l'accueil et les rappels (`produire_mois::mois_ecrit`,
        // la DSN mensuelle notee au journal des ecritures), pas un fichier pose dans `sorties/<mois>/`.
        if quoi == DSN && !crate::produire_mois::mois_ecrit(self.racine(), mois) {
            return Ok(());
        }
        let fichiers = self.fichiers_deposables(mois)?;
        let Some(premier) = fichiers.get(quoi).and_then(Value::as_array).and_then(|l| l.first()).cloned() else {
            return Ok(());
        };
        let chemin = PathBuf::from(valeur_en_texte(&premier["chemin"]));
        let lignes = self.lignes()?;
        let empreinte = crate::journal::empreinte_fichier(&chemin).map_err(|m| erreur("FileNotFoundError", m))?;
        if vrai(&etats::deja_depose(&lignes, quoi, &empreinte)?) {
            return Ok(());
        }
        let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let r = etats::repetition(&lignes, quoi, &empreinte, &nom)?;
        let etape = if quoi == DSN && self.paie_changee_depuis_l_ecriture(mois, &chemin)?.is_some() {
            "paie_changee"
        } else if vrai(&r["conforme"]) {
            if est(&r["compte_rendu"], "ok") {
                "testee"
            } else {
                "en_test"
            }
        } else if vrai(&r["ratees"]) {
            "test_refuse"
        } else {
            "ecrite"
        };
        let (_, courte, phrase) = ETAPES_DEPOT.iter().find(|e| e.0 == etape).copied().expect("etape connue");
        // UN TEST REFUSE SE DIT AVEC SES ANOMALIES, la ou l'on propose d'envoyer (tickets #221, #222) : le code et le
        // message du bilan, pas seulement « refuse ».
        let phrase = match r["ratees"].as_array().and_then(|l| l.last()) {
            Some(l) if etape == "test_refuse" => format!("Le test a été refusé : {}. Le même refus attendrait le dépôt réel.", crate::depot_acte::pourquoi_rate(l).trim_end_matches('.')),
            _ => phrase.to_string(),
        };
        let avant = bilan.get("phrase").filter(|p| vrai(p)).map(valeur_en_texte).unwrap_or_default();
        let avant = avant.trim_end_matches('.');
        bilan.insert("etape".into(), json!(etape));
        bilan.insert("fichier".into(), premier["chemin"].clone());
        bilan.insert("courte".into(), json!(courte));
        bilan.insert("phrase".into(), json!(if avant.is_empty() { phrase.to_string() } else { format!("{}. {}", avant, phrase) }));
        if (etape == "test_refuse" || etape == "paie_changee") && !est(bilan.get("couleur").unwrap_or(&Value::Null), "refus") {
            bilan.insert("couleur".into(), json!("alerte"));
        }
        Ok(())
    }

    /// `bilan_du_mois` : le mois en un coup d'oeil, acte par acte, une phrase et
    /// une couleur. Aucun montant : le brut des paies n'est jamais lu.
    pub fn bilan_du_mois(&mut self, mois: &str) -> R<Value> {
        // UN MOIS TENU AVANT CE LOGICIEL (ticket #215) : ses etapes se sont faites ailleurs ; elles le disent, sans geste.
        if let Some(vue) = crate::mois_importe::vue(self.ctx, mois)? {
            let mut b = crate::mois_importe::etapes(&vue);
            let echeance = crate::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
            b[DSN]["echeance_depot"] = echeance.en_json();
            return Ok(b);
        }
        let paies = self.etat_par_engagement(mois, false)?;
        let mut out: Map<String, Value> = Map::new();

        // --- DPAE : par embauche -- celles du mois.
        let embauches: Vec<&Value> = paies.iter().filter(|p| vrai(&p["contrat"]["premier_mois"])).collect();
        let en_cours = paies.len() - embauches.len();
        let n = embauches.len();
        let d: Vec<&Value> = embauches.iter().map(|p| &p["etats"]["dpae"]).collect();
        let passees = d.iter().filter(|e| vrai(&e["passee"])).count();
        let refusees = d.iter().filter(|e| vrai(&e["refusee"])).count();
        let parties = d.iter().filter(|e| vrai(&e["fait"]) && !vrai(&e["passee"]) && !vrai(&e["refusee"])).count();
        let dpae = if n == 0 && en_cours > 0 {
            json!({"couleur": "bien", "courte": "rien ce mois-ci", "phrase": format!(
                "rien à déclarer ce mois-ci — {} contrat{} en cours depuis un mois précédent, la DPAE relève de son premier mois",
                en_cours, plur(en_cours, "", "s"))})
        } else if n == 0 {
            rien_ce_mois()
        } else if refusees > 0 {
            json!({"couleur": "refus", "phrase": format!("{} DPAE refusée{} — à corriger et à renvoyer", refusees, plur(refusees, "", "s"))})
        } else if passees == n {
            json!({"couleur": "bien", "phrase": format!("Toutes les DPAE sont faites ({}) — passées à l'Urssaf", n)})
        } else if passees + parties == n {
            json!({"couleur": "alerte", "attente": true, "courte": "parties — retour attendu", "phrase": format!(
                "Toutes les DPAE sont parties ({}) — {} retour{} attendu{}", n, parties, plur(parties, "", "s"), plur(parties, "", "s"))})
        } else if passees + parties > 0 {
            json!({"couleur": "alerte", "phrase": format!(
                "{} DPAE sur {} déclarée{} — il en reste {}", passees + parties, n, plur(passees + parties, "", "s"), n - passees - parties)})
        } else {
            json!({"couleur": "", "phrase": format!("aucune DPAE déclarée — {} à faire", n)})
        };
        let mut dpae = dpae.as_object().cloned().unwrap_or_default();
        // Une date d'embauche changee apres la limite : ce que la DPAE deposee en devient, dit (`dpae_d_une_date_changee`).
        let changees: Vec<String> = embauches.iter().filter_map(|p| p["etats"]["dpae"]["date_changee"]["phrase"].as_str().map(String::from)).collect();
        if !changees.is_empty() {
            let phrase = dpae.get("phrase").and_then(Value::as_str).unwrap_or("").to_string();
            dpae.insert("phrase".into(), json!(format!("{} — {}", phrase, changees.join(" "))));
        }
        if n > 0 && passees + parties < n {
            self.etape_de_depot(mois, DPAE, &mut dpae)?;
        }
        out.insert(DPAE.into(), Value::Object(dpae));

        // --- Contrats : envoyes, puis revenus signes -- ceux du mois. Un
        // permanent n'a pas de contrat a faire signer ici.
        let a_signer: Vec<&&Value> = embauches.iter().filter(|p| vrai(&p["spectacle"])).collect();
        let s: Vec<&Value> = a_signer.iter().map(|p| &p["etats"]["signature"]).collect();
        let ns = a_signer.len();
        let recus = s.iter().filter(|e| vrai(&e["recu"])).count();
        let envoyes = s.iter().filter(|e| vrai(&e["fait"])).count();
        let signature = if n > 0 && ns == 0 {
            let mut r = rien_ce_mois();
            r["phrase"] = json!("rien à signer ici : le contrat d'un salarié au mois se rédige à part");
            r
        } else if n == 0 && en_cours > 0 {
            json!({"couleur": "bien", "courte": "rien ce mois-ci", "phrase": format!(
                "rien à signer ce mois-ci — {} contrat{} en cours depuis un mois précédent, la signature relève de son premier mois",
                en_cours, plur(en_cours, "", "s"))})
        } else if n == 0 {
            rien_ce_mois()
        } else if s.iter().any(|e| vrai(&e["recu"]) && vrai(&e["caduc"])) {
            // UN CONTRAT SIGNE QUI NE DIT PLUS CE QUE DIT LA PAIE : il attend un avenant.
            let na = s.iter().filter(|e| vrai(&e["recu"]) && vrai(&e["caduc"])).count();
            json!({"couleur": "alerte", "courte": format!("avenant{} à faire", plur(na, "", "s")), "phrase": format!(
                "{} contrat{} signé{} ne di{} plus ce que dit la paie : à modifier par un avenant",
                na, plur(na, "", "s"), plur(na, "", "s"), if na > 1 { "sent" } else { "t" })})
        } else if recus == ns && s.iter().any(|e| vrai(&e["avenant_attendu"])) {
            // UN AVENANT PARTI, PAS REVENU : l'etape attend, sans bloquer la DSN.
            let mut qui: Vec<String> = Vec::new();
            for (p, e) in a_signer.iter().zip(&s) {
                if vrai(&e["avenant_attendu"]) {
                    let v = if vrai(&p["nom"]) { &p["nom"] } else { &p["salarie"] };
                    let t = valeur_en_texte(v);
                    if !qui.contains(&t) {
                        qui.push(t);
                    }
                }
            }
            let na = s.iter().filter(|e| vrai(&e["avenant_attendu"])).count();
            json!({"couleur": "alerte", "attente": true, "courte": format!("avenant{} à recevoir", plur(na, "", "s")), "phrase": format!(
                "Tous les contrats sont signés ({}) — {} avenant{} envoyé{}, pas encore revenu{} signé{} : {}",
                ns, na, plur(na, "", "s"), plur(na, "", "s"), plur(na, "", "s"), plur(na, "", "s"), qui.join(", "))})
        } else if recus == ns {
            json!({"couleur": "bien", "phrase": format!("Tous les contrats sont signés et reçus ({})", ns)})
        } else if envoyes == ns {
            json!({"couleur": "alerte", "attente": true, "courte": "envoyés — signatures attendues", "phrase": format!(
                "Tous les contrats sont envoyés ({}) — {} reçu{} signé{}, {} à recevoir", ns, recus, plur(recus, "", "s"), plur(recus, "", "s"), ns - recus)})
        } else if envoyes > 0 {
            json!({"couleur": "alerte", "phrase": format!(
                "{} contrat{} sur {} envoyé{} — {} reçu{} signé{}",
                envoyes, plur(envoyes, "", "s"), ns, plur(envoyes, "", "s"), recus, plur(recus, "", "s"), plur(recus, "", "s"))})
        } else {
            json!({"couleur": "", "phrase": format!("aucun contrat envoyé — {} à faire signer", ns)})
        };
        out.insert(SIGNATURE.into(), signature);

        // --- Documents : par salarie du mois.
        let salaries: BTreeSet<String> = paies.iter().map(|p| valeur_en_texte(&p["salarie"])).collect();
        let faits: BTreeSet<String> = paies.iter().filter(|p| vrai(&p["etats"]["documents"]["fait"])).map(|p| valeur_en_texte(&p["salarie"])).collect();
        // A RENVOYER (ticket #240) : ceux dont la paie a change depuis l'envoi de leurs documents.
        let mut a_renvoyer: Vec<String> = Vec::new();
        let mut noms: Vec<String> = Vec::new();
        for p in &paies {
            let cle = valeur_en_texte(&p["salarie"]);
            if vrai(&p["etats"]["documents"]["caduc"]) && !a_renvoyer.contains(&cle) {
                a_renvoyer.push(cle);
                noms.push(valeur_en_texte(&p["nom"]));
            }
        }
        let jamais = salaries.len() - faits.len();
        let documents = if salaries.is_empty() {
            rien_ce_mois()
        } else if !a_renvoyer.is_empty() {
            let n = a_renvoyer.len();
            let reste = if jamais > 0 { format!(" — et pas encore envoyés à {} salarié{}", jamais, plur(jamais, "", "s")) } else { String::new() };
            json!({"couleur": "alerte", "courte": format!("{} à renvoyer", n), "a_renvoyer": n,
                   "bouton": if jamais > 0 {
                       format!("Envoyer les documents ({} à envoyer, {} corrigé{} à renvoyer)…", jamais, n, plur(n, "", "s"))
                   } else {
                       format!("Renvoyer les documents corrigés ({})…", n)
                   },
                   "phrase": format!(
                       "Documents à renvoyer à {} salarié{} : la paie a changé depuis l'envoi ({}){}",
                       n, plur(n, "", "s"), noms.join(", "), reste)})
        } else if faits == salaries {
            json!({"couleur": "bien", "phrase": format!("Documents envoyés à tous les salariés du mois ({})", salaries.len())})
        } else if !faits.is_empty() {
            json!({"couleur": "alerte", "phrase": format!(
                "Documents envoyés à {} salarié{} sur {}", faits.len(), plur(faits.len(), "", "s"), salaries.len())})
        } else {
            json!({"couleur": "", "phrase": format!(
                "Documents pas encore envoyés — {} salarié{}", salaries.len(), plur(salaries.len(), "", "s"))})
        };
        // CE QUI PART CE MOIS-CI, NOMME PAR LE MOTEUR (#236) : les pieces des paies du mois (`pieces`), dans l'ordre de
        // l'envoi, et seulement celles qui existent pour ce mois.
        let mut documents = documents;
        let noms_pieces = [("bulletin", "bulletin"), ("certificat_cs", "certificat Congés Spectacles"), ("aer", "AER")];
        let presentes: Vec<&str> = noms_pieces
            .iter()
            .filter(|(c, _)| paies.iter().any(|p| p["pieces"].as_array().is_some_and(|a| a.iter().any(|x| x == c))))
            .map(|(_, n)| *n)
            .collect();
        if let Some(o) = documents.as_object_mut().filter(|_| !presentes.is_empty()) {
            o.insert("detail".into(), json!(presentes.join(", ")));
        }
        out.insert(DOCUMENTS.into(), documents);

        // --- DSN : le mois entier.
        let e = self.dsn_du_mois(mois)?;
        let mut b: Map<String, Value> = Map::new();
        if !vrai(&e) {
            b.insert("couleur".into(), json!(""));
            b.insert("phrase".into(), json!("DSN pas encore déposée"));
            // LE CHEMIN JUSQU'AU DEPOT se lit ici : ecrite, en test, testee.
            self.etape_de_depot(mois, DSN, &mut b)?;
        } else if vrai(get(&e, "refusee")?) {
            let anomalies: Vec<Value> = etats::items(ou(get(&e, "anomalies")?, &LISTE_VIDE))?.into_iter().map(|c| c.into_owned()).collect();
            let motif = joindre(" ; ", &anomalies)?;
            b.insert("couleur".into(), json!("refus"));
            b.insert("phrase".into(), json!(format!("DSN REFUSÉE — {}", if motif.is_empty() { "motif non lu".to_string() } else { motif })));
        } else if vrai(get(&e, "passee")?) {
            b.insert("couleur".into(), json!("bien"));
            // RIEN A DEPOSER (ticket #222) : la DSN est passee, l'etape n'a plus de bouton « Deposer la DSN… ». Un
            // annule-et-remplace a son propre chemin, plus bas (`remplacement` : « Refaire la DSN… », ou l'etape d'un
            // annule-et-remplace deja ecrit), qui rallume l'etape.
            b.insert("vide".into(), json!(true));
            b.insert("phrase".into(), json!(format!("DSN déposée le {} — passée", quand_lisible(get(&e, "quand")?))));
        } else {
            b.insert("couleur".into(), json!("alerte"));
            b.insert("attente".into(), json!(true));
            b.insert("courte".into(), json!("déposée — retour attendu"));
            b.insert("phrase".into(), json!(format!("DSN déposée le {} — retour attendu", quand_lisible(get(&e, "quand")?))));
        }
        // L'ANNULE-ET-REMPLACE (30/09/2026) : une paie modifiee apres sa DSN
        // reelle rouvre l'etape, et un annule-et-remplace ecrit montre ou il
        // en est, comme une premiere DSN.
        if vrai(&e) && !vrai(get(&e, "refusee")?) {
            let r = self.etat_remplacement(mois, &Value::Null)?;
            b.insert("remplacement".into(), r.clone());
            if vrai(&r["saisie_modifiee"]) && vrai(&r["correction_ecrite"]) {
                // Modifiee apres la limite, et sa correction est ecrite : rien a faire ici, la DSN suivante l'embarque.
                b.insert("phrase".into(), r["phrase"].clone());
                b.insert("courte".into(), json!("corrigée — part dans la DSN suivante"));
            } else if vrai(&r["saisie_modifiee"]) {
                b.insert("couleur".into(), json!("alerte"));
                b.insert("attente".into(), json!(false));
                b.insert("phrase".into(), r["phrase"].clone());
                b.insert(
                    "courte".into(),
                    json!(if vrai(&r["a_envoyer"]) { "annule-et-remplace à envoyer" } else { "modifiée — à corriger le mois suivant" }),
                );
            }
            // Seul un annule-et-remplace au-dessus du dernier depot reel a un
            // chemin a montrer, et jamais hors delai (§1.4.2).
            let ecrits = self.fichiers_deposables(mois)?;
            let premier = ecrits.get(DSN).and_then(Value::as_array).and_then(|l| l.first()).cloned();
            let apercu = premier.and_then(|f| std::fs::read(valeur_en_texte(&f["chemin"])).ok()).map(|o| crate::depot::lire(&o));
            if let Some(a) = apercu {
                if vrai(&r["admis"]) && ["03", "05"].contains(&a.champ("type_declaration")) {
                    let ordre = a.champ("ordre");
                    let ordre: i64 = if ordre.is_empty() {
                        0
                    } else {
                        ordre.trim().parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(ordre))))?
                    };
                    if ordre > r["dernier_ordre"].as_i64().unwrap_or(0) {
                        self.etape_de_depot(mois, DSN, &mut b)?;
                    }
                }
            }
            if vrai(&r["a_envoyer"]) || (vrai(&r["saisie_modifiee"]) && !vrai(&r["admis"]) && !vrai(&r["correction_ecrite"])) {
                b.remove("vide");
            }
            if vrai(b.get("etape").unwrap_or(&Value::Null)) {
                b.remove("vide");
                b.insert("attente".into(), json!(false));
                if est(b.get("couleur").unwrap_or(&Value::Null), "bien") {
                    b.insert("couleur".into(), json!("alerte"));
                }
            }
        }
        // L'echeance de depot du mois (`echeance_dsn` : le 5 ou le 15, reporte au jour ouvre, a midi).
        let echeance = crate::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        b.insert("echeance_depot".into(), echeance.en_json());
        out.insert(DSN.into(), Value::Object(b));
        Ok(Value::Object(out))
    }

    // ------------------------------------------------------- les autres routes

    /// `exercice_clos` : la cloture de l'annee de ce mois, ou `None`.
    pub(crate) fn exercice_clos(&self, mois_ou_annee: &str) -> R<bool> {
        let p = self.racine().join("config").join("exercices.json");
        if !socle::fichiers::existe(&p) {
            return Ok(false);
        }
        // LU UNE FOIS TANT QU'IL NE CHANGE PAS (ticket #244) : `etat` le demandait pour chaque mois. Le refus d'un fichier
        // illisible ne change pas (`OSError`, `JSONDecodeError`), et ne se garde pas.
        let v = socle::fichiers::json_en_cache_ou(&p, |p| {
            let t = socle::fichiers::lire_texte(p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
            serde_json::from_str::<Value>(&t).map_err(|e| erreur("JSONDecodeError", e.to_string()))
        })?;
        let Value::Object(o) = &*v else {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&v))));
        };
        let annee: String = mois_ou_annee.chars().take(4).collect();
        Ok(o.iter().filter(|(k, _)| !k.starts_with('_')).find(|(k, _)| **k == annee).is_some_and(|(_, x)| vrai(x)))
    }

    /// `mois_a_ouvrir` : le plus ancien mois passe dont une etape n'est pas
    /// verte, sinon le mois du calendrier.
    pub fn mois_a_ouvrir(&mut self) -> R<Value> {
        let courant = self.ctx.aujourdhui.mois_iso();
        for m in mois_saisis(self.racine()) {
            if m >= courant || self.exercice_clos(&m)? {
                continue;
            }
            let reste = self.etapes_restantes(&m)?;
            if !reste.is_empty() {
                return Ok(json!({"mois": m, "courant": courant, "reste": reste}));
            }
        }
        Ok(json!({"mois": courant, "courant": courant, "reste": []}))
    }

    /// LES ETAPES QUI RESTENT A UN MOIS PASSE, d'apres le journal et les empreintes des engagements seuls : `mois_a_ouvrir`
    /// parcourt tous les mois passes, il ne refait pas l'etat complet de chacun (`bilan_du_mois`, qui calcule et compare).
    /// Une etape reste quand ce qu'elle fait partir n'est pas parti, ou ne dit plus ce que dit l'engagement : la DPAE de
    /// chaque embauche du mois passee ; le contrat de chaque embauche sur un spectacle revenu signe, toujours le bon (son
    /// empreinte, son numero) et sans avenant en attente ; les documents de chaque salarie du mois envoyes (un engagement
    /// encore en saisie n'en a pas) et toujours les bons (`etat_documents`) ; la DSN du mois passee, et sans
    /// annule-et-remplace a faire (`etat_remplacement`). Un mois tenu avant ce logiciel dit les siennes (`mois_importe`).
    fn etapes_restantes(&mut self, mois: &str) -> R<Vec<String>> {
        if let Some(vue) = crate::mois_importe::vue(self.ctx, mois)? {
            let b = crate::mois_importe::etapes(&vue);
            return Ok(b.as_object().into_iter().flatten().filter(|(_, v)| !est(&v["couleur"], "bien")).map(|(k, _)| k.clone()).collect());
        }
        let vue = engagement::stock::vue_du_mois(self.racine(), mois).map_err(plain)?.unwrap_or(Value::Null);
        let engs: Vec<Value> = vue.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
        let lignes = self.lignes()?;
        let g = self.globales()?;
        let en_saisie: Vec<bool> = engagement::etats::valides_du_mois(self.ctx, mois).map_err(plain)?.into_iter().map(|v| !v).collect();
        let contrats: Vec<Cle3> = engs
            .iter()
            .map(|e| {
                let cle = engagement::cle_de_l_engagement(self.ctx, e).unwrap_or_default();
                (cle, engagement::debut_brut(e), e.get("numero").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_default())
            })
            .collect();
        let cles = etats::cles_des_paies(&lignes, &contrats)?;
        let docs = etats::documents_partis(&lignes, mois)?;
        // Les empreintes d'aujourd'hui, comme `etat_par_engagement` les compare a ce que le journal a note.
        let s = SaisieMois { data: vue.clone(), paies: Vec::new() };
        let uniques: Vec<Paire> = contrats.iter().enumerate().map(|(i, (c, d, _))| (c.clone(), format!("{}#{}", d, i))).collect();
        let actuelles = Self::empreintes_engagements(&s, &uniques);
        let saisis = Self::numeros_saisis(&s, &uniques);
        let salaries: Vec<String> = contrats.iter().map(|c| c.0.clone()).collect();
        let empreintes_docs = Self::empreintes_documents(&engs, &salaries);
        let nul = Value::Null;
        let (mut dpae, mut signature, mut documents) = (true, true, true);
        for (i, e) in engs.iter().enumerate() {
            let (cle, debut, _) = &contrats[i];
            if debut.starts_with(mois) {
                dpae &= g.dpae.get(&(cle.clone(), debut.clone())).is_some_and(|v| vrai(&v["passee"]));
                if vrai(e.get("spectacle").unwrap_or(&Value::Null)) {
                    let sig = etat_signature(
                        premier(&g.contrats, &cles[i]).unwrap_or(&nul),
                        premier(&g.recus, &cles[i]).unwrap_or(&nul),
                        premier(&g.recus_par, &cles[i]).unwrap_or(&nul),
                        premier(&g.partis, &cles[i]).unwrap_or(&nul),
                        actuelles.get(&uniques[i]).map(String::as_str),
                        mois,
                        dans_l_ensemble(&g.renvoyes, &cles[i]),
                        premier(&g.numeros_partis, &cles[i]).unwrap_or(&nul),
                        saisis.get(&uniques[i]).unwrap_or(&nul),
                        premier(&g.avenants, &cles[i]).unwrap_or(&nul),
                        "",
                    );
                    signature &= vrai(&sig["recu"]) && !vrai(&sig["caduc"]) && !vrai(&sig["avenant_attendu"]);
                }
            }
            let caducs = vrai(&etat_documents(docs.get(cle), empreintes_docs.get(cle).map(String::as_str))["caduc"]);
            documents &= !en_saisie.get(i).copied().unwrap_or(true) && docs.contains_key(cle) && !caducs;
        }
        let d = self.dsn_du_mois(mois)?;
        let mut dsn = vrai(&d) && vrai(get(&d, "passee")?) && !vrai(get(&d, "refusee")?);
        if dsn {
            // L'ANNULE-ET-REMPLACE : la saisie a change depuis la DSN reelle (l'empreinte notee a son depot). Seul un mois
            // modifie, ou dont le depot n'a pas note d'empreinte, se regarde en entier.
            let notee = self.depots_reels(mois, &nul)?.last().and_then(|l| l.pointer("/detail/empreinte_saisie").cloned()).filter(vrai);
            let maintenant = self.empreinte_saisie(mois)?;
            if notee.is_none() || maintenant.map_or(true, |m| notee.as_ref().is_some_and(|n| !est(n, &m))) {
                let r = self.etat_remplacement(mois, &nul)?;
                dsn = !(vrai(&r["saisie_modifiee"]) && !vrai(&r["correction_ecrite"]));
            }
        }
        Ok([(DPAE, dpae), (SIGNATURE, signature), (DOCUMENTS, documents), (DSN, dsn)].into_iter().filter(|(_, fait)| !fait).map(|(k, _)| k.to_string()).collect())
    }

    /// `embauches_a_declarer` : les contrats dont la DPAE n'est pas encore partie.
    pub fn embauches_a_declarer(&mut self, mois: Option<&str>) -> R<Value> {
        // UNE DERIVATION POUR TOUS LES MOIS (ticket #244) : le bilan de chaque mois charge sa saisie, et les conges d'un
        // permanent y relisent les paies des mois d'avant (`conges::paies_du_mois_memorisees`) ; sans elle, chaque mois les
        // recalculait toutes. Rien ne s'ecrit entre deux mois de cette lecture : le memo vaut pour toute la route.
        let _derivation = engagement::conges::Derivation::ouvrir();
        let lignes = self.lignes()?;
        let faites = cles_en_ensemble(etats::embauches_par_depot(&lignes)?.cles());
        let mois: Vec<String> = match mois.filter(|m| !m.is_empty()) {
            Some(m) => vec![m.to_string()],
            None => {
                let mut v = mois_saisis(self.racine());
                v.reverse();
                v
            }
        };
        let mut attente = Vec::new();
        for m in mois {
            if !engagement::stock::a_une_saisie(self.racine(), &m).unwrap_or(false) {
                continue;
            }
            let Ok(s) = self.charger_saisie(&m) else { continue };
            for p in &s.paies {
                let cle = texte_de(requis(&p.salarie, "_cle")?);
                let debut = p.engagement.debut.iso();
                if faites.contains(&(cle.clone(), debut.clone())) {
                    continue;
                }
                // Un contrat en cours se declare avec son premier mois.
                if !p.engagement.premier_mois().map_err(plain)? {
                    continue;
                }
                // UNE DATE D'EMBAUCHE CHANGEE apres la limite (21/10/2026) : reculee, la DPAE de l'ancienne date vaut ;
                // avancee, une DPAE est a deposer, et la phrase dit pourquoi (`dpae_d_une_date_changee`).
                let ancienne = p.engagement.numero.as_deref().and_then(|n| crate::routes_regularisation::debut_declare_avant(self.ctx, &cle, n, &debut));
                let pourquoi = match ancienne.filter(|a| faites.contains(&(cle.clone(), a.clone()))) {
                    Some(a) => {
                        let (vaut, phrase) = crate::routes_regularisation::dpae_d_une_date_changee(&a, &debut);
                        if vaut {
                            continue;
                        }
                        json!(phrase)
                    }
                    None => Value::Null,
                };
                let prenom = match p.salarie.get("prenom_affichage").filter(|v| vrai(v)) {
                    Some(v) => texte_de(v),
                    None => texte_de(requis(&p.salarie, "prenom")?),
                };
                let titre = if vrai(&p.spectacle) { p.spectacle.get("titre").filter(|t| vrai(t)).cloned() } else { None };
                let mut due = json!({
                    "mois": m, "salarie": cle,
                    "nom": format!("{} {}", texte_de(requis(&p.salarie, "nom")?), prenom),
                    "spectacle": titre.unwrap_or_else(|| json!("salarié au mois")),
                    "debut": debut, "fin": p.engagement.fin.map(|f| f.iso()),
                });
                // Seulement quand une date d'embauche a change : le reste du temps, rien a dire.
                if !pourquoi.is_null() {
                    due["pourquoi"] = pourquoi;
                }
                attente.push(due);
            }
        }
        Ok(Value::Array(attente))
    }

    /// `etat-du-mois` : tout ce que la page du mois affiche.
    fn etat_du_mois(&mut self, mois: &str) -> R<Value> {
        let lignes = self.lignes()?;
        let actes = etats::etat_du_mois(&lignes, mois)?;
        let recus = etats::contrats_recus(&lignes, etats::Maille::Contrat)?;
        let mut cles_recus: Dico<()> = Dico::nouveau();
        for (k, _) in &recus.items {
            cles_recus.poser(k.clone(), ())?;
        }
        let mut resumes = Map::new();
        for (k, v) in actes.as_object().unwrap() {
            resumes.insert(k.clone(), json!(etats::resume(v, &cles_recus, etats::Maille::Contrat)?));
        }
        let fichiers = self.fichiers_deposables(mois)?;
        let bilans = self.bilan_du_mois(mois)?;
        let lignes_du_mois: Vec<Value> = etats::filtrer(&lignes, None, Some(mois), false)?.into_iter().cloned().collect();
        let pose = crate::coffre::Coffre::du_poste(self.racine(), "")
            .ouverture_exigee()
            .map_err(|x| Echec { classe: Some(x.classe), message: x.message })?;
        Ok(json!({
            "mois": mois, "actes": actes, "resumes": resumes, "fichiers": fichiers,
            "bilans": bilans, "lignes": lignes_du_mois, "mot_de_passe_pose": pose,
        }))
    }
}

/// La phrase de la pastille Documents d'un salarie dont la paie a change depuis l'envoi.
pub(crate) const DOCUMENTS_CADUCS: &str = "la paie a changé depuis l'envoi";

/// LA PASTILLE DOCUMENTS d'un salarie (ticket #240). `parti` : ce que le journal garde de son dernier envoi
/// (`etats::documents_partis`) ; `actuelle` : l'empreinte de sa paie aujourd'hui (`empreintes_documents`).
/// `caduc` : ses documents sont partis, et sa paie a change depuis -- ils ne sont plus les bons, ils sont a renvoyer.
pub(crate) fn etat_documents(parti: Option<&Value>, actuelle: Option<&str>) -> Value {
    let nul = Value::Null;
    let parti = parti.unwrap_or(&nul);
    let fait = parti.get("quand").cloned().unwrap_or(Value::Null);
    let notee = parti.get("empreinte").filter(|e| vrai(e));
    let caduc = vrai(&fait) && notee.is_some_and(|n| actuelle.is_some_and(|a| !a.is_empty() && !est(n, a)));
    json!({"fait": fait, "maille": "salarie", "caduc": caduc, "pourquoi": if caduc { DOCUMENTS_CADUCS } else { "" }})
}

/// `S20\.G00\.05\.004,'(\d+)'` cherche dans les octets d'un fichier DSN.
fn rang_dans_le_fichier(octets: &[u8]) -> Option<i64> {
    let motif = b"S20.G00.05.004,'";
    let mut i = 0;
    while i + motif.len() <= octets.len() {
        if &octets[i..i + motif.len()] == motif {
            let debut = i + motif.len();
            let fin = debut + octets[debut..].iter().take_while(|c| c.is_ascii_digit()).count();
            if fin > debut && octets.get(fin) == Some(&b'\'') {
                return std::str::from_utf8(&octets[debut..fin]).ok()?.parse().ok();
            }
        }
        i += 1;
    }
    None
}

/// `DSN_[^\W\d_]+\d{4}(_AR\d+)?\.dsn` : le nom d'une DSN mensuelle.
pub(crate) fn est_une_dsn_mensuelle(nom: &str) -> bool {
    let Some(reste) = nom.strip_prefix("DSN_") else { return false };
    let Some(reste) = reste.strip_suffix(".dsn") else { return false };
    let c: Vec<char> = reste.chars().collect();
    let lettres = c.iter().take_while(|x| etats::alnum_py(**x) && !x.is_ascii_digit() && **x != '_' && !x.is_numeric()).count();
    if lettres == 0 {
        return false;
    }
    let suite = &c[lettres..];
    if suite.len() < 4 || !suite[..4].iter().all(char::is_ascii_digit) {
        return false;
    }
    let fin = &suite[4..];
    fin.is_empty() || (fin.len() > 3 && fin[..3] == ['_', 'A', 'R'] && fin[3..].iter().all(char::is_ascii_digit))
}

/// `_etat_signature` : la pastille Contrat d'une paie.
///
/// `caduc` : la paie a change depuis que son contrat est parti (dates, heures,
/// cachets, remuneration) ; le contrat envoye -- signe ou non -- n'est plus le
/// bon, il faut le renvoyer au salarie. Un contrat renvoye attend une NOUVELLE
/// signature : celle de l'ancien ne compte plus.
#[allow(clippy::too_many_arguments)]
pub fn etat_signature(
    fait: &Value, recu: &Value, recu_par: &Value, parti: &Value, actuelle: Option<&str>, mois: &str, renvoye: bool,
    numero_parti: &Value, numero_saisi: &Value, avenants: &Value, ecart: &str,
) -> Value {
    let (recu, recu_par) = if renvoye { (&Value::Null, &Value::Null) } else { (recu, recu_par) };
    let nul = Value::Null;
    let (mois_parti, empreinte) = match parti.as_array().filter(|_| vrai(parti)).map(Vec::as_slice) {
        Some([m, e]) => (m, e),
        _ => (&nul, &nul),
    };
    let mut pourquoi: Vec<String> = Vec::new();
    if vrai(fait) && vrai(empreinte) && actuelle.is_some_and(|a| !a.is_empty()) && est(mois_parti, mois) && !est(empreinte, actuelle.unwrap_or("")) {
        pourquoi.push("la paie a changé depuis son envoi".into());
    }
    // LE NUMERO (30/09/2026) : le contrat signe porte le sien, la DSN declarera
    // celui de la saisie. S'ils different, ce n'est pas celui qui est declare.
    if vrai(fait) && vrai(numero_parti) && vrai(numero_saisi) && !socle::valeur::valeurs_egales(numero_parti, numero_saisi) {
        pourquoi.push(format!("il porte le n° {}, la paie déclare le n° {}", valeur_en_texte(numero_parti), valeur_en_texte(numero_saisi)));
    }
    // CE QUE LE CONTRAT PARTI NE DIT PLUS, lu dans son PDF (30/09/2026).
    if vrai(fait) && !ecart.is_empty() && pourquoi.is_empty() {
        pourquoi.push(format!("le contrat parti ne dit plus ce que dit la paie : {}", ecart));
    }
    // L'AVENANT : le dernier parti. Pas encore revenu, il laisse la pastille orange.
    let dernier = avenants.as_array().and_then(|a| a.last()).cloned().unwrap_or(Value::Null);
    let attendu = vrai(&dernier) && !vrai(dernier.get("signe").unwrap_or(&nul));
    json!({"fait": fait, "recu": recu, "recu_par": recu_par,
           "caduc": !pourquoi.is_empty(), "pourquoi": pourquoi.join(" ; "),
           "numero_parti": numero_parti, "maille": "paie",
           "avenant": dernier,
           "avenant_attendu": attendu})
}

// ----------------------------------------------------------- l'aiguillage

pub(crate) fn mois_de(charge: &Value) -> R<(Value, String)> {
    let v = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    Ok((v.clone(), valeur_en_texte(v)))
}

/// Une route de ce module. `charge` : ce que la page envoie (`mois`).
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let mut m = Moteur::nouveau(o, ctx);
    match nom {
        "etat-par-engagement" => {
            let (brut, mois) = mois_de(charge)?;
            Ok(json!({"mois": brut, "engagements": m.etat_par_engagement(&mois, true)?}))
        }
        "etat-du-mois" => {
            let (_, mois) = mois_de(charge)?;
            m.etat_du_mois(&mois)
        }
        "bilan-du-mois" => m.bilan_du_mois(&mois_de(charge)?.1),
        "fichiers-deposables" => m.fichiers_deposables(&mois_de(charge)?.1),
        "embauches-a-declarer" => {
            let mois = charge.get("mois").filter(|v| vrai(v)).map(valeur_en_texte);
            Ok(json!({"embauches": m.embauches_a_declarer(mois.as_deref())?}))
        }
        "mois-a-ouvrir" => m.mois_a_ouvrir(),
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_nom_d_une_dsn_mensuelle() {
        for ok in ["DSN_Aout2026.dsn", "DSN_Septembre2026_AR2.dsn", "DSN_Fevrier2026_AR12.dsn", "DSN_Décembre2026.dsn"] {
            assert!(est_une_dsn_mensuelle(ok), "{}", ok);
        }
        for non in [
            "DSN_2026.dsn", "DSN_Aout26.dsn", "DSN_Aout2026.DSN", "DSN_Aout2026_AR.dsn", "DSN_Aout2026_AR2x.dsn", "DSN_Aout2026bis.dsn",
            "dsn_Aout2026.dsn", "DSN_Signalement_Aout2026.dsn", "DSN_Aout2026_AR2.dsn.zip",
        ] {
            assert!(!est_une_dsn_mensuelle(non), "{}", non);
        }
    }

    #[test]
    fn le_rang_d_un_fichier_dsn_se_lit_dans_la_rubrique_s20_g00_05_004() {
        assert_eq!(rang_dans_le_fichier(b"S20.G00.05.002,'03'\nS20.G00.05.004,'12'\n"), Some(12));
        // Un premier motif sans chiffres n'arrete pas la recherche.
        assert_eq!(rang_dans_le_fichier(b"S20.G00.05.004,'x'\nS20.G00.05.004,'3'\n"), Some(3));
        assert_eq!(rang_dans_le_fichier(b"S20.G00.05.004,''\n"), None);
        assert_eq!(rang_dans_le_fichier(b"rien"), None);
    }

    #[test]
    fn un_montant_a_deux_decimales_comme_decimal_quantize() {
        for (entree, attendu) in [("620", "620.00"), ("620.0", "620.00"), ("1234.5", "1234.50"), ("0", "0.00"), ("-3.2", "-3.20"), ("2.675", "2.68"), ("2.665", "2.66")] {
            assert_eq!(brut_en_texte(&json!(entree)).unwrap(), attendu, "{}", entree);
        }
    }

    #[test]
    fn les_noms_de_fichiers_se_decoupent() {
        assert_eq!(suffixe("DSN_Aout2026.DSN"), ".DSN");
        assert_eq!(suffixe(".dsn"), "");
        assert_eq!(suffixe("a."), "");
        assert_eq!(radical("DPAE_x_recapitulatif.txt"), "DPAE_x_recapitulatif");
    }

    #[test]
    fn la_pastille_contrat_dit_pourquoi_le_contrat_parti_n_est_plus_le_bon() {
        let nul = Value::Null;
        let parti = json!(["2026-08", "aaaa"]);
        let envoye = json!("2026-08-20T10:00:00");
        // Rien n'est parti : rien a dire.
        let s = etat_signature(&nul, &nul, &nul, &nul, Some("bbbb"), "2026-08", false, &nul, &nul, &nul, "");
        assert_eq!(s["caduc"], false);
        assert_eq!(s["pourquoi"], "");
        assert_eq!(s["avenant_attendu"], false);
        // L'empreinte de la paie a change depuis l'envoi, dans le mois ou le contrat est parti.
        let s = etat_signature(&envoye, &envoye, &nul, &parti, Some("bbbb"), "2026-08", false, &nul, &nul, &nul, "");
        assert_eq!((s["caduc"].clone(), s["pourquoi"].clone()), (json!(true), json!("la paie a changé depuis son envoi")));
        // Un autre mois : la paie d'un autre mois, que le contrat ne decrit pas.
        let s = etat_signature(&envoye, &nul, &nul, &parti, Some("bbbb"), "2026-09", false, &nul, &nul, &nul, "");
        assert_eq!(s["caduc"], false);
        // Le numero, puis l'ecart lu dans le PDF (seulement si rien d'autre n'est dit).
        let s = etat_signature(&envoye, &nul, &nul, &nul, None, "2026-08", false, &json!("AB.26.01"), &json!("AB.26.02"), &nul, "article 4");
        assert_eq!(s["pourquoi"], "il porte le n° AB.26.01, la paie déclare le n° AB.26.02");
        let s = etat_signature(&envoye, &nul, &nul, &nul, None, "2026-08", false, &nul, &nul, &nul, "article 4 (Rémunération)");
        assert_eq!(s["pourquoi"], "le contrat parti ne dit plus ce que dit la paie : article 4 (Rémunération)");
        // Un contrat renvoye attend une nouvelle signature : l'ancienne ne compte plus.
        let s = etat_signature(&envoye, &envoye, &json!("courriel"), &nul, None, "2026-08", true, &nul, &nul, &nul, "");
        assert_eq!((s["recu"].clone(), s["recu_par"].clone()), (Value::Null, Value::Null));
        // Le dernier avenant parti, pas encore signe, laisse la pastille orange.
        let avenants = json!([{"numero": 1, "signe": "2026-08-25T10:00:00"}, {"numero": 2, "signe": null}]);
        let s = etat_signature(&envoye, &envoye, &nul, &nul, None, "2026-08", false, &nul, &nul, &avenants, "");
        assert_eq!(s["avenant"]["numero"], 2);
        assert_eq!(s["avenant_attendu"], true);
    }

    #[test]
    fn un_refus_de_saisie_en_cours_garde_sa_classe() {
        let e = classer_saisie("L'engagement 2 de 2026-08 ne dit pas quel spectacle il joue, et le mois n'en propose aucun.".into());
        assert_eq!(e.classe, Some("SaisieIncomplete"));
        let e = classer_saisie("L'engagement 1 de 2026-08 ne dit aucun jour joue. Les jours se portent sur chaque ligne".into());
        assert_eq!(e.classe, Some("SaisieIncomplete"));
        let e = classer_saisie("L'engagement 1 de 2026-08 : sa ligne 1 (« Service de répétition ») ne dit pas ses heures.".into());
        assert_eq!(e.classe, Some("SaisieIncomplete"));
        let e = classer_saisie("L'engagement 1 de 2026-08 est ecrit a l'ancienne".into());
        assert_eq!(e.classe, None);
    }
}
