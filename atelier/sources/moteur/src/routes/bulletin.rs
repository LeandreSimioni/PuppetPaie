//! Configuration › Modèle de bulletin : simplifie ou complet, chacun montre sur une paie fictive ; la compagnie choisit.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::fiches::refus;
use droit::contexte::Contexte;
use droit::fiche::Case;
use documents::bulletin::{jeux_et_description, Modele, CASE_MODELE};
use socle::fichiers::dossier_du_code;
use socle::echec::{Echec, R};
use socle::fichiers::lire_json;

const MODELE: Champ = Champ { nom: "modele", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "« simplifié » ou « complet »", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Bulletin);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::BulletinModeles,
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
        route: Route::BulletinModeleApercu,
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
        route: Route::BulletinModeleChoisir,
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
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::BulletinModeles => {
            let super::SansCharge {} = super::lire_charge(charge)?;
            super::en_json(modeles(ctx)?)
        }
        Route::BulletinModeleApercu => {
            let modele = modele_demande(&super::lire_charge(charge)?)?;
            let pdf = apercu_pdf(modele).map_err(refus)?;
            super::en_json(ApercuModele {
                phrase: format!("Bulletin d'exemple dans le modèle {}, en PDF.", modele.cle()),
                modele: modele.cle().to_string(),
                pdf: socle::base64::ecrire(&pdf),
            })
        }
        Route::BulletinModeleChoisir => super::en_json(choisir(ctx, &super::lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ le modele du bulletin

/// La charge de `bulletin-modele-apercu` et de `bulletin-modele-choisir` : le modele, par sa cle.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeModele {
    pub(crate) modele: String,
}

/// Ce que rend `bulletin-modele-apercu` : le PDF d'exemple, en base64, et la phrase. La fenetre le lit aussi
/// (`interface::Fenetre::apercu_bulletin`).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApercuModele {
    phrase: String,
    modele: String,
    pub(crate) pdf: String,
}

