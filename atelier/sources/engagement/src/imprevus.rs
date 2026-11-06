//! LES IMPREVUS D'UN SALARIE : ce qui arrive a un contrat en cours et le suspend -- un ARRET de travail
//! (bloc S21.G00.60, son signalement 04/05) ou une SUSPENSION d'une autre sorte (bloc S21.G00.65 : conge parental,
//! sabbatique, sans solde...). Un imprevu se saisit UNE FOIS, sur le salarie et le contrat qu'il suspend, avec ses dates ;
//! le moteur en tire tout le reste, mois par mois :
//!
//! - la ligne de paie d'un permanent (`lignes_du_mois`) : le salaire des jours couverts retire, le maintien de salaire
//!   saisi sur l'arret verse ; la saisie du mois n'a plus d'absence a taper ;
//! - le bloc 65 de chaque DSN dont le mois croise la suspension (`suspensions_du_mois`), comme le bloc 60 l'est deja
//!   pour l'arret (`arrets_du_mois`) : « en cas de suspension supra-mensuelle, la suspension doit etre declaree dans
//!   toutes les DSN mensuelles dont le Mois Principal declare presente une periode commune avec la suspension »
//!   (cahier technique DSN 2026.1, bloc S21.G00.60 p. 265 et bloc S21.G00.65 p. 281,
//!   `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` et `bloc-S21.G00.65.txt`) ;
//! - les cachets et services d'un intermittent qui tombent dans l'imprevu (`cachets_dans`, `avertir_les_jours_payes`) :
//!   le moteur les montre et avertit, il ne les retire pas (regle 1).
//!
//! UN SEUL MAGASIN : `imprevus/AAAA-MM.json` sous les donnees de la compagnie, au mois ou l'imprevu se range (le dernier
//! jour travaille d'un arret, le premier jour d'une suspension) : `{"_origine", "mois", "imprevus": [...]}`. Chaque
//! imprevu dit son `genre` (`arret` ou `suspension`) ; sa forme est `ArretSaisi` ou `SuspensionSaisie`, et tout autre
//! champ refuse.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::conges::{avertir, flot, info};
use crate::Chiffre;
use droit::permanents::MotifDeSuspension;
use droit::contexte::Contexte;
use droit::fiche::SalarieDuContrat;
use socle::date::Jour;
use crate::Avis;
use socle::fichiers::repr;
use socle::num::{div, eur2, mul, Decimal, Resultat};

/// Le genre d'un arret de travail (bloc S21.G00.60).
pub const ARRET: &str = "arret";
/// Le genre d'une autre suspension du contrat (bloc S21.G00.65).
pub const SUSPENSION: &str = "suspension";
/// Le dossier du magasin, sous les donnees de la compagnie.
pub const DOSSIER: &str = "imprevus";

/// La phrase du refus d'un imprevu qui n'a pas sa forme.
pub const ILLISIBLE: &str = "Un imprévu ne se lit pas : le signaler au développeur.";

/// La phrase du refus d'un fichier du magasin qui n'a pas sa forme.
const ABIME: &str = "Un mois du magasin des imprévus ne se relit pas : il est abîmé.";

/// Le genre d'un arret, tel qu'il s'ecrit (`ARRET`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenreArret {
    #[serde(rename = "arret")]
    Arret,
}

/// Le genre d'une suspension, tel qu'il s'ecrit (`SUSPENSION`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenreSuspension {
    #[serde(rename = "suspension")]
    Suspension,
}

/// UN ARRET DE TRAVAIL, dans l'ordre ou il s'ecrit ; chaque cle est ecrite, nulle quand elle n'a pas de valeur, et sa
/// lecture refuse celle qui manque. Ses champs de la norme (motif, dernier
/// jour travaille, fin previsionnelle, subrogation, reprise, accident) sont ceux du bloc 60 et du signalement
/// (`signalement::verifier`) ; `debut` est le premier jour que l'avis d'arret prescrit, d'ou se comptent l'incapacite (CSS
/// R323-1, 1°) et ce que la DSN declare (`dernier_jour_declare`) ; `jours_deja_indemnises` et les indemnites journalieres
/// par jour, brutes et nettes de CSG et CRDS (`ij_brute`, `ij_nette`, et `_majoree` au-dela du vingt-huitieme jour d'un
/// accident du travail) sont ceux du maintien de salaire d'un permanent (`maintien`) ; `prolongations` garde chaque avis de
/// prolongation saisi (`moteur::routes::imprevus::arret_prolonger`), dont le moteur tire le signalement rectificatif
/// qu'une subrogation prolongee appelle (`obligations::de_l_arret`) ; `reprises` garde chaque reprise notee ou effacee
/// (`arret_reprise`). Par ces deux traces, l'arret se relit tel qu'il etait a un instant passe
/// (`moteur::routes::imprevus::arret_a_l_instant`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArretSaisi {
    pub genre: GenreArret,
    pub salarie: String,
    pub contrat_debut: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub contrat_numero: Option<String>,
    pub motif: String,
    pub dernier_jour_travaille: String,
    pub debut: String,
    pub fin_previsionnelle: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub subrogation: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub subrogation_debut: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub subrogation_fin: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise_motif: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub date_accident: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub jours_deja_indemnises: Option<Chiffre>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub ij_brute: Option<Chiffre>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub ij_nette: Option<Chiffre>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub ij_brute_majoree: Option<Chiffre>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub ij_nette_majoree: Option<Chiffre>,
    pub prolongations: Vec<Prolongation>,
    pub reprises: Vec<RepriseNotee>,
    pub saisi_le: String,
    pub qui: String,
    pub note: String,
}

/// UN AVIS DE PROLONGATION SAISI : quand (le jour du moteur, l'heure de la saisie, qui), la fin et la fin de
/// subrogation d'avant et d'apres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prolongation {
    pub le: String,
    pub saisi_le: String,
    pub qui: String,
    pub fin_avant: String,
    pub fin: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub subrogation_fin_avant: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub subrogation_fin: Option<String>,
}

/// UNE REPRISE NOTEE OU EFFACEE : quand, et la reprise et son motif d'avant et d'apres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepriseNotee {
    pub le: String,
    pub saisi_le: String,
    pub qui: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise_avant: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise_motif_avant: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise: Option<String>,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub reprise_motif: Option<String>,
}

/// UNE SUSPENSION DU CONTRAT saisie (bloc 65 : motif, debut, fin), dans l'ordre ou elle s'ecrit ; chaque cle est ecrite
/// (le numero du contrat, nul quand il n'en a pas encore).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuspensionSaisie {
    pub genre: GenreSuspension,
    pub salarie: String,
    pub contrat_debut: String,
    #[serde(deserialize_with = "socle::forme::presente")]
    pub contrat_numero: Option<String>,
    pub motif: String,
    pub debut: String,
    pub fin: String,
    pub saisi_le: String,
    pub qui: String,
    pub note: String,
}

/// UN IMPREVU DU MAGASIN : un arret de travail ou une suspension, que son genre distingue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Imprevu {
    Arret(ArretSaisi),
    Suspension(SuspensionSaisie),
}

