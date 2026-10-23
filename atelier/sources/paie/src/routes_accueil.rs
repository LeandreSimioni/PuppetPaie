//! L'accueil : la route `etat` (`paie route`), tout le JSON que la page lit en
//! s'ouvrant -- la configuration verifiee, l'historique des mois, les listes
//! ou l'on choisit, les referentiels livres avec le logiciel.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`, route `etat`, avec
//! `_configuration`, `_historique`, `_natures`, `_niveaux_sante`,
//! `_conventions`) et ce qu'il appelait : `operations_fiches.configuration`,
//! `salaries_au_mois`, `identite`, `operations_documents.boite_liee`,
//! `operations_questionnaire.fiches_attendues_json`,
//! `operations_exercice.bilan_des_mois`, `operations_catalogues` (`metiers`,
//! `metiers_autres_conventions`, `rubriques`), `operations_permanents.
//! faits_de_rupture`, `urssaf`, `simulation.active` -- portes a l'identique :
//! meme JSON, memes refus.
//!
//! AUCUN CALCUL DE PAIE, et aucun secret lu : la boite courriel se dit par un
//! booleen (l'adresse posee et le secret present dans l'environnement ou le
//! coffre, jamais sa valeur), la compagnie fictive par son marqueur.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::catalogue::{self, Rubrique};
use crate::champs;
use crate::contexte::Contexte;
use crate::contrats::{self, Nature};
use crate::emplois;
use crate::etats::{erreur, nom_type, py_str};
use crate::fichiers::{dossier_du_code, json_en_cache, vrai};
use crate::permanents;
use crate::retours::Echec;
use crate::routes_mois::{mois_saisis, Moteur};

type R<T> = Result<T, Echec>;

fn plain(m: impl Into<String>) -> Echec {
    Echec { classe: None, message: m.into() }
}

/// Une route de ce module. `charge` : ce que la page envoie (rien ici).
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, _charge: &Value) -> R<Value> {
    match nom {
        "etat" => etat(o, ctx),
        // La part « configuration » de l'etat, pour les routes qui la rendent
        // apres un enregistrement (`web._configuration`).
        "configuration" => configuration(ctx),
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

/// Les routes que ce module sert.
pub const ROUTES: [&str; 2] = ["etat", "configuration"];

// ------------------------------------------------------------------ les outils

/// `js.brut` d'un dictionnaire de fiche : sans les cles `_...`, a tous les
/// niveaux.
fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

/// Un nombre lu du fichier : un entier reste entier (`12`, pas `12.0`).
fn nombre(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        json!(x as i64)
    } else {
        json!(x)
    }
}

/// `v.get(...)` : un dictionnaire, sinon `AttributeError`.
fn objet<'a>(v: &'a Value, _qui: &str) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", crate::fichiers::attendu("un objet", nom_type(v))))
}

/// `json.loads(p.read_text(encoding="utf-8"))` : un refus garde la classe que
/// Python levait (`FileNotFoundError`, `UnicodeDecodeError`, `JSONDecodeError`).
fn lire_objet(p: &Path) -> R<Value> {
    let octets = std::fs::read(p).map_err(|e| {
        let classe = match e.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        };
        erreur(classe, format!("{} : {}", p.display(), e))
    })?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", format!("{} : {}", p.display(), e)))?;
    serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

fn ou_vide<'a>(v: Option<&'a Value>) -> &'a Value {
    static VIDE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    match v {
        Some(x) if vrai(x) => x,
        _ => VIDE.get_or_init(|| Value::Object(Map::new())),
    }
}

// ------------------------------------------------------------ la configuration

/// `_fiche_verifiee` : la fiche telle que l'ecran la montre. `donnees` est
/// COMBLEE : une fiche d'avant ne porte que la forme declaree, et l'ecran doit
/// proposer une forme lisible de depart. Rien n'est ecrit pour autant.
fn fiche_verifiee(nom: &str, cle: Option<&str>, donnees: &Value) -> R<Value> {
    let d = objet(donnees, nom)?;
    let anomalies = champs::verifier_fiche(nom, d)?;
    vue_fiche(nom, cle, d, anomalies, &Map::new())
}

/// La fiche telle que l'ecran la montre, avec les anomalies et les derivations que
/// l'appelant a deja calculees (`derives` : `{cle: [avant, apres]}`).
pub(crate) fn vue_fiche(nom: &str, cle: Option<&str>, donnees: &Map<String, Value>, anomalies: Vec<Value>, derives: &Map<String, Value>) -> R<Value> {
    let comble = champs::combler_affichage(nom, donnees)?;
    let declare = champs::apercu_derive(nom, &comble)?;
    Ok(json!({
        "nom": nom, "cle": cle, "donnees": brut(&Value::Object(comble)),
        "declare": declare, "anomalies": anomalies, "derives": derives,
    }))
}


/// `str(x or "")`.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => py_str(x),
        _ => String::new(),
    }
}

/// `etablissements.plusieurs` : la fiche porte-t-elle un etablissement autre
/// que le principal ?
fn plusieurs_etablissements(fiche: &Value) -> bool {
    let principal = texte_ou_vide(fiche.get("nic"));
    let entrees: Vec<&Value> = match fiche.get("etablissements").filter(|v| vrai(v)) {
        None => vec![],
        Some(Value::Array(a)) => a.iter().collect(),
        Some(autre) => vec![autre],
    };
    entrees.iter().any(|e| e.is_object() && texte_ou_vide(e.get("nic")) != principal)
}

/// `Contexte.compteurs()` : `config/compteurs.json`, ou `{}`.
fn compteurs(racine: &Path) -> R<Value> {
    let p = racine.join("config").join("compteurs.json");
    if p.exists() {
        lire_objet(&p)
    } else {
        Ok(Value::Object(Map::new()))
    }
}

