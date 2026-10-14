//! Le client Drive (`drive::Client`) contre un faux serveur sur `127.0.0.1` : le corps `multipart/related`, l'echappement
//! de `q`, la pagination, le remplacement par `PATCH` (dont le corps vide), la traduction des erreurs de Google, le delai
//! (`avec_delai`), le trait `Drive` sans suppression. Le jeton renouvele apres un 401 (cle du poste dans l'environnement) :
//! `tests/drive_jeton.rs`. Aucune connexion vers l'exterieur ; `google.json` est fictif.

use std::path::{Path, PathBuf};
use std::time::Duration;

use paie::drive::{corps_multipart, Adresses, Client, Distant, Drive};
use paie::sauvegarde::hors_ligne;

use crate::outils::{http, Recu, Requete};
use serde_json::{json, Value};

/// Les parametres decodes (`%XX`, `+` = espace), dans l'ordre.
trait Parametres {
    fn parametres(&self) -> Vec<(String, String)>;
    fn param(&self, nom: &str) -> Option<String> {
        self.parametres().into_iter().find(|(k, _)| k == nom).map(|(_, v)| v)
    }
}

impl Parametres for Requete {
    fn parametres(&self) -> Vec<(String, String)> {
        let d = |s: &str| -> String {
            let o = s.as_bytes();
            let mut sortie = Vec::new();
            let mut i = 0;
            while i < o.len() {
                match o[i] {
                    b'+' => sortie.push(b' '),
                    b'%' if i + 2 < o.len() => {
                        sortie.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                        i += 2;
                    }
                    b => sortie.push(b),
                }
                i += 1;
            }
            String::from_utf8(sortie).unwrap()
        };
        self.requete
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (d(k), d(v))
            })
            .collect()
    }
}

fn reponse(code: u16, corps: &[u8]) -> Vec<u8> {
    http(code, "application/json", corps)
}

fn json_ok(v: Value) -> Vec<u8> {
    reponse(200, v.to_string().as_bytes())
}

/// Le faux serveur d'`outils`, qui repond toujours.
fn serveur(f: impl Fn(&Requete) -> Vec<u8> + Send + Sync + 'static) -> (String, Recu) {
    crate::outils::serveur(move |q| Some(f(q)))
}

fn adresses(base: &str) -> Adresses {
    Adresses {
        jetons: format!("{}/token", base),
        fichiers: format!("{}/drive/v3/files", base),
        televersement: format!("{}/upload/drive/v3/files", base),
    }
}

fn racine(nom: &str) -> PathBuf {
    let r = crate::outils::dossier_de_test("drive", nom);
    std::fs::create_dir_all(r.join("config")).unwrap();
    r
}

// ------------------------------------------------------------ le corps multipart

#[test]
fn corps_multipart_octet_pour_octet() {
    let corps = corps_multipart("a b.txt", "PARENT", "text/plain", b"salut\r\n", "----ABC");
    let attendu = "------ABC\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{\"name\": \"a b.txt\", \"parents\": [\"PARENT\"]}\r\n\
                   ------ABC\r\nContent-Type: text/plain\r\n\r\nsalut\r\n\r\n------ABC--\r\n";
    assert_eq!(String::from_utf8(corps).unwrap(), attendu);
}

#[test]
fn corps_multipart_un_nom_accentue_arrive_entier() {
    // Drive lit le JSON en UTF-8 : un accent ou un emoji dans le nom arrive tel quel, quelle que soit son ecriture.
    let corps = corps_multipart("\u{e9}\u{1F600}'.pdf", "P", "application/pdf", b"%PDF", "----F");
    let texte = String::from_utf8(corps).unwrap();
    let json_part = texte.split("\r\n\r\n").nth(1).unwrap().split("\r\n").next().unwrap();
    assert_eq!(serde_json::from_str::<Value>(json_part).unwrap(), json!({"name": "\u{e9}\u{1F600}'.pdf", "parents": ["P"]}));
}

