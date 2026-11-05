//! Les natures de contrat : ce que la DSN ecrit de la STRUCTURE d'un contrat, lu dans `donnees/contrats-base.json`.
//!
//! Ce module ne decide rien et ne calcule rien : il lit le fichier par sa forme (`lu`, la seule lecture), et rend des
//! valeurs.

use serde::{Deserialize, Serialize};
use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::num::Resultat;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// Les types de contrat de la DPAE (Format 120 v5.3, C_TYPE_CONTRAT).
pub const TYPES_CONTRAT_DPAE: [(&str, &str); 3] = [("1", "CDD"), ("2", "CDI"), ("3", "CTT")];

/// Une date civile (an, mois, jour) : le minimum pour `contrat_court`.
pub type Jour = (i32, u32, u32);

fn faux(b: &bool) -> bool {
    !*b
}

/// La face de paie d'une nature : aux cachets et aux heures, ou au mois.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Paie {
    Activite,
    Mensuelle,
}

/// La rubrique du motif de recours (S21.G00.40.021), dont une nature peut dire les valeurs (`champs`).
const MOTIF_DE_RECOURS: &str = "S21.G00.40.021";

/// UNE NATURE DE CONTRAT, telle que `contrats-base.json` la decrit (`natures.<cle>`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nature {
    /// La cle de la nature dans le fichier (`cddu_spectacle`, `cdi`, `cdd`).
    #[serde(skip)]
    pub cle: String,
    pub libelle: String,
    /// Ce que le nom court veut dire, pour qui hesite en choisissant.
    pub aide: String,
    #[serde(rename = "source", default)]
    _source: Option<String>,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    pub paie: Paie,
    pub duree_indeterminee: bool,
    bareme: Bareme,
    unite_quotite: UniteDeQuotite,
    #[serde(rename = "quotite_reference_defaut", default)]
    quotite_reference: Option<QuotiteDeReference>,
    /// rubrique -> valeur fixe.
    codes: BTreeMap<String, CodeFixe>,
    /// rubrique -> d'ou la valeur vient.
    champs: BTreeMap<String, Champ>,
    /// rubrique -> pourquoi cette nature ne l'ecrit pas.
    #[serde(rename = "absents")]
    _absents: BTreeMap<String, String>,
    /// rubrique -> le controle qui la refuse pour cette nature.
    interdits: BTreeMap<String, String>,
    /// Le contrat ecrit : son modele, celui de son avenant, ses mentions obligatoires.
    #[serde(default)]
    contrat_ecrit: Option<ContratEcrit>,
    dpae: Dpae,
    fin_de_contrat: FinDeContrat,
    remunerations: Remunerations,
    activites: Activites,
}

/// La section `par_nature` du bareme a appliquer par-dessus le statut (`None` : le corps du bareme).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bareme {
    section: Option<String>,
    #[serde(rename = "note")]
    _note: String,
}

/// S21.G00.40.011, et le plafond de 40.012 et 40.013 (CCH-12 du bloc 40, 31.00 en journee ; CCH-13, 250.00 en heure).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct UniteDeQuotite {
    valeur: String,
    plafond: f64,
    #[serde(rename = "quoi")]
    _quoi: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotiteDeReference {
    valeur: f64,
    #[serde(rename = "quoi")]
    _quoi: String,
}

/// Une rubrique a valeur fixe, et ce que cette valeur dit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeFixe {
    valeur: String,
    #[serde(rename = "quoi")]
    _quoi: String,
}

/// Une rubrique dont la valeur se lit sur l'engagement ou la fiche. Le motif de recours (S21.G00.40.021) d'un CDD porte
/// en plus ses `valeurs` et s'il est `obligatoire`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Champ {
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note", default)]
    _note: Option<String>,
    #[serde(default)]
    obligatoire: bool,
    #[serde(default)]
    valeurs: Vec<MotifDeRecours>,
}

/// UN MOTIF DE RECOURS (S21.G00.40.021) qu'une nature propose.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotifDeRecours {
    pub code: String,
    pub libelle: String,
    /// Le CDD conclu au titre du 1° de L1242-2 nomme la personne remplacee (L1242-12, 1°).
    #[serde(default, skip_serializing_if = "faux")]
    pub personne_remplacee: bool,
}

