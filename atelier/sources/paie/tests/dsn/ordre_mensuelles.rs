//! L'ordre d'une DSN MENSUELLE se lit sur ce que la plate-forme a vu, et le reel attend le bilan du test (tickets #221,
//! #222 du depot prive, 19/10/2026 : un essai d'octobre a pris B1-208-11, « 'annule et remplace' d'ordre 20261021
//! inferieur (ou egal) au dernier ordre accepte 20261030 », pendant que l'ecran disait « testee — conforme » sur le seul
//! accuse d'enregistrement de l'essai suivant).
//!
//! Le numero d'ordre d'une mensuelle est « le rang de constitution d'une declaration au cours du mois courant », remis a
//! zero chaque premier jour du mois civil (cahier technique DSN 2026.1, p. 136,
//! `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`). La plate-forme se souvient de tout ce qui est parti sous le
//! SIREN, essais de l'atelier compris : ces essais ne s'affichent pas (`journal-reprise`), mais ils comptent pour l'ordre
//! (Leandre, 20/10/2026).
//!
//! Sur des copies de `fictive-3090-grande` (compagnie inventee, ses envois sont joues : rien ne quitte le poste). Les
//! valeurs du poste sont transcrites en fictif : juillet 2026 au lieu d'octobre, un ordre 30 deja accepte.

use crate::outils::{self, compagnie, ecrire, lire, refus, reussi};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Le SIREN de `fictive-3090-grande`.
const SIREN: &str = "123456915";
const JOUR: &str = "2026-08-03";
const JUILLET: &str = "sorties/2026-07/DSN_Juillet2026.dsn";

/// Une copie de la 3090-grande avec la DSN de juillet 2026 ecrite (constituee le 31/07/2026), et un journal ou juillet
/// n'est jamais parti, ni en test ni en reel.
fn copie(test: &str) -> PathBuf {
    let racine = compagnie::ses_dsn("fictive-3090-grande", &["2026-07"], &outils::dossier_de_test("ordre_mensuelles", test));
    let mut j = lire(&racine, "journal/depots.json");
    j.as_array_mut().unwrap().retain(|l| !(l["quoi"] == json!("dsn") && l["mois"] == json!("2026-07")));
    ecrire(&racine, "journal/depots.json", &j);
    racine
}

fn route(racine: &Path, nom: &str, charge: Value) -> Value {
    outils::route(racine, nom, charge, JOUR)
}

fn deposer(racine: &Path, reel: bool) -> Value {
    route(racine, "deposer", json!({"quoi": "dsn", "fichier": racine.join(JUILLET).to_string_lossy(), "reel": reel}))
}

/// Un bilan de nature 11 comme la plate-forme le rend (schema `v01r08`) : un certificat (`CCO`), ou un bilan d'anomalies
/// (`BAN`) quand `anomalie` est donnee. `essai_reel` : 01 essai, 02 reel ; `recu` : la date de reception.
fn bilan(flux: &str, essai_reel: &str, ordre: &str, recu: &str, anomalie: Option<(&str, &str)>) -> String {
    let (t, etat) = if anomalie.is_some() { ("BAN", "KO") } else { ("CCO", "OK") };
    let a = anomalie
        .map(|(code, message)| {
            format!("<declaration_anomalie><description><code>{}</code><categorie>bloquant</categorie><message>{}</message><nombre>1</nombre></description></declaration_anomalie>", code, message)
        })
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gipmds:rapport xmlns:gipmds=\"http://www.gip-mds.fr/\" profil=\"DSN\" type=\"{t}\" version=\"v01r08\">\
<envoi><envoi_identification><declarant><siret>{s}00011</siret><nom>FICTIF</nom><prenom>DECLARANT</prenom></declarant><idflux>{f}</idflux>\
<date_reception>{r}</date_reception><heure_reception>10:00:00</heure_reception><nom_logiciel>ATELIER</nom_logiciel>\
<essai_reel>{e}</essai_reel><emetteur_siret>{s}00011</emetteur_siret></envoi_identification><envoi_bilan><envoi_etat>OK</envoi_etat></envoi_bilan></envoi>\
<declaration><declaration_identification><rang>1</rang><nature>01</nature><type>01</type><fraction>11</fraction><ordre>{o}</ordre>\
<mois>01072026</mois><SIREN>{s}</SIREN><nic_siege>00011</nic_siege><nic_affectation>00011</nic_affectation></declaration_identification>\
<declaration_bilan><etat>{et}</etat><declaration_contenu><salaries>1</salaries><contrats>1</contrats></declaration_contenu></declaration_bilan>{a}</declaration>\
</gipmds:rapport>",
        t = t, s = SIREN, f = flux, r = recu, e = essai_reel, o = ordre, et = etat, a = a
    )
}

