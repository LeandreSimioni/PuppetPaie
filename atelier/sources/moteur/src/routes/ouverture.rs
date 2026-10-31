//! Les routes de l'ouverture : `etat` (tout le JSON que la page lit en s'ouvrant -- la configuration verifiee,
//! l'historique des mois, les listes ou l'on choisit, les referentiels livres avec le logiciel), `configuration`,
//! `maj-etat`, `sauvegarde-derniere`, `echeance-faite`, `journal-reprise` et `courriels-fictifs`.
//!
//! AUCUN CALCUL DE PAIE, et aucun secret lu : la boite courriel se dit par un
//! booleen (l'adresse posee et le secret present dans l'environnement ou le
//! coffre, jamais sa valeur), la compagnie fictive par son marqueur.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::routes::etapes::Moteur;
use crate::routes::fiches::exiger;
use crate::routes::mois::lire_saisie;
use super::{lire_json_tolerant, lire_texte, ou_texte, qui_de};
use declarations::champs;
use droit::catalogue::{self, Rubrique};
use droit::contexte::Contexte;
use droit::emplois::{self};
use droit::natures::{self, Nature};
use droit::permanents;
use engagement::{self};
use envois::etats::get;
use socle::echec::{erreur, Echec};
use socle::ecriture::ecrire_json;
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::valeur::{nom_type, valeur_en_texte};



type R<T> = Result<T, Echec>;

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "etat", "configuration", "maj-etat", "sauvegarde-derniere",
    "echeance-faite", "journal-reprise", "courriels-fictifs",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["echeance-faite", "journal-reprise", "courriels-fictifs"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "etat" => etat(ctx),
        // La part « configuration » de l'etat, pour les routes qui la rendent apres un enregistrement.
        "configuration" => configuration(ctx),
        "maj-etat" => version(),
        // Ce que l'index local de la sauvegarde dit du dernier passage, sans reseau (`sauvegarde::derniere`).
        "sauvegarde-derniere" => Ok(crate::compagnie::sauvegarde::derniere(&ctx.racine).en_json()),
        "echeance-faite" => echeance_faite(ctx, charge),
        "journal-reprise" => crate::compagnie::incidents::noter_ceux_des_retours(
            &ctx.racine,
            envois::reprise_journal::reprise(
                ctx,
                charge.get("ecrire").is_some_and(vrai),
                &crate::routes::qui_de(charge),
                &socle::paris::iso_secondes(socle::temps::secondes()),
            ),
        ).map(pastilles_de_la_reprise),
        "courriels-fictifs" => crate::simulation_courriels::courriels_fictifs(ctx, charge),
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

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

/// Un dictionnaire, sinon `AttributeError`.
fn objet<'a>(v: &'a Value, _qui: &str) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v))))
}

/// Un refus garde la classe que
/// la lecture rencontre (`FileNotFoundError`, `UnicodeDecodeError`, `JSONDecodeError`).
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

/// La fiche telle que l'ecran la montre. `donnees` est
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
    // Sa pastille : archivee (une information), ses erreurs (a corriger, la compagnie le peut), ce qui est a verifier,
    // ou complete.
    let erreurs = anomalies.iter().filter(|a| a["gravite"] == "erreur").count();
    let a_verifier = anomalies.len() - erreurs;
    let pluriel = |n: usize| if n > 1 { "s" } else { "" };
    // Ce qui est a corriger ou a verifier se nomme (le libelle de la case), et sa raison se lit au survol (`detail`).
    let nommer = |grave: bool| -> R<(String, String)> {
        let siennes: Vec<&Value> = anomalies.iter().filter(|a| (a["gravite"] == "erreur") == grave).collect();
        let mut cases: Vec<String> = Vec::new();
        for a in &siennes {
            let cle = valeur_en_texte(&a["champ"]);
            let libelle = champs::champ(nom, &cle)?.map(|c| c.libelle).filter(|l| !l.is_empty()).unwrap_or(cle);
            let libelle = libelle.chars().next().map(|p| p.to_lowercase().chain(libelle.chars().skip(1)).collect()).unwrap_or_default();
            if !cases.contains(&libelle) {
                cases.push(libelle);
            }
        }
        let detail = siennes.iter().map(|a| valeur_en_texte(&a["message"])).collect::<Vec<_>>().join(" ");
        Ok((cases.join(", "), detail))
    };
    let pastille = if comble.get("actif") == Some(&Value::Bool(false)) {
        super::pastille("information", "archivé")
    } else if erreurs > 0 {
        let (cases, detail) = nommer(true)?;
        let mut p = super::pastille("a_faire", &format!("{} erreur{} : {}", erreurs, pluriel(erreurs), cases));
        p["detail"] = Value::String(detail);
        p
    } else if a_verifier > 0 {
        let (cases, detail) = nommer(false)?;
        let mut p = super::pastille("a_faire", &format!("à vérifier : {}", cases));
        p["detail"] = Value::String(detail);
        p
    } else {
        super::pastille("fait", "complet")
    };
    Ok(json!({
        "nom": nom, "cle": cle, "donnees": brut(&Value::Object(comble)),
        "declare": declare, "anomalies": anomalies, "derives": derives, "pastille": pastille,
    }))
}


