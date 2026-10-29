//! La demande du mot de passe du logiciel, faite PAR LE MOTEUR (Leandre, 08/10/2026 : « une IA ne fait rien de reel
//! sans humain », lot J1).
//!
//! Pour un acte qui engage -- depot reel, envoi aux salaries, demande de signature, tout ce que
//! `depot_acte::verifier_preuve` et `filet::exiger_mot_de_passe` gardent --, le moteur demande LUI-MEME le mot de
//! passe, par un canal qu'un agent ne tient pas :
//!
//!   1. s'il y a un ecran : une petite fenetre du moteur, qui dit ce qui va partir (quoi, quel mois, a qui) ; la
//!      fermer, c'est refuser ;
//!   2. sinon, si un humain est au terminal : la question dans le terminal, SANS ECHO ;
//!   3. sinon (un agent seul, un conteneur) : REFUS, avec une phrase qui dit pourquoi et quoi faire.
//!
//! L'APPELANT (une route, une commande, un agent) NE RECOIT QUE `Accepte` OU `Refuse(phrase)`. Le mot de passe tape
//! est verifie ICI contre le coffre, retenu pour la suite de la commande (il ouvre le coffre : `coffre::
//! retenir_mot_de_passe_tape`), et ne va nulle part ailleurs : ni fichier, ni journal, ni erreur, ni sortie. Une demande
//! refusee ne laisse rien derriere elle.
//!
//! LA MEME DEMANDE SERT A LA SEANCE (`ouvrir_seance` : le mot de passe tape une fois, garde 12 h par
//! `Coffre::garder_ouverture`). La seance OUVRE le coffre, elle n'AUTORISE JAMAIS un acte reel : `verifier_preuve` ne
//! lit ni la seance, ni `PUPPETPAIE_MOTDEPASSE`, ni le mot de passe retenu -- seulement la reponse d'une demande.
//!
//! COMMENT UNE COMMANDE S'EN SERT : au demarrage du processus, `let _canaux = demande::installer(Rc::new(
//! demande::Reel::du_poste()));` ; ensuite `verifier_preuve` et `exiger_mot_de_passe` demandent, et la commande
//! `coffre seance` appelle `ouvrir_seance`. Sans rien d'installe (la bibliotheque seule, les tests), la demande REFUSE.
//!
//! COMMENT LA REMPLACER DANS UN TEST : `let _g = demande::installer(Rc::new(demande::Fausse::mot("...")));` -- la
//! fausse repond ce qu'on lui a dit et compte les questions ; le garde rend l'installation precedente en tombant.
//! L'installation est PAR FIL, comme le mot de passe retenu : un test ne la transmet pas a un autre.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::compagnie::coffre::{retenir_mot_de_passe_tape, Coffre};

/// Combien de fois on laisse retaper un mot de passe refuse avant de renoncer.
const ESSAIS: u8 = 3;

/// Heures de la seance (`coffre seance`).
pub const HEURES_SEANCE: i64 = 12;

/// Ce qui va partir, tel que l'humain doit le lire avant de taper.
#[derive(Clone, Debug, PartialEq)]
pub struct Acte {
    pub quoi: String,
    pub mois: String,
    pub a_qui: String,
}

impl Acte {
    pub fn nouveau(quoi: impl Into<String>, mois: impl Into<String>, a_qui: impl Into<String>) -> Acte {
        Acte { quoi: quoi.into(), mois: mois.into(), a_qui: a_qui.into() }
    }
}

/// Ce qu'un canal montre : un titre, des lignes, et l'essai en cours. Jamais un secret.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub titre: String,
    pub lignes: Vec<String>,
    /// 1 pour la premiere fois ; plus, apres un mot de passe refuse.
    pub essai: u8,
    /// Vrai quand le mot de passe precedent a ete refuse : le canal le dit a l'humain.
    pub precedent_refuse: bool,
    /// Vrai pour la seance (elle n'autorise aucun acte), faux pour un acte.
    pub seance: bool,
    /// La ligne qui precede la saisie, quand ce n'est pas celle du mot de passe du logiciel (poser le mot de passe d'un organisme :
    /// `saisir_secret`). Jamais un secret.
    pub invite: Option<String>,
}

/// Ce qu'un canal rapporte : le mot de passe tape, ou le refus (fenetre fermee, rien tape, personne la).
pub enum Reponse {
    Mot(String),
    Refus(String),
}

