//! Les lectures que les ecrans partagent : `etat` (tout le JSON que la page lit en s'ouvrant -- la configuration
//! verifiee, l'historique des mois, les listes ou l'on choisit, les referentiels livres avec le logiciel), `configuration`,
//! `maj-etat` et `courriels-fictifs`.
//!
//! AUCUN CALCUL DE PAIE, et aucun secret lu : la boite courriel se dit par un
//! booleen (l'adresse posee et le secret present dans l'environnement ou le
//! coffre, jamais sa valeur), la compagnie fictive par son marqueur.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::etapes::Moteur;
use crate::compagnie::etat_dsn::Etat;
use super::lire_texte;
use declarations::champs;
use droit::catalogue::{self, Rubrique};
use droit::contexte::Contexte;
use droit::emplois::{self};
use droit::natures::{self, Nature};
use droit::permanents;
use engagement::{self};
use socle::echec::{erreur, Echec};
use socle::fichiers::{dossier_du_code, vrai};
use socle::valeur::{nom_type, valeur_en_texte};
use crate::routes::catalogue::{Ecran, Place};



type R<T> = Result<T, Echec>;

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// CE QUE LA CARTE D'UN ENGAGEMENT DIT DE SES CASES (`aides` de la route `etat`) : l'aide de chaque case, qu'elle montre
/// au survol ou sous la case ; la page n'en ecrit aucune. Les articles cites sont ceux du code du travail, sauf mention.
const AIDES: &[(&str, &str)] = &[
    ("heures_par_mois", "La durée du travail du contrat, en heures par mois (S21.G00.40.013)."),
    ("net", "Ce que le salarié touche sur son compte."),
    ("cout", "Le brut et les cotisations patronales : ce que l'employeur sort."),
    ("viser", "Écrire dans une case : les taux se recalculent pour y tomber."),
    ("metier", "Le métier de ce contrat, s'il diffère de la fiche."),
    ("recours", "Pourquoi ce CDD est conclu : la DSN le déclare (motif de recours)."),
    ("periode_essai_jours", "La durée de la période d'essai que le contrat prévoit ; vide : aucune."),
    ("personne_remplacee", "Le contrat d'un remplacement nomme la personne remplacée et sa qualification (L. 1242-12, 1°)."),
    ("repartition_horaire", "À temps partiel, le contrat dit la répartition des heures entre les jours de la semaine ou les semaines du mois (L. 3123-6) : par exemple, lundi, mardi et jeudi, de 9 h à 17 h."),
    ("clauses_particulieres", "À la place des clauses particulières de la compagnie (Configuration › Modèles de contrat) ; vide : celles de la compagnie."),
    ("engagement_licenciement", "La convocation à l'entretien préalable (L1232-2 ; L1332-2 pour une faute)."),
    ("signature_convention", "La date de signature de la convention de rupture (L1237-13)."),
    ("dernier_paye", "Le dernier jour payé au salaire habituel : la fin du contrat, sauf préavis non effectué."),
    ("en_droit_a_la_retraite", "CGI, art. 80 duodecies, 6° : l'indemnité n'est imposable que si le salarié est en droit de bénéficier d'une pension de retraite d'un régime obligatoire."),
    ("preavis_non_effectue", "L1234-5 : le préavis dont l'employeur dispense le salarié est payé en indemnité compensatrice."),
    ("preavis_mois", "La durée du préavis, en mois ; vide : celle de la loi et de la convention."),
    ("precarite_due", "L1243-10 : deux cas ne se lisent pas dans le contrat, l'étudiant pendant ses vacances et le salarié qui refuse un CDI ; dire lequel."),
    ("date_paiement", "Le jour du virement de ce contrat, s'il diffère de celui de la configuration."),
    ("lieu", "Le lieu que le contrat imprime quand une ligne de paie n'en dit pas."),
    ("taux_cachet", "Le taux du cachet de ce contrat quand aucune ligne de cachet ne le dit."),
    ("jours", "Les jours travaillés que la DSN déclare (sinon les jours cochés)."),
    ("quotite_reference", "La durée de référence de l'entreprise pour ce contrat (S21.G00.40.012)."),
    ("indemnite_licenciement", "Le montant retenu, entre l'indemnité légale et la conventionnelle."),
    ("faute_grave", "Une fin de période d'essai pour faute grave : pas d'indemnité du délai de prévenance (L1221-25)."),
    ("bornes_au_mois", "Un contrat payé au mois n'a pas de jour coché : ses bornes se disent ici. La date de fin d'un CDD est celle qui déclenche le signalement de fin de contrat (nature 07) : son délai et sa date butoir se lisent sous « Fin de contrat »."),
    ("bornes_au_calendrier", "Le contrat va du premier au dernier jour coché, planning compris, quand on a coché des jours dans les mois suivants depuis le calendrier d'une ligne. Ses cachets suivants se paient dans le mois suivant (le mois venu, le contrat y est repris, ses jours cochés) ; la DPAE et le contrat imprimé ne partent qu'avec ce mois-ci, le certificat Congés Spectacles à la fin du contrat."),
    ("fin_signalee", "Ce que le signalement de fin de contrat (nature 07) déclarera."),
    ("fin_derogatoire", "Elle part dans la DSN du mois, par le circuit dérogatoire : rien à signaler."),
    ("numero_contrat", "Attribué à la production, il ne change plus."),
    ("numero", "Le numéro sous lequel votre ancien logiciel a déjà déclaré ce contrat en DSN ; vide : le logiciel en donne un."),
    ("debut_au_mois", "Sa date de début, celle du contrat signé."),
    ("bornes_jouees", "Du premier au dernier jour joué."),
    ("plusieurs_mois", "Les cachets des mois suivants se paieront dans le fichier de leur mois (« reprendre un contrat en cours » les retrouvera cochés) ; ce mois-ci partent le contrat et la DPAE pour le contrat entier."),
    ("imprevu_valide", "L'engagement est validé : cliquer sur sa carte pour l'ouvrir, retirer ces jours, puis « Modifier l'engagement »."),
    ("jour_hors_ligne", "Ce jour n'est sur aucune ligne ouverte de cette carte : le décocher au calendrier."),
    ("a_dire_avant_signaler", "À dire dans sa carte, sous « Contrat », avant de signaler :"),
    ("absence", "Saisie une fois : chaque mois qu'elle traverse la déclare et, si elle n'est pas payée, en retire le salaire."),
    ("jours_deja_indemnises", "Les jours d'arrêt déjà indemnisés dans les douze mois : ils réduisent la durée du maintien."),
    ("indemnites_journalieres", "Le montant d'un jour, tel que l'écrit le relevé de la CPAM ou son bordereau de paiement : brut, avant CSG et CRDS, puis net. Le maintien de salaire s'en déduit."),
    ("jours_payes", "Des jours sont payés pendant l'imprévu : cocher ceux à retirer de l'engagement."),
    ("signalement_dsn", "Le logiciel écrit le fichier du signalement ; le dépôt, sur net-entreprises, est l'étape suivante. La DSN du mois le répétera : l'une ne remplace pas l'autre."),
    ("deja_signale", "Un nouveau signalement ne remplace rien : pour corriger ce qui est parti, un annule-et-remplace vise l'identifiant rendu par le point de dépôt (sur le certificat de conformité)."),
    ("signalement_fin", "Il va à France Travail, qui en tire l'attestation employeur : c'est elle qui ouvre les droits du salarié. La DSN du mois ne le remplace pas."),
    ("identifiant_remplace", "JJMMAAAA suivi du rang de l'envoi, lu sur son certificat de conformité ; vide pour un signalement normal."),
    ("reference_faite", "L'accusé de réception, le numéro du recommandé, s'il y en a un."),
    ("hors_dsn", "Elle ne part pas avec la DSN."),
    ("date_accident", "La date de l'accident, ou de sa première constatation."),
    ("debut_arret", "Le premier jour que l'avis prescrit ; prescrit un jour travaillé, ce jour-là, aussi le dernier jour travaillé."),
    ("prolongation", "La nouvelle fin prescrite, celle de l'avis de prolongation."),
    ("note_imprevu", "Une note pour vous : elle ne part nulle part."),
    ("cloture", "Les saisies et les sorties de l'année ne se modifieront plus (rouvrir reste possible, depuis l'Historique). Tout ce qu'il faut garder (saisies, bulletins, contrats signés, DPAE, DSN, certificats, retours des organismes, journal des dépôts, fiches, export compta) est réuni dans l'archive de l'exercice."),
    ("cloture_refus", "La clôture vient après la dernière DSN de l'année : elle est refusée si un mois avec des paies n'a pas de DSN acceptée au journal."),
    ("cloture_forcer", "Si une DSN manque au journal (déposée ailleurs, par exemple)."),
    ("fiche_cle", "La clé de la fiche se fabrique à l'enregistrement."),
    ("signature_lourde", "Trop lourde : une image collée pèse tout son poids, préférez une image en ligne."),
    ("urssaf_changer", "D'après l'adresse de l'entreprise, vos déclarations vont à l'Urssaf de son code postal : c'est elle qui reçoit normalement DPAE et DSN. Une Urssaf qui ne vous gère pas peut faire rejeter vos déclarations. Voulez-vous vraiment la changer ?"),
    ("historique_fiche", "Les valeurs remplacées, et jusqu'à quand elles ont servi. Elles restent pour les déclarations déjà envoyées."),
    ("non_enregistres", "Vos changements ne sont pas encore enregistrés."),
    ("dater", "Ce que vous changez vaut à partir d'une date : la voici proposée, changez-la si besoin, puis enregistrez."),
    ("dater_avant", "Avant cette date, la fiche garde ce qu'elle disait."),
    ("rien_enregistre", "Rien n'est enregistré : vos saisies restent dans la fiche, à enregistrer quand vous voulez."),
    ("importes", "Engagements de l'ancien logiciel : rien ne se calcule ici."),
    ("apercu_vide", "Ajouter un engagement, une ligne de paie et ses jours joués."),
    ("apercu_vide_permanents", "Ajouter un engagement, une ligne de paie et ses jours joués, ou, pour un permanent, son salaire mensuel."),
    ("sans_cachet", "Pas de bulletin, la DSN porte le contrat."),
    ("net_attend", "Le net s'affiche dès que l'engagement est complet."),
    ("qui_d_abord", "Choisir d'abord qui est payé."),
    ("lieu_jours", "Le lieu et les jours se posent sur chaque ligne de paie."),
    ("importe_manque", "Un clic sur la carte montre ses cases, et sous chacune pourquoi."),
    ("importe_dates", "Début et fin du contrat : les jours exacts ne sont pas lus."),
    ("importe_brut", "Le brut : le net à payer n'est pas lu."),
    ("a_completer", "Un engagement au moins reste à compléter avant de le valider : sa carte dit ce qui manque."),
    ("piece_absente", "N'est plus dans le dossier du mois."),
    ("qui_engage", "Choisir qui est engagé."),
    ("engagement_retire", "Engagement retiré."),
    ("dpae_cochees", "Cochées d'avance : les embauches sans DPAE, ou dont la DPAE a été refusée. Une DPAE déjà passée ne se redéclare pas : l'Urssaf recevrait deux déclarations pour un seul contrat."),
    ("dpae_heure", "Heure d'embauche déclarée : 08h00. Toutes les embauches cochées partent ensemble, dans un seul dépôt. D'abord le test, qui ne déclare rien ; puis, une fois le test revenu conforme, l'envoi réel."),
    ("revue_spectacles", "Décochés, ils s'archivent : ils restent lisibles et se réactivent d'un clic (Configuration › Spectacles). Ceux qui continuent gardent leur numéro d'objet."),
    ("mot_de_passe", "Le mot de passe du logiciel ne sert qu'à prouver que quelqu'un est là. Les mots de passe des organismes et de la boîte restent dans le coffre."),
    ("import_dossier", "Vide : le dossier d'import de la compagnie."),
    ("import_relire", "Un fichier ajouté depuis, un salarié ou une année qui manquait."),
    ("import_relire_note", "Les fiches qui existent déjà ne sont pas touchées ; ce que les DSN ou les bulletins disaient autrement est seulement signalé. Ce qui est déjà repris ne se double pas."),
    ("import_relire_intro", "Mettez dans ce dossier ce qui manquait, puis « Aperçu ». Rien n'est écrit avant « Reprendre »."),
    ("import_intro", "Mettez dans le dossier ci-dessous tout ce que votre ancien logiciel vous a laissé, puis « Importer » : PuppetPaie crée la fiche de la compagnie, ses salariés, ses spectacles et leurs contrats avec ce que les pièces disent."),
    ("import_natures", "Un bulletin de paie ne dit pas si le contrat est un CDI ou un CDD. Dites-le pour chacun : la DSN et les documents de fin de contrat en dépendent."),
    ("import_numero", "Le numéro du contrat : la rubrique S21.G00.40.009 de sa dernière DSN."),
    ("import_quotite", "La durée du travail, en heures par mois : la rubrique S21.G00.40.013 de la DSN."),
    ("import_ecarts", "Le bulletin fait foi : c'est son montant qui s'écrit, l'écart reste noté ici."),
    ("import_copies", "Les fichiers lus sont copiés dans l'historique de la compagnie ; vous pouvez vider le dossier d'import."),
    ("import_rien", "Rien à reprendre : tout est déjà là."),
    ("import_avant_clic", "Rien n'est écrit avant ce clic."),
];

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Etat,
        geste: "Dire tout ce que la page lit en s'ouvrant : configuration, Historique, listes.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "configuration (les fiches vérifiées), historique (les mois), les listes où l'on choisit, les référentiels, auteur (phrase, nom, adresse)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Configuration,
        geste: "Dire les fiches de la compagnie, vérifiées.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "entreprise, salaries, spectacles, leurs anomalies, l'identité, la description des cases",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MajEtat,
        geste: "Dire la version du logiciel.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "la version installée et ce que la mise à jour en sait, phrase",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Fiches, Affiche::new("Dire la version")),
    },
    Fiche {
        route: Route::CourrielsFictifs,
        geste: "Dire les courriels joués d'une compagnie fictive.",
        charge: &[Champ { nom: "apres", libelle: Affiche::new("Depuis"), forme: Forme::Texte, sens: "seulement ceux d'après cet instant", avance: true }],
        exemple: r#"{}"#,
        rend: "les courriels joués (à qui, quoi, quand)",
        refus: &[Refus { classe: "ErreurOperation", corriger: "rien : seulement dans une compagnie fictive" }],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::Etat => {
            let super::SansCharge {} = super::lire_charge(charge)?;
            super::en_json(etat(ctx)?)
        }
        // La part « configuration » de l'etat, pour les routes qui la rendent apres un enregistrement.
        Route::Configuration => {
            let super::SansCharge {} = super::lire_charge(charge)?;
            super::en_json(configuration_dite(ctx)?)
        }
        Route::MajEtat => {
            let super::SansCharge {} = super::lire_charge(charge)?;
            super::en_json(version()?)
        }
        Route::CourrielsFictifs => super::en_json(courriels_fictifs(ctx, &super::lire_charge(charge)?)?),
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ les outils

/// `js.brut` d'un dictionnaire de fiche : sans les cles `_...`, a tous les
/// niveaux.
fn brut(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, x)| (k.clone(), brut(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(brut).collect()),
        autre => autre.clone(),
    }
}

