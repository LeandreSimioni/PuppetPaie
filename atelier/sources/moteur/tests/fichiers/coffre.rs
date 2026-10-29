//! Le coffre (`moteur::compagnie::coffre`) : poser, relire, oublier un secret, le mot de passe du logiciel, la cle du poste, le scelle,
//! sur des coffres et des mots de passe inventes, chacun dans son dossier (jamais la vraie cle du poste).

use crate::outils::chemin_unique;
use moteur::compagnie::coffre::{d64, desceller, deriver, sceller, Coffre, DEPOT, CONSULTATION};
use socle::json_ascii::dumps;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn dossier() -> PathBuf {
    let d = chemin_unique("coffre");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un poste (sa cle) et une compagnie (sa racine), a eux.
fn coffre() -> Coffre {
    let base = dossier();
    std::fs::create_dir_all(base.join("racine/config")).unwrap();
    Coffre { racine: base.join("racine"), dossier_cle: base.join("poste"), maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() }
}

/// Le meme coffre vu d'un autre poste.
fn autre_poste(c: &Coffre) -> Coffre {
    Coffre { racine: c.racine.clone(), dossier_cle: dossier(), maintenant: c.maintenant.clone(), variables: BTreeMap::new() }
}

fn net() -> Value {
    json!("net-entreprises")
}

fn lire(c: &Coffre, ouverture: Option<&str>) -> Result<Option<String>, String> {
    c.lire(&net(), &json!(CONSULTATION), false, ouverture).map_err(|e| e.message)
}

fn fichier(c: &Coffre) -> Value {
    serde_json::from_slice(&std::fs::read(c.chemin()).unwrap()).unwrap()
}

#[test]
fn un_secret_se_pose_et_se_relit_sans_etre_en_clair() {
    let c = coffre();
    c.poser("urssaf", "UnSecretTresReconnaissable", None).unwrap();
    let octets = std::fs::read(c.chemin()).unwrap();
    assert!(!String::from_utf8_lossy(&octets).contains("UnSecretTresReconnaissable"));
    let r = c.lire(&json!("urssaf"), &json!(CONSULTATION), false, None).unwrap();
    assert_eq!(r.as_deref(), Some("UnSecretTresReconnaissable"));
    assert_eq!(lire(&c, None).unwrap(), None);
    assert!(c.pose("urssaf").unwrap() && !c.pose("courriel").unwrap());
}

#[test]
fn l_etat_ne_rend_jamais_la_valeur() {
    let c = coffre();
    c.poser("urssaf", "MotDePasseTresSecret42", None).unwrap();
    let etat = Value::Array(c.etat().unwrap()).to_string();
    assert!(!etat.contains("MotDePasseTresSecret42"));
    assert!(etat.contains("\"pose_le\":\"2026-10-03 14:05\""));
}

#[test]
fn le_fichier_porte_sa_version_et_le_compte_en_minuscules() {
    let c = coffre();
    c.poser("courriel", "unmotdepasse", Some(" Moi@Exemple.COM ")).unwrap();
    let brut = std::fs::read_to_string(c.chemin()).unwrap();
    let v: Value = serde_json::from_str(&brut).unwrap();
    assert_eq!(v["version"], 2);
    assert_eq!(v["fiches"]["courriel"]["compte"], "moi@exemple.com");
    assert_eq!(c.compte("courriel").unwrap(), "moi@exemple.com");
}

/// Les fiches sont l'AAD du scelle (`dumps(fiches, serre, trie)`) : si leur ecriture change d'un octet, un coffre deja
/// pose ne s'ouvre plus. Cles triees, separateurs serres, tout ce qui n'est pas ASCII echappe.
#[test]
fn l_aad_d_un_coffre_existant_ne_bouge_pas() {
    let fiches = json!({"urssaf": {"pose_le": "2026-10-03 14:05"}, "courriel": {"pose_le": "", "compte": "é😀\"\\", "x": [], "y": {}}});
    assert_eq!(
        dumps(&fiches, None, true, true),
        r#"{"courriel":{"compte":"\u00e9\ud83d\ude00\"\\","pose_le":"","x":[],"y":{}},"urssaf":{"pose_le":"2026-10-03 14:05"}}"#
    );
}

#[test]
fn le_base64_standard_se_lit_et_le_base64_abime_est_refuse() {
    assert_eq!(d64(&json!("aGVsbG8=")).unwrap(), b"hello");
    assert_eq!(d64(&json!("aGVs\nbG8=")).unwrap(), b"hello");
    assert_eq!(d64(&json!("")).unwrap(), b"");
    for abime in [json!("aGVsbG8"), json!("a"), json!(5), json!("é")] {
        assert!(d64(&abime).is_err(), "{}", abime);
    }
}

#[test]
fn un_secret_trop_court_ou_vide_est_refuse_et_rien_n_est_ecrit() {
    let c = coffre();
    for essai in ["", "   ", "a", "abcde"] {
        assert_eq!(c.poser("net-entreprises", essai, None).unwrap_err().classe, "ErreurCoffre", "{:?}", essai);
    }
    assert!(!c.chemin().exists());
    assert!(c.poser("net-entreprises", "abcde", None).unwrap_err().message.starts_with("Mot de passe de 5 caractere(s)"));
    c.poser("net-entreprises", "abcdef", None).unwrap();
    assert!(c.pose("net-entreprises").unwrap());
}

#[test]
fn la_boite_courriel_veut_son_adresse() {
    let c = coffre();
    let m = |compte| c.poser("courriel", "unmotdepasse", compte).unwrap_err().message;
    assert!(m(None).contains("elle manque"));
    assert!(m(Some("sans-arobase")).contains("'sans-arobase' n'en est pas une"));
    assert!(m(Some("a b@c.fr")).contains("n'en est pas une"));
    assert!(c.poser("nimporte", "unmotdepasse", None).unwrap_err().message.contains("nimporte"), "le refus nomme le service");
}

#[test]
fn lire_exige_un_usage_et_un_depot_sa_confirmation() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let m = |u: Value, conf| c.lire(&net(), &u, conf, None).unwrap_err().message;
    assert!(m(json!("autre"), false).starts_with("usage doit valoir 'consultation' ou 'depot' (recu 'autre')"));
    assert!(c.lire(&net(), &Value::Null, false, None).is_err());
    assert!(m(json!(DEPOT), false).starts_with("Un depot ne puise dans le coffre qu'apres confirmation"));
    assert_eq!(c.lire(&net(), &json!(DEPOT), true, None).unwrap().as_deref(), Some("unvraimotdepasse"));
}

