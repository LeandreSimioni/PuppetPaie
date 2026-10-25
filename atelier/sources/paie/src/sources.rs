//! Les sources des donnees livrees avec le logiciel, et leur veille (`moteur/sources.py`, retire au lot K, porte au lot
//! S1). Un outil de l'EDITEUR, lance avant chaque publication (`paie sources --verifier`) ; une compagnie ne fait que
//! recevoir la mise a jour signee.
//!
//! `donnees/sources.json` dit, pour chaque texte dont derive un fichier de donnees (nomenclature des emplois, catalogue
//! des metiers, minima, baremes), ce qu'il contient, de quand il date, quand on l'a releve, ou en est la copie et quel est
//! son condensat. Ce module le lit et sait retourner voir si la source en ligne a bouge.
//!
//! Le principe : une donnee ne se recopie pas d'un editeur, elle se lit dans le texte qui fait foi ; la copie est gardee
//! dans le depot public, a cote du code (`atelier/reference/sources/` de l'installation de l'editeur, qui est le depot :
//! Leandre, 15/10/2026, les textes de reference rendent le logiciel verifiable) pour qu'on puisse toujours montrer d'ou
//! vient un nombre ; et quand le condensat en ligne ne colle plus, c'est le signal de RELIRE -- pas de remplacer le
//! fichier a l'aveugle, puisque les donnees derivees ont ete ecrites a la main depuis la lecture. `--relever` pose la
//! nouvelle version A COTE de l'ancienne, suffixee de la date, jamais par-dessus.
//!
//! Legifrance refuse les telechargements automatiques : ces sources-la sont en verification « manuelle » -- la copie ne
//! se reverifie pas en ligne. Mais quand la source porte un bloc `kali` (IDCC, conteneur, texte), on lit les livraisons
//! open data KALI de la DILA depuis le releve (`kali.rs`) : elles disent si la convention ou le texte ont bouge, et la
//! commande dit quelle page ouvrir.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use socle::date::Jour;
use crate::kali::{self, Suivi};

pub const MANIFESTE: &str = "sources.json";
pub const AGENT: &str = "PuppetPaie (veille des sources ; +https://github.com/puppetpaie)";

type R<T> = Result<T, String>;

/// Ce qu'on telecharge : les octets et l'en-tete `Last-Modified`, ou la panne en une phrase.
pub type Telecharger<'a> = dyn FnMut(&str) -> R<(Vec<u8>, Option<String>)> + 'a;

/// Le manifeste livre (`donnees/sources.json`, a cote des autres referentiels).
pub fn manifeste(code: &Path) -> R<Value> {
    let p = code.join(MANIFESTE);
    let t = std::fs::read_to_string(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
    let v: Value = serde_json::from_str(&t).map_err(|e| format!("{} illisible : {}", p.display(), e))?;
    if !v.get("sources").is_some_and(Value::is_object) {
        return Err(format!("{} : pas d'objet « sources »", p.display()));
    }
    Ok(v)
}

/// Ou vivent les copies : `atelier/reference/sources/` de l'installation (`maj::installation`), c'est-a-dire du depot
/// sur le poste de l'editeur, a cote de `logiciel/`. Une compagnie n'y est pour rien : ce sont les textes du logiciel.
pub const COPIES: &str = "atelier/reference/sources";

pub fn dossier(installation: &Path) -> PathBuf {
    installation.join(COPIES)
}

fn sha256(o: &[u8]) -> String {
    Sha256::digest(o).iter().map(|b| format!("{:02x}", b)).collect()
}

fn exiger<'a>(ident: &str, s: &'a Value, cle: &str) -> R<&'a str> {
    s.get(cle).and_then(Value::as_str).ok_or_else(|| format!("sources.json : la source « {} » n'a pas de « {} » (texte)", ident, cle))
}

