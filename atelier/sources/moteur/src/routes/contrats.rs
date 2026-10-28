//! Les routes des contrats : ce que le contrat a signer et l'avenant montrent, les envoyer a la signature ou les
//! imprimer, les relancer, les noter revenus signes, et les pieces parties d'un engagement.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::depot_acte::{verifier_preuve, Preuve};
use crate::routes::documents::{charge_texte, client_courriel, cle, get_requis, jj_mm_aaaa, liste_de, nul, preuve_de, refus, requis, un_nom_commence_et_finit, vrai_ou, Ouvrir};
use crate::routes::engagement::{booleen, en_echec, maintenant, texte_requis};
use crate::routes::etapes::{dans_l_ensemble, premier, Moteur, SaisieMois};
use droit::contexte::Contexte;
use envois::envois_signature::{Choisis, UnAvenant};
use envois::etats::{self, est, get, getd, items, objet_vide, ou, LISTE_VIDE, DPAE, SIGNATURE};
use envois::journal::{self, Note};
use socle::date::jour_iso;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{repr, vrai};
use socle::valeur::{nom_type, valeur_en_texte, valeurs_egales};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "signature-apercu", "avenant-apercu", "pieces-parties",
    "demander-signature", "contrats-imprimer", "avenant-envoyer", "avenants-envoyer", "contrat-recu",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &["demander-signature", "contrats-imprimer", "avenant-envoyer", "avenants-envoyer", "contrat-recu"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "signature-apercu" => signature_apercu(ctx, charge),
        "avenant-apercu" => avenant_apercu(ctx, charge),
        "pieces-parties" => pieces_parties(ctx, charge),
        "demander-signature" => demander_signature(ctx, charge, &mut || client_courriel(ctx)),
        "contrats-imprimer" => contrats_imprimer_par(ctx, charge, "interface"),
        "avenant-envoyer" => avenant_envoyer(ctx, charge, &mut || client_courriel(ctx)),
        "avenants-envoyer" => avenants_envoyer(ctx, charge, &mut || client_courriel(ctx)),
        "contrat-recu" => contrat_recu(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// Un humain dit que le contrat signe est revenu (ou retire la note).
/// Une ligne de journal, pas un etat de la saisie : elle date un fait du monde. Rien ne part, aucun compteur ne
/// bouge.
fn contrat_recu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie, debut) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?, texte_requis(charge, "debut")?);
    let recu = booleen(charge, "recu", true);
    // `numero` (facultatif) dit QUEL contrat du jour : deux contrats du meme salarie peuvent commencer le meme jour.
    let numero = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    // `avenant` (facultatif) : le numero de l'avenant revenu signe, quand c'est lui et non le contrat.
    let avenant = match charge.get("avenant").filter(|v| vrai(v)) {
        None => Value::Null,
        Some(v) => json!(valeur_en_texte(v).trim().parse::<i64>().map_err(|_| refus(format!("Numéro d'avenant illisible : {}.", valeur_en_texte(v))))?),
    };
    contrat_recu_avec(ctx, mois, salarie, debut, numero.as_deref(), recu, &avenant, "interface", None)
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
    contrat_recu_avec(ctx, mois, salarie, debut, numero, recu, &Value::Null, qui, None)
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
    avenant: &Value,
    qui: &str,
    courriel: Option<&Map<String, Value>>,
) -> R<Value> {
    // Les paies telles que la DSN les a declarees, sans rien ecrire.
    let paies = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None).map_err(en_echec)?.1;
    // LE CONTRAT, pas le salarie : `numero` est celui sous lequel le journal le connait (la releve), ou celui que l'ecran affiche.
    let numero = numero.filter(|n| !n.is_empty());
    let lignes = envois::etats::charger(&ctx.racine)?;
    let tous = crate::routes::etapes::contrats_des_paies(&paies);
    let cles = envois::etats::cles_des_paies(&lignes, &tous)?;
    let du_jour: Vec<usize> = (0..paies.len()).filter(|&i| tous[i].0 == salarie && tous[i].1 == debut).collect();
    let trouvee = match numero {
        None => du_jour.first().copied(),
        Some(n) => du_jour.iter().copied().find(|&i| cles[i][0].2 == n).or_else(|| du_jour.iter().copied().find(|&i| tous[i].2 == n)),
    };
    let Some(rang) = trouvee else {
        return Err(refus(format!(
            "Aucun engagement de {} commençant le {}{} dans la saisie enregistrée de {}.",
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
    if vrai(avenant) {
        detail["avenant"] = avenant.clone();
    }
    let ce = if vrai(avenant) { format!("avenant n° {}", valeur_en_texte(avenant)) } else { "contrat".to_string() };
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
            if recu { format!("{} signe recu, range a la main", ce) } else { format!("mention retiree : le {} signe n'est pas la", ce) },
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
    // Ce que l'ecran dit de la note.
    let phrase = format!(
        "{} de {} {}.",
        socle::texte::capitaliser(&ce),
        format!("{} {}", prenom, texte("nom_affichage").or_else(|| texte("nom")).unwrap_or_default()).trim(),
        if recu { "noté reçu signé" } else { "de nouveau en attente" }
    );
    Ok(json!({
        "salarie": salarie, "debut": debut,
        "phrase": phrase,
        "recu": if recu { ligne["quand"].clone() } else { Value::Null },
        // De quoi revenir sur la note : la meme route, le mot contraire.
        "annuler": {"route": "contrat-recu", "charge": {"mois": mois, "salarie": salarie, "debut": debut, "numero": numero_note, "avenant": avenant, "recu": !recu}},
        "ligne": ligne,
    }))
}

/// Les contrats que la charge choisit : `salaries` (tous leurs contrats) et `contrats` ([{"salarie", "debut", "numero"}]).
fn choisis_de(charge: &Value) -> R<Choisis> {
    let salaries = liste_de(charge, "salaries")?.map(|l| l.iter().map(valeur_en_texte).collect());
    let contrats = match liste_de(charge, "contrats")? {
        None => None,
        Some(l) => {
            let mut v = Vec::new();
            for c in l {
                let numero = c.get("numero").filter(|n| vrai(n)).map(valeur_en_texte);
                v.push((valeur_en_texte(requis(c, "salarie")?), valeur_en_texte(requis(c, "debut")?), numero));
            }
            Some(v)
        }
    };
    Ok(Choisis { salaries, contrats })
}

/// La route `contrats-imprimer` (`envois_signature::contrats_imprimer`), `qui` donne (l'horloge d'une compagnie fictive
/// passe le sien).
/// `avenants` : les avenants des contrats signes qui ont change ([{"salarie", "debut", "numero", "mois", "effet"}]),
/// imprimes avec eux ; sans `contrats` ni `salaries`, seuls les avenants s'impriment.
pub(crate) fn contrats_imprimer_par(ctx: &Contexte, charge: &Value, qui: &str) -> R<Value> {
    let voulus = liste_de(charge, "avenants")?.cloned().unwrap_or_default();
    let avenants = avenants_voulus(ctx, &voulus, false)?;
    if avenants.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) {
        return Err(refus("Une relance ne s'imprime pas : l'avenant imprimé se remet en main propre. Rien n'est parti."));
    }
    let contrats_choisis = ["contrats", "salaries"].iter().any(|k| charge.get(*k).is_some_and(vrai));
    let choisis = choisis_de(charge)?;
    let choisis = (contrats_choisis || avenants.is_empty()).then_some(&choisis);
    envois::envois_signature::contrats_imprimer(ctx, &charge_texte(charge, "mois")?, choisis, &avenants, qui)
}

/// La route `demander-signature` : la boite, la preuve, puis `envois_signature::demander_signature`. `relance` : l'etat
/// des contrats du mois (`signature-apercu`) dit lesquels peuvent se relancer.
pub(crate) fn demander_signature(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let mois = charge_texte(charge, "mois")?;
    let relance = charge.get("relance").is_some_and(vrai);
    let remplace = charge.get("remplace").is_some_and(vrai);
    let choisis = choisis_de(charge)?;
    let a_qui = match &choisis.salaries {
        Some(l) if !l.is_empty() => format!("{} salarie(s) : {}", l.len(), l.join(", ")),
        _ => "les salaries du mois qui ont un contrat a signer".to_string(),
    };
    let etats_des_contrats = if relance {
        let apercu = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": mois}))?;
        apercu.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default()
    } else {
        Vec::new()
    };
    let demande = envois::envois_signature::DemandeDeSignature { mois: &mois, relance, remplace, choisis, etats_des_contrats };
    // Un refus se dit d'abord : la boite et la preuve ne se demandent que pour ce qui part.
    envois::envois_signature::verifier_demande(ctx, &demande)?;
    let mut client = ouvrir()?;
    let quoi = if relance { "relance de demande de signature de contrat" } else if remplace { "contrat refait, a signer (il annule et remplace le precedent)" } else { "demande de signature de contrat" };
    verifier_preuve(ctx, Some(&preuve_de(charge)), &mois, quoi, &a_qui)?;
    envois::envois_signature::demander_signature(ctx, &demande, client.as_mut())
}

/// La route `avenant-envoyer` : un seul avenant. La reponse : `partis`, `fichier`, `numero`, `ligne`.
fn avenant_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulu = json!({
        "salarie": requis(charge, "salarie")?, "debut": requis(charge, "debut")?, "effet": charge.get("effet").cloned().unwrap_or(Value::Null),
        "numero": charge.get("numero").cloned().unwrap_or(Value::Null), "mois": charge.get("mois").cloned().unwrap_or(Value::Null),
    });
    let r = envoyer_avenants(ctx, &[voulu], &preuve_de(charge), ouvrir)?;
    let a = r.avenants.first().ok_or_else(|| erreur("ValueError", "aucun avenant n'est parti"))?;
    Ok(json!({"partis": r.partis, "fichier": a["fichier"], "numero": a["numero"], "ligne": r.lignes.first().cloned().unwrap_or(Value::Null)}))
}

/// La route `avenants-envoyer` : les avenants coches, UN COURRIEL PAR SALARIE. La reponse : `partis`, `avenants`.
fn avenants_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let voulus: Vec<Value> = match requis(charge, "avenants")? {
        Value::Array(a) => a.clone(),
        Value::Null => Vec::new(),
        autre => return Err(erreur("TypeError", format!("`avenants` : une liste est attendue, pas {}", socle::valeur::nom_type(autre)))),
    };
    let r = envoyer_avenants(ctx, &voulus, &preuve_de(charge), ouvrir)?;
    Ok(json!({"partis": r.partis, "avenants": r.avenants}))
}

