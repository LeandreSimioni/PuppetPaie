//! Configuration › Entreprise, Salariés, Spectacles : les fiches des salaries, des spectacles et de l'entreprise
//! (`verifier-fiche`, `activer-fiche`, `enregistrer-fiche`, leur avertissement, `permanents`), leur demande par courriel
//! (`fiche-demander`, `fiches-relever`, `fiche-oublier-demande`), le taux AT/MP (`taux-atmp`), le regime de la taxe sur
//! les salaires, le cout de l'annee (`couts`), la sauvegarde et la suppression de la compagnie.
//!
//! Les refus portent leur classe (`KeyError`, `TypeError`, `ErreurOperation`...), que la page releve.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::documents::client_courriel;
use crate::routes::{en_json, lire_charge, INTERFACE};
use crate::routes::mois::sauver_avant;
use declarations::champs;
use documents::fiche_questionnaire as fq;
use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::courriel_lecture as lecture;
use envois::envoi::{self, Message};
use envois::journal;
use envois::ligne_d_envoi::{echec_d_envoi, noter, par_courriel};
use envois::simulation_courriels::BoiteQuiRepond;
use socle::date::Jour;
use socle::echec::{erreur, Echec, R};
use socle::ecriture::ecrire_json;
use socle::fichiers::repr;
use socle::html::{html_echapper, mailto};
use socle::texte::{capitaliser, strip};
use documents::valeur::{euros_de, nom_type, valeur_ecrite, valeur_en_texte, valeurs_egales, vrai};

