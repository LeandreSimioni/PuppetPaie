//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ce que la compagnie y
//! saisit, et seulement cela, s'ecrit dans `config/organismes.json`.
//!
//! Le reste se compose a la date de chaque periode (`droit::organismes`, `Contexte::organismes`) : la partie normative,
//! les contrats que les textes donnent entiers (ceux de toutes les compagnies, ceux de la convention, la CASC-SVP, les
//! contributions de branche du bareme). Une compagnie qui n'a rien a saisir n'a rien a enregistrer.
//!
//! LE FICHIER EST DATE comme toute la configuration (`droit::fiche::dater`) : sa cle `adhesions_prevoyance` est une
//! liste `{depuis, valeur}`. Enregistrer pour un mois ecrit ce qui change a partir du premier jour de ce mois
//! (`droit::fiche::dater`) ; ce qui precede garde ses contrats. La premiere composition vaut de tout temps. Les identifiants
//! d'adhesion (S21.G00.15.005) attribues ne bougent plus.

use droit::fiche::Lue as _;
use serde_json::{json, Value};

use droit::contexte::Contexte;
use droit::organismes::{attendus, composer, ligne, saisis, Adhesion, Attendu, Saisie};
use socle::date::Jour;
use documents::valeur::en_texte;
use std::collections::BTreeMap;
use socle::ecriture::ecrire_json;
use documents::valeur::vrai;
use socle::echec::Echec;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

type R<T> = Result<T, Echec>;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Organismes);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::OrganismesContrats,
        geste: "Dire les contrats de la DSN (prévoyance, santé…) d'un mois, et ce qui manque.",
        charge: &[Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois regardé, AAAA-MM ; le mois en cours sinon", avance: false }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "idcc, a_saisir, mois, contrats (une ligne par contrat attendu : libellé, organisme, periode), encarts",
        refus: &[OPERATION, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::OrganismesEnregistrer,
        geste: "Écrire, à partir d'un mois, les contrats de la DSN que la compagnie dit.",
        charge: &[
            Champ { nom: "contrats", libelle: Affiche::new("Contrats"), forme: Forme::Objet, sens: "{clé du contrat: {libelle, organisme}}, ce qui est saisi", avance: false },
            Champ { nom: "mois", libelle: Affiche::new("À partir de"), forme: Forme::Mois, sens: "le mois à partir duquel ils valent, AAAA-MM ; le mois en cours sinon", avance: false },
        ],
        exemple: r#"{"mois":"2026-11","contrats":{"prevoyance_permanents":{"libelle":"PREVOYANCE","organisme":"P0983"},"sante_permanents":{"libelle":"SANTE","organisme":"P0983"}}}"#,
        rend: "ce que organismes-contrats rend",
        refus: &[Refus { classe: "ErreurOperation", corriger: "la référence du contrat que la phrase nomme ; rien n'est écrit" }, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    let v = match nom {
        Route::OrganismesContrats => lire(ctx, charge)?,
        _ => enregistrer(ctx, charge)?,
    };
    // Ce qui reste a dire a la compagnie (`a_saisir`), sinon combien de contrats la DSN declare.
    let n = v["contrats"].as_array().map_or(0, Vec::len);
    let p = if v["a_saisir"] == json!(true) {
        format!("Contrats de la DSN : {} ; il en reste à saisir.", n)
    } else {
        format!("Contrats de la DSN : {} ; rien à saisir.", n)
    };
    let p = if nom == Route::OrganismesContrats { p } else { format!("Enregistré. {}", p) };
    let mut v = v;
    // Ce que l'ecran en dit : d'ou viennent les valeurs, les contrats de quel mois, ce qui manque ; la periode de chaque
    // contrat.
    let mut encarts = vec![crate::routes::encart("information", "", "Ce que la DSN déclare de vos adhésions de prévoyance et de santé. La référence du contrat et le code de l'organisme se lisent sur le certificat d'adhésion et sur la fiche de paramétrage DSN de l'organisme. Le reste (organismes payés, Urssaf, taux AT/MP, compte France Travail) vient de la loi, de la convention et de la fiche Entreprise.")];
    if let Some(m) = v["mois"].as_str().filter(|m| !m.is_empty()) {
        encarts.push(crate::routes::encart("information", "", format!("Les contrats en vigueur en {}.", socle::mois::Mois::en_clair(m))));
    }
    if v["a_saisir"] == json!(true) {
        encarts.push(crate::routes::encart("a_faire", "", "Une référence de contrat manque : la DSN refuse les salariés que ce contrat couvre tant qu'elle n'est pas enregistrée."));
    }
    for c in v["contrats"].as_array_mut().into_iter().flatten() {
        let (de, a) = (crate::routes::jour_en_clair(&json!(crate::routes::mot(&c["depuis"]).get(..10).unwrap_or(""))), crate::routes::jour_en_clair(&json!(crate::routes::mot(&c["jusqu_a"]).get(..10).unwrap_or(""))));
        c["periode"] = json!(match (de.is_empty(), a.is_empty()) {
            (false, false) => format!("Du {} au {}.", de, a),
            (false, true) => format!("Depuis le {}.", de),
            (true, false) => format!("Jusqu'au {}.", a),
            _ => String::new(),
        });
    }
    v["encarts"] = Value::Array(encarts);
    Ok(crate::routes::phrase_en_tete(v, p))
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some("ErreurOperation"), m)
}

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v).trim().to_string()
    } else {
        String::new()
    }
}

