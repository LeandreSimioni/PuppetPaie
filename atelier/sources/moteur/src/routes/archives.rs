//! Les routes des Exports : ce qui reste d'une annee de paies (le dossier d'un salarie, les exports et leur logiciel
//! comptable, les droits des intermittents), et ce qui part d'elles (le dossier par courriel, la regularisation de la
//! CASC-SVP).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, MANQUE, MOIS, OPERATION};
use crate::routes::qui_de;
use crate::routes::documents::{client_courriel, expediteur_de, preuve_de, refus, requis, vrai_ou, Ouvrir};
use crate::routes::engagement::{booleen, maintenant};

use documents::plan_comptable::{controler_plan, tables};
use droit::contexte::Contexte;
use envois::envoi::{self, Message};
use envois::etats::{getd, ou, texte_vide};
use envois::ligne_d_envoi::{echec_d_envoi, noter, noter_echec_d_envoi};
use socle::date::jour_iso;
use socle::echec::{erreur, Echec, R};
use socle::ecriture::ecrire_json;
use socle::fichiers::lire_json_echec as lire_json;
use socle::fichiers::{nom_du, repr, vrai};
use socle::html::courriel_en_html;
use socle::valeur::{nom_type, valeur_en_texte};
use crate::routes::catalogue::{Ecran, Place};

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

