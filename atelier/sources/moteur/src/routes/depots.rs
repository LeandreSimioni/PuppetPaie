//! Les routes des declarations : ecrire ou reprendre la DSN ou la DPAE du mois et dire ce
//! qu'elle declare (`depot-apercu`), la deposer (`deposer`), suivre un depot (`suivre-depot`), le bilan de son test (`bilan-repetition`).
//!
//! Les formats sont ailleurs (`dpae`, la DSN de `declarations`, la fabrique de PDF) : ce module enchaine -- charger,
//! choisir, refuser, nommer, ecrire, et passer a `envois` ce qui part. Le jour est celui du contexte
//! (`PUPPETPAIE_AUJOURDHUI`) ; l'horodatage d'un fichier partiel de DPAE est la minute de l'horloge, a l'heure de Paris.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::compagnie::depot_acte::{self, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, MANQUE, MOIS, OPERATION};
use crate::routes::catalogue::Route;
use declarations::dpae::{Chemins, Embauche, Parametres};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use envois::etats;
use envois::releve_retours;
use socle::date::en_texte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{dossier_du_code, vrai};
use socle::mois::{Graphie, Mois};
use serde::{Deserialize, Serialize};
use envois::depot_acte::DepotFait;
use envois::etats::Repetition;
use envois::ligne::Ligne;
use crate::apercu_dsn::ApercuDsn;
use super::{en_json, lire_charge, Encart, Pastille};
use crate::routes::catalogue::{Ecran, Place};

// LA DESIGNATION d'une declaration (`designee`) : la DSN ou la DPAE d'un mois, ou un signalement ecrit.
const QUOI: Champ = Champ { nom: "quoi", libelle: Affiche::new("DPAE ou DSN"), forme: Forme::Texte, sens: "dpae ou dsn", avance: false };
const ETABLISSEMENT: Champ = Champ { nom: "etablissement", libelle: Affiche::new("Établissement"), forme: Forme::Texte, sens: "le NIC de l'établissement (DSN), le principal sinon", avance: true };
const ENGAGEMENTS: Champ = Champ { nom: "engagements", libelle: Affiche::new("Embauches"), forme: Forme::Objet, sens: "seulement ces embauches (DPAE) : [{salarie, numero}]", avance: true };
const SIGNALEMENT: Champ = Champ { nom: "fichier", libelle: Affiche::new("Signalement"), forme: Forme::Texte, sens: "un signalement écrit, tel que sa route le rend, à la place du mois", avance: true };
/// L'exemple : la DPAE de novembre, que `depot-apercu` ecrit.
const DPAE_DE_L_EXEMPLE: &str = r#"{"quoi":"dpae","mois":"2026-11"}"#;
const ILLISIBLE_FICHIER: Refus = Refus { classe: "FileNotFoundError", corriger: "le signalement : le fichier que sa route a rendu" };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::DepotApercu,
        geste: "Écrire ou reprendre la DSN ou la DPAE du mois ; dire ce qu'elle déclare.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "ce qu'elle déclare (description, embauches, ou salariés, recapitulatif avec son titre et sa phrase, ordres et ordres_phrase), deja_depose, repetition (où en est son test, titre et phrase de l'encart), encarts (titre, phrase, couleur), phrase, avertissements, informations",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 300,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Deposer,
        geste: "Déposer la DSN ou la DPAE du mois : en test, ou en réel après son test.",
        charge: &[
            QUOI,
            MOIS,
            ETABLISSEMENT,
            ENGAGEMENTS,
            SIGNALEMENT,
            Champ { nom: "reel", libelle: Affiche::new("Dépôt réel"), forme: Forme::OuiNon, sens: "vrai : le dépôt réel (le mot de passe du logiciel, tapé par un humain au moment de l'acte)", avance: false },
            Champ { nom: "forcer", libelle: Affiche::new("Annule et remplace"), forme: Forme::OuiNon, sens: "vrai : l'annule-et-remplace d'une DSN déjà déposée, après l'avertissement", avance: true },
            Champ { nom: "contournement", libelle: Affiche::new("Motif d'un dépôt réel sans test"), forme: Forme::Texte, sens: "le motif écrit d'un dépôt réel sans test", avance: true },
        ],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "le dépôt : accepté ou non, le retour de l'organisme, ses suites (chaque demande de taux), phrase, avis (l'encart à montrer après la DSN réelle d'un mois qui a des salariés)",
        refus: &[
            Refus { classe: "PreuveInsuffisante", corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est parti" },
            Refus { classe: "RepetitionManquante", corriger: "déposer d'abord ce fichier en test" },
            Refus { classe: "DsnDejaDeposee", corriger: "rien, ou renvoyer avec forcer si l'annule-et-remplace est voulu" },
            ILLISIBLE_FICHIER,
            OPERATION,
        ],
        acte: Acte::Engage,
        avance: false,
        phrase: 400,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SuivreDepot,
        geste: "Après un dépôt réel, relever les retours et dire s'il est passé ou refusé.",
        charge: &[QUOI, Champ { nom: "flux", libelle: Affiche::new("Flux"), forme: Forme::Texte, sens: "le flux du dépôt, tel que deposer le rend", avance: false }, MOIS],
        exemple: r#"{"quoi":"dpae"}"#,
        rend: "fini, passe, refuse, incident (le relevé qui n'a pu se faire), phrase et couleur",
        refus: &[MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 80,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::BilanRepetition,
        geste: "Dire où en est le test de la DSN ou de la DPAE ; chercher son compte rendu.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "etat (testee : le réel peut partir ; en_test ; test_refuse, et son motif ; ecrite), titre et phrase de l'encart, conforme, incident",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 500,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les formes

/// CE QUI SE DEPOSE : la DPAE ou la DSN (un signalement est une DSN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Quoi {
    Dpae,
    Dsn,
}

impl Quoi {
    /// Sa sorte au journal des envois.
    pub(crate) fn acte(self) -> envois::journal::Acte {
        match self {
            Quoi::Dpae => envois::journal::Acte::Dpae,
            Quoi::Dsn => envois::journal::Acte::Dsn,
        }
    }
}

/// Une embauche que la charge designe : le salarie et le numero du contrat de son engagement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmbaucheDesignee {
    pub(crate) salarie: String,
    pub(crate) numero: String,
}

/// LA DESIGNATION D'UNE DECLARATION, la charge de `depot-apercu` et de `bilan-repetition` : la DSN ou la DPAE d'un mois
/// (`mois`, et l'etablissement d'une DSN, les embauches d'une DPAE), ou un signalement ecrit (`fichier`). C'est aussi
/// la charge du geste qui ouvre la boite du depot (`etapes::Action`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Designation {
    pub(crate) quoi: Quoi,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mois: Option<Mois>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) etablissement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) engagements: Option<Vec<EmbaucheDesignee>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fichier: Option<String>,
}

impl Designation {
    /// La declaration du mois `mois`.
    pub(crate) fn du_mois(quoi: Quoi, mois: Mois) -> Designation {
        Designation { quoi, mois: Some(mois), etablissement: None, engagements: None, fichier: None }
    }

    /// Le signalement ecrit `fichier`.
    pub(crate) fn du_fichier(fichier: impl Into<String>) -> Designation {
        Designation { quoi: Quoi::Dsn, mois: None, etablissement: None, engagements: None, fichier: Some(fichier.into()) }
    }

