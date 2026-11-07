//! Les clauses d'une convention collective qui changent la fin d'un contrat ou un arret : preavis, indemnites de
//! licenciement et de retraite, maintien de salaire, prime d'anciennete, heures en plus, remise du contrat.
//!
//! Le catalogue est `clauses-conventionnelles-base.json`, lu par sa forme (`Table`) une fois tant que le fichier ne
//! change pas ; chaque clause y porte le texte qui la fonde (`source`, verifie par `moteur/tests/fichiers/sources.rs`).
//! Le module rend ce que dit la convention, ou ce qui manque pour le dire ; il ne choisit pas entre la convention et la
//! loi (l'appelant : `engagement::solde`).

use crate::emplois;
use crate::fiche::{ChampsSalarie, Lue};
use crate::permanents::PartDuContrat;
use crate::source::forme_notee as forme;
use serde::Deserialize;
use socle::date::Jour;
use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::forme::{en_ordre, Nombre};
use socle::num::{Decimal, Resultat};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

pub use socle::d28::P;

const FICHIER: &str = "clauses-conventionnelles-base.json";

// ------------------------------------------------------------ le catalogue
forme! {
    /// `clauses-conventionnelles-base.json`, entier : les clauses de chaque convention relevee, sous son IDCC. Une
    /// convention absente n'a pas ses clauses relevees : le moteur calcule la loi seule et le dit.
    pub struct Table {
        pub conventions: BTreeMap<String, Convention>,
    }
}

/// Une clause de rupture que la convention laisse a la loi (`Convention::loi_seule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Preavis,
    IndemniteLicenciement,
    MiseALaRetraite,
    DepartALaRetraite,
}

forme! {
    /// Les clauses relevees d'une convention. Une section absente n'est pas relevee ; une section que la convention
    /// laisse a la loi se dit dans `loi_seule`, avec la phrase qui le dit.
    pub struct Convention {
        /// Le titre de la convention dans KALI : `idcc_nomme` y cherche le nom qu'un bulletin imprime.
        pub libelle: String,
        pub loi_seule: Option<BTreeMap<Section, String>>,
        pub categories: Option<Categories>,
        pub preavis: Option<Preavis>,
        pub indemnite_licenciement: Option<Licenciement>,
        pub rupture_conventionnelle: Option<RuptureConventionnelle>,
        pub mise_a_la_retraite: Option<MiseALaRetraite>,
        pub depart_a_la_retraite: Option<DepartALaRetraite>,
        pub maintien_de_salaire: Option<MaintienDeSalaire>,
        pub prime_anciennete: Option<PrimeAnciennete>,
        pub heures_supplementaires: Option<HeuresSupplementaires>,
        pub heures_complementaires: Option<HeuresComplementaires>,
        pub remise_du_contrat: Option<RemiseDuContrat>,
    }
}

impl Convention {
    /// La phrase qui dit que la convention laisse cette clause a la loi, si elle la laisse.
    pub fn loi_seule(&self, s: Section) -> Option<&str> {
        self.loi_seule.as_ref().and_then(|l| l.get(&s)).map(String::as_str)
    }
}

forme! {
    /// Les categories de la convention, dans l'ordre du texte, chacune dite en mots (ce que le bulletin imprime : la
    /// premiere phrase).
    pub struct Categories {
        #[serde(deserialize_with = "en_ordre")]
        pub valeurs: Vec<(String, String)>,
        /// Vrai : la convention ne distingue que le cadre et le non-cadre, et la case cadre de la fiche le dit.
        pub cadre_ou_non_cadre: Option<bool>,
        /// La categorie de chaque niveau de la grille 3090 (`emplois-base.json`, `classification_3090.niveau`).
        pub par_niveau_3090: Option<BTreeMap<String, String>>,
    }
}

forme! {
    /// Le preavis : par categorie, ou le meme pour toutes ; des motifs qui n'en ouvrent pas ; un supplement selon l'age.
    pub struct Preavis {
        pub article: String,
        /// Les motifs de rupture (S21.G00.62.002) qui n'ouvrent pas ce preavis, chacun avec la phrase qui le dit.
        pub motifs_hors_preavis: Option<BTreeMap<String, String>>,
        pub toutes_categories: Option<Vec<TranchePreavis>>,
        pub par_categorie: Option<BTreeMap<String, PreavisDeLaCategorie>>,
        /// Ajoute a l'age du salarie, au licenciement (et a la demission si `supplement_age_demission`).
        pub supplement_age: Option<Vec<SupplementAge>>,
        pub supplement_age_demission: Option<bool>,
    }
}

forme! {
    /// Le preavis d'une categorie : celui du licenciement, et celui de la demission quand il differe (sans
    /// `demission`, le preavis est reciproque ; une liste vide : la convention n'en fixe pas).
    pub struct PreavisDeLaCategorie {
        pub licenciement: Vec<TranchePreavis>,
        pub demission: Option<Vec<TranchePreavis>>,
    }
}

/// Une tranche d'anciennete du preavis : des mois, ou des jours par semaine calendaire d'anciennete sous un plafond.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranchePreavis {
    pub anciennete_mois_min: Nombre,
    /// `null` : sans borne.
    pub anciennete_mois_max: Option<Nombre>,
    pub mois: Option<Nombre>,
    pub jours_par_semaine: Option<Nombre>,
    pub plafond_jours: Option<Nombre>,
}

/// Des mois de preavis de plus, de `age_min` inclus a `age_max` exclu.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupplementAge {
    pub age_min: Nombre,
    pub age_max: Nombre,
    pub mois: Nombre,
}

forme! {
    /// L'indemnite de licenciement de la convention commune, et les clauses d'annexe qui la remplacent pour certains.
    pub struct Licenciement {
        pub article: String,
        pub anciennete_minimale_mois: Nombre,
        pub motifs_sans_indemnite: Option<Vec<String>>,
        /// Le bareme, quand il ne depend pas du motif.
        pub taux: Option<Vec<Tranche>>,
        /// Le bareme du motif economique, et celui des autres.
        pub par_motif: Option<ParMotif>,
        pub plafond_mois: Option<Nombre>,
        /// Avant `mois` d'anciennete, une fraction a part par annee.
        pub taux_avant: Option<TauxAvant>,
        /// Une majoration a partir d'un age (la 3097).
        pub majoration_age: Option<MajorationAge>,
        /// Plusieurs ages, chacun son pourcentage et son plancher (la 2121), sous `plafond_majore_mois`.
        pub majorations_age: Option<Vec<MajorationParAge>>,
        pub plafond_majore_mois: Option<Nombre>,
        /// Les clauses de chaque annexe (« 1 », « 3 »).
        pub par_annexe: Option<BTreeMap<String, ClausesDAnnexe>>,
    }
}

