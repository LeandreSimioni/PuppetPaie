//! Le signalement de fin de contrat (nature 07) et le signalement d'amorcage (nature 08) : le texte ou le refus de chaque
//! cas de `tests/donnees/fctu_amorcage.json` sur `demo-3090-grande` (les numeros de bulletin et de contrat y sont poses),
//! un instantane du moteur que le test reecrit sur demande ; et les controles propres a la nature 07 (CCH-14, CCH-15),
//! la rupture d'un CDD (CCH-11 du 62.002), l'etablissement secondaire, le 62.006, l'amorcage 03 (CCH-15).

use crate::outils::{self, compagnie};
use paie::amorcage;
use paie::calcul::Paie;
use paie::contexte::Contexte;
use paie::date::Jour;
use paie::engagement::charger;
use paie::fctu::{self, Echec};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

fn chemin_du_releve() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/fctu_amorcage.json")
}

fn releve() -> Value {
    serde_json::from_slice(&std::fs::read(chemin_du_releve()).unwrap()).unwrap()
}

fn racine(nom: &str) -> PathBuf {
    outils::demo(nom)
}

/// La paie du cas, avec les numeros que le releve lui attribue.
fn paie_du_cas(ctx: &Contexte, nom: &str, c: &Value) -> Paie {
    let mois = c["mois"].as_str().unwrap();
    let mut s = charger(ctx, &racine(nom).join("paies").join(format!("{}.json", mois))).unwrap();
    let numeros = c["numeros"].as_array().unwrap();
    assert_eq!(numeros.len(), s.paies.len(), "{}", c["titre"]);
    for (p, n) in s.paies.iter_mut().zip(numeros) {
        p.numero_bulletin = n[0].as_str().map(str::to_string);
        p.engagement.numero = n[1].as_str().map(str::to_string);
    }
    s.paies.remove(c["index"].as_u64().unwrap() as usize)
}

fn texte_de(v: &Value, cle: &str) -> Option<String> {
    v.get(cle).filter(|x| !x.is_null()).map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string()))
}

/// Le cas `c`, son attendu (`attendu`, le texte, ou `refus`, la phrase) remplace par ce que le moteur rend.
fn avec_le_resultat(c: &Value, r: Result<String, Echec>) -> Value {
    let mut m: Map<String, Value> = c.as_object().unwrap().iter().filter(|(k, _)| *k != "attendu" && *k != "refus").map(|(k, v)| (k.clone(), v.clone())).collect();
    match r {
        Ok(texte) => m.insert("attendu".into(), Value::String(texte)),
        Err(Echec::Impossible(phrase)) => m.insert("refus".into(), Value::String(phrase)),
        Err(e) => panic!("{} : {:?}", c["titre"], e),
    };
    Value::Object(m)
}

/// Rejoue chaque cas de `quoi` (`fctu` ou `amorcage`) du releve `v` et y pose le resultat ; le releve doit porter au
/// moins un texte et un refus.
fn rejouer(v: &mut Value, quoi: &str, generer: impl Fn(&Contexte, Paie, &Value, Jour) -> Result<String, Echec>) {
    let nom = v["racine"].as_str().unwrap().to_string();
    let ctx = Contexte::nouveau(&racine(&nom), Some(Jour::lire(v["date"].as_str().unwrap()).unwrap())).unwrap();
    let constitution = Jour::lire(v["constitution"].as_str().unwrap()).unwrap();
    let cas: Vec<Value> = v[quoi].as_array().unwrap().iter().map(|c| avec_le_resultat(c, generer(&ctx, paie_du_cas(&ctx, &nom, c), c, constitution))).collect();
    assert!(cas.iter().any(|c| c.get("attendu").is_some()) && cas.iter().any(|c| c.get("refus").is_some()), "{}", quoi);
    v[quoi] = Value::Array(cas);
}

