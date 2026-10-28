//! Les compagnies du poste, telles que l'accueil les propose (`moteur/compagnies.py`, lot J3b).
//!
//! A cote du code : `prive/` (la vraie), `compagnies/*` (les autres vraies), `fictives/*` (les compagnies fictives, livrees
//! avec le logiciel). La copie de developpement (`prive-dev/`) n'est jamais proposee. Bride (`bridage.rs`), l'accueil ne
//! montre que les fictives. Supprimer une compagnie, et garder au deblocage celles qu'on a ajoutees en demo : `suppression.rs`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use socle::fichiers::dossier_du_code;

pub const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";
const COULEUR_DEFAUT: &str = "#B22222";
const DOSSIER_ORIGINE: &str = ".origine";
/// Ce qui appartient au poste et survit a une remise a zero.
const PROPRES_AU_POSTE: [&str; 4] =
    ["config/diagnostic.json", "journal/signalements.json", "journal/incidents.json", "journal/interface.log"];

/// Le dossier du logiciel (`logiciel/`) : le `CODE` du Python.
pub fn logiciel() -> PathBuf {
    // Le dossier du code peut porter un `..` (`paie/../donnees`) : on le resout avant d'en prendre le parent.
    let code = dossier_du_code();
    // Sous Windows, `canonicalize` rend un chemin `\\?\C:\...` que l'explorateur ne sait pas ouvrir : on retire le prefixe.
    let code = std::fs::canonicalize(&code)
        .map(|c| c.to_string_lossy().strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(c))
        .unwrap_or(code);
    code.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// Ce qui est a cote du logiciel : `prive/`, `fictives/`, `compagnies/`, `maj/`. Le temps d'un rejeu de scenario
/// (`scenario::rejouer`), l'installation que le rejeu a montee, et elle seule.
pub fn base() -> PathBuf {
    if let Some(b) = BASE_DU_REJEU.read().ok().and_then(|b| b.clone()) {
        return b;
    }
    logiciel().parent().map(Path::to_path_buf).unwrap_or_default()
}

/// L'INSTALLATION DU REJEU (#236) : un rejeu de scenario monte sa propre installation, bridee en toutes lettres
/// (`.edition.json`), et le moteur n'en lit pas d'autre -- ni le `.edition.json` du depot, ni le paquet soude d'a cote du
/// code. Pose et retiree par `scenario::rejouer` seulement ; un poste n'en a jamais.
static BASE_DU_REJEU: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

pub fn poser_l_installation_du_rejeu(inst: Option<PathBuf>) {
    if let Ok(mut b) = BASE_DU_REJEU.write() {
        *b = inst;
    }
}

fn lire(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn ecrire(p: &Path, v: &Value) -> Result<(), String> {
    socle::ecriture::ecrire_json(p, v, 2)
}

/// La vraie compagnie : `prive/` a cote du code, sinon le code lui-meme.
pub fn dossier_reel() -> PathBuf {
    let voisin = base().join("prive");
    if voisin.join("config").is_dir() {
        voisin
    } else {
        logiciel()
    }
}

pub fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR).is_file()
}

/// Le dossier des COMPAGNIES FICTIVES, a cote du logiciel (lot U1 ; Leandre, 14/10/2026 : « On pourrait renommer les
/// compagnies demo en fictives »). Livrees avec le logiciel, rien n'en part : leurs envois sont joues.
pub const DOSSIER_FICTIVES: &str = "fictives";
/// Le nom de ce dossier avant le lot U1. Une installation deja posee le garde (la mise a jour ne porte que `logiciel/`) :
/// `ranger_les_fictives` le renomme une fois, au lancement.
const ANCIEN_DOSSIER_FICTIVES: &str = "demo";
/// Le marqueur d'une compagnie fictive : `COPIE-DE-DEVELOPPEMENT.json`, dont `copie_de` commence par ceci. Celles d'avant le
/// lot U1 disent `compagnie de demonstration …` : elles se lisent encore.
pub const MARQUE_FICTIVE: &str = "compagnie fictive";
const ANCIENNE_MARQUE_FICTIVE: &str = "compagnie de demonstration";

/// Une COMPAGNIE FICTIVE : son marqueur le dit (ses envois sont joues, rien n'en part).
pub fn est_fictive(racine: &Path) -> bool {
    lire(&racine.join(MARQUEUR))
        .and_then(|i| i.get("copie_de").filter(|v| socle::fichiers::vrai(v)).map(socle::valeur::valeur_en_texte))
        .is_some_and(|c| c.starts_with(MARQUE_FICTIVE) || c.starts_with(ANCIENNE_MARQUE_FICTIVE))
}

