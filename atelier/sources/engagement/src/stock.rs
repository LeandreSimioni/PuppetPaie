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

/// Ce que le moteur tient seul sur un engagement, jamais ce que l'ecran ecrit : sa suite, sa validation, la validation
/// qu'une modification va remplacer, ses versions passees (`etats`), pour qui son numero de contrat a ete donne
/// (`numeros`), qu'il n'est plus actif (`NON_ACTIF`), ce qui lui manque (`A_COMPLETER`), ce que l'import a lu de lui
/// (`importe::IMPORTE`).
pub const TENUS_PAR_LE_MOTEUR: [&str; 8] =
    [SUITE, "valide", "precedente", "versions", "numero_pour", NON_ACTIF, A_COMPLETER, crate::importe::IMPORTE];

/// UN ENGAGEMENT EN SAISIE AUQUEL IL MANQUE UN FAIT (un salaire, une duree du travail, une anciennete, un spectacle,
/// des jours) : `{"mois", "manque"}`, le mois dont l'ecriture l'a trouve incomplet et ce que le calcul en dit ; l'ecran
/// le montre avec ce qui lui manque (`vue_a_l_ecran`), et sa validation dit ce qui manque. Seul un engagement jamais
/// valide le porte (un engagement valide ne change que d'un geste qui le revalide, `etats::modifier`) : il est ecrit
/// tel que l'ecran l'a saisi et ne touche aucun mois (`Objet::couvre`) : rien ne le calcule, rien ne part de lui.
pub const A_COMPLETER: &str = "a_completer";

/// Dans un engagement qui n'est plus actif (`etats::desactiver`) : `{"le", "version"}`, quand, et sa version (celle qui
/// suit la derniere validee : rien de paye). Il ne touche plus aucun mois (`Objet::couvre`) : il ne se calcule plus,
/// n'entre dans aucun document ni aucune declaration ; il se lit (`non_actifs_du_mois`).
pub const NON_ACTIF: &str = "non_actif";

/// Ce que la vue d'un mois porte et que le moteur deduit : les jours du contrat entier (son premier mois et son
/// planning), que la DSN declare chaque mois (S21.G00.40.013). Il ne s'ecrit pas.
pub const DEDUITS: [&str; 1] = ["jours_contrat"];

/// Ce que tous les mois d'un engagement partagent : le salarie, les bornes du contrat, son numero.
pub const PARTAGES: [&str; 4] = ["salarie", "debut", "fin", "numero"];

/// Ce qu'un mois reconduit au suivant : ce qui definit le contrat (sa nature, son motif, sa duree du travail, son
/// salaire, son emploi), son spectacle, son lieu et son metier. Ni ses lignes (celles d'un contrat paye au mois se
/// composent chaque mois), ni ce qui n'appartient qu'a un mois (conges pris, absences, primes, rupture, date de
/// paiement, avenant date).
pub const RECONDUITS: [&str; 21] = [
    "salarie",
    "spectacle",
    "debut",
    "fin",
    "numero",
    "metier",
    "cadre",
    "lieu",
    "etablissement",
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
    Ok(lire_partage(racine, mois)?.map(|v| v.as_object().cloned().unwrap_or_default()))
}

