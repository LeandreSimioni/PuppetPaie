//! CE QUI PART POUR DEBOGUER : le plus possible sur la machine, rien sur les salaries -- c'est le pacte avec
//! l'utilisateur. Avec chaque envoi part l'historique des actions des trois dernieres minutes.
//!
//! Un incident `{"route": "mois"}` seul ne dit ni la charge, ni ce que la page faisait, ni l'endroit du code. Ce module
//! tient, en UN SEUL ENDROIT, tout ce qu'un retour (« Un probleme, une
//! idee ? ») et chaque incident portent de plus :
//!
//! - LA FRONTIERE (`en_forme`, `texte_en_forme`) : rien qui designe une personne ne passe. Noms, prenoms, NIR, adresses,
//!   IBAN, BIC, naissances, courriels, telephones et secrets sont REMPLACES PAR LEUR FORME (« salarie n°2 », « NIR
//!   present, cle valide », « texte de 12 car. ») ; la compagnie n'est pas nommee (« [compagnie] »). Les montants, dates,
//!   nombres, codes, natures, motifs, spectacles et lieux, qui servent a reproduire, RESTENT. Deux serrures : la cle du
//!   champ (`genre_de_cle`) et, dans tout texte, les personnes que la compagnie connait (`config/salaries.json`,
//!   `config/entreprise.json`) et les motifs (NIR, IBAN, BIC, courriel, telephone, jeton). C'est ICI que la page et le
//!   moteur sont mis en forme : la page envoie ce qu'elle a vu, le moteur decide (regle 2).
//! - LE FIL (`noter_route`, `noter_gestes`, `noter_moteur`, `fil`) : les trois dernieres minutes de la fenetre, en memoire
//!   seulement (rien ne s'ecrit sur le disque au-dela du journal des incidents, qui l'emporte) -- chaque geste de la page
//!   (clic, champ modifie, boite ouverte, console), chaque route (sa charge mise en forme, sa duree, son issue), ce que la
//!   veille et le fond ont joue. Une ligne par action, l'heure relative (« -2:41 ») ; les repetitions se comptent.
//! - LA TRACE D'UN REFUS (`noter_echec`, `trace_du_refus`) : l'endroit du code ou chaque echec a ete construit
//!   (`#[track_caller]` sur `socle::echec::erreur` et sur les conversions de `socle::echec::Echec`), et les paniques ; un refus de route
//!   retrouve la chaine des echecs construits pendant la route, et celui qui porte son message.
//! - LA MACHINE (`machine`) : version et build exacts, systeme, architecture, coeurs, memoire, disque libre, installation
//!   (installee, portable, atelier ; chemin sans nom de session), edition (bride, debloque), compagnie fictive ou reelle
//!   (non nommee), duree depuis le lancement, dernieres mises a jour, et ce que la page a vu (ecran, echelle, langue,
//!   fuseau, moteur de rendu).
//! - L'ETAT D'UN MOIS (`etat_du_mois`) : ce que l'etat garde en dit, et ses depots.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde_json::{json, Map, Value};

use socle::echec::{Echoue, FENETRE_MS};
use socle::temps::maintenant_ms;
/// Au plus tant d'actions gardees en memoire, et rendues (les plus recentes).
const FIL_MAX: usize = 600;
const FIL_RENDU: usize = 150;
/// Une ligne du fil, au plus.
const LIGNE_MAX: usize = 400;
/// Une charge, dans une ligne du fil.
const CHARGE_LIGNE_MAX: usize = 260;
/// Un texte libre mis en forme, au plus (au-dela : « … (N car.) »).
const TEXTE_MAX: usize = 300;
/// Une repetition (meme action, meme issue) se compte dans la ligne d'avant si elle arrive dans ce delai.
const REPETITION_MS: u64 = 60_000;

fn verrou<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}


fn demarrage() -> &'static Instant {
    static D: OnceLock<Instant> = OnceLock::new();
    D.get_or_init(Instant::now)
}

/// Au lancement de la fenetre : l'heure du lancement, et les paniques qui se notent (leur endroit) avant d'etre dites.
pub fn installer() {
    demarrage();
    static FAIT: OnceLock<()> = OnceLock::new();
    FAIT.get_or_init(|| {
        let avant = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let ou = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "?".into());
            socle::echec::noter_echec_a(&ou, Some("panique"), &message);
            avant(info);
        }));
    });
}

// =================================================================================================== la frontiere

/// Ce qu'une cle de champ dit de sa valeur.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Genre {
    Secret,
    Nir,
    Iban,
    Bic,
    Naissance,
    Nom,
    Adresse,
    CodePostal,
    Courriel,
    Telephone,
    Compagnie,
    Salarie,
}

/// Les mots d'une cle, en minuscules sans accents : `nomAffichage` -> [nom, affichage] ; `date_naissance` -> [date, naissance].
fn mots_de_cle(k: &str) -> Vec<String> {
    let mut mots = Vec::new();
    let mut cur = String::new();
    let mut prec_min = false;
    for c in declarations::champs::sans_accents(k).chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                mots.push(std::mem::take(&mut cur));
            }
            prec_min = false;
            continue;
        }
        if c.is_uppercase() && prec_min && !cur.is_empty() {
            mots.push(std::mem::take(&mut cur));
        }
        prec_min = c.is_lowercase() || c.is_ascii_digit();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        mots.push(cur);
    }
    mots
}

