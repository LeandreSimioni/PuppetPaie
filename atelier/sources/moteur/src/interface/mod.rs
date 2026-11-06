//! La fenetre de PuppetPaie : le serveur local qui sert la page, l'accueil des compagnies et l'instance unique du
//! poste.
//!
//! UN SERVEUR LOCAL, TROIS ETATS. Le serveur (127.0.0.1, un jeton exige a chaque appel) sert `/` selon l'etat :
//!   - l'ACCUEIL : les tuiles des compagnies ;
//!   - le MOT DE PASSE de la compagnie choisie (ou son choix, a la premiere ouverture) ; une fictive n'en a pas ;
//!   - la COMPAGNIE OUVERTE : la page du logiciel (`moteur/interface/`), ses appels `/api/<route>` (`api.rs`) et la veille.
//! Les pages de l'accueil appellent `window.pywebview.api.<nom>(...)`, comme dans la fenetre pywebview : une cale
//! (`CALE`) poste ces appels a `/lanceur/<nom>`. La page ne sait pas dans quoi elle tourne.
//!
//! LA FENETRE. Tauri (`fenetre/`, le binaire `puppetpaie`) montre `adresse()` dans une vraie fenetre ; `moteur interface`
//! l'ouvre dans un Chromium en mode application, sinon dans le navigateur par defaut (`navigateur.rs`).

pub mod api;
pub mod auteur;
pub mod compagnies;
pub mod http;
pub mod instance;
pub mod navigateur;
pub mod poste;
pub mod suppression;
pub mod veille;

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use http::{Reponse, Requete};
use crate::etat_garde::{Fil, FileDesEtats};
use veille::Veille;
use crate::routes::catalogue::Route;

pub const MINUTES_VEILLE: u64 = 20;

/// Le journal de la fenetre, sous la racine de la compagnie qu'elle sert.
pub const JOURNAL_INTERFACE: &str = "journal/interface.log";

const MOT_DE_PASSE: &str = include_str!("pages/mot_de_passe.html");
const CHOIX_MOT_DE_PASSE: &str = include_str!("pages/choix_mot_de_passe.html");
const ACCUEIL: &str = include_str!("pages/accueil.html");
const CRAYON: &str = include_str!("pages/crayon.html");
const ZERO: &str = include_str!("pages/zero.html");
const TUILE_AJOUTER: &str = include_str!("pages/tuile_ajouter.html");
const AVIS_AJOUTER: &str = include_str!("pages/avis_ajouter.html");
const GARDER: &str = include_str!("pages/garder.html");

/// Les appels `window.pywebview.api.<nom>(...)` de l'accueil, postes au serveur. Une reponse qui dit `recharger`
/// fait revenir la fenetre sur `/` : c'est le serveur qui sait quelle page montrer ensuite.
const CALE: &str = r#"<script>
(function () {
  const JETON = "{{JETON}}";
  // Une reponse se lit toujours en `{ok, erreur}`, `erreur` en phrase : un corps qui n'est pas du JSON, un
  // refus en objet ou un serveur parti ne levent pas dans la page.
  const lire = (r) => r.text().then((t) => {
    try { return JSON.parse(t); }
    catch (e) { return { ok: false, erreur: "PuppetPaie a répondu " + r.status + " sans dire pourquoi." }; }
  });
  const appel = (nom, args) => fetch("/lanceur/" + nom, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
      body: JSON.stringify(args),
    }).then(lire, () => ({ ok: false, erreur: "PuppetPaie ne répond plus : relancez le logiciel." }))
      .then((d) => {
        if (d && d.erreur && typeof d.erreur === "object") d.erreur = d.erreur.message || "Refusé.";
        if (d && d.recharger) location.replace("/");
        return d;
      });
  const api = {};
  for (const nom of {{NOMS}}) api[nom] = (...args) => appel(nom, args);
  window.pywebview = { api };
  // UNE ROUTE DU CATALOGUE, par la porte de toute la page (`/api/<route>`) : `{ok, resultat}`, ou `{ok: false, erreur}`
  // avec la phrase du refus.
  window.appel = (nom, charge) => fetch("/api/" + nom, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
      body: JSON.stringify(charge || {}),
    }).then(lire, () => ({ ok: false, erreur: "PuppetPaie ne répond plus : relancez le logiciel." }))
      .then((d) => {
        if (d && d.erreur && typeof d.erreur === "object") d.erreur = d.erreur.message || "Refusé.";
        return d;
      });
  document.addEventListener("DOMContentLoaded", () => window.dispatchEvent(new Event("pywebviewready")));
})();
</script>
"#;

/// LES GESTES DE LA FENETRE ELLE-MEME (`window.pywebview.api.<nom>`, servis par `Fenetre::lanceur`) : aucun n'est un geste
/// du logiciel, chacun dit ce que la fenetre montre ou fait d'elle-meme. Tout le reste passe par la porte de la page, les
/// routes du catalogue (`window.appel`, `/api/<route>`). A l'accueil et aux pages du mot de passe :
/// - `choisir` : montrer la compagnie d'une tuile (sa page, ou d'abord son mot de passe) ; `verifier` : la deverrouiller dans
///   cette fenetre par son mot de passe (en ligne de commande, `--racine` et la seance du coffre) ; `accueil` : revenir
///   aux tuiles ;
/// - `fermer`, `redemarrer` : fermer la fenetre, la rouvrir sur la version prete ; `maj` : ou en est la recherche que la
///   fenetre mene en fond (la version, elle, est la route `maj-etat`) ; `site` : ouvrir dans le navigateur du poste le site
///   de l'editeur (le lien de l'auteur, `poste::ouvrir_site`, qui refuse toute autre adresse) ;
/// - `basculer` : brider ou debloquer le logiciel (la demo), le seul geste qui n'existe qu'a l'ecran : la ligne de commande
///   n'a pas de bridage.
const NOMS_ACCUEIL: &str = r#"["choisir", "verifier", "accueil", "fermer", "redemarrer", "maj", "site", "basculer"]"#;
/// Dans la page d'une compagnie : revenir aux compagnies, la mise a jour (`js/mise-a-jour.js`), ouvrir ce qu'une route a
/// rendu et choisir un dossier (`poste.rs`), le site de l'editeur (`site`), la veille de la fenetre (`js/veille.js`).
const NOMS_LOGICIEL: &str = r#"["accueil", "maj", "redemarrer", "site", "ouvrir", "choisir_dossier", "veille", "veille_etape", "veille_maintenant"]"#;

#[derive(Clone)]
enum Phase {
    Accueil,
    MotDePasse { racine: PathBuf, premiere: bool },
    Ouverte { racine: PathBuf, veille: Arc<Veille>, etats: Arc<FileDesEtats> },
}

