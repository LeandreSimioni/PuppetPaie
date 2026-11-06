//! OU VIVENT LES ENGAGEMENTS : `engagements/AAAA-MM.json`, le fichier du mois ou chaque engagement commence. Un
//! engagement est UN objet, qu'il dure un jour ou six mois ; il n'est ecrit que la. Ses paies n'y sont pas : chacune vit dans le
//! fichier de son mois (`paies`).
//!
//! Le fichier d'un mois (`FichierDuMois`) : le mois, le cachet par defaut du mois, que le calcul lit pour chaque
//! engagement du mois, et ses engagements ; le mois ne porte rien d'autre.
//!
//! UN ENGAGEMENT ECRIT (`Ecrit`) porte sa saisie (`Saisie`) et ce que le moteur tient seul : sa validation, celle
//! qu'une modification va remplacer, ses versions passees (`etats`), pour qui son numero de contrat a ete donne
//! (`numeros`), qu'il n'est plus actif, ce que l'import a lu de lui (`importe`). SA SAISIE est la carte de son premier
//! mois, telle que l'ecran l'ecrit ; ses avenants : ce qui definit le contrat (ses termes, `saisie::Groupe::Terme`) vaut
//! jusqu'a ce qu'il change, et un changement s'ecrit date (`Avenant`), valant de son jour au changement suivant ; et les
//! faits de chaque mois d'apres le premier (ses lignes, ses conges pris, ses primes). Ce que l'engagement dit d'un mois,
//! sa VUE (`Objet::vue`), se lit a la date : chaque terme a la fin du mois, les faits du mois ; un mois sans faits d'un
//! contrat d'usage prend les jours que son planning y met. Le salarie, les bornes et le numero du contrat sont ceux de
//! l'engagement (`saisie::Groupe::Partage`), dans tous ses mois.
//!
//! LE MOIS DE L'ECRAN (`vue_du_mois`) montre les engagements qui le touchent : ceux de son fichier, et ceux des fichiers
//! d'avant qui courent encore : ce que les calculs lisent. Une carte de l'ecran ecrit son engagement, et lui seul
//! (`ecrire_la_carte`).
//!
//! L'identite d'un engagement : son salarie et son premier jour, et son rang
//! parmi ceux du meme salarie et du meme jour (`identites`) : deux contrats d'usage du meme jour. Un contrat au mois
//! n'en a qu'un par salarie et par jour (`refuser_un_second_contrat_au_mois`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use socle::forme::NombreLu;

use droit::contexte::Contexte;
use crate::saisie::{Avant, Carte, Champ, Groupe, VueDuMois};
use socle::date::Jour;
use socle::mois::Mois;
use socle::num::Resultat;

/// Le dossier des engagements, sous les donnees de la compagnie.
pub const DOSSIER: &str = "engagements";

/// La phrase d'un fichier d'engagements qui n'a pas sa forme.
const ABIME: &str = "Un mois d'engagements ne se relit pas : il est abîmé.";

/// Les deux dossiers que le stock tient, un fichier par mois : les engagements (`DOSSIER`), et les paies du mois
/// (`paies::DOSSIER`), que seul `paies` lit et ecrit. Un brouillon (`Brouillon`) les tient tous les deux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Dossier {
    Engagements,
    Paies,
}

impl Dossier {
    fn nom(self) -> &'static str {
        match self {
            Dossier::Engagements => DOSSIER,
            Dossier::Paies => crate::paies::DOSSIER,
        }
    }
}

// ------------------------------------------------------------------ les formes

/// LE FICHIER D'ENGAGEMENTS D'UN MOIS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichierDuMois {
    pub mois: Mois,
    pub taux_cachet: Option<NombreLu>,
    pub engagements: Vec<Ecrit>,
}

/// UN ENGAGEMENT ECRIT : sa saisie, et ce que le moteur tient seul.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecrit {
    pub saisie: Saisie,
    /// Sa validation (`etats`).
    pub valide: Option<Validation>,
    /// La validation qu'une modification va remplacer (`etats::modifier`).
    pub precedente: Option<Validation>,
    /// Ses versions passees, de la plus ancienne a la plus recente (`etats`).
    pub versions: Vec<VersionPassee>,
    /// Pour qui son numero de contrat a ete donne (`numeros`).
    pub numero_pour: Option<crate::numeros::NumeroPour>,
    /// Il n'est plus actif (`etats::desactiver`) : il ne touche plus aucun mois (`Objet::couvre`), ne se calcule plus,
    /// n'entre dans aucun document ni aucune declaration ; il se lit (`non_actifs_du_mois`).
    pub non_actif: Option<NonActif>,
    /// Ce que l'import a lu de lui (`importe::Pieces`).
    pub importe: Option<crate::importe::Pieces>,
}

/// LA SAISIE D'UN ENGAGEMENT : la carte de son premier mois, ses avenants dates, les faits de ses mois d'apres. C'est
/// aussi ce qu'une validation garde (`Validation::saisie`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saisie {
    pub carte: Carte,
    /// Les termes qui changent apres le premier mois, du plus ancien au plus recent.
    pub avenants: Vec<Avenant>,
    /// Ce que chaque mois d'apres le premier dit de lui seul (`Groupe::Fait`, `Groupe::Fin`).
    pub faits: BTreeMap<Mois, Carte>,
}

/// UN AVENANT : les termes qui changent a partir du jour `depuis`, et ceux qui se retirent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Avenant {
    pub depuis: Jour,
    pub termes: Carte,
    pub retires: Vec<Champ>,
}

impl Avenant {
    /// Il ne change rien.
    fn vide(&self) -> bool {
        self.retires.is_empty() && self.termes.muette()
    }
}

/// UNE VALIDATION : quand, sa version, et la saisie validee (`None` : comptee comme valide par le calcul qui l'ecrit,
/// `etats::calculer_les_rangs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validation {
    pub le: String,
    pub version: Option<u64>,
    pub saisie: Option<Saisie>,
}

/// UNE VERSION PASSEE : son numero, quand elle avait ete validee et quand elle a ete remplacee, sa saisie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionPassee {
    pub version: u64,
    pub valide_le: String,
    pub remplacee_le: String,
    pub saisie: Option<Saisie>,
}

/// UNE VERSION DE L'ENGAGEMENT, telle qu'il l'ecrit : la presente (sa validation, ou celle qu'une modification va
/// remplacer) ou une passee.
#[derive(Debug, Clone, Copy)]
pub enum Version<'a> {
    Presente(&'a Validation),
    Passee(&'a VersionPassee),
}

impl<'a> Version<'a> {
    /// Son numero ; `None` pour une validation qui ne le porte pas (`etats::a_revalider`).
    pub fn numero(&self) -> Option<u64> {
        match self {
            Version::Presente(v) => v.version,
            Version::Passee(v) => Some(v.version),
        }
    }

    /// Quand elle a ete validee.
    pub fn valide_le(&self) -> Option<&'a str> {
        match self {
            Version::Presente(v) => Some(v.le.as_str()),
            Version::Passee(v) => Some(v.valide_le.as_str()),
        }
    }

    /// La saisie qu'elle a validee.
    pub fn saisie(&self) -> Option<&'a Saisie> {
        match self {
            Version::Presente(v) => v.saisie.as_ref(),
            Version::Passee(v) => v.saisie.as_ref(),
        }
    }
}

impl Ecrit {
    /// Toutes ses versions ecrites : la validation, celle qu'une modification va remplacer, puis les passees.
    pub fn toutes_les_versions(&self) -> impl Iterator<Item = Version<'_>> {
        self.valide.iter().chain(self.precedente.iter()).map(Version::Presente).chain(self.versions.iter().map(Version::Passee))
    }

    /// La version `n`, presente ou passee.
    pub fn version(&self, n: u64) -> Option<Version<'_>> {
        self.toutes_les_versions().find(|v| v.numero() == Some(n))
    }
}

/// UN ENGAGEMENT QUI N'EST PLUS ACTIF : quand, et sa version (celle qui suit la derniere validee : rien de paye).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonActif {
    pub le: String,
    pub version: u64,
}

/// Le chemin d'un engagement dans un refus : son fichier, son rang.
#[derive(Serialize)]
struct OuDansLeFichier {
    chemin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rang: Option<usize>,
}

// ------------------------------------------------------------------ les fichiers

/// Le fichier des engagements qui commencent en `mois`.
pub fn chemin(racine: &Path, mois: &str) -> PathBuf {
    chemin_dans(racine, Dossier::Engagements, mois)
}

/// Le fichier de `mois` dans le dossier `d`.
pub(crate) fn chemin_dans(racine: &Path, d: Dossier, mois: &str) -> PathBuf {
    racine.join(d.nom()).join(format!("{}.json", mois))
}

