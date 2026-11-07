//! LES ORGANISMES ET LES CONTRATS DE LA DSN d'une compagnie, composes a une date (`Contexte::organismes`) : ce que la
//! DSN, les signalements, l'amorcage et le contrat ecrit lisent des organismes.
//!
//! Tout se compose a la date lue, depuis les textes livres et la configuration de cette date :
//! - la partie normative (organismes payes, CTP, bloc 08) vient de `donnees/organismes-base.json` (`Base`), la meme
//!   pour toutes ;
//! - les contrats attendus aussi (`contrats` : ceux de toutes les compagnies, puis ceux de la convention en vigueur a
//!   cette date), plus la CASC-SVP (`casc-svp-base.json`, `adhesions::casc_svp`) et les contributions de branche en DSN
//!   (`contributions_dsn` du bareme de cette date) ;
//! - de la compagnie, seulement ce qui est a elle (`config/organismes.json`, ecrit par Configuration › Organismes et
//!   contrats) : la reference de ses contrats chez l'organisme quand aucun texte ne la donne, l'organisme quand la
//!   branche n'en designe pas, et les identifiants d'adhesion deja attribues.
//!
//! Une compagnie qui n'a rien a saisir (les intermittents d'une 3090 : tous leurs contrats sont donnes par les textes)
//! n'a donc rien a enregistrer. Ce qui manque ne se devine pas : un contrat des permanents sans reference laisse les
//! references des permanents vides (`ParNature`), et la DSN refuse le permanent (`dsn::affiliations`), pas la compagnie.
//!
//! `config/organismes.json` est DATE comme toute la configuration (`fiche::dater`) : sa cle
//! `adhesions_prevoyance` est une liste `{depuis, valeur}`. Les identifiants d'adhesion (S21.G00.15.005) ne bougent
//! jamais : relus dans le fichier, et un identifiant nouveau n'en reprend aucun qu'une autre periode porte.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::num::Resultat;

use crate::contexte::{Contexte, ORGANISMES};
use crate::fiche::Datee;
use crate::source::{forme_notee, Fondement};

/// Audiens, delegataire des contrats de branche du spectacle (S21.G00.15.002).
const AUDIENS: &str = "P0983";
/// Le premier identifiant d'adhesion libre : ceux d'en dessous sont ceux que les textes donnent (`reference` des
/// contrats de `organismes-base.json`, `identifiant` de la CASC-SVP).
const PREMIER_LIBRE: u32 = 104;

// ------------------------------------------------------------------------------------------ la partie normative

forme_notee! {
    /// `donnees/organismes-base.json`, entier.
    pub struct Base {
        pub versements: Vec<Versement>,
        pub sans_individu: SansIndividu,
        pub ctp_urssaf: TableCtp,
        pub contrats: Contrats,
    }
}

forme_notee! {
    /// UN ORGANISME QUE LA DSN PAIE (bloc S21.G00.20) : son identifiant (20.001), son libelle, son nom propre, ce que
    /// couvre son versement, le jour du mois suivant ou il preleve quand un texte le dit, ses cles de totalisation (les
    /// codes 81 qu'il recoit), et son adresse quand le contrat ecrit la nomme.
    pub struct Versement {
        pub siret: String,
        pub libelle: String,
        pub nom: String,
        pub objet: String,
        pub jour_de_paiement: Option<u32>,
        pub codes81: Vec<String>,
        pub adresse: Option<String>,
    }
}

impl Versement {
    /// L'Urssaf : l'organisme du bordereau et des CTP, reconnu a son libelle.
    pub fn est_urssaf(&self) -> bool {
        self.libelle == "URSSAF"
    }
}

forme_notee! {
    /// Le bloc S20.G00.08 d'un mois sans individu : les organismes qui n'ont plus d'individu rattache.
    pub struct SansIndividu {
        pub absence_de_rattachement: Vec<String>,
    }
}

forme_notee! {
    /// La table des CTP du bordereau Urssaf (bloc S21.G00.23).
    pub struct TableCtp {
        pub lignes: Vec<Ctp>,
    }
}

