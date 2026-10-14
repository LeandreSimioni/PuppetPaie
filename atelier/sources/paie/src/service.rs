//! `paie route` et `paie interne NOM` : la demande JSON sur l'entree standard, la reponse en une ligne de JSON sur la
//! sortie standard ; un refus : `{"ok": false, "erreur": "..."}`. Et le calcul d'une saisie, `{"fichier": CHEMIN}` ou
//! `{"saisie": {...}}` (`calculer_avec`) : `{"ok": true, "mois", "paies": [...]}`, une paie par engagement de la saisie
//! et dans son ordre, sous la forme de l'oracle ; les avertissements gardent leur nature (`{"texte", "info"}`).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::contexte::Contexte;
use crate::date::Jour;
use crate::engagement::{charger, charger_donnees, Saisie};
use crate::fichiers::dossier_du_code;
use crate::num::Resultat;

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
    crate::engagement::FIDELE.with(|f| f.set(true));
    let paies = paies_fideles(ctx, &s);
    crate::engagement::FIDELE.with(|f| f.set(false));
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

/// `bulletin` : la description du bulletin (et son PDF, si `sortie`) pour des
/// paies choisies dans des saisies chargees une fois. Demande : `saisies`
/// (chacune `{"fichier"|"saisie", "numeros": [[bulletin, contrat], ...]}`, une
/// paire par paie), `jeux` (`{"saisie", "index", "rectificatif"}`, dans l'ordre
/// d'impression), `mois` (null : pas de cumul), `paies_du_mois` (indice de
/// saisie), `borne_au_mois`, `ent`, `sortie` (null : description seule).
fn bulletin(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let demande: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let ctx = contexte(o)?;
    let liste = demande.get("saisies").and_then(Value::as_array).ok_or("demande illisible : il faut `saisies`")?;
    let mut saisies = Vec::new();
    for (k, d) in liste.iter().enumerate() {
        let mut s = saisie(&ctx, d)?;
        let numeros = d.get("numeros").and_then(Value::as_array).ok_or("demande illisible : il faut `numeros`")?;
        if numeros.len() != s.paies.len() {
            return Err(format!(
                "demande illisible : la saisie {} a {} paies, `numeros` en donne {}",
                k, s.paies.len(), numeros.len()
            ));
        }
        for (p, n) in s.paies.iter_mut().zip(numeros) {
            let texte = |i: usize| n.get(i).and_then(Value::as_str).map(str::to_string);
            p.numero_bulletin = texte(0);
            p.engagement.numero = texte(1);
        }
        saisies.push(s);
    }
    let mut paies = Vec::new();
    let mut rectificatifs = HashMap::new();
    for (i, j) in demande.get("jeux").and_then(Value::as_array).ok_or("demande illisible : il faut `jeux`")?.iter().enumerate() {
        let entier = |c: &str| j.get(c).and_then(Value::as_u64).map(|n| n as usize);
        let p = entier("saisie")
            .and_then(|k| saisies.get(k))
            .and_then(|s| s.paies.get(entier("index")?))
            .ok_or("demande illisible : un jeu designe une paie qui n'existe pas")?;
        paies.push(p.clone());
        if let Some(r) = j.get("rectificatif").filter(|r| !r.is_null()) {
            rectificatifs.insert(i, r.clone());
        }
    }
    let du_mois = match demande.get("paies_du_mois").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(
            v.as_u64()
                .and_then(|k| saisies.get(k as usize))
                .map(|s| s.paies.as_slice())
                .ok_or("demande illisible : `paies_du_mois` designe une saisie qui n'existe pas")?,
        ),
    };
    let mois = demande.get("mois").and_then(Value::as_str).unwrap_or("");
    let borne = demande.get("borne_au_mois").and_then(Value::as_bool).unwrap_or(false);
    let ent = demande.get("ent").cloned().unwrap_or(Value::Null);
    let description = crate::bulletin::jeux_et_description(&ctx, &paies, &ent, mois, du_mois, borne, &rectificatifs)?;
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    match demande.get("sortie").and_then(Value::as_str) {
        None => m.insert("description".into(), description),
        Some(chemin) => {
            crate::fabrique::ecrire(&description, Path::new(chemin))?;
            m.insert("chemin".into(), Value::String(chemin.to_string()))
        }
    };
    Ok(Value::Object(m))
}

