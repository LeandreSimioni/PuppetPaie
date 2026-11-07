//! LA PREVOYANCE DE BRANCHE DES PERMANENTS : ce que la convention collective impose, lu dans
//! `donnees/prevoyance-conventionnelle-base.json` par sa forme (`Catalogue`), une fois tant que le fichier ne change
//! pas. Le catalogue porte le minimum de la branche ; ce que la compagnie souscrit se dit sur sa fiche
//! (`prevoyance_permanents`), et le moteur le confronte a ce minimum.

use super::{alsace_moselle, arrondi4, deux_virgule, nom_du_salarie, ChampsEntreprise, Decimal, Jour, Mutuelle, Resultat, SalarieDuContrat};
use crate::fiche::{ExtensionT2, Lue};
use socle::echec::APart;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::forme::Nombre;
use socle::num::div;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::source::forme_notee as forme;

forme! {
    /// `prevoyance-conventionnelle-base.json`, entier : le plancher national des cadres, et le regime de chaque
    /// convention relevee, dans l'ordre du fichier.
    pub struct Catalogue {
        pub obligation_cadre: ObligationCadre,
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub conventions: Vec<(String, Regime)>,
    }
}

forme! {
    /// Le plancher national des cadres, quelle que soit la convention : un taux de la tranche 1, et la part de
    /// l'employeur (l'accord du 17/11/2017, art. 1er, l'oblige a verser la cotisation : `source`), celle d'un cadre dont
    /// la convention n'a pas de bloc `obligatoire_cadre`.
    pub struct ObligationCadre {
        pub taux_t1: f64,
        pub part_patronale: f64,
    }
}

forme! {
    /// Le regime d'une convention collective : ses cotisations, qui en beneficie, ses options, sa sante.
    pub struct Regime {
        pub libelle: String,
        pub organisme: String,
        /// Le texte qui le porte, tel qu'un message le cite.
        pub texte: String,
        pub en_vigueur: Jour,
        pub etat: String,
        /// Faux : les taux sont releves, pas confirmes, et le moteur ne paie pas. Absent : confirme.
        pub applicable: Option<bool>,
        /// Ce qui reste a verifier d'un regime qui n'est pas applicable.
        pub a_confirmer: Option<String>,
        /// Le texte fixe la repartition, pas le taux : celui du contrat de l'entreprise, sur sa fiche.
        pub contrat_requis: Option<bool>,
        pub obligatoire: Bloc,
        pub obligatoire_cadre: Option<Bloc>,
        pub beneficiaires: Beneficiaires,
        pub options: Option<Options>,
        pub sante_minimum: Option<SanteMinimum>,
        pub financement_paritarisme: Option<Financement>,
        pub annexes: Option<BTreeMap<String, Annexe>>,
        pub sante: Option<Sante>,
    }
}

forme! {
    /// Une cotisation de prevoyance (tous, ou les cadres) : tranche 1, tranche 2, repartition, et ce qui change a une date.
    pub struct Bloc {
        pub taux: Option<f64>,
        /// L'assiette de la tranche 1 (celle du bloc `obligatoire` s'applique) et la part de l'employeur, que le catalogue
        /// dit pour chaque bloc.
        pub base: Option<String>,
        pub part_patronale: f64,
        pub t2: Option<f64>,
        pub t2_base: Option<String>,
        pub t2_part_patronale: Option<f64>,
        pub ventilation: Option<Ventilation>,
        pub a_partir_du: Option<BTreeMap<Jour, Changement>>,
    }
}

forme! {
    /// La part d'un taux par garantie.
    pub struct Ventilation {
        pub deces: f64,
        pub incapacite_invalidite: f64,
    }
}

forme! {
    /// Ce qu'un bloc prend a une date : ses nouveaux taux.
    pub struct Changement {
        pub taux: Option<f64>,
        pub t2: Option<f64>,
        pub ventilation: Option<Ventilation>,
    }
}

forme! {
    /// Qui beneficie du regime, et apres combien de mois d'anciennete dans la branche.
    pub struct Beneficiaires {
        /// Faux : la convention n'a pas de regime pour ses non cadres.
        pub non_cadres: Option<bool>,
        pub ouverture_cadre_mois: i64,
        pub ouverture_non_cadre_mois: i64,
    }
}

