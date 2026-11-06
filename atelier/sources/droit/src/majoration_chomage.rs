//! LA MAJORATION DE LA CONTRIBUTION CHOMAGE D'UN CDD D'USAGE COURT. Circulaire Unedic 2025-05, § 2.1.1
//! (`atelier/reference/sources/plafond-intermittents/unedic-circulaire-2025-05-contributions-20250501.txt`) : « La
//! majoration de 0,5 % des contributions dues par les employeurs au titre des CDD visés à l'article L. 1242-2 3° du code
//! du travail (CDD dits « d'usage »), excepté pour les emplois à caractère saisonnier, d'une durée inférieure ou égale à
//! 3 mois, est applicable pour l'emploi [...] des salariés intermittents du spectacle ». Le bareme la porte sur la ligne du
//! chomage et sa cotisation 041 (`majoration_chomage_cddu`) ; la pile la retire d'un contrat qui ne la doit pas
//! (`bareme::bareme_majoration_chomage`).
//!
//! « La majoration n'est pas due [...] dès lors que le salarié est embauché par l'employeur en contrat à durée
//! indéterminée à l'issue du contrat » : le moteur ne le sait qu'a l'embauche du CDI, apres la paie du contrat ; il ne le
//! porte pas.

use socle::date::Jour;

use crate::natures::Nature;

/// Le contrat doit-il la majoration ? Un CDD d'usage (nature 02, motif 05 : `cpf_cdd::codes_de`) de trois mois au plus :
/// sa fin tombe avant le meme quantieme, trois mois apres son debut (le dernier jour du mois quand il n'y est pas).
pub fn due(nat: &Nature, motif_recours: Option<&str>, debut: &Jour, fin: Option<&Jour>) -> bool {
    let (nature, _, motif) = crate::cpf_cdd::codes_de(nat, motif_recours);
    if nature != "02" || motif != "05" {
        return false;
    }
    let Some(fin) = fin else { return false };
    let (mut an, mut mois) = (debut.an, debut.mois + 3);
    if mois > 12 {
        mois -= 12;
        an += 1;
    }
    let dernier = socle::date::jours_du_mois(an, mois);
    match Jour::nouveau(an, mois, debut.jour.min(dernier)) {
        Some(limite) => *fin < limite,
        None => false,
    }
}
