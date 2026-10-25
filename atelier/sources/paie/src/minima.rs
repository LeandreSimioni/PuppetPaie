//! Les minima conventionnels confrontes a une paie (`minima.py`) : des
//! avertissements, jamais un refus. `avertissements` lit la grille de la
//! convention ; `sous_le_smic` lit la loi. Ni `etat` ni `non_compares` : ils ne
//! servent qu'au rapport en ligne de commande.
//!
//! Chaque minimum prend, quand la paie ne dit pas tout, l'hypothese la plus
//! basse : un avertissement qui crie a tort ne se lit plus.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rust_decimal::RoundingStrategy;
use serde_json::Value;

use crate::calcul::{LigneBrut, Paie};
use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use socle::texte::sigle;
use socle::date::{en_texte, repr_texte, Jour};
use droit::emplois;
use crate::engagement::{Avis, Engagement};
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use socle::num::{add, dec, dec_f64, div, mul, sub, Decimal, Resultat};

static NUL: Value = Value::Null;
static SANS: AtomicBool = AtomicBool::new(false);

/// Coupe les minima de `charger` (le `--sans-minima` de l'oracle).
pub fn sans_minima(oui: bool) {
    SANS.store(oui, Ordering::SeqCst);
}

pub fn actif() -> bool {
    !SANS.load(Ordering::SeqCst)
}

fn cachet(c: &str) -> bool {
    c == "cachet"
}
fn repetition(c: &str) -> bool {
    c == "repetition"
}
fn horaire(c: &str) -> bool {
    matches!(c, "heure" | "journee" | "semaine")
}
fn mensuel(c: &str) -> bool {
    c == "mois"
}

pub fn table() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("minima-conventionnels.json"))
}

// ------------------------------------------------------------- les petits outils

/// `x.get(k)` ; `Null` si absent. `Null.get(..)` reste vide : c'est le `or {}`.
fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// `x[k]` : un `KeyError` si absent.
fn cle<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// `str(x or "")`.
fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v)
    } else {
        String::new()
    }
}

fn texte_de(v: &Value, k: &str) -> String {
    txt(g(v, k))
}

/// `x or None`, lu comme texte.
fn ou_rien(v: &Value) -> Option<String> {
    vrai(v).then(|| en_texte(v))
}

fn plus_bas<T>(v: Vec<T>, valeur: impl Fn(&T) -> Decimal) -> Option<T> {
    let mut mini: Option<(Decimal, T)> = None;
    for x in v {
        let k = valeur(&x);
        if mini.as_ref().map_or(true, |(m, _)| k < *m) {
            mini = Some((k, x));
        }
    }
    mini.map(|(_, x)| x)
}

/// `Decimal.quantize(Decimal("0.01"), ROUND_DOWN)`.
fn bas(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

/// `Decimal.quantize(Decimal("0.01"))` : au pair sur le demi.
fn pair(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven)
}

/// `float(x)` d'un `Decimal`, correctement arrondi.
fn flottant(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(0.0)
}

/// `"%.2f" % float(x)` avec la virgule.
fn euros(x: Decimal) -> String {
    euros_f(flottant(x))
}

fn euros_f(x: f64) -> String {
    format!("{:.2}", x).replace('.', ",")
}

/// `round(x, 2)` d'un flottant, comme CPython.
fn arrondi2(x: f64) -> f64 {
    format!("{:.2}", x).parse().unwrap_or(x)
}

/// « 12 », « 151,67 » : des heures qui se lisent.
fn heures_texte(x: Decimal) -> String {
    let x = x.normalize();
    if x == x.trunc() {
        return x.trunc().to_string();
    }
    let t = format!("{:.2}", flottant(x));
    t.trim_end_matches('0').trim_end_matches('.').replace('.', ",")
}

pub fn qui(sal: &Value) -> String {
    let nom = sal.get("nom").map(en_texte).unwrap_or_default();
    let prenom = match g(sal, "prenom_affichage") {
        p if vrai(p) => en_texte(p),
        _ => sal.get("prenom").map(en_texte).unwrap_or_default(),
    };
    format!("{} {}", nom, prenom)
}

/// `date.fromisoformat(str(x)[:10])` : AAAA-MM-JJ ou AAAAMMJJ.
fn iso(s: &str) -> Option<Jour> {
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

fn ans_revolus(depuis: &Value, jour: Option<&str>) -> Option<i64> {
    let a = iso(&en_texte(depuis))?;
    let b = iso(jour?)?;
    Some(b.an as i64 - a.an as i64 - i64::from((b.mois, b.jour) < (a.mois, a.jour)))
}

/// La duree du contrat en jours calendaires, bornes comprises.
fn jours_du_contrat(eng: &Engagement) -> Option<i64> {
    Some(eng.fin?.moins(&eng.debut) + 1)
}

/// Le dernier jour de la periode payee, en AAAA-MM-JJ.
fn jour_de_paie(paie: &Paie) -> Option<String> {
    paie.engagement.fin.map(|j| j.iso())
}

fn premier_jour(paie: &Paie) -> Option<String> {
    paie.engagement.dates_travail.iter().map(|j| j.iso()).min()
}

fn permanent(paie: &Paie) -> bool {
    paie.engagement.nat().map_or(false, |n| n.mensuelle())
}

/// `entreprise.convention_du_contrat` : la convention dont ce contrat lit les
/// minima. Un refus du Python retombe sur la convention de l'entreprise.
fn idcc_du_contrat(ent: &Value, paie: &Paie) -> Resultat<String> {
    let idcc = texte_de(ent, "idcc");
    if (idcc != "2642" && idcc != "3241") || !vrai(g(&paie.spectacle, "emission_tv_1734")) {
        return Ok(idcc);
    }
    let statut = match g(&paie.salarie, "statut") {
        s if vrai(s) => en_texte(s),
        _ => "artiste".into(),
    };
    if statut != "artiste" {
        return Ok(idcc);
    }
    let code = texte_de(&paie.salarie, "code_emploi_ft");
    let m = if code.is_empty() { None } else { emplois::metier(&code)? };
    match m.as_ref().and_then(|m| m.get("famille")) {
        Some(f) if matches!(en_texte(f).as_str(), "Artistes" | "Cirque et rue") => Ok("1734".into()),
        _ => Ok(idcc),
    }
}

// ------------------------------------------------------------------ les planchers

/// Un minimum : le montant, d'ou il vient, et s'il se compare a l'unite.
pub struct Plancher {
    pub montant: Decimal,
    pub quoi: String,
    pub unite: bool,
}

fn plancher(montant: Decimal, quoi: String) -> Option<Plancher> {
    Some(Plancher { montant, quoi, unite: false })
}

/// Le montant du palier ou tombe `nombre` de representations.
fn palier(paliers: &Value, nombre: i64) -> Resultat<Option<Decimal>> {
    for p in paliers.as_array().into_iter().flatten() {
        let j = cle(p, "jusqu_a")?;
        if j.is_null() || j.as_f64().map_or(false, |x| nombre as f64 <= x) {
            return Ok(Some(dec(cle(p, "montant")?)?));
        }
    }
    Ok(None)
}

/// La grille NAO non etendue de la CCNEAC, si elle s'impose a cette compagnie :
/// adherente d'une organisation signataire, travail posterieur a l'entree en vigueur.
pub fn grille_non_etendue<'a>(t: &'a Value, organisation: Option<&str>, jour: Option<&str>) -> Option<&'a Value> {
    let ne = g(g(t, "1285"), "non_etendu");
    let (organisation, jour) = (organisation?, jour?);
    if !vrai(ne) {
        return None;
    }
    let s = sigle(organisation);
    let signe = g(ne, "signataires").as_array().into_iter().flatten().any(|x| sigle(&en_texte(x)) == s);
    if !signe {
        return None;
    }
    let vigueur = match g(ne, "en_vigueur") {
        v if vrai(v) => en_texte(v),
        _ => "9999".into(),
    };
    let jour: String = jour.chars().take(10).collect();
    (jour >= vigueur).then_some(ne)
}

