//! Les apercus des documents du mois : `signature-apercu`, `avenant-apercu`,
//! `envoi-apercu` -- ce que la page montre AVANT qu'on envoie un contrat, un avenant ou les
//! documents du mois.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et de `operations_documents.py`
//! (`apercu_signature`, `apercu_avenant`, `apercu_envoi`, `documents_prets`, `aer_manquantes`,
//! `ranger_aer`, `contrat_parti`, `saisie_aux_numeros_declares`), porte a l'identique : meme JSON,
//! memes refus sous la meme classe.
//!
//! Aucun reseau, aucun mot de passe. Les ecritures de ce module sont celles que le Python faisait
//! deja : le cache `paies/AAAA-MM.calcul.json` du calcul du mois, et, pour `envoi-apercu`, le
//! rangement des AER revenues (`ranger_aer`) dans `sorties/<mois>/envoi/`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::etats::{self, erreur, est, get, getd, items, objet_vide, py_str, LISTE_VIDE, SIGNATURE};
use crate::fichiers::vrai;
use crate::retours::Echec;
use crate::routes_mois::{dans_l_ensemble, premier, Moteur, SaisieMois};

type R<T> = Result<T, Echec>;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["signature-apercu", "avenant-apercu", "envoi-apercu"];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "signature-apercu" => signature_apercu(o, ctx, charge),
        "avenant-apercu" => avenant_apercu(o, ctx, charge),
        "envoi-apercu" => envoi_apercu(o, ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

/// `charge["cle"]` : `KeyError` si elle manque.
fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(k)))
}

/// `Path.glob("<prefixe>*<suffixe>")` sur un dossier : y a-t-il un nom qui s'y accorde ? Un dossier
/// qui n'en est pas un, ou qu'on ne peut lire, ne rend rien.
fn un_nom_commence_et_finit(dossier: &Path, prefixe: &str, suffixe: &str) -> bool {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return false };
    lecture.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(suffixe)
    })
}

/// La paire (salarie, debut) d'une ligne du journal, si les deux sont des textes : toute autre
/// valeur ne peut egaler une cle (salarie, debut) de la saisie.
fn paire_de(c: &Value) -> R<Option<(String, String)>> {
    let (s, d) = (get(c, "salarie")?, get(c, "debut")?);
    Ok(match (s.as_str(), d.as_str()) {
        (Some(s), Some(d)) => Some((s.to_string(), d.to_string())),
        _ => None,
    })
}

fn nul() -> &'static Value {
    static V: Value = Value::Null;
    &V
}

// ------------------------------------------------------------------ signature-apercu

/// `apercu_signature` : les contrats du mois et l'etat de leur demande de signature.
fn signature_apercu(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    signature_apercu_avec(o, ctx, charge, true)
}

/// `cache` : `false` calcule les paies sans toucher a `paies/AAAA-MM.calcul.json` (la relance d'un contrat, route
/// d'envoi : le `charger` du Python ne l'ecrit jamais).
pub(crate) fn signature_apercu_avec(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, cache: bool) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = py_str(mois_v);
    let (data, _rep, paies) = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, &mois, None, cache)?;
    let mut m = Moteur::nouveau(o, ctx);
    let g = m.globales()?;
    let lignes = m.lignes()?;
    // `empreintes_engagements` et `numeros_saisis` : `{}` quand le mois ne se lit pas ; ici il s'est lu.
    let rangs: Vec<(String, String)> = paies
        .iter()
        .map(|p| Ok((py_str(get(&p.salarie, "_cle")?), p.engagement.debut.iso())))
        .collect::<R<_>>()?;
    // Par RANG dans la saisie : deux contrats du meme salarie et du meme jour ont chacun leur empreinte et leur numero.
    let uniques: Vec<(String, String)> = rangs.iter().enumerate().map(|(i, (c, d))| (c.clone(), format!("{}#{}", d, i))).collect();
    let saisie = SaisieMois { data, paies: Vec::new() };
    let actuelles = Moteur::empreintes_engagements(&saisie, &uniques);
    let saisis = Moteur::numeros_saisis(&saisie, &uniques);
    let cles_paies = etats::cles_des_paies(&lignes, &crate::routes_mois::contrats_des_paies(&paies))?;

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
    let mut contrats = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        // UN PERMANENT N'A PAS DE CONTRAT D'USAGE A SIGNER ICI.
        if !vrai(&p.spectacle) {
            continue;
        }
        let (cle_s, debut) = rangs[i].clone();
        let cles = &cles_paies[i];
        let rang_du_jour = rangs[..i].iter().filter(|r| **r == rangs[i]).count();
        let ecart = crate::avenant::ecart_du_contrat(ctx, &mois, &cle_s, &debut, Some(cles[0].2.as_str()), rang_du_jour);
        let etat = crate::routes_mois::etat_signature(
            premier(&g.contrats, cles).unwrap_or(nul()),
            premier(&g.recus, cles).unwrap_or(nul()),
            premier(&g.recus_par, cles).unwrap_or(nul()),
            premier(&g.partis, cles).unwrap_or(nul()),
            actuelles.get(&uniques[i]).map(String::as_str),
            &mois,
            dans_l_ensemble(&g.renvoyes, cles),
            premier(&g.numeros_partis, cles).unwrap_or(nul()),
            saisis.get(&uniques[i]).unwrap_or(nul()),
            premier(&g.avenants, cles).unwrap_or(nul()),
            &ecart,
        );
        let sal = &p.salarie;
        let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => py_str(get_requis(sal, "prenom")?),
        };
        let numero = p.engagement.numero.clone().map_or(Value::Null, Value::String);
        let fin = p.engagement.fin.map_or("None".to_string(), |f| f.iso());
        let mut l = Map::new();
        l.insert("salarie".into(), json!(cle_s));
        l.insert("nom".into(), json!(format!("{} {}", py_str(get_requis(sal, "nom")?), prenom)));
        l.insert("destinataire".into(), sal.get("email").cloned().unwrap_or_else(|| json!("")));
        l.insert("spectacle".into(), json!(p.titre()?));
        l.insert("debut".into(), json!(debut));
        l.insert("fin".into(), json!(fin));
        l.insert("numero".into(), numero);
        l.insert("demande_le".into(), premier(&demandes, cles).cloned().unwrap_or(Value::Null));
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
        "boite_liee": crate::routes_accueil::boite_liee(ctx)?,
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