#[test]
fn chaque_fin_de_contrat_et_chaque_amorcage_du_releve_ecrit_son_texte_ou_refuse() {
    let mut v = releve();
    rejouer(&mut v, "fctu", |ctx, paie, c, constitution| {
        let f = &c["fin"];
        let t = |k: &str| texte_de(f, k);
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
        let o = &c["options"];
        let op = fctu::Options {
            type_declaration: texte_de(o, "type_declaration").unwrap_or_else(|| "01".into()),
            ordre: o.get("ordre").and_then(Value::as_i64).unwrap_or(1),
            remplace: texte_de(o, "remplace"),
            envoi_test: true,
            date_constitution: Some(constitution),
        };
        let suspensions = paie.engagement.suspensions.clone();
        fctu::generer(ctx, &paie, &fin, &op, &suspensions, &[])
    });
    rejouer(&mut v, "amorcage", |ctx, paie, c, constitution| {
        let o = &c["options"];
        let op = amorcage::Options {
            motif: c["motif"].as_str().unwrap().to_string(),
            type_declaration: texte_de(o, "type_declaration").unwrap_or_else(|| "01".into()),
            ordre: o.get("ordre").and_then(Value::as_i64).unwrap_or(1),
            remplace: texte_de(o, "remplace"),
            envoi_test: true,
            date_constitution: Some(constitution),
        };
        amorcage::generer(ctx, &paie, &op)
    });
    outils::oracle("fctu_amorcage.json", "", &v);
}

#[test]
fn cch_14_la_base_31_est_interdite() {
    let faux = "S21.G00.78.001,'23'\r\nS21.G00.78.001,'31'\r\n";
    match fctu::verifier_controles_07(faux) {
        Err(Echec::Impossible(m)) => assert!(m.starts_with("CCH-14 : la valeur « 31 »"), "{}", m),
        r => panic!("une base 31 est passee : {:?}", r),
    }
}

#[test]
fn cch_15_il_faut_les_trois_blocs_51() {
    let faux = "S21.G00.51.011,'001'\nS21.G00.51.011,'002'\n";
    match fctu::verifier_controles_07(faux) {
        Err(Echec::Impossible(m)) => assert!(m.starts_with("CCH-15") && m.contains("Manque : 010."), "{}", m),
        r => panic!("un FCTU sans salaire de base est passe : {:?}", r),
    }
    let moins = "S21.G00.51.011,'001'\n";
    match fctu::verifier_controles_07(moins) {
        Err(Echec::Impossible(m)) => assert!(m.contains("Manquent : 002, 010."), "{}", m),
        r => panic!("{:?}", r),
    }
    fctu::verifier_controles_07("S21.G00.51.011,'001'\nS21.G00.51.011,'002'\nS21.G00.51.011,'010'\n").unwrap();
}

/// V1h, point 5 : la CCH-11 de S21.G00.62.002 (cahier technique DSN 2026.1, p. 272-273) n'autorise 091 et 092
/// (licenciement pour inaptitude) que pour les natures 01, 07, 08, 09, 50, 53, 54, 82, 91 ; un CDD (02) rompu pour
/// inaptitude est en 033 (p. 271). Le CDD les proposait, et le moteur les ecrivait.
#[test]
fn l_inaptitude_d_un_cdd_est_033_jamais_091_ni_092() {
    let cdd = paie::contrats::nature(Some("cdd")).unwrap();
    let codes: Vec<String> = cdd.motifs_rupture().iter().map(|m| m["code"].as_str().unwrap().to_string()).collect();
    assert!(!codes.iter().any(|c| c == "091" || c == "092") && codes.iter().any(|c| c == "033"), "{:?}", codes);
    let cdi = paie::contrats::nature(Some("cdi")).unwrap();
    assert!(cdi.motif_rupture_refuse("091", "X").is_none() && cdi.motif_rupture_refuse("092", "X").is_none());

    // Le FCTU de la demo (un CDD, AU.25.01) avec 091 : refuse ; avec 033 : ecrit.
    let v = releve();
    let nom = v["racine"].as_str().unwrap();
    let ctx = Contexte::nouveau(&racine(nom), Some(Jour::lire(v["date"].as_str().unwrap()).unwrap())).unwrap();
    let c = &v["fctu"][0];
    let paie = paie_du_cas(&ctx, nom, c);
    let op = fctu::Options { envoi_test: true, date_constitution: Some(Jour::lire("2026-10-02").unwrap()), ..Default::default() };
    for (motif, refuse) in [("091", true), ("092", true), ("033", false)] {
        let fin = fctu::FinDeContrat {
            fin: "2026-06-30".into(),
            motif: motif.into(),
            mois_rattachement: "2026-06".into(),
            type_preavis: "90".into(),
            preavis_debut: None,
            preavis_fin: None,
            notification: None,
            signature_convention: None,
            engagement_licenciement: None,
            transaction: None,
        };
        match fctu::generer(&ctx, &paie, &fin, &op, &[], &[]) {
            Err(Echec::Impossible(m)) if refuse => assert!(m.contains(motif) && m.contains("CCH-11") && m.contains("033"), "{}", m),
            Ok(t) if !refuse => assert!(t.contains(&format!("S21.G00.62.002,'{}'", motif)), "{}", t),
            autre => panic!("{} : {:?}", motif, autre),
        }
    }
}