/// `salaries_au_mois` : les salaries qui ont, ou ont eu, un contrat paye au
/// mois. Lu dans les saisies : la nature se dit sur l'engagement, pas sur la
/// fiche. L'ecran n'ouvre le bloc « Salarie au mois » qu'a eux.
fn salaries_au_mois(ctx: &Contexte) -> R<Vec<String>> {
    let mensuelles: BTreeSet<String> = contrats::natures().map_err(plain)?.iter().filter(|n| n.mensuelle()).map(|n| n.cle.clone()).collect();
    let mut noms_de_fichiers: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(ctx.racine.join("paies")) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".json") {
                noms_de_fichiers.push(e.path());
            }
        }
    }
    noms_de_fichiers.sort();
    let mut trouves: BTreeSet<String> = BTreeSet::new();
    for f in noms_de_fichiers {
        // Un fichier illisible ou qui n'est pas une saisie est ignore.
        // Lu une fois tant qu'il ne change pas (`json_en_cache`, ticket #244) : `etat` le relisait a chaque appel.
        let Ok(v) = crate::fichiers::json_en_cache(&f) else { continue };
        let Some(d) = v.as_object() else { continue };
        let engs = d.get("engagements").filter(|x| vrai(x)).cloned().unwrap_or(Value::Array(vec![]));
        let Value::Array(engs) = engs else { continue };
        for e in &engs {
            let Some(e) = e.as_object() else { continue };
            if !e.get("nature").and_then(Value::as_str).is_some_and(|n| mensuelles.contains(n)) {
                continue;
            }
            let noms: Vec<Value> = match e.get("salaries").filter(|x| vrai(x)) {
                Some(Value::Array(a)) => a.clone(),
                Some(autre) => vec![autre.clone()],
                None => vec![e.get("salarie").cloned().unwrap_or(Value::Null)],
            };
            for nom in noms {
                if !vrai(&nom) {
                    continue;
                }
                if let Ok(s) = ctx.salarie(&py_str(&nom)) {
                    if let Some(c) = s.get("_cle").and_then(Value::as_str) {
                        trouves.insert(c.to_string());
                    }
                }
            }
        }
    }
    Ok(trouves.into_iter().collect())
}

/// La racine des donnees du poste (`contexte.RACINE`) : `PUPPETPAIE_DONNEES`,
/// sinon le dossier `prive/` voisin du code, sinon le code lui-meme. LA LIGNE DE COMMANDE SEULEMENT (`paie COMMANDE`,
/// `paie maj --publier`) : une route recoit sa compagnie (`o["racine"]`, `Contexte::racine`) et ne lit jamais cette
/// variable -- la fenetre sert ses compagnies dans un seul processus (ticket #244).
pub(crate) fn racine_du_poste() -> PathBuf {
    if let Some(e) = std::env::var_os("PUPPETPAIE_DONNEES").filter(|e| !e.is_empty()) {
        return PathBuf::from(e);
    }
    let logiciel = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    let voisin = logiciel.parent().map(|p| p.join("prive")).unwrap_or_default();
    if voisin.join("config").is_dir() {
        voisin
    } else {
        logiciel
    }
}

/// `DOCUMENTS_IDENTITE` : ou va chaque image.
fn documents_identite(quoi: &str) -> Value {
    match quoi {
        "signature" => json!([
            "les contrats d'engagement", "l'AEM en PDF", "l'attestation employeur (AER)",
            "le certificat Congés Spectacles",
            "le certificat de travail et le reçu pour solde de tout compte"
        ]),
        "cachet" => json!(["l'attestation employeur (AER), case « Cachet de l'entreprise »"]),
        "logo" => json!([
            "les bulletins de paie", "les contrats d'engagement",
            "le certificat de travail et le reçu pour solde de tout compte"
        ]),
        _ => json!([]),
    }
}

/// `signataire_de` : la personne qui signe, telle que la fiche Entreprise la
/// nomme, en forme lisible.
pub(crate) fn signataire_de(ent: &Value) -> R<Value> {
    let s = ou_vide(ent.get("signataire"));
    objet(s, "signataire")?;
    let lisible = |affichage: &str, brut: &str| -> String {
        match s.get(affichage).filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => champs::titre(&texte_ou_vide(s.get(brut))),
        }
    };
    let (prenom, nom) = (lisible("prenom_affichage", "prenom"), lisible("nom_affichage", "nom"));
    let complet: Vec<String> = [prenom, nom].into_iter().filter(|x| !x.is_empty()).collect();
    Ok(json!({"nom": complet.join(" "), "fonction": s.get("fonction").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!(""))}))
}