/// `d["cle"]` sur une fiche : `KeyError` si elle manque.
fn get_requis<'a>(d: &'a Value, k: &str) -> R<&'a Value> {
    d.get(k).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(k)))
}

// ------------------------------------------------------------------ avenant-apercu

/// `jj_mm_aaaa(x)` : `JJ/MM/AAAA`, `ValueError` si `x` ne se lit pas comme une date.
fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(crate::date::d(v).map_err(|m| erreur("ValueError", m))?.jj_mm_aaaa())
}

/// Les numeros propres des paies de ce salarie qui commencent ce jour-la (celles d'un spectacle), dans l'ordre de la saisie.
pub(crate) fn numeros_du_jour(paies: &[crate::calcul::Paie], salarie: &str, debut: &str) -> R<Vec<String>> {
    let mut v = Vec::new();
    for x in paies {
        if est(get_requis(&x.salarie, "_cle")?, salarie) && x.engagement.debut.iso() == debut && vrai(&x.spectacle) {
            v.push(x.engagement.numero.clone().unwrap_or_default());
        }
    }
    Ok(v)
}

/// L'indice, dans `paies`, de la paie du contrat que le journal connait sous `numero_j` : celle qui lui est appariee
/// (`etats::apparier`), sinon celle qui porte ce numero ; sinon, si `a_defaut_la_premiere`, la premiere paie du jour.
pub(crate) fn paie_du_contrat(
    paies: &[crate::calcul::Paie],
    lignes: &[Value],
    salarie: &str,
    debut: &str,
    numero_j: &str,
    a_defaut_la_premiere: bool,
) -> R<Option<usize>> {
    let mut des_jours: Vec<(usize, String)> = Vec::new();
    for (i, x) in paies.iter().enumerate() {
        if est(get_requis(&x.salarie, "_cle")?, salarie) && x.engagement.debut.iso() == debut && vrai(&x.spectacle) {
            des_jours.push((i, x.engagement.numero.clone().unwrap_or_default()));
        }
    }
    let propres: Vec<String> = des_jours.iter().map(|(_, n)| n.clone()).collect();
    let appariees = etats::apparier(&propres, &etats::numeros_demandes(lignes, salarie, debut)?);
    Ok((0..des_jours.len())
        .find(|&k| appariees[k].as_deref() == Some(numero_j))
        .or_else(|| (0..des_jours.len()).find(|&k| propres[k] == numero_j))
        .or(if a_defaut_la_premiere && !des_jours.is_empty() { Some(0) } else { None })
        .map(|k| des_jours[k].0))
}

/// Le numero SOUS LEQUEL LE JOURNAL parle du contrat que la page vise (Retours Windows, B, suite : deux contrats du meme salarie
/// qui commencent le meme jour sont deux contrats). `numero` : celui que l'ecran affiche (`signature-apercu`), `mois` : le mois de
/// l'ecran, qui donne les paies jumelles. `None` : ce contrat n'est jamais parti. Sans `numero`, le contrat est celui qui est
/// parti ; s'il y en a plusieurs, l'appelant doit preciser lequel (refus).
pub(crate) fn numero_du_contrat(
    o: &HashMap<String, String>,
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
        let (_d, _rep, paies) = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, m, None, false)?;
        let propres = numeros_du_jour(&paies, salarie, debut)?;
        if let Some(i) = propres.iter().position(|x| x == n) {
            return Ok(etats::apparier(&propres, &parti)[i].clone());
        }
    }
    Ok(parti.iter().find(|x| x.is_empty()).cloned())
}

/// `apercu_avenant` : ce que l'avenant d'un contrat parti modifierait, article par article, l'ancien
/// texte (lu dans le PDF parti) et le nouveau (la paie d'aujourd'hui). Rien n'est ecrit.
fn avenant_apercu(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    avenant_apercu_avec(o, ctx, charge, true)
}