/// Le taux AT/MP d'une ligne du bordereau : celui du bareme du mois declare.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TauxAt {
    Bareme,
}

forme_notee! {
    /// UNE LIGNE DU BORDEREAU URSSAF : le CTP (23.001), son qualifiant d'assiette (23.002), ce qu'il lit (une assiette,
    /// ou un montant), son taux AT/MP, la population qui l'ecrit (`statut`, `nature`) et les seuils d'effectif.
    pub struct Ctp {
        pub code: String,
        pub qualifiant: String,
        pub assiette: Option<String>,
        pub montant: Option<String>,
        pub taux_at: Option<TauxAt>,
        pub statut: Option<String>,
        pub nature: Option<String>,
        pub effectif_au_moins: Option<i64>,
        pub effectif_sous: Option<i64>,
    }
}

forme_notee! {
    /// Les contrats que les textes exigent : ceux de toute compagnie, puis ceux de chaque convention (par IDCC).
    pub struct Contrats {
        pub tous: Vec<Contrat>,
        pub conventions: BTreeMap<String, Vec<Contrat>>,
    }
}

/// UN CONTRAT QU'UN TEXTE EXIGE (bloc S21.G00.15) : sa cle, son role pour un permanent (`prevoyance`, `sante`), sa
/// reference chez l'organisme quand le texte la donne (15.001), l'identifiant de l'adhesion quand il est fixe (15.005),
/// son type (15.003), ses dates, qui il couvre, l'organisme (15.002) et s'il est designe, s'il n'est exige qu'avec un
/// permanent, et les options que l'organisme demande. `quoi` le nomme a l'ecran, `note` s'y lit sous lui ; `source` le
/// fonde.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contrat {
    pub cle: String,
    pub role: Option<String>,
    pub libelle: Option<String>,
    pub reference: Option<String>,
    pub r#type: String,
    pub depuis: Option<Jour>,
    pub jusqu_a: Option<Jour>,
    pub beneficiaires: String,
    pub quoi: String,
    pub organisme: Option<String>,
    pub organisme_fixe: bool,
    pub si_permanents: Option<bool>,
    pub options: Option<Options>,
    pub note: Option<String>,
    pub source: Option<Fondement>,
}

forme_notee! {
    /// Les codes option (S21.G00.70.004) que l'organisme demande sous ce contrat, par niveau de garantie ; `regime_local` :
    /// ils valent aussi pour le regime local d'Alsace-Moselle.
    pub struct Options {
        pub regime_local: bool,
        pub niveaux: BTreeMap<String, Niveau>,
    }
}

/// Les codes option d'un niveau : seul (`isole`), ou avec un conjoint ou des enfants (`famille`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Niveau {
    pub isole: String,
    pub famille: String,
}

/// LA LECTURE DE `donnees/organismes-base.json`, la seule, une fois tant que le fichier ne change pas.
pub fn base() -> Resultat<Arc<Base>> {
    static CACHE: OnceLock<FormeGardee<Base>> = OnceLock::new();
    forme_en_cache(&CACHE, "organismes-base.json", "La table livrée des organismes ne se lit pas : le signaler au développeur.", Ok)
}

impl Base {
    /// Tous les contrats que les textes donnent, toutes conventions confondues.
    fn tous_les_contrats(&self) -> impl Iterator<Item = &Contrat> {
        self.contrats.tous.iter().chain(self.contrats.conventions.values().flatten())
    }

    /// Les contrats de toute compagnie, puis ceux de la convention `idcc`.
    pub fn contrats_de(&self, idcc: &str) -> impl Iterator<Item = &Contrat> {
        self.contrats.tous.iter().chain(self.contrats.conventions.get(idcc).into_iter().flatten())
    }

    /// Les options que l'organisme demande sous le contrat de reference `libelle` (S21.G00.15.001), pour la convention
    /// `idcc`.
    pub fn options(&self, idcc: &str, libelle: &str) -> Option<&Options> {
        self.contrats_de(idcc).filter(|c| c.libelle.as_deref().map(str::trim) == Some(libelle)).find_map(|c| c.options.as_ref())
    }

