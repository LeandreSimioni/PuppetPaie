//! Le fichier `.modele` : sa grammaire, ses marqueurs `{{...}}`, ses refus.
//!
//! Le format est
//! volontairement pauvre — une ligne-clé, puis son texte jusqu'à la clé suivante ; les lignes
//! qui commencent par `#` (après blancs) sont des commentaires — et ce module ne connaît AUCUNE
//! mise en page : il rend des textes. La mise en page d'un contrat, d'un avenant, d'un certificat
//! de travail ou d'un reçu de solde de tout compte (mêmes clés) est dans `style_contrat.rs`.

use std::collections::BTreeMap;

use crate::description::{refus, Refus};

/// Les clés d'un modèle, dans l'ordre où elles se testent.
const CLES: [&str; 7] = [
    "TITRE:",
    "SOUSTITRE:",
    "PREAMBULE:",
    "ANNULE:",
    "MODIFICATION:",
    "ARTICLE:",
    "SIGNATURES:",
];

/// Un modèle lu : des textes bruts (marqueurs et `**gras**` encore en place).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Modele {
    pub titre: String,
    pub soustitre: String,
    pub preambule: Vec<String>,
    /// (titre, paragraphes), dans l'ordre du fichier.
    pub articles: Vec<(String, Vec<String>)>,
    pub signatures: Vec<String>,
    pub annule: Vec<String>,
    pub modification: Vec<String>,
}

/// Les valeurs offertes au modèle : « employeur.siret » -> « 123… ». Triées (BTreeMap), comme
/// le message d'erreur les nomme.
pub type Valeurs = BTreeMap<String, String>;

/// Un blanc : celui d'Unicode, plus les séparateurs ASCII 0x1C-0x1F.
fn est_blanc(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn strip(s: &str) -> &str {
    s.trim_matches(est_blanc)
}

/// Les lignes, coupées à toute fin de ligne : \n, \r\n, \r, \v, \f, 0x1C-0x1E, 0x85, U+2028, U+2029.
fn lignes(texte: &str) -> Vec<&str> {
    let mut sortie = Vec::new();
    let mut debut = 0;
    let mut it = texte.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let coupe = matches!(
            c,
            '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if coupe {
            sortie.push(&texte[debut..i]);
            let mut fin = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = it.peek() {
                    it.next();
                    fin = j + 1;
                }
            }
            debut = fin;
        }
    }
    if debut < texte.len() {
        sortie.push(&texte[debut..]);
    }
    sortie
}

/// Une ligne vide sépare deux paragraphes ; le reste est recollé.
fn paragraphes(tampon: &[&str]) -> Vec<String> {
    let mut blocs = Vec::new();
    let mut courant: Vec<&str> = Vec::new();
    for l in tampon {
        if !strip(l).is_empty() {
            courant.push(strip(l));
        } else if !courant.is_empty() {
            blocs.push(courant.join(" "));
            courant.clear();
        }
    }
    if !courant.is_empty() {
        blocs.push(courant.join(" "));
    }
    blocs
}

/// Découpe le texte d'un modèle en titre, préambule, articles et signatures (`lire_modele`).
/// `nom` est ce qu'on nomme dans les refus (le chemin du fichier).
pub fn lire_modele(texte: &str, nom: &str) -> Result<Modele, Refus> {
    let mut m = Modele::default();
    let mut cle: Option<&str> = None;
    let mut titre_art = String::new();
    let mut tampon: Vec<&str> = Vec::new();

    fn vider(m: &mut Modele, cle: Option<&str>, titre_art: &str, tampon: &[&str]) {
        match cle {
            Some("PREAMBULE:") => m.preambule = paragraphes(tampon),
            Some("ANNULE:") => m.annule = paragraphes(tampon),
            Some("MODIFICATION:") => m.modification = paragraphes(tampon),
            Some("ARTICLE:") => m.articles.push((titre_art.to_string(), paragraphes(tampon))),
            Some("SIGNATURES:") => m.signatures = paragraphes(tampon),
            _ => {}
        }
    }

    for ligne in lignes(texte) {
        if ligne.trim_start_matches(est_blanc).starts_with('#') {
            continue;
        }
        let trouvee = CLES.iter().find(|k| ligne.starts_with(**k));
        if let Some(&k) = trouvee {
            vider(&mut m, cle, &titre_art, &tampon);
            cle = Some(k);
            tampon.clear();
            let reste = strip(&ligne[k.len()..]);
            match k {
                "TITRE:" => m.titre = reste.to_string(),
                "SOUSTITRE:" => m.soustitre = reste.to_string(),
                "ARTICLE:" => titre_art = reste.to_string(),
                _ => {}
            }
        } else if cle.is_some() {
            tampon.push(ligne);
        }
    }
    vider(&mut m, cle, &titre_art, &tampon);
    if m.articles.is_empty() {
        return refus(format!("{} ne contient aucun article", nom));
    }
    Ok(m)
}

/// Lit le fichier `chemin` (UTF-8) et le découpe.
pub fn lire_fichier(chemin: &str) -> Result<Modele, Refus> {
    let octets = std::fs::read(chemin)
        .map_err(|e| Refus(format!("modèle illisible : {} : {}", chemin, e)))?;
    let texte = String::from_utf8(octets)
        .map_err(|_| Refus(format!("modèle illisible : {} n'est pas de l'UTF-8", chemin)))?;
    lire_modele(&texte, chemin)
}

// ---------------------------------------------------------------------------
// Les marqueurs.
// ---------------------------------------------------------------------------