/// `identite` : ce qui est pose (signature, cachet, logo), et depuis quand.
/// `gabarits` est celui du poste (`contexte.GABARITS`).
pub fn identite(ctx: &Contexte, gabarits: &Path) -> R<Value> {
    let dossier = gabarits.join("identite");
    let mut out = Map::new();
    for quoi in ["signature", "cachet", "logo"] {
        let auto = quoi == "cachet";
        let pose = [".png", ".jpg", ".svg"].iter().map(|ext| dossier.join(format!("{}{}", quoi, ext))).find(|f| f.exists());
        let mut e = Map::new();
        match pose {
            None => {
                e.insert("pose".into(), json!(false));
                e.insert("automatique".into(), json!(auto));
                e.insert("dossier".into(), json!(dossier.display().to_string()));
            }
            Some(f) => {
                let st = std::fs::metadata(&f).map_err(|x| erreur("OSError", format!("{} : {}", f.display(), x)))?;
                e.insert("pose".into(), json!(true));
                e.insert("automatique".into(), json!(auto));
                e.insert("octets".into(), json!(st.len()));
                e.insert("format".into(), json!(f.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()));
                e.insert("quand".into(), json!(crate::paris::iso_secondes(crate::paris::mtime(&st))));
                e.insert("fichier".into(), json!(f.display().to_string()));
            }
        }
        e.insert("documents".into(), documents_identite(quoi));
        out.insert(quoi.into(), Value::Object(e));
    }
    // LA SIGNATURE EST CELLE D'UNE PERSONNE : celle que la fiche Entreprise
    // nomme sous « Qui signe ». Si l'image a ete posee pour une autre, la page
    // le dit.
    let signataire = signataire_de(&ctx.entreprise)?;
    let pose = out["signature"]["pose"] == json!(true);
    let pour: Value = if pose {
        let p = dossier.join("identite.json");
        let lu = if p.exists() { lire_objet(&p)? } else { Value::Object(Map::new()) };
        ou_vide(objet(&lu, "identite.json")?.get("signature")).get("de").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let nom_signataire = signataire["nom"].as_str().unwrap_or("").to_string();
    let autre = vrai(&pour) && !nom_signataire.is_empty() && pour != Value::String(nom_signataire);
    let sig = out.get_mut("signature").and_then(Value::as_object_mut).unwrap();
    sig.insert("signataire".into(), signataire);
    sig.insert("posee_pour".into(), if vrai(&pour) { pour } else { json!("") });
    sig.insert("autre_personne".into(), json!(autre));
    Ok(Value::Object(out))
}

/// `simulation.active` : les envois de ce contexte sont-ils joues ? Le paquet
/// soude (`bridage::paquet_soude`), ou une COMPAGNIE FICTIVE (son marqueur le
/// dit, `compagnies::est_fictive`). Le logiciel simplement bride, lui, ne joue rien.
///
/// JAMAIS UNE FICTIVE AJOUTEE (ticket #236, `compagnies::est_ajoutee`), meme dans le paquet soude : ses envois ne sont pas
/// joues, ils sont refuses (`bridage::refus_d_envoi`).
pub fn simulation_active(racine: &Path) -> bool {
    use crate::interface::compagnies::{est_ajoutee, est_fictive};
    !est_ajoutee(racine) && (crate::bridage::paquet_soude() || est_fictive(racine))
}

/// `boite_liee` : peut-on envoyer, et les signatures reviennent-elles seules ?
/// Une seule reponse aux deux : le mot de passe de la boite est pose. Ne lit
/// jamais ce mot de passe : l'adresse (`compte`), et si un secret existe.
pub fn boite_liee(ctx: &Contexte) -> R<bool> {
    if simulation_active(&ctx.racine) {
        return Ok(true);
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let adresse = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    if adresse.trim().is_empty() {
        return Ok(false);
    }
    coffre.disponible("courriel").map_err(vers)
}

/// `fiches_attendues_json` : les demandes de fiche parties et pas encore
/// revenues, les plus anciennes d'abord.
fn fiches_attendues(m: &mut Moteur) -> R<Value> {
    let lignes = m.lignes()?;
    Ok(Value::Array(attendues(&lignes)?))
}

/// Les demandes de fiche parties et pas encore revenues, d'apres les lignes du journal.
pub(crate) fn attendues(lignes: &[Value]) -> R<Vec<Value>> {
    let filtrees = |quoi: &str| -> R<Vec<&Value>> { crate::etats::filtrer(&lignes, Some(quoi), None, false) };
    let detail = |l: &Value| -> Value { ou_vide(l.get("detail")).clone() };
    let closes: Vec<Value> = filtrees("fiche-recue")?.iter().map(|l| detail(l).get("code").cloned().unwrap_or(Value::Null)).collect();
    let mut out = Vec::new();
    for l in filtrees("fiche-demande")? {
        let d = detail(l);
        let code = d.get("code").cloned().unwrap_or(Value::Null);
        if !l.get("accepte").is_some_and(vrai) || !vrai(&code) || closes.contains(&code) {
            continue;
        }
        out.push(json!({
            "code": code, "adresse": d.get("adresse").cloned().unwrap_or_else(|| json!("")),
            "prenom": d.get("prenom").cloned().unwrap_or_else(|| json!("")),
            "quand": l.get("quand").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(out)
}

/// `_configuration` : toute la configuration, verifiee, et ce que la page en
/// tire sans un appel de plus.
fn configuration(ctx: &Contexte) -> R<Value> {
    let o = HashMap::new();
    let mut m = Moteur::nouveau(&o, ctx);
    configuration_de(ctx, &mut m)
}

fn configuration_de(ctx: &Contexte, m: &mut Moteur) -> R<Value> {
    let entreprise = fiche_verifiee("entreprise", None, &ctx.entreprise)?;
    let salaries = ctx.salaries.iter().map(|(k, v)| fiche_verifiee("salarie", Some(k), v)).collect::<R<Vec<_>>>()?;
    let spectacles = ctx.spectacles.iter().map(|(k, v)| fiche_verifiee("spectacle", Some(k), v)).collect::<R<Vec<_>>>()?;
    let permanents = ctx.emploie_des_permanents();
    Ok(json!({
        "entreprise": entreprise,
        "salaries": salaries,
        "spectacles": spectacles,
        "compteurs": compteurs(&ctx.racine)?,
        "description": {
            "entreprise": champs::description("entreprise")?,
            "salarie": champs::description("salarie")?,
            "spectacle": champs::description("spectacle")?,
        },
        // CE QUE LA CASE << Emploie des permanents >> COMMANDE A L'ECRAN : les
        // champs `si_permanents` restent dans la description -- les retirer, ce
        // serait les effacer au premier enregistrement. La page les replie.
        "permanents": permanents,
        // Et ce que la liste des etablissements commande.
        "etablissements": plusieurs_etablissements(&ctx.entreprise),
        // Et, parmi les salaries, ceux qui sont payes au mois.
        "salaries_au_mois": if permanents { salaries_au_mois(ctx)? } else { vec![] },
        // CE QUI EST POSE : signature et logo, avec leur date.
        "identite": identite(ctx, &ctx.gabarits())?,
        // LA FICHE REMPLIE PAR LE SALARIE : sans boite liee, la demande ne peut
        // pas partir. Et ce qui attend encore sa reponse.
        "boite_liee": boite_liee(ctx)?,
        "fiches_attendues": fiches_attendues(m)?,
    }))
}

// ------------------------------------------------------------- l'historique

/// `SORTES` : les sortes de documents d'un mois, par le debut de leur nom.
const SORTES: [(&str, &str); 6] =
    [("Bulletins_", "bulletins"), ("Contrats_", "contrats"), ("DPAE_", "DPAE"), ("DSN_", "DSN"), ("CertificatsCS_", "certificats CS"), ("AEM_", "AEM")];

/// `mois_connus` : les mois saisis et les mois produits, du plus recent au plus
/// ancien -- `(mois, fichiers du dossier de sortie)`.
pub(crate) fn mois_connus(racine: &Path) -> Vec<(String, Vec<String>)> {
    let mut vus: BTreeSet<String> = mois_saisis(racine).into_iter().collect();
    let sorties = racine.join("sorties");
    let mut dossiers: BTreeMap<String, PathBuf> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&sorties) {
        for e in rd.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            let b = nom.as_bytes();
            // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
            let est_mois = b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit());
            if est_mois && e.path().is_dir() {
                vus.insert(nom.clone());
                dossiers.insert(nom, e.path());
            }
        }
    }
    let mut out = Vec::new();
    for mois in vus.into_iter().rev() {
        let mut fichiers: Vec<String> = Vec::new();
        if let Some(d) = dossiers.get(&mois) {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    if e.path().is_file() {
                        fichiers.push(e.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            fichiers.sort();
        }
        out.push((mois, fichiers));
    }
    out
}

/// `bilan_des_mois` : chaque mois connu, du plus recent au plus ancien, avec ou
/// en est sa DSN (en mots), s'il se corrige, et quelles SORTES de documents il
/// a. L'etat de la DSN : « passee » (bilan OK), « refusee », « attente »,
/// « a_deposer », « a_ecrire », « en_retard » ; la meme lecture que les
/// rappels : le journal des depots, et lui seul.
fn bilan_des_mois(m: &mut Moteur, ctx: &Contexte, mois: &[(String, Vec<String>)]) -> R<Value> {
    let mut out = Vec::new();
    for (nom, noms) in mois {
        let e = m.dsn_du_mois(nom)?;
        let echeance = crate::echeance_dsn::de_la_compagnie(ctx, nom).map_err(|m| erreur("ValueError", m))?;
        let limite = echeance.jour;
        let en_retard = ctx.aujourdhui.ordinal() > limite.ordinal();
        let drapeau = |k: &str| e.get(k).is_some_and(vrai);
        // UN MOIS TENU AVANT CE LOGICIEL (ticket #215) n'est jamais « en retard » ni « a ecrire » : sa DSN est au
        // journal (un depot que les retours attestent y entre seul), ou c'est celle que l'import a lue (« importee »), ou
        // il n'en a aucune trace (« sans_trace ») -- et rien ne se refait ici.
        let importe = crate::mois_importe::importe(&ctx.racine, nom)?;
        let importee = !vrai(&e) && importe && crate::mois_importe::dsn_importee(&ctx.racine, nom)?.is_some();
        let etat: Option<&str> = if vrai(&e) && drapeau("refusee") {
            Some("refusee")
        } else if vrai(&e) && drapeau("passee") {
            Some("passee")
        } else if vrai(&e) {
            Some("attente")
        } else if importee {
            Some("importee")
        } else if importe {
            Some("sans_trace")
        } else if en_retard {
            Some("en_retard")
        } else {
            None
        };
        let mut sortes: Vec<&str> = SORTES.iter().filter(|(p, _)| noms.iter().any(|n| n.starts_with(p))).map(|(_, l)| *l).collect();
        // LES AER DE FRANCE TRAVAIL (#236) : rangees avec les documents des salaries (`sorties/<mois>/envoi/`), pas a la
        // racine du mois ; elles se nomment avec les autres sortes.
        if crate::routes_documents::un_nom_commence_et_finit(&ctx.racine.join("sorties").join(nom).join("envoi"), "AER_", ".pdf") {
            let rang = sortes.iter().position(|s| *s == "AEM").unwrap_or(sortes.len());
            sortes.insert(rang, "attestations AER");
        }
        // Une DPAE deposee en cours de mois fait un fichier, pas un mois ecrit :
        // c'est la DSN sur le disque qui dit « a deposer ».
        let etat = etat.unwrap_or(if sortes.contains(&"DSN") { "a_deposer" } else { "a_ecrire" });
        // CE QUI RESTE A FAIRE DANS LA PAGE PAIE (Historique, 20/10/2026) : une DSN a ecrire, a deposer, refusee ou en
        // retard, ou une DSN passee dont la paie a change depuis, tant que l'annule-et-remplace est admis. Le reste se
        // lit dans l'Historique : un mois fini n'a plus de chemin vers la page Paie.
        let a_faire = match etat {
            _ if importe => false,
            "passee" => vrai(&m.etat_remplacement(nom, &Value::Null)?["a_envoyer"]),
            "attente" => false,
            _ => true,
        };
        out.push(json!({
            "mois": nom, "produit": !noms.is_empty(), "dsn": etat,
            "depose_le": if vrai(&e) { e.get("quand").cloned().unwrap_or(Value::Null) } else { Value::Null },
            "echeance": limite.iso(),
            "echeance_depot": echeance.en_json(),
            // Deposee, passee, et l'echeance derriere : la correction part dans
            // la DSN du mois courant.
            "corrigeable": vrai(&e) && !drapeau("refusee") && en_retard && !importe,
            "importe": importe,
            "a_faire": a_faire,
            "sortes": sortes, "fichiers": noms,
            "clos": m.exercice_clos(nom)?,
        }));
    }
    Ok(Value::Array(out))
}

/// `_historique` : ce que l'Historique montre de chaque mois.
fn historique(m: &mut Moteur, ctx: &Contexte) -> Value {
    let mut mois = mois_connus(&ctx.racine);
    // Les mois tenus avant ce logiciel (ticket #215) sont des mois comme les autres : l'Historique les montre aussi.
    for im in crate::mois_importe::mois_importes(&ctx.racine).unwrap_or_default() {
        if !mois.iter().any(|(x, _)| *x == im) {
            mois.push((im, Vec::new()));
        }
    }
    mois.sort_by(|a, b| b.0.cmp(&a.0));
    match bilan_des_mois(m, ctx, &mois) {
        Ok(v) => v,
        // Une fiche entreprise incomplete (effectif, echeance) ne doit pas
        // empecher la page de s'ouvrir : on retombe sur la liste nue.
        Err(_) => Value::Array(
            mois.iter().map(|(nom, noms)| json!({"mois": nom, "produit": !noms.is_empty(), "dsn": null, "corrigeable": false, "a_faire": true, "sortes": [], "fichiers": noms})).collect(),
        ),
    }
}

// ----------------------------------------------------------- les referentiels

/// `rubriques` : le catalogue qui PAIE, dans l'ordre ou le proposer.
pub(crate) fn rubrique_json(r: &Rubrique) -> Value {
    let non_appliques: Vec<&str> = [
        ("annexe", r.annexe.as_deref()),
        ("plafond", r.plafond.as_deref()),
        ("aem", r.aem.as_deref()),
        ("dsn_type", r.dsn_type.as_deref()),
    ]
    .into_iter()
    .filter(|(_, v)| v.is_some_and(|s| !s.is_empty()))
    .map(|(c, _)| c)
    .collect();
    // Les valeurs de `genre`, quand la rubrique le demande : les cinq avantages
    // en nature du bloc 54.
    let avec_genre = r.saisie.iter().any(|s| s == "genre") && r.dsn_autres_bruts.as_deref().is_some_and(|c| catalogue::AVANTAGES_EN_NATURE.contains(&c));
    let genres: Vec<Value> = if avec_genre {
        catalogue::AVANTAGES_EN_NATURE
            .iter()
            .map(|g| {
                let libelle = catalogue::AUTRES_BRUTS.iter().find(|(k, _)| k == g).map(|(_, l)| *l).unwrap_or("");
                json!({"code": g, "libelle": libelle, "defaut": Some(*g) == r.dsn_autres_bruts.as_deref()})
            })
            .collect()
    } else {
        vec![]
    };
    json!({
        "code": r.code, "libelle": r.libelle, "unite": r.unite, "nature": r.nature, "saisie": r.saisie,
        "sens": r.sens, "heures_forfait": r.heures_forfait.map(nombre), "plafond": r.plafond,
        "aem": r.aem, "dsn_type": r.dsn_type, "dsn_autres_bruts": r.dsn_autres_bruts, "annexe": r.annexe,
        "soumis": r.soumis, "imposable": r.imposable, "conges_spectacles": r.conges_spectacles,
        "compte_pour": r.compte_pour, "plafond_mensuel": r.plafond_mensuel,
        "plafond_annuel_heures": r.plafond_annuel_heures, "origine": r.origine, "active": r.active,
        "usages": r.usages,
        // A quelle forme de paie elle s'adresse (« mensuelle », « activite », ou
        // rien = les deux).
        "reservee_a": r.reservee_a(),
        "genres": genres,
        // CE QUE LA RUBRIQUE PROMET SANS QUE RIEN NE LE TIENNE.
        "non_appliques": non_appliques,
        "source": r.source,
    })
}

/// Les preferences de metiers de la compagnie (`config/metiers-ajoutes.json`) :
/// `(codes ajoutes, codes retires)`.
pub(crate) fn preferences_metiers(racine: &Path) -> R<(Vec<String>, Vec<String>)> {
    let p = racine.join("config").join("metiers-ajoutes.json");
    if !p.exists() {
        return Ok((vec![], vec![]));
    }
    let v = lire_objet(&p)?;
    let liste = |k: &str| -> Vec<String> { v.get(k).and_then(Value::as_array).map(|a| a.iter().map(py_str).collect()).unwrap_or_default() };
    objet(&v, "metiers-ajoutes.json")?;
    Ok((liste("codes"), liste("retires")))
}

/// `_visible` : ce metier est-il dans le deroulant de cette compagnie ? Ceux de
/// sa convention le sont d'office ; elle en ajoute d'une autre, elle en retire
/// des siens (Configuration > Metiers).
pub(crate) fn metier_visible(m: &Value, idcc: &str, ajoutes: &[String], retires: &[String]) -> bool {
    let code = m.get("code").and_then(Value::as_str).unwrap_or("");
    if retires.iter().any(|c| c == code) {
        return false;
    }
    let conventions = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
    // Sans IDCC (une compagnie qui s'installe), rien ne se cache.
    let de_la_convention = conventions.is_empty() || idcc.is_empty() || conventions.iter().any(|c| c.as_str() == Some(idcc));
    de_la_convention || ajoutes.iter().any(|c| c == code)
}

fn texte_de(m: &Value, k: &str) -> String {
    m.get(k).map(py_str).unwrap_or_default()
}

/// `metiers` : le catalogue des metiers, groupe par famille (les artistes en
/// tete), chaque convention voyant ses metiers et seulement eux.
pub(crate) fn metiers(ctx: &Contexte) -> R<Value> {
    let familles = emplois::familles().map_err(plain)?;
    let ordre: HashMap<&str, usize> = familles.iter().enumerate().map(|(i, f)| (f.as_str(), i)).collect();
    let (ajoutes, retires) = preferences_metiers(&ctx.racine)?;
    let idcc = texte_ou_vide(ctx.entreprise.get("idcc"));
    let mut liste: Vec<Value> = emplois::catalogue().map_err(plain)?.into_iter().filter(|m| metier_visible(m, &idcc, &ajoutes, &retires)).collect();
    liste.sort_by_cached_key(|m| (ordre.get(texte_de(m, "famille").as_str()).copied().unwrap_or(ordre.len()), texte_de(m, "libelle").to_lowercase()));
    Ok(Value::Array(liste))
}

/// `metiers_autres_conventions` : les metiers des AUTRES conventions, ceux que
/// la compagnie peut ajouter a son deroulant.
pub(crate) fn metiers_autres_conventions(ctx: &Contexte) -> R<Value> {
    let noms = emplois::noms_des_conventions().map_err(plain)?;
    let (ajoutes, retires) = preferences_metiers(&ctx.racine)?;
    let idcc = texte_ou_vide(ctx.entreprise.get("idcc"));
    let mut out: Vec<(String, String, Value)> = Vec::new();
    for m in emplois::catalogue().map_err(plain)? {
        if metier_visible(&m, &idcc, &ajoutes, &retires) {
            continue;
        }
        let conventions = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
        let de: Vec<String> = conventions
            .iter()
            .map(|c| {
                let cle = py_str(c);
                format!("{} ({})", noms.get(&cle).map(py_str).unwrap_or_else(|| cle.clone()), cle)
            })
            .collect();
        let de = de.join(", ");
        let mut e = m.as_object().cloned().unwrap_or_default();
        e.insert("de".into(), Value::String(de.clone()));
        out.push((de, texte_de(&m, "libelle").to_lowercase(), Value::Object(e)));
    }
    out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    Ok(Value::Array(out.into_iter().map(|(_, _, v)| v).collect()))
}

/// `faits_de_rupture` : les faits a demander a l'ecran pour CE motif, et eux
/// seuls (« si compagnie simple, interface simple »).
fn faits_de_rupture(n: &Nature, motif: &Value) -> R<Vec<&'static str>> {
    const SANS_NOTIFICATION: [&str; 5] = ["031", "043", "065", "083", "084"];
    const LICENCIEMENTS: [&str; 8] = ["014", "020", "087", "088", "091", "092", "095", "096"];
    let m = if vrai(motif) { py_str(motif) } else { String::new() };
    if m.is_empty() {
        return Ok(vec![]);
    }
    let mut faits = Vec::new();
    // La rupture conventionnelle, ou le moteur refuse sans le fait (CGI 80
    // duodecies 6°).
    let sec = permanents::rupture(Some(&m)).map_err(plain)?;
    if sec.as_ref().and_then(|s| s.get("_cle")).and_then(Value::as_str) == Some("rupture_conventionnelle") {
        faits.push("en_droit_a_la_retraite");
    }
    // Un contrat a duree indeterminee dont le motif ouvre un preavis, et dont le preavis non effectue se paie : pas
    // celui que le texte n'indemnise pas (`preavis.indemnite_compensatrice.motifs_sans_indemnite` : 059, 039, 091).
    let sans_indemnite = matches!(permanents::indemnite_de_preavis(Some(&m)).map_err(plain)?, permanents::IndemniteDePreavis::Aucune(_));
    if n.duree_indeterminee && !sans_indemnite {
        let regles = permanents::regles().map_err(plain)?;
        let preavis = regles.get("preavis").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("preavis")))?;
        let sans = ou_vide(preavis.get("motifs_sans_preavis"));
        let dedans = match sans {
            Value::Object(o) => o.contains_key(&m),
            Value::Array(a) => a.iter().any(|x| x.as_str() == Some(m.as_str())),
            _ => false,
        };
        if !dedans {
            faits.push("preavis_non_effectue");
        }
    }
    // Un CDD dont l'indemnite de fin de contrat est due.
    if !n.duree_indeterminee && permanents::precarite_due(n, Some(&m)).map_err(plain)?.is_none() {
        faits.push("precarite_due");
    }
    // LES DATES ET LA TRANSACTION SELON LE MOTIF : une rupture decidee par l'un
    // des deux se NOTIFIE ; un licenciement s'ENGAGE par la convocation a
    // l'entretien prealable ; une rupture conventionnelle se date par la
    // SIGNATURE de la convention ; la TRANSACTION suit un litige.
    // Et toujours quand le signalement l'exige (CCH-12 de S21.G00.62.003, `fctu::notification_exigee`) : la force majeure
    // (083) est de la table ci-dessus, et sa date de notification est pourtant obligatoire dans un FCTU.
    if !SANS_NOTIFICATION.contains(&m.as_str()) || crate::fctu::notification_exigee(&m).map_err(plain)? {
        faits.push("notification_rupture");
    }
    if LICENCIEMENTS.contains(&m.as_str()) {
        faits.push("engagement_licenciement");
    }
    if m == "043" {
        faits.push("signature_convention");
    }
    if LICENCIEMENTS.contains(&m.as_str()) || m == "038" || m == "058" {
        faits.push("transaction");
    }
    Ok(faits)
}

/// `_natures` : les natures de contrat, telles que l'ecran doit les proposer.
/// Tout vient de `contrats-base.json`. SAUF LES NATURES PAYEES AU MOIS : elles
/// ne descendent que si la fiche entreprise dit employer des permanents.
fn natures(ctx: &Contexte) -> R<Value> {
    let toutes = contrats::natures().map_err(plain)?;
    let defaut = contrats::defaut().map_err(plain)?;
    let permanents = ctx.emploie_des_permanents();
    let gardees: Vec<&std::sync::Arc<Nature>> = toutes.iter().filter(|n| permanents || !n.mensuelle() || n.cle == defaut).collect();
    // L'ordre est celui du fichier, le defaut en tete.
    let premier = gardees.iter().find(|n| n.cle == defaut).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(&defaut)))?;
    let mut ordre: Vec<&Nature> = vec![premier];
    ordre.extend(gardees.iter().filter(|n| n.cle != defaut).map(|n| &***n));
    let mut sortie = Vec::new();
    for n in ordre {
        let motifs = n.motifs_rupture();
        let (mut faits, mut signales) = (Map::new(), Map::new());
        for m in &motifs {
            let code = m.get("code").cloned().ok_or_else(|| erreur("KeyError", crate::fichiers::absent("code")))?;
            faits.insert(py_str(&code), json!(faits_de_rupture(n, &code)?));
            signales.insert(py_str(&code), json!(n.signalement_fctu(Some(&py_str(&code)))));
        }
        sortie.push(json!({
            "cle": n.cle, "libelle": n.libelle,
            "mensuelle": n.mensuelle(), "duree_indeterminee": n.duree_indeterminee,
            // Un permanent ne se rattache a AUCUN spectacle : le numero d'objet
            // (S21.G00.40.050) est INTERDIT hors des natures 02 et 10 (CCH-11).
            "spectacle": n.interdit("S21.G00.40.050").map_or(true, |s| s.is_empty()),
            // La fin se signale-t-elle (`Nature::signalement_fctu`) : au motif par defaut, et motif par motif -- un CDDU
            // a son terme (031) part dans la mensuelle, rompu avant (036, 037...) il se signale. La page ne decide rien.
            "signalement_fctu": n.signalement_fctu(None),
            "signalement_par_motif": signales,
            "motifs_rupture": motifs,
            "faits_de_rupture": faits,
            "motif_rupture_defaut": n.motif_rupture_par_defaut(),
            "motifs_recours": n.motifs_recours(),
            "motif_recours_exige": n.motif_recours_exige(),
            "quotite_defaut": n.quotite_reference_defaut.map(nombre),
            "unite_quotite": n.unite_quotite,
        }));
    }
    Ok(Value::Array(sortie))
}