/// Le genre d'une cle, s'il dit une personne, la compagnie ou un secret. Les autres cles laissent passer leur valeur
/// (mise en forme comme tout texte : `texte_en_forme`).
fn genre_de_cle(k: &str) -> Option<Genre> {
    let mots = mots_de_cle(k);
    let joint = mots.concat();
    let a = |m: &str| mots.iter().any(|x| x == m);
    let contient = |m: &str| joint.contains(m);
    if contient("motdepasse") || contient("password") || a("mdp") || contient("jeton") || contient("token") || contient("secret") || a("authorization") {
        return Some(Genre::Secret);
    }
    if a("nir") || a("ntt") || contient("securitesociale") || contient("numerosecu") {
        return Some(Genre::Nir);
    }
    if a("iban") || a("rib") {
        return Some(Genre::Iban);
    }
    if a("bic") {
        return Some(Genre::Bic);
    }
    if mots.iter().any(|m| m.contains("naissance")) {
        return Some(Genre::Naissance);
    }
    if a("email") || a("courriel") || a("mail") || a("mel") || contient("courriel") {
        return Some(Genre::Courriel);
    }
    if a("telephone") || a("tel") || a("portable") || a("mobile") || a("fax") || a("gsm") {
        return Some(Genre::Telephone);
    }
    if a("siren") || a("siret") || a("nic") || contient("raisonsociale") || a("enseigne") || contient("urssafcompte") || a("licence") {
        return Some(Genre::Compagnie);
    }
    if (a("code") && a("postal")) || a("cp") || contient("codepostal") {
        return Some(Genre::CodePostal);
    }
    if a("adresse") || a("rue") || a("voie") || a("complement") || a("batiment") {
        return Some(Genre::Adresse);
    }
    // Un salarie se designe par sa cle (son nom) : `salarie`, `salaries`, `cle_salarie`.
    if mots.iter().any(|m| m == "salarie" || m == "salaries") && !a("nombre") && !a("code") && !a("statut") && !a("categorie") {
        return Some(Genre::Salarie);
    }
    // Un nom de personne ; pas celui d'un fichier, d'un spectacle, d'un metier, d'un organisme...
    const PAS_UNE_PERSONNE: [&str; 14] = [
        "fichier", "spectacle", "metier", "emploi", "organisme", "rubrique", "convention", "modele", "dossier", "route", "piece",
        "caisse", "champ", "onglet",
    ];
    let personne = ["nom", "noms", "prenom", "prenoms", "signataire", "destinataire", "beneficiaire", "titulaire", "conjoint", "representant"];
    if mots.iter().any(|m| personne.contains(&m.as_str())) && !mots.iter().any(|m| PAS_UNE_PERSONNE.contains(&m.as_str())) {
        return Some(Genre::Nom);
    }
    None
}

/// Les personnes et la compagnie que la compagnie connait, et ce par quoi on les remplace.
struct Personnes {
    /// Une expression par personne (tout ce qui la nomme), et son remplacement : « salarie n°2 », « compagnie ».
    une: Vec<(Regex, String)>,
    /// Toutes a la fois : un texte qui ne s'y accroche pas n'a rien a remplacer.
    toutes: Option<Regex>,
}

/// Une lettre, sans regarder ses accents ni sa casse.
fn classe_de(c: char) -> String {
    let variantes = match c {
        'a' => "aàâäáãå",
        'e' => "eéèêë",
        'i' => "iîïíì",
        'o' => "oôöóòõ",
        'u' => "uùûüú",
        'c' => "cç",
        'y' => "yÿý",
        'n' => "nñ",
        _ => return regex::escape(&c.to_string()),
    };
    format!("[{}]", variantes)
}

/// L'expression d'un nom : sans accents ni casse, les separateurs (espace, tiret, apostrophe) au choix.
fn motif_de(nom: &str) -> Option<String> {
    let base = declarations::champs::sans_accents(nom.trim()).to_lowercase();
    let mut s = String::new();
    let mut sep = false;
    for c in base.chars() {
        if c.is_whitespace() || c == '-' || c == '\'' || c == '\u{2019}' || c == '_' {
            sep = true;
            continue;
        }
        if sep && !s.is_empty() {
            s.push_str(r"[\s\-'’_]+");
        }
        sep = false;
        s.push_str(&classe_de(c));
    }
    // Deux caracteres au moins, et pas un nombre seul (un code salarie « 1 » ne se cherche pas dans les montants).
    let lettres = base.chars().filter(|c| c.is_alphabetic()).count();
    let chiffres = base.chars().filter(|c| c.is_ascii_digit()).count();
    if base.chars().filter(|c| c.is_alphanumeric()).count() < 2 || (lettres == 0 && chiffres < 9) {
        return None;
    }
    Some(s)
}

fn signature(p: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn lire_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Le numero d'un salarie : son `code_salarie` (celui des bulletins), sinon son rang dans les fiches.
/// La fiche datee, chaque champ a sa derniere valeur.
fn derniere_valeur(datee: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (k, v) in datee.as_object().into_iter().flatten() {
        if let Some(x) = droit::contexte::toutes_les_valeurs(Some(v)).last() {
            out.insert(k.clone(), (*x).clone());
        }
    }
    Value::Object(out)
}

fn numero_de(fiche: &Value, rang: usize) -> String {
    match fiche.get("code_salarie") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => format!("{}", rang + 1),
    }
}

/// Les champs d'une fiche qui nomment la personne : ceux-la se cherchent dans tout texte.
const CHAMPS_QUI_NOMMENT: [&str; 13] = [
    "nom", "prenom", "prenoms", "nom_affichage", "prenom_affichage", "nom_usage", "nom_naissance", "email", "courriel",
    "adresse", "adresse_affichage", "telephone", "nir",
];
const CHAMPS_DE_LA_COMPAGNIE: [&str; 11] = [
    "raison_sociale", "nom_affichage", "enseigne", "siren", "siret", "adresse", "adresse_affichage", "urssaf_compte",
    "licence_spectacle", "email", "telephone",
];

