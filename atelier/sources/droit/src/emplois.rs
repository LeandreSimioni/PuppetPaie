//! Les metiers du spectacle (`moteur/emplois.py`) et la fiche telle qu'elle
//! vaut pour un engagement (`calcul.emploi_de`, `calcul._emploi_a_la_date`).
//!
//! Deux fichiers livres avec le code : `emplois-base.json` (le catalogue) et
//! `nomenclature-emplois-ft.json` (les codes France Travail).
//!
//! De l'interface (l'accueil, `routes_accueil.rs`) : `catalogue`, `familles`,
//! `noms_des_conventions` et `controler_fiche`.

use socle::fichiers::{dossier_du_code, json_en_cache, repr, repr_valeur, vrai};
use socle::num::Resultat;
use serde_json::{Map, Value};

/// Les champs de la fiche qu'un historique peut dater (`calcul.CHAMPS_EMPLOI`).
pub const CHAMPS_EMPLOI: [&str; 7] = [
    "emploi",
    "emploi_affichage",
    "code_emploi_ft",
    "pcs_ese",
    "statut",
    "statut_conventionnel",
    "statut_categoriel_rc",
];

/// `str.capitalize()` : premiere lettre en majuscule, le reste en minuscules.
fn capitaliser(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
    }
}

fn statut_de_l_annexe(annexe: &Value) -> &'static str {
    let a = annexe.as_i64().or_else(|| annexe.as_f64().map(|x| x as i64));
    if a == Some(10) { "artiste" } else { "technicien" }
}

/// Un metier par son code, catalogue d'abord, nomenclature ensuite. `None` si
/// le code n'existe nulle part.
pub fn metier(code: &str) -> Resultat<Option<Value>> {
    if code.is_empty() {
        return Ok(None);
    }
    let code = code.trim().to_uppercase();
    let ici = dossier_du_code();
    let base = json_en_cache(&ici.join("emplois-base.json"))?;
    if let Some(m) = base.get("emplois").and_then(|e| e.get(&code)) {
        if vrai(m) {
            let mut m = m.as_object().cloned().unwrap_or_default();
            m.insert("code".into(), Value::String(code));
            m.insert("catalogue".into(), Value::Bool(true));
            return Ok(Some(Value::Object(m)));
        }
    }
    let nomenclature = json_en_cache(&ici.join("nomenclature-emplois-ft.json"))?;
    if let Some(n) = nomenclature.get("emplois").and_then(|e| e.get(&code)) {
        if vrai(n) {
            let libelle = n.get("libelle").and_then(|l| l.as_str()).ok_or_else(|| socle::fichiers::absent("libelle"))?;
            let annexe = n.get("annexe").ok_or_else(|| socle::fichiers::absent("annexe"))?;
            let cap = Value::String(capitaliser(libelle));
            let mut m = Map::new();
            m.insert("code".into(), Value::String(code));
            m.insert("libelle".into(), cap.clone());
            m.insert("feminin".into(), cap);
            m.insert("emploi".into(), Value::String(libelle.to_string()));
            m.insert("annexe".into(), annexe.clone());
            m.insert("statut".into(), Value::String(statut_de_l_annexe(annexe).into()));
            m.insert("famille".into(), Value::String("Hors catalogue".into()));
            m.insert("cadre_par_defaut".into(), Value::Bool(false));
            m.insert("pcs_ese".into(), Value::Null);
            m.insert("statut_conventionnel".into(), Value::Null);
            m.insert("minima".into(), Value::Object(Map::new()));
            m.insert("catalogue".into(), Value::Bool(false));
            return Ok(Some(Value::Object(m)));
        }
    }
    Ok(None)
}

