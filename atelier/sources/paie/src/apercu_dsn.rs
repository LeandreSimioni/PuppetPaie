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
use crate::depot::{self, Apercu};
use envois::etats;
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};
use socle::num::Decimal;
use socle::num::{decimal_py, int_py, q2};
use socle::texte::{blanc, lignes_py, texte_latin1};
use socle::echec::Echec;
use crate::routes_apercus::{lire_octets, nom_du_chemin, resoudre};
use crate::routes_mois::Moteur;
use documents::ecrire_documents::paies_aux_numeros_declares;

type R<T> = Result<T, Echec>;

const TYPES: [(&str, &str); 5] = depot::TYPES;

// ------------------------------------------------------------------ les petits outils de Python

/// `format(d.normalize(), "f")`, avec `0` pour zero.
fn norm(d: Decimal) -> String {
    if d.is_zero() {
        "0".to_string()
    } else {
        d.normalize().to_string()
    }
}

/// `float(d)`.
fn flottant(d: Decimal) -> f64 {
    d.to_string().parse::<f64>().unwrap_or(0.0)
}

// ------------------------------------------------------------------ dsn_recap.lire

/// `dsn_recap._date` : JJMMAAAA -> AAAA-MM-JJ.
fn date_dsn(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 {
        let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
    } else {
        v.to_string()
    }
}

#[derive(Default)]
pub(crate) struct Contrat {
    pub(crate) numero: String,
    pub(crate) debut: String,
    pub(crate) fin: String,
    pub(crate) emploi: String,
    pub(crate) nature: String,
    pub(crate) objet: String,
    pub(crate) brut: Decimal,
    pub(crate) jours: Decimal,
    pub(crate) cachets: Decimal,
    pub(crate) heures: Decimal,
    pub(crate) fin_declaree: String,
    pub(crate) motif_fin: String,
    pub(crate) remunere: bool,
}

#[derive(Default)]
pub(crate) struct Versement {
    pub(crate) date: String,
    pub(crate) net_verse: String,
    pub(crate) net_fiscal: String,
    pub(crate) taux_pas: String,
    pub(crate) montant_pas: String,
}

#[derive(Default)]
pub(crate) struct Individu {
    pub(crate) nir: String,
    pub(crate) nom: String,
    pub(crate) prenoms: String,
    pub(crate) contrats: Vec<Contrat>,
    pub(crate) versements: Vec<Versement>,
    pub(crate) par_numero: HashMap<String, (usize, usize)>,
}

/// Les sorties de `dsn_recap.lire` sous la forme que le recapitulatif complete : les montants
/// en texte, comme le Python les rend.
impl Contrat {
    fn brut_texte(&self) -> String {
        q2(self.brut)
    }
    fn mesure_texte(&self, cle: &str) -> String {
        norm(match cle {
            "jours" => self.jours,
            "cachets" => self.cachets,
            _ => self.heures,
        })
    }
}

