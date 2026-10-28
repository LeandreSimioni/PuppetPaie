//! Les apercus des documents du mois : `signature-apercu`, `avenant-apercu`,
//! `envoi-apercu` -- ce que la page montre AVANT qu'on envoie un contrat, un avenant ou les
//! documents du mois.
//!
//! Aucun reseau, aucun mot de passe. La seule ecriture de ce module : le rangement des AER revenues (`ranger_aer`) dans
//! `sorties/<mois>/envoi/`, pour `envoi-apercu`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use envois::etats::{self, est, get, getd, items, objet_vide, LISTE_VIDE, SIGNATURE};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use crate::routes_mois::{dans_l_ensemble, premier, Moteur, SaisieMois};

type R<T> = Result<T, Echec>;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["signature-apercu", "avenant-apercu", "envoi-apercu"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "signature-apercu" => signature_apercu(ctx, charge),
        "avenant-apercu" => avenant_apercu(ctx, charge),
        "envoi-apercu" => envoi_apercu(ctx, charge),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

/// `charge["cle"]` : `KeyError` si elle manque.
fn cle<'a>(charge: &'a Value, k: &str) -> R<&'a Value> {
    charge.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

/// `Path.glob("<prefixe>*<suffixe>")` sur un dossier : y a-t-il un nom qui s'y accorde ? Un dossier
/// qui n'en est pas un, ou qu'on ne peut lire, ne rend rien.
pub(crate) fn un_nom_commence_et_finit(dossier: &Path, prefixe: &str, suffixe: &str) -> bool {
    let Ok(lecture) = std::fs::read_dir(dossier) else { return false };
    lecture.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(suffixe)
    })
}

fn nul() -> &'static Value {
    static V: Value = Value::Null;
    &V
}

// ------------------------------------------------------------------ signature-apercu

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
    let avenants = crate::routes_mois::avenants_a_faire(ctx, &mois)?;
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
        let etat = crate::routes_mois::etat_signature(
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
        // CE QUE LA BOITE EN FAIT : un contrat a envoyer (jamais parti, ou caduc) se coche ; un contrat parti et pas
        // revenu attend, et seul celui parti par courriel se relance.
        let parti = l["demande_le"].clone();
        let (recu, caduc) = (vrai(&etat["recu"]), vrai(&etat["caduc"]));
        let en_attente = vrai(&parti) && !recu && !caduc;
        l.insert("par".into(), etat["par"].clone());
        l.insert("a_envoyer".into(), json!(caduc || (!recu && !vrai(&parti))));
        l.insert("en_attente".into(), json!(en_attente));
        l.insert("relancer".into(), json!(en_attente && est(&etat["par"], "courriel")));
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
    d.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)))
}

// ------------------------------------------------------------------ avenant-apercu

/// `jj_mm_aaaa(x)` : `JJ/MM/AAAA`, `ValueError` si `x` ne se lit pas comme une date.
fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(socle::date::d(v).map_err(|m| erreur("ValueError", m))?.jj_mm_aaaa())
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
            format!("Aucun contrat de {} commençant le {} dans la paie de {}.", salarie, jj_mm_aaaa(debut_v)?, mois_parti),
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
        "destinataire": sal.get("email").cloned().unwrap_or_else(|| json!("")),
    }))
}

// ------------------------------------------------------------------ envoi-apercu

/// `int(texte)` d'un morceau de `mois` : `ValueError` sinon.
fn entier(t: &str) -> R<i64> {
    let s = t.trim();
    let (neg, chiffres) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if chiffres.is_empty() || !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return Err(erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(t))));
    }
    let n: i64 = chiffres.parse().unwrap_or(i64::MAX);
    Ok(if neg { -n } else { n })
}

/// `(int(mois[:4]), int(mois[5:7]))`.
fn an_et_mois(mois: &str) -> R<(i64, i64)> {
    let tranche = |a: usize, b: usize| -> String { mois.chars().skip(a).take(b - a).collect() };
    Ok((entier(&tranche(0, 4))?, entier(&tranche(5, 7))?))
}

