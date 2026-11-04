//! LES MODELES DE CONTRAT DE LA COMPAGNIE : ce que Configuration › Modeles de contrat montre et fait.
//!
//! Chaque nature de contrat a son modele livre (`contrat_ecrit` de `contrats-base.json`) ; une compagnie qui a le sien le
//! pose dans son dossier `gabarits/`, sous le meme nom : il est pris a la place du livre (`socle::fichiers::modele_de`),
//! et sauvegarde avec elle. Un modele propre se verifie avant d'etre pose : sa grammaire, ses champs (un champ que le
//! logiciel ne connait pas ne ferait aucun contrat), et les mentions que la loi exige d'un contrat ecrit (code du travail
//! L1242-12 et R1221-34, `documents::contrat_ecrit::refuser_les_mentions_absentes`) ; s'il manque quelque chose, rien n'est
//! pose, et le refus dit quoi. Les clauses particulieres de la compagnie (`clauses_particulieres` de sa fiche) s'ecrivent
//! ici une fois et se reprennent dans chaque contrat.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::engagement::texte_requis;
use droit::contexte::Contexte;
use droit::natures::Nature;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::dossier_du_code;
use crate::routes::catalogue::{Ecran, Place};

const NATURE: Champ = Champ { nom: "nature", libelle: Affiche::new("Nature du contrat"), forme: Forme::Texte, sens: "la nature du contrat : cddu_spectacle, cdd, cdi…", avance: false };
const QUOI: Champ = Champ { nom: "quoi", libelle: Affiche::new("Contrat ou avenant"), forme: Forme::Texte, sens: "contrat ou avenant", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::ModelesContrat);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ModelesContrat,
        geste: "Lister les modèles de contrat et d'avenant, livrés ou propres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "natures (chacune : ses modèles, livrés ou propres), champs permis, clauses de la compagnie",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModeleContratLivre,
        geste: "Lire le texte d'un modèle livré, pour en partir.",
        charge: &[NATURE, QUOI],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"contrat"}"#,
        rend: "texte",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PoserModeleContrat,
        geste: "Poser le modèle de contrat ou d'avenant propre à la compagnie.",
        charge: &[NATURE, QUOI, Champ { nom: "texte", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "le modèle entier (le format des modèles livrés : TITRE:, ARTICLE:…)", avance: false }],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"avenant","texte":"TITRE: Avenant n° {{avenant.numero}}\n\nMODIFICATION:\nL'article {{modification.numero}} est désormais rédigé ainsi : « {{modification.apres}} »\n\nARTICLE: Le reste\nLe reste du contrat est inchangé.\n"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[Refus { classe: "ErreurOperation", corriger: "le modèle : la mention obligatoire ou le champ que la phrase nomme ; rien n'est posé" }, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RetirerModeleContrat,
        geste: "Remettre le modèle livré à la place de celui de la compagnie.",
        charge: &[NATURE, QUOI],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"avenant"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ClausesParticulieres,
        geste: "Écrire les clauses particulières que chaque contrat reprend dès aujourd'hui.",
        charge: &[Champ { nom: "texte", libelle: Affiche::new("Clauses"), forme: Forme::Texte, sens: "les clauses ; vide : aucune", avance: false }],
        exemple: r#"{"texte":"Les répétitions ont lieu au théâtre."}"#,
        rend: "phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 60,
        ecran: ECRAN,
    },
];

/// La case de la fiche entreprise des clauses particulieres de la compagnie.
const CLAUSES: &str = "clauses_particulieres";

/// Les champs qu'un modele d'avenant cite en plus de ceux du contrat (`documents::contrat::ecrire_avenant`).
const CHAMPS_DE_L_AVENANT: [&str; 10] = [
    "avenant.numero", "avenant.date_contrat", "avenant.reference", "avenant.effet", "avenant.date", "modification.numero", "modification.article",
    "modification.avant", "modification.apres", "modification.phrase",
];

