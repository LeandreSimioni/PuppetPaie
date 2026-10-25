//! `operations_permanents.py` : d'une saisie de permanent aux lignes d'un
//! bulletin (`lignes_du_mois`), et le compteur de conges payes qu'elle
//! derive des mois produits (PORTAGE-RUST §14).
//!
//! Rien n'est stocke : le compteur se derive des fichiers de paie, et le
//! brut d'un mois passe se recalcule par `engagement::charger`, qui redemande
//! un compteur. Le memo d'une derivation (profondeur + brut par mois) coupe
//! l'explosion de ces appels ; il meurt avec la derivation qui l'a ouvert.

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
use socle::date::{d, dernier_du_mois, en_texte, jours_du_mois, premier_du_mois, repr_py, Jour};
use crate::engagement::{charger, Avis};
use socle::feries;
use socle::fichiers::{repr, vrai};
use socle::num::{add, dec, dec_f64, dec_py, div, eur2, mul, zero2, Decimal, Resultat};
use droit::permanents::{arrondir_jours, indemnite_conges, jours_acquis, periode_de_reference, plafonner};

// ------------------------------------------------------------ les aides

fn requis<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// `e.get("numero") or "?"`.
pub(crate) fn numero(e: &Value) -> String {
    e.get("numero").filter(|x| vrai(x)).map_or_else(|| "?".to_string(), en_texte)
}

/// `float(Decimal)` : l'arrondi correct de CPython, par le texte.
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
        Some(x) if vrai(x) => dec_py(x),
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

/// Le nombre de jours ouvrables entre deux dates, bornes incluses : du lundi
/// au samedi, moins les feries que l'entreprise chome.
pub fn jours_ouvrables(debut: &Jour, fin: &Jour, entreprise: Option<&Value>) -> Resultat<i64> {
    jours_de_semaine(debut, fin, entreprise, 6)
}

/// LES JOURS OEUVRES entre deux dates, bornes incluses : du lundi au vendredi, moins les feries que l'entreprise chome
/// -- les jours ou un salarie a temps plein « doit travailler » dans la modalite classique (cinq jours de sept heures
/// pour 35 h). Fiche DSN 2640 (`atelier/reference/sources/suspension-non-remuneree/net-entreprises-fiche2640-volumes-
/// activite-s21g00-53-20260929.txt`, « Le decompte et calcul des absences au reel ») : quand le planning du salarie n'est
/// pas connu du logiciel, « Nombre d'heures d'absence = Nb d'heures travaillees/jours oeuvres x Nb de jours
/// theoriquement oeuvres sur la periode », sur « une ou plusieurs hypotheses de modalites classiques de travail » ;
/// fiche 2693, exemple 10 : un mois de septembre 2026 entier, « 154 heures (22 x 7 heures) ». INFERENCE NOMMEE : la
/// saisie ne dit pas les jours de la semaine d'un permanent, et le moteur prend l'hypothese classique, du lundi au
/// vendredi (la mesure des absences, `imprevus::lignes_du_mois`).
pub fn jours_oeuvres(debut: &Jour, fin: &Jour, entreprise: Option<&Value>) -> Resultat<i64> {
    jours_de_semaine(debut, fin, entreprise, 5)
}

/// Les jours de `debut` a `fin` dont le rang dans la semaine (lundi = 0) est sous `jusqu_a`, moins les feries chomes.
fn jours_de_semaine(debut: &Jour, fin: &Jour, entreprise: Option<&Value>, jusqu_a: u32) -> Resultat<i64> {
    let chomes = feries::chomes_entre(debut, fin, entreprise)?;
    let (mut n, mut jour) = (0, *debut);
    while jour <= *fin {
        if jour.jour_de_la_semaine() < jusqu_a && !chomes.iter().any(|(j, _)| *j == jour) {
            n += 1;
        }
        jour = jour.plus_jours(1)?;
    }
    Ok(n)
}

