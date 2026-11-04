//! La pile de bareme, de `bareme_du_statut` a `bareme_cadre` : statut, nature du contrat, convention, sante, retraite,
//! categorie, prevoyance, regime local, versement mobilite.
//!
//! Un bareme est un `serde_json::Value` : chaque etage rend un NOUVEAU bareme et ne modifie pas le precedent. Un refus
//! est une `String`. Les cles `_` d'un bareme sont des commentaires, sauf celles que la pile pose elle-meme.
//!
//! Avec elle, parce que la pile les appelle : la lecture du catalogue de prevoyance de branche (module `prevoyance`
//! ci-dessous) et l'etablissement du salarie (`etablissement_de`, `surcharger`).

use socle::texte::sans_accent;
use crate::forme_bareme::Pile;
use crate::natures::Nature;
use crate::fiche::{Case, ChampsEntreprise, ChampsSalarie, Etablissement, SalarieDuContrat, Statut, StatutCategorielRc};
use socle::date::{d, en_texte, Jour};
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use socle::num::{add, dec, div, flottant, mul, sub, Decimal, Resultat};
use rust_decimal::RoundingStrategy;
use serde_json::{Map, Number, Value};

// ------------------------------------------------------------ les petits outils

static NUL: Value = Value::Null;

/// `v.get(k)` d'un objet ; `Null` si la cle manque ou si `v` n'en est pas un.
fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

/// La valeur si la cle est la, meme nulle.
fn g_def<'a>(v: &'a Value, k: &str, defaut: &'a Value) -> &'a Value {
    v.get(k).unwrap_or(defaut)
}

/// La valeur de `k`, exigee.
fn exiger_m<'a>(m: &'a Map<String, Value>, k: &str) -> Resultat<&'a Value> {
    m.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// La valeur de `k`, exigee : son absence se refuse en la nommant.
fn exiger<'a>(v: &'a Value, k: &str) -> Resultat<&'a Value> {
    v.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// Les éléments d'une liste ; aucun si ce n'en est pas une.
fn liste(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// Les couples de `(x or {}).items()`.
fn paires(v: &Value) -> impl Iterator<Item = (&String, &Value)> {
    v.as_object().into_iter().flat_map(|o| o.iter())
}

/// L'objet `v`, recopie.
fn copie(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

/// L'objet `v` quand `v` n'est plus servi : lui-meme, sans recopie.
fn en_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// Une valeur en texte, vide si elle manque ou est vide.
fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v)
    } else {
        String::new()
    }
}

fn texte(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Le flottant d'une valeur JSON.
fn nf(x: f64) -> Value {
    Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
}

/// Le décimal en flottant.
fn flt(x: Decimal) -> f64 {
    x.to_string().parse().unwrap_or(f64::NAN)
}

fn nd(x: Decimal) -> Value {
    nf(flt(x))
}

/// `round(x, 4)` d'un flottant : arrondi correct, au pair sur le demi exact.
fn arrondi4(x: f64) -> f64 {
    format!("{:.4}", x).parse().unwrap_or(x)
}

/// `round(Decimal, n)` : au pair.
fn rond(x: Decimal, n: u32) -> Decimal {
    x.round_dp_with_strategy(n, RoundingStrategy::MidpointNearestEven)
}

/// `"%.2f" % x` avec la virgule.
fn deux_virgule(x: f64) -> String {
    format!("{:.2}", x).replace('.', ",")
}

/// L'annee d'un bareme, `?` si elle manque, pour un message.
fn annee(bar: &Value) -> String {
    bar.get("annee").map(en_texte).unwrap_or_else(|| "?".to_string())
}

/// L'entier d'une valeur, 0 si elle manque ou est vide.
fn entier(v: &Value) -> i64 {
    if !vrai(v) {
        return 0;
    }
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|x| x.trunc() as i64)).unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        Value::Bool(b) => i64::from(*b),
        _ => 0,
    }
}

/// Les cles d'un objet qui ne commencent pas par `_`, avec leurs valeurs.
fn sans_souligne(v: &Value) -> Map<String, Value> {
    paires(v).filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), x.clone())).collect()
}

/// `d.update({k: v ... si k ne commence pas par "_"})`.
fn mettre_a_jour(cible: &mut Map<String, Value>, source: &Value) {
    for (k, v) in paires(source) {
        if !k.starts_with('_') {
            cible.insert(k.clone(), v.clone());
        }
    }
}

/// La date de fin de période (ses dix premiers caractères), vide si elle manque.
fn jour_texte(fin: Option<&Jour>) -> String {
    fin.map(Jour::iso).unwrap_or_default()
}

/// L'indice de la rubrique d'identifiant `id`.
fn rang(rubriques: &[Value], id: &Value) -> Option<usize> {
    rubriques.iter().position(|r| g(r, "id") == id)
}

fn insere(rubriques: &mut Vec<Value>, i: Option<usize>, v: Value) {
    match i {
        None => rubriques.push(v),
        Some(i) => rubriques.insert(i + 1, v),
    }
}

/// La nature, quand elle est un permanent (`nat and nat.mensuelle`).
fn mensuelle(nat: Option<&Nature>) -> bool {
    nat.map_or(false, |n| n.mensuelle())
}

fn nom_du_salarie(sal: &SalarieDuContrat, defaut: &str) -> String {
    sal.nomme()
        .or_else(|| Some(sal.id.to_string()))
        .unwrap_or_else(|| defaut.to_string())
}

// ---------------------------------------------------------------- le statut

/// Le bareme applicable a ce statut, ou une erreur qui dit ce qui manque.
/// Trois issues : le statut du corps (on rend le bareme), une section
/// `par_statut` verifiee et sans trou (ses sections remplacent celles du
/// corps), sinon un refus qui nomme tout ce qui bloque.
pub fn bareme_du_statut(bar: Value, statut: &str) -> Resultat<Value> {
    let statut = statut.trim().to_lowercase();
    let statuts: Vec<String> = if vrai(g(&bar, "statuts")) {
        liste(g(&bar, "statuts")).iter().map(en_texte).collect()
    } else {
        vec!["artiste".to_string()]
    };
    if statut.is_empty() || statuts.contains(&statut) {
        return Ok(bar);
    }

    let section = g(g(&bar, "par_statut"), &statut);
    if !vrai(section) {
        return Err(socle::echec::avec(format!("Le barème {} ne sait pas payer ce statut : le signaler au développeur.", annee(&bar)), serde_json::json!({"detail": format!(
            "Statut '{}' : le bareme {} ne sait pas le calculer.\n  \
Il decrit les statuts {}. L'abattement de 30 % sur les taux \
(arrete du 24/01/1975) ne vaut que pour les artistes, et il est \
cuit dans ces taux : s'en servir pour un autre statut donnerait \
des cotisations d'artiste a quelqu'un qui n'en est pas, sans que \
rien ne le signale.\n  \
Ajouter une section par_statut['{}'] au bareme.",
            statut,
            annee(&bar),
            statuts.join(", "),
            statut
        )})));
    }

    let mut trous: Vec<String> = Vec::new();
    for r in liste(g(section, "rubriques_bulletin")) {
        if vrai(g(r, "rempli_par")) {
            continue;
        }
        if g(r, "taux_pat").is_null() && g(r, "taux_sal").is_null() && !vrai(g(r, "composants")) {
            let quoi = g(r, "a_confirmer");
            trous.push(format!(
                "{} ({})",
                en_texte(g(r, "id")),
                if vrai(quoi) { en_texte(quoi) } else { "taux absent".to_string() }
            ));
        }
    }
    if !trous.is_empty() || !vrai(g(section, "verifie")) {
        let mut detail: Vec<String> = Vec::new();
        if !trous.is_empty() {
            detail.push(format!("{} rubrique(s) sans taux :\n    - {}", trous.len(), trous.join("\n    - ")));
        }
        let etat = g(section, "note");
        let detail = if detail.is_empty() {
            "(rien de precis n'est signale)".to_string()
        } else {
            detail.join("\n  ")
        };
        return Err(socle::echec::avec(format!("Le barème {} ne paie pas encore ce statut : ses taux ne sont pas vérifiés.", annee(&bar)), serde_json::json!({"detail": format!(
            "Statut '{}', bareme {} : refus de payer.\n  {}\n  {}\n  \
Rien de tout cela ne se devine : un taux se releve (Urssaf, \
BOSS, fiche de parametrage Audiens, bulletin de reference), un \
mecanisme se decide. Tant que la section n'est pas marquee \
\"verifie\": true par un humain, le moteur refuse.",
            statut,
            annee(&bar),
            if vrai(etat) { en_texte(etat) } else { "Section non marquee verifiee.".to_string() },
            detail
        )})));
    }

    let mut poses: Vec<(&str, Value)> = Vec::new();
    for cle in ["rubriques_bulletin", "cotisations_dsn", "taux_chomage_aem", "reduction_generale"] {
        if let Some(v) = section.get(cle) {
            poses.push((cle, v.clone()));
        }
    }
    let mut applique = en_map(bar);
    for (cle, v) in poses {
        applique.insert(cle.to_string(), v);
    }
    Ok(Value::Object(applique))
}

// --------------------------------------------------- la nature du contrat

/// Le bareme du statut, avec la surcouche de la NATURE DU CONTRAT
/// (`par_nature`). Sans section verifiee et sans trou, on REFUSE.
pub fn bareme_de_la_nature(bar: Value, nat: &Nature) -> Resultat<Value> {
    let nom = match &nat.bareme {
        Some(n) if !n.is_empty() => n.clone(),
        _ => return Ok(bar),
    };
    let sec = g(g(&bar, "par_nature"), &nom);
    if !vrai(sec) {
        return Err(socle::echec::avec(format!("Le barème {} ne sait pas payer cette nature de contrat : le signaler au développeur.", annee(&bar)), serde_json::json!({"detail": format!(
            "Contrat de nature '{}' : le bareme {} n'a pas de section \
par_nature['{}'].\n  \
Le corps de ce bareme decrit un INTERMITTENT : Conges \
Spectacles, chomage majore, formation a 2 %, assiettes majorees \
pour conges. Un permanent n'a rien de tout cela, et il a en plus \
une mutuelle sante obligatoire. Le payer avec ce bareme-la \
donnerait des cotisations fausses sans que rien ne le signale.",
            nat.cle,
            annee(&bar),
            nom
        )})));
    }
    let mut trous: Vec<String> = liste(g(sec, "rubriques_ajoutees"))
        .iter()
        .filter(|r| {
            g(r, "taux_pat").is_null()
                && g(r, "taux_sal").is_null()
                && !vrai(g(r, "composants"))
                && !vrai(g(r, "rempli_par"))
        })
        .map(|r| format!("rubriques_ajoutees[{}]", en_texte(g(r, "id"))))
        .collect();
    trous.extend(a_confirmer(sec, ""));
    let mut uniques: Vec<String> = Vec::new();
    for t in trous {
        if !t.is_empty() && !uniques.contains(&t) {
            uniques.push(t);
        }
    }
    if !uniques.is_empty() || !vrai(g(sec, "verifie")) {
        let etat = g(sec, "note");
        return Err(socle::echec::avec(format!("Le barème {} ne paie pas encore cette nature de contrat : ses taux ne sont pas vérifiés.", annee(&bar)), serde_json::json!({"detail": format!(
            "Contrat de nature '{}', bareme {} : refus de payer.\n  {}\n  A remplir : {}\n  \
Un taux se releve (BOSS, Agirc-Arrco, contrat de mutuelle, \
fiche de parametrage Audiens), il ne se devine pas. Tant que la \
section par_nature['{}'] n'est pas marquee \"verifie\": true \
par un humain, le moteur refuse.",
            nat.cle,
            annee(&bar),
            if vrai(etat) { en_texte(etat) } else { "Section non marquee verifiee.".to_string() },
            if uniques.is_empty() { "(rien de precis)".to_string() } else { uniques.join(", ") },
            nom
        )})));
    }
    let sec = sec.clone();
    empiler(bar, &sec)
}

/// LA CONTRIBUTION CPF-CDD d'un contrat qui l'ouvre (`assujettissement::contrat_ouvre_cpf_cdd` :
/// un CDD de droit commun, ni saisonnier, ni CDDU, ni contrat aide) : la section `cpf_cdd` de la nature du contrat
/// (`par_nature.<nature>.cpf_cdd`) ajoute son taux comme un composant de la rubrique qu'elle nomme (« Autres
/// contributions dues par l'employeur », a cote de la formation professionnelle) et sa cotisation au nominatif (129). Le
/// taux vient du texte (C. trav. L. 6331-6, 1 %), cite par la section. Une nature sans section, pour un contrat qui
/// ouvre la contribution : refus (regle 1).
pub fn bareme_cpf_cdd(bar: Value, nat: &Nature, cpf_cdd: bool) -> Resultat<Value> {
    if !cpf_cdd {
        return Ok(bar);
    }
    let nom = nat.bareme.clone().unwrap_or_default();
    let sec = g(g(g(&bar, "par_nature"), &nom), "cpf_cdd").clone();
    if !vrai(&sec) {
        return Err(socle::echec::avec(format!("Le barème {} ne porte pas la contribution formation d'un CDD : le signaler au développeur.", annee(&bar)), serde_json::json!({"detail": format!(
            "Contrat de nature '{}' : il ouvre la contribution CPF-CDD (code du travail L. 6331-6), et le bareme {} n'a pas de \
section par_nature['{}'].cpf_cdd qui dise son taux et sa cotisation. Rien n'est calcule.",
            nat.cle,
            annee(&bar),
            nom
        )})));
    }
    let taux = g(&sec, "taux").as_f64().ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", &format!("par_nature.{}.cpf_cdd.taux", nom), ""))?;
    let rubrique = txt(g(&sec, "rubrique"));
    let mut applique = en_map(bar);
    let mut rubriques: Vec<Value> = liste(exiger_m(&applique, "rubriques_bulletin")?).to_vec();
    let r = rubriques
        .iter_mut()
        .find(|r| txt(g(r, "id")) == rubrique)
        .ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", &format!("par_nature.{}.cpf_cdd.rubrique", nom), &rubrique))?;
    let mut composants: Vec<Value> = liste(g(r, "composants")).to_vec();
    if composants.is_empty() {
        composants.push(r.get("taux_pat").cloned().unwrap_or(Value::Null));
    }
    composants.push(Value::from(taux));
    let total: f64 = composants.iter().map(|c| c.as_f64().unwrap_or(0.0)).sum();
    let r = r.as_object_mut().ok_or("rubrique illisible")?;
    r.insert("composants".into(), Value::Array(composants));
    // La somme affichee, au millieme comme les composants du bareme (0,30 + 0,016 + 0,55 + 1,0 = 1,866).
    r.insert("taux_pat".into(), Value::from((total * 1000.0).round() / 1000.0));
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    let mut cotisations: Vec<Value> = liste(applique.get("cotisations_dsn").unwrap_or(&NUL)).to_vec();
    cotisations.push(g(&sec, "cotisation_dsn").clone());
    applique.insert("cotisations_dsn".into(), Value::Array(cotisations));
    Ok(Value::Object(applique))
}

