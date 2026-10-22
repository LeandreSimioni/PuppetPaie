//! LES COMPAGNIES DE L'INSTALLATION TENUES A JOUR EN FOND DES LE LANCEMENT (ticket #244 : « ouvrir n'importe quelle
//! compagnie instantanement, meme a la premiere ouverture du jour »). Le contrat : au lancement, sans rien ouvrir, le fil
//! de la fenetre (`etat_garde::Fil`, une file par compagnie) refait les etats perimes de chaque vraie compagnie de
//! l'installation (l'ouverture, les mois, et pour une compagnie de l'administrateur son tableau) ; ouvrir ensuite
//! chacune sert un etat frais (`perime: false`) sans attendre. Ce fil ne sort pas et ne touche a aucun coffre : aucune
//! lecture n'en revient refusee. Une installation de test, deux compagnies de test rattachees a l'administrateur
//! (`administrateur::rattacher`, comme `administrateur.rs`), calculs au 03/10/2026.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use paie::administrateur::{self, Poste};
use paie::etat_garde::{empreinte_de, servir, Fil, FileDesEtats, OUVERTURE};
use serde_json::{json, Value};

use crate::outils::{compagnie, dossier_de_test, installation};

const JOUR: &str = "2026-10-03";
const MOIS: [&str; 2] = ["2026-08", "2026-09"];
const ADMIN: &str = "administrateur-essai-1";

/// Une compagnie reelle de test de l'installation (`compagnies/<nom>`) : la config et deux mois de la petite fictive.
fn compagnie_de(inst: &Path, nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &MOIS, &inst.join("compagnies").join(nom))
}

/// Le jour des empreintes (celui d'un `Contexte`).
fn jour() -> String {
    paie::contexte::aujourdhui_sans_note().unwrap().iso()
}

/// La date de modification d'un fichier, decalee de `secondes` (ce que fait toute ecriture).
fn dater(p: &Path, secondes: u64) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    if !p.exists() {
        std::fs::write(p, b"").unwrap();
    }
    let f = std::fs::OpenOptions::new().append(true).open(p).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(secondes)).unwrap();
}

/// Tout est-il juste pour `racine` : son ouverture, ses mois suivis (ceux des saisies et le mois du jour), son tableau ?
fn tout_juste(racine: &Path) -> bool {
    let j = jour();
    let mois_du_jour = j[..7].to_string();
    let juste = |cle: &str| servir(racine, cle, &j).is_some_and(|(_, ok)| ok);
    let tableau = std::fs::read_to_string(racine.join(administrateur::TABLEAU))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|t| t["empreinte"] == json!(empreinte_de(racine, OUVERTURE, &j)));
    juste(OUVERTURE) && MOIS.iter().all(|m| juste(m)) && juste(&mois_du_jour) && tableau
}

/// Attend que le fil ait tout refait, pour toutes (cinq minutes au plus).
fn attendre_tout_juste(racines: &[PathBuf]) {
    let limite = Instant::now() + Duration::from_secs(300);
    while !racines.iter().all(|r| tout_juste(r)) {
        assert!(Instant::now() < limite, "le fil n'a pas tout refait en fond");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Le lancement de la fenetre, tel que `Fenetre::prechauffer` le fait : un fil, une file par compagnie a tenir a jour
/// (`compagnies::a_tenir_a_jour`), chacune a verifier.
fn lancer(inst: &Path) -> Vec<Arc<FileDesEtats>> {
    let fil = Fil::nouveau();
    paie::interface::compagnies::a_tenir_a_jour(inst)
        .iter()
        .map(|r| {
            let f = fil.file(r, Some(JOUR));
            f.tout_verifier();
            f
        })
        .collect()
}

#[test]
fn au_lancement_les_compagnies_se_refont_en_fond_et_s_ouvrent_fraiches() {
    let inst = installation(&dossier_de_test("prechauffage", "lancement"), "2026.10.22");
    // Debloquee par le geste (ticket #246) : bridee, elle ne tiendrait aucune compagnie reelle a jour.
    crate::outils::debloquer(&inst);
    let (a, b) = (compagnie_de(&inst, "a"), compagnie_de(&inst, "b"));
    // Une fictive de l'installation : jamais prechauffee.
    let f = compagnie::saisies("fictive-3090-petite", &MOIS, &inst.join("fictives").join("fictive-3090-petite"));
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), f.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let poste = Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: "2026-10-02T10:00".into() };
    poste.coffre(&a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&inst, &b, ADMIN, None, &poste).unwrap();

    // L'ORDRE : la derniere ouverte d'abord (b), la fictive jamais.
    dater(&a.join("journal").join("interface.log"), 0);
    dater(&b.join("journal").join("interface.log"), 60);
    assert_eq!(paie::interface::compagnies::a_tenir_a_jour(&inst), vec![b.clone(), a.clone()]);

    // Un premier lancement : rien n'est garde, tout se fait en fond.
    let racines = vec![a.clone(), b.clone()];
    let files = lancer(&inst);
    attendre_tout_juste(&racines);
    files.iter().for_each(|f| f.arreter());

    // LE LENDEMAIN (une source a bouge dans chacune) : tout est perime.
    for r in &racines {
        dater(&r.join("config").join("entreprise.json"), 5);
        assert!(!tout_juste(r), "{} : une source changee perime ses etats", r.display());
    }

    // UN LANCEMENT, SANS RIEN OUVRIR : les etats se refont en fond.
    let files = lancer(&inst);
    attendre_tout_juste(&racines);
    assert!(servir(&f, OUVERTURE, &jour()).is_none(), "la fictive n'est pas prechauffee");

    // OUVRIR CHAQUE COMPAGNIE sert un etat frais, sans attendre (aucun delai accorde).
    for file in &files {
        let o = file.ouverture(Duration::ZERO, false).unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(o["pret"], json!(true), "{}", o);
        assert_eq!(o["perime"], json!(false));
        assert_eq!(o["ecran"]["perime"], json!(false));
    }

    // RIEN N'EST REFUSE PAR LE FIL : aucune lecture n'a voulu sortir ni ouvrir un coffre.
    for r in &racines {
        for cle in [OUVERTURE, MOIS[0], MOIS[1]] {
            let etat = std::fs::read_to_string(paie::etat_garde::chemin(r, cle)).unwrap();
            assert!(!etat.contains("calcul en fond de la fenetre"), "{} {} : une lecture refusee par le fil", r.display(), cle);
        }
    }
    files.iter().for_each(|f| f.arreter());
}
