//! `avenant` : le texte qu'on compare (`nfkc` contre 174 paires de `unicodedata.normalize`, `donnees/nfkc.json` ;
//! `normaliser`), le brut et l'emploi que le contrat ecrit. La compagnie est celle de la compagnie fictive (inventee).

use crate::outils;
use paie::avenant::{normaliser, valeurs_du_contrat};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::nfkc::nfkc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

// ------------------------------------------------------------ le texte qu'on compare

fn fixture() -> Vec<(String, String)> {
    let f = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/nfkc.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
    v.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect()
}

#[test]
fn nfkc_rend_la_forme_de_chaque_paire_relevee() {
    let cas = fixture();
    assert!(cas.len() > 150);
    for (entree, attendu) in cas {
        assert_eq!(nfkc(&entree), attendu, "{:?}", entree);
    }
}

#[test]
fn normaliser_defait_les_ligatures_recolle_les_coupures_et_ramene_les_blancs() {
    assert_eq!(normaliser("  Le  contrat\u{a0}n°\u{202f}1\n\tﬁn  "), "Le contrat n° 1 fin");
    // Une coupure apres un trait d'union est recollee ; avant un blanc, elle reste un blanc.
    assert_eq!(normaliser("porte-\nparole"), "porte-parole");
    assert_eq!(normaliser("tiret -\n suivi"), "tiret - suivi");
    assert_eq!(normaliser("a-\n"), "a-");
}


// ------------------------------------------------------------ la compagnie inventee

const MOIS: &str = "2026-03";
const DEBUT: &str = "2026-03-17";
const AUJOURDHUI: &str = "2026-03-20";

fn saisie(taux: f64, lieu: &str) -> Value {
    json!({
        "mois": MOIS, "taux_cachet": 180.0, "date_paiement": "2026-03-31", "numeros_figes": true,
        "engagements": [{
            "salarie": "Biancolelli", "spectacle": "266Z00910022", "debut": DEBUT, "fin": "2026-03-20",
            "date_paiement": "2026-03-20", "numero": "BI4.26.03",
            "lignes": [
                {"rubrique": "cachet", "quantite": 4, "taux": taux, "lieu": lieu,
                 "dates": ["2026-03-17", "2026-03-18", "2026-03-19", "2026-03-20"]},
                {"rubrique": "defraiement_forfait", "quantite": 4, "taux": 20.7}
            ]
        }]
    })
}

struct Monde {
    racine: PathBuf,
    cle: String,
}

impl Monde {
    fn ecrire_saisie(&self, v: &Value) {
        crate::outils::poser_saisie(&self.racine, MOIS, v);
    }

