//! Le questionnaire de la fiche salarie remplie par la personne elle-meme, par courriel
//! (`moteur/operations_questionnaire.py` : `QUESTIONS`, `questionnaire`, `code_fiche`, `lire_reponses`, `_date`,
//! `_pays`, `fiche_depuis_reponses`). Lot H4a.
//!
//! ECRIT, NON PROUVE. Les textes (questions, objet, corps, HTML) et les deux tables (pays, nationalites) sont
//! extraits MOT POUR MOT du Python par script. Depuis la vague 1j (10/10/2026), les deux tables ne portent plus que le code
//! ISO : la codification UE (quatre valeurs) se deduit dans `codification_ue`. Depuis la vague 1k, les pays se
//! lisent dans la liste livree (`pays.rs`, COG de l'INSEE) ; seule la table des nationalites (adjectifs) reste ici. Ce que la personne a repondu se lit, et ce qu'on ne sait pas lire
//! laisse la case VIDE et le dit (`illisible`) : rien n'est complete au hasard. Le departement et le pays de
//! naissance se lisent dans le NIR.
//!
//! Ecarts de lecture connus avec `re` / `str` de Python (notes dans la passation) : `\d` et `int()` ne lisent ici que
//! les chiffres ASCII pour les dates ; la decomposition NFD des accents est celle de `contexte::sans_accent` (latin
//! courant).

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use socle::texte::{blanc as espace, strip};
use socle::date::Jour;

pub const ALPHABET_CODE: &str = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// `code_fiche` : `FIC-` puis six signes tires au hasard dans `ALPHABET_CODE`.
pub fn code_fiche() -> String {
    crate::envois_signature::code_au_hasard("FIC-")
}

/// Les questions : (cle de la reponse, libelle, facultatif).
pub const QUESTIONS: [(&str, &str, bool); 13] = [
    ("prenom", "Prénom", false),
    ("nom", "Nom de famille (de naissance)", false),
    ("naissance", "Date de naissance (JJ/MM/AAAA)", false),
    ("lieu", "Commune de naissance", false),
    ("pays", "Pays de naissance", false),
    ("nationalite", "Nationalité", false),
    ("nir", "Numéro de sécurité sociale (15 chiffres)", false),
    ("sexe", "Sans numéro de sécurité sociale : femme ou homme ?", true),
    ("adresse", "Adresse (numéro et rue)", false),
    ("code_postal", "Code postal", false),
    ("commune", "Commune", false),
    ("conges", "Numéro Congés Spectacles", true),
    ("anniversaire", "Date anniversaire de vos droits France Travail (JJ/MM/AAAA)", true),
];

pub const SUJET: &str = "Ta fiche pour {compagnie}";

pub const CORPS: &str = "Bonjour{virgule_prenom},\n\nPour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.\n\nLe plus simple : appuie sur le bouton « Remplir ma fiche » de ce message. Une réponse s'ouvre avec les questions ; écris chaque réponse après les deux-points, puis envoie-la. Les lignes marquées « facultatif » peuvent rester vides.\n\nSi ta messagerie n'affiche pas le bouton, réponds simplement à ce message en recopiant les lignes ci-dessous, avec tes réponses :\n\n{questionnaire}\n\nCes informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.\n\nBien à toi,\n{signataire}\n{compagnie}\n";

pub const HTML: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour{virgule_prenom},</p>\n<p>Pour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.</p>\n<p>Appuie sur le bouton : une réponse s'ouvre avec les questions. Écris chaque réponse après les deux-points, puis envoie-la. Les lignes marquées « facultatif » peuvent rester vides.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{mailto}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Remplir ma fiche</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si le bouton ne fait rien, réponds simplement à ce message en recopiant ces lignes avec tes réponses :</p>\n<pre style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;background:#f4f4f4;padding:12px\">{questionnaire}</pre>\n<p>Ces informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.</p>\n<p>Bien à toi,<br>{signataire}<br>{compagnie}</p>\n</div>\n";