/// LA MAJORATION DU CHOMAGE QUE LE CONTRAT NE DOIT PAS (`majoration_chomage::due` : un CDD d'usage de plus de trois mois).
/// Le bareme la porte sur le corps (`majoration_chomage_cddu` : la ligne du bulletin, son taux, sa cotisation 041) ; un
/// contrat qui ne la doit pas paie la ligne sans elle et ne declare pas la cotisation. Une pile qui ne la porte pas (un
/// permanent) n'a rien a retirer.
pub fn bareme_majoration_chomage(bar: Value, due: bool) -> Resultat<Value> {
    let sec = g(&bar, "majoration_chomage_cddu").clone();
    let code = txt(g(&sec, "code81"));
    let porte = liste(g(&bar, "cotisations_dsn")).iter().any(|c| txt(g(c, "code81")) == code);
    if due || !vrai(&sec) || !porte {
        return Ok(bar);
    }
    let taux = dec(exiger(&sec, "taux")?)?;
    let rubrique = txt(g(&sec, "rubrique"));
    let mut applique = en_map(bar);
    let mut rubriques: Vec<Value> = liste(exiger_m(&applique, "rubriques_bulletin")?).to_vec();
    let r = rubriques
        .iter_mut()
        .find(|r| txt(g(r, "id")) == rubrique)
        .ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année est incomplet : le signaler au développeur.", "majoration_chomage_cddu.rubrique", &rubrique))?;
    let avant = dec(exiger(r, "taux_pat")?)?;
    if let Some(o) = r.as_object_mut() {
        o.insert("taux_pat".into(), nd(sub(avant, taux)));
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    let cotisations: Vec<Value> = liste(applique.get("cotisations_dsn").unwrap_or(&NUL)).iter().filter(|c| txt(g(c, "code81")) != code).cloned().collect();
    applique.insert("cotisations_dsn".into(), Value::Array(cotisations));
    Ok(Value::Object(applique))
}

/// LA TAXE D'APPRENTISSAGE NON DUE. Quand la fiche entreprise dit un motif de non-assujettissement
/// (table MNATA, `taxe_apprentissage_motif_non_assujettissement`) qui vaut pour la periode d'emploi `fin`
/// (`apprentissage::taxe_apprentissage_non_due_au_mois`), la taxe n'est pas due : C. trav. L. 6241-1, I et III, dit
/// qui la doit et qui n'en est « pas redevable ». Une taxe que l'employeur ne doit pas ne se calcule pas : la ligne du
/// bulletin qui la porte (`"taxe_apprentissage": true` dans le bareme, part principale et solde) et sa cotisation
/// individuelle (81.001 = 130, « Part principale de la taxe d'apprentissage », norme DSN, bloc 81) sortent de la pile ;
/// le bordereau retire ses CTP 992 et 993 par la meme decision (`dsn::lignes_bordereau`) : le bulletin, le nominatif
/// et le bordereau disent la meme taxe. Un motif qui
/// vaut, et une pile sans ligne marquee : refus (regle 1), plutot qu'une taxe qu'on garderait sans le dire.
pub fn bareme_taxe_apprentissage(bar: Value, ent: &ChampsEntreprise, fin: Option<&Jour>) -> Resultat<Value> {
    let Some(fin) = fin else { return Ok(bar) };
    if !crate::apprentissage::taxe_apprentissage_non_due_au_mois(ent, fin.an, fin.mois)? {
        return Ok(bar);
    }
    let mut applique = en_map(bar);
    let rubriques: Vec<Value> = liste(exiger_m(&applique, "rubriques_bulletin")?).to_vec();
    let avant = rubriques.len();
    let rubriques: Vec<Value> = rubriques.into_iter().filter(|r| !vrai(g(r, "taxe_apprentissage"))).collect();
    if rubriques.len() == avant {
        return Err(socle::echec::avec(format!("Le barème {} ne sait pas retirer la taxe d'apprentissage du bulletin : le signaler au développeur.", annee(&Value::Object(applique.clone()))), serde_json::json!({"detail": format!(
            "La fiche entreprise dit la compagnie non assujettie a la taxe d'apprentissage pour {:02}/{} (motif MNATA), et le bareme {} n'a aucune ligne marquee \"taxe_apprentissage\": true : le moteur ne sait pas quelle ligne retirer du bulletin. Marquer la ligne de la taxe dans le bareme. Rien n'est calcule.",
            fin.mois,
            fin.an,
            annee(&Value::Object(applique.clone()))
        )})));
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    let cotisations: Vec<Value> =
        liste(applique.get("cotisations_dsn").unwrap_or(&NUL)).iter().filter(|c| txt(g(c, "code81")) != "130").cloned().collect();
    applique.insert("cotisations_dsn".into(), Value::Array(cotisations));
    Ok(Value::Object(applique))
}

/// Les chemins de la section qui portent encore un `a_confirmer`.
pub fn a_confirmer(noeud: &Value, chemin: &str) -> Vec<String> {
    let mut out = Vec::new();
    match noeud {
        Value::Object(o) => {
            if vrai(g(noeud, "a_confirmer")) {
                out.push(if chemin.is_empty() { "(section)".to_string() } else { chemin.to_string() });
            }
            for (cle, v) in o {
                if !cle.starts_with('_') {
                    let sous = if chemin.is_empty() { cle.clone() } else { format!("{}.{}", chemin, cle) };
                    out.extend(a_confirmer(v, &sous));
                }
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                let mut nom = i.to_string();
                if v.is_object() {
                    let id = g(v, "id");
                    let code = g(v, "code81");
                    nom = if vrai(id) {
                        en_texte(id)
                    } else if vrai(code) {
                        en_texte(code)
                    } else {
                        nom
                    };
                    if vrai(g(v, "affectation")) {
                        nom = format!("{}/{}", nom, en_texte(g(v, "affectation")));
                    }
                }
                out.extend(a_confirmer(v, &format!("{}[{}]", chemin, nom)));
            }
        }
        _ => {}
    }
    out
}

/// Applique une section de surcouche sur les rubriques du bulletin :
/// remplacer par identifiant, retirer, ajouter apres une rubrique nommee.
pub fn empiler(bar: Value, sec: &Value) -> Resultat<Value> {
    let mut applique = en_map(bar);
    let retirees = liste(g(sec, "rubriques_retirees"));
    let mut rubriques: Vec<Value> = Vec::new();
    for r in liste(exiger_m(&applique, "rubriques_bulletin")?) {
        let id = exiger(r, "id")?;
        if retirees.contains(id) {
            continue;
        }
        let mut r = copie(r);
        if let Some(champs) = id.as_str().and_then(|i| g(sec, "rubriques").get(i)) {
            for (k, v) in paires(champs) {
                if !k.starts_with('_') {
                    r.insert(k.clone(), v.clone());
                }
            }
        }
        rubriques.push(Value::Object(r));
    }
    for a in liste(g(sec, "rubriques_ajoutees")) {
        let mut a = sans_souligne(a);
        let apres = a.shift_remove("apres").unwrap_or(Value::Null);
        let i = rang(&rubriques, &apres);
        insere(&mut rubriques, i, Value::Object(a));
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    for cle in [
        "cotisations_dsn",
        "cotisation_prevoyance_retraite_supp",
        "cotisation_sante_patronale",
        "majoration_conges_115",
        "majoration_conges_110",
    ] {
        if let Some(v) = sec.get(cle) {
            applique.insert(cle.to_string(), v.clone());
        }
    }
    // La reduction generale se complete, elle ne se remplace pas.
    if let Some(rgs) = sec.get("reduction_generale") {
        let mut rg = copie(applique.get("reduction_generale").unwrap_or(&NUL));
        for (k, v) in paires(rgs) {
            if k == "dsn" && v.is_object() {
                let mut dsn = copie(rg.get("dsn").unwrap_or(&NUL));
                for (kk, vv) in paires(v) {
                    dsn.insert(kk.clone(), vv.clone());
                }
                rg.insert("dsn".into(), Value::Object(dsn));
            } else {
                rg.insert(k.clone(), v.clone());
            }
        }
        applique.insert("reduction_generale".into(), Value::Object(rg));
    }
    Ok(Value::Object(applique))
}

// ------------------------------------------------------------ les cadres

/// S21.G00.40.003 = 01 : cadre au sens de la retraite complementaire.
pub fn est_cadre(sal: &ChampsSalarie) -> bool {
    sal.statut_categoriel_rc.dite() == Some(&StatutCategorielRc::Cadre)
}

/// Le salarie travaille-t-il dans le Bas-Rhin, le Haut-Rhin ou la Moselle ?
/// Le code postal de SON etablissement, sinon celui de l'entreprise.
pub fn alsace_moselle(ent: &ChampsEntreprise, sal: &SalarieDuContrat) -> bool {
    let mut cp = ent.code_postal.texte().trim();
    let nic = sal.etablissement.as_deref().unwrap_or("").trim();
    for e in ent.etablissements.dite().into_iter().flatten() {
        if !nic.is_empty() && e.nic.texte().trim() == nic {
            let c = e.code_postal.texte().trim();
            if !c.is_empty() {
                cp = c;
            }
        }
    }
    let debut: String = cp.chars().take(2).collect();
    matches!(debut.as_str(), "57" | "67" | "68")
}

/// Les cles d'un objet, triees, jusqu'a `jour` inclus (toutes si `jour` est vide).
fn dates_jusqu_a(table: &Value, jour: &str) -> Vec<String> {
    let mut ks: Vec<String> =
        paires(table).map(|(k, _)| k.clone()).filter(|k| jour.is_empty() || k.as_str() <= jour).collect();
    ks.sort();
    ks
}

/// `(self-table of conventions).get(idcc) or {}`.
fn regime_de(ent: &ChampsEntreprise) -> Resultat<Value> {
    let t = prevoyance::table()?;
    let idcc = ent.idcc.texte();
    let reg = g(g(&t, "conventions"), idcc);
    Ok(if vrai(reg) { reg.clone() } else { Value::Object(Map::new()) })
}

fn sans_mutuelle(bar: Value) -> Resultat<Value> {
    let mut applique = en_map(bar);
    let rubriques: Vec<Value> = liste(exiger_m(&applique, "rubriques_bulletin")?)
        .iter()
        .filter(|r| g(r, "id") != "mutuelle")
        .cloned()
        .collect();
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// Pose la cotisation de la mutuelle sur sa rubrique.
fn poser_mutuelle(
    bar: Value,
    base: Option<&Value>,
    total: Decimal,
    pat: Decimal,
    libelle: Option<&str>,
) -> Resultat<Value> {
    let mut applique = en_map(bar);
    let mut rubriques = Vec::new();
    for r in liste(exiger_m(&applique, "rubriques_bulletin")?) {
        if exiger(r, "id")? == "mutuelle" {
            let mut r = copie(r);
            if let Some(b) = base {
                r.insert("base".into(), b.clone());
            }
            r.insert("taux_pat".into(), nd(rond(mul(total, pat), 4)));
            r.insert("taux_sal".into(), nd(rond(sub(total, mul(total, pat)), 4)));
            if let Some(l) = libelle {
                r.insert("libelle".into(), texte(l));
            }
            rubriques.push(Value::Object(r));
        } else {
            rubriques.push(r.clone());
        }
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// Ce que la convention fait de la ligne mutuelle.
enum Mutuelle {
    Sans,
    Pose { base: Value, total: Decimal, pat: Decimal },
}

/// Le champ de la fiche entreprise qui dit ce que le contrat de sante de la compagnie fait d'un mois entierement suspendu
/// sans indemnisation, hors de la 3090 (`logiciel/donnees/champs-base.json`, `atelier/docs/ARCHITECTURE.md`) : « suspendue » (les
/// garanties sont suspendues, rien sur la paie), « maintenue » (garanties et cotisations maintenues sur la paie).
pub const SANTE_SUSPENSION: &str = "sante_suspension_non_indemnisee";
pub const SANTE_SUSPENSION_SUSPENDUE: &str = "suspendue";
pub const SANTE_SUSPENSION_MAINTENUE: &str = "maintenue";

/// La garantie sante est-elle sans objet ce mois-la pour la fiche `sal` que la paie lit : une dispense
/// (`sante_dispense`), ou un mois entierement suspendu sans indemnisation (`SalarieDuContrat::sante_suspendue`) : le bareme pose alors la
/// paie sans mutuelle (`bareme_sante`), et la DSN ne rattache pas le salarie a l'adhesion sante ce mois-la
/// (`dsn::affiliations_du_permanent`), comme pour une dispense.
pub fn sans_sante(sal: &SalarieDuContrat) -> bool {
    !sal.sante_dispense.texte().is_empty() || sal.sante_suspendue.as_deref().is_some_and(|s| !s.is_empty())
}

/// La complementaire sante que la CONVENTION fixe, quand elle en fixe une ;
/// `None` si la convention n'a pas de section `sante`.
fn sante_conventionnelle(ent: &ChampsEntreprise, sal: &SalarieDuContrat, fin: Option<&Jour>) -> Resultat<Option<Mutuelle>> {
    let reg = regime_de(ent)?;
    let sante = g(&reg, "sante");
    if !vrai(sante) {
        return Ok(None);
    }
    if sans_sante(sal) {
        return Ok(Some(Mutuelle::Sans));
    }
    let jour = jour_texte(fin);
    let taux = exiger(sante, "taux")?;
    let dates = dates_jusqu_a(taux, &jour);
    let dernier = match dates.last() {
        Some(x) => x,
        None => {
            return Err(format!(
                "Complementaire sante des permanents, convention {} : aucun taux \
en vigueur au {} dans le catalogue de branche.",
                ent.idcc.texte(),
                jour
            ))
        }
    };
    let mut total = dec(&taux[dernier.as_str()])?;
    let local = g(sante, "taux_alsace_moselle");
    if vrai(local) && alsace_moselle(ent, sal) {
        let dates_l = dates_jusqu_a(local, &jour);
        if let Some(dl) = dates_l.last() {
            total = dec(&local[dl.as_str()])?;
        }
    }
    let choix = ent.sante_permanents.dite().and_then(|s| s.part_patronale.dite());
    let defaut_min = Value::from(50);
    let minimum_v = g_def(sante, "part_patronale_minimum", &defaut_min);
    let minimum = dec(minimum_v)?;
    let part = choix.map_or(minimum, |c| c.decimal());
    if part < minimum || part > Decimal::ONE_HUNDRED {
        return Err(format!(
            "Fiche Entreprise, « Santé des permanents », part employeur {} % : \
la convention {} met au moins {} % de la cotisation sante a la \
charge de l'employeur (XII.2.1.11), et pas plus de 100.",
            choix.map(|c| en_texte(c.valeur())).unwrap_or_default(),
            ent.idcc.texte(),
            en_texte(minimum_v)
        ));
    }
    let defaut_base = texte("pss_periode");
    let base = g_def(sante, "base", &defaut_base);
    Ok(Some(Mutuelle::Pose { base: base.clone(), total, pat: div(part, Decimal::ONE_HUNDRED) }))
}

/// La cotisation globale minimale des frais de sante d'un permanent (% du
/// PMSS, salarie seul) que la convention impose a cette date, et d'ou elle
/// vient -- ou `(None, "")` quand la convention n'en fixe pas.
pub fn minimum_sante(ent: &ChampsEntreprise, jour: &str) -> Resultat<(Option<Decimal>, String)> {
    let reg = regime_de(ent)?;
    let m = g(&reg, "sante_minimum");
    if !vrai(m) {
        return Ok((None, String::new()));
    }
    let annexe = ent.annexe_convention.code().to_string();
    let (table, qui) = match g(m, "annexes").get(&annexe) {
        Some(t) => (t, format!("annexe {}", annexe)),
        None => (exiger(m, "socle")?, "socle".to_string()),
    };
    let dates = dates_jusqu_a(table, jour);
    let dernier = match dates.last() {
        Some(x) => x.clone(),
        None => return Ok((None, String::new())),
    };
    let car = |a: usize, b: usize| -> String { dernier.chars().skip(a).take(b - a).collect() };
    let origine = format!("{}, depuis le {}/{}/{}", qui, car(8, 10), car(5, 7), car(0, 4));
    Ok((Some(dec(&table[dernier.as_str()])?), origine))
}

/// Le niveau souscrit cotise-t-il au moins le minimum de la convention ?
fn controler_minimum_sante(ent: &ChampsEntreprise, sal: &SalarieDuContrat, niveau: &str, taux: &Value, fin: Option<&Jour>) -> Resultat<()> {
    let (mini, origine) = minimum_sante(ent, &jour_texte(fin))?;
    let mini = match mini {
        Some(m) => m,
        None => return Ok(()),
    };
    let assure = dec(exiger(taux, "assure")?)?;
    if assure < mini {
        return Err(socle::echec::avec(format!("{} : la complémentaire santé de son niveau est sous le minimum de la convention : vérifier le niveau souscrit sur sa fiche.", nom_du_salarie(sal, "ce salarié")), serde_json::json!({"cle": "sante_svp.niveau", "detail": format!(
            "{} : complementaire sante au niveau {}, {} % du PMSS pour le \
salarie seul -- SOUS le minimum de la convention, {} % ({} ; \
accord du 19/07/2024, art. 13).\n  \
Soit le niveau souscrit est plus haut (le lire sur le bulletin \
d'adhesion, `sante_svp.niveau`), soit les taux des niveaux ont \
change et sont a relire sur la fiche de parametrage Audiens de \
l'annee (bareme, rubrique `mutuelle`, `releve_svp_sante`). Rien \
n'est calcule.",
            nom_du_salarie(sal, "ce salarie"),
            niveau,
            deux_virgule(flt(assure)),
            deux_virgule(flt(mini)),
            origine
        )})));
    }
    Ok(())
}

/// D'OU VIENT LA COMPLEMENTAIRE SANTE D'UN SALARIE AU MOIS sous la convention de la fiche entreprise `ent`, quand ce
/// n'est pas un niveau dit sur sa fiche, dans l'ordre ou `bareme_sante` les lit : le regime de la branche
/// (`sante_conventionnelle`) ou le contrat de la compagnie. `None` : la 3090, dont le niveau se lit sur le bulletin
/// d'adhesion du salarie (`sante_svp`), ou une compagnie qui n'a pas dit sa convention.
pub fn sante_hors_fiche(ent: &ChampsEntreprise) -> Resultat<Option<String>> {
    let idcc = ent.idcc.texte();
    if vrai(g(&regime_de(ent)?, "sante")) {
        return Ok(Some(format!(
            "Sa complémentaire santé est le régime de la convention {} : le taux est celui de la branche, et la part de l'employeur se dit dans Configuration › Entreprise, « Santé des permanents ». Une dispense se dit ci-dessous.",
            idcc
        )));
    }
    if idcc.is_empty() || idcc == "3090" {
        return Ok(None);
    }
    Ok(Some(format!(
        "Sa complémentaire santé est le contrat de la compagnie (la convention {} n'a pas de régime de santé à elle) : son taux et la part de l'employeur se disent dans Configuration › Entreprise, « Santé des permanents ». Une dispense se dit ci-dessous.",
        idcc
    )))
}

/// La complementaire sante d'un permanent, selon SON contrat a lui.
pub fn bareme_sante(bar: Value, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>, fin: Option<&Jour>) -> Resultat<Value> {
    if !mensuelle(nat) {
        return Ok(bar);
    }
    match sante_conventionnelle(ent, sal, fin)? {
        Some(Mutuelle::Sans) => return sans_mutuelle(bar),
        Some(Mutuelle::Pose { base, total, pat }) => {
            return poser_mutuelle(bar, Some(&base), total, pat, Some("Complémentaire santé conventionnelle"))
        }
        None => {}
    }
    let idcc = ent.idcc.texte();
    if !idcc.is_empty() && idcc != "3090" {
        // Une convention sans regime de sante a elle : le contrat de l'entreprise.
        let choix = ent.sante_permanents.dite();
        let Some(taux_contrat) = choix.and_then(|c| c.taux_contrat.dite()) else {
            return Err(format!(
                "{} : sa complémentaire santé ne se calcule pas encore. Un salarié au mois y est affilié \
d'office (article L911-7 du code de la sécurité sociale), et la convention {} n'a pas de régime de santé à elle : \
c'est le contrat de la compagnie qui fixe la cotisation. À dire dans Configuration › Entreprise, « Santé des \
permanents » : le taux du contrat (en % du plafond mensuel, pour le salarié seul) et la part de l'employeur.",
                nom_du_salarie(sal, "ce salarie"),
                idcc
            ));
        };
        if sans_sante(sal) {
            return sans_mutuelle(bar);
        }
        let total = taux_contrat.decimal();
        let pp = choix.and_then(|c| c.part_patronale.dite());
        let part = pp.map_or(Decimal::from(50), |m| m.decimal());
        if part < Decimal::from(50) || part > Decimal::ONE_HUNDRED {
            return Err(format!(
                "Fiche Entreprise, « Santé des permanents », part employeur {} % : \
l'employeur en paie au moins la moitie (art. L911-7 du code de \
la Securite sociale), et pas plus de 100.",
                pp.map_or("50".to_string(), |m| en_texte(m.valeur()))
            ));
        }
        let pat = div(part, Decimal::ONE_HUNDRED);
        return poser_mutuelle(bar, Some(&texte("pss_periode")), total, pat, Some("Complémentaire santé (contrat de l'entreprise)"));
    }
    // La dispense d'un permanent 3090 : l'accord du 19/07/2024 (art. 9.1, 9.2,
    // `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`) rend l'adhesion obligatoire
    // « sous reserve des cas de dispenses facultatives visees a l'article 9.2.1 et des cas de dispenses de droit visees
    // a l'article 9.2.2 » ; le salarie dispense n'adhere pas au contrat collectif : pas de mutuelle, et la DSN ne le
    // rattache pas a la sante (`dsn::affiliations_du_permanent`). Ses bornes sont deja lues
    // (`calcul::dispense_de_la_periode`). Un mois entierement suspendu sans indemnisation suspend les garanties (art. 4.2 :
    // `calcul::sante_de_la_suspension`) : pas de mutuelle non plus (`calcul::sans_sante`).
    if sans_sante(sal) {
        return sans_mutuelle(bar);
    }
    let sec = match &nat.and_then(|n| n.bareme.clone()) {
        Some(b) => g(g(&bar, "par_nature"), b),
        None => &NUL,
    };
    let releve = liste(g(sec, "rubriques_ajoutees"))
        .iter()
        .find(|r| g(r, "id") == "mutuelle")
        .map(|r| g(r, "releve_svp_sante"))
        .unwrap_or(&NUL);
    if !vrai(releve) {
        return Ok(bar);
    }
    let choix = sal.sante_svp.dite();
    let niveau = choix.map_or("", |c| c.niveau.texte()).to_string();
    let taux = match exiger(releve, "niveaux")?.get(&niveau) {
        Some(t) => t,
        None => {
            return Err(format!(
                "{} : sa complémentaire santé n'est pas encore dite. Un salarié au mois y est affilié \
d'office (article L911-7 du code de la sécurité sociale), et son niveau se lit sur son bulletin d'adhésion. \
À choisir sur sa fiche, Configuration › Salariés, « Complémentaire santé » : le niveau, ou sa dispense s'il en a une.",
                nom_du_salarie(sal, "ce salarie")
            ))
        }
    };
    controler_minimum_sante(ent, sal, &niveau, taux, fin)?;
    let mut total = dec(exiger(taux, "assure")?)?;
    if choix.is_some_and(|c| c.conjoint.oui()) {
        total = add(total, dec(exiger(taux, "conjoint")?)?);
    }
    total = add(total, mul(dec(exiger(taux, "enfant")?)?, Decimal::from(choix.and_then(|c| c.enfants.dite()).map_or(0, |m| entier(m.valeur())))));
    // Au moins la moitie a la charge de l'employeur (L911-7).
    let defaut = Value::from(50);
    let part_pat = div(dec(g_def(releve, "part_employeur", &defaut))?, Decimal::ONE_HUNDRED);
    let composant = txt(g(g(releve, "composant_79"), "type"));
    let bar = poser_mutuelle(bar, None, total, part_pat, None)?;
    composant_79(bar, "mutuelle", &composant)
}

/// Le type de composant de base assujettie (S21.G00.79.001) que la base 31 de la ligne `id` declare, quand la source de
/// son taux le dit (`composant_79` sur la ligne ; `dsn::cotisations` le lit) : sans lui, la DSN garde la tranche A.
fn composant_79(bar: Value, id: &str, type_79: &str) -> Resultat<Value> {
    if type_79.is_empty() {
        return Ok(bar);
    }
    let mut applique = en_map(bar);
    let mut rubriques = Vec::new();
    for r in liste(exiger_m(&applique, "rubriques_bulletin")?) {
        if g(r, "id") == id {
            let mut r = copie(r);
            r.insert("composant_79".into(), texte(type_79));
            rubriques.push(Value::Object(r));
        } else {
            rubriques.push(r.clone());
        }
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// LE TAUX AT/MP DU BAREME, RESOLU : celui que la paie applique (`taux_atmp_applique` : le dit, ou celui de l'arrete pour
/// un risque « TC ») s'ecrit dans `taux_atmp.valeur`, que la DSN declare (S21.G00.40.043) et que ses CTP portent ; `None`
/// quand il ne se resout pas (`bareme_atmp` dit alors pourquoi). La compagnie et chacun de ses etablissements
/// (`surcharger`) passent par ici.
pub fn taux_atmp_resolu(mut bar: Value, ent: &ChampsEntreprise) -> (Value, Option<Decimal>) {
    let alsace = vrai(g(g(&bar, "taux_atmp"), "_alsace_moselle"))
        || (g(&bar, "_etablissement").is_null() && en_alsace_moselle(ent.code_postal.texte()));
    let applique = taux_atmp_applique(&bar, g(&bar, "taux_atmp"), alsace);
    if let (Some(t), Some(o)) = (applique, bar.get_mut("taux_atmp").and_then(Value::as_object_mut)) {
        if o.get("valeur").map_or(true, Value::is_null) {
            o.insert("valeur".into(), nd(t));
        }
    }
    (bar, applique)
}

/// Le taux AT/MP : un seul endroit ou l'ecrire, les rubriques s'en deduisent. Le taux que la paie applique
/// (`taux_atmp_applique` : le dit, ou celui de l'arrete pour un risque « TC ») s'ecrit dans `taux_atmp.valeur`, que la
/// DSN declare (S21.G00.40.043).
pub fn bareme_atmp(bar: Value, ent: &ChampsEntreprise) -> Resultat<Value> {
    let (bar, applique) = taux_atmp_resolu(bar, ent);
    let mut rubriques = Vec::new();
    let mut vu = false;
    for r in liste(g(&bar, "rubriques_bulletin")) {
        let facteur = g(r, "facteur_atmp");
        if facteur.is_null() {
            rubriques.push(r.clone());
            continue;
        }
        vu = true;
        let Some(taux) = applique else {
            let a = annee(&bar);
            let sans_notification = g(g(&bar, "taux_atmp"), "code_risque").as_str() == Some(SANS_NOTIFICATION);
            // Le taux d'une annee que la compagnie n'a pas dit, et que celui d'avant ne donne pas (`taux_atmp_reporte`).
            if let Some(pourquoi) = g(g(&bar, "taux_atmp"), "_manque").as_str() {
                return Err(pourquoi.to_string());
            }
            return Err(if sans_notification {
                format!(
                    "Le taux AT/MP {} manque : le code risque est 999ZZ (pas encore de notification de la CARSAT), que la DSN déclare sans taux, et aucun texte ne dit quel taux le bulletin cotise en attendant. Le code et le taux notifiés se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) », dès que la notification arrive (compte AT/MP sur net-entreprises).",
                    a
                )
            } else {
                format!(
                    "Le taux AT/MP {} manque. Il se lit sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisit avec le code risque dans Configuration › Entreprise, « Accidents du travail (AT/MP) ». Il est propre à l’établissement : le moteur ne le devine pas.",
                    a
                )
            });
        };
        let mut r = copie(r);
        r.insert("taux_pat".into(), nd(rond(mul(taux, dec(facteur)?), 3)));
        rubriques.push(Value::Object(r));
    }
    if !vu {
        return Ok(bar);
    }
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// La prevoyance d'un permanent : la CONVENTION la fixe, pas le bareme.
pub fn bareme_prevoyance(bar: Value, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>, fin: Option<&Jour>) -> Resultat<Value> {
    if !mensuelle(nat) {
        return Ok(bar);
    }
    let cot = prevoyance::cotisation(sal, est_cadre(sal), fin, ent)?;
    let mut rubriques: Vec<Value> = Vec::new();
    let mut posee = false;
    let t1 = g(&cot, "t1");
    let t2 = g(&cot, "t2");
    for r in liste(exiger(&bar, "rubriques_bulletin")?) {
        if exiger(r, "id")? != "prevoyance" {
            rubriques.push(r.clone());
            continue;
        }
        let mut rr = copie(r);
        rr.insert("taux_pat".into(), g(t1, "taux_pat").clone());
        rr.insert("taux_sal".into(), g(t1, "taux_sal").clone());
        let base = g(t1, "base");
        rr.insert("base".into(), if vrai(base) { base.clone() } else { texte("t1_mensuelle") });
        let famille = g(r, "famille");
        let famille = if vrai(famille) { famille.clone() } else { texte("SANTÉ") };
        rubriques.push(Value::Object(rr));
        posee = true;
        if vrai(t2) {
            let mut x = Map::new();
            x.insert("id".into(), texte("prevoyance_t2"));
            x.insert("famille".into(), famille);
            x.insert(
                "libelle".into(),
                texte(if vrai(g(t2, "obligatoire")) {
                    "Prévoyance Tranche 2"
                } else {
                    "Prévoyance Tranche 2 (extension conventionnelle)"
                }),
            );
            let base = g(t2, "base");
            x.insert("base".into(), if vrai(base) { base.clone() } else { texte("t2_conventionnelle") });
            x.insert("taux_pat".into(), g(t2, "taux_pat").clone());
            x.insert("taux_sal".into(), g(t2, "taux_sal").clone());
            rubriques.push(Value::Object(x));
        }
    }
    if vrai(g(&cot, "sans_regime")) {
        // Une convention sans regime pour ses non cadres : pas de ligne.
        rubriques.retain(|r| g(r, "id") != "prevoyance");
    }
    if !posee {
        return Err(socle::echec::avec(format!("Le barème {} ne porte pas la prévoyance d'un permanent : le signaler au développeur.", annee(&bar)), serde_json::json!({"detail": format!(
            "Le bareme {} n'a pas de rubrique `prevoyance` pour un permanent. \
Elle doit exister, meme sans taux : c'est elle que la convention \
remplit (voir par_nature.permanent.rubriques.prevoyance).",
            annee(&bar)
        )})));
    }
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    applique.insert("_prevoyance_appliquee".into(), cot);
    Ok(Value::Object(applique))
}

/// L'entreprise est-elle hors du dispositif CASC-SVP ? (CCN 3090, 4.22 B)
pub fn casc_exemptee(ent: &ChampsEntreprise) -> Resultat<bool> {
    if !ent.casc_svp_exemptee.oui() {
        return Ok(false);
    }
    let rep = ent.representation_personnel.code();
    if rep != "cse" && rep != "cse_adherent_fnas" {
        return Err(socle::echec::avec("La fiche entreprise dit la compagnie exemptée de la CASC-SVP, et l'exemption demande un comité social et économique : le déclarer, ou décocher l'exemption.", serde_json::json!({"cle": "casc_svp_exemptee", "detail": format!(
            "config/entreprise.json : `casc_svp_exemptee` est coche, mais \
l'exemption de la CASC-SVP ne vaut que pour une entreprise d'au \
moins 50 salaries dotee d'un CSE (CCN 3090, art. 4.22 B), et \
`representation_personnel` dit '{}'. Declarer le CSE (<< cse >>) \
ou decocher l'exemption. Rien n'est calcule.",
            if rep.is_empty() { "rien" } else { rep }
        )})));
    }
    Ok(true)
}

/// Retire la CASC-SVP (rubrique `ccn` de la 3090) d'une entreprise exemptee.
pub fn bareme_casc(bar: Value, ent: &ChampsEntreprise) -> Resultat<Value> {
    if ent.idcc.texte() != "3090" || !casc_exemptee(ent)? {
        return Ok(bar);
    }
    let rubriques: Vec<Value> =
        liste(exiger(&bar, "rubriques_bulletin")?).iter().filter(|r| g(r, "id") != "ccn").cloned().collect();
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// La representation du personnel (CCNEAC), lue sur la fiche entreprise et
/// confrontee a l'effectif : elle decide du FNAS (article III.3.1).
pub fn representation_du_personnel(ent: &EntrepriseLue) -> Resultat<String> {
    let v = ent.fiche.representation_personnel.code();
    if !v.is_empty() {
        return Ok(v.to_string());
    }
    match ent.effectif {
        // III.3.1.b : moins de onze salaries, 1,45 %.
        Some(e) if e < 11.0 => Ok("aucune".to_string()),
        Some(_) => Err(
            "Comité social et économique (CSE) : la compagnie compte au moins onze salariés, et la CCNEAC fait dépendre de son CSE \
la contribution au FNAS (article III.3.1). Le dire dans Configuration › Entreprise : pas de CSE (carence constatée), un CSE \
conventionnel (11 à 49 salariés), ou un CSE de 50 salariés et plus, adhérent au FNAS ou non. Il ne se devine pas : le CSE \
conventionnel est la règle « sauf carence »."
                .to_string(),
        ),
        // Sans effectif connu, le CSE ne se demande pas : c'est l'effectif qui manque (`contexte::Effectif::a_dire`).
        None => Err("L'effectif de la compagnie n'est pas connu : sous onze salariés, la CCNEAC fixe la contribution au FNAS \
sans CSE (article III.3.1). Le dire dans « Pour démarrer »."
            .to_string()),
    }
}

/// LA CAISSE DES CONGES SPECTACLES (`par_caisse_conges`), quand le contrat n'est pas ce que sa nature decrit : un
/// CDDU de douze mois ou plus n'y est pas declare (`non_affilie` : ni la ligne de la caisse, ni le 066, ni les
/// majorations d'assiette) ; un CDD de droit commun de moins de douze mois d'un artiste l'est
/// (`affilie` : la ligne et le 066 reprises du statut tel qu'il etait avant la nature -- `avant_la_nature` --, et les
/// majorations). Qui est declare : `caisse_conges::affilie` ; les textes : la section du bareme et
/// `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`. Sans section verifiee : refus.
pub fn bareme_caisse_conges(bar: Value, avant_la_nature: &Value, nat: &Nature, affilie: bool) -> Resultat<Value> {
    if affilie == crate::caisse_conges::par_defaut(nat) {
        return Ok(bar);
    }
    let cle = if affilie { "affilie" } else { "non_affilie" };
    let sec = g(g(&bar, "par_caisse_conges"), cle).clone();
    if !vrai(&sec) || !vrai(g(&sec, "verifie")) {
        return Err(socle::echec::avec(format!("Le barème {} ne sait pas payer ce contrat avec les Congés Spectacles : le signaler au développeur.", annee(&bar)), serde_json::json!({"detail": format!(
            "Contrat de nature '{}' {} la caisse des Conges Spectacles, et le bareme {} n'a pas de section \
par_caisse_conges['{}'] marquee \"verifie\": true. Le payer comme sa nature le decrit donnerait {}. Refus.",
            nat.cle,
            if affilie { "declare a" } else { "non declare a" },
            annee(&bar),
            cle,
            if affilie {
                "un bulletin sans cotisation a la caisse ni majoration d'assiette"
            } else {
                "une cotisation a la caisse et des assiettes majorees que l'employeur ne doit pas"
            }
        )})));
    }
    let mut out = en_map(empiler(bar, &sec)?);
    let corps = g(g(&sec, "du_corps"), "rubriques_bulletin");
    let mut rubriques: Vec<Value> = liste(exiger_m(&out, "rubriques_bulletin")?).to_vec();
    let d_avant = liste(g(avant_la_nature, "rubriques_bulletin"));
    for id in liste(corps) {
        if rang(&rubriques, id).is_some() {
            continue;
        }
        let i = rang(d_avant, id).ok_or_else(|| {
            format!("par_caisse_conges['{}'] reprend la rubrique {} du corps du bareme {}, qui ne la porte pas.", cle, en_texte(id), annee(&out_annee(&out)))
        })?;
        // A sa place : apres la rubrique qui la precede dans le corps, si elle est la ; sinon a la fin.
        let apres = d_avant[..i].iter().rev().find_map(|r| rang(&rubriques, g(r, "id")));
        insere(&mut rubriques, apres, d_avant[i].clone());
    }
    out.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    let retirees: Vec<String> = liste(g(&sec, "cotisations_dsn_retirees")).iter().map(en_texte).collect();
    let mut cots: Vec<Value> = liste(exiger_m(&out, "cotisations_dsn")?).to_vec();
    cots.retain(|c| !retirees.contains(&txt(g(c, "code81"))));
    for code in liste(g(g(&sec, "du_corps"), "cotisations_dsn")) {
        let code = en_texte(code);
        if cots.iter().any(|c| txt(g(c, "code81")) == code) {
            continue;
        }
        let reprises: Vec<&Value> = liste(g(avant_la_nature, "cotisations_dsn")).iter().filter(|c| txt(g(c, "code81")) == code).collect();
        if reprises.is_empty() {
            return Err(socle::echec::avec("Le barème de l'année est incomplet : le signaler au développeur.", serde_json::json!({"detail": format!(
                "par_caisse_conges['{}'] reprend la cotisation DSN {} du corps du bareme {}, qui ne la porte pas.",
                cle,
                code,
                annee(&out_annee(&out))
            )})));
        }
        for c in reprises {
            // Dans l'ordre des blocs 78 : avant le premier bloc de type superieur.
            let b = txt(g(c, "bloc78"));
            match cots.iter().position(|x| txt(g(x, "bloc78")) > b) {
                Some(i) => cots.insert(i, c.clone()),
                None => cots.push(c.clone()),
            }
        }
    }
    out.insert("cotisations_dsn".into(), Value::Array(cots));
    Ok(Value::Object(out))
}

/// L'annee d'un bareme en cours de construction, pour un message.
fn out_annee(m: &Map<String, Value>) -> Value {
    let mut a = Map::new();
    if let Some(v) = m.get("annee") {
        a.insert("annee".into(), v.clone());
    }
    Value::Object(a)
}

/// Le libellé d'une convention collective au catalogue : `par_convention.<idcc>.libelle` du socle livré de l'année
/// (`donnees/baremes/<an>.json`), celui que la fiche entreprise propose au choix de l'IDCC. Le bulletin et le contrat
/// l'impriment d'ici, à la date de leur période ; une convention que le socle de l'année ne porte pas n'a pas de
/// libellé, et le refus le dit (règle 1).
pub fn libelle_convention(idcc: &str, an: i32) -> Resultat<String> {
    let chemin = dossier_du_code().join("baremes").join(format!("{}.json", an));
    if !socle::fichiers::existe(&chemin) {
        return Err(format!("Aucun barème livré pour {} : le libellé de la convention {} ne se lit pas.", an, idcc));
    }
    let socle_annee = json_en_cache(&chemin)?;
    match g(g(g(&socle_annee, "par_convention"), idcc), "libelle").as_str().filter(|l| !l.trim().is_empty()) {
        Some(l) => Ok(l.to_string()),
        None => Err(socle::echec::a_la_cle(&format!("La convention collective « {} » n'est pas au catalogue du barème {} : le signaler au développeur.", idcc, an), "par_convention", "")),
    }
}

/// Les contributions de la convention collective, posees sur les rubriques
/// que le statut et la nature ont laissees.
pub fn bareme_convention(bar: Value, ent: &EntrepriseLue, nat: Option<&Nature>) -> Resultat<Value> {
    let contribs = g(&bar, "contributions_conventionnelles");
    if !vrai(contribs) {
        return Ok(bar);
    }
    let population = if mensuelle(nat) { "permanent" } else { "intermittent" };
    let mut representation: Option<String> = None;
    let mut rubriques: Vec<Value> = liste(exiger(&bar, "rubriques_bulletin")?).to_vec();
    for c in liste(contribs) {
        let mut c = sans_souligne(c);
        let taux = c.shift_remove("par_representation").unwrap_or(Value::Null);
        let apres = c.shift_remove("apres").unwrap_or(Value::Null);
        // Les taux d'une paie importee se lisent sur ses pieces (`EntrepriseLue::taux_lus`) : la contribution garde sa
        // rubrique et sa base, sans taux, et la representation du personnel ne se demande pas.
        let sans_taux = ent.taux_lus && !taux.is_null();
        if sans_taux {
            c.insert("taux_pat".into(), Value::Null);
        } else if !taux.is_null() {
            if representation.is_none() {
                representation = Some(representation_du_personnel(ent)?);
            }
            let rep = representation.clone().unwrap_or_default();
            let t = taux.get(&rep).ok_or_else(|| socle::fichiers::absent(&rep))?;
            let t = if t.is_object() { g(t, population).clone() } else { t.clone() };
            c.insert("taux_pat".into(), t);
        }
        if let Some(b) = c.get("base").filter(|b| b.is_object()) {
            let pris = b.get(population).ok_or_else(|| socle::fichiers::absent(population))?.clone();
            c.insert("base".into(), pris);
        }
        let id = c.get("id").ok_or_else(|| socle::fichiers::absent("id"))?.clone();
        let i = rang(&rubriques, &id);
        let get = |k: &str| c.get(k).unwrap_or(&NUL);
        if !sans_taux && !vrai(get("taux_pat")) && !vrai(get("taux_sal")) {
            if let Some(i) = i {
                rubriques.remove(i);
            }
            continue;
        }
        if let Some(i) = i {
            let mut r = copie(&rubriques[i]);
            for (k, v) in &c {
                r.insert(k.clone(), v.clone());
            }
            r.retain(|k, _| !k.starts_with('_'));
            rubriques[i] = Value::Object(r);
            continue;
        }
        let j = rang(&rubriques, &apres);
        insere(&mut rubriques, j, Value::Object(c));
    }
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    Ok(Value::Object(applique))
}

/// LA PILE ENTIERE, ET LE SEUL ENDROIT OU ELLE S'ECRIT : l'etablissement du
/// salarie, le statut, le taux AT/MP, la nature, la convention, la CASC, la
/// sante, la retraite en tranche 1, la categorie, la
/// prevoyance, le regime local, le versement mobilite -- dans cet ordre.
///
/// Le contrat dit s'il est declare a la caisse des Conges Spectacles (`caisse_conges::affilie`) : la caisse est un
/// etage, pose juste apres la nature (`bareme_caisse_conges`).
///
/// La pile finie se lit une fois, typee (`forme_bareme::Pile`) : le calcul et la DSN la lisent ainsi.
///
/// `ent` : la compagnie telle que la pile la lit, au jour du bareme `partage` (`Contexte::entreprise_lue`).
pub fn bareme_applique(
    partage: &Value, ent: &EntrepriseLue, sal: &SalarieDuContrat, nat: &Nature, fin_periode: Option<&Jour>, caisse_conges: bool,
    cpf_cdd: bool, majoration_chomage: bool,
) -> Resultat<Pile> {
    let fiche = &ent.fiche;
    let mut bar = if fiche.secondaires().is_empty() {
        partage.clone()
    } else {
        surcharger(partage.clone(), fiche, &etablissement_de(fiche, sal)?)?
    };
    bar = bareme_du_statut(bar, sal.statut.dite().map_or("", |s| s.as_str()))?;
    bar = bareme_atmp(bar, fiche)?;
    // Ce que la caisse reprend du statut tel qu'avant la nature : ses rubriques et ses cotisations (`bareme_caisse_conges`).
    let avant_la_nature = if caisse_conges == crate::caisse_conges::par_defaut(nat) {
        Value::Null
    } else {
        serde_json::json!({"rubriques_bulletin": g(&bar, "rubriques_bulletin"), "cotisations_dsn": g(&bar, "cotisations_dsn")})
    };
    bar = bareme_de_la_nature(bar, nat)?;
    bar = bareme_caisse_conges(bar, &avant_la_nature, nat, caisse_conges)?;
    bar = bareme_cpf_cdd(bar, nat, cpf_cdd)?;
    bar = bareme_majoration_chomage(bar, majoration_chomage)?;
    bar = bareme_taxe_apprentissage(bar, fiche, fin_periode)?;
    bar = bareme_convention(bar, ent, Some(nat))?;
    bar = bareme_casc(bar, fiche)?;
    bar = bareme_sante(bar, fiche, sal, Some(nat), fin_periode)?;
    bar = bareme_retraite_t1(bar, fiche, sal, Some(nat))?;
    bar = bareme_cadre(bar, sal, Some(nat))?;
    bar = bareme_prevoyance(bar, fiche, sal, Some(nat), fin_periode)?;
    bar = bareme_regime_local(bar, fiche, sal)?;
    crate::forme_bareme::lire(&bareme_vm(bar, sal)?)
}

/// LA COMPAGNIE QUE LA PILE LIT : sa fiche au jour du bareme, et son effectif de l'annee (`contexte::effectif_de`), que
/// la representation du personnel confronte (CCNEAC, III.3.1 : `representation_du_personnel`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntrepriseLue {
    pub fiche: ChampsEntreprise,
    pub effectif: Option<f64>,
    /// La pile d'une paie IMPORTEE : ses taux se lisent sur ses pieces, la pile ne fait que ranger ses bases
    /// (`engagement::importe`). Ce qui ne decide qu'un taux ne se demande pas : la representation du personnel, et
    /// l'effectif qui la remplace (`bareme_convention`). Une annee que l'ancien logiciel a payee n'a pas a dire
    /// l'effectif de l'annee d'avant.
    pub taux_lus: bool,
}

// ---------------------------------------------------- les etablissements

/// Le NIC de l'etablissement du salarie, de la fiche entreprise.
pub fn etablissement_de(fiche: &ChampsEntreprise, sal: &SalarieDuContrat) -> Resultat<String> {
    rattachement(&fiche.nics(), sal)
}

/// `nics` : le siege, puis les autres etablissements.
fn rattachement(nics: &[String], sal: &SalarieDuContrat) -> Resultat<String> {
    let nic = sal.etablissement.as_deref().unwrap_or("").trim().to_string();
    if nic.is_empty() || nic == nics[0] {
        return Ok(nics[0].clone());
    }
    if !nics[1..].contains(&nic) {
        return Err(format!(
            "{} : l'engagement le rattache a l'etablissement {}, que la \
fiche entreprise ne porte pas a cette date (Configuration › Entreprise › Avancé, « Autres établissements » : son ouverture ; connus : {}).",
            nom_ou(sal, "Ce salarie"),
            nic,
            nics.join(", ")
        ));
    }
    Ok(nic)
}

/// Le nom du salarie, ou `defaut` s'il n'en a pas.
fn nom_ou(sal: &ChampsSalarie, defaut: &str) -> String {
    match sal.nom.texte() {
        "" => defaut.to_string(),
        n => n.to_string(),
    }
}

/// L'entreprise vue depuis l'etablissement `nic`.
/// Un autre etablissement que le siege : son NIC, son SIRET (celui du SIREN et de ce NIC), et son adresse quand il la dit.
pub fn fiche_de_l_etablissement(fiche: &ChampsEntreprise, nic: &str) -> Resultat<ChampsEntreprise> {
    if nic.is_empty() || nic == fiche.nic_principal() {
        return Ok(fiche.clone());
    }
    let e = fiche.etablissement(nic).ok_or_else(|| format!("Etablissement inconnu : {}.", nic))?;
    let mut vue = fiche.clone();
    vue.nic = Case::Dite(e.nic.ecrite("nic")?.to_string());
    vue.siret = Case::Dite(format!("{}{}", fiche.siren.texte(), e.nic.texte()));
    // L'adresse de l'etablissement vient avec son complement, vide ou non : celui du siege n'est pas le sien.
    if e.adresse.remplie().is_some() {
        vue.adresse = e.adresse.clone();
        vue.complement_adresse = e.complement_adresse.clone();
        vue.complement_adresse_affichage = e.complement_adresse_affichage.clone();
    }
    if e.code_postal.remplie().is_some() {
        vue.code_postal = e.code_postal.clone();
    }
    if e.commune.remplie().is_some() {
        vue.commune = e.commune.clone();
    }
    for (case, de) in [
        (&mut vue.adresse_affichage, &e.adresse_affichage),
        (&mut vue.commune_affichage, &e.commune_affichage),
    ] {
        if de.remplie().is_some() {
            *case = de.clone();
        }
    }
    Ok(vue)
}

/// L'etablissement sous lequel un salarie se declare : la fiche entreprise vue depuis l'etablissement de rattachement
/// que dit SA fiche (`etablissement`, sinon le principal ; un NIC que la fiche entreprise ne porte pas refuse).
///
/// « Dans le cadre d'un contrat de travail, il s'agit de l'etablissement de rattachement administratif du salarie »
/// (S21.G00.11.001, cahier technique DSN 2026.1, p. 144, `atelier/reference/norme-dsn/texte/bloc-S21.G00.11.txt`). Le
/// NIC est une donnee identifiante : un signalement porte « les memes valeurs que celles portees dans la derniere DSN
/// mensuelle deposee » (§ 2.1.1, p. 18 ; § 2.5.1, p. 67, `chapitre-2-principes-de-constitution.txt`). La mensuelle
/// (une DSN par etablissement, qui refuse un salarie d'un autre, `generer`) et les signalements (arret, reprise, FCTU,
/// amorcage) le lisent donc au meme endroit, comme le bulletin (« la designation de l'etablissement dont depend le salarie »,
/// code du travail, R. 3243-1, 1°, `atelier/reference/sources/code-du-travail/xml/R3243-1.xml`) : `etablissement_de`,
/// puis `fiche_de_l_etablissement`.
pub fn entreprise_du_salarie(ent: &ChampsEntreprise, salarie: &SalarieDuContrat) -> Resultat<ChampsEntreprise> {
    fiche_de_l_etablissement(ent, &etablissement_de(ent, salarie)?)
}
/// Le taux AT/MP et le versement mobilite de
/// l'etablissement remplacent ceux du siege.
pub fn surcharger(bar: Value, fiche: &ChampsEntreprise, nic: &str) -> Resultat<Value> {
    let e = if !nic.is_empty() && nic != fiche.nic_principal() { fiche.etablissement(nic) } else { None };
    let e = match e {
        Some(e) => e,
        None => return Ok(bar),
    };
    let an = annee(&bar);
    let mut out = en_map(bar);
    // Le taux de l'etablissement, que `bareme_atmp` lit comme celui de la compagnie ; en Alsace-Moselle d'apres SON code postal.
    if e.taux_atmp.est_absente() {
        return Err(socle::fichiers::absent("taux_atmp"));
    }
    let mut atmp = taux_atmp_d_un_etablissement(e, &an);
    let alsace = en_alsace_moselle(e.code_postal.texte());
    if let Some(o) = atmp.as_object_mut() {
        if alsace {
            o.insert("_alsace_moselle".into(), Value::Bool(true));
        }
        // Le taux saisi d'une autre annee ne vaut pas pour celle-ci, et l'arrete ne le donne pas (un risque sans « TC ») :
        // `bareme_atmp` le refuse en le disant.
        if o.get("valeur").map_or(true, Value::is_null) && taux_atmp_applique(&Value::Object(out.clone()), &Value::Object(o.clone()), alsace).is_none() {
            o.insert(
                "_manque".into(),
                Value::String(format!(
                    "Le taux AT/MP {} de l'établissement « {} » manque : la CARSAT le notifie chaque année à cet établissement \
(compte AT/MP sur net-entreprises), et son risque {} n'a pas de taux collectif dans l'arrêté de l'année. Il se saisit dans \
Configuration › Entreprise › Avancé, « Autres établissements », à partir du 1er janvier {}.",
                    an,
                    e.libelle.remplie().filter(|l| !l.trim().is_empty()).unwrap_or(e.nic.texte()),
                    o.get("code_risque").map(txt).unwrap_or_default().trim(),
                    an
                )),
            );
        }
    }
    out.insert("taux_atmp".into(), atmp);
    // « aucun », ou un taux de zero : la commune de l'etablissement n'en a pas, ou il y emploie moins de onze salaries
    // (CGCT, L. 2333-64, I, l'effectif « employe dans chacune des zones »).
    // Le bareme porte le versement mobilite en JSON (`forme_bareme::Pile::versement_mobilite`) : celui de l'etablissement
    // s'y ecrit par sa forme.
    let vm = e.versement_mobilite.dite();
    let aucun = vm.is_some_and(|v| v.taux.dite().is_some_and(|t| t.decimal().is_zero()));
    if aucun || (matches!(e.versement_mobilite, Case::Absente | Case::Nulle) && !vrai(out.get("versement_mobilite").unwrap_or(&NUL))) {
        out.shift_remove("versement_mobilite");
    } else if let Some(v) = vm {
        out.insert("versement_mobilite".into(), serde_json::to_value(v).map_err(|e| e.to_string())?);
    }
    out.insert("_etablissement".into(), texte(e.nic.ecrite("nic")?));
    Ok(Value::Object(out))
}

// ------------------------------------------- le versement mobilite, le local

/// L'Urssaf, destinataire des cotisations individuelles du bloc 81.
const ORG_URSSAF_DEFAUT: &str = "78861779300013";

/// Le versement mobilite que la compagnie a ecrit dans sa surcouche.
#[derive(Debug, Clone, PartialEq)]
pub struct VersementMobilite {
    pub taux: Decimal,
    pub code_insee: String,
    pub autorite: String,
}

/// Le versement mobilite ecrit dans le bareme, ou `None` (rien d'ecrit, ou
/// « aucun »). Un taux sans code INSEE de cinq caracteres : refus.
pub fn versement_mobilite(bar: &Value) -> Resultat<Option<VersementMobilite>> {
    let vm = g(bar, "versement_mobilite");
    if !vm.is_object() {
        return Ok(None);
    }
    let taux = g(vm, "taux");
    let insee = txt(g(vm, "code_insee")).trim().to_string();
    if taux.is_null() || insee.chars().count() != 5 {
        return Err(socle::echec::avec("Le versement mobilité est écrit sans son taux ou sans le code de la commune de l'établissement : les lire sur le bordereau Urssaf.", serde_json::json!({"detail": format!(
            "Bareme {} : le versement mobilite est ecrit sans son taux ou sans \
le code INSEE (cinq caracteres) de la commune de l'etablissement. \
Les deux se lisent sur la deliberation de l'autorite \
organisatrice ou sur le bordereau Urssaf :\n    \
\"versement_mobilite\": {{\"taux\": 1.75, \"code_insee\": \"37261\", \
\"autorite\": \"...\", \"source\": \"...\"}}",
            annee(bar)
        )})));
    }
    Ok(Some(VersementMobilite { taux: dec(taux)?, code_insee: insee, autorite: txt(g(vm, "autorite")) }))
}

/// L'organisme du premier 075 de la DSN, sinon l'Urssaf.
fn org_urssaf(bar: &Value) -> Value {
    liste(g(bar, "cotisations_dsn"))
        .iter()
        .find(|c| g(c, "code81") == "075" && vrai(g(c, "org")))
        .map(|c| g(c, "org").clone())
        .unwrap_or_else(|| texte(ORG_URSSAF_DEFAUT))
}

fn ajouter(bar: &Value, cle: &str, v: Value) -> Vec<Value> {
    let mut l = liste(g(bar, cle)).to_vec();
    l.push(v);
    l
}

/// La ligne du versement mobilite, au bulletin et dans la DSN.
pub fn bareme_vm(bar: Value, sal: &SalarieDuContrat) -> Resultat<Value> {
    let vm = match versement_mobilite(&bar)? {
        Some(v) => v,
        None => return Ok(bar),
    };
    let artiste = sal.statut.dite().map_or(true, |s| *s == Statut::Artiste);
    let facteur = if artiste { Decimal::new(70, 2) } else { Decimal::ONE };
    let taux = rond(mul(vm.taux, facteur), 3);
    let org = org_urssaf(&bar);
    let mut r = Map::new();
    r.insert("id".into(), texte("versement_mobilite"));
    r.insert("famille".into(), texte("AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR"));
    r.insert("libelle".into(), texte("Versement mobilité"));
    // L'ASSIETTE MAJOREE (CSS L. 243-1-3, 1° ; D. 243-0-2, 1° : 11,5 %) : l'employeur affilie a une caisse de conges
    // acquitte le versement mobilite du sur les indemnites de conges par une majoration de son assiette (lettre
    // circulaire Acoss 2013-019, § 1.3) -- celle du FNAL, `majoration_conges_115`, qui vaut 1 pour un permanent.
    // Le 78.004 reste le brut. Textes : atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md.
    r.insert("base".into(), texte("brut_x115"));
    r.insert("taux_pat".into(), nd(taux));
    r.insert("taux_sal".into(), Value::Null);
    let rubriques = ajouter(&bar, "rubriques_bulletin", Value::Object(r));
    let mut c = Map::new();
    c.insert("bloc78".into(), texte("03"));
    c.insert("code81".into(), texte("081"));
    c.insert("org".into(), org);
    c.insert("base".into(), texte("brut_x115"));
    c.insert("taux".into(), nd(taux));
    c.insert("base78".into(), texte("brut"));
    c.insert("code_insee".into(), texte(&vm.code_insee));
    let cotisations = ajouter(&bar, "cotisations_dsn", Value::Object(c));
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    applique.insert("cotisations_dsn".into(), Value::Array(cotisations));
    Ok(Value::Object(applique))
}

/// La cotisation salariale maladie du regime local d'Alsace-Moselle (CSS
/// L242-13), au bulletin et dans la DSN (bloc 81, code 115).
pub fn bareme_regime_local(bar: Value, ent: &ChampsEntreprise, sal: &SalarieDuContrat) -> Resultat<Value> {
    if !alsace_moselle(ent, sal) {
        return Ok(bar);
    }
    let rl = g(&bar, "regime_local_alsace_moselle");
    if !rl.is_object() || g(rl, "taux_sal").is_null() {
        return Err(socle::echec::avec("Le barème de l'année ne porte pas le taux du régime local d'Alsace-Moselle : le signaler au développeur.", serde_json::json!({"detail": format!(
            "Bareme {} : le salarie travaille en Alsace-Moselle, et le bareme \
ne porte pas le taux du regime local (`regime_local_alsace_moselle`).",
            annee(&bar)
        )})));
    }
    let taux = nf(flottant(g(rl, "taux_sal"))?);
    let org = org_urssaf(&bar);
    let mut r = Map::new();
    r.insert("id".into(), texte("regime_local"));
    r.insert("famille".into(), texte("SANTÉ"));
    r.insert("libelle".into(), texte("Régime local Alsace-Moselle — maladie"));
    r.insert("base".into(), texte("brut"));
    r.insert("taux_pat".into(), Value::Null);
    r.insert("taux_sal".into(), taux.clone());
    let rubriques = ajouter(&bar, "rubriques_bulletin", Value::Object(r));
    let code = txt(g(rl, "code81"));
    let mut c = Map::new();
    c.insert("bloc78".into(), texte("03"));
    c.insert("code81".into(), texte(if code.is_empty() { "115" } else { &code }));
    c.insert("org".into(), org);
    c.insert("base".into(), texte("brut"));
    c.insert("taux".into(), taux);
    c.insert("base78".into(), texte("brut"));
    let cotisations = ajouter(&bar, "cotisations_dsn", Value::Object(c));
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    applique.insert("cotisations_dsn".into(), Value::Array(cotisations));
    Ok(Value::Object(applique))
}

// ------------------------------------------- la retraite en tranche 1

/// « Scène Ensemble » -> « SCENE ENSEMBLE ».
fn sigle(v: &str) -> String {
    let s: String = sans_accent(v).chars().filter(char::is_ascii).collect();
    s.to_uppercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// La couche de l'organisation patronale `nom` (sigle ou alias), ou `None`.
pub fn couche_organisation<'a>(bar: &'a Value, nom: &str) -> Option<&'a Value> {
    let cle = sigle(nom);
    if cle.is_empty() {
        return None;
    }
    for (k, c) in paires(g(bar, "par_organisation_patronale")) {
        if k.starts_with('_') || !c.is_object() {
            continue;
        }
        if cle == sigle(k) || liste(g(c, "alias")).iter().any(|a| cle == sigle(&en_texte(a))) {
            return Some(c);
        }
    }
    None
}

/// Une fraction `num/den` irreductible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fraction {
    pub num: i128,
    pub den: i128,
}

impl Fraction {
    fn nouvelle(num: i128, den: i128) -> Fraction {
        fn pgcd(a: i128, b: i128) -> i128 {
            if b == 0 {
                a.abs()
            } else {
                pgcd(b, a % b)
            }
        }
        let g = pgcd(num, den).max(1);
        Fraction { num: num / g, den: den / g }
    }

    /// `Fraction("2/3")` ou `Fraction("1")`.
    fn lire(s: &str) -> Resultat<Fraction> {
        let s = s.trim();
        let faux = || format!("fraction illisible : {}", socle::fichiers::repr(s));
        let (n, dn) = match s.split_once('/') {
            Some((a, b)) => (a.trim().parse::<i128>().map_err(|_| faux())?, b.trim().parse::<i128>().map_err(|_| faux())?),
            None => (s.parse::<i128>().map_err(|_| faux())?, 1),
        };
        if dn == 0 {
            return Err(format!("fraction de denominateur nul : {}", socle::fichiers::repr(s)));
        }
        Ok(Fraction::nouvelle(n, dn))
    }
}

/// (part patronale, part salariale) de la tranche 1, en fractions, pour la
/// convention de l'entreprise et cette population.
pub fn repartition_t1(bar: &Value, ent: &ChampsEntreprise, population: &str) -> Resultat<(Fraction, Fraction)> {
    let rep = g(bar, "retraite_t1_repartition");
    let premiere = ent.premiere_embauche.dite().map(Jour::iso).unwrap_or_default();
    let texte_rep: String = if !premiere.is_empty() && premiere.as_str() < "1999-01-01" && vrai(g(rep, "avant_1999")) {
        en_texte(&rep["avant_1999"])
    } else {
        let idcc = ent.idcc.texte().to_string();
        let t = g(g(g(rep, "par_idcc"), &idcc), population);
        if !vrai(t) {
            return Err(socle::echec::avec("Le barème de l'année ne porte pas la répartition de la retraite complémentaire de cette convention : le signaler au développeur.", serde_json::json!({"detail": format!(
                "Bareme {} : la repartition de la retraite complementaire en \
tranche 1 n'est pas relevee pour la convention {} ({}). Elle \
se lit dans le tableau n 3 de la fiche Audiens << Taux et \
repartitions >>, puis s'ecrit dans `retraite_t1_repartition` \
du socle.",
                annee(bar),
                if idcc.is_empty() { "(aucune)" } else { &idcc },
                population
            )})));
        }
        en_texte(t)
    };
    // « 60/40 » en pourcentages, « 2/3-1/3 » en fractions.
    if texte_rep.contains('-') {
        let mut p = texte_rep.split('-');
        match (p.next(), p.next(), p.next()) {
            (Some(a), Some(b), None) => Ok((Fraction::lire(a)?, Fraction::lire(b)?)),
            _ => Err(format!("repartition de la tranche 1 illisible : {}", socle::fichiers::repr(&texte_rep))),
        }
    } else {
        let (a, b) = texte_rep
            .split_once('/')
            .ok_or_else(|| format!("repartition de la tranche 1 illisible : {}", socle::fichiers::repr(&texte_rep)))?;
        let ent = |s: &str| {
            s.trim().parse::<i128>().map_err(|_| format!("nombre entier illisible : {}", socle::fichiers::repr(s)))
        };
        Ok((Fraction::nouvelle(ent(a)?, 100), Fraction::nouvelle(ent(b)?, 100)))
    }
}

/// Un taux global partage : la part salariale arrondie au centieme, la demie
/// vers le bas, la patronale en est le reste.
pub fn parts_t1(taux: &Value, sal: Fraction) -> Resultat<(Decimal, Decimal)> {
    let total = dec(taux)?;
    let part_sal = div(mul(total, Decimal::from_i128_with_scale(sal.num, 0)), Decimal::from_i128_with_scale(sal.den, 0))
        .round_dp_with_strategy(2, RoundingStrategy::MidpointTowardZero);
    Ok((total - part_sal, part_sal))
}

/// Remplace la retraite de la ligne `retraite_t1` (CEG inchangee) et, dans la
/// DSN, le 131 (regime unifie Agirc-Arrco, cahier technique 2026.1 p. 302) qui
/// porte un taux sur la meme assiette.
fn poser_t1(bar: Value, retraite_pat: Decimal, retraite_sal: Decimal) -> Resultat<Value> {
    let ceg = g(g(&bar, "retraite_t1_repartition"), "ceg");
    let ceg_pat = dec(g(ceg, "pat"))?;
    let ceg_sal = dec(g(ceg, "sal"))?;
    let pat = retraite_pat + ceg_pat;
    let sal = retraite_sal + ceg_sal;
    let mut rubriques = Vec::new();
    let mut base = Value::Null;
    for r in liste(exiger(&bar, "rubriques_bulletin")?) {
        if exiger(r, "id")? == "retraite_t1" {
            let mut r = copie(r);
            r.insert("taux_pat".into(), nd(pat));
            r.insert("taux_sal".into(), nd(sal));
            if r.contains_key("composants_pat") {
                r.insert("composants_pat".into(), Value::Array(vec![nd(retraite_pat), nd(ceg_pat)]));
                r.insert("composants_sal".into(), Value::Array(vec![nd(retraite_sal), nd(ceg_sal)]));
            }
            base = r.get("base").cloned().unwrap_or(Value::Null);
            rubriques.push(Value::Object(r));
        } else {
            rubriques.push(r.clone());
        }
    }
    let mut cots = Vec::new();
    for c in liste(g(&bar, "cotisations_dsn")) {
        if g(c, "code81") == "131" && *g(c, "base") == base && c.get("taux").is_some() && !vrai(g(c, "lignes")) {
            let mut c = copie(c);
            c.insert("taux".into(), nd(pat + sal));
            if c.contains_key("composants") {
                c.insert(
                    "composants".into(),
                    Value::Array(vec![nd(retraite_pat), nd(ceg_pat), nd(retraite_sal), nd(ceg_sal)]),
                );
            }
            cots.push(Value::Object(c));
        } else {
            cots.push(c.clone());
        }
    }
    let mut applique = en_map(bar);
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    applique.insert("cotisations_dsn".into(), Value::Array(cots));
    Ok(Value::Object(applique))
}

/// La retraite complementaire en tranche 1, quand elle n'est pas celle du
/// corps du bareme : la repartition de la branche pour un permanent, le taux
/// d'une organisation patronale pour un intermittent non cadre.
pub fn bareme_retraite_t1(bar: Value, ent: &ChampsEntreprise, sal: &SalarieDuContrat, nat: Option<&Nature>) -> Resultat<Value> {
    if !vrai(g(&bar, "retraite_t1_repartition")) {
        return Ok(bar);
    }
    if nat.map_or(false, |n| n.mensuelle()) {
        let ligne = liste(exiger(&bar, "rubriques_bulletin")?).iter().find(|r| g(r, "id") == "retraite_t1");
        match ligne {
            Some(l) if txt(g(l, "base")).starts_with("t1_mensuelle") => {}
            _ => return Ok(bar),
        }
        if ent.idcc.texte().is_empty() && !ent.premiere_embauche.est_dite() {
            return Ok(bar);
        }
        let (_, part_sal) = repartition_t1(&bar, ent, "permanent")?;
        let taux = exiger(&bar["retraite_t1_repartition"], "taux_droit_commun")?;
        let (p, s) = parts_t1(taux, part_sal)?;
        return poser_t1(bar, p, s);
    }
    let couche = match couche_organisation(&bar, ent.organisation_patronale.texte()) {
        Some(c) if g(c, "applicable") == &Value::Bool(true) => c,
        _ => return Ok(bar),
    };
    let st = sal.statut.dite().map_or("", |s| s.as_str()).to_string();
    let statut = {
        let s = if st.is_empty() { "artiste".to_string() } else { st };
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            "artiste".to_string()
        } else {
            s
        }
    };
    let taux = g(g(couche, "retraite_t1"), &statut);
    if taux.is_null() {
        return Ok(bar);
    }
    let avant = g(couche, "adhesion_avant");
    if vrai(avant) {
        let depuis = ent.organisation_patronale_depuis.dite().map(Jour::iso).unwrap_or_default();
        if depuis.is_empty() {
            return Err(format!(
                "Fiche Entreprise : l'adhesion a {} ne donne son taux de \
retraite ({} %) qu'aux adherents d'avant le {}. Renseigner \
« Adhérente depuis le » -- le logiciel ne le devine pas.",
                en_texte(g(couche, "libelle")),
                en_texte(taux),
                d(avant)?.jj_mm_aaaa()
            ));
        }
        if depuis >= en_texte(avant) {
            return Ok(bar);
        }
    }
    if est_cadre(sal) {
        return Err(socle::echec::avec("La retraite complémentaire d'un cadre intermittent ne se lit pas pour cette adhésion : la demander à Audiens.", serde_json::json!({"detail": format!(
            "{}, cadre : l'adhesion de la compagnie a {} porte la retraite \
complementaire des {}s intermittents a {} % en tranche 1, et la \
source (Audiens, tableau n 2) ne dit pas si elle vaut pour un \
cadre, dont le taux n'est deja pas celui d'un non cadre. A \
demander a Audiens, puis a ecrire dans la couche de \
`par_organisation_patronale` du socle.",
            nom_ou(sal, "Ce salarie"),
            en_texte(g(couche, "libelle")),
            statut,
            en_texte(taux)
        )})));
    }
    let (_, part_sal) = repartition_t1(&bar, ent, &statut)?;
    let (p, s) = parts_t1(taux, part_sal)?;
    poser_t1(bar, p, s)
}

/// Les cotisations DSN qu'une section de categorie AJOUTE (`cotisations_ajoutees`) : l'Apec d'un cadre, qui se declare
/// a part depuis la norme 2026.1 (« 132 - Cotisation Apec », a cote de « 131 - Cotisation regime unifie Agirc-Arrco »,
/// cahier technique 2026.1 p. 302 et 305). Chacune se place juste apres la derniere cotisation de code `apres` sur la
/// meme base 78 : elle partage son bloc 78 (`dsn::bases_et_cotisations` ouvre un bloc par base). Sans elle, refus : une
/// cotisation posee ailleurs que sous la base qu'elle suit changerait les blocs 78 sans que rien ne le dise.
fn ajouter_cotisations(cots: &mut Vec<Value>, ajouts: &[Value], nom: &str, bar: &Value) -> Resultat<()> {
    for a in ajouts {
        let mut x = copie(a);
        let apres = en_texte(&x.shift_remove("apres").unwrap_or(Value::Null));
        let base78 = g(a, "base78");
        let i = cots.iter().rposition(|c| txt(g(c, "code81")) == apres && g(c, "base78") == base78).ok_or_else(|| {
            format!(
                "Bareme {} : par_categorie['{}'].cotisations_ajoutees ajoute la cotisation {} apres une cotisation {} sur \
la base {}, que la pile de bareme de ce contrat ne declare pas. Elle ne se place pas au hasard.",
                annee(bar),
                nom,
                en_texte(g(a, "code81")),
                apres,
                en_texte(base78)
            )
        })?;
        cots.insert(i + 1, Value::Object(x));
    }
    Ok(())
}

/// Le bareme du statut, avec la surcouche cadre si la fiche le dit : la
/// section `par_categorie.cadre` (un intermittent) ou `cadre_permanent`.
pub fn bareme_cadre(bar: Value, sal: &SalarieDuContrat, nat: Option<&Nature>) -> Resultat<Value> {
    if !est_cadre(sal) {
        return Ok(bar);
    }
    let mensuelle = mensuelle(nat);
    let nom = if mensuelle { "cadre_permanent" } else { "cadre" };
    let sec = g(g(&bar, "par_categorie"), nom);
    if !vrai(sec) || !vrai(g(sec, "verifie")) {
        return Err(socle::echec::avec("Le barème de l'année ne sait pas payer un cadre : le signaler au développeur.", serde_json::json!({"detail": format!(
            "Cadre (statut categoriel 01){} : le bareme {} n'a pas de section \
par_categorie['{}'] verifiee. Prevoyance, retraite et Apec d'un \
cadre ne sont pas celles d'un non-cadre{}.",
            if mensuelle { " en contrat permanent" } else { "" },
            annee(&bar),
            nom,
            if mensuelle {
                ", et celles d'un cadre permanent ne sont pas celles d'un cadre \
intermittent : ses tranches sont mensuelles"
            } else {
                ""
            }
        )})));
    }
    // Un cadre d'un statut donne, quand la convention le distingue.
    let st = sal.statut.dite().map_or("", |s| s.as_str()).to_string();
    let sous = g(g(sec, "par_statut"), &st);
    let mut sec = sec.clone();
    if vrai(sous) {
        let mut m = copie(&sec);
        let mut rubs = copie(g(&sec, "rubriques"));
        for (rid, champs) in paires(g(sous, "rubriques")) {
            let mut r = copie(rubs.get(rid).unwrap_or(&NUL));
            for (k, v) in paires(champs) {
                r.insert(k.clone(), v.clone());
            }
            rubs.insert(rid.clone(), Value::Object(r));
        }
        m.insert("rubriques".into(), Value::Object(rubs));
        let mut ajoutees = liste(g(&sec, "rubriques_ajoutees")).to_vec();
        ajoutees.extend(liste(g(sous, "rubriques_ajoutees")).iter().cloned());
        m.insert("rubriques_ajoutees".into(), Value::Array(ajoutees));
        let mut cots_ajoutees = liste(g(&sec, "cotisations_ajoutees")).to_vec();
        cots_ajoutees.extend(liste(g(sous, "cotisations_ajoutees")).iter().cloned());
        if !cots_ajoutees.is_empty() {
            m.insert("cotisations_ajoutees".into(), Value::Array(cots_ajoutees));
        }
        let mut dsn = copie(g(&sec, "cotisations_dsn"));
        for (cle, champs) in paires(g(sous, "cotisations_dsn")) {
            let mut c = copie(dsn.get(cle).unwrap_or(&NUL));
            for (k, v) in paires(champs) {
                c.insert(k.clone(), v.clone());
            }
            dsn.insert(cle.clone(), Value::Object(c));
        }
        m.insert("cotisations_dsn".into(), Value::Object(dsn));
        sec = Value::Object(m);
    }
    let mut applique = Map::new();
    let mut rubriques = Vec::new();
    for r in liste(exiger(&bar, "rubriques_bulletin")?) {
        let mut r = copie(r);
        let id = r.get("id").ok_or_else(|| socle::fichiers::absent("id"))?.clone();
        if let Some(champs) = id.as_str().and_then(|i| g(&sec, "rubriques").get(i)) {
            for (k, v) in paires(champs) {
                if !k.starts_with('_') {
                    r.insert(k.clone(), v.clone());
                }
            }
        }
        rubriques.push(Value::Object(r));
    }
    // Les ajouts se placent « apres » une rubrique -- existante ou ajoutee
    // juste avant --, dans l'ordre du bareme.
    for a in liste(g(&sec, "rubriques_ajoutees")) {
        let i = rubriques
            .iter()
            .position(|r| g(r, "id") == g(a, "apres"))
            .map_or(rubriques.len() as isize - 1, |p| p as isize);
        let mut x = copie(a);
        x.shift_remove("apres");
        rubriques.insert((i + 1) as usize, Value::Object(x));
    }
    applique.insert("rubriques_bulletin".into(), Value::Array(rubriques));
    if let Some(v) = sec.get("cotisation_prevoyance_retraite_supp") {
        applique.insert("cotisation_prevoyance_retraite_supp".into(), v.clone());
    }
    let dsn = g(&sec, "cotisations_dsn");
    let mut cots = Vec::new();
    for c in liste(exiger(&bar, "cotisations_dsn")?) {
        let mut c = copie(c);
        let code = en_texte(c.get("code81").ok_or_else(|| socle::fichiers::absent("code81"))?);
        let cle = match c.get("affectation") {
            Some(a) if vrai(a) => format!("{}:{}", code, en_texte(a)),
            _ => code,
        };
        if let Some(champs) = dsn.get(&cle) {
            for (k, v) in paires(champs) {
                if !k.starts_with('_') {
                    c.insert(k.clone(), v.clone());
                }
            }
        }
        cots.push(Value::Object(c));
    }
    ajouter_cotisations(&mut cots, liste(g(&sec, "cotisations_ajoutees")), nom, &bar)?;
    applique.insert("cotisations_dsn".into(), Value::Array(cots));
    let mut sortie = en_map(bar);
    for (k, v) in applique {
        sortie.insert(k, v);
    }
    Ok(Value::Object(sortie))
}

// ------------------------------------------------------------ la canonisation

/// La forme canonique d'une valeur : nombres en chaines
/// normalisees, cles `_` retirees sauf `_cle`.
pub fn canon(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            let x = if let Some(i) = n.as_i64() {
                Decimal::from(i)
            } else if let Some(u) = n.as_u64() {
                Decimal::from(u)
            } else {
                socle::num::dec_f64(n.as_f64().unwrap_or(0.0)).unwrap_or_default()
            };
            let s = x.normalize().to_string();
            Value::String(if s == "-0" { "0".to_string() } else { s })
        }
        Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
        Value::Object(o) => Value::Object(
            o.iter().filter(|(k, _)| !k.starts_with('_') || k.as_str() == "_cle").map(|(k, x)| (k.clone(), canon(x))).collect(),
        ),
        autre => autre.clone(),
    }
}

// ------------------------------------------------ la prevoyance de branche

/// Ce que la convention impose aux permanents, lu
/// dans `prevoyance-conventionnelle-base.json`.
mod prevoyance {
    use super::*;
    use std::sync::Arc;

    pub fn table() -> Resultat<Arc<Value>> {
        json_en_cache(&dossier_du_code().join("prevoyance-conventionnelle-base.json"))
    }

    fn pc(v: &Value) -> Resultat<String> {
        Ok(deux_virgule(flottant(v)?))
    }

    fn pcf(x: f64) -> String {
        deux_virgule(x)
    }

    /// Les dix premiers caracteres, au format ISO.
    fn date(v: &str) -> Option<Jour> {
        let s: Vec<char> = v.chars().take(10).collect();
        if s.len() != 10 || s[4] != '-' || s[7] != '-' {
            return None;
        }
        let nombre = |a: usize, b: usize| s[a..b].iter().collect::<String>().parse::<u32>().ok();
        Jour::nouveau(nombre(0, 4)? as i32, nombre(5, 7)?, nombre(8, 10)?)
    }

    /// Le regime conventionnel de cette convention collective ; refuse quand
    /// la convention n'est pas relevee ou pas confirmee.
    pub fn regime(idcc: &str) -> Resultat<Value> {
        let t = table()?;
        let idcc = idcc.trim().to_string();
        let conventions = exiger(&t, "conventions")?;
        let reg = if idcc.is_empty() { &NUL } else { g(conventions, &idcc) };
        if !reg.is_object() {
            let connues: Vec<&str> = paires(conventions).map(|(k, _)| k.as_str()).filter(|k| !k.starts_with('_')).collect();
            return Err(socle::echec::avec("La prévoyance des permanents de cette convention collective n'est pas relevée : le signaler au développeur.", serde_json::json!({"detail": format!(
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
                if idcc.is_empty() { "(non renseignee dans config/entreprise.json)" } else { &idcc },
                if connues.is_empty() { "aucune".to_string() } else { connues.join(", ") }
            )})));
        }
        if g(reg, "applicable") == &Value::Bool(false) {
            return Err(socle::echec::avec("La prévoyance des permanents de cette convention n'est pas encore vérifiée : le signaler au développeur.", serde_json::json!({"detail": format!(
                "Prevoyance des permanents, convention {} ({}) : les taux sont \
RELEVES mais pas confirmes, le moteur ne paie donc pas.\n  \
{}\n  \
Ce qui a ete lu, a verifier : {}.\n  \
Quand c'est fait : `applicable: true` dans \
donnees/prevoyance-conventionnelle-base.json.",
                idcc,
                reg.get("libelle").map(en_texte).unwrap_or_default(),
                reg.get("_a_confirmer").map(en_texte).unwrap_or_default(),
                resume(reg)?
            )})));
        }
        Ok(reg.clone())
    }

    /// Le regime tel qu'il vaut ce jour-la (`a_partir_du`).
    fn a_la_date(reg: Value, jour: Option<&Jour>) -> Value {
        let j = match jour {
            Some(j) => j,
            None => return reg,
        };
        let mut out = copie(&reg);
        for bloc in ["obligatoire", "obligatoire_cadre"] {
            let b = g(&reg, bloc);
            if !b.is_object() || !vrai(g(b, "a_partir_du")) {
                continue;
            }
            let mut b2 = copie(b);
            let mut dates: Vec<&String> = paires(g(b, "a_partir_du")).map(|(k, _)| k).filter(|k| !k.starts_with('_')).collect();
            dates.sort();
            for dt in dates {
                if date(dt).map_or(false, |x| x <= *j) {
                    mettre_a_jour(&mut b2, &g(b, "a_partir_du")[dt.as_str()]);
                }
            }
            out.insert(bloc.to_string(), Value::Object(b2));
        }
        Value::Object(out)
    }

    /// Les nombres d'un regime, en une ligne.
    fn resume(reg: &Value) -> Resultat<String> {
        let mut out = Vec::new();
        let obl = g(reg, "obligatoire");
        if g(g(reg, "beneficiaires"), "non_cadres") == &Value::Bool(false) {
            out.push("non cadres : aucun regime de branche".to_string());
        } else if !g(obl, "taux").is_null() {
            out.push(format!("non cadres {} % de la tranche 1", pc(g(obl, "taux"))?));
        }
        let oc = g(reg, "obligatoire_cadre");
        if !g(oc, "taux").is_null() {
            let mut s = format!("cadres {} % de la tranche 1", pc(g(oc, "taux"))?);
            if !g(oc, "t2").is_null() {
                s += &format!(" et {} % de la tranche 2", pc(g(oc, "t2"))?);
            }
            out.push(s);
        }
        Ok(if out.is_empty() { "(aucun taux dans l'entree)".to_string() } else { out.join(" ; ") })
    }

    fn mois_entre(depuis: &Value, jusqu_a: Option<&Jour>) -> Option<i64> {
        if !vrai(depuis) {
            return None;
        }
        let b = jusqu_a?;
        let a = date(&en_texte(depuis))?;
        Some(
            (b.an - a.an) as i64 * 12 + (b.mois as i64 - a.mois as i64) - if b.jour < a.jour { 1 } else { 0 },
        )
    }

    /// (la cotisation est-elle due, pourquoi) pour ce salarie, ce mois-la. Le seuil de la convention
    /// (`beneficiaires.ouverture_*_mois`) : sous la 3090, « due pour les non cadres, des lors que le salarie acquiert un
    /// an d'anciennete dans la branche du secteur prive du spectacle vivant ; pour les cadres, des l'embauche » (fiche
    /// produit Audiens, `atelier/reference/sources/audiens-3090-prevoyance-sante-fiche-produit-2021.txt`, l. 102-106).
    /// Ce temps dans la branche est un fait que seule la fiche dit (`prevoyance_branche_depuis`) : il se demande.
    fn ouverture(reg: &Value, sal: &SalarieDuContrat, cadre: bool, fin: Option<&Jour>) -> Resultat<(bool, String)> {
        let b = g(reg, "beneficiaires");
        let seuil = entier(g(b, if cadre { "ouverture_cadre_mois" } else { "ouverture_non_cadre_mois" }));
        if seuil <= 0 {
            return Ok((true, "due des l'embauche".to_string()));
        }
        let branche = &en_valeur(&sal.prevoyance_branche_depuis);
        if let Some(mois) = mois_entre(branche, fin) {
            return Ok((
                mois >= seuil,
                format!("{} mois dans la branche depuis le {} (seuil {})", mois, en_texte(branche), seuil),
            ));
        }
        let mois_maison = mois_entre(&en_valeur(&sal.date_anciennete), fin);
        if let Some(m) = mois_maison {
            if m >= seuil {
                return Ok((
                    true,
                    format!(
                        "{} mois d'anciennete dans l'entreprise, donc au moins autant dans la branche (seuil {})",
                        m, seuil
                    ),
                ));
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
    fn minimum_t1(reg: &Value, annexe: &str, cadre: bool) -> Resultat<(f64, String)> {
        let obl = exiger(reg, "obligatoire")?;
        let texte_reg = en_texte(exiger(reg, "texte")?);
        let brut = g(obl, "taux");
        let mut taux = if vrai(brut) { flottant(brut)? } else { 0.0 };
        let mut pourquoi = format!("{} : {} %", texte_reg, pcf(taux));
        let sup = g(g(reg, "annexes"), annexe);
        if vrai(g(sup, "taux")) && flottant(&sup["taux"])? > taux {
            taux = flottant(&sup["taux"])?;
            pourquoi = format!("annexe {} de la convention : {} %", annexe, pcf(taux));
        }
        if cadre {
            let propre = g(g(reg, "obligatoire_cadre"), "taux");
            if !propre.is_null() {
                let s = if vrai(g(sup, "taux")) { flottant(&sup["taux"])? } else { 0.0 };
                taux = flottant(propre)?.max(s);
                pourquoi = format!("{} : {} % (cadres)", texte_reg, pcf(taux));
            }
            let t = table()?;
            let plancher = flottant(exiger(exiger(&t, "obligation_cadre")?, "taux_t1")?)?;
            if plancher > taux {
                pourquoi = format!(
                    "{}, porte a {} % par l'obligation cadre (ANI du 17/11/2017)",
                    pourquoi,
                    pcf(plancher)
                );
                taux = plancher;
            }
        }
        Ok((taux, pourquoi))
    }

    fn repartir(taux: f64, part_patronale: f64) -> Map<String, Value> {
        let pat = arrondi4(taux * part_patronale / 100.0);
        let mut m = Map::new();
        m.insert("taux_pat".into(), nf(pat));
        m.insert("taux_sal".into(), nf(arrondi4(taux - pat)));
        m
    }

    fn ou_texte(v: &Value, defaut: &str) -> Value {
        if vrai(v) {
            v.clone()
        } else {
            texte(defaut)
        }
    }

    /// Ce qu'un permanent cotise en prevoyance ce mois-la :
    /// `{"t1": {taux_pat, taux_sal, base}, "t2": ..., "due", "pourquoi"}`.
    pub fn cotisation(sal: &SalarieDuContrat, cadre: bool, fin: Option<&Jour>, entreprise: &ChampsEntreprise) -> Resultat<Value> {
        let idcc = entreprise.idcc.texte();
        let reg = a_la_date(regime(idcc)?, fin);
        let ent = entreprise.prevoyance_permanents.dite();
        let choix = if ent.is_some_and(|p| p.regime.est_dite()) { "contrat" } else { "conventionnel" };
        let ent_taux = ent.and_then(|p| p.taux.dite());
        let part_du_contrat = ent_taux.and_then(|t| t.part_patronale.dite());
        let zero = || {
            let mut t1 = Map::new();
            t1.insert("taux_pat".into(), nf(0.0));
            t1.insert("taux_sal".into(), nf(0.0));
            Value::Object(t1)
        };
        let pourquoi_v = |p: &[String]| Value::Array(p.iter().map(|s| texte(s)).collect());
        if !cadre && g(g(&reg, "beneficiaires"), "non_cadres") == &Value::Bool(false) && choix != "contrat" {
            let mut m = Map::new();
            m.insert("t1".into(), zero());
            m.insert("t2".into(), Value::Null);
            m.insert("due".into(), Value::Bool(false));
            m.insert("sans_regime".into(), Value::Bool(true));
            m.insert(
                "pourquoi".into(),
                pourquoi_v(&[format!("{} : aucun regime de prevoyance pour les non cadres", en_texte(&reg["texte"]))]),
            );
            return Ok(Value::Object(m));
        }
        if vrai(g(&reg, "contrat_requis")) && choix != "contrat" {
            return Err(socle::echec::avec("La prévoyance des permanents de cette convention suit le contrat de la compagnie : poser ses taux dans la fiche entreprise, « Prévoyance des permanents ».", serde_json::json!({"detail": format!(
                "Prevoyance des permanents, convention {} : {} fixe les garanties \
et leur repartition, pas le taux -- c'est celui du contrat que \
l'entreprise a souscrit.\n  \
A poser : fiche Entreprise, « Prévoyance des permanents », \
regime « contrat », avec ses taux (t1_non_cadre, t1_cadre), \
lus sur le contrat ou l'appel de cotisations. Rien n'est calcule.",
                idcc,
                en_texte(exiger(&reg, "texte")?)
            )})));
        }
        let mut pourquoi: Vec<String> = Vec::new();
        let (due, raison) = ouverture(&reg, sal, cadre, fin)?;
        pourquoi.push(raison);
        if !due {
            let mut m = Map::new();
            m.insert("t1".into(), zero());
            m.insert("t2".into(), Value::Null);
            m.insert("due".into(), Value::Bool(false));
            m.insert("pourquoi".into(), pourquoi_v(&pourquoi));
            return Ok(Value::Object(m));
        }

        let (mini, origine) = minimum_t1(&reg, entreprise.annexe_convention.code(), cadre)?;
        let obl = exiger(&reg, "obligatoire")?;
        let oc = g(&reg, "obligatoire_cadre");
        let defaut_100 = Value::from(100);
        let mut part_pat = flottant(g_def(obl, "part_patronale", &defaut_100))?;
        if cadre && !g(oc, "part_patronale").is_null() {
            part_pat = flottant(&oc["part_patronale"])?;
        }
        if cadre && part_pat < 100.0 && g(oc, "part_patronale").is_null() {
            part_pat = 100.0;
        }
        let mut taux = mini;
        pourquoi.push(origine.clone());

        if choix == "contrat" {
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
            let propose = flottant(t1_contrat.valeur())?;
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
            if propose > mini {
                pourquoi.push(format!("contrat d'entreprise plus favorable : {} %", pcf(propose)));
            }
            taux = propose;
            if let Some(p) = part_du_contrat {
                part_pat = flottant(p.valeur())?;
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
        t1.insert("base".into(), g_def(obl, "base", &texte("t1_mensuelle")).clone());
        let mut t2: Option<Map<String, Value>> = None;

        let part_t2 = |bloc: &Value, d100: &Value| -> Resultat<f64> {
            let defaut = g_def(bloc, "part_patronale", d100);
            flottant(g_def(bloc, "t2_part_patronale", defaut))
        };
        if cadre && !g(oc, "t2").is_null() && choix != "contrat" {
            let mut m = repartir(flottant(&oc["t2"])?, part_t2(oc, &defaut_100)?);
            m.insert("base".into(), ou_texte(g(oc, "t2_base"), "t2_conventionnelle"));
            m.insert("obligatoire".into(), Value::Bool(true));
            pourquoi.push(format!("tranche 2 obligatoire (cadres) : {} %", pc(&oc["t2"])?));
            t2 = Some(m);
        } else if !g(obl, "t2").is_null() && choix != "contrat" {
            let mut m = repartir(flottant(&obl["t2"])?, part_t2(obl, &defaut_100)?);
            m.insert("base".into(), ou_texte(g(obl, "t2_base"), "t2_conventionnelle"));
            m.insert("obligatoire".into(), Value::Bool(true));
            pourquoi.push(format!("tranche 2 obligatoire : {} %", pc(&obl["t2"])?));
            t2 = Some(m);
        }
        let ext = ent.and_then(|p| p.extension_t2.dite()).map_or("aucune", |e| e.as_str());
        if ext != "aucune" && (ext == "tous" || cadre) {
            let opt = g(g(&reg, "options"), "extension_t2");
            if g(opt, ext).is_null() {
                return Err(format!(
                    "L'extension T2 << {} >> n'existe pas dans le regime de la \
convention {}.",
                    ext,
                    idcc
                ));
            }
            let part = match part_du_contrat {
                Some(p) => flottant(p.valeur())?,
                None => flottant(g_def(opt, "part_patronale", &defaut_100))?,
            };
            let mut m = repartir(flottant(&opt[ext])?, part);
            m.insert("base".into(), g_def(opt, "base", &texte("t2_conventionnelle")).clone());
            pourquoi.push(format!(
                "extension facultative T2 ({}) : {} %",
                ext,
                pc(&opt[ext])?
            ));
            t2 = Some(m);
        }
        let mut out = Map::new();
        out.insert("t1".into(), Value::Object(t1));
        out.insert("t2".into(), t2.map_or(Value::Null, Value::Object));
        out.insert("due".into(), Value::Bool(true));
        out.insert("pourquoi".into(), pourquoi_v(&pourquoi));
        Ok(Value::Object(out))
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
        if g(&cot, "due") == &Value::Bool(false) {
            return Ok(A::NonAffilie);
        }
        let reg = a_la_date(regime(entreprise.idcc.texte())?, Some(fin));
        let seuil = entier(g(g(&reg, "beneficiaires"), if cadre { "ouverture_cadre_mois" } else { "ouverture_non_cadre_mois" }));
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
}

/// L'affiliation d'un permanent a la prevoyance de sa convention collective, un mois donne (`affiliation_prevoyance`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffiliationPrevoyance {
    /// Pas affilie : la convention n'a pas de regime pour lui (non cadre sans regime de branche), ou l'anciennete qui
    /// l'ouvre n'est pas atteinte -- sa cotisation est nulle (`prevoyance::cotisation`, `due` faux).
    NonAffilie,
    /// Affilie des l'embauche.
    DesLEmbauche,
    /// Affilie depuis ce jour : l'anciennete dans la branche que la fiche date (`prevoyance_branche_depuis`) l'a ouverte.
    Le(Jour),
    /// Affilie, au plus tard depuis ce jour : la fiche ne date pas l'entree dans la branche, seulement l'anciennete dans
    /// l'entreprise -- assez pour la cotisation, pas pour dire quand l'affiliation a commence.
    AuPlusTardLe(Jour),
}

/// L'affiliation d'un permanent a la prevoyance de sa convention au `fin` de la periode, selon la regle de sa
/// cotisation : le meme regime, la meme anciennete (`bareme_prevoyance`).
pub fn affiliation_prevoyance(ent: &ChampsEntreprise, sal: &SalarieDuContrat, fin: &Jour) -> Resultat<AffiliationPrevoyance> {
    prevoyance::affiliation(sal, est_cadre(sal), fin, ent)
}

/// La somme des parts d'une base `a` a chacun des taux `taux` (en pourcent), chacune arrondie au centime : une ligne
/// dont la part se compose de plusieurs taux.
pub fn somme_pct(a: Decimal, taux: &[f64]) -> Resultat<Decimal> {
    let mut m = Decimal::new(0, 2);
    for t in taux {
        m += socle::num::pct(a, socle::num::dec_f64(*t)?);
    }
    Ok(m)
}

// ------------------------------------------------------------ les risques AT/MP de l'arrete de l'annee

/// UN RISQUE AT/MP QUE LE SOCLE DE L'ANNEE CONNAIT (`risques_atmp.codes`), tel que l'annexe 1 de l'arrete de tarification
/// de l'annee l'ecrit (`atelier/reference/sources/tarification-atmp/`, transcription des annexes l. 497-561).
#[derive(Debug, Clone, PartialEq)]
pub struct RisqueAtmp {
    /// La forme de la DSN (S21.G00.40.040), sans point : `923AD`.
    pub code: String,
    /// La forme de l'arrete, `92.3AD` ; rien pour 999ZZ.
    pub code_arrete: Option<String>,
    pub libelle: String,
    pub precisions: Option<String>,
    /// Le taux net collectif de l'annexe 1, en pourcentage ; rien pour 999ZZ.
    pub taux: Option<Decimal>,
    /// « TC » : le taux reste collectif quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete).
    pub tc: bool,
    /// Le groupe et le taux de l'annexe 3, en Alsace-Moselle (art. 4 de l'arrete ; D. 242-35, non releve).
    pub alsace_moselle: Option<(i64, Decimal)>,
}

/// Le code sans notification de la CARSAT (S21.G00.40.040, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1236-1252) : la DSN
/// interdit alors le taux (SIG-11, l. 1311-1313).
pub const SANS_NOTIFICATION: &str = "999ZZ";

/// Les risques que le socle `socle` connait (vide pour une annee sans table : son arrete n'est pas depose).
pub fn risques_atmp(socle: &Value) -> Vec<RisqueAtmp> {
    let texte = |v: &Value| v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    liste(g(g(socle, "risques_atmp"), "codes"))
        .iter()
        .filter_map(|r| {
            Some(RisqueAtmp {
                code: texte(g(r, "code"))?,
                code_arrete: texte(g(r, "code_arrete")),
                libelle: texte(g(r, "libelle")).unwrap_or_default(),
                precisions: texte(g(r, "precisions")),
                taux: g(r, "taux").as_f64().and_then(|t| Decimal::try_from(t).ok()).map(|t| t.round_dp(2)),
                tc: vrai(g(r, "tc")),
                alsace_moselle: g(g(r, "alsace_moselle"), "groupe").as_i64().zip(g(g(r, "alsace_moselle"), "taux").as_f64().and_then(|t| Decimal::try_from(t).ok()).map(|t| t.round_dp(2))),
            })
        })
        .collect()
}

/// Le risque que le socle pose a la creation d'une compagnie (`risques_atmp.par_defaut`), s'il le connait.
pub fn risque_par_defaut(socle: &Value) -> Option<RisqueAtmp> {
    let code = g(g(socle, "risques_atmp"), "par_defaut").as_str()?;
    risques_atmp(socle).into_iter().find(|r| r.code == code)
}

/// LE TAUX QUE L'ARRETE FIXE pour ce risque : le taux net collectif de l'annexe 1 quand il s'applique, c'est-a-dire pour
/// un risque « TC » quel que soit l'effectif (D. 242-6-14, I), et pour un autre sous 20 salaries (D. 242-6-2, 1°). `moins_de_20`
/// : l'effectif dit-il moins de 20 salaries (`None` : il ne le dit pas) ? Rien quand le taux est celui que la CARSAT
/// notifie, ou qu'il n'y en a pas (999ZZ). Rien en Alsace-Moselle (`alsace_moselle`) : l'annexe 3 y fixe d'autres taux
/// collectifs, sans lettres TC, en application de D. 242-35, qui n'est pas releve : ce qu'il dit de l'effectif n'est pas
/// etabli, le taux se saisit.
pub fn taux_de_l_arrete(r: &RisqueAtmp, moins_de_20: Option<bool>, alsace_moselle: bool) -> Option<Decimal> {
    if alsace_moselle {
        None
    } else if r.tc || moins_de_20 == Some(true) {
        r.taux
    } else {
        None
    }
}

/// LE TAUX AT/MP QUE LA PAIE APPLIQUE, celui de la compagnie ou celui d'un de ses etablissements (`taux_atmp` :
/// `valeur`, `code_risque`), pour l'annee du bareme `bar` : la valeur dite, sinon, pour un risque « TC » de l'annexe 1 de
/// l'arrete de l'annee (`risques_atmp` du socle), le taux collectif qu'il fixe, quel que soit l'effectif (D. 242-6-14, I ;
/// art. 5 de l'arrete, `atelier/reference/sources/tarification-atmp/`). Le taux est determine par etablissement, le
/// classement suivant l'activite qui y est exercee (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml` ; arrete du
/// 17/10/1995, art. 1, I, 1°). Rien : le taux est celui que la CARSAT notifie (un risque sans TC, 999ZZ, l'Alsace-Moselle,
/// `taux_de_l_arrete`).
pub fn taux_atmp_applique(bar: &Value, taux_atmp: &Value, alsace_moselle: bool) -> Option<Decimal> {
    let v = g(taux_atmp, "valeur");
    if !v.is_null() {
        return dec(v).ok();
    }
    if !g(taux_atmp, "_manque").is_null() {
        return None;
    }
    let code = txt(g(taux_atmp, "code_risque"));
    let r = risques_atmp(bar).into_iter().find(|r| r.code == code.trim())?;
    // Un etablissement nouvellement cree a le taux collectif, quel que soit l'effectif (`taux_atmp_d_un_etablissement`).
    let collectif = if vrai(g(taux_atmp, "_etablissement_neuf")) { Some(true) } else { None };
    taux_de_l_arrete(&r, collectif, alsace_moselle)
}

/// La derniere annee de cotisations a laquelle s'applique D. 242-6-17 dans sa version LEGIARTI000022446511 : la version
/// issue du decret n° 2026-951 s'applique aux cotisations dues a compter de 2027 (sa note,
/// `code-de-la-securite-sociale/xml/D242-6-17.xml`).
const DERNIERE_ANNEE_D242_6_17_DE_2012: i32 = 2026;

/// LE CODE ET LE TAUX D'UN AUTRE ETABLISSEMENT (`e`, une ligne de `etablissements`) POUR L'ANNEE `an`.
///
/// Le taux saisi porte l'annee pour laquelle la CARSAT l'a notifie (`annee`) et ne vaut que pour elle, comme celui de la
/// compagnie se dit dans la surcouche de chaque annee : le taux est determine par etablissement (D. 242-6-1) et « le taux
/// net notifie » l'est d'une annee sur l'autre (D. 242-6-15, `code-de-la-securite-sociale/xml-anterieur/D242-6-15.xml`, la
/// version applicable en 2026). Une autre annee garde le code, sans taux.
///
/// Sans taux saisi, « les taux nets collectifs sont applicables aux etablissements nouvellement crees durant l'annee de
/// leur creation et les deux annees civiles suivantes, quel que soit leur effectif ou celui de l'entreprise dont ils
/// relevent » (D. 242-6-17, version applicable aux cotisations jusqu'a 2026,
/// `code-de-la-securite-sociale/xml-anterieur/D242-6-17.xml`) : l'annee de son ouverture et les deux suivantes, le taux
/// de l'arrete de l'annee pour son risque, TC ou non (`_etablissement_neuf`, lu par `taux_atmp_applique`). Ce que cet
/// article excepte (un etablissement issu d'un precedent, le taux unique d'une entreprise qui l'a choisi) se dit par le taux
/// notifie, qui se saisit. A compter de 2027, la version du decret n° 2026-951 ne se lit pas ici : un risque sans TC
/// demande son taux.
///
/// Le taux s'ecrit dans le bareme, qui est en JSON (`forme_bareme::Pile::taux_atmp`).
pub fn taux_atmp_d_un_etablissement(e: &Etablissement, an: &str) -> Value {
    let mut t = serde_json::to_value(&e.taux_atmp).unwrap_or(Value::Null);
    let annee: Option<i32> = an.trim().parse().ok();
    let ouvert: Option<i32> = e.ouverture.dite().map(|j| j.an);
    if let Some(o) = t.as_object_mut() {
        let dite = o.get("annee").filter(|a| vrai(a)).map(en_texte);
        if dite.is_some_and(|a| a.trim() != an.trim()) {
            o.shift_remove("valeur");
        }
        let neuf = matches!((annee, ouvert), (Some(a), Some(y)) if a <= DERNIERE_ANNEE_D242_6_17_DE_2012 && y <= a && a <= y + 2);
        if neuf && o.get("valeur").map_or(true, Value::is_null) {
            o.insert("_etablissement_neuf".into(), Value::Bool(true));
        }
    }
    t
}

/// L'etablissement est-il dans le Haut-Rhin, le Bas-Rhin ou la Moselle (art. 4 de l'arrete) ? Lu dans les deux premiers
/// chiffres de son code postal (68, 67, 57) : une inference, le code postal suivant le numero du departement dans ces trois
/// departements.
pub fn en_alsace_moselle(code_postal: &str) -> bool {
    matches!(code_postal.trim().get(..2), Some("57" | "67" | "68"))
}

#[cfg(test)]
mod tests_risques_atmp {
    use super::*;

    /// 92.3AD est « TC » a 1,50 % en 2026 (`tarification-atmp/legifrance-arrete-tarification-atmp-2026-complet-20261029.txt`,
    /// annexe 1, l. 496-498) : son taux est celui de l'arrete quel que soit l'effectif
    /// (D. 242-6-14, I) ; 92.1CC, sans TC (l. 489-493), ne l'a que sous 20 salaries (D. 242-6-2, 1°) ; en Alsace-Moselle,
    /// l'annexe 3 vaut et le taux se saisit (art. 4).
    #[test]
    fn le_taux_d_un_risque_tc_est_celui_de_l_arrete_quel_que_soit_l_effectif() {
        let socle = socle::fichiers::lire_json(&dossier_du_code().join("baremes").join("2026.json")).unwrap();
        let r = risque_par_defaut(&socle).unwrap();
        assert_eq!((r.code.as_str(), r.code_arrete.as_deref(), r.tc), ("923AD", Some("92.3AD"), true));
        assert_eq!(taux_de_l_arrete(&r, Some(false), false), Some(Decimal::new(150, 2)));
        assert_eq!(taux_de_l_arrete(&r, None, false), Some(Decimal::new(150, 2)));
        assert_eq!(taux_de_l_arrete(&r, Some(true), true), None);
        assert_eq!(r.alsace_moselle, Some((3, Decimal::new(334, 2))));
        let gestion = risques_atmp(&socle).into_iter().find(|r| r.code == "921CC").unwrap();
        assert_eq!(taux_de_l_arrete(&gestion, Some(true), false), Some(Decimal::new(107, 2)));
        assert_eq!(taux_de_l_arrete(&gestion, Some(false), false), None);
        assert_eq!(taux_de_l_arrete(&gestion, None, false), None);
        let sans = risques_atmp(&socle).into_iter().find(|r| r.code == SANS_NOTIFICATION).unwrap();
        assert_eq!(taux_de_l_arrete(&sans, Some(true), false), None);
    }

    /// Le taux est determine par etablissement (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml`) : un autre
    /// etablissement classe en 92.3AD, « TC », a le taux collectif de l'arrete de l'annee sans le saisir (1,50 % en 2026,
    /// D. 242-6-14), lu par `bareme_atmp` comme celui de la compagnie et ecrit pour la DSN ; un taux notifie saisi vaut ;
    /// en Alsace-Moselle, le taux se saisit (art. 4 de l'arrete).
    #[test]
    fn le_taux_d_un_etablissement_en_risque_tc_est_celui_de_l_arrete() {
        let socle = socle::fichiers::lire_json(&dossier_du_code().join("baremes").join("2026.json")).unwrap();
        let mut bar = socle.clone();
        bar["nic"] = serde_json::json!("00017");
        let fiche: ChampsEntreprise = crate::fiche::lire_champs(&serde_json::json!({"nic": "00017", "etablissements": [
            {"nic": "00025", "code_postal": "44000", "taux_atmp": {"code_risque": "923AD"}},
            {"nic": "00033", "code_postal": "67000", "taux_atmp": {"code_risque": "923AD"}},
            {"nic": "00041", "code_postal": "44000", "taux_atmp": {"code_risque": "921CC", "valeur": 2.4}}]}), "fiche").unwrap();
        let taux = |nic: &str| {
            let b = taux_atmp_resolu(surcharger(bar.clone(), &fiche, nic).unwrap(), &fiche).0;
            b["taux_atmp"]["valeur"].as_f64()
        };
        assert_eq!(taux("00025"), Some(1.5));
        assert_eq!(taux("00033"), None);
        assert_eq!(taux("00041"), Some(2.4));
    }

    /// « Les taux nets collectifs sont applicables aux etablissements nouvellement crees durant l'annee de leur creation et
    /// les deux annees civiles suivantes, quel que soit leur effectif » (D. 242-6-17, version applicable aux cotisations de
    /// 2026, `code-de-la-securite-sociale/xml-anterieur/D242-6-17.xml`) : un etablissement ouvert en 2024 ou apres, classe
    /// en 92.1CC (sans TC), a en 2026 le taux collectif de l'arrete (1,07 %, `legifrance-arrete-tarification-atmp-2026-
    /// complet-20261029.txt`, annexe 1) ; ouvert en 2023, il demande son taux notifie. Le taux saisi pour une annee
    /// (D. 242-6-15) ne vaut pas pour la suivante.
    #[test]
    fn un_etablissement_neuf_a_le_taux_collectif_trois_annees() {
        let socle = socle::fichiers::lire_json(&dossier_du_code().join("baremes").join("2026.json")).unwrap();
        let mut bar = socle.clone();
        bar["nic"] = serde_json::json!("00017");
        let fiche: ChampsEntreprise = crate::fiche::lire_champs(&serde_json::json!({"nic": "00017", "etablissements": [
            {"nic": "00025", "code_postal": "44000", "ouverture": "2024-06-01", "taux_atmp": {"code_risque": "921CC"}},
            {"nic": "00033", "code_postal": "38000", "ouverture": "2023-12-01", "taux_atmp": {"code_risque": "921CC"}},
            {"nic": "00041", "code_postal": "38000", "ouverture": "2023-12-01", "taux_atmp": {"code_risque": "921CC", "valeur": 2.4, "annee": 2025}}]}), "fiche").unwrap();
        let taux = |nic: &str| {
            let b = taux_atmp_resolu(surcharger(bar.clone(), &fiche, nic).unwrap(), &fiche).0;
            b["taux_atmp"]["valeur"].as_f64()
        };
        assert_eq!(taux("00025"), Some(1.07));
        assert_eq!(taux("00033"), None);
        assert_eq!(taux("00041"), None);
    }

    /// L'annee que la compagnie n'a pas dite : 92.3AD, « TC », prend le taux de l'arrete de l'annee (1,65 % en 2025, annexe 1
    /// de `legifrance-arrete-tarification-atmp-2025-complet-20261029.txt` ; 1,50 % en 2026), quel que soit l'effectif
    /// (D. 242-6-14, I). Un taux notifie propre a la compagnie, un risque sans TC (92.1CC, D. 242-6-2) ou une annee dont
    /// l'arrete n'est pas depose demandent le taux notifie de l'annee.
    #[test]
    fn le_taux_collectif_d_un_risque_tc_se_lit_dans_l_arrete_de_chaque_annee() {
        let lire = |an: i32| socle::fichiers::lire_json(&dossier_du_code().join("baremes").join(format!("{}.json", an))).unwrap();
        let (s25, s26) = (lire(2025), lire(2026));
        let pose = |code: &str, t: f64| serde_json::json!({"code_risque": code, "valeur": t});
        let t = taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 1.65)), Some(&s25), false).unwrap().unwrap();
        assert_eq!((t["code_risque"].as_str(), dec(&t["valeur"]).unwrap()), (Some("923AD"), Decimal::new(150, 2)));
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 2.10)), Some(&s25), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("921CC", 1.04)), Some(&s25), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &pose("923AD", 1.65)), Some(&s25), true).unwrap().is_err());
        assert!(taux_atmp_reporte(2027, None, (2026, &pose("923AD", 1.50)), Some(&s26), false).unwrap().is_err());
        assert!(taux_atmp_reporte(2026, Some(&s26), (2025, &serde_json::json!({"valeur": 1.65})), Some(&s25), false).is_none());
    }
}