/// `jj_mm_aaaa(x)` : `ValueError` si `x` ne se lit pas comme une date.
fn jj(iso: &str) -> R<String> {
    socle::date::Jour::lire(iso).map(|j| j.jj_mm_aaaa()).map_err(|m| erreur("ValueError", m))
}

/// LES AVENANTS VOULUS, verifies avant que rien ne parte : chacun a son apercu (`avenant-apercu`), qui dit ce qui change
/// (un avenant sans objet ou une date d'effet illisible refuse le tout) ; une relance (`relance`) reprend l'avenant parti
/// et pas revenu, tel quel. `par_courriel` : chacun doit avoir un courriel dans sa fiche. Regroupes par salarie.
fn avenants_voulus(ctx: &Contexte, voulus: &[Value], par_courriel: bool) -> R<Vec<(String, Vec<UnAvenant>)>> {
    let mut par_salarie: Vec<(String, Vec<UnAvenant>)> = Vec::new();
    for v in voulus {
        let salarie_v = requis(v, "salarie")?;
        let debut_v = requis(v, "debut")?;
        let un = if v.get("relance").is_some_and(vrai) {
            avenant_a_relancer(ctx, v)?
        } else {
            // `numero` (celui que `signature-apercu` rend) et `mois` (celui de l'ecran) disent QUEL contrat du jour : facultatifs.
            let apercu = crate::routes::contrats::avenant_apercu(
                ctx,
                &json!({"salarie": salarie_v, "debut": debut_v, "numero": v.get("numero").cloned().unwrap_or(Value::Null), "mois": v.get("mois").cloned().unwrap_or(Value::Null)}),
            )?;
            let modifications = apercu["modifications"].as_array().map_or(0, Vec::len);
            let nom = apercu.get("nom").map(valeur_en_texte).unwrap_or_default();
            if modifications == 0 {
                return Err(refus(format!(
                    "{}, contrat du {} : le contrat parti dit déjà ce que dit l'engagement, pas d'avenant à faire. Rien n'est parti.",
                    nom,
                    jj(&valeur_en_texte(debut_v))?
                )));
            }
            let effet = vrai_ou(v, "effet").unwrap_or_else(|| apercu.get("effet").map(valeur_en_texte).unwrap_or_default());
            if jour_iso(&effet).is_none() {
                return Err(refus(format!("Date d'effet illisible : {}.", repr(&effet))));
            }
            UnAvenant { apercu, effet }
        };
        if par_courriel && un.texte("destinataire").trim().is_empty() {
            return Err(refus(format!("{} n'a pas de courriel dans sa fiche : l'avenant ne peut pas partir.", valeur_en_texte(salarie_v))));
        }
        let cle = valeur_en_texte(salarie_v);
        match par_salarie.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, l)) => l.push(un),
            None => par_salarie.push((cle, vec![un])),
        }
    }
    Ok(par_salarie)
}

