//! `regularisation-apercu` : ce que la correction d'un mois deja depose changerait -- avant, apres,
//! difference. Rien n'est ecrit.
//!
//! C'est le Python de `operations_regularisation.apercu_correction` et de ce qu'il appelle
//! (`etat_mois`, `engagements_d_origine`, `_engagement_declare`, `_paie_d_un_engagement`,
//! `_poser_numeros`, `numeros_declares`, `_dsn_deposees`, `lire_corrections`, `_normaliser`,
//! `_resume`), porte a l'identique : meme JSON, memes refus. La seule ecriture est celle du cache du
//! calcul du mois (`calcul_du_mois`), comme pour les autres apercus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use crate::calcul::Paie;
use crate::contexte::Contexte;
use crate::etats::{self, erreur, get, nom_type, py_str, R};
use crate::fichiers::vrai;
use crate::num::Decimal;
use crate::retours::Echec;
use crate::routes_mois::Moteur;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 1] = ["regularisation-apercu"];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "regularisation-apercu" => apercu_correction(o, ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(k)))
}

fn non_regularisable(message: impl Into<String>) -> Echec {
    erreur("MoisNonRegularisable", message)
}

/// `d["cle"]` : `KeyError` si elle manque.
pub(crate) fn requis<'a>(d: &'a Value, k: &str) -> R<&'a Value> {
    d.get(k).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(k)))
}

/// `jj_mm_aaaa(x)`.
fn jj_mm_aaaa(iso: &str) -> R<String> {
    Ok(crate::date::Jour::lire(iso).map_err(|m| erreur("ValueError", m))?.jj_mm_aaaa())
}

// ------------------------------------------------------------------ le mois d'origine

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
pub(crate) fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois_v: &Value) -> R<Value> {
    let e = m.dsn_du_mois(&py_str(mois_v))?;
    let Some(mois) = mois_v.as_str() else {
        return Err(erreur("TypeError", format!("'mois' : {}", crate::fichiers::attendu("un texte", nom_type(mois_v)))));
    };
    let echeance = crate::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let passee = ctx.aujourdhui > limite;
    let deposee = vrai(&e);
    let drapeau = |k: &str| deposee && e.get(k).is_some_and(vrai);
    let mut out = Map::new();
    out.insert("mois".into(), json!(mois));
    out.insert("deposee".into(), json!(deposee));
    out.insert("passee".into(), json!(drapeau("passee")));
    out.insert("refusee".into(), json!(drapeau("refusee")));
    out.insert("echeance".into(), json!(limite.iso()));
    out.insert("echeance_depot".into(), echeance.en_json());
    out.insert("echeance_passee".into(), json!(passee));
    out.insert("mois_courant".into(), json!(ctx.aujourdhui.iso().chars().take(7).collect::<String>()));
    out.insert("regularisable".into(), json!(false));
    out.insert("raison".into(), json!(""));
    if !deposee {
        let suite = if passee { format!(" (en retard : `paie mois paies/{}.json --forcer`)", mois) } else { String::new() };
        out.insert(
            "raison".into(),
            json!(format!("Aucune DSN reelle n'est partie pour {} : ce mois se saisit et se depose normalement{}.", mois, suite)),
        );
    } else if drapeau("refusee") {
        out.insert(
            "raison".into(),
            json!(format!("La DSN de {} a ete REFUSEE : elle se corrige et se redepose, il n'y a rien a regulariser.", mois)),
        );
    } else {
        // LA FRONTIERE EST LA VEILLE A MINUIT, pas le jour de l'echeance.
        let veille = m.limite(mois)?;
        if ctx.aujourdhui <= veille {
            out.insert(
                "raison".into(),
                json!(format!(
                    "La DSN de {} est partie et l'annule-et-remplace est admis jusqu'au {} a minuit : modifier le mois et refaire la DSN, elle partira en annule-et-remplace.",
                    mois,
                    veille.jj_mm_aaaa()
                )),
            );
        } else {
            out.insert("regularisable".into(), json!(true));
            out.insert(
                "raison".into(),
                json!(format!(
                    "La DSN de {} est deposee et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance du {}) : une correction part dans la DSN du mois courant, datee de {}.",
                    mois,
                    veille.jj_mm_aaaa(),
                    limite.jj_mm_aaaa(),
                    mois
                )),
            );
        }
    }
    Ok(Value::Object(out))
}