/// Un dictionnaire, sinon `AttributeError`.
fn objet<'a>(v: &'a Value, _qui: &str) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(v))))
}

/// Un refus garde la classe que
/// la lecture rencontre (`FileNotFoundError`, `UnicodeDecodeError`, `JSONDecodeError`).
fn lire_objet(p: &Path) -> R<Value> {
    let octets = std::fs::read(p).map_err(|e| {
        let classe = match e.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            _ => "OSError",
        };
        erreur(classe, socle::echec::du_disque(&p, &e))
    })?;
    let texte = String::from_utf8(octets).map_err(|e| erreur("UnicodeDecodeError", socle::echec::du_disque(&p, &e)))?;
    serde_json::from_str(&texte).map_err(|e| erreur("JSONDecodeError", e.to_string()))
}

// ------------------------------------------------------------ la configuration

/// La pastille d'une fiche, et la raison de ce qui est a corriger ou a verifier, qui se lit au survol.
#[derive(Debug, Clone, serde::Serialize)]
struct PastilleDeFiche {
    #[serde(flatten)]
    pastille: super::Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// UNE FICHE TELLE QUE L'ECRAN LA MONTRE (entreprise, salarie, spectacle) : ce que sa ligne et son en-tete disent, ses
/// cases comblees (`donnees`, par le nom que `champs-base.json` leur donne, avec leur `historique`), ce qu'elle declare,
/// ses anomalies, sa pastille, et les cases que la page en montre (`fiches::montres`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct VueFiche {
    quoi_dit: String,
    geste_dit: &'static str,
    matricule_dit: Option<String>,
    titre_dit: String,
    erreurs_titre: Option<String>,
    erreurs_note: &'static str,
    nom: String,
    cle: Option<String>,
    donnees: Value,
    declare: Value,
    anomalies: Vec<Value>,
    pastille: PastilleDeFiche,
    sans_titre: bool,
    montres: Vec<String>,
}

impl VueFiche {
    /// Ses erreurs (gravite « erreur »), celles que le bandeau compte.
    fn erreurs(&self) -> usize {
        self.anomalies.iter().filter(|a| a["gravite"] == "erreur").count()
    }
}

/// La fiche telle que l'ecran la montre. `donnees` est COMBLEE : une fiche d'avant ne porte que la forme declaree, et
/// l'ecran doit proposer une forme lisible de depart. Rien n'est ecrit pour autant. `montres` : les cases que la page en
/// montre (`fiches::montres`, que la fiche de l'entreprise `ent` commande).
fn fiche_verifiee(ctx: &Contexte, nom: &str, cle: Option<&str>, donnees: &Value, ent: &Value) -> R<VueFiche> {
    let d = objet(donnees, nom)?;
    let anomalies = champs::verifier_fiche(nom, d)?;
    let comble = champs::combler_affichage(nom, d)?;
    let declare = champs::apercu_derive(nom, &comble)?;
    // Sa pastille : archivee (une information), ses erreurs (a corriger, la compagnie le peut), ce qui est a verifier,
    // ou complete.
    let erreurs = anomalies.iter().filter(|a| a["gravite"] == "erreur").count();
    let a_verifier = anomalies.len() - erreurs;
    let pluriel = |n: usize| if n > 1 { "s" } else { "" };
    // Ce qui est a corriger ou a verifier se nomme (le libelle de la case), et sa raison se lit au survol (`detail`).
    let nommer = |grave: bool| -> R<(String, String)> {
        let siennes: Vec<&Value> = anomalies.iter().filter(|a| (a["gravite"] == "erreur") == grave).collect();
        let mut cases: Vec<String> = Vec::new();
        for a in &siennes {
            let cle = valeur_en_texte(&a["champ"]);
            let libelle = champs::champ(nom, &cle)?.map(|c| c.libelle).filter(|l| !l.is_empty()).unwrap_or(cle);
            let libelle = libelle.chars().next().map(|p| p.to_lowercase().chain(libelle.chars().skip(1)).collect()).unwrap_or_default();
            if !cases.contains(&libelle) {
                cases.push(libelle);
            }
        }
        let detail = siennes.iter().map(|a| valeur_en_texte(&a["message"])).collect::<Vec<_>>().join(" ");
        Ok((cases.join(", "), detail))
    };
    let pastille = if comble.get("actif") == Some(&Value::Bool(false)) {
        PastilleDeFiche { pastille: super::Pastille::de("information", "archivé"), detail: None }
    } else if erreurs > 0 {
        let (cases, detail) = nommer(true)?;
        PastilleDeFiche { pastille: super::Pastille::de("a_faire", &format!("{} erreur{} : {}", erreurs, pluriel(erreurs), cases)), detail: Some(detail) }
    } else if a_verifier > 0 {
        let (cases, detail) = nommer(false)?;
        PastilleDeFiche { pastille: super::Pastille::de("a_faire", &format!("à vérifier : {}", cases)), detail: Some(detail) }
    } else {
        PastilleDeFiche { pastille: super::Pastille::de("fait", "complet"), detail: None }
    };
    // Un spectacle dont l'import n'a lu que le numero d'objet porte le titre « A COMPLETER » : la page le range a part.
    let sans_titre = nom == "spectacle"
        && comble.get("titre").map(valeur_en_texte).is_some_and(|t| t.trim().eq_ignore_ascii_case(crate::import::completer::A_COMPLETER));
    // Ce que l'en-tete de la fiche dit : le titre d'un spectacle, et les champs a corriger.
    let titre_dit = match cle {
        None => "Nouveau spectacle".to_string(),
        Some(_) if sans_titre => "Spectacle à nommer".to_string(),
        Some(c) => comble.get("titre").filter(|t| vrai(t)).map(valeur_en_texte).unwrap_or_else(|| c.to_string()),
    };
    let erreurs_titre = match erreurs {
        0 => None,
        1 => Some("Un champ à corriger".to_string()),
        n => Some(format!("{} champs à corriger", n)),
    };
    let erreurs_note = if erreurs > 1 { "marqués ci-dessous" } else { "marqué ci-dessous" };
    // Ce que la ligne de la fiche dit, dans la liste : ce qui l'identifie (le metier et le matricule d'un salarie, le
    // numero d'objet et le code analytique d'un spectacle), et le geste qui l'archive ou la reactive.
    let lu = |k: &str| comble.get(k).filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default();
    let quoi = match nom {
        "salarie" => [if lu("emploi_affichage").is_empty() { lu("emploi") } else { lu("emploi_affichage") }, if lu("code_salarie").is_empty() { String::new() } else { format!("matricule {}", lu("code_salarie")) }],
        _ => [lu("numero_objet"), if lu("code_analytique").is_empty() { String::new() } else { format!("analytique {}", lu("code_analytique")) }],
    };
    let quoi_dit = quoi.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
    let geste_dit = if comble.get("actif") == Some(&Value::Bool(false)) { "Réactiver" } else { "Archiver" };
    let matricule_dit = (nom == "salarie" && !lu("code_salarie").is_empty()).then(|| format!("matricule {}", lu("code_salarie")));
    let donnees = brut(&Value::Object(comble));
    let montres = super::fiches::montres(ctx, nom, &donnees, ent)?;
    Ok(VueFiche {
        quoi_dit,
        geste_dit,
        matricule_dit,
        titre_dit,
        erreurs_titre,
        erreurs_note,
        nom: nom.to_string(),
        cle: cle.map(str::to_string),
        donnees,
        declare,
        anomalies,
        pastille,
        sans_titre,
        montres,
    })
}


/// Une valeur en texte, vide si elle manque ou est vide.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// LES AUTRES ETABLISSEMENTS OUVERTS tels que la fiche les dit en dernier (ceux qui ouvrent plus tard compris) : la liste
/// datee (`etablissements::ecrire`) se montre et se saisit ; un etablissement ferme n'y est plus.
pub(crate) fn etablissements_dits(ctx: &Contexte) -> Vec<Value> {
    // La route rend la fiche telle que la page la lit.
    ctx.etablissements_dits().iter().map(|e| serde_json::to_value(e).unwrap_or(Value::Null)).collect()
}

/// Un etablissement qu'un engagement choisit : son NIC (vide : le siege), son nom, son ouverture et son dernier jour s'il
/// est ferme, et ce que l'option en dit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ChoixEtablissement {
    periode_dit: Option<String>,
    nic: String,
    nom: String,
    ouverture: Option<String>,
    fermeture: Option<String>,
}

