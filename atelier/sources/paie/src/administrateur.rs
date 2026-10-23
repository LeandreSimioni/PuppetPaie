//! UN LOGICIEL = UN ADMINISTRATEUR (ticket #244 du depot prive ; Leandre, 22/10/2026). Une installation de PuppetPaie
//! (le dossier qui porte `logiciel/`, et a cote `prive/`, `compagnies/*`, `fictives/*` : `interface/compagnies.rs`)
//! appartient a une personne qui gere une ou plusieurs compagnies ; qui veut deux administrateurs installe un second
//! logiciel. Les compagnies fictives restent hors de tout cela.
//!
//! CE QUI EST SCELLE PAR QUOI. Rien de neuf : chaque compagnie garde SON coffre (`config/coffre.bin`, `coffre.rs`), sa
//! cle maitresse scellee deux fois, par la cle du poste et par le mot de passe du logiciel. RATTACHER une compagnie, c'est
//! sceller son enveloppe « mot de passe » par le mot de passe de l'administrateur (`Coffre::preparer_ouverture`) ; le
//! detacher, la resceller par un mot de passe a elle. Chaque coffre reste propre a sa compagnie : sa cle maitresse ne
//! sert qu'a lui ; seul le mot de passe qui en ouvre l'enveloppe est le meme.
//!
//! LE REGISTRE : `administrateur.json`, a la racine de l'installation (a cote de `compagnies/` : il voyage avec elle, une
//! cle USB porte tout). AUCUN SECRET : la liste des dossiers rattaches, relatifs a l'installation, et depuis quand. Il dit
//! quelles compagnies s'ouvrent ensemble ; ce qui le prouve, c'est que le mot de passe ouvre chacun des coffres
//! (`verifier_ouverture`), verifie a chaque geste.
//!
//! LES GESTES. `rattacher` (le mot de passe de l'administrateur, verifie sur une compagnie deja rattachee -- ou, quand il n'y
//! a pas encore d'administrateur, sur les compagnies deja la : toutes celles qu'il ouvre deviennent les siennes),
//! `detacher` (un mot de passe propre, tape deux fois), `changer` (le mot de passe de l'administrateur rescelle toutes ses
//! compagnies : tout se prepare, puis tout se remplace ; un echec au milieu remet en etat, aucune ne change). Rien de cela
//! n'est un acte qui engage : un acte reel (depot, envoi, signature) redemande toujours le mot de passe au moment de l'acte.
//!
//! LE TABLEAU DE BORD (Leandre, 22/10/2026) : pour chaque compagnie rattachee, les declarations obligatoires, triees par
//! urgence avec leur date limite -- la DSN mensuelle, les signalements (arret, reprise anticipee, fin de contrat) et ce que
//! `obligations.rs` fait naitre, les DPAE. Tout vient des RAPPELS de la compagnie (`routes_ouverture`, route `rappels` :
//! les mois a declarer et leur echeance, `obligations::de_l_arret` et `de_la_fin` pour les delais et leur etat) : ce module
//! n'en recalcule aucun. Il y ajoute, pour ce qui part par la DSN, ou en est le fichier ecrit (a tester, en test, teste,
//! refuse) : la meme lecture du journal que l'etape de depot du mois (`etats::repetition`, `etats::deja_depose`). Le
//! tableau d'une compagnie se garde dans `config/.tableau.json` : l'accueil le lit tel quel, sans calcul
//! (`tableau_garde`) ; il se refait par la route `administrateur-tableau`, que la fenetre lance en fond.
//!
//! LE DEPOT GROUPE (`deposer`, route `administrateur-deposer`) : ce qui part par la DSN et qui est PRET (le moteur le
//! decide ici : en test, un fichier ecrit qui n'a pas encore ete essaye ; en reel, un fichier dont le test est revenu
//! conforme) se depose compagnie par compagnie, par la route `deposer` de chacune -- la meme que la page d'une compagnie,
//! avec tous ses controles et son bilan. Une compagnie qui refuse n'arrete pas les autres. En reel, UN mot de passe tape
//! au moment de l'acte (dans la charge, par la fenetre ; sinon le moteur le demande lui-meme a un humain,
//! `demande::mot_de_passe_de_l_acte`) sert de preuve a chaque depot, qui le reverifie sur son propre coffre.
//! Ce qui est hors DSN (la declaration d'accident, l'attestation de salaire) s'affiche avec son echeance et la marche a
//! suivre : rien ne le depose.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::coffre::Coffre;
use crate::contexte::Contexte;
use crate::etats::{self, erreur, est, py_str, R};
use crate::fichiers::vrai;
use crate::interface::compagnies::{self, Compagnie};
use crate::retours::Echec;

/// Le registre de l'administrateur, a la racine de l'installation.
pub const FICHIER: &str = "administrateur.json";
/// Le dernier tableau connu d'une compagnie (hors de l'empreinte des etats des mois : `etat_garde::HORS_DE_L_ECRAN`).
pub const TABLEAU: &str = "config/.tableau.json";

/// Les routes de l'administrateur (servies par `routes_ecritures_g3`).
pub const ROUTES: [&str; 2] = ["administrateur-tableau", "administrateur-deposer"];

// ------------------------------------------------------------------ le poste

/// Ou est la cle du poste, et quelle heure il est : ce qu'il faut pour monter le coffre d'une compagnie.
#[derive(Clone, Debug)]
pub struct Poste {
    pub dossier_cle: PathBuf,
    /// `AAAA-MM-JJTHH:MM` : le coffre note quand un mot de passe est pose.
    pub maintenant: String,
}