// ------------------------------------------------------------ dossier, lister

#[test]
fn dossier_trouve_l_echappement_de_q_apostrophe_et_barre_oblique_inverse() {
    let (base, recu) = serveur(|_| json_ok(json!({"files": [{"id": "D1", "name": "x"}]})));
    let mut c = Client::pour_essai(&racine("q"), adresses(&base), "jeton", Some("c@exemple.invalid"));
    assert_eq!(c.dossier("l'an \\ 2026", "root").unwrap(), "D1");
    let r = recu.lock().unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].methode.as_str(), r[0].chemin.as_str()), ("GET", "/drive/v3/files"));
    assert_eq!(
        r[0].param("q").unwrap(),
        "name = 'l\\'an \\\\ 2026' and mimeType = 'application/vnd.google-apps.folder' and 'root' in parents and trashed = false"
    );
    assert_eq!(r[0].param("fields").unwrap(), "files(id,name)");
    assert_eq!(r[0].param("pageSize").unwrap(), "10");
    assert_eq!(r[0].entete("authorization"), "Bearer jeton");
}

#[test]
fn dossier_absent_est_cree_par_un_post_json() {
    let (base, recu) = serveur(|v| if v.methode == "GET" { json_ok(json!({"files": []})) } else { json_ok(json!({"id": "NEUF"})) });
    let mut c = Client::pour_essai(&racine("cree"), adresses(&base), "jeton", None);
    assert_eq!(c.dossier("2026-06", "P1").unwrap(), "NEUF");
    let r = recu.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!((r[1].methode.as_str(), r[1].chemin.as_str()), ("POST", "/drive/v3/files"));
    assert_eq!(r[1].entete("content-type"), "application/json");
    assert_eq!(r[1].json(), json!({"name": "2026-06", "mimeType": "application/vnd.google-apps.folder", "parents": ["P1"]}));
}

#[test]
fn chemin_cree_toute_la_suite_a_partir_de_root() {
    let (base, recu) = serveur(|v| {
        if v.methode == "GET" {
            json_ok(json!({"files": []}))
        } else {
            let nom = serde_json::from_slice::<Value>(&v.corps).unwrap()["name"].as_str().unwrap().to_string();
            json_ok(json!({"id": format!("id-{}", nom)}))
        }
    });
    let mut c = Client::pour_essai(&racine("chemin"), adresses(&base), "jeton", None);
    assert_eq!(c.chemin(&["PuppetPaie", "sauvegarde", "config"]).unwrap(), "id-config");
    let r = recu.lock().unwrap();
    let parents: Vec<String> = r.iter().filter(|v| v.methode == "POST").map(|v| serde_json::from_slice::<Value>(&v.corps).unwrap()["parents"][0].as_str().unwrap().to_string()).collect();
    assert_eq!(parents, ["root", "id-PuppetPaie", "id-sauvegarde"]);
}

#[test]
fn lister_pagine_ecarte_les_dossiers_et_lit_md5_et_taille() {
    let (base, recu) = serveur(|v| match v.param("pageToken").as_deref() {
        None => json_ok(json!({
            "nextPageToken": "SUITE 1/&",
            "files": [
                {"id": "1", "name": "a.txt", "md5Checksum": "m1", "size": "12"},
                {"id": "2", "name": "sous", "mimeType": "application/vnd.google-apps.folder"},
            ]
        })),
        Some("SUITE 1/&") => json_ok(json!({"files": [{"id": "3", "name": "doc"}, {"id": "4", "name": "b.txt", "md5Checksum": "m4", "size": "0"}]})),
        Some(autre) => panic!("jeton de page inattendu : {}", autre),
    });
    let mut c = Client::pour_essai(&racine("lister"), adresses(&base), "jeton", None);
    let l = c.lister("PAR'ENT").unwrap();
    assert_eq!(l.keys().cloned().collect::<Vec<_>>(), ["a.txt", "b.txt", "doc"]);
    assert_eq!(l["a.txt"], Distant { id: "1".into(), md5: Some("m1".into()), octets: 12 });
    assert_eq!(l["doc"], Distant { id: "3".into(), md5: None, octets: 0 });
    let r = recu.lock().unwrap();
    assert_eq!(r.len(), 2);
    // Le parent du `q` n'est pas echappe (un identifiant Drive), le listage prend 200 par page.
    assert_eq!(r[0].param("q").unwrap(), "'PAR'ENT' in parents and trashed = false");
    assert_eq!(r[0].param("pageSize").unwrap(), "200");
    assert_eq!(r[0].param("fields").unwrap(), "nextPageToken,files(id,name,md5Checksum,size,mimeType)");
    assert_eq!(r[1].param("pageToken").unwrap(), "SUITE 1/&");
}

