//! `coffre-verifier` : le secret pose ouvre-t-il une session chez l'organisme ? UNE tentative, meme en echec (net-entreprises
//! bloque le compte au-dela de quelques echecs) ; le bon secret au bon service, jamais dans la reponse ; l'environnement
//! passe devant le coffre. AUCUN RESEAU, AUCUN VRAI SECRET : `verifier_secret_avec` recoit des faux qui comptent leurs appels.
//!
//! Seul dans `tests/` : il pose des mots de passe FICTIFS d'organisme dans l'environnement du processus, c'est ce qu'il
//! prouve ; les autres variables des services sont retirees.

#[path = "outils/mod.rs"]
mod outils;

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, Once};

use paie::coffre::Coffre;
use paie::coffre_verifier::{self, verifier_secret_avec};
use droit::contexte::Contexte;
use paie::courriel::ErreurCourriel;
use socle::date::Jour;
use paie::releve_retours::Panne;
use serde_json::{json, Value};

static VERROU: Mutex<()> = Mutex::new(());
static UNE_FOIS: Once = Once::new();
const SERVICES: [&str; 3] = ["net-entreprises", "urssaf", "courriel"];

fn tour() -> MutexGuard<'static, ()> {
    let g = VERROU.lock().unwrap_or_else(|e| e.into_inner());
    UNE_FOIS.call_once(|| {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", "2026-10-03");
        std::env::set_var("PUPPETPAIE_CLE", dossier_poste());
    });
    for v in ["COURRIEL_COMPTE", "COURRIEL_MOTDEPASSE", "NET_ENTREPRISES_MOTDEPASSE", "URSSAF_MOTDEPASSE", "PUPPETPAIE_INCIDENTS"] {
        std::env::remove_var(v);
    }
    g
}

fn dossier_poste() -> PathBuf {
    outils::dossier_du_processus("coffre-verifier-poste")
}

fn compagnie(test: &str) -> Contexte {
    let racine = outils::compagnie::config_et_mois("fictive-3090-petite", &[], &outils::dossier_de_test("coffre_verifier", test));
    Contexte::nouveau(&racine, Some(Jour::lire("2026-10-03").unwrap())).unwrap()
}

/// Pose un secret FICTIF au coffre de la copie (la boite : avec son adresse).
fn poser(ctx: &Contexte, service: &str, secret: &str) {
    let c = Coffre::du_poste(&ctx.racine, "2026-10-03T10:00");
    c.poser(service, secret, if service == "courriel" { Some("boite@exemple.invalid") } else { None }).unwrap();
}

fn api(message: &str) -> Panne {
    Panne { classe: "ErreurApi".into(), message: message.into(), code: 0 }
}

/// Un authentifier qui compte ses appels, garde le secret recu, et rend `resultat`.
struct Compte {
    appels: usize,
    recu: Vec<(String, Option<String>)>,
}

fn verifier(ctx: &Contexte, service: Option<&Value>, resultat: Result<(), Panne>, boite: Result<Value, ErreurCourriel>) -> (Result<Value, (String, String)>, Compte, usize) {
    let mut compte = Compte { appels: 0, recu: Vec::new() };
    let mut ouvertures = 0;
    let r = verifier_secret_avec(
        ctx,
        service,
        &mut |_, s, mdp| {
            compte.appels += 1;
            compte.recu.push((s.to_string(), mdp));
            match &resultat {
                Ok(()) => Ok(()),
                Err(p) => Err(Panne { classe: p.classe.clone(), message: p.message.clone(), code: p.code }),
            }
        },
        &mut |_| {
            ouvertures += 1;
            boite.clone()
        },
    )
    .map_err(|e| (e.classe.unwrap_or("").to_string(), e.message));
    (r, compte, ouvertures)
}

fn dit(s: &str) -> Option<Value> {
    Some(json!(s))
}

fn refus(r: Result<Value, (String, String)>) -> (String, String) {
    r.expect_err("un refus etait attendu")
}