/// CE QU'UN ENGAGEMENT CHOISIT quand la compagnie a d'autres etablissements que le siege : le siege (`nic` vide) puis
/// chacun, nomme par son nom et sa commune, avec son ouverture, et son dernier jour s'il est ferme (un engagement passe
/// le garde). `None` : un seul etablissement, rien a choisir.
pub(crate) fn choix_d_etablissement(ctx: &Contexte) -> Option<Vec<ChoixEtablissement>> {
    let principal = ctx.entreprise.nic_principal();
    let autres: Vec<(Value, Option<socle::date::Jour>)> = droit::contexte::histoire_des_etablissements(ctx.entreprise_brute())
        .unwrap_or_default()
        .into_iter()
        .filter(|(e, _)| texte_ou_vide(e.get("nic")) != principal)
        .collect();
    if autres.is_empty() {
        return None;
    }
    let commune = |f: &Value| texte_ou_vide(f.get("commune_affichage").filter(|v| vrai(v)).or(f.get("commune")));
    let siege = ctx.entreprise.commune_affichage.remplie().unwrap_or(ctx.entreprise.commune.texte()).to_string();
    let nom_du_siege = format!("Le siège{}", Some(siege).filter(|c| !c.is_empty()).map(|c| format!(", {}", c)).unwrap_or_default());
    let mut out = vec![ChoixEtablissement { periode_dit: None, nic: String::new(), nom: nom_du_siege, ouverture: None, fermeture: None }];
    for (e, fermeture) in autres {
        let nom = [texte_ou_vide(e.get("libelle")), commune(&e)].into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(", ");
        // Son ouverture, et son dernier jour s'il est ferme, tels que l'option les dit.
        let ouverture = Some(texte_ou_vide(e.get("ouverture"))).filter(|o| !o.is_empty());
        let periode_dit = ouverture.as_ref().map(|o| socle::date::Jour::lire(o).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| o.clone())).map(|o| match &fermeture {
            Some(f) => format!("ouvert le {}, jusqu'au {}", o, f.jj_mm_aaaa()),
            None => format!("ouvert le {}", o),
        });
        let nic = texte_ou_vide(e.get("nic"));
        out.push(ChoixEtablissement { periode_dit, nom: if nom.is_empty() { nic.clone() } else { nom }, nic, ouverture, fermeture: fermeture.map(|j| j.iso()) });
    }
    Some(out)
}

/// Les lignes des autres etablissements demandent-elles leur versement mobilite ? Oui quand le siege en paie un : le
/// controle des etablissements l'exige alors de chacun (`droit::contexte::controler_etablissements`).
fn versement_mobilite_des_etablissements(ctx: &Contexte) -> bool {
    ctx.bareme_fondu(ctx.aujourdhui.an).ok().and_then(|b| b.get("versement_mobilite").cloned()).is_some_and(|v| v.is_object())
}

/// `salaries_au_mois` : les salaries qui ont, ou ont eu, un contrat paye au mois. Lu dans les engagements : la nature se
/// dit sur l'engagement (ou sur un de ses mois), pas sur la fiche. L'ecran n'ouvre le bloc « Salarie au mois » qu'a eux.
fn salaries_au_mois(ctx: &Contexte) -> R<Vec<String>> {
    let mensuelles: BTreeSet<String> = natures::natures().map_err(plain)?.iter().filter(|n| n.mensuelle()).map(|n| n.cle.clone()).collect();
    let mut trouves: BTreeSet<String> = BTreeSet::new();
    // Un fichier illisible est ignore ; les autres se lisent.
    let fichiers = engagement::stock::fichiers(&ctx.racine);
    let lus = fichiers.iter().filter_map(|f| engagement::stock::lire_fichier(&ctx.racine, f).ok().flatten());
    for e in lus.flat_map(|m| m.engagements) {
        // La nature du premier mois, et chaque nature qu'un avenant lui donne ensuite.
        let s = &e.saisie;
        let natures = std::iter::once(&s.carte.nature).chain(s.avenants.iter().map(|a| &a.termes.nature));
        if !natures.flatten().any(|n| mensuelles.contains(n)) {
            continue;
        }
        if let Some(c) = s.carte.cle(ctx) {
            trouves.insert(c.to_string());
        }
    }
    Ok(trouves.into_iter().collect())
}

/// La racine des donnees du poste (`contexte.RACINE`) : `PUPPETPAIE_DONNEES`,
/// sinon le dossier `prive/` voisin du code, sinon le code lui-meme. LA LIGNE DE COMMANDE SEULEMENT (`moteur COMMANDE`,
/// `moteur maj --publier`) : une route recoit sa compagnie (`o["racine"]`, `Contexte::racine`) et ne lit jamais cette
/// variable -- la fenetre sert ses compagnies dans un seul processus.
pub(crate) fn racine_du_poste() -> PathBuf {
    if let Some(e) = std::env::var_os("PUPPETPAIE_DONNEES").filter(|e| !e.is_empty()) {
        return PathBuf::from(e);
    }
    let logiciel = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    let voisin = logiciel.parent().map(|p| p.join("prive")).unwrap_or_default();
    if voisin.join(socle::fichiers::CONFIG).is_dir() {
        voisin
    } else {
        logiciel
    }
}

/// `DOCUMENTS_IDENTITE` : ou va chaque image.
fn documents_identite(quoi: &str) -> &'static [&'static str] {
    match quoi {
        "signature" => &[
            "les contrats d'engagement", "l'AEM en PDF", "l'attestation employeur (AER)",
            "le certificat Congés Spectacles",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
        "cachet" => &["l'attestation employeur (AER), case « Cachet de l'entreprise »"],
        "logo" => &[
            "les bulletins de paie", "les contrats d'engagement",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
        _ => &[],
    }
}

/// La personne qui signe, telle que la fiche Entreprise la nomme, en forme lisible.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Signataire {
    pub(crate) nom: String,
    pub(crate) fonction: String,
}

/// `signataire_de` : la personne qui signe, telle que la fiche Entreprise la nomme, en forme lisible.
pub(crate) fn signataire_de(ent: &droit::fiche::ChampsEntreprise) -> Signataire {
    let s = ent.signataire.dite().cloned().unwrap_or_default();
    let lisible = |affichage: &droit::fiche::Case<String>, brut: &droit::fiche::Case<String>| -> String {
        match affichage.remplie() {
            Some(v) => v.to_string(),
            None => champs::titre(brut.texte()),
        }
    };
    let (prenom, nom) = (lisible(&s.prenom_affichage, &s.prenom), lisible(&s.nom_affichage, &s.nom));
    let complet: Vec<String> = [prenom, nom].into_iter().filter(|x| !x.is_empty()).collect();
    Signataire { nom: complet.join(" "), fonction: s.fonction.texte().to_string() }
}

/// `identite/identite.json` des gabarits : pour qui la signature a ete posee, et quand (`poser-identite` l'ecrit).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistreIdentite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) signature: Option<PoseePour>,
}

/// La personne pour qui une image est posee, et le jour.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PoseePour {
    pub(crate) de: String,
    pub(crate) le: String,
}

/// Le registre des images posees, lu par sa forme ; vide s'il n'existe pas. Sa seule lecture.
pub(crate) fn registre_identite(dossier: &Path) -> R<RegistreIdentite> {
    let p = dossier.join("identite.json");
    if !p.exists() {
        return Ok(RegistreIdentite::default());
    }
    socle::forme::lire(&lire_objet(&p)?, "Le registre des images de la compagnie (gabarits/identite/identite.json) ne se lit pas.").map_err(plain)
}

/// UNE IMAGE DE L'IDENTITE (signature, cachet, logo) : posee ou non, dessinee par le logiciel (le cachet), son fichier et
/// sa forme quand elle est posee, les documents qui la portent ; la signature dit pour qui elle est posee. La route
/// `identite` y ajoute sa pastille et ce que son ecran en dit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ImageIdentite {
    pub(crate) pose: bool,
    pub(crate) automatique: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    dossier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) octets: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fichier: Option<String>,
    pub(crate) documents: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signataire: Option<Signataire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) posee_pour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) autre_personne: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pastille: Option<super::Pastille>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(crate) ecran: Option<EcranDeLImage>,
}

/// Ce que l'ecran de la signature et du tampon dit d'une image : son titre, ce qu'elle engage et ou elle va, ce que dit
/// l'image posee, et les mots de ses gestes.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EcranDeLImage {
    pub(crate) titre: String,
    pub(crate) dit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) automatique_dit: Option<&'static str>,
    pub(crate) pose_dit: Option<String>,
    pub(crate) retirer_dit: &'static str,
    pub(crate) importer_dit: &'static str,
}

/// `identite` : ce qui est pose (signature, cachet, logo), et depuis quand.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Identite {
    pub(crate) signature: ImageIdentite,
    pub(crate) cachet: ImageIdentite,
    pub(crate) logo: ImageIdentite,
}

/// `identite` : ce qui est pose (signature, cachet, logo), et depuis quand. `gabarits` est celui du poste
/// (`contexte.GABARITS`).
pub(crate) fn identite(ctx: &Contexte, gabarits: &Path) -> R<Identite> {
    let dossier = gabarits.join("identite");
    let image = |quoi: &str| -> R<ImageIdentite> {
        let pose = [".png", ".jpg", ".svg"].iter().map(|ext| dossier.join(format!("{}{}", quoi, ext))).find(|f| f.exists());
        let mut e = ImageIdentite {
            pose: pose.is_some(),
            automatique: quoi == "cachet",
            dossier: None,
            octets: None,
            format: None,
            quand: None,
            fichier: None,
            documents: documents_identite(quoi),
            signataire: None,
            posee_pour: None,
            autre_personne: None,
            pastille: None,
            ecran: None,
        };
        match pose {
            None => e.dossier = Some(dossier.display().to_string()),
            Some(f) => {
                let st = std::fs::metadata(&f).map_err(|x| erreur("OSError", socle::echec::du_disque(&f, &x)))?;
                e.octets = Some(st.len());
                e.format = Some(f.extension().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default());
                e.quand = Some(socle::paris::iso_secondes(socle::paris::mtime(&st)));
                e.fichier = Some(f.display().to_string());
            }
        }
        Ok(e)
    };
    let mut signature = image("signature")?;
    // LA SIGNATURE EST CELLE D'UNE PERSONNE : celle que la fiche Entreprise nomme sous « Qui signe ». Si l'image a ete
    // posee pour une autre, la page le dit.
    let signataire = signataire_de(&ctx.entreprise);
    let pour = if signature.pose { registre_identite(&dossier)?.signature.map(|s| s.de).unwrap_or_default() } else { String::new() };
    signature.autre_personne = Some(!pour.is_empty() && !signataire.nom.is_empty() && pour != signataire.nom);
    signature.posee_pour = Some(pour);
    signature.signataire = Some(signataire);
    Ok(Identite { signature, cachet: image("cachet")?, logo: image("logo")? })
}

impl Identite {
    /// Les trois images, dans l'ordre ou l'ecran les montre.
    pub(crate) fn images_mut(&mut self) -> [(&'static str, &mut ImageIdentite); 3] {
        [("signature", &mut self.signature), ("cachet", &mut self.cachet), ("logo", &mut self.logo)]
    }
}

/// `boite_liee` : peut-on envoyer, et les signatures reviennent-elles seules ?
/// Une seule reponse aux deux : le mot de passe de la boite est pose. Ne lit
/// jamais ce mot de passe : l'adresse (`compte`), et si un secret existe.
pub fn boite_liee(ctx: &Contexte) -> R<bool> {
    if crate::compagnie::bridage::branchement(&ctx.racine).joue() {
        return Ok(true);
    }
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = |x: crate::compagnie::coffre::Echec| Echec { classe: Some(x.classe), message: x.message };
    let adresse = match std::env::var("COURRIEL_COMPTE").ok().filter(|v| !v.is_empty()) {
        Some(a) => a,
        None => coffre.compte("courriel").map_err(vers)?,
    };
    if adresse.trim().is_empty() {
        return Ok(false);
    }
    coffre.disponible("courriel").map_err(vers)
}

/// Une demande de fiche partie et pas encore revenue : son code, a qui, quand, et ce que la page en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct FicheAttendue {
    code: String,
    adresse: String,
    prenom: String,
    quand: String,
    demande_dit: String,
}

