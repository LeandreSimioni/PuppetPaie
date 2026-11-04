//! Les routes de l'Historique, hors du detail d'un mois (`historique-mois`) : les annees (leurs DSN, leur cloture, leur
//! export comptable), les corrections par mois d'origine, l'archive d'un mois dans le Drive, et la reprise du journal
//! depuis ce que les retours attestent.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::documents::requis;
use crate::routes::lire_json_ou_vide;
use crate::routes::ouverture::mois_connus;
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::valeur::{nom_type, valeur_en_texte};
use socle::fichiers::vrai;

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Historique);

/// Une lecture sans charge.
const fn lecture(route: Route, geste: &'static str, rend: &'static str) -> Fiche {
    Fiche { route, geste, charge: &[], exemple: "{}", rend, refus: &[OPERATION], acte: Acte::Lit, avance: false, phrase: 0, ecran: ECRAN }
}

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    lecture(Route::Exercices, "Lister les années : leurs DSN, leur clôture, leur export comptable.", "annees (chacune : DSN, clôture, export)"),
    lecture(Route::Regularisations, "Lister les corrections à porter et portées, par mois d'origine.", "les corrections, mois par mois"),
    Fiche {
        route: Route::Cloture,
        geste: "Clore une année passée : archiver ses engagements, documents et déclarations.",
        charge: &[
            ANNEE,
            Champ { nom: "forcer", libelle: Affiche::new("Clore malgré tout"), forme: Forme::OuiNon, sens: "vrai : clore malgré un mois sans DSN réelle acceptée", avance: true },
            Champ { nom: "rouvrir", libelle: Affiche::new("Rouvrir l'année"), forme: Forme::OuiNon, sens: "vrai : rouvrir l'année close (son archive reste)", avance: true },
        ],
        exemple: r#"{"annee":"2025"}"#,
        rend: "annee, archive, fichiers, empreinte, manques (ce qui n'a pu s'archiver) ; rouverte : etait_clos_depuis",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ExportCompta,
        geste: "Écrire l'export comptable d'une année, du mois du au mois au.",
        charge: &[
            ANNEE,
            Champ { nom: "du", libelle: Affiche::new("Du mois"), forme: Forme::Nombre, sens: "le premier mois, 1 à 12", avance: true },
            Champ { nom: "au", libelle: Affiche::new("Au mois"), forme: Forme::Nombre, sens: "le dernier mois, 1 à 12", avance: true },
            Champ { nom: "apercu", libelle: Affiche::new("Aperçu seulement"), forme: Forme::OuiNon, sens: "vrai : dire ce qui s'écrirait, sans écrire", avance: true },
        ],
        exemple: r#"{"annee":"2026","apercu":true}"#,
        rend: "les fichiers de l'export, ou ce qu'ils porteraient",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Archiver,
        geste: "Déposer les documents d'un mois dans le Drive de la compagnie.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "mois, dossier (dans le Drive), compte, fichiers (nom, octets, remplace), phrase ; une fictive joue le dépôt, rien ne part",
        refus: &[
            Refus { classe: "SortieAbsente", corriger: "un mois dont les documents sont écrits (une paie validée)" },
            Refus { classe: "ErreurGoogle", corriger: "lier le Drive (route google-autoriser), ou ce que la phrase de Google dit" },
            Refus { classe: "CopieDeDeveloppement", corriger: "rien : une copie de développement n'écrit jamais dans le Drive" },
            OPERATION,
        ],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::JournalReprise,
        geste: "Reprendre le journal des envois d'après les retours relevés.",
        charge: &[Champ { nom: "ecrire", libelle: Affiche::new("Écrire ce qui se reprend"), forme: Forme::OuiNon, sens: "vrai : écrire ce qui se reprend ; sinon, le dire seulement", avance: false }],
        exemple: r#"{}"#,
        rend: "ce qui se reprend, ligne par ligne, et ses pastilles",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    let t = |v: &Value, k: &str| super::mot(&v[k]);
    match nom {
        Route::Exercices => exercices_route(ctx).map(|mut v| {
            let lignes: Vec<Value> = v["exercices"].as_array().into_iter().flatten().map(|e| json!(format!("{} : {}", t(e, "annee"), t(&e["pastille"], "mots")))).collect();
            v["phrases"] = Value::Array(lignes);
            let p = format!("Exercices : {}.", super::nombre(&v["exercices"]));
            super::phrase_en_tete(v, p)
        }),
        Route::Regularisations => regularisations(ctx).map(|v| {
            let p = format!("Mois qui portent des corrections : {}.", super::nombre(&v["corrections"]));
            super::phrase_en_tete(v, p)
        }),
        Route::Cloture => crate::compagnie::exercice_ecrit::cloture_route(ctx, charge).map(|v| {
            let p = if v.get("etait_clos_depuis").is_some() {
                format!("Exercice {} rouvert ; son archive reste.", t(&v, "annee"))
            } else {
                let manques: Vec<String> = v["manques"].as_array().into_iter().flatten().map(socle::valeur::valeur_en_texte).collect();
                let reste = if manques.is_empty() { String::new() } else { format!(" Pas archivé : {}.", manques.iter().map(|m| m.trim_end_matches('.')).collect::<Vec<_>>().join(" ; ")) };
                format!("Exercice {} clos : {} fichiers archivés.{}", t(&v, "annee"), t(&v, "fichiers"), reste)
            };
            super::phrase_en_tete(v, p)
        }),
        Route::ExportCompta => crate::compagnie::exercice_ecrit::export_compta_route(ctx, charge).map(|v| {
            let p = match v["fichier"].as_str() {
                Some(f) => format!("Export comptable de {} : {} écritures, dans {} ({}).", t(&v, "annee"), super::nombre(&v["ecritures"]), f, t(&v, "dossier")),
                None => format!("Export comptable de {} : {} écritures, rien n'est écrit.", t(&v, "annee"), super::nombre(&v["ecritures"])),
            };
            super::phrase_en_tete(v, p)
        }),
        Route::Archiver => archiver(ctx, charge),
        Route::JournalReprise => crate::compagnie::incidents::noter_ceux_des_retours(
            &ctx.racine,
            envois::reprise_journal::reprise(
                ctx,
                charge.get("ecrire").is_some_and(vrai),
                &crate::routes::INTERFACE.to_string(),
                &socle::paris::iso_secondes(socle::temps::secondes()),
            ),
        )
        .map(pastilles_de_la_reprise)
        .map(|mut v| {
            // Ce que l'Historique en dit, en tete : le resume replie, l'explication, le nom du logiciel lu.
            let n = super::nombre(&v["ajoutes"]);
            let reels = v["ajoutes"].as_array().into_iter().flatten().filter(|d| d["mode"] == "reel" && vrai(&d["accepte"])).count();
            let s = if n > 1 { "s" } else { "" };
            v["ecran"] = json!({
                "resume": format!("{} dépôt{} retrouvé{} dans les retours, absent{} du journal{}", n, s, s, s, if reels > 0 { format!(" — dont {} réel{}", reels, if reels > 1 { "s" } else { "" }) } else { String::new() }),
                "titre": format!("{} dépôt(s) attestés par les retours, absents du journal", n),
                "explication": "Le journal des dépôts est né le 11/09/2026 ; ce qui est parti avant ne s'y est pas écrit. Les accusés d'enregistrement reçus des organismes, eux, s'en souviennent. Les reprendre ne dépose rien et n'envoie rien : ça remet au journal ce qui a déjà eu lieu.",
                "reels": if reels > 0 { json!(format!("{} sont des dépôts réels acceptés.", reels)) } else { Value::Null },
                "logiciel": "Le nom du logiciel n'est pas une annotation : il est lu dans le certificat de conformité. C'est lui qui distingue le dépôt d'un autre logiciel d'un dépôt PuppetPaie.",
            });
            v["ecran"]["bouton"] = json!(format!("Reprendre ces {} dépôt(s)", n));
            for d in v["ajoutes"].as_array_mut().into_iter().flatten() {
                let mois = d["mois"].as_str().filter(|m| !m.is_empty()).map(socle::mois::Mois::en_clair).unwrap_or_else(|| "mois inconnu".to_string());
                d["dit"] = json!(format!("{} — {}", super::quand(&valeur_en_texte(&d["quand"])), mois));
                let logiciel = d["logiciel"].as_str().filter(|l| !l.is_empty()).unwrap_or("logiciel inconnu").to_string();
                d["origine_dit"] = json!(format!("déposé par {} · flux {}", logiciel, valeur_en_texte(&d["flux"])));
            }
            let p = format!("Dépôts trouvés dans les retours : {} ; ajoutés au journal : {}{}.", super::nombre(&v["trouves"]), super::nombre(&v["ajoutes"]), if v["ecrit"] == json!(true) { "" } else { " (rien n'est écrit)" });
            super::phrase_en_tete(v, p)
        }),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// Par mois d'origine, les corrections que la page montre (`crate::regularisation::corrections_du_mois`) : celles a
/// porter (`etat` « à porter ») et celles deja portees par la DSN d'un mois suivant (« portée »).
fn corrections_par_mois(ctx: &Contexte) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for mois in engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().map(String::from) {
        let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, &mois)?;
        let mut corrections: Vec<Value> = portees
            .into_iter()
            .map(|mut c| {
                c["etat"] = json!("portée");
                c
            })
            .collect();
        corrections.extend(a_porter.into_iter().map(|mut c| {
            c["etat"] = json!("à porter");
            c
        }));
        if !corrections.is_empty() {
            out.push(json!({"mois": mois, "corrections": corrections}));
        }
    }
    Ok(out)
}