    /// Le versement de l'organisme `siret` (S21.G00.20.001).
    pub fn versement(&self, siret: &str) -> Option<&Versement> {
        self.versements.iter().find(|v| v.siret == siret)
    }
}

// ------------------------------------------------------------------------------------------ ce que la compagnie saisit

/// `config/organismes.json` : ce que la compagnie a saisi (Configuration › Organismes et contrats), date. Ecrit par
/// `moteur::compagnie::organismes`, lu par `saisis`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichierOrganismes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adhesions_prevoyance: Option<Vec<Datee<Vec<Adhesion>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoi: Option<String>,
}

/// UNE ADHESION DE PREVOYANCE OU DE SANTE (bloc S21.G00.15) : le contrat (`cle`), sa reference (15.001), l'organisme
/// (15.002), son type (15.003), son identifiant (15.005), et ses dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adhesion {
    pub cle: String,
    pub libelle: String,
    pub delegataire: String,
    pub r#type: String,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depuis: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jusqu_a: Option<Jour>,
}

impl Adhesion {
    /// L'adhesion vaut-elle pour la periode `debut`..=`fin` (`adhesions::en_vigueur`) ?
    pub fn en_vigueur(&self, debut: &Jour, fin: &Jour) -> bool {
        crate::adhesions::en_vigueur(self.depuis, self.jusqu_a, debut, fin)
    }
}

/// Ce que l'ecran saisit d'un contrat (`organismes-enregistrer`, `contrats` : cle -> saisie) : sa reference chez
/// l'organisme et le code de l'organisme.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saisie {
    pub libelle: Option<String>,
    pub organisme: Option<String>,
}

/// LA LECTURE DE `config/organismes.json`, la seule ; absent, il ne dit rien.
pub fn fichier_organismes(ctx: &Contexte) -> Resultat<FichierOrganismes> {
    let p = ctx.racine.join(socle::fichiers::CONFIG).join(ORGANISMES);
    if !p.is_file() {
        return Ok(FichierOrganismes::default());
    }
    crate::fiche::lire_fichier(&p, &format!("config/{}", ORGANISMES))
}

/// Le numero d'une reference d'adhesion (S21.G00.15.001) : une reference qui n'en est pas un se refuse.
fn numero_de_reference(reference: &str) -> Resultat<u32> {
    reference.trim().parse::<u32>().map_err(|_| {
        format!("La référence d'adhésion « {} » (S21.G00.15.001) n'est pas un nombre : la corriger dans les organismes de la compagnie.", reference)
    })
}

/// Ce que la compagnie a saisi, a la date du contexte (rien s'il n'y a rien), et les identifiants d'adhesion qu'une
/// periode quelconque du fichier porte. Le fichier ne porte que ce qu'aucun texte ne donne : de lui ne se lisent que les
/// adhesions, et une adhesion qui recopie un contrat qu'un texte donne entier (`donnes_par_un_texte`) n'est pas une
/// saisie, sous quelque convention qu'on lise. Son identifiant reste pris : il a pu partir dans une DSN.
pub fn saisis(ctx: &Contexte) -> Resultat<(Vec<Adhesion>, Vec<u32>)> {
    let fichier = fichier_organismes(ctx)?;
    let Some(liste) = fichier.adhesions_prevoyance.as_deref() else { return Ok((Vec::new(), Vec::new())) };
    let prises = liste
        .iter()
        .flat_map(|e| e.valeur.dite().into_iter().flatten())
        .map(|a| numero_de_reference(&a.reference))
        .collect::<Resultat<Vec<u32>>>()?;
    let au = crate::fiche::valeur_au(liste,&ctx.lue_au);
    let textes = donnes_par_un_texte()?;
    let adhesions: Vec<Adhesion> = au
        .dite()
        .into_iter()
        .flatten()
        .filter(|a| {
            let libelle = a.libelle.trim().to_uppercase();
            let contribution = a.contribution.as_deref().unwrap_or("").trim().to_uppercase();
            !textes.contains(&libelle) && !(!contribution.is_empty() && libelle == contribution)
        })
        .cloned()
        .collect();
    Ok((adhesions, prises))
}

