//! Le texte d'un courriel ou d'un lien : l'echappement HTML, l'encodage d'une URL, le lien `mailto:`, le corps d'un
//! courriel mis en HTML.

/// `html.escape(s)` : `&`, `<`, `>`, `"` et `'`.
pub fn html_echapper(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#x27;"),
            c => o.push(c),
        }
    }
    o
}

/// `urllib.parse.quote(s, safe=surs)` : tout est encode en UTF-8 par `%XX`, sauf les lettres et chiffres ASCII,
/// `_.-~` et `surs`.
pub fn pourcent(s: &str, surs: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) || surs.as_bytes().contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// `_mailto` : un lien `mailto:` (RFC 6068), objet et corps encodes comme une URL.
pub fn mailto(adresse: &str, objet: &str, corps: &str) -> String {
    format!("mailto:{}?subject={}&body={}", pourcent(adresse, "@"), pourcent(objet, ""), pourcent(corps, ""))
}

/// `operations_casc._html` : le courriel en HTML, le meme texte, mais le tableau des masses en vrai tableau -- en
/// texte brut, ses barres ne s'alignent que dans une police a chasse fixe, ce qu'aucune messagerie n'emploie.
pub fn courriel_en_html(corps: &str) -> String {
    let blanc = |c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c);
    let cellule = |c: &str, balise: &str| -> String {
        let c = c.trim_matches(blanc);
        let gras = c.starts_with("**") && c.ends_with("**");
        let c = html_echapper(c.trim_matches('*'));
        let mut style = "padding:3px 10px;border-bottom:1px solid #ddd".to_string();
        if c.chars().any(|ch| ch.is_ascii_digit()) && c.contains('€') {
            style.push_str(";text-align:right");
        }
        format!("<{b} style=\"{s}\">{c}</{b}>", b = balise, s = style, c = if gras { format!("<b>{}</b>", c) } else { c })
    };
    let mut blocs: Vec<String> = Vec::new();
    for para in corps.split("\n\n") {
        let lignes: Vec<&str> = para.split('\n').collect();
        if lignes.iter().filter(|l| !l.is_empty()).all(|l| l.starts_with('|')) {
            let tableau: Vec<Vec<&str>> = lignes.iter().filter(|l| !l.is_empty()).map(|l| l.trim_matches('|').split('|').collect()).collect();
            if tableau.is_empty() {
                continue;
            }
            let entete: String = tableau[0].iter().map(|c| cellule(c, "th")).collect();
            let rangees: String = tableau.iter().skip(2).map(|l| format!("<tr>{}</tr>", l.iter().map(|c| cellule(c, "td")).collect::<String>())).collect();
            blocs.push(format!("<table style=\"border-collapse:collapse\"><tr>{}</tr>{}</table>", entete, rangees));
        } else {
            blocs.push(format!("<p>{}</p>", lignes.iter().map(|l| html_echapper(l)).collect::<Vec<_>>().join("<br>")));
        }
    }
    format!("<div style=\"font-family:sans-serif;font-size:14px\">{}</div>", blocs.concat())
}