/// La route `regularisations`.
fn regularisations(ctx: &Contexte) -> R<Value> {
    let mut corrections = corrections_par_mois(ctx)?;
    // La pastille de chaque correction : portee par une DSN partie, faite ; a porter par la prochaine, a faire.
    for c in corrections.iter_mut().flat_map(|m| m.get_mut("corrections").and_then(Value::as_array_mut).into_iter().flatten()) {
        let mots = c.get("etat").map(valeur_en_texte).unwrap_or_default();
        c["pastille"] = super::pastille(if c.get("portee_par").is_some_and(vrai) { "fait" } else { "a_faire" }, &mots);
    }
    Ok(json!({"corrections": corrections}))
}

/// Les clotures, sans les cles `_...`.
fn exercices_clos(racine: &Path) -> R<Map<String, Value>> {
    let v = lire_json_ou_vide(&racine.join(crate::compagnie::exercice_ecrit::EXERCICES))?;
    match v {
        Value::Object(o) => Ok(o.into_iter().filter(|(k, _)| !k.starts_with('_')).collect()),
        autre => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&autre)))),
    }
}

/// La route `exercices` : `{"exercices": etat_exercices, "dossier": ...}`.
fn exercices_route(ctx: &Contexte) -> R<Value> {
    let liste = etat_exercices(ctx)?;
    Ok(json!({"exercices": liste, "dossier": ctx.racine.join("sorties").join("exercices").display().to_string()}))
}

