//! La liste des pays qu'une fiche salarie choisit : pays de naissance (`pays_naissance`, `S21.G00.30.015`) et pays de
//! nationalite (`pays_nationalite`, d'ou se deduit `S21.G00.30.013`).
//!
//! LA LISTE est `logiciel/donnees/pays-base.json`, lue dans le Code officiel geographique de l'INSEE au 1er janvier
//! 2026, fichier « Pays et territoires etrangers »
//! (`atelier/reference/sources/insee-cog-pays/insee-cog-pays-territoires-etrangers-20260101.csv`, `LISEZ-MOI.md` a
//! cote) : le nom (LIBCOG), le code ISO 3166 a deux lettres (CODEISO2), le pays de rattachement d'un territoire
//! (CRPAY). Une ligne du COG sans code ISO, ou qui reprend celui d'un Etat, n'est pas dans la liste (`ecartes`).
//!
//! LA NORME (cahier technique DSN 2026.1, p. 173, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 297-303) :
//! « Le Code pays de naissance est à déterminer dans la table "Codes Pays" ISO 3166-1-A2 » ; « X [2,2] Table Pays ».
//!
//! LE PAYS DE NAISSANCE (`S21.G00.30.015`) se lit dans LA TABLE « PAYS » DE LA DSN,
//! livree dans `logiciel/donnees/pays-dsn-base.json` : la nomenclature NEODeS de net-entreprises du 24/09/2026
//! (`atelier/reference/sources/dsn-nomenclatures/tables-PAYS-et-PAY-20260924.txt`, `LISEZ-MOI.md` a cote), 274 codes,
//! les 213 du COG et 61 de plus (XK Kosovo, les Etats disparus : SU URSS, YU Yougoslavie, CS Tchecoslovaquie... ; les
//! territoires et les DROM-COM) : un ne au Kosovo se declare.
//! LE PAYS DE NATIONALITE garde la liste du COG : la DSN n'en declare que la codification UE (`30.013`), qui se deduit
//! du pays ou de son pays de rattachement, et la table « Pays » ne dit ni l'un ni l'autre.
//!
//! LA CODIFICATION UE d'un pays se lit dans `codification_ue::code_du_pays`, sur son code, ou sur celui de son pays de
//! rattachement pour un territoire (Groenland : Danemark ; Aruba, Curacao : Pays-Bas ; Gibraltar :
//! Royaume-Uni ; Hong Kong : Chine). Ce rattachement vaut nationalite : c'est une INFERENCE (le COG dit le
//! rattachement, pas la nationalite de ceux qui y naissent). Les Feroe font exception : les traites ne s'y appliquent
//! pas (`codification_ue::TERRITOIRES_HORS_UNION`), leur codification est celle d'un pays hors de l'Union.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

use crate::fichiers::{dossier_du_code, json_en_cache};
use crate::num::Resultat;

/// Une ligne de la liste.
#[derive(Debug, Clone)]
pub struct Pays {
    pub code: String,
    pub nom: String,
    pub nom_officiel: String,
    pub rattachement: Option<String>,
}

/// Une table livree, lue et construite une fois tant que son fichier ne change pas (chemin, date, taille) ; servie du
/// cache, elle se note quand meme au registre d'un calcul suivi. La fiche d'un salarie la consulte a chaque champ, et
/// l'ecran recoit la liste triee.
type Garde<T> = Mutex<Option<(PathBuf, Option<(u128, u64)>, Arc<Vec<T>>)>>;

fn garde<T>(cache: &'static OnceLock<Garde<T>>, nom: &str, construire: impl Fn(&Value) -> Resultat<Vec<T>>) -> Resultat<Arc<Vec<T>>> {
    let chemin = dossier_du_code().join(nom);
    let sig = crate::fichiers::signature(&chemin);
    let cache = cache.get_or_init(|| Mutex::new(None));
    if let Some((p, s, v)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *p == chemin && sig.is_some() && *s == sig {
            crate::fichiers::noter_si_besoin(&chemin);
            return Ok(v.clone());
        }
    }
    let v = Arc::new(construire(&*json_en_cache(&chemin)?)?);
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((chemin, sig, v.clone()));
    Ok(v)
}

