//! Corrections DSN, lot V4a : la version de la norme se choisit par fichier, d'apres une donnee livree
//! (`logiciel/donnees/norme-dsn-base.json`), et non d'une constante.
//!
//! La regle de bascule n'est pas dans le cahier technique 2027.1.1 (une seule valeur de S10.G00.00.006 par cahier, p. 129 ;
//! lot N9 § 4) : la donnee retient une INFERENCE (`_inference`) -- une DSN mensuelle suit son mois principal declare, un
//! signalement le jour ou il est constitue, P27V01 a partir de janvier 2027. Ces tests prouvent que le moteur applique la
//! donnee, pas que la donnee est juste.
//!
//! Ce que P27V01 change dans ce que le moteur ecrit (cahier 2027.1.1, `atelier/reference/norme-dsn/texte-2027/`) :
//! `S21.G00.06.003` (code APEN) et `S21.G00.11.002` (code APET) n'existent plus (p. 142 et 145 ; usages p. 371) ; le code
//! 105 n'est plus dans la liste de 81.001 (p. 312) ; le 146 devient « hors CET » et la CET part en 148, a l'Urssaf (fiche
//! net-entreprises 2556, lot R5b).
//!
//! Sur des copies des compagnies fictives (compagnies inventees). Les tables 2027 et le refus d'un fichier sans version connue
//! se prouvent dans `src/usages.rs` et `src/formats.rs`.

use paie::amorcage;
use paie::calcul::Paie;
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::dsn::{generer, Options};
use paie::engagement::charger;
use paie::fctu::{self, Echec};
use serde_json::{json, Value};
use crate::outils::{self, compagnie, fictive, ecrire, lire, porte};
use std::path::{Path, PathBuf};

/// Une copie de `fictive-3090-grande` dont la surcouche de bareme couvre aussi 2027 et 2028 sur le socle 2026 (le socle
/// 2027 n'est pas livre ; `contexte::bareme_de_l_annee` accepte un socle d'une autre annee). Les taux n'importent pas
/// ici : seule la version de la norme est en cause.
fn copie(test: &str) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &[], &outils::dossier_de_test("norme_par_fichier", test));
    let surcouche = lire(&racine, "config/baremes/2026.json");
    for an in [2027, 2028] {
        let mut b = surcouche.clone();
        b["annee"] = json!(an);
        b["socle"] = json!(2026);
        ecrire(&racine, &format!("config/baremes/{}.json", an), &b);
    }
    // Le minimum conventionnel de la complementaire sante monte en 2027 et 2028 (1,60 % puis 1,75 % du PMSS) : les deux
    // permanents souscrivent le niveau 3 (2,04 % sur le socle 2026), pour que la paie se calcule.
    let mut sal = lire(&racine, "config/salaries.json");
    for nom in ["NICOLET", "MONTANSIER"] {
        sal[nom]["sante_svp"]["niveau"] = json!("3");
    }
    ecrire(&racine, "config/salaries.json", &sal);
    en_naf_2025(&racine);
    racine
}

/// La fiche entreprise en NAF 2025 : un fichier P27V01 ne part qu'avec un code NAF 2025 (`naf::verifier_pour`, lot
/// R5b) ; 9020Y, l'un des deux codes vers lesquels passe le 9001Z des compagnies fictives.
fn en_naf_2025(racine: &Path) {
    let mut ent = lire(racine, "config/entreprise.json");
    ent["naf"] = json!("9020Y");
    ecrire(racine, "config/entreprise.json", &ent);
}

/// Une copie de la compagnie fictive figee `nom`, en NAF 2025 ; une par appel (les tests tournent en parallele).
fn copie_en_naf_2025(nom: &str) -> PathBuf {
    let racine = compagnie::ses_dsn(nom, &[], &outils::chemin_unique(nom));
    en_naf_2025(&racine);
    racine
}