/// Les demandes de fiche parties et pas encore revenues, les plus anciennes d'abord, comme la page les lit.
fn fiches_attendues(ctx: &Contexte) -> R<Vec<FicheAttendue>> {
    Ok(crate::routes::fiches::fiches_attendues(ctx)?
        .into_iter()
        .map(|a| {
            let jour: String = a.quand.chars().take(10).collect();
            let demande_dit = format!("demandée le {}", socle::date::Jour::lire(&jour).map(|j| j.jj_mm_aaaa()).unwrap_or(jour));
            FicheAttendue { code: a.code, adresse: a.adresse, prenom: a.prenom, quand: a.quand, demande_dit }
        })
        .collect())
}


/// La fiche telle qu'elle vaut, avec `historique` : les valeurs qu'une autre a remplacees dans la fiche datee `brute`, et
/// jusqu'a quand elles valaient (la veille du `depuis` de la suivante), avec la note du changement. La page les montre.
fn avec_historique(fiche: &Value, brute: Option<&Value>) -> R<Value> {
    let mut lignes = Vec::new();
    for (champ, _) in brute.and_then(Value::as_object).into_iter().flatten().filter(|(k, _)| !k.starts_with('_')) {
        let liste = brute.and_then(|b| b.get(champ)).and_then(Value::as_array).cloned().unwrap_or_default();
        let valeurs = droit::contexte::valeurs_datees(brute.unwrap(), champ).map_err(Echec::from)?;
        for paire in valeurs.windows(2) {
            let Some(depuis) = paire[1].0 else { continue };
            let note = liste.iter().find(|e| e.get("depuis").and_then(Value::as_str) == Some(depuis.iso().as_str())).and_then(|e| e.get("note")).cloned();
            let veille = depuis.plus_jours(-1).map_err(Echec::from)?;
            lignes.push(json!({"champ": champ, "valeur": paire[0].1, "jusqu_au": veille.iso(), "jusqu_au_dit": format!("jusqu'au {}", veille.jj_mm_aaaa()), "note": note.unwrap_or(Value::Null)}));
        }
    }
    let mut f = fiche.clone();
    if let (Some(o), false) = (f.as_object_mut(), lignes.is_empty()) {
        o.insert("historique".into(), Value::Array(lignes));
    }
    Ok(f)
}

/// Les mots de la liste des fiches d'une sorte : le bouton qui en ajoute une, la liste vide, la case des archives.
#[derive(Debug, Clone, serde::Serialize)]
struct MotsDeLaListe {
    nouvelle: String,
    aucun: String,
    archives: String,
}

/// Les mots des listes de fiches, par sorte, et le mot d'une valeur trouvee dans l'historique d'une fiche.
#[derive(Debug, Clone, serde::Serialize)]
struct EcranDesFiches {
    salarie: MotsDeLaListe,
    spectacle: MotsDeLaListe,
    ancienne: &'static str,
}

/// La description des cases de chaque fiche (`champs::description`, lue de `champs-base.json`).
#[derive(Debug, Clone, serde::Serialize)]
struct Descriptions {
    entreprise: Value,
    salarie: Value,
    spectacle: Value,
}

/// LA FICHE A FAIRE REMPLIR PAR LA PERSONNE : ce que dit l'invitation, et ce qui lui manque sans boite liee.
#[derive(Debug, Clone, serde::Serialize)]
struct Invitation {
    aide: &'static str,
    sans_boite: Option<&'static str>,
    ou: &'static str,
}

/// TOUTE LA CONFIGURATION, VERIFIEE (`configuration`, et `configuration` de `etat`), et ce que la page en tire sans un
/// appel de plus.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigurationDeLaPage {
    ecran: EcranDesFiches,
    fiches_attendues_titre: String,
    compteurs_dit: Option<String>,
    deduction_forfaitaire: String,
    numeros_sans_titre: Option<String>,
    entreprise: VueFiche,
    salaries: Vec<VueFiche>,
    spectacles: Vec<VueFiche>,
    compteurs: documents::ecrire_documents::Compteurs,
    erreurs: Option<super::Pastille>,
    description: Descriptions,
    /// CE QUE LA CASE « Emploie des permanents » COMMANDE A L'ECRAN : les champs `si_permanents` restent dans la
    /// description -- les retirer, ce serait les effacer au premier enregistrement. La page les replie.
    permanents: bool,
    /// Ce que la liste des etablissements commande.
    etablissements: Option<Vec<ChoixEtablissement>>,
    versement_mobilite_des_etablissements: bool,
    /// Parmi les salaries, ceux qui sont payes au mois.
    salaries_au_mois: Vec<String>,
    /// CE QUI EST POSE : signature et logo, avec leur date.
    identite: Identite,
    invitation: Invitation,
    boite_liee: bool,
    fiches_attendues: Vec<FicheAttendue>,
}

/// La route `configuration` : la configuration et sa phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigurationDite {
    phrase: String,
    #[serde(flatten)]
    configuration: ConfigurationDeLaPage,
}

/// La route `configuration` : celle aussi que rendent les routes qui l'ont changee (`rappels::revue`).
pub(crate) fn configuration_dite(ctx: &Contexte) -> R<ConfigurationDite> {
    let configuration = configuration(ctx)?;
    Ok(ConfigurationDite { phrase: dire_la_configuration(&configuration), configuration })
}

/// Toute la configuration, verifiee, et ce que la page en tire sans un appel de plus.
fn configuration(ctx: &Contexte) -> R<ConfigurationDeLaPage> {
    // La route rend la fiche telle que la page la lit : ses cases par leur nom.
    let ent = ctx.entreprise.en_valeur();
    let mut vue = avec_historique(&ent, Some(ctx.entreprise_brute()))?;
    if let Some(o) = vue.as_object_mut() {
        let autres = etablissements_dits(ctx);
        if autres.is_empty() {
            o.shift_remove("etablissements");
        } else {
            o.insert("etablissements".into(), Value::Array(autres));
        }
        // Leur histoire est celle de leurs ouvertures, que chacun dit.
        if let Some(Value::Array(h)) = o.get_mut("historique") {
            h.retain(|l| l.get("champ").and_then(Value::as_str) != Some("etablissements"));
        }
    }
    let entreprise = fiche_verifiee(ctx, "entreprise", None, &vue, &ent)?;
    let salaries = ctx
        .salaries
        .iter()
        .map(|(k, v)| fiche_verifiee(ctx, "salarie", Some(k.as_str()), &avec_historique(&v.champs.en_valeur(), ctx.salarie_brut(k.as_str()))?, &ent))
        .collect::<R<Vec<_>>>()?;
    let spectacles = ctx.spectacles.iter().map(|(k, v)| fiche_verifiee(ctx, "spectacle", Some(k), &avec_historique(v, ctx.spectacle_brut(k))?, &ent)).collect::<R<Vec<_>>>()?;
    // LES ERREURS DE CONFIGURATION, toutes fiches comptees, que le bandeau montre (orange) : une valeur saisie et fausse,
    // les memes que la pastille de chaque fiche. Une case vide n'en est jamais une : ce qu'un acte exige, « Pour demarrer » le dit.
    let erreurs: usize = std::iter::once(&entreprise).chain(salaries.iter()).chain(spectacles.iter()).map(VueFiche::erreurs).sum();
    let erreurs = (erreurs > 0).then(|| super::Pastille::de("a_faire", &format!("{} erreur{} de configuration", erreurs, if erreurs > 1 { "s" } else { "" })));
    let permanents = ctx.emploie_des_permanents();
    // LA DEDUCTION FORFAITAIRE SPECIFIQUE vue de la fiche d'un salarie : l'option est celle de l'employeur, dans la fiche de
    // l'entreprise ; la fiche du salarie dit si elle s'applique (`calcul::deduction_forfaitaire` decide).
    let description_entreprise = champs::description("entreprise")?;
    let option = ctx.entreprise.deduction_forfaitaire_option.code().to_string();
    let deduction = if option.is_empty() {
        "La compagnie ne l'applique pas : c'est une option de l'employeur, qui se règle dans la fiche de l'entreprise (« Avancé — cas particuliers »). Une fois l'option posée, la profession et l'accord du salarié se disent ici.".to_string()
    } else {
        let nom = description_entreprise.as_array().into_iter().flatten().find(|c| c["cle"] == "deduction_forfaitaire_option").map(|c| super::mot(&c["libelles"][&option])).filter(|n| !n.is_empty()).unwrap_or_else(|| option.clone());
        format!("La compagnie l'applique (fiche de l'entreprise) : « {} ».", nom)
    };
    // LES NUMEROS D'OBJET SANS TITRE (importes des DSN, « A COMPLETER »), archives : la liste des spectacles les groupe sous
    // cette phrase.
    let sans_titre: Vec<&Value> = ctx.spectacles.values().filter(|s| s.get("actif") == Some(&json!(false)) && super::mot(&s["titre"]).trim().eq_ignore_ascii_case("a completer")).collect();
    let ans: Vec<u32> = sans_titre.iter().filter_map(|s| super::mot(&s["numero_objet"]).get(..2).and_then(|a| a.parse::<u32>().ok())).map(|a| 2000 + a).collect();
    let numeros_sans_titre = match (sans_titre.len(), ans.iter().min(), ans.iter().max()) {
        (0, _, _) => None,
        (n, de, a) => {
            let s = if n > 1 { "s" } else { "" };
            let annees = match (de, a) {
                (Some(de), Some(a)) if de != a => format!(" de {}–{}", de, a),
                (Some(de), _) => format!(" de {}", de),
                _ => String::new(),
            };
            Some(format!("{} numéro{} sans titre, trouvé{} dans les DSN{} : les nommer si vous savez à quel spectacle ils étaient (cliquer sur la ligne).", n, s, s, annees))
        }
    };
    // Les compteurs de la derniere annee, en une ligne : ce que l'en-tete de la Paie montre.
    let compteurs = documents::ecrire_documents::compteurs(&ctx.racine)?;
    let compteurs_dit = compteurs.iter().filter(|(k, _)| k.len() == 4 && k.chars().all(|c| c.is_ascii_digit())).max_by(|a, b| a.0.cmp(b.0)).map(|(an, c)| {
        let n = |x: Option<i64>| x.map_or_else(|| "?".to_string(), |n| n.to_string());
        format!("compteurs {} — bulletin {} · CS {}", an, n(c.bulletin), n(c.certificat_cs))
    });
    let fiches_attendues = fiches_attendues(ctx)?;
    let liste = |un: &str, les: &str| MotsDeLaListe { nouvelle: format!("+ un {}", un), aucun: format!("Aucun {} actif", un), archives: format!("Afficher les {} archivés", les) };
    let boite_liee = boite_liee(ctx)?;
    Ok(ConfigurationDeLaPage {
        ecran: EcranDesFiches { salarie: liste("salarié", "salariés"), spectacle: liste("spectacle", "spectacles"), ancienne: "ancienne valeur" },
        fiches_attendues_titre: format!("{} fiche(s) en attente", fiches_attendues.len()),
        compteurs_dit,
        deduction_forfaitaire: deduction,
        numeros_sans_titre,
        entreprise,
        salaries,
        spectacles,
        compteurs,
        erreurs,
        description: Descriptions { entreprise: description_entreprise, salarie: champs::description("salarie")?, spectacle: champs::description("spectacle")? },
        permanents,
        etablissements: choix_d_etablissement(ctx),
        versement_mobilite_des_etablissements: versement_mobilite_des_etablissements(ctx),
        salaries_au_mois: if permanents { salaries_au_mois(ctx)? } else { vec![] },
        identite: identite(ctx, &ctx.gabarits())?,
        invitation: Invitation {
            aide: "Elle reçoit un courriel avec un bouton : une réponse s'ouvre avec les questions (identité, naissance, numéro de sécurité sociale, adresse, compte bancaire), elle y répond et l'envoie. Sa fiche se crée seule à l'arrivée de la réponse ; il vous restera l'emploi à choisir.",
            sans_boite: (!boite_liee).then_some("Il faut d'abord lier la boîte courriel de la compagnie (Configuration › Mots de passe)."),
            ou: "Ou remplissez la fiche vous-même ci-dessous. Le matricule se donne à l'enregistrement.",
        },
        boite_liee,
        fiches_attendues,
    })
}

// ------------------------------------------------------------- l'historique