/// Reconnaît `{{\s*[a-z_]+\.[a-z_]+\s*}}` à l'octet `i` ; rend (clé, octet de fin).
pub fn marqueur_en(texte: &str, i: usize) -> Option<(&str, usize)> {
    let reste = texte.get(i..)?.strip_prefix("{{")?;
    let sans_blancs = reste.trim_start_matches(est_blanc);
    let debut = i + 2 + (reste.len() - sans_blancs.len());
    let nom = |s: &str| s.len() - s.trim_start_matches(|c: char| c.is_ascii_lowercase() || c == '_').len();
    let a = nom(&texte[debut..]);
    if a == 0 || !texte[debut + a..].starts_with('.') {
        return None;
    }
    let b = nom(&texte[debut + a + 1..]);
    if b == 0 {
        return None;
    }
    let fin_cle = debut + a + 1 + b;
    let apres = texte[fin_cle..].trim_start_matches(est_blanc);
    let fin = texte.len() - apres.len();
    if !apres.starts_with("}}") {
        return None;
    }
    Some((&texte[debut..fin_cle], fin + 2))
}

/// Substitue les marqueurs. Un marqueur inconnu est un refus qui les nomme tous :
/// on ne signe pas un contrat à moitié rempli.
pub fn substituer(texte: &str, vals: &Valeurs) -> Result<String, Refus> {
    let mut sortie = String::with_capacity(texte.len());
    let mut manquants: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < texte.len() {
        if texte[i..].starts_with("{{") {
            if let Some((cle, fin)) = marqueur_en(texte, i) {
                match vals.get(cle) {
                    Some(v) => sortie.push_str(v),
                    None => manquants.push(cle),
                }
                i = fin;
                continue;
            }
        }
        let c = texte[i..].chars().next().expect("caractère");
        sortie.push(c);
        i += c.len_utf8();
    }
    if !manquants.is_empty() {
        return refus(format!(
            "marqueur inconnu dans le modèle de contrat : {}. Marqueurs disponibles : {}",
            manquants.iter().map(|c| format!("{{{{{}}}}}", c)).collect::<Vec<_>>().join(", "),
            vals.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(sortie)
}

/// Le prochain `\*\*(.+?)\*\*` à partir de l'octet `depuis` : (début, début du contenu, fin du
/// contenu, fin). `.` ne couvre pas « \n » ; le contenu a au moins un caractère.
fn gras_apres(texte: &str, depuis: usize) -> Option<(usize, usize, usize, usize)> {
    let mut s = depuis;
    while let Some(k) = texte[s..].find("**") {
        let debut = s + k;
        let c0 = debut + 2;
        if let Some(ch) = texte[c0..].chars().next() {
            let p = c0 + ch.len_utf8();
            if ch != '\n' {
                if let Some(q) = texte[p..].find("**") {
                    let q = p + q;
                    if !texte[c0..q].contains('\n') {
                        return Some((debut, c0, q, q + 2));
                    }
                }
            }
        }
        s = debut + 1;
    }
    None
}

/// Le texte tel qu'il se lit sur le document imprimé : marqueurs substitués, sans gras
/// (`texte_brut`). C'est aussi le titre du fichier PDF.
pub fn texte_brut(texte: &str, vals: &Valeurs) -> Result<String, Refus> {
    let t = substituer(texte, vals)?;
    let mut sortie = String::new();
    let mut dernier = 0;
    let mut depuis = 0;
    while let Some((d, c0, c1, f)) = gras_apres(&t, depuis) {
        sortie.push_str(&t[dernier..d]);
        sortie.push_str(&t[c0..c1]);
        dernier = f;
        depuis = f;
    }
    sortie.push_str(&t[dernier..]);
    Ok(sortie)
}

/// Les blancs de HTML : une suite de « espace, tab, \r, \n, \f, \v » vaut un seul espace. Pas les
/// autres blancs : l'espace insécable est une espace du texte (« 1 234,50 € »).
fn reduire_blancs(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    let mut dans_blanc = false;
    for c in t.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}' | '\u{b}') {
            if !dans_blanc {
                s.push(' ');
            }
            dans_blanc = true;
        } else {
            s.push(c);
            dans_blanc = false;
        }
    }
    s
}

/// Le texte du modèle en morceaux (texte, gras) : marqueurs substitués, blancs comptés comme en
/// HTML, `**…**` en gras. `gras` rend tout le texte gras.
pub fn morceaux(texte: &str, vals: &Valeurs, gras: bool) -> Result<Vec<(String, bool)>, Refus> {
    Ok(morceaux_substitues(&substituer(texte, vals)?, gras))
}

/// Comme `morceaux`, pour un texte dont les marqueurs sont déjà substitués (la valeur substituée
/// n'est pas relue : on ne substitue pas deux fois).
pub fn morceaux_substitues(texte: &str, gras: bool) -> Vec<(String, bool)> {
    let t = reduire_blancs(texte);
    let t = strip(&t);
    let mut sortie = Vec::new();
    let mut dernier = 0;
    let mut depuis = 0;
    while let Some((d, c0, c1, f)) = gras_apres(t, depuis) {
        if d > dernier {
            sortie.push((t[dernier..d].to_string(), gras));
        }
        sortie.push((t[c0..c1].to_string(), true));
        dernier = f;
        depuis = f;
    }
    if dernier < t.len() {
        sortie.push((t[dernier..].to_string(), gras));
    }
    sortie
}

/// Le texte substitué coupé en lignes non vides : une
/// valeur peut en valoir plusieurs, comme l'inventaire du reçu de solde de tout compte, une
/// somme par ligne. Un `**` ne se referme pas d'une ligne à l'autre.
pub fn lignes_substituees(texte: &str, vals: &Valeurs) -> Result<Vec<String>, Refus> {
    Ok(substituer(texte, vals)?
        .split('\n')
        .filter(|l| !strip(l).is_empty())
        .map(str::to_string)
        .collect())
}
