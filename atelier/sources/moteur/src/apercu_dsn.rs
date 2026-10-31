//! `depot-apercu` pour une DSN : ce qui partirait, en clair, sans rien envoyer ni demander -- le fichier lu, son
//! annule-et-remplace, et ce qu'il declare compare aux engagements valides du mois, contrat par contrat.
//!
//! Aucun reseau, aucun mot de passe, aucune ecriture de ce module.
//!
//! Les pieces reprises du reste du moteur : `Moteur::etat_remplacement` (l'annule-et-
//! remplace d'un etablissement), `engagement::etats::validees_du_mois` et
//! `ecrire_documents::paies_aux_numeros_declares` (les paies validees, numerotees), `depot::lire` (la lecture du
//! fichier), `etats::{deja_depose, repetition}` (le journal).
//!
//! Un nombre `NaN`, `Infinity`, ou dont l'exposant sortirait des 28 chiffres de `rust_decimal`, se refuse comme un
//! nombre illisible.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use envois::depot::{self, Apercu};
use envois::etats;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::num::Decimal;
use socle::num::{decimal_lu, entier_lu, q2};

use socle::echec::Echec;
use crate::routes::depots::{lire_octets, nom_du_chemin, resoudre};
use crate::routes::etapes::Moteur;
use documents::ecrire_documents::paies_aux_numeros_declares;
use declarations::recap_dsn::{lire_recap, norm};

type R<T> = Result<T, Echec>;

const TYPES: [(&str, &str); 5] = depot::TYPES;

/// Le décimal en flottant.
fn flottant(d: Decimal) -> f64 {
    d.to_string().parse::<f64>().unwrap_or(0.0)
}

/// La meme, sans le detail du refus : le moindre refus, quel qu'il soit, rend `Err`.
fn saisie_aux_numeros_declares(ctx: &Contexte, mois: &str) -> Result<Vec<Paie>, String> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    paies_aux_numeros_declares(ctx, mois, None, &departs).map(|t| t.1).map_err(|r| r.message)
}

// ------------------------------------------------------------------ recapitulatif_dsn

