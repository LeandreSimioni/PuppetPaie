//! La route `dossier-salarie` : tous les documents d'un salarie sur une periode, en un PDF par categorie, ou en un ZIP,
//! ecrits dans `exports/dossiers/`.
//!
//! UN DOCUMENT REMIS NE SE REGENERE PAS (regle 4) : les bulletins d'un mois ecrit sont les pages memes de
//! `sorties/<mois>/Bulletins_<Mois>.pdf`, les certificats Conges Spectacles celles de `CertificatsCS_<Mois>.pdf` -- a
//! chaque salarie les siennes, rien de recalcule ni de reimprime ; ce qui ne se lit pas refuse. Les contrats, eux, se
//! recreent depuis la saisie (aucun compteur n'avance ; les numeros se lisent dans le fichier groupe archive ou dans la
//! saisie quand ils y sont figes) et portent en pied la mention « Reedite ». Le contrat signe (revenu de la signature, ou
//! depose par la compagnie quand il a ete signe hors du logiciel), l'AER, les documents de fin et les PDF de CoolPaie se
//! prennent tels quels, page a page, dedoublonnes.
//!
//! `dossier-envoyer` (`archives`) envoie ce dossier-ci, construit par `construire`.
//!
//! Le texte de CoolPaie se normalise par la table latine de `contexte::sans_accent` (Latin-1 et Latin etendu A) : un
//! accent hors de cette table n'est pas retire. Les chiffres des motifs de date et de nom de fichier sont les chiffres
//! ASCII. Un mois saisi est un mois que des engagements touchent (`engagements/`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use socle::mois::Mois;

use engagement::calcul::Paie;
use documents::conges_spectacles::{self, ContratCs, MoisPaye, PaieCs};
use droit::contexte::Contexte;
use socle::date::Jour;
use engagement::{self, Engagement};
use documents::fiche_recapitulative::Recapitulatifs;
use documents::fabrique::{fusion_octets, fusion_pieces, Temporaire};
use socle::num::{eur2, Decimal};
use socle::echec::Echec;
use socle::echec::Refus;

type R<T> = Result<T, Refus>;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// La route de ce module, et sa fiche.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::DossierSalarie,
    geste: "Assembler les documents de salariés sur une période, en PDF ou en ZIP.",
    charge: &[
        Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "les clés des salariés", avance: false },
        Champ { nom: "mois", libelle: Affiche::new("Période"), forme: Forme::Texte, sens: "un mois AAAA-MM ; ou annee, ou toutes_les_annees", avance: false },
        Champ { nom: "categories", libelle: Affiche::new("Sortes de documents"), forme: Forme::Liste, sens: "les sortes de documents (bulletins, contrats…) ; toutes par défaut", avance: true },
        Champ { nom: "ensemble", libelle: Affiche::new("Un seul fichier"), forme: Forme::OuiNon, sens: "vrai : un seul fichier pour tous", avance: true },
        Champ { nom: "apercu", libelle: Affiche::new("Aperçu seulement"), forme: Forme::OuiNon, sens: "vrai : dire ce qui serait assemblé, sans rien écrire", avance: true },
    ],
    exemple: r#"{"salaries":["FULLER"],"mois":"2026-11","apercu":true}"#,
    rend: "les fichiers écrits (ou à écrire, en aperçu), par salarié et par sorte",
    refus: &[OPERATION, MANQUE, ILLISIBLE, FORME],
    acte: Acte::Ecrit,
    avance: false,
    phrase: 0,
    ecran: ECRAN,
}];


/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> Result<Value, Echec> {
    match nom {
        Route::DossierSalarie => super::en_json(dossier_salarie(ctx, &super::lire_charge(charge)?)?),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

/// `dossier-salarie` : le dossier construit (`construire`), et ce qu'il a ecrit.
fn dossier_salarie(ctx: &Contexte, demande: &DemandeDeDossier) -> R<DossierRendu> {
    let d = construire(ctx, demande)?;
    let p = if d.fichiers.is_empty() {
        format!("Dossier de {} ({}) : rien n'est écrit.", d.salaries.join(", "), d.periode)
    } else {
        format!("Dossier de {} ({}) : {}.", d.salaries.join(", "), d.periode, super::compte(d.fichiers.len(), "fichier écrit", "fichiers écrits"))
    };
    Ok(DossierRendu::de(d, p, None))
}

// ------------------------------------------------------------------ les formes

/// L'annee d'une demande, en nombre (AAAA) ou telle que la page la choisit (« 2026 »).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum AnneeDite {
    Nombre(i64),
    Texte(String),
}

impl AnneeDite {
    pub(crate) fn texte(&self) -> String {
        match self {
            AnneeDite::Nombre(n) => n.to_string(),
            AnneeDite::Texte(t) => t.clone(),
        }
    }
}

/// La demande d'un dossier (la charge de `dossier-salarie` ; `dossier-envoyer` la compose) : les salaries (aucun :
/// chacun), UNE periode (le mois, l'annee, ou toutes les annees), ce qu'il porte (rien : tout), un seul fichier pour
/// tous (`ensemble`), et `apercu` pour dire ce qui s'ecrirait sans rien ecrire.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DemandeDeDossier {
    #[serde(default)]
    pub(crate) salaries: Vec<String>,
    #[serde(default)]
    pub(crate) mois: Option<Mois>,
    #[serde(default)]
    pub(crate) annee: Option<AnneeDite>,
    #[serde(default)]
    pub(crate) toutes_les_annees: bool,
    #[serde(default)]
    pub(crate) categories: Vec<String>,
    #[serde(default)]
    pub(crate) ensemble: bool,
    #[serde(default)]
    pub(crate) apercu: bool,
}

/// Une table JSON dont les cles gardent l'ordre ou elles sont venues (les salaries, puis leurs categories).
#[derive(Debug, Clone)]
pub(crate) struct Ordonnee<V>(pub(crate) Vec<(String, V)>);

impl<V> Default for Ordonnee<V> {
    fn default() -> Self {
        Ordonnee(Vec::new())
    }
}

impl<V> Ordonnee<V> {
    pub(crate) fn get(&self, cle: &str) -> Option<&V> {
        self.0.iter().find(|(k, _)| k == cle).map(|(_, v)| v)
    }

    fn entree(&mut self, cle: &str, neuf: impl FnOnce() -> V) -> &mut V {
        let i = match self.0.iter().position(|(k, _)| k == cle) {
            Some(i) => i,
            None => {
                self.0.push((cle.to_string(), neuf()));
                self.0.len() - 1
            }
        };
        &mut self.0[i].1
    }
}

impl<V: Serialize> Serialize for Ordonnee<V> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.iter().map(|(k, v)| (k, v)))
    }
}

/// Ce que l'envoi du dossier a fait : les adresses servies, le nombre de messages, la boite qui envoie.
#[derive(Debug, Serialize)]
pub(crate) struct DossierEnvoye {
    pub(crate) partis: Vec<String>,
    pub(crate) messages: usize,
    pub(crate) expediteur: String,
}

/// Ce que rendent `dossier-salarie` et `dossier-envoyer` : la phrase, le dossier (ses salaries, ses categories, sa
/// periode, ses fichiers, ses pages par salarie et par categorie, ce qu'il dit), le libelle de chaque categorie, et
/// sous `envoi` ce que l'envoi a fait.
#[derive(Debug, Serialize)]
pub(crate) struct DossierRendu {
    phrase: String,
    salaries: Vec<String>,
    categories: Vec<&'static str>,
    periode: String,
    fichiers: Vec<String>,
    pages: Ordonnee<Ordonnee<usize>>,
    avertissements: Vec<String>,
    refus: Vec<String>,
    libelles: Ordonnee<&'static str>,
    phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    envoi: Option<DossierEnvoye>,
}

impl DossierRendu {
    pub(crate) fn de(d: Dossier, phrase: String, envoi: Option<DossierEnvoye>) -> DossierRendu {
        let phrases = d.avertissements.iter().chain(&d.refus).cloned().collect();
        DossierRendu {
            phrase,
            salaries: d.salaries,
            categories: d.categories,
            periode: d.periode,
            fichiers: d.fichiers.iter().map(|f| f.to_string_lossy().into_owned()).collect(),
            pages: d.pages,
            avertissements: d.avertissements,
            refus: d.refus,
            libelles: Ordonnee(CATEGORIES.iter().map(|c| (c.to_string(), libelle_de(c))).collect()),
            phrases,
            envoi,
        }
    }
}