/// Un bareme en mois de salaire : par tranches (`taux`, R1234-2), ou par bandes (`bandes` : la bande atteinte donne sa
/// fraction a toutes les annees). Sous `economique`, les motifs de rupture qu'il vise.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bareme {
    pub motifs: Option<Vec<String>>,
    pub taux: Option<Vec<Tranche>>,
    pub bandes: Option<Vec<Bande>>,
}

/// Une tranche d'annees et sa fraction de mois ; `annees_entieres` : le bareme ne compte que les annees revolues.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tranche {
    pub jusqu_a_annees: Option<Nombre>,
    pub fraction_de_mois: Nombre,
    pub annees_entieres: Option<bool>,
}

/// Une bande : a partir de `anciennete_annees_min`, la fraction de toutes les annees.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bande {
    pub anciennete_annees_min: Nombre,
    pub fraction_de_mois: Nombre,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParMotif {
    pub economique: Option<Bareme>,
    pub autre: Bareme,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxAvant {
    pub mois: Nombre,
    pub fraction_de_mois: Nombre,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MajorationAge {
    pub age_min: Nombre,
    pub anciennete_mois_min: Nombre,
    pub pourcentage: Nombre,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MajorationParAge {
    pub age_min: Nombre,
    pub age_max: Option<Nombre>,
    pub anciennete_mois_min: Nombre,
    pub pourcentage: Nombre,
    pub plancher_mois: Option<Nombre>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClausesDAnnexe {
    pub technicien: Option<ClauseDAnnexe>,
    pub cadre_de_direction: Option<ClauseDAnnexe>,
    pub artiste: Option<ClauseDAnnexe>,
}

forme! {
    /// La clause d'une annexe qui remplace l'indemnite commune pour une categorie : ses lectures du texte, dans l'ordre
    /// du fichier (deux qui divergent : l'appelant demande le montant retenu).
    pub struct ClauseDAnnexe {
        pub article: String,
        pub anciennete_minimale_mois: Nombre,
        pub plafond_mois: Option<Nombre>,
        /// Le salaire de reference s'entend hors primes.
        pub hors_primes: Option<bool>,
        /// Les motifs pour lesquels l'annexe renvoie a la loi.
        pub motifs_legaux_seulement: Option<Vec<String>>,
        #[serde(deserialize_with = "en_ordre")]
        pub lectures: Vec<(String, Bareme)>,
    }
}

forme! {
    /// La rupture conventionnelle : son plancher est l'indemnite conventionnelle de licenciement.
    pub struct RuptureConventionnelle {
        pub article: String,
    }
}

forme! {
    /// L'indemnite de mise a la retraite : par paliers, par le bareme d'un licenciement (`taux`), ou une fraction par
    /// annee et une autre au-dela de `au_dela_de_annees`.
    pub struct MiseALaRetraite {
        pub article: String,
        pub paliers: Option<Vec<Palier>>,
        pub prorata: Option<Prorata>,
        pub taux: Option<Vec<Tranche>>,
        pub plafond_mois: Option<Nombre>,
        pub par_annee: Option<Nombre>,
        pub au_dela_de_annees: Option<Nombre>,
        pub en_plus_par_annee_au_dela: Option<Nombre>,
        pub hors_primes: Option<bool>,
        /// Le delai d'information, en mois : rappele, pas paye.
        pub prevenance_mois: Option<i64>,
        /// La clause du cadre de direction, relevee et non calculee : l'appelant demande le montant.
        pub cadre_de_direction: Option<CadreDeDirection>,
    }
}

forme! {
    pub struct CadreDeDirection {
        pub article: String,
    }
}

forme! {
    /// L'indemnite de depart volontaire a la retraite : un palier, ou le bareme d'un licenciement (`taux`) passe une
    /// anciennete minimale.
    pub struct DepartALaRetraite {
        pub article: String,
        pub paliers: Option<Vec<Palier>>,
        pub prorata: Option<Prorata>,
        pub taux: Option<Vec<Tranche>>,
        pub anciennete_minimale_mois: Option<Nombre>,
        pub prevenance_mois: Option<i64>,
    }
}

/// A partir de `anciennete_annees`, `mois_de_salaire`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palier {
    pub anciennete_annees: i64,
    pub mois_de_salaire: Nombre,
}

/// Avant le premier palier, a partir de `anciennete_annees_min`, une fraction de mois par annee.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prorata {
    pub anciennete_annees_min: Nombre,
    pub fraction_de_mois: Nombre,
}

forme! {
    /// Le maintien de salaire d'un arret : la maladie, et l'accident du travail quand la convention le distingue.
    pub struct MaintienDeSalaire {
        pub maladie: Option<MaintienMaladie>,
        pub accident_du_travail: Option<MaintienAccident>,
    }
}

/// Sur quoi porte le pourcentage garanti.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseDuMaintien {
    Brut,
    Net,
}

/// A qui la convention donne la subrogation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subrogation {
    ToutSalarie,
}

forme! {
    /// Le maintien en maladie : une ligne par anciennete.
    pub struct MaintienMaladie {
        pub article: String,
        pub anciennete_mois_minimum: Nombre,
        pub carence_jours: i64,
        pub base: BaseDuMaintien,
        pub plafond_net: Option<bool>,
        pub subrogation: Option<Subrogation>,
        pub par_anciennete: Vec<LigneDeMaintien>,
    }
}

/// A partir de `anciennete_annees_min` : ses tranches, ou des jours a 90 % puis des jours a deux tiers.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LigneDeMaintien {
    pub anciennete_annees_min: Nombre,
    pub tranches: Option<Vec<TrancheDeMaintien>>,
    pub jours_90: Option<i64>,
    pub jours_66: Option<i64>,
}

