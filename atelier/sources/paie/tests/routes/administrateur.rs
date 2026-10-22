//! L'administrateur (ticket #244, Leandre 22/10/2026 : « un logiciel = un administrateur ») : les contrats du coffre
//! (rattacher, detacher, changer le mot de passe de toutes les compagnies d'un coup, et rien d'ecrit sur un refus), puis le
//! tableau de bord et le depot groupe par la route, en test comme en reel, sur des compagnies de test d'une installation
//! de test. Aucun reseau : les compagnies qui deposent portent le marqueur de la compagnie fictive (leurs envois sont
//! joues, la route `deposer` deroule tout le reste : repetition, preuve, journal) ; celle qui refuse n'a pas de mot de
//! passe net-entreprises, et sa route le dit avant toute sortie.

use std::path::{Path, PathBuf};

use paie::administrateur::{self, Poste};
use paie::coffre::Coffre;
use serde_json::{json, Value};

use crate::outils::{compagnie, dossier_de_test, ecrire, installation, lire, refus, reussi};

const ADMIN: &str = "administrateur-essai-1";
const JOUR: &str = "2026-10-02";

/// Une installation de test (`logiciel/donnees` copie) : son dossier.
fn installation_de_test(nom: &str) -> PathBuf {
    installation(&dossier_de_test("administrateur", nom), "2026.10.22")
}

/// Une compagnie reelle de test, `compagnies/<nom>` de l'installation : la config de la petite fictive 3090, sans son
/// marqueur.
fn compagnie_config(inst: &Path, nom: &str) -> PathBuf {
    let r = inst.join("compagnies").join(nom);
    crate::outils::copier(&crate::outils::fictive("fictive-3090-petite").join("config"), &r.join("config"));
    r
}

/// La meme, entiere (saisies, journal, sorties), devenue ordinaire.
fn compagnie_entiere(inst: &Path, nom: &str) -> PathBuf {
    let r = compagnie::entiere("fictive-3090-petite", &inst.join("compagnies").join(nom));
    compagnie::ordinaire(&r);
    r
}

fn poste(inst: &Path) -> Poste {
    Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: "2026-10-02T10:00".into() }
}

fn coffre(p: &Poste, r: &Path) -> Coffre {
    p.coffre(r)
}

fn ouvre(p: &Poste, r: &Path, m: &str) -> bool {
    coffre(p, r).verifier_ouverture(m).unwrap()
}