/// L'AVENANT A RELANCER : celui que `signature-apercu` montre en attente pour ce contrat, parti par courriel. Il repart
/// tel qu'il est parti (son PDF, ses modifications). Un contrat dont aucun avenant n'attend, ou dont l'avenant est parti
/// imprime, se refuse : rien n'est ecrit.
fn avenant_a_relancer(ctx: &Contexte, v: &Value) -> R<UnAvenant> {
    let (salarie, debut) = (valeur_en_texte(requis(v, "salarie")?), valeur_en_texte(requis(v, "debut")?));
    let numero = vrai_ou(v, "numero").unwrap_or_default();
    let apercu = crate::routes::contrats::signature_apercu(ctx, &json!({"mois": requis(v, "mois")?}))?;
    let c = apercu["contrats"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["salarie"].as_str() == Some(&salarie) && c["debut"].as_str() == Some(&debut) && (numero.is_empty() || valeur_en_texte(&c["numero"]) == numero))
        .ok_or_else(|| refus(format!("Aucun contrat de {} commençant le {} dans {}. Rien n'est parti.", salarie, debut, valeur_en_texte(&v["mois"]))))?;
    let a = &c["avenant"];
    if !vrai(&c["avenant_attendu"]) || !vrai(&c["en_attente"]) {
        return Err(refus(format!("Le contrat de {} du {} n'a aucun avenant en attente de signature : il n'y a rien à relancer. Rien n'est parti.", salarie, jj(&debut)?)));
    }
    if c["par"].as_str() != Some("courriel") {
        return Err(refus(format!("L'avenant n° {} de {} est parti imprimé, à remettre en main propre : il ne se relance pas. Rien n'est parti.", valeur_en_texte(&a["numero"]), salarie)));
    }
    let apercu = json!({
        "salarie": salarie, "debut": debut, "fin": c["fin"], "spectacle": c["spectacle"], "mois": a["mois"],
        "contrat": a["contrat"], "numero": a["numero"], "fichier": a["fichier"], "modifications": a["modifications"],
        "destinataire": c["destinataire"], "relance": true,
    });
    Ok(UnAvenant { apercu, effet: valeur_en_texte(&a["effet"]) })
}

