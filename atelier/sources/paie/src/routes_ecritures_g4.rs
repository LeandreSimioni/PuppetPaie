//! Les routes qui ECRIVENT du lot G4 : `valider-engagement`, `retirer-engagement`, `contrat-recu`, `journal-reprise`, `ecrire-documents`,
//! `exporter`, `retenir-logiciel`, `casc-regularise` et `courriels-fictifs`. Un refus garde sa phrase quand elle est
//! utile a la page.
//!
//! - `valider-engagement` valide un engagement (ses paies s'ecrivent) ou le remet en saisie (« Modifier… »).
//! - `retirer-engagement` supprime un engagement dont rien n'est parti, ou le rend non actif.
//! - `contrat-recu` note au journal qu'un contrat signe est revenu (ou retire la note).
//! - `exporter` route vers tous les etats (`etats_paie`, `etats_organismes`, `etats_personnel`, `etats_compta`,
//!   `casc_regularisation`) et rend le tableau en JSON (`tableaux_export::brut_tableau`).
//! - `retenir-logiciel` ecrit `config/compta.json` et rend le controle du plan pour ce logiciel.


use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::ecriture::ecrire_json;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::journal::{self, Note};
use socle::echec::Echec;
use socle::echec::Refus;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 8] = ["valider-engagement", "retirer-engagement", "contrat-recu", "retenir-logiciel", "casc-regularise", "courriels-fictifs", "journal-reprise", "exporter"];

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "valider-engagement" => valider_engagement(ctx, charge),
        "retirer-engagement" => retirer_engagement(ctx, charge),
        "contrat-recu" => contrat_recu(ctx, charge),
        "retenir-logiciel" => retenir_logiciel(ctx, charge),
        "casc-regularise" => casc_regularise(ctx, charge),
        "exporter" => exporter(ctx, charge),
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

// ------------------------------------------------------------------ valider-engagement

/// La classe du refus qui pose la question avant de modifier un engagement dont des documents sont partis
/// (`engagement::etats::QUESTION_DOCUMENTS_PARTIS`) : la page la montre, et renvoie la demande avec `confirme`.
pub const CLASSE_DOCUMENTS_PARTIS: &str = "DocumentsPartis";

/// « Valider l'engagement » (`valide` vrai, par defaut) ou « Modifier… » (`valide` faux). L'engagement se designe
/// dans la saisie enregistree de `mois` : `salarie`, `debut`, et, si deux contrats du salarie commencent ce jour-la,
/// `numero` ou `rang` (son rang parmi eux). Refus : un engagement absent de la saisie enregistree (valider ce qui
/// n'est pas enregistre poserait une pastille verte sur un formulaire qu'un rechargement effacerait).
///
/// VALIDER ecrit les paies de l'engagement, une par mois qu'il couvre (`engagement::etats::valider`) ; un engagement
/// deja valide le reste, tel quel. MODIFIER le remet en saisie (`engagement::etats::modifier`) ; si les documents d'un
/// de ses mois sont partis, le moteur pose d'abord sa question (`CLASSE_DOCUMENTS_PARTIS`) et ne modifie qu'avec
/// `confirme`.
fn valider_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let valide = booleen(charge, "valide", true);
    let confirme = booleen(charge, "confirme", false);
    let (data, i) = engagement_designe(ctx, charge)?;
    let deja = data["engagements"][i].get("valide").and_then(|v| v.get("le")).cloned();
    let rendu = if valide {
        match deja {
            Some(le) => le,
            None => Value::String(engagement::etats::valider(ctx, mois, i).map_err(|m| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &data, m)))?.le().to_string()),
        }
    } else {
        if !confirme {
            if let Some(q) = engagement::etats::question_avant_de_modifier(ctx, mois, i).map_err(Echec::from)? {
                return Err(erreur(CLASSE_DOCUMENTS_PARTIS, q));
            }
        }
        engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
        Value::Null
    };
    Ok(json!({"salarie": salarie, "debut": debut, "valide": rendu}))
}