/// `SORTES` : les sortes de documents d'un mois, par le debut de leur nom.
const SORTES: [(&str, &str); 6] =
    [("Bulletins_", "bulletins"), ("Contrats_", "contrats"), ("DPAE_", "DPAE"), ("DSN_", "DSN"), ("CertificatsCS_", "certificats CS"), ("AEM_", "AEM")];

/// `mois_connus` : les mois saisis et les mois produits, du plus recent au plus
/// ancien -- `(mois, fichiers du dossier de sortie)`.
pub(crate) fn mois_connus(racine: &Path) -> Vec<(String, Vec<String>)> {
    let mut vus: BTreeSet<String> = engagement::stock::mois_saisis(racine).unwrap_or_default().into_iter().map(String::from).collect();
    let sorties = racine.join("sorties");
    let mut dossiers: BTreeMap<String, PathBuf> = BTreeMap::new();
    if let Ok(rd) = std::fs::read_dir(&sorties) {
        for e in rd.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
            if socle::mois::Mois::lire(&nom).is_ok() && e.path().is_dir() {
                vus.insert(nom.clone());
                dossiers.insert(nom, e.path());
            }
        }
    }
    let mut out = Vec::new();
    for mois in vus.into_iter().rev() {
        let mut fichiers: Vec<String> = Vec::new();
        if let Some(d) = dossiers.get(&mois) {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    if e.path().is_file() {
                        fichiers.push(e.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            fichiers.sort();
        }
        out.push((mois, fichiers));
    }
    out
}

/// Ou en est la DSN d'un mois, telle que l'Historique la lit : l'etat lu (`compagnie::etat_dsn`), ou « en_retard » pour
/// une DSN a deposer passee l'echeance.
#[derive(Debug, Clone, Copy)]
enum DsnDuBilan {
    Lue(Etat),
    EnRetard,
}

impl serde::Serialize for DsnDuBilan {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            DsnDuBilan::Lue(e) => e.serialize(s),
            DsnDuBilan::EnRetard => s.serialize_str("en_retard"),
        }
    }
}

/// UN MOIS DE L'HISTORIQUE : le mois, s'il a des documents (`produit`), ou en est sa DSN, sa pastille, le jour du depot,
/// ses documents en mots, son echeance, s'il se corrige, s'il est tenu avant ce logiciel, ce qui reste a faire, ses
/// sortes et ses fichiers, et si son exercice est clos. Un mois dont la DSN ne se lit pas (une fiche entreprise
/// incomplete) ne dit que ses fichiers (`MoisDuBilan::nu`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct MoisDuBilan {
    mois: String,
    produit: bool,
    dsn: Option<DsnDuBilan>,
    saisi_dit: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pastille: Option<socle::ecran::Pastille>,
    depose_le: Option<String>,
    depose_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    documents_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    echeance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    echeance_depot: Option<declarations::echeance_dsn::EcheanceDite>,
    corrigeable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    importe: Option<bool>,
    a_faire: bool,
    sortes: Vec<&'static str>,
    fichiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clos: Option<bool>,
}

impl MoisDuBilan {
    /// Un mois dont la DSN ne se lit pas : ses fichiers, et rien a corriger.
    fn nu(mois: &str, fichiers: &[String]) -> MoisDuBilan {
        MoisDuBilan {
            mois: mois.to_string(),
            produit: !fichiers.is_empty(),
            dsn: None,
            saisi_dit: "saisi",
            pastille: None,
            depose_le: None,
            depose_dit: None,
            documents_dit: None,
            echeance: None,
            echeance_depot: None,
            corrigeable: false,
            importe: None,
            a_faire: true,
            sortes: Vec::new(),
            fichiers: fichiers.to_vec(),
            clos: None,
        }
    }
}

/// `bilan_des_mois` : chaque mois connu, du plus recent au plus ancien, avec ou
/// en est sa DSN (en mots), s'il se corrige, et quelles SORTES de documents il
/// a. L'etat de la DSN : « passee » (bilan OK), « refusee », « attente »,
/// « a_deposer », « en_retard » ; la meme lecture que les
/// rappels : le journal des depots, et lui seul.
fn bilan_des_mois(m: &mut Moteur, ctx: &Contexte, mois: &[(String, Vec<String>)]) -> R<Vec<MoisDuBilan>> {
    let mut out = Vec::new();
    for (nom, noms) in mois {
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, nom).map_err(|m| erreur("ValueError", m))?;
        let limite = echeance.jour;
        let en_retard = echeance.passee_au(&ctx.aujourdhui);
        // L'ETAT DE LA DSN, lu une fois (`compagnie::etat_dsn`) ; une DSN a deposer passee l'echeance est en retard.
        let importe = crate::import::mois::importe(&ctx.racine, nom)?;
        let lu = crate::compagnie::etat_dsn::du_mois(ctx, socle::mois::Mois::lire(nom)?, None)?;
        let retard = lu.etat == Etat::ADeposer && en_retard;
        let depose = lu.deposee();
        let mut sortes: Vec<&'static str> = SORTES.iter().filter(|(p, _)| noms.iter().any(|n| n.starts_with(p))).map(|(_, l)| *l).collect();
        // Un mois tenu avant : ses bulletins sont ceux de l'ancien logiciel, que l'import garde.
        if importe && !crate::import::mois::bulletins_repris(&ctx.racine, nom).is_empty() {
            sortes.insert(0, "bulletins repris");
        }
        // LES AER DE FRANCE TRAVAIL : rangees avec les documents des salaries (`sorties/<mois>/envoi/`), pas a la
        // racine du mois ; elles se nomment avec les autres sortes.
        if crate::routes::documents::un_nom_commence_et_finit(&ctx.racine.join("sorties").join(nom).join("envoi"), "AER_", ".pdf") {
            let rang = sortes.iter().position(|s| *s == "AEM").unwrap_or(sortes.len());
            sortes.insert(rang, "attestations AER");
        }
        // CE QUI RESTE A FAIRE DANS LA PAGE PAIE (Historique) : une DSN a deposer, refusee ou en
        // retard, ou une DSN passee dont la paie a change depuis, tant que l'annule-et-remplace est admis. Le reste se
        // lit dans l'Historique : un mois fini n'a plus de chemin vers la page Paie.
        let a_faire = match lu.etat {
            _ if importe => false,
            // La DSN de l'ancien logiciel, et la saisie qui ajoute au mois : ce qui reste a declarer va dans une DSN
            // suivante (`import::mois::dsn_de_l_ancien_logiciel`).
            Etat::Importee => crate::import::mois::dsn_de_l_ancien_logiciel(ctx, nom)?.is_some_and(|d| d.engagements > 0),
            Etat::Passee => m.etat_remplacement(socle::mois::Mois::lire(nom)?, &Value::Null)?.a_envoyer,
            Etat::Attente => false,
            _ => true,
        };
        // CE QUE LA PASTILLE DIT, et sa couleur (`routes::couleur_de_l_etat`) : une DSN qui attend son bilan, ou dont
        // l'ancien logiciel garde la trace, n'est rien que la compagnie ait a faire -- une information, en gris.
        let (couleur, mots) = match lu.etat {
            _ if retard => ("orange", "DSN en retard"),
            Etat::Passee => ("vert", "DSN passée"),
            Etat::Attente => ("gris", "DSN partie, bilan attendu"),
            Etat::Refusee => ("rouge", "DSN refusée"),
            Etat::Importee => ("vert", "DSN déclarée avant PuppetPaie"),
            Etat::SansTrace => ("gris", "tenu avant PuppetPaie — sa DSN est celle de l'ancien logiciel"),
            Etat::ADeposer => ("orange", "DSN à déposer"),
        };
        let quand = if depose { lu.quand.clone() } else { None };
        out.push(MoisDuBilan {
            mois: nom.clone(),
            produit: !noms.is_empty(),
            dsn: Some(if retard { DsnDuBilan::EnRetard } else { DsnDuBilan::Lue(lu.etat) }),
            saisi_dit: "saisi",
            pastille: Some(socle::ecran::Pastille { couleur, mots: mots.to_string() }),
            depose_dit: depose.then(|| format!("le {}", super::jour_en_clair(&json!(quand.clone().unwrap_or_default().chars().take(10).collect::<String>())))),
            depose_le: quand,
            documents_dit: Some(if sortes.is_empty() { "aucun document".to_string() } else { sortes.join(" · ") }),
            echeance: Some(limite.iso()),
            echeance_depot: Some(echeance.dite()),
            // Deposee, passee, et l'echeance derriere : la correction part dans
            // la DSN du mois courant.
            corrigeable: lu.partie() && en_retard && !importe,
            importe: Some(importe),
            a_faire,
            sortes,
            fichiers: noms.clone(),
            clos: Some(m.exercice_clos(nom)?),
        });
    }
    Ok(out)
}

/// Ce que l'Historique montre de chaque mois.
fn historique(m: &mut Moteur, ctx: &Contexte) -> Vec<MoisDuBilan> {
    let mut mois = mois_connus(&ctx.racine);
    // Les mois tenus avant ce logiciel sont des mois comme les autres : l'Historique les montre aussi.
    for im in crate::import::mois::mois_importes(&ctx.racine).unwrap_or_default() {
        if !mois.iter().any(|(x, _)| *x == im) {
            mois.push((im, Vec::new()));
        }
    }
    mois.sort_by(|a, b| b.0.cmp(&a.0));
    // Une fiche entreprise incomplete (effectif, echeance) ne doit pas empecher la page de s'ouvrir : on retombe sur la
    // liste nue.
    bilan_des_mois(m, ctx, &mois).unwrap_or_else(|_| mois.iter().map(|(nom, noms)| MoisDuBilan::nu(nom, noms)).collect())
}

// ----------------------------------------------------------- les referentiels

/// Ce que la liste du genre dit des cinq avantages en nature du bloc 54 (`catalogue::AVANTAGES_EN_NATURE` ;
/// `catalogue::AUTRES_BRUTS` donne leur libelle de la norme).
const GENRES_EN_CLAIR: [(&str, &str); 5] = [("02", "repas"), ("03", "logement"), ("04", "véhicule"), ("05", "NTIC"), ("06", "autres")];

/// L'aide du choix « Lieu » d'une ligne de frais.
const LIEU_AIDE: &str = "Où dort et mange le salarié : hors de la métropole, la limite exonérée change ; à l'étranger, c'est l'indemnité du pays pour la nuitée et les repas du jour.";

/// `rubriques` : le catalogue qui PAIE, dans l'ordre ou le proposer.
/// Les formes de paie dont la carte propose une ligne de la rubrique `r` (`"activite"`, `"mensuelle"`).
fn formes_de(r: &Rubrique) -> Vec<&'static str> {
    let activite = r.reservee_a() != Some("mensuelle");
    let mensuelle = engagement::se_saisit_au_mois(&r.code);
    [("activite", activite), ("mensuelle", mensuelle)].into_iter().filter(|(_, oui)| *oui).map(|(f, _)| f).collect()
}

/// Un code et son libelle, dans une liste ou l'on choisit (le genre d'un avantage, le lieu d'une ligne de frais).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Choix {
    code: String,
    libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaut: Option<bool>,
}

/// Un nombre lu du fichier, rendu entier quand il l'est (`12`, pas `12.0`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct NombreLu(f64);

impl serde::Serialize for NombreLu {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.0.fract() == 0.0 && self.0.abs() < 1e15 {
            s.serialize_i64(self.0 as i64)
        } else {
            s.serialize_f64(self.0)
        }
    }
}