fn construire(racine: &Path) -> Personnes {
    let mut une: Vec<(Regex, String)> = Vec::new();
    let mut tous: Vec<String> = Vec::new();
    let mut ajouter = |noms: Vec<String>, quoi: String| {
        let mut motifs: Vec<String> = noms.iter().filter_map(|n| motif_de(n)).collect();
        motifs.sort_by_key(|m| std::cmp::Reverse(m.len()));
        motifs.dedup();
        if motifs.is_empty() {
            return;
        }
        let alt = format!(r"(?i)\b(?:{})\b", motifs.join("|"));
        if let Ok(r) = Regex::new(&alt) {
            tous.extend(motifs);
            une.push((r, quoi));
        }
    };
    if let Some(Value::Object(fiches)) = lire_json(&racine.join("config").join("salaries.json")) {
        for (rang, (cle, datee)) in fiches.iter().filter(|(k, _)| !k.starts_with('_')).enumerate() {
            let mut noms = vec![cle.clone()];
            for c in CHAMPS_QUI_NOMMENT {
                for v in droit::contexte::toutes_les_valeurs(datee.get(c)) {
                    if let Value::String(s) = v {
                        noms.push(s.clone());
                    }
                }
            }
            let fiche = &derniere_valeur(datee);
            // Le prenom et le nom ensemble, dans les deux ordres : « Fanchon Brioche » devient un seul « [salarie n°1] ».
            let texte = |c: &str| fiche.get(c).and_then(Value::as_str).unwrap_or("").trim().to_string();
            for (p, n) in [("prenom", "nom"), ("prenom_affichage", "nom_affichage"), ("prenom_affichage", "nom_usage")] {
                let (p, n) = (texte(p), texte(n));
                if !p.is_empty() && !n.is_empty() {
                    noms.push(format!("{} {}", p, n));
                    noms.push(format!("{} {}", n, p));
                }
            }
            ajouter(noms, format!("salarié n°{}", numero_de(fiche, rang)));
        }
    }
    if let Some(e @ Value::Object(_)) = lire_json(&racine.join("config").join("entreprise.json")) {
        let mut noms = Vec::new();
        for c in CHAMPS_DE_LA_COMPAGNIE {
            for s in droit::contexte::toutes_les_valeurs(e.get(c)).into_iter().filter_map(Value::as_str) {
                noms.push(s.to_string());
                // « Le Fil tendu (démonstration) » : le nom sans sa parenthese aussi.
                if let Some((avant, _)) = s.split_once(" (") {
                    noms.push(avant.to_string());
                }
            }
        }
        ajouter(noms, "compagnie".to_string());
    }
    tous.sort_by_key(|m| std::cmp::Reverse(m.len()));
    let toutes = if tous.is_empty() { None } else { Regex::new(&format!(r"(?i)\b(?:{})\b", tous.join("|"))).ok() };
    Personnes { une, toutes }
}

type Signatures = (Option<(SystemTime, u64)>, Option<(SystemTime, u64)>);

/// Les personnes de la compagnie `racine`, relues quand ses fiches changent.
fn personnes(racine: Option<&Path>) -> Arc<Personnes> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (Signatures, Arc<Personnes>)>>> = OnceLock::new();
    let Some(racine) = racine else { return Arc::new(Personnes { une: Vec::new(), toutes: None }) };
    let sig = (signature(&racine.join("config/salaries.json")), signature(&racine.join("config/entreprise.json")));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((s, p)) = verrou(cache).get(racine) {
        if *s == sig {
            return p.clone();
        }
    }
    let p = Arc::new(construire(racine));
    verrou(cache).insert(racine.to_path_buf(), (sig, p.clone()));
    p
}

fn motif_courriel() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r#"[^\s<>()\[\]"',;:]+@[^\s<>()\[\]"',;:]+\.[A-Za-z]{2,}"#).expect("motif"))
}

/// Un IBAN en clair (lettres comprises, avec ou sans espaces) : le caviardage des incidents ne voit que les IBAN francais
/// tout en chiffres.
fn motif_iban() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?\b").expect("motif"))
}

fn motif_telephone() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"(?:\+33[\s.\-]?|\b0)[1-9](?:[\s.\-]?\d{2}){4}\b").expect("motif"))
}

/// « 12 car. », au singulier comme au pluriel.
fn car(n: usize) -> String {
    format!("{} car.", n)
}

fn nb_car(s: &str) -> usize {
    s.chars().count()
}

/// Un texte libre, mis en forme : les donnees encodees (une capture, un fichier) deviennent leur taille ; NIR, IBAN, BIC,
/// jeton, courriel et telephone deviennent ce qu'ils sont ; toute personne et la compagnie connues deviennent leur numero ;
/// le reste passe, coupe a `limite` caracteres.
pub fn texte_en_forme(racine: Option<&Path>, s: &str, limite: usize) -> String {
    texte_avec(&personnes(racine), s, limite)
}

fn texte_avec(p: &Personnes, s: &str, limite: usize) -> String {
    let n = nb_car(s);
    let tete: String = s.chars().take(200).collect();
    if n > 200 && !tete.chars().any(char::is_whitespace) {
        return format!("(données de {})", car(n));
    }
    let (t, _) = crate::compagnie::incidents::caviarder(s);
    let t = motif_courriel().replace_all(&t, "[adresse courriel]").into_owned();
    let t = motif_telephone().replace_all(&t, "[téléphone]").into_owned();
    let mut t = motif_iban().replace_all(&t, "[IBAN]").into_owned();
    if p.toutes.as_ref().is_some_and(|r| r.is_match(&t)) {
        for (r, quoi) in &p.une {
            t = r.replace_all(&t, format!("[{}]", quoi).as_str()).into_owned();
        }
    }
    let n = nb_car(&t);
    if n > limite {
        let coupe: String = t.chars().take(limite).collect();
        return format!("{}… ({})", coupe, car(n));
    }
    t
}

