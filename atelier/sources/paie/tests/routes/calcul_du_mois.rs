//! La paie calculee a cote de la saisie (`paies/AAAA-MM.calcul.json`, `calcul_du_mois`) : relue tant que ses entrees n'ont
//! pas change, recalculee sinon, figee quand le mois est parti. Chaque test monte sa compagnie (la config de la demo et deux
//! mois, sans le journal) ; le code livre se lit tel quel, et ce qu'il pese dans le calcul se prouve par l'entree stockee.

use crate::outils::{copier, demo, dossier_de_test};
use paie::calcul_du_mois::{calculer_suivi, commande, ecrire_calcul};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MOIS: &str = "2026-08";
const AVANT: &str = "2026-07";
const DATE: &str = "2026-10-03";

struct Compagnie {
    racine: PathBuf,
}

fn compagnie(nom: &str) -> Compagnie {
    let racine = dossier_de_test("calcul_du_mois", nom).join("compagnie");
    copier(&demo("demo-3090-grande").join("config"), &racine.join("config"));
    std::fs::create_dir_all(racine.join("paies")).unwrap();
    for m in [AVANT, MOIS] {
        std::fs::copy(demo("demo-3090-grande").join("paies").join(format!("{}.json", m)), racine.join("paies").join(format!("{}.json", m))).unwrap();
    }
    Compagnie { racine }
}

impl Compagnie {
    fn options(&self, date: &str) -> HashMap<String, String> {
        HashMap::from([("racine".to_string(), self.racine.to_string_lossy().into_owned()), ("date".to_string(), date.to_string())])
    }

    fn fichier(&self, mois: &str) -> PathBuf {
        self.racine.join("paies").join(format!("{}.json", mois))
    }

    fn calcul(&self, mois: &str) -> PathBuf {
        self.racine.join("paies").join(format!("{}.calcul.json", mois))
    }

    /// Le calcul frais de la saisie du mois telle qu'elle est sur le disque, sans rien ecrire.
    fn direct(&self) -> Value {
        calculer_suivi(&self.options(DATE), &json!({ "fichier": self.fichier(MOIS).to_str().unwrap() })).unwrap().0
    }

    fn du_mois_le(&self, parti: bool, date: &str) -> Value {
        commande(&self.options(date), &json!({ "mois": MOIS, "parti": parti }).to_string()).unwrap()
    }

    fn du_mois(&self, parti: bool) -> Value {
        self.du_mois_le(parti, DATE)
    }

    /// Le calcul de `MOIS` a-t-il ete (re)ecrit par ce `du_mois` ?
    fn reecrit(&self, parti: bool) -> (Value, bool) {
        let avant = std::fs::read(self.calcul(MOIS)).ok();
        let rendu = self.du_mois(parti);
        (rendu, std::fs::read(self.calcul(MOIS)).ok() != avant)
    }
}