/// `bareme-applique` : la pile de barème (`bareme::bareme_applique`) sur le
/// barème que la DSN a choisi, `{"bareme", "salarie", "nature", "fin_periode"}`.
fn bareme_applique(entree: &str) -> Resultat<Value> {
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let nat = crate::contrats::nature(d.get("nature").and_then(Value::as_str))?;
    let fin = d.get("fin_periode").and_then(Value::as_str).map(Jour::lire).transpose()?;
    let bar = d.get("bareme").cloned().ok_or("demande illisible : il faut `bareme`")?;
    let sal = d.get("salarie").cloned().unwrap_or(Value::Null);
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("bareme".into(), crate::bareme::bareme_applique(bar, &sal, &nat, fin.as_ref())?);
    Ok(Value::Object(m))
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

fn avis(v: &[crate::engagement::Avis]) -> Value {
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
    use crate::cumul::{anniversaire_configure, lire_engagements, position_droits, tableau_cumul, Constantes};
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
    pub report_restant: crate::num::Decimal,
    pub acquis: crate::num::Decimal,
    pub pris: crate::num::Decimal,
    pub solde: crate::num::Decimal,
    pub avertissements: Vec<crate::engagement::Avis>,
    pub indemnite: Option<crate::conges::IndemniteDesJours>,
}

/// Le calcul de `provision-conges`, pour un contexte deja lu : le compteur d'un permanent a la fin de `mois`, et
/// l'indemnite de son solde (le dixieme ou le maintien, L3141-24) quand il est positif. `salaire_mensuel` : la
/// valeur de l'engagement (`None` ou fausse = 0).
pub(crate) fn provision_conges_de(ctx: &Contexte, salarie: &str, mois: &str, salaire_mensuel: Option<&Value>) -> Resultat<Provision> {
    use crate::conges::{compteur_conges, indemnite_des_jours, jours_ouvrables};
    use crate::num::{dec, div, eur, mul, Decimal};
    let fin = crate::date::dernier_du_mois(mois)?;
    let cpt = compteur_conges(ctx, salarie, &fin, Some(&fin.mois_suivant()))?;
    let solde = cpt.solde();
    let salaire = match salaire_mensuel {
        Some(v) if crate::fichiers::vrai(v) => dec(v)?,
        _ => Decimal::ZERO,
    };
    let ouvrables = match jours_ouvrables(&crate::date::premier_du_mois(mois)?, &fin, None)? {
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

/// `provision-conges` : le compteur d'un permanent a la fin du mois et
/// l'indemnite de son solde (le dixieme ou le maintien, L3141-24),
/// `{"salarie", "mois", "salaire_mensuel"}`.
fn provision_conges(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let mois = texte(&d, "mois")?;
    let p = provision_conges_de(&ctx, texte(&d, "salarie")?, mois, d.get("salaire_mensuel"))?;
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("report_restant".into(), Value::String(p.report_restant.to_string()));
    m.insert("acquis".into(), Value::String(p.acquis.to_string()));
    m.insert("pris".into(), Value::String(p.pris.to_string()));
    m.insert("solde".into(), Value::String(p.solde.to_string()));
    m.insert("avertissements".into(), avis(&p.avertissements));
    let ind = match &p.indemnite {
        Some(i) => {
            let mut x = Map::new();
            x.insert("dixieme".into(), Value::String(i.dixieme.to_string()));
            x.insert("maintien".into(), Value::String(i.maintien.to_string()));
            x.insert("retenu".into(), Value::String(i.retenu.to_string()));
            x.insert("regle".into(), Value::String(i.regle.to_string()));
            Value::Object(x)
        }
        None => Value::Null,
    };
    m.insert("indemnite".into(), ind);
    Ok(Value::Object(m))
}

/// `preavis` : la duree du preavis d'un engagement (la loi, la convention, la
/// plus longue), `{"engagement"}` ; l'anciennete court du debut a la fin.
fn preavis(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let e = d.get("engagement").filter(|e| e.is_object()).ok_or("demande illisible : il faut `engagement`")?;
    let (debut, fin) = (crate::date::d(&e["debut"])?, crate::date::d(&e["fin"])?);
    let motif = e.get("motif_rupture").and_then(Value::as_str);
    let p = crate::solde::duree_du_preavis(&ctx, e, crate::permanents::anciennete_mois(&debut, &fin), motif)?;
    let nombre = |x: Option<crate::num::Decimal>| x.map_or(Value::Null, |x| Value::String(x.to_string()));
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("mois".into(), nombre(p.mois));
    m.insert("legal".into(), nombre(p.legal));
    m.insert("convention".into(), nombre(p.convention));
    m.insert("qui".into(), Value::String(p.qui));
    m.insert("dit".into(), Value::String(p.dit));
    m.insert("pourquoi".into(), p.pourquoi.map_or(Value::Null, Value::String));
    Ok(Value::Object(m))
}

/// `lignes` : les lignes que le moteur deduit de chaque engagement paye au
/// mois (null pour les autres), sans rien calculer d'autre ; la DPAE lit une
/// saisie sans la payer. `{"fichier"|"saisie"}` -> `{"lignes": [...]}`.
fn lignes(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let data = match (d.get("fichier").and_then(Value::as_str), d.get("saisie")) {
        (Some(f), _) => crate::engagement::lire_saisie(Path::new(f))?,
        (None, Some(s)) if s.is_object() => s.clone(),
        _ => return Err("demande illisible : il faut `fichier` ou `saisie`".to_string()),
    };
    let mois = texte(&data, "mois")?;
    let _derivation = crate::conges::Derivation::ouvrir();
    let mut out = Vec::new();
    for e in data.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let nat = crate::contrats::nature(e.get("nature").and_then(Value::as_str))?;
        out.push(if nat.mensuelle() {
            let (l, dits) = crate::conges::lignes_du_mois(&ctx, e, mois)?;
            let mut m = Map::new();
            m.insert("lignes".into(), Value::Array(l));
            m.insert("avertissements".into(), avis(&dits));
            Value::Object(m)
        } else {
            Value::Null
        });
    }
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("lignes".into(), Value::Array(out));
    Ok(Value::Object(m))
}

/// Les saisies d'une demande, chacune avec ses numeros (`bulletin`, `contrat`).
fn saisies_numerotees(ctx: &Contexte, liste: &[Value]) -> Resultat<Vec<Saisie>> {
    let mut saisies = Vec::new();
    for (k, d) in liste.iter().enumerate() {
        let mut s = saisie(ctx, d)?;
        let numeros = d.get("numeros").and_then(Value::as_array).ok_or("demande illisible : il faut `numeros`")?;
        if numeros.len() != s.paies.len() {
            return Err(format!(
                "demande illisible : la saisie {} a {} paies, `numeros` en donne {}",
                k, s.paies.len(), numeros.len()
            ));
        }
        for (p, n) in s.paies.iter_mut().zip(numeros) {
            let texte = |i: usize| n.get(i).and_then(Value::as_str).map(str::to_string);
            p.numero_bulletin = texte(0);
            p.engagement.numero = texte(1);
        }
        saisies.push(s);
    }
    Ok(saisies)
}

/// `dsn` : la DSN mensuelle. Demande : `saisies` (comme `bulletin`), `paies`
/// (`{"saisie", "index"}`, dans l'ordre), `mois`, `regularisations`
/// (`{"mois", "avant", "apres", "dsn_deposee", "taux_pas_origine"}`, `avant`/`apres` comme
/// une paie, `dsn_deposee` le chemin de la DSN deposee du mois d'origine, ou se lit ce que
/// l'annulation retire), `arrets` (les arrets tels que saisis), `pas_par_salarie`
/// (cle -> triplet ou null), les options de `generer`, et `sortie` (null :
/// le texte dans la reponse).
/// La paie qu'une demande designe par `{"saisie", "index"}`.
fn paie_designee(saisies: &[Saisie], j: &Value) -> Resultat<crate::calcul::Paie> {
    let entier = |c: &str| j.get(c).and_then(Value::as_u64).map(|n| n as usize);
    entier("saisie")
        .and_then(|k| saisies.get(k))
        .and_then(|s| s.paies.get(entier("index")?))
        .cloned()
        .ok_or_else(|| "demande illisible : une paie designe une saisie qui n'existe pas".to_string())
}

fn dsn(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    use crate::dsn::{Arret, Options, Regularisation};
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let liste = |c: &str| d.get(c).and_then(Value::as_array).cloned().unwrap_or_default();
    let saisies = saisies_numerotees(&ctx, &liste("saisies"))?;
    let paie = |j: &Value| paie_designee(&saisies, j);
    let paies = liste("paies").iter().map(&paie).collect::<Resultat<Vec<_>>>()?;
    let mut regs = Vec::new();
    for r in liste("regularisations") {
        let mois_o = texte(&r, "mois")?.to_string();
        let apres = paie(r.get("apres").unwrap_or(&Value::Null))?;
        // Ce qui a ete declare se lit dans la DSN deposee du mois d'origine : la demande la nomme.
        let deposee = texte(&r, "dsn_deposee")?;
        let nir = apres.salarie.get("nir").and_then(Value::as_str).unwrap_or("").to_string();
        let numero = apres.engagement.numero.clone().unwrap_or_default();
        let declare = crate::versement_declare::lire(Path::new(deposee), &mois_o, &nir, &numero)?;
        regs.push(Regularisation {
            mois: mois_o,
            avant: paie(r.get("avant").unwrap_or(&Value::Null))?,
            declare,
            apres,
            taux_pas_origine: r.get("taux_pas_origine").and_then(Value::as_str).map(str::to_string),
            // Le net deja verse se lit sur le bulletin recu ; la demande le dit, il ne se devine pas.
            net_deja_verse: crate::num::dec(r.get("net_deja_verse").ok_or(
                "regularisation sans `net_deja_verse` : le net a payer avant impot deja verse (lu sur le bulletin recu) \
                 dit si la correction verse quelque chose, et a quelle periode se rattache son montant net social",
            )?)?,
        });
    }
    let arrets: Vec<Arret> = liste("arrets").into_iter().map(|v| Arret { v }).collect();
    let mut op = Options::default();
    let s = |c: &str| d.get(c).filter(|v| !v.is_null()).map(crate::date::en_texte);
    if let Some(v) = s("date_constitution") {
        op.date_constitution = Some(Jour::lire(&v)?);
    }
    if let Some(v) = s("fraction") {
        op.fraction = v;
    }
    if let Some(v) = s("type_declaration") {
        op.type_declaration = v;
    }
    op.envoi_test = d.get("envoi_test").and_then(Value::as_bool).unwrap_or(false);
    if let Some(v) = s("ordre") {
        op.ordre = v.trim().parse().map_err(|_| format!("ordre illisible : {}", v))?;
    }
    op.remplace = s("remplace").filter(|r| !r.is_empty());
    op.etablissement = s("etablissement").filter(|r| !r.is_empty());
    // Un annule et remplace : la declaration remplacee portait-elle un versement DGFiP ? Absent, on ne sait pas.
    op.remplacee_versait_pas = d.get("remplacee_versait_pas").and_then(Value::as_bool);
    if let Some(m) = d.get("pas_par_salarie").and_then(Value::as_object) {
        for (cle, t) in m {
            let triplet = match t.as_array() {
                Some(a) if a.len() == 3 => Some((a[0].clone(), a[1].clone(), a[2].clone())),
                _ => None,
            };
            op.pas_par_salarie.insert(cle.clone(), triplet);
        }
    }
    let mois = texte(&d, "mois")?;
    let texte_dsn = crate::dsn::generer(&ctx, &paies, mois, &regs, &arrets, &op)?;
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    match d.get("sortie").and_then(Value::as_str) {
        Some(chemin) => {
            crate::dsn::ecrire(&texte_dsn, Path::new(chemin))?;
            m.insert("chemin".into(), Value::String(chemin.to_string()))
        }
        None => m.insert("texte".into(), Value::String(texte_dsn)),
    };
    Ok(Value::Object(m))
}

/// La reponse d'un signalement : un refus de la norme (`SignalementImpossible`)
/// est `{"ok": true, "refus": "..."}` ; le Python releve la bonne classe.
fn reponse_signalement(r: Result<Map<String, Value>, crate::signalement::Echec>) -> Resultat<Value> {
    use crate::signalement::Echec;
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    match r {
        Ok(corps) => m.extend(corps),
        Err(Echec::Impossible(refus)) => {
            m.insert("refus".into(), Value::String(refus));
        }
        Err(Echec::Autre(e)) => return Err(e),
    }
    Ok(Value::Object(m))
}

/// `signalement` : le fichier d'un signalement d'arret (04) ou de reprise (05).
/// Demande : `arret` (l'arret tel que saisi), `nature`, `type_declaration`,
/// `ordre`, `remplace`, `envoi_test`, `date_constitution`, et `sortie` (null :
/// le texte dans la reponse, sinon le chemin du fichier ecrit en latin-1).
fn signalement(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    use crate::signalement::Options;
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let arret = crate::dsn::Arret { v: d.get("arret").cloned().unwrap_or(Value::Null) };
    let mut op = Options::default();
    let s = |c: &str| d.get(c).filter(|v| !v.is_null()).map(crate::date::en_texte);
    if let Some(v) = s("nature") {
        op.nature = v;
    }
    if let Some(v) = s("type_declaration") {
        op.type_declaration = v;
    }
    if let Some(v) = s("ordre") {
        op.ordre = v.trim().parse().map_err(|_| format!("ordre illisible : {}", v))?;
    }
    op.remplace = s("remplace").filter(|r| !r.is_empty());
    op.envoi_test = d.get("envoi_test").and_then(Value::as_bool).unwrap_or(false);
    if let Some(v) = s("date_constitution") {
        op.date_constitution = Some(Jour::lire(&v)?);
    }
    reponse_signalement(crate::signalement::generer(&ctx, &arret, &op).and_then(|texte_dsn| {
        let mut corps = Map::new();
        match d.get("sortie").and_then(Value::as_str) {
            Some(chemin) => {
                crate::dsn::ecrire(&texte_dsn, Path::new(chemin))?;
                corps.insert("chemin".into(), Value::String(chemin.to_string()))
            }
            None => corps.insert("texte".into(), Value::String(texte_dsn)),
        };
        Ok(corps)
    }))
}

/// `signalement-verifier` : les controles de la norme sur un arret, sans
/// contexte. Demande : `arret`, `nature`, `constitution` (null : aujourd'hui).
/// Reponse : `{"avertissements": [...]}` ou le refus.
fn signalement_verifier(entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let arret = crate::dsn::Arret { v: d.get("arret").cloned().unwrap_or(Value::Null) };
    let nature = d.get("nature").map(crate::date::en_texte).unwrap_or_default();
    let constitution = d
        .get("constitution")
        .filter(|v| !v.is_null())
        .map(|v| crate::date::d(v))
        .transpose()?;
    reponse_signalement(crate::signalement::verifier(&arret, &nature, constitution.as_ref()).map(|avis| {
        let mut corps = Map::new();
        corps.insert("avertissements".into(), Value::Array(avis.into_iter().map(Value::String).collect()));
        corps
    }))
}

/// `montant-urssaf` : le bordereau Urssaf d'un mois pour l'apercu. Demande :
/// `saisies` et `paies` (comme `dsn`), `mois`, `etablissement` (null : le
/// principal) -> `{"montant"}`.
fn montant_urssaf(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let liste = |c: &str| d.get(c).and_then(Value::as_array).cloned().unwrap_or_default();
    let saisies = saisies_numerotees(&ctx, &liste("saisies"))?;
    let paies = liste("paies").iter().map(|j| paie_designee(&saisies, j)).collect::<Resultat<Vec<_>>>()?;
    let etab = d.get("etablissement").and_then(Value::as_str);
    let montant = crate::dsn::montant_urssaf(&ctx, &paies, texte(&d, "mois")?, etab)?;
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("montant".into(), Value::String(montant.to_string()));
    Ok(Value::Object(m))
}

/// Ce que `fctu` et `amorcage` ont en commun : la paie designee, puis les
/// parametres d'un signalement (type de declaration, ordre, declaration
/// remplacee, essai, date de constitution).
struct Signalement {
    paie: crate::calcul::Paie,
    type_declaration: String,
    ordre: i64,
    remplace: Option<String>,
    envoi_test: bool,
    date_constitution: Option<Jour>,
}

/// Les saisies d'une demande vues comme des EMBAUCHES (`embauche: true`) :
/// les engagements et leur fiche, sans calculer la paie (la DPAE n'en a pas).
fn saisies_embauches(ctx: &Contexte, liste: &[Value]) -> Resultat<Vec<Saisie>> {
    let mut saisies = Vec::new();
    for (k, d) in liste.iter().enumerate() {
        let data = match (d.get("fichier").and_then(Value::as_str), d.get("saisie")) {
            (Some(f), _) => crate::engagement::lire_saisie(Path::new(f))?,
            (None, Some(s)) if s.is_object() => s.clone(),
            _ => return Err("demande illisible : il faut `fichier` ou `saisie`".to_string()),
        };
        let _derivation = crate::conges::Derivation::ouvrir();
        let (engs, _) = crate::engagement::engagements(ctx, &data, &crate::conges::lignes_du_mois)?;
        let numeros = d.get("numeros").and_then(Value::as_array).ok_or("demande illisible : il faut `numeros`")?;
        if numeros.len() != engs.len() {
            return Err(format!(
                "demande illisible : la saisie {} a {} paies, `numeros` en donne {}",
                k, engs.len(), numeros.len()
            ));
        }
        let mut paies = Vec::new();
        for (e, n) in engs.iter().zip(numeros) {
            let mut p = crate::amorcage::embauche(ctx, e)?;
            p.engagement.numero = n.get(1).and_then(Value::as_str).map(str::to_string);
            paies.push(p);
        }
        saisies.push(Saisie { mois: crate::date::en_texte(&data["mois"]), paies });
    }
    Ok(saisies)
}

fn signalement_de_paie(ctx: &Contexte, d: &Value) -> Resultat<Signalement> {
    let liste = |c: &str| d.get(c).and_then(Value::as_array).cloned().unwrap_or_default();
    let saisies = if d.get("embauche").and_then(Value::as_bool).unwrap_or(false) {
        saisies_embauches(ctx, &liste("saisies"))?
    } else {
        saisies_numerotees(ctx, &liste("saisies"))?
    };
    let paie = paie_designee(&saisies, d.get("paie").unwrap_or(&Value::Null))?;
    let s = |c: &str| d.get(c).filter(|v| !v.is_null()).map(crate::date::en_texte);
    let ordre = match s("ordre") {
        Some(v) => v.trim().parse().map_err(|_| format!("ordre illisible : {}", v))?,
        // Sans ordre donne : le suivant de tous les signalements deja partis (cahier technique DSN, p. 136).
        None => crate::journal::dernier_ordre_des_signalements(&ctx.racine)? + 1,
    };
    Ok(Signalement {
        paie,
        type_declaration: s("type_declaration").unwrap_or_else(|| "01".to_string()),
        ordre,
        remplace: s("remplace").filter(|r| !r.is_empty()),
        envoi_test: d.get("envoi_test").and_then(Value::as_bool).unwrap_or(false),
        date_constitution: s("date_constitution").map(|v| Jour::lire(&v)).transpose()?,
    })
}

/// La reponse d'un signalement : le texte, ou le chemin si `sortie` est donne.
/// Un refus de la norme (`Echec::Impossible`) rend sa phrase et sa `classe`,
/// que le Python releve avec la bonne exception.
fn reponse_fctu_amorcage(d: &Value, r: Result<String, crate::fctu::Echec>, classe: &str) -> Resultat<Value> {
    let mut m = Map::new();
    let texte_s = match r {
        Ok(t) => t,
        Err(crate::fctu::Echec::Autre(e)) => return Err(e),
        Err(crate::fctu::Echec::Impossible(e)) => {
            m.insert("ok".into(), Value::Bool(false));
            m.insert("erreur".into(), Value::String(e));
            m.insert("classe".into(), Value::String(classe.into()));
            return Ok(Value::Object(m));
        }
    };
    m.insert("ok".into(), Value::Bool(true));
    match d.get("sortie").and_then(Value::as_str) {
        Some(chemin) => {
            crate::dsn::ecrire(&texte_s, Path::new(chemin))?;
            m.insert("chemin".into(), Value::String(chemin.to_string()))
        }
        None => m.insert("texte".into(), Value::String(texte_s)),
    };
    Ok(Value::Object(m))
}

/// `fctu` : le signalement de fin de contrat (nature 07). Demande : `saisies`
/// et `paie` (`{"saisie", "index"}`, comme `dsn`), `fin` (la `FinDeContrat`
/// en dictionnaire), `type_declaration`, `ordre`, `remplace`, `envoi_test`,
/// `date_constitution`, `suspensions` (les blocs 65 a ecrire), `arrets`, et
/// `sortie` (null : le texte dans la reponse).
fn fctu(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let sg = signalement_de_paie(&ctx, &d)?;
    let fin = crate::fctu::FinDeContrat::lire(d.get("fin").ok_or("demande illisible : il faut `fin`")?)?;
    let liste = |c: &str| d.get(c).and_then(Value::as_array).cloned().unwrap_or_default();
    let arrets: Vec<crate::dsn::Arret> = liste("arrets").into_iter().map(|v| crate::dsn::Arret { v }).collect();
    let op = crate::fctu::Options {
        type_declaration: sg.type_declaration,
        ordre: sg.ordre,
        remplace: sg.remplace,
        envoi_test: sg.envoi_test,
        date_constitution: sg.date_constitution,
    };
    let r = crate::fctu::generer(&ctx, &sg.paie, &fin, &op, &liste("suspensions"), &arrets);
    reponse_fctu_amorcage(&d, r, "FctuImpossible")
}

/// `amorcage` : le signalement d'amorcage des donnees variables (nature 08).
/// Demande : `saisies` et `paie`, `motif`, `type_declaration`, `ordre`,
/// `remplace`, `envoi_test`, `date_constitution`, `sortie`.
fn amorcage(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let d = demande(entree)?;
    let ctx = contexte(o)?;
    let sg = signalement_de_paie(&ctx, &d)?;
    let op = crate::amorcage::Options {
        motif: texte(&d, "motif")?.to_string(),
        type_declaration: sg.type_declaration,
        ordre: sg.ordre,
        remplace: sg.remplace,
        envoi_test: sg.envoi_test,
        date_constitution: sg.date_constitution,
    };
    let r = crate::amorcage::generer(&ctx, &sg.paie, &op);
    reponse_fctu_amorcage(&d, r, "AmorcageImpossible")
}

/// `bordereau` : le montant de bordereaux Urssaf donnes par leurs lignes 23.
/// Demande : `bordereaux`, chacun `{"jour", "lignes"}`, une ligne
/// `{"code", "qualifiant", "taux_at", "assiette" | "montant"}` ->
/// `{"montants"}`, dans l'ordre.
fn bordereau(entree: &str) -> Resultat<Value> {
    use crate::bordereau::{montant, LigneBordereau};
    let d = demande(entree)?;
    let champ = |l: &Value, c: &str| l.get(c).and_then(Value::as_str).unwrap_or("").to_string();
    let nombre = |l: &Value, c: &str| -> Resultat<Option<crate::num::Decimal>> {
        l.get(c).filter(|v| !v.is_null()).map(crate::num::dec_py).transpose()
    };
    let mut montants = Vec::new();
    for b in d.get("bordereaux").and_then(Value::as_array).ok_or("demande illisible : il faut `bordereaux`")? {
        let mut lignes = Vec::new();
        for l in b.get("lignes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            lignes.push(LigneBordereau {
                code: champ(l, "code"),
                qualifiant: champ(l, "qualifiant"),
                taux_at: champ(l, "taux_at"),
                assiette: nombre(l, "assiette")?,
                montant: nombre(l, "montant")?,
                commune: None,
            });
        }
        montants.push(Value::String(montant(&lignes, &Jour::lire(texte(b, "jour")?)?)?.to_string()));
    }
    let mut m = Map::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("montants".into(), Value::Array(montants));
    Ok(Value::Object(m))
}