/// `_dsn_deposees` : les DSN de `mois` sur le disque, lues -- celle que le journal nomme d'abord, les
/// autres ensuite.
pub(crate) fn dsn_deposees(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Vec<Value>> {
    let dossier = ctx.racine.join("sorties").join(mois);
    if !dossier.exists() {
        return Ok(Vec::new());
    }
    let lignes = m.lignes()?;
    let mut fichiers: Vec<PathBuf> = Vec::new();
    for l in etats::filtrer(&lignes, Some("dsn"), Some(mois), false)? {
        if etats::est(get(l, "mode")?, "reel") && vrai(get(l, "fichier")?) {
            let p = dossier.join(py_str(get(l, "fichier")?));
            if p.exists() {
                fichiers.push(p);
            }
        }
    }
    let mut autres: Vec<PathBuf> = match std::fs::read_dir(&dossier) {
        Ok(l) => l.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "dsn")).collect(),
        Err(_) => Vec::new(),
    };
    autres.sort();
    for f in autres {
        if !fichiers.contains(&f) {
            fichiers.push(f);
        }
    }
    let mut out = Vec::new();
    for f in fichiers {
        if let Some(lu) = crate::importation::lire_une_dsn(&f) {
            if lu.get("mois").and_then(Value::as_str) == Some(mois) {
                out.push(lu);
            }
        }
    }
    Ok(out)
}

/// `numeros_declares` : (NIR, premier jour) -> numero de contrat, LUS DANS LA DSN DEPOSEE.
fn numeros_declares(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<HashMap<(String, String), String>> {
    let mut out = HashMap::new();
    for lu in dsn_deposees(m, ctx, mois)? {
        for c in lu["contrats"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let t = |k: &str| c.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
            if let (Some(numero), Some(nir), Some(debut)) = (t("numero"), t("nir"), t("debut")) {
                out.insert((nir.to_string(), debut.to_string()), numero.to_string());
            }
        }
    }
    Ok(out)
}

/// `_poser_numeros` : rend a chaque paie le numero sous lequel son contrat a ete declare.
pub(crate) fn poser_numeros(m: &mut Moteur, ctx: &Contexte, mois: &str, paies: &mut [Paie]) -> R<()> {
    let mut declares: Option<HashMap<(String, String), String>> = None;
    for p in paies.iter_mut() {
        if p.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) {
            continue;
        }
        if declares.is_none() {
            declares = Some(numeros_declares(m, ctx, mois)?);
        }
        let nir = p.salarie.get("nir").and_then(Value::as_str).unwrap_or("").to_string();
        p.engagement.numero = declares.as_ref().and_then(|d| d.get(&(nir, p.engagement.debut.iso())).cloned());
    }
    Ok(())
}

/// `charger(ctx, data, sans_numeros=True).paies`.
pub(crate) fn paies_de(o: &HashMap<String, String>, ctx: &Contexte, data: &Value) -> R<Vec<Paie>> {
    paies_de_en(o, ctx, data, false)
}

/// `frais` : la paie se CALCULE (`charger(ctx, donnees)` du Python : jamais `calcul.json`, rien d'ecrit). Faux :
/// celle de l'ecran (`paie_du_mois`, relue ou ecrite dans `calcul.json`).
pub(crate) fn paies_de_en(o: &HashMap<String, String>, ctx: &Contexte, data: &Value, frais: bool) -> R<Vec<Paie>> {
    let rep = if frais {
        crate::service::calculer_avec(ctx, &json!({ "saisie": data })).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, data, m)))?
    } else {
        crate::routes_ouverture::paie_du_mois(o, ctx, data)?
    };
    let engs = crate::routes_ouverture::engagements_calcules(ctx, data, &rep)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
    let mut paies = Vec::new();
    for (e, v) in engs.into_iter().zip(paies_json) {
        paies.push(crate::routes_ouverture::paie_de_json(ctx, e, v)?);
    }
    Ok(paies)
}

