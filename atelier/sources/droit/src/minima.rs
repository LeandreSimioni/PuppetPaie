//! Les minima conventionnels : la table livree (`minima-conventionnels.json`, une grille par convention), ses paliers,
//! la grille NAO non etendue de la CCNEAC et qui elle oblige, et ce qu'un minimum est. Les confronter a une paie, et
//! l'avertir, est l'affaire de l'engagement (`engagement::minima`).

use std::sync::Arc;

use serde_json::Value;

use socle::date::{en_texte, Jour};
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use socle::num::{dec, Decimal, Resultat};
use socle::texte::sigle;

static NUL: Value = Value::Null;

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

fn cle<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// La table des minima conventionnels livree.
pub fn table() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("minima-conventionnels.json"))
}

/// L'IDCC d'un contrat : celui de la compagnie (`ent`), sauf l'artiste-interprète d'une émission de télévision
/// (`emission_tv_1734` du spectacle `sp`) d'une compagnie de la 2642 ou de la 3241, qui relève de la 1734
/// (`minima-conventionnels.json`, `_regles_1734`). Le contrat le déclare (S21.G00.40.017), le bulletin l'imprime, le
/// minimum lit sa grille. Artiste-interprète se lit au métier du catalogue (familles « Artistes », « Cirque et rue ») :
/// un métier hors catalogue ne dit pas s'il l'est, l'IDCC se refuse (règle 1).
pub fn idcc_du_contrat(ent: &Value, sp: &Value, sal: &Value) -> Resultat<String> {
    let texte = |v: &Value, k: &str, defaut: &str| match g(v, k) {
        x if vrai(x) => en_texte(x),
        _ => defaut.to_string(),
    };
    let idcc = texte(ent, "idcc", "");
    if (idcc != "2642" && idcc != "3241") || !vrai(g(sp, "emission_tv_1734")) || texte(sal, "statut", "artiste") != "artiste" {
        return Ok(idcc);
    }
    let code = Some(g(sal, "code_emploi_ft")).filter(|c| vrai(c)).map(en_texte);
    let metier = match &code {
        Some(c) => crate::emplois::metier(c)?,
        None => None,
    };
    match metier.as_ref().map(|m| g(m, "famille")).filter(|f| !f.is_null()) {
        None => Err(format!(
            "Spectacle {}, émission de télévision (convention 1734) : le métier {} de {} n'est pas au catalogue, et \
             le logiciel ne sait pas s'il est artiste-interprète (1734) ou musicien, metteur en scène, artiste de \
             complément (convention de l'entreprise). Choisir un métier du catalogue.",
            texte(sp, "titre", ""),
            code.unwrap_or_else(|| "(aucun)".into()),
            texte(sal, "nom", "ce salarié")
        )),
        Some(f) if matches!(f.as_str(), Some("Artistes") | Some("Cirque et rue")) => Ok("1734".into()),
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
pub fn ans_revolus(depuis: &Value, jour: Option<&str>) -> Option<i64> {
    let a = iso(&en_texte(depuis))?;
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
pub fn palier(paliers: &Value, nombre: i64) -> Resultat<Option<Decimal>> {
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

/// Une condition de grille (`si_*`) : la valeur lue est-elle celle qu'on attend ?
pub fn vaut(attendu: &Value, lu: &Value) -> bool {
    let texte = |v: &Value| if v.is_null() { String::new() } else { en_texte(v) };
    match attendu {
        Value::Bool(b) => vrai(lu) == *b,
        Value::Array(l) => l.iter().any(|x| en_texte(x) == texte(lu)),
        autre => texte(lu) == en_texte(autre),
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