const NOM_FICHE: Champ = Champ { nom: "nom", libelle: Affiche::new("Fiche"), forme: Forme::Texte, sens: "la fiche : entreprise, salarie ou spectacle", avance: false };
const CLE_FICHE: Champ = Champ { nom: "cle", libelle: Affiche::new("Clé"), forme: Forme::Texte, sens: "la clé d'un salarié ou d'un spectacle ; absente pour une fiche neuve ou l'entreprise", avance: false };
const DONNEES: Champ = Champ { nom: "donnees", libelle: Affiche::new("Cases de la fiche"), forme: Forme::Objet, sens: "les cases de la fiche, que la route configuration décrit (description : nom, forme, valeurs) ; une case inconnue se dit", avance: false };
const CHANGEMENTS: Champ = Champ { nom: "changements", libelle: Affiche::new("Cases qui changent"), forme: Forme::Objet, sens: "[{case, valeur}, …], au moins un, dans l'ordre, tout ou rien : chacun une case de la fiche (description de la route configuration : nom, forme, valeurs ; une case inconnue se refuse), null pour la vider (\"\" se refuse) ; une structure se donne entière ; une case non nommée garde sa valeur", avance: false };
pub(crate) const REFUSEE: Refus = Refus { classe: "FicheRefusee", corriger: "les cases que les anomalies nomment ; forcer passe outre une erreur de forme seulement" };
const PREUVE: Refus = Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est envoyé" };
const BOITE: Refus = Refus { classe: "BoiteAbsente", corriger: "poser la boîte courriel (Configuration, Mots de passe)" };
const ETAT_REND: &str = "etat : ce que la page lit de la compagnie (la route etat), relu après l'écriture";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Fiches);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Permanents,
        geste: "Dire les cases que montre la fiche en cours de saisie.",
        charge: &[
            Champ { nom: "entreprise", libelle: Affiche::new("Fiche Entreprise"), forme: Forme::Objet, sens: "la fiche Entreprise en cours de saisie ; sans elle, celle de la compagnie", avance: false },
            Champ { nom: "nom", libelle: Affiche::new("Fiche"), forme: Forme::Texte, sens: "la fiche dont on veut les cases : entreprise (par défaut), salarie ou spectacle", avance: true },
            Champ { nom: "donnees", libelle: Affiche::new("Fiche en cours"), forme: Forme::Objet, sens: "la fiche d'un salarié ou d'un spectacle en cours de saisie (pour entreprise, c'est `entreprise`)", avance: true },
        ],
        exemple: r#"{}"#,
        rend: "permanents (vrai ou faux), montres (les cases que la fiche montre)",
        refus: &[FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::TaxeSurLesSalairesRegime,
        geste: "Dire le régime de la taxe sur les salaires d'après les réponses de la fiche.",
        charge: &[
            Champ { nom: "taxe_sur_les_salaires", libelle: Affiche::new("Réponses"), forme: Forme::Objet, sens: "les réponses en cours de saisie ; sans elles, celles de la compagnie", avance: false },
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année des salaires (ses plafonds) ; l'année en cours sinon", avance: true },
        ],
        exemple: r#"{}"#,
        rend: "franchise (la question et ses plafonds), champs à montrer, consequence en clair, complet, due, part_taxee",
        refus: &[FORME, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::VerifierFiche,
        geste: "Vérifier une fiche avant de l'écrire : anomalies, valeurs déclarées.",
        charge: &[NOM_FICHE, CLE_FICHE, DONNEES],
        exemple: r#"{"nom":"spectacle","donnees":{"titre":"Les Bancs publics"}}"#,
        rend: "anomalies (case, gravité, message), declare (ce que la DSN dirait), suggestions",
        refus: &[MANQUE, FORME, OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EnregistrerFiche,
        geste: "Écrire une fiche (entreprise, salarié, spectacle) ; ce qui change se date.",
        charge: &[
            NOM_FICHE,
            CLE_FICHE,
            CHANGEMENTS,
            Champ { nom: "confirme", libelle: Affiche::new("Avertissement lu"), forme: Forme::OuiNon, sens: "vrai : l'avertissement rendu (ce qui change) a été lu ; la fiche s'écrit", avance: false },
            Champ { nom: "depuis", libelle: Affiche::new("Depuis le"), forme: Forme::Date, sens: "la date d'effet de ce qui change, AAAA-MM-JJ ; la date proposée sinon", avance: false },
            Champ { nom: "note", libelle: Affiche::new("Pourquoi"), forme: Forme::Texte, sens: "pourquoi cela change (correction, déménagement…)", avance: true },
            Champ { nom: "forcer", libelle: Affiche::new("Écrire malgré une erreur de forme"), forme: Forme::OuiNon, sens: "vrai : écrire malgré une erreur de forme", avance: true },
        ],
        exemple: r#"{"nom":"spectacle","changements":[{"case":"titre","valeur":"Les Bancs publics"},{"case":"numero_objet","valeur":"266Z00952012"}]}"#,
        rend: "une valeur existante change, ou un choix se confirme : avertissement (changements, depuis proposé, pourquoi ; usage, la phrase à confirmer) et ses phrases, rien n'est écrit ; sinon : cle, ecrits (les champs changés), enregistre_a (vide si rien n'a changé), phrase",
        refus: &[REFUSEE, OPERATION, MANQUE, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ActiverFiche,
        geste: "Archiver une fiche de salarié ou de spectacle, ou la réactiver.",
        charge: &[NOM_FICHE, CLE_FICHE, Champ { nom: "actif", libelle: Affiche::new("Active"), forme: Forme::OuiNon, sens: "vrai : réactiver ; faux : archiver", avance: false }],
        exemple: r#"{"nom":"spectacle","cle":"le-banc-des-menteurs","actif":true}"#,
        rend: ETAT_REND,
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FicheDemander,
        geste: "Demander à une personne, par courriel, de remplir sa fiche.",
        charge: &[
            Champ { nom: "adresse", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "son courriel", avance: false },
            Champ { nom: "prenom", libelle: Affiche::new("Prénom"), forme: Forme::Texte, sens: "son prénom, pour lui écrire", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "le mot de passe du logiciel, s'il est posé ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{"adresse":"isadora@exemple.invalid","prenom":"Isadora"}"#,
        rend: "code (celui de la demande), adresse",
        refus: &[PREUVE, BOITE, OPERATION],
        acte: Acte::Engage,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FichesRelever,
        geste: "Relever dans la boîte les fiches que les personnes ont remplies.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "attendues, recues (les fiches écrites), incidents",
        refus: &[BOITE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FicheOublierDemande,
        geste: "Renoncer aux demandes de fiche restées sans réponse d'une personne.",
        charge: &[Champ { nom: "adresse", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "le courriel à qui la fiche a été demandée", avance: false }],
        exemple: r#"{"adresse":"isadora@exemple.invalid"}"#,
        rend: "adresse",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::TauxAtmp,
        geste: "Dire le code risque et le taux AT/MP d'une année ; les poser s'il y a lieu.",
        charge: &[
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false },
            Champ { nom: "code_risque", libelle: Affiche::new("Code risque"), forme: Forme::Texte, sens: "le code risque notifié par la CARSAT", avance: false },
            Champ { nom: "valeur", libelle: Affiche::new("Taux net"), forme: Forme::Nombre, sens: "le taux net notifié, en %", avance: false },
        ],
        exemple: r#"{"annee":2026}"#,
        rend: "le code, le taux, leur libellé et d'où ils viennent, les codes que le socle connaît ; phrase : ce que le geste a écrit",
        refus: &[OPERATION, MANQUE, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 60,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Effectif,
        geste: "Dire l'effectif d'une année ; le poser quand l'Urssaf ne l'a pas renvoyé.",
        charge: &[
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année civile mesurée, AAAA ; absente : celles qui manquent pour payer cette année", avance: false },
            Champ { nom: "valeur", libelle: Affiche::new("Effectif annuel moyen"), forme: Forme::Nombre, sens: "son effectif annuel moyen, tel que l'Urssaf l'a calculé (au centième)", avance: false },
        ],
        exemple: r#"{"annee":2024,"valeur":6.5}"#,
        rend: "annee, valeur et d'où elle vient (l'Urssaf, la compagnie, la première embauche), a_dire (les années qui manquent pour payer cette année), phrase",
        refus: &[OPERATION, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 300,
        ecran: Ecran::Auto(Place::Fiches, Affiche::new("Dire ou poser l'effectif")),
    },
    Fiche {
        route: Route::Couts,
        geste: "Dire le coût de l'année, spectacle par spectacle.",
        charge: &[Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false }],
        exemple: r#"{"annee":"2026"}"#,
        rend: "le coût de l'année par spectacle",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
    Fiche {
        route: Route::SauvegardeMaintenant,
        geste: "Sauvegarder maintenant le dossier de la compagnie dans le Drive.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "sauvegarde (la dernière : quand, ce qui est parti), erreur (ce qui l'a empêchée) ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
    Fiche {
        route: Route::CompagnieSupprimer,
        geste: "Supprimer la compagnie de ce poste : avertir, puis effacer tout son dossier.",
        charge: &[
            Champ { nom: "confirme", libelle: Affiche::new("Effacer"), forme: Forme::OuiNon, sens: "vrai : effacer ; sans lui, la route dit seulement ce qui sera effacé et ce qu'elle demande", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "une compagnie réelle : le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain", avance: false },
        ],
        exemple: r#"{"confirme": true}"#,
        rend: "sans confirme : nom, genre (ajoutee ou reelle), avertissement, conservation et sauvegarde à proposer, ou la phrase qui dit pourquoi elle ne se supprime pas ; avec : supprimee, phrase",
        refus: &[Refus { classe: "ErreurOperation", corriger: "ce que la phrase dit (une fictive livrée se remet à zéro, le mot de passe du logiciel refusé, un dossier tenu ouvert) ; rien n'est effacé" }],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
];
/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        // Supprimer la compagnie : avertir, puis effacer. Par `moteur route`, rien a lacher ; la fenetre sert elle-meme la
        // suppression confirmee, pour lacher d'abord ce qu'elle tient de la compagnie (`interface::Fenetre`).
        Route::CompagnieSupprimer => crate::interface::suppression::route(&ctx.racine, charge, &|_| {}).map_err(|m| erreur("ErreurOperation", m)),
        // La sauvegarde seule, comme la seconde moitie d'un passage de la veille (`compagnie::ouverture`).
        Route::SauvegardeMaintenant => {
            use crate::compagnie::ouverture::{ouverture, Clients, Options};
            let opt = Options::de_la_charge(&json!({"sauvegarde_seule": true, "incidents_editeur": false}));
            let r = ouverture(ctx, &opt, Clients::default(), None);
            let erreur: Vec<String> = r.manquants.into_iter().chain(r.incidents).collect();
            let s = crate::compagnie::sauvegarde::derniere(&ctx.racine).en_json();
            let phrase = match (erreur.is_empty(), valeur_en_texte(&s["quand"])) {
                (false, _) => format!("Sauvegarde : {}", erreur.join(" ")),
                (true, q) if q.is_empty() => "Aucune sauvegarde faite.".to_string(),
                (true, q) => format!("Dernière sauvegarde : {}, {} fichiers.", q, valeur_en_texte(&s["fichiers"])),
            };
            Ok(json!({"phrase": phrase, "sauvegarde": s, "erreur": erreur.join(" ")}))
        }
        Route::Couts => couts(o, ctx, charge).map(|mut v| {
            // Chaque spectacle et son cout, comme l'ecran les liste ; le total de l'annee en tete.
            let cout = |x: &Value| x["cout"].as_str().and_then(|c| c.parse::<f64>().ok()).unwrap_or(0.0);
            let spectacles: Vec<(String, Value)> = v["spectacles"].as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default();
            let total: f64 = spectacles.iter().map(|(_, x)| cout(x)).sum();
            let jusqu_a = socle::mois::Mois::en_clair(&valeur_en_texte(&v["jusqu_a"]));
            let p = format!("Coût de {}, jusqu'à {} : {} pour {}.", valeur_en_texte(&v["annee"]), jusqu_a, euros_de(&json!(total)), super::compte(spectacles.len(), "spectacle", "spectacles"));
            // La ligne de chaque spectacle et de chaque salarie, telle que sa fiche la montre.
            for cote in ["spectacles", "salaries"] {
                for x in v[cote].as_object_mut().into_iter().flat_map(|o| o.values_mut()) {
                    let prevu = x["prevu"].as_str().and_then(|c| c.parse::<f64>().ok()).filter(|p| *p != 0.0);
                    x["phrase"] = json!(format!(
                        "coût employeur {} — brut {}, charges {}, {}{}{}",
                        euros_de(&x["cout"]), euros_de(&x["brut"]), euros_de(&x["charges"]),
                        super::compte(x["bulletins"].as_u64().unwrap_or(0) as usize, "bulletin", "bulletins"),
                        if vrai(&x["incomplet"]) { ", coût incomplet avant PuppetPaie" } else { "" },
                        prevu.map_or(String::new(), |p| format!(" · + {} déjà saisis pour la suite de l'année", euros_de(&json!(p))))
                    ));
                }
            }
            v["phrases"] = json!(spectacles.iter().map(|(nom, x)| format!("{} : {} ({}, brut {})", nom, euros_de(&x["cout"]), super::compte(x["bulletins"].as_u64().unwrap_or(0) as usize, "bulletin", "bulletins"), euros_de(&x["brut"]))).collect::<Vec<_>>());
            super::phrase_en_tete(v, p)
        }),
        Route::Permanents => en_json(permanents(ctx, lire_charge(charge)?)?),
        Route::TaxeSurLesSalairesRegime => en_json(taxe_sur_les_salaires_regime(ctx, lire_charge(charge)?)?),
        Route::VerifierFiche => verifier_fiche(charge).map(|mut v| {
            let lignes: Vec<Value> = v["anomalies"].as_array().into_iter().flatten().map(|a| json!(format!("{} : {}", valeur_en_texte(&a["champ"]), valeur_en_texte(&a["message"])))).collect();
            let p = if lignes.is_empty() { "La fiche est juste.".to_string() } else { format!("Anomalies de la fiche : {}.", lignes.len()) };
            v["phrases"] = Value::Array(lignes);
            super::phrase_en_tete(v, p)
        }),
        Route::TauxAtmp => crate::routes::atmp::route(ctx, charge).map(|mut v| {
            // Ce que dit la saisie avancee d'un taux notifie.
            v["aide"] = json!("Le taux vide : celui de l'arrêté de l'année, quand il s'applique au code choisi. Avec 999ZZ, pas de taux.");
            v
        }),
        Route::Effectif => effectif(ctx, charge),
        Route::ActiverFiche => activer_fiche_route(o, ctx, charge),
        Route::EnregistrerFiche => enregistrer_fiche(ctx, charge),
        Route::FicheOublierDemande => oublier_demande(o, ctx, charge).map(|v| {
            let p = format!("Demande à {} abandonnée.", valeur_en_texte(&v["adresse"]));
            super::phrase_en_tete(v, p)
        }),
        Route::FicheDemander => en_json(fiche_demander(ctx, lire_charge(charge)?)?),
        Route::FichesRelever => {
            let r = relever_fiches(ctx, None, "interface")?;
            let phrase = match (r.recues.is_empty(), r.incidents.first()) {
                (false, _) => format!("{}.", r.resume()),
                (true, Some(i)) => i.clone(),
                (true, None) => format!("Aucune réponse pour l'instant ; fiches attendues : {}.", r.attendues),
            };
            Ok(json!({"phrase": phrase, "attendues": r.attendues, "recues": r.recues, "incidents": r.incidents}))
        }
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ l'effectif

/// Ou une compagnie trouve l'effectif d'une annee : la moyenne de ses effectifs mensuels (L. 130-1, I), chaque salarie a due
/// proportion de ses jours et de son temps partiel (R. 130-1, II), que l'Urssaf renvoie en compte rendu de nature 118.
const OU_TROUVER_L_EFFECTIF: &str =
    "C'est la moyenne de ses effectifs mensuels, chaque salarié compté au prorata de ses jours sous contrat et de son temps partiel (R. 130-1 CSS) ; l'Urssaf la renvoie en compte rendu « Effectifs » (nature 118) : `annee` et `valeur`.";

/// L'EFFECTIF D'UNE ANNEE (`config/effectifs.json`, A01) : le dire, et le poser quand la compagnie le donne pour une annee
/// que l'Urssaf n'a pas renvoyee en compte rendu de nature 118 (`envois::releve_retours::poser_effectif`). L'effectif
/// annuel est la moyenne des mois de l'annee civile, tronquee au centieme (CSS, L. 130-1, I, et R. 130-1, I :
/// `atelier/reference/sources/code-de-la-securite-sociale/LISEZ-MOI.md`) ; il ne se dit qu'une annee finie. Sans
/// `annee` : les annees qui manquent pour payer cette annee (`Effectif::a_dire`).
fn effectif(ctx: &Contexte, charge: &Value) -> R<Value> {
    let an_paie = ctx.aujourdhui.an;
    // Les annees qui manquent : celles des seuils (`Effectif::a_dire`), et celle d'avant quand la paie la lit elle-meme et
    // ne la connait pas (le FNAS de la CCNEAC sans CSE dit) : ce que « Pour démarrer » demande (`demarrage::manques`).
    let a_dire = |ctx: &Contexte| -> R<Vec<i32>> {
        let e = ctx.effectif().map_err(Echec::from)?;
        let mut v = e.a_dire(an_paie);
        let demande = declarations::demarrage::manques(ctx).map_err(Echec::from)?.iter().any(|m| m["cle"] == "effectif");
        if demande && e.valeur(an_paie).is_none() && !v.contains(&(an_paie - 1)) {
            v.push(an_paie - 1);
            v.sort();
        }
        Ok(v)
    };
    let Some(an) = charge.get("annee").filter(|a| vrai(a)) else {
        let manquent = a_dire(ctx)?;
        let phrase = if manquent.is_empty() {
            format!("L'effectif de la paie de {} est connu.", an_paie)
        } else {
            format!(
                "Pour payer en {}, il manque l'effectif de {}. {}",
                an_paie,
                manquent.iter().map(i32::to_string).collect::<Vec<_>>().join(", "),
                OU_TROUVER_L_EFFECTIF
            )
        };
        return Ok(json!({"a_dire": manquent, "phrase": phrase}));
    };
    let an = entier(an)? as i32;
    let mut phrase = String::new();
    if let Some(v) = charge.get("valeur").filter(|v| !v.is_null() && valeur_en_texte(v).trim() != "") {
        if an >= an_paie {
            return Err(erreur("ErreurOperation", format!("L'effectif de {} se calcule sur l'année entière : il se dit une fois l'année finie. Rien n'est écrit.", an)));
        }
        let lu = valeur_en_texte(v).replace(',', ".");
        let x: f64 = lu.trim().parse().ok().filter(|x: &f64| x.is_finite() && *x >= 0.0).ok_or_else(|| erreur("ValueError", format!("Effectif illisible : {}.", valeur_ecrite(v))))?;
        // Tronque au centieme (R. 130-1, I).
        let x = (x * 100.0).trunc() / 100.0;
        let le = ctx.aujourdhui.iso();
        if !envois::releve_retours::poser_effectif(&ctx.racine, an, x, &le)? {
            return Err(erreur("ErreurOperation", format!("L'Urssaf a renvoyé l'effectif de {} : c'est le sien qui vaut. Rien n'est écrit.", an)));
        }
        phrase = format!("Effectif de {} : {}.", an, socle::num::dec_f64(x).map(|e| socle::texte::virgule(e.round_dp(2))).unwrap_or_default());
    }
    let e = ctx.effectif().map_err(Echec::from)?;
    let lu = std::fs::read_to_string(ctx.racine.join(droit::contexte::EFFECTIFS)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null);
    let dit = lu.pointer(&format!("/annees/{}/A01/dit_le", an)).is_some();
    let (valeur, source) = match e.annees.get(&an) {
        Some(v) => (json!(v), if dit { "la compagnie" } else { "l'Urssaf" }),
        None => match e.annee(an) {
            Some(v) => (json!(v), "la première embauche"),
            None => (Value::Null, ""),
        },
    };
    if phrase.is_empty() {
        phrase = match valeur.as_f64() {
            Some(v) => format!("Effectif de {} : {}, d'après {}.", an, socle::num::dec_f64(v).map(|e| socle::texte::virgule(e.round_dp(2))).unwrap_or_default(), source),
            None => format!("L'effectif de {} n'est pas connu. {}", an, OU_TROUVER_L_EFFECTIF),
        };
    }
    Ok(json!({"annee": an, "valeur": valeur, "source": source, "a_dire": a_dire(ctx)?, "phrase": phrase}))
}

// ------------------------------------------------------------------ permanents

/// La charge de `permanents` : la fiche Entreprise en cours de saisie (sans elle, celle de la compagnie) ; ou une autre
/// fiche (`nom` : salarie, spectacle) et ses donnees en cours de saisie.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargePermanents {
    #[serde(default)]
    entreprise: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    nom: Option<String>,
    #[serde(default)]
    donnees: Option<serde_json::Map<String, Value>>,
}

/// Ce que rend `permanents` : la phrase, le curseur des permanents, et les cases que la fiche montre.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Permanents {
    phrase: &'static str,
    permanents: bool,
    montres: Vec<String>,
}

/// `permanents` : rend le curseur (`contexte::emploie_des_permanents`) : la page montre ou replie ce qui ne vaut que
/// pour les permanents selon cette reponse, et ne le decide pas elle-meme quand on coche la case.
fn permanents(ctx: &Contexte, charge: ChargePermanents) -> R<Permanents> {
    let saisie = charge.entreprise.map(Value::Object);
    let entreprise: &dyn declarations::champs::ParCase = match &saisie {
        Some(e) => e,
        None => &ctx.entreprise,
    };
    let nom = charge.nom.as_deref().unwrap_or("entreprise");
    let autre = Value::Object(charge.donnees.unwrap_or_default());
    let donnees: &dyn declarations::champs::ParCase = if nom == "entreprise" { entreprise } else { &autre };
    let p = permanents_de(entreprise);
    Ok(Permanents {
        phrase: if p { "La compagnie emploie des permanents." } else { "La compagnie n'emploie pas de permanents." },
        permanents: p,
        montres: montres(ctx, nom, donnees, entreprise)?,
    })
}

/// Le curseur des permanents (`contexte::emploie_des_permanents`) de la fiche entreprise telle que l'ecran la saisit.
fn permanents_de(entreprise: &dyn declarations::champs::ParCase) -> bool {
    entreprise.case("emploie_des_permanents") == Some(Value::Bool(true))
}

/// CE QUE LA FICHE `nom` MONTRE pour `donnees` : chaque case que `champs::Champ::montre` dit a l'ecran, d'apres la
/// fiche, l'entreprise `entreprise` (ses permanents, ses conditions) et les etablissements de la compagnie. La page ne
/// montre que ces cases-la.
pub(crate) fn montres(ctx: &Contexte, nom: &str, donnees: &dyn declarations::champs::ParCase, entreprise: &dyn declarations::champs::ParCase) -> R<Vec<String>> {
    let permanents = permanents_de(entreprise);
    let etablissements = super::ouverture::choix_d_etablissement(ctx).is_some();
    let autre = (nom != "entreprise").then_some(entreprise);
    Ok(champs::fiche(nom)?.iter().filter(|c| c.montre(donnees, autre, permanents, etablissements)).map(|c| c.cle.clone()).collect())
}

// ------------------------------------------------------- taxe-sur-les-salaires-regime

/// La charge de `taxe-sur-les-salaires-regime` : les reponses en cours de saisie (absentes : celles de la compagnie ;
/// nulles : aucune), et l'annee des salaires (absente : l'annee en cours).
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeTaxeSurLesSalaires {
    #[serde(default, deserialize_with = "socle::forme::dite")]
    taxe_sur_les_salaires: Option<Option<serde_json::Map<String, Value>>>,
    #[serde(default)]
    annee: Option<i64>,
}

/// Les plafonds de la franchise en base de TVA, au bareme de l'annee.
#[derive(Debug, Clone, serde::Serialize)]
struct PlafondsDeLaFranchise {
    prestations_de_services: String,
    total: String,
}

/// Les deux reponses a la question de la franchise : sous les plafonds (`true`), au-dessus (`false`).
#[derive(Debug, Clone, serde::Serialize)]
struct ChoixDeLaFranchise {
    #[serde(rename = "true")]
    sous: String,
    #[serde(rename = "false")]
    dessus: String,
}

/// La question de la franchise : son aide, ses reponses, ses plafonds et l'annee.
#[derive(Debug, Clone, serde::Serialize)]
struct Franchise {
    aide: String,
    choix: ChoixDeLaFranchise,
    plafonds: Option<PlafondsDeLaFranchise>,
    annee: i32,
}

/// Ce que rend `taxe-sur-les-salaires-regime` : la phrase (la consequence), la question de la franchise, les cases a
/// montrer, la consequence, si la reponse est complete, si la taxe est due et sur quelle part.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RegimeDeLaTaxe {
    phrase: String,
    franchise: Franchise,
    champs: Vec<&'static str>,
    consequence: String,
    complet: bool,
    due: bool,
    part_taxee: Option<String>,
}

/// `taxe-sur-les-salaires-regime` : ce que la fiche en cours de saisie dit -- sans lui, celle de la compagnie. Rend la
/// conclusion du moteur (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`, CGI art. 231, 1) : les cases a montrer,
/// la consequence en clair, et si la reponse est complete. La page n'en decide rien.
fn taxe_sur_les_salaires_regime(ctx: &Contexte, charge: ChargeTaxeSurLesSalaires) -> R<RegimeDeLaTaxe> {
    let cle = documents::taxe_sur_les_salaires::CHAMP_TAXE_SUR_LES_SALAIRES;
    let ent = match charge.taxe_sur_les_salaires {
        Some(None) => droit::fiche::ChampsEntreprise { taxe_sur_les_salaires: droit::fiche::Case::Nulle, ..Default::default() },
        Some(Some(v)) => droit::fiche::ChampsEntreprise {
            taxe_sur_les_salaires: droit::fiche::Case::Dite(droit::fiche::lire_champs(&Value::Object(v), cle).map_err(plain)?),
            ..Default::default()
        },
        None => ctx.entreprise.clone(),
    };
    // LES PLAFONDS DE LA FRANCHISE, CHIFFRES : ceux du bareme de l'annee des salaires -- `annee` dans la
    // charge, sinon l'annee en cours. Un bareme absent ne fait pas refuser : la question se pose sans montants.
    let annee = match charge.annee {
        None => ctx.aujourdhui.an,
        Some(a) if (1900..=2999).contains(&a) => a as i32,
        Some(a) => return Err(erreur("ValueError", socle::date::annee_impossible(a))),
    };
    let bar = socle::date::Jour::nouveau(annee, 1, 1).and_then(|j| ctx.bareme(&j).ok());
    let q = documents::taxe_sur_les_salaires::question_franchise(bar.as_deref(), annee);
    // La conclusion nomme les deux plafonds.
    let r = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ent, q.plafonds);
    Ok(RegimeDeLaTaxe {
        phrase: r.consequence.clone(),
        franchise: Franchise {
            aide: q.aide,
            choix: ChoixDeLaFranchise { sous: q.sous, dessus: q.dessus },
            plafonds: q.plafonds.map(|(s, t)| PlafondsDeLaFranchise { prestations_de_services: s.normalize().to_string(), total: t.normalize().to_string() }),
            annee,
        },
        champs: r.champs,
        consequence: r.consequence,
        complet: r.decision.is_ok(),
        due: matches!(r.decision, Ok(Some(_))),
        part_taxee: match r.decision {
            Ok(Some(p)) => Some(p.normalize().to_string()),
            _ => None,
        },
    })
}

// ------------------------------------------------------------------ outils

/// `charge["cle"]`.
pub(crate) fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    match charge {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", nom_type(autre)))),
    }
}

/// `champs.FICHES[nom]` : le nom d'une fiche connue, `KeyError` sinon,
/// `TypeError` quand ce qui est donne ne se hache pas.
pub(crate) fn nom_de_fiche(v: &Value) -> R<&str> {
    match v {
        Value::String(s) if matches!(s.as_str(), "entreprise" | "salarie" | "spectacle") => Ok(s),
        Value::Array(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        autre => Err(erreur("KeyError", format!("fiche inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ taux-atmp

/// Un entier, de ce qu'un JSON porte.
pub(crate) fn entier(v: &Value) -> R<i64> {
    let trop_grand = || Echec::from("annee hors de la plage d'un entier de 64 bits".to_string());
    match v {
        Value::Bool(b) => Ok(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => {
                let x = n.as_f64().unwrap_or(0.0).trunc();
                if x.abs() < 9.0e18 {
                    Ok(x as i64)
                } else {
                    Err(trop_grand())
                }
            }
        },
        Value::String(t) => {
            let illisible = || erreur("ValueError", format!("nombre entier illisible : {}", repr(t)));
            let u = t.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c));
            let (neg, chiffres) = match u.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, u.strip_prefix('+').unwrap_or(u)),
            };
            let c: Vec<char> = chiffres.chars().collect();
            let bien_forme = !c.is_empty()
                && crate::routes::fiches::chiffre_lu(c[0]).is_some()
                && crate::routes::fiches::chiffre_lu(c[c.len() - 1]).is_some()
                && c.iter().all(|x| crate::routes::fiches::chiffre_lu(*x).is_some() || *x == '_')
                && !chiffres.contains("__");
            if !bien_forme {
                return Err(illisible());
            }
            let ascii: String = c.iter().filter(|x| **x != '_').filter_map(|x| crate::routes::fiches::chiffre_lu(*x)).map(|d| char::from(b'0' + d as u8)).collect();
            let n: i64 = ascii.parse().map_err(|_| trop_grand())?;
            Ok(if neg { -n } else { n })
        }
        autre => Err(erreur(
            "TypeError",
            socle::fichiers::attendu("un nombre entier", nom_type(autre)),
        )),
    }
}

/// Le JSON d'un fichier :
/// `None` quand le fichier est absent, illisible comme texte ou comme JSON ;
/// un refus du disque se leve.
pub(crate) fn lire_tolerant(p: &Path) -> R<Option<Value>> {
    if !p.exists() {
        return Ok(None);
    }
    let octets = std::fs::read(p).map_err(|e| {
        let classe = match e.kind() {
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        };
        erreur(classe, socle::echec::du_disque(&p, &e))
    })?;
    Ok(String::from_utf8(octets).ok().and_then(|t| serde_json::from_str(&t).ok()))
}

// ------------------------------------------------------------ verifier-fiche

/// Un objet, de ce qu'un JSON porte : un objet se copie ; une liste
/// de paires aussi ; le reste se refuse sous `TypeError`.
pub(crate) fn copie_en_objet(v: &Value) -> R<Map<String, Value>> {
    let paires: Vec<Value> = match v {
        Value::Object(o) => return Ok(o.clone()),
        Value::Array(a) => a.clone(),
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        autre => return Err(erreur("TypeError", socle::fichiers::attendu("une liste", nom_type(autre)))),
    };
    let mut sortie = Map::new();
    for (i, p) in paires.iter().enumerate() {
        let couple: Vec<Value> = match p {
            Value::Array(a) => a.clone(),
            Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
            Value::Object(o) => o.keys().map(|k| Value::String(k.clone())).collect(),
            _ => return Err(erreur("TypeError", socle::fichiers::pas_une_paire(i, nom_type(p)))),
        };
        if couple.len() != 2 {
            return Err(erreur("ValueError", socle::fichiers::paire_attendue(i, couple.len())));
        }
        match &couple[0] {
            Value::String(k) => {
                sortie.insert(k.clone(), couple[1].clone());
            }
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            _ => {}
        }
    }
    Ok(sortie)
}

/// Une valeur en texte, vide si elle manque ou est vide.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// `faute_de_norme(champ, actuel)` pour une valeur deja la, vraie : un texte
/// passe par le controle de la norme ; ce qui n'en est pas un se refuse sous les
/// memes `TypeError` que la table des formats.
fn fautif(c: &champs::Champ, actuel: &Value) -> R<bool> {
    if let Value::String(s) = actuel {
        return Ok(champs::faute_de_norme(c, s)?.is_some());
    }
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(false) };
    let format = declarations::formats::format_de(rubrique)?;
    if format.as_object().map_or(true, |o| o.is_empty()) {
        return Ok(false);
    }
    let motifs = format.get("csl").and_then(Value::as_array).map_or(false, |l| !l.is_empty());
    let pas_un_texte = || erreur("TypeError", socle::fichiers::attendu("un texte", nom_type(actuel)));
    let mut faute = false;
    if let Some(lon) = format.get("longueur").and_then(Value::as_array).filter(|l| !l.is_empty()) {
        let n = match actuel {
            Value::Array(a) => a.len() as i64,
            Value::Object(o) => o.len() as i64,
            autre => return Err(erreur("TypeError", socle::fichiers::attendu("un texte ou une liste", nom_type(autre)))),
        };
        let (mini, maxi) = (lon[0].as_i64().unwrap_or(0), lon[1].as_i64().unwrap_or(0));
        faute = !(mini <= n && n <= maxi);
    }
    if !faute && motifs {
        return Err(pas_un_texte());
    }
    if faute && motifs {
        return Err(pas_un_texte());
    }
    Ok(faute)
}

/// Sans accents, en capitales, espaces resserrees.
fn capitales(v: &Value) -> String {
    let t: String = champs::sans_accents(&valeur_en_texte(v)).chars().flat_map(char::to_uppercase).collect();
    t.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Les formes declarees du signataire et du contact DSN ;
/// rend `{cle: [avant, apres]}` pour les structures modifiees.
fn deriver_personnes(donnees: &mut Map<String, Value>, forcer: bool) -> Map<String, Value> {
    let mut faits = Map::new();
    // LE CONTACT DSN ET CELUI DE LA MEDECINE DU TRAVAIL INTERNE : la norme les ecrit en « nom et prenom » en une seule
    // rubrique (S20.G00.07.001, `norme-dsn/texte/bloc-S20.G00.07.txt` l. 63-71) ; la fiche les saisit en une case
    // (`nom_prenom_affichage`), dont la forme declaree est la meme en capitales sans accents.
    let personnes: [(&str, &[&str], Option<(&str, &[&str])>); 3] = [
        ("signataire", &["nom", "prenom"], None),
        ("declarant_dsn", &[], Some(("nom_prenom", &["nom_prenom"]))),
        ("contact_sante_travail", &[], Some(("nom_prenom", &["nom_prenom"]))),
    ];
    for (cle, officiels, compose) in personnes {
        let Some(Value::Object(p)) = donnees.get(cle) else { continue };
        let mut neuf = p.clone();
        let faux = |m: &Map<String, Value>, k: &str| m.get(k).map_or(true, |v| !vrai(v));
        for k in officiels {
            let lisible = neuf.get(&format!("{}_affichage", k)).cloned().unwrap_or(Value::Null);
            if vrai(&lisible) && (forcer || faux(&neuf, k)) {
                neuf.insert(k.to_string(), Value::String(capitales(&lisible)));
            }
        }
        if let Some((cible, morceaux)) = compose {
            let lisibles: Vec<Value> = morceaux.iter().map(|m| neuf.get(&format!("{}_affichage", m)).cloned().unwrap_or(Value::Null)).collect();
            if lisibles.iter().any(vrai) && (forcer || faux(&neuf, cible)) {
                let mots: Vec<String> = lisibles.iter().filter(|x| vrai(x)).map(valeur_en_texte).collect();
                neuf.insert(cible.to_string(), Value::String(capitales(&Value::String(mots.join(" ")))));
            }
        }
        if !valeurs_egales(&Value::Object(neuf.clone()), &Value::Object(p.clone())) {
            faits.insert(cle.to_string(), json!([Value::Object(p.clone()), Value::Object(neuf.clone())]));
            donnees.insert(cle.to_string(), Value::Object(neuf));
        }
    }
    faits
}

/// LE SIREN ET LE NIC SE DEDUISENT DU SIRET (regle 2 : la page ne les pose pas) : le SIRET
/// est le SIREN (9 chiffres) suivi du NIC (5 chiffres) -- c'est sa definition (INSEE), aucune valeur ne se devine. Un
/// SIRET qui n'a pas 14 chiffres ne deduit rien : sa case le dit (`champs::verifier_fiche`). Rend `{cle: [avant, apres]}`.
fn deriver_siren_nic(donnees: &mut Map<String, Value>) -> Map<String, Value> {
    let mut faits = Map::new();
    let siret = texte_ou_vide(donnees.get("siret")).trim().to_string();
    if siret.chars().count() != 14 || !siret.chars().all(|c| c.is_ascii_digit()) {
        return faits;
    }
    for (cle, valeur) in [("siren", &siret[..9]), ("nic", &siret[9..])] {
        let attendu = Value::String(valeur.to_string());
        let actuel = donnees.get(cle).cloned();
        if actuel.as_ref() != Some(&attendu) {
            faits.insert(cle.into(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
            donnees.insert(cle.into(), attendu);
        }
    }
    faits
}

/// LES AUTRES ETABLISSEMENTS S'ECRIVENT COMME LE SIEGE : le NIC se lit dans le SIRET (ses cinq derniers chiffres),
/// l'adresse et la commune declarees se fabriquent depuis leur forme lisible comme celles du siege (`deriver`), le code
/// risque s'ecrit sans point ni blanc (S21.G00.40.040, « 923AD »), le taux saisi en nombre (`atmp::taux_saisi`).
fn deriver_etablissements(donnees: &mut Map<String, Value>) -> R<()> {
    let Some(Value::Array(liste)) = donnees.get("etablissements").cloned() else { return Ok(()) };
    let fiche = champs::fiche("entreprise")?;
    let derive = |cle: &str, lisible: &Value| -> R<Option<Value>> {
        let Some(c) = fiche.iter().find(|c| c.cle == cle) else { return Ok(None) };
        Ok(Some(champs::normaliser(c, lisible)?))
    };
    let mut sortie = Vec::new();
    for e in liste {
        let Value::Object(mut o) = e else { continue };
        let siret: String = texte_ou_vide(o.get("siret")).chars().filter(|c| !c.is_whitespace()).collect();
        o.insert("siret".into(), Value::String(siret.clone()));
        if siret.chars().count() == 14 && siret.chars().all(|c| c.is_ascii_digit()) {
            o.insert("nic".into(), Value::String(siret[9..].to_string()));
        }
        for (declare, lisible) in [("adresse", "adresse_affichage"), ("commune", "commune_affichage")] {
            if let Some(v) = o.get(lisible).filter(|v| vrai(v)).cloned() {
                if let Some(d) = derive(declare, &v)? {
                    o.insert(declare.into(), d);
                }
            }
        }
        if let Some(Value::Object(t)) = o.get_mut("taux_atmp") {
            let code = texte_ou_vide(t.get("code_risque")).trim_matches(socle::texte::blanc).replace(['.', ' '], "").to_uppercase();
            t.insert("code_risque".into(), Value::String(code));
            match crate::routes::atmp::taux_saisi(t.get("valeur"))? {
                Some(v) => t.insert("valeur".into(), json!(v.to_string().parse::<f64>().unwrap_or(0.0))),
                None => t.shift_remove("valeur"),
            };
        }
        // Le versement mobilite de sa commune : un taux (zero : aucun) et le code INSEE de la commune que la DSN declare.
        if let Some(Value::Object(vm)) = o.get_mut("versement_mobilite") {
            let insee = texte_ou_vide(vm.get("code_insee")).trim().to_uppercase();
            let brut = vm.get("taux").filter(|t| !t.is_null()).map(valeur_en_texte).unwrap_or_default().replace(',', ".").replace('%', "");
            if brut.trim().is_empty() {
                vm.shift_remove("taux");
            } else {
                let t = socle::num::decimal_lu(brut.trim())
                    .ok_or_else(|| erreur("ErreurOperation", format!("Taux du versement mobilité illisible : {}.", brut.trim())))?;
                vm.insert("taux".into(), json!(t.to_string().parse::<f64>().unwrap_or(0.0)));
            }
            if insee.is_empty() {
                vm.shift_remove("code_insee");
            } else {
                vm.insert("code_insee".into(), Value::String(insee));
            }
        }
        if o.get("versement_mobilite").is_some_and(|v| sans_valeur(Some(v))) {
            o.shift_remove("versement_mobilite");
        }
        sortie.push(Value::Object(o));
    }
    donnees.insert("etablissements".into(), Value::Array(sortie));
    Ok(())
}

/// Fabrique les formes declarees depuis les formes lisibles.
/// Sans `forcer`, on ne remplit que ce qui est vide ou indeclarable. Rend
/// `{cle: [avant, apres]}` : ce que le programme a fabrique tout seul.
pub(crate) fn deriver(nom: &str, donnees: &mut Map<String, Value>, forcer: bool) -> R<Map<String, Value>> {
    let mut faits = Map::new();
    for c in champs::fiche(nom)? {
        let Some(source) = c.derive_de.as_deref().filter(|d| !d.is_empty()) else { continue };
        let Some(Value::String(texte)) = donnees.get(source).cloned() else { continue };
        if texte.is_empty() {
            continue;
        }
        let attendu = champs::normaliser(&c, &Value::String(texte))?;
        let actuel = donnees.get(&c.cle).cloned();
        if actuel.as_ref() == Some(&attendu) {
            continue;
        }
        if let Some(a) = actuel.as_ref().filter(|a| vrai(a)) {
            if !forcer && !fautif(&c, a)? {
                continue;
            }
        }
        faits.insert(c.cle.clone(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
        donnees.insert(c.cle.clone(), attendu);
    }
    if nom == "entreprise" {
        // Le numero d'adherent Conges Spectacles s'ecrit dans sa forme (`conges_spectacles::forme_adherent`), avant d'etre
        // controle et enregistre : « 0123 450001-f » est la meme valeur que « 0123450001F ».
        let cle = documents::conges_spectacles::CLE_ADHERENT;
        if let Some(Value::String(brut)) = donnees.get(cle).cloned() {
            let forme = documents::conges_spectacles::forme_adherent(&brut);
            if forme != brut {
                faits.insert(cle.into(), json!([brut, forme.clone()]));
                donnees.insert(cle.into(), Value::String(forme));
            }
        }
        faits.extend(deriver_personnes(donnees, forcer));
        faits.extend(deriver_siren_nic(donnees));
        deriver_etablissements(donnees)?;
    }
    // LE COMPTE BANCAIRE S'ECRIT COMPACT : l'IBAN et le BIC sans blancs, en majuscules -- « FR76 3000 6000 … » est le
    // meme compte que « FR763000 6000… » (`socle::iban::iban_compact`, communique IBAN de la Banque centrale de Turquie,
    // annexe 3 : `reference/sources/iban/`).
    if let Some(Value::Object(b)) = donnees.get_mut("banque") {
        for k in ["iban", "bic"] {
            if let Some(Value::String(brut)) = b.get(k).cloned() {
                let forme = socle::iban::iban_compact(&brut);
                if forme != brut {
                    b.insert(k.into(), Value::String(forme));
                }
            }
        }
    }
    // LE DEPARTEMENT DE NAISSANCE SE LIT DANS LE NIR (regle 2 : la page le montre, le moteur l'ecrit) : ses 6e et 7e
    // caracteres sont le departement de naissance, 99 pour une personne nee a l'etranger (S21.G00.30.001 et .014,
    // `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Il ne s'ecrit que dans une case vide : une
    // valeur saisie qui le contredit se dit (`champs::controles_croises`), elle ne se remplace pas.
    if nom == "salarie" && socle::texte::strip(&texte_ou_vide(donnees.get("departement_naissance"))).is_empty() {
        if let Some(dep) = champs::departement_du_nir(&texte_ou_vide(donnees.get("nir"))) {
            faits.insert("departement_naissance".into(), json!([donnees.get("departement_naissance").cloned().unwrap_or(Value::Null), dep]));
            donnees.insert("departement_naissance".into(), Value::String(dep));
        }
    }
    if nom == "salarie" && socle::texte::strip(&texte_ou_vide(donnees.get("initiales_contrat"))).is_empty() {
        let lettres: String = texte_ou_vide(donnees.get("nom")).chars().filter(|c| c.is_ascii_uppercase()).collect();
        if !lettres.is_empty() {
            let deux: String = lettres.chars().take(2).collect();
            faits.insert("initiales_contrat".into(), json!(["", deux.clone()]));
            donnees.insert("initiales_contrat".into(), Value::String(deux));
        }
    }
    // LA NATIONALITE DSN SE DEDUIT DU PAYS : avec un pays de nationalite de la liste, la case 30.013 ne se
    // choisit plus, le moteur l'ecrit (`pays::codification`). Sans pays, ou avec un pays inconnu (qui refuse sur sa
    // case), la case reste ce qu'elle est.
    if nom == "salarie" {
        if let Some(Value::String(code)) = donnees.get("pays_nationalite").filter(|v| vrai(v)).cloned() {
            if let Some(p) = socle::pays::du_code(&code).map_err(Echec::from)? {
                let attendu = Value::String(socle::pays::codification(&p).to_string());
                let actuel = donnees.get("code_nationalite_dsn").cloned();
                if actuel.as_ref() != Some(&attendu) {
                    faits.insert("code_nationalite_dsn".into(), json!([actuel.unwrap_or(Value::Null), attendu.clone()]));
                    donnees.insert("code_nationalite_dsn".into(), attendu);
                }
            }
        }
    }
    Ok(faits)
}

/// Ce que le moteur PROPOSERAIT pour une case encore vide.
fn suggestions(nom: &str, donnees: &Map<String, Value>) -> R<Value> {
    let mut propose = Map::new();
    if nom == "salarie" {
        if let Some(dep) = champs::departement_du_nir(&texte_ou_vide(donnees.get("nir"))) {
            let mut trouvees = vec![("departement_naissance", dep.clone())];
            if dep != "99" {
                trouvees.push(("pays_naissance", "FR".to_string()));
            }
            for (cle, valeur) in trouvees {
                if socle::texte::strip(&texte_ou_vide(donnees.get(cle))).is_empty() {
                    propose.insert(cle.into(), json!({"valeur": valeur, "pourquoi": "lu dans le numéro de sécurité sociale"}));
                }
            }
        }
    } else if nom == "entreprise" && socle::texte::strip(&texte_ou_vide(donnees.get("code_urssaf"))).is_empty() {
        let code = declarations::urssaf::base().map_err(Echec::from)?.du_code_postal(&texte_ou_vide(donnees.get("code_postal")));
        if !code.is_empty() {
            propose.insert(
                "code_urssaf".into(),
                json!({
                    "valeur": code,
                    "pourquoi": "l'organisme du département, d'après votre code postal — à corriger si le vôtre diffère (versement en lieu unique, pôle grandes entreprises, GUSO)",
                }),
            );
        }
    }
    Ok(Value::Object(propose))
}

/// Les anomalies de la fiche APRES derivation, sur une copie.
pub(crate) fn verifier(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut copie = donnees.clone();
    deriver(nom, &mut copie, false)?;
    champs::verifier_fiche(nom, &copie)
}

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme `ecrire_fiche` la
/// derive avant d'ecrire (`deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu
/// de l'import les montre telles : verifier la fiche avant derivation ferait crier chaque champ que le bulletin ou
/// la DSN imprime en minuscules (« 3 quai de la Fosse ») « doit etre en MAJUSCULES » sous sa forme lisible,
/// alors que l'ecriture le declare bien en majuscules.
///
/// Chaque anomalie porte le libelle de sa case (`libelle`), celui que la
/// fiche montre : la page le dit a l'utilisateur, pas la cle.
pub(crate) fn verifier_comme_a_l_ecriture(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut copie = donnees.clone();
    deriver(nom, &mut copie, true)?;
    let mut anomalies = champs::verifier_fiche(nom, &copie)?;
    avec_libelles(nom, &mut anomalies)?;
    Ok(anomalies)
}

/// Pose sur chaque anomalie le libelle de sa case (`libelle`), celui que la fiche montre.
pub(crate) fn avec_libelles(nom: &str, anomalies: &mut [Value]) -> R<()> {
    let champs = champs::fiche(nom)?;
    for a in anomalies.iter_mut() {
        let cle = a.get("champ").and_then(Value::as_str).unwrap_or("").to_string();
        // Le code PCS-ESE ne se tape pas : la fiche le remplit du metier choisi (`emplois.rs`).
        if cle == "pcs_ese" {
            a["libelle"] = json!("Métier (il donne le code PCS-ESE)");
        } else if let Some(c) = champs.iter().find(|c| c.cle == cle) {
            a["libelle"] = json!(c.libelle);
        }
    }
    Ok(())
}

/// `verifier-fiche` : les anomalies de la fiche telle qu'elle SERAIT derivee, ce qui
/// serait declare, et ce que le moteur proposerait pour les cases vides.
fn verifier_fiche(charge: &Value) -> R<Value> {
    let nom_brut = exiger(charge, "nom")?;
    let donnees = exiger(charge, "donnees")?;
    let mut copie = copie_en_objet(donnees)?;
    let nom = nom_de_fiche(nom_brut)?;
    deriver(nom, &mut copie, false)?;
    let mut anomalies = champs::verifier_fiche(nom, &copie)?;
    // Une case que la fiche ne decrit pas se dit comme a l'ecriture (`ecrire_fiche`).
    anomalies.extend(champs::cases_inconnues(nom, &copie, true)?);
    let objet = donnees.as_object();
    let declare = match objet {
        Some(o) => champs::apercu_derive(nom, o)?,
        None if champs::fiche(nom)?.iter().any(champs::Champ::calcule) => {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(donnees))));
        }
        None => json!({}),
    };
    let propose = match objet {
        Some(o) => suggestions(nom, o)?,
        None if nom == "spectacle" => json!({}),
        None => return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(donnees)))),
    };
    Ok(json!({"anomalies": anomalies, "declare": declare, "suggestions": propose}))
}

// ------------------------------------------------------ l'avertissement d'une fiche

/// Ou l'import copie les pieces qu'il reprend de l'ancien logiciel, les DSN deposees comprises (`historique/bulletins/`).
pub(crate) fn pieces_importees(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// `usages_declares` : combien de DSN deja deposees declarent cette valeur dans la rubrique de la case (`rubrique`, de la
/// table des champs), telle qu'une DSN l'ecrit, et sur quels mois (`AAAA-MM`). Une case sans rubrique n'est dans aucune
/// DSN.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DejaDeclare {
    pub(crate) declarations: usize,
    pub(crate) du: Option<String>,
    pub(crate) au: Option<String>,
}

fn usages_declares(ctx: &Contexte, rubrique: Option<&str>, valeur: &Value) -> R<DejaDeclare> {
    let valeur = valeur_en_texte(valeur);
    let (mut n, mut mois) = (0usize, Vec::<String>::new());
    if let Some(rubrique) = rubrique {
        for dossier in [pieces_importees(ctx), ctx.racine.join("sorties")] {
            for f in declarations::dsn_lue::fichiers(&dossier) {
                let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
                if d.declare(rubrique, &valeur) {
                    // Un signalement declare la valeur sans mois principal (S20.G00.05.005).
                    n += 1;
                    mois.extend(d.mois.map(|m| m.to_string()));
                }
            }
        }
    }
    Ok(DejaDeclare { declarations: n, du: mois.iter().min().cloned(), au: mois.iter().max().cloned() })
}

/// Une cle que l'ecran ne montre pas et qu'une ecriture garde : une note de la fiche (`droit::fiche::est_une_note`), ou
/// une marque que le moteur a posee dans une structure (`_...`).
pub(crate) fn interne(k: &str) -> bool {
    k.starts_with('_') || droit::fiche::est_une_note(k)
}

/// L'AVERTISSEMENT D'UN ENREGISTREMENT DE FICHE : ce qu'il ferait changer (chaque valeur existante qui change demande sa
/// date d'effet), avec ce que la valeur sortante a deja servi a declarer, et la date que le moteur propose
/// (`date_proposee`, `depuis` et `pourquoi`) ; et ce qui se confirme sans se dater (`usage`, le jour fixe du versement des
/// intermittents : `jour_des_intermittents_a_confirmer`). `None` : rien n'est a dater ni a confirmer. N'ecrit rien.
fn avertissement_de_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, donnees: &Value) -> R<Option<Value>> {
    let vide = Value::Object(Map::new());
    let montree = crate::routes::etablissements::entreprise_montree(ctx);
    // L'ecriture d'une fiche compare ses cases par leur nom (`champs-base.json`) : la fiche lue, en JSON.
    let lue = cle.and_then(|k| ctx.salaries.get(k)).map(|f| socle::ecriture::en_valeur(&f.champs));
    let spectacle_lu = cle.and_then(|k| ctx.spectacles.get(k)).map(socle::ecriture::en_valeur);
    let ancienne: &Value = match nom {
        "entreprise" => &montree,
        "salarie" => lue.as_ref().unwrap_or(&vide),
        "spectacle" => spectacle_lu.as_ref().unwrap_or(&vide),
        autre => return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    };
    let usage = if nom == "entreprise" { jour_des_intermittents_a_confirmer(ancienne, donnees) } else { None };
    let changements = changements(ctx, nom, ancienne, copie_en_objet(donnees)?)?;
    if changements.is_empty() {
        return Ok(usage.map(|u| json!({"changements": [], "usage": u})));
    }
    let (depuis, pourquoi) = date_proposee(ctx, nom, cle)?;
    let mut a = json!({"changements": changements, "depuis": depuis.iso(), "pourquoi": pourquoi});
    if let Some(u) = usage {
        a["usage"] = json!(u);
    }
    Ok(Some(a))
}

/// Le jour du versement des intermittents que `donnees` ecrit, quand il change et que la configuration le fait confirmer
/// (`droit::versement::Versement::a_confirmer_pour_les_intermittents`) : la phrase a lire avant d'ecrire.
fn jour_des_intermittents_a_confirmer(ancienne: &Value, donnees: &Value) -> Option<String> {
    let cle = droit::versement::CLE_INTERMITTENTS;
    let neuf = donnees.get(cle)?.as_str()?.trim();
    let avant = ancienne.get(cle).and_then(Value::as_str).unwrap_or("").trim();
    (neuf != avant).then(|| droit::versement::Versement::a_confirmer_pour_les_intermittents(neuf)).flatten()
}

/// L'avertissement en phrases : ce qui se confirme, chaque valeur qui change, puis la date proposee et ce qui l'ecrit.
fn avertissement_en_phrases(a: &Value) -> Vec<Value> {
    let dite = |v: &Value| if sans_valeur(Some(v)) { "vide".to_string() } else { valeur_en_texte(v) };
    let mut lignes: Vec<Value> = a.get("usage").into_iter().cloned().collect();
    lignes.extend(a["changements"].as_array().into_iter().flatten().map(|c| {
        let (avant, apres) = if c.get("avant_lisible").is_some() { (&c["avant_lisible"], &c["apres_lisible"]) } else { (&c["avant"], &c["apres"]) };
        json!(format!("{} : {} devient {}.", valeur_en_texte(&c["libelle"]), dite(avant), dite(apres)))
    }));
    if a.get("depuis").is_none() {
        lignes.push(json!("La même charge avec « confirme » l'écrit."));
        return lignes;
    }
    let depuis = Jour::lire(a["depuis"].as_str().unwrap_or("")).map(|j| j.jj_mm_aaaa()).unwrap_or_default();
    lignes.push(json!(format!(
        "À partir du {}. {} La même charge avec « confirme » l'écrit (« depuis » pour une autre date).",
        depuis,
        valeur_en_texte(&a["pourquoi"])
    )));
    lignes
}

/// Le coeur de `avertissement_de_fiche` : la fiche `ancienne` (telle que l'ecran l'a montree) et les `donnees` qui la
/// remplaceraient ; chaque valeur existante qui changerait (`crate::routes::fiches::changements_a_dater`), celles-la memes
/// dont l'enregistrement exigera la date d'effet. Un champ declare dit aussi ce que sa valeur sortante a deja servi a
/// declarer.
pub(crate) fn changements(ctx: &Contexte, nom: &str, ancienne: &Value, copie: Map<String, Value>) -> R<Vec<Value>> {
    let mut donnees = copie;
    deriver(nom, &mut donnees, true)?;
    let Value::Object(anc) = ancienne else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(ancienne))));
    };
    let champs_fiche = champs::fiche(nom)?;
    let mut sortie = Vec::new();
    let a_dater = crate::routes::fiches::changements_a_dater(nom, ancienne, &donnees, false)?;
    for cle in &a_dater {
        let cle = cle.clone();
        if nom == "entreprise" && cle == "etablissements" {
            let changes = crate::routes::etablissements::a_dater(anc.get("etablissements"), donnees.get("etablissements"));
            sortie.extend(changes.iter().map(|c| c.en_json()));
            continue;
        }
        let avant = anc.get(&cle).cloned().unwrap_or(Value::Null);
        let apres = donnees[&cle].clone();
        // La forme lisible d'une personne ne se declare pas, mais elle change : elle se date comme le reste.
        let c = champs_fiche.iter().find(|c| c.cle == cle);
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        if c.filter(|c| c.calcule()).and_then(|c| c.derive_de.as_ref()).is_some_and(|s| a_dater.contains(s)) {
            continue;
        }
        let mut entree = json!({
            "champ": cle, "libelle": c.map_or(cle.as_str(), |c| c.libelle.as_str()), "avant": avant, "apres": apres,
        });
        let o = entree.as_object_mut().unwrap();
        if c.is_some_and(|c| c.declare) {
            let deja = usages_declares(ctx, c.and_then(|c| c.rubrique.as_deref()), &avant)?;
            o.insert("deja_declare".into(), serde_json::to_value(deja).map_err(|e| erreur("ValueError", e.to_string()))?);
        }
        if let Some(c) = c.filter(|c| c.calcule()) {
            let source = c.derive_de.as_deref().unwrap_or("");
            let lisible = champs_fiche.iter().find(|x| x.cle == source);
            let vue = champs::combler_affichage(nom, anc)?;
            o.insert("libelle".into(), json!(lisible.map_or(&c.libelle, |l| &l.libelle)));
            o.insert("avant_lisible".into(), vue.get(source).cloned().unwrap_or(Value::Null));
            o.insert("apres_lisible".into(), donnees.get(source).cloned().unwrap_or(Value::Null));
        }
        sortie.push(entree);
    }
    Ok(sortie)
}