// ------------------------------------------------------------------ vague V2a

/// La valeur de la premiere ligne `rubrique` d'un fichier DSN.
fn valeur(texte: &str, rubrique: &str) -> Option<String> {
    outils::valeur(&outils::paires(texte), rubrique)
}

/// Un etablissement secondaire, 00099, ajoute a la fiche de la demonstration.
fn avec_un_secondaire(ctx: &mut Contexte) {
    ctx.entreprise["etablissements"] = serde_json::json!([{
        "nic": "00099", "naf": ctx.entreprise["naf"].clone(), "adresse": "1 RUE DE L'ESSAI",
        "code_postal": "69001", "commune": "LYON",
        "taux_atmp": {"valeur": 1.25, "code_risque": "923AD"}
    }]);
}

/// V2a, point 4 de la relecture (fiche N6, S21.G00.11.001 des signalements) : « Dans le cadre d'un contrat de travail,
/// il s'agit de l'etablissement de rattachement administratif du salarie » (cahier technique DSN 2026.1, p. 144), et le
/// NIC est une donnee identifiante qu'un signalement porte comme la derniere mensuelle (§ 2.1.1 p. 18, § 2.5.1 p. 67).
/// La mensuelle declarait deja un salarie d'un etablissement secondaire sous le NIC de celui-ci ; le FCTU et
/// l'amorcage ecrivaient le NIC principal (et le SIRET principal en 40.019 et 05.012).
#[test]
fn le_fctu_et_l_amorcage_d_un_salarie_d_un_secondaire_portent_son_etablissement() {
    let v = releve();
    let nom = v["racine"].as_str().unwrap();
    let mut ctx = Contexte::nouveau(&racine(nom), Some(Jour::lire(v["date"].as_str().unwrap()).unwrap())).unwrap();
    avec_un_secondaire(&mut ctx);
    let siren = ctx.entreprise["siren"].as_str().unwrap().to_string();
    let principal = ctx.entreprise["nic"].as_str().unwrap().to_string();
    let constitution = Some(Jour::lire("2026-10-02").unwrap());

    // La mensuelle de juin 2026, DSN de l'etablissement 00099, pour le salarie qu'on y rattache.
    let c = &v["fctu"][0];
    let mut paie = paie_du_cas(&ctx, nom, c);
    paie.salarie["etablissement"] = Value::String("00099".into());
    // Le contrat a ete declare par le principal jusqu'en mai : la mutation se date sur la fiche (au 01/06/2026), sans
    // quoi la mensuelle refuse d'ecrire le SIRET de l'ancien etablissement sans date (41.012, vague V3c).
    let mut historique = paie.salarie["historique"].as_array().cloned().unwrap_or_default();
    historique.push(serde_json::json!({"champ": "etablissement", "valeur": "", "jusqu_au": "2026-05-31", "note": ""}));
    paie.salarie["historique"] = Value::Array(historique);
    let o = paie::dsn::Options { etablissement: Some("00099".into()), date_constitution: constitution, ..Default::default() };
    let mensuelle = paie::dsn::generer(&ctx, std::slice::from_ref(&paie), "2026-06", &[], &[], &o).unwrap();
    assert_eq!(valeur(&mensuelle, "S21.G00.11.001").as_deref(), Some("00099"));

    // Son FCTU : le meme etablissement (bloc 11, et le SIRET du contrat en 40.019) ; le siege reste le siege (06.002).
    let fin = fctu::FinDeContrat::lire(&c["fin"]).unwrap();
    let op = fctu::Options { envoi_test: true, date_constitution: constitution, ..Default::default() };
    let t = fctu::generer(&ctx, &paie, &fin, &op, &[], &[]).unwrap();
    for (rub, attendu) in [
        ("S21.G00.06.002", principal.clone()),
        ("S21.G00.11.001", "00099".to_string()),
        ("S21.G00.11.003", "1 RUE DE L'ESSAI".to_string()),
        ("S21.G00.11.004", "69001".to_string()),
        ("S21.G00.40.019", format!("{}00099", siren)),
    ] {
        assert_eq!(valeur(&t, rub), Some(attendu), "FCTU, {}", rub);
        if rub != "S21.G00.11.003" && rub != "S21.G00.11.004" {
            assert_eq!(valeur(&t, rub), valeur(&mensuelle, rub), "FCTU et mensuelle, {}", rub);
        }
    }

    // Son amorcage : le meme etablissement ; au changement de logiciel, 05.012 nomme le SIRET qui le declarait.
    for c in v["amorcage"].as_array().unwrap().iter().filter(|c| c.get("attendu").is_some()) {
        let mut paie = paie_du_cas(&ctx, nom, c);
        paie.salarie["etablissement"] = Value::String("00099".into());
        let op = amorcage::Options {
            motif: c["motif"].as_str().unwrap().to_string(),
            type_declaration: "01".into(),
            ordre: 1,
            remplace: None,
            envoi_test: true,
            date_constitution: constitution,
        };
        let t = amorcage::generer(&ctx, &paie, &op).unwrap();
        assert_eq!(valeur(&t, "S21.G00.11.001").as_deref(), Some("00099"), "{}", c["titre"]);
        assert_eq!(valeur(&t, "S21.G00.06.002"), Some(principal.clone()), "{}", c["titre"]);
        if c["motif"] == "07" {
            assert_eq!(valeur(&t, "S20.G00.05.012"), Some(format!("{}00099", siren)), "{}", c["titre"]);
        } else {
            assert_eq!(valeur(&t, "S21.G00.40.019"), Some(format!("{}00099", siren)), "{}", c["titre"]);
        }
    }

    // Un etablissement que la fiche entreprise ne porte pas : refus, rien n'est ecrit sous le principal.
    let mut perdu = paie_du_cas(&ctx, nom, c);
    perdu.salarie["etablissement"] = Value::String("00077".into());
    match fctu::generer(&ctx, &perdu, &fin, &op, &[], &[]) {
        Err(e) => assert!(e.message().contains("00077"), "{}", e.message()),
        Ok(t) => panic!("un etablissement inconnu est passe : {}", t),
    }
}

