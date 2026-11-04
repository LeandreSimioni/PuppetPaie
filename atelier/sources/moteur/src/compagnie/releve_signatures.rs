//! La releve des reponses « Je signe ». La demande de signature (`crate::compagnie::signature::demander_signature`) a garde,
//! au journal, un CODE (`SIG-XXXXXX`) par courriel parti. Cette releve cherche chaque code attendu dans la boite
//! (`envois::releve_signatures::chercher_reponse`, le tri strict) et fait tout ce qui suit : la preuve archivee
//! (`Signature_<salarie>_<code>.eml`), le contrat contresigne (`depart::contresigner` : le PDF parti, plus la mention de l'artiste dans sa case), le journal (sur la ligne de
//! l'envoi, par contrat, date de la SIGNATURE, `crate::routes::contrats::contrat_recu_avec`), et le renvoi du
//! contresigne au salarie (`envois::releve_signatures::renvoyer_contresigne`).
//!
//! NE LEVE PAS pour une boite absente : c'est un travail de fond, il dit ce qui l'empeche dans `incidents` et rend la
//! main.
//!
//! Dans une compagnie fictive, les salaries « signent » (`boite_des_signatures`) : chaque demande partie
//! recoit sa reponse aussitot -- celle que le bouton du courriel prepare --, et c'est la VRAIE releve qui la lit.
//! `relever_signatures_fictive` ne joue rien dans une compagnie qui n'est pas une compagnie fictive (la garde de
//! l'horloge).

use std::path::PathBuf;

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::envoi;
use envois::etats;
use envois::releve_signatures::{chercher_reponse, identifiants_de, renvoyer_contresigne, Contresigne, Reponse};
pub use envois::releve_signatures::{ReleveSignatures, SignatureRecue};
use socle::echec::R;
use socle::echec::erreur;
use socle::echec::Echec;
use crate::horloge::{HorlogeFictive, QUI_HORLOGE};
use envois::simulation_courriels::BoiteQuiRepond;

// ------------------------------------------------------------------ la compagnie fictive

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de signature partie
/// y trouve aussitot la reponse « Je signe » que le bouton de son courriel prepare (a defaut, une reponse
/// au code dans les termes de `CORPS_REPONSE`) ; la vraie releve la lit, note la signature, contresigne le contrat et le
/// renvoie au salarie.
pub fn boite_des_signatures(ctx: &Contexte) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx);
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let attendues = etats::signatures_attendues(|c| ctx.salaries.get(c).map(|f| &f.champs), &lignes, None);
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine);
    for a in &attendues {
        let code = a.code.clone();
        let nom = a.nom.clone();
        let (sujet, corps) = match envois::simulation_courriels::bouton_de_la_demande(&envoyes, &identifiants_de(a)) {
            Some(b) => b,
            None => (
                "Re: contrat".to_string(),
                envoi::formater(
                    crate::compagnie::signature::CORPS_REPONSE,
                    &[("nom", nom), ("quoi", "le contrat d'engagement".to_string()), ("code", code.clone())],
                ),
            ),
        };
        let brut = envois::simulation_courriels::reponse_jouee(&a.adresse, Courriel::compte(&boite), &sujet, &corps, &format!("signature-{}", code))
            .map_err(|m| erreur("ValueError", m))?;
        boite.repondre(&code, brut);
    }
    Ok(boite)
}

/// Les salaries de la compagnie fictive signent, et la VRAIE releve lit leur reponse. Rien
/// ne se joue dans une compagnie qui n'est pas une compagnie fictive (la garde de l'horloge) : la releve rendue est alors
/// vide.
pub fn relever_signatures_fictive(ctx: &Contexte) -> R<ReleveSignatures> {
    if HorlogeFictive::verifier(&ctx.racine).is_none() {
        return Ok(ReleveSignatures::nouvelle());
    }
    let mut boite = boite_des_signatures(ctx)?;
    relever_signatures(ctx, Some(&mut boite), None, QUI_HORLOGE)
}


#[track_caller]
fn os(m: String) -> Echec {
    erreur("OSError", m)
}

