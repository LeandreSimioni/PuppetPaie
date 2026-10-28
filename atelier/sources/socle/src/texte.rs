//! Le texte : le blanc, les accents, la cle de recherche.

/// Le blanc d'Unicode (`char::is_whitespace`), plus `\x1c` a `\x1f` : ce qu'on retire autour d'une valeur lue et ce qui
/// separe les mots.
pub fn blanc(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// Le texte sans le blanc autour (`blanc`).
pub fn strip(s: &str) -> &str {
    s.trim_matches(blanc)
}

/// Les lettres accentuees du latin (Latin-1 et Latin etendu A) que
/// `unicodedata.normalize("NFD")` decompose en lettre de base + diacritique.
/// Pas de crate de normalisation : la table suffit aux noms de personnes.
const ACCENTS: [(&str, char); 38] = [
    ("àáâãäåāăą", 'a'),
    ("ÀÁÂÃÄÅĀĂĄ", 'A'),
    ("çćĉċč", 'c'),
    ("ÇĆĈĊČ", 'C'),
    ("ď", 'd'),
    ("Ď", 'D'),
    ("èéêëēĕėęě", 'e'),
    ("ÈÉÊËĒĔĖĘĚ", 'E'),
    ("ĝğġģ", 'g'),
    ("ĜĞĠĢ", 'G'),
    ("ĥ", 'h'),
    ("Ĥ", 'H'),
    ("ìíîïĩīĭį", 'i'),
    ("ÌÍÎÏĨĪĬĮİ", 'I'),
    ("ĵ", 'j'),
    ("Ĵ", 'J'),
    ("ķ", 'k'),
    ("Ķ", 'K'),
    ("ĺļľ", 'l'),
    ("ĹĻĽ", 'L'),
    ("ñńņň", 'n'),
    ("ÑŃŅŇ", 'N'),
    ("òóôõöōŏő", 'o'),
    ("ÒÓÔÕÖŌŎŐ", 'O'),
    ("ŕŗř", 'r'),
    ("ŔŖŘ", 'R'),
    ("śŝşš", 's'),
    ("ŚŜŞŠ", 'S'),
    ("ţť", 't'),
    ("ŢŤ", 'T'),
    ("ùúûüũūŭůűų", 'u'),
    ("ÙÚÛÜŨŪŬŮŰŲ", 'U'),
    ("ŵ", 'w'),
    ("Ŵ", 'W'),
    ("ŷÿý", 'y'),
    ("ŶŸÝ", 'Y'),
    ("źżž", 'z'),
    ("ŹŻŽ", 'Z'),
];

/// La lettre de base d'un caractere accentue (NFD sans les marques), ou lui-meme.
fn sans_diacritique(c: char) -> char {
    ACCENTS.iter().find(|(lettres, _)| lettres.contains(c)).map_or(c, |(_, base)| *base)
}

/// Sans les accents (NFD, marques retirees). Les lettres
/// hors du latin courant sont gardees telles quelles.
pub fn sans_accent(s: &str) -> String {
    s.chars().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).map(sans_diacritique).collect()
}

/// La cle de recherche : minuscules, sans accent, sans ponctuation, espaces simples.
pub fn cle_recherche(s: &str) -> String {
    let bas: String = sans_accent(s).chars().flat_map(char::to_lowercase).collect();
    let net: String = bas.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    net.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Le sigle : « Scène Ensemble » -> « SCENE ENSEMBLE » (NFKD, ASCII seul, majuscules).
pub fn sigle(s: &str) -> String {
    let ascii: String = sans_accent(s).chars().filter(char::is_ascii).collect();
    ascii.to_uppercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Les lignes d'un texte comme les compte `str.splitlines()` de Python (le numéro qu'on affiche est le sien) : `\n`,
/// `\r`, `\r\n`, et les séparateurs `\x0b`, `\x0c`, `\x1c`-`\x1e`, `\u{85}`, `\u{2028}`, `\u{2029}` ; une fin de ligne
/// finale ne fait pas de ligne vide. La seule copie du moteur (DSN, retours, DILA, garde avant commit).
pub fn lignes_py(t: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut debut = 0;
    let mut it = t.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            v.push(&t[debut..i]);
            debut = i + c.len_utf8();
            if c == '\r' && it.peek().is_some_and(|&(_, d)| d == '\n') {
                it.next();
                debut += 1;
            }
        }
    }
    if debut < t.len() {
        v.push(&t[debut..]);
    }
    v
}

/// Des octets lus en Latin-1 (ISO 8859-1) : chaque octet est le caractere de meme code (la DSN, les retours).
pub fn texte_latin1(o: &[u8]) -> String {
    o.iter().map(|&b| b as char).collect()
}


/// `unicodedata.normalize("NFKD", v).encode("ascii", "ignore").decode("ascii")`.
pub fn ascii_nfkd(v: &str) -> String {
    let mut ascii = String::new();
    for c in v.chars() {
        if c.is_ascii() {
            ascii.push(c);
            continue;
        }
        let cp = c as u32;
        if let Some((a, _, t, avance)) = crate::nfkd_ascii::TABLE
            .binary_search_by(|(a, b, _, _)| {
                if cp < *a {
                    std::cmp::Ordering::Greater
                } else if cp > *b {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|i| crate::nfkd_ascii::TABLE[i])
        {
            if avance {
                let base = t.chars().next().unwrap_or(' ') as u32;
                ascii.push(char::from_u32(base + (cp - a)).unwrap_or(' '));
            } else {
                ascii.push_str(t);
            }
        }
    }
    ascii
}

#[cfg(test)]
mod tests_lignes {
    use super::*;

    #[test]
    fn les_lignes_se_comptent_comme_splitlines() {
        assert_eq!(lignes_py("a\r\nb\rc\n\nd\u{85}e\u{2028}f\n"), ["a", "b", "c", "", "d", "e", "f"]);
        assert_eq!(lignes_py("a\n"), ["a"]);
        assert!(lignes_py("").is_empty());
    }

}

/// `str.capitalize()`.
pub fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}
