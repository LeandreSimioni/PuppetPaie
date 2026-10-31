//! LES SUITES D'UN ENVOI, du cote de la compagnie (`envois::suites`) : ce qu'un envoi a releve en chemin et qui touche a
//! la compagnie -- noter les incidents, ranger les attestations France Travail, marquer les arrets qu'une DSN a portes,
//! faire taire le reveil du filet. Elles se font toutes, que l'envoi ait abouti ou non.

use serde_json::json;

use droit::contexte::Contexte;
use envois::suites::Suites;
use socle::echec::R;
use socle::fichiers::vrai;

/// Ce que les suites laissent a dire : ce qu'il reste a faire, et ce qui n'a pas pu se faire.
#[derive(Default)]
pub struct Faites {
    pub a_faire: Vec<String>,
    pub incidents: Vec<String>,
}

/// Fait les suites d'un envoi. Un rangement d'AER qui echoue se dit ; une marque qui ne s'ecrit pas est un refus.
pub fn faire(ctx: &Contexte, s: Suites) -> R<Faites> {
    let mut faites = Faites::default();
    if !s.incidents_des_retours.is_empty() {
        crate::compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::etats::AvecIncidents { resultat: Ok(()), incidents: s.incidents_des_retours })?;
    }
    if let Some(d) = &s.depot_refuse {
        noter_incident_depot(ctx, d);
    }
    if let Some(d) = &s.depot_reel {
        marquer_depot_reel(ctx, d)?;
    }
    if !s.taux_pas.is_empty() {
        if let Err(e) = appliquer_taux_pas(ctx, &s.taux_pas) {
            faites.incidents.push(format!("Taux de PAS recus mais non appliques : {}", e.message));
        }
    }
    if !s.taux_atmp.is_empty() {
        match appliquer_taux_atmp(ctx, &s.taux_atmp) {
            Ok(a_faire) => faites.a_faire.extend(a_faire),
            Err(e) => faites.incidents.push(format!("Notification du taux AT/MP reçue mais non posée : {}", e.message)),
        }
    }
    if !s.aer.is_empty() {
        match crate::routes::documents::ranger_aer(ctx, None, Some(&s.aer)) {
            Ok(a_faire) => faites.a_faire.extend(a_faire),
            Err(e) => faites.incidents.push(format!("Attestations France Travail recues mais non rangees : {}", e.message)),
        }
    }
    Ok(faites)
}

/// Les taux qu'un compte rendu DGFiP de nature 94 communique vont au magasin du PAS (`config/pas.json`) : un taux se lit
/// du compte rendu, il ne se devine pas (regle 1). Chaque releve se rattache a une fiche par le NIR ; un NIR qu'aucune
/// fiche ne porte ne se rattache a personne. La validite court de la mise a disposition (`droit::pas`).
fn appliquer_taux_pas(ctx: &Contexte, crm: &[(serde_json::Value, std::path::PathBuf)]) -> R<()> {
    use serde_json::Value;
    let nirs: Vec<Value> = ctx.salaries.iter().filter(|(_, s)| s.get("nir").is_some_and(vrai)).map(|(cle, s)| json!([cle, s["nir"]])).collect();
    let mut tous = Vec::new();
    for (retour, chemin) in crm {
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
        let lus = envois::retours::taux_pas_d_octets(&octets)?;
        let releves: Vec<Value> = lus.get("releves").and_then(Value::as_array).cloned().unwrap_or_default();
        let r = envois::retours::releves_vers_salaries(&releves, &nirs);
        let production: String = retour.get("production").filter(|p| vrai(p)).map(socle::valeur::valeur_en_texte).unwrap_or_default().chars().take(6).collect();
        for x in r.get("rattaches").and_then(Value::as_array).cloned().unwrap_or_default() {
            let mut m = x.as_object().cloned().unwrap_or_default();
            m.insert("periode".into(), json!(production));
            m.insert("mis_a_disposition".into(), retour.get("publication").cloned().unwrap_or(Value::Null));
            tous.push(Value::Object(m));
        }
    }
    crate::compagnie::taux_pas::enregistrer(ctx, &tous, "CRM DGFiP nature 94")?;
    Ok(())
}