/// `_niveaux_sante` : les niveaux de garantie que le bareme connait, pour la
/// fiche salarie. Vide quand le bareme n'en porte pas.
fn niveaux_sante(ctx: &Contexte) -> R<Value> {
    let Ok(bar) = ctx.bareme(&ctx.aujourdhui) else { return Ok(json!([])) };
    let sec = ou_vide(ou_vide(bar.get("par_nature")).get("permanent"));
    let ajoutees = sec.get("rubriques_ajoutees").and_then(Value::as_array).cloned().unwrap_or_default();
    let releve = ajoutees.iter().find(|r| r.get("id").and_then(Value::as_str) == Some("mutuelle")).and_then(|r| r.get("_releve_svp_sante"));
    let Some(releve) = releve.filter(|v| vrai(v)) else { return Ok(json!([])) };
    let niveaux = releve.get("niveaux").and_then(Value::as_object).ok_or_else(|| erreur("KeyError", crate::fichiers::absent("niveaux")))?;
    let mut tries: Vec<(&String, &Value)> = niveaux.iter().collect();
    tries.sort_by(|a, b| a.0.cmp(b.0));
    Ok(Value::Array(
        tries
            .into_iter()
            .map(|(n, t)| {
                let champ = |k: &str| t.get(k).cloned().unwrap_or(Value::Null);
                json!({"niveau": n, "assure": champ("assure"), "conjoint": champ("conjoint"), "enfant": champ("enfant")})
            })
            .collect(),
    ))
}

