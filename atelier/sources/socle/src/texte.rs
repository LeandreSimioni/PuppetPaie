//! Le texte : le blanc, les accents, la cle de recherche.

/// Le blanc d'Unicode (`char::is_whitespace`), plus `\x1c` a `\x1f` : ce qu'on retire autour d'une valeur lue et ce qui
/// separe les mots.
pub fn blanc(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
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