/// Les sept champs declares pour ce metier (`emploi_affichage` compris).
/// `cadre` a `None` prend le defaut du catalogue. Refuse un code inconnu.
pub fn resoudre(code: &str, cadre: Option<bool>, feminin: bool) -> Resultat<Value> {
    let m = metier(code)?.ok_or_else(|| {
        format!(
            "Metier {} inconnu : ni dans le catalogue (emplois-base.json), ni \
             dans la nomenclature France Travail (nomenclature-emplois-ft.json).",
            repr(code))
    })?;
    let est_cadre = match cadre {
        None => vrai(m.get("cadre_par_defaut").ok_or_else(|| socle::fichiers::absent("cadre_par_defaut"))?),
        Some(c) => c,
    };
    let texte = |c: &str| m.get(c).filter(|v| vrai(v)).cloned();
    let choisi = if feminin { texte("feminin") } else { texte("libelle") };
    let libelle = match choisi {
        Some(l) => l,
        None => m.get("libelle").cloned().ok_or_else(|| socle::fichiers::absent("libelle"))?,
    };
    let valeur = |c: &str| m.get(c).cloned().unwrap_or(Value::Null);
    let mut out = Map::new();
    out.insert("emploi".into(), m.get("emploi").cloned().ok_or_else(|| socle::fichiers::absent("emploi"))?);
    out.insert("emploi_affichage".into(), libelle);
    out.insert("pcs_ese".into(), valeur("pcs_ese"));
    out.insert("code_emploi_ft".into(), valeur("code"));
    out.insert("statut".into(), m.get("statut").cloned().ok_or_else(|| socle::fichiers::absent("statut"))?);
    out.insert(
        "statut_conventionnel".into(),
        if est_cadre {
            Value::String("04".into())
        } else {
            statut_non_cadre(&m).map_or(Value::Null, Value::String)
        },
    );
    out.insert(
        "statut_categoriel_rc".into(),
        Value::String(if est_cadre { "01" } else { "04" }.into()),
    );
    Ok(Value::Object(out))
}

/// Le statut conventionnel DSN (40.002) d'un metier declare NON cadre.
pub fn statut_non_cadre(m: &Value) -> Option<String> {
    if let Some(st) = m.get("statut_conventionnel").and_then(|s| s.as_str()) {
        if !st.is_empty() && st != "04" {
            return Some(st.to_string());
        }
    }
    if m.get("statut").and_then(|s| s.as_str()) == Some("artiste") {
        return Some("05".into());
    }
    let c = m.get("classification_3090").filter(|v| vrai(v));
    let lire = |cle: &str, defaut: &'static str| -> String {
        c.and_then(|c| c.get(cle))
            .and_then(|v| v.as_str())
            .unwrap_or(defaut)
            .to_string()
    };
    let niveau = lire("niveau", "");
    let grille = lire("grille", "technique");
    if niveau.starts_with("cadre") || niveau.starts_with("agent") {
        return Some("05".into());
    }
    if !niveau.is_empty() {
        return Some(if grille == "administratif" { "06" } else { "07" }.into());
    }
    None
}

/// Une fonction `telle_qu_au(fiche, date_ref)` : `contexte.telle_qu_au`.
pub type TelleQuAu<'a> = &'a dyn Fn(&Value, &str) -> Resultat<Value>;

