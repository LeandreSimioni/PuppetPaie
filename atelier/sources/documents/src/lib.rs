//! Les documents : ce qu'un engagement validé fait imprimer, signer ou envoyer. Chacun se fabrique au moment où il part,
//! depuis les paies écrites de l'engagement validé (`engagement::etats`) ; aucun ne recalcule une paie.

pub mod aer;
pub mod avenant;
pub mod bordereau;
pub mod bulletin;
pub mod conges_spectacles;
pub mod contrat;
pub mod contrat_ecrit;
pub mod ecritures_comptables;
pub mod etats_compta;
pub mod ecrire_documents;
pub mod etats_paie;
pub mod etats_personnel;
pub mod fabrique;
pub mod fin_de_contrat;
pub mod marqueurs;
pub mod fiche_questionnaire;
pub mod fiche_recapitulative;
pub mod plan_comptable;
pub mod prelevement;
pub mod tableaux_export;
pub mod taxe_sur_les_salaires;

/// La plus grande largeur, en points, à laquelle un document imprime l'image d'identité `quoi` (`logo`,
/// `signature`, `cachet`) : ce qu'il faut de pixels, et pas plus (`fabrique_pdf::alleger`). Le logo : en tête des
/// bulletins et des contrats (`bulletin::LOGO_LARGEUR`). La signature : dans le contrat
/// (`fabrique_pdf::style_contrat::SIGNATURE_LARGEUR`), le certificat Congés Spectacles (`conges_spectacles`, son cadre)
/// et l'AER (`aer::cadre_signature` dans la case du formulaire, `aer::CASE_SIGNATURE`). Le tampon : la case cachet de
/// l'AER (`aer::CASE_CACHET`).
pub fn largeur_imprimee(quoi: &str) -> Option<f64> {
    match quoi {
        "logo" => Some(bulletin::LOGO_LARGEUR as f64),
        "signature" => Some(
            [fabrique_pdf::style_contrat::SIGNATURE_LARGEUR as f64, conges_spectacles::SIGNATURE_LARGEUR, { let r = aer::cadre_signature([0.0, 0.0, aer::CASE_SIGNATURE, 1.0]); r[2] - r[0] }]
                .into_iter()
                .fold(0.0, f64::max),
        ),
        "cachet" => Some(aer::CASE_CACHET),
        _ => None,
    }
}
