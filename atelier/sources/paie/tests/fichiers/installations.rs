//! UNE FENETRE PAR INSTALLATION (ticket #244, `interface::instance`) : deux installations s'ouvrent cote a cote sur un
//! poste, chacune a son creneau (son verrou et le port de sa page) ; la meme, relancee, ramene sa fenetre devant ; le
//! port d'une installation reste le meme d'un lancement a l'autre (retenu dans `.fenetre.json`), et un creneau pris par
//! un autre programme ou par une copie de l'installation se saute. Sur une plage de ports a ces essais (jamais celle du
//! poste, 7330-7429), dans des installations de test.
//!
//! Le fil de fond de la fenetre (`fil_de_fond`) ne sort pas et ne touche a aucune cle du coffre.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use paie::interface::instance::{creneau_retenu, ouverte, prendre_de, Instance, Plage, Prise};

use crate::outils::dossier_de_test;

/// Une plage de six creneaux a cet essai : douze ports libres a la suite, sous ceux que le systeme tire (20000-32000).
fn plage() -> Plage {
    let mut premier = 20000 + (std::process::id() % 1000) as u16 * 12;
    loop {
        if (premier..premier + 12).all(|p| TcpListener::bind(("127.0.0.1", p)).is_ok()) {
            return Plage { premier, creneaux: 6 };
        }
        premier = 20000 + (premier - 20000 + 12 * 37) % 12000;
    }
}

/// Le verrou d'une prise tombee se rend en moins de 50 ms (`instance::repondre`).
fn rendu() {
    std::thread::sleep(Duration::from_millis(200));
}

fn installation(nom: &str) -> PathBuf {
    let d = dossier_de_test("installations", nom);
    std::fs::create_dir_all(d.join("logiciel")).unwrap();
    d
}

fn prise(i: Instance) -> Prise {
    match i {
        Instance::Tenue(p) => p,
        Instance::Devant => panic!("la fenetre devait s'ouvrir"),
    }
}

fn compteur() -> (Arc<AtomicUsize>, impl Fn() + Send + 'static) {
    let n = Arc::new(AtomicUsize::new(0));
    let m = n.clone();
    (n, move || {
        m.fetch_add(1, Ordering::SeqCst);
    })
}

#[test]
fn deux_installations_s_ouvrent_cote_a_cote_et_la_meme_revient_devant() {
    let p = plage();
    let (a, b) = (installation("cote-a-cote-a"), installation("cote-a-cote-b"));
    let (devant_a, appel_a) = compteur();
    let pa = prise(prendre_de(&a, p, appel_a));
    // Une AUTRE installation : elle s'ouvre, a cote, a un autre port ; la premiere n'est pas derangee.
    let (devant_b, appel_b) = compteur();
    let pb = prise(prendre_de(&b, p, appel_b));
    assert_ne!(pa.port, pb.port, "chacune son port");
    assert!(pa.verrou.is_some() && pb.verrou.is_some(), "chacune son verrou");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(devant_a.load(Ordering::SeqCst), 0, "l'autre installation ne ramene pas la premiere devant");
    assert!(ouverte(&a, p) && ouverte(&b, p));
    // LA MEME, relancee : pas de seconde fenetre, la sienne revient devant (et pas l'autre).
    assert!(matches!(prendre_de(&a, p, || {}), Instance::Devant));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(devant_a.load(Ordering::SeqCst), 1);
    assert_eq!(devant_b.load(Ordering::SeqCst), 0);
    // Les pages ecoutent : chacune a son port.
    for port in [pa.port, pb.port] {
        std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    }
    // Fermee, elle n'est plus ouverte ; l'autre l'est toujours.
    let port_a = pa.port;
    drop(pa);
    rendu();
    assert!(!ouverte(&a, p) && ouverte(&b, p));
    // RELANCEE, elle reprend LE MEME port (retenu) : le navigateur y retrouve ce qu'il a garde.
    let pa = prise(prendre_de(&a, p, || {}));
    assert_eq!(pa.port, port_a, "le port d'une installation est stable");
    assert_eq!(creneau_retenu(&a, p), pa.creneau);
    drop((pa, pb));
}

#[test]
fn un_creneau_pris_par_un_autre_programme_ou_par_une_copie_se_saute() {
    let p = plage();
    let a = installation("saute-a");
    // Une COPIE de l'installation, avec le creneau retenu de l'original : un autre dossier, une autre installation.
    let pa = prise(prendre_de(&a, p, || {}));
    let copie = installation("saute-copie");
    std::fs::copy(a.join(".fenetre.json"), copie.join(".fenetre.json")).unwrap();
    let pc = prise(prendre_de(&copie, p, || {}));
    assert_ne!(pc.creneau, pa.creneau, "la copie prend le creneau suivant");
    assert_eq!(creneau_retenu(&copie, p), pc.creneau, "et le retient pour elle");
    assert!(ouverte(&copie, p));
    drop((pa, pc));
    rendu();

    // UN AUTRE PROGRAMME tient le port de page du creneau retenu : le creneau se saute (son verrou est rendu), la fenetre
    // s'ouvre au suivant, et le retient.
    let b = installation("saute-b");
    let pb = prise(prendre_de(&b, p, || {}));
    let k = pb.creneau.unwrap();
    drop(pb);
    rendu();
    let intrus = TcpListener::bind(("127.0.0.1", p.page(k))).unwrap();
    let pb = prise(prendre_de(&b, p, || {}));
    assert_ne!(pb.creneau, Some(k));
    assert!(TcpListener::bind(("127.0.0.1", p.verrou(k))).is_ok(), "le verrou du creneau saute est rendu");
    // UN AUTRE PROGRAMME tient le VERROU d'un creneau et accepte sans rien dire : sauté aussi.
    let k2 = pb.creneau.unwrap();
    drop(pb);
    rendu();
    let muet = TcpListener::bind(("127.0.0.1", p.verrou(k2))).unwrap();
    std::thread::spawn(move || {
        for c in muet.incoming() {
            drop(c);
        }
    });
    let pb = prise(prendre_de(&b, p, || {}));
    assert!(pb.creneau != Some(k2) && pb.creneau != Some(k), "{:?}", pb.creneau);
    drop((pb, intrus));
}

#[test]
fn le_fil_de_fond_ne_sort_pas_et_ne_touche_a_aucune_cle() {
    let racine = dossier_de_test("installations", "fil-de-fond");
    std::fs::create_dir_all(racine.join("config")).unwrap();
    let coffre = move |r: &Path| paie::coffre::Coffre::sur(r, r.join("cle"), "2026-10-22T10:00");
    let r = racine.clone();
    let (reseau, cle) = std::thread::spawn(move || {
        paie::fil_de_fond::marquer();
        (
            paie::garde_reseau::sortie_vers("127.0.0.1").is_err() && paie::garde_reseau::sortie_vers("net-entreprises.fr").is_err(),
            coffre(&r).poser_cle_poste().is_err(),
        )
    })
    .join()
    .unwrap();
    assert!(reseau, "aucune sortie depuis le fil de fond");
    assert!(cle && !racine.join("cle").join(paie::coffre::FICHIER_CLE).exists(), "la cle du poste ne s'y sert ni ne s'y pose");
    // Hors du fil de fond, rien ne change.
    assert!(paie::garde_reseau::sortie_vers("127.0.0.1").is_ok());
    assert!(coffre(&racine).poser_cle_poste().is_ok());
}
