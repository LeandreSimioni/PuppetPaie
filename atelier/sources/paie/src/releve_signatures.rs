//! La releve des reponses « Je signe ». La demande de signature (`envois::envois_signature::demander_signature`) a garde,
//! au journal, un CODE (`SIG-XXXXXX`) par courriel parti. Cette releve cherche chaque code attendu dans la boite
//! (`envois::releve_signatures::chercher_reponse`, le tri strict) et fait tout ce qui suit : la preuve archivee
//! (`Signature_<salarie>_<code>.eml`), le contrat contresigne (`documents::contrat::contresigner`), le journal (une ligne
//! `contrat-recu` par contrat, datee de la SIGNATURE, `routes_ecritures_g4::contrat_recu_avec`), et le renvoi du
//! contresigne au salarie (`envois::releve_signatures::renvoyer_contresigne`).
//!
//! NE LEVE PAS pour une boite absente : c'est un travail de fond, il dit ce qui l'empeche dans `incidents` et rend la
//! main.
//!
//! Dans une compagnie fictive, les salaries « signent » (`boite_des_signatures`) : chaque demande partie depuis `delai`
//! secondes recoit sa reponse -- celle que le bouton du courriel prepare --, et c'est la VRAIE releve qui la lit.
//! `relever_signatures_fictive` ne joue rien dans une compagnie qui n'est pas une compagnie fictive (la garde de
//! l'horloge).

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::envoi;
use envois::etats;
use envois::releve_signatures::{chercher_reponse, identifiants_de, renvoyer_contresigne, Contresigne, Reponse};
pub use envois::releve_signatures::{ReleveSignatures, SignatureRecue};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::horloge::{HorlogeFictive, QUI_HORLOGE};
use socle::echec::Echec;
use envois::simulation_courriels::BoiteQuiRepond;

/// Les routes de ce module.
pub const ROUTES: [&str; 1] = ["signatures-relever"];

/// Secondes entre une demande de signature et le « Je signe » du salarie fictif de la compagnie fictive.
pub const DELAI_FICTIVE: i64 = crate::routes_fiches_courriel::DELAI_FICTIVE;

// ------------------------------------------------------------------ la route

/// `signatures-relever` : `{"mois"?, "qui"?}` -> `ReleveSignatures.en_json`. La vraie boite (ou celle d'une
/// compagnie fictive) : a n'appeler, hors d'une compagnie fictive, que pour relever pour de bon.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    route_avec(ctx, nom, charge, None)
}

/// La route, la boite venant de `client` quand il est donne (un faux client dans les tests).
pub fn route_avec(ctx: &Contexte, nom: &str, charge: &Value, client: Option<&mut dyn Courriel>) -> R<Value> {
    match nom {
        "signatures-relever" => {
            let texte = |cle: &str| charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte);
            let mois = texte("mois");
            let qui = texte("qui").unwrap_or_else(|| "interface".to_string());
            Ok(relever_signatures(ctx, client, mois.as_deref(), &qui)?.en_json())
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ la compagnie fictive

/// `simulation.BoiteDesSignatures` : la boite de la compagnie fictive, vue par la releve. Chaque demande de signature partie
/// depuis `delai` secondes y trouve la reponse « Je signe » que le bouton de son courriel prepare (a defaut, une reponse
/// au code dans les termes de `CORPS_REPONSE`) ; la vraie releve la lit, note la signature, contresigne le contrat et le
/// renvoie au salarie.
pub fn boite_des_signatures(ctx: &Contexte, delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx);
    let lignes = etats::charger(&ctx.racine)?;
    let attendues = etats::signatures_attendues(&lignes, None, &crate::routes_documents::salaries_d_identite(ctx))?;
    let maintenant_naif = socle::paris::maintenant_naif();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine);
    for a in attendues.as_array().into_iter().flatten() {
        let quand = a.get("quand").map(valeur_en_texte).unwrap_or_default();
        let age = socle::paris::naif_de_iso(&quand).map_or(0, |t| maintenant_naif - t);
        if age < delai {
            continue;
        }
        let code = texte_de(a, "code");
        let nom = texte_de(a, "nom");
        let (sujet, corps) = match envois::simulation_courriels::bouton_de_la_demande(&envoyes, &identifiants_de(a)) {
            Some(b) => b,
            None => (
                "Re: contrat".to_string(),
                envoi::formater(
                    envois::envois_signature::CORPS_REPONSE,
                    &[("nom", nom), ("quoi", "le contrat d'engagement".to_string()), ("code", code.clone())],
                ),
            ),
        };
        let brut = envois::simulation_courriels::reponse_jouee(&texte_de(a, "adresse"), Courriel::compte(&boite), &sujet, &corps, &format!("signature-{}", code))
            .map_err(|m| erreur("ValueError", m))?;
        boite.repondre(&code, brut);
    }
    Ok(boite)
}

/// `simulation.relever_signatures` : les salaries de la compagnie fictive signent, et la VRAIE releve lit leur reponse. Rien
/// ne se joue dans une compagnie qui n'est pas une compagnie fictive (la garde de l'horloge) : la releve rendue est alors
/// vide.
pub fn relever_signatures_fictive(ctx: &Contexte, delai: i64) -> R<ReleveSignatures> {
    if HorlogeFictive::verifier(&ctx.racine).is_none() {
        return Ok(ReleveSignatures::nouvelle());
    }
    let mut boite = boite_des_signatures(ctx, delai)?;
    relever_signatures(ctx, Some(&mut boite), None, QUI_HORLOGE)
}


fn texte_de(d: &Value, cle: &str) -> String {
    d.get(cle).map(valeur_en_texte).unwrap_or_default()
}

