//! Le harnais de la preuve du lot H3a (messagerie) : appelle `envoi` et `courriel` depuis un script de comparaison
//! avec l'ancien Python. Ce n'est PAS un test et rien d'ici n'est dans le binaire : il lit un JSON et ecrit un JSON
//! ou des fichiers, dans un dossier de travail que le script donne.
//!
//!     h3a_preuve mime SPEC.json SORTIE/       un `.eml` par cas (Date, Message-ID, frontieres figes par le cas)
//!     h3a_preuve dates T... | -               `envoi::date_de` des instants unix donnes (ou lus sur l'entree standard), un par ligne
//!     h3a_preuve preparer RACINE MOIS DOSSIER SORTIE.json [fiche]
//!     h3a_preuve envoyer SPEC.json SORTIE/    `envoyer` contre un faux client (echec au n-ieme)
//!     h3a_preuve ecrire RACINE MOIS           `pieces_individuelles` par la route `ecrire-documents`
//!     h3a_preuve lire RACINE                  `demonstration-courriels` : les courriels joues, relus
//!     h3a_preuve boite RACINE MESSAGE.eml     `BoiteSimulee::envoyer`
//!     h3a_preuve client RACINE SCENARIO.json SORTIE.json   le vrai `Client` contre des serveurs locaux
//!
//! Aucun mot de passe n'est imprime : le client le lit dans le coffre de la RACINE donnee (un coffre fictif).

use std::path::{Path, PathBuf};

use paie::calcul::{calculer, Paie};
use paie::contexte::Contexte;
use paie::courriel::{self, BoiteSimulee, Client, Courriel, ErreurCourriel};
use paie::date::d;
use paie::engagement::{engagements_du_fichier, LignesPermanent};
use paie::envoi::{self, Hasards, Message};
use serde_json::{json, Value};