/// Le nom du mois `mo` (`date::nom_du_mois`) ; hors 1-12, `IndexError` (l'indice negatif du Python lisait « 00 »
/// decembre : il se refuse).
fn nom_du_mois(mo: i64, g: socle::date::Graphie) -> R<&'static str> {
    socle::date::nom_du_mois(mo, g).map_err(|e| erreur("IndexError", e))
}

/// `envoi.etiquette` : `2026-08` -> `Aout2026`.
fn etiquette(mois: &str) -> R<String> {
    let (an, mo) = an_et_mois(mois)?;
    Ok(format!("{}{}", nom_du_mois(mo, socle::date::Graphie::Titre)?, an))
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
            let champ = |c: &str| v.get(c).map_or("".to_string(), valeur_en_texte);
            json!([cle, format!("{} {}", champ("nom"), champ("prenom"))])
        })
        .collect();
    let mut vues: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
    for nom in fichiers {
        let chemin = dossier.join(&nom);
        let octets = crate::routes_apercus::lire_octets(&chemin.to_string_lossy())?;
        let atts = envois::retours::aer_spectacle(&octets)?;
        let liste = atts.as_array().cloned().unwrap_or_default();
        let cles = if liste.is_empty() {
            Value::Array(Vec::new())
        } else {
            let quis: Vec<Value> = liste.iter().map(|a| a["qui"].clone()).collect();
            envois::retours::rattacher(&quis, &index)
        };
        let mut de_ce_retour: HashMap<(String, String), Vec<Attestation>> = HashMap::new();
        for (a, c) in liste.iter().zip(cles.as_array().map(Vec::as_slice).unwrap_or(&[])) {
            let periode = valeur_en_texte(&a["periode"]);
            if mois.is_some_and(|m| !m.is_empty()) && Some(periode.as_str()) != mois {
                continue;
            }
            let qui = valeur_en_texte(&a["qui"]);
            let donnees = socle::base64::lire(a["donnees"].as_str().unwrap_or("")).map_err(Echec::from)?;
            de_ce_retour.entry((periode, qui.clone())).or_default().push(Attestation {
                nom_archive: valeur_en_texte(&a["nom_archive"]),
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

/// Les paies de `periode`, sous les numeros de contrat que la DSN a DECLARES (regle 4 : un numero se lit --
/// `ecrire_documents::paies_aux_numeros_declares`), sans rien dire de ce qui les fait echouer (vide).
fn paies_de_la_periode(ctx: &Contexte, periode: &str) -> Vec<engagement::calcul::Paie> {
    documents::ecrire_documents::paies_aux_numeros_declares(ctx, periode, None).map(|t| t.1).unwrap_or_default()
}

/// `mois_suivant("2026-12")` = `"2027-01"`.
fn mois_suivant(m: &str) -> String {
    let (a, mo): (i32, u32) = (m.get(..4).and_then(|x| x.parse().ok()).unwrap_or(0), m.get(5..7).and_then(|x| x.parse().ok()).unwrap_or(12));
    if mo >= 12 {
        format!("{:04}-01", a + 1)
    } else {
        format!("{:04}-{:02}", a, mo + 1)
    }
}

/// Toutes les periodes que l'AER de `cle` pour `periode` doit porter (`aer::ecarts`) : chaque contrat dont l'AER est due
/// ce mois-ci (`aer::due`), mois par mois depuis son debut, chacun lu dans la paie de son mois. `None` : la saisie de la
/// periode n'est pas dans le logiciel (un mois d'avant lui) -- rien a quoi comparer.
fn periodes_attendues(ctx: &Contexte, par_mois: &mut HashMap<String, Vec<engagement::calcul::Paie>>, periode: &str, cle: &str) -> R<Option<Vec<documents::aer::PaieAer>>> {
    if !engagement::stock::a_une_saisie(&ctx.racine, periode).unwrap_or(false) {
        return Ok(None);
    }
    let mut du_mois = |m: &str| -> Vec<engagement::calcul::Paie> {
        par_mois.entry(m.to_string()).or_insert_with(|| paies_de_la_periode(ctx, m)).clone()
    };
    let de_lui = |p: &engagement::calcul::Paie| p.salarie.get("_cle").map(valeur_en_texte).as_deref() == Some(cle);
    let mut out = Vec::new();
    for p in du_mois(periode).iter().filter(|p| de_lui(p)) {
        let e = &p.engagement;
        if !documents::aer::due(e).map_err(Echec::from)? {
            continue;
        }
        let numero = e.numero.clone().unwrap_or_default();
        let mut m = e.debut.iso()[..7].to_string();
        while m.as_str() < periode {
            for q in du_mois(&m).iter().filter(|q| de_lui(q) && q.engagement.numero.as_deref() == Some(numero.as_str()) && !q.engagement.sans_paie()) {
                out.push(documents::aer::PaieAer::du_mois(&q.engagement).map_err(Echec::from)?);
            }
            m = mois_suivant(&m);
        }
        if !e.sans_paie() {
            out.push(documents::aer::PaieAer::du_mois(e).map_err(Echec::from)?);
        }
    }
    Ok(Some(out))
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
    let dossier = documents::bulletin::gabarits_de(ent)?.join("identite");
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
pub(crate) fn ranger_aer(ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>) -> R<Vec<String>> {
    let fait = ranger_aer_detail(ctx, mois, fichiers, false)?;
    let mut phrases: Vec<String> = Vec::new();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    for (qui, chemin) in &fait.a_ranger {
        let refuge = chemin.parent().map(|p| p.display().to_string()).unwrap_or_default();
        phrases.push(format!("Attestation France Travail de {} : aucun salarie ne porte ce nom. Deposee dans {} ; verifier la fiche, puis `moteur aer`.", qui, refuge));
    }
    for (cle, periode, ecarts) in &fait.refusees {
        phrases.push(format!("Attestation France Travail de {} ({}) NON SIGNEE : elle ne dit pas ce que la paie dit ({}). La correction se fait sur la DSN, pas sur le PDF.", cle, periode, ecarts.join("; ")));
    }
    Ok(phrases)
}

/// `ranger_aer` du Python, structure : le rangement et ce qu'il a laisse. `forcer` : signer malgre un ecart (`aer --forcer`).
pub(crate) fn ranger_aer_detail(ctx: &Contexte, mois: Option<&str>, fichiers: Option<&[std::path::PathBuf]>, forcer: bool) -> R<RangementAer> {
    use documents::aer::{authentifier, Authentification, Echec as AerEchec};
    let vues = attestations_archivees(ctx, mois, fichiers)?;
    let mut par_mois: HashMap<String, Vec<engagement::calcul::Paie>> = HashMap::new();
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
        // LA VERIFICATION SE FAIT SUR TOUTES LES ATTESTATIONS DU SALARIE A LA FOIS : au-dela de cinq periodes, France
        // Travail en edite plusieurs, et chacune ne porte qu'une partie des periodes attendues.
        let mut imprime: Vec<documents::aer::Ligne> = Vec::new();
        for att in groupe {
            let champs = documents::aer::champs(&att.donnees).map_err(|e| match e {
                AerEchec::Fabrique(m) | AerEchec::Autre(m) => Echec::from(m),
                AerEchec::Ecarts(e) => Echec::from(e.join("; ")),
            })?;
            imprime.extend(documents::aer::lire_quantites(&champs).map_err(Echec::from)?);
        }
        let ecarts = match periodes_attendues(ctx, &mut par_mois, periode, &cle)? {
            Some(attendues) => documents::aer::ecarts(&imprime, &attendues),
            None => Vec::new(),
        };
        if !ecarts.is_empty() && !forcer {
            // Une seule fausse et rien ne part.
            fait.refusees.push((cle, periode.clone(), ecarts));
            continue;
        }
        let signature = documents::bulletin::identite_de(&ctx.entreprise_et_gabarits(), "signature").map(|f| f.to_string_lossy().into_owned());
        let cachet = cachet_pose(&ctx.entreprise_et_gabarits());
        let mut signees: Vec<Vec<u8>> = Vec::new();
        for att in groupe {
            let a = Authentification {
                ent: &ctx.entreprise,
                aujourdhui: ctx.aujourdhui,
                signature: signature.as_deref(),
                // Deja verifiee ci-dessus, sur l'ensemble des attestations du salarie.
                paies: None,
                forcer,
                cachet_fichier: cachet.as_deref(),
            };
            match authentifier(&att.donnees, &a) {
                Ok(octets) => signees.push(octets),
                Err(AerEchec::Ecarts(e)) => return Err(Echec::from(e.join("; "))),
                Err(AerEchec::Fabrique(m)) | Err(AerEchec::Autre(m)) => return Err(Echec::from(m)),
            }
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

/// Une AER Cinema Spectacle est-elle due a l'un des salaries du mois (`aer::due`) ? Sans elle, France Travail n'en
/// publie aucune (pas de retour 46) : le mois d'un permanent seul n'a rien a attendre.
fn aer_attendue(paies: &[engagement::calcul::Paie]) -> R<bool> {
    for p in paies {
        if documents::aer::due(&p.engagement).map_err(Echec::from)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `documents_prets` : peut-on envoyer les documents du mois ? Le moteur repond, et dit pourquoi.
fn documents_prets(m: &mut Moteur, ctx: &Contexte, mois: &str, paies: &[engagement::calcul::Paie]) -> R<Map<String, Value>> {
    let dossier = ctx.racine.join("sorties").join(mois).join("envoi");
    // LES DOCUMENTS PARTENT DES QU'UN ENGAGEMENT EST VALIDE : ils se fabriquent a leur envoi, depuis sa paie ecrite.
    let mut empechements: Vec<String> = Vec::new();
    if paies.is_empty() {
        empechements.push("Aucun engagement valide ce mois-ci : rien ne part d'un engagement en saisie.".to_string());
    }
    // L'AER NE SE FABRIQUE PAS : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
    let mut informations: Vec<String> = Vec::new();
    let e = m.dsn_du_mois(mois)?;
    if vrai(&e) && e.get("passee").is_some_and(vrai) && !e.get("aer_recue").is_some_and(vrai) && aer_attendue(paies)? {
        informations.push(
            "L'attestation employeur (AER) de France Travail n'est pas encore revenue : France Travail l'édite à partir de la DSN, et chaque relevé des retours la cherche. Elle partira avec un envoi suivant."
                .to_string(),
        );
    }
    let manquent = aer_manquantes(ctx, mois)?;
    if !manquent.is_empty() {
        informations.push(format!(
            "L'attestation employeur (AER) est revenue de France Travail, mais elle n'est pas signee pour {} : la paie ne confirme pas ce qu'elle affiche, ou le nom n'est pas reconnu. Voir les messages de l'ouverture.",
            manquent.join(", ")
        ));
    }
    let mut r = Map::new();
    r.insert("pret".into(), json!(empechements.is_empty()));
    r.insert("empechements".into(), json!(empechements));
    r.insert("informations".into(), json!(informations));
    r.insert("boite_liee".into(), json!(crate::routes_accueil::boite_liee(ctx)?));
    r.insert("dossier".into(), json!(dossier.to_string_lossy()));
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
fn preparer(ctx: &Contexte, paies: &[engagement::calcul::Paie], mois: &str, dossier: &Path, renvois: &HashMap<String, envois::envoi::Renvoi>) -> R<Vec<Message>> {
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
        let connues: Vec<String> = salaries.iter().map(|s| s.get("_cle").map_or("None".to_string(), valeur_en_texte)).collect();
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
        let cle = valeur_en_texte(get_requis(s, "_cle")?);
        if vus.contains(&cle) {
            continue;
        }
        vus.push(cle.clone());
        if !s.get("email").is_some_and(vrai) {
            return Err(erreur(
                "ValueError",
                format!(
                    "Pas d'adresse e-mail pour {} {} : completer config/salaries.json avant d'envoyer.",
                    valeur_en_texte(get_requis(s, "nom")?),
                    valeur_en_texte(get_requis(s, "prenom")?)
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
            capitaliser(nom_du_mois(mo, socle::date::Graphie::Accentuee)?),
            an,
            if renvois.contains_key(&cle) { "documents corrigés : " } else { "" },
            match courts.len() {
                0 => "documents".to_string(),
                1 => courts[0].to_string(),
                n => format!("{} et {}", courts[..n - 1].join(", "), courts[n - 1]),
            }
        );
        messages.push(Message {
            nom: format!("{} {}", valeur_en_texte(get_requis(s, "prenom_affichage")?), valeur_en_texte(get_requis(s, "nom")?)),
            destinataire: valeur_en_texte(get_requis(s, "email")?).to_lowercase(),
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

/// Ce qui peut partir de `mois` (`engagement::etats::validees_du_mois` : la saisie des engagements valides et leurs paies
/// ecrites), numerote. L'envoi des documents (`routes_envois`) prepare ses messages sur les memes paies que l'apercu.
pub(crate) fn charger_le_mois(ctx: &Contexte, mois: &str) -> R<(Value, Vec<engagement::calcul::Paie>)> {
    let Some((data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois)? else {
        return Ok((json!({ "mois": mois, "engagements": [] }), Vec::new()));
    };
    let annee: i32 = mois.chars().take(4).collect::<String>().parse().map_err(|_| erreur("ValueError", format!("mois illisible : {}", socle::fichiers::repr(mois))))?;
    documents::ecrire_documents::numeroter(ctx, &mut paies, annee)?;
    Ok((data, paies))
}

/// `apercu_envoi` : ce que chaque salarie recevrait, sans rien envoyer.
fn envoi_apercu(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois_v = cle(charge, "mois")?;
    let mois = valeur_en_texte(mois_v);
    // LE RATTRAPAGE : une AER relevee avant que la releve ne sache la ranger se range ici.
    if !aer_manquantes(ctx, &mois)?.is_empty() {
        let _ = ranger_aer(ctx, Some(&mois), None)?;
    }
    // Ce qui peut partir du mois, numerote.
    let (data, paies) = charger_le_mois(ctx, &mois)?;
    let mut m = Moteur::nouveau(ctx);
    let prets = documents_prets(&mut m, ctx, &mois, &paies)?;
    let dossier = ctx.racine.join("sorties").join(&mois).join("envoi");
    // A QUI ILS PARTENT (ticket #240) : le meme choix que l'envoi (`routes_envois::choisir`).
    let choix = crate::routes_envois::choisir(ctx, &mois, &paies)?;
    // CE QUI PARTIRA, FABRIQUE TEL QUEL pour etre montre : leurs numeros ne s.ecrivent qu.a l.envoi (`routes_envois`).
    let messages = if prets.get("pret").is_some_and(vrai) {
        let mut cles: Vec<String> = Vec::new();
        for p in &paies {
            let c = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
            if !cles.contains(&c) {
                cles.push(c);
            }
        }
        let cles = choix.garder(cles, |c| c.as_str());
        crate::depart::fabriquer_documents(ctx, &mois, &cles, "interface")?;
        preparer(ctx, &paies, &mois, &dossier, &choix.renvois)?
    } else {
        Vec::new()
    };
    let tous_a_jour = messages.iter().all(|x| choix.a_jour.contains(&x.cle));
    let renvoi_voulu = !messages.is_empty() && tous_a_jour;
    let messages = choix.garder(messages, |x| x.cle.as_str());
    let (mut expediteur, mut avertissements) = (String::new(), Vec::<String>::new());
    if prets.get("boite_liee").is_some_and(vrai) {
        // `preparer_envoi` : la saisie rechargee, les messages refaits, le compte de la boite.
        let mois_saisie = data.get("mois").map(socle::date::en_texte).ok_or_else(|| socle::fichiers::absent("mois"))?;
        let dossier = ctx.racine.join("sorties").join(&mois_saisie).join("envoi");
        if dossier.exists() {
            preparer(ctx, &paies, &mois_saisie, &dossier, &choix.renvois)?;
            expediteur = compte_de_la_boite(ctx)?.to_lowercase();
            let sg = get_requis(&ctx.entreprise, "signataire")?;
            let signataire = sg.get("email").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().to_lowercase();
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
                               vec![envois::envoi::RIEN_DANS_LE_RESUME.trim_start_matches([' ', '-']).to_string()]
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
    if crate::bridage::branchement(&ctx.racine).joue() {
        let sg = ctx.entreprise.get("signataire").filter(|v| vrai(v));
        let mail = sg.and_then(|s| s.get("email")).filter(|v| vrai(v)).map(valeur_en_texte);
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