/// `etat_exercices` : chaque annee connue du dossier -- ses mois, sa cloture,
/// son export.
fn etat_exercices(ctx: &Contexte) -> R<Value> {
    let clos = exercices_clos(&ctx.racine)?;
    let mois_tous: Vec<String> = mois_connus(&ctx.racine).into_iter().map(|(m, _)| m).collect();
    let mut annees: BTreeSet<String> = mois_tous.iter().map(|m| m.chars().take(4).collect()).collect();
    annees.extend(clos.keys().cloned());
    let mut out = Vec::new();
    for a in annees.iter().rev() {
        let mut mois: Vec<&String> = mois_tous.iter().filter(|x| x.starts_with(a.as_str())).collect();
        mois.sort();
        let export = ctx.racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", a));
        let export = export.exists().then_some(export);
        let mut acceptees = Vec::new();
        for x in &mois {
            if crate::compagnie::etat_dsn::du_mois(ctx, socle::mois::Mois::lire(x)?, None)?.partie() {
                acceptees.push(json!(x));
            }
        }
        let export_json = match &export {
            Some(p) => {
                json!({
                    "fichier": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    "ecrit": crate::compagnie::depart::ecrit_le(&ctx.racine, p),
                })
            }
            None => Value::Null,
        };
        // Ce que la ligne de l'annee dit : ses mois, ses DSN acceptees, sa cloture et son export compta.
        let clos_dit = clos.get(a).map(|c| format!(" · clos le {}", super::quand(&super::mot(&c["le"])))).unwrap_or_default();
        let export_dit = match export_json.get("ecrit").filter(|e| vrai(e)) {
            Some(e) => format!(" · export compta du {}", super::quand(&super::mot(e))),
            None if !export_json.is_null() => " · export compta".to_string(),
            None => " · pas d'export compta".to_string(),
        };
        let n_dsn = super::nombre(&json!(acceptees));
        let infos_dit = format!("{} mois · {}{}{}", super::nombre(&json!(mois)), super::compte(n_dsn, "DSN acceptée", "DSN acceptées"), clos_dit, export_dit);
        out.push(json!({
            "infos_dit": infos_dit,
            "annee": a, "mois": mois, "dsn_acceptees": acceptees,
            "clos": clos.get(a).cloned().unwrap_or(Value::Null),
            // Sa pastille : close, faite ; ouverte, une information (une annee se clot quand la compagnie le veut).
            "pastille": if clos.contains_key(a.as_str()) { super::pastille("fait", "clos") } else { super::pastille("information", "ouvert") },
            "export": export_json,
        }));
    }
    Ok(Value::Array(out))
}