impl Poste {
    /// Ce poste-ci : la cle la ou `Coffre::du_poste` la trouve, l'heure de Paris.
    pub fn de_ce_poste() -> Poste {
        Poste { dossier_cle: Coffre::dossier_cle_du_poste(), maintenant: crate::commandes::actes::maintenant_minutes() }
    }

    pub fn coffre(&self, racine: &Path) -> Coffre {
        Coffre::sur(racine, &self.dossier_cle, &self.maintenant)
    }
}

// ------------------------------------------------------------------ le registre

/// L'installation d'une compagnie : `<installation>/compagnies/<nom>` ou `<installation>/prive` (ou `fictives/<nom>`).
pub fn installation_de(racine: &Path) -> PathBuf {
    let parent = racine.parent().unwrap_or(racine);
    if parent.file_name().is_some_and(|n| n == "compagnies" || n == compagnies::DOSSIER_FICTIVES) {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

fn absolu(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let t = c.to_string_lossy();
    t.strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(c)
}

/// Le dossier d'une compagnie, relatif a l'installation, en barres droites (`compagnies/les-pupazzi`, `prive`).
fn relatif(inst: &Path, racine: &Path) -> Option<String> {
    let r = absolu(racine);
    let i = absolu(inst);
    let rel = r.strip_prefix(&i).ok()?;
    let t = rel.to_string_lossy().replace('\\', "/");
    (!t.is_empty()).then_some(t)
}

fn lire_registre(inst: &Path) -> Vec<Value> {
    std::fs::read_to_string(inst.join(FICHIER))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("compagnies").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn ecrire_registre(inst: &Path, entrees: &[Value]) -> Result<(), String> {
    let p = inst.join(FICHIER);
    if entrees.is_empty() {
        return match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{} : {}", p.display(), e)),
        };
    }
    let v = json!({
        "_lisez_moi": "Les compagnies que gere l'administrateur de ce logiciel : un seul mot de passe les ouvre toutes. Aucun secret ici.",
        "compagnies": entrees,
    });
    crate::ecriture::ecrire_json(&p, &v, 2)
}

/// Les compagnies rattachees a l'administrateur, telles qu'elles sont sur le disque (un dossier parti n'y est plus). Une
/// compagnie fictive n'y entre pas : `rattacher` la refuse.
pub fn rattachees(inst: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in lire_registre(inst) {
        let Some(d) = e.get("dossier").and_then(Value::as_str) else { continue };
        let racine = inst.join(d);
        if racine.join("config").join("entreprise.json").is_file() {
            out.push(racine);
        }
    }
    out
}

/// UNE COMPAGNIE SUPPRIMEE (`interface::suppression`, ticket #236) sort du registre : une compagnie creee plus tard sous
/// le meme dossier n'en herite pas. Rien a faire si elle n'y est pas.
pub fn oublier(inst: &Path, racine: &Path) -> Result<(), String> {
    let Some(rel) = relatif(inst, racine) else { return Ok(()) };
    let avant = lire_registre(inst);
    let apres: Vec<Value> = avant.iter().filter(|e| e.get("dossier").and_then(Value::as_str) != Some(rel.as_str())).cloned().collect();
    if apres.len() == avant.len() {
        return Ok(());
    }
    ecrire_registre(inst, &apres)
}

/// La compagnie `racine` est-elle rattachee a l'administrateur de son installation ?
pub fn est_rattachee(inst: &Path, racine: &Path) -> bool {
    let Some(rel) = relatif(inst, racine) else { return false };
    rattachees(inst).iter().any(|r| relatif(inst, r).as_deref() == Some(rel.as_str()))
}

fn sans(liste: Vec<PathBuf>, inst: &Path, racine: &Path) -> Vec<PathBuf> {
    let rel = relatif(inst, racine);
    liste.into_iter().filter(|r| relatif(inst, r) != rel).collect()
}

/// Les compagnies reelles de l'installation (ni fictive, ni copie), avec leur fiche d'accueil.
fn reelles(inst: &Path) -> Vec<Compagnie> {
    compagnies::liste_de(inst).into_iter().filter(|c| c.reelle && !compagnies::est_fictive(&c.racine)).collect()
}

fn nom_de(inst: &Path, racine: &Path) -> String {
    let rel = relatif(inst, racine);
    compagnies::liste_de(inst)
        .into_iter()
        .find(|c| relatif(inst, &c.racine) == rel)
        .map(|c| c.nom)
        .unwrap_or_else(|| racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

fn en_liste(noms: &[String]) -> String {
    match noms.len() {
        0 => String::new(),
        1 => noms[0].clone(),
        n => format!("{} et {}", noms[..n - 1].join(", "), noms[n - 1]),
    }
}

fn exigee(poste: &Poste, racine: &Path) -> bool {
    poste.coffre(racine).ouverture_exigee().unwrap_or(false)
}

fn ouvre(poste: &Poste, racine: &Path, motdepasse: &str) -> bool {
    !motdepasse.is_empty() && poste.coffre(racine).verifier_ouverture(motdepasse).unwrap_or(false)
}

/// Le mot de passe ouvre-t-il le coffre de cette compagnie ?
pub fn mot_de_passe_ouvre(poste: &Poste, racine: &Path, motdepasse: &str) -> bool {
    ouvre(poste, racine, motdepasse)
}

/// L'administrateur de l'installation, tel que l'ecran le dit : les compagnies qu'il gere, et ce que la compagnie
/// `racine` est pour lui. `question` : les compagnies dont le mot de passe ouvrirait aussi `racine`, quand il y a lieu de
/// demander « meme administrateur ? » (une compagnie reelle sans mot de passe, et au moins une autre qui en a un).
pub fn etat(inst: &Path, racine: Option<&Path>, poste: &Poste) -> Value {
    let groupe = rattachees(inst);
    let noms: Vec<String> = groupe.iter().map(|r| nom_de(inst, r)).collect();
    let mut v = json!({"compagnies": noms, "administrateur": !groupe.is_empty()});
    let Some(racine) = racine else { return v };
    let fictive = compagnies::est_fictive(racine);
    let rattachee = !fictive && est_rattachee(inst, racine);
    v["rattachee"] = json!(rattachee);
    v["fictive"] = json!(fictive);
    // Les autres compagnies que le mot de passe de l'administrateur ouvre : celles du registre, sinon celles qui ont deja
    // un mot de passe (il n'y a pas encore d'administrateur : l'une d'elles le devient).
    let autres: Vec<String> = if !groupe.is_empty() {
        sans(groupe, inst, racine).iter().map(|r| nom_de(inst, r)).collect()
    } else {
        reelles(inst).into_iter().filter(|c| relatif(inst, &c.racine) != relatif(inst, racine) && exigee(poste, &c.racine)).map(|c| c.nom).collect()
    };
    v["autres"] = json!(autres);
    v["peut_rattacher"] = json!(!fictive && !rattachee && !autres.is_empty() && !crate::bridage::est_bride_de(inst));
    v["question"] = json!(!fictive && !rattachee && !autres.is_empty() && !exigee(poste, racine) && !crate::bridage::est_bride_de(inst));
    v["mot_de_passe_pose"] = json!(exigee(poste, racine));
    v
}

/// Remplace `chemin` par `octets`, d'un coup (a cote puis a la place) ; 0600 sous Unix.
fn remplacer(chemin: &Path, octets: &[u8]) -> Result<(), String> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{} : {}", d.display(), e))?;
    }
    let tmp = chemin.with_extension("bin.administrateur");
    std::fs::write(&tmp, octets).map_err(|e| format!("{} : {}", tmp.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, chemin).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} : {}", chemin.display(), e)
    })
}