/// La cle de controle d'un NIR (13 chiffres, Corse 2A/2B comprise) : 97 - (NIR mod 97).
fn cle_nir(n13: &str) -> Option<u64> {
    let corse = match &n13[5..7] {
        "2A" | "2a" => Some(1_000_000u64),
        "2B" | "2b" => Some(2_000_000u64),
        _ => None,
    };
    let chiffres: String = n13.chars().map(|c| if c.is_ascii_alphabetic() { '0' } else { c }).collect();
    let v: u64 = chiffres.parse().ok()?;
    let v = match corse {
        Some(m) => v.checked_sub(m)?,
        None => v,
    };
    Some(97 - v % 97)
}

fn forme_nir(s: &str, cle_soeur: Option<&str>) -> String {
    let c: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if c.is_empty() {
        return String::new();
    }
    let lisible = |x: &str| x.len() == 13 && x.chars().enumerate().all(|(i, ch)| ch.is_ascii_digit() || ((i == 5 || i == 6) && "ABab".contains(ch)));
    let verdict = |n13: &str, cle: Option<&str>| match (cle_nir(n13), cle.and_then(|k| k.trim().parse::<u64>().ok())) {
        (Some(attendue), Some(k)) if attendue == k => "NIR présent, clé valide".to_string(),
        (Some(_), Some(_)) => "NIR présent, clé invalide".to_string(),
        (Some(_), None) => "NIR présent (13 car.), sans clé".to_string(),
        (None, _) => "NIR présent, illisible".to_string(),
    };
    if c.is_ascii() && c.len() == 15 && lisible(&c[..13]) {
        return verdict(&c[..13], Some(&c[13..]));
    }
    if lisible(&c) {
        return verdict(&c, cle_soeur);
    }
    if c.len() <= 2 && c.chars().all(|x| x.is_ascii_digit()) {
        return "clé de NIR présente".to_string();
    }
    format!("NIR de forme inattendue ({})", car(nb_car(&c)))
}

fn forme_iban(s: &str) -> String {
    let c: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    if c.len() < 5 || !c.chars().all(|x| x.is_ascii_alphanumeric()) {
        return format!("IBAN de forme inattendue ({})", car(nb_car(&c)));
    }
    let tourne = format!("{}{}", &c[4..], &c[..4]);
    let mut reste: u64 = 0;
    for ch in tourne.chars() {
        let v = ch.to_digit(36).unwrap_or(0) as u64;
        reste = if v >= 10 { (reste * 100 + v) % 97 } else { (reste * 10 + v) % 97 };
    }
    format!("IBAN {} de {}, clé {}", &c[..2], car(c.len()), if reste == 1 { "valide" } else { "invalide" })
}

fn forme_date(s: &str) -> String {
    let b = s.as_bytes();
    let iso = b.len() == 10 && b[4] == b'-' && b[7] == b'-';
    let fr = b.len() == 10 && b[2] == b'/' && b[5] == b'/';
    if iso || fr {
        "date présente (bien formée)".to_string()
    } else {
        format!("texte de {}", car(nb_car(s)))
    }
}

fn forme_courriel(s: &str) -> String {
    let t = s.trim();
    match t.split_once('@') {
        Some((a, d)) if !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.') && !t.contains(char::is_whitespace) => {
            "adresse courriel bien formée".to_string()
        }
        _ => format!("adresse courriel mal formée ({})", car(nb_car(t))),
    }
}

/// Un salarie designe par sa cle (son nom) : son numero, ou qu'il n'est pas dans les fiches -- ce qu'un refus
/// « salarie introuvable » demande de savoir.
fn forme_salarie(p: &Personnes, s: &str) -> String {
    for (r, quoi) in &p.une {
        if let Some(m) = r.find(s.trim()) {
            if m.start() == 0 && m.end() == s.trim().len() {
                return quoi.clone();
            }
        }
    }
    format!("salarié absent des fiches ({})", car(nb_car(s)))
}

/// Les sortes de fiches de la page (`js/fiche.js`, `FICHE_ACTIVE`).
const SORTES_DE_FICHE: [&str; 12] = [
    "salarie", "salaries", "entreprise", "spectacle", "spectacles", "organismes", "metiers", "bulletin", "coffre", "identite", "importer", "permanents",
];

/// La forme d'une valeur scalaire sous une cle d'un genre donne.
fn scalaire_de_genre(p: &Personnes, g: Genre, v: &Value, soeurs: Option<&Map<String, Value>>) -> Value {
    let s = match v {
        Value::Null => return Value::Null,
        Value::Bool(b) => return Value::Bool(*b),
        Value::String(s) if s.is_empty() => return json!(""),
        Value::String(s) => s.clone(),
        autre => autre.to_string(),
    };
    let f = match g {
        Genre::Secret => "[secret retiré]".to_string(),
        Genre::Nir => forme_nir(&s, soeurs.and_then(|m| m.get("nir_cle")).and_then(Value::as_str)),
        Genre::Iban => forme_iban(&s),
        Genre::Bic => format!("BIC de {}", car(nb_car(s.trim()))),
        Genre::Naissance => forme_date(&s),
        // `nom` nomme aussi la sorte d'une fiche (`enregistrer-fiche` : « salaries », « entreprise ») : ces mots passent.
        Genre::Nom if SORTES_DE_FICHE.contains(&s.as_str()) => s.clone(),
        Genre::Nom => {
            let connu = forme_salarie(p, &s);
            if connu.starts_with("salarié absent") {
                format!("nom de {}", car(nb_car(&s)))
            } else {
                connu
            }
        }
        Genre::Adresse => format!("adresse de {}", car(nb_car(&s))),
        Genre::CodePostal => format!("code postal de {}", car(nb_car(s.trim()))),
        Genre::Courriel => forme_courriel(&s),
        Genre::Telephone => format!("téléphone de {} chiffres", s.chars().filter(char::is_ascii_digit).count()),
        Genre::Compagnie => format!("identifiant de la compagnie ({})", car(nb_car(s.trim()))),
        Genre::Salarie => forme_salarie(p, &s),
    };
    Value::String(f)
}