/// `paie route` et `paie interne bulletin|droits|provision-conges|preavis|lignes|dsn|montant-urssaf|fctu|amorcage|...
/// [--racine R] [--date D]` ; la demande sur l'entree standard.
/// `None` : usage.
pub fn commande(quoi: &str, args: &[String]) -> Option<i32> {
    let o = options(args)?;
    let mut entree = String::new();
    let corps = match std::io::stdin().read_to_string(&mut entree) {
        Ok(_) => {
            let entree = entree.trim_start_matches('\u{feff}');
            match quoi {
                "route" => crate::routes::commande(&o, entree),
                "bulletin" => bulletin(&o, entree),
                "bareme-applique" => bareme_applique(entree),
                "droits" => droits(&o, entree),
                "provision-conges" => provision_conges(&o, entree),
                "preavis" => preavis(&o, entree),
                "lignes" => lignes(&o, entree),
                "dsn" => dsn(&o, entree),
                "montant-urssaf" => montant_urssaf(&o, entree),
                "fctu" => fctu(&o, entree),
                "amorcage" => amorcage(&o, entree),
                "bordereau" => bordereau(entree),
                "signalement" => signalement(&o, entree),
                "signalement-verifier" => signalement_verifier(entree),
                "dpae" => crate::dpae::commande(entree),
                "dpae-lire" => crate::dpae::commande_lire(entree),
                "retours" => crate::retours::commande(entree),
                "ecrire-json" => crate::ecriture::commande(entree),
                "verrou" => crate::verrou::commande(entree),
                q if q.starts_with("aer") => match crate::aer::service(q, entree) {
                    Some(r) => r,
                    None => return None,
                },
                _ => return None,
            }
        }
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
