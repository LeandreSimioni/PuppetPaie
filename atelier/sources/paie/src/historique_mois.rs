//! `historique-mois` : tout ce qu'un mois a produit, envoye et recu, pour l'Historique (tickets #219, #220, #222 du
//! depot prive ; Leandre, 20/10/2026 : « tous les envois et tous les retours se lisent dans l'onglet Historique »).
//!
//! La page Paie est l'endroit ou l'on FAIT le mois ; l'Historique, celui ou l'on VOIT ce qu'il a donne. Cette route
//! rend, pour un mois :
//!
//! - `documents` : chaque fichier de `sorties/<mois>/` et de `sorties/<mois>/envoi/` (les memes listes que
//!   l'Historique et `documents-liste` : `routes_accueil::mois_connus`, `routes_apercus::documents_du_mois`), range
//!   par sorte, chacun avec le chemin que `ouvrir-piece` ouvre ;
//! - `envois` : chaque ligne du journal des depots du mois (DPAE, DSN, signalements, documents, contrats), TESTS
//!   COMPRIS : le journal ne porte que ce que CE logiciel a envoye (un test d'ailleurs n'y entre pas, `reprise_journal`,
//!   PR #17), et ce qu'il a envoye se montre partout (Leandre, 20/10/2026 : « les gens ont besoin de tout savoir, le
//!   logiciel doit etre d'une transparence totale, ce sont leurs infos, leurs tests ») ;
//! - sous chaque envoi, ses `retours` dans l'ordre d'arrivee, chacun LU : l'organisme, ce que dit son statut, et ce
//!   que dit sa piece archivee quand ce logiciel sait la lire (`lire_piece`). Sinon le statut, et la piece telle quelle.
//!
//! Lecture seule : rien ne s'ecrit, rien ne part.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::contexte::Contexte;
use crate::etats::{self, erreur, py_str};
use crate::fichiers::vrai;
use crate::retours::{self, Echec};

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "historique-mois";

// ------------------------------------------------------------------ les tables