// ------------------------------------------------------------------ les categories

const BULLETINS: &str = "bulletins";
const CONTRATS: &str = "contrats";
const CONTRATS_SIGNES: &str = "contrats-signes";
const CERTIFICATS_CS: &str = "certificats-cs";
const ATTESTATIONS: &str = "attestations";
const DOCUMENTS_FIN: &str = "documents-fin";
const RECAPITULATIF: &str = "recapitulatif";
pub(crate) const CATEGORIES: [&str; 7] = [BULLETINS, CONTRATS, CONTRATS_SIGNES, CERTIFICATS_CS, ATTESTATIONS, DOCUMENTS_FIN, RECAPITULATIF];

pub(crate) fn libelle_de(c: &str) -> &'static str {
    match c {
        BULLETINS => "Bulletins de paie",
        CONTRATS => "Contrats",
        CONTRATS_SIGNES => "Contrats signés",
        CERTIFICATS_CS => "Certificats Congés Spectacles",
        ATTESTATIONS => "Attestations employeur (AER, AEM)",
        DOCUMENTS_FIN => "Documents de fin de contrat",
        _ => "Fiche récapitulative",
    }
}

fn nom_de(c: &str) -> &'static str {
    match c {
        BULLETINS => "Bulletins",
        CONTRATS => "Contrats",
        CONTRATS_SIGNES => "Contrats-signes",
        CERTIFICATS_CS => "Certificats-Conges-Spectacles",
        ATTESTATIONS => "Attestations-employeur",
        DOCUMENTS_FIN => "Documents-de-fin",
        _ => "Fiche-recapitulative",
    }
}

fn rang_de(c: &str) -> usize {
    CATEGORIES.iter().position(|x| *x == c).unwrap_or(usize::MAX)
}

const PIED_REEDITE: &str = "Réédité par PuppetPaie le {} depuis les données de paie — numéros et montants identiques au document remis ; mise en page d'aujourd'hui.";


// ------------------------------------------------------------------ les refus

pub(crate) fn avec(classe: &'static str, message: impl Into<String>) -> Refus {
    Refus { classe: Some(classe), message: message.into() }
}

fn erreur_operation(message: impl Into<String>) -> Refus {
    avec("ErreurOperation", message)
}

/// Un refus de la fabrique de PDF (`FabriqueRefus`) : sa ligne de francais.
fn refus_fabrique(m: String) -> Refus {
    avec("FabriqueRefus", m.replace('\n', " ").trim().to_string())
}

// ------------------------------------------------------------------ la fabrique de PDF

fn texte_pdf(chemin: &Path) -> R<Vec<String>> {
    fabrique_pdf::lecture::texte(&chemin.to_string_lossy()).map(|(t, _)| t).map_err(|r| refus_fabrique(r.0))
}

fn pages_pdf(chemin: &Path) -> R<usize> {
    fabrique_pdf::lecture::pages(&[chemin.to_string_lossy().into_owned()])
        .map(|n| n[0])
        .map_err(|r| refus_fabrique(r.0))
}

fn compter(octets: &[u8]) -> R<usize> {
    fabrique_pdf::lecture::charger(octets).map(|d| d.get_pages().len()).map_err(|r| refus_fabrique(r.0))
}

/// Le pied « reedite » sur chaque page d'un PDF du dossier de travail, en place.
fn marquer(chemin: &Path, aujourdhui: &Jour) -> R<()> {
    let nom = chemin.to_string_lossy().into_owned();
    let octets = std::fs::read(chemin).map_err(|e| refus_fabrique(format!("{} illisible ({})", nom, e)))?;
    let pied = PIED_REEDITE.replace("{}", &aujourdhui.jj_mm_aaaa());
    let marque = fabrique_pdf::marquage::marquer(&octets, &pied).map_err(|r| refus_fabrique(format!("{} : {}", nom, r.0)))?;
    std::fs::write(chemin, marque).map_err(|e| refus_fabrique(format!("écriture impossible de {} : {}", nom, e)))
}

// ------------------------------------------------------------------ la periode

/// Un chiffre lu dans un nom (`\d`) : ASCII, arabe, devanagari, pleine chasse.
fn chiffre(c: char) -> Option<u32> {
    crate::routes::fiches::chiffre_lu(c)
}

/// Un mois, une annee, ou tout. Les bornes servent aux pages CoolPaie, qui n'ont qu'une date.
struct Periode {
    mois: Option<String>,
    annee: Option<String>,
    toutes: bool,
}

impl Periode {
    fn nouveau(mois: Option<Mois>, annee: Option<String>, toutes: bool) -> R<Periode> {
        let n = mois.is_some() as usize + annee.is_some() as usize + toutes as usize;
        if n != 1 {
            return Err(erreur_operation(
                "Un dossier se demande pour UNE période : un mois (--mois AAAA-MM), une année (--annee AAAA), ou toute la période (--toutes-les-annees).",
            ));
        }
        let mut p = Periode { mois: mois.map(|m| m.to_string()), annee: None, toutes };
        if let Some(t) = annee {
            if !(t.chars().count() == 4 && t.chars().all(|x| chiffre(*&x).is_some())) {
                return Err(erreur_operation(format!("Année attendue sous la forme AAAA : « {} ».", t)));
            }
            p.annee = Some(t);
        }
        Ok(p)
    }

    /// `m` : AAAA-MM.
    fn contient(&self, m: &str) -> bool {
        if self.toutes {
            return true;
        }
        match &self.mois {
            Some(x) => m == x,
            None => m.starts_with(self.annee.as_deref().unwrap_or("")),
        }
    }

    fn libelle(&self) -> R<String> {
        if self.toutes {
            return Ok("toute la période".to_string());
        }
        if let Some(m) = &self.mois {
            return socle::mois::Mois::lire(m).map(|m| m.libelle(socle::mois::Graphie::Minuscule)).map_err(|e| avec("ValueError", e));
        }
        Ok(self.annee.clone().unwrap_or_default())
    }

    fn nom(&self) -> String {
        if self.toutes {
            return "toute-la-periode".to_string();
        }
        self.mois.clone().or_else(|| self.annee.clone()).unwrap_or_default()
    }
}

// ------------------------------------------------------------------ ce qu'on rend

/// Une page du dossier, d'ou qu'elle vienne.
struct Page {
    categorie: &'static str,
    cle: String,
    /// AAAA-MM-JJ : l'ordre de lecture.
    tri: String,
    /// Le PDF d'ou la page est tiree.
    source: PathBuf,
    /// La page dans ce PDF (0 = la premiere).
    numero: usize,
}

#[derive(Default)]
pub(crate) struct Dossier {
    pub(crate) salaries: Vec<String>,
    pub(crate) categories: Vec<&'static str>,
    pub(crate) periode: String,
    pub(crate) fichiers: Vec<PathBuf>,
    /// `{cle: {categorie: n}}`, dans l'ordre ou les cles et les categories sont apparues.
    pub(crate) pages: Ordonnee<Ordonnee<usize>>,
    avertissements: Vec<String>,
    refus: Vec<String>,
    /// Les PDF de contrats signes hors du logiciel deja pris (un contrat court sur plusieurs mois).
    deposes_vus: HashSet<PathBuf>,
}


// ------------------------------------------------------------------ les noms de fichier

/// Les noms d'un dossier qui commencent par `debut` et finissent par `fin`, dans
/// l'ordre des noms.
fn noms_commencant_finissant(dossier: &Path, debut: &str, fin: &str) -> Vec<String> {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut out: Vec<String> = lecture
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.len() >= debut.len() + fin.len() && n.starts_with(debut) && n.ends_with(fin))
        .collect();
    out.sort();
    out
}