/// Les references de contrat (S21.G00.15.001) qu'un texte livre donne entieres, toutes conventions confondues : les
/// `libelle` des contrats de `organismes-base.json` et celui de la CASC-SVP (`casc-svp-base.json`). Une contribution de
/// branche se reconnait a son nom (`saisis`).
fn donnes_par_un_texte() -> Resultat<Vec<String>> {
    let base = base()?;
    let mut out: Vec<String> =
        base.tous_les_contrats().filter_map(|c| c.libelle.as_deref()).map(|l| l.trim().to_uppercase()).filter(|l| !l.is_empty()).collect();
    let l = crate::adhesions::casc_svp_base()?.dsn.contrat.trim().to_uppercase();
    if !l.is_empty() {
        out.push(l);
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------ les contrats attendus

/// UN CONTRAT ATTENDU d'une compagnie a une date : un contrat qu'un texte exige (`Contrat`), la CASC-SVP, ou une
/// contribution de branche du bareme. `libelle_defaut` : la reference que l'ecran propose quand rien n'est saisi.
#[derive(Debug, Clone, Default)]
pub struct Attendu {
    pub cle: String,
    pub role: Option<String>,
    pub libelle: Option<String>,
    pub libelle_defaut: Option<String>,
    pub reference: Option<String>,
    pub r#type: String,
    pub depuis: Option<Jour>,
    pub jusqu_a: Option<Jour>,
    pub beneficiaires: String,
    pub quoi: String,
    pub organisme: Option<String>,
    pub organisme_fixe: bool,
    pub si_permanents: bool,
    pub contribution: Option<String>,
    pub note: Option<String>,
}

impl From<&Contrat> for Attendu {
    fn from(c: &Contrat) -> Attendu {
        Attendu {
            cle: c.cle.clone(),
            role: c.role.clone(),
            libelle: c.libelle.clone(),
            reference: c.reference.clone(),
            r#type: c.r#type.clone(),
            depuis: c.depuis,
            jusqu_a: c.jusqu_a,
            beneficiaires: c.beneficiaires.clone(),
            quoi: c.quoi.clone(),
            organisme: c.organisme.clone(),
            organisme_fixe: c.organisme_fixe,
            si_permanents: c.si_permanents == Some(true),
            note: c.note.clone(),
            ..Attendu::default()
        }
    }
}

/// Le texte d'un champ facultatif, sans ses blancs (vide s'il manque).
fn net(s: &Option<String>) -> String {
    s.as_deref().unwrap_or_default().trim().to_string()
}

impl Attendu {
    /// La reference donnee par le texte, sans ses blancs (vide s'il n'en donne pas).
    pub fn libelle_donne(&self) -> String {
        net(&self.libelle)
    }

    /// Le contrat vaut-il pour la periode (`adhesions::en_vigueur`) ?
    pub fn couvre(&self, (debut, fin): &(Jour, Jour)) -> bool {
        crate::adhesions::en_vigueur(self.depuis, self.jusqu_a, debut, fin)
    }
}

/// Les contrats que cette compagnie declare a la date du contexte, dans l'ordre ou ils se presentent.
pub fn attendus(ctx: &Contexte) -> Resultat<Vec<Attendu>> {
    let idcc = ctx.entreprise.idcc.texte().trim().to_string();
    let base = base()?;
    let mut out: Vec<Attendu> = base.contrats.tous.iter().map(Attendu::from).collect();
    if let Some(casc) = crate::adhesions::casc_svp(&ctx.entreprise)? {
        out.push(Attendu {
            cle: "casc".into(),
            libelle: Some(casc.libelle),
            reference: Some(casc.reference),
            r#type: casc.r#type,
            beneficiaires: "tous les salariés".into(),
            quoi: "CASC-SVP (activités sociales et culturelles)".into(),
            organisme: Some(casc.delegataire),
            organisme_fixe: true,
            ..Attendu::default()
        });
    }
    // Les contributions de branche en DSN : la convention de la date dit lesquelles (`Contexte::contributions_dsn`).
    for c in ctx.contributions_dsn(&ctx.lue_au)? {
        let nom = c.contribution.clone();
        if nom.is_empty() {
            continue;
        }
        out.push(Attendu {
            cle: format!("contribution_{}", nom.to_lowercase()),
            contribution: Some(nom.clone()),
            libelle_defaut: Some(nom.clone()),
            r#type: "01".into(),
            beneficiaires: "selon le contrat".into(),
            quoi: format!("Contribution {} (en DSN depuis {})", nom, c.en_dsn_depuis),
            organisme: Some(AUDIENS.into()),
            organisme_fixe: true,
            ..Attendu::default()
        });
    }
    out.extend(base.contrats.conventions.get(&idcc).into_iter().flatten().map(Attendu::from));
    Ok(out)
}

/// L'adhesion saisie qui tient ce contrat : par sa cle, son identifiant fixe, ou sa contribution.
fn en_place<'a>(spec: &Attendu, saisi: &'a [Adhesion]) -> Option<&'a Adhesion> {
    if let Some(a) = saisi.iter().find(|a| a.cle.trim() == spec.cle.trim()) {
        return Some(a);
    }
    let reference = net(&spec.reference);
    if !reference.is_empty() {
        return saisi.iter().find(|a| a.reference.trim() == reference);
    }
    let contribution = net(&spec.contribution).to_uppercase();
    if contribution.is_empty() {
        return None;
    }
    saisi.iter().find(|a| net(&a.contribution).to_uppercase() == contribution || a.libelle.trim().to_uppercase() == contribution)
}

