//! Les routes qui ECRIVENT du lot G4 (`moteur/web.py`, `operations_paies`, `operations_documents`,
//! `operations_retours`, `operations_exports`, `operations_ecritures`, `operations_casc`, `simulation`) :
//! `valider-paie`, `contrat-recu`, `journal-reprise`, `ecrire-documents`, `exporter`, `retenir-logiciel`,
//! `casc-regularise` et `courriels-fictifs`.
//!
//! Memes fichiers ecrits, memes reponses que le Python (ce qui compte, ce sont les VALEURS). Un refus garde sa
//! phrase quand elle est utile a la page ; la classe de l'exception peut differer.
//!
//! - `valider-paie` ferme (ou rouvre) l'edition d'une paie : il ne touche que la saisie, par la meme porte que
//!   `enregistrer-saisie` (`routes_ecritures::ecrire_saisie`), sans fusionner avec l'ancien fichier.
//! - `contrat-recu` note au journal qu'un contrat signe est revenu (ou retire la note).
//! - `exporter` route vers tous les etats (`etats_paie`, `etats_organismes`, `etats_personnel`, `etats_compta`,
//!   `casc_regularisation`) et rend le tableau en JSON (`tableaux_export::brut_tableau`).
//! - `retenir-logiciel` ecrit `config/compta.json` et rend le controle du plan pour ce logiciel.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::ecriture::ecrire_json;
use socle::valeur::py_str;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::journal::{self, Note};
use socle::echec::Echec;
use crate::routes_ouverture::Refus;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 8] = ["valider-paie", "contrat-recu", "retenir-logiciel", "casc-regularise", "courriels-fictifs", "journal-reprise", "ecrire-documents", "exporter"];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "valider-paie" => valider_paie(o, ctx, charge),
        "contrat-recu" => contrat_recu(o, ctx, charge),
        "retenir-logiciel" => retenir_logiciel(ctx, charge),
        "casc-regularise" => casc_regularise(ctx, charge),
        "exporter" => exporter(ctx, charge),
        "ecrire-documents" => crate::ecrire_documents::ecrire_documents(ctx, charge),
        "journal-reprise" => crate::reprise_journal::reprise(ctx, charge, &maintenant()),
        "courriels-fictifs" => crate::simulation_courriels::courriels_fictifs(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_echec(r: Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

/// `charge["cle"]` : un texte requis.
fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes_fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

/// `bool(charge.get(cle, defaut))`.
fn booleen(charge: &Value, cle: &str, defaut: bool) -> bool {
    charge.get(cle).map_or(defaut, vrai)
}

/// `datetime.now().isoformat(timespec="seconds")`, a l'heure de Paris.
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

// ------------------------------------------------------------------ valider-paie

/// `operations_paies.valider_paie` : ferme (ou rouvre) l'edition d'une paie. N'ecrit que la saisie, et refuse
/// une paie absente du fichier ENREGISTRE : valider ce qui n'est pas enregistre poserait une pastille verte sur
/// un formulaire qu'un rechargement de page effacerait.
fn valider_paie(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let valide = booleen(charge, "valide", true);
    let forcer = booleen(charge, "forcer", false);
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    if !chemin.exists() {
        return Err(refus(format!("Le mois {} n'est pas enregistre : il n'y a rien a valider. Enregistrer la saisie d'abord.", mois)));
    }
    let mut data = crate::routes_saisie::lire_saisie(&chemin)?;
    let rangs = crate::routes_ecritures::identifiants(ctx, &data).map_err(Echec::from)?;
    // LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : deux contrats du meme salarie peuvent commencer le meme jour.
    // `numero` (facultatif) dit lequel ; sans numero pose, `rang` (facultatif) : son rang parmi ceux de ce jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(py_str);
    let rang = charge.get("rang").and_then(Value::as_u64);
    let du_jour: Vec<usize> = (0..rangs.len()).filter(|&j| rangs[j].0 == salarie && rangs[j].1 == debut).collect();
    let par_numero = numero.as_ref().and_then(|n| du_jour.iter().copied().find(|&j| rangs[j].2.as_ref() == Some(n)));
    let i = match (par_numero, du_jour.as_slice()) {
        (_, []) => {
            return Err(refus(format!(
                "Aucune paie de {} commencant le {} dans la saisie enregistree de {}. Enregistrer la saisie avant de valider.",
                salarie, debut, mois
            )))
        }
        (Some(j), _) => j,
        // Un seul contrat ce jour-la : c'est lui, quel que soit le numero que l'ecran lui prete.
        (None, [j]) => *j,
        (None, plusieurs) => match rang.and_then(|r| plusieurs.get(r as usize)) {
            Some(j) if numero.is_none() || rangs[*j].2.is_none() => *j,
            _ => {
                let lesquels: Vec<String> = plusieurs.iter().map(|&j| rangs[j].2.clone().unwrap_or_else(|| "sans numero".into())).collect();
                return Err(refus(format!(
                    "{} a {} contrats qui commencent le {} dans la saisie de {} ({}){} : dire lequel valider (son numero, ou son rang parmi eux). Rien n'est ecrit.",
                    salarie,
                    plusieurs.len(),
                    debut,
                    mois,
                    lesquels.join(", "),
                    numero.as_ref().map_or(String::new(), |n| format!(", aucun sous le n° {}", n))
                )));
            }
        },
    };
    let a_l_index = |data: &mut Value| -> R<Map<String, Value>> {
        match data.get_mut("engagements").and_then(|e| e.get_mut(i)) {
            Some(Value::Object(m)) => Ok(std::mem::take(m)),
            _ => Err(erreur("TypeError", "engagements : un objet est attendu")),
        }
    };
    let mut eng = a_l_index(&mut data)?;
    if valide {
        eng.remove("valide");
        eng.remove("rouverte");
    } else {
        eng.remove("valide");
        // Rouverte A LA MAIN : une paie dont la DSN est partie se lit validee sans l'avoir ete ; la marque dit
        // qu'on l'a rouverte.
        eng.insert("rouverte".into(), Value::String(maintenant()));
    }
    data["engagements"][i] = Value::Object(eng);
    if valide {
        // VALIDER FIGE LA CARTE (`calcul_du_mois::sceller`) : la paie se calcule une fois, ici, et ce que l'ecran en
        // montre s'ecrit avec l'empreinte de ce dont ce calcul depend. Ensuite elle se lit.
        let (rep, entrees) = crate::calcul_du_mois::calculer_suivi(o, &json!({"saisie": data.clone()}))
            .map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &data, m)))?;
        let (_, vues) = crate::routes_ouverture::vues_du_calcul(ctx, &data, &rep, false).map_err(Echec::from)?;
        let vue = vues.get(i).ok_or_else(|| erreur("IndexError", "le calcul a rendu une paie de moins que la saisie"))?;
        data["engagements"][i]["valide"] = crate::calcul_du_mois::sceller(&maintenant(), vue, &data, i, &entrees, mois);
    }
    let rendu = crate::calcul_du_mois::valide_le(data["engagements"][i].get("valide"));
    // `fusionner=False` : `data` EST le fichier, validation comprise.
    let Value::Object(donnees) = &data else {
        return Err(erreur("AttributeError", "la saisie n'est pas un objet"));
    };
    crate::routes_ecritures::ecrire_saisie(o, ctx, mois, donnees, forcer, false)?;
    Ok(json!({"salarie": salarie, "debut": debut, "valide": rendu}))
}