    /// L'etablissement nomme (son NIC), en valeur : `Null` sans lui.
    fn nic(&self) -> Value {
        self.etablissement.as_deref().filter(|n| !n.is_empty()).map_or(Value::Null, |n| json!(n))
    }
}

/// La charge de `deposer` : la designation, et ce que le depot demande (le reel et son mot de passe, l'annule-et-remplace
/// voulu, le motif d'un reel sans test).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDeposer {
    quoi: Quoi,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mois: Option<Mois>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etablissement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engagements: Option<Vec<EmbaucheDesignee>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fichier: Option<String>,
    #[serde(default)]
    reel: bool,
    #[serde(default)]
    forcer: bool,
    #[serde(default)]
    contournement: String,
    /// Le mot de passe du logiciel, tape pour l'acte (`routes::servir` le retient le temps de la route).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    motdepasse: Option<String>,
}

impl ChargeDeposer {
    /// Le depot de la declaration `d`, reel ou de test, sous le mot de passe tape.
    pub(crate) fn de(d: Designation, reel: bool, motdepasse: Option<String>) -> ChargeDeposer {
        let Designation { quoi, mois, etablissement, engagements, fichier } = d;
        ChargeDeposer { quoi, mois, etablissement, engagements, fichier, reel, forcer: false, contournement: String::new(), motdepasse }
    }
}

/// La charge de `suivre-depot` : ce qui est parti (`flux`, tel que `deposer` le rend) et son mois.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuivre {
    quoi: Quoi,
    #[serde(default)]
    flux: String,
    #[serde(default)]
    mois: Option<Mois>,
}

/// CE QUE LA BOITE DU DEPOT DIT, autour de ce que le fichier declare (`A`, l'apercu d'une DSN ou d'une DPAE) : sa
/// phrase, son titre, ses encarts, et les phrases pour `moteur route --texte`.
#[derive(Debug, Serialize)]
pub(crate) struct Dit<A> {
    phrase: String,
    titre: &'static str,
    encarts: Vec<Encart>,
    phrases: Vec<String>,
    avertissements: Vec<String>,
    informations: Vec<String>,
    #[serde(flatten)]
    apercu: A,
}

/// La reponse de `depot-apercu` : ce que la DSN ou la DPAE designee declare.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DepotApercu {
    Dsn(Dit<ApercuDsn>),
    Dpae(Dit<ApercuDpae>),
}

/// La reponse de `deposer` : le depot fait, sa phrase, et l'avis a montrer apres la DSN reelle d'un mois qui a des
/// salaries.
#[derive(Debug, Serialize)]
pub(crate) struct Deposee {
    phrase: String,
    #[serde(flatten)]
    fait: DepotFait,
    #[serde(skip_serializing_if = "Option::is_none")]
    avis: Option<Encart>,
}

/// La reponse de `suivre-depot` : le depot tranche ou non, passe ou refuse, le releve qui n'a pu se faire, la phrase
/// et sa couleur.
#[derive(Debug, Serialize)]
pub(crate) struct Suivi {
    fini: bool,
    passe: bool,
    refuse: bool,
    incident: String,
    phrase: String,
    couleur: &'static str,
}

/// La reponse de `bilan-repetition` : ou en est le test (`etats::Repetition`), et le releve qui n'a pu se faire.
#[derive(Debug, Serialize)]
pub(crate) struct BilanDuTest {
    #[serde(flatten)]
    repetition: Repetition,
    incident: String,
}

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    match nom {
        Route::DepotApercu => en_json(depot_apercu(ctx, lire_charge(charge)?)?),
        Route::Deposer => en_json(deposer(ctx, lire_charge(charge)?)?),
        Route::SuivreDepot => en_json(suivre_depot(ctx, lire_charge(charge)?)?),
        Route::BilanRepetition => en_json(bilan_repetition(ctx, lire_charge(charge)?)?),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// CE QUE LA BOITE DU DEPOT DIT DU FICHIER, encart par encart, dans l'ordre ou elle les montre : l'annule-et-remplace
/// qu'il porte, le prelevement a la source sur la grille par defaut (CGI, art. 204 H), les embauches deja declarees, le
/// fichier deja depose, le fichier code envoi de test.
fn encarts_du_depot(apercu: &DepotApercu) -> Vec<Encart> {
    let mut e = Vec::new();
    let (deja_depose, reel) = match apercu {
        DepotApercu::Dsn(Dit { apercu: a, .. }) => {
            if !a.remplace.is_empty() {
                e.push(Encart::de("information", "Annule et remplace", format!("Ce fichier remplace la déclaration {}.", a.remplace)));
            }
            if let Some(r) = a.remplacement.as_ref().filter(|r| r.fichier_remplace) {
                e.push(Encart::de(
                    "a_faire",
                    format!("Annule-et-remplace intégral — ordre {}", a.ordre),
                    format!(
                        "Il remplace la DSN réelle déjà partie pour ce mois (ordre {}), ordres de télépaiement compris : c'est lui qui vaudra. Admis jusqu'au {} à minuit.",
                        r.etat.dernier_ordre.map(|o| o.to_string()).unwrap_or_default(),
                        r.etat.limite.jj_mm_aaaa()
                    ),
                ));
            }
            if a.pas_bareme > 0 {
                let n = a.pas_bareme;
                e.push(Encart::de(
                    "a_faire",
                    format!("Prélèvement à la source : {} versement{} sur {} sur la grille par défaut", n, if n > 1 { "s" } else { "" }, a.pas_versements),
                    "Sans taux transmis par la DGFiP, la grille de l'article 204 H du CGI s'applique ; le dépôt n'est pas bloqué. Elle prélève souvent plus que le taux réel, et le salarié ne récupère la différence qu'à sa déclaration de revenus. Relever les retours avant de déposer peut changer cela.",
                ));
            }
            (&a.deja_depose, a.reel)
        }
        DepotApercu::Dpae(Dit { apercu: a, .. }) => {
            let deja = a.embauches.iter().filter(|x| x.deja_declaree).count();
            if deja > 0 {
                e.push(Encart::de(
                    "a_faire",
                    format!("{} déjà envoyée{}", super::compte(deja, "embauche", "embauches"), if deja > 1 { "s" } else { "" }),
                    "Le fichier porte tout le mois : le déposer les redéclare, et l'Urssaf reçoit deux déclarations pour un seul contrat.",
                ));
            }
            (&a.deja_depose, a.reel)
        }
    };
    if let Some(deja) = deja_depose {
        e.push(Encart::de(
            "a_faire",
            "Ce fichier a déjà été déposé",
            format!("Le {}, en mode {}. Redéposer le même fichier ne remplace rien : un annule-et-remplace se produit, il ne se redépose pas.", super::quand(deja.quand()), if deja.reel() { "réel" } else { "test" }),
        ));
    }
    if !reel {
        e.push(Encart::de("information", "Envoi de test", "Le fichier est codé envoi de test : déposé pour de vrai, il ne déclarerait rien et rien ne serait conservé."));
    }
    e
}

/// LES PASTILLES DES LIGNES DU JOURNAL QUE CES ROUTES RENDENT (`routes::pastille`) : la ligne d'un certificat de
/// conformite DPAE (`enregistree`) des retours d'un depot, enregistree, faite ; deja existante, une information ;
/// refusee, refusee.
fn poser_les_pastilles(v: &mut Option<Ligne>) {
    v.iter_mut().for_each(etats::poser_les_pastilles);
}

/// Les pastilles des lignes du journal qu'un test porte (`poser_les_pastilles`).
fn pastilles_du_test(r: &mut Repetition) {
    r.lignes_mut().for_each(etats::poser_les_pastilles);
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

/// Les secondes unix de l'horloge.
fn maintenant() -> f64 {
    socle::temps::secondes()
}

/// L'heure de Paris, en `AAAA-MM-JJTHH:MM:SS`.
fn maintenant_iso() -> String {
    socle::paris::iso_secondes(maintenant())
}

fn ecrire_texte(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| plain(socle::echec::du_disque(&d, &e)))?;
    }
    std::fs::write(chemin, texte).map_err(|e| plain(socle::echec::du_disque(&chemin, &e)))
}