forme! {
    pub struct Options {
        pub extension_t2: Extension,
    }
}

forme! {
    /// L'extension facultative a la tranche 2 : son taux pour tous, ou pour les cadres seuls.
    pub struct Extension {
        pub tous: Option<f64>,
        pub cadres: Option<f64>,
        pub base: String,
        pub part_patronale: f64,
    }
}

/// Des valeurs datees : chacune vaut de son jour au suivant.
pub type Datees = BTreeMap<Jour, Nombre>;

forme! {
    /// La cotisation minimale de la sante des permanents, en % du plafond mensuel : le socle, et ce qu'une annexe impose.
    pub struct SanteMinimum {
        pub socle: Datees,
        pub annexes: Option<BTreeMap<String, Datees>>,
        pub part_patronale_minimum: Nombre,
    }
}

forme! {
    /// Une contribution annuelle de l'entreprise, hors DSN et hors bulletin (le FCAP-SVP de la 3090).
    pub struct Financement {
        pub taux: Nombre,
        pub assiette: String,
        pub plancher: Nombre,
        pub plafond: Nombre,
    }
}

forme! {
    /// Ce qu'une annexe de la convention ajoute a son regime.
    pub struct Annexe {
        pub taux: f64,
        pub base: String,
        pub part_patronale: f64,
    }
}

forme! {
    /// Le regime de sante de la branche : un taux au titre du seul salarie, et la part minimale de l'employeur.
    pub struct Sante {
        pub base: String,
        pub taux: Datees,
        pub taux_alsace_moselle: Option<Datees>,
        /// La part minimale de l'employeur (L. 911-7), que le catalogue dit pour chaque regime de sante.
        pub part_patronale_minimum: Nombre,
    }
}

/// Le catalogue livre.
pub fn catalogue() -> Resultat<Arc<Catalogue>> {
    static CACHE: OnceLock<FormeGardee<Catalogue>> = OnceLock::new();
    forme_en_cache(&CACHE, "prevoyance-conventionnelle-base.json", "Le catalogue de la prévoyance de branche livré ne se lit pas : le signaler au développeur.", Ok)
}

impl Catalogue {
    /// Le regime de la convention `idcc`, s'il est releve.
    pub fn regime(&self, idcc: &str) -> Option<&Regime> {
        self.conventions.iter().find(|(k, _)| k == idcc).map(|(_, r)| r)
    }
}

/// Le financement du paritarisme de la convention `idcc` (le FCAP-SVP de la 3090), si elle en a un.
pub fn financement(idcc: &str) -> Resultat<Option<Financement>> {
    Ok(catalogue()?.regime(idcc).and_then(|r| r.financement_paritarisme.clone()))
}

/// La derniere des valeurs datees jusqu'a `jour` inclus (toutes sans jour), et son jour.
fn derniere<'a>(table: &'a Datees, jour: Option<&Jour>) -> Option<(&'a Jour, Decimal)> {
    table.iter().filter(|(k, _)| jour.is_none_or(|j| *k <= j)).next_back().map(|(k, v)| (k, v.0))
}

/// La convention a-t-elle un regime de sante a elle ?
pub fn a_un_regime_de_sante(ent: &ChampsEntreprise) -> Resultat<bool> {
    Ok(catalogue()?.regime(ent.idcc.texte()).is_some_and(|r| r.sante.is_some()))
}