forme! {
    /// Le maintien d'un accident du travail : les origines qu'il couvre (sans `couvre` : l'accident du travail et la
    /// maladie professionnelle), ses tranches, et celles d'une anciennete plus longue.
    pub struct MaintienAccident {
        pub article: String,
        pub couvre: Option<Vec<crate::maintien::Origine>>,
        pub anciennete_mois_minimum: Nombre,
        pub carence_jours: i64,
        pub base: BaseDuMaintien,
        pub plafond_net: Option<bool>,
        pub subrogation: Option<Subrogation>,
        /// Passe cette anciennete, l'accident suit la clause de la maladie.
        pub comme_maladie_apres_mois: Option<Nombre>,
        pub tranches: Vec<TrancheDeMaintien>,
        /// A deux ans d'anciennete.
        pub tranches_deux_ans: Option<Vec<TrancheDeMaintien>>,
        pub tranches_apres: Option<TranchesApres>,
    }
}

/// Une tranche de maintien : `jours`, `mois` de calendrier, ou tant que la caisse indemnise (`pendant_ij`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrancheDeMaintien {
    pub taux: Nombre,
    pub jours: Option<i64>,
    pub mois: Option<i64>,
    pub pendant_ij: Option<bool>,
}

/// Au-dela de `anciennete_mois` d'anciennete, ces tranches.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranchesApres {
    pub anciennete_mois: Nombre,
    pub tranches: Vec<TrancheDeMaintien>,
}

forme! {
    /// La prime d'anciennete des techniciens d'une annexe : un pourcentage par annee de presence passe `apres_annees`,
    /// sous un plafond, du salaire conventionnel de la grille en vigueur.
    pub struct PrimeAnciennete {
        pub article: String,
        pub annexe: String,
        pub apres_annees: i64,
        pub pourcentage_par_annee: i64,
        pub plafond_pourcentage: i64,
        /// Les familles de metiers payees a la grille du costume.
        pub familles_costume: Option<Vec<String>>,
        /// La ligne de la grille de chaque niveau 3090.
        pub niveaux: BTreeMap<String, String>,
        /// Les grilles, dans l'ordre de leurs dates.
        pub salaires_conventionnels: Vec<GrilleDeSalaires>,
    }
}

forme! {
    /// Une grille de salaires conventionnels, a partir de `depuis` : celle du costume, celle des techniques par jauge
    /// de la salle (dans l'ordre du texte).
    pub struct GrilleDeSalaires {
        pub depuis: Jour,
        pub costume: BTreeMap<String, Nombre>,
        #[serde(deserialize_with = "en_ordre")]
        pub technique: Vec<(String, BTreeMap<String, Nombre>)>,
    }
}

forme! {
    /// Les heures supplementaires : le contingent annuel, et les paliers annuels de majoration quand la convention les
    /// fixe (au-dela du dernier, `au_dela_du_contingent`).
    pub struct HeuresSupplementaires {
        pub contingent_annuel: Option<Nombre>,
        pub paliers_annuels: Option<Vec<PalierDHeures>>,
        pub au_dela_du_contingent: Option<Nombre>,
    }
}

/// `heures` de plus a `majoration` %.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PalierDHeures {
    pub heures: Nombre,
    pub majoration: Nombre,
}

forme! {
    /// Les heures complementaires : la part du contrat qu'elles peuvent atteindre.
    pub struct HeuresComplementaires {
        pub plafond: PartDuContrat,
    }
}

forme! {
    /// La remise du contrat a duree determinee, plus tot que la loi pour les metiers qu'elle nomme.
    pub struct RemiseDuContrat {
        pub article: String,
        pub jours_ouvrables_avant: i64,
        pub metiers: Vec<String>,
    }
}

/// Le catalogue livre. Une clause que la convention laisse a la loi n'est pas aussi relevee.
pub fn table() -> Resultat<Arc<Table>> {
    static CACHE: OnceLock<FormeGardee<Table>> = OnceLock::new();
    forme_en_cache(&CACHE, FICHIER, "Les clauses conventionnelles livrées ne se lisent pas : le signaler au développeur.", |t: Table| {
        for (idcc, c) in &t.conventions {
            let releves = [
                (Section::Preavis, c.preavis.is_some()),
                (Section::IndemniteLicenciement, c.indemnite_licenciement.is_some()),
                (Section::MiseALaRetraite, c.mise_a_la_retraite.is_some()),
                (Section::DepartALaRetraite, c.depart_a_la_retraite.is_some()),
            ];
            if let Some((s, _)) = releves.iter().find(|(s, releve)| *releve && c.loi_seule.as_ref().is_some_and(|l| l.contains_key(s))) {
                return Err(socle::echec::livree(FICHIER, &format!("conventions.{}.loi_seule.{:?}", idcc, s)));
            }
        }
        Ok(t)
    })
}

impl Table {
    /// Les clauses de la convention `idcc`, si elles sont relevees.
    pub fn convention(&self, idcc: &str) -> Option<&Convention> {
        self.conventions.get(idcc.trim())
    }
}

/// Les clauses relevees de cette convention, ou `None` si elles ne le sont pas (ou si la compagnie n'a pas d'IDCC).
pub fn releve(idcc: &str) -> Resultat<Option<Convention>> {
    Ok(table()?.convention(idcc).cloned())
}

// ------------------------------------------------------------ les aides
fn p(n: &Nombre) -> P {
    P::depuis(n.0)
}

fn manque(cle: &str) -> String {
    socle::echec::livree(FICHIER, cle)
}

/// Pour les phrases : « la convention 3090 », ou « la convention collective » quand l'IDCC manque.
pub fn nom(idcc: &str) -> String {
    let i = idcc.trim().to_string();
    if i.is_empty() {
        "la convention collective".to_string()
    } else {
        format!("la convention {}", i)
    }
}

/// Les mots d'un nom de convention qui la distinguent : sans accent ni ponctuation, sans les mots que tout titre de
/// convention porte (« convention collective nationale »), les mots de liaison, les dates.
fn mots_du_nom(nom: &str) -> std::collections::BTreeSet<String> {
    const COMMUNS: &[&str] = &[
        "convention", "collective", "nationale", "ccn", "pour", "des", "les", "aux", "dans", "janvier", "fevrier", "mars",
        "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre",
    ];
    socle::texte::cle_recherche(nom)
        .split(' ')
        .filter(|m| m.chars().count() > 2 && m.chars().all(char::is_alphabetic) && !COMMUNS.contains(m))
        .map(str::to_string)
        .collect()
}