/// Les nationalites (adjectifs) : (mot sans accent, code ISO du pays). La fiche garde le pays (`pays_nationalite`) ;
/// la codification UE s'en deduit (`codification_ue::code_du_pays`).
const NATIONALITES: [(&str, &str); 60] = [
    ("francaise", "FR"),
    ("francais", "FR"),
    ("italienne", "IT"),
    ("italien", "IT"),
    ("espagnole", "ES"),
    ("espagnol", "ES"),
    ("belge", "BE"),
    ("allemande", "DE"),
    ("allemand", "DE"),
    ("portugaise", "PT"),
    ("portugais", "PT"),
    ("polonaise", "PL"),
    ("polonais", "PL"),
    ("roumaine", "RO"),
    ("roumain", "RO"),
    ("grecque", "GR"),
    ("grec", "GR"),
    ("neerlandaise", "NL"),
    ("neerlandais", "NL"),
    ("irlandaise", "IE"),
    ("irlandais", "IE"),
    ("luxembourgeoise", "LU"),
    ("luxembourgeois", "LU"),
    ("autrichienne", "AT"),
    ("autrichien", "AT"),
    ("suedoise", "SE"),
    ("suedois", "SE"),
    ("danoise", "DK"),
    ("danois", "DK"),
    ("finlandaise", "FI"),
    ("finlandais", "FI"),
    ("norvegienne", "NO"),
    ("norvegien", "NO"),
    ("islandaise", "IS"),
    ("islandais", "IS"),
    ("britannique", "GB"),
    ("anglaise", "GB"),
    ("anglais", "GB"),
    ("suisse", "CH"),
    ("americaine", "US"),
    ("americain", "US"),
    ("canadienne", "CA"),
    ("canadien", "CA"),
    ("algerienne", "DZ"),
    ("algerien", "DZ"),
    ("marocaine", "MA"),
    ("marocain", "MA"),
    ("tunisienne", "TN"),
    ("tunisien", "TN"),
    ("bresilienne", "BR"),
    ("bresilien", "BR"),
    ("argentine", "AR"),
    ("argentin", "AR"),
    ("chilienne", "CL"),
    ("chilien", "CL"),
    ("ukrainienne", "UA"),
    ("ukrainien", "UA"),
    ("russe", "RU"),
    ("liechtensteinoise", "LI"),
    ("liechtensteinois", "LI"),
];

/// Les nationalites d'un pays que la liste livree n'a pas (`pays-base.json`, COG de l'INSEE : le Kosovo n'y a pas de
/// code ISO, vague 2h) : (mot sans accent, nom du pays). Le nom va dans `pays_nationalite_hors_liste` ; la zone
/// (`code_nationalite_dsn`) ne s'en deduit pas, l'administrateur la choisit (Leandre, 10/10/2026, PASSATION-V2h). Reste
/// R1 #147.
const NATIONALITES_HORS_LISTE: [(&str, &str); 3] = [("kosovare", "Kosovo"), ("kosovar", "Kosovo"), ("kosovo", "Kosovo")];
/// `FORME_CODE` : un code de fiche bien forme.
pub fn est_un_code(t: &str) -> bool {
    t.len() == 10 && t.starts_with("FIC-") && t[4..].chars().all(|c| ALPHABET_CODE.contains(c))
}

/// `questionnaire` : ce que le bouton prepare. Les questions, puis le code, en DERNIER, sur sa ligne ; la releve le
/// cherche dans ce que la personne a ecrit, avant la citation.
pub fn questionnaire(code: &str) -> String {
    let mut lignes: Vec<String> = QUESTIONS.iter().map(|(_, lib, fac)| format!("{}{} : ", lib, if *fac { " (facultatif)" } else { "" })).collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", code));
    lignes.join("\n")
}

// ---------------------------------------------------------------- la lecture

/// `_sans_accents` : sans accents, en minuscules, sans blancs aux bords.
fn sans_accents(t: &str) -> String {
    strip(&socle::texte::sans_accent(t).to_lowercase()).to_string()
}

/// `_cle_libelle` : sans accents ni casse, les parentheses (des consignes) retirees, les mots de `a-z` seulement.
fn cle_libelle(texte: &str) -> String {
    let t: Vec<char> = socle::texte::sans_accent(texte).to_lowercase().chars().collect();
    // `re.sub(r"\(.*?\)", " ", t)` : de la parenthese ouvrante a la premiere fermante, sans passer a la ligne.
    let mut sans = String::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] == '(' {
            let fin = t[i + 1..].iter().position(|c| *c == ')' || *c == '\n');
            if let Some(f) = fin.filter(|f| t[i + 1 + f] == ')') {
                sans.push(' ');
                i += f + 2;
                continue;
            }
        }
        sans.push(t[i]);
        i += 1;
    }
    // `" ".join(re.findall(r"[a-z]+", t))`.
    let mut mots: Vec<String> = Vec::new();
    let mut mot = String::new();
    for c in sans.chars() {
        if c.is_ascii_lowercase() {
            mot.push(c);
        } else if !mot.is_empty() {
            mots.push(std::mem::take(&mut mot));
        }
    }
    if !mot.is_empty() {
        mots.push(mot);
    }
    mots.join(" ")
}