// ------------------------------------------------------------------ dpae

/// Les noms `DPAE_<etiquette>_AAAAMMJJ-HHMM.txt` d'un dossier, du plus grand au plus petit
/// (le plus récent d'abord).
fn fichiers_partiels(dossier: &Path, etiq: &str) -> Vec<PathBuf> {
    let debut = format!("DPAE_{}_", etiq);
    let Ok(lecture) = std::fs::read_dir(dossier) else { return Vec::new() };
    let mut out = Vec::new();
    for e in lecture.flatten() {
        let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
        let Some(reste) = nom.strip_prefix(&debut) else { continue };
        let Some(reste) = reste.strip_suffix(".txt") else { continue };
        let c: Vec<char> = reste.chars().collect();
        // `AAAAMMJJ-HHMM`, et `AAAAMMJJ-HHMM-N` : le nom que prend un fichier partiel quand un autre porte deja le sien
        // dans la meme minute (`nom_libre_du_partiel`).
        let rang_ok = c.len() == 13 || (c.len() > 14 && c[13] == '-' && c[14..].iter().all(char::is_ascii_digit));
        if c.len() >= 13 && c[8] == '-' && rang_ok {
            out.push(e.path());
        }
    }
    out.sort();
    out.reverse();
    out
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel ecrit maintenant.
fn marque_a_la_minute() -> String {
    let now = maintenant_iso();
    format!("_{}{}{}-{}{}", &now[0..4], &now[5..7], &now[8..10], &now[11..13], &now[14..16])
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel, suivie de `-2`, `-3`... tant que `DPAE_<etiq><marque>.txt` existe deja.
pub(crate) fn nom_libre_du_partiel(dossier: &Path, etiq: &str, marque: &str) -> String {
    let mut essai = marque.to_string();
    let mut rang = 2;
    while dossier.join(format!("DPAE_{}{}.txt", etiq, essai)).exists() {
        essai = format!("{}-{}", marque, rang);
        rang += 1;
    }
    essai
}

fn latin1(octets: &[u8]) -> String {
    octets.iter().map(|&o| o as char).collect()
}

/// Le code Urssaf que la DPAE de cette compagnie porte.
fn code_urssaf(ent: &droit::fiche::ChampsEntreprise) -> R<String> {
    let ecrit = ent.code_urssaf.texte().trim();
    if !ecrit.is_empty() {
        return Ok(ecrit.to_string());
    }
    let territoire = declarations::urssaf::base().map_err(plain)?.du_code_postal(ent.code_postal.texte());
    Ok(if territoire.is_empty() { "117".to_string() } else { territoire })
}

/// `Embauche` de la DPAE, lue d'une paie sans montants.
fn embauche_dpae(p: &Paie, spectacle: bool) -> Embauche {
    Embauche {
        salarie: p.salarie.clone(),
        debut: json!(p.engagement.debut.iso()),
        fin: p.engagement.fin.map_or(Value::Null, |f| json!(f.iso())),
        nature: p.engagement.nature.clone(),
        spectacle: if spectacle && vrai(&p.spectacle) { p.spectacle.clone() } else { Value::Null },
    }
}


/// La DPAE, due AVANT le premier jour travaille : tout le mois `le_mois`, ou les embauches `selection` (le salarie et le
/// numero du contrat de l'engagement) dans un fichier horodate a cote du complet. La phrase rendue dit combien
/// d'embauches elle declare ; la demande de taux de PAS part avec son depot (`depot_acte::deposer_dpae`).
pub(crate) fn dpae(ctx: &Contexte, le_mois: Mois, selection: Option<&[EmbaucheDesignee]>) -> R<Dpae> {
    let mois = le_mois.to_string();
    // L'heure d'embauche que la DPAE declare.
    let heure = json!(declarations::dpae::HEURE_EMBAUCHE);

    // Les engagements valides du mois (rien ne part d'un engagement en saisie), vus comme des embauches, sans relire leur
    // paie.
    let journal = envois::journal::lu(&ctx.racine).map_err(plain)?;
    let data = engagement::etats::validees_du_mois(ctx, &mois, &journal.departs).map_err(plain)?.map(|(d, _)| d)
        .ok_or_else(|| erreur("FileNotFoundError", format!("Aucune saisie pour {}.", mois)))?;
    let paies: Vec<Paie> = {
        let engs = engagement::engagements_de_la_saisie(ctx, &data).map_err(plain)?;
        engs.iter().map(|e| engagement::embauche(ctx, e)).collect::<Result<_, _>>().map_err(plain)?
    };
    let etiq = le_mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(&mois);
    let cle_de = |p: &Paie| -> (String, String) { (p.salarie.id.to_string(), p.engagement.numero.as_deref().unwrap_or_default().to_string()) };

    // LES DPAE A FAIRE DU MOIS (`Moteur::embauches_a_declarer`, la seule liste) : une embauche deja partie en DPAE
    // reelle, ou un contrat en cours depuis un mois precedent (sa DPAE part avec son premier mois), n'y est pas.
    let a_faire: BTreeSet<(String, String)> = crate::routes::etapes::Moteur::nouveau(ctx)
        .embauches_a_declarer(Some(le_mois))?
        .into_iter()
        .map(|e| (e.salarie, e.numero.to_string()))
        .collect();
    let mut retenues: Vec<&Paie> = paies.iter().filter(|p| a_faire.contains(&cle_de(p))).collect();
    let mut marque = String::new();
    if let Some(sel) = selection {
        let voulues: BTreeSet<(String, String)> = sel.iter().map(|e| (e.salarie.clone(), e.numero.clone())).collect();
        let joint = |s: BTreeSet<&(String, String)>| s.into_iter().map(|(a, b)| format!("{} (contrat {})", a, b)).collect::<Vec<_>>().join(", ");
        let hors: BTreeSet<&(String, String)> = voulues.difference(&a_faire).collect();
        if !hors.is_empty() {
            let tous = hors.len() == voulues.len();
            let phrase = format!(
                "Aucune DPAE a faire pour : {} (deja partie, en cours depuis un mois precedent, ou absente de la saisie de {}). Rien n'est ecrit.",
                joint(hors),
                mois
            );
            return if tous { Ok(Dpae::Rien(phrase)) } else { Err(refus(phrase)) };
        }
        retenues.retain(|p| voulues.contains(&cle_de(p)));
        marque = marque_a_la_minute();
    }
    if retenues.is_empty() {
        return Ok(Dpae::Rien(format!("Aucune DPAE à faire en {}. Rien n'est écrit.", le_mois.libelle(Graphie::Accentuee))));
    }

    // RETOURS WINDOWS, D : DEUX LIGNES IDENTIQUES d'une saisie declareraient deux fois la meme embauche : la DPAE ne se
    // produit pas, le refus dit lesquelles (salarie, jour) et qu'il faut corriger la saisie. Ni le numero de contrat ni
    // la validation ne distinguent deux lignes.
    let brutes: Vec<Value> = data.engagements.iter().map(|e| e.en_valeur()).collect();
    if let Some(brutes) = Some(&brutes).filter(|b| b.len() == paies.len()) {
        let sans_numero = |v: &Value| -> Value {
            match v {
                Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| *k != "numero" && *k != "valide").map(|(k, x)| (k.clone(), x.clone())).collect()),
                autre => autre.clone(),
            }
        };
        let retenue = |i: usize| retenues.iter().any(|r| std::ptr::eq(*r, &paies[i]));
        let mut doubles: Vec<String> = Vec::new();
        for i in 1..paies.len() {
            if retenue(i) && (0..i).any(|j| retenue(j) && sans_numero(&brutes[i]) == sans_numero(&brutes[j])) {
                let (c, d) = (cle_de(&paies[i]).0, paies[i].engagement.debut.iso());
                let texte = format!("{} au {}", c, d);
                if !doubles.contains(&texte) {
                    doubles.push(texte);
                }
            }
        }
        if !doubles.is_empty() {
            return Err(refus(format!(
                "Deux lignes identiques dans la saisie de {} declareraient deux fois la meme embauche : {}. Corrigez la saisie (une seule ligne par embauche). Rien n'est ecrit.",
                mois,
                doubles.join(", ")
            )));
        }
    }

    // LE MEME CONTENU, LE MEME FICHIER : une DPAE deja ecrite qui dit la meme chose (la selection, ou tout ce qui reste a
    // declarer) se reprend telle quelle : le test qu'elle a passe vaut pour le reel (regle 10).
    let ent = ctx.entreprise_et_gabarits();
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins {
        defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"),
        poste: ctx.gabarits(),
    };
    let heure_texte = en_texte(&heure);
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let mut f: Option<PathBuf> = None;
    {
        let h = horodatage(&maintenant_iso());
        let voulu = declarations::dpae::contenu(&embauches, &parametres_de(&h, &heure_texte, &code, &ent, &chemins)).map_err(plain)?;
        let voulu = latin1(&voulu);
        let mut pareils = Vec::new();
        let complet = sortie.join(format!("DPAE_{}.txt", etiq));
        // LA DPAE DU MOIS DEJA PARTIE ne se recrit pas (regle 4) : ce qui reste a declarer se cherche, et s'ecrit, sous un
        // nom a la minute, a cote.
        let partiel = !marque.is_empty() || crate::compagnie::depart::partie_le(&ctx.racine, &complet)?.is_some();
        let candidats = if !partiel { vec![complet].into_iter().filter(|c| c.is_file()).collect() } else { fichiers_partiels(&sortie, &etiq) };
        for x in candidats {
            let contenu = std::fs::read(&x).map_err(|e| erreur("OSError", socle::echec::du_disque(&x, &e)))?;
            if declarations::dpae::meme_declaration(&latin1(&contenu), &voulu) {
                pareils.push(x);
            }
        }
        let mut repetes = Vec::new();
        for x in &pareils {
            let empreinte = envois::journal::empreinte_fichier(x).map_err(plain)?;
            let nom = x.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if etats::repetition(&journal.lignes, Quoi::Dpae.acte(), &empreinte, &nom).conforme.is_some() {
                repetes.push(x.clone());
            }
        }
        if !pareils.is_empty() {
            let choisi = repetes.first().unwrap_or(&pareils[0]).clone();
            let stem = choisi.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            marque = stem.chars().skip(format!("DPAE_{}", etiq).chars().count()).collect();
            f = Some(choisi);
        } else if partiel && marque.is_empty() {
            marque = marque_a_la_minute();
        }
    }
    // JAMAIS PAR-DESSUS UN AUTRE (deux DPAE jouees dans la meme minute s'ecraseraient) : si le
    // nom a la minute est pris par une declaration DIFFERENTE, le fichier prend `-2`, `-3`... ; le nom ordinaire ne change pas.
    if f.is_none() && !marque.is_empty() {
        marque = nom_libre_du_partiel(&sortie, &etiq, &marque);
    }
    let recap = sortie.join(format!("DPAE_{}{}_recapitulatif.txt", etiq, marque));
    let f = match f {
        Some(f) => f,
        None => {
            let cible = sortie.join(format!("DPAE_{}{}.txt", etiq, marque));
            ecrire_la_dpae(ctx, &retenues, &cible, &recap, &heure, &maintenant_iso())?;
            crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None, None)?;
            cible
        }
    };

    let n = retenues.len();
    Ok(Dpae::Ecrite(DpaeEcrite {
        chemin: f.to_string_lossy().into_owned(),
        phrase: format!("DPAE {} écrite : {} embauche{}.", socle::texte::de(&le_mois.libelle(Graphie::Accentuee)), n, if n > 1 { "s" } else { "" }),
    }))
}

