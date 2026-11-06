//! Le texte : le blanc, les accents, la cle de recherche.

/// Le blanc d'Unicode (`char::is_whitespace`), plus `\x1c` a `\x1f` : ce qu'on retire autour d'une valeur lue et ce qui
/// separe les mots.
pub fn blanc(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// Un chiffre decimal d'Unicode (`str.isdigit()` : la table `nfkd_ascii::CHIFFRES`).
pub fn est_chiffre(c: char) -> bool {
    let n = c as u32;
    crate::nfkd_ascii::CHIFFRES
        .binary_search_by(|(a, b)| {
            if n < *a {
                std::cmp::Ordering::Greater
            } else if n > *b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
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
    match ACCENTS.iter().find(|(lettres, _)| lettres.contains(c)) {
        Some((_, base)) => *base,
        None => c,
    }
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

/// Les lignes d'un texte, coupées à toute fin de ligne d'Unicode (le numéro qu'on affiche les compte ainsi) : `\n`,
/// `\r`, `\r\n`, et les séparateurs `\x0b`, `\x0c`, `\x1c`-`\x1e`, `\u{85}`, `\u{2028}`, `\u{2029}` ; une fin de ligne
/// finale ne fait pas de ligne vide. La seule copie du moteur (DSN, retours, DILA, garde avant commit).
pub fn lignes_du_texte(t: &str) -> Vec<&str> {
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
        let plage = crate::nfkd_ascii::TABLE.binary_search_by(|(a, b, _, _)| {
            if cp < *a {
                std::cmp::Ordering::Greater
            } else if cp > *b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        });
        // Hors de la table, le caractere n'a pas d'ASCII : il tombe (`encode("ascii", "ignore")`).
        let (a, t, avance) = match plage {
            Ok(i) => (crate::nfkd_ascii::TABLE[i].0, crate::nfkd_ascii::TABLE[i].2, crate::nfkd_ascii::TABLE[i].3),
            Err(_sans_ascii) => continue,
        };
        if avance {
            // Une plage qui avance (`a` a `b` vers `t`, `t+1`...) : le caractere de meme rang depuis le premier de `t`.
            let decale = t.chars().next().and_then(|base| char::from_u32(base as u32 + (cp - a)));
            ascii.push(match decale {
                Some(c) => c,
                None => ' ',
            });
        } else {
            ascii.push_str(t);
        }
    }
    ascii
}
/// La première lettre en capitale, le reste en minuscules.
pub fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// « de » devant `mot`, elide devant une voyelle : « de mars 2026 », « d'avril 2026 », « d'absence non rémunérée ».
pub fn de(mot: &str) -> String {
    let voyelle = mot.chars().next().is_some_and(|c| "aàâeéèêëiîïoôuùûyAÀÂEÉÈÊIÎOÔUÛY".contains(c));
    format!("{}{}", if voyelle { "d'" } else { "de " }, mot)
}

/// LE SALARIE TEL QUE L'ECRAN LE NOMME, partout ou le moteur parle de lui (refus, avis, listes, phrases d'un envoi) :
/// « NOM Prénom » comme la page le montre : le nom avec ses accents (`nom_affichage`, sinon `nom`) en capitales, puis le
/// prénom d'usage (`prenom_affichage`, sinon `prenom`). `None` si la fiche ne porte ni nom ni prénom.
pub fn salarie_nomme(nom: &str, nom_affichage: &str, prenom: &str, prenom_affichage: &str) -> Option<String> {
    let l_un_ou_l_autre = |usage: &str, etat_civil: &str| match strip(usage) {
        "" => strip(etat_civil).to_string(),
        u => u.to_string(),
    };
    let tout = format!("{} {}", l_un_ou_l_autre(nom_affichage, nom).to_uppercase(), l_un_ou_l_autre(prenom_affichage, prenom)).trim().to_string();
    (!tout.is_empty()).then_some(tout)
}

/// UN MONTANT TEL QU'ON LE LIT, et le seul endroit qui en écrit un (une phrase du moteur, une ligne d'écran, un
/// document imprimé) : « 1 234,50 € », au centime (`num::eur2`), la virgule décimale, les milliers et l'euro séparés par
/// une espace insécable (le montant ne se coupe pas en fin de ligne ; U+00A0, que toute police d'un PDF porte). Un
/// fichier déclaré (DSN, bordereau, format d'un organisme) écrit ses montants dans la forme que sa norme impose.
pub fn euros(x: rust_decimal::Decimal) -> String {
    let t = crate::num::eur2(x).to_string();
    let (signe, t) = match t.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", t.as_str()),
    };
    let (entier, centimes) = match t.split_once('.') {
        Some(e_c) => e_c,
        None => (t, "00"),
    };
    let mut milliers = String::new();
    for (i, c) in entier.chars().enumerate() {
        if i > 0 && (entier.len() - i) % 3 == 0 {
            milliers.push('\u{a0}');
        }
        milliers.push(c);
    }
    format!("{}{},{}\u{a0}€", signe, milliers, centimes)
}

/// UNE QUANTITÉ TELLE QU'ON LA LIT (des heures, des jours, un nombre de cachets) : la virgule décimale, sans zéro
/// inutile (« 151,67 », « 8 », « 2,5 »).
pub fn virgule(x: rust_decimal::Decimal) -> String {
    x.normalize().to_string().replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::salarie_nomme;

    #[test]
    fn le_salarie_est_nomme_comme_la_page_le_montre() {
        assert_eq!(salarie_nomme("SERAPHIN", "Séraphin", "Loie", "Loïe").as_deref(), Some("SÉRAPHIN Loïe"));
        assert_eq!(salarie_nomme("Martin", "", "Léa", " ").as_deref(), Some("MARTIN Léa"));
        assert_eq!(salarie_nomme("", "", "", ""), None);
    }
}
