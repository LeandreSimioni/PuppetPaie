//! LE JOURNAL DE CE QUI EST PARTI : `journal/depots.json`, une liste a laquelle on n'ajoute qu'a la fin (`noter`) et
//! qu'on relit filtree (`lire`), l'empreinte d'un fichier, et ce qui est parti de chaque engagement, sur quelle version
//! (`lu` : une lecture, d'ou `departs`, que `moteur` passe a l'engagement). Les etats du mois, les resumes et le rattachement des retours sont dans
//! `etats`.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use socle::ecriture::ecrire_json;
use socle::fichiers::{repr, vrai};
use socle::num::Resultat;

/// Les actes que l'on note ; un autre est refuse.
pub const ACTES: [&str; 8] = [
    "dsn",
    "dpae",
    "documents",
    "contrat-signature",
    "contrat-contresigne",
    "regularisation-casc",
    "dossier-salarie",
    "fiche-demande",
];

pub const REEL: &str = "reel";

/// `<racine>/journal/depots.json`.
pub fn chemin(racine: &Path) -> PathBuf {
    racine.join("journal").join("depots.json")
}

/// LA TRACE D'UN DEPOT (`trace` d'une ligne du journal) : relative a la compagnie (elle
/// peut changer de dossier), jointe a `racine` ; un chemin absolu se lit tel quel.
pub fn trace_de(racine: &Path, ligne: &serde_json::Value) -> Option<PathBuf> {
    let t = ligne.get("trace")?.as_str().filter(|t| !t.is_empty())?;
    let p = Path::new(t);
    Some(if p.is_absolute() { p.to_path_buf() } else { racine.join(p) })
}

/// Les 16 premiers caracteres du SHA-256.
pub fn empreinte(contenu: &[u8]) -> String {
    let h = Sha256::digest(contenu);
    h.iter().take(8).map(|o| format!("{:02x}", o)).collect()
}

/// L'empreinte d'un fichier, comme le journal l'écrit.
pub fn empreinte_fichier(chemin: &Path) -> Resultat<String> {
    std::fs::read(chemin).map(|b| empreinte(&b)).map_err(|e| format!("{} : {}", chemin.display(), e))
}

/// LE JOURNAL LU : ses lignes (ce qui est parti et ce qui en est revenu : bilans, contrats signes, anomalies), et ce qui
/// est parti de chaque engagement, sur quelle version (`departs`), la part que le moteur passe a l'engagement. Une seule
/// lecture : tiree une fois par contenu du fichier (`socle::fichiers::json_partage`), la meme valeur partagee pour
/// chaque lecteur d'un geste tant que le journal ne change pas.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    pub lignes: std::sync::Arc<Vec<Value>>,
    pub departs: engagement::departs::Departs,
}

/// `journal/depots.json` lu (`Journal`) ; vide s'il n'existe pas.
pub fn lu(racine: &Path) -> Resultat<Journal> {
    type Tires = std::sync::Mutex<std::collections::HashMap<PathBuf, (std::sync::Arc<Value>, Journal)>>;
    static TIRES: std::sync::OnceLock<Tires> = std::sync::OnceLock::new();
    let p = chemin(racine);
    let Some(v) = socle::fichiers::json_partage(&p)? else { return Ok(Journal::default()) };
    socle::fichiers::noter_si_besoin(&p);
    let tires = TIRES.get_or_init(Default::default);
    if let Some((vu, j)) = tires.lock().unwrap_or_else(|e| e.into_inner()).get(&p) {
        if std::sync::Arc::ptr_eq(vu, &v) {
            return Ok(j.clone());
        }
    }
    let Value::Array(lignes) = v.as_ref() else { return Err(format!("{} : une liste etait attendue", p.display())) };
    let j = Journal { departs: departs_des_lignes(lignes), lignes: std::sync::Arc::new(lignes.clone()) };
    socle::fichiers::ranger(tires, p, (v.clone(), j.clone()));
    Ok(j)
}