fn saisie(ctx: &Contexte, s: Value) -> Vec<Paie> {
    let chemin = outils::chemin_unique("norme-par-fichier.json");
    std::fs::write(&chemin, s.to_string()).unwrap();
    charger(ctx, &chemin).unwrap().paies
}

/// Le mois `mois` d'un permanent non cadre (Nicolet, CDI) : ni CET ni Apec.
fn mois_de_nicolet(ctx: &Contexte, mois: &str) -> Vec<Paie> {
    saisie(
        ctx,
        json!({"mois": mois, "date_paiement": format!("{}-28", mois), "engagements": [
            {"salarie": "Nicolet", "nature": "cdi", "debut": "2024-03-01", "numero": "NI.24.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 2310.0, "heures": 151.67}]}
        ]}),
    )
}

/// Le mois `mois` d'un permanent cadre au-dessus du plafond (Montansier, CDI, 5 000 EUR) : la CET est due.
fn mois_de_montansier(ctx: &Contexte, mois: &str) -> Vec<Paie> {
    saisie(
        ctx,
        json!({"mois": mois, "date_paiement": format!("{}-28", mois), "engagements": [
            {"salarie": "Montansier", "nature": "cdi", "debut": "2023-09-04", "numero": "MO.23.01", "quotite_contrat": 151.67,
             "lignes": [{"rubrique": "mois", "quantite": 1, "taux": 5000.0, "heures": 151.67}]}
        ]}),
    )
}

fn valeur(texte: &str, rubrique: &str) -> Option<String> {
    outils::valeur(&outils::paires(texte), rubrique)
}

fn options(jour: &str) -> Options {
    Options { date_constitution: Some(Jour::lire(jour).unwrap()), ..Options::default() }
}

// ------------------------------------------------------------------ la DSN mensuelle

#[test]
fn la_dsn_de_janvier_2027_part_en_p27v01_sans_code_apen_ni_code_apet() {
    let racine = copie("janvier-2027");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-02-03").unwrap())).unwrap();
    let paies = mois_de_nicolet(&ctx, "2027-01");
    let dsn = generer(&ctx, &paies, "2027-01", &[], &[], &options("2027-02-03")).unwrap();
    assert_eq!(valeur(&dsn, "S10.G00.00.006").as_deref(), Some("P27V01"));
    assert_eq!(valeur(&dsn, "S20.G00.05.005").as_deref(), Some("01012027"));
    assert!(!porte(&dsn, "S21.G00.06.003"), "le code APEN n'existe plus en P27V01 (cahier 2027.1.1, p. 142)");
    assert!(!porte(&dsn, "S21.G00.11.002"), "le code APET n'existe plus en P27V01 (cahier 2027.1.1, p. 145)");
    assert!(porte(&dsn, "S21.G00.06.004") && porte(&dsn, "S21.G00.11.003"), "le reste des blocs 06 et 11 demeure");
    // `generer` l'a deja controle ; le controle relit la version dans le fichier et prend les tables 2027.
    paie::usages::verifier_fichier(&dsn).unwrap();
    paie::formats::verifier_fichier(&dsn).unwrap();
    // Les memes lignes, relues comme un fichier P26V01 : la table 2026 exige 06.003 et 11.002, mais ne refuse pas leur
    // absence (le tableau des usages ne controle que ce qui est ecrit) ; c'est la table 2027 qui refuse leur presence.
    let avec_apen = dsn.replacen("S21.G00.06.004,", &format!("S21.G00.06.003,'9001Z'\r\nS21.G00.06.004,"), 1);
    let e = paie::usages::verifier_fichier(&avec_apen).unwrap_err();
    assert!(e.contains("S21.G00.06.003 : inconnue du tableau") && e.contains("usages-dsn-p27v01-base.json"), "{}", e);
}

#[test]
fn la_dsn_de_decembre_2026_reste_en_p26v01_avec_code_apen_et_code_apet() {
    // Inference de la donnee : la version suit le mois principal declare, meme deposee en janvier 2027.
    let racine = copie("decembre-2026");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-01-05").unwrap())).unwrap();
    let paies = mois_de_nicolet(&ctx, "2026-12");
    let dsn = generer(&ctx, &paies, "2026-12", &[], &[], &options("2027-01-05")).unwrap();
    assert_eq!(valeur(&dsn, "S10.G00.00.006").as_deref(), Some("P26V01"));
    assert_eq!(valeur(&dsn, "S21.G00.06.003"), ctx.entreprise["naf"].as_str().map(String::from));
    assert_eq!(valeur(&dsn, "S21.G00.11.002"), ctx.entreprise["naf"].as_str().map(String::from));
}

#[test]
fn la_dsn_d_un_mois_que_la_donnee_ne_couvre_pas_refuse_avec_la_phrase() {
    let racine = copie("janvier-2028");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2028-02-03").unwrap())).unwrap();
    let paies = mois_de_nicolet(&ctx, "2028-01");
    let e = generer(&ctx, &paies, "2028-01", &[], &[], &options("2028-02-03")).unwrap_err();
    assert!(e.starts_with("DSN de 2028-01 : aucune version de la norme DSN livree avec le logiciel ne couvre ce mois"), "{}", e);
    assert!(e.contains("norme-dsn-base.json") && e.contains("P27V01 du 2027-01 au 2027-12") && e.contains("Rien n'est ecrit."), "{}", e);
}

/// Les lignes 81 du fichier : (code 81.001, type de la base 78 qui la porte, organisme 81.002, montant 81.004).
fn cotisations_81(dsn: &str) -> Vec<(String, String, String, String)> {
    let (mut out, mut base78) = (Vec::new(), String::new());
    for l in dsn.lines() {
        let Some((r, v)) = l.split_once(",'") else { continue };
        let v = v.trim_end_matches('\'').to_string();
        match r {
            "S21.G00.78.001" => base78 = v,
            "S21.G00.81.001" => out.push((v, base78.clone(), String::new(), String::new())),
            "S21.G00.81.002" => out.last_mut().unwrap().2 = v,
            "S21.G00.81.004" => out.last_mut().unwrap().3 = v,
            _ => {}
        }
    }
    out
}

#[test]
fn la_cet_d_un_cadre_part_en_146_en_p26v01() {
    let racine = copie("cet-2026");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-01-05").unwrap())).unwrap();
    // Decembre 2026 : la CET part en 146, avec la part patronale de tranche 2 (guide Urssaf du 04/06/2026, p. 38 ; fiche
    // net-entreprises 2556, l. 19 : « de maniere transitoire la CET Tranche 1 et Tranche 2 en 2026 »).
    let paies = mois_de_montansier(&ctx, "2026-12");
    let dsn = generer(&ctx, &paies, "2026-12", &[], &[], &options("2027-01-05")).unwrap();
    assert_eq!(valeur(&dsn, "S10.G00.00.006").as_deref(), Some("P26V01"));
    let cet = paies[0].lignes.iter().find(|l| l.id == "cet").map(|l| l.montant_pat).unwrap();
    assert!(!cet.is_zero(), "Montansier paie la CET");
    let c81 = cotisations_81(&dsn);
    assert!(c81.iter().any(|(c, _, _, m)| c == "146" && *m == format!("{:.2}", cet)), "{:?}", c81);
    assert!(!c81.iter().any(|(c, ..)| c == "148"), "{:?}", c81);
}

#[test]
fn la_cet_d_un_cadre_part_en_148_sous_la_base_deplafonnee_en_p27v01() {
    // Fiche net-entreprises 2556 (l. 21, 83-87, cas n° 1 de janvier 2027) : en 2027 la part patronale de la CET (T1 et T2)
    // se declare en 148, « entierement sous l'assiette deplafonnee » (03, 22 ou 23), a l'Urssaf ; le 146 est « hors CET ».
    // Tableau des combinaisons P27V01 de l'Urssaf (fiche 1265) : 148 sous 03, 22 et 23.
    let racine = copie("cet-2027");
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-02-03").unwrap())).unwrap();
    let paies = mois_de_montansier(&ctx, "2027-01");
    let dsn = generer(&ctx, &paies, "2027-01", &[], &[], &options("2027-02-03")).unwrap();
    assert_eq!(valeur(&dsn, "S10.G00.00.006").as_deref(), Some("P27V01"));
    let cet = paies[0].lignes.iter().find(|l| l.id == "cet").map(|l| l.montant_pat).unwrap();
    assert!(!cet.is_zero(), "Montansier paie la CET");
    let c81 = cotisations_81(&dsn);
    let l148: Vec<_> = c81.iter().filter(|(c, ..)| c == "148").collect();
    assert_eq!(l148.len(), 1, "{:?}", c81);
    let (_, base78, org, montant) = l148[0];
    assert!(["03", "22", "23"].contains(&base78.as_str()), "le 148 sous une base deplafonnee : {:?}", l148);
    assert_eq!(*montant, format!("{:.2}", cet));
    // L'organisme : l'Urssaf, celui de la part patronale de tranche 1 (142).
    let urssaf = c81.iter().find(|(c, ..)| c == "142").map(|x| x.2.clone()).unwrap();
    assert_eq!(*org, urssaf);
    // Le 146 ne porte plus la CET.
    assert!(!c81.iter().any(|(c, _, _, m)| c == "146" && *m == format!("{:.2}", cet)), "{:?}", c81);
    paie::usages::verifier_fichier(&dsn).unwrap();
    paie::formats::verifier_fichier(&dsn).unwrap();
}

#[test]
fn la_cet_fondue_d_un_cadre_intermittent_part_en_148_en_p27v01() {
    // Le cas de `agirc_arrco::un_cadre_intermittent_au_dela_de_la_tranche_1_declare_142_et_146` (un cachet de 250,00 :
    // CET fondue dans les deux tranches, 0,52), en janvier 2027 : la part de la CET se separe de meme, et part en 148.
    let racine = copie("cet-intermittent-2027");
    let chemin = racine.join("config/salaries.json");
    let mut sal: Value = serde_json::from_slice(&std::fs::read(&chemin).unwrap()).unwrap();
    sal["BIANCOLELLI"]["statut_categoriel_rc"] = json!("01");
    sal["BIANCOLELLI"]["statut_conventionnel"] = json!("04");
    std::fs::write(&chemin, serde_json::to_vec_pretty(&sal).unwrap()).unwrap();
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire("2027-01-28").unwrap())).unwrap();
    let paies = saisie(
        &ctx,
        json!({"mois": "2027-01", "date_paiement": "2027-01-31", "engagements": [
            {"salarie": "Biancolelli", "spectacle": "266Z00910022", "debut": "2027-01-20", "fin": "2027-01-20", "numero": "BI4.27.01",
             "date_paiement": "2027-01-20",
             "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 250.0, "lieu": "Theatre fictif", "dates": ["2027-01-20"]}]}
        ]}),
    );
    let dsn = generer(&ctx, &paies, "2027-01", &[], &[], &options("2027-02-03")).unwrap();
    assert_eq!(valeur(&dsn, "S10.G00.00.006").as_deref(), Some("P27V01"));
    let c81 = cotisations_81(&dsn);
    let l148: Vec<_> = c81.iter().filter(|(c, ..)| c == "148").collect();
    assert_eq!(l148.len(), 1, "{:?}", c81);
    assert_eq!(l148[0].1, "23", "sous la base 23 du 131 : {:?}", c81);
    assert_eq!(l148[0].3, "0.52");
    assert_eq!(c81.iter().filter(|(c, ..)| c == "146").count(), 1, "le 146 de la seule tranche 2 : {:?}", c81);
    paie::usages::verifier_fichier(&dsn).unwrap();
    paie::formats::verifier_fichier(&dsn).unwrap();
}

