//! Les minima conventionnels : la table livree (`minima-conventionnels.json`, une grille par convention), lue par sa
//! forme (`Table`, la seule lecture : `table`), ses paliers, les accords de salaires qui la suivent et depuis quand ils
//! lient une compagnie, et ce qu'un minimum est. Les confronter a une paie, et l'avertir, est l'affaire de l'engagement
//! (`engagement::minima`).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use socle::date::Jour;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::forme::Nombre;
use socle::num::{Decimal, Resultat};
use socle::texte::sigle;

use crate::source::forme_notee;

// ------------------------------------------------------------ la forme du fichier

forme_notee! {
    /// `donnees/minima-conventionnels.json` : une grille par convention, par son IDCC.
    pub struct Table {
        pub conventions: BTreeMap<String, Convention>,
    }
}

forme_notee! {
    /// UNE CONVENTION, OU UN ACCORD DE SALAIRES QUI SUIT SA GRILLE (`accords`, de la meme forme) : le texte qui la donne
    /// (`texte`, sa premiere partie dans la phrase d'un avertissement), le lien vers la convention, et ses grilles. Chaque
    /// convention n'en porte qu'une partie : la 3090 par annexe (`annexes`, `clauses_communes`), la 1285 ses cachets, son
    /// service de repetition, ses emplois non artistiques et ses accords, la 2642 et la 3097 leurs tables propres, toutes
    /// des grilles (`grilles`, `grilles_complementaires`).
    pub struct Convention {
        /// L'accord, tel qu'une valeur du bareme le nomme (`accord`).
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default)]
        pub libelle: Option<String>,
        #[serde(default)]
        pub texte: Option<String>,
        #[serde(default)]
        pub lien: Option<String>,
        #[serde(default)]
        pub en_vigueur: Option<String>,
        #[serde(default)]
        pub etendu: Option<bool>,
        /// Le jour ou l'extension de l'accord prend effet (sa publication au JORF).
        #[serde(default)]
        pub etendu_le: Option<String>,
        /// Les organisations d'employeurs signataires de l'accord.
        #[serde(default)]
        pub signataires: Vec<String>,
        #[serde(default)]
        pub annexes: BTreeMap<String, Annexe>,
        #[serde(default)]
        pub clauses_communes: Option<ClausesCommunes>,
        #[serde(default)]
        pub cachets: BTreeMap<String, GrilleDeCachets>,
        #[serde(default)]
        pub repetition: Option<RepetitionParService>,
        #[serde(default)]
        pub emplois_autres_qu_artistiques: Option<EmploisAutres>,
        #[serde(default)]
        pub accords: Vec<Convention>,
        #[serde(default)]
        pub grilles: Vec<Grille>,
        #[serde(default)]
        pub grilles_complementaires: Vec<Grille>,
        /// 2642 : le minimum d'un CDDU a la semaine de 35 h, par fonction.
        #[serde(default)]
        pub cddu_semaine_35h: BTreeMap<String, Cddu2642>,
        /// 2642 : le mensuel pour 35 h d'un permanent, par fonction.
        #[serde(default)]
        pub permanents_mensuel_35h: BTreeMap<String, Nombre>,
        /// 3097 : la semaine de 39 h d'un technicien, par fonction.
        #[serde(default)]
        pub techniciens_semaine_39h: BTreeMap<String, Nombre>,
        /// 3097 : les heures que la semaine de 39 h paie au taux de base (35 h + 4 h a 125 %).
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub heures_payees_de_la_semaine: Option<Decimal>,
        #[serde(default)]
        pub avant_2026_05: Option<Avant2026>,
        #[serde(default)]
        pub annexe_iii: Option<AnnexeIii>,
        #[serde(default)]
        pub artistes: Option<Artistes3097>,
        #[serde(default)]
        pub complement: Option<Complement3097>,
        #[serde(default)]
        pub permanents_mensuel: Option<Permanents3097>,
    }
}

forme_notee! {
    /// Une annexe de la 3090 : ses cachets d'artistes, sa repetition, le taux horaire de ses techniciens.
    pub struct Annexe {
        pub libelle: String,
        #[serde(default)]
        pub cachets: BTreeMap<String, GrilleDeCachets>,
        #[serde(default)]
        pub repetition: Option<RepetitionDeLAnnexe>,
        #[serde(default)]
        pub techniciens: Option<ParNiveau<Nombre>>,
    }
}

