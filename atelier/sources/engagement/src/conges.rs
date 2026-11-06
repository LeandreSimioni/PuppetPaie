//! Les conges payes d'un permanent, et les lignes de son mois : d'une saisie de permanent aux lignes d'un bulletin
//! (`lignes_du_mois`), et son compteur de conges payes. Les jours et l'indemnite retenue sont la regle du droit
//! (`droit::conges`, `droit::permanents`).
//!
//! Le compteur se tire des paies validees du salarie (`etats::paies_validees`), et d'elles seules : leur presence,
//! leurs jours pris, leur brut. Ce qui n'est pas valide n'est pas paye, et aucun mois ne se calcule pour un autre. La
//! validation ecrit dans chaque paie le compteur que son bulletin montre et le compteur avant son mois
//! (`compteur_ecrit`, `Avant`) ; le bulletin et la provision lisent le premier, le mois suivant reprend le second et n'y
//! ajoute que les paies du mois d'avant (`suivant`) : un mois ne relit pas les mois d'avant. Sans etat ecrit au mois
//! d'avant (le premier mois paye, un mois sans paie), le compteur se tire de chaque mois de la periode. Le memo d'une
//! derivation lit chaque mois une fois ; il meurt avec la derivation qui l'a ouvert.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use socle::forme::NombreLu;

use crate::calcul::Paie;
use crate::saisie::{Carte, CongePris};
use droit::catalogue::rubrique;
use droit::clauses;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::mois::Mois;
use crate::{Avis, Chiffre, LignePaie};
use socle::feries;
use socle::texte::{euros, virgule};
use socle::num::{add, dec_f64, div, eur2, mul, zero2, Decimal, Resultat};
use droit::conges::{jours_ouvrables, IndemniteDesJours};
use droit::permanents::{arrondir_jours, duree_du_conge, jours_acquis, periode_de_reference, plafonner, Acquisition};

// ------------------------------------------------------------ les aides

/// Le salarie que nomme en tete ce que le moteur dit d'un engagement paye au mois (`crate::salarie_nomme`).
pub(crate) fn qui(ctx: &Contexte, e: &Carte) -> String {
    crate::salarie_nomme(ctx, e.salarie.as_deref())
}

/// Le salarie que la carte `e` ecrit ; refus s'il n'est pas dit.
pub(crate) fn salarie_de(e: &Carte) -> Resultat<&str> {
    e.salarie.as_deref().ok_or_else(|| socle::fichiers::absent("salarie"))
}

/// Le debut que la carte `e` ecrit, lu ; refus s'il n'est pas dit.
pub(crate) fn debut_de(e: &Carte) -> Resultat<Jour> {
    Jour::lire(e.debut.as_deref().ok_or_else(|| socle::fichiers::absent("debut"))?)
}

/// Un jour dit et non vide de la carte, lu.
pub(crate) fn jour_dit(x: &Option<String>) -> Resultat<Option<Jour>> {
    x.as_deref().filter(|j| !j.is_empty()).map(Jour::lire).transpose()
}

/// La regle qui paie le mois incomplet [`debut`, `fin`] (borne au mois [`debut_mois`, `fin_mois`]) du contrat de
/// `salarie` (`droit::periodes::regle_du_mois_incomplet`) : la convention de l'entreprise, le statut de la fiche
/// (`calcul::statut_de`), et si le contrat commence ou finit dans le mois.
pub(crate) fn regle_du_mois_incomplet(
    ctx: &Contexte,
    salarie: &str,
    debut: &Jour,
    fin: &Jour,
    debut_mois: &Jour,
    fin_mois: &Jour,
) -> Resultat<droit::periodes::RegleDuMoisIncomplet> {
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let fiche = ctx.salarie(salarie)?;
    let statut = crate::calcul::statut_de(&fiche)?;
    Ok(droit::periodes::regle_du_mois_incomplet(&idcc, statut, debut > debut_mois || fin < fin_mois))
}

/// L'horaire de `salarie` pour le mois [`debut_mois`, `fin_mois`] (`droit::periodes::Horaire`) : ses jours travailles,
/// lus sur sa fiche (`droit::conges::JoursTravailles`), et les feries chomes du mois que l'entreprise ne lui doit pas
/// (`droit::conges::feries_non_payes`, L3133-3), son anciennete comptee depuis sa fiche (« Dans la compagnie depuis »)
/// ou, sans elle, depuis le debut de son contrat `debut_contrat` (la date que la validation pose sur la fiche).
pub(crate) fn horaire_du_mois(
    ctx: &Contexte,
    salarie: &str,
    debut_contrat: &Jour,
    debut_mois: &Jour,
    fin_mois: &Jour,
) -> Resultat<droit::periodes::Horaire> {
    let fiche = ctx.salarie(salarie)?;
    let jours = droit::conges::JoursTravailles::de_la_fiche(&fiche)?;
    let anciennete = crate::calcul::date_d_anciennete(&fiche, debut_contrat);
    let feries_non_payes = droit::conges::feries_non_payes(debut_mois, fin_mois, ctx.entreprise.feries_chomes(), &jours, &anciennete)?;
    Ok(droit::periodes::Horaire { jours, feries_non_payes })
}

/// Le flottant d'un decimal, correctement arrondi, par le texte.
pub(crate) fn flot(x: Decimal) -> f64 {
    socle::num::flottant(x)
}

/// LE TAUX D'UNE LIGNE QUI PAIE `montant` POUR `quantite` : la ligne paie son taux par sa quantite, au centime
/// (`calcul::lignes_brut`) ; un taux arrondi au centime paierait un autre montant que celui qui est retenu. Le taux
/// garde toute sa precision, et le produit retombe sur le montant.
pub(crate) fn taux_au_montant(montant: Decimal, quantite: Decimal) -> Decimal {
    div(montant, quantite)
}

pub(crate) fn info(texte: String) -> Avis {
    Avis { info: true, texte }
}

pub(crate) fn avertir(texte: String) -> Avis {
    Avis { info: false, texte }
}

pub(crate) fn sub(a: Decimal, b: Decimal) -> Decimal {
    add(a, -b)
}

/// Le decimal d'un nombre de la carte, son echelle gardee (`Chiffre::dec_avec_echelle`), ou, s'il n'est pas dit ou nul,
/// `defaut`.
pub(crate) fn dec_ou(v: Option<&NombreLu>, defaut: Decimal) -> Resultat<Decimal> {
    match v.map(|n| Chiffre::Nombre(n.clone())) {
        Some(x) if x.vrai() => x.dec_avec_echelle(),
        _ => Ok(defaut),
    }
}

/// Le decimal d'un nombre de la carte, son echelle gardee (`Chiffre::dec_avec_echelle`).
pub(crate) fn dec_de(n: &NombreLu) -> Resultat<Decimal> {
    Chiffre::Nombre(n.clone()).dec_avec_echelle()
}

// -------------------------------------------------- les jours ouvrables

/// Les jours ouvrables d'un conge pris dans une entreprise qui chome `chomes` (`droit::conges::jours_ouvrables`).
pub(crate) fn jours_ouvrables_de(c: &CongePris, chomes: Option<&str>) -> Resultat<i64> {
    let (debut, fin) = c.jours()?;
    jours_ouvrables(&debut, &fin, chomes)
}

fn fin_mois_de(mois: &str) -> Resultat<Jour> {
    Mois::lire(mois).map(Mois::dernier_jour)
}

// ------------------------------------------------ le compteur de conges

/// Ou en est un salarie de ses conges payes, a une date. Il s'ecrit dans la paie validee (`paie_ecrite::PaieEcrite::conges`) :
/// les nombres en texte tels qu'ils sont, les jours en AAAA-MM-JJ.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompteurConges {
    pub salarie: String,
    pub periode_debut: Jour,
    pub periode_fin: Jour,
    /// Jours ouvrables, compte exact.
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub acquis: Decimal,
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub pris: Decimal,
    /// L'assiette du dixieme.
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub remuneration_periode: Decimal,
    /// Ce qui reste des jours acquis sur la periode precedente.
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub reports: Decimal,
    pub report_debut: Option<Jour>,
    pub report_fin: Option<Jour>,
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub acquis_report: Decimal,
    /// L'assiette du dixieme de la periode du report (zero sans report).
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub remuneration_report: Decimal,
    /// Les jours pris que l'ancien logiciel a deja imputes sur la periode en
    /// cours (reprise) : ils ne consomment pas le report.
    #[serde(with = "crate::paie_ecrite::decimal_texte")]
    pub pris_sur_la_periode: Decimal,
    pub avertissements: Vec<Avis>,
}

