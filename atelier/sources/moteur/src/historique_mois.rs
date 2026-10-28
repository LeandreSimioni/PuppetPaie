//! `historique-mois` : tout ce qu'un mois a produit, envoye et recu, pour l'Historique (tickets #219, #220, #222 du
//! depot prive ; Leandre, 20/10/2026 : « tous les envois et tous les retours se lisent dans l'onglet Historique »).
//!
//! La page Paie est l'endroit ou l'on FAIT le mois ; l'Historique, celui ou l'on VOIT ce qu'il a donne. Cette route
//! rend, pour un mois :
//!
//! - `documents` : chaque fichier de `sorties/<mois>/` et de `sorties/<mois>/envoi/` (les memes listes que
//!   l'Historique : `crate::routes::ouverture::mois_connus`, `crate::routes::archives::documents_du_mois`), range
//!   par sorte, chacun avec le chemin que `ouvrir-piece` ouvre ;
//! - `envois` : chaque ligne du journal des depots du mois (DPAE, DSN, signalements, documents, contrats), TESTS
//!   COMPRIS : le journal ne porte que ce que CE logiciel a envoye (un test d'ailleurs n'y entre pas, `reprise_journal`,
//!   PR #17), et ce qu'il a envoye se montre partout (Leandre, 20/10/2026 : « les gens ont besoin de tout savoir, le
//!   logiciel doit etre d'une transparence totale, ce sont leurs infos, leurs tests ») ;
//! - sous chaque envoi, ses `retours` dans l'ordre d'arrivee, chacun LU : l'organisme, ce que dit son statut, et ce
//!   que dit sa piece archivee (`lire_piece` : bilans harmonises, NEOReS, organismes complementaires, certificats DPAE,
//!   AER). Le format de l'AGIRC-ARRCO, que personne ne publie, garde son statut et sa piece telle quelle.
//!
//! Lecture seule : rien ne s'ecrit, rien ne part.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use envois::retours;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "historique-mois";

// ------------------------------------------------------------------ les tables

