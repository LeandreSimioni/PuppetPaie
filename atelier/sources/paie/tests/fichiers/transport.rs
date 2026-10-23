//! Le transport (`paie::transport::operation`) contre un faux serveur local : les pannes que net-entreprises ne produit
//! pas a la demande (delai, erreur 500, reponse coupee, gzip, redirections), l'authentification, le depot, l'archivage
//! et le journal des retours vus. Les classes `TimeoutError`, `RemoteDisconnected` et `IncompleteRead` sont figees : le
//! moteur les lit pour dire « reseau » (`src/releve_retours.rs`). Aucun reseau hors de la machine ; mot de passe invente.

use crate::outils::{dossier_de_test, serveur, Recu, Requete};
use paie::retours::{base64_ecrire, base64_lire};
use paie::transport::operation;
use serde_json::{json, Value};
use std::io::Write;
use std::time::Duration;

fn reponse(code: u16, entetes: &[(&str, &str)], corps: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {} X\r\n", code).into_bytes();
    let mut a_longueur = false;
    for (k, v) in entetes {
        a_longueur |= k.eq_ignore_ascii_case("content-length") || k.eq_ignore_ascii_case("transfer-encoding");
        r.extend(format!("{}: {}\r\n", k, v).bytes());
    }
    if !a_longueur {
        r.extend(format!("Content-Length: {}\r\n", corps.len()).bytes());
    }
    r.extend(b"\r\n");
    r.extend(corps);
    r
}

fn gzip(donnees: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(donnees).unwrap();
    e.finish().unwrap()
}

/// La premiere ligne de la requete (`GET /ok`), sans la version.
fn ligne(v: &Requete) -> String {
    format!("{} {}", v.methode, v.chemin)
}

fn en_tete<'a>(v: &'a Requete, nom: &str) -> Option<&'a str> {
    v.entetes.iter().find(|(k, _)| k == nom).map(|(_, v)| v.as_str())
}

fn faux() -> (String, Recu) {
    serveur(|q| standard(&q.chemin))
}

// ------------------------------------------------------------------- les appels

fn op(d: Value) -> Value {
    match operation(&d) {
        Ok(v) | Err(v) => v,
    }
}

fn appel(url: &str, plus: Value) -> Value {
    let mut d = json!({"operation": "appel", "url": url});
    for (k, v) in plus.as_object().into_iter().flatten() {
        d[k] = v.clone();
    }
    op(d)
}

fn corps_de(r: &Value) -> Vec<u8> {
    base64_lire(r["corps"].as_str().unwrap_or("")).unwrap()
}

fn erreur(r: &Value) -> (String, String, Option<i64>) {
    assert_eq!(r["ok"], json!(false), "une panne etait attendue : {}", r);
    (r["classe"].as_str().unwrap().to_string(), r["erreur"].as_str().unwrap().to_string(), r["code"].as_i64())
}