// Un `Debug` qui ne montre jamais le mot de passe : un `{:?}` perdu ne doit rien fuir.
impl std::fmt::Debug for Reponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reponse::Mot(_) => f.write_str("Reponse::Mot(<masque>)"),
            Reponse::Refus(p) => write!(f, "Reponse::Refus({:?})", p),
        }
    }
}

/// Ce que l'appelant recoit : « accepte » ou « refuse », et la phrase du refus (sans rien du secret).
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Accepte,
    Refuse(String),
}

impl Verdict {
    pub fn est_accepte(&self) -> bool {
        matches!(self, Verdict::Accepte)
    }
}

/// Un moyen de poser la question a un humain : une fenetre, un terminal, une fausse.
pub trait Canal {
    fn demander(&self, question: &Question) -> Reponse;
}

// ------------------------------------------------------------------ l'installation (par fil)

thread_local! {
    static INSTALLE: RefCell<Option<Rc<dyn Canal>>> = const { RefCell::new(None) };
}

/// Tient l'installation d'un canal ; en tombant, rend celle d'avant.
pub struct Installation {
    avant: Option<Rc<dyn Canal>>,
}

impl Drop for Installation {
    fn drop(&mut self) {
        let avant = self.avant.take();
        INSTALLE.with(|c| *c.borrow_mut() = avant);
    }
}

/// Installe le canal qui repondra aux demandes de ce fil (`Reel::du_poste()` pour une commande, `Fausse` pour un test).
#[must_use = "l'installation ne dure que tant que son garde vit"]
pub fn installer(canal: Rc<dyn Canal>) -> Installation {
    let avant = INSTALLE.with(|c| c.borrow_mut().replace(canal));
    Installation { avant }
}

fn canal_installe() -> Option<Rc<dyn Canal>> {
    INSTALLE.with(|c| c.borrow().clone())
}

// ------------------------------------------------------------------ la demande

/// La phrase de refus quand personne ne peut taper : pourquoi, et quoi faire.
pub const SANS_HUMAIN: &str = "Rien n'est parti : aucun humain n'est la pour taper le mot de passe du logiciel (ni ecran, ni terminal). \
Un acte reel ne se fait pas sans lui : le lancer depuis la fenetre PuppetPaie, ou depuis un terminal ouvert par vous.";

fn question_de_l_acte(acte: &Acte, essai: u8, precedent_refuse: bool) -> Question {
    let mut lignes = vec![format!("Acte : {}", acte.quoi)];
    if !acte.mois.trim().is_empty() {
        lignes.push(format!("Mois : {}", acte.mois.trim()));
    }
    if !acte.a_qui.trim().is_empty() {
        lignes.push(format!("A : {}", acte.a_qui.trim()));
    }
    lignes.push("Taper le mot de passe du logiciel autorise CET acte, maintenant. Fermer la fenetre le refuse.".into());
    Question { titre: "PuppetPaie : autoriser cet acte".into(), lignes, essai, precedent_refuse, seance: false, invite: None }
}

fn question_de_la_seance(heures: i64, essai: u8, precedent_refuse: bool) -> Question {
    Question {
        titre: "PuppetPaie : ouvrir le coffre".into(),
        lignes: vec![
            format!("Le mot de passe du logiciel ouvre le coffre pour {} h.", heures),
            "Il n'autorise AUCUN acte reel : chaque depot, envoi ou signature le redemandera.".into(),
        ],
        essai,
        precedent_refuse,
        seance: true,
        invite: None,
    }
}

/// Pose la question jusqu'a `ESSAIS` fois ; rend le mot de passe qui ouvre le coffre de `racine`, ou la phrase du refus.
/// Le mot de passe sort d'ici vers l'appelant interne seul (jamais vers un agent) ; rien d'autre n'en garde trace.
fn obtenir(racine: &Path, canal: &dyn Canal, question: impl Fn(u8, bool) -> Question) -> Result<String, String> {
    let coffre = Coffre::du_poste(racine, "");
    let mut refuse = false;
    for essai in 1..=ESSAIS {
        match canal.demander(&question(essai, refuse)) {
            Reponse::Refus(phrase) => return Err(phrase),
            Reponse::Mot(m) => {
                if m.is_empty() {
                    return Err("Rien n'a ete tape : refuse.".into());
                }
                match coffre.verifier_ouverture(&m) {
                    Ok(true) => return Ok(m),
                    Ok(false) => refuse = true,
                    // Un coffre illisible ne dit rien du mot de passe : la phrase de l'erreur est celle du coffre.
                    Err(e) => return Err(e.message),
                }
            }
        }
    }
    Err("Mot de passe du logiciel refuse. Rien n'est parti.".into())
}