/// LES NOTIFICATIONS DU TAUX AT/MP (nature 34) : le code risque et le taux se lisent de la notification (regle 1 ;
/// `envois::retours::taux_atmp_d_octets`, `atelier/reference/sources/gip-mds-api-dsn/`) et se posent dans la surcouche de
/// l'annee de leur date d'effet (`routes::atmp::poser_notifie`). Seul l'etablissement de la fiche entreprise (son SIREN et
/// son NIC) a un taux dans la configuration : une notification pour un autre, illisible, ou qui donne plusieurs codes
/// risques pour la meme annee ne pose rien et se dit, dans ce qu'il reste a faire.
fn appliquer_taux_atmp(ctx: &Contexte, notifications: &[(serde_json::Value, std::path::PathBuf)]) -> R<Vec<String>> {
    use std::collections::BTreeMap;
    let mut a_faire = Vec::new();
    let texte = |k: &str| ctx.entreprise.get(k).map(socle::valeur::valeur_en_texte).unwrap_or_default();
    let (siren, nic) = (texte("siren"), texte("nic"));
    for (retour, chemin) in notifications {
        let le = envois::releve_retours::date_publication(retour);
        let le_iso = socle::date::Jour::lire(&le).map(|j| j.iso()).unwrap_or_default();
        let archive = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let source = format!("Notification AT/MP de la CARSAT reçue le {} (retour DSN de nature 34, archive retours/{})", le, archive);
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
        let lus = match envois::retours::taux_atmp_d_octets(&octets) {
            Ok(l) => l,
            Err(e) => {
                a_faire.push(format!("{} : {} Rien n'est posé ; le code et le taux de la notification (compte AT/MP sur net-entreprises) se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, e.message));
                continue;
            }
        };
        let mut par_annee: BTreeMap<i32, Vec<envois::retours::TauxAtmpNotifie>> = BTreeMap::new();
        for t in lus {
            if t.siren != siren || t.nic != nic {
                a_faire.push(format!(
                    "{} : le taux {} (code risque {}) est celui de l'établissement {}{}, qui n'est pas celui de la fiche Entreprise ({}{}). Rien n'est posé.",
                    source, t.taux, t.code_risque, t.siren, t.nic, siren, nic
                ));
                continue;
            }
            par_annee.entry(t.date_effet.an).or_default().push(t);
        }
        for (an, mut taux) in par_annee {
            taux.sort_by_key(|t| t.date_effet);
            let codes: std::collections::BTreeSet<&str> = taux.iter().map(|t| t.code_risque.as_str()).collect();
            if codes.len() > 1 {
                a_faire.push(format!(
                    "{} : plusieurs codes risques pour {} ({}). La configuration n'en porte qu'un par année : rien n'est posé ; saisissez celui de l'activité principale dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                    source, an, codes.into_iter().collect::<Vec<_>>().join(", ")
                ));
                continue;
            }
            for t in &taux {
                let source = format!("{}, effet au {}.", source, t.date_effet.jj_mm_aaaa());
                if let Some(pourquoi) = crate::routes::atmp::poser_notifie(ctx, t, &le_iso, &source)? {
                    a_faire.push(pourquoi);
                }
            }
        }
    }
    Ok(a_faire)
}

/// Un depot refuse s'ecrit au journal des incidents, test comme reel. LE CAS URGENT SE DISTINGUE ICI : un refus en REEL
/// alors que la repetition etait conforme.
fn noter_incident_depot(ctx: &Contexte, d: &envois::suites::DepotRefuse) {
    let apres_test = d.reel && d.repetition.get("test").is_some_and(vrai);
    let _ = crate::compagnie::incidents::noter(
        &ctx.racine,
        &crate::compagnie::incidents::Note {
            nature: if d.quoi == "dsn" { crate::compagnie::incidents::DSN } else { crate::compagnie::incidents::DPAE },
            ou: &format!("depot-{}", d.quoi),
            quoi: &d.classe,
            resume: &d.message,
            detail: json!({"reel": d.reel, "apres_test_conforme": apres_test, "repetition": if vrai(&d.repetition) { d.repetition.clone() } else { json!({}) }}),
            ..Default::default()
        },
    );
}

/// Ce qu'un depot reel accepte laisse derriere lui : la demande de taux d'un signalement d'amorcage, et le reveil du
/// filet qui se tait. Ce qu'un signalement ou une mensuelle declare d'un arret se lit au journal (`obligations::avec_ce_qui_est_parti`).
fn marquer_depot_reel(ctx: &Contexte, d: &envois::suites::DepotReel) -> R<()> {
    if d.quoi == "dsn" {
        match &d.signalement {
            Some(s) => {
                // Sans la trace d'une demande de taux, le moteur redirait « aucun taux connu, il faut demander ».
                if s.nature == declarations::amorcage::NATURE {
                    crate::compagnie::marques_depot::marquer_demande_depuis(ctx, &d.chemin, &d.quand)?;
                }
            }
            None => {
                if !d.joue {
                    // Ne leve jamais.
                    let _ = crate::compagnie::filet::desarmer(&ctx.racine, &format!("dsn|{}", d.mois));
                }
            }
        }
    } else if !d.joue {
        for e in &d.embauches {
            let _ = crate::compagnie::filet::desarmer(&ctx.racine, &crate::compagnie::filet::cle_du_reveil(&json!({"salarie": e["salarie"], "debut": e["debut"]})));
        }
    }
    Ok(())
}