/// LES HEURES D'UN JOUR OEUVRE d'un contrat paye au mois de quotite `quotite` (heures par mois) : la duree hebdomadaire
/// (quotite x 12 / 52, au centieme : 151,67 h font 35,00 h) sur cinq jours (`jours_oeuvres`) -- 7 h pour un temps plein.
/// Fiche DSN 2640, « Nb d'heures travaillees/jours oeuvres » ; l'hypothese des cinq jours est l'inference de
/// `jours_oeuvres`.
pub fn heures_par_jour_oeuvre(quotite: Decimal) -> Decimal {
    div(eur2(div(mul(quotite, Decimal::from(12)), Decimal::from(52))), Decimal::from(5))
}

pub(crate) fn jours_ouvrables_de(c: &Value, ent: &Value) -> Resultat<i64> {
    jours_ouvrables(&d(requis(c, "debut")?)?, &d(requis(c, "fin")?)?, Some(ent))
}

fn fin_mois_de(mois: &str) -> Resultat<Jour> {
    dernier_du_mois(mois)
}

// ------------------------------------------------ le compteur de conges

/// Ou en est un salarie de ses conges payes, a une date.
#[derive(Debug, Clone)]
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

// Le memo d'une derivation : la profondeur des derivations ouvertes, et les
// paies de chaque (racine, mois) deja recalcule -- (cle, brut, deduction
// forfaitaire specifique), ou l'erreur d'un mois qui ne se charge pas. Un mois
// se charge une fois pour tous ses salaries.
type PaiesDuMois = Result<Vec<(String, Decimal, Decimal)>, String>;

/// Ce que le memo garde d'une paie : la cle de sa fiche, son brut, et la deduction forfaitaire specifique qu'elle a
/// retenue (zero sans DFS).
fn resume_des_paies(paies: &[Paie]) -> Vec<(String, Decimal, Decimal)> {
    paies
        .iter()
        .map(|p| {
            (
                p.salarie.get("_cle").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                p.brut,
                p.deduction_forfaitaire.as_ref().map_or(zero2(), |d| d.montant),
            )
        })
        .collect()
}
thread_local! {
    static MEMO: RefCell<(u32, HashMap<(String, String), PaiesDuMois>)> =
        RefCell::new((0, HashMap::new()));
}

/// Une derivation ouverte : le memo se vide quand la derniere se ferme.
pub(crate) struct Derivation;

impl Derivation {
    pub(crate) fn ouvrir() -> Derivation {
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

/// La cle d'un mois au memo de la derivation : la compagnie, le mois, et le registre de lectures ouvert sur ce fil
/// (`fichiers::generation`, 0 sans registre). Un calcul suivi ne se sert ainsi que des mois relus par lui-meme (leurs
/// lectures se notent), meme dans une derivation ouverte avant lui (`routes_mois::mois_a_ouvrir`, ticket #244).
fn cle_du_memo(ctx: &Contexte, mois: &str) -> (String, String) {
    (ctx.racine.display().to_string(), format!("{}|{}", mois, socle::fichiers::generation()))
}

/// Le brut des paies d'un mois deja charge, pose dans le memo de la derivation
/// ouverte : `remuneration_du_mois` ne le recalcule pas.
pub(crate) fn memoriser_mois(ctx: &Contexte, mois: &str, paies: &[Paie]) {
    let reference = cle_du_memo(ctx, mois);
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 > 0 && !m.1.contains_key(&reference) {
            m.1.insert(reference, Ok(resume_des_paies(paies)));
        }
    });
}