/// LE FICHIER D'UN MOIS DU MAGASIN : ce qu'il range, son mois, ses imprevus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FichierDesImprevus {
    #[serde(rename = "_origine")]
    origine: String,
    mois: String,
    imprevus: Vec<Imprevu>,
}

/// CE QUI DESIGNE LE CONTRAT QU'UN IMPREVU SUSPEND : le salarie (la cle de sa fiche), la date de debut du contrat
/// (S21.G00.40.001) et son numero (40.009), tels que l'imprevu les ecrit.
pub trait Suspend {
    fn salarie(&self) -> &str;
    fn contrat_debut(&self) -> &str;
    fn contrat_numero(&self) -> Option<&str>;
}

impl Suspend for ArretSaisi {
    fn salarie(&self) -> &str {
        &self.salarie
    }
    fn contrat_debut(&self) -> &str {
        &self.contrat_debut
    }
    fn contrat_numero(&self) -> Option<&str> {
        self.contrat_numero.as_deref()
    }
}

impl Suspend for SuspensionSaisie {
    fn salarie(&self) -> &str {
        &self.salarie
    }
    fn contrat_debut(&self) -> &str {
        &self.contrat_debut
    }
    fn contrat_numero(&self) -> Option<&str> {
        self.contrat_numero.as_deref()
    }
}

impl Suspend for Imprevu {
    fn salarie(&self) -> &str {
        match self {
            Imprevu::Arret(a) => a.salarie(),
            Imprevu::Suspension(s) => s.salarie(),
        }
    }
    fn contrat_debut(&self) -> &str {
        match self {
            Imprevu::Arret(a) => a.contrat_debut(),
            Imprevu::Suspension(s) => s.contrat_debut(),
        }
    }
    fn contrat_numero(&self) -> Option<&str> {
        match self {
            Imprevu::Arret(a) => a.contrat_numero(),
            Imprevu::Suspension(s) => s.contrat_numero(),
        }
    }
}

/// Un jour ecrit, lu.
fn jour(x: &str) -> Resultat<Jour> {
    Jour::lire(x)
}

/// Un jour dit et non vide, lu.
fn jour_dit(x: &Option<String>) -> Resultat<Option<Jour>> {
    x.as_deref().filter(|j| !j.is_empty()).map(Jour::lire).transpose()
}

impl Imprevu {
    /// Son genre, tel qu'il s'ecrit (`ARRET`, `SUSPENSION`).
    pub fn genre(&self) -> &'static str {
        match self {
            Imprevu::Arret(_) => ARRET,
            Imprevu::Suspension(_) => SUSPENSION,
        }
    }

    /// L'arret qu'il est, s'il en est un.
    pub fn arret(&self) -> Option<&ArretSaisi> {
        match self {
            Imprevu::Arret(a) => Some(a),
            Imprevu::Suspension(_) => None,
        }
    }

    /// Son motif, tel qu'il s'ecrit ; vide s'il n'en dit pas.
    pub fn motif(&self) -> &str {
        match self {
            Imprevu::Arret(a) => &a.motif,
            Imprevu::Suspension(s) => &s.motif,
        }
    }

    /// Son debut, tel qu'il s'ecrit (le debut prescrit d'un arret, le premier jour d'une suspension).
    pub fn debut(&self) -> &str {
        match self {
            Imprevu::Arret(a) => &a.debut,
            Imprevu::Suspension(s) => &s.debut,
        }
    }
}

pub fn est_un_arret(imp: &Imprevu) -> bool {
    matches!(imp, Imprevu::Arret(_))
}

pub fn est_une_suspension(imp: &Imprevu) -> bool {
    matches!(imp, Imprevu::Suspension(_))
}