/// `_conventions` : les conventions collectives que le bareme sait servir, pour
/// la fiche. La page ne les invente pas, et elle n'en invente pas le libelle :
/// `par_convention` du socle dit lesquelles existent, et si leur couche de taux
/// est RELEVEE (`applicable`).
fn conventions(ctx: &Contexte) -> R<Value> {
    let dossier = ctx.code.join("baremes");
    let mut fichiers: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dossier) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".json") {
                fichiers.push(e.path());
            }
        }
    }
    // Le socle de l'annee en cours, et a defaut le plus recent livre : la liste
    // des conventions ne depend pas d'un mois de paie.
    fichiers.sort();
    fichiers.reverse();
    let voulu = dossier.join(format!("{}.json", ctx.aujourdhui.an));
    let mut candidats: Vec<PathBuf> = if voulu.exists() { vec![voulu] } else { vec![] };
    candidats.extend(fichiers);
    let mut par = Map::new();
    for f in candidats {
        // Lu une fois tant qu'il ne change pas (`json_en_cache`, ticket #244) : `etat` le relisait a chaque appel.
        let Ok(v) = crate::fichiers::json_en_cache(&f) else { continue };
        let Some(d) = v.as_object() else { continue };
        if let Some(Value::Object(p)) = d.get("par_convention").filter(|x| vrai(x)) {
            par = p.clone();
        }
        if !par.is_empty() {
            break;
        }
    }
    let mut items: Vec<(&String, &Value)> = par.iter().filter(|(k, _)| !k.starts_with('_')).collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    Ok(Value::Array(
        items
            .into_iter()
            .map(|(k, v)| {
                let libelle = v.get("libelle").filter(|l| vrai(l)).cloned().unwrap_or_else(|| json!(k));
                json!({"idcc": k, "libelle": libelle, "applicable": v.get("applicable") != Some(&Value::Bool(false))})
            })
            .collect(),
    ))
}