fn archiver(racine: &Path, flux: &str, xml: &str) {
    std::fs::create_dir_all(racine.join("retours")).unwrap();
    std::fs::write(racine.join(format!("retours/20260731-101010_nature11_{}.xml", flux)), xml).unwrap();
}

/// La derniere ligne DSN du journal.
fn derniere(racine: &Path) -> Value {
    lire(racine, "journal/depots.json").as_array().unwrap().iter().rev().find(|l| l["quoi"] == json!("dsn")).unwrap().clone()
}

#[test]
fn un_essai_de_l_atelier_ne_s_affiche_pas_mais_le_test_suivant_part_au_dessus_de_son_ordre() {
    // L'atelier (meme compte net-entreprises, autre dossier) a fait accepter un essai de juillet a l'ordre 30 ; le journal
    // de la compagnie n'en sait rien. Sans le lire, le premier essai partirait en normale a l'ordre 1, et la base d'essai,
    // qui a deja une normale de juillet, le refuserait ; le suivant, en annule-et-remplace a un ordre trop petit,
    // prendrait B1-208-11.
    let racine = copie("atelier-invisible");
    archiver(&racine, "FLUX-ATELIER", &bilan("FLUX-ATELIER", "01", "30", "2026-07-31", None));

    // Invisible : la reprise ne le montre pas et ne le remet pas au journal.
    let r = reussi(&route(&racine, "journal-reprise", json!({}))).clone();
    assert!(!r.to_string().contains("FLUX-ATELIER"), "un essai que ce logiciel n'a pas envoye ne s'affiche pas : {}", r);

    // Mais il compte pour l'ordre : l'essai part en annule-et-remplace a 31.
    reussi(&deposer(&racine, false));
    let l = derniere(&racine);
    assert_eq!((&l["mode"], &l["detail"]["ordre_test"]), (&json!("test"), &json!(31)), "{}", l);
}

#[test]
fn le_rang_cite_par_un_refus_b1_208_11_recale_le_compteur() {
    // Le refus du poste, transcrit : la plate-forme cite le rang « AAAAMM + ordre » du dernier accepte. Un essai de ce
    // dossier (ordre 21) l'a pris ; le suivant part au-dessus du rang cite, pas a 22.
    let racine = copie("b1-208-11");
    let mut j = lire(&racine, "journal/depots.json");
    j.as_array_mut().unwrap().push(json!({
        "quoi": "dsn", "quand": "2026-08-02T16:59:06", "origine": "", "mois": "2026-07", "fichier": "DSN_Juillet2026.dsn",
        "empreinte": "autre", "mode": "test", "qui": "interface", "accepte": true, "destinataire": "net-entreprises",
        "flux": "FLUX-REFUSE", "retours": [], "reponse": "", "trace": null, "detail": {"type": "01", "ordre": "1", "ordre_test": 21},
    }));
    ecrire(&racine, "journal/depots.json", &j);
    archiver(
        &racine,
        "FLUX-REFUSE",
        &bilan(
            "FLUX-REFUSE",
            "01",
            "21",
            "2026-08-02",
            Some(("B1-208-11", "'annule et remplace' d'ordre 2026072 inferieur (ou egal) au dernier ordre accepte 20260730.")),
        ),
    );
    reussi(&deposer(&racine, false));
    assert_eq!(derniere(&racine)["detail"]["ordre_test"], json!(31), "{}", derniere(&racine));
}

#[test]
fn le_reel_attend_le_bilan_du_test_et_l_accuse_d_enregistrement_ne_suffit_pas() {
    // Regle 5 : un accuse de transport OK n'est pas une declaration acceptee ; c'est le bilan (CCO/BAN) qui tranche.
    let racine = copie("reel-attend-bilan");
    reussi(&deposer(&racine, false));
    let flux = derniere(&racine)["flux"].clone();
    let (classe, m) = refus(&deposer(&racine, true));
    assert_eq!(classe, "RepetitionManquante", "{}", m);
    assert!(m.contains("pas encore arrive") && m.contains("Rien n'est parti"), "{}", m);

    let poser = |retours: Value| {
        let mut j = lire(&racine, "journal/depots.json");
        for l in j.as_array_mut().unwrap() {
            if l["flux"] == flux {
                l["retours"] = retours.clone();
            }
        }
        ecrire(&racine, "journal/depots.json", &j);
    };
    // L'accuse d'enregistrement seul : l'etape du mois dit « en test », et le reel attend encore.
    poser(json!([{"nature": "10", "verdict": "OK", "anomalies": []}]));
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": "2026-07"}))).clone();
    assert_eq!(b["dsn"]["etape"], json!("en_test"), "{}", b["dsn"]);
    assert_eq!(refus(&deposer(&racine, true)).0, "RepetitionManquante");

    // Le bilan d'anomalies : l'etape dit le refus AVEC son anomalie, la ou l'on propose d'envoyer.
    poser(json!([{"nature": "10", "verdict": "OK"}, {"nature": "11", "verdict": "KO", "anomalies": ["B1-208-11 : ordre trop petit"]}]));
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": "2026-07"}))).clone();
    assert_eq!(b["dsn"]["etape"], json!("test_refuse"), "{}", b["dsn"]);
    assert!(b["dsn"]["phrase"].as_str().unwrap().contains("B1-208-11 : ordre trop petit"), "{}", b["dsn"]);

    // Le certificat de conformite : testee, et le reel part.
    poser(json!([{"nature": "10", "verdict": "OK"}, {"nature": "11", "verdict": "OK", "anomalies": []}]));
    let b = reussi(&route(&racine, "bilan-du-mois", json!({"mois": "2026-07"}))).clone();
    assert_eq!(b["dsn"]["etape"], json!("testee"), "{}", b["dsn"]);
    reussi(&deposer(&racine, true));
    assert_eq!(derniere(&racine)["mode"], json!("reel"));
}

