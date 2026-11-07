//! Les routes des Exports : ce qui reste d'une annee de paies (le dossier d'un salarie, les exports et leur logiciel
//! comptable, les droits des intermittents), et ce qui part d'elles (le dossier par courriel, la regularisation de la
//! CASC-SVP).

use droit::fiche::Lue as _;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::import::casc::{marquer_regularise, RegularisationNotee};
use crate::routes::dossier_salarie::{construire, AnneeDite, DemandeDeDossier, DossierEnvoye, DossierRendu};
use crate::routes::{en_json, lire_charge, INTERFACE};
use socle::mois::Mois;
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::documents::{client_courriel, expediteur_de, refus, Ouvrir};
use crate::routes::engagement::maintenant;

use documents::etats_compta::ChoixCompta;
use documents::plan_comptable::{controler_plan, tables};
use droit::contexte::Contexte;
use envois::envoi::{self, Message};
use envois::ligne_d_envoi::{echec_d_envoi, noter, noter_echec_d_envoi, par_courriel};
use envois::journal;
use socle::date::jour_iso;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{nom_du, repr};
use socle::html::courriel_en_html;
use crate::routes::catalogue::{Ecran, Place};

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

/// Une lecture sans charge de l'Historique ou des Exports.
const fn lecture(route: Route, geste: &'static str, rend: &'static str) -> Fiche {
    Fiche { route, geste, charge: &[], exemple: "{}", rend, refus: &[OPERATION], acte: Acte::Lit, avance: false, phrase: 0, ecran: ECRAN }
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    lecture(Route::DossierFormes, "Dire ce que peut porter le dossier d'un salarié : catégories, périodes.", "salaries, categories, periodes"),
    lecture(Route::ExportsPeriodes, "Dire ce qui s'exporte : mois, années, états, formats, logiciel comptable.", "mois, annees, etats, formats, logiciel retenu"),
    Fiche {
        route: Route::RetenirLogiciel,
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
        route: Route::CascRegularise,
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
        route: Route::Exporter,
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
        route: Route::DossierEnvoyer,
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
        route: Route::Droits,
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
        route: Route::CascEnvoyer,
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
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::DossierFormes => {
            let super::SansCharge {} = lire_charge(charge)?;
            en_json(dossier_formes(ctx)?)
        }
        Route::ExportsPeriodes => en_json(exports_periodes(ctx, lire_charge(charge)?)?),
        Route::RetenirLogiciel => en_json(retenir_logiciel(ctx, lire_charge(charge)?)?),
        Route::CascRegularise => en_json(casc_regularise(ctx, lire_charge(charge)?)?),
        Route::Exporter => en_json(exporter(ctx, &lire_charge(charge)?)?),
        Route::DossierEnvoyer => en_json(dossier_envoyer(ctx, lire_charge(charge)?, &mut || client_courriel(ctx))?),
        Route::CascEnvoyer => en_json(casc_envoyer(ctx, lire_charge(charge)?, &mut || client_courriel(ctx))?),
        Route::Droits => en_json(droits(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
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

/// Une piece du mois dans `sorties/<mois>/envoi/` : son nom, et la cle du salarie qu'elle nomme (vide sinon).
pub(crate) struct PieceDeLEnvoi {
    pub(crate) nom: String,
    pub(crate) salarie: String,
}

/// Les pieces du mois a ouvrir ou a remettre a la main : les PDF de `sorties/<mois>/envoi/`, chacune seule (jamais le PDF
/// a imprimer, qui reunit celles de plusieurs salaries).
pub(crate) fn pieces_de_l_envoi(ctx: &Contexte, mois: &str) -> R<Vec<PieceDeLEnvoi>> {
    let envoi = ctx.racine.join("sorties").join(mois).join("envoi");
    if !envoi.exists() {
        return Ok(Vec::new());
    }
    Ok(pdf_de(&envoi)?
        .into_iter()
        .filter(|nom| !nom.starts_with(crate::compagnie::signature::PREFIXE_IMPRESSION))
        .map(|nom| {
            let morceaux: Vec<&str> = nom.strip_suffix(".pdf").filter(|s| !s.is_empty()).unwrap_or(&nom).split('_').collect();
            let salarie = if morceaux.len() > 2 { morceaux[1].to_string() } else { String::new() };
            PieceDeLEnvoi { nom, salarie }
        })
        .collect())
}

// -------------------------------------------------------------- dossier-formes


/// Les années des mois, de la plus récente à la plus ancienne.
fn annees_de(dispo: &[String]) -> Vec<String> {
    let mut annees: Vec<String> = dispo.iter().map(|m| m.chars().take(4).collect::<String>()).collect::<HashSet<_>>().into_iter().collect();
    annees.sort();
    annees.reverse();
    annees
}

/// Un salarie que la page propose : sa cle, son nom, s'il est actif, s'il a un courriel.
#[derive(Debug, Serialize)]
struct SalariePropose {
    cle: String,
    nom: String,
    actif: bool,
    email: bool,
}

/// Une categorie a cocher : son nom, et ce qu'elle porte.
#[derive(Debug, Serialize)]
struct CategorieProposee {
    categorie: &'static str,
    quoi: &'static str,
}

/// Ce que rend `dossier-formes` : les salaries, les categories a cocher, les mois et les annees, si une boite est posee
/// (l'envoi), et le dossier ou le dossier s'ecrit.
#[derive(Debug, Serialize)]
struct FormesDuDossier {
    phrase: String,
    salaries: Vec<SalariePropose>,
    categories: Vec<CategorieProposee>,
    mois: Vec<String>,
    annees: Vec<String>,
    boite_liee: bool,
    dossier: String,
}

/// Les salaries que la page propose.
fn salaries_de_la_page(ctx: &Contexte) -> Vec<SalariePropose> {
    let mut fiches: Vec<(&droit::contexte::IdSalarie, &droit::fiche::FicheSalarie)> = ctx.salaries.iter().collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    fiches
        .into_iter()
        .map(|(k, v)| SalariePropose {
            cle: k.to_string(),
            nom: format!("{} {}", v.nom.texte(), if v.prenom_affichage.texte().is_empty() { v.prenom.texte() } else { v.prenom_affichage.texte() }),
            actif: !v.archivee(),
            email: !v.email.texte().is_empty(),
        })
        .collect()
}

/// Ce que la page propose -- les salaries,
/// les categories a cocher, les periodes, et si l'envoi est possible.
fn dossier_formes(ctx: &Contexte) -> R<FormesDuDossier> {
    let salaries = salaries_de_la_page(ctx);
    // `periodes_exportables` : les mois, puis (sans rien en dire) le logiciel du comptable.
    let mois: Vec<String> = documents::etats_paie::mois_disponibles(ctx)?.into_keys().collect();
    let annees = annees_de(&mois);
    etat_logiciel(ctx)?;
    let categories: Vec<CategorieProposee> = crate::routes::dossier_salarie::CATEGORIES.iter().map(|c| CategorieProposee { categorie: c, quoi: crate::routes::dossier_salarie::libelle_de(c) }).collect();
    Ok(FormesDuDossier {
        phrase: format!("Dossiers possibles : {} ; sortes de pièces : {}.", super::compte(salaries.len(), "salarié", "salariés"), categories.len()),
        salaries,
        categories,
        mois,
        annees,
        boite_liee: crate::routes::ouverture::boite_liee(ctx)?,
        dossier: ctx.racine.join("exports").join("dossiers").display().to_string(),
    })
}

// ------------------------------------------------- le logiciel du comptable

/// Un logiciel du comptable que la page propose : tel que les tables des exports le livrent
/// (`documents::plan_comptable::tables`), et ce que l'ecran en dit une fois choisi.
#[derive(Debug, Serialize)]
struct LogicielPropose {
    #[serde(flatten)]
    livre: Value,
    dit: String,
}

/// Ce que la page montre du logiciel du comptable : les logiciels, celui retenu (vide : aucun), l'analytique, et ce que
/// le plan comptable ne tiendrait pas dans son format.
#[derive(Debug, Serialize)]
pub(crate) struct LogicielDuComptable {
    logiciels: Vec<LogicielPropose>,
    logiciel: String,
    analytique: bool,
    controle: Vec<String>,
}

/// Le logiciel du comptable, tel que la page le montre.
pub(crate) fn etat_logiciel(ctx: &Contexte) -> R<LogicielDuComptable> {
    let choix = ChoixCompta::lu(&ctx.racine)?;
    let livres = tables()["logiciels"].as_array().cloned().unwrap_or_default();
    let controle = match livres.iter().find(|l| l["logiciel"].as_str() == Some(choix.logiciel.as_str())) {
        Some(l) => controler_plan(ctx, l["format"].as_str().unwrap_or(""))?,
        None => Vec::new(),
    };
    // Ce que l'ecran dit de chaque logiciel, une fois choisi.
    let logiciels = livres
        .into_iter()
        .map(|l| LogicielPropose { dit: format!("Fichier {} : {}. Retenu pour les prochaines fois.", super::mot(&l["format_lisible"]), super::mot(&l["pourquoi"])), livre: l })
        .collect();
    Ok(LogicielDuComptable { logiciels, logiciel: choix.logiciel, analytique: choix.analytique, controle })
}

/// La page ne propose pas la CASC-SVP de la
/// 3090 a une compagnie de la 1285, ni la dette de conges a qui n'emploie pas de
/// permanents.
fn etat_concerne(ctx: &Contexte, etat: &str) -> bool {
    let idcc = ctx.entreprise.idcc.texte().to_string();
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

/// La charge de `exports-periodes` : deux faits du poste, que la fenetre constate (`interface::api`) -- la bibliotheque
/// des classeurs est-elle la (`xlsx`), la fabrique des PDF est-elle installee (`pdf`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeExportsPeriodes {
    #[serde(default)]
    xlsx: bool,
    #[serde(default)]
    pdf: bool,
}

/// CE QUE REND `exports-periodes` : sa phrase, les mois et les annees qu'il y a matiere a exporter, les etats que la
/// compagnie peut demander, les formats et les regroupements (tels que les tables des exports les livrent), le dossier
/// des exports, les deux faits du poste, le logiciel du comptable, et ce que l'ecran dit (`ecran`).
#[derive(Debug, Serialize)]
struct ExportsPossibles {
    phrase: String,
    mois: Vec<String>,
    annees: Vec<String>,
    etats: Vec<Value>,
    formats: Value,
    regroupements: Value,
    dossier: String,
    xlsx: bool,
    pdf: bool,
    compta: LogicielDuComptable,
    ecran: EcranDesExports,
}

/// CE QUE L'ECRAN DES EXPORTS DIT : rien a exporter, les regroupements, l'analytique, le format, les coordonnees, le
/// dossier, les categories du dossier d'un salarie.
#[derive(Debug, Serialize)]
struct EcranDesExports {
    vide: Option<&'static str>,
    regroupement_livre: &'static str,
    regroupement: &'static str,
    analytique: &'static str,
    format_libre: &'static str,
    brique_absente: &'static str,
    coordonnees: &'static str,
    dossier: String,
    categories: &'static str,
}

/// Les mois et les annees qu'il y a matiere a exporter, les etats, les formats, et le logiciel du comptable.
fn exports_periodes(ctx: &Contexte, c: ChargeExportsPeriodes) -> R<ExportsPossibles> {
    let mois: Vec<String> = documents::etats_paie::mois_disponibles(ctx)?.into_keys().collect();
    let t = tables();
    let etats: Vec<Value> = t["etats"].as_array().expect("etats").iter().filter(|e| etat_concerne(ctx, e["etat"].as_str().expect("etat"))).cloned().collect();
    let annees = annees_de(&mois);
    let dossier = ctx.racine.join("exports").display().to_string();
    let ecran = EcranDesExports {
        vide: mois.is_empty().then_some("Aucune paie saisie : il n'y a rien à exporter pour l'instant."),
        regroupement_livre: "Par salarié, ce qu'on envoie au comptable ; par mois, ce qu'on rapproche de la banque ; par spectacle, ce qui justifie une subvention. Les trois disent la même chose, et le total ne bouge pas.",
        regroupement: "Le total ne bouge pas : seule la répartition des lignes change.",
        analytique: "Les salaires bruts ventilés par spectacle. Le code de chaque spectacle se règle sur sa fiche : Configuration › Spectacles, case « Code analytique ». Vide, il est tiré du titre.",
        format_libre: "Le format se choisit ci-dessous.",
        brique_absente: "brique absente, le CSV sera écrit",
        coordonnees: "Le NIR, la naissance, l'adresse. Sans la case, rien de tout cela ne sort ; un fichier qui les porte ne s'envoie qu'à qui en a besoin.",
        dossier: format!("{} — et nulle part ailleurs : un export emporte des salaires", dossier),
        categories: "Une seule case : un PDF. Plusieurs : un ZIP, un PDF par catégorie.",
    };
    Ok(ExportsPossibles {
        phrase: format!("Exports : {} ; mois payés : {} ; années : {}.", super::compte(etats.len(), "état", "états"), mois.len(), annees.len()),
        mois,
        annees,
        etats,
        formats: t["formats"].clone(),
        regroupements: t["regroupements"].clone(),
        dossier,
        xlsx: c.xlsx,
        pdf: c.pdf,
        compta: etat_logiciel(ctx)?,
        ecran,
    })
}

/// La charge de `retenir-logiciel` : le logiciel du comptable (absent ou vide : aucun), et l'analytique.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeRetenirLogiciel {
    #[serde(default)]
    logiciel: Option<String>,
    #[serde(default)]
    analytique: bool,
}

/// Ce que rend `retenir-logiciel` : sa phrase, et le logiciel du comptable tel que la page le montre.
#[derive(Debug, Serialize)]
struct LogicielRetenu {
    phrase: String,
    #[serde(flatten)]
    compta: LogicielDuComptable,
}

/// Le logiciel du comptable, et l'analytique, retenus une fois pour
/// toutes. Rend le controle du plan pour ce logiciel. Ce n'est pas un export : un reglage, pas une donnee de paie.
fn retenir_logiciel(ctx: &Contexte, c: ChargeRetenirLogiciel) -> R<LogicielRetenu> {
    let nom = c.logiciel.unwrap_or_default();
    if !nom.is_empty() && !documents::plan_comptable::logiciels_connus().iter().any(|l| *l == nom) {
        return Err(refus(format!("Logiciel inconnu : « {} ».", nom)));
    }
    ChoixCompta::nouveau(nom, c.analytique).ecrire(&ctx.racine)?;
    let compta = etat_logiciel(ctx)?;
    let phrase = if compta.logiciel.is_empty() { "Aucun logiciel comptable retenu.".to_string() } else { format!("Logiciel comptable retenu : {}.", compta.logiciel) };
    Ok(LogicielRetenu { phrase, compta })
}

/// Un nombre d'une charge, ecrit en nombre ou tel que la page le saisit (« 2025 », « 12.80 »).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum NombreDit {
    Nombre(serde_json::Number),
    Texte(String),
}

impl NombreDit {
    fn texte(&self) -> String {
        match self {
            NombreDit::Nombre(n) => n.to_string(),
            NombreDit::Texte(t) => t.clone(),
        }
    }
}

/// La charge de `casc-regularise` : l'annee, le jour du virement, le montant vire.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCascRegularise {
    annee: NombreDit,
    vire_le: String,
    montant: NombreDit,
}

/// Ce que rend `casc-regularise` : sa phrase, et la regularisation notee.
#[derive(Debug, Serialize)]
struct CascNotee {
    phrase: String,
    #[serde(flatten)]
    notee: RegularisationNotee,
}

/// Un montant ecrit en texte (« 12.80 », « 12,80 »), tel qu'une phrase le dit en euros ; tel quel s'il ne se lit pas.
fn en_euros(t: &str) -> String {
    t.trim().replace(',', ".").parse().map(socle::texte::euros).unwrap_or_else(|_| t.to_string())
}

/// Note qu'une annee de CASC-SVP est regularisee (apres le virement).
fn casc_regularise(ctx: &Contexte, c: ChargeCascRegularise) -> R<CascNotee> {
    let aujourd_hui = maintenant();
    let notee = marquer_regularise(ctx, &c.annee.texte(), &c.vire_le, &c.montant.texte(), INTERFACE, &aujourd_hui[..10])?;
    let montant = en_euros(&notee.montant);
    Ok(CascNotee { phrase: format!("CASC-SVP {} notée régularisée : {} virés le {}.", notee.annee, montant, super::quand(&notee.vire_le)), notee })
}

/// La charge de `exporter` : l'etat (le livre de paie sans lui), la periode (le mois, l'annee, ou toutes les annees),
/// le regroupement, le format, les coordonnees des salaries, l'apercu (rien n'est ecrit), et pour les ecritures
/// comptables le logiciel du comptable et l'analytique. Un texte vide vaut absent : la page envoie ses cases telles
/// qu'elles sont.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeExporter {
    #[serde(default)]
    etat: Option<String>,
    #[serde(default)]
    mois: Option<String>,
    #[serde(default)]
    annee: Option<AnneeDite>,
    #[serde(default)]
    toutes_les_annees: bool,
    #[serde(default)]
    regroupement: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    coordonnees: bool,
    #[serde(default)]
    apercu: bool,
    #[serde(default)]
    logiciel: Option<String>,
    #[serde(default)]
    analytique: Option<bool>,
}