// ------------------------------------------------------------------ contrat-recu

/// `operations_documents.contrat_recu` : un humain dit que le contrat signe est revenu (ou retire la note).
/// Une ligne de journal, pas un etat de la saisie : elle date un fait du monde. Rien ne part, aucun compteur ne
/// bouge.
fn contrat_recu(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let recu = booleen(charge, "recu", true);
    // `numero` (facultatif) dit QUEL contrat du jour : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(py_str);
    contrat_recu_avec(o, ctx, mois, salarie, debut, numero.as_deref(), recu, "interface", None)
}

/// `contrat_recu(ctx, mois, salarie, debut, recu=..., qui=...)` : la meme note, `qui` donne (l'horloge d'une
/// compagnie fictive passe `horloge`).
pub(crate) fn contrat_recu_par(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    qui: &str,
) -> R<Value> {
    contrat_recu_avec(o, ctx, mois, salarie, debut, numero, recu, qui, None)
}

/// `contrat_recu(..., courriel=...)` : quand la releve des signatures a trouve la reponse « Je signe », elle passe
/// `courriel` (code, adresse, date, identifiant du message, fichier archive, ...). La ligne est alors datee de la
/// SIGNATURE (`courriel["quand"]`), pas du releve, avec le message pour origine, et son detail porte `origine:
/// "courriel"` puis les champs de `courriel` dans l'ordre ou ils sont donnes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn contrat_recu_avec(
    o: &HashMap<String, String>,
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    qui: &str,
    courriel: Option<&Map<String, Value>>,
) -> R<Value> {
    // Le Python calcule sans rien ecrire (`charger`) : `cache = false`, `paies/AAAA-MM.calcul.json` n'est ni lu ni ecrit
    // (reste 163 : il etait ecrit puis remis tel qu'il etait).
    let paies = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, mois, None, false).map_err(en_echec)?.2;
    // LE CONTRAT, pas le salarie : `numero` est celui sous lequel le journal le connait (la releve), ou celui que l'ecran affiche.
    let numero = numero.filter(|n| !n.is_empty());
    let lignes = crate::etats::charger(&ctx.racine)?;
    let tous = crate::routes_mois::contrats_des_paies(&paies);
    let cles = crate::etats::cles_des_paies(&lignes, &tous)?;
    let du_jour: Vec<usize> = (0..paies.len()).filter(|&i| tous[i].0 == salarie && tous[i].1 == debut).collect();
    let trouvee = match numero {
        None => du_jour.first().copied(),
        Some(n) => du_jour.iter().copied().find(|&i| cles[i][0].2 == n).or_else(|| du_jour.iter().copied().find(|&i| tous[i].2 == n)),
    };
    let Some(rang) = trouvee else {
        return Err(refus(format!(
            "Aucune paie de {} commencant le {}{} dans la saisie enregistree de {}.",
            salarie,
            debut,
            numero.map_or(String::new(), |n| format!(" (n° {})", n)),
            mois
        )));
    };
    let p = &paies[rang];
    // La ligne porte le numero du contrat tel que le journal le connait (celui de sa demande), a defaut le sien.
    let numero_note = if cles[rang][0].2.is_empty() { Value::Null } else { json!(cles[rang][0].2) };
    let texte = |cle: &str| p.salarie.get(cle).filter(|v| vrai(v)).map(py_str);
    let prenom = texte("prenom_affichage").or_else(|| texte("prenom")).unwrap_or_default();
    let nom = format!("{} {}", texte("nom").unwrap_or_default(), prenom);
    let mut detail = json!({
        "salarie": salarie, "debut": debut, "recu": recu,
        "numero": numero_note, "nom": nom,
    });
    let courriel = courriel.filter(|c| !c.is_empty());
    let texte_du_courriel = |cle: &str, defaut: &str| courriel.and_then(|c| c.get(cle)).map_or_else(|| defaut.to_string(), py_str);
    let (reponse, quand, origine) = match courriel {
        Some(c) => {
            detail["origine"] = json!("courriel");
            for (cle, valeur) in c {
                detail[cle.as_str()] = valeur.clone();
            }
            (
                format!("contrat signe par courriel ({}, {})", texte_du_courriel("code", "?"), texte_du_courriel("adresse", "?")),
                c.get("quand").filter(|v| vrai(v)).map(py_str).unwrap_or_default(),
                format!("courriel {}", texte_du_courriel("message", "?")),
            )
        }
        None => (
            (if recu { "contrat signe recu, range a la main" } else { "mention retiree : le contrat signe n'est pas la" }).to_string(),
            String::new(),
            String::new(),
        ),
    };
    let note = Note {
        quoi: "contrat-recu",
        mois: Some(mois),
        fichier: None,
        mode: journal::REEL,
        qui,
        accepte: true,
        destinataire: "",
        flux: "",
        reponse: &reponse,
        trace: None,
        detail: Some(&detail),
        quand: &quand,
        origine: &origine,
    };
    let ligne = journal::noter(&ctx.racine, &note, &maintenant()).map_err(Echec::from)?;
    Ok(json!({
        "salarie": salarie, "debut": debut,
        "recu": if recu { ligne["quand"].clone() } else { Value::Null },
        "ligne": ligne,
    }))
}