/// Les avenants voulus, verifies, puis la boite, la preuve, et `envois_signature::envoyer_avenants`.
fn envoyer_avenants(ctx: &Contexte, voulus: &[Value], preuve: &Preuve, ouvrir: Ouvrir) -> R<envois::envois_signature::Avenants> {
    if voulus.is_empty() {
        return Err(refus("Aucun avenant coché. Rien n'est parti."));
    }
    // La porte du branchement, avant de rien lire.
    crate::bridage::branchement(&ctx.racine).ouvrir()?;
    let par_salarie = avenants_voulus(ctx, voulus, true)?;
    let mut client = ouvrir()?;
    let mut mois_vus: Vec<String> = par_salarie.iter().flat_map(|(_, aa)| aa.iter().map(|a| a.texte("mois"))).collect();
    mois_vus.sort();
    let a_qui = format!("{} salarie(s) : {}", par_salarie.len(), par_salarie.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", "));
    let quoi = if par_salarie.iter().any(|(_, aa)| aa.iter().any(UnAvenant::relance)) { "relance d'avenant(s) a signer" } else { "envoi d'avenant(s) a signer" };
    verifier_preuve(ctx, Some(preuve), mois_vus.first().map_or("", String::as_str), quoi, &a_qui)?;
    envois::envois_signature::envoyer_avenants(ctx, &par_salarie, client.as_mut())
}