/// Un texte de la charge, vide : rien.
fn dit(t: &Option<String>) -> Option<String> {
    t.clone().filter(|t| !t.is_empty())
}

/// CE QUE REND `exporter` : sa phrase, l'etat tel que son module le rend (le livre de paie, un tableau, la
/// regularisation CASC-SVP), ce qu'il dit de lui-meme sous la phrase (`phrases` : ses avertissements, ce qui est du, ce
/// qui est infere), et ce que l'ecran en dit (`ecran`).
#[derive(Debug, Serialize)]
pub(crate) struct EtatExporte {
    phrase: String,
    #[serde(flatten)]
    etat: Value,
    phrases: Vec<String>,
    ecran: EcranDUnEtat,
}

/// CE QUE L'ECRAN DIT D'UN ETAT EXPORTE : ses donnees personnelles, ses totaux partiels, et pour la regularisation
/// CASC-SVP ce que disent ses deux etapes.
#[derive(Debug, Serialize)]
struct EcranDUnEtat {
    personnelles: Option<&'static str>,
    partiels: Option<String>,
    virement: &'static str,
    automatique: &'static str,
    a_la_main: &'static str,
}

/// Le verbe unique, `exporter <etat>` : l'etat demande, tel que l'ecran le montre.
fn exporter(ctx: &Contexte, c: &ChargeExporter) -> R<EtatExporte> {
    let nom = |f: &Option<std::path::PathBuf>| f.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    // (le titre, les fichiers ecrits, ses avertissements, ses donnees personnelles, ses totaux partiels, la boite posee)
    let (etat, titre, ecrits, avertissements, personnelles, partiels, boite_liee) = match exporter_objet(ctx, c)? {
        Exporte::Casc(r) => (crate::import::casc::vers_json(&r), r.periode.clone(), r.fichiers.iter().map(|f| nom_du(std::path::Path::new(f))).collect::<Vec<_>>(), r.avertissements.clone(), false, Vec::new(), r.boite_liee),
        Exporte::Livre(l) => {
            let bulletins = l.totaux.bulletins;
            let partiels = l
                .totaux
                .inconnus
                .iter()
                .map(|(colonne, n)| {
                    let sur = bulletins.saturating_sub(*n);
                    format!("{} sur {} bulletin{} sur {}", colonne.replace('_', " "), sur, if sur == 1 { "" } else { "s" }, bulletins)
                })
                .collect();
            (l.json(), l.periode.clone(), nom(&l.fichier).into_iter().collect(), l.avertissements.clone(), false, partiels, false)
        }
        Exporte::Tableau(t) => (documents::tableaux_export::brut_tableau(&t), t.titre.clone(), nom(&t.fichier).into_iter().collect(), t.avertissements.clone(), t.donnees_personnelles, Vec::new(), false),
    };
    let phrase = if ecrits.is_empty() { format!("{} : aperçu, rien n'est écrit.", titre) } else { format!("{} : écrit dans {}.", titre, ecrits.join(", ")) };
    let ecran = EcranDUnEtat {
        personnelles: personnelles.then_some("Cet état porte des données personnelles : il reste dans prive/, et ne s'envoie qu'à qui en a besoin."),
        partiels: (!partiels.is_empty()).then(|| format!("Totaux partiels : {}.", partiels.join(" ; "))),
        virement: "Vers Audiens Santé Prévoyance – Encaissement. L'IBAN se lit dans l'espace employeur Audiens ou sur la page employeurs du CASC-SVP.",
        automatique: if boite_liee {
            "Audiens reçoit les masses et les dates des virements ; le CASC-SVP, la liste nominative. Une copie reste dans le dossier, et les années sont notées régularisées."
        } else {
            "La boîte courriel n'est pas posée : Configuration › Courriel. Une fois posée, l'envoi se fait d'ici en un clic."
        },
        a_la_main: "Joindre la liste nominative au courriel du CASC-SVP, recopier les deux courriels ci-dessous, puis noter la régularisation.",
    };
    Ok(EtatExporte { phrase, etat, phrases: avertissements, ecran })
}