#[test]
fn l_annule_et_remplace_reel_part_au_dessus_de_l_ordre_que_la_plate_forme_a_vu_ce_mois() {
    // Juillet est parti en reel a l'ordre 1 (journal). Un autre poste de la compagnie a depose ce mois civil (aout) une
    // mensuelle reelle a l'ordre 5 : l'annule-et-remplace produit le 03/08 part a 6, pas a 2.
    let racine = compagnie::ses_dsn("fictive-3090-grande", &["2026-07"], &outils::dossier_de_test("ordre_mensuelles", "reel-recale"));
    std::fs::create_dir_all(racine.join("retours")).unwrap();
    std::fs::write(racine.join("retours/20260802-101010_nature11_FLUX-AUTRE-POSTE.xml"), bilan("FLUX-AUTRE-POSTE", "02", "5", "2026-08-02", None)).unwrap();
    // Le journal sait juillet parti a l'ordre 1 ; une saisie modifiee appelle l'annule-et-remplace.
    let r = route(&racine, "dsn-a-deposer", json!({"mois": "2026-07"}));
    reussi(&r);
    let ar = std::fs::read_dir(racine.join("sorties/2026-07"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("DSN_Juillet2026_AR"))
        .expect("un annule-et-remplace ecrit");
    let t = String::from_utf8_lossy(&std::fs::read(ar).unwrap()).into_owned();
    assert!(t.contains("S20.G00.05.002,'03'") && t.contains("S20.G00.05.004,'6'"), "{}", &t[..t.len().min(1500)]);
}

#[test]
fn un_annule_et_remplace_reel_sous_l_ordre_qu_un_autre_poste_a_fait_accepter_ne_part_pas() {
    // L'annule-et-remplace de juillet s'ecrit a l'ordre 2 (le journal ne sait que l'ordre 1). Puis un autre poste de la
    // compagnie fait accepter, ce mois civil, une mensuelle reelle a l'ordre 5 : le depot reel de l'ordre 2 prendrait
    // B1-208-11 (cahier technique DSN, p. 15). Il est refuse avant de partir, et le refus dit l'ordre a prendre.
    let racine = compagnie::ses_dsn("fictive-3090-grande", &["2026-07"], &outils::dossier_de_test("ordre_mensuelles", "reel-autre-poste"));
    reussi(&route(&racine, "dsn-a-deposer", json!({"mois": "2026-07"})));
    let ar = std::fs::read_dir(racine.join("sorties/2026-07"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("DSN_Juillet2026_AR"))
        .expect("un annule-et-remplace ecrit");
    assert!(String::from_utf8_lossy(&std::fs::read(&ar).unwrap()).contains("S20.G00.05.004,'2'"));
    std::fs::create_dir_all(racine.join("retours")).unwrap();
    std::fs::write(racine.join("retours/20260803-101010_nature11_FLUX-AUTRE-POSTE.xml"), bilan("FLUX-AUTRE-POSTE", "02", "5", "2026-08-03", None)).unwrap();
    let avant = std::fs::read(racine.join("journal/depots.json")).unwrap();
    let (classe, m) = refus(&route(&racine, "deposer", json!({"quoi": "dsn", "fichier": ar.to_string_lossy(), "reel": true})));
    assert_eq!(classe, "OrdreTropPetit", "{}", m);
    assert!(m.contains("l'ordre 5") && m.contains("l'ordre 6") && m.contains("Rien n'est parti"), "{}", m);
    assert_eq!(std::fs::read(racine.join("journal/depots.json")).unwrap(), avant, "rien n'est ecrit");
}