/// `apercu_signature` : les contrats du mois et l'etat de leur demande de signature.
pub(crate) fn signature_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = valeur_en_texte(mois_v);
    let (data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None)?;
    let mut m = Moteur::nouveau(ctx);
    let g = m.globales()?;
    let lignes = m.lignes()?;
    // `numeros_saisis` : `{}` quand le mois ne se lit pas ; ici il s'est lu.
    let rangs: Vec<(String, String)> = paies
        .iter()
        .map(|p| Ok((valeur_en_texte(get(&p.salarie, "_cle")?), p.engagement.debut.iso())))
        .collect::<R<_>>()?;
    // Par RANG dans la saisie : deux contrats du meme salarie et du meme jour ont chacun leur numero.
    let uniques: Vec<(String, String)> = rangs.iter().enumerate().map(|(i, (c, d))| (c.clone(), format!("{}#{}", d, i))).collect();
    let saisie = SaisieMois { data, paies: Vec::new() };
    let avenants = crate::routes::etapes::avenants_a_faire(ctx, &mois)?;
    let saisis = Moteur::numeros_saisis(&saisie, &uniques);
    let cles_paies = etats::cles_des_paies(&lignes, &crate::routes::etapes::contrats_des_paies(&paies))?;

    // Quand chaque CONTRAT a ete demande (ce mois-ci).
    let mut demandes: HashMap<etats::Cle3, Value> = HashMap::new();
    for l in etats::filtrer(&lignes, Some(SIGNATURE), Some(&mois), false)? {
        if !etats::accepte(l)? {
            continue;
        }
        let contrats = getd(getd(l, "detail", objet_vide())?, "contrats", &LISTE_VIDE)?;
        for c in items(contrats)? {
            if let [Value::String(a), Value::String(b), Value::String(n)] = etats::cle_contrat(&c, etats::Maille::Contrat)?.as_slice() {
                demandes.insert((a.clone(), b.clone(), n.clone()), get(l, "quand")?.clone());
            }
        }
    }
    let aujourdhui = ctx.aujourdhui.iso();
    let boite_liee = crate::routes::ouverture::boite_liee(ctx)?;
    let mut contrats = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        // UN PERMANENT N'A PAS DE CONTRAT D'USAGE A SIGNER ICI.
        if !vrai(&p.spectacle) {
            continue;
        }
        let (cle_s, debut) = rangs[i].clone();
        let cles = &cles_paies[i];
        let etat = crate::routes::etapes::etat_signature(
            premier(&g.contrats, cles).unwrap_or(nul()),
            premier(&g.par, cles).unwrap_or(nul()),
            premier(&g.recus, cles).unwrap_or(nul()),
            premier(&g.recus_par, cles).unwrap_or(nul()),
            dans_l_ensemble(&g.renvoyes, cles),
            premier(&g.numeros_partis, cles).unwrap_or(nul()),
            saisis.get(&uniques[i]).unwrap_or(nul()),
            premier(&g.avenants, cles).unwrap_or(nul()),
            avenants.contains(&(cle_s.clone(), p.engagement.numero.clone().unwrap_or_default())),
        );
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => valeur_en_texte(v),
            None => valeur_en_texte(get_requis(sal, "prenom")?),
        };
        let numero = p.engagement.numero.clone().map_or(Value::Null, Value::String);
        let fin = p.engagement.fin.map_or("None".to_string(), |f| f.iso());
        let mut l = Map::new();
        l.insert("salarie".into(), json!(cle_s));
        l.insert("nom".into(), json!(format!("{} {}", valeur_en_texte(get_requis(sal, "nom")?), prenom)));
        l.insert("destinataire".into(), sal.get("email").cloned().unwrap_or_else(|| json!("")));
        l.insert("spectacle".into(), json!(p.titre()?));
        l.insert("debut".into(), json!(debut));
        l.insert("fin".into(), json!(fin));
        l.insert("numero".into(), numero);
        l.insert("demande_le".into(), premier(&demandes, cles).cloned().unwrap_or(Value::Null));
        // CE QUE LA BOITE EN FAIT : un contrat a envoyer (jamais parti, ou caduc) se coche, et part en avenant s'il est
        // deja revenu signe (`en_avenant`) ; un contrat ou un avenant parti et pas revenu attend : il se note reçu signe
        // (`recu`), et seul celui parti par courriel se relance (`relancer`), quand la boite est posee. Les deux gestes
        // sont des routes et leurs charges : la page n'en compose aucun.
        let parti = l["demande_le"].clone();
        let (recu, caduc) = (vrai(&etat["recu"]), vrai(&etat["caduc"]));
        let avenant_en_attente = !caduc && vrai(&etat["avenant_attendu"]);
        let en_attente = !caduc && ((vrai(&parti) && !recu) || avenant_en_attente);
        let a_envoyer = caduc || (!recu && !vrai(&parti));
        let par = etat["par"].clone();
        let (ce, depuis) = if avenant_en_attente {
            (format!("avenant n° {} ", valeur_en_texte(&etat["avenant"]["numero"])), etat["avenant"]["quand"].clone())
        } else {
            (String::new(), parti.clone())
        };
        let jour = valeur_en_texte(&depuis);
        let jj_mm = if jour.len() >= 10 { format!("{}/{}", &jour[8..10], &jour[5..7]) } else { jour };
        let attente = format!("{}{} le {}", ce, if est(&par, "imprime") { "imprimé" } else { "envoyé" }, jj_mm);
        // Ce que la demande du mot de passe dit avant une relance.
        let pourquoi = format!("relancer la signature : {}, {}", valeur_en_texte(&l["nom"]), attente);
        let numero_charge = p.engagement.numero.clone().map_or(json!(""), Value::String);
        let (recu_g, relancer) = if en_attente {
            let avenant = if avenant_en_attente { etat["avenant"]["numero"].clone() } else { Value::Null };
            let recu_g = json!({"route": "contrat-recu", "charge": {"mois": mois, "salarie": cle_s, "debut": debut, "numero": numero_charge, "avenant": avenant, "recu": true}});
            let relancer = if !(est(&par, "courriel") && boite_liee) {
                Value::Null
            } else if avenant_en_attente {
                json!({"route": "avenants-envoyer", "pourquoi": pourquoi, "charge": {"avenants": [{"salarie": cle_s, "debut": debut, "numero": numero_charge, "mois": mois, "relance": true}]}})
            } else {
                json!({"route": "demander-signature", "pourquoi": pourquoi, "charge": {"mois": mois, "relance": true, "contrats": [{"salarie": cle_s, "debut": debut, "numero": numero_charge}]}})
            };
            (recu_g, relancer)
        } else {
            (Value::Null, Value::Null)
        };
        l.insert("par".into(), par.clone());
        l.insert("a_envoyer".into(), json!(a_envoyer));
        l.insert("en_avenant".into(), json!(a_envoyer && caduc && recu));
        l.insert("en_attente".into(), json!(en_attente));
        l.insert(
            "attente".into(),
            if en_attente { json!(attente) } else { Value::Null },
        );
        l.insert("recu".into(), recu_g);
        l.insert("relancer".into(), relancer);
        l.insert("recu_le".into(), etat["recu"].clone());
        l.insert("recu_par".into(), etat["recu_par"].clone());
        l.insert("caduc".into(), etat["caduc"].clone());
        l.insert("pourquoi".into(), etat["pourquoi"].clone());
        l.insert("avenant".into(), etat["avenant"].clone());
        l.insert("avenant_attendu".into(), etat["avenant_attendu"].clone());
        l.insert("commence".into(), json!(debut <= aujourdhui));
        contrats.push(Value::Object(l));
    }
    let dossier = ctx.racine.join("sorties").join(&mois);
    let envoi = dossier.join("envoi");
    let produit = dossier.exists()
        && if envoi.exists() {
            un_nom_commence_et_finit(&dossier, "Contrats_", ".pdf") || un_nom_commence_et_finit(&envoi, "Contrat_", ".pdf")
        } else {
            un_nom_commence_et_finit(&dossier, "Contrats_", ".pdf")
        };
    let salaries = salaries_d_identite(ctx);
    let attendues = etats::signatures_attendues(&lignes, Some(&mois), &salaries)?;
    let n_attendues = attendues.as_array().map_or(0, Vec::len);
    Ok(json!({
        "mois": mois_v,
        "contrats": contrats,
        "boite_liee": boite_liee,
        "attendues": n_attendues,
        "dossier": dossier.to_string_lossy(),
        "produit": produit,
    }))
}