/// CE QUE `dpae` REND : la DPAE ecrite, ou rien a declarer de ce que la charge designe (tout est parti, ou rien n'est
/// valide), avec la phrase qui le dit. Un manque (une case de fiche, deux lignes identiques) est un refus, jamais
/// « rien » : celui qui depose doit le lire.
pub(crate) enum Dpae {
    Ecrite(DpaeEcrite),
    Rien(String),
}

/// La DPAE ecrite : le chemin de son fichier (son `.zip` et son recapitulatif en clair sont a cote), et la phrase qui
/// dit combien d'embauches elle declare.
pub(crate) struct DpaeEcrite {
    pub(crate) chemin: String,
    pub(crate) phrase: String,
}

/// LES FICHIERS D'UNE DPAE : l'enregistrement de chaque embauche de `retenues` dans `cible` (et son `.zip`), horodate
/// `quand` (`AAAA-MM-JJTHH:MM:SS`, a Paris), l'heure d'embauche `heure` ; et son recapitulatif en clair dans `recap`.
/// Ce que `dpae` ecrit, et ce que `compagnie::depart::piece_partie` refait d'une DPAE partie dont le fichier manque.
pub(crate) fn ecrire_la_dpae(ctx: &Contexte, retenues: &[&Paie], cible: &Path, recap: &Path, heure: &Value, quand: &str) -> R<()> {
    let ent = ctx.entreprise_et_gabarits();
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins { defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"), poste: ctx.gabarits() };
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let h = horodatage(quand);
    declarations::dpae::ecrire(&embauches, cible, &parametres_de(&h, &en_texte(heure), &code, &ent, &chemins), true).map_err(plain)?;
    let pour_recap: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, true)).collect();
    ecrire_texte(recap, &declarations::dpae::recapitulatif(&pour_recap, heure).map_err(plain)?)
}

/// Les parametres d'une ecriture de DPAE.
fn parametres_de<'a>(horodatage: &'a str, heure: &str, code: &str, ent: &'a droit::contexte::EntrepriseDuDocument, chemins: &'a Chemins) -> Parametres<'a> {
    Parametres { horodatage, heure_embauche: heure.to_string(), code_urssaf: json!(code), ent: Some(ent), chemins }
}