/// `glob("Contrat*_<cle>_*_signe.pdf")`.
fn contrats_signes_de(dossier: &Path, cle: &str) -> Vec<String> {
    let marque = format!("_{}_", cle);
    let fin = envois::releve_signatures::SUFFIXE_CONTRESIGNE;
    noms_commencant_finissant(dossier, "Contrat", fin)
        .into_iter()
        .filter(|n| {
            let mut de = "Contrat".len();
            while let Some(i) = n.get(de..).and_then(|r| r.find(&marque)) {
                let apres = de + i + marque.len();
                if n.len() >= apres + fin.len() {
                    return true;
                }
                de += i + 1;
                while !n.is_char_boundary(de) {
                    de += 1;
                }
            }
            false
        })
        .collect()
}

/// Les numéros de contrat d'un nom de fichier : des capitales, des chiffres, un point, deux chiffres, un point, des chiffres.
fn numeros_de_contrat(nom: &str) -> Vec<String> {
    let c: Vec<char> = nom.chars().collect();
    let chiffres = |de: usize| c.get(de..).map_or(0, |r| r.iter().take_while(|x| x.is_ascii_digit()).count());
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let lettres = c[i..].iter().take_while(|x| x.is_ascii_uppercase()).count();
        let mut fini = None;
        if lettres > 0 {
            let a = i + lettres;
            let n1 = chiffres(a);
            let b = a + n1;
            if n1 > 0 && c.get(b) == Some(&'.') && chiffres(b + 1) == 2 && c.get(b + 3) == Some(&'.') {
                let n3 = chiffres(b + 4);
                if n3 > 0 {
                    fini = Some(b + 4 + n3);
                }
            }
        }
        match fini {
            Some(f) => {
                out.push(c[i..f].iter().collect());
                i = f;
            }
            None => i += 1,
        }
    }
    out
}

/// Le nom d'un salarie pour un nom de fichier : ce qui n'est ni chiffre, ni lettre ASCII, ni tiret devient un tiret.
fn nom_salarie(cle: &str) -> String {
    let mut sortie = String::new();
    let mut dans_un_trou = false;
    for c in cle.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            sortie.push(c);
            dans_un_trou = false;
        } else if !dans_un_trou {
            sortie.push('-');
            dans_un_trou = true;
        }
    }
    let net = sortie.trim_matches('-').to_string();
    if net.is_empty() {
        "salarie".to_string()
    } else {
        net
    }
}

// ------------------------------------------------------------------ les mois PuppetPaie

pub(crate) fn cle_de(p: &Paie) -> String {
    p.salarie.id.to_string()
}

/// Ce qui peut partir de `mois` (`engagement::etats::validees_du_mois`) : la saisie des engagements valides et leurs paies
/// ecrites, sans numeros ; un mois sans saisie n'a rien.
pub(crate) fn charger(ctx: &Contexte, mois: &str) -> R<(engagement::saisie::VueDuMois, Vec<Paie>)> {
    let vide = engagement::saisie::VueDuMois::vide(socle::mois::Mois::lire(mois).map_err(|m| avec("ValueError", m))?);
    Ok(engagement::etats::validees_du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(Echec::from)?.unwrap_or((vide, Vec::new())))
}

/// `numeroter` d'un mois dont les numeros ne sont pas dans l'archive : les compteurs en attente
/// d'un mois a l'autre restent dans `attente`, comme `Contexte._compteurs_en_attente`.
fn numeroter(ctx: &Contexte, data: &engagement::saisie::VueDuMois, paies: &mut [Paie], attente: &mut HashMap<i32, i64>) -> R<()> {
    documents::ecrire_documents::numeroter_en_attente(ctx, paies, data.mois.annee(), attente, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)
}

/// L'etiquette d'un mois dans le nom d'un fichier (`Aout2026`) ; un mois illisible se refuse (`ValueError`).
pub(crate) fn etiquette(mois: &str) -> R<String> {
    socle::mois::Mois::lire(mois).map(socle::mois::Mois::etiquette).map_err(|m| avec("ValueError", m))
}


/// `Paie` -> ce que le certificat en lit : le brut est celui du calcul, a deux decimales.
pub(crate) fn paie_cs(p: &Paie) -> R<PaieCs> {
    Ok(PaieCs { engagement: ContratCs::de(&p.engagement)?, brut: eur2(p.brut), salarie: p.salarie.champs.clone() })
}

/// Ce que l'ancien logiciel a verse en `mois` pour un contrat qui continue ici : le brut, les cachets (une ligne, aux
/// jours que le bulletin imprime) et les jours travailles. `None` sans bulletin qui donne ses jours et ses cachets : le
/// mois reste parmi les manquants (rien ne se devine).
fn verse_par_l_ancien_logiciel(i: &engagement::importe::Importe, mois: &str) -> Option<(Decimal, Vec<engagement::LignePaie>, i64)> {
    let p = i.paies().get(mois)?;
    let brut = eur2(p.brut?);
    let mut dates: Vec<String> = Vec::new();
    for d in &p.dates {
        let mut j = d.debut;
        while j <= d.fin {
            dates.push(j.iso());
            j = j.plus_jours(1).ok()?;
        }
    }
    let cachets = p.lignes.iter().find(|l| l.rubrique.as_deref() == Some("cachet"))?;
    let (quantite, taux) = (Some(cachets.quantite?), Some(cachets.taux?));
    if dates.is_empty() {
        return None;
    }
    let jours = dates.len() as i64;
    let cachet = engagement::LignePaie::Unites(engagement::ligne::Unites {
        rubrique: "cachet".to_string(),
        quantite,
        taux,
        lieu: None,
        dates: dates.into_iter().map(engagement::DateDeLigne::Nue).collect(),
        planning: Vec::new(),
    });
    Some((brut, vec![cachet], jours))
}

