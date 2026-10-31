//! Les routes du mois : `etat-par-engagement`, `etat-du-mois`, `mois-a-ouvrir`, et les lectures
//! qu'elles partagent (`bilan_du_mois`, `fichiers_deposables`) : l'etat de chaque engagement (sa validation, sa DPAE,
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
//! la route le demande (`etat-du-mois` et `mois-a-ouvrir` ne le demandent jamais).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use engagement;
use engagement::etats::ARefaire;
use envois::etats::{self, est, get, getd, objet_vide, ou, quand_lisible, texte_vide, DOCUMENTS, DPAE, DSN, LISTE_VIDE, SIGNATURE};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{sha256_hex, vrai};
use socle::num::Decimal;
use socle::echec::Echec;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["etat-par-engagement", "etat-du-mois", "mois-a-ouvrir"];

pub(crate) type Paire = (String, String);

// ------------------------------------------------------------ les groupes

/// Les quatre etapes du mois, dans l'ordre de l'habitude (aucun texte n'impose d'ordre).
const ORDRE: [(&str, &str); 4] = [(DPAE, "DPAE"), (SIGNATURE, "Contrats"), (DSN, "DSN"), (DOCUMENTS, "Documents")];

/// Un groupe d'une etape : son etat (`fait`, `a_faire`, `en_attente`, `refuse`), son compte, ce qu'une case en ecrit
/// (`compte_texte` en gras : « 2 », « 1/2 », « Lyon », ou rien ; `quoi` : « à déposer », « reçu »), les deux joints
/// (`phrase`), son echeance et le geste qu'ouvre un clic.
fn groupe(etat: &str, compte: Value, compte_texte: String, quoi: String, echeance: Value, action: Value) -> Value {
    let phrase = if compte_texte.is_empty() { quoi.clone() } else { format!("{} {}", compte_texte, quoi) };
    json!({"etat": etat, "compte": compte, "compte_texte": compte_texte, "quoi": quoi, "phrase": phrase, "echeance": echeance, "action": action})
}

/// Ce qui est fait : le nom de l'etape et la coche, rien d'autre.
fn fait() -> Value {
    json!({"etat": "fait", "compte": null, "compte_texte": "", "quoi": "", "phrase": null, "echeance": null, "action": null})
}

/// L'echeance d'un depot de DSN : `mots` le `jour` a `heure`, d'apres `source`.
fn echeance_dsn(jour: Jour, heure: &'static str, source: &str) -> Value {
    droit::echeances::Echeance { jour, mots: "avant le", heure, source: source.into() }.en_json()
}

/// Le geste d'un clic : ce que la page ouvre (`geste`) et ce que dit le bouton (`libelle`).
fn action(geste: &str, libelle: String) -> Value {
    json!({"geste": geste, "libelle": libelle})
}

/// Le geste d'un carre Contrat en attente : noter revenu signe le contrat, ou l'avenant `avenant` (son numero) ;
/// la route `contrat-recu` rend la phrase qui le dit et de quoi l'annuler.
fn noter_recu(libelle: &str, avenant: Value) -> Value {
    json!({"geste": "contrat_recu", "libelle": libelle, "recu": true, "avenant": avenant})
}

/// Un mois sans aucun engagement : l'etape n'a rien a faire, elle n'est pas faite pour autant. Elle se dit en gris.
fn rien_ce_mois() -> Value {
    json!({"etat": "rien", "compte": null, "compte_texte": "", "quoi": "rien ce mois-ci", "phrase": "rien ce mois-ci", "echeance": null, "action": null})
}

/// `n` engagements auxquels il manque un fait (`A_COMPLETER`) : ce qui reste a faire, en orange, sans geste ; la carte
/// de chacun dit ce qui lui manque.
fn a_completer(n: usize) -> Value {
    groupe("a_faire", json!(n), n.to_string(), "à compléter".into(), Value::Null, Value::Null)
}

/// Marque `suivante` la premiere etape qui a quelque chose a faire de notre cote (a faire, ou refuse), et donne a chaque
/// groupe sa couleur (`couleur_de_l_etat`).
fn suivante(mut etapes: Vec<Value>) -> Vec<Value> {
    let i = etapes.iter().position(|e| e["groupes"].as_array().is_some_and(|g| g.iter().any(|x| est(&x["etat"], "a_faire") || est(&x["etat"], "refuse"))));
    for (j, e) in etapes.iter_mut().enumerate() {
        e["suivante"] = json!(Some(j) == i);
        if let Some(gs) = e["groupes"].as_array_mut() {
            for g in gs {
                g["couleur"] = json!(super::couleur_de_l_etat(&valeur_en_texte(&g["etat"])));
                // Une attente se dessine coupee : la moitie faite (partie), la moitie qui attend (le retour).
                g["coupee"] = json!(est(&g["etat"], "en_attente"));
            }
        }
    }
    etapes
}

// ------------------------------------------------------------ les colonnes
//
// Les cases des quatre lignes s'alignent sur les memes colonnes. G, la grande, porte ce qui est le plus avance ; une
// colonne de cote s'ouvre par vague de ce qui est en cours (des embauches nouvelles, un contrat qui change, un
// etablissement dont la DSN diverge), et une vague qui en est au meme point que G y rentre : on ne separe que le temps
// d'un changement. Chaque ligne fond les colonnes voisines qui en sont au meme point en une seule case.

/// Ou en est un engagement, ligne par ligne. Une ligne qui ne le concerne pas (la DPAE d'un contrat qui a commence un
/// mois d'avant, la signature d'un salarie au mois) est faite pour lui. `dsn` : la classe de son etablissement
/// (`groupes_dsn`).
/// Le point d'un engagement auquel il manque un fait, sur chacune de ses lignes.
const A_COMPLETER: &str = "a_completer";

#[derive(Clone, Copy)]
struct Point {
    dpae: &'static str,
    contrat: &'static str,
    documents: &'static str,
    dsn: usize,
}

impl Point {
    fn de(p: &Value, dsn: usize) -> Point {
        let embauche = vrai(&p["contrat"]["premier_mois"]);
        // UN ENGAGEMENT A COMPLETER, jamais valide (`ligne_a_completer`) : rien n'est parti de lui, et tout ce qui partira
        // attend qu'il soit complet. Il reste a faire, en orange, pour qu'on ne l'oublie pas.
        if vrai(&p["non_calcule"]) {
            let spectacle = vrai(&p["spectacle"]);
            return Point {
                dpae: if embauche { A_COMPLETER } else { "fait" },
                contrat: if embauche && spectacle { A_COMPLETER } else { "fait" },
                documents: A_COMPLETER,
                dsn,
            };
        }
        let d = &p["etats"]["dpae"];
        let dpae = if !embauche {
            "fait"
        } else if vrai(&d["refusee"]) {
            "refuse"
        } else if vrai(&d["fait"]) {
            "fait"
        } else {
            "a_faire"
        };
        let s = &p["etats"]["signature"];
        // Le contrat se fait signer au mois de l'embauche ; un avenant, a chaque mois du contrat : tant qu'il n'est pas
        // revenu signe, chacun de ses mois le dit.
        let ecrit = vrai(&p["contrat"]["ecrit"]);
        let contrat = if ecrit && !embauche && vrai(&s["caduc"]) {
            "a_refaire"
        } else if ecrit && !embauche && vrai(&s["avenant_attendu"]) {
            "en_attente"
        } else if !(embauche && ecrit) {
            "fait"
        } else if !vrai(&s["fait"]) {
            "a_envoyer"
        } else if vrai(&s["caduc"]) {
            "a_refaire"
        } else if !vrai(&s["recu"]) || vrai(&s["avenant_attendu"]) {
            "en_attente"
        } else {
            "fait"
        };
        let o = &p["etats"]["documents"];
        let documents = if vrai(&o["caduc"]) {
            "a_renvoyer"
        } else if vrai(&o["fait"]) {
            "fait"
        } else {
            "a_envoyer"
        };
        Point { dpae, contrat, documents, dsn }
    }

    /// Sa vague : ce qui reste a faire de notre cote. Une DPAE partie l'est, revenue refusee ou non ; un contrat parti
    /// l'est, signe ou non.
    fn vague(&self) -> (bool, &'static str, &'static str, usize) {
        let contrat = if matches!(self.contrat, "a_envoyer" | "a_refaire") { self.contrat } else { "parti" };
        (self.dpae == "a_faire", contrat, self.documents, self.dsn)
    }

    /// Combien de ses lignes sont faites de notre cote : l'ordre des colonnes, la plus avancee en G.
    fn avance(&self, dsn_fait: &[bool]) -> usize {
        [!matches!(self.dpae, "a_faire" | A_COMPLETER), matches!(self.contrat, "fait" | "en_attente"), self.documents == "fait", dsn_fait.get(self.dsn).copied().unwrap_or(false)]
            .iter()
            .filter(|x| **x)
            .count()
    }
}

/// Une colonne : son nom, la classe DSN de ses etablissements, et ses engagements (rangs dans `etat_par_engagement`).
struct Colonne {
    nom: String,
    dsn: usize,
    membres: Vec<usize>,
}

/// LES COLONNES DU MOIS. Les engagements d'une meme vague font une colonne ; la plus avancee est G. Une DPAE revenue
/// refusee ne reste pas dans G, qui dit ce qui est fait : elle sort a cote. Un etablissement sans engagement du mois
/// dont la DSN diverge a sa colonne. `dsn_fait` : par classe DSN, si elle est faite.
fn colonnes(points: &[Point], dsn_fait: &[bool]) -> Vec<Colonne> {
    let mut vagues: Vec<((bool, &'static str, &'static str, usize), Vec<usize>)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        match vagues.iter_mut().find(|(v, _)| *v == p.vague()) {
            Some((_, m)) => m.push(i),
            None => vagues.push((p.vague(), vec![i])),
        }
    }
    let mut cols: Vec<(usize, Vec<usize>)> = vagues.into_iter().map(|(v, m)| (v.3, m)).collect();
    cols.sort_by_key(|(_, m)| std::cmp::Reverse(points[m[0]].avance(dsn_fait)));
    if let Some((dsn, g)) = cols.first_mut() {
        let (refusees, reste): (Vec<usize>, Vec<usize>) = g.iter().partition(|i| points[**i].dpae == "refuse");
        if !refusees.is_empty() && !reste.is_empty() {
            let dsn = *dsn;
            *g = reste;
            cols.insert(1, (dsn, refusees));
        }
    }
    for c in 0..dsn_fait.len() {
        if !cols.iter().any(|(d, _)| *d == c) {
            cols.push((c, Vec::new()));
        }
    }
    if cols.is_empty() {
        cols.push((0, Vec::new()));
    }
    const COTE: &str = "ABCDEFHIJKLMNOPQRSTUVWXYZ";
    cols.into_iter()
        .enumerate()
        .map(|(k, (dsn, membres))| {
            let nom = match k {
                0 => "G".to_string(),
                k => COTE.chars().nth(k - 1).map_or_else(|| format!("C{}", k), String::from),
            };
            Colonne { nom, dsn, membres }
        })
        .collect()
}

/// Une ligne en cases : les colonnes voisines au meme point (`point`) se fondent. Chaque case : son point, ses
/// engagements, ses colonnes, et la classe DSN de sa premiere colonne.
fn cases_de_la_ligne(cols: &[Colonne], point: impl Fn(&Colonne) -> String) -> Vec<(String, Vec<usize>, Vec<String>, usize)> {
    let mut out: Vec<(String, Vec<usize>, Vec<String>, usize)> = Vec::new();
    for c in cols {
        let p = point(c);
        match out.last_mut() {
            Some((q, m, n, _)) if *q == p => {
                m.extend(&c.membres);
                n.push(c.nom.clone());
            }
            _ => out.push((p, c.membres.clone(), vec![c.nom.clone()], c.dsn)),
        }
    }
    out
}

/// Le point d'une colonne sur une ligne : le premier etat de `ordre` qu'un de ses engagements porte, sinon `fait`.
fn point_de(c: &Colonne, points: &[Point], ligne: fn(&Point) -> &'static str, ordre: &[&'static str]) -> String {
    ordre.iter().find(|o| c.membres.iter().any(|i| ligne(&points[*i]) == **o)).copied().unwrap_or("fait").to_string()
}

// ------------------------------------------------------------ petits outils

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `un if k == 1 else des` : le pluriel de `_pluriel`, qui compte zero au pluriel.
/// Le nom d'un etablissement dans une case : sa commune (celle de la fiche entreprise pour le principal), sinon son NIC.
fn nom_de_l_etablissement(ent: &Value, nic: &str) -> String {
    let fiche = if nic == droit::bareme::nic_principal(ent) { Some(ent.clone()) } else { droit::bareme::entree(ent, nic) };
    let commune = fiche.and_then(|f| ["commune_affichage", "commune"].iter().filter_map(|c| f[*c].as_str()).map(str::trim).find(|c| !c.is_empty()).map(String::from));
    commune.unwrap_or_else(|| format!("NIC {}", nic))
}