/// UN MOTIF DE RUPTURE (S21.G00.62.002) que la CCH-11 autorise pour la nature, et ce qu'il ouvre (`dit`, sa `source`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotifDeRupture {
    pub code: String,
    pub libelle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContratEcrit {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "note")]
    _note: String,
    /// Le nom du contrat, tel qu'une phrase le dit (« contrat de travail à durée déterminée »).
    nom: String,
    modele: String,
    avenant: String,
    phrases: Phrases,
    mentions: Vec<Mention>,
}

/// LES PHRASES DE RENVOI du contrat ecrit (R. 1221-35 permet le renvoi aux textes) ; le motif, celui d'une nature qui
/// fixe le sien (le CDDU).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrases {
    #[serde(default)]
    pub motif: Option<String>,
    pub formation: String,
    pub conges: String,
    pub rupture: String,
}

/// UNE MENTION que la loi exige du contrat ecrit : son marqueur, le texte qui l'exige, et si elle n'est due que dite.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mention {
    pub marqueur: String,
    pub texte: String,
    #[serde(default)]
    pub si_dite: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dpae {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: crate::source::Fondement,
    type_contrat: TypeContratDpae,
    date_fin: DateFinDpae,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeContratDpae {
    valeur: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct DateFinDpae {
    obligatoire: bool,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Le bloc S21.G00.62.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinDeContrat {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    /// Le motif quand le contrat va a son terme (`None` pour un CDI : il se saisit).
    motif_par_defaut: Option<CodeFixe>,
    motifs: Vec<MotifDeRupture>,
    signalement_fctu: SignalementFctu,
    #[serde(default)]
    circuit_derogatoire: Option<CircuitDerogatoire>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignalementFctu {
    valeur: bool,
    #[serde(rename = "note")]
    _note: String,
}

/// La modalite de declaration de la fin d'un contrat d'usage (S21.G00.62.017).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CircuitDerogatoire {
    rubrique: String,
    motifs: Vec<String>,
    applique: String,
    non_applique: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Les blocs S21.G00.51 d'une paie.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Remunerations {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note", default)]
    _note: Option<String>,
    types: Vec<TypeDeRemuneration>,
    /// Les quatre blocs 51 valent-ils tous le brut ? (vrai sur un CDDU)
    toutes_au_brut: bool,
}

/// UN TYPE DE REMUNERATION OBLIGATOIRE (S21.G00.51.011, cahier technique § 2.1.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TypeDeRemuneration {
    /// 001 - Rémunération brute non plafonnée.
    #[serde(rename = "001")]
    Brute,
    /// 002 - Salaire brut servant aux calculs des droits de l'Assurance chômage.
    #[serde(rename = "002")]
    AssietteChomage,
    /// 003 - Salaire rétabli - reconstitué.
    #[serde(rename = "003")]
    SalaireRetabli,
    /// 010 - Salaire de base.
    #[serde(rename = "010")]
    SalaireDeBase,
}

impl TypeDeRemuneration {
    pub fn code(self) -> &'static str {
        match self {
            TypeDeRemuneration::Brute => "001",
            TypeDeRemuneration::AssietteChomage => "002",
            TypeDeRemuneration::SalaireRetabli => "003",
            TypeDeRemuneration::SalaireDeBase => "010",
        }
    }
}

/// Les blocs S21.G00.53 sous la remuneration de type 002.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activites {
    #[serde(rename = "quoi")]
    _quoi: String,
    unites: Vec<UniteDActivite>,
    #[serde(default)]
    absences: Option<UniteDAbsence>,
}

/// Ce qu'une unite d'activite (S21.G00.53) compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mesure {
    Jours,
    Cachets,
    Heures,
}

/// UNE UNITE D'ACTIVITE (S21.G00.53.003), le type d'activite (53.001, `01` s'il ne se dit pas) et ce qu'elle mesure.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniteDActivite {
    pub code: String,
    pub quoi: String,
    pub mesure: Mesure,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_activite: Option<String>,
    /// Elle ne s'ecrit pas quand elle vaut zero.
    #[serde(default, skip_serializing_if = "faux")]
    pub si_non_nul: bool,
    /// Elle ne s'ecrit que pour un contrat declare a la caisse des Conges Spectacles (`caisse_conges::affilie`).
    #[serde(default, skip_serializing_if = "faux")]
    pub si_caisse_conges: bool,
}