/// Remet un fichier dans l'etat d'avant (`None` : il n'existait pas).
fn remettre(chemin: &Path, avant: &Option<Vec<u8>>) {
    match avant {
        Some(o) => {
            let _ = remplacer(chemin, o);
        }
        None => {
            let _ = std::fs::remove_file(chemin);
        }
    }
}

fn entree_registre(inst: &Path, racine: &Path, poste: &Poste) -> Option<Value> {
    Some(json!({"dossier": relatif(inst, racine)?, "depuis": poste.maintenant}))
}

/// RATTACHER `racine` a l'administrateur : son coffre se rescelle par `admin` (le mot de passe de l'administrateur). `admin`
/// est verifie d'abord sur une compagnie deja rattachee ; sans administrateur encore, il doit ouvrir au moins une des
/// compagnies qui ont deja un mot de passe, et toutes celles qu'il ouvre deviennent les siennes. Une compagnie qui a deja
/// son mot de passe donne aussi `actuel` (c'est lui que le rattachement remplace). Un refus n'ecrit rien. Rend les noms des
/// compagnies de l'administrateur.
pub fn rattacher(inst: &Path, racine: &Path, admin: &str, actuel: Option<&str>, poste: &Poste) -> Result<Vec<String>, String> {
    if crate::bridage::est_bride_de(inst) {
        return Err("Le logiciel est bridé : il ne gère que des compagnies fictives.".into());
    }
    if compagnies::est_fictive(racine) {
        return Err("Une compagnie fictive s'ouvre sans mot de passe : elle n'a pas d'administrateur.".into());
    }
    let Some(rel) = relatif(inst, racine) else {
        return Err(format!("{} n'est pas une compagnie de ce logiciel ({}).", racine.display(), inst.display()));
    };
    if est_rattachee(inst, racine) {
        return Err("Cette compagnie est déjà gérée par l'administrateur.".into());
    }
    if admin.is_empty() {
        return Err("Tapez le mot de passe de l'administrateur. Rien n'a changé.".into());
    }
    let groupe = rattachees(inst);
    let mut nouvelles: Vec<PathBuf> = Vec::new();
    if groupe.is_empty() {
        let candidates: Vec<Compagnie> = reelles(inst).into_iter().filter(|c| relatif(inst, &c.racine).as_deref() != Some(rel.as_str()) && exigee(poste, &c.racine)).collect();
        if candidates.is_empty() {
            return Err("Aucune autre compagnie n'a encore de mot de passe : choisissez celui de cette compagnie.".into());
        }
        nouvelles = candidates.iter().filter(|c| ouvre(poste, &c.racine, admin)).map(|c| c.racine.clone()).collect();
        if nouvelles.is_empty() {
            let noms: Vec<String> = candidates.iter().map(|c| c.nom.clone()).collect();
            return Err(format!("Ce mot de passe n'ouvre pas {} : ce n'est pas celui de l'administrateur. Rien n'a changé.", en_liste(&noms)));
        }
    } else if !groupe.iter().any(|r| ouvre(poste, r, admin)) {
        return Err("Ce n'est pas le mot de passe de l'administrateur. Rien n'a changé.".into());
    }
    let coffre = poste.coffre(racine);
    let deja = coffre.ouverture_exigee().map_err(|e| e.message)?;
    let actuel = actuel.filter(|a| !a.is_empty());
    if deja {
        match actuel {
            None => return Err("Tapez aussi le mot de passe actuel de cette compagnie : c'est lui que celui de l'administrateur remplace. Rien n'a changé.".into()),
            Some(a) if !ouvre(poste, racine, a) => return Err("Le mot de passe actuel de cette compagnie est refusé. Rien n'a changé.".into()),
            _ => {}
        }
    }
    let octets = coffre.preparer_ouverture(admin, actuel).map_err(|e| format!("{} Rien n'a changé.", e.message.trim_end_matches(" Rien n'a ete pose.")))?;
    let chemin = coffre.chemin();
    let avant = std::fs::read(&chemin).ok();
    remplacer(&chemin, &octets).map_err(|m| format!("Le coffre de cette compagnie ne s'écrit pas ({}). Rien n'a changé.", m))?;
    let mut entrees = lire_registre(inst);
    for r in nouvelles.iter().chain(std::iter::once(&racine.to_path_buf())) {
        if let Some(e) = entree_registre(inst, r, poste) {
            if !entrees.iter().any(|x| x.get("dossier") == e.get("dossier")) {
                entrees.push(e);
            }
        }
    }
    if let Err(m) = ecrire_registre(inst, &entrees) {
        remettre(&chemin, &avant);
        return Err(format!("Le registre de l'administrateur ne s'écrit pas ({}). Rien n'a changé.", m));
    }
    Ok(rattachees(inst).iter().map(|r| nom_de(inst, r)).collect())
}

