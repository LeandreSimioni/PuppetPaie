//! `moteur/importation.py` : lire ce que CoolPaie a depose (DSN et AEM), pour ne pas le ressaisir.
//!
//! Ce module LIT et rend des donnees, comme le Python : il n'ecrit rien. Les DSN se lisent ligne a
//! ligne en gardant l'individu courant ; les AEM par positions fixes (1000 caracteres, latin-1).
//! Les dictionnaires gardent l'ordre de leurs cles (`preserve_order`), comme ceux de Python.

use std::path::Path;

use serde_json::{json, Map, Value};

use socle::texte::{lignes_py, texte_latin1};
use socle::texte::blanc;

/// Bloc S21.G00.30 (individu) -> champ de `salaries.json`.
///
/// Le sexe (30.005, « 01 - masculin », « 02 - féminin ») devient le « 1 » ou « 2 » de la fiche : la norme ne le fait
/// porter que s'il differe de celui du NIR ou si le NIR manque (cahier technique 2026.1, p. 171,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 154-163). Le nom d'usage (30.003, meme bloc, l. 138-144)
/// n'a pas de case dans la fiche salarie (`champs-base.json`) : il n'est pas lu (lot I2, a decider).
const IDENTITE: [(&str, &str); 14] = [
    ("S21.G00.30.001", "nir"),
    ("S21.G00.30.002", "nom"),
    ("S21.G00.30.004", "prenom"),
    ("S21.G00.30.005", "sexe"),
    ("S21.G00.30.006", "date_naissance"),
    ("S21.G00.30.007", "lieu_naissance"),
    ("S21.G00.30.008", "adresse"),
    ("S21.G00.30.009", "code_postal"),
    ("S21.G00.30.010", "commune"),
    ("S21.G00.30.013", "code_nationalite_dsn"),
    ("S21.G00.30.014", "departement_naissance"),
    ("S21.G00.30.015", "pays_naissance"),
    ("S21.G00.30.018", "email"),
    ("S21.G00.30.019", "code_salarie"),
];

/// Bloc S21.G00.40 (contrat) -> ce que la fiche salarie porte de l'emploi.
pub(crate) const EMPLOI: [(&str, &str); 5] = [
    ("S21.G00.40.002", "statut_conventionnel"),
    ("S21.G00.40.003", "statut_categoriel_rc"),
    ("S21.G00.40.004", "pcs_ese"),
    ("S21.G00.40.005", "code_emploi_ft"),
    ("S21.G00.40.006", "emploi"),
];

/// L'entreprise : les blocs 06 (l'entreprise), 11 (l'etablissement) et 40, PAS l'en-tete S10.G00.01, qui est
/// l'EMETTEUR de l'envoi -- « dans le cas d'un tiers declarant ou d'un concentrateur, c'est l'identifiant SIREN de ce
/// tiers qui doit figurer ici » (cahier technique 2026.1, p. 130, `atelier/reference/norme-dsn/texte/bloc-S10.G00.01.txt`) :
/// un cabinet qui depose y met son nom et son adresse (lot I2, 3b). Le Siren et l'APEN se lisent en S21.G00.06 (p. 141-142,
/// `bloc-S21.G00.06.txt`) ; le NIC, l'adresse et l'APET de l'etablissement, en S21.G00.11 (p. 144-145, `bloc-S21.G00.11.txt`)
/// -- la fiche ne porte qu'un etablissement, celui de son NIC, que le moteur declare en 06.002 et en 11.001 (`dsn.rs`) ;
/// l'adresse du siege (06.004-006) et son NIC (06.002) ne servent qu'a defaut. La DSN ne declare pas la raison sociale de
/// l'entreprise : celle de l'emetteur (S10.G00.01.003) n'est reprise que si l'emetteur EST l'entreprise (meme Siren en
/// S10.G00.01.001 et en S21.G00.06.001) -- une inference : la norme ne dit pas que l'entreprise qui depose elle-meme y met
/// son nom, mais l'emetteur « ayant elabore le present envoi » est alors elle. L'IDCC de la fiche est la convention
/// principale de l'etablissement (S21.G00.11.022, p. 147, `bloc-S21.G00.11.txt`), qui vient avant tout contrat dans le
/// fichier ; l'IDCC d'un contrat (S21.G00.40.017) n'en tient lieu qu'a defaut : il en differe pour la 1734
/// (`droit::minima::idcc_du_contrat`). Le code convention DSN de la fiche, le moteur le tire de l'IDCC (`routes::fiches::deriver`).
const ENTREPRISE: [(&str, &str); 8] = [
    ("S21.G00.06.001", "siren"),
    ("S21.G00.11.001", "nic"),
    ("S21.G00.11.003", "adresse"),
    ("S21.G00.11.004", "code_postal"),
    ("S21.G00.11.005", "commune"),
    ("S21.G00.06.003", "naf"),
    ("S21.G00.11.022", "idcc"),
    ("S21.G00.40.017", "idcc"),
];

