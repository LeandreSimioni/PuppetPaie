//! `journal-reprise` (`moteur/operations_retours.reconstruire_journal`) : remet au journal les depots DSN que
//! les retours archives (`retours/*.xml`) attestent. `ecrire` absent : elle DIT ce qu'elle ferait. Aucun reseau,
//! aucun depot ; pas de mot de passe (rien ne part).
//!
//! Idempotente : un flux deja au journal est saute. Une ligne reprise porte la date du depot (lue dans la piece)
//! et sa `origine` (`retours/<piece>`) : c'est une reconstitution, pas un acte de ce logiciel.
//!
//! Une piece qui ne se lit pas (fichier inaccessible, XML mal forme) est NOMMEE dans `illisibles`, « nom (classe) »
//! comme le Python, et les autres passent (reste 164) ; un XML bien forme qui n'est pas un bilan de depot est saute sans
//! bruit : le dossier melange des retours de toutes natures.

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::etats::{erreur, R};
use crate::fichiers::vrai;
use crate::journal::{self, Note};

fn texte(b: &Value, cle: &str) -> String {
    b.get(cle).filter(|v| vrai(v)).map(crate::etats::py_str).unwrap_or_default()
}

fn accepte(b: &Value) -> bool {
    let etat = if vrai(&b["etat_declaration"]) { texte(b, "etat_declaration") } else { texte(b, "etat_envoi") };
    etat.to_uppercase() == "OK"
}

/// Un depot dont une piece de `retours/` temoigne.
struct Atteste {
    flux: String,
    bilan: Value,
    pieces: Vec<String>,
    deja: bool,
}

/// `operations_retours.depots_attestes` : lecture pure.
fn depots_attestes(ctx: &Contexte) -> R<(Vec<Atteste>, Vec<String>)> {
    let dossier = ctx.racine.join("retours");
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    let mut noms: Vec<String> = std::fs::read_dir(&dossier)
        .map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".xml"))
        .collect();
    noms.sort();
    let mut par_flux: Vec<(String, Vec<(Value, String)>)> = vec![];
    let mut illisibles: Vec<String> = vec![];
    for nom in &noms {
        let octets = match std::fs::read(dossier.join(nom)) {
            Ok(o) => o,
            Err(_) => {
                illisibles.push(format!("{} (OSError)", nom));
                continue;
            }
        };
        let b = match crate::retours::bilan_ou_illisible(&octets) {
            Ok(b) => b,
            Err(classe) => {
                illisibles.push(format!("{} ({})", nom, classe));
                continue;
            }
        };
        if b.is_null() {
            continue;
        }
        let flux = texte(&b, "flux");
        match par_flux.iter_mut().find(|(f, _)| *f == flux) {
            Some((_, l)) => l.push((b, nom.clone())),
            None => par_flux.push((flux, vec![(b, nom.clone())])),
        }
    }
    let connus: Vec<String> = journal::lire(&ctx.racine, None, None, false)
        .map_err(crate::retours::Echec::from)?
        .iter()
        .filter_map(|l| l.get("flux").filter(|v| vrai(v)).map(crate::etats::py_str))
        .collect();
    let mut out: Vec<Atteste> = par_flux
        .into_iter()
        .map(|(flux, couples)| {
            let bilans: Vec<Value> = couples.iter().map(|(b, _)| b.clone()).collect();
            let mut pieces: Vec<String> = couples.into_iter().map(|(_, n)| n).collect();
            pieces.sort();
            Atteste { deja: connus.contains(&flux), flux, bilan: crate::retours::fusionner_bilans(&bilans), pieces }
        })
        .collect();
    out.sort_by(|a, b| texte(&a.bilan, "recu_le").cmp(&texte(&b.bilan, "recu_le")));
    Ok((out, illisibles))
}