fn sans_mot_de_passe_pose(racine: &Path) -> Option<String> {
    match Coffre::du_poste(racine, "").ouverture_exigee() {
        Ok(true) => None,
        Ok(false) => Some(
            "Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel. Le poser d'abord (la fenetre PuppetPaie le demande a sa premiere ouverture). Rien n'est parti."
                .into(),
        ),
        Err(e) => Some(e.message),
    }
}

/// Demande l'aval d'un humain pour `acte`. Accepte : le mot de passe verifie est retenu pour la suite de la commande
/// (il ouvre le coffre). Refuse : rien n'est retenu, rien n'est ecrit.
pub fn autoriser_acte(racine: &Path, acte: &Acte) -> Verdict {
    if let Some(phrase) = sans_mot_de_passe_pose(racine) {
        return Verdict::Refuse(phrase);
    }
    let Some(canal) = canal_installe() else { return Verdict::Refuse(SANS_HUMAIN.into()) };
    match obtenir(racine, canal.as_ref(), |essai, refuse| question_de_l_acte(acte, essai, refuse)) {
        Ok(m) => {
            retenir_mot_de_passe_tape(&m);
            Verdict::Accepte
        }
        Err(phrase) => Verdict::Refuse(phrase),
    }
}

/// L'aval d'un humain pour un acte qui part pour PLUSIEURS compagnies de l'administrateur (`administrateur::deposer`, ticket
/// #244) : le mot de passe tape, verifie sur le coffre de `racine` (une compagnie rattachee), RENDU a l'appelant, qui le
/// passe comme preuve au depot de chaque compagnie -- chaque depot le reverifie sur son propre coffre (`verifier_preuve`).
/// Rien n'est retenu ici. Refuse comme `autoriser_acte` : sans mot de passe pose, sans humain, fenetre fermee.
pub fn mot_de_passe_de_l_acte(racine: &Path, acte: &Acte) -> Result<String, String> {
    if let Some(phrase) = sans_mot_de_passe_pose(racine) {
        return Err(phrase);
    }
    let Some(canal) = canal_installe() else { return Err(SANS_HUMAIN.into()) };
    obtenir(racine, canal.as_ref(), |essai, refuse| question_de_l_acte(acte, essai, refuse))
}

/// La seance : le mot de passe tape une fois, GARDE `HEURES_SEANCE` h (`Coffre::garder_ouverture`) et retenu pour la
/// commande. Elle ouvre le coffre ; elle n'autorise jamais un acte reel.
pub fn ouvrir_seance(racine: &Path) -> Verdict {
    if let Some(phrase) = sans_mot_de_passe_pose(racine) {
        return Verdict::Refuse(phrase);
    }
    let Some(canal) = canal_installe() else { return Verdict::Refuse(SANS_HUMAIN.into()) };
    match obtenir(racine, canal.as_ref(), |essai, refuse| question_de_la_seance(HEURES_SEANCE, essai, refuse)) {
        Ok(m) => {
            let maintenant = socle::paris::maintenant_naif();
            let fin = socle::paris::iso_de_naif(maintenant + HEURES_SEANCE * 3600)[..16].to_string();
            let coffre = Coffre::du_poste(racine, &socle::paris::iso_de_naif(maintenant)[..16]);
            match coffre.garder_ouverture(&m, &fin) {
                Ok(_) => {
                    retenir_mot_de_passe_tape(&m);
                    Verdict::Accepte
                }
                Err(e) => Verdict::Refuse(e.message),
            }
        }
        Err(phrase) => Verdict::Refuse(phrase),
    }
}

/// La phrase de refus quand personne ne peut taper un mot de passe a poser.
pub const SANS_HUMAIN_SAISIE: &str = "Rien n'a ete pose : aucun humain n'est la pour taper le mot de passe sans l'afficher (ni ecran, ni terminal). \
Poser un mot de passe est un geste d'humain : le faire depuis un terminal ouvert par vous.";