fn octets(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

#[test]
fn rattacher_avec_le_bon_mot_de_passe_ouvre_et_un_mauvais_n_ecrit_rien() {
    let inst = installation_de_test("rattacher");
    let p = poste(&inst);
    let a = compagnie_config(&inst, "a");
    let b = compagnie_config(&inst, "b");
    coffre(&p, &a).poser_ouverture(ADMIN, None).unwrap();
    // La question ne se pose que pour une compagnie sans mot de passe, quand une autre en a un.
    let etat = administrateur::etat(&inst, Some(&b), &p);
    assert_eq!(etat["question"], json!(true), "{}", etat);
    assert_eq!(administrateur::etat(&inst, Some(&a), &p)["question"], json!(false));

    let avant_a = octets(&coffre(&p, &a).chemin());
    let refuse = administrateur::rattacher(&inst, &b, "pas-le-bon-1", None, &p);
    assert!(refuse.is_err(), "un mauvais mot de passe est refuse");
    assert!(!inst.join(administrateur::FICHIER).exists(), "un refus n'ecrit pas le registre");
    assert!(!coffre(&p, &b).chemin().exists(), "un refus n'ecrit pas le coffre");
    assert_eq!(octets(&coffre(&p, &a).chemin()), avant_a, "ni celui de l'autre");

    let noms = administrateur::rattacher(&inst, &b, ADMIN, None, &p).unwrap();
    assert_eq!(noms.len(), 2, "la premiere compagnie devient celle de l'administrateur avec la neuve : {:?}", noms);
    assert!(ouvre(&p, &b, ADMIN), "le mot de passe de l'administrateur ouvre la compagnie rattachee");
    assert!(administrateur::est_rattachee(&inst, &a) && administrateur::est_rattachee(&inst, &b));
    let registre = std::fs::read_to_string(inst.join(administrateur::FICHIER)).unwrap();
    assert!(!registre.contains(ADMIN), "le registre ne porte aucun secret");

    // Une troisieme, deja munie de son propre mot de passe : l'actuel est exige, puis celui de l'administrateur la scelle.
    let c = compagnie_config(&inst, "c");
    coffre(&p, &c).poser_ouverture("a-elle-seule-1", None).unwrap();
    let avant_c = octets(&coffre(&p, &c).chemin());
    assert!(administrateur::rattacher(&inst, &c, ADMIN, None, &p).is_err(), "sans son mot de passe actuel");
    assert!(administrateur::rattacher(&inst, &c, ADMIN, Some("pas-le-sien-1"), &p).is_err(), "avec un faux");
    assert!(administrateur::rattacher(&inst, &c, "pas-le-bon-1", Some("a-elle-seule-1"), &p).is_err(), "avec un faux administrateur");
    assert_eq!(octets(&coffre(&p, &c).chemin()), avant_c, "trois refus, rien d'ecrit");
    assert!(!administrateur::est_rattachee(&inst, &c));
    administrateur::rattacher(&inst, &c, ADMIN, Some("a-elle-seule-1"), &p).unwrap();
    assert!(ouvre(&p, &c, ADMIN) && !ouvre(&p, &c, "a-elle-seule-1"), "son ancien mot de passe ne l'ouvre plus");

    // Une compagnie fictive n'a pas d'administrateur.
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), compagnie_config(&inst, "f").join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    assert!(administrateur::rattacher(&inst, &inst.join("compagnies/f"), ADMIN, None, &p).is_err());
}

#[test]
fn detacher_pose_un_mot_de_passe_propre_et_celui_de_l_administrateur_ne_l_ouvre_plus() {
    let inst = installation_de_test("detacher");
    let p = poste(&inst);
    let a = compagnie_config(&inst, "a");
    let b = compagnie_config(&inst, "b");
    coffre(&p, &a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&inst, &b, ADMIN, None, &p).unwrap();

    let avant = octets(&coffre(&p, &b).chemin());
    assert!(administrateur::detacher(&inst, &b, "pas-le-bon-1", "propre-b-1", "propre-b-1", &p).is_err());
    assert!(administrateur::detacher(&inst, &b, ADMIN, "propre-b-1", "propre-b-2", &p).is_err(), "deux saisies differentes");
    assert!(administrateur::detacher(&inst, &b, ADMIN, ADMIN, ADMIN, &p).is_err(), "le meme que l'administrateur");
    assert_eq!(octets(&coffre(&p, &b).chemin()), avant, "les refus n'ecrivent rien");
    assert!(administrateur::est_rattachee(&inst, &b));

    administrateur::detacher(&inst, &b, ADMIN, "propre-b-1", "propre-b-1", &p).unwrap();
    assert!(ouvre(&p, &b, "propre-b-1"), "son propre mot de passe l'ouvre");
    assert!(!ouvre(&p, &b, ADMIN), "celui de l'administrateur ne l'ouvre plus");
    assert!(!administrateur::est_rattachee(&inst, &b) && administrateur::est_rattachee(&inst, &a));
    assert!(ouvre(&p, &a, ADMIN), "l'autre compagnie ne bouge pas");
}

