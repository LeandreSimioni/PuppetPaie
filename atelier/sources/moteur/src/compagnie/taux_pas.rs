//! Le magasin `config/pas.json` : les taux de prelevement a la source que la DGFiP a fait connaitre (un compte rendu de
//! nature 94, `suites`) ou que les DSN deposees declarent (`moteur pas --amorcer`, l'import). `droit::pas` les lit.

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::Echec;

pub(crate) fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

fn maintenant_court() -> String {
    // A l'heure de Paris, a la minute.
    socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].replace('T', " ")
}

/// Le magasin, indente de deux espaces, sans saut de ligne final.
fn ecrire(ctx: &Contexte, data: &Value) -> Result<(), Echec> {
    let p = ctx.racine.join("config").join("pas.json");
    std::fs::write(&p, socle::ecriture::json_indente(data, 2)).map_err(|e| Echec { classe: Some("OSError"), message: format!("{} : {}", p.display(), e) })
}

/// Applique des releves de CRM ; rend les changements lisibles. Un releve :
/// `{"salarie", "taux", "type", "identifiant", "periode", "mis_a_disposition", "applique_le"}`.
pub fn enregistrer(ctx: &Contexte, releves: &[Value], source: &str) -> Result<Vec<String>, Echec> {
    let mut data = droit::pas::charger(ctx).map_err(ko)?;
    let mut changements = Vec::new();
    let mut soldees = false;
    let horodatage = maintenant_court();
    for r in releves {
        let cle = r.get("salarie").and_then(Value::as_str).unwrap_or("").to_string();
        // LA REPONSE EST LA : la demande qui l'attendait n'a plus lieu d'etre. Elle se solde AVANT le test d'egalite.
        let demandes = data["demandes"].as_object_mut().ok_or_else(|| ko("demandes illisibles".into()))?;
        soldees = demandes.shift_remove(&cle).is_some() || soldees;
        let avant = data["salaries"].get(&cle).cloned();
        let get = |k: &str| r.get(k).cloned().unwrap_or(Value::Null);
        let mut neuf = json!({
            "taux": get("taux"), "type": get("type"), "identifiant": get("identifiant"),
            "periode": get("periode"), "mis_a_disposition": get("mis_a_disposition"),
            "source": source, "releve_le": horodatage,
        });
        if let Some(a) = r.get("applique_le").filter(|a| socle::fichiers::vrai(a)) {
            neuf["applique_le"] = a.clone();
        }
        if let Some(a) = avant.as_ref().filter(|a| socle::fichiers::vrai(a)) {
            if ["taux", "type", "identifiant"].iter().all(|k| a.get(*k) == neuf.get(*k)) {
                continue;
            }
        }
        let t = |v: &Value, k: &str| crate::commandes::rendu::champ(v, k);
        data["salaries"].as_object_mut().ok_or_else(|| ko("salaries illisibles".into()))?.insert(cle.clone(), neuf.clone());
        match avant.filter(|a| socle::fichiers::vrai(a)) {
            Some(a) => changements.push(format!(
                "{} : taux {} -> {}, type {} -> {}, identifiant {} -> {}",
                cle, t(&a, "taux"), t(&neuf, "taux"), t(&a, "type"), t(&neuf, "type"), t(&a, "identifiant"), t(&neuf, "identifiant")
            )),
            None => changements.push(format!("{} : premier releve - taux {}, type {}, identifiant {}", cle, t(&neuf, "taux"), t(&neuf, "type"), t(&neuf, "identifiant"))),
        }
        let mut ligne = neuf.as_object().cloned().unwrap_or_default();
        ligne.insert("salarie".into(), json!(cle));
        data["journal"].as_array_mut().ok_or_else(|| ko("journal illisible".into()))?.push(Value::Object(ligne));
    }
    if !changements.is_empty() || soldees {
        ecrire(ctx, &data)?;
    }
    if !changements.is_empty() {
        refaire_les_paies(ctx)?;
    }
    Ok(changements)
}

/// UN TAUX RECU REFAIT, DANS LE MEME GESTE, LES PAIES QU'IL TOUCHE (le recalcul de lui-meme, « taux de PAS reçu ») :
/// chaque mois saisi se relit (`engagement::etats::du_mois`), et ses paies validees dont l'empreinte ne tient plus se
/// reecrivent. Un mois dont les documents sont partis ne bouge pas. Aucune lecture ulterieure n'a donc a le decouvrir.
fn refaire_les_paies(ctx: &Contexte) -> Result<(), Echec> {
    let departs = envois::journal::lu(&ctx.racine).map_err(ko)?.departs;
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(ko)? {
        engagement::etats::du_mois(ctx, &mois, &departs).map_err(ko)?;
    }
    Ok(())
}