/// CE QU'UN IMPREVU DOIT TENIR AU-DELA DE SA FORME : les indemnites journalieres d'un arret, par jour, sont positives.
pub fn verifier(imp: &Imprevu) -> Resultat<()> {
    let Imprevu::Arret(a) = imp else { return Ok(()) };
    for (k, x) in [("ij_brute", &a.ij_brute), ("ij_nette", &a.ij_nette), ("ij_brute_majoree", &a.ij_brute_majoree), ("ij_nette_majoree", &a.ij_nette_majoree)] {
        if let Some(x) = x.as_ref().filter(|x| x.vrai()) {
            if x.dec_avec_echelle().map_or(true, |m| m <= Decimal::ZERO) {
                return Err(socle::echec::a_la_cle("Un montant par jour de l'arrêt doit être positif.", k, x.en_texte()));
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ le magasin

/// `imprevus/` sous les donnees de la compagnie.
pub fn dossier(racine: &Path) -> PathBuf {
    racine.join(DOSSIER)
}

fn chemin(racine: &Path, mois: &str) -> PathBuf {
    dossier(racine).join(format!("{}.json", mois))
}

/// Les imprevus d'un fichier deja lu, verifies (`verifier`).
fn du_contenu(contenu: &FichierDesImprevus, chemin: &Path) -> Resultat<Vec<Imprevu>> {
    for imp in &contenu.imprevus {
        verifier(imp).map_err(|e| socle::echec::du_disque(chemin, &e))?;
    }
    Ok(contenu.imprevus.clone())
}

/// Le fichier `f` du magasin, lu en sa forme ; la lecture se note au registre du calcul (`socle::forme::lire_fichier`).
fn lu(f: &Path) -> Resultat<Vec<Imprevu>> {
    du_contenu(&socle::forme::lire_fichier(f, ABIME)?, f)
}

/// Tous les imprevus de la compagnie, dans l'ordre des mois du magasin puis de leur saisie, verifies. La lecture se note
/// au registre du calcul (`fichiers::lister`, `lu`) : un imprevu saisi refait la paie.
pub fn tous(racine: &Path) -> Resultat<Vec<Imprevu>> {
    let Some(fichiers) = socle::fichiers::lister(&dossier(racine)) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for f in fichiers {
        let nom = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !nom.strip_suffix(".json").is_some_and(|m| socle::mois::Mois::lire(m).is_ok()) {
            continue;
        }
        out.extend(lu(&f)?);
    }
    Ok(out)
}

/// LES IMPREVUS D'UN SALARIE (`cle`, la cle de sa fiche), dans l'ordre du magasin : ce que le calcul de ses paies lit,
/// et rien d'autre. La lecture se note au registre du calcul en une entree, `imprevus racine:imprevus <cle>`, qui vaut
/// l'empreinte de ces seuls imprevus (`empreinte_du_salarie`) : un imprevu saisi pour un autre salarie ne refait pas
/// ses paies.
pub fn du_salarie(racine: &Path, cle: &str) -> Resultat<Vec<Imprevu>> {
    let siens = siens_sans_noter(racine, cle)?;
    socle::fichiers::noter_valeur(&entree_du_salarie(cle), empreinte_de(&siens));
    Ok(siens)
}

/// L'entree du registre que `du_salarie` note.
pub fn entree_du_salarie(cle: &str) -> String {
    format!("{} racine:{} {}", DOSSIER, DOSSIER, cle)
}

/// Le salarie d'une entree `du_salarie`, s'il en est une.
pub fn salarie_de_l_entree(k: &str) -> Option<&str> {
    k.strip_prefix(DOSSIER)?.strip_prefix(" racine:")?.strip_prefix(DOSSIER)?.strip_prefix(' ')
}

/// Ce que vaut aujourd'hui l'entree `du_salarie` de `cle` : l'empreinte de ses imprevus, relus sans rien noter.
pub fn empreinte_du_salarie(racine: &Path, cle: &str) -> Resultat<String> {
    Ok(empreinte_de(&siens_sans_noter(racine, cle)?))
}

fn empreinte_de(imprevus: &[Imprevu]) -> String {
    socle::fichiers::sha256_hex(socle::ecriture::json_en_ligne(imprevus).as_bytes())
}

/// Les imprevus de `cle`, lus mois par mois du magasin sans se noter (`socle::forme::partagee`).
fn siens_sans_noter(racine: &Path, cle: &str) -> Resultat<Vec<Imprevu>> {
    let mut fichiers: Vec<PathBuf> = match std::fs::read_dir(dossier(racine)) {
        Ok(l) => l.flatten().map(|f| f.path()).filter(|p| p.file_stem().and_then(|n| n.to_str()).is_some_and(|m| socle::mois::Mois::lire(m).is_ok()) && p.extension().is_some_and(|x| x == "json")).collect(),
        Err(_) => return Ok(Vec::new()),
    };
    fichiers.sort();
    let mut out = Vec::new();
    for f in fichiers {
        let Some(contenu) = socle::forme::partagee::<FichierDesImprevus>(&f, ABIME)? else { continue };
        out.extend(du_contenu(&contenu, &f)?.into_iter().filter(|imp| imp.salarie() == cle));
    }
    Ok(out)
}

/// Les imprevus d'un genre (ou de tous les genres) d'un salarie (`du_salarie`) qui croisent le mois `AAAA-MM`.
pub fn du_mois_du_salarie(racine: &Path, mois: &str, genre_voulu: Option<&str>, cle: &str) -> Resultat<Vec<Imprevu>> {
    croisant(du_salarie(racine, cle)?, mois, genre_voulu)
}

/// Les imprevus que le fichier d'un mois du magasin range, verifies ; vide s'il n'existe pas.
pub fn du_fichier(racine: &Path, mois: &str) -> Resultat<Vec<Imprevu>> {
    let p = chemin(racine, mois);
    if !socle::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    lu(&p)
}

/// Ecrit le fichier d'un mois du magasin.
pub fn ecrire(racine: &Path, mois: &str, imprevus: Vec<Imprevu>) -> Resultat<()> {
    let origine = format!(
        "Les imprevus ranges en {} : arrets de travail (au mois du dernier jour travaille) et suspensions (au mois \
de leur premier jour). Chacun se saisit une fois ; le moteur en tire la ligne de paie d'un permanent, le bloc 60 ou 65 \
de chaque DSN mensuelle dont le mois le croise, et, pour un arret, son signalement (nature 04, 05).",
        mois
    );
    socle::ecriture::ecrire_json(&chemin(racine, mois), &FichierDesImprevus { origine, mois: mois.to_string(), imprevus }, 2)
}

/// Le jour qui range un imprevu dans le magasin : le dernier jour travaille d'un arret, le premier jour d'une suspension.
pub fn jour_de_rangement(imp: &Imprevu) -> Resultat<Jour> {
    match imp {
        Imprevu::Arret(a) => jour(&a.dernier_jour_travaille),
        Imprevu::Suspension(s) => jour(&s.debut),
    }
}

// ------------------------------------------------------------------ les jours

/// LE DERNIER JOUR TRAVAILLE QUE LA DSN DECLARE D'UN ARRET (S21.G00.60.002) : « la veille de la date de debut de la
/// prescription de l'arret de travail, sauf [...] arret prescrit lors d'un jour travaille : le dernier jour travaille
/// correspond alors a la date de l'arret » (cahier technique DSN 2026.1, p. 266-267,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`). Un arret prescrit un jour travaille a son debut egal au
/// dernier jour travaille saisi ; sinon la DSN declare la veille de son debut, meme un dimanche : les organismes en lisent
/// le debut de l'arret, « Date du dernier jour travaille S21.G00.60.002 + 1 [...] Aucune autre donnee ne permet d'avoir la
/// date de debut de l'arret reelle en DSN » (fiche consigne 568, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.txt`, l. 1683).
pub fn dernier_jour_declare(arret: &ArretSaisi) -> Resultat<Jour> {
    let (travaille, debut) = (jour(&arret.dernier_jour_travaille)?, jour(&arret.debut)?);
    Ok(if debut <= travaille { travaille } else { debut.plus_jours(-1)? })
}

/// Le premier jour d'absence d'un arret (`premier_jour`).
pub(crate) fn premier_jour_d_arret(arret: &ArretSaisi) -> Resultat<Jour> {
    dernier_jour_declare(arret)?.plus_jours(1)
}

/// Le dernier jour d'absence d'un arret : la veille de sa reprise, sinon sa fin previsionnelle (`dernier_jour`).
pub(crate) fn dernier_jour_d_arret(arret: &ArretSaisi) -> Resultat<Jour> {
    match jour_dit(&arret.reprise)? {
        Some(r) => r.plus_jours(-1),
        None => jour(&arret.fin_previsionnelle),
    }
}

/// Le premier jour d'absence : le lendemain du dernier jour que la DSN declare d'un arret (`dernier_jour_declare`) -- le
/// debut que l'avis prescrit, ou son lendemain quand il est prescrit un jour travaille --, d'ou se compte « le quatrieme
/// jour de l'incapacite de travail » (CSS R323-1, 1°) ; le debut d'une suspension (S21.G00.65.002).
pub fn premier_jour(imp: &Imprevu) -> Resultat<Jour> {
    match imp {
        Imprevu::Arret(a) => premier_jour_d_arret(a),
        Imprevu::Suspension(s) => jour(&s.debut),
    }
}

/// Le dernier jour d'absence : la veille de la reprise d'un arret, « premier jour travaille et paye suite a l'arret »
/// (S21.G00.60.010, p. 269), sinon sa fin previsionnelle (60.003) ; la fin d'une suspension (65.003).
pub fn dernier_jour(imp: &Imprevu) -> Resultat<Jour> {
    match imp {
        Imprevu::Arret(a) => dernier_jour_d_arret(a),
        Imprevu::Suspension(s) => jour(&s.fin),
    }
}

/// Une periode commune avec [debut, fin] : ce qui fait reporter l'imprevu dans la DSN de ce mois. « En cas de suspension
/// supra-mensuelle, la suspension doit etre declaree dans toutes les DSN mensuelles dont le Mois Principal declare
/// presente une periode commune avec la suspension » (bloc S21.G00.60, cahier technique DSN 2026.1, p. 265). Un arret
/// court du dernier jour travaille qu'il declare (S21.G00.60.002, `dernier_jour_declare`) a son dernier jour d'absence (`dernier_jour`) : la veille de la
/// reprise, qui est le « premier jour travaille et paye suite a l'arret de travail » (S21.G00.60.010, p. 269), sinon la
/// fin previsionnelle (S21.G00.60.003, p. 267). Un arret repris le 1er du mois n'a donc aucun jour commun avec ce mois.
pub fn croise(imp: &Imprevu, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    if let Imprevu::Arret(a) = imp {
        return arret_croise(a, debut, fin);
    }
    Ok(premier_jour(imp)? <= *fin && dernier_jour(imp)? >= *debut)
}

/// La meme regle pour un arret de travail (un arret de la DSN, `dsn::Arret`).
pub fn arret_croise(arret: &ArretSaisi, debut: &Jour, fin: &Jour) -> Resultat<bool> {
    let dernier = dernier_jour_d_arret(arret)?;
    Ok(dernier_jour_declare(arret)? <= *fin && dernier >= *debut)
}

/// Les imprevus d'un genre (ou de tous les genres) qui croisent le mois `AAAA-MM`.
pub fn du_mois(racine: &Path, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Imprevu>> {
    croisant(tous(racine)?, mois, genre_voulu)
}

fn croisant(imprevus: Vec<Imprevu>, mois: &str, genre_voulu: Option<&str>) -> Resultat<Vec<Imprevu>> {
    let (debut, fin) = (socle::mois::Mois::lire(mois)?.premier_jour(), socle::mois::Mois::lire(mois)?.dernier_jour());
    let mut out = Vec::new();
    for imp in imprevus {
        if genre_voulu.is_some_and(|gv| gv != imp.genre()) {
            continue;
        }
        if croise(&imp, &debut, &fin)? {
            out.push(imp);
        }
    }
    Ok(out)
}

/// L'imprevu suspend-il CE contrat ? Le meme salarie (la cle de sa fiche), la meme date de debut du contrat
/// (S21.G00.40.001), et le meme numero (40.009) quand l'un et l'autre en ont un : un contrat saisi avant sa premiere
/// production n'a pas encore de numero (il se fige a la production), et c'est sa date de debut qui le designe.
pub fn du_contrat<I: Suspend>(imp: &I, cle: &str, numero: Option<&str>, debut: &Jour) -> Resultat<bool> {
    if imp.salarie() != cle || Jour::lire(imp.contrat_debut())? != *debut {
        return Ok(false);
    }
    Ok(numeros_compatibles(imp.contrat_numero(), numero))
}

/// Deux numeros de contrat se contredisent quand l'un et l'autre en ont un, et qu'ils different.
fn numeros_compatibles(a: Option<&str>, b: Option<&str>) -> bool {
    let (a, b) = (a.filter(|n| !n.is_empty()), b.filter(|n| !n.is_empty()));
    a.zip(b).is_none_or(|(a, b)| a == b)
}

// ------------------------------------------------------------- les motifs de suspension

/// Le motif de suspension (`permanents-base.json`, section `suspensions`), s'il existe.
pub fn motif_de_suspension(code: &str) -> Resultat<Option<MotifDeSuspension>> {
    droit::permanents::motif_de_suspension(code)
}

/// Ce que le motif d'une suspension interdit de declarer : un motif inconnu, ou le 200 (conges payes).
pub fn verifier_motif_de_suspension(code: &str, qui: &str) -> Resultat<MotifDeSuspension> {
    let Some(m) = motif_de_suspension(code)? else {
        return Err(socle::echec::cle(
            format!(
                "{} : motif de suspension {}, que le logiciel ne connaît pas : le choisir dans la liste des motifs.",
                qui,
                if code.is_empty() { "absent".to_string() } else { format!("inconnu ({})", repr(code)) }
            ),
            "motif",
        ));
    };
    if let Some(pourquoi) = &m.ne_se_declare_pas {
        return Err(format!(
            "Motif de suspension {} ({}) : {} Les conges payes pris se saisissent dans le mois (« conges pris »).",
            code, m.libelle, pourquoi
        ));
    }
    Ok(m)
}

// ------------------------------------------------------------ le bloc 65 d'une paie

/// UNE SUSPENSION DU CONTRAT QUE LA DSN DECLARE (bloc S21.G00.65) : son motif et ses deux dates entieres. L'engagement
/// la porte (`Engagement::suspensions`), sa forme ecrite aussi.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suspension {
    pub motif: String,
    pub debut: Jour,
    pub fin: Jour,
}

/// Les blocs S21.G00.65 d'une paie : chaque suspension du magasin qui suspend CE contrat et croise le mois que la DSN
/// declare, avec son motif et ses DEUX dates entieres (65.002 et 65.003) -- la meme dans chaque mois qu'elle traverse,
/// sans recopie (cahier technique DSN 2026.1, bloc S21.G00.65 p. 281, `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`).
pub fn suspensions_du_mois(ctx: &Contexte, paie: &crate::calcul::Paie, mois: &str) -> Resultat<Vec<Suspension>> {
    let cle = paie.salarie.id.to_string();
    let e = &paie.engagement;
    let qui = ctx.nomme(&cle);
    let mut out = Vec::new();
    let mut siennes = Vec::new();
    for imp in du_mois_du_salarie(&ctx.racine, mois, Some(SUSPENSION), &cle)? {
        if du_contrat(&imp, &cle, e.numero.as_deref(), &e.debut)? {
            siennes.push(imp);
        }
    }
    siennes.sort_by_key(|i| i.debut().to_string());
    for imp in siennes {
        let code = imp.motif().to_string();
        let motif = verifier_motif_de_suspension(&code, &qui)?;
        let (debut, fin) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        if fin < debut {
            return Err(format!(
                "{} : corrigez les dates de son absence « {} », dans « Les imprévus » : elle finit le {}, avant son \
                 premier jour, le {}.",
                qui,
                motif.libelle,
                fin.jj_mm_aaaa(),
                debut.jj_mm_aaaa()
            ));
        }
        out.push(Suspension { motif: code, debut, fin });
    }
    Ok(out)
}

/// LA PERIODE ENTIERE SUSPENDUE SANS REMUNERATION : les suspensions du magasin (bloc 65, pas les arrets) qui suspendent
/// ce contrat -- la fiche `cle`, le contrat commence le `contrat_debut`, de numero `numero` -- dont le motif n'est pas
/// remunere (`permanents-base.json`, `suspensions.motifs`, `remunere`), quand elles couvrent a elles toutes chaque jour de
/// [debut, fin] ; vide sinon. Un mois traverse en partie n'est pas entierement suspendu.
pub fn suspendu_sans_remuneration(
    racine: &Path,
    cle: &str,
    numero: Option<&str>,
    contrat_debut: &Jour,
    debut: &Jour,
    fin: &Jour,
) -> Resultat<Vec<Imprevu>> {
    let mut siennes = Vec::new();
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? || !croise(&imp, debut, fin)? {
            continue;
        }
        let remunere = motif_de_suspension(imp.motif())?.is_none_or(|m| m.remunere);
        if !remunere {
            siennes.push(imp);
        }
    }
    siennes.sort_by_key(|i| premier_jour(i).map(|j| j.iso()).unwrap_or_default());
    let mut couvert_jusqu_a = debut.plus_jours(-1)?;
    for imp in &siennes {
        if premier_jour(imp)? > couvert_jusqu_a.plus_jours(1)? {
            return Ok(Vec::new());
        }
        couvert_jusqu_a = couvert_jusqu_a.max(dernier_jour(imp)?);
    }
    Ok(if !siennes.is_empty() && couvert_jusqu_a >= *fin { siennes } else { Vec::new() })
}

/// LES JOURS DE CONGES QU'UNE SUSPENSION N'ACQUIERT PAS sur la presence [debut, fin] du contrat (`cle`, `numero`,
/// `contrat_debut`) : ceux des jours de ses suspensions dont le motif n'acquiert pas (`permanents-base.json`,
/// `suspensions.motifs`, `acquiert_conges`), au meme prorata des jours calendaires que l'acquisition
/// (`permanents::jours_acquis`). L3141-3 : 2,5 jours « par mois de travail effectif » ; L3141-5 et la CCN 3090, art.
/// 10.1.1, listent les periodes assimilees, et ni le conge parental, ni le sabbatique, ni le conge sans solde n'y sont
/// (`atelier/reference/sources/code-du-travail/articles-20261015.md`, `kali-3090-titres-vii-x-xi-annexes-20260924.md` ;
/// `suspension-non-remuneree/RAPPORT-20261015.md`, point 5). Un mois entierement suspendu n'acquiert rien ; un mois
/// suspendu en partie perd la part calendaire de la suspension (inference : L3141-4 compte en semaines de travail, le
/// moteur en jours calendaires, comme toute son acquisition). Les arrets de travail ne sont pas comptes ici.
pub fn jours_de_conges_non_acquis(
    racine: &Path,
    cle: &str,
    numero: Option<&str>,
    contrat_debut: &Jour,
    debut: &Jour,
    fin: &Jour,
) -> Resultat<Decimal> {
    let mut total = Decimal::ZERO;
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || !du_contrat(&imp, cle, numero, contrat_debut)? {
            continue;
        }
        let acquiert = motif_de_suspension(imp.motif())?.is_none_or(|m| m.acquiert_conges);
        if acquiert {
            continue;
        }
        let (d, f) = (premier_jour(&imp)?.max(*debut), dernier_jour(&imp)?.min(*fin));
        if d <= f {
            total += droit::permanents::jours_acquis(&d, &f)?;
        }
    }
    Ok(total)
}

/// L'ANCIENNETE QUE LES SUSPENSIONS LAISSENT : le jour d'ou se compte l'anciennete du salarie `cle` au
/// `a`, `depuis` recule des jours de suspension qui n'y comptent pas, et ce qu'il y a a en dire. Pour les droits que le
/// salarie tient de son anciennete (le maintien de salaire, L1226-1 ; la prime d'anciennete de la 3090) :
///
/// - le conge parental d'education a temps plein (632) « est prise en compte pour moitie pour la determination des droits
///   que le salarie tient de son anciennete » (C. trav. L1225-54, `atelier/reference/sources/suspension-non-remuneree/
///   legi-extrait-code-du-travail-20261015.md`) : la moitie de ses jours ne compte pas (arrondie au jour, au profit du
///   salarie) ;
/// - une autre suspension non remuneree (sabbatique 639, sans solde 501...) : AUCUN TEXTE DEPOSE ne dit comment elle
///   compte ; le moteur ne la compte pas (le temps suspendu n'est pas du travail effectif, et L1225-54 n'accorde la moitie
///   qu'au conge parental : inference) et l'avertit ;
/// - une suspension remuneree (637) compte entiere ; un arret de travail n'est pas touche ici.
///
/// Tous les contrats du salarie comptent : l'anciennete est celle de l'entreprise (`date_anciennete`).
pub fn anciennete_depuis(racine: &Path, cle: &str, depuis: &Jour, a: &Jour) -> Resultat<(Jour, Vec<Avis>)> {
    let mut retranches = 0i64;
    let mut dit = Vec::new();
    for imp in du_salarie(racine, cle)? {
        if !est_une_suspension(&imp) || imp.salarie() != cle {
            continue;
        }
        let code = imp.motif().to_string();
        let Some(motif) = motif_de_suspension(&code)? else { continue };
        if motif.remunere {
            continue;
        }
        let (d, f) = (premier_jour(&imp)?.max(*depuis), dernier_jour(&imp)?.min(*a));
        if f < d {
            continue;
        }
        let jours = f.moins(&d) + 1;
        let libelle = motif.libelle.clone();
        if motif.anciennete == Some(droit::permanents::AncienneteDeSuspension::Moitie) {
            retranches += jours / 2;
            dit.push(info(format!(
                "{} : {} ({}) du {} au {} -- {} jour(s) compte(s) pour moitie dans l'anciennete (C. trav. L1225-54).",
                cle,
                libelle,
                code,
                d.jj_mm_aaaa(),
                f.jj_mm_aaaa(),
                jours
            )));
        } else {
            retranches += jours;
            dit.push(avertir(format!(
                "{} : {} ({}) du {} au {} -- {} jour(s) NON comptes dans l'anciennete : aucun texte depose ne dit comment \
cette suspension compte (le conge parental seul compte pour moitie, L1225-54). Si la convention ou un accord de la compagnie \
la compte, a reprendre a la main.",
                cle,
                libelle,
                code,
                d.jj_mm_aaaa(),
                f.jj_mm_aaaa(),
                jours
            )));
        }
    }
    Ok((depuis.plus_jours(retranches)?, dit))
}

/// Les arrets du magasin qui suspendent ce contrat et croisent [debut, fin] (le FCTU les reporte, bloc 60).
pub fn arrets_du_contrat(ctx: &Contexte, paie: &crate::calcul::Paie, debut: &Jour, fin: &Jour) -> Resultat<Vec<ArretSaisi>> {
    let cle = paie.salarie.id.to_string();
    let mut out = Vec::new();
    for imp in du_salarie(&ctx.racine, &cle)? {
        let Imprevu::Arret(a) = imp else { continue };
        if du_contrat(&a, &cle, paie.engagement.numero.as_deref(), &paie.engagement.debut)? && arret_croise(&a, debut, fin)? {
            out.push(a);
        }
    }
    Ok(out)
}

// ------------------------------------------------------ les lignes d'un permanent

/// Ce que le mois d'un contrat paye au mois donne pour mesurer une absence (`conges::lignes_du_mois`) : les bornes du
/// contrat dans le mois, le salaire que la ligne « mois » paie et les jours travailles payes qu'elle porte
/// (`droit::periodes::part_du_mois`), l'horaire du salarie qui les compte et les heures d'un jour
/// (`droit::conges::heures_par_jour`), et ce que vaut un jour ouvrable (l'assiette du maintien de salaire que la loi
/// donne, dit sans etre applique).
pub struct MoisDuContrat<'a> {
    pub debut: Jour,
    pub fin: Jour,
    pub fin_mois: Jour,
    pub salaire_du_mois: Decimal,
    pub heures_par_jour: Decimal,
    pub jours_payes: i64,
    pub horaire: &'a droit::periodes::Horaire,
    pub taux_jour_ouvrable: Decimal,
}

/// UN TAUX SANS ARRONDI : le quotient tel quel, en texte (`num::dec` le relit sans perte), pour que la quantite fois le
/// taux, arrondi une fois au centime par la ligne (`calcul::lignes_brut`), donne le montant du quotient -- et le
/// salaire entier quand les heures sont toutes celles du mois. Un taux arrondi au centime AVANT de multiplier laissait
/// 0,07 EUR de salaire sur un mois entier d'absence (2 310 / 151,67 = 15,23, x 151,67 = 2 309,93) : aucun texte ne
/// l'arrondit (Cass. soc. 11/02/1982, ci-dessous, est un quotient).
fn taux_exact(montant: Decimal, quantite: Decimal) -> crate::Chiffre {
    crate::Chiffre::texte_de(div(montant, quantite).normalize())
}

/// LA FICHE DU SALARIE D'UN IMPREVU, rattachee a l'etablissement du contrat qu'il suspend (`calcul::etablissement_du_contrat`) :
/// un signalement d'arret porte l'etablissement de la derniere DSN mensuelle du contrat (cahier technique DSN 2026.1,
/// § 2.5.1, p. 67). Le contrat se lit dans la saisie du mois ou il commence ; introuvable, la fiche telle quelle.
pub fn fiche_du_salarie<I: Suspend>(ctx: &Contexte, imp: &I) -> Resultat<SalarieDuContrat> {
    let fiche = SalarieDuContrat::de(ctx.salarie(imp.salarie())?);
    let cle = fiche.id.to_string();
    let debut = Jour::lire(imp.contrat_debut())?;
    let mois = debut.mois_civil().to_string();
    let Some(data) = crate::stock::vue_du_mois(&ctx.racine, &mois)? else { return Ok(fiche) };
    for e in &data.engagements {
        let Some(debut_dit) = e.debut.as_deref().filter(|x| !x.is_empty()) else { continue };
        if Jour::lire(debut_dit)? != debut || e.cle(ctx).as_deref() != Some(cle.as_str()) {
            continue;
        }
        if !numeros_compatibles(imp.contrat_numero(), e.numero.as_deref()) {
            continue;
        }
        let mut f = fiche;
        f.etablissement = e.etablissement.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
        return Ok(f);
    }
    Ok(fiche)
}

/// Les imprevus du contrat qui croisent le mois, du premier au dernier jour : le contrat de `salarie` (le nom que la carte
/// ecrit) commence le `debut`, de numero `numero` (vide avant la validation).
pub fn du_contrat_dans_le_mois(ctx: &Contexte, salarie: &str, debut: &Jour, numero: Option<&str>, mois: &str) -> Resultat<Vec<Imprevu>> {
    let cle = ctx.cle_du_salarie(salarie)?.to_string();
    let mut out = Vec::new();
    for imp in du_mois_du_salarie(&ctx.racine, mois, None, &cle)? {
        if du_contrat(&imp, &cle, numero, debut)? {
            out.push(imp);
        }
    }
    out.sort_by_key(|i| premier_jour(i).map(|j| j.iso()).unwrap_or_default());
    Ok(out)
}

/// LES LIGNES QU'UN IMPREVU DONNE AU MOIS D'UN PERMANENT -- le seul endroit qui les tire. Chaque imprevu du contrat
/// retire le salaire de ses heures d'absence dans le mois (bornees par le contrat) ; un arret le retire sous
/// la rubrique de son motif (`arrets-base.json`, `rubrique` ; le catalogue la marque `arret_de_travail`), une suspension non remuneree sous
/// `absence_non_remuneree`. Une suspension que `permanents-base.json` dit remuneree (637 : C. trav. L3142-2, « n'entrainent
/// pas de reduction de la remuneration ») ne retire rien. Le maintien de salaire d'un arret est celui SAISI SUR L'ARRET
/// pour ce mois (`maintien`) : la loi le dit (`droit::maintien::legal`), elle ne le verse pas -- les indemnites journalieres a
/// deduire (D1226-5) ne se devinent pas.
///
/// LA RETENUE AU REEL DU MOIS. « La retenue par heure d'absence d'un salarie paye au mois doit etre en
/// principe egale au quotient du salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le mois
/// considere », et non sur un horaire moyen (Cass. soc. 11/02/1982, n° 80-40.359 ; 14/05/1987, n° 84-43.854 :
/// `atelier/reference/sources/suspension-non-remuneree/cass-soc-19820211-80-40359-JURITEXT000007009242.xml` et
/// `cass-soc-19870514-84-43854-JURITEXT000007018764.xml` ; la DDETS en tire « salaire mensualise / heures programmees sur
/// le mois », `dreets-limousin-qr-assmat-retenues-absence-20150101.txt`). Les heures d'absence se comptent au reel
/// (fiche DSN 2640, « sans application d'une regle de trois visant a rapprocher le volume d'absence a la quotite de
/// travail mensualisee » ; fiche 2693, exemple 10 : septembre 2026 entier, 154 h = 22 x 7 h ; exemple 4 : les jours
/// feries comptent parmi les heures du mois ; meme dossier) : les jours travailles de l'absence, comptes comme ceux du
/// salaire du mois (`droit::periodes::part_du_mois`, sur l'horaire du salarie : ses jours travailles, les feries qu'il ne
/// voit pas payes deja hors de son salaire), fois les heures d'un jour (`droit::conges::heures_par_jour`). La retenue est
/// le salaire du mois fois ces heures sur les heures payees du mois, au taux exact (`taux_exact`) : un mois entier
/// d'absence retire exactement le salaire du mois.
///
/// Rend aussi les jours travailles que les absences retirees ote du mois (un arret, une suspension non remuneree) :
/// `conges::lignes_du_mois` en tire les jours travailles du mois.
pub fn lignes_du_mois(ctx: &Contexte, e: &crate::saisie::Carte, mois: &str, m: &MoisDuContrat) -> Resultat<(Vec<crate::LignePaie>, Vec<Avis>, i64)> {
    let qui = crate::conges::qui(ctx, e);
    let (mut lignes, mut avert, mut absents) = (Vec::new(), Vec::new(), 0i64);
    let nat = e.nature_lue()?;
    let mut maintien = crate::maintien::DuMois::default();
    // Le taux de l'heure d'absence : le salaire du mois sur ses heures de travail (Cass. soc. 11/02/1982).
    let heures_du_mois = eur2(mul(m.heures_par_jour, Decimal::from(m.jours_payes)));
    let taux_horaire = if heures_du_mois > Decimal::ZERO { Some(taux_exact(m.salaire_du_mois, heures_du_mois)) } else { None };
    // Un contrat sans date de debut ecrite ne se designe pas : aucun imprevu ne peut le nommer (S21.G00.40.001).
    let imprevus = match crate::conges::jour_dit(&e.debut)? {
        Some(debut) => du_contrat_dans_le_mois(ctx, crate::conges::salarie_de(e)?, &debut, e.numero.as_deref(), mois)?,
        None => Vec::new(),
    };
    for imp in imprevus {
        let (premier, dernier) = (premier_jour(&imp)?, dernier_jour(&imp)?);
        let (debut, fin) = (premier.max(m.debut), dernier.min(m.fin));
        let jours = if debut <= fin {
            droit::periodes::part_du_mois(&debut, &fin, &m.fin_mois, droit::periodes::RegleDuMoisIncomplet::HoraireReel, m.horaire)?.jours
        } else {
            0
        };
        let heures = eur2(mul(m.heures_par_jour, Decimal::from(jours)));
        let retenue = |rubrique: &str| -> Option<crate::LignePaie> {
            match &taux_horaire {
                Some(t) if heures > Decimal::ZERO => Some(crate::LignePaie::de(rubrique, crate::Chiffre::de_f64(flot(heures)), t.clone())),
                _ => None,
            }
        };
        let au_reel = format!(
            "{} d'absence ce mois-ci, {} h sur les {} h de travail du mois, retirées au prorata du salaire du mois",
            if jours > 1 { format!("{} jours travaillés", jours) } else { format!("{} jour travaillé", jours) },
            socle::texte::virgule(heures),
            socle::texte::virgule(heures_du_mois)
        );
        let code = imp.motif().to_string();
        if let Imprevu::Arret(arret) = &imp {
            let motif = droit::arrets::motif_d_arret(&code)?;
            // Le bulletin nomme l'absence d'apres le motif de l'arret (S21.G00.60.001) : sa rubrique est celle du motif
            // (`arrets-base.json`, `rubrique`).
            let rubrique = motif.as_ref().and_then(|m| m.rubrique.clone()).unwrap_or_default();
            if rubrique.is_empty() {
                return Err(socle::echec::cle(format!(
                    "{} : l'arret du {} au {} a pour motif {} ({}), que le logiciel ne paie pas : aucune ligne de bulletin ne \
nomme cette absence. Rien n'est calcule.",
                    qui,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    code,
                    motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str())
                ), "arrets-base.json"));
            }
            lignes.extend(retenue(&rubrique).map(|l| crate::LignePaie { arret: Some(true), ..l }));
            absents += jours;
            avert.push(info(format!(
                "{} : arrêt de travail ({}) du {} au {} : {} ; la DSN de chaque mois qu'il traverse le déclare.",
                qui,
                motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str()),
                premier.jj_mm_aaaa(),
                dernier.jj_mm_aaaa(),
                au_reel
            )));
            if motif.as_ref().is_some_and(|m| m.origine.is_some()) {
                crate::maintien::ajouter(ctx, e, arret, &nat, mois, m, &mut maintien)?;
            }
        } else {
            let motif = verifier_motif_de_suspension(&code, &qui)?;
            let libelle = motif.libelle.clone();
            if motif.remunere {
                avert.push(info(format!(
                    "{} : absence ({}) du {} au {} : un congé rémunéré, rien n'est retiré du salaire ({}).",
                    qui,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    motif.texte()
                )));
            } else {
                lignes.extend(retenue("absence_non_remuneree"));
                absents += jours;
                avert.push(info(format!(
                    "{} : absence ({}) du {} au {} : {} ; la DSN de chaque mois qu'elle traverse la déclare.",
                    qui,
                    libelle,
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa(),
                    au_reel
                )));
            }
        }
    }
    lignes.extend(crate::maintien::lignes(&maintien, mois));
    avert.extend(maintien.avis);
    Ok((lignes, avert, absents))
}