fn standard(chemin: &str) -> Option<Vec<u8>> {
    let ok = reponse(200, &[("Content-Type", "application/xml"), ("Expires", "Sat, 03 Oct 2026 14:00:00 GMT")], b"<retours/>");
    Some(match chemin {
        "/ok" => ok,
        "/vide" => reponse(204, &[], b""),
        "/auth" => reponse(200, &[("Expires", "Sat, 03 Oct 2026 14:00:00 GMT")], b" JETON-123 \n"),
        "/auth-non-ascii" => reponse(200, &[], "j\u{e9}ton\n".as_bytes()),
        "/gzip" => reponse(200, &[("Content-Encoding", "gzip")], &gzip(&b"<retours/>".repeat(50))),
        "/gzip-deux" => {
            let mut g = gzip(b"abc");
            g.extend(gzip(b"def"));
            g.extend([0, 0, 0]);
            reponse(200, &[("Content-Encoding", "gzip")], &g)
        }
        "/gzip-vide" => reponse(200, &[("Content-Encoding", "gzip")], b""),
        "/gzip-pas-gzip" => reponse(200, &[("Content-Encoding", "gzip")], b"ceci n'est pas du gzip"),
        "/gzip-un-octet" => reponse(200, &[("Content-Encoding", "gzip")], b"x"),
        "/gzip-coupe" => {
            let g = gzip(&b"<retours/>".repeat(50));
            reponse(200, &[("Content-Encoding", "gzip")], &g[..g.len() - 12])
        }
        "/gzip-crc" => {
            let mut g = gzip(b"<retours/>");
            let n = g.len();
            g[n - 8] ^= 0xff;
            reponse(200, &[("Content-Encoding", "gzip")], &g)
        }
        "/gzip-longueur" => {
            let mut g = gzip(b"<retours/>");
            let n = g.len();
            g[n - 4] ^= 0xff;
            reponse(200, &[("Content-Encoding", "gzip")], &g)
        }
        "/gzip-erreur" => reponse(500, &[("Content-Encoding", "gzip")], &gzip(b"panne")),
        "/500" => reponse(500, &[], b"Erreur interne\n"),
        "/500-vide" => reponse(500, &[], b""),
        "/500-long" => reponse(500, &[], &b"erreur ".repeat(120)),
        "/500-accents" => reponse(500, &[], "R\u{e9}ponse accentu\u{e9}e ? avec point d'interrogation".as_bytes()),
        "/500-latin1" => reponse(500, &[], b"caf\xe9 et \xe0 la fin"),
        "/500-point" => reponse(500, &[], b"avant ? apres"),
        "/301" => reponse(301, &[("Location", "/ok")], b""),
        "/302-relatif" => reponse(302, &[("Location", "ok")], b""),
        "/307" => reponse(307, &[("Location", "/ok")], b""),
        "/boucle" => reponse(302, &[("Location", "/boucle")], b""),
        "/coupee" => b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n0123456789".to_vec(),
        "/coupee-chunked" => b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n".to_vec(),
        "/ferme" => return None,
        "/chunked" => b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n".to_vec(),
        c if c.starts_with("/code/") => {
            let code: u16 = c[6..].parse().unwrap();
            reponse(code, &[("Content-Type", "text/plain")], format!("corps {}", code).as_bytes())
        }
        "/lent" => {
            std::thread::sleep(Duration::from_secs(4));
            ok
        }
        _ => reponse(404, &[], b"inconnu"),
    })
}

#[test]
fn un_get_rend_le_corps_et_les_entetes_comme_le_serveur_les_ecrit() {
    let (base, recu) = faux();
    let r = appel(&format!("{}/ok", base), json!({"jeton": "J-77"}));
    assert_eq!(r["ok"], json!(true));
    assert_eq!(corps_de(&r), b"<retours/>");
    assert_eq!(r["entetes"]["Content-Type"], json!("application/xml"));
    assert_eq!(r["entetes"]["Expires"], json!("Sat, 03 Oct 2026 14:00:00 GMT"));
    let v = &recu.lock().unwrap()[0];
    assert_eq!(ligne(v), "GET /ok");
    assert_eq!(en_tete(v, "user-agent"), Some("Client-DSN (PuppetPaie/1.0; PuppetPaie)"));
    assert_eq!(en_tete(v, "accept-encoding"), Some("gzip"));
    assert_eq!(en_tete(v, "authorization"), Some("DSNLogin jeton=J-77"));
    assert_eq!(en_tete(v, "content-type"), None);
}

#[test]
fn sans_jeton_pas_d_authorization() {
    let (base, recu) = faux();
    appel(&format!("{}/ok", base), json!({}));
    assert_eq!(en_tete(&recu.lock().unwrap()[0], "authorization"), None);
}

