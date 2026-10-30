//! LE CODE RISQUE ET LE TAUX AT/MP DE LA COMPAGNIE (`taux-atmp`), dans sa surcouche du bareme
//! (`config/baremes/<annee>.json`, `taux_atmp` : `valeur`, `code_risque`).
//!
//! Ce que les textes disent (`atelier/reference/sources/tarification-atmp/LISEZ-MOI.md`) : la CARSAT notifie le code risque
//! et le taux ; l'employeur ne declare qu'un code notifie, et 999ZZ tant qu'il n'a pas de notification, sans taux (norme DSN,
//! S21.G00.40.040 et .043, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315). Le taux d'un risque « TC » de l'annexe 1 de
//! l'arrete de l'annee est collectif quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete) ; celui d'un autre risque
//! ne l'est que sous 20 salaries (D. 242-6-2).
//!
//! A LA CREATION d'une compagnie, le risque par defaut du socle de l'annee (`risques_atmp.par_defaut`, 92.3AD : « tout
//! intermittent du spectacle ») et son taux de l'arrete se posent d'eux-memes (`poser_a_la_creation`) ; l'ecran les dit
//! une fois, avec le libelle de l'arrete, l'explication, ou les lire et ou les changer (`lire`). Une annee que la compagnie
//! n'a pas dite reprend son code, et le taux de l'arrete de l'annee pour un risque TC (`Contexte::taux_atmp_reporte`). L'avance de l'ecran propose
//! les codes que le socle connait, ou un autre code notifie.

use std::path::Path;

use serde_json::{json, Map, Value};

use droit::bareme::{self, RisqueAtmp};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::ecriture::ecrire_json;
use socle::fichiers::{dossier_du_code, lire_json, vrai};
use socle::num::Decimal;
use socle::valeur::{nom_type, valeur_ecrite, valeur_en_texte};

/// La marque d'un taux que le moteur a pose a la creation (et que personne n'a encore change).
const POSE_A_LA_CREATION: &str = "_pose_a_la_creation";

fn surcouche(racine: &Path, an: i64) -> std::path::PathBuf {
    racine.join("config").join("baremes").join(format!("{}.json", an))
}

/// Le socle livre de l'annee, s'il existe.
fn socle_de(an: i64) -> Option<Value> {
    let p = dossier_du_code().join("baremes").join(format!("{}.json", an));
    p.is_file().then(|| lire_json(&p).ok()).flatten()
}

fn aujourdhui_jj_mm_aaaa() -> String {
    let iso = socle::paris::iso_secondes(socle::temps::secondes());
    format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4])
}

fn en_pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// LA FORME D'UN CODE RISQUE (S21.G00.40.040, `X [5,6]`) : trois chiffres et deux caracteres, suivis du « B » du taux bureau
/// s'il est sur la notification (`bloc-S21.G00.40.txt` l. 1229-1240) ; 999ZZ sans notification.
fn code_risque_valide(code: &str) -> bool {
    let c: Vec<char> = code.chars().collect();
    (c.len() == 5 || (c.len() == 6 && c[5] == 'B'))
        && c[..3].iter().all(|x| x.is_ascii_digit())
        && c[3..5].iter().all(|x| x.is_ascii_digit() || x.is_ascii_uppercase())
}

/// Moins de 20 salaries (D. 242-6-2, 1°), d'apres l'effectif que le moteur connait ; `None` s'il ne le sait pas.
fn moins_de_20(ctx: &Contexte, an: i64) -> Option<bool> {
    ctx.effectif().ok()?.atteint(20, an as i32).ok().map(|atteint| !atteint)
}

/// Ecrit `taux_atmp` dans la surcouche de l'annee ; le reste de la surcouche est garde.
fn ecrire(racine: &Path, an: i64, taux: Map<String, Value>) -> R<()> {
    let p = surcouche(racine, an);
    let lu = if p.exists() { crate::routes::mois::lire_saisie(&p)? } else { json!({}) };
    let mut data = if vrai(&lu) { lu } else { json!({"annee": an, "socle": an}) };
    let Value::Object(d) = &mut data else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&data))));
    };
    d.insert("taux_atmp".into(), Value::Object(taux));
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur("OSError", format!("{} : {}", parent.display(), e)))?;
    }
    if p.exists() {
        crate::routes::mois::sauver_avant(&p)?;
    }
    ecrire_json(&p, &data, 2).map_err(|m| erreur("OSError", m))
}