fn plur(k: usize, un: &'static str, des: &'static str) -> &'static str {
    if k <= 1 {
        un
    } else {
        des
    }
}

/// Un refus du chargement d'une saisie. Les refus que l'ecran ne tient pas
/// pour un defaut du logiciel -- le spectacle pas encore choisi, les jours ou
/// les heures pas encore saisis (`SaisieIncomplete`, leur liste est
/// `engagement::saisie_incomplete`) -- gardent leur classe ; tout autre est
/// celui du moteur.
pub fn classer_saisie(message: String) -> Echec {
    let incomplete = engagement::saisie_incomplete(&message);
    Echec { classe: if incomplete { Some("SaisieIncomplete") } else { None }, message }
}

/// `d["cle"]` sur un dictionnaire de fiche : `KeyError` si elle manque.
fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    d.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn paire(cle: &Value, numero: &Value) -> Option<Paire> {
    Some((cle.as_str()?.to_string(), numero.as_str()?.to_string()))
}

/// Les `[salarie, numero, valeur]` rendus par `etats.rs` pour un dictionnaire a cle de contrat, en table. Une cle qui
/// n'est pas faite de deux textes ne peut egaler aucun engagement.
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

/// Quand le contrat (ou son dernier avenant) de l'engagement `numero` du salarie `cle` est parti (`departs`) ; `null` s'il
/// n'est pas parti.
pub(crate) fn contrat_parti_le(departs: &engagement::departs::Departs, cle: &str, numero: Option<&str>) -> Value {
    let Some(n) = numero else { return Value::Null };
    departs.de(cle, n).iter().rev().find(|d| d.envoi == engagement::departs::Envoi::Contrat).map_or(Value::Null, |d| Value::String(d.le.clone()))
}

/// Ce qui est a refaire pour l'engagement `e` : le present compare a la version sur laquelle chaque envoi est parti
/// (`engagement::etats::a_refaire`) ; rien pour un engagement en saisie.
pub(crate) fn a_refaire_de(ctx: &Contexte, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> Vec<ARefaire> {
    // Un contrat rompu n'a pas d'avenant (`contrats::seule_la_rupture`).
    let saisie = |x: Option<&Value>| x.and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let Some(v) = e.valide() else { return Vec::new() };
    let presente = saisie(v.objet().brut.get("valide"));
    let mut r = engagement::etats::a_refaire(ctx, v, departs);
    r.retain(|a| !matches!(a, ARefaire::Avenant { depuis } if crate::routes::contrats::seule_la_rupture(&saisie(v.version_numero(*depuis)), &presente)));
    r
}

/// Les contrats de `mois` dont l'avenant est a faire signer : `(cle du salarie, numero)` des engagements dont les clauses
/// ont change depuis la version sur laquelle le contrat est parti (`a_refaire_de`).
pub(crate) fn avenants_a_faire(ctx: &Contexte, mois: &str) -> R<HashSet<(String, String)>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let etats = engagement::etats::du_mois(ctx, mois, &departs).map_err(plain)?;
    Ok(etats
        .iter()
        .filter(|e| a_refaire_de(ctx, e, &departs).iter().any(|a| matches!(a, ARefaire::Avenant { .. })))
        .filter_map(|e| {
            let b = Value::Object(e.objet().brut.clone());
            Some((engagement::cle_de_l_engagement(ctx, &b)?, b.get("numero").map(valeur_en_texte)?))
        })
        .collect())
}

/// La date d'embauche que la DPAE partie a declaree, quand elle n'est plus celle de l'engagement (`ARefaire::Dpae`).
pub(crate) fn debut_declare(refaire: &[ARefaire]) -> Option<String> {
    refaire.iter().find_map(|a| match a {
        ARefaire::Dpae { debut_declare, .. } => Some(debut_declare.clone()),
        _ => None,
    })
}

/// L'etablissement (NIC) dont releve l'engagement `e` pour `mois` : celui de la fiche de son salarie telle que le calcul
/// de sa paie du mois l'a lue (`fiches`) ; sans paie ecrite du mois, de sa fiche d'aujourd'hui.
pub(crate) fn nic_de_l_engagement(ctx: &Contexte, mois: &str, e: &engagement::etats::Etat) -> Option<String> {
    nic_de_l_engagement_ecrit(ctx, mois, &Value::Object(e.objet().brut.clone()))
}

/// `nic_de_l_engagement`, sur l'engagement tel qu'il est ecrit (`brut`).
pub(crate) fn nic_de_l_engagement_ecrit(ctx: &Contexte, mois: &str, brut: &Value) -> Option<String> {
    let fiche = match brut.pointer(&format!("/valide/paies/{}", mois)) {
        Some(ecrite) => ecrite.pointer("/fiches/paie").filter(|f| f.is_object()).cloned()?,
        None => {
            let mut f = ctx.salarie(&engagement::cle_de_l_engagement(ctx, brut)?).ok()?;
            let nic = brut.get("etablissement").and_then(Value::as_str).filter(|n| !n.trim().is_empty());
            if let (Some(o), Some(n)) = (f.as_object_mut(), nic) {
                o.insert("etablissement".into(), Value::String(n.to_string()));
            }
            f
        }
    };
    droit::bareme::etablissement_du_salarie(&ctx.entreprise, &fiche).ok()
}

/// La DSN de `mois` est-elle a refaire ? Une DSN est celle d'un etablissement (`derniere` : sa ligne du journal, son NIC
/// en `detail.etablissement`, le principal s'il est tu) : seuls comptent les engagements qui en relevent
/// (`nic_de_l_engagement`). La paie de l'un d'eux a change depuis la version sur laquelle elle est partie
/// (`a_refaire_de`), ou ils ne sont plus ceux qu'elle a emportes (`versions`) : un engagement ajoute, ou retire.
pub(crate) fn dsn_a_refaire(ctx: &Contexte, mois: &str, derniere: &Value, etats: &[engagement::etats::Etat], departs: &engagement::departs::Departs) -> bool {
    // Une ligne qui ne dit pas ce qu'elle a emporte ne se compare a rien.
    let Some(emportes) = derniere.get("versions").and_then(Value::as_array) else { return false };
    let nic = derniere
        .get("detail")
        .and_then(|d| d.get("etablissement"))
        .map(valeur_en_texte)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| droit::bareme::nic_principal(&ctx.entreprise));
    let siens: Vec<&engagement::etats::Etat> = etats.iter().filter(|e| nic_de_l_engagement(ctx, mois, e).as_deref() == Some(nic.as_str())).collect();
    if siens.iter().flat_map(|e| a_refaire_de(ctx, e, departs)).any(|a| matches!(a, ARefaire::Dsn { mois: ref m, .. } if m == mois)) {
        return true;
    }
    let paire = |v: &Value| (valeur_en_texte(&v["salarie"]), valeur_en_texte(&v["numero"]));
    // Les corrections de mois passes que la ligne porte ne sont pas des engagements de ce mois.
    let mut emportes: Vec<(String, String)> = emportes.iter().filter(|v| v.get("mois").is_none()).map(paire).collect();
    let mut presents: Vec<(String, String)> = siens
        .iter()
        .filter(|e| e.valide().is_some())
        .filter_map(|e| {
            let b = Value::Object(e.objet().brut.clone());
            Some((engagement::cle_de_l_engagement(ctx, &b)?, b.get("numero").map(valeur_en_texte)?))
        })
        .collect();
    emportes.sort();
    presents.sort();
    emportes != presents
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

/// La saisie en JSON, sans les cles
/// `_...`, ni `valide` -- ce qui change dans une saisie sans
/// changer la paie.
pub(crate) fn sans_bruit(x: &Value) -> Value {
    match x {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !k.starts_with('_') && *k != "valide")
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

/// L'ORDRE DES FICHIERS DEPOSABLES D'UN MOIS (`fichiers_deposables`), le meme partout : plus jamais la date de
/// modification du fichier, que deux machines -- ou deux rejeux -- ne donnent pas pareille.
/// - Les DSN : D'ABORD LA MENSUELLE (nature 01 ; un etablissement par fichier, le principal `DSN_<Mois>.dsn` avant les
///   `DSN_<Mois>_<NIC>.dsn`, par le nom) : c'est « la DSN du mois » que l'ecran et l'echeance attendent. D'un meme
///   etablissement, LA DERNIERE ECRITE D'ABORD : un annule-et-remplace s'ecrit a cote de la DSN deja
///   partie, sous `DSN_<Mois>_AR<ordre>.dsn` (`depart::fabriquer_dsn`), et c'est lui qui part ; il se reconnait a son numero
///   d'ordre (S20.G00.05.004, ou `_AR<n>` du nom), le plus grand d'abord. Rangees par le seul nom, la DSN deja deposee
///   (`DSN_Janvier2025.dsn`) passait avant son annule-et-remplace (`DSN_Janvier2025_AR2.dsn`) : la boite « Deposer » et
///   l'etape DSN montraient, testaient et auraient redepose l'ancienne. PUIS LES SIGNALEMENTS (arret, fin de contrat,
///   amorcage...), dans l'ordre de leur numero d'ordre (S20.G00.05.004) : unique et jamais remis a zero, il dit l'ordre ou
///   le moteur les a produits (`ordre_signalements`, cahier technique DSN 2026.1 p. 136), et chacun part a son tour.
/// - Les DPAE : la plus recemment produite d'abord, lue dans son nom (`DPAE_<Mois>_<AAAAMMJJ-HHMM>.txt` apres
///   `DPAE_<Mois>.txt` : l'ordre inverse des noms).
pub(crate) fn ordre_de_depot(nom: &str, chemin: &Path) -> (u8, i64, String, std::cmp::Reverse<String>) {
    if suffixe(nom).to_lowercase() == ".dsn" {
        let apercu = std::fs::read(chemin).map(|o| envois::depot::lire(&o)).ok();
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
    /// Ce qui manque a chaque engagement du mois (`None` : rien), dans l'ordre de `stock::objets_du_mois` : `data` et
    /// `paies` ne portent que ceux qui ne manquent de rien.
    pub(crate) manques: Vec<Option<String>>,
}

/// LA PAIE DE L'ENGAGEMENT QUE DESIGNE `id` (`engagement::stock::identites`) parmi `paies` : son salarie, son premier jour,
/// et son rang parmi celles-la.
fn paie_de<'p>(paies: &'p [Paie], id: &Value) -> Option<&'p Paie> {
    let (salarie, debut) = (id.get("salarie")?.as_str()?, id.get("debut")?.as_str()?);
    let rang = usize::try_from(id.get("rang")?.as_u64()?).ok()?;
    paies.iter().filter(|p| p.engagement.salarie == salarie && p.engagement.debut.iso() == debut).nth(rang)
}

fn texte_de(v: &Value) -> String {
    valeur_en_texte(v)
}

/// Les etats du journal et de la saisie, calcules une fois par processus.
pub(crate) struct Globales {
    pub(crate) dpae: HashMap<Paire, Value>,
    pub(crate) recus: HashMap<Paire, Value>,
    pub(crate) recus_par: HashMap<Paire, Value>,
    pub(crate) renvoyes: HashSet<Paire>,
    pub(crate) avenants: HashMap<Paire, Value>,
    /// Comment chaque contrat est parti la derniere fois : `imprime` ou `courriel`.
    pub(crate) par: HashMap<Paire, Value>,
}

/// Tout ce qu'une route lit, lu une fois.
pub struct Moteur<'a> {
    ctx: &'a Contexte,
    globales: Option<Rc<Globales>>,
    fichiers: HashMap<String, Value>,
    /// LES ENGAGEMENTS TELS QU'ILS SONT ECRITS, sans recalcul d'eux-memes (`engagement::etats::tels_qu_ecrits`) : ce que
    /// lit `mois_a_ouvrir`, qui regarde chaque mois passe a l'ouverture et ne calcule rien.
    ecrit: bool,
}