/// Les mois qui ont un fichier d'engagements, du plus ancien au plus recent, ceux du brouillon ouvert compris.
pub fn fichiers(racine: &Path) -> Vec<String> {
    fichiers_dans(racine, Dossier::Engagements)
}

/// Les mois qui ont un fichier dans le dossier `d`, du plus ancien au plus recent, ceux du brouillon ouvert compris.
pub(crate) fn fichiers_dans(racine: &Path, d: Dossier) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(racine.join(d.nom()))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".json").filter(|m| socle::mois::Mois::lire(m).is_ok()).map(String::from))
        .collect();
    BROUILLON.with(|b| v.extend(b.borrow().iter().filter(|(r, _)| r == racine).flat_map(|(_, f)| f.keys().filter(|(x, _)| *x == d).map(|(_, m)| m.clone()))));
    v.sort();
    v.dedup();
    v
}

/// LA SEULE LECTURE D'UN FICHIER D'ENGAGEMENTS, par sa forme ; `None` s'il n'existe pas.
pub fn lire_fichier(racine: &Path, mois: &str) -> Resultat<Option<FichierDuMois>> {
    Ok(lire_partage(racine, mois)?.map(|f| (*f).clone()))
}

/// UN FICHIER DU STOCK, l'un des deux dossiers qu'il tient (`Dossier`) : sa forme, que le brouillon tient telle quelle
/// (`Tenu`), et la phrase du refus d'un fichier qui ne l'a pas.
pub(crate) trait FichierDuStock: Clone + Serialize + serde::de::DeserializeOwned + Send + Sync + 'static {
    const DOSSIER: Dossier;
    const ILLISIBLE: &'static str;
    fn tenu(f: std::sync::Arc<Self>) -> Tenu;
    fn de(t: &Tenu) -> Option<std::sync::Arc<Self>>;
}

/// Un fichier que le brouillon tient, en sa forme.
#[derive(Clone)]
pub(crate) enum Tenu {
    Engagements(std::sync::Arc<FichierDuMois>),
    Paies(std::sync::Arc<crate::paies::FichierDesPaies>),
}

impl FichierDuStock for FichierDuMois {
    const DOSSIER: Dossier = Dossier::Engagements;
    const ILLISIBLE: &'static str = ABIME;
    fn tenu(f: std::sync::Arc<Self>) -> Tenu {
        Tenu::Engagements(f)
    }
    fn de(t: &Tenu) -> Option<std::sync::Arc<Self>> {
        match t {
            Tenu::Engagements(f) => Some(f.clone()),
            Tenu::Paies(_) => None,
        }
    }
}

thread_local! {
    /// Les fichiers ecrits dans le brouillon ouvert (`Brouillon`), pour sa racine : chacun par son dossier et son mois.
    static BROUILLON: std::cell::RefCell<Option<(PathBuf, BTreeMap<(Dossier, String), Tenu>)>> = const { std::cell::RefCell::new(None) };
}

/// UN BROUILLON : tant qu'il est ouvert, ce qui s'ecrit dans les engagements de `racine` et dans leurs paies (`reecrire`,
/// `paies`) reste en memoire, et toute lecture le voit. `ecrire` le pose sur le disque ; tombe sans, il n'a rien ecrit.
pub struct Brouillon(PathBuf);

impl Brouillon {
    pub fn ouvrir(racine: &Path) -> Brouillon {
        BROUILLON.with(|b| *b.borrow_mut() = Some((racine.to_path_buf(), BTreeMap::new())));
        Brouillon(racine.to_path_buf())
    }

    /// Ce que le brouillon tient s'ecrit sur le disque.
    pub fn ecrire(self) -> Resultat<()> {
        // Un brouillon deja ecrit ne tient plus rien.
        let fichiers = BROUILLON.with(|b| b.borrow_mut().take()).into_iter().flat_map(|(_, f)| f);
        for ((_, mois), contenu) in fichiers {
            match contenu {
                Tenu::Engagements(f) => poser_dans(&self.0, &mois, &*f)?,
                Tenu::Paies(f) => poser_dans(&self.0, &mois, &*f)?,
            };
        }
        Ok(())
    }

    /// Le fichier de `mois` de la forme `T` tel que le brouillon ouvert pour `racine` le tient.
    fn lu<T: FichierDuStock>(racine: &Path, mois: &str) -> Option<std::sync::Arc<T>> {
        BROUILLON.with(|b| b.borrow().as_ref().filter(|(r, _)| r == racine).and_then(|(_, f)| f.get(&(T::DOSSIER, mois.to_string())).and_then(T::de)))
    }

    /// Ecrit `contenu` comme fichier de `mois` dans le brouillon ouvert pour `racine`, tel que le disque le relirait (ce
    /// que sa forme n'ecrit pas, il ne le tient pas) ; faux : aucun ne l'est.
    fn tient<T: FichierDuStock>(racine: &Path, mois: &str, contenu: &T) -> Resultat<bool> {
        if !BROUILLON.with(|b| b.borrow().as_ref().is_some_and(|(r, _)| r == racine)) {
            return Ok(false);
        }
        let relu: T = socle::forme::lire_texte(&socle::ecriture::json_en_ligne(contenu), T::ILLISIBLE)?;
        BROUILLON.with(|b| {
            if let Some((_, f)) = b.borrow_mut().as_mut() {
                f.insert((T::DOSSIER, mois.to_string()), T::tenu(std::sync::Arc::new(relu)));
            }
        });
        Ok(true)
    }
}

impl Drop for Brouillon {
    fn drop(&mut self) {
        BROUILLON.with(|b| *b.borrow_mut() = None);
    }
}

/// LA SEULE ECRITURE D'UN FICHIER D'ENGAGEMENTS : `contenu` devient le fichier de son mois (`poser_dans`).
pub(crate) fn poser(racine: &Path, contenu: &FichierDuMois) -> Resultat<PathBuf> {
    poser_dans(racine, &contenu.mois.to_string(), contenu)
}

/// LE SEUL ENDROIT QUI ECRIT UN FICHIER D'ENGAGEMENTS OU DE PAIES : `contenu` devient le fichier de `mois` de son dossier,
/// dans le brouillon ouvert pour `racine` s'il y en a un (`Brouillon`), sinon sur le disque. Rend son chemin.
pub(crate) fn poser_dans<T: FichierDuStock>(racine: &Path, mois: &str, contenu: &T) -> Resultat<PathBuf> {
    let p = chemin_dans(racine, T::DOSSIER, mois);
    if Brouillon::tient(racine, mois, contenu)? {
        return Ok(p);
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| socle::fichiers::panne_fichier(&e, dir))?;
    }
    socle::ecriture::ecrire_json(&p, contenu, 2)?;
    Ok(p)
}

/// LE FICHIER D'ENGAGEMENTS DE `mois` QUI N'A PLUS RIEN (un fichier que l'import avait pose) s'en va : un mois sans
/// fichier ni engagement n'a pas de saisie. Dans un brouillon, il s'ecrit vide.
pub(crate) fn retirer(racine: &Path, mois: &str) -> Resultat<PathBuf> {
    let vide = FichierDuMois { mois: Mois::lire(mois)?, taux_cachet: None, engagements: Vec::new() };
    if Brouillon::tient(racine, mois, &vide)? {
        return Ok(chemin(racine, mois));
    }
    let p = chemin(racine, mois);
    socle::ecriture::effacer(&p)?;
    Ok(p)
}