/// Ce qu'un export rend, avant d'etre mis en JSON (le chemin ecrit en entier, les montants en `Decimal`).
pub(crate) enum Exporte {
    Casc(crate::import::casc::Regularisation),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON ; sans `format`, le CSV.
pub(crate) fn exporter_objet(ctx: &Contexte, c: &ChargeExporter) -> R<Exporte> {
    use documents::etats_paie::Demande;
    let etat = dit(&c.etat).unwrap_or_else(|| "livre-de-paie".to_string());
    let tables = documents::plan_comptable::tables();
    let etats = tables["etats"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let Some(forme) = etats.iter().find(|e| e["etat"].as_str() == Some(etat.as_str())) else {
        let mut noms: Vec<&str> = etats.iter().filter_map(|e| e["etat"].as_str()).collect();
        noms.sort_unstable();
        return Err(refus(format!("État inconnu : « {} ». Les états possibles sont {}.", etat, noms.join(", "))));
    };
    let mut d = Demande {
        mois: dit(&c.mois),
        annee: c.annee.as_ref().map(AnneeDite::texte).filter(|a| !a.is_empty()),
        toutes: c.toutes_les_annees,
        regroupement: dit(&c.regroupement).unwrap_or_else(|| "salarie".to_string()),
        format: dit(&c.format).unwrap_or_else(|| "csv".to_string()),
        coordonnees: c.coordonnees,
        ecrire: !c.apercu,
        logiciel: dit(&c.logiciel),
        analytique: c.analytique,
        departs: envois::journal::lu(&ctx.racine).map(|j| j.departs)?,
    };
    if etat == "regularisation-casc-svp" {
        let demande = crate::import::casc::Demande { mois: d.mois.clone(), annee: d.annee.clone(), toutes_les_annees: d.toutes, boite_liee: crate::routes::ouverture::boite_liee(ctx).unwrap_or(false), ..Default::default() };
        let mut r = crate::import::casc::regularisation(ctx, &demande)?;
        if d.ecrire {
            crate::import::casc::ecrire(ctx, &mut r)?;
        }
        return Ok(Exporte::Casc(r));
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
        "journal-des-declarations" => documents::etats_personnel::journal_des_declarations(ctx, &d, &actes_du_journal(ctx)?),
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

/// La charge de `dossier-envoyer` : les salaries (aucun : chacun), la periode (le mois, l'annee, ou toutes les annees), ce
/// que le dossier porte (tout sinon), et le mot de passe du logiciel tape pour l'envoi. L'envoi refait le dossier de
/// chacun : ni apercu, ni dossier d'ensemble.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDossierEnvoyer {
    #[serde(default)]
    salaries: Vec<String>,
    #[serde(default)]
    mois: Option<Mois>,
    #[serde(default)]
    annee: Option<AnneeDite>,
    #[serde(default)]
    toutes_les_annees: bool,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    motdepasse: Option<String>,
}

/// La route `dossier-envoyer` : le dossier se refait ici (`dossier_salarie`, un fichier par salarie, ecrit dans
/// `exports/dossiers/`), puis part a chaque salarie par la boite de la compagnie. La reponse est celle du dossier
/// (`DossierRendu`) et, sous `envoi`, ce que l'envoi a fait.
fn dossier_envoyer(ctx: &Contexte, c: ChargeDossierEnvoyer, ouvrir: Ouvrir) -> R<DossierRendu> {
    let demande = |apercu: bool| DemandeDeDossier {
        salaries: c.salaries.clone(),
        mois: c.mois,
        annee: c.annee.clone(),
        toutes_les_annees: c.toutes_les_annees,
        categories: c.categories.clone(),
        ensemble: false,
        apercu,
    };
    // CE QUI PARTIRAIT, SANS RIEN ECRIRE : chacun a son adresse, puis la preuve ; le dossier ne s'ecrit qu'ensuite.
    let vu = construire(ctx, &demande(true))?;
    adresses_de(ctx, &vu.salaries)?;
    let a_qui = format!("{} salarie(s) : {}", vu.salaries.len(), vu.salaries.iter().map(|c| ctx.nomme(c)).collect::<Vec<_>>().join(", "));
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let forme = verifier_preuve(ctx, Some(&Preuve::du_mot_de_passe(c.motdepasse.clone())), &aujourdhui.mois_civil().to_string(), "envoi du dossier aux salaries", &a_qui)?;
    let d = construire(ctx, &demande(false))?;
    let envoi = envoyer_dossier(ctx, &d, &Preuve { donnee: Some(forme), ..Preuve::default() }, INTERFACE, ouvrir)?;
    let phrase = format!("Dossier de {} ({}) : {}.", d.salaries.join(", "), d.periode, super::compte(d.fichiers.len(), "fichier joint", "fichiers joints"));
    Ok(DossierRendu::de(d, phrase, Some(envoi)))
}

/// Chacun de `cles` a une adresse dans sa fiche ; sinon le refus qui le nomme. Rien n'est parti.
fn adresses_de(ctx: &Contexte, cles: &[String]) -> R<Vec<String>> {
    cles.iter()
        .map(|cle| {
            let s = ctx.salaries.get(cle.as_str()).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
            s.email.dite().filter(|e| !e.is_empty()).cloned().ok_or_else(|| {
                refus(format!("Pas d'adresse e-mail pour {} : compléter sa fiche avant d'envoyer. Rien n'est parti.", ctx.nomme(cle)))
            })
        })
        .collect()
}

/// Envoie a chaque salarie SON dossier, par la boite de la compagnie.
/// Irreversible : la meme preuve que les pieces du mois, le meme journal. Un dossier `ensemble` ne part a personne.
fn envoyer_dossier(ctx: &Contexte, d: &crate::routes::dossier_salarie::Dossier, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<DossierEnvoye> {
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
        let s = ctx.salaries.get(cle.as_str()).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
        let prenom = match s.prenom_affichage.texte() {
            "" if s.prenom.is_none() => return Err(erreur("KeyError", socle::fichiers::absent("prenom"))),
            "" => s.prenom.texte().to_string(),
            p => p.to_string(),
        };
        let signataire = ctx.entreprise.signataire.dite().cloned().unwrap_or_default();
        let ses_pages = d.pages.get(cle);
        let liste = d
            .categories
            .iter()
            .filter(|c| ses_pages.is_some_and(|p| p.get(c).is_some()))
            .map(|c| crate::routes::dossier_salarie::libelle_de(c).to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        let corps = envoi::formater(
            CORPS_DOSSIER,
            &[
                ("prenom", prenom.clone()),
                ("periode", if d.periode == "toute la période" { String::new() } else { format!(" ({})", d.periode) }),
                ("liste", liste),
                ("reedition", if ses_pages.is_some_and(|p| p.get("contrats").is_some()) { REEDITION_CONTRATS.to_string() } else { String::new() }),
                ("signataire", signataire.prenom_affichage.remplie().unwrap_or(signataire.prenom.texte()).to_string()),
                ("raison_sociale", ctx.entreprise.nommee()),
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
    verifier_preuve(ctx, Some(preuve), &aujourdhui.mois_civil().to_string(), "envoi du dossier aux salaries", &a_qui)?;
    let expediteur = expediteur_de(client.as_ref());
    let detail = journal::DetailDossier {
        periode: d.periode.clone(),
        categories: d.categories.iter().map(|c| c.to_string()).collect(),
        salaries: messages.iter().map(|m| Ok(journal::DossierServi { cle: droit::contexte::IdSalarie::lire(&m.cle)?, piece: m.pieces.first().map(|p| nom_du(p)) })).collect::<Result<_, String>>()?,
        expediteur: expediteur.clone(),
        partis: None,
        echec_a: None,
    };
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, journal::Ligne::DossierSalarie, None, qui, &e.partis, &e.echoue, &e.message, detail, None)?;
            return Err(echec_d_envoi(e.message));
        }
    };
    noter(ctx, journal::Ligne::DossierSalarie(par_courriel(None, qui, true, &partis.join(", "), "", detail, None)))?;
    Ok(DossierEnvoye { partis, messages: messages.len(), expediteur })
}

/// Les actes du journal des envois, tels que le journal des declarations les montre.
fn actes_du_journal(ctx: &Contexte) -> R<Vec<documents::etats_personnel::ActeDuJournal>> {
    let lignes = journal::lu(&ctx.racine).map_err(|m| erreur("OSError", m))?.lignes;
    Ok(lignes
        .iter()
        .map(|l| documents::etats_personnel::ActeDuJournal {
            quand: l.quand().to_string(),
            quoi: l.acte().nom().to_string(),
            main_propre: l.main_propre(),
            mois: l.mois().map(|m| m.to_string()),
            mode: l.mode().nom().to_string(),
            accepte: l.accepte(),
            fichier: l.fichier().unwrap_or_default().to_string(),
            destinataire: l.destinataire().to_string(),
            reponse: l.reponse().to_string(),
            origine: l.origine().to_string(),
        })
        .collect())
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
/// La charge de `casc-envoyer` : le jour de chaque virement fait, par annee (AAAA-MM-JJ), et le mot de passe du logiciel
/// tape pour l'envoi.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCasc {
    #[serde(default)]
    vire_le: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    motdepasse: Option<String>,
}

fn casc_envoyer(ctx: &Contexte, c: ChargeCasc, ouvrir: Ouvrir) -> R<CascEnvoyee> {
    let qui = INTERFACE;
    use crate::import::casc::{ecrire, regularisation, Demande};
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let mut dates: Vec<(String, String)> = Vec::new();
    for (a, texte) in &c.vire_le {
        let Some(jour) = jour_iso(texte) else {
            return Err(refus(format!("Date du virement {} illisible : {} (attendu AAAA-MM-JJ). Rien n'est parti.", a, repr(texte))));
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
    verifier_preuve(ctx, Some(&Preuve::du_mot_de_passe(c.motdepasse.clone())), &dernier_mois, "envoi a Audiens et au CASC-SVP (liste nominative)", "Audiens et CASC-SVP")?;

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
    let date_de = |a: &str| dates.iter().find(|(k, _)| k == a).map(|(_, d)| d.clone()).unwrap_or_default();
    let mut detail = journal::DetailCasc {
        annees: r.annees.iter().map(|a| journal::AnneeCasc { annee: a.annee.clone(), vire_le: date_de(&a.annee), montant: a.cotisation.to_string() }).collect(),
        mois: r.mois.iter().map(|m| m.mois.clone()).collect(),
        total: r.total.to_string(),
        expediteur: compte.clone(),
        eml: Vec::new(),
        partis: None,
        echec_a: None,
    };
    let mut copies: Vec<Vec<u8>> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &compte, client.as_mut(), None, Some(&mut copies)) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, journal::Ligne::RegularisationCasc, None, qui, &e.partis, &e.echoue, &e.message, detail, None)?;
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
    detail.eml = emls.clone();
    noter(ctx, journal::Ligne::RegularisationCasc(par_courriel(None, qui, true, &partis.join(", "), &format!("{} courriel(s) envoye(s)", partis.len()), detail, None)))?;
    // Les annees regularisees : leurs mois sortent des dossiers suivants. S'ecrit APRES le virement, jamais avant.
    let auj = socle::paris::maintenant();
    let mut annees: Vec<RegularisationNotee> = Vec::new();
    for a in &r.annees {
        annees.push(marquer_regularise(ctx, &a.annee, &date_de(&a.annee), &a.cotisation.to_string(), qui, &auj[..10])?);
    }
    let dites: Vec<&str> = annees.iter().map(|a| a.annee.as_str()).collect();
    let notee = if dites.len() > 1 { "notées régularisées" } else { "notée régularisée" };
    Ok(CascEnvoyee {
        phrase: format!("CASC-SVP, {} : {}, envoyé à {} ; {} {}.", r.periode, socle::texte::euros(r.total), partis.join(" et "), dites.join(" et "), notee),
        partis,
        expediteur: compte,
        periode: r.periode,
        total: r.total.to_string(),
        annees,
        eml: emls,
        dossier: garde.to_string_lossy().into_owned(),
    })
}

/// Ce que rend `casc-envoyer` : sa phrase, les destinataires servis, la boite qui envoie, la periode et le total
/// regularises, les annees notees regularisees, les deux messages gardes et leur dossier.
#[derive(Debug, Serialize)]
struct CascEnvoyee {
    phrase: String,
    partis: Vec<String>,
    expediteur: String,
    periode: String,
    total: String,
    annees: Vec<RegularisationNotee>,
    eml: Vec<String>,
    dossier: String,
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

/// La charge de `droits` : le mois du tableau (aucun : celui du jour).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDroits {
    #[serde(default)]
    mois: Option<Mois>,
}

/// Une rangee du tableau du mois : ce qu'elle compte, et sa valeur dans chaque colonne.
#[derive(Debug, Serialize)]
struct Rangee {
    quoi: &'static str,
    valeurs: Vec<String>,
}

/// Le tableau du mois d'un intermittent : les titres des colonnes, et ses rangees.
#[derive(Debug, Serialize)]
struct TableauDesDroits {
    titres: Vec<String>,
    lignes: Vec<Rangee>,
}

/// Ce que `droits` dit d'un intermittent : son nom, sa fenetre, ses comptes (`[quoi, valeur]`), ou il en est, ses notes,
/// et le tableau du mois.
#[derive(Debug, Serialize)]
struct DroitsDUnSalarie {
    cle: String,
    nom: String,
    fenetre: String,
    lignes: Vec<(String, String)>,
    phrase: String,
    notes: Vec<String>,
    colonnes: TableauDesDroits,
}

/// Ce que rend `droits` : le mois, ce que vaut ce compte, chaque intermittent.
#[derive(Debug, Serialize)]
struct Droits {
    phrase: &'static str,
    mois: String,
    phrases: Vec<String>,
    salaries: Vec<DroitsDUnSalarie>,
}

/// LES HEURES ET CACHETS DES INTERMITTENTS au regard des annexes 8 et 10 (`droit::cumul` : la fenetre, le plafond des
/// cachets d'un mois, l'enseignement plafonne, le seuil, lus du bareme) : pour chaque salarie dont un engagement est connu
/// (importe ou valide), sa position dans sa fenetre et le tableau du mois. Le compte ne porte que les engagements de cette
/// compagnie : la phrase le dit.
fn droits(ctx: &Contexte, c: ChargeDroits) -> R<Droits> {
    use droit::cumul::{anniversaire_configure, position_droits, tableau_cumul};
    let mois = c.mois.unwrap_or_else(|| ctx.aujourdhui.mois_civil()).to_string();
    let engs = engagement::cumul::lire_engagements(ctx, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let cles: BTreeSet<&str> = engs.iter().map(|e| e.salarie.as_str()).collect();
    let mut salaries = Vec::new();
    for cle in cles {
        let anniv = anniversaire_configure(ctx, cle)?;
        let p = position_droits(ctx, cle, &engs, anniv, None)?;
        let c = &p.constantes;
        let h = |n: i64| format!("{} h", n);
        let mut lignes: Vec<(String, String)> = vec![("Cachets déclarés".into(), p.cachets().to_string())];
        if p.cachets_perdus() != 0 {
            lignes.push((format!("Retenus ({} par mois au plus)", c.cachets_par_mois), p.cachets_retenus().to_string()));
        }
        lignes.push((format!("Convertis ({} h par cachet)", c.heures_par_cachet), h(p.cachets_retenus() * c.heures_par_cachet)));
        lignes.push(("Heures déclarées".into(), h(p.heures())));
        lignes.push(("Total d'heures équivalentes".into(), h(p.heures_retenues())));
        if p.heures_enseignement() != 0 {
            // Deux comptes des qu'il y a de l'enseignement : ces heures ouvrent les droits (plafonnees), sans augmenter l'allocation.
            lignes.push(("Dont enseignement".into(), h(p.heures_enseignement())));
            lignes.push((format!("Enseignement retenu (plafond {} h)", p.plafond_enseignement()?), h(p.heures_enseignement_retenues()?)));
            lignes.push(("Pour l'ouverture des droits".into(), h(p.heures_ouverture()?)));
            lignes.push(("Pour le montant de l'allocation".into(), h(p.heures_montant())));
        }
        lignes.push(("Seuil des annexes 8 et 10".into(), h(c.seuil_heures)));
        let jours = p.jours_restants(ctx.aujourdhui);
        if anniv.is_some() && jours > 0 {
            lignes.push(("Avant le réexamen".into(), format!("{} j", jours)));
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
        let colonnes = tableau_cumul(ctx, cle, &mois, &engs)?;
        let rangee = |quoi: &'static str, valeur: &dyn Fn(&droit::cumul::Colonne) -> String| Rangee { quoi, valeurs: colonnes.iter().map(valeur).collect() };
        let heures = |n: i64| format!("{} h", n);
        let tableau = TableauDesDroits {
            titres: colonnes.iter().map(|x| format!("{} {}", x.titre, x.sous_titre).trim().to_string()).collect(),
            lignes: vec![
                rangee("Cachets", &|x| x.cachets.to_string()),
                rangee("Heures déclarées", &|x| heures(x.heures)),
                rangee("Total d'heures", &|x| heures(x.heures_equivalentes)),
                rangee("Salaire brut", &|x| socle::texte::euros(x.brut)),
            ],
        };
        let nom = ctx.salaries.get(cle).map(|f| crate::routes::nom_a_l_ecran(f)).unwrap_or_else(|| cle.to_string());
        salaries.push(DroitsDUnSalarie { cle: cle.to_string(), nom, fenetre: p.libelle_fenetre.clone(), lignes, phrase, notes, colonnes: tableau });
    }
    let phrase = if salaries.is_empty() {
        "Aucun engagement d'intermittent n'est encore validé ni importé."
    } else {
        "Ce compte ne porte que les engagements de cette compagnie : le relevé de France Travail seul fait foi, et le seuil se compte sur tous les employeurs. Ces totaux sont un plancher."
    };
    let phrases = salaries.iter().map(|s| format!("{} ({}) : {}", s.nom, s.fenetre, s.phrase)).collect();
    Ok(Droits { phrase, mois, phrases, salaries })
}