/// `calcul._emploi_a_la_date` : l'emploi de la fiche au `jour` (AAAA-MM-JJ),
/// d'apres son historique. `jour` vide ou absent : la fiche telle quelle.
pub fn emploi_a_la_date(sal: &Value, jour: Option<&str>, telle_qu_au: TelleQuAu) -> Resultat<Value> {
    let fiche = sal.as_object().ok_or("fiche salarie : un objet est attendu")?;
    let hist: Vec<Value> = fiche
        .get("historique")
        .and_then(|h| h.as_array())
        .map(|a| {
            a.iter()
                .filter(|e| {
                    e.get("champ")
                        .and_then(|c| c.as_str())
                        .map_or(false, |c| CHAMPS_EMPLOI.contains(&c))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let jour = match jour {
        Some(j) if !j.is_empty() && !hist.is_empty() => j,
        _ => return Ok(sal.clone()),
    };
    let lire = |m: &Map<String, Value>, c: &str| m.get(c).cloned().unwrap_or(Value::Null);
    let mut reduite = Map::new();
    reduite.insert("historique".into(), Value::Array(hist));
    for c in CHAMPS_EMPLOI {
        reduite.insert(c.into(), lire(fiche, c));
    }
    let date = telle_qu_au(&Value::Object(reduite), jour)?;
    let date = date.as_object().ok_or("telle_qu_au : un objet est attendu")?;
    let mut change: Vec<(&str, Value)> = Vec::new();
    for c in CHAMPS_EMPLOI {
        if lire(date, c) != lire(fiche, c) {
            change.push((c, lire(date, c)));
        }
    }
    if change.is_empty() {
        return Ok(sal.clone());
    }
    let a_change = |c: &str| change.iter().find(|(k, _)| *k == c).map(|(_, v)| v);
    let mut out = fiche.clone();
    if a_change("code_emploi_ft").map_or(false, vrai) {
        let code = a_change("code_emploi_ft").and_then(|v| v.as_str()).unwrap_or("");
        let cat = resoudre(code, None, false).unwrap_or(Value::Object(Map::new()));
        // Ce que l'historique ne date pas vient du catalogue du metier d'alors.
        for c in CHAMPS_EMPLOI {
            if a_change(c).is_none() {
                if let Some(v) = cat.get(c).filter(|v| vrai(v)) {
                    out.insert(c.into(), v.clone());
                }
            }
        }
    } else if a_change("emploi").is_some() && a_change("emploi_affichage").is_none() {
        // Le libelle lisible d'aujourd'hui dirait l'autre metier.
        out.shift_remove("emploi_affichage");
    }
    for (c, v) in &change {
        out.insert((*c).into(), v.clone());
    }
    Ok(Value::Object(out))
}

/// Ce qu'un contrat paye au mois ne laisse pas changer en cours de mois : ce qui choisit la pile du bareme (le statut
/// artiste / technicien, la categorie cadre, le metier qui la dit a defaut de la fiche) et ce que le bloc 40 porte avec
/// une rubrique en bloc 41 (40.002, 40.003, 40.004). Le libelle de l'emploi n'en est pas : la DSN declare « la derniere
/// valeur connue » du mois (cahier technique 2026.1, § 2.1.2.4.2, p. 22).
const UNE_VALEUR_PAR_MOIS: [&str; 5] = ["statut_categoriel_rc", "statut_conventionnel", "statut", "code_emploi_ft", "pcs_ese"];

/// La fiche salarie d'un contrat PAYE AU MOIS (CDI, CDD de droit commun), telle qu'elle vaut pour la periode payee
/// (`debut`, `fin`, AAAA-MM-JJ) : l'emploi (`CHAMPS_EMPLOI`) et le statut BOETH lus a la fin de la periode, plus au
/// premier jour du contrat. Un permanent passe cadre, change de poste, obtient ou perd sa RQTH en cours de contrat :
/// la paie et la DSN du mois ou cela prend effet le disent (vague V3c ; jusqu'au 10/10/2026, le moteur relisait la
/// fiche telle qu'au premier jour du contrat, et le passage cadre d'un CDI n'atteignait ni le bareme ni la DSN).
///
/// `sal` est la fiche du contexte (lue a sa date) ; `brute`, la meme telle qu'ecrite : ce qu'elle porte aujourd'hui
/// fait foi pour ces champs, la date du contexte pouvant preceder la fin du mois paye.
///
/// UN CHANGEMENT DANS LA PERIODE REFUSE (`UNE_VALEUR_PAR_MOIS`) : une paie se calcule sur une seule pile de bareme
/// (regle 6). Depuis le lot R10, la paie d'un mois ou ces valeurs changent se decoupe en periodes elementaires aux jours
/// du changement (`jours_de_changement`, `periodes::bornes`, norme § 2.1.3.2, p. 28), et chaque periode lit sa fiche
/// ici, sur ses bornes : le refus ne garde que l'appel sur une periode qui n'aurait pas ete coupee. Le statut BOETH, qui
/// ne change pas la paie, peut changer en cours de mois.
pub fn emploi_du_mois(sal: &Value, brute: Option<&Value>, debut: &str, fin: &str, telle_qu_au: TelleQuAu) -> Resultat<Value> {
    use socle::date::Jour;
    let base = fiche_ecrite(sal, brute)?;
    let au_debut = emploi_a_la_date(&base, Some(debut), telle_qu_au)?;
    let mut a_la_fin = emploi_a_la_date(&base, Some(fin), telle_qu_au)?;
    let lire = |v: &Value, c: &str| v.get(c).cloned().unwrap_or(Value::Null);
    let historique: Vec<&Value> = base.get("historique").and_then(Value::as_array).into_iter().flatten().collect();
    for c in UNE_VALEUR_PAR_MOIS {
        let (avant, apres) = (lire(&au_debut, c), lire(&a_la_fin, c));
        if avant == apres {
            continue;
        }
        // Le jour du changement : le lendemain de la fenetre de l'historique qui finit dans la periode (celle du champ,
        // a defaut celle du metier qui l'a change).
        let fenetre = |de: &dyn Fn(&str) -> bool| {
            historique
                .iter()
                .filter(|e| e.get("champ").and_then(Value::as_str).map_or(false, |x| de(x)))
                .filter_map(|e| e.get("jusqu_au").and_then(Value::as_str))
                .filter(|j| *j >= debut && *j < fin)
                .max()
                .and_then(|j| Jour::lire(j).ok())
                .and_then(|j| j.plus_jours(1).ok())
        };
        let le = fenetre(&|x| x == c).or_else(|| fenetre(&|x| CHAMPS_EMPLOI.contains(&x)));
        let premier = Jour { jour: 1, ..Jour::lire(debut)? };
        let suivant = Jour { jour: 1, ..premier.plus_jours(31)? };
        let texte = |v: &Value| if v.is_null() { "(vide)".to_string() } else { repr_valeur(v) };
        let qui = [lire(&base, "prenom"), lire(&base, "nom")]
            .iter()
            .filter_map(|v| v.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string))
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "{} : la fiche change `{}` le {} ({} -> {}), dans une periode de paie qui n'est pas coupee a ce jour-la : une \
periode se calcule sur une seule pile de bareme. Dater le changement au premier jour d'un mois sur la fiche : le {} (il \
vaut pour tout ce mois-ci) ou le {} (il vaut a partir du mois suivant).",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            c,
            le.map_or_else(|| "?".to_string(), |j| j.jj_mm_aaaa()),
            texte(&avant),
            texte(&apres),
            premier.jj_mm_aaaa(),
            suivant.jj_mm_aaaa()
        ));
    }
    // Le statut BOETH du mois : celui de la fin de la periode (40.072 se renseigne « pour le contrat concerne dans la
    // periode declaree », p. 211).
    let boeth: Vec<Value> =
        historique.iter().filter(|e| e.get("champ").and_then(Value::as_str) == Some("statut_boeth")).map(|e| (*e).clone()).collect();
    if !boeth.is_empty() {
        let mut reduite = Map::new();
        reduite.insert("historique".into(), Value::Array(boeth));
        reduite.insert("statut_boeth".into(), lire(&base, "statut_boeth"));
        let lu = telle_qu_au(&Value::Object(reduite), fin)?;
        let out = a_la_fin.as_object_mut().ok_or("fiche salarie : un objet est attendu")?;
        match lu.get("statut_boeth").filter(|v| !v.is_null()) {
            Some(v) => {
                out.insert("statut_boeth".into(), v.clone());
            }
            None => {
                out.shift_remove("statut_boeth");
            }
        }
    }
    Ok(a_la_fin)
}

/// La fiche `sal` (lue a la date du contexte) dont l'emploi (`CHAMPS_EMPLOI`) et le statut BOETH sont ceux de la fiche
/// telle qu'ecrite (`brute`) : la date du contexte peut preceder la fin du mois paye.
fn fiche_ecrite(sal: &Value, brute: Option<&Value>) -> Resultat<Value> {
    let mut base = sal.as_object().ok_or("fiche salarie : un objet est attendu")?.clone();
    if let Some(b) = brute.and_then(Value::as_object) {
        for c in CHAMPS_EMPLOI.iter().copied().chain(["statut_boeth"]) {
            match b.get(c) {
                Some(v) => {
                    base.insert(c.into(), v.clone());
                }
                None => {
                    base.shift_remove(c);
                }
            }
        }
    }
    Ok(Value::Object(base))
}

/// Les jours, apres `debut` et au plus tard `fin` (AAAA-MM-JJ, la periode payee d'un contrat paye au mois), ou la fiche
/// change une valeur qui choisit la pile du bareme ou que le bloc 40 porte avec une rubrique en bloc 41
/// (`UNE_VALEUR_PAR_MOIS`) : le lendemain d'une fenetre `jusqu_au` de l'historique des champs de l'emploi, quand la
/// fiche d'avant et celle d'apres different sur l'une de ces valeurs. C'est la que la paie du mois se decoupe en periodes
/// elementaires (`periodes::bornes`, lot R10 ; norme DSN 2026.1, § 2.1.3.2, p. 28 : « passage de non cadre a cadre en
/// cours de periode de paie »). Vide : le mois se paie d'un seul tenant.
pub fn jours_de_changement(sal: &Value, brute: Option<&Value>, debut: &str, fin: &str, telle_qu_au: TelleQuAu) -> Resultat<Vec<socle::date::Jour>> {
    use socle::date::Jour;
    let base = fiche_ecrite(sal, brute)?;
    let mut jours: Vec<Jour> = Vec::new();
    for e in base.get("historique").and_then(Value::as_array).into_iter().flatten() {
        if !e.get("champ").and_then(Value::as_str).map_or(false, |c| CHAMPS_EMPLOI.contains(&c)) {
            continue;
        }
        let Some(j) = e.get("jusqu_au").and_then(Value::as_str).filter(|j| *j >= debut && *j < fin) else { continue };
        jours.push(Jour::lire(j)?.plus_jours(1)?);
    }
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    for j in jours {
        let veille = j.plus_jours(-1)?.iso();
        let (avant, apres) = (emploi_a_la_date(&base, Some(&veille), telle_qu_au)?, emploi_a_la_date(&base, Some(&j.iso()), telle_qu_au)?);
        if UNE_VALEUR_PAR_MOIS.iter().any(|c| avant.get(*c) != apres.get(*c)) {
            out.push(j);
        }
    }
    Ok(out)
}

/// Ce que `calcul.emploi_de` lit d'un `Engagement`.
#[derive(Debug, Clone, Default)]
pub struct EmploiEngagement {
    pub emploi: Option<String>,
    /// Le libelle lisible que l'engagement a resolu (`Engagement.emploi_affichage`).
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
}

/// `calcul.emploi_de` : la fiche salarie telle qu'elle vaut POUR CET
/// ENGAGEMENT. `debut` est le premier jour du contrat (AAAA-MM-JJ). Ce que
/// l'engagement dit l'emporte ; le reste vient de la fiche.
pub fn emploi_de(
    sal: &Value,
    debut: Option<&str>,
    eng: &EmploiEngagement,
    telle_qu_au: TelleQuAu,
) -> Resultat<Value> {
    let sal = emploi_a_la_date(sal, debut, telle_qu_au)?;
    let champs: [(&str, &Option<String>); 6] = [
        ("emploi", &eng.emploi),
        ("pcs_ese", &eng.pcs_ese),
        ("code_emploi_ft", &eng.code_emploi_ft),
        ("statut", &eng.statut),
        ("statut_conventionnel", &eng.statut_conventionnel),
        ("statut_categoriel_rc", &eng.statut_categoriel_rc),
    ];
    let pris: Vec<(&str, &String)> = champs
        .iter()
        .filter_map(|(c, v)| v.as_ref().filter(|s| !s.is_empty()).map(|s| (*c, s)))
        .collect();
    let affichage = eng.emploi_affichage.as_ref().filter(|s| !s.is_empty());
    if pris.is_empty() && affichage.is_none() {
        return Ok(sal);
    }
    let mut out = sal.as_object().ok_or("fiche salarie : un objet est attendu")?.clone();
    for (c, v) in &pris {
        out.insert((*c).into(), Value::String((*v).clone()));
    }
    // Un metier choisi sur la ligne : son libelle lisible aussi, tel que
    // l'engagement l'a resolu (au feminin si la fiche le dit) ; a defaut, le
    // libelle du catalogue.
    if let Some(a) = affichage {
        out.insert("emploi_affichage".into(), Value::String(a.clone()));
    } else if let Some((_, code)) = pris.iter().find(|(c, _)| *c == "code_emploi_ft") {
        if sal.get("code_emploi_ft").and_then(|v| v.as_str()) != Some(code.as_str()) {
            match resoudre(code, None, false) {
                Ok(r) => {
                    out.insert("emploi_affichage".into(), r["emploi_affichage"].clone());
                }
                Err(_) => {
                    out.shift_remove("emploi_affichage");
                }
            }
        }
    }
    Ok(Value::Object(out))
}

// ------------------------------------------------------------- l'interface

/// `emplois.catalogue()` : le catalogue du spectacle vivant, pour un
/// deroulant, dans l'ordre du fichier.
pub fn catalogue() -> Resultat<Vec<Value>> {
    let base = json_en_cache(&dossier_du_code().join("emplois-base.json"))?;
    let emplois = base.get("emplois").and_then(Value::as_object).ok_or_else(|| socle::fichiers::absent("emplois"))?;
    let mut out = Vec::new();
    for (code, m) in emplois {
        let champ = |k: &str| m.get(k).cloned().ok_or_else(|| socle::fichiers::absent(k));
        let niveau = m.get("classification_3090").filter(|v| vrai(v)).and_then(|c| c.get("niveau")).cloned().unwrap_or(Value::Null);
        let conventions = m.get("conventions").filter(|v| vrai(v)).cloned().unwrap_or_else(|| Value::Array(vec![]));
        let mut e = Map::new();
        e.insert("code".into(), Value::String(code.clone()));
        e.insert("libelle".into(), champ("libelle")?);
        e.insert("feminin".into(), champ("feminin")?);
        e.insert("famille".into(), champ("famille")?);
        e.insert("statut".into(), champ("statut")?);
        e.insert("annexe".into(), champ("annexe")?);
        e.insert("pcs_ese".into(), champ("pcs_ese")?);
        e.insert("cadre_par_defaut".into(), champ("cadre_par_defaut")?);
        e.insert("niveau_3090".into(), niveau);
        e.insert("conventions".into(), conventions);
        e.insert("catalogue".into(), Value::Bool(true));
        out.push(Value::Object(e));
    }
    Ok(out)
}

/// Le metier du catalogue qu'un intitule imprime nomme (un bulletin d'un logiciel precedent : « MUSICIEN »), compare
/// par `norme` (sans casse ni accents) a son libelle, a son feminin et a sa forme declaree. `(code, feminin)` quand un
/// seul metier porte cet intitule ; `None` quand aucun, ou plusieurs (« SCRIPTE ») : on ne choisit pas entre eux.
pub fn par_intitule(intitule: &str, norme: impl Fn(&str) -> String) -> Resultat<Option<(String, bool)>> {
    let cherche = norme(intitule);
    if cherche.is_empty() {
        return Ok(None);
    }
    let base = json_en_cache(&dossier_du_code().join("emplois-base.json"))?;
    let emplois = base.get("emplois").and_then(Value::as_object).ok_or_else(|| socle::fichiers::absent("emplois"))?;
    let mut trouves: Vec<(String, bool)> = Vec::new();
    for (code, m) in emplois {
        let t = |k: &str| m.get(k).and_then(Value::as_str).map(&norme).unwrap_or_default();
        let (libelle, feminin, emploi) = (t("libelle"), t("feminin"), t("emploi"));
        if [&libelle, &feminin, &emploi].contains(&&cherche) {
            trouves.push((code.clone(), feminin == cherche && libelle != cherche));
        }
    }
    Ok(if trouves.len() == 1 { trouves.pop() } else { None })
}

/// `emplois.noms_des_conventions()` : IDCC -> le nom court de la convention.
pub fn noms_des_conventions() -> Resultat<Map<String, Value>> {
    let base = json_en_cache(&dossier_du_code().join("emplois-base.json"))?;
    Ok(base.get("_conventions").and_then(Value::as_object).cloned().unwrap_or_default())
}

/// `emplois.familles()` : les familles du catalogue, dans l'ordre ou les
/// montrer : les artistes d'abord, parce que c'est le cas de presque toutes
/// les paies.
pub fn familles() -> Resultat<Vec<String>> {
    const ORDRE: [&str; 11] = [
        "Artistes", "Musique", "Cirque et rue", "Mise en scène et création", "Régie et plateau", "Son", "Lumière",
        "Décor et construction", "Costume, coiffure, maquillage", "Production et administration", "Audiovisuel dans le spectacle",
    ];
    let base = json_en_cache(&dossier_du_code().join("emplois-base.json"))?;
    let emplois = base.get("emplois").and_then(Value::as_object).ok_or_else(|| socle::fichiers::absent("emplois"))?;
    let mut vues = std::collections::BTreeSet::new();
    for m in emplois.values() {
        vues.insert(m.get("famille").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("famille"))?.to_string());
    }
    let mut out: Vec<String> = ORDRE.iter().filter(|f| vues.contains(**f)).map(|f| f.to_string()).collect();
    out.extend(vues.iter().filter(|f| !ORDRE.contains(&f.as_str())).cloned());
    Ok(out)
}