#[test]
fn un_service_inconnu_est_refuse_sans_rien_tenter() {
    let _t = tour();
    let ctx = compagnie("inconnu");
    let illisibles = [None, Some(json!(12)), Some(json!(true)), Some(json!(["urssaf"])), Some(json!({"a": 1}))];
    for service in [dit("xxx"), dit(""), dit("NET-ENTREPRISES"), dit("l'urssaf")].into_iter().chain(illisibles) {
        let (r, compte, ouvertures) = verifier(&ctx, service.as_ref(), Ok(()), Ok(json!({})));
        let (_, message) = refus(r);
        assert!(message.ends_with("Connus : net-entreprises, urssaf, courriel."), "{}", message);
        if let Some(Value::String(s)) = &service {
            assert!(message.starts_with(&format!("Service inconnu : « {} ».", s)), "{}", message);
        }
        assert_eq!((compte.appels, ouvertures), (0, 0), "{:?} : rien n'est tente", service);
    }
}

#[test]
fn rien_de_pose_rien_a_verifier() {
    let _t = tour();
    let ctx = compagnie("rien-pose");
    for s in SERVICES {
        let (r, compte, ouvertures) = verifier(&ctx, Some(&json!(s)), Ok(()), Ok(json!({})));
        assert!(refus(r).1.contains(s), "le refus nomme le service ({})", s);
        assert_eq!((compte.appels, ouvertures), (0, 0), "aucune tentative sans secret ({})", s);
    }
}

#[test]
fn un_succes_ouvre_une_session_apres_une_seule_tentative() {
    let _t = tour();
    let ctx = compagnie("succes");
    poser(&ctx, "net-entreprises", "fictif-net-entreprises-1");
    poser(&ctx, "urssaf", "fictif-urssaf-2");
    for (s, secret) in [("net-entreprises", "fictif-net-entreprises-1"), ("urssaf", "fictif-urssaf-2")] {
        let (r, compte, ouvertures) = verifier(&ctx, Some(&json!(s)), Ok(()), Ok(json!({})));
        let r = r.unwrap();
        assert_eq!(r, json!({"service": s, "ok": true, "message": "session ouverte"}));
        assert_eq!(compte.appels, 1, "UNE tentative");
        assert_eq!(ouvertures, 0);
        // Le bon secret va au bon service, et ne sort jamais dans la reponse.
        assert_eq!(compte.recu, vec![(s.to_string(), Some(secret.to_string()))]);
        assert!(!r.to_string().contains(secret));
    }
}

#[test]
fn le_mot_de_passe_de_l_environnement_passe_devant_le_coffre() {
    let _t = tour();
    let ctx = compagnie("environnement");
    poser(&ctx, "net-entreprises", "fictif-du-coffre");
    std::env::set_var("NET_ENTREPRISES_MOTDEPASSE", "fictif-de-l-environnement");
    let (r, compte, _) = verifier(&ctx, Some(&json!("net-entreprises")), Ok(()), Ok(json!({})));
    std::env::remove_var("NET_ENTREPRISES_MOTDEPASSE");
    assert_eq!(r.unwrap()["ok"], json!(true));
    assert_eq!(compte.recu, vec![("net-entreprises".to_string(), Some("fictif-de-l-environnement".to_string()))]);
    // Une variable posee suffit : pas de coffre, rien n'est « absent ».
    let ctx = compagnie("environnement-seule");
    std::env::set_var("URSSAF_MOTDEPASSE", "fictif-urssaf-env");
    let (r, compte, _) = verifier(&ctx, Some(&json!("urssaf")), Ok(()), Ok(json!({})));
    std::env::remove_var("URSSAF_MOTDEPASSE");
    assert_eq!(r.unwrap()["ok"], json!(true));
    assert_eq!(compte.recu, vec![("urssaf".to_string(), Some("fictif-urssaf-env".to_string()))]);
}