#[test]
fn l_environnement_passe_devant_le_coffre() {
    let mut c = coffre();
    c.variables.insert("NET_ENTREPRISES_MOTDEPASSE".into(), "depuis-env".into());
    assert_eq!(lire(&c, None).unwrap().as_deref(), Some("depuis-env"));
    assert!(c.disponible("net-entreprises").unwrap());
    assert!(!c.disponible("urssaf").unwrap());
}

#[test]
fn le_mot_de_passe_du_logiciel_ouvre_l_enveloppe_et_rien_d_autre() {
    let c = coffre();
    assert!(!c.ouverture_exigee().unwrap());
    assert!(!c.verifier_ouverture("sesame-123").unwrap());
    c.poser_ouverture("sesame-123", None).unwrap();
    assert!(c.ouverture_exigee().unwrap());
    assert!(c.verifier_ouverture("sesame-123").unwrap());
    assert!(!c.verifier_ouverture("sesame-123 ").unwrap());
    assert!(!c.verifier_ouverture("").unwrap());
    assert!(!String::from_utf8_lossy(&std::fs::read(c.chemin()).unwrap()).contains("sesame-123"));
    assert!(c.poser_ouverture("12345", None).unwrap_err().message.starts_with("Mot de passe du logiciel trop court : 6 caracteres"));
    assert!(c.oublier_ouverture(None).unwrap());
    assert!(!c.ouverture_exigee().unwrap() && !c.chemin().exists());
}