/// UNE RUBRIQUE QUI PAIE, telle que la carte d'un engagement la propose : ce que le catalogue en dit, et ce que la page en
/// tire sans rien decider (ses genres, ses lieux, ses formes, ce qu'elle promet sans que rien ne le tienne, ses mots).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RubriqueDeLaPage {
    lieux: Vec<Choix>,
    lieu_aide: Option<&'static str>,
    code: String,
    libelle: String,
    unite: String,
    nature: String,
    saisie: Vec<String>,
    sens: String,
    heures_forfait: Option<NombreLu>,
    plafond: Option<String>,
    aem: Option<String>,
    dsn_type: Option<String>,
    dsn_autres_bruts: Option<String>,
    annexe: Option<String>,
    soumis: bool,
    imposable: bool,
    conges_spectacles: bool,
    compte_pour: String,
    plafond_mensuel: Option<i64>,
    plafond_annuel_heures: Option<Value>,
    origine: String,
    /// Les cartes qui la proposent en ligne (« activite », « mensuelle ») : un contrat paye au mois ne porte en ligne que
    /// ses repas et ses nuitees (`engagement::se_saisit_au_mois`), un contrat d'usage tout ce qui n'est pas reserve au mois.
    formes: Vec<&'static str>,
    /// Le statut du salarie auquel elle est reservee (« artiste »), ou rien : `statut` de `salaries` de la route etat.
    reservee_au_statut: Option<&'static str>,
    genres: Vec<Choix>,
    /// CE QUE LA RUBRIQUE PROMET SANS QUE RIEN NE LE TIENNE.
    non_appliques: Vec<&'static str>,
    source: String,
    dit: Option<String>,
    /// Les mots entre les cases de sa ligne : « quantité à taux € », ou « base € à taux % » pour un pourcentage.
    mots: [&'static str; 2],
}

pub(crate) fn rubrique_de_la_page(r: &Rubrique) -> RubriqueDeLaPage {
    let non_appliques: Vec<&'static str> = [
        ("annexe", r.annexe.as_deref()),
        ("plafond", r.plafond.as_deref()),
        ("aem", r.aem.as_deref()),
        ("dsn_type", r.dsn_type.as_deref()),
    ]
    .into_iter()
    .filter(|(_, v)| v.is_some_and(|s| !s.is_empty()))
    .map(|(c, _)| c)
    .collect();
    // Les valeurs de `genre`, quand la rubrique le demande : les cinq avantages en nature du bloc 54.
    let avec_genre = r.saisie.iter().any(|s| s == "genre") && r.dsn_autres_bruts.as_deref().is_some_and(|c| catalogue::AVANTAGES_EN_NATURE.contains(&c));
    let genres: Vec<Choix> = if avec_genre {
        GENRES_EN_CLAIR
            .iter()
            .map(|(g, libelle)| Choix { code: g.to_string(), libelle: libelle.to_string(), defaut: Some(Some(*g) == r.dsn_autres_bruts.as_deref()) })
            .collect()
    } else {
        vec![]
    };
    // LE LIEU D'UNE LIGNE DE NUITEE OU DE REPAS (`engagement::TERRITOIRE`), sous « Avancé » : la metropole d'abord (rien
    // a choisir), l'outre-mer et Monaco, puis chaque lieu de la table des missions a l'etranger (`droit::missions::lieux`).
    let choix = |code: &str, libelle: &str| Choix { code: code.to_string(), libelle: libelle.to_string(), defaut: None };
    let lieux: Vec<Choix> = if [engagement::NUITEE, engagement::REPAS].contains(&r.code.as_str()) {
        std::iter::once(choix("", "Métropole"))
            .chain(engagement::TERRITOIRES.iter().map(|(c, l)| choix(c, l)))
            .chain(droit::missions::lieux().unwrap_or_default().into_iter().map(|(c, l)| choix(&c, &l)))
            .collect()
    } else {
        vec![]
    };
    RubriqueDeLaPage {
        lieu_aide: (!lieux.is_empty()).then_some(LIEU_AIDE),
        lieux,
        code: r.code.clone(),
        libelle: r.libelle.clone(),
        unite: r.unite.clone(),
        nature: r.nature.clone(),
        saisie: r.saisie.clone(),
        sens: r.sens.clone(),
        heures_forfait: r.heures_forfait.map(NombreLu),
        plafond: r.plafond.clone(),
        aem: r.aem.clone(),
        dsn_type: r.dsn_type.clone(),
        dsn_autres_bruts: r.dsn_autres_bruts.clone(),
        annexe: r.annexe.clone(),
        soumis: r.soumis,
        imposable: r.imposable,
        conges_spectacles: r.conges_spectacles,
        compte_pour: r.compte_pour.clone(),
        plafond_mensuel: r.plafond_mensuel,
        plafond_annuel_heures: r.plafond_annuel_heures.clone(),
        origine: r.origine.clone(),
        formes: formes_de(r),
        reservee_au_statut: r.reservee_au_statut(),
        genres,
        non_appliques,
        source: r.source.clone(),
        dit: r.dit.clone(),
        mots: if r.saisie.iter().any(|s| s == "taux_majoration") { ["€ à", "%"] } else { ["à", "€"] },
    }
}

/// `faits_de_rupture` : les faits a demander a l'ecran pour CE motif, et eux
/// seuls (« si compagnie simple, interface simple »).
fn faits_de_rupture(n: &Nature, m: &str) -> R<Vec<&'static str>> {
    const SANS_NOTIFICATION: [&str; 5] = ["031", "043", "065", "083", "084"];
    const LICENCIEMENTS: [&str; 8] = ["014", "020", "087", "088", "091", "092", "095", "096"];
    let m = m.to_string();
    if m.is_empty() {
        return Ok(vec![]);
    }
    let mut faits = Vec::new();
    // La rupture conventionnelle, ou le moteur refuse sans le fait (CGI 80
    // duodecies 6°).
    let sec = permanents::rupture(Some(&m)).map_err(plain)?;
    if sec.is_some_and(|(fin, _)| fin == permanents::FinDeCdi::RuptureConventionnelle) {
        faits.push("en_droit_a_la_retraite");
    }
    // Un contrat a duree indeterminee dont le motif ouvre un preavis, et dont le preavis non effectue se paie : pas
    // celui que le texte n'indemnise pas (`preavis.indemnite_compensatrice.motifs_sans_indemnite` : 059, 039, 091).
    let sans_indemnite = matches!(permanents::indemnite_de_preavis(Some(&m)).map_err(plain)?, permanents::IndemniteDePreavis::Aucune(_));
    if n.duree_indeterminee && !sans_indemnite {
        if !permanents::sans_preavis(Some(&m)).map_err(plain)? {
            faits.push("preavis_non_effectue");
        }
    }
    // Un CDD dont l'indemnite de fin de contrat est due.
    if !n.duree_indeterminee && permanents::precarite_due(n, Some(&m)).map_err(plain)?.is_none() {
        faits.push("precarite_due");
    }
    // LES DATES ET LA TRANSACTION SELON LE MOTIF : une rupture decidee par l'un
    // des deux se NOTIFIE ; un licenciement s'ENGAGE par la convocation a
    // l'entretien prealable ; une rupture conventionnelle se date par la
    // SIGNATURE de la convention ; la TRANSACTION suit un litige.
    // Et toujours quand le signalement l'exige (CCH-12 de S21.G00.62.003, `fctu::notification_exigee`) : la force majeure
    // (083) est de la table ci-dessus, et sa date de notification est pourtant obligatoire dans un FCTU.
    if !SANS_NOTIFICATION.contains(&m.as_str()) || declarations::fctu::notification_exigee(&m).map_err(plain)? {
        faits.push("notification_rupture");
    }
    if LICENCIEMENTS.contains(&m.as_str()) {
        faits.push("engagement_licenciement");
    }
    if m == "043" {
        faits.push("signature_convention");
    }
    if LICENCIEMENTS.contains(&m.as_str()) || m == "038" || m == "058" {
        faits.push("transaction");
    }
    Ok(faits)
}

/// UNE NATURE DE CONTRAT telle que l'ecran la propose : ce que `contrats-base.json` en dit, et ce que la page en tire sans
/// rien decider (les faits a demander et le signalement, motif par motif ; ses mots).
#[derive(Debug, Clone, serde::Serialize)]
struct NatureDeLaPage {
    cle: String,
    libelle: String,
    aide: String,
    mensuelle: bool,
    duree_indeterminee: bool,
    /// Un permanent ne se rattache a AUCUN spectacle : le numero d'objet (S21.G00.40.050) est INTERDIT hors des natures 02
    /// et 10 (CCH-11).
    spectacle: bool,
    /// La fin se signale-t-elle (`Nature::signalement_fctu`) : au motif par defaut, et motif par motif -- un CDDU a son
    /// terme (031) part dans la mensuelle, rompu avant (036, 037...) il se signale. La page ne decide rien.
    signalement_fctu: bool,
    signalement_par_motif: BTreeMap<String, bool>,
    motifs_rupture: Vec<natures::MotifDeRupture>,
    faits_de_rupture: BTreeMap<String, Vec<&'static str>>,
    motif_rupture_defaut: Option<String>,
    motifs_recours: Vec<natures::MotifDeRecours>,
    motif_recours_exige: bool,
    quotite_defaut: Option<NombreLu>,
    /// Le bouton qui propose la duree d'un temps plein, et le choix vide des motifs de fin (le defaut d'une fin qui ne se
    /// signale pas : le CDDU a son terme).
    temps_plein_dit: Option<String>,
    motif_rupture_vide_dit: String,
    unite_quotite: String,
}

/// Les natures de contrat, telles que l'ecran doit les proposer. Tout vient de `contrats-base.json`. SAUF LES NATURES
/// PAYEES AU MOIS : elles ne descendent que si la fiche entreprise dit employer des permanents.
fn natures(ctx: &Contexte) -> R<Vec<NatureDeLaPage>> {
    let toutes = natures::natures().map_err(plain)?;
    let defaut = natures::defaut().map_err(plain)?;
    let permanents = ctx.emploie_des_permanents();
    let gardees: Vec<&std::sync::Arc<Nature>> = toutes.iter().filter(|n| permanents || !n.mensuelle() || n.cle == defaut).collect();
    // L'ordre est celui du fichier, le defaut en tete.
    let premier = gardees.iter().find(|n| n.cle == defaut).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(&defaut)))?;
    let mut ordre: Vec<&Nature> = vec![premier];
    ordre.extend(gardees.iter().filter(|n| n.cle != defaut).map(|n| &***n));
    let mut sortie = Vec::new();
    for n in ordre {
        let motifs = n.motifs_rupture();
        let (mut faits, mut signales) = (BTreeMap::new(), BTreeMap::new());
        for m in motifs {
            faits.insert(m.code.clone(), faits_de_rupture(n, &m.code)?);
            signales.insert(m.code.clone(), n.signalement_fctu(Some(&m.code)));
        }
        sortie.push(NatureDeLaPage {
            cle: n.cle.clone(),
            libelle: n.libelle.clone(),
            aide: n.aide.clone(),
            mensuelle: n.mensuelle(),
            duree_indeterminee: n.duree_indeterminee,
            spectacle: n.interdit("S21.G00.40.050").map_or(true, |s| s.is_empty()),
            signalement_fctu: n.signalement_fctu(None),
            signalement_par_motif: signales,
            motifs_rupture: motifs.to_vec(),
            faits_de_rupture: faits,
            motif_rupture_defaut: n.motif_rupture_par_defaut(),
            motifs_recours: n.motifs_recours().to_vec(),
            motif_recours_exige: n.motif_recours_exige(),
            quotite_defaut: n.quotite_reference_defaut().map(NombreLu),
            temps_plein_dit: n.quotite_reference_defaut().map(|q| format!("temps plein {} h", socle::texte::virgule(socle::num::dec_f64(q).unwrap_or_default()))),
            motif_rupture_vide_dit: match n.motif_rupture_par_defaut() {
                Some(d) if !n.signalement_fctu(None) => format!("— {}, à son terme —", d),
                _ => "— à choisir —".to_string(),
            },
            unite_quotite: n.unite_quotite().to_string(),
        });
    }
    Ok(sortie)
}

/// Un niveau de complementaire sante que le bareme connait : son numero, sa cotisation (en % du PMSS) pour l'assure, le
/// conjoint et chaque enfant, et ce que le choix en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct NiveauDeSante {
    niveau: String,
    assure: Option<f64>,
    conjoint: Option<f64>,
    enfant: Option<f64>,
    libelle: String,
}