/// Une valeur en texte, vide si elle manque ou est vide.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// LES AUTRES ETABLISSEMENTS OUVERTS tels que la fiche les dit en dernier (ceux qui ouvrent plus tard compris) : la liste
/// datee (`etablissements::ecrire`) se montre et se saisit ; un etablissement ferme n'y est plus.
pub(crate) fn etablissements_dits(ctx: &Contexte) -> Vec<Value> {
    let derniere = droit::contexte::valeurs_datees(ctx.entreprise_brute(), "etablissements").ok().and_then(|v| v.last().map(|(_, x)| (*x).clone()));
    let principal = texte_ou_vide(ctx.entreprise.get("nic"));
    match derniere {
        Some(Value::Array(a)) => a.into_iter().filter(|e| e.is_object() && texte_ou_vide(e.get("nic")) != principal).collect(),
        _ => Vec::new(),
    }
}

/// CE QU'UN ENGAGEMENT CHOISIT quand la compagnie a d'autres etablissements que le siege : le siege (`nic` vide) puis
/// chacun, nomme par son nom et sa commune, avec son ouverture, et son dernier jour s'il est ferme (un engagement passe
/// le garde). `Null` : un seul etablissement, rien a choisir.
fn choix_d_etablissement(ctx: &Contexte) -> Value {
    let principal = texte_ou_vide(ctx.entreprise.get("nic"));
    let autres: Vec<(Value, Option<socle::date::Jour>)> = droit::contexte::histoire_des_etablissements(ctx.entreprise_brute())
        .unwrap_or_default()
        .into_iter()
        .filter(|(e, _)| texte_ou_vide(e.get("nic")) != principal)
        .collect();
    if autres.is_empty() {
        return Value::Null;
    }
    let commune = |f: &Value| texte_ou_vide(f.get("commune_affichage").filter(|v| vrai(v)).or(f.get("commune")));
    let mut out = vec![json!({"nic": "", "nom": format!("Le siège{}", Some(commune(&ctx.entreprise)).filter(|c| !c.is_empty()).map(|c| format!(", {}", c)).unwrap_or_default())})];
    for (e, fermeture) in autres {
        let nom = [texte_ou_vide(e.get("libelle")), commune(&e)].into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(", ");
        out.push(json!({
            "nic": texte_ou_vide(e.get("nic")),
            "nom": if nom.is_empty() { texte_ou_vide(e.get("nic")) } else { nom },
            "ouverture": e.get("ouverture").cloned().unwrap_or(Value::Null),
            "fermeture": fermeture.map(|j| j.iso()),
        }));
    }
    Value::Array(out)
}

/// Les lignes des autres etablissements demandent-elles leur versement mobilite ? Oui quand le siege en paie un : le
/// controle des etablissements l'exige alors de chacun (`droit::contexte::controler_etablissements`).
fn versement_mobilite_des_etablissements(ctx: &Contexte) -> bool {
    ctx.bareme_fondu(ctx.aujourdhui.an).ok().and_then(|b| b.get("versement_mobilite").cloned()).is_some_and(|v| v.is_object())
}

/// `salaries_au_mois` : les salaries qui ont, ou ont eu, un contrat paye au mois. Lu dans les engagements : la nature se
/// dit sur l'engagement (ou sur un de ses mois), pas sur la fiche. L'ecran n'ouvre le bloc « Salarie au mois » qu'a eux.
fn salaries_au_mois(ctx: &Contexte) -> R<Vec<String>> {
    let mensuelles: BTreeSet<String> = natures::natures().map_err(plain)?.iter().filter(|n| n.mensuelle()).map(|n| n.cle.clone()).collect();
    let mut trouves: BTreeSet<String> = BTreeSet::new();
    // Un fichier illisible est ignore ; les autres se lisent.
    let fichiers = engagement::stock::fichiers(&ctx.racine);
    let lus = fichiers.iter().filter_map(|f| engagement::stock::lire_fichier(&ctx.racine, f).ok().flatten());
    let objets: Vec<Map<String, Value>> =
        lus.flat_map(|m| m.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default()).filter_map(|e| e.as_object().cloned()).collect();
    for e in &objets {
        let suites = e.get(engagement::stock::SUITE).and_then(Value::as_object).into_iter().flat_map(|s| s.values());
        let natures = std::iter::once(e.get("nature")).chain(suites.map(|s| s.get("nature")));
        if !natures.flatten().filter_map(Value::as_str).any(|n| mensuelles.contains(n)) {
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
            if let Ok(s) = ctx.salarie(&valeur_en_texte(&nom)) {
                if let Some(c) = s.get("_cle").and_then(Value::as_str) {
                    trouves.insert(c.to_string());
                }
            }
        }
    }
    Ok(trouves.into_iter().collect())
}

