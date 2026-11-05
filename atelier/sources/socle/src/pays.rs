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

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::fichiers::{forme_en_cache, FormeGardee};
use crate::num::Resultat;

/// Une ligne de la liste : le code ISO, le nom et le nom officiel du COG, le pays de rattachement d'un territoire.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pays {
    pub code: String,
    pub nom: String,
    pub nom_officiel: String,
    /// Le code du COG et son actualite (1 actuel, 4 territoire) : la provenance de la ligne.
    #[serde(rename = "cog")]
    _cog: String,
    #[serde(rename = "actualite")]
    _actualite: String,
    pub rattachement: Option<String>,
}

/// `pays-base.json` : la liste, et les lignes du COG qu'elle ecarte (sans code ISO, ou qui reprennent celui d'un Etat).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BasePays {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    pays: Vec<Pays>,
    #[serde(rename = "ecartes")]
    _ecartes: Vec<Ecarte>,
}

/// Une ligne du COG que la liste ecarte, et pourquoi.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ecarte {
    #[serde(rename = "cog")]
    _cog: String,
    #[serde(rename = "nom")]
    _nom: String,
    #[serde(rename = "nom_officiel")]
    _nom_officiel: String,
    #[serde(rename = "pourquoi")]
    _pourquoi: String,
}

/// La liste livree, dans l'ordre du COG.
pub fn liste() -> Resultat<Arc<Vec<Pays>>> {
    static CACHE: OnceLock<FormeGardee<Vec<Pays>>> = OnceLock::new();
    forme_en_cache(&CACHE, "pays-base.json", |base| {
        let b: BasePays = crate::forme::lire(base, "La liste des pays livrée avec le logiciel ne se lit pas : le signaler au développeur.")?;
        Ok(b.pays)
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
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaysDsn {
    pub code: String,
    pub libelle: String,
    /// Le code est aussi dans la table PAY (la meme sans la France, Monaco, les DROM et les COM).
    pub ne_a_l_etranger: bool,
}

/// `pays-dsn-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BasePaysDsn {
    #[serde(rename = "_quoi")]
    _quoi: String,
    #[serde(rename = "_source")]
    _source: String,
    pays: Vec<PaysDsn>,
}

/// La table « Pays » de la DSN, dans l'ordre de la nomenclature.
pub fn table_dsn() -> Resultat<Arc<Vec<PaysDsn>>> {
    static CACHE: OnceLock<FormeGardee<Vec<PaysDsn>>> = OnceLock::new();
    forme_en_cache(&CACHE, "pays-dsn-base.json", |base| {
        let b: BasePaysDsn = crate::forme::lire(base, "La table des pays de la DSN livrée avec le logiciel ne se lit pas : le signaler au développeur.")?;
        Ok(b.pays)
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

/// Un pays de naissance tel que la page le recoit : son code et son nom.
#[derive(Debug, Clone, Serialize)]
pub struct PaysServi {
    pub code: String,
    pub nom: String,
}

/// Ce que la page recoit pour le pays de naissance (`etat.pays_naissance`) : la table « Pays » de la DSN, triee par nom.
pub fn servir_naissance() -> Resultat<Vec<PaysServi>> {
    let cog = liste()?;
    let mut l: Vec<PaysServi> = table_dsn()?.iter().map(|p| PaysServi { code: p.code.clone(), nom: nom_de_naissance(p, &cog) }).collect();
    // La cle se calcule une fois par pays (`sort_by_cached_key`, stable comme `sort_by_key`), pas a chaque comparaison.
    l.sort_by_cached_key(|p| cle_de_nom(&p.nom));
    Ok(l)
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

/// Un pays de nationalite tel que la page le recoit : son code, son nom, la codification UE que le moteur en deduit, et
/// ce que la case de nationalite dit quand elle s'en deduit.
#[derive(Debug, Clone, Serialize)]
pub struct PaysDeNationalite {
    pub code: String,
    pub nom: String,
    pub codification_ue: &'static str,
    pub deduit_dit: String,
}

/// Ce que la page recoit (`etat.pays`) : la liste triee par nom, chaque pays avec la codification UE que le moteur
/// en deduit. La page l'affiche ; elle ne decide rien.
pub fn servir() -> Resultat<Vec<PaysDeNationalite>> {
    let mut l = (*liste()?).clone();
    l.sort_by_cached_key(|p| cle_de_nom(&p.nom));
    Ok(l.iter()
        .map(|p| PaysDeNationalite { code: p.code.clone(), nom: p.nom.clone(), codification_ue: codification(p), deduit_dit: format!("déduite de la nationalité ({})", p.nom) })
        .collect())
}

/// `S21.G00.30.015` du pays de naissance `code` d'une fiche (`qui` : celui qu'elle nomme) : le code, s'il est dans la
/// table « Pays » de la DSN ; un code blanc tel quel.
pub fn de_naissance(code: &str, qui: &str) -> Resultat<String> {
    if code.trim().is_empty() {
        return Ok(code.to_string());
    }
    match de_la_table_dsn(code)? {
        Some(p) => Ok(p.code),
        None => Err(inconnu_naissance(&format!("Le pays de naissance de {}", if qui.is_empty() { "ce salarié" } else { qui }), code)),
    }
}