/// `x.get(cle) or y` : la valeur de `x` quand elle est vraie, sinon celle de `y` (ou `null`).
fn ou_valeur(x: &Value, y: &Value, cle: &str) -> Value {
    match x.get(cle).filter(|v| vrai(v)) {
        Some(v) => v.clone(),
        None => y.get(cle).cloned().unwrap_or(Value::Null),
    }
}

/// `x.get(cle) or defaut`, en texte.
fn texte_ou(x: &Value, cle: &str, defaut: &str) -> String {
    x.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| defaut.to_string())
}

#[track_caller]
fn os(m: String) -> Echec {
    erreur("OSError", m)
}

/// La boite ou chercher les reponses, quand l'appelant n'en donne pas : celle de la compagnie fictive (les salaries
/// fictifs repondent), sinon la vraie. `Err` : l'incident qui dit pourquoi rien ne se releve.
fn boite_de_la_releve(ctx: &Contexte) -> R<Result<Box<dyn Courriel>, String>> {
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        return Ok(Ok(Box::new(boite_des_signatures(ctx, DELAI_FICTIVE)?)));
    }
    // UNE FICTIVE AJOUTEE : la releve renverrait le contrat contresigne au salarie -- un envoi. Rien ne se releve.
    if let Some(m) = crate::bridage::refus_d_envoi(&ctx.racine) {
        return Ok(Err(m.to_string()));
    }
    if !crate::boite::disponible(&ctx.racine) {
        return Ok(Err(
            "Aucune boite courriel : les signatures ne peuvent pas etre relevees. Poser le mot de passe d'application de la boite (Configuration > Mots de passe)."
                .to_string(),
        ));
    }
    let vrai_client = crate::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
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
    let lignes = etats::charger(&ctx.racine)?;
    let attendues = etats::signatures_attendues(&lignes, mois, &crate::routes_documents::salaries_d_identite(ctx))?;
    let attendues: Vec<Value> = attendues.as_array().cloned().unwrap_or_default();
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

    let (signataire, raison) = envois::envois_signature::signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = texte_de(a, "code");
        let a_mois = texte_de(a, "mois");
        let a_nom = texte_de(a, "nom");
        let a_salarie = texte_de(a, "salarie");
        let contrats: Vec<Value> = a.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default();
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
        let a_fichier = texte_de(a, "fichier");
        let mut fichiers: Vec<(String, String)> = Vec::new();
        for c in &contrats {
            let paire = (texte_ou(c, "mois", &a_mois), texte_ou(c, "fichier", &a_fichier));
            if !fichiers.contains(&paire) {
                fichiers.push(paire);
            }
        }
        let quand_lisible = etats::quand_lisible(&json!(quand));
        let mut signes: Vec<(String, PathBuf)> = Vec::new();
        for (mois_f, nom_f) in &fichiers {
            let envoye = ctx.racine.join("sorties").join(mois_f).join("envoi").join(nom_f);
            let souche = envoye.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let signe = envoye.with_file_name(format!("{}_signe.pdf", souche));
            if envoye.exists() {
                match documents::contrat::contresigner(&envoye, &signe, &a_nom, &quand_lisible, &de, &code) {
                    Ok(()) => match signes.iter_mut().find(|(n, _)| n == nom_f) {
                        Some(deja) => deja.1 = signe,
                        None => signes.push((nom_f.clone(), signe)),
                    },
                    Err(e) => r.incidents.push(format!("{} : contrat contresigne impossible ({}).", a_nom, e)),
                }
            } else {
                r.incidents.push(format!(
                    "{} : le contrat envoye ({}) n'est plus dans {} ; la signature est notee, sans exemplaire contresigne.",
                    a_nom,
                    nom_f,
                    envoye.parent().map(|p| p.display().to_string()).unwrap_or_default()
                ));
            }
        }
        let noms_signes: Vec<String> = signes.iter().map(|(_, p)| socle::fichiers::nom_du(p)).collect();
        let fichier_signe = noms_signes.join(", ");

        // LE JOURNAL D'ABORD : une fois la ligne ecrite, la releve suivante n'attend plus ce code, et le renvoi ne se fera
        // pas deux fois. Un contrat que `contrat_recu` refuse (la paie n'est plus dans la saisie du mois...) devient un
        // incident NOMME (le salarie, le contrat, la raison) et la releve continue.
        let acte = a.get("acte").cloned().unwrap_or_else(|| json!("contrat"));
        let est_avenant = acte == json!("avenant");
        let mut notes = 0usize;
        for c in &contrats {
            let sien = signes.iter().find(|(n, _)| *n == texte_ou(c, "fichier", &a_fichier)).map(|(_, p)| socle::fichiers::nom_du(p));
            let mut courriel = Map::new();
            courriel.insert("code".into(), json!(code));
            courriel.insert("adresse".into(), json!(de));
            courriel.insert("quand".into(), json!(quand));
            courriel.insert("message".into(), json!(ident));
            courriel.insert("preuve".into(), json!(nom_preuve));
            courriel.insert("fichier_signe".into(), json!(sien.unwrap_or_default()));
            courriel.insert("acte".into(), acte.clone());
            if est_avenant {
                courriel.insert("avenant".into(), ou_valeur(c, a, "avenant"));
            }
            let debut = texte_de(c, "debut");
            // Le contrat que cette reponse signe, par son numero : le jumeau du meme jour garde sa propre attente.
            let numero = etats::numero_de_cle(c.get("numero").unwrap_or(&Value::Null));
            match crate::routes_ecritures_g4::contrat_recu_avec(ctx, &a_mois, &texte_de(c, "salarie"), &debut, numero.as_str(), true, qui, Some(&courriel)) {
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
        let (an, nom_mois) = envois::envois_signature::an_et_nom_du_mois(&a_mois)?;
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