/// Le fichier d'un mois tel que le processus l'a lu (`socle::fichiers::json_partage`) : chaque lecture des engagements
/// passe par lui, et il ne se relit que quand il a change. `None` s'il n'existe pas.
fn lire_partage(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<Value>>> {
    let p = chemin(racine, mois);
    match socle::fichiers::json_partage(&p)? {
        Some(v) if !v.is_object() => Err(format!("{} : un objet est attendu, lu {}", p.display(), socle::valeur::nom_type(&v))),
        v => Ok(v),
    }
}

/// Le fichier de `mois` ne porte-t-il que des engagements importes (`importe`) ? Un tel mois n'a pas de saisie : il se
/// lit. Un fichier illisible ou sans engagement n'est pas de l'import.
fn importes_seuls(racine: &Path, mois: &str) -> bool {
    let Ok(Some(f)) = lire_partage(racine, mois) else { return false };
    let es = f.get("engagements").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    !es.is_empty() && es.iter().all(|e| e.get(crate::importe::IMPORTE).is_some_and(Value::is_object))
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
            None if !self.mensuel() => {
                // Un contrat d'usage sans fin dite finit a son dernier jour joue ou planifie ; sans jour encore, a son
                // premier mois : seul un contrat paye au mois court sans terme.
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
                let dernier = jours.last().map(|j| j.chars().take(7).collect::<String>()).filter(|m| mois_valide(m));
                Ok(Some(dernier.map_or(self.fichier.clone(), |m| m.max(self.fichier.clone()))))
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
    /// Un engagement non actif ne touche plus aucun mois ; un engagement importe n'en touche aucun de la saisie : il se
    /// lit (`importe`), rien ne le calcule.
    pub fn couvre(&self, mois: &str) -> Resultat<bool> {
        Ok(!self.est_a_completer() && self.a_l_ecran(mois)?)
    }

    /// L'ecran de `mois` le montre-t-il ? Ceux qu'il touche (`couvre`), et ceux auxquels il manque un fait
    /// (`A_COMPLETER`), qui se completent la.
    pub fn a_l_ecran(&self, mois: &str) -> Resultat<bool> {
        Ok(!self.est_non_actif() && !self.est_importe() && self.dans_ses_bornes(mois)?)
    }

    /// Lui manque-t-il un fait (`A_COMPLETER`) ? Ce qui manque, alors.
    pub fn a_completer(&self) -> Option<String> {
        self.brut.get(A_COMPLETER)?.get("manque").map(en_texte)
    }

    /// Hors de toutes les lectures du mois : un engagement auquel il manque un fait (`A_COMPLETER`).
    pub fn est_a_completer(&self) -> bool {
        self.brut.get(A_COMPLETER).is_some_and(Value::is_object)
    }

    /// Est-il importe (`importe::IMPORTE`) ?
    pub fn est_importe(&self) -> bool {
        self.brut.get(crate::importe::IMPORTE).is_some_and(Value::is_object)
    }

    /// N'est-il plus actif (`NON_ACTIF`) ?
    pub fn est_non_actif(&self) -> bool {
        self.brut.get(NON_ACTIF).is_some_and(Value::is_object)
    }

    /// `mois` est-il entre son premier mois et celui de sa fin, qu'il soit actif ou non ?
    fn dans_ses_bornes(&self, mois: &str) -> Resultat<bool> {
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
    /// (`None` s'il ne le touche pas). Un engagement non actif dit encore ce qu'il etait, pour qu'il se lise.
    pub fn vue(&self, mois: &str) -> Resultat<Option<Map<String, Value>>> {
        if !self.dans_ses_bornes(mois)? {
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
            // Un contrat d'usage qui ne dit pas son debut commence a son premier jour joue, celui de son premier mois :
            // les mois suivants le declarent (S21.G00.40.001) et ne le prennent pas pour une embauche.
            let debut = crate::debut_brut(&Value::Object(premiere.clone()));
            if !v.contains_key("debut") && !debut.is_empty() {
                v.insert("debut".into(), Value::String(debut));
            }
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
        let unite = socle::date::dernier_du_mois(mois).and_then(|j| droit::catalogue::rubrique(racine, &rubrique, &j)).map(|r| r.unite).unwrap_or_default();
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
    Ok(tous(racine)?.1.objets.clone())
}

/// Les fichiers des engagements d'une compagnie tels qu'ils viennent d'etre lus : chaque mois, et ce que son fichier
/// contient (`socle::fichiers::json_partage` : la meme valeur tant que les octets sont les memes).
type Lus = Vec<(String, std::sync::Arc<Value>)>;

/// Deux lectures ont-elles trouve exactement les memes fichiers, aux memes octets ?
fn memes(a: &Lus, b: &Lus) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((m, x), (n, y))| m == n && std::sync::Arc::ptr_eq(x, y))
}

fn lus(racine: &Path) -> Resultat<Lus> {
    let mut out = Vec::new();
    for f in fichiers(racine) {
        if let Some(v) = lire_partage(racine, &f)? {
            out.push((f, v));
        }
    }
    Ok(out)
}

/// Les engagements d'une compagnie tires de ses fichiers, et les bornes de chacun (son dernier mois), tirees avec lui :
/// lire un mois ne relit que les objets qui le touchent.
struct Tires {
    objets: Vec<Objet>,
    derniers: Vec<Result<Option<String>, String>>,
}

impl Tires {
    /// `mois` est-il entre le premier et le dernier mois de l'objet de rang `i` ?
    fn dans_ses_bornes(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        if mois < o.fichier.as_str() {
            return Ok(false);
        }
        Ok(match &self.derniers[i] {
            Ok(Some(d)) => mois <= d.as_str(),
            Ok(None) => true,
            Err(e) => return Err(e.clone()),
        })
    }

    /// L'objet de rang `i` touche-t-il `mois` (`Objet::couvre`) ?
    fn couvre(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        Ok(!o.est_non_actif() && !o.est_importe() && self.dans_ses_bornes(i, mois)?)
    }

    /// Les objets qui touchent `mois`, dans l'ordre des fichiers ; `ecran` : avec ceux auxquels il manque un fait
    /// (`Objet::a_l_ecran`).
    fn du_mois(&self, mois: &str, ecran: bool) -> Resultat<Vec<&Objet>> {
        let mut out = Vec::new();
        for (i, o) in self.objets.iter().enumerate() {
            if o.fichier.as_str() <= mois && (ecran || !o.est_a_completer()) && self.couvre(i, mois)? {
                out.push(o);
            }
        }
        Ok(out)
    }
}

/// TOUS LES ENGAGEMENTS D'UNE COMPAGNIE, tires une fois des memes fichiers : chaque lecture relit les fichiers, et ce
/// qu'on en avait tire ne sert que si leurs octets sont exactement les memes (`memes`). Partage : on n'y touche pas, on
/// recopie ce qu'on garde.
fn tous(racine: &Path) -> Resultat<(Lus, std::sync::Arc<Tires>)> {
    type Gardes = std::sync::Mutex<std::collections::HashMap<PathBuf, (Lus, std::sync::Arc<Tires>)>>;
    static GARDES: std::sync::OnceLock<Gardes> = std::sync::OnceLock::new();
    let gardes = GARDES.get_or_init(Default::default);
    let fichiers = lus(racine)?;
    if let Some((vus, v)) = gardes.lock().unwrap_or_else(|p| p.into_inner()).get(racine) {
        if memes(vus, &fichiers) {
            return Ok((fichiers, v.clone()));
        }
    }
    let mut objets = Vec::new();
    for (f, m) in &fichiers {
        for (rang, e) in m.get("engagements").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if let Value::Object(brut) = e {
                objets.push(Objet { racine: racine.to_path_buf(), fichier: f.clone(), rang, brut: brut.clone() });
            }
        }
    }
    let derniers = objets.iter().map(Objet::dernier_mois).collect();
    let out = std::sync::Arc::new(Tires { objets, derniers });
    socle::fichiers::ranger(gardes, racine.to_path_buf(), (fichiers.clone(), out.clone()));
    Ok((fichiers, out))
}

/// Les engagements qui touchent `mois`, dans l'ordre de l'ecran : ceux des fichiers d'avant qui courent encore, puis
/// ceux du mois.
pub fn objets_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.du_mois(mois, false)?.into_iter().cloned().collect())
}