// ------------------------------------------------------------------ retenir-logiciel

/// `operations_ecritures.retenir_logiciel` : le logiciel du comptable, et l'analytique, retenus une fois pour
/// toutes. Rend le controle du plan pour ce logiciel. Ce n'est pas un export : un reglage, pas une donnee de paie.
fn retenir_logiciel(ctx: &Contexte, charge: &Value) -> R<Value> {
    let logiciel = charge.get("logiciel").filter(|v| vrai(v));
    let analytique = booleen(charge, "analytique", false);
    let nom = logiciel.map(py_str).unwrap_or_default();
    if logiciel.is_some() {
        let connus = crate::routes_dossiers::logiciels_connus();
        if !connus.iter().any(|l| *l == nom) {
            return Err(refus(format!("Logiciel inconnu : « {} ».", nom)));
        }
    }
    let chemin = ctx.racine.join("config").join("compta.json");
    let contenu = json!({
        "_quoi": "Le logiciel du comptable, retenu pour les écritures comptables (Exports, ou paie compta-logiciel).",
        "logiciel": nom,
        "analytique": analytique,
    });
    ecrire_json(&chemin, &contenu, 2).map_err(|m| erreur("OSError", m))?;
    crate::routes_dossiers::etat_logiciel(ctx).map_err(Echec::from)
}