/// Ce que le manifeste dit, confronte au disque -- sans reseau. Un objet par source, dans l'ordre du manifeste.
pub fn etats(installation: &Path, manifeste: &Value, aujourdhui: &Jour) -> R<Vec<Value>> {
    let d = dossier(installation);
    let mut out = Vec::new();
    for (ident, s) in manifeste["sources"].as_object().into_iter().flatten() {
        let fichier = exiger(ident, s, "fichier")?;
        let attendu = exiger(ident, s, "sha256")?;
        let p = d.join(fichier);
        let presente = p.is_file();
        // L'original d'un organisme prive n'est pas dans le depot public : seule sa transcription y est (`transcription`,
        // chemin relatif au meme dossier). Elle ne se confronte a aucun condensat : c'est l'original qui fait foi.
        let transcription = s.get("transcription").and_then(Value::as_str);
        let transcription_presente = transcription.map(|t| d.join(t).is_file());
        let conforme = if presente { json!(std::fs::read(&p).map(|o| sha256(&o) == attendu).unwrap_or(false)) } else { Value::Null };
        let releve = s.get("releve_le").and_then(Value::as_str).filter(|r| !r.is_empty());
        let age = match releve {
            Some(r) => json!(aujourdhui.moins(&Jour::lire(r).map_err(|e| format!("sources.json : « {} », releve_le : {}", ident, e))?)),
            None => Value::Null,
        };
        let mut extra = Map::new();
        for k in ["en_vigueur", "etendu_le", "verification_comment"] {
            if let Some(v) = s.get(k) {
                extra.insert(k.into(), v.clone());
            }
        }
        let nourrit = match s.get("nourrit") {
            None | Some(Value::Null) => json!([]),
            Some(Value::Array(a)) => Value::Array(a.clone()),
            Some(_) => return Err(format!("sources.json : la source « {} » : « nourrit » n'est pas une liste", ident)),
        };
        out.push(json!({
            "ident": ident, "titre": exiger(ident, s, "titre")?, "editeur": exiger(ident, s, "editeur")?, "url": exiger(ident, s, "url")?,
            "contenu": exiger(ident, s, "contenu")?, "version": s.get("version").cloned().unwrap_or(Value::Null),
            "releve_le": releve, "verification": exiger(ident, s, "verification")?,
            "nourrit": nourrit, "fichier": fichier, "copie_presente": presente, "copie_conforme": conforme,
            "transcription": transcription, "transcription_presente": transcription_presente,
            "en_ligne": null, "detail": "", "page_de_veille": s.get("page_de_veille").cloned().unwrap_or(Value::Null),
            "age_jours": age, "extra": extra, "kali": s.get("kali").cloned().unwrap_or(Value::Null), "evenements": [],
        }));
    }
    Ok(out)
}