/// Ce qui ne se reprend qu'a defaut (le siege, l'APET) ou sous condition (l'emetteur) : voir `ENTREPRISE`.
const A_DEFAUT: [(&str, &str); 7] = [
    ("S10.G00.01.001", "emetteur_siren"),
    ("S10.G00.01.003", "emetteur_nom"),
    ("S21.G00.06.002", "nic_siege"),
    ("S21.G00.06.004", "adresse_siege"),
    ("S21.G00.06.005", "code_postal_siege"),
    ("S21.G00.06.006", "commune_siege"),
    ("S21.G00.11.002", "apet"),
];

/// Les montants qu'une DSN declare, versement par versement (bloc 50). 50.004 est le montant net verse de la norme (la
/// remuneration nette fiscale moins CSG/CRDS non deductible et sante patronale, p. 244), pas le net a payer : CoolPaie y
/// ecrivait son net a payer, PuppetPaie y ecrit le net verse depuis la vague 1a. La DSN ne declare pas le net a payer ;
/// une paie importee d'une DSN seule n'en a pas, un bulletin le donne (`importation_bulletins::completer`).
const VERSEMENT: [(&str, &str); 3] =
    [("S21.G00.50.002", "net_imposable"), ("S21.G00.50.004", "net_verse"), ("S21.G00.50.009", "pas")];

/// Une valeur que la NATURE du contrat fixe (`contrats-base.json`, `codes` : « 999SPT » en 40.005 pour tout permanent,
/// CCH-18 du bloc 40) ne dit rien du salarie : elle ne passe pas dans sa fiche. `999SPT` n'est pas un metier, et une
/// DSN de PuppetPaie reimportee le donnerait comme `code_emploi_ft` d'un permanent (#152).
fn fixe_par_une_nature(rubrique: &str, valeur: &str) -> bool {
    droit::natures::natures().map_or(false, |n| n.iter().any(|nat| nat.code(rubrique).as_deref() == Some(valeur)))
}

/// `01` -> `1`, `02` -> `2` (les choix du champ `sexe` de la fiche) ; une autre valeur reste telle quelle, et la fiche
/// la signalera.
fn sexe_de_la_fiche(v: &str) -> String {
    match v.trim() {
        "01" => "1".into(),
        "02" => "2".into(),
        autre => autre.to_string(),
    }
}

fn table(t: &[(&'static str, &'static str)], k: &str) -> Option<&'static str> {
    t.iter().find(|(c, _)| *c == k).map(|(_, v)| *v)
}

/// `str.isdigit()` pour un caractere latin-1 : les chiffres, et `²`, `³`, `¹`.
fn chiffre(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
}

/// `_jjmmaaaa` : `23071990` -> `1990-07-23` ; une date illisible est rendue telle quelle.
pub(crate) fn jjmmaaaa(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 && c.iter().all(|x| chiffre(*x)) {
        let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
    } else {
        v.to_string()
    }
}

/// `_mois_dsn` : `01072026` -> `2026-07`.
fn mois_dsn(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 {
        format!("{}-{}", c[4..8].iter().collect::<String>(), c[2..4].iter().collect::<String>())
    } else {
        String::new()
    }
}

fn texte(s: &str) -> Value {
    Value::String(s.to_string())
}