/// L'IDCC d'une convention que le bulletin nomme sans son numéro : celle, seule, dont le libellé de la table (le
/// titre de la convention dans KALI) porte tous les mots distinctifs du nom imprimé. Un nom que plusieurs conventions
/// portent (« spectacle », « production »), ou qu'aucune ne porte entier, ne donne rien : rien ne se devine (règle 1).
pub fn idcc_nomme(nom: &str) -> Resultat<Option<String>> {
    let lus = mots_du_nom(nom);
    if lus.is_empty() {
        return Ok(None);
    }
    let t = table()?;
    let mut trouves = t.conventions.iter().filter(|(_, c)| lus.is_subset(&mots_du_nom(&c.libelle)));
    Ok(match (trouves.next(), trouves.next()) {
        (Some((idcc, _)), None) => Some(idcc.clone()),
        _ => None,
    })
}

/// Une categorie de la convention dite en mots (« agent de maîtrise », « non-cadre »), telle que la fiche la nomme.
pub fn categorie_en_mots(c: &str) -> String {
    match c {
        "employe" => "employé".into(),
        "agent_de_maitrise" => "agent de maîtrise".into(),
        "non_cadre" => "non-cadre".into(),
        autre => autre.replace('_', " "),
    }
}

/// La categorie `c` dite par la convention `idcc`, telle que le bulletin l'imprime (la premiere phrase de son libelle),
/// ou `None` si la convention ne la nomme pas.
pub fn categorie_imprimee(idcc: &str, c: &str) -> Resultat<Option<String>> {
    let t = table()?;
    let dite = t.convention(idcc).and_then(|conv| conv.categories.as_ref()).and_then(|s| s.valeurs.iter().find(|(k, _)| k == c));
    Ok(dite.map(|(_, l)| l.split(". ").next().unwrap_or(l).trim_end_matches('.').to_string()))
}

fn non_relevees(idcc: &str) -> String {
    format!("les clauses de {} ne sont pas relevées", nom(idcc))
}

// ------------------------------------------------------------ la categorie
/// CE QUE L'ENGAGEMENT DIT DU SALARIE la ou la fiche le dit aussi : il l'emporte sur elle. Chaque champ seulement
/// quand l'engagement le dit ; `Default` : il ne dit rien.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DitParLEngagement {
    pub categorie_conventionnelle: Option<String>,
    pub statut: Option<String>,
    /// Le code emploi (`metier`).
    pub metier: Option<String>,
    pub cadre: Option<bool>,
    pub debut: Option<Jour>,
}

/// La categorie conventionnelle d'un salarie pour CET engagement : `categorie` vaut `None` quand rien ne permet de la
/// lire sans deviner, et `source` dit alors pourquoi ; `grille` est celle du niveau 3090 du metier.
#[derive(Debug, Clone, PartialEq)]
pub struct Profil {
    pub categorie: Option<String>,
    pub grille: Option<String>,
    pub source: String,
}

pub fn profil(idcc: &str, fiche: &ChampsSalarie, e: &DitParLEngagement) -> Resultat<Profil> {
    let t = table()?;
    let sec = t.convention(idcc).and_then(|c| c.categories.as_ref());
    let code = e.metier.clone().or_else(|| fiche.code_emploi_ft.remplie().map(str::to_string));
    let m = code.as_deref().map(emplois::metier).transpose()?.flatten();
    let c3090 = m.as_ref().and_then(|m| m.classification_3090());
    let grille = c3090.map(|c| c.grille.clone());
    let profil = |categorie: Option<&str>, source: String| Profil { categorie: categorie.map(str::to_string), grille: grille.clone(), source };

    let dit = e.categorie_conventionnelle.clone().or_else(|| Some(fiche.categorie_conventionnelle.code().to_string()).filter(|c| !c.is_empty()));
    if let Some(dit) = dit {
        let valeurs = sec.map_or(&[][..], |s| &s.valeurs[..]);
        if !valeurs.is_empty() && !valeurs.iter().any(|(k, _)| *k == dit) {
            return Ok(profil(None, format!(
                "la catégorie dans la convention de la fiche ({}) est inconnue de {} (catégories : {})",
                repr(&dit),
                nom(idcc),
                valeurs.iter().map(|(c, _)| categorie_en_mots(c)).collect::<Vec<_>>().join(", ")
            )));
        }
        return Ok(profil(Some(&dit), "la fiche du salarié (« Catégorie dans la convention »)".into()));
    }

    let statut = match (&e.statut, &e.metier, &m) {
        (Some(s), _, _) => Some(s.as_str()),
        (None, Some(_), Some(m)) => Some(m.statut().as_str()),
        _ => Some(fiche.statut.code()),
    };
    if statut == Some("artiste") {
        return Ok(profil(Some("artiste"), "le statut artiste".into()));
    }
    let est_cadre = e.cadre.unwrap_or_else(|| fiche.statut_categoriel_rc.code() == "01");
    let niveau = c3090.map(|c| c.niveau.clone());
    let selon_niveau = sec.and_then(|s| s.par_niveau_3090.as_ref().zip(niveau.as_ref()).and_then(|(m, n)| m.get(n))).filter(|c| !c.is_empty());
    if est_cadre {
        return Ok(profil(Some("cadre"), "la case cadre".into()));
    }
    if sec.is_some_and(|s| s.cadre_ou_non_cadre == Some(true)) {
        return Ok(profil(Some("non_cadre"), "la case cadre, non cochée".into()));
    }
    if let (Some(c), Some(code), Some(niveau)) = (selon_niveau, code.as_deref(), niveau.as_deref()) {
        if c == "cadre" {
            return Ok(profil(None, format!(
                "le métier {} est de niveau {} dans la grille 3090, et la fiche le \
                    déclare non cadre : agent de maîtrise ou employé, le logiciel ne le devine pas",
                code, niveau
            )));
        }
        return Ok(profil(Some(c), format!("le niveau 3090 du métier {} ({})", code, niveau)));
    }
    let code_dit = code.unwrap_or_else(|| "aucun code métier".to_string());
    Ok(profil(None, format!("ni la fiche, ni le métier ({}) ne disent la catégorie", code_dit)))
}

// --------------------------------------------------------------- le preavis
/// Le preavis de la convention. `mois` vaut `None` quand la convention ne dit rien -- pas relevee, ou pas de
/// categorie -- et `pourquoi` le dit ; 0 quand le motif n'ouvre pas de preavis ; `jours` pour un preavis dit en jours.
#[derive(Debug, Clone, PartialEq)]
pub struct PreavisDit {
    pub mois: Option<P>,
    pub article: Option<String>,
    pub jours: Option<P>,
    pub pourquoi: Option<String>,
}