// ------------------------------------------------------------ sous_dossiers, telecharger (la restauration, lot R1)

#[test]
fn sous_dossiers_pagine_ne_demande_que_des_dossiers_et_garde_les_homonymes() {
    let (base, recu) = serveur(|v| match v.param("pageToken").as_deref() {
        None => json_ok(json!({"nextPageToken": "P2", "files": [{"id": "D1", "name": "config"}, {"id": "D2", "name": "paies"}]})),
        Some("P2") => json_ok(json!({"files": [{"id": "D3", "name": "paies"}]})),
        Some(autre) => panic!("jeton de page inattendu : {}", autre),
    });
    let mut c = Client::pour_essai(&racine("sous-dossiers"), adresses(&base), "jeton", None);
    let l = c.sous_dossiers("root").unwrap();
    assert_eq!(l, [("config".to_string(), "D1".to_string()), ("paies".into(), "D2".into()), ("paies".into(), "D3".into())]);
    let r = recu.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|v| v.methode == "GET" && v.chemin == "/drive/v3/files"), "rien ne se cree");
    assert_eq!(r[0].param("q").unwrap(), "'root' in parents and mimeType = 'application/vnd.google-apps.folder' and trashed = false");
    assert_eq!(r[0].param("fields").unwrap(), "nextPageToken,files(id,name)");
    assert_eq!(r[1].param("pageToken").unwrap(), "P2");
}

#[test]
fn telecharger_rend_les_octets_tels_quels_par_alt_media() {
    // Des octets qui ne sont pas du texte, ni du JSON : ils reviennent sans etre lus.
    let contenu: Vec<u8> = vec![0x00, 0xFF, b'{', 0xC3, 0x28, b'\r', b'\n'];
    let servi = contenu.clone();
    let (base, recu) = serveur(move |_| http(200, "application/octet-stream", &servi));
    let mut c = Client::pour_essai(&racine("telecharger"), adresses(&base), "jeton", None);
    assert_eq!(c.telecharger("F-1_a").unwrap(), contenu);
    let r = recu.lock().unwrap();
    assert_eq!((r[0].methode.as_str(), r[0].chemin.as_str()), ("GET", "/drive/v3/files/F-1_a"));
    assert_eq!(r[0].param("alt").unwrap(), "media");
    assert_eq!(r[0].entete("authorization"), "Bearer jeton");
}