#[test]
fn un_echec_d_authentification_est_un_verdict_avec_sa_premiere_ligne_et_son_conseil() {
    let _t = tour();
    let ctx = compagnie("echecs");
    poser(&ctx, "net-entreprises", "fictif-net-entreprises-1");
    poser(&ctx, "urssaf", "fictif-urssaf-2");
    // Le conseil d'un 422 et celui d'un 403, chacun dit par son code.
    let c422 = "422";
    let c403 = "403";
    for (message, premiere, conseil) in [
        ("HTTP 401 sur https://services.net-entreprises.fr/x : identifiants refuses\nAutre ligne", "HTTP 401 sur https://services.net-entreprises.fr/x : identifiants refuses", ""),
        ("HTTP 422 sur https://x\nparametres invalides", "HTTP 422 sur https://x", c422),
        ("HTTP 403 sur https://x\ncompte bloque", "HTTP 403 sur https://x", c403),
        // Les deux nombres : le conseil du 422 l'emporte.
        ("HTTP 403 apres 422 essais", "HTTP 403 apres 422 essais", c422),
        // Garde tel quel (decision de Leandre) : le conseil se cherche dans une sous-chaine, une URL ou un port compris.
        ("Connexion impossible a https://hote:4220/chemin\nreseau", "Connexion impossible a https://hote:4220/chemin", c422),
        // Seule la premiere ligne compte.
        ("HTTP 500\n403 ailleurs", "HTTP 500", ""),
        ("\nHTTP 422 ensuite", "", ""),
        ("Mot de passe absent.\nTrois facons de le fournir", "Mot de passe absent.", ""),
        ("Identite du declarant incomplete dans config/entreprise.json :\n  \"declarant\": {...}\nManque : nom, prenom.", "Identite du declarant incomplete dans config/entreprise.json :", ""),
        ("HTTP 401", "HTTP 401", ""),
    ] {
        for s in ["net-entreprises", "urssaf"] {
            let (r, compte, _) = verifier(&ctx, Some(&json!(s)), Err(api(message)), Ok(json!({})));
            let r = r.unwrap();
            assert_eq!((r["service"].clone(), r["ok"].clone(), r["message"].clone()), (json!(s), json!(false), json!(premiere)), "{:?}", message);
            let dit = r["conseil"].as_str().unwrap();
            assert!(if conseil.is_empty() { dit.is_empty() } else { dit.starts_with(conseil) }, "{:?} : {}", message, dit);
            assert_eq!(compte.appels, 1, "UNE tentative, meme en echec");
        }
    }
}

/// Une panne du transport qui n'est pas un echec d'authentification (le reseau, un delai, une configuration) n'est pas un
/// verdict : elle remonte rangee comme le transport l'a rangee (`releve_retours::classe_connue`), pour que le journal des
/// incidents la classe en panne ou en refus.
#[test]
fn une_panne_du_transport_remonte_telle_quelle() {
    let _t = tour();
    let ctx = compagnie("pannes");
    poser(&ctx, "net-entreprises", "fictif-net-entreprises-1");
    for (classe, message) in [("OSError", "reseau coupe"), ("TimeoutError", "delai depasse"), ("ValueError", "environnement inconnu")] {
        let panne = Panne { classe: classe.into(), message: message.into(), code: 0 };
        let (r, compte, _) = verifier(&ctx, Some(&json!("net-entreprises")), Err(panne), Ok(json!({})));
        assert_eq!(refus(r), (classe.to_string(), message.to_string()));
        assert_eq!(compte.appels, 1);
    }
}