pub struct Fenetre {
    jeton: String,
    /// Le port de la page : le cookie du jeton porte son nom (`jeton-<port>`), car un cookie ne connait pas le port --
    /// deux installations ouvertes dans le meme navigateur ne s'ecrasent pas leur jeton.
    port: u16,
    /// Le dossier de cette fenetre sur le poste (`instance::dossier_local`) : le jeton, le profil du navigateur, le
    /// journal d'avant toute compagnie.
    local: PathBuf,
    /// LE FIL DES CALCULS EN FOND (`etat_garde::Fil`) et la file de chaque compagnie qu'il tient, par
    /// dossier : nees au lancement pour les compagnies de l'installation (`prechauffer`), ou a l'ouverture d'une autre ;
    /// reprises, jamais refaites, quand la compagnie s'ouvre, se ferme et se rouvre.
    fil: Arc<Fil>,
    files: Mutex<HashMap<PathBuf, Arc<FileDesEtats>>>,
    phase: Mutex<Phase>,
    /// LE MOT DE PASSE DE L'ADMINISTRATEUR, tape une fois dans cette fenetre : il ouvre ses autres
    /// compagnies sans le redemander tant que la fenetre vit. Il OUVRE, il n'autorise rien : un acte reel redemande
    /// toujours le mot de passe au moment de l'acte. Il ne va dans aucune route, aucun fichier, aucun journal.
    administrateur: Mutex<Option<String>>,
    temoin: Mutex<Instant>,
    journal: Mutex<Option<PathBuf>>,
    minutes_veille: u64,
    /// Ce que fait « fermer » : arreter le processus (le navigateur), ou fermer la fenetre Tauri.
    pub fermer: Mutex<Option<Box<dyn Fn() + Send>>>,
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

/// La version telle que la fenetre la montre : `JJ.MM.AAAA` (`maj::version_lue`) ; le fichier garde l'identifiant.
/// Le binaire de l'atelier n'est pas une version publiee : il dit de quelles sources il est fait (le commit, `build.rs`).
#[cfg(feature = "atelier")]
fn version() -> String {
    let commit = env!("PAIE_COMMIT");
    if commit.is_empty() {
        "de développement".to_string()
    } else {
        format!("de développement, {}", commit)
    }
}

#[cfg(not(feature = "atelier"))]
fn version() -> String {
    crate::compagnie::maj::version_lue(&std::fs::read_to_string(compagnies::logiciel().join("VERSION")).unwrap_or_else(|_| "0.0.0".into()))
}

fn ressources() -> PathBuf {
    compagnies::logiciel().join("interface")
}

/// Douze caracteres qui changent des qu'une feuille ou un script change : la fenetre ne garde rien de perime.
/// Tous les `.css` et `.js` de `interface/` et de `interface/js/`, tries par nom : la meme empreinte d'un
/// lancement a l'autre. Le nom entre dans le condensat avec le contenu.
fn empreinte() -> String {
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;
    let mut noms: Vec<(String, Option<(u128, u64)>)> = Vec::new();
    for dossier in ["", "js"] {
        let Ok(it) = std::fs::read_dir(ressources().join(dossier)) else { continue };
        for e in it.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_file()) && (nom.ends_with(".css") || nom.ends_with(".js")) {
                let nom = if dossier.is_empty() { nom } else { format!("{}/{}", dossier, nom) };
                let sig = socle::fichiers::signature(&ressources().join(&nom));
                noms.push((nom, sig));
            }
        }
    }
    noms.sort();
    // LE CONTENU NE SE RELIT QUE S'IL A CHANGE : la date et la taille de chaque fichier suffisent a savoir
    // que rien n'a bouge depuis la derniere page -- la relecture et le condensat de 800 Ko a chaque `GET /` coutaient.
    static DERNIERE: OnceLock<Mutex<Option<(Vec<(String, Option<(u128, u64)>)>, String)>>> = OnceLock::new();
    let memo = DERNIERE.get_or_init(|| Mutex::new(None));
    if let Some((vus, e)) = memo.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        if *vus == noms {
            return e.clone();
        }
    }
    let mut h = Sha256::new();
    for (nom, _) in &noms {
        if let Ok(o) = std::fs::read(ressources().join(nom)) {
            h.update(nom.as_bytes());
            h.update([0u8]);
            h.update((o.len() as u64).to_le_bytes());
            h.update(&o);
        }
    }
    let e: String = h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect();
    *memo.lock().unwrap_or_else(|p| p.into_inner()) = Some((noms, e.clone()));
    e
}

/// Un fichier de la page (`interface/`), servi tel quel. Le chemin arrive decode : il n'est pris que s'il est fait de
/// noms ordinaires (ni `..`, ni `.`, ni racine, ni lecteur, ni `\`, ni `:`), d'une extension connue, et si, resolu,
/// il est un fichier SOUS `interface/`. Tout le reste est un 404.
fn fichier_de_la_page(chemin: &str) -> Option<(Vec<u8>, &'static str)> {
    use std::path::Component;
    let rel = chemin.strip_prefix('/')?;
    if rel.is_empty() || rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        return None;
    }
    if rel.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return None;
    }
    let rel = Path::new(rel);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let mime = match rel.extension().and_then(|e| e.to_str())? {
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "html" => "text/html; charset=utf-8",
        _ => return None,
    };
    let racine = std::fs::canonicalize(ressources()).ok()?;
    let vrai = std::fs::canonicalize(racine.join(rel)).ok()?;
    if !vrai.starts_with(&racine) || !vrai.is_file() {
        return None;
    }
    std::fs::read(&vrai).ok().map(|o| (o, mime))
}

/// Le jeton de cette fenetre, tire une fois et garde dans son dossier sur le poste (`instance::dossier_local`).
fn jeton(local: &Path) -> String {
    let p = local.join(".jeton-interface");
    if let Ok(j) = std::fs::read_to_string(&p) {
        let j = j.trim();
        if j.len() >= 24 && j.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return j.to_string();
        }
    }
    let mut o = [0u8; 24];
    let _ = getrandom::fill(&mut o);
    let j: String = o.iter().map(|b| format!("{:02x}", b)).collect();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&p, &j);
    j
}