#[test]
fn changer_le_mot_de_passe_de_l_administrateur_rescelle_tout_et_un_echec_n_en_change_aucune() {
    let inst = installation_de_test("changer");
    let p = poste(&inst);
    let cies: Vec<PathBuf> = ["a", "b", "c"].iter().map(|n| compagnie_config(&inst, n)).collect();
    coffre(&p, &cies[0]).poser_ouverture(ADMIN, None).unwrap();
    for r in &cies[1..] {
        administrateur::rattacher(&inst, r, ADMIN, None, &p).unwrap();
    }
    let photo = |cies: &[PathBuf]| cies.iter().map(|r| octets(&coffre(&p, r).chemin())).collect::<Vec<_>>();

    // Un mauvais actuel : rien ne change.
    let avant = photo(&cies);
    assert!(administrateur::changer(&inst, "pas-le-bon-1", "nouveau-1", "nouveau-1", &p).is_err());
    assert_eq!(photo(&cies), avant);

    // UN ECHEC AU MOMENT DE REMPLACER, au troisieme coffre (son fichier d'a cote est pris par un dossier) : les deux
    // premiers, deja remplaces, reviennent a l'ancien ; aucune compagnie ne change.
    let bloque = coffre(&p, &cies[2]).chemin().with_extension("bin.administrateur");
    std::fs::create_dir_all(&bloque).unwrap();
    let m = administrateur::changer(&inst, ADMIN, "nouveau-1", "nouveau-1", &p).unwrap_err();
    assert!(m.contains("revenus"), "{}", m);
    for r in &cies {
        assert!(ouvre(&p, r, ADMIN) && !ouvre(&p, r, "nouveau-1"), "{} n'a pas change", r.display());
    }
    std::fs::remove_dir_all(&bloque).unwrap();

    // Un coffre qui ne s'ouvre plus par l'actuel, au milieu : aucune ne change.
    let b_avant = octets(&coffre(&p, &cies[1]).chemin()).unwrap();
    std::fs::write(coffre(&p, &cies[1]).chemin(), b"{}").unwrap();
    assert!(administrateur::changer(&inst, ADMIN, "nouveau-1", "nouveau-1", &p).is_err());
    assert!(ouvre(&p, &cies[0], ADMIN) && ouvre(&p, &cies[2], ADMIN));
    std::fs::write(coffre(&p, &cies[1]).chemin(), b_avant).unwrap();

    let noms = administrateur::changer(&inst, ADMIN, "nouveau-1", "nouveau-1", &p).unwrap();
    assert_eq!(noms.len(), 3);
    for r in &cies {
        assert!(ouvre(&p, r, "nouveau-1") && !ouvre(&p, r, ADMIN), "{} rescelle", r.display());
    }
}

// ------------------------------------------------------------------ le tableau et le depot groupe

fn route(inst: &Path, racine: &Path, nom: &str, charge: Value) -> Value {
    let code = crate::outils::code_de(inst);
    crate::outils::route_avec(racine, nom, charge, JOUR, &[("PUPPETPAIE_CODE", code.as_str())])
}

fn marquer_fictive(r: &Path) {
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), r.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
}

/// La DSN de septembre 2026 de la compagnie, telle que le tableau la dit.
fn dsn_de_septembre<'a>(t: &'a Value, nom: &str) -> &'a Value {
    t["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["mois"] == json!("2026-09") && d["genre"] == json!("dsn") && d["fichier"].as_str().is_some_and(|f| f.contains(&format!("/compagnies/{}/", nom))))
        .unwrap_or_else(|| panic!("la DSN de septembre de {} : {}", nom, t))
}

/// Les lignes DSN de septembre 2026 du journal des depots.
fn depots_de_septembre(r: &Path) -> Vec<Value> {
    lire(r, "journal/depots.json").as_array().unwrap().iter().filter(|l| l["quoi"] == json!("dsn") && l["mois"] == json!("2026-09")).cloned().collect()
}