/// Le chiffre (0 a 9) que `c` represente : les chiffres ASCII, et
/// ceux des ecritures les plus courantes (arabe, devanagari, pleine chasse).
pub(crate) fn chiffre_lu(c: char) -> Option<u32> {
    const DEBUTS: [u32; 4] = [0x0660, 0x06F0, 0x0966, 0xFF10];
    let u = c as u32;
    if c.is_ascii_digit() {
        return Some(u - 0x30);
    }
    DEBUTS.iter().find(|d| u >= **d && u < **d + 10).map(|d| u - d)
}


#[track_caller]
pub(crate) fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

#[track_caller]
pub(crate) fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

pub(super) fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", socle::echec::charge(cle, "un texte")))
}

pub(crate) fn creer_dossier(d: &Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur("OSError", socle::echec::du_disque(&d, &e)))
}

pub(crate) fn ecrire(chemin: &Path, v: Value) -> R<()> {
    ecrire_json(chemin, &v, 2).map_err(en_os)
}

/// La configuration, relue apres l'ecriture (`web._configuration(Contexte(racine))`).
/// Ce que la page lit de la compagnie (la route `etat`), relu apres une ecriture : la page n'a rien d'autre a relire.
pub(crate) fn etat_relu(o: &HashMap<String, String>, ctx: &Contexte) -> R<Value> {
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    crate::routes::ouverture::route(o, &neuf, Route::Etat, &json!({}))
}

