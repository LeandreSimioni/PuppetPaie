//! OU VIVENT LES ENGAGEMENTS : `engagements/AAAA-MM.json`, le fichier du mois ou chaque engagement commence. Un
//! engagement est UN objet, qu'il dure un jour ou six mois ; il n'est ecrit que la.
//!
//! Le fichier d'un mois : `{"mois", <ce que le mois dit par defaut>, "engagements": [...]}`. Les defauts du mois
//! (`taux_cachet`, `date_paiement`, `spectacle`, `lieu`) valent pour tous les engagements que le mois montre.
//!
//! Un engagement porte la saisie de son premier mois, telle que l'ecran l'ecrit (salarie, spectacle, dates, lignes,
//! nature, metier...), et, s'il deborde, `mois_suivants` : pour chaque mois d'apres que la saisie a touche, ce qu'il y
//! change (`null` : une valeur retiree). Ce que l'engagement dit d'un mois, sa VUE (`Objet::vue`), se lit de proche en
//! proche : le mois d'avant reconduit ce qui definit le contrat (`RECONDUITS`), le mois y ajoute ou y change ce qu'il
//! dit. Le salarie, les bornes et le numero du contrat sont ceux de l'engagement (`PARTAGES`), dans tous ses mois.
//!
//! LE MOIS DE L'ECRAN (`vue_du_mois`) montre les engagements qui le touchent : ceux de son fichier, et ceux des fichiers
//! d'avant qui courent encore. C'est ce que la saisie d'un mois a toujours ete (`{"mois", "engagements": [...]}`), et
//! ce que les calculs lisent ; l'ecran l'ecrit en entier (`ecrire_vue_du_mois`), et chaque engagement y retrouve le sien.
//!
//! L'identite d'un engagement : son salarie et son premier jour, et son numero de contrat quand il est pose, ou son rang
//! parmi ceux du meme salarie et du meme jour (`droit::jumeaux`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::date::{dernier_du_mois, en_texte, mois_entre, premier_du_mois, Jour};
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Le dossier des engagements, sous les donnees de la compagnie.
pub const DOSSIER: &str = "engagements";

/// Les mois d'apres que la saisie d'un engagement a touches, et ce que chacun change.
pub const SUITE: &str = "mois_suivants";

/// Ce que le moteur tient seul sur un engagement, jamais ce que l'ecran ecrit : sa suite, sa validation, pour qui son
/// numero de contrat a ete donne (`numeros`), les paies de
/// ses mois dont les documents sont partis pendant qu'on le modifie (`etats`).
pub const TENUS_PAR_LE_MOTEUR: [&str; 4] = [SUITE, "valide", "figees", "numero_pour"];

/// Ce que la vue d'un mois porte et que le moteur deduit : les jours du contrat entier (son premier mois et son
/// planning), que la DSN declare chaque mois (S21.G00.40.013). Il ne s'ecrit pas.
pub const DEDUITS: [&str; 1] = ["jours_contrat"];

/// Ce que tous les mois d'un engagement partagent : le salarie, les bornes du contrat, son numero.
pub const PARTAGES: [&str; 4] = ["salarie", "debut", "fin", "numero"];

/// Ce qu'un mois reconduit au suivant : ce qui definit le contrat (sa nature, son motif, sa duree du travail, son
/// salaire, son emploi), son spectacle, son lieu et son metier. Ni ses lignes (celles d'un contrat paye au mois se
/// composent chaque mois), ni ce qui n'appartient qu'a un mois (conges pris, absences, primes, rupture, date de
/// paiement, avenant date).
pub const RECONDUITS: [&str; 20] = [
    "salarie",
    "spectacle",
    "debut",
    "fin",
    "numero",
    "metier",
    "cadre",
    "lieu",
    "nature",
    "motif_recours",
    "quotite_contrat",
    "quotite_reference",
    "modalite_temps_travail",
    "salaire_mensuel",
    "emploi",
    "pcs_ese",
    "code_emploi_ft",
    "statut",
    "statut_conventionnel",
    "statut_categoriel_rc",
];

/// Le fichier des engagements qui commencent en `mois`.
pub fn chemin(racine: &Path, mois: &str) -> PathBuf {
    racine.join(DOSSIER).join(format!("{}.json", mois))
}

fn mois_valide(m: &str) -> bool {
    let b = m.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}

/// Les mois qui ont un fichier, du plus ancien au plus recent.
pub fn fichiers(racine: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(racine.join(DOSSIER))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").filter(|m| mois_valide(m)).map(String::from))
        .collect();
    v.sort();
    v
}