/// `AAAA-MM-JJTHH:MM:SS` -> `JJMMAAAAHHMM`.
fn horodatage(iso: &str) -> String {
    format!("{}{}{}{}{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
}

#[cfg(test)]
mod tests_dpae_partielles {
    use super::*;

    fn dossier() -> PathBuf {
        let r = crate::horloge::racine_d_essai(false).join("sorties").join("2026-10");
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn deux_dpae_dans_la_meme_minute_ne_s_ecrasent_jamais() {
        // Le nom `DPAE_<mois>_<AAAAMMJJ-HHMM>.txt` est a la minute : la seconde declaration de la minute ecraserait la
        // premiere, et le depot du premier jour ne serait plus relisible tel qu'il a ete depose.
        let d = dossier();
        let marque = "_20261007-1815";
        // Cas ordinaire : le nom ne change pas.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), marque);
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815.txt"), "premiere").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-2");
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815-2.txt"), "seconde").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-3");
        // Une autre minute, ou un autre mois, garde son nom.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", "_20261007-1816"), "_20261007-1816");
        assert_eq!(nom_libre_du_partiel(&d, "Novembre2026", marque), marque);
        // Les fichiers deja ecrits sont intacts.
        assert_eq!(std::fs::read_to_string(d.join("DPAE_Octobre2026_20261007-1815.txt")).unwrap(), "premiere");
    }
}

// ------------------------------------------------------------------------ deposer

/// CE QU'ECRIRE LA DECLARATION A DIT : les avertissements et les informations de la DSN fabriquee (`DsnFabriquee::avis`),
/// ou la phrase de la DPAE ecrite.
#[derive(Debug, Default)]
pub(crate) struct DitALEcriture {
    phrase: Option<String>,
    avertissements: Vec<String>,
    informations: Vec<String>,
}

/// LA DECLARATION QUE LA DESIGNATION DESIGNE (son chemin), et ce que son ecriture a dit. La DSN du mois (`quoi` : dsn,
/// `mois`, `etablissement`) : fabriquee depuis les engagements valides (`fabriquer_dsn`, qui reprend tel quel le fichier
/// qui dit la meme chose : le test vaut pour le reel, regle 10) ; partie en reel, celle qui est partie, sauf si elle est
/// a corriger. La DPAE du mois (`quoi` : dpae, `mois`, `engagements`) : celle de ces embauches, ou de celles qui restent
/// a declarer (`dpae`, meme reprise) ; une fois partie, il n'en reste aucune a declarer, et c'est celle qui est partie
/// (`partie`), jamais refaite (regle 4). `fichier` : un signalement ecrit.
pub(crate) fn designee(ctx: &Contexte, d: &Designation) -> R<(String, DitALEcriture)> {
    // Un signalement se nomme par son fichier, la declaration du mois par son mois : les deux ne designent pas la meme.
    let (le_mois, fichier) = (d.mois, d.fichier.as_deref().filter(|f| !f.is_empty()));
    let Some(le_mois) = le_mois else {
        return fichier.map(|f| (f.to_string(), DitALEcriture::default())).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("fichier")));
    };
    if fichier.is_some() {
        return Err(refus(
            "`fichier` et `mois` désignent deux déclarations : `fichier` seul pour un signalement écrit, `mois` seul pour la \
             déclaration du mois. Rien n'est parti.",
        ));
    }
    if d.quoi == Quoi::Dsn {
        // UNE DSN PARTIE EN REEL NE SE REFAIT QUE SI CE QU'ELLE A EMPORTE EST A CORRIGER (regle 4) : un engagement du
        // mois change depuis, ajoute ou retire (`Moteur::etat_remplacement`, `a_envoyer`) ; sinon, c'est celle qui est
        // partie. Une DSN refusee au bilan n'est pas partie (regle 5).
        let nic = d.nic();
        let e = crate::compagnie::etat_dsn::du_mois(ctx, le_mois, nic.as_str())?;
        if e.deposee() && !e.refusee() && !crate::routes::etapes::Moteur::nouveau(ctx).etat_remplacement(le_mois, &nic)?.a_envoyer {
            if let Some(c) = partie(ctx, d, le_mois)? {
                return Ok((c, DitALEcriture::default()));
            }
        }
        let dsn = crate::compagnie::depart::fabriquer_dsn(ctx, le_mois, "interface")?;
        let chemin = match d.etablissement.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => dsn.fichiers.iter().find(|f| f.file_name().is_some_and(|x| x.to_string_lossy().contains(&format!("_{}", n)))),
            None => dsn.fichiers.first(),
        };
        let chemin = chemin.ok_or_else(|| refus("Aucune DSN de cet etablissement ce mois-ci. Rien n'est parti."))?;
        let (avertissements, informations) = dsn.avis();
        return Ok((chemin.to_string_lossy().into_owned(), DitALEcriture { phrase: None, avertissements, informations }));
    }
    // Ce qui reste a declarer s'ecrit, et un manque refuse ; quand rien ne reste, c'est la DPAE partie.
    match dpae(ctx, le_mois, d.engagements.as_deref())? {
        Dpae::Ecrite(w) => Ok((w.chemin, DitALEcriture { phrase: Some(w.phrase), ..Default::default() })),
        Dpae::Rien(phrase) => partie(ctx, d, le_mois)?.map(|c| (c, DitALEcriture::default())).ok_or_else(|| refus(phrase)),
    }
}

/// CE QUI EST PARTI POUR LA DESIGNATION `d` du mois `mois`, en test ou en reel, lu au journal (`envois::journal::lu`) :
/// le fichier de la derniere ligne du mois, du meme etablissement (DSN mensuelle) ou des memes embauches (DPAE, quand la
/// designation les nomme). Rien ne se fabrique.
fn partie(ctx: &Contexte, d: &Designation, mois: Mois) -> R<Option<String>> {
    let lignes = envois::journal::lu(&ctx.racine).map_err(plain)?.lignes;
    let principal = ctx.entreprise.nic_principal();
    let nic = d.etablissement.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| principal.clone());
    let voulues: Option<BTreeSet<(String, String)>> = d.engagements.as_ref().filter(|e| !e.is_empty()).map(|e| e.iter().map(|x| (x.salarie.clone(), x.numero.clone())).collect());
    let ligne = envois::journal::filtrer(&lignes, Some(d.quoi.acte()), Some(mois), false).rev().find(|l| match l {
        Ligne::Dsn(_) => l.fichier().unwrap_or("").starts_with("DSN_") && l.etablissement().unwrap_or(&principal) == nic,
        Ligne::Dpae(e) => voulues.as_ref().is_none_or(|v| e.detail.embauches.iter().map(|x| (x.salarie.to_string(), x.numero.as_deref().unwrap_or("").to_string())).collect::<BTreeSet<_>>() == *v),
        _ => false,
    });
    Ok(ligne.map(|l| ctx.racine.join("sorties").join(mois.to_string()).join(l.fichier().unwrap_or("")).to_string_lossy().into_owned()))
}

