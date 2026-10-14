//! L'import des paies declarees ailleurs : `importation-apercu` (rien n'est ecrit) et `importer`, depuis un export
//! (DSN et AEM) ou le dossier `import/` (DSN, AEM, bulletins PDF) ; ce qui est d'une autre entreprise n'est pas lu,
//! l'incident propose est caviarde. Sur la configuration de `fictive-3090-petite`, avec un export fictif sous `export/`.

use crate::outils::{compagnie, dossier_de_test, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn fictive() -> PathBuf {
    crate::outils::fictive("fictive-3090-petite")
}

/// La configuration de la compagnie fictive, et un export fictif : la DSN de mars 2025 sous `export/DSN`, `export/AEM_EDI` vide.
fn compagnie(nom: &str) -> PathBuf {
    let racine = compagnie::saisies("fictive-3090-petite", &[], &dossier_de_test("routes_importation", nom).join("compagnie"));
    std::fs::remove_file(racine.join("journal/depots.json")).unwrap();
    std::fs::create_dir_all(racine.join("export/DSN")).unwrap();
    std::fs::create_dir_all(racine.join("export/AEM_EDI")).unwrap();
    std::fs::copy(fictive().join("sorties/2025-03/DSN_Mars2025.dsn"), racine.join("export/DSN/DSN_Mars2025.dsn")).unwrap();
    racine
}

/// Un enregistrement AEM au Siret de la compagnie fictive (`importation::AEM_SIRET`), attestation n° 000123.
fn aem(nir: &str, debut: &str, fin: &str, objet: &str) -> String {
    aem_de(Some("12345692300010"), nir, debut, fin, objet)
}

/// Le meme, au Siret donne (`None` : la zone reste blanche).
fn aem_de(siret: Option<&str>, nir: &str, debut: &str, fin: &str, objet: &str) -> String {
    let zones: Vec<(usize, &str)> = std::iter::once((36, "000123")).chain(siret.map(|s| (50, s))).collect();
    crate::outils::aem(&zones, nir, debut, fin, objet)
}

/// L'en-tete que le tri du dossier `import/` reconnait.
fn marquee(enregistrement: String) -> String {
    format!("AEMEDIC2{}", &enregistrement[8..])
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    crate::outils::route(racine, nom, charge, "2026-10-03")
}

#[test]
fn un_contrat_deja_dans_l_historique_n_est_plus_nouveau() {
    let racine = compagnie("historique");
    std::fs::write(
        racine.join("export").join("AEM_EDI").join("AEM_EDIV5_1.txt"),
        format!("{}\r\n", aem("2881175999001", "20250310", "20250314", "256Z00920022")),
    )
    .unwrap();
    // BRIOCHE a une fiche (le NIR de la compagnie fictive) : le contrat est a importer.
    let r = route(&racine, "importation-apercu", json!({}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    assert_eq!(r["resultat"]["contrats"], json!(1));
    assert_eq!(r["resultat"]["contrats_nouveaux"], json!(1), "{}", r);
    // Le meme contrat, deja dans `historique/engagements.json` (le brut s'y lit comme texte).
    std::fs::create_dir_all(racine.join("historique")).unwrap();
    std::fs::write(
        racine.join("historique").join("engagements.json"),
        json!({"engagements": [{
            "salarie": "BRIOCHE", "debut": "2025-03-10", "fin": "2025-03-14", "numero_objet": "256Z00920022",
            "heures": 24, "cachets": 5, "jours": 5, "brut": "100.00"
        }]})
        .to_string(),
    )
    .unwrap();
    let r = route(&racine, "importation-apercu", json!({}));
    assert_eq!(r["resultat"]["contrats"], json!(1));
    assert_eq!(r["resultat"]["contrats_nouveaux"], json!(0), "{}", r);
}

// ------------------------------------------------------------------ le dossier `import/`

use crate::bulletins_fictifs;
use bulletins_fictifs::{a_heures, a_repetitions, b_cachets, page_a, page_b, pdf, pdf_en_traces};

/// Un texte de la page fabriquee remplace par un autre (le Siret de la compagnie fictive, un autre salarie...).
fn remplacer(page: Vec<Value>, de: &str, vers: &str) -> Vec<Value> {
    page.into_iter()
        .map(|mut b| {
            if let Some(t) = b["texte"].as_str() {
                b["texte"] = json!(t.replace(de, vers));
            }
            b
        })
        .collect()
}

/// Le Siret de la compagnie fictive, a la place de celui des PDF fictifs.
fn a_la_compagnie(page: Vec<Value>) -> Vec<Value> {
    remplacer(remplacer(page, "99999999900017", "12345692300010"), "999 999 999 00025", "123 456 923 00010")
}

/// CoolPaie : la paie de BRIOCHE que la DSN de mars 2025 declare (memes dates), mais avec d'autres montants -- le
/// bulletin l'emporte. Ne de la veille de ce que dit la fiche : un ecart.
fn a_mars_2025() -> Vec<Value> {
    let mut a = a_repetitions();
    a.numero = "3/2025";
    a.objet = Some(("256Z00920022", "Les Pupazzi"));
    a.periode = "du 10/03/2025 au 14/03/2025";
    a.dates = vec!["du 10/03/2025 au 14/03/2025"];
    a.lignes = vec![("5", "Cachets isolés", "146,00", "730,00")];
    a.brut = "730,00";
    a.csg = "717,22";
    a.net = "617,82"; // 730,00 - 112,18
    a.pas = ["600,00", "4.173", "25.04", "25,04"];
    a.net_imposable = ["600,00", "600,00"];
    a.net_a_payer = "592,78"; // 617,82 - 25,04
    a.regle = "14/03/2025";
    a.cumuls = [["5,00", "5,00"], ["730,00", "730,00"], ["1 600,00", "1 600,00"], ["1 000,00", "1 000,00"]];
    a.total_verse = "1 000,00";
    remplacer(a_la_compagnie(page_a(&a)), "04/11/1988", "03/11/1988")
}

/// CoolPaie : un salarie qu'aucune fiche ni aucune DSN ne porte (NIR et nom inventes), sur un spectacle inconnu.
fn a_nouveau_salarie() -> Vec<Value> {
    remplacer(remplacer(a_la_compagnie(page_a(&a_repetitions())), "288117599900146", "185079999912388"), "BRIOCHE FANCHON", "TARTEMPION ZOE")
}

/// Le dossier `import/` fictif : une DSN, une AEM, deux PDF CoolPaie, un sPAIEctacle, un PDF sans texte, un `.txt`,
/// et un bulletin d'un mois deja saisi.
fn boite_aux_lettres(nom: &str) -> PathBuf {
    let racine = compagnie(nom);
    let import = racine.join("import");
    std::fs::create_dir_all(import.join("aem")).unwrap();
    std::fs::create_dir_all(import.join("bulletins")).unwrap();
    std::fs::copy(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn"), import.join("DSN_Mars2025.dsn")).unwrap();
    std::fs::write(import.join("aem/AEM_EDIV5_1.txt"), format!("{}\r\n", marquee(aem("2881175999001", "20250310", "20250314", "256Z00920022")))).unwrap();
    pdf(vec![a_mars_2025(), a_nouveau_salarie()], &import.join("bulletins").join("coolpaie.pdf"));
    pdf(vec![a_la_compagnie(page_b(&b_cachets()))], &import.join("bulletins").join("spaiectacle.pdf"));
    let mut juin = a_heures();
    juin.periode = "du 09/06/2025 au 13/06/2025";
    juin.dates = vec!["du 09/06/2025 au 13/06/2025"];
    pdf(vec![a_la_compagnie(page_a(&juin))], &import.join("juin.pdf"));
    std::fs::write(racine.join("paies").join("2025-06.json"), "{}").unwrap();
    std::fs::write(import.join("scan.pdf"), pdf_en_traces()).unwrap();
    std::fs::write(import.join("notes.txt"), "a ne pas oublier\n").unwrap();
    racine
}

fn fichier<'a>(r: &'a Value, chemin: &str) -> &'a Value {
    r["tri"]["fichiers"].as_array().unwrap().iter().find(|f| f["chemin"] == json!(chemin)).unwrap_or_else(|| panic!("{} absent : {}", chemin, r["tri"]))
}

fn sort(r: &Value, chemin: &str) -> (Value, Value) {
    let f = fichier(r, chemin);
    (f["sort"].clone(), f["raison"].clone())
}

#[test]
fn le_dossier_import_rend_un_compte_rendu_par_fichier_les_fiches_les_contrats_et_les_paies() {
    let racine = boite_aux_lettres("dossier");
    let r = route(&racine, "importation-apercu", json!({"dossier": ""}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let r = &r["resultat"];
    assert_eq!(r["ecrit"], json!(false));
    assert_eq!(r["source"], json!(racine.join("import").display().to_string()));

    // Le compte rendu : chaque fichier, son sort et sa raison.
    assert_eq!(r["tri"]["total"], json!(7), "{}", r["tri"]);
    assert_eq!(
        sort(r, "DSN_Mars2025.dsn"),
        (json!("repris"), json!("DSN de 2025-03 : 2 versement(s), dont 2 à reprendre"))
    );
    assert_eq!(sort(r, "aem/AEM_EDIV5_1.txt"), (json!("repris"), json!("AEM : 1 contrat(s) attesté(s), dont 1 nouveau(x)")));
    assert_eq!(sort(r, "bulletins/coolpaie.pdf").0, json!("repris"));
    assert_eq!(sort(r, "bulletins/coolpaie.pdf").1, json!("2 pages : 2 repris"));
    assert_eq!(sort(r, "bulletins/spaiectacle.pdf").0, json!("repris"));
    assert_eq!(sort(r, "juin.pdf"), (json!("deja_saisi"), json!("le mois 2025-06 a sa saisie dans paies/ : non repris")));
    assert_eq!(sort(r, "scan.pdf").0, json!("sans_texte"));
    assert_eq!(sort(r, "notes.txt"), (json!("non_reconnu"), json!("ni DSN, ni AEM, ni PDF, ni archive zip (extension .txt)")));
    assert_eq!(r["tri"]["comptes"]["repris"], json!(4));
    assert_eq!(r["tri"]["comptes"]["deja_saisi"], json!(1));
    assert_eq!(r["tri"]["comptes"]["sans_texte"], json!(1));
    assert_eq!(r["tri"]["comptes"]["non_reconnu"], json!(1));
    assert!(r["avertissements"].as_array().unwrap().contains(&json!(
        "2 fichier(s) du dossier ne sont pas repris : le compte rendu (`tri`) dit pourquoi, fichier par fichier."
    )));

    // La page qui contredit la DSN : le bulletin l'emporte, l'ecart est note.
    let p = &fichier(r, "bulletins/coolpaie.pdf")["pages"][0];
    assert_eq!(p["sort"], json!("repris"));
    assert_eq!(p["salarie"], json!("BRIOCHE"));
    assert_eq!(p["paie"], json!("completee"));
    assert_eq!(p["contrat"], json!("ecart"), "{}", p);
    assert_eq!(
        p["raison"],
        json!("complète la paie déclarée en DSN (2025-03) ; le bulletin l'emporte sur : brut, net imposable ; un contrat attesté aux mêmes dates (AEM ou historique) porte d'autres compteurs ou un autre numéro d'objet : il est gardé, celui du bulletin n'est pas ajouté")
    );
    assert_eq!(
        p["ecarts"],
        json!([
            {"champ": "brut", "source": "dsn", "valeur": "725.00", "bulletin": "730.00", "le_bulletin_l_emporte": true},
            {"champ": "net_imposable", "source": "dsn", "valeur": "582.30", "bulletin": "600.00", "le_bulletin_l_emporte": true},
            // 50.004 est le net verse, pas le net a payer : la DSN ne declare pas celui-ci, le bulletin le donne sans ecart.
        ])
    );
    let q = &fichier(r, "bulletins/coolpaie.pdf")["pages"][1];
    assert_eq!((q["sort"].clone(), q["salarie"].clone(), q["paie"].clone(), q["contrat"].clone()), (json!("repris"), json!("<a creer>"), json!("nouvelle"), json!("nouveau")));

    // Les bulletins : combien, par gabarit, de quand a quand.
    let b = &r["bulletins"];
    assert_eq!((b["pages"].clone(), b["lus"].clone(), b["repris"].clone()), (json!(4), json!(4), json!(3)));
    assert_eq!(b["par_gabarit"], json!({"coolpaie": 3, "spaiectacle": 1}));
    assert_eq!((b["du"].clone(), b["au"].clone()), (json!("2025-03-10"), json!("2026-04-03")));

    // Les paies : celle de la DSN, completee et corrigee par le bulletin ; deux nouvelles.
    assert_eq!((b["paies_completees"].clone(), b["paies_nouvelles"].clone()), (json!(1), json!(2)));
    let paies = b["paies"].as_array().unwrap();
    let mars = paies.iter().find(|p| p["mois"] == json!("2025-03")).unwrap();
    for (k, v) in [
        ("salarie", "BRIOCHE"),
        ("contrat", "BR1.25.02"),
        ("declare_par", "DSN"),
        ("brut", "730.00"),
        ("net_imposable", "600.00"),
        ("net_a_payer", "592.78"),
        ("net_verse", "561.42"),
        ("pas", "25.04"),
        ("numero_objet", "256Z00920022"),
        ("spectacle", "Les Pupazzi"),
        ("cotisations_salariales", "112.18"),
        ("cotisations_patronales", "255.53"),
        ("cout_employeur", "1000.00"),
    ] {
        assert_eq!(mars[k], json!(v), "{} : {}", k, mars);
    }
    // Ce qui s'ecrit nomme chaque fichier par sa copie (son empreinte), jamais par son nom d'origine.
    let copie = |c: &str| fichier(r, c)["copie"].as_str().unwrap_or_else(|| panic!("{} sans copie", c)).to_string();
    let (dsn, coolpaie, spaiectacle) = (copie("DSN_Mars2025.dsn"), copie("bulletins/coolpaie.pdf"), copie("bulletins/spaiectacle.pdf"));
    assert!(dsn.ends_with(".dsn") && dsn.len() == 20, "{}", dsn);
    assert!(coolpaie.ends_with(".pdf") && coolpaie.len() == 20, "{}", coolpaie);
    assert_eq!(fichier(r, "juin.pdf").get("copie"), None);
    assert_eq!(fichier(r, "scan.pdf").get("copie"), None);
    assert_eq!(mars["fichier"], json!(dsn));
    assert_eq!(mars["dates"], json!([{"debut": "2025-03-10", "fin": "2025-03-14"}]));
    assert_eq!(mars["lignes"][0]["rubrique"], json!("cachet"));
    assert!(!mars["cotisations"].as_array().unwrap().is_empty());
    assert_eq!(mars["bulletin"], json!({"fichier": coolpaie, "page": 1, "gabarit": "coolpaie", "numero": "3/2025"}));
    let nouvelle = paies.iter().find(|p| p["contrat"] == json!("bulletin 7/2026")).unwrap();
    for (k, v) in [("mois", "2026-01"), ("salarie", "<a creer>"), ("nir", "1850799999123"), ("declare_par", "bulletin"), ("brut", "590.00"), ("verse_le", "2026-01-24")] {
        assert_eq!(nouvelle[k], json!(v), "{} : {}", k, nouvelle);
    }
    let b_paie = paies.iter().find(|p| p["contrat"] == json!(spaiectacle)).unwrap();
    for (k, v) in [("mois", "2026-04"), ("salarie", "BRIOCHE"), ("declare_par", "bulletin"), ("brut", "330.00"), ("net_a_payer", "288.33"), ("numero_objet", "186Z99999901")] {
        assert_eq!(b_paie[k], json!(v), "{} : {}", k, b_paie);
    }

    // Les contrats : un par bulletin, sauf celui que l'AEM atteste deja aux memes dates.
    assert_eq!(
        b["contrats"],
        json!([
            {"salarie": "<a creer>", "debut": "2026-01-14", "fin": "2026-01-24", "numero_objet": "999Z99999901", "heures": 20, "cachets": 2,
             "jours": 5, "brut": "590.00", "atteste_par": "bulletin", "numero_bulletin": "7/2026", "fichier": coolpaie, "page": 2},
            {"salarie": "BRIOCHE", "debut": "2026-04-02", "fin": "2026-04-03", "numero_objet": "186Z99999901", "heures": 0, "cachets": 3,
             "jours": 2, "brut": "330.00", "atteste_par": "bulletin", "fichier": spaiectacle, "page": 1},
        ])
    );
    // L'AEM, elle, passe comme avant.
    assert_eq!((r["aem"].clone(), r["contrats"].clone(), r["contrats_nouveaux"].clone()), (json!(1), json!(1), json!(1)));

    // Les fiches : le salarie nouveau par son NIR ; BRIOCHE existe, l'ecart de sa date de naissance est signale.
    let salaries = r["salaries"].as_array().unwrap();
    let neuf = salaries.iter().find(|s| s["donnees"]["nir"] == json!("1850799999123")).unwrap();
    assert_eq!(neuf["etat"], json!("a_creer"));
    for (k, v) in [
        ("nom", "TARTEMPION"),
        ("prenom", "ZOE"),
        ("nir_cle", "88"),
        ("date_naissance", "1988-11-04"),
        ("adresse", "3 IMPASSE DES ESSAIS"),
        ("code_postal", "99999"),
        ("commune", "VILLE FICTIVE"),
        ("statut", "artiste"),
        ("date_anciennete", "2026-01-14"),
    ] {
        assert_eq!(neuf["donnees"][k], json!(v), "{} : {}", k, neuf);
    }
    let brioche = salaries.iter().find(|s| s["cle"] == json!("BRIOCHE")).unwrap();
    assert_eq!(brioche["etat"], json!("existante"));
    assert_eq!(
        brioche["ecarts"]["date_naissance"],
        json!({"fiche": "1988-11-04", "bulletin": "1988-11-03", "fichier": "bulletins/coolpaie.pdf", "page": 1})
    );

    // Les spectacles : celui de CoolPaie avec son titre, celui de sPAIEctacle sans (il ne l'imprime pas).
    let spectacles = r["spectacles"].as_array().unwrap();
    let titre = |n: &str| spectacles.iter().find(|s| s["donnees"]["numero_objet"] == json!(n)).map(|s| s["donnees"]["titre"].clone());
    assert_eq!(titre("999Z99999901"), Some(json!("Les Fictifs en Tournée")));
    assert_eq!(titre("186Z99999901"), Some(json!("A COMPLETER")));
    assert!(spectacles.iter().any(|s| s["cle"] == json!("les-pupazzi") && s["etat"] == json!("existante")));

    // Rien n'est ecrit.
    assert!(!racine.join("historique").exists());
    assert_eq!(r["rien_a_faire"], json!(false));
}

#[test]
fn un_second_apercu_ne_double_rien() {
    let racine = boite_aux_lettres("second");
    let premier = route(&racine, "importation-apercu", json!({"dossier": ""}));
    assert_eq!(premier["ok"], json!(true), "{}", premier);
    // Deux appels, la meme reponse : rien n'a ete ecrit entre les deux.
    assert_eq!(route(&racine, "importation-apercu", json!({"dossier": ""})), premier);

    // Ce que l'ecriture (lot E) poserait dans l'historique : les paies et les contrats annonces.
    let b = &premier["resultat"]["bulletins"];
    std::fs::create_dir_all(racine.join("historique")).unwrap();
    std::fs::write(racine.join("historique").join("paies.json"), json!({"paies": b["paies"]}).to_string()).unwrap();
    std::fs::write(racine.join("historique").join("engagements.json"), json!({"engagements": b["contrats"]}).to_string()).unwrap();
    let r = route(&racine, "importation-apercu", json!({"dossier": ""}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let r = &r["resultat"];
    let b = &r["bulletins"];
    assert_eq!(
        (b["paies"].clone(), b["contrats"].clone(), b["paies_nouvelles"].clone(), b["paies_completees"].clone(), b["contrats_nouveaux"].clone()),
        (json!([]), json!([]), json!(0), json!(0), json!(0)),
        "{}",
        b
    );
    assert_eq!(sort(r, "bulletins/coolpaie.pdf").0, json!("deja_repris"), "{}", fichier(r, "bulletins/coolpaie.pdf"));
    assert_eq!(sort(r, "bulletins/spaiectacle.pdf"), (json!("deja_repris"), json!("paie déjà dans l'historique ; contrat déjà attesté")));
    // La DSN : le versement de BRIOCHE est dans l'historique (complete), celui de LEMERCIER pas encore.
    assert_eq!(sort(r, "DSN_Mars2025.dsn"), (json!("repris"), json!("DSN de 2025-03 : 2 versement(s), dont 1 à reprendre")));
}

#[test]
fn un_bulletin_d_une_autre_entreprise_n_est_pas_repris() {
    let racine = compagnie("autre-entreprise");
    std::fs::create_dir_all(racine.join("import")).unwrap();
    // Le Siret du PDF fictif tel quel : ce n'est pas celui de la compagnie fictive.
    pdf(vec![page_b(&b_cachets())], &racine.join("import").join("ailleurs.pdf"));
    let r = route(&racine, "importation-apercu", json!({"dossier": racine.join("import").to_str().unwrap()}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let r = &r["resultat"];
    assert_eq!(sort(r, "ailleurs.pdf"), (json!("incoherent"), json!("bulletin d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris")));
    assert_eq!((r["bulletins"]["paies"].clone(), r["bulletins"]["contrats"].clone()), (json!([]), json!([])));
    assert_eq!((r["salaries"].clone(), r["spectacles"].clone()), (json!([]), json!([])));
    assert_eq!(r["dsn"], json!(0));
}

#[test]
fn un_bulletin_sans_numero_d_objet_ne_double_pas_le_contrat_atteste() {
    let racine = compagnie("sans-objet");
    std::fs::create_dir_all(racine.join("import")).unwrap();
    // Des heures, sans numero d'objet imprime ; l'AEM du meme contrat (memes compteurs, meme brut) en portait un.
    pdf(vec![a_la_compagnie(page_a(&a_heures()))], &racine.join("import").join("mai.pdf"));
    std::fs::create_dir_all(racine.join("historique")).unwrap();
    std::fs::write(
        racine.join("historique").join("engagements.json"),
        json!({"engagements": [{
            "salarie": "BRIOCHE", "debut": "2025-05-12", "fin": "2025-05-16", "numero_objet": "256Z00920011",
            "heures": 40, "cachets": 0, "jours": 5, "brut": "640.00", "atteste_par": "AEM"
        }]})
        .to_string(),
    )
    .unwrap();
    let r = route(&racine, "importation-apercu", json!({"dossier": ""}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let p = &fichier(&r["resultat"], "mai.pdf")["pages"][0];
    assert_eq!((p["sort"].clone(), p["paie"].clone(), p["contrat"].clone()), (json!("repris"), json!("nouvelle"), json!("deja")), "{}", p);
    assert_eq!(r["resultat"]["bulletins"]["contrats"], json!([]));
    // Le meme bulletin avec d'autres heures : un ecart, pas un second contrat.
    let mut autre = a_heures();
    autre.lignes = vec![("41", "Heures", "16,00", "656,00")];
    autre.brut = "656,00";
    autre.net = "543,82"; // 656,00 - 112,18
    autre.net_a_payer = "543,82";
    autre.cumuls[0] = ["41,00", "113,00"];
    pdf(vec![a_la_compagnie(page_a(&autre))], &racine.join("import").join("mai.pdf"));
    let r = route(&racine, "importation-apercu", json!({"dossier": ""}));
    let p = &fichier(&r["resultat"], "mai.pdf")["pages"][0];
    assert_eq!(p["contrat"], json!("ecart"), "{}", p);
    assert_eq!(r["resultat"]["bulletins"]["contrats"], json!([]));
}

// ------------------------------------------------------------------ l'ecriture et l'incident

fn ok(r: Value) -> Value {
    reussi(&r).clone()
}

/// La cle de la fiche qui porte ce NIR.
fn cle_du_nir(racine: &Path, nir: &str) -> String {
    let s = lire(racine, "config/salaries.json");
    s.as_object().unwrap().iter().find(|(_, f)| f["nir"] == json!(nir)).map(|(k, _)| k.clone()).unwrap_or_else(|| panic!("aucune fiche pour {}", nir))
}

#[test]
fn une_page_qui_ne_retombe_pas_sur_ses_totaux_d_un_centime_est_reprise_le_bulletin_fait_foi() {
    let racine = compagnie("centime");
    std::fs::create_dir_all(racine.join("import")).unwrap();
    // Le net avant impot imprime a un centime de plus que brut - cotisations : comme les bulletins CoolPaie reels.
    let mut a = a_heures();
    a.net = "527,83";
    a.net_a_payer = "527,83";
    pdf(vec![a_la_compagnie(page_a(&a))], &racine.join("import").join("mai.pdf"));
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let p = &fichier(&r, "mai.pdf")["pages"][0];
    assert_eq!(p["sort"], json!("repris"), "{}", p);
    assert_eq!(p["paie"], json!("nouvelle"));
    let avertissements = p["avertissements"].as_array().unwrap();
    assert_eq!(avertissements.len(), 1, "{}", p);
    assert!(avertissements[0].as_str().unwrap().starts_with("incohérent : le net avant impôt lu (527.83)"), "{}", p);
    assert!(p["raison"].as_str().unwrap().contains("le bulletin fait foi, ses montants imprimés sont repris malgré : incohérent"), "{}", p);
    // Les montants imprimes s'ecrivent tels quels.
    assert_eq!(r["bulletins"]["paies"][0]["net_a_payer"], json!("527.83"));
    assert_eq!(r["bulletins"]["repris"], json!(1));
    assert!(r["avertissements"].as_array().unwrap().iter().any(|a| a.as_str().unwrap().starts_with("1 bulletin(s) ne retombent pas sur leurs propres totaux")), "{}", r["avertissements"]);
}

#[test]
fn l_import_du_dossier_copie_ecrit_l_historique_et_se_rejoue_sans_dommage() {
    let racine = boite_aux_lettres("ecrire");
    let r = ok(route(&racine, "importer", json!({"dossier": ""})));
    assert_eq!(r["ecrit"], json!(true));
    // Le compte rendu est celui du dossier lu, chemins d'origine compris.
    assert_eq!(sort(&r, "scan.pdf").0, json!("sans_texte"));
    assert_eq!(sort(&r, "notes.txt").0, json!("non_reconnu"));
    assert!(r["avertissements"].as_array().unwrap().contains(&json!(
        "2 fichier(s) du dossier ne sont pas repris : le compte rendu (`tri`) dit pourquoi, fichier par fichier."
    )));

    // La copie : ce qui est repris, sous le nom de sa copie, identique a l'original ; rien d'autre.
    assert_eq!(r["copie"]["copies"], json!(4), "{}", r["copie"]);
    let copies = racine.join("historique").join("bulletins");
    let mut noms: Vec<String> = std::fs::read_dir(&copies).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    noms.sort();
    let mut attendus: Vec<String> = Vec::new();
    for (chemin, origine) in [
        ("DSN_Mars2025.dsn", "DSN_Mars2025.dsn"),
        ("aem/AEM_EDIV5_1.txt", "aem/AEM_EDIV5_1.txt"),
        ("bulletins/coolpaie.pdf", "bulletins/coolpaie.pdf"),
        ("bulletins/spaiectacle.pdf", "bulletins/spaiectacle.pdf"),
    ] {
        let c = fichier(&r, chemin)["copie"].as_str().unwrap().to_string();
        assert_eq!(std::fs::read(copies.join(&c)).unwrap(), std::fs::read(racine.join("import").join(origine)).unwrap());
        attendus.push(c);
    }
    attendus.sort();
    assert_eq!(noms, attendus);
    let coolpaie = fichier(&r, "bulletins/coolpaie.pdf")["copie"].as_str().unwrap().to_string();

    // Les fiches : le salarie que seul un bulletin porte est cree.
    let neuf = cle_du_nir(&racine, "1850799999123");
    // Les contrats : celui de l'AEM, et ceux des bulletins, rattaches a la fiche qui existe maintenant.
    let h = lire(&racine, "historique/engagements.json");
    assert_eq!(h["importe_le"], json!("2026-10-03"));
    assert!(h["source"].as_str().unwrap().ends_with("historique/bulletins"), "{}", h["source"]);
    assert!(h["_note"].as_str().unwrap().contains("atteste_par bulletin"));
    let e = h["engagements"].as_array().unwrap();
    assert_eq!(e.iter().filter(|x| x["atteste_par"] == json!("AEM")).count(), 1);
    let du_bulletin: Vec<&Value> = e.iter().filter(|x| x["atteste_par"] == json!("bulletin")).collect();
    assert_eq!(du_bulletin.len(), 2, "{}", h);
    let le_sien = du_bulletin.iter().find(|x| x["debut"] == json!("2026-01-14")).unwrap();
    assert_eq!((le_sien["salarie"].clone(), le_sien["fichier"].clone(), le_sien["page"].clone()), (json!(neuf), json!(coolpaie), json!(2)));
    assert!(!e.iter().any(|x| x["salarie"] == json!("<a creer>")));

    // Les paies : la DSN completee par le bulletin, une seule ligne par paie ; les paies des bulletins seuls.
    let p = lire(&racine, "historique/paies.json");
    assert_eq!(p["importe_le"], json!("2026-10-03"));
    assert!(p["_note"].as_str().unwrap().contains("de deux sources"));
    let paies = p["paies"].as_array().unwrap();
    assert_eq!(paies.len(), 4, "{}", p);
    let mars: Vec<&Value> = paies.iter().filter(|x| x["mois"] == json!("2025-03") && x["salarie"] == json!("BRIOCHE")).collect();
    assert_eq!(mars.len(), 1, "{:?}", mars);
    assert_eq!((mars[0]["declare_par"].clone(), mars[0]["brut"].clone(), mars[0]["cotisations_salariales"].clone()), (json!("DSN"), json!("730.00"), json!("112.18")));
    assert_eq!(mars[0]["bulletin"]["fichier"], json!(coolpaie));
    let lemercier = paies.iter().find(|x| x["mois"] == json!("2025-03") && x["salarie"] != json!("BRIOCHE")).unwrap();
    assert_eq!(lemercier.get("bulletin"), None);
    let sienne = paies.iter().find(|x| x["salarie"] == json!(neuf)).unwrap();
    assert_eq!((sienne["mois"].clone(), sienne["declare_par"].clone(), sienne["contrat"].clone()), (json!("2026-01"), json!("bulletin"), json!("bulletin 7/2026")));
    assert!(sienne["cout_employeur"].as_str().is_some() && sienne["cotisations_patronales"].as_str().is_some(), "{}", sienne);
    // Rien n'est parti, rien n'est note pour l'editeur.
    assert!(!racine.join("journal").join("incidents.json").exists());

    // Rejoue : rien de nouveau, les copies sont reconnues, les fichiers ne bougent pas.
    let avant: Vec<Vec<u8>> = ["historique/paies.json", "historique/engagements.json", "config/salaries.json", "config/spectacles.json"]
        .iter()
        .map(|f| std::fs::read(racine.join(f)).unwrap())
        .collect();
    let r2 = ok(route(&racine, "importer", json!({"dossier": ""})));
    assert_eq!(r2["rien_a_faire"], json!(true), "{}", r2);
    assert_eq!((r2["copie"]["copies"].clone(), r2["copie"]["identiques"].clone()), (json!(0), json!(4)));
    let apres: Vec<Vec<u8>> = ["historique/paies.json", "historique/engagements.json", "config/salaries.json", "config/spectacles.json"]
        .iter()
        .map(|f| std::fs::read(racine.join(f)).unwrap())
        .collect();
    assert!(avant == apres, "un import rejoue a change l'historique ou les fiches");
    // Et l'apercu d'apres ne voit plus rien a faire.
    let a = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(a["rien_a_faire"], json!(true), "{}", a);

    // Le livre de paie lit les colonnes du bulletin : un mois de bulletins seuls n'a plus de totaux partiels...
    let l = ok(route(&racine, "exporter", json!({"etat": "livre-de-paie", "mois": "2026-01", "apercu": true})));
    let ligne = &l["groupes"][0]["lignes"][0];
    assert_eq!((ligne["source"].clone(), ligne["cotisations_salariales"].clone()), (json!("bulletin de paie"), sienne["cotisations_salariales"].clone()), "{}", l);
    assert_eq!(ligne["cout_employeur"], sienne["cout_employeur"]);
    assert_eq!(ligne["spectacle"], json!("Les Fictifs en Tournée"));
    assert_eq!(l["totaux"]["inconnus"], json!({}), "{}", l["totaux"]);
    // ... un mois de DSN completee en partie n'en a plus qu'une ligne : celle qu'aucun bulletin ne complete (son net a
    // payer aussi : la DSN ne le declare pas, 50.004 est le net verse).
    let l = ok(route(&racine, "exporter", json!({"etat": "livre-de-paie", "mois": "2025-03", "apercu": true})));
    assert_eq!(l["totaux"]["bulletins"], json!(2));
    assert_eq!(l["totaux"]["inconnus"], json!({"cotisations_salariales": 1, "cotisations_patronales": 1, "net_a_payer": 1, "cout_employeur": 1}), "{}", l["totaux"]);
    assert_eq!(l["totaux"]["cotisations_salariales"], json!("112.18"));

    // Le dossier du salarie : la page du bulletin repris, copiee.
    let d = ok(route(&racine, "dossier-salarie", json!({"salaries": [neuf], "mois": "2026-01", "categories": ["bulletins"], "apercu": true})));
    assert_eq!(d["pages"][&neuf]["bulletins"], json!(1), "{}", d);
}

/// Un dossier `import/` fictif : la DSN de mars 2025 et/ou le bulletin CoolPaie de la meme paie.
fn mettre(racine: &Path, dsn: bool, bulletin: bool) {
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    if dsn {
        std::fs::copy(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn"), import.join("DSN_Mars2025.dsn")).unwrap();
    }
    if bulletin {
        pdf(vec![a_mars_2025()], &import.join("mars.pdf"));
    }
}

/// Les paies de l'historique, sans le nom de fichier de la DSN (la copie de l'export et celle du dossier ne le
/// nomment pas pareil).
fn paies_sans_fichier(racine: &Path) -> Vec<Value> {
    let mut p: Vec<Value> = lire(racine, "historique/paies.json")["paies"].as_array().unwrap().clone();
    for x in p.iter_mut() {
        if x["declare_par"] == json!("DSN") {
            x.as_object_mut().unwrap().remove("fichier");
        }
    }
    p
}

#[test]
fn la_dsn_puis_les_bulletins_ou_l_inverse_donnent_le_meme_historique() {
    // La DSN, puis le bulletin.
    let a = compagnie("ordre-dsn-bulletins");
    mettre(&a, true, false);
    ok(route(&a, "importer", json!({"dossier": ""})));
    mettre(&a, false, true);
    let r = ok(route(&a, "importer", json!({"dossier": ""})));
    assert_eq!(r["bulletins"]["paies_completees"], json!(1), "{}", r["bulletins"]);
    let pa = lire(&a, "historique/paies.json")["paies"].clone();
    let lignes = pa.as_array().unwrap();
    assert_eq!(lignes.len(), 2, "{}", pa);
    assert!(!lignes.iter().any(|x| x["declare_par"] == json!("bulletin")), "un complement, pas un doublon : {}", pa);
    let brioche = lignes.iter().find(|x| x["salarie"] == json!("BRIOCHE")).unwrap();
    assert_eq!((brioche["contrat"].clone(), brioche["brut"].clone(), brioche["net_a_payer"].clone()), (json!("BR1.25.02"), json!("730.00"), json!("592.78")));

    // Le bulletin, puis la DSN (toujours dans le dossier) : la paie du bulletin seul s'efface devant celle de la DSN.
    let b = compagnie("ordre-bulletins-dsn");
    mettre(&b, false, true);
    ok(route(&b, "importer", json!({"dossier": ""})));
    let seul = lire(&b, "historique/paies.json")["paies"].clone();
    assert_eq!(seul.as_array().unwrap().len(), 1);
    assert_eq!((seul[0]["declare_par"].clone(), seul[0]["contrat"].clone()), (json!("bulletin"), json!("bulletin 3/2025")));
    mettre(&b, true, false);
    ok(route(&b, "importer", json!({"dossier": ""})));
    assert_eq!(lire(&b, "historique/paies.json")["paies"], pa);
    assert_eq!(lire(&b, "historique/engagements.json")["engagements"], lire(&a, "historique/engagements.json")["engagements"]);

    // Le bulletin, puis la DSN par l'export de CoolPaie (l'ancien chemin) : le meme rapprochement.
    let c = compagnie("ordre-bulletins-export");
    mettre(&c, false, true);
    ok(route(&c, "importer", json!({"dossier": ""})));
    let r = ok(route(&c, "importer", json!({"export": c.join("export").to_str().unwrap()})));
    assert_eq!(r["ecrit"], json!(true));
    assert_eq!(paies_sans_fichier(&c), paies_sans_fichier(&a));
    // Et la DSN de l'export, rejouee, ne defait pas ce que le bulletin a ecrit.
    let avant = std::fs::read(c.join("historique").join("paies.json")).unwrap();
    let r = ok(route(&c, "importer", json!({"export": c.join("export").to_str().unwrap()})));
    assert_eq!(r["rien_a_faire"], json!(true), "{}", r);
    assert_eq!(std::fs::read(c.join("historique").join("paies.json")).unwrap(), avant);
}

/// Les mots d'un texte, en capitales : ce que le test cherche « mot a mot ».
fn mots(t: &str) -> std::collections::HashSet<String> {
    t.split(|c: char| !c.is_alphanumeric()).filter(|m| !m.is_empty()).map(|m| m.to_uppercase()).collect()
}

#[test]
fn un_dossier_de_n_importe_quoi_propose_un_incident_caviarde_et_rien_ne_part_sans_l_envoi() {
    let racine = compagnie("incident");
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    // Le bulletin d'une autre entreprise (le Siret du PDF fictif), d'une personne inventee.
    pdf(
        vec![remplacer(remplacer(page_a(&a_repetitions()), "288117599900146", "185079999912388"), "BRIOCHE FANCHON", "TARTEMPION ZOE")],
        &import.join("ailleurs.pdf"),
    );
    // Un bulletin qu'aucun gabarit ne reconnait, ou la meme personne revient sous d'autres ecritures.
    let mut inconnu = Vec::new();
    for (y, ligne) in [
        (60.0, "FICHE DE PAIE"),
        (80.0, "Salariée : Zoé TARTEMPION"),
        (92.0, "3 impasse des Essais"),
        (104.0, "99999 Ville Fictive"),
        (116.0, "N° SS : 1 85 07 99 999 123 88"),
        (128.0, "Née le 04/11/1988"),
        (160.0, "Salaire de base 1 234,56"),
        (172.0, "Net à payer 987,65"),
    ] {
        bulletins_fictifs::t(&mut inconnu, 40.0, y, ligne);
    }
    pdf(vec![inconnu], &import.join("inconnu.pdf"));
    // Un bulletin de la compagnie dont le NIR ne se lit pas, d'une autre personne inventee.
    let mut sans = a_la_compagnie(page_a(&a_heures()));
    for (de, vers) in [
        ("288117599900146", "illisible"),
        ("BRIOCHE FANCHON", "CROUTON ALBIN"),
        ("3 IMPASSE DES ESSAIS", "8 CHEMIN DU LEURRE"),
        ("99999 VILLE FICTIVE", "99997 BOURG INVENTE"),
        ("04/11/1988", "12/02/1979"),
        ("Z0000001", "Z0000077"),
    ] {
        sans = remplacer(sans, de, vers);
    }
    pdf(vec![sans], &import.join("sans-nir.pdf"));
    std::fs::write(import.join("scan.pdf"), pdf_en_traces()).unwrap();
    std::fs::write(import.join("notes.txt"), "Appeler Zoé Tartempion au 06 12 34 56 78\n").unwrap();

    // Le compte rendu : chaque fichier a son sort, aucun n'est repris.
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(r["tri"]["total"], json!(5));
    assert_eq!(sort(&r, "ailleurs.pdf").0, json!("incoherent"));
    assert_eq!(sort(&r, "inconnu.pdf").0, json!("gabarit_inconnu"));
    assert_eq!(sort(&r, "sans-nir.pdf").0, json!("incoherent"));
    assert_eq!(sort(&r, "scan.pdf").0, json!("sans_texte"));
    assert_eq!(sort(&r, "notes.txt").0, json!("non_reconnu"));
    assert_eq!((r["bulletins"]["paies"].clone(), r["bulletins"]["contrats"].clone()), (json!([]), json!([])));

    // L'incident propose : ce qui partirait, caviarde ; rien n'est ecrit.
    let i = ok(route(&racine, "importation-incident", json!({"dossier": ""})));
    assert_eq!(i["a_envoyer"], json!(true), "{}", i);
    let chemins: Vec<&str> = i["fichiers"].as_array().unwrap().iter().map(|f| f["chemin"].as_str().unwrap()).collect();
    assert_eq!(chemins.len(), 5, "{}", i);
    for c in ["ailleurs.pdf", "inconnu.pdf", "sans-nir.pdf", "scan.pdf", "notes.txt"] {
        assert!(chemins.contains(&c), "{} : {:?}", c, chemins);
    }
    let texte = i["texte"].as_str().unwrap();
    for genre in ["fichier non repris : incoherent", "fichier non repris : gabarit_inconnu", "fichier non repris : sans_texte", "fichier non repris : non_reconnu"] {
        assert!(texte.contains(genre), "{} : {}", genre, texte);
    }
    // Les montants restent ; ce qui identifie une personne est remplace.
    assert!(texte.contains("1 234,56") && texte.contains("<NOM>") && texte.contains("<NIR>"), "{}", texte);
    let vus = mots(texte);
    for interdit in ["TARTEMPION", "ZOE", "ZOÉ", "CROUTON", "ALBIN", "IMPASSE", "LEURRE", "BOURG", "INVENTE", "FICTIVE", "99999", "99997",
        "1850799999123", "185079999912388", "Z0000001", "Z0000077", "APPELER", "AILLEURS"] {
        assert!(!vus.contains(interdit), "« {} » partirait : {}", interdit, texte);
    }
    for interdit in ["04/11/1988", "12/02/1979", "1 85 07 99 999 123 88", "sans-nir.pdf", "notes.txt", "inconnu.pdf"] {
        assert!(!texte.contains(interdit), "« {} » partirait : {}", interdit, texte);
    }
    assert!(i["resume"].as_str().unwrap().contains("enlevé"), "{}", i["resume"]);
    // Proposer n'ecrit rien : ni le journal des incidents (l'ouverture enverrait ce qui y attend), ni l'historique.
    assert!(!racine.join("journal").join("incidents.json").exists());
    assert!(!racine.join("historique").exists());
    // Ecrire l'historique n'envoie rien non plus.
    ok(route(&racine, "importer", json!({"dossier": ""})));
    assert!(!racine.join("journal").join("incidents.json").exists());

    // L'envoi, sur le clic de la personne. L'envoi a l'editeur est coupe sur ce poste d'essai : rien ne va au reseau, le
    // message est note, et la reponse le dit.
    std::fs::write(racine.join("config").join("diagnostic.json"), json!({"envoi": false}).to_string()).unwrap();
    let e = ok(route(&racine, "importation-incident-envoyer", json!({"dossier": ""})));
    assert_eq!(e["envoye"], json!(false), "{}", e);
    assert!(e["message"].as_str().unwrap().contains("coupé"), "{}", e);
    let journal = lire(&racine, "journal/incidents.json");
    let lignes = journal.as_array().unwrap();
    assert_eq!(lignes.len(), 4, "un incident par sort : {}", journal);
    for l in lignes {
        assert_eq!((l["quoi"].clone(), l["ou"].clone(), l["envoi"]["etat"].clone()), (json!("logiciel"), json!("importation"), json!("en-attente")));
        let tout = l.to_string();
        for interdit in ["TARTEMPION", "CROUTON", "IMPASSE", "185079999912388", "ailleurs.pdf"] {
            assert!(!tout.to_uppercase().contains(&interdit.to_uppercase()), "« {} » est note : {}", interdit, tout);
        }
    }
    // Ce qui est note est ce qui etait propose : le texte de la page inconnue, caviarde.
    let inconnue = lignes.iter().find(|l| l["genre"] == json!("fichier non repris : gabarit_inconnu")).unwrap();
    assert!(inconnue["trace"].as_str().unwrap().contains("1 234,56"));
    assert!(texte.contains(inconnue["trace"].as_str().unwrap()));
    // Un second clic ne note pas le message deux fois.
    ok(route(&racine, "importation-incident-envoyer", json!({"dossier": ""})));
    assert_eq!(lire(&racine, "journal/incidents.json").as_array().unwrap().len(), 4);
}

#[test]
fn un_dossier_tout_repris_ne_propose_rien() {
    let racine = compagnie("rien-a-envoyer");
    mettre(&racine, true, false);
    let i = ok(route(&racine, "importation-incident", json!({"dossier": ""})));
    assert_eq!(i, json!({"a_envoyer": false, "texte": "", "resume": "", "fichiers": []}));
    let e = ok(route(&racine, "importation-incident-envoyer", json!({"dossier": ""})));
    assert_eq!(e["envoye"], json!(false));
    assert!(!racine.join("journal").join("incidents.json").exists());
}

// ------------------------------------------------------------------------------------------- une autre entreprise (lot G1)

/// Le Siret d'une autre entreprise, inventee (celui des PDF fictifs : sa cle de Luhn tient).
const AUTRE_SIRET: &str = "99999999900017";
/// Les NIR et le numero d'objet de l'autre entreprise, inventes : s'ils paraissent quelque part, quelque chose en a ete lu.
const AUTRES_NIR: [&str; 3] = ["1790599999055", "2010199999077", "1660299999033"];
const AUTRE_OBJET: &str = "999Z88888801";

fn remplacer_octets(octets: &[u8], de: &str, vers: &str) -> Vec<u8> {
    let (de, vers) = (de.as_bytes(), vers.as_bytes());
    let mut out = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i..].starts_with(de) {
            out.extend_from_slice(vers);
            i += de.len();
        } else {
            out.push(octets[i]);
            i += 1;
        }
    }
    out
}

/// La DSN de mars 2025 de la compagnie fictive, devenue celle d'une autre entreprise (Siren, raison sociale, mois, NIR, objet).
fn dsn_d_une_autre_entreprise(siren: &str) -> Vec<u8> {
    let mut o = std::fs::read(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn")).unwrap();
    for (de, vers) in [
        ("'123456923'", format!("'{}'", siren)),
        ("CIE FICTIVE DU FIL TENDU", "AUTRE CIE INVENTEE".to_string()),
        ("01032025", "01052025".to_string()),
        ("2881175999001", AUTRES_NIR[0].to_string()),
        ("2950803999012", AUTRES_NIR[1].to_string()),
        ("256Z00920022", AUTRE_OBJET.to_string()),
    ] {
        o = remplacer_octets(&o, de, &vers);
    }
    o
}

/// Une AEM (marquee comme le tri les reconnait) d'un salarie de l'autre entreprise.
fn aem_marquee(siret: Option<&str>, nir: &str, objet: &str) -> String {
    format!("{}\r\n", marquee(aem_de(siret, nir, "20250512", "20250516", objet)))
}

/// Rien de l'autre entreprise dans ce texte (une reponse, un fichier ecrit).
fn rien_de_l_autre(texte: &str) {
    for x in AUTRES_NIR.iter().chain([&AUTRE_OBJET, &"AUTRE CIE INVENTEE", &"2025-05"]) {
        assert!(!texte.contains(x), "{} lu : {}", x, texte);
    }
}

#[test]
fn une_dsn_et_une_aem_d_une_autre_entreprise_ne_sont_pas_lues() {
    let racine = boite_aux_lettres("autre-dsn-aem");
    let ailleurs = racine.join("import").join("ailleurs");
    std::fs::create_dir_all(&ailleurs).unwrap();
    std::fs::write(ailleurs.join("autre.dsn"), dsn_d_une_autre_entreprise("999999999")).unwrap();
    std::fs::write(ailleurs.join("AEM_autre.txt"), aem_marquee(Some(AUTRE_SIRET), AUTRES_NIR[2], AUTRE_OBJET)).unwrap();

    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(
        sort(&r, "ailleurs/autre.dsn"),
        (json!("incoherent"), json!("DSN d'une autre entreprise : son Siren n'est pas celui de la compagnie, non reprise"))
    );
    assert_eq!(
        sort(&r, "ailleurs/AEM_autre.txt"),
        (json!("incoherent"), json!("AEM d'une autre entreprise : son Siret n'est pas celui de la compagnie, non reprise"))
    );
    assert_eq!(fichier(&r, "ailleurs/autre.dsn").get("copie"), None);
    assert_eq!(fichier(&r, "ailleurs/AEM_autre.txt").get("copie"), None);
    assert_eq!(r["tri"]["comptes"]["incoherent"], json!(2), "{}", r["tri"]);
    // Le reste du dossier passe comme avant.
    assert_eq!(sort(&r, "DSN_Mars2025.dsn"), (json!("repris"), json!("DSN de 2025-03 : 2 versement(s), dont 2 à reprendre")));
    assert_eq!(sort(&r, "aem/AEM_EDIV5_1.txt"), (json!("repris"), json!("AEM : 1 contrat(s) attesté(s), dont 1 nouveau(x)")));
    // Rien n'en est lu : ni fiche (salarie, spectacle, entreprise), ni contrat, ni paie, ni mois.
    assert_eq!((r["dsn"].clone(), r["aem"].clone(), r["contrats"].clone()), (json!(1), json!(1), json!(1)));
    assert_eq!((r["premier_mois"].clone(), r["dernier_mois"].clone()), (json!("2025-03"), json!("2025-03")));
    rien_de_l_autre(&r["salaries"].to_string());
    rien_de_l_autre(&r["spectacles"].to_string());
    rien_de_l_autre(&r["entreprise"].to_string());
    rien_de_l_autre(&r["bulletins"].to_string());

    // L'import n'en ecrit rien, et n'en copie rien.
    let r = ok(route(&racine, "importer", json!({"dossier": ""})));
    assert_eq!(sort(&r, "ailleurs/autre.dsn").0, json!("incoherent"));
    assert_eq!(r["copie"]["copies"], json!(4), "{}", r["copie"]);
    for rel in ["historique/paies.json", "historique/engagements.json", "config/salaries.json", "config/spectacles.json", "config/entreprise.json"] {
        rien_de_l_autre(&std::fs::read_to_string(racine.join(rel)).unwrap_or_default());
    }
    // Le contrat que l'AEM atteste, rattache a la fiche BRIOCHE, avec son attestation.
    let e = &lire(&racine, "historique/engagements.json")["engagements"][0];
    assert_eq!((e["salarie"].clone(), e["debut"].clone(), e["numero_attestation"].clone(), e["atteste_le"].clone()), (json!("BRIOCHE"), json!("2025-03-10"), json!("000123"), json!("2025-03-14")));
    for e in std::fs::read_dir(racine.join("historique").join("bulletins")).unwrap().flatten() {
        let o = std::fs::read(e.path()).unwrap();
        assert!(!o.windows(9).any(|w| w == b"999999999"), "copie de l'autre entreprise : {:?}", e.file_name());
    }
}

#[test]
fn la_fiche_vide_et_des_dsn_de_deux_entreprises_tout_est_refuse() {
    let racine = compagnie("deux-sirens");
    // La fiche Entreprise ne dit pas son Siren.
    let mut e = lire(&racine, "config/entreprise.json");
    e["siren"] = json!("");
    std::fs::write(racine.join("config").join("entreprise.json"), e.to_string()).unwrap();
    let import = racine.join("import");
    std::fs::create_dir_all(&import).unwrap();
    std::fs::copy(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn"), import.join("DSN_Mars2025.dsn")).unwrap();
    std::fs::write(import.join("autre.dsn"), dsn_d_une_autre_entreprise("999999999")).unwrap();
    std::fs::write(import.join("AEM_1.txt"), aem_marquee(Some("12345692300010"), "2881175999001", "256Z00920022")).unwrap();
    pdf(vec![a_la_compagnie(page_b(&b_cachets()))], &import.join("bulletin.pdf"));

    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let plusieurs = json!("le dossier porte plusieurs entreprises (Siren des DSN, Siret des bulletins ou des AEM), et ni la fiche Entreprise ni une seule DSN ne dit laquelle est la compagnie : non repris");
    for f in ["DSN_Mars2025.dsn", "autre.dsn", "AEM_1.txt", "bulletin.pdf"] {
        assert_eq!(sort(&r, f), (json!("incoherent"), plusieurs.clone()), "{}", f);
    }
    assert_eq!((r["dsn"].clone(), r["aem"].clone(), r["contrats"].clone()), (json!(0), json!(0), json!(0)));
    assert_eq!((r["salaries"].clone(), r["spectacles"].clone(), r["entreprise"].clone()), (json!([]), json!([]), Value::Null));
    assert_eq!((r["bulletins"]["paies"].clone(), r["bulletins"]["contrats"].clone()), (json!([]), json!([])));

    // Une seule entreprise dans les DSN : c'est elle, et ce qui vient d'elle passe.
    std::fs::remove_file(import.join("autre.dsn")).unwrap();
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    for f in ["DSN_Mars2025.dsn", "AEM_1.txt", "bulletin.pdf"] {
        assert_eq!(sort(&r, f).0, json!("repris"), "{} : {}", f, fichier(&r, f));
    }
}

#[test]
fn l_export_coolpaie_garde_le_meme_garde_fou() {
    let racine = compagnie("export-autre");
    let export = racine.join("export");
    std::fs::write(export.join("DSN").join("autre.dsn"), dsn_d_une_autre_entreprise("999999999")).unwrap();
    std::fs::write(export.join("AEM_EDI").join("AEM_EDIV5_autre.txt"), aem_marquee(Some(AUTRE_SIRET), AUTRES_NIR[2], AUTRE_OBJET)).unwrap();
    std::fs::write(
        export.join("AEM_EDI").join("AEM_EDIV5_1.txt"),
        format!("{}\r\n", aem("2881175999001", "20250310", "20250314", "256Z00920022")),
    )
    .unwrap();

    let r = ok(route(&racine, "importation-apercu", json!({})));
    let av = r["avertissements"].as_array().unwrap();
    assert!(av.contains(&json!("DSN d'une autre entreprise : son Siren n'est pas celui de la compagnie, non reprise : autre.dsn")), "{:?}", av);
    assert!(av.contains(&json!("AEM d'une autre entreprise : son Siret n'est pas celui de la compagnie, non reprise : AEM_EDIV5_autre.txt")), "{:?}", av);
    assert_eq!((r["dsn"].clone(), r["aem"].clone(), r["contrats"].clone()), (json!(1), json!(1), json!(1)));
    assert_eq!(r["dernier_mois"], json!("2025-03"));
    rien_de_l_autre(&r["salaries"].to_string());
    rien_de_l_autre(&r["spectacles"].to_string());
    rien_de_l_autre(&r["entreprise"].to_string());

    let r = ok(route(&racine, "importer", json!({})));
    assert!(r["avertissements"].as_array().unwrap().iter().any(|a| a.as_str().is_some_and(|t| t.starts_with("DSN d'une autre entreprise"))), "{}", r);
    let copie = racine.join("historique").join("coolpaie");
    assert!(copie.join("DSN").join("DSN_Mars2025.dsn").is_file());
    assert!(!copie.join("DSN").join("autre.dsn").exists());
    assert!(!copie.join("AEM_EDI").join("AEM_EDIV5_autre.txt").exists());
    for rel in ["historique/paies.json", "historique/engagements.json", "config/salaries.json", "config/spectacles.json", "config/entreprise.json"] {
        rien_de_l_autre(&std::fs::read_to_string(racine.join(rel)).unwrap_or_default());
    }
    // Le contrat que l'AEM atteste, rattache a la fiche BRIOCHE, avec son attestation.
    let e = &lire(&racine, "historique/engagements.json")["engagements"][0];
    assert_eq!((e["salarie"].clone(), e["debut"].clone(), e["numero_attestation"].clone(), e["atteste_le"].clone()), (json!("BRIOCHE"), json!("2025-03-10"), json!("000123"), json!("2025-03-14")));

    // Un export qui n'a rien de la compagnie : refuse, et rien ne s'ecrit.
    let seul = compagnie("export-seul-autre");
    std::fs::remove_file(seul.join("export").join("DSN").join("DSN_Mars2025.dsn")).unwrap();
    std::fs::write(seul.join("export").join("DSN").join("autre.dsn"), dsn_d_une_autre_entreprise("999999999")).unwrap();
    let r = route(&seul, "importer", json!({}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(r["erreur"].as_str().unwrap_or("").contains("DSN d'une autre entreprise"), "{}", r);
    assert!(!seul.join("historique").exists());
}

#[test]
fn une_aem_sans_siret_lisible_passe_avec_un_avertissement() {
    let racine = compagnie("aem-sans-siret");
    std::fs::create_dir_all(racine.join("import")).unwrap();
    std::fs::write(racine.join("import").join("AEM_1.txt"), aem_marquee(None, "2881175999001", "256Z00920022")).unwrap();
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(
        sort(&r, "AEM_1.txt"),
        (
            json!("repris"),
            json!("AEM : 1 contrat(s) attesté(s), dont 1 nouveau(x) ; son Siret ne se lit pas (caractères 51 à 64 de l'enregistrement) : l'entreprise n'est pas contrôlée")
        )
    );
    let copie = fichier(&r, "AEM_1.txt")["copie"].as_str().unwrap().to_string();
    assert!(r["avertissements"].as_array().unwrap().contains(&json!(format!(
        "AEM reprise, mais son Siret ne se lit pas (caractères 51 à 64 de l'enregistrement) : l'entreprise n'est pas contrôlée : {}",
        copie
    ))));
    assert_eq!(r["contrats"], json!(1));
}

#[test]
fn la_boite_aux_lettres_absente_se_cree_vide_un_dossier_donne_absent_est_refuse() {
    let racine = compagnie("import-absent");
    assert!(!racine.join("import").exists());
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert!(racine.join("import").is_dir());
    assert_eq!(r["tri"]["total"], json!(0), "{}", r["tri"]);
    assert_eq!(r["rien_a_faire"], json!(true), "{}", r);
    std::fs::remove_dir(racine.join("import")).unwrap();
    let r = ok(route(&racine, "importer", json!({"dossier": ""})));
    assert!(racine.join("import").is_dir());
    assert_eq!(r["rien_a_faire"], json!(true), "{}", r);
    // Un dossier donne qui n'existe pas : refuse, et pas cree.
    let ailleurs = racine.join("nulle-part");
    let r = route(&racine, "importation-apercu", json!({"dossier": ailleurs.to_str().unwrap()}));
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert!(!ailleurs.exists());
}

/// La DSN de mars 2025 de la compagnie fictive, son premier salarie devenu inconnu de la compagnie (une fiche a creer), avec
/// ces valeurs a sa place : `30.013`, `30.015`, `40.005`.
fn dsn_d_un_inconnu(nationalite: &str, pays: &str, emploi: &str) -> Vec<u8> {
    let mut o = std::fs::read(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn")).unwrap();
    for (de, vers) in [
        ("2881175999001".to_string(), AUTRES_NIR[2].to_string()),
        ("S21.G00.30.013,'01'\r\nS21.G00.30.014".to_string(), format!("S21.G00.30.013,'{}'\r\nS21.G00.30.014", nationalite)),
        ("MAR010".to_string(), emploi.to_string()),
    ] {
        o = remplacer_octets(&o, &de, &vers);
    }
    // Le premier 30.015 est celui du salarie inconnu.
    let t = String::from_utf8(o.clone()).unwrap();
    let i = t.find("S21.G00.30.015,'FR'").unwrap();
    let mut t = t;
    t.replace_range(i..i + "S21.G00.30.015,'FR'".len(), &format!("S21.G00.30.015,'{}'", pays));
    t.into_bytes()
}

/// #152 : ce que l'import relit d'une DSN se controle avant d'ecrire une fiche. Un pays de naissance hors de la liste
/// et une nationalite « 03 » sans pays sont des anomalies de la fiche a creer, avec la phrase qui dit quoi faire ;
/// `999SPT` (le complement PCS-ESE que la DSN porte pour TOUT permanent, `contrats-base.json`) n'est pas un metier :
/// il ne devient pas le `code_emploi_ft` de la fiche.
#[test]
fn ce_que_l_import_relit_de_l_individu_se_controle_et_999spt_n_est_pas_un_metier() {
    let racine = compagnie("individu-controle");
    std::fs::write(racine.join("export").join("DSN").join("DSN_Mars2025.dsn"), dsn_d_un_inconnu("03", "ZZ", "999SPT")).unwrap();
    let r = route(&racine, "importation-apercu", json!({}));
    assert_eq!(r["ok"], json!(true), "{}", r);
    let s = r["resultat"]["salaries"].as_array().unwrap().iter().find(|s| s["etat"] == json!("a_creer")).expect("une fiche a creer").clone();
    let anomalies = s["anomalies"].as_array().unwrap();
    let sur = |cle: &str| anomalies.iter().find(|a| a["champ"] == json!(cle) || a["cle"] == json!(cle)).cloned();
    let naissance = sur("pays_naissance").unwrap_or_else(|| panic!("pays de naissance hors liste non signale : {}", s));
    assert!(naissance["message"].as_str().unwrap().contains("ZZ"), "{}", naissance);
    let nationalite = sur("code_nationalite_dsn").unwrap_or_else(|| panic!("03 sans pays non signale : {}", s));
    assert!(nationalite["message"].as_str().unwrap().contains("pays"), "{}", nationalite);
    assert!(s["donnees"].get("code_emploi_ft").is_none(), "999SPT n'est pas un metier : {}", s["donnees"]);
}

#[test]
fn un_export_absent_refuse_sans_rien_ecrire_ni_nommer_de_logiciel() {
    let racine = compagnie("export-absent");
    let (_, m) = refus(&route(&racine, "importer", json!({"export": racine.join("nexiste-pas").to_str().unwrap()})));
    assert!(m.starts_with("Aucune DSN ni AEM sous") && !m.contains("CoolPaie"), "{}", m);
    assert!(!racine.join("historique").exists());
}

#[test]
fn un_historique_qui_n_est_pas_un_objet_refuse_avant_toute_ecriture() {
    let racine = compagnie("import-historique");
    std::fs::write(racine.join("config/salaries.json"), "{}").unwrap();
    std::fs::create_dir_all(racine.join("historique")).unwrap();
    std::fs::write(racine.join("historique/paies.json"), "[1]").unwrap();
    refus(&route(&racine, "importer", json!({"export": racine.join("export").to_str().unwrap()})));
    assert_eq!(std::fs::read_to_string(racine.join("config/salaries.json")).unwrap(), "{}");
    assert!(!racine.join("historique/engagements.json").exists());
}

// ------------------------------------------------------------------ ce que l'import complete (lot I2)

/// Le titre d'un spectacle de la compagnie, ecrit dans sa fiche.
fn poser_titre(racine: &Path, cle: &str, titre: &str) {
    let mut sp = lire(racine, "config/spectacles.json");
    sp[cle]["titre"] = json!(titre);
    std::fs::write(racine.join("config/spectacles.json"), sp.to_string()).unwrap();
}

fn entree<'a>(r: &'a Value, liste: &str, cle: &str) -> &'a Value {
    r[liste].as_array().unwrap().iter().find(|e| e["cle"] == json!(cle)).unwrap_or_else(|| panic!("{} absent : {}", cle, r[liste]))
}

/// Lot I2, point 2 (Leandre, 14/10/2026 : « pourquoi la fiche fait loi alors que le bulletin a les bonnes infos ? ») :
/// la fiche d'un spectacle que l'import des DSN a creee porte le titre « A COMPLETER » (la DSN ne porte que le numero
/// d'objet, S21.G00.40.050) ; le bulletin imprime le titre : il le prend, sans casse, comme pour une fiche qu'il cree.
/// Un vrai titre saisi reste, et l'ecart est seulement signale.
#[test]
fn un_titre_a_completer_prend_celui_du_bulletin_un_vrai_titre_reste() {
    let racine = compagnie("titre-a-completer");
    poser_titre(&racine, "les-pupazzi", "a completer");
    mettre(&racine, false, true);
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let s = entree(&r, "spectacles", "les-pupazzi");
    assert_eq!(s["etat"], json!("existante"), "{}", s);
    assert_eq!(s["completer"]["titre"]["valeur"], json!("Les Pupazzi"), "{}", s);
    assert_eq!((s["completer"]["titre"]["source"].clone(), s["completer"]["titre"]["fichier"].clone()), (json!("bulletin"), json!("mars.pdf")));
    assert_eq!(s["ecarts"].get("titre"), None, "un titre a completer n'est pas un ecart : {}", s);
    assert_eq!(r["rien_a_faire"], json!(false));

    let r = ok(route(&racine, "importer", json!({"dossier": ""})));
    assert_eq!(entree(&r, "spectacles", "les-pupazzi")["etat"], json!("completee"));
    let sp = lire(&racine, "config/spectacles.json");
    assert_eq!(sp["les-pupazzi"]["titre"], json!("Les Pupazzi"));
    assert_eq!(sp["les-pupazzi"]["numero_objet"], json!("256Z00920022"));
    assert_eq!(sp["les-pupazzi"].get("historique"), None, "une case vide n'avait rien declare : rien ne se date");
    // Rejoue : la fiche a son titre, rien a completer.
    let a = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(entree(&a, "spectacles", "les-pupazzi").get("completer"), None, "{}", a);

    // Un vrai titre saisi reste : l'ecart est signale, rien n'est complete ni ecrit.
    let autre = compagnie("titre-saisi");
    poser_titre(&autre, "les-pupazzi", "Pupazzi, la reprise");
    mettre(&autre, false, true);
    let r = ok(route(&autre, "importation-apercu", json!({"dossier": ""})));
    let s = entree(&r, "spectacles", "les-pupazzi");
    assert_eq!(s.get("completer"), None, "{}", s);
    assert_eq!(s["ecarts"]["titre"]["bulletin"], json!("Les Pupazzi"), "{}", s);
    ok(route(&autre, "importer", json!({"dossier": ""})));
    assert_eq!(lire(&autre, "config/spectacles.json")["les-pupazzi"]["titre"], json!("Pupazzi, la reprise"));
}

/// Des champs d'une fiche de la compagnie (`config/<fichier>`, sous `cle` ; rien : le fichier entier, l'entreprise).
fn poser(racine: &Path, fichier: &str, cle: Option<&str>, champs: Value) {
    let chemin = racine.join("config").join(fichier);
    let mut f: Value = serde_json::from_str(&std::fs::read_to_string(&chemin).unwrap()).unwrap();
    let cible = match cle {
        Some(k) => &mut f[k],
        None => &mut f,
    };
    for (k, v) in champs.as_object().unwrap() {
        cible[k] = v.clone();
    }
    std::fs::write(&chemin, f.to_string()).unwrap();
}

/// Lot I2, point 3a (Leandre, 14/10/2026 : « est-ce qu'il prend bien TOUT ce qu'il peut tirer des imports ? ») : une
/// fiche existante recoit de la DSN ce qui y est vide (avec sa forme lisible), et garde tout champ rempli -- l'ecart reste
/// signale. Puis rien n'est plus a completer.
#[test]
fn une_fiche_existante_recoit_de_la_dsn_ce_qui_y_est_vide_et_garde_ce_qui_est_rempli() {
    let racine = compagnie("completer-dsn");
    poser(&racine, "salaries.json", Some("BRIOCHE"), json!({"email": "", "lieu_naissance": "", "lieu_naissance_affichage": "", "commune": "AUBIERE"}));
    poser(&racine, "entreprise.json", None, json!({"naf": "a completer"}));
    let r = ok(route(&racine, "importation-apercu", json!({})));
    let b = entree(&r, "salaries", "BRIOCHE");
    assert_eq!(b["completer"]["email"], json!({"valeur": "fanchon.brioche@exemple.invalid", "source": "dsn", "mois": "2025-03"}), "{}", b);
    assert_eq!(b["completer"]["lieu_naissance"]["valeur"], json!("VILLENEUVE-SUR-SEINE"), "{}", b);
    assert!(b["completer"]["lieu_naissance_affichage"]["valeur"].is_string(), "la forme lisible suit : {}", b);
    assert_eq!(b["completer"].get("commune"), None, "un champ rempli ne se complete pas : {}", b);
    assert_eq!(b["ecarts"]["commune"]["dsn"], json!("CLERMONT FERRAND"), "{}", b);
    assert_eq!(b["completer"].get("taux_pas"), None, "taux_pas n'est plus lu par personne : {}", b);
    assert_eq!(r["entreprise"]["completer"]["naf"]["valeur"], json!("9001Z"), "{}", r["entreprise"]);
    assert_eq!(r["entreprise"]["ecarts"].get("naf"), None);
    assert_eq!(r["rien_a_faire"], json!(false));

    let r = ok(route(&racine, "importer", json!({})));
    assert_eq!((entree(&r, "salaries", "BRIOCHE")["etat"].clone(), r["entreprise"]["etat"].clone()), (json!("completee"), json!("completee")));
    let s = &lire(&racine, "config/salaries.json")["BRIOCHE"];
    assert_eq!((s["email"].clone(), s["lieu_naissance"].clone(), s["commune"].clone()), (json!("fanchon.brioche@exemple.invalid"), json!("VILLENEUVE-SUR-SEINE"), json!("AUBIERE")));
    assert!(s["lieu_naissance_affichage"].as_str().is_some_and(|t| !t.is_empty()), "{}", s);
    assert_eq!(lire(&racine, "config/entreprise.json")["naf"], json!("9001Z"));
    let a = ok(route(&racine, "importation-apercu", json!({})));
    assert_eq!(entree(&a, "salaries", "BRIOCHE").get("completer"), None, "{}", a);
    assert_eq!(a["rien_a_faire"], json!(true), "{}", a);
}

/// Lot I2, point 3a : les bulletins completent aussi une fiche existante -- le n° Congés Spectacles qu'elle n'a pas, le
/// NAF que la fiche Entreprise n'a pas --, la date de naissance remplie reste (l'ecart est signale).
#[test]
fn une_fiche_existante_recoit_du_bulletin_ce_qui_y_est_vide() {
    let racine = compagnie("completer-bulletin");
    poser(&racine, "entreprise.json", None, json!({"naf": ""}));
    mettre(&racine, false, true);
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let b = entree(&r, "salaries", "BRIOCHE");
    assert_eq!(b["completer"]["num_conges_spectacles"], json!({"valeur": "Z0000001", "source": "bulletin", "fichier": "mars.pdf", "page": 1}), "{}", b);
    assert_eq!(b["completer"].get("date_naissance"), None);
    assert_eq!(b["ecarts"]["date_naissance"]["bulletin"], json!("1988-11-03"));
    assert_eq!(r["entreprise"]["completer"]["naf"]["valeur"], json!("9001Z"), "{}", r["entreprise"]);
    ok(route(&racine, "importer", json!({"dossier": ""})));
    let s = &lire(&racine, "config/salaries.json")["BRIOCHE"];
    assert_eq!((s["num_conges_spectacles"].clone(), s["date_naissance"].clone()), (json!("Z0000001"), json!("1988-11-04")));
    assert_eq!(lire(&racine, "config/entreprise.json")["naf"], json!("9001Z"));
}

/// Lot I2, point 3d : une fiche que la DSN cree est completee par le bulletin de la meme personne -- le n° Congés
/// Spectacles, que la DSN ne porte pas ; ce que la DSN donne reste (sa date de naissance, quand le bulletin en imprime une
/// autre).
#[test]
fn une_fiche_que_la_dsn_cree_est_completee_par_le_bulletin() {
    let racine = compagnie("dsn-cree-bulletin-complete");
    let mut s = lire(&racine, "config/salaries.json");
    s.as_object_mut().unwrap().remove("BRIOCHE");
    std::fs::write(racine.join("config/salaries.json"), s.to_string()).unwrap();
    mettre(&racine, true, true);
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let neuves: Vec<&Value> = r["salaries"].as_array().unwrap().iter().filter(|e| e["donnees"]["nir"] == json!("2881175999001")).collect();
    assert_eq!(neuves.len(), 1, "{}", r["salaries"]);
    let d = &neuves[0]["donnees"];
    assert_eq!(neuves[0]["etat"], json!("a_creer"));
    assert_eq!((d["num_conges_spectacles"].clone(), d["date_naissance"].clone()), (json!("Z0000001"), json!("1988-11-04")), "{}", d);
    ok(route(&racine, "importer", json!({"dossier": ""})));
    let cle = cle_du_nir(&racine, "2881175999001");
    assert_eq!(lire(&racine, "config/salaries.json")[&cle]["num_conges_spectacles"], json!("Z0000001"));
}

/// La DSN de mars 2025 de la compagnie fictive, deposee par un cabinet (invente) : son Siren, son nom et son adresse dans
/// l'en-tete de l'emetteur (S10.G00.01), l'entreprise inchangee en S21.G00.06 et S21.G00.11.
fn dsn_deposee_par_un_cabinet() -> Vec<u8> {
    let mut o = std::fs::read(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn")).unwrap();
    for (de, vers) in [
        ("S10.G00.01.001,'123456923'", "S10.G00.01.001,'888888888'"),
        ("S10.G00.01.003,'CIE FICTIVE DU FIL TENDU'", "S10.G00.01.003,'CABINET INVENTE'"),
        ("S10.G00.01.004,'21 RUE DU PORT'", "S10.G00.01.004,'1 RUE DU CABINET'"),
        ("S10.G00.01.005,'63000'", "S10.G00.01.005,'99990'"),
        ("S10.G00.01.006,'CLERMONT FERRAND'", "S10.G00.01.006,'VILLE CABINET'"),
    ] {
        let avant = o.clone();
        o = remplacer_octets(&o, de, vers);
        assert_ne!(o, avant, "{} absent de la DSN de fictive", de);
    }
    o
}

/// Lot I2, point 3b : l'entreprise se lit dans S21.G00.06 et S21.G00.11, pas dans l'en-tete de l'emetteur (S10.G00.01),
/// qui est celui du cabinet quand un tiers depose (cahier technique 2026.1, p. 130). La DSN d'un cabinet est celle de la
/// compagnie ; rien du cabinet n'entre dans la fiche ; la raison sociale de l'emetteur ne se reprend que s'il est
/// l'entreprise.
#[test]
fn l_entreprise_se_lit_dans_les_blocs_06_et_11_pas_chez_l_emetteur() {
    // La fiche Entreprise existe : la DSN du cabinet n'est pas « d'une autre entreprise », et ne fait aucun ecart.
    let racine = compagnie("cabinet");
    std::fs::create_dir_all(racine.join("import")).unwrap();
    std::fs::write(racine.join("import").join("cabinet.dsn"), dsn_deposee_par_un_cabinet()).unwrap();
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(sort(&r, "cabinet.dsn").0, json!("repris"), "{}", fichier(&r, "cabinet.dsn"));
    assert!(!r["entreprise"].to_string().contains("CABINET"), "{}", r["entreprise"]);

    // Sans Siren dans la fiche : la fiche a creer est celle de l'entreprise, sans le nom ni l'adresse du cabinet.
    poser(&racine, "entreprise.json", None, json!({"siren": ""}));
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let d = &r["entreprise"]["donnees"];
    assert_eq!(r["entreprise"]["etat"], json!("a_creer"), "{}", r["entreprise"]);
    for (k, v) in [("siren", "123456923"), ("nic", "00010"), ("siret", "12345692300010"), ("naf", "9001Z"), ("adresse", "21 RUE DU PORT"), ("code_postal", "63000")] {
        assert_eq!(d[k], json!(v), "{} : {}", k, d);
    }
    assert_eq!(d.get("raison_sociale"), None, "la DSN ne declare pas la raison sociale de l'entreprise : {}", d);
    assert!(!d.to_string().contains("CABINET"), "{}", d);

    // L'entreprise qui depose elle-meme : la raison sociale de l'emetteur est la sienne.
    let seule = compagnie("emetteur-entreprise");
    poser(&seule, "entreprise.json", None, json!({"siren": ""}));
    mettre(&seule, true, false);
    let r = ok(route(&seule, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(r["entreprise"]["donnees"]["raison_sociale"], json!("CIE FICTIVE DU FIL TENDU"), "{}", r["entreprise"]);
}

/// Lot I2, point 3c : le sexe que la DSN declare (S21.G00.30.005, « 02 - féminin », cahier technique 2026.1, p. 171)
/// devient le « 2 » de la fiche : dans une fiche qu'elle cree, et dans une fiche existante ou il manque.
#[test]
fn le_sexe_que_la_dsn_declare_entre_dans_la_fiche() {
    let racine = compagnie("sexe");
    let o = std::fs::read(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn")).unwrap();
    let o = remplacer_octets(&o, "S21.G00.30.006,'04111988'", "S21.G00.30.005,'02'\r\nS21.G00.30.006,'04111988'");
    std::fs::write(racine.join("export").join("DSN").join("DSN_Mars2025.dsn"), &o).unwrap();
    let r = ok(route(&racine, "importation-apercu", json!({})));
    assert_eq!(entree(&r, "salaries", "BRIOCHE")["completer"]["sexe"]["valeur"], json!("2"), "{}", r["salaries"]);
    ok(route(&racine, "importer", json!({})));
    assert_eq!(lire(&racine, "config/salaries.json")["BRIOCHE"]["sexe"], json!("2"));

    // Une fiche que la DSN cree.
    let neuve = compagnie("sexe-neuve");
    let o = remplacer_octets(&o, "2881175999001", AUTRES_NIR[2]);
    std::fs::write(neuve.join("export").join("DSN").join("DSN_Mars2025.dsn"), &o).unwrap();
    let r = ok(route(&neuve, "importation-apercu", json!({})));
    let s = r["salaries"].as_array().unwrap().iter().find(|s| s["etat"] == json!("a_creer")).expect("une fiche a creer");
    assert_eq!(s["donnees"]["sexe"], json!("2"), "{}", s);
}

/// Lot I2, point 3e : ce que l'import lisait sans l'ecrire. De la DSN, le compte qui paie les organismes (BIC et IBAN du
/// bloc 20, cahier technique 2026.1, p. 156) et le dossier de recouvrement France Travail (20.002 du versement a France
/// Travail, p. 155) ; du bulletin, le compte Urssaf imprime apres « Urssaf : ». Dans les cases vides de la fiche.
#[test]
fn la_banque_le_dossier_france_travail_et_le_compte_urssaf_entrent_dans_la_fiche_entreprise() {
    let racine = compagnie("organismes");
    let o = std::fs::read(fictive().join("sorties").join("2025-03").join("DSN_Mars2025.dsn")).unwrap();
    let o = remplacer_octets(&o, "S21.G00.20.001,'13000548100010'", "S21.G00.20.001,'13000548100010'\r\nS21.G00.20.002,'CS00012345'");
    std::fs::write(racine.join("export").join("DSN").join("DSN_Mars2025.dsn"), &o).unwrap();
    poser(&racine, "entreprise.json", None, json!({"banque": {}, "france_travail_dossier_recouvrement": "", "urssaf_compte": ""}));
    let r = ok(route(&racine, "importation-apercu", json!({})));
    let c = &r["entreprise"]["completer"];
    assert_eq!(c["banque"]["valeur"], json!({"bic": "BANQFRPPXXX", "iban": "FR7630001007941234567890185"}), "{}", r["entreprise"]);
    assert_eq!(c["france_travail_dossier_recouvrement"]["valeur"], json!("CS00012345"), "{}", r["entreprise"]);
    ok(route(&racine, "importer", json!({})));
    let e = lire(&racine, "config/entreprise.json");
    assert_eq!((e["banque"]["iban"].clone(), e["france_travail_dossier_recouvrement"].clone()), (json!("FR7630001007941234567890185"), json!("CS00012345")));

    // Le bulletin : le compte Urssaf.
    mettre(&racine, false, true);
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    assert_eq!(r["entreprise"]["completer"]["urssaf_compte"]["valeur"], json!("999999999999"), "{}", r["entreprise"]);
    // Une banque deja la, avec sa note : pas d'ecart pour une cle que la DSN ne declare pas.
    let r = ok(route(&compagnie("banque-notee"), "importation-apercu", json!({})));
    assert_eq!(r["entreprise"]["ecarts"].get("banque"), None, "{}", r["entreprise"]);
}

/// Lot I2, point 3g : le bulletin compare a la fiche Entreprise sa raison sociale, son adresse et son IDCC, plus seulement
/// le Siret et le NAF ; la fiche fait foi, l'ecart est signale. (Le bulletin fictif imprime « Compagnie des Essais »,
/// « 12 rue Imaginaire, 99998 Nullepart » ; la compagnie fictive est « CIE FICTIVE DU FIL TENDU » a Clermont-Ferrand.)
#[test]
fn le_bulletin_compare_la_raison_sociale_et_l_adresse_de_l_entreprise() {
    let racine = compagnie("ecarts-entreprise");
    mettre(&racine, false, true);
    let r = ok(route(&racine, "importation-apercu", json!({"dossier": ""})));
    let ec = &r["entreprise"]["ecarts"];
    assert_eq!(ec["raison_sociale"]["bulletin"], json!("Compagnie des Essais"), "{}", ec);
    assert_eq!(ec["commune"]["bulletin"], json!("Nullepart"), "{}", ec);
    assert_eq!(ec["code_postal"]["fiche"], json!("63000"), "{}", ec);
    assert_eq!(ec.get("idcc"), None, "meme IDCC : {}", ec);
    ok(route(&racine, "importer", json!({"dossier": ""})));
    assert_eq!(lire(&racine, "config/entreprise.json")["raison_sociale"], json!("CIE FICTIVE DU FIL TENDU"));
}
