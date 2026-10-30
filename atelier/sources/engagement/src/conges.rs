//! Les conges payes d'un permanent, et les lignes de son mois : d'une saisie de permanent aux lignes d'un bulletin
//! (`lignes_du_mois`), et son compteur de conges payes. Les jours et l'indemnite retenue sont la regle du droit
//! (`droit::conges`, `droit::permanents`).
//!
//! Le compteur se tire des paies validees (`etats::paies_validees`) : leur presence, leurs jours pris, leur brut. Ce qui
//! n'est pas valide n'est pas paye, et aucun mois ne se calcule pour un autre. La validation ecrit dans chaque paie le
//! compteur que son bulletin montre et le compteur avant son mois (`compteur_ecrit`, `Avant`) ; le bulletin et la
//! provision lisent le premier, le mois suivant reprend le second et n'y ajoute que les paies du mois d'avant
//! (`suivant`) : un mois ne relit pas les mois d'avant. Sans etat ecrit au mois d'avant (le premier mois paye, un mois
//! sans paie), le compteur se tire de chaque mois de la periode. Le memo d'une derivation lit chaque mois une fois ; il
//! meurt avec la derivation qui l'a ouvert.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use serde_json::{json, Value};

use crate::calcul::Paie;
use droit::catalogue::rubrique;
use droit::clauses;
use droit::contexte::Contexte;
use droit::natures::{self, Nature};
use socle::date::{d, dernier_du_mois, en_texte, premier_du_mois, forme_ecrite, Jour};
use crate::Avis;
use socle::feries;
use socle::fichiers::{repr, vrai};
use socle::num::{add, dec, dec_f64, dec_avec_echelle, div, eur2, mul, zero2, Decimal, Resultat};
use droit::conges::{heures_par_jour_oeuvre, jours_oeuvres, jours_ouvrables, IndemniteDesJours};
use droit::permanents::{arrondir_jours, jours_acquis, periode_de_reference, plafonner, Acquisition};

// ------------------------------------------------------------ les aides

fn requis<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// Le salarie que nomme en tete ce que le moteur dit d'un engagement paye au mois (`crate::salarie_nomme`).
pub(crate) fn qui(ctx: &Contexte, e: &Value) -> String {
    crate::salarie_nomme(ctx, e.get("salarie"))
}

/// La regle qui paie le mois incomplet [`debut`, `fin`] (borne au mois [`debut_mois`, `fin_mois`]) du contrat de
/// `salarie` (`droit::periodes::regle_du_mois_incomplet`) : la convention de l'entreprise, le statut de la fiche (lu comme
/// le bareme le lit : vide, un artiste), et si le contrat commence ou finit dans le mois.
pub(crate) fn regle_du_mois_incomplet(
    ctx: &Contexte,
    salarie: &str,
    debut: &Jour,
    fin: &Jour,
    debut_mois: &Jour,
    fin_mois: &Jour,
) -> droit::periodes::RegleDuMoisIncomplet {
    let idcc = ctx.entreprise.get("idcc").map(en_texte).unwrap_or_default();
    let fiche = ctx.salarie(salarie).unwrap_or(Value::Null);
    let statut = fiche.get("statut").filter(|s| vrai(s)).map(en_texte).unwrap_or_else(|| "artiste".into());
    droit::periodes::regle_du_mois_incomplet(&idcc, &statut, debut > debut_mois || fin < fin_mois)
}

/// Le flottant d'un decimal, correctement arrondi, par le texte.
pub(crate) fn flot(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(0.0)
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

/// `Decimal(str(x))` d'une valeur ou, si elle est fausse, de `defaut`.
pub(crate) fn dec_ou(v: Option<&Value>, defaut: Decimal) -> Resultat<Decimal> {
    match v {
        Some(x) if vrai(x) => dec_avec_echelle(x),
        _ => Ok(defaut),
    }
}

/// `int(x)` d'une valeur JSON.
pub(crate) fn entier(v: &Value) -> Resultat<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|x| x.trunc() as i64))
            .ok_or_else(|| socle::fichiers::entier_hors_limites(&n.to_string())),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("nombre entier illisible : {}", repr(s))),
        Value::Bool(b) => Ok(*b as i64),
        autre => Err(socle::fichiers::attendu("un nombre entier", socle::valeur::nom_type(autre))),
    }
}

pub(crate) fn nature_de(v: Option<&Value>) -> Resultat<std::sync::Arc<Nature>> {
    let cle = match v {
        None | Some(Value::Null) => None,
        Some(x) => Some(en_texte(x)),
    };
    natures::nature(cle.as_deref())
}

// -------------------------------------------------- les jours ouvrables

/// Les jours ouvrables d'un conge pris (`{"debut", "fin"}`) dans l'entreprise `ent` (`droit::conges::jours_ouvrables`).
pub(crate) fn jours_ouvrables_de(c: &Value, ent: &Value) -> Resultat<i64> {
    jours_ouvrables(&d(requis(c, "debut")?)?, &d(requis(c, "fin")?)?, Some(ent))
}