/// Le fichier d'engagements d'un mois (`lire_dans`) ; un refus dit son chemin.
fn lire_partage(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<FichierDuMois>>> {
    lire_dans(racine, mois).map_err(|m| socle::echec::avec(m, OuDansLeFichier { chemin: format!("{}/{}.json", DOSSIER, mois), rang: None }))
}

/// Le fichier de `mois` de la forme `T` tel que le geste l'a lu ou ecrit (`socle::forme::partagee`), ou tel que le
/// brouillon ouvert le tient : chaque lecture des engagements et des paies passe par lui. `None` s'il n'existe pas.
pub(crate) fn lire_dans<T: FichierDuStock>(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<T>>> {
    if let Some(f) = Brouillon::lu::<T>(racine, mois) {
        return Ok(Some(f));
    }
    socle::forme::partagee(&chemin_dans(racine, T::DOSSIER, mois), T::ILLISIBLE)
}

/// Le fichier de `mois` ne porte-t-il que des engagements importes seuls (`Objet::importe_seul`) ? Un tel mois n'a pas de
/// saisie : il se lit. Un fichier sans engagement n'est pas de l'import ; un fichier illisible se refuse.
fn importes_seuls(racine: &Path, mois: &str) -> Resultat<bool> {
    let (_, t) = tous(racine)?;
    let mut es = t.objets.iter().filter(|o| o.fichier == mois).peekable();
    Ok(es.peek().is_some() && es.all(Objet::importe_seul))
}

// ------------------------------------------------------------------ l'objet

/// Un engagement tel qu'il est ecrit : la compagnie, le mois de son fichier (son premier mois), son rang dans le
/// fichier, et l'engagement lui-meme, partage : le copier ne recopie pas ce qu'il porte (ses versions) ; on ne recopie
/// que ce qu'on change (`brut_mut`).
#[derive(Debug, Clone)]
pub struct Objet {
    pub racine: PathBuf,
    pub fichier: String,
    pub rang: usize,
    /// L'engagement ecrit. TRAVAIL A FAIRE (chaine A) : il se nomme `ecrit` quand `conges.rs` (l. 468-469) ne le lit
    /// plus sous ce nom.
    pub brut: std::sync::Arc<Ecrit>,
}

impl Objet {
    /// La carte de son premier mois.
    pub fn carte(&self) -> &Carte {
        &self.brut.saisie.carte
    }

    /// L'engagement a changer : recopie s'il est partage.
    pub fn brut_mut(&mut self) -> &mut Ecrit {
        std::sync::Arc::make_mut(&mut self.brut)
    }

    /// Ce que l'import a lu de lui, s'il est importe.
    pub fn pieces(&self) -> Option<&crate::importe::Pieces> {
        self.brut.importe.as_ref()
    }

    /// Le premier mois de l'engagement : celui de son fichier.
    pub fn premier_mois(&self) -> &str {
        &self.fichier
    }

    /// Le dernier mois d'un engagement borne (celui de sa fin) ; `None` pour un contrat sans terme.
    pub fn dernier_mois(&self) -> Resultat<Option<String>> {
        let c = self.carte();
        let mensuel = self.mensuel()?;
        match c.fin.as_deref().filter(|f| !f.is_empty()) {
            Some(f) => Ok(Some(Jour::lire(f)?.mois_civil().to_string().max(self.fichier.clone()))),
            None if !mensuel => {
                // Un contrat d'usage sans fin dite finit a son dernier jour joue ou planifie ; sans jour encore, a son
                // premier mois : seul un contrat paye au mois court sans terme.
                let mut jours: Vec<String> = c.dates().iter().chain(c.planning().iter()).map(|j| j.jour().to_string()).collect();
                jours.sort();
                let dernier = jours.last().map(|j| j.chars().take(7).collect::<String>()).filter(|m| socle::mois::Mois::lire(m).is_ok());
                Ok(Some(dernier.into_iter().fold(self.fichier.clone(), String::max)))
            }
            None => Ok(None),
        }
    }

    /// Un contrat paye au mois ?
    pub fn mensuel(&self) -> Resultat<bool> {
        self.carte().mensuelle()
    }

    /// Les mois que la saisie a touches : le premier, ceux qui ont des faits, et ceux ou un terme change.
    pub fn mois_saisis(&self) -> Resultat<Vec<Mois>> {
        let premier = Mois::lire(&self.fichier)?;
        let mut v = vec![premier];
        v.extend(self.brut.saisie.faits.keys().copied().filter(|m| *m > premier));
        v.extend(self.avenants().iter().map(|a| Mois::du_jour(&a.depuis)).filter(|m| *m > premier));
        v.sort();
        v.dedup();
        Ok(v)
    }

    /// L'engagement touche-t-il `mois` ? Du premier mois a celui de sa fin ; sans terme, de son premier mois a toujours.
    /// Un engagement non actif ne touche plus aucun mois ; un engagement importe seul (`importe_seul`) n'en touche aucun de la saisie : il se
    /// lit (`importe`), rien ne le calcule.
    pub fn couvre(&self, mois: &str) -> Resultat<bool> {
        Ok(!self.est_non_actif() && !self.importe_seul() && self.dans_ses_bornes(mois)?)
    }

    /// Est-il importe ? Ses mois repris le sont ; il peut continuer ici (`importe_seul`).
    pub fn est_importe(&self) -> bool {
        self.brut.importe.is_some()
    }

    /// N'est-il qu'importe : tous ses mois sont repris d'un ancien logiciel, et il ne touche aucun mois d'ici. Un contrat
    /// de permanent qui continue ici vit dans le fichier de son premier mois neuf, apres tous ses mois verses
    /// (`importe::ecrire_tous`) : ce mois et les suivants sont de la saisie.
    pub fn importe_seul(&self) -> bool {
        self.pieces().is_some_and(|p| p.paies.keys().max().is_none_or(|m| m.as_str() >= self.fichier.as_str()))
    }

    /// N'est-il plus actif ?
    pub fn est_non_actif(&self) -> bool {
        self.brut.non_actif.is_some()
    }

    /// `mois` est-il entre son premier mois et celui de sa fin, qu'il soit actif ou non ?
    fn dans_ses_bornes(&self, mois: &str) -> Resultat<bool> {
        if mois < self.fichier.as_str() {
            return Ok(false);
        }
        Ok(match self.dernier_mois()? {
            Some(d) => mois <= d.as_str(),
            None => true,
        })
    }

    /// Les mois de l'engagement jusqu'a `jusqu_a` : un engagement borne les a tous ; un contrat sans terme, ceux que la
    /// saisie a touches et ceux qui vont jusqu'a `jusqu_a`.
    pub fn mois_couverts(&self, jusqu_a: &str) -> Resultat<Vec<String>> {
        let fin = match self.dernier_mois()? {
            Some(d) => d,
            None => self.mois_saisis()?.last().map(Mois::to_string).into_iter().fold(jusqu_a.to_string().max(self.fichier.clone()), String::max),
        };
        Ok(Mois::lire(&self.fichier)?.jusqu_a(Mois::lire(&fin)?).iter().map(Mois::to_string).collect())
    }

    /// Valide ?
    pub fn est_valide(&self) -> bool {
        self.brut.valide.is_some()
    }

    /// Ce que l'engagement dit de `mois` : la saisie de ce mois-la, telle que l'ecran la montre et que le calcul la lit
    /// (`None` s'il ne le touche pas). Un engagement non actif dit encore ce qu'il etait, pour qu'il se lise. Elle se lit a
    /// la date, sans relire les mois d'entre-deux : la carte du premier mois, ou, apres lui, ses termes et les faits du
    /// mois ; chaque terme y prend sa valeur au dernier jour du mois (`termes_au`).
    ///
    /// Un contrat paye au mois dont un terme change dans `mois` y porte la date de l'avenant (`date_modification`) : le
    /// document qui dit la modification la date de sa prise d'effet (code du travail R1221-40,
    /// `atelier/reference/sources/code-du-travail/articles-20261029.md`, l. 856), et la DSN date de meme le bloc
    /// S21.G00.41 (41.001). Un changement en cours de mois y porte aussi les termes de la veille (`avant` : le salaire et
    /// la duree du travail, `saisie::Avant`), qui paient ses jours d'avant (`periodes::bornes`, chaque part par
    /// `droit::periodes::part_du_mois`).
    pub fn vue(&self, mois: &str) -> Resultat<Option<Carte>> {
        if !self.dans_ses_bornes(mois)? {
            return Ok(None);
        }
        let premiere = self.carte();
        let mensuel = self.mensuel()?;
        let mut v = if mois == self.fichier {
            premiere.clone()
        } else {
            let mut v = premiere.du_groupe(&[Groupe::Terme, Groupe::Partage]);
            match self.faits_de(mois)? {
                Some(f) => v.recouvrir(f),
                None if !mensuel => v.lignes = Some(self.lignes_du_planning(mois)?),
                None => {}
            }
            v
        };
        self.termes_au(&mut v, &Mois::lire(mois)?.dernier_jour());
        if mois > self.fichier.as_str() && mensuel {
            self.avenant_du_mois(&mut v, mois)?;
        }
        for k in Champ::des(&[Groupe::Partage]) {
            v.prendre(premiere, k);
        }
        if mois > self.fichier.as_str() && !mensuel {
            // Un contrat d'usage qui ne dit pas son debut commence a son premier jour joue, celui de son premier mois :
            // les mois suivants le declarent (S21.G00.40.001) et ne le prennent pas pour une embauche.
            let debut = crate::debut_brut(&self.brut);
            if v.debut.is_none() && !debut.is_empty() {
                v.debut = Some(debut);
            }
            v.jours_contrat = Some(NombreLu::from(premiere.jours_du_contrat() as u64));
        }
        Ok(Some(v))
    }

    /// Les faits que `mois` (apres le premier) dit de lui seul, s'il en dit.
    fn faits_de(&self, mois: &str) -> Resultat<Option<&Carte>> {
        Ok(self.brut.saisie.faits.get(&Mois::lire(mois)?))
    }

    /// Ses avenants, du plus ancien au plus recent.
    pub fn avenants(&self) -> &[Avenant] {
        &self.brut.saisie.avenants
    }

    /// Pose dans `v` chaque avenant venu au plus tard le `jour`.
    fn termes_au(&self, v: &mut Carte, jour: &Jour) {
        for a in self.avenants().iter().filter(|a| a.depuis <= *jour) {
            v.recouvrir(&a.termes);
            for c in &a.retires {
                v.ote(*c);
            }
        }
    }

    /// Les termes du contrat au `jour` : ceux du premier mois, et chaque avenant venu jusqu'a lui.
    fn termes_a(&self, jour: &Jour) -> Carte {
        let mut v = self.carte().du_groupe(&[Groupe::Terme]);
        self.termes_au(&mut v, jour);
        v
    }

    /// L'avenant de `mois` dans sa vue `v` (voir `vue`) : le dernier jour du mois ou un terme change, et, s'il n'est pas
    /// le 1er, les termes de la veille quand le salaire ou la duree du travail changent (l'avenant change autre chose :
    /// il se date, rien ne se coupe).
    fn avenant_du_mois(&self, v: &mut Carte, mois: &str) -> Resultat<()> {
        let Some(j) = self.avenants().iter().map(|a| a.depuis).filter(|j| j.mois_civil().to_string() == mois).max() else { return Ok(()) };
        v.date_modification = Some(j.iso());
        if j.jour == 1 {
            return Ok(());
        }
        let a = self.termes_a(&j.plus_jours(-1)?);
        let quotite = |x: &Carte| -> Resultat<Option<NombreLu>> {
            Ok(match &x.quotite_contrat {
                Some(q) => Some(q.clone()),
                None => x.nature_lue()?.quotite_reference_defaut().and_then(NombreLu::de_f64),
            })
        };
        let avant = Avant { salaire_mensuel: a.salaire_mensuel.clone(), quotite_contrat: quotite(&a)? };
        let apres = Avant { salaire_mensuel: v.salaire_mensuel.clone(), quotite_contrat: quotite(v)? };
        if socle::ecriture::canonique(&avant) != socle::ecriture::canonique(&apres) {
            v.avant = Some(avant);
        }
        Ok(())
    }

    /// LES LIGNES D'UN MOIS SANS FAITS d'un contrat d'usage : les jours que les lignes du dernier mois qui en dit (le
    /// premier, ou un mois qui a ses faits) ont mis au planning de `mois` ; une ligne par ligne qui en a, a la meme
    /// rubrique, au meme taux et au meme lieu, ses jours de `mois` en dates et ceux d'apres au planning ; un cachet, une
    /// journee ou un service par date (une autre unite, des heures, ne se deduit pas d'un nombre de jours : quantite 0, a
    /// completer). Sans planning, aucune ligne.
    fn lignes_du_planning(&self, mois: &str) -> Resultat<Vec<crate::LignePaie>> {
        let faits = &self.brut.saisie.faits;
        let dit = faits.keys().filter(|m| m.to_string().as_str() < mois && m.to_string().as_str() > self.fichier.as_str()).max();
        let source = match dit {
            Some(m) => faits[m].lignes(),
            None => self.carte().lignes(),
        };
        crate::lignes_du_planning(&self.racine, source, mois)
    }
}

/// Tous les engagements, fichier par fichier, du plus ancien au plus recent, dans l'ordre de chaque fichier.
pub fn objets(racine: &Path) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.objets.clone())
}