/// La SAISIE d'un secret a POSER (le mot de passe d'un organisme, d'une boite courriel, le nouveau mot de passe du logiciel), par le
/// meme canal que la demande d'un acte : la fenetre du moteur s'il y en a une, sinon le terminal SANS ECHO, sinon un refus. Ni
/// l'entree standard ni la sortie standard ne servent (une commande y echange du JSON que lit un agent). `titre` et `lignes` disent
/// ce qu'on va poser (jamais un secret) ; `invite` est la ligne qui precede la frappe.
///
/// Rend le texte tape, ou la phrase du refus. RIEN n'est verifie ici (ce n'est pas une preuve d'acte : `autoriser_acte` en est
/// une) et RIEN n'est retenu : l'appelant range le secret et le laisse tomber. Un texte vide est un refus.
pub fn saisir_secret(titre: &str, lignes: &[String], invite: &str) -> Result<String, String> {
    let Some(canal) = canal_installe() else { return Err(SANS_HUMAIN_SAISIE.into()) };
    let question = Question { titre: titre.into(), lignes: lignes.to_vec(), essai: 1, precedent_refuse: false, seance: false, invite: Some(invite.into()) };
    match canal.demander(&question) {
        Reponse::Mot(m) if m.is_empty() => Err("Rien n'a ete tape : rien n'est pose.".into()),
        Reponse::Mot(m) => Ok(m),
        Reponse::Refus(p) if p == SANS_HUMAIN => Err(SANS_HUMAIN_SAISIE.into()),
        Reponse::Refus(p) => Err(p),
    }
}

// ------------------------------------------------------------------ une fausse, pour les tests

/// Une fausse demande : repond ce qu'on lui a dit, compte les questions et les garde (sans secret : une `Question` n'en
/// porte pas). `Fausse::mot("x")` tape toujours `x` ; `Fausse::ferme()` ferme la fenetre ; `Fausse::suite(...)` repond
/// dans l'ordre (la derniere reponse se repete).
pub struct Fausse {
    reponses: Vec<Option<String>>,
    rang: RefCell<usize>,
    vues: RefCell<Vec<Question>>,
}

impl Fausse {
    pub fn mot(mot: &str) -> Fausse {
        Fausse::suite(&[Some(mot)])
    }

    pub fn ferme() -> Fausse {
        Fausse::suite(&[None])
    }

    pub fn suite(reponses: &[Option<&str>]) -> Fausse {
        Fausse { reponses: reponses.iter().map(|r| r.map(String::from)).collect(), rang: RefCell::new(0), vues: RefCell::new(Vec::new()) }
    }

    /// Les questions posees, dans l'ordre.
    pub fn questions(&self) -> Vec<Question> {
        self.vues.borrow().clone()
    }

    pub fn nombre(&self) -> usize {
        self.vues.borrow().len()
    }
}

impl Canal for Fausse {
    fn demander(&self, question: &Question) -> Reponse {
        self.vues.borrow_mut().push(question.clone());
        let mut rang = self.rang.borrow_mut();
        let prochaine = self.reponses.get((*rang).min(self.reponses.len().saturating_sub(1))).cloned().flatten();
        *rang += 1;
        match prochaine {
            Some(m) => Reponse::Mot(m),
            None => Reponse::Refus("Fenetre fermee : refuse. Rien n'est parti.".into()),
        }
    }
}

// ------------------------------------------------------------------ le canal reel

/// Le canal d'un poste : la fenetre s'il y a un ecran ET une fenetre, sinon le terminal s'il y a un humain, sinon rien.
pub struct Reel {
    /// Y a-t-il un ecran (`DISPLAY` / `WAYLAND_DISPLAY` sous Linux ; present sous Windows et macOS) ?
    pub ecran: bool,
    /// La fenetre native du moteur. AUCUNE POUR L'INSTANT : le choix de la bibliotheque est celui de Leandre
    /// (`prive/journal/j/PASSATION-J1.md`). Sans elle, un ecran seul ne suffit pas : on passe au terminal, sinon on refuse.
    pub fenetre: Option<Box<dyn Canal>>,
    /// Le terminal de l'humain (`/dev/tty`), saisie sans echo.
    pub terminal: Option<Box<dyn Canal>>,
}

