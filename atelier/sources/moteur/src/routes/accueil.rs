//! L'ACCUEIL DES COMPAGNIES : les routes de l'installation (le dossier qui porte `logiciel/`, `prive/`, `compagnies/`,
//! `fictives/` : `interface/compagnies.rs`), et non d'une compagnie. Les lister, en ajouter une, choisir sa couleur, remettre
//! une fictive livree a zero, garder au deblocage les compagnies creees en demo, completer un dossier portable des systemes
//! qui lui manquent (`maj::completer_le_portable`) ; et ce que l'administrateur fait sur
//! toutes ses compagnies a la fois (son tableau, son depot groupe).
//!
//! L'INSTALLATION D'UNE ROUTE : celle de la compagnie nommee (`--racine`, `administrateur::installation_de`), sinon celle
//! du programme (`compagnies::base`, a cote de `logiciel/`) : `moteur route` sans `--racine` sert l'installation ou il est
//! pose. La fenetre sert ces routes a l'accueil, avant toute compagnie ouverte (`interface::api::route_de_l_installation`).
//!
//! Ouvrir une compagnie dans la fenetre (sa tuile, son mot de passe) n'est pas une route : la route rend son `chemin`, la
//! fenetre l'ouvre ; en ligne de commande, on la nomme par `--racine`. Le bridage (la demo) est un reglage de l'accueil :
//! il n'a pas de route, son geste n'existe qu'a l'ecran.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::compagnie::administrateur;
use crate::interface::{compagnies, suppression};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, MANQUE, OPERATION};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Accueil);

const COMPAGNIE: Champ = Champ { nom: "compagnie", libelle: Affiche::new("Compagnie"), forme: Forme::Texte, sens: "l'identifiant de la compagnie, tel que `compagnies` le rend", avance: false };

