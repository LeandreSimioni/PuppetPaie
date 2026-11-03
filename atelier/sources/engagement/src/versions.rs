//! LES VERSIONS D'UN ENGAGEMENT, DITES EN MOTS. Un engagement modifie apres un envoi garde la version sur laquelle l'envoi
//! est parti (`etats`) ; l'ecran de l'engagement montre alors une ligne (« Modifié le 12/11, après l'envoi du contrat »)
//! qui ouvre la frise : par version, sa date, ce qui a change depuis la precedente (« Cachets : 4 → 5 ») et ce qui est
//! parti sur elle. Un engagement jamais modifie apres un envoi n'a pas de frise.

use serde_json::{json, Map, Value};

use crate::departs::{Depart, Departs, Envoi};
use crate::etats::{ses_departs, ARefaire, Valide};
use crate::stock::{DATES, FAITS};
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;
use socle::texte::{euros_de, virgule};

/// « 14/11 » d'un jour ISO ; tel quel s'il ne se lit pas.
fn jour_court(iso: &str) -> String {
    Jour::lire(iso.get(..10).unwrap_or(iso)).map(|j| j.jj_mm()).unwrap_or_else(|_| iso.to_string())
}

/// Le jour d'une date de ligne, nue ou `{"jour", "lieu"}`.
fn jour_de(x: &Value) -> String {
    match x {
        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
        autre => en_texte(autre),
    }
}

/// Ce qui change dans les jours d'une ligne, et seulement cela : « Jours : + 16/11 », « Jours : − 14/11 »,
/// « Jours : + 16/11 · − 14/11 ». `None` si la ligne a les memes jours.
fn jours_changes(x: &Value, y: &Value) -> Option<String> {
    let js = |l: &Value| {
        let mut j: Vec<String> = l.get("dates").and_then(Value::as_array).into_iter().flatten().map(jour_de).filter(|d| !d.is_empty()).collect();
        j.sort();
        j.dedup();
        j
    };
    let (a, b) = (js(x), js(y));
    let dire = |signe: &str, xs: Vec<&String>| (!xs.is_empty()).then(|| format!("{} {}", signe, xs.iter().map(|d| jour_court(d)).collect::<Vec<_>>().join(", ")));
    let parts: Vec<String> = [dire("+", b.iter().filter(|d| !a.contains(d)).collect()), dire("−", a.iter().filter(|d| !b.contains(d)).collect())]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| format!("Jours : {}", parts.join(" · ")))
}

/// Le premier et le dernier jour d'une saisie : ses bornes dites, sinon ses jours travailles (`crate::debut_brut`,
/// `crate::dates_engagement`).
fn bornes(ctx: &Contexte, s: &Map<String, Value>) -> (String, String) {
    let e = Value::Object(s.clone());
    let fin = s.get("fin").filter(|v| vrai(v)).map(en_texte);
    (crate::debut_brut(&ctx.racine, &e), fin.unwrap_or_else(|| crate::dates_engagement(&ctx.racine, &e).iter().map(jour_de).max().unwrap_or_default()))
}

/// Le libelle d'une rubrique au catalogue, ou son code.
fn libelle(ctx: &Contexte, code: &str) -> (String, String) {
    droit::catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).map(|r| (r.libelle, r.unite)).unwrap_or_else(|_| (code.to_string(), String::new()))
}

/// « Cachets », « Heures de travail », « Services de répétition » : le premier mot du libelle au pluriel.
fn au_pluriel(libelle: &str) -> String {
    let (premier, reste) = libelle.split_once(' ').map_or((libelle, ""), |(a, b)| (a, b));
    let pluriel = if premier.ends_with(['s', 'x', 'z']) { premier.to_string() } else { format!("{}s", premier) };
    if reste.is_empty() { pluriel } else { format!("{} {}", pluriel, reste) }
}