/// Par salarie, l'historique des taux declares dans les DSN deja deposees (une lecture, pas un calcul).
fn releves_historiques(ctx: &Contexte, dossier: &std::path::Path) -> Vec<(String, Vec<Value>)> {
    let par_nir: std::collections::HashMap<String, String> = ctx
        .salaries
        .iter()
        .filter_map(|(cle, s)| s.get("nir").filter(|n| socle::fichiers::vrai(n)).map(|n| (crate::commandes::rendu::texte(n), cle.clone())))
        .collect();
    let mut fichiers = Vec::new();
    crate::routes::fiches::fichiers_dsn(dossier, &mut fichiers);
    fichiers.sort();
    // salarie -> mois -> releve (dans l'ordre d'insertion des salaries, mois tries).
    let mut trouve: Vec<(String, std::collections::BTreeMap<String, Value>)> = Vec::new();
    for f in fichiers {
        let Ok(octets) = std::fs::read(&f) else { continue };
        let texte: String = octets.iter().map(|b| *b as char).collect();
        let (mut mois, mut nir): (Option<String>, Option<String>) = (None, None);
        let mut cur: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for ligne in socle::texte::lignes_du_texte(&texte) {
            if !ligne.contains(',') {
                continue;
            }
            let Some((code, val)) = ligne.trim().split_once(',') else { continue };
            let val = val.trim().trim_matches('\'').to_string();
            if code == "S20.G00.05.005" {
                let c: Vec<char> = val.chars().collect();
                let tr = |a: usize, b: usize| -> String { c.iter().skip(a).take(b.saturating_sub(a)).collect() };
                mois = Some(format!("{}-{}", tr(4, 8), tr(2, 4)));
            } else if code == "S21.G00.30.001" {
                nir = Some(val);
            } else if code.starts_with("S21.G00.50.00") {
                cur.insert(code.to_string(), val.clone());
                // « -1 » : un CDD court sans taux personnalise (`norme-dsn/texte/bloc-S21.G00.50.txt`, S21.G00.50.008) ;
                // aucun taux n'a ete transmis.
                if code == "S21.G00.50.008" && val != "-1" {
                    if let (Some(cle), Some(m)) = (nir.as_ref().and_then(|n| par_nir.get(n)), mois.as_ref()) {
                        let g = |k: &str| cur.get(k).map_or(Value::Null, |v| json!(v));
                        let mut rel = json!({"taux": g("S21.G00.50.006"), "type": g("S21.G00.50.007"), "identifiant": val, "periode": m});
                        // S21.G00.50.001, la date du versement (JJMMAAAA).
                        if let Ok(j) = socle::date::Jour::lire(&crate::import::dsn_aem::jjmmaaaa(cur.get("S21.G00.50.001").map_or("", String::as_str))) {
                            rel["applique_le"] = json!(j.iso());
                        }
                        match trouve.iter_mut().find(|(k, _)| k == cle) {
                            Some((_, mm)) => {
                                mm.insert(m.clone(), rel);
                            }
                            None => trouve.push((cle.clone(), [(m.clone(), rel)].into_iter().collect())),
                        }
                    }
                    cur.clear();
                }
            }
        }
    }
    trouve.into_iter().map(|(cle, mm)| (cle, mm.into_values().collect())).collect()
}

/// LE DERNIER TAUX TRANSMIS PAR LA DGFiP QUE LES DSN DEJA DEPOSEES DE `dossier` DECLARENT, par salarie (un releve tel que
/// `enregistrer` le prend). Une DSN qui declare un taux de type 01 avec son identifiant (S21.G00.50.006 a .008,
/// `norme-dsn/texte/bloc-S21.G00.50.txt` : « Identifiant du taux porté par le CRM PAS […] transmis par la DGFIP ») dit
/// que la DGFiP a transmis ce taux a l'employeur, qui l'applique (CGI, art. 1671, 2,
/// `atelier/reference/sources/code-general-des-impots/articles-20261021.md`). Sa validite court de sa mise a disposition
/// (`droit::pas::VALIDITE_MOIS`), que la DSN ne porte pas. Ce que les textes en disent : la retenue se fait « lors du
/// paiement » (CGI, art. 1671, 1) avec un taux « toujours valide à la date de son application » (BOI-IR-PAS-30-10-20,
/// § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) : le taux etait donc mis a disposition au
/// plus tard le jour du versement (S21.G00.50.001) et au plus tot deux mois pleins avant ; le compte rendu « est
/// transmis le mois au cours duquel la DSN […] a été déposée » (meme page, § 30), mais la DSN qui applique le
/// taux n'en porte pas la date. INFERENCE (aucun texte ne date un taux lu dans une DSN) : le releve est date du jour du versement
/// (`applique_le`), la mise a disposition la plus tardive que les textes permettent ; le compte rendu de la DSN suivante
/// le remplace.
pub(crate) fn derniers_taux_des_dsn(ctx: &Contexte, dossier: &std::path::Path) -> Vec<Value> {
    let mut releves = Vec::new();
    for (cle, entrees) in releves_historiques(ctx, dossier) {
        if let Some(dernier) = entrees.last() {
            let mut r = dernier.as_object().cloned().unwrap_or_default();
            r.insert("salarie".into(), json!(cle));
            releves.push(Value::Object(r));
        }
    }
    releves
}