/// `recapitulatif_dsn` : ce que la DSN declare, contrat par contrat, comparé aux contrats saisis.
fn recapitulatif_dsn(ctx: &Contexte, octets: &[u8], mois: &str, nic: &str) -> Result<Value, String> {
    let sals = lire_recap(octets)?;
    // Un mois sans engagement n'a rien a comparer.
    let (mut compare, liste) = match saisie_aux_numeros_declares(ctx, mois) {
        Ok(l) if !l.is_empty() => (true, l),
        _ => (false, Vec::new()),
    };
    // `paies` : numero -> paie, dans l'ordre ou les numeros sont apparus
    let mut ordre: Vec<String> = Vec::new();
    let mut par_numero: HashMap<String, usize> = HashMap::new();
    for (i, p) in liste.iter().enumerate() {
        if !nic.is_empty() {
            match droit::bareme::etablissement_du_salarie(&ctx.entreprise, &p.salarie) {
                Ok(e) if e == nic => {}
                Ok(_) => continue,
                Err(_) => {
                    compare = false;
                    break;
                }
            }
        }
        let n = p.engagement.numero.clone().unwrap_or_else(|| "None".to_string());
        if !par_numero.contains_key(&n) {
            ordre.push(n.clone());
        }
        par_numero.insert(n, i);
    }
    let mut vus: HashSet<String> = HashSet::new();
    let mut salaries_json = Vec::new();
    let (mut n_contrats, mut n_ecarts, mut n_corrections) = (0usize, 0usize, 0usize);
    for sal in &sals {
        let mut contrats_json = Vec::new();
        // Un contrat se dit par ce qu'il declare pour le mois, comparé a la paie du mois, et par ses rappels d'un mois deja
        // declare (`recap_dsn::Contrat`), qui ne se comparent pas a la paie du mois. Un contrat qui n'a que des rappels
        // n'a pas de ligne du mois.
        for (c, correction) in sal.contrats.iter().flat_map(|c| [(c, false), (c, true)]) {
            let m = if correction { &c.rappel } else { &c.mois };
            if (correction || c.rappel.remunere) && !m.remunere {
                continue;
            }
            vus.insert(c.numero.clone());
            let p = par_numero.get(&c.numero).map(|&i| &liste[i]);
            let mut ecarts: Vec<String> = Vec::new();
            let brut = m.brut_texte();
            match p {
                _ if correction => {}
                None => {
                    if compare {
                        ecarts.push("aucune paie du mois ne porte ce contrat".to_string());
                    }
                }
                Some(p) => {
                    let e = &p.engagement;
                    let mut attendu: HashMap<&str, String> = HashMap::new();
                    // Le brut que la paie declare en 51 de type 001 (`dsn::brut_du_51` : abattu sous deduction forfaitaire
                    // specifique), a deux decimales.
                    attendu.insert("brut", q2(declarations::dsn::brut_du_51(p)));
                    for k in ["jours", "cachets", "heures"] {
                        attendu.insert(k, "0".to_string());
                    }
                    let nat = e.nat()?;
                    for u in nat.unites_activite() {
                        let genre_ok = match u.get("type") {
                            None => true,
                            Some(Value::String(s)) => s == "01",
                            Some(_) => false,
                        };
                        if !genre_ok {
                            continue;
                        }
                        let code = u.get("code").ok_or_else(|| socle::fichiers::absent("code"))?;
                        let Some(cle) = (match code.as_str() {
                            Some("10") | Some("35") => Some("heures"),
                            Some("12") => Some("jours"),
                            Some("37") => Some("cachets"),
                            _ => None,
                        }) else {
                            continue;
                        };
                        let mesure = u.get("mesure").and_then(Value::as_str).ok_or_else(|| socle::fichiers::absent("mesure"))?;
                        let valeur = match mesure {
                            "jours" => e.jours as f64,
                            "cachets" => e.cachets as f64,
                            "heures" => e.heures.unwrap_or(0.0),
                            "heures_travaillees" => flottant(p.heures_travaillees),
                            autre => return Err(format!("mesure inconnue : {}", repr(autre))),
                        };
                        let avant: f64 = attendu[cle].parse().unwrap_or(0.0);
                        let somme = avant + valeur;
                        let t = format!("{}", if somme == 0.0 { 0.0 } else { somme });
                        let d = decimal_lu(&t).ok_or_else(|| socle::num::illisible(&t))?;
                        attendu.insert(cle, norm(d));
                    }
                    for cle in ["brut", "jours", "cachets", "heures"] {
                        let fichier = if cle == "brut" { brut.clone() } else { m.mesure_texte(cle) };
                        let a = decimal_lu(&attendu[cle]).ok_or_else(|| socle::num::illisible(&attendu[cle]))?;
                        let f = decimal_lu(&fichier).ok_or_else(|| socle::num::illisible(&fichier))?;
                        if norm(f) != norm(a) {
                            ecarts.push(format!("{} : la paie dit {}, le fichier {}", cle, attendu[cle], fichier));
                        }
                    }
                    let emploi = p.salarie.get("emploi");
                    let attendu_emploi: Option<&str> = match emploi {
                        None | Some(Value::Null) => Some(""),
                        Some(Value::String(s)) => Some(s.as_str()),
                        Some(v) if !vrai(v) => Some(""),
                        Some(_) => None,
                    };
                    if attendu_emploi != Some(c.emploi.as_str()) {
                        let dit = match emploi {
                            None | Some(Value::Null) => "None".to_string(),
                            Some(v) => socle::valeur::valeur_en_texte(v),
                        };
                        ecarts.push(format!("emploi : la paie dit {}, le fichier {}", dit, c.emploi));
                    }
                }
            }
            if correction {
                n_corrections += 1;
            } else {
                n_contrats += 1;
            }
            if !ecarts.is_empty() {
                n_ecarts += 1;
            }
            contrats_json.push(json!({
                "numero": c.numero, "debut": c.debut, "fin": c.fin, "emploi": c.emploi,
                "nature": c.nature, "objet": c.objet, "brut": brut,
                "jours": m.mesure_texte("jours"), "cachets": m.mesure_texte("cachets"),
                "heures": m.mesure_texte("heures"),
                "fin_declaree": c.fin_declaree, "motif_fin": c.motif_fin, "remunere": m.remunere,
                "paie": p.is_some(), "correction": correction,
                // Un contrat du fichier qui ne dit pas ce que dit la paie : a corriger, la compagnie le peut.
                "pastille_ecarts": if ecarts.is_empty() { Value::Null } else { crate::routes::pastille("a_faire", &ecarts.join(" ; ")) },
                "ecarts": ecarts,
            }));
        }
        let versements: Vec<Value> = sal
            .versements
            .iter()
            .map(|v| json!({"date": v.date, "net_verse": v.net_verse, "net_fiscal": v.net_fiscal, "taux_pas": v.taux_pas, "montant_pas": v.montant_pas}))
            .collect();
        salaries_json.push(json!({"nir": sal.nir, "nom": sal.nom, "prenoms": sal.prenoms, "contrats": contrats_json, "versements": versements}));
    }
    let mut absents = Vec::new();
    for n in &ordre {
        if vus.contains(n) {
            continue;
        }
        let p = &liste[par_numero[n]];
        absents.push(json!({
            "salarie": p.salarie.get("_cle").cloned().ok_or_else(|| socle::fichiers::absent("_cle"))?,
            "nom": socle::texte::salarie_nomme_ou(Some(&p.salarie), "Ce salarié"),
            "numero": n,
            "debut": p.engagement.debut.iso(),
            "fin": p.engagement.fin.map_or(String::new(), |f| f.iso()),
            "pastille": crate::routes::pastille("a_faire", "absent du fichier"),
        }));
    }
    let conforme = compare && absents.is_empty() && n_ecarts == 0;
    let s = |n: usize| if n > 1 { "s" } else { "" };
    let mut titre = format!("{} contrat{} dans ce fichier", n_contrats, s(n_contrats));
    if n_corrections > 0 {
        titre += &format!(" (et {} correction{} d'un mois déjà déclaré)", n_corrections, s(n_corrections));
    }
    if compare {
        titre += &format!(", {} saisi{} dans la paie du mois", par_numero.len(), s(par_numero.len()));
    }
    let phrase = if conforme {
        "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit.".to_string()
    } else if compare {
        let mut p = Vec::new();
        if !absents.is_empty() {
            let n = absents.len();
            p.push(format!("{} contrat{} de la paie manque{} au fichier.", n, s(n), if n > 1 { "nt" } else { "" }));
        }
        if n_ecarts > 0 {
            let dit = if n_ecarts > 1 { "disent" } else { "dit" };
            p.push(format!("{} contrat{} du fichier ne {} pas ce que dit la paie : réécrire le mois.", n_ecarts, s(n_ecarts), dit));
        }
        p.join(" ")
    } else {
        "Pas de paie saisie à comparer : le fichier se lit seul.".to_string()
    };
    Ok(json!({
        "salaries": salaries_json, "absents": absents,
        "titre": titre, "phrase": phrase,
        "compare": compare, "conforme": conforme,
        // La couleur du bilan : conforme, vert ; un ecart ou un absent, orange ; rien a comparer, gris.
        "couleur": if conforme { "vert" } else if compare { "orange" } else { "gris" },
    }))
}