/// Seuls les quatre champs d'identite que la demande affiche passent a `signatures_attendues` (`journal.signatures_attendues`).
pub(crate) fn salaries_d_identite(ctx: &Contexte) -> Value {
    let mut salaries = Map::new();
    for (k, v) in &ctx.salaries {
        if let Value::Object(f) = v {
            let garde: Map<String, Value> = ["prenom_affichage", "prenom", "nom_affichage", "nom"]
                .iter()
                .filter_map(|c| f.get(*c).map(|x| (c.to_string(), x.clone())))
                .collect();
            salaries.insert(k.clone(), Value::Object(garde));
        }
    }
    Value::Object(salaries)
}

/// Les numeros propres des paies de ce salarie qui commencent ce jour-la (celles d'un spectacle), dans l'ordre de la saisie.
pub(crate) fn numeros_du_jour(paies: &[engagement::calcul::Paie], salarie: &str, debut: &str) -> R<Vec<String>> {
    let mut v = Vec::new();
    for x in paies {
        if est(get_requis(&x.salarie, "_cle")?, salarie) && x.engagement.debut.iso() == debut && vrai(&x.spectacle) {
            v.push(x.engagement.numero.clone().unwrap_or_default());
        }
    }
    Ok(v)
}

/// Le numero SOUS LEQUEL LE JOURNAL parle du contrat que la page vise (Retours Windows, B, suite : deux contrats du meme salarie
/// qui commencent le meme jour sont deux contrats). `numero` : celui que l'ecran affiche (`signature-apercu`), `mois` : le mois de
/// l'ecran, qui donne les paies jumelles. `None` : ce contrat n'est jamais parti. Sans `numero`, le contrat est celui qui est
/// parti ; s'il y en a plusieurs, l'appelant doit preciser lequel (refus).
pub(crate) fn numero_du_contrat(
    ctx: &Contexte,
    lignes: &[Value],
    salarie: &str,
    debut: &str,
    numero: Option<&str>,
    mois: Option<&str>,
) -> R<Option<String>> {
    let parti = etats::numeros_demandes(lignes, salarie, debut)?;
    let Some(n) = numero else {
        if parti.len() > 1 {
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "Plusieurs contrats de {} commencent le {} (n° {}) : precisez lequel, par son numero.",
                    salarie,
                    debut,
                    parti.iter().map(|x| if x.is_empty() { "sans numero" } else { x.as_str() }).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        return Ok(parti.first().cloned());
    };
    if parti.iter().any(|x| x == n) {
        return Ok(Some(n.to_string()));
    }
    if let Some(m) = mois.filter(|m| !m.is_empty()) {
        let (_d, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, m, None)?;
        let propres = numeros_du_jour(&paies, salarie, debut)?;
        if let Some(i) = propres.iter().position(|x| x == n) {
            return Ok(etats::apparier(&propres, &parti)[i].clone());
        }
    }
    Ok(parti.iter().find(|x| x.is_empty()).cloned())
}

/// `apercu_avenant` : ce que l'avenant d'un contrat parti modifierait, article par article, l'ancien
/// texte (celui de la version sur laquelle il est parti) et le nouveau (la paie validee). Rien n'est ecrit.
pub(crate) fn avenant_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let salarie_v = cle(charge, "salarie")?;
    let debut_v = cle(charge, "debut")?;
    let mut m = Moteur::nouveau(ctx);
    // `contrat_parti` : le dernier contrat parti a la signature pour cette paie.
    let lignes = m.lignes()?;
    // Le contrat que la page vise : son numero (que `signature-apercu` rend) et le mois de l'ecran, facultatifs.
    let numero_voulu = charge.get("numero").filter(|v| vrai(v)).map(valeur_en_texte);
    let mois_voulu = charge.get("mois").filter(|v| vrai(v)).map(valeur_en_texte);
    let parti = match (salarie_v.as_str(), debut_v.as_str()) {
        (Some(s), Some(d)) => {
            let j = numero_du_contrat(ctx, &lignes, s, d, numero_voulu.as_deref(), mois_voulu.as_deref())?;
            // Un contrat dont le numero est donne et que le journal ne connait pas n'est jamais parti (le jumeau qui n'est pas parti).
            if numero_voulu.is_some() && j.is_none() {
                None
            } else {
                envois::etats::contrat_parti(&lignes, s, d, j.as_deref())?
            }
        }
        _ => None,
    };
    let Some(parti) = parti else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat n'est parti à la signature pour {} du {} : il n'y a rien à modifier.", valeur_en_texte(salarie_v), jj_mm_aaaa(debut_v)?),
        ));
    };
    let (salarie, debut) = (valeur_en_texte(salarie_v), valeur_en_texte(debut_v));
    // `_paie_du_contrat(ctx, parti["mois"], salarie, debut)`.
    let mois_parti = valeur_en_texte(&parti.mois);
    let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois_parti, None)?;
    // La paie de CE contrat : celle dont le journal connait le numero (`apparier`), sinon celle qui le porte.
    let numero_parti = valeur_en_texte(&etats::numero_de_cle(&parti.numero));
    let trouvee = etats::paie_du_contrat(&paies, &lignes, &salarie, &debut, &numero_parti, numero_voulu.is_none())?;
    let Some(i) = trouvee else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat de {} commençant le {} dans les engagements de {}.", salarie, jj_mm_aaaa(debut_v)?, mois_parti),
        ));
    };
    let p = &paies[i];
    let g = m.globales()?;
    let deja = envois::etats::avenants_du_contrat(g.avenants.get(&(salarie.clone(), debut.clone(), numero_parti.clone())), &parti);
    let ent = documents::avenant::ent_du_mois(ctx, &mois_parti)?;
    let modele = documents::avenant::modele_du_contrat(&ent, &ctx.gabarits())?;
    let paie_json = engagement::relire::ecrire(ctx, p)?;
    let numero_porte = Some(p.engagement.numero.clone().unwrap_or_else(|| "None".to_string()));
    let vals = documents::avenant::valeurs_de_la_paie(ctx, &ent, &paie_json, Some(&numero_porte))?;
    // L'ANCIEN TEXTE : celui que le contrat disait sur la version de l'engagement sur laquelle il (ou son dernier avenant)
    // est parti, ecrit du modele avec les valeurs de cette version.
    let departs = envois::journal::departs(&ctx.racine)?;
    let siens = engagement::etats::du_mois(ctx, &mois_parti)?.into_iter().find_map(|e| {
        let v = e.valide()?.clone();
        let b = Value::Object(v.objet().brut.clone());
        (engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(salarie.as_str()) && b.get("numero").map(valeur_en_texte) == p.engagement.numero).then_some(v)
    });
    let ancienne = siens
        .as_ref()
        .and_then(|v| engagement::etats::version_au_depart(ctx, v, &departs, engagement::departs::Envoi::Contrat))
        .and_then(|x| x.get("paies")?.get(&mois_parti)?.get("paie").cloned())
        .ok_or_else(|| {
            erreur(
                "ErreurOperation",
                format!("Le journal ne dit pas sur quelle version de l'engagement le contrat n° {} est parti : l'avenant ne peut pas citer son ancien texte.", numero_parti),
            )
        })?;
    let avant = documents::avenant::valeurs_de_la_paie(ctx, &ent, &ancienne, Some(&numero_porte))?;
    let modifs = documents::avenant::modifications(&modele, &avant, &vals)?;
    let mut signes: Vec<i64> = Vec::new();
    for a in deja.iter().filter(|a| a.get("signe").is_some_and(vrai)) {
        signes.push(a.get("numero").and_then(Value::as_i64).ok_or_else(|| erreur("TypeError", "un numero d'avenant n'est pas un entier"))?);
    }
    let sal = &p.salarie;
    let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => valeur_en_texte(get_requis(sal, "prenom")?),
    };
    let modifications: Vec<Value> = modifs
        .iter()
        .map(|x| json!({"numero": x.numero, "article": x.article, "avant": x.avant, "apres": x.apres, "blocs": x.blocs}))
        .collect();
    Ok(json!({
        "salarie": salarie_v,
        "debut": debut_v,
        "fin": p.engagement.fin.map_or("None".to_string(), |f| f.iso()),
        "nom": format!("{} {}", prenom, valeur_en_texte(get_requis(sal, "nom")?)),
        "spectacle": p.titre()?,
        "mois": parti.mois,
        "contrat": parti.numero,
        "date_contrat": parti.date(),
        "signe": parti.signe,
        "fichier_parti": parti.fichier,
        "numero": signes.iter().max().copied().unwrap_or(0) + 1,
        "effet": debut_v,
        "modifications": modifications,
        "avenants": deja,
        "remplacable": debut > ctx.aujourdhui.iso(),
        // Sans boite courriel, l'avenant s'imprime avec les contrats (`contrats-imprimer`) : rien ne part d'ici.
        "boite_liee": crate::routes::ouverture::boite_liee(ctx)?,
        "destinataire": sal.get("email").cloned().unwrap_or_else(|| json!("")),
    }))
}