/// Les niveaux de garantie que le bareme connait, pour la fiche salarie (`releve_svp_sante` de la rubrique `mutuelle` des
/// permanents). Vide quand le bareme n'en porte pas, ou ne se lit pas encore (une fiche entreprise incomplete).
fn niveaux_sante(ctx: &Contexte) -> Vec<NiveauDeSante> {
    use rust_decimal::prelude::ToPrimitive;
    let Ok(pile) = ctx.bareme_lu(&ctx.aujourdhui) else { return vec![] };
    let releve = pile.par_nature.get("permanent").and_then(|p| p.rubriques_ajoutees.iter().find(|r| r.id == "mutuelle")).and_then(|r| r.releve_svp_sante.as_ref());
    let Some(releve) = releve else { return vec![] };
    releve
        .niveaux
        .iter()
        .map(|(n, t)| NiveauDeSante {
            niveau: n.clone(),
            assure: t.assure.to_f64(),
            conjoint: t.conjoint.to_f64(),
            enfant: t.enfant.to_f64(),
            libelle: format!("niveau {} — {} % du PMSS pour l'assuré seul", n, t.assure),
        })
        .collect()
}

/// Une convention collective que le bareme sait servir : son IDCC, son libelle, si sa couche de taux est RELEVEE
/// (`applicable`), et l'option qui la propose.
#[derive(Debug, Clone, serde::Serialize)]
struct ConventionServie {
    idcc: String,
    libelle: String,
    applicable: bool,
    option_dit: String,
}

/// Les conventions collectives que le bareme sait servir, pour la fiche (`Contexte::conventions_servies`). La page ne les
/// invente pas, et elle n'en invente pas le libelle.
fn conventions(ctx: &Contexte) -> R<Vec<ConventionServie>> {
    Ok(ctx
        .conventions_servies()
        .map_err(plain)?
        .into_iter()
        .map(|(idcc, c)| {
            let libelle = if c.libelle.trim().is_empty() { idcc.clone() } else { c.libelle };
            let option_dit = format!("IDCC {} — {}{}", idcc, libelle, if c.applicable { "" } else { " (taux non relevés)" });
            ConventionServie { idcc, libelle, applicable: c.applicable, option_dit }
        })
        .collect())
}

// -------------------------------------------------------------------- l'etat

/// Un salarie des listes ou l'on choisit : sa cle, son nom, et son statut comme la paie le lit.
#[derive(Debug, Clone, serde::Serialize)]
struct SalarieDeLaListe {
    cle: String,
    libelle: String,
    statut: String,
}

/// Un spectacle des listes ou l'on choisit : sa cle, son titre et son numero d'objet, tels que sa fiche les porte.
#[derive(Debug, Clone, serde::Serialize)]
struct SpectacleDeLaListe {
    cle: String,
    titre: Value,
    numero_objet: Value,
}

/// La rubrique d'une ligne neuve, par statut du salarie.
#[derive(Debug, Clone, serde::Serialize)]
struct ParStatut {
    artiste: Option<String>,
    technicien: Option<String>,
}

/// La rubrique d'une ligne neuve sur la carte d'un contrat d'usage (`activite`) et d'un contrat paye au mois.
#[derive(Debug, Clone, serde::Serialize)]
struct RubriqueParDefaut {
    activite: ParStatut,
    mensuelle: ParStatut,
}

/// Un organisme de recouvrement que la DPAE fait choisir.
#[derive(Debug, Clone, serde::Serialize)]
struct OrganismeUrssaf {
    code: String,
    nom: String,
}

/// CE QUE LA PAGE LIT EN S'OUVRANT (`etat`) : la configuration verifiee et sa phrase, l'auteur, ce qu'une compagnie
/// fictive en dit, l'historique des mois, les listes ou l'on choisit et les referentiels livres avec le logiciel.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EtatDeLaPage {
    phrase: String,
    configuration: ConfigurationDeLaPage,
    auteur: crate::interface::auteur::Auteur,
    simulation: bool,
    coulisses: Option<Coulisses>,
    note_fictive: Option<super::Encart>,
    envoi_sans_mot_de_passe: bool,
    supprimable: bool,
    suppression: &'static str,
    demo: Option<crate::interface::suppression::MentionDemo>,
    commencer_par_l_import: bool,
    historique: Vec<MoisDuBilan>,
    salaries: Vec<SalarieDeLaListe>,
    spectacles: Vec<SpectacleDeLaListe>,
    rubriques: Vec<RubriqueDeLaPage>,
    rubrique_par_defaut: RubriqueParDefaut,
    emplois: Vec<emplois::MetierDuDeroulant>,
    familles_emplois: Vec<String>,
    emplois_autres: Vec<super::metiers::MetierAutre>,
    natures: Vec<NatureDeLaPage>,
    nature_defaut: String,
    types_preavis: Vec<droit::preavis::TypePreavis>,
    aides: Aides,
    transaction: Vec<droit::preavis::Code>,
    niveaux_sante: Vec<NiveauDeSante>,
    sante_hors_fiche: Option<String>,
    sante_aide: &'static str,
    conventions: Vec<ConventionServie>,
    organismes_urssaf: Vec<OrganismeUrssaf>,
    urssaf_deduit_dit: String,
    urssaf_deduit: String,
    pays: Vec<socle::pays::PaysDeNationalite>,
    pays_naissance: Vec<socle::pays::PaysServi>,
}

/// La route `etat` : tout ce que la page lit en s'ouvrant.
pub fn etat(ctx: &Contexte) -> R<EtatDeLaPage> {
    let mut m = Moteur::nouveau(ctx);
    let configuration = configuration(ctx)?;
    let historique = historique(&mut m, ctx);

    // Les listes ou l'on CHOISIT : les fiches courantes seulement. Une archive
    // reste dans `configuration`, ou on peut la relire et la reactiver.
    let mut cles: Vec<&str> = ctx.salaries_actifs();
    cles.sort();
    let salaries = cles
        .into_iter()
        .map(|k| {
            // Son statut, comme la paie le lit (vide, un artiste) : les rubriques `reservee_au_statut` s'y comparent.
            let statut = ctx.salaries.get(k).and_then(|f| f.statut.dite()).map_or("artiste", |s| s.as_str()).to_string();
            SalarieDeLaListe { cle: k.to_string(), libelle: ctx.nomme(k), statut }
        })
        .collect();
    let spectacles = ctx
        .spectacles_actifs()
        .into_iter()
        .map(|k| {
            let v = &ctx.spectacles[k];
            let titre = v.get("titre").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("titre")))?;
            Ok(SpectacleDeLaListe { cle: k.to_string(), titre: titre.clone(), numero_objet: v.get("numero_objet").cloned().unwrap_or(Value::Null) })
        })
        .collect::<R<Vec<_>>>()?;

    // Le catalogue qui paie, deja dans l'ordre ou le proposer.
    let cat = catalogue::catalogue(&ctx.racine, &ctx.lue_au).map_err(plain)?;
    // Une rubrique que le moteur pose (`saisie` vide) ne se propose pas.
    let rubriques: Vec<RubriqueDeLaPage> = cat.iter().filter(|r| !r.saisie.is_empty()).map(rubrique_de_la_page).collect();
    // La rubrique d'une ligne neuve, par forme de paie et par statut du salarie (sans salarie, celui d'un artiste, comme
    // la paie le lit) : le moteur la choisit, la page la pose. Sur la carte d'un contrat d'usage, la premiere rubrique de
    // salaire qu'il propose (`catalogue::par_defaut`) ; sur celle d'un contrat paye au mois, son premier frais
    // (`engagement::se_saisit_au_mois`), dans l'ordre du catalogue.
    let activite = |statut: &str| catalogue::par_defaut(&cat, "activite", statut).map(|r| r.code.clone());
    let au_mois = cat.iter().find(|r| !r.saisie.is_empty() && engagement::se_saisit_au_mois(&r.code)).map(|r| r.code.clone());
    let rubrique_par_defaut = RubriqueParDefaut {
        activite: ParStatut { artiste: activite("artiste"), technicien: activite("technicien") },
        mensuelle: ParStatut { artiste: au_mois.clone(), technicien: au_mois },
    };
    // LA FIN D'UN CONTRAT, telle qu'elle se saisit. Les types de preavis
    // SAISISSABLES seulement.
    let fctu = droit::preavis::base().map_err(plain)?;
    let types_preavis: Vec<droit::preavis::TypePreavis> = fctu.types_preavis.codes.into_iter().filter(|c| c.saisissable).collect();
    let transaction = fctu.transaction.codes;

    // Les organismes de recouvrement, pour que le code Urssaf de la DPAE se
    // CHOISISSE au lieu de se taper ; et celui que le moteur prend quand la
    // fiche n'en dit rien : l'organisme du territoire.
    let urssaf = declarations::urssaf::base().map_err(plain)?;
    let deduit = urssaf.du_code_postal(ctx.entreprise.code_postal.texte());
    let organismes_urssaf: Vec<OrganismeUrssaf> = urssaf.organismes.into_iter().map(|o| OrganismeUrssaf { code: o.code, nom: o.nom }).collect();
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
    let joue = branchement.joue();

    Ok(EtatDeLaPage {
        phrase: dire_la_configuration(&configuration),
        configuration,
        // L'auteur, au pied de la page (`interface::auteur`).
        auteur: crate::interface::auteur::auteur(),
        // UNE COMPAGNIE FICTIVE : les envois sont joues (`bridage::branchement`).
        simulation: joue,
        // Ce que la boite d'un envoi joue en dit.
        coulisses: joue.then(coulisses_fictives),
        note_fictive: joue.then(|| super::Encart::de("information", "Compagnie fictive", "Rien ne part : la réponse est là aussitôt ; en vrai, elle peut prendre quelques jours.")),
        // UN ENVOI N'A RIEN A PROUVER : joue (`depot_acte::verifier_preuve` ne demande rien a une fictive), ou refuse
        // avant toute preuve (une fictive ajoutee) ; la page ne demande alors pas le mot de passe du logiciel.
        envoi_sans_mot_de_passe: joue || branchement.refus().is_some(),
        // SUPPRIMER LA COMPAGNIE : la Configuration ne le propose que si elle se supprime -- jamais une
        // fictive livree, qui se remet a zero (`interface::suppression::genre`).
        supprimable: crate::interface::suppression::genre(&crate::compagnie::administrateur::installation_de(&ctx.racine), &ctx.racine).is_ok(),
        suppression: "Tout son dossier est effacé de ce poste : fiches, paies, documents, journaux.",
        // UNE COMPAGNIE CREEE EN DEMO PUIS GARDEE : sa mention, et les mois saisis en demo, ou la page la montre.
        demo: crate::interface::suppression::mention(&ctx.racine).map_err(plain)?,
        // UNE COMPAGNIE VENUE D'UN LOGICIEL PRECEDENT, DONT RIEN N'EST ENCORE REPRIS : la page s'ouvre sur l'import et
        // son guide (`import::guide::commencer_par_l_import`) ; ce qui manque pour demarrer se dit apres lui.
        commencer_par_l_import: crate::import::guide::commencer_par_l_import(&ctx.racine),
        historique,
        salaries,
        spectacles,
        rubriques,
        rubrique_par_defaut,
        // Le catalogue des metiers, deja dans l'ordre des familles.
        emplois: super::metiers::metiers(ctx)?,
        familles_emplois: emplois::familles().map_err(plain)?,
        // Les metiers des autres conventions : on les AJOUTE a son deroulant,
        // ils n'y sont pas d'office.
        emplois_autres: super::metiers::metiers_autres_conventions(ctx)?,
        // LES NATURES DE CONTRAT. Un engagement qui ne dit rien est un CDDU du
        // spectacle : c'est le defaut que l'ecran garde.
        natures: natures(ctx)?,
        nature_defaut: natures::defaut().map_err(plain)?,
        types_preavis,
        aides: Aides(AIDES),
        transaction,
        // Les niveaux de complementaire sante, pour la fiche d'un permanent.
        niveaux_sante: niveaux_sante(ctx),
        // Ou, quand la convention ne la fait pas choisir sur la fiche, d'ou elle vient (`bareme::sante_hors_fiche`).
        sante_hors_fiche: droit::bareme::sante_hors_fiche(&ctx.entreprise).map_err(plain)?,
        // Ce que la fiche d'un permanent dit de sa complementaire sante, sous le choix du niveau.
        sante_aide: "Obligatoire pour un salarié au mois (art. L911-7) : sans elle, le logiciel refuse de le payer plutôt que de cotiser moins que le contrat. Elle se lit sur son bulletin d'adhésion.",
        // Les conventions collectives servies, pour la fiche entreprise.
        conventions: conventions(ctx)?,
        organismes_urssaf,
        urssaf_deduit_dit: format!("— code postal ({}) —", if deduit.is_empty() { "117" } else { deduit.as_str() }),
        urssaf_deduit: if deduit.is_empty() { "117".to_string() } else { deduit },
        // LES PAYS : la liste livree, triee par nom, chacun avec la codification UE que le moteur en
        // deduit. Le pays de naissance et le pays de nationalite s'y choisissent.
        pays: socle::pays::servir().map_err(plain)?,
        // Le pays de naissance : la table « Pays » de la DSN, ou se lit S21.G00.30.015.
        pays_naissance: socle::pays::servir_naissance().map_err(plain)?,
    })
}