/// `_saisie_d_origine`.
/// La saisie TELLE QUE DECLAREE quand elle a ete modifiee apres la limite de l'annule-et-remplace
/// (`chemin_saisie_declaree`, 21/10/2026) : c'est d'elle que la correction part.
pub(crate) fn saisie_d_origine(ctx: &Contexte, mois: &str) -> R<Value> {
    let declaree = chemin_saisie_declaree(ctx, mois);
    if declaree.exists() {
        return Ok(crate::engagement::lire_saisie(&declaree)?);
    }
    let p = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !p.exists() {
        return Err(non_regularisable(format!("La saisie de {} (paies/{}.json) n'est pas sur le disque : rien a corriger.", mois, mois)));
    }
    Ok(crate::engagement::lire_saisie(&p)?)
}

/// `lire_corrections`, reduit a ce que l'apercu lit : l'engagement que la derniere correction PORTEE a
/// laisse, par (salarie, debut).
fn engagements_portes(ctx: &Contexte, mois: &str) -> R<HashMap<(String, String), Value>> {
    let p = ctx.racine.join("regularisations").join(format!("{}.json", mois));
    let mut portees = HashMap::new();
    if !p.exists() {
        return Ok(portees);
    }
    let texte = std::fs::read_to_string(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
    let d: Value = serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))?;
    let corrections = d.get("corrections").cloned().unwrap_or_else(|| json!([]));
    for c in etats::items(&corrections)? {
        for k in ["mois", "numero", "salarie", "debut", "motif", "engagement", "decidee_le", "qui"] {
            if c.get(k).is_none() {
                return Err(erreur("TypeError", format!("Correction : champ manquant '{}'", k)));
            }
        }
        if c.get("portee_par").is_some_and(vrai) {
            let k = (py_str(&c["salarie"]), py_str(&c["debut"]));
            portees.insert(k, c["engagement"].clone());
        }
    }
    Ok(portees)
}

/// `engagements_d_origine`, puis `_engagement_declare` : l'engagement du contrat tel que DECLARE, la
/// saisie du mois, et ce que la page montre du contrat (nom, spectacle, numero).
pub(crate) fn engagement_declare(
    o: &HashMap<String, String>, m: &mut Moteur, ctx: &Contexte, mois: &str, salarie: &Value, debut: &Value,
) -> R<(Value, Value, Value)> {
    engagement_declare_en(o, m, ctx, mois, salarie, debut, false)
}

pub(crate) fn engagement_declare_en(
    o: &HashMap<String, String>, m: &mut Moteur, ctx: &Contexte, mois: &str, salarie: &Value, debut: &Value, frais: bool,
) -> R<(Value, Value, Value)> {
    let donnees = saisie_d_origine(ctx, mois)?;
    let mut paies = paies_de_en(o, ctx, &donnees, frais)?;
    poser_numeros(m, ctx, mois, &mut paies)?;
    let portees = engagements_portes(ctx, mois)?;
    let engs = donnees.get("engagements").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("engagements")))?;
    let engs = etats::items(engs)?;
    let mut trouve: Option<(Value, Value)> = None;
    for (p, e) in paies.iter().zip(engs.iter()) {
        let cle_s = py_str(requis(&p.salarie, "_cle")?);
        let debut_s = p.engagement.debut.iso();
        let declare = portees.get(&(cle_s.clone(), debut_s.clone())).cloned().unwrap_or_else(|| e.as_ref().clone());
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => py_str(requis(sal, "prenom")?),
        };
        let fiche = json!({
            "nom": format!("{} {}", py_str(requis(sal, "nom")?), prenom),
            "spectacle": p.titre()?,
            "numero": p.engagement.numero,
        });
        if trouve.is_none() && salarie.as_str() == Some(&cle_s) && debut.as_str() == Some(&debut_s) {
            trouve = Some((declare, fiche));
        }
    }
    match trouve {
        Some((declare, fiche)) => Ok((donnees, declare, fiche)),
        None => Err(non_regularisable(format!(
            "Aucun engagement de {} commencant le {} dans la saisie de {}. Un contrat jamais declare n'est pas une correction : c'est un contrat en retard, avec sa DPAE -- a saisir dans son mois avec `--forcer`.",
            py_str(salarie),
            py_str(debut),
            mois
        ))),
    }
}

