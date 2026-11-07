//! La lecture des retours : le compte rendu de nature 94 tel qu'il arrive (transcrit d'un CRM recu, NIR inventes), le
//! certificat de conformite et l'accuse (le bilan tranche, regle 5), les anomalies sans rien de nominatif,
//! l'archive d'une AER de nature 46. Le schema des
//! bilans : `atelier/reference/sources/gip-mds-api-dsn/gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`.

use envois::retours::{
    aer_spectacle, anomalies_codees_d_octets, bilan_d_octets, fusionner_bilans, releves_vers_salaries,
    taux_pas_d_octets,
};
use serde_json::{json, Value};

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
    assert_eq!(att.len(), 2);
    assert_eq!((att[0].periode.as_str(), att[0].qui.as_str()), ("2026-09", "DUPONT ALICE"));
    assert_eq!(att[0].donnees, b"%PDF-un");
    assert_eq!((att[1].periode.as_str(), att[1].qui.as_str()), ("2026-09", "MARTIN PAUL JEAN"));
}