/// La liste livree, dans l'ordre du COG.
pub fn liste() -> Resultat<Arc<Vec<Pays>>> {
    static CACHE: OnceLock<Garde<Pays>> = OnceLock::new();
    garde(&CACHE, "pays-base.json", |base| {
        let lignes = base.get("pays").and_then(Value::as_array).ok_or("pays-base.json : la liste « pays » manque")?;
        let texte = |l: &Value, k: &str| l.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok(lignes
            .iter()
            .map(|l| Pays {
                code: texte(l, "code"),
                nom: texte(l, "nom"),
                nom_officiel: texte(l, "nom_officiel"),
                rattachement: l.get("rattachement").and_then(Value::as_str).map(str::to_string),
            })
            .collect())
    })
}

/// Le pays d'un code ISO (casse et blancs ignores), ou `None` s'il n'est pas dans la liste.
pub fn du_code(code: &str) -> Resultat<Option<Pays>> {
    let c = code.trim().to_uppercase();
    Ok(liste()?.iter().find(|p| p.code == c).cloned())
}

/// La codification UE (`S21.G00.30.013`) d'un pays de la liste : celle de son code, ou de son pays de rattachement.
pub fn codification(p: &Pays) -> &'static str {
    if crate::codification_ue::TERRITOIRES_HORS_UNION.contains(&p.code.as_str()) {
        return crate::codification_ue::code_du_pays(&p.code);
    }
    crate::codification_ue::code_du_pays(p.rattachement.as_deref().unwrap_or(&p.code))
}

/// La phrase d'un code qui n'est pas dans la liste. `quoi` : « Le pays de naissance de X ».
pub fn inconnu(quoi: &str, code: &str) -> String {
    format!(
        "{} (« {} ») n'est pas un pays de la liste du logiciel (Code officiel géographique de l'INSEE, codes ISO 3166 à \
         deux lettres) : le choisir dans la liste de la fiche.",
        quoi,
        code.trim()
    )
}

/// La phrase d'un pays de nationalite qui n'est pas dans la liste : il s'ecrit a part, et la nationalite DSN se choisit
/// (`codification_ue::codification_ue`).
pub fn inconnu_nationalite(quoi: &str, code: &str) -> String {
    format!(
        "{} S'il n'y est pas, l'écrire dans « Pays de nationalité absent de la liste », laisser « Pays de nationalité » \
         vide, et choisir la nationalité que la DSN déclare (S21.G00.30.013).",
        inconnu(quoi, code)
    )
}

/// La phrase d'un pays de naissance qui n'est pas dans la table « Pays » de la DSN. `30.015` est obligatoire en DSN
/// mensuelle (`atelier/reference/norme-dsn/texte/tableaux-des-usages.txt` : « O » en nature 01), se lit dans cette table,
/// « CRE-11 : valeurs autorisées » (cahier technique DSN 2026.1, p. 174, `bloc-S21.G00.30.txt`) : un code qu'elle n'a pas
/// serait invente (regle 1).
pub fn inconnu_naissance(quoi: &str, code: &str) -> String {
    format!(
        "{} (« {} ») n'est pas dans la table « Pays » de la DSN, où se lit S21.G00.30.015 (cahier technique DSN 2026.1 \
         p. 174 ; nomenclature de net-entreprises, 274 pays, le Kosovo et les États disparus compris) : le choisir dans \
         la liste de la fiche.",
        quoi,
        code.trim()
    )
}

/// Une ligne de la table « Pays » de la DSN (`pays-dsn-base.json`).
#[derive(Debug, Clone)]
pub struct PaysDsn {
    pub code: String,
    pub libelle: String,
    /// Le code est aussi dans la table PAY (la meme sans la France, Monaco, les DROM et les COM).
    pub ne_a_l_etranger: bool,
}

/// La table « Pays » de la DSN, dans l'ordre de la nomenclature.
pub fn table_dsn() -> Resultat<Arc<Vec<PaysDsn>>> {
    static CACHE: OnceLock<Garde<PaysDsn>> = OnceLock::new();
    garde(&CACHE, "pays-dsn-base.json", |base| {
        let lignes = base.get("pays").and_then(Value::as_array).ok_or("pays-dsn-base.json : la liste « pays » manque")?;
        Ok(lignes
            .iter()
            .map(|l| PaysDsn {
                code: l.get("code").and_then(Value::as_str).unwrap_or("").to_string(),
                libelle: l.get("libelle").and_then(Value::as_str).unwrap_or("").to_string(),
                ne_a_l_etranger: l.get("ne_a_l_etranger").and_then(Value::as_bool).unwrap_or(false),
            })
            .collect())
    })
}