/// Le fichier d'un mois, lu tel quel ; `None` s'il n'existe pas.
pub fn lire_fichier(racine: &Path, mois: &str) -> Resultat<Option<Map<String, Value>>> {
    let p = chemin(racine, mois);
    let octets = match std::fs::read(&p) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(socle::fichiers::panne_fichier(&e, &p)),
    };
    let v: Value = serde_json::from_slice(&octets).map_err(|e| format!("{} : {}", p.display(), e))?;
    match v {
        Value::Object(m) => Ok(Some(m)),
        autre => Err(format!("{} : un objet est attendu, lu {}", p.display(), socle::valeur::nom_type(&autre))),
    }
}

/// Le mois suivant `AAAA-MM`.
pub fn mois_apres(mois: &str) -> Resultat<String> {
    Ok(premier_du_mois(mois)?.mois_suivant())
}

// ------------------------------------------------------------------ l'objet

/// Un engagement tel qu'il est ecrit : la compagnie, le mois de son fichier (son premier mois), son rang dans le
/// fichier, et l'objet lui-meme.
#[derive(Debug, Clone)]
pub struct Objet {
    pub racine: PathBuf,
    pub fichier: String,
    pub rang: usize,
    pub brut: Map<String, Value>,
}

/// Le debut d'un engagement brut : `debut` s'il le dit, sinon son premier jour joue.
fn debut_de(e: &Map<String, Value>) -> String {
    crate::debut_brut(&Value::Object(e.clone()))
}

impl Objet {
    /// Le premier mois de l'engagement : celui de son fichier.
    pub fn premier_mois(&self) -> &str {
        &self.fichier
    }

    /// Le dernier mois d'un engagement borne (celui de sa fin) ; `None` pour un contrat sans terme.
    pub fn dernier_mois(&self) -> Resultat<Option<String>> {
        match self.brut.get("fin").filter(|f| vrai(f)) {
            Some(f) => Ok(Some(socle::date::d(f)?.mois_iso().max(self.fichier.clone()))),
            None if self.brut.get("lignes").is_some_and(|l| !l.is_null()) && !self.mensuel() => {
                // Un contrat d'usage sans fin dite finit a son dernier jour joue ou planifie.
                let v = Value::Object(self.brut.clone());
                let mut jours: Vec<String> = crate::dates_engagement(&v)
                    .iter()
                    .chain(crate::planning_engagement(&v).iter())
                    .map(|j| match j {
                        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
                        autre => en_texte(autre),
                    })
                    .collect();
                jours.sort();
                Ok(jours.last().map(|j| j.chars().take(7).collect::<String>()).filter(|m| mois_valide(m)).map(|m| m.max(self.fichier.clone())))
            }
            None => Ok(None),
        }
    }

    /// Un contrat paye au mois ?
    pub fn mensuel(&self) -> bool {
        droit::natures::nature(self.brut.get("nature").and_then(Value::as_str)).is_ok_and(|n| n.mensuelle())
    }

    /// Les mois que la saisie a touches : le premier, puis ceux de `mois_suivants`.
    pub fn mois_saisis(&self) -> Vec<String> {
        let mut v = vec![self.fichier.clone()];
        if let Some(Value::Object(s)) = self.brut.get(SUITE) {
            v.extend(s.keys().filter(|m| m.as_str() > self.fichier.as_str()).cloned());
        }
        v.sort();
        v.dedup();
        v
    }

    /// L'engagement touche-t-il `mois` ? Du premier mois a celui de sa fin ; sans terme, de son premier mois a toujours.
    pub fn couvre(&self, mois: &str) -> Resultat<bool> {
        if mois < self.fichier.as_str() {
            return Ok(false);
        }
        Ok(match self.dernier_mois()? {
            Some(d) => mois <= d.as_str(),
            None => true,
        })
    }

    /// Les mois de l'engagement jusqu'a `jusqu_a` : un engagement borne les a tous ; un contrat sans terme, ceux que la
    /// saisie a touches et ceux qui vont jusqu'a `jusqu_a`.
    pub fn mois_couverts(&self, jusqu_a: &str) -> Resultat<Vec<String>> {
        let fin = match self.dernier_mois()? {
            Some(d) => d,
            None => self.mois_saisis().last().cloned().unwrap_or_default().max(jusqu_a.to_string()).max(self.fichier.clone()),
        };
        Ok(mois_entre(&premier_du_mois(&self.fichier)?, &dernier_du_mois(&fin)?))
    }