/// Ce que dit le taux d'une rubrique : « Salaire du cachet », « Salaire de l'heure » ; le libelle pour un forfait.
fn nom_du_taux(libelle: &str, unite: &str) -> String {
    match unite {
        "cachet" => "Salaire du cachet".into(),
        "heure" => "Salaire de l'heure".into(),
        "jour" => "Salaire de la journée".into(),
        "semaine" => "Salaire de la semaine".into(),
        "service" => "Salaire du service".into(),
        _ => format!("{}, montant", libelle),
    }
}

/// Les clauses de l'engagement hors lignes et bornes : leur nom a l'ecran.
const CLAUSES: [(&str, &str); 13] = [
    ("salarie", "Salarié"),
    ("spectacle", "Spectacle"),
    ("lieu", "Lieu"),
    ("etablissement", "Établissement"),
    ("nature", "Nature du contrat"),
    ("motif_recours", "Motif de recours"),
    ("salaire_mensuel", "Salaire mensuel"),
    ("quotite_contrat", "Quotité du contrat"),
    ("modalite_temps_travail", "Temps de travail"),
    ("metier", "Métier"),
    ("emploi", "Emploi"),
    ("cadre", "Cadre"),
    ("numero", "Numéro du contrat"),
];

/// Une valeur de clause en mots.
fn en_mots(cle: &str, v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "—".into(),
        Some(Value::Bool(b)) => if *b { "oui" } else { "non" }.into(),
        Some(x) if cle == "salaire_mensuel" || cle == "taux" => euros_de(x),
        Some(x @ Value::Number(_)) => socle::num::dec(x).map(virgule).unwrap_or_else(|_| en_texte(x)),
        Some(x) => en_texte(x),
    }
}

/// CE QUI A CHANGE d'une saisie a l'autre, en mots, dans l'ordre ou l'on lit un engagement : les bornes, chaque ligne
/// (son nombre, son taux, ses jours, son lieu), puis les autres clauses ; chaque terme qui change a une date (« À partir
/// du 01/09 : Salaire mensuel : … → … ») ; et les faits de chaque mois suivant d'un contrat qui deborde.
pub fn changements(ctx: &Contexte, avant: &Value, apres: &Value) -> Vec<String> {
    let vide = Map::new();
    let (a, b) = (avant.as_object().unwrap_or(&vide), apres.as_object().unwrap_or(&vide));
    let mut out = changements_du_mois(ctx, a, b);
    let par_mois = |m: &Map<String, Value>, k: &str| m.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
    // Un terme a une date : sa derniere valeur datee au plus tard ce jour, ou celle du premier mois.
    let au = |m: &Map<String, Value>, terme: &str, jour: &str| -> Option<Value> {
        let datees = par_mois(m, DATES);
        let derniere = datees.get(terme).and_then(Value::as_array).into_iter().flatten().filter(|e| e.get("depuis").map(en_texte).is_some_and(|d| d.as_str() <= jour)).last().cloned();
        match derniere {
            Some(e) => e.get("valeur").cloned().filter(|v| !v.is_null()),
            None => m.get(terme).cloned(),
        }
    };
    let (da, db) = (par_mois(a, DATES), par_mois(b, DATES));
    let mut jours: Vec<(String, String)> = Vec::new();
    for (k, liste) in da.iter().chain(db.iter()) {
        for e in liste.as_array().into_iter().flatten() {
            jours.push((e.get("depuis").map(en_texte).unwrap_or_default(), k.clone()));
        }
    }
    jours.sort();
    jours.dedup();
    let mut dits: Vec<String> = Vec::new();
    for (jour, _) in &jours {
        if dits.contains(jour) {
            continue;
        }
        dits.push(jour.clone());
        let termes: Vec<&String> = jours.iter().filter(|(j, _)| j == jour).map(|(_, k)| k).collect();
        let tels = |m: &Map<String, Value>| -> Map<String, Value> { termes.iter().filter_map(|k| au(m, k, jour).map(|v| ((*k).clone(), v))).collect() };
        for c in changements_du_mois(ctx, &tels(a), &tels(b)) {
            out.push(format!("À partir du {} : {}", jour_court(jour), c));
        }
    }
    let (fa, fb) = (par_mois(a, FAITS), par_mois(b, FAITS));
    let mut mois: Vec<&String> = fa.keys().chain(fb.keys()).collect();
    mois.sort();
    mois.dedup();
    for m in mois {
        let (x, y) = (fa.get(m).and_then(Value::as_object).unwrap_or(&vide), fb.get(m).and_then(Value::as_object).unwrap_or(&vide));
        for c in changements_du_mois(ctx, x, y) {
            out.push(format!("En {} : {}", crate::libelle_mois(m), c));
        }
    }
    out
}

