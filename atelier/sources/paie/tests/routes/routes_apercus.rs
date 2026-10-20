//! `paie route` : les apercus, qui ne font que lire (`documents-liste`, `bilan-repetition`, `depot-apercu` d'une DPAE et
//! d'une DSN). Sur `fictive-3090-petite` (config, aout et septembre 2026, journal vide), que chaque test mute ; ce qu'une
//! charge ou un fichier ne permet pas de lire refuse.

use crate::outils::{compagnie, fictive, dossier_de_test, ecrire, route, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08", "2026-09"], &dossier_de_test("routes_apercus", nom).join("compagnie"))
}

fn lire(c: &Path, nom: &str, charge: Value) -> Value {
    reussi(&route(c, nom, charge, "2026-10-03")).clone()
}

fn refuse(c: &Path, nom: &str, charge: Value) -> bool {
    route(c, nom, charge, "2026-10-03")["ok"] == json!(false)
}

fn ecrire_texte(c: &Path, rel: &str, t: &str) {
    let p = c.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, t).unwrap();
}

// ------------------------------------------------------------------ documents-liste

fn pdf(c: &Path, relatif: &str, octets: usize) {
    let p = c.join(relatif);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, vec![b'x'; octets]).unwrap();
}

#[test]
fn documents_liste_trie_filtre_et_nomme_le_salarie() {
    let c = &compagnie("documents");
    for (n, t) in [
        ("sorties/2026-09/envoi/Bulletin_DUPONT_Sept2026.pdf", 3),
        ("sorties/2026-09/envoi/Contrat_DUPONT_Sept2026.pdf", 4),
        ("sorties/2026-09/envoi/Contrats_Sept2026.pdf", 5),
        ("sorties/2026-09/envoi/AER_MARTIN_Sept2026.pdf", 6),
        ("sorties/2026-09/envoi/CertificatCS_MARTIN_Sept2026.pdf", 7),
        ("sorties/2026-09/envoi/bulletin_minuscule_x_y.pdf", 8),
        ("sorties/2026-09/envoi/Bulletin_X_Y.PDF", 9),
        ("sorties/2026-09/envoi/.cache.pdf", 1),
        ("sorties/2026-09/Bulletins_Sept2026.pdf", 10),
    ] {
        pdf(c, n, t);
    }
    let tout = lire(c, "documents-liste", json!({"mois": "2026-09"}));
    let noms: Vec<&str> = tout["fichiers"].as_array().unwrap().iter().map(|f| f["nom"].as_str().unwrap()).collect();
    // `envoi/` d'abord, trie par points de code (majuscules avant minuscules, `.` avant les lettres) ; puis le mois.
    assert_eq!(
        noms,
        [
            ".cache.pdf",
            "AER_MARTIN_Sept2026.pdf",
            "Bulletin_DUPONT_Sept2026.pdf",
            "CertificatCS_MARTIN_Sept2026.pdf",
            "Contrat_DUPONT_Sept2026.pdf",
            "Contrats_Sept2026.pdf",
            "bulletin_minuscule_x_y.pdf",
            "Bulletins_Sept2026.pdf"
        ]
    );
    assert_eq!(tout["mois"], json!("2026-09"));
    let f = &tout["fichiers"];
    assert_eq!(f[1], json!({"nom": "AER_MARTIN_Sept2026.pdf", "octets": 6, "salarie": "MARTIN"}));
    assert_eq!(f[5]["salarie"], json!(""), "deux morceaux seulement : pas de salarie");
    assert_eq!(f[0]["salarie"], json!(""), "`.cache` : le stem est `.cache`");

    let contrats = lire(c, "documents-liste", json!({"mois": "2026-09", "quoi": "contrats"}));
    let noms: Vec<&str> = contrats["fichiers"].as_array().unwrap().iter().map(|f| f["nom"].as_str().unwrap()).collect();
    assert_eq!(noms, ["Contrat_DUPONT_Sept2026.pdf", "Contrats_Sept2026.pdf"], "les groupes du mois ne viennent pas avec un `quoi`");

    let docs = lire(c, "documents-liste", json!({"mois": "2026-09", "quoi": "documents"}));
    let noms: Vec<&str> = docs["fichiers"].as_array().unwrap().iter().map(|f| f["nom"].as_str().unwrap()).collect();
    assert_eq!(noms, ["AER_MARTIN_Sept2026.pdf", "Bulletin_DUPONT_Sept2026.pdf", "CertificatCS_MARTIN_Sept2026.pdf"]);

    // Un `quoi` inconnu mais vrai : aucun prefixe, mais plus de fichiers groupes.
    let autre = lire(c, "documents-liste", json!({"mois": "2026-09", "quoi": "autre"}));
    assert_eq!(autre["fichiers"].as_array().unwrap().len(), 7);
    // Faux (`""`, `0`, `null`) : comme absent.
    for q in [json!(""), json!(0), json!(null), json!(false)] {
        assert_eq!(lire(c, "documents-liste", json!({"mois": "2026-09", "quoi": q}))["fichiers"].as_array().unwrap().len(), 8);
    }
}