fn chemin(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join(socle::fichiers::CONFIG).join(droit::contexte::ORGANISMES)
}


/// Une ligne se montre-t-elle a cette compagnie ? Un contrat des permanents (`role` : prevoyance ou sante, les
/// affiliations d'un salarie paye au mois) ne se montre qu'a une compagnie qui en emploie (rien a remplir pour qui
/// n'emploie pas de permanent) : le curseur est `Contexte::emploie_des_permanents`, le meme que partout.
fn se_montre(ctx: &Contexte, spec: &Attendu) -> bool {
    spec.role.as_deref().unwrap_or_default().trim().is_empty() || ctx.emploie_des_permanents()
}

/// La periode que l'ecran regarde : le mois `mois` de la charge (celui de l'onglet Paie), sinon celui du jour.
fn periode(ctx: &Contexte, charge: &Value) -> R<(Jour, Jour)> {
    let mois = match txt(g(charge, "mois")) {
        m if m.is_empty() => ctx.aujourdhui.iso()[..7].to_string(),
        m => m,
    };
    let premier = socle::mois::Mois::lire(&mois).map(socle::mois::Mois::premier_jour).map_err(plain)?;
    let dernier = socle::mois::Mois::lire(&mois).map(socle::mois::Mois::dernier_jour).map_err(plain)?;
    Ok((premier, dernier))
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`Attendu::couvre`) ; `a_saisir` : une de ces lignes attend encore ce que la compagnie seule peut dire.
pub fn lire(ctx: &Contexte, charge: &Value) -> R<Value> {
    let periode = periode(ctx, charge)?;
    let ctx = &ctx.au(&periode.1).map_err(plain)?;
    let (saisi, _) = saisis(ctx).map_err(plain)?;
    let lignes: Vec<droit::organismes::Ligne> =
        attendus(ctx).map_err(plain)?.iter().filter(|s| se_montre(ctx, s) && s.couvre(&periode)).map(|s| ligne(s, &saisi)).collect();
    let a_saisir = lignes.iter().any(|l| l.libelle.is_none() || l.organisme.is_none());
    Ok(json!({
        "idcc": ctx.entreprise.idcc.texte().trim(),
        "a_saisir": a_saisir,
        "mois": periode.0.mois_civil().to_string(),
        "contrats": lignes,
    }))
}

/// `organismes-enregistrer` : `{"contrats": {cle: {libelle, organisme}}, "mois": "AAAA-MM"}` ; compose les adhesions
/// du mois (sous la configuration du mois) avec ce qui est saisi, refuse un contrat propose dont la reference manque,
/// ecrit ce qui change a partir du premier jour du mois (`droit::fiche::dater` : la premiere composition vaut de tout
/// temps), et rend la lecture. Ce qui precede le mois ne change pas.
pub fn enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let saisies: BTreeMap<String, Saisie> = socle::forme::lire::<Option<BTreeMap<String, Saisie>>>(g(charge, "contrats"), "Les contrats saisis ne se lisent pas : chacun dit sa référence (libelle) et son organisme.")
        .map_err(refus)?
        .unwrap_or_default();
    let periode = periode(ctx, charge)?;
    let du_mois = ctx.au(&periode.1).map_err(plain)?;
    let (saisi, prises) = saisis(&du_mois).map_err(plain)?;
    let compose = composer(&du_mois, &saisi, prises, &saisies, Some(&periode)).map_err(refus)?;
    // Ce qu'un texte donne entier (sa reference de contrat et son identifiant) ne s'ecrit pas : il se recompose a la
    // date de chaque mois, sous la convention de ce mois.
    let fixes: Vec<String> = attendus(&du_mois).map_err(plain)?.iter().filter(|s| s.libelle_donne().is_some()).map(|s| s.cle.trim().to_string()).collect();
    let adhesions: Vec<Adhesion> = compose.adhesions_prevoyance.into_iter().filter(|a| !fixes.contains(&a.cle)).collect();
    let (en_vigueur, _) = saisis(&du_mois.au(&periode.0).map_err(plain)?).map_err(plain)?;
    // Le fichier ne garde que la saisie : tout autre cle se recompose des textes (`droit::organismes`).
    let mut date = droit::organismes::fichier_organismes(ctx).map_err(plain)?;
    date.quoi = Some(
        "Ce que la compagnie a saisi dans Configuration › Organismes et contrats, et les identifiants d'adhesion \
attribues, date : chaque cle est une liste {depuis, valeur}. Le reste se compose des textes livres a la date de chaque \
mois. Ne pas modifier a la main."
            .to_string(),
    );
    // Rien de saisi en vigueur s'ecrit aussi : la composition du mois devient la saisie de la periode.
    if en_vigueur.is_empty() || en_vigueur != adhesions {
        droit::fiche::dater(date.adhesions_prevoyance.get_or_insert_with(Vec::new),Some(adhesions), Some(&periode.0), None);
    }
    let p = chemin(ctx);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| plain(e.to_string()))?;
    }
    crate::routes::mois::sauver_avant(&p)?;
    ecrire_json(&p, &date, 2).map_err(plain)?;
    lire(ctx, charge)
}