// ------------------------------------------------------------------ les signalements

fn releve_fctu_amorcage() -> Value {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fctu_amorcage.json");
    serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
}

/// La paie du premier cas `quoi` du releve (un CDD de juin 2026 ; un CDI pour l'amorcage), aux numeros du releve.
fn paie_du_releve(ctx: &Contexte, quoi: &str) -> (Paie, Value) {
    let v = releve_fctu_amorcage();
    let c = v[quoi].as_array().unwrap().iter().find(|c| c.get("attendu").is_some()).unwrap().clone();
    let mois = c["mois"].as_str().unwrap();
    let mut s = charger(ctx, &fictive("fictive-3090-grande").join("paies").join(format!("{}.json", mois))).unwrap();
    for (p, n) in s.paies.iter_mut().zip(c["numeros"].as_array().unwrap()) {
        p.numero_bulletin = n[0].as_str().map(str::to_string);
        p.engagement.numero = n[1].as_str().map(str::to_string);
    }
    (s.paies.remove(c["index"].as_u64().unwrap() as usize), c)
}

fn ctx_fictive() -> Contexte {
    Contexte::nouveau(&copie_en_naf_2025("fictive-3090-grande"), Some(Jour::lire("2026-10-20").unwrap())).unwrap()
}

fn fctu_constitue_le(jour: &str) -> Result<String, Echec> {
    let ctx = ctx_fictive();
    let (paie, c) = paie_du_releve(&ctx, "fctu");
    let f = &c["fin"];
    let t = |k: &str| f.get(k).and_then(Value::as_str).map(String::from);
    let fin = fctu::FinDeContrat {
        fin: t("fin").unwrap(),
        motif: t("motif").unwrap(),
        mois_rattachement: t("mois_rattachement").unwrap(),
        type_preavis: t("type_preavis").unwrap(),
        preavis_debut: t("preavis_debut"),
        preavis_fin: t("preavis_fin"),
        notification: t("notification"),
        signature_convention: t("signature_convention"),
        engagement_licenciement: t("engagement_licenciement"),
        transaction: t("transaction"),
    };
    let op = fctu::Options { envoi_test: true, date_constitution: Some(Jour::lire(jour).unwrap()), ..Default::default() };
    let suspensions = paie.engagement.suspensions.clone();
    fctu::generer(&ctx, &paie, &fin, &op, &suspensions, &[])
}