/// `deposer` : `motdepasse` est celui DU LOGICIEL, et il ne sert qu'a prouver la presence. Il arrive par le
/// corps du POST, il n'est ni journalise ni renvoye. `contournement` : le motif ECRIT d'un depot reel sans
/// repetition. `forcer` : l'annule-et-remplace voulu, apres l'avertissement que la page affiche sur
/// `DsnDejaDeposee` -- jamais par defaut.
fn deposer(ctx: &Contexte, c: ChargeDeposer) -> R<Deposee> {
    let ChargeDeposer { quoi, mois, etablissement, engagements, fichier, reel, forcer, contournement, motdepasse } = c;
    let d = Designation { quoi, mois, etablissement, engagements, fichier };
    let (fichier, _) = designee(ctx, &d)?;
    let preuve = Preuve::du_mot_de_passe(motdepasse);
    let contournement = contournement.trim();
    let mut fait = match quoi {
        Quoi::Dsn => depot_acte::deposer_dsn(ctx, &fichier, reel, Some(&preuve), "interface", forcer, contournement)?,
        Quoi::Dpae => depot_acte::deposer_dpae(ctx, &fichier, reel, Some(&preuve), "interface", false, contournement)?,
    };
    // LA PHRASE DU DEPOT : ou en est son test, ou ce qu'ont dit les retours du reel s'ils sont deja la (le simulateur repond
    // dans le geste) ; puis ses suites.
    let tranche = if !reel {
        let b = bilan_de_la_repetition(ctx, &Designation::du_fichier(fichier.clone()).avec(quoi))?;
        format!("{} {}", b.titre, b.phrase)
    } else if fait.ligne.nombre_de_retours() > 0 {
        suivre_depot(ctx, ChargeSuivre { quoi, flux: fait.flux.clone(), mois: Some(fait.mois) })?.phrase
    } else {
        "Dépôt réel effectué — ses retours sont attendus.".to_string()
    };
    let phrase = std::iter::once(tranche).chain(fait.suites.iter().cloned()).collect::<Vec<_>>().join(" ");
    // APRES LA DSN REELLE D'UN MOIS QUI A DES SALARIES, la boite d'avis : leurs documents partent de l'etape 4.
    let avis = (reel && quoi == Quoi::Dsn && envois::depot::lire(&lire_octets(&resoudre(&ctx.racine, &fichier)?)?).salaries > 0)
        .then(|| Encart::de("information", "Les documents des salariés", "Ils partent de l'étape 4, « Envoyer les documents », qui dit ce qu'il leur manque encore."));
    etats::poser_les_pastilles(&mut fait.ligne);
    poser_les_pastilles(&mut fait.deja);
    Ok(Deposee { phrase, fait, avis })
}

impl Designation {
    /// La meme designation, pour `quoi`.
    fn avec(self, quoi: Quoi) -> Designation {
        Designation { quoi, ..self }
    }
}

// ------------------------------------------------------------------ suivre-depot

/// `suivre_le_depot` : juste apres un depot REEL, relever ses retours et dire s'il est tranche. Une LECTURE. Fini quand
/// chaque embauche du depot est passee ou refusee (DPAE), quand le bilan du mois est la (DSN).
fn suivre_depot(ctx: &Contexte, c: ChargeSuivre) -> R<Suivi> {
    let ChargeSuivre { quoi, flux, mois } = c;
    let dpae = quoi == Quoi::Dpae;
    let (debut, fin) = releve_retours::derniere_journee();
    let releve = if dpae {
        crate::compagnie::releve::relever_retours_dpae(ctx, debut, fin, false, &flux)
    } else {
        crate::compagnie::releve::relever_retours(ctx, debut, fin, false, "production", None, "")
    };
    let incident = match releve {
        Ok(r) => releve_retours::incidents_en_phrase(&r),
        Err(e) => e.phrase(),
    };
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let du_flux = |q: envois::journal::Acte| lignes.iter().rev().find(|l| l.acte() == q && l.flux() == flux);
    let (passe, refuse);
    if dpae {
        let etat = etats::etat_des_embauches(&lignes);
        // Une embauche que l'etat ne connait pas (une ligne non acceptee) n'est ni passee ni refusee.
        let embauches: Vec<Option<&etats::EtatEmbauche>> = du_flux(Quoi::Dpae.acte())
            .and_then(Ligne::dpae)
            .map(|e| e.detail.embauches.iter().map(|x| etat.get(&(x.salarie.to_string(), x.numero.as_deref().unwrap_or("").to_string()))).collect())
            .unwrap_or_default();
        passe = !embauches.is_empty() && embauches.iter().all(|x| x.is_some_and(|x| x.passee));
        refuse = embauches.iter().any(|x| x.is_some_and(|x| x.refusee));
    } else {
        let ligne = if flux.is_empty() { None } else { du_flux(Quoi::Dsn.acte()) };
        let nic = ligne.and_then(Ligne::etablissement);
        // Sans son mois, rien ne dit ou en est la DSN : elle attend.
        let e = mois.map(|m| crate::compagnie::etat_dsn::du_mois(ctx, m, nic)).transpose()?;
        passe = e.as_ref().is_some_and(|e| e.etat == crate::compagnie::etat_dsn::Etat::Passee);
        refuse = e.as_ref().is_some_and(|e| e.refusee());
    }
    // Ce que l'ecran en dit, et en quelle couleur, une fois le depot tranche.
    let nom = if dpae { "La DPAE" } else { "La DSN" };
    let (phrase, etat) = if passe {
        (format!("{} est passée — retour de l'organisme reçu.", nom), "fait")
    } else if refuse {
        (format!("{} est refusée — voir le motif dans l'étape.", nom), "refuse")
    } else {
        (format!("{} attend le retour de l'organisme.", nom), "en_attente")
    };
    Ok(Suivi { fini: passe || refuse, passe, refuse, incident, phrase, couleur: crate::routes::couleur_de_l_etat(etat) })
}

// ------------------------------------------------------------------ bilan-repetition

/// `bilan_de_la_repetition` : ou en est le test de CE fichier -- et, s'il attend son compte rendu, aller le
/// chercher maintenant. Une LECTURE : rien ne part. Rend `etats::Repetition`, et `incident` si le releve n'a
/// pas pu se faire (le test reste ce qu'il est ; on le redira au passage suivant).
fn bilan_repetition(ctx: &Contexte, d: Designation) -> R<BilanDuTest> {
    let mut repetition = bilan_de_la_repetition(ctx, &d)?;
    let mut incident = String::new();
    if repetition.etat == etats::EtatDuTest::EnTest {
        let (debut, fin) = releve_retours::derniere_journee();
        let releve = match d.quoi {
            Quoi::Dpae => crate::compagnie::releve::relever_retours_dpae(ctx, debut, fin, false, &repetition.flux_conforme()),
            Quoi::Dsn => crate::compagnie::releve::relever_retours(ctx, debut, fin, false, "production", None, ""),
        };
        incident = match releve {
            Ok(rel) => releve_retours::incidents_en_phrase(&rel),
            Err(e) => e.phrase(),
        };
        repetition = bilan_de_la_repetition(ctx, &d)?;
    }
    pastilles_du_test(&mut repetition);
    Ok(BilanDuTest { repetition, incident })
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

// ------------------------------------------------------------------ les chemins

/// Un chemin POSIX normalisé : les `//` et les `.` intermédiaires s'en vont, un `/`
/// de tete reste (deux, s'il y en a exactement deux).
pub(crate) fn normaliser(t: &str) -> String {
    let tete = if t.starts_with("//") && !t.starts_with("///") {
        "//"
    } else if t.starts_with('/') {
        "/"
    } else {
        ""
    };
    let morceaux: Vec<&str> = t.split('/').filter(|m| !m.is_empty() && *m != ".").collect();
    let corps = morceaux.join("/");
    if tete.is_empty() && corps.is_empty() {
        return ".".to_string();
    }
    format!("{}{}", tete, corps)
}

/// `PurePosixPath(t).name`.
pub(crate) fn nom_du_chemin(t: &str) -> String {
    let n = normaliser(t);
    if n == "." || n.chars().all(|c| c == '/') {
        return String::new();
    }
    n.rsplit('/').next().unwrap_or("").to_string()
}

fn existe(t: &str) -> bool {
    !t.contains('\0') && Path::new(t).exists()
}

/// Un chemin tape a la main, tel quel
/// s'il existe, sinon sous les donnees, un mois nu designant sa saisie. Rend
/// le texte du chemin, normalisé.
pub(crate) fn resoudre(racine: &Path, t: &str) -> R<String> {
    if t.chars().all(socle::texte::blanc) {
        return Err(erreur("FileNotFoundError", "Aucun fichier indique."));
    }
    let p = normaliser(t);
    if existe(&p) || p.starts_with('/') {
        return Ok(p);
    }
    let base = normaliser(&racine.display().to_string());
    let candidat = normaliser(&format!("{}/{}", base, p));
    if existe(&candidat) {
        return Ok(candidat);
    }
    // Un mois nu (`\d{4}-\d{2}`) designe sa saisie.
    let c: Vec<char> = t.chars().collect();
    if c.len() == 7
        && c[..4].iter().all(|x| envois::retours::est_chiffre(*x))
        && c[4] == '-'
        && c[5..].iter().all(|x| envois::retours::est_chiffre(*x))
    {
        let saisie = normaliser(&format!("{}/{}/{}.json", base, engagement::stock::DOSSIER, t));
        if existe(&saisie) {
            return Ok(saisie);
        }
    }
    Ok(p)
}

/// `read_bytes`, l'erreur du disque sous sa classe.
fn empreinte_du_fichier(chemin: &str) -> R<String> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(envois::journal::empreinte(&octets))
}