/// Le taux AT/MP `t` ecrit pour l'ecran : « 1,50 % ».
fn pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// Ou le code et le taux notifies se saisissent.
const OU_SAISIR_ATMP: &str = "Ils se lisent sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».";

/// LE TAUX AT/MP D'UNE ANNEE QUE LA COMPAGNIE N'A PAS DIT, d'apres celui qu'elle a dit pour une annee d'avant (`avant` :
/// `(annee, taux_atmp)` de sa surcouche). Le code risque est celui que la CARSAT a notifie a l'etablissement, et il le
/// reste. Pour un risque « TC » de l'annexe 1 de l'arrete de l'annee (`socle_an`, `risques_atmp`), le taux est collectif
/// quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete, `atelier/reference/sources/tarification-atmp/`) : c'est
/// celui de l'arrete de l'annee, et la compagnie n'a rien a redire au 1er janvier. Il ne se reprend que si celui d'avant
/// etait deja ce taux collectif (`socle_avant`) : un taux notifie propre a la compagnie ne dit rien de l'annee suivante.
/// Tout autre cas (un risque sans TC, 999ZZ, l'Alsace-Moselle, une annee dont l'arrete n'est pas depose) demande le taux
/// notifie de l'annee : `Err` dit lequel et ou le saisir. `None` : aucun code n'a ete dit avant.
pub fn taux_atmp_reporte(an: i32, socle_an: Option<&Value>, avant: (i32, &Value), socle_avant: Option<&Value>, alsace_moselle: bool) -> Option<Result<Map<String, Value>, String>> {
    let (pa, pose) = avant;
    let code = g(pose, "code_risque").as_str().map(str::trim).filter(|c| !c.is_empty())?.to_string();
    if code == SANS_NOTIFICATION {
        return Some(Err(format!(
            "Le code risque et le taux AT/MP de {} manquent : en {}, la compagnie n'avait pas encore de notification (999ZZ). {}",
            an, pa, OU_SAISIR_ATMP
        )));
    }
    let r_avant = socle_avant.and_then(|s| risques_atmp(s).into_iter().find(|r| r.code == code));
    let nom = r_avant.as_ref().and_then(|r| r.code_arrete.clone()).unwrap_or_else(|| code.clone());
    let valeur_avant = g(pose, "valeur").as_f64().and_then(|t| Decimal::try_from(t).ok()).map(|t| t.round_dp(2));
    let collectif_avant = r_avant.as_ref().and_then(|r| taux_de_l_arrete(r, None, alsace_moselle));
    let notifie = |pourquoi: String| {
        Err(format!("Le taux AT/MP de {} manque (code risque {}). {} Celui de {} se lit sur la notification de la CARSAT (compte AT/MP sur net-entreprises) et se saisit dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", an, nom, pourquoi, an))
    };
    if alsace_moselle {
        return Some(notifie("L'établissement est en Alsace-Moselle : son taux est celui que la CARSAT lui notifie chaque année.".into()));
    }
    if valeur_avant.is_none() || valeur_avant != collectif_avant {
        return Some(notifie(format!(
            "Celui de {}{} était le taux notifié à la compagnie, qui change chaque année.",
            pa,
            valeur_avant.map(|t| format!(" ({})", pourcent(t))).unwrap_or_default()
        )));
    }
    let Some(r) = socle_an.and_then(|s| risques_atmp(s).into_iter().find(|r| r.code == code)) else {
        return Some(notifie(format!(
            "Son taux est celui de l'arrêté de tarification de {}, que le logiciel n'a pas.",
            an
        )));
    };
    if !r.tc {
        return Some(notifie("Ce risque n'a un taux collectif que sous 20 salariés (code de la sécurité sociale, art. D. 242-6-2) : au-delà, c'est celui que la CARSAT notifie.".into()));
    }
    let Some(t) = taux_de_l_arrete(&r, None, false) else {
        return Some(notifie(format!("L'arrêté de tarification de {} ne donne pas son taux.", an)));
    };
    let mut m = Map::new();
    m.insert("valeur".into(), nd(t));
    m.insert("code_risque".into(), Value::String(code));
    m.insert("_reporte_de".into(), Value::from(pa));
    m.insert(
        "_source".into(),
        Value::String(format!(
            "Code risque notifié, dit pour {} ; taux collectif de {} ({}, « TC » : code de la sécurité sociale, art. D. 242-6-14), atelier/reference/sources/tarification-atmp/.",
            pa,
            socle_an.and_then(arrete_atmp).unwrap_or_else(|| format!("l'arrêté de tarification de {}", an)),
            r.code_arrete.as_deref().unwrap_or(&r.code)
        )),
    );
    Some(Ok(m))
}