/// Un JSON pose dans la page, dans un `<script type="application/json">` : rien ne peut y fermer la balise.
fn json_dans_la_page(v: &impl serde::Serialize) -> String {
    serde_json::to_string(v).unwrap_or_default().replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

fn page(corps: &str, jeton: &str, noms: &str) -> String {
    // Ce que l'accueil et les pages du mot de passe disent : les phrases de la fenetre (`{{P:...}}`), la page n'en ecrit aucune.
    let corps = PHRASES_DE_L_ACCUEIL.iter().fold(corps.to_string(), |c, (cle, phrase)| c.replace(&format!("{{{{P:{}}}}}", cle), phrase));
    format!(
        "<!doctype html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>PuppetPaie</title>\n{}</head><body>\n{}\n</body></html>",
        CALE.replace("{{JETON}}", jeton).replace("{{NOMS}}", noms),
        corps
    )
}

fn logo_en_ligne(logo: Option<&str>) -> String {
    match logo.map(Path::new) {
        Some(p) => {
            let mime = if p.extension().is_some_and(|e| e == "svg") { "image/svg+xml" } else { "image/png" };
            std::fs::read(&p).map(|o| format!("data:{};base64,{}", mime, socle::base64::ecrire(&o))).unwrap_or_default()
        }
        None => String::new(),
    }
}

/// L'accueil : les tuiles des compagnies, telles que la route `compagnies` les dit.
fn page_accueil() -> String {
    let lu = api::route_de_l_installation(Route::Compagnies, &json!({}));
    let r = &lu["resultat"];
    let t = |c: &Value, k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut tuiles = Vec::new();
    for c in r["compagnies"].as_array().into_iter().flatten() {
        let (nom, couleur, reelle) = (t(c, "nom"), t(c, "couleur"), c["reelle"] == json!(true));
        let logo = logo_en_ligne(c.get("logo").and_then(Value::as_str));
        let medaillon = if logo.is_empty() {
            format!("<span class=\"initiale\">{}</span>", echapper(&nom.chars().take(1).collect::<String>()))
        } else {
            format!("<img src=\"{}\" alt=\"\">", logo)
        };
        tuiles.push((
            reelle,
            format!(
                "<div class=\"tuile{}\" data-id=\"{}\"{}><div class=\"tete\"><div class=\"medaillon\">{}</div>{}</div><div class=\"corps\"><span class=\"nom\">{}</span><span class=\"lieu\">{}</span></div></div>",
                if reelle { "" } else { " fictive" },
                echapper(&t(c, "id")),
                if reelle { format!(" style=\"--cie:{}\"", couleur) } else { String::new() },
                medaillon,
                if reelle {
                    CRAYON.replace("%s", &couleur)
                } else if c["remise_a_zero"] == json!(true) {
                    ZERO.to_string()
                } else {
                    String::new()
                },
                echapper(&nom),
                echapper(&t(c, "lieu"))
            ),
        ));
    }
    let bride = r["bride"] == json!(true);
    let grille = if bride {
        // BRIDE, LE « + » AUSSI : il ajoute une fictive (la route `compagnie-ajouter`), apres le message.
        format!("<div class=\"grille\">{}{}</div>{}", tuiles.iter().map(|(_, t)| t.as_str()).collect::<String>(), TUILE_AJOUTER, AVIS_AJOUTER)
    } else {
        let reelles: String = tuiles.iter().filter(|(r, _)| *r).map(|(_, t)| t.as_str()).collect::<String>() + TUILE_AJOUTER;
        let fictives: Vec<&str> = tuiles.iter().filter(|(r, _)| !*r).map(|(_, t)| t.as_str()).collect();
        let mut g = format!("<div class=\"grille\">{}</div>", reelles);
        if !fictives.is_empty() {
            g += &format!(
                "<details class=\"fictives\"><summary>Compagnies fictives ({}) &mdash; pour essayer, rien ne part</summary><div class=\"grille\">{}</div></details>",
                fictives.len(),
                fictives.concat()
            );
        }
        g
    };
    // AU DEBLOCAGE : tant qu'il reste des compagnies creees en demo, la question se pose a chaque accueil.
    let a_garder = r["a_garder"].as_array().cloned().unwrap_or_default();
    let grille = if a_garder.is_empty() {
        grille
    } else {
        let liste: String = a_garder
            .iter()
            .map(|c| format!("<li><label><input type=\"checkbox\" value=\"{}\" checked> {}</label></li>", echapper(&t(c, "id")), echapper(&t(c, "nom"))))
            .collect();
        format!("{}{}", grille, GARDER.replace("{{LISTE}}", &liste))
    };
    // La version, sous le nom, comme dans la page d'une compagnie ; les tuiles en dernier : un nom de
    // compagnie n'est jamais lu comme une marque a remplacer.
    ACCUEIL
        .replace("{{VERSION_DITE}}", &format!("version {}", version()))
        .replace("{{AUTEUR}}", &json_dans_la_page(&r["auteur"]))
        .replace("{{EMPREINTE}}", &empreinte())
        .replace(
            "{{SOUS_TITRE}}",
            if bride { "des compagnies fictives, et rien ne part" } else { "quelle compagnie ouvrir ?" },
        )
        .replace("{{TUILES}}", &grille)
}

/// La page du logiciel, telle que la fenetre la recoit (publique pour `tests/interface_fichiers.rs`).
pub fn page_logiciel(racine: &Path, jeton: &str) -> Result<String, String> {
    let brut = std::fs::read_to_string(ressources().join("index.html")).map_err(|e| socle::echec::du_disque(&ressources().join("index.html"), &e))?;
    let copie = compagnies::est_copie(racine);
    let mut p = brut
        .replace("{{JETON}}", jeton)
        .replace("{{EMPREINTE}}", &empreinte())
        .replace("{{COPIE}}", if copie { "1" } else { "" })
        .replace("{{COPIE_TEXTE}}", &compagnies::texte_copie(racine))
        .replace(
            "{{STYLE_COMPAGNIE}}",
            &if copie { String::new() } else { format!("<style>:root {{ --compagnie: {}; }}</style>", compagnies::couleur(racine)) },
        )
        .replace("{{PHRASES}}", &PHRASES_DE_LA_FENETRE.to_string())
        .replace("{{VERSION_DITE}}", &format!("version {}{}", version(), if crate::compagnie::bridage::est_bride() { " &middot; d&eacute;mo" } else { "" }));
    let cale = CALE.replace("{{JETON}}", jeton).replace("{{NOMS}}", NOMS_LOGICIEL);
    // Avant le PREMIER script de la page (`/js/tuyau.js`, qui definit `api`) : la cale doit exister avant lui.
    p = p.replacen("<script src=\"/js/", &format!("{}<script src=\"/js/", cale), 1);
    Ok(p)
}

impl Fenetre {
    pub fn nouvelle(minutes_veille: u64, local: PathBuf, port: u16) -> Arc<Fenetre> {
        Arc::new(Fenetre {
            jeton: jeton(&local),
            port,
            local,
            fil: Fil::nouveau(),
            files: Mutex::new(HashMap::new()),
            phase: Mutex::new(Phase::Accueil),
            administrateur: Mutex::new(None),
            temoin: Mutex::new(Instant::now()),
            journal: Mutex::new(None),
            minutes_veille,
            fermer: Mutex::new(None),
        })
    }

    pub fn ecrire(&self, texte: &str) {
        let chemin = self.journal.lock().ok().and_then(|j| j.clone()).unwrap_or_else(|| self.local.join("interface.log"));
        if let Some(d) = chemin.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&chemin) {
            let _ = f.write_all(texte.as_bytes());
        }
    }

    fn phase(&self) -> Phase {
        self.phase.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// La phase change. La compagnie qu'on quitte arrete sa veille ; sa file reste au fil (elle se rouvrira sans attendre),
    /// et ne passe plus devant les autres. Revenir a l'accueil : aucune compagnie devant.
    fn poser_phase(&self, p: Phase) {
        let accueil = matches!(p, Phase::Accueil);
        let avant = std::mem::replace(&mut *self.phase.lock().unwrap_or_else(|e| e.into_inner()), p);
        if let Phase::Ouverte { veille, .. } = avant {
            veille.arreter();
        }
        if accueil {
            self.fil.devant(None);
        }
    }

    /// Le dossier de cette fenetre sur le poste (`instance::dossier_local`).
    pub fn dossier_local(&self) -> &Path {
        &self.local
    }

    /// La file de la compagnie `racine` sur le fil de la fenetre : celle qu'il tient deja, sinon une neuve.
    fn file_de(&self, racine: &Path) -> Arc<FileDesEtats> {
        let cle = std::fs::canonicalize(racine).unwrap_or_else(|_| racine.to_path_buf());
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        files.entry(cle).or_insert_with(|| self.fil.file(racine, None)).clone()
    }

    /// LES COMPAGNIES DE L'INSTALLATION, TENUES A JOUR DES LE LANCEMENT : n'importe quelle compagnie s'ouvre
    /// instantanement, meme a la premiere ouverture du jour. Chaque vraie compagnie de l'installation -- celles de
    /// l'administrateur, et celles qui ont garde leur mot de passe : toutes sont sur ce disque, en clair -- recoit sa file
    /// sur le fil de la fenetre, qui verifie et refait en fond son etat d'ouverture, son tableau (une compagnie de
    /// l'administrateur), puis ses mois. La derniere ouverte d'abord (la date de son journal de la fenetre, que chaque
    /// ouverture recommence), puis chacune son tour.
    ///
    /// AVANT TOUT MOT DE PASSE : du calcul local seulement. Le fil ne sort pas et n'ouvre aucun coffre (`fil_de_fond`) ;
    /// la veille (le reseau, le coffre, la releve, la sauvegarde) ne demarre qu'a l'ouverture d'une compagnie, apres son
    /// mot de passe (`ouvrir`). Les donnees sont en clair sur le disque : ce calcul ne revele rien que le disque ne montre
    /// deja. Les compagnies fictives ne sont pas prechauffees : elles s'ouvrent sur leur dernier etat garde, et le fil les
    /// prend a leur ouverture. Bride, rien.
    pub fn prechauffer(self: &Arc<Self>) {
        if crate::compagnie::bridage::est_bride() {
            return;
        }
        let f = self.clone();
        crate::routes::fil_de_routes(move || {
            let reelles = compagnies::a_tenir_a_jour(&compagnies::base());
            for r in &reelles {
                f.file_de(r).tout_verifier();
            }
            let noms: Vec<String> = reelles.iter().map(|r| r.display().to_string()).collect();
            f.ecrire(&format!("Calcul en fond, sans rien ouvrir : {} compagnie(s) a tenir a jour ({}).\n", reelles.len(), noms.join(", ")));
        });
    }

    /// Le nom du cookie du jeton : un par port (un cookie ne distingue pas les ports d'un meme hote).
    fn nom_du_cookie(&self) -> String {
        format!("jeton-{}", self.port)
    }

    /// Les secondes depuis la derniere requete autorisee (le guet du navigateur : plus de page, plus de serveur).
    pub fn silence(&self) -> Duration {
        self.temoin.lock().map(|t| t.elapsed()).unwrap_or_default()
    }

    /// La compagnie choisie devient celle de la fenetre ; une fictive s'ouvre sans mot de passe.
    fn prendre(&self, racine: PathBuf) -> Result<(), String> {
        if let Err(e) = compagnies::figer_origine(&racine) {
            self.ecrire(&format!("Photo d'origine non prise : {}\n", e));
        }
        // Aucune variable du processus ne dit la compagnie : chaque route la recoit (`--racine`, `o["racine"]`,
        // le `Contexte`), et la fenetre pourra en servir plusieurs a la fois.
        let journal = racine.join(JOURNAL_INTERFACE);
        if let Some(d) = journal.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&journal, "");
        *self.journal.lock().unwrap_or_else(|e| e.into_inner()) = Some(journal);
        self.ecrire(&format!("Compagnie : {}\n", racine.display()));
        // La compagnie choisie passe devant les autres sur le fil des le choix : son calcul local se fait pendant qu'on
        // tape son mot de passe.
        let etats = self.file_de(&racine);
        self.fil.devant(Some(&etats));
        etats.tout_verifier();
        if crate::compagnie::bridage::est_bride() || compagnies::est_fictive(&racine) {
            self.ouvrir(racine);
            return Ok(());
        }
        let exigee = crate::compagnie::coffre::Coffre::du_poste(&racine, "").ouverture_exigee().map_err(|e| e.message)?;
        // UNE COMPAGNIE DE L'ADMINISTRATEUR, quand son mot de passe a deja ete tape dans cette fenetre : elle s'ouvre sans
        // le redemander (il ouvre encore son coffre : c'est verifie, pas suppose).
        if exigee && self.administrateur_ouvre(&racine) {
            self.ecrire("Ouverte par le mot de passe de l'administrateur, deja tape dans cette fenetre.\n");
            self.ouvrir(racine);
            return Ok(());
        }
        self.poser_phase(Phase::MotDePasse { racine, premiere: !exigee });
        Ok(())
    }

    /// LA FENETRE LACHE LA COMPAGNIE `racine`, qui va etre supprimee ou deplacee (`suppression.rs`) : ouverte, sa veille
    /// s'arrete (le tour en cours se finit) et l'on revient a l'accueil, son journal n'est plus celui de la fenetre ; sa
    /// file quitte le fil des calculs (le calcul en cours se finit). Ensuite rien de la fenetre n'ecrit plus dans son
    /// dossier, et rien ne la refait apparaitre.
    fn lacher(&self, racine: &Path) {
        let cle = std::fs::canonicalize(racine).unwrap_or_else(|_| racine.to_path_buf());
        if let Phase::Ouverte { racine: r, veille, .. } = self.phase() {
            if std::fs::canonicalize(&r).unwrap_or(r) == cle {
                veille.arreter_et_attendre();
                self.poser_phase(Phase::Accueil);
            }
        }
        // Le journal de la fenetre est celui de la derniere compagnie ouverte, meme revenu a l'accueil : il ne doit plus
        // recreer son dossier.
        let mut journal = self.journal.lock().unwrap_or_else(|e| e.into_inner());
        if journal.as_ref().is_some_and(|j| j.starts_with(racine) || j.starts_with(&cle)) {
            *journal = None;
        }
        drop(journal);
        let file = self.files.lock().unwrap_or_else(|e| e.into_inner()).remove(&cle);
        if let Some(f) = file {
            f.quitter();
        }
    }

    /// Le mot de passe de l'administrateur, tape dans cette fenetre, ouvre-t-il `racine`, compagnie rattachee ?
    fn administrateur_ouvre(&self, racine: &Path) -> bool {
        let inst = compagnies::base();
        if !crate::compagnie::administrateur::est_rattachee(&inst, racine) {
            return false;
        }
        let garde = self.administrateur.lock().unwrap_or_else(|e| e.into_inner()).clone();
        garde.is_some_and(|m| crate::compagnie::administrateur::mot_de_passe_ouvre(&crate::compagnie::administrateur::Poste::de_ce_poste(), racine, &m))
    }

    /// Retient le mot de passe de l'administrateur pour la fenetre, s'il ouvre une compagnie rattachee.
    fn retenir_administrateur(&self, racine: &Path, motdepasse: &str) {
        if crate::compagnie::administrateur::est_rattachee(&compagnies::base(), racine) {
            *self.administrateur.lock().unwrap_or_else(|e| e.into_inner()) = Some(motdepasse.to_string());
        }
    }

    fn ouvrir(&self, racine: PathBuf) {
        // La mise a jour ne se fait pas ici : le logiciel la cherche au lancement (`maj::chercher_en_fond`) et l'applique
        // au lancement suivant, avant toute fenetre (`maj::au_lancement`), sans qu'aucune compagnie soit ouverte.
        // Les etats des mois se refont en fond, dans ce processus, s'ils ne sont plus justes (`etat_garde::FileDesEtats`) :
        // a l'ouverture, apres chaque ecriture et chaque tour de veille. Un mois s'affiche ensuite sans calcul. La file est
        // celle que le fil de la fenetre tient deja pour cette compagnie (`prechauffer`) : elle passe devant.
        let etats = self.file_de(&racine);
        self.fil.devant(Some(&etats));
        etats.tout_verifier();
        let veille = Veille::nouvelle(&racine, self.minutes_veille, Some(etats.clone()));
        veille.demarrer();
        self.poser_phase(Phase::Ouverte { racine, veille, etats });
    }

    fn autorise(&self, r: &Requete) -> bool {
        let fourni = r.params.get("jeton").cloned().or_else(|| r.entete("x-jeton").map(String::from)).or_else(|| r.cookie(&self.nom_du_cookie()));
        let ok = fourni.is_some_and(|f| egal_en_temps_constant(f.as_bytes(), self.jeton.as_bytes()));
        if ok {
            *self.temoin.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        }
        ok
    }

    fn fermer_le_logiciel(&self) {
        if let Some(f) = self.fermer.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f();
        }
    }

    /// LES GESTES DU BANC D'ESSAI (l'atelier seulement ; `atelier/outils/banc/`) : `/atelier/<geste>`, avec le jeton.
    /// - `etat` : le jour simule, le scenario, la compagnie ouverte ;
    /// - `jour` `{"jour": "AAAA-MM-JJ"}` : le calendrier avance jusqu'a ce jour, 9 h (Paris) ; puis, si une compagnie est
    ///   ouverte, un passage de la veille, comme a l'ouverture : l'horloge d'une fictive aux envois simules joue ce qui
    ///   etait du, les retours des depots joues reviennent. Rend ce que la veille a fait ;
    /// - `simuler` : la compagnie ouverte, une fictive ajoutee par le « + », devient une fictive aux envois simules
    ///   (`scenario::rendre_simulee`).
    #[cfg(feature = "atelier")]
    fn atelier(&self, geste: &str, charge: &Value) -> Value {
        let refus = |m: String| json!({"ok": false, "erreur": m});
        let phase = self.phase();
        let ouverte = match &phase {
            Phase::Ouverte { racine, .. } => Some(racine.clone()),
            _ => None,
        };
        let enr = crate::atelier::scenario::enregistreur_actif();
        match geste {
            "etat" => json!({"ok": true,
                "simule": socle::temps::simule(),
                "jour": socle::temps::jour_de_paris().iso(),
                "heure": socle::paris::iso_secondes(socle::temps::secondes()),
                "secondes": socle::temps::secondes(),
                "scenario": enr.map(|e| e.dossier().display().to_string()),
                "compagnie": ouverte.as_ref().map(|r| r.display().to_string()),
                "simulee": ouverte.as_ref().is_some_and(|r| crate::compagnie::bridage::branchement(r).joue()),
                "ajoutee": ouverte.as_ref().is_some_and(|r| compagnies::est_ajoutee(r))}),
            "jour" => {
                if !socle::temps::simule() {
                    return refus("Le temps n'est pas simule : lancer l'interface avec --jour.".into());
                }
                let Some(jour) = charge.get("jour").and_then(Value::as_str).and_then(|j| socle::date::Jour::lire(j).ok()) else {
                    return refus("Il faut {\"jour\": \"AAAA-MM-JJ\"}.".into());
                };
                let instant = socle::temps::instant_de(jour, 9);
                if instant <= socle::temps::secondes() {
                    return refus(format!("Le {} n'est pas apres l'instant simule ({}) : le calendrier n'avance que vers demain.", jour.iso(), socle::paris::iso_secondes(socle::temps::secondes())));
                }
                socle::temps::couler_depuis(instant);
                if let Some(e) = enr {
                    let _ = e.geste("jour", || {
                        let mut m = serde_json::Map::new();
                        m.insert("jour".into(), json!(jour.iso()));
                        // La compagnie ouverte passe sa veille : l'horloge joue dans la route `ouverture` qui suit, et
                        // seulement la (`scenario::rejouer`).
                        if ouverte.is_some() {
                            m.insert("veille".into(), json!(true));
                        }
                        Ok(((), m))
                    });
                }
                self.ecrire(&format!("Banc d'essai : le calendrier avance au {}.\n", jour.iso()));
                match phase {
                    Phase::Ouverte { veille, etats, .. } => {
                        let v = veille.tour(false, false);
                        etats.tout_verifier();
                        json!({"ok": true, "jour": jour.iso(), "veille": v})
                    }
                    _ => json!({"ok": true, "jour": jour.iso()}),
                }
            }
            "simuler" => {
                let Some(racine) = ouverte else { return refus("Aucune compagnie n'est ouverte.".into()) };
                let faire = || crate::atelier::scenario::rendre_simulee(&racine).map(|()| {
                    let mut m = serde_json::Map::new();
                    if let Some(e) = enr {
                        m.insert("compagnie".into(), json!(e.compagnie(&racine)));
                    }
                    ((), m)
                });
                match enr.map_or_else(|| faire().map(|_| ()), |e| e.geste("simuler", faire)) {
                    Ok(()) => {
                        self.ecrire("Banc d'essai : la compagnie devient une fictive aux envois simules.\n");
                        json!({"ok": true})
                    }
                    Err(m) => refus(m),
                }
            }
            _ => refus(format!("Geste d'atelier inconnu : {}.", geste)),
        }
    }

    /// UNE ROUTE DE L'INSTALLATION, demandee par la page (`routes::accueil`). La fenetre n'y ajoute que ce qui la regarde :
    /// avant que les compagnies creees en demo soient gardees ou supprimees, elle lache ce qu'elle en tient (`lacher`) ;
    /// le tableau de l'administrateur se lit, comme l'ecran d'un mois, dans ce que son fil a deja refait (`prechauffer`),
    /// et ne releve les comptes rendus (le reseau, le coffre) qu'une fois le mot de passe de l'administrateur tape ici.
    fn route_de_l_installation(&self, nom: Route, charge: &Value) -> Value {
        let inst = compagnies::base();
        if nom == Route::CompagniesGarder {
            for c in suppression::a_garder(&inst) {
                self.lacher(&c.racine);
            }
        }
        let vrai = |k: &str| charge.get(k).is_some_and(documents::valeur::vrai);
        if nom == Route::AdministrateurTableau && !vrai("garde") && !vrai("relever") {
            if self.administrateur.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                return api::route_de_l_installation(nom, &json!({"relever": true}));
            }
            let rattachees = crate::compagnie::administrateur::rattachees(&inst);
            let par = rattachees.iter().map(|r| (r.clone(), self.file_de(r).tableau().map_err(|e| e.message))).collect();
            return json!({"ok": true, "resultat": crate::compagnie::administrateur::tableau_de(&inst, par)});
        }
        api::route_de_l_installation(nom, charge)
    }

    /// Les gestes de la fenetre elle-meme (`window.pywebview.api.<nom>(...)`, `NOMS_ACCUEIL`, `NOMS_LOGICIEL`).
    fn lanceur(&self, nom: &str, args: &[Value]) -> Value {
        let a = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let ok = |recharger: bool| json!({"ok": true, "recharger": recharger});
        let refus = |m: String| json!({"ok": false, "erreur": m});
        match (nom, self.phase()) {
            ("choisir", Phase::Accueil) => match compagnies::racine_choisie(&compagnies::base(), &a(0)).and_then(|r| self.prendre(r)) {
                Ok(()) => ok(true),
                Err(m) => refus(m),
            },
            ("basculer", Phase::Accueil) => match crate::compagnie::bridage::basculer() {
                Ok(()) => ok(true),
                Err(m) => refus(m),
            },
            ("verifier", Phase::MotDePasse { racine, .. }) => match crate::compagnie::coffre::Coffre::du_poste(&racine, "").verifier_ouverture(&a(0)) {
                Ok(true) => {
                    self.retenir_administrateur(&racine, &a(0));
                    self.ouvrir(racine);
                    ok(true)
                }
                Ok(false) => refus("Mot de passe refusé.".into()),
                Err(e) => refus(e.message),
            },
            ("accueil", _) => {
                self.ecrire("Retour a l'accueil.\n");
                self.poser_phase(Phase::Accueil);
                ok(true)
            }
            ("fermer", _) => {
                self.fermer_le_logiciel();
                ok(false)
            }
            // LES GESTES DU POSTE (`poste.rs`) : ouvrir ce qu'une route a rendu (un chemin), choisir un dossier.
            ("ouvrir", Phase::Ouverte { racine, .. }) => {
                match poste::ouvrir_pour_la_page(&racine, &a(0), args.get(1).and_then(Value::as_bool) == Some(true)) {
                    Ok(phrase) => json!({"ok": true, "phrase": phrase}),
                    Err(m) => refus(m),
                }
            }
            ("choisir_dossier", Phase::Ouverte { .. }) => match poste::choisir_dossier(&a(0)) {
                Ok(d) => json!({"ok": true, "dossier": d}),
                Err(m) => refus(m),
            },
            // LA VEILLE DE LA FENETRE : son etat, l'etape du passage en cours, un passage tout de suite (un passage appelle
            // la route `ouverture`, qui releve et sauvegarde).
            ("veille", Phase::Ouverte { veille, .. }) => json!({"ok": true, "etat": veille.etat()}),
            ("veille_etape", Phase::Ouverte { veille, .. }) => json!({"ok": true, "etat": veille.etape()}),
            ("veille_maintenant", Phase::Ouverte { veille, .. }) => json!({"ok": true, "etat": veille.tour(false, false)}),
            // Ou en est la recherche de la mise a jour : l'accueil et la page d'une compagnie l'affichent.
            ("maj", _) => json!({"ok": true, "etat": crate::compagnie::maj::RECHERCHE.etat()}),
            // Le site de l'editeur, dans le navigateur du poste : la fenetre ne quitte jamais sa page.
            ("site", _) => match poste::ouvrir_site(&a(0)) {
                Ok(()) => ok(false),
                Err(m) => refus(m),
            },
            // « Redemarrer pour mettre a jour » : la version prete part au relais, puis le logiciel se ferme comme par sa
            // croix (la veille s'arrete) ; le relais attend sa fin, installe et rouvre la fenetre neuve. Rien de pret :
            // refuse, et rien ne se ferme.
            ("redemarrer", _) => match crate::compagnie::maj::redemarrer_pour_mettre_a_jour(&crate::compagnie::maj::installation(), &crate::compagnie::maj::RECHERCHE) {
                Ok(v) => {
                    self.ecrire(&format!("Redemarrage pour la mise a jour {}.\n", crate::compagnie::maj::version_lue(&v)));
                    self.poser_phase(Phase::Accueil);
                    self.fermer_le_logiciel();
                    ok(false)
                }
                Err(m) => refus(m),
            },
            _ => refus(format!("Geste inconnu ici : {}.", nom)),
        }
    }

    /// Une requete de la fenetre.
    pub fn traiter(&self, r: Requete) -> Reponse {
        let texte = |code: u16| Reponse::nouvelle(code, "text/plain; charset=utf-8", Vec::new());
        if r.methode == "POST" {
            // Les gestes de l'accueil lisent `erreur` en phrase (`err.textContent = r.erreur`) ; les routes de la page,
            // en objet (`d.erreur.message`). Un refus du serveur parle a chacun sa langue.
            let geste = r.chemin.starts_with("/lanceur/");
            let refus = |code: u16, genre: &str, message: String| {
                let erreur = if geste { json!(message) } else { json!({"type": genre, "message": message}) };
                Reponse::json(code, &json!({"ok": false, "erreur": erreur}))
            };
            if !self.autorise(&r) {
                return if geste {
                    refus(403, "NonAutorise", "Cette page n'est plus reconnue par PuppetPaie (jeton absent) : fermez la fenêtre et relancez le logiciel.".into())
                } else {
                    refus(403, "NonAutorise", "jeton absent".into())
                };
            }
            let charge: Value = match serde_json::from_slice(if r.corps.is_empty() { b"{}" } else { &r.corps }) {
                Ok(v) => v,
                Err(e) => return refus(400, "JSONInvalide", e.to_string()),
            };
            if let Some(nom) = r.chemin.strip_prefix("/lanceur/") {
                let args = charge.as_array().cloned().unwrap_or_default();
                return Reponse::json(200, &self.lanceur(nom, &args));
            }
            #[cfg(feature = "atelier")]
            if let Some(nom) = r.chemin.strip_prefix("/atelier/") {
                return Reponse::json(200, &self.atelier(nom, &charge));
            }
            if let Some(texte) = r.chemin.strip_prefix("/api/") {
                // LE NOM DE LA ROUTE SE LIT ICI, UNE FOIS : un nom qui n'est celui d'aucune route se refuse.
                let Some(nom) = Route::lire(texte) else {
                    let mut refus = api::Refus::simple("CalculRefuse", crate::routes::catalogue::ROUTE_INCONNUE);
                    refus.plus.insert("route".into(), json!(texte));
                    return Reponse::json(200, &api::en_page(Err(refus)));
                };
                // LA SEULE PORTE DE LA PAGE VERS LE MOTEUR : une route du catalogue. Celle de l'installation se sert a tout
                // moment ; celle d'une compagnie, quand la compagnie est ouverte -- ou quand elle n'a pas encore de mot de
                // passe (rien a deverrouiller : la page du premier mot de passe le pose par sa route).
                // Ecrire au developpeur se sert aussi a l'accueil, aucune compagnie ouverte (`routes::filet::sans_compagnie`).
                // Le catalogue (`routes`) ne porte sur aucune compagnie : l'accueil le lit pour son « Avancé » (`js/avance.js`).
                let au_developpeur = crate::routes::catalogue::dans(crate::routes::filet::ROUTES, nom).is_some() && !matches!(self.phase(), Phase::Ouverte { .. });
                let catalogue = crate::routes::catalogue::dans(crate::routes::catalogue::ROUTES, nom).is_some();
                if crate::routes::catalogue::dans(crate::routes::accueil::ROUTES, nom).is_some() || au_developpeur || catalogue {
                    let debut = Instant::now();
                    let rendu = self.route_de_l_installation(nom, &charge);
                    self.ecrire(&ligne_de_route(nom, debut.elapsed(), &rendu));
                    return Reponse::json(200, &rendu);
                }
                let (racine, veille, etats) = match self.phase() {
                    Phase::Ouverte { racine, veille, etats } => (racine, Some(veille), Some(etats)),
                    Phase::MotDePasse { racine, premiere: true } => (racine, None, None),
                    _ => return Reponse::json(200, &json!({"ok": false, "erreur": {"type": "ErreurOperation", "message": "Aucune compagnie n'est ouverte."}})),
                };
                // LE JOURNAL DE L'INTERFACE note chaque route et sa duree (de quoi mesurer l'ouverture sur le poste) :
                // l'heure, le nom, les millisecondes, et la classe d'un refus. Jamais la charge : elle peut porter un mot
                // de passe.
                // CE QUE LA PAGE A FAIT depuis sa derniere demande (`X-Gestes`, js/tuyau.js : clics, champs, boites, console),
                // note dans le fil AVANT la route qu'il a declenchee (`diagnostic::noter_gestes`, qui le met en forme).
                if let Some(g) = r.entete("x-gestes").filter(|g| !g.is_empty()) {
                    if let Ok(lot) = serde_json::from_str::<Value>(&http::decoder_pourcent(g)) {
                        crate::compagnie::diagnostic::noter_gestes(&racine, &lot);
                    }
                }
                // SUPPRIMER LA COMPAGNIE OUVERTE (`compagnie-supprimer` confirmee) : la route decide tout
                // (`suppression::route`) ; la fenetre lache d'abord ce qu'elle tient de la compagnie (sa veille, sa file
                // des etats, son journal), et ne note rien apres dans un dossier qui n'est plus. Un passage de la veille en
                // cours ecrit dans ce dossier : on attend qu'il finisse.
                if nom == Route::CompagnieSupprimer && charge.get("confirme").is_some_and(documents::valeur::vrai) {
                    let rendu = if veille.as_ref().is_some_and(|v| v.occupee()) {
                        Err(api::Refus::simple("ErreurOperation", "Un relevé ou une sauvegarde de cette compagnie est en cours : attendez qu'il se termine, puis recommencez. Rien n'a été supprimé."))
                    } else {
                        suppression::route(&racine, &charge, &|r| self.lacher(r)).map_err(|m| api::Refus::simple("ErreurOperation", m))
                    };
                    if rendu.is_ok() {
                        self.ecrire(&format!("Compagnie supprimee : {}.\n", racine.display()));
                    }
                    return Reponse::json(200, &api::en_page(rendu));
                }
                let debut = Instant::now();
                let rendu = api::repondre(&racine, nom, &charge, veille.as_deref(), etats.as_deref());
                self.ecrire(&ligne_de_route(nom, debut.elapsed(), &rendu));
                return Reponse::json(200, &rendu);
            }
            return Reponse::json(404, &json!({"ok": false, "erreur": {"type": "Inconnue", "message": "route inconnue"}}));
        }
        match r.chemin.as_str() {
            "/" | "/index.html" => {
                if !self.autorise(&r) {
                    return Reponse::nouvelle(403, "text/plain; charset=utf-8", "jeton absent ou invalide");
                }
                if r.params.contains_key("jeton") && r.chemin == "/" {
                    // Le jeton passe en cookie, et l'adresse redevient propre.
                    return Reponse::nouvelle(303, "text/plain", Vec::new())
                        .avec("Location", "/")
                        .avec("Set-Cookie", format!("{}={}; Path=/; HttpOnly; SameSite=Strict", self.nom_du_cookie(), self.jeton));
                }
                let html = match self.phase() {
                    Phase::Accueil => page(&page_accueil(), &self.jeton, NOMS_ACCUEIL),
                    Phase::MotDePasse { racine, premiere } => {
                        // Ce que l'administrateur est pour cette compagnie (la route `administrateur-etat`) : la page le lit,
                        // sans rien decider.
                        let etat = crate::routes::coffre::administrateur_etat(&racine);
                        let gabarit = if premiere { CHOIX_MOT_DE_PASSE } else { MOT_DE_PASSE };
                        page(&gabarit.replace("{{ADMINISTRATEUR}}", &json_dans_la_page(&etat)), &self.jeton, NOMS_ACCUEIL)
                    }
                    Phase::Ouverte { racine, .. } => match page_logiciel(&racine, &self.jeton) {
                        Ok(p) => p,
                        Err(m) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", m),
                    },
                };
                Reponse::nouvelle(200, "text/html; charset=utf-8", html)
            }
            "/fictive/page" => self.piece_fictive(&r),
            "/apercu/bulletin" => self.apercu_bulletin(&r),
            c if c.starts_with("/identite/") => self.identite(&r, &c["/identite/".len()..]),
            c => match fichier_de_la_page(c) {
                // UNE FEUILLE OU UN SCRIPT NOMME PAR SON EMPREINTE (`?v=`, `empreinte`) se garde : la page
                // d'une compagnie charge vingt-cinq fichiers, qu'elle ne redemande plus a chaque ouverture. Ce qui change
                // change d'empreinte, donc d'adresse : rien de perime ne se garde.
                Some((o, mime)) if r.params.contains_key("v") && (c.ends_with(".js") || c.ends_with(".css")) => {
                    Reponse::nouvelle(200, mime, o).avec("Cache-Control", "max-age=31536000, immutable")
                }
                Some((o, mime)) => Reponse::nouvelle(200, mime, o),
                None => texte(404),
            },
        }
    }

    /// Le PDF joint a un courriel joue d'une compagnie fictive, tel quel (le lecteur de la fenetre l'affiche).
    fn piece_fictive(&self, r: &Requete) -> Reponse {
        let interdit = Reponse::nouvelle(403, "text/plain", Vec::new());
        let Phase::Ouverte { racine, .. } = self.phase() else { return interdit };
        if !self.autorise(r) || !crate::compagnie::bridage::branchement(&racine).joue() {
            return interdit;
        }
        let nom = r.params.get("c").cloned().unwrap_or_default();
        let rang: usize = r.params.get("p").and_then(|p| p.parse().ok()).unwrap_or(0);
        let Some(brut) = envois::simulation_courriels::courriels(&racine).into_iter().find(|(n, _)| *n == nom).map(|(_, o)| o) else {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        };
        let Ok(m) = mailparse::parse_mail(&brut) else { return Reponse::nouvelle(404, "text/plain", Vec::new()) };
        let mut feuilles = Vec::new();
        envois::simulation_courriels::feuilles(&m, &mut feuilles);
        let pieces: Vec<_> = feuilles.into_iter().filter(|f| envois::simulation_courriels::nom_de_fichier(f).is_some()).collect();
        let Some(piece) = pieces.get(rang) else { return Reponse::nouvelle(404, "text/plain", Vec::new()) };
        let octets = piece.get_body_raw().unwrap_or_default();
        if piece.ctype.mimetype != "application/pdf" || !octets.starts_with(b"%PDF") {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        }
        let nom_pdf = envois::simulation_courriels::nom_de_fichier(piece).unwrap_or_else(|| "piece.pdf".into());
        let sur: String = nom_pdf.chars().map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
        Reponse::nouvelle(200, "application/pdf", octets)
            .avec("Content-Disposition", format!("inline; filename=\"{}\"", sur))
            .avec("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'self'")
    }

    /// L'apercu d'un modele de bulletin : le PDF d'une paie fictive, que la route `bulletin-modele-apercu` fait dans le
    /// processus de la fenetre, comme toute route ; le lecteur de la fenetre l'affiche (Configuration).
    fn apercu_bulletin(&self, r: &Requete) -> Reponse {
        let interdit = Reponse::nouvelle(403, "text/plain", Vec::new());
        let Phase::Ouverte { racine, .. } = self.phase() else { return interdit };
        if !self.autorise(r) {
            return interdit;
        }
        let demande = r.params.get("modele").map(String::as_str).unwrap_or_default();
        let Some(modele) = crate::routes::bulletin::modele_de_l_adresse(demande) else {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        };
        // La route, comme la page l'appellerait : sa charge et sa reponse lues par leur forme.
        let charge = crate::routes::bulletin::ChargeModele { modele: modele.cle().to_string() };
        let pdf = serde_json::to_value(charge)
            .ok()
            .and_then(|c| api::route(&racine, Route::BulletinModeleApercu, &c).ok())
            .and_then(|v| crate::routes::lire_charge::<crate::routes::bulletin::ApercuModele>(&v).ok())
            .and_then(|a| socle::base64::lire(&a.pdf).ok());
        match pdf {
            Some(octets) if octets.starts_with(b"%PDF") => Reponse::nouvelle(200, "application/pdf", octets)
                .avec("Content-Disposition", format!("inline; filename=\"apercu-bulletin-{}.pdf\"", if modele == documents::bulletin::Modele::Complet { "complet" } else { "simplifie" }))
                .avec("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'self'"),
            _ => Reponse::nouvelle(500, "text/plain; charset=utf-8", "L'aperçu du bulletin ne s'est pas fait."),
        }
    }

    /// L'image posee pour la signature, le logo ou le cachet ; ou le cachet que le logiciel dessine.
    fn identite(&self, r: &Requete, quoi: &str) -> Reponse {
        let rien = || Reponse::nouvelle(404, "text/plain", Vec::new());
        if !self.autorise(r) {
            return Reponse::nouvelle(403, "text/plain", Vec::new());
        }
        let Phase::Ouverte { racine, .. } = self.phase() else { return rien() };
        if quoi == "cachet-automatique" {
            let ent = droit::contexte::Contexte::nouveau(&racine, None).map(|c| c.entreprise).unwrap_or_default();
            return match engagement::cachet::svg(&ent) {
                Some(d) => Reponse::nouvelle(200, "image/svg+xml; charset=utf-8", d),
                None => rien(),
            };
        }
        if !["cachet", "logo", "signature"].contains(&quoi) {
            return rien();
        }
        let dossier = racine.join("gabarits").join("identite");
        for (ext, mime) in [(".png", "image/png"), (".jpg", "image/jpeg"), (".svg", "image/svg+xml")] {
            if let Ok(o) = std::fs::read(dossier.join(format!("{}{}", quoi, ext))) {
                return Reponse::nouvelle(200, mime, o);
            }
        }
        rien()
    }

    /// L'adresse que la fenetre charge (avec le jeton, qui passe aussitot en cookie).
    pub fn adresse(&self, hote: &str, port: u16) -> String {
        format!("http://{}:{}/?jeton={}", hote, port, self.jeton)
    }
}