/// L'organisme et le FORMAT de chaque nature de retour de la DSN : le tableau des retours du guide d'implementation de
/// l'API DSN v6.8, §1.3, p. 11-13 (colonnes « OPS » et « Format phase 3 » ;
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`). « harmonise » :
/// le schema des bilans (`retours::lire_rapport`) ; « aucun » : un accuse sans document, son statut dit tout ; « pdf » :
/// une archive d'AER ; « neores », « oc », « agirc-arrco » : des formats dont la norme n'est pas deposee ici (le
/// guide les cite, l'archive v6.8 deposee ne les contient pas : ni `NEOReS_CT*.pdf`, ni le format temporaire AGIRC-ARRCO),
/// qui ne se traduisent donc pas. Le format de la nature 93 n'est pas rempli dans le tableau du guide : « harmonise »
/// est une INFERENCE (la nature 95, de la meme DGFiP, a son exemple sous « Format DSN harmonise », §6.3).
const NATURES: [(&str, &str, &str); 29] = [
    ("10", "le point de dépôt (SI DSN)", "harmonise"),
    ("11", "le point de dépôt (SI DSN)", "harmonise"),
    ("20", "le SI DSN", "harmonise"),
    ("21", "le SI DSN", "harmonise"),
    ("22", "le SI DSN", "harmonise"),
    ("23", "le SI DSN", "harmonise"),
    ("34", "la CNAM (taux AT/MP)", "harmonise"),
    ("40", "France Travail", "aucun"),
    ("42", "France Travail", "harmonise"),
    ("43", "France Travail", "pdf"),
    ("44", "France Travail", "harmonise"),
    ("45", "France Travail", "harmonise"),
    ("46", "France Travail", "pdf"),
    ("50", "l'organisme complémentaire", "aucun"),
    ("51", "l'organisme complémentaire", "oc"),
    ("60", "l'Urssaf", "aucun"),
    ("61", "l'Urssaf", "harmonise"),
    ("70", "l'AGIRC-ARRCO", "aucun"),
    ("71", "l'AGIRC-ARRCO", "agirc-arrco"),
    ("92", "la DGFiP", "aucun"),
    ("93", "la DGFiP", "harmonise"),
    ("94", "la DGFiP", "harmonise"),
    ("95", "la DGFiP", "harmonise"),
    ("117", "l'Urssaf ou la MSA", "harmonise"),
    ("118", "l'Urssaf ou la MSA", "neores"),
    ("119", "l'Urssaf", "neores"),
    ("120", "l'Urssaf", "neores"),
    ("121", "le SI DSN", "neores"),
    ("124", "l'Urssaf", "neores"),
];

/// Ce que dit le statut d'un retour : guide v6.8, §3.4.4 « Reponse du service » (`flux.retour.statut`).
fn phrase_du_statut(statut: &str) -> &'static str {
    match statut {
        "OK" => "le traitement s'est terminé normalement",
        "KO" => "au moins une anomalie bloquante a empêché la poursuite du traitement",
        "ANO" => "des anomalies non bloquantes sont survenues, le traitement est allé à son terme",
        "EN COURS" => "la déclaration est en cours de distribution vers l'organisme",
        _ => "",
    }
}

/// La nature et le type d'une declaration (S20.G00.05.001 et .002, `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`).
fn nature_dsn(nature: &str, type_: &str) -> String {
    let n = match nature {
        "01" => "DSN mensuelle",
        "04" => "Signalement arrêt de travail",
        "05" => "Signalement reprise suite à arrêt de travail",
        "07" => "Signalement fin du contrat de travail unique",
        "08" => "Signalement amorçage des données variables",
        "09" => "DSN de substitution",
        _ => "DSN",
    };
    match type_ {
        "03" | "05" => format!("{} (annule et remplace)", n),
        "04" => format!("{} (annulation)", n),
        _ => n.to_string(),
    }
}

/// L'acte d'une ligne du journal, en mots.
fn acte(l: &Value) -> String {
    let d = l.get("detail").cloned().unwrap_or(Value::Null);
    let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    match l.get("quoi").and_then(Value::as_str).unwrap_or("") {
        "dsn" => nature_dsn(&t("nature"), &t("type")),
        "dpae" => "DPAE".into(),
        "documents" => "Documents aux salariés".into(),
        "contrat-signature" => "Contrat envoyé à la signature".into(),
        "contrat-recu" => "Contrat revenu signé".into(),
        "contrat-contresigne" => "Contrat contresigné".into(),
        "regularisation-casc" => "Régularisation Congés Spectacles".into(),
        "dossier-salarie" => "Dossier d'un salarié".into(),
        "fiche-demande" => "Fiche demandée à un salarié".into(),
        "fiche-recue" => "Fiche reçue d'un salarié".into(),
        autre => autre.to_string(),
    }
}

/// `AAAAMMJJhhmmss` -> `AAAA-MM-JJThh:mm:ss` ; autre chose reste tel quel.
fn quand_publication(p: &str) -> String {
    let c: Vec<char> = p.chars().collect();
    if c.len() >= 14 && c[..14].iter().all(|x| x.is_ascii_digit()) {
        let s = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        return format!("{}-{}-{}T{}:{}:{}", s(0, 4), s(4, 6), s(6, 8), s(8, 10), s(10, 12), s(12, 14));
    }
    p.to_string()
}

// ------------------------------------------------------------------ les documents

/// Les sortes de documents, dans l'ordre ou on les cherche, par le debut du nom (les fichiers du mois et ceux d'envoi).
const SORTES: [(&str, &[&str]); 7] = [
    ("Bulletins de paie", &["Bulletins_", "Bulletin_"]),
    ("Certificats Congés Spectacles", &["CertificatsCS_", "CertificatCS_"]),
    ("Attestations employeur (AER)", &["AER_"]),
    ("Contrats", &["Contrats_", "Contrat_", "Avenant"]),
    ("DPAE", &["DPAE_"]),
    ("DSN", &["DSN_"]),
    ("AEM", &["AEM_"]),
];

fn sorte_de(nom: &str) -> &'static str {
    SORTES.iter().find(|(_, p)| p.iter().any(|x| nom.starts_with(x))).map(|(s, _)| *s).unwrap_or("Autres pièces")
}

/// Les documents du mois : les fichiers de `sorties/<mois>/` (`mois_connus`, la liste de l'Historique) et les PDF de
/// `sorties/<mois>/envoi/` (`documents_du_mois`, celle de `documents-liste`). Une archive `.zip` n'est que l'emballage
/// du fichier de meme nom qui part au depot : elle ne se liste pas.
fn documents(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let noms = crate::routes_accueil::mois_connus(&ctx.racine).into_iter().find(|(m, _)| m == mois).map(|(_, n)| n).unwrap_or_default();
    let mut out: Vec<Value> = Vec::new();
    for nom in noms.iter().filter(|n| !n.ends_with(".zip")) {
        out.push(json!({"nom": nom, "sorte": sorte_de(nom), "salarie": "", "chemin": format!("sorties/{}/{}", mois, nom)}));
    }
    let envoi = crate::routes_apercus::documents_du_mois(ctx, &json!(mois), &json!("tout"))?;
    for d in envoi.as_array().into_iter().flatten() {
        let nom = py_str(&d["nom"]);
        out.push(json!({"nom": nom, "sorte": sorte_de(&nom), "salarie": d["salarie"], "chemin": format!("sorties/{}/envoi/{}", mois, nom)}));
    }
    let rang = |v: &Value| SORTES.iter().position(|(s, _)| v["sorte"] == json!(s)).unwrap_or(SORTES.len());
    out.sort_by_key(|v| rang(v));
    Ok(out)
}

// ------------------------------------------------------------------ les retours, lus

/// La piece archivee d'un retour : `retours/[dpae/]AAAAMMJJ-hhmmss_natureN_<flux>.<ext>` (`transport::archiver`),
/// celle de sa publication d'abord, sinon la premiere de sa nature pour ce flux (`etats::archive_du_retour`).
fn piece_du_retour(racine: &Path, canal: &str, flux: &str, nature: &str, publication: &str) -> Option<PathBuf> {
    let mut dossier = racine.join("retours");
    if !canal.is_empty() {
        dossier = dossier.join(canal);
    }
    let flux: String = if flux.is_empty() { "sansflux".into() } else { flux.chars().take(24).collect() };
    let suffixe = format!("_nature{}_{}", nature, flux);
    let mut noms: Vec<String> = std::fs::read_dir(&dossier)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let radical = n.rsplit_once('.').map_or(n.as_str(), |(r, _)| r);
            radical.ends_with(&suffixe)
        })
        .collect();
    noms.sort();
    let p: String = publication.chars().take(14).collect();
    let horo = if p.len() == 14 { format!("{}-{}", &p[..8], &p[8..]) } else { String::new() };
    let choisi = noms.iter().find(|n| !horo.is_empty() && n.starts_with(&horo)).or(noms.first())?;
    Some(dossier.join(choisi))
}

/// Ce qu'on lit d'une piece, en phrases : `(titre, phrases, anomalies, traduite)`.
struct Lecture {
    titre: String,
    phrases: Vec<String>,
    anomalies: Vec<Value>,
    traduite: bool,
}

fn verdict_en_mots(etat: &str) -> &'static str {
    match etat {
        "OK" => "conforme",
        "KO" => "refusée",
        "ANO" => "acceptée avec des anomalies",
        _ => "",
    }
}

/// Une piece lue : le certificat DPAE (nature 41 du canal DPAE, `dpae::lire_certificat`), un rapport au format
/// harmonise (`retours::lire_rapport`), une archive d'AER (`retours::aer_spectacle`), un PDF. Autre chose ne se
/// traduit pas : son format est dit, et la piece s'ouvre telle quelle.
fn lire_piece(o: &[u8], canal: &str, nature: &str, format: &str) -> Lecture {
    let mut l = Lecture { titre: String::new(), phrases: Vec::new(), anomalies: Vec::new(), traduite: true };
    if canal == "dpae" && nature == "41" {
        let texte: String = o.iter().map(|&b| b as char).collect();
        let lignes = crate::dpae::lire_certificat(&texte);
        if !lignes.is_empty() {
            l.titre = "Certificat de conformité des DPAE".into();
            for x in &lignes {
                let t = |k: &str| x.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
                let qui = [t("nom"), t("prenom")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
                let etat = if vrai(&x["enregistree"]) {
                    let dossier = t("dossier");
                    if dossier.is_empty() { "enregistrée".to_string() } else { format!("enregistrée, dossier {}", dossier) }
                } else if vrai(&x["deja_existante"]) {
                    "déjà enregistrée auparavant".to_string()
                } else {
                    format!("refusée : {}", [t("code"), t("libelle")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" — "))
                };
                l.phrases.push(format!("{}{} : {}.", qui, if t("embauche").is_empty() { String::new() } else { format!(" (embauche du {})", t("embauche")) }, etat));
            }
            return l;
        }
    }
    if o.starts_with(b"%PDF") {
        l.titre = "Pièce PDF".into();
        l.phrases.push("Un document PDF : il s'ouvre tel quel.".into());
        return l;
    }
    if o.starts_with(b"PK") {
        let aer = retours::aer_spectacle(o).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
        l.titre = "Archive d'attestations employeur (AER)".into();
        if aer.is_empty() {
            let n = retours::membres_du_zip(o).len();
            l.phrases.push(format!("Une archive de {} fichier{}.", n, if n > 1 { "s" } else { "" }));
        } else {
            l.phrases.push(format!("{} attestation{} employeur rematérialisée{} :", aer.len(), if aer.len() > 1 { "s" } else { "" }, if aer.len() > 1 { "s" } else { "" }));
            for a in &aer {
                let p = py_str(&a["periode"]);
                l.phrases.push(format!("{}{}", py_str(&a["qui"]), if p.is_empty() { String::new() } else { format!(" (période {})", p) }));
            }
            l.phrases.push("Chaque AER se range aussi parmi les documents du mois, prête à partir au salarié.".into());
        }
        return l;
    }
    if let Some(r) = retours::lire_rapport(o) {
        let type_ = py_str(&r["type"]);
        l.titre = if vrai(&r["libelle"]) { py_str(&r["libelle"]) } else if type_.is_empty() { "Compte rendu".into() } else { format!("Rapport {}", type_) };
        if vrai(&r["message"]) {
            l.phrases.push(py_str(&r["message"]));
        }
        if r["essai_reel"] == json!("01") {
            l.phrases.push("Envoi d'essai (test) : rien n'est déclaré ni conservé.".into());
        }
        let etat = py_str(&r["etat_envoi"]);
        let envoi = match etat.as_str() {
            "OK" => "accepté",
            "KO" => "refusé",
            "ANO" => "accepté avec des anomalies",
            _ => "",
        };
        if !envoi.is_empty() {
            l.phrases.push(format!("L'envoi est {} ({}).", envoi, etat));
        }
        for d in r["declarations"].as_array().into_iter().flatten() {
            let e = py_str(&d["etat"]);
            let mois = py_str(&d["mois"]);
            let ordre = py_str(&d["ordre"]);
            l.phrases.push(format!(
                "{}{}{} : {}.",
                nature_dsn(&py_str(&d["nature"]), &py_str(&d["type"])),
                if mois.is_empty() { String::new() } else { format!(" de {}", mois) },
                if ordre.is_empty() { String::new() } else { format!(", ordre {}", ordre) },
                if verdict_en_mots(&e).is_empty() { if e.is_empty() { "sans état".to_string() } else { e.clone() } } else { format!("{} ({})", verdict_en_mots(&e), e) }
            ));
        }
        for i in r["informations"].as_array().into_iter().flatten() {
            let donnees: Vec<String> = i["donnees"].as_array().into_iter().flatten().map(|d| format!("{} : {}", py_str(&d["nom"]), py_str(&d["valeur"]))).collect();
            let t = py_str(&i["type"]);
            let quoi = match t.as_str() {
                "atmp" => "Information : taux accident du travail et maladie professionnelle".to_string(),
                "bmcc" => "Information : bonus-malus de la contribution chômage".to_string(),
                "" => "Information".to_string(),
                autre => format!("Information ({})", autre),
            };
            l.phrases.push(if donnees.is_empty() { format!("{}.", quoi) } else { format!("{} — {}.", quoi, donnees.join(" ; ")) });
        }
        if nature == retours::NATURE_TAUX_PAS {
            let n = retours::nombre_de_taux_pas(o);
            l.phrases.push(format!("{} taux de prélèvement à la source communiqué{}.", n, if n > 1 { "s" } else { "" }));
        }
        l.anomalies = r["anomalies"].as_array().cloned().unwrap_or_default();
        return l;
    }
    l.traduite = false;
    l.titre = "Compte rendu".into();
    let pourquoi = match format {
        "neores" => "au format NEOReS",
        "oc" => "au format des comptes rendus des organismes complémentaires",
        "agirc-arrco" => "au format propre à l'AGIRC-ARRCO",
        _ => "dans un format",
    };
    l.phrases.push(format!(
        "Ce compte rendu est {} que ce logiciel ne sait pas encore traduire (sa norme n'y est pas déposée) : le statut ci-dessus est celui que donne net-entreprises, et la pièce s'ouvre telle quelle.",
        pourquoi
    ));
    l
}

/// Un retour d'une ligne du journal, lu.
fn retour_lu(racine: &Path, ligne: &Value, r: &Value) -> Value {
    let quoi = ligne.get("quoi").and_then(Value::as_str).unwrap_or("");
    let canal = if quoi == "dpae" { "dpae" } else { "" };
    let nature = r.get("nature").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let statut = r.get("statut").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let publication = r.get("publication").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let flux = ligne.get("flux").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let (organisme, format) = if canal == "dpae" {
        ("l'Urssaf (DPAE)", "")
    } else {
        NATURES.iter().find(|(c, _, _)| *c == nature).map(|(_, o, f)| (*o, *f)).unwrap_or(("", ""))
    };
    let libelle = r.get("libelle").filter(|v| vrai(v)).map(py_str).unwrap_or_else(|| retours::libelle_nature(&nature));
    let piece = if flux.is_empty() && publication.is_empty() { None } else { piece_du_retour(racine, canal, &flux, &nature, &publication) };
    let mut phrases: Vec<String> = Vec::new();
    let s = phrase_du_statut(&statut);
    if !s.is_empty() {
        phrases.push(format!("Statut {} : {}.", statut, s));
    }
    let (titre, anomalies, traduite) = match piece.as_ref().and_then(|p| std::fs::read(p).ok()) {
        Some(o) => {
            let l = lire_piece(&o, canal, &nature, format);
            phrases.extend(l.phrases);
            (l.titre, l.anomalies, l.traduite)
        }
        None => {
            // Sans piece : ce que le releve en a lu quand il l'a rattachee (`journal.rattacher_retours`), et rien d'autre.
            if format == "aucun" {
                phrases.push("Un accusé de réception, sans document : son statut dit tout.".into());
            } else {
                phrases.push("La pièce de ce retour n'est pas archivée sur ce poste : ce qui est dit ici vient du relevé.".into());
            }
            let anomalies: Vec<Value> = r["anomalies"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| if a.is_string() { json!({"code": "", "categorie": "", "message": a, "rubrique": "", "ligne": "", "nombre": "", "salarie": ""}) } else { a.clone() })
                .collect();
            for d in r["dpae"].as_array().into_iter().flatten() {
                let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
                let etat = if vrai(&d["enregistree"]) { "enregistrée".to_string() } else if vrai(&d["deja_existante"]) { "déjà enregistrée auparavant".into() } else { format!("refusée : {}", t("libelle")) };
                phrases.push(format!("{} {} : {}.", t("nom"), t("prenom"), etat));
            }
            (String::new(), anomalies, piece.is_some())
        }
    };
    let verdict = r.get("verdict").filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    json!({
        "nature": nature, "libelle": libelle, "organisme": organisme, "statut": statut, "verdict": verdict,
        "quand": quand_publication(&publication), "titre": titre, "phrases": phrases, "anomalies": anomalies, "traduite": traduite,
        "piece": piece.and_then(|p| p.strip_prefix(racine).ok().map(|x| x.to_string_lossy().replace('\\', "/"))),
    })
}

// ------------------------------------------------------------------ la route

/// `historique-mois {mois}`.
pub fn route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = charge.get("mois").and_then(Value::as_str).unwrap_or("");
    let b = mois.as_bytes();
    if !(b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())) {
        return Err(erreur("ErreurOperation", format!("Mois illisible : {}.", mois)));
    }
    let lignes = crate::journal::lire(&ctx.racine, None, Some(mois), false).map_err(|m| erreur("OSError", m))?;
    let recus = etats::contrats_recus(&lignes, etats::Maille::Contrat)?;
    let mut cles: etats::Dico<()> = etats::Dico::nouveau();
    for (k, _) in &recus.items {
        cles.poser(k.clone(), ())?;
    }
    let mut envois = Vec::new();
    for l in &lignes {
        let mut retours: Vec<Value> = l["retours"].as_array().into_iter().flatten().map(|r| retour_lu(&ctx.racine, l, r)).collect();
        retours.sort_by(|a, b| py_str(&a["quand"]).cmp(&py_str(&b["quand"])));
        let t = |k: &str| l.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
        // Ce qui REVIENT (un contrat signe, une fiche remplie) n'a pas ete « envoye » et n'attend rien ; un depot attend
        // ses retours par son flux (`journal.rattacher_retours`).
        let recu = ["contrat-recu", "fiche-recue"].contains(&t("quoi").as_str());
        let resume = if recu { format!("reçu le {}", etats::quand_lisible(&l["quand"])) } else { etats::resume(l, &cles, etats::Maille::Contrat)? };
        envois.push(json!({
            "quoi": t("quoi"), "acte": acte(l), "mode": t("mode"), "quand": t("quand"), "fichier": t("fichier"), "flux": t("flux"),
            "destinataire": t("destinataire"), "accepte": vrai(&l["accepte"]), "resume": resume,
            "attend_retour": !recu && !t("flux").is_empty(), "retours": retours,
        }));
    }
    // Le plus recent en premier : c'est le dernier envoi qu'on vient lire ; a la meme heure, le reel devant son test.
    envois.sort_by(|a, b| (py_str(&b["quand"]), b["mode"] == json!("reel")).cmp(&(py_str(&a["quand"]), a["mode"] == json!("reel"))));
    Ok(json!({"mois": mois, "documents": documents(ctx, mois)?, "envois": envois}))
}

/// Le chemin d'une piece que `historique-mois` a listee pour `mois` (un document ou la piece d'un retour), ou `None` :
/// `ouvrir-piece` n'ouvre que ce que la route a montre.
pub fn piece_listee(listage: &Value, chemin: &str) -> bool {
    let doc = listage["documents"].as_array().into_iter().flatten().any(|d| d["chemin"] == json!(chemin));
    let ret = listage["envois"].as_array().into_iter().flatten().flat_map(|e| e["retours"].as_array().into_iter().flatten()).any(|r| r["piece"] == json!(chemin));
    doc || ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_publication_se_lit_en_heure() {
        assert_eq!(quand_publication("20261019171100"), "2026-10-19T17:11:00");
        assert_eq!(quand_publication(""), "");
    }

    #[test]
    fn la_nature_d_une_declaration_se_dit() {
        assert_eq!(nature_dsn("01", "01"), "DSN mensuelle");
        assert_eq!(nature_dsn("01", "03"), "DSN mensuelle (annule et remplace)");
        assert_eq!(nature_dsn("07", "04"), "Signalement fin du contrat de travail unique (annulation)");
    }
}