#[test]
fn telecharger_un_refus_de_google_est_une_erreur_et_un_identifiant_ne_fabrique_pas_de_chemin() {
    let (base, recu) = serveur(|_| reponse(404, br#"{"error": {"code": 404, "message": "File not found: X."}}"#));
    let mut c = Client::pour_essai(&racine("telecharger-404"), adresses(&base), "jeton", None);
    let e = c.telecharger("../x?y").unwrap_err();
    assert_eq!(e.statut, Some(404));
    assert_eq!(recu.lock().unwrap()[0].chemin, "/drive/v3/files/..%2Fx%3Fy");
}

#[test]
fn lister_doublons_le_dernier_gagne() {
    let (base, _) = serveur(|_| json_ok(json!({"files": [{"id": "1", "name": "x", "md5Checksum": "a"}, {"id": "2", "name": "x", "md5Checksum": "b"}]})));
    let mut c = Client::pour_essai(&racine("doublons"), adresses(&base), "jeton", None);
    let l = c.lister("P").unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l["x"].id, "2");
}

// ------------------------------------------------------------ televerser

#[test]
fn televerser_cree_par_un_multipart_related() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "F1", "name": "note.txt", "webViewLink": "https://exemple.invalid/f1"})));
    let r = racine("creer");
    let f = r.join("note.txt");
    std::fs::write(&f, "contenu\n").unwrap();
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    let d = c.televerser(&f, "PARENT", None, None).unwrap();
    assert_eq!((d.id.as_deref(), d.nom.as_deref(), d.lien.as_deref(), d.remplace), (Some("F1"), Some("note.txt"), Some("https://exemple.invalid/f1"), false));
    let v = recu.lock().unwrap()[0].clone();
    assert_eq!((v.methode.as_str(), v.chemin.as_str()), ("POST", "/upload/drive/v3/files"));
    assert_eq!(v.param("uploadType").unwrap(), "multipart");
    assert_eq!(v.param("fields").unwrap(), "id,name,webViewLink");
    let ct = v.entete("content-type");
    let frontiere = ct.strip_prefix("multipart/related; boundary=").expect(ct).to_string();
    // `----` et 32 chiffres hexadecimaux (`secrets.token_hex(16)`).
    assert!(frontiere.starts_with("----") && frontiere.len() == 4 + 32 && frontiere[4..].bytes().all(|b| b.is_ascii_hexdigit()), "{}", frontiere);
    assert_eq!(v.corps, corps_multipart("note.txt", "PARENT", "text/plain", b"contenu\n", &frontiere));
    assert_eq!(v.entete("content-length").parse::<usize>().unwrap(), v.corps.len());
}

#[test]
fn televerser_deux_fois_deux_frontieres_differentes() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "F"})));
    let r = racine("frontieres");
    let f = r.join("a.txt");
    std::fs::write(&f, "x").unwrap();
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    c.televerser(&f, "P", None, None).unwrap();
    c.televerser(&f, "P", None, None).unwrap();
    let v = recu.lock().unwrap();
    assert_ne!(v[0].entete("content-type"), v[1].entete("content-type"));
}

#[test]
fn televerser_un_nom_donne_remplace_celui_du_fichier() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "F"})));
    let r = racine("nom-donne");
    let f = r.join("local.pdf");
    std::fs::write(&f, "%PDF").unwrap();
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    c.televerser(&f, "P", Some("autre.pdf"), None).unwrap();
    let v = recu.lock().unwrap()[0].clone();
    let texte = String::from_utf8_lossy(&v.corps).into_owned();
    assert!(texte.contains("autre.pdf") && !texte.contains("local.pdf") && texte.contains("Content-Type: application/pdf\r\n\r\n%PDF"), "{}", texte);
}

#[test]
fn televerser_remplace_par_un_patch_sur_l_identifiant_connu() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "EXIST", "name": "a.json", "webViewLink": "L"})));
    let r = racine("patch");
    let f = r.join("a.json");
    std::fs::write(&f, "{\"a\": 1}").unwrap();
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    let connu = Distant { id: "EXIST".into(), md5: Some("zz".into()), octets: 3 };
    let d = c.televerser(&f, "PARENT", None, Some(&connu)).unwrap();
    assert!(d.remplace);
    assert_eq!(d.id.as_deref(), Some("EXIST"));
    let v = recu.lock().unwrap();
    // Une seule requete : plus aucune recherche par nom (defauts 3 et 4).
    assert_eq!(v.len(), 1);
    assert_eq!((v[0].methode.as_str(), v[0].chemin.as_str()), ("PATCH", "/upload/drive/v3/files/EXIST"));
    assert_eq!(v[0].param("uploadType").unwrap(), "media");
    assert_eq!(v[0].param("fields").unwrap(), "id,name,webViewLink");
    assert_eq!(v[0].entete("content-type"), "application/json");
    assert_eq!(v[0].corps, b"{\"a\": 1}");
}

