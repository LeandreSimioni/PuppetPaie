//! `regularisation-apercu` : ce que la correction d'un mois deja depose changerait -- avant, apres,
//! difference. Rien n'est ecrit.
//!
//! C'est le Python de `operations_regularisation.apercu_correction` et de ce qu'il appelle
//! (`etat_mois`, `engagements_d_origine`, `_engagement_declare`, `_paie_d_un_engagement`,
//! `_poser_numeros`, `numeros_declares`, `_dsn_deposees`, `lire_corrections`, `_normaliser`,
//! `_resume`), porte a l'identique : meme JSON, memes refus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use crate::etats::{self, get};
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::Decimal;
use socle::echec::Echec;
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
    charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

#[track_caller]
fn non_regularisable(message: impl Into<String>) -> Echec {
    erreur("MoisNonRegularisable", message)
}

/// `d["cle"]` : `KeyError` si elle manque.
pub(crate) fn requis<'a>(d: &'a Value, k: &str) -> R<&'a Value> {
    d.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

// ------------------------------------------------------------------ le mois d'origine

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
pub(crate) fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois_v: &Value) -> R<Value> {
    let e = m.dsn_du_mois(&valeur_en_texte(mois_v))?;
    let Some(mois) = mois_v.as_str() else {
        return Err(erreur("TypeError", format!("'mois' : {}", socle::fichiers::attendu("un texte", nom_type(mois_v)))));
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
        let suite = if passee { format!(" (en retard : `paie depot-dsn {} --test`, puis --reel)", mois) } else { String::new() };
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
            let p = dossier.join(valeur_en_texte(get(l, "fichier")?));
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

/// Les paies de la saisie `data` (une saisie de mois, celle de l'ecran ou celle qu'une DSN a declaree), calculees
/// (`engagement::charger_donnees`), rien d'ecrit.
pub(crate) fn paies_de(ctx: &Contexte, data: &Value) -> R<Vec<Paie>> {
    Ok(engagement::charger_donnees(ctx, data).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, data, m)))?.paies)
}

/// La saisie TELLE QUE DECLAREE quand elle a ete modifiee apres la limite de l'annule-et-remplace
/// (`chemin_saisie_declaree`) : c'est d'elle que la correction part ; sinon la saisie du mois.
pub(crate) fn saisie_d_origine(ctx: &Contexte, mois: &str) -> R<Value> {
    let declaree = chemin_saisie_declaree(ctx, mois);
    if declaree.exists() {
        return Ok(socle::fichiers::lire_json(&declaree)?);
    }
    engagement::stock::vue_du_mois(&ctx.racine, mois)?
        .ok_or_else(|| non_regularisable(format!("Aucun engagement ne touche {} : rien a corriger.", mois)))
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
            let k = (valeur_en_texte(&c["salarie"]), valeur_en_texte(&c["debut"]));
            portees.insert(k, c["engagement"].clone());
        }
    }
    Ok(portees)
}

/// `engagements_d_origine`, puis `_engagement_declare` : l'engagement du contrat tel que DECLARE, la
/// saisie du mois, et ce que la page montre du contrat (nom, spectacle, numero).
pub(crate) fn engagement_declare(m: &mut Moteur, ctx: &Contexte, mois: &str, salarie: &Value, debut: &Value) -> R<(Value, Value, Value)> {
    let donnees = saisie_d_origine(ctx, mois)?;
    let mut paies = paies_de(ctx, &donnees)?;
    poser_numeros(m, ctx, mois, &mut paies)?;
    let portees = engagements_portes(ctx, mois)?;
    let engs = donnees.get("engagements").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("engagements")))?;
    let engs = etats::items(engs)?;
    let mut trouve: Option<(Value, Value)> = None;
    for (p, e) in paies.iter().zip(engs.iter()) {
        let cle_s = valeur_en_texte(requis(&p.salarie, "_cle")?);
        let debut_s = p.engagement.debut.iso();
        let declare = portees.get(&(cle_s.clone(), debut_s.clone())).cloned().unwrap_or_else(|| e.as_ref().clone());
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => valeur_en_texte(v),
            None => valeur_en_texte(requis(sal, "prenom")?),
        };
        let fiche = json!({
            "nom": format!("{} {}", valeur_en_texte(requis(sal, "nom")?), prenom),
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
            "Aucun engagement de {} commencant le {} dans la saisie de {}. Un contrat jamais declare n'est pas une correction : c'est un contrat en retard, avec sa DPAE -- a saisir dans son mois, que sa DSN declare.",
            valeur_en_texte(salarie),
            valeur_en_texte(debut),
            mois
        ))),
    }
}