#[test]
fn un_coffre_apporte_sur_un_autre_poste_s_ouvre_par_le_mot_de_passe() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    c.poser_ouverture("sesame-123", None).unwrap();
    let ailleurs = autre_poste(&c);
    assert!(!ailleurs.ouvre_a_ce_poste().unwrap());
    assert!(lire(&ailleurs, None).unwrap_err().contains("n'a pas ete donne"));
    assert!(lire(&ailleurs, Some("faux-faux")).unwrap_err().contains("ne l'ouvre pas non plus"));
    // le mot de passe ouvre sans rien changer au poste
    assert_eq!(lire(&ailleurs, Some("sesame-123")).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(!ailleurs.chemin_cle().exists());
    // et reposer le mot de passe repose une cle : le poste retrouve son autonomie
    ailleurs.poser_ouverture("sesame-124", Some("sesame-123")).unwrap();
    assert!(ailleurs.ouvre_a_ce_poste().unwrap());
    assert_eq!(lire(&ailleurs, None).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(ailleurs.verifier_ouverture("sesame-124").unwrap());
}

#[test]
fn sans_cle_ni_mot_de_passe_le_coffre_refuse_et_le_dit() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    std::fs::remove_file(c.chemin_cle()).unwrap();
    assert!(lire(&c, None).unwrap_err().contains("et elle est absente"));
    assert!(c.poser("urssaf", "unvraimotdepasse", None).unwrap_err().starts_with_ce_coffre());
}

trait Phrase {
    fn starts_with_ce_coffre(&self) -> bool;
}

impl Phrase for moteur::compagnie::coffre::Echec {
    fn starts_with_ce_coffre(&self) -> bool {
        self.message.starts_with("Ce coffre ne s'ouvre pas ici")
    }
}

#[test]
fn les_fiches_scellent_les_secrets_avec_elles() {
    let c = coffre();
    c.poser("courriel", "unmotdepasse", Some("a@b.fr")).unwrap();
    let mut v = fichier(&c);
    v["fiches"]["courriel"]["compte"] = json!("pirate@x.fr");
    std::fs::write(c.chemin(), dumps(&v, Some(2), false, true)).unwrap();
    let e = c.lire(&json!("courriel"), &json!(CONSULTATION), false, None).unwrap_err();
    assert!(e.message.starts_with("Le scelle refuse de s'ouvrir"));
}

#[test]
fn un_fichier_altere_ou_d_une_autre_version_est_refuse() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let bon = std::fs::read_to_string(c.chemin()).unwrap();
    let mut v: Value = serde_json::from_str(&bon).unwrap();
    let t = v["secrets"].as_str().unwrap().to_string();
    v["secrets"] = json!(format!("{}{}", if t.starts_with('A') { "B" } else { "A" }, &t[1..]));
    std::fs::write(c.chemin(), dumps(&v, Some(2), false, true)).unwrap();
    assert!(lire(&c, None).unwrap_err().starts_with("Le scelle refuse"));
    for contenu in ["", "{}", "[]", "pas du json", "{\"version\": 1}"] {
        std::fs::write(c.chemin(), contenu).unwrap();
        let e = lire(&c, None).unwrap_err();
        assert!(e.contains("n'est pas au format de cette version"), "{:?}", contenu);
        assert!(!c.ouverture_exigee().unwrap());
        assert!(c.etat().unwrap().iter().all(|s| s["pose"] == false));
    }
}

#[test]
fn le_coffre_se_vide_quand_il_ne_reste_rien() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    assert!(!c.oublier("urssaf").unwrap());
    assert!(c.oublier("net-entreprises").unwrap());
    assert!(!c.chemin().exists());
    c.poser_ouverture("sesame-123", None).unwrap();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    assert!(c.oublier("net-entreprises").unwrap());
    assert!(c.chemin().exists(), "le mot de passe du logiciel reste");
}