/// `urssaf.pour_departement` : l'organisme du TERRITOIRE de ce departement. Une
/// suggestion, pas une decision.
fn urssaf_du_departement(base: &Value, dep: &str) -> String {
    let d = dep.trim().to_uppercase();
    let table = base.get("par_departement").and_then(Value::as_object);
    let lu = |k: &str| table.and_then(|t| t.get(k)).filter(|v| vrai(v)).map(py_str);
    lu(&d).or_else(|| if d == "2A" || d == "2B" { lu("20") } else { None }).unwrap_or_default()
}

/// `urssaf.pour_code_postal` : idem, depuis un code postal francais. L'outre-mer
/// se lit sur TROIS chiffres (97139 -> 971), la metropole sur deux.
pub(crate) fn urssaf_du_code_postal(base: &Value, cp: Option<&Value>) -> String {
    let v: Vec<char> = texte_ou_vide(cp).chars().filter(|c| c.is_ascii_digit()).collect();
    if v.len() < 2 {
        return String::new();
    }
    let n = if v[0] == '9' && (v[1] == '7' || v[1] == '8') { 3 } else { 2 };
    urssaf_du_departement(base, &v.iter().take(n).collect::<String>())
}

// -------------------------------------------------------------------- l'etat

/// La route `etat` : tout ce que la page lit en s'ouvrant.
pub fn etat(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let mut m = Moteur::nouveau(o, ctx);
    let configuration = configuration_de(ctx, &mut m)?;
    let historique = historique(&mut m, ctx);

    // Les listes ou l'on CHOISIT : les fiches courantes seulement. Une archive
    // reste dans `configuration`, ou on peut la relire et la reactiver.
    let mut cles: Vec<&str> = ctx.salaries_actifs();
    cles.sort();
    let salaries = cles
        .into_iter()
        .map(|k| {
            let v = &ctx.salaries[k];
            let nom = v.get("nom").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("nom")))?;
            let prenom = match v.get("prenom_affichage").filter(|p| vrai(p)) {
                Some(p) => p,
                None => v.get("prenom").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("prenom")))?,
            };
            Ok(json!({"cle": k, "libelle": format!("{} {}", py_str(nom), py_str(prenom))}))
        })
        .collect::<R<Vec<Value>>>()?;
    let spectacles = ctx
        .spectacles_actifs()
        .into_iter()
        .map(|k| {
            let v = &ctx.spectacles[k];
            let titre = v.get("titre").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("titre")))?;
            Ok(json!({"cle": k, "titre": titre, "numero_objet": v.get("numero_objet").cloned().unwrap_or(Value::Null)}))
        })
        .collect::<R<Vec<Value>>>()?;

    // Le catalogue qui paie, deja dans l'ordre ou le proposer.
    let cat = catalogue::catalogue(&ctx.racine).map_err(plain)?;
    let rubriques: Vec<Value> = cat.iter().map(rubrique_json).collect();
    // La rubrique d'une ligne neuve, par forme de paie : le moteur la choisit (`catalogue::par_defaut`), la page la pose.
    let par_defaut = |forme: &str| catalogue::par_defaut(&cat, forme).map_or(Value::Null, |r| json!(r.code));
    let rubrique_par_defaut = json!({"activite": par_defaut("activite"), "mensuelle": par_defaut("mensuelle")});

    // LA FIN D'UN CONTRAT, telle qu'elle se saisit. Les types de preavis
    // SAISISSABLES seulement.
    let fctu = json_en_cache(&dossier_du_code().join("fctu-base.json")).map_err(plain)?;
    let codes = |quoi: &str| -> R<Vec<Value>> {
        let n = fctu.get(quoi).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(quoi)))?;
        Ok(n.get("codes").and_then(Value::as_array).cloned().unwrap_or_default())
    };
    let types_preavis: Vec<Value> = codes("types_preavis")?.into_iter().filter(|c| c.get("saisissable").is_some_and(vrai)).collect();
    let transaction = codes("transaction")?;

    // Les organismes de recouvrement, pour que le code Urssaf de la DPAE se
    // CHOISISSE au lieu de se taper ; et celui que le moteur prend quand la
    // fiche n'en dit rien : l'organisme du territoire.
    let urssaf = json_en_cache(&dossier_du_code().join("urssaf-dpae-base.json")).map_err(plain)?;
    let organismes: Vec<Value> = urssaf
        .get("organismes")
        .and_then(Value::as_array)
        .ok_or_else(|| erreur("KeyError", crate::fichiers::absent("organismes")))?
        .iter()
        .map(|x| json!({"code": x.get("code").cloned().unwrap_or(Value::Null), "nom": x.get("nom").cloned().unwrap_or(Value::Null)}))
        .collect();
    let deduit = urssaf_du_code_postal(&urssaf, ctx.entreprise.get("code_postal"));

    Ok(json!({
        "configuration": configuration,
        // UNE COMPAGNIE FICTIVE : les envois sont joues, et la page ne demande aucun
        // mot de passe.
        "simulation": simulation_active(&ctx.racine),
        // UNE FICTIVE AJOUTEE (ticket #236) : ses envois sont refuses par le moteur (`bridage::refus_d_envoi`) ; la page
        // ne demande pas le mot de passe du logiciel pour un envoi qui ne partira pas.
        "envois_refuses": crate::bridage::refus_d_envoi(&ctx.racine).is_some(),
        // SUPPRIMER LA COMPAGNIE (ticket #236) : la Configuration ne le propose que si elle se supprime -- jamais une
        // fictive livree, qui se remet a zero (`interface::suppression::genre`).
        "supprimable": crate::interface::suppression::genre(&crate::administrateur::installation_de(&ctx.racine), &ctx.racine).is_ok(),
        // UNE COMPAGNIE CREEE EN DEMO PUIS GARDEE : sa mention, et les mois saisis en demo, ou la page la montre.
        "demo": crate::interface::suppression::mention(&ctx.racine),
        "historique": historique,
        "salaries": salaries,
        "spectacles": spectacles,
        "rubriques": rubriques,
        "rubrique_par_defaut": rubrique_par_defaut,
        // Le catalogue des metiers, deja dans l'ordre des familles.
        "emplois": metiers(ctx)?,
        "familles_emplois": emplois::familles().map_err(plain)?,
        // Les metiers des autres conventions : on les AJOUTE a son deroulant,
        // ils n'y sont pas d'office.
        "emplois_autres": metiers_autres_conventions(ctx)?,
        // LES NATURES DE CONTRAT. Un engagement qui ne dit rien est un CDDU du
        // spectacle : c'est le defaut que l'ecran garde.
        "natures": natures(ctx)?,
        "nature_defaut": contrats::defaut().map_err(plain)?,
        "types_preavis": types_preavis,
        "transaction": transaction,
        // Les niveaux de complementaire sante, pour la fiche d'un permanent.
        "niveaux_sante": niveaux_sante(ctx)?,
        // Les conventions collectives servies, pour la fiche entreprise.
        "conventions": conventions(ctx)?,
        "organismes_urssaf": organismes,
        "urssaf_deduit": if deduit.is_empty() { "117".to_string() } else { deduit },
        // LES PAYS (vague 1k) : la liste livree, triee par nom, chacun avec la codification UE que le moteur en
        // deduit. Le pays de naissance et le pays de nationalite s'y choisissent.
        "pays": crate::pays::servir().map_err(plain)?,
        // Le pays de naissance (lot R2) : la table « Pays » de la DSN, ou se lit S21.G00.30.015.
        "pays_naissance": crate::pays::servir_naissance().map_err(plain)?,
    }))
}