/// `_paie_d_un_engagement` : la paie d'UN engagement, calculee comme le mois l'a calculee, avec le
/// numero de contrat DECLARE.
pub(crate) fn paie_d_un_engagement(m: &mut Moteur, ctx: &Contexte, donnees: &Value, engagement: &Value) -> R<Paie> {
    let mut seule = donnees.clone();
    if let Some(obj) = seule.as_object_mut() {
        obj.insert("engagements".into(), Value::Array(vec![engagement.clone()]));
    }
    let mut paies = paies_de(ctx, &seule)?;
    let mois = valeur_en_texte(requis(donnees, "mois")?);
    poser_numeros(m, ctx, &mois, &mut paies)?;
    if paies.is_empty() {
        return Err(erreur("IndexError", socle::fichiers::trop_courte("la paie corrigée")));
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
                    return Err(erreur("TypeError", socle::fichiers::attendu("un objet", "list")));
                }
            }
            Value::String(s) => {
                if s.contains(k) {
                    return Err(erreur("TypeError", socle::fichiers::attendu("un objet", "str")));
                }
            }
            autre => return Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
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
        Some(h) if h != 0.0 => socle::ecriture::repr_flottant(h),
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
        return Err(non_regularisable(valeur_en_texte(&etat["raison"])));
    }
    let mois = valeur_en_texte(mois_v);
    // UN CONTRAT AJOUTE APRES LA LIMITE (21/10/2026) : rien de declare, sa correction le declare en entier.
    let change = match (salarie.as_str(), debut.as_str()) {
        (Some(sal), Some(deb)) => contrat_change(ctx, &mois, sal, deb)?,
        _ => None,
    };
    if let Some(c) = change.as_ref().filter(|c| c.ajoute) {
        return apercu_d_un_ajout(&mut m, ctx, charge, etat, &c.engagement);
    }
    // SES DATES CHANGEES DANS LA SAISIE (21/10/2026) : la correction les reprend (`phrase_des_dates` dit ce que la DSN suivante en declare).
    let nouvelles = change.filter(|c| c.dates_changees).map(|c| c.bornes);
    let (donnees, declare, fiche) = engagement_declare(&mut m, ctx, &mois, salarie, debut)?;
    let avant = paie_d_un_engagement(&mut m, ctx, &donnees, &declare)?;
    if !avant.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) {
        // Sans le numero sous lequel le contrat a ete DECLARE, la correction ne designe rien.
        return Err(non_regularisable(format!(
            "Le numero de contrat declare pour {} (engagement du {}) est introuvable : la saisie de {} ne le porte pas, et la DSN deposee de ce mois n'est pas relisible dans sorties/{}/. Une correction doit citer le numero d'origine (S21.G00.40.009), il ne se recalcule pas. Remettre la DSN deposee dans sorties/{}/ suffit.",
            valeur_en_texte(salarie),
            valeur_en_texte(debut),
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
    // Les bornes : celles du contrat declare, ou celles que la saisie lui donne maintenant -- jamais celles que les
    // lignes de la correction suggereraient (un cachet ajoute en debut de mois ne deplace pas le contrat).
    let declarees = bornes(&avant);
    let cible = nouvelles.clone().unwrap_or_else(|| declarees.clone());
    if let Some(n) = &nouvelles {
        poser_les_bornes(&mut corrige, n);
    }
    let apres = paie_d_un_engagement(&mut m, ctx, &donnees, &corrige)?;
    if bornes(&apres) != cible {
        return Err(non_regularisable(format!(
            "La correction deplace les bornes du contrat {} ({} -> {}). Le contrat garde les dates de la saisie : seules ses lignes se corrigent ici.",
            avant.engagement.numero.clone().unwrap_or_default(),
            format!("{}..{}", cible.0, cible.1.clone().unwrap_or_else(|| "sans fin".into())),
            format!("{}..{}", apres.engagement.debut.iso(), texte_fin(&apres))
        )));
    }
    let dates = match nouvelles.as_ref() {
        Some(n) => Some(phrase_des_dates(&mois, &declarees, n)?),
        None => None,
    };
    let (r_avant, r_apres) = (resume(&avant), resume(&apres));
    let mut delta = Map::new();
    let mut tout_nul = true;
    for k in ["brut", "net_a_payer", "net_imposable", "cotisations_salariales", "cotisations_patronales", "cout_employeur"] {
        let d = decimal(&r_apres, k)? - decimal(&r_avant, k)?;
        tout_nul &= d.is_zero();
        delta.insert(k.to_string(), json!(d.to_string()));
    }
    difference_d_activite(&mut delta, &r_avant, &r_apres)?;
    let rien_ne_change = tout_nul
        && dates.is_none()
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
        "dates": dates,
        "avertissements": apres.avertissements.iter().map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "engagement": corrige,
    }))
}