/// DETACHER `racine` : elle recoit son propre mot de passe (`nouveau`, tape deux fois), et celui de l'administrateur
/// (`admin`, verifie) ne l'ouvre plus. Un refus n'ecrit rien.
pub fn detacher(inst: &Path, racine: &Path, admin: &str, nouveau: &str, confirmation: &str, poste: &Poste) -> Result<(), String> {
    if !est_rattachee(inst, racine) {
        return Err("Cette compagnie n'est pas gérée par l'administrateur : elle a déjà son propre mot de passe.".into());
    }
    if !ouvre(poste, racine, admin) {
        return Err("Ce n'est pas le mot de passe de l'administrateur. Rien n'a changé.".into());
    }
    if nouveau != confirmation {
        return Err("Les deux saisies du nouveau mot de passe diffèrent. Rien n'a changé.".into());
    }
    if nouveau == admin {
        return Err("Choisissez un mot de passe différent de celui de l'administrateur : c'est lui qui la sépare des autres. Rien n'a changé.".into());
    }
    let coffre = poste.coffre(racine);
    let octets = coffre.preparer_ouverture(nouveau, Some(admin)).map_err(|e| format!("{} Rien n'a changé.", e.message.trim_end_matches(" Rien n'a ete pose.")))?;
    let rel = relatif(inst, racine);
    let avant_registre = lire_registre(inst);
    let reste: Vec<Value> = avant_registre.iter().filter(|e| e.get("dossier").and_then(Value::as_str) != rel.as_deref()).cloned().collect();
    ecrire_registre(inst, &reste).map_err(|m| format!("Le registre de l'administrateur ne s'écrit pas ({}). Rien n'a changé.", m))?;
    if let Err(m) = remplacer(&coffre.chemin(), &octets) {
        let _ = ecrire_registre(inst, &avant_registre);
        return Err(format!("Le coffre de cette compagnie ne s'écrit pas ({}). Rien n'a changé.", m));
    }
    Ok(())
}

/// CHANGER le mot de passe de l'administrateur : toutes ses compagnies se rescellent d'un coup. `actuel` doit ouvrir
/// chacune ; tout se prepare d'abord (si une seule refuse, rien ne s'ecrit) ; puis chaque coffre est remplace, et si un
/// remplacement echoue, ceux deja faits reviennent a leur etat d'avant. Rend les noms des compagnies rescellees.
pub fn changer(inst: &Path, actuel: &str, nouveau: &str, confirmation: &str, poste: &Poste) -> Result<Vec<String>, String> {
    let groupe = rattachees(inst);
    if groupe.is_empty() {
        return Err("Ce logiciel n'a pas d'administrateur : chaque compagnie a son propre mot de passe.".into());
    }
    if nouveau != confirmation {
        return Err("Les deux saisies du nouveau mot de passe diffèrent. Rien n'a changé.".into());
    }
    let mut prets: Vec<(PathBuf, Vec<u8>, Option<Vec<u8>>)> = Vec::new();
    for r in &groupe {
        let nom = nom_de(inst, r);
        if !ouvre(poste, r, actuel) {
            return Err(format!("Le mot de passe actuel n'ouvre pas {} : rien n'a changé, dans aucune compagnie.", nom));
        }
        let coffre = poste.coffre(r);
        let octets = coffre
            .preparer_ouverture(nouveau, Some(actuel))
            .map_err(|e| format!("{} : {} Rien n'a changé, dans aucune compagnie.", nom, e.message.trim_end_matches(" Rien n'a ete pose.")))?;
        let chemin = coffre.chemin();
        let avant = std::fs::read(&chemin).ok();
        prets.push((chemin, octets, avant));
    }
    let mut faits: Vec<usize> = Vec::new();
    for (i, (chemin, octets, _)) in prets.iter().enumerate() {
        if let Err(m) = remplacer(chemin, octets) {
            for j in faits {
                remettre(&prets[j].0, &prets[j].2);
            }
            return Err(format!("Un coffre ne s'écrit pas ({}) : tous sont revenus à l'ancien mot de passe.", m));
        }
        faits.push(i);
    }
    Ok(groupe.iter().map(|r| nom_de(inst, r)).collect())
}

// ------------------------------------------------------------------ le tableau d'une compagnie

fn jj_mm_aaaa(iso: &str) -> String {
    match (iso.get(..4), iso.get(5..7), iso.get(8..10)) {
        (Some(a), Some(m), Some(j)) => format!("{}/{}/{}", j, m, a),
        _ => iso.to_string(),
    }
}

