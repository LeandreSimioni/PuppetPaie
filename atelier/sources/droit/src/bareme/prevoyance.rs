//! LA PREVOYANCE DE BRANCHE DES PERMANENTS : ce que la convention collective impose, lu dans
//! `donnees/prevoyance-conventionnelle-base.json` par sa forme (`Catalogue`), une fois tant que le fichier ne change
//! pas. Le catalogue porte le minimum de la branche ; ce que la compagnie souscrit se dit sur sa fiche
//! (`prevoyance_permanents`), et le moteur le confronte a ce minimum.

use super::{alsace_moselle, arrondi4, deux_virgule, jour_texte, nom_du_salarie, ChampsEntreprise, Decimal, Jour, Mutuelle, Resultat, SalarieDuContrat};
use crate::fiche::ExtensionT2;
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
    /// Le plancher national des cadres, quelle que soit la convention : un taux de la tranche 1, a la charge de
    /// l'employeur.
    pub struct ObligationCadre {
        pub taux_t1: f64,
        pub charge: String,
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
        #[serde(default)]
        pub applicable: Option<bool>,
        /// Ce qui reste a verifier d'un regime qui n'est pas applicable.
        #[serde(default)]
        pub a_confirmer: Option<String>,
        /// Le texte fixe la repartition, pas le taux : celui du contrat de l'entreprise, sur sa fiche.
        #[serde(default)]
        pub contrat_requis: bool,
        pub obligatoire: Bloc,
        #[serde(default)]
        pub obligatoire_cadre: Option<Bloc>,
        pub beneficiaires: Beneficiaires,
        #[serde(default)]
        pub options: Option<Options>,
        #[serde(default)]
        pub sante_minimum: Option<SanteMinimum>,
        #[serde(default)]
        pub financement_paritarisme: Option<Financement>,
        #[serde(default)]
        pub annexes: BTreeMap<String, Annexe>,
        #[serde(default)]
        pub sante: Option<Sante>,
    }
}

forme! {
    /// Une cotisation de prevoyance (tous, ou les cadres) : tranche 1, tranche 2, repartition, et ce qui change a une date.
    pub struct Bloc {
        #[serde(default)]
        pub taux: Option<f64>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub part_patronale: Option<f64>,
        #[serde(default)]
        pub t2: Option<f64>,
        #[serde(default)]
        pub t2_base: Option<String>,
        #[serde(default)]
        pub t2_part_patronale: Option<f64>,
        #[serde(default)]
        pub ventilation: Option<Ventilation>,
        #[serde(default)]
        pub a_partir_du: BTreeMap<Jour, Changement>,
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
        #[serde(default)]
        pub taux: Option<f64>,
        #[serde(default)]
        pub t2: Option<f64>,
        #[serde(default)]
        pub ventilation: Option<Ventilation>,
    }
}

forme! {
    /// Qui beneficie du regime, et apres combien de mois d'anciennete dans la branche.
    pub struct Beneficiaires {
        /// Faux : la convention n'a pas de regime pour ses non cadres.
        #[serde(default)]
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
        #[serde(default)]
        pub tous: Option<f64>,
        #[serde(default)]
        pub cadres: Option<f64>,
        #[serde(default)]
        pub base: Option<String>,
        #[serde(default)]
        pub part_patronale: Option<f64>,
    }
}

/// Des valeurs datees : chacune vaut de son jour au suivant.
pub type Datees = BTreeMap<Jour, Nombre>;

