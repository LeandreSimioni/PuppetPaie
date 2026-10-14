//! `envoi::en_mime`, a l'octet pres, contre `tests/donnees/envoi_mime/` (une demande `.json`, le message `.eml` attendu,
//! Date et Message-ID fixes, frontieres masquees) : le pliage RFC 2047 des sujets, les noms de pieces (RFC 2231), le
//! quoted-printable et le base64 du corps, les types MIME. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit les `.eml`.

use std::path::{Path, PathBuf};

use crate::outils::{dossier_de_test, reecrire_oracles};
use paie::envoi::{self, Hasards, Message};
use serde_json::Value;

fn dossier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("donnees").join("envoi_mime")
}

fn octets(taille: usize) -> Vec<u8> {
    (0..taille).map(|i| ((i * 7 + 3) % 256) as u8).collect()
}

/// `={15}\d{19}={2}(\.\d+)?` -> `<BORNE>`.
fn masquer(o: &[u8]) -> Vec<u8> {
    let mut s = Vec::new();
    let mut i = 0;
    while i < o.len() {
        let ok = o[i..].len() >= 36
            && o[i..i + 15].iter().all(|c| *c == b'=')
            && o[i + 15..i + 34].iter().all(|c| c.is_ascii_digit())
            && &o[i + 34..i + 36] == b"==";
        if ok {
            s.extend_from_slice(b"<BORNE>");
            i += 36;
            if o.get(i) == Some(&b'.') {
                let mut j = i + 1;
                while j < o.len() && o[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 {
                    i = j;
                }
            }
        } else {
            s.push(o[i]);
            i += 1;
        }
    }
    s
}

fn produire(spec: &Value, travail: &Path) -> Result<Vec<u8>, String> {
    let texte = |k: &str| spec[k].as_str().unwrap().to_string();
    let mut pieces = Vec::new();
    for p in spec["pieces"].as_array().unwrap() {
        let f = travail.join(p["nom"].as_str().unwrap());
        std::fs::write(&f, octets(p["taille"].as_u64().unwrap() as usize)).unwrap();
        pieces.push(f);
    }
    let m = Message {
        cle: "x".into(),
        nom: "X".into(),
        destinataire: texte("dest"),
        sujet: texte("sujet"),
        corps: texte("corps"),
        pieces,
        html: texte("html"),
    };
    let exp = texte("exp");
    let domaine = exp.rsplit('@').next().unwrap().to_string();
    let mut compteur = 0u64;
    let mut frontiere = move || {
        compteur += 1;
        format!("{}{:019}==", "=".repeat(15), 1_000_000_000_000_000_000u64 + compteur)
    };
    envoi::en_mime_avec(
        &m,
        &exp,
        Hasards {
            date: "Tue, 06 Oct 2026 21:26:31 +0200".into(),
            message_id: format!("<FIXE.1.2@{}>", domaine),
            frontiere: &mut frontiere,
        },
    )
}

#[test]
fn en_mime_rend_les_octets_attendus() {
    let mut noms: Vec<String> = std::fs::read_dir(dossier())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter_map(|n| n.strip_suffix(".json").map(String::from))
        .collect();
    noms.sort();
    assert!(noms.len() > 100, "les cas manquent : {}", noms.len());
    let racine = dossier_de_test("envoi_mime", "cas");
    let mut ecarts: Vec<String> = Vec::new();
    for nom in &noms {
        let spec: Value = serde_json::from_slice(&std::fs::read(dossier().join(format!("{}.json", nom))).unwrap()).unwrap();
        let attendu = std::fs::read(dossier().join(format!("{}.eml", nom))).unwrap();
        let travail = racine.join(nom);
        std::fs::create_dir_all(&travail).unwrap();
        match produire(&spec, &travail) {
            Err(e) => ecarts.push(format!("{} : refus {}", nom, e)),
            Ok(o) => {
                let o = masquer(&o);
                if reecrire_oracles() {
                    std::fs::write(dossier().join(format!("{}.eml", nom)), &o).unwrap();
                } else if o != attendu {
                    let a = String::from_utf8_lossy(&attendu).into_owned();
                    let b = String::from_utf8_lossy(&o).into_owned();
                    let (la, lb): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
                    let i = (0..la.len().max(lb.len())).find(|i| la.get(*i) != lb.get(*i)).unwrap_or(0);
                    ecarts.push(format!(
                        "{} : ligne {} -- attendu {:?} / obtenu {:?}",
                        nom,
                        i + 1,
                        la.get(i).unwrap_or(&"<fin>"),
                        lb.get(i).unwrap_or(&"<fin>")
                    ));
                }
            }
        }
    }
    assert!(ecarts.is_empty(), "{} ecart(s) sur {} cas :\n{}", ecarts.len(), noms.len(), ecarts.join("\n"));
}