/// Ou en est un fichier DSN ecrit (mensuelle ou signalement), lu du journal : `deposee` (un depot reel accepte de ce
/// contenu), `paie_changee` (une mensuelle qui ne declare plus la paie), `testee` (test conforme, bilan lu), `en_test`
/// (test parti, bilan attendu), `test_refuse`, `a_tester`. Les memes lectures que l'etape de depot du mois
/// (`routes_mois::etape_de_depot`).
fn etape_du_fichier(o: &HashMap<String, String>, ctx: &Contexte, lignes: &[Value], chemin: &Path, mensuelle: Option<&str>) -> R<&'static str> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let empreinte = crate::journal::empreinte(&octets);
    if vrai(&etats::deja_depose(lignes, "dsn", &empreinte)?) {
        return Ok("deposee");
    }
    if let Some(m) = mensuelle {
        if crate::routes_mois::Moteur::nouveau(o, ctx).paie_changee_depuis_l_ecriture(m, chemin)?.is_some() {
            return Ok("paie_changee");
        }
    }
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let r = etats::repetition(lignes, "dsn", &empreinte, &nom)?;
    Ok(if vrai(&r["conforme"]) {
        if est(&r["compte_rendu"], "ok") {
            "testee"
        } else {
            "en_test"
        }
    } else if vrai(&r["ratees"]) {
        "test_refuse"
    } else {
        "a_tester"
    })
}

/// Ce qu'on dit d'un fichier selon son etape.
fn marche_du_fichier(etape: &str) -> &'static str {
    match etape {
        "a_tester" => "Écrite : prête pour le dépôt de test.",
        "en_test" => "Partie en test, compte rendu attendu : le dépôt réel se fait une fois le test revenu conforme.",
        "testee" => "Testée, conforme : prête à envoyer pour de vrai.",
        "test_refuse" => "Le test a été refusé : à corriger dans la compagnie, puis à retester.",
        "paie_changee" => "La paie a changé depuis l'écriture : réécrire le mois, le tester, puis l'envoyer.",
        _ => "Déposée.",
    }
}

/// Le fichier d'un signalement ecrit, s'il y en a un : `sorties/<mois>/<nom>` (le nom compare sans casse).
fn fichier_ecrit(racine: &Path, mois: &str, nom: &str) -> Option<PathBuf> {
    let dossier = racine.join("sorties").join(mois);
    std::fs::read_dir(&dossier).ok()?.flatten().map(|e| e.path()).find(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(nom)))
}

/// Le fichier que l'ecriture d'un signalement pose pour cette obligation (`routes_actes_h1` : `arret-signaler`,
/// `fin-contrat-signaler`), d'apres la cle du rappel (`<obligation>:<salarie>:<jour>`, `fin-contrat:<salarie>:<fin>`).
fn fichier_du_signalement(ctx: &Contexte, rappel: &Value) -> Option<PathBuf> {
    let cle = rappel.get("cle").and_then(Value::as_str)?;
    let mut morceaux = cle.splitn(3, ':');
    let (genre, salarie, jour) = (morceaux.next()?, morceaux.next()?, morceaux.next()?);
    let mois = jour.get(..7)?;
    let compact = jour.replace('-', "");
    let nom = match genre {
        "signalement_arret" | "signalement_arret_rectificatif" => format!("SignalementArret_{}_{}.dsn", salarie, compact),
        "signalement_reprise" => format!("SignalementReprise_{}_{}.dsn", salarie, compact),
        "fin-contrat" => {
            let fiche = ctx.salarie(salarie).ok()?;
            format!("SignalementFinContrat_{}_{}.dsn", py_str(fiche.get("_cle")?), compact)
        }
        _ => return None,
    };
    fichier_ecrit(&ctx.racine, mois, &nom)
}

