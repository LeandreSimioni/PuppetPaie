//! SUPPRIMER UNE COMPAGNIE, ET GARDER AU DEBLOCAGE CELLES QU'ON A AJOUTEES EN DEMO (ticket #236, points 3 et 4 ; Leandre,
//! 22/10/2026). Tout se decide ici ; la fenetre (`interface/mod.rs`) passe la saisie de la page et lache ce qu'elle tient
//! de la compagnie (sa veille, sa file sur le fil des calculs, son journal) au moment ou on le lui dit.
//!
//! SUPPRIMER (`supprimer`). Une FICTIVE LIVREE ne se supprime pas : elle se remet a zero (`compagnies::remettre_a_zero`),
//! et la route le refuse (`genre`). Une FICTIVE AJOUTEE (`compagnies::est_ajoutee`) se supprime sur une confirmation simple
//! de la page. Une COMPAGNIE REELLE (`prive/`, `compagnies/<nom>`) porte des documents qu'un employeur conserve (le double
//! des bulletins, cinq ans : code du travail, article L3243-4, `atelier/reference/sources/code-du-travail/
//! articles-20261021.md`) : la supprimer est irreversible, et se confirme PAR LE MOT DE PASSE DU LOGICIEL, tape par un
//! humain, comme un acte reel -- le popup de la page (`demanderMotDePasse`, `js/boites.js`) le met dans la charge ;
//! sans lui, le moteur le demande lui-meme (`demande::mot_de_passe_de_l_acte` : fenetre, terminal, ou refus). Il est
//! verifie sur le coffre de la compagnie ; un refus n'efface rien.
//!
//! EFFACER SUREMENT (`effacer`). Sous Windows, un dossier dont un fichier est ouvert s'efface a moitie. Le dossier est
//! d'abord RENOMME en dossier cache a cote (`.supprimee-<nom>-<instant>`) -- d'un coup : la tuile disparait --, puis
//! efface ; ce qui ne s'efface pas reste cache (l'accueil, la mise a jour et l'administrateur ignorent un dossier qui
//! commence par un point) et s'efface au lancement suivant (`effacer_les_restes`). Avec la compagnie partent : son
//! entree au registre de l'administrateur (`administrateur::oublier`), sa seance gardee a cote de la cle du poste
//! (`Coffre::oublier_ouverture_gardee`). Le prechauffage (`compagnies::a_tenir_a_jour`) et la derniere ouverte (la date
//! de son `journal/interface.log`) se lisent sur le disque : rien a defaire.
//!
//! AU DEBLOCAGE (`a_garder`, `repondre`). Un logiciel debloque qui porte encore des fictives ajoutees demande, a l'accueil,
//! lesquelles garder ; tant qu'il en reste, la question revient. Celle qu'on GARDE devient une compagnie reelle
//! (`garder`) : elle passe sous `compagnies/`, perd son marqueur de fictive (`COPIE-DE-DEVELOPPEMENT.json`) et recoit sa
//! MENTION DE DEMO (`config/demo.json` : quand elle a ete creee, quand elle a ete gardee, les mois saisis jusque-la). Celle
//! qu'on ne garde pas est supprimee, par `supprimer` (une fictive ajoutee : sans mot de passe).
//!
//! CE QUI SUIT UNE COMPAGNIE GARDEE, tel quel : ses saisies, ses mois importes, ses journaux, ses fichiers ecrits. SON
//! COFFRE AUSSI : dans une fictive ajoutee il n'est scelle que par la cle du poste (elle s'ouvre sans mot de passe) ; une
//! compagnie reelle sans mot de passe du logiciel le fait choisir a sa premiere ouverture (`Phase::MotDePasse`, premiere),
//! et `Coffre::poser_ouverture` rescelle alors ce coffre, la cle du poste l'ouvrant -- ce qui y a ete pose en demo (un mot
//! de passe d'organisme) est garde, scelle comme celui de toute compagnie reelle. Rien n'est a reposer. Ses etats gardes
//! se refont seuls : leur empreinte porte le chemin de la compagnie (`etat_garde::empreinte_de`).
//!
//! LES PAIES FAITES EN DEMO n'ont ete ni declarees ni envoyees : en demo, tout envoi est refuse avant d'ecrire au journal
//! des depots (`bridage::refus_d_envoi`). La page d'un mois les montre donc a faire (une DPAE « rien n'est parti », meme
//! pour une embauche passee : `routes_mois`, `etats.dpae.fait` nul) ; la mention de demo le dit en plus, sur chaque mois
//! saisi en demo (`mention`, lue par la route `etat`). ELLES COMPTENT au nombre de paies annuel (`nombre_de_paies`) si
//! elles sont validees : ce sont des paies que le logiciel a faites, la regle du compte (une paie validee) ne regarde pas
//! si elle est partie ; une paie non validee ne compte pas, en demo comme ailleurs.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::compagnies::{self, Compagnie};
use crate::administrateur::Poste;