/// Les engagements que l'ecran de `mois` montre, dans son ordre (`vue_a_l_ecran`) : ceux qui le touchent et ceux
/// auxquels il manque un fait.
pub fn objets_a_l_ecran(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.du_mois(mois, true)?.into_iter().cloned().collect())
}

/// Les engagements non actifs (`NON_ACTIF`) qui toucheraient `mois` s'ils l'etaient encore : ce qui se lit d'eux.
pub fn non_actifs_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    let t = tous(racine)?.1;
    let mut out = Vec::new();
    for (i, o) in t.objets.iter().enumerate() {
        if o.est_non_actif() && o.fichier.as_str() <= mois && t.dans_ses_bornes(i, mois)? {
            out.push(o.clone());
        }
    }
    Ok(out)
}

/// Efface l'objet `o` de son fichier (meme fichier, meme rang), apres une copie dans `avant/`.
pub fn effacer(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = lire_fichier(racine, &o.fichier)?.ok_or_else(|| format!("{} : le fichier de l'engagement a disparu", o.fichier))?;
    let Some(Value::Array(es)) = contenu.get_mut("engagements") else {
        return Err(format!("{} : `engagements` manque", o.fichier));
    };
    if o.rang >= es.len() || es[o.rang] != Value::Object(o.brut.clone()) {
        return Err(format!("{} : l'engagement {} a change sur le disque ; rien n'est efface", o.fichier, o.rang + 1));
    }
    es.remove(o.rang);
    let p = chemin(racine, &o.fichier);
    sauver_avant(&p)?;
    socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)?;
    Ok(p)
}

/// LE MOIS DE L'ECRAN : `{"mois", <defauts du mois>, "engagements": [la vue de chacun]}`, ou `None` quand aucun
/// engagement ne le touche et qu'il n'a pas de fichier. Un engagement valide y dit quand (`valide` : `{"le"}`).
pub fn vue_du_mois(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    Ok(saisie_du_mois(racine, mois, false)?.map(|s| {
        socle::fichiers::noter_valeur(&cle_de_saisie(mois), s.1.clone());
        s.0.clone()
    }))
}

/// LE MOIS TEL QUE L'ECRAN LE MONTRE ET LE SAISIT : `vue_du_mois`, et, a leur place, les engagements auxquels il manque
/// un fait (`A_COMPLETER`), chacun avec `a_completer` : ce qui lui manque. Rien ne se calcule sur elle : un calcul lit
/// `vue_du_mois`.
pub fn vue_a_l_ecran(racine: &Path, mois: &str) -> Resultat<Option<Value>> {
    Ok(saisie_du_mois(racine, mois, true)?.map(|s| s.0.clone()))
}