#[test]
fn proteger_lie_le_contenu_a_sa_description_et_a_la_cle_du_poste() {
    let c = coffre();
    let s = c.proteger(b"jeton", "jeton google").unwrap();
    assert_eq!(c.deproteger(&s, "jeton google").unwrap(), b"jeton");
    assert!(c.deproteger(&s, "cle de signature").unwrap_err().message.starts_with("Le scelle refuse"));
    let ailleurs = autre_poste(&c);
    assert!(ailleurs.deproteger(&s, "jeton google").unwrap_err().message.starts_with("La cle de ce poste est absente"));
    std::fs::write(c.chemin_cle(), [0u8; 31]).unwrap();
    assert!(c.deproteger(&s, "jeton google").unwrap_err().message.contains("fait 31 octets au lieu de 32"));
}

#[test]
fn la_cle_du_poste_se_pose_en_0600_sans_ecraser() {
    let c = coffre();
    let p = c.poser_cle_poste().unwrap();
    let cle = std::fs::read(&p).unwrap();
    assert_eq!(cle.len(), 32);
    c.poser_cle_poste().unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), cle);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&c.dossier_cle).unwrap().permissions().mode() & 0o777, 0o700);
    }
}

#[test]
fn un_scelle_tronque_ou_altere_ne_s_ouvre_pas() {
    let cle = [7u8; 32];
    let s = sceller(&cle, b"clair", b"aad").unwrap();
    assert_eq!(desceller(&cle, &s, b"aad").unwrap(), b"clair");
    assert_eq!(desceller(&cle, &s, b"autre").unwrap_err().classe, "ErreurCoffre");
    assert_eq!(desceller(&cle, &s[..12], b"aad").unwrap_err().classe, "ErreurCoffre");
    let mut t = s.clone();
    *t.last_mut().unwrap() ^= 1;
    assert!(desceller(&cle, &t, b"aad").is_err());
    assert_ne!(sceller(&cle, b"clair", b"aad").unwrap(), s, "un nonce neuf a chaque scellage");
}

#[test]
fn scrypt_est_celui_de_la_rfc_7914_et_refuse_les_parametres_d_un_fichier_piege() {
    let kdf = json!({"algo": "scrypt", "n": 1024, "r": 8, "p": 16, "sel": "TmFDbA=="});
    let cle = deriver("password", Some(&kdf)).unwrap();
    assert_eq!(cle.iter().map(|b| format!("{:02x}", b)).collect::<String>(), "fdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162");
    let refus = |k: Value| deriver("x", Some(&k)).unwrap_err();
    let base = |n: i64, r: i64, p: i64| json!({"algo": "scrypt", "n": n, "r": r, "p": p, "sel": "AAAA"});
    // n qui n'est pas une puissance de 2, r nul, une memoire demesuree, un negatif
    for (n, r, p) in [(3, 8, 1), (2, 0, 1), (131072, 8, 1), (-4, 8, 1)] {
        assert!(deriver("x", Some(&base(n, r, p))).is_err(), "n={} r={} p={}", n, r, p);
    }
    assert!(refus(json!({"algo": "argon2"})).message.contains("argon2"), "le refus nomme l'algorithme");
    assert_eq!(deriver("x", None).unwrap_err().message, "Ce coffre dit avoir ete ferme par 'rien', que cette version ne sait pas ouvrir.");
}