#[test]
fn documents_liste_sans_dossier_rend_une_liste_vide_et_refuse_une_charge_fausse() {
    let c = &compagnie("documents-refus");
    assert_eq!(lire(c, "documents-liste", json!({"mois": "2031-01"})), json!({"mois": "2031-01", "fichiers": []}));
    ecrire_texte(c, "sorties/2031-02/envoi", "un fichier, pas un dossier");
    assert_eq!(lire(c, "documents-liste", json!({"mois": "2031-02"}))["fichiers"], json!([]), "`envoi` est un fichier : rien a lister");
    for charge in [json!({}), json!({"mois": 5}), json!({"mois": null}), json!({"mois": "2026-09", "quoi": ["contrats"]}), json!({"mois": "2026-09", "quoi": {}})] {
        assert!(refuse(c, "documents-liste", charge.clone()), "{}", charge);
    }
}

// ------------------------------------------------------------------ bilan-repetition

fn empreinte_de(octets: &[u8]) -> String {
    paie::journal::empreinte(octets)
}

fn depot_test(quoi: &str, fichier: &str, contenu: &[u8], retours: Value) -> Value {
    json!({"quoi": quoi, "mode": "test", "accepte": true, "fichier": fichier, "empreinte": empreinte_de(contenu), "retours": retours, "mois": "2026-09"})
}

#[test]
fn bilan_repetition_dit_ou_en_est_le_test_de_ce_fichier() {
    let c = &compagnie("repetition");
    ecrire_texte(c, "sorties/2026-09/DSN_Sept2026.dsn", "le contenu a declarer");
    let fichier = "sorties/2026-09/DSN_Sept2026.dsn";
    let charge = json!({"quoi": "dsn", "fichier": fichier});

    // Aucun test : rien de conforme.
    let r = lire(c, "bilan-repetition", charge.clone());
    assert_eq!(r["conforme"], json!(null));
    assert_eq!(r["compte_rendu"], json!(""));
    assert_eq!(r["fichier"], json!("DSN_Sept2026.dsn"));
    assert_eq!(r["empreinte"], json!(empreinte_de(b"le contenu a declarer")));
    assert_eq!(r["incident"], json!(""));

    // L'ACCUSE D'ENREGISTREMENT SEUL (nature 10, `etat_envoi` OK) n'est pas un compte rendu : « envoi recu », pas
    // « conforme » (regle 5 ; tickets #221, #222 : l'ecran disait « testee — conforme » sur lui seul).
    let accuse = depot_test("dsn", "DSN_Sept2026.dsn", b"le contenu a declarer", json!([{"nature": "10", "verdict": "OK"}]));
    ecrire(c, "journal/depots.json", &json!([accuse]));
    let r = lire(c, "bilan-repetition", charge.clone());
    assert_eq!(r["compte_rendu"], json!("attendu"), "{}", r);

    // Un test accepte, et son BILAN (nature 11) OK : c'est lui qui dit conforme.
    let bon = depot_test("dsn", "DSN_Sept2026.dsn", b"le contenu a declarer", json!([{"nature": "10", "verdict": "OK"}, {"nature": "11", "verdict": "OK"}]));
    ecrire(c, "journal/depots.json", &json!([bon]));
    let r = lire(c, "bilan-repetition", charge.clone());
    assert_eq!(r["compte_rendu"], json!("ok"));
    assert_eq!(r["conforme"]["fichier"], json!("DSN_Sept2026.dsn"));

    // Sans compte rendu : « attendu ».
    let attendu = depot_test("dsn", "DSN_Sept2026.dsn", b"le contenu a declarer", json!([]));
    ecrire(c, "journal/depots.json", &json!([attendu]));
    assert_eq!(lire(c, "bilan-repetition", charge.clone())["compte_rendu"], json!("attendu"));

    // Un compte rendu KO : le test est rate, pas conforme.
    let ko = depot_test("dsn", "DSN_Sept2026.dsn", b"le contenu a declarer", json!([{"verdict": "KO"}]));
    ecrire(c, "journal/depots.json", &json!([ko]));
    let r = lire(c, "bilan-repetition", charge.clone());
    assert_eq!(r["conforme"], json!(null));
    assert_eq!(r["ratees"].as_array().unwrap().len(), 1);

    // Le fichier a change depuis le test : la ligne est perimee.
    let vieux = depot_test("dsn", "DSN_Sept2026.dsn", b"l'ancien contenu", json!([{"verdict": "OK"}]));
    ecrire(c, "journal/depots.json", &json!([vieux]));
    let r = lire(c, "bilan-repetition", charge);
    assert_eq!(r["conforme"], json!(null));
    assert_eq!(r["perimees"].as_array().unwrap().len(), 1);
}