    /// Valide ?
    pub fn est_valide(&self) -> bool {
        self.brut.get("valide").is_some_and(Value::is_object)
    }

    /// La saisie du premier mois : l'objet sans ce que le moteur tient seul.
    fn premiere_vue(&self) -> Map<String, Value> {
        let mut v = self.brut.clone();
        for k in TENUS_PAR_LE_MOTEUR {
            v.remove(k);
        }
        v
    }

    /// Ce que l'engagement dit de `mois` : la saisie de ce mois-la, telle que l'ecran la montre et que le calcul la lit
    /// (`None` s'il ne le touche pas).
    pub fn vue(&self, mois: &str) -> Resultat<Option<Map<String, Value>>> {
        if !self.couvre(mois)? {
            return Ok(None);
        }
        let premiere = self.premiere_vue();
        let mut v = premiere.clone();
        let mut m = self.fichier.clone();
        while m.as_str() < mois {
            m = mois_apres(&m)?;
            v = reconduire(&self.racine, &v, self.mensuel(), &m);
            if let Some(change) = self.brut.get(SUITE).and_then(|s| s.get(&m)).and_then(Value::as_object) {
                appliquer(&mut v, change);
            }
        }
        for k in PARTAGES {
            match premiere.get(k) {
                Some(x) => v.insert(k.to_string(), x.clone()),
                None => v.remove(k),
            };
        }
        if mois > self.fichier.as_str() && !self.mensuel() {
            v.insert("jours_contrat".into(), Value::from(crate::jours_du_contrat(&Value::Object(premiere)) as i64));
        }
        Ok(Some(v))
    }
}

/// Ce qu'un mois reconduit au suivant, `mois` (`RECONDUITS`). Un contrat d'usage y commence avec les jours que les
/// lignes du mois d'avant ont mis au planning de `mois` : une ligne par ligne qui en a, a la meme rubrique, au meme
/// taux et au meme lieu, ses jours de `mois` en dates et ceux d'apres au planning ; un cachet, une journee ou un
/// service par date (une autre unite, des heures, ne se deduit pas d'un nombre de jours : quantite 0, a completer).
/// Sans planning, il commence sans ligne.
fn reconduire(racine: &Path, v: &Map<String, Value>, mensuel: bool, mois: &str) -> Map<String, Value> {
    let mut out: Map<String, Value> = RECONDUITS.iter().filter_map(|k| v.get(*k).map(|x| (k.to_string(), x.clone()))).collect();
    if mensuel {
        return out;
    }
    let jour_de = |x: &Value| match x {
        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
        autre => en_texte(autre),
    };
    let mut lignes = Vec::new();
    for l in v.get("lignes").and_then(Value::as_array).into_iter().flatten() {
        let plan: Vec<Value> = l.get("planning").and_then(Value::as_array).cloned().unwrap_or_default();
        let dedans: Vec<Value> = plan.iter().filter(|x| jour_de(x).starts_with(mois)).cloned().collect();
        let apres: Vec<Value> = plan.iter().filter(|x| jour_de(x).get(..7).is_some_and(|m| m > mois)).cloned().collect();
        if dedans.is_empty() && apres.is_empty() {
            continue;
        }
        let rubrique = l.get("rubrique").map(en_texte).unwrap_or_else(|| "cachet".into());
        let unite = droit::catalogue::rubrique(racine, &rubrique).map(|r| r.unite).unwrap_or_default();
        let par_date = matches!(unite.as_str(), "cachet" | "jour" | "service");
        let mut n = Map::new();
        n.insert("rubrique".into(), Value::String(rubrique));
        n.insert("taux".into(), l.get("taux").cloned().unwrap_or(Value::Null));
        if let Some(lieu) = l.get("lieu").filter(|x| vrai(x)) {
            n.insert("lieu".into(), lieu.clone());
        }
        n.insert("quantite".into(), if par_date { Value::from(dedans.len()) } else { Value::from(0) });
        if let Some(h) = l.get("heures").filter(|x| par_date && vrai(x)) {
            n.insert("heures".into(), h.clone());
        }
        n.insert("dates".into(), Value::Array(dedans));
        n.insert("planning".into(), Value::Array(apres));
        lignes.push(Value::Object(n));
    }
    out.insert("lignes".into(), Value::Array(lignes));
    out
}

