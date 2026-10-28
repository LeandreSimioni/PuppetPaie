//! La lecture des retours DSN (`envois::retours::commande`) : listes, depots, CRM, bilans, anomalies, fiches de parametrage,
//! AER ; sur des retours fabriques (NIR et SIRET inventes).

use envois::retours::commande;
use socle::base64::{ecrire as base64_ecrire, lire as base64_lire};
use serde_json::{json, Value};

fn appel(operation: &str, mut demande: Value) -> Value {
    demande["operation"] = json!(operation);
    commande(&demande.to_string()).unwrap()
}

fn texte(operation: &str, contenu: &str) -> Value {
    appel(operation, json!({ "texte": contenu }))
}

fn octets(operation: &str, contenu: &[u8]) -> Value {
    appel(operation, json!({ "base64": base64_ecrire(contenu) }))
}

// ---------------------------------------------------- reponses de l'API (guide)

const LISTE_XML: &str = "<retours>
  <flux>
    <id>F1spdeAilkR4q3UUlQA7haa</id>
    <retour>
      <publication>20260904123242</publication>
      <production>20260904123242</production>
      <nature>11</nature>
      <statut>OK</statut>
      <id>0123456789</id>
      <url>https://telechargement.dsnrg.net-entreprises.fr/telecharger-retour/1.0/F1/01</url>
    </retour>
    <retour>
      <publication>20260906080000</publication>
      <production>20260906080000</production>
      <nature>94</nature>
      <statut>OK</statut>
      <id>0123456790</id>
      <url>https://telechargement.dsnrg.net-entreprises.fr/telecharger-retour/1.0/F1/02</url>
    </retour>
  </flux>
</retours>";

#[test]
fn la_liste_des_retours_en_xml() {
    let r = texte("liste_retours", LISTE_XML);
    let r = r["retours"].as_array().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0]["nature"], "11");
    assert_eq!(r[0]["statut"], "OK");
    assert_eq!(r[1]["nature"], "94");
    assert_eq!(r[1]["flux"], "F1spdeAilkR4q3UUlQA7haa");
    assert!(r[1]["url"].as_str().unwrap().ends_with("/F1/02"));
}

#[test]
fn la_liste_des_retours_en_json() {
    let brut = r#"{"retours": {"flux": [{"id": "F1", "retour": [
        {"publication": "20260906080000", "production": "20260906080000",
         "nature": "94", "statut": "OK", "url": "https://h/t/1"}]}]}}"#;
    let r = texte("liste_retours", brut);
    let r = r["retours"].as_array().unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["nature"], "94");
    assert_eq!(r[0]["flux"], "F1");
}

