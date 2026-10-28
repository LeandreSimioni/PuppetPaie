//! La releve des reponses « Je signe » (`moteur/operations_documents.py` : `relever_signatures`, `SignatureRecue`,
//! `ReleveSignatures` ; `moteur/simulation.py` : `relever_signatures`, `BoiteDesSignatures`). Lot I2a.
//!
//! ECRIT, NON PROUVE, NON BRANCHE. La demande de signature (`envois_signature::demander_signature`) a garde, au journal,
//! un CODE (`SIG-XXXXXX`) par courriel parti. Cette releve cherche chaque code attendu dans la boite et fait tout ce qui
//! suit : le tri strict, le journal (une ligne `contrat-recu` par contrat, datee de la SIGNATURE), la preuve archivee
//! (`Signature_<salarie>_<code>.eml`), le contrat contresigne (refait depuis la description gardee a l'envoi) et son
//! renvoi au salarie, avec sa ligne `contrat-contresigne`.
//!
//! UNE RECHERCHE PAR CODE ATTENDU. La boite cherche le code partout (objet, corps, pieces jointes) ; c'est nous qui trions
//! ensuite, et strictement :
//! - notre propre envoi est ecarte par son identifiant (le `Message-ID` note avec la demande, relu dans chaque message), et
//!   par sa phrase (« bouton « Je signe » ») -- pas par son expediteur : en test le salarie, c'est nous ;
//! - le code doit figurer dans la partie ECRITE de la reponse, avant la citation du message d'origine
//!   (`courriel_lecture::texte_propre`) ;
//! - elle doit venir de l'adresse a laquelle le contrat a ete envoye : une reponse venue d'ailleurs est signalee, pas
//!   retenue.
//!
//! NE LEVE PAS pour une boite absente : c'est un travail de fond, il dit ce qui l'empeche dans `incidents` et rend la
//! main. C'est UNE FONCTION, que l'ouverture (lot I2b) appellera avec son `qui` et son client.
//!
//! Dans une compagnie fictive, les salaries « signent » (`boite_des_signatures`) : chaque demande partie depuis `delai` secondes
//! recoit sa reponse -- celle que le bouton du courriel prepare --, et c'est la VRAIE releve qui la lit.
//! `relever_signatures_fictive` est `simulation.relever_signatures`, avec la garde de l'horloge (rien ne se joue
//! dans une compagnie qui n'est pas une compagnie fictive : le Python laissait ce garde a ses appelants).
//!
//! Reutilise : `etats::signatures_attendues` (le journal), `courriel_lecture` (adresse, identifiant, date, texte ecrit),
//! `routes_ecritures_g4::contrat_recu_avec` (la ligne `contrat-recu`), `contrat::contresigner`, `envoi::envoyer`,
//! `simulation_courriels::BoiteQuiRepond` (la boite de la compagnie fictive, aussi celle des fiches).
//!
//! Les phrases des incidents sont celles du Python ; la classe d'un refus peut differer.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::courriel_lecture as lecture;
use envois::envoi::{self, Message};
use envois::etats;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use crate::horloge::{HorlogeFictive, QUI_HORLOGE};
use envois::journal::{self, Note};
use socle::echec::Echec;
use socle::paris::maintenant;
use envois::simulation_courriels::BoiteQuiRepond;

/// Les routes de ce module.
pub const ROUTES: [&str; 1] = ["signatures-relever"];

/// Secondes entre une demande de signature et le « Je signe » du salarie fictif de la compagnie fictive
/// (`simulation.DELAI_SIGNATURE`).
pub const DELAI_FICTIVE: i64 = crate::routes_fiches_courriel::DELAI_FICTIVE;

const SUJET_CONTRESIGNE: &str = "Ton contrat signé - {spectacle}";
const SUJET_CONTRESIGNE_PLURIEL: &str = "Tes contrats signés - {mois} {annee}";
const CORPS_CONTRESIGNE: &str = "Bonjour {prenom},\n\nNous avons bien reçu ta signature le {quand}. Ci-joint {quoi}, signé des deux parties : c'est l'exemplaire à conserver.\n\nBien à toi,\n{signataire}\n{raison_sociale}\n";

// ------------------------------------------------------------------ ce que la releve rend

/// `SignatureRecue` : une signature trouvee, notee, contresignee (si elle a pu l'etre).
#[derive(Debug, Clone)]
pub struct SignatureRecue {
    pub mois: String,
    pub salarie: String,
    pub nom: String,
    pub code: String,
    pub adresse: String,
    /// ISO, la date du message.
    pub quand: String,
    /// `[{salarie, debut, fin, numero, spectacle, fichier, avenant, mois}]`, tels que le journal les donne.
    pub contrats: Vec<Value>,
    pub fichier_signe: String,
    pub preuve: String,
    pub renvoye: bool,
}

