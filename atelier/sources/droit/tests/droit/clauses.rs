//! Les clauses conventionnelles (`clauses-conventionnelles-base.json`) contre `clauses/` : profil, preavis, lecture des
//! baremes, paliers, clause d'annexe, indemnites, prime d'anciennete. Des blocs (une grille par jeu de parametres, ses
//! resultats resumes par leur empreinte, `typee::empreinte`) et des cas lisibles (un resultat type par cas). Les grilles
//! (`clauses/grilles.json`) sont les entrees : une fiche s'y ecrit en texte et se lit par la porte du socle, comme une
//! fiche du releve. `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit les references.

use crate::outils::typee::{cas_de, chemin_donnee, empreinte, jouer, lire_chemin, Rendu};
use droit::clauses::{self as c, Bande, DitParLEngagement, Indemnite, PreavisDit, Prime, Profil, Retraite, Tranche, P};
use droit::fiche::ChampsSalarie;
use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::num::{Decimal, Resultat};
use std::str::FromStr;

// ------------------------------------------------------------- les entrees

/// Ce qu'un engagement de la grille dit du salarie.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Eng {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categorie_conventionnelle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cadre: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
}

/// Un profil de la grille (`{categorie, grille}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Prof {
    pub categorie: Option<String>,
    pub grille: Option<String>,
}

/// Les grilles : les entrees que chaque bloc croise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Grilles {
    pub fiches: Vec<String>,
    pub engagements: Vec<Option<Eng>>,
    pub anc_p: Vec<String>,
    pub ages_p: Vec<Option<String>>,
    pub annees: Vec<String>,
    pub profs: Vec<Prof>,
    pub motifs: Vec<Option<String>>,
    pub motifs_l: Vec<Option<String>>,
    pub anc_l: Vec<String>,
    pub refs_peu: Vec<String>,
    pub ages_l: Vec<Option<String>>,
    pub anc: Vec<String>,
    pub refs: Vec<String>,
    pub fiches_p: Vec<String>,
    pub eng_p: Vec<Option<Eng>>,
    pub jours: Vec<Jour>,
    pub jauges: Vec<Option<String>>,
}

pub(crate) fn grilles() -> Grilles {
    lire_chemin(&chemin_donnee("clauses/grilles.json"))
}

pub(crate) fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

/// La fiche ecrite `texte`, avec la case `cle` posee a `valeur` (un texte).
pub(crate) fn avec(texte: &str, cle: &str, valeur: &str) -> String {
    let corps = texte.trim().strip_prefix('{').and_then(|t| t.strip_suffix('}')).unwrap().trim();
    let case = format!("{}: {}", socle::ecriture::json_en_ligne(cle), socle::ecriture::json_en_ligne(valeur));
    if corps.is_empty() {
        format!("{{{}}}", case)
    } else {
        format!("{{{}, {}}}", corps, case)
    }
}

/// La fiche du releve, lue par sa forme : celle que la forme refuse ne parvient pas au droit.
pub(crate) fn champs(texte: &str) -> Resultat<ChampsSalarie> {
    socle::forme::lire_texte(texte, "la fiche du releve")
}

/// Ce qu'un engagement dit du salarie, lu comme le lit `engagement::solde` : une case vide ne dit rien, un debut
/// illisible refuse.
pub(crate) fn dit(e: Option<&Eng>) -> Resultat<DitParLEngagement> {
    let Some(e) = e else { return Ok(DitParLEngagement::default()) };
    let dite = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
    let debut = match dite(&e.debut) {
        Some(s) => Some(s.get(..10).filter(|j| j.as_bytes().get(4) == Some(&b'-') && j.as_bytes().get(7) == Some(&b'-')).and_then(|j| Jour::lire(j).ok()).ok_or("date illisible")?),
        None => None,
    };
    Ok(DitParLEngagement { categorie_conventionnelle: dite(&e.categorie_conventionnelle), statut: dite(&e.statut), metier: dite(&e.metier), cadre: e.cadre, debut })
}

