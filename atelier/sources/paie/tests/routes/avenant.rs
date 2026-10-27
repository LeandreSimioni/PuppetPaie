//! `avenant` : l'ecart du contrat parti a la paie d'aujourd'hui (`ecart_au_contrat_signe`), le texte qu'on compare
//! (`nfkc` contre 174 paires de `unicodedata.normalize`, `donnees/nfkc.json` ; `normaliser`, `motif`), le brut et
//! l'emploi que le contrat ecrit. La compagnie est celle de la compagnie fictive (inventee).

use crate::outils;
use paie::avenant::{ecart_au_contrat_signe, motif, normaliser, retrouve, valeurs_du_contrat};
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

#[test]
fn le_motif_d_un_paragraphe_laisse_libres_ses_marqueurs() {
    let bloc = "Le paiement intervient par virement le {{contrat.date_paiement}}. Les congés payés sont réglés par la caisse sous le numéro {{employeur.congesspectacles}}.";
    let seg: Vec<char> = "Article 4 — Rémunération L'artiste perçoit X. Le paiement intervient par virement le 20/03/2026. Les congés payés sont réglés par la caisse sous le numéro ZZ-1. Article 5 — Suite".chars().collect();
    assert!(retrouve(&motif(bloc), &seg));
    // Le texte fixe doit y etre tel quel.
    let autre: Vec<char> = "Le paiement intervient par chèque le 20/03/2026. Les congés payés sont réglés par la caisse sous le numéro ZZ-1.".chars().collect();
    assert!(!retrouve(&motif(bloc), &autre));
    // Un dernier marqueur qui acheve le paragraphe s'arrete au titre suivant, a « Fait en » ou a la fin.
    let fin = "Retraite complémentaire : {{employeur.congesspectacles}}";
    for ok in ["Retraite complémentaire : AUDIENS Article 5 — Suite", "Retraite complémentaire : AUDIENS Fait en deux", "Retraite complémentaire : AUDIENS"] {
        let c: Vec<char> = ok.chars().collect();
        assert!(retrouve(&motif(fin), &c), "{}", ok);
    }
    // Les espaces autour d'un marqueur sont facultatifs, pas ceux du texte fixe.
    let sans_espace: Vec<char> = "Retraite complémentaire :AUDIENS".chars().collect();
    assert!(retrouve(&motif(fin), &sans_espace));
    let trop_court: Vec<char> = "Retraite complementaire : AUDIENS".chars().collect();
    assert!(!retrouve(&motif(fin), &trop_court));
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
        std::fs::write(self.racine.join("engagements").join(format!("{}.json", MOIS)), serde_json::to_string_pretty(v).unwrap()).unwrap();
    }

    fn journal(&self, lignes: Vec<Value>) {
        std::fs::create_dir_all(self.racine.join("journal")).unwrap();
        std::fs::write(self.racine.join("journal").join("depots.json"), serde_json::to_string_pretty(&lignes).unwrap()).unwrap();
    }

    fn contexte(&self) -> Contexte {
        Contexte::nouveau(&self.racine, Some(Jour::lire(AUJOURDHUI).unwrap())).unwrap()
    }

    fn pdf(&self) -> PathBuf {
        self.racine.join("sorties").join(MOIS).join("envoi").join("Contrat_BIANCOLELLI_BI4.26.03_Mars2026.pdf")
    }

    /// L'ecart, ce jour-la : les articles nommes.
    fn ecart(&self, aujourdhui: &str, mois: &str) -> String {
        let mut ctx = self.contexte();
        ctx.aujourdhui = Jour::lire(aujourdhui).unwrap();
        ecart_au_contrat_signe(&ctx, mois, &self.cle, DEBUT)
    }

    fn ligne(&self, quoi: &str, quand: &str, mois: &str, detail: Value) -> Value {
        json!({"quoi": quoi, "quand": quand, "origine": "", "mois": mois, "fichier": "x", "empreinte": "e",
               "mode": "reel", "qui": "?", "accepte": true, "destinataire": "", "flux": "", "retours": [],
               "reponse": "", "trace": null, "detail": detail})
    }

    fn ligne_parti(&self, mois: &str) -> Value {
        self.ligne(
            "contrat-signature",
            "2026-03-10T10:00:00",
            mois,
            json!({"contrats": [{"salarie": self.cle, "debut": DEBUT, "numero": "BI4.26.03",
                                 "fichier": self.pdf().file_name().unwrap().to_str().unwrap(), "mois": mois}]}),
        )
    }

    fn ligne_avenant(&self, quand: &str, modifications: Value) -> Value {
        self.ligne(
            "contrat-signature",
            quand,
            MOIS,
            json!({"contrats": [{"salarie": self.cle, "debut": DEBUT, "numero": "BI4.26.03", "acte": "avenant",
                                 "avenant": 1, "fichier": "Avenant1.pdf", "mois": MOIS, "modifications": modifications}]}),
        )
    }
}