forme_notee! {
    /// Les clauses communes de la 3090 : le taux horaire des techniciens, la grille mensuelle des administratifs.
    pub struct ClausesCommunes {
        pub techniciens: ParNiveau<Nombre>,
        pub administratifs_mensuel_echelons: ParNiveau<Vec<f64>>,
    }
}

/// UNE VALEUR PAR NIVEAU DE LA 3090 (`classification_3090.niveau` d'un metier), et ses notes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
pub struct ParNiveau<T> {
    #[serde(default)]
    pub cadre_groupe_1: Option<T>,
    #[serde(default)]
    pub cadre_groupe_2: Option<T>,
    #[serde(default)]
    pub cadre_groupe_3: Option<T>,
    #[serde(default)]
    pub agent_de_maitrise: Option<T>,
    #[serde(default)]
    pub employe_qualifie_1: Option<T>,
    #[serde(default)]
    pub employe_qualifie_2: Option<T>,
    #[serde(default)]
    pub employe: Option<T>,
    #[serde(default)]
    pub quoi: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub source: Option<crate::source::Fondement>,
}

impl<T> ParNiveau<T> {
    /// La valeur du niveau `niveau`, si la grille en a une.
    pub fn du_niveau(&self, niveau: &str) -> Option<&T> {
        match niveau {
            "cadre_groupe_1" => self.cadre_groupe_1.as_ref(),
            "cadre_groupe_2" => self.cadre_groupe_2.as_ref(),
            "cadre_groupe_3" => self.cadre_groupe_3.as_ref(),
            "agent_de_maitrise" => self.agent_de_maitrise.as_ref(),
            "employe_qualifie_1" => self.employe_qualifie_1.as_ref(),
            "employe_qualifie_2" => self.employe_qualifie_2.as_ref(),
            "employe" => self.employe.as_ref(),
            _ => None,
        }
    }
}

forme_notee! {
    /// La grille des cachets d'un emploi : ses paliers sur le nombre de representations, et sa jauge quand elle en depend.
    pub struct GrilleDeCachets {
        pub libelle: String,
        #[serde(default)]
        pub periode: Option<String>,
        #[serde(default)]
        pub jauge: Option<String>,
        pub paliers: Vec<Palier>,
        #[serde(default)]
        pub autres_jauges: BTreeMap<String, Vec<Nombre>>,
    }
}

/// Un palier : le montant jusqu'a `jusqu_a` representations (`None` : au-dela).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palier {
    pub jusqu_a: Option<i64>,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub montant: Decimal,
}

forme_notee! {
    /// La repetition d'une annexe de la 3090 : le cachet de repetition, le taux horaire, le service d'un musicien, le
    /// service isole.
    pub struct RepetitionDeLAnnexe {
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub cachet: Option<Decimal>,
        #[serde(default)]
        pub cachet_libelle: Option<String>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub heure: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub service_musicien: Option<Decimal>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub service_isole: Option<Decimal>,
    }
}

forme_notee! {
    /// Le service de repetition de la 1285, et celui de chaque grille d'artiste qui en a un propre.
    pub struct RepetitionParService {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub service: Decimal,
        pub service_libelle: String,
        #[serde(default)]
        pub par_categorie: BTreeMap<String, ServiceDeCategorie>,
    }
}

/// Le service de repetition propre a une grille d'artiste.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceDeCategorie {
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub service: Decimal,
    pub libelle: String,
}

forme_notee! {
    /// Les emplois autres qu'artistiques de la 1285 : le mensuel de chaque echelon, par groupe, pour `heures_mensuelles`.
    pub struct EmploisAutres {
        pub heures_mensuelles: f64,
        pub echelon_par_defaut: i64,
        pub groupes: BTreeMap<String, Vec<f64>>,
    }
}

/// Une fonction de la 2642 : sa filiere, son niveau, le minimum de la semaine par genre de programme.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cddu2642 {
    pub filiere: String,
    pub niveau: String,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub fiction: Option<Decimal>,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub flux: Option<Decimal>,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub autre: Option<Decimal>,
}