/// Les fichiers des engagements d'une compagnie tels qu'ils viennent d'etre lus : chaque mois, et ce que son fichier
/// contient (`socle::forme::partagee` : la meme forme tant que les octets sont les memes).
type Lus = Vec<(String, std::sync::Arc<FichierDuMois>)>;

/// Deux lectures ont-elles trouve exactement les memes fichiers, aux memes octets ?
fn memes(a: &Lus, b: &Lus) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((m, x), (n, y))| m == n && std::sync::Arc::ptr_eq(x, y))
}

fn lus(racine: &Path) -> Resultat<Lus> {
    let mut out = Vec::new();
    for f in fichiers(racine) {
        if let Some(v) = lire_partage(racine, &f)? {
            out.push((f, v));
        }
    }
    Ok(out)
}

/// Les engagements d'une compagnie tires de ses fichiers, et les bornes de chacun (son dernier mois), tirees avec lui :
/// lire un mois ne relit que les objets qui le touchent.
struct Tires {
    objets: Vec<Objet>,
    derniers: Vec<Result<Option<String>, String>>,
}

impl Tires {
    /// `mois` est-il entre le premier et le dernier mois de l'objet de rang `i` ?
    fn dans_ses_bornes(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        if mois < o.fichier.as_str() {
            return Ok(false);
        }
        Ok(match &self.derniers[i] {
            Ok(Some(d)) => mois <= d.as_str(),
            Ok(None) => true,
            Err(e) => return Err(e.clone()),
        })
    }

    /// L'objet de rang `i` touche-t-il `mois` (`Objet::couvre`) ?
    fn couvre(&self, i: usize, mois: &str) -> Resultat<bool> {
        let o = &self.objets[i];
        Ok(!o.est_non_actif() && !o.importe_seul() && self.dans_ses_bornes(i, mois)?)
    }

    /// Les objets qui touchent `mois`, dans l'ordre des fichiers.
    fn du_mois(&self, mois: &str) -> Resultat<Vec<&Objet>> {
        let mut out = Vec::new();
        for (i, o) in self.objets.iter().enumerate() {
            if o.fichier.as_str() <= mois && self.couvre(i, mois)? {
                out.push(o);
            }
        }
        Ok(out)
    }
}

/// Un verrou qu'un fil a lache en tombant se reprend tel qu'il est : ce qu'il garde n'est qu'une lecture deja faite.
fn verrouille<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// TOUS LES ENGAGEMENTS D'UNE COMPAGNIE, tires une fois des memes fichiers : ce qu'on en avait tire ne sert que si ce
/// sont les memes lectures (`memes`) ; pendant un geste, les fichiers se lisent une fois (`socle::forme::geste`).
/// Partage : on n'y touche pas, et un objet se copie sans recopier ce qu'il porte (`Objet`).
fn tous(racine: &Path) -> Resultat<(Lus, std::sync::Arc<Tires>)> {
    type Gardes = std::sync::Mutex<std::collections::HashMap<PathBuf, (Lus, std::sync::Arc<Tires>)>>;
    static GARDES: std::sync::OnceLock<Gardes> = std::sync::OnceLock::new();
    let gardes = GARDES.get_or_init(Default::default);
    let fichiers = lus(racine)?;
    let avant = verrouille(gardes).get(racine).cloned();
    if let Some((vus, v)) = &avant {
        if memes(vus, &fichiers) {
            return Ok((fichiers, v.clone()));
        }
    }
    let mut objets = Vec::new();
    for (f, m) in &fichiers {
        // Les engagements d'un fichier qui n'a pas change depuis la lecture gardee sont ceux qu'elle a tires.
        let inchange = avant.as_ref().filter(|(vus, _)| vus.iter().any(|(n, y)| n == f && std::sync::Arc::ptr_eq(m, y)));
        if let Some((_, v)) = inchange {
            objets.extend(v.objets.iter().filter(|o| o.fichier == *f).cloned());
            continue;
        }
        for (rang, e) in m.engagements.iter().enumerate() {
            objets.push(Objet { racine: racine.to_path_buf(), fichier: f.clone(), rang, brut: std::sync::Arc::new(e.clone()) });
        }
    }
    let derniers = objets.iter().map(Objet::dernier_mois).collect();
    let out = std::sync::Arc::new(Tires { objets, derniers });
    socle::fichiers::ranger(gardes, racine.to_path_buf(), (fichiers.clone(), out.clone()));
    Ok((fichiers, out))
}

/// Les engagements qui touchent `mois`, dans l'ordre de l'ecran : ceux des fichiers d'avant qui courent encore, puis
/// ceux du mois.
pub fn objets_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    Ok(tous(racine)?.1.du_mois(mois)?.into_iter().cloned().collect())
}

/// L'IDENTITE D'UN ENGAGEMENT A L'ECRAN, ce par quoi sa carte le designe : son salarie et son premier jour tels que
/// l'engagement les ecrit (le debut est vide tant qu'il n'a aucun jour : `debut_brut`), et son rang parmi ceux du meme
/// salarie qui commencent le meme jour (l'ordre de leur fichier, qui est le meme pour tous : le fichier de leur premier
/// mois). Le moteur la rend a la page (`identites`), la page la renvoie telle quelle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identite {
    pub salarie: String,
    pub debut: String,
    pub rang: usize,
}