/// LA FICTIVE AJOUTEE (ticket #236, Leandre 22/10/2026) : la compagnie que le « + » d'un logiciel bride ajoute. C'est une
/// fictive (`est_fictive` : elle vit sous `fictives/`, s'ouvre sans mot de passe, ne retient aucune version, rien ne part
/// d'elle a l'editeur qu'un incident ou un message), mais elle porte des donnees qu'on y a mises, pas celles d'une
/// fictive livree : ses envois NE SONT PAS JOUES, ILS SONT REFUSES (`bridage::branchement`), elle n'a pas de photo
/// d'origine ni de remise a zero, et la livraison des fictives (`maj.rs`) ne la touche jamais. SA MARQUE : la cle
/// `ajoutee` de son marqueur (`COPIE-DE-DEVELOPPEMENT.json`, que `ajouter` ecrit) -- lue ICI et nulle part ailleurs.
pub const CLE_AJOUTEE: &str = "ajoutee";

///
/// UN DOSSIER DE `fictives/` SANS MARQUEUR DE FICTIVE (un marqueur retire ou illisible) se lit aussi comme ajoute : l'accueil
/// le propose parmi les fictives, rien ne doit en partir, et rien n'en est joue.
pub fn est_ajoutee(racine: &Path) -> bool {
    let marque = lire(&racine.join(MARQUEUR)).and_then(|i| i.get(CLE_AJOUTEE).cloned()).is_some_and(|v| socle::fichiers::vrai(&v));
    marque || (racine.parent().and_then(Path::file_name).is_some_and(|n| n == DOSSIER_FICTIVES) && !est_fictive(racine))
}

/// UNE FICTIVE LIVREE : une fictive qui n'est pas ajoutee. Ses envois sont joues ; elle a sa photo d'origine et sa remise
/// a zero.
pub fn est_fictive_livree(racine: &Path) -> bool {
    est_fictive(racine) && !est_ajoutee(racine)
}

/// Le prefixe du dossier d'une fictive ajoutee. Les fictives livrees s'appellent `fictive-…` (`ranger_les_fictives`, le
/// dossier `fictives/` du depot) : une fictive ajoutee ne prend jamais le nom d'une livree, et la livraison, qui ne pose
/// pas une fictive dont le nom est pris (`maj::poser_les_fictives_pretes`), n'en est jamais empechee.
pub const PREFIXE_AJOUTEE: &str = "ajoutee-";

/// AU LANCEMENT, une fois : le dossier `demo/` d'une installation posee avant le lot U1 devient `fictives/`, et chaque
/// `demo-<nom>` (et sa photo d'origine) `fictive-<nom>`. Rien ne bouge si `fictives/` existe deja (une reinstallation l'a
/// pose : l'ancien reste ou il est). Ce qui ne se renomme pas (un fichier ouvert ailleurs) se retente au lancement
/// suivant ; rien n'est perdu. Rend ce qui s'est passe, ou `None` s'il n'y avait rien a faire.
pub fn ranger_les_fictives(inst: &Path) -> Option<String> {
    let (ancien, neuf) = (inst.join(ANCIEN_DOSSIER_FICTIVES), inst.join(DOSSIER_FICTIVES));
    if !ancien.is_dir() || neuf.exists() {
        return None;
    }
    let renommer_dedans = |d: &Path| {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if let Some(reste) = nom.strip_prefix("demo-") {
                let _ = std::fs::rename(e.path(), d.join(format!("fictive-{}", reste)));
            }
        }
    };
    renommer_dedans(&ancien);
    renommer_dedans(&ancien.join(DOSSIER_ORIGINE));
    Some(match std::fs::rename(&ancien, &neuf) {
        Ok(()) => format!("Compagnies fictives rangees dans {}.", neuf.display()),
        Err(e) => format!("Compagnies fictives non rangees ({} -> {}) : {}", ancien.display(), neuf.display(), e),
    })
}

/// La fiche entreprise telle qu'elle vaut aujourd'hui ; vide si elle ne se lit pas (la tuile dit alors le dossier, et
/// la compagnie, ouverte, dit pourquoi).
fn fiche(racine: &Path) -> Value {
    lire(&racine.join("config").join("entreprise.json"))
        .and_then(|b| droit::contexte::fiche_au(&b, &droit::contexte::jour_reel(), "config/entreprise.json").ok())
        .unwrap_or_else(|| json!({}))
}