/// La complementaire sante que la CONVENTION fixe, quand elle en fixe une ; `None` si la convention n'a pas de section
/// `sante`.
pub(crate) fn sante_conventionnelle(ent: &ChampsEntreprise, sal: &SalarieDuContrat, fin: Option<&Jour>) -> Resultat<Option<Mutuelle>> {
    let c = catalogue()?;
    let Some(sante) = c.regime(ent.idcc.texte()).and_then(|r| r.sante.as_ref()) else { return Ok(None) };
    if super::sans_sante(sal) {
        return Ok(Some(Mutuelle::Sans));
    }
    let Some((_, mut total)) = derniere(&sante.taux, fin) else {
        return Err(format!(
            "Complementaire sante des permanents, convention {} : aucun taux \
en vigueur au {} dans le catalogue de branche.",
            ent.idcc.texte(),
            match fin {
                Some(j) => j.iso(),
                None => "jour de la paie".to_string(),
            }
        ));
    };
    if let Some(local) = sante.taux_alsace_moselle.as_ref().filter(|t| !t.is_empty()).filter(|_| alsace_moselle(ent, sal)) {
        if let Some((_, local)) = derniere(local, fin) {
            total = local;
        }
    }
    let choix = ent.sante_permanents.dite().and_then(|s| s.part_patronale.dite());
    let minimum = sante.part_patronale_minimum.0;
    // La part de l'employeur est celle que la compagnie a decidee (au moins le minimum de la convention) : elle seule la
    // connait.
    let Some(choix) = choix else {
        return Err(format!(
            "Fiche Entreprise, « Santé des permanents », part employeur : à remplir. La convention {} en met au moins {} % \
à la charge de l'employeur ; la part de la compagnie est sur son contrat de complémentaire santé.",
            ent.idcc.texte(),
            minimum
        ));
    };
    let part = choix.decimal();
    if part < minimum || part > Decimal::ONE_HUNDRED {
        return Err(format!(
            "Fiche Entreprise, « Santé des permanents », part employeur {} % : \
la convention {} met au moins {} % de la cotisation sante a la \
charge de l'employeur (XII.2.1.11), et pas plus de 100.",
            choix.ecrit(),
            ent.idcc.texte(),
            minimum
        ));
    }
    let base = sante.base.clone();
    Ok(Some(Mutuelle::Pose { base, total, pat: div(part, Decimal::ONE_HUNDRED) }))
}

/// `super::minimum_sante`.
pub fn minimum_sante(ent: &ChampsEntreprise, jour: Option<&Jour>) -> Resultat<Option<(Decimal, String)>> {
    let c = catalogue()?;
    let Some(m) = c.regime(ent.idcc.texte()).and_then(|r| r.sante_minimum.as_ref()) else { return Ok(None) };
    let annexe = ent.annexe_convention.code().to_string();
    let (table, qui) = match m.annexes.as_ref().and_then(|a| a.get(&annexe)) {
        Some(t) => (t, format!("annexe {}", annexe)),
        None => (&m.socle, "socle".to_string()),
    };
    Ok(derniere(table, jour).map(|(depuis, taux)| (taux, format!("{}, depuis le {}", qui, depuis.jj_mm_aaaa()))))
}

fn pcf(x: f64) -> String {
    deux_virgule(x)
}

/// Le regime conventionnel de cette convention collective ; refuse quand la convention n'est pas relevee ou pas
/// confirmee.
pub fn regime(idcc: &str) -> Resultat<Regime> {
    let c = catalogue()?;
    let idcc = idcc.trim();
    let Some(reg) = c.regime(idcc).filter(|_| !idcc.is_empty()) else {
        let connues: Vec<&str> = c.conventions.iter().map(|(k, _)| k.as_str()).collect();
        return Err(socle::echec::avec(
            "La prévoyance des permanents de cette convention collective n'est pas relevée : le signaler au développeur.",
            APart {
                detail: Some(format!(
                    "Prevoyance des permanents : la convention collective {} n'est pas \
relevee.\n  \
Relevees a ce jour : {}.\n  \
Ce qui manque n'est pas du code -- le mecanisme est ecrit -- mais \
un bloc de nombres : le titre << prevoyance >> de cette convention \
sur Legifrance (taux, assiette, repartition, conditions \
d'ouverture), a ajouter dans logiciel/donnees/prevoyance-\
conventionnelle-base.json avec sa source.\n  \
Tant qu'il manque, le moteur refuse de payer un permanent : une \
prevoyance oubliee ne se voit pas sur un bulletin, elle se \
decouvre le jour ou le salarie en a besoin.",
                    if idcc.is_empty() { "(non renseignee dans config/entreprise.json)" } else { idcc },
                    if connues.is_empty() { "aucune".to_string() } else { connues.join(", ") }
                )),
                ..APart::default()
            },
        ));
    };
    if reg.applicable == Some(false) {
        return Err(socle::echec::avec(
            "La prévoyance des permanents de cette convention n'est pas encore vérifiée : le signaler au développeur.",
            APart {
                detail: Some(format!(
                    "Prevoyance des permanents, convention {} ({}) : les taux sont \
RELEVES mais pas confirmes, le moteur ne paie donc pas.\n  \
{}\n  \
Ce qui a ete lu, a verifier : {}.\n  \
Quand c'est fait : `applicable: true` dans \
donnees/prevoyance-conventionnelle-base.json.",
                    idcc,
                    reg.libelle,
                    match &reg.a_confirmer {
                        Some(a) => a.as_str(),
                        None => "(le catalogue ne le dit pas)",
                    },
                    resume(reg)
                )),
                ..APart::default()
            },
        ));
    }
    Ok(reg.clone())
}