/// Le pays de naissance d'un code (casse et blancs ignores) : sa ligne de la table « Pays » de la DSN, ou `None`.
pub fn de_la_table_dsn(code: &str) -> Resultat<Option<PaysDsn>> {
    let c = code.trim().to_uppercase();
    Ok(table_dsn()?.iter().find(|p| p.code == c).cloned())
}

/// Le nom d'un pays de naissance a l'ecran : celui du COG quand il en a un (accentue, en casse ordinaire), le libelle de
/// la table « Pays » sinon (« KOSOVO », « URSS »), tel quel.
fn nom_de_naissance(p: &PaysDsn, cog: &[Pays]) -> String {
    cog.iter().find(|c| c.code == p.code).map_or_else(|| p.libelle.clone(), |c| c.nom.clone())
}

/// Ce que la page recoit pour le pays de naissance (`etat.pays_naissance`) : la table « Pays » de la DSN, triee par nom.
pub fn servir_naissance() -> Resultat<Value> {
    let cog = liste()?;
    let mut l: Vec<(String, String)> = table_dsn()?.iter().map(|p| (p.code.clone(), nom_de_naissance(p, &cog))).collect();
    // La cle se calcule une fois par pays (`sort_by_cached_key`, stable comme `sort_by_key`), pas a chaque comparaison.
    l.sort_by_cached_key(|(_, n)| cle_de_nom(n));
    Ok(Value::Array(l.into_iter().map(|(code, nom)| json!({"code": code, "nom": nom})).collect()))
}

/// Le code d'un pays de naissance nomme `texte` (le nom du COG, le nom officiel, ou le libelle de la table « Pays » de la
/// DSN), accents et casse ignores ; `None` sinon.
pub fn naissance_du_nom(texte: &str) -> Resultat<Option<String>> {
    if let Some(p) = du_nom(texte)? {
        return Ok(Some(p.code));
    }
    let t = cle_de_nom(texte);
    if t.is_empty() {
        return Ok(None);
    }
    Ok(table_dsn()?.iter().find(|p| cle_de_nom(&p.libelle) == t).map(|p| p.code.clone()))
}

/// Le nom qui sert a trier et a reconnaitre : sans accents, en minuscules, l'apostrophe typographique droite.
pub fn cle_de_nom(nom: &str) -> String {
    crate::texte::sans_accent(&nom.replace('\u{2019}', "'")).to_lowercase().trim().to_string()
}

/// Le pays dont le nom (celui du COG, ou le nom officiel) est `texte`, accents et casse ignores.
pub fn du_nom(texte: &str) -> Resultat<Option<Pays>> {
    let t = cle_de_nom(texte);
    if t.is_empty() {
        return Ok(None);
    }
    Ok(liste()?.iter().find(|p| cle_de_nom(&p.nom) == t || cle_de_nom(&p.nom_officiel) == t).cloned())
}

/// Ce que la page recoit (`etat.pays`) : la liste triee par nom, chaque pays avec la codification UE que le moteur
/// en deduit. La page l'affiche ; elle ne decide rien.
pub fn servir() -> Resultat<Value> {
    let mut l = (*liste()?).clone();
    l.sort_by_cached_key(|p| cle_de_nom(&p.nom));
    Ok(Value::Array(
        l.iter().map(|p| json!({"code": p.code, "nom": p.nom, "codification_ue": codification(p)})).collect(),
    ))
}

/// `S21.G00.30.015` d'une fiche : le code, s'il est dans la table « Pays » de la DSN. Absent : `'pays_naissance'` (la
/// rubrique manque).
pub fn de_naissance(fiche: &Value) -> Resultat<Value> {
    let v = fiche.get("pays_naissance").cloned().ok_or_else(|| crate::fichiers::absent("pays_naissance"))?;
    let Some(code) = v.as_str().filter(|c| !c.trim().is_empty()) else { return Ok(v) };
    match de_la_table_dsn(code)? {
        Some(p) => Ok(Value::String(p.code)),
        None => {
            let qui = [fiche.get("prenom"), fiche.get("nom")]
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            Err(inconnu_naissance(&format!("Le pays de naissance de {}", if qui.is_empty() { "ce salarié" } else { &qui }), code))
        }
    }
}