/// La ligne du journal de l'interface pour une route servie : `HH:MM:SS route NOM N ms`, et ` refus CLASSE` quand elle a
/// refuse. Ni la charge ni la reponse : seulement de quoi mesurer.
pub fn ligne_de_route(nom: Route, duree: Duration, rendu: &Value) -> String {
    let heure = socle::paris::iso_de_naif(socle::paris::maintenant_naif());
    let refus = match rendu.get("ok").and_then(Value::as_bool) {
        Some(true) => String::new(),
        _ => format!(" refus {}", rendu.pointer("/erreur/type").and_then(Value::as_str).unwrap_or("?")),
    };
    format!("{} route {} {} ms{}\n", heure.get(11..).unwrap_or(""), nom, duree.as_millis(), refus)
}

fn egal_en_temps_constant(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// AU LANCEMENT, avant tout : la livraison de fictives prete (`maj::poser_les_fictives_pretes`), et le reste cache d'une
/// compagnie supprimee (`suppression::effacer_les_restes`). Ce qui s'est passe va dans le journal de l'installation
/// (`maj::noter`).
pub fn preparer_l_installation() {
    let inst = crate::compagnie::maj::installation();
    // Une livraison de compagnies fictives prete (`maj::preparer_les_fictives`) remplace celles du poste, avant toute fenetre.
    if let Some(t) = crate::compagnie::maj::poser_les_fictives_pretes(&inst) {
        crate::compagnie::maj::noter(&inst, &t);
    }
    // Une compagnie supprimee dont un fichier resistait (`suppression::effacer`) : son reste cache part maintenant.
    let restes = suppression::effacer_les_restes(&inst);
    if !restes.is_empty() {
        let noms: Vec<String> = restes.iter().map(|p| p.display().to_string()).collect();
        crate::compagnie::maj::noter(&inst, &format!("Restes de compagnies supprimees effaces : {}.", noms.join(", ")));
    }
}

pub use instance::{Instance, Prise};

/// LE VERROU DE CETTE INSTALLATION (`instance.rs`) : un creneau par installation -- son verrou et le port de sa page.
/// La meme installation relancee : `Devant` (sa fenetre est revenue devant) ; une autre installation ouverte sur le poste
/// ne change rien, chacune a son creneau.
pub fn prendre_instance(devant: impl Fn() + Send + 'static) -> Instance {
    instance::prendre(devant)
}

/// Demarre le serveur de la fenetre dans un fil, sur l'ecoute de la page que le verrou a prise (`Prise`, que l'appelant
/// garde tant que la fenetre vit : elle porte le verrou). Rend la fenetre et son port.
pub fn demarrer(minutes_veille: u64, prise: &Prise) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let page = prise.page.try_clone()?;
    http::sans_heritage(&page);
    servir_sur(minutes_veille, page, prise.port)
}