/// Les routes de l'installation, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        nom: "compagnies",
        geste: "Dire les compagnies de cette installation, et où est chacune.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "compagnies (id, nom, lieu, chemin — le --racine de ses routes —, reelle, remise_a_zero, couleur), bride, a_garder (au déblocage), phrase, phrases (une par compagnie), auteur (phrase, nom, adresse)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 80,
        ecran: Ecran::Lecture,
    },
    Fiche {
        nom: "compagnie-ajouter",
        geste: "Ajouter une compagnie : neuve, ou venant d'un logiciel précédent.",
        charge: &[
            Champ { nom: "nom", libelle: Affiche::new("Nom de la compagnie"), forme: Forme::Texte, sens: "le nom de la compagnie", avance: false },
            Champ { nom: "venue", libelle: Affiche::new("Venue"), forme: Forme::Texte, sens: "neuve, ou logiciel_precedent (elle commence par reprendre son passé)", avance: false },
        ],
        exemple: r#"{"nom":"Les Tréteaux d'essai","venue":"neuve"}"#,
        rend: "id, nom, chemin (le --racine de ses routes), phrase ; logiciel bridé : une compagnie fictive ajoutée",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        nom: "compagnie-couleur",
        geste: "Choisir la couleur d'une compagnie.",
        charge: &[COMPAGNIE, Champ { nom: "couleur", libelle: Affiche::new("Couleur"), forme: Forme::Texte, sens: "#RRGGBB", avance: false }],
        exemple: r##"{"compagnie":"cie:la-compagnie","couleur":"#2E5A7D"}"##,
        rend: "couleur, phrase",
        refus: &[MANQUE, Refus { classe: "ErreurOperation", corriger: "une couleur #RRGGBB, une compagnie réelle ; rien n'est écrit" }],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 80,
        ecran: ECRAN,
    },
    Fiche {
        nom: "compagnie-remettre-a-zero",
        geste: "Remettre une compagnie fictive livrée dans son état d'origine.",
        charge: &[COMPAGNIE],
        exemple: r#"{"compagnie":"fictive:fictive-essai"}"#,
        rend: "phrase",
        refus: &[MANQUE, Refus { classe: "ErreurOperation", corriger: "une compagnie fictive livrée ; rien n'est effacé" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        nom: "compagnies-garder",
        geste: "Au déblocage : garder les compagnies créées en démo, ou les supprimer.",
        charge: &[
            Champ { nom: "garder", libelle: Affiche::new("À garder"), forme: Forme::Liste, sens: "les identifiants des compagnies à garder (elles deviennent réelles)", avance: false },
            Champ { nom: "ne_pas_garder", libelle: Affiche::new("À supprimer"), forme: Forme::Liste, sens: "les identifiants des compagnies à supprimer", avance: false },
        ],
        exemple: r#"{"garder":["fictive:ajoutee-demo"]}"#,
        rend: "gardees, supprimees, erreurs, phrase",
        refus: &[Refus { classe: "ErreurOperation", corriger: "une compagnie créée en démo, d'un logiciel débloqué ; rien n'a changé" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        nom: "administrateur-tableau",
        geste: "Dire les déclarations à faire de chaque compagnie de l'administrateur.",
        charge: &[
            Champ { nom: "garde", libelle: Affiche::new("Tableau gardé"), forme: Forme::OuiNon, sens: "vrai : le tableau gardé, sans calcul", avance: false },
            Champ { nom: "relever", libelle: Affiche::new("Relever d'abord les comptes rendus"), forme: Forme::OuiNon, sens: "vrai : relever d'abord les comptes rendus attendus", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "compagnies (chacune ses déclarations triées par urgence, et où en est leur fichier), groupe (ce que le dépôt groupé propose)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "administrateur-deposer",
        geste: "Déposer d'un geste ce qui est prêt dans chaque compagnie de l'administrateur.",
        charge: &[
            Champ { nom: "reel", libelle: Affiche::new("En réel"), forme: Forme::OuiNon, sens: "vrai : en réel (les fichiers testés conformes) ; en test sinon", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe de l'administrateur"), forme: Forme::Texte, sens: "le mot de passe de l'administrateur ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "partis, refuses, resultats (par dépôt : la compagnie, ce qui est parti ou le refus), tableau ; rien de prêt : phrase",
        refus: &[Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe, tapé par un humain ; rien n'est déposé" }, OPERATION],
        acte: Acte::Engage,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "portable-completer",
        geste: "Ajouter au dossier les systèmes qui lui manquent (Windows, Linux, macOS).",
        charge: &[Champ { nom: "portable", libelle: Affiche::new("Rendre ce dossier portable"), forme: Forme::OuiNon, sens: "vrai : faire aussi de ce dossier un dossier portable, pour une clé USB (PORTABLE.json)", avance: false }],
        exemple: r#"{"portable":true}"#,
        rend: "ajoutes, absents (les systèmes que la version n'a pas), version, portable, phrase",
        refus: &[Refus { classe: "ErreurOperation", corriger: "le réseau, la mise à jour en attente, ou ce que la phrase dit ; rien n'est posé" }],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 140,
        ecran: Ecran::Auto(Place::Accueil, Affiche::new("Ajouter les systèmes qui manquent")),
    },
];

/// L'installation de la route : celle de la compagnie nommee (`--racine`), sinon celle du programme.
pub fn installation(o: &HashMap<String, String>) -> PathBuf {
    match o.get("racine").filter(|r| !r.is_empty()) {
        Some(r) => administrateur::installation_de(std::path::Path::new(r)),
        None => compagnies::base(),
    }
}

fn texte<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    charge.get(cle).and_then(Value::as_str).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn ids(charge: &Value, cle: &str) -> Vec<String> {
    charge.get(cle).and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default()
}

fn operation(m: String) -> Echec {
    erreur("ErreurOperation", m)
}

/// Une route de l'installation.
pub fn route(o: &HashMap<String, String>, nom: &str, charge: &Value) -> R<Value> {
    // `--racine` nomme une compagnie (son dossier porte sa fiche entreprise) : l'installation est la sienne. Un autre
    // dossier n'en est pas une, et son parent n'est pas une installation : rien ne s'y ecrit.
    if let Some(r) = o.get("racine").filter(|r| !r.is_empty()) {
        if !std::path::Path::new(r).join(droit::contexte::ENTREPRISE).is_file() {
            return Err(operation(format!(
                "{} n'est pas le dossier d'une compagnie : --racine nomme une compagnie (le chemin que rend la route compagnies). Les routes de l'installation se servent sans --racine. Rien n'est écrit.",
                r
            )));
        }
    }
    let inst = installation(o);
    match nom {
        "compagnies" => Ok(liste(&inst)),
        "compagnie-ajouter" => {
            let venue = compagnies::Venue::lire(charge.get("venue").and_then(Value::as_str).unwrap_or(""));
            let c = compagnies::ajouter_venant(&inst, texte(charge, "nom")?, venue).map_err(operation)?;
            let phrase = if c.reelle {
                format!("{} est ajoutée.", c.nom)
            } else {
                format!("{} est ajoutée, en compagnie fictive : rien n'en part.", c.nom)
            };
            let chemin = c.racine.display().to_string();
            Ok(json!({"id": c.id, "nom": c.nom, "chemin": chemin, "phrase": phrase}))
        }
        "compagnie-couleur" => {
            let couleur = compagnies::poser_couleur(&inst, texte(charge, "compagnie")?, texte(charge, "couleur")?).map_err(operation)?;
            Ok(json!({"couleur": couleur, "phrase": format!("Couleur posée : {}.", couleur)}))
        }
        "compagnie-remettre-a-zero" => {
            let (n, deja) = compagnies::remettre_a_zero(&inst, texte(charge, "compagnie")?).map_err(operation)?;
            Ok(json!({"phrase": format!("{} est {}.", n, if deja { "déjà dans son état d'origine" } else { "revenue à son état d'origine" })}))
        }
        "compagnies-garder" => {
            let poste = administrateur::Poste::de_ce_poste();
            let mut v = suppression::repondre(&inst, &ids(charge, "garder"), &ids(charge, "ne_pas_garder"), &poste, &|_| {}).map_err(operation)?;
            let n = |k: &str| v[k].as_array().map(Vec::len).unwrap_or(0);
            let mut phrase = format!("{} compagnie(s) gardée(s), {} supprimée(s).", n("gardees"), n("supprimees"));
            for e in v["erreurs"].as_array().into_iter().flatten() {
                phrase.push(' ');
                phrase.push_str(&valeur_en_texte(e));
            }
            v["phrase"] = json!(phrase);
            Ok(v)
        }
        "administrateur-tableau" if charge.get("garde").is_some_and(vrai) => Ok(dire_le_tableau(administrateur::tableau_garde(&inst))),
        "administrateur-tableau" => Ok(dire_le_tableau(administrateur::tableau(o, &inst, charge.get("relever").is_some_and(vrai)))),
        "portable-completer" => {
            let cles = crate::compagnie::maj::cles_de_l_editeur();
            crate::compagnie::maj::completer_le_portable(&inst, charge.get("portable").is_some_and(vrai), &cles).map_err(operation)
        }
        "administrateur-deposer" => {
            let reel = charge.get("reel").is_some_and(vrai);
            let mdp = charge.get("motdepasse").filter(|m| vrai(m)).map(valeur_en_texte);
            administrateur::deposer(o, &inst, reel, mdp)
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// Les compagnies de l'installation, dans l'ordre des tuiles de l'accueil, et ce que l'accueil en dit.
fn liste(inst: &std::path::Path) -> Value {
    let cies: Vec<Value> = compagnies::liste_de(inst)
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "nom": c.nom, "lieu": c.lieu, "chemin": c.racine.display().to_string(), "reelle": c.reelle,
                "remise_a_zero": !c.reelle && compagnies::est_fictive_livree(&c.racine), "couleur": c.couleur,
                "logo": compagnies::logo(&c.racine).map(|p| p.display().to_string()),
            })
        })
        .collect();
    let a_garder: Vec<Value> = suppression::a_garder(inst).iter().map(|c| json!({"id": c.id, "nom": c.nom})).collect();
    let reelles = cies.iter().filter(|c| c["reelle"] == json!(true)).count();
    let phrase = format!("{} compagnie(s), dont {} fictive(s).", cies.len(), cies.len() - reelles);
    let t = |c: &Value, k: &str| valeur_en_texte(&c[k]);
    let phrases: Vec<String> = cies.iter().map(|c| format!("{}{}", t(c, "nom"), if c["reelle"] == json!(true) { "" } else { ", fictive" })).collect();
    json!({"compagnies": cies, "bride": crate::compagnie::bridage::est_bride_de(inst), "a_garder": a_garder, "phrase": phrase, "phrases": phrases, "auteur": crate::interface::auteur::auteur()})
}

/// Le tableau de l'administrateur en phrases : combien de declarations, puis chacune (sa compagnie, ce qu'elle est, quand).
fn dire_le_tableau(mut v: Value) -> Value {
    let t = |x: &Value, k: &str| super::mot(&x[k]);
    let lignes: Vec<Value> = v["declarations"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| json!(format!("{} : {}, {}{}", t(d, "compagnie"), t(d, "titre"), if d["en_retard"] == json!(true) { "en retard, c'était " } else { "au plus tard " }, t(d, "quand"))))
        .collect();
    let p = format!("Déclarations à faire : {} ; compagnies : {}.", lignes.len(), super::nombre(&v["compagnies"]));
    v["phrases"] = Value::Array(lignes);
    super::phrase_en_tete(v, p)
}