/// La mention de demo d'une compagnie gardee.
pub const FICHIER_DEMO: &str = "config/demo.json";
/// Le debut du nom d'un dossier en cours d'effacement.
pub const PREFIXE_RESTE: &str = ".supprimee-";
/// Ce que la boite d'une compagnie reelle rappelle avant de la supprimer (code du travail, article L3243-4, `atelier/
/// reference/sources/code-du-travail/articles-20261021.md`).
pub const CONSERVATION: &str = "L'employeur conserve un double des bulletins de paie pendant cinq ans (code du travail, article L3243-4).";

/// Ce qu'est une compagnie pour sa suppression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Une fictive ajoutee : une confirmation simple.
    Ajoutee,
    /// Une compagnie reelle : le mot de passe du logiciel.
    Reelle,
}

fn absolu(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let t = c.to_string_lossy();
    t.strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(c)
}

/// La compagnie `racine` est-elle une compagnie de l'installation `inst` -- `prive/`, `compagnies/<nom>`,
/// `fictives/<nom>`, et rien d'autre (ni `logiciel/`, ni un dossier cache) ?
fn de_l_installation(inst: &Path, racine: &Path) -> bool {
    let (i, r) = (absolu(inst), absolu(racine));
    let nom = r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if nom.is_empty() || nom.starts_with('.') || !r.join("config").join("entreprise.json").is_file() {
        return false;
    }
    r == i.join("prive") || r.parent().is_some_and(|p| p == i.join("compagnies") || p == i.join(compagnies::DOSSIER_FICTIVES))
}

/// Ce qu'est `racine` pour sa suppression, ou pourquoi elle ne se supprime pas.
pub fn genre(inst: &Path, racine: &Path) -> Result<Genre, String> {
    if !de_l_installation(inst, racine) {
        return Err(format!("{} n'est pas une compagnie de ce logiciel : rien n'a été supprimé.", racine.display()));
    }
    if compagnies::est_ajoutee(racine) {
        return Ok(Genre::Ajoutee);
    }
    if compagnies::est_fictive(racine) {
        return Err("Une compagnie fictive livrée ne se supprime pas : elle se remet à zéro, depuis sa tuile de l'accueil.".into());
    }
    if compagnies::est_copie(racine) {
        return Err("Une copie de test ne se supprime pas depuis le logiciel.".into());
    }
    Ok(Genre::Reelle)
}

/// LA QUESTION de la suppression, telle que la page la pose (route `compagnie-suppression`) : le nom, le genre, et pour
/// une compagnie reelle le rappel de la conservation et la sauvegarde a proposer (un Drive lie : la date du dernier
/// passage). Refuse ce qui ne se supprime pas.
pub fn question(racine: &Path) -> Result<Value, String> {
    let inst = crate::administrateur::installation_de(racine);
    let g = genre(&inst, racine)?;
    let mut v = json!({"nom": compagnies::nom_de(racine), "genre": if g == Genre::Ajoutee { "ajoutee" } else { "reelle" }, "mot_de_passe": g == Genre::Reelle});
    if g == Genre::Reelle {
        v["conservation"] = json!(CONSERVATION);
        let liee = racine.join("config").join(crate::routes_google::FICHIER_JETON).is_file();
        v["sauvegarde"] = json!({"liee": liee, "derniere": crate::sauvegarde::derniere(racine).quand});
    }
    Ok(v)
}

/// La preuve d'une suppression reelle : le mot de passe du logiciel, verifie sur le coffre de la compagnie. Vide : le
/// moteur le demande lui-meme (`demande::mot_de_passe_de_l_acte`). Un coffre sans mot de passe du logiciel ne prouve rien.
fn prouver(racine: &Path, nom: &str, motdepasse: Option<&str>, poste: &Poste) -> Result<(), String> {
    let coffre = poste.coffre(racine);
    if !coffre.ouverture_exigee().map_err(|e| e.message)? {
        return Err("Aucun mot de passe du logiciel n'est posé pour cette compagnie : rien ne prouve qui la supprime. Rien n'a été supprimé.".into());
    }
    let tape = match motdepasse.filter(|m| !m.is_empty()) {
        Some(m) => m.to_string(),
        None => {
            let acte = crate::demande::Acte::nouveau(format!("supprimer définitivement la compagnie {}", nom), "", "");
            crate::demande::mot_de_passe_de_l_acte(racine, &acte).map_err(|m| format!("{} Rien n'a été supprimé.", m))?
        }
    };
    if !coffre.verifier_ouverture(&tape).map_err(|e| e.message)? {
        return Err("Mot de passe du logiciel refusé. Rien n'a été supprimé.".into());
    }
    Ok(())
}

