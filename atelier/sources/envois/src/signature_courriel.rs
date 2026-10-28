//! LA SIGNATURE DES COURRIELS (Leandre, 08/10/2026).
//!
//! Une seule signature pour tous les courriels du logiciel (documents, contrats, avenants, demandes de fiche, CASC),
//! et un nom affiche devant l'adresse de la boite. Les deux se posent dans la fiche Entreprise :
//! `courriel_nom` (« Compagnie du Fil ») et `courriel_signature` (du HTML colle depuis une messagerie).
//!
//! Elle s'applique a UN endroit, `envoi::envoyer_suivi`, que tout courriel traverse : la boite (`Courriel::racine`)
//! dit ou lire la fiche. La formule finale du modele (« Bien a toi, ») reste ; ce qui la suivait (le prenom du
//! signataire et le nom de la compagnie) est remplace par la signature, en texte et en HTML. Sans signature posee,
//! rien ne change.
//!
//! Le HTML colle est nettoye (scripts, formulaires, cadres, gestionnaires d'evenements retires) et pese 150 Ko au
//! plus, images comprises : une image collee dans la signature pese tout son poids, un lien vers une image en ligne
//! presque rien. Au-dela, la fiche le signale et l'envoi refuse plutot que de partir sans signature.

use std::path::Path;

use regex::Regex;
use serde_json::Value;

use declarations::champs::{trop_lourde, CLE_SIGNATURE};
use crate::envoi::Message;

pub const CLE_NOM: &str = "courriel_nom";