// ------------------------------------------------------------------ depot-apercu, la DPAE

/// Le contenu d'un fichier, en octets.
pub(crate) fn lire_octets(chemin: &str) -> R<Vec<u8>> {
    if chemin.contains('\0') {
        return Err(erreur("ValueError", "embedded null byte"));
    }
    let p = Path::new(chemin);
    std::fs::read(p).map_err(|e| erreur_io(&e, p))
}

/// Un chiffre du latin-1 : les chiffres, et les exposants `²`, `³`, `¹`.
fn chiffre_du_latin1(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
}

/// Un jour JJ/MM/AAAA en ISO.
fn iso_depuis_jjmmaaaa(v: &str) -> String {
    let t: Vec<char> = v.trim_matches(socle::texte::blanc).chars().collect();
    if t.len() != 8 || !t.iter().all(|c| chiffre_du_latin1(*c)) {
        return String::new();
    }
    let s = |a: usize, b: usize| t[a..b].iter().collect::<String>();
    format!("{}-{}-{}", s(4, 8), s(2, 4), s(0, 2))
}

/// UNE EMBAUCHE QUE LE FICHIER DPAE DECLARE : le salarie (`?` s'il n'est pas de la configuration), son nom, son contrat
/// (numero, bornes, nature), si elle est deja declaree, et sa pastille.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmbaucheDuFichier {
    pub(crate) salarie: String,
    pub(crate) nom: String,
    pub(crate) debut: String,
    pub(crate) numero: Option<String>,
    pub(crate) fin: String,
    pub(crate) type_contrat: String,
    pub(crate) deja_declaree: bool,
    pastille: Pastille,
}

/// CE QUE LE FICHIER DPAE DECLARE (`apercu_depot_dpae`) : son nom, sa taille, son indicateur, son mois (celui de sa
/// premiere embauche), ses embauches, s'il est deja depose (`deja_depose` : la ligne du journal) et son test.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuDpae {
    pub(crate) description: String,
    pub(crate) fichier: String,
    pub(crate) nom: String,
    pub(crate) enregistrements: usize,
    pub(crate) octets: usize,
    pub(crate) reel: bool,
    pub(crate) indicateur: String,
    pub(crate) mois: Option<Mois>,
    pub(crate) embauches: Vec<EmbaucheDuFichier>,
    pub(crate) deja_depose: Option<Ligne>,
    pub(crate) repetition: Repetition,
}

/// `apercu_depot_dpae` : ce que le fichier DPAE declare, enregistrement par enregistrement.
pub(crate) fn apercu_depot_dpae(ctx: &Contexte, fichier: &str) -> R<ApercuDpae> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let enrs = declarations::dpae::relire(&texte);
    let m = crate::routes::etapes::Moteur::nouveau(ctx);
    let lignes_journal = m.lignes()?;
    let faites = etats::embauches_par_depot(&lignes_journal);
    let mut par_nir: HashMap<String, (&String, &Value)> = HashMap::new();
    // L'identite d'une embauche se compare aux cases de la fiche par leur nom : en JSON.
    let vues: Vec<(String, Value)> = ctx.salaries.iter().map(|(k, f)| (k.to_string(), f.champs.en_valeur())).collect();
    for (cle, s) in &vues {
        let nir = etats::get(s, "nir")?;
        if !vrai(nir) {
            continue;
        }
        match nir {
            Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
            Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
            Value::String(t) => {
                par_nir.insert(t.clone(), (cle, s));
            }
            _ => {}
        }
    }
    // LE NUMERO DU CONTRAT DE CHAQUE EMBAUCHE : celui de l'engagement valide du salarie qui commence ce jour-la. C'est lui
    // qui relie la DPAE, partie ou refusee, a son engagement.
    let departs = m.departs()?;
    let mut numeros: HashMap<(String, String), String> = HashMap::new();
    // La nature de chaque embauche, par son nom court (« CDDU ») : le fichier ne dit que CDD, CDI ou CTT (Format 120).
    let mut natures: HashMap<(String, String), String> = HashMap::new();
    let mut mois_vus: Vec<String> = Vec::new();
    let vide = json!({});
    let mut lignes = Vec::new();
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let m_e: String = iso_depuis_jjmmaaaa(champ("date_embauche")).chars().take(7).collect();
        if m_e.len() == 7 && !mois_vus.contains(&m_e) {
            for etat in engagement::etats::tels_qu_ecrits(ctx, &m_e, &departs).map_err(Echec::from)? {
                let Some(v) = etat.valide() else { continue };
                let brut = &*v.objet().brut;
                let Some(cle) = engagement::cle_de_l_engagement(ctx, &brut) else { continue };
                let id = (cle.into(), engagement::debut_brut(&ctx.racine, &brut));
                if let Ok(nat) = droit::natures::nature(brut.get("nature").and_then(Value::as_str)) {
                    natures.insert(id.clone(), nat.libelle.clone());
                }
                if let Some(n) = brut.get("numero").and_then(Value::as_str) {
                    numeros.insert(id, n.to_string());
                }
            }
            mois_vus.push(m_e);
        }
    }
    for e in &enrs {
        let champ = |k: &str| e.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        // Sans NIR, la DPAE declare un numero temporaire (zone a blanc) : l'embauche se reconnait a l'identite qu'elle
        // porte, nom, prenom et date de naissance (`dpae::enregistrement`).
        let identite = |s: &Value| {
            let t = |k: &str| s.get(k).map(socle::date::en_texte).unwrap_or_default().trim().to_uppercase();
            let n = s.get("date_naissance").and_then(Value::as_str).and_then(|d| socle::date::Jour::lire(d).ok()).map(|j| j.jjmmaaaa());
            t("nom") == champ("nom").trim() && t("prenom") == champ("prenom").trim() && n.as_deref() == Some(champ("date_naissance"))
        };
        let trouve = par_nir.get(champ("nir").trim()).copied().or_else(|| {
            if !champ("nir").trim().is_empty() {
                return None;
            }
            vues.iter().find(|(_, s)| identite(s)).map(|(k, s)| (k, s))
        });
        let (cle, s) = match trouve {
            Some((c, s)) => (Some(c), s),
            None => (None, &vide),
        };
        let debut = iso_depuis_jjmmaaaa(champ("date_embauche"));
        let nom = socle::texte::salarie_nomme(s).unwrap_or_else(|| "salarie inconnu de la configuration".to_string());
        let type_brut = champ("type_contrat").trim_matches(socle::texte::blanc);
        let type_contrat = cle.and_then(|c| natures.get(&(c.clone(), debut.clone()))).cloned().unwrap_or_else(|| {
            let t = droit::natures::TYPES_CONTRAT_DPAE.iter().find(|(k, _)| *k == type_brut);
            t.map_or_else(|| format!("type {}", type_brut), |(_, l)| l.to_string())
        });
        let numero = cle.and_then(|c| numeros.get(&(c.clone(), debut.clone()))).cloned();
        let deja = match &numero {
            Some(n) => cle.is_some_and(|c| faites.contains_key(&(c.clone(), n.clone()))),
            None => false,
        };
        lignes.push(EmbaucheDuFichier {
            salarie: cle.map_or("?", |c| c.as_str()).to_string(),
            nom,
            debut,
            numero,
            fin: iso_depuis_jjmmaaaa(champ("date_fin")),
            type_contrat,
            deja_declaree: deja,
            // Deja declaree, a voir (elle partirait deux fois) ; a declarer, une information.
            pastille: if deja { Pastille::de("a_faire", "déjà déclarée") } else { Pastille::de("information", "à déclarer") },
        });
    }
    let mois = lignes.iter().filter_map(|l| l.debut.get(..7)).filter_map(|m| Mois::lire(m).ok()).min();
    let empreinte = envois::journal::empreinte(&contenu);
    let nom_fichier = nom_du_chemin(&chemin);
    let mut deja_depose = etats::deja_depose(&lignes_journal, Quoi::Dpae.acte(), &empreinte);
    poser_les_pastilles(&mut deja_depose);
    let mut repetition = etats::repetition(&lignes_journal, Quoi::Dpae.acte(), &empreinte, &nom_fichier);
    pastilles_du_test(&mut repetition);
    Ok(ApercuDpae {
        description: format!("{} — {} enregistrement{}, {} octets, indicateur {}.", nom_fichier, enrs.len(), if enrs.len() > 1 { "s" } else { "" }, contenu.len(), envois::depot::indicateur(&contenu)),
        fichier: chemin,
        nom: nom_fichier,
        enregistrements: enrs.len(),
        octets: contenu.len(),
        reel: envois::depot::est_reel(&contenu),
        indicateur: envois::depot::indicateur(&contenu),
        mois,
        embauches: lignes,
        deja_depose,
        repetition,
    })
}