impl Reel {
    /// Les canaux de CE poste, d'apres l'environnement du processus.
    pub fn du_poste() -> Reel {
        let env = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
        Reel { ecran: ecran_present(&env), fenetre: fenetre_native(), terminal: terminal_du_poste() }
    }
}

/// Y a-t-il un ecran ? Linux : un serveur d'affichage nomme ; Windows et macOS : oui (la session est graphique).
pub fn ecran_present(env: &dyn Fn(&str) -> Option<String>) -> bool {
    if cfg!(any(windows, target_os = "macos")) {
        return true;
    }
    env("DISPLAY").is_some() || env("WAYLAND_DISPLAY").is_some()
}

/// LA FENETRE NATIVE. Pas encore choisie (decision de Leandre) : rend `None`. Le jour ou une bibliotheque est retenue,
/// c'est ICI qu'elle se branche, derriere `Canal`, et rien d'autre ne change.
fn fenetre_native() -> Option<Box<dyn Canal>> {
    None
}

impl Canal for Reel {
    fn demander(&self, question: &Question) -> Reponse {
        if self.ecran {
            if let Some(f) = &self.fenetre {
                // Une fenetre fermee refuse : on ne retombe PAS sur le terminal, l'humain a dit non.
                return f.demander(question);
            }
        }
        if let Some(t) = &self.terminal {
            return t.demander(question);
        }
        Reponse::Refus(SANS_HUMAIN.into())
    }
}

// ------------------------------------------------------------------ le terminal

#[cfg(unix)]
fn terminal_du_poste() -> Option<Box<dyn Canal>> {
    terminal_unix::Tty::presente().then(|| Box::new(terminal_unix::Tty) as Box<dyn Canal>)
}

/// Windows : la console, par l'API de la console (CONIN$ / CONOUT$, saisie sans echo) que fait la bibliotheque `rpassword`.
/// Comme sous Unix, ni l'entree ni la sortie standard : une console absente (un agent sans terminal) refuse.
#[cfg(windows)]
fn terminal_du_poste() -> Option<Box<dyn Canal>> {
    Some(Box::new(terminal_windows::Console))
}

#[cfg(not(any(unix, windows)))]
fn terminal_du_poste() -> Option<Box<dyn Canal>> {
    None
}

#[cfg(windows)]
mod terminal_windows {
    use super::{Canal, Question, Reponse};
    use std::sync::mpsc;
    use std::time::Duration;

    /// Combien de temps on attend l'humain avant de refuser.
    const ATTENTE: Duration = Duration::from_secs(180);

    /// La console de ce processus. NON COMPILE NI ESSAYE DEPUIS LE CONTENEUR (pas de cible Windows) : a essayer sur le poste.
    pub struct Console;

    impl Canal for Console {
        fn demander(&self, q: &Question) -> Reponse {
            let mut invite = format!("\n{}\n", q.titre);
            for l in &q.lignes {
                invite.push_str(&format!("  {}\n", l));
            }
            if q.precedent_refuse {
                invite.push_str(&format!("  Mot de passe refuse (essai {}).\n", q.essai));
            }
            invite.push_str(q.invite.as_deref().unwrap_or("Mot de passe du logiciel (rien ne s'affiche) : "));
            let (envoi, recu) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = envoi.send(rpassword::prompt_password(invite).ok());
            });
            match recu.recv_timeout(ATTENTE).ok().flatten() {
                Some(m) => Reponse::Mot(m.trim_end_matches(['\r', '\n']).to_string()),
                None => Reponse::Refus("Rien n'a ete tape (delai depasse, ou pas de console) : refuse. Rien n'est parti.".into()),
            }
        }
    }
}

#[cfg(unix)]
mod terminal_unix {
    use super::{Canal, Question, Reponse};
    use std::fs::{File, OpenOptions};
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    /// Combien de temps on attend l'humain avant de refuser.
    const ATTENTE: Duration = Duration::from_secs(180);

    /// Le terminal de commande de ce processus, ouvert par son nom : ni l'entree standard (une commande y recoit sa
    /// demande JSON), ni la sortie (un agent la lit), le terminal lui-meme. Un processus sans terminal ne l'ouvre pas.
    pub struct Tty;

    fn ouvrir() -> std::io::Result<File> {
        OpenOptions::new().read(true).write(true).open("/dev/tty")
    }

