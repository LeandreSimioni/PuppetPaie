//! OU VIVENT LES PAIES : `paies/AAAA-MM.json`, le fichier du mois paye, `{"mois", "paies": [...]}`. Une entree par
//! engagement valide qui touche le mois, designee par son salarie et le numero de son contrat (`salarie`, `numero`, tels
//! que l'engagement les porte) : sa paie ecrite (`etats` la calcule : `paie`, `fiches`, `saisie_du_mois`, `empreinte`,
//! `pas`, `conges`, `conges_avant`, `bulletin`, `ce`, `rectifie`, `recalcul`), et, sous `versions`, la paie de ce mois que
//! chaque version passee de l'engagement avait ecrite (`{"<n>": <paie ecrite>}`) : ce qui est parti sur elle s'y relit.
//! L'engagement ne porte que ses termes ; un mois se lit dans son seul fichier.
//!
//! UNE SEULE FONCTION LIT UN FICHIER DE PAIES (`fichier`), UNE SEULE L'ECRIT (`ecrire`), par le stock et son brouillon
//! (`stock::Brouillon`) : un engagement et ses paies s'ecrivent dans la meme ecriture.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Map, Value};

use crate::stock::{self, Dossier, Objet};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::num::Resultat;

/// Le dossier des paies, sous les donnees de la compagnie.
pub const DOSSIER: &str = "paies";

/// Ce qu'une entree porte en plus de la paie ecrite : qui elle est, et les paies des versions passees.
const SALARIE: &str = "salarie";
const NUMERO: &str = "numero";
const VERSIONS: &str = "versions";

/// Le fichier des paies de `mois`, pour le lire tel qu'il est sur le disque (l'archive de l'exercice).
pub fn chemin(racine: &std::path::Path, mois: &str) -> std::path::PathBuf {
    stock::chemin_dans(racine, Dossier::Paies, mois)
}

/// Le salarie et le numero du contrat de l'engagement `o` ; `None` sans numero (un engagement que rien n'a valide).
fn cle(o: &Objet) -> Option<(String, String)> {
    let numero = o.brut.get(NUMERO).filter(|n| vrai(n)).map(en_texte)?;
    Some((o.brut.get(SALARIE).map(en_texte).unwrap_or_default(), numero))
}

fn est_sienne(e: &Value, (salarie, numero): &(String, String)) -> bool {
    e.get(SALARIE).map(en_texte).as_deref() == Some(salarie.as_str()) && e.get(NUMERO).map(en_texte).as_deref() == Some(numero.as_str())
}

/// LA LECTURE D'UN FICHIER DE PAIES : celui de `mois`, tel que le geste l'a lu ou que le brouillon le tient.
fn fichier(racine: &std::path::Path, mois: &str) -> Resultat<Option<Arc<Value>>> {
    stock::lire_dans(racine, Dossier::Paies, mois)
}

/// Une entree telle qu'elle est ecrite, et sa paie presente, partagee : la lire ne la recopie pas.
struct Entree {
    brut: Map<String, Value>,
    presente: Option<Arc<Value>>,
}

/// Les entrees d'un fichier de paies, par salarie et numero.
type Index = HashMap<(String, String), Arc<Entree>>;

/// Les entrees du fichier de `mois`, tirees une fois de son contenu : ce qu'on en avait tire ne sert que si c'est le
/// meme contenu (`Arc::ptr_eq` : le geste et le brouillon servent la meme valeur tant qu'elle n'a pas change).
fn index(racine: &std::path::Path, mois: &str) -> Resultat<Option<Arc<Index>>> {
    type Gardes = Mutex<HashMap<(std::path::PathBuf, String), (Arc<Value>, Arc<Index>)>>;
    static GARDES: OnceLock<Gardes> = OnceLock::new();
    let Some(f) = fichier(racine, mois)? else { return Ok(None) };
    let gardes = GARDES.get_or_init(Default::default);
    let cle_du_fichier = (racine.to_path_buf(), mois.to_string());
    if let Some((vu, i)) = gardes.lock().unwrap_or_else(|p| p.into_inner()).get(&cle_du_fichier) {
        if Arc::ptr_eq(vu, &f) {
            return Ok(Some(i.clone()));
        }
    }
    let mut i = Index::new();
    for e in f.get("paies").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object) {
        let cle = (e.get(SALARIE).map(en_texte).unwrap_or_default(), e.get(NUMERO).map(en_texte).unwrap_or_default());
        i.entry(cle).or_insert_with(|| Arc::new(Entree { brut: e.clone(), presente: presente_de(e).map(Arc::new) }));
    }
    let i = Arc::new(i);
    socle::fichiers::ranger(gardes, cle_du_fichier, (f, i.clone()));
    Ok(Some(i))
}

/// L'entree de l'engagement `o` dans le fichier de `mois`, telle qu'elle est ecrite.
fn entree(o: &Objet, mois: &str) -> Resultat<Option<Arc<Entree>>> {
    let Some(cle) = cle(o) else { return Ok(None) };
    Ok(index(&o.racine, mois)?.and_then(|i| i.get(&cle).cloned()))
}