/// Un terme du contrat qui change a un jour : sa valeur convenue jusque-la (`avant`) et la nouvelle (`apres`) ; `None` :
/// le terme n'est pas dit.
#[derive(Debug, Clone, PartialEq)]
pub struct TermeChange {
    pub terme: String,
    pub jour: Jour,
    pub avant: Option<Value>,
    pub apres: Option<Value>,
}

/// LES TERMES QUI CHANGENT entre la saisie sur laquelle le contrat a ete convenu (`convenue`) et celle d'aujourd'hui
/// (`presente`), jour par jour : chaque jour ou un terme du contrat (`etats::CLAUSES` que `stock::DATES` peut dater) ne
/// vaut pas ce qui etait convenu. L'ancienne valeur est celle que le contrat disait ce jour-la, ou celle qu'un changement
/// plus tot de la meme liste vient de poser ; un changement qui ne change rien ne se dit pas.
pub fn termes_changes(convenue: &Value, presente: &Value) -> Vec<TermeChange> {
    let datees = |s: &Value, k: &str| -> Vec<(String, Option<Value>)> {
        s.get(DATES).and_then(|t| t.get(k)).and_then(Value::as_array).into_iter().flatten()
            .filter_map(|e| Some((e.get("depuis").map(en_texte)?, e.get("valeur").cloned().filter(|v| !v.is_null()))))
            .collect()
    };
    let au = |s: &Value, k: &str, jour: &str| -> Option<Value> {
        match datees(s, k).into_iter().filter(|(d, _)| d.as_str() <= jour).last() {
            Some((_, v)) => v,
            None => s.get(k).cloned().filter(|v| !v.is_null()),
        }
    };
    let pareil = |a: &Option<Value>, b: &Option<Value>| a.as_ref().map(crate::stock::canonique) == b.as_ref().map(crate::stock::canonique);
    let mut out = Vec::new();
    for k in crate::etats::CLAUSES.iter().filter(|k| crate::stock::TERMES.contains(k)) {
        let mut jours: Vec<String> = datees(convenue, k).into_iter().chain(datees(presente, k)).map(|(d, _)| d).collect();
        jours.sort();
        jours.dedup();
        let mut dernier: Option<(String, Option<Value>)> = None;
        for j in jours {
            let (c, p) = (au(convenue, k, &j), au(presente, k, &j));
            let avant = match &dernier {
                Some((dj, v)) if pareil(&au(convenue, k, dj), &c) => v.clone(),
                _ => c,
            };
            if pareil(&avant, &p) {
                continue;
            }
            let Ok(jour) = Jour::lire(j.get(..10).unwrap_or(&j)) else { continue };
            out.push(TermeChange { terme: k.to_string(), jour, avant, apres: p.clone() });
            dernier = Some((j, p));
        }
    }
    out.sort_by_key(|t| t.jour);
    out
}

/// CE QUI A CHANGE DEPUIS LA VERSION `n` de l'engagement valide `v` jusqu'a son present, en mots (`changements`) : ce
/// que dit la frise, et ce que dit le renvoi des documents partis sur la version `n`. Vide si `n` est le present ou ne se
/// lit pas.
pub fn depuis(ctx: &Contexte, v: &Valide, n: u64) -> Vec<String> {
    match (v.version_numero(n), v.objet().brut.get("valide")) {
        (Some(passee), Some(present)) if v.version() != Some(n) => {
            changements(ctx, passee.get("saisie").unwrap_or(&Value::Null), present.get("saisie").unwrap_or(&Value::Null))
        }
        _ => Vec::new(),
    }
}