/// L'IDENTITE DE CHAQUE ENGAGEMENT D'UNE LISTE (le plan), dans son ordre.
pub fn identites(objets: &[Objet]) -> Vec<Identite> {
    // Une carte neuve, qui ne nomme pas encore son salarie, a pour identite un salarie vide.
    let paires: Vec<(String, String)> =
        objets.iter().map(|o| (o.carte().salarie.as_ref().map(ToString::to_string).unwrap_or_default(), crate::debut_brut(&o.brut))).collect();
    paires
        .iter()
        .enumerate()
        .map(|(i, (s, d))| Identite { salarie: s.clone(), debut: d.clone(), rang: paires[..i].iter().filter(|(s2, d2)| s2 == s && d2 == d).count() })
        .collect()
}

/// L'ENGAGEMENT QUE DESIGNE L'IDENTITE `id` (`identites`) dans l'ecran de `mois`, et sa place dans cet ecran (l'ordre
/// de `vue_du_mois`). Refus : une identite que l'ecran ne montre pas.
pub fn designe(racine: &Path, mois: &str, id: &Identite) -> Resultat<(usize, Objet)> {
    let objets = objets_du_mois(racine, mois)?;
    let k = identites(&objets).iter().position(|x| x == id).ok_or_else(|| {
        format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", socle::mois::Mois::en_clair(mois))
    })?;
    Ok((k, objets.into_iter().nth(k).expect("la place vient de la liste")))
}

/// Les engagements non actifs qui toucheraient `mois` s'ils l'etaient encore : ce qui se lit d'eux.
pub fn non_actifs_du_mois(racine: &Path, mois: &str) -> Resultat<Vec<Objet>> {
    let t = tous(racine)?.1;
    let mut out = Vec::new();
    for (i, o) in t.objets.iter().enumerate() {
        if o.est_non_actif() && o.fichier.as_str() <= mois && t.dans_ses_bornes(i, mois)? {
            out.push(o.clone());
        }
    }
    Ok(out)
}

/// Le fichier de l'objet `o`, relu ; refus s'il a disparu.
fn fichier_de(racine: &Path, o: &Objet) -> Resultat<FichierDuMois> {
    lire_fichier(racine, &o.fichier)?.ok_or_else(|| format!("{} : le fichier de l'engagement a disparu", o.fichier))
}

/// Efface l'objet `o` de son fichier (meme fichier, meme rang).
pub fn effacer(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = fichier_de(racine, o)?;
    if contenu.engagements.get(o.rang) != Some(&*o.brut) {
        return Err(format!("{} : l'engagement {} a change sur le disque ; rien n'est efface", o.fichier, o.rang + 1));
    }
    contenu.engagements.remove(o.rang);
    poser(racine, &contenu)
}

/// LE MOIS DE L'ECRAN (`VueDuMois`) : le mois, ce que son fichier dit par defaut, et la vue de chaque engagement qui le
/// touche ; `None` quand aucun engagement ne le touche et qu'il n'a pas de fichier. Un engagement valide y dit quand
/// (`valide` : `{"le"}`).
pub fn vue_du_mois(racine: &Path, mois: &str) -> Resultat<Option<VueDuMois>> {
    Ok(vue_et_empreinte(racine, mois)?.map(|s| {
        socle::fichiers::noter_valeur(&cle_de_saisie(mois, None), s.1.clone());
        s.0.clone()
    }))
}

/// L'empreinte de la saisie de `mois` (`empreinte_de_saisie`) : celle que `vue_du_mois` note, ou, pour `salarie`, celle
/// que la paie de ses engagements du mois note ; `None` sans saisie.
pub fn empreinte_de_la_saisie_du_mois(racine: &Path, mois: &str, salarie: Option<&str>) -> Resultat<Option<String>> {
    Ok(vue_et_empreinte(racine, mois)?.map(|s| if salarie.is_none() { s.1.clone() } else { empreinte_de_saisie(&s.0, salarie) }))
}

/// La vue de `mois` et son empreinte, faites une fois pour les memes fichiers aux memes octets (`memes`) :
/// chaque paie qui se verifie relit la saisie des mois qu'elle a lus. La vue d'une ligne lit aussi l'unite de sa rubrique
/// (`droit::catalogue::rubrique`) : les deux catalogues en font partie.
pub(crate) fn vue_et_empreinte(racine: &Path, mois: &str) -> Resultat<Option<std::sync::Arc<(VueDuMois, String)>>> {
    type Saisie = Option<std::sync::Arc<(VueDuMois, String)>>;
    // Le contenu d'un catalogue de rubriques, sans le lire : la meme forme tant que ses octets sont les memes.
    type Catalogue = std::sync::Arc<serde::de::IgnoredAny>;
    type Vues = std::sync::Mutex<std::collections::HashMap<(PathBuf, String), (Lus, Vec<Catalogue>, Saisie)>>;
    static VUES: std::sync::OnceLock<Vues> = std::sync::OnceLock::new();
    static ABSENT: std::sync::OnceLock<Catalogue> = std::sync::OnceLock::new();
    let vues = VUES.get_or_init(Default::default);
    let (lus, tous) = tous(racine)?;
    let mut catalogues = Vec::new();
    for p in [socle::fichiers::dossier_du_code().join("rubriques-base.json"), racine.join(socle::fichiers::CONFIG).join(droit::catalogue::RUBRIQUES_COMPAGNIE)] {
        // Absent, il est le meme d'une lecture a l'autre ; illisible, il se refuse.
        catalogues.push(match socle::forme::partagee::<serde::de::IgnoredAny>(&p, ABIME)? {
            Some(v) => v,
            None => ABSENT.get_or_init(|| std::sync::Arc::new(serde::de::IgnoredAny)).clone(),
        });
    }
    let cle = (racine.to_path_buf(), mois.to_string());
    if let Some((vus, cs, s)) = verrouille(vues).get(&cle) {
        if memes(vus, &lus) && cs.len() == catalogues.len() && cs.iter().zip(&catalogues).all(|(a, b)| std::sync::Arc::ptr_eq(a, b)) {
            return Ok(s.clone());
        }
    }
    // Ce que le fichier du mois pose pour tous ; ses engagements, la vue les refait (ceux des mois d'avant qui courent).
    let fichier = lire_partage(racine, mois)?;
    let objets = tous.du_mois(mois)?;
    let saisie = if (fichier.is_none() || importes_seuls(racine, mois)?) && objets.is_empty() {
        None
    } else {
        let mut vue = VueDuMois { mois: Mois::lire(mois)?, taux_cachet: fichier.and_then(|f| f.taux_cachet.clone()), engagements: Vec::new() };
        for o in &objets {
            let Some(mut v) = o.vue(mois)? else { continue };
            if let Some(x) = &o.brut.valide {
                v.valide = Some(crate::saisie::Validee { le: Some(x.le.clone()) });
            }
            vue.engagements.push(v);
        }
        let empreinte = empreinte_de_saisie(&vue, None);
        Some(std::sync::Arc::new((vue, empreinte)))
    };
    socle::fichiers::ranger(vues, cle, (lus, catalogues, saisie.clone()));
    Ok(saisie)
}

/// La cle sous laquelle un calcul note la saisie d'un mois qu'il a lue (`socle::fichiers::suivre`) : `saisie <mois>`, ou
/// `saisie <mois> <salarie>` pour la saisie dont depend la paie de ce salarie dans son mois (`etats::calculer_les_rangs`).
pub fn cle_de_saisie(mois: &str, salarie: Option<&str>) -> String {
    match salarie {
        Some(s) => format!("saisie {} {}", mois, s),
        None => format!("saisie {}", mois),
    }
}

/// L'empreinte de la saisie d'un mois : ce qu'elle dit, sans ce que le moteur tient seul (la validation) ni le numero du
/// contrat (pose a l'envoi, il ne change aucun montant), sous sa forme canonique (`socle::ecriture::canonique`). Deux
/// parts, `<le mois>:<ses engagements>` : ce que le mois pose pour tous (le taux du cachet), et ses
/// engagements ; la raison d'un recalcul dit laquelle a change (`etats::raison`). Seuls les engagements valides
/// comptent : une paie ecrite ne lit que ceux-la (`etats::calculer_les_rangs`), et un brouillon tape a cote d'elle ne
/// la fait pas recalculer. Avec `salarie`, seuls ses engagements comptent : la paie d'un engagement ne lit, des autres
/// engagements du mois, que ceux du meme salarie (`etats::calculer_les_rangs`).
pub fn empreinte_de_saisie(vue: &VueDuMois, salarie: Option<&str>) -> String {
    #[derive(Serialize)]
    struct Tete<'a> {
        mois: &'a Mois,
        taux_cachet: &'a Option<NombreLu>,
    }
    let engagements: Vec<Carte> = vue
        .engagements
        .iter()
        .filter(|e| e.valide.is_some() && salarie.is_none_or(|s| e.salarie.as_deref() == Some(s)))
        .map(|e| Carte { valide: None, numero: None, ..e.clone() })
        .collect();
    let tete = Tete { mois: &vue.mois, taux_cachet: &vue.taux_cachet };
    let h = |x: String| socle::fichiers::sha256_hex(x.as_bytes());
    format!("{}:{}", h(socle::ecriture::canonique(&tete)), h(socle::ecriture::canonique(&engagements)))
}