/// Les lignes, les plus recentes en dernier. Un filtre vide
/// ne filtre pas.
pub fn lire(racine: &Path, quoi: Option<&str>, mois: Option<&str>, reel_seulement: bool) -> Resultat<Vec<Value>> {
    let mut lignes = (*lu(racine)?.lignes).clone();
    let egal = |l: &Value, c: &str, v: &str| l.get(c).and_then(Value::as_str) == Some(v);
    if let Some(q) = quoi.filter(|q| !q.is_empty()) {
        lignes.retain(|l| egal(l, "quoi", q));
    }
    if let Some(m) = mois.filter(|m| !m.is_empty()) {
        lignes.retain(|l| egal(l, "mois", m));
    }
    if reel_seulement {
        lignes.retain(|l| egal(l, "mode", REEL));
    }
    Ok(lignes)
}

/// Les arguments de `journal.noter`.
pub struct Note<'a> {
    pub quoi: &'a str,
    pub mois: Option<&'a str>,
    pub fichier: Option<&'a str>,
    pub mode: &'a str,
    pub qui: &'a str,
    pub accepte: bool,
    pub destinataire: &'a str,
    pub flux: &'a str,
    pub reponse: &'a str,
    pub trace: Option<&'a str>,
    pub detail: Option<&'a Value>,
    /// Pour une ligne d'envoi : la version de chaque engagement qu'elle couvre (`versions_du_mois`).
    pub versions: Option<&'a Value>,
    /// L'heure de la ligne (ISO, a la seconde) ; `origine` doit alors la justifier.
    pub quand: &'a str,
    pub origine: &'a str,
}