#[test]
fn l_ouverture_gardee_vaut_pour_son_coffre_et_expire() {
    let c = coffre();
    c.poser_ouverture("sesame-123", None).unwrap();
    assert!(c.garder_ouverture("faux-faux", "2026-10-04T02:05").unwrap_err().message.starts_with("Mot de passe du logiciel refuse"));
    assert_eq!(c.ouverture_gardee().unwrap(), None);
    c.garder_ouverture("sesame-123", "2026-10-04T02:05").unwrap();
    assert!(!String::from_utf8_lossy(&std::fs::read(c.chemin_gardee()).unwrap()).contains("sesame-123"));
    assert_eq!(c.ouverture_gardee().unwrap().as_deref(), Some("sesame-123"));
    assert_eq!(c.fin_ouverture_gardee().unwrap().as_deref(), Some("2026-10-04T02:05"));
    // un autre coffre, meme poste : rien
    let autre = Coffre { racine: dossier().join("autre"), dossier_cle: c.dossier_cle.clone(), maintenant: c.maintenant.clone(), variables: BTreeMap::new() };
    assert_eq!(autre.ouverture_gardee().unwrap(), None);
    assert!(c.chemin_gardee().exists());
    // expiree : elle s'efface
    let tard = Coffre { racine: c.racine.clone(), dossier_cle: c.dossier_cle.clone(), maintenant: "2026-10-04T02:06".into(), variables: BTreeMap::new() };
    assert_eq!(tard.fin_ouverture_gardee().unwrap(), None);
    assert_eq!(tard.ouverture_gardee().unwrap(), None);
    assert!(!c.chemin_gardee().exists());
}

#[test]
fn le_mot_de_passe_tape_pour_la_commande_ouvre_le_coffre_d_un_autre_poste() {
    // Leandre, 07/10/2026 : le mot de passe du logiciel tape ouvre tout, ligne de commande comprise.
    let c = coffre();
    c.poser_ouverture("sesame-123", None).unwrap();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let ailleurs = autre_poste(&c);
    // sans rien de tape, la cle d'ailleurs n'ouvre pas
    assert!(lire(&ailleurs, None).is_err());
    // tape pour cette commande (le fil de ce test seulement), il ouvre, sans rien changer au poste
    moteur::compagnie::coffre::retenir_mot_de_passe_tape("sesame-123");
    assert_eq!(lire(&ailleurs, None).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(!ailleurs.chemin_cle().exists());
    // et la cle du poste, elle, n'est pas reputee ouvrir
    assert!(!ailleurs.ouvre_a_ce_poste().unwrap());
    // un mauvais mot de passe tape n'ouvre pas
    moteur::compagnie::coffre::retenir_mot_de_passe_tape("pas-le-bon");
    assert!(lire(&ailleurs, None).is_err());
}

/// Reste 158 : deux commandes qui posent la cle du poste en meme temps (deux tests, ou l'interface et la ligne de
/// commande au premier lancement) ne se font pas echouer l'une l'autre sur « File exists », et ne scellent pas avec
/// deux cles differentes : la seconde lit celle que la premiere a posee.
#[test]
fn la_cle_du_poste_posee_par_plusieurs_a_la_fois_est_une_seule() {
    let c = coffre();
    let n = 16;
    let barriere = std::sync::Arc::new(std::sync::Barrier::new(n));
    let fils: Vec<_> = (0..n)
        .map(|i| {
            let (racine, poste, b) = (c.racine.clone(), c.dossier_cle.clone(), barriere.clone());
            std::thread::spawn(move || {
                let k = Coffre { racine, dossier_cle: poste, maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() };
                b.wait();
                k.proteger(format!("secret {}", i).as_bytes(), "essai").map_err(|e| format!("{:?}", e))
            })
        })
        .collect();
    let scelles: Vec<_> = fils.into_iter().map(|f| f.join().unwrap()).collect();
    for (i, s) in scelles.into_iter().enumerate() {
        let s = s.unwrap_or_else(|e| panic!("fil {} : {}", i, e));
        assert_eq!(c.deproteger(&s, "essai").unwrap(), format!("secret {}", i).into_bytes(), "fil {}", i);
    }
    assert_eq!(std::fs::read(c.chemin_cle()).unwrap().len(), 32);
}