#[test]
fn un_fctu_suit_le_jour_ou_il_est_constitue() {
    let avant = fctu_constitue_le("2026-12-31").unwrap();
    assert_eq!(valeur(&avant, "S10.G00.00.006").as_deref(), Some("P26V01"));
    assert!(porte(&avant, "S21.G00.06.003") && porte(&avant, "S21.G00.11.002"));
    let apres = fctu_constitue_le("2027-01-04").unwrap();
    assert_eq!(valeur(&apres, "S10.G00.00.006").as_deref(), Some("P27V01"));
    assert!(!porte(&apres, "S21.G00.06.003") && !porte(&apres, "S21.G00.11.002"), "{}", apres);
    // Le reste est le meme fichier : seules la version, la date de constitution et les deux rubriques changent.
    let sans = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !["S10.G00.00.006", "S20.G00.05.007", "S21.G00.06.003", "S21.G00.11.002", "S90.G00.90.001"].iter().any(|r| l.starts_with(r)))
            .map(String::from)
            .collect()
    };
    assert_eq!(sans(&avant), sans(&apres));
    match fctu_constitue_le("2028-01-03") {
        Err(e) => assert!(e.message().starts_with("Signalement constitue le 03/01/2028 : aucune version de la norme DSN livree"), "{}", e.message()),
        Ok(t) => panic!("un FCTU de 2028 est parti : {}", t),
    }
}