#[test]
fn des_donnees_font_un_post_avec_leur_type_ou_celui_d_un_formulaire() {
    let (base, recu) = faux();
    appel(&format!("{}/ok", base), json!({"donnees": base64_ecrire(b"<a>&</a>"), "content_type": "application/xml"}));
    appel(&format!("{}/ok", base), json!({"donnees": base64_ecrire(b"d")}));
    appel(&format!("{}/ok", base), json!({"donnees": ""}));
    let r = recu.lock().unwrap();
    let vus: Vec<(String, Option<&str>, &[u8])> = r.iter().map(|v| (ligne(v), en_tete(v, "content-type"), v.corps.as_slice())).collect();
    assert!(vus.contains(&("POST /ok".into(), Some("application/xml"), b"<a>&</a>".as_slice())));
    assert!(vus.contains(&("POST /ok".into(), Some("application/x-www-form-urlencoded"), b"d".as_slice())));
    assert!(vus.contains(&("GET /ok".into(), None, b"".as_slice())), "des donnees vides font un GET");
}

#[test]
fn un_corps_gzip_est_decompresse_et_un_corps_par_morceaux_est_recolle() {
    let (base, _) = faux();
    assert_eq!(corps_de(&appel(&format!("{}/gzip", base), json!({}))), b"<retours/>".repeat(50));
    assert_eq!(corps_de(&appel(&format!("{}/gzip-deux", base), json!({}))), b"abcdef");
    assert_eq!(corps_de(&appel(&format!("{}/gzip-vide", base), json!({}))), b"");
    assert_eq!(corps_de(&appel(&format!("{}/chunked", base), json!({}))), b"hello world");
    assert_eq!(corps_de(&appel(&format!("{}/vide", base), json!({}))), b"");
}

#[test]
fn un_gzip_illisible_est_une_panne_sans_code() {
    let (base, _) = faux();
    for c in ["gzip-pas-gzip", "gzip-un-octet", "gzip-coupe", "gzip-crc", "gzip-longueur"] {
        assert_eq!(erreur(&appel(&format!("{}/{}", base, c), json!({}))).2, None, "{}", c);
    }
}

#[test]
fn les_statuts_d_erreur_portent_leur_message_et_leur_code() {
    let (base, _) = faux();
    for (code, phrase) in [
        (401, "Authentification refusee, ou declarant non inscrit au service DSN."),
        (403, "Compte temporairement bloque (trop de tentatives), ou tentative d'acces a des donnees deposees par quelqu'un d'autre."),
        (404, "Service ou identifiant inconnu. Peut aussi signaler une interruption de service."),
        (406, "Format de reponse demande non supporte."),
        (422, "Parametres invalides."),
        (429, "Contraintes de consultation non respectees : trop d'appels, ou plage de recherche trop large. Attendre et relancer."),
        (503, "Service indisponible."),
        (400, ""),
        (418, ""),
    ] {
        let url = format!("{}/code/{}?secret=abc", base, code);
        let (classe, message, c) = erreur(&appel(&url, json!({})));
        assert_eq!(classe, "ErreurApi");
        assert_eq!(c, Some(code));
        // Le message dit le statut, l'adresse, et ce que le guide de l'API en dit.
        assert!(message.contains(&format!("HTTP {} sur {}/code/{}", code, base, code)) && message.contains(phrase), "{}", message);
        assert!(!message.contains("secret"), "la requete ne fuit pas dans le message");
    }
}

#[test]
fn le_corps_d_une_erreur_est_coupe_a_400_caracteres_et_decode_en_remplacant() {
    let (base, _) = faux();
    let m = |c: &str| erreur(&appel(&format!("{}/{}", base, c), json!({}))).1;
    assert_eq!(m("500"), format!("HTTP 500 sur {}/500\n  \n  Erreur interne\n", base));
    assert_eq!(m("500-vide"), format!("HTTP 500 sur {}/500-vide\n  ", base));
    let long = m("500-long");
    assert_eq!(long.chars().count(), format!("HTTP 500 sur {}/500-long\n  \n  ", base).chars().count() + 400);
    assert!(m("500-accents").ends_with("R\u{e9}ponse accentu\u{e9}e ? avec point d'interrogation"));
    assert!(m("500-latin1").ends_with("caf\u{fffd} et \u{fffd} la fin"));
    assert!(m("500-point").ends_with("avant ? apres"));
    // Le corps d'une erreur n'est pas decompresse : du gzip y devient des caracteres de remplacement.
    assert!(m("gzip-erreur").contains('\u{fffd}'));
}