/// La difference d'activite (cachets, heures) entre deux paies resumees (`resume`), dans `delta` : la page la montre a
/// cote de celle des montants, sans la calculer (regle 2).
fn difference_d_activite(delta: &mut Map<String, Value>, avant: &Value, apres: &Value) -> R<()> {
    let entier = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    delta.insert("cachets".into(), json!(entier(apres, "cachets") - entier(avant, "cachets")));
    let d = decimal(apres, "heures")? - decimal(avant, "heures")?;
    delta.insert("heures".into(), json!(d.normalize().to_string()));
    Ok(())
}

/// Ce que la page dit d'une correction qui change les dates du contrat : les dates declarees, les corrigees, ce que la
/// DSN suivante en declare (`les_dates_d_un_contrat_declare`), et, quand le contrat court sur d'autres mois, que chacun
/// se corrige pour lui-meme.
fn phrase_des_dates(mois: &str, declarees: &Bornes, corrigees: &Bornes) -> R<String> {
    let mut quoi = Vec::new();
    if declarees.0 != corrigees.0 {
        quoi.push("la nouvelle date de début avec l'ancienne, dans un bloc « Changements contrat » (S21.G00.41.021, fiche net-entreprises 1952)");
    }
    if declarees.1 != corrigees.1 {
        quoi.push("la nouvelle fin dans un bloc « Fin du contrat » (S21.G00.62, fiche net-entreprises 1722) quand le contrat a fini");
    }
    let mut phrase = format!(
        "Dates corrigées : {} (déclaré : {}). La DSN suivante déclare {}.",
        bornes_en_clair(corrigees),
        bornes_en_clair(declarees),
        quoi.join(", et ")
    );
    let autres: Vec<String> = mois_des_bornes(corrigees)?.into_iter().chain(mois_des_bornes(declarees)?).filter(|m| m != mois).fold(Vec::new(), |mut v, m| {
        if !v.contains(&m) {
            v.push(m);
        }
        v
    });
    if !autres.is_empty() {
        phrase.push_str(&format!(
            " Le contrat court aussi sur {} : chaque mois se corrige pour lui-même, et la DSN suivante porte un versement par mois civil (cahier technique DSN §2.2.2.5).",
            autres.iter().map(|m| mois_en_clair(m)).collect::<Vec<_>>().join(" et ")
        ));
    }
    Ok(phrase)
}

/// Les mois (AAAA-MM) que couvrent des bornes ; sans fin, le seul premier mois.
fn mois_des_bornes(b: &Bornes) -> R<Vec<String>> {
    let debut = socle::date::Jour::lire(&b.0).map_err(|m| erreur("ValueError", m))?;
    let fin = match &b.1 {
        Some(f) => socle::date::Jour::lire(f).map_err(|m| erreur("ValueError", m))?,
        None => debut,
    };
    Ok(socle::date::mois_entre(&debut, &fin))
}

// ------------------------------------------------- modifier la paie d'un mois deja declare

/// LA PAIE D'UN MOIS DEJA DECLARE SE MODIFIE EN LE SACHANT (Leandre, 21/10/2026). La cle de la charge qui confirme, pour
/// `enregistrer-saisie` (et toute route qui ecrit la saisie d'un mois, `routes_ecritures::ecrire_saisie`).
pub const CONFIRMATION: &str = "modifier_apres_echeance";

/// La classe du refus : la page la reconnait et montre l'avertissement, avec « Modifier quand meme » et « Annuler ».
pub const CLASSE_A_CONFIRMER: &str = "ModificationApresEcheance";