/// UNE LIGNE DE L'ECRAN (`organismes-contrats`) : ce que le contrat couvre, et ce qui vaut (le texte quand il le donne,
/// sinon la saisie).
#[derive(Debug, Clone, Serialize)]
pub struct Ligne {
    pub cle: String,
    pub quoi: String,
    pub beneficiaires: String,
    pub libelle: String,
    pub libelle_fixe: bool,
    pub organisme: String,
    pub organisme_fixe: bool,
    pub si_permanents: bool,
    pub depuis: String,
    pub jusqu_a: String,
    pub note: String,
}

/// La ligne de l'ecran d'un contrat attendu, sous ce que la compagnie a saisi.
pub fn ligne(spec: &Attendu, saisi: &[Adhesion]) -> Ligne {
    let place = en_place(spec, saisi);
    let libelle_fixe = !spec.libelle_donne().is_empty();
    let libelle = if libelle_fixe {
        spec.libelle_donne()
    } else {
        place.map(|a| a.libelle.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| net(&spec.libelle_defaut))
    };
    let organisme = if spec.organisme_fixe {
        net(&spec.organisme)
    } else {
        place.map(|a| a.delegataire.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| net(&spec.organisme))
    };
    Ligne {
        cle: spec.cle.trim().to_string(),
        quoi: spec.quoi.trim().to_string(),
        beneficiaires: spec.beneficiaires.trim().to_string(),
        libelle,
        libelle_fixe,
        organisme,
        organisme_fixe: spec.organisme_fixe,
        si_permanents: spec.si_permanents,
        depuis: spec.depuis.map(|j| j.iso()).unwrap_or_default(),
        jusqu_a: spec.jusqu_a.map(|j| j.iso()).unwrap_or_default(),
        note: net(&spec.note),
    }
}

// ------------------------------------------------------------------------------------------ la composition

/// Ce qu'une nature de contrat rattache (S21.G00.70.013) : les identifiants de ses adhesions, dans l'ordre que le
/// socle attend (pour un permanent, la prevoyance puis la sante), ou pourquoi elles manquent.
#[derive(Debug, Clone, PartialEq)]
pub struct Rattachement {
    pub references: Vec<String>,
    pub a_confirmer: Option<&'static str>,
}

/// Les rattachements de chaque nature : les intermittents, les permanents.
#[derive(Debug, Clone, PartialEq)]
pub struct ParNature {
    pub intermittent: Rattachement,
    pub permanent: Rattachement,
}

