//! Ticket #223 : les versements a Audiens (bloc S21.G00.20).
//!
//! Audiens, dossier DSN (`atelier/reference/sources/audiens-dsn/audiens-dossier-dsn-20261020.md`) : « Pour la retraite
//! Audiens > 44832375800038 ; Pour les Congés Spectacles > 77567608300030 ; Pour Audiens Santé Prévoyance > P0983 »,
//! et « L'échéance de paiement pour Audiens est pour un mois M, le 25 du mois M+1. Cette date est automatiquement retenue
//! pour les paiements retraite et Congés Spectacles mais doit être renseignée dans le bloc paiement à destination
//! d'Audiens Santé Prévoyance (P0983). » La norme (cahier technique 2026.1, `norme-dsn/texte/bloc-S21.G00.20.txt`,
//! rubrique 011) : « AGIRC-ARRCO : non concerné », « Congés spectacles (AUDIENS) : non concerné ».
//!
//! Sur la copie figee des compagnies fictives (`tests/donnees/fictives/`, compagnie inventee).

use crate::outils::{fictive, paires as lignes};
use droit::contexte::Contexte;
use socle::date::Jour;
use paie::dsn::{generer, Options};
use engagement::charger;

/// Le bloc 20 de l'organisme `id` : ses rubriques jusqu'au bloc suivant.
fn bloc(dsn: &str, id: &str) -> Vec<(String, String)> {
    let l = lignes(dsn);
    let i = l.iter().position(|(k, v)| k == "S21.G00.20.001" && v == id).unwrap_or_else(|| panic!("pas de versement a {}", id));
    let fin = l[i + 1..]
        .iter()
        .position(|(k, _)| k == "S21.G00.20.001" || !k.starts_with("S21.G00.20."))
        .map_or(l.len(), |p| i + 1 + p);
    l[i..fin].to_vec()
}

fn val<'a>(b: &'a [(String, String)], r: &str) -> Option<&'a str> {
    b.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str())
}

#[test]
fn audiens_sante_prevoyance_est_prelevee_le_25_du_mois_suivant() {
    let ctx = Contexte::nouveau(&fictive("fictive-3090-grande"), Some(Jour::lire("2026-07-28").unwrap())).unwrap();
    let s = charger(&ctx, "2026-07").unwrap();
    let dsn = generer(&ctx, &s.paies, "2026-07", &[], &[], &Options::default()).unwrap();
    // P0983 : la date doit etre renseignee, le 25 du mois qui suit juillet.
    assert_eq!(val(&bloc(&dsn, "P0983"), "S21.G00.20.011"), Some("25082026"));
    // Retraite et Conges Spectacles : la norme ne demande pas de date, Audiens retient le 25 d'elle-meme.
    for id in ["44832375800038", "77567608300030"] {
        assert_eq!(val(&bloc(&dsn, id), "S21.G00.20.011"), None, "{} : non concerne (rubrique 011)", id);
    }
}
