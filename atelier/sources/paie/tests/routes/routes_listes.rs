//! `paie route` : les listes que la page lit en s'ouvrant (`arrets`, `fins-de-contrat`, `regularisations`). Sur
//! `fictive-3090-petite` (config, aout et septembre 2026, journal vide), que chaque test mute ; un fichier abime refuse.

use crate::outils::{compagnie, dossier_de_test, ecrire, lire, reussi, route};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn compagnie(nom: &str) -> PathBuf {
    compagnie::saisies("fictive-3090-petite", &["2026-08", "2026-09"], &dossier_de_test("routes_listes", nom).join("compagnie"))
}

fn liste(c: &Path, nom: &str, charge: Value, jour: &str) -> Value {
    reussi(&route(c, nom, charge, jour)).clone()
}

/// La phrase d'un refus, le 03/10/2026.
fn refus(c: &Path, nom: &str, charge: Value) -> String {
    crate::outils::refus(&route(c, nom, charge, "2026-10-03")).1
}

fn ecrire_texte(c: &Path, rel: &str, t: &str) {
    let p = c.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, t).unwrap();
}

fn un_arret(motif: &str, envois: Value) -> Value {
    json!({
        "genre": "arret", "salarie": "BRIOCHE", "contrat_debut": "2026-08-19", "contrat_numero": "BR1.26.04", "motif": motif,
        "dernier_jour_travaille": "2026-08-20", "fin_previsionnelle": "2026-08-30", "envois": envois,
    })
}

// ---------------------------------------------------------------------- arrets

#[test]
fn sans_dossier_d_arrets_la_liste_est_vide_mais_les_tables_sont_la() {
    let c = &compagnie("arrets-vide");
    let r = liste(c, "arrets", json!({}), "2026-10-03");
    assert_eq!(r["arrets"], json!([]));
    assert_eq!(r["a_signaler"], json!([]));
    assert!(!r["motifs"].as_array().unwrap().is_empty(), "les motifs saisissables viennent de arrets-base.json");
    assert!(r["motifs"].as_array().unwrap().iter().all(|m| m["saisissable"] == json!(true)));
    assert!(!r["subrogation"].as_array().unwrap().is_empty());
    assert!(r["motifs_reprise"].is_array());
}

#[test]
fn un_arret_se_resume_signale_ou_non() {
    let c = &compagnie("arrets-resume");
    let code = liste(c, "arrets", json!({}), "2026-10-03")["motifs"][0]["code"].as_str().unwrap().to_string();
    let signale = json!([{"nature": "04", "type": "01", "ordre": 1}]);
    let reporte = json!([{"nature": "01", "mois": "2026-09"}]);
    ecrire(c, 
        "imprevus/2026-08.json",
        &json!({"mois": "2026-08", "imprevus": [un_arret(&code, json!([])), un_arret(&code, signale.clone()), un_arret("ZZ", reporte.clone())]}),
    );
    let r = liste(c, "arrets", json!({}), "2026-10-03");
    let tous = r["arrets"].as_array().unwrap();
    assert_eq!(tous.len(), 3);
    assert_eq!(tous[0]["etat"], json!("a signaler"));
    assert_eq!(tous[0]["signale"], json!(false));
    assert_ne!(tous[0]["motif_libelle"], json!("?"));
    assert_eq!(tous[1]["etat"], json!("signale"));
    assert_eq!(tous[1]["signalements"], signale);
    assert_eq!(tous[2]["motif_libelle"], json!("?"), "un motif inconnu de la norme");
    assert_eq!(tous[2]["reporte_par"], json!(["2026-09"]));
    assert_eq!(tous[2]["etat"], json!("a signaler"), "un report de DSN n'est pas un signalement");
    assert_eq!(tous[2]["note"], json!(""));
    assert_eq!(tous[2]["contrat"], json!("BR1.26.04"));
    // `a_signaler` : ceux qu'aucun signalement n'a annonces.
    assert_eq!(r["a_signaler"].as_array().unwrap().len(), 2);
}