/// Ce qu'un mois change : chaque cle remplacee, `null` retire.
fn appliquer(v: &mut Map<String, Value>, change: &Map<String, Value>) {
    for (k, x) in change {
        if x.is_null() {
            v.remove(k);
        } else {
            v.insert(k.clone(), x.clone());
        }
    }
}

/// Une valeur telle qu'elle se compare : sa forme canonique (`canonique`), sans les listes vides (une ligne sans
/// `planning` et une ligne au planning vide disent la meme chose).
fn comparable(v: &Value) -> String {
    fn sans_vides(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(m.iter().filter(|(_, x)| !x.as_array().is_some_and(Vec::is_empty)).map(|(k, x)| (k.clone(), sans_vides(x))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(sans_vides).collect()),
            autre => autre.clone(),
        }
    }
    canonique(&sans_vides(v))
}

/// Ce qui distingue `vue` de `base` : ce qui change ou s'ajoute, et `null` pour ce qui est retire.
fn difference(base: &Map<String, Value>, vue: &Map<String, Value>) -> Map<String, Value> {
    let mut d = Map::new();
    for (k, x) in vue {
        if base.get(k).map(comparable) != Some(comparable(x)) {
            d.insert(k.clone(), x.clone());
        }
    }
    for k in base.keys() {
        if !vue.contains_key(k) {
            d.insert(k.clone(), Value::Null);
        }
    }
    d
}

/// Tous les engagements, fichier par fichier, du plus ancien au plus recent, dans l'ordre de chaque fichier.
pub fn objets(racine: &Path) -> Resultat<Vec<Objet>> {
    let mut out = Vec::new();
    for f in fichiers(racine) {
        let Some(m) = lire_fichier(racine, &f)? else { continue };
        for (rang, e) in m.get("engagements").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if let Value::Object(brut) = e {
                out.push(Objet { racine: racine.to_path_buf(), fichier: f.clone(), rang, brut: brut.clone() });
            }
        }
    }
    Ok(out)
}

/// Les engagements qui touchent `mois`, dans l'ordre de l'ecran : ceux des fichiers d'avant qui courent encore, puis
/// ceux du mois.
pub fn objets_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    let mut out = Vec::new();
    for o in objets(racine)? {
        if o.fichier.as_str() <= mois && o.couvre(mois)? {
            out.push(o);
        }
    }
    Ok(out)
}

/// LE MOIS DE L'ECRAN : `{"mois", <defauts du mois>, "engagements": [la vue de chacun]}`, ou `None` quand aucun
/// engagement ne le touche et qu'il n'a pas de fichier. Un engagement valide y dit quand (`valide` : `{"le"}`).
pub fn vue_du_mois(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    let fichier = lire_fichier(racine, mois)?;
    let objets = objets_du_mois(racine, mois)?;
    if fichier.is_none() && objets.is_empty() {
        return Ok(None);
    }
    let mut data = fichier.unwrap_or_default();
    data.insert("mois".into(), Value::String(mois.to_string()));
    let mut engagements = Vec::new();
    for o in &objets {
        let Some(mut v) = o.vue(mois)? else { continue };
        if let Some(le) = o.brut.get("valide").and_then(|x| x.get("le")) {
            v.insert("valide".into(), serde_json::json!({ "le": le }));
        }
        engagements.push(Value::Object(v));
    }
    data.insert("engagements".into(), Value::Array(engagements));
    let vue = Value::Object(data);
    socle::fichiers::noter_valeur(&cle_de_saisie(mois), empreinte_de_saisie(&vue));
    Ok(Some(vue))
}

/// La cle sous laquelle un calcul note la saisie d'un mois qu'il a lue (`socle::fichiers::suivre`).
pub fn cle_de_saisie(mois: &str) -> String {
    format!("saisie {}", mois)
}

/// L'empreinte de la saisie d'un mois : ce qu'elle dit, sans ce que le moteur tient seul (la validation) ni le numero du
/// contrat (pose a l'envoi, il ne change aucun montant), les cles triees et un nombre ecrit comme un nombre. Deux parts,
/// `<le mois>:<ses engagements>` : ce que le mois pose pour tous (taux du cachet, date de paiement...), et ses
/// engagements ; la raison d'un recalcul dit laquelle a change (`etats::raison`).
pub fn empreinte_de_saisie(vue: &Value) -> String {
    let mut v = vue.clone();
    if let Some(Value::Array(es)) = v.get_mut("engagements") {
        for e in es.iter_mut() {
            if let Value::Object(m) = e {
                m.remove("valide");
                m.remove("numero");
            }
        }
    }
    let engagements = match v.as_object_mut() {
        Some(m) => {
            m.remove("_origine");
            m.remove("numeros_figes");
            m.remove("engagements").unwrap_or(Value::Null)
        }
        None => Value::Null,
    };
    let h = |x: &Value| socle::fichiers::sha256_hex(canonique(x).as_bytes());
    format!("{}:{}", h(&v), h(&engagements))
}