fn changements_du_mois(ctx: &Contexte, a: &Map<String, Value>, b: &Map<String, Value>) -> Vec<String> {
    let mut out = Vec::new();
    let ((da, fa), (db, fb)) = (bornes(ctx, a), bornes(ctx, b));
    if da != db {
        out.push(format!("Début : {} → {}", jour_court(&da), jour_court(&db)));
    }
    if fa != fb {
        out.push(format!("Fin : {} → {}", jour_court(&fa), jour_court(&fb)));
    }
    let lignes = |m: &Map<String, Value>| m.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    let (la, lb) = (lignes(a), lignes(b));
    let code = |l: &Value| l.get("rubrique").map(en_texte).unwrap_or_default();
    // Les lignes se rapprochent par leur rubrique, dans l'ordre : la deuxieme ligne de cachets de l'une va avec la
    // deuxieme de l'autre.
    let mut vues: Vec<String> = Vec::new();
    for l in la.iter().chain(lb.iter()) {
        if !vues.contains(&code(l)) {
            vues.push(code(l));
        }
    }
    for c in vues {
        let (lib, unite) = libelle(ctx, &c);
        let xs: Vec<&Value> = la.iter().filter(|l| code(l) == c).collect();
        let ys: Vec<&Value> = lb.iter().filter(|l| code(l) == c).collect();
        for k in 0..xs.len().max(ys.len()) {
            match (xs.get(k), ys.get(k)) {
                (Some(_), None) => out.push(format!("{} : retiré", lib)),
                (None, Some(y)) => out.push(format!("{} : ajouté ({})", lib, en_mots("quantite", y.get("quantite")))),
                (Some(x), Some(y)) => {
                    let champ = |l: &Value, k: &str| l.get(k).cloned().unwrap_or(Value::Null);
                    let (qx, qy) = (champ(x, "quantite"), champ(y, "quantite"));
                    if qx.as_f64() != qy.as_f64() {
                        out.push(format!("{} : {} → {}", au_pluriel(&lib), en_mots("quantite", Some(&qx)), en_mots("quantite", Some(&qy))));
                    }
                    let (tx, ty) = (champ(x, "taux"), champ(y, "taux"));
                    if en_mots("taux", Some(&tx)) != en_mots("taux", Some(&ty)) {
                        out.push(format!("{} : {} → {}", nom_du_taux(&lib, &unite), en_mots("taux", Some(&tx)), en_mots("taux", Some(&ty))));
                    }
                    if let Some(j) = jours_changes(x, y) {
                        out.push(j);
                    }
                    let (lx, ly) = (en_mots("lieu", x.get("lieu")), en_mots("lieu", y.get("lieu")));
                    if lx != ly {
                        out.push(format!("Lieu : {} → {}", lx, ly));
                    }
                }
                (None, None) => {}
            }
        }
    }
    for (cle, nom) in CLAUSES {
        let (x, y) = (en_mots(cle, a.get(cle)), en_mots(cle, b.get(cle)));
        if x != y {
            out.push(format!("{} : {} → {}", nom, x, y));
        }
    }
    out
}

/// Un depart en mots, sans sa date : « le contrat », « l'avenant n° 1 », « la DPAE », « les documents de novembre 2026 ».
/// Une DSN se nomme par le mois de la DSN qui est partie : la correction d'un mois passe portee par la DSN d'un mois
/// suivant (`Depart::par`) est « la DSN de décembre 2026 (correction de novembre 2026) ».
pub(crate) fn quoi_du_depart(d: &Depart) -> String {
    match (d.envoi, d.mois.as_deref()) {
        (Envoi::Contrat, _) if d.avenant.is_some() => format!("l'avenant n° {}", d.avenant.as_deref().unwrap_or_default()),
        (Envoi::Contrat, _) => "le contrat".to_string(),
        (Envoi::Dpae, _) => "la DPAE".to_string(),
        (Envoi::Documents, Some(m)) => format!("les documents {}", de(&crate::libelle_mois(m))),
        (Envoi::Dsn, Some(m)) => match d.par.as_deref() {
            Some(p) => format!("la DSN {} (correction {})", de(&crate::libelle_mois(p)), de(&crate::libelle_mois(m))),
            None => format!("la DSN {}", de(&crate::libelle_mois(m))),
        },
        (Envoi::Signalement, _) => "un signalement".to_string(),
        (_, None) => "un envoi".to_string(),
    }
}