#[test]
fn un_fichier_d_arrets_abime_refuse() {
    let c = &compagnie("arrets-abime");
    let mut inconnu = un_arret("01", json!([]));
    inconnu["zzz"] = json!(1);
    let envoi_texte = un_arret("01", json!([{"nature": "01"}, "oops"]));
    for f in [json!({"imprevus": [{"salarie": "BRIOCHE"}]}), json!({"imprevus": [inconnu]}), json!({"imprevus": ["x"]}), json!([1]), json!({"imprevus": [envoi_texte]})] {
        ecrire(c, "imprevus/2026-08.json", &f);
        refus(c, "arrets", json!({}));
    }
    ecrire_texte(c, "imprevus/2026-08.json", "{");
    refus(c, "arrets", json!({}));
}

#[test]
fn seuls_les_fichiers_nommes_comme_un_mois_sont_lus() {
    let c = &compagnie("arrets-noms");
    ecrire(c, "imprevus/2026-08.json", &json!({"imprevus": [un_arret("01", json!([]))]}));
    ecrire(c, "imprevus/2026-08.calcul.json", &json!({"imprevus": [{"x": 1}]}));
    ecrire(c, "imprevus/notes.json", &json!("rien"));
    let r = liste(c, "arrets", json!({}), "2026-10-03");
    assert_eq!(r["arrets"].as_array().unwrap().len(), 1);
}

// ------------------------------------------------------------ fins-de-contrat

/// Un contrat a duree determinee ordinaire qui finit en aout 2026.
fn un_cdd(motif: Option<&str>, preavis: Option<Value>) -> Value {
    let mut e = json!({"salarie": "LEMERCIER", "nature": "cdd", "debut": "2026-03-02", "fin": "2026-08-20", "numero": "LE1.26.01"});
    if let Some(m) = motif {
        e["motif_rupture"] = json!(m);
    }
    if let Some(p) = preavis {
        e["preavis"] = p;
    }
    e
}

fn avec_engagements(c: &Path, mois: &str, extra: Vec<Value>) {
    let mut saisie = lire(c, &format!("paies/{}.json", mois));
    let engagements = saisie["engagements"].as_array_mut().unwrap();
    engagements.extend(extra);
    ecrire(c, &format!("paies/{}.json", mois), &saisie);
}

#[test]
fn un_cdd_qui_finit_dans_le_mois_dit_ce_que_la_saisie_ne_dit_pas() {
    let c = &compagnie("fins-manque");
    avec_engagements(c, "2026-08", vec![un_cdd(None, None)]);
    let r = liste(c, "fins-de-contrat", json!({"mois": "2026-08"}), "2026-10-03");
    assert_eq!(r["mois"], json!("2026-08"));
    let fins = r["fins"].as_array().unwrap();
    assert_eq!(fins.len(), 1, "les cachets de la compagnie fictive (CDDU) ne se signalent pas");
    let f = &fins[0];
    assert_eq!(f["salarie"], json!("LEMERCIER"));
    assert_eq!(f["numero"], json!("LE1.26.01"));
    assert_eq!(f["fin"], json!("2026-08-20"));
    assert_eq!(f["motif_rupture"], Value::Null);
    let manque = f["a_saisir"].as_array().unwrap();
    assert_eq!(manque.len(), 2);
    assert!(manque[0].as_str().unwrap().starts_with("`motif_rupture` : le motif de la rupture (S21.G00.62.002)"));
    let preavis = manque[1].as_str().unwrap();
    assert!(preavis.starts_with("`preavis` : {\"type\": \"...\""), "{}", preavis);
    assert!(preavis.contains("Le bloc S21.G00.63 est OBLIGATOIRE"), "{}", preavis);
}

#[test]
fn une_fin_complete_n_a_rien_a_saisir_et_un_autre_mois_est_vide() {
    let c = &compagnie("fins-complet");
    let preavis = json!({"type": "90"});
    avec_engagements(c, "2026-08", vec![un_cdd(Some("031"), Some(preavis.clone()))]);
    let r = liste(c, "fins-de-contrat", json!({"mois": "2026-08"}), "2026-10-03");
    assert_eq!(r["fins"][0]["a_saisir"], json!([]));
    assert_eq!(r["fins"][0]["preavis"], preavis);
    // Le meme contrat ne finit pas en septembre ; et un mois sans saisie n'a rien.
    assert_eq!(liste(c, "fins-de-contrat", json!({"mois": "2026-09"}), "2026-10-03")["fins"], json!([]));
    assert_eq!(liste(c, "fins-de-contrat", json!({"mois": "2031-01"}), "2026-10-03")["fins"], json!([]));
}