/// Les champs du bloc ANNULE: d'un contrat qui en remplace un autre.
const CHAMPS_DU_REMPLACEMENT: [&str; 2] = ["remplace.numero", "remplace.date"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::ModelesContrat => modeles(ctx).map(|mut v| {
            v["ecran"] = ecran_des_modeles(&v);
            super::phrase_en_tete(v.clone(), dire_les_modeles(&v))
        }),
        Route::ModeleContratLivre => livre(charge).map(|v| {
            let p = format!("Modèle livré {}.", socle::valeur::valeur_en_texte(&v["libelle"]));
            super::phrase_en_tete(v, p)
        }),
        Route::PoserModeleContrat => poser(ctx, charge).map(|v| {
            let p = format!("Modèle de la compagnie posé. {}", dire_les_modeles(&v));
            super::phrase_en_tete(v, p)
        }),
        Route::RetirerModeleContrat => retirer(ctx, charge).map(|v| {
            let p = format!("Modèle de la compagnie retiré : le modèle livré revient. {}", dire_les_modeles(&v));
            super::phrase_en_tete(v, p)
        }),
        Route::ClausesParticulieres => clauses(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// Les natures dont la compagnie fait les contrats : le contrat d'usage toujours ; celles d'un permanent quand elle en
/// emploie (`emploie_des_permanents` de sa fiche).
fn natures_de(ctx: &Contexte) -> R<Vec<std::sync::Arc<Nature>>> {
    let permanents = ctx.emploie_des_permanents();
    Ok(droit::natures::natures()
        .map_err(Echec::from)?
        .iter()
        .filter(|n| n.modele_contrat().is_some() && (permanents || !n.permanent()))
        .cloned()
        .collect())
}

/// Le nom du modele livre de `quoi` (« contrat » ou « avenant ») pour la nature `cle`.
fn nom_du_modele(cle: &str, quoi: &str) -> R<(std::sync::Arc<Nature>, String)> {
    let nat = droit::natures::nature(Some(cle)).map_err(Echec::from)?;
    let nom = match quoi {
        "contrat" => nat.modele_contrat(),
        "avenant" => nat.modele_avenant(),
        autre => return Err(refus(format!("« {} » : un modèle de contrat ou d'avenant est attendu.", autre))),
    };
    let nom = nom.ok_or_else(|| refus(format!("La nature {} n'a pas de modèle de {}.", nat.libelle, quoi)))?;
    Ok((nat, nom))
}

/// Le modele de la compagnie, s'il est pose dans son dossier `gabarits/`.
fn propre(ctx: &Contexte, nom: &str) -> Option<PathBuf> {
    Some(ctx.gabarits().join(nom)).filter(|p| p.is_file())
}

/// CE QUE LA PAGE MONTRE : pour chaque nature, le modele du contrat et celui de l'avenant (le livre, ou celui de la
/// compagnie, et s'il est bon), les champs qu'un modele peut citer, ceux que la loi exige, le dossier ou poser le sien ;
/// et les clauses particulieres de la compagnie.
fn modeles(ctx: &Contexte) -> R<Value> {
    let mut sortie = Vec::new();
    for nat in natures_de(ctx)? {
        let mut pieces = Vec::new();
        for quoi in ["contrat", "avenant"] {
            let (_, nom) = nom_du_modele(&nat.cle, quoi)?;
            let a_soi = propre(ctx, &nom);
            let (verdict, texte) = match &a_soi {
                Some(p) => match std::fs::read_to_string(p) {
                    Ok(t) => (verifier(&nat, quoi, &nom, &t).err(), Some(t)),
                    Err(e) => (Some(format!("{} ne se lit pas : {}", p.display(), e)), None),
                },
                None => (None, None),
            };
            // CE QUE LA PASTILLE DIT, et sa couleur (`couleur_de_l_etat`) : le modele de la compagnie, bon ou refuse ; le
            // modele du logiciel, une information.
            let (etat, mots) = match (&a_soi, &verdict) {
                (Some(_), Some(_)) => ("refuse", "le modèle de la compagnie, refusé"),
                (Some(_), None) => ("fait", "le modèle de la compagnie"),
                (None, _) => ("info", "le modèle du logiciel"),
            };
            pieces.push(json!({"quoi": quoi, "nom": nom, "propre": a_soi.is_some(), "refus": verdict, "texte": texte,
                               "libelle": if quoi == "avenant" { "L'avenant" } else { "Le contrat" },
                               "bouton": if a_soi.is_some() { "Modifier le modèle" } else { "Écrire le modèle" },
                               "pastille": {"couleur": crate::routes::couleur_de_l_etat(etat), "mots": mots}}));
        }
        let mut champs = documents::contrat_ecrit::champs_des_modeles(&nat);
        champs.extend(CHAMPS_DU_REMPLACEMENT.iter().map(|c| json!({"marqueur": format!("{{{{{}}}}}", c), "quoi": "le contrat remplacé (bloc ANNULE: seulement)", "obligatoire": false})));
        let champs_titre = format!("Ses champs ({})", champs.len());
        sortie.push(json!({"nature": nat.cle, "libelle": nat.libelle, "nom": nat.nom_du_contrat(), "modeles": pieces, "champs": champs, "champs_titre": champs_titre}));
    }
    Ok(json!({
        "natures": sortie,
        "dossier": ctx.gabarits().to_string_lossy(),
        "champs_de_l_avenant": CHAMPS_DE_L_AVENANT.iter().map(|c| format!("{{{{{}}}}}", c)).collect::<Vec<_>>(),
        "clauses_particulieres": ctx.entreprise.clauses_particulieres.texte(),
    }))
}

/// Le texte du modele livre, pour partir de lui.
fn livre(charge: &Value) -> R<Value> {
    let (nat, nom) = nom_du_modele(texte_requis(charge, "nature")?, texte_requis(charge, "quoi")?)?;
    let chemin = dossier_du_code().join("modeles").join(&nom);
    let texte = std::fs::read_to_string(&chemin).map_err(|e| erreur("OSError", socle::echec::du_disque(&chemin, &e)))?;
    let de = if texte_requis(charge, "quoi")? == "avenant" { "de l'avenant" } else { "du contrat" };
    Ok(json!({"nom": nom, "texte": texte, "libelle": format!("{} {}", de, nat.libelle)}))
}

/// UN MODELE PROPRE SE VERIFIE AVANT D'ETRE POSE : sa grammaire (`fabrique_pdf::modele::lire_modele`), chacun de ses
/// champs connu du logiciel (un champ inconnu arreterait chaque contrat), et, pour un contrat, chaque mention que la loi
/// exige de sa nature.
fn verifier(nat: &Nature, quoi: &str, nom: &str, texte: &str) -> Result<(), String> {
    let m = fabrique_pdf::modele::lire_modele(texte, nom).map_err(|r| format!("Ce modèle ne se lit pas : {}", r.0))?;
    if quoi == "avenant" && m.modification.is_empty() {
        return Err("Ce modèle d'avenant n'a pas de bloc MODIFICATION: : c'est lui qui dit, article par article, ce qui change.".into());
    }
    let connus: Vec<String> = documents::contrat_ecrit::CHAMPS
        .iter()
        .map(|(c, _, _)| c.to_string())
        .chain(CHAMPS_DU_REMPLACEMENT.iter().map(|c| c.to_string()))
        .chain(if quoi == "avenant" { CHAMPS_DE_L_AVENANT.iter().map(|c| c.to_string()).collect() } else { Vec::new() })
        .collect();
    let mut inconnus: Vec<String> = Vec::new();
    for l in texte.lines().filter(|l| !l.trim_start().starts_with('#')) {
        for (i, _) in l.match_indices("{{") {
            if let Some((cle, _)) = fabrique_pdf::modele::marqueur_en(l, i) {
                if !connus.iter().any(|c| c == cle) && !inconnus.iter().any(|c| c == cle) {
                    inconnus.push(cle.to_string());
                }
            }
        }
    }
    if !inconnus.is_empty() {
        return Err(format!(
            "Ce modèle cite des champs que le logiciel ne connaît pas : {}. Les corriger d'après la liste des champs, ou les écrire en texte.",
            inconnus.iter().map(|c| format!("{{{{{}}}}}", c)).collect::<Vec<_>>().join(", ")
        ));
    }
    if quoi == "contrat" {
        // La verification ne sait pas encore ce que chaque contrat dira : une mention due « si elle est dite » (la
        // personne remplacee) se demande comme si elle l'etait.
        let toutes: serde_json::Map<String, Value> = nat.mentions_obligatoires().into_iter().map(|(c, _, _)| (c, json!("dite"))).collect();
        documents::contrat_ecrit::refuser_les_mentions_absentes(&std::path::Path::new("gabarits").join(nom), texte, nat, &Value::Object(toutes))?;
    }
    Ok(())
}

/// POSER LE MODELE DE LA COMPAGNIE : `{nature, quoi, texte}`. Verifie d'abord ; l'ancien modele propre, s'il y en a un,
/// est garde a cote (`.avant`).
fn poser(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (nat, nom) = nom_du_modele(texte_requis(charge, "nature")?, texte_requis(charge, "quoi")?)?;
    let quoi = texte_requis(charge, "quoi")?;
    let texte = texte_requis(charge, "texte")?;
    verifier(&nat, quoi, &nom, texte).map_err(|m| refus(format!("{} Rien n'est posé.", m.trim_end_matches(" Rien n'est écrit."))))?;
    let dossier = ctx.gabarits();
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", socle::echec::du_disque(&dossier, &e)))?;
    let cible = dossier.join(&nom);
    if cible.is_file() {
        let ancien = dossier.join(format!("{}.avant", nom));
        std::fs::rename(&cible, &ancien).map_err(|e| erreur("OSError", socle::echec::du_disque(&cible, &e)))?;
    }
    std::fs::write(&cible, texte).map_err(|e| erreur("OSError", socle::echec::du_disque(&cible, &e)))?;
    modeles(ctx)
}

/// RETIRER LE MODELE DE LA COMPAGNIE : le livre reprend sa place ; le sien est garde a cote (`.avant`).
fn retirer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (_, nom) = nom_du_modele(texte_requis(charge, "nature")?, texte_requis(charge, "quoi")?)?;
    if let Some(p) = propre(ctx, &nom) {
        let ancien = ctx.gabarits().join(format!("{}.avant", nom));
        std::fs::rename(&p, &ancien).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
    }
    modeles(ctx)
}

/// LES CLAUSES PARTICULIERES DE LA COMPAGNIE : `{texte}`, reprises dans chaque contrat a partir d'aujourd'hui ;
/// l'engagement peut dire les siennes (« Plus d'options »). Vide : aucune.
fn clauses(ctx: &Contexte, charge: &Value) -> R<Value> {
    let texte = charge.get("texte").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    crate::routes::fiches::poser_entreprise(ctx, CLAUSES, json!(texte), Some(ctx.aujourdhui))?;
    Ok(json!({"phrase": "Clauses particulières enregistrées."}))
}

/// Ce que `modeles` rend, en une phrase : les modeles de chaque nature, et ceux que la compagnie a poses.
fn dire_les_modeles(v: &Value) -> String {
    let modeles: Vec<&Value> = v["natures"].as_array().into_iter().flatten().flat_map(|n| n["modeles"].as_array().into_iter().flatten()).collect();
    let propres = modeles.iter().filter(|m| m["propre"] == json!(true)).count();
    format!("Modèles de contrat et d'avenant : {} ; propres à la compagnie : {}.", modeles.len(), propres)
}

/// CE QUE L'ECRAN DES MODELES DE CONTRAT DIT (`ecran` de `modeles-contrat`) : a quoi sert un modele, les clauses
/// particulieres, comment ecrire celui de la compagnie, et ce que cite un modele d'avenant.
fn ecran_des_modeles(v: &Value) -> Value {
    let avenant: Vec<String> = v["champs_de_l_avenant"].as_array().into_iter().flatten().map(super::mot).collect();
    json!({
        "intro": "Chaque contrat que le logiciel fait signer suit un modèle. Celui du logiciel porte déjà ce que la loi exige d'un contrat écrit, rempli de lui-même depuis l'engagement et la configuration. Si la compagnie a son propre contrat, elle le pose ici : le logiciel le prend à la place du sien, pour tous les contrats de cette sorte.",
        "clauses": "Ce qu'elle ajoute à chacun de ses contrats (confidentialité, image, déplacements…). Un engagement peut dire les siennes à la place, dans « Plus d'options ».",
        "comment": [
            "Partir du modèle du logiciel (« Écrire le modèle »), ou poser un fichier texte déjà écrit.",
            "Changer le texte comme on veut. Chaque article commence par une ligne « ARTICLE: son titre » ; une ligne vide sépare deux paragraphes ; **texte** met en gras ; une ligne qui commence par # est une note, qui ne s'imprime pas.",
            "Les champs entre accolades ({{salarie.nom_complet}}…) se remplissent d'eux-mêmes : leur liste est sous chaque contrat. Ceux que la loi exige doivent rester ; sans l'un d'eux, le modèle est refusé, et le refus dit lequel.",
            format!("« Poser ce modèle » : il est vérifié, puis rangé dans le dossier de la compagnie ({}), et sauvegardé avec elle. L'ancien est gardé à côté.", super::mot(&v["dossier"])),
        ],
        "avenant": format!("Un modèle d'avenant cite aussi : {}.", avenant.join(", ")),
    })
}
