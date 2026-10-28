//! La base de la CASC-SVP (`casc::base`) est le fichier livre.

use crate::outils::{code_livre, lire};
use declarations::casc::base;

#[test]
fn la_base_de_la_casc_est_le_fichier_livre() {
    assert_eq!(base().unwrap(), lire(&code_livre(), "casc-svp-base.json"));
}