/// Le compteur de conges d'un salarie, reconstruit depuis les saisies.
/// `jusqu_au_mois` : ne lire que les mois strictement anterieurs a celui-la.
pub fn compteur_conges(
    ctx: &Contexte,
    cle_salarie: &str,
    a_la_date: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<CompteurConges> {
    let _derivation = Derivation::ouvrir();
    compteur(ctx, cle_salarie, a_la_date, jusqu_au_mois)
}

/// Ce qu'une periode de reference a acquis et vu prendre :
/// `(acquis plafonne, pris, remuneration, avertissements)`. Sans
/// `remuneration`, aucun brut n'est recalcule.
fn periode(
    ctx: &Contexte,
    cle_salarie: &str,
    debut: &Jour,
    fin: &Jour,
    jusqu_au_mois: Option<&str>,
    remuneration: bool,
) -> Resultat<(Decimal, Decimal, Decimal, Vec<Avis>)> {
    let ent = &ctx.entreprise;
    let (mut acquis, mut pris, mut brut) = (Decimal::ZERO, Decimal::ZERO, zero2());
    let mut avert = Vec::new();
    let rep = reprise_conges(ctx, cle_salarie)?;
    if let Some(r) = rep.as_ref().filter(|r| *debut <= r.date && r.date <= *fin) {
        acquis = add(acquis, r.acquis);
        pris += r.pris;
        brut = add(brut, r.remuneration_periode);
    } else if let Some(r) = rep.as_ref().filter(|r| {
        periode_de_reference(&r.date, Some(ent)).and_then(|p| p.0.plus_jours(-1)).map_or(false, |v| *fin == v)
    }) {
        brut = add(brut, r.remuneration_report);
    }
    for (mois, saisie) in mois_du_salarie(ctx, cle_salarie, debut, fin, jusqu_au_mois)? {
        for e in saisie["engagements"].as_array().map_or(&[][..], |a| &a[..]) {
            if e.get("salarie").and_then(|s| s.as_str()) != Some(cle_salarie) {
                continue;
            }
            if !nature_de(e.get("nature"))?.mensuelle() {
                continue;
            }
            // La presence du mois : le contrat borne au mois, borne a la periode.
            let debut_mois = premier_du_mois(&mois)?;
            let mut p_debut = d(requis(e, "debut")?)?.max(debut_mois).max(*debut);
            if let Some(r) = &rep {
                p_debut = p_debut.max(r.date);
            }
            let fin_du_mois = fin_mois_de(&mois)?;
            let p_fin = match e.get("fin").filter(|x| vrai(x)) {
                Some(f) => d(f)?.min(fin_du_mois),
                None => fin_du_mois,
            }
            .min(*fin);
            if p_fin < p_debut {
                continue;
            }
            // La presence, moins les suspensions qui n'acquierent pas (`imprevus::jours_de_conges_non_acquis`).
            let non_acquis = non_acquis_du_contrat(ctx, e, &p_debut, &p_fin)?;
            acquis = add(acquis, sub(jours_acquis(&p_debut, &p_fin)?, non_acquis).max(Decimal::ZERO));
            // Les jours pris se comptent comme la retenue du bulletin.
            for c in e.get("conges_pris").and_then(|c| c.as_array()).map_or(&[][..], |a| &a[..]) {
                pris += Decimal::from(jours_ouvrables_de(c, ent)?);
            }
            if crate::imprevus::du_contrat_dans_le_mois(ctx, e, &mois)?.iter().any(crate::imprevus::est_un_arret) {
                avert.push(avertir(format!(
                    "{} : un arret de travail en {}. Le compteur de conges compte \
                     le TEMPS DE PRESENCE d'un arret ; les periodes assimilees a \
                     du travail effectif pendant un arret (L3141-5) ne sont pas \
                     appliquees. A verifier a la main.",
                    cle_salarie, mois
                )));
            }
            let avant_la_reprise = rep.as_ref().is_some_and(|r| fin_du_mois < r.date);
            if remuneration && !avant_la_reprise {
                brut = add(brut, remuneration_du_mois(ctx, &mois, cle_salarie)?);
            }
        }
    }
    Ok((plafonner(acquis)?, pris, brut, avert))
}

/// `imprevus::jours_de_conges_non_acquis` du contrat d'un engagement de saisie (sa fiche, son numero, sa date de debut).
fn non_acquis_du_contrat(ctx: &Contexte, e: &Value, debut: &Jour, fin: &Jour) -> Resultat<Decimal> {
    let Some(contrat_debut) = e.get("debut").filter(|x| vrai(x)).map(d).transpose()? else { return Ok(Decimal::ZERO) };
    let cle = cle_de_fiche(ctx, &en_texte(requis(e, "salarie")?));
    let numero = e.get("numero").filter(|x| vrai(x)).map(en_texte);
    crate::imprevus::jours_de_conges_non_acquis(&ctx.racine, &cle, numero.as_deref(), &contrat_debut, debut, fin)
}

fn compteur(
    ctx: &Contexte,
    cle_salarie: &str,
    a_la_date: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<CompteurConges> {
    let (debut, fin) = periode_de_reference(a_la_date, Some(&ctx.entreprise))?;
    let (acquis, pris, remuneration, avert) = periode(ctx, cle_salarie, &debut, &fin, jusqu_au_mois, true)?;
    let report = report(ctx, cle_salarie, &debut, jusqu_au_mois)?;
    let mut avertissements = report.avertissements;
    avertissements.extend(avert);
    Ok(CompteurConges {
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
        pris_sur_la_periode: pris_de_la_reprise(ctx, cle_salarie, &debut, &fin)?,
    })
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
}

fn report(ctx: &Contexte, cle_salarie: &str, debut_courant: &Jour, jusqu_au_mois: Option<&str>) -> Resultat<Report> {
    let ent = Some(&ctx.entreprise);
    let vide = Report { reports: Decimal::ZERO, acquis: Decimal::ZERO, debut: None, fin: None, avertissements: vec![] };
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
            avert.push(avertir(format!(
                "{} : le contrat a commence le {}, avant le premier mois \
                 produit par PuppetPaie ({}). Les conges acquis avant ne \
                 sont pas comptes : les recopier du dernier bulletin de \
                 l'ancien logiciel (fiche salarie, conges payes a la reprise).",
                cle_salarie,
                avant.jj_mm_aaaa(),
                premier
            )));
        }
    } else {
        return Ok(Report { avertissements: vec![], ..out });
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
            avert.push(avertir(format!(
                "{} : {} jour(s) de conges acquis avant le {} n'ont pas ete \
                 pris au {}. Ils ne sont PAS reportes une seconde fois : un \
                 conge non pris a la fin de la periode de prise est perdu, \
                 sauf accord de report ou empechement (maladie : L3141-19-1). \
                 Si c'est le cas, a reprendre a la main.",
                cle_salarie,
                arrondir_jours(perdu),
                p_debut.jj_mm_aaaa(),
                p_fin.jj_mm_aaaa()
            )));
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
pub fn reprise_conges(ctx: &Contexte, cle_salarie: &str) -> Resultat<Option<Reprise>> {
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
            repr_py(r.get("date").unwrap_or(&Value::Null))
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
                repr_py(r.get(c).unwrap_or(&Value::Null))
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

/// La date de debut du contrat au mois lu dans le premier mois produit, si
/// elle est anterieure a ce mois ; sinon `None`.
fn commence_avant(ctx: &Contexte, cle_salarie: &str, premier: &str) -> Resultat<Option<Jour>> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", premier));
    let Ok(saisie) = socle::fichiers::lire_texte(&chemin).map_err(|_| ()).and_then(|t| serde_json::from_str::<Value>(&t).map_err(|_| ()))
    else {
        return Ok(None);
    };
    for e in saisie.get("engagements").and_then(|x| x.as_array()).map_or(&[][..], |a| &a[..]) {
        if e.is_object()
            && e.get("salarie").and_then(|s| s.as_str()) == Some(cle_salarie)
            && nature_de(e.get("nature"))?.mensuelle()
            && e.get("debut").is_some_and(vrai)
        {
            let debut = d(&e["debut"])?;
            return Ok((debut < premier_du_mois(premier)?).then_some(debut));
        }
    }
    Ok(None)
}

/// Les fichiers `AAAA-MM.json` du dossier des paies, du plus ancien au plus recent.
fn fichiers_de_paie(ctx: &Contexte) -> Vec<(String, std::path::PathBuf)> {
    let Some(dossier) = socle::fichiers::lister(&ctx.racine.join("paies")) else {
        return Vec::new();
    };
    let mut out: Vec<(String, std::path::PathBuf)> = dossier
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let mois = p.file_stem()?.to_str()?.to_string();
            (mois.chars().count() == 7 && mois.chars().take(4).all(|c| c.is_ascii_digit())).then_some((mois, p))
        })
        .collect();
    out.sort();
    out
}