fn fin_mois_de(mois: &str) -> Resultat<Jour> {
    dernier_du_mois(mois)
}

// ------------------------------------------------ le compteur de conges

/// Ou en est un salarie de ses conges payes, a une date.
#[derive(Debug, Clone, PartialEq)]
pub struct CompteurConges {
    pub salarie: String,
    pub periode_debut: Jour,
    pub periode_fin: Jour,
    /// Jours ouvrables, compte exact.
    pub acquis: Decimal,
    pub pris: Decimal,
    /// L'assiette du dixieme.
    pub remuneration_periode: Decimal,
    /// Ce qui reste des jours acquis sur la periode precedente.
    pub reports: Decimal,
    pub avertissements: Vec<Avis>,
    pub report_debut: Option<Jour>,
    pub report_fin: Option<Jour>,
    pub acquis_report: Decimal,
    /// L'assiette du dixieme de la periode du report (zero sans report).
    pub remuneration_report: Decimal,
    /// Les jours pris que l'ancien logiciel a deja imputes sur la periode en
    /// cours (reprise) : ils ne consomment pas le report.
    pub pris_sur_la_periode: Decimal,
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

    /// Le compteur tel qu'il s'ecrit dans la paie validee (`etats::CONGES`) : les nombres en texte, les jours en
    /// AAAA-MM-JJ.
    pub fn en_json(&self) -> Value {
        let n = |x: Decimal| Value::String(x.to_string());
        let j = |x: &Option<Jour>| x.map_or(Value::Null, |j| Value::String(j.iso()));
        json!({
            "salarie": self.salarie,
            "periode_debut": self.periode_debut.iso(),
            "periode_fin": self.periode_fin.iso(),
            "acquis": n(self.acquis),
            "pris": n(self.pris),
            "remuneration_periode": n(self.remuneration_periode),
            "reports": n(self.reports),
            "report_debut": j(&self.report_debut),
            "report_fin": j(&self.report_fin),
            "acquis_report": n(self.acquis_report),
            "remuneration_report": n(self.remuneration_report),
            "pris_sur_la_periode": n(self.pris_sur_la_periode),
            "avertissements": self.avertissements.iter().map(|a| json!({ "texte": a.texte, "info": a.info })).collect::<Vec<_>>(),
        })
    }