#[test]
fn la_boite_se_verifie_par_une_connexion_et_porte_son_service() {
    let _t = tour();
    let ctx = compagnie("boite");
    poser(&ctx, "courriel", "fictif-boite-3");
    let ok = json!({"ok": true, "message": "boite x@y ouverte (imap.gmail.com).", "conseil": ""});
    let (r, compte, ouvertures) = verifier(&ctx, Some(&json!("courriel")), Err(api("jamais appele")), Ok(ok.clone()));
    assert_eq!(r.unwrap(), json!({"ok": true, "message": "boite x@y ouverte (imap.gmail.com).", "conseil": "", "service": "courriel"}));
    assert_eq!((compte.appels, ouvertures), (0, 1));
    // La boite refusee est un verdict, pas un refus de la route ; le verdict garde ses cles et recoit `service` a la fin.
    let refusee = json!({"ok": false, "message": "Identifiants refuses", "conseil": ""});
    let (r, _, _) = verifier(&ctx, Some(&json!("courriel")), Ok(()), Ok(refusee));
    assert_eq!(r.unwrap(), json!({"ok": false, "message": "Identifiants refuses", "conseil": "", "service": "courriel"}));
    // Un service deja dans le verdict est remplace.
    let (r, _, _) = verifier(&ctx, Some(&json!("courriel")), Ok(()), Ok(json!({"ok": true, "service": "autre", "message": "m", "conseil": "c"})));
    assert_eq!(r.unwrap(), json!({"ok": true, "service": "courriel", "message": "m", "conseil": "c"}));
    // Une panne d'ouverture est un refus, qui dit la panne.
    let (r, _, ouvertures) = verifier(&ctx, Some(&json!("courriel")), Ok(()), Err(ErreurCourriel("IMAP : connexion impossible".into())));
    assert_eq!(refus(r).1, "IMAP : connexion impossible");
    assert_eq!(ouvertures, 1);
}

#[test]
fn un_service_pose_ne_rend_pas_un_autre_verifiable() {
    let _t = tour();
    let ctx = compagnie("autre-service");
    poser(&ctx, "net-entreprises", "fictif-net-entreprises-1");
    let (r, compte, _) = verifier(&ctx, Some(&json!("urssaf")), Ok(()), Ok(json!({})));
    assert!(refus(r).1.contains("urssaf"));
    assert_eq!(compte.appels, 0);
}

#[test]
fn la_route_sans_secret_ne_tente_aucune_connexion() {
    // La vraie route, sans rien de pose : elle refuse AVANT toute connexion (aucun reseau dans ce test).
    let _t = tour();
    let ctx = compagnie("route");
    for s in SERVICES {
        let e = coffre_verifier::route(&ctx, &json!({"service": s})).expect_err("rien de pose");
        assert!(e.message.contains(s), "le refus nomme le service : {}", e.message);
    }
    let e = coffre_verifier::route(&ctx, &json!({})).expect_err("service absent");
    assert!(e.message.starts_with("La demande ne nomme pas de service."), "{}", e.message);
    let e = coffre_verifier::route(&ctx, &json!({"service": "xxx"})).expect_err("service inconnu");
    assert!(e.message.starts_with("Service inconnu : « xxx »."), "{}", e.message);
}

#[test]
fn la_vraie_authentification_d_un_declarant_incomplet_echoue_avant_tout_reseau() {
    // Le seul chemin « pour de vrai » sans reseau : le declarant manque, `transport` refuse AVANT d'ouvrir une connexion.
    let _t = tour();
    let ctx = compagnie("declarant-incomplet");
    let mut ent = ctx.entreprise.clone();
    ent.as_object_mut().unwrap().remove("declarant");
    ent.as_object_mut().unwrap().remove("api_dpae");
    ent["declarant_dsn"] = json!({"nom_prenom": ""});
    outils::ecrire(&ctx.racine, "config/entreprise.json", &ent);
    let ctx = Contexte::nouveau(&ctx.racine, Some(Jour::lire("2026-10-03").unwrap())).unwrap();
    assert!(ctx.entreprise.get("declarant").is_none() && ctx.entreprise["declarant_dsn"]["nom_prenom"] == json!(""), "le declarant est bien incomplet : on ne lance pas sinon");
    poser(&ctx, "net-entreprises", "fictif-net-entreprises-1");
    poser(&ctx, "urssaf", "fictif-urssaf-2");
    for s in ["net-entreprises", "urssaf"] {
        let r = coffre_verifier::route(&ctx, &json!({"service": s})).unwrap();
        assert_eq!(r["ok"], json!(false));
        assert!(r["message"].as_str().unwrap().contains("declarant"), "{}", r["message"]);
        assert_eq!(r["service"], json!(s));
        assert!(!r.to_string().contains("fictif-"));
    }
}
