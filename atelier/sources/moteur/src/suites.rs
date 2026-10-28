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
        crate::incidents::noter_ceux_des_retours(&ctx.racine, envois::etats::AvecIncidents { resultat: Ok(()), incidents: s.incidents_des_retours })?;
    }
    if let Some(d) = &s.depot_refuse {
        noter_incident_depot(ctx, d);
    }
    if let Some(d) = &s.depot_reel {
        marquer_depot_reel(ctx, d)?;
    }
    if !s.aer.is_empty() {
        match crate::routes_documents::ranger_aer(ctx, None, Some(&s.aer)) {
            Ok(a_faire) => faites.a_faire.extend(a_faire),
            Err(e) => faites.incidents.push(format!("Attestations France Travail recues mais non rangees : {}", e.message)),
        }
    }
    Ok(faites)
}

/// Un depot refuse s'ecrit au journal des incidents, test comme reel. LE CAS URGENT SE DISTINGUE ICI : un refus en REEL
/// alors que la repetition etait conforme.
fn noter_incident_depot(ctx: &Contexte, d: &envois::suites::DepotRefuse) {
    let apres_test = d.reel && d.repetition.get("test").is_some_and(vrai);
    let _ = crate::incidents::noter(
        &ctx.racine,
        &crate::incidents::Note {
            nature: if d.quoi == "dsn" { crate::incidents::DSN } else { crate::incidents::DPAE },
            ou: &format!("depot-{}", d.quoi),
            quoi: &d.classe,
            resume: &d.message,
            detail: json!({"reel": d.reel, "apres_test_conforme": apres_test, "repetition": if vrai(&d.repetition) { d.repetition.clone() } else { json!({}) }}),
            ..Default::default()
        },
    );
}

/// Ce qu'un depot reel accepte laisse derriere lui : la trace sur l'arret qu'un signalement a declare (et sur la demande
/// de taux d'un signalement d'amorcage), les arrets qu'une mensuelle a reportes, et le reveil du filet qui se tait.
fn marquer_depot_reel(ctx: &Contexte, d: &envois::suites::DepotReel) -> R<()> {
    if d.quoi == "dsn" {
        match &d.signalement {
            Some(s) => {
                crate::marques_depot::marquer_envoye_depuis(ctx, &d.chemin, &s.nature, &s.ordre, &s.type_declaration, &s.evenement, &d.quand)?;
                // Sans la trace d'une demande de taux, le moteur redirait « aucun taux connu, il faut demander ».
                if s.nature == declarations::amorcage::NATURE {
                    crate::marques_depot::marquer_demande_depuis(ctx, &d.chemin, &d.quand)?;
                }
            }
            None => {
                if !d.joue {
                    // Ne leve jamais.
                    let _ = crate::filet::desarmer(&ctx.racine, &format!("dsn|{}", d.mois));
                }
                crate::marques_depot::marquer_reportes(ctx, &d.mois, &d.chemin, &d.quand)?;
            }
        }
    } else if !d.joue {
        for e in &d.embauches {
            let _ = crate::filet::desarmer(&ctx.racine, &crate::filet::cle_du_reveil(&json!({"salarie": e["salarie"], "debut": e["debut"]})));
        }
    }
    Ok(())
}