pub fn preavis_mois(idcc: &str, categorie: Option<&str>, anciennete: Decimal, motif: Option<&str>, age: Option<Decimal>) -> Resultat<PreavisDit> {
    let (a, age) = (P::depuis(anciennete), age.map(P::depuis));
    let rien = |article: Option<String>, pourquoi: String| PreavisDit { mois: None, article, jours: None, pourquoi: Some(pourquoi) };
    let t = table()?;
    let conv = t.convention(idcc);
    if let Some(loi) = conv.and_then(|c| c.loi_seule(Section::Preavis)) {
        return Ok(rien(None, loi.to_string()));
    }
    let Some(sec) = conv.and_then(|c| c.preavis.as_ref()) else {
        return Ok(rien(None, non_relevees(idcc)));
    };
    let article = format!("{}, art. {}", nom(idcc), sec.article);
    if let Some(hors) = motif.and_then(|m| sec.motifs_hors_preavis.as_ref().and_then(|h| h.get(m))).filter(|h| !h.is_empty()) {
        return Ok(PreavisDit { mois: Some(P::ZERO), article: Some(article), jours: None, pourquoi: Some(hors.clone()) });
    }
    let categorie = categorie.filter(|c| !c.is_empty());
    let demission = motif == Some("059");
    let tranches: &[TranchePreavis] = match sec.toutes_categories.as_deref() {
        Some(t) if !t.is_empty() => t,
        _ => {
            let Some(c) = categorie else {
                return Ok(rien(Some(article), "la catégorie du salarié dans la convention n'est pas connue".into()));
            };
            match sec.par_categorie.as_ref().and_then(|m| m.get(c)) {
                Some(pc) if demission => match pc.demission.as_deref() {
                    Some(d) => d,
                    None => &pc.licenciement,
                },
                Some(pc) => &pc.licenciement,
                None => &[],
            }
        }
    };
    if tranches.is_empty() {
        return Ok(rien(Some(article), format!("{} ne fixe pas de préavis pour la catégorie {}", nom(idcc), categorie_en_mots(categorie.unwrap_or("")))));
    }
    for t in tranches {
        if a >= p(&t.anciennete_mois_min) && t.anciennete_mois_max.as_ref().is_none_or(|h| a < p(h)) {
            if let Some(par_semaine) = &t.jours_par_semaine {
                // « 1 jour par semaine calendaire, dans la limite de 15 jours » : des jours, rendus en mois de 30 jours.
                let semaines = a.mul(P::entier(52)).div(P::entier(12)).plancher();
                let plafond = p(t.plafond_jours.as_ref().ok_or_else(|| manque("plafond_jours"))?);
                let produit = semaines.mul(p(par_semaine));
                let jours = if produit < plafond { produit } else { plafond };
                let mois = jours.div(P::entier(30)).quantifier(4);
                return Ok(PreavisDit { mois: Some(mois), article: Some(article), jours: Some(jours), pourquoi: None });
            }
            let mut mois = p(t.mois.as_ref().ok_or_else(|| manque("mois"))?);
            if let Some(supplements) = sec.supplement_age.as_ref().filter(|s| !s.is_empty() && (!demission || sec.supplement_age_demission == Some(true))) {
                let Some(age) = age else {
                    let pourquoi = format!("le préavis de {} dépend de l'âge du salarié, et sa \
                            date de naissance manque", article);
                    return Ok(rien(Some(article), pourquoi));
                };
                for sup in supplements {
                    if p(&sup.age_min) <= age && age < p(&sup.age_max) {
                        mois = mois.add(p(&sup.mois));
                    }
                }
            }
            return Ok(PreavisDit { mois: Some(mois), article: Some(article), jours: None, pourquoi: None });
        }
    }
    let pourquoi = format!("aucune tranche de {} ne couvre {} mois d'ancienneté", article, a);
    Ok(rien(Some(article), pourquoi))
}

// ------------------------------------------------------------- les baremes
/// En mois de salaire : chaque tranche d'annees a sa fraction (R1234-2).
pub fn par_tranches(taux: &[Tranche], annees: P, entieres: bool) -> P {
    let annees = if entieres { annees.plancher() } else { annees };
    let (mut total, mut bas) = (P::ZERO, P::ZERO);
    for t in taux {
        let haut = t.jusqu_a_annees.as_ref().map_or(annees, p);
        let part = annees.min(haut).sub(bas);
        let part = if part > P::ZERO { part } else { P::ZERO };
        total = total.add(part.mul(p(&t.fraction_de_mois)));
        bas = haut;
        if annees <= haut {
            break;
        }
    }
    total
}

/// Par tranches, en annees revolues quand une tranche le dit.
fn tranches(taux: &[Tranche], annees: P) -> P {
    par_tranches(taux, annees, taux.iter().any(|t| t.annees_entieres == Some(true)))
}

/// En mois de salaire : la bande atteinte donne sa fraction a TOUTES les annees.
pub fn par_bandes(bandes: &[Bande], annees: P) -> P {
    let mut triees: Vec<&Bande> = bandes.iter().collect();
    triees.sort_by_key(|b| p(&b.anciennete_annees_min));
    let mut fraction = P::ZERO;
    for b in triees {
        if annees >= p(&b.anciennete_annees_min) {
            fraction = p(&b.fraction_de_mois);
        }
    }
    fraction.mul(annees)
}

impl Bareme {
    /// Les mois de salaire de `annees` d'anciennete : par bandes, sinon par tranches.
    pub fn mois(&self, annees: P) -> Resultat<P> {
        match (&self.bandes, &self.taux) {
            (Some(b), _) => Ok(par_bandes(b, annees)),
            (None, Some(t)) => Ok(tranches(t, annees)),
            (None, None) => Err(manque("taux")),
        }
    }
}