fn nombre(t: Decimal) -> Value {
    serde_json::Number::from_f64(t.to_string().parse::<f64>().unwrap_or(0.0)).map_or(Value::Null, Value::Number)
}

/// A LA CREATION D'UNE COMPAGNIE : le risque par defaut du socle de l'annee et le taux que l'arrete lui fixe. Une annee dont
/// le socle n'a pas de table des risques (son arrete n'est pas depose) ne pose rien : « Pour demarrer » le demande.
pub(crate) fn poser_a_la_creation(racine: &Path, an: i64) -> R<()> {
    let Some(socle) = socle_de(an) else { return Ok(()) };
    let Some(r) = bareme::risque_par_defaut(&socle) else { return Ok(()) };
    let Some(t) = bareme::taux_de_l_arrete(&r, None, false) else { return Ok(()) };
    let mut m = Map::new();
    m.insert("valeur".into(), nombre(t));
    m.insert("code_risque".into(), json!(r.code));
    m.insert(POSE_A_LA_CREATION.into(), json!(true));
    m.insert("_source".into(), json!(format!("Pose a la creation de la compagnie : le risque par defaut du socle {} et son taux collectif (atelier/reference/sources/tarification-atmp/).", an)));
    ecrire(racine, an, m)
}

/// POSE LE CODE RISQUE ET LE TAUX DE L'ANNEE `an`. `valeur` vide : le taux de l'arrete s'il s'applique au code (un risque
/// TC, ou un autre sous 20 salaries) ; sinon il se demande. 999ZZ n'a pas de taux (SIG-11). `source` dit d'ou vient la
/// valeur.
pub(crate) fn poser(ctx: &Contexte, an: i64, code: &str, valeur: Option<Decimal>, source: &str) -> R<()> {
    let code = code.trim_matches(socle::texte::blanc).replace(['.', ' '], "").to_uppercase();
    if !code_risque_valide(&code) {
        return Err(erreur(
            "ErreurOperation",
            format!(
                "Code risque « {} » : trois chiffres et deux lettres ou chiffres, suivis du « B » du taux bureau s'il est sur la notification (923AD, 602MDB) ; 999ZZ sans notification.",
                code
            ),
        ));
    }
    let socle = socle_de(an);
    let connu = socle.as_ref().and_then(|s| bareme::risques_atmp(s).into_iter().find(|r| r.code == code));
    let valeur = if code == bareme::SANS_NOTIFICATION {
        if valeur.is_some() {
            return Err(erreur(
                "ErreurOperation",
                "Avec 999ZZ (pas encore de notification), la DSN ne déclare aucun taux (norme DSN, S21.G00.40.043) : laissez le taux vide.".to_string(),
            ));
        }
        None
    } else {
        match valeur.or_else(|| connu.as_ref().and_then(|r| bareme::taux_de_l_arrete(r, moins_de_20(ctx, an), en_alsace_moselle(ctx)))) {
            Some(v) if v > Decimal::ZERO && v < Decimal::from(20) => Some(v),
            Some(v) => {
                return Err(erreur(
                    "ErreurOperation",
                    format!("Taux AT/MP de {} % : il se donne en pourcentage, par exemple 1,50 pour 1,50 %.", v.normalize()),
                ))
            }
            None => {
                return Err(erreur(
                    "ErreurOperation",
                    format!("Code risque {} : son taux est celui que la CARSAT vous a notifié (compte AT/MP sur net-entreprises) ; saisissez-le.", code),
                ))
            }
        }
    };
    let mut m = Map::new();
    if let Some(v) = valeur {
        m.insert("valeur".into(), nombre(v));
    }
    m.insert("code_risque".into(), json!(code));
    m.insert("_source".into(), json!(source));
    ecrire(&ctx.racine, an, m)
}