// ------------------------------------------------------------- les jours payes

/// Les jours d'une ligne de saisie (`dates`), lus.
fn dates_de_ligne(l: &crate::LignePaie) -> Vec<Jour> {
    l.dates.iter().filter_map(|x| x.lu().ok()).collect()
}

/// Le libelle d'une rubrique, ou son code si le catalogue ne la connait pas.
fn libelle_rubrique(racine: &Path, code: &str, jour: &Jour) -> String {
    droit::catalogue::rubrique(racine, code, jour).map(|r| r.libelle).unwrap_or_else(|_| code.to_string())
}

/// LES CACHETS ET SERVICES DE LA SAISIE QUI TOMBENT DANS UN IMPREVU : pour chaque mois saisi qu'il croise, les lignes des
/// engagements du MEME salarie (tout contrat : un arret de travail est une suspension « durant laquelle le salarie ne
/// peut exercer son activite », bloc S21.G00.60 p. 265) dont une date tombe entre son premier et son dernier jour
/// d'absence. Une ligne par ligne de saisie, avec les seules dates qui tombent dedans, pour qu'on les retire une a une.
/// Le moteur ne retire rien (regle 1).
pub fn cachets_dans(ctx: &Contexte, imp: &Imprevu) -> Resultat<Vec<CachetPaye>> {
    let (premier, dernier) = (premier_jour(imp)?, dernier_jour(imp)?);
    let cle = imp.salarie().to_string();
    let mut out = Vec::new();
    if dernier < premier {
        return Ok(out);
    }
    for mois in socle::mois::Mois::entre(&premier, &dernier).iter().map(socle::mois::Mois::to_string) {
        let Some(data) = crate::stock::vue_du_mois(&ctx.racine, &mois)? else { continue };
        out.extend(cachets_de_la_saisie(ctx, &data, &mois, &cle, &premier, &dernier)?);
    }
    Ok(out)
}