fn lire(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn ecrire(p: &Path, v: &Value) {
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

fn modifier(p: &Path, f: impl FnOnce(&mut Value)) {
    let mut v = lire(p);
    f(&mut v);
    ecrire(p, &v);
}

fn empreinte(c: &Compagnie) -> String {
    lire(&c.calcul(MOIS))["empreinte"].as_str().unwrap().to_string()
}

/// L'entree `cle` du fichier stocke prend `valeur`, et l'empreinte suit la liste, comme si le calcul l'avait lue ainsi.
fn fausser(c: &Compagnie, cle: &str, valeur: &str) {
    modifier(&c.calcul(MOIS), |v| {
        v["entrees"][cle] = json!(valeur);
        let mut t = String::new();
        for (k, x) in v["entrees"].as_object().unwrap() {
            t.push_str(&format!("{}\t{}\n", k, x.as_str().unwrap()));
        }
        v["empreinte"] = json!(paie::fichiers::sha256_hex(t.as_bytes()));
    });
}

#[test]
fn le_premier_appel_calcule_et_ecrit_le_suivant_relit_a_l_identique() {
    let c = compagnie("identique");
    let direct = c.direct();
    assert_eq!(direct["ok"], json!(true));
    let (premier, ecrit) = c.reecrit(false);
    assert!(ecrit, "pas de fichier : il est calcule et ecrit");
    assert_eq!(premier, direct);
    let (second, ecrit) = c.reecrit(false);
    assert!(!ecrit, "entrees inchangees : relu sans rien ecrire");
    assert_eq!(second, direct);
    // La forme du fichier : l'empreinte, les entrees, le resultat tel que rendu.
    let f = lire(&c.calcul(MOIS));
    assert_eq!(f["empreinte"].as_str().unwrap().len(), 64);
    assert!(f["entrees"]["moteur"].as_str().unwrap().starts_with(env!("CARGO_PKG_VERSION")));
    assert!(f["entrees"].as_object().unwrap().contains_key(&format!("fichier racine:paies/{}.json", MOIS)));
    assert!(f["entrees"].as_object().unwrap().contains_key("fichier code:baremes/2026.json"));
    assert_eq!(f["resultat"], direct);
    // La saisie n'a pas bouge.
    assert!(std::fs::read_to_string(c.fichier(MOIS)).unwrap().contains("Montansier"));
}

#[test]
fn chaque_categorie_d_entree_change_l_empreinte_et_le_resultat_egale_le_direct() {
    type Mutation = Box<dyn Fn(&Compagnie)>;
    let cas: Vec<(&str, Mutation)> = vec![
        ("la saisie du mois", Box::new(|c| modifier(&c.fichier(MOIS), |v| v["_origine"] = json!("autre")))),
        ("une fiche salarie", Box::new(|c| modifier(&c.racine.join("config/salaries.json"), |v| v.as_object_mut().unwrap().values_mut().nth(1).unwrap()["ville"] = json!("Ailleurs")))),
        ("la fiche entreprise", Box::new(|c| modifier(&c.racine.join("config/entreprise.json"), |v| v["_note"] = json!("x")))),
        ("les effectifs", Box::new(|c| modifier(&c.racine.join("config/effectifs.json"), |v| v["_note"] = json!("x")))),
        ("la surcouche du bareme", Box::new(|c| modifier(&c.racine.join("config/baremes/2026.json"), |v| v["_note"] = json!("x")))),
        ("un mois precedent", Box::new(|c| modifier(&c.fichier(AVANT), |v| v["_note"] = json!("x")))),
        ("un fichier lu qui apparait", Box::new(|c| ecrire(&c.racine.join("config/rubriques-compagnie.json"), &json!({})))),
        ("un mois qui apparait dans paies/", Box::new(|c| ecrire(&c.fichier("2026-09"), &json!({"mois": "2026-09", "engagements": []})))),
        ("un fichier lu qui disparait", Box::new(|c| std::fs::remove_file(c.racine.join("config/rubriques-actives.json")).unwrap())),
    ];
    for (nom, mutation) in cas {
        let c = compagnie("categories");
        let (avant, _) = c.reecrit(false);
        assert_eq!(avant, c.direct());
        let e0 = empreinte(&c);
        mutation(&c);
        let direct = c.direct();
        let (rendu, ecrit) = c.reecrit(false);
        assert!(ecrit, "{} : le calcul doit etre refait", nom);
        assert_ne!(empreinte(&c), e0, "{} : l'empreinte doit changer", nom);
        assert_eq!(rendu, direct, "{} : le resultat doit egaler le calcul direct", nom);
        let (_, ecrit) = c.reecrit(false);
        assert!(!ecrit, "{} : relu ensuite", nom);
    }
}

/// Le socle du bareme et un catalogue livre sont des entrees : une empreinte stockee qui n'est plus celle du fichier livre
/// fait refaire le calcul, qui reprend celle du fichier.
#[test]
fn les_fichiers_du_code_lus_sont_des_entrees() {
    for rel in ["baremes/2026.json", "minima-conventionnels.json"] {
        let c = compagnie("code");
        c.du_mois(false);
        let cle = format!("fichier code:{}", rel);
        let lue = paie::fichiers::empreinte_fichier(&paie::fichiers::dossier_du_code().join(rel));
        assert_eq!(lire(&c.calcul(MOIS))["entrees"][&cle], json!(lue), "{} : une entree du calcul", rel);
        fausser(&c, &cle, &"0".repeat(64));
        let (rendu, ecrit) = c.reecrit(false);
        assert!(ecrit, "{} : le calcul doit etre refait", rel);
        assert_eq!(lire(&c.calcul(MOIS))["entrees"][&cle], json!(lue), "{}", rel);
        assert_eq!(rendu, c.direct(), "{}", rel);
        let (_, ecrit) = c.reecrit(false);
        assert!(!ecrit, "{} : relu ensuite", rel);
    }
}

#[test]
fn ce_qui_n_est_pas_une_entree_ne_recalcule_pas() {
    let c = compagnie("sans-effet");
    c.du_mois(false);
    std::fs::write(c.racine.join("config/notes.txt"), "bonjour").unwrap();
    std::fs::create_dir_all(c.racine.join("sorties")).unwrap();
    std::fs::write(c.racine.join("sorties/x.pdf"), "x").unwrap();
    std::fs::create_dir_all(c.racine.join("paies/avant")).unwrap();
    std::fs::write(c.racine.join("paies/avant/copie.json"), "{}").unwrap();
    std::fs::write(c.calcul("2026-09"), "{}").unwrap();
    let (rendu, ecrit) = c.reecrit(false);
    assert!(!ecrit);
    assert_eq!(rendu, c.direct());
    // Un fichier livre que ce calcul ne lit pas n'est pas dans ses entrees : le changer ne le referait pas.
    let entrees = lire(&c.calcul(MOIS))["entrees"].clone();
    assert!(entrees.as_object().unwrap().keys().all(|k| !k.contains("casc-svp-base.json")), "{}", entrees);
}

#[test]
fn un_mois_parti_garde_ses_chiffres() {
    let c = compagnie("parti");
    c.du_mois(false);
    let ancien = c.direct();
    modifier(&c.fichier(MOIS), |v| v["engagements"][0]["salaire_mensuel"] = json!(4000.0));
    let nouveau = c.direct();
    assert_ne!(ancien, nouveau);
    let (rendu, ecrit) = c.reecrit(true);
    assert!(!ecrit);
    assert_eq!(rendu, ancien, "parti : les chiffres stockes, meme si l'empreinte a change");
    let (rendu, ecrit) = c.reecrit(false);
    assert!(ecrit);
    assert_eq!(rendu, nouveau);
}

/// Une ligne du journal des depots : une DSN de ce mois-la, dans ce mode.
fn depot_dsn(c: &Compagnie, mois: &str, mode: &str, accepte: bool) {
    std::fs::create_dir_all(c.racine.join("journal")).unwrap();
    let ligne = json!({"quoi": "dsn", "quand": "2026-09-05T10:00:00", "mois": mois, "mode": mode, "accepte": accepte,
                       "fichier": "DSN.dsn", "retours": [], "detail": {}});
    std::fs::write(c.racine.join("journal/depots.json"), json!([ligne]).to_string()).unwrap();
}

#[test]
fn le_mois_est_parti_quand_le_journal_porte_une_dsn_reelle_acceptee() {
    let c = compagnie("parti-du-journal");
    c.du_mois(false);
    // Ni un depot de test, ni un depot refuse, ni la DSN d'un autre mois : le
    // mois n'est pas parti, il se recalcule quand la saisie change.
    let mut salaire = 4000.0;
    for (mois, mode, accepte) in [(MOIS, "test", true), (MOIS, "reel", false), (AVANT, "reel", true)] {
        depot_dsn(&c, mois, mode, accepte);
        salaire += 100.0;
        modifier(&c.fichier(MOIS), |v| v["engagements"][0]["salaire_mensuel"] = json!(salaire));
        let (rendu, ecrit) = c.reecrit(false);
        assert!(ecrit, "{} {} {} : recalcule", mois, mode, accepte);
        assert_eq!(rendu, c.direct());
    }
    // Une DSN reelle acceptee : le mois est parti, sans que la demande le dise ;
    // les chiffres stockes ne bougent plus, meme si la saisie change.
    depot_dsn(&c, MOIS, "reel", true);
    let figes = c.direct();
    modifier(&c.fichier(MOIS), |v| v["engagements"][0]["salaire_mensuel"] = json!(salaire + 100.0));
    assert_ne!(figes, c.direct());
    let stocke = std::fs::read(c.calcul(MOIS)).unwrap();
    let (rendu, ecrit) = c.reecrit(false);
    assert!(!ecrit, "parti d'apres le journal : rien n'est reecrit");
    assert_eq!(std::fs::read(c.calcul(MOIS)).unwrap(), stocke);
    assert_eq!(rendu, figes);
}

#[test]
fn parti_sans_fichier_calcule_sans_ecrire() {
    let c = compagnie("parti-sans-fichier");
    assert_eq!(c.du_mois(true), c.direct());
    assert!(!c.calcul(MOIS).exists());
}

#[test]
fn un_fichier_abime_se_recalcule() {
    let c = compagnie("abime");
    c.du_mois(false);
    std::fs::write(c.calcul(MOIS), "{pas du json").unwrap();
    let (rendu, ecrit) = c.reecrit(false);
    assert!(ecrit);
    assert_eq!(rendu, c.direct());
    modifier(&c.calcul(MOIS), |v| v["empreinte"] = json!("0".repeat(64)));
    let (_, ecrit) = c.reecrit(false);
    assert!(ecrit, "une empreinte qui ne dit pas ce que la liste contient ne vaut rien");
}

#[test]
fn la_version_du_moteur_fait_partie_des_entrees() {
    let c = compagnie("moteur");
    c.du_mois(false);
    fausser(&c, "moteur", "0.0.0+autre");
    let (rendu, ecrit) = c.reecrit(false);
    assert!(ecrit);
    assert_eq!(rendu, c.direct());
}

#[test]
fn la_date_entre_par_les_fiches_telles_qu_elles_etaient() {
    let c = compagnie("date");
    modifier(&c.racine.join("config/salaries.json"), |v| {
        v.as_object_mut().unwrap().values_mut().nth(1).unwrap()["historique"] = json!([{"champ": "ville", "jusqu_au": "2026-10-31", "valeur": "Ancienne"}]);
    });
    let a = c.du_mois_le(false, DATE);
    let f0 = std::fs::read(c.calcul(MOIS)).unwrap();
    c.du_mois_le(false, "2026-10-20");
    assert_eq!(std::fs::read(c.calcul(MOIS)).unwrap(), f0, "deux dates qui lisent les memes fiches : relu");
    c.du_mois_le(false, "2026-12-01");
    assert_ne!(std::fs::read(c.calcul(MOIS)).unwrap(), f0, "une date qui lit d'autres fiches : recalcule");
    assert_eq!(a, c.direct());
}

#[test]
fn une_saisie_sauvee_s_ecrit_avec_le_calcul_qui_l_a_validee() {
    let c = compagnie("ecrire");
    let saisie = lire(&c.fichier(MOIS));
    // Le calcul de la saisie en main, avec ses entrees ; la saisie est ecrite telle quelle (ici elle l'est deja), puis le
    // calcul.
    let (resultat, entrees) = calculer_suivi(&c.options(DATE), &json!({ "saisie": saisie })).unwrap();
    assert!(ecrire_calcul(&c.options(DATE), MOIS, &resultat, entrees));
    let ecrit = std::fs::read(c.calcul(MOIS)).unwrap();
    // Meme octets qu'un calcul fait sur le fichier, et relu sans recalcul.
    let (rendu, reecrit) = c.reecrit(false);
    assert!(!reecrit);
    assert_eq!(rendu, c.direct());
    std::fs::remove_file(c.calcul(MOIS)).unwrap();
    c.du_mois(false);
    assert_eq!(std::fs::read(c.calcul(MOIS)).unwrap(), ecrit);
}

#[test]
fn les_demandes_mal_formees_sont_refusees() {
    let c = compagnie("refus");
    for demande in [json!({}), json!({"mois": "../etc"}), json!({"mois": "2026-8"}), json!({"mois": 202608})] {
        assert!(commande(&c.options(DATE), &demande.to_string()).is_err(), "{}", demande);
    }
    // Un mois sans fichier de saisie : le refus du calcul, et rien d'ecrit.
    assert!(commande(&c.options(DATE), &json!({"mois": "2030-01"}).to_string()).is_err());
    assert!(!c.calcul("2030-01").exists());
}