fn texte(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn nom(racine: &Path) -> String {
    let ent = fiche(racine);
    let repli = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let trouve = [&ent["nom_affichage"], &ent["raison_sociale"]]
        .into_iter()
        .find_map(|v| v.as_str().filter(|s| !s.is_empty()).map(String::from));
    trouve.unwrap_or(repli)
}

/// Le nom d'une compagnie tel que sa tuile le dit (`nom_affichage`, sinon la raison sociale, sinon son dossier).
pub fn nom_de(racine: &Path) -> String {
    nom(racine)
}

fn titre(s: &str) -> String {
    let mut sortie = String::new();
    let mut debut = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if debut {
                sortie.extend(c.to_uppercase());
            } else {
                sortie.extend(c.to_lowercase());
            }
            debut = false;
        } else {
            sortie.push(c);
            debut = true;
        }
    }
    sortie
}

fn lieu(racine: &Path) -> String {
    let ent = fiche(racine);
    let mut commune = texte(&ent["commune_affichage"]);
    if commune.is_empty() {
        commune = titre(&texte(&ent["commune"]));
    }
    let idcc = texte(&ent["idcc"]);
    let idcc = if idcc.is_empty() { String::new() } else { format!("IDCC {}", idcc) };
    [commune, idcc].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")
}