/// `cache` : `false` calcule les paies sans toucher a `paies/AAAA-MM.calcul.json` (l'envoi d'un avenant : le
/// `charger` du Python ne l'ecrit jamais).
pub(crate) fn avenant_apercu_avec(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value, cache: bool) -> R<Value> {
    let salarie_v = cle(charge, "salarie")?;
    let debut_v = cle(charge, "debut")?;
    let mut m = Moteur::nouveau(o, ctx);
    // `contrat_parti` : le dernier contrat parti a la signature pour cette paie.
    let lignes = m.lignes()?;
    // Le contrat que la page vise : son numero (que `signature-apercu` rend) et le mois de l'ecran, facultatifs.
    let numero_voulu = charge.get("numero").filter(|v| vrai(v)).map(py_str);
    let mois_voulu = charge.get("mois").filter(|v| vrai(v)).map(py_str);
    let parti = match (salarie_v.as_str(), debut_v.as_str()) {
        (Some(s), Some(d)) => {
            let j = numero_du_contrat(o, ctx, &lignes, s, d, numero_voulu.as_deref(), mois_voulu.as_deref())?;
            // Un contrat dont le numero est donne et que le journal ne connait pas n'est jamais parti (le jumeau qui n'est pas parti).
            if numero_voulu.is_some() && j.is_none() {
                None
            } else {
                crate::avenant::contrat_parti(&lignes, s, d, j.as_deref())?
            }
        }
        _ => None,
    };
    let Some(parti) = parti else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat n'est parti à la signature pour {} du {} : il n'y a rien à modifier.", py_str(salarie_v), jj_mm_aaaa(debut_v)?),
        ));
    };
    let (salarie, debut) = (py_str(salarie_v), py_str(debut_v));
    // `_paie_du_contrat(ctx, parti["mois"], salarie, debut)`.
    let mois_parti = py_str(&parti.mois);
    let (_data, rep, paies) = crate::apercu_dsn::paies_aux_numeros_declares_avec(o, ctx, &mois_parti, None, cache)?;
    // La paie de CE contrat : celle dont le journal connait le numero (`apparier`), sinon celle qui le porte.
    let numero_parti = py_str(&etats::numero_de_cle(&parti.numero));
    let trouvee = paie_du_contrat(&paies, &lignes, &salarie, &debut, &numero_parti, numero_voulu.is_none())?;
    let Some(i) = trouvee else {
        return Err(erreur(
            "ErreurOperation",
            format!("Aucun contrat de {} commençant le {} dans la paie de {}.", salarie, jj_mm_aaaa(debut_v)?, mois_parti),
        ));
    };
    let p = &paies[i];
    let Some(pdf) = crate::avenant::chemin_piece(&ctx.racine, &parti.mois, &parti.fichier) else {
        return Err(erreur(
            "ErreurOperation",
            format!("Le contrat parti ({}) n'est plus dans sorties/{} : l'avenant ne peut pas citer son texte.", py_str(&parti.fichier), mois_parti),
        ));
    };
    let g = m.globales()?;
    let deja = crate::avenant::avenants_du_contrat(g.avenants.get(&(salarie.clone(), debut.clone(), numero_parti.clone())), &parti);
    // Ce que les avenants SIGNES ont deja ecrit fait foi, paragraphe par paragraphe.
    let texte_signe = crate::avenant::textes_des_avenants_de(&deja, true)?;
    let ent = crate::avenant::ent_du_mois(ctx, &mois_parti)?;
    let modele = crate::avenant::modele_du_contrat(&ent)?;
    let paie_json = rep
        .get("paies")
        .and_then(Value::as_array)
        .and_then(|l| l.get(i))
        .ok_or("reponse du calcul illisible : `paies`")?;
    let numero_porte = Some(p.engagement.numero.clone().unwrap_or_else(|| "None".to_string()));
    let vals = crate::avenant::valeurs_de_la_paie(ctx, &ent, paie_json, Some(&numero_porte))?;
    let texte_pdf = crate::avenant::texte_du_pdf(&pdf)?;
    let modifs = match crate::avenant::modifications(&modele, &vals, &texte_pdf, &numero_parti, &texte_signe) {
        Ok(v) => v,
        Err(crate::avenant::ErrAvenant::Impossible(msg)) => return Err(erreur("ErreurOperation", msg)),
        Err(crate::avenant::ErrAvenant::Autre(msg)) => return Err(Echec::from(msg)),
    };
    let mut signes: Vec<i64> = Vec::new();
    for a in deja.iter().filter(|a| a.get("signe").is_some_and(vrai)) {
        signes.push(a.get("numero").and_then(Value::as_i64).ok_or_else(|| erreur("TypeError", "un numero d'avenant n'est pas un entier"))?);
    }
    let sal = &p.salarie;
    let prenom = match sal.get("prenom_affichage").filter(|v| vrai(v)) {
        Some(v) => py_str(v),
        None => py_str(get_requis(sal, "prenom")?),
    };
    let modifications: Vec<Value> = modifs
        .iter()
        .map(|x| json!({"numero": x.numero, "article": x.article, "avant": x.avant, "apres": x.apres, "blocs": x.blocs}))
        .collect();
    Ok(json!({
        "salarie": salarie_v,
        "debut": debut_v,
        "fin": p.engagement.fin.map_or("None".to_string(), |f| f.iso()),
        "nom": format!("{} {}", prenom, py_str(get_requis(sal, "nom")?)),
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
        "destinataire": sal.get("email").cloned().unwrap_or_else(|| json!("")),
    }))
}

// ------------------------------------------------------------------ envoi-apercu

const MOIS_FR: [&str; 12] =
    ["Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre"];
const MOIS_ACCENTS: [&str; 12] =
    ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

/// `int(texte)` d'un morceau de `mois` : `ValueError` sinon.
fn entier(t: &str) -> R<i64> {
    let s = t.trim();
    let (neg, chiffres) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if chiffres.is_empty() || !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return Err(erreur("ValueError", format!("nombre entier illisible : {}", crate::fichiers::repr(t))));
    }
    let n: i64 = chiffres.parse().unwrap_or(i64::MAX);
    Ok(if neg { -n } else { n })
}

/// `(int(mois[:4]), int(mois[5:7]))`.
fn an_et_mois(mois: &str) -> R<(i64, i64)> {
    let tranche = |a: usize, b: usize| -> String { mois.chars().skip(a).take(b - a).collect() };
    Ok((entier(&tranche(0, 4))?, entier(&tranche(5, 7))?))
}

/// `LISTE[mo - 1]` avec l'indice negatif de Python : `IndexError` hors de la liste.
fn rang_du_mois(mo: i64) -> R<usize> {
    let r = mo - 1;
    let r = if r < 0 { r + 12 } else { r };
    if (0..12).contains(&r) {
        Ok(r as usize)
    } else {
        Err(erreur("IndexError", crate::date::mois_impossible(mo)))
    }
}

/// `envoi.etiquette` : `2026-08` -> `Aout2026`.
fn etiquette(mois: &str) -> R<String> {
    let (an, mo) = an_et_mois(mois)?;
    Ok(format!("{}{}", MOIS_FR[rang_du_mois(mo)?], an))
}

