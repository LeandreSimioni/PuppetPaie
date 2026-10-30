//! LE GUIDE DE L'IMPORT : ce qu'une compagnie qui vient d'un logiciel precedent met dans sa boite aux lettres `import/`,
//! ce que le moteur sait lire de chaque piece, et ce qui restera a dire. La page le montre tel quel
//! (`importation-etat`, `guide`) ; chaque phrase dit ce que le code de l'import lit, et rien d'autre :
//! - les bulletins : `bulletin_lu` (`Entreprise`, `Salarie`, `BulletinLu`), et la fiche que `bulletins` en tire
//!   (`fiche_entreprise`, `statut_et_metier`) ;
//! - les DSN : `dsn_aem::lire_une_dsn` (`ENTREPRISE`, `IDENTITE`, `EMPLOI`, `VERSEMENT`, le bloc 20, la licence 40.049,
//!   le numero d'adherent Conges Spectacles 40.022, le contact S20.G00.07) et `parametrage` (le code risque 40.040, les
//!   adhesions S21.G00.15) ;
//! - les AEM : `dsn_aem::lire_aem` (`ContratAtteste`) ;
//! - les archives : `tri` (un zip s'ouvre, son contenu se trie comme le reste ; un PDF sans texte ne se lit pas).
//!
//! Ce qui restera a dire : ce qu'aucune de ces lectures ne donne -- la nature d'un contrat de permanent
//! (`nature`), et ce que « Pour demarrer » demande encore (les conges d'un permanent repris compris)
//! (`declarations::demarrage`), que la page montre apres l'import.

use std::path::Path;

use serde_json::{json, Value};

/// Le guide, tel que la page le montre.
pub fn guide() -> Value {
    json!({
        "pieces": [
            {
                "quoi": "Les bulletins de paie, en PDF",
                "ou": "Ceux que votre ancien logiciel imprime ou exporte : un bulletin par fichier, ou plusieurs dans un même fichier. Pas une photo ni un scan : le logiciel lit le texte du PDF.",
                "lu": [
                    "la compagnie : son nom, son adresse, son Siret, son code APE, sa convention collective ; son compte Urssaf et sa licence quand le bulletin les imprime",
                    "chaque salarié : nom, prénom, adresse, numéro de sécurité sociale, emploi, artiste ou non, cadre ou non, numéro Congés Spectacles ; sa date de naissance quand le bulletin l'imprime",
                    "chaque spectacle : son numéro d'objet et son titre",
                    "chaque paie : les jours travaillés, les cachets ou les heures, le brut, les cotisations, le net et l'impôt prélevé ; le bulletin lui-même se garde, et se rouvre depuis l'Historique",
                ],
            },
            {
                "quoi": "Les DSN déposées",
                "ou": "Les fichiers que votre ancien logiciel a déposés chaque mois sur net-entreprises, s'il vous les a laissés. Mettez-les avec les bulletins : ils portent ce que les bulletins n'impriment pas toujours (le compte bancaire, le numéro Congés Spectacles, la licence, le contact des organismes, le code risque des accidents du travail, les contrats de prévoyance).",
                "lu": [
                    "la compagnie : son Siret, son adresse, son code APE, sa convention, sa licence, son numéro d'adhérent Congés Spectacles, le compte qui paie les cotisations, la personne que les organismes contactent",
                    "ses accidents du travail : le code risque et le taux de chaque année déclarée",
                    "ses organismes et ses contrats de prévoyance et de santé",
                    "chaque salarié : son identité entière, naissance comprise",
                    "chaque contrat : son début, sa fin, son numéro, sa durée de travail, son emploi",
                    "chaque mois : ce qui a été versé et déclaré",
                ],
            },
            {
                "quoi": "Les attestations employeur (AEM)",
                "ou": "Les fichiers d'attestations envoyés à France Travail pour les intermittents.",
                "lu": ["chaque contrat d'intermittent : ses dates, ses cachets ou ses heures, son salaire, son numéro d'objet"],
            },
            {
                "quoi": "Une archive zip",
                "ou": "Telle que vous l'avez reçue : elle s'ouvre et son contenu se lit comme le reste.",
                "lu": [],
            },
        ],
        "reste": "Ce qu'aucune pièce ne porte se demande ensuite, une fois, avec l'endroit où le trouver : par exemple qui signe les contrats, la date de la première embauche de la compagnie, ou, pour un salarié payé au mois, si son contrat est un CDI ou un CDD, ce qu'un bulletin n'imprime pas.",
        "rien_n_est_perdu": "Un fichier que le logiciel ne sait pas lire est dit, avec sa raison ; il ne change rien au reste. Les noms des fichiers ne comptent pas : le logiciel lit leur contenu.",
    })
}

/// LA COMPAGNIE COMMENCE PAR L'IMPORT : elle est venue par « d'un logiciel precedent » (`compagnies::Venue`) et rien
/// n'en est encore repris (aucun contrat ni aucune paie importes, `engagement::importe`). La page s'ouvre alors sur le
/// guide (`routes::ouverture::etat`).
pub fn commencer_par_l_import(racine: &Path) -> bool {
    crate::interface::compagnies::vient_d_un_logiciel_precedent(racine)
        && engagement::importe::contrats_lus(racine).map_or(true, |c| c.is_empty())
        && engagement::importe::paies_lues(racine).map_or(true, |p| p.is_empty())
}