pub(crate) fn profil_de(p: &Prof) -> Profil {
    Profil { categorie: p.categorie.clone(), grille: p.grille.clone(), source: String::new() }
}

// ------------------------------------------------------------- les calculs

pub(crate) fn profil_lu(idcc: &str, fiche: &str, eng: Option<&Eng>) -> Resultat<Profil> {
    c::profil(idcc, &champs(fiche)?, &dit(eng)?)
}

pub(crate) fn bloc_profil(g: &Grilles, idcc: &str, code: &str) -> Vec<Resultat<Profil>> {
    let mut out = Vec::new();
    for fi in &g.fiches {
        for en in &g.engagements {
            let par_l_engagement = Eng { metier: Some(code.to_string()), ..en.clone().unwrap_or_default() };
            out.push(profil_lu(idcc, fi, Some(&par_l_engagement)));
            out.push(profil_lu(idcc, &avec(fi, "code_emploi_ft", code), en.as_ref()));
        }
    }
    out
}

pub(crate) fn bloc_preavis(g: &Grilles, idcc: &str, categorie: Option<&str>, motif: Option<&str>) -> Vec<Resultat<PreavisDit>> {
    g.anc_p.iter().flat_map(|a| g.ages_p.iter().map(move |age| c::preavis_mois(idcc, categorie, dec(a), motif, age.as_deref().map(dec)))).collect()
}

pub(crate) fn annees(g: &Grilles) -> Vec<P> {
    g.annees.iter().map(|a| P::depuis(dec(a))).collect()
}

/// La lecture d'une liste du bareme (`texte`), en tranches ou en bandes (`genre`), a chaque anciennete de la grille ;
/// `None` : elle ne se lit pas ainsi.
pub(crate) fn bloc_lecture(g: &Grilles, texte: &str, genre: &str) -> Option<Vec<P>> {
    match genre {
        "tranches" => {
            let taux: Vec<Tranche> = socle::forme::lire_texte(texte, "").ok()?;
            Some(annees(g).into_iter().flat_map(|a| [c::par_tranches(&taux, a, true), c::par_tranches(&taux, a, false), c::Bareme { motifs: None, taux: Some(taux.clone()), bandes: None }.mois(a).unwrap()]).collect())
        }
        _ => {
            let bandes: Vec<Bande> = socle::forme::lire_texte(texte, "").ok()?;
            Some(annees(g).into_iter().map(|a| c::par_bandes(&bandes, a)).collect())
        }
    }
}

pub(crate) fn bloc_paliers(g: &Grilles, idcc: &str, section: &str) -> Vec<(P, Option<i64>)> {
    let t = c::table().unwrap();
    let conv = t.convention(idcc).unwrap();
    let (paliers, prorata) = match section {
        "mise_a_la_retraite" => conv.mise_a_la_retraite.as_ref().map(|m| (m.paliers.as_ref().unwrap(), m.prorata.as_ref())).unwrap(),
        _ => conv.depart_a_la_retraite.as_ref().map(|m| (m.paliers.as_ref().unwrap(), m.prorata.as_ref())).unwrap(),
    };
    annees(g).into_iter().map(|a| c::paliers(paliers, prorata, a)).collect()
}

pub(crate) fn bloc_annexe(g: &Grilles, idcc: &str, annexe: &str) -> Vec<Resultat<Option<c::ClauseDAnnexe>>> {
    g.profs.iter().flat_map(|pr| g.motifs.iter().map(move |m| c::clause_d_annexe(idcc, annexe, &profil_de(pr), m.as_deref()))).collect()
}

pub(crate) fn indemnite(idcc: &str, annexe: &str, pr: &Prof, anc: &str, r: &str, motif: Option<&str>, age: Option<&str>) -> Resultat<Indemnite> {
    c::indemnite_licenciement(idcc, annexe, &profil_de(pr), dec(anc), dec(r), motif, age.map(dec))
}