/// `ne["texte"].split(",")[0]`.
fn premiere_partie(v: &Value) -> Resultat<String> {
    Ok(en_texte(cle(v, "texte")?).split(',').next().unwrap_or("").to_string())
}

/// Les donnees d'une ligne et de la paie qu'un plancher lit.
struct Cx<'a> {
    t: &'a Value,
    idcc: &'a str,
    m: &'a Value,
    sal: &'a Value,
    paie: &'a Paie,
    ent: &'a Value,
    annexe: Option<String>,
    organisation: Option<String>,
    nb_cachets: i64,
}

impl<'a> Cx<'a> {
    fn refs(&self) -> &'a Value {
        g(g(self.m, "minima"), "3090")
    }

    /// Les annexes de la 3090 ou ce metier a une ligne, restreintes a celle de
    /// l'entreprise quand elle est connue.
    fn annexes_candidates(&self) -> Resultat<Vec<String>> {
        let refs = self.refs();
        if let Some(a) = &self.annexe {
            if refs.get(a).is_some() {
                return Ok(vec![a.clone()]);
            }
            if cle(cle(self.t, "3090")?, "annexes")?.get(a).is_some() {
                return Ok(vec![]);
            }
        }
        Ok(refs.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default())
    }

    /// Les annexes candidates, ou a defaut celle de l'entreprise si la table la connait.
    fn annexes_ou_la_sienne(&self) -> Resultat<Vec<String>> {
        let c = self.annexes_candidates()?;
        if !c.is_empty() {
            return Ok(c);
        }
        let annexes = cle(cle(self.t, "3090")?, "annexes")?;
        Ok(match &self.annexe {
            Some(a) if annexes.get(a).is_some() => vec![a.clone()],
            _ => vec![],
        })
    }

    fn plancher_cachet_3090(&self) -> Resultat<Option<Plancher>> {
        let t3090 = cle(self.t, "3090")?;
        let refs = self.refs();
        let mut meilleurs: Vec<(Decimal, String, &Value)> = Vec::new();
        for annexe in self.annexes_candidates()? {
            let ligne = cle(refs, &annexe)?;
            let grille = cle(cle(cle(cle(t3090, "annexes")?, &annexe)?, "cachets")?, ligne.as_str().ok_or_else(|| en_texte(ligne))?)?;
            if let Some(montant) = palier(cle(grille, "paliers")?, self.nb_cachets)? {
                meilleurs.push((montant, annexe, grille));
            }
        }
        if meilleurs.is_empty() {
            return Ok(None);
        }
        let toutes = meilleurs.len() > 1;
        let mut noms: Vec<String> = meilleurs.iter().map(|x| x.1.clone()).collect();
        noms.sort();
        let (montant, annexe, grille) = plus_bas(meilleurs, |x| x.0).unwrap();
        let mut quoi = format!("CCN 3090, annexe {}, {}", annexe, en_texte(cle(grille, "libelle")?));
        if vrai(g(grille, "jauge")) {
            quoi += " (plus petite jauge retenue)";
        }
        if self.annexe.is_none() && toutes {
            quoi += &format!(" -- la plus basse des annexes {}, l'entreprise ne disant pas la sienne", noms.join(" et "));
        }
        Ok(plancher(montant, quoi))
    }

    fn non_etendue(&self) -> Option<&'a Value> {
        let jour = premier_jour(self.paie);
        grille_non_etendue(self.t, self.organisation.as_deref(), jour.as_deref())
    }

    fn plancher_cachet_1285(&self) -> Resultat<Option<Plancher>> {
        let r = g(g(self.m, "minima"), "1285");
        if !vrai(r) {
            return Ok(None);
        }
        let reference = en_texte(r);
        let grille = cle(cle(cle(self.t, "1285")?, "cachets")?, &reference)?;
        if let Some(ne) = self.non_etendue() {
            if let Some(cachets) = g(ne, "cachets").get(&reference) {
                if let Some(montant) = palier(cachets, self.nb_cachets)? {
                    return Ok(plancher(
                        montant,
                        format!(
                            "CCNEAC (1285), {} -- {}, que la compagnie doit comme adherente de {}",
                            en_texte(cle(grille, "libelle")?),
                            premiere_partie(ne)?,
                            self.organisation.clone().unwrap_or_default()
                        ),
                    ));
                }
            }
        }
        let Some(montant) = palier(cle(grille, "paliers")?, self.nb_cachets)? else { return Ok(None) };
        Ok(plancher(montant, format!("CCNEAC (1285), {}", en_texte(cle(grille, "libelle")?))))
    }

    fn plancher_repetition(&self) -> Resultat<Option<Plancher>> {
        match self.idcc {
            "1285" => {
                let r = cle(cle(self.t, "1285")?, "repetition")?;
                let reference = texte_de(g(self.m, "minima"), "1285");
                let ne = self.non_etendue();
                let propre = g(g(r, "par_categorie"), &reference);
                if let Some(ne) = ne {
                    let p = g(g(g(ne, "repetition"), "par_categorie"), &reference);
                    if vrai(p) {
                        return Ok(plancher(
                            dec(cle(p, "service")?)?,
                            format!(
                                "CCNEAC (1285), service de repetition, {} -- {}, que la compagnie doit comme adherente de {}",
                                en_texte(cle(p, "libelle")?),
                                premiere_partie(ne)?,
                                self.organisation.clone().unwrap_or_default()
                            ),
                        ));
                    }
                }
                if vrai(propre) {
                    return Ok(plancher(dec(cle(propre, "service")?)?, format!("CCNEAC (1285), {}", en_texte(cle(propre, "libelle")?))));
                }
                Ok(plancher(dec(cle(r, "service")?)?, format!("CCNEAC (1285), {}", en_texte(cle(r, "service_libelle")?))))
            }
            "3090" => {
                // Une heure n'est pas un cachet : seul un taux horaire de repetition se compare.
                let t3090 = cle(self.t, "3090")?;
                let mut candidats = Vec::new();
                for annexe in self.annexes_ou_la_sienne()? {
                    let r = g(cle(cle(t3090, "annexes")?, &annexe)?, "repetition");
                    if let Some(h) = r.get("heure") {
                        candidats.push((dec(h)?, annexe));
                    }
                }
                Ok(plus_bas(candidats, |x| x.0).map(|(montant, annexe)| Plancher {
                    montant,
                    quoi: format!("CCN 3090, annexe {}, services de repetition (taux horaire)", annexe),
                    unite: false,
                }))
            }
            _ => Ok(None),
        }
    }

    /// L'echelon de la CCNEAC : celui de la fiche (1 a 12), ou le premier.
    fn echelon_ccneac(&self) -> (usize, String) {
        let v = texte_de(self.sal, "echelon_ccneac");
        let v = v.trim();
        if !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()) {
            if let Ok(n) = v.parse::<usize>() {
                if (1..=12).contains(&n) {
                    return (n, format!("echelon {} de la fiche", v));
                }
            }
        }
        (1, "echelon 1, la fiche ne disant pas le sien (`echelon_ccneac`)".into())
    }

    /// Le taux horaire minimum d'un emploi autre qu'artistique de la CCNEAC.
    fn plancher_horaire_1285(&self) -> Resultat<Option<Plancher>> {
        let groupe = texte_de(g(self.m, "classification_1285"), "groupe");
        let grille = g(cle(self.t, "1285")?, "emplois_autres_qu_artistiques");
        if groupe.is_empty() || g(grille, "groupes").get(&groupe).is_none() {
            return Ok(None);
        }
        let (echelon, d_ou) = self.echelon_ccneac();
        let mensuel = cle(g(grille, "groupes"), &groupe)?
            .get(echelon - 1)
            .ok_or_else(|| socle::fichiers::trop_courte(&format!("l'échelon {} du groupe {}", echelon, groupe)))?
            .as_f64()
            .ok_or("nombre attendu")?;
        let heures = cle(grille, "heures_mensuelles")?.as_f64().ok_or("nombre attendu")?;
        let horaire = arrondi2(mensuel / heures);
        Ok(plancher(
            dec_f64(horaire)?,
            format!(
                "CCNEAC (1285), groupe {} ({}), {} : {} EUR pour 151 h 40, ramenes a l'heure",
                groupe,
                texte_de(g(self.m, "classification_1285"), "emploi_xi3"),
                d_ou,
                euros_f(mensuel)
            ),
        ))
    }

    /// La fonction d'un metier dans la grille de la 2642 ou de la 3097 : celle de
    /// la fiche quand la grille la connait, sinon celle du catalogue.
    fn fonction(&self, idcc: &str, fonctions: &[&Value]) -> Option<String> {
        let propre = texte_de(self.sal, "classement_conventionnel");
        let propre = propre.trim();
        if !propre.is_empty() && fonctions.iter().any(|f| f.get(propre).is_some()) {
            return Some(propre.to_string());
        }
        ou_rien(g(g(self.m, &format!("classification_{}", idcc)), "fonction"))
    }

    fn plancher_2642(&self) -> Resultat<Option<Plancher>> {
        let c = cle(self.t, "2642")?;
        let (cddu, permanents) = (g(c, "cddu_semaine_35h"), g(c, "permanents_mensuel_35h"));
        let Some(fonction) = self.fonction("2642", &[cddu, permanents]) else { return Ok(None) };
        let d = |s: &str| dec(&Value::String(s.into()));
        if permanent(self.paie) {
            let Some(mensuel) = permanents.get(&fonction) else { return Ok(None) };
            let m = dec(mensuel)?;
            let h = bas(div(m, d("151.67")?));
            return Ok(plancher(
                h,
                format!(
                    "CCN 2642, {} en CDI ou CDD de droit commun : {} EUR par mois pour 35 h, ramenes a l'heure sur 151,67 h",
                    fonction,
                    euros(m)
                ),
            ));
        }
        let grille = g(cddu, &fonction);
        let presents: Vec<&str> = ["fiction", "flux", "autre"].into_iter().filter(|x| grille.get(*x).is_some()).collect();
        if presents.is_empty() {
            return Ok(None);
        }
        let genre = texte_de(&self.paie.spectacle, "genre_audiovisuel");
        let (semaine, dit) = if presents.contains(&genre.as_str()) {
            (dec(cle(grille, &genre)?)?, format!("programme de {}", genre))
        } else {
            let mut valeurs = Vec::new();
            for x in &presents {
                valeurs.push((*x, dec(cle(grille, x)?)?));
            }
            let (g, s) = plus_bas(valeurs, |x| x.1).unwrap();
            (s, format!("genre {}, le plus bas des genres de cette fonction -- la fiche du spectacle ne dit pas le sien (`genre_audiovisuel`)", g))
        };
        let jours = jours_du_contrat(&self.paie.engagement);
        let (h, regle) = match jours {
            Some(j) if j <= 4 => (div(semaine, d("31.5")?), "contrat de 4 jours ou moins : la semaine / 4,5 par journee de 7 h"),
            Some(j) if j >= 89 => {
                let mensuel = mul(semaine, d("3.8")?);
                let smic = mul(mul(dec_f64(self.paie.smic)?, d("151.67")?), d("1.3")?);
                let mensuel = if smic > mensuel { smic } else { mensuel };
                (div(mensuel, d("151.67")?), "contrat de 3 mois ou plus : la semaine x 3,8 par mois de 151,67 h, pas moins de 130 % du SMIC")
            }
            None => (div(mul(semaine, d("3.8")?), d("151.67")?), "duree du contrat inconnue : la base mensuelle, la plus basse"),
            Some(_) => (div(semaine, d("35")?), "la semaine de 35 h"),
        };
        Ok(plancher(
            bas(h),
            format!(
                "CCN 2642, {} en CDDU, {} : {} EUR la semaine de 35 h ; {} (IV.2.1)",
                fonction,
                dit,
                euros(semaine),
                regle
            ),
        ))
    }

    fn plancher_3097(&self, ligne: &LigneBrut) -> Resultat<Option<Plancher>> {
        let c = cle(self.t, "3097")?;
        let d = |s: &str| dec(&Value::String(s.into()));
        let spec = &self.paie.spectacle;
        let statut = texte_de(self.m, "statut");
        let a_fonction = vrai(g(g(self.m, "classification_3097"), "fonction"));
        let heures = !ligne.heures_annexes.is_zero();
        if (statut == "technicien" || a_fonction) && (horaire(&ligne.code) || mensuel(&ligne.code)) && heures {
            if permanent(self.paie) {
                let emploi = g(g(self.m, "classification_3097"), "titre_iv");
                let pm = cle(c, "permanents_mensuel")?;
                let niveau = if vrai(emploi) { g(cle(pm, "emplois_reperes")?, &en_texte(emploi)) } else { &NUL };
                if niveau.is_null() {
                    return Ok(None);
                }
                let niveau = en_texte(niveau);
                let mensuel = dec(cle(cle(pm, "niveaux")?, &niveau)?)?;
                return Ok(plancher(
                    bas(div(mensuel, d("151.67")?)),
                    format!(
                        "CCN 3097, titre IV, {} ({}) : {} EUR par mois, salaire de base sans les complements, ramene a l'heure sur 151,67 h",
                        en_texte(emploi),
                        niveau.replace('_', " "),
                        euros(mensuel)
                    ),
                ));
            }
            let techniciens = cle(c, "techniciens_semaine_39h")?;
            let Some(fonction) = self.fonction("3097", &[techniciens]) else { return Ok(None) };
            let Some(semaine) = techniciens.get(&fonction) else { return Ok(None) };
            let mut semaine = semaine;
            let debut = self.paie.engagement.debut.iso();
            let avant = g(g(c, "_avant_2026_05"), "montants");
            if debut.as_str() < "2026-05-01" {
                if let Some(a) = avant.get(&fonction) {
                    semaine = a;
                }
            }
            let mut semaine = dec(semaine)?;
            let mut dit = format!("{} EUR la semaine de 39 h (annexe I du titre II)", euros(semaine));
            if vrai(g(spec, "derogation_annexe_iii_3097")) {
                let a = cle(c, "annexe_iii")?;
                let socle = dec(cle(a, "socle")?)?;
                if semaine > socle {
                    semaine = add(socle, div(mul(sub(semaine, socle), dec(cle(a, "part_au_dela")?)?), Decimal::ONE_HUNDRED));
                }
                dit = format!("film sous derogation de l'annexe III : {} EUR la semaine de 39 h", euros(bas(semaine)));
            }
            let h = bas(div(semaine, dec(cle(c, "_heures_payees_de_la_semaine")?)?));
            return Ok(plancher(h, format!("CCN 3097, {} : {}, soit 40 h au taux de base (35 h + 4 h a 125 %)", fonction, dit)));
        }
        if statut == "artiste" && cachet(&ligne.code) {
            let r = g(g(self.m, "minima"), "3097");
            if !vrai(r) {
                return Ok(None);
            }
            let reference = en_texte(r);
            let fmt = texte_de(spec, "format_cinema");
            let (grille, quoi) = if reference == "acteur" {
                let cl = if fmt == "long" { "long" } else { "court" };
                let sous = if fmt == "long" || fmt == "court" {
                    String::new()
                } else {
                    " (le plus bas : la fiche du spectacle ne dit pas le format, `format_cinema`)".into()
                };
                (cle(cle(c, "artistes")?, cl)?, format!("artiste-interprete, {}-metrage{}", cl, sous))
            } else {
                let complement = cle(c, "complement")?;
                if fmt != "long" || complement.get(&reference).is_none() {
                    return Ok(None);
                }
                (cle(complement, &reference)?, format!("acteur de complement ({}), long-metrage", reference.replace('_', " ")))
            };
            let (montant, regle) = if self.nb_cachets >= 5 {
                let s5 = div(dec(cle(grille, "semaine_5j")?)?, Decimal::from(5));
                let s6 = div(dec(cle(grille, "semaine_6j")?)?, Decimal::from(6));
                (bas(if s6 < s5 { s6 } else { s5 }), "engagement a la semaine, rapporte au cachet")
            } else {
                (dec(cle(grille, "journee")?)?, "engagement a la journee")
            };
            return Ok(plancher(montant, format!("CCN 3097, titre III, {}, {}", quoi, regle)));
        }
        Ok(None)
    }

    fn a_grilles(&self) -> bool {
        vrai(g(g(self.t, self.idcc), "grilles"))
    }

    fn conditions(&self, c: &Value) -> Resultat<bool> {
        let m = self.m;
        if let Some(cats) = c.get("categories") {
            let ref_ = texte_de(g(m, "minima"), self.idcc);
            if !cats.as_array().into_iter().flatten().any(|x| x.as_str() == Some(ref_.as_str())) {
                return Ok(false);
            }
        }
        if let Some(codes) = c.get("codes") {
            let code = g(m, "code");
            if !codes.as_array().into_iter().flatten().any(|x| x == code) {
                return Ok(false);
            }
        }
        for (k, source) in [("si_spectacle", &self.paie.spectacle), ("si_entreprise", self.ent), ("si_salarie", self.sal)] {
            for (cl, v) in g(c, k).as_object().into_iter().flatten() {
                if !vaut(v, g(source, cl)) {
                    return Ok(false);
                }
            }
        }
        let a = g(c, "anciennete");
        if vrai(a) {
            let champ = en_texte(cle(a, "champ")?);
            let ans = ans_revolus(g(self.sal, &champ), jour_de_paie(self.paie).as_deref());
            match ans {
                Some(n) if (n as f64) >= cle(a, "ans")?.as_f64().ok_or("nombre attendu")? => {}
                _ => return Ok(false),
            }
        }
        let max = g(c, "duree_contrat_max_jours");
        if !max.is_null() {
            match jours_du_contrat(&self.paie.engagement) {
                Some(j) if (j as f64) <= max.as_f64().ok_or("nombre attendu")? => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Les grilles de cette convention qui visent cette ligne.
    fn grilles_de(&self, ligne: &LigneBrut, cl: &str) -> Resultat<Vec<&'a Value>> {
        let population = if permanent(self.paie) { "permanent" } else { "intermittent" };
        let statut = texte_de(self.m, "statut");
        let est_cachet = cachet(&ligne.code);
        let heures = !ligne.heures_annexes.is_zero();
        let mut out: Vec<&'a Value> = Vec::new();
        for gr in g(cle(self.t, self.idcc)?, cl).as_array().into_iter().flatten() {
            let pop = gr.get("population").map_or("tous".to_string(), en_texte);
            if pop != "tous" && pop != population {
                continue;
            }
            let st = gr.get("statut").map_or("tous".to_string(), en_texte);
            if st != "tous" && st != statut {
                continue;
            }
            let unite = en_texte(cle(gr, "unite")?);
            if est_cachet != (unite == "cachet") {
                continue;
            }
            if !est_cachet {
                if vrai(g(gr, "heures")) {
                    if !(heures && (horaire(&ligne.code) || mensuel(&ligne.code))) {
                        continue;
                    }
                } else if ligne_de_l_unite(&unite) != Some(ligne.code.as_str()) {
                    continue;
                }
            }
            if !self.conditions(gr)? {
                continue;
            }
            out.push(gr);
        }
        // Une grille en remplace une autre (`remplace`) quand elle s'applique.
        let remplacees: BTreeSet<String> =
            out.iter().flat_map(|gr| g(gr, "remplace").as_array().into_iter().flatten().map(en_texte)).collect();
        let mut gardees = Vec::new();
        for gr in out {
            if !remplacees.contains(&en_texte(cle(gr, "nom")?)) {
                gardees.push(gr);
            }
        }
        Ok(gardees)
    }

    /// Les lignes d'une grille, majorations comprises, et ce qui les a majorees.
    fn valeurs(&self, gr: &Value) -> Resultat<(Vec<(String, Decimal)>, Vec<String>)> {
        let mut valeurs = Vec::new();
        for (k, v) in cle(gr, "valeurs")?.as_object().into_iter().flatten() {
            if !k.starts_with('_') {
                valeurs.push((k.clone(), dec(v)?));
            }
        }
        // Dans un meme groupe, seule la derniere majoration qui s'applique compte.
        let mut retenues: Vec<(String, &Value)> = Vec::new();
        for maj in g(gr, "majorations").as_array().into_iter().flatten() {
            if self.conditions(maj)? {
                let groupe = match g(maj, "groupe") {
                    v if vrai(v) => en_texte(v),
                    _ => en_texte(cle(maj, "nom")?),
                };
                match retenues.iter_mut().find(|r| r.0 == groupe) {
                    Some(r) => r.1 = maj,
                    None => retenues.push((groupe, maj)),
                }
            }
        }
        for (_, maj) in &retenues {
            for (k, plus) in cle(maj, "par_cle")?.as_object().into_iter().flatten() {
                if let Some(v) = valeurs.iter_mut().find(|v| &v.0 == k) {
                    v.1 = add(v.1, dec(plus)?);
                }
            }
        }
        let mut noms = Vec::new();
        for (_, maj) in &retenues {
            noms.push(en_texte(cle(maj, "nom")?));
        }
        Ok((valeurs, noms))
    }

    /// Le plancher d'une ligne dans une convention a grilles : le montant du
    /// cachet, un taux horaire (la ligne de grille sur les heures qu'elle
    /// couvre, au centime inferieur), ou le montant d'une unite.
    fn plancher_grilles(&self, ligne: &LigneBrut, cl: &str) -> Resultat<Option<Plancher>> {
        let mut grilles = self.grilles_de(ligne, cl)?;
        if grilles.is_empty() {
            return Ok(None);
        }
        let idcc = self.idcc;
        let classement = {
            let c = texte_de(self.sal, "classement_conventionnel");
            let c = c.trim().to_string();
            if c.is_empty() { texte_de(g(self.m, "classement"), idcc) } else { c }
        };
        // Une grille a l'unite prime : elle prix la ligne telle qu'elle est payee.
        let unitaires: Vec<&Value> = grilles
            .iter()
            .copied()
            .filter(|gr| g(gr, "unite").as_str() != Some("cachet") && !vrai(g(gr, "heures")))
            .collect();
        if !unitaires.is_empty() {
            grilles = unitaires;
        }
        // Un classement connu d'une grille ne se noie pas dans les autres.
        let connues: Vec<&Value> =
            grilles.iter().copied().filter(|gr| !classement.is_empty() && g(gr, "valeurs").get(&classement).is_some()).collect();
        let cherchees = if connues.is_empty() { grilles } else { connues };
        let mut trouves: Vec<Plancher> = Vec::new();
        for gr in cherchees {
            let (valeurs, majorees) = self.valeurs(gr)?;
            if valeurs.is_empty() {
                continue;
            }
            let (k, mut dit) = if valeurs.iter().any(|v| v.0 == classement) {
                (classement.clone(), classement.clone())
            } else {
                let k = plus_bas(valeurs.iter().collect(), |v| v.1).unwrap().0.clone();
                let dit = if classement.is_empty() {
                    format!("{}, le plus bas de la grille (la fiche ne dit pas le classement, `classement_conventionnel`)", k)
                } else {
                    format!("{}, le plus bas de la grille ({} n'y est pas)", k, repr_texte(&classement))
                };
                (k, dit)
            };
            if !majorees.is_empty() {
                dit += &format!(" ; {}", majorees.join(", "));
            }
            let mut montant = valeurs.iter().find(|v| v.0 == k).unwrap().1;
            if vrai(g(gr, "abattement_max")) {
                let ab = dec(cle(gr, "abattement_max")?)?;
                montant = bas(div(mul(montant, sub(Decimal::ONE_HUNDRED, ab)), Decimal::ONE_HUNDRED));
                let ab_txt = en_texte(cle(gr, "abattement_max")?).replace('.', ",");
                let pourquoi = match g(gr, "_abattement") {
                    v if v.is_string() => en_texte(v),
                    _ => "ensemble".into(),
                };
                dit += &format!(" ; abattu de {} % au plus ({})", ab_txt, pourquoi);
            }
            let nom = texte_de(gr, "nom");
            let unite = en_texte(cle(gr, "unite")?);
            if unite == "cachet" {
                trouves.push(Plancher {
                    montant,
                    quoi: format!("CCN {}, {}, {} : {} EUR le cachet", idcc, nom, dit, euros(montant)),
                    unite: false,
                });
            } else if !vrai(g(gr, "heures")) {
                let par = match unite.as_str() {
                    "mois" => "par mois",
                    "semaine" => "la semaine",
                    "jour" => "la journee",
                    autre => return Err(format!("unité inconnue : {}", repr_texte(autre))),
                };
                trouves.push(Plancher {
                    montant,
                    quoi: format!("CCN {}, {}, {} : {} EUR {}", idcc, nom, dit, euros(montant), par),
                    unite: true,
                });
            } else {
                let heures = dec(cle(gr, "heures")?)?;
                let mut h = bas(div(montant, heures));
                if vrai(g(gr, "facteur")) {
                    // Un taux horaire qui ne vaut pas pour toutes les heures : le plus bas taux effectif.
                    h = bas(mul(h, dec(cle(gr, "facteur")?)?));
                    dit += &format!(
                        " ; {}",
                        match g(gr, "_facteur") {
                            v if vrai(v) => en_texte(v),
                            _ => "taux effectif le plus bas".into(),
                        }
                    );
                }
                let par = match unite.as_str() {
                    "mois" => "par mois",
                    "semaine" => "la semaine",
                    "jour" => "la journee",
                    "heure" => "de l'heure",
                    autre => return Err(format!("unité inconnue : {}", repr_texte(autre))),
                };
                trouves.push(Plancher {
                    montant: h,
                    quoi: format!("CCN {}, {}, {} : {} EUR {}, pour {} h, ramene a l'heure", idcc, nom, dit, euros(montant), par, heures_texte(heures)),
                    unite: false,
                });
            }
        }
        Ok(plus_bas(trouves, |p| p.montant))
    }

    /// L'echelon de la grille administrative de la 3090 : 1 sous 5 ans dans la
    /// fonction, puis un de plus par tranche de 5 ans passee, jusqu'au 5e.
    fn echelon_3090(&self) -> Resultat<(usize, String)> {
        let depuis: String = texte_de(self.sal, "poste_depuis").chars().take(10).collect();
        let jour = jour_de_paie(self.paie);
        let Some(jour) = jour.filter(|_| !depuis.is_empty()) else {
            return Ok((1, "echelon 1, la fiche ne disant pas depuis quand (`poste_depuis`)".into()));
        };
        let veille = iso(&jour).ok_or("date illisible")?.plus_jours(-1)?.iso();
        let Some(ans) = ans_revolus(&Value::String(depuis.clone()), Some(&veille)) else {
            return Ok((1, "echelon 1, `poste_depuis` illisible".into()));
        };
        let e = (ans.div_euclid(5) + 1).clamp(1, 5) as usize;
        Ok((e, format!("echelon {}, dans la fonction depuis le {}", e, Jour::lire(&depuis)?.jj_mm_aaaa())))
    }

    /// Le taux horaire minimum d'un technicien, par son niveau 3090 ou son
    /// groupe CCNEAC. `mensuel` : la ligne est un salaire mensuel de permanent.
    fn plancher_horaire(&self, mensuel: bool) -> Resultat<Option<Plancher>> {
        if self.idcc == "1285" {
            return self.plancher_horaire_1285();
        }
        if self.idcc != "3090" {
            return Ok(None);
        }
        let classement = g(self.m, "classification_3090");
        let niveau = g(classement, "niveau");
        if !vrai(niveau) {
            return Ok(None);
        }
        let niveau = en_texte(niveau);
        let t3090 = cle(self.t, "3090")?;
        let communes_cl = cle(t3090, "clauses_communes")?;
        if g(classement, "grille").as_str() == Some("administratif") {
            // Grille mensuelle pour un temps plein, ramenee a l'heure sur 151,67 h.
            let grille = cle(communes_cl, "administratifs_mensuel_echelons")?;
            let Some(valeurs) = grille.get(&niveau) else { return Ok(None) };
            let valeurs = valeurs.as_array().ok_or("liste attendue")?;
            let (echelon, mut d_ou) = self.echelon_3090()?;
            if valeurs.is_empty() {
                return Err(socle::fichiers::trop_courte(&format!("les échelons du niveau {}", niveau)));
            }
            let montant = valeurs.get(echelon.min(valeurs.len()) - 1).ok_or_else(|| socle::fichiers::trop_courte(&format!("l'échelon {} du niveau {}", echelon, niveau)))?.as_f64().ok_or("nombre attendu")?;
            if valeurs.len() == 1 {
                d_ou = "sans echelon".into();
            }
            let horaire = arrondi2(montant / 151.67);
            return Ok(plancher(
                dec_f64(horaire)?,
                format!(
                    "CCN 3090, grille administrative, {}, {} : {} EUR par mois ramenes a l'heure sur 151,67 h",
                    niveau.replace('_', " "),
                    d_ou,
                    euros_f(montant)
                ),
            ));
        }
        let communes = cle(communes_cl, "techniciens")?;
        let (mut grille, mut ou) = (communes, "clauses communes".to_string());
        if let Some(a) = self.annexe.as_deref().filter(|a| cle(t3090, "annexes").map_or(false, |x| x.get(*a).is_some())) {
            grille = cle(cle(cle(t3090, "annexes")?, a)?, "techniciens")?;
            ou = format!("annexe {}", a);
            // Un salaire mensuel : le plus bas de la grille des clauses communes et de celle de l'annexe.
            if mensuel {
                if let Some(c) = communes.get(&niveau) {
                    let en_dessous = match grille.get(&niveau) {
                        None => true,
                        Some(x) => c.as_f64() < x.as_f64(),
                    };
                    if en_dessous {
                        grille = communes;
                        ou = "clauses communes".into();
                    }
                }
            }
        }
        let Some(montant) = grille.get(&niveau) else { return Ok(None) };
        Ok(plancher(
            dec(montant)?,
            format!(
                "CCN 3090, {}, taux horaire {}{}",
                ou,
                niveau.replace('_', " "),
                if mensuel { " (le mensuel pour 151,67 h, ramene a l'heure)" } else { "" }
            ),
        ))
    }

    /// Le plancher d'une ligne, ou rien.
    fn plancher_de(&self, ligne: &LigneBrut) -> Resultat<Option<Plancher>> {
        let statut = texte_de(self.m, "statut");
        let heures = !ligne.heures_annexes.is_zero();
        let code = ligne.code.as_str();
        if self.idcc == "2642" {
            if statut == "technicien" && (horaire(code) || mensuel(code)) && heures {
                if let Some(p) = self.plancher_2642()? {
                    return Ok(Some(p));
                }
            }
            return self.plancher_grilles(ligne, "grilles_complementaires");
        }
        if self.idcc == "3097" {
            if let Some(p) = self.plancher_3097(ligne)? {
                return Ok(Some(p));
            }
            return self.plancher_grilles(ligne, "grilles_complementaires");
        }
        if self.a_grilles() {
            return self.plancher_grilles(ligne, "grilles");
        }
        if cachet(code) && statut == "artiste" {
            return if self.idcc == "3090" { self.plancher_cachet_3090() } else { self.plancher_cachet_1285() };
        }
        if repetition(code) && statut == "artiste" {
            return self.plancher_repetition();
        }
        if horaire(code) && statut == "technicien" {
            return self.plancher_horaire(false);
        }
        // Le salaire mensuel d'un permanent de la 3090 : sa grille est mensuelle, pour 151,67 h.
        if mensuel(code) && statut == "technicien" && self.idcc == "3090" && heures {
            return self.plancher_horaire(true);
        }
        // Les mensuels des artistes de la CCNEAC : par grille, ramenes a l'heure.
        if mensuel(code) && statut == "artiste" && self.idcc == "1285" && heures {
            return self.plancher_grilles(ligne, "grilles_complementaires");
        }
        // Un salaire mensuel de la CCNEAC : le minimum de l'echelon pour 151 h 40, ramene a l'heure.
        if mensuel(code) && statut == "technicien" && self.idcc == "1285" && heures {
            return self.plancher_horaire_1285();
        }
        Ok(None)
    }

    /// Le minimum d'une ligne « Service de repetition » dans la 3090 :
    /// (montant, d'ou il vient, a l'heure), ou rien.
    fn plancher_service_3090(&self, par_jour: Option<Decimal>) -> Resultat<Option<(Decimal, String, bool)>> {
        let t3090 = cle(self.t, "3090")?;
        let refs = self.refs();
        // (montant, annexe, a l'heure, libelle)
        let mut trouves: Vec<(Decimal, String, bool, &str)> = Vec::new();
        for annexe in self.annexes_ou_la_sienne()? {
            let r = g(cle(cle(t3090, "annexes")?, &annexe)?, "repetition");
            let sorte = texte_de(refs, &annexe);
            let sorte = sorte.split('.').next().unwrap_or("");
            let musicien = sorte == "musicien" || sorte == "chef_orchestre";
            if musicien && r.get("service_musicien").is_some() {
                trouves.push((dec(&r["service_musicien"])?, annexe, false, "service de repetition d'un musicien"));
            } else if !musicien && r.get("heure").is_some() {
                trouves.push((dec(&r["heure"])?, annexe, true, "services de repetition (taux horaire)"));
            } else if r.get("service_isole").is_some() && par_jour.map_or(false, |p| p <= Decimal::ONE) {
                trouves.push((dec(&r["service_isole"])?, annexe, false, "service isole de repetition"));
            } else {
                return Ok(None); // une annexe candidate sans minimum : on se tait
            }
        }
        if trouves.is_empty() || trouves.iter().any(|x| x.2 != trouves[0].2) {
            return Ok(None);
        }
        let (montant, annexe, heure, lib) = plus_bas(trouves, |x| x.0).unwrap();
        Ok(Some((montant, format!("CCN 3090, annexe {}, {}", annexe, lib), heure)))
    }
}

fn ligne_de_l_unite(unite: &str) -> Option<&'static str> {
    match unite {
        "jour" => Some("journee"),
        "semaine" => Some("semaine"),
        "mois" => Some("mois"),
        _ => None,
    }
}