    fn contexte(&self) -> Contexte {
        Contexte::nouveau(&self.racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap()
    }
}

/// La configuration de la compagnie fictive (inventee), sans paie.
fn configuration(test: &str) -> PathBuf {
    let racine = outils::dossier_de_test("avenant", test);
    let fictive = outils::fictive("fictive-3090-grande");
    outils::copier(&fictive.join("config"), &racine.join("config"));
    std::fs::copy(fictive.join("COPIE-DE-DEVELOPPEMENT.json"), racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    std::fs::create_dir_all(racine.join("engagements")).unwrap();
    racine
}

/// Une compagnie : la configuration de la compagnie fictive (inventee), et un contrat de mars.
fn monde(nom: &str) -> Monde {
    let racine = configuration(nom);
    let mut m = Monde { racine, cle: String::new() };
    m.ecrire_saisie(&saisie(185.0, "Scene fictive du Mans"));
    m.cle = m.contexte().salarie("Biancolelli").unwrap()["_cle"].as_str().unwrap().to_string();
    m
}

// ------------------------------------------------------------ le brut du contrat

/// Le paragraphe « Rémunération » tel que le contrat d'aujourd'hui l'ecrit.
fn paragraphe_remuneration(m: &Monde) -> (fabrique_pdf::modele::Valeurs, String) {
    let ctx = m.contexte();
    let valeurs = valeurs_du_contrat(&ctx, MOIS, &m.cle, DEBUT).unwrap().unwrap();
    let modele = fabrique_pdf::modele::lire_fichier(
        socle::fichiers::dossier_du_code().join("modeles").join("contrat-cddu.modele").to_str().unwrap(),
    )
    .unwrap();
    let texte = fabrique_pdf::modele::texte_brut(&modele.articles[3].1[0], &valeurs).unwrap();
    (valeurs, texte)
}

#[test]
fn le_brut_du_contrat_ne_cite_que_ce_qui_fait_le_brut() {
    // 4 cachets a 185 et 4 defraiements au forfait a 20,70 : le brut est de 740,00, les
    // defraiements n'en sont pas. Le contrat les cite, a part, pour ce qu'ils sont.
    let m = monde("hors-brut");
    let (v, texte) = paragraphe_remuneration(&m);
    assert_eq!(v["remuneration.detail"], "4 cachets à 185,00 €");
    assert_eq!(v["remuneration.brut"], "740,00");
    assert_eq!(v["remuneration.hors_brut"], ", auxquels s'ajoutent 4 défraiements au forfait à 20,70 € hors brut");
    assert!(
        texte.starts_with(
            "L'artiste perçoit 4 cachets à 185,00 € brut, soit une rémunération brute totale de 740,00 €, \
             auxquels s'ajoutent 4 défraiements au forfait à 20,70 € hors brut, dont le net à payer estimé est de "
        ),
        "{}",
        texte
    );
    // Sans defraiement, la phrase est celle d'avant, mot pour mot : ni morceau vide, ni virgule orpheline.
    let mut s = saisie(185.0, "Scene fictive du Mans");
    s["engagements"][0]["lignes"].as_array_mut().unwrap().pop();
    m.ecrire_saisie(&s);
    let (v, texte) = paragraphe_remuneration(&m);
    assert_eq!(v["remuneration.hors_brut"], "");
    assert!(
        texte.starts_with("L'artiste perçoit 4 cachets à 185,00 € brut, soit une rémunération brute totale de 740,00 €, dont le net à payer estimé est de "),
        "{}",
        texte
    );
}

// ------------------------------------------------------------ l'emploi au contrat

/// « En qualite de ... » : l'emploi que le contrat ecrit pour `salarie` engage sur `metier`.
fn emploi_au_contrat(nom: &str, salarie: &str, metier: &str) -> (String, String) {
    let racine = configuration(&format!("emploi-{}", nom));
    let mut s = saisie(185.0, "Scene fictive du Mans");
    s["engagements"][0]["salarie"] = json!(salarie);
    s["engagements"][0]["metier"] = json!(metier);
    crate::outils::poser_saisie(&racine, MOIS, &s);
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap();
    let cle = ctx.salarie(salarie).unwrap()["_cle"].as_str().unwrap().to_string();
    let v = valeurs_du_contrat(&ctx, MOIS, &cle, DEBUT).unwrap().expect("la paie de ce contrat");
    (v["salarie.emploi"].clone(), v["salarie.qualite_de"].clone())
}

#[test]
fn le_contrat_ecrit_l_emploi_de_l_engagement_au_feminin_si_la_fiche_le_dit() {
    // Clairon est comedienne sur sa fiche (COM010) ; engagee comme danseuse (DAN010).
    assert_eq!(emploi_au_contrat("clairon-dan", "Clairon", "DAN010"), ("danseuse".to_string(), "de ".to_string()));
    // Sur le metier de sa fiche : la fiche le dit deja.
    assert_eq!(emploi_au_contrat("clairon-com", "Clairon", "COM010"), ("comédienne".to_string(), "de ".to_string()));
    // Biancolelli, comedien, engage comme danseur.
    assert_eq!(emploi_au_contrat("bianco-dan", "Biancolelli", "DAN010"), ("danseur".to_string(), "de ".to_string()));
}