pub(crate) fn bloc_indemnite(g: &Grilles, idcc: &str, annexe: &str, pr: &Prof) -> Vec<Resultat<Indemnite>> {
    let mut out = Vec::new();
    for m in &g.motifs_l {
        for a in &g.anc_l {
            for r in &g.refs_peu {
                for age in &g.ages_l {
                    out.push(indemnite(idcc, annexe, pr, a, r, m.as_deref(), age.as_deref()));
                }
            }
        }
    }
    out
}

pub(crate) fn mise(idcc: &str, a: &str, r: &str) -> Resultat<Retraite> {
    c::indemnite_mise_a_la_retraite(idcc, dec(a), dec(r))
}

pub(crate) fn depart(idcc: &str, a: &str, r: &str) -> Resultat<Retraite> {
    c::indemnite_depart_retraite(idcc, dec(a), dec(r))
}

pub(crate) fn bloc_retraite(g: &Grilles, f: impl Fn(&str, &str) -> Resultat<Retraite>) -> Vec<Resultat<Retraite>> {
    g.anc.iter().flat_map(|a| g.refs.iter().map(|r| f(a, r)).collect::<Vec<_>>()).collect()
}

pub(crate) fn prime(idcc: &str, annexe: &str, fiche: &str, eng: Option<&Eng>, j: &Jour, jauge: Option<&str>) -> Resultat<Option<Prime>> {
    c::prime_anciennete(idcc, annexe, &champs(fiche)?, &dit(eng)?, j, jauge)
}

pub(crate) fn bloc_prime(g: &Grilles, idcc: &str, annexe: &str, code: &str) -> Vec<Resultat<Option<Prime>>> {
    let mut out = Vec::new();
    for fi in &g.fiches_p {
        let fiche = avec(fi, "code_emploi_ft", code);
        for en in &g.eng_p {
            for j in &g.jours {
                for jg in &g.jauges {
                    out.push(prime(idcc, annexe, &fiche, en.as_ref(), j, jg.as_deref()));
                }
            }
        }
    }
    out
}

// ------------------------------------------------------------- les resultats ecrits