/// Le premier mois produit ou le salarie a un contrat paye au mois.
fn premier_mois(ctx: &Contexte, cle_salarie: &str, jusqu_au_mois: Option<&str>) -> Resultat<Option<String>> {
    for (mois, chemin) in fichiers_de_paie(ctx) {
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) {
            return Ok(None);
        }
        let Some(saisie) = socle::fichiers::lire_texte(&chemin).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            continue;
        };
        for e in saisie.get("engagements").and_then(|x| x.as_array()).map_or(&[][..], |a| &a[..]) {
            if e.is_object()
                && e.get("salarie").and_then(|s| s.as_str()) == Some(cle_salarie)
                && nature_de(e.get("nature"))?.mensuelle()
            {
                return Ok(Some(mois));
            }
        }
    }
    Ok(None)
}

/// Les fichiers de paie qui croisent la periode, du plus ancien au plus
/// recent. Un fichier illisible est ignore.
fn mois_du_salarie(
    ctx: &Contexte,
    _cle: &str,
    debut: &Jour,
    fin: &Jour,
    jusqu_au_mois: Option<&str>,
) -> Resultat<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for (mois, chemin) in fichiers_de_paie(ctx) {
        if jusqu_au_mois.is_some_and(|j| !j.is_empty() && mois.as_str() >= j) {
            continue;
        }
        if fin_mois_de(&mois)? < *debut || premier_du_mois(&mois)? > *fin {
            continue;
        }
        let Some(saisie) = socle::fichiers::lire_texte(&chemin).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            continue;
        };
        if !saisie.get("engagements").is_some_and(|e| e.is_array()) {
            continue;
        }
        out.push((mois, saisie));
    }
    Ok(out)
}