#[test]
fn les_redirections_se_suivent_et_un_post_redirige_repart_en_get() {
    let (base, recu) = faux();
    for c in ["301", "302-relatif"] {
        assert_eq!(corps_de(&appel(&format!("{}/{}", base, c), json!({"jeton": "J"}))), b"<retours/>", "{}", c);
    }
    // Un POST redirige en 302 repart en GET, sans corps ni type.
    recu.lock().unwrap().clear();
    let r = appel(&format!("{}/302-relatif", base), json!({"jeton": "J", "donnees": base64_ecrire(b"d"), "content_type": "application/xml"}));
    assert_eq!(r["ok"], json!(true));
    let v = recu.lock().unwrap();
    let suite = v.iter().find(|v| ligne(v) == "GET /ok").unwrap();
    assert_eq!(en_tete(suite, "content-type"), None);
    assert_eq!(en_tete(suite, "authorization"), Some("DSNLogin jeton=J"));
    drop(v);
    // Un 307 sur un POST n'est pas suivi : c'est une erreur HTTP 307.
    let e = erreur(&appel(&format!("{}/307", base), json!({"donnees": base64_ecrire(b"d")})));
    assert_eq!((e.0.as_str(), e.2), ("ErreurApi", Some(307)));
    // Une boucle s'arrete.
    let e = erreur(&appel(&format!("{}/boucle", base), json!({})));
    assert_eq!(e.2, Some(302));
}

#[test]
fn un_delai_depasse_est_un_timeout_error() {
    let (base, _) = faux();
    let t = std::time::Instant::now();
    assert_eq!(erreur(&appel(&format!("{}/lent", base), json!({"delai": 1}))).0, "TimeoutError");
    assert!(t.elapsed() < Duration::from_secs(3));
}

#[test]
fn une_reponse_coupee_est_une_incomplete_read() {
    let (base, _) = faux();
    for c in ["coupee", "coupee-chunked"] {
        assert_eq!(erreur(&appel(&format!("{}/{}", base, c), json!({}))).0, "IncompleteRead", "{}", c);
    }
}

#[test]
fn une_connexion_fermee_sans_reponse_est_remote_disconnected() {
    let (base, _) = faux();
    assert_eq!(erreur(&appel(&format!("{}/ferme", base), json!({}))).0, "RemoteDisconnected");
}

#[test]
fn un_port_ferme_est_un_reseau_injoignable_sans_le_secret() {
    let base = crate::outils::port_ferme();
    let (classe, message, code) = erreur(&appel(&format!("{}/x?secret=zzz", base), json!({})));
    assert_eq!((classe.as_str(), code), ("ErreurApi", Some(0)));
    assert!(message.starts_with(&format!("Reseau injoignable pour {}/x : ", base)) && !message.contains("zzz"), "{}", message);
}

// ------------------------------------------------------- l'authentification

fn authentifier(base: &str, chemin: &str, plus: Value) -> Value {
    let mut d = json!({
        "operation": "authentifier", "url": format!("{}{}", base, chemin), "service": "97", "variable": "NET_ENTREPRISES_MOTDEPASSE",
        "declarant": {"siret": "12345678901234", "nom": "DUPONT", "prenom": "Jean Pierre"}, "motdepasse": "secret-de-test"
    });
    for (k, v) in plus.as_object().into_iter().flatten() {
        d[k] = v.clone();
    }
    op(d)
}