// ----------------------------------------------- l'indemnite de licenciement
/// La clause d'annexe qui s'applique a ce salarie, ou `None`. Le personnel technique de l'annexe I se lit sur la
/// GRILLE du metier.
pub fn clause_d_annexe(idcc: &str, annexe: &str, prof: &Profil, motif: Option<&str>) -> Resultat<Option<ClauseDAnnexe>> {
    let t = table()?;
    let par = t.convention(idcc).and_then(|c| c.indemnite_licenciement.as_ref()).and_then(|s| s.par_annexe.as_ref().and_then(|m| m.get(annexe.trim())));
    let Some(par) = par else { return Ok(None) };
    let cat = prof.categorie.as_deref();
    let choix = match (&par.cadre_de_direction, &par.technicien, &par.artiste) {
        (Some(c), _, _) if cat == Some("cadre_de_direction") => Some(c),
        (_, Some(c), _) if prof.grille.as_deref() == Some("technique") && cat != Some("artiste") && cat != Some("cadre_de_direction") => Some(c),
        (_, _, Some(c)) if cat == Some("artiste") => Some(c),
        _ => None,
    };
    Ok(choix.filter(|c| !motif.is_some_and(|mo| c.motifs_legaux_seulement.iter().flatten().any(|m| m == mo))).cloned())
}

/// L'indemnite CONVENTIONNELLE de licenciement. `bas` et `haut` sont egaux quand le texte n'a qu'une lecture ; quand
/// ils different, le montant n'est pas tranche et l'appelant demande. `releve` faux : la convention n'est pas relevee
/// (ou laisse l'indemnite a la loi), et `pourquoi` le dit.
#[derive(Debug, Clone, PartialEq)]
pub struct Indemnite {
    pub releve: bool,
    pub du: bool,
    pub bas: Option<P>,
    pub haut: Option<P>,
    /// Chaque lecture du texte et son montant, dans l'ordre du fichier.
    pub lectures: Vec<(String, P)>,
    pub hors_primes: bool,
    pub article: Option<String>,
    pub pourquoi: Option<String>,
}

pub fn indemnite_licenciement(
    idcc: &str,
    annexe: &str,
    prof: &Profil,
    anciennete: Decimal,
    reference: Decimal,
    motif: Option<&str>,
    age: Option<Decimal>,
) -> Resultat<Indemnite> {
    let (a, ref_, age) = (P::depuis(anciennete), P::depuis(reference), age.map(P::depuis));
    let pas_releve = |pourquoi: String| Indemnite { releve: false, du: false, bas: None, haut: None, lectures: Vec::new(), hors_primes: false, article: None, pourquoi: Some(pourquoi) };
    let t = table()?;
    let conv = t.convention(idcc);
    if let Some(loi) = conv.and_then(|c| c.loi_seule(Section::IndemniteLicenciement)) {
        return Ok(pas_releve(loi.to_string()));
    }
    let Some(sec) = conv.and_then(|c| c.indemnite_licenciement.as_ref()) else {
        return Ok(pas_releve(non_relevees(idcc)));
    };
    let annees = a.div(P::entier(12));
    let motif_t = motif.unwrap_or("");
    let dans = |l: &[String]| l.iter().any(|x| x == motif_t);
    let mut lectures: Vec<(String, P)> = Vec::new();
    let mut article = format!("{}, art. {}", nom(idcc), sec.article);
    let mut hors_primes = false;
    let mut pourquoi = None;
    let sortie = |lectures: Vec<(String, P)>, article: String, hors_primes: bool, pourquoi: Option<String>, du: bool| {
        let (bas, haut) = if du { (lectures.iter().map(|x| x.1).min(), lectures.iter().map(|x| x.1).max()) } else { (None, None) };
        Indemnite { releve: true, du, bas, haut, lectures, hors_primes, article: Some(article), pourquoi }
    };
    if sec.motifs_sans_indemnite.as_deref().is_some_and(dans) {
        let p = format!("le motif {} n'ouvre pas d'indemnite de licenciement", motif_t);
        return Ok(sortie(lectures, article, hors_primes, Some(p), false));
    }

    // La convention commune d'abord ; ou, pour une convention qui distingue le motif, le bareme du motif.
    if a >= p(&sec.anciennete_minimale_mois) {
        let mut mois = match (&sec.par_motif, &sec.taux) {
            (Some(pm), _) => {
                let choix = pm.economique.as_ref().filter(|e| e.motifs.as_deref().is_some_and(dans)).unwrap_or(&pm.autre);
                choix.mois(annees)?
            }
            (None, Some(taux)) => tranches(taux, annees),
            (None, None) => return Err(manque("indemnite_licenciement.taux")),
        };
        if let Some(plafond) = &sec.plafond_mois {
            mois = mois.min(p(plafond));
        }
        let mut commun = mois.mul(ref_).eur2();
        if let Some(avant) = sec.taux_avant.as_ref().filter(|avant| a < p(&avant.mois)) {
            // Avant le bareme, une fraction a part.
            commun = annees.mul(p(&avant.fraction_de_mois)).mul(ref_).eur2();
        }
        if let Some(majorations) = sec.majorations_age.as_ref().filter(|m| !m.is_empty()) {
            // Plusieurs ages, chacun son pourcentage et son plancher, puis le plafond. L'age en annees revolues ; sans
            // lui, les deux lectures.
            let mut vises: Vec<&MajorationParAge> = majorations.iter().filter(|m| a >= p(&m.anciennete_mois_min)).collect();
            match age {
                None if !vises.is_empty() => {
                    set(&mut lectures, "sans_majoration_age", commun);
                    pourquoi = Some(format!("l'indemnite de {} depend de l'age du salarie, et sa date de naissance manque", article));
                    vises = vises.split_off(vises.len() - 1);
                }
                Some(age) => vises.retain(|m| p(&m.age_min) <= age && m.age_max.as_ref().is_none_or(|h| age < p(h))),
                None => {}
            }
            let mut meilleur = commun;
            for m in vises {
                let f = P::entier(1).add(p(&m.pourcentage).div(P::entier(100)));
                let mut v = mois.mul(ref_).mul(f).eur2();
                if let Some(plancher) = &m.plancher_mois {
                    v = v.max(p(plancher).mul(ref_).eur2());
                }
                meilleur = meilleur.max(v);
            }
            if let Some(plafond) = &sec.plafond_majore_mois {
                meilleur = meilleur.min(p(plafond).mul(ref_).eur2());
            }
            commun = meilleur;
        } else if let Some(maj) = sec.majoration_age.as_ref().filter(|m| a >= p(&m.anciennete_mois_min)) {
            // Une majoration a partir d'un age. Sans l'age, les deux lectures.
            let majore = commun.mul(P::entier(1).add(p(&maj.pourcentage).div(P::entier(100)))).eur2();
            match age {
                None => {
                    set(&mut lectures, "sans_majoration_age", commun);
                    commun = majore;
                    pourquoi = Some(format!(
                        "la majoration de {} % depend de l'age du salarie, et sa date de \
                         naissance manque",
                        maj.pourcentage.0
                    ));
                }
                Some(age) if age >= p(&maj.age_min) => commun = majore,
                Some(_) => {}
            }
        }
        set(&mut lectures, "convention", commun);
    }
    // Puis la clause de l'annexe, si elle vise ce salarie : elle REMPLACE la clause commune pour lui, et la loi reste
    // le plancher (l'appelant).
    if let Some(c) = clause_d_annexe(idcc, annexe, prof, motif)? {
        article = format!("{}, {}", nom(idcc), c.article);
        hors_primes = c.hors_primes == Some(true);
        let minimale = p(&c.anciennete_minimale_mois);
        if a < minimale {
            let p = format!("{} demande {} mois d'anciennete", article, c.anciennete_minimale_mois.0);
            return Ok(sortie(Vec::new(), article, hors_primes, Some(p), false));
        }
        lectures = Vec::new();
        for (n, bareme) in &c.lectures {
            let mut mois = bareme.mois(annees)?;
            if let Some(plafond) = &c.plafond_mois {
                mois = mois.min(p(plafond));
            }
            set(&mut lectures, n, mois.mul(ref_).eur2());
        }
    }
    if lectures.is_empty() {
        let p = format!("{} mois d'anciennete, et {} en demande {}", a.plancher(), article, sec.anciennete_minimale_mois.0);
        return Ok(sortie(lectures, article, hors_primes, Some(p), false));
    }
    Ok(sortie(lectures, article, hors_primes, pourquoi, true))
}