#[test]
fn bilan_repetition_resout_le_chemin_et_refuse_ce_qui_ne_designe_pas_un_fichier() {
    let c = &compagnie("repetition-refus");
    let sous = |fichier: Value| json!({"quoi": "dsn", "fichier": fichier});
    // Un mois nu designe sa saisie ; un chemin relatif se cherche sous les donnees.
    assert_eq!(lire(c, "bilan-repetition", sous(json!("2026-09")))["fichier"], json!("2026-09.json"));
    assert_eq!(lire(c, "bilan-repetition", sous(json!("paies//./2026-08.json")))["fichier"], json!("2026-08.json"));
    for charge in [json!({}), json!({"quoi": "dsn"}), sous(json!(null)), sous(json!("  ")), sous(json!("absent.dsn")), sous(json!(5)), sous(json!("paies")), sous(json!("a\u{0}b"))] {
        assert!(refuse(c, "bilan-repetition", charge.clone()), "{}", charge);
    }
    ecrire_texte(c, "journal/depots.json", "{abime");
    assert!(refuse(c, "bilan-repetition", sous(json!("2026-09"))), "un journal abime");
}

// ------------------------------------------------------------------ depot-apercu, la DPAE

fn poser_dpae(c: &Path) -> String {
    let src = fictive("fictive-3090-petite").join("sorties/2026-08/DPAE_Aout2026.txt");
    let dst = c.join("sorties/2026-08/DPAE_Aout2026.txt");
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    std::fs::copy(src, &dst).unwrap();
    "sorties/2026-08/DPAE_Aout2026.txt".to_string()
}

#[test]
fn depot_apercu_dpae_relit_le_fichier_et_dit_les_embauches_deja_declarees() {
    let c = &compagnie("dpae");
    let f = poser_dpae(c);
    let r = lire(c, "depot-apercu", json!({"quoi": "dpae", "fichier": f}));
    assert_eq!(r["nom"], json!("DPAE_Aout2026.txt"));
    assert_eq!(r["reel"], json!(true));
    assert_eq!(r["indicateur"], json!("120"));
    let embauches = r["embauches"].as_array().unwrap();
    assert_eq!(r["enregistrements"], json!(embauches.len()));
    assert!(!embauches.is_empty());
    assert!(embauches.iter().all(|e| e["deja_declaree"] == json!(false)));
    assert!(embauches.iter().all(|e| e["salarie"] != json!("?")), "les NIR de la compagnie fictive sont connus : {}", r);
    assert_eq!(r["mois"], json!("2026-08"));
    assert_eq!(r["deja_depose"], json!(null));
    assert_eq!(r["repetition"]["conforme"], json!(null));
    // `pas_avant` : huit jours avant l'embauche la plus lointaine.
    let dernier = embauches.iter().map(|e| e["debut"].as_str().unwrap()).max().unwrap().to_string();
    assert!(r["pas_avant"].as_str().unwrap() < dernier.as_str());

    // Le journal dit une embauche deja declaree et le fichier deja depose.
    let e0 = &embauches[0];
    let octets = std::fs::read(c.join(&f)).unwrap();
    ecrire(c, 
        "journal/depots.json",
        &json!([{"quoi": "dpae", "mode": "reel", "accepte": true, "quand": "2026-08-01T10:00:00", "fichier": "DPAE_Aout2026.txt",
                 "empreinte": paie::journal::empreinte(&octets),
                 "detail": {"embauches": [{"salarie": e0["salarie"], "debut": e0["debut"]}]}}]),
    );
    let r = lire(c, "depot-apercu", json!({"quoi": "dpae", "fichier": f}));
    assert_eq!(r["embauches"][0]["deja_declaree"], json!(true));
    assert_eq!(r["deja_depose"]["fichier"], json!("DPAE_Aout2026.txt"));
}