#[test]
fn la_loi_donne_deux_mois_de_preavis_a_un_licencie_de_plus_de_deux_ans() {
    // L1234-1, 3° (`code-du-travail/articles-20261013.md`) : au moins deux ans d'anciennete, un preavis de deux mois.
    let c = &compagnie("fins-preavis");
    let mut e = un_cdd(Some("014"), None);
    e["nature"] = json!("cdi");
    e["debut"] = json!("2019-03-04");
    avec_engagements(c, "2026-08", vec![e]);
    let r = liste(c, "fins-de-contrat", json!({"mois": "2026-08"}), "2026-10-03");
    // Le preavis, et la date de notification que la CCH-12 de S21.G00.62.003 exige pour un licenciement (014).
    let manque = r["fins"][0]["a_saisir"].as_array().unwrap();
    assert_eq!(manque.len(), 2, "{:?}", manque);
    assert!(manque[0].as_str().unwrap().starts_with("`notification_rupture`"), "{}", manque[0]);
    assert!(manque[1].as_str().unwrap().contains("La loi en donne 2 mois (L1234-1)."), "{}", manque[1]);
}

#[test]
fn un_cdd_arrive_a_son_terme_n_a_aucun_preavis() {
    // L1243-5 (`code-du-travail/articles-20261013.md`) : le CDD « cesse de plein droit a l'echeance du terme » ; L1234-1 est
    // le preavis du licenciement. Meme a plus de deux ans d'anciennete, le moteur n'en donne pas.
    let c = &compagnie("fins-terme");
    let mut e = un_cdd(Some("031"), None);
    e["debut"] = json!("2019-03-04");
    avec_engagements(c, "2026-08", vec![e]);
    let r = liste(c, "fins-de-contrat", json!({"mois": "2026-08"}), "2026-10-03");
    let manque = r["fins"][0]["a_saisir"].as_array().unwrap();
    assert_eq!(manque.len(), 1);
    let preavis = manque[0].as_str().unwrap();
    assert!(preavis.contains("La loi ne donne aucun preavis pour ce motif") && preavis.contains("L1243-5"), "{}", preavis);
    assert!(!preavis.contains("La loi en donne") && !preavis.contains("c'est elle qui s'applique"), "{}", preavis);
    assert!(preavis.contains("<< 90 - pas de clause de preavis applicable >>"), "{}", preavis);
}