/// Les formules qui closent nos modeles : ce qui les suit est la signature.
const FORMULES: [&str; 2] = ["Bien à toi,", "Bien cordialement,"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Signature {
    /// Le nom affiche devant l'adresse (vide : l'adresse seule).
    pub nom: String,
    /// La signature en HTML, nettoyee (vide : la signature des modeles).
    pub html: String,
    /// La meme en texte, pour les messageries sans HTML.
    pub texte: String,
}

/// La signature d'une fiche Entreprise. `Err` : elle est posee mais trop lourde.
pub fn de_la_fiche(ent: &Value) -> Result<Option<Signature>, String> {
    let lire = |k: &str| ent.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
    let nom = lire(CLE_NOM);
    let brut = lire(CLE_SIGNATURE);
    if nom.is_empty() && brut.is_empty() {
        return Ok(None);
    }
    if let Some(m) = trop_lourde(&brut) {
        return Err(format!("Rien n'est parti : {}. Allegez-la dans la fiche Entreprise.", m));
    }
    let html = nettoyer(&brut);
    let texte = en_texte(&html);
    Ok(Some(Signature { nom: nom_sur(&nom), html, texte }))
}

/// La signature de la compagnie de `racine` (`config/entreprise.json`).
pub fn du_poste(racine: &Path) -> Result<Option<Signature>, String> {
    let p = racine.join("config").join("entreprise.json");
    if !p.exists() {
        return Ok(None);
    }
    let ent = socle::fichiers::lire_json(&p)?;
    de_la_fiche(&ent)
}

/// Un nom affiche ne porte ni saut de ligne ni chevron : il part dans un en-tete.
fn nom_sur(nom: &str) -> String {
    nom.chars().filter(|c| !c.is_control() && *c != '<' && *c != '>').collect::<String>().trim().to_string()
}

fn re(motif: &str) -> Regex {
    Regex::new(motif).expect("motif de la signature")
}

/// Le HTML colle, sans ce qui n'a rien a faire dans une signature : scripts, styles de page, cadres, objets,
/// formulaires, en-tete de document, commentaires, gestionnaires d'evenements et liens `javascript:`.
pub fn nettoyer(html: &str) -> String {
    let mut h = html.to_string();
    h = re(r"(?s)<!--.*?-->").replace_all(&h, "").into_owned();
    for balise in ["script", "style", "iframe", "object", "embed", "form", "noscript", "template", "head", "title", "svg", "math"] {
        h = re(&format!(r"(?is)<{b}\b[^>]*>.*?</{b}\s*>", b = balise)).replace_all(&h, "").into_owned();
        h = re(&format!(r"(?is)</?{b}\b[^>]*>", b = balise)).replace_all(&h, "").into_owned();
    }
    for balise in ["html", "body", "meta", "link", "base", "input", "button", "textarea", "select", "option"] {
        h = re(&format!(r"(?is)</?{b}\b[^>]*>", b = balise)).replace_all(&h, "").into_owned();
    }
    h = re(r#"(?is)\s+on[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#).replace_all(&h, "").into_owned();
    h = re(r"(?i)(java|vb)script\s*:").replace_all(&h, "").into_owned();
    h.trim().to_string()
}

/// La signature en texte : les lignes du HTML, sans balises, entites decodees.
pub fn en_texte(html: &str) -> String {
    let mut t = re(r"(?i)<br\s*/?>").replace_all(html, "\n").into_owned();
    t = re(r"(?i)</(p|div|tr|li|h[1-6]|table|blockquote)\s*>").replace_all(&t, "\n").into_owned();
    t = re(r"(?i)</t[dh]\s*>").replace_all(&t, " ").into_owned();
    t = re(r"(?s)<[^>]*>").replace_all(&t, "").into_owned();
    let t = decoder_entites(&t);
    let mut lignes: Vec<String> = Vec::new();
    for l in t.split('\n') {
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() && lignes.last().map_or(true, |d: &String| d.is_empty()) {
            continue;
        }
        lignes.push(l);
    }
    while lignes.last().map_or(false, |l| l.is_empty()) {
        lignes.pop();
    }
    lignes.join("\n")
}

fn decoder_entites(t: &str) -> String {
    let nommees = [("&nbsp;", " "), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'"), ("&eacute;", "é"), ("&egrave;", "è"), ("&agrave;", "à"), ("&ccedil;", "ç")];
    let mut o = t.to_string();
    for (e, c) in nommees {
        o = o.replace(e, c);
    }
    let num = re(r"&#(x[0-9a-fA-F]+|[0-9]+);");
    o = num
        .replace_all(&o, |c: &regex::Captures| {
            let v = &c[1];
            let n = if let Some(h) = v.strip_prefix('x').or_else(|| v.strip_prefix('X')) { u32::from_str_radix(h, 16).ok() } else { v.parse().ok() };
            n.and_then(char::from_u32).map(String::from).unwrap_or_default()
        })
        .into_owned();
    // `&amp;` en dernier : `&amp;lt;` reste `&lt;`.
    o.replace("&amp;", "&")
}

/// Le texte du message jusqu'a sa formule finale comprise (et son saut de ligne) ; sans formule, tout le texte
/// suivi d'une ligne vide.
fn tete_du_texte(corps: &str) -> String {
    let fin = FORMULES.iter().filter_map(|f| corps.rfind(&format!("{}\n", f)).map(|i| i + f.len() + 1)).max();
    match fin {
        Some(i) => corps[..i].to_string(),
        None => format!("{}\n\n", corps.trim_end()),
    }
}

/// Le bloc HTML de la signature.
fn bloc(s: &Signature) -> String {
    format!("<div class=\"signature\">{}</div>", s.html)
}

/// Le HTML du message, la signature a la place de celle du modele.
fn html_signe(html: &str, s: &Signature) -> String {
    // « Bien a toi,<br>Prenom<br>Compagnie</p> » : la formule reste, la suite part.
    for f in FORMULES {
        let cle = format!("{}<br>", f);
        if let Some(i) = html.rfind(&cle) {
            let debut = i + f.len();
            if let Some(j) = html[debut..].find("</p>") {
                let apres = debut + j + "</p>".len();
                return format!("{}</p>\n{}{}", &html[..debut], bloc(s), &html[apres..]);
            }
        }
    }
    // « <p>Bien cordialement,</p><p>...</p></div> » (le HTML tire du texte) : tout ce qui suit la formule part,
    // le cadre se referme.
    for f in FORMULES {
        let cle = format!("<p>{}</p>", f);
        if let Some(i) = html.rfind(&cle) {
            let apres = i + cle.len();
            let ferme = if html.trim_end().ends_with("</div>") { "</div>" } else { "" };
            return format!("{}{}{}", &html[..apres], bloc(s), ferme);
        }
    }
    // Ni l'un ni l'autre : la signature se pose a la fin, dans le cadre s'il y en a un.
    let t = html.trim_end();
    match t.strip_suffix("</div>") {
        Some(sans) => format!("{}{}</div>", sans, bloc(s)),
        None => format!("{}{}", t, bloc(s)),
    }
}

/// Le message signe. Sans signature HTML posee (le nom affiche seul), il reste tel quel.
pub fn signer(message: &Message, s: &Signature) -> Message {
    let mut m = message.clone();
    if s.html.is_empty() {
        return m;
    }
    let tete = tete_du_texte(&m.corps);
    m.corps = format!("{}{}\n", tete, s.texte);
    m.html = if m.html.is_empty() {
        html_signe(&socle::html::courriel_en_html(tete.trim_end()), s)
    } else {
        html_signe(&m.html, s)
    };
    m
}