/// `paie_du_contrat_entier` : le contrat recompose sur tous ses mois, pour son certificat. Les mois d'avant se lisent
/// dans les paies validees de l'engagement (`engagement::etats::paie_ecrite_de`). Rend la paie du certificat et les
/// mois du contrat dont aucune paie n'est ecrite.
fn paie_du_contrat_entier(ctx: &Contexte, p: &Paie) -> R<(PaieCs, Vec<String>)> {
    let eng = &p.engagement;
    let cle = cle_de(p);
    let mois_du = eng.mois_du_contrat()?;
    let debut = eng.debut.iso();
    let mut manquants: Vec<String> = Vec::new();
    let mut lignes: Vec<engagement::LignePaie> = Vec::new();
    // Un contrat commence dans l'ancien logiciel : ses mois d'avant sont ceux que l'engagement importe a verses.
    let cherche = engagement::saisie::Carte { salarie: Some(cle.to_string()), debut: Some(debut.clone()), numero: eng.numero.as_ref().map(|n| n.to_string()), ..Default::default() };
    let importe = engagement::importe::continue_par(&ctx.racine, &cherche).map_err(|e| avec("ValueError", e))?;
    let mut bruts = Decimal::new(0, 2);
    let mut jours = 0i64;
    let mut mois_payes: Vec<MoisPaye> = Vec::new();
    for m in mois_du.iter().filter(|m| **m < eng.mois) {
        let ecrite = match eng.numero.as_deref() {
            Some(n) => engagement::etats::paie_ecrite_de(ctx, m, &cle, n, &envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(|e| avec("ValueError", e))?).map_err(|e| avec("ValueError", e))?,
            None => None,
        };
        let Some(paie) = ecrite.as_ref().map(|x| &x.paie) else {
            match importe.as_ref().and_then(|i| verse_par_l_ancien_logiciel(i, m)) {
                Some((brut, l, j)) => {
                    bruts += brut;
                    lignes.extend(l);
                    jours += j;
                }
                None => manquants.push(m.clone()),
            }
            continue;
        };
        lignes.extend(paie.engagement.lignes.iter().cloned());
        let (brut_du_mois, jours_du_mois) = (eur2(paie.brut), paie.engagement.jours);
        bruts += brut_du_mois;
        jours += jours_du_mois;
        mois_payes.push(MoisPaye { fin: socle::mois::Mois::lire(m).map(socle::mois::Mois::dernier_jour).map_err(|x| avec("ValueError", x))?, jours: jours_du_mois, brut: brut_du_mois });
    }
    let brut = bruts + eur2(p.brut);
    // Un contrat paye au mois n'a ni cachet ni date jouee : son certificat porte ses bornes, et ses jours travailles et son
    // brut mois par mois (`conges::lignes_du_mois`) ; le 31 mars le coupe entre deux mois.
    if eng.nat()?.mensuelle() {
        let fin_du_mois = socle::mois::Mois::lire(&eng.mois).map(socle::mois::Mois::dernier_jour).map_err(|x| avec("ValueError", x))?;
        mois_payes.push(MoisPaye { fin: fin_du_mois, jours: eng.jours, brut: eur2(p.brut) });
        let contrat = ContratCs { jours: jours + eng.jours, dates_travail: Vec::new(), cachets: 0, heures: None, taux_cachet: None, mois_payes, ..ContratCs::de(eng)? };
        return Ok((PaieCs { engagement: contrat, brut, salarie: p.salarie.champs.clone() }, manquants));
    }
    lignes.extend(eng.lignes.iter().cloned());
    // Le planning des mois passes a ete paye dans son mois : il ne compte pas deux fois.
    for l in lignes.iter_mut() {
        *l = l.avec_les_jours(l.dates().to_vec(), Vec::new());
    }
    // Les jours du contrat entier : ceux de ses lignes de travail (`engagement::dates_engagement`).
    let dates = engagement::saisie::Carte { lignes: Some(lignes.clone()), ..Default::default() }.dates();
    let entier = engagement::Assemble {
        salarie: Some(eng.salarie.clone()),
        spectacle: eng.spectacle.clone(),
        dates_travail: dates,
        lignes: Some(lignes),
        debut: Some(eng.debut.iso()),
        fin: eng.fin.map(|f| f.iso()),
        numero: eng.numero.as_ref().map(|n| n.to_string()),
        jours: Some(engagement::Chiffre::entier(jours + eng.jours)),
        taux_cachet: eng.taux_cachet.map(engagement::Chiffre::de_f64),
        date_paiement: eng.date_paiement.clone(),
        lieu: Some(eng.lieu.clone()),
        emploi: eng.emploi.clone(),
        pcs_ese: eng.pcs_ese.clone(),
        code_emploi_ft: eng.code_emploi_ft.clone(),
        statut: eng.statut.clone(),
        statut_conventionnel: eng.statut_conventionnel.clone(),
        statut_categoriel_rc: eng.statut_categoriel_rc.clone(),
        ..Default::default()
    };
    let entier = Engagement::nouveau(&ctx.racine, &entier, &ctx.nomme(&eng.salarie)).map_err(|m| avec("ValueError", m))?;
    let cs = PaieCs { engagement: ContratCs::de(&entier)?, brut, salarie: p.salarie.champs.clone() };
    Ok((cs, manquants))
}

/// Un certificat du : `(salarie, debut du contrat, paie du certificat)`.
pub(crate) type Du = (String, String, Option<PaieCs>);