/// La route `journal-reprise`.
pub fn reprise(ctx: &Contexte, charge: &Value, maintenant: &str) -> R<Value> {
    let ecrire = charge.get("ecrire").is_some_and(vrai);
    let qui = crate::routes_ecritures_g2::qui_de(charge);
    let (attestes, illisibles) = depots_attestes(ctx)?;
    let (mut trouves, mut ajoutes, mut deja, mut sans_mois) = (vec![], vec![], vec![], vec![]);
    for d in &attestes {
        let b = &d.bilan;
        let reel = texte(b, "essai_reel") == "02";
        // UN TEST QUE CE LOGICIEL N'A PAS ENVOYE NE S'AFFICHE PAS ET NE SE REPREND PAS (Leandre, 20/10/2026) : sous le
        // meme compte net-entreprises, les essais de l'atelier (ligne de commande, `paie --dev`, seances du nuage)
        // reviennent dans les retours de la compagnie. Un test n'engage rien : absent du journal, il vient d'ailleurs.
        // Il compte pourtant pour l'ordre (`ordre_signalements::dernier_ordre_des_mensuelles`, lu en silence). Un REEL
        // absent du journal est un depot de la compagnie (l'atelier n'en fait jamais) : il se reprend.
        if !reel && !d.deja {
            continue;
        }
        let vue = json!({
            "flux": d.flux,
            "quand": texte(b, "recu_le"),
            "mois": texte(b, "mois"),
            "mode": if reel { "reel" } else { "test" },
            "accepte": accepte(b),
            "verdict_connu": vrai(&b["etat_declaration"]),
            "logiciel": texte(b, "logiciel"),
            "pieces": d.pieces,
            "deja": d.deja,
        });
        trouves.push(vue.clone());
        if d.deja {
            deja.push(vue);
            continue;
        }
        if texte(b, "recu_le").is_empty() {
            // Sans date, la ligne ne saurait pas dire QUAND : on refuse plutot que de dater d'aujourd'hui.
            sans_mois.push(vue);
            continue;
        }
        ajoutes.push(vue);
    }
    let mut rapport = Map::new();
    rapport.insert("trouves".into(), Value::Array(trouves));
    rapport.insert("ajoutes".into(), Value::Array(ajoutes.clone()));
    rapport.insert("deja".into(), Value::Array(deja));
    rapport.insert("sans_mois".into(), Value::Array(sans_mois));
    rapport.insert("illisibles".into(), json!(illisibles));
    rapport.insert("ecrit".into(), json!(ecrire));
    rapport.insert("rattaches".into(), json!(0));
    if !ecrire {
        return Ok(Value::Object(rapport));
    }
    for vue in &ajoutes {
        let Some(d) = attestes.iter().find(|x| Some(x.flux.as_str()) == vue["flux"].as_str()) else { continue };
        let b = &d.bilan;
        let reel = texte(b, "essai_reel") == "02";
        let mois = texte(b, "mois");
        let origine = format!("retours/{}", d.pieces.last().map_or("?", String::as_str));
        let reponse = texte(b, "certificat");
        let quand = texte(b, "recu_le");
        let detail = json!({
            "reconstruit": true,
            "logiciel": texte(b, "logiciel"),
            "type": texte(b, "type_declaration"),
            "fraction": texte(b, "fraction"),
            "ordre": texte(b, "ordre"),
            "salaries": texte(b, "salaries"),
            "contrats": texte(b, "contrats"),
            "etat_envoi": texte(b, "etat_envoi"),
            "etat_declaration": texte(b, "etat_declaration"),
            "verdict_connu": vrai(&b["etat_declaration"]),
            "pieces": d.pieces,
        });
        let note = Note {
            quoi: "dsn",
            mois: if mois.is_empty() { None } else { Some(&mois) },
            fichier: None,
            mode: if reel { "reel" } else { "test" },
            qui: &qui,
            accepte: accepte(b),
            destinataire: "net-entreprises",
            flux: &d.flux,
            reponse: &reponse,
            trace: None,
            detail: Some(&detail),
            quand: &quand,
            origine: &origine,
        };
        journal::noter(&ctx.racine, &note, maintenant).map_err(crate::retours::Echec::from)?;
    }
    // Les retours deja archives se rangent en face des lignes qui viennent d'apparaitre -- par le flux.
    let p = ctx.racine.join("retours").join("journal.json");
    if p.exists() {
        let texte_j = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        let retours: Value = serde_json::from_slice(&texte_j).map_err(|e| erreur("JSONDecodeError", e.to_string()))?;
        let n = crate::etats::rattacher_retours_ecrits(&ctx.racine, &retours)?;
        rapport.insert("rattaches".into(), json!(n));
    }
    Ok(Value::Object(rapport))
}