fn couleur_valide(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// La couleur d'une compagnie (`config/affichage.json`), rouge par defaut.
pub fn couleur(racine: &Path) -> String {
    lire(&racine.join("config").join("affichage.json"))
        .and_then(|d| d.get("couleur").and_then(Value::as_str).map(String::from))
        .filter(|c| couleur_valide(c))
        .unwrap_or_else(|| COULEUR_DEFAUT.to_string())
}

/// Le logo de la compagnie, s'il est pose (svg d'abord).
pub fn logo(racine: &Path) -> Option<PathBuf> {
    ["logo.svg", "logo.png"].iter().map(|n| racine.join("gabarits").join("identite").join(n)).find(|p| p.is_file())
}

#[derive(Clone, Debug)]
pub struct Compagnie {
    pub id: String,
    pub nom: String,
    pub lieu: String,
    pub racine: PathBuf,
    pub reelle: bool,
    pub couleur: String,
}

fn entree(id: String, racine: &Path, reelle: bool) -> Compagnie {
    let mut n = nom(racine);
    if !reelle {
        // Le nom d'une fictive porte parfois sa nature, entre parentheses : la tuile la dit deja.
        for suffixe in [" (fictive)", " (démonstration)", " (demonstration)"] {
            n = n.replace(suffixe, "");
        }
    }
    Compagnie { id, nom: n, lieu: lieu(racine), racine: racine.to_path_buf(), reelle, couleur: couleur(racine) }
}

/// Un dossier dont le nom commence par un point n'est jamais une compagnie : la photo d'origine des fictives
/// (`.origine/`), le reste d'une compagnie supprimee qui ne s'est pas efface (`suppression::effacer`).
pub fn est_cache(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

fn sous_dossiers(dossier: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dossier)
        .map(|it| {
            it.filter_map(|e| e.ok().map(|e| e.path())).filter(|d| !est_cache(d) && d.join("config").join("entreprise.json").is_file()).collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn nom_dossier(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Ce que propose l'accueil, dans l'ordre des tuiles : les vraies, puis les fictives.
pub fn liste() -> Vec<Compagnie> {
    liste_de(&base())
}

/// Les compagnies de l'installation `b` (le dossier qui porte `logiciel/`, `prive/`, `compagnies/`, `fictives/`), dans
/// l'ordre des tuiles. L'administrateur (`administrateur.rs`) lit celles de l'installation qu'on lui donne.
pub fn liste_de(b: &Path) -> Vec<Compagnie> {
    let mut out = Vec::new();
    if !crate::bridage::est_bride_de(b) {
        let reel = if b == base() { dossier_reel() } else { b.join("prive") };
        if reel.join("config").is_dir() {
            out.push(entree("reelle".into(), &reel, true));
        }
        for d in sous_dossiers(&b.join("compagnies")) {
            out.push(entree(format!("cie:{}", nom_dossier(&d)), &d, true));
        }
    }
    for d in sous_dossiers(&b.join(DOSSIER_FICTIVES)) {
        out.push(entree(format!("fictive:{}", nom_dossier(&d)), &d, false));
    }
    out
}

/// LES COMPAGNIES QUE LA FENETRE TIENT A JOUR EN FOND des son lancement (ticket #244, `Fenetre::prechauffer`) : les
/// vraies compagnies de l'installation `b` (pas les fictives), la derniere ouverte d'abord -- la date de son journal de
/// la fenetre (`journal/interface.log`), que chaque ouverture recommence ; jamais ouverte, a la fin.
pub fn a_tenir_a_jour(b: &Path) -> Vec<PathBuf> {
    let date = |r: &Path| std::fs::metadata(r.join("journal").join("interface.log")).and_then(|m| m.modified()).ok();
    let mut reelles: Vec<PathBuf> = liste_de(b).into_iter().filter(|c| c.reelle).map(|c| c.racine).collect();
    reelles.sort_by_key(|r| std::cmp::Reverse(date(r)));
    reelles
}

/// Le dossier d'une tuile -- et seulement d'une tuile : l'identifiant ne designe jamais un chemin qu'on n'a pas propose.
pub fn racine_choisie(id: &str) -> Result<PathBuf, String> {
    liste()
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.racine)
        .ok_or_else(|| format!("Compagnie inconnue ou absente de cette version : {}", id))
}

pub fn poser_couleur(id: &str, valeur: &str) -> Result<String, String> {
    if !couleur_valide(valeur) {
        return Err(format!("Couleur attendue sous la forme #RRGGBB : '{}'", valeur));
    }
    let c = liste().into_iter().find(|c| c.id == id && c.reelle).ok_or("Seule une compagnie reelle a une couleur a choisir.")?;
    let v = valeur.to_uppercase();
    ecrire(&c.racine.join("config").join("affichage.json"), &json!({"couleur": v}))?;
    Ok(v)
}

pub(crate) fn nom_de_dossier(nom: &str) -> String {
    let mut s = String::new();
    for c in nom.to_lowercase().chars() {
        let pli = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'î' | 'ï' | 'í' | 'ì' => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' => "u",
            'ç' => "c",
            'ñ' => "n",
            'ÿ' => "y",
            'œ' => "oe",
            'æ' => "ae",
            c if c.is_ascii_alphanumeric() => {
                s.push(c);
                continue;
            }
            c if !c.is_ascii() => "",
            _ => "-",
        };
        s.push_str(pli);
    }
    let mut net = String::new();
    for c in s.chars() {
        if c == '-' && (net.is_empty() || net.ends_with('-')) {
            continue;
        }
        net.push(c);
    }
    let net: String = net.trim_end_matches('-').chars().take(40).collect();
    if net.is_empty() {
        "compagnie".into()
    } else {
        net
    }
}

/// LE JOUR OU UNE COMPAGNIE A ETE AJOUTEE (#236) : `config/creation.json`, ecrit par `ajouter_dans` et nulle part
/// ailleurs ; il suit la compagnie quand elle change de dossier. Une compagnie d'avant (une fictive livree, la vraie
/// compagnie d'une installation) n'en a pas : `None`.
pub const FICHIER_CREATION: &str = "creation.json";

pub fn cree_le(racine: &Path) -> Option<socle::date::Jour> {
    lire(&racine.join("config").join(FICHIER_CREATION))?.get("le").and_then(Value::as_str).and_then(|j| socle::date::Jour::lire(j).ok())
}

/// « Ajouter une compagnie » : le dossier et les trois fiches sans lesquelles le contexte ne s'ouvre pas.
///
/// LOGICIEL BRIDE (ticket #236) : la compagnie ajoutee est une FICTIVE AJOUTEE (`est_ajoutee`) -- sous `fictives/`, dans
/// un dossier `ajoutee-…`, avec le marqueur d'une fictive et sa marque. Debloque : une compagnie reelle, sous `compagnies/`.
pub fn ajouter(nom: &str) -> Result<Compagnie, String> {
    ajouter_dans(&base(), nom)
}

/// `ajouter`, dans l'installation `inst` (le dossier qui porte `logiciel/`).
pub fn ajouter_dans(inst: &Path, nom: &str) -> Result<Compagnie, String> {
    let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
    if nom.chars().count() < 2 {
        return Err("Le nom de la compagnie, s'il vous plait.".into());
    }
    let bride = crate::bridage::est_bride_de(inst);
    let (parent, racine_nom) = if bride {
        (inst.join(DOSSIER_FICTIVES), format!("{}{}", PREFIXE_AJOUTEE, nom_de_dossier(&nom)))
    } else {
        (inst.join("compagnies"), nom_de_dossier(&nom))
    };
    let mut dossier = parent.join(&racine_nom);
    let mut n = 2;
    while dossier.exists() {
        dossier = parent.join(format!("{}-{}", racine_nom, n));
        n += 1;
    }
    let cfg = dossier.join("config");
    std::fs::create_dir_all(&cfg).map_err(|e| e.to_string())?;
    // La raison sociale se deduit du nom comme a chaque enregistrement de la fiche (`routes_fiches::deriver` : majuscules
    // SANS ACCENTS, la forme declaree). `to_uppercase` gardait les accents : la fiche neuve naissait avec une erreur de
    // configuration, et le premier enregistrement datait la correction comme un changement declare (#236).
    let mut ent = serde_json::Map::new();
    ent.insert("nom_affichage".into(), Value::String(nom.clone()));
    crate::routes_fiches::deriver("entreprise", &mut ent, false).map_err(|e| e.message)?;
    // Toute la configuration est datee (`contexte::fiche_au`) : la premiere valeur vaut de tout temps.
    // Le jour d'ajout : le plus tot des deux jours (Paris, et celui du moteur, en UTC).
    let creation = socle::temps::jour_de_paris().min(droit::contexte::jour_reel());
    ecrire(&cfg.join("entreprise.json"), &Value::Object(droit::contexte::fiche_datee(&ent)))?;
    ecrire(&cfg.join("salaries.json"), &json!({}))?;
    ecrire(&cfg.join("spectacles.json"), &json!({}))?;
    ecrire(
        &cfg.join(FICHIER_CREATION),
        &json!({"le": creation.iso(), "_quoi": "Le jour ou la compagnie a ete ajoutee a PuppetPaie (« Ajouter une compagnie »). Ce qui tombait avant lui ne la regarde pas (l'horloge d'une fictive ne coche pas une echeance d'avant)."}),
    )?;
    if !bride {
        return Ok(entree(format!("cie:{}", nom_dossier(&dossier)), &dossier, true));
    }
    // Le marqueur d'une fictive (`est_fictive`), et la marque qui la dit ajoutee. `copie_de` ne dit pas le nom d'une
    // fictive livree (`maj::marquee_comme_livree` compare « compagnie fictive <nom du dossier> ») : elle n'en est pas une.
    ecrire(
        &dossier.join(MARQUEUR),
        &json!({
            "copie_de": format!("{} ajoutee sur ce poste", MARQUE_FICTIVE),
            CLE_AJOUTEE: true,
            "quand": socle::paris::maintenant(),
            "_quoi": "Une compagnie fictive ajoutee sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.",
        }),
    )?;
    Ok(entree(format!("fictive:{}", nom_dossier(&dossier)), &dossier, false))
}

fn origine(racine: &Path) -> PathBuf {
    racine.parent().unwrap_or(racine).join(DOSSIER_ORIGINE).join(nom_dossier(racine))
}

fn copier_arbre(de: &Path, vers: &Path, rel: &Path, sauf: &[&str]) -> std::io::Result<()> {
    std::fs::create_dir_all(vers.join(rel))?;
    for e in std::fs::read_dir(de.join(rel))? {
        let e = e?;
        let r = rel.join(e.file_name());
        let rs = r.to_string_lossy().replace('\\', "/");
        if sauf.contains(&rs.as_str()) || e.file_name() == "__pycache__" {
            continue;
        }
        if e.file_type()?.is_dir() {
            copier_arbre(de, vers, &r, sauf)?;
        } else {
            std::fs::copy(e.path(), vers.join(&r))?;
        }
    }
    Ok(())
}

/// La photo d'origine d'une fictive, prise avant le premier geste (un dossier `.origine/<nom>/`).
pub fn figer_origine(racine: &Path) -> Result<(), String> {
    if !est_fictive_livree(racine) {
        return Ok(());
    }
    let o = origine(racine);
    if o.is_dir() {
        return Ok(());
    }
    let provisoire = o.with_extension("part");
    let _ = std::fs::remove_dir_all(&provisoire);
    copier_arbre(racine, &provisoire, Path::new(""), &PROPRES_AU_POSTE).map_err(|e| e.to_string())?;
    std::fs::rename(&provisoire, &o).map_err(|e| e.to_string())
}

/// La fictive revient a sa photo d'origine ; ce qui appartient au poste est garde. Rend (nom, deja).
pub fn remettre_a_zero(id: &str) -> Result<(String, bool), String> {
    let c = liste()
        .into_iter()
        .find(|c| c.id == id && !c.reelle && est_fictive_livree(&c.racine))
        .ok_or("Seule une compagnie fictive livrée se remet à zéro.")?;
    let o = origine(&c.racine);
    if !o.is_dir() {
        return Ok((c.nom, true));
    }
    let parent = c.racine.parent().unwrap_or(&c.racine).to_path_buf();
    let n = nom_dossier(&c.racine);
    let neuve = parent.join(format!(".{}.neuve", n));
    let ancienne = parent.join(format!(".{}.ancienne", n));
    for d in [&neuve, &ancienne] {
        let _ = std::fs::remove_dir_all(d);
    }
    copier_arbre(&o, &neuve, Path::new(""), &[]).map_err(|e| e.to_string())?;
    for rel in PROPRES_AU_POSTE {
        if let Ok(octets) = std::fs::read(c.racine.join(rel)) {
            let cible = neuve.join(rel);
            if let Some(p) = cible.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::write(cible, octets).map_err(|e| e.to_string())?;
        }
    }
    std::fs::rename(&c.racine, &ancienne).map_err(|e| e.to_string())?;
    std::fs::rename(&neuve, &c.racine).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&ancienne);
    Ok((c.nom, false))
}

/// L'etiquette du bandeau d'une compagnie qui n'est pas reelle (deja en HTML).
pub fn texte_copie(racine: &Path) -> String {
    if est_ajoutee(racine) {
        return "compagnie fictive &mdash; version d&eacute;mo, rien ne part".into();
    }
    if est_fictive(racine) {
        return "compagnie fictive &mdash; les envois sont simul&eacute;s".into();
    }
    "copie de test &mdash; rien ne part en r&eacute;el".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("puppetpaie-compagnies-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Une installation posee avant le lot U1 : `demo/demo-<nom>` (et sa photo d'origine) devient `fictives/fictive-<nom>`,
    /// une fois ; ensuite il n'y a plus rien a faire.
    #[test]
    fn l_ancien_dossier_des_fictives_se_range_une_fois() {
        let inst = dossier("ranger");
        std::fs::create_dir_all(inst.join("demo/demo-3090-petite/config")).unwrap();
        std::fs::create_dir_all(inst.join("demo/.origine/demo-3090-petite/config")).unwrap();
        std::fs::write(inst.join("demo/demo-3090-petite/config/entreprise.json"), "{}").unwrap();
        assert!(ranger_les_fictives(&inst).is_some_and(|t| t.starts_with("Compagnies fictives rangees")));
        assert!(!inst.join("demo").exists());
        assert!(inst.join("fictives/fictive-3090-petite/config/entreprise.json").is_file());
        assert!(inst.join("fictives/.origine/fictive-3090-petite/config").is_dir(), "la photo d'origine suit");
        assert_eq!(ranger_les_fictives(&inst), None, "une seule fois");
        // `fictives/` deja la (une reinstallation) : l'ancien reste ou il est.
        std::fs::create_dir_all(inst.join("demo/demo-x")).unwrap();
        assert_eq!(ranger_les_fictives(&inst), None);
        assert!(inst.join("demo/demo-x").is_dir());
        let _ = std::fs::remove_dir_all(&inst);
    }

    /// Le marqueur d'une fictive : le neuf, et celui d'avant le lot U1 ; une copie de developpement n'en est pas une.
    #[test]
    fn une_fictive_se_lit_a_son_marqueur_neuf_ou_ancien() {
        let r = dossier("marqueur");
        let poser = |t: &str| std::fs::write(r.join(MARQUEUR), format!("{{\"copie_de\": \"{}\"}}", t)).unwrap();
        poser("compagnie fictive fictive-3090-petite");
        assert!(est_fictive(&r) && est_copie(&r));
        poser("compagnie de demonstration demo-3090-petite");
        assert!(est_fictive(&r), "le marqueur d'avant le lot U1");
        poser("prive");
        assert!(!est_fictive(&r) && est_copie(&r));
        let _ = std::fs::remove_dir_all(&r);
    }
}