/// L'heure de Paris, a la seconde.
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// Aujourd'hui, a Paris (`date.today()`).
pub(crate) fn aujourd_hui() -> Jour {
    Jour::lire(&maintenant()[..10]).expect("date de Paris")
}

// --------------------------------------------------------------- activer-fiche

fn fichier_de(nom: &str) -> Option<&'static str> {
    match nom {
        "entreprise" => Some("entreprise.json"),
        "salarie" => Some("salaries.json"),
        "spectacle" => Some("spectacles.json"),
        _ => None,
    }
}

/// UNE VALEUR QU'UNE FICHE DATEE A PORTEE : son `depuis`, la valeur en JSON, sa note.
pub(crate) struct Portee {
    pub depuis: Option<Jour>,
    pub valeur: Value,
    pub note: Option<String>,
}

/// TOUTES LES VALEURS QU'UNE FICHE DATEE A PORTEES, champ par champ dans l'ordre de sa forme, chacune dans l'ordre ecrit
/// (`droit::fiche::VoirListe`) : l'historique que l'ecran montre, les gardes qui cherchent tout ce qui nomme une
/// personne, a quelque date que ce soit. Un champ que la fiche ne dit pas n'y est pas.
pub(crate) fn portees<F: droit::fiche::FicheDatee>(f: &F) -> Vec<(&'static str, Vec<Portee>)> {
    struct Toutes(Vec<(&'static str, Vec<Portee>)>);
    impl droit::fiche::VoirListe for Toutes {
        fn liste<T: serde::Serialize>(&mut self, cle: &'static str, liste: &[droit::fiche::Datee<T>]) {
            if !liste.is_empty() {
                self.0.push((cle, liste.iter().map(|e| Portee { depuis: e.depuis, valeur: socle::ecriture::en_valeur(&e.valeur), note: e.note.clone() }).collect()));
            }
        }
    }
    let mut t = Toutes(Vec::new());
    f.voir_listes(&mut t);
    t.0
}

/// Les valeurs que le champ `cle` a portees (`portees`).
pub(crate) fn valeurs_portees<'a>(p: &'a [(&'static str, Vec<Portee>)], cle: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    p.iter().filter(move |(c, _)| *c == cle).flat_map(|(_, l)| l.iter().map(|x| &x.valeur))
}

/// UNE FICHE DATEE QUE LE MOTEUR ECRIT, ses cases nommees comme `champs-base.json` les nomme et dites en JSON comme l'ecran
/// les envoie : ce qui s'ecrit passe par la forme de la fiche (`droit::fiche::lire_saisie`), qui refuse ce qu'elle ne lit
/// pas, et se date par elle (`droit::fiche::FicheDatee::dater`).
pub(crate) trait FicheAEcrire {
    /// La fiche a `jour`, en JSON.
    fn vue_au(&self, jour: &Jour) -> Value;
    /// Les jours ou le champ `cle` prend une valeur (`droit::fiche::FicheDatee::depuis`).
    fn jours_de(&self, cle: &str) -> Vec<Option<Jour>>;
    /// La valeur `valeur` du champ `cle` (une note `_…` comprise), datee a partir de `depuis` ; `fichier` nomme le fichier
    /// dans le refus.
    fn ecrire_case(&mut self, cle: &str, valeur: &Value, depuis: Option<&Jour>, note: Option<&str>, fichier: &str) -> R<()>;
}

impl<F: droit::fiche::FicheDatee> FicheAEcrire for F {
    fn vue_au(&self, jour: &Jour) -> Value {
        socle::ecriture::en_valeur(&droit::fiche::FicheDatee::au(self, jour))
    }

    fn jours_de(&self, cle: &str) -> Vec<Option<Jour>> {
        droit::fiche::FicheDatee::depuis(self, cle)
    }

    fn ecrire_case(&mut self, cle: &str, valeur: &Value, depuis: Option<&Jour>, note: Option<&str>, fichier: &str) -> R<()> {
        if interne(cle) {
            let plate: F::Plate = droit::fiche::lire_saisie(&un_champ(cle, valeur.clone()), fichier).map_err(plain)?;
            droit::fiche::FicheDatee::noter(self, &plate);
            return Ok(());
        }
        // La case, comme un changement de la route (`droit::fiche::Changements`) : une case inconnue se refuse en se nommant.
        let ch: F::Changement = droit::fiche::lire_saisie(&json!({"case": cle, "valeur": valeur}), fichier).map_err(plain)?;
        droit::fiche::FicheDatee::changer(self, ch, depuis, note);
        Ok(())
    }
}

/// LE FICHIER D'UNE FICHE (`config/entreprise.json`, `config/salaries.json`, `config/spectacles.json`), lu par sa forme
/// (`droit::fiche::lire_fichier`) ; absent, il est vide. Il s'ecrit par elle.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FichierDeFiche {
    Entreprise(droit::fiche::FicheEntrepriseDatee),
    Salaries(droit::fiche::Salaries),
    Spectacles(droit::fiche::Spectacles),
}

impl FichierDeFiche {
    /// Le fichier de la fiche `nom` (`entreprise`, `salarie`, `spectacle`) de la compagnie `racine`.
    pub(crate) fn lire(racine: &Path, nom: &str) -> R<FichierDeFiche> {
        let Some(fichier) = fichier_de(nom) else {
            return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
        };
        let chemin = racine.join(socle::fichiers::CONFIG).join(fichier);
        let quoi = format!("config/{}", fichier);
        fn lu<T: serde::de::DeserializeOwned + Default>(chemin: &Path, quoi: &str) -> R<T> {
            if !chemin.exists() {
                return Ok(T::default());
            }
            droit::fiche::lire_fichier(chemin, quoi).map_err(plain)
        }
        Ok(match nom {
            "entreprise" => FichierDeFiche::Entreprise(lu(&chemin, &quoi)?),
            "salarie" => FichierDeFiche::Salaries(lu(&chemin, &quoi)?),
            _ => FichierDeFiche::Spectacles(lu(&chemin, &quoi)?),
        })
    }

    /// La fiche `cle` (`None` : l'entreprise, le fichier entier), si elle existe.
    pub(crate) fn fiche(&mut self, cle: Option<&str>) -> Option<&mut dyn FicheAEcrire> {
        match (self, cle) {
            (FichierDeFiche::Entreprise(e), _) => Some(e),
            (FichierDeFiche::Salaries(s), Some(k)) => s.fiches.get_mut(k).map(|f| f as &mut dyn FicheAEcrire),
            (FichierDeFiche::Spectacles(s), Some(k)) => s.fiches.get_mut(k).map(|f| f as &mut dyn FicheAEcrire),
            _ => None,
        }
    }

    /// La fiche `cle`, neuve si le fichier ne la porte pas encore.
    pub(crate) fn fiche_ou_neuve(&mut self, cle: &str) -> R<&mut dyn FicheAEcrire> {
        Ok(match self {
            FichierDeFiche::Entreprise(e) => e,
            FichierDeFiche::Salaries(s) => s.fiches.entry(droit::contexte::IdSalarie::lire(cle).map_err(plain)?).or_default(),
            FichierDeFiche::Spectacles(s) => s.fiches.entry(droit::contexte::IdSpectacle::lire(cle).map_err(plain)?).or_default(),
        })
    }

    /// Le fichier porte-t-il la fiche `cle` ?
    pub(crate) fn contient(&self, cle: &str) -> bool {
        match self {
            FichierDeFiche::Entreprise(_) => false,
            FichierDeFiche::Salaries(s) => s.fiches.contains_key(cle),
            FichierDeFiche::Spectacles(s) => s.fiches.contains_key(cle),
        }
    }

    /// Le fichier en JSON, tel qu'il s'ecrit.
    pub(crate) fn en_valeur(&self) -> Value {
        match self {
            FichierDeFiche::Entreprise(e) => socle::ecriture::en_valeur(e),
            FichierDeFiche::Salaries(s) => socle::ecriture::en_valeur(s),
            FichierDeFiche::Spectacles(s) => socle::ecriture::en_valeur(s),
        }
    }
}

/// Seul `actif` change, a partir d'aujourd'hui (archiver, c'est ne plus proposer a partir de ce jour ; ce qui precede
/// reste ce qu'il etait), par l'ecrivain de la fiche (`poser_champs`).
pub(crate) fn activer_fiche(ctx: &Contexte, nom: &str, cle: &str, actif: bool) -> R<()> {
    if !matches!(nom, "salarie" | "spectacle") {
        return Err(refus(format!("Seuls un salarie ou un spectacle s'activent : {}.", socle::fichiers::repr(nom))));
    }
    poser_champs(ctx, nom, Some(cle), un_champ("actif", Value::Bool(actif)), Some(ctx.aujourdhui), None)
}

fn activer_fiche_route(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let (nom, cle) = (texte_requis(charge, "nom")?, texte_requis(charge, "cle")?);
    activer_fiche(ctx, nom, cle, charge.get("actif").is_some_and(vrai))?;
    let actif = charge.get("actif").is_some_and(vrai);
    // Le nom que la liste montre : le salarie par son nom, le spectacle par son titre ; la cle seule si la fiche n'en a pas.
    let qui = if nom == "salarie" {
        super::salarie(ctx, &json!(cle)).map(|s| super::nom_a_l_ecran(&s)).ok()
    } else {
        let fiche = ctx.spectacles.get(cle).ok_or_else(|| refus(format!("Spectacle inconnu : {}.", socle::fichiers::repr(cle))))?;
        fiche.titre.portee().filter(|t| !t.trim().is_empty()).map(String::from)
    }
    .unwrap_or_else(|| cle.to_string());
    let p = match (nom, actif) {
        ("salarie", true) => format!("{} : fiche réactivée.", qui),
        ("salarie", false) => format!("{} : fiche archivée.", qui),
        (_, true) => format!("Spectacle « {} » réactivé.", qui),
        (_, false) => format!("Spectacle « {} » archivé.", qui),
    };
    Ok(json!({"phrase": p, "etat": etat_relu(o, ctx)?}))
}

// ----------------------------------------------------- fiche-oublier-demande

/// `fiche-oublier-demande` : `{"adresse"}`, la personne a qui la fiche a ete demandee ; ses demandes en attente sont
/// abandonnees.
fn oublier_demande(_o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let adresse = strip(&charge.get("adresse").filter(|c| vrai(c)).map(valeur_en_texte).unwrap_or_default()).to_lowercase();
    let codes: Vec<String> = fiches_attendues(ctx)?.into_iter().filter(|a| a.adresse == adresse).map(|a| a.code).collect();
    if codes.is_empty() {
        return Err(refus(format!("Aucune demande de fiche en attente pour {}.", adresse)));
    }
    // CE QUI REVIENT SE RANGE SUR LA LIGNE DE LA DEMANDE : ici, qu'on y renonce.
    for code in &codes {
        let retour = journal::RetourFiche {
            code: code.clone(),
            abandon: true,
            salarie: None,
            illisible: None,
            erreurs: None,
            quand: maintenant(),
            qui: "interface".into(),
            de: None,
            message: None,
            reponse: "demande abandonnee, sans reponse".into(),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, code), journal::Revenu::Fiche(retour)).map_err(plain)?;
    }
    // Ni NIR ni IBAN dans la reponse : la page relit `configuration` (comme apres `fiche-demander`).
    Ok(json!({"adresse": adresse}))
}

// ------------------------------------------------------------- enregistrer-fiche

/// Le prochain matricule (S21.G00.30.019, que l'employeur choisit : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`) : le
/// plus grand code salarie que le fichier ait jamais porte, plus un. Le fichier est date : chaque fiche porte la suite
/// de ses valeurs (`[{"valeur"}, {"depuis", "valeur"}]`), et chacune compte.
fn matricule_suivant(salaries: &droit::fiche::Salaries) -> i64 {
    let codes = salaries.fiches.values().flat_map(|s| &s.code_salarie).filter_map(|e| entier(&Value::String(e.valeur.texte().trim().to_string())).ok());
    codes.into_iter().max().map_or(1, |m| m + 1)
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier.
fn cle_neuve(nom: &str, donnees: &Map<String, Value>, existants: &FichierDeFiche) -> R<String> {
    let propre = |texte: &str, bas: bool| -> String {
        let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
        let t: String = t.chars().map(|c| if envois::etats::lettre_ou_chiffre(c) { c } else { '-' }).collect();
        let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
        if bas {
            t.to_lowercase()
        } else {
            t.to_uppercase()
        }
    };
    let texte = |k: &str| donnees.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let ou = |a: &str, b: &str| {
        let x = texte(a);
        if x.is_empty() {
            texte(b)
        } else {
            x
        }
    };
    let candidats: Vec<String> = if nom == "salarie" {
        let base = propre(&ou("nom", "nom_affichage"), false);
        let prenom = propre(&ou("prenom", "prenom_affichage"), false);
        vec![base.clone(), format!("{}-{}", base, prenom)]
    } else {
        let titre = texte("titre");
        let titre = titre.trim();
        let base = if (titre.is_empty() || titre.to_uppercase() == "A COMPLETER") && donnees.get("numero_objet").is_some_and(vrai) {
            format!("objet-{}", valeur_en_texte(&donnees["numero_objet"]).trim())
        } else {
            propre(titre, true)
        };
        vec![base]
    };
    let candidats: Vec<String> = candidats.into_iter().filter(|c| !c.is_empty() && !c.starts_with('-')).collect();
    if candidats.is_empty() {
        return Err(refus(format!(
            "Impossible de nommer cette fiche {} : il lui faut au moins un {}.",
            nom,
            if nom == "salarie" { "nom" } else { "titre" }
        )));
    }
    if let Some(c) = candidats.iter().find(|c| !existants.contient(c)) {
        return Ok(c.clone());
    }
    let dernier = candidats.last().unwrap();
    let mut n = 2;
    while existants.contient(&format!("{}-{}", dernier, n)) {
        n += 1;
    }
    Ok(format!("{}-{}", dernier, n))
}

/// Les cases a cocher arrivees en texte redeviennent booleennes.
fn coercer_booleens(nom: &str, donnees: &mut Map<String, Value>) -> R<()> {
    for c in champs::fiche(nom)? {
        if c.forme != "booleen" {
            continue;
        }
        let Some(v) = donnees.get(&c.cle) else { continue };
        let neuf = match v {
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "true" | "vrai" | "oui" | "1" | "on" | "coche" => Some(Value::Bool(true)),
                "false" | "faux" | "non" | "0" | "off" | "" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Null => None,
            autre => Some(Value::Bool(vrai(autre))),
        };
        if let Some(n) = neuf {
            donnees.insert(c.cle.clone(), n);
        }
    }
    Ok(())
}

// ------------------------------------------------ ecrire une valeur datee

/// Une case vide (`import::completer::vide` : le titre « A COMPLETER » compris) : la remplir n'est pas un changement a
/// dater.
use crate::import::completer::vide;
use crate::routes::catalogue::{Ecran, Place};

/// La meme valeur, a la forme pres : un nombre saisi en texte (100 et « 100 »), une personne et sa forme lisible (les
/// cles `_affichage` que l'ecran comble, `champs::combler_affichage`, ne se declarent pas).
fn meme(a: &Value, b: &Value) -> bool {
    // Une case d'une structure laissee vide (le BIC d'une banque) est une case absente : la fiche les montre pareil.
    let sans_affichage = |v: &Value| match v {
        Value::Object(o) => Value::Object(
            o.iter().filter(|(k, x)| !k.ends_with("_affichage") && !sans_valeur(Some(x))).map(|(k, v)| (k.clone(), v.clone())).collect(),
        ),
        autre => autre.clone(),
    };
    valeurs_egales(a, b)
        || (a.is_object() && valeurs_egales(&sans_affichage(a), &sans_affichage(b)))
        || (!a.is_object() && !a.is_array() && valeur_en_texte(a) == valeur_en_texte(b))
}

/// La structure `apres` garde chaque case de `avant` qui a une valeur (elle n'en remplit que des vides).
fn seulement_completee(avant: &Value, apres: &Value) -> bool {
    match (avant.as_object(), apres.as_object()) {
        (Some(a), Some(b)) => a.iter().filter(|(k, x)| !interne(k) && !sans_valeur(Some(x))).all(|(k, x)| b.get(k).is_some_and(|y| meme(x, y))),
        _ => false,
    }
}

/// La case avait une valeur dans la fiche (`avant`, telle que la fiche la dit) : la changer est un changement, a dater.
/// Une case a cocher que la fiche dit a la sienne, cochee ou non ; une case que la fiche ne dit pas est vide, meme si le
/// moteur lit son defaut en attendant.
fn avait_une_valeur(avant: Option<&Value>) -> bool {
    match avant {
        Some(Value::Bool(_)) => true,
        autre => !sans_valeur(autre),
    }
}

/// Sans valeur : une case vide (`vide`), une case a cocher non cochee, une liste vide, une structure dont aucune case
/// n'a de valeur. La fiche les montre vides ; un enregistrement qui les laisse ainsi ne change rien.
pub(crate) fn sans_valeur(v: Option<&Value>) -> bool {
    vide(v)
        || v.is_some_and(|v| {
            v == &Value::Bool(false)
                || v.as_array().is_some_and(Vec::is_empty)
                || v.as_object().is_some_and(|o| o.iter().all(|(k, x)| interne(k) || sans_valeur(Some(x))))
        })
}

/// LES CHAMPS DE `donnees` QUI CHANGENT LA FICHE `ancienne` (telle que l'ecran l'a montree) : ceux-la demandent leur date
/// d'effet (modele, « Le temps » : toute la configuration est datee, et une valeur sans date rend le passe faux des
/// qu'elle change). Une valeur deja dite qui change, toujours (`avait_une_valeur`) ; une case que la fiche ne dit pas
/// est vide. UNE CASE VIDE QU'ON REMPLIT NE SE DATE PAS : elle comble un manque, sa premiere valeur vaut de
/// tout temps ; ce qui est parti ne se refait pas (regle 4), seul ce qui reste a faire la lit. Sauf `premiere_datee` :
/// une valeur qui, par nature, ne vaut qu'a partir d'un jour (le taux de PAS recu, « le taux personnalise ne s'applique
/// que pour l'avenir », BOI-IR-PAS-30-10-50 § 20, `bofip/BOI-IR-PAS-30-10-50-20190227.html`). Une fiche neuve n'a rien a
/// dater.
/// LES FAITS QUI NE SE DATENT PAS : une case qui dit un fait du passe, vrai de tout temps, et non une valeur qui vaut a
/// partir d'un jour. La date de la premiere embauche de la compagnie (CSS L. 130-1, II, l'effectif d'une entreprise de
/// moins de cinq ans) : la changer la corrige, elle ne change pas a une date. De meme l'anciennete d'un salarie, le jour
/// ou il est entre dans l'entreprise (« Dans la compagnie depuis », d'ou se compte S21.G00.86.003 a la fin de chaque mois
/// declare : `norme-dsn/texte-2027/bloc-S21.G00.86.txt`, l. 91-95). Elles s'ecrivent sans date d'effet.
const FAITS_SANS_DATE: [(&str, &str); 2] = [("entreprise", "premiere_embauche"), ("salarie", "date_anciennete")];

pub(crate) fn changements_a_dater(nom: &str, ancienne: &Value, donnees: &Map<String, Value>, premiere_datee: bool) -> R<Vec<String>> {
    let existait = ancienne.as_object().is_some_and(|o| o.keys().any(|k| !interne(k)));
    if !existait {
        return Ok(Vec::new());
    }
    // Une case que la fiche ne dit pas se montre a ce que le moteur lit alors : une case a cocher a son defaut
    // (`booleen_defaut`), les jours travailles du lundi au vendredi (`droit::conges::JoursTravailles`). La laisser ainsi
    // ne change rien.
    let mut defauts: HashMap<String, Value> =
        champs::fiche(nom)?.into_iter().filter(|c| c.forme == "booleen").map(|c| (c.cle, Value::Bool(c.booleen_defaut))).collect();
    defauts.insert("jours_travailles".into(), json!(droit::conges::JOURS_DE_LA_SEMAINE[..5]));
    let mut cles: Vec<String> = donnees
        .iter()
        .filter(|(k, v)| {
            let avant = ancienne.get(k.as_str()).filter(|a| !vide(Some(a))).or_else(|| defauts.get(k.as_str()));
            !interne(k)
                && !FAITS_SANS_DATE.contains(&(nom, k.as_str()))
                // Les autres etablissements : ligne par ligne (`etablissements::a_dater`).
                && !(nom == "entreprise" && k.as_str() == "etablissements")
                && !avant.is_some_and(|a| meme(a, v))
                && !(sans_valeur(avant) && sans_valeur(Some(v)))
                && (premiere_datee || avait_une_valeur(ancienne.get(k.as_str()).filter(|a| !vide(Some(a)))))
                // Une structure dont on remplit des cases vides sans changer celles qui ont une valeur : c'est leur
                // premiere valeur, comme une case vide.
                && (premiere_datee || !avant.is_some_and(|a| seulement_completee(a, v)))
        })
        .map(|(k, _)| k.clone())
        .collect();
    if nom == "entreprise"
        && donnees.contains_key("etablissements")
        && !crate::routes::etablissements::a_dater(ancienne.get("etablissements"), donnees.get("etablissements")).is_empty()
    {
        cles.push("etablissements".into());
    }
    Ok(cles)
}

/// LE DERNIER MOIS PARTI pour la fiche `nom` (`cle` pour un salarie) : le dernier dont les documents ou la DSN sont
/// partis (`engagement::departs`), ou que l'ancien logiciel a verse et declare (un engagement importe), pour ce salarie,
/// ou pour quiconque quand c'est la fiche de l'entreprise. `None` : aucun ; une fiche de spectacle n'en a pas.
pub(crate) fn dernier_mois_parti(ctx: &Contexte, nom: &str, cle: Option<&str>) -> R<Option<String>> {
    use engagement::departs::Envoi;
    if nom == "spectacle" {
        return Ok(None);
    }
    let sien = |c: &str| nom == "entreprise" || Some(c) == cle;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let partis = departs
        .engagements()
        .filter(|(c, _)| sien(c))
        .flat_map(|(c, n)| departs.de(c, n))
        .filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn))
        .filter_map(|d| d.mois.clone());
    let mut importes = Vec::new();
    for i in engagement::importe::tous(&ctx.racine).map_err(plain)? {
        if i.objet().brut.saisie.carte.salarie.as_deref().is_some_and(sien) {
            importes.extend(i.paies().keys().cloned());
        }
    }
    Ok(partis.chain(importes).max())
}