/// La classe du refus de ce qui ne se declare pas apres la limite (`refus_non_declarable`) : la page le dit et recharge
/// la saisie enregistree.
pub const CLASSE_NON_DECLARABLE: &str = "ModificationNonDeclarable";

/// `septembre 2026`, avec ses accents.
fn mois_en_clair(mois: &str) -> String {
    socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
}

/// `de septembre 2026`, `d'octobre 2026`.
pub(crate) fn de_mois_en_clair(mois: &str) -> String {
    crate::envoi::de_mois(&mois_en_clair(mois))
}

/// La saisie d'un mois TELLE QUE DECLAREE, gardee quand on la modifie apres la limite de l'annule-et-remplace : la
/// correction se calcule depuis ce que la DSN deposee a declare, pas depuis la saisie modifiee.
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
///   `produire_regularisations::corrections_a_porter`), une fois ecrite (`depart::corrections_dues`) -- modifier la saisie
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
    let porteur = de_mois_en_clair(&valeur_en_texte(&etat["mois_courant"]));
    Ok(json!({
        "confirmer": true,
        "limite": veille.iso(),
        "mois_courant": etat["mois_courant"],
        "phrase": format!(
            "La DSN {} est déposée et ne peut plus être remplacée : modifier cette paie se confirme, et la correction se déclare dans la DSN {}.",
            de, porteur
        ),
        "avertissement": format!(
            "La limite de la DSN {} est passée : l'annule-et-remplace n'est plus admis depuis le {} à minuit, on ne peut plus la remplacer. La DSN déposée ne changera pas : la correction se déclarera dans la DSN {}. Êtes-vous sûr de vouloir modifier cette paie ?",
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
    if copie.exists() {
        return Ok(());
    }
    let Some(saisie) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(()) };
    let faits = m.depots_reels(mois, &Value::Null)?;
    let notee = faits.last().and_then(|l| l.get("detail")).and_then(|d| d.get("empreinte_saisie")).filter(|e| vrai(e)).map(valeur_en_texte);
    if let Some(n) = notee {
        if m.empreinte_saisie(mois)?.as_deref() != Some(n.as_str()) {
            return Ok(());
        }
    }
    socle::ecriture::ecrire_json(&copie, &saisie, 2).map_err(|m| erreur("OSError", m))?;
    Ok(())
}

/// Un contrat de la saisie dont la paie n'est plus celle que la DSN deposee declare.
pub(crate) struct ContratModifie {
    /// La cle du salarie, et le premier jour du contrat : ce qu'une correction designe.
    pub(crate) salarie: String,
    pub(crate) debut: String,
    /// L'engagement tel qu'il est saisi maintenant.
    pub(crate) engagement: Value,
    /// Un contrat que la DSN deposee ne porte pas : ajoute apres elle (un contrat oublie).
    pub(crate) ajoute: bool,
    /// Ses bornes saisies maintenant (premier jour, fin), telles que le moteur les lit.
    pub(crate) bornes: Bornes,
    /// Un contrat declare dont les dates ont change (21/10/2026) : sa correction en reprend les dates, et la DSN
    /// suivante les declare (`apercu_correction`).
    pub(crate) dates_changees: bool,
}

/// Les bornes d'un contrat : son premier jour et sa fin (ISO), telles que le moteur les lit.
pub(crate) type Bornes = (String, Option<String>);

fn bornes(p: &Paie) -> Bornes {
    (p.engagement.debut.iso(), p.engagement.fin.map(|f| f.iso()))
}

/// « du 02/07/2026 au 25/07/2026 ».
pub(crate) fn bornes_en_clair(b: &Bornes) -> String {
    let jour = |t: &str| socle::date::Jour::lire(t).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| t.to_string());
    match &b.1 {
        Some(f) => format!("du {} au {}", jour(&b.0), jour(f)),
        None => format!("à partir du {}, sans fin", jour(&b.0)),
    }
}

/// Ce qui a change dans la saisie depuis qu'elle a ete declaree.
pub(crate) struct Changements {
    /// Les contrats declares dont la paie a change, et les contrats ajoutes.
    pub(crate) contrats: Vec<ContratModifie>,
    /// Les contrats declares que la saisie ne porte plus (le moteur les refuse apres la limite, `refus_non_declarable`).
    pub(crate) retires: usize,
}

/// Les engagements d'une saisie, identifies (`jumeaux`) ; `None` s'ils ne s'identifient pas.
fn identifies(ctx: &Contexte, d: &Value) -> Option<Vec<Option<droit::jumeaux::Ident>>> {
    crate::routes_ecritures::identifiants(ctx, d).ok().map(|v| v.into_iter().map(Some).collect())
}

/// Ce qui a change dans LA PAIE depuis que la saisie a ete declaree (`chemin_saisie_declaree`) : les contrats declares
/// dont la paie a change -- ce que la paie calculee en dit (`resume` : montants, cachets, heures, jours), calculee sur
/// les deux saisies : la page reecrit la saisie a sa facon (cles, nombres), ce n'est pas un changement --, les contrats
/// ajoutes, et le nombre de contrats retires. `None` sans copie de la saisie declaree, ou si l'une des deux ne se
/// calcule pas.
pub(crate) fn ce_qui_a_change(ctx: &Contexte, mois: &str) -> R<Option<Changements>> {
    let copie = chemin_saisie_declaree(ctx, mois);
    if !copie.exists() {
        return Ok(None);
    }
    let Some(actuelle) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let declaree = socle::fichiers::lire_json(&copie)?;
    let (Ok(p_avant), Ok(p_apres)) = (paies_de(ctx, &declaree), paies_de(ctx, &actuelle)) else {
        return Ok(None);
    };
    let (Some(avant), Some(apres)) = (identifies(ctx, &declaree), identifies(ctx, &actuelle)) else { return Ok(None) };
    let e_apres = actuelle.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    // Le meme contrat a d'autres dates garde son numero (`jumeaux::apparier_meme_numero`) : il designe le contrat
    // declare, a son premier jour declare.
    let paires = droit::jumeaux::apparier_meme_numero(&apres, &avant);
    let mut contrats = Vec::new();
    for (i, j) in paires.iter().enumerate() {
        let (Some(Some(id)), Some(e), Some(p)) = (apres.get(i), e_apres.get(i), p_apres.get(i)) else { continue };
        let ajoute = j.is_none();
        let (debut, dates_changees, change) = match j {
            None => (id.1.clone(), false, true),
            Some(j) => {
                let d = avant.get(*j).and_then(|x| x.as_ref()).map_or_else(|| id.1.clone(), |x| x.1.clone());
                let dates = p_avant.get(*j).map(bornes) != Some(bornes(p));
                (d, dates, dates || p_avant.get(*j).map(resume) != Some(resume(p)))
            }
        };
        if change {
            contrats.push(ContratModifie { salarie: id.0.clone(), debut, engagement: e.clone(), ajoute, bornes: bornes(p), dates_changees });
        }
    }
    let gardes = paires.iter().filter(|j| j.is_some()).count();
    Ok(Some(Changements { contrats, retires: avant.len().saturating_sub(gardes) }))
}

/// UN CONTRAT DECLARE NE SE RETIRE PAS APRES LA LIMITE (21/10/2026). Ce que la norme permet apres la date
/// d'exigibilite (cahier technique DSN 2026.1, §2.2.2.5 et §2.1.3.6,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : des rappels de paie dans une DSN
/// suivante, et, pour un contrat, le changement d'une donnee par bloc « Changements contrat » (S21.G00.41). Rien
/// n'annule un contrat declare : seule une FIN de contrat s'annule (motif S21.G00.62.002 = 099, fiche net-entreprises
/// 1722, `atelier/reference/sources/net-entreprises-base-connaissances/`). Un contrat qui n'a pas eu lieu garde donc son
/// bloc 40 et voit ses montants annules (ses quantites a zero : la correction les declare en negatif).
///
/// SES DATES SE CORRIGENT (21/10/2026) : le contrat garde son numero, il se retrouve a ses nouvelles dates
/// (`jumeaux::apparier_meme_numero`), et sa correction les declare : l'ancienne date de debut en bloc 41 (fiche
/// net-entreprises 1952), la fin en bloc 62 (fiche 1722), un versement par mois civil (§2.2.2.5). Les autres mois du
/// contrat suivent (`propager_les_bornes`).
///
/// `base` : la saisie telle que declaree ; `nouvelle` : ce qu'on veut ecrire. `Ok(None)` : rien d'indeclarable.
pub(crate) fn refus_non_declarable(ctx: &Contexte, mois: &str, base: &Value, nouvelle: &Value) -> R<Option<String>> {
    let (Some(avant), Some(apres)) = (identifies(ctx, base), identifies(ctx, nouvelle)) else { return Ok(None) };
    let paires = droit::jumeaux::apparier_meme_numero(&avant, &apres);
    let e_avant = base.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let de = de_mois_en_clair(mois);
    for (i, j) in paires.iter().enumerate() {
        let Some(Some(id)) = avant.get(i) else { continue };
        let nom = e_avant.get(i).and_then(|e| e.get("salarie")).map(valeur_en_texte).unwrap_or_else(|| id.0.clone());
        let debut = socle::date::Jour::lire(&id.1).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| id.1.clone());
        if j.is_none() {
            return Ok(Some(format!(
                "Le contrat de {} du {} est déclaré dans la DSN {}, dont l'annule-et-remplace est passé : il ne se retire plus. La norme DSN n'annule pas un contrat déclaré (seule une fin de contrat s'annule, motif 099, fiche net-entreprises 1722). S'il n'a pas eu lieu, gardez-le et mettez ses quantités à zéro : la DSN suivante déclarera l'annulation de ses montants dans la DSN suivante (rappel de paie, cahier technique DSN §2.1.3.6) ; sa DPAE s'annule auprès de l'Urssaf. Rien n'est enregistré.",
                nom, debut, de
            )));
        }
    }
    Ok(None)
}