/// UNE LIGNE DE LA SAISIE PAYEE PENDANT UN IMPREVU (`cachets_dans`) : son mois, l'engagement de la saisie et sa ligne
/// (leur rang), son contrat et son spectacle, sa rubrique et son libelle, ses seules dates qui tombent dans l'imprevu
/// (AAAA-MM-JJ), sa quantite et son taux.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CachetPaye {
    pub mois: String,
    pub engagement: usize,
    pub numero: Option<String>,
    pub spectacle: Option<String>,
    pub ligne: usize,
    pub rubrique: String,
    pub libelle: String,
    pub dates: Vec<String>,
    pub quantite: Option<Chiffre>,
    pub taux: Option<Chiffre>,
}

fn cachets_de_la_saisie(ctx: &Contexte, data: &crate::saisie::VueDuMois, mois: &str, cle: &str, premier: &Jour, dernier: &Jour) -> Resultat<Vec<CachetPaye>> {
    let mut out = Vec::new();
    for (i, e) in data.engagements.iter().enumerate() {
        if e.cle(ctx).as_deref() != Some(cle) {
            continue;
        }
        for (j, l) in e.lignes().iter().enumerate() {
            let dedans: Vec<String> = dates_de_ligne(l).into_iter().filter(|x| premier <= x && x <= dernier).map(|x| x.iso()).collect();
            if dedans.is_empty() {
                continue;
            }
            let rubrique = l.code().to_string();
            out.push(CachetPaye {
                mois: mois.to_string(),
                engagement: i,
                numero: e.numero.clone(),
                spectacle: e.spectacle.clone(),
                ligne: j,
                libelle: libelle_rubrique(&ctx.racine, &rubrique, &ctx.lue_au),
                rubrique,
                dates: dedans,
                quantite: l.quantite.clone(),
                taux: l.taux.clone(),
            });
        }
    }
    Ok(out)
}