/// La forme d'une valeur JSON (une charge de route, un champ de la page) : la frontiere, en un seul endroit.
pub fn en_forme(racine: Option<&Path>, v: &Value) -> Value {
    let p = personnes(racine);
    forme_avec(&p, v, None, None, 0)
}

/// La forme d'une valeur saisie dans un champ de la page nomme `nom` (son `name`, son `id`) : comme sous la cle `nom`.
pub fn champ_en_forme(racine: Option<&Path>, nom: &str, v: &Value) -> Value {
    let p = personnes(racine);
    forme_avec(&p, v, genre_de_cle(nom), None, 0)
}

fn forme_avec(p: &Personnes, v: &Value, genre: Option<Genre>, soeurs: Option<&Map<String, Value>>, prof: usize) -> Value {
    if prof > 14 {
        return json!("…");
    }
    match v {
        Value::Object(o) => {
            let mut m = Map::new();
            for (k, x) in o {
                // Une cle peut etre un nom (des fiches rangees par salarie) : elle se met en forme comme un texte.
                let cle = texte_avec(p, k, 80);
                let g = genre.or_else(|| genre_de_cle(k));
                m.insert(cle, forme_avec(p, x, g, Some(o), prof + 1));
            }
            Value::Object(m)
        }
        Value::Array(a) => {
            const MAX: usize = 60;
            let mut out: Vec<Value> = a.iter().take(MAX).map(|x| forme_avec(p, x, genre, soeurs, prof + 1)).collect();
            if a.len() > MAX {
                out.push(json!(format!("… et {} de plus", a.len() - MAX)));
            }
            Value::Array(out)
        }
        scalaire => match genre {
            Some(g) => scalaire_de_genre(p, g, scalaire, soeurs),
            None => match scalaire {
                Value::String(s) => Value::String(texte_avec(p, s, TEXTE_MAX)),
                autre => autre.clone(),
            },
        },
    }
}

/// Chaque texte d'une valeur deja ecrite (une ligne du journal des incidents, au moment ou elle part), mis en forme sans
/// etre coupe : une ligne notee avant la frontiere (ou avant qu'un salarie entre dans les fiches) ne part pas avec un nom.
/// Les cles ne disent rien ici : une valeur deja mise en forme (« salarié n°2 ») le reste.
pub fn textes_en_forme(racine: Option<&Path>, v: &Value) -> Value {
    fn passe(p: &Personnes, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(texte_avec(p, s, usize::MAX)),
            Value::Array(a) => Value::Array(a.iter().map(|x| passe(p, x)).collect()),
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (texte_avec(p, k, usize::MAX), passe(p, x))).collect()),
            autre => autre.clone(),
        }
    }
    passe(&personnes(racine), v)
}

/// Une valeur en une ligne, coupee a `limite` caracteres.
pub fn compact(v: &Value, limite: usize) -> String {
    let t = v.to_string();
    let n = nb_car(&t);
    if n <= limite {
        t
    } else {
        format!("{}… ({})", t.chars().take(limite).collect::<String>(), car(n))
    }
}

// ========================================================================================================== le fil

struct Entree {
    premier: u64,
    dernier: u64,
    fois: u32,
    texte: String,
    /// Ce qui fait qu'une action est la meme qu'une autre (une lecture qui se repete toutes les secondes).
    cle: String,
}

fn le_fil() -> &'static Mutex<VecDeque<Entree>> {
    static F: OnceLock<Mutex<VecDeque<Entree>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Ajoute une action au fil, a son heure (`t`, en ms) : une repetition recente se compte dans sa ligne.
fn ajouter(t: u64, texte: String, cle: String) {
    let texte = if nb_car(&texte) > LIGNE_MAX { format!("{}…", texte.chars().take(LIGNE_MAX).collect::<String>()) } else { texte };
    let mut f = verrou(le_fil());
    let limite = maintenant_ms().saturating_sub(FENETRE_MS);
    while f.front().is_some_and(|e| e.dernier < limite) || f.len() >= FIL_MAX {
        f.pop_front();
    }
    let n = f.len();
    for e in f.iter_mut().skip(n.saturating_sub(3)) {
        if e.cle == cle && t >= e.dernier && t - e.dernier < REPETITION_MS {
            e.fois += 1;
            e.dernier = t;
            return;
        }
    }
    // A sa place dans le temps : un geste de la page arrive avec la route qu'il a declenchee, mais il est d'avant elle.
    let i = f.iter().rposition(|e| e.premier <= t).map_or(0, |i| i + 1);
    f.insert(i, Entree { premier: t, dernier: t, fois: 1, texte, cle });
}

/// « -2:41 » : il y a deux minutes et quarante et une secondes.
fn relatif(maintenant: u64, t: u64) -> String {
    let s = maintenant.saturating_sub(t) / 1000;
    format!("-{}:{:02}", s / 60, s % 60)
}

/// Le fil des trois dernieres minutes, une ligne par action, la plus ancienne d'abord ; vide hors de la fenetre.
pub fn fil(maintenant: u64) -> Vec<String> {
    let f = verrou(le_fil());
    let limite = maintenant.saturating_sub(FENETRE_MS);
    let gardees: Vec<&Entree> = f.iter().filter(|e| e.dernier >= limite && e.premier <= maintenant + 5_000).collect();
    let debut = gardees.len().saturating_sub(FIL_RENDU);
    gardees[debut..]
        .iter()
        .map(|e| {
            let mut l = format!("{}  {}", relatif(maintenant, e.premier), e.texte);
            if e.fois > 1 {
                l.push_str(&format!("  (×{}, la dernière à {})", e.fois, relatif(maintenant, e.dernier)));
            }
            l
        })
        .collect()
}