/// SUPPRIMER `racine`, compagnie de l'installation `inst` : la preuve d'abord (une compagnie reelle), puis `avant` (la
/// fenetre lache ce qu'elle tient de la compagnie), puis l'effacement (`effacer`). Rend le nom de la compagnie. Un refus
/// n'efface rien et n'appelle pas `avant`.
pub fn supprimer(inst: &Path, racine: &Path, motdepasse: Option<&str>, poste: &Poste, avant: &dyn Fn(&Path)) -> Result<String, String> {
    let g = genre(inst, racine)?;
    let nom = compagnies::nom_de(racine);
    if g == Genre::Reelle {
        prouver(racine, &nom, motdepasse, poste)?;
    }
    avant(racine);
    let _ = poste.coffre(racine).oublier_ouverture_gardee();
    crate::administrateur::oublier(inst, racine)?;
    effacer(racine)?;
    Ok(nom)
}

/// Le dossier `racine` renomme en dossier cache a cote, puis efface ; ce qui resiste reste cache, et s'efface au lancement
/// suivant (`effacer_les_restes`). Echoue seulement si le renommage echoue : alors rien n'a bouge.
fn effacer(racine: &Path) -> Result<(), String> {
    let parent = racine.parent().ok_or("Un dossier sans parent ne se supprime pas.")?;
    let nom = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let instant = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let cache = parent.join(format!("{}{}-{}", PREFIXE_RESTE, nom, instant));
    std::fs::rename(racine, &cache).map_err(|e| format!("{} ne se supprime pas ({}) : fermez ce qui l'utilise, puis recommencez. Rien n'a été supprimé.", racine.display(), e))?;
    let _ = std::fs::remove_dir_all(&cache);
    Ok(())
}

/// AU LANCEMENT : les restes d'une suppression qui ne s'etaient pas effaces. Rend ceux qui sont partis.
pub fn effacer_les_restes(inst: &Path) -> Vec<PathBuf> {
    let mut partis = Vec::new();
    for d in [inst.to_path_buf(), inst.join("compagnies"), inst.join(compagnies::DOSSIER_FICTIVES)] {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().starts_with(PREFIXE_RESTE) && std::fs::remove_dir_all(e.path()).is_ok() {
                partis.push(e.path());
            }
        }
    }
    partis
}

// ------------------------------------------------------------------ au deblocage

/// LES FICTIVES AJOUTEES A GARDER OU NON : sur un logiciel debloque, celles qui sont encore la ; bride, aucune (la question
/// ne se pose pas).
pub fn a_garder(inst: &Path) -> Vec<Compagnie> {
    if crate::bridage::est_bride_de(inst) {
        return Vec::new();
    }
    compagnies::liste_de(inst).into_iter().filter(|c| !c.reelle && compagnies::est_ajoutee(&c.racine)).collect()
}

/// GARDER la fictive ajoutee `racine` : elle devient une compagnie reelle, sous `compagnies/` (un nom de dossier tire du
/// sien, jamais celui d'une compagnie deja la), sans marqueur de fictive, avec sa mention de demo. Rend la compagnie.
fn garder(inst: &Path, racine: &Path) -> Result<Compagnie, String> {
    let nom = compagnies::nom_de(racine);
    let marqueur = racine.join(compagnies::MARQUEUR);
    let creee_le = std::fs::read_to_string(&marqueur)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("quand").and_then(Value::as_str).map(String::from))
        .unwrap_or_default();
    let parent = inst.join("compagnies");
    std::fs::create_dir_all(&parent).map_err(|e| format!("{} : {}", parent.display(), e))?;
    let base = compagnies::nom_de_dossier(&nom);
    let mut cible = parent.join(&base);
    let mut n = 2;
    while cible.exists() {
        cible = parent.join(format!("{}-{}", base, n));
        n += 1;
    }
    let mention = json!({
        "creee_le": creee_le,
        "gardee_le": crate::incidents::maintenant(),
        "mois": crate::routes_mois::mois_saisis(racine),
        "_quoi": "Cette compagnie a ete creee dans la version demo de PuppetPaie, puis gardee au deblocage : rien de ce qui y a ete fait avant n'est parti (ni DPAE, ni contrat a la signature, ni DSN, ni document).",
    });
    crate::ecriture::ecrire_json(&racine.join(FICHIER_DEMO), &mention, 2)?;
    if let Err(e) = std::fs::rename(racine, &cible) {
        let _ = std::fs::remove_file(racine.join(FICHIER_DEMO));
        return Err(format!("{} ne se garde pas ({}) : rien n'a changé.", nom, e));
    }
    // Sans marqueur : une compagnie reelle. S'il ne s'efface pas, elle resterait une fictive ajoutee hors de `fictives/` :
    // le dire, plutot que la laisser envoyer sans le savoir... ou ne jamais envoyer.
    std::fs::remove_file(cible.join(compagnies::MARQUEUR)).map_err(|e| format!("{} : le marqueur de fictive ne s'efface pas ({}).", cible.display(), e))?;
    compagnies::liste_de(inst)
        .into_iter()
        .find(|c| absolu(&c.racine) == absolu(&cible))
        .ok_or_else(|| format!("{} a été gardée, mais l'accueil ne la trouve pas ({}).", nom, cible.display()))
}