/// `_paie_d_un_engagement` : la paie d'UN engagement, calculee comme le mois l'a calculee, avec le
/// numero de contrat DECLARE.
pub(crate) fn paie_d_un_engagement(o: &HashMap<String, String>, m: &mut Moteur, ctx: &Contexte, donnees: &Value, engagement: &Value) -> R<Paie> {
    paie_d_un_engagement_en(o, m, ctx, donnees, engagement, false)
}

pub(crate) fn paie_d_un_engagement_en(o: &HashMap<String, String>, m: &mut Moteur, ctx: &Contexte, donnees: &Value, engagement: &Value, frais: bool) -> R<Paie> {
    let mut seule = donnees.clone();
    if let Some(obj) = seule.as_object_mut() {
        obj.insert("engagements".into(), Value::Array(vec![engagement.clone()]));
    }
    let mut paies = paies_de_en(o, ctx, &seule, frais)?;
    let mois = py_str(requis(donnees, "mois")?);
    poser_numeros(m, ctx, &mois, &mut paies)?;
    if paies.is_empty() {
        return Err(erreur("IndexError", crate::fichiers::trop_courte("la paie corrigée")));
    }
    Ok(paies.remove(0))
}

/// `_normaliser` : l'engagement corrige garde ce qui identifie le contrat declare.
fn normaliser(declare: &Value, engagement: &Value) -> R<Value> {
    let mut corrige = match declare {
        Value::Object(o) => o.clone(),
        _ => return Err(erreur("TypeError", "'declare' n'est pas un dictionnaire")),
    };
    for k in ["lignes", "taux_cachet", "jours", "lieu", "date_paiement"] {
        match engagement {
            Value::Object(e) => {
                if let Some(v) = e.get(k) {
                    corrige.insert(k.to_string(), v.clone());
                }
            }
            Value::Array(a) => {
                if a.iter().any(|x| x.as_str() == Some(k)) {
                    return Err(erreur("TypeError", crate::fichiers::attendu("un objet", "list")));
                }
            }
            Value::String(s) => {
                if s.contains(k) {
                    return Err(erreur("TypeError", crate::fichiers::attendu("un objet", "str")));
                }
            }
            autre => return Err(erreur("TypeError", crate::fichiers::attendu("un objet", nom_type(autre)))),
        }
    }
    for k in ["salarie", "spectacle", "numero", "debut", "fin", "metier", "cadre", "emploi", "pcs_ese", "code_emploi_ft", "statut"] {
        if let Some(v) = declare.get(k) {
            corrige.insert(k.to_string(), v.clone());
        }
    }
    corrige.shift_remove("planning");
    corrige.shift_remove("valide");
    Ok(Value::Object(corrige))
}

/// `paie_rust._euros` : deux decimales, la moitie au pair.
pub(crate) fn q2(d: Decimal) -> String {
    let mut r = d.round_dp(2);
    r.rescale(2);
    r.to_string()
}

/// `_resume`.
fn resume(p: &Paie) -> Value {
    let e = &p.engagement;
    let heures = match e.heures {
        Some(h) if h != 0.0 => crate::ecriture::repr_flottant(h),
        _ => "0".to_string(),
    };
    json!({
        "brut": q2(p.brut),
        "net_a_payer": q2(p.net_a_payer),
        "net_imposable": q2(p.net_imposable),
        "cotisations_salariales": q2(p.total_sal),
        "cotisations_patronales": q2(p.total_pat),
        "cout_employeur": q2(p.cout_employeur),
        "cachets": e.cachets,
        "heures": heures,
        "jours": e.jours,
        "remuneration": p.remuneration.iter().map(|l| l.designation()).collect::<Vec<_>>(),
    })
}

fn decimal(v: &Value, k: &str) -> R<Decimal> {
    Decimal::from_str(v[k].as_str().unwrap_or("0")).map_err(|e| erreur("InvalidOperation", e.to_string()))
}