impl CompteurConges {
    /// Ce qui reste du report apres les jours deja pris sur la periode.
    pub fn report_restant(&self) -> Decimal {
        let pris_fifo = sub(self.pris, self.pris_sur_la_periode).max(Decimal::ZERO);
        sub(self.reports, pris_fifo).max(Decimal::ZERO)
    }

    /// L'indemnite de `jours` de conges pris sur ce compteur, le maintien `maintien` compare au total : la regle de
    /// `droit::permanents::indemnite_des_jours` (L3141-24) sur ce que le compteur a acquis.
    pub fn indemnite_des_jours(&self, jours: Decimal, maintien: Decimal) -> Resultat<IndemniteDesJours> {
        let report = Acquisition { jours: self.acquis_report, remuneration: self.remuneration_report };
        let en_cours = Acquisition { jours: self.acquis, remuneration: self.remuneration_periode };
        droit::permanents::indemnite_des_jours(jours, self.report_restant(), report, en_cours, maintien)
    }

    pub fn solde(&self) -> Decimal {
        sub(add(self.acquis, self.reports), self.pris)
    }

    pub fn solde_affiche(&self) -> Decimal {
        arrondir_jours(self.solde())
    }

    pub fn resume(&self) -> String {
        let report = if self.reports != Decimal::ZERO {
            format!("{} jours reportes de la periode precedente, ", arrondir_jours(self.reports))
        } else {
            String::new()
        };
        format!(
            "conges payes du {} au {} : {}{} jours acquis, {} pris, solde {} jours ouvrables",
            self.periode_debut.jj_mm_aaaa(),
            self.periode_fin.jj_mm_aaaa(),
            report,
            arrondir_jours(self.acquis),
            arrondir_jours(self.pris),
            self.solde_affiche()
        )
    }
}

/// Ce qu'une paie validee apporte au compteur de conges et a la deduction forfaitaire de l'annee.
#[derive(Debug, Clone)]
struct PaieLue {
    brut: Decimal,
    /// La deduction forfaitaire specifique retenue (zero sans DFS).
    dfs: Decimal,
    /// Un contrat paye au mois : sa presence dans le mois, et ce qu'il en decompte.
    mensuel: Option<MoisPaye>,
    /// Le compteur avant le mois que la paie a ecrit (`etats::CONGES_AVANT`).
    avant: Option<Avant>,
    /// Les heures supplementaires payees (`heures_en_plus::HEURE_SUPPLEMENTAIRE`) et le montant net des heures en plus
    /// exonere d'impot (`heures_en_plus::EXONERE`).
    heures_supplementaires: Decimal,
    exonere: Decimal,
}

#[derive(Debug, Clone)]
struct MoisPaye {
    /// La presence du mois : le contrat borne au mois (`Engagement::debut_periode`, `fin_periode`).
    debut: Jour,
    fin: Jour,
    contrat_debut: Jour,
    numero: Option<crate::numeros::NumeroContrat>,
    /// Les jours de conges que la paie decompte (`conges_pris_de_la_paie`).
    pris: Decimal,
    /// Un arret de travail du contrat range dans ce mois (`imprevus`).
    arret: bool,
}

/// Ce que la paie validee `p` de `mois` apporte au compteur.
fn lue(ctx: &Contexte, mois: &str, p: &Paie) -> Resultat<PaieLue> {
    let eng = &p.engagement;
    let cle = p.salarie.id.to_string();
    let mensuel = if eng.nat()?.mensuelle() {
        let mut arret = false;
        for imp in crate::imprevus::du_mois_du_salarie(&ctx.racine, mois, None, &cle)? {
            arret |= crate::imprevus::est_un_arret(&imp) && crate::imprevus::du_contrat(&imp, &cle, eng.numero.as_deref(), &eng.debut)?;
        }
        Some(MoisPaye {
            debut: eng.debut_periode()?,
            fin: eng.fin_periode()?,
            contrat_debut: eng.debut,
            numero: eng.numero.clone(),
            pris: conges_pris_de_la_paie(&ctx.racine, p)?,
            arret,
        })
    } else {
        None
    };
    // Sans deduction forfaitaire, rien n'est deduit ; sans base exoneree, rien n'est exonere.
    let dfs = match &p.deduction_forfaitaire {
        Some(d) => d.montant,
        None => zero2(),
    };
    let heures_supplementaires =
        p.remuneration.iter().filter(|l| l.code == crate::heures_en_plus::HEURE_SUPPLEMENTAIRE).fold(Decimal::ZERO, |a, l| a + l.quantite);
    let exonere = match p.bases.get(crate::heures_en_plus::EXONERE) {
        Some(x) => *x,
        None => zero2(),
    };
    Ok(PaieLue { brut: p.brut, dfs, mensuel, avant: p.conges_avant.clone(), heures_supplementaires, exonere })
}

// Le memo d'une derivation : la profondeur des derivations ouvertes, et ce que les paies de chaque (racine, mois,
// salarie) ont deja apporte (`PaieLue`), ou le refus d'un mois qui ne se lit pas.
type PaiesDuMois = Result<Vec<PaieLue>, String>;

thread_local! {
    static MEMO: RefCell<(u32, HashMap<(String, String), PaiesDuMois>)> =
        RefCell::new((0, HashMap::new()));
}

/// Une derivation ouverte : chaque mois s'y lit une fois ; le memo se vide quand la derniere se ferme.
pub struct Derivation;

impl Derivation {
    pub fn ouvrir() -> Derivation {
        MEMO.with(|m| m.borrow_mut().0 += 1);
        Derivation
    }
}

impl Drop for Derivation {
    fn drop(&mut self) {
        MEMO.with(|m| {
            let mut m = m.borrow_mut();
            m.0 -= 1;
            if m.0 == 0 {
                m.1.clear();
            }
        });
    }
}

/// La cle d'un mois au memo de la derivation : la compagnie, le mois, le salarie, le registre de lectures ouvert sur ce
/// fil (`fichiers::generation`, 0 sans registre) et l'etat de la validation en cours (`etats::marque_de_validation`). Un
/// calcul suivi ne se sert ainsi que des mois relus par lui-meme (leurs lectures se notent), meme dans une derivation
/// ouverte avant lui (`moteur::routes::etapes::mois_a_ouvrir`).
fn cle_du_memo(ctx: &Contexte, mois: &str, cle: &str) -> (String, String) {
    (
        ctx.racine.display().to_string(),
        format!("{}|{}|{}|{}", mois, cle, socle::fichiers::generation(), crate::etats::marque_de_validation()),
    )
}

/// LE COMPTEUR AVANT UN MOIS, tel que la paie de ce mois l'ecrit pour le mois suivant (`etats::CONGES_AVANT`) : le
/// compteur tire des mois d'avant (`compteur_conges`), le nombre de ses avis qui viennent du report de la periode
/// precedente (les premiers), le premier mois ou le salarie a la paie d'un contrat paye au mois, et la reprise de
/// l'ancien logiciel telle que la fiche la portait. Le mois suivant le reprend et n'y ajoute que ce mois-ci (`suivant`) :
/// il ne relit aucun mois d'avant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Avant {
    pub compteur: CompteurConges,
    avis_du_report: usize,
    premier: Option<String>,
    reprise: Option<droit::fiche::CongesReprise>,
}

/// La reprise des conges telle que la fiche du salarie la porte (`conges_reprise`), `None` sans elle : ce que l'etat
/// avant un mois a pris pour acquis.
fn reprise_ecrite(ctx: &Contexte, cle_salarie: &str) -> Resultat<Option<droit::fiche::CongesReprise>> {
    let fiche = match ctx.salaries.get(cle_salarie) {
        Some(f) => f.clone(),
        None => ctx.salarie(cle_salarie)?,
    };
    Ok(fiche.conges_reprise.dite().cloned())
}

/// LE COMPTEUR QUI NE COMPTE PAS TOUT : sa periode la plus ancienne (`depuis`, le debut du report, sinon de la periode
/// en cours) commence avant le premier mois que PuppetPaie a paye au salarie, son contrat paye au mois a commence avant
/// ce mois, et sa fiche ne dit pas ses conges a la reprise (`conges_reprise`). Les conges acquis avant ce mois ne se
/// devinent pas (regle 1). Rend le debut du contrat et ce premier mois ; `None` : le compteur compte tout.
pub(crate) fn compte_sans_la_reprise(ctx: &Contexte, cle_salarie: &str, depuis: &Jour) -> Resultat<Option<(Jour, String)>> {
    if reprise_conges(ctx, cle_salarie)?.is_some() {
        return Ok(None);
    }
    let Some(premier) = premier_mois(ctx, cle_salarie, None)? else { return Ok(None) };
    if *depuis >= Mois::lire(&premier)?.premier_jour() {
        return Ok(None);
    }
    Ok(commence_avant(&paies_du_mois(ctx, &premier, &cle_de_fiche(ctx, cle_salarie)?)?, &premier)?.map(|debut| (debut, premier)))
}