/// Une copie de `demo-3090-grande` ou le CDD AU.25.01 (fin le 30/06/2026) est rompu par l'employeur (036) avec un
/// preavis non effectue et non paye (03 ; c'etait 02 jusqu'au lot P2, qui refuse un « paye » sans indemnite dans la paie)
/// du 16 au 30 juin ; `dernier_jour_paye` : ce que la saisie en dit.
fn rupture_avec_preavis_non_effectue(test: &str, dernier_jour_paye: Option<&str>) -> Contexte {
    let racine = compagnie::ses_dsn(releve()["racine"].as_str().unwrap(), &[], &outils::dossier_de_test("fctu_amorcage", test));
    let mut saisie = outils::lire(&racine, "paies/2026-06.json");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == "AU.25.01").unwrap();
    e["motif_rupture"] = "036".into();
    e["notification_rupture"] = "2026-06-10".into();
    e["preavis"] = serde_json::json!({"type": "03", "debut": "2026-06-16", "fin": "2026-06-30"});
    if let Some(j) = dernier_jour_paye {
        e["dernier_jour_paye"] = j.into();
    }
    outils::ecrire(&racine, "paies/2026-06.json", &saisie);
    Contexte::nouveau(&racine, Some(Jour::lire("2026-10-20").unwrap())).unwrap()
}