/// `emplois.controler_fiche` : ce qu'une fiche salarie declare, confronte au
/// catalogue. Des phrases d'avertissement, jamais d'erreur : la fiche peut
/// avoir raison contre le catalogue.
pub fn controler_fiche(d: &Map<String, Value>) -> Result<Vec<String>, socle::echec::Echec> {
    use socle::valeur::eq_py;
    use socle::valeur::py_repr;
    use socle::valeur::nom_type;
    use socle::echec::erreur;
    let mut avis = Vec::new();
    let code = match d.get("code_emploi_ft") {
        Some(Value::String(s)) => s.trim().to_uppercase(),
        Some(v) if vrai(v) => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(v)))),
        _ => String::new(),
    };
    if code.is_empty() {
        return Ok(avis);
    }
    let Some(m) = metier(&code)? else {
        avis.push(format!("Code emploi France Travail {} inconnu de la nomenclature (S21.G00.40.005).", repr(&code)));
        return Ok(avis);
    };
    let texte = |v: Option<&Value>| v.map(socle::valeur::py_str).unwrap_or_default();
    let libelle = texte(m.get("libelle"));
    if let Some(statut) = d.get("statut").filter(|v| vrai(v)) {
        if !m.get("statut").map_or(false, |x| eq_py(statut, x)) {
            let annexe = m.get("annexe").and_then(|a| a.as_i64().or_else(|| a.as_f64().map(|x| x as i64))).unwrap_or(0);
            avis.push(format!(
                "Le code emploi {} ({}) est de l'annexe {}, donc {} ; la fiche dit {}.",
                code, libelle, annexe, texte(m.get("statut")), py_repr(statut)
            ));
        }
    }
    let attendu = m.get("pcs_ese").filter(|v| vrai(v));
    let saisi = d.get("pcs_ese").filter(|v| vrai(v));
    if let (Some(attendu), Some(saisi)) = (attendu, saisi) {
        let Value::String(_) = saisi else {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(saisi))));
        };
        let (a, s) = (texte(Some(attendu)), texte(Some(saisi)));
        if s.to_lowercase() != a.to_lowercase() {
            avis.push(format!("PCS-ESE {} pour {} ; le catalogue attend {} (fiche Insee de la rubrique).", s, libelle, a));
        }
    }
    Ok(avis)
}