/// Les aides des cases (`AIDES`), rendues comme un dictionnaire dans leur ordre.
#[derive(Debug, Clone, Copy)]
struct Aides(&'static [(&'static str, &'static str)]);

impl serde::Serialize for Aides {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, a) in self.0 {
            m.serialize_entry(k, a)?;
        }
        m.end()
    }
}


/// Le contenu du fichier `VERSION`, ou `0.0.0`.
pub(crate) fn courante(code: &Path) -> R<String> {
    let p = code.join("VERSION");
    if p.exists() {
        Ok(lire_texte(&p)?.trim().to_string())
    } else {
        Ok("0.0.0".to_string())
    }
}

/// LA VERSION DU LOGICIEL (`maj-etat`) : l'installee, si l'editeur est pose, le depot d'ou l'on recoit, la version
/// prete et ce qu'elle dit (`calcul`, `exigee`), la signature, la version d'avant, et la phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Version {
    courante: String,
    editeur: bool,
    depot: String,
    prete: Option<String>,
    calcul: String,
    exigee: String,
    signee: bool,
    avant: Option<String>,
    phrase: String,
}

/// La version du logiciel, ce qui est pret, ce
/// qui est configure. Ne va voir aucun depot.
pub(crate) fn version() -> R<Version> {
    let code = dossier_du_code().parent().map(Path::to_path_buf).unwrap_or_default();
    // La mise a jour est celle de l'INSTALLATION : son reglage (`maj::reglage`), sa version prete (`maj::prete`).
    let inst = code.parent().map(Path::to_path_buf).unwrap_or_default();
    let (depot, editeur) = crate::compagnie::maj::reglage(&inst).map_err(|e| erreur("ValueError", e))?;
    let prete = crate::compagnie::maj::prete(&inst).map_err(|e| erreur("ValueError", e))?;
    let courante_v = courante(&code)?;
    let avant = if code.join(".avant").join("donnees").is_dir() { Some(courante(&code.join(".avant"))?) } else { None };
    let dite = match &prete {
        Some(p) if !p.version.is_empty() => format!(" La version {} est prête.", p.version),
        _ => String::new(),
    };
    let (prete, calcul, exigee) = match prete {
        Some(p) => (Some(p.version), p.calcul, p.exigee),
        None => (None, String::new(), String::new()),
    };
    Ok(Version {
        phrase: format!("Version installée : {}.{}", courante_v, dite),
        courante: courante_v,
        editeur,
        // Le depot d'ou l'on recoit : celui qui est pose, sinon celui du moteur (`maj::DEPOT_PAR_DEFAUT`).
        depot: crate::compagnie::maj::depot(depot),
        prete,
        calcul,
        exigee,
        // La cle publique de l'editeur est ecrite dans le moteur (`maj::CLE_PUBLIQUE`) : toujours posee.
        signee: true,
        avant,
    })
}

/// La configuration en une phrase : la compagnie, ses fiches, et ses erreurs s'il y en a.
fn dire_la_configuration(c: &ConfigurationDeLaPage) -> String {
    let nom = super::mot(&c.entreprise.donnees["nom_affichage"]);
    let erreurs = c.erreurs.as_ref().map(|p| format!(" ; {}", p.mots)).unwrap_or_default();
    format!(
        "{} : {}, {}{}.",
        nom,
        super::compte(c.salaries.len(), "salarié", "salariés"),
        super::compte(c.spectacles.len(), "spectacle", "spectacles"),
        erreurs
    )
}

/// LE CIRCUIT D'UN COURRIEL JOUE PAR UNE COMPAGNIE FICTIVE, genre par genre (`circuits` de `courriels-fictifs`) : ce que
/// recoit le destinataire, ses etapes (bulles) et ce que chacune dit (notes), et ce qui se dit en attendant la reponse.
fn circuits_fictifs() -> Circuits {
    Circuits {
        signature: Circuit {
            titre: "Ce que reçoit le salarié",
            bulles: &["Le contrat part", "Il appuie sur « Je signe »", "Le contrat signé lui revient"],
            notes: &[
                "Un courriel par salarié, le contrat en pièce jointe et un bouton « Je signe ».",
                "Le bouton ouvre sa messagerie sur cette réponse toute écrite, avec son code unique : il n'a qu'à l'envoyer. Le logiciel la relève seul, à l'ouverture et avec la veille, et la garde comme preuve.",
                "Dès la signature relevée, le contrat contresigné part tout seul : le seul envoi sans clic du logiciel, puisque le clic est celui du salarié. La pastille du contrat passe au vert.",
            ],
            attente: Some("La signature du salarié fictif arrive une vingtaine de secondes après l'envoi."),
            creee: None,
        },
        fiche: Circuit {
            titre: "Ce que reçoit la personne",
            bulles: &["La demande part", "Elle appuie sur « Remplir ma fiche »", "Sa fiche se crée"],
            notes: &[
                "Un courriel avec un bouton « Remplir ma fiche », et les questions recopiées dans le texte pour les messageries qui n'affichent pas le bouton.",
                "Le bouton ouvre sa messagerie sur les questions, une par ligne : elle écrit après chaque deux-points et envoie. Seul ce qu'elle sait lui est demandé ; l'emploi reste à l'employeur.",
                "Sa réponse est relevée comme une signature : la fiche salarié se crée avec ce qu'elle a écrit, marquée « à compléter » pour l'emploi. Une réponse illisible laisse la case vide, et le dit.",
            ],
            attente: Some("La réponse de la personne fictive arrive une vingtaine de secondes après l'envoi."),
            creee: Some("Elle est dans Configuration › Salariés, avec ce que la personne a écrit ; il reste à choisir son emploi."),
        },
        documents: Circuit {
            titre: "Ce que reçoit le salarié",
            bulles: &["Les documents partent"],
            notes: &["Un courriel par salarié, avec ses documents en pièces jointes. L'envoi est noté au journal, et un second envoi le signale avant de repartir."],
            attente: None,
            creee: None,
        },
        courriel: Circuit {
            titre: "Le courriel envoyé",
            bulles: &["Le courriel part"],
            notes: &["Voici le message tel qu'il serait parti. Dans la compagnie fictive, il est écrit dans le dossier courriels-simules/ de la compagnie, et rien ne quitte le poste."],
            attente: None,
            creee: None,
        },
        sans_bouton: "Ce courriel n'a pas de bouton.",
    }
}

/// CE QU'UNE COMPAGNIE FICTIVE NE FAIT PAS, DIT AU LIEU D'ETRE TU (`coulisses` de la route `etat`, pour une fictive) : ce
/// que fait une compagnie reelle (la sauvegarde, `compagnie::sauvegarde` ; la mise a jour, `compagnie::maj`), et l'horloge
/// de la fictive (`horloge`).
fn coulisses_fictives() -> Coulisses {
    Coulisses {
        titre: "Les coulisses de la compagnie fictive",
        sections: [
            ["La sauvegarde", "Pour une compagnie réelle, à l'ouverture puis toutes les vingt minutes quand quelque chose a changé, le dossier de la compagnie (saisies, bulletins, contrats, déclarations, retours, journal) part dans son propre Google Drive. Seul ce qui a changé repart, et rien de ce qui est effacé sur le poste ne l'est dans le Drive. Les mots de passe, eux, ne quittent jamais l'ordinateur. Le logiciel ne voit dans le Drive que ce qu'il y a lui-même créé."],
            ["Les mises à jour", "Au lancement, le logiciel regarde si l'éditeur a publié une nouvelle version. Il la télécharge en arrière-plan, vérifie qu'elle est bien signée par l'éditeur, et l'installe au démarrage suivant, sans rien demander. Une version qui change un calcul de paie le dit en une phrase ; une version marquée « exigée » (une norme DSN qui change) bloque les dépôts réels tant qu'elle n'est pas installée."],
            ["Dans la compagnie fictive", "La sauvegarde ne tourne pas : aucun compte Google n'est lié, rien ne quitte le poste. La mise à jour, elle, est celle du logiciel : elle se fait de la même façon, qu'on ouvre une compagnie fictive ou réelle. Le bandeau dit « À jour » parce que la relève des retours, elle, est jouée."],
            ["L'horloge de la compagnie fictive", "La compagnie vit sans vous. À chaque ouverture, ce qui tombait depuis votre dernier passage a été fait par un administrateur fictif, chaque geste à sa date limite : le 1er du mois, le mois s'écrit ; le jour d'une embauche, sa DPAE part et son contrat revient signé ; le 15, la DSN du mois précédent part, puis les documents aux salariés ; fin janvier, l'année passée se clôt. Avant la date limite, c'est à vous : faites la DPAE deux jours avant, ou la DSN le 14, et l'horloge ne la refera pas. Ajoutez ou retirez un contrat : à votre retour, le mois est réécrit et tout est à jour."],
        ],
    }
}

/// Le circuit d'un genre de courriel joue : ce que recoit le destinataire, ses etapes, ce qu'elles disent, et ce qui se
/// dit en attendant la reponse et une fois la fiche creee.
#[derive(Debug, Clone, serde::Serialize)]
struct Circuit {
    titre: &'static str,
    bulles: &'static [&'static str],
    notes: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    attente: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creee: Option<&'static str>,
}

/// Les circuits des courriels joues, genre par genre, et ce qu'on dit d'un courriel sans bouton.
#[derive(Debug, Clone, serde::Serialize)]
struct Circuits {
    signature: Circuit,
    fiche: Circuit,
    documents: Circuit,
    courriel: Circuit,
    sans_bouton: &'static str,
}

/// Les coulisses d'une compagnie fictive : un titre, et ses sections (titre, texte).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Coulisses {
    titre: &'static str,
    sections: [[&'static str; 2]; 4],
}

/// LES COURRIELS JOUES (`courriels-fictifs`), tels que la page les montre : sa phrase, les courriels et le dernier lu,
/// l'intro, le circuit de chaque genre et la phrase de chaque courriel.
#[derive(Debug, Clone, serde::Serialize)]
struct CourrielsFictifs {
    phrase: String,
    courriels: Vec<envois::simulation_courriels::CourrielJoue>,
    dernier: String,
    intro: String,
    circuits: Circuits,
    phrases: Vec<String>,
}

/// La route `courriels-fictifs`.
fn courriels_fictifs(ctx: &Contexte, charge: &crate::simulation_courriels::ChargeCourrielsFictifs) -> R<CourrielsFictifs> {
    let joues = crate::simulation_courriels::courriels_fictifs(ctx, charge)?;
    let phrases: Vec<String> = joues.courriels.iter().map(|m| format!("{} → {} : {}", m.de, m.a, m.sujet)).collect();
    // Ce que la fenetre d'une compagnie fictive en dit : combien sont joues, et le circuit de chaque genre.
    let n = phrases.len();
    let s = if n > 1 { "s" } else { "" };
    Ok(CourrielsFictifs {
        phrase: format!("Courriels simulés : {}.", n),
        courriels: joues.courriels,
        dernier: joues.dernier,
        intro: format!("{} courriel{} joué{} — rien n'a quitté le poste.{}", n, s, s, if n > 1 { " Chacun reçoit les siens :" } else { "" }),
        circuits: circuits_fictifs(),
        phrases,
    })
}