/// LA DATE D'EMBAUCHE DECLAREE d'un contrat dont la saisie a change le premier jour apres la limite (21/10/2026) : son
/// premier jour dans une saisie telle que declaree (`chemin_saisie_declaree`, n'importe quel mois), meme salarie, meme
/// numero, autre jour. `None` : il n'a pas change (ou rien n'a ete garde).
pub(crate) fn debut_declare_avant(ctx: &Contexte, cle: &str, numero: &str, debut: &str) -> Option<String> {
    crate::routes_mois::mois_saisis(&ctx.racine).into_iter().find_map(|m| {
        let copie = chemin_saisie_declaree(ctx, &m);
        let declaree = socle::fichiers::lire_json(&copie).ok()?;
        let ids = crate::routes_ecritures::identifiants(ctx, &declaree).ok()?;
        ids.into_iter().find(|(s, d, n)| s == cle && n.as_deref() == Some(numero) && d != debut).map(|(_, d, _)| d)
    })
}

/// La DPAE d'un contrat dont la date d'embauche a change (`debut_declare_avant`), deposee pour l'ancienne date `avant` :
/// vaut-elle pour la nouvelle `apres`, et la phrase qui le dit. Code du travail, `atelier/reference/sources/code-du-travail/`
/// (`xml/L1221-10.xml`, `xml/R1221-1.xml`, `xml/R1221-4.xml`) : « L'embauche d'un salarie ne peut intervenir qu'apres
/// declaration nominative » (L. 1221-10) ; la DPAE porte la « Date et heure d'embauche » (R. 1221-1, 3°) et part au plus
/// tot huit jours avant « la date previsible de l'embauche » (R. 1221-4). INFERENCE : reculee, l'embauche a eu lieu apres
/// la declaration, qui portait sa date previsible -- elle vaut ; avancee, l'embauche a precede la declaration -- une DPAE
/// a la date vraie est a deposer. La DPAE n'a pas d'annule-et-remplace (guide de l'API DPAE de l'Urssaf,
/// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3).
pub(crate) fn dpae_d_une_date_changee(avant: &str, apres: &str) -> (bool, String) {
    let jour = |t: &str| socle::date::Jour::lire(t).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| t.to_string());
    if avant < apres {
        (
            true,
            format!(
                "Embauche reportée du {} au {} : la DPAE déposée pour le {} vaut — elle a précédé l'embauche (code du travail, art. L. 1221-10) et portait sa date prévisible (art. R. 1221-4). Rien à refaire.",
                jour(avant),
                jour(apres),
                jour(avant)
            ),
        )
    } else {
        (
            false,
            format!(
                "Embauche avancée du {} au {} : la DPAE déposée pour le {} ne l'a pas précédée (code du travail, art. L. 1221-10) et porte une autre date d'embauche (art. R. 1221-1, 3°). Une DPAE au {} est à déposer : une DPAE ne se remplace pas (pas d'annule-et-remplace, guide de l'API DPAE de l'Urssaf).",
                jour(avant),
                jour(apres),
                jour(avant),
                jour(apres)
            ),
        )
    }
}