/// Vide le fil (les essais).
pub fn oublier_le_fil() {
    verrou(le_fil()).clear();
    verrou(les_ecritures()).clear();
}

/// Une route servie par la fenetre : `source` « page » (la page l'a demandee) ou « fond » (la veille, le filet, l'etat des
/// mois...) ; sa charge mise en forme, sa duree, son issue.
pub fn noter_route(racine: &Path, source: &str, nom: &str, charge: &Value, duree: Duration, issue: Result<(), (&str, &str)>) {
    let forme = en_forme(Some(racine), charge);
    let ch = match &forme {
        Value::Object(o) if o.is_empty() => String::new(),
        Value::Null => String::new(),
        v => format!(" {}", compact(v, CHARGE_LIGNE_MAX)),
    };
    let ms = duree.as_millis();
    let (fin, cle_issue) = match issue {
        Ok(()) => (format!("→ ok, {} ms", ms), "ok".to_string()),
        Err((classe, message)) => {
            let m = texte_en_forme(Some(racine), message, 200);
            (format!("→ refus {} « {} », {} ms", classe, m, ms), format!("refus {} {}", classe, m))
        }
    };
    let quoi = if source == "page" { "route" } else { "fond, route" };
    let t = maintenant_ms().saturating_sub(ms as u64);
    // DE QUOI REJOUER : la ligne du fil coupe la charge ; une ecriture de la page (ou un refus) la garde entiere, mise en
    // forme -- la saisie qui a mene au refus se rejoue sur une fictive (`ecritures`).
    if source == "page" && (crate::routes::est_une_ecriture(nom) || issue.is_err()) {
        let mut e = verrou(les_ecritures());
        while e.len() >= ECRITURES_MAX || e.front().is_some_and(|x| x.0 + FENETRE_MS < t) {
            e.pop_front();
        }
        let issue_texte = fin.rsplit_once(", ").map_or(fin.clone(), |(a, _)| a.to_string());
        // La meme route, la meme charge, la meme issue que la derniere : le fil la compte, elle ne se garde qu'une fois.
        match e.back_mut() {
            Some(d) if d.1 == nom && d.2 == forme && d.3 == issue_texte => d.0 = t,
            _ => e.push_back((t, nom.to_string(), forme.clone(), issue_texte)),
        }
    }
    ajouter(t, format!("{} {}{} {}", quoi, nom, ch, fin), format!("{}|{}|{}|{}", source, nom, ch, cle_issue));
}

/// Les ecritures (et les refus) de la page gardees entieres : au plus tant, dans la fenetre.
const ECRITURES_MAX: usize = 12;

type Ecriture = (u64, String, Value, String);

fn les_ecritures() -> &'static Mutex<VecDeque<Ecriture>> {
    static E: OnceLock<Mutex<VecDeque<Ecriture>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Les `n` dernieres ecritures (et refus) de la page dans les trois minutes, la plus ancienne d'abord : l'heure relative,
/// la route, sa charge ENTIERE mise en forme, son issue. De quoi rejouer, sur une fictive, ce qui a mene a un refus.
pub fn ecritures(maintenant: u64, n: usize) -> Vec<Value> {
    let e = verrou(les_ecritures());
    let gardees: Vec<&Ecriture> = e.iter().filter(|x| x.0 + FENETRE_MS >= maintenant).collect();
    gardees[gardees.len().saturating_sub(n)..]
        .iter()
        .map(|(t, nom, charge, issue)| json!({"il_y_a": relatif(maintenant, *t), "route": nom, "charge": charge, "issue": issue}))
        .collect()
}

/// Une action du moteur qui n'est pas une route (la veille qui commence un tour, un releve...).
pub fn noter_moteur(texte: &str) {
    ajouter(maintenant_ms(), texte.to_string(), format!("moteur|{}", texte));
}

fn page_vue() -> &'static Mutex<Option<Value>> {
    static P: OnceLock<Mutex<Option<Value>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

/// Un texte de la page, mis en forme et coupe.
fn texte_page(racine: &Path, v: Option<&Value>, limite: usize) -> String {
    match v {
        Some(Value::String(s)) => texte_en_forme(Some(racine), s.trim(), limite),
        Some(Value::Null) | None => String::new(),
        Some(autre) => texte_en_forme(Some(racine), &autre.to_string(), limite),
    }
}