fn lire_json(p: &str) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn texte(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

fn message_de(v: &Value) -> Message {
    Message {
        cle: texte(v, "cle"),
        nom: texte(v, "nom"),
        destinataire: texte(v, "dest"),
        sujet: texte(v, "sujet"),
        corps: texte(v, "corps"),
        pieces: v["pieces"].as_array().map(|a| a.iter().map(|p| PathBuf::from(p.as_str().unwrap())).collect()).unwrap_or_default(),
        html: texte(v, "html"),
    }
}

fn message_en_json(m: &Message) -> Value {
    json!({
        "cle": m.cle, "nom": m.nom, "dest": m.destinataire, "sujet": m.sujet, "corps": m.corps, "html": m.html,
        "pieces": m.pieces.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
    })
}

fn mime(spec: &str, sortie: &str) {
    std::fs::create_dir_all(sortie).unwrap();
    let cas = lire_json(spec);
    for c in cas.as_array().unwrap() {
        let nom = texte(c, "nom");
        let m = message_de(c);
        let exp = texte(c, "exp");
        let mut compteur = 0u64;
        let mut frontiere = move || {
            compteur += 1;
            format!("{}{:019}==", "=".repeat(15), 1_000_000_000_000_000_000u64 + compteur)
        };
        let r = envoi::en_mime_avec(&m, &exp, Hasards { date: texte(c, "date"), message_id: texte(c, "mid"), frontiere: &mut frontiere });
        match r {
            Ok(o) => std::fs::write(Path::new(sortie).join(format!("{}.eml", nom)), o).unwrap(),
            Err(e) => std::fs::write(Path::new(sortie).join(format!("{}.err", nom)), e).unwrap(),
        }
    }
}

fn demo(racine: &str) -> Contexte {
    Contexte::nouveau(Path::new(racine), Some(d(&json!("2026-09-27")).unwrap())).unwrap()
}

fn paies_du_mois(racine: &str, ctx: &Contexte, mois: &str) -> Vec<Paie> {
    let lignes: LignesPermanent = &paie::conges::lignes_du_mois;
    let (_, engs, _) = engagements_du_fichier(ctx, &Path::new(racine).join("paies").join(format!("{}.json", mois)), lignes).unwrap();
    engs.iter().map(|e| calculer(ctx, e).unwrap()).collect()
}

fn preparer(racine: &str, mois: &str, dossier: &str, sortie: &str, avec_fiche: bool) {
    let ctx = demo(racine);
    let paies = paies_du_mois(racine, &ctx, mois);
    let fiches = ctx.salaries.clone();
    let fiche = move |cle: &str| -> Value { fiches[cle].clone() };
    let f: Option<&dyn Fn(&str) -> Value> = if avec_fiche { Some(&fiche) } else { None };
    let r = envoi::preparer(&paies, &ctx.entreprise, mois, Path::new(dossier), f);
    let v = match r {
        Ok(ms) => json!({"messages": ms.iter().map(message_en_json).collect::<Vec<_>>()}),
        Err(e) => json!({"erreur": e}),
    };
    // Le recapitulatif de chaque salarie, a part : meme calcul que `preparer`, plus direct a comparer.
    let mut recaps = serde_json::Map::new();
    let ent_nom = ctx.entreprise["nom_affichage"].as_str().unwrap_or("").to_string();
    let mut cles: Vec<String> = paies.iter().map(|p| p.salarie["_cle"].as_str().unwrap().to_string()).collect();
    cles.dedup();
    for cle in cles {
        let siennes: Vec<&Paie> = paies.iter().filter(|p| p.salarie["_cle"] == cle.as_str()).collect();
        let r = envoi::recap_france_travail(&siennes, mois, &ent_nom);
        recaps.insert(cle, match r { Ok(t) => json!(t), Err(e) => json!({"erreur": e}) });
    }
    let mut v = v;
    v["recaps"] = Value::Object(recaps);
    std::fs::write(sortie, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

/// Un faux client : il garde ce qu'on lui passe, et refuse au n-ieme envoi.
struct Faux {
    partis: Vec<Vec<u8>>,
    echoue_au: Option<usize>,
}

impl Courriel for Faux {
    fn compte(&self) -> &str {
        "compagnie@gmail.com"
    }
    fn envoyer(&mut self, mime: &[u8]) -> Result<String, ErreurCourriel> {
        if self.echoue_au == Some(self.partis.len() + 1) {
            return Err(ErreurCourriel("La boite refuse".into()));
        }
        self.partis.push(mime.to_vec());
        Ok(format!("<fausse-{}@gmail.com>", self.partis.len()))
    }
    fn rechercher(&mut self, _: &str, _: usize) -> Result<Vec<String>, ErreurCourriel> {
        Ok(vec![])
    }
    fn message_brut(&mut self, _: &str) -> Result<Vec<u8>, ErreurCourriel> {
        Ok(vec![])
    }
    fn fermer(&mut self) {}
}

fn envoyer(spec: &str, sortie: &str) {
    std::fs::create_dir_all(sortie).unwrap();
    let s = lire_json(spec);
    let messages: Vec<Message> = s["messages"].as_array().unwrap().iter().map(message_de).collect();
    let mut faux = Faux { partis: Vec::new(), echoue_au: s["echoue_au"].as_u64().map(|n| n as usize) };
    let (mut ids, mut copies) = (Vec::new(), Vec::new());
    let r = envoi::envoyer(&messages, &texte(&s, "exp"), &mut faux, Some(&mut ids), Some(&mut copies));
    for (i, o) in faux.partis.iter().enumerate() {
        std::fs::write(Path::new(sortie).join(format!("passe-{}.eml", i)), o).unwrap();
    }
    for (i, o) in copies.iter().enumerate() {
        std::fs::write(Path::new(sortie).join(format!("copie-{}.eml", i)), o).unwrap();
    }
    let v = match r {
        Ok(p) => json!({"partis": p, "identifiants": ids, "nb_passes": faux.partis.len(), "nb_copies": copies.len()}),
        Err(e) => json!({"erreur": e, "identifiants": ids, "nb_passes": faux.partis.len(), "nb_copies": copies.len()}),
    };
    std::fs::write(Path::new(sortie).join("resultat.json"), serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

/// `envoi::pieces_individuelles` par la route `ecrire-documents` (le redecoupage d'un mois produit).
fn ecrire(racine: &str, mois: &str) {
    let ctx = demo(racine);
    match paie::ecrire_documents::ecrire_documents(&ctx, &json!({ "mois": mois })) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("{}", json!({"erreur": e.message})),
    }
}

/// `demonstration-courriels` : les courriels joues d'une racine de demonstration.
fn lire(racine: &str) {
    let ctx = demo(racine);
    match paie::simulation_courriels::demonstration_courriels(&ctx, &json!({})) {
        Ok(v) => println!("{}", v),
        Err(e) => println!("{}", json!({"erreur": e.message})),
    }
}

fn boite(racine: &str, eml: &str) {
    let ctx = demo(racine);
    let mut b = BoiteSimulee::du_contexte(&ctx);
    let ident = b.envoyer(&std::fs::read(eml).unwrap()).unwrap();
    println!("{}", json!({"compte": b.compte(), "ident": ident}));
}

fn client(racine: &str, scenario: &str, sortie: &str) {
    let sc = lire_json(scenario);
    let mut resultats = Vec::new();
    let mut c = Client::nouveau(Path::new(racine));
    for op in sc.as_array().unwrap() {
        let nom = texte(op, "op");
        let r: Value = match nom.as_str() {
            "verifier" => match courriel::verifier(Path::new(racine)) {
                Ok(v) => v,
                Err(e) => json!({"erreur": e.0}),
            },
            _ => match c.as_mut() {
                Err(e) => json!({"erreur": e.0.clone()}),
                Ok(c) => match nom.as_str() {
                    "compte" => json!({"compte": c.compte()}),
                    "envoyer" => {
                        let mime = std::fs::read(texte(op, "fichier")).unwrap();
                        match c.envoyer(&mime) {
                            Ok(i) => json!({"ident": i}),
                            Err(e) => json!({"erreur": e.0}),
                        }
                    }
                    "rechercher" => match c.rechercher(&texte(op, "texte"), op["max"].as_u64().unwrap_or(50) as usize) {
                        Ok(u) => json!({"uids": u}),
                        Err(e) => json!({"erreur": e.0}),
                    },
                    "brut" => match c.message_brut(&texte(op, "uid")) {
                        Ok(o) => {
                            let f = texte(op, "vers");
                            std::fs::write(&f, &o).unwrap();
                            json!({"octets": o.len()})
                        }
                        Err(e) => json!({"erreur": e.0}),
                    },
                    "fermer" => {
                        c.fermer();
                        json!({"ok": true})
                    }
                    autre => json!({"erreur": format!("operation inconnue {}", autre)}),
                },
            },
        };
        resultats.push(r);
    }
    std::fs::write(sortie, serde_json::to_vec_pretty(&Value::Array(resultats)).unwrap()).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("mime") => mime(&a[1], &a[2]),
        Some("dates") => {
            // `dates -` : les instants viennent de l'entree standard, un par ligne.
            let instants: Vec<String> = if a.get(1).map(String::as_str) == Some("-") {
                std::io::read_to_string(std::io::stdin()).unwrap().lines().map(String::from).collect()
            } else {
                a[1..].to_vec()
            };
            let mut sortie = String::new();
            for t in &instants {
                sortie.push_str(&envoi::date_de(t.trim().parse().unwrap()));
                sortie.push('\n');
            }
            print!("{}", sortie);
        }
        Some("preparer") => preparer(&a[1], &a[2], &a[3], &a[4], a.get(5).map(String::as_str) == Some("fiche")),
        Some("envoyer") => envoyer(&a[1], &a[2]),
        Some("ecrire") => ecrire(&a[1], &a[2]),
        Some("lire") => lire(&a[1]),
        Some("boite") => boite(&a[1], &a[2]),
        Some("client") => client(&a[1], &a[2], &a[3]),
        _ => eprintln!("usage : voir l'en-tete"),
    }
}