fn amorcage_constitue_le(jour: &str) -> Result<String, Echec> {
    let ctx = ctx_fictive();
    let (paie, c) = paie_du_releve(&ctx, "amorcage");
    let op = amorcage::Options {
        motif: c["motif"].as_str().unwrap().to_string(),
        type_declaration: "01".into(),
        ordre: 1,
        remplace: None,
        envoi_test: true,
        date_constitution: Some(Jour::lire(jour).unwrap()),
    };
    amorcage::generer(&ctx, &paie, &op)
}

#[test]
fn un_amorcage_suit_le_jour_ou_il_est_constitue() {
    let avant = amorcage_constitue_le("2026-12-31").unwrap();
    assert_eq!(valeur(&avant, "S10.G00.00.006").as_deref(), Some("P26V01"));
    assert!(porte(&avant, "S21.G00.11.002"), "le code APET est obligatoire en amorcage en 2026 (usages p. 363)");
    let apres = amorcage_constitue_le("2027-01-04").unwrap();
    assert_eq!(valeur(&apres, "S10.G00.00.006").as_deref(), Some("P27V01"));
    assert!(!porte(&apres, "S21.G00.11.002"), "{}", apres);
    match amorcage_constitue_le("2028-01-03") {
        Err(e) => assert!(e.message().contains("aucune version de la norme DSN livree"), "{}", e.message()),
        Ok(t) => panic!("un amorcage de 2028 est parti : {}", t),
    }
}