    impl Tty {
        pub fn presente() -> bool {
            ouvrir().is_ok()
        }
    }

    /// `stty` avec le terminal pour entree ; sa sortie si elle reussit.
    fn stty(tty: &File, args: &[&str]) -> Option<String> {
        let entree = tty.try_clone().ok()?;
        let sortie = Command::new("stty").args(args).stdin(Stdio::from(entree)).stderr(Stdio::null()).output().ok()?;
        sortie.status.success().then(|| String::from_utf8_lossy(&sortie.stdout).trim().to_string())
    }

    impl Canal for Tty {
        fn demander(&self, q: &Question) -> Reponse {
            let Ok(mut tty) = ouvrir() else { return Reponse::Refus(super::SANS_HUMAIN.into()) };
            // L'etat du terminal avant, pour le rendre tel quel : pas de saisie sans pouvoir masquer l'echo.
            let Some(avant) = stty(&tty, &["-g"]) else {
                return Reponse::Refus("Ce terminal ne sait pas masquer la saisie : rien n'est tape, refuse. Rien n'est parti.".into());
            };
            if stty(&tty, &["-echo"]).is_none() {
                return Reponse::Refus("Ce terminal ne sait pas masquer la saisie : rien n'est tape, refuse. Rien n'est parti.".into());
            }
            let _ = writeln!(tty, "\n{}", q.titre);
            for l in &q.lignes {
                let _ = writeln!(tty, "  {}", l);
            }
            if q.precedent_refuse {
                let _ = writeln!(tty, "  Mot de passe refuse (essai {}).", q.essai);
            }
            let _ = write!(tty, "{}", q.invite.as_deref().unwrap_or("Mot de passe du logiciel (rien ne s'affiche) : "));
            let _ = tty.flush();
            let lecteur = tty.try_clone().ok();
            let (envoi, recu) = mpsc::channel();
            if let Some(l) = lecteur {
                std::thread::spawn(move || {
                    let mut ligne = String::new();
                    let ok = BufReader::new(l).read_line(&mut ligne).is_ok();
                    let _ = envoi.send(ok.then_some(ligne));
                });
            }
            let ligne = recu.recv_timeout(ATTENTE).ok().flatten();
            // Rendre le terminal dans tous les cas.
            let _ = stty(&tty, &[&avant]);
            let _ = writeln!(tty);
            match ligne {
                Some(l) => Reponse::Mot(l.trim_end_matches(['\r', '\n']).to_string()),
                None => Reponse::Refus("Rien n'a ete tape (delai depasse ou terminal ferme) : refuse. Rien n'est parti.".into()),
            }
        }
    }
}