/// L'empreinte de la saisie de `mois` (`empreinte_de_saisie`), celle que `vue_du_mois` note ; `None` sans saisie.
pub fn empreinte_de_la_saisie_du_mois(racine: &Path, mois: &str) -> Resultat<Option<String>> {
    Ok(saisie_du_mois(racine, mois, false)?.map(|s| s.1.clone()))
}

/// La vue de `mois` et son empreinte, faites une fois pour les memes fichiers aux memes octets (`memes`) :
/// chaque paie qui se verifie relit la saisie des mois qu'elle a lus. La vue d'une ligne lit aussi l'unite de sa rubrique
/// (`droit::catalogue::rubrique`) : les deux catalogues en font partie.
fn saisie_du_mois(racine: &Path, mois: &str, ecran: bool) -> Resultat<Option<std::sync::Arc<(Value, String)>>> {
    type Saisie = Option<std::sync::Arc<(Value, String)>>;
    type Vues = std::sync::Mutex<std::collections::HashMap<(PathBuf, String, bool), (Lus, Saisie)>>;
    static VUES: std::sync::OnceLock<Vues> = std::sync::OnceLock::new();
    static ABSENT: std::sync::OnceLock<std::sync::Arc<Value>> = std::sync::OnceLock::new();
    let vues = VUES.get_or_init(Default::default);
    let (mut lus, tous) = tous(racine)?;
    for p in [socle::fichiers::dossier_du_code().join("rubriques-base.json"), racine.join("config").join(droit::catalogue::RUBRIQUES_COMPAGNIE)] {
        // Absent, il est le meme d'une lecture a l'autre ; illisible, il ne l'est jamais (la vue se refait).
        let v = match socle::fichiers::json_partage(&p) {
            Ok(Some(v)) => v,
            Ok(None) => ABSENT.get_or_init(|| std::sync::Arc::new(Value::Null)).clone(),
            Err(_) => std::sync::Arc::new(Value::Null),
        };
        lus.push((p.to_string_lossy().into_owned(), v));
    }
    let cle = (racine.to_path_buf(), mois.to_string(), ecran);
    if let Some((vus, s)) = vues.lock().unwrap_or_else(|p| p.into_inner()).get(&cle) {
        if memes(vus, &lus) {
            return Ok(s.clone());
        }
    }
    // Ce que le fichier du mois pose pour tous ; ses engagements, la vue les refait (ceux des mois d'avant qui courent).
    let fichier: Option<Map<String, Value>> = lire_partage(racine, mois)?
        .map(|v| v.as_object().into_iter().flatten().map(|(k, x)| (k.clone(), if k == "engagements" { Value::Null } else { x.clone() })).collect());
    let objets = tous.du_mois(mois, ecran)?;
    let saisie = if (fichier.is_none() || importes_seuls(racine, mois)) && objets.is_empty() {
        None
    } else {
        let mut data = fichier.unwrap_or_default();
        data.insert("mois".into(), Value::String(mois.to_string()));
        let mut engagements = Vec::new();
        for o in &objets {
            let vu = o.vue(mois)?;
            let Some(mut v) = vu else { continue };
            if let Some(le) = o.brut.get("valide").and_then(|x| x.get("le")) {
                v.insert("valide".into(), serde_json::json!({ "le": le }));
            }
            if let Some(manque) = o.a_completer().filter(|_| ecran) {
                v.insert(A_COMPLETER.into(), Value::String(manque));
            }
            engagements.push(Value::Object(v));
        }
        data.insert("engagements".into(), Value::Array(engagements));
        let vue = Value::Object(data);
        let empreinte = empreinte_de_saisie(&vue);
        Some(std::sync::Arc::new((vue, empreinte)))
    };
    socle::fichiers::ranger(vues, cle, (lus, saisie.clone()));
    Ok(saisie)
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
/// un contrat sans terme, jusqu'au dernier mois ecrit ou jusqu'au mois d'aujourd'hui s'il est plus loin : un CDI de
/// janvier que rien n'a saisi depuis a une paie, une DSN et des documents chaque mois qu'il court.
pub fn mois_saisis(racine: &Path) -> Resultat<Vec<String>> {
    let mut v: Vec<String> = fichiers(racine).into_iter().filter(|m| !importes_seuls(racine, m)).collect();
    let courant = droit::contexte::aujourdhui_sans_note()?.mois_iso();
    let horizon = v.iter().max().cloned().map(|h| h.max(courant));
    for o in objets(racine)?.into_iter().filter(|o| !o.est_importe()) {
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
/// sien (`droit::jumeaux::apparier_meme_numero`, puis, pour qui reste, le meme salarie pas encore valide, dans l'ordre), un nouveau
/// commence dans le fichier du mois, un engagement que l'ecran ne montre plus et qui commencait ce mois-ci s'efface.
///
/// UN ENGAGEMENT VALIDE NE CHANGE PAS PAR CE CHEMIN : il se modifie d'un geste, remis en saisie (`etats::modifier`) puis
/// sa saisie posee (`poser_la_saisie`) et revalide ; ce que l'ecran envoie pour lui ne compte pas, et il ne s'efface pas. Ce que l'ecran envoie de
/// `valide` ne compte jamais.
///
/// `manques` : pour chaque engagement de la vue, dans son ordre, ce qui lui manque pour se calculer (`None` : rien) ;
/// l'engagement s'ecrit avec (`A_COMPLETER`). Un engagement complet en `mois` perd ce que `mois` lui faisait manquer.
///
/// Refus, et rien n'est ecrit : un engagement dont les nouvelles bornes laisseraient hors du contrat un mois ou il a des
/// jours payes ; un engagement du salarie et du premier jour d'un engagement importe qui touche `mois` (il se lit, il ne
/// se reecrit pas ni ne se double).
pub fn ecrire_vue_du_mois(ctx: &Contexte, mois: &str, vue: &Value, manques: &[Option<String>]) -> Resultat<Vec<PathBuf>> {
    let racine = &ctx.racine;
    let entrants: Vec<Value> = vue.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    for e in &entrants {
        let Some(cle) = crate::cle_de_l_engagement(ctx, e) else { continue };
        let debut = crate::debut_brut(e);
        if crate::importe::celui_de(racine, mois, &cle, &debut)?.is_some() {
            return Err(crate::importe::refus(&e.get("salarie").map(en_texte).unwrap_or(cle), &debut));
        }
    }
    let mut tous = objets(racine)?;
    // Les engagements que le mois montre, par leur place dans `tous`.
    let mut du_mois: Vec<usize> = Vec::new();
    for (i, o) in tous.iter().enumerate() {
        if o.fichier.as_str() <= mois && o.a_l_ecran(mois)? {
            du_mois.push(i);
        }
    }
    // Telles que l'ecran les montre : c'est elles qu'il renvoie.
    let vues_actuelles: Vec<Value> =
        du_mois.iter().map(|i| tous[*i].vue(mois).map(|v| Value::Object(v.unwrap_or_default()))).collect::<Resultat<_>>()?;
    let ids_entrants: Vec<Option<droit::jumeaux::Ident>> = entrants.iter().map(|e| identite(ctx, e)).collect();
    let ids_actuels: Vec<Option<droit::jumeaux::Ident>> = vues_actuelles.iter().map(|e| identite(ctx, e)).collect();
    let mut paires = droit::jumeaux::apparier_meme_numero(&ids_entrants, &ids_actuels);
    // Le meme salarie, dont les dates ont change et qui n'a pas de numero : dans l'ordre, un a un. Jamais un engagement
    // valide : ses dates ne changent pas par ce chemin, l'ecran le renvoie tel qu'il le montre ; un engagement du meme
    // salarie a d'autres dates est un autre engagement (un cachet a cote d'un CDD), qui s'ecrit.
    let mut pris: Vec<bool> = vec![false; ids_actuels.len()];
    for j in paires.iter().flatten() {
        pris[*j] = true;
    }
    let valides: Vec<bool> = du_mois.iter().map(|i| tous[*i].est_valide()).collect();
    for (i, p) in paires.iter_mut().enumerate() {
        if p.is_some() {
            continue;
        }
        let Some(x) = &ids_entrants[i] else { continue };
        if let Some(j) = (0..ids_actuels.len())
            .find(|j| !pris[*j] && !valides[*j] && ids_actuels[*j].as_ref().is_some_and(|y| y.0 == x.0 && (x.2.is_none() || y.2.is_none())))
        {
            *p = Some(j);
            pris[j] = true;
        }
    }
    // Le meme numero sous un autre salarie : le meme engagement, dont le salarie a change (`numeros` dit s'il garde son
    // numero).
    for (i, p) in paires.iter_mut().enumerate() {
        let Some(n) = ids_entrants[i].as_ref().and_then(|x| x.2.as_ref()).filter(|_| p.is_none()) else { continue };
        if let Some(j) = (0..ids_actuels.len()).find(|j| !pris[*j] && ids_actuels[*j].as_ref().is_some_and(|y| y.2.as_ref() == Some(n))) {
            *p = Some(j);
            pris[j] = true;
        }
    }
    let mut touches: Vec<String> = vec![mois.to_string()];
    let mut nouveaux: Vec<Map<String, Value>> = Vec::new();
    for (i, e) in entrants.iter().enumerate() {
        let Value::Object(e) = e else { continue };
        let mut saisie = e.clone();
        for k in TENUS_PAR_LE_MOTEUR.iter().chain(DEDUITS.iter()) {
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
                match manques.get(i).cloned().flatten() {
                    Some(m) => {
                        poser_le_mois(o, mois, &saisie)?;
                        o.brut.insert(A_COMPLETER.into(), serde_json::json!({ "mois": mois, "manque": m }));
                    }
                    None => {
                        poser_le_mois(o, mois, &saisie)?;
                        if o.brut.get(A_COMPLETER).and_then(|a| a.get("mois")).and_then(Value::as_str).is_none_or(|m| m == mois) {
                            o.brut.remove(A_COMPLETER);
                        }
                    }
                }
                touches.push(o.fichier.clone());
            }
            None => {
                if let Some(m) = manques.get(i).cloned().flatten() {
                    saisie.insert(A_COMPLETER.into(), serde_json::json!({ "mois": mois, "manque": m }));
                }
                nouveaux.push(saisie)
            }
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

/// LA SAISIE D'UN ENGAGEMENT, POSEE SUR SON MOIS : l'engagement de rang `rang` de l'ecran de `mois` prend `saisie` (ce que
/// sa carte ecrit pour `mois`), comme l'ecriture de la saisie du mois le fait pour un engagement en saisie
/// (`ecrire_vue_du_mois`) : ce que le moteur tient seul ne se prend pas de la saisie, le numero du contrat et le nom du
/// spectacle ecrits se gardent. Refus : un engagement valide (il se rouvre d'abord, `etats::modifier`), ou des bornes qui
/// laisseraient hors du contrat un mois ou il a des jours payes ; rien n'est ecrit.
pub fn poser_la_saisie(ctx: &Contexte, mois: &str, rang: usize, saisie: &Map<String, Value>) -> Resultat<()> {
    let mut o = objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    if o.est_valide() {
        return Err(format!("{} : l'engagement de rang {} est valide, il ne prend pas de saisie", mois, rang + 1));
    }
    let mut saisie = saisie.clone();
    for k in TENUS_PAR_LE_MOTEUR.iter().chain(DEDUITS.iter()) {
        saisie.remove(*k);
    }
    if !saisie.get("numero").is_some_and(vrai) {
        if let Some(n) = o.brut.get("numero").filter(|n| vrai(n)).cloned() {
            saisie.insert("numero".into(), n);
        }
    }
    if let Some(ecrit) = o.vue(mois)?.and_then(|v| v.get("spectacle").filter(|s| vrai(s)).cloned()) {
        if saisie.get("spectacle").is_some_and(|s| *s != ecrit && meme_spectacle(ctx, s, &ecrit)) {
            saisie.insert("spectacle".into(), ecrit);
        }
    }
    poser_le_mois(&mut o, mois, &saisie)?;
    reecrire(&ctx.racine, &o)?;
    Ok(())
}

/// UN ENGAGEMENT NEUF, AJOUTE A L'ECRAN DE `mois` : `saisie` (ce que sa carte ecrit pour `mois`) commence dans le fichier
/// du mois, comme un nouveau de l'ecriture de la saisie (`ecrire_vue_du_mois`) ; ce que le moteur tient seul ne se prend
/// pas de la saisie. Rend son rang dans l'ecran de `mois`. Refus : un engagement du salarie et du premier jour d'un
/// engagement importe qui touche `mois`, ou un engagement que l'ecran de `mois` ne montrerait pas ; rien n'est ecrit.
pub fn ajouter_au_mois(ctx: &Contexte, mois: &str, saisie: &Map<String, Value>) -> Resultat<usize> {
    let racine = &ctx.racine;
    let e = Value::Object(saisie.clone());
    if let Some(cle) = crate::cle_de_l_engagement(ctx, &e) {
        let debut = crate::debut_brut(&e);
        if crate::importe::celui_de(racine, mois, &cle, &debut)?.is_some() {
            return Err(crate::importe::refus(&e.get("salarie").map(en_texte).unwrap_or(cle), &debut));
        }
    }
    let mut saisie = saisie.clone();
    for k in TENUS_PAR_LE_MOTEUR.iter().chain(DEDUITS.iter()) {
        saisie.remove(*k);
    }
    let p = chemin(racine, mois);
    let mut contenu = lire_fichier(racine, mois)?.unwrap_or_default();
    contenu.insert("mois".into(), Value::String(mois.to_string()));
    let es = contenu.entry("engagements").or_insert_with(|| Value::Array(Vec::new()));
    let Value::Array(es) = es else { return Err(format!("{} : `engagements` n'est pas une liste", mois)) };
    let rang = es.len();
    es.push(Value::Object(saisie));
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    socle::ecriture::ecrire_json(&p, &Value::Object(contenu), 2)?;
    objets_du_mois(racine, mois)?
        .iter()
        .position(|o| o.fichier == mois && o.rang == rang)
        .ok_or_else(|| format!("{} : l'engagement ajouté ne tombe pas dans ce mois (ses dates le placent ailleurs).", crate::libelle_mois(mois)))
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
        let tenus: Vec<(&str, Option<Value>)> = TENUS_PAR_LE_MOTEUR.iter().map(|k| (*k, o.brut.remove(*k))).collect();
        let mut neuf = saisie.clone();
        for k in PARTAGES {
            match o.brut.get(k) {
                Some(x) => neuf.insert(k.to_string(), x.clone()),
                None => neuf.remove(k),
            };
        }
        o.brut = neuf;
        for (k, v) in tenus {
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
        if !o.dans_ses_bornes(m)? {
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

impl Objet {
    /// L'engagement tel qu'une saisie gardee (`saisie`, celle d'une validation) le disait : meme fichier, meme rang.
    pub fn d_apres_la_saisie(&self, saisie: &Map<String, Value>) -> Objet {
        Objet { racine: self.racine.clone(), fichier: self.fichier.clone(), rang: self.rang, brut: saisie.clone() }
    }

    /// Ce que la vue de `mois` dit du contrat qui se reconduit (`RECONDUITS`, hors `PARTAGES`) : ce qui change « a partir
    /// d'un mois ».
    pub fn termes_du_mois(&self, mois: &str) -> Resultat<Option<Map<String, Value>>> {
        Ok(self.vue(mois)?.map(|v| v.into_iter().filter(|(k, _)| RECONDUITS.contains(&k.as_str()) && !PARTAGES.contains(&k.as_str())).collect()))
    }

    /// LE CHANGEMENT « A PARTIR DE `depuis` » d'un contrat paye au mois : `self` est l'engagement modifie, `ancien` ce
    /// qu'il disait a sa validation. Le changement, c'est chaque terme du contrat (`RECONDUITS`) que `self` dit autrement
    /// qu'`ancien`, avec la valeur qu'il prend au premier mois ou il differe. Les mois d'avant `depuis` reprennent les
    /// termes d'`ancien` ; `depuis` et tous les mois d'apres prennent les termes d'`ancien` et le changement par-dessus,
    /// meme quand `ancien` y portait deja un autre chiffre : ce qui vaut « a partir de » vaut jusqu'au terme. Chaque mois
    /// garde ce qui n'appartient qu'a lui (conges, primes). Le mois `depuis`, s'il n'est pas le premier, porte la date
    /// de l'avenant (`date_modification`, le 1er du mois, quand la saisie n'en dit pas) : le document qui dit la
    /// modification la date de sa prise d'effet (code du travail R1221-40, `atelier/reference/sources/code-du-travail/
    /// articles-20261029.md`, l. 856), et la DSN date de meme le bloc S21.G00.41 (41.001). Un mois d'apres dont les
    /// termes ne changent plus perd la date d'un avenant qui n'y a plus lieu.
    ///
    /// `le` : le jour du mois `depuis` ou le changement prend effet, quand ce n'est pas le premier. Le mois porte alors
    /// l'avenant a cette date, et les termes d'avant (`avant` : le salaire et la duree du travail d'`ancien`) qui paient
    /// ses jours d'avant (`periodes::bornes`, chaque part par `droit::periodes::part_du_mois`).
    pub fn a_partir_du(&self, ancien: &Objet, depuis: &str, le: Option<Jour>) -> Resultat<Objet> {
        let termes: Vec<&str> = RECONDUITS.iter().copied().filter(|k| !PARTAGES.contains(k)).collect();
        let mut mois: Vec<String> = self.mois_saisis();
        mois.extend(ancien.mois_saisis());
        mois.push(depuis.to_string());
        mois.sort();
        mois.dedup();
        // Le changement : chaque terme, a sa valeur au premier mois ou il differe (`None` : retire).
        let mut change: BTreeMap<&str, Option<Value>> = BTreeMap::new();
        for m in &mois {
            let (Some(n), Some(a)) = (self.vue(m)?, ancien.vue(m)?) else { continue };
            for k in &termes {
                if !change.contains_key(k) && n.get(*k).map(canonique) != a.get(*k).map(canonique) {
                    change.insert(k, n.get(*k).cloned());
                }
            }
        }
        let mut cibles: Vec<(String, Map<String, Value>)> = Vec::new();
        for m in mois {
            let Some(mut v) = self.vue(&m)? else { continue };
            if let Some(a) = ancien.vue(&m)? {
                for k in &termes {
                    match a.get(*k) {
                        Some(x) => v.insert(k.to_string(), x.clone()),
                        None => v.remove(*k),
                    };
                }
            }
            if m.as_str() >= depuis {
                for (k, x) in &change {
                    match x {
                        Some(x) => v.insert(k.to_string(), x.clone()),
                        None => v.remove(*k),
                    };
                }
            }
            let au_jour = le.filter(|j| m.as_str() == depuis && j.jour > 1);
            if let Some(j) = au_jour {
                v.insert("date_modification".into(), Value::String(j.iso()));
                match termes_d_avant(ancien, &m, &v)? {
                    Some(a) => v.insert("avant".into(), Value::Object(a)),
                    None => v.remove("avant"),
                };
            } else if m.as_str() == depuis && le.is_some() {
                // Le changement vaut du premier jour du mois : rien ne s'y coupe.
                v.remove("avant");
                if m == self.fichier {
                    v.remove("date_modification");
                } else {
                    v.insert("date_modification".into(), Value::String(premier_du_mois(&m)?.iso()));
                }
            } else if m.as_str() == depuis && m != self.fichier && !v.get("date_modification").is_some_and(vrai) {
                v.insert("date_modification".into(), Value::String(premier_du_mois(&m)?.iso()));
            } else if m.as_str() > depuis {
                let memes = |x: &Map<String, Value>, y: &Map<String, Value>| termes.iter().all(|k| x.get(*k).map(canonique) == y.get(*k).map(canonique));
                if cibles.last().is_some_and(|(_, p)| memes(p, &v)) {
                    v.remove("date_modification");
                }
            }
            cibles.push((m, v));
        }
        let mut r = self.clone();
        let tenus: Vec<(&str, Value)> = TENUS_PAR_LE_MOTEUR.iter().filter(|k| **k != SUITE).filter_map(|k| r.brut.get(*k).map(|v| (*k, v.clone()))).collect();
        let premiere = cibles.iter().find(|(m, _)| *m == self.fichier).map(|(_, v)| v.clone()).unwrap_or_else(|| self.premiere_vue());
        r.brut = premiere;
        for k in DEDUITS {
            r.brut.remove(k);
        }
        for (k, v) in tenus {
            r.brut.insert(k.to_string(), v);
        }
        for (m, v) in cibles.iter().filter(|(m, _)| *m != self.fichier) {
            changer_un_mois(&mut r, m, v)?;
        }
        if let Some(Value::Object(s)) = r.brut.get_mut(SUITE) {
            s.retain(|_, v| !v.as_object().is_some_and(Map::is_empty));
        }
        if r.brut.get(SUITE).and_then(Value::as_object).is_some_and(Map::is_empty) {
            r.brut.remove(SUITE);
        }
        Ok(r)
    }
}

/// Les termes du contrat qu'`ancien` disait en `mois` (`droit::periodes::TERMES_D_AVANT` : le salaire mensuel et la duree
/// du travail, celle de la nature quand la saisie n'en dit pas), quand la vue `v` du mois les change ; `None` quand ils
/// ne changent pas (l'avenant change autre chose : il se date, rien ne se coupe).
fn termes_d_avant(ancien: &Objet, mois: &str, v: &Map<String, Value>) -> Resultat<Option<Map<String, Value>>> {
    let Some(a) = ancien.vue(mois)? else { return Ok(None) };
    let terme = |x: &Map<String, Value>, k: &str| -> Resultat<Option<Value>> {
        match x.get(k).filter(|y| !y.is_null()) {
            Some(y) => Ok(Some(y.clone())),
            None if k == "quotite_contrat" => {
                let nat = crate::conges::nature_de(x.get("nature"))?;
                Ok(nat.quotite_reference_defaut.map(Value::from))
            }
            None => Ok(None),
        }
    };
    let mut avant = Map::new();
    let mut change = false;
    for k in droit::periodes::TERMES_D_AVANT {
        let (x, y) = (terme(&a, k)?, terme(v, k)?);
        change |= x.as_ref().map(canonique) != y.as_ref().map(canonique);
        if let Some(x) = x {
            avant.insert(k.to_string(), x);
        }
    }
    Ok(change.then_some(avant))
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
pub(crate) fn sauver_avant(chemin: &Path) -> Resultat<()> {
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
    if chemin(racine, mois).is_file() && !importes_seuls(racine, mois) {
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