/// Une declaration du tableau, a partir d'un rappel de la compagnie.
fn declaration(o: &HashMap<String, String>, ctx: &Contexte, lignes: &[Value], r: &Value) -> R<Option<Value>> {
    let texte = |k: &str| r.get(k).filter(|v| vrai(v)).map(py_str).unwrap_or_default();
    let quoi = texte("quoi");
    let mut d = Map::new();
    d.insert("cle".into(), json!(texte("cle")));
    d.insert("en_retard".into(), json!(r.get("en_retard").is_some_and(vrai)));
    let (mut fichier, mut mensuelle): (Option<PathBuf>, Option<String>) = (None, None);
    match quoi.as_str() {
        "dsn" => {
            let mois = texte("mois");
            let e = r.get("echeance_depot").cloned().unwrap_or(Value::Null);
            d.insert("genre".into(), json!("dsn"));
            d.insert("titre".into(), json!(format!("DSN de {}", crate::date::libelle_du_mois_ou_tel_quel(&mois, crate::date::Graphie::Accentuee))));
            d.insert("mois".into(), json!(mois));
            d.insert("butoir".into(), e.get("jour").cloned().unwrap_or(Value::Null));
            d.insert("quand".into(), e.get("phrase").cloned().unwrap_or(Value::Null));
            d.insert("detail".into(), json!(texte("detail")));
            let etat = texte("etat");
            let marche = match etat.as_str() {
                "a_saisir" => "Saisir le mois dans la compagnie (vide s'il n'y a pas eu de paie : une DSN « néant » se dépose aussi).",
                "a_produire" => "Écrire le mois dans la compagnie : la DSN s'écrit avec les bulletins, puis se teste.",
                "a_corriger" => "Corriger dans la compagnie, réécrire le mois, puis retester.",
                _ => "",
            };
            d.insert("marche".into(), json!(marche));
            if etat == "a_deposer" {
                fichier = r.get("fichier").and_then(Value::as_str).map(PathBuf::from);
                mensuelle = Some(mois);
            }
        }
        "obligation" => {
            let ob = r.get("obligation").cloned().unwrap_or(Value::Null);
            let cle = py_str(ob.get("cle").unwrap_or(&Value::Null));
            let butoir = ob.get("butoir").filter(|b| vrai(b)).map(py_str);
            let heure = ob.get("butoir_heure").filter(|b| vrai(b)).map(py_str);
            d.insert("titre".into(), json!(texte("titre")));
            d.insert("butoir".into(), json!(butoir));
            d.insert(
                "quand".into(),
                json!(butoir.as_ref().map(|b| match &heure {
                    Some(h) if h == "24:00" => format!("le {}, jusqu'à minuit", jj_mm_aaaa(b)),
                    Some(h) => format!("le {} à {}", jj_mm_aaaa(b), h),
                    None => format!("le {}", jj_mm_aaaa(b)),
                })),
            );
            d.insert("detail".into(), json!(ob.get("phrase").filter(|p| vrai(p)).map(py_str).unwrap_or_else(|| texte("detail"))));
            if cle.starts_with("signalement_") {
                d.insert("genre".into(), json!("signalement"));
                fichier = fichier_du_signalement(ctx, r);
                let a_saisir = r.get("a_saisir").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                d.insert(
                    "marche".into(),
                    json!(if a_saisir {
                        "Compléter le contrat dans la compagnie, puis écrire le signalement."
                    } else {
                        "Écrire le signalement dans la compagnie (Imprévus, ou la colonne du mois), puis le tester."
                    }),
                );
            } else {
                // HORS DSN : la declaration d'accident, l'attestation de salaire. Le logiciel ne la fait pas : ou la faire.
                d.insert("genre".into(), json!("hors_dsn"));
                let ou: Vec<Value> = ob.get("ou").and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.get("libelle").cloned()).collect()).unwrap_or_default();
                d.insert("ou".into(), Value::Array(ou));
                d.insert("marche".into(), json!("Elle ne part pas avec la DSN : à faire sur le service de l'organisme, puis à dire faite dans la compagnie."));
            }
        }
        "dpae" => {
            d.insert("genre".into(), json!("dpae"));
            d.insert("titre".into(), json!(texte("titre")));
            let jour = r.get("jour").filter(|j| vrai(j)).map(py_str);
            d.insert("quand".into(), json!(jour.as_ref().map(|j| format!("le {}", jj_mm_aaaa(j)))));
            d.insert("butoir".into(), json!(jour));
            d.insert("detail".into(), json!(texte("detail")));
            d.insert("marche".into(), json!("La DPAE se dépose dans la compagnie, avant la première heure de travail."));
        }
        _ => return Ok(None),
    }
    let mut pret_test = false;
    let mut pret_reel = false;
    if let Some(f) = fichier.filter(|f| f.is_file()) {
        let etape = etape_du_fichier(o, ctx, lignes, &f, mensuelle.as_deref())?;
        if etape == "deposee" {
            // Le depot reel de ce contenu est deja note : le rappel se taira au prochain releve.
            return Ok(None);
        }
        let reel_code = std::fs::read(&f).map(|o| crate::depot::lire(&o).reel()).unwrap_or(false);
        pret_test = etape == "a_tester";
        pret_reel = etape == "testee" && reel_code;
        d.insert("fichier".into(), json!(f.to_string_lossy()));
        d.insert("etape".into(), json!(etape));
        d.insert("marche".into(), json!(marche_du_fichier(etape)));
    }
    d.insert("pret".into(), json!({"test": pret_test, "reel": pret_reel}));
    Ok(Some(Value::Object(d)))
}

/// Le plus presse d'abord : en retard, puis par date limite ; sans date, a la fin.
fn cle_d_urgence(d: &Value) -> (bool, String) {
    (!d.get("en_retard").is_some_and(vrai), d.get("butoir").and_then(Value::as_str).unwrap_or("9999-99-99").to_string())
}

/// LE TABLEAU DE LA COMPAGNIE `ctx` : ses declarations obligatoires, du plus presse au moins presse, chacune avec sa date
/// limite et, pour ce qui part par la DSN, ou en est son fichier. Ecrit dans `config/.tableau.json` (ce que l'accueil lit).
pub fn tableau_compagnie(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    // L'EMPREINTE DE TOUTE LA COMPAGNIE, prise avant (celle de l'etat d'ouverture, `etat_garde::empreinte_de`) : le fil de
    // la fenetre ne le refait pas tant qu'elle n'a pas bouge (ticket #244, `FileDesEtats::faire_le_tableau`).
    let empreinte = crate::etat_garde::empreinte_de(&ctx.racine, crate::etat_garde::OUVERTURE, &ctx.aujourdhui.iso());
    let rappels = match crate::routes_ouverture::traiter(o, "rappels", &json!({})) {
        Some(Ok(v)) => v,
        Some(Err(m)) => {
            let (_, message) = crate::routes_ouverture::decoder(&m);
            return Err(erreur("ErreurOperation", message.to_string()));
        }
        None => Value::Null,
    };
    let lignes = etats::charger(&ctx.racine)?;
    let mut declarations = Vec::new();
    for r in rappels.get("rappels").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if let Some(d) = declaration(o, ctx, &lignes, r)? {
            declarations.push(d);
        }
    }
    declarations.sort_by_key(cle_d_urgence);
    let t = json!({"le": ctx.aujourdhui.iso(), "declarations": declarations, "empreinte": empreinte});
    let _ = crate::ecriture::ecrire_json(&ctx.racine.join(TABLEAU), &t, 2);
    Ok(t)
}