/// Le compteur de conges d'un salarie avant `mois` (les mois strictement anterieurs), a la date `a_la_date` de ce mois,
/// tire des paies validees : ce qui n'est pas valide n'est pas paye, et aucun mois ne se calcule.
pub(crate) fn compteur_conges(ctx: &Contexte, cle_salarie: &str, a_la_date: &Jour, mois: &str) -> Resultat<CompteurConges> {
    Ok(avant_le_mois(ctx, cle_salarie, a_la_date, mois)?.compteur)
}

/// L'ETAT AVANT `mois` (`Avant`) : celui que la paie du salarie du mois d'avant a ecrit, plus ce mois d'avant
/// (`suivant`, qui ne lit que lui) ; sans lui (le premier mois paye, un mois sans paie, une reprise changee depuis),
/// tire de chaque mois de la periode (`compteur`).
pub(crate) fn avant_le_mois(ctx: &Contexte, cle_salarie: &str, a_la_date: &Jour, mois: &str) -> Resultat<Avant> {
    let _derivation = Derivation::ouvrir();
    if let Some(a) = suivant(ctx, cle_salarie, a_la_date, mois)? {
        return Ok(a);
    }
    let cle = cle_de_fiche(ctx, cle_salarie)?;
    let mensuels = mensuels_d_avant(ctx, mois, &cle)?;
    socle::fichiers::noter_valeur(&format!("{}{} {}", MENSUELS, mois, cle), empreinte_des(&mensuels));
    if mensuels.is_empty() {
        // Aucun mois d'avant ne se lit : `RIEN_AVANT` les tient tous pour d'apres.
        return compteur(ctx, cle_salarie, a_la_date, Some(RIEN_AVANT));
    }
    compteur(ctx, cle_salarie, a_la_date, Some(mois))
}

/// Un mois avant tout mois (`avant_le_mois`, `mois_de_la_periode`, `premier_mois`) : aucun ne se lit.
const RIEN_AVANT: &str = "0000-00";

/// La cle sous laquelle un calcul note les contrats payes au mois d'un salarie avant un mois (`mensuels <mois> <salarie>`).
pub const MENSUELS: &str = "mensuels ";

/// LE PREMIER MOIS D'UN PERMANENT NE RELIT RIEN : seul un mois paye au mois apporte des conges (`Tenue::ajouter`). Sans
/// contrat paye au mois du salarie `cle` (la cle de sa fiche) commence avant `mois` (un engagement actif, qui n'est pas
/// qu'importe), l'etat avant le mois est le fait de son entree : zero, ou la reprise de sa fiche (`conges_reprise`). Rend
/// ces contrats (leur premier mois et leur debut) ; la paie note leur empreinte (`empreinte_des_mensuels_d_avant`) : qu'un
/// contrat s'ajoute avant elle, et elle se refait.
fn mensuels_d_avant(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for o in crate::stock::objets(&ctx.racine)? {
        if o.fichier.as_str() < mois && !o.est_non_actif() && !o.importe_seul() && o.mensuel()? && o.carte().salarie.as_deref() == Some(cle) {
            out.push(format!("{}#{}", o.fichier, crate::debut_brut(&o.brut)));
        }
    }
    out.sort();
    Ok(out)
}

fn empreinte_des(mensuels: &[String]) -> String {
    socle::fichiers::sha256_hex(mensuels.join(";").as_bytes())
}

