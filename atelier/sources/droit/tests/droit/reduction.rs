//! La reduction generale d'un technicien declare a la caisse des Conges Spectacles (`droit::reduction`). Textes :
//! CSS D. 241-7, II (`atelier/reference/sources/code-de-la-securite-sociale/xml/D241-7.xml` : Tmin 0,0200, Tdelta
//! 0,3781, P 1,75, le SMIC « applicable au 1er janvier 2026 », le resultat « arrondi à quatre décimales ») ; D. 241-10,
//! I et IV (`atelier/reference/sources/legi-entrepot/articles.jsonl.gz`, LEGIARTI000054252223) : « Coefficient = (Tmin +
//! (Tdelta x [...]P)) x b », b = 100/90 pour les professions dont les conges sont payes par une caisse.

use crate::outils::{code_livre, dec, lire};
use droit::reduction::reduction_generale;

#[test]
fn le_coefficient_majore_ne_s_arrondit_qu_une_fois() {
    let socle = lire(&code_livre(), "baremes/2026.json");
    let technicien = &socle["par_statut"]["technicien"];
    // Une journee de 7 h payee 100 : SMIC de reference 12,02 x 7 = 84,14 ; (0,02 + 0,3781 x 0,7621^1,75) = 0,25503...
    let sans = reduction_generale(technicien, Some("technicien"), dec("100"), dec("7"), false).unwrap().unwrap();
    assert_eq!(sans.coefficient, dec("0.2550"));
    // ... x 100/90 = 0,28337... -> 0,2834 ; arrondir avant la majoration donnerait 0,2550 x 100/90 -> 0,2833.
    let avec = reduction_generale(technicien, Some("technicien"), dec("100"), dec("7"), true).unwrap().unwrap();
    assert_eq!(avec.coefficient, dec("0.2834"));
    assert_eq!(avec.montant, dec("28.34"));
}