/// La racine des donnees du poste (`contexte.RACINE`) : `PUPPETPAIE_DONNEES`,
/// sinon le dossier `prive/` voisin du code, sinon le code lui-meme. LA LIGNE DE COMMANDE SEULEMENT (`moteur COMMANDE`,
/// `moteur maj --publier`) : une route recoit sa compagnie (`o["racine"]`, `Contexte::racine`) et ne lit jamais cette
/// variable -- la fenetre sert ses compagnies dans un seul processus.
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
            Some(v) => valeur_en_texte(v),
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
                e.insert("quand".into(), json!(socle::paris::iso_secondes(socle::paris::mtime(&st))));
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

/// `boite_liee` : peut-on envoyer, et les signatures reviennent-elles seules ?
/// Une seule reponse aux deux : le mot de passe de la boite est pose. Ne lit
/// jamais ce mot de passe : l'adresse (`compte`), et si un secret existe.
pub fn boite_liee(ctx: &Contexte) -> R<bool> {
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Ok(true);
    }
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::compagnie::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let adresse = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    if adresse.trim().is_empty() {
        return Ok(false);
    }
    coffre.disponible("courriel").map_err(vers)
}

/// Les demandes de fiche parties et pas encore revenues, les plus anciennes d'abord, comme la page les lit.
fn fiches_attendues(ctx: &Contexte) -> R<Value> {
    let attendues = crate::routes::fiches::fiches_attendues(ctx)?;
    Ok(Value::Array(attendues.into_iter().map(|a| json!({"code": a.code, "adresse": a.adresse, "prenom": a.prenom, "quand": a.quand})).collect()))
}

/// La fiche telle qu'elle vaut, avec `historique` : les valeurs qu'une autre a remplacees dans la fiche datee `brute`, et
/// jusqu'a quand elles valaient (la veille du `depuis` de la suivante), avec la note du changement. La page les montre.
fn avec_historique(fiche: &Value, brute: Option<&Value>) -> R<Value> {
    let mut lignes = Vec::new();
    for (champ, _) in brute.and_then(Value::as_object).into_iter().flatten().filter(|(k, _)| !k.starts_with('_')) {
        let liste = brute.and_then(|b| b.get(champ)).and_then(Value::as_array).cloned().unwrap_or_default();
        let valeurs = droit::contexte::valeurs_datees(brute.unwrap(), champ).map_err(Echec::from)?;
        for paire in valeurs.windows(2) {
            let Some(depuis) = paire[1].0 else { continue };
            let note = liste.iter().find(|e| e.get("depuis").and_then(Value::as_str) == Some(depuis.iso().as_str())).and_then(|e| e.get("note")).cloned();
            let veille = depuis.plus_jours(-1).map_err(Echec::from)?;
            lignes.push(json!({"champ": champ, "valeur": paire[0].1, "jusqu_au": veille.iso(), "note": note.unwrap_or(Value::Null)}));
        }
    }
    let mut f = fiche.clone();
    if let (Some(o), false) = (f.as_object_mut(), lignes.is_empty()) {
        o.insert("historique".into(), Value::Array(lignes));
    }
    Ok(f)
}