/// Pose `k` a `v` dans une liste ordonnee de paires : une cle existante garde sa place.
fn set(l: &mut Vec<(String, P)>, k: &str, v: P) {
    match l.iter_mut().find(|x| x.0 == k) {
        Some(x) => x.1 = v,
        None => l.push((k.to_string(), v)),
    }
}

// ------------------------------------------------------------ la retraite
/// `(mois de salaire, palier atteint)` d'un bareme par paliers, avec la part au prorata qui le precede quand la
/// convention en a une.
pub fn paliers(paliers: &[Palier], prorata: Option<&Prorata>, annees: P) -> (P, Option<i64>) {
    let mut triees: Vec<&Palier> = paliers.iter().collect();
    triees.sort_by_key(|p| p.anciennete_annees);
    let atteint = triees.into_iter().filter(|x| annees >= P::entier(x.anciennete_annees)).last();
    if let Some(x) = atteint {
        return (p(&x.mois_de_salaire), Some(x.anciennete_annees));
    }
    match prorata {
        Some(pr) if annees >= p(&pr.anciennete_annees_min) => (annees.mul(p(&pr.fraction_de_mois)), None),
        _ => (P::ZERO, None),
    }
}

/// Une indemnite conventionnelle de retraite. `releve` faux : la convention n'est pas relevee ; `montant` `None` : elle
/// laisse l'indemnite a la loi, et `pourquoi` le dit. `mois` et `palier` : ceux du depart volontaire.
#[derive(Debug, Clone, PartialEq)]
pub struct Retraite {
    pub releve: bool,
    pub montant: Option<P>,
    pub mois: Option<P>,
    pub palier: Option<i64>,
    pub article: Option<String>,
    pub hors_primes: bool,
    pub prevenance_mois: Option<i64>,
    /// L'article de la clause du cadre de direction (mise a la retraite).
    pub cadre_de_direction: Option<String>,
    pub pourquoi: Option<String>,
}

impl Retraite {
    fn sans_montant(releve: bool, pourquoi: String) -> Retraite {
        Retraite { releve, montant: None, mois: None, palier: None, article: None, hors_primes: false, prevenance_mois: None, cadre_de_direction: None, pourquoi: Some(pourquoi) }
    }
}

/// L'indemnite conventionnelle de mise a la retraite.
pub fn indemnite_mise_a_la_retraite(idcc: &str, anciennete: Decimal, reference: Decimal) -> Resultat<Retraite> {
    let (anciennete, reference) = (P::depuis(anciennete), P::depuis(reference));
    let t = table()?;
    let conv = t.convention(idcc);
    if let Some(loi) = conv.and_then(|c| c.loi_seule(Section::MiseALaRetraite)) {
        return Ok(Retraite::sans_montant(true, loi.to_string()));
    }
    let Some(sec) = conv.and_then(|c| c.mise_a_la_retraite.as_ref()) else {
        return Ok(Retraite::sans_montant(false, non_relevees(idcc)));
    };
    let annees = anciennete.div(P::entier(12));
    let (a_des_paliers, a_des_taux) = (sec.paliers.as_ref().is_some_and(|p| !p.is_empty()), sec.taux.as_ref().is_some_and(|t| !t.is_empty()));
    let mois = if let Some(pal) = sec.paliers.as_ref().filter(|p| !p.is_empty()) {
        paliers(pal, sec.prorata.as_ref(), annees).0
    } else if let Some(taux) = sec.taux.as_ref().filter(|t| !t.is_empty()) {
        // Le bareme du licenciement, par tranches, avec son plafond.
        let mois = tranches(taux, annees);
        sec.plafond_mois.as_ref().map_or(mois, |plafond| mois.min(p(plafond)))
    } else {
        let par_annee = sec.par_annee.as_ref().ok_or_else(|| manque("mise_a_la_retraite.par_annee"))?;
        let au_dela = annees.sub(p(sec.au_dela_de_annees.as_ref().ok_or_else(|| manque("mise_a_la_retraite.au_dela_de_annees"))?));
        let mut mois = annees.mul(p(par_annee));
        if au_dela > P::ZERO {
            let en_plus = sec.en_plus_par_annee_au_dela.as_ref().ok_or_else(|| manque("mise_a_la_retraite.en_plus_par_annee_au_dela"))?;
            mois = mois.add(au_dela.mul(p(en_plus)));
        }
        mois
    };
    Ok(Retraite {
        releve: true,
        montant: Some(mois.mul(reference).eur2()),
        mois: None,
        palier: None,
        article: Some(format!("{}, art. {}", nom(idcc), sec.article)),
        // Un bareme par paliers ne dit pas de salaire hors primes.
        hors_primes: !a_des_paliers && sec.hors_primes == Some(true),
        prevenance_mois: sec.prevenance_mois,
        cadre_de_direction: sec.cadre_de_direction.as_ref().filter(|_| !a_des_paliers && !a_des_taux).map(|c| c.article.clone()),
        pourquoi: None,
    })
}