// ------------------------------------------------------------------ casc-regularise

/// `operations_casc.marquer_regularise` : note qu'une annee de CASC-SVP est regularisee (apres le virement).
fn casc_regularise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = crate::routes_fiches::exiger(charge, "annee")?;
    let vire_le = crate::routes_fiches::exiger(charge, "vire_le")?;
    let montant = crate::routes_fiches::exiger(charge, "montant")?;
    let aujourd_hui = maintenant();
    let qui = charge.get("qui").filter(|v| vrai(v)).map(py_str).unwrap_or_else(|| "interface".to_string());
    crate::casc_regularisation::marquer_regularise(ctx, annee, vire_le, montant, &qui, &aujourd_hui[..10])
}

// ------------------------------------------------------------------ exporter

/// `charge[cle] or None` : un texte, ou rien.
fn texte_ou_rien(charge: &Value, cle: &str) -> Option<String> {
    charge.get(cle).filter(|v| vrai(v)).map(py_str)
}

/// `operations_exports.exporter` : le verbe unique, `exporter <etat>`. Lit la charge (etat, mois, annee,
/// toutes_les_annees, regroupement, format, coordonnees, apercu, logiciel, analytique), refuse un etat inconnu,
/// ne passe a l'etat que ce que sa forme demande, puis rend l'etat tel que `js.brut` le rend.
fn exporter(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(match exporter_objet(ctx, charge)? {
        Exporte::Casc(v) => v,
        Exporte::Livre(l) => l.json(),
        Exporte::Tableau(t) => crate::tableaux_export::brut_tableau(&t),
    })
}

