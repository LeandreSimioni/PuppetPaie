//! L'IMPORT QUI PARAMETRE LA COMPAGNIE : ce que les DSN reprises disent de la configuration et que la fiche Entreprise ne
//! porte pas, pose apres l'ecriture des fiches (`ecrite::importer`). La fiche Entreprise recoit deja des DSN ce qu'elle
//! declare (`dsn_aem::lire_une_dsn` : l'entreprise, le compte qui paie, le numero Conges Spectacles, la licence, le
//! contact DSN, la medecine du travail interne) ; ici se pose ce qui vit ailleurs :
//!
//! - LE CODE RISQUE ET LE TAUX AT/MP (S21.G00.40.040, .043 ; `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315) dans la
//!   surcouche du bareme de chaque annee (`routes::atmp::poser`). Une annee dont une DSN est reprise recoit le code et le
//!   taux qu'elle declare, s'ils n'y sont pas deja dits par la compagnie. L'annee en cours, sans DSN, recoit le code de la
//!   derniere DSN a la place de celui pose a la creation ou repris d'une annee d'avant, s'il est autre : le code reste celui que la CARSAT a notifie tant qu'elle n'en
//!   notifie pas un autre (inference : la norme dit que l'employeur declare un code « qui lui a deja ete notifie ») ; son
//!   taux est celui que l'arrete de l'annee fixe a ce risque (`droit::bareme::taux_de_l_arrete`), et sinon il se demande.
//! - LES ORGANISMES ET LES CONTRATS DE LA DSN (`config/organismes.json`, `compagnie::organismes`) : chaque contrat
//!   que l'ecran attend encore (`organismes-contrats`, ceux des permanents des que la compagnie en emploie : `poser_organismes_repris`) recoit la reference
//!   (S21.G00.15.001) et l'organisme (S21.G00.15.002) d'une adhesion de la derniere DSN (`norme-dsn/texte/bloc-S21.G00.15.txt`).
//!   Un contrat dont la reference est fixee (celui de la branche) n'a rien a recevoir ; un contrat a saisir recoit
//!   l'adhesion de la DSN qu'aucun contrat fixe ne porte, s'il n'y en a qu'une et qu'un seul contrat l'attend (inference :
//!   c'est la sienne) ; sinon rien ne se choisit, et « Pour demarrer » le demande.

use serde_json::{json, Map, Value};

use crate::import::dsn_aem::LectureDsn;
use droit::contexte::Contexte;
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