/// `journal.CONTRAT_RECU`.
const CONTRAT_RECU: &str = "contrat-recu";

/// `_chemin_piece` : le fichier d'une piece partie existe-t-il, dans
/// `sorties/<mois>/` ou son `envoi/` ? Un NOM, jamais un chemin.
fn piece_existe(racine: &Path, mois: &Value, fichier: &Value) -> R<bool> {
    if !vrai(mois) || !vrai(fichier) {
        return Ok(false);
    }
    let nom = valeur_en_texte(fichier);
    if ["/", "\\", ".."].iter().any(|x| nom.contains(x)) {
        return Ok(false);
    }
    let Value::String(f) = fichier else {
        return Err(erreur("TypeError", format!("'fichier' : {}", socle::fichiers::attendu("un texte", nom_type(fichier)))));
    };
    let base = racine.join("sorties").join(valeur_en_texte(mois));
    Ok([base.join("envoi").join(f), base.join(f)].iter().any(|c| c.is_file()))
}

fn piece(racine: &Path, quoi: &str, quand: &Value, mois: &Value, fichier: &Value) -> R<Value> {
    Ok(json!({"quoi": quoi, "quand": quand, "mois": mois, "fichier": fichier, "existe": piece_existe(racine, mois, fichier)?}))
}

/// `(c.get("salarie"), c.get("debut")) == (salarie, debut)`.
fn est_cette_paie(c: &Value, salarie: &Value, debut: &Value) -> R<bool> {
    Ok(valeurs_egales(get(c, "salarie")?, salarie) && valeurs_egales(get(c, "debut")?, debut))
}

