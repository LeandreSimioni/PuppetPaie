//! La lecture des retours : le compte rendu de nature 94 tel qu'il arrive (transcrit d'un CRM recu, NIR inventes), le
//! certificat de conformite et l'accuse (le bilan tranche, regle 5), les anomalies sans rien de nominatif, les fiches de
//! parametrage au format depose (`atelier/reference/sources/fpoc-format/`), l'archive d'une AER de nature 46. Le schema des
//! bilans : `atelier/reference/sources/gip-mds-api-dsn/gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`.

use envois::retours::{
    aer_spectacle, anomalies_codees_d_octets, bilan_d_octets, fiche_d_octets, fusionner_bilans, releves_vers_salaries,
    taux_pas_d_octets,
};
use serde_json::{json, Value};
use socle::base64::lire as base64_lire;

use crate::outils::donnee;

const CRM_REEL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" identifiant=\"450189378\" type=\"CRM nominatif\" profil=\"DSN\" version=\"v02r01\"><envoi><envoi_identification><idflux>WSUrBZ50Y4RWJdDpsRsh.3r</idflux></envoi_identification></envoi><declaration><declaration_bilan><etat>OK</etat><salarie><NIR>1990101999999</NIR><matricule>2</matricule><taux_imposition_PAS>0.00</taux_imposition_PAS></salarie><salarie><NIR>2990101999999</NIR><matricule>1</matricule><taux_imposition_PAS>0.00</taux_imposition_PAS></salarie></declaration_bilan></declaration></ns2:rapport>";

#[test]
fn le_format_reel_du_crm_94() {
    let r = taux_pas_d_octets(CRM_REEL.as_bytes()).unwrap();
    assert_eq!(
        r["releves"],
        json!([
            {"nir": "1990101999999", "taux": "0.00", "type": "01", "identifiant": "450189378"},
            {"nir": "2990101999999", "taux": "0.00", "type": "01", "identifiant": "450189378"},
        ])
    );
}