impl UniteDActivite {
    /// S21.G00.53.001 : `01` (travail remunere) quand l'unite ne dit pas le sien.
    pub fn type_d_activite(&self) -> &str {
        self.type_activite.as_deref().unwrap_or("01")
    }
}

/// L'unite des absences (S21.G00.53, type 02).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniteDAbsence {
    pub code: String,
    #[serde(rename = "type")]
    pub type_activite: String,
    pub quoi: String,
}

impl Nature {
    /// Paye au mois : un permanent.
    pub fn mensuelle(&self) -> bool {
        self.paie == Paie::Mensuelle
    }

    pub fn permanent(&self) -> bool {
        self.mensuelle()
    }

    /// La section `par_nature` du bareme a appliquer par-dessus le statut, ou `None` : le corps du bareme.
    pub fn bareme(&self) -> Option<&str> {
        self.bareme.section.as_deref()
    }

    /// S21.G00.40.011.
    pub fn unite_quotite(&self) -> &str {
        &self.unite_quotite.valeur
    }

    /// Le plafond de 40.012 et 40.013 : CCH-12 du bloc 40, 31.00 en journee ; CCH-13, 250.00 en heure.
    pub fn plafond_quotite(&self) -> f64 {
        self.unite_quotite.plafond
    }

    /// La quotite de reference d'un contrat au mois (la duree legale, L. 3121-27), ou `None` : la nature n'en a pas.
    pub fn quotite_reference_defaut(&self) -> Option<f64> {
        self.quotite_reference.as_ref().map(|q| q.valeur)
    }

    /// Le modele livre du contrat ecrit de cette nature, ou `None` : elle n'en a pas.
    pub fn modele_contrat(&self) -> Option<String> {
        self.contrat_ecrit.as_ref().map(|c| c.modele.clone())
    }

    /// Le nom de son contrat ecrit, tel qu'une phrase le dit (« contrat de travail à durée déterminée »).
    pub fn nom_du_contrat(&self) -> String {
        self.contrat_ecrit.as_ref().map_or_else(|| "contrat de travail".into(), |c| c.nom.clone())
    }

    /// Le modele livre de l'avenant de son contrat ecrit.
    pub fn modele_avenant(&self) -> Option<String> {
        self.contrat_ecrit.as_ref().map(|c| c.avenant.clone())
    }

    /// Les phrases de renvoi de son contrat ecrit.
    pub fn phrases(&self) -> Option<&Phrases> {
        self.contrat_ecrit.as_ref().map(|c| &c.phrases)
    }

    /// Les mentions obligatoires de son contrat ecrit, dans l'ordre de `contrats-base.json`.
    pub fn mentions_obligatoires(&self) -> &[Mention] {
        self.contrat_ecrit.as_ref().map_or(&[], |c| &c.mentions)
    }

    /// Le motif de recours `code` appelle-t-il le nom et la qualification de la personne remplacee ? (L1242-12, 1° :
    /// le CDD conclu au titre du 1° de L1242-2.)
    pub fn motif_de_remplacement(&self, code: &str) -> bool {
        self.motifs_recours().iter().any(|m| m.code == code && m.personne_remplacee)
    }

    /// Le libelle du motif de recours `code`, tel que `contrats-base.json` le dit.
    pub fn libelle_motif_recours(&self, code: &str) -> Option<String> {
        self.motifs_recours().iter().find(|m| m.code == code).map(|m| m.libelle.clone())
    }

    /// La valeur fixe d'une rubrique pour cette nature, ou `None` (absente comme interdite : `interdit` les distingue).
    pub fn code(&self, rubrique: &str) -> Option<String> {
        self.codes.get(rubrique).map(|c| c.valeur.clone())
    }

    /// Le controle qui refuse cette rubrique pour cette nature, ou `None`.
    pub fn interdit(&self, rubrique: &str) -> Option<String> {
        self.interdits.get(rubrique).cloned()
    }

    /// Cette nature ecrit-elle cette rubrique ? (valeur fixe ou champ)
    pub fn ecrit(&self, rubrique: &str) -> bool {
        self.codes.contains_key(rubrique) || self.champs.contains_key(rubrique)
    }

    /// Le motif de rupture quand le contrat va a son terme (`None` pour un CDI).
    pub fn motif_rupture_par_defaut(&self) -> Option<String> {
        self.fin_de_contrat.motif_par_defaut.as_ref().map(|m| m.valeur.clone())
    }