/// Ce que l'ecran des fins de contrat dit du preavis d'un CDD de plus de deux ans (L1234-1 en donnerait deux mois a un
/// licenciement) rompu pour `motif` (la date de notification, que la CCH-12 de S21.G00.62.003 exige pour certains motifs,
/// est demandee a cote).
fn preavis_d_un_cdd_rompu(nom: &str, motif: &str) -> String {
    let c = &compagnie(nom);
    let mut e = un_cdd(Some(motif), None);
    e["debut"] = json!("2019-03-04");
    avec_engagements(c, "2026-08", vec![e]);
    let r = liste(c, "fins-de-contrat", json!({"mois": "2026-08"}), "2026-10-03");
    let manque: Vec<&str> = r["fins"][0]["a_saisir"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    let preavis: Vec<&str> = manque.iter().copied().filter(|m| m.starts_with("`preavis`")).collect();
    assert_eq!(preavis.len(), 1, "{:?}", manque);
    assert!(manque.iter().all(|m| m.starts_with("`preavis`") || m.starts_with("`notification_rupture`")), "{:?}", manque);
    preavis[0].to_string()
}

#[test]
fn un_cdd_rompu_avant_son_terme_par_l_employeur_par_accord_pour_faute_force_majeure_ou_inaptitude_n_a_aucun_preavis() {
    // L1243-1 (`code-du-travail/articles-20261013.md`) : ces ruptures avant le terme, sans preavis ; L1243-4 : hors de ses
    // cas, des dommages et interets (036) ; L1226-4-3 : une indemnite pour l'inaptitude (033). L1234-1 est le preavis du
    // licenciement : ni ses deux mois, ni la convention.
    for (motif, article) in [("036", "L1243-4"), ("084", "L1243-1"), ("095", "L1243-1"), ("096", "L1243-1"), ("083", "L1243-4"), ("033", "L1226-4-3")] {
        let p = preavis_d_un_cdd_rompu(&format!("fins-cdd-{}", motif), motif);
        assert!(p.contains("La loi ne donne aucun preavis pour ce motif") && p.contains(article), "{} : {}", motif, p);
        assert!(!p.contains("La loi en donne") && !p.contains("c'est elle qui s'applique"), "{} : {}", motif, p);
    }
}

#[test]
fn un_cdd_rompu_par_le_salarie_dit_le_preavis_de_l1243_2_sans_le_compter() {
    // L1243-2 : un jour par semaine, deux semaines au plus, s'il justifie d'un CDI ; le moteur ne sait ni le CDI ni le terme
    // d'origine : il dit la regle, sans duree ni convention du licenciement.
    let p = preavis_d_un_cdd_rompu("fins-cdd-037", "037");
    assert!(p.contains("La loi en donne un que le moteur ne compte pas") && p.contains("L1243-2"), "{}", p);
    assert!(p.contains("un jour par semaine") && p.contains("deux semaines"), "{}", p);
    assert!(!p.contains("(L1234-1)") && !p.contains("c'est elle qui s'applique"), "{}", p);
}

#[test]
fn une_nature_inconnue_ou_un_mois_abime_refuse_les_fins_de_contrat() {
    let c = &compagnie("fins-refus");
    refus(c, "fins-de-contrat", json!({}));
    let mut e = un_cdd(Some("031"), Some(json!({"type": "90"})));
    e["nature"] = json!("bidon");
    avec_engagements(c, "2026-08", vec![e]);
    let message = refus(c, "fins-de-contrat", json!({"mois": "2026-08"}));
    assert!(message.starts_with("Nature de contrat inconnue : 'bidon'. Connues : "), "{}", message);
    ecrire_texte(c, "paies/2026-08.json", "{");
    refus(c, "fins-de-contrat", json!({"mois": "2026-08"}));
    ecrire(c, "paies/2026-08.json", &json!([]));
    refus(c, "fins-de-contrat", json!({"mois": "2026-08"}));
}

// ------------------------------------------------------------- regularisations

fn depot_dsn(mois: &str, retours: Value) -> Value {
    json!({
        "quoi": "dsn", "quand": "2026-09-12T10:00:00", "origine": "", "mois": mois, "fichier": format!("DSN_{}.dsn", mois),
        "empreinte": "abc", "mode": "reel", "qui": "test", "accepte": true, "destinataire": "net-entreprises", "flux": "F", "retours": retours,
    })
}

#[test]
fn les_mois_se_classent_selon_leur_depot_et_leur_echeance() {
    let c = &compagnie("regul-mois");
    // Aout est parti et accepte ; septembre n'est pas parti.
    ecrire(c, "journal/depots.json", &json!([depot_dsn("2026-08", json!([{"nature": "11", "verdict": "OK"}]))]));
    let r = liste(c, "regularisations", json!({}), "2026-10-03");
    let reg = r["mois"]["regularisables"].as_array().unwrap();
    assert_eq!(reg.len(), 1);
    assert_eq!(reg[0]["mois"], json!("2026-08"));
    assert_eq!(reg[0]["deposee"], json!(true));
    assert_eq!(reg[0]["passee"], json!(true));
    assert_eq!(reg[0]["echeance"], json!("2026-09-15"));
    assert_eq!(reg[0]["mois_courant"], json!("2026-10"));
    assert_eq!(reg[0]["regularisable"], json!(true));
    assert!(reg[0]["raison"].as_str().unwrap().contains("14/09/2026 a minuit"), "{}", reg[0]["raison"]);
    // Septembre : echeance le 15 octobre, pas passee, rien n'est parti.
    assert_eq!(r["mois"]["en_retard"], json!([]));
    assert_eq!(r["mot_de_passe_pose"], json!(false));
    assert_eq!(r["corrections"], json!([]));
    // Quelques semaines plus tard, septembre est en retard.
    let tard = liste(c, "regularisations", json!({}), "2026-10-20");
    let retard = tard["mois"]["en_retard"].as_array().unwrap();
    assert_eq!(retard.len(), 1);
    assert_eq!(retard[0]["mois"], json!("2026-09"));
    assert!(retard[0]["raison"].as_str().unwrap().contains("(en retard : `paie mois paies/2026-09.json --forcer`)"));
    // Avant la veille de l'echeance, l'annule-et-remplace est encore admis.
    let tot = liste(c, "regularisations", json!({}), "2026-09-10");
    assert_eq!(tot["mois"]["regularisables"], json!([]));
}

#[test]
fn une_dsn_refusee_ne_se_regularise_pas() {
    let c = &compagnie("regul-refus");
    ecrire(c, "journal/depots.json", &json!([depot_dsn("2026-08", json!([{"nature": "11", "verdict": "KO", "anomalies": []}]))]));
    let r = liste(c, "regularisations", json!({}), "2026-10-03");
    assert_eq!(r["mois"]["regularisables"], json!([]));
}

#[test]
fn les_corrections_se_rangent_par_mois_d_origine_avec_leur_etat() {
    let c = &compagnie("regul-corrections");
    let base = |numero: u32| {
        json!({
            "mois": "2026-08", "numero": numero, "salarie": "BRIOCHE", "debut": "2026-08-19", "motif": "oubli",
            "engagement": {"salarie": "BRIOCHE"}, "decidee_le": "2026-09-20 10:00", "qui": "test",
        })
    };
    let mut preparee = base(2);
    preparee["preparee_par"] = json!({"mois": "2026-10", "fichier": "x", "quand": "q"});
    let mut portee = base(3);
    portee["portee_par"] = json!({"mois": "2026-10"});
    let mut annulee = base(4);
    annulee["annulee_le"] = json!("2026-09-25 09:00");
    annulee["portee_par"] = json!({"mois": "2026-10"});
    ecrire(c, "regularisations/2026-08.json", &json!({"mois": "2026-08", "corrections": [base(1), preparee, portee, annulee]}));
    let mut autre = base(1);
    autre["mois"] = json!("2026-06");
    ecrire(c, "regularisations/2026-06.json", &json!({"mois": "2026-06", "corrections": [autre]}));
    let r = liste(c, "regularisations", json!({}), "2026-10-03");
    let par_mois = r["corrections"].as_array().unwrap();
    assert_eq!(par_mois.len(), 2);
    assert_eq!(par_mois[0]["mois"], json!("2026-06"), "trie par mois d'origine");
    let etats: Vec<&Value> = par_mois[1]["corrections"].as_array().unwrap().iter().map(|x| &x["etat"]).collect();
    assert_eq!(etats, vec![&json!("en attente"), &json!("preparee"), &json!("portee"), &json!("annulee")]);
    let premiere = &par_mois[1]["corrections"][0];
    assert_eq!(premiere["preparee_par"], Value::Null, "les champs absents prennent leur valeur par defaut");
    assert_eq!(premiere["engagement"], json!({"salarie": "BRIOCHE"}));
    // Une correction abimee refuse.
    ecrire(c, "regularisations/2026-09.json", &json!({"corrections": [{"mois": "2026-09"}]}));
    refus(c, "regularisations", json!({}));
}

#[test]
fn le_mot_de_passe_pose_se_lit_dans_le_coffre_sans_jamais_le_rendre() {
    let c = &compagnie("regul-coffre");
    // Un coffre qui n'est pas un coffre : le poste n'en a pas d'ouvert, donc rien n'est pose.
    ecrire_texte(c, "config/coffre.bin", "pas un coffre");
    let r = liste(c, "regularisations", json!({}), "2026-10-03");
    assert_eq!(r["mot_de_passe_pose"], json!(false));
    assert_eq!(r.as_object().unwrap().len(), 3);
}