/// Demarre le serveur de la fenetre dans un fil, au port `port` s'il est libre (sinon un port que le systeme donne : la
/// ligne « la fenetre ecoute sur » dit lequel). Rend la fenetre et le port. Hors du verrou : `moteur interface --port`, et
/// les essais.
pub fn demarrer_sur(minutes_veille: u64, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let (ecouteur, port) = http::ecouter(port)?;
    servir_sur(minutes_veille, ecouteur, port)
}

fn servir_sur(minutes_veille: u64, ecouteur: TcpListener, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    // L'heure du lancement et les paniques notees (`diagnostic`) : ce qu'un retour et un incident disent de la fenetre.
    crate::compagnie::diagnostic::installer();
    let f = Fenetre::nouvelle(minutes_veille, instance::dossier_local(&compagnies::base(), port), port);
    let servie = f.clone();
    std::thread::spawn(move || http::servir(ecouteur, Arc::new(move |r| servie.traiter(r))));
    f.ecrire(&format!("PuppetPaie {} : la fenetre ecoute sur 127.0.0.1:{}.\n", version(), port));
    Ok((f, port))
}

/// Ce que `moteur interface` a lu de sa ligne de commande. `port` : celui qu'on impose ; sans lui, celui de l'installation
/// (`instance.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub port: Option<u16>,
    pub minutes_veille: u64,
    pub sans_navigateur: bool,
}

