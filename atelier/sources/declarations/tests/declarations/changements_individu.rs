//! Corrections DSN, vague 3d : le bloc S21.G00.31 (changements individu). Quand un identifiant de l'individu (NIR, nom
//! de famille, prenoms, date de naissance) differe de ce qui a ete DECLARE pour lui -- relu dans la derniere DSN
//! reelle acceptee qui le porte, jamais recalcule --, la DSN porte l'ANCIENNE valeur, datee par l'historique de la fiche
//! (cahier technique 2026.1, p. 177-179 ; § 2.1.1, p. 18 ; § 2.1.2.2-2.1.2.3, p. 20-21 ;
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.31.txt`, `chapitre-2-principes-de-constitution.txt`).

#[test]
fn le_bloc_31_n_est_admis_que_dans_la_dsn_mensuelle() {
    // Le tableau des usages (p. 363 et suivantes ; `usages-dsn-base.json`) : 31.001 obligatoire et 31.008-011
    // conditionnelles en nature 01, sans objet (N) dans tous les signalements. De meme dans le tableau 2027 (p. 371 et
    // suivantes ; `usages-dsn-p27v01-base.json`) : la table se lit par version de la norme depuis le lot V4a.
    for version in ["P26V01", "P27V01"] {
        let norme = declarations::norme::Norme::connue(version).unwrap();
        for r in ["S21.G00.31.001", "S21.G00.31.008", "S21.G00.31.009", "S21.G00.31.010", "S21.G00.31.011"] {
            let u = declarations::usages::usage_en(&norme, r, "01").unwrap();
            assert!(u == "O" || u == "C", "{} en mensuelle {} : {}", r, version, u);
            for n in ["04", "05", "07", "08", "10"] {
                assert_eq!(declarations::usages::usage_en(&norme, r, n).unwrap(), "N", "{} en nature {} {}", r, n, version);
            }
        }
    }
}

