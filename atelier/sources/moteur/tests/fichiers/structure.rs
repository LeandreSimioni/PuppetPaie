//! LES OS, GARDES QUOI QU'ON CHANGE DESSUS : chaque route dit en une phrase breve ce qu'elle fait, et l'exemple de sa
//! fiche est accepte ; la page et la ligne de commande ont le meme catalogue ; une crate ne prend qu'a celles du
//! dessous ; l'engagement ne lit pas le journal ; un seul endroit ecrit un fichier d'engagements.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use moteur::routes::catalogue::{catalogue, GESTE_MAX};
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf};
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
        assert!(!f.geste.is_empty() && f.geste.chars().count() <= GESTE_MAX, "{} : le geste, en {} caracteres au plus : {}", f.nom, GESTE_MAX, f.geste);
        assert!(!f.rend.is_empty() && !f.refus.is_empty(), "{} : sa fiche dit ce qu'elle rend et ses refus", f.nom);
        assert!(serde_json::from_str::<Value>(f.exemple).is_ok_and(|v| v.is_object()), "{} : l'exemple n'est pas un objet JSON", f.nom);
    }
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
    // Le serveur du filet et du développeur : un faux, sur cette machine ; il donne une identité de poste et accepte tout.
    let (filet, _) = crate::outils::serveur(|_| {
        Some(crate::outils::http_json(200, &json!({"poste": "0".repeat(32), "cle": "0".repeat(64), "confirme": false, "numero": 1})))
    });
    std::fs::write(c.join("config/filet-dpae.json"), json!({"serveur": filet}).to_string()).unwrap();
    // Les bulletins d'un ancien logiciel, dans la boîte de l'import : un artiste (janvier 2026, sans la CASC-SVP) et un
    // permanent (mai 2025) dont aucune pièce ne dit la nature du contrat.
    let (mut artiste, mut permanent) = (a_repetitions(), a_heures());
    for a in [&mut artiste, &mut permanent] {
        a.id.siret = "32520440200011";
        a.id.entreprise[0] = "Les Tréteaux du Banc";
    }
    permanent.id.nom = "TARTEMPION";
    permanent.id.nir = "185079999912388";
    permanent.id.conges_spectacles = None;
    permanent.cotisations.retain(|c| c.2 != "conges_spectacles");
    std::fs::create_dir_all(c.join("import")).unwrap();
    pdf(vec![page_a(&artiste)], &c.join("import/artiste.pdf"));
    pdf(vec![page_a(&permanent)], &c.join("import/permanent.pdf"));
    // Un permanent en CDI, arrete deux fois (un accident du travail, un deuil d'enfant) : ce que seul un contrat paye au mois
    // appelle (le maintien de salaire), et ce que ces motifs font declarer hors DSN.
    let svoboda = json!({"nom": "salarie", "cle": null, "donnees": {"prenom_affichage": "Josef",
        "nom_affichage": "Svoboda", "nir": "1750544999001", "nir_cle": "19", "date_naissance": "1975-05-10", "lieu_naissance_affichage": "Villeneuve-sur-Loire",
        "departement_naissance": "44", "pays_naissance": "FR", "code_nationalite_dsn": "01", "adresse_affichage": "4 rue Kervégan", "code_postal": "44000",
        "commune_affichage": "Nantes", "email": "josef.svoboda@exemple.invalid", "emploi_affichage": "Directeur technique", "code_emploi_ft": "DIR210",
        "pcs_ese": "353C", "statut": "technicien", "statut_conventionnel": "04", "statut_categoriel_rc": "01", "date_anciennete": "2026-11-02", "sante_svp": {"niveau": "1"}, "actif": true}});
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let fiche_entreprise = gestes.lines().filter(|l| l.contains("\"route\":\"enregistrer-fiche\"") && l.contains("\"nom\":\"entreprise\"")).last().unwrap();
    let mut entreprise = serde_json::from_str::<Value>(fiche_entreprise).unwrap()["charge"].clone();
    entreprise["donnees"]["emploie_des_permanents"] = json!(true);
    entreprise["depuis"] = json!("2026-11-01");
    entreprise["confirme"] = json!(true);
    for (nom, charge) in [
        ("enregistrer-fiche", entreprise),
        ("organismes-enregistrer", json!({"contrats": {"prevoyance_permanents": {"libelle": "PREVOYANCE", "organisme": "P0983"}, "sante_permanents": {"libelle": "SANTE", "organisme": "P0983"}}})),
        ("enregistrer-fiche", svoboda),
        ("valider-engagement", json!({"mois": "2026-11", "saisie": {"salarie": "SVOBODA", "nature": "cdi", "debut": "2026-11-02", "salaire_mensuel": 3400, "quotite_contrat": 151.67}})),
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-02", "fin": "2026-11-10", "motif": "04", "date_accident": "2026-11-02", "subrogation": "02"})),
        ("arret-enregistrer", json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-16", "fin": "2026-11-18", "motif": "19", "subrogation": "02"})),
    ] {
        let r = route(&c, nom, charge, "2026-11-02");
        assert_eq!(r["ok"], json!(true), "le permanent des exemples : {} : {}", nom, r);
    }
    // Le retrait de l'engagement en dernier : les exemples des imprevus portent sur celui que valider-engagement pose.
    let mut fiches = catalogue();
    fiches.sort_by_key(|f| f.nom == "retirer-engagement");
    // Un administrateur, pour son tableau et son dépôt groupé : la compagnie est rattachée à son registre, après les
    // exemples du mot de passe du logiciel (celui d'une compagnie seule).
    let inst = c.parent().unwrap().parent().unwrap();
    let registre = json!({"compagnies": [{"dossier": c.strip_prefix(inst).unwrap().to_string_lossy()}]});
    for f in &fiches {
        if f.nom.starts_with("administrateur-") && !inst.join("administrateur.json").exists() {
            std::fs::write(inst.join("administrateur.json"), registre.to_string()).unwrap();
        }
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
