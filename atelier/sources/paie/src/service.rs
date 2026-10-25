//! `paie route` : la demande JSON sur l'entree standard, la reponse en une ligne de JSON sur la sortie standard ; un
//! refus : `{"ok": false, "erreur": "..."}`. Et le calcul d'une saisie, `{"fichier": CHEMIN}` ou `{"saisie": {...}}`
//! (`calculer_avec`) : `{"ok": true, "mois", "paies": [...]}`, une paie par engagement de la saisie et dans son ordre ;
//! les avertissements gardent leur nature (`{"texte", "info"}`).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use engagement::{charger, charger_donnees, Saisie};
use socle::fichiers::dossier_du_code;
use socle::num::Resultat;

pub(crate) fn options(args: &[String]) -> Option<HashMap<String, String>> {
    let mut o = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let cle = args[i].strip_prefix("--")?;
        if !["racine", "date"].contains(&cle) {
            return None;
        }
        o.insert(cle.to_string(), args.get(i + 1)?.clone());
        i += 2;
    }
    Some(o)
}

fn saisie(ctx: &Contexte, demande: &Value) -> Resultat<Saisie> {
    match (demande.get("fichier").and_then(Value::as_str), demande.get("saisie")) {
        (Some(f), _) => charger(ctx, Path::new(f)),
        (None, Some(s)) if s.is_object() => charger_donnees(ctx, s),
        _ => Err("demande illisible : il faut `fichier` ou `saisie`".to_string()),
    }
}

/// La compagnie d'une commande : `--racine` (`o["racine"]`, toujours pose par la fenetre et la file des etats), sinon,
/// a la ligne de commande seulement, `PUPPETPAIE_DONNEES` (`paie route` lance a la main ou par un test).
pub(crate) fn racine_de(o: &HashMap<String, String>) -> PathBuf {
    match (o.get("racine"), std::env::var("PUPPETPAIE_DONNEES")) {
        (Some(r), _) if !r.is_empty() => PathBuf::from(r),
        (_, Ok(e)) if !e.is_empty() => PathBuf::from(e),
        _ => dossier_du_code(),
    }
}

pub(crate) fn contexte(o: &HashMap<String, String>) -> Resultat<Contexte> {
    let date = o.get("date").map(|d| Jour::lire(d)).transpose()?;
    Contexte::nouveau(&racine_de(o), date)
}

pub(crate) fn calculer_avec(ctx: &Contexte, demande: &Value) -> Resultat<Value> {
    let s = saisie(ctx, demande)?;
    socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
    let paies = paies_fideles(ctx, &s);
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("mois".into(), Value::String(s.mois.clone()));
    m.insert("paies".into(), Value::Array(paies?));
    Ok(Value::Object(m))
}

fn paies_fideles(ctx: &Contexte, s: &Saisie) -> Resultat<Vec<Value>> {
    let mut paies = Vec::new();
    for p in &s.paies {
        let mut v = p.canon(ctx)?;
        v["avertissements"] = avis(&p.avertissements);
        // Les lignes telles quelles (celles d'un permanent, deduites) : le
        // Python reconstruit l'engagement avec elles.
        v["engagement"]["lignes"] = Value::Array(p.engagement.lignes.clone());
        paies.push(v);
    }
    Ok(paies)
}

fn demande(entree: &str) -> Resultat<Value> {
    serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))
}

fn texte<'a>(d: &'a Value, cle: &str) -> Resultat<&'a str> {
    d.get(cle).and_then(Value::as_str).ok_or_else(|| format!("demande illisible : il faut `{}`", cle))
}

fn jour(j: Option<Jour>) -> Value {
    j.map_or(Value::Null, |j| Value::String(j.iso()))
}

fn avis(v: &[engagement::Avis]) -> Value {
    Value::Array(v.iter().map(|a| {
        let mut m = Map::new();
        m.insert("texte".into(), Value::String(a.texte.clone()));
        m.insert("info".into(), Value::Bool(a.info));
        Value::Object(m)
    }).collect())
}

