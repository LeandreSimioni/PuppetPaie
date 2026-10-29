//! SUPPRIMER UNE COMPAGNIE, ET GARDER AU DEBLOCAGE CELLES QU'ON A AJOUTEES EN DEMO
//! (`interface::suppression`). Les contrats : la question du deblocage ne liste que les fictives ajoutees d'un logiciel
//! debloque ; une compagnie gardee devient reelle (sous `compagnies/`, sans marqueur, son coffre garde, ses mois faits en
//! demo montres non declares) ; une compagnie qu'on ne garde pas est effacee ; une fictive livree ne se supprime pas ; une
//! compagnie reelle ne se supprime que sur le mot de passe du logiciel (code du travail, article L3243-4, `atelier/
//! reference/sources/code-du-travail/articles-20261021.md` : le double des bulletins se conserve cinq ans) -- refusee,
//! rien n'est efface.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use moteur::compagnie::administrateur::{self, Poste};
use moteur::interface::compagnies::{ajouter_dans, liste_de};
use moteur::interface::suppression::{self, supprimer};
use serde_json::json;

use crate::outils::{debloquer, dossier_de_test, installation};

const ADMIN: &str = "administrateur-essai-1";

fn inst(test: &str) -> PathBuf {
    installation(&dossier_de_test("compagnie_supprimee", test), "2026.10.22")
}

fn poste(inst: &Path) -> Poste {
    Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: "2026-10-22T10:00".into() }
}

fn sans_rien(_: &Path) {}

#[test]
fn une_compagnie_reelle_ne_se_supprime_que_sur_le_mot_de_passe_du_logiciel() {
    let i = inst("reelle");
    debloquer(&i);
    let p = poste(&i);
    let a = ajouter_dans(&i, "Compagnie A").unwrap().racine;
    let b = ajouter_dans(&i, "Compagnie B").unwrap().racine;
    p.coffre(&a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&i, &b, ADMIN, None, &p).unwrap();
    assert!(administrateur::est_rattachee(&i, &b));
    let q = suppression::question(&b).unwrap();
    assert_eq!((q["genre"].clone(), q["mot_de_passe"].clone()), (json!("reelle"), json!(true)), "{}", q);
    assert!(q["conservation"].as_str().unwrap().contains("L3243-4"));
    // La sauvegarde ne se propose que si un Drive est lie (son jeton est la).
    assert_eq!(q["sauvegarde"]["liee"], json!(false));
    std::fs::write(b.join("config/google-jeton.bin"), b"jeton-invente").unwrap();
    assert_eq!(suppression::question(&b).unwrap()["sauvegarde"]["liee"], json!(true));

    let appele = Cell::new(false);
    // Sans mot de passe : personne pour le taper ici, refuse ; un mauvais, refuse. Rien n'est efface.
    for m in [None, Some(""), Some("pas-le-bon-1")] {
        assert!(supprimer(&i, &b, m, &p, &|_| appele.set(true)).is_err(), "{:?}", m);
        assert!(!appele.get() && b.join("config/entreprise.json").is_file(), "{:?} : rien n'est efface", m);
        assert!(administrateur::est_rattachee(&i, &b));
    }
    // Une compagnie reelle sans mot de passe du logiciel pose : rien ne prouve qui la supprime.
    let c = ajouter_dans(&i, "Compagnie C").unwrap().racine;
    assert!(supprimer(&i, &c, Some(ADMIN), &p, &sans_rien).is_err() && c.is_dir());

    assert_eq!(supprimer(&i, &b, Some(ADMIN), &p, &|_| appele.set(true)).unwrap(), "Compagnie B");
    assert!(appele.get() && !b.exists());
    assert!(liste_de(&i).iter().all(|x| x.racine != b), "la tuile a disparu");
    assert_eq!(administrateur::rattachees(&i), vec![a.clone()], "le registre l'oublie");
    // Une compagnie neuve sous le meme dossier n'en herite rien.
    let neuve = ajouter_dans(&i, "Compagnie B").unwrap().racine;
    assert_eq!(neuve, b);
    assert!(!administrateur::est_rattachee(&i, &neuve));
}