/// `lire_reponses` : `{cle: reponse brute}` pour chaque ligne « Libelle : reponse » reconnue.
///
/// Une ligne se reconnait si ce qui precede le premier deux-points COMMENCE comme un libelle -- le plus long qui
/// convienne gagne (« Commune de naissance » avant « Commune »). Une reponse vide n'est pas une reponse. La premiere
/// ligne reconnue d'une cle l'emporte.
pub fn lire_reponses(texte: &str) -> HashMap<&'static str, String> {
    let libelles: Vec<(String, &'static str)> = QUESTIONS.iter().map(|(cle, lib, _)| (cle_libelle(lib), *cle)).collect();
    let mut sortie: HashMap<&'static str, String> = HashMap::new();
    // Un libelle qui porte lui-meme un deux-points (« Sans numero de securite sociale : femme ou homme ? ») : la reponse
    // est derriere le dernier. UN ECART VOLONTAIRE avec le Python, qui coupait au premier deux-points et prenait
    // « femme ou homme ? (facultatif) : homme » pour la reponse (« femme », sans NIR) -- voir la passation.
    let avec_deux_points: Vec<(usize, String, &'static str)> =
        QUESTIONS.iter().filter(|(_, lib, _)| lib.contains(':')).map(|(cle, lib, _)| (lib.matches(':').count(), cle_libelle(lib), *cle)).collect();
    for ligne in socle::texte::lignes_py(texte) {
        let coupes: Vec<usize> = ligne.match_indices(':').map(|(i, _)| i).collect();
        if let Some((n, _, cle)) = avec_deux_points.iter().find(|(n, lib, _)| {
            coupes.get(*n).is_some_and(|i| cle_libelle(ligne[..*i].trim_start_matches(['*', '>', ' '])) == *lib)
        }) {
            let valeur = strip(strip(&ligne[coupes[*n] + 1..]).trim_matches('*'));
            if !valeur.is_empty() {
                sortie.entry(*cle).or_insert_with(|| valeur.to_string());
            }
            continue;
        }
        let Some((tete, valeur)) = ligne.split_once(':') else { continue };
        // Une messagerie peut mettre la ligne en citation ou en gras.
        let tete = cle_libelle(tete.trim_start_matches(['*', '>', ' ']));
        let valeur = strip(strip(valeur).trim_matches('*'));
        if tete.is_empty() || valeur.is_empty() {
            continue;
        }
        // Le libelle exact d'abord : « Commune » n'est pas le debut tronque de « Commune de naissance ».
        let exact = libelles.iter().find(|(lib, _)| *lib == tete).map(|(_, cle)| *cle);
        let candidat = libelles
            .iter()
            .filter(|(lib, _)| !lib.is_empty() && (tete.starts_with(&format!("{} ", lib)) || (lib.starts_with(&tete) && tete.chars().count() >= 6)))
            .map(|(lib, cle)| (lib.chars().count(), *cle))
            .max();
        if let Some(cle) = exact.or(candidat.map(|(_, c)| c)) {
            sortie.entry(cle).or_insert_with(|| valeur.to_string());
        }
    }
    sortie
}

/// Le premier chiffre (le zero) de chaque ecriture decimale d'Unicode (categorie `Nd`) : `\d` de `re` et `int()` de
/// Python les lisent toutes. Chaque ecriture compte dix chiffres consecutifs.
const ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
    0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
    0x11D50, 0x11DA0, 0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    0x1FBF0,
];

/// La valeur d'un chiffre decimal d'Unicode (`unicodedata.decimal`), ou `None`.
fn valeur_chiffre(c: char) -> Option<u32> {
    let n = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&n)).map(|z| n - z)
}

/// Un chiffre pour `\d` : la categorie `Nd` d'Unicode.
fn chiffre(c: char) -> bool {
    valeur_chiffre(c).is_some()
}

/// `int(t)` d'un groupe de chiffres decimaux (toutes ecritures), ou `None`.
fn nombre(t: &str) -> Option<u32> {
    if t.is_empty() {
        return None;
    }
    let mut v: u32 = 0;
    for c in t.chars() {
        v = v.checked_mul(10)?.checked_add(valeur_chiffre(c)?)?;
    }
    Some(v)
}