/// Ce que la page a vu et fait depuis sa derniere demande (`X-Gestes`, js/tuyau.js) : `{"page": {...}, "gestes": [...],
/// "perdus": n}`. La page envoie ce qu'elle a vu ; la mise en forme se decide ici.
pub fn noter_gestes(racine: &Path, lot: &Value) {
    if let Some(Value::Object(p)) = lot.get("page") {
        let mut m = Map::new();
        for (k, v) in p {
            let v = match v {
                Value::String(s) => Value::String(texte_en_forme(Some(racine), s, 300)),
                Value::Number(_) | Value::Bool(_) | Value::Null => v.clone(),
                autre => en_forme(Some(racine), autre),
            };
            m.insert(texte_en_forme(None, k, 40), v);
        }
        *verrou(page_vue()) = Some(Value::Object(m));
        ajouter(maintenant_ms(), "page chargée".to_string(), format!("page|{}", maintenant_ms()));
    }
    if let Some(n) = lot.get("perdus").and_then(Value::as_u64).filter(|n| *n > 0) {
        ajouter(maintenant_ms(), format!("({} gestes de la page perdus : trop nombreux)", n), "perdus".into());
    }
    let Some(Value::Array(gestes)) = lot.get("gestes") else { return };
    let maintenant = maintenant_ms();
    for g in gestes.iter().take(200) {
        // L'heure du geste (celle de la page, sur la meme machine) ; sans heure lisible, ou dans le futur, maintenant. Plus
        // vieux que la fenetre : il n'a rien a faire dans le fil.
        let t = g.get("t").and_then(Value::as_u64).filter(|t| *t <= maintenant + 5_000).unwrap_or(maintenant);
        if t + FENETRE_MS < maintenant {
            continue;
        }
        let quoi = g.get("quoi").and_then(Value::as_str).unwrap_or("");
        let texte = match quoi {
            "clic" => {
                let c = g.get("cible").cloned().unwrap_or(Value::Null);
                let etiquette = texte_page(racine, c.get("texte"), 80);
                let mut ou = texte_page(racine, c.get("balise"), 20);
                let id = texte_page(racine, c.get("id"), 60);
                if !id.is_empty() {
                    ou.push('#');
                    ou.push_str(&id);
                }
                let classe = texte_page(racine, c.get("classe"), 60);
                if !classe.is_empty() {
                    ou.push('.');
                    ou.push_str(&classe);
                }
                let vue = texte_page(racine, g.get("vue"), 30);
                format!("clic « {} » ({}){}", etiquette, ou, if vue.is_empty() { String::new() } else { format!(" — écran {}", vue) })
            }
            "champ" => {
                let nom = texte_page(racine, g.get("nom"), 60);
                let genre = texte_page(racine, g.get("type"), 20);
                let valeur = if g.get("secret").and_then(Value::as_bool) == Some(true) {
                    json!("[secret retiré]")
                } else {
                    champ_en_forme(Some(racine), &nom, g.get("valeur").unwrap_or(&Value::Null))
                };
                let v = match &valeur {
                    Value::Bool(true) => "coché".to_string(),
                    Value::Bool(false) => "décoché".to_string(),
                    autre => compact(autre, 120),
                };
                // Une case dit ce qu'elle coche (un salarie -- qui devient son numero --, un jour, une rubrique).
                let de = texte_page(racine, g.get("de"), 80);
                let de = if de.is_empty() || de == "on" { String::new() } else { format!(" « {} »", de) };
                format!("champ {}{} ({}) = {}", nom, de, genre, v)
            }
            "boite" => format!("boîte « {} »", texte_page(racine, g.get("titre"), 100)),
            "console" => format!("console.{} : {}", texte_page(racine, g.get("niveau"), 10), texte_page(racine, g.get("message"), 240)),
            "ecran" => format!("écran {}", texte_page(racine, g.get("vue"), 60)),
            autre => format!("{} {}", texte_page(racine, Some(&json!(autre)), 20), compact(&en_forme(Some(racine), g), 200)),
        };
        ajouter(t, texte.clone(), format!("geste|{}", texte));
    }
}

/// Ce que la page a dit d'elle-meme a son chargement (ecran, echelle, langue, fuseau, moteur de rendu).
pub fn page() -> Value {
    verrou(page_vue()).clone().unwrap_or(Value::Null)
}

// ===================================================================================================== les echecs

/// La trace d'un refus de route : l'endroit ou son echec a ete construit (celui qui porte son message, sur quelque fil
/// que ce soit : un etat de mois se calcule dans le fil de fond), et la chaine des echecs construits avant lui sur le
/// meme fil, depuis `debut` (le debut de la route) ou cinq secondes avant lui. `None` : aucun echec note ne s'y rattache.
pub fn trace_du_refus(racine: &Path, message: &str, debut: u64) -> Option<String> {
    let l = socle::echec::echecs_notes();
    let tete: String = message.chars().take(300).collect();
    let porte = |e: &Echoue| !e.message.is_empty() && (e.message == tete || tete.contains(&e.message) || e.message.contains(tete.as_str()));
    let ici = socle::echec::fil_courant();
    let (fil, jusqu_a, depuis, endroit) = match l.iter().rev().find(|e| porte(e)) {
        Some(e) => (e.fil.clone(), e.t, if e.fil == ici { debut.min(e.t) } else { e.t.saturating_sub(5_000) }, Some(e)),
        None => (ici, u64::MAX, debut, None),
    };
    let chaine: Vec<&Echoue> = l.iter().filter(|e| e.fil == fil && e.t >= depuis && e.t <= jusqu_a).collect();
    if chaine.is_empty() && endroit.is_none() {
        return None;
    }
    let debut_chaine = chaine.len().saturating_sub(12);
    let mut t = String::new();
    match endroit {
        Some(e) => t.push_str(&format!("endroit : {} ({}), fil « {} »\n", e.ou, e.classe, e.fil)),
        None => t.push_str("endroit : aucun echec note ne porte ce message (construit sans `erreur`)\n"),
    }
    t.push_str("echecs construits avant lui sur ce fil, du plus ancien au plus recent :\n");
    for e in &chaine[debut_chaine..] {
        t.push_str(&format!("  {}  {}  {}\n", e.ou, e.classe, texte_en_forme(Some(racine), &e.message, 200)));
    }
    Some(t)
}

// ===================================================================================================== la machine

/// La memoire du poste, en Mo : (totale, disponible).
#[cfg(target_os = "linux")]
fn memoire() -> (Option<u64>, Option<u64>) {
    let t = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let lire = |cle: &str| t.lines().find_map(|l| l.strip_prefix(cle)).and_then(|r| r.split_whitespace().next()).and_then(|k| k.parse::<u64>().ok()).map(|k| k / 1024);
    (lire("MemTotal:"), lire("MemAvailable:"))
}

#[cfg(target_os = "macos")]
fn memoire() -> (Option<u64>, Option<u64>) {
    let total = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .map(|o| o / (1024 * 1024));
    (total, None)
}