/// Le bloc tel qu'il vaut ce jour-la : ses changements dates jusqu'a `jour` inclus, dans l'ordre.
fn bloc_au(b: &Bloc, jour: &Jour) -> Bloc {
    let mut out = b.clone();
    for (_, c) in b.a_partir_du.iter().flatten().filter(|(d, _)| *d <= jour) {
        if c.taux.is_some() {
            out.taux = c.taux;
        }
        if c.t2.is_some() {
            out.t2 = c.t2;
        }
        if c.ventilation.is_some() {
            out.ventilation = c.ventilation.clone();
        }
    }
    out
}

/// Le regime tel qu'il vaut ce jour-la (`a_partir_du`).
fn a_la_date(mut reg: Regime, jour: Option<&Jour>) -> Regime {
    if let Some(j) = jour {
        reg.obligatoire = bloc_au(&reg.obligatoire, j);
        reg.obligatoire_cadre = reg.obligatoire_cadre.as_ref().map(|b| bloc_au(b, j));
    }
    reg
}

/// Les nombres d'un regime, en une ligne.
fn resume(reg: &Regime) -> String {
    let mut out = Vec::new();
    if reg.beneficiaires.non_cadres == Some(false) {
        out.push("non cadres : aucun regime de branche".to_string());
    } else if let Some(t) = reg.obligatoire.taux {
        out.push(format!("non cadres {} % de la tranche 1", pcf(t)));
    }
    if let Some((oc, taux)) = reg.obligatoire_cadre.as_ref().and_then(|oc| oc.taux.map(|t| (oc, t))) {
        let mut s = format!("cadres {} % de la tranche 1", pcf(taux));
        if let Some(t2) = oc.t2 {
            s += &format!(" et {} % de la tranche 2", pcf(t2));
        }
        out.push(s);
    }
    if out.is_empty() {
        "(aucun taux dans l'entree)".to_string()
    } else {
        out.join(" ; ")
    }
}

fn mois_entre(depuis: Option<&Jour>, jusqu_a: Option<&Jour>) -> Option<i64> {
    let (a, b) = (depuis?, jusqu_a?);
    Some((b.an - a.an) as i64 * 12 + (b.mois as i64 - a.mois as i64) - if b.jour < a.jour { 1 } else { 0 })
}

/// Le seuil d'anciennete dans la branche qui ouvre la cotisation, en mois.
fn seuil(reg: &Regime, cadre: bool) -> i64 {
    if cadre {
        reg.beneficiaires.ouverture_cadre_mois
    } else {
        reg.beneficiaires.ouverture_non_cadre_mois
    }
}