/// L'empreinte des contrats payes au mois du salarie `cle` commences avant `mois` (`mensuels_d_avant`).
pub(crate) fn empreinte_des_mensuels_d_avant(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<String> {
    Ok(empreinte_des(&mensuels_d_avant(ctx, mois, cle)?))
}

/// Ce qu'une periode de reference a acquis (avant le plafond), vu prendre et paye, a mesure que ses mois s'y ajoutent,
/// et ce qu'il y a a en dire.
struct Tenue {
    acquis: Decimal,
    pris: Decimal,
    brut: Decimal,
    avis: Vec<Avis>,
}

impl Tenue {
    /// Ce que la periode [debut, fin] tient de la reprise de l'ancien logiciel, avant tout mois produit ici.
    fn ouvrir(rep: Option<&Reprise>, debut: &Jour, fin: &Jour) -> Resultat<Tenue> {
        let mut t = Tenue { acquis: Decimal::ZERO, pris: Decimal::ZERO, brut: zero2(), avis: Vec::new() };
        let Some(r) = rep else { return Ok(t) };
        if *debut <= r.date && r.date <= *fin {
            t.acquis = add(t.acquis, r.acquis);
            t.pris += r.pris;
            t.brut = add(t.brut, r.remuneration_periode);
        } else if *fin == periode_de_reference(&r.date)?.0.plus_jours(-1)? {
            t.brut = add(t.brut, r.remuneration_report);
        }
        Ok(t)
    }

    /// Ce que les paies validees `paies` du salarie (`cle`, la cle de sa fiche) en `mois` apportent a la periode [debut,
    /// fin] : sa presence, ses jours pris, et, avec `remuneration`, son brut d'un mois ou il est present.
    #[allow(clippy::too_many_arguments)]
    fn ajouter(
        &mut self, ctx: &Contexte, cle_salarie: &str, cle: &str, rep: Option<&Reprise>, debut: &Jour, fin: &Jour, mois: &str,
        paies: &[PaieLue], remuneration: bool,
    ) -> Resultat<()> {
        let mut present = false;
        for m in paies.iter().filter_map(|p| p.mensuel.as_ref()) {
            // La presence du mois, bornee a la periode et a la reprise.
            let mut p_debut = m.debut.max(*debut);
            if let Some(r) = rep {
                p_debut = p_debut.max(r.date);
            }
            let p_fin = m.fin.min(*fin);
            if p_fin < p_debut {
                continue;
            }
            present = true;
            // La presence, moins les suspensions qui n'acquierent pas (`imprevus::jours_de_conges_non_acquis`).
            let non_acquis = crate::imprevus::jours_de_conges_non_acquis(
                &ctx.racine,
                cle,
                m.numero.as_deref(),
                &m.contrat_debut,
                &p_debut,
                &p_fin,
            )?;
            self.acquis = add(self.acquis, sub(jours_acquis(&p_debut, &p_fin)?, non_acquis).max(Decimal::ZERO));
            self.pris += m.pris;
            if m.arret {
                self.avis.push(avertir(format!(
                    "{} : un arret de travail en {}. Le compteur de conges compte \
                     le TEMPS DE PRESENCE d'un arret ; les periodes assimilees a \
                     du travail effectif pendant un arret (L3141-5) ne sont pas \
                     appliquees. A verifier a la main.",
                    cle_salarie, mois
                )));
            }
        }
        let fin_du_mois = fin_mois_de(mois)?;
        let avant_la_reprise = rep.is_some_and(|r| fin_du_mois < r.date);
        // L'assiette du dixieme : tout le brut du salarie dans un mois ou il est present.
        if remuneration && present && !avant_la_reprise {
            self.brut = paies.iter().fold(self.brut, |a, p| add(a, p.brut));
        }
        Ok(())
    }
}

/// Ce qu'une periode de reference a acquis et vu prendre :
/// `(acquis plafonne, pris, remuneration, avertissements)`. Sans
/// `remuneration`, le brut n'est pas somme.
fn periode(
    ctx: &Contexte,
    cle_salarie: &str,
    debut: &Jour,
    fin: &Jour,
    jusqu_au_mois: Option<&str>,
    remuneration: bool,
) -> Resultat<(Decimal, Decimal, Decimal, Vec<Avis>)> {
    let rep = reprise_conges(ctx, cle_salarie)?;
    let mut t = Tenue::ouvrir(rep.as_ref(), debut, fin)?;
    let cle = cle_de_fiche(ctx, cle_salarie)?;
    for (mois, paies) in mois_de_la_periode(ctx, &cle, debut, fin, jusqu_au_mois)? {
        t.ajouter(ctx, cle_salarie, &cle, rep.as_ref(), debut, fin, &mois, &paies, remuneration)?;
    }
    Ok((plafonner(t.acquis)?, t.pris, t.brut, t.avis))
}

fn compteur(
    ctx: &Contexte,
    cle_salarie: &str,
    a_la_date: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<Avant> {
    let (debut, fin) = periode_de_reference(a_la_date)?;
    let (acquis, pris, remuneration, avert) = periode(ctx, cle_salarie, &debut, &fin, jusqu_au_mois, true)?;
    let report = report(ctx, cle_salarie, &debut, jusqu_au_mois)?;
    // Un jour du report se paie au dixieme de la periode ou il a ete acquis (`indemnite_des_jours`).
    let remuneration_report = match (report.debut, report.fin) {
        (Some(rd), Some(rf)) if report.reports > Decimal::ZERO => periode(ctx, cle_salarie, &rd, &rf, jusqu_au_mois, true)?.2,
        _ => zero2(),
    };
    let avis_du_report = report.avertissements.len();
    let mut avertissements = report.avertissements;
    avertissements.extend(avert);
    Ok(Avant {
        compteur: CompteurConges {
            salarie: cle_salarie.to_string(),
            periode_debut: debut,
            periode_fin: fin,
            acquis,
            pris,
            remuneration_periode: remuneration,
            reports: report.reports,
            avertissements,
            report_debut: report.debut,
            report_fin: report.fin,
            acquis_report: report.acquis,
            remuneration_report,
            pris_sur_la_periode: pris_de_la_reprise(ctx, cle_salarie, &debut, &fin)?,
        },
        avis_du_report,
        premier: report.premier,
        reprise: reprise_ecrite(ctx, cle_salarie)?,
    })
}

/// L'ETAT AVANT `mois` TIRE DE CELUI DU MOIS D'AVANT : l'etat que la paie du salarie de `mois - 1` a ecrit (avant
/// `mois - 1`), plus ce que les paies validees de `mois - 1` apportent, comme `compteur` l'aurait tire de tous les mois :
/// dans la meme periode de reference, le mois d'avant s'ajoute a la periode ; au passage d'une periode, la periode qui
/// finit fait son report (le pas que `report` ferait pour elle) et la nouvelle s'ouvre. `None` : rien a reprendre (le
/// mois d'avant n'a pas d'etat ecrit pour ce salarie, ou la reprise de sa fiche a change depuis), ou une periode de
/// reference qui ne commence pas un premier du mois.
fn suivant(ctx: &Contexte, cle_salarie: &str, a_la_date: &Jour, mois: &str) -> Resultat<Option<Avant>> {
    let (debut, fin) = periode_de_reference(a_la_date)?;
    if debut.jour != 1 {
        return Ok(None);
    }
    let precedent = Mois::lire(mois)?.premier_jour().plus_jours(-1)?.mois_civil().to_string();
    let cle = cle_de_fiche(ctx, cle_salarie)?;
    let paies = paies_du_mois(ctx, &precedent, &cle)?;
    let Some(a) = paies.iter().find_map(|p| p.avant.clone()) else { return Ok(None) };
    if a.reprise != reprise_ecrite(ctx, cle_salarie)? {
        return Ok(None);
    }
    let rep = reprise_conges(ctx, cle_salarie)?;
    let c = a.compteur;
    if a.avis_du_report > c.avertissements.len() {
        return Err(socle::echec::cle("Le compteur de congés du mois d'avant ne se relit pas : revalider l'engagement de ce mois.", "avis_du_report"));
    }
    let (q_debut, q_fin) = (c.periode_debut, c.periode_fin);
    let mut tenue = Tenue { acquis: c.acquis, pris: c.pris, brut: c.remuneration_periode, avis: c.avertissements[a.avis_du_report..].to_vec() };
    tenue.ajouter(ctx, cle_salarie, &cle, rep.as_ref(), &q_debut, &q_fin, &precedent, &paies, true)?;
    let mut avis_du_report = c.avertissements[..a.avis_du_report].to_vec();
    let mensuel_au_precedent = paies.iter().any(|p| p.mensuel.is_some());
    let premier = a.premier.clone().or_else(|| mensuel_au_precedent.then(|| precedent.clone()));
    if rep.is_none() && a.premier.is_none() {
        if let Some(avant) = commence_avant(&paies, &precedent)? {
            avis_du_report.push(avis_commence_avant(cle_salarie, &avant, &precedent));
        }
    }
    let fini = |tenue: Tenue, reports, report_debut, report_fin, acquis_report, remuneration_report, pris_sur_la_periode, avis_du_report: Vec<Avis>| -> Resultat<Avant> {
        let n = avis_du_report.len();
        let mut avertissements = avis_du_report;
        avertissements.extend(tenue.avis);
        Ok(Avant {
            compteur: CompteurConges {
                salarie: cle_salarie.to_string(),
                periode_debut: debut,
                periode_fin: fin,
                acquis: plafonner(tenue.acquis)?,
                pris: tenue.pris,
                remuneration_periode: tenue.brut,
                reports,
                avertissements,
                report_debut,
                report_fin,
                acquis_report,
                remuneration_report,
                pris_sur_la_periode,
            },
            avis_du_report: n,
            premier: premier.clone(),
            reprise: a.reprise.clone(),
        })
    };
    if (q_debut, q_fin) == (debut, fin) {
        return fini(tenue, c.reports, c.report_debut, c.report_fin, c.acquis_report, c.remuneration_report, c.pris_sur_la_periode, avis_du_report).map(Some);
    }
    if q_fin.plus_jours(1)? != debut {
        return Ok(None);
    }
    // La periode qui finit fait son report, si `report` la parcourt : depuis la periode de la reprise, ou depuis celle
    // du premier mois paye.
    let parcourue = match &rep {
        Some(r) => periode_de_reference(&r.date)?.0 < debut,
        None => premier.is_some(),
    };
    let (mut reste, mut acquis_report, mut report_debut, mut report_fin) = (c.reports, c.acquis_report, c.report_debut, c.report_fin);
    if parcourue {
        // L3141-7 : la periode qui se clot donne sa duree de conge, a l'entier superieur.
        let acquis_total = duree_du_conge(plafonner(tenue.acquis)?)?;
        let deja = c.pris_sur_la_periode;
        let (pris, acquis) = (sub(tenue.pris, deja), sub(acquis_total, deja));
        let sur_reste = pris.min(reste.max(Decimal::ZERO));
        let perdu = sub(reste.max(Decimal::ZERO), sur_reste);
        if perdu > Decimal::ZERO {
            avis_du_report.push(avis_perdu(cle_salarie, perdu, &q_debut, &q_fin));
        }
        reste = add(sub(acquis, sub(pris, sur_reste)), reste.min(Decimal::ZERO));
        acquis_report = acquis_total;
        report_debut = Some(q_debut);
        report_fin = Some(q_fin);
    }
    // Un jour du report se paie au dixieme de la periode ou il a ete acquis (`indemnite_des_jours`).
    let remuneration_report = match (report_debut, report_fin) {
        (Some(_), Some(_)) if reste > Decimal::ZERO && parcourue => tenue.brut,
        (Some(_), Some(_)) if reste > Decimal::ZERO => c.remuneration_report,
        _ => zero2(),
    };
    let ouverte = Tenue::ouvrir(rep.as_ref(), &debut, &fin)?;
    let pris_sur_la_periode = pris_de_la_reprise(ctx, cle_salarie, &debut, &fin)?;
    fini(ouverte, reste, report_debut, report_fin, acquis_report, remuneration_report, pris_sur_la_periode, avis_du_report).map(Some)
}

/// L'avis d'un contrat commence avant le premier mois que ce logiciel a paye.
fn avis_commence_avant(cle_salarie: &str, avant: &Jour, premier: &str) -> Avis {
    avertir(format!(
        "{} : le contrat a commence le {}, avant le premier mois \
         produit par PuppetPaie ({}). Les conges acquis avant ne \
         sont pas comptes : les recopier du dernier bulletin de \
         l'ancien logiciel (fiche salarie, conges payes a la reprise).",
        cle_salarie,
        avant.jj_mm_aaaa(),
        premier
    ))
}

/// L'avis des jours d'une periode de prise qui finit sans avoir ete pris.
fn avis_perdu(cle_salarie: &str, perdu: Decimal, p_debut: &Jour, p_fin: &Jour) -> Avis {
    avertir(format!(
        "{} : {} jour(s) de conges acquis avant le {} n'ont pas ete \
         pris au {}. Ils ne sont PAS reportes une seconde fois : un \
         conge non pris a la fin de la periode de prise est perdu, \
         sauf accord de report ou empechement (maladie : L3141-19-1). \
         Si c'est le cas, a reprendre a la main.",
        cle_salarie,
        arrondir_jours(perdu),
        p_debut.jj_mm_aaaa(),
        p_fin.jj_mm_aaaa()
    ))
}

fn pris_de_la_reprise(ctx: &Contexte, cle_salarie: &str, debut: &Jour, fin: &Jour) -> Resultat<Decimal> {
    Ok(match reprise_conges(ctx, cle_salarie)? {
        Some(r) if *debut <= r.date && r.date <= *fin => r.pris,
        _ => Decimal::ZERO,
    })
}

/// Le report de la periode precedente, periode par periode depuis le premier
/// mois produit : un jour pris consomme d'abord ce qui reste de la periode
/// precedente, puis, par anticipation, la periode en cours.
struct Report {
    reports: Decimal,
    acquis: Decimal,
    debut: Option<Jour>,
    fin: Option<Jour>,
    avertissements: Vec<Avis>,
    /// Le premier mois ou le salarie a la paie d'un contrat paye au mois (`premier_mois`).
    premier: Option<String>,
}

fn report(ctx: &Contexte, cle_salarie: &str, debut_courant: &Jour, jusqu_au_mois: Option<&str>) -> Resultat<Report> {
    let vide = Report { reports: Decimal::ZERO, acquis: Decimal::ZERO, debut: None, fin: None, avertissements: vec![], premier: None };
    let rep = reprise_conges(ctx, cle_salarie)?;
    let premier = premier_mois(ctx, cle_salarie, jusqu_au_mois)?;
    let mut out = Report { avertissements: vec![], ..vide };
    let mut avert = Vec::new();
    let (mut p_debut, mut p_fin, mut reste);
    if let Some(r) = &rep {
        // La reprise : le dernier bulletin de l'ancien logiciel, son reste
        // N-1, et la periode N-1 elle-meme.
        (p_debut, p_fin) = periode_de_reference(&r.date)?;
        reste = r.report;
        let avant_fin = p_debut.plus_jours(-1)?;
        let avant_debut = periode_de_reference(&avant_fin)?.0;
        out.acquis = r.acquis_report;
        out.debut = Some(avant_debut);
        out.fin = Some(avant_fin);
        if reste > Decimal::ZERO && r.remuneration_report == Decimal::ZERO {
            avert.push(info(format!(
                "{} : {} jour(s) repris de l'ancien logiciel sans le brut de \
                 leur periode (fiche salarie, conges a la reprise) : ils se \
                 paient au maintien de salaire, sans comparaison au dixieme \
                 (L3141-24).",
                cle_salarie,
                arrondir_jours(reste)
            )));
        }
    } else if let Some(premier) = &premier {
        (p_debut, p_fin) = periode_de_reference(&Mois::lire(premier)?.premier_jour())?;
        reste = Decimal::ZERO;
        if let Some(avant) = commence_avant(&paies_du_mois(ctx, premier, &cle_de_fiche(ctx, cle_salarie)?)?, premier)? {
            avert.push(avis_commence_avant(cle_salarie, &avant, premier));
        }
    } else {
        return Ok(Report { avertissements: vec![], premier, ..out });
    }
    while p_debut < *debut_courant {
        let (acquis, pris, _, _) = periode(ctx, cle_salarie, &p_debut, &p_fin, jusqu_au_mois, false)?;
        // Les jours deja pris par l'ancien logiciel sur SA periode N sont
        // imputes sur N, pas sur le report.
        let deja = pris_de_la_reprise(ctx, cle_salarie, &p_debut, &p_fin)?;
        // L3141-7 : la periode close donne sa duree de conge, a l'entier superieur.
        let acquis = duree_du_conge(acquis)?;
        let acquis_total = acquis;
        let (pris, acquis) = (sub(pris, deja), sub(acquis, deja));
        let sur_reste = pris.min(reste.max(Decimal::ZERO));
        let perdu = sub(reste.max(Decimal::ZERO), sur_reste);
        if perdu > Decimal::ZERO {
            avert.push(avis_perdu(cle_salarie, perdu, &p_debut, &p_fin));
        }
        // Un reste negatif (une avance deja prise) s'impute sur les jours
        // que la periode vient d'acquerir.
        reste = add(sub(acquis, sub(pris, sur_reste)), reste.min(Decimal::ZERO));
        out.acquis = acquis_total;
        out.debut = Some(p_debut);
        out.fin = Some(p_fin);
        let suivant = p_fin.plus_jours(1)?;
        (p_debut, p_fin) = periode_de_reference(&suivant)?;
    }
    out.reports = reste;
    out.avertissements = avert;
    out.premier = premier;
    Ok(out)
}

/// Le solde de conges repris de l'ancien logiciel (fiche `conges_reprise`).
#[derive(Debug, Clone)]
pub struct Reprise {
    pub date: Jour,
    pub report: Decimal,
    pub acquis_report: Decimal,
    pub acquis: Decimal,
    pub pris: Decimal,
    pub remuneration_report: Decimal,
    pub remuneration_periode: Decimal,
}

/// Le solde repris de l'ancien logiciel, ou `None` (la fiche n'en dit pas, ou ne dit pas sa date). Datee, la reprise dit
/// chacun de ses nombres (0 s'il n'y en a pas) ; un montant illisible s'est refuse a la lecture de la fiche
/// (`fiche::Montant`).
pub(crate) fn reprise_conges(ctx: &Contexte, cle_salarie: &str) -> Resultat<Option<Reprise>> {
    let Some(r) = reprise_ecrite(ctx, cle_salarie)? else { return Ok(None) };
    let Some(date) = r.date.dite().copied() else { return Ok(None) };
    let nombre = |c: &droit::fiche::Case<droit::fiche::Montant>, cle: &str| {
        c.dite().map(droit::fiche::Montant::decimal).ok_or_else(|| {
            socle::echec::cle(
                format!("La reprise des congés du {} ne dit pas tous ses nombres : écrire chacun sur la fiche du salarié, 0 s'il n'y en a pas.", date.jj_mm_aaaa()),
                cle,
            )
        })
    };
    Ok(Some(Reprise {
        date,
        report: nombre(&r.report, "conges_reprise.report")?,
        acquis_report: nombre(&r.acquis_report, "conges_reprise.acquis_report")?,
        acquis: nombre(&r.acquis, "conges_reprise.acquis")?,
        pris: nombre(&r.pris, "conges_reprise.pris")?,
        remuneration_report: nombre(&r.remuneration_report, "conges_reprise.remuneration_report")?,
        remuneration_periode: nombre(&r.remuneration_periode, "conges_reprise.remuneration_periode")?,
    }))
}

/// La date de debut du contrat paye au mois que portent les paies `paies` du salarie en `mois`, si elle est anterieure
/// a ce mois ; sinon `None`. Elle se lit dans l'engagement de la paie.
fn commence_avant(paies: &[PaieLue], mois: &str) -> Resultat<Option<Jour>> {
    let debut_du_mois = Mois::lire(mois)?.premier_jour();
    Ok(paies.iter().filter_map(|p| p.mensuel.as_ref()).map(|m| m.contrat_debut).find(|debut| *debut < debut_du_mois))
}

/// Le premier mois ou le salarie a la paie validee d'un contrat paye au mois.
fn premier_mois(ctx: &Contexte, cle_salarie: &str, jusqu_au_mois: Option<&str>) -> Resultat<Option<String>> {
    let cle = cle_de_fiche(ctx, cle_salarie)?;
    for mois in crate::stock::mois_saisis(&ctx.racine)?.into_iter().map(String::from) {
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) {
            return Ok(None);
        }
        if paies_du_mois(ctx, &mois, &cle)?.iter().any(|p| p.mensuel.is_some()) {
            return Ok(Some(mois));
        }
    }
    Ok(None)
}

/// Les mois qui croisent la periode, du plus ancien au plus recent, avec ce que les paies validees du salarie `cle`
/// y apportent (`paies_du_mois`) : un contrat paye au mois a une paie dans chacun de ses mois.
fn mois_de_la_periode(
    ctx: &Contexte,
    cle: &str,
    debut: &Jour,
    fin: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<Vec<(String, Vec<PaieLue>)>> {
    let mut out = Vec::new();
    if fin < debut {
        return Ok(out);
    }
    let saisis = crate::stock::mois_saisis(&ctx.racine)?;
    for m in socle::mois::Mois::entre(debut, fin) {
        let mois = m.to_string();
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) || !saisis.contains(&m) {
            continue;
        }
        let paies = paies_du_mois(ctx, &mois, cle)?;
        out.push((mois, paies));
    }
    Ok(out)
}

/// Ce que les paies validees du salarie `cle` (la cle de sa fiche) en `mois` apportent (`etats::paies_validees`, `lue`),
/// lu une fois par derivation.
fn paies_du_mois(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Vec<PaieLue>> {
    let reference = cle_du_memo(ctx, mois, cle);
    let memo = MEMO.with(|m| {
        let m = m.borrow();
        if m.0 > 0 { m.1.get(&reference).cloned() } else { None }
    });
    if let Some(p) = memo {
        return p;
    }
    let p = crate::etats::paies_validees(ctx, mois, cle).and_then(|paies| paies.iter().map(|p| lue(ctx, mois, p)).collect());
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 > 0 {
            m.1.insert(reference, p.clone());
        }
    });
    p
}