/// LA DATE QU'UN CHANGEMENT DE FICHE PROPOSE, et pourquoi : le premier jour du mois qui suit le dernier mois parti
/// (`dernier_mois_parti`) : ce qui est parti garde ce qu'il a dit, le reste prend la valeur nouvelle. Aucun mois parti :
/// le premier jour du mois en cours. Une autre date se choisit, dans le passe aussi : ce qui est parti se corrige alors.
pub(crate) fn date_proposee(ctx: &Contexte, nom: &str, cle: Option<&str>) -> R<(Jour, String)> {
    let qui = if nom == "entreprise" { "" } else { " pour ce salarié" };
    Ok(match dernier_mois_parti(ctx, nom, cle)? {
        Some(m) => {
            let m = socle::mois::Mois::lire(&m).map_err(plain)?;
            let phrase = format!(
                "Proposé : le premier mois dont rien n'est parti{}. Jusqu'à {} compris, ce qui est parti garde l'ancienne valeur.",
                qui,
                m.libelle(socle::mois::Graphie::Accentuee)
            );
            (m.suivant().premier_jour(), phrase)
        }
        None => {
            let mois = ctx.aujourdhui.mois_civil();
            let phrase = format!("Proposé : le premier jour de {}, rien n'étant encore parti{}.", mois.libelle(socle::mois::Graphie::Accentuee), qui);
            (mois.premier_jour(), phrase)
        }
    })
}

