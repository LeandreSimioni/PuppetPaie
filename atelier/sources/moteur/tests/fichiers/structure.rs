//! LES OS, GARDES QUOI QU'ON CHANGE DESSUS : chaque route dit en une phrase breve ce qu'elle fait, et l'exemple de sa
//! fiche est accepte ; la page et la ligne de commande ont le meme catalogue ; une crate ne prend qu'a celles du
//! dessous ; l'engagement ne lit pas le journal ; un seul endroit ecrit un fichier d'engagements.
//!
//! Une route dont la fiche n'est pas encore ecrite (`Fiche::a_ecrire`, un geste vide) est nommee, pas en echec.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::routes::catalogue::{catalogue, GESTE_MAX};
use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, route};

fn espace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Les fichiers `.rs` sous `d`.
fn sources(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(sources(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v
}

#[test]
fn chaque_route_dit_brievement_ce_qu_elle_fait() {
    let tout = catalogue();
    let mut vus = BTreeSet::new();
    for f in &tout {
        assert!(vus.insert(f.nom), "{} est deux fois au catalogue", f.nom);
        if !f.remplie() {
            continue;
        }
        assert!(f.geste.chars().count() <= GESTE_MAX, "{} : le geste passe {} caracteres : {}", f.nom, GESTE_MAX, f.geste);
        assert!(!f.rend.is_empty() && !f.refus.is_empty(), "{} : sa fiche dit ce qu'elle rend et ses refus", f.nom);
        assert!(serde_json::from_str::<Value>(f.exemple).is_ok_and(|v| v.is_object()), "{} : l'exemple n'est pas un objet JSON", f.nom);
    }
    let a_ecrire: Vec<&str> = tout.iter().filter(|f| !f.remplie()).map(|f| f.nom).collect();
    println!("{} routes sans fiche encore : {}", a_ecrire.len(), a_ecrire.join(", "));
}

/// Les Treteaux du Banc au 02/11, avant son premier jour joue : une compagnie configuree, FULLER en saisie.
fn treteaux(test: &str) -> PathBuf {
    let d = dossier_de_test("structure", test);
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    rejouer_dossier(&sc, &d).1
}

/// Les exemples des fiches, joues dans l'ordre du catalogue sur une compagnie de test (Les Treteaux du Banc, au 02/11) :
/// chacun est accepte, et la phrase rendue tient dans sa place.
#[test]
fn l_exemple_de_chaque_fiche_est_accepte() {
    let c = treteaux("exemples");
    // Le retrait de l'engagement en dernier : les exemples des imprevus portent sur celui que valider-engagement pose.
    let mut fiches: Vec<_> = catalogue().into_iter().filter(|f| f.remplie()).collect();
    fiches.sort_by_key(|f| f.nom == "retirer-engagement");
    for f in &fiches {
        let r = route(&c, f.nom, serde_json::from_str(f.exemple).unwrap(), "2026-11-02");
        assert_eq!(r["ok"], json!(true), "{} refuse l'exemple de sa fiche : {}", f.nom, r);
        if f.phrase > 0 {
            let p = r["resultat"]["phrase"].as_str().unwrap_or_else(|| panic!("{} : la phrase manque : {}", f.nom, r));
            assert!(p.chars().count() <= f.phrase, "{} : la phrase passe sa place ({}) : {}", f.nom, f.phrase, p);
        }
    }
}

/// Les gestes que seule la fenetre sert (`interface::api::traiter`) : ils ne sont pas encore au catalogue. La liste ne
/// fait que diminuer : un geste qui y entre fait echouer le test, un geste porte au catalogue doit en sortir.
const FENETRE_SEULE: [&str; 11] = [
    "ouvrir-piece", "ouvrir-dossier", "ouvrir-exercices", "ouvrir-exports", "ouvrir-import", "choisir-dossier",
    "veille", "veille-etape", "veille-maintenant", "sauvegarde-maintenant", "google-autoriser",
];

/// Tout ce qui se fait a l'ecran se fait en ligne de commande, et l'inverse : chaque route que la page appelle est au
/// catalogue, et chaque route du catalogue se sert par `moteur route`.
#[test]
fn la_page_et_la_ligne_de_commande_ont_le_meme_catalogue() {
    let noms: BTreeSet<&str> = catalogue().iter().map(|f| f.nom).collect();
    let js = espace().join("../../logiciel/interface/js");
    let mut page = String::new();
    for e in std::fs::read_dir(&js).unwrap().flatten() {
        let t = std::fs::read_to_string(e.path()).unwrap();
        page.push_str(&t);
        for morceau in t.split("api(\"").skip(1) {
            let nom = morceau.split('"').next().unwrap().to_string();
            if !noms.contains(nom.as_str()) {
                assert!(FENETRE_SEULE.contains(&nom.as_str()), "{} appelle la route {}, absente du catalogue", e.file_name().to_string_lossy(), nom);
            }
        }
    }
    for n in FENETRE_SEULE {
        assert!(page.contains(&format!("\"{}\"", n)) && !noms.contains(n), "{} : au catalogue, ou plus appelee par la page ; elle sort de FENETRE_SEULE", n);
    }
    let c = treteaux("servies");
    let inconnue = |r: &Value| r["erreur"].as_str().is_some_and(|m| m.starts_with("route inconnue"));
    assert!(inconnue(&route(&c, "pas-une-route", json!({}), "2026-11-02")));
    for nom in &noms {
        let r = route(&c, nom, json!({}), "2026-11-02");
        assert!(!inconnue(&r), "{} n'est pas servie par moteur route", nom);
    }
}

/// Les crates de l'espace, de bas en haut : chacune ne prend qu'a celles d'en dessous.
#[test]
fn une_crate_ne_prend_qu_a_celles_du_dessous() {
    const ORDRE: [&str; 7] = ["socle", "droit", "engagement", "documents", "declarations", "envois", "moteur"];
    for (rang, nom) in ORDRE.iter().enumerate() {
        let t = std::fs::read_to_string(espace().join(nom).join("Cargo.toml")).unwrap();
        let deps = t.split("\n[dependencies]").nth(1).unwrap_or("").split("\n[").next().unwrap();
        for l in deps.lines() {
            let dep = l.split([' ', '=']).next().unwrap_or("");
            if let Some(r) = ORDRE.iter().position(|x| *x == dep) {
                assert!(r < rang, "{} prend a {}, qui n'est pas en dessous", nom, dep);
            }
        }
    }
}

/// L'engagement ne lit pas le journal : le moteur le lit une fois par geste (`envois::journal::lu`) et lui passe ce qui
/// est parti.
#[test]
fn l_engagement_ne_lit_pas_le_journal() {
    for p in sources(&espace().join("engagement/src")) {
        for (n, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
            let code = l.split("//").next().unwrap();
            assert!(!code.contains("\"journal"), "{}:{} lit le journal : {}", p.display(), n + 1, l.trim());
        }
    }
}

/// Un seul endroit ecrit un fichier d'engagements : `engagement::stock::poser`. Hors de `stock.rs`, le chemin d'un tel
/// fichier (`stock::chemin`) ne se nomme que pour le lire : dans la cloture (l'archive de l'exercice) et l'apercu d'un
/// acte (le fichier designe est-il celui du mois ?).
#[test]
fn un_seul_endroit_ecrit_un_fichier_d_engagements() {
    const LECTEURS: [&str; 2] = ["moteur/src/compagnie/exercice_ecrit.rs", "moteur/src/commandes/actes.rs"];
    let stock = espace().join("engagement/src/stock.rs");
    assert_eq!(std::fs::read_to_string(&stock).unwrap().matches("ecrire_json(").count(), 1, "stock.rs n'ecrit que par `poser`");
    for crate_ in ["engagement", "documents", "declarations", "envois", "moteur"] {
        for p in sources(&espace().join(crate_).join("src")).into_iter().filter(|p| *p != stock) {
            if std::fs::read_to_string(&p).unwrap().contains("stock::chemin(") {
                let rel = p.strip_prefix(espace()).unwrap().to_string_lossy().into_owned();
                assert!(LECTEURS.contains(&rel.as_str()), "{} nomme le fichier d'un engagement : il passe par `stock`", rel);
            }
        }
    }
}