/// LE CODE ET LE TAUX QUE LA CARSAT NOTIFIE (retour DSN de nature 34, `envois::retours::taux_atmp_d_octets`) se posent
/// d'eux-memes dans la surcouche de l'annee de leur date d'effet : la compagnie n'a rien a recopier. Effet au 1er janvier :
/// le taux de l'annee. Effet en cours d'annee : `valeur` devient la liste datee `[{depuis, valeur}]`, le taux d'avant
/// (dit pour l'annee, ou repris) jusqu'a la date d'effet (`droit::bareme::taux_atmp_au`, et le bareme a la date). `_notification`
/// garde le jour de la notification et la date d'effet ; une notification plus ancienne que celle deja posee ne remplace
/// rien. `Ok(Some(phrase))` : rien n'est pose, et la phrase dit pourquoi et quoi faire (un changement de code en cours
/// d'annee, que la configuration ne date pas).
pub(crate) fn poser_notifie(ctx: &Contexte, n: &envois::retours::TauxAtmpNotifie, le: &str, source: &str) -> R<Option<String>> {
    let an = n.date_effet.an as i64;
    let code = n.code_risque.clone();
    if !code_risque_valide(&code) || code == bareme::SANS_NOTIFICATION || n.taux <= Decimal::ZERO || n.taux >= Decimal::from(20) {
        return Ok(Some(format!("{} : code risque {} et taux {} illisibles ; rien n'est posé. Le code et le taux se lisent sur la notification (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, code, n.taux)));
    }
    let deja = crate::routes::fiches::lire_tolerant(&surcouche(&ctx.racine, an))?.and_then(|v| v.get("taux_atmp").and_then(Value::as_object).cloned());
    let calcul = n.date_calcul.map(|j| j.iso()).unwrap_or_default();
    if let Some(avant) = deja.as_ref().and_then(|t| t.get("_notification")).and_then(|x| x.get("calcul")).and_then(Value::as_str) {
        if !calcul.is_empty() && avant > calcul.as_str() {
            return Ok(None);
        }
    }
    let premier_janvier = n.date_effet.mois == 1 && n.date_effet.jour == 1;
    let valeur = if premier_janvier {
        nombre(n.taux)
    } else {
        // Le taux d'avant la date d'effet : celui que l'annee dit, sinon celui qu'elle reprend.
        let avant = match &deja {
            Some(t) => Some(t.clone()),
            None => ctx.taux_atmp_de(an as i32).and_then(Result::ok),
        };
        let code_avant = avant.as_ref().and_then(|t| t.get("code_risque")).filter(|c| vrai(c)).map(valeur_en_texte);
        if code_avant.as_deref().is_some_and(|c| c != code) {
            return Ok(Some(format!(
                "{} : le code risque passe de {} à {} au {} ; rien n'est posé, car la configuration ne date pas le code risque. Saisissez le code et le taux notifiés dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                source, code_avant.unwrap_or_default(), code, n.date_effet.jj_mm_aaaa()
            )));
        }
        let effet = n.date_effet.iso();
        let mut liste: Vec<Value> = match avant.as_ref().and_then(|t| t.get("valeur")) {
            Some(Value::Array(a)) => a.iter().filter(|e| e.get("depuis").and_then(Value::as_str).is_some_and(|d| d < effet.as_str())).cloned().collect(),
            Some(v) if !v.is_null() => vec![json!({"depuis": format!("{}-01-01", an), "valeur": v})],
            _ => vec![],
        };
        liste.push(json!({"depuis": effet, "valeur": nombre(n.taux)}));
        Value::Array(liste)
    };
    let mut m = Map::new();
    m.insert("valeur".into(), valeur);
    m.insert("code_risque".into(), json!(code));
    m.insert("_source".into(), json!(source));
    m.insert("_notification".into(), json!({"le": le, "calcul": calcul, "effet": n.date_effet.iso()}));
    ecrire(&ctx.racine, an, m)?;
    Ok(None)
}

/// Un taux saisi : `1,5`, `1.50`, `1,50 %` ; vide : rien.
pub(crate) fn taux_saisi(v: Option<&Value>) -> R<Option<Decimal>> {
    let Some(v) = v.filter(|v| vrai(v)) else { return Ok(None) };
    let brut = valeur_en_texte(v).replace(',', ".").replace('%', "");
    let brut = brut.trim();
    if brut.is_empty() {
        return Ok(None);
    }
    socle::num::decimal_lu(brut)
        .map(Some)
        .ok_or_else(|| erreur("ErreurOperation", format!("Taux AT/MP illisible : {}.", valeur_ecrite(v))))
}

fn risque_en_json(r: &RisqueAtmp) -> Value {
    json!({
        "code": r.code, "code_arrete": r.code_arrete, "libelle": r.libelle, "precisions": r.precisions,
        "taux": r.taux.map(nombre), "tc": r.tc,
    })
}

/// CE QUE L'ECRAN DIT du code et du taux de l'annee : ce qui est pose, le risque tel que l'arrete l'ecrit, la phrase qui
/// l'explique, ou le lire, ou le changer, et les codes que l'avance propose.
pub(crate) fn lire(ctx: &Contexte, an: i64) -> R<Value> {
    // Ce que l'annee dit, ou, l'annee que la compagnie n'a pas dite, le taux qui se reprend d'une annee d'avant, ou
    // pourquoi il ne se reprend pas (`Contexte::taux_atmp_de`).
    let (pose, reporte, manque) = match ctx.taux_atmp_de(an as i32) {
        Some(Ok(t)) => {
            let de = t.get("_reporte_de").and_then(Value::as_i64);
            (Some(t), de, None)
        }
        Some(Err(m)) => (None, None, Some(m)),
        None => (None, None, None),
    };
    let code = pose.as_ref().and_then(|t| t.get("code_risque")).filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default();
    // Le taux en vigueur aujourd'hui, ramene dans l'annee (une liste datee : celui de sa date d'effet).
    let jour = match ctx.aujourdhui.an as i64 {
        a if a < an => socle::date::Jour::nouveau(an as i32, 1, 1),
        a if a > an => socle::date::Jour::nouveau(an as i32, 12, 31),
        _ => Some(ctx.aujourdhui),
    };
    let valeur = pose
        .as_ref()
        .and_then(|t| t.get("valeur"))
        .zip(jour)
        .and_then(|(v, j)| bareme::taux_atmp_au(v, &j))
        .map(nombre);
    let notification = pose.as_ref().and_then(|t| t.get("_notification")).cloned();
    let a_la_creation = pose.as_ref().is_some_and(|t| t.get(POSE_A_LA_CREATION).is_some_and(vrai));
    let socle = socle_de(an);
    let risques = socle.as_ref().map(bareme::risques_atmp).unwrap_or_default();
    let arrete = socle.as_ref().and_then(bareme::arrete_atmp);
    let connu = risques.iter().find(|r| r.code == code);
    let nom = |r: &RisqueAtmp| r.code_arrete.clone().unwrap_or_else(|| r.code.clone());
    let taux_dit = valeur.as_ref().map(|v| en_pourcent(socle::num::dec(v).unwrap_or_default()));
    let explication = match (connu, &taux_dit) {
        (Some(r), _) if r.code == bareme::SANS_NOTIFICATION => format!(
            "999ZZ : la compagnie n'a pas encore reçu de notification de la CARSAT. La DSN le déclare ainsi, sans taux. Aucun texte ne dit quel taux le bulletin cotise en attendant : le logiciel ne calcule pas la paie de {} tant que le code et le taux notifiés ne sont pas saisis ici.",
            an
        ),
        (Some(r), Some(t)) if r.tc && r.taux.map(en_pourcent).as_deref() != Some(t.as_str()) => format!(
            "Le taux de ce risque est collectif, le même pour toutes les compagnies qui y sont classées quel que soit leur effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}. Celui que la compagnie a saisi, {}, est celui que la paie applique ; s'il n'est pas sur la notification, corrigez-le dans « Avancé ».",
            arrete.clone().unwrap_or_else(|| format!("l'arrêté de tarification de {}", an)), r.taux.map(en_pourcent).unwrap_or_default(), an, t
        ),
        (Some(r), Some(t)) if r.tc => format!(
            "Le taux de ce risque est collectif : le même pour toutes les compagnies qui y sont classées, quel que soit leur effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}.",
            arrete.clone().unwrap_or_else(|| format!("l'arrêté de tarification de {}", an)), t, an
        ),
        (Some(r), Some(t)) if r.taux.map(en_pourcent).as_deref() == Some(t.as_str()) => format!(
            "Ce risque a un taux collectif sous 20 salariés (code de la sécurité sociale, art. D. 242-6-2) : {} en {} ({}). À partir de 20 salariés, le taux est celui que la CARSAT notifie.",
            t, an, arrete.clone().unwrap_or_default()
        ),
        (_, Some(t)) => format!("Taux de {} en {}, tel que la notification de la CARSAT le dit.", t, an),
        (_, None) if !code.is_empty() => format!("Code risque {} : le taux de {} manque ; il est sur la notification de la CARSAT.", code, an),
        _ => manque.unwrap_or_else(|| format!("Le code risque et le taux AT/MP de {} ne sont pas encore dits.", an)),
    };
    let explication = match reporte {
        Some(de) => format!("Le code risque est celui que la compagnie a dit pour {}. {}", de, explication),
        None => explication,
    };
    Ok(json!({
        "annee": an,
        "taux": pose.as_ref().map(|_| json!({"valeur": valeur, "code_risque": code})),
        "risque": connu.map(risque_en_json),
        "nom": connu.map(nom).unwrap_or_else(|| code.clone()),
        "resume": if code.is_empty() { String::new() } else { format!("Code risque {}{}", connu.map(nom).unwrap_or_else(|| code.clone()), taux_dit.as_ref().map(|t| format!(", taux {}", t)).unwrap_or_default()) },
        "pose_a_la_creation": a_la_creation,
        "reporte_de": reporte,
        "notification": notification,
        "explication": match (connu.and_then(|r| r.alsace_moselle.map(|g| (r, g))), en_alsace_moselle(ctx), &notification) {
            (_, _, Some(n)) => {
                let date = |k: &str| n.get(k).and_then(Value::as_str).and_then(|d| socle::date::Jour::lire(d).ok()).map(|j| j.jj_mm_aaaa());
                format!(
                    "Code risque {}{}, notifiés par la CARSAT{} et en vigueur depuis le {} : le logiciel les a lus dans la notification reçue avec les retours de la DSN, et posés de lui-même.",
                    connu.map(nom).unwrap_or_else(|| code.clone()),
                    taux_dit.as_ref().map(|t| format!(", taux {}", t)).unwrap_or_default(),
                    date("calcul").or_else(|| date("le")).map(|d| format!(" (notification du {})", d)).unwrap_or_default(),
                    date("effet").unwrap_or_default()
                )
            }
            (Some((r, (groupe, t))), true, None) => format!(
                "L'établissement est en Alsace-Moselle : l'annexe 3 de {} classe {} au groupe {}, à {} en {}. Le logiciel ne pose pas ce taux de lui-même (l'article D. 242-35, qui dit à quelles compagnies il s'applique, n'est pas relevé) : celui de la paie est celui que vous saisissez, tel que la notification de la CARSAT le dit. {}",
                arrete.clone().unwrap_or_else(|| format!("l'arrêté de tarification de {}", an)), nom(r), groupe, en_pourcent(t), an, taux_dit.as_ref().map(|t| format!("Taux saisi : {}.", t)).unwrap_or_default()
            ),
            _ => explication,
        },
        "ou_le_lire": "La CARSAT notifie à la compagnie son code risque et son taux : la notification se lit dans le compte AT/MP de net-entreprises (compte entreprise › Accidents du travail et maladies professionnelles).",
        "ou_le_changer": "Si la notification dit un autre code, ou un autre taux, ou si la compagnie n'en a pas encore reçu (999ZZ), changez-les dans « Avancé » ci-dessous.",
        "codes": risques.iter().map(risque_en_json).collect::<Vec<_>>(),
        "etablissements": crate::routes::ouverture::etablissements_dits(ctx).iter().map(|e| d_un_etablissement(e, an, socle.as_ref(), arrete.as_deref())).collect::<Vec<_>>(),
    }))
}

/// CE QUE L'ECRAN DIT du code et du taux d'un autre etablissement pour l'annee `an` : le taux est determine par
/// etablissement, le classement suivant l'activite qui y est exercee (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml` ;
/// arrete du 17/10/1995, art. 1, I, 1°) ; un code « TC » a le taux de l'arrete de l'annee quel que soit l'effectif
/// (D. 242-6-14, I), lu au meme endroit que pour la compagnie (`bareme::taux_atmp_applique`).
fn d_un_etablissement(e: &Value, an: i64, socle: Option<&Value>, arrete: Option<&str>) -> Value {
    let t = bareme::taux_atmp_d_un_etablissement(e, &an.to_string());
    let code = t.get("code_risque").map(valeur_en_texte).unwrap_or_default();
    let r = socle.and_then(|s| bareme::risques_atmp(s).into_iter().find(|r| r.code == code));
    let nom_code = r.as_ref().and_then(|r| r.code_arrete.clone()).unwrap_or_else(|| code.clone());
    let cp = e.get("code_postal").map(valeur_en_texte).unwrap_or_default();
    let taux = socle.and_then(|s| bareme::taux_atmp_applique(s, &t, bareme::en_alsace_moselle(&cp)));
    let arrete = arrete.map(String::from).unwrap_or_else(|| format!("l'arrêté de tarification de {}", an));
    let mut phrases = Vec::new();
    if t.get("_code_de_la_compagnie").is_some_and(vrai) {
        phrases.push(format!(
            "Le code risque {} est celui de la compagnie, posé par le logiciel. Le code d'un établissement suit l'activité qui y est exercée (arrêté du 17 octobre 1995, art. 1 ; code de la sécurité sociale, art. D. 242-6-1) : si la CARSAT notifie un autre code à cet établissement, changez-le ici.",
            nom_code
        ));
    }
    phrases.push(match (&taux, t.get("valeur").filter(|v| !v.is_null())) {
        (Some(_), Some(_)) => "Le taux est celui que vous avez saisi, tel que la notification de la CARSAT le dit.".to_string(),
        (Some(x), None) if r.as_ref().is_some_and(|r| r.tc) => format!(
            "Le taux de ce risque est collectif, le même quel que soit l'effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}.",
            arrete, en_pourcent(*x), an
        ),
        (Some(x), None) => format!(
            "Un établissement nouvellement créé a le taux collectif de son risque l'année de sa création et les deux années suivantes, quel que soit l'effectif (code de la sécurité sociale, art. D. 242-6-17, et {}) : {} en {}. Si la CARSAT notifie un autre taux à cet établissement, saisissez-le.",
            arrete, en_pourcent(*x), an
        ),
        (None, _) if code == bareme::SANS_NOTIFICATION => "999ZZ : pas encore de notification ; la paie de cet établissement attend le code et le taux notifiés.".to_string(),
        (None, _) => format!("Son taux est celui que la CARSAT notifie à cet établissement (compte AT/MP sur net-entreprises) : saisissez-le avec le code {}.", nom_code),
    });
    json!({
        "nic": e.get("nic").cloned().unwrap_or(Value::Null),
        "libelle": e.get("libelle").cloned().unwrap_or(Value::Null),
        "resume": format!("Code risque {}{}", nom_code, taux.map(|x| format!(", taux {}", en_pourcent(x))).unwrap_or_default()),
        "risque": r.as_ref().map(risque_en_json),
        "explication": phrases.join(" "),
    })
}

/// `taux-atmp` : `{"annee"}` lit ; `{"annee", "code_risque"?, "valeur"?}` pose d'abord (`poser`), puis lit. Un taux donne
/// seul garde le code deja pose pour l'annee ; sans code pose, il se refuse.
pub(crate) fn route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let an = crate::routes::fiches::entier(crate::routes::fiches::exiger(charge, "annee")?)?;
    let code = match charge.get("code_risque").filter(|c| vrai(c)) {
        Some(Value::String(c)) => Some(c.clone()),
        Some(autre) => return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", nom_type(autre)))),
        None if charge.get("valeur").is_some_and(vrai) => {
            let pose = crate::routes::fiches::lire_tolerant(&surcouche(&ctx.racine, an))?
                .and_then(|v| v.pointer("/taux_atmp/code_risque").filter(|c| vrai(c)).map(valeur_en_texte));
            Some(pose.ok_or_else(|| {
                Echec::from("Le code risque manque : il se dit avec le taux, tel que la notification de la CARSAT les écrit.".to_string())
            })?)
        }
        None => None,
    };
    if let Some(code) = code {
        let valeur = taux_saisi(charge.get("valeur"))?;
        let source = format!("Saisi dans PuppetPaie le {} (notification AT/MP).", aujourdhui_jj_mm_aaaa());
        poser(ctx, an, &code, valeur, &source)?;
    }
    lire(ctx, an)
}

/// L'etablissement de la fiche est-il en Alsace-Moselle (`droit::bareme::en_alsace_moselle`, par son code postal) ?
fn en_alsace_moselle(ctx: &Contexte) -> bool {
    ctx.entreprise.get("code_postal").map(valeur_en_texte).is_some_and(|c| bareme::en_alsace_moselle(&c))
}