/// Ajoute une ligne a la fin du fichier et la rend. `quand`
/// est l'heure locale que l'appelant donne (le moteur ne connait pas le
/// fuseau) ; vide, elle est lue dans `maintenant`.
pub fn noter(racine: &Path, n: &Note, maintenant: &str) -> Resultat<Value> {
    if !ACTES.contains(&n.quoi) {
        return Err(format!(
            "acte inconnu : {} (attendu {})",
            repr(n.quoi),
            ACTES.join(", ")
        ));
    }
    if !n.quand.is_empty() && n.origine.is_empty() {
        return Err("Une ligne datee doit dire d'ou vient sa date : passer `origine`. Sans ca, rien ne distingue une reprise d'une ligne fabriquee.".to_string());
    }
    let fichier = n.fichier.filter(|f| !f.is_empty());
    let nom = fichier.map(|f| Path::new(f).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let empreinte = match fichier {
        Some(f) if Path::new(f).exists() => Some(empreinte_fichier(Path::new(f))?),
        _ => None,
    };
    let mut l = Map::new();
    l.insert("quoi".into(), json!(n.quoi));
    l.insert("quand".into(), json!(if n.quand.is_empty() { maintenant } else { n.quand }));
    l.insert("origine".into(), json!(n.origine));
    l.insert("mois".into(), json!(n.mois));
    l.insert("fichier".into(), json!(nom));
    l.insert("empreinte".into(), json!(empreinte));
    l.insert("mode".into(), json!(n.mode));
    l.insert("qui".into(), json!(n.qui));
    l.insert("accepte".into(), json!(n.accepte));
    l.insert("destinataire".into(), json!(n.destinataire));
    l.insert("flux".into(), json!(n.flux));
    l.insert("retours".into(), json!([]));
    l.insert("reponse".into(), json!(n.reponse));
    l.insert("trace".into(), json!(n.trace.filter(|t| !t.is_empty())));
    l.insert("detail".into(), match n.detail {
        Some(d) if vrai(d) => d.clone(),
        _ => json!({}),
    });
    if let Some(v) = n.versions {
        l.insert("versions".into(), v.clone());
    }
    let ligne = Value::Object(l);
    let p = chemin(racine);
    let mut lignes = (*lu(racine)?.lignes).clone();
    lignes.push(ligne.clone());
    ecrire_json(&p, &Value::Array(lignes), 2)?;
    Ok(ligne)
}

/// CE QUI REVIENT D'UN ENVOI (un contrat signe, la mention retiree) : `retour` se range dans les `retours` de la derniere
/// ligne que `envoi` reconnait, et la rend. `None` : aucune ligne ne la porte, rien n'est ecrit.
pub fn rattacher(racine: &Path, envoi: impl Fn(&Value) -> bool, retour: Value) -> Resultat<Option<Value>> {
    let mut lignes = (*lu(racine)?.lignes).clone();
    let Some(l) = lignes.iter_mut().rev().find(|l| envoi(l)) else { return Ok(None) };
    let Some(Value::Array(retours)) = l.as_object_mut().map(|o| o.entry("retours").or_insert_with(|| json!([]))) else {
        return Err("journal : une ligne dont les retours ne sont pas une liste".to_string());
    };
    retours.push(retour);
    let ligne = l.clone();
    ecrire_json(&chemin(racine), &Value::Array(lignes), 2)?;
    Ok(Some(ligne))
}

/// Ce que les lignes `lignes` du journal disent de ce qui est parti (`departs`) : les lignes qui precedent un envoi
/// disent ce qui etait parti avant lui.
pub fn departs_des_lignes(lignes: &[Value]) -> engagement::departs::Departs {
    use engagement::departs::{Depart, Departs, Envoi};
    let mut d = Departs::default();
    for l in lignes {
        let envoi = match l.get("quoi").and_then(Value::as_str) {
            Some("contrat-signature") => Envoi::Contrat,
            Some("dpae") => Envoi::Dpae,
            Some("documents") => Envoi::Documents,
            Some("dsn") if l.get("detail").and_then(|d| d.get("nature")).and_then(Value::as_str).is_some_and(|n| declarations::norme::SIGNALEMENTS.contains(&n)) => {
                Envoi::Signalement
            }
            Some("dsn") => Envoi::Dsn,
            _ => continue,
        };
        if l.get("mode").and_then(Value::as_str) != Some(REEL) {
            continue;
        }
        // Une declaration refusee au depot ou au bilan n'a rien depose (`etats::refuse_par_un_retour`). Un courriel coupe en
        // route a servi ceux qui l'ont recu : la ligne ne porte que leurs versions (`ligne_d_envoi::noter_echec_d_envoi`),
        // et ce qui a atteint le salarie est parti.
        let au_salarie = matches!(envoi, Envoi::Documents | Envoi::Contrat);
        if !au_salarie && (l.get("accepte") == Some(&Value::Bool(false)) || crate::etats::refuse_par_un_retour(l).unwrap_or(false)) {
            continue;
        }
        let mois = matches!(envoi, Envoi::Documents | Envoi::Dsn).then(|| l.get("mois").and_then(Value::as_str).map(String::from)).flatten();
        let le = l.get("quand").and_then(Value::as_str).unwrap_or_default().to_string();
        // Un contrat parti sans que sa ligne dise sur quelle version (une ligne qui ne porte pas `versions`) est parti sur
        // la premiere : une validation qui ne dit pas sa version s'ecrit en version 1 (`engagement::etats`). Il se range par
        // le numero que la ligne nomme, comme les autres.
        let sans_versions: Vec<Value> = match (envoi, l.get("versions")) {
            (Envoi::Contrat, None) => l
                .pointer("/detail/contrats")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| json!({"salarie": c.get("salarie"), "numero": c.get("numero").map(socle::valeur::valeur_en_texte), "version": 1}))
                .collect(),
            _ => Vec::new(),
        };
        for v in l.get("versions").and_then(Value::as_array).into_iter().flatten().chain(&sans_versions) {
            let (Some(cle), Some(numero), Some(version)) =
                (v.get("salarie").and_then(Value::as_str), v.get("numero").and_then(Value::as_str), v.get("version").and_then(Value::as_u64))
            else {
                continue;
            };
            // La correction d'un mois passe que la DSN porte : sa version est celle de l'engagement du mois corrige.
            let corrige = (envoi == Envoi::Dsn).then(|| v.get("mois").and_then(Value::as_str).map(String::from)).flatten().filter(|m| Some(m) != mois.as_ref());
            let (m, par) = match corrige {
                Some(c) => (Some(c), mois.clone()),
                None => (mois.clone(), None),
            };
            // Un avenant : la ligne du journal le dit (`acte`), et son contrat porte son numero.
            let avenant = (envoi == Envoi::Contrat && l.pointer("/detail/acte").and_then(Value::as_str) == Some("avenant"))
                .then(|| {
                    l.pointer("/detail/contrats")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .find(|c| c.get("salarie").and_then(Value::as_str) == Some(cle) && c.get("numero").and_then(Value::as_str) == Some(numero))
                        .and_then(|c| c.get("avenant"))
                        .map(socle::valeur::valeur_en_texte)
                })
                .flatten();
            d.noter(cle, numero, Depart { envoi, mois: m, par, avenant, version, le: le.clone() });
        }
    }
    d
}