#[test]
fn un_flux_ou_un_retour_seul_vaut_une_liste_d_un() {
    let r = texte("liste_retours", r#"{"flux": {"id": "F9", "retour": {"nature": "10"}}}"#);
    assert_eq!(r["retours"], json!([{"nature": "10", "flux": "F9"}]));
}

#[test]
fn l_identifiant_du_flux_vient_de_l_attribut_puis_de_l_enfant() {
    let r = texte("liste_retours", "<r><flux id=\"A\"><retour><nature>1</nature></retour></flux><flux><id> B </id><retour/></flux></r>");
    assert_eq!(r["retours"], json!([{"nature": "1", "flux": "A"}, {"flux": "B"}]));
}

#[test]
fn la_liste_des_depots() {
    let brut = "<depots><declarant><siret>12345678900017</siret>
      <depot><idflux>WSA82</idflux><date>20260903093654</date><statut>OK</statut></depot>
    </declarant></depots>";
    let d = texte("liste_depots", brut);
    assert_eq!(d["depots"], json!([{"idflux": "WSA82", "date": "20260903093654", "statut": "OK"}]));
    let j = texte("liste_depots", r#"{"depots": {"declarant": {"depot": {"idflux": "W1"}}}}"#);
    assert_eq!(j["depots"], json!([{"idflux": "W1"}]));
}

#[test]
fn un_xml_illisible_est_refuse() {
    for op in ["liste_retours", "liste_depots", "taux_pas", "fiche"] {
        let r = texte(op, "<retours><flux>");
        assert_eq!(r["ok"], false, "{}", op);
    }
    // Un JSON illisible l'est aussi.
    assert_eq!(texte("liste_retours", "{\"retours\": ")["ok"], false);
}

#[test]
fn un_json_de_mauvaise_forme_est_refuse() {
    assert_eq!(texte("liste_retours", r#"{"flux": ["x"]}"#)["ok"], false);
    assert_eq!(texte("liste_retours", r#"{"flux": null}"#)["ok"], false);
}

#[test]
fn les_libelles_de_nature() {
    assert!(appel("libelle", json!({"code": "94"}))["libelle"].as_str().unwrap().contains("PAS"));
    assert_eq!(appel("libelle", json!({"code": "999"}))["libelle"], "nature 999 (inconnue de ce moteur)");
}

// ------------------------------------------------- CRM 94 (formats plausibles)

fn crm_par_code(nir: &str, taux: &str, type_taux: &str, identifiant: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<bilan><declaration><individu>
  <rubrique code=\"S21.G00.30.001\">{}</rubrique>
  <rubrique code=\"S21.G00.50.006\">{}</rubrique>
  <rubrique code=\"S21.G00.50.007\">{}</rubrique>
  <rubrique code=\"S21.G00.50.008\">{}</rubrique>
</individu></declaration></bilan>",
        nir, taux, type_taux, identifiant
    )
}

fn crm_par_nom_metier(nir: &str, taux: &str, type_taux: &str, identifiant: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<bilan><individu><NumeroInscription>{}</NumeroInscription>
  <VersementIndividu>
    <TauxPAS>{}</TauxPAS><TypeTauxPAS>{}</TypeTauxPAS><IdTauxPAS>{}</IdTauxPAS>
  </VersementIndividu>
</individu></bilan>",
        nir, taux, type_taux, identifiant
    )
}

#[test]
fn le_crm_par_code_de_rubrique() {
    let r = texte("taux_pas", &crm_par_code("2990101999999", "0.00", "01", "450189378"));
    assert_eq!(r["strategie"], "rubriques codees");
    assert_eq!(r["releves"], json!([{"nir": "2990101999999", "taux": "0.00", "type": "01", "identifiant": "450189378"}]));
}

#[test]
fn le_crm_par_nom_metier() {
    let r = texte("taux_pas", &crm_par_nom_metier("1990202999999", "3.50", "01", "450189378"));
    assert_eq!(r["strategie"], "noms metier NEODeS");
    assert_eq!(r["releves"][0]["taux"], "3.50");
    assert_eq!(r["releves"][0]["identifiant"], "450189378");
}

#[test]
fn le_balayage_large_est_le_dernier_recours() {
    let r = texte("taux_pas", "<a><b>2990101999999</b><c>3.50</c><d>01</d><e>444207754</e></a>");
    assert_eq!(r["strategie"], "balayage large");
    assert_eq!(r["releves"], json!([{"nir": "2990101999999", "taux": "3.50", "type": "01", "identifiant": "444207754"}]));
}

#[test]
fn un_crm_illisible_ne_pretend_rien() {
    let r = texte("taux_pas", "<bilan><rien/></bilan>");
    assert_eq!(r["strategie"], Value::Null);
    assert_eq!(r["releves"], json!([]));
}

const CRM_REEL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" identifiant=\"450189378\" type=\"CRM nominatif\" profil=\"DSN\" version=\"v02r01\"><envoi><envoi_identification><idflux>WSUrBZ50Y4RWJdDpsRsh.3r</idflux></envoi_identification></envoi><declaration><declaration_bilan><etat>OK</etat><salarie><NIR>1990101999999</NIR><matricule>2</matricule><taux_imposition_PAS>0.00</taux_imposition_PAS></salarie><salarie><NIR>2990101999999</NIR><matricule>1</matricule><taux_imposition_PAS>0.00</taux_imposition_PAS></salarie></declaration_bilan></declaration></ns2:rapport>";

#[test]
fn le_format_reel_du_crm_94() {
    let r = texte("taux_pas", CRM_REEL);
    assert_eq!(r["strategie"], "rapport CRM nominatif");
    assert_eq!(
        r["releves"],
        json!([
            {"nir": "1990101999999", "taux": "0.00", "type": "01", "identifiant": "450189378"},
            {"nir": "2990101999999", "taux": "0.00", "type": "01", "identifiant": "450189378"},
        ])
    );
}

#[test]
fn sans_taux_communique_le_bareme_par_defaut() {
    let sans = CRM_REEL.replacen("<taux_imposition_PAS>0.00</taux_imposition_PAS>", "", 1);
    let r = texte("taux_pas", &sans);
    assert_eq!(r["releves"][0]["type"], "13");
    assert_eq!(r["releves"][0]["identifiant"], "-1");
    assert_eq!(r["releves"][0]["taux"], "0.00");
}

#[test]
fn le_crm_est_lu_en_octets_comme_en_texte() {
    let r = octets("taux_pas", CRM_REEL.as_bytes());
    assert_eq!(r["strategie"], "rapport CRM nominatif");
    // Un octet invalide devient U+FFFD (`decode("utf-8", "replace")`) sans rien casser.
    let mut o = CRM_REEL.as_bytes().to_vec();
    let i = CRM_REEL.find("WSUr").unwrap();
    o.splice(i..i, [0xff]);
    assert_eq!(octets("taux_pas", &o)["ok"], true);
}

#[test]
fn les_releves_se_rattachent_par_le_nir() {
    let releves = json!([
        {"nir": "2990101999999", "taux": "0.00", "type": "01", "identifiant": "1"},
        {"nir": "1234567890123", "taux": "0.00", "type": "01", "identifiant": "1"},
        {"nir": null, "taux": null, "type": null, "identifiant": null},
    ]);
    let nirs = json!([["ALPHA", "2990101999999"], ["BETA", "1990101999999"]]);
    let r = appel("releves", json!({"releves": releves, "nirs": nirs}));
    assert_eq!(r["rattaches"].as_array().unwrap().len(), 1);
    assert_eq!(r["rattaches"][0]["salarie"], "ALPHA");
    // Un NIR qu'on ne connait pas n'est jamais rattache au hasard.
    assert_eq!(r["orphelins"].as_array().unwrap().len(), 2);
}

// ------------------------------------------------------- bilans de depot

fn rapport(type_rapport: &str, corps: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"{}\">{}</ns2:rapport>", type_rapport, corps)
}

fn cco() -> String {
    rapport(
        "CCO",
        "<envoi><idflux>WSAAAA</idflux><date_reception>2026-09-03</date_reception><heure_reception>09:36:54</heure_reception>\
         <essai_reel>01</essai_reel><envoi_etat>OK</envoi_etat><nom_logiciel>PUPPETPAIE</nom_logiciel></envoi>\
         <declaration><declaration_identification><rang>1</rang><type>01</type><fraction>11</fraction><ordre>0</ordre></declaration_identification>\
         <declaration_bilan><etat>OK</etat><etat_conformite>OK</etat_conformite><mois>01082026</mois>\
         <certificat_conformite>CERTIFIE</certificat_conformite><salaries>3</salaries><contrats>4</contrats>\
         </declaration_bilan><siret>12345678900017</siret></declaration>",
    )
}

fn aee() -> String {
    rapport(
        "AEE",
        "<envoi><idflux>WSAAAA</idflux><date_reception>2026-09-03</date_reception><heure_reception>09:36:50</heure_reception>\
         <essai_reel>01</essai_reel><envoi_etat>OK</envoi_etat></envoi>",
    )
}

#[test]
fn un_certificat_de_conformite() {
    let b = texte("bilan", &cco())["bilan"].clone();
    assert_eq!(
        b,
        json!({"flux": "WSAAAA", "recu_le": "2026-09-03T09:36:54", "essai_reel": "01", "etat_envoi": "OK",
               "etat_declaration": "OK", "mois": "2026-08", "logiciel": "PUPPETPAIE", "type_rapport": "CCO",
               "type_declaration": "01", "fraction": "11", "ordre": "0", "salaries": "3", "contrats": "4",
               "conformite": "OK", "certificat": "CERTIFIE", "siret_declarant": "12345678900017"})
    );
}

#[test]
fn un_accuse_ne_dit_ni_le_mois_ni_le_verdict() {
    let b = texte("bilan", &aee())["bilan"].clone();
    assert_eq!(b["mois"], "");
    assert_eq!(b["etat_declaration"], "");
    assert_eq!(b["etat_envoi"], "OK");
    assert_eq!(b["type_rapport"], "AEE");
}

#[test]
fn ce_qui_n_est_pas_un_bilan_de_depot_n_en_est_pas_un() {
    for contenu in [
        rapport("CRM nominatif", "<envoi><idflux>X</idflux></envoi>"),
        rapport("CCO", "<envoi><envoi_etat>OK</envoi_etat></envoi>"), // sans flux
        "<autre type=\"CCO\"><idflux>X</idflux></autre>".to_string(),
        "pas du xml".to_string(),
        String::new(),
        cco()[..80].to_string(),
    ] {
        assert_eq!(texte("bilan", &contenu)["bilan"], Value::Null, "{}", contenu);
    }
}

#[test]
fn le_blanc_de_tete_ne_gene_pas_le_bilan() {
    let b = texte("bilan", &format!("\n  {}\n", cco()))["bilan"].clone();
    assert_eq!(b["flux"], "WSAAAA");
}

#[test]
fn le_bilan_de_declaration_l_emporte_sur_l_accuse() {
    let a = texte("bilan", &aee())["bilan"].clone();
    let c = texte("bilan", &cco())["bilan"].clone();
    for ordre in [json!([a, c]), json!([c, a])] {
        let f = appel("fusionner", json!({ "bilans": ordre }))["bilan"].clone();
        assert_eq!(f["type_rapport"], "CCO");
        assert_eq!(f["recu_le"], "2026-09-03T09:36:54");
        assert_eq!(f["mois"], "2026-08");
    }
    // Un accuse seul reste exploitable ; une liste vide ne rend rien.
    let seul = appel("fusionner", json!({ "bilans": [a] }))["bilan"].clone();
    assert_eq!(seul["flux"], "WSAAAA");
    assert_eq!(appel("fusionner", json!({ "bilans": [] }))["bilan"], Value::Null);
}

#[test]
fn la_fusion_complete_ce_qui_manque() {
    let mut c = texte("bilan", &cco())["bilan"].clone();
    c["recu_le"] = json!("");
    c["essai_reel"] = json!("");
    let a = texte("bilan", &aee())["bilan"].clone();
    let f = appel("fusionner", json!({ "bilans": [a, c] }))["bilan"].clone();
    assert_eq!(f["type_rapport"], "CCO");
    assert_eq!(f["recu_le"], "2026-09-03T09:36:50");
    assert_eq!(f["essai_reel"], "01");
}

fn ban() -> String {
    rapport(
        "BAN",
        "<envoi><idflux>WSBBBB</idflux></envoi><declaration><declaration_bilan><etat>KO</etat>\
         <declaration_anomalie><code>CST-04</code><categorie>Bloquante</categorie>\
         <message> Code URSSAF invalide </message><identifiant>S21.G00.20.001</identifiant><valeur>X</valeur>\
         <nom_famille>FAVART</nom_famille></declaration_anomalie>\
         <declaration_anomalie><categorie>Bloquante</categorie></declaration_anomalie>\
         <declaration_anomalie><code>B1-208-08</code><message>Sans categorie</message></declaration_anomalie>\
         </declaration_bilan></declaration><envoi_anomalie><code>ENV-01</code></envoi_anomalie>",
    )
}

#[test]
fn les_anomalies_en_phrases_lisibles() {
    let r = octets("anomalies", ban().as_bytes());
    assert_eq!(r["anomalies"], json!(["Code URSSAF invalide (Bloquante) -- FAVART", "Sans categorie"]));
}

#[test]
fn les_anomalies_en_codes_ne_gardent_ni_le_nom_ni_la_valeur() {
    let r = octets("anomalies_codees", ban().as_bytes());
    assert_eq!(
        r["anomalies"],
        json!([
            {"code": "CST-04", "categorie": "Bloquante", "message": "Code URSSAF invalide", "rubrique": "S21.G00.20.001"},
            {"code": "B1-208-08", "categorie": "", "message": "Sans categorie", "rubrique": ""},
            {"code": "ENV-01", "categorie": "", "message": "", "rubrique": ""},
        ])
    );
}

#[test]
fn des_anomalies_en_octets_lisent_l_encodage_declare() {
    let latin1: Vec<u8> = "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r><declaration_anomalie><message>caf\u{e9}</message></declaration_anomalie></r>"
        .chars()
        .map(|c| c as u8)
        .collect();
    assert_eq!(octets("anomalies", &latin1)["anomalies"], json!(["caf\u{e9}"]));
    // Du latin-1 sous une declaration UTF-8 n'est pas du XML : aucune anomalie, pas d'exception.
    let faux: Vec<u8> = latin1.iter().copied().chain([]).collect();
    let mut faux = String::from_utf8_lossy(&faux).replace("ISO-8859-1", "UTF-8").into_bytes();
    faux.extend_from_slice(&[0xe9]);
    assert_eq!(octets("anomalies", &faux)["anomalies"], json!([]));
}

// ------------------------------------------------ fiche de parametrage OC

#[test]
fn la_fiche_de_parametrage() {
    let fiche = "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>
<FicheParametrage IdentifiantFiche=\"F1\" Version=\"2\" DateHeureCreation=\"2026-01-01T00:00:00\">
  <ProducteurFiche RaisonSocialeProducteur=\"AUDIENS\"/>
  <GROUPE NumeroGrp=\"1\" LibelleGrp=\"Intermittents\">
    <ParametresContrats DateDebutValidite=\"2026-01-01\" DateFinValidite=\"2026-12-31\">
      <Contrat LibelleContrat=\"Prevoyance\" ReferenceContrat=\"R1\"/>
      <ElementsDeCalculAttendus>
        <TrancheA LibelleCodeNature=\"Salariale\"><Taux> 1.20 </Taux></TrancheA>
        <TrancheB LibelleCodeNature=\"Patronale\"><Autre>x</Autre></TrancheB>
      </ElementsDeCalculAttendus>
    </ParametresContrats>
    <ParametresContrats/>
  </GROUPE>
</FicheParametrage>";
    let f = texte("fiche", fiche)["fiche"].clone();
    assert_eq!(f["identifiant"], "F1");
    assert_eq!(f["producteur"], "AUDIENS");
    assert_eq!(f["groupes"][0]["numero"], "1");
    assert_eq!(
        f["groupes"][0]["contrats"][0],
        json!({"debut": "2026-01-01", "fin": "2026-12-31", "libelle": "Prevoyance", "reference": "R1", "option": null,
               "population": null, "taux": [{"assiette": "TrancheA", "nature": "Salariale", "taux": "1.20"}],
               "bases_specifiques": [], "compteurs": []})
    );
    assert_eq!(
        f["groupes"][0]["contrats"][1],
        json!({"debut": null, "fin": null, "libelle": null, "reference": null, "option": null, "population": null, "taux": [],
               "bases_specifiques": [], "compteurs": []})
    );
    // Une version que les formats deposes ne nomment pas se lit comme avant, sans format.
    assert_eq!(f["format"], json!(null));
}

/// Les fixtures fictives `tests/donnees/fiches-parametrage/fiche-fictive-139.xml` et `-140.xml`, fabriquees depuis le
/// format depose (`atelier/reference/sources/fpoc-format/`) et validees contre son schema : la 1.4.0 par `fpoc-r1.4.0.xsd`
/// (et refusee par `fpoc-r1.3.9.xsd` sur sa version, `BasePlafond` et `BasePlancher`), la 1.3.9 par `fpoc-r1.3.9.xsd`.
fn fiche_fictive(version: &str) -> Value {
    let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/donnees/fiches-parametrage/fiche-fictive-{}.xml", version));
    octets("fiche", &std::fs::read(chemin).unwrap())["fiche"].clone()
}

#[test]
fn une_fiche_1_3_9_se_lit_avec_ses_conditions_tarifaires() {
    let f = fiche_fictive("139");
    assert_eq!((f["version"].as_str(), f["format"].as_str()), (Some("139"), Some("1.3.9")));
    assert_eq!(f["producteur"], "ORGANISME FICTIF");
    // La prevoyance : une tranche A et une base forfaitaire sans condition (comme la fiche Audiens), aucun compteur.
    let prev = &f["groupes"][0]["contrats"][0];
    assert_eq!(prev["population"], "NC");
    assert_eq!(prev["taux"], json!([{"assiette": "TAPrev", "nature": null, "taux": "0.7500"},
                                     {"assiette": "BaseMontantSpecifique", "nature": "ISOLE PSS", "taux": "0.5000"}]));
    assert_eq!(prev["bases_specifiques"][0],
               json!({"code_nature": "18", "libelle": "ISOLE PSS", "condition": null, "libelle_condition": null, "taux": "0.5000",
                      "montant": null, "coef": "1.00", "assiette": "PMSS", "plancher": null, "plafond": null}));
    assert_eq!(prev["compteurs"], json!([]));
    // La sante en trois forfaits 02/05/07 (combinaison OC.D5 / OC.39) : trois bases, et le seul compteur 70.009 (des
    // ayants droit comptes sans type, specification l. 422-423).
    let sante = &f["groupes"][1]["contrats"][0];
    assert_eq!(sante["option"], "AD");
    let b = sante["bases_specifiques"].as_array().unwrap();
    let vu: Vec<(&str, &str, &str)> = b.iter().map(|x| (x["condition"].as_str().unwrap(), x["code_nature"].as_str().unwrap(), x["montant"].as_str().unwrap())).collect();
    assert_eq!(vu, [("02", "20", "40.00"), ("05", "20", "70.00"), ("07", "20", "95.00")]);
    assert_eq!(b[1]["libelle_condition"], "tarif unique pour l'assure et 1 ayant droit");
    assert_eq!(sante["compteurs"], json!(["S21.G00.70.009"]));
}

#[test]
fn une_fiche_1_4_0_se_lit_avec_ses_planchers_et_plafonds() {
    let f = fiche_fictive("140");
    assert_eq!((f["version"].as_str(), f["format"].as_str()), (Some("140"), Some("1.4.0")));
    // Ouvrant droit, conjoint, par enfant (90/91/94) : trois bases ; les compteurs 70.008, 70.010 et 70.011 (des ayants
    // droit de type Conjoint, Autre ou Enfant, specification R1.4.0 l. 527-530). Le plafond de la base enfant (1.4.0) :
    // un taux, un coefficient et une assiette.
    let famille = &f["groupes"][0]["contrats"][0];
    assert_eq!(famille["option"], "FAM");
    let b = famille["bases_specifiques"].as_array().unwrap();
    let conditions: Vec<&str> = b.iter().map(|x| x["condition"].as_str().unwrap()).collect();
    assert_eq!(conditions, ["90", "91", "94"]);
    assert_eq!(famille["compteurs"], json!(["S21.G00.70.008", "S21.G00.70.010", "S21.G00.70.011"]));
    assert_eq!(b[2]["plafond"], json!({"montant": null, "taux": "2.2200", "coef": "1.00", "assiette": "PMSS"}));
    assert_eq!(b[2]["plancher"], json!(null));
    assert_eq!(b[0]["plafond"], json!(null));
    // Une condition seule (02), que la 1.4.0 admet (OC.39) : un forfait et son plancher en montant ; aucun compteur.
    let isole = &f["groupes"][0]["contrats"][1];
    assert_eq!(isole["bases_specifiques"][0]["montant"], "45.00");
    assert_eq!(isole["bases_specifiques"][0]["plancher"], json!({"montant": "30.00", "taux": null, "coef": null, "assiette": null}));
    assert_eq!(isole["compteurs"], json!([]));
}

#[test]
fn une_condition_tarifaire_inconnue_refuse() {
    let fiche = r#"<FICHE IdentifiantFiche="F9" Version="140"><GROUPE NumeroGrp="1"><ParametresContrats>
<Contrat ReferenceContrat="R9" LibelleContrat="SANTE"/><ElementsDeCalculAttendus>
<BaseMontantSpecifique ValeurCodeNature="20" Condition="08"><Montant>10.00</Montant></BaseMontantSpecifique>
</ElementsDeCalculAttendus></ParametresContrats></GROUPE></FICHE>"#;
    let e = commande(&json!({"operation": "fiche", "texte": fiche}).to_string()).unwrap_err();
    assert!(e.contains("« 08 »") && e.contains("R9") && e.contains("ne se devinent pas"), "{}", e);
}

#[test]
fn les_octets_d_une_fiche_se_lisent_en_latin_1() {
    // La specification : « respecter l'encodage ISO_8859_1 »
    // (`atelier/reference/sources/fpoc-format/fpoc-specification-r1.4.0-20260721.txt:14`). Un E accent aigu en UTF-8 (deux
    // octets) y est donc deux lettres.
    let o = "<F IdentifiantFiche=\"\u{e9}\"/>".as_bytes();
    assert_eq!(octets("fiche", o)["fiche"]["identifiant"], "\u{c3}\u{a9}");
}

// ------------------------------------------------------- l'AER, le .zip

/// Un .zip a la main : `(nom, contenu, deflate)`.
fn zip(membres: &[(&str, &[u8], bool)]) -> Vec<u8> {
    use flate2::write::DeflateEncoder;
    use flate2::{Compression, Crc};
    use std::io::Write;
    let (mut corps, mut central) = (Vec::new(), Vec::new());
    for (nom, donnees, deflate) in membres {
        let mut crc = Crc::new();
        crc.update(donnees);
        let comprimes = if *deflate {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            e.write_all(donnees).unwrap();
            e.finish().unwrap()
        } else {
            donnees.to_vec()
        };
        let methode: u16 = if *deflate { 8 } else { 0 };
        let drapeaux: u16 = if nom.is_ascii() { 0 } else { 0x800 };
        let decalage = corps.len() as u32;
        for (t, ent) in [(0x04034b50u32, false), (0x02014b50u32, true)] {
            let cible: &mut Vec<u8> = if ent { &mut central } else { &mut corps };
            cible.extend(t.to_le_bytes());
            if ent {
                cible.extend(20u16.to_le_bytes());
            }
            cible.extend(20u16.to_le_bytes());
            cible.extend(drapeaux.to_le_bytes());
            cible.extend(methode.to_le_bytes());
            cible.extend([0u8; 4]);
            cible.extend(crc.sum().to_le_bytes());
            cible.extend((comprimes.len() as u32).to_le_bytes());
            cible.extend((donnees.len() as u32).to_le_bytes());
            cible.extend((nom.len() as u16).to_le_bytes());
            cible.extend(0u16.to_le_bytes());
            if ent {
                cible.extend([0u8; 10]);
                cible.extend(decalage.to_le_bytes());
            }
            cible.extend(nom.as_bytes());
        }
        corps.extend(comprimes);
    }
    let debut = corps.len() as u32;
    let taille = central.len() as u32;
    corps.extend(central);
    corps.extend(0x06054b50u32.to_le_bytes());
    corps.extend([0u8; 4]);
    corps.extend((membres.len() as u16).to_le_bytes());
    corps.extend((membres.len() as u16).to_le_bytes());
    corps.extend(taille.to_le_bytes());
    corps.extend(debut.to_le_bytes());
    corps.extend(0u16.to_le_bytes());
    corps
}

#[test]
fn les_attestations_d_une_archive_de_nature_46() {
    let a = zip(&[
        ("AER_12345678900017__092026_ABC_03102026_DUPONT_ALICE_1_123456.pdf", b"%PDF-un", true),
        ("dossier/", b"", false),
        ("AER_12345678900017__092026_ABC_03102026_MARTIN_PAUL_JEAN_2_77.PDF", b"%PDF-deux", false),
        ("sous/autre nom.pdf", b"%PDF-trois", true),
    ]);
    let r = octets("aer", &a);
    let att = r["attestations"].as_array().unwrap();
    assert_eq!(att.len(), 3);
    assert_eq!(att[0]["periode"], "2026-09");
    assert_eq!(att[0]["qui"], "DUPONT ALICE");
    assert_eq!(base64_lire(att[0]["donnees"].as_str().unwrap()).unwrap(), b"%PDF-un");
    assert_eq!(att[1]["qui"], "MARTIN PAUL JEAN");
    assert_eq!(att[1]["periode"], "2026-09");
    // Un nom hors motif est rendu quand meme, periode vide.
    assert_eq!(att[2]["nom_archive"], "sous/autre nom.pdf");
    assert_eq!(att[2]["periode"], "");
    assert_eq!(att[2]["qui"], "autre nom");
    assert_eq!(base64_lire(att[2]["donnees"].as_str().unwrap()).unwrap(), b"%PDF-trois");
}

#[test]
fn un_contenu_qui_n_est_pas_une_archive_rend_une_liste_vide() {
    for o in [&b""[..], b"pas un zip", b"PK\x05\x06 trop court", &zip(&[("a.pdf", b"x", false)])[..20]] {
        assert_eq!(octets("aer", o)["attestations"], json!([]));
    }
}

#[test]
fn une_archive_dont_les_donnees_sont_alterees_leve() {
    let mut a = zip(&[("a.pdf", b"contenu intact", false)]);
    a[30 + 5] ^= 1;
    let r = octets("aer", &a);
    assert_eq!(r["ok"], false);
}

#[test]
fn les_noms_de_l_archive_en_cp437() {
    // 0x82 est « e accent aigu » en cp437.
    let mut a = zip(&[("AER_1__092026_X_03102026_Z_1_2.pdf", b"x", false)]);
    let i = a.windows(1).position(|w| w[0] == b'Z').unwrap();
    a.splice(i..=i, [0x82]);
    let j = a.iter().rposition(|&o| o == b'Z').unwrap();
    a.splice(j..=j, [0x82]);
    let r = octets("aer", &a);
    assert_eq!(r["attestations"][0]["qui"], "\u{e9}");
}

#[test]
fn le_rattachement_par_le_nom_le_plus_long() {
    let index = json!([["DUPONT", "Dupont Alice"], ["DUPONT-DUPARC", "Dupont Duparc Alice"], ["MARTIN", "Martin Paul"], ["VIDE", ""]]);
    let r = appel("rattacher", json!({"quis": ["DUPONT DUPARC ALICE", "dupont alice", "Zorro", "MARTIN PAUL JEAN", "\u{c9}ric"], "index": index}));
    assert_eq!(r["cles"], json!(["DUPONT-DUPARC", "DUPONT", null, "MARTIN", null]));
}

// ------------------------------------------------------------------ base64

#[test]
fn le_base64_aller_retour() {
    for o in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0u8, 255, 128, 7, 9]] {
        assert_eq!(base64_lire(&base64_ecrire(o)).unwrap(), o);
    }
    assert_eq!(base64_ecrire(b"Man"), "TWFu");
    assert_eq!(base64_ecrire(b"Ma"), "TWE=");
    assert!(base64_lire("***").is_err());
}

#[test]
fn une_demande_mal_formee_se_refuse() {
    assert!(commande("pas du json").is_err());
    assert!(commande("{}").is_err());
    assert!(commande(r#"{"operation": "bilan"}"#).is_err());
    assert!(commande(r#"{"operation": "inconnue"}"#).is_err());
}