/// L'indemnite conventionnelle de depart volontaire a la retraite : un palier, pas un cumul.
pub fn indemnite_depart_retraite(idcc: &str, anciennete: Decimal, reference: Decimal) -> Resultat<Retraite> {
    let (anciennete, reference) = (P::depuis(anciennete), P::depuis(reference));
    let t = table()?;
    let conv = t.convention(idcc);
    if let Some(loi) = conv.and_then(|c| c.loi_seule(Section::DepartALaRetraite)) {
        return Ok(Retraite::sans_montant(true, loi.to_string()));
    }
    let Some(sec) = conv.and_then(|c| c.depart_a_la_retraite.as_ref()) else {
        return Ok(Retraite::sans_montant(false, non_relevees(idcc)));
    };
    let annees = anciennete.div(P::entier(12));
    let (mois, palier) = if let Some(taux) = sec.taux.as_ref().filter(|t| !t.is_empty()) {
        // Le bareme d'un licenciement, par tranches.
        let minimale = p(sec.anciennete_minimale_mois.as_ref().ok_or_else(|| manque("depart_a_la_retraite.anciennete_minimale_mois"))?);
        (if annees.mul(P::entier(12)) >= minimale { tranches(taux, annees) } else { P::ZERO }, None)
    } else if let Some(pal) = sec.paliers.as_ref().filter(|p| !p.is_empty()) {
        paliers(pal, sec.prorata.as_ref(), annees)
    } else {
        return Err(manque("depart_a_la_retraite.paliers"));
    };
    Ok(Retraite {
        releve: true,
        montant: Some(mois.mul(reference).eur2()),
        mois: Some(mois),
        palier,
        article: Some(format!("{}, art. {}", nom(idcc), sec.article)),
        hors_primes: false,
        prevenance_mois: sec.prevenance_mois,
        cadre_de_direction: None,
        pourquoi: None,
    })
}

// ------------------------------------------------- la prime d'anciennete
/// La prime d'anciennete d'un technicien pour le mois qui finit un jour : `base` vaut `None` et `pourquoi` dit ce qui
/// manque (la jauge de la salle, une grille non relevee pour cette date, le niveau du metier).
#[derive(Debug, Clone, PartialEq)]
pub struct Prime {
    pub pourcentage: i64,
    pub base: Option<P>,
    pub article: String,
    pub pourquoi: Option<String>,
    pub annees: i64,
}

/// La prime d'anciennete d'un technicien de l'annexe de la convention qui en porte une, pour le mois qui finit `jour`,
/// ou `None` si elle ne le concerne pas.
pub fn prime_anciennete(idcc: &str, annexe: &str, fiche: &ChampsSalarie, e: &DitParLEngagement, jour: &Jour, jauge: Option<&str>) -> Resultat<Option<Prime>> {
    let t = table()?;
    let Some(sec) = t.convention(idcc).and_then(|c| c.prime_anciennete.as_ref()) else { return Ok(None) };
    if annexe.trim() != sec.annexe {
        return Ok(None);
    }
    let code = e.metier.clone().or_else(|| fiche.code_emploi_ft.remplie().map(str::to_string));
    let m = code.as_deref().map(emplois::metier).transpose()?.flatten();
    let statut = e.statut.as_deref().unwrap_or(fiche.statut.code());
    let (Some(m), Some(code)) = (m.as_ref(), code.as_deref()) else { return Ok(None) };
    let Some(c3090) = m.classification_3090().filter(|c| c.grille == "technique") else { return Ok(None) };
    if statut == "artiste" {
        return Ok(None);
    }
    let famille = m.famille();
    let article = format!("{}, {}", nom(idcc), sec.article);
    let Some(debut) = fiche.date_anciennete.dite().copied().or(e.debut) else { return Ok(None) };
    let annees = jour.an as i64 - debut.an as i64 - ((jour.mois, jour.jour) < (debut.mois, debut.jour)) as i64;
    let pourcentage = sec.plafond_pourcentage.min((annees - (sec.apres_annees - 1)).max(0) * sec.pourcentage_par_annee);
    if pourcentage <= 0 {
        return Ok(None);
    }
    let prime = |base: Option<P>, pourquoi: Option<String>| Prime { pourcentage, base, article: article.clone(), pourquoi, annees };
    let Some(g) = sec.salaires_conventionnels.iter().filter(|g| g.depuis <= *jour).last() else {
        let premiere = sec.salaires_conventionnels.first().map(|g| g.depuis.iso()).ok_or_else(|| manque("prime_anciennete.salaires_conventionnels"))?;
        return Ok(Some(prime(None, Some(format!(
            "les grilles de salaire conventionnel de l'annexe I ne sont relevees qu'a partir du {}",
            premiere
        )))));
    };
    let Some(niveau) = sec.niveaux.get(&c3090.niveau).filter(|n| !n.is_empty()) else {
        return Ok(Some(prime(None, Some(format!("le niveau 3090 du metier {} n'est pas connu", code)))));
    };
    let base = if sec.familles_costume.iter().flatten().any(|f| f == famille) {
        g.costume.get(niveau).ok_or_else(|| manque(&format!("prime_anciennete.salaires_conventionnels.costume.{}", niveau)))?
    } else {
        let Some((_, salaires)) = jauge.and_then(|j| g.technique.iter().find(|(k, _)| k == j)) else {
            let cles: Vec<String> = g.technique.iter().map(|(k, _)| format!("\"{}\"", k)).collect();
            return Ok(Some(prime(None, Some(format!(
                "le salaire conventionnel d'un technicien de l'annexe I depend de la JAUGE de la \
                 salle, et la fiche de la compagnie ne la dit pas (`jauge_annexe_1` : {})",
                cles.join(", ")
            )))));
        };
        salaires.get(niveau).ok_or_else(|| manque(&format!("prime_anciennete.salaires_conventionnels.technique.{}", niveau)))?
    };
    Ok(Some(prime(Some(p(base)), None)))
}