/// Les paies d'un mois de saisie, recalculees une fois par derivation (`resume_des_paies`).
fn paies_du_mois_memorisees(ctx: &Contexte, mois: &str) -> PaiesDuMois {
    let reference = cle_du_memo(ctx, mois);
    let memo = MEMO.with(|m| {
        let m = m.borrow();
        if m.0 > 0 { m.1.get(&reference).cloned() } else { None }
    });
    if let Some(p) = memo {
        return p;
    }
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let p = charger(ctx, &chemin).map(|s| resume_des_paies(&s.paies));
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

/// Le brut d'un salarie sur un mois deja produit -- l'assiette du dixieme.
/// On recalcule plutot que de lire un total ecrit.
pub(crate) fn remuneration_du_mois(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    let paies = match paies_du_mois_memorisees(ctx, mois) {
        Ok(p) => p,
        // Ce qui n'est pas porte n'est pas un mois sans brut.
        Err(e) if e.starts_with("non porte") => return Err(e),
        Err(_) => return Ok(zero2()),
    };
    let cle_fiche = cle_de_fiche(ctx, cle);
    Ok(paies.iter().filter(|(c, _, _)| c == cle || *c == cle_fiche).fold(zero2(), |a, (_, b, _)| add(a, *b)))
}

/// La deduction forfaitaire specifique deja retenue pour un salarie dans les mois de saisie de la meme annee civile,
/// strictement avant `mois` (AAAA-MM), chacun recalcule depuis sa saisie (`calcul::dfs_deja_deduite` ; arrete du
/// 4 septembre 2025, art. 9 : << dans la limite de 7 600 euros par annee civile >>). Un mois ou le salarie a un engagement et
/// qui ne se calcule pas refuse : le cumul de l'annee ne se devine pas.
pub(crate) fn dfs_des_mois_anterieurs(ctx: &Contexte, mois: &str, cle: &str) -> Resultat<Decimal> {
    // Chaque mois anterieur se calcule en demandant les siens : sans le memo d'une derivation, un salarie present tous
    // les mois ferait recalculer 2^n mois. Avec lui, chaque mois se calcule une fois.
    let _derivation = Derivation::ouvrir();
    let annee: String = mois.chars().take(4).collect();
    let cle_fiche = cle_de_fiche(ctx, cle);
    let mut total = zero2();
    for (m, chemin) in fichiers_de_paie(ctx) {
        if !m.starts_with(&annee) || m.as_str() >= mois {
            continue;
        }
        let Some(saisie) = socle::fichiers::lire_texte(&chemin).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            continue;
        };
        let sien = saisie.get("engagements").and_then(|e| e.as_array()).map_or(false, |a| {
            a.iter().filter_map(|e| e.get("salarie").and_then(|s| s.as_str())).any(|s| s == cle || cle_de_fiche(ctx, s) == cle_fiche)
        });
        if !sien {
            continue;
        }
        let paies = paies_du_mois_memorisees(ctx, &m).map_err(|e| {
            format!(
                "Deduction forfaitaire specifique de {} : le mois {} ne se calcule pas ({}). Le plafond de 7 600 EUR par \
annee civile (arrete du 4 septembre 2025, art. 9) se suit sur toute l'annee : sans ce mois, le cumul ne se connait pas. Rien \
n'est calcule.",
                cle, m, e
            )
        })?;
        for (c, _, dfs) in &paies {
            if c == cle || *c == cle_fiche {
                total = add(total, *dfs);
            }
        }
    }
    Ok(total)
}