/// `pieces_parties` : les FICHIERS partis pour une paie -- la DPAE deposee, le
/// contrat envoye a la signature, le contrat revenu signe et la preuve de la
/// signature. Lu au journal ; rien ne part.
fn pieces_parties(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = |k: &str| charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
    let salarie = cle("salarie")?;
    let debut = cle("debut")?;
    let racine = ctx.racine.as_path();
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;

    let mut dpae = Vec::new();
    for l in etats::filtrer(&lignes, Some(DPAE), None, true)? {
        if !vrai(get(l, "accepte")?) || !vrai(get(l, "fichier")?) {
            continue;
        }
        let embauches = items(getd(ou(get(l, "detail")?, objet_vide()), "embauches", &etats::LISTE_VIDE)?)?;
        let mut trouve = false;
        for e in &embauches {
            if est_cette_paie(e, salarie, debut)? {
                trouve = true;
                break;
            }
        }
        if trouve {
            dpae.push(piece(racine, "DPAE déposée", get(l, "quand")?, get(l, "mois")?, get(l, "fichier")?)?);
        }
    }

    let mut contrat: Vec<Value> = Vec::new();
    for l in etats::filtrer(&lignes, Some(SIGNATURE), None, false)? {
        if !vrai(get(l, "accepte")?) {
            continue;
        }
        let parti = if l.pointer("/detail/main_propre").is_some_and(vrai) { "imprimé" } else { "envoyé à la signature" };
        for c in items(getd(ou(get(l, "detail")?, objet_vide()), "contrats", &etats::LISTE_VIDE)?)? {
            if !(est_cette_paie(&c, salarie, debut)? && vrai(get(&c, "fichier")?)) {
                continue;
            }
            let quoi = if est(get(&c, "acte")?, "avenant") {
                format!("Avenant n° {} {}", valeur_en_texte(get(&c, "avenant")?), parti)
            } else if est(get(&c, "acte")?, "remplace") {
                format!("Contrat qui annule et remplace le n° {}, {}", valeur_en_texte(get(&c, "remplace")?), parti)
            } else {
                format!("Contrat {}", parti)
            };
            let mois = ou(get(&c, "mois")?, get(l, "mois")?);
            contrat.push(piece(racine, &quoi, get(l, "quand")?, mois, get(&c, "fichier")?)?);
        }
    }
    for l in etats::filtrer(&lignes, Some(CONTRAT_RECU), None, false)? {
        let d = ou(get(l, "detail")?, objet_vide());
        if !vrai(get(l, "accepte")?) || !est_cette_paie(d, salarie, debut)? {
            continue;
        }
        if vrai(get(d, "fichier_signe")?) {
            let quoi = if est(get(d, "acte")?, "avenant") {
                format!("Avenant n° {} signé, contresigné", valeur_en_texte(get(d, "avenant")?))
            } else {
                "Contrat signé, contresigné".to_string()
            };
            contrat.push(piece(racine, &quoi, get(l, "quand")?, get(l, "mois")?, get(d, "fichier_signe")?)?);
        }
        if vrai(get(d, "preuve")?) {
            contrat.push(piece(racine, "Réponse « Je signe » du salarié (preuve)", get(l, "quand")?, get(l, "mois")?, get(d, "preuve")?)?);
        }
    }
    // `contrat.sort(key=lambda x: x["quand"] or "")` : stable.
    contrat.sort_by_key(|x| if vrai(&x["quand"]) { valeur_en_texte(&x["quand"]) } else { String::new() });
    Ok(json!({"dpae": dpae, "contrat": contrat}))
}
