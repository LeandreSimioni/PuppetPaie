//! Le binaire livre (`moteur`, construit SANS `--features atelier`) ne contient pas la redirection de l'atelier : ni les noms
//! des variables `PUPPETPAIE_ATELIER_*`, ni la phrase du refus.

#[cfg(not(feature = "atelier"))]
mod livre {
    const LIVRE: &str = env!("CARGO_BIN_EXE_moteur");

    fn contient(octets: &[u8], aiguille: &[u8]) -> bool {
        octets.windows(aiguille.len()).any(|f| f == aiguille)
    }

    /// Le binaire livre ne porte ni les noms des variables de redirection, ni la phrase du refus, ni le mot « atelier » de
    /// `version` : le code de l'atelier n'y est pas compile.
    #[test]
    fn le_binaire_livre_ne_contient_pas_la_redirection() {
        let octets = std::fs::read(LIVRE).unwrap();
        for absent in ["PUPPETPAIE_ATELIER", "moteur-atelier", "BINAIRE DE L'ATELIER", "adresse-non-locale-refusee"] {
            assert!(!contient(&octets, absent.as_bytes()), "le binaire livre contient « {} »", absent);
        }
    }
}