/// `apercu_correction` : ce que la correction changerait : avant, apres, difference.
pub(crate) fn apercu_correction(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let salarie = cle(charge, "salarie")?;
    let debut = cle(charge, "debut")?;
    let engagement = cle(charge, "engagement")?;
    let mut m = Moteur::nouveau(o, ctx);
    let etat = etat_mois(&mut m, ctx, mois_v)?;
    if !etat["regularisable"].as_bool().unwrap_or(false) {
        return Err(non_regularisable(py_str(&etat["raison"])));
    }
    let mois = py_str(mois_v);
    let (donnees, declare, fiche) = engagement_declare(o, &mut m, ctx, &mois, salarie, debut)?;
    let avant = paie_d_un_engagement(o, &mut m, ctx, &donnees, &declare)?;
    if !avant.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) {
        // Sans le numero sous lequel le contrat a ete DECLARE, la correction ne designe rien.
        return Err(non_regularisable(format!(
            "Le numero de contrat declare pour {} (engagement du {}) est introuvable : la saisie de {} ne le porte pas, et la DSN deposee de ce mois n'est pas relisible dans sorties/{}/. Une correction doit citer le numero d'origine (S21.G00.40.009), il ne se recalcule pas. Remettre la DSN deposee dans sorties/{}/ suffit.",
            py_str(salarie),
            py_str(debut),
            mois,
            mois,
            mois
        )));
    }
    let mut corrige = normaliser(&declare, engagement)?;
    // Les bornes declarees, explicites : un cachet ajoute en debut de mois ne deplace pas le contrat. Un contrat sans
    // fin (un CDI, paye au mois) garde sa fin absente : `null`, que `Engagement` relit comme il l'a ecrite
    // (`engagement::Engagement::canon`), et non le texte « None », qui n'est pas une date (vague 3a).
    let fin_json = |p: &Paie| p.engagement.fin.map_or(Value::Null, |f| json!(f.iso()));
    let texte_fin = |p: &Paie| p.engagement.fin.map_or("sans fin".to_string(), |f| f.iso());
    if let Some(c) = corrige.as_object_mut() {
        c.insert("debut".into(), json!(avant.engagement.debut.iso()));
        c.insert("fin".into(), fin_json(&avant));
        c.insert("numero".into(), json!(avant.engagement.numero));
    }
    let apres = paie_d_un_engagement(o, &mut m, ctx, &donnees, &corrige)?;
    if apres.engagement.debut.iso() != avant.engagement.debut.iso() || apres.engagement.fin != avant.engagement.fin {
        return Err(non_regularisable(format!(
            "La correction deplace les bornes du contrat {} ({} -> {}). Le contrat declare garde ses dates : seules ses lignes se corrigent.",
            avant.engagement.numero.clone().unwrap_or_default(),
            format!("{}..{}", avant.engagement.debut.iso(), texte_fin(&avant)),
            format!("{}..{}", apres.engagement.debut.iso(), texte_fin(&apres))
        )));
    }
    let (r_avant, r_apres) = (resume(&avant), resume(&apres));
    let mut delta = Map::new();
    let mut tout_nul = true;
    for k in ["brut", "net_a_payer", "net_imposable", "cotisations_salariales", "cotisations_patronales", "cout_employeur"] {
        let d = decimal(&r_apres, k)? - decimal(&r_avant, k)?;
        tout_nul &= d.is_zero();
        delta.insert(k.to_string(), json!(d.to_string()));
    }
    let rien_ne_change = tout_nul
        && r_avant["cachets"] == r_apres["cachets"]
        && r_avant["heures"] == r_apres["heures"]
        && r_avant["jours"] == r_apres["jours"];
    Ok(json!({
        "mois": mois_v,
        "salarie": salarie,
        "debut": debut,
        "nom": fiche["nom"],
        "spectacle": fiche["spectacle"],
        "numero": fiche["numero"],
        "etat": etat,
        "avant": r_avant,
        "apres": r_apres,
        "delta": delta,
        "rien_ne_change": rien_ne_change,
        "avertissements": apres.avertissements.iter().map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "engagement": corrige,
    }))
}

// ------------------------------------------------- modifier la paie d'un mois deja declare