/// La mensuelle de juin 2026 et le FCTU du CDD AU.25.01 (la fin que la route `fin-contrat-signaler` declarerait).
fn mensuelle_et_fctu(ctx: &Contexte) -> (Result<String, String>, Result<String, Echec>) {
    let s = charger(ctx, &ctx.racine.join("paies/2026-06.json")).unwrap();
    let constitution = Some(Jour::lire("2026-10-02").unwrap());
    let o = paie::dsn::Options { date_constitution: constitution, ..Default::default() };
    let mensuelle = paie::dsn::generer(ctx, &s.paies, "2026-06", &[], &[], &o);
    let paie = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("AU.25.01")).unwrap();
    // `dernier_jour_paye` y est encore pour l'ancienne `FinDeContrat` (la route y mettait la fin a defaut) ; la
    // nouvelle l'ignore et le lit sur le contrat.
    let fin = fctu::FinDeContrat::lire(&serde_json::json!({
        "fin": "2026-06-30", "motif": "036", "dernier_jour_paye": "2026-06-30", "mois_rattachement": "2026-06",
        "type_preavis": "03", "preavis_debut": "2026-06-16", "preavis_fin": "2026-06-30", "notification": "2026-06-10",
    }))
    .unwrap();
    let op = fctu::Options { envoi_test: true, date_constitution: constitution, ..Default::default() };
    (mensuelle, fctu::generer(ctx, paie, &fin, &op, &[], &[]))
}

/// V2a, point 6 de la relecture (fiche N5, S21.G00.62.006) : « Dernier jour travaille paye selon le salaire defini par
/// le contrat de travail ou la convention collective » ; sur un preavis non effectue, il precede le debut du preavis
/// (CCH-12, cahier technique DSN 2026.1, p. 275). La mensuelle ecrivait la fin du contrat, le FCTU `dernier_jour_paye` :
/// sur un preavis non effectue ils se contredisaient, alors que la mensuelle reporte le signalement « a l'identique »
/// (§ 2.5.2, p. 68). Les deux le lisent maintenant au meme endroit (`fctu::dernier_jour_paye`).
#[test]
fn la_mensuelle_et_le_fctu_du_meme_contrat_portent_le_meme_dernier_jour_paye() {
    let ctx = rupture_avec_preavis_non_effectue("62-006-saisi", Some("2026-06-15"));
    let (mensuelle, fctu) = mensuelle_et_fctu(&ctx);
    let (mensuelle, fctu) = (mensuelle.unwrap(), fctu.unwrap());
    assert_eq!(valeur(&mensuelle, "S21.G00.62.001").as_deref(), Some("30062026"));
    assert_eq!(valeur(&mensuelle, "S21.G00.62.006").as_deref(), Some("15062026"), "la mensuelle");
    assert_eq!(valeur(&fctu, "S21.G00.62.006").as_deref(), Some("15062026"), "le FCTU");

    // Sans le jour saisi, la fin du contrat (30/06) serait fausse (CCH-12 : avant le 16/06) : les deux refusent.
    let ctx = rupture_avec_preavis_non_effectue("62-006-absent", None);
    let (mensuelle, fctu) = mensuelle_et_fctu(&ctx);
    match mensuelle {
        Err(m) => assert!(m.contains("dernier_jour_paye") && m.contains("CCH-12"), "{}", m),
        Ok(t) => panic!("la mensuelle a ecrit 62.006 = {:?}", valeur(&t, "S21.G00.62.006")),
    }
    match fctu {
        Err(Echec::Impossible(m)) => assert!(m.contains("dernier_jour_paye") && m.contains("CCH-12"), "{}", m),
        r => panic!("le FCTU : {:?}", r.map(|t| valeur(&t, "S21.G00.62.006"))),
    }
}