impl ParNature {
    /// Les natures, dans l'ordre : `intermittent`, `permanent`.
    pub fn natures(&self) -> [(&'static str, &Rattachement); 2] {
        [("intermittent", &self.intermittent), ("permanent", &self.permanent)]
    }

    /// Le rattachement de la nature `cle` (`Nature::bareme`), s'il est compose.
    pub fn de(&self, cle: &str) -> Option<&Rattachement> {
        self.natures().into_iter().find(|(n, _)| *n == cle).map(|(_, r)| r)
    }
}

/// Pourquoi les permanents n'ont pas de rattachement.
const PERMANENTS_A_SAISIR: &str = "La prévoyance et la santé des permanents ne sont pas toutes deux saisies (Configuration › \
Organismes et contrats) : un permanent est refusé tant qu'elles manquent.";

/// LES ORGANISMES COMPOSES d'une compagnie a une date : la partie normative (`base`), ses adhesions (bloc S21.G00.15,
/// dans l'ordre de leurs identifiants) et ce que chaque nature rattache.
#[derive(Debug, Clone)]
pub struct Organismes {
    pub base: Arc<Base>,
    pub adhesions_prevoyance: Vec<Adhesion>,
    pub par_nature: ParNature,
}

impl Organismes {
    /// Les organismes que la DSN paie (bloc S21.G00.20).
    pub fn versements(&self) -> &[Versement] {
        &self.base.versements
    }

    /// L'Urssaf, l'organisme du bordereau.
    pub fn urssaf(&self) -> Option<&Versement> {
        self.base.versements.iter().find(|v| v.est_urssaf())
    }

    /// La table des CTP du bordereau.
    pub fn ctp_urssaf(&self) -> &[Ctp] {
        &self.base.ctp_urssaf.lignes
    }

    /// Les organismes du bloc S20.G00.08 d'un mois sans individu.
    pub fn absence_de_rattachement(&self) -> &[String] {
        &self.base.sans_individu.absence_de_rattachement
    }

    /// L'adhesion d'identifiant `reference` (S21.G00.15.005).
    pub fn adhesion(&self, reference: &str) -> Option<&Adhesion> {
        self.adhesions_prevoyance.iter().find(|a| a.reference == reference)
    }