#[test]
fn l_authentification_envoie_les_identifiants_du_guide_et_rend_le_jeton() {
    let (base, recu) = faux();
    let r = authentifier(&base, "/auth", json!({"motdepasse": "a&b<c>d"}));
    assert_eq!(r["jeton"], json!("JETON-123"));
    assert_eq!(r["expire_le"], json!("Sat, 03 Oct 2026 14:00:00 GMT"));
    let v = &recu.lock().unwrap()[0];
    assert_eq!(ligne(v), "POST /auth");
    assert_eq!(en_tete(v, "content-type"), Some("application/xml"));
    assert_eq!(
        String::from_utf8_lossy(&v.corps),
        "<identifiants><siret>12345678901234</siret><nom>DUPONT</nom><prenom>Jean Pierre</prenom><motdepasse>a&amp;b&lt;c&gt;d</motdepasse><service>97</service></identifiants>"
    );
}

#[test]
fn sans_service_la_balise_tombe_et_un_jeton_deja_la_est_renvoye_dans_l_en_tete() {
    let (base, recu) = faux();
    authentifier(&base, "/auth", json!({"service": null, "jeton": "ANCIEN"}));
    let v = &recu.lock().unwrap()[0];
    assert!(!String::from_utf8_lossy(&v.corps).contains("<service>"));
    assert_eq!(en_tete(v, "authorization"), Some("DSNLogin jeton=ANCIEN"));
}

#[test]
fn le_jeton_non_ascii_devient_des_caracteres_de_remplacement() {
    let (base, _) = faux();
    assert_eq!(authentifier(&base, "/auth-non-ascii", json!({}))["jeton"], json!("j\u{fffd}\u{fffd}ton"));
}

#[test]
fn un_mot_de_passe_absent_ou_un_declarant_incomplet_ne_partent_pas() {
    let (base, recu) = faux();
    for mdp in [json!(null), json!("")] {
        let (classe, message, code) = erreur(&authentifier(&base, "/auth", json!({"motdepasse": mdp})));
        assert_eq!((classe.as_str(), code), ("ErreurApi", Some(0)));
        assert!(message.contains("NET_ENTREPRISES_MOTDEPASSE"), "le message dit ou poser le mot de passe : {}", message);
    }
    let (_, message, _) = erreur(&authentifier(&base, "/auth", json!({"declarant": {"siret": "1", "nom": "", "prenom": null}})));
    assert!(message.contains("Manque : nom, prenom."), "{}", message);
    assert!(recu.lock().unwrap().is_empty(), "rien n'est parti");
}

#[test]
fn le_mot_de_passe_ne_fuit_dans_aucun_message() {
    let (base, _) = faux();
    for chemin in ["/code/401", "/code/500", "/ferme", "/introuvable-du-tout"] {
        let r = authentifier(&base, chemin, json!({"motdepasse": "Tres-Secret-42"}));
        assert!(!r.to_string().contains("Tres-Secret-42"), "{} : {}", chemin, r);
    }
}

// ------------------------------------------------------------------ les depots

#[test]
fn un_depot_part_en_post_text_plain_gzip_avec_le_jeton() {
    let (base, recu) = faux();
    let corps = gzip(b"faux depot");
    let r = op(json!({"operation": "depot", "url": format!("{}/ok", base), "jeton": "J-9", "forme": "dsn", "corps": base64_ecrire(&corps)}));
    assert_eq!(r["ok"], json!(true));
    assert_eq!(corps_de(&r), b"<retours/>");
    let v = &recu.lock().unwrap()[0];
    assert_eq!(ligne(v), "POST /ok");
    assert_eq!(en_tete(v, "content-type"), Some("text/plain"));
    assert_eq!(en_tete(v, "content-encoding"), Some("gzip"));
    assert_eq!(en_tete(v, "authorization"), Some("DSNLogin jeton=J-9"));
    assert_eq!(v.corps, corps);
}