/// Une lecture sans charge de l'Historique ou des Exports.
const fn lecture(nom: &'static str, geste: &'static str, rend: &'static str) -> Fiche {
    Fiche { nom, geste, charge: &[], exemple: "{}", rend, refus: &[OPERATION], acte: Acte::Lit, avance: false, phrase: 0, ecran: ECRAN }
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    lecture("dossier-formes", "Dire ce que peut porter le dossier d'un salarié : catégories, périodes.", "salaries, categories, periodes"),
    lecture("exports-periodes", "Dire ce qui s'exporte : mois, années, états, formats, logiciel comptable.", "mois, annees, etats, formats, logiciel retenu"),
    Fiche {
        nom: "retenir-logiciel",
        geste: "Retenir le logiciel du comptable, dont les écritures prennent le format.",
        charge: &[Champ { nom: "logiciel", libelle: Affiche::new("Logiciel"), forme: Forme::Texte, sens: "son nom, parmi ceux que exports-periodes connaît ; absent : aucun", avance: false }, Champ { nom: "analytique", libelle: Affiche::new("Par spectacle"), forme: Forme::OuiNon, sens: "vrai : par spectacle", avance: true }],
        exemple: r#"{}"#,
        rend: "le logiciel retenu",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "casc-regularise",
        geste: "Noter régularisée, hors DSN, la CASC-SVP d'une année de mois importés.",
        charge: &[
            ANNEE,
            Champ { nom: "vire_le", libelle: Affiche::new("Viré le"), forme: Forme::Date, sens: "le jour du virement, AAAA-MM-JJ", avance: false },
            Champ { nom: "montant", libelle: Affiche::new("Montant viré"), forme: Forme::Nombre, sens: "le montant viré", avance: false },
        ],
        exemple: r#"{"annee":"2025","vire_le":"2026-11-02","montant":"12.80"}"#,
        rend: "la régularisation notée : année, mois, montant, calculé, vire_le",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "exporter",
        geste: "Exporter un état (livre de paie, charges…) en tableau ou en fichier.",
        charge: &[
            Champ { nom: "etat", libelle: Affiche::new("État"), forme: Forme::Texte, sens: "l'état, parmi ceux que exports-periodes dit", avance: false },
            MOIS,
            ANNEE,
            Champ { nom: "format", libelle: Affiche::new("Format"), forme: Forme::Texte, sens: "le format du fichier", avance: true },
            Champ { nom: "apercu", libelle: Affiche::new("Aperçu seulement"), forme: Forme::OuiNon, sens: "vrai : le tableau, sans écrire de fichier", avance: true },
        ],
        exemple: r#"{"etat":"livre-de-paie","mois":"2026-11","apercu":true}"#,
        rend: "le tableau, ou le fichier écrit",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "dossier-envoyer",
        geste: "Refaire le dossier de chaque salarié choisi et le lui envoyer par courriel.",
        charge: &[
            Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "les clés de leurs fiches", avance: false },
            MOIS,
            ANNEE,
            Champ { nom: "categories", libelle: Affiche::new("Ce que le dossier porte"), forme: Forme::Liste, sens: "ce que le dossier porte ; tout sinon", avance: true },
        ],
        exemple: r#"{"salaries":["FULLER"],"mois":"2026-11"}"#,
        rend: "le dossier, et envoi (partis)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est écrit" },
            OPERATION,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "droits",
        geste: "Dire les heures et cachets cumulés de chaque intermittent (annexes 8 et 10).",
        charge: &[Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois du tableau (le mois, l'année civile), AAAA-MM ; absent : celui du jour", avance: false }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "salaries (chacun : nom, fenetre, lignes [quoi, valeur], phrase, notes, colonnes du tableau), phrase (ce que vaut ce compte)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        nom: "casc-envoyer",
        geste: "Envoyer à Audiens et au CASC-SVP la régularisation des mois importés.",
        charge: &[
            Champ { nom: "vire_le", libelle: Affiche::new("Virements faits"), forme: Forme::Objet, sens: "{année: jour du virement fait, AAAA-MM-JJ}", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "le mot de passe du logiciel ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{"vire_le":{"2026":"2026-11-02"}}"#,
        rend: "partis (les destinataires), expediteur, periode, total, annees (les régularisations notées), eml",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est parti" },
            Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe)" },
            Refus { classe: "ErreurOperation", corriger: "la date du virement fait, ou l'année que la phrase nomme ; rien n'est parti" },
        ],
        acte: Acte::Engage,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let t = |v: &Value, k: &str| super::mot(&v[k]);
    match nom {
        "dossier-formes" => dossier_formes(ctx).map(|v| {
            let p = format!("Dossiers possibles : {} ; sortes de pièces : {}.", super::compte(super::nombre(&v["salaries"]), "salarié", "salariés"), super::nombre(&v["categories"]));
            super::phrase_en_tete(v, p)
        }),
        "exports-periodes" => exports_periodes(ctx, charge).map(|mut v| {
            v["ecran"] = ecran_des_exports(&v);
            let p = format!("Exports : {} ; mois payés : {} ; années : {}.", super::compte(super::nombre(&v["etats"]), "état", "états"), super::nombre(&v["mois"]), super::nombre(&v["annees"]));
            super::phrase_en_tete(v, p)
        }),
        "retenir-logiciel" => retenir_logiciel(ctx, charge).map(|v| {
            let p = if t(&v, "logiciel").is_empty() { "Aucun logiciel comptable retenu.".to_string() } else { format!("Logiciel comptable retenu : {}.", t(&v, "logiciel")) };
            super::phrase_en_tete(v, p)
        }),
        "casc-regularise" => casc_regularise(ctx, charge).map(|v| {
            let p = format!("CASC-SVP {} notée régularisée : {} virés le {}.", t(&v, "annee"), socle::texte::euros_de(&v["montant"]), super::quand(&t(&v, "vire_le")));
            super::phrase_en_tete(v, p)
        }),
        "exporter" => exporter(ctx, charge).map(|mut v| {
            // Ce que l'ecran dit d'un etat : ses donnees personnelles, ses totaux partiels, la regularisation CASC-SVP.
            v["ecran"] = ecran_d_un_etat(&v);
            // Ce que l'etat dit de lui-meme (ce qui est du, ce qui est infere), sous la phrase : ce que l'ecran montre au-dessus du tableau.
            v["phrases"] = v["avertissements"].clone();
            let titre = if v["titre"].is_string() { t(&v, "titre") } else { t(&v, "periode") };
            let p = match v["fichier"].as_str() {
                Some(f) => format!("{} : écrit dans {}.", titre, f),
                None => format!("{} : aperçu, rien n'est écrit.", titre),
            };
            super::phrase_en_tete(v, p)
        }),
        "dossier-envoyer" => dossier_envoyer(ctx, charge, &mut || client_courriel(ctx)).map(|v| {
            let fichiers: Vec<String> = v["fichiers"].as_array().into_iter().flatten().map(valeur_en_texte).collect();
            let p = format!("Dossier de {} ({}) : {}.", v["salaries"].as_array().into_iter().flatten().map(valeur_en_texte).collect::<Vec<_>>().join(", "), t(&v, "periode"), super::compte(fichiers.len(), "fichier joint", "fichiers joints"));
            super::phrase_en_tete(v, p)
        }),
        "casc-envoyer" => casc_envoyer(ctx, charge, &mut || client_courriel(ctx)).map(|v| {
            let partis: Vec<String> = v["partis"].as_array().into_iter().flatten().map(valeur_en_texte).collect();
            let annees: Vec<String> = v["annees"].as_array().into_iter().flatten().map(|a| super::mot(&a["annee"])).collect();
            let notee = if annees.len() > 1 { "notées régularisées" } else { "notée régularisée" };
            let p = format!("CASC-SVP, {} : {}, envoyé à {} ; {} {}.", t(&v, "periode"), socle::texte::euros_de(&v["total"]), partis.join(" et "), annees.join(" et "), notee);
            super::phrase_en_tete(v, p)
        }),
        "droits" => droits(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `ctx.racine / "sorties" / mois` : `mois` doit etre un texte (`TypeError` sinon,
/// avec la phrase de `PosixPath.__truediv__`).
fn sous_sorties(ctx: &Contexte, mois: &Value) -> R<std::path::PathBuf> {
    match mois {
        Value::String(m) => Ok(ctx.racine.join("sorties").join(m)),
        autre => Err(erreur(
            "TypeError",
            format!("'mois' : {}", socle::fichiers::attendu("un texte", nom_type(autre))),
        )),
    }
}

/// Les noms des PDF d'un dossier (sensible à la casse, fichiers cachés compris),
/// en ordre de points de code.
fn pdf_de(d: &Path) -> R<Vec<String>> {
    let mut noms = Vec::new();
    // Ce qui n'est pas un dossier, ou un dossier interdit, n'a aucun PDF :
    // rien ne se refuse.
    let lecture = match std::fs::read_dir(d) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(20) => return Ok(noms),
        Err(e) => return Err(erreur_io(&e, d)),
    };
    for e in lecture {
        let e = e.map_err(|e| erreur_io(&e, d))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".pdf") {
            noms.push(n);
        }
    }
    noms.sort();
    Ok(noms)
}

/// `documents_du_mois` : les fichiers du mois a ouvrir ou a remettre a la main.
pub(crate) fn documents_du_mois(ctx: &Contexte, mois: &Value, quoi: &Value) -> R<Value> {
    let base = sous_sorties(ctx, mois)?;
    let envoi = base.join("envoi");
    // `{...}.get(quoi)` : un texte seulement se trouve ; une liste ou un dictionnaire ne se hache pas.
    let prefixes: Option<&[&str]> = match quoi {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(q) if q == "contrats" => Some(&["Contrat_", "Contrats_"]),
        Value::String(q) if q == "documents" => Some(&["Bulletin_", "CertificatCS_", "AER_"]),
        _ => None,
    };
    let mut out = Vec::new();
    for (d, est_base) in [(&envoi, false), (&base, true)] {
        if !d.exists() {
            continue;
        }
        for nom in pdf_de(d)? {
            // Le PDF a imprimer reunit les pieces de plusieurs salaries : chacune se liste seule, lui jamais.
            if nom.starts_with(crate::compagnie::signature::PREFIXE_IMPRESSION) {
                continue;
            }
            if let Some(p) = prefixes {
                if !p.iter().any(|x| nom.starts_with(x)) {
                    continue;
                }
            }
            if est_base && vrai(quoi) {
                continue; // les fichiers groupes du mois : pas ici
            }
            let chemin = d.join(&nom);
            let st = std::fs::metadata(&chemin).map_err(|e| erreur_io(&e, &chemin))?;
            let stem = nom.strip_suffix(".pdf").filter(|s| !s.is_empty()).unwrap_or(&nom);
            let morceaux: Vec<&str> = stem.split('_').collect();
            let salarie = if morceaux.len() > 2 { morceaux[1] } else { "" };
            out.push(json!({"nom": nom, "octets": st.len(), "salarie": salarie}));
        }
    }
    Ok(Value::Array(out))
}

// -------------------------------------------------------------- dossier-formes


/// Les années des mois, de la plus récente à la plus ancienne.
fn annees_de(dispo: &[String]) -> Vec<String> {
    let mut annees: Vec<String> = dispo.iter().map(|m| m.chars().take(4).collect::<String>()).collect::<HashSet<_>>().into_iter().collect();
    annees.sort();
    annees.reverse();
    annees
}

/// Les salaries que la page propose.
fn salaries_de_la_page(ctx: &Contexte) -> Vec<Value> {
    let mut fiches: Vec<(&String, &Value)> = ctx.salaries.iter().filter(|(k, v)| !k.starts_with('_') && v.is_object()).collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    fiches
        .into_iter()
        .map(|(k, v)| {
            let nom = format!(
                "{} {}",
                valeur_en_texte(v.get("nom").unwrap_or(texte_vide())),
                valeur_en_texte(ou(v.get("prenom_affichage").unwrap_or(&Value::Null), v.get("prenom").unwrap_or(texte_vide())))
            );
            json!({"cle": k, "nom": nom, "actif": v.get("actif").map_or(true, vrai), "email": v.get("email").is_some_and(vrai)})
        })
        .collect()
}

/// Ce que la page propose -- les salaries,
/// les categories a cocher, les periodes, et si l'envoi est possible.
fn dossier_formes(ctx: &Contexte) -> R<Value> {
    let salaries = salaries_de_la_page(ctx);
    // `periodes_exportables` : les mois, puis (sans rien en dire) le logiciel du comptable.
    let dispo: Vec<String> = documents::etats_paie::mois_disponibles(ctx)?.into_keys().collect();
    let mois: Vec<&String> = dispo.iter().collect();
    let annees = annees_de(&dispo);
    etat_logiciel(ctx)?;
    let boite = crate::routes::ouverture::boite_liee(ctx)?;
    Ok(json!({
        "salaries": salaries,
        "categories": [
            {"categorie": "bulletins", "quoi": "Bulletins de paie"},
            {"categorie": "contrats", "quoi": "Contrats"},
            {"categorie": "contrats-signes", "quoi": "Contrats signés"},
            {"categorie": "certificats-cs", "quoi": "Certificats Congés Spectacles"},
            {"categorie": "attestations", "quoi": "Attestations employeur (AER, AEM)"},
            {"categorie": "documents-fin", "quoi": "Documents de fin de contrat"},
            {"categorie": "recapitulatif", "quoi": "Fiche récapitulative"},
        ],
        "mois": mois,
        "annees": annees,
        "boite_liee": boite,
        "dossier": ctx.racine.join("exports").join("dossiers").display().to_string(),
    }))
}

// ------------------------------------------------- le logiciel du comptable

/// Ce que la page montre -- les logiciels,
/// celui retenu (`config/compta.json`), et ce que le plan comptable ne tiendrait
/// pas dans son format.
pub(crate) fn etat_logiciel(ctx: &Contexte) -> R<Value> {
    let chemin = ctx.racine.join("config").join("compta.json");
    let choix = if chemin.exists() { lire_json(&chemin)? } else { json!({}) };
    let retenu = getd(&choix, "logiciel", &Value::Null)?;
    let retenu = ou(retenu, texte_vide());
    let analytique = vrai(getd(&choix, "analytique", &Value::Null)?);
    let controle = match retenu {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(nom) => {
            let logiciels = tables()["logiciels"].as_array().expect("logiciels");
            match logiciels.iter().find(|l| l["logiciel"].as_str() == Some(nom)) {
                Some(l) => controler_plan(ctx, l["format"].as_str().expect("format"))?,
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    // Ce que l'ecran dit de chaque logiciel, une fois choisi.
    let mut logiciels = tables()["logiciels"].clone();
    for l in logiciels.as_array_mut().into_iter().flatten() {
        l["dit"] = json!(format!("Fichier {} : {}. Retenu pour les prochaines fois.", super::mot(&l["format_lisible"]), super::mot(&l["pourquoi"])));
    }
    Ok(json!({"logiciels": logiciels, "logiciel": retenu, "analytique": analytique, "controle": controle}))
}

/// La page ne propose pas la CASC-SVP de la
/// 3090 a une compagnie de la 1285, ni la dette de conges a qui n'emploie pas de
/// permanents.
fn etat_concerne(ctx: &Contexte, etat: &str) -> bool {
    let idcc = valeur_en_texte(ou(ctx.entreprise.get("idcc").unwrap_or(&Value::Null), texte_vide()));
    let t = tables();
    if let Some(liste) = t["si_convention"].get(etat) {
        if !idcc.is_empty() && !liste.as_array().expect("liste").iter().any(|c| c.as_str() == Some(idcc.as_str())) {
            return false;
        }
    }
    if t["si_permanents"].as_array().expect("permanents").iter().any(|e| e.as_str() == Some(etat)) {
        return ctx.emploie_des_permanents();
    }
    true
}

/// Les mois et les annees qu'il y a
/// matiere a exporter, les etats, les formats, et le logiciel du comptable. `xlsx`
/// (la bibliotheque est-elle la ?) et `pdf` (la fabrique est-elle installee ?)
/// sont deux faits du poste, que la page dit dans la charge.
fn exports_periodes(ctx: &Contexte, charge: &Value) -> R<Value> {
    let dispo: Vec<String> = documents::etats_paie::mois_disponibles(ctx)?.into_keys().collect();
    let t = tables();
    let etats: Vec<&Value> = t["etats"].as_array().expect("etats").iter().filter(|e| etat_concerne(ctx, e["etat"].as_str().expect("etat"))).collect();
    let annees = annees_de(&dispo);
    let compta = etat_logiciel(ctx)?;
    let fait = |k: &str| charge.get(k).is_some_and(vrai);
    Ok(json!({
        "mois": dispo,
        "annees": annees,
        "etats": etats,
        "formats": t["formats"],
        "regroupements": t["regroupements"],
        "dossier": ctx.racine.join("exports").display().to_string(),
        "xlsx": fait("xlsx"),
        "pdf": fait("pdf"),
        "compta": compta,
    }))
}

/// Le logiciel du comptable, et l'analytique, retenus une fois pour
/// toutes. Rend le controle du plan pour ce logiciel. Ce n'est pas un export : un reglage, pas une donnee de paie.
fn retenir_logiciel(ctx: &Contexte, charge: &Value) -> R<Value> {
    let logiciel = charge.get("logiciel").filter(|v| vrai(v));
    let analytique = booleen(charge, "analytique", false);
    let nom = logiciel.map(valeur_en_texte).unwrap_or_default();
    if logiciel.is_some() {
        let connus = documents::plan_comptable::logiciels_connus();
        if !connus.iter().any(|l| *l == nom) {
            return Err(refus(format!("Logiciel inconnu : « {} ».", nom)));
        }
    }
    let chemin = ctx.racine.join("config").join("compta.json");
    let contenu = json!({
        "_quoi": "Le logiciel du comptable, retenu pour les écritures comptables (Exports).",
        "logiciel": nom,
        "analytique": analytique,
    });
    ecrire_json(&chemin, &contenu, 2).map_err(|m| erreur("OSError", m))?;
    crate::routes::archives::etat_logiciel(ctx).map_err(Echec::from)
}

/// Note qu'une annee de CASC-SVP est regularisee (apres le virement).
fn casc_regularise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = crate::routes::fiches::exiger(charge, "annee")?;
    let vire_le = crate::routes::fiches::exiger(charge, "vire_le")?;
    let montant = crate::routes::fiches::exiger(charge, "montant")?;
    let aujourd_hui = maintenant();
    let qui = qui_de(charge);
    crate::import::casc::marquer_regularise(ctx, annee, vire_le, montant, &qui, &aujourd_hui[..10])
}

/// `charge[cle] or None` : un texte, ou rien.
fn texte_ou_rien(charge: &Value, cle: &str) -> Option<String> {
    charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// Le verbe unique, `exporter <etat>`. Lit la charge (etat, mois, annee,
/// toutes_les_annees, regroupement, format, coordonnees, apercu, logiciel, analytique), refuse un etat inconnu,
/// ne passe a l'etat que ce que sa forme demande, puis rend l'etat tel que `js.brut` le rend.
fn exporter(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(match exporter_objet(ctx, charge)? {
        Exporte::Casc(v) => v,
        Exporte::Livre(l) => l.json(),
        Exporte::Tableau(t) => documents::tableaux_export::brut_tableau(&t),
    })
}

/// Ce qu'un export rend, avant d'etre mis en JSON (le chemin ecrit en entier, les montants en `Decimal`).
pub(crate) enum Exporte {
    Casc(Value),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON ; sans `format`, le CSV.
pub(crate) fn exporter_objet(ctx: &Contexte, charge: &Value) -> R<Exporte> {
    use documents::etats_paie::Demande;
    let etat = texte_ou_rien(charge, "etat").unwrap_or_else(|| "livre-de-paie".to_string());
    let tables = documents::plan_comptable::tables();
    let etats = tables["etats"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let Some(forme) = etats.iter().find(|e| e["etat"].as_str() == Some(etat.as_str())) else {
        let mut noms: Vec<&str> = etats.iter().filter_map(|e| e["etat"].as_str()).collect();
        noms.sort_unstable();
        return Err(refus(format!("État inconnu : « {} ». Les états possibles sont {}.", etat, noms.join(", "))));
    };
    let mut d = Demande {
        mois: texte_ou_rien(charge, "mois"),
        annee: texte_ou_rien(charge, "annee"),
        toutes: charge.get("toutes_les_annees").is_some_and(vrai),
        regroupement: texte_ou_rien(charge, "regroupement").unwrap_or_else(|| "salarie".to_string()),
        format: texte_ou_rien(charge, "format").unwrap_or_else(|| "csv".to_string()),
        coordonnees: charge.get("coordonnees").is_some_and(vrai),
        ecrire: !charge.get("apercu").is_some_and(vrai),
        logiciel: texte_ou_rien(charge, "logiciel"),
        analytique: charge.get("analytique").map(vrai),
        departs: envois::journal::lu(&ctx.racine).map(|j| j.departs)?,
    };
    if etat == "regularisation-casc-svp" {
        let demande = crate::import::casc::Demande { mois: d.mois.clone(), annee: d.annee.clone(), toutes_les_annees: d.toutes, boite_liee: crate::routes::ouverture::boite_liee(ctx).unwrap_or(false), ..Default::default() };
        let mut r = crate::import::casc::regularisation(ctx, &demande)?;
        if d.ecrire {
            crate::import::casc::ecrire(ctx, &mut r)?;
        }
        return Ok(Exporte::Casc(crate::import::casc::vers_json(&r)));
    }
    if etat == "livre-de-paie" {
        return Ok(Exporte::Livre(documents::etats_paie::livre_de_paie(ctx, &d)?));
    }
    // Ce que l'etat ne demande pas ne lui est pas passe : un regroupement par defaut (« salarie ») n'a pas de
    // sens pour les cotisations.
    let questions: Vec<&str> = forme["questions"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !questions.contains(&"regroupement") {
        d.regroupement = String::new();
    } else {
        let permis: Vec<&str> = forme["regroupements"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if !permis.contains(&d.regroupement.as_str()) {
            d.regroupement = permis.first().map(|s| s.to_string()).unwrap_or_default();
        }
    }
    if !questions.contains(&"coordonnees") {
        d.coordonnees = false;
    }
    let tableau = match etat.as_str() {
        "livre-de-paie-detaille" => documents::etats_paie::livre_detaille(ctx, &d),
        "recapitulatif-annuel" => documents::etats_paie::recapitulatif_annuel(ctx, &d),
        "masse-salariale" => documents::etats_paie::masse_salariale(ctx, &d),
        "etat-des-virements" => documents::etats_paie::etat_des_virements(ctx, &d),
        "prelevement-a-la-source" => documents::etats_paie::prelevement_a_la_source(ctx, &d),
        "cotisations-par-organisme" => declarations::etats_organismes::cotisations_par_organisme(ctx, &d),
        "versements-organismes" => declarations::etats_organismes::versements_organismes(ctx, &d, &crate::compagnie::depart::dsn_du_mois),
        "bordereau-urssaf" => declarations::etats_organismes::bordereau_urssaf(ctx, &d, &crate::compagnie::depart::dsn_du_mois),
        "fcap-svp" => declarations::etats_organismes::fcap_svp(ctx, &d),
        "afdas-2642" => declarations::etats_organismes::afdas_2642(ctx, &d),
        "paritarisme-branche" => declarations::etats_organismes::paritarisme_branche(ctx, &d),
        "reduction-generale" => documents::etats_paie::reduction_generale(ctx, &d),
        "assiettes" => documents::etats_paie::assiettes(ctx, &d),
        "heures-et-cachets" => documents::etats_paie::heures_et_cachets(ctx, &d),
        "cout-par-spectacle" => documents::etats_paie::cout_par_spectacle(ctx, &d),
        "contrats" => documents::etats_personnel::liste_des_contrats(ctx, &d),
        "registre-du-personnel" => documents::etats_personnel::registre_du_personnel(ctx, &d),
        "fichier-du-personnel" => documents::etats_personnel::fichier_du_personnel(ctx, &d),
        "journal-des-declarations" => documents::etats_personnel::journal_des_declarations(ctx, &d, &envois::journal::lire(&ctx.racine, None, None, false).map_err(|m| erreur("OSError", m))?),
        "ecritures-comptables" => documents::etats_compta::ecritures_comptables(ctx, &d),
        "grand-livre-paie" => documents::etats_compta::grand_livre(ctx, &d),
        "balance-paie" => documents::etats_compta::balance(ctx, &d),
        "provision-conges" => documents::etats_compta::provision_conges(ctx, &d),
        "taxe-sur-les-salaires" => documents::etats_compta::taxe_sur_les_salaires(ctx, &d),
        autre => Err(refus(format!("État inconnu : « {} ».", autre))),
    }?;
    Ok(Exporte::Tableau(tableau))
}

const CORPS_DOSSIER: &str = "Bonjour {prenom},\n\nTu trouveras ci-joint ton dossier de salarié{periode} : {liste}.\n\n{reedition}Bien à toi,\n{signataire}\n{raison_sociale}\n";

/// Ce que le courriel dit des contrats, quand le dossier en porte : eux seuls se reeditent (les bulletins et les
/// certificats sont les pages memes de l'archive ; le reste se prend tel quel).
const REEDITION_CONTRATS: &str = "Les contrats ont été réédités par notre logiciel de paie depuis les données\nd'origine : les numéros et les montants sont ceux des contrats qui t'ont été\nremis ; seule la mise en page peut avoir changé depuis.\n\n";

/// La route `dossier-envoyer` : le dossier se refait ici (`dossier_salarie`, un fichier par salarie, ecrit dans
/// `exports/dossiers/`), puis part a chaque salarie par la boite de la compagnie. La reponse est celle du dossier
/// (`dossier_json`) et, sous `envoi`, ce que l'envoi a fait.
fn dossier_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    let mut demande = Map::new();
    for cle in ["salaries", "mois", "annee", "toutes_les_annees", "categories"] {
        if let Some(v) = charge.get(cle) {
            demande.insert(cle.to_string(), v.clone());
        }
    }
    // CE QUI PARTIRAIT, SANS RIEN ECRIRE : chacun a son adresse, puis la preuve ; le dossier ne s'ecrit qu'ensuite.
    let mut apercu = demande.clone();
    apercu.insert("apercu".into(), json!(true));
    let vu = crate::routes::dossier_salarie::construire(ctx, &Value::Object(apercu))?;
    adresses_de(ctx, &vu.salaries)?;
    let a_qui = format!("{} salarie(s) : {}", vu.salaries.len(), vu.salaries.iter().map(|c| ctx.nomme(c)).collect::<Vec<_>>().join(", "));
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let forme = verifier_preuve(ctx, Some(&preuve_de(charge)), &format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois), "envoi du dossier aux salaries", &a_qui)?;
    let d = crate::routes::dossier_salarie::construire(ctx, &Value::Object(demande))?;
    let mut r = crate::routes::dossier_salarie::dossier_json(&d);
    let envoi = envoyer_dossier(ctx, &d, &Preuve { donnee: Some(forme), ..Preuve::default() }, &qui, ouvrir)?;
    if let Value::Object(o) = &mut r {
        o.insert("envoi".into(), envoi);
    }
    Ok(r)
}

/// Chacun de `cles` a une adresse dans sa fiche ; sinon le refus qui le nomme. Rien n'est parti.
fn adresses_de(ctx: &Contexte, cles: &[String]) -> R<Vec<String>> {
    cles.iter()
        .map(|cle| {
            let s = ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
            s.get("email").filter(|v| vrai(v)).map(valeur_en_texte).ok_or_else(|| {
                refus(format!("Pas d'adresse e-mail pour {} : compléter sa fiche avant d'envoyer. Rien n'est parti.", ctx.nomme(cle)))
            })
        })
        .collect()
}

/// Envoie a chaque salarie SON dossier, par la boite de la compagnie.
/// Irreversible : la meme preuve que les pieces du mois, le meme journal. Un dossier `ensemble` ne part a personne.
pub(crate) fn envoyer_dossier(ctx: &Contexte, d: &crate::routes::dossier_salarie::Dossier, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    if d.fichiers.iter().any(|f| nom_du(f).starts_with("Dossiers_")) {
        return Err(refus(
            "Un dossier qui réunit tous les salariés ne s'envoie à personne : il porte la paie de chacun. Refaire un fichier par salarié.",
        ));
    }
    if d.fichiers.len() != d.salaries.len() {
        return Err(refus("Le dossier n'a pas été écrit : rien à envoyer."));
    }
    let mut messages: Vec<Message> = Vec::new();
    let adresses = adresses_de(ctx, &d.salaries)?;
    for ((cle, f), email) in d.salaries.iter().zip(&d.fichiers).zip(adresses) {
        let s = ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
        let prenom = match vrai_ou(s, "prenom_affichage") {
            Some(p) => p,
            None => valeur_en_texte(requis(s, "prenom")?),
        };
        let signataire = ctx.entreprise.get("signataire").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
        let ses_pages = d.pages.get(cle).and_then(Value::as_object);
        let liste = d
            .categories
            .iter()
            .filter(|c| ses_pages.is_some_and(|p| p.contains_key(**c)))
            .map(|c| crate::routes::dossier_salarie::libelle_de(c).to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        let corps = envoi::formater(
            CORPS_DOSSIER,
            &[
                ("prenom", prenom.clone()),
                ("periode", if d.periode == "toute la période" { String::new() } else { format!(" ({})", d.periode) }),
                ("liste", liste),
                ("reedition", if ses_pages.is_some_and(|p| p.contains_key("contrats")) { REEDITION_CONTRATS.to_string() } else { String::new() }),
                ("signataire", vrai_ou(&signataire, "prenom_affichage").or_else(|| signataire.get("prenom").map(valeur_en_texte)).unwrap_or_default()),
                ("raison_sociale", vrai_ou(&ctx.entreprise, "nom_affichage").or_else(|| ctx.entreprise.get("raison_sociale").map(valeur_en_texte)).unwrap_or_default()),
            ],
        );
        messages.push(Message {
            cle: cle.clone(),
            nom: ctx.nomme(cle),
            destinataire: email.to_lowercase(),
            sujet: format!("Ton dossier de salarié — {}", d.periode),
            corps,
            pieces: vec![f.clone()],
            html: String::new(),
        });
    }
    let mut client = ouvrir()?;
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    verifier_preuve(ctx, Some(preuve), &format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois), "envoi du dossier aux salaries", &a_qui)?;
    let expediteur = expediteur_de(client.as_ref());
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "dossier-salarie", None, qui, &e.partis, &e.echoue, &e.message, None, None)?;
            return Err(echec_d_envoi(e.message));
        }
    };
    let detail = json!({
        "periode": d.periode,
        "categories": d.categories,
        "salaries": messages.iter().map(|m| json!({"cle": m.cle, "piece": m.pieces.first().map(|p| nom_du(p))})).collect::<Vec<_>>(),
        "expediteur": expediteur,
    });
    let ligne = noter(ctx, "dossier-salarie", None, qui, true, &partis.join(", "), "", Some(&detail))?;
    Ok(json!({"partis": partis, "messages": messages.len(), "expediteur": expediteur, "ligne": ligne}))
}

/// Un jour ISO en JJ/MM/AAAA.
fn jjmmaaaa(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// Envoie les deux courriels de la regularisation, APRES les virements.
/// Irreversible. `vire_le` : {annee: AAAA-MM-JJ}, une date par virement fait ; le dossier envoye est celui de ces
/// annees-la, exactement.
///
/// Dans cet ordre : la boite, la preuve, le dossier (ecrit, pour que la liste nominative jointe soit celle du jour),
/// l'envoi, puis ce qui s'en garde -- les deux `.eml` dans `exports/casc-svp/envoyes/`, la ligne de
/// journal, et les annees notees regularisees. Rien de tout cela ne s'ecrit si l'envoi casse.
fn casc_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    use crate::import::casc::{ecrire, marquer_regularise, regularisation, Demande};
    // Les virements : une table nom -> valeur, vide si la charge n'en dit pas.
    let vire_le: Vec<(String, Value)> = match charge.get("vire_le").filter(|v| vrai(v)) {
        None => Vec::new(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(autre) => return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    };
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let mut dates: Vec<(String, String)> = Vec::new();
    for (a, d) in &vire_le {
        let texte = valeur_en_texte(d);
        let Some(jour) = jour_iso(&texte) else {
            return Err(refus(format!("Date du virement {} illisible : {} (attendu AAAA-MM-JJ). Rien n'est parti.", a, repr(&d_en_repr(d)))));
        };
        if jour > aujourdhui {
            return Err(refus(format!(
                "Le virement {} est date du {}, qui n'est pas encore arrive : le courriel annonce un virement FAIT. Rien n'est parti.",
                a,
                jjmmaaaa(&jour.iso())
            )));
        }
        match dates.iter_mut().find(|(k, _)| k == a) {
            Some(e) => e.1 = jour.iso(),
            None => dates.push((a.clone(), jour.iso())),
        }
    }
    if dates.is_empty() {
        return Err(refus("Aucune date de virement : les courriels annoncent des virements faits, ils partent apres. Rien n'est parti."));
    }
    let mut annees_voulues: Vec<String> = dates.iter().map(|(a, _)| a.clone()).collect();
    annees_voulues.sort();

    // La boite et la preuve, puis le dossier, ecrit : la liste nominative jointe est celle du jour.
    let mut r = regularisation(ctx, &Demande { toutes_les_annees: true, annees: annees_voulues.clone(), vire_le: dates.clone(), ..Default::default() })?;
    let mut manquent: Vec<String> = annees_voulues.iter().filter(|a| !r.annees.iter().any(|x| &x.annee == *a)).cloned().collect();
    manquent.sort();
    if !manquent.is_empty() {
        return Err(refus(format!("Rien a regulariser pour {} (deja fait, ou aucune DSN deposee). Rien n'est parti.", manquent.join(", "))));
    }

    let mut client = ouvrir()?;
    let dernier_mois = r.mois.last().map(|m| m.mois.clone()).ok_or_else(|| erreur("IndexError", socle::fichiers::trop_courte("les mois de la régularisation")))?;
    verifier_preuve(ctx, Some(&preuve_de(charge)), &dernier_mois, "envoi a Audiens et au CASC-SVP (liste nominative)", "Audiens et CASC-SVP")?;

    // La liste nominative est la premiere chose qu'ecrit le dossier (`casc::ecrire`) : si elle ne peut s'ecrire, le refus
    // tombe avant tout autre fichier, et rien ne part.
    ecrire(ctx, &mut r)?;
    let classeur = std::path::Path::new(&r.dossier).join("CASC-SVP_liste-nominative.xlsx");

    let messages = vec![
        Message {
            cle: "audiens".into(),
            nom: "Audiens".into(),
            destinataire: r.courriel_audiens.a.clone(),
            sujet: r.courriel_audiens.objet.clone(),
            corps: r.courriel_audiens.corps.clone(),
            pieces: Vec::new(),
            html: courriel_en_html(&r.courriel_audiens.corps),
        },
        Message {
            cle: "casc".into(),
            nom: "CASC-SVP".into(),
            destinataire: r.courriel_casc.a.clone(),
            sujet: r.courriel_casc.objet.clone(),
            corps: r.courriel_casc.corps.clone(),
            pieces: vec![classeur],
            html: String::new(),
        },
    ];
    let compte = client.compte().to_string();
    let mut copies: Vec<Vec<u8>> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &compte, client.as_mut(), None, Some(&mut copies)) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "regularisation-casc", None, &qui, &e.partis, &e.echoue, &e.message, Some(json!({"annees": annees_voulues})), None)?;
            return Err(echec_d_envoi(e.message));
        }
    };

    // Ce que l'envoi garde : les deux messages tels qu'ils sont partis.
    let garde = std::path::Path::new(&r.dossier).join("envoyes");
    std::fs::create_dir_all(&garde).map_err(|e| erreur("OSError", socle::echec::du_disque(&garde, &e)))?;
    let iso = socle::paris::maintenant();
    let horodate = format!("{}_{}{}{}", &iso[..10], &iso[11..13], &iso[14..16], &iso[17..19]);
    let mut emls: Vec<String> = Vec::new();
    for (m, brut) in messages.iter().zip(&copies) {
        let p = garde.join(format!("{}_CASC-SVP_{}.eml", horodate, m.cle));
        std::fs::write(&p, brut).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
        emls.push(nom_du(&p));
    }
    let date_de = |a: &str| dates.iter().find(|(k, _)| k == a).map(|(_, d)| d.clone()).unwrap_or_default();
    let detail = json!({
        "annees": r.annees.iter().map(|a| json!({"annee": a.annee, "vire_le": date_de(&a.annee), "montant": a.cotisation.to_string()})).collect::<Vec<_>>(),
        "mois": r.mois.iter().map(|m| m.mois.clone()).collect::<Vec<_>>(),
        "total": r.total.to_string(),
        "expediteur": compte,
        "eml": emls,
    });
    let ligne = noter(ctx, "regularisation-casc", None, &qui, true, &partis.join(", "), &format!("{} courriel(s) envoye(s)", partis.len()), Some(&detail))?;
    // Les annees regularisees : leurs mois sortent des dossiers suivants. S'ecrit APRES le virement, jamais avant.
    let auj = socle::paris::maintenant();
    let mut notees: Vec<Value> = Vec::new();
    for a in &r.annees {
        notees.push(marquer_regularise(ctx, &json!(a.annee), &json!(date_de(&a.annee)), &json!(a.cotisation.to_string()), &qui, &auj[..10])?);
    }
    Ok(json!({
        "partis": partis,
        "expediteur": compte,
        "periode": r.periode,
        "total": r.total.to_string(),
        "annees": notees,
        "eml": emls,
        "dossier": garde.to_string_lossy(),
        "ligne": ligne,
    }))
}

/// Le texte d'une valeur de `vire_le` comme `%r` l'ecrit : ce qui n'est pas un texte s'ecrit sans guillemets.
fn d_en_repr(d: &Value) -> String {
    valeur_en_texte(d)
}

/// L'erreur du disque, sous sa classe (`FileNotFoundError`, `PermissionError`, `OSError`).
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match (e.kind(), e.raw_os_error()) {
        (std::io::ErrorKind::NotFound, _) => "FileNotFoundError",
        (std::io::ErrorKind::PermissionDenied, _) => "PermissionError",
        (_, Some(20)) => "NotADirectoryError",
        (_, Some(21)) => "IsADirectoryError",
        _ => "OSError",
    };
    erreur(classe, socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------------------------ les droits des intermittents, l'archive d'un mois

/// LES HEURES ET CACHETS DES INTERMITTENTS au regard des annexes 8 et 10 (`droit::cumul` : la fenetre, le plafond des
/// cachets d'un mois, l'enseignement plafonne, le seuil, lus du bareme) : pour chaque salarie dont un engagement est connu
/// (importe ou valide), sa position dans sa fenetre et le tableau du mois. Le compte ne porte que les engagements de cette
/// compagnie : la phrase le dit.
fn droits(ctx: &Contexte, charge: &Value) -> R<Value> {
    use droit::cumul::{anniversaire_configure, position_droits, tableau_cumul};
    let mois = match charge.get("mois").filter(|v| vrai(v)) {
        Some(m) => valeur_en_texte(m),
        None => format!("{:04}-{:02}", ctx.aujourdhui.an, ctx.aujourdhui.mois),
    };
    let engs = engagement::cumul::lire_engagements(ctx, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let cles: BTreeSet<&str> = engs.iter().map(|e| e.salarie.as_str()).collect();
    let mut salaries = Vec::new();
    for cle in cles {
        let anniv = anniversaire_configure(ctx, cle)?;
        let p = position_droits(ctx, cle, &engs, anniv, None)?;
        let c = &p.constantes;
        let h = |n: i64| format!("{} h", n);
        let mut lignes = vec![json!(["Cachets déclarés", p.cachets().to_string()])];
        if p.cachets_perdus() != 0 {
            lignes.push(json!([format!("Retenus ({} par mois au plus)", c.cachets_par_mois), p.cachets_retenus().to_string()]));
        }
        lignes.push(json!([format!("Convertis ({} h par cachet)", c.heures_par_cachet), h(p.cachets_retenus() * c.heures_par_cachet)]));
        lignes.push(json!(["Heures déclarées", h(p.heures())]));
        lignes.push(json!(["Total d'heures équivalentes", h(p.heures_retenues())]));
        if p.heures_enseignement() != 0 {
            // Deux comptes des qu'il y a de l'enseignement : ces heures ouvrent les droits (plafonnees), sans augmenter l'allocation.
            lignes.push(json!(["Dont enseignement", h(p.heures_enseignement())]));
            lignes.push(json!([format!("Enseignement retenu (plafond {} h)", p.plafond_enseignement()?), h(p.heures_enseignement_retenues()?)]));
            lignes.push(json!(["Pour l'ouverture des droits", h(p.heures_ouverture()?)]));
            lignes.push(json!(["Pour le montant de l'allocation", h(p.heures_montant())]));
        }
        lignes.push(json!(["Seuil des annexes 8 et 10", h(c.seuil_heures)]));
        let jours = p.jours_restants(ctx.aujourdhui);
        if anniv.is_some() && jours > 0 {
            lignes.push(json!(["Avant le réexamen", format!("{} j", jours)]));
        }
        let phrase = if p.atteint()? {
            format!("Seuil atteint, avec {} h de marge.", p.heures_ouverture()? - c.seuil_heures)
        } else {
            format!("Il manque {} h, soit environ {} cachets.", p.manque()?, p.cachets_manquants()?)
        };
        let mut notes = Vec::new();
        if p.cachets_perdus() != 0 {
            notes.push(format!(
                "{} cachet(s) dépassent les {} d'un mois ({}) : joués, payés et déclarés, mais sans effet sur les droits.",
                p.cachets_perdus(),
                c.cachets_par_mois,
                p.mois_plafonnes().join(", ")
            ));
        }
        if p.enseignement_perdu()? != 0 {
            notes.push(format!("{} h d'enseignement dépassent le plafond : payées et déclarées, mais sans effet sur les droits.", p.enseignement_perdu()?));
        }
        if p.heures_enseignement() != 0 && p.date_naissance.is_none() {
            notes.push("Date de naissance absente de la fiche : le plafond bas de l'enseignement s'applique, peut-être à tort.".to_string());
        }
        if anniv.is_none() {
            notes.push("Fenêtre déduite du dernier contrat déclaré : la date anniversaire que France Travail notifie (anniversaire_droits, dans la fiche du salarié) prime sur ce calcul.".to_string());
        }
        let colonnes: Vec<Value> = tableau_cumul(ctx, cle, &mois, &engs)?.iter().map(|x| x.canon()).collect();
        let rangee = |quoi: &str, cle: &str, ecrire: &dyn Fn(&Value) -> String| {
            json!({"quoi": quoi, "valeurs": colonnes.iter().map(|x| ecrire(&x[cle])).collect::<Vec<_>>()})
        };
        let heures = |v: &Value| format!("{} h", socle::num::dec(v).map(socle::texte::virgule).unwrap_or_else(|_| valeur_en_texte(v)));
        let nom = ctx.salaries.get(cle).map(crate::routes::nom_a_l_ecran).unwrap_or_else(|| cle.to_string());
        salaries.push(json!({
            "cle": cle,
            "nom": nom,
            "fenetre": p.libelle_fenetre,
            "lignes": lignes,
            "phrase": phrase,
            "notes": notes,
            "colonnes": {
                "titres": colonnes.iter().map(|x| format!("{} {}", valeur_en_texte(&x["titre"]), valeur_en_texte(&x["sous_titre"])).trim().to_string()).collect::<Vec<_>>(),
                "lignes": [rangee("Cachets", "cachets", &valeur_en_texte), rangee("Heures déclarées", "heures", &heures), rangee("Total d'heures", "heures_equivalentes", &heures), rangee("Salaire brut", "brut", &socle::texte::euros_de)],
            },
        }));
    }
    let phrase = if salaries.is_empty() {
        "Aucun engagement d'intermittent n'est encore validé ni importé."
    } else {
        "Ce compte ne porte que les engagements de cette compagnie : le relevé de France Travail seul fait foi, et le seuil se compte sur tous les employeurs. Ces totaux sont un plancher."
    };
    let phrases: Vec<String> = salaries.iter().map(|s| format!("{} ({}) : {}", valeur_en_texte(&s["nom"]), valeur_en_texte(&s["fenetre"]), valeur_en_texte(&s["phrase"]))).collect();
    Ok(json!({"mois": mois, "phrase": phrase, "phrases": phrases, "salaries": salaries}))
}


/// CE QUE L'ECRAN DES EXPORTS DIT (`ecran` de `exports-periodes`) : rien a exporter, les regroupements, l'analytique, le
/// format, les coordonnees, le dossier.
fn ecran_des_exports(v: &Value) -> Value {
    json!({
        "vide": if super::nombre(&v["mois"]) == 0 { json!("Aucune paie saisie : il n'y a rien à exporter pour l'instant.") } else { Value::Null },
        "regroupement_livre": "Par salarié, ce qu'on envoie au comptable ; par mois, ce qu'on rapproche de la banque ; par spectacle, ce qui justifie une subvention. Les trois disent la même chose, et le total ne bouge pas.",
        "regroupement": "Le total ne bouge pas : seule la répartition des lignes change.",
        "analytique": "Les salaires bruts ventilés par spectacle. Le code de chaque spectacle se règle sur sa fiche : Configuration › Spectacles, case « Code analytique ». Vide, il est tiré du titre.",
        "format_libre": "Le format se choisit ci-dessous.",
        "brique_absente": "brique absente, le CSV sera écrit",
        "coordonnees": "Le NIR, la naissance, l'adresse. Sans la case, rien de tout cela ne sort ; un fichier qui les porte ne s'envoie qu'à qui en a besoin.",
        "dossier": format!("{} — et nulle part ailleurs : un export emporte des salaires", super::mot(&v["dossier"])),
        "categories": "Une seule case : un PDF. Plusieurs : un ZIP, un PDF par catégorie.",
    })
}

/// CE QUE L'ECRAN DIT D'UN ETAT EXPORTE (`ecran` de `exporter`) : ses donnees personnelles, ses totaux partiels, et pour
/// la regularisation CASC-SVP ce que disent ses deux etapes.
fn ecran_d_un_etat(v: &Value) -> Value {
    let bulletins = v["totaux"]["bulletins"].as_u64().unwrap_or(0);
    let partiels: Vec<String> = v["totaux"]["inconnus"].as_object().into_iter().flatten().map(|(c, n)| {
        let sur = bulletins.saturating_sub(n.as_u64().unwrap_or(0));
        format!("{} sur {} bulletin{} sur {}", c.replace('_', " "), sur, if sur == 1 { "" } else { "s" }, bulletins)
    }).collect();
    json!({
        "personnelles": if vrai(&v["donnees_personnelles"]) { json!("Cet état porte des données personnelles : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.") } else { Value::Null },
        "partiels": if partiels.is_empty() { Value::Null } else { json!(format!("Totaux partiels : {}.", partiels.join(" ; "))) },
        "virement": "Vers Audiens Santé Prévoyance – Encaissement. L'IBAN se lit dans l'espace employeur Audiens ou sur la page employeurs du CASC-SVP.",
        "automatique": if vrai(&v["boite_liee"]) { "Audiens reçoit les masses et les dates des virements ; le CASC-SVP, la liste nominative. Une copie reste dans le dossier, et les années sont notées régularisées." } else { "La boîte courriel n'est pas posée : Configuration › Courriel. Une fois posée, l'envoi se fait d'ici en un clic." },
        "a_la_main": "Joindre la liste nominative au courriel du CASC-SVP, recopier les deux courriels ci-dessous, puis noter la régularisation.",
    })
}