#[test]
fn le_depot_groupe_depose_chaque_compagnie_avec_ses_controles_et_un_refus_n_arrete_pas_les_autres() {
    let inst = installation_de_test("groupe");
    let p = poste(&inst);
    let (a, b, c) = (compagnie_entiere(&inst, "a"), compagnie_entiere(&inst, "b"), compagnie_entiere(&inst, "c"));
    coffre(&p, &a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&inst, &b, ADMIN, None, &p).unwrap();
    administrateur::rattacher(&inst, &c, ADMIN, None, &p).unwrap();
    // `a` et `b` jouent leurs envois (rien ne part) ; `c` n'a pas de mot de passe net-entreprises : son depot refuse.
    marquer_fictive(&a);
    marquer_fictive(&b);

    // LE TABLEAU : la DSN de septembre de chacune, ecrite et prete pour le test, avec son echeance.
    let t = reussi(&route(&inst, &a, "administrateur-tableau", json!({}))).clone();
    for n in ["a", "b", "c"] {
        let d = dsn_de_septembre(&t, n);
        assert_eq!(d["pret"], json!({"test": true, "reel": false}), "{}", d);
        assert_eq!(d["butoir"], json!("2026-10-15"), "l'echeance des rappels : {}", d);
    }
    assert_eq!(t["groupe"]["test"]["propose"], json!(true), "{}", t["groupe"]);
    assert_eq!(t["groupe"]["reel"]["propose"], json!(false));
    assert!(a.join(administrateur::TABLEAU).is_file(), "le tableau de la compagnie est garde");
    // Ce qui est garde se relit sans calcul, et dit la meme chose.
    assert_eq!(administrateur::tableau_garde(&inst)["groupe"]["test"], t["groupe"]["test"]);

    // LE DEPOT GROUPE EN TEST : a et b partent, c refuse, et son refus n'arrete pas les autres.
    let d = reussi(&route(&inst, &a, "administrateur-deposer", json!({"reel": false}))).clone();
    assert_eq!((d["partis"].as_u64(), d["refuses"].as_u64()), (Some(2), Some(1)), "{}", d);
    let refus_c = d["resultats"].as_array().unwrap().iter().find(|x| x["parti"] == json!(false)).unwrap();
    assert!(refus_c["refus"].as_str().unwrap().contains("net-entreprises"), "{}", refus_c);
    for r in [&a, &b] {
        let l = depots_de_septembre(r);
        assert!(l.iter().any(|x| x["mode"] == json!("test")), "{} : le test est note au journal de sa compagnie", r.display());
    }
    assert!(depots_de_septembre(&c).iter().all(|x| x["mode"] != json!("test") || x["accepte"] != json!(true)), "c n'a rien depose");

    // Le bilan de chaque test revient conforme (le certificat, nature 11) : chaque compagnie est prete a envoyer.
    for r in [&a, &b] {
        let mut j = lire(r, "journal/depots.json");
        let i = j.as_array().unwrap().iter().rposition(|l| l["quoi"] == json!("dsn") && l["mois"] == json!("2026-09") && l["mode"] == json!("test")).unwrap();
        j[i]["accepte"] = json!(true);
        j[i]["retours"] = json!([{"nature": "11", "verdict": "OK", "anomalies": []}]);
        ecrire(r, "journal/depots.json", &j);
    }
    let t = reussi(&route(&inst, &a, "administrateur-tableau", json!({}))).clone();
    assert_eq!(dsn_de_septembre(&t, "a")["pret"]["reel"], json!(true), "{}", dsn_de_septembre(&t, "a"));
    assert_eq!(t["groupe"]["reel"]["propose"], json!(true), "{}", t["groupe"]);

    // EN REEL, un mauvais mot de passe : rien ne part.
    let avant = depots_de_septembre(&a).len();
    let (classe, _) = refus(&route(&inst, &a, "administrateur-deposer", json!({"reel": true, "motdepasse": "pas-le-bon-1"})));
    assert_eq!(classe, "PreuveInsuffisante");
    assert_eq!(depots_de_septembre(&a).len(), avant, "rien n'est note");

    // Le bon : chaque compagnie prete depose en reel, par sa route `deposer`.
    let d = reussi(&route(&inst, &a, "administrateur-deposer", json!({"reel": true, "motdepasse": ADMIN}))).clone();
    assert_eq!(d["partis"].as_u64(), Some(2), "{}", d);
    for r in [&a, &b] {
        assert!(depots_de_septembre(r).iter().any(|x| x["mode"] == json!("reel")), "{} : le reel est note", r.display());
    }
    // Et le tableau refait ne la propose plus.
    assert_eq!(d["tableau"]["groupe"]["reel"]["declarations"].as_u64(), Some(0), "{}", d["tableau"]["groupe"]);
}