/// Les mois que la saisie a touches : les fichiers, et les mois que chaque engagement a saisis ou couvre jusqu'a sa fin ;
/// un contrat sans terme, jusqu'au dernier mois ecrit ou jusqu'au mois d'aujourd'hui s'il est plus loin : un CDI de
/// janvier que rien n'a saisi depuis a une paie, une DSN et des documents chaque mois qu'il court.
pub fn mois_saisis(racine: &Path) -> Resultat<Vec<Mois>> {
    let mut v: Vec<Mois> = Vec::new();
    for m in fichiers(racine) {
        if !importes_seuls(racine, &m)? {
            v.push(Mois::lire(&m)?);
        }
    }
    let courant = Mois::du_jour(&droit::contexte::aujourdhui_sans_note()?);
    let horizon = v.iter().max().copied().map(|h| h.max(courant));
    for o in objets(racine)?.into_iter().filter(|o| !o.importe_seul()) {
        v.extend(o.mois_saisis()?);
        let premier = Mois::lire(&o.fichier)?;
        let fin = match o.dernier_mois()? {
            Some(d) => Some(Mois::lire(&d)?),
            None => horizon.filter(|h| *h >= premier),
        };
        if let Some(d) = fin {
            v.extend(premier.jusqu_a(d));
        }
    }
    v.sort();
    v.dedup();
    Ok(v)
}

// ------------------------------------------------------------------ l'ecriture

/// L'ECRITURE D'UNE CARTE : l'engagement en saisie que designe l'identite `id` dans l'ecran de `mois` (`designe`)
/// prend `saisie`, ce que sa carte ecrit pour `mois` ; sans identite, l'engagement est neuf et commence dans le fichier
/// du mois (le fichier se cree avec ce que le mois dit par defaut, `defauts`). C'est la seule ecriture qui cree un
/// engagement. Rend l'identite de l'engagement ecrit (`identites`).
///
/// Ce que le moteur tient seul ne se prend pas de la saisie ; le numero du contrat ecrit se garde (`poser_sur`). Refus, et rien n'est ecrit : un engagement valide (il se modifie d'un geste,
/// `etats::modifier`), une identite que l'ecran ne montre pas, un engagement du salarie et du premier jour d'un engagement
/// importe, des bornes qui laisseraient hors du contrat un mois ou il a des jours payes, un engagement neuf que l'ecran
/// de `mois` ne montrerait pas.
pub fn ecrire_la_carte(ctx: &Contexte, mois: &str, id: Option<&Identite>, saisie: &Carte, defauts: &VueDuMois) -> Resultat<Identite> {
    let racine = &ctx.racine;
    if let Some(cle) = &saisie.salarie {
        let debut = saisie.debut_lu();
        if crate::importe::celui_de(racine, mois, cle, &debut)?.is_some() {
            return Err(crate::importe::refus(cle, &debut));
        }
    }
    // Un engagement neuf, ou qui n'est encore qu'un brouillon de son premier mois, s'ecrit dans le fichier de son premier
    // mois (`ou_s_ecrit`) : le brouillon que ses bornes placent ailleurs y demenage.
    let fichier = ou_s_ecrit(racine, mois, id, saisie)?;
    let (fichier, rang) = match id.map(|id| designe(racine, mois, id)).transpose()? {
        Some((_, mut o)) if o.fichier == fichier => {
            if o.est_valide() {
                return Err(format!("{} : cet engagement est validé, il se modifie d'un geste. Rien n'est écrit.", socle::mois::Mois::en_clair(mois)));
            }
            if mois == o.fichier {
                refuser_un_second_contrat_au_mois(ctx, saisie, Some(&o))?;
            }
            poser_sur(&mut o, mois, saisie, None)?;
            reecrire(racine, &o)?;
            (o.fichier, o.rang)
        }
        soi => {
            refuser_un_second_contrat_au_mois(ctx, saisie, soi.as_ref().map(|(_, o)| o))?;
            if let Some((_, o)) = &soi {
                effacer(racine, o)?;
            }
            let neuf = Ecrit { saisie: Saisie { carte: en_forme(saisie)?, ..Saisie::default() }, ..Ecrit::default() };
            let mut contenu = match lire_fichier(racine, &fichier)? {
                Some(c) => c,
                None => FichierDuMois { mois: Mois::lire(&fichier)?, taux_cachet: defauts.taux_cachet.clone(), engagements: Vec::new() },
            };
            let rang = contenu.engagements.len();
            contenu.engagements.push(neuf);
            poser(racine, &contenu)?;
            (fichier, rang)
        }
    };
    let objets = objets_du_mois(racine, mois)?;
    let k = objets
        .iter()
        .position(|o| o.fichier == fichier && o.rang == rang)
        .ok_or_else(|| format!("{} : l'engagement écrit ne tombe pas dans ce mois (ses dates le placent ailleurs).", socle::mois::Mois::en_clair(mois)))?;
    Ok(identites(&objets).swap_remove(k))
}

/// LE MOIS DU FICHIER OU S'ECRIT LA CARTE `saisie` de l'ecran de `mois` : celui de son engagement (`id`) ; pour un
/// engagement neuf, ou qui n'est qu'un brouillon de son premier mois (jamais valide, sans numero donne, sans avenant ni
/// fait d'un autre mois), le premier mois que ses bornes disent (`mois_d_ecriture`).
pub fn ou_s_ecrit(racine: &Path, mois: &str, id: Option<&Identite>, saisie: &Carte) -> Resultat<String> {
    let Some(id) = id else { return mois_d_ecriture(racine, mois, saisie) };
    let (_, o) = designe(racine, mois, id)?;
    let e = &o.brut;
    let brouillon_seul = e.valide.is_none()
        && e.precedente.is_none()
        && e.versions.is_empty()
        && e.numero_pour.is_none()
        && e.non_actif.is_none()
        && e.importe.is_none()
        && e.saisie.avenants.is_empty()
        && e.saisie.faits.is_empty();
    if o.fichier == mois && brouillon_seul {
        mois_d_ecriture(racine, mois, saisie)
    } else {
        Ok(o.fichier.clone())
    }
}

/// UN CONTRAT AU MOIS N'A QU'UN ENGAGEMENT : la carte d'un contrat paye au mois (CDI, CDD), neuve ou en saisie dans son
/// premier mois (`soi`, l'engagement qu'elle ecrit), dont le salarie a deja un autre contrat au mois commence le meme jour
/// (actif, et qui n'est pas qu'importe : `importe::refus` dit le sien) ne le refait pas, elle refuse et n'ecrit rien ;
/// cet engagement se designe et se modifie depuis sa carte. Le rang de l'identite (`identites`) ne distingue que des
/// contrats d'usage du meme jour (deux spectacles) ; deux contrats au mois du meme salarie commencent deux jours
/// differents (ils se suivent).
fn refuser_un_second_contrat_au_mois(ctx: &Contexte, e: &Carte, soi: Option<&Objet>) -> Resultat<()> {
    if !e.mensuelle()? {
        return Ok(());
    }
    let (Some(cle), debut) = (&e.salarie, e.debut_lu()) else { return Ok(()) };
    if debut.is_empty() {
        return Ok(());
    }
    let tires = tous(&ctx.racine)?.1;
    let mut second = None;
    for o in &tires.objets {
        if !soi.is_some_and(|s| s.fichier == o.fichier && s.rang == o.rang)
            && !o.est_non_actif()
            && !o.importe_seul()
            && o.mensuel()?
            && crate::debut_brut(&o.brut) == debut
            && o.carte().salarie.as_ref() == Some(cle)
        {
            second = Some(o);
            break;
        }
    }
    let Some(o) = second else {
        return Ok(());
    };
    let nature = o.carte().nature_lue()?;
    let numero: String = o.carte().numero_dit().map(|n| format!(" (n° {})", n)).into_iter().collect();
    let debut_dit = match Jour::du_texte(&debut) {
        Some(j) => j.jj_mm_aaaa(),
        None => debut.clone(),
    };
    Err(format!(
        "{} : son {} commencé le {}{} a déjà son engagement ; une carte neuve ne le refait pas, il se modifie depuis sa \
         carte. Un nouveau contrat commence un autre jour. Rien n'est écrit.",
        ctx.nomme(cle),
        nature.libelle,
        debut_dit,
        numero
    ))
}

