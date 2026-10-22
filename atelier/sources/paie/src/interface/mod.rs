//! La fenetre de PuppetPaie, en Rust (lot J3) : ce que faisaient `web.py`, `lanceur_interface.pyw`, `fenetre.py`,
//! `compagnies.py` et `instance.py`.
//!
//! UN SERVEUR LOCAL, TROIS ETATS. Le serveur (127.0.0.1, un jeton exige a chaque appel) sert `/` selon l'etat :
//!   - l'ACCUEIL : les tuiles des compagnies ;
//!   - le MOT DE PASSE de la compagnie choisie (ou son choix, a la premiere ouverture) ; une fictive n'en a pas ;
//!   - la COMPAGNIE OUVERTE : la page du logiciel (`moteur/interface/`), ses appels `/api/<route>` (`api.rs`) et la veille.
//! Les pages de l'accueil appellent `window.pywebview.api.<nom>(...)`, comme dans la fenetre pywebview : une cale
//! (`CALE`) poste ces appels a `/lanceur/<nom>`. La page ne sait pas dans quoi elle tourne.
//!
//! LA FENETRE. Tauri (`fenetre/`, le binaire `puppetpaie`) montre `adresse()` dans une vraie fenetre ; `paie interface`
//! l'ouvre dans un Chromium en mode application, sinon dans le navigateur par defaut (`navigateur.rs`).

pub mod api;
pub mod compagnies;
pub mod http;
pub mod instance;
pub mod navigateur;
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

pub const MINUTES_VEILLE: u64 = 20;

const MOT_DE_PASSE: &str = include_str!("pages/mot_de_passe.html");
const CHOIX_MOT_DE_PASSE: &str = include_str!("pages/choix_mot_de_passe.html");
const ACCUEIL: &str = include_str!("pages/accueil.html");
const CRAYON: &str = include_str!("pages/crayon.html");
const ZERO: &str = include_str!("pages/zero.html");
const TUILE_AJOUTER: &str = include_str!("pages/tuile_ajouter.html");

/// Les appels `window.pywebview.api.<nom>(...)` de l'accueil, postes au serveur. Une reponse qui dit `recharger`
/// fait revenir la fenetre sur `/` : c'est le serveur qui sait quelle page montrer ensuite.
const CALE: &str = r#"<script>
(function () {
  const JETON = "{{JETON}}";
  // Une reponse se lit toujours en `{ok, erreur}`, `erreur` en phrase (#141) : un corps qui n'est pas du JSON, un
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
  document.addEventListener("DOMContentLoaded", () => window.dispatchEvent(new Event("pywebviewready")));
})();
</script>
"#;

/// Les pages du mot de passe sont servies avec ces noms : leur « Retour » (et Echap) appelle `accueil`. Absent de la
/// liste, la cale ne le definissait pas et le bouton levait sans rien faire (capture de Leandre, version 20.10.2026).
/// L'administrateur (ticket #244) : `rattacher_nouvelle` (la question « meme administrateur ? » d'une compagnie neuve),
/// `tableau`, `tableau_rafraichir` et `deposer_groupe` (le tableau de bord de l'accueil).
const NOMS_ACCUEIL: &str = r#"["choisir", "couleur", "basculer", "remettre_a_zero", "ajouter", "verifier", "poser", "accueil", "fermer", "maj", "redemarrer", "rattacher_nouvelle", "tableau", "tableau_rafraichir", "deposer_groupe"]"#;
/// Dans la page du logiciel : revenir aux compagnies (ticket #113) ; et, comme a l'accueil, lire ou en est la mise a
/// jour et redemarrer pour l'installer (ticket #210, `js/mise-a-jour.js`). L'administrateur, dans la Configuration
/// (ticket #244, `js/coffre-compte.js`) : ce qu'il est pour cette compagnie, la rattacher, la detacher, changer son mot
/// de passe.
const NOMS_LOGICIEL: &str = r#"["accueil", "maj", "redemarrer", "administrateur", "rattacher", "detacher", "changer_administrateur"]"#;

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
    /// LE FIL DES CALCULS EN FOND (ticket #244, `etat_garde::Fil`) et la file de chaque compagnie qu'il tient, par
    /// dossier : nees au lancement pour les compagnies de l'installation (`prechauffer`), ou a l'ouverture d'une autre ;
    /// reprises, jamais refaites, quand la compagnie s'ouvre, se ferme et se rouvre.
    fil: Arc<Fil>,
    files: Mutex<HashMap<PathBuf, Arc<FileDesEtats>>>,
    phase: Mutex<Phase>,
    /// LE MOT DE PASSE DE L'ADMINISTRATEUR, tape une fois dans cette fenetre (ticket #244) : il ouvre ses autres
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
fn version() -> String {
    crate::maj::version_lue(&std::fs::read_to_string(compagnies::logiciel().join("VERSION")).unwrap_or_else(|_| "0.0.0".into()))
}