/// `envoi.nom_fichier` : `AER_<cle>_<etiquette>.pdf`.
fn nom_fichier(prefixe: &str, cle: &str, etiq: &str) -> String {
    format!("{}_{}_{}.pdf", prefixe, cle, etiq)
}

/// Une attestation d'un retour de nature 46, rattachee ou non a une fiche.
struct Attestation {
    nom_archive: String,
    qui: String,
    donnees: Vec<u8>,
    cle: Option<String>,
}

/// `_attestations_archivees` : (periode, qui) -> les attestations du DERNIER retour qui le porte, lues
/// dans les retours 46, dans l'ordre de l'archive. `mois` : seulement cette periode.
fn attestations_archivees(ctx: &Contexte, mois: Option<&str>, seulement: Option<&[std::path::PathBuf]>) -> R<HashMap<(String, String), Vec<Attestation>>> {
    let dossier = ctx.racine.join("retours");
    // `fichiers` : seulement ces archives (la releve range ce qu'elle vient de telecharger).
    let mut fichiers: Vec<String> = match seulement {
        Some(liste) => liste.iter().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect(),
        None => match std::fs::read_dir(&dossier) {
            Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("_nature46_")).collect(),
            Err(_) => Vec::new(),
        },
    };
    fichiers.sort();
    let index: Vec<Value> = ctx
        .salaries
        .iter()
        .filter(|(_, v)| v.is_object())
        .map(|(cle, v)| {
            let champ = |c: &str| v.get(c).map_or("".to_string(), py_str);
            json!([cle, format!("{} {}", champ("nom"), champ("prenom"))])
        })
        .collect();
    let mut vues: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
    for nom in fichiers {
        let chemin = dossier.join(&nom);
        let octets = crate::routes_apercus::lire_octets(&chemin.to_string_lossy())?;
        let atts = crate::retours::aer_spectacle(&octets)?;
        let liste = atts.as_array().cloned().unwrap_or_default();
        let cles = if liste.is_empty() {
            Value::Array(Vec::new())
        } else {
            let quis: Vec<Value> = liste.iter().map(|a| a["qui"].clone()).collect();
            crate::retours::rattacher(&quis, &index)
        };
        let mut de_ce_retour: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
        for (a, c) in liste.iter().zip(cles.as_array().map(Vec::as_slice).unwrap_or(&[])) {
            let periode = py_str(&a["periode"]);
            if mois.is_some_and(|m| !m.is_empty()) && Some(periode.as_str()) != mois {
                continue;
            }
            let qui = py_str(&a["qui"]);
            let donnees = crate::retours::base64_lire(a["donnees"].as_str().unwrap_or("")).map_err(Echec::from)?;
            de_ce_retour.entry((periode, qui.clone())).or_default().push(Attestation {
                nom_archive: py_str(&a["nom_archive"]),
                qui,
                donnees,
                cle: c.as_str().map(str::to_string),
            });
        }
        vues.extend(de_ce_retour);
    }
    Ok(vues)
}