/// Une condition de grille (`si_*`) : la valeur lue est-elle celle qu'on attend ?
fn vaut(attendu: &Value, lu: &Value) -> bool {
    let texte = |v: &Value| if v.is_null() { String::new() } else { en_texte(v) };
    match attendu {
        Value::Bool(b) => vrai(lu) == *b,
        Value::Array(l) => l.iter().any(|x| en_texte(x) == texte(lu)),
        autre => texte(lu) == en_texte(autre),
    }
}

// ------------------------------------------------------------------ les deux lectures

/// Une lecture, pas un verdict : la phrase renvoie au texte pour qu'on verifie.
fn a_verifier(t: &Value, idcc: &str, phrase: String) -> Avis {
    let lien = g(g(t, idcc), "lien");
    let lien = if vrai(lien) { format!(" : {}", en_texte(lien)) } else { String::new() };
    Avis {
        info: true,
        texte: format!(
            "{} Nous avons pu passer a cote d'un cas qui change ce minimum : verifiez-le sur le texte de la convention{}.",
            phrase, lien
        ),
    }
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

/// Le contexte d'un plancher pour cette paie (la convention du contrat, le metier de la fiche), passe a `f` ; rien
/// quand la table ne connait pas la convention ou que la fiche ne dit pas de metier connu.
fn avec_cx<T>(ctx: &Contexte, paie: &Paie, f: impl FnOnce(&Cx) -> Resultat<Option<T>>) -> Resultat<Option<T>> {
    let ent = &ctx.entreprise;
    let idcc = idcc_du_contrat(ent, paie)?;
    let tab = table()?;
    let t: &Value = &tab;
    if t.get(&idcc).is_none() {
        return Ok(None);
    }
    let sal = &paie.salarie;
    let Some(m) = emplois::metier(&texte_de(sal, "code_emploi_ft"))? else { return Ok(None) };
    let nb_cachets = paie
        .remuneration
        .iter()
        .filter(|l| cachet(&l.code))
        .fold(Decimal::ZERO, |a, l| add(a, l.quantite))
        .trunc()
        .to_string()
        .parse::<i64>()
        .map_err(|e| e.to_string())?;
    let cx = Cx {
        t,
        idcc: &idcc,
        m: &m,
        sal,
        paie,
        ent,
        annexe: ou_rien(g(ent, "annexe_convention")),
        organisation: ou_rien(g(ent, "organisation_patronale")),
        nb_cachets,
    };
    f(&cx)
}

/// Le minimum d'une ligne de remuneration (`saisie` : la ligne telle que saisie, pour ses dates), ou rien.
fn minimum_dans(cx: &Cx, l: &LigneBrut, saisie: &Value) -> Resultat<Option<Minimum>> {
    let statut = texte_de(cx.m, "statut");
    // Le service de repetition de la 3090 : un minimum par service ou par heure selon
    // l'annexe, jamais le cachet de la journee.
    if repetition(&l.code) && cx.idcc == "3090" && statut == "artiste" {
        let dates: BTreeSet<String> = g(saisie, "dates").as_array().into_iter().flatten().map(en_texte).collect();
        let par_jour = (!dates.is_empty()).then(|| div(l.quantite, Decimal::from(dates.len())));
        return Ok(cx.plancher_service_3090(par_jour)?.map(|(montant, quoi, heure)| Minimum {
            montant,
            quoi,
            par: if heure { Par::Heure } else { Par::Service },
            service_3090: true,
        }));
    }
    let Some(p) = cx.plancher_de(l)? else { return Ok(None) };
    let par = if p.unite {
        Par::Unite
    } else if horaire(&l.code) || mensuel(&l.code) {
        Par::Heure
    } else if cachet(&l.code) {
        Par::Cachet
    } else {
        Par::Service
    };
    Ok(Some(Minimum { montant: p.montant, quoi: p.quoi, par, service_3090: false }))
}

/// Les lignes saisies de cette paie, une par ligne de remuneration ; `Null` quand elles ne se correspondent pas.
pub fn saisies_de(paie: &Paie) -> Vec<&Value> {
    let lignes = &paie.remuneration;
    if paie.engagement.lignes.len() == lignes.len() {
        paie.engagement.lignes.iter().collect()
    } else {
        vec![&NUL; lignes.len()]
    }
}

/// Le minimum conventionnel de la ligne `i` de la remuneration de cette paie, tel que `avertissements` le compare
/// (les conseils de saisie s'en servent pour ne jamais proposer moins que lui ; `conseils::de_saisie`).
pub fn minimum_de_la_ligne(ctx: &Contexte, paie: &Paie, i: usize) -> Resultat<Option<Minimum>> {
    let Some(l) = paie.remuneration.get(i) else { return Ok(None) };
    let saisie = saisies_de(paie)[i];
    avec_cx(ctx, paie, |cx| minimum_dans(cx, l, saisie))
}

/// LE CACHET DE REPETITION de la convention, quand elle en a un pour ce metier : la 3090 fixe un « cachet de
/// repetition pour 1 ou 2 services de repetition dans la meme journee » (annexe 1, 84,00 EUR), un « cachet de base des
/// journees de repetition » (annexe 2, 106,20 EUR), un cachet de repetition « pour un ou deux services de repetitions de
/// quatre heures dans la meme journee » (annexe 4, 94,34 EUR), un « cachet de base par jour » de repetition (annexe 5,
/// 108,03 EUR) -- avenant du 21/07/2025, `atelier/reference/sources/legifrance-3090-avenant-salaires-20250721.md`, lus
/// dans `minima-conventionnels.json` (`annexes.<a>.repetition.cachet`). Rien pour un musicien de l'annexe 1 (« Sauf
/// dispositions particulieres prevues pour les artistes musiciens » : son cachet est un service de 3 h 30,
/// `chomage-intermittents/kali-3090-repetitions-cachets-representations-20261020.md`, KALIARTI000028157549) ; rien
/// pour une autre convention (la CCNEAC paie la repetition par service : `kali-1285-x3-remuneration-artistes-20260929.md`).
/// L'annexe de l'entreprise inconnue : le plus HAUT des cachets possibles, pour qu'un conseil ne propose jamais moins
/// que le minimum de la bonne annexe.
pub fn cachet_de_repetition(ctx: &Contexte, paie: &Paie) -> Resultat<Option<Minimum>> {
    avec_cx(ctx, paie, |cx| {
        if cx.idcc != "3090" || texte_de(cx.m, "statut") != "artiste" {
            return Ok(None);
        }
        let annexes = cle(cle(cx.t, "3090")?, "annexes")?;
        let refs = cx.refs();
        let mut trouves: Vec<(Decimal, String, String)> = Vec::new();
        for annexe in cx.annexes_ou_la_sienne()? {
            let sorte = texte_de(refs, &annexe);
            let sorte = sorte.split('.').next().unwrap_or("");
            if annexe == "1" && (sorte == "musicien" || sorte == "chef_orchestre") {
                return Ok(None);
            }
            let r = g(cle(annexes, &annexe)?, "repetition");
            let Some(c) = r.get("cachet") else { return Ok(None) };
            trouves.push((dec(c)?, annexe, texte_de(r, "cachet_libelle")));
        }
        Ok(trouves.into_iter().max_by(|a, b| a.0.cmp(&b.0)).map(|(montant, annexe, libelle)| Minimum {
            montant,
            quoi: format!("CCN 3090, annexe {}, {}", annexe, libelle),
            par: Par::Cachet,
            service_3090: false,
        }))
    })
}

/// Les lignes de cette paie payees sous un minimum conventionnel : une phrase
/// par ligne fautive, qui dit le paye, le minimum, et la grille lue.
pub fn avertissements(ctx: &Contexte, paie: &Paie) -> Resultat<Vec<Avis>> {
    let avis = avec_cx(ctx, paie, |cx| {
        let t = cx.t;
        let idcc = cx.idcc;
        let qui = qui(cx.sal);
        let texte_source = premiere_partie(cle(t, idcc)?)?;
        let mut avis = Vec::new();
        for (l, saisie) in paie.remuneration.iter().zip(saisies_de(paie)) {
            let Some(p) = minimum_dans(cx, l, saisie)? else { continue };
            if p.service_3090 {
                let (paye, unite) = if p.par == Par::Heure {
                    if l.heures_annexes.is_zero() {
                        continue;
                    }
                    (pair(div(l.montant, l.heures_annexes)), "l'heure")
                } else {
                    (l.taux, "le service")
                };
                if paye < p.montant {
                    avis.push(a_verifier(
                        t,
                        idcc,
                        format!(
                            "{}, {} : {} \u{20ac} {}. Notre lecture de la convention donne un minimum conventionnel de {} \u{20ac} {} ({} ; {}).",
                            qui,
                            l.designation(),
                            euros(paye),
                            unite,
                            euros(p.montant),
                            unite,
                            p.quoi,
                            texte_source
                        ),
                    ));
                }
                continue;
            }
            let (paye, unite) = match p.par {
                // Une grille a l'unite : le taux de la ligne contre le montant de la grille.
                Par::Unite => (
                    l.taux,
                    match l.code.as_str() {
                        "journee" => "la journee",
                        "semaine" => "la semaine",
                        "mois" => "le mois",
                        _ => "l'unite",
                    },
                ),
                Par::Heure => {
                    if l.heures_annexes.is_zero() {
                        continue;
                    }
                    (pair(div(l.montant, l.heures_annexes)), "l'heure")
                }
                Par::Cachet => (l.taux, "le cachet"),
                Par::Service => (l.taux, "le service"),
            };
            if paye < p.montant {
                avis.push(a_verifier(
                    t,
                    idcc,
                    format!(
                        "{}, {} : {} \u{20ac} {}. Notre lecture de la convention donne un minimum conventionnel de {} \u{20ac} ({} ; {}).",
                        qui,
                        l.designation(),
                        euros(paye),
                        unite,
                        euros(p.montant),
                        p.quoi,
                        texte_source
                    ),
                ));
            }
        }
        Ok(Some(avis))
    })?;
    Ok(avis.unwrap_or_default())
}

/// Les lignes de cette paie payees sous le SMIC horaire. Le SMIC est la loi :
/// aucune convention, aucun statut, aucun catalogue de metiers n'entre ici.
/// Les lignes visees portent des heures reellement faites (pas un forfait) ;
/// la comparaison porte sur le total, pas sur un taux reconstitue.
pub fn sous_le_smic(ctx: &Contexte, paie: &Paie) -> Resultat<Vec<Avis>> {
    let qui = qui(&paie.salarie);
    let jour = paie.engagement.fin_periode()?;
    let smic = dec_f64(ctx.smic(&jour)?)?;
    let mut avis = Vec::new();
    for l in &paie.remuneration {
        let heures = l.heures_annexes;
        if heures <= Decimal::ZERO {
            continue;
        }
        if rubrique(&ctx.racine, &l.code)?.heures_forfait.is_some() {
            continue;
        }
        let du = pair(mul(smic, heures));
        let paye = l.montant;
        if paye >= du {
            continue;
        }
        avis.push(Avis {
            info: false,
            texte: format!(
                "{}, {} : {} \u{20ac} pour {} h. Le SMIC est de {} \u{20ac} de l'heure au {} : il fallait {} \u{20ac}, il manque {} \u{20ac}. A corriger avant de declarer (le SMIC vaut pour tous, art. L. 3231-2).",
                qui,
                l.designation(),
                euros(paye),
                heures_texte(heures),
                euros(smic),
                jour.jj_mm_aaaa(),
                euros(du),
                euros(sub(du, paye))
            ),
        });
    }
    Ok(avis)
}