    pub fn motifs_rupture(&self) -> &[MotifDeRupture] {
        &self.fin_de_contrat.motifs
    }

    /// Le refus d'un motif de rupture (`S21.G00.62.002`) que cette nature ne declare pas, ou `None`. Les `motifs` de
    /// `contrats-base.json` sont ceux que la CCH-11 de la rubrique autorise pour la nature (cahier technique DSN
    /// 2026.1, p. 270-273, `norme-dsn/texte/bloc-S21.G00.62.txt`) : un motif hors de la liste serait rejete (091 et 092,
    /// l'inaptitude, ne valent que pour les natures 01, 07, 08, 09, 50, 53, 54, 82, 91 ; un CDD rompu pour inaptitude
    /// est en 033). Le CDDU, de nature 02, a la liste du CDD. Une nature sans liste n'est pas controlee.
    pub fn motif_rupture_refuse(&self, motif: &str, numero: &str) -> Option<String> {
        let motifs = self.motifs_rupture();
        if motifs.is_empty() || motifs.iter().any(|m| m.code == motif) {
            return None;
        }
        let connus: Vec<String> = motifs.iter().map(|m| format!("{} ({})", m.code, m.libelle)).collect();
        Some(format!(
            "Contrat {} ({}) : le motif de rupture {} n'est pas autorise pour cette nature de contrat (CCH-11 de S21.G00.62.002, cahier technique DSN p. 270-273) ; la declaration serait rejetee. Motifs : {}. Rien n'est ecrit.",
            numero,
            self.libelle,
            motif,
            connus.join(", ")
        ))
    }

    /// Les motifs de recours (S21.G00.40.021) que cette nature accepte.
    pub fn motifs_recours(&self) -> &[MotifDeRecours] {
        self.champs.get(MOTIF_DE_RECOURS).map_or(&[], |c| &c.valeurs)
    }

    /// Le refus d'un motif de recours saisi (`S21.G00.40.021`) que cette nature n'accepte pas, ou `None`. La rubrique
    /// est une liste de codes de deux caracteres (cahier technique DSN 2026.1, p. 194-195,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`) ; la nature en propose une partie (`valeurs` de
    /// `contrats-base.json`). Une nature sans liste (le CDDU, qui fixe le sien ; le CDI) n'est pas controlee.
    pub fn motif_recours_refuse(&self, motif: &str, qui: &str) -> Option<String> {
        let motifs = self.motifs_recours();
        if motif.is_empty() || motifs.is_empty() || motifs.iter().any(|m| m.code == motif) {
            return None;
        }
        let connus: Vec<String> = motifs.iter().map(|m| format!("{} ({})", m.code, m.libelle)).collect();
        Some(format!(
            "« {} » n'est pas un motif de recours d'un {} ({}) ; la DSN le déclare par son code (S21.G00.40.021). Choisir l'un de : {}. Rien n'est écrit.",
            motif,
            self.libelle,
            qui,
            connus.join(", ")
        ))
    }

    pub fn motif_recours_exige(&self) -> bool {
        self.champs.get(MOTIF_DE_RECOURS).is_some_and(|c| c.obligatoire)
    }

    /// La fin de ce contrat, pour ce motif de rupture (S21.G00.62.002 ; `None` ou vide : le defaut de la nature),
    /// part-elle par le circuit derogatoire ? Pour un CDDU, le seul 031 (CCH-12 de S21.G00.62.017, fiche 2062).
    pub fn circuit_derogatoire_pour(&self, motif: Option<&str>) -> bool {
        let Some(c) = &self.fin_de_contrat.circuit_derogatoire else { return false };
        let motif = motif.filter(|m| !m.is_empty()).map(str::to_string).or_else(|| self.motif_rupture_par_defaut());
        motif.is_some_and(|m| c.motifs.contains(&m))
    }

