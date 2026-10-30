//! LES VERSIONS D'UN ENGAGEMENT, DITES EN MOTS. Un engagement modifie apres un envoi garde la version sur laquelle l'envoi
//! est parti (`etats`) ; l'ecran de l'engagement montre alors une ligne (« Modifié le 12/11, après l'envoi du contrat »)
//! qui ouvre la frise : par version, sa date, ce qui a change depuis la precedente (« Cachets : 4 → 5 ») et ce qui est
//! parti sur elle. Un engagement jamais modifie apres un envoi n'a pas de frise.

use serde_json::{json, Map, Value};

use crate::departs::{Depart, Departs, Envoi};
use crate::etats::{ses_departs, Valide};
use crate::stock::SUITE;
use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::fichiers::vrai;

/// « 14/11 » d'un jour ISO ; tel quel s'il ne se lit pas.
fn jour_court(iso: &str) -> String {
    Jour::lire(iso.get(..10).unwrap_or(iso)).map(|j| j.jj_mm()).unwrap_or_else(|_| iso.to_string())
}

/// Un montant saisi (`150`, `150.0`, `"150"`) en euros : « 150,00 € ».
fn euros(v: &Value) -> String {
    let x: Option<f64> = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.replace(',', ".").parse().ok(),
        _ => None,
    };
    x.map_or_else(|| en_texte(v), |x| format!("{:.2} €", x).replace('.', ","))
}

/// Un nombre saisi sans ses decimales inutiles : `3.0` → « 3 », `2.5` → « 2,5 ».
fn nombre(v: &Value) -> String {
    match v.as_f64() {
        Some(x) if x.fract() == 0.0 => format!("{}", x as i64),
        Some(x) => format!("{}", x).replace('.', ","),
        None => en_texte(v),
    }
}

/// Le jour d'une date de ligne, nue ou `{"jour", "lieu"}`.
fn jour_de(x: &Value) -> String {
    match x {
        Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
        autre => en_texte(autre),
    }
}

/// Les jours d'une saisie, toutes lignes confondues, tries.
fn jours(s: &Map<String, Value>) -> Vec<String> {
    let mut j: Vec<String> = s
        .get("lignes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|l| l.get("dates").and_then(Value::as_array).cloned().unwrap_or_default())
        .map(|d| jour_de(&d))
        .filter(|d| !d.is_empty())
        .collect();
    j.sort();
    j.dedup();
    j
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

/// Le premier et le dernier jour d'une saisie : ses bornes dites, sinon ses jours.
fn bornes(s: &Map<String, Value>) -> (String, String) {
    let j = jours(s);
    let dite = |k: &str| s.get(k).filter(|v| vrai(v)).map(en_texte);
    (dite("debut").unwrap_or_else(|| j.first().cloned().unwrap_or_default()), dite("fin").unwrap_or_else(|| j.last().cloned().unwrap_or_default()))
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
const CLAUSES: [(&str, &str); 12] = [
    ("salarie", "Salarié"),
    ("spectacle", "Spectacle"),
    ("lieu", "Lieu"),
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
        Some(x) if cle == "salaire_mensuel" => euros(x),
        Some(x @ Value::Number(_)) => nombre(x),
        Some(x) => en_texte(x),
    }
}

/// CE QUI A CHANGE d'une saisie a l'autre, en mots, dans l'ordre ou l'on lit un engagement : les bornes, chaque ligne
/// (son nombre, son taux, ses jours, son lieu), puis les autres clauses, et chaque mois suivant d'un contrat qui deborde.
pub fn changements(ctx: &Contexte, avant: &Value, apres: &Value) -> Vec<String> {
    let vide = Map::new();
    let (a, b) = (avant.as_object().unwrap_or(&vide), apres.as_object().unwrap_or(&vide));
    let mut out = changements_du_mois(ctx, a, b);
    let suite = |m: &Map<String, Value>| m.get(SUITE).and_then(Value::as_object).cloned().unwrap_or_default();
    let (sa, sb) = (suite(a), suite(b));
    let mut mois: Vec<&String> = sa.keys().chain(sb.keys()).collect();
    mois.sort();
    mois.dedup();
    for m in mois {
        let (x, y) = (sa.get(m).and_then(Value::as_object).unwrap_or(&vide), sb.get(m).and_then(Value::as_object).unwrap_or(&vide));
        for c in changements_du_mois(ctx, x, y) {
            out.push(format!("En {} : {}", crate::libelle_mois(m), c));
        }
    }
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
    let ((da, fa), (db, fb)) = (bornes(a), bornes(b));
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
                (None, Some(y)) => out.push(format!("{} : ajouté ({})", lib, nombre(y.get("quantite").unwrap_or(&Value::Null)))),
                (Some(x), Some(y)) => {
                    let champ = |l: &Value, k: &str| l.get(k).cloned().unwrap_or(Value::Null);
                    let (qx, qy) = (champ(x, "quantite"), champ(y, "quantite"));
                    if qx.as_f64() != qy.as_f64() {
                        out.push(format!("{} : {} → {}", au_pluriel(&lib), nombre(&qx), nombre(&qy)));
                    }
                    let (tx, ty) = (champ(x, "taux"), champ(y, "taux"));
                    if euros(&tx) != euros(&ty) {
                        out.push(format!("{} : {} → {}", nom_du_taux(&lib, &unite), euros(&tx), euros(&ty)));
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
        (Envoi::Documents, Some(m)) => format!("les documents de {}", crate::libelle_mois(m)),
        (Envoi::Dsn, Some(m)) => match d.par.as_deref() {
            Some(p) => format!("la DSN de {} (correction de {})", crate::libelle_mois(p), crate::libelle_mois(m)),
            None => format!("la DSN de {}", crate::libelle_mois(m)),
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
    } else if quoi.starts_with(['a', 'e', 'i', 'o', 'u', 'é', 'h']) {
        format!("d'{}", quoi)
    } else {
        format!("de {}", quoi)
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
    let paie_du_mois = |x: &Value| x.get("paies").and_then(|p| p.get(mois)).and_then(|p| p.get("paie")).cloned();
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
        versions.push(json!({"version": n, "le": le, "jour": jour_court(le), "changements": changes, "partis": partis}));
        precedente = Some(x);
    }
    // CE QUI ETAIT PARTI quand l'engagement a change pour la derniere fois : les envois de la version d'avant.
    let avant = numero(toutes[toutes.len() - 2].0);
    let mut quoi: Vec<String> = siens.iter().filter(|d| Some(d.version) == avant).map(|d| complement(&quoi_du_depart(d))).collect();
    quoi.dedup();
    let phrase = if quoi.is_empty() {
        format!("Modifié le {}", jour_court(v.le()))
    } else {
        format!("Modifié le {}, après l'envoi {}", jour_court(v.le()), enumerer(&quoi))
    };
    Some(json!({"phrase": phrase, "versions": versions}))
}