/// L'organisme et le FORMAT de chaque nature de retour de la DSN : le tableau des retours du guide d'implementation de
/// l'API DSN v6.8, §1.3, p. 11-13 (colonnes « OPS » et « Format phase 3 » ;
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`). « harmonise » :
/// le schema des bilans (`retours::lire_rapport`) ; « aucun » : un accuse sans document, son statut dit tout ; « pdf » :
/// une archive d'AER ; « neores » : le format NEOReS (`retours::lire_neores`, cahier technique depose dans
/// `atelier/reference/sources/neores/`) ; « oc » : le compte rendu des organismes complementaires (`retours::lire_crm_oc`,
/// specification et schema V1.1.4 deposes dans `gip-mds-api-dsn/`) ; « agirc-arrco » : le format temporaire de
/// l'AGIRC-ARRCO, que personne ne publie (le guide le cite, [7], sans le joindre), et qui ne se traduit donc pas. Le
/// format de la nature 93 n'est pas rempli dans le tableau du guide : « harmonise » est une INFERENCE (la nature 95, de la meme DGFiP, a son exemple sous « Format DSN harmonise », §6.3).
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

/// Le salarie d'une ligne de contrat : le nom que la ligne porte (`detail.nom`, « NOM Prenom », ecrit par
/// `crate::routes::contrats::contrat_recu_avec` et `releve_signatures`), sinon celui de sa fiche (`detail.salarie` est sa cle),
/// sinon la cle elle-meme.
fn qui_du_contrat(ctx: &Contexte, d: &Value) -> String {
    let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    if !t("nom").is_empty() {
        return t("nom");
    }
    let cle = t("salarie");
    let fiche = ctx.salarie_brut(&cle);
    let f = |k: &str| fiche.and_then(|x| x.get(k)).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let prenom = if f("prenom_affichage").is_empty() { f("prenom") } else { f("prenom_affichage") };
    let nom = [f("nom"), prenom].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" ");
    if nom.is_empty() { cle } else { nom }
}

/// Le contrat d'une ligne `contrat-recu` en mots : son numero et son debut (`detail.numero`, `detail.debut`).
fn contrat_en_mots(d: &Value) -> String {
    let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let numero = if t("numero").is_empty() { String::new() } else { format!(" n° {}", t("numero")) };
    let debut = if t("debut").is_empty() { String::new() } else { format!(" commençant le {}", etats::quand_lisible(&json!(t("debut")))) };
    format!("contrat{}{}", numero, debut)
}

/// La piece qu'une ligne `contrat-recu` note recue (ou plus) : le salarie, le contrat, son debut, l'avenant.
fn piece_de_la_mention(l: &Value) -> Option<String> {
    if l.get("quoi").and_then(Value::as_str) != Some("contrat-recu") {
        return None;
    }
    let d = &l["detail"];
    Some(["salarie", "numero", "debut", "avenant"].iter().map(|k| d.get(*k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).collect::<Vec<_>>().join("|"))
}

/// Les lignes `contrat-recu` (leurs rangs dans `lignes`) dont la mention « reçu » a ete retiree ensuite : la mention qui
/// suit, pour la meme piece, est un retrait (`recu: false`).
fn mentions_defaites(lignes: &[Value]) -> HashSet<usize> {
    let mut ordre: Vec<usize> = (0..lignes.len()).filter(|i| piece_de_la_mention(&lignes[*i]).is_some()).collect();
    ordre.sort_by(|a, b| valeur_en_texte(&lignes[*a]["quand"]).cmp(&valeur_en_texte(&lignes[*b]["quand"])).then(a.cmp(b)));
    let mut derniere: HashMap<String, usize> = HashMap::new();
    let mut defaites = HashSet::new();
    for i in ordre {
        let piece = piece_de_la_mention(&lignes[i]).unwrap_or_default();
        let retrait = lignes[i]["detail"]["recu"] == json!(false);
        if let Some(&avant) = derniere.get(&piece) {
            if retrait && lignes[avant]["detail"]["recu"] != json!(false) {
                defaites.insert(avant);
            }
        }
        derniere.insert(piece, i);
    }
    defaites
}

/// L'acte d'une ligne du journal, en mots ; un contrat qui revient ou repart dit de qui il est. Une mention « reçu »
/// retiree ensuite (`defaite`), ou son retrait, ne dit pas « revenu signé » : la piece ne l'est pas.
fn acte(ctx: &Contexte, l: &Value, defaite: bool) -> String {
    let d = l.get("detail").cloned().unwrap_or(Value::Null);
    let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let piece = if t("avenant").is_empty() { "Contrat".to_string() } else { format!("Avenant n° {}", t("avenant")) };
    match l.get("quoi").and_then(Value::as_str).unwrap_or("") {
        "contrat-recu" if d["recu"] == json!(false) => format!("{} : mention « signé » retirée — {}", piece, qui_du_contrat(ctx, &d)),
        "contrat-recu" if defaite => format!("{} noté signé, mention retirée ensuite — {}", piece, qui_du_contrat(ctx, &d)),
        "dsn" => nature_dsn(&t("nature"), &t("type")),
        "dpae" => "DPAE".into(),
        "documents" => "Documents aux salariés".into(),
        "contrat-signature" if d.get("main_propre").is_some_and(vrai) && t("acte") == "avenant" => "Avenant imprimé, à remettre en main propre".into(),
        "contrat-signature" if d.get("main_propre").is_some_and(vrai) => "Contrat imprimé, à remettre en main propre".into(),
        "contrat-signature" if t("acte") == "avenant" => "Avenant envoyé à la signature".into(),
        "contrat-signature" => "Contrat envoyé à la signature".into(),
        "contrat-recu" if !t("avenant").is_empty() => format!("Avenant n° {} revenu signé — {}", t("avenant"), qui_du_contrat(ctx, &d)),
        "contrat-recu" => format!("Contrat revenu signé — {}", qui_du_contrat(ctx, &d)),
        "contrat-contresigne" => format!("Contrat contresigné renvoyé — {}", qui_du_contrat(ctx, &d)),
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
/// `sorties/<mois>/envoi/` (`documents_du_mois`). Une archive `.zip` n'est que l'emballage
/// du fichier de meme nom qui part au depot : elle ne se liste pas.
fn documents(ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let noms = crate::routes::ouverture::mois_connus(&ctx.racine).into_iter().find(|(m, _)| m == mois).map(|(_, n)| n).unwrap_or_default();
    let mut out: Vec<Value> = Vec::new();
    for nom in noms.iter().filter(|n| !n.ends_with(".zip")) {
        out.push(json!({"nom": nom, "sorte": sorte_de(nom), "salarie": "", "chemin": format!("sorties/{}/{}", mois, nom)}));
    }
    let envoi = crate::routes::archives::documents_du_mois(ctx, &json!(mois), &json!("tout"))?;
    for d in envoi.as_array().into_iter().flatten() {
        let nom = valeur_en_texte(&d["nom"]);
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
    /// L'organisme que la piece nomme (NEOReS : `RO.B002.R005` ; OC : le producteur du compte rendu), sinon `""`.
    organisme: String,
    phrases: Vec<String>,
    anomalies: Vec<Value>,
    traduite: bool,
    /// Ce que la piece dit d'elle-meme (un rapport harmonise : `OK`, `ANO`, `KO`), sinon `""`.
    verdict: String,
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
    let mut l = Lecture { titre: String::new(), organisme: String::new(), phrases: Vec::new(), anomalies: Vec::new(), traduite: true, verdict: String::new() };
    if canal == "dpae" && nature == "41" {
        let texte: String = o.iter().map(|&b| b as char).collect();
        let lignes = declarations::dpae::lire_certificat(&texte);
        if !lignes.is_empty() {
            l.titre = "Certificat de conformité des DPAE".into();
            for x in &lignes {
                let t = |k: &str| x.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
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
                let p = valeur_en_texte(&a["periode"]);
                l.phrases.push(format!("{}{}", valeur_en_texte(&a["qui"]), if p.is_empty() { String::new() } else { format!(" (période {})", p) }));
            }
            l.phrases.push("Chaque AER se range aussi parmi les documents du mois, prête à partir au salarié.".into());
        }
        return l;
    }
    if let Some(r) = retours::lire_rapport(o) {
        let type_ = valeur_en_texte(&r["type"]);
        l.titre = if vrai(&r["libelle"]) { valeur_en_texte(&r["libelle"]) } else if type_.is_empty() { "Compte rendu".into() } else { format!("Rapport {}", type_) };
        if vrai(&r["message"]) {
            l.phrases.push(valeur_en_texte(&r["message"]));
        }
        if r["essai_reel"] == json!("01") {
            l.phrases.push("Envoi d'essai (test) : rien n'est déclaré ni conservé.".into());
        }
        let etat = valeur_en_texte(&r["etat_envoi"]);
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
            let e = valeur_en_texte(&d["etat"]);
            let mois = valeur_en_texte(&d["mois"]);
            let ordre = valeur_en_texte(&d["ordre"]);
            l.phrases.push(format!(
                "{}{}{} : {}.",
                nature_dsn(&valeur_en_texte(&d["nature"]), &valeur_en_texte(&d["type"])),
                if mois.is_empty() { String::new() } else { format!(" de {}", mois) },
                if ordre.is_empty() { String::new() } else { format!(", ordre {}", ordre) },
                if verdict_en_mots(&e).is_empty() { if e.is_empty() { "sans état".to_string() } else { e.clone() } } else { format!("{} ({})", verdict_en_mots(&e), e) }
            ));
        }
        for i in r["informations"].as_array().into_iter().flatten() {
            let donnees: Vec<String> = i["donnees"].as_array().into_iter().flatten().map(|d| format!("{} : {}", valeur_en_texte(&d["nom"]), valeur_en_texte(&d["valeur"]))).collect();
            let t = valeur_en_texte(&i["type"]);
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
        // LE VERDICT EST CELUI DE LA PIECE (21/10/2026) : France Travail publie son compte rendu « cinéma, spectacle »
        // (nature 45) toujours au statut ANOBLOQUANTE, même quand il ne relève rien (guide « De la DSN à l'attestation
        // employeur », v3.2, p. 10, dépose dans `net-entreprises-base-connaissances/`). Ce que dit la piece passe devant.
        let etats: Vec<String> = std::iter::once(valeur_en_texte(&r["etat_envoi"]))
            .chain(r["declarations"].as_array().into_iter().flatten().map(|d| valeur_en_texte(&d["etat"])))
            .filter(|e| !e.is_empty())
            .collect();
        l.verdict = if etats.iter().any(|e| e == "KO") {
            "KO".into()
        } else if etats.iter().any(|e| e == "ANO") || !l.anomalies.is_empty() {
            "ANO".into()
        } else if etats.iter().any(|e| e == "OK") {
            "OK".into()
        } else {
            String::new()
        };
        return l;
    }
    if let Some(crm) = retours::lire_neores(o) {
        lire_neores(&crm, &mut l);
        return l;
    }
    if let Some(crm) = retours::lire_crm_oc(o) {
        lire_crm_oc(&crm, &mut l);
        return l;
    }
    l.traduite = false;
    l.titre = "Compte rendu".into();
    l.phrases.push(match format {
        // Le format temporaire de l'AGIRC-ARRCO (guide v6.8, « Documents de reference », [7] : `Schéma XSD RC V07.xsd`,
        // `accuseReceptionDSNDTO.xsd`, `notificationCcmdDSNDTO.xsd`) n'est publie ni avec le guide, ni sur net-entreprises,
        // ni sur agirc-arrco.fr (releve du 20/10/2026 : `atelier/reference/sources/gip-mds-api-dsn/LISEZ-MOI.md`).
        "agirc-arrco" => "Ce compte rendu de l'AGIRC-ARRCO est dans un format que l'AGIRC-ARRCO ne publie pas : ce logiciel ne peut donc pas le traduire. Le statut ci-dessus est celui que donne net-entreprises, et la pièce s'ouvre telle quelle.".to_string(),
        _ => "Cette pièce n'a pas la forme que son format annonce : ce logiciel ne sait pas la lire. Le statut ci-dessus est celui que donne net-entreprises, et la pièce s'ouvre telle quelle.".to_string(),
    });
    l
}

/// `AAAA-MM-JJ` (ou `AAAA-MM-JJThh:mm:ss`) en `JJ/MM/AAAA` ; autre chose tel quel.
fn date_lisible(d: &str) -> String {
    etats::quand_lisible(&json!(d.chars().take(10).collect::<String>()))
}

/// `JJMMAAAA` (les dates des comptes rendus des organismes complementaires) en `JJ/MM/AAAA`.
fn date_oc(d: &str) -> String {
    let c: Vec<char> = d.chars().collect();
    if c.len() == 8 && c.iter().all(|x| x.is_ascii_digit()) {
        return format!("{}/{}/{}", c[..2].iter().collect::<String>(), c[2..4].iter().collect::<String>(), c[4..].iter().collect::<String>());
    }
    d.to_string()
}

fn non_vides(v: &[String], sep: &str) -> String {
    v.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(sep)
}

/// La premiere lettre en capitale.
fn majuscule(t: &str) -> String {
    let mut c = t.chars();
    c.next().map(|p| p.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Un compte rendu NEOReS (`retours::lire_neores`, d'apres le cahier technique NEOReS 2026.1.2 et 2027.1.0,
/// `atelier/reference/sources/neores/`), dit en phrases : qui l'emet et pourquoi, la periode, la conformite, chaque
/// action attendue (en anomalie : le code du traitement, son libelle, les rubriques et les salaries en jeu ; en phrase :
/// le controle, l'action attendue, ce que vaut l'absence de correction), les parametres a prendre en paie, les ecarts
/// d'identite, les informations.
fn lire_neores(crm: &Value, l: &mut Lecture) {
    l.titre = if s(&crm["libelle"]).is_empty() { "Compte rendu NEOReS".into() } else { s(&crm["libelle"]) };
    l.organisme = s(&crm["organisme"]);
    let pourquoi = s(&crm["declencheur_libelle"]);
    if !pourquoi.is_empty() {
        let p = majuscule(&pourquoi);
        l.phrases.push(format!("{}{}.", p, if s(&crm["code"]).is_empty() { String::new() } else { format!(" (compte rendu {})", s(&crm["code"])) }));
    }
    if crm["remplace"] == json!(true) {
        l.phrases.push("Ce compte rendu annule et remplace un compte rendu précédent de l'organisme.".into());
    }
    if crm["essai_reel"] == json!("01") {
        l.phrases.push("Il répond à un envoi d'essai (test).".into());
    }
    let (debut, fin) = (s(&crm["debut"]), s(&crm["fin"]));
    if !debut.is_empty() {
        l.phrases.push(if fin.is_empty() || fin == debut { format!("Période : {}.", date_lisible(&debut)) } else { format!("Période : du {} au {}.", date_lisible(&debut), date_lisible(&fin)) });
    }
    let metier = s(&crm["metier"]);
    match s(&crm["conformite"]).as_str() {
        "OK" => l.phrases.push(format!("Aucun écart constaté sur les données analysées{}.", if metier.is_empty() { String::new() } else { format!(" ({})", metier) })),
        "ANO" => l.phrases.push(format!("Des écarts ont été constatés sur les données analysées{} : chacun appelle une correction dans la paie, au plus tard dans la DSN suivante.", if metier.is_empty() { String::new() } else { format!(" ({})", metier) })),
        _ => {}
    }
    for a in crm["actions"].as_array().into_iter().flatten() {
        let mut rubriques: Vec<String> = Vec::new();
        let mut salaries: Vec<String> = Vec::new();
        let mut precisions: Vec<String> = Vec::new();
        for d in a["details"].as_array().into_iter().flatten() {
            if !s(&d["atypie"]).is_empty() {
                precisions.push(s(&d["atypie"]).trim_end_matches('.').to_string());
            }
            for i in d["individus"].as_array().into_iter().flatten() {
                let qui = non_vides(&[s(&i["nom"]), s(&i["prenom"])], " ");
                let contrat = s(&i["contrat"]);
                let q = if contrat.is_empty() { qui } else { format!("{} (contrat {})", qui, contrat) };
                if !q.is_empty() && !salaries.contains(&q) {
                    salaries.push(q);
                }
            }
            for x in d["donnees"].as_array().into_iter().flatten() {
                let r = s(&x["rubrique"]);
                if !r.is_empty() && !rubriques.contains(&r) {
                    rubriques.push(r.clone());
                }
                let mut v = Vec::new();
                if !s(&x["declaree"]).is_empty() {
                    v.push(format!("déclaré « {} »", s(&x["declaree"])));
                }
                if !s(&x["attendue"]).is_empty() {
                    v.push(format!("attendu « {} »", s(&x["attendue"])));
                }
                let acceptees: Vec<String> = x["acceptees"].as_array().into_iter().flatten().map(s).collect();
                if !acceptees.is_empty() {
                    v.push(format!("valeurs acceptées : {}", acceptees.join(", ")));
                }
                if !s(&x["resultat"]).is_empty() {
                    v.push(s(&x["resultat"]));
                }
                if !v.is_empty() && !r.is_empty() {
                    precisions.push(format!("{} : {}", r, v.join(", ")));
                }
            }
            if !s(&d["substituable"]).is_empty() {
                precisions.push(s(&d["substituable"]));
            }
        }
        let message = non_vides(&[s(&a["libelle"]), s(&a["resultat"])], " — ");
        // Un ecart a corriger (types 01 a 03, ou un resultat de controle) se range parmi les anomalies ; un parametre a
        // prendre (04, 05) n'en est pas une : il se dit en phrase, avec sa valeur (`RO.B008`, plus bas).
        let parametre = matches!(s(&a["type_code"]).as_str(), "04" | "05") && s(&a["resultat"]).is_empty();
        if !parametre {
            l.anomalies.push(json!({
                "code": s(&a["code"]), "categorie": s(&a["type"]), "message": message, "rubrique": rubriques.join(", "),
                "ligne": "", "nombre": s(&a["individus"]), "salarie": salaries.join(", "),
            }));
        } else if !message.is_empty() {
            l.phrases.push(format!("{} : {}.", majuscule(&s(&a["type"])), message));
        }
        let code = s(&a["code"]);
        let tete = if code.is_empty() { String::new() } else { format!("{} — ", code) };
        if !s(&a["controle"]).is_empty() {
            l.phrases.push(format!("{}Contrôle : {}", tete, s(&a["controle"])));
        }
        if !precisions.is_empty() {
            l.phrases.push(format!("{}Détail : {}.", tete, precisions.join(" ; ")));
        }
        if !s(&a["action"]).is_empty() {
            l.phrases.push(format!("{}Action attendue : {}", tete, s(&a["action"])));
        }
        for (k, quoi) in [("consequences", "Sans correction"), ("substituable", "Substitution"), ("rappel", "Avant correction par l'organisme"), ("exception", "Cas d'exception"), ("reference", "Référence"), ("aide", "Aide")] {
            if !s(&a[k]).is_empty() {
                l.phrases.push(format!("{}{} : {}", tete, quoi, s(&a[k])));
            }
        }
    }
    for p in crm["parametres"].as_array().into_iter().flatten() {
        let quoi = if s(&p["libelle"]).is_empty() { format!("Paramètre {}", s(&p["type"])) } else { format!("{} ({})", s(&p["libelle"]), s(&p["type"])) };
        let q = majuscule(&quoi);
        let periode = match (s(&p["effet"]), s(&p["fin"])) {
            (e, f) if !e.is_empty() && !f.is_empty() => format!(", du {} au {}", date_lisible(&e), date_lisible(&f)),
            (e, _) if !e.is_empty() => format!(", à partir du {}", date_lisible(&e)),
            _ => String::new(),
        };
        let rub = if s(&p["rubrique"]).is_empty() { String::new() } else { format!(", rubrique {}", s(&p["rubrique"])) };
        let calcul = if s(&p["calcule_le"]).is_empty() { String::new() } else { format!(" (calculé le {})", date_lisible(&s(&p["calcule_le"]))) };
        l.phrases.push(format!("{} : {}{}{}{}.", q, s(&p["valeur"]), periode, rub, calcul));
    }
    if crm["parametres"].as_array().is_some_and(|p| p.iter().any(|x| s(&x["type"]).starts_with('A'))) {
        l.phrases.push("Ces effectifs sont repris d'eux-mêmes dans config/effectifs.json à chaque relevé des retours : ce sont eux qui décident des seuils de cotisation.".into());
    }
    for i in crm["identites"].as_array().into_iter().flatten() {
        let declare = non_vides(&[s(&i["nom_declare"]), s(&i["prenom_declare"])], " ");
        let reference = non_vides(&[s(&i["nom_reference"]), s(&i["prenom_reference"])], " ");
        let mut ecarts = Vec::new();
        if !s(&i["nom_reference"]).is_empty() && s(&i["nom_reference"]) != s(&i["nom_declare"]) {
            ecarts.push(format!("nom de référence « {} »", s(&i["nom_reference"])));
        }
        if !s(&i["prenom_reference"]).is_empty() && s(&i["prenom_reference"]) != s(&i["prenom_declare"]) {
            ecarts.push(format!("prénom de référence « {} »", s(&i["prenom_reference"])));
        }
        if !s(&i["naissance_reference"]).is_empty() && s(&i["naissance_reference"]) != s(&i["naissance_declaree"]) {
            ecarts.push(format!("née ou né le {} selon le répertoire (déclaré : {})", date_lisible(&s(&i["naissance_reference"])), date_lisible(&s(&i["naissance_declaree"]))));
        }
        if !s(&i["nir_reference"]).is_empty() && s(&i["nir_reference"]) != s(&i["nir_declare"]) {
            ecarts.push(format!("NIR de référence {} (déclaré : {})", s(&i["nir_reference"]), s(&i["nir_declare"])));
        }
        l.phrases.push(format!(
            "Identité en écart avec le répertoire national (SNGI) : {} — {}. La fiche du salarié est à corriger avec l'identité de référence.",
            if declare.is_empty() { reference } else { declare },
            if ecarts.is_empty() { "à vérifier".to_string() } else { ecarts.join(", ") }
        ));
    }
    for i in crm["informations"].as_array().into_iter().flatten() {
        let qui: Vec<String> = i["individus"].as_array().into_iter().flatten().map(|x| non_vides(&[s(&x["nom"]), s(&x["prenom"])], " ")).filter(|x| !x.is_empty()).collect();
        let numero = if s(&i["numero"]).is_empty() { String::new() } else { format!(", n° {}", s(&i["numero"])) };
        l.phrases.push(format!(
            "Information{} : {}{}{}{}.",
            if s(&i["code"]).is_empty() { String::new() } else { format!(" {}", s(&i["code"])) },
            non_vides(&[s(&i["libelle"]), s(&i["information"])], " — "),
            numero,
            if qui.is_empty() { String::new() } else { format!(" ({})", qui.join(", ")) },
            if s(&i["aide"]).is_empty() { String::new() } else { format!(" — {}", s(&i["aide"])) }
        ));
    }
}

/// Un compte rendu d'organisme complementaire (`retours::lire_crm_oc`, d'apres la specification et le schema CRM DSN
/// OC P3 V1.1.4, `atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-*`), dit en phrases : l'organisme
/// qui l'a produit, le code retour global, son complement, les mandats SEPA a creer, le contact ; chaque retour metier en
/// anomalie (code, libelle, valeurs, rubrique, salarie).
fn lire_crm_oc(crm: &Value, l: &mut Lecture) {
    l.titre = match s(&crm["type_crm"]).as_str() {
        "2" => "Compte rendu de traitement (CRT)".into(),
        "3" => "Compte rendu complet (CRC)".into(),
        _ => "Compte rendu de l'organisme complémentaire".into(),
    };
    if crm["essai_reel"] == json!("01") {
        l.phrases.push("Il répond à un envoi d'essai (test).".into());
    }
    for d in crm["declarations"].as_array().into_iter().flatten() {
        let producteur = s(&d["producteur"]);
        if !producteur.is_empty() {
            l.organisme = producteur.clone();
        }
        let organisme = non_vides(&[producteur, s(&d["code_producteur"])], ", code ");
        let mois = s(&d["mois"]);
        l.phrases.push(format!(
            "{}{}{} : {}.",
            nature_dsn(&s(&d["nature"]), &s(&d["type"])),
            if mois.is_empty() { String::new() } else { format!(" de {}", mois) },
            if organisme.is_empty() { String::new() } else { format!(", vue par {}", organisme) },
            if s(&d["retour_global"]).is_empty() { format!("code retour {}", s(&d["code_retour"])) } else { s(&d["retour_global"]) }
        ));
        if !s(&d["complement"]).is_empty() {
            l.phrases.push(s(&d["complement"]));
        }
        let compte = |k: &str, un: &str, plusieurs: &str| -> Option<String> {
            let n = s(&d[k]);
            (!n.is_empty() && n != "0").then(|| format!("{} {}", n, if n == "1" { un } else { plusieurs }))
        };
        let comptes: Vec<String> = [
            compte("erreurs", "erreur", "erreurs"),
            compte("critiques", "erreur critique", "erreurs critiques"),
            compte("affiliations", "affiliation à prendre en compte", "affiliations à prendre en compte"),
            compte("radiations", "radiation prise en compte", "radiations prises en compte"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !comptes.is_empty() {
            l.phrases.push(format!("{}.", comptes.join(", ")));
        }
        for m in d["mandats"].as_array().into_iter().flatten() {
            let ou = if m["net_entreprises"] == json!(true) {
                "à créer sur net-entreprises (module Mandat)".to_string()
            } else if s(&m["url"]).is_empty() {
                "à créer auprès de l'organisme".to_string()
            } else {
                format!("à créer sur le site de l'organisme : {}", s(&m["url"]))
            };
            l.phrases.push(format!("Pas de mandat de prélèvement SEPA pour le compte {} : {}.", s(&m["iban"]), ou));
        }
        let contact = non_vides(&[s(&d["contact"]), s(&d["courriel"]), s(&d["telephone"])], ", ");
        if !contact.is_empty() {
            l.phrases.push(format!("Contact chez l'organisme : {}.", contact));
        }
        for r in d["retours"].as_array().into_iter().flatten() {
            let code = s(&r["code"]);
            let genre = match code.chars().next() {
                Some('E') => "erreur",
                Some('I') => "information",
                _ => "",
            };
            let mut message = vec![s(&r["libelle"])];
            if !s(&r["description"]).is_empty() {
                message.push(s(&r["description"]));
            }
            let valeur = non_vides(&[s(&r["valeur"]), if s(&r["libelle_valeur"]).is_empty() { String::new() } else { format!("({})", s(&r["libelle_valeur"])) }], " ");
            if !valeur.is_empty() {
                message.push(format!("déclaré « {} »", valeur));
            }
            let corrigee = non_vides(&[s(&r["corrigee"]), if s(&r["libelle_corrigee"]).is_empty() { String::new() } else { format!("({})", s(&r["libelle_corrigee"])) }], " ");
            if !corrigee.is_empty() {
                message.push(format!("{} « {} »", if genre == "information" { "pris en compte" } else { "attendu" }, corrigee));
            }
            if !s(&r["date_evenement"]).is_empty() {
                message.push(format!("événement du {}", date_oc(&s(&r["date_evenement"]))));
            }
            let categorie = non_vides(&[genre.to_string(), s(&r["criticite"])], ", ");
            let rubrique = non_vides(&[s(&r["rubrique"]), if s(&r["libelle_rubrique"]).is_empty() { String::new() } else { format!("({})", s(&r["libelle_rubrique"])) }], " ");
            l.anomalies.push(json!({
                "code": code, "categorie": categorie, "message": non_vides(&message, " — "), "rubrique": rubrique,
                "ligne": "", "nombre": s(&r["nombre"]), "salarie": s(&r["salarie"]),
            }));
        }
    }
}

/// Un retour d'une ligne du journal, lu.
fn retour_lu(racine: &Path, ligne: &Value, r: &Value) -> Value {
    let quoi = ligne.get("quoi").and_then(Value::as_str).unwrap_or("");
    let canal = if quoi == "dpae" { "dpae" } else { "" };
    let nature = r.get("nature").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let statut = r.get("statut").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let publication = r.get("publication").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let flux = ligne.get("flux").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let (organisme, format) = if canal == "dpae" {
        ("l'Urssaf (DPAE)", "")
    } else {
        NATURES.iter().find(|(c, _, _)| *c == nature).map(|(_, o, f)| (*o, *f)).unwrap_or(("", ""))
    };
    let libelle = r.get("libelle").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| retours::libelle_nature(&nature));
    let piece = if flux.is_empty() && publication.is_empty() { None } else { piece_du_retour(racine, canal, &flux, &nature, &publication) };
    let mut phrases: Vec<String> = Vec::new();
    let lecture = piece.as_ref().and_then(|p| std::fs::read(p).ok()).map(|o| lire_piece(&o, canal, &nature, format));
    let lu_verdict = lecture.as_ref().map(|l| l.verdict.clone()).unwrap_or_default();
    let s = phrase_du_statut(&statut);
    if !lu_verdict.is_empty() && !statut.is_empty() && lu_verdict != statut {
        phrases.push(format!(
            "Publié au statut {}, mais la pièce dit : {} ({}).",
            statut,
            if lu_verdict == "OK" && lecture.as_ref().is_some_and(|l| l.anomalies.is_empty()) { "aucune anomalie" } else { phrase_du_statut(&lu_verdict) },
            lu_verdict
        ));
    } else if !s.is_empty() {
        phrases.push(format!("Statut {} : {}.", statut, s));
    }
    let (titre, anomalies, traduite, lu_organisme) = match lecture {
        Some(l) => {
            phrases.extend(l.phrases);
            (l.titre, l.anomalies, l.traduite, l.organisme)
        }
        None => {
            // Sans piece : ce que le releve en a lu quand il l'a rattachee (`journal.rattacher_retours`), et rien d'autre.
            if format == "aucun" {
                phrases.push("Un accusé de réception, sans document : son statut dit tout.".into());
            } else if statut == "fictive" || statut == "demonstration" {
                // Le retour d'un depot joue par une compagnie fictive (`crate::releve::rattacher_simulation`) : le
                // bilan est joue sans piece ; seule l'AER (nature 46) a la sienne, fabriquee (`aer_fictive`).
                phrases.push("Compagnie fictive : ce retour est joué par le logiciel, sans pièce d'organisme.".into());
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
                let t = |k: &str| d.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
                let etat = if vrai(&d["enregistree"]) { "enregistrée".to_string() } else if vrai(&d["deja_existante"]) { "déjà enregistrée auparavant".into() } else { format!("refusée : {}", t("libelle")) };
                phrases.push(format!("{} {} : {}.", t("nom"), t("prenom"), etat));
            }
            (String::new(), anomalies, piece.is_some(), String::new())
        }
    };
    // L'organisme que la piece nomme passe devant celui de la nature (« l'organisme complementaire » : lequel).
    let organisme = if lu_organisme.is_empty() { organisme.to_string() } else { lu_organisme };
    let verdict = if lu_verdict.is_empty() { r.get("verdict").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default() } else { lu_verdict };
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
    let lignes = envois::journal::lire(&ctx.racine, None, Some(mois), false).map_err(|m| erreur("OSError", m))?;
    let recus = etats::contrats_recus(&lignes, etats::Maille::Contrat)?;
    let avenants = etats::avenants_par_contrat(&lignes, etats::Maille::Contrat)?;
    let mut cles: etats::Dico<()> = etats::Dico::nouveau();
    for (k, _) in &recus.items {
        cles.poser(k.clone(), ())?;
    }
    let mut envois = Vec::new();
    let defaites = mentions_defaites(&lignes);
    for (rang, l) in lignes.iter().enumerate() {
        let mut retours: Vec<Value> = l["retours"].as_array().into_iter().flatten().map(|r| retour_lu(&ctx.racine, l, r)).collect();
        retours.sort_by(|a, b| valeur_en_texte(&a["quand"]).cmp(&valeur_en_texte(&b["quand"])));
        let t = |k: &str| l.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        // Ce qui REVIENT (un contrat signe, une fiche remplie) n'a pas ete « envoye » et n'attend rien ; un depot attend
        // ses retours par son flux (`journal.rattacher_retours`).
        let recu = ["contrat-recu", "fiche-recue"].contains(&t("quoi").as_str());
        let quand = etats::quand_lisible(&l["quand"]);
        let resume = match t("quoi").as_str() {
            // Le contrat revenu : lequel, et comment il est revenu (une reponse « Je signe » ou une mention a la main).
            "contrat-recu" => {
                let d = &l["detail"];
                let comment = if d["recu"] == json!(false) {
                    format!("mention « reçu » retirée le {}", quand)
                } else if d["origine"] == json!("courriel") {
                    format!("signé d'un clic le {}", quand)
                } else {
                    format!("reçu signé le {}, noté à la main", quand)
                };
                format!("{} — {}", contrat_en_mots(d), comment)
            }
            _ if recu => format!("reçu le {}", quand),
            _ => etats::resume(l, &cles, &avenants, etats::Maille::Contrat)?,
        };
        envois.push(json!({
            "quoi": t("quoi"), "acte": acte(ctx, l, defaites.contains(&rang)), "mode": t("mode"), "quand": t("quand"), "fichier": t("fichier"), "flux": t("flux"),
            "destinataire": t("destinataire"), "accepte": vrai(&l["accepte"]), "resume": resume,
            "attend_retour": !recu && !t("flux").is_empty(), "retours": retours,
        }));
    }
    // Le plus recent en premier : c'est le dernier envoi qu'on vient lire ; a la meme heure, le reel devant son test.
    envois.sort_by(|a, b| (valeur_en_texte(&b["quand"]), b["mode"] == json!("reel")).cmp(&(valeur_en_texte(&a["quand"]), a["mode"] == json!("reel"))));
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