    /// LA REGLE, ET SON SEUL ENDROIT : la fin de ce contrat, pour ce motif de rupture (`None` ou vide : le defaut de la
    /// nature), se declare-t-elle par un signalement de fin de contrat (nature 07) ? Toute fin de contrat donne lieu a
    /// l'attestation employeur (R. 1234-9), par le FCTU en DSN (fiche net-entreprises 119) ; le CDDU n'en est dispense
    /// que par le circuit derogatoire, pour le 031 : « Tous les autres motifs de rupture doivent etre declares par
    /// l'emission d'un signalement FCTU » (fiche 2062, `atelier/reference/sources/net-entreprises-base-connaissances/
    /// net-entreprises-fiche2062-derogations-fin-de-contrat-fctu-20250905.txt`). Les listes des fins a signaler (ligne de
    /// commande, page, rappels), le FCTU et l'ecran de saisie la lisent ici.
    pub fn signalement_fctu(&self, motif: Option<&str>) -> bool {
        self.fin_de_contrat.signalement_fctu.valeur && !self.circuit_derogatoire_pour(motif)
    }

    /// Les rubriques du bloc 62 que la nature fixe, pour ce motif de rupture : rubrique -> valeur. La modalite de
    /// declaration de la fin d'un contrat d'usage (S21.G00.62.017) suit le motif : « 01 » au circuit derogatoire,
    /// « 02 » hors de lui (CCH-11 et CCH-12, cahier technique DSN 2026.1 p. 277, `atelier/reference/norme-dsn/texte/
    /// bloc-S21.G00.62.txt`).
    pub fn codes_fin(&self, motif: Option<&str>) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        if let Some(c) = &self.fin_de_contrat.circuit_derogatoire {
            let valeur = if self.circuit_derogatoire_pour(motif) { &c.applique } else { &c.non_applique };
            out.insert(c.rubrique.clone(), valeur.clone());
        }
        out
    }

    /// C_TYPE_CONTRAT du Format 120 : 1 CDD, 2 CDI, 3 CTT.
    pub fn type_contrat_dpae(&self) -> String {
        self.dpae.type_contrat.valeur.clone()
    }

    /// La DPAE de ce contrat porte-t-elle une date de fin ?
    pub fn date_fin_dpae(&self) -> bool {
        self.dpae.date_fin.obligatoire
    }

    pub fn types_remuneration(&self) -> &[TypeDeRemuneration] {
        &self.remunerations.types
    }

    /// Les quatre blocs 51 valent-ils tous le brut ? (vrai sur un CDDU)
    pub fn remunerations_au_brut(&self) -> bool {
        self.remunerations.toutes_au_brut
    }

    pub fn unites_activite(&self) -> &[UniteDActivite] {
        &self.activites.unites
    }

    pub fn unite_absence(&self) -> Option<&UniteDAbsence> {
        self.activites.absences.as_ref()
    }
}

// ------------------------------------ l'abattement << contrats courts >> du PAS

pub const DUREE_CONTRAT_COURT_MOIS: u32 = 2;

fn jours_du_mois(an: i32, mois: u32) -> u32 {
    match mois {
        4 | 6 | 9 | 11 => 30,
        2 if (an % 4 == 0 && an % 100 != 0) || an % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// La meme quantieme, n mois plus tard, ramenee au dernier jour du mois.
fn dans_n_mois(depuis: Jour, mois: u32) -> Jour {
    let rang = depuis.1 - 1 + mois;
    let an = depuis.0 + (rang / 12) as i32;
    let m = rang % 12 + 1;
    (an, m, depuis.2.min(jours_du_mois(an, m)))
}

/// L'abattement contrats courts s'applique-t-il a ce versement ? `debut` et
/// `fin` sont les bornes du CONTRAT SIGNE (`fin` a `None` : terme imprecis),
/// `le` la date du versement.
pub fn contrat_court(nat: &Nature, debut: Jour, fin: Option<Jour>, le: Jour) -> bool {
    if nat.duree_indeterminee {
        return false;
    }
    let terme = dans_n_mois(debut, DUREE_CONTRAT_COURT_MOIS);
    if let Some(f) = fin {
        if f > terme {
            return false;
        }
    }
    le <= terme
}

// ------------------------------------------------------------------- lecture

/// LE FICHIER `contrats-base.json`, tel qu'il se lit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fichier {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    #[serde(rename = "source")]
    _source: crate::source::Fondement,
    categories_d_emploi: CategoriesDEmploi,
    /// Dans l'ordre du fichier.
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    natures: Vec<(String, Nature)>,
    defaut: Defaut,
}

/// La nature d'un engagement qui ne dit pas la sienne.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Defaut {
    nature: String,
    #[serde(rename = "note")]
    _note: String,
}

