//! LE COMPTE BANCAIRE : la forme d'un IBAN et sa cle, la forme d'un BIC (`atelier/reference/sources/iban/`).

/// L'IBAN tel qu'il se garde : sans blancs, en majuscules -- l'etape preliminaire de la validation (Banque centrale de
/// Turquie, communique IBAN, annexe 3 : `atelier/reference/sources/iban/tcmb-communique-iban-20261029.txt`, l. 189-194).
pub fn iban_compact(iban: &str) -> String {
    iban.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_uppercase).collect()
}

/// LA FAUTE D'UN IBAN, ou rien. La forme que la DSN accepte : de 15 a 34 caracteres, lettres et chiffres
/// (S21.G00.20.004, `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`, l. 146-160, ISO 13616) ; deux lettres de pays et deux
/// chiffres de cle en tete ; la cle MOD 97-10 d'ISO 7064 : les quatre premiers caracteres passent a droite, chaque
/// lettre devient un nombre (A = 10 ... Z = 35), le reste de la division par 97 doit etre 1 (communique de la Banque
/// centrale de Turquie, art. 4 (7) et annexes 2 et 3 : `atelier/reference/sources/iban/tcmb-communique-iban-20261029.txt`,
/// l. 96, 175-213). Le calcul se fait chiffre a chiffre (l. 219-227 : par morceaux, le reste est le meme).
pub fn faute_iban(saisi: &str) -> Option<String> {
    let iban = iban_compact(saisi);
    let n = iban.chars().count();
    if !iban.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some("un IBAN ne porte que des lettres et des chiffres".into());
    }
    if !(15..=34).contains(&n) {
        return Some(format!("un IBAN a de 15 à 34 caractères, celui-ci en a {}", n));
    }
    let tete: Vec<char> = iban.chars().take(4).collect();
    if !tete[0].is_ascii_alphabetic() || !tete[1].is_ascii_alphabetic() || !tete[2].is_ascii_digit() || !tete[3].is_ascii_digit() {
        return Some("un IBAN commence par les deux lettres du pays, puis deux chiffres (FR76…)".into());
    }
    let mut reste: u32 = 0;
    for c in iban.chars().skip(4).chain(iban.chars().take(4)) {
        let v = c.to_digit(36).unwrap_or(0);
        reste = if v >= 10 { (reste * 100 + v) % 97 } else { (reste * 10 + v) % 97 };
    }
    if reste != 1 {
        return Some("la clé de cet IBAN est fausse : un caractère a sans doute été mal recopié du RIB".into());
    }
    None
}

/// LA FAUTE D'UN BIC, ou rien : 8 ou 11 caracteres, quatre lettres (la banque), deux lettres (le pays), deux lettres ou
/// chiffres (le lieu), et trois lettres ou chiffres facultatifs (l'agence) -- la forme de la DSN (S21.G00.20.003,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`, l. 140-144).
pub fn faute_bic(saisi: &str) -> Option<String> {
    let bic: Vec<char> = iban_compact(saisi).chars().collect();
    let lettres = |a: usize, b: usize| bic[a..b].iter().all(char::is_ascii_uppercase);
    let alnum = |a: usize, b: usize| bic[a..b].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let bon = (bic.len() == 8 || bic.len() == 11) && lettres(0, 6) && alnum(6, bic.len());
    (!bon).then(|| "un BIC a 8 ou 11 caractères : quatre lettres pour la banque, deux pour le pays, puis des lettres ou des chiffres".into())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// L'exemple de l'annexe 3 du communique (`tcmb-communique-iban-20261029.txt`, l. 193-213) : « TR47 0000 1001 0000
    /// 0350 9300 01 » a sa cle juste ; un chiffre change, elle est fausse. Le BIC a la forme de S21.G00.20.003.
    #[test]
    fn la_cle_d_un_iban_se_verifie_comme_l_annexe_3_du_communique() {
        assert_eq!(faute_iban("TR47 0000 1001 0000 0350 9300 01"), None);
        assert!(faute_iban("TR47 0000 1001 0000 0350 9300 02").is_some());
        assert!(faute_iban("TR47-0000").is_some());
        assert_eq!(faute_bic("AGRIFRPP"), None);
        assert_eq!(faute_bic("agrifrpp 882"), None);
        assert!(faute_bic("AGRIFRP").is_some());
    }
}
