//! Configuration › Métiers : les metiers que la compagnie coche, ceux de sa convention et ceux qu'elle prend aux autres.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::fiches::{creer_dossier, ecrire, plain, refus};
use crate::routes::mois::sauver_avant;
use crate::routes::{en_json, lire_charge};
use droit::contexte::Contexte;
use droit::emplois::{self, MetierDuDeroulant};
use socle::echec::{erreur, Echec, R};

const METIERS_REND: &str = "emplois (les métiers proposés) et emplois_autres (ceux des autres conventions)";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Metiers);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::MetiersCatalogue,
        geste: "Dire les métiers du catalogue, ceux de la convention et les autres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "metiers (le catalogue, chacun dit retenu ou non)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MetierRetenu,
        geste: "Proposer un métier du catalogue dans les listes, ou ne plus le proposer.",
        charge: &[Champ { nom: "code", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier au catalogue", avance: false }, Champ { nom: "retenu", libelle: Affiche::new("Le proposer"), forme: Forme::OuiNon, sens: "vrai : le proposer", avance: false }],
        exemple: r#"{"code":"CHE350","retenu":true}"#,
        rend: METIERS_REND,
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::MetiersCatalogue => {
            let super::SansCharge {} = lire_charge(charge)?;
            en_json(metiers_catalogue(ctx)?)
        }
        Route::MetierRetenu => en_json(metier_retenu(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `config/metiers-ajoutes.json` : les metiers que la compagnie a coches ou decoches dans Configuration > Metiers.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetiersAjoutes {
    quoi: String,
    /// Pris a une autre convention collective.
    pub(crate) codes: Vec<String>,
    /// Decoches dans la sienne.
    pub(crate) retires: Vec<String>,
}

const QUOI_METIERS: &str = "Les metiers que cette compagnie a coches ou decoches dans Configuration > Metiers : `codes`, pris a une autre convention collective ; `retires`, decoches dans la sienne. Preference d'affichage : aucun calcul ne change. Le catalogue est livre avec le logiciel (donnees/emplois-base.json).";

/// Les metiers que la compagnie a ajoutes ou retires, sous sa racine.
pub(crate) const METIERS_AJOUTES: &str = "config/metiers-ajoutes.json";

impl MetiersAjoutes {
    /// LA SEULE LECTURE de `config/metiers-ajoutes.json` ; rien de pose s'il n'existe pas.
    pub(crate) fn lire(racine: &Path) -> R<MetiersAjoutes> {
        let p = racine.join(METIERS_AJOUTES);
        if !p.exists() {
            return Ok(MetiersAjoutes::default());
        }
        let v = socle::fichiers::lire_json(&p).map_err(|m| erreur("ValueError", m))?;
        socle::forme::lire(&v, "Les métiers retenus par la compagnie ne se lisent pas : effacez leur fichier, puis cochez-les de nouveau dans Configuration › Métiers.").map_err(|m| {
            let (dite, mut plus) = socle::echec::separer(&m);
            plus.insert("chemin".into(), Value::String(p.to_string_lossy().into_owned()));
            erreur("ValueError", socle::echec::avec(dite, Value::Object(plus)))
        })
    }

    fn ecrire(&self, ctx: &Contexte) -> R<()> {
        let p = ctx.racine.join(METIERS_AJOUTES);
        if let Some(parent) = p.parent() {
            creer_dossier(parent)?;
        }
        sauver_avant(&p)?;
        ecrire(&p, en_json(MetiersAjoutes { quoi: QUOI_METIERS.into(), codes: self.codes.clone(), retires: self.retires.clone() })?)
    }

    /// Ce metier est-il dans le deroulant de cette compagnie ? Ceux de sa convention le sont d'office ; elle en ajoute
    /// d'une autre, elle en retire des siens (Configuration > Metiers).
    fn visible(&self, m: &MetierDuDeroulant, idcc: &str) -> bool {
        !self.retires.contains(&m.code) && (de_la_convention(m, idcc) || self.codes.contains(&m.code))
    }
}

/// Le metier est-il de la convention `idcc` ? Sans IDCC (une compagnie qui s'installe), ou sans convention dite, rien ne
/// se cache.
fn de_la_convention(m: &MetierDuDeroulant, idcc: &str) -> bool {
    m.conventions.is_empty() || idcc.is_empty() || m.conventions.iter().any(|c| c == idcc)
}

/// Les conventions d'un metier en mots : « Spectacle vivant privé (3090) » quand `idcc` est dit.
fn conventions_dites(m: &MetierDuDeroulant, noms: &BTreeMap<String, String>, avec_l_idcc: bool) -> String {
    m.conventions
        .iter()
        .map(|c| {
            let nom = noms.get(c).cloned().unwrap_or_else(|| c.clone());
            if avec_l_idcc { format!("{} ({})", nom, c) } else { nom }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Les familles, et l'ordre ou les montrer : les artistes en tete.
fn rang_des_familles(familles: &[String]) -> HashMap<String, usize> {
    familles.iter().enumerate().map(|(i, f)| (f.clone(), i)).collect()
}

/// `metiers` : le catalogue des metiers que le deroulant de cette compagnie montre, groupe par famille (les artistes en
/// tete), chaque convention voyant ses metiers et seulement eux.
pub(crate) fn metiers(ctx: &Contexte) -> R<Vec<MetierDuDeroulant>> {
    let ordre = rang_des_familles(&emplois::familles().map_err(plain)?);
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let mut liste: Vec<MetierDuDeroulant> = emplois::catalogue(&idcc).map_err(plain)?.into_iter().filter(|m| retenus.visible(m, &idcc)).collect();
    liste.sort_by_cached_key(|m| (ordre.get(&m.famille).copied().unwrap_or(ordre.len()), m.libelle.to_lowercase()));
    Ok(liste)
}

/// Un metier d'une autre convention, que la compagnie peut ajouter a son deroulant : `de` dit lesquelles.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MetierAutre {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    de: String,
}

/// `metiers_autres_conventions` : les metiers des AUTRES conventions, ceux que la compagnie peut ajouter a son deroulant.
pub(crate) fn metiers_autres_conventions(ctx: &Contexte) -> R<Vec<MetierAutre>> {
    let noms = emplois::noms_des_conventions().map_err(plain)?;
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let mut out: Vec<MetierAutre> =
        emplois::catalogue(&idcc).map_err(plain)?.into_iter().filter(|m| !retenus.visible(m, &idcc)).map(|m| MetierAutre { de: conventions_dites(&m, &noms, true), metier: m }).collect();
    out.sort_by(|a, b| (&a.de, a.metier.libelle.to_lowercase()).cmp(&(&b.de, b.metier.libelle.to_lowercase())));
    Ok(out)
}

/// Un metier de l'onglet Metiers : s'il est retenu, s'il est de la convention de la compagnie, et sinon de laquelle.
#[derive(Debug, Serialize)]
struct MetierDeLOnglet {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    retenu: bool,
    de_la_convention: bool,
    de: String,
}

/// Ce que rend `metiers-catalogue`.
#[derive(Debug, Serialize)]
struct MetiersDuCatalogue {
    phrase: String,
    metiers: Vec<MetierDeLOnglet>,
    familles: Vec<String>,
}

/// L'onglet Metiers : tout le catalogue, chaque metier coche ou non.
fn metiers_catalogue(ctx: &Contexte) -> R<MetiersDuCatalogue> {
    let familles = emplois::familles().map_err(plain)?;
    let ordre = rang_des_familles(&familles);
    let noms = emplois::noms_des_conventions().map_err(plain)?;
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let mut metiers: Vec<MetierDeLOnglet> = emplois::catalogue(&idcc)
        .map_err(plain)?
        .into_iter()
        .map(|m| {
            let conv = de_la_convention(&m, &idcc);
            MetierDeLOnglet { retenu: retenus.visible(&m, &idcc), de_la_convention: conv, de: if conv { String::new() } else { conventions_dites(&m, &noms, false) }, metier: m }
        })
        .collect();
    metiers.sort_by_cached_key(|m| (ordre.get(&m.metier.famille).copied().unwrap_or(ordre.len()), m.metier.libelle.to_lowercase()));
    let phrase = format!(
        "Métiers du catalogue : {}. Ceux qui sont cochés se proposent dans la fiche d'un salarié et dans un engagement ; ceux de votre convention le sont d'office.",
        metiers.len()
    );
    Ok(MetiersDuCatalogue { phrase, metiers, familles })
}

// ---------------------------------------------------------------- les metiers

/// La charge de `metier-retenu`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetierARetenir {
    code: String,
    retenu: bool,
}

/// Ce que rend `metier-retenu` : les metiers proposes et ceux des autres conventions.
#[derive(Debug, Serialize)]
struct EmploisRendus {
    phrase: String,
    emplois: Vec<MetierDuDeroulant>,
    emplois_autres: Vec<MetierAutre>,
}

fn metier_retenu(ctx: &Contexte, c: MetierARetenir) -> R<EmploisRendus> {
    let idcc = ctx.entreprise.idcc.texte().to_string();
    let catalogue = emplois::catalogue(&idcc).map_err(plain)?;
    let Some(m) = catalogue.iter().find(|m| m.code == c.code) else {
        return Err(refus(format!("Metier hors catalogue : {}.", c.code)));
    };
    let conv = de_la_convention(m, &idcc);
    let mut retenus = MetiersAjoutes::lire(&ctx.racine)?;
    retenus.codes.retain(|x| *x != c.code);
    retenus.retires.retain(|x| *x != c.code);
    if c.retenu && !conv {
        retenus.codes.push(c.code.clone());
    }
    if !c.retenu && conv {
        retenus.retires.push(c.code.clone());
    }
    retenus.ecrire(ctx)?;
    let emplois = metiers(ctx)?;
    Ok(EmploisRendus { phrase: format!("Métiers retenus par la compagnie : {}.", emplois.len()), emplois, emplois_autres: metiers_autres_conventions(ctx)? })
}