/// Toute la configuration, verifiee, et ce que la page en tire sans un appel de plus.
fn configuration(ctx: &Contexte) -> R<Value> {
    let mut vue = avec_historique(&ctx.entreprise, Some(ctx.entreprise_brute()))?;
    if let Some(o) = vue.as_object_mut() {
        let autres = etablissements_dits(ctx);
        if autres.is_empty() {
            o.shift_remove("etablissements");
        } else {
            o.insert("etablissements".into(), Value::Array(autres));
        }
        // Leur histoire est celle de leurs ouvertures, que chacun dit.
        if let Some(Value::Array(h)) = o.get_mut("historique") {
            h.retain(|l| l.get("champ").and_then(Value::as_str) != Some("etablissements"));
        }
    }
    let entreprise = fiche_verifiee("entreprise", None, &vue)?;
    let salaries = ctx
        .salaries
        .iter()
        .map(|(k, v)| fiche_verifiee("salarie", Some(k), &avec_historique(v, ctx.salarie_brut(k))?))
        .collect::<R<Vec<_>>>()?;
    let spectacles = ctx
        .spectacles
        .iter()
        .map(|(k, v)| fiche_verifiee("spectacle", Some(k), &avec_historique(v, ctx.spectacle_brut(k))?))
        .collect::<R<Vec<_>>>()?;
    // LES ERREURS DE CONFIGURATION, toutes fiches comptees, que le bandeau montre (orange) : une valeur a corriger, ou un
    // champ obligatoire vide, a remplir -- les memes que la pastille de chaque fiche.
    let erreurs = std::iter::once(&entreprise)
        .chain(salaries.iter())
        .chain(spectacles.iter())
        .flat_map(|f| f["anomalies"].as_array().cloned().unwrap_or_default())
        .filter(|a| a["gravite"] == "erreur")
        .count();
    let erreurs = if erreurs > 0 { super::pastille("a_faire", &format!("{} erreur{} de configuration", erreurs, if erreurs > 1 { "s" } else { "" })) } else { Value::Null };
    let permanents = ctx.emploie_des_permanents();
    Ok(json!({
        "entreprise": entreprise,
        "salaries": salaries,
        "spectacles": spectacles,
        "compteurs": documents::ecrire_documents::compteurs(&ctx.racine)?,
        "erreurs": erreurs,
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
        "etablissements": choix_d_etablissement(ctx),
        "versement_mobilite_des_etablissements": versement_mobilite_des_etablissements(ctx),
        // Et, parmi les salaries, ceux qui sont payes au mois.
        "salaries_au_mois": if permanents { salaries_au_mois(ctx)? } else { vec![] },
        // CE QUI EST POSE : signature et logo, avec leur date.
        "identite": identite(ctx, &ctx.gabarits())?,
        // LA FICHE REMPLIE PAR LE SALARIE : sans boite liee, la demande ne peut
        // pas partir. Et ce qui attend encore sa reponse.
        "boite_liee": boite_liee(ctx)?,
        "fiches_attendues": fiches_attendues(ctx)?,
    }))
}

// ------------------------------------------------------------- l'historique

/// `SORTES` : les sortes de documents d'un mois, par le debut de leur nom.
const SORTES: [(&str, &str); 6] =
    [("Bulletins_", "bulletins"), ("Contrats_", "contrats"), ("DPAE_", "DPAE"), ("DSN_", "DSN"), ("CertificatsCS_", "certificats CS"), ("AEM_", "AEM")];