/// « du contrat », « de la DPAE », « des documents de novembre 2026 », « d'un signalement », « de l'avenant n° 1 » :
/// le complement d'« après l'envoi », élidé devant une voyelle.
fn complement(quoi: &str) -> String {
    if let Some(r) = quoi.strip_prefix("le ") {
        format!("du {}", r)
    } else if let Some(r) = quoi.strip_prefix("les ") {
        format!("des {}", r)
    } else {
        de(quoi)
    }
}

/// « le contrat, la DPAE et les documents de novembre 2026 ».
fn enumerer(xs: &[String]) -> String {
    match xs {
        [] => String::new(),
        [x] => x.clone(),
        [debut @ .., dernier] => format!("{} et {}", debut.join(", "), dernier),
    }
}

/// LA FRISE D'UN ENGAGEMENT VALIDE, s'il a ete modifie apres un envoi : `{"phrase", "versions"}`, la phrase de la ligne
/// sous la carte et, de la plus ancienne a la presente, chaque version (`version`, `le`, `changements`, `partis`).
/// `None` : aucune version passee, jamais modifie apres un envoi, ou la paie de `mois` est restee celle de la version
/// d'avant (un contrat au mois change a partir d'un mois suivant : ce mois-ci reste tel qu'il est parti).
pub fn frise(ctx: &Contexte, v: &Valide, departs: &Departs, mois: &str) -> Option<Value> {
    let passees = v.versions();
    let paie_du_mois = |x: &Value| crate::etats::paie_ecrite_de_la_version(v.objet(), x, mois).ok().flatten().and_then(|p| p.get("paie").cloned());
    let present = v.objet().brut.get("valide");
    if passees.last().zip(present).is_none_or(|(a, p)| paie_du_mois(a).is_some() && paie_du_mois(a) == paie_du_mois(p)) {
        return None;
    }
    let siens = ses_departs(ctx, v.objet(), departs);
    let present = v.objet().brut.get("valide").cloned().unwrap_or(Value::Null);
    let numero = |x: &Value| x.get("version").and_then(Value::as_u64);
    let toutes: Vec<(&Value, String)> = passees
        .iter()
        .map(|x| (x, x.get("valide_le").map(en_texte).unwrap_or_default()))
        .chain(std::iter::once((&present, v.le().to_string())))
        .collect();
    let mut versions = Vec::new();
    let mut precedente: Option<&Value> = None;
    for (x, le) in &toutes {
        let n = numero(x);
        let mut partis: Vec<String> = siens
            .iter()
            .filter(|d| Some(d.version) == n)
            .map(|d| format!("{}, le {}", quoi_du_depart(d), jour_court(&d.le)))
            .collect();
        partis.dedup();
        let changes = precedente.map_or_else(Vec::new, |p| changements(ctx, p.get("saisie").unwrap_or(&Value::Null), x.get("saisie").unwrap_or(&Value::Null)));
        // Le titre de la version et son jour, tels que la frise les lit.
        let titre_dit = format!("Version {}", n.map(|n| n.to_string()).unwrap_or_default()).trim().to_string();
        let jour_dit = format!("le {}", jour_court(le));
        versions.push(json!({"version": n, "le": le, "jour": jour_court(le), "titre_dit": titre_dit, "jour_dit": jour_dit, "changements": changes, "partis": partis}));
        precedente = Some(x);
    }
    Some(json!({"phrase": phrase_de_la_frise(ctx, v, departs, siens), "versions": versions}))
}