impl Cddu2642 {
    /// Les genres que la fonction connait, dans l'ordre fiction, flux, autre, avec leur minimum.
    pub fn genres(&self) -> Vec<(&'static str, Decimal)> {
        [("fiction", self.fiction), ("flux", self.flux), ("autre", self.autre)].into_iter().filter_map(|(g, m)| m.map(|m| (g, m))).collect()
    }
}

forme_notee! {
    /// 3097 : les deux fonctions reevaluees au 01/05/2026, a leur montant d'avant.
    pub struct Avant2026 {
        pub montants: BTreeMap<String, Nombre>,
    }
}

forme_notee! {
    /// 3097, titre II, annexe III : le socle de la semaine et la part payee au-dela.
    pub struct AnnexeIii {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub socle: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub part_au_dela: Decimal,
        pub depuis: String,
    }
}

forme_notee! {
    /// 3097, titre III : les artistes-interpretes d'un long et d'un court-metrage.
    pub struct Artistes3097 {
        pub long: Engagement3097,
        pub court: Engagement3097,
    }
}

/// Un engagement d'artiste de la 3097 : la journee, la semaine de 5 et de 6 jours, et les repetitions d'un
/// long-metrage.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engagement3097 {
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub journee: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub semaine_5j: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub semaine_6j: Decimal,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub repetition_service_4h: Option<Decimal>,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub repetition_2_services_4h: Option<Decimal>,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub repetition_service_3h_danse_chant_musique: Option<Decimal>,
    #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
    pub repetition_2_services_3h_danse_chant_musique: Option<Decimal>,
}

forme_notee! {
    /// 3097, titre III, sous-titre II : les acteurs de complement d'un long-metrage, par role.
    pub struct Complement3097 {
        pub figuration: Engagement3097,
        pub silhouette_muette: Engagement3097,
        pub silhouette_parlante: Engagement3097,
        pub doublure_simple: Engagement3097,
        pub doublure_polyvalente: Engagement3097,
    }
}

impl Complement3097 {
    /// Le role `role` (la ligne de minima du metier dans la 3097), s'il en est un.
    pub fn du_role(&self, role: &str) -> Option<&Engagement3097> {
        match role {
            "figuration" => Some(&self.figuration),
            "silhouette_muette" => Some(&self.silhouette_muette),
            "silhouette_parlante" => Some(&self.silhouette_parlante),
            "doublure_simple" => Some(&self.doublure_simple),
            "doublure_polyvalente" => Some(&self.doublure_polyvalente),
            _ => None,
        }
    }
}

forme_notee! {
    /// 3097, titre IV : le mensuel de base des permanents par niveau, et le niveau de chaque emploi repere.
    pub struct Permanents3097 {
        pub niveaux: BTreeMap<String, Nombre>,
        pub emplois_reperes: BTreeMap<String, String>,
    }
}

forme_notee! {
    /// UNE GRILLE D'UNE CONVENTION : qui elle vise (population, statut, unite de paie, categories du metier, codes,
    /// conditions sur le spectacle, l'entreprise et le salarie, anciennete, duree du contrat), ses valeurs par classement
    /// (dans l'ordre du texte), ses majorations, et ce qui la ramene a l'heure (`heures`, `facteur`) ou l'abat.
    pub struct Grille {
        pub nom: String,
        #[serde(default)]
        pub population: Option<String>,
        #[serde(default)]
        pub statut: Option<String>,
        pub unite: String,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub heures: Option<Decimal>,
        #[serde(default)]
        pub categories: Option<Vec<String>>,
        #[serde(default)]
        pub codes: Option<Vec<String>>,
        #[serde(default)]
        pub si_spectacle: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub si_entreprise: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub si_salarie: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub anciennete: Option<Anciennete>,
        #[serde(default)]
        pub duree_contrat_max_jours: Option<i64>,
        #[serde(default)]
        pub remplace: Vec<String>,
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub valeurs: Vec<(String, Nombre)>,
        #[serde(default)]
        pub majorations: Vec<Majoration>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub abattement_max: Option<Decimal>,
        #[serde(default)]
        pub abattement_motif: Option<String>,
        #[serde(default, deserialize_with = "socle::forme::decimal_opt")]
        pub facteur: Option<Decimal>,
        #[serde(default)]
        pub facteur_motif: Option<String>,
    }
}