const USAGE: &str = "Usage : moteur interface [--port N] [--sans-navigateur] [--veille MINUTES]";

/// Les options de `moteur interface`. Une valeur qui ne se lit pas est refusee, jamais remplacee par le defaut.
pub fn lire_options(args: &[String]) -> Result<Options, String> {
    let mut o = Options { port: None, minutes_veille: MINUTES_VEILLE, sans_navigateur: false };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sans-navigateur" => {
                o.sans_navigateur = true;
                i += 1;
            }
            "--port" if i + 1 < args.len() => {
                o.port = Some(args[i + 1].parse().ok().filter(|p| *p > 0).ok_or_else(|| format!("Port illisible : {}. {}", args[i + 1], USAGE))?);
                i += 2;
            }
            "--veille" if i + 1 < args.len() => {
                o.minutes_veille = args[i + 1].parse().map_err(|_| format!("Duree de veille illisible : {}. {}", args[i + 1], USAGE))?;
                i += 2;
            }
            autre => return Err(format!("Option inconnue : {}. {}", autre, USAGE)),
        }
    }
    Ok(o)
}

/// LE BANC D'ESSAI (l'atelier seulement ; `atelier/outils/banc/`) : `moteur-atelier interface ... [--enregistrer
/// DOSSIER [--lectures]] [--jour AAAA-MM-JJ]`. `--jour` : le temps du moteur coule depuis ce jour, 9 h a Paris (`temps`) ;
/// `--enregistrer` : chaque geste qui ecrit entre dans le scenario `DOSSIER` (`scenario::Enregistreur`), les lectures
/// aussi avec `--lectures`. Un scenario deja commence se poursuit : sans `--jour`, le temps reprend une minute apres son dernier geste ; un `--jour` avant lui est
/// refuse. Rend les autres options, pour `lire_options`.
#[cfg(feature = "atelier")]
fn poser_le_banc(args: &[String]) -> Result<Vec<String>, String> {
    let (mut reste, mut dossier, mut jour, mut lectures) = (Vec::new(), None, None, false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--enregistrer" if i + 1 < args.len() => {
                dossier = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--lectures" => {
                lectures = true;
                i += 1;
            }
            "--jour" if i + 1 < args.len() => {
                jour = Some(socle::date::Jour::lire(&args[i + 1]).map_err(|_| format!("Jour illisible : {} (AAAA-MM-JJ).", args[i + 1]))?);
                i += 2;
            }
            a => {
                reste.push(a.to_string());
                i += 1;
            }
        }
    }
    let enr = match &dossier {
        Some(d) => Some(crate::atelier::scenario::Enregistreur::demarrer(d, &compagnies::base(), lectures)?),
        None => None,
    };
    let dernier = enr.and_then(|e| e.dernier_instant());
    let depart = match (jour, dernier) {
        (Some(j), Some(t)) if socle::temps::instant_de(j, 9) <= t => {
            return Err(socle::echec::avec(format!("Le scénario va déjà jusqu'au {} : ce jour le ferait reculer.", socle::paris::iso_secondes(t)), serde_json::json!({"jour": j.iso()})))
        }
        (Some(j), _) => Some(socle::temps::instant_de(j, 9)),
        (None, Some(t)) => Some(t + 60.0),
        (None, None) => None,
    };
    if let Some(t) = depart {
        socle::temps::couler_depuis(t);
    }
    if let Some(e) = enr {
        // LES ALEAS DU BANC sont ceux du rejeu (`temps::aleas`) : la compagnie du banc et son rejeu s'ecrivent pareil, et
        // `scenario::verifier` le controle. Le banc n'est jamais un poste.
        socle::temps::aleas_fixes();
        e.geste("debut", || {
            let mut m = serde_json::Map::new();
            m.insert("jour".into(), json!(socle::temps::jour_de_paris().iso()));
            m.insert("logiciel".into(), json!(version()));
            m.insert("format".into(), json!(crate::atelier::scenario::FORMAT));
            Ok(((), m))
        })?;
    }
    Ok(reste)
}