impl SignatureRecue {
    /// Ce que `commandes_ouverture._signatures_json` en lit.
    pub fn en_json(&self) -> Value {
        json!({
            "mois": self.mois, "salarie": self.salarie, "nom": self.nom, "code": self.code, "adresse": self.adresse,
            "quand": self.quand, "contrats": self.contrats, "fichier_signe": self.fichier_signe, "preuve": self.preuve,
            "renvoye": self.renvoye,
        })
    }
}

/// `ReleveSignatures` : combien de signatures attendaient, celles qui sont arrivees, ce qui a empeche le reste.
#[derive(Debug, Clone)]
pub struct ReleveSignatures {
    pub quand: String,
    pub attendues: usize,
    pub recues: Vec<SignatureRecue>,
    pub incidents: Vec<String>,
}

impl ReleveSignatures {
    fn nouvelle() -> ReleveSignatures {
        ReleveSignatures { quand: maintenant(), attendues: 0, recues: Vec::new(), incidents: Vec::new() }
    }

    /// `ReleveSignatures.resume` : la phrase de la veille, vide quand rien n'etait attendu ni arrive.
    pub fn resume(&self) -> String {
        if self.attendues == 0 && self.recues.is_empty() {
            return String::new();
        }
        if self.recues.is_empty() {
            return format!("{} signature(s) attendue(s), aucune arrivée", self.attendues);
        }
        let noms: Vec<&str> = self.recues.iter().map(|r| r.nom.as_str()).collect();
        format!("{} signature(s) reçue(s) : {}", self.recues.len(), noms.join(", "))
    }

    /// `commandes_ouverture._signatures_json`, avec le resume en plus (celui que l'ouverture met dans son bilan).
    pub fn en_json(&self) -> Value {
        json!({
            "quand": self.quand,
            "attendues": self.attendues,
            "recues": self.recues.iter().map(SignatureRecue::en_json).collect::<Vec<_>>(),
            "incidents": self.incidents,
            "resume": self.resume(),
        })
    }
}

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

// ------------------------------------------------------------------ la releve

fn texte_de(d: &Value, cle: &str) -> String {
    d.get(cle).map(valeur_en_texte).unwrap_or_default()
}