/// (la cotisation est-elle due, pourquoi) pour ce salarie, ce mois-la. Le seuil de la convention
/// (`beneficiaires.ouverture_*_mois`) : sous la 3090, « due pour les non cadres, des lors que le salarie acquiert un
/// an d'anciennete dans la branche du secteur prive du spectacle vivant ; pour les cadres, des l'embauche » (fiche
/// produit Audiens, `atelier/reference/sources/audiens-3090-prevoyance-sante-fiche-produit-2021.txt`, l. 102-106).
/// Ce temps dans la branche est un fait que seule la fiche dit (`prevoyance_branche_depuis`) : il se demande.
fn ouverture(reg: &Regime, sal: &SalarieDuContrat, cadre: bool, fin: Option<&Jour>) -> Resultat<(bool, String)> {
    let seuil = seuil(reg, cadre);
    if seuil <= 0 {
        return Ok((true, "due des l'embauche".to_string()));
    }
    let branche = sal.prevoyance_branche_depuis.dite();
    if let (Some(depuis), Some(mois)) = (branche, mois_entre(branche, fin)) {
        return Ok((mois >= seuil, format!("{} mois dans la branche depuis le {} (seuil {})", mois, depuis.iso(), seuil)));
    }
    let mois_maison = mois_entre(sal.date_anciennete.dite(), fin);
    if let Some(m) = mois_maison {
        if m >= seuil {
            return Ok((true, format!("{} mois d'anciennete dans l'entreprise, donc au moins autant dans la branche (seuil {})", m, seuil)));
        }
    }
    Err(format!(
        "{} : depuis quand travaille-t-il dans le spectacle vivant ? Sa prévoyance commence après {} mois \
dans la branche, chez nous ou dans une autre compagnie, et il n'en a que {} ici. À dire sur sa fiche, \
Configuration › Salariés, « Dans la branche depuis » : le jour où il y est entré ; s'il n'y travaillait pas \
avant, son jour d'embauche chez nous. Le supposer venu de la branche le ferait cotiser pour rien, le supposer \
nouveau le laisserait sans couverture pendant un an.",
        nom_du_salarie(sal),
        seuil,
        match mois_maison {
            Some(m) => format!("{} mois", m),
            None => "aucune date d'anciennete".to_string(),
        }
    ))
}

/// (taux minimum de tranche 1, d'ou il vient) pour ce salarie ; `None` : ni la convention (elle fixe la repartition,
/// pas le taux : `contrat_requis`) ni l'obligation cadre n'en fixent.
fn minimum_t1(reg: &Regime, annexe: &str, cadre: bool) -> Resultat<Option<(f64, String)>> {
    let mut retenu = reg.obligatoire.taux.map(|t| (t, format!("{} : {} %", reg.texte, pcf(t))));
    let sup = reg.annexes.as_ref().and_then(|a| a.get(annexe)).map(|a| a.taux).filter(|t| *t != 0.0);
    if let Some(s) = sup.filter(|s| retenu.as_ref().is_none_or(|(t, _)| s > t)) {
        retenu = Some((s, format!("annexe {} de la convention : {} %", annexe, pcf(s))));
    }
    if cadre {
        if let Some(propre) = reg.obligatoire_cadre.as_ref().and_then(|oc| oc.taux) {
            let t = match sup {
                Some(s) => propre.max(s),
                None => propre,
            };
            retenu = Some((t, format!("{} : {} % (cadres)", reg.texte, pcf(t))));
        }
        let plancher = catalogue()?.obligation_cadre.taux_t1;
        if retenu.as_ref().is_none_or(|(t, _)| plancher > *t) {
            let pourquoi = match &retenu {
                Some((_, p)) => format!("{}, porte a {} % par l'obligation cadre (ANI du 17/11/2017)", p, pcf(plancher)),
                None => format!("l'obligation cadre (ANI du 17/11/2017) : {} %", pcf(plancher)),
            };
            retenu = Some((plancher, pourquoi));
        }
    }
    Ok(retenu)
}

/// Une tranche de la prevoyance d'un permanent : ses deux parts, en pourcent, et son assiette.
#[derive(Debug, Clone, Default)]
pub struct Part {
    pub taux_pat: f64,
    pub taux_sal: f64,
    pub base: Option<String>,
    pub obligatoire: bool,
}

/// Ce qu'un permanent cotise en prevoyance ce mois-la.
#[derive(Debug, Clone, Default)]
pub struct Cotisation {
    pub t1: Part,
    pub t2: Option<Part>,
    pub due: bool,
    /// La convention n'a pas de regime pour ses non cadres : pas de ligne.
    pub sans_regime: bool,
}

fn repartir(taux: f64, part_patronale: f64) -> Part {
    let pat = arrondi4(taux * part_patronale / 100.0);
    Part { taux_pat: pat, taux_sal: arrondi4(taux - pat), ..Part::default() }
}

/// La part patronale de la tranche 2 d'un bloc : la sienne quand le texte les distingue, sinon celle du bloc, que le
/// texte fixe pour toute la cotisation.
fn part_t2(b: &Bloc) -> f64 {
    match b.t2_part_patronale {
        Some(propre) => propre,
        None => b.part_patronale,
    }
}