/// `moteur interface [--port N] [--sans-navigateur] [--veille MINUTES]` : la fenetre dans le navigateur du poste.
pub fn commande(args: &[String]) -> i32 {
    crate::commandes::compagnie::fixer_le_code();
    crate::compagnie::incidents::brancher_l_engagement();
    #[cfg(feature = "atelier")]
    let args = &match poser_le_banc(args) {
        Ok(reste) => reste,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let options = match lire_options(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let minutes = options.minutes_veille;
    let devant: Arc<Mutex<Option<(String, PathBuf)>>> = Arc::new(Mutex::new(None));
    let pour_devant = devant.clone();
    let instance = prendre_instance(move || {
        if let Some((a, profil)) = pour_devant.lock().ok().and_then(|a| a.clone()) {
            navigateur::ouvrir(&a, &profil, &|_| {});
        }
    });
    let prise = match instance {
        Instance::Tenue(p) => p,
        Instance::Devant => {
            dire!("PuppetPaie est deja ouvert depuis cette installation : sa fenetre revient devant.");
            return 0;
        }
    };
    // La mise a jour se cherche au lancement, pour l'installation (`maj::chercher_en_fond`) ; elle s'appliquera au
    // prochain lancement de la fenetre.
    preparer_l_installation();
    crate::compagnie::maj::chercher_en_fond();
    let demarre = match options.port {
        Some(p) => demarrer_sur(minutes, p),
        None => demarrer(minutes, &prise),
    };
    let (f, port) = match demarre {
        Ok(x) => x,
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}.", e);
            return 1;
        }
    };
    // Les compagnies de l'installation se tiennent a jour en fond, des maintenant, sans rien ouvrir (`prechauffer`).
    f.prechauffer();
    *f.fermer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(|| {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            std::process::exit(0);
        });
    }));
    let url = f.adresse("puppetpaie.localhost", port);
    *devant.lock().unwrap_or_else(|e| e.into_inner()) = Some((url.clone(), f.dossier_local().join("navigateur")));
    dire!("{}", url);
    let navigateur = if options.sans_navigateur { None } else { navigateur::ouvrir(&url, &f.dossier_local().join("navigateur"), &|t| f.ecrire(t)) };
    // Le navigateur a nous : sa fermeture arrete PuppetPaie. Sinon, plus aucune page depuis cinq minutes l'arrete
    // (la page interroge la veille chaque minute).
    if let Some(mut p) = navigateur {
        let debut = Instant::now();
        let _ = p.wait();
        if debut.elapsed() > Duration::from_secs(5) {
            return 0;
        }
    }
    if options.sans_navigateur {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
    loop {
        std::thread::sleep(Duration::from_secs(5));
        if f.silence() > Duration::from_secs(5 * 60) {
            return 0;
        }
    }
}