/// Reste R1 #42 (fiche N5, S21.G00.60.004) : en nature 07, le tableau des usages du cahier technique 2026.1
/// (`prive/reference/norme-dsn/texte/tableaux-des-usages.txt`, p. 363-382, bloc S21.G00.60) dit 60.001 a 60.003 « O »,
/// 60.010 et 60.011 « C », et 60.004 a 60.008 et 60.012 « I ». Le FCTU appelait `bloc_60` avec la nature « 01 » en dur :
/// un arret subroge y ecrivait la subrogation, interdite en 07, et le controle des usages arretait le fichier.
#[test]
fn le_fctu_d_un_salarie_en_arret_subroge_n_ecrit_que_ce_que_la_nature_07_admet() {
    let ctx = rupture_avec_preavis_non_effectue("60-subroge", Some("2026-06-15"));
    let s = charger(&ctx, &ctx.racine.join("paies/2026-06.json")).unwrap();
    let paie = s.paies.iter().find(|p| p.engagement.numero.as_deref() == Some("AU.25.01")).unwrap();
    let fin = fctu::FinDeContrat::lire(&serde_json::json!({
        "fin": "2026-06-30", "motif": "036", "mois_rattachement": "2026-06",
        "type_preavis": "03", "preavis_debut": "2026-06-16", "preavis_fin": "2026-06-30", "notification": "2026-06-10",
    }))
    .unwrap();
    let arret = paie::dsn::Arret {
        v: serde_json::json!({
            "motif": "04", "dernier_jour_travaille": "2026-06-03", "fin_previsionnelle": "2026-06-12",
            "subrogation": "01", "subrogation_debut": "2026-06-04", "subrogation_fin": "2026-06-12",
            "date_accident": "2026-06-03", "reprise": "2026-06-13", "reprise_motif": "01",
        }),
    };
    let op = fctu::Options { envoi_test: true, date_constitution: Some(Jour::lire("2026-10-02").unwrap()), ..Default::default() };
    let texte = fctu::generer(&ctx, paie, &fin, &op, &[], &[arret]).unwrap_or_else(|e| panic!("{}", e.message()));
    assert_eq!(valeur(&texte, "S21.G00.60.001").as_deref(), Some("04"));
    assert_eq!(valeur(&texte, "S21.G00.60.002").as_deref(), Some("03062026"));
    assert_eq!(valeur(&texte, "S21.G00.60.003").as_deref(), Some("12062026"));
    assert_eq!(valeur(&texte, "S21.G00.60.010").as_deref(), Some("13062026"));
    assert_eq!(valeur(&texte, "S21.G00.60.011").as_deref(), Some("01"));
    for interdite in ["004", "005", "006", "007", "008", "012"] {
        assert_eq!(valeur(&texte, &format!("S21.G00.60.{}", interdite)), None, "60.{} est interdite en nature 07", interdite);
    }
}

/// Reste R1 #52 (fiche N6, S20.G00.05.002 de l'amorcage) : « CCH-15 : Le type de declaration "03 - declaration annule et
/// remplace integral" est interdit pour une nature de declaration (S20.G00.05.001) "08 - Signalement amorcage des donnees
/// variables". » (cahier technique 2026.1, p. 135, `prive/reference/norme-dsn/texte/bloc-S20.G00.05.txt` l. 76-77). Un
/// amorcage se corrige en l'annulant (04), puis en en deposant un nouveau (§ 1.4.3.4, p. 15). Le moteur offrait et
/// ecrivait le 03 ; le point de depot l'aurait rejete.
#[test]
fn un_amorcage_annule_et_remplace_est_refuse_cch_15() {
    let v = releve();
    let nom = v["racine"].as_str().unwrap();
    let ctx = Contexte::nouveau(&racine(nom), Some(Jour::lire(v["date"].as_str().unwrap()).unwrap())).unwrap();
    let c = v["amorcage"].as_array().unwrap().iter().find(|c| c.get("attendu").is_some()).unwrap();
    let paie = paie_du_cas(&ctx, nom, c);
    let op = amorcage::Options {
        motif: c["motif"].as_str().unwrap().to_string(),
        type_declaration: "03".into(),
        ordre: 2,
        remplace: Some("0207202612345".into()),
        envoi_test: true,
        date_constitution: Some(Jour::lire(v["constitution"].as_str().unwrap()).unwrap()),
    };
    match amorcage::generer(&ctx, &paie, &op) {
        Err(e) => {
            assert!(matches!(e, Echec::Impossible(_)), "{:?}", e);
            assert!(e.message().contains("CCH-15"), "{}", e.message());
        }
        Ok(t) => panic!("un amorcage 03 est passe : {}", valeur(&t, "S20.G00.05.002").unwrap_or_default()),
    }
}