/// La cle de fiche d'un salarie tel qu'une saisie ou un appelant le nomme.
fn cle_de_fiche(ctx: &Contexte, cle: &str) -> Resultat<String> {
    Ok(ctx.salarie(cle)?.id.to_string())
}

/// Le brut d'un salarie sur un mois anterieur -- l'assiette du dixieme -- tel que ses paies validees le portent.
pub(crate) fn remuneration_du_mois(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    Ok(paies_du_mois(ctx, mois, &cle_de_fiche(ctx, cle)?)?.iter().fold(zero2(), |a, p| add(a, p.brut)))
}

/// La deduction forfaitaire specifique deja retenue pour un salarie dans les mois de la meme annee civile, strictement
/// avant `mois` (AAAA-MM), telle que ses paies validees la portent (`calcul::dfs_deja_deduite` ; arrete du 4 septembre
/// 2025, art. 9 : << dans la limite de 7 600 euros par annee civile >>). Une paie validee qui ne se lit pas refuse : le
/// cumul de l'annee ne se devine pas.
pub(crate) fn dfs_des_mois_anterieurs(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    cumul_de_l_annee(ctx, mois, cle, |p| p.dfs, "Déduction forfaitaire spécifique", "Le plafond de 7 600 € par année civile (arrêté du 4 septembre 2025, art. 9)")
}