/// La date d'effet que la charge d'une route d'ecriture donne (`depuis`, AAAA-MM-JJ), ou `None`.
pub(crate) fn depuis_de(depuis: &Value) -> R<Option<Jour>> {
    if vrai(depuis) {
        documents::valeur::d(depuis).map(Some).map_err(|m| erreur("ValueError", m))
    } else {
        Ok(None)
    }
}

/// POSER DES CHAMPS D'UNE FICHE (un geste qui ne touche que quelques cases : le modele de bulletin, la case des
/// permanents, le compte des DPAE, l'anciennete d'un salarie, une fiche archivee, ce que l'import complete) : comme une
/// fiche de l'ecran (`ecrire_datee`), face a la fiche telle qu'elle est sur le disque a `lue_au`, l'ancien fichier
/// sauvegarde. `cle` : celle d'un salarie ou d'un spectacle, `None` pour l'entreprise.
fn poser_champs(ctx: &Contexte, nom: &str, cle: Option<&str>, donnees: Map<String, Value>, depuis: Option<Jour>, note: Option<&str>) -> R<()> {
    let mut contenu = FichierDeFiche::lire(&ctx.racine, nom)?;
    let quoi = format!("config/{}", fichier_de(nom).unwrap_or_default());
    let Some(brute) = contenu.fiche(cle) else {
        return Err(refus(format!("La fiche {} {} n'existe pas : rien n'est écrit.", nom, cle.unwrap_or_default())));
    };
    let ancienne = brute.vue_au(&ctx.lue_au);
    ecrire_datee(brute, nom, &quoi, &ancienne, &donnees, false, depuis, note, &ctx.lue_au)?;
    let chemin = ctx.racine.join(&quoi);
    sauver_avant(&chemin)?;
    ecrire(&chemin, contenu.en_valeur())
}

/// UN TAUX DE PAS RECU S'ECRIT DANS LA FICHE DE SON SALARIE (`droit::pas::CHAMP`), date du jour ou il commence de valoir
/// `depuis` : avant ce jour, la fiche ne le disait pas (la case vide). `note` dit d'ou il vient.
pub(crate) fn poser_taux_recu(ctx: &Contexte, salarie: &str, valeur: Value, depuis: Jour, note: &str) -> R<()> {
    let mut contenu = FichierDeFiche::lire(&ctx.racine, "salarie")?;
    let Some(brute) = contenu.fiche(Some(salarie)) else {
        return Err(refus(format!("La fiche salarie {} n'existe pas : rien n'est écrit.", salarie)));
    };
    let ancienne = brute.vue_au(&depuis);
    ecrire_datee(brute, "salarie", droit::contexte::SALARIES, &ancienne, &un_champ(droit::pas::CHAMP, valeur), true, Some(depuis), Some(note), &depuis)?;
    let chemin = ctx.racine.join(droit::contexte::SALARIES);
    sauver_avant(&chemin)?;
    ecrire(&chemin, contenu.en_valeur())
}

/// Un seul champ, pour `poser_champs`.
fn un_champ(champ: &str, valeur: Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(champ.to_string(), valeur);
    m
}

/// Un champ de la fiche entreprise (`poser_champs`).
pub(crate) fn poser_entreprise(ctx: &Contexte, champ: &str, valeur: Value, depuis: Option<Jour>) -> R<()> {
    poser_champs(ctx, "entreprise", None, un_champ(champ, valeur), depuis, None)
}

/// L'ANCIENNETE D'UN SALARIE PART DE SON EMBAUCHE (S21.G00.86.003, l'anciennete dans l'entreprise :
/// `norme-dsn/texte/bloc-S21.G00.86.txt`) : la validation de son premier engagement la pose sur sa fiche, au premier jour
/// de ce contrat (`debut`), quand la fiche ne la dit pas, par l'ecrivain de la fiche (`poser_champs`) ; elle se change
/// ensuite dans « Avancé » de la fiche. Une fiche qui la dit la garde. INFERENCE : l'anciennete commence a l'entree dans
/// l'entreprise.
pub(crate) fn poser_anciennete_a_l_embauche(ctx: &Contexte, salarie: &str, debut: &str) -> R<()> {
    if ctx.salaries.get(salarie).is_none_or(|f| f.date_anciennete.est_dite()) {
        return Ok(());
    }
    poser_champs(ctx, "salarie", Some(salarie), un_champ("date_anciennete", json!(debut)), None, None)
}

/// L'ANCIENNETE D'UN SALARIE REPRIS QUE SON CONTRAT FAIT REMONTER : l'import la pose au premier bulletin lu
/// (`import::bulletins`) ; un contrat repris qui a commence avant (`debut`, celui que la DSN de l'ancien logiciel
/// declare, S21.G00.40.001) dit qu'il etait dans l'entreprise des ce jour-la (S21.G00.86.003, l'anciennete dans
/// l'entreprise : `norme-dsn/texte/bloc-S21.G00.86.txt` ; inference : un contrat en cours dans l'entreprise y compte).
/// Seule la valeur que l'import a posee, `pose_le`, se remplace : l'anciennete est un fait du passe (`FAITS_SANS_DATE`),
/// elle se corrige. Une date saisie depuis reste la sienne. Rend vrai si elle a ete remplacee.
pub(crate) fn reprendre_anciennete(ctx: &Contexte, salarie: &str, pose_le: &str, debut: &str) -> R<bool> {
    let FichierDeFiche::Salaries(contenu) = FichierDeFiche::lire(&ctx.racine, "salarie")? else { return Ok(false) };
    let Some([seule]) = contenu.fiches.get(salarie).map(|f| f.date_anciennete.as_slice()) else { return Ok(false) };
    if seule.valeur.dite().map(Jour::iso).as_deref() != Some(pose_le) || debut >= pose_le {
        return Ok(false);
    }
    // La note de l'import (d'ou vient la valeur) reste : la valeur vient toujours des pieces importees.
    let note = seule.note.clone();
    poser_champs(ctx, "salarie", Some(salarie), un_champ("date_anciennete", json!(debut)), None, note.as_deref())?;
    Ok(true)
}

/// ECRIRE UNE FICHE DATEE : chaque valeur de `donnees` qui differe de `ancienne` (la fiche lue a `lue_au`, celle que
/// l'ecran a montree) s'ecrit dans la fiche datee `brute` (`FicheAEcrire::ecrire_case`, par sa forme : `droit::fiche::dater`). Ce qui change une valeur existante vaut a
/// partir de `depuis`, et sans lui se refuse : ce qui precede garde l'ancienne valeur (`changements_a_dater` dit ce qui
/// change). Une case vide qu'on remplit prend sa valeur de tout temps, `depuis` ou non. Une valeur `premiere_datee`
/// (le taux de PAS recu) vaut a partir de `depuis` meme dans une case vide : avant lui, la fiche ne disait rien. Ce qui
/// ne change rien s'ecrit tel quel. Les notes `_...` s'ecrivent telles quelles.
pub(crate) fn ecrire_datee(
    brute: &mut dyn FicheAEcrire,
    nom: &str,
    fichier: &str,
    ancienne: &Value,
    donnees: &Map<String, Value>,
    premiere_datee: bool,
    depuis: Option<Jour>,
    note: Option<&str>,
    lue_au: &Jour,
) -> R<()> {
    let a_dater = changements_a_dater(nom, ancienne, donnees, premiere_datee)?;
    if depuis.is_none() && !a_dater.is_empty() {
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        let fiche = champs::fiche(nom)?;
        let derive_d_un_change = |k: &str| {
            fiche.iter().find(|c| c.cle == k).and_then(|c| c.derive_de.as_ref()).is_some_and(|s| a_dater.contains(s))
        };
        let libelles: HashMap<&str, &str> = fiche.iter().map(|c| (c.cle.as_str(), c.libelle.as_str())).collect();
        let noms: Vec<String> = a_dater
            .iter()
            .filter(|k| !derive_d_un_change(k))
            .map(|k| format!("« {} »", libelles.get(k.as_str()).copied().unwrap_or(k)))
            .collect();
        return Err(refus(format!(
            "Changer {} demande sa date d'effet (« a partir du ») : ce qui precede garde l'ancienne valeur, et la date ne \
             se devine pas. Rien n'a ete enregistre.",
            noms.join(", ")
        )));
    }
    for (k, v) in donnees {
        if interne(k) {
            brute.ecrire_case(k, v, None, None, fichier)?;
            continue;
        }
        let avant = ancienne.get(k.as_str());
        // « A COMPLETER » est une case vide a remplir, pas rien : il s'ecrit (une fiche importee le porte, et la fiche
        // d'un spectacle sans titre ne se lit pas).
        // Une structure dont aucune case n'est remplie (la banque, les congés à la reprise) ne dit rien non plus.
        let rien = v.is_null() || v.as_str() == Some("") || (v.is_object() && sans_valeur(Some(v)));
        if (vide(avant) && rien) || avant.is_some_and(|a| meme(a, v)) {
            continue;
        }
        let jours = brute.jours_de(k);
        let remplit_un_absent = depuis.is_some() && a_dater.contains(k) && jours.is_empty();
        let jour = if depuis.is_some() && a_dater.contains(k) {
            depuis
        } else {
            jours.into_iter().filter(|j| j.map_or(true, |j| j <= *lue_au)).last().flatten()
        };
        if remplit_un_absent {
            // Avant `depuis`, la fiche ne disait rien : la case, vide.
            brute.ecrire_case(k, &Value::Null, None, None, fichier)?;
        }
        brute.ecrire_case(k, v, jour.as_ref(), note, fichier)?;
    }
    Ok(())
}

/// CE QUE L'ENREGISTREMENT D'UNE FICHE PASSE OUTRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forcer {
    /// Rien : une erreur refuse la fiche.
    Rien,
    /// Ce que l'ecran peut forcer (« Enregistrer quand meme ») : une erreur de forme (`forcable`), jamais une case
    /// qu'une structure ne decrit pas (`champs::cases_inconnues`).
    Forme,
    /// Tout : l'import et le questionnaire du salarie posent une fiche a completer, qui le dit (« A COMPLETER »).
    Tout,
}

impl Forcer {
    /// La charge `forcer` de la route `enregistrer-fiche` : l'ecran ne force que la forme.
    pub(crate) fn depuis_l_ecran(forcer: bool) -> Forcer {
        if forcer {
            Forcer::Forme
        } else {
            Forcer::Rien
        }
    }

    /// Ces anomalies refusent-elles la fiche ?
    fn refuse(self, anomalies: &[Value]) -> bool {
        let erreurs = || anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur"));
        match self {
            Forcer::Rien => erreurs().next().is_some(),
            Forcer::Forme => erreurs().any(|a| a.get("forcable") == Some(&Value::Bool(false))),
            Forcer::Tout => false,
        }
    }
}

/// Ce que `echoue` dit : la fiche porte au moins une erreur, elle n'a pas ete ecrite.
fn fiche_refusee(nom: &str, anomalies: &[Value]) -> Echec {
    let phrases: Vec<String> = anomalies
        .iter()
        .filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur"))
        .map(|a| {
            let cle = a.get("champ").map_or_else(String::new, valeur_en_texte);
            let libelle = champs::champ(nom, &cle).ok().flatten().map(|c| c.libelle.to_string()).filter(|l| !l.is_empty());
            let message = a.get("message").map_or_else(String::new, valeur_en_texte);
            libelle.map_or(message.clone(), |l| format!("« {} » : {}", l, message))
        })
        .collect();
    let message = format!("La fiche n'est pas écrite : {}", phrases.join(" ; "));
    let erreurs_dit = if phrases.len() > 1 { format!("{} erreurs", phrases.len()) } else { "Une erreur".to_string() };
    Echec { classe: Some("FicheRefusee"), message: socle::echec::avec(message, json!({"anomalies": anomalies, "erreurs_dit": erreurs_dit})) }
}

/// Ce que l'ecriture d'une fiche a fait : sa cle (celle d'une fiche neuve est fabriquee ici), les anomalies, et si le
/// fichier a change (une fiche enregistree telle qu'elle est n'ecrit rien), et les champs qui ont change.
pub(crate) struct FicheEcrite {
    pub(crate) cle: Option<String>,
    pub(crate) anomalies: Vec<Value>,
    pub(crate) ecrit: bool,
    pub(crate) champs: Vec<String>,
}

/// `enregistrer-fiche` : une fiche de l'ecran, ecrite (`ecrire_fiche`). Une valeur existante qui change demande sa date
/// d'effet : sans `confirme`, la route rend l'avertissement (`avertissement_de_fiche`) et n'ecrit rien ; la meme charge
/// avec `confirme` ecrit, a partir de `depuis` (la date proposee, s'il manque).
fn enregistrer_fiche(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = texte_requis(charge, "nom")?;
    let cle: Option<String> = match charge.get("cle") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => return Err(erreur("TypeError", socle::echec::charge("cle", "un texte"))),
    };
    let donnees = &cases_changees(nom, charge)?;
    let forcer = charge.get("forcer").is_some_and(vrai);
    let mut depuis = charge.get("depuis").cloned().unwrap_or(Value::Null);
    let note = charge.get("note").unwrap_or(&Value::Null);
    if let Some(a) = avertissement_de_fiche(ctx, nom, cle.as_deref().filter(|k| !k.is_empty()), donnees)? {
        if !charge.get("confirme").is_some_and(vrai) {
            let phrase = if a.get("depuis").is_some() { "Ce qui change se date : rien n'est écrit." } else { "À confirmer : rien n'est écrit." };
            return Ok(json!({"avertissement": a, "phrase": phrase, "phrases": avertissement_en_phrases(&a)}));
        }
        if !vrai(&depuis) {
            depuis = a["depuis"].clone();
        }
    }
    let neuve = nom != "entreprise" && cle.as_deref().map_or(true, str::is_empty);
    let e = ecrire_fiche(ctx, nom, cle, donnees, Forcer::depuis_l_ecran(forcer), &depuis, note)?;
    let en_erreur = e.anomalies.iter().any(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur"));
    // Une fiche creee se nomme ensuite par sa cle (la saisie d'une carte, `cle` d'une autre ecriture) : la reponse la rend.
    let phrase = match (e.ecrit, neuve, forcer && en_erreur, &e.cle) {
        (false, _, _, _) => "Rien n'a changé : rien n'est écrit.".to_string(),
        (true, _, true, _) => "Enregistré malgré les erreurs.".to_string(),
        (true, true, false, _) => "Fiche créée.".to_string(),
        (true, false, false, _) => "Enregistré.".to_string(),
    };
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let enregistre_a = e.ecrit.then(maintenant);
    let enregistre_dit = enregistre_a.as_ref().map(|a| format!("enregistré à {}", a.chars().skip(11).take(5).collect::<String>()));
    Ok(json!({"cle": e.cle, "ecrits": e.champs, "enregistre_a": enregistre_a, "enregistre_dit": enregistre_dit, "phrase": phrase}))
}