#[test]
fn depot_apercu_dpae_refuse_ce_qui_n_est_pas_un_fichier_et_lit_un_flux_vide() {
    let c = &compagnie("dpae-refus");
    for charge in [json!({}), json!({"quoi": "dpae"}), json!({"quoi": "dpae", "fichier": "absent.txt"}), json!({"quoi": "dpae", "fichier": ""}), json!({"quoi": "dpae", "fichier": 5}), json!({"quoi": "dpae", "fichier": "paies"})] {
        assert!(refuse(c, "depot-apercu", charge.clone()), "{}", charge);
    }
    // Un flux sans embauche : rien a dire, pas de mois.
    ecrire_texte(c, "vide.txt", "");
    let r = lire(c, "depot-apercu", json!({"quoi": "dpae", "fichier": "vide.txt"}));
    assert_eq!(r["enregistrements"], json!(0));
    assert_eq!(r["mois"], json!(""));
    assert_eq!(r["pas_avant"], json!(""));
}

// ------------------------------------------------------------------ depot-apercu, la DSN

/// La DSN d'aout de la compagnie fictive, et l'archive des bulletins dont elle porte les numeros.
fn poser_dsn(c: &Path) -> String {
    for nom in ["DSN_Aout2026.dsn", "Bulletins_Aout2026.pdf"] {
        let dst = c.join(&format!("sorties/2026-08/{}", nom));
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(fictive("fictive-3090-petite").join("sorties/2026-08").join(nom), &dst).unwrap();
    }
    "sorties/2026-08/DSN_Aout2026.dsn".to_string()
}

/// Une rubrique du fichier DSN, reecrite (les fins de ligne du fichier sont gardees).
fn changer_rubrique(c: &Path, relatif: &str, rubrique: &str, valeur: &str) {
    let p = c.join(relatif);
    let t = String::from_utf8(std::fs::read(&p).unwrap()).unwrap();
    let motif = format!("{},'", rubrique);
    let debut = t.find(&motif).unwrap_or_else(|| panic!("{} absente", rubrique)) + motif.len();
    let fin = debut + t[debut..].find('\'').unwrap();
    std::fs::write(&p, format!("{}{}{}", &t[..debut], valeur, &t[fin..])).unwrap();
}

fn depot_reel(ordre: &str, saisie: &str) -> Value {
    json!({"quoi": "dsn", "mode": "reel", "accepte": true, "quand": "2026-09-10T10:00:00", "mois": "2026-08",
           "fichier": "DSN_Aout2026.dsn", "empreinte": "0000000000000000",
           "detail": {"type": "01", "ordre": ordre, "empreinte_saisie": saisie, "etablissement": "00010"}})
}

#[test]
fn depot_apercu_dsn_relit_le_fichier_et_son_recapitulatif() {
    let c = &compagnie("dsn");
    let f = poser_dsn(c);
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["nom"], json!("DSN_Aout2026.dsn"));
    assert_eq!((r["mois"].as_str(), r["reel"].as_bool(), r["signalement"].as_bool()), (Some("2026-08"), Some(true), Some(false)));
    assert_eq!((r["nature"].as_str(), r["libelle_type"].as_str()), (Some("01"), Some("declaration normale")));
    assert_eq!(r["salaries"], json!(3));
    assert_eq!(r["evenement"], json!(""));
    assert_eq!(r["deja_depose"], json!(null));
    assert_eq!(r["repetition"]["conforme"], json!(null));
    // Les numeros de contrat se lisent dans l'archive des bulletins : la DSN et la saisie s'accordent.
    let rec = &r["recapitulatif"];
    assert_eq!((rec["contrats"].as_i64(), rec["contrats_saisis"].as_i64()), (Some(3), Some(3)));
    assert_eq!((rec["compare"].as_bool(), rec["conforme"].as_bool()), (Some(true), Some(true)), "{}", rec);
    assert_eq!(rec["salaries"][0]["nom"], json!("BRIOCHE"));
    let contrat = &rec["salaries"][0]["contrats"][0];
    assert_eq!((contrat["numero"].as_str(), contrat["brut"].as_str(), contrat["cachets"].as_str()), (Some("BR1.26.04"), Some("620.00"), Some("4")));
    assert_eq!(contrat["ecarts"], json!([]));
    assert_eq!(r["remplacement"]["deposee"], json!(false));
}

