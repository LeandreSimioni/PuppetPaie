//! LA NATURE DES CONTRATS QUE LES BULLETINS N'ONT PAS DITE (#236). L'import d'un bulletin de paie atteste un contrat
//! (`historique/engagements.json`, `importation_bulletins::engagement`) ; aucun gabarit n'imprime sa nature. Un bulletin
//! qui porte une marque du spectacle (numero d'objet, cotisation Congés Spectacles) laisse le contrat muet, et un
//! engagement muet est un CDDU (regle 6). Un bulletin qui n'en porte aucune est celui d'un permanent : CDI ou CDD de
//! droit commun, que rien ne dit. Ses contrats sont marques `nature_a_dire` ; ce module :
//!
//! - `a_dire` : les salaries dont des contrats repris attendent leur nature, avec les natures qu'on peut leur donner
//!   (celles d'un permanent : `contrats-base.json`, paie « mensuelle ») et, pour chacune, ce qu'il faut dire avec elle
//!   (`demande`) -- `importation-etat` les rend, la page les demande apres l'import (Configuration, « Reprendre
//!   l'historique ») ;
//! - la route `importation-nature` `{"salarie", "nature", "debut"?, "fin"?, "numero"?}` : la nature dite s'ecrit sur
//!   chacun de ses contrats a dire, la marque s'efface, et la fiche Entreprise dit que la compagnie emploie des
//!   permanents. Une nature qui n'est pas d'un permanent, un salarie sans contrat a dire, ce que `demande` exige et qui
//!   manque : refus, rien n'est ecrit.
//!
//! LE CONTRAT QUI CONTINUE. Un permanent paye le dernier mois repris (le plus recent de l'historique) a un contrat qui
//! court encore, sauf s'il finissait ce mois-la. Le mois suivant le reprend, et la DSN le declare sous le MEME numero
//! (S21.G00.40.009, « un des identifiants du contrat » ; CCH-12) et la meme date de debut (S21.G00.40.001) que
//! l'ancien logiciel : `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`. Aucun bulletin ne les imprime ; un
//! numero ne se recalcule pas, il se lit (regle 4) : sur la DSN deja deposee. Ils se demandent donc avec la nature
//! (`demande` : `debut`, `numero`, le salaire mensuel et la duree du travail du contrat, `fin` s'il a un terme),
//! s'ecrivent sur ses contrats repris (`contrat_debut`, `contrat_fin`, `numero`, `contrat_salaire_mensuel`,
//! `contrat_quotite`), et le contrat se reprend au mois suivant (`repris_en_cours`, lu par
//! `crate::routes::mois::contrats_en_cours`) avec son numero, que l'engagement relit (`engagement::suivre_permanent`).
//!
//! Un salarie, une reponse : les bulletins d'un permanent attestent un contrat par mois, tous du meme contrat.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;

/// La marque d'un contrat repris dont la nature reste a dire.
pub const A_DIRE: &str = "nature_a_dire";

/// La route de ce module.
pub const ROUTE: &str = "importation-nature";

fn chemin(racine: &Path) -> std::path::PathBuf {
    racine.join("historique").join("engagements.json")
}

/// Les natures qu'un permanent peut recevoir.
fn natures_permanentes() -> R<Vec<std::sync::Arc<droit::natures::Nature>>> {
    let toutes = droit::natures::natures().map_err(|m| erreur("ErreurOperation", m))?;
    Ok(toutes.iter().filter(|n| n.paie == "mensuelle").cloned().collect())
}

/// Ce qu'une nature demande avec elle pour un contrat qui court encore : son debut, son numero, son salaire et
/// sa duree du travail, et sa fin s'il a un terme. Rien pour un contrat fini.
fn demande(n: &droit::natures::Nature, en_cours: bool) -> Vec<&'static str> {
    match (en_cours, n.duree_indeterminee) {
        (false, _) => vec![],
        (true, true) => vec!["debut", "numero", "salaire", "quotite"],
        (true, false) => vec!["debut", "fin", "numero", "salaire", "quotite"],
    }
}

fn texte(e: &Value, k: &str) -> String {
    e.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()
}