/// Une valeur ecrite sans ce qui ne change rien a ce qu'elle dit : les cles triees, un nombre ecrit comme un flottant
/// (`180` et `180.0` sont le meme taux : la page ecrit l'un, un fichier ancien l'autre).
pub fn canonique(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut cles: Vec<&String> = m.keys().collect();
            cles.sort();
            let corps: Vec<String> = cles.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), canonique(&m[*k]))).collect();
            format!("{{{}}}", corps.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonique).collect::<Vec<_>>().join(",")),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), |f| format!("{:?}", f)),
        Value::String(t) if !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit() || c == b'.' || c == b'-') && t.parse::<f64>().is_ok() => {
            format!("{:?}", t.parse::<f64>().unwrap_or_default())
        }
        autre => autre.to_string(),
    }
}

/// Les mois que la saisie a touches : les fichiers, et les mois que chaque engagement a saisis ou couvre jusqu'a sa fin ;
/// un contrat sans terme, jusqu'au dernier mois ecrit.
pub fn mois_saisis(racine: &Path) -> Resultat<Vec<String>> {
    let mut v = fichiers(racine);
    let horizon = v.iter().max().cloned();
    for o in objets(racine)? {
        v.extend(o.mois_saisis());
        let fin = match o.dernier_mois()? {
            Some(d) => Some(d),
            None => horizon.clone().filter(|h| h.as_str() >= o.fichier.as_str()),
        };
        if let Some(d) = fin {
            v.extend(mois_entre(&premier_du_mois(&o.fichier)?, &dernier_du_mois(&d)?));
        }
    }
    v.sort();
    v.dedup();
    Ok(v)
}

// ------------------------------------------------------------------ l'ecriture

/// L'identite d'un engagement brut du mois : (cle du salarie, premier jour, numero pose), `None` s'il ne nomme aucune
/// fiche.
fn identite(ctx: &Contexte, e: &Value) -> Option<droit::jumeaux::Ident> {
    let cle = crate::cle_de_l_engagement(ctx, e)?;
    Some((cle, crate::debut_brut(e), droit::jumeaux::numero_pose(e)))
}