#[test]
fn un_depot_refuse_se_raconte_selon_son_canal() {
    let (base, _) = faux();
    let depot = |forme: &str, chemin: &str| {
        erreur(&op(json!({"operation": "depot", "url": format!("{}{}", base, chemin), "jeton": "J", "forme": forme, "corps": base64_ecrire(b"x")})))
    };
    // DSN : `DepotRefuse`, le detail du statut n'a jamais ete affiche (cles entieres cherchees par texte).
    let e = depot("dsn", "/code/401");
    assert_eq!((e.0.as_str(), e.1.as_str(), e.2), ("DepotRefuse", "Le point de depot a repondu 401.\ncorps 401", None));
    // ... et le corps passe par `_sans_secret` : il est coupe au premier `?`.
    assert_eq!(depot("dsn", "/500-point").1, "Le point de depot a repondu 500.\navant ");
    // DPAE : `ErreurApi`, avec le message du statut et sans code.
    let e = depot("dpae", "/code/401");
    assert_eq!(
        (e.0.as_str(), e.1.as_str(), e.2),
        ("ErreurApi", format!("HTTP 401 au depot sur {}/code/401\n  Authentification refusee, ou declarant non inscrit au service DSN.\n  corps 401", base).as_str(), None)
    );
    assert_eq!(depot("dpae", "/500-vide").1, format!("HTTP 500 au depot sur {}/500-vide\n  ", base));
}

#[test]
fn le_delai_d_un_depot_est_de_120_secondes_celui_d_un_appel_de_60() {
    let (base, _) = faux();
    let e = erreur(&op(json!({"operation": "depot", "url": format!("{}/lent", base), "jeton": "J", "forme": "dsn", "corps": "", "delai": 1})));
    assert_eq!(e.0, "TimeoutError");
    let c = op(json!({"operation": "constantes"}));
    assert_eq!((c["delai"].as_u64(), c["delai_depot"].as_u64()), (Some(60), Some(120)));
}

// --------------------------------------------------------------- les outils

#[test]
fn horodatage_echappement_et_sans_secret() {
    let h = |v: Value| op(json!({"operation": "horodatage", "valeur": v}));
    assert_eq!(h(json!("20260901000000"))["texte"], json!("20260901000000"));
    assert_eq!(h(json!({"iso": "2026-09-04T12:32:42.123456"}))["texte"], json!("20260904123242"));
    assert_eq!(h(json!({"iso": "2026-09-04"}))["texte"], json!("20260904000000"));
    assert_eq!(h(json!({"iso": "2026-01-02T03:04:05+02:00"}))["texte"], json!("20260102030405"));
    assert_eq!(h(json!({"iso": "pas une date"}))["ok"], json!(false));
    assert_eq!(op(json!({"operation": "echapper", "texte": "a&b<c>d&amp;"}))["texte"], json!("a&amp;b&lt;c&gt;d&amp;amp;"));
    assert_eq!(op(json!({"operation": "sans_secret", "texte": "https://h/x?jeton=1&b=2"}))["texte"], json!("https://h/x"));
}

#[test]
fn le_declarant_vient_de_la_cle_declarant_ou_du_nom_prenom_de_la_dsn() {
    let d = |e: Value| op(json!({"operation": "declarant", "entreprise": e}));
    assert_eq!(d(json!({"siret": "1", "declarant": {"siret": "9", "nom": "N", "prenom": "P"}}))["declarant"], json!({"siret": "9", "nom": "N", "prenom": "P"}));
    assert_eq!(
        d(json!({"siret": "123", "declarant_dsn": {"nom_prenom": "  DUPONT\u{a0}Jean\u{1680}Paul  "}}))["declarant"],
        json!({"siret": "123", "nom": "DUPONT", "prenom": "Jean Paul"})
    );
    assert_eq!(d(json!({"siret": "123", "declarant_dsn": {"nom_prenom": "SEUL"}}))["declarant"], json!({"siret": "123"}));
    assert_eq!(d(json!({}))["declarant"], json!({"siret": null}));
    assert_eq!(d(json!({"declarant_dsn": {"nom_prenom": null}}))["ok"], json!(false));
}

// -------------------------------------------------------- l'archivage