/// `importation.lire_une_dsn` : une DSN deposee, son mois, ses individus, ses contrats. `None` si le
/// fichier ne se lit pas, ou si elle ne dit pas son mois.
pub(crate) fn lire_une_dsn(chemin: &Path) -> Option<Value> {
    let octets = std::fs::read(chemin).ok()?;
    let contenu = texte_latin1(&octets);
    let lignes = lignes_py(&contenu);
    let (mut mois, mut depot) = (String::new(), String::new());
    let mut entreprise: Map<String, Value> = Map::new();
    let mut individus: Vec<Map<String, Value>> = Vec::new();
    let mut contrats: Vec<Map<String, Value>> = Vec::new();
    let mut versements: Vec<Map<String, Value>> = Vec::new();
    let mut versement: Option<usize> = None;
    let mut cur: Option<usize> = None;
    // L'emetteur, le siege et l'APET : ce qui ne sert qu'a defaut, ou a dire que la fiche ne porte pas tout.
    let mut autres: Map<String, Value> = Map::new();
    let mut nics: Vec<String> = Vec::new();
    // Les versements aux organismes (bloc 20) : identifiant, entite d'affectation, BIC, IBAN.
    let mut blocs20: Vec<[String; 4]> = Vec::new();
    for l in &lignes {
        let Some((k, v)) = l.split_once(',') else { continue };
        let v = v.trim_matches(blanc).trim_matches('\'');
        if let Some(c) = table(&A_DEFAUT, k) {
            autres.entry(c.to_string()).or_insert_with(|| texte(v));
        }
        match k {
            "S21.G00.20.001" => blocs20.push([v.to_string(), String::new(), String::new(), String::new()]),
            "S21.G00.20.002" | "S21.G00.20.003" | "S21.G00.20.004" => {
                if let Some(b) = blocs20.last_mut() {
                    b[k[k.len() - 1..].parse::<usize>().unwrap_or(1) - 1] = v.to_string();
                }
            }
            _ => {}
        }
        if k == "S21.G00.11.001" && !nics.iter().any(|n| n == v) {
            nics.push(v.to_string());
        }
        if k == "S20.G00.05.005" {
            mois = mois_dsn(v);
        } else if k == "S20.G00.05.007" {
            depot = jjmmaaaa(v);
        } else if let Some(champ) = table(&ENTREPRISE, k) {
            entreprise.entry(champ.to_string()).or_insert_with(|| texte(v));
        } else if k == "S21.G00.40.022" {
            // La DSN declare « 97 puis le numero d'adherent » (`conges_spectacles::code_dsn`) : la fiche ne garde que le
            // numero d'adherent, et seulement s'il a la forme que dit la fiche d'Audiens. Autre chose ne se reprend pas.
            if let Some(n) = v.strip_prefix("97").filter(|n| documents::conges_spectacles::faute_adherent(n).is_none()) {
                entreprise.entry(documents::conges_spectacles::CLE_ADHERENT.to_string()).or_insert_with(|| texte(n));
            }
        } else if k == "S21.G00.40.049" {
            entreprise.entry("licence".to_string()).or_insert_with(|| texte(v));
        } else if k == "S21.G00.30.001" {
            let mut m = Map::new();
            m.insert("nir".into(), texte(v));
            individus.push(m);
            cur = Some(individus.len() - 1);
        } else if let (Some(champ), Some(i)) = (table(&IDENTITE, k), cur) {
            let valeur = match champ {
                "date_naissance" => jjmmaaaa(v),
                "sexe" => sexe_de_la_fiche(v),
                _ => v.to_string(),
            };
            individus[i].insert(champ.to_string(), Value::String(valeur));
        } else if let (true, Some(i)) = (k == "S21.G00.50.001", cur) {
            let mut m = Map::new();
            m.insert("nir".into(), individus[i]["nir"].clone());
            m.insert("verse_le".into(), Value::String(jjmmaaaa(v)));
            versements.push(m);
            versement = Some(versements.len() - 1);
        } else if let (Some(champ), Some(i)) = (table(&VERSEMENT, k), versement) {
            versements[i].insert(champ.to_string(), texte(v));
        } else if let (true, Some(i)) = (k == "S21.G00.51.010", versement) {
            versements[i].insert("contrat".into(), texte(v));
        } else if let (true, Some(i)) = (k == "S21.G00.51.011", versement) {
            versements[i].insert("_type".into(), texte(v));
        } else if let (true, Some(i)) = (k == "S21.G00.51.013", versement) {
            // Seul le type 001 est le brut.
            let type_ = versements[i].shift_remove("_type");
            if type_.as_ref().and_then(Value::as_str) == Some("001") {
                versements[i].insert("brut".into(), texte(v));
            }
        } else if let (true, Some(i)) = (k == "S21.G00.50.006", cur) {
            individus[i].insert("taux_pas".into(), texte(v));
        } else if let (true, Some(i)) = (k == "S21.G00.40.001", cur) {
            let mut m = Map::new();
            m.insert("nir".into(), individus[i]["nir"].clone());
            m.insert("debut".into(), Value::String(jjmmaaaa(v)));
            contrats.push(m);
        } else if let (Some(champ), Some(c)) = (table(&EMPLOI, k), contrats.last_mut()) {
            c.insert(champ.to_string(), texte(v));
        } else if let (true, Some(c)) = (k == "S21.G00.40.009", contrats.last_mut()) {
            c.insert("numero".into(), texte(v));
        } else if let (true, Some(c)) = (k == "S21.G00.40.013", contrats.last_mut()) {
            c.insert("quotite".into(), texte(v));
        } else if let (true, Some(c)) = (k == "S21.G00.40.010", contrats.last_mut()) {
            c.insert("fin".into(), Value::String(jjmmaaaa(v)));
        } else if let (true, Some(c)) = (k == "S21.G00.40.050", contrats.last_mut()) {
            c.insert("objet".into(), texte(v));
        }
    }
    if mois.is_empty() {
        return None;
    }
    let lu = |m: &Map<String, Value>, k: &str| m.get(k).and_then(Value::as_str).map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    if lu(&autres, "emetteur_siren").is_some() && lu(&autres, "emetteur_siren") == lu(&entreprise, "siren") {
        if let Some(n) = lu(&autres, "emetteur_nom") {
            entreprise.insert("raison_sociale".into(), texte(&n));
        }
    }
    for (de, vers) in [("nic_siege", "nic"), ("apet", "naf")] {
        if lu(&entreprise, vers).is_none() {
            if let Some(x) = lu(&autres, de) {
                entreprise.insert(vers.into(), texte(&x));
            }
        }
    }
    if lu(&entreprise, "adresse").is_none() && lu(&entreprise, "code_postal").is_none() {
        for (de, vers) in [("adresse_siege", "adresse"), ("code_postal_siege", "code_postal"), ("commune_siege", "commune")] {
            if let Some(x) = lu(&autres, de) {
                entreprise.insert(vers.into(), texte(&x));
            }
        }
    }
    // LE COMPTE QUI PAIE (lot I2, 3e) : le BIC et l'IBAN « du compte sur lequel doit etre preleve le montant de l'ordre
    // de reglement » (20.003, 20.004 ; cahier technique 2026.1, p. 156, `bloc-S21.G00.20.txt`), la `banque` de la fiche
    // Entreprise que le moteur y ecrit (`dsn.rs`) -- quand tous les versements de la DSN disent le meme compte ; deux
    // comptes, rien (on ne choisit pas). Le dossier de recouvrement France Travail : l'entite d'affectation (20.002) du
    // versement a France Travail, « identifiant attribue par le centre de recouvrement Spectacle (CS suivi de 8
    // caracteres numeriques) » (p. 155), que le moteur ecrit depuis `france_travail_dossier_recouvrement`.
    let comptes: std::collections::BTreeSet<(&str, &str)> =
        blocs20.iter().filter(|b| !b[2].is_empty() && !b[3].is_empty()).map(|b| (b[2].as_str(), b[3].as_str())).collect();
    if comptes.len() == 1 {
        let (bic, iban) = comptes.iter().next().copied().unwrap_or_default();
        entreprise.entry("banque").or_insert_with(|| json!({"bic": bic, "iban": iban}));
    }
    if let Some(b) = blocs20.iter().find(|b| b[0] == declarations::dsn::SIRET_FRANCE_TRAVAIL && !b[1].is_empty()) {
        entreprise.entry("france_travail_dossier_recouvrement").or_insert_with(|| texte(&b[1]));
    }
    let versements: Vec<Value> = versements
        .into_iter()
        .filter(|v| v.get("contrat").and_then(Value::as_str).is_some_and(|c| !c.is_empty()))
        .map(Value::Object)
        .collect();
    let mut out = Map::new();
    out.insert("mois".into(), Value::String(mois));
    out.insert("depot".into(), Value::String(depot));
    out.insert("entreprise".into(), Value::Object(entreprise));
    out.insert("individus".into(), Value::Array(individus.into_iter().map(Value::Object).collect()));
    out.insert("contrats".into(), Value::Array(contrats.into_iter().map(Value::Object).collect()));
    out.insert("versements".into(), Value::Array(versements));
    out.insert("etablissements".into(), json!(nics));
    out.insert("nic_siege".into(), json!(lu(&autres, "nic_siege").unwrap_or_default()));
    out.insert("fichier".into(), Value::String(chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    Some(Value::Object(out))
}

// ------------------------------------------------------------------ toutes les DSN d'un dossier

/// `importation.SalarieDeclare` : ce qu'une DSN dit d'un salarie.
pub(crate) struct SalarieDeclare {
    pub(crate) nir: String,
    pub(crate) fiche: Map<String, Value>,
    pub(crate) mois: String,
    pub(crate) initiales: String,
    pub(crate) premier_contrat: String,
}

/// `importation.ObjetDeclare` : un numero d'objet spectacle, vu dans une DSN ou dans une AEM.
pub(crate) struct ObjetDeclare {
    pub(crate) numero: String,
    pub(crate) premier_mois: String,
    pub(crate) dernier_mois: String,
    pub(crate) contrats: i64,
    pub(crate) source: &'static str,
}

/// `importation.LectureDsn`. Les listes gardent l'ordre ou les cles sont apparues, comme les
/// dictionnaires de Python.
#[derive(Default)]
pub(crate) struct LectureDsn {
    pub(crate) fichiers: usize,
    pub(crate) salaries: Vec<SalarieDeclare>,
    pub(crate) objets: Vec<ObjetDeclare>,
    pub(crate) entreprise: Map<String, Value>,
    pub(crate) licences: Vec<(String, String)>,
    pub(crate) premier_mois: String,
    pub(crate) dernier_mois: String,
    pub(crate) illisibles: Vec<String>,
    /// Les etablissements declares (S21.G00.11.001), et le NIC du siege (S21.G00.06.002), d'une DSN a l'autre.
    pub(crate) etablissements: std::collections::BTreeSet<String>,
    pub(crate) nics_siege: std::collections::BTreeSet<String>,
    pub(crate) paies: std::collections::BTreeMap<(String, String, String), Map<String, Value>>,
}

/// `Path.suffix.lower()`.
fn suffixe_minuscule(nom: &str) -> String {
    match nom.rfind('.') {
        Some(i) if i > 0 && i < nom.len() - 1 => nom[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn noms_tries(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = match std::fs::read_dir(dossier) {
        Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

/// `importation.fichiers_dsn` : une DSN se reconnait a son contenu, pas a son nom.
pub(crate) fn fichiers_dsn(dossier: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for nom in noms_tries(dossier) {
        let f = dossier.join(&nom);
        if !f.is_file() {
            continue;
        }
        if suffixe_minuscule(&nom) == ".dsn" {
            out.push(f);
            continue;
        }
        let Ok(mut h) = std::fs::File::open(&f) else { continue };
        let mut tete = [0u8; 64];
        let mut lu = 0;
        {
            use std::io::Read;
            while lu < tete.len() {
                match h.read(&mut tete[lu..]) {
                    Ok(0) => break,
                    Ok(n) => lu += n,
                    Err(_) => break,
                }
            }
        }
        let mut t = &tete[..lu];
        while t.starts_with(b"\xef\xbb\xbf") {
            t = &t[3..];
        }
        if t.starts_with(b"S10.G00.00.001,") {
            out.push(f);
        }
    }
    out
}

fn texte_de<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// `importation.lire_dsn` : toutes les DSN d'un dossier, la plus recente faisant foi par salarie.
pub(crate) fn lire_dsn(dossier: &Path) -> LectureDsn {
    let mut out = LectureDsn::default();
    let mut lues: Vec<Value> = Vec::new();
    for f in fichiers_dsn(dossier) {
        match lire_une_dsn(&f) {
            Some(info) => lues.push(info),
            None => out.illisibles.push(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        }
    }
    // Par mois declare, puis par date de depot : un annule-et-remplace depose apres l'original fait foi.
    lues.sort_by(|a, b| {
        (texte_de(a, "mois"), texte_de(a, "depot"), texte_de(a, "fichier")).cmp(&(texte_de(b, "mois"), texte_de(b, "depot"), texte_de(b, "fichier")))
    });
    out.fichiers = lues.len();
    if let (Some(p), Some(d)) = (lues.first(), lues.last()) {
        out.premier_mois = texte_de(p, "mois").to_string();
        out.dernier_mois = texte_de(d, "mois").to_string();
    }
    let mut index_salaries: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut index_objets: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for info in &lues {
        let mois = texte_de(info, "mois").to_string();
        let depot = texte_de(info, "depot").to_string();
        out.etablissements.extend(info["etablissements"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from));
        out.nics_siege.extend(Some(texte_de(info, "nic_siege").to_string()).filter(|n| !n.is_empty()));
        if let Some(e) = info["entreprise"].as_object() {
            for (k, v) in e {
                if k == "licence" {
                    let v = v.as_str().unwrap_or("").to_string();
                    if out.licences.last().map_or(true, |l| l.1 != v) {
                        out.licences.push((mois.clone(), v));
                    }
                } else {
                    out.entreprise.insert(k.clone(), v.clone());
                }
            }
        }
        let contrats: &[Value] = info["contrats"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let mut dernier_contrat: std::collections::HashMap<&str, &Value> = std::collections::HashMap::new();
        let mut par_numero: std::collections::HashMap<&str, &Value> = std::collections::HashMap::new();
        for c in contrats {
            dernier_contrat.insert(texte_de(c, "nir"), c);
            if let Some(n) = c.get("numero").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                par_numero.insert(n, c);
            }
            if let Some(objet) = c.get("objet").and_then(Value::as_str).filter(|o| !o.is_empty()) {
                let i = *index_objets.entry(objet.to_string()).or_insert_with(|| {
                    out.objets.push(ObjetDeclare {
                        numero: objet.to_string(),
                        premier_mois: mois.clone(),
                        dernier_mois: mois.clone(),
                        contrats: 0,
                        source: "DSN",
                    });
                    out.objets.len() - 1
                });
                out.objets[i].dernier_mois = mois.clone();
                out.objets[i].contrats += 1;
            }
        }
        for v in info["versements"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let numero = texte_de(v, "contrat");
            let c = par_numero.get(numero).copied();
            let ici = |k: &str| c.map_or("", |c| texte_de(c, k)).to_string();
            let mut m: Map<String, Value> = v.as_object().cloned().unwrap_or_default();
            m.insert("mois".into(), Value::String(mois.clone()));
            m.insert("objet".into(), Value::String(ici("objet")));
            m.insert("debut".into(), Value::String(ici("debut")));
            m.insert("fin".into(), Value::String(ici("fin")));
            if !ici("quotite").is_empty() {
                m.insert("quotite".into(), Value::String(ici("quotite")));
            }
            m.insert("depot".into(), Value::String(depot.clone()));
            m.insert("fichier".into(), info["fichier"].clone());
            out.paies.insert((mois.clone(), texte_de(v, "nir").to_string(), numero.to_string()), m);
        }
        for ind in info["individus"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let nir = texte_de(ind, "nir").to_string();
            let mut fiche: Map<String, Value> = ind.as_object().cloned().unwrap_or_default();
            fiche.shift_remove("nir");
            let c = dernier_contrat.get(nir.as_str()).copied();
            for (rubrique, champ) in EMPLOI {
                if let Some(v) = c.and_then(|c| c.get(champ)).filter(|v| v.as_str().is_some_and(|s| !s.is_empty())) {
                    if fixe_par_une_nature(rubrique, v.as_str().unwrap_or_default()) {
                        continue;
                    }
                    fiche.insert(champ.to_string(), v.clone());
                }
            }
            let numero = c.map_or("", |c| texte_de(c, "numero"));
            let initiales: String = numero.split('.').next().unwrap_or("").chars().filter(|ch| ch.is_alphabetic()).take(2).collect();
            let mut debuts: Vec<String> = contrats
                .iter()
                .filter(|x| texte_de(x, "nir") == nir && !texte_de(x, "debut").is_empty())
                .map(|x| texte_de(x, "debut").to_string())
                .collect();
            if let Some(&i) = index_salaries.get(&nir) {
                if !out.salaries[i].premier_contrat.is_empty() {
                    debuts.push(out.salaries[i].premier_contrat.clone());
                }
            }
            let premier = debuts.into_iter().min().unwrap_or_default();
            let decl = SalarieDeclare { nir: nir.clone(), fiche, mois: mois.clone(), initiales, premier_contrat: premier };
            match index_salaries.get(&nir) {
                Some(&i) => out.salaries[i] = decl,
                None => {
                    out.salaries.push(decl);
                    index_salaries.insert(nir, out.salaries.len() - 1);
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------------ les AEM

/// `importation.CHAMPS` : nom -> (debut, fin), en caracteres.
/// `ligne[a:b]` : une ligne plus courte rend ce qu'elle a.
fn tranche(ligne: &[char], a: usize, b: usize) -> String {
    ligne.iter().skip(a).take(b.saturating_sub(a)).collect()
}

/// `importation._aaaammjj` : `19900723` -> `1990-07-23` (les dates de l'AEM), le reste tel quel.
fn aaaammjj(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    // `str.isdigit()` : les chiffres ASCII et les exposants de Latin-1 (qu'une AEM lue en latin-1 peut porter).
    if c.len() == 8 && c.iter().all(|x| x.is_ascii_digit() || matches!(x, '\u{b2}' | '\u{b3}' | '\u{b9}')) {
        return format!("{}-{}-{}", c[0..4].iter().collect::<String>(), c[4..6].iter().collect::<String>(), c[6..8].iter().collect::<String>());
    }
    v.to_string()
}

fn champ(ligne: &[char], a: usize, b: usize) -> String {
    ligne[a..b].iter().collect()
}

/// `importation.ContratAtteste` : un enregistrement AEM, un contrat tel qu'il a ete atteste.
pub(crate) struct ContratAtteste {
    pub(crate) nir: String,
    pub(crate) debut: socle::date::Jour,
    pub(crate) fin: socle::date::Jour,
    pub(crate) numero_objet: String,
    pub(crate) heures: i128,
    /// Les cachets, et les cachets « groupes » (la case que le gabarit garde a 00).
    pub(crate) cachets: i128,
    pub(crate) jours: i128,
    /// `str(c.brut)` : le brut en euros, deux decimales (`Decimal` quantifie).
    pub(crate) brut: String,
    /// Le numero de l'attestation (zone 36..42), et sa date de signature (858..866, AAAA-MM-JJ si c'en est une).
    pub(crate) numero_attestation: String,
    pub(crate) date_attestation: String,
    pub(crate) fichier: String,
}

/// `importation.lire_aem` : tous les contrats attestes d'un dossier AEM_EDI, dedoublonnes, et les
/// fichiers ou enregistrements illisibles. Un `InvalidOperation` (un brut qui n'est pas un nombre)
/// n'est pas rattrape, comme en Python.
pub(crate) fn lire_aem(dossier: &Path) -> Result<(Vec<ContratAtteste>, Vec<String>), socle::echec::Echec> {
    use socle::num::{decimal_py, int_py};
    let mut vus: Vec<(Vec<String>, ContratAtteste)> = Vec::new();
    let mut connus: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
    let mut illisibles = Vec::new();
    // `glob.glob(".../*.txt")` : pas de fichier cache, ordre des chaines.
    let mut noms: Vec<String> = noms_tries(dossier).into_iter().filter(|n| !n.starts_with('.') && n.ends_with(".txt")).collect();
    noms.sort();
    for nom in noms {
        let f = dossier.join(&nom);
        let Ok(octets) = std::fs::read(&f) else {
            illisibles.push(nom);
            continue;
        };
        for ligne in lignes_py(&texte_latin1(&octets)) {
            let l: Vec<char> = ligne.chars().collect();
            if l.len() < 700 {
                continue;
            }
            let illisible = |illisibles: &mut Vec<String>| illisibles.push(format!("{} : un enregistrement illisible", nom));
            let nir: String = champ(&l, 375, 390).chars().take(13).collect();
            let (Some(debut), Some(fin)) =
                (socle::date::lire_aaaammjj(&champ(&l, 618, 626)), socle::date::lire_aaaammjj(&champ(&l, 627, 635)))
            else {
                illisible(&mut illisibles);
                continue;
            };
            let numero_objet = champ(&l, 638, 650).trim_matches(blanc).to_string();
            let entiers = [(650, 653), (653, 655), (655, 657), (657, 659)].map(|(a, b)| int_py(&champ(&l, a, b)));
            let [Some(heures), Some(c1), Some(c2), Some(jours)] = entiers else {
                illisible(&mut illisibles);
                continue;
            };
            let brut_texte = champ(&l, 659, 668);
            let Some(brut_cts) = decimal_py(&brut_texte) else {
                return Err(socle::echec::Echec { classe: Some("InvalidOperation"), message: socle::num::illisible(&brut_texte) });
            };
            let mut brut = brut_cts / socle::num::Decimal::from(100);
            brut = brut.round_dp(2);
            brut.rescale(2);
            let cle = vec![
                nir.clone(),
                debut.iso(),
                fin.iso(),
                numero_objet.clone(),
                heures.to_string(),
                (c1 + c2).to_string(),
                jours.to_string(),
                brut.to_string(),
            ];
            if connus.insert(cle.clone()) {
                vus.push((cle, ContratAtteste {
                    nir,
                    debut,
                    fin,
                    numero_objet,
                    heures,
                    cachets: c1 + c2,
                    jours,
                    brut: brut.to_string(),
                    numero_attestation: tranche(&l, 36, 42).trim_matches(blanc).to_string(),
                    date_attestation: aaaammjj(&tranche(&l, 858, 866)),
                    fichier: nom.clone(),
                }));
            }
        }
    }
    let mut contrats: Vec<ContratAtteste> = vus.into_iter().map(|(_, c)| c).collect();
    contrats.sort_by(|a, b| (a.debut, a.fin, &a.nir).cmp(&(b.debut, b.fin, &b.nir)));
    Ok((contrats, illisibles))
}

// ------------------------------------------------------------------ le dossier

/// `importation.dossiers` : ou sont les DSN et les AEM sous un dossier `export/` de CoolPaie.
pub(crate) fn dossiers(export: &Path) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    let mut dsn = export.join("DSN");
    let mut aem = export.join("AEM_EDI");
    if !dsn.is_dir() && !fichiers_dsn(export).is_empty() {
        dsn = export.to_path_buf();
    }
    if !aem.is_dir() && noms_tries(export).iter().any(|n| !n.starts_with('.') && n.contains("EDIV5") && n.ends_with(".txt")) {
        aem = export.to_path_buf();
    }
    (if dsn.is_dir() { Some(dsn) } else { None }, if aem.is_dir() { Some(aem) } else { None })
}

// ------------------------------------------------------------------ l'entreprise de chaque piece (lot G1)

/// Ou l'AEM porte le Siret de l'employeur : `ligne[50:64]`, 14 chiffres. Ni `CHAMPS` ni l'ancien `moteur/aem.py` ne
/// nommaient cette zone (elle etait figee dans le gabarit, comme tout ce que l'AEM ne change pas d'un contrat a
/// l'autre) : elle a ete relevee sur 190 enregistrements AEM produits par CoolPaie de 2021 a 2026, ou le Siret de la
/// fiche de la compagnie se lit a cette place dans tous. Un Siret ne se tient pour lu que s'il a ses 14 chiffres et sa
/// cle de Luhn ; sinon l'AEM passe, avec un avertissement nomme (`AEM_SANS_SIRET`) -- on ne refuse pas au hasard.
pub(crate) const AEM_SIRET: (usize, usize) = (50, 64);

/// Ce qu'une piece (DSN, AEM) dit de son employeur.
#[derive(Default, Clone)]
pub(crate) struct Employeur {
    /// Les Siren qu'elle porte (un, d'ordinaire ; une AEM en porte un par enregistrement).
    pub(crate) sirens: std::collections::BTreeSet<String>,
    /// Les enregistrements (AEM) ou l'identifiant de l'employeur ne se lit pas.
    pub(crate) illisibles: usize,
}

/// Une DSN : le Siren de l'ENTREPRISE (`S21.G00.06.001`), pas celui de l'emetteur (`S10.G00.01.001`), qui est celui
/// du cabinet quand un tiers depose (lot I2, 3b ; voir `ENTREPRISE`).
pub(crate) fn employeur_dsn(chemin: &Path) -> Employeur {
    let mut e = Employeur::default();
    let Ok(octets) = std::fs::read(chemin) else { return e };
    for l in lignes_py(&texte_latin1(&octets)) {
        if let Some(v) = l.strip_prefix("S21.G00.06.001,") {
            let v = v.trim_matches(blanc).trim_matches('\'').trim();
            if !v.is_empty() {
                e.sirens.insert(v.to_string());
            }
        }
    }
    e
}

/// Une AEM : le Siret de chaque enregistrement (`AEM_SIRET`), ramene a son Siren. Un fichier qui ne se lit pas ne dit
/// rien ici : `lire_aem` le dit illisible.
pub(crate) fn employeur_aem(chemin: &Path) -> Employeur {
    let mut e = Employeur::default();
    let Ok(octets) = std::fs::read(chemin) else { return e };
    for ligne in lignes_py(&texte_latin1(&octets)) {
        let l: Vec<char> = ligne.chars().collect();
        // Les enregistrements que `lire_aem` lit.
        if l.len() < 700 {
            continue;
        }
        let s = tranche(&l, AEM_SIRET.0, AEM_SIRET.1);
        if s.len() == 14 && s.chars().all(|c| c.is_ascii_digit()) && declarations::champs::luhn(&s) {
            e.sirens.insert(s[..9].to_string());
        } else {
            e.illisibles += 1;
        }
    }
    e
}

/// LE SIREN DE LA COMPAGNIE, pour tout ce que l'import lit (DSN, AEM, bulletins) : celui de la fiche Entreprise ;
/// sinon celui des DSN, si elles n'en portent qu'un (plusieurs : aucune reference) ; sinon, sans DSN qui le dise, celui
/// des autres pieces (bulletins, AEM) si elles n'en portent qu'un. `None` : rien ne dit laquelle est la compagnie.
pub(crate) fn siren_de_reference(
    fiche: &Value,
    dsn: &std::collections::BTreeSet<String>,
    autres: &std::collections::BTreeSet<String>,
) -> Option<String> {
    let s = fiche.get("siren").filter(|v| socle::fichiers::vrai(v)).map(socle::valeur::valeur_en_texte).map(|s| s.trim().to_string());
    if let Some(s) = s.filter(|s| !s.is_empty()) {
        return Some(s);
    }
    let seul = |e: &std::collections::BTreeSet<String>| (e.len() == 1).then(|| e.iter().next().cloned()).flatten();
    if !dsn.is_empty() {
        return seul(dsn);
    }
    seul(autres)
}

/// Une piece dont l'entreprise ne se rapporte a rien : le dossier porte plusieurs entreprises, aucune ne se tranche.
pub(crate) const PLUSIEURS_ENTREPRISES: &str = "le dossier porte plusieurs entreprises (Siren des DSN, Siret des bulletins ou des AEM), et ni la fiche Entreprise ni une seule DSN ne dit laquelle est la compagnie : non repris";
pub(crate) const DSN_AUTRE: &str = "DSN d'une autre entreprise : son Siren n'est pas celui de la compagnie, non reprise";
pub(crate) const DSN_SANS_SIREN: &str = "DSN sans Siren lisible (S21.G00.06.001) : rien ne dit de quelle entreprise elle vient, non reprise";
pub(crate) const AEM_AUTRE: &str = "AEM d'une autre entreprise : son Siret n'est pas celui de la compagnie, non reprise";
pub(crate) const AEM_SANS_SIRET: &str =
    "son Siret ne se lit pas (caractères 51 à 64 de l'enregistrement) : l'entreprise n'est pas contrôlée";

/// Le sort d'une DSN (`aem` faux) ou d'une AEM devant la reference : `Err(raison)` -- elle n'est pas reprise et RIEN
/// n'en est lu --, `Ok(Some(avertissement))` -- elle passe, mais son entreprise ne se controle pas --, ou `Ok(None)`.
pub(crate) fn verdict_entreprise(aem: bool, e: &Employeur, reference: Option<&str>) -> Result<Option<String>, String> {
    if e.sirens.is_empty() {
        // Une AEM sans enregistrement lisible ne rend rien a `lire_aem` : rien a controler.
        return if aem { Ok((e.illisibles > 0).then(|| AEM_SANS_SIRET.to_string())) } else { Err(DSN_SANS_SIREN.to_string()) };
    }
    let Some(r) = reference else { return Err(PLUSIEURS_ENTREPRISES.to_string()) };
    if e.sirens.iter().any(|s| s != r) {
        return Err((if aem { AEM_AUTRE } else { DSN_AUTRE }).to_string());
    }
    Ok((e.illisibles > 0).then(|| AEM_SANS_SIRET.to_string()))
}

/// Les DSN et les AEM d'un export (`dossiers`) devant la reference : celles qui ne sont pas reprises (chemin, raison),
/// et celles qui passent sans controle (chemin, avertissement). Pas de bulletins dans un export.
pub(crate) struct Garde {
    pub(crate) ecartes: Vec<(std::path::PathBuf, String)>,
    pub(crate) avertis: Vec<(std::path::PathBuf, String)>,
}

impl Garde {
    /// Ce que l'apercu en dit, piece par piece (le nom du fichier, rien d'autre).
    pub(crate) fn avertissements(&self, ecartes: bool, avertis: bool) -> Vec<String> {
        let nom = |f: &std::path::PathBuf| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        if ecartes {
            out.extend(self.ecartes.iter().map(|(f, raison)| format!("{} : {}", raison, nom(f))));
        }
        if avertis {
            out.extend(self.avertis.iter().map(|(f, a)| format!("AEM reprise, mais {} : {}", a, nom(f))));
        }
        out
    }
}

pub(crate) fn garder_export(fiche: &Value, dsn: Option<&Path>, aem: Option<&Path>) -> Garde {
    let dsns: Vec<(std::path::PathBuf, Employeur)> =
        dsn.map(fichiers_dsn).unwrap_or_default().into_iter().map(|f| { let e = employeur_dsn(&f); (f, e) }).collect();
    let aems: Vec<(std::path::PathBuf, Employeur)> =
        aem.map(fichiers_aem).unwrap_or_default().into_iter().map(|f| { let e = employeur_aem(&f); (f, e) }).collect();
    let sirens_dsn = dsns.iter().flat_map(|(_, e)| e.sirens.iter().cloned()).collect();
    let sirens_aem = aems.iter().flat_map(|(_, e)| e.sirens.iter().cloned()).collect();
    let reference = siren_de_reference(fiche, &sirens_dsn, &sirens_aem);
    let mut g = Garde { ecartes: Vec::new(), avertis: Vec::new() };
    for (aem, liste) in [(false, dsns), (true, aems)] {
        for (f, e) in liste {
            match verdict_entreprise(aem, &e, reference.as_deref()) {
                Err(raison) => g.ecartes.push((f, raison)),
                Ok(Some(a)) => g.avertis.push((f, a)),
                Ok(None) => {}
            }
        }
    }
    g
}

/// Les fichiers qu'un dossier AEM_EDI donne a `lire_aem` : les `*.txt` (`glob`), sans fichier cache, dans l'ordre.
pub(crate) fn fichiers_aem(dossier: &Path) -> Vec<std::path::PathBuf> {
    noms_tries(dossier).into_iter().filter(|n| !n.starts_with('.') && n.ends_with(".txt")).map(|n| dossier.join(n)).collect()
}