/// `certificats_dus` : un certificat par contrat qui FINIT, pour le contrat entier quand il deborde
/// du mois. Rend les certificats (dans l'ordre ou ils sont dus) et les avertissements.
pub(crate) fn certificats_dus(ctx: &Contexte, paies: &[Paie]) -> R<(Vec<Du>, Vec<String>)> {
    let mut dus: Vec<Du> = Vec::new();
    let mut avertissements = Vec::new();
    let poser = |dus: &mut Vec<Du>, cle: String, debut: String, v: Option<PaieCs>| match dus.iter_mut().find(|d| d.0 == cle && d.1 == debut) {
        Some(d) => d.2 = v,
        None => dus.push((cle, debut, v)),
    };
    for p in paies {
        // Le certificat suit la declaration a la caisse (`caisse_conges::affilie`, code du travail D. 7121-41) : « Un
        // certificat d'emploi doit etre remis au salarie par son employeur a l'issue de chacun de ses contrats de
        // travail » d'artiste ou de technicien « embauche sous contrat a duree determinee de moins de 12 mois », CDD de
        // droit commun ou d'usage (fiche Audiens, `majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-
        // 202302.txt`, l. 20-31). Un permanent, un contrat de douze mois ou plus n'en ont pas.
        if !p.declaree_a_la_caisse_des_conges()? {
            continue;
        }
        let (cle, debut) = (cle_de(p), p.engagement.debut.iso());
        if !p.engagement.dernier_mois()? {
            poser(&mut dus, cle, debut, None);
        } else if !p.engagement.a_cheval()? {
            poser(&mut dus, cle, debut, Some(paie_cs(p)?));
        } else {
            let (entier, manquants) = paie_du_contrat_entier(ctx, p)?;
            poser(&mut dus, cle, debut, Some(entier));
            if !manquants.is_empty() {
                avertissements.push(format!(
                    "Certificat Conges Spectacles du contrat {} : aucune paie sur le disque pour {} -- il ne porte que les mois presents.",
                    p.engagement.numero.as_deref().unwrap_or("None"),
                    manquants.iter().map(|m| socle::mois::Mois::en_clair(m)).collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }
    Ok((dus, avertissements))
}

/// Les pages d'un mois de PuppetPaie, pour les salaries demandes.
///
/// `contrats_vus` : les numeros de contrat deja reedites par un mois precedent. Un contrat
/// plurimensuel est sur la paie de chacun de ses mois, mais c'est UN contrat : il ne sort qu'une
/// fois, a son premier mois.
#[allow(clippy::too_many_arguments)]
fn mois_puppetpaie(
    ctx: &Contexte,
    mois: &str,
    cles: &HashSet<String>,
    categories: &HashSet<&str>,
    tmp: &Path,
    d: &mut Dossier,
    aujourdhui: &Jour,
    contrats_vus: &mut HashSet<String>,
    attente: &mut HashMap<i32, i64>,
) -> R<Vec<Page>> {
    let mut pages = Vec::new();
    let sortie = ctx.racine.join("sorties").join(mois);
    let etiq = etiquette(mois)?;
    let envoi_dir = sortie.join("envoi");
    let (_, paies) = charger(ctx, mois)?;
    // Les documents du mois sont partis quand une paie porte le numero de son bulletin (ecrit a son depart).
    let numerote = |p: &Paie| p.numero_bulletin.as_deref().is_some_and(|n| !n.is_empty());
    let ecrit = paies.iter().any(numerote);
    let siennes: Vec<usize> = (0..paies.len()).filter(|i| cles.contains(&cle_de(&paies[*i]))).collect();
    if siennes.is_empty() {
        return Ok(pages);
    }
    let ent = ctx.pour_un_modele(&ctx.entreprise_du_mois(mois)?);
    let lib = socle::mois::Mois::en_clair(mois);

    // Les paies dont on tire les contrats d'un mois pas encore ecrit : celles de la saisie
    // dont chaque engagement porte son numero.
    let mut paies_contrats: Vec<Paie> = Vec::new();
    let mut siennes_contrats: Vec<usize> = Vec::new();
    if !ecrit {
        if categories.contains(BULLETINS) || categories.contains(CERTIFICATS_CS) {
            d.avertissements.push(format!(
                "{} : aucun document n'est encore parti, ni bulletin ni certificat à rééditer.",
                lib
            ));
        }
        // Un engagement valide porte son numero, ecrit a sa validation ; un engagement en saisie n'en a
        // pas, et un numero ne s'invente pas.
        let (data, mut figees) = charger(ctx, mois)?;
        let partis: Vec<usize> = (0..figees.len()).filter(|i| cles.contains(&cle_de(&figees[*i])) && figees[*i].engagement.numero.is_some()).collect();
        if partis.is_empty() {
            if categories.contains(CONTRATS) {
                d.refus.push(format!("{} : contrats non réédités — aucun n'est parti à la signature, et un numéro ne s'invente pas.", lib));
            }
        } else {
            numeroter(ctx, &data, &mut figees, attente)?;
            siennes_contrats = partis;
            paies_contrats = figees;
        }
    }
    let mut cles_des_siennes: Vec<String> = siennes.iter().map(|i| cle_de(&paies[*i])).collect();
    cles_des_siennes.sort();
    cles_des_siennes.dedup();

    // 1. Les bulletins, tels qu'ils sont partis (regle 4 : un document remis ne se regenere pas) : la piece que le
    // salarie a recue pour le mois (`depart::fabriquer_documents`, `envoi/Bulletin_<cle>_<Mois>.pdf`), qui porte tous ses
    // bulletins numerotes. Rien ne se recalcule ni ne se reimprime : un moteur qui a change de calcul depuis envoie quand
    // meme ce que le salarie a recu. Une piece qui manque refuse, sans repli.
    if categories.contains(BULLETINS) && ecrit {
        for cle in &cles_des_siennes {
            if !siennes.iter().any(|i| cle_de(&paies[*i]) == *cle && numerote(&paies[*i])) {
                continue;
            }
            let piece = envoi_dir.join(envois::envoi::nom_fichier("Bulletin", cle, &etiq));
            if !piece.is_file() {
                return Err(erreur_operation(format!(
                    "{}, {} : son bulletin n'est pas dans sorties/{}/envoi/. Un bulletin remis ne se réimprime pas : rien n'est envoyé à sa place.",
                    lib, cle, mois
                )));
            }
            for n in 0..pages_pdf(&piece)? {
                pages.push(Page { categorie: BULLETINS, cle: cle.clone(), tri: format!("{}-00", mois), source: piece.clone(), numero: n });
            }
        }
    }

    // 2. Les contrats : ceux du mois ecrit portent les numeros de leur engagement valide.
    if categories.contains(CONTRATS) {
        let (source, indices): (&Vec<Paie>, &Vec<usize>) = if ecrit { (&paies, &siennes) } else { (&paies_contrats, &siennes_contrats) };
        let mut cles_contrats: Vec<String> = indices.iter().map(|i| cle_de(&source[*i])).collect();
        cles_contrats.sort();
        cles_contrats.dedup();
        for cle in &cles_contrats {
            // Les contrats ecrits de ce salarie (`engagement::contrat_a_signer`), quelle que soit leur nature.
            let a_lui: Vec<Paie> = indices
                .iter()
                .map(|i| &source[*i])
                .filter(|p| {
                    cle_de(p) == *cle
                        && p.engagement.numero.is_some()
                        && engagement::contrat_a_signer(&ctx.racine, &p.engagement).unwrap_or(false)
                        && !contrats_vus.contains(p.engagement.numero.as_deref().unwrap_or(""))
                })
                .cloned()
                .collect();
            contrats_vus.extend(a_lui.iter().filter_map(|p| p.engagement.numero.clone().map(String::from)));
            let sans = indices
                .iter()
                .map(|i| &source[*i])
                .filter(|p| cle_de(p) == *cle && engagement::contrat_a_signer(&ctx.racine, &p.engagement).unwrap_or(false) && p.engagement.numero.is_none())
                .count();
            if sans > 0 {
                d.refus.push(format!("{}, {} : {} contrat(s) sans numéro lu, non réédité(s).", lib, cle, sans));
            }
            if a_lui.is_empty() {
                continue;
            }
            let chemin = tmp.join(format!("C_{}_{}.pdf", cle, mois));
            documents::contrat::ecrire_contrat(ctx, &a_lui, &ent, &chemin, &[])?;
            marquer(&chemin, aujourdhui)?;
            for i in 0..pages_pdf(&chemin)? {
                pages.push(Page { categorie: CONTRATS, cle: cle.clone(), tri: format!("{}-01", mois), source: chemin.clone(), numero: i });
            }
        }
    }

    // 3. Les certificats Conges Spectacles partis, a leurs N CE, ecrits dans la paie a leur depart.
    if categories.contains(CERTIFICATS_CS) && ecrit {
        let (pour_cs, avert) = certificats_dus(ctx, &paies)?;
        d.avertissements.extend(avert.into_iter().map(|a| format!("{} : {}", lib, a)));
        let ecrits = engagement::etats::numeros_des_documents(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(Echec::from)?;
        let trouve = |cle: &str, debut: &str| pour_cs.iter().find(|(c, de, _)| c == cle && de == debut).and_then(|(_, _, v)| v.as_ref());
        // Chaque certificat parti, et tous les N CE de l'archive du mois.
        let mut tous: Vec<i64> = Vec::new();
        let mut paires: Vec<(String, Vec<(&PaieCs, i64)>)> = Vec::new();
        for (i, p) in paies.iter().enumerate() {
            let cle = cle_de(p);
            let (Some(pc), Some(n)) = (trouve(&cle, &p.engagement.debut.iso()), ecrits.get(i).and_then(|e| e.premier_ce)) else { continue };
            let combien = conges_spectacles::decouper(&pc.engagement)?.len() as i64;
            tous.extend(n..n + combien);
            if cles.contains(&cle) {
                match paires.iter_mut().find(|(c, _)| *c == cle) {
                    Some((_, v)) => v.push((pc, n)),
                    None => paires.push((cle.clone(), vec![(pc, n)])),
                }
            }
        }
        tous.sort_unstable();
        paires.sort_by(|a, b| a.0.cmp(&b.0));
        // Les certificats remis sont dans `CertificatsCS_<Mois>.pdf`, un par page, dans l'ordre de leurs N CE
        // (`depart::fabriquer_documents`) ; la page porte le nom du salarie, sinon le fichier ne dit pas ce que les paies
        // disent, et rien ne se joint. Comme les bulletins : la page meme, jamais un certificat reimprime (regle 4).
        let archive = sortie.join(format!("CertificatsCS_{}.pdf", etiq));
        let nom_archive = archive.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let textes = if archive.exists() { Some(texte_pdf(&archive)?) } else { None };
        match textes {
            None if !paires.is_empty() => d.refus.push(format!(
                "{} : certificats Congés Spectacles non joints — {} n'est pas dans sorties/{}/, et un certificat remis ne se réimprime pas.",
                lib, nom_archive, mois
            )),
            Some(t) if !paires.is_empty() && t.len() != tous.len() => d.refus.push(format!(
                "{} : certificats Congés Spectacles non joints — {} a {} page(s), les paies en attendent {}. Le fichier ne dit pas ce que les paies disent : rien n'est joint à sa place.",
                lib,
                nom_archive,
                t.len(),
                tous.len()
            )),
            Some(t) => {
                for (cle, siens) in &paires {
                    let nom = siens
                        .first()
                        .and_then(|(pc, _)| pc.salarie.nom.ecrite("nom").ok())
                        .map(str::to_string)
                        .ok_or_else(|| avec("KeyError", socle::fichiers::absent("nom")))?;
                    let mut ses_pages: Vec<usize> = Vec::new();
                    let mut etrangeres: Vec<i64> = Vec::new();
                    for (pc, n) in siens {
                        for k in 0..conges_spectacles::decouper(&pc.engagement)?.len() as i64 {
                            let page = tous.iter().position(|x| *x == n + k).unwrap_or(usize::MAX);
                            if t.get(page).is_some_and(|p| sans_accents(p).contains(&sans_accents(&nom))) {
                                ses_pages.push(page);
                            } else {
                                etrangeres.push(n + k);
                            }
                        }
                    }
                    if !etrangeres.is_empty() {
                        d.refus.push(format!(
                            "{}, {} : certificats Congés Spectacles non joints — dans {}, la page du N° CE {} ne porte pas son nom. Le fichier ne dit pas ce que les paies disent : rien n'est joint à sa place.",
                            lib,
                            cle,
                            nom_archive,
                            etrangeres.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
                        ));
                        continue;
                    }
                    for page in ses_pages {
                        pages.push(Page { categorie: CERTIFICATS_CS, cle: cle.clone(), tri: format!("{}-02", mois), source: archive.clone(), numero: page });
                    }
                }
            }
            None => {}
        }
    }

    // 4. Ce qui se prend tel quel : contrats signes, AER, documents de fin.
    for cle in &cles_des_siennes {
        if categories.contains(CONTRATS_SIGNES) {
            // Le contrat signe hors du logiciel (`engagement::signe_ailleurs`) : le PDF que la compagnie a depose dans son
            // dossier (`signature::contrat_signe_depose`), une fois sur la periode.
            for p in siennes.iter().map(|i| &paies[*i]).filter(|p| cle_de(p) == *cle) {
                if !engagement::signe_ailleurs(&ctx.racine, &p.engagement).unwrap_or(false) {
                    continue;
                }
                for f in crate::compagnie::signature::contrat_signe_depose(&ctx.racine, cle, &p.engagement.debut.iso()) {
                    if !d.deposes_vus.insert(f.clone()) {
                        continue;
                    }
                    for i in 0..pages_pdf(&f)? {
                        pages.push(Page { categorie: CONTRATS_SIGNES, cle: cle.clone(), tri: format!("{}-01", mois), source: f.clone(), numero: i });
                    }
                }
            }
        }
        if categories.contains(CONTRATS_SIGNES) && envoi_dir.is_dir() {
            for nom in contrats_signes_de(&envoi_dir, cle) {
                let f = envoi_dir.join(&nom);
                // Le contrat signe est pris tel quel ; ses numeros, eux, se CONFRONTENT a ceux du
                // mois ecrit.
                let portes: std::collections::BTreeSet<String> = numeros_de_contrat(&nom).into_iter().collect();
                let connus: std::collections::BTreeSet<String> = siennes
                    .iter()
                    .map(|i| &paies[*i])
                    .filter(|p| cle_de(p) == *cle)
                    .filter_map(|p| p.engagement.numero.clone().map(String::from))
                    .collect();
                let hors: Vec<&String> = portes.difference(&connus).collect();
                if ecrit && !hors.is_empty() {
                    d.avertissements.push(format!(
                        "{}, {} : le contrat signé {} porte {}, que le mois écrit ne connaît pas (bulletins et DSN : {}). Il est au dossier tel qu'il a été signé ; l'écart est à regarder.",
                        lib,
                        cle,
                        nom,
                        hors.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                        if connus.is_empty() { "aucun".to_string() } else { connus.iter().cloned().collect::<Vec<_>>().join(", ") }
                    ));
                }
                for i in 0..pages_pdf(&f)? {
                    pages.push(Page { categorie: CONTRATS_SIGNES, cle: cle.clone(), tri: format!("{}-01", mois), source: f.clone(), numero: i });
                }
            }
        }
        if categories.contains(ATTESTATIONS) {
            let f = envoi_dir.join(format!("AER_{}_{}.pdf", cle, etiq));
            if f.exists() {
                for i in 0..pages_pdf(&f)? {
                    pages.push(Page { categorie: ATTESTATIONS, cle: cle.clone(), tri: format!("{}-03", mois), source: f.clone(), numero: i });
                }
            }
        }
        if categories.contains(DOCUMENTS_FIN) {
            for debut in [format!("CertificatTravail_{}_", cle), format!("RecuSoldeToutCompte_{}_", cle)] {
                for nom in noms_commencant_finissant(&sortie, &debut, ".pdf") {
                    let f = sortie.join(&nom);
                    for i in 0..pages_pdf(&f)? {
                        pages.push(Page { categorie: DOCUMENTS_FIN, cle: cle.clone(), tri: format!("{}-04", mois), source: f.clone(), numero: i });
                    }
                }
            }
        }
    }
    Ok(pages)
}

/// Sans accent, en majuscules.
fn sans_accents(t: &str) -> String {
    socle::texte::sans_accent(t).to_uppercase()
}

/// `signatures` : les courriels de signature d'un salarie sur la periode, la preuve de ses contrats
/// signes. Ils vont dans le ZIP, pas dans un PDF.
fn signatures(ctx: &Contexte, cle: &str, p: &Periode) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for mois in engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().map(String::from) {
        if !p.contient(&mois) {
            continue;
        }
        let d = ctx.racine.join("sorties").join(&mois).join("envoi");
        if d.is_dir() {
            out.extend(noms_commencant_finissant(&d, &format!("Signature_{}_", cle), ".eml").into_iter().map(|n| d.join(n)));
        }
    }
    out
}

/// Les pages des bulletins de paie que l'import a repris (les paies versees des engagements importes, colonne `bulletin` : le fichier de
/// sa copie dans `historique/bulletins/` et la page) : rattachees par la cle du salarie, datees du debut de la paie,
/// dans la periode par le mois de la paie. Une copie qui manque se dit ; rien d'autre ne se devine.
fn pages_importees(ctx: &Contexte, cles: &HashSet<String>, p: &Periode, d: &mut Dossier) -> Vec<Page> {
    let Ok(lignes) = engagement::importe::paies_lues(&ctx.racine) else { return Vec::new() };
    let dossier = ctx.racine.join("historique").join("bulletins");
    let (mut pages, mut manquent): (Vec<Page>, Vec<String>) = (Vec::new(), Vec::new());
    for x in lignes {
        let Some(b) = &x.paie.bulletin else { continue };
        let (cle, mois, fichier) = (x.salarie.clone(), x.mois.clone(), b.fichier.clone());
        let page = b.page;
        if page < 1 {
            continue;
        }
        if !cles.contains(&cle) || mois.is_empty() || !p.contient(&mois) || fichier.is_empty() || fichier.contains(['/', '\\']) {
            continue;
        }
        let source = dossier.join(&fichier);
        if !source.is_file() {
            if !manquent.contains(&fichier) {
                manquent.push(fichier);
            }
            continue;
        }
        let debut = x.paie.debut.map(|j| j.iso()).unwrap_or_default();
        let tri = if debut.is_empty() { format!("{}-01", mois) } else { debut };
        pages.push(Page { categorie: BULLETINS, cle, tri, source, numero: page as usize - 1 });
    }
    if !manquent.is_empty() {
        d.avertissements.push(format!(
            "Bulletins repris à l'import dont la copie n'est plus dans historique/bulletins/, laissés hors du dossier : {}.",
            manquent.join(", ")
        ));
    }
    pages
}

// ------------------------------------------------------------------ l'assemblage

/// Un PDF des pages donnees, dans l'ordre, dedoublonne, le logo une fois. Le
/// dedoublonnage lit le TEXTE des pages (un appel par fichier).
fn assembler(pages: &[&Page]) -> R<Option<Vec<u8>>> {
    if pages.is_empty() {
        return Ok(None);
    }
    let mut textes: HashMap<&Path, Vec<String>> = HashMap::new();
    let mut vues: HashSet<String> = HashSet::new();
    let mut retenues: Vec<&Page> = Vec::new();
    for pg in pages {
        if !textes.contains_key(pg.source.as_path()) {
            textes.insert(pg.source.as_path(), texte_pdf(&pg.source)?);
        }
        let texte = textes[pg.source.as_path()].get(pg.numero).cloned().ok_or_else(|| avec("IndexError", socle::fichiers::trop_courte(&format!("la page {} de {}", pg.numero + 1, pg.source.display()))))?;
        // Une page sans texte (un scan) ne se compare pas : elle se garde.
        if !texte.trim().is_empty() && !vues.insert(texte) {
            continue;
        }
        retenues.push(pg);
    }
    if retenues.is_empty() {
        return Ok(None);
    }
    // Les pages qui se suivent dans un meme fichier ne font qu'une piece.
    let mut pieces: Vec<(PathBuf, Option<Vec<usize>>)> = Vec::new();
    for pg in retenues {
        match pieces.last_mut() {
            Some((source, Some(liste))) if *source == pg.source => liste.push(pg.numero),
            _ => pieces.push((pg.source.clone(), Some(vec![pg.numero]))),
        }
    }
    fusion_pieces(&pieces).map(Some)
}

// ------------------------------------------------------------------ l'ecriture

/// Un fichier du ZIP : son nom, son contenu, sa date locale (DOS), ses droits.
pub(crate) struct Entree {
    pub(crate) nom: String,
    pub(crate) octets: Vec<u8>,
    pub(crate) date: u16,
    pub(crate) heure: u16,
    pub(crate) attributs: u32,
}

/// La date et l'heure locales (Paris) au format DOS, a partir de secondes unix.
fn horodatage_dos(secondes: i64) -> (u16, u16) {
    let local = secondes + socle::paris::decalage(secondes);
    let jour = Jour::depuis_jours_unix(local.div_euclid(86_400)).unwrap_or(Jour { an: 1980, mois: 1, jour: 1 });
    let reste = local.rem_euclid(86_400);
    let (h, m, s) = (reste / 3600, (reste % 3600) / 60, reste % 60);
    if jour.an < 1980 {
        return ((1 << 5) | 1, 0);
    }
    ((((jour.an - 1980) as u16) << 9) | ((jour.mois as u16) << 5) | jour.jour as u16, ((h as u16) << 11) | ((m as u16) << 5) | (s / 2) as u16)
}

fn maintenant() -> i64 {
    socle::temps::secondes().floor() as i64
}

/// Deflate, CRC-32, cree sous Unix.
pub(crate) fn zip(entrees: &[Entree]) -> R<Vec<u8>> {
    use flate2::write::DeflateEncoder;
    use flate2::{Compression, Crc};
    use std::io::Write;

    let u16_ = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let u32_ = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    let mut z: Vec<u8> = Vec::new();
    let mut repertoire: Vec<u8> = Vec::new();
    for e in entrees {
        let mut crc = Crc::new();
        crc.update(&e.octets);
        let somme = crc.sum();
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&e.octets).map_err(|x| avec("OSError", x.to_string()))?;
        let comprime = enc.finish().map_err(|x| avec("OSError", x.to_string()))?;
        let nom = e.nom.as_bytes();
        let drapeau: u16 = if e.nom.is_ascii() { 0 } else { 0x800 };
        let debut = z.len() as u32;
        u32_(&mut z, 0x0403_4b50);
        u16_(&mut z, 20);
        u16_(&mut z, drapeau);
        u16_(&mut z, 8);
        u16_(&mut z, e.heure);
        u16_(&mut z, e.date);
        u32_(&mut z, somme);
        u32_(&mut z, comprime.len() as u32);
        u32_(&mut z, e.octets.len() as u32);
        u16_(&mut z, nom.len() as u16);
        u16_(&mut z, 0);
        z.extend_from_slice(nom);
        z.extend_from_slice(&comprime);
        u32_(&mut repertoire, 0x0201_4b50);
        u16_(&mut repertoire, 20 | (3 << 8));
        u16_(&mut repertoire, 20);
        u16_(&mut repertoire, drapeau);
        u16_(&mut repertoire, 8);
        u16_(&mut repertoire, e.heure);
        u16_(&mut repertoire, e.date);
        u32_(&mut repertoire, somme);
        u32_(&mut repertoire, comprime.len() as u32);
        u32_(&mut repertoire, e.octets.len() as u32);
        u16_(&mut repertoire, nom.len() as u16);
        u16_(&mut repertoire, 0);
        u16_(&mut repertoire, 0);
        u16_(&mut repertoire, 0);
        u16_(&mut repertoire, 0);
        u32_(&mut repertoire, e.attributs);
        u32_(&mut repertoire, debut);
        repertoire.extend_from_slice(nom);
    }
    let debut_repertoire = z.len() as u32;
    z.extend_from_slice(&repertoire);
    u32_(&mut z, 0x0605_4b50);
    u16_(&mut z, 0);
    u16_(&mut z, 0);
    u16_(&mut z, entrees.len() as u16);
    u16_(&mut z, entrees.len() as u16);
    u32_(&mut z, repertoire.len() as u32);
    u32_(&mut z, debut_repertoire);
    u16_(&mut z, 0);
    Ok(z)
}

/// `z.writestr(nom, octets)`.
pub(crate) fn entree_ecrite(nom: String, octets: Vec<u8>) -> Entree {
    let (date, heure) = horodatage_dos(maintenant());
    Entree { nom, octets, date, heure, attributs: 0o600 << 16 }
}

/// Le fichier, date de l'instant ou le zip s'ecrit (le temps du moteur, comme tout ce qu'il ecrit : deux rejeux ecrivent le
/// meme zip), avec ses droits.
pub(crate) fn entree_fichier(f: &Path, nom: String) -> R<Entree> {
    let octets = std::fs::read(f).map_err(|e| avec("OSError", socle::echec::du_disque(&f, &e)))?;
    let meta = std::fs::metadata(f).map_err(|e| avec("OSError", socle::echec::du_disque(&f, &e)))?;
    let (date, heure) = horodatage_dos(maintenant());
    Ok(Entree { nom, octets, date, heure, attributs: (mode(&meta) & 0xFFFF) << 16 })
}

/// `st_mode` d'un fichier du zip. Sous Windows, il se deduit de l'attribut lecture seule : `0o100444`, sinon
/// `0o100666`.
#[cfg(unix)]
fn mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}

#[cfg(not(unix))]
fn mode(meta: &std::fs::Metadata) -> u32 {
    if meta.permissions().readonly() { 0o100444 } else { 0o100666 }
}

fn ecrire_octets(chemin: &Path, octets: &[u8]) -> R<()> {
    std::fs::write(chemin, octets).map_err(|e| avec("OSError", socle::fichiers::ecriture_impossible(&e, chemin)))
}

/// `{cle: {categorie: octets}}`, dans l'ordre ou les cles et les categories sont apparues.
type Contenus = Vec<(String, Vec<(&'static str, Vec<u8>)>)>;

fn contenu_de<'a>(contenus: &'a Contenus, cle: &str) -> Option<&'a Vec<(&'static str, Vec<u8>)>> {
    contenus.iter().find(|(c, _)| c == cle).map(|(_, v)| v)
}

/// Ecrit le dossier d'un salarie (un fichier par categorie, ou un seul PDF) et rend son chemin.
fn ecrire_un(dossier: &Path, cle: &str, contenus: &[(&'static str, Vec<u8>)], preuves: &[PathBuf], cats: &[&'static str], p: &Periode, un_seul: bool) -> R<PathBuf> {
    let base = format!("Dossier_{}_{}", nom_salarie(cle), p.nom());
    if un_seul && !(cats == [CONTRATS_SIGNES] && !preuves.is_empty()) {
        let cat = cats[0];
        let chemin = dossier.join(format!("{}_{}.pdf", base, nom_de(cat)));
        let octets = contenus.iter().find(|(c, _)| *c == cat).map(|(_, o)| o.as_slice()).unwrap_or(&[]);
        ecrire_octets(&chemin, octets)?;
        return Ok(chemin);
    }
    let chemin = dossier.join(format!("{}.zip", base));
    let mut entrees = Vec::new();
    for (i, (cat, octets)) in CATEGORIES.iter().filter_map(|c| contenus.iter().find(|(x, _)| x == c)).enumerate() {
        entrees.push(entree_ecrite(format!("{}-{}.pdf", i + 1, nom_de(cat)), octets.clone()));
    }
    for f in preuves {
        entrees.push(entree_fichier(f, format!("preuves-de-signature/{}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?);
    }
    ecrire_octets(&chemin, &zip(&entrees)?)?;
    Ok(chemin)
}

/// Tous les salaries dans le meme fichier : un PDF par categorie qui les
/// enchaine (une seule categorie : ce PDF-la), sinon un ZIP.
fn ecrire_ensemble(dossier: &Path, contenus: &Contenus, preuves: &[(String, Vec<PathBuf>)], cats: &[&'static str], p: &Periode, un_seul: bool, tmp: &Path) -> R<PathBuf> {
    let base = format!("Dossiers_{}", p.nom());
    let mut cles: Vec<&String> = contenus.iter().map(|(c, _)| c).collect();
    cles.sort();
    let mut par_cat: Vec<(&'static str, Vec<Vec<u8>>)> = Vec::new();
    for cle in cles {
        for (cat, octets) in contenu_de(contenus, cle).into_iter().flatten() {
            match par_cat.iter_mut().find(|(c, _)| c == cat) {
                Some((_, v)) => v.push(octets.clone()),
                None => par_cat.push((cat, vec![octets.clone()])),
            }
        }
    }
    let mut reunis: Vec<(&'static str, Vec<u8>)> = Vec::new();
    for (cat, morceaux) in &par_cat {
        reunis.push((cat, fusion_octets(morceaux, tmp)?));
    }
    let tout_preuves: Vec<&PathBuf> = preuves.iter().flat_map(|(_, l)| l.iter()).collect();
    if un_seul && tout_preuves.is_empty() {
        let chemin = dossier.join(format!("{}_{}.pdf", base, nom_de(cats[0])));
        let octets = reunis.iter().find(|(c, _)| *c == cats[0]).map(|(_, o)| o.as_slice()).unwrap_or(&[]);
        ecrire_octets(&chemin, octets)?;
        return Ok(chemin);
    }
    let chemin = dossier.join(format!("{}.zip", base));
    let mut entrees = Vec::new();
    for (i, (cat, octets)) in CATEGORIES.iter().filter_map(|c| reunis.iter().find(|(x, _)| x == c)).enumerate() {
        entrees.push(entree_ecrite(format!("{}-{}.pdf", i + 1, nom_de(cat)), octets.clone()));
    }
    for f in tout_preuves {
        entrees.push(entree_fichier(f, format!("preuves-de-signature/{}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?);
    }
    ecrire_octets(&chemin, &zip(&entrees)?)?;
    Ok(chemin)
}

// ------------------------------------------------------------------ la route

/// Le jour reel de Paris : `date.today()`, pas `ctx.aujourdhui`.
pub(crate) fn aujourdhui_reel() -> Jour {
    let t = maintenant();
    Jour::depuis_jours_unix((t + socle::paris::decalage(t)).div_euclid(86_400)).unwrap_or_else(droit::contexte::jour_reel)
}

/// Le dossier construit (et ecrit, sauf `apercu`). L'envoi (`archives`, `dossier-envoyer`)
/// en a besoin tel quel : ses salaries, ses fichiers, ses pages.
pub(crate) fn construire(ctx: &Contexte, demande: &DemandeDeDossier) -> R<Dossier> {
    let annee = demande.annee.as_ref().map(AnneeDite::texte).filter(|a| !a.is_empty());
    let (ensemble, ecrire) = (demande.ensemble, !demande.apercu);
    let p = Periode::nouveau(demande.mois, annee, demande.toutes_les_annees)?;
    let aujourdhui = aujourdhui_reel();
    let inconnues: Vec<&str> = demande.categories.iter().map(String::as_str).filter(|c| !CATEGORIES.contains(c)).collect();
    if !inconnues.is_empty() {
        return Err(erreur_operation(format!("Catégorie inconnue : {}. Les catégories sont {}.", inconnues.join(", "), CATEGORIES.join(", "))));
    }
    let cats: Vec<&'static str> = CATEGORIES.iter().copied().filter(|c| demande.categories.is_empty() || demande.categories.iter().any(|x| x == c)).collect();
    let fiches: Vec<String> = ctx.salaries.keys().map(|k| k.to_string()).collect();
    let demandes: Option<&Vec<String>> = Some(&demande.salaries).filter(|l| !l.is_empty());
    let cles: Vec<String> = match demandes {
        Some(liste) => {
            let absents: Vec<&str> = liste.iter().map(String::as_str).filter(|s| !fiches.iter().any(|f| f == s)).collect();
            if !absents.is_empty() {
                return Err(erreur_operation(format!("Salarié inconnu : {} (les clés de config/salaries.json).", absents.join(", "))));
            }
            liste.clone()
        }
        None => {
            let mut tous: Vec<String> = fiches.iter().map(|k| (*k).clone()).collect();
            tous.sort();
            tous
        }
    };
    let mut d = Dossier { categories: cats.clone(), periode: p.libelle()?, ..Dossier::default() };

    let racine_tmp = Temporaire::nouveau_dans(&std::env::temp_dir(), "puppetpaie-dossier")?;
    let tmp = racine_tmp.0.as_path();
    let ensemble_cles: HashSet<String> = cles.iter().cloned().collect();
    let ensemble_cats: HashSet<&str> = cats.iter().copied().collect();
    let mut pages: Vec<Page> = Vec::new();
    let mut contrats_vus: HashSet<String> = HashSet::new();
    let mut attente: HashMap<i32, i64> = HashMap::new();
    let mois_saisis = engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().map(String::from).collect::<Vec<_>>();
    for m in &mois_saisis {
        if p.contient(m) {
            pages.extend(mois_puppetpaie(ctx, m, &ensemble_cles, &ensemble_cats, tmp, &mut d, &aujourdhui, &mut contrats_vus, &mut attente)?);
        }
    }
    // Les bulletins repris a l'import : la page meme, copiee dans `historique/bulletins/`. Un engagement importe n'a
    // aucun document d'ici : sa page est la seule, meme dans un mois que la saisie touche aussi.
    if ensemble_cats.contains(BULLETINS) {
        pages.extend(pages_importees(ctx, &ensemble_cles, &p, &mut d));
    }
    if cats.contains(&RECAPITULATIF) {
        let mut recaps = Recapitulatifs::nouveau(ctx, p.mois.clone(), p.annee.clone(), p.toutes, envois::journal::lu(&ctx.racine).map(|j| j.departs)?);
        for cle in &cles {
            if let Some(r) = recaps.pdf(cle, tmp)? {
                for i in 0..pages_pdf(&r)? {
                    pages.push(Page { categorie: RECAPITULATIF, cle: cle.clone(), tri: "9999".to_string(), source: r.clone(), numero: i });
                }
            }
        }
    }

    // Par salarie, par categorie, dans l'ordre de lecture.
    pages.sort_by(|a, b| (&a.cle, rang_de(a.categorie), &a.tri).cmp(&(&b.cle, rang_de(b.categorie), &b.tri)));
    let mut par: Vec<(String, Vec<(&'static str, Vec<&Page>)>)> = Vec::new();
    for pg in &pages {
        let i = match par.iter().position(|(c, _)| *c == pg.cle) {
            Some(i) => i,
            None => {
                par.push((pg.cle.clone(), Vec::new()));
                par.len() - 1
            }
        };
        let cats_du_salarie = &mut par[i].1;
        match cats_du_salarie.iter_mut().find(|(c, _)| *c == pg.categorie) {
            Some((_, v)) => v.push(pg),
            None => cats_du_salarie.push((pg.categorie, vec![pg])),
        }
    }
    let preuves: Vec<(String, Vec<PathBuf>)> = if cats.contains(&CONTRATS_SIGNES) { par.iter().map(|(c, _)| (c.clone(), signatures(ctx, c, &p))).collect() } else { Vec::new() };
    d.salaries = cles.iter().filter(|c| par.iter().any(|(x, _)| x == *c)).cloned().collect();
    if d.salaries.is_empty() {
        // `AucunDocument` : une reponse, pas un incident.
        return Err(avec("AucunDocument", format!(
            "Aucun document sur la période ({}) pour {}.",
            p.libelle()?,
            if demandes.is_some() { cles.join(", ") } else { "aucun salarié".to_string() }
        )));
    }

    let mut contenus: Contenus = Vec::new();
    for cle in &d.salaries {
        let Some((_, cats_du_salarie)) = par.iter().find(|(c, _)| c == cle) else { continue };
        for (cat, siennes) in cats_du_salarie {
            if let Some(octets) = assembler(siennes)? {
                let n = compter(&octets)?;
                match contenus.iter_mut().find(|(c, _)| c == cle) {
                    Some((_, v)) => match v.iter_mut().find(|(x, _)| x == cat) {
                        Some(e) => e.1 = octets,
                        None => v.push((*cat, octets)),
                    },
                    None => contenus.push((cle.clone(), vec![(*cat, octets)])),
                }
                *d.pages.entree(cle, Ordonnee::default).entree(cat, || n) = n;
            }
        }
    }
    if !ecrire {
        return Ok(d);
    }
    let dossier = ctx.racine.join("exports").join("dossiers");
    std::fs::create_dir_all(&dossier).map_err(|e| avec("OSError", socle::fichiers::ecriture_impossible(&e, &dossier)))?;
    let un_seul = cats.len() == 1;
    if ensemble {
        d.fichiers.push(ecrire_ensemble(&dossier, &contenus, &preuves, &cats, &p, un_seul, tmp)?);
    } else {
        for cle in d.salaries.clone() {
            let siens = contenu_de(&contenus, &cle).cloned().unwrap_or_default();
            let ses_preuves = preuves.iter().find(|(c, _)| *c == cle).map(|(_, l)| l.clone()).unwrap_or_default();
            d.fichiers.push(ecrire_un(&dossier, &cle, &siens, &ses_preuves, &cats, &p, un_seul)?);
        }
    }
    Ok(d)
}