fn ressources() -> PathBuf {
    compagnies::logiciel().join("interface")
}

/// Douze caracteres qui changent des qu'une feuille ou un script change : la fenetre ne garde rien de perime.
/// Tous les `.css` et `.js` de `interface/` et de `interface/js/` (lot L), tries par nom : la meme empreinte d'un
/// lancement a l'autre. Le nom entre dans le condensat avec le contenu.
fn empreinte() -> String {
    use sha2::{Digest, Sha256};
    let mut noms: Vec<String> = Vec::new();
    for dossier in ["", "js"] {
        let Ok(it) = std::fs::read_dir(ressources().join(dossier)) else { continue };
        for e in it.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_file()) && (nom.ends_with(".css") || nom.ends_with(".js")) {
                noms.push(if dossier.is_empty() { nom } else { format!("{}/{}", dossier, nom) });
            }
        }
    }
    noms.sort();
    let mut h = Sha256::new();
    for nom in &noms {
        if let Ok(o) = std::fs::read(ressources().join(nom)) {
            h.update(nom.as_bytes());
            h.update([0u8]);
            h.update((o.len() as u64).to_le_bytes());
            h.update(&o);
        }
    }
    h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect()
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
fn json_dans_la_page(v: &Value) -> String {
    v.to_string().replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

fn page(corps: &str, jeton: &str, noms: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>PuppetPaie</title>\n{}</head><body>\n{}\n</body></html>",
        CALE.replace("{{JETON}}", jeton).replace("{{NOMS}}", noms),
        corps
    )
}

fn logo_en_ligne(racine: &Path) -> String {
    match compagnies::logo(racine) {
        Some(p) => {
            let mime = if p.extension().is_some_and(|e| e == "svg") { "image/svg+xml" } else { "image/png" };
            std::fs::read(&p).map(|o| format!("data:{};base64,{}", mime, crate::retours::base64_ecrire(&o))).unwrap_or_default()
        }
        None => String::new(),
    }
}