/// L'ENGAGEMENT QUE LA PAGE DESIGNE dans la saisie enregistree de `mois` : `salarie`, `debut`, et, si deux contrats du
/// salarie commencent ce jour-la, `numero` ou `rang` (son rang parmi eux). Rend la saisie du mois et son rang dans
/// l'ecran. Refus : un engagement absent de la saisie enregistree (agir sur ce qui n'est pas enregistre poserait un etat
/// sur un formulaire qu'un rechargement effacerait).
fn engagement_designe(ctx: &Contexte, charge: &Value) -> R<(Value, usize)> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Err(refus(format!("Le mois {} n'est pas enregistre : il n'y a pas d'engagement a designer. Enregistrer la saisie d'abord.", mois)));
    };
    let rangs = crate::routes_ecritures::identifiants(ctx, &data).map_err(Echec::from)?;
    // LE CONTRAT, PAS LE SALARIE DU JOUR (`jumeaux`) : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    let rang = charge.get("rang").and_then(Value::as_u64);
    let du_jour: Vec<usize> = (0..rangs.len()).filter(|&j| rangs[j].0 == salarie && rangs[j].1 == debut).collect();
    let par_numero = numero.as_ref().and_then(|n| du_jour.iter().copied().find(|&j| rangs[j].2.as_ref() == Some(n)));
    let i = match (par_numero, du_jour.as_slice()) {
        (_, []) => {
            return Err(refus(format!(
                "Aucun engagement de {} commencant le {} dans la saisie enregistree de {}. Enregistrer la saisie avant de valider.",
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
    Ok((data, i))
}

// ------------------------------------------------------------------ retirer-engagement

/// La classe du refus de supprimer un engagement dont quelque chose est parti : la page montre la phrase, et renvoie la
/// demande avec `non_actif` si l'utilisateur veut qu'il ne soit plus actif.
pub const CLASSE_ENGAGEMENT_PARTI: &str = "EngagementParti";

/// « Retirer cet engagement » : l'engagement se designe comme pour `valider-engagement` (`engagement_designe`). Tant
/// que rien n'est parti de lui, il se supprime, meme valide, et rend son numero (`engagement::etats::supprimer`). Des
/// qu'un envoi est parti, le moteur refuse (`CLASSE_ENGAGEMENT_PARTI`, la phrase dit ce qui est parti) ; avec
/// `non_actif`, il devient non actif (`engagement::etats::desactiver`) et la reponse dit ce que cela rend a faire
/// (`a_faire`, en phrases : la DPAE a annuler, la DSN d'un mois a corriger). Rien ne part de lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let (_, i) = engagement_designe(ctx, charge)?;
    if !booleen(charge, "non_actif", false) {
        return match engagement::etats::supprimer(ctx, mois, i) {
            Ok(()) => Ok(json!({"retire": "supprime"})),
            Err(m) => Err(erreur(CLASSE_ENGAGEMENT_PARTI, m)),
        };
    }
    let n = engagement::etats::desactiver(ctx, mois, i).map_err(Echec::from)?;
    let departs = crate::journal::departs(&ctx.racine).map_err(Echec::from)?;
    Ok(json!({"retire": "non_actif", "le": n.le(), "a_faire": a_faire_en_phrases(&n.a_refaire(ctx, &departs))}))
}

/// Ce qu'un engagement non actif rend a faire, en phrases pour l'ecran.
pub(crate) fn a_faire_en_phrases(a: &[engagement::etats::ARefaire]) -> Vec<String> {
    use engagement::etats::ARefaire;
    a.iter()
        .filter_map(|x| match x {
            ARefaire::DpaeAAnnuler { .. } => Some("DPAE à annuler".to_string()),
            ARefaire::Dsn { mois, .. } => Some(format!("DSN de {} à corriger", engagement::libelle_mois(mois))),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------ contrat-recu

/// `operations_documents.contrat_recu` : un humain dit que le contrat signe est revenu (ou retire la note).
/// Une ligne de journal, pas un etat de la saisie : elle date un fait du monde. Rien ne part, aucun compteur ne
/// bouge.
fn contrat_recu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let recu = booleen(charge, "recu", true);
    // `numero` (facultatif) dit QUEL contrat du jour : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    contrat_recu_avec(ctx, mois, salarie, debut, numero.as_deref(), recu, "interface", None)
}

/// `contrat_recu(ctx, mois, salarie, debut, recu=..., qui=...)` : la meme note, `qui` donne (l'horloge d'une
/// compagnie fictive passe `horloge`).
pub(crate) fn contrat_recu_par(
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    qui: &str,
) -> R<Value> {
    contrat_recu_avec(ctx, mois, salarie, debut, numero, recu, qui, None)
}

/// `contrat_recu(..., courriel=...)` : quand la releve des signatures a trouve la reponse « Je signe », elle passe
/// `courriel` (code, adresse, date, identifiant du message, fichier archive, ...). La ligne est alors datee de la
/// SIGNATURE (`courriel["quand"]`), pas du releve, avec le message pour origine, et son detail porte `origine:
/// "courriel"` puis les champs de `courriel` dans l'ordre ou ils sont donnes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn contrat_recu_avec(
    ctx: &Contexte,
    mois: &str,
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    recu: bool,
    qui: &str,
    courriel: Option<&Map<String, Value>>,
) -> R<Value> {
    // Les paies telles que la DSN les a declarees, sans rien ecrire.
    let paies = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None).map_err(en_echec)?.1;
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
    let texte = |cle: &str| p.salarie.get(cle).filter(|v| vrai(v)).map(valeur_en_texte);
    let prenom = texte("prenom_affichage").or_else(|| texte("prenom")).unwrap_or_default();
    let nom = format!("{} {}", texte("nom").unwrap_or_default(), prenom);
    let mut detail = json!({
        "salarie": salarie, "debut": debut, "recu": recu,
        "numero": numero_note, "nom": nom,
    });
    let courriel = courriel.filter(|c| !c.is_empty());
    let texte_du_courriel = |cle: &str, defaut: &str| courriel.and_then(|c| c.get(cle)).map_or_else(|| defaut.to_string(), valeur_en_texte);
    let (reponse, quand, origine) = match courriel {
        Some(c) => {
            detail["origine"] = json!("courriel");
            for (cle, valeur) in c {
                detail[cle.as_str()] = valeur.clone();
            }
            (
                format!("contrat signe par courriel ({}, {})", texte_du_courriel("code", "?"), texte_du_courriel("adresse", "?")),
                c.get("quand").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default(),
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
        versions: None,
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
    let nom = logiciel.map(valeur_en_texte).unwrap_or_default();
    if logiciel.is_some() {
        let connus = documents::plan_comptable::logiciels_connus();
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
    let qui = charge.get("qui").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| "interface".to_string());
    declarations::casc_regularisation::marquer_regularise(ctx, annee, vire_le, montant, &qui, &aujourd_hui[..10])
}

// ------------------------------------------------------------------ exporter

/// `charge[cle] or None` : un texte, ou rien.
fn texte_ou_rien(charge: &Value, cle: &str) -> Option<String> {
    charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// `operations_exports.exporter` : le verbe unique, `exporter <etat>`. Lit la charge (etat, mois, annee,
/// toutes_les_annees, regroupement, format, coordonnees, apercu, logiciel, analytique), refuse un etat inconnu,
/// ne passe a l'etat que ce que sa forme demande, puis rend l'etat tel que `js.brut` le rend.
fn exporter(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(match exporter_objet(ctx, charge)? {
        Exporte::Casc(v) => v,
        Exporte::Livre(l) => l.json(),
        Exporte::Tableau(t) => documents::tableaux_export::brut_tableau(&t),
    })
}

/// Ce qu'un export rend, avant d'etre mis en JSON : la ligne de commande (`paie exporter`) le rend en texte a partir de l'objet, comme le
/// Python le faisait (le chemin ecrit en entier, les montants en `Decimal`).
pub(crate) enum Exporte {
    Casc(Value),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON. La charge peut porter `format_du_logiciel_retenu` : sans `format`, les ecritures comptables laissent
/// le logiciel retenu (`compta-logiciel`) choisir leur format (la ligne de commande), au lieu du CSV (la page, qui envoie son format).
pub(crate) fn exporter_objet(ctx: &Contexte, charge: &Value) -> R<Exporte> {
    use documents::etats_paie::Demande;
    let etat = texte_ou_rien(charge, "etat").unwrap_or_else(|| "livre-de-paie".to_string());
    let tables = documents::plan_comptable::tables();
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
        let demande = declarations::casc_regularisation::Demande { mois: d.mois.clone(), annee: d.annee.clone(), toutes_les_annees: d.toutes, boite_liee: crate::routes_accueil::boite_liee(ctx).unwrap_or(false), ..Default::default() };
        let mut r = declarations::casc_regularisation::regularisation(ctx, &demande)?;
        if d.ecrire {
            declarations::casc_regularisation::ecrire(ctx, &mut r)?;
        }
        return Ok(Exporte::Casc(declarations::casc_regularisation::vers_json(&r)));
    }
    if etat == "livre-de-paie" {
        return Ok(Exporte::Livre(documents::etats_paie::livre_de_paie(ctx, &d)?));
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
        "livre-de-paie-detaille" => documents::etats_paie::livre_detaille(ctx, &d),
        "recapitulatif-annuel" => documents::etats_paie::recapitulatif_annuel(ctx, &d),
        "masse-salariale" => documents::etats_paie::masse_salariale(ctx, &d),
        "etat-des-virements" => documents::etats_paie::etat_des_virements(ctx, &d),
        "prelevement-a-la-source" => documents::etats_paie::prelevement_a_la_source(ctx, &d),
        "cotisations-par-organisme" => declarations::etats_organismes::cotisations_par_organisme(ctx, &d),
        "versements-organismes" => declarations::etats_organismes::versements_organismes(ctx, &d, &crate::depart::dsn_du_mois),
        "bordereau-urssaf" => declarations::etats_organismes::bordereau_urssaf(ctx, &d, &crate::depart::dsn_du_mois),
        "fcap-svp" => declarations::etats_organismes::fcap_svp(ctx, &d),
        "afdas-2642" => declarations::etats_organismes::afdas_2642(ctx, &d),
        "paritarisme-branche" => declarations::etats_organismes::paritarisme_branche(ctx, &d),
        "reduction-generale" => documents::etats_paie::reduction_generale(ctx, &d),
        "assiettes" => documents::etats_paie::assiettes(ctx, &d),
        "heures-et-cachets" => documents::etats_paie::heures_et_cachets(ctx, &d),
        "cout-par-spectacle" => documents::etats_paie::cout_par_spectacle(ctx, &d),
        "contrats" => documents::etats_personnel::liste_des_contrats(ctx, &d),
        "registre-du-personnel" => documents::etats_personnel::registre_du_personnel(ctx, &d),
        "fichier-du-personnel" => documents::etats_personnel::fichier_du_personnel(ctx, &d),
        "journal-des-declarations" => documents::etats_personnel::journal_des_declarations(ctx, &d, &crate::journal::lire(&ctx.racine, None, None, false).map_err(|m| erreur("OSError", m))?),
        "ecritures-comptables" => documents::etats_compta::ecritures_comptables(ctx, &d),
        "grand-livre-paie" => documents::etats_compta::grand_livre(ctx, &d),
        "balance-paie" => documents::etats_compta::balance(ctx, &d),
        "provision-conges" => documents::etats_compta::provision_conges(ctx, &d),
        "taxe-sur-les-salaires" => documents::etats_compta::taxe_sur_les_salaires(ctx, &d),
        autre => Err(refus(format!("État inconnu : « {} ».", autre))),
    }?;
    Ok(Exporte::Tableau(tableau))
}