#[cfg(windows)]
fn memoire() -> (Option<u64>, Option<u64>) {
    #[repr(C)]
    struct EtatMemoire {
        longueur: u32,
        charge: u32,
        physique_totale: u64,
        physique_libre: u64,
        pagination_totale: u64,
        pagination_libre: u64,
        virtuelle_totale: u64,
        virtuelle_libre: u64,
        etendue_libre: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(etat: *mut EtatMemoire) -> i32;
    }
    let mut e = EtatMemoire {
        longueur: std::mem::size_of::<EtatMemoire>() as u32,
        charge: 0,
        physique_totale: 0,
        physique_libre: 0,
        pagination_totale: 0,
        pagination_libre: 0,
        virtuelle_totale: 0,
        virtuelle_libre: 0,
        etendue_libre: 0,
    };
    // SAFETY : la structure est celle de MEMORYSTATUSEX, sa longueur posee comme l'appel l'exige.
    let ok = unsafe { GlobalMemoryStatusEx(&mut e) } != 0;
    if ok {
        (Some(e.physique_totale / (1024 * 1024)), Some(e.physique_libre / (1024 * 1024)))
    } else {
        (None, None)
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn memoire() -> (Option<u64>, Option<u64>) {
    (None, None)
}

/// Le disque libre la ou vit `p`, en Mo.
#[cfg(windows)]
fn disque_libre(p: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(dossier: *const u16, libre_appelant: *mut u64, total: *mut u64, libre_total: *mut u64) -> i32;
    }
    let large: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut libre, mut total, mut libre_total) = (0u64, 0u64, 0u64);
    // SAFETY : un chemin termine par zero, trois sorties vivantes.
    let ok = unsafe { GetDiskFreeSpaceExW(large.as_ptr(), &mut libre, &mut total, &mut libre_total) } != 0;
    ok.then_some(libre / (1024 * 1024))
}

#[cfg(not(windows))]
fn disque_libre(p: &Path) -> Option<u64> {
    let o = std::process::Command::new("df").arg("-Pk").arg(p).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    t.lines().nth(1)?.split_whitespace().nth(3)?.parse::<u64>().ok().map(|k| k / 1024)
}

/// « 1 h 12 min 5 s ».
fn duree_lisible(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{} h {} min {} s", h, m, s)
    } else if m > 0 {
        format!("{} min {} s", m, s)
    } else {
        format!("{} s", s)
    }
}

/// Un chemin du poste, sans le nom de la session : le dossier qui suit `Users` ou `home` devient « [session] ».
fn chemin_sans_session(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    crate::compagnie::incidents::motif_maison().replace_all(&s, "${1}[session]").into_owned()
}

/// La machine, en entier : tout ce qui aide a reproduire, rien qui designe la personne.
pub fn machine(racine: &Path) -> Value {
    let base = crate::interface::compagnies::base();
    let (totale, libre) = memoire();
    let installation = if base.join(crate::compagnie::coffre::MARQUEUR_PORTABLE).is_file() {
        "portable"
    } else if base.join("atelier").is_dir() {
        "atelier (dépôt de développement)"
    } else {
        "installée"
    };
    let maj: Vec<Value> = std::fs::read_to_string(crate::compagnie::maj::dossier_maj(&base).join("journal.log"))
        .map(|t| {
            let l: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
            l[l.len().saturating_sub(6)..].iter().map(|x| json!(texte_en_forme(None, &chemin_sans_session(Path::new(x)), 200))).collect()
        })
        .unwrap_or_default();
    let langue = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|x| !x.is_empty()));
    json!({
        "version": crate::compagnie::incidents::version(),
        "moteur": env!("PAIE_SOURCES"),
        "profil": if cfg!(debug_assertions) { "debug" } else { "release" },
        "systeme": crate::compagnie::incidents::machine(),
        "os": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "coeurs": std::thread::available_parallelism().map(|n| n.get()).ok(),
        "memoire_mo": {"totale": totale, "disponible": libre},
        "disque_libre_mo": disque_libre(racine),
        "installation": installation,
        "chemin": chemin_sans_session(&base),
        "edition": crate::compagnie::bridage::etat(),
        "compagnie": if crate::interface::compagnies::est_fictive(racine) { "fictive" } else { "réelle (non nommée)" },
        "langue_systeme": langue,
        "lance_depuis": duree_lisible(demarrage().elapsed()),
        "mises_a_jour": maj,
        "page": page(),
    })
}

// ===================================================================================================== l'etat d'un mois

/// L'etat utile de `mois` pour un refus : ses engagements (combien, combien valides) et ses depots (DSN, DPAE,
/// annule et remplace), mis en forme. `Null` si le mois n'est pas lisible.
pub fn etat_du_mois(racine: &Path, mois: &str) -> Value {
    let b = mois.as_bytes();
    if !(b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())) {
        return Value::Null;
    }
    let mut m = Map::new();
    let objets = engagement::stock::objets_du_mois(racine, mois).unwrap_or_default();
    m.insert("saisie".into(), json!(!objets.is_empty()));
    m.insert("engagements".into(), json!(objets.len()));
    m.insert("valides".into(), json!(objets.iter().filter(|o| o.est_valide()).count()));
    if let Ok(journal) = envois::journal::lu(racine) {
        let depots = &journal.lignes;
        let du_mois: Vec<Value> = depots
            .iter()
            .filter(|d| d.get("mois").and_then(Value::as_str) == Some(mois))
            .map(|d| {
                let mut x = Map::new();
                for k in ["quoi", "quand", "mode", "accepte", "nature", "annule_et_remplace", "type_envoi"] {
                    if let Some(v) = d.get(k) {
                        x.insert(k.into(), v.clone());
                    }
                }
                let verdicts: Vec<Value> = d.get("retours").and_then(Value::as_array).into_iter().flatten().map(|r| json!(format!("{} {}", r["nature"].as_str().unwrap_or("?"), r["verdict"].as_str().unwrap_or("?")))).collect();
                x.insert("retours".into(), Value::Array(verdicts));
                Value::Object(x)
            })
            .collect();
        let n = du_mois.len();
        m.insert("depots".into(), Value::Array(du_mois[n.saturating_sub(8)..].to_vec()));
    }
    Value::Object(m)
}
