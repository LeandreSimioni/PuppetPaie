//! Les metiers du spectacle et la fiche telle qu'elle vaut pour un engagement (`emploi_de`, `emploi_du_mois`).
//!
//! Deux fichiers livres avec le code : `emplois-base.json` (le catalogue) et
//! `nomenclature-emplois-ft.json` (les codes France Travail).
//!
//! De l'interface (l'accueil, `routes_accueil.rs`) : `catalogue`, `familles`,
//! `noms_des_conventions` et `controler_fiche`.

use socle::fichiers::{dossier_du_code, json_en_cache, repr, repr_valeur, vrai};
use socle::num::Resultat;
use serde_json::{Map, Value};
use socle::date::Jour;

use crate::contexte::{fiche_au, valeurs_datees, Contexte};

/// Premiere lettre en majuscule, le reste en minuscules.
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

/// Ce qu'un contrat paye au mois ne laisse pas changer en cours de mois : ce qui choisit la pile du bareme (le statut
/// artiste / technicien, la categorie cadre, le metier qui la dit a defaut de la fiche) et ce que le bloc 40 porte avec
/// une rubrique en bloc 41 (40.002, 40.003, 40.004). Le libelle de l'emploi n'en est pas : la DSN declare « la derniere
/// valeur connue » du mois (cahier technique 2026.1, § 2.1.2.4.2, p. 22).
const UNE_VALEUR_PAR_MOIS: [&str; 5] = ["statut_categoriel_rc", "statut_conventionnel", "statut", "code_emploi_ft", "pcs_ese"];

/// La fiche salarie `cle` d'un contrat PAYE AU MOIS (CDI, CDD de droit commun), telle qu'elle vaut pour la periode payee
/// (`debut`, `fin`) : lue a la fin de la periode. Un permanent passe cadre, change de poste, obtient ou perd sa RQTH en
/// cours de contrat : la paie et la DSN du mois ou cela prend effet le disent.
///
/// UN CHANGEMENT DANS LA PERIODE REFUSE (`UNE_VALEUR_PAR_MOIS`) : une paie se calcule sur une seule pile de bareme
/// (regle 6). La paie d'un mois ou ces valeurs changent se decoupe en periodes elementaires aux jours du changement
/// (`jours_de_changement`, `periodes::bornes`, norme § 2.1.3.2, p. 28), et chaque periode lit sa fiche ici, sur ses
/// bornes : le refus ne garde que l'appel sur une periode qui n'aurait pas ete coupee. Le statut BOETH, qui ne change
/// pas la paie, peut changer en cours de mois (40.072 se renseigne « pour le contrat concerne dans la periode
/// declaree », p. 211 : celui de la fin).
pub fn emploi_du_mois(ctx: &Contexte, cle: &str, debut: &Jour, fin: &Jour) -> Resultat<Value> {
    let au_debut = ctx.salarie_au(cle, debut)?;
    let a_la_fin = ctx.salarie_au(cle, fin)?;
    let lire = |v: &Value, c: &str| v.get(c).cloned().unwrap_or(Value::Null);
    for c in UNE_VALEUR_PAR_MOIS {
        let (avant, apres) = (lire(&au_debut, c), lire(&a_la_fin, c));
        if avant == apres {
            continue;
        }
        let brute = ctx.salarie_brut(cle).unwrap_or(&Value::Null);
        let le = valeurs_datees(brute, c)?.into_iter().map(|(j, _)| j).filter(|j| j > debut && j <= fin).last();
        let premier = Jour { jour: 1, ..*debut };
        let suivant = Jour { jour: 1, ..premier.plus_jours(31)? };
        let texte = |v: &Value| if v.is_null() { "(vide)".to_string() } else { repr_valeur(v) };
        let qui = [lire(&a_la_fin, "prenom"), lire(&a_la_fin, "nom")]
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
    Ok(a_la_fin)
}

/// Les jours, apres `debut` et au plus tard `fin` (la periode payee d'un contrat paye au mois), ou la fiche `cle` change
/// une valeur qui choisit la pile du bareme ou que le bloc 40 porte avec une rubrique en bloc 41 (`UNE_VALEUR_PAR_MOIS`) :
/// le `depuis` d'une de ces valeurs, quand la fiche de la veille et celle du jour different. C'est la que la paie du mois
/// se decoupe en periodes elementaires (`periodes::bornes` ; norme DSN 2026.1, § 2.1.3.2, p. 28 : « passage de non
/// cadre a cadre en cours de periode de paie »). Vide : le mois se paie d'un seul tenant.
pub fn jours_de_changement(ctx: &Contexte, cle: &str, debut: &Jour, fin: &Jour) -> Resultat<Vec<Jour>> {
    let brute = ctx.salarie_brut(cle).unwrap_or(&Value::Null);
    let mut jours: Vec<Jour> = Vec::new();
    for c in UNE_VALEUR_PAR_MOIS {
        jours.extend(valeurs_datees(brute, c)?.into_iter().map(|(j, _)| j).filter(|j| j > debut && j <= fin));
    }
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    for j in jours {
        let (avant, apres) = (fiche_au(brute, &j.plus_jours(-1)?, cle)?, fiche_au(brute, &j, cle)?);
        if UNE_VALEUR_PAR_MOIS.iter().any(|c| avant.get(*c) != apres.get(*c)) {
            out.push(j);
        }
    }
    Ok(out)
}

/// Ce que `emploi_de` lit d'un engagement.
#[derive(Debug, Clone, Default)]
pub struct EmploiEngagement {
    pub emploi: Option<String>,
    /// Le libelle lisible que l'engagement a resolu.
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<String>,
    pub code_emploi_ft: Option<String>,
    pub statut: Option<String>,
    pub statut_conventionnel: Option<String>,
    pub statut_categoriel_rc: Option<String>,
}

/// La fiche salarie `sal` (lue a la date de la periode) telle qu'elle vaut POUR CET ENGAGEMENT : ce que l'engagement
/// dit l'emporte ; le reste vient de la fiche.
pub fn emploi_de(sal: &Value, eng: &EmploiEngagement) -> Resultat<Value> {
    let sal = sal.clone();
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

/// Le catalogue du spectacle vivant, pour un
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

/// IDCC -> le nom court de la convention.
pub fn noms_des_conventions() -> Resultat<Map<String, Value>> {
    let base = json_en_cache(&dossier_du_code().join("emplois-base.json"))?;
    Ok(base.get("_conventions").and_then(Value::as_object).cloned().unwrap_or_default())
}

/// Les familles du catalogue, dans l'ordre ou les
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

/// Ce qu'une fiche salarie declare, confronte au
/// catalogue. Des phrases d'avertissement, jamais d'erreur : la fiche peut
/// avoir raison contre le catalogue.
pub fn controler_fiche(d: &Map<String, Value>) -> Result<Vec<String>, socle::echec::Echec> {
    use socle::valeur::valeurs_egales;
    use socle::valeur::valeur_ecrite;
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
    let texte = |v: Option<&Value>| v.map(socle::valeur::valeur_en_texte).unwrap_or_default();
    let libelle = texte(m.get("libelle"));
    if let Some(statut) = d.get("statut").filter(|v| vrai(v)) {
        if !m.get("statut").map_or(false, |x| valeurs_egales(statut, x)) {
            let annexe = m.get("annexe").and_then(|a| a.as_i64().or_else(|| a.as_f64().map(|x| x as i64))).unwrap_or(0);
            avis.push(format!(
                "Le code emploi {} ({}) est de l'annexe {}, donc {} ; la fiche dit {}.",
                code, libelle, annexe, texte(m.get("statut")), valeur_ecrite(statut)
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