// ====================================================================================================== les tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compagnie::coffre::oublier_mot_de_passe_tape;
    use droit::contexte::Contexte;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    // Tout ce que ces tests inventent : un mot de passe du logiciel, des secrets d'organisme. Rien de reel.
    const LOGICIEL: &str = "Logiciel-de-test-0801";
    const NET: &str = "SecretNet-fictif-4471";
    const BOITE: &str = "boite-fictive-9921";

    // `PUPPETPAIE_CLE` est une variable du processus : un test a la fois y touche.
    static TOUR: Mutex<()> = Mutex::new(());

    struct Monde {
        _tour: MutexGuard<'static, ()>,
        racine: PathBuf,
        /// Le poste qui a ferme le coffre (sa cle y est).
        poste_a: PathBuf,
        /// Le poste d'ici (`PUPPETPAIE_CLE`) : sans la cle du coffre -- l'« AppData virtuel » d'un agent sous Windows.
        poste_b: PathBuf,
    }

    impl Monde {
        fn ctx(&self) -> Contexte {
            Contexte::nouveau(&self.racine, None).unwrap()
        }

        fn coffre_a(&self) -> Coffre {
            Coffre { racine: self.racine.clone(), dossier_cle: self.poste_a.clone(), maintenant: "2026-10-08T10:00".into(), variables: Default::default() }
        }
    }

    impl Drop for Monde {
        fn drop(&mut self) {
            oublier_mot_de_passe_tape();
            std::env::remove_var("PUPPETPAIE_CLE");
            std::env::remove_var("PUPPETPAIE_MOTDEPASSE");
        }
    }

    /// Un coffre ferme par un poste A, mot de passe du logiciel pose, trois secrets ; le processus est sur un poste B.
    fn monde() -> Monde {
        let tour = TOUR.lock().unwrap_or_else(|e| e.into_inner());
        let racine = crate::horloge::racine_d_essai(false);
        let poste_a = racine.join("poste-a");
        let poste_b = racine.join("poste-b");
        std::fs::create_dir_all(&poste_b).unwrap();
        for v in ["PUPPETPAIE_MOTDEPASSE", "NET_ENTREPRISES_MOTDEPASSE", "URSSAF_MOTDEPASSE", "COURRIEL_MOTDEPASSE", "COURRIEL_COMPTE"] {
            std::env::remove_var(v);
        }
        oublier_mot_de_passe_tape();
        let m = Monde { _tour: tour, racine, poste_a, poste_b };
        let c = m.coffre_a();
        c.poser("net-entreprises", NET, None).unwrap();
        c.poser("courriel", BOITE, Some("moi@exemple.com")).unwrap();
        c.poser_ouverture(LOGICIEL, None).unwrap();
        std::env::set_var("PUPPETPAIE_CLE", &m.poste_b);
        m
    }

    fn le_poste_n_ouvre_pas(m: &Monde) -> bool {
        let r = crate::compagnie::releve::mot_de_passe_de_consultation(&m.ctx(), "net-entreprises");
        r.is_err()
    }

    fn proteger_un_acte(m: &Monde) -> Result<String, String> {
        crate::compagnie::depot_acte::verifier_preuve(&m.ctx(), None, "2026-09", "depot REEL d'une DSN", "net-entreprises").map_err(|e| e.message)
    }

    // ------------------------------------------------------------ le mot de passe tape ouvre tout

    #[test]
    fn un_mauvais_mot_de_passe_n_ouvre_pas_et_ne_dit_rien_du_secret() {
        let m = monde();
        let mauvais = "Pas-le-bon-mot-de-passe-77";
        crate::compagnie::coffre::retenir_mot_de_passe_tape(mauvais);
        for r in [
            crate::compagnie::releve::mot_de_passe_de_consultation(&m.ctx(), "net-entreprises").map(|_| ()).map_err(|e| e.message),
            crate::compagnie::boite::mot_de_passe(&m.racine).map(|_| ()).map_err(|e| e.to_string()),
            crate::compagnie::depot_acte::motdepasse_organisme(&m.ctx(), "net-entreprises", None).map(|_| ()).map_err(|e| e.message),
        ] {
            let dit = r.expect_err("un mauvais mot de passe ne doit rien ouvrir");
            for secret in [NET, BOITE, LOGICIEL, mauvais] {
                assert!(!dit.contains(secret), "le refus ne doit nommer aucun mot de passe : {}", dit);
            }
        }
    }

    // ------------------------------------------------------------ l'acte reel : le moteur demande

    #[test]
    fn un_acte_reel_sans_demande_acceptee_est_refuse() {
        let m = monde();
        // Rien d'installe : la bibliotheque seule refuse, avec la phrase qui dit quoi faire.
        let dit = proteger_un_acte(&m).unwrap_err();
        assert!(dit.contains("aucun humain") && dit.contains("terminal"), "{}", dit);
        // La fenetre fermee refuse.
        let fermee = Rc::new(Fausse::ferme());
        {
            let _g = installer(fermee.clone());
            assert!(proteger_un_acte(&m).unwrap_err().contains("Fenetre fermee"));
        }
        assert_eq!(fermee.nombre(), 1);
        // Trois mauvais mots de passe refusent, et la demande s'arrete la.
        let mauvaise = Rc::new(Fausse::mot("Pas-le-bon-mot-de-passe-77"));
        {
            let _g = installer(mauvaise.clone());
            let dit = proteger_un_acte(&m).unwrap_err();
            assert!(dit.contains("refuse") && !dit.contains("Pas-le-bon"), "{}", dit);
        }
        assert_eq!(mauvaise.nombre(), ESSAIS as usize);
        assert!(le_poste_n_ouvre_pas(&m));
    }

    #[test]
    fn la_question_dit_ce_qui_part_et_ne_porte_aucun_secret() {
        let m = monde();
        let f = Rc::new(Fausse::mot(LOGICIEL));
        let _g = installer(f.clone());
        let acte = Acte::nouveau("envoi des documents", "2026-09", "3 salaries : Ana, Bo, Cy");
        assert!(autoriser_acte(&m.racine, &acte).est_accepte());
        let q = &f.questions()[0];
        let texte = format!("{} {}", q.titre, q.lignes.join(" | "));
        for attendu in ["envoi des documents", "2026-09", "Ana, Bo, Cy"] {
            assert!(texte.contains(attendu), "{}", texte);
        }
        assert!(!q.seance);
        for secret in [NET, BOITE, LOGICIEL] {
            assert!(!texte.contains(secret));
        }
    }

    #[test]
    fn ni_la_variable_ni_le_mot_de_passe_retenu_ni_la_seance_ne_prouvent_un_acte() {
        let m = monde();
        // Le mot de passe est LA, par les trois voies qui ouvrent le coffre : aucune ne vaut aval.
        std::env::set_var("PUPPETPAIE_MOTDEPASSE", LOGICIEL);
        crate::compagnie::coffre::retenir_mot_de_passe_tape(LOGICIEL);
        {
            let _g = installer(Rc::new(Fausse::mot(LOGICIEL)));
            assert!(ouvrir_seance(&m.racine).est_accepte());
        }
        assert!(proteger_un_acte(&m).unwrap_err().contains("aucun humain"));
        // Et la charge de la route, elle, reste une preuve (la fenetre Tk, la page) : le mot de passe tape y arrive.
        let preuve = crate::compagnie::depot_acte::Preuve { motdepasse: Some(LOGICIEL.into()), mois_recopie: None };
        assert!(crate::compagnie::depot_acte::verifier_preuve(&m.ctx(), Some(&preuve), "2026-09", "x", "y").is_ok());
        let mauvaise = crate::compagnie::depot_acte::Preuve { motdepasse: Some("Pas-le-bon-mot-de-passe-77".into()), mois_recopie: None };
        assert!(crate::compagnie::depot_acte::verifier_preuve(&m.ctx(), Some(&mauvaise), "2026-09", "x", "y").is_err());
    }

    // ------------------------------------------------------------ la seance

    #[test]
    fn la_seance_ouvre_le_coffre_mais_n_autorise_pas_un_acte_reel() {
        let m = monde();
        assert!(le_poste_n_ouvre_pas(&m));
        let f = Rc::new(Fausse::mot(LOGICIEL));
        {
            let _g = installer(f.clone());
            assert!(ouvrir_seance(&m.racine).est_accepte());
        }
        assert!(f.questions()[0].seance);
        // Plus rien de retenu en memoire : c'est la seance GARDEE (12 h) qui ouvre, dans un processus neuf.
        oublier_mot_de_passe_tape();
        let lu = crate::compagnie::releve::mot_de_passe_de_consultation(&m.ctx(), "net-entreprises").unwrap();
        assert_eq!(lu.as_deref(), Some(NET));
        assert_eq!(crate::compagnie::boite::mot_de_passe(&m.racine).unwrap(), BOITE);
        // Mais un acte reel, lui, redemande -- et sans humain, refuse.
        assert!(proteger_un_acte(&m).unwrap_err().contains("aucun humain"));
    }

    // ------------------------------------------------------------ sans humain

    fn tout_refuse(r: Reponse) -> String {
        match r {
            Reponse::Refus(p) => p,
            Reponse::Mot(_) => panic!("la demande aurait du refuser"),
        }
    }

    fn q() -> Question {
        question_de_l_acte(&Acte::nouveau("x", "2026-09", "y"), 1, false)
    }

    #[test]
    fn sans_ecran_ni_terminal_la_demande_refuse() {
        let sans = Reel { ecran: false, fenetre: None, terminal: None };
        assert_eq!(tout_refuse(sans.demander(&q())), SANS_HUMAIN);
        // Un ecran sans fenetre ni terminal ne suffit pas non plus.
        let ecran_seul = Reel { ecran: true, fenetre: None, terminal: None };
        assert_eq!(tout_refuse(ecran_seul.demander(&q())), SANS_HUMAIN);
        // Une fenetre sans ecran ne s'ouvre pas.
        let fenetre_sans_ecran = Reel { ecran: false, fenetre: Some(Box::new(Fausse::mot("x"))), terminal: None };
        assert_eq!(tout_refuse(fenetre_sans_ecran.demander(&q())), SANS_HUMAIN);
    }

}