/// LES VERSIONS QU'UN ENVOI EMPORTE : pour chaque engagement valide que `mois` montre et que `garder` retient (la cle du
/// salarie, le numero du contrat, l'engagement), `{"salarie", "numero", "version"}`. Ce que la ligne du journal de l'envoi
/// porte : elles se lisent AVANT l'envoi, qui ne part pas d'un engagement a revalider (`Valide::a_revalider`).
pub fn versions_du_mois(ctx: &droit::contexte::Contexte, mois: &str, departs: &engagement::departs::Departs, garder: impl Fn(&str, &str, &Value) -> bool) -> Resultat<Value> {
    let mut out = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, departs)? {
        let Some(v) = e.valide() else { continue };
        let brut = Value::Object(v.objet().brut.clone());
        let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &brut), brut.get("numero").and_then(Value::as_str)) else { continue };
        if garder(&cle, numero, &brut) {
            if let Some(phrase) = v.a_revalider(ctx) {
                return Err(phrase);
            }
            let version = v.version().ok_or("validation illisible : `version`")?;
            out.push(json!({"salarie": cle, "numero": numero, "version": version}));
        }
    }
    Ok(Value::Array(out))
}

/// `versions_du_mois` des contrats qui partent (`[{"salarie", "numero", "mois"}]`, chacun lu dans le mois qu'il nomme).
pub fn versions_des_contrats(ctx: &droit::contexte::Contexte, contrats: &[Value], departs: &engagement::departs::Departs) -> Resultat<Value> {
    let mut mois: Vec<String> = contrats.iter().filter_map(|c| c.get("mois").and_then(Value::as_str).map(String::from)).collect();
    mois.sort();
    mois.dedup();
    let mut out = Vec::new();
    for m in mois {
        let siens = |cle: &str, n: &str, _: &Value| {
            contrats.iter().any(|c| c["mois"].as_str() == Some(m.as_str()) && c["salarie"].as_str() == Some(cle) && c["numero"].as_str() == Some(n))
        };
        if let Value::Array(v) = versions_du_mois(ctx, &m, departs, siens)? {
            for x in v {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
        }
    }
    Ok(Value::Array(out))
}

/// Le plus grand numero d'ordre (`S20.G00.05.004`) des signalements REELS deja envoyes par la compagnie, tous
/// evenements confondus (arret, reprise, fin de contrat, amorcage) ; 0 s'il n'y en a aucun.
///
/// Cahier technique DSN 2026.1, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin
/// qu'il soit unique quel que soit le mois d'emission » ; p. 14 : un second signalement normal au meme SIRET et
/// au meme numero d'ordre n'est pas accepte. Le numero se LIT de ce qui est parti (`detail.ordre` des lignes
/// `dsn` reelles du journal), il ne se devine pas : une ligne reelle acceptee qui ne dit pas son ordre refuse.
/// Un signalement se reconnait a `detail.nature` (04, 05, 07, 08) ; une ligne plus ancienne qui ne la porte pas,
/// a son `mois` : une mensuelle se date `AAAA-MM`, un signalement par son evenement (`AAAA-MM-JJ`). Une ligne
/// non acceptee (le point de depot n'a rien recu) ne consomme pas de numero.
pub fn dernier_ordre_des_signalements(racine: &Path) -> Resultat<i64> {
    let mut dernier = 0;
    for l in lire(racine, Some("dsn"), None, true)? {
        let detail = l.get("detail").cloned().unwrap_or(Value::Null);
        let signalement = match detail.get("nature").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            Some(n) => declarations::norme::SIGNALEMENTS.contains(&n),
            None => {
                let m: Vec<char> = l.get("mois").and_then(Value::as_str).unwrap_or("").chars().collect();
                !(m.len() == 7 && m[4] == '-' && m.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit()))
            }
        };
        if !signalement {
            continue;
        }
        let ordre = match detail.get("ordre") {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
            _ => None,
        };
        match ordre {
            Some(o) => dernier = dernier.max(o),
            None if l.get("accepte").map(vrai).unwrap_or(false) => {
                return Err(format!(
                    "Le journal des envois ({}) note le signalement reel du {} ({}) sans son numero d'ordre : l'ecrire dans le `detail` de cette ligne (`\"ordre\": N`, N lu en S20.G00.05.004 du fichier envoye). Le numero d'ordre d'un signalement ne se remet jamais a zero (cahier technique DSN, p. 136) : sans lui, le suivant ne se numerote pas. Rien n'est ecrit.",
                    chemin(racine).display(),
                    l.get("quand").and_then(Value::as_str).unwrap_or("?"),
                    l.get("fichier").and_then(Value::as_str).unwrap_or("?"),
                ))
            }
            None => {}
        }
    }
    Ok(dernier)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// La trace d'un depot se lit relative a la compagnie, ou absolue telle qu'elle avait ete ecrite avant.
    #[test]
    fn la_trace_relative_suit_la_compagnie_et_l_absolue_d_avant_se_lit_telle_quelle() {
        let r = Path::new("/ici/compagnies/la-cie");
        assert_eq!(trace_de(r, &json!({"trace": "retours/depots/x.txt"})), Some(r.join("retours/depots/x.txt")));
        assert_eq!(trace_de(r, &json!({"trace": "/ailleurs/fictives/la-cie/retours/depots/x.txt"})), Some(PathBuf::from("/ailleurs/fictives/la-cie/retours/depots/x.txt")));
        assert_eq!(trace_de(r, &json!({"trace": null})), None);
    }

    /// Bug reel : une DSN ou une DPAE refusee au bilan comptait pour partie ; le mois se figeait et sa correction allait a
    /// la DSN suivante. Rien n'a ete recu (cahier technique DSN, § 4.2, `norme-dsn/texte/chapitre-4-controles.txt`).
    #[test]
    fn une_declaration_refusee_au_bilan_n_est_pas_partie() {
        let ligne = |quoi: &str, salarie: &str, retours: Value, detail: Value| {
            json!({"quoi": quoi, "mode": REEL, "accepte": true, "mois": "2026-06", "quand": "2026-07-05T10:00:00", "detail": detail,
                   "retours": retours, "versions": [{"salarie": salarie, "numero": "N1", "version": 1}]})
        };
        let ko = json!([{"nature": "10", "verdict": "OK"}, {"nature": "11", "verdict": "KO"}]);
        let lignes = vec![
            ligne("dsn", "ALPHA", ko.clone(), json!({})),
            ligne("dsn", "BETA", json!([]), json!({"reconstruit": true, "etat_declaration": "KO"})),
            ligne("dpae", "GAMMA", ko, json!({})),
            ligne("dsn", "DELTA", json!([{"nature": "11", "verdict": "OK"}]), json!({})),
        ];
        let d = departs_des_lignes(&lignes);
        for refusee in ["ALPHA", "BETA", "GAMMA"] {
            assert!(d.de(refusee, "N1").is_empty(), "{refusee}");
        }
        assert_eq!(d.de("DELTA", "N1").len(), 1);
    }
}