/// LE TAUX AT/MP EN VIGUEUR CE JOUR-LA, tel que `taux_atmp.valeur` l'ecrit : un nombre, ou une liste datee
/// `[{depuis, valeur}]` quand une notification change le taux en cours d'annee (la valeur dont le `depuis` est le plus
/// recent au plus tard ce jour ; avant la premiere, la premiere). `None` : pas de taux.
pub fn taux_atmp_au(valeur: &Value, jour: &Jour) -> Option<Decimal> {
    match valeur {
        Value::Array(a) => {
            let mut v: Vec<(String, &Value)> = a.iter().map(|e| (en_texte(g(e, "depuis")), g(e, "valeur"))).collect();
            v.sort_by(|x, y| x.0.cmp(&y.0));
            let jour = jour.iso();
            let choisi = v.iter().rev().find(|(d, _)| *d <= jour).or(v.first())?;
            dec(choisi.1).ok()
        }
        Value::Null => None,
        v => dec(v).ok(),
    }
}

/// Le nom de l'arrete qui fixe les taux du socle (`risques_atmp.arrete`), pour le dire a l'ecran.
pub fn arrete_atmp(socle: &Value) -> Option<String> {
    g(g(socle, "risques_atmp"), "arrete").as_str().map(String::from)
}

/// Une date de la fiche telle qu'un calcul de mois la lit (`AAAA-MM-JJ`, nulle si elle n'est pas dite).
fn en_valeur(c: &Case<Jour>) -> Value {
    c.dite().map_or(Value::Null, |j| Value::String(j.iso()))
}