/// `mois_connus` : les mois saisis et les mois produits, du plus recent au plus
/// ancien -- `(mois, fichiers du dossier de sortie)`.
pub(crate) fn mois_connus(racine: &Path) -> Vec<(String, Vec<String>)> {
    let mut vus: BTreeSet<String> = engagement::stock::mois_saisis(racine).unwrap_or_default().into_iter().collect();
    let sorties = racine.join("sorties");
    let mut dossiers: BTreeMap<String, PathBuf> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&sorties) {
        for e in rd.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
            if socle::date::mois_valide(&nom) && e.path().is_dir() {
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
/// « a_deposer », « en_retard » ; la meme lecture que les
/// rappels : le journal des depots, et lui seul.
fn bilan_des_mois(m: &mut Moteur, ctx: &Contexte, mois: &[(String, Vec<String>)]) -> R<Value> {
    let mut out = Vec::new();
    for (nom, noms) in mois {
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, nom).map_err(|m| erreur("ValueError", m))?;
        let limite = echeance.jour;
        let en_retard = echeance.passee_au(&ctx.aujourdhui);
        // L'ETAT DE LA DSN, lu une fois (`compagnie::etat_dsn`) ; une DSN a deposer passee l'echeance est en retard.
        let importe = crate::import::mois::importe(&ctx.racine, nom)?;
        let lu = crate::compagnie::etat_dsn::du_mois(ctx, nom, &Value::Null)?;
        let etat = match lu.0["etat"].as_str().unwrap_or("a_deposer") {
            "a_deposer" if en_retard => "en_retard",
            autre => autre,
        };
        let depose = lu.deposee();
        let mut sortes: Vec<&str> = SORTES.iter().filter(|(p, _)| noms.iter().any(|n| n.starts_with(p))).map(|(_, l)| *l).collect();
        // Un mois tenu avant : ses bulletins sont ceux de l'ancien logiciel, que l'import garde.
        if importe && !crate::import::mois::bulletins_repris(&ctx.racine, nom).is_empty() {
            sortes.insert(0, "bulletins repris");
        }
        // LES AER DE FRANCE TRAVAIL : rangees avec les documents des salaries (`sorties/<mois>/envoi/`), pas a la
        // racine du mois ; elles se nomment avec les autres sortes.
        if crate::routes::documents::un_nom_commence_et_finit(&ctx.racine.join("sorties").join(nom).join("envoi"), "AER_", ".pdf") {
            let rang = sortes.iter().position(|s| *s == "AEM").unwrap_or(sortes.len());
            sortes.insert(rang, "attestations AER");
        }
        // CE QUI RESTE A FAIRE DANS LA PAGE PAIE (Historique) : une DSN a deposer, refusee ou en
        // retard, ou une DSN passee dont la paie a change depuis, tant que l'annule-et-remplace est admis. Le reste se
        // lit dans l'Historique : un mois fini n'a plus de chemin vers la page Paie.
        let a_faire = match etat {
            _ if importe => false,
            // La DSN de l'ancien logiciel, et la saisie qui ajoute au mois : ce qui reste a declarer va dans une DSN
            // suivante (`import::mois::dsn_de_l_ancien_logiciel`).
            "importee" => crate::import::mois::dsn_de_l_ancien_logiciel(ctx, nom)?.is_some_and(|d| d["engagements"].as_u64().is_some_and(|n| n > 0)),
            "passee" => vrai(&m.etat_remplacement(nom, &Value::Null)?["a_envoyer"]),
            "attente" => false,
            _ => true,
        };
        // CE QUE LA PASTILLE DIT, et sa couleur (`routes::couleur_de_l_etat`) : une DSN qui attend son bilan, ou dont
        // l'ancien logiciel garde la trace, n'est rien que la compagnie ait a faire -- une information, en gris.
        let (couleur, mots) = match etat {
            "passee" => ("vert", "DSN passée"),
            "attente" => ("gris", "DSN partie, bilan attendu"),
            "refusee" => ("rouge", "DSN refusée"),
            "en_retard" => ("orange", "DSN en retard"),
            "importee" => ("vert", "DSN déclarée avant PuppetPaie"),
            "sans_trace" => ("gris", "tenu avant PuppetPaie — sa DSN est celle de l'ancien logiciel"),
            _ => ("orange", "DSN à déposer"),
        };
        out.push(json!({
            "mois": nom, "produit": !noms.is_empty(), "dsn": etat,
            "pastille": {"couleur": couleur, "mots": mots},
            "depose_le": if depose { lu.0["quand"].clone() } else { Value::Null },
            "echeance": limite.iso(),
            "echeance_depot": echeance.en_json(),
            // Deposee, passee, et l'echeance derriere : la correction part dans
            // la DSN du mois courant.
            "corrigeable": lu.partie() && en_retard && !importe,
            "importe": importe,
            "a_faire": a_faire,
            "sortes": sortes, "fichiers": noms,
            "clos": m.exercice_clos(nom)?,
        }));
    }
    Ok(Value::Array(out))
}

/// Ce que l'Historique montre de chaque mois.
fn historique(m: &mut Moteur, ctx: &Contexte) -> Value {
    let mut mois = mois_connus(&ctx.racine);
    // Les mois tenus avant ce logiciel sont des mois comme les autres : l'Historique les montre aussi.
    for im in crate::import::mois::mois_importes(&ctx.racine).unwrap_or_default() {
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
        "plafond_annuel_heures": r.plafond_annuel_heures, "origine": r.origine,
        // A quelle forme de paie elle s'adresse (« mensuelle », « activite », ou
        // rien = les deux).
        "reservee_a": r.reservee_a(),
        "genres": genres,
        // CE QUE LA RUBRIQUE PROMET SANS QUE RIEN NE LE TIENNE.
        "non_appliques": non_appliques,
        "source": r.source,
        "dit": r.dit,
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
    let liste = |k: &str| -> Vec<String> { v.get(k).and_then(Value::as_array).map(|a| a.iter().map(valeur_en_texte).collect()).unwrap_or_default() };
    objet(&v, "metiers-ajoutes.json")?;
    Ok((liste("codes"), liste("retires")))
}

/// Ce metier est-il dans le deroulant de cette compagnie ? Ceux de
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
    m.get(k).map(valeur_en_texte).unwrap_or_default()
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
                let cle = valeur_en_texte(c);
                format!("{} ({})", noms.get(&cle).map(valeur_en_texte).unwrap_or_else(|| cle.clone()), cle)
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
    let m = if vrai(motif) { valeur_en_texte(motif) } else { String::new() };
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
        let preavis = regles.get("preavis").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("preavis")))?;
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
    if !SANS_NOTIFICATION.contains(&m.as_str()) || declarations::fctu::notification_exigee(&m).map_err(plain)? {
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

/// Les natures de contrat, telles que l'ecran doit les proposer.
/// Tout vient de `contrats-base.json`. SAUF LES NATURES PAYEES AU MOIS : elles
/// ne descendent que si la fiche entreprise dit employer des permanents.
fn natures(ctx: &Contexte) -> R<Value> {
    let toutes = natures::natures().map_err(plain)?;
    let defaut = natures::defaut().map_err(plain)?;
    let permanents = ctx.emploie_des_permanents();
    let gardees: Vec<&std::sync::Arc<Nature>> = toutes.iter().filter(|n| permanents || !n.mensuelle() || n.cle == defaut).collect();
    // L'ordre est celui du fichier, le defaut en tete.
    let premier = gardees.iter().find(|n| n.cle == defaut).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(&defaut)))?;
    let mut ordre: Vec<&Nature> = vec![premier];
    ordre.extend(gardees.iter().filter(|n| n.cle != defaut).map(|n| &***n));
    let mut sortie = Vec::new();
    for n in ordre {
        let motifs = n.motifs_rupture();
        let (mut faits, mut signales) = (Map::new(), Map::new());
        for m in &motifs {
            let code = m.get("code").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("code")))?;
            faits.insert(valeur_en_texte(&code), json!(faits_de_rupture(n, &code)?));
            signales.insert(valeur_en_texte(&code), json!(n.signalement_fctu(Some(&valeur_en_texte(&code)))));
        }
        sortie.push(json!({
            "cle": n.cle, "libelle": n.libelle, "aide": n.aide,
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

/// Les niveaux de garantie que le bareme connait, pour la
/// fiche salarie. Vide quand le bareme n'en porte pas.
fn niveaux_sante(ctx: &Contexte) -> R<Value> {
    let Ok(bar) = ctx.bareme(&ctx.aujourdhui) else { return Ok(json!([])) };
    let sec = ou_vide(ou_vide(bar.get("par_nature")).get("permanent"));
    let ajoutees = sec.get("rubriques_ajoutees").and_then(Value::as_array).cloned().unwrap_or_default();
    let releve = ajoutees.iter().find(|r| r.get("id").and_then(Value::as_str) == Some("mutuelle")).and_then(|r| r.get("_releve_svp_sante"));
    let Some(releve) = releve.filter(|v| vrai(v)) else { return Ok(json!([])) };
    let niveaux = releve.get("niveaux").and_then(Value::as_object).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("niveaux")))?;
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

/// Les conventions collectives que le bareme sait servir, pour
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
        // Lu une fois tant qu'il ne change pas (`json_en_cache`), et non a chaque appel.
        let Ok(v) = socle::fichiers::json_en_cache(&f) else { continue };
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

// -------------------------------------------------------------------- l'etat

/// La route `etat` : tout ce que la page lit en s'ouvrant.
pub fn etat(ctx: &Contexte) -> R<Value> {
    let mut m = Moteur::nouveau(ctx);
    let configuration = configuration(ctx)?;
    let historique = historique(&mut m, ctx);

    // Les listes ou l'on CHOISIT : les fiches courantes seulement. Une archive
    // reste dans `configuration`, ou on peut la relire et la reactiver.
    let mut cles: Vec<&str> = ctx.salaries_actifs();
    cles.sort();
    let salaries = cles
        .into_iter()
        .map(|k| {
            Ok(json!({"cle": k, "libelle": ctx.nomme(k)}))
        })
        .collect::<R<Vec<Value>>>()?;
    let spectacles = ctx
        .spectacles_actifs()
        .into_iter()
        .map(|k| {
            let v = &ctx.spectacles[k];
            let titre = v.get("titre").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("titre")))?;
            Ok(json!({"cle": k, "titre": titre, "numero_objet": v.get("numero_objet").cloned().unwrap_or(Value::Null)}))
        })
        .collect::<R<Vec<Value>>>()?;

    // Le catalogue qui paie, deja dans l'ordre ou le proposer.
    let cat = catalogue::catalogue(&ctx.racine, &ctx.lue_au).map_err(plain)?;
    let rubriques: Vec<Value> = cat.iter().map(rubrique_json).collect();
    // La rubrique d'une ligne neuve, par forme de paie : le moteur la choisit (`catalogue::par_defaut`), la page la pose.
    let par_defaut = |forme: &str| catalogue::par_defaut(&cat, forme).map_or(Value::Null, |r| json!(r.code));
    let rubrique_par_defaut = json!({"activite": par_defaut("activite"), "mensuelle": par_defaut("mensuelle")});

    // LA FIN D'UN CONTRAT, telle qu'elle se saisit. Les types de preavis
    // SAISISSABLES seulement.
    let fctu = json_en_cache(&dossier_du_code().join("fctu-base.json")).map_err(plain)?;
    let codes = |quoi: &str| -> R<Vec<Value>> {
        let n = fctu.get(quoi).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(quoi)))?;
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
        .ok_or_else(|| erreur("KeyError", socle::fichiers::absent("organismes")))?
        .iter()
        .map(|x| json!({"code": x.get("code").cloned().unwrap_or(Value::Null), "nom": x.get("nom").cloned().unwrap_or(Value::Null)}))
        .collect();
    let deduit = declarations::champs::urssaf_du_code_postal(&urssaf, ctx.entreprise.get("code_postal"));

    Ok(json!({
        "configuration": configuration,
        // UNE COMPAGNIE FICTIVE : les envois sont joues, et la page ne demande aucun
        // mot de passe.
        "simulation": crate::compagnie::bridage::branchement(&ctx.racine).joue(),
        // UNE FICTIVE AJOUTEE : ses envois sont refuses par le moteur (`bridage::branchement`) ; la page
        // ne demande pas le mot de passe du logiciel pour un envoi qui ne partira pas.
        "envois_refuses": crate::compagnie::bridage::branchement(&ctx.racine).refus().is_some(),
        // SUPPRIMER LA COMPAGNIE : la Configuration ne le propose que si elle se supprime -- jamais une
        // fictive livree, qui se remet a zero (`interface::suppression::genre`).
        "supprimable": crate::interface::suppression::genre(&crate::compagnie::administrateur::installation_de(&ctx.racine), &ctx.racine).is_ok(),
        // UNE COMPAGNIE CREEE EN DEMO PUIS GARDEE : sa mention, et les mois saisis en demo, ou la page la montre.
        "demo": crate::interface::suppression::mention(&ctx.racine),
        // UNE COMPAGNIE VENUE D'UN LOGICIEL PRECEDENT, DONT RIEN N'EST ENCORE REPRIS : la page s'ouvre sur l'import et
        // son guide (`import::guide::commencer_par_l_import`) ; ce qui manque pour demarrer se dit apres lui.
        "commencer_par_l_import": crate::import::guide::commencer_par_l_import(&ctx.racine),
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
        "nature_defaut": natures::defaut().map_err(plain)?,
        "types_preavis": types_preavis,
        "transaction": transaction,
        // Les niveaux de complementaire sante, pour la fiche d'un permanent.
        "niveaux_sante": niveaux_sante(ctx)?,
        // Ou, quand la convention ne la fait pas choisir sur la fiche, d'ou elle vient (`bareme::sante_hors_fiche`).
        "sante_hors_fiche": droit::bareme::sante_hors_fiche(&ctx.entreprise).map_err(plain)?,
        // Les conventions collectives servies, pour la fiche entreprise.
        "conventions": conventions(ctx)?,
        "organismes_urssaf": organismes,
        "urssaf_deduit": if deduit.is_empty() { "117".to_string() } else { deduit },
        // LES PAYS : la liste livree, triee par nom, chacun avec la codification UE que le moteur en
        // deduit. Le pays de naissance et le pays de nationalite s'y choisissent.
        "pays": socle::pays::servir().map_err(plain)?,
        // Le pays de naissance : la table « Pays » de la DSN, ou se lit S21.G00.30.015.
        "pays_naissance": socle::pays::servir_naissance().map_err(plain)?,
    }))
}