// ------------------------------------------------------------- la reprise du canal DPAE

/// `_salarie_par_nir` : le NIR (sans espaces autour) -> la cle de la fiche. Si deux fiches portent le meme NIR, la derniere gagne
/// (comme le dictionnaire du Python).
fn salarie_par_nir(ctx: &Contexte) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for (cle, s) in &ctx.salaries {
        let Some(nir) = s.get("nir").filter(|n| vrai(n)).and_then(Value::as_str) else { continue };
        let nir = nir.trim().to_string();
        match v.iter_mut().find(|(n, _)| *n == nir) {
            Some(e) => e.1 = cle.clone(),
            None => v.push((nir, cle.clone())),
        }
    }
    v
}

/// `_flux_du_nom` : `20260904-224552_nature41_WSxxxx.txt` -> `WSxxxx`.
fn flux_du_nom(nom: &str) -> String {
    let reste = nom.splitn(3, '_').last().unwrap_or(nom);
    reste.rsplit_once('.').map_or(reste, |(a, _)| a).to_string()
}

/// `operations_retours.embauches_attestees` : les DPAE dont `retours/dpae/` porte le certificat (nature 41). Une entree par FLUX, avec
/// les embauches qu'il declare ; les NIR qu'on ne sait rattacher a aucune fiche sont DITS, jamais devines. LECTURE PURE.
fn embauches_attestees(ctx: &Contexte) -> (Vec<Value>, Vec<String>) {
    let dossier = ctx.racine.join("retours").join("dpae");
    let par_nir = salarie_par_nir(ctx);
    let mut noms: Vec<String> = std::fs::read_dir(&dossier)
        .map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("nature41")).collect())
        .unwrap_or_default();
    noms.sort();
    let (mut sortie, mut inconnus): (Vec<Value>, Vec<String>) = (vec![], vec![]);
    for nom in &noms {
        let octets = match std::fs::read(dossier.join(nom)) {
            Ok(o) => o,
            Err(_) => {
                inconnus.push(format!("{} (OSError)", nom));
                continue;
            }
        };
        let texte: String = octets.iter().map(|&b| b as char).collect();
        let (mut embauches, mut quand, mut mois) = (Vec::<Value>::new(), String::new(), String::new());
        for ligne in crate::dpae::relire(&texte) {
            let champ = |k: &str| ligne.iter().find(|(n, _)| *n == k).map(|(_, v)| v.trim().to_string()).unwrap_or_default();
            let (nir, emb) = (champ("nir"), champ("date_embauche"));
            let e: Vec<char> = emb.chars().collect();
            if e.len() != 8 {
                continue;
            }
            let tr = |a: usize, b: usize| -> String { e[a..b].iter().collect() };
            let debut = format!("{}-{}-{}", tr(4, 8), tr(2, 4), tr(0, 2));
            let Some((_, cle)) = par_nir.iter().find(|(n, _)| *n == nir) else {
                // On ne devine pas qui c'est : un NIR qu'on ne reconnait pas est peut-etre une fiche archivee sous une autre cle, ou
                // une declaration faite pour quelqu'un d'autre.
                inconnus.push(format!("NIR {} ({}), embauche du {}", nir, nom, debut));
                continue;
            };
            embauches.push(json!({"salarie": cle, "debut": debut}));
            if mois.is_empty() {
                mois = debut.chars().take(7).collect();
            }
            let h: Vec<char> = champ("horodatage").chars().collect();
            if quand.is_empty() && h.len() == 12 {
                let t = |a: usize, b: usize| -> String { h[a..b].iter().collect() };
                quand = format!("{}-{}-{}T{}:{}:00", t(4, 8), t(2, 4), t(0, 2), t(8, 10), t(10, 12));
            }
        }
        if !embauches.is_empty() {
            sortie.push(json!({"flux": flux_du_nom(nom), "quand": quand, "mois": mois, "piece": nom, "embauches": embauches}));
        }
    }
    sortie.sort_by(|a, b| texte(a, "quand").cmp(&texte(b, "quand")));
    (sortie, inconnus)
}