/// CE QUE LA CHARGE CHANGE, case par case : ses `changements` (`droit::fiche::ChangementEntreprise`,
/// `ChangementSalarie`, `ChangementSpectacle`, lus a la porte : une case inconnue ou une valeur d'une autre forme se
/// refuse, rien n'est ecrit), chacun posant sa case, `null` la vidant ; une case qu'ils ne nomment pas ne change pas.
/// Une structure (le signataire, la banque…) se donne entiere : elle remplace celle de la fiche.
fn cases_changees(nom: &str, charge: &Value) -> R<Value> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Lus<C> {
        #[allow(dead_code)] // lus pour refuser a la porte ce qui ne s'ecrirait pas
        changements: droit::fiche::Changements<C>,
    }
    let liste = exiger(charge, "changements")?;
    let lus = json!({"changements": liste});
    match nom {
        "entreprise" => lire_charge::<Lus<droit::fiche::ChangementEntreprise>>(&lus).map(drop)?,
        "salarie" => lire_charge::<Lus<droit::fiche::ChangementSalarie>>(&lus).map(drop)?,
        "spectacle" => lire_charge::<Lus<droit::fiche::ChangementSpectacle>>(&lus).map(drop)?,
        autre => return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
    let mut cases = Map::new();
    for ch in liste.as_array().into_iter().flatten() {
        cases.insert(valeur_en_texte(&ch["case"]), ch["valeur"].clone());
    }
    Ok(Value::Object(cases))
}

/// Verifie, date ce qui change, sauvegarde l'ancien fichier et
/// ecrit. Rend ce qui a ete ecrit, sans la page : l'import s'en sert fiche par fiche.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, cle: Option<String>, donnees: &Value, forcer: Forcer, depuis: &Value, note: &Value) -> R<FicheEcrite> {
    let mut cle = cle;
    let Some(fichier) = fichier_de(nom) else {
        return Err(refus(format!("Fiche inconnue : {}", socle::fichiers::repr(nom))));
    };
    if !donnees.is_object() {
        return Err(erreur("TypeError", socle::echec::charge("donnees", "un objet")));
    }
    let vide = Value::Object(Map::new());
    let montree = crate::routes::etablissements::entreprise_montree(ctx);
    // L'ecriture d'une fiche compare ses cases par leur nom (`champs-base.json`) : la fiche lue, en JSON.
    let lue = match nom {
        "salarie" => cle.as_deref().and_then(|k| ctx.salaries.get(k)).map(|f| socle::ecriture::en_valeur(&f.champs)),
        _ => cle.as_deref().and_then(|k| ctx.spectacles.get(k)).map(socle::ecriture::en_valeur),
    };
    let ancienne: &Value = if nom == "entreprise" {
        &montree
    } else {
        lue.as_ref().unwrap_or(&vide)
    };
    let Value::Object(mut donnees) = donnees.clone() else { unreachable!() };
    coercer_booleens(nom, &mut donnees)?;

    // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
    if nom == "salarie" && cle.as_deref().is_some_and(|k| !k.is_empty()) {
        let (avant, apres) = (ancienne.get("code_salarie"), donnees.get("code_salarie"));
        if let (Some(a), Some(b)) = (avant.filter(|v| vrai(v)), apres.filter(|v| vrai(v))) {
            let nir = donnees.get("nir").is_some_and(vrai) || ancienne.get("nir").is_some_and(vrai);
            if valeur_en_texte(a) != valeur_en_texte(b) && !nir {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    valeur_en_texte(a),
                    valeur_en_texte(b)
                )));
            }
        }
    }
    // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let nir = donnees.get("nir").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let nir = nir.trim();
        if !nir.is_empty() {
            let autres: Vec<&str> = ctx
                .salaries
                .iter()
                .filter(|(k, s)| Some(k.as_str()) != cle.as_deref() && s.nir.texte().trim() == nir)
                .map(|(k, _)| k.as_str())
                .collect();
            if !autres.is_empty() {
                return Err(refus(format!(
                    "Le NIR {} est déjà celui de la fiche {}. Un NIR désigne une personne et une seule : complétez ou réactivez cette \
                     fiche-là plutôt que d'en créer une seconde.",
                    nir,
                    autres.join(", ")
                )));
            }
        }
    }
    // UN MATRICULE, UN SALARIE : la DSN rapproche chaque salarie de ses declarations par son matricule (S21.G00.30.019, dont
    // « le choix de la valeur revient a l'employeur », `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 327-329) ; deux fiches au
    // meme matricule s'y confondraient. `forcer` ne leve pas ce refus.
    if nom == "salarie" {
        let matricule = donnees.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let matricule = matricule.trim();
        if !matricule.is_empty() {
            let porteur = ctx.salaries.iter().find(|(k, s)| {
                Some(k.as_str()) != cle.as_deref()
                    && s.code_salarie.texte().trim() == matricule
            });
            if let Some((k, s)) = porteur {
                let prenom = if s.prenom_affichage.texte().is_empty() { s.prenom.texte() } else { s.prenom_affichage.texte() };
                let qui = format!("{} {}", s.nom.texte(), prenom).trim().to_string();
                return Err(refus(format!(
                    "Le matricule {} est déjà celui de {} (fiche {}). Un matricule désigne un salarié et un seul : la DSN \
rapproche chacun de ses déclarations par lui. En choisir un autre, ou laisser la case vide pour prendre le suivant.",
                    matricule,
                    if qui.is_empty() { k.as_str() } else { qui.as_str() },
                    k
                )));
            }
        }
    }
    // UN NUMERO D'OBJET, UN SPECTACLE : France Travail l'attribue « pour chaque spectacle ou chaque production » et il
    // « permet d'identifier toutes les personnes ayant travaille » dessus (S21.G00.40.050, `norme-dsn/texte/bloc-S21.G00.40.txt`,
    // l. 1379-1385) ; deux fiches au meme numero se confondraient dans la DSN. `forcer` ne leve pas ce refus.
    if nom == "spectacle" {
        let numero = donnees.get("numero_objet").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
        let numero = numero.trim();
        if !numero.is_empty() {
            let porteur = ctx.spectacles.iter().find(|(k, s)| {
                Some(k.as_str()) != cle.as_deref()
                    && s.numero_objet.texte().trim().eq_ignore_ascii_case(numero)
            });
            if let Some((k, s)) = porteur {
                let qui = match s.titre.remplie().filter(|t| !t.trim().is_empty() && !t.trim().eq_ignore_ascii_case(crate::import::completer::A_COMPLETER)) {
                    Some(t) => format!("du spectacle « {} » (fiche {})", t, k),
                    None => format!("de la fiche {}", k),
                };
                return Err(refus(format!(
                    "Le numéro d'objet {} est déjà celui {}. Un numéro d'objet désigne un spectacle et un seul : complétez \
cette fiche-là, ou corrigez le numéro.",
                    numero, qui
                )));
            }
        }
    }
    let chemin = ctx.racine.join(socle::fichiers::CONFIG).join(fichier);
    if cle.as_deref().map_or(true, str::is_empty) && nom == "salarie" {
        // Le matricule d'une fiche neuve : le suivant. Un matricule saisi est garde tel quel.
        if !donnees.get("code_salarie").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default().trim().is_empty() {
        } else {
            let FichierDeFiche::Salaries(existants) = FichierDeFiche::lire(&ctx.racine, nom)? else { unreachable!() };
            donnees.insert("code_salarie".into(), Value::String(matricule_suivant(&existants).to_string()));
        }
        // L'ANCIENNETE PART DE L'EMBAUCHE, pas du jour de la fiche : la validation du premier engagement la pose
        // (`poser_anciennete_a_l_embauche`) ; une date saisie est gardee telle quelle.
    }
    crate::routes::fiches::deriver(nom, &mut donnees, true)?;

    // UNE CHARGE PARTIELLE ne change que les champs qu'elle nomme : les autres gardent leur valeur (`ecrire_datee` n'ecrit
    // que ceux-la), et la fiche se verifie entiere, telle qu'elle sera.
    let mut entiere = donnees.clone();
    if let Value::Object(a) = ancienne {
        for (k, v) in a {
            entiere.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    let mut anomalies = crate::routes::fiches::verifier(nom, &entiere)?;
    anomalies.extend(champs::cases_inconnues(nom, &donnees, !matches!(forcer, Forcer::Tout))?);
    if forcer.refuse(&anomalies) {
        return Err(fiche_refusee(nom, &anomalies));
    }

    let (depuis, note) = (depuis_de(depuis)?, Some(valeur_en_texte(note)).filter(|n| vrai(note) && !n.trim().is_empty()));
    let quoi = format!("config/{}", fichier);
    let mut contenu = FichierDeFiche::lire(&ctx.racine, nom)?;
    let avant = contenu.en_valeur();
    if let FichierDeFiche::Entreprise(e) = &mut contenu {
        // La fiche entreprise est le fichier entier. Les autres etablissements se datent chacun a son ouverture.
        let mut sans_etablissements = donnees.clone();
        let etablissements = sans_etablissements.shift_remove("etablissements");
        ecrire_datee(e, nom, &quoi, ancienne, &sans_etablissements, false, depuis, note.as_deref(), &ctx.lue_au)?;
        if let Some(l) = etablissements {
            crate::routes::etablissements::ecrire(ctx, e, &donnees, &l, depuis, note.as_deref())?;
        }
        return finir_fiche(None, &chemin, avant, contenu, anomalies);
    }
    if cle.as_deref().map_or(true, str::is_empty) {
        // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
        cle = Some(cle_neuve(nom, &donnees, &contenu)?);
    }
    let k = cle.clone().unwrap();
    ecrire_datee(contenu.fiche_ou_neuve(&k)?, nom, &quoi, ancienne, &donnees, false, depuis, note.as_deref(), &ctx.lue_au)?;
    finir_fiche(cle, &chemin, avant, contenu, anomalies)
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (`import::completer`) : chaque champ de `champs` va dans la fiche
/// telle qu'elle est sur le disque, s'il y est vide ou « A COMPLETER » -- et seulement alors : un champ rempli depuis
/// l'apercu reste ce qu'il est. Une case vide n'avait rien declare : la valeur qui la remplit vaut depuis elle (un champ
/// absent : de tout temps), par l'ecrivain de la fiche (`poser_champs`). Rend les champs ecrits.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    let mut contenu = FichierDeFiche::lire(&ctx.racine, nom)?;
    let Some(brute) = contenu.fiche(cle) else {
        return Err(refus(format!("La fiche {} {} n'existe plus : rien n'est complete.", nom, cle.unwrap_or_default())));
    };
    let au = brute.vue_au(&ctx.lue_au);
    let ecrits: Map<String, Value> = champs.iter().filter(|(k, _)| vide(au.get(k.as_str()))).map(|(k, v)| (k.clone(), v.clone())).collect();
    if !ecrits.is_empty() {
        poser_champs(ctx, nom, cle, ecrits.clone(), None, None)?;
    }
    Ok(ecrits)
}

/// LES LICENCES QUI SUIVENT LA PREMIERE (`routes::import::poser_licence`) : chacune vaut a partir du premier jour du mois
/// ou les pieces importees la portent, comme un numero renouvele saisi « a partir du » (`poser_champs`). Seulement quand
/// la fiche, sur le disque, porte pour seule licence la premiere que les pieces donnent : une licence saisie autrement
/// reste ce qu'elle est.
pub(crate) fn dater_licences_suivantes(ctx: &Contexte, licences: &[(String, String)], note: &Value) -> R<()> {
    let Some(((_, premiere), suivantes)) = licences.split_first().filter(|(_, s)| !s.is_empty()) else { return Ok(()) };
    let FichierDeFiche::Entreprise(fiche) = FichierDeFiche::lire(&ctx.racine, "entreprise")? else { return Ok(()) };
    if !matches!(fiche.licence_spectacle.as_slice(), [seule] if seule.depuis.is_none() && seule.valeur.dite() == Some(premiere)) {
        return Ok(());
    }
    let note = Some(valeur_en_texte(note)).filter(|n| !n.trim().is_empty());
    for (mois, numero) in suivantes {
        let jour = socle::mois::Mois::lire(mois).map(socle::mois::Mois::premier_jour).map_err(plain)?;
        poser_champs(ctx, "entreprise", None, un_champ("licence_spectacle", json!(numero)), Some(jour), note.as_deref())?;
    }
    Ok(())
}

fn finir_fiche(cle: Option<String>, chemin: &Path, avant: Value, contenu: FichierDeFiche, anomalies: Vec<Value>) -> R<FicheEcrite> {
    let apres = contenu.en_valeur();
    let ecrit = avant != apres;
    let (a, n) = match cle.as_deref() {
        Some(k) => (avant.get(k).unwrap_or(&Value::Null), apres.get(k).unwrap_or(&Value::Null)),
        None => (&avant, &apres),
    };
    let champs: Vec<String> = n.as_object().into_iter().flatten().filter(|(k, v)| !interne(k) && a.get(k.as_str()) != Some(*v)).map(|(k, _)| k.clone()).collect();
    if ecrit {
        sauver_avant(chemin)?;
        ecrire(chemin, apres)?;
    }
    Ok(FicheEcrite { cle, anomalies, ecrit, champs })
}


// ------------------------------------------------------------------ la fiche demandee par courriel

/// Secondes entre une demande de fiche et la reponse de la personne fictive de la compagnie fictive (`DELAI_SIGNATURE`).
pub const DELAI_FICTIVE: i64 = 20;

/// Une adresse courriel de forme `x@y.z`, sans blanc ni second `@`.
fn adresse_valide(adresse: &str) -> bool {
    let blanc = socle::texte::blanc;
    if adresse.chars().any(blanc) || adresse.matches('@').count() != 1 {
        return false;
    }
    let (local, domaine) = adresse.split_once('@').unwrap_or(("", ""));
    let d: Vec<char> = domaine.chars().collect();
    !local.is_empty() && d.len() >= 3 && (1..d.len() - 1).any(|i| d[i] == '.')
}

/// Ce que `demander_fiche` rend : le code, l'adresse (en minuscules), la ligne du journal.
pub struct FicheDemandee {
    pub code: String,
    pub adresse: String,
    pub ligne: envois::journal::Ligne,
}

/// Envoie a `adresse` le courriel « Remplir ma fiche ». Le journal garde
/// le code : c'est par lui que la releve reconnaitra la reponse. `client` : la boite, ouverte ici (boite simulee dans une

/// La charge de `fiche-demander` : l'adresse, le prenom (pour lui ecrire), et le mot de passe du logiciel tape pour l'envoi.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFicheDemander {
    adresse: String,
    prenom: String,
    #[serde(default)]
    motdepasse: Option<String>,
}

/// Ce que rend `fiche-demander`.
#[derive(Debug, serde::Serialize)]
struct DemandeDeFiche {
    phrase: String,
    code: String,
    adresse: String,
}