forme! {
    /// La cotisation minimale de la sante des permanents, en % du plafond mensuel : le socle, et ce qu'une annexe impose.
    pub struct SanteMinimum {
        pub socle: Datees,
        #[serde(default)]
        pub annexes: BTreeMap<String, Datees>,
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
        #[serde(default)]
        pub base: Option<String>,
        pub taux: Datees,
        #[serde(default)]
        pub taux_alsace_moselle: Datees,
        #[serde(default)]
        pub part_patronale_minimum: Option<Nombre>,
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

/// La derniere des valeurs datees jusqu'a `jour` inclus (toutes si `jour` est vide), et son jour.
fn derniere<'a>(table: &'a Datees, jour: &str) -> Option<(&'a Jour, Decimal)> {
    table.iter().filter(|(k, _)| jour.is_empty() || k.iso().as_str() <= jour).next_back().map(|(k, v)| (k, v.0))
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
    let jour = jour_texte(fin);
    let Some((_, mut total)) = derniere(&sante.taux, &jour) else {
        return Err(format!(
            "Complementaire sante des permanents, convention {} : aucun taux \
en vigueur au {} dans le catalogue de branche.",
            ent.idcc.texte(),
            jour
        ));
    };
    if !sante.taux_alsace_moselle.is_empty() && alsace_moselle(ent, sal) {
        if let Some((_, local)) = derniere(&sante.taux_alsace_moselle, &jour) {
            total = local;
        }
    }
    let choix = ent.sante_permanents.dite().and_then(|s| s.part_patronale.dite());
    let minimum = sante.part_patronale_minimum.map_or(Decimal::from(50), |n| n.0);
    let part = choix.map_or(minimum, |c| c.decimal());
    if part < minimum || part > Decimal::ONE_HUNDRED {
        return Err(format!(
            "Fiche Entreprise, « Santé des permanents », part employeur {} % : \
la convention {} met au moins {} % de la cotisation sante a la \
charge de l'employeur (XII.2.1.11), et pas plus de 100.",
            choix.map(|c| c.ecrit()).unwrap_or_default(),
            ent.idcc.texte(),
            minimum
        ));
    }
    let base = sante.base.clone().unwrap_or_else(|| "pss_periode".to_string());
    Ok(Some(Mutuelle::Pose { base, total, pat: div(part, Decimal::ONE_HUNDRED) }))
}

/// `super::minimum_sante`.
pub fn minimum_sante(ent: &ChampsEntreprise, jour: &str) -> Resultat<(Option<Decimal>, String)> {
    let c = catalogue()?;
    let Some(m) = c.regime(ent.idcc.texte()).and_then(|r| r.sante_minimum.as_ref()) else { return Ok((None, String::new())) };
    let annexe = ent.annexe_convention.code().to_string();
    let (table, qui) = match m.annexes.get(&annexe) {
        Some(t) => (t, format!("annexe {}", annexe)),
        None => (&m.socle, "socle".to_string()),
    };
    Ok(match derniere(table, jour) {
        Some((depuis, taux)) => (Some(taux), format!("{}, depuis le {}", qui, depuis.jj_mm_aaaa())),
        None => (None, String::new()),
    })
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
                    reg.a_confirmer.as_deref().unwrap_or_default(),
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
    for (_, c) in b.a_partir_du.iter().filter(|(d, _)| *d <= jour) {
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
    if let Some(oc) = reg.obligatoire_cadre.as_ref().filter(|oc| oc.taux.is_some()) {
        let mut s = format!("cadres {} % de la tranche 1", pcf(oc.taux.unwrap_or_default()));
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
    if let Some(mois) = mois_entre(branche, fin) {
        return Ok((mois >= seuil, format!("{} mois dans la branche depuis le {} (seuil {})", mois, branche.map(Jour::iso).unwrap_or_default(), seuil)));
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
        nom_du_salarie(sal, "ce salarie"),
        seuil,
        match mois_maison {
            Some(m) => format!("{} mois", m),
            None => "aucune date d'anciennete".to_string(),
        }
    ))
}

/// (taux minimum de tranche 1, d'ou il vient) pour ce salarie.
fn minimum_t1(reg: &Regime, annexe: &str, cadre: bool) -> Resultat<(f64, String)> {
    let mut taux = reg.obligatoire.taux.unwrap_or(0.0);
    let mut pourquoi = format!("{} : {} %", reg.texte, pcf(taux));
    let sup = reg.annexes.get(annexe).map(|a| a.taux).filter(|t| *t != 0.0);
    if let Some(s) = sup.filter(|s| *s > taux) {
        taux = s;
        pourquoi = format!("annexe {} de la convention : {} %", annexe, pcf(taux));
    }
    if cadre {
        if let Some(propre) = reg.obligatoire_cadre.as_ref().and_then(|oc| oc.taux) {
            taux = propre.max(sup.unwrap_or(0.0));
            pourquoi = format!("{} : {} % (cadres)", reg.texte, pcf(taux));
        }
        let plancher = catalogue()?.obligation_cadre.taux_t1;
        if plancher > taux {
            pourquoi = format!("{}, porte a {} % par l'obligation cadre (ANI du 17/11/2017)", pourquoi, pcf(plancher));
            taux = plancher;
        }
    }
    Ok((taux, pourquoi))
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

/// La part patronale de la tranche 2 d'un bloc : la sienne, sinon celle du bloc, sinon 100.
fn part_t2(b: &Bloc) -> f64 {
    b.t2_part_patronale.or(b.part_patronale).unwrap_or(100.0)
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
    if reg.contrat_requis && !contrat {
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

    let (mini, origine) = minimum_t1(&reg, entreprise.annexe_convention.code(), cadre)?;
    let obl = &reg.obligatoire;
    let oc = reg.obligatoire_cadre.as_ref();
    let pat_cadre = oc.and_then(|b| b.part_patronale);
    let mut part_pat = obl.part_patronale.unwrap_or(100.0);
    if cadre {
        part_pat = pat_cadre.unwrap_or(part_pat.max(100.0));
    }
    let mut taux = mini;

    if contrat {
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
        if propose + 1e-9 < mini {
            return Err(format!(
                "Fiche Entreprise, « Prévoyance des permanents », taux {} : \
{} % : c'est sous le minimum de la branche ({} %, {}).\n  \
Une entreprise peut souscrire mieux que la convention, \
jamais moins : le minimum conventionnel s'impose a elle. \
Corriger le contrat, ou revenir au regime conventionnel.",
                pour,
                pcf(propose),
                pcf(mini),
                origine
            ));
        }
        taux = propose;
        if let Some(p) = part_du_contrat {
            part_pat = p.flottant()?;
        }
    }

    if cadre && part_pat < 100.0 {
        return Err("Fiche Entreprise, « Prévoyance des permanents » : la part \
patronale d'un cadre ne peut pas descendre sous 100 % -- les \
1,50 % de la tranche A sont dus << a titre exclusif >> par \
l'employeur (ANI du 17/11/2017, art. 1er)."
            .to_string());
    }

    let mut t1 = repartir(taux, part_pat);
    t1.base = Some(obl.base.clone().filter(|b| !b.is_empty()).unwrap_or_else(|| "t1_mensuelle".to_string()));
    let mut t2: Option<Part> = None;
    let obligatoire_t2 = match oc.filter(|b| cadre && b.t2.is_some()) {
        Some(b) => Some(b),
        None => Some(obl).filter(|b| b.t2.is_some()),
    };
    if let Some(b) = obligatoire_t2.filter(|_| !contrat) {
        let mut m = repartir(b.t2.unwrap_or_default(), part_t2(b));
        m.base = Some(b.t2_base.clone().filter(|b| !b.is_empty()).unwrap_or_else(|| "t2_conventionnelle".to_string()));
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
            None => opt.part_patronale.unwrap_or(100.0),
        };
        let mut m = repartir(propre, part);
        m.base = Some(opt.base.clone().filter(|b| !b.is_empty()).unwrap_or_else(|| "t2_conventionnelle".to_string()));
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
            nom_du_salarie(sal, "ce salarie"),
            seuil
        )),
    }
}