// ------------------------------------------------------------------ l'apercu

/// L'annule-et-remplace du mois, ou `null` s'il ne se lit pas.
fn remplacement_du_fichier(m: &mut Moteur, a: &Apercu, mois: &str) -> R<Value> {
    let nic = a.champ("etablissement");
    let nic = if nic.is_empty() { Value::Null } else { json!(nic) };
    let Ok(mut e) = m.etat_remplacement(mois, &nic) else { return Ok(Value::Null) };
    let genre = a.champ("type_declaration");
    let remplace = if (genre == "03" || genre == "05") && vrai(&e["deposee"]) {
        let ordre = a.champ("ordre");
        let n = if ordre.is_empty() {
            0
        } else {
            entier_lu(ordre).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr(ordre))))?
        };
        n > i128::from(e["dernier_ordre"].as_i64().unwrap_or(0))
    } else {
        false
    };
    e["fichier_remplace"] = json!(remplace);
    Ok(e)
}

/// `apercu_depot_dsn` : ce qui partirait, en clair, sans rien envoyer ni demander.
pub(crate) fn apercu_depot_dsn(ctx: &Contexte, fichier: &Value) -> R<Value> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let a = depot::lire(&contenu);
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let empreinte = envois::journal::empreinte(&contenu);
    let deja = etats::deja_depose(&lignes, "dsn", &empreinte)?;
    let nom = nom_du_chemin(&chemin);
    let signalement = a.est_signalement();
    let mois = a.mois_lisible();
    let remplacement = if signalement { Value::Null } else { remplacement_du_fichier(&mut m, &a, &mois)? };
    let recapitulatif = if signalement {
        Value::Null
    } else {
        match recapitulatif_dsn(ctx, &contenu, &mois, a.champ("etablissement")) {
            Ok(v) => v,
            Err(message) => json!({ "erreur": message }),
        }
    };
    let repetition = etats::repetition(&lignes, "dsn", &empreinte, &nom)?;
    // LES ORDRES DE PAIEMENT EN CLAIR : a qui, pour quoi, combien, pour quand -- ceux de la DSN fabriquee
    // depuis les paies qu'elle couvre (`depart::refabriquer`) ; un fichier qui ne dit plus ce que disent les paies n'en
    // montre aucun.
    let organismes = ctx.du_mois(&mois)?.organismes()?;
    let (ordres, ordres_erreur) = if signalement {
        (Vec::new(), Value::Null)
    } else {
        let nic = a.champ("etablissement");
        let nic = Some(nic).filter(|n| !n.is_empty());
        match crate::compagnie::depart::refabriquer(ctx, std::path::Path::new(&chemin), &mois, nic)? {
            Some(f) => (declarations::ordres_paiement::decrire(&f.ordres, &mois, &organismes, ctx.echeance_dsn()?)?, Value::Null),
            None => (Vec::new(), json!("Ce fichier ne dit plus ce que disent les paies du mois : le refaire (Produire la DSN) avant de le déposer.")),
        }
    };
    let genre = a.champ("type_declaration");
    let nature = a.champ("nature");
    let libelle_type = TYPES.iter().find(|(c, _)| *c == genre).map_or("?", |(_, l)| *l);
    let description = format!(
        "{} — {} (type {}), ordre {}, fraction {}, constituée le {}. {} salarié{}, {} lignes, SIREN {}.",
        nom,
        libelle_type,
        genre,
        a.champ("ordre"),
        a.champ("fraction"),
        en_clair_jjmmaaaa(a.champ("constitution")),
        a.salaries,
        if a.salaries > 1 { "s" } else { "" },
        a.lignes,
        a.champ("siren")
    );
    Ok(json!({
        "fichier": chemin, "nom": nom, "description": description,
        "mois": mois, "nature_envoi": a.champ("nature_envoi"),
        "reel": a.reel(),
        "signalement": signalement,
        "nature": if nature.is_empty() { depot::MENSUELLE } else { nature },
        "libelle_nature": a.nature_lisible(),
        "evenement": if signalement { a.evenement_lisible() } else { String::new() },
        "type_declaration": genre,
        "libelle_type": libelle_type,
        "ordre": a.champ("ordre"), "fraction": a.champ("fraction"),
        "remplace": a.champ("remplace"), "constitution": en_clair_jjmmaaaa(&a.champ("constitution")),
        "siren": a.champ("siren"), "salaries": a.salaries, "lignes": a.lignes,
        "pas_versements": a.pas_versements, "pas_bareme": a.pas_bareme,
        "ordres": ordres, "ordres_erreur": ordres_erreur,
        // Un depot reel les declenche tous : a regarder (orange) ; joues, rien ne part (une information).
        "couleur_ordres": crate::routes::couleur_de_l_etat(if crate::compagnie::bridage::branchement(&ctx.racine).joue() { "information" } else { "a_faire" }),
        "deja_depose": deja,
        "remplacement": remplacement,
        "recapitulatif": recapitulatif,
        "repetition": repetition,
    }))
}

/// Une date de la norme (JJMMAAAA, S20.G00.05.007) telle qu'on la lit : JJ/MM/AAAA. Autre chose se montre tel quel.
fn en_clair_jjmmaaaa(d: &str) -> String {
    if d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}/{}/{}", &d[..2], &d[2..4], &d[4..])
    } else {
        d.to_string()
    }
}