    /// Le compteur ecrit dans une paie validee (`en_json`). Refus : un champ qui manque ou ne se lit pas.
    pub fn lire(v: &Value) -> Resultat<CompteurConges> {
        let illisible = |k: &str| format!("compteur de conges illisible : `{}`", k);
        let n = |k: &str| -> Resultat<Decimal> {
            requis(v, k)?.as_str().and_then(|t| Decimal::from_str(t).ok()).ok_or_else(|| illisible(k))
        };
        let j = |k: &str| -> Resultat<Option<Jour>> { v.get(k).filter(|x| !x.is_null()).map(d).transpose() };
        let avis = v.get("avertissements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]);
        Ok(CompteurConges {
            salarie: en_texte(requis(v, "salarie")?),
            periode_debut: d(requis(v, "periode_debut")?)?,
            periode_fin: d(requis(v, "periode_fin")?)?,
            acquis: n("acquis")?,
            pris: n("pris")?,
            remuneration_periode: n("remuneration_periode")?,
            reports: n("reports")?,
            avertissements: avis
                .iter()
                .map(|a| Avis { info: a.get("info").is_some_and(vrai), texte: a.get("texte").map(en_texte).unwrap_or_default() })
                .collect(),
            report_debut: j("report_debut")?,
            report_fin: j("report_fin")?,
            acquis_report: n("acquis_report")?,
            remuneration_report: n("remuneration_report")?,
            pris_sur_la_periode: n("pris_sur_la_periode")?,
        })
    }
}

/// Ce qu'une paie validee apporte au compteur de conges et a la deduction forfaitaire de l'annee.
#[derive(Debug, Clone)]
struct PaieLue {
    /// La cle de la fiche du salarie.
    cle: String,
    brut: Decimal,
    /// La deduction forfaitaire specifique retenue (zero sans DFS).
    dfs: Decimal,
    /// Un contrat paye au mois : sa presence dans le mois, et ce qu'il en decompte.
    mensuel: Option<MoisPaye>,
    /// Le compteur avant le mois que la paie a ecrit (`etats::CONGES_AVANT`).
    avant: Option<Avant>,
}

#[derive(Debug, Clone)]
struct MoisPaye {
    /// La presence du mois : le contrat borne au mois (`Engagement::debut_periode`, `fin_periode`).
    debut: Jour,
    fin: Jour,
    contrat_debut: Jour,
    numero: Option<String>,
    /// Les jours de conges que la paie decompte (`conges_pris_de_la_paie`).
    pris: Decimal,
    /// Un arret de travail du contrat range dans ce mois (`imprevus`).
    arret: bool,
}

/// Ce que la paie validee `p` de `mois` apporte au compteur.
fn lue(ctx: &Contexte, mois: &str, p: &Paie) -> Resultat<PaieLue> {
    let eng = &p.engagement;
    let cle = p.salarie.get("_cle").and_then(|c| c.as_str()).unwrap_or("").to_string();
    let mensuel = if eng.nat()?.mensuelle() {
        let mut arret = false;
        for imp in crate::imprevus::du_mois(&ctx.racine, mois, None)? {
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
    let dfs = p.deduction_forfaitaire.as_ref().map_or(zero2(), |d| d.montant);
    Ok(PaieLue { cle, brut: p.brut, dfs, mensuel, avant: p.conges_avant.clone() })
}

// Le memo d'une derivation : la profondeur des derivations ouvertes, et ce que chaque (racine, mois) a deja apporte
// (`PaieLue`), ou le refus d'un mois qui ne se lit pas. Un mois se lit une fois pour tous ses salaries.
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

/// La cle d'un mois au memo de la derivation : la compagnie, le mois, le registre de lectures ouvert sur ce fil
/// (`fichiers::generation`, 0 sans registre) et l'etat de la validation en cours (`etats::marque_de_validation`). Un
/// calcul suivi ne se sert ainsi que des mois relus par lui-meme (leurs lectures se notent), meme dans une derivation
/// ouverte avant lui (`moteur::routes::etapes::mois_a_ouvrir`).
fn cle_du_memo(ctx: &Contexte, mois: &str) -> (String, String) {
    (
        ctx.racine.display().to_string(),
        format!("{}|{}|{}", mois, socle::fichiers::generation(), crate::etats::marque_de_validation()),
    )
}

/// LE COMPTEUR AVANT UN MOIS, tel que la paie de ce mois l'ecrit pour le mois suivant (`etats::CONGES_AVANT`) : le
/// compteur tire des mois d'avant (`compteur_conges`), le nombre de ses avis qui viennent du report de la periode
/// precedente (les premiers), le premier mois ou le salarie a la paie d'un contrat paye au mois, et la reprise de
/// l'ancien logiciel telle que la fiche la portait. Le mois suivant le reprend et n'y ajoute que ce mois-ci (`suivant`) :
/// il ne relit aucun mois d'avant.
#[derive(Debug, Clone, PartialEq)]
pub struct Avant {
    pub compteur: CompteurConges,
    avis_du_report: usize,
    premier: Option<String>,
    reprise: Value,
}

impl Avant {
    /// L'etat tel qu'il s'ecrit dans la paie validee.
    pub fn en_json(&self) -> Value {
        json!({
            "compteur": self.compteur.en_json(),
            "avis_du_report": self.avis_du_report,
            "premier": self.premier,
            "reprise": self.reprise,
        })
    }

    /// L'etat ecrit dans une paie validee (`en_json`). Refus : un champ qui manque ou ne se lit pas.
    pub fn lire(v: &Value) -> Resultat<Avant> {
        let compteur = CompteurConges::lire(requis(v, "compteur")?)?;
        let avis_du_report = requis(v, "avis_du_report")?.as_u64().filter(|n| *n as usize <= compteur.avertissements.len()).ok_or("compteur avant le mois illisible : `avis_du_report`")? as usize;
        let premier = match requis(v, "premier")? {
            Value::Null => None,
            Value::String(m) => Some(m.clone()),
            _ => return Err("compteur avant le mois illisible : `premier`".into()),
        };
        Ok(Avant { compteur, avis_du_report, premier, reprise: requis(v, "reprise")?.clone() })
    }
}

/// La reprise des conges telle que la fiche du salarie la porte (`conges_reprise`), `null` sans elle : ce que l'etat
/// avant un mois a pris pour acquis.
fn reprise_ecrite(ctx: &Contexte, cle_salarie: &str) -> Value {
    let fiche = match ctx.salaries.get(cle_salarie) {
        Some(f) => Some(f.clone()),
        None => ctx.salarie(cle_salarie).ok(),
    };
    fiche.as_ref().and_then(|f| f.get("conges_reprise")).cloned().unwrap_or(Value::Null)
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
    compteur(ctx, cle_salarie, a_la_date, Some(mois))
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
    fn ouvrir(ctx: &Contexte, rep: Option<&Reprise>, debut: &Jour, fin: &Jour) -> Tenue {
        let mut t = Tenue { acquis: Decimal::ZERO, pris: Decimal::ZERO, brut: zero2(), avis: Vec::new() };
        if let Some(r) = rep.filter(|r| *debut <= r.date && r.date <= *fin) {
            t.acquis = add(t.acquis, r.acquis);
            t.pris += r.pris;
            t.brut = add(t.brut, r.remuneration_periode);
        } else if let Some(r) = rep.filter(|r| {
            periode_de_reference(&r.date, Some(&ctx.entreprise)).and_then(|p| p.0.plus_jours(-1)).map_or(false, |v| *fin == v)
        }) {
            t.brut = add(t.brut, r.remuneration_report);
        }
        t
    }

    /// Ce que les paies validees `paies` de `mois` apportent a la periode [debut, fin] : la presence du salarie (`cle`,
    /// la cle de sa fiche), ses jours pris, et, avec `remuneration`, son brut d'un mois ou il est present.
    #[allow(clippy::too_many_arguments)]
    fn ajouter(
        &mut self, ctx: &Contexte, cle_salarie: &str, cle: &str, rep: Option<&Reprise>, debut: &Jour, fin: &Jour, mois: &str,
        paies: &[PaieLue], remuneration: bool,
    ) -> Resultat<()> {
        let siennes: Vec<&PaieLue> = paies.iter().filter(|p| p.cle == cle).collect();
        let mut present = false;
        for m in siennes.iter().filter_map(|p| p.mensuel.as_ref()) {
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
        let avant_la_reprise = match rep {
            Some(r) => fin_mois_de(mois)? < r.date,
            None => false,
        };
        // L'assiette du dixieme : tout le brut du salarie dans un mois ou il est present.
        if remuneration && present && !avant_la_reprise {
            self.brut = siennes.iter().fold(self.brut, |a, p| add(a, p.brut));
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
    let mut t = Tenue::ouvrir(ctx, rep.as_ref(), debut, fin);
    let cle = cle_de_fiche(ctx, cle_salarie);
    for (mois, paies) in mois_de_la_periode(ctx, debut, fin, jusqu_au_mois)? {
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
    let (debut, fin) = periode_de_reference(a_la_date, Some(&ctx.entreprise))?;
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
        reprise: reprise_ecrite(ctx, cle_salarie),
    })
}

/// L'ETAT AVANT `mois` TIRE DE CELUI DU MOIS D'AVANT : l'etat que la paie du salarie de `mois - 1` a ecrit (avant
/// `mois - 1`), plus ce que les paies validees de `mois - 1` apportent, comme `compteur` l'aurait tire de tous les mois :
/// dans la meme periode de reference, le mois d'avant s'ajoute a la periode ; au passage d'une periode, la periode qui
/// finit fait son report (le pas que `report` ferait pour elle) et la nouvelle s'ouvre. `None` : rien a reprendre (le
/// mois d'avant n'a pas d'etat ecrit pour ce salarie, ou la reprise de sa fiche a change depuis), ou une periode de
/// reference qui ne commence pas un premier du mois.
fn suivant(ctx: &Contexte, cle_salarie: &str, a_la_date: &Jour, mois: &str) -> Resultat<Option<Avant>> {
    let ent = Some(&ctx.entreprise);
    let (debut, fin) = periode_de_reference(a_la_date, ent)?;
    if debut.jour != 1 {
        return Ok(None);
    }
    let precedent = premier_du_mois(mois)?.plus_jours(-1)?.mois_iso();
    let cle = cle_de_fiche(ctx, cle_salarie);
    let paies = paies_du_mois(ctx, &precedent)?;
    let Some(a) = paies.iter().filter(|p| p.cle == cle).find_map(|p| p.avant.clone()) else { return Ok(None) };
    if a.reprise != reprise_ecrite(ctx, cle_salarie) {
        return Ok(None);
    }
    let rep = reprise_conges(ctx, cle_salarie)?;
    let c = a.compteur;
    let (q_debut, q_fin) = (c.periode_debut, c.periode_fin);
    let mut tenue = Tenue { acquis: c.acquis, pris: c.pris, brut: c.remuneration_periode, avis: c.avertissements[a.avis_du_report..].to_vec() };
    tenue.ajouter(ctx, cle_salarie, &cle, rep.as_ref(), &q_debut, &q_fin, &precedent, &paies, true)?;
    let mut avis_du_report = c.avertissements[..a.avis_du_report].to_vec();
    let mensuel_au_precedent = paies.iter().any(|p| p.cle == cle && p.mensuel.is_some());
    let premier = a.premier.clone().or_else(|| mensuel_au_precedent.then(|| precedent.clone()));
    if rep.is_none() && a.premier.is_none() {
        if let Some(p) = &premier {
            if let Some(avant) = commence_avant(ctx, cle_salarie, p)? {
                avis_du_report.push(avis_commence_avant(cle_salarie, &avant, p));
            }
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
        Some(r) => periode_de_reference(&r.date, ent)?.0 < debut,
        None => premier.is_some(),
    };
    let (mut reste, mut acquis_report, mut report_debut, mut report_fin) = (c.reports, c.acquis_report, c.report_debut, c.report_fin);
    if parcourue {
        let acquis_total = plafonner(tenue.acquis)?;
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
    let ouverte = Tenue::ouvrir(ctx, rep.as_ref(), &debut, &fin);
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
    let ent = Some(&ctx.entreprise);
    let vide = Report { reports: Decimal::ZERO, acquis: Decimal::ZERO, debut: None, fin: None, avertissements: vec![], premier: None };
    let rep = reprise_conges(ctx, cle_salarie)?;
    let premier = premier_mois(ctx, cle_salarie, jusqu_au_mois)?;
    let mut out = Report { avertissements: vec![], ..vide };
    let mut avert = Vec::new();
    let (mut p_debut, mut p_fin, mut reste);
    if let Some(r) = &rep {
        // La reprise : le dernier bulletin de l'ancien logiciel, son reste
        // N-1, et la periode N-1 elle-meme.
        (p_debut, p_fin) = periode_de_reference(&r.date, ent)?;
        reste = r.report;
        let avant_fin = p_debut.plus_jours(-1)?;
        let avant_debut = periode_de_reference(&avant_fin, ent)?.0;
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
        (p_debut, p_fin) = periode_de_reference(&premier_du_mois(premier)?, ent)?;
        reste = Decimal::ZERO;
        if let Some(avant) = commence_avant(ctx, cle_salarie, premier)? {
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
        (p_debut, p_fin) = periode_de_reference(&suivant, ent)?;
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

/// Le solde repris de l'ancien logiciel, ou `None`. Un montant illisible
/// refuse : un solde de conges ne se devine pas.
pub(crate) fn reprise_conges(ctx: &Contexte, cle_salarie: &str) -> Resultat<Option<Reprise>> {
    let fiche = match ctx.salaries.get(cle_salarie) {
        Some(f) => Some(f.clone()),
        None => ctx.salarie(cle_salarie).ok(),
    };
    let Some(r) = fiche.as_ref().and_then(|f| f.get("conges_reprise")).filter(|r| r.is_object()) else {
        return Ok(None);
    };
    let texte = |c: &str| r.get(c).filter(|x| !x.is_null()).map(en_texte).unwrap_or_default();
    let date = r.get("date").filter(|x| vrai(x)).map(en_texte).unwrap_or_default();
    if date.trim().is_empty() {
        return Ok(None);
    }
    let jour = Jour::lire(date.trim()).map_err(|_| {
        format!(
            "{} : la date des conges a la reprise ({}) n'est pas une date AAAA-MM-JJ.",
            cle_salarie,
            forme_ecrite(r.get("date").unwrap_or(&Value::Null))
        )
    })?;
    let nombre = |c: &str| -> Resultat<Decimal> {
        let v = texte(c).trim().replace(',', ".").replace('\u{a0}', "").replace(' ', "");
        if v.is_empty() {
            return Ok(Decimal::ZERO);
        }
        Decimal::from_str(&v).or_else(|_| Decimal::from_scientific(&v)).map_err(|_| {
            format!(
                "{} : conges a la reprise, {} = {} n'est pas un nombre.",
                cle_salarie,
                c,
                forme_ecrite(r.get(c).unwrap_or(&Value::Null))
            )
        })
    };
    Ok(Some(Reprise {
        date: jour,
        report: nombre("report")?,
        acquis_report: nombre("acquis_report")?,
        acquis: nombre("acquis")?,
        pris: nombre("pris")?,
        remuneration_report: nombre("remuneration_report")?,
        remuneration_periode: nombre("remuneration_periode")?,
    }))
}

/// La date de debut du contrat paye au mois de ce salarie, lue dans sa paie validee de `premier` (son premier mois
/// paye), si elle est anterieure a ce mois ; sinon `None`.
fn commence_avant(ctx: &Contexte, cle_salarie: &str, premier: &str) -> Resultat<Option<Jour>> {
    let cle = cle_de_fiche(ctx, cle_salarie);
    let debut_du_mois = premier_du_mois(premier)?;
    Ok(paies_du_mois(ctx, premier)?
        .iter()
        .filter(|p| p.cle == cle)
        .filter_map(|p| p.mensuel.as_ref())
        .map(|m| m.contrat_debut)
        .find(|debut| *debut < debut_du_mois))
}

/// Le premier mois ou le salarie a la paie validee d'un contrat paye au mois.
fn premier_mois(ctx: &Contexte, cle_salarie: &str, jusqu_au_mois: Option<&str>) -> Resultat<Option<String>> {
    let cle = cle_de_fiche(ctx, cle_salarie);
    for mois in crate::stock::mois_saisis(&ctx.racine)? {
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) {
            return Ok(None);
        }
        if paies_du_mois(ctx, &mois)?.iter().any(|p| p.cle == cle && p.mensuel.is_some()) {
            return Ok(Some(mois));
        }
    }
    Ok(None)
}

/// Les mois qui croisent la periode, du plus ancien au plus recent, avec ce que leurs paies validees apportent
/// (`paies_du_mois`) : un contrat paye au mois a une paie dans chacun de ses mois.
fn mois_de_la_periode(
    ctx: &Contexte,
    debut: &Jour,
    fin: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<Vec<(String, Vec<PaieLue>)>> {
    let mut out = Vec::new();
    if fin < debut {
        return Ok(out);
    }
    let saisis = crate::stock::mois_saisis(&ctx.racine)?;
    for mois in socle::date::mois_entre(debut, fin) {
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) || !saisis.contains(&mois) {
            continue;
        }
        let paies = paies_du_mois(ctx, &mois)?;
        out.push((mois, paies));
    }
    Ok(out)
}

/// Ce que les paies validees de `mois` apportent (`etats::paies_validees`, `lue`), lu une fois par derivation.
fn paies_du_mois(ctx: &Contexte, mois: &str) -> Resultat<Vec<PaieLue>> {
    let reference = cle_du_memo(ctx, mois);
    let memo = MEMO.with(|m| {
        let m = m.borrow();
        if m.0 > 0 { m.1.get(&reference).cloned() } else { None }
    });
    if let Some(p) = memo {
        return p;
    }
    let p = crate::etats::paies_validees(ctx, mois).and_then(|paies| paies.iter().map(|p| lue(ctx, mois, p)).collect());
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 > 0 {
            m.1.insert(reference, p.clone());
        }
    });
    p
}

/// La cle de fiche d'un salarie tel qu'une saisie ou un appelant le nomme.
fn cle_de_fiche(ctx: &Contexte, cle: &str) -> String {
    ctx.salarie(cle)
        .ok()
        .and_then(|f| f.get("_cle").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_else(|| cle.to_string())
}

/// Le brut d'un salarie sur un mois anterieur -- l'assiette du dixieme -- tel que ses paies validees le portent.
pub(crate) fn remuneration_du_mois(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    let cle_fiche = cle_de_fiche(ctx, cle);
    Ok(paies_du_mois(ctx, mois)?.iter().filter(|p| p.cle == cle_fiche).fold(zero2(), |a, p| add(a, p.brut)))
}

/// La deduction forfaitaire specifique deja retenue pour un salarie dans les mois de la meme annee civile, strictement
/// avant `mois` (AAAA-MM), telle que ses paies validees la portent (`calcul::dfs_deja_deduite` ; arrete du 4 septembre
/// 2025, art. 9 : << dans la limite de 7 600 euros par annee civile >>). Une paie validee qui ne se lit pas refuse : le
/// cumul de l'annee ne se devine pas.
pub(crate) fn dfs_des_mois_anterieurs(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    let _derivation = Derivation::ouvrir();
    let annee: String = mois.chars().take(4).collect();
    let cle_fiche = cle_de_fiche(ctx, cle);
    let mut total = zero2();
    for m in crate::stock::mois_saisis(&ctx.racine)? {
        if !m.starts_with(&annee) || m.as_str() >= mois {
            continue;
        }
        let paies = paies_du_mois(ctx, &m).map_err(|e| {
            format!(
                "Deduction forfaitaire specifique de {} : les paies validees de {} ne se lisent pas ({}). Le plafond de \
7 600 EUR par annee civile (arrete du 4 septembre 2025, art. 9) se suit sur toute l'annee : sans ce mois, le cumul ne se \
connait pas. Rien n'est calcule.",
                cle, m, e
            )
        })?;
        total = paies.iter().filter(|p| p.cle == cle_fiche).fold(total, |a, p| add(a, p.dfs));
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
    let (debut, fin) = periode_de_reference(&fin_du_mois, Some(&ctx.entreprise))?;
    // La presence de CE mois, bornee a la periode de reference.
    let p_debut = eng.debut_periode()?.max(debut);
    let p_fin = eng.fin_periode()?.min(fin);
    if p_fin >= p_debut {
        let cle_fiche = cle_de_fiche(ctx, cle);
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
        if rubrique(racine, &en_texte(requis(l, "rubrique")?), jour)?.conges_pris {
            total += dec_avec_echelle(requis(l, "quantite")?)?;
        }
    }
    Ok(total)
}

// ------------------------------------------------- les lignes d'un mois paye

/// Les lignes de paie d'un mois de permanent, et ce qu'il y a a en dire.
/// L'ordre des lignes est celui du bulletin : le salaire de base, les
/// absences, les indemnites, les primes, puis le solde de tout compte.
pub(crate) fn lignes_du_mois(ctx: &Contexte, e: &Value, mois: &str) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let nat = nature_de(e.get("nature"))?;
    if !nat.mensuelle() {
        return Err(format!("{} n'est pas un contrat paye au mois : ses lignes se saisissent.", nat.libelle));
    }
    // LES ABSENCES NE SE SAISISSENT PLUS DANS LE MOIS : un arret de travail ou une suspension est un
    // imprevu, saisi une fois sur le salarie (`imprevus`), dont le moteur tire la ligne de chaque mois qu'il traverse.
    if e.get("absences").is_some() {
        return Err(format!(
            "{} : la saisie du mois porte des `absences`. Un arret de travail ou une suspension (conge parental, \
             sabbatique, sans solde...) se saisit une fois, comme un imprevu du salarie, et le moteur en tire la ligne de \
             chaque mois qu'il traverse. Saisir l'imprevu, puis enregistrer ce mois sans ses absences.",
            qui(ctx, e)
        ));
    }
    // Une saisie qui ecrit ses lignes les garde : un imprevu du contrat n'y retire rien, et cela se dit.
    if let Some(l) = e.get("lignes").filter(|l| !l.is_null()) {
        let mut dit = Vec::new();
        for imp in crate::imprevus::du_contrat_dans_le_mois(ctx, e, mois)? {
            dit.push(avertir(format!(
                "{} : ses lignes sont ecrites a la main, et l'imprevu ({}) du {} au {} n'en retire rien. Le retirer des \
                 lignes, ou laisser le moteur les tirer de la saisie.",
                qui(ctx, e),
                crate::imprevus::genre(&imp),
                crate::imprevus::premier_jour(&imp)?.jj_mm_aaaa(),
                crate::imprevus::dernier_jour(&imp)?.jj_mm_aaaa()
            )));
        }
        return Ok((l.as_array().ok_or("lignes")?.clone(), dit));
    }
    let salaire = match e.get("salaire_mensuel").filter(|s| !s.is_null()) {
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
    let defaut = dec_f64(nat.quotite_reference_defaut.ok_or("quotite_reference_defaut")?)?;
    let quotite = dec_ou(e.get("quotite_contrat"), defaut)?;
    let salaire = dec_avec_echelle(salaire)?;

    let debut_mois = premier_du_mois(mois)?;
    let fin_mois = fin_mois_de(mois)?;
    // La date de debut d'un contrat paye au mois : la meme phrase que `engagement::Engagement` (une saisie neuve sans
    // debut ne refuse pas sur « champ 'debut' absent »).
    let Some(debut) = e.get("debut").filter(|x| vrai(x)) else {
        return Err(format!(
            "{} : saisissez la date de début de son {}, celle du contrat signé, sous « Contrat ».\n\
             Elle identifie le contrat dans la déclaration (S21.G00.40.001) : elle ne se déduit de rien.",
            qui(ctx, e),
            nat.libelle
        ));
    };
    let debut = d(debut)?.max(debut_mois);
    let fin = match e.get("fin").filter(|x| vrai(x)) {
        Some(f) => d(f)?.min(fin_mois),
        None => fin_mois,
    };
    // Le salaire d'un mois incomplet : l'horaire reel du mois, ou la regle de la convention qui en a une
    // (`droit::periodes::part_du_mois`).
    let regle = regle_du_mois_incomplet(ctx, &en_texte(requis(e, "salarie")?), &debut, &fin, &debut_mois, &fin_mois);
    let part = droit::periodes::part_du_mois(&debut, &fin, &fin_mois, regle)?;
    let fraction = part.fraction;
    let heures_par_jour = heures_par_jour_oeuvre(quotite);
    let mut avert = Vec::new();
    if fraction != Decimal::ONE {
        avert.push(info(format!(
            "{} : mois incomplet, du {} au {} : {}.",
            qui(ctx, e),
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa(),
            part.methode(heures_par_jour)
        )));
    }

    // `heures` est par unite (`Rubrique::heures`) : un mois vaut la quotite,
    // et la fraction, en `quantite`, en fait la part ; `precision` la dit sur le bulletin.
    let mut lignes = vec![json!({
        "rubrique": "mois", "quantite": flot(fraction), "taux": flot(salaire),
        "heures": flot(eur2(quotite)),
    })];
    if let Some(p) = part.sur_le_bulletin(heures_par_jour) {
        lignes[0]["precision"] = json!(p);
    }

    // Le salaire du mois tel que la ligne « mois » le paie (`calcul::lignes_brut` : le montant arrondi ligne par ligne).
    let salaire_du_mois = socle::num::eur(mul(dec(requis(&lignes[0], "taux")?)?, dec(requis(&lignes[0], "quantite")?)?));
    // L'entreprise dit quels jours feries elle chome.
    let ent = &ctx.entreprise;
    let ouvrables = Decimal::from(jours_ouvrables(&debut, &fin, Some(ent))?);
    let ouvrables_du_mois = if ouvrables != Decimal::ZERO { ouvrables } else { Decimal::ONE };
    let taux_jour_ouvrable = eur2(div(mul(salaire, fraction), ouvrables_du_mois));
    // Les heures de travail du mois, au reel (la mesure des absences : `imprevus::lignes_du_mois`).
    let heures_du_mois = eur2(mul(heures_par_jour, Decimal::from(jours_oeuvres(&debut, &fin, Some(ent))?)));

    // --- les conges pris : une retenue, et une indemnite qui la couvre
    let mut pris = Decimal::ZERO;
    for c in e.get("conges_pris").and_then(|c| c.as_array()).map_or(&[][..], |a| &a[..]) {
        pris += Decimal::from(jours_ouvrables_de(c, ent)?);
    }
    if pris != Decimal::ZERO {
        let salarie = en_texte(requis(e, "salarie")?);
        let cpt = compteur_conges(ctx, &salarie, &fin, mois)?;
        let maintien = eur2(mul(taux_jour_ouvrable, pris));
        let ind = indemnite_des_jours(&cpt, pris, maintien)?;
        lignes.push(json!({"rubrique": "absence_conges_payes", "quantite": flot(pris), "taux": flot(taux_jour_ouvrable)}));
        lignes.push(json!({
            "rubrique": "indemnite_conges_payes", "quantite": flot(pris),
            "taux": flot(eur2(div(ind.retenu, pris))),
        }));
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
    let (l, a) = crate::imprevus::lignes_du_mois(
        ctx,
        e,
        mois,
        &crate::imprevus::MoisDuContrat { debut, fin, salaire_du_mois, heures_par_jour, heures_du_mois, taux_jour_ouvrable },
    )?;
    lignes.extend(l);
    avert.extend(a);

    // --- les primes
    for pr in e.get("primes").and_then(|p| p.as_array()).map_or(&[][..], |a| &a[..]) {
        let montant = socle::num::flottant(requis(pr, "montant")?)?;
        lignes.push(json!({
            "rubrique": pr.get("rubrique").cloned().unwrap_or_else(|| json!("prime")),
            "quantite": 1, "taux": montant,
        }));
    }

    // --- la prime d'anciennete de l'annexe I de la 3090
    let mut fiche = ctx.salarie(&en_texte(requis(e, "salarie")?)).unwrap_or_else(|_| json!({}));
    // Les annees de presence que les suspensions laissent (L1225-54 : `imprevus::anciennete_depuis`).
    let mut sur_l_anciennete = Vec::new();
    if let Some(depuis) = fiche.get("date_anciennete").filter(|x| vrai(x)).or_else(|| e.get("debut").filter(|x| vrai(x))).map(d).transpose()? {
        let cle = fiche.get("_cle").map(en_texte).unwrap_or_default();
        let (retenu, dit) = crate::imprevus::anciennete_depuis(&ctx.racine, &cle, &depuis, &fin_mois)?;
        if retenu != depuis {
            if let Some(o) = fiche.as_object_mut() {
                o.insert("date_anciennete".into(), Value::String(retenu.iso()));
            }
            sur_l_anciennete = dit;
        }
    }
    let null = Value::Null;
    let pa = clauses::prime_anciennete(
        ctx.entreprise.get("idcc").unwrap_or(&null),
        ctx.entreprise.get("annexe_convention").unwrap_or(&null),
        &fiche,
        Some(e),
        &fin_mois,
        ctx.entreprise.get("jauge_annexe_1").and_then(|j| j.as_str()),
    )?;
    if let Some(pa) = pa {
        let Some(base_conv) = pa.get("base").filter(|b| !b.is_null()).map(dec).transpose()? else {
            return Err(format!(
                "{} : prime d'anciennete ({}) -- {}.",
                qui(ctx, e),
                en_texte(&pa["article"]),
                en_texte(&pa["pourquoi"])
            ));
        };
        let pourcentage = dec(requis(&pa, "pourcentage")?)?;
        let reference = Decimal::from_str("151.67").unwrap();
        let base = eur2(div(mul(base_conv, quotite), reference));
        let montant = eur2(div(mul(base, pourcentage), Decimal::ONE_HUNDRED));
        lignes.push(json!({"rubrique": "prime_anciennete", "quantite": 1, "taux": flot(montant)}));
        let proratise = if quotite != reference { format!(" proratise a {} h", quotite) } else { String::new() };
        avert.push(info(format!(
            "{} : prime d'anciennete {} % ({} an(s) de presence) du salaire \
             conventionnel de {} EUR{} = {} EUR -- {}.",
            qui(ctx, e),
            pourcentage,
            en_texte(&pa["annees"]),
            base_conv,
            proratise,
            montant,
            en_texte(&pa["article"])
        )));
        // Ce que les suspensions ont retire des annees de presence se dit avec la prime.
        avert.extend(sur_l_anciennete);
    }

    // --- le solde de tout compte, le mois ou le contrat se termine
    if let Some(f) = e.get("fin").filter(|x| vrai(x)) {
        let f = d(f)?;
        if debut_mois <= f && f <= fin_mois {
            let (l, a) = crate::solde::solde_de_tout_compte(ctx, e, mois, &lignes)?;
            lignes.extend(l);
            avert.extend(a);
        }
    }
    Ok((lignes, avert))
}

/// Ce que le moteur a retire du decompte des jours ouvrables, et ce qu'il n'a
/// pas pu retirer : a chaque mois qui porte un ferie.
fn avertir_feries(ctx: &Contexte, e: &Value, debut: &Jour, fin: &Jour) -> Resultat<Vec<Avis>> {
    let ent = Some(&ctx.entreprise);
    let qui = qui(ctx, e);
    let tous = feries::chomes_entre(debut, fin, Some(&json!({"jours_feries_chomes": "tous"})))?;
    if tous.is_empty() {
        return Ok(vec![]);
    }
    let chomes = feries::chomes_entre(debut, fin, ent)?;
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
    if !restants.is_empty() && feries::rien_n_est_dit(ent) {
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