impl<'a> Moteur<'a> {
    pub fn nouveau(ctx: &'a Contexte) -> Moteur<'a> {
        Moteur { ctx, globales: None, fichiers: HashMap::new(), ecrit: false }
    }

    /// Ce qui est parti (`envois::journal::lu`, tire une fois par contenu du journal) : ce que le moteur passe a
    /// l'engagement, qui ne lit pas le journal.
    pub(crate) fn departs(&self) -> R<engagement::departs::Departs> {
        envois::journal::lu(&self.ctx.racine).map(|j| j.departs).map_err(plain)
    }

    /// Les engagements de `mois`, dans l'ordre de l'ecran : recalcules d'eux-memes s'il le faut (`du_mois`), ou tels
    /// qu'ils sont ecrits (`ecrit`).
    pub(crate) fn etats_du_mois(&self, mois: &str) -> R<Vec<engagement::etats::Etat>> {
        let departs = self.departs()?;
        if self.ecrit {
            return engagement::etats::tels_qu_ecrits(self.ctx, mois, &departs).map_err(plain);
        }
        engagement::etats::du_mois(self.ctx, mois, &departs).map_err(plain)
    }

    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    /// Les lignes de `journal/depots.json` (`envois::journal::lu`, la lecture qui donne aussi `departs`).
    pub(crate) fn lignes(&self) -> R<std::sync::Arc<Vec<Value>>> {
        envois::journal::lu(self.racine()).map(|j| j.lignes).map_err(plain)
    }

    fn nic(&self) -> String {
        droit::bareme::nic_principal(&self.ctx.entreprise)
    }

    // ------------------------------------------------------ la saisie du mois

    /// La saisie d'un mois, chargee sans calculer la paie : ses engagements qui ne manquent de rien
    /// (`mois::manques_de_la_saisie`), et ce qui manque a chacun de ses engagements, dans son ordre.
    fn charger_saisie(&mut self, mois: &str) -> R<SaisieMois> {
        let mut data = engagement::saisie_du_mois(self.racine(), mois).map_err(classer_saisie)?;
        let manques = super::mois::manques_de_la_saisie(self.ctx, &data);
        if let Some(Value::Array(es)) = data.get_mut("engagements") {
            let mut m = manques.iter();
            es.retain(|_| m.next().is_some_and(Option::is_none));
        }
        let mois_dit = data.get("mois").map(en_texte).ok_or_else(|| plain(socle::fichiers::absent("mois")))?;
        let mut paies = engagement::embauches_suivies(self.ctx, &data).map_err(classer_saisie)?;
        let annee: i32 = mois_dit.get(..4).and_then(|a| a.parse().ok()).ok_or_else(|| {
            erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(mois_dit.get(..4).unwrap_or(&mois_dit))))
        })?;
        engagement::numeros::numeroter_les_contrats(self.ctx, &mut paies, annee, &self.departs()?).map_err(plain)?;
        Ok(SaisieMois { data, paies, manques })
    }

    // --------------------------------------------- ce que le journal et la saisie disent

    pub(crate) fn globales(&mut self) -> R<Rc<Globales>> {
        if let Some(g) = &self.globales {
            return Ok(g.clone());
        }
        let lignes = self.lignes()?;
        let dpae = en_table(etats::etat_des_embauches(&lignes)?);
        let recus = etats::contrats_recus(&lignes)?;
        let recus = en_table(recus.sortie(Clone::clone));
        let recus_par = en_table(etats::contrats_recus_comment(&lignes)?);
        let renvoyes = cles_en_ensemble(etats::renvoyes_depuis_la_signature(&lignes)?.cles());
        let avenants = en_table(etats::avenants(&lignes)?);
        let par = en_table(etats::contrats_partis_par(&lignes)?);
        let g = Rc::new(Globales { dpae, recus, recus_par, renvoyes, avenants, par });
        self.globales = Some(g.clone());
        Ok(g)
    }

    // ------------------------------------------------------------ etat_par_engagement

    /// `etat_par_engagement` : ou en est CHAQUE engagement du mois. `avec_brut` : le
    /// montant brut de chaque paie y figure ; le bilan, qui ne l'affiche pas,
    /// ne le demande jamais.
    pub fn etat_par_engagement(&mut self, mois: &str, avec_brut: bool) -> R<Vec<Value>> {
        // UN MOIS SANS ENGAGEMENT N'EST PAS UNE ERREUR.
        if !engagement::stock::a_une_saisie(self.racine(), mois).map_err(plain)? {
            return Ok(Vec::new());
        }
        let s = self.charger_saisie(mois)?;
        // Valide, ou parti : la date de la validation (`engagement::etats`), engagement par engagement ; chacun retrouve son
        // embauche par son identite (`engagement::stock::identites`).
        let etats_eng = self.etats_du_mois(mois)?;
        let ids = engagement::stock::identites(&etats_eng.iter().map(|e| e.objet().clone()).collect::<Vec<_>>());
        let brut_ecrit = |e: &engagement::etats::Etat| e.valide().and_then(|v| v.paie_ecrite(mois)).and_then(|p| p.pointer("/paie/brut")).cloned();
        let calcul = if avec_brut && etats_eng.iter().any(|e| brut_ecrit(e).is_none()) {
            Some(engagement::pour_l_ecran(self.ctx, &s.data).map_err(classer_saisie)?)
        } else {
            None
        };
        let g = self.globales()?;
        let departs = self.departs()?;

        // Chaque ligne porte `engagement` : l'identite par laquelle sa carte le designe. Un engagement auquel il manque un
        // fait a sa ligne a lui (`ligne_a_completer`) ; les autres retrouvent leur paie parmi ceux qui ne manquent de rien.
        let mut out = Vec::new();
        for (j, (e, id)) in etats_eng.iter().zip(&ids).enumerate() {
            if let Some(Some(manque)) = s.manques.get(j) {
                let mut ligne = self.ligne_a_completer(mois, e.objet(), manque)?;
                ligne["engagement"] = id.clone();
                out.push(ligne);
                continue;
            }
            let jumeaux_a_completer = (0..j).filter(|&x| s.manques.get(x).is_some_and(Option::is_some) && ids[x]["salarie"] == id["salarie"] && ids[x]["debut"] == id["debut"]).count();
            let mut id_calcule = id.clone();
            id_calcule["rang"] = json!(id["rang"].as_u64().unwrap_or_default() as usize - jumeaux_a_completer);
            let id_calcule = &id_calcule;
            let Some(p) = paie_de(&s.paies, id_calcule) else { continue };
            let cle = texte_de(requis(&p.salarie, "_cle")?);
            let debut = p.engagement.debut.iso();
            // CE QUI EST A REFAIRE : le present compare a la version sur laquelle chaque envoi est parti.
            let refaire = a_refaire_de(self.ctx, e, &departs);
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
            let valide = e.valide().map_or(Value::Null, |v| Value::String(v.le().to_string()));
            // Un CDI n'a pas de fin : `null`, comme `Engagement::canon`.
            let fin = p.engagement.fin.map_or(Value::Null, |f| Value::String(f.iso()));
            let mut ligne = Map::new();
            ligne.insert("salarie".into(), Value::String(cle.clone()));
            ligne.insert("nom".into(), Value::String(super::nom_a_l_ecran(sal)));
            ligne.insert("spectacle".into(), spectacle);
            ligne.insert("nature".into(), Value::String(nat.libelle.clone()));
            ligne.insert("numero".into(), p.engagement.numero.clone().map_or(Value::Null, Value::String));
            ligne.insert("debut".into(), Value::String(debut.clone()));
            ligne.insert("fin".into(), fin);
            ligne.insert("emploi".into(), sal.get("emploi").cloned().unwrap_or(Value::Null));
            ligne.insert("statut".into(), sal.get("statut").cloned().unwrap_or(Value::Null));
            // Son etablissement (NIC) : la DSN qui le porte.
            let nic = nic_de_l_engagement(self.ctx, mois, e);
            ligne.insert("etablissement".into(), nic.map_or(Value::Null, Value::String));
            // LES ECHEANCES de l'engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche
            // de la convention), `droit::echeances`. Les documents n'en ont pas : chaque compagnie les envoie quand elle
            // veut, souvent apres la DSN qui fait remonter l'AER. Un mois passe lu a l'ouverture (`ecrit`) ne les demande pas.
            if !self.ecrit {
                let code_emploi = p.engagement.code_emploi_ft.clone().unwrap_or_else(|| sal.get("code_emploi_ft").map(valeur_en_texte).unwrap_or_default());
                let idcc = self.ctx.entreprise.get("idcc").cloned().unwrap_or(Value::Null);
                let remise = droit::echeances::remise_du_contrat(&p.engagement.debut, &idcc, &code_emploi, Some(&self.ctx.entreprise)).map_err(plain)?;
                ligne.insert("echeances".into(), json!({"dpae": droit::echeances::dpae(&p.engagement.debut).en_json(), "contrat": remise.en_json()}));
            }
            if avec_brut {
                // Le brut de sa paie ecrite s'il est valide, celui du calcul de la saisie sinon. Qu'il manque est un defaut du
                // logiciel, jamais la saisie de l'utilisateur, et la phrase le lui dit.
                let calcule = calcul.as_ref().and_then(|c| paie_de(&c.paies, id_calcule)).map(|x| Value::String(x.brut.to_string()));
                let brut = brut_ecrit(e).or(calcule).ok_or_else(|| Echec {
                    classe: Some("DefautDuLogiciel"),
                    message: format!(
                        "Le montant de la paie de {} {} ne se lit pas : le calcul de {} ne correspond pas à la saisie \
                         enregistrée. Ce n'est pas votre saisie qui est en cause : c'est un défaut du logiciel, \
                         noté dans ses incidents.",
                        nom_famille, prenom, mois
                    ),
                })?;
                ligne.insert("brut".into(), Value::String(brut_en_texte(&brut)?));
            }
            ligne.insert("valide".into(), valide);
            // RECALCULEE D'ELLE-MEME (`engagement::etats::du_mois`) : pourquoi, et quand.
            let recalcul = e.valide().and_then(|v| v.recalcul(mois)).and_then(engagement::etats::phrase_du_recalcul);
            ligne.insert("recalcul".into(), recalcul.map_or(Value::Null, Value::String));
            // REPRIS D'UN ANCIEN LOGICIEL : la carte le dit (`engagement::importe::vient_d_un_ancien_logiciel`).
            let repris = engagement::importe::vient_d_un_ancien_logiciel(&self.ctx.racine, &p.engagement.salarie, &p.engagement.debut.iso(), p.engagement.numero.as_deref().unwrap_or(""));
            ligne.insert(
                "contrat".into(),
                // ECRIT : le contrat de cet engagement se fait signer ici (`engagement::contrat_a_signer`).
                json!({"premier_mois": premier_mois, "dernier_mois": dernier_mois,
                       "depuis": mois_du_contrat.first(), "jusqu_a": mois_du_contrat.last(),
                       "repris": repris, "ecrit": engagement::contrat_a_signer(&self.ctx.racine, &p.engagement).map_err(plain)?}),
            );
            // CE QUE CE SALARIE RECOIT POUR CETTE PAIE : le bulletin s'il y a une paie, le certificat Conges Spectacles
            // du contrat d'usage qui finit (`crate::routes::dossier_salarie::certificats_dus`), l'AER Cinema Spectacle qui lui est due
            // (`aer::due`). L'etape « Documents » du mois les nomme d'apres ceci, et rien d'autre.
            let mut pieces: Vec<&str> = Vec::new();
            if !p.engagement.sans_paie() {
                pieces.push("bulletin");
            }
            if !nat.mensuelle() && dernier_mois && p.declaree_a_la_caisse_des_conges().map_err(plain)? {
                pieces.push("certificat_cs");
            }
            if documents::aer::due(&p.engagement).map_err(plain)? {
                pieces.push("aer");
            }
            ligne.insert("pieces".into(), json!(pieces));
            // LA DPAE DE CET ENGAGEMENT : partie (`departs`), et ce qui en est revenu (la ligne qui la porte, par le numero du
            // contrat). Refusee au bilan, elle n'est pas partie ; la ligne dit quand elle a ete refusee.
            let numero = p.engagement.numero.clone().unwrap_or_default();
            let mut dpae_v = g.dpae.get(&(cle.clone(), numero.clone())).filter(|v| vrai(v)).unwrap_or(objet_vide());
            let mut partie_le = departs.de(&cle, &numero).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dpae).map(|d| d.le.clone()).max();
            // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie (`ARefaire::Dpae`) : la DPAE de
            // l'ancienne date vaut-elle (`crate::regularisation::dpae_d_une_date_changee`) ?
            let mut date_changee = Value::Null;
            if let Some(avant) = debut_declare(&refaire) {
                let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&avant, &debut);
                if !vaut {
                    dpae_v = objet_vide();
                    partie_le = None;
                }
                date_changee = json!({"avant": avant, "vaut": vaut, "phrase": phrase});
            }
            let fait = match partie_le {
                Some(le) => Value::String(le),
                None if vrai(get(dpae_v, "refusee")?) => get(dpae_v, "quand")?.clone(),
                None => Value::Null,
            };
            let dpae = json!({
                "date_changee": date_changee,
                "fait": fait,
                "passee": vrai(get(dpae_v, "passee")?),
                "refusee": vrai(get(dpae_v, "refusee")?),
                "anomalies": ou(get(dpae_v, "anomalies")?, &LISTE_VIDE),
                "dossier": ou(get(dpae_v, "dossier")?, texte_vide()),
                "deja_existante": vrai(get(dpae_v, "deja_existante")?),
                "fin_declaree": if nat.date_fin_dpae() { get(dpae_v, "fin")?.clone() } else { Value::Null },
            });
            let k = (cle.clone(), numero.clone());
            let nul = Value::Null;
            let avenant = refaire.iter().any(|a| matches!(a, ARefaire::Avenant { .. }));
            let mut signature = etat_signature(
                &contrat_parti_le(&departs, &cle, p.engagement.numero.as_deref()),
                g.par.get(&k).unwrap_or(&nul),
                g.recus.get(&k).unwrap_or(&nul),
                g.recus_par.get(&k).unwrap_or(&nul),
                g.renvoyes.contains(&k),
                g.avenants.get(&k).unwrap_or(&nul),
                avenant,
            );
            // UN CDI EN COURS N'A PAS D'ETAT DE CONTRAT MENSUEL.
            if nat.mensuelle() && nat.duree_indeterminee && !premier_mois {
                let recu = g.recus.get(&k).filter(|r| vrai(r));
                let depuis = mois_du_contrat.first().cloned().unwrap_or_default();
                let signe_en = match recu {
                    Some(r) => valeur_en_texte(r).chars().take(7).collect::<String>(),
                    None => depuis.clone(),
                };
                signature["cdi_en_cours"] = json!({"depuis": depuis, "signe_en": signe_en});
            }
            // LES DOCUMENTS DE CET ENGAGEMENT, pour ce mois : son dernier envoi (`departs`), et son bulletin a refaire.
            let documents_le = p.engagement.numero.as_deref().and_then(|n| {
                departs.de(&cle, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.as_deref() == Some(mois) && d.par.is_none()).map(|d| d.le.clone()).max()
            });
            let bulletin = refaire.iter().any(|a| matches!(a, ARefaire::Bulletin { mois: m, .. } if m == mois));
            let documents = etat_documents(documents_le, bulletin);
            ligne.insert("etats".into(), json!({"dpae": dpae, "signature": signature, "documents": documents}));
            let vue = Value::Object(ligne.clone());
            ligne.insert("carres".into(), carres(&vue));
            ligne.insert("dpae_choix".into(), choix_dpae(&vue));
            // LA FRISE : un engagement modifie apres un envoi dit quand, et ce qui a change de version en version.
            let frise = e.valide().and_then(|v| engagement::versions::frise(self.ctx, v, &departs, mois));
            ligne.insert("versions".into(), frise.unwrap_or(Value::Null));
            ligne.insert("engagement".into(), id.clone());
            out.push(Value::Object(ligne));
        }
        Ok(out)
    }

    /// LA LIGNE D'UN ENGAGEMENT JAMAIS VALIDE AUQUEL IL MANQUE UN FAIT : rien ne se calcule sur lui, rien n'est parti ;
    /// il dit qui, quand, et ce qui lui manque. Ses carres disent « à compléter » (`Point::de`).
    fn ligne_a_completer(&self, mois: &str, o: &engagement::stock::Objet, manque: &str) -> R<Value> {
        let vue = o.vue(mois).map_err(plain)?.unwrap_or_default();
        let e = Value::Object(vue);
        let cle = engagement::cle_de_l_engagement(self.ctx, &e).unwrap_or_else(|| e.get("salarie").map(valeur_en_texte).unwrap_or_default());
        let nom = self.ctx.salarie(&cle).ok().map(|f| super::nom_a_l_ecran(&f)).unwrap_or_else(|| cle.clone());
        let spectacle = match e.get("spectacle").filter(|s| vrai(s)) {
            Some(s) => self.ctx.spectacle(&valeur_en_texte(s)).ok().and_then(|f| f.get("titre").cloned()).unwrap_or_else(|| s.clone()),
            None => Value::Null,
        };
        let nature = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map(|n| n.libelle.clone()).unwrap_or_default();
        let premier_mois = o.premier_mois() == mois;
        let mut ligne = json!({
            "salarie": cle, "nom": nom, "spectacle": spectacle, "nature": nature,
            "numero": e.get("numero").filter(|n| vrai(n)).cloned().unwrap_or(Value::Null),
            "debut": engagement::debut_brut(&e), "fin": e.get("fin").filter(|f| vrai(f)).cloned().unwrap_or(Value::Null),
            "valide": Value::Null, "recalcul": Value::Null, "versions": Value::Null, "pieces": [],
            "a_completer": manque, "non_calcule": true,
            "contrat": {"premier_mois": premier_mois, "repris": false},
            "etats": {"dpae": {}, "signature": {}, "documents": {}},
        });
        ligne["carres"] = carres(&ligne);
        ligne["dpae_choix"] = choix_dpae(&ligne);
        Ok(ligne)
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
            // L'ORDRE NE DEPEND PAS DE L'HEURE D'UN FICHIER (deux rejeux, deux machines, le meme ordre) : il se lit
            // dans ce que le moteur sait de chacun (`ordre_de_depot`).
            let racine = self.racine().to_path_buf();
            let ranger = |mut v: Vec<&(String, PathBuf, u64)>| -> Value {
                v.sort_by_cached_key(|f| ordre_de_depot(&f.0, &f.1));
                Value::Array(
                    v.into_iter()
                        .map(|(nom, chemin, octets)| {
                            // « ecrit le ... » : ce que le moteur a consigne en l'ecrivant, pas la date du fichier.
                            json!({"nom": nom, "chemin": chemin.to_string_lossy(), "octets": octets, "ecrit": crate::compagnie::depart::ecrit_le(&racine, chemin)})
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

    /// `empreinte_saisie` : ce que la DSN du mois declare, resume en 16 caracteres -- les paies validees du mois
    /// (`engagement::etats::empreinte_des_paies_validees` : un brouillon n'y est pas, la DSN ne le declare pas), et la
    /// version de chaque engagement d'un mois passe dont elle porte la correction
    /// (`produire_regularisations::regularisations_pour`) : une correction validee apres l'ecriture du fichier le rend
    /// perime comme une paie du mois qui change. `None` si rien ne se lit ni ne se corrige.
    pub(crate) fn empreinte_saisie(&self, mois: &str) -> R<Option<String>> {
        let vue = engagement::etats::empreinte_des_paies_validees(self.ctx, mois);
        let portees: Vec<Value> = match declarations::produire_regularisations::regularisations_pour(self.ctx, mois, &self.departs()?) {
            Ok(regs) => regs.iter().map(|r| json!([r.mois, valeur_en_texte(r.apres.salarie.get("_cle").unwrap_or(&Value::Null)), r.apres.engagement.numero, r.version])).collect(),
            Err(e) => vec![json!(e.message)],
        };
        Ok(match (vue, portees.is_empty()) {
            (vue, true) => vue.map(|v| v.chars().take(16).collect()),
            (vue, false) => Some(empreinte_de(&json!({"saisie": vue, "corrections": portees}))),
        })
    }

    /// `limite` : le dernier jour ou un annule-et-remplace est admis, jusqu'a minuit : la veille de l'echeance de depot
    /// (`echeance_dsn::Echeance::veille`, reportee comme elle).
    pub(crate) fn limite(&self, mois: &str) -> R<Jour> {
        let e = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        e.veille().map_err(plain)
    }

    /// Le refus d'un annule-et-remplace du mois apres sa `limite` (cahier technique DSN §1.4.2) ; `Ok` jusqu'a elle.
    pub(crate) fn refuser_si_hors_delai(&self, mois: &str) -> R<()> {
        let limite = self.limite(mois)?;
        if self.ctx.aujourdhui <= limite {
            return Ok(());
        }
        Err(erreur(
            "AnnuleRemplaceHorsDelai",
            format!(
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). Ce qui a change dans ce mois part dans la DSN d'un mois suivant, en correction. Rien ne part.",
                mois,
                limite.jj_mm_aaaa()
            ),
        ))
    }

    /// Le plus haut ordre porte par ces depots reels du mois (`depots_reels`) ; `None` s'il n'y en a pas.
    pub(crate) fn dernier_ordre(&self, mois: &str, faits: &[Value]) -> R<Option<i64>> {
        let mut dernier: Option<i64> = None;
        for l in faits {
            let n = self.ordre_de_la_ligne(mois, l)?;
            dernier = Some(dernier.map_or(n, |d| d.max(n)));
        }
        Ok(dernier)
    }

    /// Le rang d'un depot reel du journal : l'ordre que sa ligne porte (S20.G00.05.004 du fichier parti, noté au depot).
    pub(crate) fn ordre_de_la_ligne(&self, mois: &str, l: &Value) -> R<i64> {
        let d = ou(get(l, "detail")?, objet_vide());
        let ordre = get(d, "ordre")?;
        let t = if vrai(ordre) { valeur_en_texte(ordre) } else { String::new() };
        t.trim().parse::<i64>().map_err(|_| {
            erreur(
                "ValueError",
                format!("journal/depots.json : un depot reel de la DSN de {} ne porte pas son ordre (S20.G00.05.004). Le rang d'un annule-et-remplace ne se devine pas.", mois),
            )
        })
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

    /// Ou en est le mois vis-a-vis de
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
        // Apres la limite : la DSN suivante corrige chaque contrat dont la paie validee a change.
        out.insert("corrigee_par_la_suivante".into(), json!(false));
        let Some(derniere) = faits.last() else {
            return Ok(Value::Object(out));
        };
        let dernier = self.dernier_ordre(mois, &faits)?.unwrap_or(i64::MIN);
        out.insert("dernier_ordre".into(), json!(dernier));
        out.insert("prochain_ordre".into(), json!(dernier + 1));
        // Ce que la DSN a emporte, compare au present des engagements du mois (`dsn_a_refaire`).
        let mut modifiee = false;
        if derniere.get("versions").is_some() {
            let etats_eng = self.etats_du_mois(mois)?;
            modifiee = dsn_a_refaire(self.ctx, mois, derniere, &etats_eng, &self.departs()?);
            out.insert("saisie_modifiee".into(), json!(modifiee));
        }
        // Apres la limite : les contrats dont la paie validee n'est plus celle que la DSN a declaree ; la DSN suivante
        // porte leur correction d'elle-meme (`engagement::etats::correction_due`, comme
        // `produire_regularisations::corrections_dues`), ceux de cet etablissement seuls.
        let corriges = if admis {
            0
        } else {
            let departs = self.departs()?;
            let plusieurs = !droit::bareme::secondaires(&self.ctx.entreprise_du_mois(mois).map_err(plain)?).is_empty();
            let voulu = if vrai(nic) { valeur_en_texte(nic) } else { self.nic() };
            let mut n = 0;
            for e in self.etats_du_mois(mois)? {
                let Some(v) = e.valide() else { continue };
                if plusieurs && nic_de_l_engagement(self.ctx, mois, &e).as_deref() != Some(voulu.as_str()) {
                    continue;
                }
                if engagement::etats::correction_due(self.ctx, v, &departs, mois, "9999-12").map_err(plain)?.is_some() {
                    n += 1;
                }
            }
            n
        };
        if corriges > 0 {
            modifiee = true;
            out.insert("saisie_modifiee".into(), json!(true));
        }
        let quand = quand_lisible(get(derniere, "quand")?);
        let jour = lim.jj_mm_aaaa();
        if corriges > 0 {
            let pluriel = if corriges > 1 { "s" } else { "" };
            out.insert("corrigee_par_la_suivante".into(), json!(true));
            out.insert(
                "phrase".into(),
                json!(format!(
                    "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — {} contrat{} corrigé{} par la DSN suivante",
                    quand, jour, corriges, pluriel, pluriel
                )),
            );
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
                        "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — la correction part dans la DSN suivante",
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
        let d = Value::Object(crate::compagnie::depart::journal_des_ecritures(self.racine()));
        let note = get(&d, &format!("{}/{}", mois, nom))?;
        if vrai(note) && vrai(get(note, "empreinte_saisie")?) {
            let notee = get(note, "empreinte_saisie")?;
            let maintenant = self.empreinte_saisie(mois)?;
            if maintenant.map_or(true, |m| !est(notee, &m)) {
                return Ok(Some(format!("l'engagement a changé depuis que ce fichier a été écrit (le {})", quand_lisible(get(note, "quand")?))));
            }
            return Ok(None);
        }
        // SANS EMPREINTE NOTEE, LE MOTEUR NE SAIT PAS : il ne compare plus les dates des fichiers. Une fictive
        // livree, decompressee chez un visiteur, a des dates de fichier qui ne disent rien de l'ordre ou elle a ete ecrite,
        // et chacun de ses mois se disait « paie changee ».
        Ok(None)
    }

    // ----------------------------------------------------------- les groupes

    /// LE CHEMIN DU FICHIER A DEPOSER (`quoi` : DPAE ou DSN) : ou il en est (ecrite, en test, testee, test refuse,
    /// paie changee), le fichier, et ce qu'un test refuse a dit ; `None` sans fichier a deposer, ou s'il est deja parti.
    /// `nic` : pour la DSN d'une compagnie a plusieurs etablissements, celle de cet etablissement (son en-tete,
    /// S21.G00.11.001) ; `Null` : le premier fichier, quel qu'il soit.
    fn chemin_de_depot(&mut self, mois: &str, quoi: &str, nic: &Value) -> R<Option<(&'static str, Value, Option<String>)>> {
        // « Ecrite » pour une DSN : le meme critere que l'horloge, l'accueil et les rappels (`depart::mois_ecrit`,
        // la DSN mensuelle notee au journal des ecritures), pas un fichier pose dans `sorties/<mois>/`. Un mois passe lu a
        // l'ouverture (`ecrit`) ne demande que ce qui reste, pas le geste qui l'ouvre.
        if self.ecrit || (quoi == DSN && !crate::compagnie::depart::mois_ecrit(self.racine(), mois)) {
            return Ok(None);
        }
        let fichiers = self.fichiers_deposables(mois)?;
        let de_l_etablissement = |f: &&Value| -> bool {
            if quoi != DSN || !vrai(nic) {
                return true;
            }
            let octets = std::fs::read(valeur_en_texte(&f["chemin"])).unwrap_or_default();
            envois::depot::lire(&octets).champ("etablissement") == valeur_en_texte(nic)
        };
        let Some(premier) = fichiers.get(quoi).and_then(Value::as_array).and_then(|l| l.iter().find(de_l_etablissement)).cloned() else {
            return Ok(None);
        };
        let chemin = PathBuf::from(valeur_en_texte(&premier["chemin"]));
        let lignes = self.lignes()?;
        let empreinte = envois::journal::empreinte_fichier(&chemin).map_err(|m| erreur("FileNotFoundError", m))?;
        if vrai(&etats::deja_depose(&lignes, quoi, &empreinte)?) {
            return Ok(None);
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
        // UN TEST REFUSE SE DIT AVEC SES ANOMALIES, la ou l'on propose d'envoyer : le code et le message du bilan.
        let refus = match r["ratees"].as_array().and_then(|l| l.last()) {
            Some(l) if etape == "test_refuse" => Some(crate::compagnie::depot_acte::pourquoi_rate(l).trim_end_matches('.').to_string()),
            _ => None,
        };
        Ok(Some((etape, premier, refus)))
    }

    /// LE GESTE d'une case orange qui depose (DPAE, DSN) : le chemin du fichier deja ecrit s'il y en a un, sinon `defaut`.
    fn action_de_depot(&mut self, mois: &str, quoi: &str, nic: &Value, defaut: Value) -> R<(Value, Option<String>)> {
        let Some((etape, fichier, refus)) = self.chemin_de_depot(mois, quoi, nic)? else { return Ok((defaut, None)) };
        // Le geste ouvre le recapitulatif du depot (`depot-apercu`), qui prend le chemin du fichier.
        let fichier = fichier["chemin"].clone();
        let nom = if quoi == DSN { "la DSN" } else { "la DPAE" };
        let action = match etape {
            // Le fichier ecrit ne declare plus la paie : la DSN se refait depuis les engagements valides, et repasse par
            // le test.
            "paie_changee" => action("deposer_dsn", format!("Réécrire {} et la tester…", nom)),
            "ecrite" => json!({"geste": "deposer_fichier", "libelle": format!("Tester {}…", nom), "fichier": fichier, "quoi": quoi}),
            "en_test" => json!({"geste": "deposer_fichier", "libelle": "Voir le test en cours…", "fichier": fichier, "quoi": quoi}),
            "testee" => json!({"geste": "deposer_fichier", "libelle": format!("Envoyer {} testée…", nom), "fichier": fichier, "quoi": quoi}),
            _ => json!({"geste": "deposer_fichier", "libelle": "Voir le refus du test…", "fichier": fichier, "quoi": quoi}),
        };
        Ok((action, refus.map(|r| format!("test refusé : {}", r))))
    }

    /// `bilan_du_mois` : LES ETAPES DU MOIS, dans l'ordre de l'habitude (DPAE, Contrats, DSN, Documents), chacune
    /// avec les groupes que le panneau dessine : `{etat, compte, compte_texte, quoi, phrase, echeance, action}` (`groupe`), `etat` parmi `fait`,
    /// `a_faire`, `en_attente`, `refuse`. Un groupe `fait` n'a ni compte ni phrase ; un groupe qui reste compte en nombre
    /// simple, avec son echeance (`texte`, et `court` sans l'annee, que la case ecrit) et le geste qu'ouvre un clic. `colonnes` : celles du mois (`colonnes`, G d'abord) ;
    /// chaque groupe nomme celles qu'il couvre, les memes d'une etape a l'autre. L'etape `suivante` est la premiere qui a
    /// quelque chose a faire de notre cote. Aucun montant : le brut n'est jamais lu.
    pub fn bilan_du_mois(&mut self, mois: &str) -> R<Value> {
        // UN MOIS TENU AVANT CE LOGICIEL : ses etapes se sont faites ailleurs ; elles le disent, sans geste.
        if let Some(vue) = crate::import::mois::vue(self.ctx, mois)? {
            let b = crate::import::mois::etapes(&vue);
            let etapes = ORDRE
                .iter()
                .map(|(cle, nom)| {
                    let x = &b[*cle];
                    let mut g = if est(&x["couleur"], "vert") {
                        fait()
                    } else {
                        groupe(if est(&x["couleur"], "rouge") { "refuse" } else { "information" }, Value::Null, String::new(), valeur_en_texte(&x["courte"]), Value::Null, Value::Null)
                    };
                    g["colonnes"] = json!(["G"]);
                    json!({"cle": cle, "nom": nom, "groupes": [g], "phrase": x["phrase"].clone()})
                })
                .collect();
            return Ok(json!({"mois": mois, "colonnes": ["G"], "etapes": suivante(etapes)}));
        }
        let paies = self.etat_par_engagement(mois, false)?;
        let echeance_de = |ps: &[&Value], quoi: &str| -> Value {
            ps.iter().map(|p| &p["echeances"][quoi]).filter(|e| e.is_object()).min_by_key(|e| valeur_en_texte(&e["jour"])).cloned().unwrap_or(Value::Null)
        };
        let avec = |mut g: Value, noms: &[String]| -> Value {
            g["colonnes"] = json!(noms);
            g
        };

        // --- Les colonnes : chaque engagement a son point, la DSN celui de son etablissement.
        let (classes, second) = self.groupes_dsn(mois)?;
        let classe_de = |p: &Value| -> usize {
            let nic = valeur_en_texte(&p["etablissement"]);
            classes.iter().position(|(nics, _)| nics.contains(&nic)).unwrap_or(0)
        };
        let points: Vec<Point> = paies.iter().map(|p| Point::de(p, classe_de(p))).collect();
        let dsn_fait: Vec<bool> = classes.iter().map(|(_, g)| est(&g["etat"], "fait")).collect();
        let cols = colonnes(&points, &dsn_fait);
        let de = |m: &[usize]| -> Vec<&Value> { m.iter().map(|i| &paies[*i]).collect() };

        // --- DPAE : par embauche du mois. Partie, elle est faite tant qu'aucun retour ne revient KO.
        let mut dpae = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.dpae, &["a_faire", "refuse", A_COMPLETER])) {
            let siens: Vec<&Value> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].dpae == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "refuse" => groupe("refuse", json!(n), n.to_string(), format!("refusée{} · l'éditeur est prévenu", plur(n, "", "s")), echeance_de(&siens, DPAE), action("historique", "Voir le bilan…".into())),
                "a_faire" => {
                    let (a, refus) = self.action_de_depot(mois, DPAE, &Value::Null, action("declarer_embauches", "Envoyer les DPAE…".into()))?;
                    let quoi = refus.map_or_else(|| "à déposer".to_string(), |r| format!("à déposer · {}", r));
                    groupe("a_faire", json!(n), n.to_string(), quoi, echeance_de(&siens, DPAE), a)
                }
                A_COMPLETER => a_completer(n),
                _ => fait(),
            };
            dpae.push(avec(g, &noms));
        }

        // --- Contrats : un par embauche dont la nature a un contrat ecrit (`engagement::contrat_a_signer`).
        let sig = |p: &Value| p["etats"]["signature"].clone();
        let mut contrats = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.contrat, &["a_envoyer", "a_refaire", "en_attente", A_COMPLETER])) {
            let siens: Vec<&Value> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].contrat == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "en_attente" => {
                    // CE QUI EST REVENU SUR CE QUI VIENT DE PARTIR, et sur cela seul : les contrats de la case partis le
                    // jour du dernier envoi encore attendu.
                    let jour = |p: &Value| valeur_en_texte(&sig(p)["fait"]).chars().take(10).collect::<String>();
                    let dernier = siens.iter().map(|p| jour(p)).max().unwrap_or_default();
                    let partis: Vec<&Value> = de(&m)
                        .into_iter()
                        .zip(m.iter())
                        .filter(|(p, i)| matches!(points[**i].contrat, "fait" | "en_attente") && vrai(&sig(p)["fait"]) && jour(p) == dernier)
                        .map(|(p, _)| p)
                        .collect();
                    let recus = partis.iter().filter(|p| vrai(&sig(p)["recu"]) && !vrai(&sig(p)["avenant_attendu"])).count();
                    // L'echeance est celle des contrats attendus (L. 1242-13) ; un avenant attendu n'en a pas (aucun texte
                    // depose ne date sa remise).
                    let contrats_attendus: Vec<&Value> = siens.iter().copied().filter(|p| !vrai(&sig(p)["avenant_attendu"])).collect();
                    groupe(
                        "en_attente",
                        json!({"recus": recus, "partis": partis.len()}),
                        format!("{}/{}", recus, partis.len()),
                        format!("reçu{}", plur(recus, "", "s")),
                        echeance_de(&contrats_attendus, "contrat"),
                        action("contrats_en_attente", "Reçus ou à relancer…".into()),
                    )
                }
                // L'AVENANT n'a pas d'echeance : L. 1242-13 date la remise du contrat a l'embauche, aucun texte depose ne
                // date celle d'un avenant (regle 1 : rien ne se devine).
                "a_refaire" => groupe("a_faire", json!(n), n.to_string(), "à refaire signer".into(), Value::Null, action("faire_signer", "Demander la signature…".into())),
                "a_envoyer" => groupe("a_faire", json!(n), n.to_string(), "à envoyer".into(), echeance_de(&siens, "contrat"), action("faire_signer", "Demander la signature…".into())),
                A_COMPLETER => a_completer(n),
                _ => fait(),
            };
            contrats.push(avec(g, &noms));
        }

        // --- DSN : par etablissement (`groupes_dsn`), chaque colonne sous la DSN de ses etablissements.
        let dsn: Vec<Value> = cases_de_la_ligne(&cols, |c| c.dsn.to_string()).into_iter().map(|(_, _, noms, k)| avec(classes[k].1.clone(), &noms)).collect();

        // --- Documents : par salarie du mois, des que son engagement est valide, sans echeance : ils partent quand on veut.
        let mut documents = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.documents, &["a_renvoyer", "a_envoyer", A_COMPLETER])) {
            let siens: Vec<&Value> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].documents == point).map(|(p, _)| p).collect();
            let n = siens.iter().map(|p| valeur_en_texte(&p["salarie"])).collect::<BTreeSet<String>>().len();
            let g = match point.as_str() {
                "a_renvoyer" => groupe("a_faire", json!(n), n.to_string(), "à renvoyer".into(), Value::Null, action("envoyer_documents", format!("Renvoyer les documents corrigés ({})…", n))),
                "a_envoyer" => groupe("a_faire", json!(n), n.to_string(), "à envoyer".into(), Value::Null, action("envoyer_documents", "Envoyer les documents…".into())),
                // Ce qui reste a completer se compte en engagements, comme a la DPAE.
                A_COMPLETER => a_completer(siens.len()),
                _ => fait(),
            };
            documents.push(avec(g, &noms));
        }
        // CE QUI PART CE MOIS-CI, NOMME PAR LE MOTEUR : les pieces des paies du mois, dans l'ordre de l'envoi.
        let noms_pieces = [("bulletin", "bulletin"), ("certificat_cs", "certificat Congés Spectacles"), ("aer", "AER")];
        let presentes: Vec<&str> = noms_pieces
            .iter()
            .filter(|(c, _)| paies.iter().any(|p| p["pieces"].as_array().is_some_and(|a| a.iter().any(|x| x == c))))
            .map(|(_, n)| *n)
            .collect();

        // UN MOIS SANS ENGAGEMENT n'a rien de fait : ce qui n'a rien a faire s'y dit « rien ce mois-ci », en gris. Ce que le
        // moteur demande quand meme (une DSN due sans salarie) garde son etat.
        let vide = paies.is_empty();
        let rien = |gs: Vec<Value>| -> Vec<Value> {
            gs.into_iter()
                .map(|g| if vide && est(&g["etat"], "fait") { let mut r = rien_ce_mois(); r["colonnes"] = g["colonnes"].clone(); r } else { g })
                .collect()
        };
        let etapes = vec![
            json!({"cle": DPAE, "nom": "DPAE", "groupes": rien(dpae)}),
            json!({"cle": SIGNATURE, "nom": "Contrats", "groupes": rien(contrats)}),
            json!({"cle": DSN, "nom": "DSN", "groupes": rien(dsn), "second": second}),
            json!({"cle": DOCUMENTS, "nom": "Documents", "groupes": rien(documents), "detail": presentes.join(", ")}),
        ];
        let noms: Vec<String> = cols.iter().map(|c| c.nom.clone()).collect();
        Ok(json!({"mois": mois, "colonnes": noms, "etapes": suivante(etapes)}))
    }

    /// LA DSN DU MOIS, par etablissement. Elle « est mensuelle et produite par établissement employeur » (cahier technique
    /// DSN 2026.1, § 2.4, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : chaque
    /// etablissement a la sienne (`groupes_dsn_de`). Les etablissements au meme point (meme etat, phrase et echeance)
    /// forment une classe, une case ; quand ils ne sont pas tous au meme point, la case d'une classe qui n'est pas faite
    /// nomme ses etablissements (« Lyon refusée · l'éditeur est prévenu »). Rend les classes (leurs NIC, leur case), le
    /// principal dans la premiere, et le geste de plus (un annule-et-remplace admis sans necessite) du principal.
    fn groupes_dsn(&mut self, mois: &str) -> R<(Vec<(Vec<String>, Value)>, Value)> {
        let ent = self.ctx.entreprise_du_mois(mois).map_err(plain)?;
        if droit::bareme::secondaires(&ent).is_empty() {
            let (g, second) = self.groupes_dsn_de(mois, &Value::Null)?;
            return Ok((vec![(vec![droit::bareme::nic_principal(&ent)], g)], second));
        }
        let point = |g: &Value| (valeur_en_texte(&g["etat"]), valeur_en_texte(&g["phrase"]), valeur_en_texte(&g["echeance"]["jour"]));
        let mut classes: Vec<(Vec<String>, Value)> = Vec::new();
        let mut second = Value::Null;
        for (rang, nic) in declarations::dsn::nics(&ent).into_iter().enumerate() {
            let (g, s) = self.groupes_dsn_de(mois, &json!(nic))?;
            if rang == 0 {
                second = s;
            }
            match classes.iter_mut().find(|(_, x)| point(x) == point(&g)) {
                Some((nics, _)) => nics.push(nic),
                None => classes.push((vec![nic], g)),
            }
        }
        if classes.len() > 1 {
            for (nics, g) in classes.iter_mut().filter(|(_, g)| !est(&g["etat"], "fait")) {
                let noms: Vec<String> = nics.iter().map(|n| nom_de_l_etablissement(&ent, n)).collect();
                g["compte_texte"] = json!(noms.join(", "));
                g["phrase"] = json!(format!("{} {}", noms.join(", "), valeur_en_texte(&g["quoi"])));
            }
        }
        Ok((classes, second))
    }

    /// LES GROUPES DE LA DSN de l'etablissement `nic` (`Null` : la compagnie n'en a qu'un), et le geste de plus qu'elle
    /// offre (un annule-et-remplace encore admis alors que rien ne l'exige). L'echeance : celle du depot (`echeance_dsn`, le jour meme a midi) ; la veille a minuit pour un
    /// annule-et-remplace ; apres un refus, celle du depot qui la reprend.
    ///
    /// LA DSN REFUSEE n'est pas integree : celle qui la reprend est un depot initial (cahier technique DSN 2026.1,
    /// § 1.4.2, `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt` : le depot initial va jusqu'au jour de
    /// l'echeance a midi, l'annule-et-remplace jusqu'a la veille a minuit). INFERENCE : un depot refuse n'ouvre pas
    /// l'annule-et-remplace, puisqu'il n'y a rien a annuler ; l'echeance de la reprise est celle du depot initial.
    fn groupes_dsn_de(&mut self, mois: &str, nic: &Value) -> R<(Value, Value)> {
        let echeance = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        let au_depot = echeance_dsn(echeance.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 ; CSS R. 243-6 et R. 133-14 (declarations::echeance_dsn)");
        let e = crate::import::mois::dsn_avant(self.ctx, mois, nic)?;
        let partie = matches!(e["etat"].as_str(), Some("passee" | "attente" | "refusee"));
        // LA DSN DEPOSEE PAR L'ANCIEN LOGICIEL ne se refait pas ici (regle 4) : ce que la saisie ajoute au mois se
        // declare dans la DSN suivante qui n'est pas partie, et la case le dit (`import::mois::dsn_de_l_ancien_logiciel`).
        if let Some(d) = crate::import::mois::dsn_de_l_ancien_logiciel(self.ctx, mois)?.filter(|_| !partie) {
            let mut g = fait();
            if d["engagements"].as_u64().is_some_and(|n| n > 0) {
                let porteur = valeur_en_texte(&d["porteur"]);
                let e2 = declarations::echeance_dsn::de_la_compagnie(self.ctx, &porteur).map_err(|m| erreur("ValueError", m))?;
                let mut a = action("deposer_dsn", format!("Déposer la DSN de {}…", socle::date::libelle_du_mois_ou_tel_quel(&porteur, socle::date::Graphie::Accentuee)));
                a["mois"] = json!(porteur);
                g = groupe("a_faire", Value::Null, String::new(), valeur_en_texte(&d["quoi"]), echeance_dsn(e2.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 (declarations::echeance_dsn)"), a);
            }
            g["phrase"] = d["phrase"].clone();
            return Ok((g, Value::Null));
        }
        if !partie {
            if !self.dsn_a_declarer(mois, nic)? {
                return Ok((fait(), Value::Null));
            }
            // Avant son premier jour de depot, la case reste a deposer : c'est le depot qui dit le jour
            // (`compagnie::depart::dsn_a_deposer`).
            let (a, refus) = self.action_de_depot(mois, DSN, nic, action("deposer_dsn", "Déposer la DSN…".into()))?;
            let quoi = refus.map_or_else(|| "à déposer".to_string(), |r| format!("à déposer · {}", r));
            return Ok((groupe("a_faire", Value::Null, String::new(), quoi, au_depot, a), Value::Null));
        }
        if e["etat"] == "refusee" {
            return Ok((
                groupe("refuse", Value::Null, String::new(), "refusée · l'éditeur est prévenu".into(), au_depot, action("historique", "Voir le bilan…".into())),
                Value::Null,
            ));
        }
        // Partie : faite, tant qu'aucun retour ne revient KO. Ce qui la rouvre : une paie modifiee depuis.
        let r = self.etat_remplacement(mois, nic)?;
        let libelle = format!("Refaire la DSN (annule-et-remplace, ordre {})…", r["prochain_ordre"]);
        let veille = Jour::lire(&valeur_en_texte(&r["limite"])).map_or(Value::Null, |j| echeance_dsn(j, "à minuit", "Cahier technique DSN 2026.1, § 1.4.2 (atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt)"));
        if vrai(&r["saisie_modifiee"]) && vrai(&r["a_envoyer"]) {
            // Un annule-et-remplace deja ecrit montre son chemin ; sinon, le refaire.
            let ecrit = self.chemin_de_depot(mois, DSN, nic)?.is_some();
            let (a, refus) = if ecrit { self.action_de_depot(mois, DSN, nic, Value::Null)? } else { (action("deposer_dsn", libelle), None) };
            let quoi = refus.map_or_else(|| "à refaire".to_string(), |x| format!("à refaire · {}", x));
            return Ok((groupe("a_faire", Value::Null, String::new(), quoi, veille, a), Value::Null));
        }
        if vrai(&r["saisie_modifiee"]) {
            // Apres la veille : la correction part dans la DSN qui porte (la premiere d'un mois suivant qui n'est pas
            // partie), celle que l'avertissement de « Modifier l'engagement » nomme aussi.
            let suivant = crate::regularisation::dsn_qui_porte(self.ctx, mois)?;
            let e2 = declarations::echeance_dsn::de_la_compagnie(self.ctx, &suivant).map_err(|m| erreur("ValueError", m))?;
            let mut a = action("deposer_dsn", format!("Déposer la DSN de {}…", socle::date::libelle_du_mois_ou_tel_quel(&suivant, socle::date::Graphie::Accentuee)));
            a["mois"] = json!(suivant);
            return Ok((
                groupe(
                    "a_faire",
                    Value::Null,
                    String::new(),
                    format!("à corriger dans la DSN de {}", socle::date::libelle_du_mois_ou_tel_quel(&suivant, socle::date::Graphie::Accentuee)),
                    echeance_dsn(e2.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 (declarations::echeance_dsn)"),
                    a,
                ),
                Value::Null,
            ));
        }
        let second = if vrai(&r["admis"]) { action("deposer_dsn", libelle) } else { Value::Null };
        Ok((fait(), second))
    }

    /// LA DSN DE `mois` A-T-ELLE QUELQUE CHOSE A DECLARER pour l'etablissement `nic` (`Null` : le seul) ? Elle est a
    /// deposer des qu'un engagement valide touche le mois (« le dépôt initial de la DSN "normale" peut
    /// être émis jusqu'à 1 mois à l'avance et 3 mois en retard », cahier technique DSN 2026.1, § 1.4.2,
    /// `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`), ou qu'elle porte la correction d'un mois passe
    /// dont l'annule-et-remplace n'est plus admis (§ 2.1.2.1, `chapitre-2-principes-de-constitution.txt` : « Une
    /// correction est un évènement connu après la date de clôture de la paie. Il donne lieu à la déclaration d'une
    /// correction dans la DSN suivante » ; `produire_regularisations::regularisations_pour`). Sinon, rien n'est a faire.
    /// Ne lit que ce qui est ecrit.
    fn dsn_a_declarer(&mut self, mois: &str, nic: &Value) -> R<bool> {
        let plusieurs = !droit::bareme::secondaires(&self.ctx.entreprise_du_mois(mois).map_err(plain)?).is_empty();
        let voulu = if vrai(nic) { valeur_en_texte(nic) } else { self.nic() };
        let de_l_etablissement = |ctx: &Contexte, m: &str, brut: &Value| !plusieurs || nic_de_l_engagement_ecrit(ctx, m, brut).as_deref() == Some(voulu.as_str());
        for e in self.etats_du_mois(mois)? {
            if e.valide().is_some() && de_l_etablissement(self.ctx, mois, &Value::Object(e.objet().brut.clone())) {
                return Ok(true);
            }
        }
        let departs = self.departs()?;
        for m in engagement::stock::mois_saisis(self.racine()).unwrap_or_default() {
            if m.as_str() >= mois {
                break;
            }
            if !declarations::produire_regularisations::se_corrige_ailleurs(self.ctx, &departs, &m)? {
                continue;
            }
            for e in self.etats_du_mois(&m)? {
                let Some(v) = e.valide() else { continue };
                if de_l_etablissement(self.ctx, &m, &Value::Object(e.objet().brut.clone())) && engagement::etats::correction_due(self.ctx, v, &departs, &m, mois).map_err(plain)?.is_some() {
                    return Ok(true);
                }
            }
            for n in engagement::etats::non_actifs_du_mois(self.ctx, &m).map_err(plain)? {
                if de_l_etablissement(self.ctx, &m, &Value::Object(n.objet().brut.clone())) && n.correction_due(self.ctx, &departs, &m, mois).map_err(plain)?.is_some() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // ------------------------------------------------------- les autres routes

    /// `exercice_clos` : la cloture de l'annee de ce mois, ou `None`.
    pub(crate) fn exercice_clos(&self, mois_ou_annee: &str) -> R<bool> {
        let p = self.racine().join("config").join("exercices.json");
        if !socle::fichiers::existe(&p) {
            return Ok(false);
        }
        // LU UNE FOIS TANT QU'IL NE CHANGE PAS : `etat` le demandait pour chaque mois. Le refus d'un fichier
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
        // A L'OUVERTURE, RIEN NE SE CALCULE : les groupes d'un mois passe se tirent de ce qui est ecrit.
        self.ecrit = true;
        for m in engagement::stock::mois_saisis(self.racine()).unwrap_or_default() {
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

    /// LES ETAPES QUI RESTENT A UN MOIS PASSE : celles dont un groupe n'est pas fait (`bilan_du_mois`).
    pub(crate) fn etapes_restantes(&mut self, mois: &str) -> R<Vec<String>> {
        let b = self.bilan_du_mois(mois)?;
        Ok(b["etapes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["groupes"].as_array().is_some_and(|g| g.iter().any(|x| !est(&x["etat"], "fait"))))
            .map(|e| valeur_en_texte(&e["cle"]))
            .collect())
    }

    /// `embauches_a_declarer` : les contrats des engagements valides dont la DPAE n'est pas encore partie, chacun avec
    /// l'empreinte de ce que sa DPAE declare. La seule liste des DPAE a faire : le panneau, les rappels, le filet,
    /// l'horloge la lisent.
    pub fn embauches_a_declarer(&mut self, mois: Option<&str>) -> R<Value> {
        // UNE DERIVATION POUR TOUS LES MOIS : le bilan de chaque mois charge sa saisie, et les conges d'un
        // permanent y lisent les paies validees des mois d'avant (`conges::paies_du_mois`) une fois pour toute la route.
        // Rien ne s'ecrit entre deux mois de cette lecture : le memo vaut pour toute la route.
        let _derivation = engagement::conges::Derivation::ouvrir();
        let departs = self.departs()?;
        let mois: Vec<String> = match mois.filter(|m| !m.is_empty()) {
            Some(m) => vec![m.to_string()],
            None => {
                let mut v = engagement::stock::mois_saisis(self.racine()).unwrap_or_default();
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
            let etats_m = engagement::etats::tels_qu_ecrits(self.ctx, &m, &departs).map_err(plain)?;
            // La DPAE part de l'engagement valide : un engagement en saisie n'a rien a declarer. Chacun retrouve son embauche
            // par son identite (salarie, numero du contrat).
            for e in &etats_m {
                let Some(v) = e.valide() else { continue };
                let brut = Value::Object(v.objet().brut.clone());
                let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(self.ctx, &brut), brut.get("numero").and_then(Value::as_str)) else { continue };
                let Some(p) = s.paies.iter().find(|p| p.salarie.get("_cle").and_then(Value::as_str) == Some(cle.as_str()) && p.engagement.numero.as_deref() == Some(numero)) else {
                    continue;
                };
                let refaire = a_refaire_de(self.ctx, e, &departs);
                let debut = p.engagement.debut.iso();
                // UNE DPAE PAR EMBAUCHE, reconnue a sa date : « L'embauche d'un salarié ne peut intervenir qu'après
                // déclaration nominative » (code du travail, L. 1221-10), et la DPAE porte la « Date et heure
                // d'embauche » (R. 1221-1, 3°) ; `atelier/reference/sources/code-du-travail/xml/L1221-10.xml` et
                // `R1221-1.xml`. INFERENCE : un cachet ajoute a une date nouvelle d'un engagement deja declare prolonge le
                // meme engagement (un avenant) ; l'embauche et sa date ne changent pas, aucune DPAE ne ressort -- aucun texte
                // depose n'en demande une nouvelle quand la duree (R. 1221-1, 4°) s'allonge. Une date d'embauche avancee,
                // elle, en appelle une (`dpae_d_une_date_changee`, plus bas).
                // La DPAE de cet engagement est partie (`departs`) ; sa date d'embauche n'a pas change depuis (`ARefaire::Dpae`).
                let partie = departs.de(&cle, numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Dpae);
                let ancienne = debut_declare(&refaire);
                if partie && ancienne.is_none() {
                    continue;
                }
                // Un contrat en cours se declare avec son premier mois.
                if !p.engagement.premier_mois().map_err(plain)? {
                    continue;
                }
                // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie : reculee, la DPAE de l'ancienne
                // date vaut ; avancee, une DPAE est a deposer, et la phrase dit pourquoi (`dpae_d_une_date_changee`).
                let pourquoi = match ancienne {
                    Some(a) => {
                        let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&a, &debut);
                        if vaut {
                            continue;
                        }
                        json!(phrase)
                    }
                    None => Value::Null,
                };
                let titre = if vrai(&p.spectacle) { p.spectacle.get("titre").filter(|t| vrai(t)).cloned() } else { None };
                let mut due = json!({
                    "mois": m, "salarie": cle,
                    "nom": super::nom_a_l_ecran(&p.salarie),
                    "spectacle": titre.unwrap_or_else(|| json!("salarié au mois")),
                    "debut": debut, "fin": p.engagement.fin.map(|f| f.iso()),
                    "empreinte": empreinte_de_l_embauche(&p.engagement, &p.salarie),
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

    /// `etat-du-mois` : ce que le panneau des etapes du mois dessine (`colonnes`, `etapes`).
    fn etat_du_mois(&mut self, mois: &str) -> R<Value> {
        let mut bilan = self.bilan_du_mois(mois)?;
        Ok(json!({"mois": mois, "colonnes": bilan["colonnes"].take(), "etapes": bilan["etapes"].take()}))
    }
}

/// « le 05/11/2026 » d'un instant du journal.
fn le_jour(v: &Value) -> String {
    let t = valeur_en_texte(v);
    Jour::lire(t.get(..10).unwrap_or(&t)).map(|j| format!("le {}", j.jj_mm_aaaa())).unwrap_or(t)
}

/// LES TROIS CARRES DE LA LIGNE D'UN ENGAGEMENT VALIDE (DPAE, Contrat, Documents), lus comme les cases du panneau
/// (`Point::de`) : chacun son nom, son etat (`fait`, `a_faire`, `en_attente`, `refuse`), sa phrase (l'infobulle) et le
/// geste qu'ouvre un clic. Le contrat ou l'avenant parti et pas revenu se note signe d'un clic (`contrat_recu`).
fn carres(p: &Value) -> Value {
    let pt = Point::de(p, 0);
    if vrai(&p["non_calcule"]) {
        let manque = valeur_en_texte(&p["a_completer"]);
        let carre = |cle: &str, nom: &str, point: &str| {
            if point == A_COMPLETER {
                json!({"cle": cle, "nom": nom, "etat": "a_faire", "phrase": format!("À compléter avant de valider : {}", manque),
                       "action": null, "couleur": super::couleur_de_l_etat("a_faire"), "reste": "à compléter"})
            } else {
                json!({"cle": cle, "nom": nom, "etat": "fait", "phrase": "Rien à faire ici pour cet engagement", "action": null,
                       "couleur": super::couleur_de_l_etat("fait"), "reste": ""})
            }
        };
        return json!([carre(DPAE, "DPAE", pt.dpae), carre(SIGNATURE, "Contrat", pt.contrat), carre(DOCUMENTS, "Documents", pt.documents)]);
    }
    let d = &p["etats"]["dpae"];
    let s = &p["etats"]["signature"];
    let o = &p["etats"]["documents"];
    let embauche = vrai(&p["contrat"]["premier_mois"]);
    let depuis = engagement::libelle_mois(&valeur_en_texte(&p["contrat"]["depuis"]));

    let (etat, phrase, geste) = match pt.dpae {
        "refuse" => {
            let motif = motif_dpae(d);
            ("refuse", format!("DPAE refusée {} : {}", le_jour(&d["fait"]), motif), action("historique", "Voir le bilan…".into()))
        }
        "a_faire" => ("a_faire", "DPAE à déposer".to_string(), action("declarer_embauches", "Envoyer les DPAE…".into())),
        _ if !embauche => ("fait", format!("DPAE faite en {}, à l'embauche", depuis), Value::Null),
        _ if vrai(&d["passee"]) => ("fait", format!("DPAE déposée {}, passée", le_jour(&d["fait"])), Value::Null),
        _ => ("fait", format!("DPAE déposée {}, retour pas encore arrivé", le_jour(&d["fait"])), Value::Null),
    };
    let pieces = json!({"vide": "Aucune DPAE déposée pour cette embauche.", "encart": null, "gestes": []});
    let dpae = json!({"cle": DPAE, "nom": "DPAE", "etat": etat, "phrase": phrase, "action": geste, "pieces": pieces});

    let cdi = &s["cdi_en_cours"];
    let nom_contrat = if vrai(cdi) { format!("CDI, signé en {}", engagement::libelle_mois(&valeur_en_texte(&cdi["signe_en"]))) } else { "Contrat".to_string() };
    let (etat, phrase, geste) = match pt.contrat {
        "a_envoyer" => ("a_faire", "Contrat à envoyer".to_string(), action("faire_signer", "Demander la signature…".into())),
        "a_refaire" => ("a_faire", format!("Contrat parti {} : {}", le_jour(&s["fait"]), valeur_en_texte(&s["pourquoi"])), action("faire_signer", "Demander la signature…".into())),
        // Un avenant parti, imprime ou par courriel, se note signe d'un clic, comme le contrat ; la relance est dans la
        // boite des contrats en attente. `par` est celui du dernier depart, l'avenant attendu.
        "en_attente" if vrai(&s["avenant_attendu"]) => (
            "en_attente",
            format!(
                "Avenant n° {} {} {}",
                valeur_en_texte(&s["avenant"]["numero"]),
                if s["par"].as_str() == Some("imprime") { "imprimé" } else { "envoyé" },
                le_jour(&s["avenant"]["quand"])
            ),
            noter_recu("Noter l'avenant signé", s["avenant"]["numero"].clone()),
        ),
        "en_attente" => (
            "en_attente",
            format!("Contrat {} {}", if s["par"].as_str() == Some("imprime") { "imprimé" } else { "envoyé" }, le_jour(&s["fait"])),
            noter_recu("Noter le contrat signé", Value::Null),
        ),
        _ if vrai(cdi) => ("fait", format!("CDI en cours depuis {}, sans terme : rien à envoyer", depuis), Value::Null),
        _ if !embauche => ("fait", format!("Contrat signé en {}", depuis), Value::Null),
        _ if !vrai(&p["contrat"]["ecrit"]) => ("fait", "Pas de contrat à faire signer ici".to_string(), Value::Null),
        _ => ("fait", format!("Contrat signé {}", le_jour(&s["recu"])), Value::Null),
    };
    let contrat = json!({"cle": SIGNATURE, "nom": nom_contrat, "etat": etat, "phrase": phrase, "action": geste, "pieces": pieces_du_contrat(s)});

    let (etat, phrase, geste) = match pt.documents {
        "a_renvoyer" => ("a_faire", format!("Documents à renvoyer : {}", valeur_en_texte(&o["pourquoi"])), action("envoyer_documents", "Renvoyer les documents corrigés…".into())),
        "a_envoyer" => ("a_faire", "Documents à envoyer".to_string(), action("envoyer_documents", "Envoyer les documents…".into())),
        _ => ("fait", format!("Documents partis {}", le_jour(&o["fait"])), Value::Null),
    };
    let documents = json!({"cle": DOCUMENTS, "nom": "Documents", "etat": etat, "phrase": phrase, "action": geste});
    // CE QUI RESTE, EN UN MOT, sur le carre meme (la phrase entiere est l'infobulle), et la couleur de son etat.
    let restes = [
        match pt.dpae { "refuse" => "refusée", "a_faire" => "à déposer", _ => "" },
        match pt.contrat { "a_envoyer" => "à envoyer", "a_refaire" => "à refaire signer", "en_attente" => "signature attendue", _ => "" },
        match pt.documents { "a_renvoyer" => "à renvoyer", "a_envoyer" => "à envoyer", _ => "" },
    ];
    // LE LIEN DU DETAIL vers ce qui est parti : seulement quand une piece est passee par le logiciel.
    let liens = [
        vrai(&d["fait"]).then_some("La DPAE partie"),
        vrai(&s["fait"]).then_some("Le contrat parti"),
        None,
    ];
    let mut tous = [dpae, contrat, documents];
    for ((c, reste), lien) in tous.iter_mut().zip(restes).zip(liens) {
        c["couleur"] = json!(super::couleur_de_l_etat(&valeur_en_texte(&c["etat"])));
        c["coupee"] = json!(est(&c["etat"], "en_attente"));
        c["reste"] = json!(reste);
        c["lien"] = json!(lien);
    }
    json!(tous)
}

/// CE QUE MONTRE LE DETAIL DU CONTRAT d'un engagement (les pieces parties, ouvertes depuis la ligne) : la phrase quand
/// aucune piece n'est passee par le logiciel, l'encart d'un contrat qui n'est plus le bon, et les gestes, chacun son
/// libelle, son geste et s'il est le principal. Un contrat parti ou recu se note signe (ou ne l'est plus) ; un contrat
/// parti se modifie par un avenant, signe ou non, commence ou non ; un contrat jamais parti part a la signature.
fn pieces_du_contrat(s: &Value) -> Value {
    let (fait, recu, caduc) = (vrai(&s["fait"]), vrai(&s["recu"]), vrai(&s["caduc"]));
    // Un contrat ou un avenant parti (`fait`, imprime ou par courriel) est une piece : le detail la nomme, il n'a pas de
    // phrase « sans piece ».
    let vide = match (fait, recu) {
        (true, _) => Value::Null,
        (false, true) => json!("Le contrat a été noté signé à la main : aucun fichier n'est passé par le logiciel."),
        (false, false) => json!("Aucun contrat n'est parti à la signature pour cet engagement."),
    };
    let encart = caduc.then(|| {
        let pourquoi = valeur_en_texte(&s["pourquoi"]);
        let mut c = pourquoi.chars();
        let pourquoi = match c.next() {
            Some(p) => format!("{}{}.", p.to_uppercase(), c.as_str()),
            None => "L'engagement a changé depuis qu'il est parti.".to_string(),
        };
        json!({"titre": "Ce contrat n'est plus le bon", "texte": format!("{} Il se modifie par un avenant, que le salarié signe à son tour.", pourquoi)})
    });
    let mut gestes = Vec::new();
    if fait || recu {
        let libelle = if recu { "Retirer la mention « signé »" } else { "Noter le contrat signé reçu" };
        gestes.push(json!({"libelle": libelle, "action": {"geste": "contrat_recu", "recu": !recu}, "principal": false}));
    }
    if fait {
        gestes.push(json!({"libelle": "Avenant…", "titre": "Modifier le contrat parti : l'ancien texte et le nouveau, article par article", "action": {"geste": "avenant"}, "principal": caduc}));
    }
    if !fait {
        gestes.push(json!({"libelle": "Envoyer à la signature…", "action": {"geste": "faire_signer"}, "principal": true}));
    }
    json!({"vide": vide, "encart": encart, "gestes": gestes})
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT : s'il se coche (une embauche du mois), s'il est coche d'avance (pas de
/// DPAE, ou une DPAE refusee : c'est la seule raison de redeposer ; une DPAE passee ne se redeclare pas), et son etat
/// (`fait`, `a_faire`, `en_attente`, `refuse`) avec sa phrase.
fn choix_dpae(p: &Value) -> Value {
    let d = &p["etats"]["dpae"];
    let choix = |cochable: bool, cochee: bool, etat: &str, phrase: String| json!({"cochable": cochable, "cochee": cochee, "etat": etat, "couleur": super::couleur_de_l_etat(etat), "phrase": phrase});
    // Il lui manque un fait : sa DPAE attend qu'il soit complet.
    if vrai(&p["non_calcule"]) {
        return choix(false, false, "a_faire", format!("à compléter avant de valider : {}", valeur_en_texte(&p["a_completer"])));
    }
    if !vrai(&p["contrat"]["premier_mois"]) {
        let depuis = engagement::libelle_mois(&valeur_en_texte(&p["contrat"]["depuis"]));
        return choix(false, false, "fait", format!("contrat en cours depuis {} — sa DPAE a été faite ce mois-là", depuis));
    }
    if !vrai(&d["fait"]) {
        return choix(true, true, "a_faire", "à déclarer".into());
    }
    let le = le_jour(&d["fait"]);
    if vrai(&d["refusee"]) {
        let motif = motif_dpae(d);
        return choix(true, true, "refuse", format!("refusée {} — {} — à redéclarer", le, motif));
    }
    if vrai(&d["passee"]) {
        let dossier = if vrai(&d["deja_existante"]) {
            " (déjà existante à l'Urssaf, rien de créé)".to_string()
        } else if vrai(&d["dossier"]) {
            format!(" (dossier {})", valeur_en_texte(&d["dossier"]))
        } else {
            String::new()
        };
        return choix(true, false, "fait", format!("déclarée {} — passée{}", le, dossier));
    }
    choix(true, false, "en_attente", format!("déclarée {} — retour pas encore arrivé : on attend, on ne renvoie pas", le))
}

/// Les motifs d'un refus de DPAE, lus des anomalies du retour : « E99 : anomalie fictive », ou « motif non lu ».
fn motif_dpae(d: &Value) -> String {
    let motifs: Vec<String> = d["anomalies"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| match (a.get("code").map(valeur_en_texte).filter(|c| !c.is_empty()), a.get("message").map(valeur_en_texte).filter(|m| !m.is_empty())) {
            (Some(c), Some(m)) => format!("{} : {}", c, m),
            (None, Some(m)) => m,
            (Some(c), None) => c,
            (None, None) => valeur_en_texte(a),
        })
        .collect();
    if motifs.is_empty() { "motif non lu".to_string() } else { motifs.join(" ; ") }
}

pub(crate) fn documents_partis(ctx: &Contexte, mois: &str, etats: &[engagement::etats::Etat], lignes: &[Value]) -> R<HashMap<String, Value>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let resumes = etats::resumes_des_documents(lignes, mois)?;
    let mut out: HashMap<String, Value> = HashMap::new();
    // Les documents d'un salarie partent ensemble, pour tous ses engagements valides du mois : un engagement valide dont
    // ils ne sont pas partis (valide apres leur envoi) les rend a envoyer, avec son bulletin.
    let a_envoyer: HashSet<String> = etats
        .iter()
        .filter(|e| matches!(e, engagement::etats::Etat::Valide(_)))
        .filter_map(|e| engagement::cle_de_l_engagement(ctx, &Value::Object(e.objet().brut.clone())))
        .collect();
    for e in etats {
        if !matches!(e, engagement::etats::Etat::Partis(_)) {
            continue;
        }
        let brut = Value::Object(e.objet().brut.clone());
        let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &brut), brut.get("numero").filter(|n| vrai(n)).map(valeur_en_texte)) else { continue };
        let dernier = departs
            .de(&cle, &numero)
            .iter()
            .filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.as_deref() == Some(mois) && d.par.is_none())
            .map(|d| d.le.clone())
            .max();
        let Some(quand) = dernier else { continue };
        if a_envoyer.contains(&cle) {
            continue;
        }
        let plus_recent = out.get(&cle).and_then(|v| v["quand"].as_str()).is_some_and(|q| q >= quand.as_str());
        if !plus_recent {
            out.insert(cle.clone(), json!({"quand": quand, "resume": resumes.get(&cle).cloned().unwrap_or(Value::Null)}));
        }
    }
    Ok(out)
}
/// La phrase de la pastille Documents d'un salarie dont la paie a change depuis l'envoi.
pub(crate) const DOCUMENTS_CADUCS: &str = "la paie a changé depuis l'envoi";

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois. `parti_le` : le dernier envoi de ses documents ; `caduc` : ils
/// sont partis, et sa paie a change depuis la version sur laquelle ils sont partis (`engagement::etats::a_refaire`) --
/// ils ne sont plus les bons, ils sont a renvoyer.
pub(crate) fn etat_documents(parti_le: Option<String>, caduc: bool) -> Value {
    let fait = parti_le.map_or(Value::Null, Value::String);
    let caduc = vrai(&fait) && caduc;
    json!({"fait": fait, "caduc": caduc, "pourquoi": if caduc { DOCUMENTS_CADUCS } else { "" }})
}

/// `DSN_[^\W\d_]+\d{4}(_AR\d+)?\.dsn` : le nom d'une DSN mensuelle.
pub(crate) fn est_une_dsn_mensuelle(nom: &str) -> bool {
    let Some(reste) = nom.strip_prefix("DSN_") else { return false };
    let Some(reste) = reste.strip_suffix(".dsn") else { return false };
    let c: Vec<char> = reste.chars().collect();
    let lettres = c.iter().take_while(|x| etats::lettre_ou_chiffre(**x) && !x.is_ascii_digit() && **x != '_' && !x.is_numeric()).count();
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

/// La phrase de la pastille Contrat d'un engagement qui a change depuis la version sur laquelle son contrat est parti.
pub(crate) const CONTRAT_A_REFAIRE: &str = "l'engagement a changé depuis l'envoi du contrat : un avenant est à faire signer";

/// La pastille Contrat d'un engagement.
///
/// `caduc` : le contrat parti -- signe ou non -- ne dit plus ce que dit l'engagement : ses clauses ont change depuis la
/// version sur laquelle il est parti (`a_refaire_signer`, `engagement::etats::a_refaire`), ou il porte un autre numero
/// que celui que la DSN declare. Un contrat renvoye attend une NOUVELLE signature : celle de l'ancien ne compte plus.
#[allow(clippy::too_many_arguments)]
pub fn etat_signature(
    fait: &Value, par: &Value, recu: &Value, recu_par: &Value, renvoye: bool, avenants: &Value,
    a_refaire_signer: bool,
) -> Value {
    let (recu, recu_par) = if renvoye { (&Value::Null, &Value::Null) } else { (recu, recu_par) };
    let nul = Value::Null;
    let caduc = vrai(fait) && a_refaire_signer;
    // L'AVENANT : le dernier parti. Pas encore revenu, il laisse la pastille orange.
    let dernier = avenants.as_array().and_then(|a| a.last()).cloned().unwrap_or(Value::Null);
    let attendu = vrai(&dernier) && !vrai(dernier.get("signe").unwrap_or(&nul));
    json!({"fait": fait, "par": if vrai(fait) { par } else { &nul }, "recu": recu, "recu_par": recu_par,
           "caduc": caduc, "pourquoi": if caduc { CONTRAT_A_REFAIRE } else { "" },
           "avenant": dernier,
           "avenant_attendu": attendu})
}

// ----------------------------------------------------------- l'aiguillage

pub(crate) fn mois_de(charge: &Value) -> R<(Value, String)> {
    let v = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    Ok((v.clone(), valeur_en_texte(v)))
}

/// LES ENGAGEMENTS NON ACTIFS que `mois` montrerait (`engagement::etats::non_actifs_du_mois`), pour qu'ils se lisent :
/// le salarie, son nom, le contrat, depuis quand, et ce que leur desactivation rend a faire (`a_faire`).
fn non_actifs(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let mut out = Vec::new();
    for n in engagement::etats::non_actifs_du_mois(ctx, mois).map_err(plain)? {
        let b = Value::Object(n.objet().brut.clone());
        let cle = engagement::cle_de_l_engagement(ctx, &b).unwrap_or_else(|| b.get("salarie").map(valeur_en_texte).unwrap_or_default());
        let nom = ctx
            .salarie(&cle)
            .ok()
            .map(|f| super::nom_a_l_ecran(&f))
            .unwrap_or_else(|| cle.clone());
        out.push(json!({
            "salarie": cle,
            "nom": nom.trim(),
            "numero": b.get("numero").cloned().unwrap_or(Value::Null),
            "debut": engagement::debut_brut(&b),
            "fin": b.get("fin").filter(|v| vrai(v)).cloned().unwrap_or_else(|| engagement::dates_engagement(&b).iter().map(|d| d.get("jour").map_or_else(|| valeur_en_texte(d), valeur_en_texte)).max().map_or(Value::Null, Value::String)),
            "non_actif": n.le(),
            "a_faire": crate::routes::engagement::a_faire_en_phrases(&n.a_refaire(ctx, &departs)),
        }));
    }
    Ok(out)
}

/// Une route de ce module. `charge` : ce que la page envoie (`mois`).
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let mut m = Moteur::nouveau(ctx);
    match nom {
        "etat-par-engagement" => {
            let (brut, mois) = mois_de(charge)?;
            Ok(json!({"mois": brut, "engagements": m.etat_par_engagement(&mois, true)?, "non_actifs": non_actifs(ctx, &mois)?}))
        }
        "etat-du-mois" => {
            let (_, mois) = mois_de(charge)?;
            m.etat_du_mois(&mois)
        }
        "mois-a-ouvrir" => m.mois_a_ouvrir(),
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

/// Ce que la DPAE d'une embauche declare, en 16 caracteres : l'identite du salarie telle que l'engagement la lit, les
/// bornes du contrat, le spectacle, la nature ; pas les taux ni les cachets. Un test de DPAE fait sur une autre empreinte
/// ne vaut plus.
fn empreinte_de_l_embauche(e: &engagement::Engagement, sal: &Value) -> String {
    let champ = |k: &str| socle::fichiers::repr(&sal.get(k).filter(|x| vrai(x)).map(en_texte).unwrap_or_default());
    let ident: Vec<String> = ["nir", "nom", "prenom", "date_naissance", "lieu_naissance", "departement_naissance", "sexe", "nationalite"].iter().map(|k| champ(k)).collect();
    let fin = e.fin.map_or("None".to_string(), |f| f.iso());
    let spectacle = e.spectacle.clone().unwrap_or_else(|| "None".to_string());
    let brut = format!(
        "({}, {}, {}, ({}), {})",
        socle::fichiers::repr(&e.debut.iso()),
        socle::fichiers::repr(&fin),
        socle::fichiers::repr(&spectacle),
        ident.join(", "),
        socle::fichiers::repr(e.nature.as_deref().unwrap_or(""))
    );
    sha256_hex(brut.as_bytes())[..16].to_string()
}
