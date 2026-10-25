//! La route `ecrire-documents` (`moteur/web.py`, `operations_documents.reecrire_pieces`) : les pieces PAR
//! SALARIE d'un mois deja produit, redecoupees sans rien reproduire.
//!
//! Ne touche ni la DSN, ni les contrats, ni la DPAE, ni le fichier groupe des bulletins, et n'avance aucun
//! compteur de bulletin ni de contrat. Les numeros de bulletin et de contrat se LISENT dans le fichier groupe
//! archive (`apercu_dsn::numeroter_et_lire`) ; le N CE de depart des certificats se relit dans leur
//! recapitulatif, et ne se prend a la sequence (`prochain_continu`) que si les certificats n'existent pas
//! encore : ils sont alors emis pour de bon, sous le verrou des compteurs.
//!
//! Les PDF sont horodates : ils se prouvent par leur contenu (texte des pages), pas par leurs octets.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use crate::calcul::Paie;
use crate::conges_spectacles::{self, PaieCs};
use droit::contexte::Contexte;
use socle::ecriture::json_py;
use crate::engagement;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::lire_json;
use socle::echec::Echec;
use crate::routes_dossier_salarie::{avec, certificats_dus, charger, cle_de, ecrire_pdf, etiquette, premier_ce};
use socle::verrou::Verrou;

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// `envoi.nom_fichier`.
fn nom_fichier(prefixe: &str, cle: &str, etiq: &str) -> String {
    format!("{}_{}_{}.pdf", prefixe, cle, etiq)
}

/// `Contexte.prochain_continu` : une sequence qui ne repart pas a 1 chaque an. La LECTURE balaie toutes les
/// annees de `compteurs.json` et retient le maximum.
pub(crate) fn prochain_continu(ctx: &Contexte, cle: &str, defaut: i64) -> R<i64> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let mut vu = defaut - 1;
    if p.exists() {
        let c = lire_json(&p).map_err(|m| Echec::from(m))?;
        if let Value::Object(annees) = &c {
            for valeurs in annees.values() {
                if let Some(v) = valeurs.as_object().and_then(|o| o.get(cle)) {
                    let n = match v {
                        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
                        Value::String(s) => s.trim().parse().ok(),
                        Value::Bool(b) => Some(*b as i64),
                        _ => None,
                    }
                    .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr_valeur(v))))?;
                    vu = vu.max(n);
                }
            }
        }
    }
    Ok(vu + 1)
}

/// `enregistrer_compteur` puis `valider_compteurs` : ecrit `compteurs.json` (JSON a deux espaces, sans saut de
/// ligne final, comme `write_text(json.dumps(..., indent=2))`).
fn ecrire_compteur(ctx: &Contexte, cle: &str, annee: i32, valeur: i64) -> R<()> {
    let p = ctx.racine.join("config").join("compteurs.json");
    let mut c = if p.exists() { lire_json(&p).map_err(Echec::from)? } else { json!({}) };
    let an = annee.to_string();
    let lu = socle::valeur::nom_type(&c);
    let racine = c.as_object_mut().ok_or_else(|| erreur("AttributeError", format!("config/compteurs.json : {}", socle::fichiers::attendu("un objet", lu))))?;
    let annee_obj = racine.entry(an).or_insert_with(|| json!({}));
    let annee_obj = annee_obj.as_object_mut().ok_or_else(|| erreur("AttributeError", "l'annee des compteurs n'est pas un dictionnaire"))?;
    annee_obj.insert(cle.to_string(), json!(valeur));
    std::fs::write(&p, json_py(&c, 2)).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))
}

/// `verrou_compteurs(ctx, qui=...)` : refuse si un autre tient les compteurs.
pub(crate) fn prendre_le_verrou(ctx: &Contexte, qui: &str) -> R<Verrou> {
    let chemin = socle::verrou::chemin_compteurs(&ctx.racine);
    let maintenant = crate::incidents::maintenant().replace('T', " ");
    let marque = socle::verrou::marque(qui, std::process::id(), &maintenant);
    match Verrou::prendre(&chemin, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur("OSError", m))? {
        Ok(v) => Ok(v),
        Err(v) => Err(erreur("CompteursVerrouilles", v.message())),
    }
}

