//! LES LIBELLES COURTS DES EXPORTS COMPTABLES : ce qui ne tient pas dans une zone du format du logiciel du comptable
//! (le libelle d'une ecriture, l'intitule d'un compte), la valeur courte que le moteur propose, et celle que la compagnie
//! a validee (`documents::libelles_courts`). L'export la lit ; sans elle, il se refuse et renvoie ici. Une route rend ce
//! qui depasse, une autre enregistre ; l'ecran (une boite des Exports) et le LLM font la meme chose.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use documents::libelles_courts::{self as lc, Depasse, FormatBorne, Quoi};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};

use crate::routes::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Place, Refus, Route, FORME, OPERATION};
use crate::routes::{en_json, lire_charge};

const FORMAT: Champ = Champ {
    nom: "format", libelle: Affiche::new("Format"), forme: Forme::Texte,
    sens: "le format des écritures (ximport, oxygene, sage, quadra, ebp) ; absent : celui du logiciel du comptable retenu (exports-periodes, compta)",
    avance: true,
};

const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::LibellesCourts,
        geste: "Dire les libellés trop longs pour le format du comptable, et leur valeur courte.",
        charge: &[FORMAT],
        exemple: r#"{}"#,
        rend: "format, nom, libelles [quoi (libelle, compte ou compagnie), cle, zone, long, largeur, propose (la proposition du moteur), court (la valeur validée, si elle tient)], phrase ; aucun format à zone bornée : libelles vide",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 160,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::LibellesCourtsEnregistrer,
        geste: "Valider la valeur courte de libellés trop longs pour le format du comptable.",
        charge: &[
            FORMAT,
            Champ {
                nom: "libelles", libelle: Affiche::new("Libellés courts"), forme: Forme::Objet,
                sens: "[{quoi, cle, court}] : quoi et cle tels que libelles-courts les rend, court la valeur validée (la proposition, ou une autre), de largeur caractères au plus",
                avance: false,
            },
        ],
        exemple: r#"{"format":"ximport","libelles":[]}"#,
        rend: "ce que rend libelles-courts, après l'enregistrement, et sa phrase ; une liste vide : rien n'est écrit, libelles vide",
        refus: &[
            Refus { classe: "ErreurOperation", corriger: "la valeur que la phrase nomme : vide, trop longue, ou d'un libellé qui ne dépasse pas ; rien n'est écrit" },
            FORME,
        ],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 160,
        ecran: ECRAN,
    },
];

/// Une route de ce module : sa charge lue en sa forme, sa reponse une structure.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::LibellesCourts => en_json(libelles_courts(ctx, lire_charge(charge)?)?),
        Route::LibellesCourtsEnregistrer => en_json(enregistrer(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// La charge de « libelles-courts » : le format, celui du logiciel retenu s'il manque.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeLire {
    #[serde(default)]
    format: Option<String>,
}

/// La charge de « libelles-courts-enregistrer » : le format, et les valeurs courtes validees.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEnregistrer {
    #[serde(default)]
    format: Option<String>,
    libelles: Vec<Valide>,
}

/// Une valeur courte validee : ce qu'elle remplace (`quoi`, `cle`, tels que rendus), et elle.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Valide {
    quoi: Quoi,
    cle: String,
    court: String,
}

/// Ce que rend la page des libelles courts.
#[derive(Debug, Serialize)]
struct Libelles {
    format: String,
    nom: String,
    libelles: Vec<Ligne>,
    phrase: String,
}

/// Une valeur trop longue : ce qu'elle est et sa cle, la zone en mots, son texte long, la place qu'elle a, la
/// proposition du moteur, et la valeur validee si elle tient.
#[derive(Debug, Serialize)]
struct Ligne {
    quoi: Quoi,
    cle: String,
    zone: String,
    long: String,
    largeur: usize,
    propose: String,
    court: Option<String>,
}

/// Le format de la charge, ou celui du logiciel retenu ; `None` s'il n'y en a pas.
fn format_de(ctx: &Contexte, format: Option<String>) -> R<Option<String>> {
    match format.filter(|f| !f.is_empty()) {
        Some(f) => Ok(Some(f)),
        None => documents::etats_compta::format_retenu(ctx),
    }
}

/// Ce qui depasse les zones de `format`, sur tous les mois de la compagnie.
fn depasses(ctx: &Contexte, format: &str) -> R<Vec<Depasse>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    documents::etats_compta::trop_longs(ctx, format, &departs)
}

fn zone(d: &Depasse) -> String {
    match d.quoi {
        Quoi::Libelle => format!("Libellé, {} caractères et le mois", d.largeur),
        Quoi::Compte => format!("Compte {}, {} caractères", d.cle, d.largeur),
        Quoi::Compagnie => format!("Nom de la compagnie, {} caractères", d.largeur),
    }
}

