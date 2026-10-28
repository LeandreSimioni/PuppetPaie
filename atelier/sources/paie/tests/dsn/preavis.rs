//! Le preavis et le delai de prevenance d'une fin de contrat, tels que la DSN les declare : le code du bloc 52 de
//! l'indemnite selon le motif (S21.G00.52.001), le type de preavis du signalement (S21.G00.63.001) confronte a ce que la
//! paie verse. Textes : `atelier/reference/norme-dsn/texte/bloc-S21.G00.52.txt` (cahier technique 2026.1, p. 250-254),
//! `texte-2027/bloc-S21.G00.63.txt` (les valeurs de 63.001 ; l'extrait 2026.1 s'arrete a la page 279) et
//! `atelier/reference/sources/code-du-travail/articles-20261013.md` (L1234-5, L1226-14, L1221-25, L1221-26).

#[test]
fn le_code_032_n_accompagne_qu_une_inaptitude() {
    // CCH-29 : avec 032, le motif de rupture est 033, 091, 092 ou 099 ; un 032 avec un licenciement ordinaire refuse.
    assert_eq!(droit::permanents::code_52_permis(Some("032"), Some("092")).unwrap(), None);
    assert_eq!(droit::permanents::code_52_permis(Some("032"), Some("033")).unwrap(), None);
    let refus = droit::permanents::code_52_permis(Some("032"), Some("020")).unwrap().expect("032 avec 020");
    assert!(refus.contains("CCH-29") && refus.contains("092"), "{}", refus);
}