/// Les heures supplementaires payees au salarie `cle` dans les mois de la meme annee civile, strictement avant `mois` :
/// les paliers annuels d'une convention et son contingent les comptent (`droit::heures_en_plus::supplementaires`).
pub(crate) fn heures_supplementaires_des_mois_anterieurs(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    cumul_de_l_annee(ctx, mois, cle, |p| p.heures_supplementaires, "Heures supplémentaires", "Le contingent annuel et ses paliers")
}

/// Le montant net des heures en plus deja exonere d'impot pour le salarie `cle` dans l'annee civile, avant `mois` : la
/// limite annuelle de CGI 81 quater le compte (`heures_en_plus`).
pub(crate) fn exonere_des_mois_anterieurs(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    cumul_de_l_annee(ctx, mois, cle, |p| p.exonere, "Exonération d'impôt des heures en plus", "La limite annuelle de 7 500 € (CGI 81 quater)")
}

/// LE CUMUL DE L'ANNEE CIVILE, strictement avant `mois`, de ce que `quoi` lit dans chaque paie validee du salarie `cle`.
/// Une paie validee qui ne se lit pas refuse : le cumul de l'annee ne se devine pas.
fn cumul_de_l_annee(ctx: &Contexte, mois: &str, cle: &str, quoi: impl Fn(&PaieLue) -> Decimal, qui: &str, regle: &str) -> Resultat<Decimal> {
    let _derivation = Derivation::ouvrir();
    let annee: String = mois.chars().take(4).collect();
    let cle_fiche = cle_de_fiche(ctx, cle)?;
    let mut total = zero2();
    for m in crate::stock::mois_saisis(&ctx.racine)?.into_iter().map(String::from) {
        if !m.starts_with(&annee) || m.as_str() >= mois {
            continue;
        }
        let paies = paies_du_mois(ctx, &m, &cle_fiche).map_err(|e| {
            format!(
                "{} de {} : les paies validées de {} ne se lisent pas ({}). {} se suit sur toute l'année : sans ce mois, le cumul \
ne se connaît pas. Rien n'est calculé.",
                qui, cle, m, e, regle
            )
        })?;
        total = paies.iter().fold(total, |a, p| add(a, quoi(p)));
    }
    Ok(total)
}

// ------------------------------------------------------ l'indemnite

/// L'indemnite de `jours` de conges du compteur `cpt` (`droit::permanents::indemnite_des_jours`, L3141-24).
pub(crate) fn indemnite_des_jours(cpt: &CompteurConges, jours: Decimal, maintien: Decimal) -> Resultat<IndemniteDesJours> {
    cpt.indemnite_des_jours(jours, maintien)
}

/// Le compteur tel que le BULLETIN du mois doit le montrer : celui d'avant (`Avant`, rendu avec lui), plus ce que ce
/// mois acquiert et prend.
pub(crate) fn compteur_au_bulletin(ctx: &Contexte, paie: &Paie, mois: &str) -> Resultat<(CompteurConges, Avant)> {
    let eng = &paie.engagement;
    let cle = eng.salarie.as_str();
    let fin_du_mois = fin_mois_de(mois)?;
    let avant = avant_le_mois(ctx, cle, &fin_du_mois, mois)?;
    let mut cpt = avant.compteur.clone();
    let (debut, fin) = periode_de_reference(&fin_du_mois)?;
    // La presence de CE mois, bornee a la periode de reference.
    let p_debut = eng.debut_periode()?.max(debut);
    let p_fin = eng.fin_periode()?.min(fin);
    if p_fin >= p_debut {
        let cle_fiche = cle_de_fiche(ctx, cle)?;
        let non_acquis =
            crate::imprevus::jours_de_conges_non_acquis(&ctx.racine, &cle_fiche, eng.numero.as_deref(), &eng.debut, &p_debut, &p_fin)?;
        cpt.acquis = plafonner(add(cpt.acquis, sub(jours_acquis(&p_debut, &p_fin)?, non_acquis).max(Decimal::ZERO)))?;
    }
    cpt.pris += conges_pris_de_la_paie(&ctx.racine, paie)?;
    Ok((cpt, avant))
}

/// LE COMPTEUR QUE LA VALIDATION ECRIT dans la paie `p` de `mois` (`etats::CONGES`) : celui que son bulletin montre
/// (`compteur_au_bulletin`), pour un contrat paye au mois dont l'employeur tient les conges, et l'etat avant le mois
/// que le mois suivant reprend (`etats::CONGES_AVANT`). Un contrat declare a la caisse des Conges Spectacles
/// (`calcul::declaree_a_la_caisse_des_conges`, D. 7121-41) n'en a pas : la caisse lui paie ses conges.
pub(crate) fn compteur_ecrit(ctx: &Contexte, p: &Paie, mois: &str) -> Resultat<Option<(CompteurConges, Avant)>> {
    if !p.engagement.nat()?.mensuelle() || crate::calcul::declaree_a_la_caisse_des_conges(p)? {
        return Ok(None);
    }
    compteur_au_bulletin(ctx, p, mois).map(Some)
}

/// Les jours de conges que CETTE paie decompte, lus sur ses lignes : le
/// catalogue marque la rubrique.
pub(crate) fn conges_pris_de_la_paie(racine: &Path, paie: &Paie) -> Resultat<Decimal> {
    let jour = &paie.engagement.fin_periode()?;
    let mut total = Decimal::ZERO;
    for l in &paie.engagement.lignes {
        if rubrique(racine, l.rubrique(), jour)?.conges_pris {
            total += l.quantite().ok_or_else(|| socle::fichiers::absent("quantite"))?;
        }
    }
    Ok(total)
}

// ------------------------------------------------- les lignes d'un mois paye