#[test]
fn patch_d_un_fichier_vide_part_avec_un_corps_vide() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "E"})));
    let r = racine("patch-vide");
    let f = r.join("vide.txt");
    std::fs::write(&f, "").unwrap();
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    let connu = Distant { id: "E".into(), md5: None, octets: 0 };
    assert!(c.televerser(&f, "P", None, Some(&connu)).unwrap().remplace);
    let v = recu.lock().unwrap()[0].clone();
    assert_eq!(v.methode, "PATCH");
    assert!(v.corps.is_empty());
    assert_eq!(v.entete("content-length"), "0");
}

#[test]
fn televerser_un_fichier_disparu_est_une_file_not_found() {
    let (base, recu) = serveur(|_| json_ok(json!({"id": "E"})));
    let r = racine("disparu");
    let mut c = Client::pour_essai(&r, adresses(&base), "jeton", None);
    let e = c.televerser(&r.join("absent.txt"), "P", None, None).unwrap_err();
    assert_eq!(e.classe, "FileNotFoundError");
    assert!(recu.lock().unwrap().is_empty(), "rien ne part pour un fichier absent");
}

// ------------------------------------------------------------ les erreurs de Google

#[test]
fn reponse_vide_est_un_objet_vide_et_sans_id_est_une_erreur() {
    let (base, _) = serveur(|_| reponse(200, b""));
    let mut c = Client::pour_essai(&racine("vide"), adresses(&base), "jeton", None);
    assert_eq!(c.appel(&format!("{}/drive/v3/files", base), None, None, None).unwrap(), json!({}));
    // Un dossier cree sans `id` est une erreur.
    assert!(c.dossier("x", "root").is_err());
}