/// Une compagnie : la configuration de la compagnie fictive (inventee), un contrat de mars parti a la
/// signature, ecrit par la fabrique avec ce que la paie disait ce jour-la.
/// La configuration de la compagnie fictive (inventee), sans paie.
fn configuration(test: &str) -> PathBuf {
    let racine = outils::dossier_de_test("avenant", test);
    let fictive = outils::fictive("fictive-3090-grande");
    outils::copier(&fictive.join("config"), &racine.join("config"));
    std::fs::copy(fictive.join("COPIE-DE-DEVELOPPEMENT.json"), racine.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    std::fs::create_dir_all(racine.join("engagements")).unwrap();
    racine
}

fn monde(nom: &str) -> Monde {
    let racine = configuration(nom);
    let mut m = Monde { racine, cle: String::new() };
    m.ecrire_saisie(&saisie(185.0, "Scene fictive du Mans"));
    let ctx = m.contexte();
    m.cle = ctx.salarie("Biancolelli").unwrap()["_cle"].as_str().unwrap().to_string();
    let valeurs = valeurs_du_contrat(&ctx, MOIS, &m.cle, DEBUT).unwrap().expect("la paie de ce contrat");
    let modele = socle::fichiers::dossier_du_code().join("modeles").join("contrat-cddu.modele");
    let description = json!({
        "format": fabrique_pdf::FORMAT, "style": "contrat",
        "documents": [{"modele": {"chemin": modele.to_str().unwrap(), "valeurs": valeurs}}]
    });
    paie::fabrique::ecrire(&description, &m.pdf()).unwrap();
    let parti = m.ligne_parti(MOIS);
    m.journal(vec![parti]);
    m
}

#[test]
fn rien_n_a_change_rien_a_dire() {
    let m = monde("rien");
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
}

#[test]
fn ce_que_la_paie_a_change_depuis_l_envoi_se_nomme_article_par_article() {
    let m = monde("change");
    m.ecrire_saisie(&saisie(190.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 4 (Rémunération)");
    m.ecrire_saisie(&saisie(190.0, "Theatre fictif de Vendome"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 3 (Lieu de travail), article 4 (Rémunération)");
    // Remis comme au depart : plus rien a dire.
    m.ecrire_saisie(&saisie(185.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
    // Les dates : le jour d'une date change, l'article « Durée » le dit.
    let mut s = saisie(185.0, "Scene fictive du Mans");
    s["engagements"][0]["lignes"][0]["dates"] = json!(["2026-03-17", "2026-03-18", "2026-03-19", "2026-03-21"]);
    s["engagements"][0]["fin"] = json!("2026-03-21");
    m.ecrire_saisie(&s);
    let ecart = m.ecart(AUJOURDHUI, MOIS);
    assert!(ecart.starts_with("article 2 (Durée)"), "{}", ecart);
}

#[test]
fn un_avenant_envoye_compte_deja_signe_ou_non() {
    let m = monde("avenant");
    m.ecrire_saisie(&saisie(190.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 4 (Rémunération)");
    // L'avenant 1 a ecrit le nouveau texte du premier paragraphe de l'article 4 : la paie le dit deja.
    let ctx = m.contexte();
    let valeurs = valeurs_du_contrat(&ctx, MOIS, &m.cle, DEBUT).unwrap().unwrap();
    let modele = fabrique_pdf::modele::lire_fichier(
        socle::fichiers::dossier_du_code().join("modeles").join("contrat-cddu.modele").to_str().unwrap(),
    )
    .unwrap();
    let neuf = fabrique_pdf::modele::texte_brut(&modele.articles[3].1[0], &valeurs).unwrap();
    let modifs = json!([{"numero": 4, "article": "Rémunération", "blocs": [0], "apres": [neuf]}]);
    let mut lignes = vec![m.ligne_parti(MOIS), m.ligne_avenant("2026-03-12T10:00:00", modifs.clone())];
    m.journal(lignes.clone());
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "", "envoye, pas signe : il attend sa signature, pas un autre avenant");
    // Signe : meme chose.
    lignes.push(m.ligne("contrat-recu", "2026-03-13T09:00:00", MOIS, json!({"salarie": m.cle, "debut": DEBUT, "recu": true})));
    m.journal(lignes.clone());
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
    // La paie change encore apres l'avenant : l'ecart est a l'avenant, et il le dit.
    m.ecrire_saisie(&saisie(195.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 4 (Rémunération)");
    // Un avenant d'avant l'envoi du contrat n'est pas le sien (un contrat remplace).
    let vieux = m.ligne_avenant("2026-03-01T10:00:00", modifs);
    m.journal(vec![vieux, m.ligne_parti(MOIS)]);
    m.ecrire_saisie(&saisie(190.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 4 (Rémunération)");
}

#[test]
fn sans_calcul_tant_qu_on_peut() {
    let m = monde("sans-calcul");
    m.ecrire_saisie(&saisie(190.0, "Scene fictive du Mans"));
    // Un mois dont la DSN etait due : on ne regarde rien.
    assert_eq!(m.ecart("2026-04-20", MOIS), "");
    // Rien de parti ce mois-la.
    let autre = m.ligne_parti("2026-02");
    m.journal(vec![autre]);
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
    // Rien du tout au journal.
    m.journal(vec![]);
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
    // Le PDF n'est plus la.
    let parti = m.ligne_parti(MOIS);
    m.journal(vec![parti]);
    std::fs::remove_file(m.pdf()).unwrap();
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
}

#[test]
fn le_modele_a_change_depuis_l_envoi_la_pastille_n_invente_pas() {
    let m = monde("modele");
    m.ecrire_saisie(&saisie(190.0, "Scene fictive du Mans"));
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "article 4 (Rémunération)");
    // La compagnie reecrit une clause fixe de l'article 2 : l'ancien texte ne se retrouve plus.
    let livre = std::fs::read_to_string(socle::fichiers::dossier_du_code().join("modeles").join("contrat-cddu.modele")).unwrap();
    assert!(livre.contains("sans qu'il soit besoin d'un préavis"));
    std::fs::create_dir_all(m.racine.join("gabarits")).unwrap();
    std::fs::write(
        m.racine.join("gabarits").join("contrat-cddu.modele"),
        livre.replace("sans qu'il soit besoin d'un préavis", "sans qu'il soit besoin d'aucun préavis"),
    )
    .unwrap();
    assert_eq!(m.ecart(AUJOURDHUI, MOIS), "");
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
    std::fs::write(racine.join("engagements").join(format!("{}.json", MOIS)), s.to_string()).unwrap();
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

/// LE TEXTE GARDE D'UN CONTRAT NE DEPEND QUE DE SON CONTENU (#236, banc du Petit Castelet : `.textes/Contrat_….pdf.<date a
/// la nanoseconde>.<taille>.txt`, un nom qu'aucun rejeu ne retrouve, et le rejeu ne refaisait pas la compagnie fichier pour
/// fichier). Deux copies du meme PDF, a deux dates, rangent leur texte sous le meme nom ; un PDF refait, sous un autre.
#[test]
fn le_texte_garde_d_un_contrat_porte_l_empreinte_de_son_contenu() {
    let pdf = outils::fictive("fictive-3090-petite").join("sorties/2026-08/Contrats_Aout2026.pdf");
    let noms = |cas: &str, quand: u64| -> Vec<String> {
        let d = outils::dossier_de_test("avenant", cas);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let copie = d.join("Contrats_Aout2026.pdf");
        std::fs::copy(&pdf, &copie).unwrap();
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(quand);
        std::fs::File::options().write(true).open(&copie).unwrap().set_modified(t).unwrap();
        paie::avenant::texte_du_pdf(&copie).unwrap();
        let mut n: Vec<String> = std::fs::read_dir(d.join(".textes")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        n.sort();
        n
    };
    let (a, b) = (noms("texte-garde-a", 1_577_836_800), noms("texte-garde-b", 1_798_761_600));
    assert_eq!(a.len(), 1, "{:?}", a);
    assert_eq!(a, b, "le meme contrat, deux dates : le meme texte garde");
}