/// LA SAISIE D'UN ENGAGEMENT VALIDE ROUVERT, POSEE SUR SON MOIS : l'engagement de rang `rang` de `mois`
/// (`objets_du_mois`), remis en saisie (`etats::modifier`), prend `saisie` (`poser_sur`) ; un terme qui y change vaut a
/// partir du `jour` de ce mois quand il est dit, du 1er sinon. Refus : un engagement valide, un jour qui n'est pas un
/// jour du contrat dans ce mois, ou des bornes qui laisseraient hors du contrat un mois ou il a des jours payes ; rien
/// n'est ecrit.
pub fn poser_la_saisie(ctx: &Contexte, mois: &str, rang: usize, saisie: &Carte, jour: Option<u32>) -> Resultat<()> {
    let mut o = objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| format!("{} : pas d'engagement de rang {}", mois, rang + 1))?;
    if o.est_valide() {
        return Err(format!("{} : l'engagement de rang {} est valide, il ne prend pas de saisie", mois, rang + 1));
    }
    let le = match jour {
        None => None,
        Some(n) => {
            let premier = Mois::lire(mois)?.premier_jour();
            let qui = crate::salarie_nomme(ctx, o.carte().salarie.as_deref());
            // AU PREMIER MOIS DE L'ENGAGEMENT, ses termes valent de son debut : un jour n'a rien a dater.
            if mois == o.fichier {
                return Err(format!(
                    "{} : {} est le premier mois de cet engagement, ce qu'il dit vaut depuis le début du contrat ; un jour \
                     n'y date rien. Pour que le contrat commence un autre jour, changez son début (« Début du contrat »). Rien n'est écrit.",
                    qui,
                    socle::mois::Mois::en_clair(mois)
                ));
            }
            let j = Jour::nouveau(premier.an, premier.mois, n)
                .ok_or_else(|| format!("{} : « le {} {} » n'est pas un jour de ce mois. Rien n'est écrit.", qui, n, socle::mois::Mois::en_clair(mois)))?;
            let debut = Jour::lire(o.carte().debut.as_deref().ok_or_else(|| socle::fichiers::absent("debut"))?)?;
            let fin = o.carte().fin.as_deref().filter(|f| !f.is_empty()).map(Jour::lire).transpose()?;
            if j < debut || fin.is_some_and(|f| j > f) {
                return Err(format!(
                    "{} : le contrat court du {}{} ; un changement au {} n'en est pas un jour. Rien n'est écrit.",
                    qui,
                    debut.jj_mm_aaaa(),
                    fin.map(|f| format!(" au {}", f.jj_mm_aaaa())).into_iter().collect::<String>(),
                    j.jj_mm_aaaa()
                ));
            }
            // Le premier jour du contrat dans le mois : le changement couvre tout ce que le mois en paie.
            (j > debut).then_some(j)
        }
    };
    poser_sur(&mut o, mois, saisie, le)?;
    reecrire(&ctx.racine, &o)?;
    Ok(())
}

/// Ce que la carte ecrit pour `mois`, pose sur l'objet `o` : ce que le moteur tient seul ne se prend pas de la saisie,
/// le numero du contrat ecrit se garde.
fn poser_sur(o: &mut Objet, mois: &str, saisie: &Carte, le: Option<Jour>) -> Resultat<()> {
    let mut saisie = en_forme(saisie)?;
    if saisie.numero_dit().is_none() {
        if let Some(n) = o.carte().numero_dit() {
            saisie.numero = Some(n.to_string());
        }
    }
    let vue = o.vue(mois)?;
    // Le debut qu'un mois suivant d'un contrat d'usage deduit de son premier jour (`Objet::vue`) n'est pas une saisie.
    if mois != o.fichier && o.carte().debut.is_none() && saisie.debut.is_some() && vue.as_ref().is_some_and(|v| v.debut == saisie.debut) {
        saisie.debut = None;
    }
    poser_le_mois(o, mois, &saisie, le)
}

/// CE QU'UNE CARTE ECRIT, DANS LA FORME ECRITE : ni ce que le moteur tient seul ni ce qu'il deduit ; sa nature, toujours ;
/// `cadre`
/// seulement s'il s'ecarte du defaut du metier au catalogue (`emplois::metier`), et toujours pour un metier dont le
/// defaut change d'une convention a l'autre (`emplois::Metier::cadre_selon_la_convention`). Une carte ecrite deux fois a
/// l'identique s'ecrit donc de meme, et sa relecture ne voit aucun changement.
/// L'apercu lit chaque carte dans cette forme aussi.
pub fn en_forme(saisie: &Carte) -> Resultat<Carte> {
    // Ce que le moteur deduit, ou tient seul.
    let mut saisie = saisie.sans(&[Groupe::Deduit, Groupe::Vue]);
    let catalogue = match saisie.metier.as_deref().filter(|m| !m.is_empty()) {
        Some(m) => droit::emplois::metier(m)?
            .filter(|f| !f.cadre_selon_la_convention())
            .map(|f| f.cadre_par_defaut("")),
        None => None,
    };
    if catalogue.is_some() && saisie.cadre == catalogue {
        saisie.cadre = None;
    }
    let dite = |x: &Option<String>| x.as_deref().is_some_and(|x| !x.is_empty());
    if !saisie.nature_lue()?.mensuelle() && saisie.lignes.is_none() && dite(&saisie.debut) && dite(&saisie.fin) {
        // UN MOIS DU CONTRAT SANS CACHET : un contrat aux bornes dites, sans ligne, s'ecrit `lignes: []` -- la DSN porte
        // le contrat avec un versement a zero, sans bulletin.
        saisie.lignes = Some(Vec::new());
    }
    Ok(saisie)
}

/// Pose la saisie que l'ecran montre pour `mois` sur l'objet `o` : son premier mois s'il commence en `mois` ; sinon,
/// LE MOIS DE LA CARTE EST LE « A PARTIR DE » : chaque terme qu'elle dit autrement que l'engagement a la fin du mois
/// change a partir du `le` dit, du 1er du mois sinon (`dater`), et vaut jusqu'au changement suivant ; ce qu'elle dit
/// d'autre (ses lignes, ses conges, ses primes, sa rupture) est le fait du mois, qui ne s'ecrit que s'il dit autre chose
/// que ce que le mois tient seul (les jours que le planning y met). Ce que tous les mois partagent (`Groupe::Partage`)
/// va a l'engagement. Les bornes ne laissent pas hors du contrat un mois ou il a des jours payes.
fn poser_le_mois(o: &mut Objet, mois: &str, saisie: &Carte, le: Option<Jour>) -> Resultat<()> {
    let fin_du_mois = Mois::lire(mois)?.dernier_jour();
    let actuels = o.termes_a(&fin_du_mois);
    {
        let carte = &mut o.brut_mut().saisie.carte;
        for k in Champ::des(&[Groupe::Partage]) {
            if saisie.dit(k) || k != Champ::Numero {
                carte.prendre(saisie, k);
            }
        }
    }
    // La fin que la carte dit d'un autre mois que le sien va au mois ou le contrat finit (`Groupe::Fin`).
    let dernier = o.dernier_mois()?.filter(|d| d != mois);
    let (reste, de_la_fin) = match &dernier {
        Some(_) => (saisie.sans(&[Groupe::Fin]), saisie.du_groupe(&[Groupe::Fin])),
        None => (saisie.clone(), Carte::default()),
    };
    if mois == o.fichier {
        let mut neuf = reste.clone();
        for k in Champ::des(&[Groupe::Partage]) {
            neuf.prendre(o.carte(), k);
        }
        o.brut_mut().saisie.carte = neuf;
    } else {
        let depuis = match le {
            Some(j) => j,
            None => Mois::lire(mois)?.premier_jour(),
        };
        for k in Champ::des(&[Groupe::Terme]) {
            if actuels.comparable_du(k) != reste.comparable_du(k) {
                dater(o, k, mois, depuis, &reste)?;
            }
        }
        let faits = reste.du_groupe(&[Groupe::Fait, Groupe::Fin]);
        let seul = if o.mensuel()? { Carte::default() } else { Carte { lignes: Some(o.lignes_du_planning(mois)?), ..Carte::default() } };
        let m = Mois::lire(mois)?;
        if socle::ecriture::comparable(&faits) == socle::ecriture::comparable(&seul) {
            o.brut_mut().saisie.faits.remove(&m);
        } else {
            o.brut_mut().saisie.faits.insert(m, faits);
        }
    }
    if let Some(d) = dernier.filter(|_| Champ::des(&[Groupe::Fin]).any(|c| de_la_fin.dit(c))) {
        poser_la_fin(o, &d, &de_la_fin)?;
    }
    // Les bornes ne laissent pas hors du contrat un mois ou il a des jours payes.
    let mois_des_faits: Vec<Mois> = o.brut.saisie.faits.keys().copied().collect();
    for m in mois_des_faits {
        if o.dans_ses_bornes(&m.to_string())? {
            continue;
        }
        if o.brut.saisie.faits.get(&m).is_some_and(|f| !f.lignes().is_empty()) {
            return Err(format!(
                "Le contrat irait du {} au {}, et {} y porte des jours payes : un mois de paie ne tient pas un contrat \
                 qui ne le couvre pas. Rien n'est ecrit. Retirer d'abord les lignes de {}, ou garder des bornes qui le \
                 couvrent.",
                Carte::texte(&o.carte().debut),
                match &o.carte().fin {
                    Some(f) => f.clone(),
                    None => "?".to_string(),
                },
                socle::mois::Mois::en_clair(&m.to_string()),
                socle::mois::Mois::en_clair(&m.to_string())
            ));
        }
        o.brut_mut().saisie.faits.remove(&m);
    }
    Ok(())
}