forme_notee! {
    /// Une majoration d'une grille : ses conditions, son groupe (seule la derniere d'un groupe compte), et ce qu'elle
    /// ajoute a chaque valeur.
    pub struct Majoration {
        pub nom: String,
        #[serde(default)]
        pub groupe: Option<String>,
        #[serde(default)]
        pub categories: Option<Vec<String>>,
        #[serde(default)]
        pub codes: Option<Vec<String>>,
        #[serde(default)]
        pub si_spectacle: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub si_entreprise: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub si_salarie: BTreeMap<String, Attendu>,
        #[serde(default)]
        pub anciennete: Option<Anciennete>,
        #[serde(default)]
        pub duree_contrat_max_jours: Option<i64>,
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub par_cle: Vec<(String, Nombre)>,
    }
}

/// L'anciennete qu'une grille ou une majoration demande : `ans` revolus depuis la case `champ` de la fiche du salarie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anciennete {
    pub champ: String,
    pub ans: i64,
}

/// LES CONDITIONS D'UNE GRILLE OU D'UNE MAJORATION, telles que l'engagement les confronte a une ligne de paie.
pub struct Conditions<'a> {
    pub categories: Option<&'a [String]>,
    pub codes: Option<&'a [String]>,
    pub si_spectacle: &'a BTreeMap<String, Attendu>,
    pub si_entreprise: &'a BTreeMap<String, Attendu>,
    pub si_salarie: &'a BTreeMap<String, Attendu>,
    pub anciennete: Option<&'a Anciennete>,
    pub duree_contrat_max_jours: Option<i64>,
}

impl Grille {
    pub fn conditions(&self) -> Conditions<'_> {
        Conditions {
            categories: self.categories.as_deref(),
            codes: self.codes.as_deref(),
            si_spectacle: &self.si_spectacle,
            si_entreprise: &self.si_entreprise,
            si_salarie: &self.si_salarie,
            anciennete: self.anciennete.as_ref(),
            duree_contrat_max_jours: self.duree_contrat_max_jours,
        }
    }
}

impl Majoration {
    pub fn conditions(&self) -> Conditions<'_> {
        Conditions {
            categories: self.categories.as_deref(),
            codes: self.codes.as_deref(),
            si_spectacle: &self.si_spectacle,
            si_entreprise: &self.si_entreprise,
            si_salarie: &self.si_salarie,
            anciennete: self.anciennete.as_ref(),
            duree_contrat_max_jours: self.duree_contrat_max_jours,
        }
    }
}

/// Ce qu'une condition de grille (`si_*`) attend d'une case : cochee ou non, ou une valeur.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Attendu {
    Oui(bool),
    Valeur(String),
}

/// Une case lue pour une condition : si elle dit quelque chose (`vraie`), et son texte (vide si elle ne dit rien).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CaseLue {
    pub vraie: bool,
    pub texte: String,
}

impl Attendu {
    /// La case lue est-elle celle qu'on attend ?
    pub fn vaut(&self, lu: &CaseLue) -> bool {
        match self {
            Attendu::Oui(b) => lu.vraie == *b,
            Attendu::Valeur(v) => lu.texte == *v,
        }
    }
}

const ILLISIBLE: &str = "Les minima conventionnels livrés avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE DE `minima-conventionnels.json` : sa forme.
pub fn table() -> Resultat<Arc<Table>> {
    static CACHE: OnceLock<FormeGardee<Table>> = OnceLock::new();
    forme_en_cache(&CACHE, "minima-conventionnels.json", ILLISIBLE, Ok)
}

// ------------------------------------------------------------ les lectures

/// Ce que l'IDCC d'un contrat lit du spectacle : s'il est une emission de television de la 1734 (`emission_tv_1734`),
/// et son titre.
pub struct SpectacleDuContrat<'a> {
    pub emission_tv_1734: bool,
    pub titre: &'a str,
}