/// `_date` : une date lue `AAAA-M-J` ou `J/M/AAAA` (separateurs `/`, `.`, `-`, espace), en ISO ; `None` si elle ne se
/// lit pas ou n'existe pas.
pub fn date_lue(texte: &str) -> Option<String> {
    let t = strip(texte);
    let c: Vec<char> = t.chars().collect();
    // Les trois groupes de chiffres, separes par `sep`, de largeurs permises.
    let groupes = |sep: &dyn Fn(char) -> bool, largeurs: [(usize, usize); 3]| -> Option<[String; 3]> {
        let mut morceaux: Vec<String> = vec![String::new()];
        for x in &c {
            if sep(*x) {
                morceaux.push(String::new());
            } else if chiffre(*x) {
                morceaux.last_mut()?.push(*x);
            } else {
                return None;
            }
        }
        if morceaux.len() != 3 || morceaux.iter().zip(largeurs).any(|(m, (a, b))| !(a..=b).contains(&m.chars().count())) {
            return None;
        }
        let mut it = morceaux.into_iter();
        Some([it.next()?, it.next()?, it.next()?])
    };
    let (a, mo, j) = if let Some([a, mo, j]) = groupes(&|x| x == '-', [(4, 4), (1, 2), (1, 2)]) {
        (a, mo, j)
    } else {
        let [j, mo, a] = groupes(&|x| matches!(x, '/' | '.' | '-' | ' '), [(1, 2), (1, 2), (4, 4)])?;
        (a, mo, j)
    };
    Jour::nouveau(nombre(&a)? as i32, nombre(&mo)?, nombre(&j)?).map(|j| j.iso())
}

// LES PAYS QUE LA RELEVE SAIT NOMMER : ceux de la liste livree (`pays.rs`, Code officiel geographique de l'INSEE,
// vague 1k), par leur nom (celui du COG ou le nom officiel, sans accents) ou leur code ISO a deux lettres. Un nom ou un
// code que la liste ne connait pas laisse la case vide et la note le dit.

/// `_pays` : (code ISO, codification UE) d'un pays de la liste nomme en francais, ou de son code a deux lettres ;
/// `None` pour ce que la liste ne connait pas.
pub fn pays_lu(texte: &str) -> Option<(String, Option<&'static str>)> {
    let t = sans_accents(texte).replace('\u{2019}', "'");
    let lu = |p: socle::pays::Pays| (p.code.clone(), Some(socle::pays::codification(&p)));
    if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
        return socle::pays::du_code(&t).ok().flatten().map(lu);
    }
    let cherche = |n: &str| socle::pays::du_nom(n).ok().flatten().map(lu);
    cherche(&t).or_else(|| cherche(&t.replace(' ', "-")))
}