/// `droits` : la position de chaque salarie au regard des annexes 8 et 10,
/// `{"mois", "aujourdhui"}` -> `{"constantes", "positions": [{"cle",
/// "anniversaire", "position", "colonnes"}]}`, les salaries dans l'ordre.
pub(crate) fn droits(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    use engagement::cumul::{anniversaire_configure, lire_engagements, position_droits, tableau_cumul, Constantes};
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let (mois, aujourdhui) = (texte(&d, "mois")?, Jour::lire(texte(&d, "aujourdhui")?)?);
    let c = Constantes::lire(&ctx.racine)?;
    let engs = lire_engagements(&ctx, None)?;
    let cles: std::collections::BTreeSet<&str> = engs.iter().map(|e| e.salarie.as_str()).collect();
    let mut positions = Vec::new();
    for cle in cles {
        let anniv = anniversaire_configure(&ctx, cle)?;
        let pos = position_droits(&ctx, cle, Some(&engs), anniv, None)?;
        let colonnes = tableau_cumul(&ctx, cle, mois, Some(&engs))?;
        let mut m = Map::new();
        m.insert("cle".into(), Value::String(cle.to_string()));
        m.insert("anniversaire".into(), jour(anniv));
        m.insert("position".into(), pos.canon(aujourdhui)?);
        m.insert("colonnes".into(), Value::Array(colonnes.iter().map(|x| x.canon()).collect()));
        positions.push(Value::Object(m));
    }
    let mut k = Map::new();
    k.insert("heures_par_cachet".into(), c.heures_par_cachet.into());
    k.insert("cachets_par_mois".into(), c.cachets_par_mois.into());
    k.insert("seuil_heures".into(), c.seuil_heures.into());
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("constantes".into(), Value::Object(k));
    m.insert("positions".into(), Value::Array(positions));
    Ok(Value::Object(m))
}

/// Le compteur d'un permanent a la fin du mois et l'indemnite de son solde.
pub(crate) struct Provision {
    pub report_restant: socle::num::Decimal,
    pub acquis: socle::num::Decimal,
    pub pris: socle::num::Decimal,
    pub solde: socle::num::Decimal,
    pub avertissements: Vec<engagement::Avis>,
    pub indemnite: Option<engagement::conges::IndemniteDesJours>,
}

/// Le calcul de `provision-conges`, pour un contexte deja lu : le compteur d'un permanent a la fin de `mois`, et
/// l'indemnite de son solde (le dixieme ou le maintien, L3141-24) quand il est positif. `salaire_mensuel` : la
/// valeur de l'engagement (`None` ou fausse = 0).
pub(crate) fn provision_conges_de(ctx: &Contexte, salarie: &str, mois: &str, salaire_mensuel: Option<&Value>) -> Resultat<Provision> {
    use engagement::conges::{compteur_conges, indemnite_des_jours, jours_ouvrables};
    use socle::num::{dec, div, eur, mul, Decimal};
    let fin = socle::date::dernier_du_mois(mois)?;
    let cpt = compteur_conges(ctx, salarie, &fin, Some(&fin.mois_suivant()))?;
    let solde = cpt.solde();
    let salaire = match salaire_mensuel {
        Some(v) if socle::fichiers::vrai(v) => dec(v)?,
        _ => Decimal::ZERO,
    };
    let ouvrables = match jours_ouvrables(&socle::date::premier_du_mois(mois)?, &fin, None)? {
        0 => Decimal::ONE,
        n => Decimal::from(n),
    };
    let indemnite = if solde > Decimal::ZERO {
        Some(indemnite_des_jours(ctx, &cpt, solde, eur(mul(div(salaire, ouvrables), solde)))?)
    } else {
        None
    };
    Ok(Provision { report_restant: cpt.report_restant(), acquis: cpt.acquis, pris: cpt.pris, solde, avertissements: cpt.avertissements.clone(), indemnite })
}

/// `paie route [--racine R] [--date D]` : la demande sur l'entree standard. `None` : usage.
pub fn commande(quoi: &str, args: &[String]) -> Option<i32> {
    if quoi != "route" {
        return None;
    }
    let o = options(args)?;
    let mut entree = String::new();
    let corps = match std::io::stdin().read_to_string(&mut entree) {
        Ok(_) => crate::routes::commande(&o, entree.trim_start_matches('\u{feff}')),
        Err(e) => Err(format!("lecture de l'entree standard impossible : {}", e)),
    };
    let corps = corps.unwrap_or_else(|e| {
        let mut m = Map::new();
        m.insert("ok".into(), Value::Bool(false));
        m.insert("erreur".into(), Value::String(e));
        Value::Object(m)
    });
    println!("{}", corps);
    Some(0)
}