fn o_de(o: &HashMap<String, String>, racine: &Path) -> HashMap<String, String> {
    let mut c = o.clone();
    c.insert("racine".into(), racine.to_string_lossy().into_owned());
    c
}

/// Le tableau d'une compagnie, refait (sa propre compagnie de routes, sa racine en argument) ; son refus en phrase.
fn refaire_compagnie(o: &HashMap<String, String>, racine: &Path) -> Result<Value, String> {
    let oc = o_de(o, racine);
    let ctx = crate::service::contexte(&oc)?;
    tableau_compagnie(&oc, &ctx).map_err(|e| e.message)
}

/// LE TABLEAU DE L'ADMINISTRATEUR, a partir du tableau de chaque compagnie (`garde` : celui qui est garde, sans calcul ;
/// sinon refait). Les declarations de toutes, du plus presse au moins presse, et ce que le depot groupe propose.
fn assembler(inst: &Path, par_compagnie: Vec<(PathBuf, Result<Value, String>)>) -> Value {
    let fiches = compagnies::liste_de(inst);
    let mut cies = Vec::new();
    let mut toutes: Vec<Value> = Vec::new();
    let (mut test_n, mut test_k, mut reel_n, mut reel_k) = (0, 0, 0, 0);
    for (racine, t) in par_compagnie {
        let fiche = fiches.iter().find(|c| relatif(inst, &c.racine) == relatif(inst, &racine));
        let nom = fiche.map(|c| c.nom.clone()).unwrap_or_else(|| nom_de(inst, &racine));
        let id = fiche.map(|c| c.id.clone()).unwrap_or_default();
        let couleur = fiche.map(|c| c.couleur.clone()).unwrap_or_default();
        let (decl, le, erreur) = match &t {
            Ok(v) => (v.get("declarations").and_then(Value::as_array).cloned().unwrap_or_default(), v.get("le").cloned().unwrap_or(Value::Null), Value::Null),
            Err(m) => (Vec::new(), Value::Null, json!(m)),
        };
        let pt = decl.iter().filter(|d| d["pret"]["test"] == json!(true)).count();
        let pr = decl.iter().filter(|d| d["pret"]["reel"] == json!(true)).count();
        test_n += pt;
        reel_n += pr;
        test_k += (pt > 0) as usize;
        reel_k += (pr > 0) as usize;
        cies.push(json!({"id": id, "nom": nom, "couleur": couleur, "le": le, "erreur": erreur,
                         "declarations": decl.len(), "pretes_test": pt, "pretes_reel": pr}));
        for mut d in decl {
            d["compagnie"] = json!(nom);
            d["id"] = json!(id);
            d["couleur"] = json!(couleur);
            toutes.push(d);
        }
    }
    toutes.sort_by_key(cle_d_urgence);
    json!({
        "compagnies": cies,
        "declarations": toutes,
        // RIEN DE GROUPE tant qu'il n'y a pas au moins deux compagnies avec quelque chose de pret : sinon le depot se fait
        // comme d'habitude, dans la compagnie.
        "groupe": {
            "test": {"declarations": test_n, "compagnies": test_k, "propose": test_k >= 2},
            "reel": {"declarations": reel_n, "compagnies": reel_k, "propose": reel_k >= 2},
        },
    })
}

/// Le tableau de l'administrateur a partir du tableau de chaque compagnie, deja fait ailleurs (le fil de la fenetre,
/// ticket #244) : celui de chacune, ou son refus en phrase.
pub fn tableau_de(inst: &Path, par_compagnie: Vec<(PathBuf, Result<Value, String>)>) -> Value {
    let mut v = assembler(inst, par_compagnie);
    v["garde"] = json!(false);
    v
}

/// Le tableau tel qu'il est garde : AUCUN CALCUL, la lecture de `config/.tableau.json` de chaque compagnie rattachee.
/// L'accueil le montre tout de suite ; la fenetre le refait ensuite en fond (`tableau`).
pub fn tableau_garde(inst: &Path) -> Value {
    let par = rattachees(inst)
        .into_iter()
        .map(|r| {
            let t = std::fs::read_to_string(r.join(TABLEAU))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .ok_or_else(|| "Pas encore lu : le tableau se fait.".to_string());
            (r, t)
        })
        .collect();
    let mut v = assembler(inst, par);
    v["garde"] = json!(true);
    v
}

/// Le tableau refait, compagnie par compagnie. `relever` : avant, pour ce qui est parti en test et attend son compte rendu,
/// aller le chercher (la route `bilan-repetition` de la compagnie, comme la page d'une compagnie le fait).
pub fn tableau(o: &HashMap<String, String>, inst: &Path, relever: bool) -> Value {
    let mut par = Vec::new();
    for r in rattachees(inst) {
        if relever {
            if let Ok(t) = refaire_compagnie(o, &r) {
                for d in t.get("declarations").and_then(Value::as_array).into_iter().flatten() {
                    if d.get("etape").and_then(Value::as_str) == Some("en_test") {
                        let charge = json!({"quoi": "dsn", "fichier": d["fichier"]});
                        let _ = crate::routes::commande(&o_de(o, &r), &json!({"route": "bilan-repetition", "charge": charge}).to_string());
                    }
                }
            }
        }
        let t = refaire_compagnie(o, &r);
        par.push((r, t));
    }
    let mut v = assembler(inst, par);
    v["garde"] = json!(false);
    v
}

// ------------------------------------------------------------------ le depot groupe

