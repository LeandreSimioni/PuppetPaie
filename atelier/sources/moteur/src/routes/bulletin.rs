//! Configuration › Modèle de bulletin : simplifie ou complet, chacun montre sur une paie fictive ; la compagnie choisit.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::routes::catalogue::{Acte, Champ, Ecran, Fiche, Place, OPERATION};
use crate::routes::fiches::refus;
use droit::contexte::Contexte;
use documents::bulletin::{jeux_et_description, Modele, CASE_MODELE};
use socle::fichiers::dossier_du_code;
use socle::echec::{Echec, R};
use socle::fichiers::lire_json;

const MODELE: Champ = Champ { nom: "modele", sens: "« simplifié » ou « complet »", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Bulletin);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "bulletin-modeles",
        geste: "Dire le modèle du bulletin choisi, et les salariés qui en ont un autre.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "modele, les modèles connus, autrement (les salariés à un autre modèle)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "bulletin-modele-apercu",
        geste: "Montrer un bulletin d'exemple dans un modèle.",
        charge: &[MODELE],
        exemple: r#"{"modele":"complet"}"#,
        rend: "modele, pdf (en base64)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        nom: "bulletin-modele-choisir",
        geste: "Choisir le modèle du bulletin des documents faits dès aujourd'hui.",
        charge: &[MODELE],
        exemple: r#"{"modele":"simplifié"}"#,
        rend: "ce que bulletin-modeles rend",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "bulletin-modeles" => modeles(ctx).map(|mut v| {
            // Ce que l'ecran des modeles dit : a quoi sert le choix, les salaries qui en gardent un autre, ce que le choix change.
            let autrement: Vec<String> = v["salaries_autrement"].as_array().into_iter().flatten().map(|s| format!("{} ({})", super::mot(&s["nom"]), super::mot(&s["modele"]))).collect();
            v["ecran"] = json!({
                "intro": "Le bulletin que la compagnie remet à ses salariés. Chaque modèle est montré sur la même paie fictive (une compagnie et un salarié inventés) : mêmes lignes du modèle réglementaire, mêmes totaux, même net. Un salarié qui demande l'autre modèle l'obtient par la case « Modèle de bulletin demandé par le salarié » de sa fiche, dans Avancé.",
                "autrement": if autrement.is_empty() { Value::Null } else { json!(format!("Ces salariés ont demandé l'autre modèle (case de leur fiche, Avancé) et le gardent : {}.", autrement.join(", "))) },
                "ensuite": "Le choix vaut pour les bulletins produits ensuite ; ceux déjà remis ne changent pas.",
            });
            let p = format!("Modèle de bulletin : {}{}.", socle::valeur::valeur_en_texte(&v["choisi"]), if v["par_defaut"] == json!(true) { " (celui du logiciel)" } else { "" });
            super::phrase_en_tete(v, p)
        }),
        "bulletin-modele-apercu" => {
            let modele = modele_demande(charge)?;
            let pdf = apercu_pdf(modele).map_err(refus)?;
            Ok(json!({"phrase": format!("Bulletin d'exemple dans le modèle {}, en PDF.", modele.cle()), "modele": modele.cle(), "pdf": socle::base64::ecrire(&pdf)}))
        }
        "bulletin-modele-choisir" => choisir(ctx, charge).map(|v| {
            let p = format!("Modèle choisi : {}, pour les documents faits à partir d'aujourd'hui.", socle::valeur::valeur_en_texte(&v["choisi"]));
            super::phrase_en_tete(v, p)
        }),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ le modele du bulletin

/// Le modele que la charge nomme (`modele`) : un modele connu, rien d'autre.
fn modele_demande(charge: &Value) -> R<Modele> {
    let v = charge.get("modele").cloned().unwrap_or(Value::Null);
    let cle = v.as_str().map(str::trim).unwrap_or_default();
    Modele::TOUS.iter().copied().find(|m| m.cle() == cle).ok_or_else(|| {
        refus(format!("Modèle de bulletin inconnu : {}. Connus : « simplifié », « complet ».", v))
    })
}

/// L'adresse ou la fenetre sert l'apercu d'un modele (`interface::Fenetre::apercu_bulletin`).
pub fn adresse_apercu(m: Modele) -> String {
    format!("/apercu/bulletin?modele={}", if m == Modele::Simplifie { "simplifie" } else { "complet" })
}