/// `echeance-faite` : coche (`fait`) ou decoche l'echeance `charge["cle"]`, au nom de `qui`.
fn echeance_faite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = exiger(charge, "cle")?;
    let fait = charge.get("fait") != Some(&Value::Bool(false));
    let Some(texte) = cle.as_str() else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(cle))));
    };
    marquer_echeance(ctx, texte, fait, &qui_de(charge))?;
    Ok(json!({"cle": texte, "fait": fait}))
}

/// Coche (ou decoche) une echeance que seul
/// l'utilisateur peut dire faite.
pub(crate) fn marquer_echeance(ctx: &Contexte, cle: &str, fait: bool, qui: &str) -> R<()> {
    let (quoi, annee) = match cle.split_once(':') {
        Some((q, a)) => (q, a),
        None => (cle, ""),
    };
    let base = lire_saisie(&socle::fichiers::dossier_du_code().join("echeancier-base.json"))?;
    let annuelles = base.get("annuelles").and_then(Value::as_array).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("annuelles")))?;
    let e = annuelles.iter().find(|x| x.get("cle").and_then(Value::as_str) == Some(quoi));
    let chiffres = !annee.is_empty() && annee.chars().all(|c| c.is_ascii_digit());
    let Some(e) = e.filter(|_| chiffres) else {
        return Err(erreur("ErreurOperation", format!("Echeance inconnue : {}.", repr(cle))));
    };
    if e.get("fait_par").and_then(Value::as_str) != Some("coche") {
        let titre = e.get("titre").and_then(Value::as_str).unwrap_or("");
        let n: i64 = annee.parse().unwrap_or(0);
        let titre = titre.replace("{annee}", annee).replace("{annee_precedente}", &(n - 1).to_string()).replace("{jour_dsn}", "");
        return Err(erreur(
            "ErreurOperation",
            format!("« {} » ne se coche pas : le logiciel le voit fait quand c'est fait.", titre),
        ));
    }
    let p = ctx.racine.join("config").join("echeancier.json");
    let mut faits = super::mois::cochees(ctx)?;
    if fait {
        faits.insert(cle.to_string(), json!({"le": ctx.aujourdhui.iso(), "qui": qui}));
    } else {
        faits.shift_remove(cle);
    }
    let d = json!({"_origine": "Les echeances que l'utilisateur a dites faites.", "fait": faits});
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    ecrire_json(&p, &d, 2).map_err(en_os)
}