/// La boite ou chercher les reponses, quand l'appelant n'en donne pas, selon le branchement de la compagnie
/// (`bridage::branchement`) : le simulateur (les salaries fictifs repondent) ; le message, rien ne se releve (la releve
/// renverrait le contrat contresigne au salarie : un envoi) ; le monde, la vraie boite. `Err` : l'incident qui dit
/// pourquoi rien ne se releve.
fn boite_de_la_releve(ctx: &Contexte) -> R<Result<Box<dyn Courriel>, String>> {
    match crate::compagnie::bridage::branchement(&ctx.racine) {
        envois::branchement::Branchement::Simulateur => return Ok(Ok(Box::new(boite_des_signatures(ctx)?))),
        envois::branchement::Branchement::Message(r) => return Ok(Err(r.phrase.to_string())),
        envois::branchement::Branchement::Monde => {}
    }
    if !crate::compagnie::boite::disponible(&ctx.racine) {
        return Ok(Err(
            "Aucune boite courriel : les signatures ne peuvent pas etre relevees. Poser le mot de passe d'application de la boite (Configuration > Mots de passe)."
                .to_string(),
        ));
    }
    let vrai_client = crate::compagnie::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
    Ok(Ok(Box::new(vrai_client)))
}

/// Cherche dans la boite les reponses « Je signe » encore attendues, et fait tout ce qui suit.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas (`boite_de_la_releve`) ; sans boite, un incident et
/// rien d'autre. `mois` : seulement les demandes de ce mois. `qui` : `veille`, `interface`, `ligne de commande`,
/// `horloge`. Une erreur de transport ou de lecture se dit dans `incidents` ; seul un fichier qui ne s'ecrit pas remonte
/// en `Err`.
pub fn relever_signatures(ctx: &Contexte, client: Option<&mut dyn Courriel>, mois: Option<&str>, qui: &str) -> R<ReleveSignatures> {
    let mut r = ReleveSignatures::nouvelle();
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let mois = mois.filter(|m| !m.is_empty()).map(socle::mois::Mois::lire).transpose()?;
    let attendues = etats::signatures_attendues(|c| ctx.salaries.get(c).map(|f| &f.champs), &lignes, mois);
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => match boite_de_la_releve(ctx)? {
            Ok(b) => ouverte.insert(b).as_mut(),
            Err(incident) => {
                r.incidents.push(incident);
                return Ok(r);
            }
        },
    };

    let (signataire, raison) = crate::compagnie::signature::signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = a.code.clone();
        let a_mois = a.mois.map(|m| m.to_string()).unwrap_or_default();
        let a_nom = a.nom.clone();
        let a_salarie = a.salarie.to_string();
        let contrats = a.contrats.clone();
        let retenue = match chercher_reponse(client, a, &expediteur, &mut r.incidents) {
            Ok(x) => x,
            // Une erreur de transport ne se corrige pas en essayant le code suivant.
            Err(()) => break,
        };
        let Some(Reponse { identifiant: ident, brut, de, quand }) = retenue else { continue };

        // La preuve archivee : le message tel qu'il est arrive.
        let dossier = ctx.racine.join("sorties").join(&a_mois).join("envoi");
        std::fs::create_dir_all(&dossier).map_err(|e| os(format!("{} : {}", dossier.display(), e)))?;
        let preuve = dossier.join(format!("Signature_{}_{}.eml", a_salarie, code));
        std::fs::write(&preuve, &brut).map_err(|e| os(format!("{} : {}", preuve.display(), e)))?;
        let nom_preuve = socle::fichiers::nom_du(&preuve);

        // UN PDF PAR FICHIER DISTINCT : des contrats groupes n'en ont qu'un (un par page) ; des avenants groupes en ont un
        // chacun.
        let a_fichier = a.fichier.clone();
        let mut fichiers: Vec<(String, String)> = Vec::new();
        for c in &contrats {
            let paire = (c.mois.to_string(), if c.fichier.is_empty() { a_fichier.clone() } else { c.fichier.clone() });
            if !fichiers.contains(&paire) {
                fichiers.push(paire);
            }
        }
        let quand_lisible = etats::quand_lisible(&quand);
        let mention = documents::contrat::mention_signature(&a_nom, &quand_lisible, &de, &code);
        let mut signes: Vec<(String, PathBuf)> = Vec::new();
        for (mois_f, nom_f) in &fichiers {
            let envoye = ctx.racine.join("sorties").join(mois_f).join("envoi").join(nom_f);
            let souche = envoye.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let signe = envoye.with_file_name(format!("{}{}", souche, envois::releve_signatures::SUFFIXE_CONTRESIGNE));
            // L'EXEMPLAIRE CONTRESIGNE : le PDF parti avec ce code, octet pour octet, plus la mention de l'artiste dans sa case.
            let fait = crate::compagnie::depart::contresigner(ctx, &lignes, &code, &envoye, &signe, &mention).map_err(|e| e.message);
            match fait {
                Ok(()) => match signes.iter_mut().find(|(n, _)| n == nom_f) {
                    Some(deja) => deja.1 = signe,
                    None => signes.push((nom_f.clone(), signe)),
                },
                Err(e) => r.incidents.push(format!("{} : contrat contresigne impossible ({}).", a_nom, e)),
            }
        }
        let noms_signes: Vec<String> = signes.iter().map(|(_, p)| socle::fichiers::nom_du(p)).collect();
        let fichier_signe = noms_signes.join(", ");

        // LE JOURNAL D'ABORD : une fois la ligne ecrite, la releve suivante n'attend plus ce code, et le renvoi ne se fera
        // pas deux fois. Un contrat que `contrat_recu` refuse (la paie n'est plus dans la saisie du mois...) devient un
        // incident NOMME (le salarie, le contrat, la raison) et la releve continue.
        let acte = a.acte.clone();
        let est_avenant = acte == "avenant";
        let mut notes = 0usize;
        for c in &contrats {
            let son_fichier = if c.fichier.is_empty() { a_fichier.clone() } else { c.fichier.clone() };
            let sien = signes.iter().find(|(n, _)| *n == son_fichier).map(|(_, p)| socle::fichiers::nom_du(p));
            let courriel = crate::routes::contrats::ParCourriel {
                code: code.clone(),
                adresse: de.clone(),
                quand: quand.clone(),
                message: ident.clone(),
                preuve: nom_preuve.clone(),
                fichier_signe: sien.unwrap_or_default(),
                // La mention posee : l'exemplaire contresigne se refait du contrat parti et d'elle (`depart::piece_partie`).
                mention: mention.clone(),
                acte: acte.clone(),
                avenant: if est_avenant { c.avenant.filter(|n| *n != 0).or(a.avenant) } else { None },
            };
            let debut = c.debut.clone();
            // Le contrat que cette reponse signe, par son numero : le jumeau du meme jour garde sa propre attente.
            match crate::routes::contrats::contrat_recu_avec(ctx, &a_mois, &c.salarie, &debut, Some(c.numero_dit()), true, None, qui, Some(&courriel)) {
                Ok(_) => notes += 1,
                Err(e) => r.incidents.push(format!(
                    "{} : la signature de {} est arrivee mais le contrat du {} n'a pas pu etre enregistre ({}) ; il n'est pas note signe.",
                    a_nom, code, debut, e.message
                )),
            }
        }
        if notes == 0 {
            // Rien n'est note : la signature reste attendue (elle sera relevee de nouveau, et l'incident redit), rien n'est renvoye.
            continue;
        }
        r.recues.push(SignatureRecue {
            mois: a_mois.clone(),
            salarie: a_salarie.clone(),
            nom: a_nom.clone(),
            code: code.clone(),
            adresse: de.clone(),
            quand: quand.clone(),
            contrats: contrats.clone(),
            fichier_signe: fichier_signe.clone(),
            preuve: nom_preuve.clone(),
            renvoye: false,
        });
        if signes.is_empty() {
            continue;
        }

        // Le contrat contresigne, renvoye au salarie.
        let (an, nom_mois) = socle::mois::Mois::lire(&a_mois).map(|m| (m.annee(), m.nom(socle::mois::Graphie::Accentuee)))?;
        let pdfs: Vec<PathBuf> = signes.iter().map(|(_, p)| p.clone()).collect();
        let contresigne = Contresigne {
            a,
            contrats: &contrats,
            est_avenant,
            de: &de,
            quand_lisible: &quand_lisible,
            signes: &pdfs,
            signataire: &signataire,
            raison: &raison,
            expediteur: &expediteur,
            an,
            nom_mois,
        };
        match renvoyer_contresigne(&ctx.racine, client, &contresigne, qui) {
            Ok(()) => {
                let dernier = r.recues.len() - 1;
                r.recues[dernier].renvoye = true;
            }
            Err(e) => r.incidents.push(format!("{} : le contrat contresigne n'a pas pu etre renvoye ({}). Il est dans {}.", a_nom, e.message, fichier_signe)),
        }
    }
    client.fermer();
    Ok(r)
}