/// LA PAIE D'UN MOIS DEJA DECLARE SE MODIFIE EN LE SACHANT (Leandre, 21/10/2026). La cle de la charge qui confirme, pour
/// `enregistrer-saisie` (et toute route qui ecrit la saisie d'un mois, `routes_ecritures::ecrire_saisie`).
pub const CONFIRMATION: &str = "modifier_apres_echeance";

/// La classe du refus : la page la reconnait et montre l'avertissement, avec « Modifier quand meme » et « Annuler ».
pub const CLASSE_A_CONFIRMER: &str = "ModificationApresEcheance";

/// `septembre 2026`, avec ses accents.
fn mois_en_clair(mois: &str) -> String {
    let n: usize = mois.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(1);
    format!("{} {}", crate::envoi::MOIS_ACCENTS[n.clamp(1, 12) - 1], mois.get(..4).unwrap_or(""))
}

/// `de septembre 2026`, `d'octobre 2026`.
pub(crate) fn de_mois_en_clair(mois: &str) -> String {
    crate::envoi::de_mois(&mois_en_clair(mois))
}

/// La saisie d'un mois TELLE QUE DECLAREE, gardee quand on la modifie apres la limite de l'annule-et-remplace : la
/// correction (`Corriger ce mois`) se calcule depuis ce que la DSN deposee a declare, pas depuis la saisie modifiee.
/// A cote des corrections du mois, sous un nom que `routes_ecrites::toutes_les_corrections` ne lit pas.
pub(crate) fn chemin_saisie_declaree(ctx: &Contexte, mois: &str) -> PathBuf {
    ctx.racine.join("regularisations").join(format!("{}.saisie-declaree.json", mois))
}

/// Ce que la page dit d'un mois dont la DSN reelle est deposee, avant qu'on en modifie la paie (`saisie`,
/// `enregistrer-saisie`) ; `Null` quand aucune DSN reelle acceptee n'est partie.
///
/// * tant que l'annule-et-remplace est admis (jusqu'a la veille de l'echeance, minuit) : une phrase, rien a confirmer ;
/// * apres : `confirmer`, et l'avertissement que la page montre a la premiere modification. Il dit ce qui est VRAI : la DSN
///   deposee ne change plus ; la correction se declare dans la DSN du mois courant (celle qui embarque les corrections,
///   `produire_regularisations::corrections_a_porter`), une fois ecrite par « Corriger ce mois » -- modifier la saisie
///   ne l'ecrit pas, et l'ecrire est un acte (mot de passe du logiciel, `regularisation-ecrire`).
pub(crate) fn modification_de_la_paie(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Value> {
    let etat = etat_mois(m, ctx, &json!(mois))?;
    if !vrai(&etat["deposee"]) || vrai(&etat["refusee"]) {
        return Ok(Value::Null);
    }
    let veille = m.limite(mois)?;
    let de = de_mois_en_clair(mois);
    if !vrai(&etat["regularisable"]) {
        return Ok(json!({
            "confirmer": false,
            "limite": veille.iso(),
            "phrase": format!(
                "La DSN {} est déposée : la modifier fera partir un annule-et-remplace, jusqu'au {} à minuit.",
                de,
                veille.jj_mm_aaaa()
            ),
        }));
    }
    let porteur = de_mois_en_clair(&py_str(&etat["mois_courant"]));
    Ok(json!({
        "confirmer": true,
        "limite": veille.iso(),
        "mois_courant": etat["mois_courant"],
        "phrase": format!(
            "La DSN {} est déposée et ne peut plus être remplacée : modifier cette paie se confirme, et la correction se déclare dans la DSN {} par « Corriger ce mois ».",
            de, porteur
        ),
        "avertissement": format!(
            "La limite de la DSN {} est passée : l'annule-et-remplace n'est plus admis depuis le {} à minuit, on ne peut plus la remplacer. La DSN déposée ne changera pas : la correction se déclarera dans la DSN {}, une fois écrite par « Corriger ce mois » (étape DSN). Êtes-vous sûr de vouloir modifier cette paie ?",
            de,
            veille.jj_mm_aaaa(),
            porteur
        ),
    }))
}

/// Garde la saisie telle que declaree avant sa premiere modification apres la limite (`chemin_saisie_declaree`). Ne
/// garde rien si elle l'est deja, ou si la saisie du disque n'est plus celle que la derniere DSN reelle a declaree (son
/// empreinte, notee au depot, ne correspond plus) : ce ne serait pas la paie declaree.
pub(crate) fn garder_la_saisie_declaree(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<()> {
    let copie = chemin_saisie_declaree(ctx, mois);
    let saisie = ctx.racine.join("paies").join(format!("{}.json", mois));
    if copie.exists() || !saisie.exists() {
        return Ok(());
    }
    let faits = m.depots_reels(mois, &Value::Null)?;
    let notee = faits.last().and_then(|l| l.get("detail")).and_then(|d| d.get("empreinte_saisie")).filter(|e| vrai(e)).map(py_str);
    if let Some(n) = notee {
        if m.empreinte_saisie(mois)?.as_deref() != Some(n.as_str()) {
            return Ok(());
        }
    }
    if let Some(parent) = copie.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur("OSError", format!("{} : {}", parent.display(), e)))?;
    }
    std::fs::copy(&saisie, &copie).map_err(|e| erreur("OSError", format!("{} : {}", copie.display(), e)))?;
    Ok(())
}

