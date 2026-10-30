//! L'IMPORT : le passe d'une compagnie, tenu dans un autre logiciel. Les DSN et les AEM deposees (`dsn_aem`), les
//! bulletins (`bulletin_lu`, `bulletins`), les certificats Congés Spectacles (`certificat_lu`) et les AEM en PDF (`aem_lu`),
//! ce qu'une piece atteste une seule fois (`attestes`), leur tri dans la boite d'import, ce qui s'ecrit (`ecrite` : les engagements
//! importes), ce qui complete une fiche, la nature des contrats que les bulletins n'ont pas dite, le mois importe tel
//! que la page le montre, la regularisation CASC-SVP des mois importes, les ecarts de calcul et l'incident d'un import
//! qui n'a pas abouti (caviarde avant de partir) ; le guide de ce qu'on y met (`guide`) ; ce que les DSN reprises
//! disent de la configuration (`parametrage`).

pub mod aem_lu;
pub mod attestes;
pub mod bulletin_lu;
pub mod bulletins;
pub mod casc;
pub mod certificat_lu;
pub mod caviardage;
pub mod completer;
pub mod dsn_aem;
pub mod ecarts;
pub mod ecrite;
pub mod guide;
pub mod incident;
pub mod mois;
pub mod nature;
pub mod parametrage;
pub mod tri;