#[test]
fn archiver_nomme_la_piece_comme_avant_et_ne_l_ecrase_pas() {
    let racine = dossier_de_test("transport", "archiver");
    let r = racine.to_str().unwrap();
    let arch = |retour: Value, contenu: &[u8], ct: &str, canal: &str| {
        let v = op(json!({"operation": "archiver", "racine": r, "retour": retour, "contenu": base64_ecrire(contenu), "content_type": ct, "canal": canal, "maintenant": "20261003-141516"}));
        std::path::PathBuf::from(v["chemin"].as_str().unwrap())
    };
    let retour = json!({"publication": "20260904123242", "nature": "11", "flux": "F1spdeAilkR4q3UUlQA7haa"});
    let p = arch(retour.clone(), b"<bilan/>", "application/xml", "");
    assert_eq!(p, racine.join("retours/20260904-123242_nature11_F1spdeAilkR4q3UUlQA7haa.xml"));
    assert_eq!(std::fs::read(&p).unwrap(), b"<bilan/>");
    // Une piece deja la n'est pas reecrite.
    let encore = arch(retour, b"AUTRE", "application/xml", "");
    assert_eq!((encore.clone(), std::fs::read(&encore).unwrap()), (p, b"<bilan/>".to_vec()));
    // L'extension : pdf, xml (type ou premier octet), sinon txt.
    let ext = |ct: &str, c: &[u8]| arch(json!({"publication": "20260904123242", "nature": 1, "flux": ct.replace('/', "_")}), c, ct, "").extension().unwrap().to_str().unwrap().to_string();
    assert_eq!(ext("application/pdf", b"x"), "pdf");
    assert_eq!(ext("text/xml", b"x"), "xml");
    assert_eq!(ext("text/plain", b" \x0b\n<a/>"), "xml");
    assert_eq!(ext("text/plain", b"texte"), "txt");
    // Un canal a son sous-dossier ; sans date, l'heure fournie nomme la piece ; sans flux, `sansflux`.
    let d = arch(json!({"nature": "41"}), b"<c/>", "application/xml", "dpae");
    assert_eq!(d, racine.join("retours/dpae/20261003-141516_nature41_sansflux.xml"));
}

#[test]
fn le_journal_des_retours_vus_note_sans_doublon() {
    let racine = dossier_de_test("transport", "journal");
    let r = racine.to_str().unwrap();
    let e1 = json!({"flux": "F1", "nature": "11", "publication": "1"});
    let e2 = json!({"flux": "F2", "nature": "94", "publication": "2", "url": "http://x/\u{e9}"});
    assert_eq!(op(json!({"operation": "deja_vus", "racine": r}))["vus"], json!([]));
    op(json!({"operation": "noter", "racine": r, "entrees": [e1, e2]}));
    op(json!({"operation": "noter", "racine": r, "entrees": [e1, {"flux": "F3", "nature": "1", "publication": "3"}]}));
    // Chaque entree une fois, entiere (l'url comprise).
    let t: Value = serde_json::from_str(&std::fs::read_to_string(racine.join("retours/journal.json")).unwrap()).unwrap();
    assert_eq!(t, json!([e1, e2, {"flux": "F3", "nature": "1", "publication": "3"}]));
    assert_eq!(op(json!({"operation": "deja_vus", "racine": r}))["vus"], json!([["F1", "11", "1"], ["F2", "94", "2"], ["F3", "1", "3"]]));
    // Le canal DPAE a son journal a part.
    assert_eq!(op(json!({"operation": "deja_vus", "racine": r, "canal": "dpae"}))["vus"], json!([]));
    // Un journal illisible est refuse.
    std::fs::write(racine.join("retours/journal.json"), "pas du json").unwrap();
    assert_eq!(op(json!({"operation": "deja_vus", "racine": r}))["ok"], json!(false));
}

#[test]
fn la_demande_inconnue_ou_illisible_est_une_reponse_pas_un_plantage() {
    assert_eq!(op(json!({"operation": "rien"}))["ok"], json!(false));
    assert_eq!(op(json!({}))["ok"], json!(false));
    assert_eq!(op(json!({"operation": "appel"}))["ok"], json!(false));
}