/// DEPOSER LES DOCUMENTS D'UN MOIS DANS LE DRIVE (`archivage::archiver`) : `PuppetPaie/<mois>` du compte lie. Une
/// compagnie fictive joue le depot (`archivage::DriveJoue`) : la meme liste, rien ne part.
fn archiver(ctx: &Contexte, charge: &Value) -> R<Value> {
    use crate::compagnie::archivage;
    let mois = valeur_en_texte(requis(charge, "mois")?);
    let joue = crate::compagnie::bridage::branchement(&ctx.racine).joue();
    let mut drive_joue = archivage::DriveJoue;
    let client: Option<&mut dyn envois::drive::Drive> = if joue { Some(&mut drive_joue) } else { None };
    let r = archivage::archiver(&ctx.racine, &mois, crate::compagnie::sauvegarde::RACINE_DRIVE, client).map_err(|e| erreur(e.classe, e.message))?;
    let dossier = format!("{}/{}", r.racine_drive, r.mois);
    // Le dossier du Drive tel que l'écran le dit : ses dossiers séparés comme les places de l'écran.
    let ou = format!("{} › {}", r.racine_drive, r.mois);
    let phrase = if joue {
        format!("Compagnie fictive : rien ne part au Drive. {} fichier(s) iraient dans le dossier {}.", r.fichiers.len(), ou)
    } else {
        format!("{} fichier(s) déposé(s) dans le Drive, dossier {}.", r.fichiers.len(), ou)
    };
    Ok(json!({
        "mois": r.mois,
        "dossier": dossier,
        "compte": r.compte,
        "fichiers": r.fichiers.iter().map(|f| json!({"nom": f.nom, "octets": f.octets, "remplace": f.remplace})).collect::<Vec<_>>(),
        "phrase": phrase,
    }))
}

/// La pastille de chaque depot que la reprise du journal remet (`ajoutes`) : un depot reel accepte, fait ; refuse,
/// refuse ; un depot de test, une information.
fn pastilles_de_la_reprise(mut v: Value) -> Value {
    for d in v.get_mut("ajoutes").and_then(Value::as_array_mut).into_iter().flatten() {
        let reel = d.get("mode").and_then(Value::as_str) == Some("reel");
        let accepte = d.get("accepte").is_some_and(vrai);
        let mots = format!("{}{}", if reel { "dépôt réel" } else { "test" }, if accepte { "" } else { " — refusé" });
        d["pastille"] = super::pastille(if !reel { "information" } else if accepte { "fait" } else { "refuse" }, &mots);
    }
    v
}