/// Ce qu'un permanent cotise en prevoyance ce mois-la.
pub fn cotisation(sal: &SalarieDuContrat, cadre: bool, fin: Option<&Jour>, entreprise: &ChampsEntreprise) -> Resultat<Cotisation> {
    let idcc = entreprise.idcc.texte();
    let reg = a_la_date(regime(idcc)?, fin);
    let ent = entreprise.prevoyance_permanents.dite();
    let contrat = ent.is_some_and(|p| p.regime.est_dite());
    let ent_taux = ent.and_then(|p| p.taux.dite());
    let part_du_contrat = ent_taux.and_then(|t| t.part_patronale.dite());
    if !cadre && reg.beneficiaires.non_cadres == Some(false) && !contrat {
        return Ok(Cotisation { sans_regime: true, ..Cotisation::default() });
    }
    if reg.contrat_requis == Some(true) && !contrat {
        return Err(socle::echec::avec(
            "La prévoyance des permanents de cette convention suit le contrat de la compagnie : poser ses taux dans la fiche entreprise, « Prévoyance des permanents ».",
            APart {
                detail: Some(format!(
                    "Prevoyance des permanents, convention {} : {} fixe les garanties \
et leur repartition, pas le taux -- c'est celui du contrat que \
l'entreprise a souscrit.\n  \
A poser : fiche Entreprise, « Prévoyance des permanents », \
regime « contrat », avec ses taux (t1_non_cadre, t1_cadre), \
lus sur le contrat ou l'appel de cotisations. Rien n'est calcule.",
                    idcc, reg.texte
                )),
                ..APart::default()
            },
        ));
    }
    let (due, _) = ouverture(&reg, sal, cadre, fin)?;
    if !due {
        return Ok(Cotisation::default());
    }

    let minimum = minimum_t1(&reg, entreprise.annexe_convention.code(), cadre)?;
    let obl = &reg.obligatoire;
    let oc = reg.obligatoire_cadre.as_ref();
    let mut part_pat = obl.part_patronale;
    if cadre {
        part_pat = match oc {
            Some(b) => b.part_patronale,
            None => catalogue()?.obligation_cadre.part_patronale,
        };
    }
    let taux = if contrat {
        let pour = if cadre { "cadres" } else { "non-cadres" };
        let Some(t1_contrat) = ent_taux.and_then(|t| if cadre { t.t1_cadre.dite() } else { t.t1_non_cadre.dite() }) else {
            return Err(format!(
                "Fiche Entreprise, « Prévoyance des permanents » : le regime \
est « contrat d'entreprise » mais son taux {} manque. Un contrat \
d'entreprise remplace le taux conventionnel : il faut le dire \
en entier, sinon le moteur ne sait pas s'il atteint le \
minimum de la branche.",
                pour
            ));
        };
        let propose = t1_contrat.flottant()?;
        if let Some((mini, origine)) = minimum.as_ref().filter(|(mini, _)| propose + 1e-9 < *mini) {
            return Err(format!(
                "Fiche Entreprise, « Prévoyance des permanents », taux {} : \
{} % : c'est sous le minimum de la branche ({} %, {}).\n  \
Une entreprise peut souscrire mieux que la convention, \
jamais moins : le minimum conventionnel s'impose a elle. \
Corriger le contrat, ou revenir au regime conventionnel.",
                pour,
                pcf(propose),
                pcf(*mini),
                origine
            ));
        }
        if let Some(p) = part_du_contrat {
            part_pat = p.flottant()?;
        }
        propose
    } else {
        match minimum {
            Some((mini, _)) => mini,
            None => {
                return Err(format!(
                    "Prévoyance des permanents, convention {} : {} ne fixe pas de taux de tranche 1 ; celui du contrat de \
la compagnie se pose dans la fiche Entreprise, « Prévoyance des permanents ».",
                    idcc, reg.texte
                ))
            }
        }
    };

    if cadre && part_pat < catalogue()?.obligation_cadre.part_patronale {
        return Err("Fiche Entreprise, « Prévoyance des permanents » : la part \
patronale d'un cadre ne peut pas descendre sous 100 % -- les \
1,50 % de la tranche A sont dus << a titre exclusif >> par \
l'employeur (ANI du 17/11/2017, art. 1er)."
            .to_string());
    }

    let mut t1 = repartir(taux, part_pat);
    t1.base = Some(obl.base.clone().ok_or_else(|| socle::fichiers::absent("obligatoire.base"))?);
    let mut t2: Option<Part> = None;
    let obligatoire_t2 = match oc.filter(|b| cadre && b.t2.is_some()) {
        Some(b) => Some(b),
        None => Some(obl).filter(|b| b.t2.is_some()),
    };
    if let Some((b, t2_taux)) = obligatoire_t2.filter(|_| !contrat).and_then(|b| b.t2.map(|t| (b, t))) {
        let base_t2 = b.t2_base.clone().ok_or_else(|| socle::fichiers::absent("t2_base"))?;
        let mut m = repartir(t2_taux, part_t2(b));
        m.base = Some(base_t2);
        m.obligatoire = true;
        t2 = Some(m);
    }
    let ext = ent.and_then(|p| p.extension_t2.dite());
    if let Some(ext) = ext.filter(|e| **e == ExtensionT2::Tous || cadre) {
        let opt = reg.options.as_ref().map(|o| &o.extension_t2);
        let propre = opt.and_then(|o| match ext {
            ExtensionT2::Tous => o.tous,
            ExtensionT2::Cadres => o.cadres,
        });
        let (Some(opt), Some(propre)) = (opt, propre) else {
            return Err(format!(
                "L'extension T2 << {} >> n'existe pas dans le regime de la \
convention {}.",
                ext.as_str(),
                idcc
            ));
        };
        let part = match part_du_contrat {
            Some(p) => p.flottant()?,
            None => opt.part_patronale,
        };
        let mut m = repartir(propre, part);
        m.base = Some(opt.base.clone());
        t2 = Some(m);
    }
    Ok(Cotisation { t1, t2, due: true, sans_regime: false })
}