/// Ce qui reste a ecrire pour que la DSN suivante declare la paie d'aujourd'hui (`etat_remplacement`).
pub(crate) struct Couverture {
    /// Les contrats changes sans correction ecrite.
    pub(crate) a_ecrire: usize,
    /// Les contrats dont la correction ecrite ne declare plus la paie d'aujourd'hui (la saisie a encore change).
    pub(crate) a_refaire: usize,
    /// Les contrats declares retires de la saisie (d'avant le refus).
    pub(crate) retires: usize,
    /// Les contrats a corriger, ceux de `a_ecrire` et de `a_refaire` : `(salarie, debut, engagement saisi)`.
    pub(crate) a_corriger: Vec<(String, String, Value)>,
}

/// LA CORRECTION ECRITE DECLARE-T-ELLE LA PAIE D'AUJOURD'HUI ? Pour chaque contrat change, la derniere correction non
/// annulee (en attente, preparee ou portee) : sa paie, et celle que la meme correction aurait avec les lignes saisies
/// aujourd'hui (`normaliser` : l'identite du contrat declare, les lignes de la saisie), calculees de la meme facon
/// (`paie_d_un_engagement`) et comparees par ce qu'elles declarent (`resume`). `None` sans copie de la saisie declaree.
pub(crate) fn couverture(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<Option<Couverture>> {
    let Some(ch) = ce_qui_a_change(ctx, mois)? else { return Ok(None) };
    let ecrites = crate::routes_ecrites::lire_corrections(ctx, mois)?;
    let donnees = saisie_d_origine(ctx, mois)?;
    let mut m = Moteur::nouveau(o, ctx);
    let mut c = Couverture { a_ecrire: 0, a_refaire: 0, retires: ch.retires, a_corriger: Vec::new() };
    for x in &ch.contrats {
        let derniere = ecrites.iter().rev().find(|e| {
            !e.get("annulee_le").is_some_and(vrai)
                && e.get("salarie").map(valeur_en_texte).as_deref() == Some(x.salarie.as_str())
                && e.get("debut").map(valeur_en_texte).as_deref() == Some(x.debut.as_str())
        });
        let Some(ecrite) = derniere else {
            c.a_ecrire += 1;
            c.a_corriger.push((x.salarie.clone(), x.debut.clone(), x.engagement.clone()));
            continue;
        };
        let corrige = ecrite.get("engagement").cloned().unwrap_or(Value::Null);
        let a_jour = normaliser(&corrige, &x.engagement).ok().and_then(|mut aujourdhui| {
            // Les dates d'aujourd'hui aussi : une correction ecrite a d'autres dates ne declare plus la saisie.
            poser_les_bornes(&mut aujourdhui, &x.bornes);
            let p1 = paie_d_un_engagement(&mut m, ctx, &donnees, &corrige).ok()?;
            let p2 = paie_d_un_engagement(&mut m, ctx, &donnees, &aujourdhui).ok()?;
            Some((resume(&p1), bornes(&p1)) == (resume(&p2), bornes(&p2)))
        });
        if a_jour != Some(true) {
            c.a_refaire += 1;
            c.a_corriger.push((x.salarie.clone(), x.debut.clone(), x.engagement.clone()));
        }
    }
    Ok(Some(c))
}

/// Le contrat `(salarie, debut)` de la saisie d'aujourd'hui dont la paie a change depuis la DSN deposee, ajoute ou
/// declare (`ce_qui_a_change`), s'il en est un.
pub(crate) fn contrat_change(ctx: &Contexte, mois: &str, salarie: &str, debut: &str) -> R<Option<ContratModifie>> {
    Ok(ce_qui_a_change(ctx, mois)?.and_then(|ch| ch.contrats.into_iter().find(|c| c.salarie == salarie && c.debut == debut)))
}

/// Pose les bornes `b` sur un engagement (une fin absente : `null`, que `Engagement` relit comme un contrat sans fin).
fn poser_les_bornes(e: &mut Value, b: &Bornes) {
    if let Some(o) = e.as_object_mut() {
        o.insert("debut".into(), json!(b.0));
        o.insert("fin".into(), b.1.as_ref().map_or(Value::Null, |f| json!(f)));
    }
}

/// Une paie a zero : ce qu'un contrat ajoute avait de declare (rien).
fn resume_nul() -> Value {
    json!({
        "brut": "0.00", "net_a_payer": "0.00", "net_imposable": "0.00", "cotisations_salariales": "0.00",
        "cotisations_patronales": "0.00", "cout_employeur": "0.00", "cachets": 0, "heures": "0", "jours": 0,
        "remuneration": [],
    })
}

/// L'apercu de la correction d'un CONTRAT AJOUTE apres la limite (un contrat oublie). Cahier technique DSN 2026.1,
/// §2.1.2.1 (« une correction est un evenement connu apres la date de cloture de la paie. Il donne lieu a la declaration
/// d'une correction dans la DSN suivante ») et §2.2.2.5 (« les corrections doivent etre portees par une DSN mensuelle
/// ulterieure [...] valorisees mois civil par mois civil ») ; fiche net-entreprises 1741 : un contrat que le systeme DSN
/// ne connait pas encore se declare en entier dans la DSN mensuelle qui le porte. La DSN suivante porte donc son bloc
/// 40 (S21.G00.40.001 = sa date de debut, passee) et un versement dont les remunerations et les bases sont rattachees au
/// mois d'origine (S21.G00.51.001/002, S21.G00.78.002/003) : `dsn::bloc_regularisation`, sur un declare vide. Rien
/// n'a ete declare, rien n'a ete verse : l'« avant » est nul.
fn apercu_d_un_ajout(
    m: &mut Moteur, ctx: &Contexte, charge: &Value, etat: Value, saisi: &Value,
) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = valeur_en_texte(mois_v);
    let donnees = saisie_d_origine(ctx, &mois)?;
    let corrige = normaliser(saisi, cle(charge, "engagement")?)?;
    let apres = paie_d_un_engagement(m, ctx, &donnees, &corrige)?;
    let (r_avant, r_apres) = (resume_nul(), resume(&apres));
    let mut delta = Map::new();
    let mut tout_nul = true;
    for k in ["brut", "net_a_payer", "net_imposable", "cotisations_salariales", "cotisations_patronales", "cout_employeur"] {
        let d = decimal(&r_apres, k)?;
        tout_nul &= d.is_zero();
        delta.insert(k.to_string(), json!(d.to_string()));
    }
    difference_d_activite(&mut delta, &r_avant, &r_apres)?;
    let rien_ne_change = tout_nul && r_apres["cachets"] == json!(0) && r_apres["heures"] == json!("0");
    let sal = &apres.salarie;
    let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => valeur_en_texte(requis(sal, "prenom")?),
    };
    Ok(json!({
        "mois": mois_v,
        "salarie": cle(charge, "salarie")?,
        "debut": cle(charge, "debut")?,
        "nom": format!("{} {}", valeur_en_texte(requis(sal, "nom")?), prenom),
        "spectacle": apres.titre()?,
        "numero": apres.engagement.numero,
        "etat": etat,
        "avant": r_avant,
        "apres": r_apres,
        "delta": delta,
        "rien_ne_change": rien_ne_change,
        "ajout": true,
        "avertissements": apres.avertissements.iter().map(|a| a.texte.clone()).collect::<Vec<_>>(),
        "engagement": corrige,
    }))
}

/// LE NUMERO D'UN CONTRAT AJOUTE, quand sa correction s'ecrit pour partir dans la DSN suivante : la correction le porte,
/// la DSN le declare sous ce numero (S21.G00.40.009), et il ne se recalcule plus (regle 4). S'il n'en a pas encore, il
/// le prend la (`engagement::numeros::numero_au_depart`).
pub(crate) fn figer_le_numero(ctx: &Contexte, mois: &str, salarie: &str, debut: &str) -> R<String> {
    let data = engagement::saisie_du_mois(&ctx.racine, mois)?;
    let ids = crate::routes_ecritures::identifiants(ctx, &data).map_err(|e| erreur("ValueError", e))?;
    let rang = ids
        .iter()
        .position(|(s, d, _)| s == salarie && d == debut)
        .ok_or_else(|| non_regularisable(format!("Aucun engagement de {} commencant le {} dans la saisie de {}.", salarie, debut, mois)))?;
    if let Some((_, _, Some(n))) = ids.get(rang) {
        return Ok(n.clone());
    }
    let numeros = engagement::numeros::numero_au_depart(ctx, mois, &[(rang, None)]).map_err(Echec::from)?;
    Ok(numeros.into_iter().next().unwrap_or_default())
}