/// LA REPONSE A LA QUESTION DU DEBLOCAGE : `garder` et `ne_pas_garder`, des identifiants de tuile (`fictive:<dossier>`)
/// parmi `a_garder`. Chacune est gardee ou supprimee ; une fictive ajoutee nommee nulle part reste, et la question
/// reviendra pour elle. `lacher` : ce que la fenetre tient de chacune (sa file sur le fil des calculs), avant qu'elle
/// bouge. Rend `{"gardees", "supprimees", "erreurs"}` : un echec sur l'une n'arrete pas les autres.
pub fn repondre(inst: &Path, garder_ids: &[String], ne_pas_garder: &[String], poste: &Poste, lacher: &dyn Fn(&Path)) -> Result<Value, String> {
    if crate::bridage::est_bride_de(inst) {
        return Err("Le logiciel est bridé : ses compagnies ajoutées restent des compagnies fictives.".into());
    }
    if let Some(id) = garder_ids.iter().find(|id| ne_pas_garder.contains(id)) {
        return Err(format!("{} est à la fois gardée et supprimée : rien n'a changé.", id));
    }
    let proposees = a_garder(inst);
    let inconnue = garder_ids.iter().chain(ne_pas_garder).find(|id| !proposees.iter().any(|c| &c.id == *id));
    if let Some(id) = inconnue {
        return Err(format!("{} n'est pas une compagnie créée en démo : rien n'a changé.", id));
    }
    let (mut gardees, mut supprimees, mut erreurs) = (Vec::new(), Vec::new(), Vec::new());
    for c in proposees {
        if garder_ids.contains(&c.id) {
            lacher(&c.racine);
            match garder(inst, &c.racine) {
                Ok(r) => gardees.push(json!({"id": r.id, "nom": r.nom})),
                Err(m) => erreurs.push(m),
            }
        } else if ne_pas_garder.contains(&c.id) {
            match supprimer(inst, &c.racine, None, poste, lacher) {
                Ok(nom) => supprimees.push(json!(nom)),
                Err(m) => erreurs.push(m),
            }
        }
    }
    Ok(json!({"gardees": gardees, "supprimees": supprimees, "erreurs": erreurs}))
}

// --------------------------------------------------------- la mention de demo

/// LA MENTION DE DEMO d'une compagnie gardee (`config/demo.json`), telle que la page la montre : quand elle a ete creee
/// et gardee, les mois saisis en demo, et la phrase. `None` : une compagnie qui n'a pas ete creee en demo.
pub fn mention(racine: &Path) -> Option<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(racine.join(FICHIER_DEMO)).ok()?).ok()?;
    let jour = |cle: &str| {
        let t = v.get(cle).and_then(Value::as_str).unwrap_or("");
        crate::etats::quand_lisible(&json!(t.chars().take(10).collect::<String>()))
    };
    let (creee, gardee) = (jour("creee_le"), jour("gardee_le"));
    let depuis = if creee.is_empty() { String::new() } else { format!(" le {}", creee) };
    Some(json!({
        "creee_le": v.get("creee_le").cloned().unwrap_or(Value::Null),
        "gardee_le": v.get("gardee_le").cloned().unwrap_or(Value::Null),
        "mois": v.get("mois").cloned().unwrap_or_else(|| json!([])),
        "phrase": format!(
            "Compagnie créée en démo{}, gardée le {}. Les paies faites en démo n'ont été ni déclarées ni envoyées par PuppetPaie : aucune DPAE, aucun contrat envoyé à la signature, aucune DSN déposée, aucun document envoyé.",
            depuis, gardee
        ),
    }))
}