/// Les engagements de l'ecran de `mois` (`vue`, la saisie entiere du mois), rendus a leurs objets : chacun retrouve le
/// sien (`droit::jumeaux::apparier_meme_numero`, puis, pour qui reste, le meme salarie, dans l'ordre), un nouveau
/// commence dans le fichier du mois, un engagement que l'ecran ne montre plus et qui commencait ce mois-ci s'efface.
///
/// UN ENGAGEMENT VALIDE NE CHANGE PAS PAR CE CHEMIN : il se modifie apres « Modifier » (`etats::modifier`), qui le
/// remet en saisie ; ce que l'ecran envoie pour lui ne compte pas, et il ne s'efface pas. Ce que l'ecran envoie de
/// `valide` ne compte jamais.
///
/// Refus, et rien n'est ecrit : un engagement dont les nouvelles bornes laisseraient hors du contrat un mois ou il a des
/// jours payes.
pub fn ecrire_vue_du_mois(ctx: &Contexte, mois: &str, vue: &Value) -> Resultat<Vec<PathBuf>> {
    let racine = &ctx.racine;
    let entrants: Vec<Value> = vue.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut tous = objets(racine)?;
    // Les engagements que le mois montre, par leur place dans `tous`.
    let mut du_mois: Vec<usize> = Vec::new();
    for (i, o) in tous.iter().enumerate() {
        if o.fichier.as_str() <= mois && o.couvre(mois)? {
            du_mois.push(i);
        }
    }
    let vues_actuelles: Vec<Value> = du_mois.iter().map(|i| tous[*i].vue(mois).map(|v| Value::Object(v.unwrap_or_default()))).collect::<Resultat<_>>()?;
    let ids_entrants: Vec<Option<droit::jumeaux::Ident>> = entrants.iter().map(|e| identite(ctx, e)).collect();
    let ids_actuels: Vec<Option<droit::jumeaux::Ident>> = vues_actuelles.iter().map(|e| identite(ctx, e)).collect();
    let mut paires = droit::jumeaux::apparier_meme_numero(&ids_entrants, &ids_actuels);
    // Le meme salarie, dont les dates ont change et qui n'a pas de numero : dans l'ordre, un a un.
    let mut pris: Vec<bool> = vec![false; ids_actuels.len()];
    for j in paires.iter().flatten() {
        pris[*j] = true;
    }
    for (i, p) in paires.iter_mut().enumerate() {
        if p.is_some() {
            continue;
        }
        let Some(x) = &ids_entrants[i] else { continue };
        if let Some(j) = (0..ids_actuels.len()).find(|j| !pris[*j] && ids_actuels[*j].as_ref().is_some_and(|y| y.0 == x.0 && (x.2.is_none() || y.2.is_none()))) {
            *p = Some(j);
            pris[j] = true;
        }
    }
    let mut touches: Vec<String> = vec![mois.to_string()];
    let mut nouveaux: Vec<Map<String, Value>> = Vec::new();
    for (i, e) in entrants.iter().enumerate() {
        let Value::Object(e) = e else { continue };
        let mut saisie = e.clone();
        for k in TENUS_PAR_LE_MOTEUR.iter().chain(DEDUITS.iter()).chain(["rouverte"].iter()) {
            saisie.remove(*k);
        }
        match paires[i] {
            Some(j) => {
                let o = &mut tous[du_mois[j]];
                if o.est_valide() {
                    continue;
                }
                if !saisie.get("numero").is_some_and(vrai) {
                    if let Some(n) = o.brut.get("numero").filter(|n| vrai(n)).cloned() {
                        saisie.insert("numero".into(), n);
                    }
                }
                // Un spectacle se nomme par la cle de sa fiche, son titre ou son numero d'objet : le meme, sous un autre
                // nom, garde celui qui est ecrit.
                if let Some(ecrit) = vues_actuelles[j].get("spectacle").filter(|s| vrai(s)) {
                    if saisie.get("spectacle").is_some_and(|s| s != ecrit && meme_spectacle(ctx, s, ecrit)) {
                        saisie.insert("spectacle".into(), ecrit.clone());
                    }
                }
                poser_le_mois(o, mois, &saisie)?;
                touches.push(o.fichier.clone());
            }
            None => nouveaux.push(saisie),
        }
    }
    // Ce que l'ecran ne montre plus : un engagement de ce mois-ci, pas encore valide, s'efface ; un engagement d'un
    // mois d'avant revient a ce que ses mois d'avant reconduisent.
    let mut effaces: Vec<(String, usize)> = Vec::new();
    for (j, i) in du_mois.iter().enumerate() {
        if pris[j] || paires.contains(&Some(j)) {
            continue;
        }
        let o = &mut tous[*i];
        if o.est_valide() {
            continue;
        }
        if o.fichier == mois {
            effaces.push((o.fichier.clone(), o.rang));
        } else if let Some(Value::Object(s)) = o.brut.get_mut(SUITE) {
            if s.remove(mois).is_some() {
                touches.push(o.fichier.clone());
            }
        }
    }
    touches.sort();
    touches.dedup();
    // Les fichiers touches, recomposes : les objets dans leur ordre, moins les effaces, plus les nouveaux au mois.
    let mut ecrits = Vec::new();
    for f in &touches {
        let mut contenu = lire_fichier(racine, f)?.unwrap_or_default();
        if f == mois {
            if let Value::Object(m) = vue {
                for (k, x) in m {
                    if k != "engagements" {
                        contenu.insert(k.clone(), x.clone());
                    }
                }
            }
            contenu.insert("mois".into(), Value::String(mois.to_string()));
        }
        let mut es: Vec<Value> = tous
            .iter()
            .filter(|o| &o.fichier == f && !effaces.contains(&(o.fichier.clone(), o.rang)))
            .map(|o| Value::Object(o.brut.clone()))
            .collect();
        if f == mois {
            es.extend(nouveaux.iter().cloned().map(Value::Object));
        }
        contenu.insert("engagements".into(), Value::Array(es));
        let p = chemin(racine, f);
        sauver_avant(&p)?;
        socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)?;
        ecrits.push(p);
    }
    Ok(ecrits)
}