/// La paie ecrite d'une entree : tout sauf qui elle est et les versions passees ; `None` si elle n'en porte pas.
fn presente_de(e: &Map<String, Value>) -> Option<Value> {
    let p: Map<String, Value> = e.iter().filter(|(k, _)| ![SALARIE, NUMERO, VERSIONS].contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    (!p.is_empty()).then_some(Value::Object(p))
}

/// L'ECRITURE D'UNE PAIE : l'entree de `o` dans le fichier de `mois` (neuve, `{"salarie", "numero"}`, s'il n'en a pas)
/// passe par `change` ; une entree qui ne porte plus rien s'en va. Le fichier ne s'ecrit que s'il change. Refus : un
/// engagement sans numero de contrat.
fn ecrire(o: &Objet, mois: &str, change: impl FnOnce(&mut Map<String, Value>)) -> Resultat<()> {
    let cle = cle(o).ok_or_else(|| format!("{} : un engagement sans numero de contrat n'a pas de paie", mois))?;
    let lu = fichier(&o.racine, mois)?;
    let mut paies: Vec<Value> = lu.as_ref().and_then(|f| f.get("paies")).and_then(Value::as_array).cloned().unwrap_or_default();
    let place = paies.iter().position(|e| est_sienne(e, &cle));
    let mut e = match place {
        Some(i) => paies[i].as_object().cloned().unwrap_or_default(),
        None => Map::from_iter([(SALARIE.to_string(), json!(cle.0)), (NUMERO.to_string(), json!(cle.1))]),
    };
    change(&mut e);
    if e.get(VERSIONS).and_then(Value::as_object).is_some_and(Map::is_empty) {
        e.remove(VERSIONS);
    }
    let vide = e.len() <= 2;
    match (place, vide) {
        (Some(i), true) => {
            paies.remove(i);
        }
        (Some(i), false) => paies[i] = Value::Object(e),
        (None, false) => paies.push(Value::Object(e)),
        (None, true) => return Ok(()),
    }
    let contenu = json!({ "mois": mois, "paies": paies });
    if lu.as_deref() == Some(&contenu) {
        return Ok(());
    }
    stock::poser_dans(&o.racine, Dossier::Paies, mois, &contenu)?;
    Ok(())
}

/// La paie ecrite de `mois` de l'engagement `o`, celle de sa version presente ; `None` s'il n'en a pas.
pub fn presente(o: &Objet, mois: &str) -> Resultat<Option<Arc<Value>>> {
    Ok(entree(o, mois)?.and_then(|e| e.presente.clone()))
}

/// La paie de `mois` que la version passee `n` de l'engagement `o` avait ecrite ; `None` si elle n'en a pas.
pub fn de_la_version(o: &Objet, n: u64, mois: &str) -> Resultat<Option<Value>> {
    Ok(entree(o, mois)?.and_then(|e| e.brut.get(VERSIONS)?.get(n.to_string()).cloned()))
}

/// Toutes les paies ecrites de `mois` de l'engagement `o` : la presente, puis celles des versions passees.
pub fn ecrites(o: &Objet, mois: &str) -> Resultat<Vec<Value>> {
    let Some(e) = entree(o, mois)? else { return Ok(Vec::new()) };
    let passees = e.brut.get(VERSIONS).and_then(Value::as_object).into_iter().flat_map(|v| v.values().cloned());
    Ok(e.presente.as_deref().cloned().into_iter().chain(passees).collect())
}

/// Les mois ou l'engagement `o` a une paie ecrite (presente ou d'une version passee), du premier au dernier.
pub fn mois_ecrits(o: &Objet) -> Resultat<Vec<String>> {
    let mut out = Vec::new();
    for m in stock::fichiers_dans(&o.racine, Dossier::Paies).into_iter().filter(|m| m.as_str() >= o.fichier.as_str()) {
        if entree(o, &m)?.is_some() {
            out.push(m);
        }
    }
    Ok(out)
}

/// Les paies ecrites de la version presente de l'engagement `o`, par mois (`mois_ecrits`).
pub fn presentes(o: &Objet) -> Resultat<Map<String, Value>> {
    let mut out = Map::new();
    for m in mois_ecrits(o)? {
        if let Some(p) = presente(o, &m)? {
            out.insert(m, (*p).clone());
        }
    }
    Ok(out)
}

/// Le nombre de paies ecrites de `mois`, celles des versions presentes des engagements.
pub fn nombre(racine: &std::path::Path, mois: &str) -> Resultat<usize> {
    let Some(f) = fichier(racine, mois)? else { return Ok(0) };
    Ok(f.get("paies").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_object).filter(|e| presente_de(e).is_some()).count())
}

/// La paie ecrite de `mois` de l'engagement `o` devient `paie` ; `None` la retire (les versions passees restent).
pub(crate) fn poser(o: &Objet, mois: &str, paie: Option<Value>) -> Resultat<()> {
    ecrire(o, mois, |e| {
        e.retain(|k, _| [SALARIE, NUMERO, VERSIONS].contains(&k.as_str()));
        if let Some(Value::Object(p)) = paie {
            for (k, v) in p {
                if ![SALARIE, NUMERO, VERSIONS].contains(&k.as_str()) {
                    e.insert(k, v);
                }
            }
        }
    })
}

/// LA VERSION `n` DE L'ENGAGEMENT `o` DEVIENT PASSEE : la paie presente de chaque mois ou il en a une se garde sous son
/// numero (`de_la_version`).
pub(crate) fn garder_la_version(o: &Objet, n: u64) -> Resultat<()> {
    for m in mois_ecrits(o)? {
        if let Some(p) = presente(o, &m)? {
            ecrire(o, &m, |e| {
                let v = e.entry(VERSIONS).or_insert_with(|| json!({}));
                if let Some(v) = v.as_object_mut() {
                    v.insert(n.to_string(), (*p).clone());
                }
            })?;
        }
    }
    Ok(())
}

/// Les paies de l'engagement `o`, presentes et passees, s'effacent de tous ses mois.
pub(crate) fn effacer(o: &Objet) -> Resultat<()> {
    for m in mois_ecrits(o)? {
        ecrire(o, &m, |e| e.retain(|k, _| [SALARIE, NUMERO].contains(&k.as_str())))?;
    }
    Ok(())
}