/// Le modele que la charge nomme : un modele connu, rien d'autre.
fn modele_demande(c: &ChargeModele) -> R<Modele> {
    let cle = c.modele.trim();
    Modele::TOUS.iter().copied().find(|m| m.cle() == cle).ok_or_else(|| {
        refus(format!("Modèle de bulletin inconnu : « {} ». Connus : « simplifié », « complet ».", c.modele))
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

/// Un modele du bulletin, tel que l'onglet le montre : sa cle, son nom, sa description, son apercu, et sa pastille s'il
/// est celui de la compagnie.
#[derive(Debug, Clone, serde::Serialize)]
struct ModeleDit {
    cle: &'static str,
    nom: &'static str,
    description: &'static str,
    apercu_dit: String,
    apercu: String,
    pastille: Option<socle::ecran::Pastille>,
}

/// Un salarie qui a demande l'autre modele (la case de sa fiche).
#[derive(Debug, Clone, serde::Serialize)]
struct SalarieAutrement {
    cle: String,
    nom: String,
    modele: &'static str,
}

/// Ce que l'onglet dit autour des modeles.
#[derive(Debug, Clone, serde::Serialize)]
struct EcranDesModeles {
    intro: &'static str,
    autrement: Option<String>,
    ensuite: &'static str,
}

/// LES MODELES DU BULLETIN (`bulletin-modeles`, et `bulletin-modele-choisir` apres le choix) : la phrase, le modele
/// choisi et s'il est celui du logiciel, les modeles connus, les salaries qui en ont un autre, et ce que l'onglet en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct ModelesDuBulletin {
    phrase: String,
    choisi: &'static str,
    par_defaut: bool,
    modeles: Vec<ModeleDit>,
    salaries_autrement: Vec<SalarieAutrement>,
    ecran: EcranDesModeles,
}

fn modeles(ctx: &Contexte) -> R<ModelesDuBulletin> {
    let choisi = Modele::du_choix(ctx.entreprise.modele_bulletin.dite());
    let modele = choisi.unwrap_or(Modele::Simplifie);
    let mut autrement = Vec::new();
    for (cle, sal) in &ctx.salaries {
        if let Some(m) = documents::bulletin::modele_du_salarie(sal).map_err(refus)? {
            if m != modele {
                let nom = format!("{} {}", sal.prenom_affichage.texte(), sal.nom_affichage.texte()).trim().to_string();
                let nom = if nom.is_empty() { cle.to_string() } else { nom };
                autrement.push(SalarieAutrement { cle: cle.to_string(), nom, modele: m.cle() });
            }
        }
    }
    let liste: Vec<ModeleDit> = Modele::TOUS
        .iter()
        .map(|m| ModeleDit {
            cle: m.cle(),
            nom: m.nom(),
            description: m.description(),
            apercu_dit: format!("Aperçu du modèle {}", m.nom()),
            apercu: adresse_apercu(*m),
            pastille: (*m == modele).then(|| socle::ecran::Pastille::de("fait", if choisi.is_none() { "celui de la compagnie (par défaut)" } else { "celui de la compagnie" })),
        })
        .collect();
    let dits: Vec<String> = autrement.iter().map(|s| format!("{} ({})", s.nom, s.modele)).collect();
    Ok(ModelesDuBulletin {
        phrase: format!("Modèle de bulletin : {}{}.", modele.cle(), if choisi.is_none() { " (celui du logiciel)" } else { "" }),
        choisi: modele.cle(),
        par_defaut: choisi.is_none(),
        modeles: liste,
        salaries_autrement: autrement,
        ecran: EcranDesModeles {
            intro: "Le bulletin que la compagnie remet à ses salariés. Chaque modèle est montré sur la même paie fictive (une compagnie et un salarié inventés) : mêmes lignes du modèle réglementaire, mêmes totaux, même net. Un salarié qui demande l'autre modèle l'obtient par la case « Modèle de bulletin demandé par le salarié » de sa fiche, dans Avancé.",
            autrement: (!dits.is_empty()).then(|| format!("Ces salariés ont demandé l'autre modèle (case de leur fiche, Avancé) et le gardent : {}.", dits.join(", "))),
            ensuite: "Le choix vaut pour les bulletins produits ensuite ; ceux déjà remis ne changent pas.",
        },
    })
}

// ------------------------------------------------------------------ l'apercu

/// La compagnie fictive de l'apercu, livree avec le logiciel.
pub fn dossier_apercu() -> PathBuf {
    dossier_du_code().join("apercu-bulletin")
}

/// LA PAIE FICTIVE DE L'APERCU (`apercu-bulletin/saisie.json`) : ce qu'elle est, son mois et ses cartes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SaisieDeLApercu {
    #[allow(dead_code)]
    source: String,
    mois: socle::mois::Mois,
    engagements: Vec<engagement::saisie::Carte>,
}

/// La description du bulletin de la paie fictive sous `modele` : le moteur du jour, la compagnie fictive, rien d'ecrit.
pub fn apercu_description(modele: Modele) -> Result<Value, String> {
    let racine = dossier_apercu();
    let lue: SaisieDeLApercu = socle::forme::lire(&lire_json(&racine.join("saisie.json"))?, "La paie de l'aperçu ne se lit pas.")?;
    let saisie = engagement::saisie::VueDuMois { mois: lue.mois, taux_cachet: None, engagements: lue.engagements };
    let mois = saisie.mois.to_string();
    let ctx = Contexte::nouveau(&racine, Some(saisie.mois.dernier_jour()))?;
    let s = engagement::pour_l_ecran(&ctx, &saisie)?;
    let mut fiche = ctx.entreprise.clone();
    fiche.modele_bulletin = Case::Dite(modele.choix());
    // Ni le logo ni la signature du poste sur une paie fictive : ses gabarits sont un dossier qui n'existe pas.
    let ent = ctx.pour_un_modele(&fiche);
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

fn choisir(ctx: &Contexte, charge: &ChargeModele) -> R<ModelesDuBulletin> {
    let modele = modele_demande(charge)?;
    // Le modele vaut pour les documents faits a partir d'aujourd'hui ; ceux d'avant gardent le leur.
    crate::routes::fiches::poser_entreprise(ctx, CASE_MODELE, json!(modele.cle()), Some(ctx.aujourdhui))?;
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(refus)?;
    let mut v = modeles(&neuf)?;
    v.phrase = format!("Modèle choisi : {}, pour les documents faits à partir d'aujourd'hui.", v.choisi);
    Ok(v)
}