/// Le telechargement reel : un GET, l'agent de la veille, 30 s, TLS verifie par la plate-forme, le mandataire de
/// l'environnement. Un statut d'erreur est une panne.
pub fn telecharger(url: &str, secondes: u64) -> R<(Vec<u8>, Option<String>)> {
    let mut c = ureq::Agent::config_builder().user_agent(AGENT).timeout_global(Some(std::time::Duration::from_secs(secondes)));
    if url.starts_with("https:") {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    // En essai, le garde-fou reseau : rien ne sort de la machine.
    let agent: ureq::Agent = socle::garde_reseau::regler(c, url)?.build().into();
    let mut r = agent.get(url).call().map_err(|e| e.to_string())?;
    let modifie = r.headers().get("last-modified").and_then(|v| v.to_str().ok()).map(String::from);
    let o = r.body_mut().with_config().limit(512 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
    Ok((o, modifie))
}

/// Retourne voir chaque source en ligne (`seulement` : ces identifiants-la, sinon toutes).
///
/// Une source « telechargement » : on la reprend, on compare le condensat a celui du manifeste. `inchangee` : rien a
/// faire. `changee` : la source a bouge, il faut la RELIRE et mettre a jour les donnees qu'elle nourrit ; avec `relever`
/// la nouvelle version est posee a cote de l'ancienne, suffixee de la date, jamais par-dessus. `injoignable` : pas de
/// reseau, ou le lien a change -- ce qui est aussi une information.
///
/// Une source « manuelle » : on rend la page a ouvrir ; on ne cache pas qu'on ne sait pas la verifier. Si elle porte un
/// bloc `kali`, on lit ce que la DILA a livre depuis le releve (`kali_telecharger`) : `inchangee` quand rien ne touche la
/// convention ou que rien ne parle de salaires, `changee` quand un texte salaires (ou le texte cite) a bouge.
pub fn verifier(
    installation: &Path,
    manifeste: &Value,
    aujourdhui: &Jour,
    seulement: &[String],
    relever: bool,
    telecharger: &mut Telecharger,
    kali_telecharger: &mut kali::Telecharger,
) -> R<Vec<Value>> {
    let d = dossier(installation);
    let mut liste: Vec<Value> =
        etats(installation, manifeste, aujourdhui)?.into_iter().filter(|e| seulement.is_empty() || seulement.iter().any(|s| e["ident"] == json!(s))).collect();
    let rapport = veiller_kali(&d, &liste, relever, kali_telecharger);
    for e in liste.iter_mut() {
        if e["verification"] != json!("telechargement") {
            e["en_ligne"] = json!("manuelle");
            e["detail"] = e["extra"].get("verification_comment").cloned().unwrap_or_else(|| json!(""));
            if e["kali"].is_object() {
                if let Some(r) = &rapport {
                    etat_kali(e, r);
                }
            }
            continue;
        }
        let url = e["url"].as_str().unwrap_or_default().to_string();
        let (octets, modifie) = match telecharger(&url) {
            Ok(x) => x,
            Err(m) => {
                e["en_ligne"] = json!("injoignable");
                e["detail"] = json!(m);
                continue;
            }
        };
        let modifie = modifie.unwrap_or_else(|| "non donne".into());
        let attendu = manifeste["sources"][e["ident"].as_str().unwrap_or_default()]["sha256"].as_str().unwrap_or_default();
        if sha256(&octets) == attendu {
            e["en_ligne"] = json!("inchangee");
            e["detail"] = json!(format!("Last-Modified : {}", modifie));
            continue;
        }
        e["en_ligne"] = json!("changee");
        let mut detail = format!(
            "le fichier en ligne n'est plus celui qui a ete lu (Last-Modified : {}). Relire, puis mettre a jour : {}",
            modifie,
            nourrit(e)
        );
        if relever {
            let p = d.join(e["fichier"].as_str().unwrap_or_default());
            let souche = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let suffixe = p.extension().map(|x| format!(".{}", x.to_string_lossy())).unwrap_or_default();
            let neuf = p.with_file_name(format!("{}.{}{}", souche, aujourdhui.iso(), suffixe));
            if let Some(parent) = neuf.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{} : {}", parent.display(), e))?;
            }
            std::fs::write(&neuf, &octets).map_err(|e| format!("{} : {}", neuf.display(), e))?;
            detail.push_str(&format!(". Copie posee : {}", neuf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
        }
        e["detail"] = json!(detail);
    }
    Ok(liste)
}

fn nourrit(e: &Value) -> String {
    let n: Vec<String> = e["nourrit"].as_array().into_iter().flatten().map(socle::valeur::valeur_en_texte).collect();
    if n.is_empty() {
        "rien".into()
    } else {
        n.join(", ")
    }
}

// ------------------------------------------------------------------ la veille KALI

/// Ce qui, dans le titre d'un texte KALI, fait qu'on le classe « salaires ». C'est un tri d'affichage, pas une regle de
/// droit : les autres mouvements sont montres aussi, en second, avec leur titre, pour que l'oeil tranche.
const MOTS_SALAIRES: [&str; 9] = ["salaire", "minim", "remuneration", "rémunération", "nao", "cachet", "grille", "barème", "bareme"];

fn parle_salaires(titre: &str) -> bool {
    let t = titre.to_lowercase();
    MOTS_SALAIRES.iter().any(|m| t.contains(m))
}

/// Une seule lecture des livraisons pour toutes les sources qui en ont besoin, depuis le releve le plus ancien ; chaque
/// source filtre ensuite sur le sien. `None` : aucune source ne porte de bloc `kali`. `Some(Err)` : la panne.
fn veiller_kali(d: &Path, liste: &[Value], relever: bool, tele: &mut kali::Telecharger) -> Option<R<Value>> {
    let avec: Vec<&Value> = liste.iter().filter(|e| e["kali"].is_object()).collect();
    if avec.is_empty() {
        return None;
    }
    let mut suivis: Vec<Suivi> = Vec::new();
    for e in &avec {
        let k = &e["kali"];
        let (Some(cont), Some(idcc)) = (k["conteneur"].as_str(), k["idcc"].as_str()) else {
            return Some(Err(format!("sources.json : le bloc « kali » de « {} » n'a pas de conteneur ou d'idcc", e["ident"].as_str().unwrap_or(""))));
        };
        let i = match suivis.iter().position(|s| s.conteneur == cont) {
            Some(i) => i,
            None => {
                suivis.push(Suivi { conteneur: cont.into(), idcc: idcc.into(), textes: Vec::new() });
                suivis.len() - 1
            }
        };
        if let Some(t) = k["texte"].as_str() {
            suivis[i].textes.push(t.into());
        }
    }
    let Some(depuis) = avec.iter().filter_map(|e| e["releve_le"].as_str()).min() else {
        return Some(Err("aucune source suivie par KALI n'a de date de releve : depuis quand lire ?".into()));
    };
    Some(kali::veiller(d, &suivis, depuis, relever, tele))
}

/// Traduit le rapport en etat de CETTE source : ses evenements sont ceux de sa convention, posterieurs a son propre
/// releve.
fn etat_kali(e: &mut Value, rapport: &R<Value>) {
    let rapport = match rapport {
        Ok(r) => r,
        Err(m) => {
            e["en_ligne"] = json!("injoignable");
            e["detail"] = json!(format!("livraisons KALI (DILA) : {}", m));
            return;
        }
    };
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let conteneur = s(&e["kali"]["conteneur"]);
    let texte_cite = s(&e["kali"]["texte"]);
    let depuis = s(&e["releve_le"]);
    let evts: Vec<Value> =
        rapport["evenements"][conteneur.as_str()].as_array().into_iter().flatten().filter(|x| s(&x["livraison"]) > depuis).cloned().collect();
    let lues = rapport["livraisons_lues"].as_array().into_iter().flatten().filter(|j| s(j) > depuis).count();
    let ou = format!("IDCC {}, {} livraison(s) KALI lue(s) depuis le {}", s(&e["kali"]["idcc"]), lues, depuis);
    let (mut forts, mut faibles) = (Vec::new(), Vec::new());
    for x in &evts {
        if x["genre"] == json!("convention") {
            let nouveaux = x["nouveaux"].as_object().cloned().unwrap_or_default();
            let mut sal: Vec<String> = nouveaux.values().map(s).filter(|t| parle_salaires(t)).collect();
            sal.sort();
            if !sal.is_empty() {
                forts.push(format!("{} : texte(s) nouveau(x) attache(s) a la convention -- {}", s(&x["livraison"]), sal.join(" ; ")));
            }
            let mut autres: Vec<String> = nouveaux.values().map(s).filter(|t| !parle_salaires(t)).collect();
            autres.sort();
            if !autres.is_empty() {
                faibles.push(format!("{} : {} texte(s) nouveau(x) -- {}", s(&x["livraison"]), autres.len(), autres.join(" ; ")));
            }
            if x["premiere"] == json!(true) {
                faibles.push(format!("{} : {}", s(&x["livraison"]), s(&x["titre"])));
            }
        } else {
            let date = Some(s(&x["date_texte"])).filter(|d| !d.is_empty()).unwrap_or_else(|| "?".into());
            let etat = Some(s(&x["etat"])).filter(|d| !d.is_empty()).unwrap_or_else(|| "?".into());
            let ext = Some(s(&x["extension"])).filter(|d| !d.is_empty()).map(|x| format!(", {}", x)).unwrap_or_default();
            let ligne = format!("{} : texte {} modifie -- {} (du {}, etat {}{})", s(&x["livraison"]), s(&x["ident"]), s(&x["titre"]), date, etat, ext);
            if s(&x["ident"]) == texte_cite || parle_salaires(&s(&x["titre"])) {
                forts.push(ligne);
            } else {
                faibles.push(ligne);
            }
        }
    }
    let mut detail = if !forts.is_empty() {
        e["en_ligne"] = json!("changee");
        let mut d = format!("{} : un texte SALAIRES a bouge. {}. Relire, puis mettre a jour : {}", ou, forts.join(" | "), nourrit(e));
        if !faibles.is_empty() {
            d.push_str(&format!(". Autres mouvements : {}", faibles.join(" | ")));
        }
        d
    } else if !faibles.is_empty() {
        e["en_ligne"] = json!("inchangee");
        format!("{} : la convention a bouge, mais rien qui parle de salaires d'apres les titres -- {}", ou, faibles.join(" | "))
    } else {
        e["en_ligne"] = json!("inchangee");
        format!("{} : rien ne touche la convention", ou)
    };
    match rapport["amorces"].get(conteneur.as_str()) {
        Some(am) => detail.push_str(&format!(" (liste de reference : {} textes, fiche du {})", am["textes"], s(&am["livraison"]))),
        None => detail.push_str(" (ATTENTION : aucune fiche de la convention trouvee dans l'index, seul le texte cite par le manifeste est suivi)"),
    }
    if let Some(t) = rapport["trou"].as_str() {
        detail.push_str(&format!(". ATTENTION, {}", t));
    }
    e["detail"] = json!(detail);
    e["evenements"] = Value::Array(evts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_titre_parle_de_salaires_par_ses_mots() {
        assert!(parle_salaires("Avenant relatif aux Salaires minimaux"));
        assert!(parle_salaires("Rémunérations conventionnelles 2026"));
        assert!(parle_salaires("Grille des cachets"));
        assert!(!parle_salaires("Formation professionnelle"));
    }

    /// Le texte cite par le manifeste qui bouge met la source « a relire », quel que soit son titre ; un autre texte sans
    /// mot de salaire la laisse inchangee, montre en second.
    #[test]
    fn le_texte_cite_qui_bouge_met_la_source_a_relire() {
        let base = json!({"releve_le": "2026-09-13", "nourrit": ["minima-conventionnels.json"],
            "kali": {"idcc": "3090", "conteneur": "C", "texte": "T1"}});
        let rapport = |ident: &str, titre: &str| -> R<Value> {
            Ok(json!({"livraisons_lues": ["2026-09-20"], "trou": null, "amorces": {"C": {"textes": 52, "livraison": "2026-06-05"}},
                "evenements": {"C": [{"livraison": "2026-09-20", "genre": "texte", "ident": ident, "titre": titre,
                    "date_texte": "2026-09-01", "etat": "VIGUEUR", "extension": ""}]}}))
        };
        let mut e = base.clone();
        etat_kali(&mut e, &rapport("T1", "Titre XII prevoyance"));
        assert_eq!(e["en_ligne"], json!("changee"));
        assert!(e["detail"].as_str().unwrap().contains("minima-conventionnels.json"), "{}", e["detail"]);
        let mut e = base.clone();
        etat_kali(&mut e, &rapport("T9", "Formation professionnelle"));
        assert_eq!(e["en_ligne"], json!("inchangee"));
        assert!(e["detail"].as_str().unwrap().contains("rien qui parle de salaires"), "{}", e["detail"]);
        // Une livraison du jour du releve ne compte pas.
        let mut e = json!({"releve_le": "2026-09-20", "kali": {"idcc": "3090", "conteneur": "C", "texte": "T1"}});
        etat_kali(&mut e, &rapport("T1", "Salaires"));
        assert_eq!(e["en_ligne"], json!("inchangee"));
        let mut e = base.clone();
        etat_kali(&mut e, &Err("connexion refusee".into()));
        assert_eq!(e["en_ligne"], json!("injoignable"));
    }
}