fn texte(v: &Value, k: &str) -> String {
    v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Le taux AT/MP de l'annee tel que le bareme le lit (`Contexte::taux_atmp_de`) : (dit par la compagnie pour l'annee,
/// code qui vaut sans qu'elle l'ait dit : pose a la creation, ou repris d'une annee d'avant).
fn deja_pose(ctx: &Contexte, an: i32) -> (bool, Option<String>) {
    match ctx.taux_atmp_de(an) {
        Some(Ok(t)) => {
            let code = t.get("code_risque").filter(|c| vrai(c)).map(valeur_en_texte);
            if t.get("_pose_a_la_creation").is_some_and(vrai) || t.get("_reporte_de").is_some() {
                (false, code)
            } else {
                (true, code)
            }
        }
        _ => (false, None),
    }
}

/// Le code risque et le taux AT/MP des DSN reprises (voir l'en-tete). Rend ce qui s'est pose, en phrases.
fn poser_atmp(ctx: &Contexte, lecture: &LectureDsn, faits: &mut Vec<String>) {
    let courante = ctx.aujourdhui.an;
    for (an, (mois, code, taux)) in &lecture.atmp {
        if *an > courante || deja_pose(ctx, *an).0 {
            continue;
        }
        let valeur = taux.as_deref().and_then(|t| socle::num::decimal_lu(&t.replace(',', ".")));
        let source = format!("Lu dans la DSN de {} reprise par l'import (S21.G00.40.040, S21.G00.40.043).", en_clair(mois));
        if crate::routes::atmp::poser(ctx, i64::from(*an), code, valeur, &source).is_ok() {
            let dit = valeur.map(|v| format!("taux {} %", v.normalize().to_string().replace('.', ","))).unwrap_or_else(|| "sans taux (pas encore de notification)".into());
            faits.push(format!("Accidents du travail {} : code risque {}, {}, lus dans la DSN de {}.", an, code, dit, en_clair(mois)));
        }
    }
    if lecture.atmp.contains_key(&courante) {
        return;
    }
    let Some((_, (mois, code, _))) = lecture.atmp.iter().next_back() else { return };
    let (dit, de_lui_meme) = deja_pose(ctx, courante);
    if dit || de_lui_meme.as_deref() == Some(code.as_str()) {
        return;
    }
    let source = format!("Code lu dans la DSN de {} reprise par l'import (S21.G00.40.040) ; taux de l'arrêté de {}.", en_clair(mois), courante);
    if crate::routes::atmp::poser(ctx, i64::from(courante), code, None, &source).is_ok() {
        faits.push(format!("Code risque AT/MP {} lu dans la DSN de {} ; le taux de {} est celui de l'arrêté.", code, en_clair(mois), courante));
    } else {
        // Le taux de l'annee n'est pas celui de l'arrete pour ce code (ou l'etablissement est en Alsace-Moselle) : le code
        // se garde, sans taux, et « Pour demarrer » demande le taux notifie.
        let mut m = Map::new();
        m.insert("code_risque".into(), json!(code));
        m.insert("_source".into(), json!(format!("Code lu dans la DSN de {} reprise par l'import (S21.G00.40.040).", en_clair(mois))));
        let p = ctx.racine.join("config").join("baremes").join(format!("{}.json", courante));
        let mut data = socle::fichiers::lire_json(&p).unwrap_or_else(|_| json!({"annee": courante, "socle": courante}));
        data["taux_atmp"] = Value::Object(m);
        if socle::ecriture::ecrire_json(&p, &data, 2).is_ok() {
            faits.push(format!("Code risque AT/MP {} lu dans la DSN de {} ; son taux de {} est celui de la notification, à saisir.", code, en_clair(mois), courante));
        }
    }
}

/// Les organismes et les contrats de la DSN (voir l'en-tete), pour les contrats que l'ecran attend encore. Un contrat
/// deja saisi a son libelle et n'attend plus rien.
fn poser_organismes(ctx: &Contexte, lecture: &LectureDsn, faits: &mut Vec<String>) {
    if lecture.adhesions.is_empty() {
        return;
    }
    let Ok(ecran) = crate::compagnie::organismes::lire(ctx, &json!({})) else { return };
    let lignes: Vec<Value> = ecran["contrats"].as_array().cloned().unwrap_or_default();
    let fixes: Vec<String> = lignes.iter().map(|l| texte(l, "libelle").to_uppercase()).filter(|l| !l.is_empty()).collect();
    let a_saisir: Vec<&Value> = lignes.iter().filter(|l| texte(l, "libelle").is_empty()).collect();
    let libres: Vec<&Value> = lecture
        .adhesions
        .iter()
        .filter(|a| !texte(a, "libelle").is_empty() && !fixes.contains(&texte(a, "libelle").to_uppercase()))
        .collect();
    let ([l], [a]) = (a_saisir.as_slice(), libres.as_slice()) else { return };
    let mut saisies = Map::new();
    saisies.insert(texte(l, "cle"), json!({"libelle": texte(a, "libelle"), "organisme": texte(a, "organisme")}));
    if crate::compagnie::organismes::enregistrer(ctx, &json!({"contrats": saisies})).is_ok() {
        faits.push(format!(
            "{} : contrat {} chez {}, lu dans la DSN reprise (S21.G00.15.001, S21.G00.15.002).",
            texte(l, "quoi"),
            texte(a, "libelle"),
            texte(a, "organisme")
        ));
    }
}

/// Ce que les DSN reprises de la compagnie (`historique/bulletins/`) disent de sa configuration, pose ; rend ce qui l'a ete.
pub(crate) fn parametrer(ctx: &Contexte) -> Vec<String> {
    let lecture = crate::import::dsn_aem::lire_dsn(&ctx.racine.join("historique").join("bulletins"));
    if lecture.fichiers == 0 {
        return Vec::new();
    }
    let mut faits = Vec::new();
    poser_atmp(ctx, &lecture, &mut faits);
    poser_organismes(ctx, &lecture, &mut faits);
    poser_taux_pas(ctx, &mut faits);
    faits
}

/// LE TAUX DE PRELEVEMENT A LA SOURCE que la DGFiP a transmis a la compagnie et que ses DSN reprises appliquent
/// (S21.G00.50.006 a .008) : au magasin du PAS, comme un taux recu, date du mois de la DSN
/// (`commandes::pas::derniers_taux_des_dsn`). La validite se lit ensuite comme pour tout taux recu (`droit::pas`).
fn poser_taux_pas(ctx: &Contexte, faits: &mut Vec<String>) {
    let dossier = ctx.racine.join("historique").join("bulletins");
    let releves = crate::commandes::pas::derniers_taux_des_dsn(ctx, &dossier);
    let Ok(changes) = crate::commandes::pas::enregistrer(ctx, &releves, "DSN reprises par l'import") else { return };
    for r in &releves {
        let cle = texte(r, "salarie");
        if !changes.iter().any(|c| c.starts_with(&format!("{} :", cle))) {
            continue;
        }
        let qui = ctx.salaries.get(&cle).map(|s| {
            let prenom = if vrai(s.get("prenom_affichage").unwrap_or(&Value::Null)) { texte(s, "prenom_affichage") } else { texte(s, "prenom") };
            format!("{} {}", prenom, texte(s, "nom")).trim().to_string()
        });
        faits.push(format!(
            "Prélèvement à la source de {} : taux {} % transmis par la DGFiP, appliqué dans la DSN de {} (S21.G00.50.006 à .008).",
            qui.filter(|q| !q.is_empty()).unwrap_or(cle),
            texte(r, "taux").replace('.', ","),
            en_clair(&texte(r, "periode"))
        ));
    }
}

/// Les contrats des permanents que les DSN reprises portent, poses quand la compagnie se met a employer un permanent
/// (la nature d'un contrat repris dite, `import::nature::dire`) : l'ecran ne les attendait pas encore a l'import.
pub(crate) fn poser_organismes_repris(ctx: &Contexte) -> Vec<String> {
    let lecture = crate::import::dsn_aem::lire_dsn(&ctx.racine.join("historique").join("bulletins"));
    let mut faits = Vec::new();
    if lecture.fichiers > 0 {
        poser_organismes(ctx, &lecture, &mut faits);
    }
    faits
}

/// « décembre 2025 » pour `2025-12`.
fn en_clair(mois: &str) -> String {
    crate::regularisation::mois_en_clair(mois)
}