/// LA LIGNE SOUS LA CARTE : quand l'engagement a change, a partir de quel mois si ce n'est pas son premier, et ce qui
/// etait parti que le changement touche (`etats::a_refaire`), groupe par sorte, les mois qui se suivent en une etendue :
/// « Modifié le 20/01, à partir de septembre 2025, après l'envoi du contrat, des documents et des DSN de septembre à
/// décembre 2025 ». Le detail, envoi par envoi, est dans la frise.
fn phrase_de_la_frise(ctx: &Contexte, v: &Valide, departs: &Departs, siens: &[Depart]) -> String {
    let mut phrase = format!("Modifié le {}", jour_court(v.le()));
    if let Some(m) = premier_mois_change(v).filter(|m| m.as_str() != v.objet().premier_mois()) {
        phrase.push_str(&format!(", à partir de {}", crate::libelle_mois(&m)));
    }
    let (mut contrat, mut dpae) = (false, false);
    let (mut documents, mut dsn) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for a in crate::etats::a_refaire(ctx, v, departs) {
        match a {
            ARefaire::Avenant { .. } => contrat = true,
            ARefaire::Dpae { .. } => dpae = true,
            ARefaire::Bulletin { mois, .. } => {
                documents.insert(mois);
            }
            ARefaire::Dsn { mois, .. } => {
                dsn.insert(mois);
            }
        }
    }
    let mut quoi = Vec::new();
    // Le contrat, ou son dernier avenant : celui que le changement rend faux.
    if let Some(d) = siens.iter().filter(|d| contrat && d.envoi == Envoi::Contrat).last() {
        quoi.push(complement(&quoi_du_depart(d)));
    }
    if dpae {
        quoi.push(complement("la DPAE"));
    }
    if !documents.is_empty() && documents == dsn {
        quoi.push("des documents".to_string());
        quoi.push(format!("des DSN {}", de_mois(&documents)));
    } else {
        if !documents.is_empty() {
            quoi.push(format!("des documents {}", de_mois(&documents)));
        }
        if !dsn.is_empty() {
            quoi.push(format!("des DSN {}", de_mois(&dsn)));
        }
    }
    if !quoi.is_empty() {
        phrase.push_str(&format!(", après l'envoi {}", enumerer(&quoi)));
    }
    phrase
}

/// Le premier mois dont la paie a change au dernier changement de l'engagement valide `v` (sa derniere version passee
/// contre son present) ; `None` si aucune ne se lit autrement.
fn premier_mois_change(v: &Valide) -> Option<String> {
    let avant = v.versions().last()?;
    let present = v.objet().brut.get("valide")?;
    let paie = |x: &Value, m: &str| crate::etats::paie_ecrite_de_la_version(v.objet(), x, m).ok().flatten().and_then(|p| p.get("paie").cloned());
    crate::paies::mois_ecrits(v.objet()).ok()?.into_iter().find(|m| paie(avant, m) != paie(present, m))
}

/// « de septembre à décembre 2025 », « d'avril 2025 et de juin 2025 », « de novembre 2025 à février 2026 » : des mois
/// `AAAA-MM`, ceux qui se suivent en une etendue.
fn de_mois(mois: &std::collections::BTreeSet<String>) -> String {
    let mut etendues: Vec<(String, String)> = Vec::new();
    for m in mois {
        match etendues.last_mut() {
            Some((_, fin)) if crate::stock::mois_apres(fin).is_ok_and(|s| &s == m) => *fin = m.clone(),
            _ => etendues.push((m.clone(), m.clone())),
        }
    }
    let dites: Vec<String> = etendues
        .iter()
        .map(|(a, b)| {
            let (la, lb) = (crate::libelle_mois(a), crate::libelle_mois(b));
            let texte = if a == b {
                la
            } else if a.get(..4) == b.get(..4) {
                format!("{} à {}", la.trim_end_matches(&format!(" {}", a.get(..4).unwrap_or_default())), lb)
            } else {
                format!("{} à {}", la, lb)
            };
            de(&texte)
        })
        .collect();
    enumerer(&dites)
}

/// « de mars 2025 », « d'avril 2025 » : la preposition, elidee devant une voyelle.
fn de(texte: &str) -> String {
    if texte.starts_with(['a', 'e', 'i', 'o', 'u', 'é', 'h']) {
        format!("d'{}", texte)
    } else {
        format!("de {}", texte)
    }
}