#[test]
fn depot_apercu_dsn_dit_ce_que_la_paie_dit_autrement() {
    let c = &compagnie("dsn-ecarts");
    let f = poser_dsn(c);
    // Le fichier dit un autre brut ; une saisie absente ne se compare pas.
    let t = std::fs::read_to_string(c.join(&f)).unwrap().replacen("S21.G00.51.013,'620.00'", "S21.G00.51.013,'621.00'", 1);
    ecrire_texte(c, &f, &t);
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["recapitulatif"]["conforme"], json!(false));
    assert_eq!(r["recapitulatif"]["salaries"][0]["contrats"][0]["ecarts"], json!(["brut : la paie dit 620.00, le fichier 621.00"]));
    std::fs::remove_file(c.join("paies/2026-08.json")).unwrap();
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["recapitulatif"]["compare"], json!(false));
    assert_eq!(r["recapitulatif"]["salaries"][0]["contrats"][0]["paie"], json!(false));
    // Un montant illisible : le recapitulatif le dit, la boite de depot s'affiche quand meme.
    changer_rubrique(c, &f, "S21.G00.51.013", "abc");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert!(r["recapitulatif"]["erreur"].is_string(), "{}", r["recapitulatif"]);
    assert_eq!(r["mois"], json!("2026-08"));
}

#[test]
fn depot_apercu_dsn_un_signalement_n_a_ni_recapitulatif_ni_remplacement() {
    let c = &compagnie("dsn-signalement");
    let f = poser_dsn(c);
    changer_rubrique(c, &f, "S20.G00.05.001", "07");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["signalement"], json!(true));
    assert_eq!(r["libelle_nature"], json!("signalement fin de contrat de travail unique"));
    assert_eq!((r["recapitulatif"].is_null(), r["remplacement"].is_null()), (true, true));
    assert_eq!(r["evenement"], json!("2026-08-22"));
}

#[test]
fn depot_apercu_dsn_l_annule_et_remplace_se_lit_dans_le_journal() {
    let c = &compagnie("dsn-remplacement");
    let f = poser_dsn(c);
    ecrire(c, "journal/depots.json", &json!([depot_reel("1", "dead")]));
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    let rem = &r["remplacement"];
    assert_eq!((rem["deposee"].as_bool(), rem["dernier_ordre"].as_i64(), rem["prochain_ordre"].as_i64()), (Some(true), Some(1), Some(2)));
    assert_eq!(rem["saisie_modifiee"], json!(true));
    assert_eq!(rem["fichier_remplace"], json!(false));
    // Un annule-et-remplace d'ordre 2 au-dessus du depot d'ordre 1 le remplace.
    changer_rubrique(c, &f, "S20.G00.05.002", "03");
    changer_rubrique(c, &f, "S20.G00.05.004", "2");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["remplacement"]["fichier_remplace"], json!(true));
    assert_eq!(r["libelle_type"], json!("annule et remplace integral"));
    // Un rang qui ne se lit pas : refus.
    changer_rubrique(c, &f, "S20.G00.05.004", "x");
    assert!(refuse(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f})));
    // Un depot d'un autre etablissement n'est pas celui de ce fichier.
    changer_rubrique(c, &f, "S20.G00.05.004", "2");
    changer_rubrique(c, &f, "S21.G00.11.001", "00028");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["remplacement"]["deposee"], json!(false));
}

#[test]
fn depot_apercu_dsn_refuse_ce_qui_n_est_pas_un_fichier_et_lit_un_fichier_vide() {
    let c = &compagnie("dsn-refus");
    for charge in [json!({"quoi": "dsn"}), json!({"quoi": "dsn", "fichier": "absent.dsn"}), json!({"quoi": "dsn", "fichier": ""}), json!({"quoi": "dsn", "fichier": 5}), json!({"quoi": "dsn", "fichier": "paies"})] {
        assert!(refuse(c, "depot-apercu", charge.clone()), "{}", charge);
    }
    // Un fichier vide se lit : rien n'y est declare.
    ecrire_texte(c, "vide.dsn", "");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": "vide.dsn"}));
    assert_eq!((r["mois"].as_str(), r["salaries"].as_i64(), r["reel"].as_bool()), (Some("?"), Some(0), Some(false)));
    assert_eq!(r["recapitulatif"]["salaries"], json!([]));
    assert_eq!(r["ordres"], json!([]));
}