/// L'IDCC d'un contrat : celui de la compagnie (`idcc`), sauf l'artiste-interprète d'une émission de télévision
/// (`emission_tv_1734` du spectacle) d'une compagnie de la 2642 ou de la 3241, qui relève de la 1734
/// (`minima-conventionnels.json`, la note de la 1734). Le contrat le déclare (S21.G00.40.017), le bulletin l'imprime, le
/// minimum lit sa grille. Artiste-interprète se lit au métier du catalogue (familles « Artistes », « Cirque et rue ») :
/// un métier hors catalogue ne dit pas s'il l'est, l'IDCC se refuse (règle 1).
pub fn idcc_du_contrat(idcc: &str, sp: &SpectacleDuContrat, sal: &crate::fiche::ChampsSalarie) -> Resultat<String> {
    let idcc = idcc.to_string();
    if (idcc != "2642" && idcc != "3241") || !sp.emission_tv_1734 || sal.statut.dite().is_some_and(|s| *s != crate::fiche::Statut::Artiste) {
        return Ok(idcc);
    }
    let code = sal.code_emploi_ft.dite().filter(|c| !c.is_empty()).cloned();
    let metier = match &code {
        Some(c) => crate::emplois::metier(c)?,
        None => None,
    };
    match metier.as_ref().and_then(|m| m.au_catalogue()).map(|f| f.famille.as_str()) {
        None => Err(format!(
            "Spectacle {}, émission de télévision (convention 1734) : le métier {} de {} n'est pas au catalogue, et \
             le logiciel ne sait pas s'il est artiste-interprète (1734) ou musicien, metteur en scène, artiste de \
             complément (convention de l'entreprise). Choisir un métier du catalogue.",
            sp.titre,
            code.unwrap_or_else(|| "(aucun)".into()),
            sal.nom.remplie().unwrap_or("ce salarié")
        )),
        Some("Artistes" | "Cirque et rue") => Ok("1734".into()),
        Some(_) => Ok(idcc),
    }
}

/// Le plus bas des candidats selon `valeur` (le premier a egalite) : un minimum prend, quand la paie ne dit pas tout,
/// l'hypothese la plus basse.
pub fn plus_bas<T>(v: Vec<T>, valeur: impl Fn(&T) -> Decimal) -> Option<T> {
    let mut mini: Option<(Decimal, T)> = None;
    for x in v {
        let k = valeur(&x);
        if mini.as_ref().map_or(true, |(m, _)| k < *m) {
            mini = Some((k, x));
        }
    }
    mini.map(|(_, x)| x)
}

/// Les dix premiers caracteres d'une date, AAAA-MM-JJ ou AAAAMMJJ.
pub fn iso(s: &str) -> Option<Jour> {
    let s: String = s.chars().take(10).collect();
    let c = s.as_bytes();
    let n = |a: usize, b: usize| -> Option<u32> {
        let x = s.get(a..b)?;
        if x.bytes().all(|c| c.is_ascii_digit()) { x.parse().ok() } else { None }
    };
    match c.len() {
        10 if c[4] == b'-' && c[7] == b'-' => Jour::nouveau(n(0, 4)? as i32, n(5, 7)?, n(8, 10)?),
        8 => Jour::nouveau(n(0, 4)? as i32, n(4, 6)?, n(6, 8)?),
        _ => None,
    }
}

/// Les annees revolues de `depuis` au `jour` (une anciennete, un age) ; `None` si l'une des dates ne se lit pas.
pub fn ans_revolus(depuis: &str, jour: Option<&str>) -> Option<i64> {
    let a = iso(depuis)?;
    let b = iso(jour?)?;
    Some(b.an as i64 - a.an as i64 - i64::from((b.mois, b.jour) < (a.mois, a.jour)))
}

/// Un minimum : le montant, d'ou il vient, et s'il se compare a l'unite.
pub struct Plancher {
    pub montant: Decimal,
    pub quoi: String,
    pub unite: bool,
}

/// Un plancher qui se compare a la ligne entiere.
pub fn plancher(montant: Decimal, quoi: String) -> Option<Plancher> {
    Some(Plancher { montant, quoi, unite: false })
}