fn nombre(p: Option<P>) -> Option<String> {
    p.map(|p| p.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProfilLu {
    categorie: Option<String>,
    grille: Option<String>,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreavisLu {
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    article: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jours: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndemniteLue {
    releve: bool,
    du: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    haut: Option<String>,
    lectures: Vec<(String, String)>,
    hors_primes: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    article: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetraiteLue {
    releve: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    montant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palier: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    article: Option<String>,
    hors_primes: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prevenance_mois: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cadre_de_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrimeLue {
    pourcentage: i64,
    base: Option<String>,
    article: String,
    pourquoi: Option<String>,
    annees: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnnexeLue {
    article: String,
    lectures: Vec<String>,
    hors_primes: Option<bool>,
    anciennete_minimale_mois: String,
    plafond_mois: Option<String>,
    motifs_legaux_seulement: Option<Vec<String>>,
}

/// Un resultat ecrit ; un refus, par sa phrase (ce qu'il porte a part nomme des chemins de la machine).
pub(crate) fn rendu<T, U>(r: Resultat<T>, f: impl Fn(T) -> U) -> Rendu<U> {
    r.map(f).map_err(|e| socle::echec::phrase_de(&e).to_string()).into()
}

pub(crate) fn profil_ecrit(p: Profil) -> ProfilLu {
    ProfilLu { categorie: p.categorie, grille: p.grille, source: p.source }
}

pub(crate) fn preavis_ecrit(p: PreavisDit) -> PreavisLu {
    PreavisLu { mois: nombre(p.mois), article: p.article, jours: nombre(p.jours), pourquoi: p.pourquoi }
}

pub(crate) fn indemnite_ecrite(i: Indemnite) -> IndemniteLue {
    IndemniteLue {
        releve: i.releve,
        du: i.du,
        bas: nombre(i.bas),
        haut: nombre(i.haut),
        lectures: i.lectures.into_iter().map(|(k, v)| (k, v.to_string())).collect(),
        hors_primes: i.hors_primes,
        article: i.article,
        pourquoi: i.pourquoi,
    }
}

pub(crate) fn retraite_ecrite(r: Retraite) -> RetraiteLue {
    RetraiteLue {
        releve: r.releve,
        montant: nombre(r.montant),
        mois: nombre(r.mois),
        palier: r.palier,
        article: r.article,
        hors_primes: r.hors_primes,
        prevenance_mois: r.prevenance_mois,
        cadre_de_direction: r.cadre_de_direction,
        pourquoi: r.pourquoi,
    }
}

pub(crate) fn prime_ecrite(p: Option<Prime>) -> Option<PrimeLue> {
    p.map(|p| PrimeLue { pourcentage: p.pourcentage, base: nombre(p.base), article: p.article, pourquoi: p.pourquoi, annees: p.annees })
}

pub(crate) fn annexe_ecrite(a: Option<c::ClauseDAnnexe>) -> Option<AnnexeLue> {
    a.map(|a| AnnexeLue {
        article: a.article,
        lectures: a.lectures.iter().map(|(n, _)| n.clone()).collect(),
        hors_primes: a.hors_primes,
        anciennete_minimale_mois: a.anciennete_minimale_mois.0.to_string(),
        plafond_mois: a.plafond_mois.map(|m| m.0.to_string()),
        motifs_legaux_seulement: a.motifs_legaux_seulement,
    })
}

/// L'empreinte d'un bloc : ses resultats ecrits, un par ligne.
fn bloc<T, U: Serialize>(r: Vec<Resultat<T>>, f: impl Fn(T) -> U + Copy) -> String {
    empreinte(&r.into_iter().map(|x| rendu(x, f)).collect::<Vec<_>>())
}

// ------------------------------------------------------------- les cas

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasProfil {
    pub idcc: String,
    pub fiche: String,
    pub engagement: Option<Eng>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasPreavis {
    pub idcc: String,
    pub categorie: Option<String>,
    pub anciennete: String,
    pub motif: Option<String>,
    pub age: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasAnnexe {
    pub idcc: String,
    pub annexe: String,
    pub profil: Prof,
    pub motif: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasIndemnite {
    pub idcc: String,
    pub annexe: String,
    pub profil: Prof,
    pub anciennete: String,
    pub reference: String,
    pub motif: Option<String>,
    pub age: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasRetraite {
    pub idcc: String,
    pub anciennete: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasPrime {
    pub idcc: String,
    pub annexe: String,
    pub fiche: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engagement: Option<Eng>,
    pub jour: Jour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jauge: Option<String>,
}

pub(crate) fn cas_profil(c: &CasProfil) -> Rendu<ProfilLu> {
    rendu(profil_lu(&c.idcc, &c.fiche, c.engagement.as_ref()), profil_ecrit)
}

pub(crate) fn cas_preavis(x: &CasPreavis) -> Rendu<PreavisLu> {
    rendu(c::preavis_mois(&x.idcc, x.categorie.as_deref(), dec(&x.anciennete), x.motif.as_deref(), x.age.as_deref().map(dec)), preavis_ecrit)
}

pub(crate) fn cas_annexe(x: &CasAnnexe) -> Rendu<Option<AnnexeLue>> {
    rendu(c::clause_d_annexe(&x.idcc, &x.annexe, &profil_de(&x.profil), x.motif.as_deref()), annexe_ecrite)
}

pub(crate) fn cas_indemnite(x: &CasIndemnite) -> Rendu<IndemniteLue> {
    rendu(indemnite(&x.idcc, &x.annexe, &x.profil, &x.anciennete, &x.reference, x.motif.as_deref(), x.age.as_deref()), indemnite_ecrite)
}

pub(crate) fn cas_mise(x: &CasRetraite) -> Rendu<RetraiteLue> {
    rendu(mise(&x.idcc, &x.anciennete, &x.reference), retraite_ecrite)
}

pub(crate) fn cas_depart(x: &CasRetraite) -> Rendu<RetraiteLue> {
    rendu(depart(&x.idcc, &x.anciennete, &x.reference), retraite_ecrite)
}

pub(crate) fn cas_prime(x: &CasPrime) -> Rendu<Option<PrimeLue>> {
    rendu(prime(&x.idcc, &x.annexe, &x.fiche, x.engagement.as_ref(), &x.jour, x.jauge.as_deref()), prime_ecrite)
}

// ------------------------------------------------------------- les blocs joues

pub(crate) fn empreinte_profil(g: &Grilles, (idcc, code): &(String, String)) -> String {
    bloc(bloc_profil(g, idcc, code), profil_ecrit)
}

pub(crate) fn empreinte_preavis(g: &Grilles, (idcc, categorie, motif): &(String, Option<String>, Option<String>)) -> String {
    bloc(bloc_preavis(g, idcc, categorie.as_deref(), motif.as_deref()), preavis_ecrit)
}

/// Une liste d'un bareme, son chemin dans le fichier livre (ce qu'elle est), son texte, et sa lecture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CasLecture {
    pub chemin: Vec<String>,
    pub liste: String,
    pub genre: String,
}

pub(crate) fn empreinte_lecture(g: &Grilles, x: &CasLecture) -> String {
    empreinte(&bloc_lecture(g, &x.liste, &x.genre).unwrap().iter().map(|p| p.to_string()).collect::<Vec<_>>())
}

pub(crate) fn empreinte_paliers(g: &Grilles, (idcc, section): &(String, String)) -> String {
    empreinte(&bloc_paliers(g, idcc, section).into_iter().map(|(m, p)| (m.to_string(), p)).collect::<Vec<_>>())
}

pub(crate) fn empreinte_annexe(g: &Grilles, (idcc, annexe): &(String, String)) -> String {
    bloc(bloc_annexe(g, idcc, annexe), annexe_ecrite)
}

pub(crate) fn empreinte_indemnite(g: &Grilles, (idcc, annexe, pr): &(String, String, Prof)) -> String {
    bloc(bloc_indemnite(g, idcc, annexe, pr), indemnite_ecrite)
}

pub(crate) fn empreinte_mise(g: &Grilles, idcc: &String) -> String {
    bloc(bloc_retraite(g, |a, r| mise(idcc, a, r)), retraite_ecrite)
}

pub(crate) fn empreinte_depart(g: &Grilles, idcc: &String) -> String {
    bloc(bloc_retraite(g, |a, r| depart(idcc, a, r)), retraite_ecrite)
}

pub(crate) fn empreinte_prime(g: &Grilles, (idcc, annexe, code): &(String, String, String)) -> String {
    bloc(bloc_prime(g, idcc, annexe, code), prime_ecrite)
}

// ----------------------------------------------------------------- les tests
#[test]
fn profil() {
    let g = grilles();
    jouer("clauses/profil-blocs.json", cas_de("clauses/profil-blocs.json"), |p| empreinte_profil(&g, p));
    jouer("clauses/profil.json", cas_de("clauses/profil.json"), cas_profil);
    jouer("clauses/nom.json", cas_de::<String>("clauses/nom.json"), |i| c::nom(i));
    jouer("clauses/releve.json", cas_de::<String>("clauses/releve.json"), |i| rendu(c::releve(i), |r| r.is_some()));
}

#[test]
fn preavis() {
    let g = grilles();
    jouer("clauses/preavis_mois-blocs.json", cas_de("clauses/preavis_mois-blocs.json"), |p| empreinte_preavis(&g, p));
    jouer("clauses/preavis_mois.json", cas_de("clauses/preavis_mois.json"), cas_preavis);
}

/// Les baremes : chaque liste de la grille lue comme des tranches (en annees revolues ou non, et comme elle le dit) ou
/// comme des bandes, selon sa forme ; les paliers de retraite de chaque section relevee.
#[test]
fn les_baremes() {
    let g = grilles();
    jouer("clauses/lecture-blocs.json", cas_de("clauses/lecture-blocs.json"), |p| empreinte_lecture(&g, p));
    jouer("clauses/paliers-blocs.json", cas_de("clauses/paliers-blocs.json"), |p| empreinte_paliers(&g, p));
}

#[test]
fn clause_d_annexe() {
    let g = grilles();
    jouer("clauses/clause_d_annexe-blocs.json", cas_de("clauses/clause_d_annexe-blocs.json"), |p| empreinte_annexe(&g, p));
    jouer("clauses/clause_d_annexe.json", cas_de("clauses/clause_d_annexe.json"), cas_annexe);
}

#[test]
fn indemnite_de_licenciement() {
    let g = grilles();
    jouer("clauses/indemnite_licenciement-blocs.json", cas_de("clauses/indemnite_licenciement-blocs.json"), |p| empreinte_indemnite(&g, p));
    jouer("clauses/indemnite_licenciement.json", cas_de("clauses/indemnite_licenciement.json"), cas_indemnite);
}

#[test]
fn retraite() {
    let g = grilles();
    jouer("clauses/indemnite_mise_a_la_retraite-blocs.json", cas_de("clauses/indemnite_mise_a_la_retraite-blocs.json"), |p| empreinte_mise(&g, p));
    jouer("clauses/indemnite_depart_retraite-blocs.json", cas_de("clauses/indemnite_depart_retraite-blocs.json"), |p| empreinte_depart(&g, p));
    jouer("clauses/indemnite_mise_a_la_retraite.json", cas_de("clauses/indemnite_mise_a_la_retraite.json"), cas_mise);
    jouer("clauses/indemnite_depart_retraite.json", cas_de("clauses/indemnite_depart_retraite.json"), cas_depart);
}

#[test]
fn prime_d_anciennete() {
    let g = grilles();
    jouer("clauses/prime_anciennete-blocs.json", cas_de("clauses/prime_anciennete-blocs.json"), |p| empreinte_prime(&g, p));
    jouer("clauses/prime_anciennete.json", cas_de("clauses/prime_anciennete.json"), cas_prime);
}

/// LA REMISE DU CONTRAT A DUREE DETERMINEE : « au plus tard, dans les deux jours ouvrables suivant l'embauche »
/// (code du travail, L. 1242-13, `atelier/reference/sources/code-du-travail/xml/L1242-13.xml`) ; pour l'artiste
/// musicien de la 1285, « au plus tard 15 jours ouvrables précédant l'engagement » (titre XV, `remise_du_contrat` de sa
/// couche, `atelier/reference/sources/chomage-intermittents/kali-1285-organisation-travail-repetitions-representations-
/// 20261020.md`, l. 763). Un jour ouvrable va du lundi au samedi.
#[test]
fn la_remise_du_contrat_suit_l1242_13_et_la_1285_pour_ses_musiciens() {
    let j = |s: &str| Jour::lire(s).unwrap();
    // Un vendredi : le samedi, puis le lundi (le dimanche n'est pas ouvrable).
    let loi = droit::echeances::remise_du_contrat(&j("2026-11-20"), "3090", "ART170", None).unwrap();
    assert_eq!(loi.jour, j("2026-11-23"));
    assert!(loi.source.contains("L. 1242-13"), "{}", loi.source);
    // Le meme musicien sous la 1285 : quinze jours ouvrables avant le jeudi 10/12, sans compter les dimanches.
    let musicien = droit::echeances::remise_du_contrat(&j("2026-12-10"), "1285", "ART170", None).unwrap();
    assert_eq!(musicien.jour, j("2026-11-23"));
    assert!(musicien.source.contains("1285") && musicien.source.contains("XV"), "{}", musicien.source);
    // Un metier hors du titre XV, sous la 1285 : la loi seule.
    let autre = droit::echeances::remise_du_contrat(&j("2026-12-10"), "1285", "ZZZ999", None).unwrap();
    assert_eq!(autre.jour, j("2026-12-12"));
    assert!(autre.source.contains("L. 1242-13"), "{}", autre.source);
}