/// `fiche-demander` : le courriel « Remplir ma fiche » part (`demander_fiche`), sous la preuve que la charge porte.
fn fiche_demander(ctx: &Contexte, c: ChargeFicheDemander) -> R<DemandeDeFiche> {
    let preuve = Preuve::du_mot_de_passe(c.motdepasse);
    let d = demander_fiche(ctx, &c.adresse, &c.prenom, &preuve, INTERFACE, None)?;
    Ok(DemandeDeFiche { phrase: format!("Demande de fiche envoyée à {} (code {}) : sa fiche se crée à l'arrivée de sa réponse.", d.adresse, d.code), code: d.code, adresse: d.adresse })
}
/// compagnie fictive, sinon refus « boite absente ») quand l'appelant n'en donne pas.
pub fn demander_fiche(ctx: &Contexte, adresse: &str, prenom: &str, preuve: &Preuve, qui: &str, client: Option<&mut dyn Courriel>) -> R<FicheDemandee> {
    let adresse = strip(adresse).to_lowercase();
    if !adresse_valide(&adresse) {
        return Err(refus(format!("« {} » n'est pas une adresse de courriel.", adresse)));
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => ouverte.insert(client_courriel(ctx)?).as_mut(),
    };
    verifier_preuve(ctx, Some(preuve), &socle::paris::maintenant()[..7], "demande de fiche (« Remplir ma fiche »)", &adresse)?;

    let compagnie = ctx.entreprise.nommee();
    let sg = ctx.entreprise.signataire.dite().cloned().unwrap_or_default();
    let signataire = match sg.prenom_affichage.remplie() {
        Some(p) => p.to_string(),
        None => capitaliser(sg.prenom.texte()),
    };
    let expediteur = client.compte().to_lowercase();
    let code = fq::code_fiche();
    let q = fq::questionnaire(&code, &fq::posees(&|c| champs::utile("salarie", c)), &|_| String::new());
    let sujet = envoi::formater(fq::SUJET, &[("compagnie", compagnie.clone())]);
    let prenom = strip(prenom).to_string();
    let virgule_prenom = if prenom.is_empty() { String::new() } else { format!(" {}", prenom) };
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![("virgule_prenom", e(&virgule_prenom)), ("compagnie", e(&compagnie)), ("signataire", e(&signataire))]
    };
    let lien = mailto(&expediteur, &format!("Re: {}", sujet), &q);
    let mut texte = champs(false);
    texte.push(("questionnaire", q.clone()));
    let mut html = champs(true);
    html.push(("questionnaire", html_echapper(&q)));
    html.push(("mailto", html_echapper(&lien)));
    let message = Message {
        cle: format!("fiche-{}", code),
        nom: if prenom.is_empty() { adresse.clone() } else { prenom.clone() },
        destinataire: adresse.clone(),
        sujet,
        corps: envoi::formater(fq::CORPS, &texte),
        html: envoi::formater(fq::HTML, &html),
        pieces: Vec::new(),
    };
    let mut identifiants: Vec<String> = Vec::new();
    let demande = |message: Option<String>, expediteur: Option<String>| journal::DetailFiche { code: code.clone(), adresse: adresse.clone(), prenom: prenom.clone(), message, expediteur };
    if let Err(e) = envoi::envoyer(&[message], &expediteur, client, Some(&mut identifiants), None) {
        noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, false, &adresse, &e, demande(None, None), None)))?;
        return Err(echec_d_envoi(e));
    }
    let detail = demande(Some(identifiants.first().cloned().unwrap_or_default()), Some(expediteur.clone()));
    let ligne = noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, true, &adresse, &format!("fiche demandee a {}", adresse), detail, None)))?;
    Ok(FicheDemandee { code, adresse, ligne })
}

// ------------------------------------------------------------------ les demandes attendues

/// Une demande partie et pas encore revenue.
#[derive(Debug, Clone)]
pub struct Attendue {
    pub code: String,
    pub adresse: String,
    pub prenom: String,
    /// L'heure de la demande (ISO), celle de la ligne du journal.
    pub quand: String,
    /// Les identifiants de NOS envois (le `Message-ID` gardé avec la demande) : jamais pris pour une reponse.
    pub envoyes: Vec<String>,
}

/// La ligne `l` est-elle la demande de fiche partie sous le code `code` ? Ce qui en revient se range sur elle.
fn demande_de(l: &journal::Ligne, code: &str) -> bool {
    l.fiche().is_some_and(|e| e.accepte && e.detail.code == code)
}

/// `fiches_attendues` : les demandes parties et pas encore revenues, les plus anciennes d'abord. Une demande a
/// laquelle on renonce se retire (`fiche-oublier-demande`, `fiches`).
pub fn fiches_attendues(ctx: &Contexte) -> R<Vec<Attendue>> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let mut sortie = Vec::new();
    for e in lignes.iter().filter_map(journal::Ligne::fiche) {
        if !e.accepte || e.detail.code.is_empty() || !e.retours.is_empty() {
            continue;
        }
        sortie.push(Attendue {
            code: e.detail.code.clone(),
            adresse: e.detail.adresse.clone(),
            prenom: e.detail.prenom.clone(),
            quand: e.quand.clone(),
            envoyes: e.detail.message.clone().filter(|m| !m.is_empty()).into_iter().collect(),
        });
    }
    Ok(sortie)
}

// ------------------------------------------------------------------ la releve

/// Ce que la releve a fait : combien de demandes attendaient, les fiches creees, ce qui l'a empechee.
#[derive(Debug, Default)]
pub struct ReleveFiches {
    pub attendues: usize,
    /// `{code, cle, nom, illisible, erreurs}` par fiche creee ou completee.
    pub recues: Vec<Value>,
    pub incidents: Vec<String>,
}

impl ReleveFiches {
    /// `ReleveFiches.resume` : la phrase de la veille, vide quand rien n'est arrive.
    pub fn resume(&self) -> String {
        if self.recues.is_empty() {
            return String::new();
        }
        let noms: Vec<String> = self.recues.iter().map(|r| r.get("nom").map(valeur_en_texte).unwrap_or_default()).collect();
        format!("{} : {}", super::compte(self.recues.len(), "fiche de salarié reçue", "fiches de salarié reçues"), noms.join(", "))
    }
}

fn os(m: String) -> Echec {
    erreur("OSError", m)
}

/// Cherche les reponses aux demandes de fiche et cree chaque fiche.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- la personne fictive de la compagnie fictive, ou la
/// vraie boite ; sans boite, un incident et rien d'autre. `qui` : `interface`, ou `veille`.
pub fn relever_fiches(ctx: &Contexte, client: Option<&mut dyn Courriel>, qui: &str) -> R<ReleveFiches> {
    let mut r = ReleveFiches::default();
    let attendues = fiches_attendues(ctx)?;
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None if branchement.joue() => {
            // LA COMPAGNIE FICTIVE : la personne fictive repond, et c'est la vraie lecture qui suit.
            ouverte.insert(Box::new(boite_des_fiches(ctx, &attendues, DELAI_FICTIVE)?)).as_mut()
        }
        None => {
            // Le message : rien n'est parti, rien ne revient.
            if let Some(refus) = branchement.refus() {
                r.incidents.push(refus.phrase.to_string());
                return Ok(r);
            }
            if !crate::compagnie::boite::disponible(&ctx.racine) {
                r.incidents.push("Aucune boite courriel : les fiches demandees ne peuvent pas etre relevees.".to_string());
                return Ok(r);
            }
            let vrai_client = crate::compagnie::boite::client_du_contexte(ctx).map_err(|e| erreur("ErreurCourriel", e.0))?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = &a.code;
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e));
                break;
            }
        };
        let mut retenu: Option<(String, Vec<u8>, String, String, String)> = None;
        for ident in identifiants {
            if a.envoyes.contains(&ident) {
                continue;
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e));
                    continue;
                }
            };
            let Ok(m) = mailparse::parse_mail(&brut) else {
                r.incidents.push(format!("Lecture du message {} : message illisible.", ident));
                continue;
            };
            if a.envoyes.contains(&lecture::identifiant_de(&m)) {
                continue;
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.as_str()) || texte.contains("Remplir ma fiche") {
                continue;
            }
            let de = lecture::adresse_de(&m);
            if de == expediteur && de != a.adresse {
                continue;
            }
            if de != a.adresse {
                r.incidents.push(format!("Fiche {} : une reponse vient de {} et non de {} ; elle n'est pas retenue.", code, de, a.adresse));
                continue;
            }
            let quand = lecture::date_de(&m, &socle::paris::maintenant());
            retenu = Some((ident, brut.clone(), texte, de, quand));
            break;
        }
        let Some((ident, brut, texte, de, quand)) = retenu else { continue };
        let (mut donnees, illisible) = fq::fiche_depuis_reponses(&fq::lire_reponses(&texte));
        donnees.insert("email".into(), json!(de));
        if !(donnees.get("nom_affichage").is_some_and(vrai) || donnees.get("prenom_affichage").is_some_and(vrai)) {
            r.incidents.push(format!("Fiche {} : la reponse de {} ne porte ni nom ni prenom lisible ; rien n'est cree.", code, de));
            continue;
        }
        let dossier = ctx.racine.join("sorties").join("fiches-salaries");
        std::fs::create_dir_all(&dossier).map_err(|e| os(socle::echec::du_disque(&dossier, &e)))?;
        let eml = dossier.join(format!("Fiche_{}.eml", code));
        std::fs::write(&eml, &brut).map_err(|e| os(socle::echec::du_disque(&eml, &e)))?;
        let jour: Vec<&str> = quand.get(..10).unwrap_or("").split('-').rev().collect();
        let mut note = format!("Fiche remplie par la personne, par courriel, le {}. Reste à choisir l'emploi.", jour.join("/"));
        if !illisible.is_empty() {
            note.push_str(&format!(" Non lu, à compléter : {}.", illisible.join(" ; ")));
        }
        let existante: Option<String> = ctx
            .salaries
            .iter()
            .find(|(_, s)| strip(s.email.texte()).to_lowercase() == de)
            .map(|(k, _)| k.to_string());
        let donnees: Map<String, Value> = match &existante {
            Some(k) => {
                // Une fiche qui existe deja : on ne complete que ses cases VIDES. Ce qu'elle porte a peut-etre deja
                // ete declare.
                let ancienne = ctx.salaries.get(k.as_str()).and_then(|f| socle::ecriture::en_valeur(&f.champs).as_object().cloned()).unwrap_or_default();
                let mut fusion = ancienne.clone();
                for (cle, valeur) in &donnees {
                    if strip(&ancienne.get(cle).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()).is_empty() {
                        fusion.insert(cle.clone(), valeur.clone());
                    }
                }
                fusion.insert("fiche_recue".into(), json!(note));
                fusion
            }
            None => {
                let mut d = donnees;
                d.insert("fiche_recue".into(), json!(note));
                d.insert("actif".into(), json!(true));
                d
            }
        };
        // Les cases vides qu'elle remplit valent de tout temps ; ce qu'elle change d'une fiche qui existe (la reponse
        // precedente, `fiche_recue`) se date comme un changement de l'ecran (`date_proposee`).
        let depuis = match &existante {
            Some(k) => json!(date_proposee(ctx, "salarie", Some(k))?.0.iso()),
            None => Value::Null,
        };
        let f = crate::routes::fiches::ecrire_fiche(ctx, "salarie", existante.clone(), &Value::Object(donnees.clone()), crate::routes::fiches::Forcer::Tout, &depuis, &Value::Null)?;
        let texte_de = |cle: &str| donnees.get(cle).map(valeur_en_texte).unwrap_or_default();
        let nom = format!("{} {}", texte_de("prenom_affichage"), texte_de("nom_affichage"));
        let erreurs = f.anomalies.iter().filter(|a| a.get("gravite").and_then(Value::as_str) == Some("erreur")).count();
        let retour = journal::RetourFiche {
            code: code.clone(),
            abandon: false,
            salarie: f.cle.as_deref().map(droit::contexte::IdSalarie::lire).transpose().map_err(os)?,
            illisible: Some(illisible.clone()),
            erreurs: Some(erreurs),
            quand: quand.clone(),
            qui: qui.to_string(),
            de: Some(de.clone()),
            message: Some(ident.clone()),
            reponse: format!("fiche {} par courriel", if existante.is_some() { "completee" } else { "creee" }),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, &code), journal::Revenu::Fiche(retour)).map_err(os)?;
        r.recues.push(json!({"code": code, "cle": f.cle, "nom": strip(&nom), "illisible": illisible, "erreurs": erreurs}));
    }
    client.fermer();
    Ok(r)
}

// ------------------------------------------------------------------ la compagnie fictive

/// Des personnes inventees, comme celles des compagnies fictives : (prenom, nom, sexe INSEE). Le NIR se
/// fabrique a la demande, cle comprise.
const PERSONNES: [(&str, &str, &str); 4] = [("Marie", "Dorval", "2"), ("Pierre", "Bocage", "1"), ("Rose", "Cheri", "2"), ("Frederick", "Lemaitre", "1")];

/// Le questionnaire que la personne fictive renvoie, rempli ligne a ligne dans les termes memes
/// des questions, le code en dernier. `bouton` : (objet, corps) du lien du courriel de la demande, quand on l'a.
fn reponse_de_fiche(demande: &Attendue, compte: &str, bouton: Option<&(String, String)>) -> Result<Vec<u8>, String> {
    let code = &demande.code;
    let somme: u32 = code.chars().map(|c| c as u32).sum();
    let (prenom, nom, sexe) = PERSONNES[somme as usize % PERSONNES.len()];
    let prenom = if demande.prenom.is_empty() { prenom.to_string() } else { demande.prenom.clone() };
    let an = 80 + code.chars().count() % 15;
    let mois = 1 + somme % 12;
    let treize = format!("{}{:02}{:02}75056{:03}", sexe, an, mois, 1 + somme % 900);
    let cle = 97 - treize.parse::<u64>().unwrap_or(0) % 97;
    let reponses: HashMap<&str, String> = HashMap::from([
        ("prenom", prenom),
        ("nom", nom.to_string()),
        ("naissance", format!("12/{:02}/19{:02}", mois, an)),
        ("lieu", "Paris".to_string()),
        ("pays", "France".to_string()),
        ("nationalite", "française".to_string()),
        ("nir", format!("{}{:02}", treize, cle)),
        ("adresse", "12 rue du Temple".to_string()),
        ("code_postal", "75003".to_string()),
        ("commune", "Paris".to_string()),
        // Le compte fabrique des fictives, a la cle juste (`socle::iban::faute_iban`).
        ("iban", "FR7630001007941234567890185".to_string()),
    ]);
    let rempli = fq::questionnaire(code, &fq::posees(&|c| champs::utile("salarie", c)), &|cle| reponses.get(cle).cloned().unwrap_or_default());
    let sujet = bouton.map(|b| b.0.clone()).unwrap_or_else(|| "Re: fiche".to_string());
    envois::simulation_courriels::reponse_jouee(&demande.adresse, compte, &sujet, &rempli, &format!("fiche-{}", code))
}

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de fiche partie
/// depuis `delai` secondes y trouve sa reponse remplie ; la vraie releve la lit et cree la fiche. Ce qu'on y envoie
/// s'ecrit comme tout courriel joue (`BoiteQuiRepond`, partagee avec `BoiteDesSignatures`).
pub fn boite_des_fiches(ctx: &Contexte, attendues: &[Attendue], delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx);
    let maintenant_naif = socle::paris::maintenant_naif();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine);
    for a in attendues {
        let age = socle::paris::naif_de_iso(&a.quand).map_or(0, |t| maintenant_naif - t);
        if age < delai {
            continue;
        }
        let bouton = envois::simulation_courriels::bouton_de_la_demande(&envoyes, &a.envoyes);
        let brut = reponse_de_fiche(a, Courriel::compte(&boite), bouton.as_ref()).map_err(|m| erreur("ValueError", m))?;
        boite.repondre(&a.code, brut);
    }
    Ok(boite)
}

/// `couts` : le cout de l'annee `charge["annee"]`, au jour du contexte.
fn couts(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let _ = o;
    let annee = valeur_en_texte(exiger(charge, "annee")?);
    let mut r = documents::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(annee), false, envois::journal::lu(&ctx.racine).map(|j| j.departs)?);
    r.couts_de_l_annee().map_err(|e| Echec { classe: e.classe, message: e.message })
}

/// `poser-courriel` : le courriel `email` du salarie `cle`, saisi la ou il manque (la boite des contrats a
/// faire signer). La fiche entiere s'ecrit comme depuis l'ecran (`ecrire_fiche`), seul le courriel y change : il se
/// verifie avec elle, et une fiche qui refuse le dit.
pub(crate) fn poser_courriel(ctx: &Contexte, cle: &str, email: &str) -> R<()> {
    let Some(Value::Object(fiche)) = ctx.salaries.get(cle).map(|f| socle::ecriture::en_valeur(&f.champs)) else {
        return Err(refus(format!("Salarié {} inconnu.", socle::fichiers::repr(cle))));
    };
    let mut donnees: Map<String, Value> = fiche.iter().filter(|(k, _)| !interne(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    donnees.insert("email".into(), Value::String(email.trim().to_string()));
    let (depuis, _) = date_proposee(ctx, "salarie", Some(cle))?;
    ecrire_fiche(ctx, "salarie", Some(cle.to_string()), &Value::Object(donnees), Forcer::Rien, &json!(depuis.iso()), &Value::Null)?;
    Ok(())
}