fn engagements(racine: &Path) -> Vec<Value> {
    crate::routes::lire_json_ou_vide(&chemin(racine))
        .ok()
        .and_then(|h| h.get("engagements").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

/// Le dernier mois repris : le plus recent que l'historique atteste (`AAAA-MM`).
fn dernier_mois(engs: &[Value]) -> String {
    engs.iter().map(|e| texte(e, "fin")).filter(|f| f.len() >= 7).map(|f| f[..7].to_string()).max().unwrap_or_default()
}

/// Le mois qui suit `AAAA-MM`.
fn mois_suivant(m: &str) -> String {
    let (a, mm) = (m.get(..4).and_then(|x| x.parse::<i32>().ok()).unwrap_or(0), m.get(5..7).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0));
    if mm >= 12 {
        format!("{:04}-01", a + 1)
    } else {
        format!("{:04}-{:02}", a, mm + 1)
    }
}

/// Le nom d'un salarie, tel que la fiche le montre ; sa cle a defaut.
fn nom_du_salarie(racine: &Path, cle: &str) -> String {
    let fiches = crate::routes::lire_json_ou_vide(&racine.join("config").join("salaries.json")).unwrap_or(Value::Null);
    let Some(f) = fiches.get(cle).filter(|f| f.is_object()) else { return cle.to_string() };
    let t = |k: &str| texte(f, k);
    let prenom = if t("prenom_affichage").is_empty() { t("prenom") } else { t("prenom_affichage") };
    let nom = if t("nom_affichage").is_empty() { t("nom") } else { t("nom_affichage") };
    let n = format!("{} {}", prenom, nom).trim().to_string();
    if n.is_empty() {
        cle.to_string()
    } else {
        n
    }
}

/// Les contrats a dire d'un salarie : (nombre, premier debut, derniere fin).
fn a_dire_par_salarie(engs: &[Value]) -> BTreeMap<String, (usize, String, String)> {
    let mut par: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    for e in engs {
        if !e.get(A_DIRE).is_some_and(vrai) {
            continue;
        }
        let Some(s) = e.get("salarie").and_then(Value::as_str) else { continue };
        let (debut, fin) = (texte(e, "debut"), texte(e, "fin"));
        let x = par.entry(s.to_string()).or_insert((0, debut.clone(), fin.clone()));
        x.0 += 1;
        if debut < x.1 {
            x.1 = debut;
        }
        if fin > x.2 {
            x.2 = fin;
        }
    }
    par
}

/// Les salaries dont des contrats repris attendent leur nature : `[{salarie, nom, contrats, du, au, en_cours, suite,
/// natures: [{cle, libelle, demande}]}]`, dans l'ordre des cles. `en_cours` : il est paye le dernier mois repris, son
/// contrat court peut-etre encore (`suite` : le mois qui suit). Ne leve jamais : un historique illisible n'a rien a dire.
pub(crate) fn a_dire(racine: &Path) -> Vec<Value> {
    let engs = engagements(racine);
    let par = a_dire_par_salarie(&engs);
    if par.is_empty() {
        return Vec::new();
    }
    let dernier = dernier_mois(&engs);
    let natures = natures_permanentes().unwrap_or_default();
    par.into_iter()
        .map(|(s, (n, du, au))| {
            let en_cours = au.get(..7) == Some(dernier.as_str());
            let natures: Vec<Value> =
                natures.iter().map(|x| json!({"cle": x.cle, "libelle": x.libelle, "demande": demande(x, en_cours)})).collect();
            json!({"salarie": s, "nom": nom_du_salarie(racine, &s), "contrats": n, "du": du, "au": au, "en_cours": en_cours,
                   "suite": if en_cours { json!(mois_suivant(&dernier)) } else { Value::Null }, "natures": natures})
        })
        .collect()
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur("ErreurOperation", m.into())
}

/// `importation-nature` : la nature dite pour les contrats a dire d'un salarie, et, s'il court encore, le debut, la fin
/// et le numero du contrat tels que la DSN de l'ancien logiciel les a declares. Rend `{"ecrits", "a_dire",
/// "permanents_declares"}`.
pub(crate) fn dire(ctx: &Contexte, charge: &Value) -> R<Value> {
    let lu = |k: &str| charge.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let salarie = lu("salarie").ok_or_else(|| refus("Le salarié manque : rien n'a été écrit."))?;
    let nature = lu("nature").ok_or_else(|| refus("La nature du contrat manque : rien n'a été écrit."))?;
    let permises = natures_permanentes()?;
    let Some(nat) = permises.iter().find(|n| n.cle == nature) else {
        let noms: Vec<&str> = permises.iter().map(|n| n.libelle.as_str()).collect();
        return Err(refus(format!("« {} » n'est pas la nature d'un contrat de permanent ({}) : rien n'a été écrit.", nature, noms.join(", "))));
    };
    let p = chemin(&ctx.racine);
    let mut h = crate::routes::lire_json_ou_vide(&p)?;
    let engs = h.get("engagements").and_then(Value::as_array).cloned().unwrap_or_default();
    let qui = nom_du_salarie(&ctx.racine, &salarie);
    let Some((_, du, au)) = a_dire_par_salarie(&engs).remove(&salarie) else {
        return Err(refus(format!("Aucun contrat repris de {} n'attend sa nature : rien n'a été écrit.", qui)));
    };
    // Le contrat qui court encore : son debut, sa fin s'il a un terme, son numero -- lus sur la DSN deja deposee.
    let en_cours = au.get(..7) == Some(dernier_mois(&engs).as_str());
    let mut contrat = Map::new();
    for champ in demande(nat, en_cours) {
        let v = lu(champ).ok_or_else(|| {
            refus(match champ {
                "debut" => format!("La date de début du contrat de {} manque (celle que la DSN déclare) : rien n'a été écrit.", qui),
                "fin" => format!("La date de fin prévue du contrat de {} manque : rien n'a été écrit.", qui),
                "salaire" => format!("Le salaire mensuel brut du contrat de {} manque (celui du contrat signé) : rien n'a été écrit.", qui),
                "quotite" => format!(
                    "La durée du travail du contrat de {} manque (en heures par mois, celle du contrat signé) : rien n'a été écrit.",
                    qui
                ),
                _ => format!("Le numéro du contrat de {} manque (celui que la DSN déclare) : rien n'a été écrit.", qui),
            })
        })?;
        match champ {
            "debut" | "fin" => {
                let j = Jour::lire(&v).map_err(|m| refus(format!("{} : rien n'a été écrit.", m)))?.iso();
                if champ == "debut" && j > du {
                    return Err(refus(format!(
                        "Le contrat de {} ne peut pas commencer le {} : un bulletin le paie dès le {}. Rien n'a été écrit.",
                        qui, j, du
                    )));
                }
                if champ == "fin" && j < au {
                    return Err(refus(format!("Le contrat de {} ne peut pas finir le {} : un bulletin le paie jusqu'au {}. Rien n'a été écrit.", qui, j, au)));
                }
                contrat.insert(if champ == "debut" { "contrat_debut" } else { "contrat_fin" }.into(), json!(j));
            }
            "salaire" | "quotite" => {
                let s = v.replace([' ', '\u{a0}'], "").replace(',', ".");
                let (cle, quoi) = if champ == "salaire" {
                    ("contrat_salaire_mensuel", "un salaire mensuel brut")
                } else {
                    ("contrat_quotite", "une durée du travail en heures par mois")
                };
                match s.parse::<socle::num::Decimal>() {
                    Ok(d) if d > socle::num::Decimal::ZERO => contrat.insert(cle.into(), json!(d.to_string().parse::<f64>().unwrap_or_default())),
                    _ => return Err(refus(format!("« {} » n'est pas {} : rien n'a été écrit.", v, quoi))),
                };
            }
            _ => {
                // S21.G00.40.009 : 20 caracteres au plus (cahier technique, bloc S21.G00.40).
                if v.chars().count() > 20 || v.chars().any(char::is_whitespace) {
                    return Err(refus(format!("« {} » n'est pas un numéro de contrat de la DSN (20 caractères au plus, sans espace) : rien n'a été écrit.", v)));
                }
                contrat.insert("numero".into(), json!(v));
            }
        }
    }
    let mut ecrits = 0usize;
    if let Some(Value::Array(liste)) = h.get_mut("engagements") {
        for e in liste.iter_mut() {
            let Some(o) = e.as_object_mut() else { continue };
            if !o.get(A_DIRE).is_some_and(vrai) || o.get("salarie").and_then(Value::as_str) != Some(salarie.as_str()) {
                continue;
            }
            o.remove(A_DIRE);
            o.insert("nature".into(), json!(nature));
            o.extend(contrat.clone());
            ecrits += 1;
        }
    }
    crate::routes::mois::sauver_avant(&p)?;
    socle::ecriture::ecrire_json(&p, &h, 1).map_err(|m| erreur("OSError", m))?;
    // LA COMPAGNIE EMPLOIE UN PERMANENT : c'est ce qu'on vient de dire (une nature payee au mois). La case « La
    // compagnie emploie aussi des salaries au mois » de la fiche Entreprise (`contexte::emploie_des_permanents`) se
    // coche donc ici, sans la redemander : sans elle, la paie du mois suivant refuserait ce contrat
    // (`engagement::nature_de`), et ce qu'elle ouvre (les contrats de prevoyance et de sante des permanents) se
    // demande alors dans « Pour demarrer », comme pour toute compagnie qui la coche.
    let permanents_declares = !droit::contexte::emploie_des_permanents(&ctx.entreprise);
    if permanents_declares {
        // A partir du premier jour que les bulletins repris paient.
        let depuis = Jour::lire(&du).map_err(|m| refus(format!("{} : la case des permanents n'a pas ete cochee.", m)))?;
        crate::routes::fiches::poser_entreprise(ctx, "emploie_des_permanents", json!(true), Some(depuis))?;
    }
    Ok(json!({"ecrits": ecrits, "a_dire": a_dire(&ctx.racine), "permanents_declares": permanents_declares}))
}

/// Un contrat de permanent repris des bulletins qui court en `mois` : le salarie, le debut, la fin (vide sans terme),
/// le numero, la nature, et le dernier mois repris.
pub(crate) struct Repris {
    pub salarie: String,
    pub debut: String,
    pub fin: String,
    pub numero: String,
    pub nature: String,
    pub depuis: String,
    /// Le salaire mensuel brut du contrat, tel qu'il a ete dit.
    pub salaire_mensuel: Option<Value>,
    /// Sa duree du travail (S21.G00.40.013), en heures par mois, telle qu'elle a ete dite.
    pub quotite: Option<Value>,
}

/// Les contrats repris (`contrat_debut` et `numero` dits) qui courent en `mois` (`AAAA-MM`), apres le dernier mois
/// que l'historique atteste : sans terme, ou dont la fin est au plus tot le 1er de `mois`.
pub(crate) fn repris_en_cours(racine: &Path, mois: &str) -> Vec<Repris> {
    let mut par: BTreeMap<(String, String, String), Repris> = BTreeMap::new();
    for e in engagements(racine) {
        let (salarie, debut, numero, nature) = (texte(&e, "salarie"), texte(&e, "contrat_debut"), texte(&e, "numero"), texte(&e, "nature"));
        if salarie.is_empty() || debut.is_empty() || numero.is_empty() || nature.is_empty() {
            continue;
        }
        let atteste = texte(&e, "fin").get(..7).unwrap_or("").to_string();
        let r = par.entry((salarie.clone(), debut.clone(), numero.clone())).or_insert_with(|| Repris {
            salarie,
            debut,
            fin: texte(&e, "contrat_fin"),
            numero,
            nature,
            depuis: atteste.clone(),
            salaire_mensuel: e.get("contrat_salaire_mensuel").filter(|v| v.is_number()).cloned(),
            quotite: e.get("contrat_quotite").filter(|v| v.is_number()).cloned(),
        });
        if atteste > r.depuis {
            r.depuis = atteste;
        }
    }
    let premier = format!("{}-01", mois);
    par.into_values()
        .filter(|r| r.depuis.as_str() < mois && r.debut.as_str() < premier.as_str() && (r.fin.is_empty() || r.fin >= premier))
        .filter(|r| droit::natures::nature(Some(&r.nature)).is_ok_and(|n| n.mensuelle() && (n.duree_indeterminee || !r.fin.is_empty())))
        .collect()
}