/// Le premier jour ou `mois_entre(depuis, jour)` atteint `seuil` : le meme quantieme `seuil` mois plus tard, ou le
/// premier du mois suivant quand ce quantieme n'existe pas (31 janvier + 1 mois : le 1er mars).
fn anniversaire(depuis: &Jour, seuil: i64) -> Option<Jour> {
    let rang = depuis.an as i64 * 12 + (depuis.mois as i64 - 1) + seuil;
    let (an, mois) = ((rang / 12) as i32, (rang % 12) as u32 + 1);
    Jour::nouveau(an, mois, depuis.jour).or_else(|| {
        let (an, mois) = if mois == 12 { (an + 1, 1) } else { (an, mois + 1) };
        Jour::nouveau(an, mois, 1)
    })
}

/// L'affiliation d'un permanent a la prevoyance de sa convention au `fin` (bloc S21.G00.70), selon la meme regle que
/// sa cotisation (`cotisation`, `ouverture`).
pub fn affiliation(sal: &SalarieDuContrat, cadre: bool, fin: &Jour, entreprise: &ChampsEntreprise) -> Resultat<super::AffiliationPrevoyance> {
    use super::AffiliationPrevoyance as A;
    let cot = cotisation(sal, cadre, Some(fin), entreprise)?;
    if !cot.due {
        return Ok(A::NonAffilie);
    }
    let reg = a_la_date(regime(entreprise.idcc.texte())?, Some(fin));
    let seuil = seuil(&reg, cadre);
    if seuil <= 0 {
        return Ok(A::DesLEmbauche);
    }
    if let Some(j) = sal.prevoyance_branche_depuis.dite().copied().and_then(|d| anniversaire(&d, seuil)) {
        return Ok(A::Le(j));
    }
    match sal.date_anciennete.dite().copied().and_then(|d| anniversaire(&d, seuil)) {
        Some(j) => Ok(A::AuPlusTardLe(j)),
        None => Err(format!(
            "{} : la prevoyance s'ouvre apres {} mois d'anciennete dans la branche, et sa fiche ne date ni son \
entree dans la branche (« Dans la branche depuis ») ni son anciennete (« Dans la compagnie depuis »).",
            nom_du_salarie(sal),
            seuil
        )),
    }
}