// ------------------------------------------------------ l'indemnite

/// L'indemnite de conges : le dixieme, le maintien, et celui qui gagne.
#[derive(Debug, Clone)]
pub struct IndemniteDesJours {
    pub dixieme: Decimal,
    pub maintien: Decimal,
    pub retenu: Decimal,
    pub regle: &'static str,
}

/// L'indemnite de `jours` de conges (L3141-24), chaque jour au dixieme DE LA
/// PERIODE OU IL A ETE ACQUIS : le report d'abord, puis la periode en cours.
/// Le maintien se compare au TOTAL.
pub fn indemnite_des_jours(
    ctx: &Contexte,
    cpt: &CompteurConges,
    jours: Decimal,
    maintien: Decimal,
) -> Resultat<IndemniteDesJours> {
    let sur_report = jours.min(cpt.report_restant());
    let en_cours = sub(jours, sur_report);
    let mut dixieme = zero2();
    let dixieme_de = |jours: Decimal, brut: Decimal, acquis: Decimal| -> Resultat<Decimal> {
        let v = indemnite_conges(jours, brut, if acquis != Decimal::ZERO { acquis } else { jours }, Decimal::ZERO)?;
        dec(requis(&v, "dixieme")?)
    };
    if sur_report != Decimal::ZERO {
        let _derivation = Derivation::ouvrir();
        let (debut, fin) = (
            cpt.report_debut.ok_or("report_debut")?,
            cpt.report_fin.ok_or("report_fin")?,
        );
        let (_, _, brut, _) = periode(ctx, &cpt.salarie, &debut, &fin, None, true)?;
        dixieme += dixieme_de(sur_report, brut, cpt.acquis_report)?;
    }
    if en_cours != Decimal::ZERO {
        dixieme += dixieme_de(en_cours, cpt.remuneration_periode, cpt.acquis)?;
    }
    let maintien = eur2(maintien);
    Ok(IndemniteDesJours {
        dixieme,
        maintien,
        retenu: if maintien > dixieme { maintien } else { dixieme },
        regle: if dixieme > maintien { "dixieme" } else { "maintien" },
    })
}