/// Les lignes de paie d'un mois de permanent, et ce qu'il y a a en dire.
/// L'ordre des lignes est celui du bulletin : le salaire de base, les
/// absences, les indemnites, les primes, le solde de tout compte, puis les repas et les nuitees que la carte porte en
/// lignes, aux limites de tout salarie (`crate::frais_au_dela`). Et les jours travailles du mois.
pub(crate) fn lignes_du_mois(ctx: &Contexte, e: &Carte, mois: &str) -> Resultat<(Vec<LignePaie>, Vec<Avis>, Option<i64>)> {
    let nat = e.nature_lue()?;
    if !nat.mensuelle() {
        return Err(format!("{} n'est pas un contrat paye au mois : ses lignes se saisissent.", nat.libelle));
    }
    let salaire = match &e.salaire_mensuel {
        Some(s) => s,
        None => {
            return Err(format!(
                "{} : saisissez le salaire mensuel brut de son {}, dans la case « Salaire mensuel brut ».\n\
                 Il se lit sur le contrat signé.",
                qui(ctx, e),
                nat.libelle
            ))
        }
    };
    let defaut = dec_f64(nat.quotite_reference_defaut().ok_or_else(|| socle::echec::livree("contrats-base.json", "quotite_reference_defaut"))?)?;
    let quotite = dec_ou(e.quotite_contrat.as_ref(), defaut)?;
    let salaire = dec_de(salaire)?;

    let debut_mois = Mois::lire(mois)?.premier_jour();
    let fin_mois = fin_mois_de(mois)?;
    // La date de debut d'un contrat paye au mois : la meme phrase que `engagement::Engagement` (une saisie neuve sans
    // debut ne refuse pas sur « champ 'debut' absent »).
    let Some(debut) = e.debut.as_deref().filter(|x| !x.is_empty()) else {
        return Err(format!(
            "{} : saisissez la date de début de son {}, celle du contrat signé, sous « Contrat ».\n\
             Elle identifie le contrat dans la déclaration (S21.G00.40.001) : elle ne se déduit de rien.",
            qui(ctx, e),
            nat.libelle
        ));
    };
    let debut_saisi = Jour::lire(debut)?;
    let debut = debut_saisi.max(debut_mois);
    let fin = match jour_dit(&e.fin)? {
        Some(f) => f.min(fin_mois),
        None => fin_mois,
    };
    // Le salaire d'un mois incomplet : l'horaire reel du mois, ou la regle de la convention qui en a une
    // (`droit::periodes::part_du_mois`).
    let salarie = salarie_de(e)?.to_string();
    let regle = regle_du_mois_incomplet(ctx, &salarie, &debut, &fin, &debut_mois, &fin_mois)?;
    let horaire = horaire_du_mois(ctx, &salarie, &debut_saisi, &debut_mois, &fin_mois)?;
    let part = droit::periodes::part_du_mois(&debut, &fin, &fin_mois, regle, &horaire)?;
    let fraction = part.fraction;
    let heures_par_jour = droit::conges::heures_par_jour(quotite, &horaire.jours);
    let mut avert = Vec::new();
    if fraction != Decimal::ONE {
        let quoi = if debut > debut_mois || fin < fin_mois { "mois incomplet" } else { "mois payé en partie" };
        avert.push(info(format!(
            "{} : {}, du {} au {} : {}.",
            qui(ctx, e),
            quoi,
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa(),
            part.methode(heures_par_jour)
        )));
    }

    // `heures` est par unite (`Rubrique::heures`) : un mois vaut la quotite,
    // et la fraction, en `quantite`, en fait la part ; `precision` la dit sur le bulletin.
    let mois_paye = LignePaie::Composee(crate::ligne::Composee {
        rubrique: "mois".to_string(),
        quantite: dec_f64(flot(fraction))?,
        taux: dec_f64(flot(salaire))?,
        heures: Some(dec_f64(flot(eur2(quotite)))?),
        precision: part.sur_le_bulletin(heures_par_jour),
        dates: Vec::new(),
    });
    // Le salaire du mois tel que la ligne « mois » le paie (`calcul::lignes_brut` : le montant arrondi ligne par ligne).
    let salaire_du_mois = socle::num::eur(mul(dec_f64(flot(salaire))?, dec_f64(flot(fraction))?));
    let mut lignes = vec![mois_paye];
    // L'entreprise dit quels jours feries elle chome.
    let chomes = ctx.entreprise.feries_chomes();
    let ouvrables = Decimal::from(jours_ouvrables(&debut, &fin, chomes)?);
    let ouvrables_du_mois = if ouvrables != Decimal::ZERO { ouvrables } else { Decimal::ONE };
    let taux_jour_ouvrable = eur2(div(mul(salaire, fraction), ouvrables_du_mois));

    // --- les conges pris : une retenue, et une indemnite qui la couvre
    let mut pris = Decimal::ZERO;
    for c in e.conges_pris.iter().flatten() {
        pris += Decimal::from(jours_ouvrables_de(c, chomes)?);
    }
    if pris != Decimal::ZERO {
        let cpt = compteur_conges(ctx, &salarie, &fin, mois)?;
        let maintien = eur2(mul(taux_jour_ouvrable, pris));
        let ind = indemnite_des_jours(&cpt, pris, maintien)?;
        lignes.push(LignePaie::composee("absence_conges_payes", dec_f64(flot(pris))?, dec_f64(flot(taux_jour_ouvrable))?));
        lignes.push(LignePaie::composee("indemnite_conges_payes", dec_f64(flot(pris))?, dec_f64(flot(taux_au_montant(ind.retenu, pris)))?));
        avert.push(info(format!(
            "{} : {} jour(s) ouvrable(s) de conges. Indemnite retenue : {} \
             ({}) -- le dixieme donnait {}, le maintien {} (L3141-24). \
             Solde apres ce mois : {} jours.",
            qui(ctx, e),
            pris,
            ind.retenu,
            ind.regle,
            ind.dixieme,
            ind.maintien,
            arrondir_jours(sub(cpt.solde(), pris))
        )));
        if cpt.solde() < pris {
            avert.push(avertir(format!(
                "{} : {} jours de conges pris pour un solde de {}. Le \
                 logiciel ne refuse pas -- une avance sur conges se decide --, \
                 mais le compteur passera en negatif.",
                qui(ctx, e),
                pris,
                arrondir_jours(cpt.solde())
            )));
        }
        avert.extend(cpt.avertissements.iter().cloned());
        avert.extend(avertir_feries(ctx, e, &debut, &fin)?);
    }

    // --- les imprevus : un arret de travail, une suspension, saisis une fois sur le salarie (`imprevus`)
    let (l, a, absents) = crate::imprevus::lignes_du_mois(
        ctx,
        e,
        mois,
        &crate::imprevus::MoisDuContrat { debut, fin, fin_mois, salaire_du_mois, heures_par_jour, jours_payes: part.jours, horaire: &horaire, taux_jour_ouvrable },
    )?;
    lignes.extend(l);
    avert.extend(a);
    // LES JOURS TRAVAILLES DU MOIS (S21.G00.53, unite 12 ; le certificat d'emploi Conges Spectacles) : ceux que l'horaire
    // du salarie programme dans le contrat (`droit::conges::JoursTravailles`, l'horaire reel de `part_du_mois`, les feries
    // qu'il ne voit pas payes hors), moins ceux qu'un arret ou une suspension non remuneree retire. La caisse les demande
    // pour « les autres emplois » que l'artiste : « renseigner l'unite de mesure en jours (code 12) » (Audiens, fiche de
    // specificites, `atelier/reference/sources/audiens-dsn/audiens-fiche-specificites-conges-spectacles-20261023.txt`,
    // l. 96-102). INFERENCE : aucun texte depose ne dit comment compter les jours d'un contrat paye au mois ; les jours
    // que son horaire programme sont ceux de son salaire (fiche DSN 2640, `droit::conges::JoursTravailles`).
    let reel = droit::periodes::part_du_mois(&debut, &fin, &fin_mois, droit::periodes::RegleDuMoisIncomplet::HoraireReel, &horaire)?;
    let jours_travailles = (reel.jours - absents).max(0);

    // --- les primes
    for pr in e.primes.iter().flatten() {
        let montant = Chiffre::Nombre(pr.montant.clone()).flottant()?;
        lignes.push(LignePaie::composee(&pr.rubrique, Decimal::ONE, dec_f64(montant)?));
    }

    // --- les heures supplementaires d'un temps plein, complementaires d'un temps partiel
    // Le temps partiel comme la DSN le declare (`Engagement::modalite_temps_travail`) : une duree du contrat inferieure a
    // la plus courte de la duree legale et de la quotite de reference (C. trav. L3123-1).
    let reference = match e.quotite_reference.as_ref().filter(|q| Chiffre::Nombre((*q).clone()).vrai()) {
        Some(r) => dec_de(r)?.min(defaut),
        None => defaut,
    };
    let partiel = match e.modalite_temps_travail.as_deref().filter(|m| !m.is_empty()) {
        Some(m) => m == "20",
        None => quotite < reference,
    };
    let (l, a) = crate::heures_en_plus::lignes(ctx, e, mois, salaire, quotite, partiel)?;
    lignes.extend(l);
    avert.extend(a);

    // --- la prime d'anciennete de l'annexe I de la 3090
    let mut fiche = ctx.salarie(&salarie)?.champs;
    let cle = ctx.cle_du_salarie(&salarie)?.to_string();
    // Les annees de presence que les suspensions laissent (L1225-54 : `imprevus::anciennete_depuis`).
    let mut sur_l_anciennete = Vec::new();
    let depuis = crate::calcul::date_d_anciennete(&fiche, &debut_saisi);
    let (retenu, dit) = crate::imprevus::anciennete_depuis(&ctx.racine, &cle, &depuis, &fin_mois)?;
    if retenu != depuis {
        fiche.date_anciennete = droit::fiche::Case::Dite(retenu);
        sur_l_anciennete = dit;
    }
    let ent = &ctx.entreprise;
    let jauge = ent.jauge_annexe_1.dite().map(droit::fiche::Choix::code);
    let pa = clauses::prime_anciennete(ent.idcc.texte(), ent.annexe_convention.code(), &fiche, &crate::solde::dit_par_l_engagement(e)?, &fin_mois, jauge)?;
    if let Some(pa) = pa {
        let Some(base_conv) = pa.base.map(|b| socle::num::dec_texte(&b.to_string())).transpose()? else {
            return Err(format!("{} : prime d'anciennete ({}) -- {}.", qui(ctx, e), pa.article, pa.pourquoi.into_iter().collect::<String>()));
        };
        let pourcentage = Decimal::from(pa.pourcentage);
        let reference = Decimal::from_str("151.67").unwrap();
        let base = eur2(div(mul(base_conv, quotite), reference));
        let montant = eur2(div(mul(base, pourcentage), Decimal::ONE_HUNDRED));
        lignes.push(LignePaie::composee("prime_anciennete", Decimal::ONE, dec_f64(flot(montant))?));
        let proratise = if quotite != reference { format!(" proratisé à {} h", virgule(quotite)) } else { String::new() };
        avert.push(info(format!(
            "{} : prime d'ancienneté {} % ({} an(s) de présence) du salaire \
             conventionnel de {}{} = {} -- {}.",
            qui(ctx, e),
            virgule(pourcentage),
            pa.annees,
            euros(base_conv),
            proratise,
            euros(montant),
            pa.article
        )));
        // Ce que les suspensions ont retire des annees de presence se dit avec la prime.
        avert.extend(sur_l_anciennete);
    }

    // --- le solde de tout compte, le mois ou le contrat se termine
    if let Some(f) = jour_dit(&e.fin)? {
        if debut_mois <= f && f <= fin_mois {
            let (l, a) = crate::solde::solde_de_tout_compte(ctx, e, mois, &lignes)?;
            lignes.extend(l);
            avert.extend(a);
        }
    }

    // --- les repas et les nuitees de la carte
    if let Some(l) = &e.lignes {
        let (l, a) = crate::frais_au_dela(ctx, e.salarie.as_deref(), l.clone(), mois, &qui(ctx, e))?;
        lignes.extend(l);
        avert.extend(a);
    }
    Ok((lignes, avert, Some(jours_travailles)))
}