/// Ce qu'un export rend, avant d'etre mis en JSON : la ligne de commande (`paie exporter`) le rend en texte a partir de l'objet, comme le
/// Python le faisait (le chemin ecrit en entier, les montants en `Decimal`).
pub(crate) enum Exporte {
    Casc(Value),
    Livre(crate::etats_paie::LivreDePaie),
    Tableau(crate::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON. La charge peut porter `format_du_logiciel_retenu` : sans `format`, les ecritures comptables laissent
/// le logiciel retenu (`compta-logiciel`) choisir leur format (la ligne de commande), au lieu du CSV (la page, qui envoie son format).
pub(crate) fn exporter_objet(ctx: &Contexte, charge: &Value) -> R<Exporte> {
    use crate::etats_paie::Demande;
    let etat = texte_ou_rien(charge, "etat").unwrap_or_else(|| "livre-de-paie".to_string());
    let tables = crate::routes_dossiers::tables();
    let etats = tables["etats"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let Some(forme) = etats.iter().find(|e| e["etat"].as_str() == Some(etat.as_str())) else {
        let mut noms: Vec<&str> = etats.iter().filter_map(|e| e["etat"].as_str()).collect();
        noms.sort_unstable();
        return Err(refus(format!("État inconnu : « {} ». Les états possibles sont {}.", etat, noms.join(", "))));
    };
    let mut d = Demande {
        mois: texte_ou_rien(charge, "mois"),
        annee: texte_ou_rien(charge, "annee"),
        toutes: charge.get("toutes_les_annees").is_some_and(vrai),
        regroupement: texte_ou_rien(charge, "regroupement").unwrap_or_else(|| "salarie".to_string()),
        format: texte_ou_rien(charge, "format").unwrap_or_else(|| {
            if etat == "ecritures-comptables" && charge.get("format_du_logiciel_retenu").is_some_and(vrai) { String::new() } else { "csv".to_string() }
        }),
        coordonnees: charge.get("coordonnees").is_some_and(vrai),
        ecrire: !charge.get("apercu").is_some_and(vrai),
        logiciel: texte_ou_rien(charge, "logiciel"),
        analytique: charge.get("analytique").map(vrai),
    };
    if etat == "regularisation-casc-svp" {
        let demande = crate::casc_regularisation::Demande { mois: d.mois.clone(), annee: d.annee.clone(), toutes_les_annees: d.toutes, ..Default::default() };
        let mut r = crate::casc_regularisation::regularisation(ctx, &demande)?;
        if d.ecrire {
            crate::casc_regularisation::ecrire(ctx, &mut r)?;
        }
        return Ok(Exporte::Casc(crate::casc_regularisation::vers_json(&r)));
    }
    if etat == "livre-de-paie" {
        return Ok(Exporte::Livre(crate::etats_paie::livre_de_paie(ctx, &d)?));
    }
    // Ce que l'etat ne demande pas ne lui est pas passe : un regroupement par defaut (« salarie ») n'a pas de
    // sens pour les cotisations.
    let questions: Vec<&str> = forme["questions"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !questions.contains(&"regroupement") {
        d.regroupement = String::new();
    } else {
        let permis: Vec<&str> = forme["regroupements"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if !permis.contains(&d.regroupement.as_str()) {
            d.regroupement = permis.first().map(|s| s.to_string()).unwrap_or_default();
        }
    }
    if !questions.contains(&"coordonnees") {
        d.coordonnees = false;
    }
    let tableau = match etat.as_str() {
        "livre-de-paie-detaille" => crate::etats_paie::livre_detaille(ctx, &d),
        "recapitulatif-annuel" => crate::etats_paie::recapitulatif_annuel(ctx, &d),
        "masse-salariale" => crate::etats_paie::masse_salariale(ctx, &d),
        "etat-des-virements" => crate::etats_paie::etat_des_virements(ctx, &d),
        "prelevement-a-la-source" => crate::etats_paie::prelevement_a_la_source(ctx, &d),
        "cotisations-par-organisme" => crate::etats_organismes::cotisations_par_organisme(ctx, &d),
        "versements-organismes" => crate::etats_organismes::versements_organismes(ctx, &d),
        "bordereau-urssaf" => crate::etats_organismes::bordereau_urssaf(ctx, &d),
        "fcap-svp" => crate::etats_organismes::fcap_svp(ctx, &d),
        "afdas-2642" => crate::etats_organismes::afdas_2642(ctx, &d),
        "paritarisme-branche" => crate::etats_organismes::paritarisme_branche(ctx, &d),
        "reduction-generale" => crate::etats_paie::reduction_generale(ctx, &d),
        "assiettes" => crate::etats_paie::assiettes(ctx, &d),
        "heures-et-cachets" => crate::etats_paie::heures_et_cachets(ctx, &d),
        "cout-par-spectacle" => crate::etats_paie::cout_par_spectacle(ctx, &d),
        "contrats" => crate::etats_personnel::liste_des_contrats(ctx, &d),
        "registre-du-personnel" => crate::etats_personnel::registre_du_personnel(ctx, &d),
        "fichier-du-personnel" => crate::etats_personnel::fichier_du_personnel(ctx, &d),
        "journal-des-declarations" => crate::etats_personnel::journal_des_declarations(ctx, &d),
        "ecritures-comptables" => crate::etats_compta::ecritures_comptables(ctx, &d),
        "grand-livre-paie" => crate::etats_compta::grand_livre(ctx, &d),
        "balance-paie" => crate::etats_compta::balance(ctx, &d),
        "provision-conges" => crate::etats_compta::provision_conges(ctx, &d),
        "taxe-sur-les-salaires" => crate::etats_compta::taxe_sur_les_salaires(ctx, &d),
        autre => Err(refus(format!("État inconnu : « {} ».", autre))),
    }?;
    Ok(Exporte::Tableau(tableau))
}