/// `depot-apercu` : la declaration designee, ecrite ou reprise (`designee`), et ce qu'elle declare, avec ce que son
/// ecriture a dit (ce que deux engagements disent pareil se dit une fois) ; les encarts de la boite et sa phrase.
fn depot_apercu(ctx: &Contexte, d: Designation) -> R<DepotApercu> {
    let (fichier, dit) = designee(ctx, &d)?;
    let une_fois = |l: Vec<String>| {
        let mut vus: Vec<String> = Vec::new();
        l.into_iter().filter(|x| if vus.contains(x) { false } else { vus.push(x.clone()); true }).collect::<Vec<_>>()
    };
    let (avertissements, informations) = (une_fois(dit.avertissements), une_fois(dit.informations));
    let ecrite = dit.phrase.unwrap_or_default();
    let mut a = match d.quoi {
        Quoi::Dsn => {
            let mut a = crate::apercu_dsn::apercu_depot_dsn(ctx, &fichier)?;
            poser_les_pastilles(&mut a.deja_depose);
            pastilles_du_test(&mut a.repetition);
            // Une DSN se dit par sa nature, son mois et ses salaries quand rien d'autre n'est a dire.
            let phrase = if ecrite.is_empty() {
                format!("{} {} : {}.", a.libelle_nature, socle::texte::de(&a.mois.libelle(Graphie::Accentuee)), super::compte(a.salaries, "salarié", "salariés"))
            } else {
                ecrite
            };
            DepotApercu::Dsn(Dit { phrase, titre: "Déposer la DSN", encarts: Vec::new(), phrases: Vec::new(), avertissements, informations, apercu: a })
        }
        Quoi::Dpae => {
            let a = apercu_depot_dpae(ctx, &fichier)?;
            // Une DPAE se dit par ses embauches quand rien d'autre n'est a dire.
            let phrase = if ecrite.is_empty() {
                let mois = a.mois.map(|m| m.libelle(Graphie::Accentuee)).unwrap_or_default();
                format!("DPAE {} : {}.", socle::texte::de(&mois), super::compte(a.embauches.len(), "embauche", "embauches"))
            } else {
                ecrite
            };
            DepotApercu::Dpae(Dit { phrase, titre: "Déposer la DPAE", encarts: Vec::new(), phrases: Vec::new(), avertissements, informations, apercu: a })
        }
    };
    // Les phrases de ses parties (le remplacement, le recapitulatif, le test), a la suite de la sienne.
    let encarts = encarts_du_depot(&a);
    let mut lignes: Vec<String> = match &a {
        DepotApercu::Dsn(Dit { apercu: x, .. }) => {
            let recap = match &x.recapitulatif {
                Some(crate::apercu_dsn::Recapitulatif::Lu(r)) => Some(r.phrase.clone()),
                _ => None,
            };
            [x.remplacement.as_ref().map(|r| r.etat.phrase.clone()), recap, Some(x.repetition.phrase.clone())].into_iter().flatten().collect()
        }
        DepotApercu::Dpae(Dit { apercu: x, .. }) => vec![x.repetition.phrase.clone()],
    };
    lignes.retain(|p| !p.is_empty());
    lignes.extend(encarts.iter().map(Encart::en_ligne));
    match &mut a {
        DepotApercu::Dsn(dit) => {
            lignes.extend(dit.avertissements.iter().cloned());
            dit.encarts = encarts;
            dit.phrases = lignes;
        }
        DepotApercu::Dpae(dit) => {
            lignes.extend(dit.avertissements.iter().cloned());
            dit.encarts = encarts;
            dit.phrases = lignes;
        }
    }
    Ok(a)
}

/// Ou en est le test de ce qui est parti pour la designation (`partie`), ou du signalement nomme.
fn bilan_de_la_repetition(ctx: &Contexte, d: &Designation) -> R<Repetition> {
    let fichier = match d.mois {
        Some(m) => partie(ctx, d, m)?.ok_or_else(|| refus(format!("Rien n'est parti pour {} : le déposer d'abord en test.", m)))?,
        None => d.fichier.clone().filter(|f| !f.is_empty()).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("fichier")))?,
    };
    let chemin = resoudre(&ctx.racine, &fichier)?;
    let empreinte = empreinte_du_fichier(&chemin)?;
    let nom = nom_du_chemin(&chemin);
    let lignes = crate::routes::etapes::Moteur::nouveau(ctx).lignes()?;
    Ok(etats::repetition(&lignes, d.quoi.acte(), &empreinte, &nom))
}