/// `dsn_recap.lire` : les salaries du fichier, leurs contrats, leurs versements.
pub(crate) fn lire_recap(octets: &[u8]) -> Result<Vec<Individu>, String> {
    let texte = texte_latin1(octets);
    let mut sals: Vec<Individu> = Vec::new();
    let mut contrat: Option<(usize, usize)> = None;
    let mut versement: Option<(usize, usize)> = None;
    let mut fin: Option<(usize, usize)> = None;
    // (numero de contrat, type) de la remuneration en cours
    let mut remu: Option<(String, String)> = None;
    // (type, mesure) de l'activite en cours
    let mut activite: Option<(String, Decimal)> = None;
    for ligne in lignes_py(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(blanc).trim_matches('\'');
        let kc: Vec<char> = k.chars().collect();
        let bloc: String = kc.iter().take(10).collect();
        let champ: String = kc.iter().skip(11).collect();
        if bloc == "S21.G00.30" {
            if champ == "001" || sals.is_empty() {
                sals.push(Individu::default());
            }
            let s = sals.last_mut().unwrap();
            match champ.as_str() {
                "001" => s.nir = v.to_string(),
                "002" => s.nom = v.to_string(),
                "004" => s.prenoms = v.to_string(),
                _ => {}
            }
            continue;
        }
        if sals.is_empty() {
            continue;
        }
        let si = sals.len() - 1;
        match bloc.as_str() {
            "S21.G00.40" => {
                if champ == "001" {
                    sals[si].contrats.push(Contrat { debut: date_dsn(v), ..Contrat::default() });
                    contrat = Some((si, sals[si].contrats.len() - 1));
                } else if let Some((cs, cc)) = contrat {
                    match champ.as_str() {
                        "006" => sals[cs].contrats[cc].emploi = v.to_string(),
                        "007" => sals[cs].contrats[cc].nature = v.to_string(),
                        "009" => {
                            sals[cs].contrats[cc].numero = v.to_string();
                            sals[si].par_numero.insert(v.to_string(), (cs, cc));
                        }
                        "010" => sals[cs].contrats[cc].fin = date_dsn(v),
                        "050" => sals[cs].contrats[cc].objet = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.62" => {
                if champ == "001" && contrat.is_some() {
                    fin = contrat;
                    let (fs, fc) = fin.unwrap();
                    sals[fs].contrats[fc].fin_declaree = date_dsn(v);
                } else if champ == "002" {
                    if let Some((fs, fc)) = fin {
                        sals[fs].contrats[fc].motif_fin = v.to_string();
                    }
                }
            }
            "S21.G00.50" => {
                if champ == "001" {
                    sals[si].versements.push(Versement { date: date_dsn(v), ..Versement::default() });
                    versement = Some((si, sals[si].versements.len() - 1));
                } else if let Some((vs, vi)) = versement {
                    let x = &mut sals[vs].versements[vi];
                    match champ.as_str() {
                        "002" => x.net_fiscal = v.to_string(),
                        "004" => x.net_verse = v.to_string(),
                        "006" => x.taux_pas = v.to_string(),
                        "009" => x.montant_pas = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.51" => {
                if champ == "001" {
                    remu = Some((String::new(), String::new()));
                } else if let Some(r) = remu.as_mut() {
                    match champ.as_str() {
                        "010" => r.0 = v.to_string(),
                        "011" => r.1 = v.to_string(),
                        "013" => {
                            let montant = decimal_py(v).ok_or_else(|| socle::num::illisible(v))?;
                            let (numero, genre) = (r.0.clone(), r.1.clone());
                            if let Some(&(cs, cc)) = sals[si].par_numero.get(&numero) {
                                let c = &mut sals[cs].contrats[cc];
                                c.remunere = true;
                                if genre == "001" {
                                    c.brut += montant;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "S21.G00.53" => {
                if champ == "001" {
                    activite = Some((v.to_string(), Decimal::ZERO));
                } else if activite.is_some() && champ == "002" {
                    activite.as_mut().unwrap().1 = decimal_py(v).ok_or_else(|| socle::num::illisible(v))?;
                } else if activite.is_some() && champ == "003" {
                    let (genre, mesure) = activite.clone().unwrap();
                    let c = remu.as_ref().and_then(|r| sals[si].par_numero.get(&r.0).copied());
                    let unite = match v {
                        "10" | "35" => Some("heures"),
                        "12" => Some("jours"),
                        "37" => Some("cachets"),
                        _ => None,
                    };
                    if let (Some((cs, cc)), Some(u)) = (c, unite) {
                        if genre == "01" {
                            let c = &mut sals[cs].contrats[cc];
                            match u {
                                "jours" => c.jours += mesure,
                                "cachets" => c.cachets += mesure,
                                _ => c.heures += mesure,
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(sals)
}

/// La meme, sans le detail du refus : le moindre refus, quel qu'il soit, rend `Err`.
fn saisie_aux_numeros_declares(ctx: &Contexte, mois: &str) -> Result<Vec<Paie>, String> {
    paies_aux_numeros_declares(ctx, mois, None).map(|t| t.1).map_err(|r| r.message)
}

// ------------------------------------------------------------------ recapitulatif_dsn

fn requis(v: &Value, cle: &str) -> Result<String, String> {
    v.get(cle).map(socle::valeur::valeur_en_texte).ok_or_else(|| socle::fichiers::absent(cle))
}

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
        for c in &sal.contrats {
            vus.insert(c.numero.clone());
            let p = par_numero.get(&c.numero).map(|&i| &liste[i]);
            let mut ecarts: Vec<String> = Vec::new();
            let brut = c.brut_texte();
            // UNE CORRECTION D'UN MOIS DEJA DECLARE (21/10/2026) : un contrat que la paie du mois ne porte pas, et dont la
            // fin (40.010) precede le mois, n'est dans le fichier que pour sa regularisation (la correction d'un mois suivant,
            // `dsn::generer` et ses `regs`). Ce n'est pas un ecart avec la paie : il se dit pour ce qu'il est.
            let correction = p.is_none() && !c.fin.is_empty() && c.fin.as_str() < format!("{}-01", mois).as_str();
            match p {
                None if correction => {}
                None => {
                    if compare {
                        ecarts.push("aucune paie du mois ne porte ce contrat".to_string());
                    }
                }
                Some(p) => {
                    let e = &p.engagement;
                    let mut attendu: HashMap<&str, String> = HashMap::new();
                    // `paie_rust._euros` : le brut de la paie est quantifie a deux decimales.
                    attendu.insert("brut", q2(p.brut));
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
                        let d = decimal_py(&t).ok_or_else(|| socle::num::illisible(&t))?;
                        attendu.insert(cle, norm(d));
                    }
                    for cle in ["brut", "jours", "cachets", "heures"] {
                        let fichier = if cle == "brut" { brut.clone() } else { c.mesure_texte(cle) };
                        let a = decimal_py(&attendu[cle]).ok_or_else(|| socle::num::illisible(&attendu[cle]))?;
                        let f = decimal_py(&fichier).ok_or_else(|| socle::num::illisible(&fichier))?;
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
                "jours": c.mesure_texte("jours"), "cachets": c.mesure_texte("cachets"),
                "heures": c.mesure_texte("heures"),
                "fin_declaree": c.fin_declaree, "motif_fin": c.motif_fin, "remunere": c.remunere,
                "paie": p.is_some(), "ecarts": ecarts, "correction": correction,
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
        let nom = requis(&p.salarie, "nom")?;
        let prenom = match p.salarie.get("prenom_affichage") {
            Some(v) if vrai(v) => socle::valeur::valeur_en_texte(v),
            _ => requis(&p.salarie, "prenom")?,
        };
        absents.push(json!({
            "salarie": p.salarie.get("_cle").cloned().ok_or_else(|| socle::fichiers::absent("_cle"))?,
            "nom": format!("{} {}", nom, prenom),
            "numero": n,
            "debut": p.engagement.debut.iso(),
            "fin": p.engagement.fin.map_or(String::new(), |f| f.iso()),
        }));
    }
    let conforme = compare && absents.is_empty() && n_ecarts == 0;
    Ok(json!({
        "salaries": salaries_json, "absents": absents,
        "contrats": n_contrats, "corrections": n_corrections, "contrats_saisis": par_numero.len(),
        "compare": compare, "conforme": conforme,
    }))
}

// ------------------------------------------------------------------ l'apercu

/// `_remplacement_du_fichier` : l'annule-et-remplace du mois, ou `null` s'il ne se lit pas.
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
            int_py(ordre).ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", repr(ordre))))?
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
    // LES ORDRES DE PAIEMENT EN CLAIR (ticket #223) : a qui, pour quoi, combien, pour quand -- ceux de la DSN fabriquee
    // depuis les paies qu'elle couvre (`depart::refabriquer`) ; un fichier qui ne dit plus ce que disent les paies n'en
    // montre aucun.
    let organismes = ctx.du_mois(&mois)?.organismes()?.unwrap_or(Value::Null);
    let (ordres, ordres_erreur) = if signalement {
        (Vec::new(), Value::Null)
    } else {
        let nic = a.champ("etablissement");
        let nic = Some(nic).filter(|n| !n.is_empty());
        match crate::depart::refabriquer(ctx, std::path::Path::new(&chemin), &mois, nic)? {
            Some(f) => (declarations::ordres_paiement::decrire(&f.ordres, &mois, &organismes, ctx.echeance_dsn()?)?, Value::Null),
            None => (Vec::new(), json!("Ce fichier ne dit plus ce que disent les paies du mois : le refaire (Produire la DSN) avant de le déposer.")),
        }
    };
    let genre = a.champ("type_declaration");
    let nature = a.champ("nature");
    Ok(json!({
        "fichier": chemin, "nom": nom,
        "mois": mois, "nature_envoi": a.champ("nature_envoi"),
        "reel": a.reel(),
        "signalement": signalement,
        "nature": if nature.is_empty() { depot::MENSUELLE } else { nature },
        "libelle_nature": a.nature_lisible(),
        "evenement": if signalement { a.evenement_lisible() } else { String::new() },
        "type_declaration": genre,
        "libelle_type": TYPES.iter().find(|(c, _)| *c == genre).map_or("?", |(_, l)| *l),
        "ordre": a.champ("ordre"), "fraction": a.champ("fraction"),
        "remplace": a.champ("remplace"), "constitution": a.champ("constitution"),
        "siren": a.champ("siren"), "salaries": a.salaries, "lignes": a.lignes,
        "pas_versements": a.pas_versements, "pas_bareme": a.pas_bareme,
        "ordres": ordres, "ordres_erreur": ordres_erreur,
        "deja_depose": deja,
        "remplacement": remplacement,
        "recapitulatif": recapitulatif,
        "repetition": repetition,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recapitulatif_lu_dans_le_fichier() {
        let f = "S21.G00.30.001,'1850'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.5'\nS21.G00.51.013,'49.5'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\nS21.G00.62.001,'31082026'\n";
        let s = lire_recap(f.as_bytes()).unwrap();
        assert_eq!(s.len(), 1);
        let c = &s[0].contrats[0];
        assert_eq!((c.brut_texte().as_str(), c.mesure_texte("cachets").as_str()), ("200.00", "2"));
        assert_eq!((c.debut.as_str(), c.fin_declaree.as_str(), c.remunere), ("2026-08-01", "2026-08-31", true));
        assert_eq!(lire_recap(b"S21.G00.30.001,'1'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'A'\nS21.G00.51.001,'x'\nS21.G00.51.010,'A'\nS21.G00.51.013,'12,5'\n").err(), Some(socle::num::illisible("12,5")));
    }
}