/// `operations_retours.reconstruire_journal_dpae` : remet au journal les DPAE que les certificats de l'Urssaf attestent. Memes formes
/// et memes regles que la reprise DSN : `ecrire` a faux par defaut (elle DIT), idempotente par le flux, chaque ligne porte `origine`.
/// Rend `{"trouves", "ajoutes", "deja", "inconnus", "ecrit"}`. Aucun reseau, aucun depot, pas de mot de passe.
pub fn reprise_dpae(ctx: &Contexte, ecrire: bool, qui: &str, maintenant: &str) -> R<Value> {
    let (attestees, inconnus) = embauches_attestees(ctx);
    let connus: Vec<String> = journal::lire(&ctx.racine, None, None, false)
        .map_err(crate::retours::Echec::from)?
        .iter()
        .filter_map(|l| l.get("flux").filter(|v| vrai(v)).map(crate::etats::py_str))
        .collect();

    // Le bilan XML du meme flux, quand il est la : il porte l'idflux ENTIER et dit si le depot etait reel ou de test.
    let dossier = ctx.racine.join("retours").join("dpae");
    let mut noms: Vec<String> = std::fs::read_dir(&dossier)
        .map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".xml")).collect())
        .unwrap_or_default();
    noms.sort();
    let mut bilans: Vec<(String, Vec<Value>)> = Vec::new();
    for nom in &noms {
        let Ok(octets) = std::fs::read(dossier.join(nom)) else { continue };
        let b = crate::retours::bilan_d_octets(&octets);
        if b.is_null() {
            continue;
        }
        let flux = texte(&b, "flux");
        match bilans.iter_mut().find(|(f, _)| *f == flux) {
            Some((_, l)) => l.push(b),
            None => bilans.push((flux, vec![b])),
        }
    }

    let (mut trouves, mut ajoutes, mut deja) = (Vec::<Value>::new(), Vec::<Value>::new(), Vec::<Value>::new());
    for e in &attestees {
        // Le nom de fichier tronque le flux ; le bilan XML porte l'entier. On rattache par prefixe, et on garde l'entier quand on le trouve.
        let court = texte(e, "flux");
        let entier = bilans.iter().map(|(f, _)| f.clone()).find(|f| f.starts_with(&court)).unwrap_or(court);
        let b = bilans.iter().find(|(f, _)| *f == entier).map(|(_, l)| crate::retours::fusionner_bilans(l));
        let mut vue = e.clone();
        vue["flux"] = json!(entier);
        vue["mode"] = json!(match &b {
            Some(b) if texte(b, "essai_reel") != "02" => "test",
            _ => "reel",
        });
        vue["accepte"] = json!(b.as_ref().map_or(true, accepte));
        let deja_la = connus.contains(&entier);
        vue["deja"] = json!(deja_la);
        trouves.push(vue.clone());
        if deja_la {
            deja.push(vue);
        } else {
            ajoutes.push(vue);
        }
    }

    if ecrire {
        for vue in &ajoutes {
            let mois = texte(vue, "mois");
            let origine = format!("retours/dpae/{}", texte(vue, "piece"));
            let detail = json!({"reconstruit": true, "embauches": vue["embauches"], "certificat": true});
            let note = Note {
                quoi: "dpae",
                mois: if mois.is_empty() { None } else { Some(&mois) },
                fichier: None,
                mode: if texte(vue, "mode") == "test" { "test" } else { "reel" },
                qui,
                accepte: vrai(&vue["accepte"]),
                destinataire: "urssaf-dpae",
                flux: &texte(vue, "flux"),
                reponse: "",
                trace: None,
                detail: Some(&detail),
                quand: &texte(vue, "quand"),
                origine: &origine,
            };
            journal::noter(&ctx.racine, &note, maintenant).map_err(crate::retours::Echec::from)?;
        }
    }
    Ok(json!({"trouves": trouves, "ajoutes": ajoutes, "deja": deja, "inconnus": inconnus, "ecrit": ecrire}))
}