/// LA FIN DITE SUR LA CARTE D'UN AUTRE MOIS s'ecrit au mois `dernier`, celui ou le contrat finit (`Groupe::Fin`) : au
/// premier mois s'il finit dans celui-ci, sinon parmi ses faits ; un contrat d'usage dont ce mois n'avait pas de faits
/// y garde les jours que son planning y met.
fn poser_la_fin(o: &mut Objet, dernier: &str, de_la_fin: &Carte) -> Resultat<()> {
    if dernier == o.fichier {
        o.brut_mut().saisie.carte.recouvrir(de_la_fin);
        return Ok(());
    }
    let mensuel = o.mensuel()?;
    let mut faits = match o.faits_de(dernier)? {
        Some(f) => f.clone(),
        None if mensuel => Carte::default(),
        None => Carte { lignes: Some(o.lignes_du_planning(dernier)?), ..Carte::default() },
    };
    faits.recouvrir(de_la_fin);
    o.brut_mut().saisie.faits.insert(Mois::lire(dernier)?, faits);
    Ok(())
}

/// UN TERME CHANGE A PARTIR DU `depuis` de `mois` : ce que les avenants du mois disaient de lui s'efface (la carte du
/// mois le redit), et sa nouvelle valeur, celle que dit `saisie` (rien : il se retire), s'ecrit a ce jour, sauf si c'est
/// deja la sienne la veille. Ce qui precede et les avenants des mois d'apres restent ce qu'ils sont.
fn dater(o: &mut Objet, terme: Champ, mois: &str, depuis: Jour, saisie: &Carte) -> Resultat<()> {
    {
        let s = &mut o.brut_mut().saisie;
        for a in s.avenants.iter_mut().filter(|a| a.depuis.mois_civil().to_string() == mois) {
            a.termes.ote(terme);
            a.retires.retain(|c| *c != terme);
        }
        s.avenants.retain(|a| !a.vide());
    }
    let veille = o.termes_a(&depuis.plus_jours(-1)?);
    if veille.comparable_du(terme) == saisie.comparable_du(terme) {
        return Ok(());
    }
    let s = &mut o.brut_mut().saisie;
    let i = match s.avenants.iter().position(|a| a.depuis == depuis) {
        Some(i) => i,
        None => {
            s.avenants.push(Avenant { depuis, termes: Carte::default(), retires: Vec::new() });
            s.avenants.sort_by_key(|a| a.depuis);
            s.avenants.iter().position(|a| a.depuis == depuis).expect("l'avenant vient d'etre pose")
        }
    };
    let a = &mut s.avenants[i];
    if saisie.dit(terme) {
        a.termes.prendre(saisie, terme);
    } else {
        a.retires.push(terme);
        a.retires.sort();
    }
    Ok(())
}

impl Objet {
    /// L'engagement tel qu'une saisie gardee (`saisie`, celle d'une validation) le disait : meme fichier, meme rang.
    pub fn d_apres_la_saisie(&self, saisie: &Saisie) -> Objet {
        Objet {
            racine: self.racine.clone(),
            fichier: self.fichier.clone(),
            rang: self.rang,
            brut: std::sync::Arc::new(Ecrit { saisie: saisie.clone(), ..Ecrit::default() }),
        }
    }
}

/// L'engagement que designe, dans le mois `mois`, le rang `rang` de son ecran.
pub fn objet_du_mois(racine: &Path, mois: &str, rang: usize) -> Resultat<Option<Objet>> {
    Ok(objets_du_mois(racine, mois)?.into_iter().nth(rang))
}

/// Remplace l'objet `o` dans son fichier (meme fichier, meme rang), tel qu'il est maintenant.
pub fn reecrire(racine: &Path, o: &Objet) -> Resultat<PathBuf> {
    let mut contenu = fichier_de(racine, o)?;
    let place = contenu.engagements.get_mut(o.rang).ok_or_else(|| format!("{} : l'engagement {} a disparu", o.fichier, o.rang + 1))?;
    *place = (*o.brut).clone();
    poser(racine, &contenu)
}

/// LE MOIS D'ENTREE DE LA COMPAGNIE DANS LE LOGICIEL : le premier mois qu'elle y paie, celui du plus ancien fichier qui
/// porte un engagement d'ici (qui n'est pas qu'importe, ni plus actif), ou `mois` s'il vient avant (la carte d'un premier mois, pas
/// encore ecrite). Les mois repris d'un ancien logiciel (`importe`) sont d'avant lui.
pub fn mois_d_entree(racine: &Path, mois: &str) -> Resultat<String> {
    let premier = tous(racine)?.1.objets.iter().find(|o| !o.importe_seul() && !o.est_non_actif()).map(|o| o.fichier.clone());
    Ok(premier.into_iter().fold(mois.to_string(), String::min))
}

/// LE MOIS OU S'ECRIT L'ENGAGEMENT NEUF `e` que la carte de `mois` saisit : un engagement appartient a son premier mois.
/// Un contrat paye au mois dont le debut tombe dans un mois d'avant la carte, mais pas avant le logiciel
/// (`commence_avant_le_logiciel`), s'ecrit dans le mois de son debut, comme saisi sur sa carte : ce mois montre ce qui est
/// a faire (sa paie, sa DPAE). Sinon, `mois`.
pub fn mois_d_ecriture(racine: &Path, mois: &str, e: &Carte) -> Resultat<String> {
    let mensuel = e.mensuelle()?;
    let debut = e.debut_lu();
    let premier = debut.get(..7).filter(|m| socle::mois::Mois::lire(m).is_ok()).map(String::from);
    Ok(match premier {
        Some(p) if mensuel && p.as_str() < mois && !commence_avant_le_logiciel(racine, &debut, mois)? => p,
        _ => mois.to_string(),
    })
}

/// UN CONTRAT COMMENCE AVANT LE LOGICIEL : son debut (`AAAA-MM-JJ`) precede le mois d'entree de la compagnie
/// (`mois_d_entree`, vu du mois `mois`). Il a commence ailleurs : sa DPAE et son contrat se sont faits avant (rien a
/// declarer ni a signer ici), son numero se dit s'il a deja ete declare en DSN, et ses conges acquis avant se reprennent
/// de l'ancien bulletin (la fiche, `conges_reprise`).
/// Sa date de debut se declare telle quelle : S21.G00.40.001 est « la date du premier jour d'applicabilité du contrat »,
/// identifiante, sans autre borne que le dernier jour du mois declare (cahier technique DSN 2026.1, § 4.4.12,
/// `atelier/reference/norme-dsn/texte/chapitre-4-controles.txt` ; bloc S21.G00.40, `texte-2027/bloc-S21.G00.40.txt`) ;
/// son numero, identifiant aussi, se reporte a l'identique d'une DSN a la suivante (§ 2.1.2, `chapitre-2-principes-de-constitution.txt`).
pub fn commence_avant_le_logiciel(racine: &Path, debut: &str, mois: &str) -> Resultat<bool> {
    Ok(debut.get(..7).is_some_and(|m| socle::mois::Mois::lire(m).is_ok()) && debut[..7] < *mois_d_entree(racine, mois)?)
}

/// Le jour `AAAA-MM-JJ` de `mois` qui est le premier, pour les bornes d'un contrat.
pub fn premier_jour(mois: &str) -> Resultat<Jour> {
    Mois::lire(mois).map(Mois::premier_jour)
}

/// Le mois a-t-il une saisie : un fichier, ou un engagement qui le touche ?
pub fn a_une_saisie(racine: &Path, mois: &str) -> Resultat<bool> {
    if chemin(racine, mois).is_file() && !importes_seuls(racine, mois)? {
        return Ok(true);
    }
    for o in objets(racine)? {
        if o.fichier.as_str() <= mois && o.couvre(mois)? {
            return Ok(true);
        }
    }
    Ok(false)
}