/// CE QUE LA PAGE DIT QUAND LE MOTEUR NE PEUT PLUS LE DIRE : le serveur qui ne repond plus, une reponse illisible. La
/// fenetre les pose dans la page qu'elle sert (`{{PHRASES}}` de `index.html`), la page n'en ecrit aucune.
static PHRASES_DE_LA_FENETRE: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
    serde_json::json!({
        "perdu": "Le serveur ne répond plus : la fenêtre PuppetPaie a été fermée, ou le terminal qui la faisait tourner. Rien de ce qui est à l'écran n'est enregistré. Relancer PuppetPaie, puis recharger cette page.",
        "perdu_court": "Le serveur PuppetPaie ne répond plus. Rien n'a été enregistré.",
        "illisible": "PuppetPaie a répondu sans dire pourquoi. Rien n'est sûr d'avoir été enregistré.",
        "lecture_absente": "Lecture absente de l'état du mois.",
    })
});

/// CE QUE DISENT L'ACCUEIL DES COMPAGNIES ET LES PAGES DU MOT DE PASSE, que la fenetre sert (`pages/`) : chacune a sa
/// place dans le gabarit (`{{P:cle}}`), y compris dans ses scripts (sans guillemet droit ni retour a la ligne).
const PHRASES_DE_L_ACCUEIL: &[(&str, &str)] = &[
    ("ouvre_pas", "La compagnie ne s'ouvre pas."),
    ("zero_confirmer", "remettre à zéro ? Tout ce qui a été fait dans cette compagnie fictive sera effacé."),
    ("toutes_faites", "Toutes les déclarations de vos compagnies sont faites."),
    ("tableau_illisible", "Le tableau de cette compagnie ne se lit pas"),
    ("administrateur_seul", "un seul mot de passe, celui de l'administrateur."),
    ("chaque_dsn", "Chaque DSN part avec ses contrôles et son bilan, compagnie par compagnie. Le mot de passe de l'administrateur le confirme."),
    ("mdp_administrateur", "le mot de passe de l'administrateur"),
    ("rien_parti", "Rien n'est parti."),
    ("rien_change", "Rien n'a changé."),
    ("avis_ajouter", "Vous pouvez ajouter votre compagnie et regarder le logiciel avec vos vraies données. Pour 2027, et afin d'offrir un service de qualité, le nombre d'utilisateurs est restreint. N'hésitez pas à contacter Léandre"),
    ("avis_ajouter_suite", "avec une courte présentation et votre motivation pour faire partie des premiers utilisateurs."),
    ("meme_administrateur", "Gérer cette compagnie avec le même administrateur ?"),
    ("ouvre_deja", "Le mot de passe qui ouvre déjà"),
    ("non_a_elle", "Non, un mot de passe à elle"),
    ("tapez_pour", "Le mot de passe que vous tapez pour"),
    ("ouvrira_aussi", "ouvrira aussi cette compagnie, et il servira aux actes sur toutes à la fois, comme déposer d'un coup toutes les DSN prêtes."),
    ("choisissez", "Choisissez le mot de passe du logiciel"),
    ("demande_ouverture", "Il sera demandé à chaque ouverture et à chaque dépôt réel. Ce n'est pas celui d'un organisme."),
    ("retour_choix", "Revenir au choix de la compagnie"),
    ("garder_titre", "Vous venez de débloquer le logiciel. Voulez-vous garder les compagnies créées ?"),
    ("garder_note", "Une compagnie gardée devient une compagnie réelle : son mot de passe du logiciel se choisit à sa première ouverture. Une compagnie décochée est supprimée, avec tout son dossier."),
    ("mdp_logiciel", "Mot de passe du logiciel"),
    ("mdp_redemande", "Celui du logiciel, pas celui d'un organisme. Il sera redemandé à chaque dépôt."),
    ("mdp_titre_admin", "Mot de passe de l'administrateur"),
    ("neuve", "Elle commence ses paies ici : vous la configurez, puis vous engagez vos salariés."),
    ("precedent", "Vous faisiez vos paies ailleurs : vous mettez dans un dossier ce que votre ancien logiciel vous a laissé (bulletins, déclarations), et PuppetPaie se paramètre avec."),
    ("zero", "Remettre à zéro : tout ce qui a été fait ici s'efface"),
];