/// UN JOUR PAYE PENDANT UN IMPREVU EST NOMME (regle 1 : le moteur montre, il ne retire rien). Pour chaque engagement de
/// la saisie (dans l'ordre de `engagements`), les avertissements de ses lignes datees d'un jour d'absence du salarie :
/// un arret de travail est une suspension « durant laquelle le salarie ne peut exercer son activite » (bloc S21.G00.60,
/// cahier technique DSN 2026.1 p. 265), une autre suspension, une raison « pour laquelle le contrat est suspendu »
/// (bloc S21.G00.65, p. 281).
pub fn avertir_les_jours_payes(ctx: &Contexte, data: &crate::saisie::VueDuMois) -> Resultat<Vec<Vec<Avis>>> {
    let engagements = &data.engagements;
    let mut out: Vec<Vec<Avis>> = vec![Vec::new(); engagements.len()];
    // Les imprevus des seuls salaries de la saisie (`du_salarie`).
    let mut salaries: Vec<String> = Vec::new();
    for cle in engagements.iter().filter_map(|e| e.cle(ctx)) {
        if !salaries.iter().any(|s| *s == cle) {
            salaries.push(cle.into());
        }
    }
    let mut siens = Vec::new();
    for cle in &salaries {
        siens.extend(du_salarie(&ctx.racine, cle)?);
    }
    let mois = data.mois.to_string();
    for imp in &siens {
        let (premier, dernier) = (premier_jour(imp)?, dernier_jour(imp)?);
        for c in cachets_de_la_saisie(ctx, data, &mois, imp.salarie(), &premier, &dernier)? {
            let i = c.engagement;
            let dates: Vec<String> = c.dates.iter().filter_map(|x| Jour::lire(x).ok()).map(|x| x.jj_mm_aaaa()).collect();
            let quoi = if est_un_arret(imp) {
                let motif = droit::arrets::motif_d_arret(imp.motif())?;
                format!(
                    "l'arret de travail ({}) du {} au {} : un salarie en arret ne peut exercer son activite (bloc S21.G00.60)",
                    motif.as_ref().map_or("inconnu de la norme", |m| m.libelle.as_str()),
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                )
            } else {
                format!(
                    "la suspension {} du {} au {} : le contrat est suspendu (bloc S21.G00.65)",
                    imp.motif(),
                    premier.jj_mm_aaaa(),
                    dernier.jj_mm_aaaa()
                )
            };
            if let Some(a) = out.get_mut(i) {
                a.push(avertir(format!(
                    "{} {} : {} le {} paye(s) pendant {}. Retirer ce jour de la ligne, ou corriger les dates de l'imprevu.",
                    imp.salarie(),
                    c.numero.as_ref().map_or_else(String::new, |n| format!("(contrat {})", n)),
                    c.libelle,
                    dates.join(", "),
                    quoi
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{json, Value};

    fn lu(v: Value) -> socle::num::Resultat<Imprevu> {
        socle::forme::lire(&v, ILLISIBLE)
    }

    /// Un arret tel que le moteur l'ecrit : chaque cle y est, nulle quand elle n'a pas de valeur.
    fn arret_entier() -> Value {
        json!({"genre": "arret", "salarie": "X", "contrat_debut": "2026-01-01", "contrat_numero": "N",
            "motif": "01", "dernier_jour_travaille": "2026-02-02", "debut": "2026-02-03", "fin_previsionnelle": "2026-02-10",
            "subrogation": null, "subrogation_debut": null, "subrogation_fin": null, "reprise": null, "reprise_motif": null,
            "date_accident": null, "jours_deja_indemnises": null, "ij_brute": null, "ij_nette": null, "ij_brute_majoree": null,
            "ij_nette_majoree": null, "prolongations": [], "reprises": [], "saisi_le": "2026-02-03T10:00:00", "qui": "interface", "note": ""})
    }

    /// Une suspension telle que le moteur l'ecrit.
    fn suspension_entiere() -> Value {
        json!({"genre": "suspension", "salarie": "X", "contrat_debut": "2026-01-01", "contrat_numero": "N",
            "motif": "632", "debut": "2026-02-01", "fin": "2026-04-30", "saisi_le": "2026-02-01T10:00:00", "qui": "interface", "note": ""})
    }

    /// Un imprevu se relit tel qu'il s'ecrit, dans l'ordre de ses cles ; une cle manquante, une cle inconnue ou un genre
    /// inconnu refusent : rien n'est rempli a la place.
    #[test]
    fn un_imprevu_se_relit_entier_et_refuse_ce_qui_manque_ou_qu_il_ne_connait_pas() {
        for entier in [arret_entier(), suspension_entiere()] {
            let relu = socle::ecriture::en_valeur(&lu(entier.clone()).unwrap());
            assert_eq!(relu, entier);
            for cle in entier.as_object().unwrap().keys() {
                let mut moins = entier.clone();
                moins.as_object_mut().unwrap().remove(cle);
                assert!(lu(moins).is_err(), "{} manque et la lecture ne refuse pas", cle);
            }
            let mut plus = entier.clone();
            plus["heures"] = json!(3);
            assert!(lu(plus).is_err());
        }
        assert!(lu(json!({"genre": "autre"})).is_err());
    }

    /// S21.G00.60.002 : « la veille de la date de debut de la prescription de l'arret de travail, sauf [...] arret prescrit
    /// lors d'un jour travaille : le dernier jour travaille correspond alors a la date de l'arret » (bloc-S21.G00.60.txt,
    /// p. 266-267) ; l'absence va du lendemain a la veille de la reprise (S21.G00.60.010).
    #[test]
    fn la_dsn_declare_la_veille_du_debut_prescrit_et_l_absence_court_de_ce_debut() {
        let mut v = arret_entier();
        v["dernier_jour_travaille"] = json!("2026-03-06");
        v["debut"] = json!("2026-03-09");
        v["fin_previsionnelle"] = json!("2026-03-20");
        let a = lu(v.clone()).unwrap();
        assert_eq!((dernier_jour_declare(a.arret().unwrap()).unwrap().iso(), premier_jour(&a).unwrap().iso()), ("2026-03-08".into(), "2026-03-09".into()));
        v["debut"] = json!("2026-03-06");
        let a = lu(v.clone()).unwrap();
        assert_eq!((dernier_jour_declare(a.arret().unwrap()).unwrap().iso(), premier_jour(&a).unwrap().iso()), ("2026-03-06".into(), "2026-03-07".into()));
        assert_eq!(dernier_jour(&a).unwrap().iso(), "2026-03-20");
        v["reprise"] = json!("2026-03-15");
        assert_eq!(dernier_jour(&lu(v).unwrap()).unwrap().iso(), "2026-03-14");
    }
}