    /// L'adhesion `reference` vaut-elle pour la periode `debut`..=`fin` ? Une reference qu'aucune adhesion ne porte (une
    /// contribution de branche du bareme) n'a pas de date.
    pub fn adhesion_en_vigueur(&self, reference: &str, debut: &Jour, fin: &Jour) -> bool {
        self.adhesion(reference).map_or(true, |a| a.en_vigueur(debut, fin))
    }
}

/// Compose les organismes a la date du contexte depuis la base, les contrats attendus, ce que la compagnie a saisi
/// (`saisi` : le fichier a cette date ; `prises` : les identifiants que le fichier porte) et ce qu'elle saisit
/// maintenant (`saisies` : cle -> saisie). Chaque contrat garde ses dates (`depuis`, `jusqu_a`) : la DSN de chaque mois
/// ne lit que ceux qui le couvrent.
///
/// `exiger` : la periode que l'ecran enregistre ; un contrat qu'il propose pour elle et dont la reference manque se
/// refuse. Sans elle (la lecture), un contrat sans reference ne se declare pas, et ce qui en depend se refuse plus loin.
pub fn composer(
    ctx: &Contexte,
    saisi: &[Adhesion],
    mut prises: Vec<u32>,
    saisies: &BTreeMap<String, Saisie>,
    exiger: Option<&(Jour, Jour)>,
) -> Resultat<Organismes> {
    let mut retenus: Vec<(Attendu, Adhesion)> = Vec::new();
    for spec in attendus(ctx)? {
        let l = ligne(&spec, saisi);
        let maintenant = saisies.get(spec.cle.trim());
        let libelle = match maintenant {
            Some(s) if !l.libelle_fixe => net(&s.libelle).to_uppercase(),
            _ => l.libelle.clone(),
        };
        let organisme = match maintenant {
            Some(s) if !l.organisme_fixe => net(&s.organisme).to_uppercase(),
            _ => l.organisme.clone(),
        };
        let quoi = spec.quoi.trim().to_string();
        if libelle.is_empty() {
            // Un contrat vide ne se reclame pas quand l'ecran ne l'a pas propose : un contrat des permanents a une
            // compagnie qui n'en emploie pas, un contrat que la periode regardee ne couvre pas.
            let Some(periode) = exiger else { continue };
            if (spec.si_permanents && !ctx.emploie_des_permanents()) || !spec.couvre(periode) {
                continue;
            }
            return Err(format!(
                "{} : la référence du contrat manque. Elle se lit sur la fiche de paramétrage DSN de l'organisme \
(« référence contrat »).",
                quoi
            ));
        }
        // Les longueurs du cahier technique : S21.G00.15.001 de 1 a 30, S21.G00.15.002 de 5 a 9.
        if libelle.chars().count() > 30 {
            return Err(format!("{} : « {} » dépasse 30 caractères, ce que la DSN n'admet pas.", quoi, libelle));
        }
        let n = organisme.chars().count();
        if !(5..=9).contains(&n) || organisme.contains(char::is_whitespace) {
            return Err(format!(
                "{} : le code de l'organisme ({}) a de 5 à 9 caractères sans espace, comme P0983 pour Audiens. Il est \
sur le certificat d'adhésion et la fiche de paramétrage DSN.",
                quoi,
                if organisme.is_empty() { "vide".to_string() } else { format!("« {} »", organisme) }
            ));
        }
        let reference = match net(&spec.reference) {
            r if r.is_empty() => en_place(&spec, saisi).map(|a| a.reference.trim().to_string()).unwrap_or_default(),
            r => r,
        };
        if !reference.is_empty() {
            prises.push(numero_de_reference(&reference)?);
        }
        let a = Adhesion {
            cle: spec.cle.trim().to_string(),
            libelle,
            delegataire: organisme,
            r#type: spec.r#type.trim().to_string(),
            reference,
            contribution: Some(net(&spec.contribution)).filter(|c| !c.is_empty()),
            depuis: spec.depuis,
            jusqu_a: spec.jusqu_a,
        };
        retenus.push((spec, a));
    }
    // Les identifiants nouveaux, a la suite des pris : ceux deja attribues ne bougent pas.
    let mut libre = PREMIER_LIBRE;
    for (_, a) in retenus.iter_mut() {
        if a.reference.is_empty() {
            while prises.contains(&libre) {
                libre += 1;
            }
            prises.push(libre);
            a.reference = libre.to_string();
        }
    }
    let role = |r: &str| retenus.iter().find(|(s, _)| s.role.as_deref().map(str::trim) == Some(r)).map(|(_, a)| a.reference.clone());
    let permanent = match (role("prevoyance"), role("sante")) {
        (Some(p), Some(s)) => Rattachement { references: vec![p, s], a_confirmer: None },
        _ => Rattachement { references: Vec::new(), a_confirmer: Some(PERMANENTS_A_SAISIR) },
    };
    let intermittent: Vec<String> = ["tsintermittents", "interbranche"]
        .iter()
        .filter_map(|c| retenus.iter().find(|(s, _)| s.cle.trim() == *c).map(|(_, a)| a.reference.clone()))
        .collect();
    let mut adhesions = retenus
        .into_iter()
        .map(|(_, a)| Ok((numero_de_reference(&a.reference)?, a)))
        .collect::<Resultat<Vec<(u32, Adhesion)>>>()?;
    adhesions.sort_by_key(|(n, _)| *n);
    let adhesions: Vec<Adhesion> = adhesions.into_iter().map(|(_, a)| a).collect();
    Ok(Organismes {
        base: base()?,
        adhesions_prevoyance: adhesions,
        par_nature: ParNature { intermittent: Rattachement { references: intermittent, a_confirmer: None }, permanent },
    })
}