/// `categories_d_emploi` de `contrats-base.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoriesDEmploi {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    /// Le libelle de la norme, par classement conventionnel (S21.G00.40.002).
    par_classement: BTreeMap<String, String>,
    artiste_non_cadre: ArtisteNonCadre,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtisteNonCadre {
    libelle: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Les natures de contrat, construites : dans l'ordre du fichier, la nature par defaut, les categories d'emploi.
struct Contrats {
    natures: Arc<Vec<Arc<Nature>>>,
    defaut: String,
    categories: CategoriesDEmploi,
}

const ILLISIBLE: &str = "Les natures de contrat livrées avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE DE `contrats-base.json` : sa forme, puis ce qu'elle ne dit pas seule (le type de contrat de la DPAE
/// est l'un des trois du Format 120 ; la nature par defaut est l'une des natures).
fn lu() -> Resultat<Arc<Contrats>> {
    static CACHE: OnceLock<FormeGardee<Contrats>> = OnceLock::new();
    forme_en_cache(&CACHE, "contrats-base.json", ILLISIBLE, |f: Fichier| {
        let mut natures = Vec::new();
        for (cle, mut n) in f.natures {
            if !TYPES_CONTRAT_DPAE.iter().any(|(k, _)| *k == n.dpae.type_contrat.valeur) {
                return Err(socle::echec::a_la_cle(ILLISIBLE, &format!("natures.{}.dpae.type_contrat.valeur", cle), format!("{} n'est pas un type de contrat du Format 120 (1, 2 ou 3)", repr(&n.dpae.type_contrat.valeur))));
            }
            n.cle = cle;
            natures.push(Arc::new(n));
        }
        if !natures.iter().any(|n| n.cle == f.defaut.nature) {
            return Err(socle::echec::a_la_cle(ILLISIBLE, "defaut.nature", format!("{} n'est pas une nature du fichier", repr(&f.defaut.nature))));
        }
        Ok(Contrats { natures: Arc::new(natures), defaut: f.defaut.nature, categories: f.categories_d_emploi })
    })
}

/// La categorie d'emploi d'un classement conventionnel (S21.G00.40.002), telle que le contrat ecrit l'imprime
/// (`categories_d_emploi` de `contrats-base.json`, libelles de la norme ; R1221-34, 3°). L'`artiste` qui n'est pas cadre
/// (03, 04) : « non-cadre », la categorie de sa convention (3090, art. 6.3 ; 1285, art. XI.2.1), pas le libelle du 05.
pub fn categorie_d_emploi(classement: &str, artiste: bool) -> Resultat<Option<String>> {
    let c = &lu()?.categories;
    if artiste && !matches!(classement, "03" | "04") {
        return Ok(Some(c.artiste_non_cadre.libelle.clone()));
    }
    Ok(c.par_classement.get(classement).cloned())
}

/// La nature d'un engagement qui ne dit pas la sienne : le CDDU.
pub fn defaut() -> Resultat<String> {
    Ok(lu()?.defaut.clone())
}

/// Toutes les natures, dans l'ordre du fichier.
pub fn natures() -> Resultat<Arc<Vec<Arc<Nature>>>> {
    Ok(lu()?.natures.clone())
}

/// Une nature par sa cle (`None` : la nature par defaut). Refuse une cle inconnue.
pub fn nature(cle: Option<&str>) -> Resultat<Arc<Nature>> {
    let lus = lu()?;
    let cle = match cle {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => lus.defaut.clone(),
    };
    if let Some(n) = lus.natures.iter().find(|n| n.cle == cle) {
        return Ok(n.clone());
    }
    let mut connues: Vec<&str> = lus.natures.iter().map(|n| n.cle.as_str()).collect();
    connues.sort();
    Err(format!("Nature de contrat inconnue : {}. Connues : {}.", repr(&cle), connues.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CCH-18 du 40.005 (cahier technique 2026.1, p. 185, `bloc-S21.G00.40.txt` l. 325-329) : « 999SPT pour les emplois
    /// permanents du spectacle » ; un CDDU garde le code de la nomenclature.
    #[test]
    fn la_nature_fixe_999spt_pour_les_permanents_seulement() {
        for cle in ["cdi", "cdd"] {
            assert_eq!(nature(Some(cle)).unwrap().code("S21.G00.40.005").as_deref(), Some("999SPT"), "{}", cle);
        }
        assert_eq!(nature(Some("cddu_spectacle")).unwrap().code("S21.G00.40.005"), None);
    }
}