/// `a` et `b` nomment-ils la meme fiche de spectacle (`Contexte::spectacle`) ?
fn meme_spectacle(ctx: &Contexte, a: &Value, b: &Value) -> bool {
    let cle = |v: &Value| ctx.spectacle(&en_texte(v)).ok().and_then(|f| f.get("_cle").cloned());
    cle(a).is_some_and(|x| Some(x) == cle(b))
}

/// Pose la saisie que l'ecran montre pour `mois` sur l'objet `o` : son premier mois s'il commence en `mois`, sinon ce
/// que `mois` change. Les mois saisis apres lui gardent la vue qu'ils avaient ; ce que tous les mois partagent
/// (`PARTAGES`) va a l'objet.
fn poser_le_mois(o: &mut Objet, mois: &str, saisie: &Map<String, Value>) -> Resultat<()> {
    // Les vues des mois saisis, avant : elles se gardent.
    let saisis: Vec<String> = o.mois_saisis().into_iter().filter(|m| m.as_str() != mois && m.as_str() != o.fichier.as_str()).collect();
    let mut gardees: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for m in &saisis {
        if let Some(v) = o.vue(m)? {
            gardees.insert(m.clone(), v);
        }
    }
    for k in PARTAGES {
        match saisie.get(k).filter(|x| !x.is_null()) {
            Some(x) => o.brut.insert(k.to_string(), x.clone()),
            None if k == "numero" => None,
            None => o.brut.remove(k),
        };
    }
    if mois == o.fichier {
        let suite = o.brut.remove(SUITE);
        let valide = o.brut.remove("valide");
        let figees = o.brut.remove("figees");
        let mut neuf = saisie.clone();
        for k in PARTAGES {
            match o.brut.get(k) {
                Some(x) => neuf.insert(k.to_string(), x.clone()),
                None => neuf.remove(k),
            };
        }
        o.brut = neuf;
        for (k, v) in [(SUITE, suite), ("valide", valide), ("figees", figees)] {
            if let Some(v) = v {
                o.brut.insert(k.to_string(), v);
            }
        }
    } else {
        changer_un_mois(o, mois, saisie)?;
    }
    // Les bornes ne laissent pas hors du contrat un mois ou il a des jours payes.
    let mut a_retirer = Vec::new();
    for (m, v) in &gardees {
        if !o.couvre(m)? {
            if v.get("lignes").and_then(Value::as_array).is_some_and(|l| !l.is_empty()) {
                return Err(format!(
                    "Le contrat irait du {} au {}, et {} y porte des jours payes : un mois de paie ne tient pas un contrat \
                     qui ne le couvre pas. Rien n'est ecrit. Retirer d'abord les lignes de {}, ou garder des bornes qui le \
                     couvrent.",
                    o.brut.get("debut").map(en_texte).unwrap_or_default(),
                    o.brut.get("fin").map(en_texte).unwrap_or_else(|| "?".into()),
                    crate::libelle_mois(m),
                    crate::libelle_mois(m)
                ));
            }
            a_retirer.push(m.clone());
        }
    }
    for (m, v) in gardees {
        if a_retirer.contains(&m) || m.as_str() <= mois {
            continue;
        }
        changer_un_mois(o, &m, &v)?;
    }
    if let Some(Value::Object(s)) = o.brut.get_mut(SUITE) {
        for m in &a_retirer {
            s.remove(m);
        }
        let vides: Vec<String> = s.iter().filter(|(_, v)| v.as_object().is_some_and(Map::is_empty)).map(|(k, _)| k.clone()).collect();
        for m in vides {
            s.remove(&m);
        }
    }
    if o.brut.get(SUITE).and_then(Value::as_object).is_some_and(Map::is_empty) {
        o.brut.remove(SUITE);
    }
    Ok(())
}

/// Ecrit dans la suite de `o` ce que `mois` change a ce que le mois d'avant reconduit, pour que sa vue soit `saisie`.
fn changer_un_mois(o: &mut Objet, mois: &str, saisie: &Map<String, Value>) -> Resultat<()> {
    let precedent = mois_avant(mois)?;
    let base = match o.vue(&precedent)? {
        Some(v) => reconduire(&o.racine, &v, o.mensuel(), mois),
        None => Map::new(),
    };
    let mut cible = saisie.clone();
    for k in PARTAGES.iter().chain(DEDUITS.iter()) {
        cible.remove(*k);
    }
    let mut base_sans = base;
    for k in PARTAGES {
        base_sans.remove(k);
    }
    let d = difference(&base_sans, &cible);
    let suite = o.brut.entry(SUITE.to_string()).or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(s) = suite {
        s.insert(mois.to_string(), Value::Object(d));
        let mut trie: Vec<(String, Value)> = std::mem::take(s).into_iter().collect();
        trie.sort_by(|a, b| a.0.cmp(&b.0));
        s.extend(trie);
    }
    Ok(())
}