#[test]
fn erreurs_de_google_traduites_et_statut_garde() {
    let (base, _) = serveur(|v| match v.chemin.as_str() {
        "/drive/v3/files" => match v.param("q").unwrap_or_default().as_str() {
            q if q.contains("'403'") => reponse(403, br#"{"error": {"status": "PERMISSION_DENIED", "message": "Request had insufficient authentication scopes."}}"#),
            q if q.contains("'404'") => reponse(404, br#"{"error": {"message": "File not found: P."}}"#),
            q if q.contains("'500'") => reponse(500, b"pas du json"),
            _ => json_ok(json!({})),
        },
        _ => json_ok(json!({})),
    });
    let mut c = Client::pour_essai(&racine("erreurs"), adresses(&base), "jeton", None);
    let e = c.lister("403").unwrap_err();
    assert_eq!((e.classe, e.statut), ("ErreurGoogle", Some(403)));
    assert!(e.message.starts_with("Permission refusee"), "{}", e.message);
    let e = c.lister("404").unwrap_err();
    assert_eq!((e.classe, e.statut, e.message.as_str()), ("ErreurGoogle", Some(404), "HTTP 404 : File not found: P."));
    let e = c.lister("500").unwrap_err();
    assert_eq!((e.classe, e.statut, e.message.as_str()), ("ErreurGoogle", Some(500), "HTTP 500 : pas du json"));
}

#[test]
fn json_illisible_dans_une_reponse_ok() {
    let (base, _) = serveur(|_| reponse(200, b"<html>"));
    let mut c = Client::pour_essai(&racine("illisible"), adresses(&base), "jeton", None);
    assert!(c.lister("P").is_err());
}

// ------------------------------------------------------------ defaut 2 : le delai n'est pas « hors ligne »

#[test]
fn defaut_2_un_delai_de_lecture_est_un_incident_pas_une_absence_de_reseau() {
    // Le serveur accepte, lit la requete, puis ne repond qu'apres le delai du client.
    let (base, _) = serveur(|_| {
        std::thread::sleep(Duration::from_millis(2500));
        json_ok(json!({}))
    });
    let mut c = Client::pour_essai(&racine("delai"), adresses(&base), "jeton", None).avec_delai(1);
    let debut = std::time::Instant::now();
    let e = c.lister("P").unwrap_err();
    assert!(debut.elapsed() < Duration::from_millis(2400), "le delai de 1 s doit couper l'appel");
    assert!(e.message.contains("timed out"), "{} / {}", e.classe, e.message);
    assert!(!hors_ligne(&e.message), "un delai de lecture ne doit pas arreter toute la sauvegarde : {}", e.message);
}

#[test]
fn reseau_absent_est_hors_ligne() {
    // Un port libre, sans personne : la connexion est refusee (aucune sortie de la machine).
    let base = crate::outils::port_ferme();
    let mut c = Client::pour_essai(&racine("refuse"), adresses(&base), "jeton", None).avec_delai(5);
    let e = c.lister("P").unwrap_err();
    assert!(e.message.starts_with("Reseau injoignable"), "{}", e.message);
    assert!(hors_ligne(&e.message));
}

// ------------------------------------------------------------ compte et copie de developpement

#[test]
fn compte_pas_lie_est_refuse_avant_tout_reseau() {
    let (base, recu) = serveur(|_| json_ok(json!({})));
    let r = racine("pas-lie");
    std::fs::write(r.join("config").join("google.json"), r#"{"client_id": "i", "client_secret": "s"}"#).unwrap();
    let mut c = Client::nouveau(&r).unwrap().avec_adresses(adresses(&base));
    assert_eq!(c.compte(), None);
    let e = c.lister("P").unwrap_err();
    assert!(e.message.starts_with("Compte Google pas encore lie"), "{}", e.message);
    assert!(recu.lock().unwrap().is_empty());
}

#[test]
fn une_copie_de_developpement_est_refusee_a_la_construction() {
    let r = racine("copie");
    std::fs::write(r.join("config").join("google.json"), r#"{"client_id": "i", "client_secret": "s"}"#).unwrap();
    std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), "{}").unwrap();
    let e = Client::nouveau(&r).err().expect("refus attendu");
    assert_eq!(e.classe, "CopieDeDeveloppement");
}

// ------------------------------------------------------------ « ne supprime jamais »

/// Le trait `Drive` ne porte que six methodes (deux de lecture pour la restauration, lot R1), et aucune ne supprime : si
/// quelqu'un en ajoute une (`supprimer`, `effacer`, `corbeille`...), ce test echoue et force a le dire. La garantie « ne
/// supprime jamais rien dans le Drive » est dans le TYPE : la sauvegarde, l'archivage et la restauration ne recoivent qu'un
/// `&mut dyn Drive`.
#[test]
fn le_trait_drive_n_a_pas_de_suppression() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("drive.rs")).unwrap();
    let debut = source.find("pub trait Drive {").expect("le trait Drive");
    let corps = &source[debut..];
    let fin = corps.find("\n}\n").expect("fin du trait");
    let trait_ = &corps[..fin];
    let methodes: Vec<&str> = trait_
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.strip_prefix("fn "))
        .map(|l| l.split(|c: char| c == '(' || c == '<').next().unwrap())
        .collect();
    assert_eq!(methodes, ["compte", "chemin", "lister", "televerser", "sous_dossiers", "telecharger"], "le trait Drive a change : {:?}", methodes);
    for m in &methodes {
        for mot in ["suppr", "supprim", "effac", "delete", "remove", "corbeille", "trash", "detruire"] {
            assert!(!m.to_lowercase().contains(mot), "une methode de suppression dans Drive : {}", m);
        }
    }
    // Et le client n'appelle jamais DELETE ni ne met a la corbeille.
    let client = &source[..debut + corps.len()];
    assert!(!client.contains("\"DELETE\"") && !client.contains("trashed\": true") && !client.contains("\"trashed\":true"));
}