/// Le contenu du fichier `VERSION`, ou `0.0.0`.
pub(crate) fn courante(code: &Path) -> R<String> {
    let p = code.join("VERSION");
    if p.exists() {
        Ok(lire_texte(&p)?.trim().to_string())
    } else {
        Ok("0.0.0".to_string())
    }
}

/// La version telechargee et controlee de l'installation (`maj::dossier_maj`), ou `None`.
pub(crate) fn prete(inst: &Path) -> R<Option<Value>> {
    let dossier = crate::compagnie::maj::dossier_maj(inst);
    let Some(d) = lire_json_tolerant(&dossier.join("prete.json"))? else {
        return Ok(None);
    };
    let o = super::objet(&d)?;
    let version = o.get("version").cloned().unwrap_or_else(|| json!(""));
    let Value::String(v) = &version else {
        return Err(erreur("TypeError", format!("prete.json : 'version' : {}", socle::fichiers::attendu("un texte", nom_type(&version)))));
    };
    if !dossier.join(v).join("logiciel").join("donnees").is_dir() {
        return Ok(None);
    }
    Ok(Some(d))
}

/// La version du logiciel, ce qui est pret, ce
/// qui est configure. Ne va voir aucun depot.
pub(crate) fn version() -> R<Value> {
    let code = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    // La mise a jour est celle de l'INSTALLATION : son reglage (`maj::reglage`), sa version prete.
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let (depot, editeur) = crate::compagnie::maj::reglage(&inst).map_err(|e| erreur("ValueError", e))?;
    // Le depot d'ou l'on recoit : celui qui est pose, sinon celui du moteur (`maj::DEPOT_PAR_DEFAUT`).
    let depot = json!(crate::compagnie::maj::depot(depot));
    let prete = prete(&inst)?;
    let courante_v = courante(&code)?;
    let avant = if code.join(".avant").join("donnees").is_dir() { json!(courante(&code.join(".avant"))?) } else { Value::Null };
    let (version_prete, calcul, exigee) = match &prete {
        Some(pr) => (pr["version"].clone(), ou_texte(get(pr, "calcul")?), ou_texte(get(pr, "exigee")?)),
        None => (Value::Null, json!(""), json!("")),
    };
    Ok(json!({
        "courante": courante_v, "editeur": editeur, "depot": depot,
        "prete": version_prete, "calcul": calcul, "exigee": exigee,
        // La cle publique de l'editeur est ecrite dans le moteur (`maj::CLE_PUBLIQUE`) : toujours posee.
        "signee": true,
        "avant": avant,
    }))
}

#[track_caller]
fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

/// Crée le dossier et ses parents ; rien s'il existe.
fn creer_dossier(d: &std::path::Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur("OSError", format!("{} : {}", d.display(), e)))
}

/// La pastille de chaque depot que la reprise du journal remet (`ajoutes`) : un depot reel accepte, fait ; refuse,
/// refuse ; un depot de test, une information.
fn pastilles_de_la_reprise(mut v: Value) -> Value {
    for d in v.get_mut("ajoutes").and_then(Value::as_array_mut).into_iter().flatten() {
        let reel = d.get("mode").and_then(Value::as_str) == Some("reel");
        let accepte = d.get("accepte").is_some_and(vrai);
        let mots = format!("{}{}", if reel { "dépôt réel" } else { "test" }, if accepte { "" } else { " — refusé" });
        d["pastille"] = super::pastille(if !reel { "information" } else if accepte { "fait" } else { "refuse" }, &mots);
    }
    v
}