/// LIRE : ce qui depasse, sa proposition, sa valeur validee.
fn libelles_courts(ctx: &Contexte, c: ChargeLire) -> R<Libelles> {
    let format = format_de(ctx, c.format)?;
    let Some(borne) = format.as_deref().and_then(FormatBorne::de) else {
        let phrase = match &format {
            Some(f) => format!("Le format « {} » ne borne pas ses libellés : rien à raccourcir.", f),
            None => "Aucun logiciel du comptable retenu : rien à raccourcir.".to_string(),
        };
        return Ok(Libelles { format: format.unwrap_or_default(), nom: String::new(), libelles: Vec::new(), phrase });
    };
    let format = format.unwrap_or_default();
    let libelles: Vec<Ligne> = depasses(ctx, &format)?
        .into_iter()
        .map(|d| Ligne { quoi: d.quoi, zone: zone(&d), propose: lc::proposer(&d.long, d.largeur), cle: d.cle, long: d.long, largeur: d.largeur, court: d.court })
        .collect();
    let reste = libelles.iter().filter(|l| l.court.is_none()).count();
    let phrase = if libelles.is_empty() {
        format!("Tous les libellés tiennent dans le format {}.", borne.nom())
    } else if reste == 0 {
        format!("{} : {} libellés trop longs, chacun a sa valeur courte.", borne.nom(), libelles.len())
    } else {
        format!("{} : {} libellés trop longs, {} sans valeur courte. Garder ou changer chaque proposition, puis enregistrer.", borne.nom(), libelles.len(), reste)
    };
    Ok(Libelles { format, nom: borne.nom().to_string(), libelles, phrase })
}

/// Le refus d'une valeur : sa phrase, et sa cle a part (il se dit seul, sans ce qui manque pour demarrer).
fn faute(phrase: String, cle: &str) -> Echec {
    erreur("ErreurOperation", socle::echec::avec(phrase, serde_json::json!({"cle": cle})))
}

/// ENREGISTRER : chaque valeur se verifie (un libelle qui depasse, une valeur non vide qui tient) ; une seule faute et
/// rien ne s'ecrit.
fn enregistrer(ctx: &Contexte, c: ChargeEnregistrer) -> R<Libelles> {
    let format = format_de(ctx, c.format)?.unwrap_or_default();
    let Some(borne) = FormatBorne::de(&format) else {
        return Err(faute(format!("Le format « {} » ne borne pas ses libellés : il n'a pas de libellé court.", format), &format));
    };
    if c.libelles.is_empty() {
        let phrase = format!("Aucun libellé court à enregistrer pour {} : rien n'est écrit.", borne.nom());
        return Ok(Libelles { format, nom: borne.nom().to_string(), libelles: Vec::new(), phrase });
    }
    let trop: Vec<Depasse> = depasses(ctx, &format)?;
    let mut fichier = lc::lire(&ctx.racine)?;
    let courts = fichier.formats.entry(borne).or_default();
    let mut vus = BTreeSet::new();
    for v in &c.libelles {
        let Some(d) = trop.iter().find(|d| d.quoi == v.quoi && d.cle == v.cle) else {
            return Err(faute(format!("« {} » n'est pas un libellé trop long pour {} : rien n'est enregistré.", v.cle, borne.nom()), &v.cle));
        };
        let court = v.court.split_whitespace().collect::<Vec<_>>().join(" ");
        if court.is_empty() {
            return Err(faute(format!("La valeur courte de « {} » est vide : rien n'est enregistré.", d.long), &v.cle));
        }
        let n = court.chars().count();
        if n > d.largeur {
            return Err(faute(format!("« {} » a {} caractères, la zone en prend {} pour {} : la raccourcir, rien n'est enregistré.", court, n, d.largeur, borne.nom()), &v.cle));
        }
        if !vus.insert((v.quoi, v.cle.clone())) {
            return Err(faute(format!("« {} » est donné deux fois : rien n'est enregistré.", d.long), &v.cle));
        }
        courts.poser(v.quoi, v.cle.clone(), court);
    }
    if fichier.quoi.is_empty() {
        fichier.quoi = "Les libellés courts que la compagnie a validés, par format du logiciel du comptable (Exports, « Libellés courts »).".to_string();
    }
    lc::ecrire(&ctx.racine, &fichier)?;
    let mut r = libelles_courts(ctx, ChargeLire { format: Some(format) })?;
    let reste = r.libelles.iter().filter(|l| l.court.is_none()).count();
    let suite = if reste == 0 { format!("chaque libellé trop long pour {} a sa valeur courte", borne.nom()) } else { format!("{} encore sans valeur courte pour {}", reste, borne.nom()) };
    r.phrase = format!("{} enregistré{} ; {}.", crate::routes::compte(c.libelles.len(), "libellé court", "libellés courts"), if c.libelles.len() > 1 { "s" } else { "" }, suite);
    Ok(r)
}