/// Un contrat de la saisie modifiee, compare a la saisie telle que declaree.
pub(crate) struct ContratModifie {
    /// La cle du salarie, et le premier jour du contrat : ce qu'une correction designe.
    pub(crate) salarie: String,
    pub(crate) debut: String,
    /// L'engagement tel qu'il est saisi maintenant.
    pub(crate) engagement: Value,
}

/// Ce qui a change dans LA PAIE depuis que la saisie a ete declaree (`chemin_saisie_declaree`) : les contrats declares
/// dont la paie a change -- ce que leurs documents disent (`envoi::resume_de` : bornes, spectacle, jours, cachets, heures,
/// brut, net), calcule sur les deux saisies : la page reecrit la saisie a sa facon (cles, nombres), ce n'est pas un
/// changement --, et le nombre de contrats ajoutes ou retires (ceux-la ne sont pas des corrections : « Corriger ce mois »
/// ne corrige que les lignes d'un contrat declare). `None` sans copie de la saisie declaree, ou si l'une des deux ne se
/// calcule pas.
pub(crate) fn ce_qui_a_change(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<Option<(Vec<ContratModifie>, usize)>> {
    let copie = chemin_saisie_declaree(ctx, mois);
    let saisie = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !copie.exists() || !saisie.exists() {
        return Ok(None);
    }
    let declaree = crate::engagement::lire_saisie(&copie)?;
    let actuelle = crate::engagement::lire_saisie(&saisie)?;
    let (Ok(p_avant), Ok(p_apres)) = (paies_de_en(o, ctx, &declaree, true), paies_de_en(o, ctx, &actuelle, true)) else {
        return Ok(None);
    };
    let ids = |d: &Value| -> Option<Vec<Option<crate::jumeaux::Ident>>> {
        crate::routes_ecritures::identifiants(ctx, d).ok().map(|v| v.into_iter().map(Some).collect())
    };
    let (Some(avant), Some(apres)) = (ids(&declaree), ids(&actuelle)) else { return Ok(None) };
    let e_apres = actuelle.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let resume = |p: Option<&Paie>| p.and_then(|p| crate::envoi::resume_de(&[p]).ok());
    let paires = crate::jumeaux::apparier(&apres, &avant);
    let mut modifies = Vec::new();
    let mut hors = 0usize;
    for (i, j) in paires.iter().enumerate() {
        let (Some(Some(id)), Some(e)) = (apres.get(i), e_apres.get(i)) else { continue };
        match j {
            None => hors += 1,
            Some(j) => {
                let (r_avant, r_apres) = (resume(p_avant.get(*j)), resume(p_apres.get(i)));
                if r_avant.is_none() || r_avant != r_apres {
                    modifies.push(ContratModifie { salarie: id.0.clone(), debut: id.1.clone(), engagement: e.clone() });
                }
            }
        }
    }
    let gardes = paires.iter().filter(|j| j.is_some()).count();
    hors += avant.len().saturating_sub(gardes);
    Ok(Some((modifies, hors)))
}