/// Le mois qui precede `AAAA-MM`.
pub fn mois_avant(mois: &str) -> Resultat<String> {
    let j = premier_du_mois(mois)?.plus_jours(-1)?;
    Ok(j.mois_iso())
}

/// L'engagement que designe, dans le mois `mois`, le rang `rang` de son ecran.
pub fn objet_du_mois(racine: &Path, mois: &str, rang: usize) -> Resultat<Option<Objet>> {
    Ok(objets_du_mois(racine, mois)?.into_iter().nth(rang))
}

/// Remplace l'objet `o` dans son fichier (meme fichier, meme rang), tel qu'il est maintenant.
pub fn reecrire(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = lire_fichier(racine, &o.fichier)?.ok_or_else(|| format!("{} : le fichier de l'engagement a disparu", o.fichier))?;
    let Some(Value::Array(es)) = contenu.get_mut("engagements") else {
        return Err(format!("{} : `engagements` manque", o.fichier));
    };
    let place = es.get_mut(o.rang).ok_or_else(|| format!("{} : l'engagement {} a disparu", o.fichier, o.rang + 1))?;
    *place = Value::Object(o.brut.clone());
    let p = chemin(racine, &o.fichier);
    socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)?;
    Ok(p)
}

/// Combien de copies d'un fichier `avant/` garde.
const GARDER_COPIES: usize = 20;

/// Une copie du fichier dans `engagements/avant/`, horodatee, avant de le remplacer : on revient a la saisie d'avant
/// si une ecriture l'a abimee. Les plus anciennes au-dela de `GARDER_COPIES` s'effacent.
fn sauver_avant(chemin: &Path) -> Resultat<()> {
    if !chemin.exists() {
        return Ok(());
    }
    let io = |e: std::io::Error| e.to_string();
    let dossier = chemin.parent().unwrap_or(Path::new(".")).join("avant");
    std::fs::create_dir_all(&dossier).map_err(io)?;
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let horodatage = socle::paris::iso_secondes(socle::temps::secondes()).replace(['-', ':'], "").replace('T', "-");
    std::fs::copy(chemin, dossier.join(format!("{}.{}", nom, horodatage))).map_err(io)?;
    let prefixe = format!("{}.", nom);
    let mut anciennes: Vec<String> = std::fs::read_dir(&dossier)
        .map_err(io)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefixe))
        .collect();
    anciennes.sort();
    while anciennes.len() > GARDER_COPIES {
        let _ = std::fs::remove_file(dossier.join(anciennes.remove(0)));
    }
    Ok(())
}

/// Le debut d'un engagement de l'ecran, tel que son identite le lit (pour qui compare deux vues).
pub fn debut_vu(e: &Map<String, Value>) -> String {
    debut_de(e)
}

/// Le jour `AAAA-MM-JJ` de `mois` qui est le premier, pour les bornes d'un contrat.
pub fn premier_jour(mois: &str) -> Resultat<Jour> {
    premier_du_mois(mois)
}

/// Le mois a-t-il une saisie : un fichier, ou un engagement qui le touche ?
pub fn a_une_saisie(racine: &Path, mois: &str) -> Resultat<bool> {
    if chemin(racine, mois).is_file() {
        return Ok(true);
    }
    for o in objets(racine)? {
        if o.fichier.as_str() <= mois && o.couvre(mois)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Pose `cle` = `valeur` sur le mois `mois` lui-meme (son fichier, cree s'il le faut : un mois que seuls des engagements
/// d'avant touchent) : ce que le mois dit de lui (`numeros_figes`...).
pub fn poser_au_mois(racine: &Path, mois: &str, cle: &str, valeur: Value) -> Resultat<()> {
    let mut contenu = lire_fichier(racine, mois)?.unwrap_or_default();
    if contenu.get(cle) == Some(&valeur) {
        return Ok(());
    }
    contenu.insert("mois".into(), Value::String(mois.to_string()));
    contenu.entry("engagements").or_insert_with(|| Value::Array(Vec::new()));
    contenu.insert(cle.to_string(), valeur);
    let p = chemin(racine, mois);
    sauver_avant(&p)?;
    socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)
}
