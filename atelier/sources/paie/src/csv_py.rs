//! `csv.writer(..., delimiter=d)` de Python : QUOTE_MINIMAL, guillemets doubles, fin de ligne CRLF.

/// Un champ : entre guillemets s'il porte le delimiteur, un guillemet ou un saut de ligne.
pub fn champ(t: &str, delim: char) -> String {
    if t.contains(delim) || t.contains('"') || t.contains('\r') || t.contains('\n') {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t.to_string()
    }
}

/// Une ligne (`writerow`), terminee par CRLF. Une ligne d'un seul champ vide s'ecrit `""`.
pub fn ligne(champs: &[String], delim: char) -> String {
    if champs.len() == 1 && champs[0].is_empty() {
        return "\"\"\r\n".to_string();
    }
    let mut s = champs.iter().map(|c| champ(c, delim)).collect::<Vec<_>>().join(&delim.to_string());
    s.push_str("\r\n");
    s
}