#[test]
fn les_releves_se_rattachent_par_le_nir() {
    let releves = [
        json!({"nir": "2990101999999", "taux": "0.00", "type": "01", "identifiant": "1"}),
        json!({"nir": "1234567890123", "taux": "0.00", "type": "01", "identifiant": "1"}),
        json!({"nir": null, "taux": null, "type": null, "identifiant": null}),
    ];
    let nirs = [json!(["ALPHA", "2990101999999"]), json!(["BETA", "1990101999999"])];
    let r = releves_vers_salaries(&releves, &nirs);
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

fn ban() -> String {
    rapport(
        "BAN",
        "<envoi><idflux>WSBBBB</idflux></envoi><declaration><declaration_bilan><etat>KO</etat>\
         <declaration_anomalie><code>CST-04</code><categorie>Bloquante</categorie>\
         <message> Code URSSAF invalide </message><identifiant>S21.G00.20.001</identifiant><valeur>X</valeur>\
         <nom_famille>FAVART</nom_famille></declaration_anomalie>\
         <declaration_anomalie><code>B1-208-08</code><message>Sans categorie</message></declaration_anomalie>\
         </declaration_bilan></declaration>",
    )
}

#[test]
fn un_certificat_de_conformite() {
    let b = bilan_d_octets(cco().as_bytes());
    assert_eq!(b["type_rapport"], "CCO");
    assert_eq!(b["essai_reel"], "01");
    assert_eq!(b["etat_declaration"], "OK");
    assert_eq!(b["conformite"], "OK");
    assert_eq!(b["certificat"], "CERTIFIE");
    assert_eq!(b["mois"], "2026-08");
    assert_eq!((b["salaries"].as_str(), b["contrats"].as_str()), (Some("3"), Some("4")));
}

#[test]
fn le_bilan_de_declaration_l_emporte_sur_l_accuse() {
    let a = bilan_d_octets(aee().as_bytes());
    let c = bilan_d_octets(cco().as_bytes());
    // Un accuse ne dit ni le mois ni le verdict de la declaration.
    assert_eq!((a["etat_declaration"].as_str(), a["mois"].as_str()), (Some(""), Some("")));
    for ordre in [[a.clone(), c.clone()], [c.clone(), a.clone()]] {
        let f = fusionner_bilans(&ordre);
        assert_eq!(f["type_rapport"], "CCO");
        assert_eq!(f["etat_declaration"], "OK");
        assert_eq!(f["mois"], "2026-08");
    }
}

#[test]
fn les_anomalies_en_codes_ne_gardent_ni_le_nom_ni_la_valeur() {
    let a = anomalies_codees_d_octets(ban().as_bytes()).unwrap();
    assert_eq!(a[0]["code"], "CST-04");
    assert_eq!(a[0]["rubrique"], "S21.G00.20.001");
    let tout = Value::Array(a).to_string();
    assert!(!tout.contains("FAVART") && !tout.contains("\"X\""), "{}", tout);
}

// ------------------------------------------------ fiche de parametrage OC

#[test]
fn une_fiche_1_4_0_se_lit_avec_ses_planchers_et_plafonds() {
    // La fixture fictive, validee contre `fpoc-r1.4.0.xsd` (et refusee par `fpoc-r1.3.9.xsd`).
    let f = fiche_d_octets(&donnee("fiches-parametrage/fiche-fictive-140.xml")).unwrap();
    assert_eq!((f["version"].as_str(), f["format"].as_str()), (Some("140"), Some("1.4.0")));
    // Ouvrant droit, conjoint, par enfant (90/91/94) : trois bases ; les compteurs 70.008, 70.010 et 70.011 (des ayants
    // droit de type Conjoint, Autre ou Enfant, specification R1.4.0 l. 527-530). Le plafond de la base enfant (1.4.0) :
    // un taux, un coefficient et une assiette.
    let famille = &f["groupes"][0]["contrats"][0];
    let b = famille["bases_specifiques"].as_array().unwrap();
    let conditions: Vec<&str> = b.iter().map(|x| x["condition"].as_str().unwrap()).collect();
    assert_eq!(conditions, ["90", "91", "94"]);
    assert_eq!(famille["compteurs"], json!(["S21.G00.70.008", "S21.G00.70.010", "S21.G00.70.011"]));
    assert_eq!(b[2]["plafond"], json!({"montant": null, "taux": "2.2200", "coef": "1.00", "assiette": "PMSS"}));
    // Une condition seule (02), que la 1.4.0 admet (OC.39) : un forfait et son plancher en montant ; aucun compteur.
    let isole = &f["groupes"][0]["contrats"][1];
    assert_eq!(isole["bases_specifiques"][0]["plancher"], json!({"montant": "30.00", "taux": null, "coef": null, "assiette": null}));
    assert_eq!(isole["compteurs"], json!([]));
}

#[test]
fn une_condition_tarifaire_inconnue_refuse() {
    // Les compteurs d'une condition hors des deux formats deposes ne se devinent pas.
    let fiche = r#"<FICHE IdentifiantFiche="F9" Version="140"><GROUPE NumeroGrp="1"><ParametresContrats>
<Contrat ReferenceContrat="R9" LibelleContrat="SANTE"/><ElementsDeCalculAttendus>
<BaseMontantSpecifique ValeurCodeNature="20" Condition="08"><Montant>10.00</Montant></BaseMontantSpecifique>
</ElementsDeCalculAttendus></ParametresContrats></GROUPE></FICHE>"#;
    assert!(fiche_d_octets(fiche.as_bytes()).is_err());
}

#[test]
fn les_octets_d_une_fiche_se_lisent_en_latin_1() {
    // La specification : « respecter l'encodage ISO_8859_1 »
    // (`atelier/reference/sources/fpoc-format/fpoc-specification-r1.4.0-20260721.txt:14`). Un E accent aigu en UTF-8 (deux
    // octets) y est donc deux lettres.
    let o = "<F IdentifiantFiche=\"\u{e9}\"/>".as_bytes();
    assert_eq!(fiche_d_octets(o).unwrap()["identifiant"], "\u{c3}\u{a9}");
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
    // Les noms des pieces recues : `AER_<siret>__<MMAAAA>_..._<NOM>_<PRENOMS>_<rang>_<n>.pdf`.
    let a = zip(&[
        ("AER_12345678900017__092026_ABC_03102026_DUPONT_ALICE_1_123456.pdf", b"%PDF-un", true),
        ("dossier/", b"", false),
        ("AER_12345678900017__092026_ABC_03102026_MARTIN_PAUL_JEAN_2_77.PDF", b"%PDF-deux", false),
    ]);
    let att = aer_spectacle(&a).unwrap();
    let att = att.as_array().unwrap();
    assert_eq!(att.len(), 2);
    assert_eq!((att[0]["periode"].as_str(), att[0]["qui"].as_str()), (Some("2026-09"), Some("DUPONT ALICE")));
    assert_eq!(base64_lire(att[0]["donnees"].as_str().unwrap()).unwrap(), b"%PDF-un");
    assert_eq!((att[1]["periode"].as_str(), att[1]["qui"].as_str()), (Some("2026-09"), Some("MARTIN PAUL JEAN")));
}