/// `fiche_depuis_reponses` : (donnees de fiche, ce qui n'a pas pu se lire). Rien n'est suppose. Les cles se posent
/// dans l'ordre du Python : c'est l'ordre du fichier de la fiche.
pub fn fiche_depuis_reponses(rep: &HashMap<&'static str, String>) -> (Map<String, Value>, Vec<String>) {
    let mut d: Map<String, Value> = Map::new();
    let mut illisible: Vec<String> = Vec::new();
    let reponse = |cle: &str| rep.get(cle).map(String::as_str).unwrap_or("");
    let poser = |d: &mut Map<String, Value>, cle: &str, valeur: &str| {
        if !valeur.is_empty() {
            d.insert(cle.to_string(), json!(valeur));
        }
    };
    poser(&mut d, "prenom_affichage", strip(reponse("prenom")));
    poser(&mut d, "nom_affichage", strip(reponse("nom")));
    poser(&mut d, "lieu_naissance_affichage", strip(reponse("lieu")));
    poser(&mut d, "adresse_affichage", strip(reponse("adresse")));
    poser(&mut d, "commune_affichage", strip(reponse("commune")));
    poser(&mut d, "num_conges_spectacles", &reponse("conges").replace(' ', ""));
    for (q, cle) in [("naissance", "date_naissance"), ("anniversaire", "anniversaire_droits")] {
        if !reponse(q).is_empty() {
            let v = date_lue(reponse(q));
            if let Some(v) = &v {
                poser(&mut d, cle, v);
            }
            if v.is_none() {
                illisible.push(format!("{} : « {} »", cle.replace('_', " "), reponse(q)));
            }
        }
    }
    if !reponse("code_postal").is_empty() {
        let cp: String = reponse("code_postal").chars().filter(|c| !espace(*c)).collect();
        if cp.chars().count() == 5 && cp.chars().all(chiffre) {
            d.insert("code_postal".into(), json!(cp));
        } else {
            illisible.push(format!("code postal : « {} »", reponse("code_postal")));
        }
    }
    if !reponse("nir").is_empty() {
        let n: Vec<char> = reponse("nir").chars().filter(|c| !espace(*c) && !matches!(c, '.' | '-')).collect();
        let tout_chiffres = n.iter().all(|c| chiffre(*c));
        if tout_chiffres && n.len() == 15 {
            d.insert("nir".into(), json!(n[..13].iter().collect::<String>()));
            d.insert("nir_cle".into(), json!(n[13..].iter().collect::<String>()));
        } else if tout_chiffres && n.len() == 13 {
            d.insert("nir".into(), json!(n.iter().collect::<String>()));
        } else {
            illisible.push(format!("numéro de sécurité sociale : « {} »", reponse("nir")));
        }
    }
    if !reponse("sexe").is_empty() && !d.get("nir").is_some_and(socle::fichiers::vrai) {
        let s = sans_accents(reponse("sexe"));
        match s.chars().next() {
            Some('f') => {
                d.insert("sexe".into(), json!("2"));
            }
            Some('h') | Some('m') => {
                d.insert("sexe".into(), json!("1"));
            }
            _ => illisible.push(format!("sexe : « {} »", reponse("sexe"))),
        }
    }
    if !reponse("pays").is_empty() {
        // Le pays de naissance se lit aussi dans la table « Pays » de la DSN (lot R2, `pays::naissance_du_nom`) : le
        // Kosovo, l'URSS, que le COG n'a pas.
        let lu = pays_lu(reponse("pays"))
            .map(|(code, _)| code)
            .or_else(|| {
                let t = sans_accents(reponse("pays")).replace('\u{2019}', "'");
                if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
                    socle::pays::de_la_table_dsn(&t).ok().flatten().map(|p| p.code)
                } else {
                    socle::pays::naissance_du_nom(&t).ok().flatten()
                }
            });
        match lu {
            Some(code) => {
                d.insert("pays_naissance".into(), json!(code));
            }
            None => illisible.push(format!("pays de naissance : « {} »", reponse("pays"))),
        }
    }
    // LA NATIONALITE : le pays se garde (`pays_nationalite`), la codification UE s'en deduit (vague 1j).
    if !reponse("nationalite").is_empty() {
        let t = sans_accents(reponse("nationalite"));
        let pays = NATIONALITES
            .iter()
            .find(|(n, _)| *n == t)
            .map(|(_, c)| c.to_string())
            .or_else(|| pays_lu(&t).and_then(|(c, r)| r.map(|_| c)));
        match pays {
            Some(pays) => {
                // La codification se lit dans la liste (un territoire suit son pays de rattachement), `pays.rs`.
                match socle::pays::du_code(&pays).ok().flatten() {
                    Some(p) => {
                        d.insert("code_nationalite_dsn".into(), json!(socle::pays::codification(&p)));
                        d.insert("pays_nationalite".into(), json!(p.code));
                    }
                    None => illisible.push(format!("nationalité : « {} »", reponse("nationalite"))),
                }
            }
            None => match NATIONALITES_HORS_LISTE.iter().find(|(n, _)| *n == t) {
                // Un pays hors de la liste se garde dans sa case ; la zone reste a choisir (la fiche le dit).
                Some((_, nom)) => {
                    d.insert("pays_nationalite_hors_liste".into(), json!(nom));
                }
                None => illisible.push(format!("nationalité : « {} »", reponse("nationalite"))),
            },
        }
    }
    for (cle, valeur) in naissance_depuis_nir(d.get("nir")) {
        if !d.contains_key(cle) {
            d.insert(cle.to_string(), json!(valeur));
        }
    }
    (d, illisible)
}

/// `operations_fiches.naissance_depuis_nir` : le departement et le pays de naissance que le NIR ECRIT (les 6e et 7e
/// signes ; 99 pour une naissance a l'etranger). Deux cas se taisent : la Corse d'avant 1976 (« 20 ») et un NIR
/// incomplet.
fn naissance_depuis_nir(nir: Option<&Value>) -> Vec<(&'static str, String)> {
    let nir = nir.filter(|v| socle::fichiers::vrai(v)).map(socle::valeur::valeur_en_texte).unwrap_or_default();
    let c: Vec<char> = strip(&nir).chars().collect();
    if c.len() != 13 || !c.iter().all(|x| chiffre(*x)) || !matches!(c[0], '1' | '2') {
        return Vec::new();
    }
    let dep: String = c[5..7].iter().collect();
    if dep == "20" {
        return Vec::new();
    }
    let mut v = vec![("departement_naissance", dep.clone())];
    if dep != "99" {
        v.push(("pays_naissance", "FR".to_string()));
    }
    v
}