/// Le montant du palier ou tombe `nombre` de representations.
pub fn palier(paliers: &[Palier], nombre: i64) -> Option<Decimal> {
    paliers.iter().find(|p| p.jusqu_a.map_or(true, |j| nombre <= j)).map(|p| p.montant)
}

impl Table {
    /// La convention `idcc`, si la table la connait.
    pub fn convention(&self, idcc: &str) -> Option<&Convention> {
        self.conventions.get(idcc)
    }
}

/// L'accord de salaires `id` d'une convention de la table (`<idcc>.accords`), ou `None`.
pub fn accord<'a>(t: &'a Table, id: &str) -> Option<&'a Convention> {
    t.conventions.values().flat_map(|c| c.accords.iter()).find(|a| a.id.as_deref() == Some(id))
}

/// La compagnie adherente de `organisation` l'est-elle d'une organisation d'employeurs signataire de l'accord ?
pub fn signataire(accord: &Convention, organisation: Option<&str>) -> bool {
    let Some(s) = organisation.map(sigle).filter(|s| !s.is_empty()) else { return false };
    accord.signataires.iter().any(|x| sigle(x) == s)
}

/// LE JOUR OU UNE STIPULATION D'UN ACCORD, DATEE `depuis` PAR L'ACCORD, LIE LA COMPAGNIE ADHERENTE DE `organisation` :
/// `depuis` pour l'adherente d'une organisation signataire (C. trav. L2262-1 : l'accord oblige « les signataires ou
/// membres des organisations ou groupements signataires »), le jour de l'extension pour toute autre compagnie de la
/// convention (`etendu_le` ; L2261-15), au plus tot `depuis` ; `None` tant que l'accord n'est pas etendu et ne la lie
/// pas. Les deux articles : `atelier/reference/sources/legi-entrepot/articles.jsonl.gz`. Les minima d'un accord
/// (`grille_au`) et les valeurs datees du bareme qui en viennent (`accord` dans l'entree,
/// `droit::contexte::dater_les_accords`) se lisent ici.
pub fn lie_depuis(accord: &Convention, organisation: Option<&str>, depuis: &str) -> Option<String> {
    let depuis: String = depuis.chars().take(10).collect();
    if signataire(accord, organisation) {
        return Some(depuis);
    }
    let etendu: String = accord.etendu_le.as_deref().filter(|e| !e.is_empty())?.chars().take(10).collect();
    Some(etendu.max(depuis))
}

/// LA GRILLE DE LA CONVENTION `idcc` QUI LIE LA COMPAGNIE AU `jour` : celle du dernier de ses accords (`accords`) dont
/// les minima (`en_vigueur`) la lient a ce jour (`lie_depuis`), avec le jour depuis lequel il la lie ; a defaut, la
/// grille de la convention elle-meme. `None` : la table ne connait pas la convention.
pub fn grille_au<'a>(t: &'a Table, idcc: &str, organisation: Option<&str>, jour: Option<&str>) -> Option<(&'a Convention, Option<String>)> {
    let base = t.convention(idcc)?;
    let jour: Option<String> = jour.map(|j| j.chars().take(10).collect());
    let lie = |a: &'a Convention| {
        let depuis = lie_depuis(a, organisation, a.en_vigueur.as_deref().unwrap_or_default())?;
        jour.as_ref().filter(|j| **j >= depuis).map(|_| (a, Some(depuis)))
    };
    Some(base.accords.iter().rev().find_map(lie).unwrap_or((base, None)))
}

/// Ce qu'un minimum compare : le prix d'une heure, d'un cachet, d'un service, ou d'une unite de grille (la journee,
/// la semaine, le mois).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Par {
    Heure,
    Cachet,
    Service,
    Unite,
}

/// Le minimum conventionnel d'une ligne de paie, tel que les avertissements le lisent : le montant, la grille d'ou il
/// vient, ce qu'il compare. `service_3090` : le service de repetition d'un artiste de la 3090 (sa phrase est a part).
#[derive(Debug, Clone, PartialEq)]
pub struct Minimum {
    pub montant: Decimal,
    pub quoi: String,
    pub par: Par,
    pub service_3090: bool,
}