/// Le modele d'une adresse d'apercu (`simplifie`, `complet` : sans accent dans l'adresse).
pub fn modele_de_l_adresse(v: &str) -> Option<Modele> {
    match v {
        "simplifie" | "simplifié" => Some(Modele::Simplifie),
        "complet" => Some(Modele::Complet),
        _ => None,
    }
}

// ------------------------------------------------------------------ bulletin-modeles

fn modeles(ctx: &Contexte) -> R<Value> {
    let choisi = Modele::de_la_fiche(&ctx.entreprise, "entreprise").map_err(refus)?;
    let modele = choisi.unwrap_or(Modele::Simplifie);
    // Les salaries dont la case dit un autre modele que la compagnie : la page les nomme, la decision reste au moteur.
    let mut autrement = Vec::new();
    for (cle, sal) in &ctx.salaries {
        if let Some(m) = Modele::de_la_fiche(sal, &format!("du salarié {}", cle)).map_err(refus)? {
            if m != modele {
                let champ = |k: &str| sal.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let nom = format!("{} {}", champ("prenom_affichage"), champ("nom_affichage")).trim().to_string();
                let nom = if nom.is_empty() { cle.clone() } else { nom };
                autrement.push(json!({"cle": cle, "nom": nom, "modele": m.cle()}));
            }
        }
    }
    let liste: Vec<Value> = Modele::TOUS
        .iter()
        .map(|m| json!({"cle": m.cle(), "nom": m.nom(), "description": m.description(), "apercu": adresse_apercu(*m),
            // Le modele de la compagnie : sa pastille, celle d'un choix fait.
            "pastille": if *m == modele { super::pastille("fait", if choisi.is_none() { "celui de la compagnie (par défaut)" } else { "celui de la compagnie" }) } else { Value::Null }}))
        .collect();
    Ok(json!({
        "choisi": modele.cle(),
        "par_defaut": choisi.is_none(),
        "modeles": liste,
        "salaries_autrement": autrement,
    }))
}

// ------------------------------------------------------------------ l'apercu

/// La compagnie fictive de l'apercu, livree avec le logiciel.
pub fn dossier_apercu() -> PathBuf {
    dossier_du_code().join("apercu-bulletin")
}

/// La description du bulletin de la paie fictive sous `modele` : le moteur du jour, la compagnie fictive, rien d'ecrit.
pub fn apercu_description(modele: Modele) -> Result<Value, String> {
    let racine = dossier_apercu();
    let saisie = lire_json(&racine.join("saisie.json"))?;
    let mois = saisie.get("mois").and_then(Value::as_str).ok_or("La paie de l'aperçu ne dit pas son mois.")?.to_string();
    let ctx = Contexte::nouveau(&racine, Some(socle::date::dernier_du_mois(&mois)?))?;
    let s = engagement::pour_l_ecran(&ctx, &saisie)?;
    let mut ent = ctx.entreprise.clone();
    let o: &mut Map<String, Value> = ent.as_object_mut().ok_or("La fiche de la compagnie de l'aperçu n'est pas un objet.")?;
    o.insert(CASE_MODELE.into(), json!(modele.cle()));
    // Ni le logo ni la signature du poste sur une paie fictive : ses gabarits sont un dossier qui n'existe pas.
    o.insert("_gabarits".into(), json!(racine.join("gabarits").to_string_lossy()));
    // Une paie de l'ecran n'a pas de numero de bulletin (il se reserve au depart) : l'apercu montre celui d'un premier
    // bulletin de l'annee.
    let numeros: Vec<Option<String>> = (1..=s.paies.len()).map(|n| Some(format!("{}/{}", n, &mois[..4]))).collect();
    let departs = engagement::departs::Departs::default();
    let passe = documents::bulletin::passe_des_bulletins(&ctx, &s.paies, &mois, &departs)?;
    jeux_et_description(&ctx, &s.paies, &numeros, &ent, &mois, None, false, &HashMap::new(), &departs, &passe)
}

/// Le PDF de la paie fictive sous `modele`.
pub fn apercu_pdf(modele: Modele) -> Result<Vec<u8>, String> {
    documents::fabrique::fabriquer_octets(&apercu_description(modele)?)
}

// ------------------------------------------------------------------ bulletin-modele-choisir

fn choisir(ctx: &Contexte, charge: &Value) -> R<Value> {
    let modele = modele_demande(charge)?;
    // Le modele vaut pour les documents faits a partir d'aujourd'hui ; ceux d'avant gardent le leur.
    crate::routes::fiches::poser_entreprise(ctx, CASE_MODELE, json!(modele.cle()), Some(ctx.aujourdhui))?;
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(refus)?;
    modeles(&neuf)
}