/// `a["envoyes"]` : les `Message-ID` de NOS envois pour cette demande.
fn identifiants_de(a: &Value) -> Vec<String> {
    a.get("envoyes").and_then(Value::as_array).map(|l| l.iter().map(valeur_en_texte).collect()).unwrap_or_default()
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

/// `operations_documents.relever_signatures` : cherche dans la boite les reponses « Je signe » encore attendues, et fait
/// tout ce qui suit.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- celle de la compagnie fictive (les salaries
/// fictifs repondent) ou la vraie ; sans boite, un incident et rien d'autre. `mois` : seulement les demandes de ce mois.
/// `qui` : `veille`, `interface`, `ligne de commande`, `horloge`. Une erreur de transport ou de lecture se dit dans
/// `incidents` ; seul un refus que le Python laissait lever (la paie du contrat n'est plus dans la saisie, un fichier qui
/// ne s'ecrit pas) remonte en `Err`.
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
        None if crate::routes_accueil::simulation_active(&ctx.racine) => {
            // LA COMPAGNIE FICTIVE : les reponses « Je signe » sont jouees, et tout ce qui suit est le vrai circuit.
            ouverte.insert(Box::new(boite_des_signatures(ctx, DELAI_FICTIVE)?)).as_mut()
        }
        None => {
            // UNE FICTIVE AJOUTEE (ticket #236) : la releve renverrait le contrat contresigne au salarie -- un envoi. Rien
            // ne se releve, et le refus se dit (`bridage::refus_d_envoi`).
            if let Some(m) = crate::bridage::refus_d_envoi(&ctx.racine) {
                r.incidents.push(m.to_string());
                return Ok(r);
            }
            if !crate::boite::disponible(&ctx.racine) {
                r.incidents.push(
                    "Aucune boite courriel : les signatures ne peuvent pas etre relevees. Poser le mot de passe d'application de la boite (Configuration > Mots de passe)."
                        .to_string(),
                );
                return Ok(r);
            }
            let vrai_client = crate::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };

    let (signataire, raison) = envois::envois_signature::signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = texte_de(a, "code");
        let a_mois = texte_de(a, "mois");
        let a_nom = texte_de(a, "nom");
        let a_adresse = texte_de(a, "adresse");
        let a_salarie = texte_de(a, "salarie");
        let envoyes = identifiants_de(a);
        let contrats: Vec<Value> = a.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default();
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e));
                // Une erreur de transport ne se corrige pas en essayant le code suivant, et repeter le meme message dix
                // fois n'aide personne.
                break;
            }
        };

        // Le tri strict : (identifiant, message brut, adresse de l'auteur, date du message).
        let mut retenu: Option<(String, Vec<u8>, String, String)> = None;
        for ident in identifiants {
            if envoyes.contains(&ident) {
                continue; // notre propre demande
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e));
                    continue;
                }
            };
            // Le Python lit tout message (`email` ne refuse jamais) : un bloc d'en-tetes rompu se recoupe comme lui.
            let lu = lecture::lisible_comme_python(&brut);
            let Ok(m) = mailparse::parse_mail(&lu) else { continue };
            if envoyes.contains(&lecture::identifiant_de(&m)) {
                continue; // notre propre demande, par son en-tete
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.as_str()) {
                continue; // le code n'est que dans la citation
            }
            if texte.contains("bouton « Je signe »") {
                continue; // notre demande elle-meme (envoyee avant que son identifiant soit note)
            }
            let de = lecture::adresse_de(&m);
            if de == expediteur && de != a_adresse {
                continue; // un autre de nos envois (le contresigne)
            }
            if de != a_adresse {
                r.incidents.push(format!("{} : une reponse portant {} vient de {} et non de {} ; elle n'est pas retenue.", a_nom, code, de, a_adresse));
                continue;
            }
            let quand = lecture::date_de(&m, &maintenant());
            retenu = Some((ident, brut.clone(), de, quand));
            break;
        }
        let Some((ident, brut, de, quand)) = retenu else { continue };

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
        // pas deux fois.
        let acte = a.get("acte").cloned().unwrap_or_else(|| json!("contrat"));
        let est_avenant = acte == json!("avenant");
        // DEFAUT DU PYTHON, CORRIGE (decision de l'utilisateur, 07/10/2026) : chez lui, un contrat signe que `contrat_recu` refuse
        // (la paie n'est plus dans la saisie du mois...) faisait lever TOUTE la releve, apres la preuve et le PDF mais avant la
        // ligne du journal : les signatures suivantes n'etaient pas relevees, et la releve suivante retombait sur le meme refus,
        // pour toujours. Ici le refus devient un incident NOMME (le salarie, le contrat, la raison) et la releve continue.
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
            let numero = envois::etats::numero_de_cle(c.get("numero").unwrap_or(&Value::Null));
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
        let nb = contrats.len();
        let mut spectacles: Vec<String> = Vec::new();
        for c in &contrats {
            let s = texte_de(c, "spectacle");
            if !spectacles.contains(&s) {
                spectacles.push(s);
            }
        }
        let numeros: Vec<String> = contrats.iter().map(|c| valeur_en_texte(&ou_valeur(c, a, "avenant"))).collect();
        let (sujet, quoi) = if est_avenant {
            (
                format!("{} - {}", if nb == 1 { "Ton avenant signé" } else { "Tes avenants signés" }, spectacles[0]),
                if nb == 1 { format!("l'avenant n° {} à ton contrat", numeros[0]) } else { format!("tes {} avenants (un document chacun)", nb) },
            )
        } else {
            (
                if nb == 1 || spectacles.len() == 1 {
                    envoi::formater(SUJET_CONTRESIGNE, &[("spectacle", spectacles[0].clone())])
                } else {
                    envoi::formater(SUJET_CONTRESIGNE_PLURIEL, &[("mois", nom_mois.to_string()), ("annee", an.to_string())])
                },
                if nb == 1 { "ton contrat d'engagement".to_string() } else { format!("tes {} contrats d'engagement (un par page)", nb) },
            )
        };
        let message = Message {
            cle: a_salarie.clone(),
            nom: a_nom.clone(),
            destinataire: de.clone(),
            sujet,
            corps: envoi::formater(
                CORPS_CONTRESIGNE,
                &[
                    ("prenom", texte_de(a, "prenom")),
                    ("quand", quand_lisible.clone()),
                    ("quoi", quoi),
                    ("signataire", signataire.clone()),
                    ("raison_sociale", raison.clone()),
                ],
            ),
            pieces: signes.iter().map(|(_, p)| p.clone()).collect(),
            html: String::new(),
        };
        let envoi_fait = envoi::envoyer(&[message], &expediteur, client, None, None);
        let dernier = r.recues.len() - 1;
        let suite = envoi_fait.and_then(|_| {
            r.recues[dernier].renvoye = true;
            let detail = json!({"salarie": a_salarie, "nom": a_nom, "code": code, "contrats": contrats, "fichiers": noms_signes});
            let premier = signes[0].1.to_string_lossy().into_owned();
            let note = Note {
                quoi: "contrat-contresigne",
                mois: Some(&a_mois),
                fichier: Some(&premier),
                mode: journal::REEL,
                qui,
                accepte: true,
                destinataire: &de,
                flux: "",
                reponse: "contrat contresigne renvoye au salarie",
                trace: None,
                detail: Some(&detail),
                versions: None,
                quand: "",
                origine: "",
            };
            journal::noter(&ctx.racine, &note, &maintenant()).map(|_| ())
        });
        if let Err(e) = suite {
            r.incidents.push(format!("{} : le contrat contresigne n'a pas pu etre renvoye ({}). Il est dans {}.", a_nom, e, fichier_signe));
        }
    }
    client.fermer();
    Ok(r)
}