/// Ticket #223 : chaque ordre de paiement (bloc S21.G00.20) dit a qui (le nom propre de `donnees/organismes-base.json`),
/// pour quoi, combien et pour quand, et d'ou vient la date (`ordres_paiement.rs`). La DSN d'aout 2026 de
/// `fictive-3090-petite` (moins de cinquante salaries : le 15).
#[test]
fn depot_apercu_dsn_dit_a_qui_pour_quoi_et_quand_chaque_ordre_est_preleve() {
    let c = &compagnie("dsn-ordres");
    let f = poser_dsn(c);
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    let ordres = r["ordres"].as_array().unwrap();
    let vu: Vec<(&str, &str, Option<&str>)> =
        ordres.iter().map(|o| (o["organisme"].as_str().unwrap(), o["montant"].as_str().unwrap(), o["date"].as_str())).collect();
    assert_eq!(
        vu,
        [
            // Audiens : « pour un mois M, le 25 du mois M+1 », retenu d'office pour les Conges Spectacles et la retraite.
            ("Caisse des Congés Spectacles", "281.48", Some("2026-09-25")),
            // S21.G00.20.011, lue dans le fichier.
            ("France Travail", "212.08", Some("2026-09-15")),
            ("Audiens Retraite", "181.76", Some("2026-09-25")),
            // P0983 est Audiens Sante Prevoyance (tables PREV et IVO de la norme) ; sa date est celle que le fichier porte.
            ("Audiens Santé Prévoyance", "39.03", Some("2026-10-31")),
            // L'Urssaf, organisme du bordereau : l'echeance de la DSN d'aout (R. 243-6, II), le mardi 15/09/2026.
            ("Urssaf", "693.00", Some("2026-09-15")),
        ]
    );
    assert_eq!(ordres[0]["quand"], json!("le 25/09/2026"));
    assert_eq!(ordres[0]["objet"], json!("La cotisation de congés payés des intermittents d'août 2026"));
    assert_eq!(ordres[2]["objet"], json!("La retraite complémentaire Agirc-Arrco d'août 2026"));
    assert_eq!(ordres[3]["objet"], json!("La prévoyance et la santé d'août 2026"));
    assert_eq!(ordres[4]["objet"], json!("Les cotisations et contributions sociales du bordereau Urssaf d'août 2026"));
    assert!(ordres[4]["source"].as_str().unwrap().contains("R. 243-6"), "{}", ordres[4]);
    assert!(ordres[0]["source"].as_str().unwrap().contains("le 25 du mois M+1"), "{}", ordres[0]);
    assert_eq!(ordres[1]["periode"], json!({"du": "2026-08-01", "au": "2026-08-31"}));
    // La DGFiP : le 15 du mois suivant jusqu'a quarante-neuf salaries (BOI-IR-PAS-30-10-30-20, § 50).
    changer_rubrique(c, &f, "S21.G00.20.001", "DGFIP");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    let o = &r["ordres"][0];
    assert_eq!((o["organisme"].as_str(), o["date"].as_str()), (Some("DGFiP"), Some("2026-09-15")));
    assert_eq!(o["quand"], json!("au plus tard le 15/09/2026"));
    assert!(o["source"].as_str().unwrap().contains("BOI-IR-PAS-30-10-30-20"), "{}", o);
    assert!(o["objet"].as_str().unwrap().contains("prélevé à la source"), "{}", o);
    // Un organisme que ni la base ni la configuration ne connaissent se dit par son identifiant, sans nom ni date inventes.
    changer_rubrique(c, &f, "S21.G00.20.001", "12345678900011");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    let o = &r["ordres"][0];
    assert_eq!(o["organisme"], json!("Organisme 12345678900011 (inconnu de la configuration)"));
    assert_eq!((o["date"].as_str(), o["quand"].as_str()), (None, Some("date non dite par la DSN")));
    // Une periode d'un autre mois est une regularisation.
    changer_rubrique(c, &f, "S21.G00.20.006", "01072026");
    changer_rubrique(c, &f, "S21.G00.20.007", "31072026");
    let r = lire(c, "depot-apercu", json!({"quoi": "dsn", "fichier": f}));
    assert_eq!(r["ordres"][0]["objet"], json!("Régularisation : les cotisations déclarées de juillet 2026"));
}