/// `aer_manquantes` : les salaries dont France Travail a renvoye l'AER du mois, mais dont le PDF signe
/// n'est pas dans `sorties/<mois>/envoi/`.
fn aer_manquantes(ctx: &Contexte, mois: &str) -> R<Vec<String>> {
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    let mut out: Vec<String> = Vec::new();
    for groupe in attestations_archivees(ctx, Some(mois), None)?.values() {
        let Some(att) = groupe.first() else { continue };
        let present = match att.cle.as_deref().filter(|c| !c.is_empty()) {
            Some(cle) => dossier.join(nom_fichier("AER", cle, &etiquette(mois)?)).exists(),
            None => false,
        };
        if !present {
            let nom = att.cle.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| att.qui.clone());
            if !out.contains(&nom) {
                out.push(nom);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// `charger(ctx, "paies/<periode>.json").paies`, sans rien dire de ce qui le fait echouer.
/// `sans_cache` : la releve des retours (`releve_retours`) range les AER sans toucher a `AAAA-MM.calcul.json`,
/// que le Python n'ecrivait pas la ; l'apercu d'envoi, lui, passe par le calcul du mois et son cache.
fn paies_de_la_periode(o: &HashMap<String, String>, ctx: &Contexte, periode: &str, sans_cache: bool) -> Vec<crate::calcul::Paie> {
    let essai = || -> Result<Vec<crate::calcul::Paie>, crate::routes_ouverture::Refus> {
        let chemin = ctx.racine.join("paies").join(format!("{}.json", periode));
        let data = crate::engagement::lire_saisie(&chemin)?;
        let rep = if sans_cache {
            crate::service::calculer_avec(ctx, &json!({ "saisie": data })).map_err(|m| crate::routes_ouverture::refus_du_calcul(ctx, &data, m))?
        } else {
            crate::routes_ouverture::paie_du_mois(o, ctx, &data)?
        };
        let engs = crate::routes_ouverture::engagements_calcules(ctx, &data, &rep)?;
        let paies_json = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
        let mut paies = Vec::new();
        for (e, v) in engs.into_iter().zip(paies_json) {
            paies.push(crate::routes_ouverture::paie_de_json(ctx, e, v)?);
        }
        let mois = data.get("mois").map(crate::date::en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
        let annee: i32 = mois.chars().take(4).collect::<String>().parse().map_err(|_| "annee illisible")?;
        crate::routes_ouverture::numeroter(ctx, &mut paies, annee)?;
        Ok(paies)
    };
    essai().unwrap_or_default()
}

/// Un dossier temporaire, retire quand on a fini.
struct Temporaire(std::path::PathBuf);

impl Temporaire {
    fn nouveau() -> R<Temporaire> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!("assemblage-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&p).map_err(|e| erreur("OSError", e.to_string()))?;
        Ok(Temporaire(p))
    }
}

impl Drop for Temporaire {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `fabrique.assembler` : les PDF mis a la suite.
fn assembler(pieces: &[Vec<u8>]) -> R<Vec<u8>> {
    let tmp = Temporaire::nouveau()?;
    let mut liste = Vec::new();
    for (i, octets) in pieces.iter().enumerate() {
        let f = tmp.0.join(format!("piece{}.pdf", i));
        std::fs::write(&f, octets).map_err(|e| erreur("OSError", e.to_string()))?;
        liste.push(json!({"fichier": f.to_string_lossy()}));
    }
    let description = json!({"format": 1, "pieces": liste});
    fabrique_pdf::assemblage::assembler(&description.to_string()).map_err(|r| Echec::from(r.0))
}

/// `chemin_identite("cachet")` : le tampon pose, quelle que soit son extension (png, jpg, svg).
fn cachet_pose(ent: &Value) -> Option<String> {
    let dossier = crate::bulletin::gabarits_de(ent).join("identite");
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("cachet{}", e))).find(|f| f.exists()).map(|f| f.to_string_lossy().into_owned())
}

/// `RangementAer` : ce que le rangement des AER a fait -- les PDF ecrits `(periode, cle, chemin)`, les attestations sans salarie
/// deposees a part `(qui, chemin)`, les refusees `(cle, periode, ecarts)`.
#[derive(Default)]
pub(crate) struct RangementAer {
    pub ecrits: Vec<(String, String, std::path::PathBuf)>,
    pub a_ranger: Vec<(String, std::path::PathBuf)>,
    pub refusees: Vec<(String, String, Vec<String>)>,
}

/// `ranger_aer(ctx, mois=mois)` : signe et range les AER deja archivees la ou l'envoi ira les chercher -- en phrases, pour la releve
/// et l'ouverture (qui ne signent jamais malgre un ecart).
pub(crate) fn ranger_aer(o: &HashMap<String, String>, ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>, sans_cache: bool) -> R<Vec<String>> {
    let fait = ranger_aer_detail(o, ctx, mois, fichiers, sans_cache, false)?;
    let mut phrases: Vec<String> = Vec::new();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    for (qui, chemin) in &fait.a_ranger {
        let refuge = chemin.parent().map(|p| p.display().to_string()).unwrap_or_default();
        phrases.push(format!("Attestation France Travail de {} : aucun salarie ne porte ce nom. Deposee dans {} ; verifier la fiche, puis `paie aer`.", qui, refuge));
    }
    for (cle, periode, ecarts) in &fait.refusees {
        phrases.push(format!("Attestation France Travail de {} ({}) NON SIGNEE : elle ne dit pas ce que la paie dit ({}). La correction se fait sur la DSN, pas sur le PDF.", cle, periode, ecarts.join("; ")));
    }
    Ok(phrases)
}

/// `ranger_aer` du Python, structure : le rangement et ce qu'il a laisse. `forcer` : signer malgre un ecart (`aer --forcer`).
pub(crate) fn ranger_aer_detail(o: &HashMap<String, String>, ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>, sans_cache: bool, forcer: bool) -> R<RangementAer> {
    use crate::aer::{authentifier, Authentification, Echec as AerEchec, PaieAer};
    let vues = attestations_archivees(ctx, mois, fichiers)?;
    let mut par_periode: HashMap<String, Vec<crate::calcul::Paie>> = HashMap::new();
    for (periode, _) in vues.keys() {
        if !periode.is_empty() && !par_periode.contains_key(periode) {
            par_periode.insert(periode.clone(), paies_de_la_periode(o, ctx, periode, sans_cache));
        }
    }
    let mut cles: Vec<&(String, String)> = vues.keys().collect();
    cles.sort();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    let mut fait = RangementAer::default();
    for k in cles {
        let (periode, _) = k;
        let groupe = &vues[k];
        let cle = groupe[0].cle.clone().filter(|c| !c.is_empty());
        let Some(cle) = cle.filter(|_| !periode.is_empty()) else {
            // On ne devine pas : un nom que le rapprochement ne reconnait pas est peut-etre un salarie archive.
            let refuge = ctx.racine.join("retours").join("aer-a-ranger");
            std::fs::create_dir_all(&refuge).map_err(|e| erreur("OSError", e.to_string()))?;
            for att in groupe {
                let nom = att.nom_archive.rsplit('/').next().unwrap_or("").to_string();
                std::fs::write(refuge.join(nom), &att.donnees).map_err(|e| erreur("OSError", e.to_string()))?;
                fait.a_ranger.push((att.qui.clone(), refuge.join(att.nom_archive.rsplit('/').next().unwrap_or(""))));
            }
            continue;
        };
        let mut paies: Vec<PaieAer> = Vec::new();
        for p in par_periode.get(periode).map(Vec::as_slice).unwrap_or(&[]) {
            if !est(get_requis(&p.salarie, "_cle")?, &cle) {
                continue;
            }
            let e = &p.engagement;
            paies.push(PaieAer {
                debut: e.debut,
                fin: e.fin.ok_or_else(|| Echec::from("contrat sans fin"))?,
                cachets: i128::from(e.cachets),
                heures: e.heures.unwrap_or(0.0) as i128,
            });
        }
        let signature = crate::bulletin::identite_de(&ctx.entreprise, "signature").map(|f| f.to_string_lossy().into_owned());
        let cachet = cachet_pose(&ctx.entreprise);
        let (mut signees, mut ecarts): (Vec<Vec<u8>>, Vec<String>) = (Vec::new(), Vec::new());
        for att in groupe {
            let a = Authentification {
                ent: &ctx.entreprise,
                aujourdhui: ctx.aujourdhui,
                signature: signature.as_deref(),
                paies: if paies.is_empty() { None } else { Some(&paies) },
                forcer,
                cachet_fichier: cachet.as_deref(),
            };
            match authentifier(&att.donnees, &a) {
                Ok(octets) => signees.push(octets),
                Err(AerEchec::Ecarts(e)) => ecarts.extend(e),
                Err(AerEchec::Fabrique(m)) | Err(AerEchec::Autre(m)) => return Err(Echec::from(m)),
            }
        }
        if !ecarts.is_empty() {
            // Une seule fausse et rien ne part.
            fait.refusees.push((cle, periode.clone(), ecarts));
            continue;
        }
        let donnees = if signees.len() == 1 { signees.remove(0) } else { assembler(&signees)? };
        let cible = ctx.racine.join("sorties").join(periode).join("envoi");
        std::fs::create_dir_all(&cible).map_err(|e| erreur("OSError", e.to_string()))?;
        let chemin = cible.join(nom_fichier("AER", &cle, &etiquette(periode)?));
        std::fs::write(&chemin, donnees).map_err(|e| erreur("OSError", e.to_string()))?;
        fait.ecrits.push((periode.clone(), cle, chemin));
    }
    Ok(fait)
}

/// `documents_prets` : peut-on envoyer les documents du mois ? Le moteur repond, et dit pourquoi.
fn documents_prets(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Map<String, Value>> {
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    let produit = dossier.exists() && un_nom_commence_et_finit(&dossier, "Bulletin_", ".pdf");
    let groupe = ctx.racine.join("sorties").join(mois).join(format!("Bulletins_{}.pdf", etiquette(mois)?));
    let a_redecouper = !produit && groupe.exists();
    let mut empechements: Vec<String> = Vec::new();
    if a_redecouper {
        empechements.push(format!(
            "Le mois est produit, mais ses documents ne sont pas decoupes par salarie : il n'y a que {}, qui porte la paie de tout le monde. « Ecrire les documents des salaries » les ecrit un a un, avec les numeros du fichier archive, sans rien redeclarer.",
            groupe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        ));
    } else if !produit {
        empechements.push("Les documents du mois ne sont pas encore ecrits.".to_string());
    }
    let e = m.dsn_du_mois(mois)?;
    if !vrai(&e) {
        empechements.push("La DSN du mois n'est pas deposee.".to_string());
    } else if e.get("refusee").is_some_and(vrai) {
        empechements.push("La DSN du mois a ete REFUSEE : a corriger et a redeposer.".to_string());
    } else if !e.get("passee").is_some_and(vrai) {
        empechements.push(format!("La DSN est deposee ({}), son bilan n'est pas encore arrive.", etats::quand_lisible(e.get("quand").unwrap_or(nul()))));
    } else if !e.get("aer_recue").is_some_and(vrai) {
        empechements.push(
            "L'attestation employeur (AER) de France Travail n'est pas encore revenue -- elle suit la DSN de quelques heures ; l'ouverture du logiciel la releve."
                .to_string(),
        );
    } else {
        let manquent = aer_manquantes(ctx, mois)?;
        if !manquent.is_empty() {
            empechements.push(format!(
                "L'attestation employeur (AER) est revenue de France Travail, mais elle n'est pas signee pour {} : la paie ne confirme pas ce qu'elle affiche, ou le nom n'est pas reconnu. Voir les messages de l'ouverture.",
                manquent.join(", ")
            ));
        }
    }
    // LES DOCUMENTS DISENT CE QUE LA DSN DECLARE (ticket #240) : une paie modifiee depuis la DSN deposee attend sa DSN
    // refaite (et ses documents avec elle) ; envoyer avant, ce serait envoyer des documents que la DSN ne dit pas. TANT
    // QUE L'ANNULE-ET-REMPLACE EST ADMIS seulement (Leandre, 21/10/2026) : apres sa limite, la DSN deposee ne se refait
    // plus, la paie modifiee se declare dans la DSN suivante (« Corriger ce mois »), et les documents refaits repartent,
    // le courriel le disant (`routes_envois::choisir`).
    let remplacement = if vrai(&e) && !e.get("refusee").is_some_and(vrai) { m.etat_remplacement(mois, &Value::Null)? } else { Value::Null };
    if vrai(&remplacement["saisie_modifiee"]) && vrai(&remplacement["admis"]) {
        empechements.push(format!(
            "La paie a changé depuis la DSN déposée le {} : refaire le mois et sa DSN (annule-et-remplace) d'abord — les documents partent avec la paie que la DSN déclare.",
            etats::quand_lisible(e.get("quand").unwrap_or(nul()))
        ));
    }
    let mut r = Map::new();
    r.insert("pret".into(), json!(empechements.is_empty()));
    r.insert("empechements".into(), json!(empechements));
    r.insert("boite_liee".into(), json!(crate::routes_accueil::boite_liee(ctx)?));
    r.insert("produit".into(), json!(produit));
    r.insert("a_redecouper".into(), json!(a_redecouper));
    r.insert("dossier".into(), json!(if produit { dossier } else { dossier.parent().map(Path::to_path_buf).unwrap_or_default() }.to_string_lossy()));
    Ok(r)
}

/// Un message prepare pour un salarie : ce que l'ecran montre avant l'envoi.
struct Message {
    cle: String,
    nom: String,
    destinataire: String,
    sujet: String,
    pieces: Vec<std::path::PathBuf>,
}

impl Message {
    /// `Message.taille` : l'octet de chaque piece qui existe.
    fn taille(&self) -> u64 {
        self.pieces.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum()
    }
}

/// `envoi.preparer` : un message par salarie, a partir des pieces individuelles deja ecrites. Le texte
/// du corps, que l'apercu ne montre pas, n'est pas compose : seuls ses refus (`KeyError` d'une fiche
/// sans nom d'affichage ou sans signataire) sont gardes.
fn preparer(ctx: &Contexte, paies: &[crate::calcul::Paie], mois: &str, dossier: &Path, renvois: &HashMap<String, crate::envoi::Renvoi>) -> R<Vec<Message>> {
    const PIECES: [(&str, &str, &str); 4] = [
        ("bulletin", "Bulletin", "bulletin"),
        ("rectificatif", "BulletinRectificatif", "bulletin rectificatif"),
        ("certificat_cs", "CertificatCS", "certificat Congés Spectacles"),
        ("aer", "AER", "attestation France Travail"),
    ];
    let (an, mo) = an_et_mois(mois)?;
    let etiq = etiquette(mois)?;
    let mut salaries: Vec<Value> = paies.iter().map(|p| p.salarie.clone()).collect();
    // Celui qui n'a AUCUNE paie ce mois-ci mais un bulletin rectificatif d'un mois precedent.
    let prefixe = "BulletinRectificatif_";
    let suffixe = format!("_{}.pdf", etiq);
    let mut rectifs: Vec<String> = match std::fs::read_dir(dossier) {
        Ok(l) => l
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(&suffixe))
            .collect(),
        Err(_) => Vec::new(),
    };
    rectifs.sort();
    for f in rectifs {
        let cle: String = f.chars().skip(prefixe.chars().count()).take(f.chars().count() - prefixe.chars().count() - suffixe.chars().count()).collect();
        let connues: Vec<String> = salaries.iter().map(|s| s.get("_cle").map_or("None".to_string(), py_str)).collect();
        if !connues.contains(&cle) {
            let mut fiche = ctx.salarie(&cle).map_err(|m| erreur("KeyError", m))?;
            if let Some(o) = fiche.as_object_mut() {
                o.insert("_cle".into(), Value::String(cle.clone()));
            }
            salaries.push(fiche);
        }
    }
    let mut vus: Vec<String> = Vec::new();
    let mut messages = Vec::new();
    for s in &salaries {
        let cle = py_str(get_requis(s, "_cle")?);
        if vus.contains(&cle) {
            continue;
        }
        vus.push(cle.clone());
        if !s.get("email").is_some_and(vrai) {
            return Err(erreur(
                "ValueError",
                format!(
                    "Pas d'adresse e-mail pour {} {} : completer config/salaries.json avant d'envoyer.",
                    py_str(get_requis(s, "nom")?),
                    py_str(get_requis(s, "prenom")?)
                ),
            ));
        }
        let (mut pieces, mut courts) = (Vec::new(), Vec::new());
        for (_, prefixe, court) in PIECES {
            let chemin = dossier.join(nom_fichier(prefixe, &cle, &etiq));
            if chemin.exists() {
                pieces.push(chemin);
                courts.push(court);
            }
        }
        // Ce que le corps du message lit : `ent["nom_affichage"]`, `ent["signataire"]`.
        let ent = &ctx.entreprise;
        get_requis(ent, "nom_affichage")?;
        let sg = get_requis(ent, "signataire")?;
        if !sg.get("prenom_affichage").is_some_and(vrai) {
            get_requis(sg, "prenom")?;
        }
        let sujet = format!(
            "{} {} — {}{}",
            capitaliser(MOIS_ACCENTS[rang_du_mois(mo)?]),
            an,
            if renvois.contains_key(&cle) { "documents corrigés : " } else { "" },
            match courts.len() {
                0 => "documents".to_string(),
                1 => courts[0].to_string(),
                n => format!("{} et {}", courts[..n - 1].join(", "), courts[n - 1]),
            }
        );
        messages.push(Message {
            nom: format!("{} {}", py_str(get_requis(s, "prenom_affichage")?), py_str(get_requis(s, "nom")?)),
            destinataire: py_str(get_requis(s, "email")?).to_lowercase(),
            cle,
            sujet,
            pieces,
        });
    }
    Ok(messages)
}

/// `str.capitalize()`.
fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// `charger(ctx, paies/<mois>.json)` : la saisie du mois (le fichier lu) et ses paies, numerotees. L'envoi des
/// documents (`routes_envois`) prepare ses messages sur les memes paies que l'apercu.
pub(crate) fn charger_le_mois(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<(Value, Vec<crate::calcul::Paie>)> {
    charger_le_mois_avec(o, ctx, mois, true)
}

/// La meme ; `cache` : `false` calcule sans toucher a `paies/AAAA-MM.calcul.json` (le `charger` du Python ne l'ecrit
/// jamais : c'est ce que font les routes d'envoi).
pub(crate) fn charger_le_mois_avec(o: &HashMap<String, String>, ctx: &Contexte, mois: &str, cache: bool) -> R<(Value, Vec<crate::calcul::Paie>)> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let data = crate::engagement::lire_saisie(&chemin)?;
    let rep = if cache {
        crate::routes_ouverture::paie_du_mois(o, ctx, &data)?
    } else {
        crate::service::calculer_avec(ctx, &json!({ "saisie": data })).map_err(|m| crate::routes_ouverture::refus_du_calcul(ctx, &data, m))?
    };
    let engs = crate::routes_ouverture::engagements_calcules(ctx, &data, &rep)?;
    let paies_json = rep.get("paies").and_then(Value::as_array).ok_or("reponse du calcul illisible : `paies`")?;
    let mut paies = Vec::new();
    for (e, v) in engs.into_iter().zip(paies_json) {
        paies.push(crate::routes_ouverture::paie_de_json(ctx, e, v)?);
    }
    let mois_saisie = data.get("mois").map(crate::date::en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
    let annee: i32 = mois_saisie.chars().take(4).collect::<String>().parse().map_err(|_| erreur("ValueError", format!("mois illisible : {}", crate::fichiers::repr(&mois_saisie))))?;
    crate::routes_ouverture::numeroter(ctx, &mut paies, annee)?;
    Ok((data, paies))
}

/// `apercu_envoi` : ce que chaque salarie recevrait, sans rien envoyer.
fn envoi_apercu(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = py_str(mois_v);
    // LE RATTRAPAGE : une AER relevee avant que la releve ne sache la ranger se range ici.
    if !aer_manquantes(ctx, &mois)?.is_empty() {
        let _ = ranger_aer(o, ctx, Some(&mois), None, false)?;
    }
    let mut m = Moteur::nouveau(o, ctx);
    let prets = documents_prets(&mut m, ctx, &mois)?;
    // `charger(ctx, paies/<mois>.json)` : la saisie du mois, numerotee.
    let (data, paies) = charger_le_mois(o, ctx, &mois)?;
    let dossier = ctx.racine.join("sorties").join(&mois).join("envoi");
    // A QUI ILS PARTENT (ticket #240) : le meme choix que l'envoi (`routes_envois::choisir`).
    let choix = crate::routes_envois::choisir(ctx, &mois, &data, &paies)?;
    let messages = if prets.get("produit").is_some_and(vrai) { preparer(ctx, &paies, &mois, &dossier, &choix.renvois)? } else { Vec::new() };
    let tous_a_jour = messages.iter().all(|x| choix.a_jour.contains(&x.cle));
    let renvoi_voulu = !messages.is_empty() && tous_a_jour;
    let messages = choix.garder(messages, |x| x.cle.as_str());
    let (mut expediteur, mut avertissements) = (String::new(), Vec::<String>::new());
    if prets.get("boite_liee").is_some_and(vrai) {
        // `preparer_envoi` : la saisie rechargee, les messages refaits, le compte de la boite.
        let mois_saisie = data.get("mois").map(crate::date::en_texte).ok_or_else(|| crate::fichiers::absent("mois"))?;
        let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
        if dossier.exists() {
            preparer(ctx, &paies, &mois_saisie, &dossier, &choix.renvois)?;
            expediteur = compte_de_la_boite(ctx)?.to_lowercase();
            let sg = get_requis(&ctx.entreprise, "signataire")?;
            let signataire = sg.get("email").filter(|v| vrai(v)).map(py_str).unwrap_or_default().to_lowercase();
            if !signataire.is_empty() && signataire != expediteur {
                avertissements.push(format!(
                    "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
                    expediteur, signataire
                ));
            }
        }
    }
    let lignes = m.lignes()?;
    let deja = etats::etat_du_mois(&lignes, &mois)?;
    let mut r = Map::new();
    r.insert("mois".into(), mois_v.clone());
    r.insert("expediteur".into(), json!(expediteur));
    r.insert("avertissements".into(), json!(avertissements));
    r.insert(
        "messages".into(),
        Value::Array(
            messages
                .iter()
                .map(|x| {
                    let renvoi = choix.renvois.get(&x.cle);
                    json!({"cle": x.cle, "nom": x.nom, "destinataire": x.destinataire, "sujet": x.sujet,
                           "pieces": x.pieces.iter().map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>(),
                           "octets": x.taille(),
                           // Un renvoi : le jour du premier envoi, et ce que le courriel dira avoir change.
                           "renvoi": renvoi.is_some(),
                           "envoye_le": renvoi.map(|r| r.envoye_le.clone()),
                           "changements": renvoi.map(|r| if r.changements.is_empty() {
                               vec![crate::envoi::RIEN_DANS_LE_RESUME.trim_start_matches([' ', '-']).to_string()]
                           } else {
                               r.changements.iter().map(|l| l.trim_start().trim_start_matches("- ").to_string()).collect()
                           }).unwrap_or_default()})
                })
                .collect(),
        ),
    );
    // « Deja envoye » ne se dit que d'un renvoi voulu, a l'identique : tous ont deja leurs documents, et leur paie n'a
    // pas change. Un renvoi de documents corriges ou un premier envoi n'envoie rien une seconde fois.
    r.insert("deja".into(), if tous_a_jour { get(&deja, "documents")?.clone() } else { Value::Null });
    let renvois = messages.iter().filter(|x| choix.renvois.contains_key(&x.cle)).count();
    let n = messages.len();
    r.insert("renvois".into(), json!(renvois));
    r.insert(
        "bouton".into(),
        json!(if renvois > 0 && renvois == n {
            format!("Renvoyer les documents corrigés à {} salarié{}", n, if n > 1 { "s" } else { "" })
        } else if renvois > 0 {
            format!("Envoyer à {} salarié{} ({} corrigé{} à renvoyer)", n, if n > 1 { "s" } else { "" }, renvois, if renvois > 1 { "s" } else { "" })
        } else if renvoi_voulu {
            format!("Renvoyer à {} salarié{}", n, if n > 1 { "s" } else { "" })
        } else {
            format!("Envoyer à {} salarié{}", n, if n > 1 { "s" } else { "" })
        }),
    );
    r.extend(prets);
    Ok(Value::Object(r))
}

/// `client.compte` : l'adresse de la boite qui enverrait. Celle de la compagnie fictive est le courriel du
/// signataire ; sinon l'adresse posee avec le mot de passe (jamais le mot de passe).
fn compte_de_la_boite(ctx: &Contexte) -> R<String> {
    if crate::routes_accueil::simulation_active(&ctx.racine) {
        let sg = ctx.entreprise.get("signataire").filter(|v| vrai(v));
        let mail = sg.and_then(|s| s.get("email")).filter(|v| vrai(v)).map(py_str);
        return Ok(mail.unwrap_or_else(|| "fictive@puppetpaie.invalid".to_string()));
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let a = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    Ok(a.trim().to_string())
}