/// Ce que le moteur a retire du decompte des jours ouvrables, et ce qu'il n'a
/// pas pu retirer : a chaque mois qui porte un ferie.
fn avertir_feries(ctx: &Contexte, e: &Carte, debut: &Jour, fin: &Jour) -> Resultat<Vec<Avis>> {
    let dits = ctx.entreprise.feries_chomes();
    let qui = qui(ctx, e);
    let tous = feries::chomes_entre(debut, fin, Some(feries::TOUS))?;
    if tous.is_empty() {
        return Ok(vec![]);
    }
    let chomes = feries::chomes_entre(debut, fin, dits)?;
    let liste = |l: &feries::Jours| {
        let mut l = l.clone();
        l.sort();
        l.iter().map(|(j, nom)| (j.jj_mm_aaaa(), nom.clone())).collect::<Vec<_>>()
    };
    let mut out = Vec::new();
    if !chomes.is_empty() {
        out.push(info(format!(
            "{} : {} ferie(s) chome(s) dans ce mois, retire(s) du decompte \
             des jours ouvrables -- {}.",
            qui,
            chomes.len(),
            liste(&chomes).iter().map(|(j, n)| format!("{} le {}", n, j)).collect::<Vec<_>>().join(", ")
        )));
    }
    let restants: feries::Jours = tous.iter().filter(|(j, _)| !chomes.iter().any(|(c, _)| c == j)).cloned().collect();
    if !restants.is_empty() && dits.is_none() {
        out.push(avertir(format!(
            "{} : {} est ferie, et il a ete compte comme un jour travaille : \
             la fiche Entreprise ne dit pas quels jours feries la compagnie \
             chome. A regler une fois pour toutes : Configuration › \
             Entreprise, « Jours feries chomes ».",
            qui,
            liste(&restants).iter().map(|(j, n)| format!("{} ({})", n, j)).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(out)
}

/// Le compteur d'un permanent a la fin du mois et l'indemnite de son solde.
pub struct Provision {
    pub report_restant: socle::num::Decimal,
    pub acquis: socle::num::Decimal,
    pub pris: socle::num::Decimal,
    pub solde: socle::num::Decimal,
    pub avertissements: Vec<crate::Avis>,
    pub indemnite: Option<droit::conges::IndemniteDesJours>,
}

/// La provision de conges d'un permanent a la fin de `mois`, lue dans la paie ecrite `paie` de ce mois : le compteur
/// que son bulletin montre (`Paie::conges`, ecrit a la validation), dont l'assiette du dixieme compte aussi le brut du
/// mois (`brut_du_mois`, celui des paies ecrites du salarie), et l'indemnite de son solde (`provision_du_compteur`).
/// `salaire_mensuel` : la valeur de l'engagement (`None` ou fausse = 0). `None` : un contrat sans compteur de
/// l'employeur (declare a la caisse des Conges Spectacles). Refus : une paie validee qui ne porte pas le compteur
/// qu'elle devrait.
pub fn provision_conges_de(paie: &Paie, mois: &str, brut_du_mois: socle::num::Decimal, salaire_mensuel: Option<&NombreLu>) -> socle::num::Resultat<Option<Provision>> {
    let Some(ecrit) = paie.conges.as_ref() else {
        if paie.declaree_a_la_caisse_des_conges()? {
            return Ok(None);
        }
        return Err(format!(
            "{} : la paie validee de {} ne porte pas le compteur de conges que son bulletin montre. Revalider l'engagement.",
            paie.engagement.salarie, mois
        ));
    };
    let mut cpt = ecrit.clone();
    cpt.remuneration_periode = add(cpt.remuneration_periode, brut_du_mois);
    let salaire = match salaire_mensuel {
        Some(n) => Chiffre::Nombre(n.clone()).dec()?,
        _ => Decimal::ZERO,
    };
    provision_du_compteur(cpt, mois, salaire).map(Some)
}

/// LA PROVISION D'UN COMPTEUR a la fin de `mois` : son solde, et l'indemnite de ce solde quand il est positif, le
/// dixieme ou le maintien (C. trav. L3141-24, `droit::permanents::indemnite_des_jours`), le maintien au salaire
/// `salaire` du mois reparti sur ses jours ouvrables.
pub fn provision_du_compteur(cpt: CompteurConges, mois: &str, salaire: Decimal) -> Resultat<Provision> {
    use socle::num::eur;
    let fin = Mois::lire(mois)?.dernier_jour();
    let solde = cpt.solde();
    let ouvrables = match jours_ouvrables(&Mois::lire(mois)?.premier_jour(), &fin, None)? {
        0 => Decimal::ONE,
        n => Decimal::from(n),
    };
    let indemnite = if solde > Decimal::ZERO {
        Some(cpt.indemnite_des_jours(solde, eur(mul(div(salaire, ouvrables), solde)))?)
    } else {
        None
    };
    Ok(Provision { report_restant: cpt.report_restant(), acquis: cpt.acquis, pris: cpt.pris, solde, avertissements: cpt.avertissements.clone(), indemnite })
}

/// LE COMPTEUR DE CONGES QUE LA FICHE DU SALARIE REPREND d'un ancien logiciel (`conges_reprise`), a sa date : celui du
/// premier jour paye ici, donc du dernier jour que l'ancien logiciel a paye. `None` : la fiche ne dit pas de reprise.
pub fn compteur_a_la_reprise(ctx: &Contexte, cle_salarie: &str) -> Resultat<Option<CompteurConges>> {
    let Some(r) = reprise_conges(ctx, cle_salarie)? else { return Ok(None) };
    compteur_conges(ctx, cle_salarie, &r.date, &r.date.mois_civil().to_string()).map(Some)
}