/// `operations_documents.reecrire_pieces`.
pub fn ecrire_documents(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = crate::routes_fiches::exiger(charge, "mois")?
        .as_str()
        .ok_or_else(|| erreur("TypeError", "`mois` : un texte est attendu"))?
        .to_string();
    let mois = mois.as_str();
    let sortie = ctx.racine.join("sorties").join(mois);
    let etiq = etiquette(mois)?;
    let groupe = sortie.join(format!("Bulletins_{}.pdf", etiq));
    if !groupe.exists() {
        return Err(refus(format!(
            "{} n'existe pas : le mois n'a pas ete produit, il n'y a rien a redecouper. C'est `produire` qu'il faut -- et sur un mois deja declare, personne : les numeros changeraient.",
            groupe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        )));
    }
    let saisie = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !saisie.exists() {
        return Err(refus(format!(
            "Aucune saisie pour {} : les bulletins sont archives mais la paie qui les a produits n'est plus la.",
            engagement::libelle_mois(mois)
        )));
    }

    let mut avertissements: Vec<String> = Vec::new();
    // La lecture ne RESERVE aucun numero : ils viennent de l'archive.
    let (_donnees, mut paies) = charger(ctx, &saisie)?;
    let annee: i32 = mois
        .chars()
        .take(4)
        .collect::<String>()
        .parse()
        .map_err(|_| avec("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(mois))))?;
    let ent = crate::oracle::entreprise_du_mois(ctx, mois).map_err(Echec::from)?;

    // UN CONTRAT AJOUTE APRES LA LIMITE DE L'ANNULE-ET-REMPLACE (21/10/2026) n'est pas dans le fichier archive : il n'a
    // ni numero de bulletin ni page a redecouper. Son bulletin est celui de sa correction, ecrit avec la DSN qui le
    // declare (`produire_regularisations::ecrire_rectificatifs`).
    let ajoutes: Vec<(String, String)> = crate::routes_regularisation::ce_qui_a_change(&HashMap::new(), ctx, mois)?
        .map(|c| c.contrats.into_iter().filter(|x| x.ajoute).map(|x| (x.salarie, x.debut)).collect())
        .unwrap_or_default();
    if !ajoutes.is_empty() {
        paies.retain(|p| {
            let cle = p.salarie.get("_cle").map(socle::valeur::py_str).unwrap_or_default();
            !ajoutes.contains(&(cle, p.engagement.debut.iso()))
        });
        avertissements.push(format!(
            "{} contrat(s) ajouté(s) après la DSN de {} : leur bulletin part avec la correction, dans la DSN qui les déclare.",
            ajoutes.len(),
            engagement::libelle_mois(mois)
        ));
    }
    let (_lus, avert) = crate::apercu_dsn::numeroter_et_lire(ctx, &mut paies, &groupe)?;
    avertissements.extend(avert);

    let (pour_cs, avert_cs) = certificats_dus(ctx, &paies)?;
    avertissements.extend(avert_cs);
    let paies_cs: Vec<&PaieCs> = pour_cs.iter().filter_map(|(_, _, v)| v.as_ref()).collect();

    let recap = sortie.join(format!("CertificatsCS_{}_recapitulatif.txt", etiq));
    let mut certificats_ecrits = false;
    let _verrou = prendre_le_verrou(ctx, "interface")?;
    let mut premier_cs = premier_ce(&recap)?;
    if premier_cs.is_none() && !paies_cs.is_empty() {
        let premier = prochain_continu(ctx, "certificat_cs", 1)?;
        premier_cs = Some(premier);
        // Les N CE se suivent dans l'ordre des paies, de contrat en contrat.
        let mut paires: Vec<(&PaieCs, i64)> = Vec::new();
        let mut numero = premier;
        for p in &paies_cs {
            paires.push((p, numero));
            numero += conges_spectacles::decouper(&p.engagement)?.len() as i64;
        }
        let nom_groupe = format!("CertificatsCS_{}.pdf", etiq);
        let description = conges_spectacles::description_numerotee(&paires, &ent)?;
        ecrire_pdf(&description, &sortie.join(&nom_groupe))?;
        let texte = conges_spectacles::recapitulatif(&paires, &ent)?;
        std::fs::write(&recap, texte).map_err(|e| erreur("OSError", format!("{} : {}", recap.display(), e)))?;
        ecrire_compteur(ctx, "certificat_cs", annee, numero - 1)?;
        certificats_ecrits = true;
        avertissements.push(format!(
            "Certificats Conges Spectacles de {} ecrits pour la premiere fois ({}) : les N CE {} et suivants sont desormais consommes sur la sequence Audiens.",
            engagement::libelle_mois(mois),
            nom_groupe,
            premier
        ));
    } else if premier_cs.is_none() {
        premier_cs = Some(prochain_continu(ctx, "certificat_cs", 1)?);
    }
    let premier_cs = premier_cs.unwrap_or(1);

    let dossier = sortie.join("envoi");
    let fichiers = pieces_individuelles(ctx, &paies, &ent, &dossier, &etiq, mois, premier_cs, &pour_cs)?;
    let mut noms = fichiers;
    noms.sort();
    Ok(json!({
        "dossier": crate::conges_spectacles::chemin_texte(&dossier),
        "fichiers": noms,
        "certificats_ecrits": certificats_ecrits,
        "avertissements": avertissements,
    }))
}

/// `envoi.pieces_individuelles` : un `Bulletin_<cle>_<Mois>.pdf` (tous ses bulletins, cumuls lus sur le mois
/// ENTIER) et un `CertificatCS_<cle>_<Mois>.pdf` par salarie, aux numeros du mois. Rend les noms ecrits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pieces_individuelles(
    ctx: &Contexte,
    paies: &[Paie],
    ent: &Value,
    dossier: &Path,
    etiq: &str,
    mois: &str,
    premier_cs: i64,
    pour_cs: &[crate::routes_dossier_salarie::Du],
) -> R<Vec<String>> {
    std::fs::create_dir_all(dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
    // Un passage pour repartir les pieces par salarie, EN GARDANT l'ordre du fichier groupe et les N CE
    // attribues ; l'ecriture vient apres.
    let mut ordre_bulletins: Vec<String> = Vec::new();
    let mut bulletins: HashMap<String, Vec<Paie>> = HashMap::new();
    let mut ordre_certificats: Vec<String> = Vec::new();
    let mut certificats: HashMap<String, Vec<(&PaieCs, i64)>> = HashMap::new();
    let mut numero_cs = premier_cs;
    for p in paies {
        let cle = cle_de(p);
        if !p.engagement.sans_paie() {
            if !bulletins.contains_key(&cle) {
                ordre_bulletins.push(cle.clone());
            }
            bulletins.entry(cle.clone()).or_default().push(p.clone());
        }
        let debut = p.engagement.debut.iso();
        let pc = pour_cs.iter().find(|(c, d, _)| *c == cle && *d == debut).and_then(|(_, _, v)| v.as_ref());
        if let Some(pc) = pc {
            if !certificats.contains_key(&cle) {
                ordre_certificats.push(cle.clone());
            }
            certificats.entry(cle.clone()).or_default().push((pc, numero_cs));
            numero_cs += conges_spectacles::decouper(&pc.engagement)?.len() as i64;
        }
    }
    let mut noms = Vec::new();
    for cle in &ordre_bulletins {
        let siennes = &bulletins[cle];
        let nom = nom_fichier("Bulletin", cle, etiq);
        let description = crate::bulletin::jeux_et_description(ctx, siennes, ent, mois, Some(paies), false, &HashMap::new())
            .map_err(|m| avec("ValueError", m))?;
        ecrire_pdf(&description, &dossier.join(&nom))?;
        noms.push(nom);
    }
    for cle in &ordre_certificats {
        let nom = nom_fichier("CertificatCS", cle, etiq);
        let description = conges_spectacles::description_numerotee(&certificats[cle], ent)?;
        ecrire_pdf(&description, &dossier.join(&nom))?;
        noms.push(nom);
    }
    Ok(noms)
}