/// Le compteur tel que le BULLETIN du mois doit le montrer : celui d'avant,
/// plus ce que ce mois acquiert et prend.
pub fn compteur_au_bulletin(ctx: &Contexte, paie: &Paie, mois: &str) -> Resultat<CompteurConges> {
    let eng = &paie.engagement;
    let cle = paie.salarie.get("_cle").and_then(|c| c.as_str()).ok_or_else(|| socle::fichiers::absent("_cle"))?;
    let fin_du_mois = fin_mois_de(mois)?;
    let mut cpt = compteur_conges(ctx, cle, &fin_du_mois, Some(mois))?;
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
    Ok(cpt)
}

/// Les jours de conges que CETTE paie decompte, lus sur ses lignes : le
/// catalogue marque la rubrique.
pub fn conges_pris_de_la_paie(racine: &Path, paie: &Paie) -> Resultat<Decimal> {
    let mut total = Decimal::ZERO;
    for l in &paie.engagement.lignes {
        if rubrique(racine, &en_texte(requis(l, "rubrique")?))?.conges_pris {
            total += dec_py(requis(l, "quantite")?)?;
        }
    }
    Ok(total)
}

// ------------------------------------------------- les lignes d'un mois paye

/// Les lignes de paie d'un mois de permanent, et ce qu'il y a a en dire.
/// L'ordre des lignes est celui du bulletin : le salaire de base, les
/// absences, les indemnites, les primes, puis le solde de tout compte.
pub fn lignes_du_mois(ctx: &Contexte, e: &Value, mois: &str) -> Resultat<(Vec<Value>, Vec<Avis>)> {
    let nat = nature_de(e.get("nature"))?;
    if !nat.mensuelle() {
        return Err(format!("{} n'est pas un contrat paye au mois : ses lignes se saisissent.", nat.libelle));
    }
    // LES ABSENCES NE SE SAISISSENT PLUS DANS LE MOIS (15/10/2026) : un arret de travail ou une suspension est un
    // imprevu, saisi une fois sur le salarie (`imprevus`), dont le moteur tire la ligne de chaque mois qu'il traverse.
    if e.get("absences").is_some() {
        return Err(format!(
            "Contrat {} : la saisie du mois porte des `absences`. Un arret de travail ou une suspension (conge parental, \
             sabbatique, sans solde...) se saisit une fois, comme un imprevu du salarie, et le moteur en tire la ligne de \
             chaque mois qu'il traverse. Saisir l'imprevu, puis enregistrer ce mois sans ses absences.",
            numero(e)
        ));
    }
    // Une saisie qui ecrit ses lignes les garde : un imprevu du contrat n'y retire rien, et cela se dit.
    if let Some(l) = e.get("lignes").filter(|l| !l.is_null()) {
        let mut dit = Vec::new();
        for imp in crate::imprevus::du_contrat_dans_le_mois(ctx, e, mois)? {
            dit.push(avertir(format!(
                "{} : ses lignes sont ecrites a la main, et l'imprevu ({}) du {} au {} n'en retire rien. Le retirer des \
                 lignes, ou laisser le moteur les tirer de la saisie.",
                numero(e),
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
                "Contrat {} ({}) : le salaire mensuel n'est pas dit \
                 (`salaire_mensuel`). Il figure au contrat signe ; le moteur ne \
                 l'invente pas.",
                numero(e),
                nat.libelle
            ))
        }
    };
    let defaut = dec_f64(nat.quotite_reference_defaut.ok_or("quotite_reference_defaut")?)?;
    let quotite = dec_ou(e.get("quotite_contrat"), defaut)?;
    let salaire = dec_py(salaire)?;

    let debut_mois = premier_du_mois(mois)?;
    let fin_mois = fin_mois_de(mois)?;
    // La date de debut d'un contrat paye au mois : la meme phrase que `engagement::Engagement` (#236 : une saisie neuve
    // refusait sur « champ 'debut' absent », et la page disait « choisir les jours travailles »).
    let Some(debut) = e.get("debut").filter(|x| vrai(x)) else {
        return Err(format!(
            "Contrat {} ({}) : la date de debut du contrat n'est pas dite. Elle part en DSN (S21.G00.40.001) et identifie \
             le contrat : elle ne se deduit de rien.",
            numero(e),
            nat.libelle
        ));
    };
    let debut = d(debut)?.max(debut_mois);
    let fin = match e.get("fin").filter(|x| vrai(x)) {
        Some(f) => d(f)?.min(fin_mois),
        None => fin_mois,
    };
    let jours_du_mois = Decimal::from(jours_du_mois(fin_mois.an, fin_mois.mois));
    let jours_payes = Decimal::from(fin.moins(&debut) + 1);
    // Le prorata d'un mois incomplet : au prorata des jours calendaires.
    let fraction = div(jours_payes, jours_du_mois);
    let mut avert = Vec::new();
    if fraction != Decimal::ONE {
        avert.push(info(format!(
            "{} : mois incomplet ({} jours sur {}), salaire calcule au prorata des jours calendaires.",
            numero(e),
            jours_payes,
            jours_du_mois
        )));
    }

    // `heures` est par unite (`Rubrique::heures`) : un mois vaut la quotite,
    // et la fraction, en `quantite`, en fait le prorata.
    let mut lignes = vec![json!({
        "rubrique": "mois", "quantite": flot(fraction), "taux": flot(salaire),
        "heures": flot(eur2(quotite)),
    })];

    // Le salaire du mois tel que la ligne « mois » le paie (`calcul::lignes_brut` : le montant arrondi ligne par ligne).
    let salaire_du_mois = socle::num::eur(mul(dec(requis(&lignes[0], "taux")?)?, dec(requis(&lignes[0], "quantite")?)?));
    // L'entreprise dit quels jours feries elle chome.
    let ent = &ctx.entreprise;
    let ouvrables = Decimal::from(jours_ouvrables(&debut, &fin, Some(ent))?);
    let ouvrables_du_mois = if ouvrables != Decimal::ZERO { ouvrables } else { Decimal::ONE };
    let taux_jour_ouvrable = eur2(div(mul(salaire, fraction), ouvrables_du_mois));
    // Les heures de travail du mois, au reel (la mesure des absences : `imprevus::lignes_du_mois`).
    let heures_par_jour = heures_par_jour_oeuvre(quotite);
    let heures_du_mois = eur2(mul(heures_par_jour, Decimal::from(jours_oeuvres(&debut, &fin, Some(ent))?)));

    // --- les conges pris : une retenue, et une indemnite qui la couvre
    let mut pris = Decimal::ZERO;
    for c in e.get("conges_pris").and_then(|c| c.as_array()).map_or(&[][..], |a| &a[..]) {
        pris += Decimal::from(jours_ouvrables_de(c, ent)?);
    }
    if pris != Decimal::ZERO {
        let salarie = en_texte(requis(e, "salarie")?);
        let cpt = compteur_conges(ctx, &salarie, &fin, Some(mois))?;
        let maintien = eur2(mul(taux_jour_ouvrable, pris));
        let ind = indemnite_des_jours(ctx, &cpt, pris, maintien)?;
        lignes.push(json!({"rubrique": "absence_conges_payes", "quantite": flot(pris), "taux": flot(taux_jour_ouvrable)}));
        lignes.push(json!({
            "rubrique": "indemnite_conges_payes", "quantite": flot(pris),
            "taux": flot(eur2(div(ind.retenu, pris))),
        }));
        avert.push(info(format!(
            "{} : {} jour(s) ouvrable(s) de conges. Indemnite retenue : {} \
             ({}) -- le dixieme donnait {}, le maintien {} (L3141-24). \
             Solde apres ce mois : {} jours.",
            numero(e),
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
                numero(e),
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
                numero(e),
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
            numero(e),
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
    let numero = numero(e);
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
            numero,
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
            numero,
            liste(&restants).iter().map(|(j, n)| format!("{} ({})", n, j)).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(out)
}