fn page_accueil() -> String {
    let choix = compagnies::liste();
    let mut tuiles = Vec::new();
    for c in &choix {
        let logo = logo_en_ligne(&c.racine);
        let medaillon = if logo.is_empty() {
            format!("<span class=\"initiale\">{}</span>", echapper(&c.nom.chars().take(1).collect::<String>()))
        } else {
            format!("<img src=\"{}\" alt=\"\">", logo)
        };
        tuiles.push((
            c.reelle,
            format!(
                "<div class=\"tuile{}\" data-id=\"{}\"{}><div class=\"tete\"><div class=\"medaillon\">{}</div>{}</div><div class=\"corps\"><span class=\"nom\">{}</span><span class=\"lieu\">{}</span></div></div>",
                if c.reelle { "" } else { " fictive" },
                echapper(&c.id),
                if c.reelle { format!(" style=\"--cie:{}\"", c.couleur) } else { String::new() },
                medaillon,
                if c.reelle { CRAYON.replace("%s", &c.couleur) } else { ZERO.to_string() },
                echapper(&c.nom),
                echapper(&c.lieu)
            ),
        ));
    }
    let bride = crate::bridage::est_bride();
    let grille = if bride {
        format!("<div class=\"grille\">{}</div>", tuiles.iter().map(|(_, t)| t.as_str()).collect::<String>())
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
    // La version, sous le nom, comme dans la page d'une compagnie (ticket #211) ; les tuiles en dernier : un nom de
    // compagnie n'est jamais lu comme une marque a remplacer.
    ACCUEIL
        .replace("{{VERSION}}", &version())
        .replace("{{EMPREINTE}}", &empreinte())
        .replace(
            "{{SOUS_TITRE}}",
            if bride { "des compagnies fictives, et rien ne part" } else { "quelle compagnie ouvrir ?" },
        )
        .replace("{{TUILES}}", &grille)
}

/// La page du logiciel, telle que la fenetre la recoit (publique pour `tests/interface_fichiers.rs`).
pub fn page_logiciel(racine: &Path, jeton: &str) -> Result<String, String> {
    let brut = std::fs::read_to_string(ressources().join("index.html")).map_err(|e| format!("index.html illisible : {}", e))?;
    let copie = compagnies::est_copie(racine);
    let mut p = brut
        .replace("{{JETON}}", jeton)
        .replace("{{VERSION}}", &version())
        .replace("{{EMPREINTE}}", &empreinte())
        .replace("{{COPIE}}", if copie { "1" } else { "" })
        .replace("{{COPIE_TEXTE}}", &compagnies::texte_copie(racine))
        .replace(
            "{{STYLE_COMPAGNIE}}",
            &if copie { String::new() } else { format!("<style>:root {{ --compagnie: {}; }}</style>", compagnies::couleur(racine)) },
        )
        .replace("{{EDITION}}", if crate::bridage::est_bride() { " &middot; brid&eacute;" } else { "" });
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

    /// LES COMPAGNIES DE L'INSTALLATION, TENUES A JOUR DES LE LANCEMENT (ticket #244 : « ouvrir n'importe quelle compagnie
    /// instantanement, meme a la premiere ouverture du jour »). Chaque vraie compagnie de l'installation -- celles de
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
        if crate::bridage::est_bride() {
            return;
        }
        let f = self.clone();
        std::thread::spawn(move || {
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
        std::env::set_var("PUPPETPAIE_DONNEES", &racine);
        let journal = racine.join("journal").join("interface.log");
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
        if crate::bridage::est_bride() || compagnies::est_fictive(&racine) {
            self.ouvrir(racine);
            return Ok(());
        }
        let exigee = crate::coffre::Coffre::du_poste(&racine, "").ouverture_exigee().map_err(|e| e.message)?;
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

    /// Le mot de passe de l'administrateur, tape dans cette fenetre, ouvre-t-il `racine`, compagnie rattachee ?
    fn administrateur_ouvre(&self, racine: &Path) -> bool {
        let inst = compagnies::base();
        if !crate::administrateur::est_rattachee(&inst, racine) {
            return false;
        }
        let garde = self.administrateur.lock().unwrap_or_else(|e| e.into_inner()).clone();
        garde.is_some_and(|m| crate::administrateur::mot_de_passe_ouvre(&crate::administrateur::Poste::de_ce_poste(), racine, &m))
    }

    /// Retient le mot de passe de l'administrateur pour la fenetre, s'il ouvre une compagnie rattachee.
    fn retenir_administrateur(&self, racine: &Path, motdepasse: &str) {
        if crate::administrateur::est_rattachee(&compagnies::base(), racine) {
            *self.administrateur.lock().unwrap_or_else(|e| e.into_inner()) = Some(motdepasse.to_string());
        }
    }

    /// Les gestes de l'administrateur (ticket #244) : tout se decide dans `administrateur.rs`, la fenetre ne fait que
    /// passer la saisie et retenir, pour sa seance, le mot de passe qui vient d'etre verifie.
    fn geste_administrateur(&self, nom: &str, args: &[Value]) -> Option<Value> {
        let a = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let refus = |m: String| json!({"ok": false, "erreur": m});
        let inst = compagnies::base();
        let poste = crate::administrateur::Poste::de_ce_poste();
        let ouverte = match self.phase() {
            Phase::Ouverte { racine, .. } => Some(racine),
            _ => None,
        };
        let premiere_racine = || crate::administrateur::rattachees(&inst).into_iter().next();
        Some(match nom {
            "administrateur" => json!({"ok": true, "etat": crate::administrateur::etat(&inst, ouverte.as_deref(), &poste)}),
            "rattacher" => {
                let Some(racine) = ouverte else { return Some(refus("Aucune compagnie n'est ouverte.".into())) };
                match crate::administrateur::rattacher(&inst, &racine, &a(0), Some(&a(1)), &poste) {
                    Ok(noms) => {
                        self.retenir_administrateur(&racine, &a(0));
                        self.ecrire("Compagnie rattachee a l'administrateur.\n");
                        json!({"ok": true, "compagnies": noms, "etat": crate::administrateur::etat(&inst, Some(&racine), &poste)})
                    }
                    Err(m) => refus(m),
                }
            }
            "detacher" => {
                let Some(racine) = ouverte else { return Some(refus("Aucune compagnie n'est ouverte.".into())) };
                match crate::administrateur::detacher(&inst, &racine, &a(0), &a(1), &a(2), &poste) {
                    Ok(()) => {
                        self.ecrire("Compagnie detachee de l'administrateur.\n");
                        json!({"ok": true, "etat": crate::administrateur::etat(&inst, Some(&racine), &poste)})
                    }
                    Err(m) => refus(m),
                }
            }
            "changer_administrateur" => match crate::administrateur::changer(&inst, &a(0), &a(1), &a(2), &poste) {
                Ok(noms) => {
                    *self.administrateur.lock().unwrap_or_else(|e| e.into_inner()) = Some(a(1));
                    self.ecrire("Mot de passe de l'administrateur change.\n");
                    json!({"ok": true, "compagnies": noms})
                }
                Err(m) => refus(m),
            },
            // Le tableau de bord : ce qui est garde, tout de suite ; puis refait (et les comptes rendus attendus releves).
            "tableau" => json!({"ok": true, "tableau": crate::administrateur::tableau_garde(&inst)}),
            // AVANT LE MOT DE PASSE (aucun n'a encore ete tape dans cette fenetre) : le tableau de chaque compagnie, refait
            // par le fil de la fenetre (calcul local, rien ne sort : `prechauffer`). APRES : comme la veille, la releve
            // des comptes rendus attendus d'abord (le reseau, le coffre), par la route de l'administrateur.
            "tableau_rafraichir" => {
                let rattachees = crate::administrateur::rattachees(&inst);
                let mot_tape = self.administrateur.lock().unwrap_or_else(|e| e.into_inner()).is_some();
                match rattachees.first() {
                    None => json!({"ok": true, "tableau": crate::administrateur::tableau_garde(&inst)}),
                    Some(r) if mot_tape => match api::route(r, "administrateur-tableau", &json!({"relever": true})) {
                        Ok(t) => json!({"ok": true, "tableau": t}),
                        Err(e) => refus(e.message),
                    },
                    Some(_) => {
                        let par = rattachees.iter().map(|r| (r.clone(), self.file_de(r).tableau().map_err(|e| e.message))).collect();
                        json!({"ok": true, "tableau": crate::administrateur::tableau_de(&inst, par)})
                    }
                }
            }
            // LE DEPOT GROUPE : la route de l'administrateur, qui depose chaque compagnie par sa route `deposer`. Le mot de
            // passe (reel) est celui que la page vient de faire taper, au moment de l'acte.
            "deposer_groupe" => {
                let reel = args.first().and_then(Value::as_bool).unwrap_or(false);
                let Some(r) = premiere_racine() else { return Some(refus("Ce logiciel n'a pas d'administrateur.".into())) };
                let mut charge = json!({"reel": reel});
                if reel {
                    charge["motdepasse"] = json!(a(1));
                }
                match api::route(&r, "administrateur-deposer", &charge) {
                    Ok(v) => json!({"ok": true, "depot": v}),
                    Err(e) => refus(e.message),
                }
            }
            _ => return None,
        })
    }

    fn ouvrir(&self, racine: PathBuf) {
        // La mise a jour ne se fait pas ici : le logiciel la cherche au lancement (`maj::chercher_en_fond`) et l'applique
        // au lancement suivant, avant toute fenetre (`maj::au_lancement`), sans qu'aucune compagnie soit ouverte.
        // Les etats des mois se refont en fond, dans ce processus, s'ils ne sont plus justes (`etat_garde::FileDesEtats`) :
        // a l'ouverture, apres chaque ecriture et chaque tour de veille. Un mois s'affiche ensuite sans calcul. La file est
        // celle que le fil de la fenetre tient deja pour cette compagnie (ticket #244, `prechauffer`) : elle passe devant.
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

    /// Les gestes de l'accueil (`window.pywebview.api.<nom>(...)`).
    fn lanceur(&self, nom: &str, args: &[Value]) -> Value {
        let a = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let ok = |recharger: bool| json!({"ok": true, "recharger": recharger});
        let refus = |m: String| json!({"ok": false, "erreur": m});
        match (nom, self.phase()) {
            ("choisir", Phase::Accueil) => match compagnies::racine_choisie(&a(0)).and_then(|r| self.prendre(r)) {
                Ok(()) => ok(true),
                Err(m) => refus(m),
            },
            ("couleur", _) => match compagnies::poser_couleur(&a(0), &a(1)) {
                Ok(c) => json!({"ok": true, "couleur": c}),
                Err(m) => refus(m),
            },
            ("basculer", Phase::Accueil) => match crate::bridage::basculer() {
                Ok(()) => ok(true),
                Err(m) => refus(m),
            },
            ("remettre_a_zero", _) => match compagnies::remettre_a_zero(&a(0)) {
                Ok((n, deja)) => json!({"ok": true, "message": format!("{} est {}.", n, if deja { "déjà dans son état d'origine" } else { "revenue à son état d'origine" })}),
                Err(m) => refus(m),
            },
            ("ajouter", Phase::Accueil) => match compagnies::ajouter(&a(0)).and_then(|c| self.prendre(c.racine)) {
                Ok(()) => ok(true),
                Err(m) => refus(m),
            },
            ("verifier", Phase::MotDePasse { racine, .. }) => match crate::coffre::Coffre::du_poste(&racine, "").verifier_ouverture(&a(0)) {
                Ok(true) => {
                    self.retenir_administrateur(&racine, &a(0));
                    self.ouvrir(racine);
                    ok(true)
                }
                Ok(false) => refus("Mot de passe refusé.".into()),
                Err(e) => refus(e.message),
            },
            // « Meme administrateur ? » -- oui (ticket #244) : le mot de passe de l'administrateur, tape une fois pour le
            // verifier, scelle aussi cette compagnie neuve.
            ("rattacher_nouvelle", Phase::MotDePasse { racine, premiere: true }) => {
                match crate::administrateur::rattacher(&compagnies::base(), &racine, &a(0), None, &crate::administrateur::Poste::de_ce_poste()) {
                    Ok(_) => {
                        self.retenir_administrateur(&racine, &a(0));
                        self.ecrire("Compagnie neuve rattachee a l'administrateur.\n");
                        self.ouvrir(racine);
                        ok(true)
                    }
                    Err(m) => refus(m),
                }
            }
            ("poser", Phase::MotDePasse { racine, premiere: true }) => {
                if a(0) != a(1) {
                    return refus("Les deux saisies diffèrent.".into());
                }
                // L'heure est due : le coffre note quand le mot de passe a ete pose (`pose_le`). Sans elle, une compagnie
                // neuve ne pouvait pas poser son premier mot de passe (« il faut `maintenant` », trouve le 09/10/2026).
                let maintenant = crate::releve_retours::iso_de_naif(crate::releve_retours::maintenant_naif())[..16].to_string();
                match crate::coffre::Coffre::du_poste(&racine, &maintenant).poser_ouverture(&a(0), None) {
                    Ok(_) => {
                        self.ouvrir(racine);
                        ok(true)
                    }
                    Err(e) => refus(e.message),
                }
            }
            ("accueil", _) => {
                self.ecrire("Retour a l'accueil.\n");
                self.poser_phase(Phase::Accueil);
                ok(true)
            }
            ("fermer", _) => {
                self.fermer_le_logiciel();
                ok(false)
            }
            // Ou en est la recherche de la mise a jour (ticket #210) : l'accueil et la page d'une compagnie l'affichent.
            ("maj", _) => json!({"ok": true, "etat": crate::maj::RECHERCHE.etat()}),
            // « Redemarrer pour mettre a jour » : la version prete part au relais, puis le logiciel se ferme comme par sa
            // croix (la veille s'arrete) ; le relais attend sa fin, installe et rouvre la fenetre neuve. Rien de pret :
            // refuse, et rien ne se ferme.
            ("redemarrer", _) => match crate::maj::redemarrer_pour_mettre_a_jour(&crate::maj::installation(), &crate::maj::RECHERCHE) {
                Ok(v) => {
                    self.ecrire(&format!("Redemarrage pour la mise a jour {}.\n", crate::maj::version_lue(&v)));
                    self.poser_phase(Phase::Accueil);
                    self.fermer_le_logiciel();
                    ok(false)
                }
                Err(m) => refus(m),
            },
            _ => self.geste_administrateur(nom, args).unwrap_or_else(|| refus(format!("Geste inconnu ici : {}.", nom))),
        }
    }

    /// Une requete de la fenetre.
    pub fn traiter(&self, r: Requete) -> Reponse {
        let texte = |code: u16| Reponse::nouvelle(code, "text/plain; charset=utf-8", Vec::new());
        if r.methode == "POST" {
            // Les gestes de l'accueil lisent `erreur` en phrase (`err.textContent = r.erreur`) ; les routes de la page,
            // en objet (`d.erreur.message`). Un refus du serveur parle a chacun sa langue (#141 : « [object Object] »).
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
            if let Some(nom) = r.chemin.strip_prefix("/api/") {
                let Phase::Ouverte { racine, veille, etats } = self.phase() else {
                    return Reponse::json(200, &json!({"ok": false, "erreur": {"type": "ErreurOperation", "message": "Aucune compagnie n'est ouverte."}}));
                };
                return Reponse::json(200, &api::repondre(&racine, nom, &charge, Some(&veille), Some(&etats)));
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
                        // Ce que l'administrateur est pour cette compagnie (ticket #244) : la page le lit, sans rien decider.
                        let etat = crate::administrateur::etat(&compagnies::base(), Some(&racine), &crate::administrateur::Poste::de_ce_poste());
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
                // UNE FEUILLE OU UN SCRIPT NOMME PAR SON EMPREINTE (`?v=`, `empreinte`) se garde (ticket #244) : la page
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
        if !self.autorise(r) || !crate::routes_accueil::simulation_active(&racine) {
            return interdit;
        }
        let nom = r.params.get("c").cloned().unwrap_or_default();
        let rang: usize = r.params.get("p").and_then(|p| p.parse().ok()).unwrap_or(0);
        let Some(brut) = crate::simulation_courriels::courriels(&racine).into_iter().find(|(n, _)| *n == nom).map(|(_, o)| o) else {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        };
        let Ok(m) = mailparse::parse_mail(&brut) else { return Reponse::nouvelle(404, "text/plain", Vec::new()) };
        let mut feuilles = Vec::new();
        crate::simulation_courriels::feuilles(&m, &mut feuilles);
        let pieces: Vec<_> = feuilles.into_iter().filter(|f| crate::simulation_courriels::nom_de_fichier(f).is_some()).collect();
        let Some(piece) = pieces.get(rang) else { return Reponse::nouvelle(404, "text/plain", Vec::new()) };
        let octets = piece.get_body_raw().unwrap_or_default();
        if piece.ctype.mimetype != "application/pdf" || !octets.starts_with(b"%PDF") {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        }
        let nom_pdf = crate::simulation_courriels::nom_de_fichier(piece).unwrap_or_else(|| "piece.pdf".into());
        let sur: String = nom_pdf.chars().map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
        Reponse::nouvelle(200, "application/pdf", octets)
            .avec("Content-Disposition", format!("inline; filename=\"{}\"", sur))
            .avec("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'self'")
    }

    /// L'apercu d'un modele de bulletin (lot R11) : le PDF d'une paie fictive, que la route `bulletin-modele-apercu` fait
    /// dans son propre processus, comme toute route ; le lecteur de la fenetre l'affiche (Configuration).
    fn apercu_bulletin(&self, r: &Requete) -> Reponse {
        let interdit = Reponse::nouvelle(403, "text/plain", Vec::new());
        let Phase::Ouverte { racine, .. } = self.phase() else { return interdit };
        if !self.autorise(r) {
            return interdit;
        }
        let demande = r.params.get("modele").map(String::as_str).unwrap_or_default();
        let Some(modele) = crate::routes_bulletin_modele::modele_de_l_adresse(demande) else {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        };
        let pdf = api::route(&racine, "bulletin-modele-apercu", &json!({"modele": modele.cle()}))
            .ok()
            .and_then(|v| v.get("pdf").and_then(Value::as_str).and_then(|b| crate::retours::base64_lire(b).ok()));
        match pdf {
            Some(octets) if octets.starts_with(b"%PDF") => Reponse::nouvelle(200, "application/pdf", octets)
                .avec("Content-Disposition", format!("inline; filename=\"apercu-bulletin-{}.pdf\"", if modele == crate::bulletin::Modele::Complet { "complet" } else { "simplifie" }))
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
            let ent = crate::contexte::Contexte::nouveau(&racine, None).map(|c| c.entreprise.clone()).unwrap_or(Value::Null);
            return match crate::cachet::svg(&ent, None) {
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

fn egal_en_temps_constant(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// AU LANCEMENT, avant tout : ce qu'une installation posee avant le lot U1 doit ranger (`demo/` devient `fictives/`,
/// `compagnies::ranger_les_fictives`). Ce qui s'est passe va dans le journal de l'installation (`maj::noter`).
pub fn preparer_l_installation() {
    let inst = crate::maj::installation();
    if let Some(t) = compagnies::ranger_les_fictives(&inst) {
        crate::maj::noter(&inst, &t);
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
/// ligne « la fenetre ecoute sur » dit lequel). Rend la fenetre et le port. Hors du verrou : `paie interface --port`, et
/// les essais.
pub fn demarrer_sur(minutes_veille: u64, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let (ecouteur, port) = http::ecouter(port)?;
    servir_sur(minutes_veille, ecouteur, port)
}

fn servir_sur(minutes_veille: u64, ecouteur: TcpListener, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let f = Fenetre::nouvelle(minutes_veille, instance::dossier_local(&compagnies::base(), port), port);
    let servie = f.clone();
    std::thread::spawn(move || http::servir(ecouteur, Arc::new(move |r| servie.traiter(r))));
    f.ecrire(&format!("PuppetPaie {} : la fenetre ecoute sur 127.0.0.1:{}.\n", version(), port));
    Ok((f, port))
}

/// Ce que `paie interface` a lu de sa ligne de commande. `port` : celui qu'on impose ; sans lui, celui de l'installation
/// (`instance.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub port: Option<u16>,
    pub minutes_veille: u64,
    pub sans_navigateur: bool,
}

const USAGE: &str = "Usage : paie interface [--port N] [--sans-navigateur] [--veille MINUTES]";

/// Les options de `paie interface`. Une valeur qui ne se lit pas est refusee, jamais remplacee par le defaut
/// (reste 143 : `--port` etait lu puis ignore).
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

/// `paie interface [--port N] [--sans-navigateur] [--veille MINUTES]` : la fenetre dans le navigateur du poste.
pub fn commande(args: &[String]) -> i32 {
    crate::commandes::compagnie::fixer_le_code();
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
            println!("PuppetPaie est deja ouvert depuis cette installation : sa fenetre revient devant.");
            return 0;
        }
    };
    // La mise a jour se cherche au lancement, pour l'installation (`maj::chercher_en_fond`) ; elle s'appliquera au
    // prochain lancement de la fenetre.
    preparer_l_installation();
    crate::maj::chercher_en_fond();
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
    println!("{}", url);
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