/// LE DEPOT GROUPE : ce qui est pret (`reel` : teste et conforme ; sinon : ecrit et pas encore essaye) dans les compagnies
/// de l'administrateur part, compagnie par compagnie, par la route `deposer` de chacune. `motdepasse` (reel) : celui que la
/// fenetre a fait taper au moment de l'acte ; sans lui, le moteur le demande a un humain. Rend, declaration par
/// declaration, ce qui est parti et ce qui a refuse, et le tableau refait.
pub fn deposer(o: &HashMap<String, String>, inst: &Path, reel: bool, motdepasse: Option<String>) -> R<Value> {
    let groupe = rattachees(inst);
    if groupe.is_empty() {
        return Err(erreur("ErreurOperation", "Ce logiciel n'a pas d'administrateur : chaque compagnie dépose depuis sa page. Rien n'est parti."));
    }
    // Ce qui est pret, decide maintenant, sur l'etat d'aujourd'hui (pas sur le tableau garde).
    let mut a_faire: Vec<(PathBuf, String, Value)> = Vec::new();
    let mut resultats: Vec<Value> = Vec::new();
    for r in &groupe {
        let nom = nom_de(inst, r);
        match refaire_compagnie(o, r) {
            Ok(t) => {
                for d in t.get("declarations").and_then(Value::as_array).into_iter().flatten() {
                    if d["pret"][if reel { "reel" } else { "test" }] == json!(true) {
                        a_faire.push((r.clone(), nom.clone(), d.clone()));
                    }
                }
            }
            Err(m) => resultats.push(json!({"compagnie": nom, "parti": false, "refus": m})),
        }
    }
    if a_faire.is_empty() {
        return Err(erreur(
            "ErreurOperation",
            if reel {
                "Aucune déclaration n'est prête à partir pour de vrai : une déclaration part en réel après un test revenu conforme. Rien n'est parti."
            } else {
                "Aucune déclaration écrite n'attend son dépôt de test. Rien n'est parti."
            },
        ));
    }
    let mut mdp: Option<String> = None;
    if reel {
        let m = match motdepasse.filter(|m| !m.is_empty()) {
            Some(m) => m,
            None => {
                let acte = crate::demande::Acte::nouveau(
                    format!("depot REEL de {} declaration(s), dans {} compagnie(s)", a_faire.len(), groupe.len()),
                    "",
                    "net-entreprises (les declarations partent a l'Urssaf et aux organismes)",
                );
                crate::demande::mot_de_passe_de_l_acte(&a_faire[0].0, &acte).map_err(|p| erreur("PreuveInsuffisante", p))?
            }
        };
        let poste = Poste::de_ce_poste();
        if !groupe.iter().any(|r| ouvre(&poste, r, &m)) {
            return Err(erreur("PreuveInsuffisante", "Mot de passe de l'administrateur refusé. Rien n'est parti."));
        }
        mdp = Some(m);
    }
    for (racine, nom, d) in &a_faire {
        let mut charge = json!({"quoi": "dsn", "fichier": d["fichier"], "reel": reel});
        if let Some(m) = &mdp {
            charge["motdepasse"] = json!(m);
        }
        let mut sortie = json!({"compagnie": nom, "titre": d["titre"], "fichier": d["fichier"], "genre": d["genre"]});
        let rep = crate::routes::commande(&o_de(o, racine), &json!({"route": "deposer", "charge": charge}).to_string());
        crate::coffre::oublier_mot_de_passe_tape();
        match rep {
            Ok(v) if v.get("ok") == Some(&json!(true)) => {
                let res = &v["resultat"];
                sortie["parti"] = json!(true);
                sortie["flux"] = res.get("flux").cloned().unwrap_or(Value::Null);
                sortie["phrase"] = json!(if reel {
                    "Partie pour de vrai : son bilan arrive avec les retours."
                } else {
                    "Partie en test, compte rendu attendu : le dépôt réel se fait une fois le test revenu conforme."
                });
            }
            Ok(v) => {
                sortie["parti"] = json!(false);
                let message = v.get("erreur").map(|e| e.as_str().map(String::from).unwrap_or_else(|| e.to_string())).unwrap_or_default();
                sortie["refus"] = json!(crate::depot_acte::phrase_de(&Echec { classe: None, message }));
                sortie["classe"] = v.get("classe").cloned().unwrap_or(Value::Null);
            }
            Err(m) => {
                let (classe, message) = crate::routes_ouverture::decoder(&m);
                sortie["parti"] = json!(false);
                sortie["refus"] = json!(message);
                sortie["classe"] = json!(classe);
            }
        }
        resultats.push(sortie);
    }
    let partis = resultats.iter().filter(|r| r["parti"] == json!(true)).count();
    Ok(json!({
        "reel": reel,
        "partis": partis,
        "refuses": resultats.len() - partis,
        "resultats": resultats,
        "tableau": tableau(o, inst, false),
    }))
}

// ------------------------------------------------------------------ les routes

/// `administrateur-tableau` (`{"relever"?}`) et `administrateur-deposer` (`{"reel", "motdepasse"?}`), depuis n'importe
/// quelle compagnie de l'installation (`paie route --racine <une compagnie>`) : elles portent sur toutes celles de
/// l'administrateur. Un agent peut tout preparer et deposer en test ; un depot reel demande le mot de passe tape par un
/// humain.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let inst = installation_de(&ctx.racine);
    match nom {
        "administrateur-tableau" => Ok(tableau(o, &inst, charge.get("relever").is_some_and(vrai))),
        "administrateur-deposer" => {
            let reel = charge.get("reel").is_some_and(vrai);
            let mdp = charge.get("motdepasse").filter(|m| vrai(m)).map(py_str);
            deposer(o, &inst, reel, mdp)
        }
        autre => Err(erreur("ErreurOperation", format!("route inconnue : {}", autre))),
    }
}