#[test]
fn un_signalement_d_arret_suit_le_jour_ou_il_est_constitue() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cas: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("tests/donnees/signalement.json")).unwrap()).unwrap();
    let c = cas.iter().find(|c| c["attendu"].get("texte").is_some()).unwrap();
    let ctx = Contexte::nouveau(&copie_en_naf_2025(c["racine"].as_str().unwrap()), Some(Jour::lire(c["date"].as_str().unwrap()).unwrap())).unwrap();
    let arret = paie::dsn::Arret { v: c["arret"].clone() };
    let le = |jour: &str| {
        let mut op = paie::signalement::Options::default();
        if let Some(n) = c["options"].get("nature").and_then(Value::as_str) {
            op.nature = n.into();
        }
        op.envoi_test = true;
        op.date_constitution = Some(Jour::lire(jour).unwrap());
        paie::signalement::generer(&ctx, &arret, &op)
    };
    let avant = le("2026-12-31").unwrap();
    let apres = le("2027-01-04").unwrap();
    assert_eq!(valeur(&avant, "S10.G00.00.006").as_deref(), Some("P26V01"));
    assert_eq!(valeur(&apres, "S10.G00.00.006").as_deref(), Some("P27V01"));
}

// ------------------------------------------------------------------ le controle d'un fichier

/// Une DSN figee de 2025 (P26V01, deposee avec un 105) : relue telle quelle, elle passe ; la meme, marquee P27V01 et
/// sans les deux rubriques retirees, refuse le 105 -- c'est ce qu'une regularisation de 2027 ecrirait en annulant tel
/// quel un 105 depose (`dsn::annuler_le_declare`).
#[test]
fn un_105_dans_un_fichier_p27v01_refuse_avec_la_phrase() {
    let chemin = fictive("fictive-3090-grande").join("sorties/2025-10/DSN_Octobre2025.dsn");
    let texte: String = std::fs::read(&chemin).unwrap().iter().map(|&b| b as char).collect();
    assert!(texte.contains("S21.G00.81.001,'105'"));
    paie::usages::verifier_fichier(&texte).unwrap();
    paie::formats::verifier_fichier(&texte).unwrap();
    let p27: String = texte
        .split("\r\n")
        .filter(|l| !l.starts_with("S21.G00.06.003,") && !l.starts_with("S21.G00.11.002,"))
        .map(|l| if l.starts_with("S10.G00.00.006,") { "S10.G00.00.006,'P27V01'".to_string() } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\r\n");
    paie::usages::verifier_fichier(&p27).unwrap();
    let e = paie::formats::verifier_fichier(&p27).unwrap_err();
    assert!(e.starts_with("Valeur(s) que la norme P27V01 n'admet plus dans leur liste"), "{}", e);
    assert!(e.contains("S21.G00.81.001 = '105'") && e.contains("p. 312") && e.contains("Rien n'est ecrit."), "{}", e);
}