/// CE QU'UN ARRET FAIT NAITRE entre au tableau, du plus presse au moins presse, avec la date limite qu'`obligations.rs`
/// lui donne : la declaration d'accident (quarante-huit heures, HORS DSN : elle s'affiche avec ou la faire, rien ne la
/// depose) avant le signalement d'arret (cinq jours ouvres sans subrogation), avant la DSN du mois. Le signalement, une
/// fois ecrit, est pret pour le test et part avec le depot groupe.
#[test]
fn les_obligations_d_un_arret_entrent_au_tableau_par_urgence_et_le_signalement_ecrit_part_avec_le_groupe() {
    let inst = installation_de_test("obligations");
    let p = poste(&inst);
    let (a, b) = (compagnie_entiere(&inst, "a"), compagnie_entiere(&inst, "b"));
    coffre(&p, &a).poser_ouverture(ADMIN, None).unwrap();
    administrateur::rattacher(&inst, &b, ADMIN, None, &p).unwrap();
    marquer_fictive(&a);
    marquer_fictive(&b);
    // Un accident du travail de BRIOCHE, su le 30/09/2026, sans subrogation (comme `routes_obligations`).
    let arret = json!({
        "genre": "arret", "salarie": "BRIOCHE", "contrat_debut": "2026-01-02", "contrat_numero": "BR9.26.01", "motif": "06",
        "dernier_jour_travaille": "2026-09-29", "fin_previsionnelle": "2026-10-20", "subrogation": "02",
        "subrogation_debut": null, "subrogation_fin": null, "reprise": null, "reprise_motif": null,
        "date_accident": "2026-09-30", "saisi_le": "2026-09-30 10:00", "qui": "essai", "note": "", "envois": [],
    });
    ecrire(&a, "imprevus/2026-09.json", &json!({"mois": "2026-09", "imprevus": [arret]}));

    let t = reussi(&route(&inst, &a, "administrateur-tableau", json!({}))).clone();
    let de_a: Vec<&Value> = t["declarations"].as_array().unwrap().iter().filter(|d| d["compagnie"] == t["compagnies"][0]["nom"] && d["fichier"].as_str().map_or(true, |f| f.contains("/compagnies/a/"))).collect();
    let rang = |genre: &str| de_a.iter().position(|d| d["genre"] == json!(genre)).unwrap_or_else(|| panic!("pas de {} : {}", genre, t));
    let accident = de_a[rang("hors_dsn")];
    assert!(accident["butoir"].as_str().is_some() && !accident["ou"].as_array().unwrap().is_empty(), "{}", accident);
    assert_eq!(accident["pret"], json!({"test": false, "reel": false}), "hors DSN, rien ne la depose");
    let signalement = de_a[rang("signalement")];
    assert_eq!(signalement["pret"]["test"], json!(false), "pas encore ecrit : {}", signalement);
    assert!(rang("hors_dsn") < rang("signalement") && rang("signalement") < rang("dsn"), "par urgence : {:?}", de_a.iter().map(|d| (&d["genre"], &d["butoir"])).collect::<Vec<_>>());

    // Le signalement s'ecrit dans la compagnie ; le tableau le dit pret pour le test, et le groupe l'emporte.
    reussi(&route(&inst, &a, "arret-signaler", json!({"salarie": "Brioche", "dernier_jour": "2026-09-29", "nature": "04"})));
    let t = reussi(&route(&inst, &a, "administrateur-tableau", json!({}))).clone();
    let s = t["declarations"].as_array().unwrap().iter().find(|d| d["genre"] == json!("signalement")).unwrap();
    assert_eq!(s["pret"]["test"], json!(true), "{}", s);
    let d = reussi(&route(&inst, &a, "administrateur-deposer", json!({"reel": false}))).clone();
    assert_eq!(d["partis"].as_u64(), Some(3), "les deux DSN de septembre et le signalement : {}", d);
    assert!(d["resultats"].as_array().unwrap().iter().any(|x| x["genre"] == json!("signalement") && x["parti"] == json!(true)), "{}", d);
}
