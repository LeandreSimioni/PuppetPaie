//! Le modele du bulletin : ce que la page de Configuration montre et enregistre. La decision vit dans `bulletin::Modele` et `bulletin::modele_du_bulletin`
//! (regle 2) ; ces routes ne font que la lire, l'apercevoir et poser le choix de la compagnie.
//!
//! - `bulletin-modeles` (lecture) : les modeles, celui que la compagnie a choisi, les salaries dont la fiche en demande
//!   un autre (leur case, dans Avance, l'emporte) et l'adresse de l'apercu de chacun.
//! - `bulletin-modele-apercu` (lecture) : le PDF d'une paie FICTIVE sous un modele, en base64. La paie est celle de la
//!   compagnie fictive livree (`donnees/apercu-bulletin/` : ni NIR, ni personne, ni spectacle reels ; regle 9a), calculee
//!   par le moteur du jour sur le socle du bareme ; rien n'y est ecrit. La fenetre la sert en PDF a
//!   `/apercu/bulletin?modele=...` (`interface::Fenetre::apercu_bulletin`), que la page affiche comme les pieces d'une
//!   compagnie fictive.
//! - `bulletin-modele-choisir` (ecriture) : pose `modele_bulletin` dans `config/entreprise.json` (copie `avant/` d'abord).
//!
//! Les bulletins deja produits ne changent pas : on renvoie toujours l'archive (V1p) ; seuls les bulletins produits apres
//! le choix le suivent.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use documents::bulletin::{jeux_et_description, Modele, CASE_MODELE};
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, lire_json};
use socle::echec::Echec;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["bulletin-modeles", "bulletin-modele-apercu", "bulletin-modele-choisir"];

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "bulletin-modeles" => modeles(ctx),
        "bulletin-modele-apercu" => {
            let modele = modele_demande(charge)?;
            let pdf = apercu_pdf(modele).map_err(refus)?;
            Ok(json!({"modele": modele.cle(), "pdf": socle::base64::ecrire(&pdf)}))
        }
        "bulletin-modele-choisir" => choisir(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

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
        .map(|m| json!({"cle": m.cle(), "nom": m.nom(), "description": m.description(), "apercu": adresse_apercu(*m)}))
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
    let jour = saisie.get("date_paiement").and_then(Value::as_str).ok_or("La paie de l'aperçu ne dit pas sa date de paiement.")?;
    let ctx = Contexte::nouveau(&racine, Some(Jour::lire(jour)?))?;
    let s = engagement::pour_l_ecran(&ctx, &saisie)?;
    let mut ent = ctx.entreprise.clone();
    let o: &mut Map<String, Value> = ent.as_object_mut().ok_or("La fiche de la compagnie de l'aperçu n'est pas un objet.")?;
    o.insert(CASE_MODELE.into(), json!(modele.cle()));
    // Ni le logo ni la signature du poste sur une paie fictive : ses gabarits sont un dossier qui n'existe pas.
    o.insert("_gabarits".into(), json!(racine.join("gabarits").to_string_lossy()));
    // Une paie de l'ecran n'a pas de numero de bulletin (il se reserve au depart) : l'apercu montre celui d'un premier
    // bulletin de l'annee.
    let numeros: Vec<Option<String>> = (1..=s.paies.len()).map(|n| Some(format!("{}/{}", n, &mois[..4]))).collect();
    jeux_et_description(&ctx, &s.paies, &numeros, &ent, &mois, None, false, &HashMap::new())
}

/// Le PDF de la paie fictive sous `modele`.
pub fn apercu_pdf(modele: Modele) -> Result<Vec<u8>, String> {
    documents::fabrique::fabriquer_octets(&apercu_description(modele)?)
}

// ------------------------------------------------------------------ bulletin-modele-choisir

fn choisir(ctx: &Contexte, charge: &Value) -> R<Value> {
    let modele = modele_demande(charge)?;
    // Le modele vaut pour les documents faits a partir d'aujourd'hui ; ceux d'avant gardent le leur.
    crate::routes::ecritures::poser_entreprise(ctx, CASE_MODELE, json!(modele.cle()), Some(ctx.aujourdhui))?;
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(refus)?;
    modeles(&neuf)
}
