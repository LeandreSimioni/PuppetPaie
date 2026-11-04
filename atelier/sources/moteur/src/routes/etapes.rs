//! Les routes du mois : `etat-par-engagement`, `etat-du-mois`, `mois-a-ouvrir`, et les lectures
//! qu'elles partagent (`bilan_du_mois`, `fichiers_deposables`) : l'etat de chaque engagement (sa validation, sa DPAE,
//! son contrat, ses documents, la DSN du mois), l'etat du mois, et l'annule-et-remplace.
//!
//! UN PROCESSUS PAR ROUTE, ET CHAQUE FICHIER LU UNE FOIS : le journal des
//! depots, la saisie de chaque mois et le Contexte ne se relisent pas d'un
//! etat a l'autre. Les etats du journal sont ceux de `etats.rs`, appeles
//! directement sur les lignes deja lues.
//!
//! AUCUN CALCUL D'UN ENGAGEMENT. La saisie se charge sans calculer (`charger_saisie` : les engagements, le contrat qui
//! deborde, les numeros attribues en memoire).
//!
//! LES FORMES. Chaque charge est une structure lue une fois a l'entree (`routes::lire_charge`) ; chaque reponse, une
//! structure : l'etat d'un engagement (`Ligne`, dont la page lit `LigneDePage`), le bilan du mois (`Bilan`, ses `Etape`
//! et leurs `Groupe`), les carres d'un engagement (`Carre`), le geste d'un clic (`Action`). Ce que ce module lit d'un
//! autre domaine se lit tel qu'il est : le journal des envois (`envois::etats` : les DPAE, les signatures, les avenants,
//! les depots ; `envois::journal`), l'engagement ecrit et sa saisie (`engagement` : l'identite, la frise, l'objet brut),
//! la fiche d'un spectacle (son titre), le dossier d'un contrat signe ailleurs (`compagnie::signature`), le journal des
//! ecritures (`compagnie::depart`) et un mois importe (`import::mois`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::echeances::Echeance;
use socle::date::Jour;
use socle::mois::Mois;
use engagement;
use engagement::etats::ARefaire;
use engagement::numeros::NumeroContrat;
use envois::etats::{self, est, get, getd, objet_vide, ou, quand_lisible, texte_vide, DOCUMENTS, DPAE, DSN, LISTE_VIDE, SIGNATURE};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{sha256_hex, vrai};
use socle::echec::Echec;
use crate::routes::catalogue::{Acte, Fiche};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::Ecran;
use crate::routes::{couleur_de_l_etat, en_json, lire_charge, SansCharge};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// La route de ce module, et sa fiche ; les deux etats d'un mois se servent de l'etat garde (`etat_garde::ROUTES`).
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::MoisAOuvrir,
    geste: "Dire quel mois ouvrir : le plus ancien dont une étape reste à faire.",
    charge: &[],
    exemple: "{}",
    rend: "mois (celui à ouvrir), courant, reste (les étapes qui restent)",
    refus: &[crate::routes::catalogue::ILLISIBLE],
    acte: Acte::Lit,
    avance: false,
    phrase: 0,
    ecran: ECRAN,
}];

pub(crate) type Paire = (String, String);

// ------------------------------------------------------------------ les charges

/// La charge des etats d'un mois (`etat-par-engagement`, `etat-du-mois`, `mois` : `etat_garde`) : le mois.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeMois {
    pub(crate) mois: Mois,
}

// ------------------------------------------------------------------ les reponses

/// Une etape du mois, et le carre d'un engagement qui la porte : sa cle, celle du journal des envois (`envois::etats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuelleEtape {
    Dpae,
    Signature,
    Dsn,
    Documents,
}

impl QuelleEtape {
    pub(crate) fn cle(self) -> &'static str {
        match self {
            QuelleEtape::Dpae => DPAE,
            QuelleEtape::Signature => SIGNATURE,
            QuelleEtape::Dsn => DSN,
            QuelleEtape::Documents => DOCUMENTS,
        }
    }

    fn nom(self) -> &'static str {
        match self {
            QuelleEtape::Dpae => "DPAE",
            QuelleEtape::Signature => "Contrats",
            QuelleEtape::Dsn => "DSN",
            QuelleEtape::Documents => "Documents",
        }
    }
}

impl Serialize for QuelleEtape {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.cle())
    }
}

/// L'etat d'une case, d'un carre, d'un choix : `fait`, `a_faire`, `en_attente`, `refuse` ; `rien` (un mois sans
/// engagement), `information` (un mois importe). Sa couleur est celle de tout l'ecran (`couleur_de_l_etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EtatCase {
    Fait,
    AFaire,
    EnAttente,
    Refuse,
    Rien,
    Information,
}

impl EtatCase {
    fn nom(self) -> &'static str {
        match self {
            EtatCase::Fait => "fait",
            EtatCase::AFaire => "a_faire",
            EtatCase::EnAttente => "en_attente",
            EtatCase::Refuse => "refuse",
            EtatCase::Rien => "rien",
            EtatCase::Information => "information",
        }
    }

    fn couleur(self) -> &'static str {
        couleur_de_l_etat(self.nom())
    }
}

/// LE GESTE D'UN CLIC, en routes du catalogue, celles que l'ecran et le LLM appellent : la route qu'il ouvre (`route`,
/// un apercu le plus souvent), sa charge (`charge`), la route de l'acte qui suit, s'il y en a un (`puis`), et ce que dit
/// le bouton (`libelle`). La page ouvre sa boite d'apres la route. `tester` : le depot d'une DSN, dont le test part des
/// qu'elle est ecrite.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Action {
    route: Route,
    charge: Value,
    puis: Option<Route>,
    libelle: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tester: bool,
}

/// L'`Action` de `route` sur `charge` : la structure de charge de la route quand elle en a une (`ChargeMois`, celles
/// des imprevus et des rappels), sinon son `json!`.
pub(crate) fn action(route: Route, charge: impl Serialize, puis: Option<Route>, libelle: String) -> Action {
    Action { route, charge: serde_json::to_value(charge).unwrap_or(Value::Null), puis, libelle, tester: false }
}

/// Le compte d'une case : un nombre, ou ce qui est revenu sur ce qui est parti.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Compte {
    Nombre(usize),
    Recus { recus: usize, partis: usize },
}

/// Une echeance telle qu'une case la rend (`Echeance::en_json` : jour, texte, court, source).
fn echeance_dite<S: serde::Serializer>(e: &Option<Echeance>, s: S) -> Result<S::Ok, S::Error> {
    e.as_ref().map(Echeance::en_json).serialize(s)
}

/// UN GROUPE D'UNE ETAPE, une case du panneau : son etat, son compte, ce qu'elle ecrit (`compte_texte` en gras : « 2 »,
/// « 1/2 », « Lyon », ou rien ; `quoi` : « à déposer », « reçu »), sa phrase entiere (`phrase_du_groupe`), son echeance,
/// le geste qu'ouvre un clic, les colonnes qu'elle couvre, sa couleur, et si elle se dessine coupee (une attente : la
/// moitie faite, la moitie qui attend le retour).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Groupe {
    pub(crate) etat: EtatCase,
    compte: Option<Compte>,
    compte_texte: String,
    quoi: String,
    phrase: Option<String>,
    #[serde(serialize_with = "echeance_dite")]
    echeance: Option<Echeance>,
    action: Option<Action>,
    colonnes: Vec<String>,
    couleur: &'static str,
    coupee: bool,
}

impl Groupe {
    /// Un groupe : son etat, son compte, ce qu'il ecrit, son echeance et son geste ; sa phrase, les deux joints.
    fn de(etat: EtatCase, compte: Option<Compte>, compte_texte: String, quoi: String, echeance: Option<Echeance>, action: Option<Action>) -> Groupe {
        let phrase = if compte_texte.is_empty() { quoi.clone() } else { format!("{} {}", compte_texte, quoi) };
        Groupe { etat, compte, compte_texte, quoi, phrase: Some(phrase), echeance, action, colonnes: Vec::new(), couleur: etat.couleur(), coupee: etat == EtatCase::EnAttente }
    }

    /// Ce qui est fait : le nom de l'etape et la coche, rien d'autre.
    fn fait() -> Groupe {
        Groupe { phrase: None, ..Groupe::de(EtatCase::Fait, None, String::new(), String::new(), None, None) }
    }

    /// Un mois sans aucun engagement : l'etape n'a rien a faire, elle n'est pas faite pour autant. Elle se dit en gris.
    fn rien_ce_mois(colonnes: Vec<String>) -> Groupe {
        Groupe { colonnes, ..Groupe::de(EtatCase::Rien, None, String::new(), "rien ce mois-ci".into(), None, None) }
    }

    fn sous(mut self, colonnes: &[String]) -> Groupe {
        self.colonnes = colonnes.to_vec();
        self
    }
}

/// UNE ETAPE DU MOIS : sa cle, son nom, ses groupes, et si elle est la suivante (la premiere qui a quelque chose a faire
/// de notre cote). Un mois importe dit sa phrase (`import::mois::etapes`) ; les documents nomment ce qui part.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Etape {
    pub(crate) cle: QuelleEtape,
    nom: &'static str,
    pub(crate) groupes: Vec<Groupe>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phrase: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    suivante: bool,
}

impl Etape {
    fn de(cle: QuelleEtape, groupes: Vec<Groupe>) -> Etape {
        Etape { cle, nom: cle.nom(), groupes, phrase: None, detail: None, suivante: false }
    }
}

/// LE BILAN DU MOIS (`bilan_du_mois`), ce que rend `etat-du-mois` : sa phrase, ses colonnes (G d'abord), ses etapes.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Bilan {
    mois: Mois,
    phrase: String,
    colonnes: Vec<String>,
    pub(crate) etapes: Vec<Etape>,
}

/// `mois-a-ouvrir` : le mois a ouvrir, celui du jour, et les etapes qui restent au premier.
#[derive(Debug, Serialize)]
struct MoisAOuvrir {
    phrase: String,
    mois: Mois,
    courant: Mois,
    reste: Vec<QuelleEtape>,
}

/// `etat-par-engagement` : les lignes que la page lit, et les engagements non actifs du mois.
#[derive(Debug, Serialize)]
struct EtatParEngagement<'a> {
    mois: Mois,
    engagements: Vec<LigneDePage<'a>>,
    non_actifs: Vec<NonActif>,
}

/// Un engagement non actif du mois (`non_actifs`).
#[derive(Debug, Serialize)]
struct NonActif {
    salarie: String,
    nom: String,
    numero: Option<String>,
    debut: String,
    fin: Option<String>,
    non_actif: String,
    non_actif_dit: String,
    dit: String,
    a_faire: Vec<String>,
}

// ------------------------------------------------------------ les groupes

/// Les quatre etapes du mois, dans l'ordre de l'habitude (aucun texte n'impose d'ordre).
const ORDRE: [QuelleEtape; 4] = [QuelleEtape::Dpae, QuelleEtape::Signature, QuelleEtape::Dsn, QuelleEtape::Documents];

/// L'echeance d'un depot de DSN : `mots` le `jour` a `heure`, d'apres `source`.
fn echeance_dsn(jour: Jour, heure: &'static str, source: &str) -> Echeance {
    Echeance { jour, mots: "avant le", heure, source: source.into() }
}

/// Faire signer les contrats du mois : l'apercu, puis la demande.
fn faire_signer(mois: &str) -> Action {
    action(Route::SignatureApercu, json!({"mois": mois}), Some(Route::DemanderSignature), "Demander la signature…".into())
}

/// Les contrats partis du mois, recus ou a relancer : l'apercu, puis noter chacun recu.
fn contrats_en_attente(mois: &str) -> Action {
    action(Route::SignatureApercu, json!({"mois": mois}), Some(Route::ContratRecu), "Reçus ou à relancer…".into())
}

/// Les documents du mois : l'apercu, puis l'envoi.
fn envoyer_documents(mois: &str, libelle: String) -> Action {
    action(Route::EnvoiApercu, json!({"mois": mois}), Some(Route::EnvoyerDocuments), libelle)
}

/// Ce que les retours du mois ont dit (le bilan d'un refus).
fn historique(mois: &str) -> Action {
    action(Route::HistoriqueMois, json!({"mois": mois}), None, "Voir le bilan…".into())
}

/// Les DPAE du mois a ecrire : l'apercu du depot (qui les ecrit), puis le depot.
fn declarer_embauches(mois: &str) -> Action {
    action(Route::DepotApercu, json!({"quoi": DPAE, "mois": mois}), Some(Route::Deposer), "Envoyer les DPAE…".into())
}

/// La DSN du mois a ecrire, ou a reecrire : l'apercu du depot (qui l'ecrit), puis le depot, dont le test part des
/// qu'elle est ecrite (`tester`).
pub(crate) fn deposer_dsn(mois: &str, nic: &Value, libelle: String) -> Action {
    let mut charge = json!({"quoi": DSN, "mois": mois});
    if vrai(nic) {
        charge["etablissement"] = nic.clone();
    }
    Action { tester: true, ..action(Route::DepotApercu, charge, Some(Route::Deposer), libelle) }
}

/// Le geste d'un carre Contrat en attente : noter revenu signe (`recu`) le contrat de l'engagement `p`, ou l'avenant
/// `avenant` (son numero) ; la route `contrat-recu` rend la phrase qui le dit et de quoi l'annuler.
fn noter_recu(p: &Vue, mois: &str, libelle: &str, avenant: Value, recu: bool) -> Action {
    action(Route::ContratRecu, json!({"mois": mois, "salarie": p.salarie, "debut": p.debut, "numero": p.numero_ou_vide(), "avenant": avenant, "recu": recu}), None, libelle.into())
}

/// L'avenant du contrat de l'engagement `p` : l'apercu, puis sa demande de signature.
fn avenant(p: &Vue, mois: &str) -> Action {
    action(Route::AvenantApercu, json!({"salarie": p.salarie, "debut": p.debut, "numero": p.numero_ou_vide(), "mois": mois}), Some(Route::DemanderSignature), "Avenant…".into())
}

/// Une etape qui n'a rien a faire ici pour les engagements de sa case (un contrat commence avant le logiciel) : faite, et
/// elle dit pourquoi (`quoi`).
fn rien_ici(quoi: &str) -> Groupe {
    Groupe::de(EtatCase::Fait, None, String::new(), quoi.into(), None, None)
}

/// `n` engagements auxquels il manque un fait (`A_COMPLETER`) : ce qui reste a faire, en orange, sans geste ; la carte
/// de chacun dit ce qui lui manque.
fn a_completer(n: usize) -> Groupe {
    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à compléter".into(), None, None)
}

/// `n` engagements complets pas encore valides (`A_VALIDER`) : ce qui reste a faire, en orange, sans geste ; la carte de
/// chacun porte « Valider l'engagement ».
fn a_valider(n: usize) -> Groupe {
    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à valider".into(), None, None)
}

/// Marque `suivante` la premiere etape qui a quelque chose a faire de notre cote (a faire, ou refuse), et donne a chaque
/// groupe sa phrase entiere (`phrase_du_groupe`).
fn suivante(mut etapes: Vec<Etape>) -> Vec<Etape> {
    let i = etapes.iter().position(|e| e.groupes.iter().any(|x| matches!(x.etat, EtatCase::AFaire | EtatCase::Refuse)));
    for (j, e) in etapes.iter_mut().enumerate() {
        e.suivante = Some(j) == i;
        let nom = e.nom;
        for g in &mut e.groupes {
            g.phrase = phrase_du_groupe(nom, g);
        }
    }
    etapes
}

/// LA PHRASE D'UNE CASE, entiere, telle qu'on la lit hors de la case (son titre, la ligne de commande) : l'etape, ce
/// qu'elle ecrit et son echeance (« DPAE : 2 à déposer, avant le 19/01/2026 »). Une etape qui n'a rien a faire dans un
/// mois sans engagement n'en a pas : la phrase du mois le dit une fois (`bilan_du_mois`).
fn phrase_du_groupe(nom: &str, g: &Groupe) -> Option<String> {
    if g.etat == EtatCase::Rien {
        return None;
    }
    let ecrit = [g.compte_texte.as_str(), g.quoi.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    let ecrit = if ecrit.is_empty() && g.etat == EtatCase::Fait { "fait".to_string() } else { ecrit };
    let echeance = g.echeance.as_ref().map(Echeance::texte).filter(|t| !t.is_empty()).map(|t| format!(", {}", t)).unwrap_or_default();
    Some(format!("{} : {}{}", nom, ecrit, echeance))
}

/// LA PHRASE DU MOIS : ce qu'en disent ses cases, l'une apres l'autre (`phrase_du_groupe`) ; un mois sans engagement le
/// dit une fois en tete (`debut`), sans redire ses etapes qui n'ont rien a faire.
fn phrase_du_mois(debut: Option<String>, etapes: &[Etape]) -> String {
    let cases = etapes.iter().flat_map(|e| &e.groupes).filter_map(|g| g.phrase.clone());
    let morceaux: Vec<String> = debut.into_iter().chain(cases).collect();
    format!("{}.", morceaux.join(". "))
}

// ------------------------------------------------------------ les colonnes
//
// Les cases des quatre lignes s'alignent sur les memes colonnes. G, la grande, porte ce qui est le plus avance ; une
// colonne de cote s'ouvre par vague de ce qui est en cours (des embauches nouvelles, un contrat qui change, un
// etablissement dont la DSN diverge), et une vague qui en est au meme point que G y rentre : on ne separe que le temps
// d'un changement. Chaque ligne fond les colonnes voisines qui en sont au meme point en une seule case.

/// Ou en est un engagement, ligne par ligne. Une ligne qui ne le concerne pas (la DPAE d'un contrat qui a commence un
/// mois d'avant, la signature d'un salarie au mois) est faite pour lui. `dsn` : la classe de son etablissement
/// (`groupes_dsn`).
/// Le point d'un engagement auquel il manque un fait, sur chacune de ses lignes.
const A_COMPLETER: &str = "a_completer";

/// Le point d'un engagement complet que personne n'a encore valide, sur chacune de ses lignes : rien ne part d'un
/// engagement en saisie (`routes/depots.rs`, `routes/documents.rs`).
const A_VALIDER: &str = "a_valider";

/// Le point, sur la ligne DPAE et sur la ligne Contrats, d'un contrat commence avant le logiciel : rien a faire ici.
const RIEN_A_DECLARER: &str = "rien_a_declarer";
const RIEN_A_SIGNER: &str = "rien_a_signer";

#[derive(Clone, Copy)]
struct Point {
    dpae: &'static str,
    contrat: &'static str,
    documents: &'static str,
    dsn: usize,
}

impl Point {
    fn de(p: &Vue, dsn: usize) -> Point {
        let embauche = p.contrat.premier_mois();
        // UN CONTRAT COMMENCE AVANT LE LOGICIEL (`engagement::stock::commence_avant_le_logiciel`) : sa DPAE et son contrat
        // se sont faits ailleurs ; ici, rien a declarer ni a signer.
        let anterieur = p.contrat.anterieur();
        // UN ENGAGEMENT A COMPLETER, jamais valide (`ligne_a_completer`) : rien n'est parti de lui, et tout ce qui partira
        // attend qu'il soit complet. Il reste a faire, en orange, pour qu'on ne l'oublie pas.
        if p.a_completer.is_some() {
            let spectacle = vrai(&p.spectacle);
            return Point {
                dpae: if anterieur { RIEN_A_DECLARER } else if embauche { A_COMPLETER } else { "fait" },
                contrat: if anterieur { RIEN_A_SIGNER } else if embauche && spectacle { A_COMPLETER } else { "fait" },
                documents: A_COMPLETER,
                dsn,
            };
        }
        // UN ENGAGEMENT EN SAISIE, complet : rien ne part de lui tant qu'il n'est pas valide ; chaque ligne qui
        // l'attend le dit.
        let ecrit = p.contrat.ecrit();
        let ailleurs = p.contrat.ailleurs();
        if !p.valide() {
            return Point {
                dpae: if anterieur { RIEN_A_DECLARER } else if embauche { A_VALIDER } else { "fait" },
                contrat: if anterieur || ailleurs { RIEN_A_SIGNER } else if embauche && ecrit { A_VALIDER } else { "fait" },
                documents: A_VALIDER,
                dsn,
            };
        }
        // LA DPAE A DEPOSER est celle que dit la liste des DPAE a faire (`Moteur::embauches_a_declarer`, `a_declarer`).
        let d = &p.etats.dpae;
        let dpae = if anterieur {
            RIEN_A_DECLARER
        } else if !embauche {
            "fait"
        } else if d.refusee {
            "refuse"
        } else if d.a_declarer {
            "a_faire"
        } else {
            "fait"
        };
        let s = &p.etats.signature;
        // Le contrat se fait signer au mois de l'embauche ; un avenant, a chaque mois du contrat : tant qu'il n'est pas
        // revenu signe, chacun de ses mois le dit.
        // Un contrat signe hors du logiciel ne se fait pas signer ici, ses avenants si.
        let contrat = if (ecrit && !embauche || ailleurs) && s.caduc {
            "a_refaire"
        } else if (ecrit && !embauche || ailleurs) && s.avenant_attendu {
            "en_attente"
        } else if anterieur || ailleurs {
            RIEN_A_SIGNER
        } else if !(embauche && ecrit) {
            "fait"
        } else if !s.parti() {
            "a_envoyer"
        } else if s.caduc {
            "a_refaire"
        } else if !vrai(&s.recu) || s.avenant_attendu {
            "en_attente"
        } else {
            "fait"
        };
        let o = &p.etats.documents;
        let documents = if o.caduc {
            "a_renvoyer"
        } else if o.parti() {
            "fait"
        } else {
            "a_envoyer"
        };
        Point { dpae, contrat, documents, dsn }
    }

    /// Sa vague : ce qui reste a faire de notre cote. Une DPAE partie l'est, revenue refusee ou non ; un contrat parti
    /// l'est, signe ou non.
    fn vague(&self) -> (bool, &'static str, &'static str, usize) {
        let contrat = if matches!(self.contrat, "a_envoyer" | "a_refaire") { self.contrat } else { "parti" };
        (self.dpae == "a_faire", contrat, self.documents, self.dsn)
    }

    /// Combien de ses lignes sont faites de notre cote : l'ordre des colonnes, la plus avancee en G.
    fn avance(&self, dsn_fait: &[bool]) -> usize {
        [!matches!(self.dpae, "a_faire" | A_COMPLETER | A_VALIDER), matches!(self.contrat, "fait" | "en_attente" | RIEN_A_SIGNER), self.documents == "fait", dsn_fait.get(self.dsn).copied().unwrap_or(false)]
            .iter()
            .filter(|x| **x)
            .count()
    }
}

/// Une colonne : son nom, la classe DSN de ses etablissements, et ses engagements (rangs dans `etat_par_engagement`).
struct Colonne {
    nom: String,
    dsn: usize,
    membres: Vec<usize>,
}

/// LES COLONNES DU MOIS. Les engagements d'une meme vague font une colonne ; la plus avancee est G. Une DPAE revenue
/// refusee ne reste pas dans G, qui dit ce qui est fait : elle sort a cote. Un etablissement sans engagement du mois
/// dont la DSN diverge a sa colonne. `dsn_fait` : par classe DSN, si elle est faite.
fn colonnes(points: &[Point], dsn_fait: &[bool]) -> Vec<Colonne> {
    let mut vagues: Vec<((bool, &'static str, &'static str, usize), Vec<usize>)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        match vagues.iter_mut().find(|(v, _)| *v == p.vague()) {
            Some((_, m)) => m.push(i),
            None => vagues.push((p.vague(), vec![i])),
        }
    }
    let mut cols: Vec<(usize, Vec<usize>)> = vagues.into_iter().map(|(v, m)| (v.3, m)).collect();
    cols.sort_by_key(|(_, m)| std::cmp::Reverse(points[m[0]].avance(dsn_fait)));
    if let Some((dsn, g)) = cols.first_mut() {
        let (refusees, reste): (Vec<usize>, Vec<usize>) = g.iter().partition(|i| points[**i].dpae == "refuse");
        if !refusees.is_empty() && !reste.is_empty() {
            let dsn = *dsn;
            *g = reste;
            cols.insert(1, (dsn, refusees));
        }
    }
    for c in 0..dsn_fait.len() {
        if !cols.iter().any(|(d, _)| *d == c) {
            cols.push((c, Vec::new()));
        }
    }
    if cols.is_empty() {
        cols.push((0, Vec::new()));
    }
    const COTE: &str = "ABCDEFHIJKLMNOPQRSTUVWXYZ";
    cols.into_iter()
        .enumerate()
        .map(|(k, (dsn, membres))| {
            let nom = match k {
                0 => "G".to_string(),
                k => COTE.chars().nth(k - 1).map_or_else(|| format!("C{}", k), String::from),
            };
            Colonne { nom, dsn, membres }
        })
        .collect()
}

/// Une ligne en cases : les colonnes voisines au meme point (`point`) se fondent. Chaque case : son point, ses
/// engagements, ses colonnes, et la classe DSN de sa premiere colonne.
fn cases_de_la_ligne(cols: &[Colonne], point: impl Fn(&Colonne) -> String) -> Vec<(String, Vec<usize>, Vec<String>, usize)> {
    let mut out: Vec<(String, Vec<usize>, Vec<String>, usize)> = Vec::new();
    for c in cols {
        let p = point(c);
        match out.last_mut() {
            Some((q, m, n, _)) if *q == p => {
                m.extend(&c.membres);
                n.push(c.nom.clone());
            }
            _ => out.push((p, c.membres.clone(), vec![c.nom.clone()], c.dsn)),
        }
    }
    out
}

/// Le point d'une colonne sur une ligne : le premier etat de `ordre` qu'un de ses engagements porte ; sinon `fait`, ou le
/// point que tous ses engagements partagent (rien a faire ici pour chacun).
fn point_de(c: &Colonne, points: &[Point], ligne: fn(&Point) -> &'static str, ordre: &[&'static str]) -> String {
    if let Some(o) = ordre.iter().find(|o| c.membres.iter().any(|i| ligne(&points[*i]) == **o)) {
        return o.to_string();
    }
    let mut tous = c.membres.iter().map(|i| ligne(&points[*i]));
    match tous.next() {
        Some(p) if tous.all(|q| q == p) => p.to_string(),
        _ => "fait".to_string(),
    }
}

// ------------------------------------------------------------ petits outils

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// Le nom d'un etablissement dans une case : sa commune (celle de la fiche entreprise pour le principal), sinon son NIC.
fn nom_de_l_etablissement(ent: &droit::fiche::ChampsEntreprise, nic: &str) -> String {
    let communes = if nic == ent.nic_principal() {
        Some([&ent.commune_affichage, &ent.commune])
    } else {
        ent.secondaires().into_iter().find(|e| e.nic.texte() == nic).map(|e| [&e.commune_affichage, &e.commune])
    };
    let commune = communes.and_then(|c| c.into_iter().map(|c| c.texte().trim()).find(|c| !c.is_empty()).map(String::from));
    commune.unwrap_or_else(|| format!("NIC {}", nic))
}

fn plur(k: usize, un: &'static str, des: &'static str) -> &'static str {
    if k <= 1 {
        un
    } else {
        des
    }
}

/// Un refus du chargement d'une saisie. Les refus que l'ecran ne tient pas
/// pour un defaut du logiciel -- le spectacle pas encore choisi, les jours ou
/// les heures pas encore saisis (`SaisieIncomplete`, leur liste est
/// `engagement::saisie_incomplete`) -- gardent leur classe ; tout autre est
/// celui du moteur.
pub fn classer_saisie(message: String) -> Echec {
    let incomplete = engagement::saisie_incomplete(&message);
    Echec { classe: if incomplete { Some("SaisieIncomplete") } else { None }, message }
}

/// `d["cle"]` sur un dictionnaire de fiche : `KeyError` si elle manque.
fn requis<'a>(d: &'a Value, cle: &str) -> R<&'a Value> {
    d.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn paire(cle: &Value, numero: &Value) -> Option<Paire> {
    Some((cle.as_str()?.to_string(), numero.as_str()?.to_string()))
}

/// Les `[salarie, numero, valeur]` rendus par `etats.rs` pour un dictionnaire a cle de contrat, en table. Une cle qui
/// n'est pas faite de deux textes ne peut egaler aucun engagement.
fn en_table(v: Value) -> HashMap<Paire, Value> {
    let mut out = HashMap::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b, x] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(k) = paire(a, b) {
                out.insert(k, x.clone());
            }
        }
    }
    out
}

fn cles_en_ensemble(v: Value) -> HashSet<Paire> {
    let mut out = HashSet::new();
    for t in v.as_array().cloned().unwrap_or_default() {
        if let [a, b] = t.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(k) = paire(a, b) {
                out.insert(k);
            }
        }
    }
    out
}

/// Quand le contrat (ou son dernier avenant) de l'engagement `numero` du salarie `cle` est parti (`departs`) ; rien s'il
/// n'est pas parti.
pub(crate) fn contrat_parti_le(departs: &engagement::departs::Departs, cle: &str, numero: Option<&str>) -> Option<String> {
    let n = numero?;
    departs.de(cle, n).iter().rev().find(|d| d.envoi == engagement::departs::Envoi::Contrat).map(|d| d.le.clone())
}

/// Ce qui est a refaire pour l'engagement `e` : le present compare a la version sur laquelle chaque envoi est parti
/// (`engagement::etats::a_refaire`) ; rien pour un engagement en saisie.
pub(crate) fn a_refaire_de(ctx: &Contexte, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> Vec<ARefaire> {
    e.valide().map(|v| engagement::etats::a_refaire(ctx, v, departs)).unwrap_or_default()
}

/// Les contrats de `mois` dont l'avenant est a faire signer : `(cle du salarie, numero)` des engagements dont les clauses
/// ont change depuis la version sur laquelle le contrat est parti (`a_refaire_parmi`, les contrats seuls).
pub(crate) fn avenants_a_faire(ctx: &Contexte, mois: &str) -> R<HashSet<(String, String)>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let etats = engagement::etats::du_mois(ctx, mois, &departs).map_err(plain)?;
    Ok(etats
        .iter()
        .filter(|e| {
            let avenant = |s: engagement::departs::Envoi| s == engagement::departs::Envoi::Contrat;
            e.valide().is_some_and(|v| engagement::etats::a_refaire_parmi(ctx, v, &departs, &avenant).iter().any(|a| matches!(a, ARefaire::Avenant { .. })))
        })
        .filter_map(|e| {
            let b = &*e.objet().brut;
            Some((String::from(engagement::cle_de_l_engagement(ctx, &b)?), b.get("numero").map(valeur_en_texte)?))
        })
        .collect())
}

/// La date d'embauche que la DPAE partie a declaree, quand elle n'est plus celle de l'engagement (`ARefaire::Dpae`).
pub(crate) fn debut_declare(refaire: &[ARefaire]) -> Option<String> {
    refaire.iter().find_map(|a| match a {
        ARefaire::Dpae { debut_declare, .. } => Some(debut_declare.clone()),
        _ => None,
    })
}

/// L'etablissement (NIC) dont releve l'engagement `e` pour `mois` : celui de la fiche de son salarie telle que le calcul
/// de sa paie du mois l'a lue (`fiches`) ; sans paie ecrite du mois, de sa fiche d'aujourd'hui.
pub(crate) fn nic_de_l_engagement(ctx: &Contexte, mois: &str, e: &engagement::etats::Etat) -> Option<String> {
    nic_de_l_engagement_ecrit(ctx, mois, &e.objet().brut)
}

/// `nic_de_l_engagement`, sur l'engagement tel qu'il est ecrit (`brut`).
pub(crate) fn nic_de_l_engagement_ecrit(ctx: &Contexte, mois: &str, brut: &Value) -> Option<String> {
    let fiche = match brut.pointer(&format!("/valide/paies/{}", mois)) {
        Some(ecrite) => droit::fiche::SalarieDuContrat::lire(ecrite.pointer("/fiches/paie").filter(|f| f.is_object())?).ok()?,
        None => {
            let mut f = droit::fiche::SalarieDuContrat::de(ctx.salarie(&engagement::cle_de_l_engagement(ctx, brut)?).ok()?);
            if let Some(n) = brut.get("etablissement").and_then(Value::as_str).filter(|n| !n.trim().is_empty()) {
                f.etablissement = Some(n.to_string());
            }
            f
        }
    };
    droit::bareme::etablissement_de(&ctx.entreprise, &fiche).ok()
}

/// La DSN de `mois` est-elle a refaire ? Une DSN est celle d'un etablissement (`derniere` : sa ligne du journal, son NIC
/// en `detail.etablissement`, le principal s'il est tu) : seuls comptent les engagements qui en relevent
/// (`nic_de_l_engagement`). La paie de l'un d'eux a change depuis la version sur laquelle elle est partie
/// (`refaire` : `Moteur::a_refaire_du_mois`), ou ils ne sont plus ceux qu'elle a emportes (`versions`) : un engagement ajoute, ou retire.
pub(crate) fn dsn_a_refaire(ctx: &Contexte, mois: &str, derniere: &Value, etats: &[engagement::etats::Etat], refaire: &mut dyn FnMut(&engagement::etats::Etat) -> Rc<Vec<ARefaire>>) -> bool {
    // Une ligne qui ne dit pas ce qu'elle a emporte ne se compare a rien.
    let Some(emportes) = derniere.get("versions").and_then(Value::as_array) else { return false };
    let nic = derniere
        .get("detail")
        .and_then(|d| d.get("etablissement"))
        .map(valeur_en_texte)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| ctx.entreprise.nic_principal());
    let siens: Vec<&engagement::etats::Etat> = etats.iter().filter(|e| nic_de_l_engagement(ctx, mois, e).as_deref() == Some(nic.as_str())).collect();
    if siens.iter().any(|e| refaire(e).iter().any(|a| matches!(a, ARefaire::Dsn { mois: ref m, .. } if m == mois))) {
        return true;
    }
    let paire = |v: &Value| (valeur_en_texte(&v["salarie"]), valeur_en_texte(&v["numero"]));
    // Les corrections de mois passes que la ligne porte ne sont pas des engagements de ce mois.
    let mut emportes: Vec<(String, String)> = emportes.iter().filter(|v| v.get("mois").is_none()).map(paire).collect();
    let mut presents: Vec<(String, String)> = siens
        .iter()
        .filter(|e| e.valide().is_some())
        .filter_map(|e| {
            let b = &*e.objet().brut;
            Some((String::from(engagement::cle_de_l_engagement(ctx, &b)?), b.get("numero").map(valeur_en_texte)?))
        })
        .collect();
    emportes.sort();
    presents.sort();
    emportes != presents
}

/// La saisie en JSON, sans les cles
/// `_...`, ni `valide` -- ce qui change dans une saisie sans
/// changer la paie.
pub(crate) fn sans_bruit(x: &Value) -> Value {
    match x {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, _)| !k.starts_with('_') && *k != "valide")
                .map(|(k, v)| (k.clone(), sans_bruit(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(sans_bruit).collect()),
        autre => autre.clone(),
    }
}

pub(crate) fn empreinte_de(v: &Value) -> String {
    sha256_hex(socle::ecriture::json_trie(&sans_bruit(v)).as_bytes())[..16].to_string()
}

/// L'ORDRE DES FICHIERS DEPOSABLES D'UN MOIS (`fichiers_deposables`), le meme partout : plus jamais la date de
/// modification du fichier, que deux machines -- ou deux rejeux -- ne donnent pas pareille.
/// - Les DSN : D'ABORD LA MENSUELLE (nature 01 ; un etablissement par fichier, le principal `DSN_<Mois>.dsn` avant les
///   `DSN_<Mois>_<NIC>.dsn`, par le nom) : c'est « la DSN du mois » que l'ecran et l'echeance attendent. D'un meme
///   etablissement, LA DERNIERE ECRITE D'ABORD : un annule-et-remplace s'ecrit a cote de la DSN deja
///   partie, sous `DSN_<Mois>_AR<ordre>.dsn` (`depart::fabriquer_dsn`), et c'est lui qui part ; il se reconnait a son numero
///   d'ordre (S20.G00.05.004, ou `_AR<n>` du nom), le plus grand d'abord. Rangees par le seul nom, la DSN deja deposee
///   (`DSN_Janvier2025.dsn`) passait avant son annule-et-remplace (`DSN_Janvier2025_AR2.dsn`) : la boite « Deposer » et
///   l'etape DSN montraient, testaient et auraient redepose l'ancienne. PUIS LES SIGNALEMENTS (arret, fin de contrat,
///   amorcage...), dans l'ordre de leur numero d'ordre (S20.G00.05.004) : unique et jamais remis a zero, il dit l'ordre ou
///   le moteur les a produits (`ordre_signalements`, cahier technique DSN 2026.1 p. 136), et chacun part a son tour.
/// - Les DPAE : la plus recemment produite d'abord, lue dans son nom (`DPAE_<Mois>_<AAAAMMJJ-HHMM>.txt` apres
///   `DPAE_<Mois>.txt` : l'ordre inverse des noms).
pub(crate) fn ordre_de_depot(nom: &str, chemin: &Path) -> (u8, i64, String, std::cmp::Reverse<String>) {
    if suffixe(nom).to_lowercase() == ".dsn" {
        let apercu = std::fs::read(chemin).map(|o| envois::depot::lire(&o)).ok();
        let signalement = apercu.as_ref().is_some_and(|a| a.est_signalement());
        let lu = apercu.as_ref().and_then(|a| a.champ("ordre").trim().parse::<i64>().ok());
        if signalement {
            return (1, lu.unwrap_or(0), nom.to_string(), std::cmp::Reverse(String::new()));
        }
        let (etablissement, du_nom) = sans_annule_et_remplace(nom);
        return (0, 0, etablissement, std::cmp::Reverse(format!("{:020}", lu.or(du_nom).unwrap_or(1).max(0))));
    }
    (0, 0, String::new(), std::cmp::Reverse(nom.to_string()))
}

/// `DSN_<Mois>[_<NIC>]_AR<n>.dsn` -> (`DSN_<Mois>[_<NIC>].dsn`, Some(n)) ; un autre nom reste tel quel.
fn sans_annule_et_remplace(nom: &str) -> (String, Option<i64>) {
    if let Some(corps) = nom.strip_suffix(".dsn").or_else(|| nom.strip_suffix(".DSN")) {
        if let Some(i) = corps.rfind("_AR") {
            if let Ok(n) = corps[i + 3..].parse::<i64>() {
                return (format!("{}.dsn", &corps[..i]), Some(n));
            }
        }
    }
    (nom.to_string(), None)
}

/// L'extension d'un nom de fichier.
fn suffixe(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[i..],
        _ => "",
    }
}

/// Le nom d'un fichier sans sa dernière extension.
fn radical(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[..i],
        _ => nom,
    }
}

/// Ce qu'une saisie chargee sans calcul porte : une « embauche » par engagement (le salarie, le spectacle, les dates,
/// la nature et le numero du contrat ; aucun montant).
pub(crate) struct SaisieMois {
    pub(crate) paies: Vec<Paie>,
    /// L'identite de chaque engagement de `paies` (`engagement::stock::identites`, sur tous les engagements du mois).
    pub(crate) identites: Vec<Value>,
    /// Ce qui manque a chaque engagement du mois (`None` : rien), dans l'ordre de `stock::objets_du_mois` :
    /// `paies` ne porte que ceux qui ne manquent de rien.
    pub(crate) manques: Vec<Option<String>>,
}

/// Les etats du journal et de la saisie, calcules une fois par processus : ce que le journal des envois dit de chaque
/// contrat (`envois::etats`), lu tel qu'il est.
pub(crate) struct Globales {
    pub(crate) dpae: HashMap<Paire, Value>,
    pub(crate) recus: HashMap<Paire, Value>,
    pub(crate) renvoyes: HashSet<Paire>,
    pub(crate) avenants: HashMap<Paire, Value>,
    /// Comment chaque contrat est parti la derniere fois : `imprime` ou `courriel`.
    pub(crate) par: HashMap<Paire, Value>,
}

// ------------------------------------------------------------ l'etat d'un engagement

/// Les echeances d'un engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche de la
/// convention), `droit::echeances`.
#[derive(Debug, Clone)]
struct Echeances {
    dpae: Echeance,
    contrat: Echeance,
}

/// Ce que la carte d'un engagement dit de son contrat : entier pour un engagement qui ne manque de rien, son premier
/// mois et s'il commence avant le logiciel pour celui auquel il manque un fait.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ContratDit {
    Complet(ContratComplet),
    ACompleter(ContratACompleter),
}

/// `chevrons` : `routes::mois::chevrons_dits` ; `dossier` : celui d'un contrat signe ailleurs
/// (`signature::contrat_signe_dit`), lus tels qu'ils sont.
#[derive(Debug, Clone, Serialize)]
struct ContratComplet {
    premier_mois: bool,
    dernier_mois: bool,
    chevrons: Value,
    depuis: String,
    jusqu_a: String,
    repris: bool,
    ecrit: bool,
    ailleurs: bool,
    dossier: Value,
    anterieur: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ContratACompleter {
    premier_mois: bool,
    repris: bool,
    anterieur: bool,
}

impl ContratDit {
    fn premier_mois(&self) -> bool {
        match self {
            ContratDit::Complet(c) => c.premier_mois,
            ContratDit::ACompleter(c) => c.premier_mois,
        }
    }

    fn anterieur(&self) -> bool {
        match self {
            ContratDit::Complet(c) => c.anterieur,
            ContratDit::ACompleter(c) => c.anterieur,
        }
    }

    fn ecrit(&self) -> bool {
        matches!(self, ContratDit::Complet(c) if c.ecrit)
    }

    fn ailleurs(&self) -> bool {
        matches!(self, ContratDit::Complet(c) if c.ailleurs)
    }

    fn depuis(&self) -> &str {
        match self {
            ContratDit::Complet(c) => &c.depuis,
            ContratDit::ACompleter(_) => "",
        }
    }

    fn dossier(&self) -> &Value {
        match self {
            ContratDit::Complet(c) => &c.dossier,
            ContratDit::ACompleter(_) => &Value::Null,
        }
    }
}

/// Une date d'embauche changee depuis la version sur laquelle la DPAE est partie : l'ancienne, si sa DPAE vaut, et la
/// phrase qui le dit (`regularisation::dpae_d_une_date_changee`).
#[derive(Debug, Clone, Serialize)]
struct DateChangee {
    avant: String,
    vaut: bool,
    phrase: String,
}

/// LA DPAE D'UN ENGAGEMENT : a declarer (la liste des DPAE a faire), partie (`fait` : quand, ou quand elle a ete
/// refusee), et ce que le journal des envois en dit (`envois::etats::etat_des_embauches`, lu tel qu'il est).
#[derive(Debug, Clone, Default)]
struct EtatDpae {
    date_changee: Option<DateChangee>,
    a_declarer: bool,
    fait: Value,
    passee: bool,
    refusee: bool,
    anomalies: Value,
    dossier: Value,
    deja_existante: bool,
}

/// Un CDI en cours, d'un mois d'apres son embauche : le mois de sa signature.
#[derive(Debug, Clone)]
pub(crate) struct CdiEnCours {
    signe_en: String,
}

/// LA PASTILLE CONTRAT D'UN ENGAGEMENT (`etat_signature`). `fait` : quand le contrat (ou son dernier avenant) est parti ;
/// `par`, `recu`, `avenant` : ce que le journal des envois en dit (`envois::etats`), lu tel qu'il est.
#[derive(Debug, Clone, Default)]
pub(crate) struct Signature {
    pub(crate) fait: Option<String>,
    pub(crate) par: Value,
    pub(crate) recu: Value,
    pub(crate) caduc: bool,
    pub(crate) pourquoi: &'static str,
    pub(crate) avenant: Value,
    pub(crate) avenant_attendu: bool,
    cdi_en_cours: Option<CdiEnCours>,
}

impl Signature {
    /// Le contrat est parti.
    pub(crate) fn parti(&self) -> bool {
        self.fait.as_deref().is_some_and(|f| !f.is_empty())
    }
}

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois (`etat_documents`).
#[derive(Debug, Clone, Default)]
struct EtatDocuments {
    fait: Option<String>,
    caduc: bool,
    pourquoi: &'static str,
}

impl EtatDocuments {
    fn parti(&self) -> bool {
        self.fait.as_deref().is_some_and(|f| !f.is_empty())
    }
}

/// Les etats des envois d'un engagement ; rien, pour un engagement auquel il manque un fait.
#[derive(Debug, Clone, Default)]
struct Etats {
    dpae: EtatDpae,
    signature: Signature,
    documents: EtatDocuments,
}

/// CE QUE LES ETAPES LISENT D'UN ENGAGEMENT DU MOIS : qui, quoi, quand, son contrat, ses echeances, son etablissement,
/// ce qu'il recoit (`pieces`), les etats de ses envois. `a_completer` : ce qui lui manque, s'il lui manque un fait (rien
/// ne se calcule sur lui). `spectacle` : le titre que sa fiche dit, lu tel qu'il est.
#[derive(Debug, Clone)]
pub(crate) struct Vue {
    salarie: String,
    nom: String,
    spectacle: Value,
    nature: String,
    numero: Option<NumeroContrat>,
    debut: String,
    fin: Option<String>,
    etablissement: Option<String>,
    echeances: Option<Echeances>,
    valide: Option<String>,
    recalcul: Option<String>,
    contrat: ContratDit,
    pieces: Vec<&'static str>,
    etats: Etats,
    a_completer: Option<String>,
}

impl Vue {
    fn valide(&self) -> bool {
        self.valide.as_deref().is_some_and(|v| !v.is_empty())
    }

    /// Le numero du contrat, ou `""` (la charge d'un geste qui le nomme).
    fn numero_ou_vide(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }
}

/// LA LIGNE D'UN ENGAGEMENT (`etat_par_engagement`) : ce que les etapes lisent (`vue`), ses carres et le choix de sa
/// DPAE, son identite (`engagement::stock::identites`) et sa frise (`engagement::versions::frise`), lues telles qu'elles
/// sont.
#[derive(Debug, Clone)]
pub(crate) struct Ligne {
    vue: Vue,
    carres: Vec<Carre>,
    dpae_choix: Choix,
    versions: Value,
    engagement: Value,
}

impl Ligne {
    fn de(vue: Vue, mois: &str, versions: Value, engagement: Value) -> Ligne {
        Ligne { carres: carres(&vue, mois), dpae_choix: choix_dpae(&vue), vue, versions, engagement }
    }
}

/// La ligne d'un engagement telle que la page la lit : son identite, sa carte fermee (qui, quand, pour quoi, le contrat,
/// la validation, le recalcul, la frise), ses carres et le choix de sa DPAE, la phrase d'une date d'embauche changee, et
/// ce que la carte, la boite des DPAE et le detail d'une piece en disent (`periode_dit`, `valide_dit`, `validee_dit`,
/// `repris_dit`, `embauche_dit`, `numero_dit`). Le reste (echeances, pieces, etablissement, etats des envois) ne sert
/// qu'aux etapes du mois.
#[derive(Debug, Serialize)]
struct LigneDePage<'a> {
    engagement: &'a Value,
    salarie: &'a str,
    nom: &'a str,
    spectacle: &'a Value,
    nature: &'a str,
    numero: Option<&'a str>,
    debut: &'a str,
    fin: Option<&'a str>,
    valide: Option<&'a str>,
    recalcul: Option<&'a str>,
    contrat: &'a ContratDit,
    versions: &'a Value,
    carres: &'a [Carre],
    dpae_choix: &'a Choix,
    etats: EtatsDePage<'a>,
    periode_dit: String,
    valide_dit: Option<String>,
    validee_dit: Option<String>,
    repris_dit: Option<&'static str>,
    embauche_dit: String,
    numero_dit: Option<String>,
}

#[derive(Debug, Serialize)]
struct EtatsDePage<'a> {
    dpae: DpaeDePage<'a>,
}

#[derive(Debug, Serialize)]
struct DpaeDePage<'a> {
    date_changee: Option<&'a DateChangee>,
}

impl<'a> LigneDePage<'a> {
    fn de(l: &'a Ligne) -> LigneDePage<'a> {
        let v = &l.vue;
        let valide = v.valide.as_deref().filter(|t| !t.is_empty()).map(super::quand);
        let embauche = format!("embauche le {}", super::jour_en_clair(&json!(v.debut)));
        LigneDePage {
            periode_dit: super::periode_en_clair(&json!(v.debut), &json!(v.fin)),
            valide_dit: valide.as_ref().map(|q| format!("le {}", q)),
            validee_dit: valide.as_ref().map(|q| format!("validée le {}", q)),
            repris_dit: matches!(&v.contrat, ContratDit::Complet(c) if c.repris).then_some("contrat repris"),
            embauche_dit: if vrai(&v.spectacle) { format!("{}, {}", valeur_en_texte(&v.spectacle), embauche) } else { embauche },
            numero_dit: v.numero.as_ref().map(|n| format!("contrat n° {}", n)),
            engagement: &l.engagement,
            salarie: &v.salarie,
            nom: &v.nom,
            spectacle: &v.spectacle,
            nature: &v.nature,
            numero: v.numero.as_deref(),
            debut: &v.debut,
            fin: v.fin.as_deref(),
            valide: v.valide.as_deref(),
            recalcul: v.recalcul.as_deref(),
            contrat: &v.contrat,
            versions: &l.versions,
            carres: &l.carres,
            dpae_choix: &l.dpae_choix,
            etats: EtatsDePage { dpae: DpaeDePage { date_changee: v.etats.dpae.date_changee.as_ref() } },
        }
    }
}

/// UN CARRE DE LA LIGNE D'UN ENGAGEMENT (`carres`) : son nom, son etat, sa phrase (l'infobulle), le geste qu'ouvre un
/// clic, ce que montre son detail (`pieces`, DPAE et Contrat), sa couleur, ce qui reste en un mot, le lien vers ce qui est
/// parti, et le titre de sa boite. Un carre qui attend qu'on complete ou valide l'engagement n'a ni detail, ni coupure, ni
/// lien.
#[derive(Debug, Clone, Serialize)]
struct Carre {
    cle: QuelleEtape,
    nom: String,
    etat: EtatCase,
    phrase: String,
    action: Option<Geste>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pieces: Option<Pieces>,
    couleur: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    coupee: Option<bool>,
    reste: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lien: Option<Option<&'static str>>,
    titre: &'static str,
}

/// Le geste d'un carre : une route du catalogue, ou le dossier d'un contrat signe hors du logiciel (`geste_du_dossier`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum Geste {
    Route(Action),
    Dossier(GesteDuDossier),
}

/// Le dossier d'un contrat signe hors du logiciel : la fenetre l'ouvre et le cree ; `chemin` et `phrase` sont ceux que
/// `signature::contrat_signe_dit` rend, lus tels qu'ils sont.
#[derive(Debug, Clone, Serialize)]
struct GesteDuDossier {
    libelle: &'static str,
    chemin: Value,
    phrase: Value,
}

/// Ce que montre le detail d'un carre (`pieces_du_contrat`).
#[derive(Debug, Clone, Serialize)]
struct Pieces {
    vide: Option<String>,
    encart: Option<EncartDePiece>,
    gestes: Vec<GesteDePiece>,
}

#[derive(Debug, Clone, Serialize)]
struct EncartDePiece {
    titre: &'static str,
    texte: String,
}

#[derive(Debug, Clone, Serialize)]
struct GesteDePiece {
    libelle: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    titre: Option<&'static str>,
    action: Option<Geste>,
    principal: bool,
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT (`choix_dpae`).
#[derive(Debug, Clone, Serialize)]
struct Choix {
    cochable: bool,
    cochee: bool,
    etat: EtatCase,
    couleur: &'static str,
    phrase: String,
}

/// UNE DPAE A FAIRE (`embauches_a_declarer`) : le mois, le contrat, qui, pour quoi, ses bornes, l'empreinte de ce qu'elle
/// declare, et pourquoi quand une date d'embauche a change. `spectacle` : le titre que sa fiche dit, lu tel qu'il est.
#[derive(Debug, Clone, Serialize)]
pub struct DpaeDue {
    pub mois: Mois,
    pub salarie: String,
    pub numero: NumeroContrat,
    pub nom: String,
    pub spectacle: Value,
    pub debut: String,
    pub fin: Option<String>,
    pub empreinte: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pourquoi: Option<String>,
}

/// UN FICHIER DEPOSABLE D'UN MOIS (`fichiers_deposables`) : son chemin.
#[derive(Debug, Clone)]
pub(crate) struct Deposable {
    pub(crate) chemin: String,
}

/// Les fichiers deposables d'un mois, par acte, le dernier ecrit en tete.
#[derive(Debug, Clone, Default)]
pub(crate) struct Deposables {
    pub(crate) dsn: Vec<Deposable>,
    dpae: Vec<Deposable>,
}

impl Deposables {
    fn de(&self, quoi: &str) -> &[Deposable] {
        if quoi == DSN { &self.dsn } else { &self.dpae }
    }
}

/// OU EN EST LE MOIS VIS-A-VIS DE L'ANNULE-ET-REMPLACE (`etat_remplacement`) : deposee, son dernier ordre et le prochain,
/// la limite et si elle est passee, si la paie a change depuis (`saisie_modifiee`, rien si rien ne se compare), si un
/// annule-et-remplace est a envoyer, si la DSN suivante porte la correction, et la phrase qui le dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Remplacement {
    mois: Mois,
    pub(crate) deposee: bool,
    pub(crate) dernier_ordre: Option<i64>,
    prochain_ordre: i64,
    limite: Jour,
    admis: bool,
    saisie_modifiee: Option<bool>,
    pub(crate) a_envoyer: bool,
    phrase: String,
    corrigee_par_la_suivante: bool,
}

/// Tout ce qu'une route lit, lu une fois.
pub struct Moteur<'a> {
    ctx: &'a Contexte,
    globales: Option<Rc<Globales>>,
    fichiers: HashMap<String, Rc<Deposables>>,
    /// LES ENGAGEMENTS TELS QU'ILS SONT ECRITS, sans recalcul d'eux-memes (`engagement::etats::tels_qu_ecrits`) : ce que
    /// lit `mois_a_ouvrir`, qui regarde chaque mois passe a l'ouverture et ne calcule rien.
    ecrit: bool,
    /// CE QU'IL A LU D'UN MOIS, une fois pour sa vie (une route, l'etat garde d'un mois) : la saisie chargee
    /// (`charger_saisie`), ce qui est a refaire de chaque engagement valide (`a_refaire_du_mois`, par mois, salarie et
    /// numero) et l'etat de chaque engagement (`etat_par_engagement`).
    saisies: HashMap<String, Rc<SaisieMois>>,
    refaire: HashMap<(String, String, String), Rc<Vec<ARefaire>>>,
    lignes: HashMap<String, Rc<Vec<Ligne>>>,
}

impl<'a> Moteur<'a> {
    pub fn nouveau(ctx: &'a Contexte) -> Moteur<'a> {
        Moteur { ctx, globales: None, fichiers: HashMap::new(), ecrit: false, saisies: HashMap::new(), refaire: HashMap::new(), lignes: HashMap::new() }
    }

    /// Ce qui est parti (`envois::journal::lu`, tire une fois par contenu du journal) : ce que le moteur passe a
    /// l'engagement, qui ne lit pas le journal.
    pub(crate) fn departs(&self) -> R<engagement::departs::Departs> {
        envois::journal::lu(&self.ctx.racine).map(|j| j.departs).map_err(plain)
    }

    /// Les engagements de `mois`, dans l'ordre de l'ecran : recalcules d'eux-memes s'il le faut (`du_mois`), ou tels
    /// qu'ils sont ecrits (`ecrit`).
    pub(crate) fn etats_du_mois(&self, mois: &str) -> R<Vec<engagement::etats::Etat>> {
        let departs = self.departs()?;
        if self.ecrit {
            return engagement::etats::tels_qu_ecrits(self.ctx, mois, &departs).map_err(plain);
        }
        engagement::etats::du_mois(self.ctx, mois, &departs).map_err(plain)
    }

    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    /// Les lignes de `journal/depots.jsonl` (`envois::journal::lu`, la lecture qui donne aussi `departs`).
    pub(crate) fn lignes(&self) -> R<std::sync::Arc<Vec<Value>>> {
        envois::journal::lu(self.racine()).map(|j| j.lignes).map_err(plain)
    }

    fn nic(&self) -> String {
        self.ctx.entreprise.nic_principal()
    }

    // ------------------------------------------------------ la saisie du mois

    /// La saisie d'un mois, chargee sans calculer la paie : ses engagements qui ne manquent de rien
    /// (`mois::manques_de_la_saisie`), et ce qui manque a chacun de ses engagements, dans son ordre.
    fn charger_saisie(&mut self, mois: &str) -> R<Rc<SaisieMois>> {
        if let Some(s) = self.saisies.get(mois) {
            return Ok(s.clone());
        }
        let s = Rc::new(self.saisie_chargee(mois)?);
        self.saisies.insert(mois.to_string(), s.clone());
        Ok(s)
    }

    /// CE QUI EST A REFAIRE de l'engagement `e` de `mois` (`a_refaire_de`), compare une fois.
    fn a_refaire_du_mois(&mut self, mois: &str, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> Rc<Vec<ARefaire>> {
        let brut = &*e.objet().brut;
        let cle = (mois.to_string(), engagement::cle_de_l_engagement(self.ctx, brut).map(String::from).unwrap_or_default(), brut.get("numero").map(valeur_en_texte).unwrap_or_default());
        if e.valide().is_none() || cle.2.is_empty() {
            return Rc::new(Vec::new());
        }
        if let Some(r) = self.refaire.get(&cle) {
            return r.clone();
        }
        let r = Rc::new(a_refaire_de(self.ctx, e, departs));
        self.refaire.insert(cle, r.clone());
        r
    }

    fn saisie_chargee(&mut self, mois: &str) -> R<SaisieMois> {
        let mut data = engagement::vue_requise(self.racine(), mois).map_err(classer_saisie)?;
        let manques = super::mois::manques_de_la_saisie(self.ctx, &data);
        let objets = engagement::stock::objets_du_mois(self.racine(), mois).map_err(plain)?;
        let mut identites = engagement::stock::identites(&objets);
        let mut m = manques.iter();
        identites.retain(|_| m.next().is_some_and(Option::is_none));
        if let Some(Value::Array(es)) = data.get_mut("engagements") {
            let mut m = manques.iter();
            es.retain(|_| m.next().is_some_and(Option::is_none));
        }
        let mut paies = engagement::lue_sans_paie(self.ctx, &data).map_err(classer_saisie)?;
        engagement::numeros::numeroter_les_contrats(self.ctx, &mut paies, &self.departs()?).map_err(plain)?;
        Ok(SaisieMois { paies, identites, manques })
    }

    // --------------------------------------------- ce que le journal et la saisie disent

    pub(crate) fn globales(&mut self) -> R<Rc<Globales>> {
        if let Some(g) = &self.globales {
            return Ok(g.clone());
        }
        let lignes = self.lignes()?;
        let dpae = en_table(etats::etat_des_embauches(&lignes)?);
        let recus = etats::contrats_recus(&lignes)?;
        let recus = en_table(recus.sortie(Clone::clone));
        let renvoyes = cles_en_ensemble(etats::renvoyes_depuis_la_signature(&lignes)?.cles());
        let avenants = en_table(etats::avenants(&lignes)?);
        let par = en_table(etats::contrats_partis_par(&lignes)?);
        let g = Rc::new(Globales { dpae, recus, renvoyes, avenants, par });
        self.globales = Some(g.clone());
        Ok(g)
    }

    // ------------------------------------------------------------ etat_par_engagement

    /// `etat_par_engagement` : ou en est CHAQUE engagement du mois (ce que les etapes lisent ; la route n'en rend que ce
    /// que la page lit, `LigneDePage`).
    pub(crate) fn etat_par_engagement(&mut self, mois: &str) -> R<Rc<Vec<Ligne>>> {
        if let Some(l) = self.lignes.get(mois) {
            return Ok(l.clone());
        }
        let l = Rc::new(self.lignes_du_mois(mois)?);
        self.lignes.insert(mois.to_string(), l.clone());
        Ok(l)
    }

    fn lignes_du_mois(&mut self, mois: &str) -> R<Vec<Ligne>> {
        // UN MOIS SANS ENGAGEMENT N'EST PAS UNE ERREUR.
        if !engagement::stock::a_une_saisie(self.racine(), mois).map_err(plain)? {
            return Ok(Vec::new());
        }
        let s = self.charger_saisie(mois)?;
        // Valide, ou parti : la date de la validation (`engagement::etats`), engagement par engagement ; chacun retrouve son
        // embauche par son identite (`engagement::stock::identites`).
        let etats_eng = self.etats_du_mois(mois)?;
        let ids = engagement::stock::identites(&etats_eng.iter().map(|e| e.objet().clone()).collect::<Vec<_>>());
        let g = self.globales()?;
        let departs = self.departs()?;
        // Ce qui est a refaire de chacun, compare une fois : la liste des DPAE, la ligne et la DSN du mois le lisent.
        for e in &etats_eng {
            self.a_refaire_du_mois(mois, e, &departs);
        }
        // LES DPAE A DEPOSER DU MOIS : la seule liste (`embauches_a_declarer`), que lisent aussi les rappels et le depot.
        let a_declarer: HashSet<Paire> = self.embauches_a_declarer(Some(mois))?.into_iter().map(|d| (d.salarie, d.numero.to_string())).collect();

        // Chaque ligne porte `engagement` : l'identite par laquelle sa carte le designe. Un engagement auquel il manque un
        // fait a sa ligne a lui (`ligne_a_completer`) ; les autres retrouvent leur paie parmi ceux qui ne manquent de rien.
        let mut out = Vec::new();
        for (j, (e, id)) in etats_eng.iter().zip(&ids).enumerate() {
            if let Some(Some(manque)) = s.manques.get(j) {
                let vue = self.ligne_a_completer(mois, e.objet(), manque)?;
                out.push(Ligne::de(vue, mois, Value::Null, id.clone()));
                continue;
            }
            let Some(k) = s.identites.iter().position(|x| x == id) else { continue };
            let p = &s.paies[k];
            let cle = p.salarie.id.to_string();
            let debut = p.engagement.debut.iso();
            // CE QUI EST A REFAIRE : le present compare a la version sur laquelle chaque envoi est parti.
            let refaire = self.a_refaire_du_mois(mois, e, &departs);
            let sal = &p.salarie;
            let nat = p.engagement.nat().map_err(plain)?;
            let spectacle = if vrai(&p.spectacle) { requis(&p.spectacle, "titre")?.clone() } else { Value::Null };
            let mois_du_contrat = p.engagement.mois_du_contrat().map_err(plain)?;
            let (Some(depuis), Some(jusqu_a)) = (mois_du_contrat.first().cloned(), mois_du_contrat.last().cloned()) else {
                return Err(erreur("IndexError", socle::fichiers::trop_courte("les mois du contrat")));
            };
            let premier_mois = p.engagement.premier_mois().map_err(plain)?;
            let dernier_mois = p.engagement.dernier_mois().map_err(plain)?;
            // Son etablissement (NIC) : la DSN qui le porte.
            let etablissement = nic_de_l_engagement(self.ctx, mois, e);
            // LES ECHEANCES de l'engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche
            // de la convention), `droit::echeances`. Les documents n'en ont pas : chaque compagnie les envoie quand elle
            // veut, souvent apres la DSN qui fait remonter l'AER. Un mois passe lu a l'ouverture (`ecrit`) ne les demande pas.
            let echeances = if self.ecrit {
                None
            } else {
                let code_emploi = p.engagement.code_emploi_ft.clone().unwrap_or_else(|| sal.code_emploi_ft.texte().to_string());
                let ent = &self.ctx.entreprise;
                let remise = droit::echeances::remise_du_contrat(&p.engagement.debut, ent.idcc.texte(), &code_emploi, ent.feries_chomes()).map_err(plain)?;
                Some(Echeances { dpae: droit::echeances::dpae(&p.engagement.debut), contrat: remise })
            };
            // RECALCULEE D'ELLE-MEME (`engagement::etats::du_mois`) : pourquoi, et quand.
            let recalcul = e.valide().and_then(|v| v.recalcul(mois)).as_ref().and_then(engagement::etats::phrase_du_recalcul);
            // REPRIS D'UN ANCIEN LOGICIEL : la carte le dit (`engagement::importe::vient_d_un_ancien_logiciel`).
            let repris = engagement::importe::vient_d_un_ancien_logiciel(&self.ctx.racine, &p.engagement.salarie, &p.engagement.debut.iso(), p.engagement.numero.as_deref().unwrap_or(""));
            // SIGNE AILLEURS : son contrat ecrit ne se fait pas signer ici, ses avenants si (`engagement::signe_ailleurs`).
            let ailleurs = engagement::signe_ailleurs(&self.ctx.racine, &p.engagement).map_err(plain)?;
            // ECRIT : le contrat de cet engagement se fait signer ici (`engagement::contrat_a_signer`).
            let contrat = ContratDit::Complet(ContratComplet {
                premier_mois,
                dernier_mois,
                chevrons: super::mois::chevrons_dits(&p.engagement)?,
                depuis,
                jusqu_a,
                repris,
                ecrit: engagement::contrat_a_signer(&self.ctx.racine, &p.engagement).map_err(plain)?,
                ailleurs,
                dossier: if ailleurs { crate::compagnie::signature::contrat_signe_dit(self.ctx, &cle, &p.engagement.debut.iso()) } else { Value::Null },
                anterieur: engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, &p.engagement.debut.iso(), mois).map_err(plain)?,
            });
            // CE QUE CE SALARIE RECOIT POUR CETTE PAIE : le bulletin s'il y a une paie, le certificat Conges Spectacles
            // du contrat declare a la caisse qui finit (`crate::routes::dossier_salarie::certificats_dus`), l'AER Cinema
            // Spectacle qui lui est due (`aer::due`). L'etape « Documents » du mois les nomme d'apres ceci, et rien d'autre.
            let mut pieces: Vec<&'static str> = Vec::new();
            if !p.engagement.sans_paie() {
                pieces.push("bulletin");
            }
            if dernier_mois && p.declaree_a_la_caisse_des_conges().map_err(plain)? {
                pieces.push("certificat_cs");
            }
            if documents::aer::due(p).map_err(plain)? {
                pieces.push("aer");
            }
            // LA DPAE DE CET ENGAGEMENT : partie (`departs`), et ce qui en est revenu (la ligne qui la porte, par le numero du
            // contrat). Refusee au bilan, elle n'est pas partie ; la ligne dit quand elle a ete refusee.
            let numero = p.engagement.numero.as_deref().unwrap_or_default().to_string();
            let mut dpae_v = g.dpae.get(&(cle.clone(), numero.clone())).filter(|v| vrai(v)).unwrap_or(objet_vide());
            let mut partie_le = departs.de(&cle, &numero).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dpae).map(|d| d.le.clone()).max();
            // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie (`ARefaire::Dpae`) : la DPAE de
            // l'ancienne date vaut-elle (`crate::regularisation::dpae_d_une_date_changee`) ?
            let mut date_changee = None;
            if let Some(avant) = debut_declare(&refaire) {
                let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&avant, &debut);
                if !vaut {
                    dpae_v = objet_vide();
                    partie_le = None;
                }
                date_changee = Some(DateChangee { avant, vaut, phrase });
            }
            let fait = match partie_le {
                Some(le) => Value::String(le),
                None if vrai(get(dpae_v, "refusee")?) => get(dpae_v, "quand")?.clone(),
                None => Value::Null,
            };
            let dpae = EtatDpae {
                date_changee,
                a_declarer: a_declarer.contains(&(cle.clone(), numero.clone())),
                fait,
                passee: vrai(get(dpae_v, "passee")?),
                refusee: vrai(get(dpae_v, "refusee")?),
                anomalies: ou(get(dpae_v, "anomalies")?, &LISTE_VIDE).clone(),
                dossier: ou(get(dpae_v, "dossier")?, texte_vide()).clone(),
                deja_existante: vrai(get(dpae_v, "deja_existante")?),
            };
            let k = (cle.clone(), numero.clone());
            let nul = Value::Null;
            let avenant = refaire.iter().any(|a| matches!(a, ARefaire::Avenant { .. }));
            let mut signature = etat_signature(
                contrat_parti_le(&departs, &cle, p.engagement.numero.as_deref()),
                g.par.get(&k).unwrap_or(&nul),
                g.recus.get(&k).unwrap_or(&nul),
                g.renvoyes.contains(&k),
                g.avenants.get(&k).unwrap_or(&nul),
                avenant,
                ailleurs,
            );
            // UN CDI EN COURS N'A PAS D'ETAT DE CONTRAT MENSUEL.
            if nat.mensuelle() && nat.duree_indeterminee && !premier_mois {
                let recu = g.recus.get(&k).filter(|r| vrai(r));
                let signe_en = match recu {
                    Some(r) => valeur_en_texte(r).chars().take(7).collect::<String>(),
                    None => mois_du_contrat.first().cloned().unwrap_or_default(),
                };
                signature.cdi_en_cours = Some(CdiEnCours { signe_en });
            }
            // LES DOCUMENTS DE CET ENGAGEMENT, pour ce mois : son dernier envoi (`departs`), et son bulletin a refaire.
            let documents_le = p.engagement.numero.as_deref().and_then(|n| {
                departs.de(&cle, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.as_deref() == Some(mois) && d.par.is_none()).map(|d| d.le.clone()).max()
            });
            let bulletin = refaire.iter().any(|a| matches!(a, ARefaire::Bulletin { mois: m, .. } if m == mois));
            let documents = etat_documents(documents_le, bulletin);
            let vue = Vue {
                salarie: cle.clone(),
                nom: super::nom_a_l_ecran(sal),
                spectacle,
                nature: nat.libelle.clone(),
                numero: p.engagement.numero.clone(),
                debut: debut.clone(),
                // Un CDI n'a pas de fin : rien, comme `Engagement::canon`.
                fin: p.engagement.fin.map(|f| f.iso()),
                etablissement,
                echeances,
                valide: e.valide().map(|v| v.le().to_string()),
                recalcul,
                contrat,
                pieces,
                etats: Etats { dpae, signature, documents },
                a_completer: None,
            };
            // LA FRISE : un engagement modifie apres un envoi dit quand, et ce qui a change de version en version.
            let frise = e.valide().and_then(|v| engagement::versions::frise(self.ctx, v, &departs, mois));
            out.push(Ligne::de(vue, mois, frise.unwrap_or(Value::Null), id.clone()));
        }
        Ok(out)
    }

    /// LA LIGNE D'UN ENGAGEMENT JAMAIS VALIDE AUQUEL IL MANQUE UN FAIT : rien ne se calcule sur lui, rien n'est parti ;
    /// il dit qui, quand, et ce qui lui manque. Ses carres disent « à compléter » (`Point::de`). L'engagement se lit tel
    /// que sa saisie l'ecrit (`Objet::vue`).
    fn ligne_a_completer(&self, mois: &str, o: &engagement::stock::Objet, manque: &str) -> R<Vue> {
        let vue = o.vue(mois).map_err(plain)?.unwrap_or_default();
        let e = Value::Object(vue);
        let cle = engagement::cle_de_l_engagement(self.ctx, &e).map(String::from).unwrap_or_else(|| e.get("salarie").map(valeur_en_texte).unwrap_or_default());
        let nom = self.ctx.salarie(&cle).ok().map(|f| super::nom_a_l_ecran(&f)).unwrap_or_else(|| cle.clone());
        let spectacle = match e.get("spectacle").filter(|s| vrai(s)) {
            Some(s) => self.ctx.spectacle(&valeur_en_texte(s)).ok().and_then(|f| f.get("titre").cloned()).unwrap_or_else(|| s.clone()),
            None => Value::Null,
        };
        let nature = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map(|n| n.libelle.clone()).unwrap_or_default();
        let premier_mois = o.premier_mois() == mois;
        let debut = engagement::debut_brut(&self.ctx.racine, &e);
        let anterieur = engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, &debut, mois).map_err(plain)?;
        let texte = |k: &str| e.get(k).filter(|v| vrai(v)).and_then(Value::as_str).map(String::from);
        Ok(Vue {
            salarie: cle,
            nom,
            spectacle,
            nature,
            numero: texte("numero").and_then(|n| NumeroContrat::lire(&n).ok()),
            debut,
            fin: texte("fin"),
            etablissement: None,
            echeances: None,
            valide: None,
            recalcul: None,
            contrat: ContratDit::ACompleter(ContratACompleter { premier_mois, repris: false, anterieur }),
            pieces: Vec::new(),
            etats: Etats::default(),
            a_completer: Some(manque.to_string()),
        })
    }

    // ------------------------------------------------------ les fichiers deposables

    /// `fichiers_deposables` : les fichiers d'un mois qu'on peut deposer, par
    /// acte, le dernier ecrit en tete.
    pub(crate) fn fichiers_deposables(&mut self, mois: &str) -> R<Rc<Deposables>> {
        if let Some(v) = self.fichiers.get(mois) {
            return Ok(v.clone());
        }
        let dossier = self.racine().join("sorties").join(mois);
        let rendu = if !dossier.is_dir() {
            Deposables::default()
        } else {
            let mut tous: Vec<(String, PathBuf)> = Vec::new();
            let rd = std::fs::read_dir(&dossier).map_err(|e| erreur("OSError", socle::echec::du_disque(&dossier, &e)))?;
            for e in rd.flatten() {
                let chemin = e.path();
                let Ok(m) = std::fs::metadata(&chemin) else { continue };
                if m.is_file() {
                    tous.push((e.file_name().to_string_lossy().into_owned(), chemin));
                }
            }
            // L'ORDRE NE DEPEND PAS DE L'HEURE D'UN FICHIER (deux rejeux, deux machines, le meme ordre) : il se lit
            // dans ce que le moteur sait de chacun (`ordre_de_depot`).
            let ranger = |mut v: Vec<&(String, PathBuf)>| -> Vec<Deposable> {
                v.sort_by_cached_key(|f| ordre_de_depot(&f.0, &f.1));
                v.into_iter().map(|(_, chemin)| Deposable { chemin: chemin.to_string_lossy().into_owned() }).collect()
            };
            let dsn: Vec<_> = tous.iter().filter(|f| suffixe(&f.0).to_lowercase() == ".dsn").collect();
            let dpae: Vec<_> = tous
                .iter()
                .filter(|f| f.0.starts_with("DPAE_") && suffixe(&f.0).to_lowercase() == ".txt" && !radical(&f.0).ends_with("_recapitulatif"))
                .collect();
            Deposables { dsn: ranger(dsn), dpae: ranger(dpae) }
        };
        let rendu = Rc::new(rendu);
        self.fichiers.insert(mois.to_string(), rendu.clone());
        Ok(rendu)
    }

    // ----------------------------------------------------- l'annule-et-remplace

    /// `empreinte_saisie` : ce que la DSN du mois declare, resume en 16 caracteres -- les paies validees du mois
    /// (`engagement::etats::empreinte_des_paies_validees` : un brouillon n'y est pas, la DSN ne le declare pas), et la
    /// version de chaque engagement d'un mois passe dont elle porte la correction
    /// (`produire_regularisations::regularisations_pour`) : une correction validee apres l'ecriture du fichier le rend
    /// perime comme une paie du mois qui change. `None` si rien ne se lit ni ne se corrige.
    pub(crate) fn empreinte_saisie(&self, mois: &str) -> R<Option<String>> {
        let departs = self.departs()?;
        self.empreinte_saisie_avec(mois, || declarations::produire_regularisations::regularisations_pour(self.ctx, mois, &departs).map(|r| Self::portees(&r)))
    }

    /// `empreinte_saisie`, sur les corrections portees que `portees` lit (`Moteur::portees`) : la fabrication de la DSN,
    /// qui les a deja lues, les passe.
    pub(crate) fn empreinte_saisie_avec(&self, mois: &str, portees: impl FnOnce() -> R<Vec<Value>>) -> R<Option<String>> {
        let vue = engagement::etats::empreinte_des_paies_validees(self.ctx, mois, None);
        let portees: Vec<Value> = portees().unwrap_or_else(|e| vec![json!(e.message)]);
        Ok(match (vue, portees.is_empty()) {
            (vue, true) => vue.map(|v| v.chars().take(16).collect()),
            (vue, false) => Some(empreinte_de(&json!({"saisie": vue, "corrections": portees}))),
        })
    }

    /// Ce que l'empreinte de la saisie retient de chaque correction portee : le mois corrige, le salarie, le contrat et
    /// la version.
    pub(crate) fn portees(regs: &[declarations::produire_regularisations::Reg]) -> Vec<Value> {
        regs.iter().map(|r| json!([r.mois, r.apres.salarie.id.to_string(), r.apres.engagement.numero, r.version])).collect()
    }

    /// `limite` : le dernier jour ou un annule-et-remplace est admis, jusqu'a minuit : la veille de l'echeance de depot
    /// (`echeance_dsn::Echeance::veille`, reportee comme elle).
    pub(crate) fn limite(&self, mois: &str) -> R<Jour> {
        let e = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        e.veille().map_err(plain)
    }

    /// Le refus d'un annule-et-remplace du mois apres sa `limite` (cahier technique DSN §1.4.2) ; `Ok` jusqu'a elle.
    pub(crate) fn refuser_si_hors_delai(&self, mois: &str) -> R<()> {
        let limite = self.limite(mois)?;
        if self.ctx.aujourdhui <= limite {
            return Ok(());
        }
        Err(erreur(
            "AnnuleRemplaceHorsDelai",
            format!(
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). Ce qui a change dans ce mois part dans la DSN d'un mois suivant, en correction. Rien ne part.",
                mois,
                limite.jj_mm_aaaa()
            ),
        ))
    }

    /// Le plus haut ordre porte par ces depots reels du mois (`depots_reels`) ; `None` s'il n'y en a pas.
    pub(crate) fn dernier_ordre(&self, mois: &str, faits: &[Value]) -> R<Option<i64>> {
        let mut dernier: Option<i64> = None;
        for l in faits {
            let n = self.ordre_de_la_ligne(mois, l)?;
            dernier = Some(dernier.map_or(n, |d| d.max(n)));
        }
        Ok(dernier)
    }

    /// Le rang d'un depot reel du journal : l'ordre que sa ligne porte (S20.G00.05.004 du fichier parti, noté au depot).
    pub(crate) fn ordre_de_la_ligne(&self, mois: &str, l: &Value) -> R<i64> {
        let d = ou(get(l, "detail")?, objet_vide());
        let ordre = get(d, "ordre")?;
        let t = if vrai(ordre) { valeur_en_texte(ordre) } else { String::new() };
        t.trim().parse::<i64>().map_err(|_| {
            erreur(
                "ValueError",
                socle::echec::avec(format!("Un dépôt réel de la DSN de {} ne porte pas son ordre (S20.G00.05.004) dans le journal des envois : le rang d'un annule-et-remplace ne se devine pas ; le signaler au développeur.", mois), serde_json::json!({"fichier": envois::journal::DEPOTS, "cle": "ordre"})),
            )
        })
    }

    /// `depots_reels` : les depots REELS de DSN mensuelle du mois que la
    /// plate-forme a pris et qu'aucun bilan n'a refuses (les lignes du journal des envois, telles qu'elles sont).
    pub(crate) fn depots_reels(&mut self, mois: &str, nic: &Value) -> R<Vec<Value>> {
        let lignes = self.lignes()?;
        let reels: Vec<Value> = etats::filtrer(&lignes, Some(DSN), Some(mois), true)?.into_iter().cloned().collect();
        let mut out = Vec::new();
        for l in etats::de_l_etablissement(&reels, nic, &self.nic())? {
            if !vrai(get(&l, "accepte")?) || etats::refuse_par_un_retour(&l)? {
                continue;
            }
            let defaut = Value::String("01".into());
            let t = getd(ou(get(&l, "detail")?, objet_vide()), "type", &defaut)?;
            if ["01", "02", "03", "05"].iter().any(|x| est(t, x)) {
                out.push(l);
            }
        }
        Ok(out)
    }

    /// Ou en est le mois vis-a-vis de
    /// l'annule-et-remplace.
    /// `nic` : l'etablissement dont on parle (`Null` : le principal).
    pub(crate) fn etat_remplacement(&mut self, mois: &str, nic: &Value) -> R<Remplacement> {
        let faits = self.depots_reels(mois, nic)?;
        let lim = self.limite(mois)?;
        let admis = self.ctx.aujourdhui <= lim;
        let mut out = Remplacement {
            mois: Mois::lire(mois).map_err(plain)?,
            deposee: !faits.is_empty(),
            dernier_ordre: None,
            prochain_ordre: 1,
            limite: lim,
            admis,
            saisie_modifiee: None,
            a_envoyer: false,
            phrase: String::new(),
            // Apres la limite : la DSN suivante corrige chaque contrat dont la paie validee a change.
            corrigee_par_la_suivante: false,
        };
        let Some(derniere) = faits.last() else {
            return Ok(out);
        };
        let dernier = self.dernier_ordre(mois, &faits)?.unwrap_or(i64::MIN);
        out.dernier_ordre = Some(dernier);
        out.prochain_ordre = dernier + 1;
        // Ce que la DSN a emporte, compare au present des engagements du mois (`dsn_a_refaire`).
        let mut modifiee = false;
        if derniere.get("versions").is_some() {
            let etats_eng = self.etats_du_mois(mois)?;
            let (ctx, departs) = (self.ctx, self.departs()?);
            modifiee = dsn_a_refaire(ctx, mois, derniere, &etats_eng, &mut |e| self.a_refaire_du_mois(mois, e, &departs));
            out.saisie_modifiee = Some(modifiee);
        }
        // Apres la limite : les contrats dont la paie validee n'est plus celle que la DSN a declaree ; la DSN suivante
        // porte leur correction d'elle-meme (`engagement::etats::correction_due`, comme
        // `produire_regularisations::corrections_dues`), ceux de cet etablissement seuls.
        let corriges = if admis {
            0
        } else {
            let departs = self.departs()?;
            let plusieurs = !self.ctx.entreprise_du_mois(mois).map_err(plain)?.secondaires().is_empty();
            let voulu = if vrai(nic) { valeur_en_texte(nic) } else { self.nic() };
            let mut n = 0;
            for e in self.etats_du_mois(mois)? {
                let Some(v) = e.valide() else { continue };
                if plusieurs && nic_de_l_engagement(self.ctx, mois, &e).as_deref() != Some(voulu.as_str()) {
                    continue;
                }
                if engagement::etats::correction_due(self.ctx, v, &departs, mois, "9999-12").map_err(plain)?.is_some() {
                    n += 1;
                }
            }
            n
        };
        if corriges > 0 {
            modifiee = true;
            out.saisie_modifiee = Some(true);
        }
        let quand = quand_lisible(get(derniere, "quand")?);
        let jour = lim.jj_mm_aaaa();
        if corriges > 0 {
            let pluriel = if corriges > 1 { "s" } else { "" };
            out.corrigee_par_la_suivante = true;
            out.phrase = format!(
                "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — {} contrat{} corrigé{} par la DSN suivante",
                quand, jour, corriges, pluriel, pluriel
            );
            return Ok(out);
        }
        if modifiee {
            if admis {
                out.a_envoyer = true;
                out.phrase = format!("Paie modifiée depuis la DSN du {} — annule-et-remplace à envoyer, jusqu'au {} à minuit", quand, jour);
            } else {
                out.phrase = format!(
                    "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — la correction part dans la DSN suivante",
                    quand, jour
                );
            }
        } else if admis {
            out.phrase = format!("Une modification reste possible : la DSN refaite partira en annule-et-remplace (ordre {}), jusqu'au {} à minuit", dernier + 1, jour);
        }
        Ok(out)
    }

    /// `paie_changee_depuis_l_ecriture` : `None` si le fichier declare la paie
    /// d'aujourd'hui, sinon POURQUOI. Le journal des ecritures (`compagnie::depart`) se lit tel qu'il est.
    pub(crate) fn paie_changee_depuis_l_ecriture(&self, mois: &str, fichier: &Path) -> R<Option<String>> {
        let nom = fichier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let d = Value::Object(crate::compagnie::depart::journal_des_ecritures(self.racine()));
        let note = get(&d, &format!("{}/{}", mois, nom))?;
        if vrai(note) && vrai(get(note, "empreinte_saisie")?) {
            let notee = get(note, "empreinte_saisie")?;
            let maintenant = self.empreinte_saisie(mois)?;
            if maintenant.map_or(true, |m| !est(notee, &m)) {
                return Ok(Some(format!("l'engagement a changé depuis que ce fichier a été écrit (le {})", quand_lisible(get(note, "quand")?))));
            }
            return Ok(None);
        }
        // SANS EMPREINTE NOTEE, LE MOTEUR NE SAIT PAS : il ne compare plus les dates des fichiers. Une fictive
        // livree, decompressee chez un visiteur, a des dates de fichier qui ne disent rien de l'ordre ou elle a ete ecrite,
        // et chacun de ses mois se disait « paie changee ».
        Ok(None)
    }

    // ----------------------------------------------------------- les groupes

    /// LE CHEMIN DU FICHIER A DEPOSER (`quoi` : DPAE ou DSN) : ou il en est (ecrite, en test, testee, test refuse,
    /// paie changee), le fichier, et ce qu'un test refuse a dit ; `None` sans fichier a deposer, ou s'il est deja parti.
    /// `nic` : pour la DSN d'une compagnie a plusieurs etablissements, celle de cet etablissement (son en-tete,
    /// S21.G00.11.001) ; `Null` : le premier fichier, quel qu'il soit.
    fn chemin_de_depot(&mut self, mois: &str, quoi: &str, nic: &Value) -> R<Option<(&'static str, Deposable, Option<String>)>> {
        // « Ecrite » pour une DSN : le meme critere que l'horloge, l'accueil et les rappels (`depart::mois_ecrit`,
        // la DSN mensuelle notee au journal des ecritures), pas un fichier pose dans `sorties/<mois>/`. Un mois passe lu a
        // l'ouverture (`ecrit`) ne demande que ce qui reste, pas le geste qui l'ouvre.
        if self.ecrit || (quoi == DSN && !crate::compagnie::depart::mois_ecrit(self.racine(), mois)) {
            return Ok(None);
        }
        let fichiers = self.fichiers_deposables(mois)?;
        let de_l_etablissement = |f: &&Deposable| -> bool {
            if quoi != DSN || !vrai(nic) {
                return true;
            }
            let octets = std::fs::read(&f.chemin).unwrap_or_default();
            envois::depot::lire(&octets).champ("etablissement") == valeur_en_texte(nic)
        };
        let Some(premier) = fichiers.de(quoi).iter().find(de_l_etablissement).cloned() else {
            return Ok(None);
        };
        let chemin = PathBuf::from(&premier.chemin);
        let lignes = self.lignes()?;
        let empreinte = envois::journal::empreinte_fichier(&chemin).map_err(|m| erreur("FileNotFoundError", m))?;
        if vrai(&etats::deja_depose(&lignes, quoi, &empreinte)?) {
            return Ok(None);
        }
        let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let r = etats::repetition(&lignes, quoi, &empreinte, &nom)?;
        let etape = if quoi == DSN && self.paie_changee_depuis_l_ecriture(mois, &chemin)?.is_some() {
            "paie_changee"
        } else {
            etats::etat_du_test(&r)
        };
        // UN TEST REFUSE SE DIT AVEC SES ANOMALIES, la ou l'on propose d'envoyer : le code et le message du bilan.
        let refus = r["motif"].as_str().filter(|_| etape == "test_refuse").map(|m| m.trim_end_matches('.').to_string());
        Ok(Some((etape, premier, refus)))
    }

    /// LE GESTE d'une case orange qui depose (DPAE, DSN), une fois sa declaration ecrite : la charge du depot qui la
    /// designe (`depots::designee` : le mois, l'etablissement, les embauches que le fichier ecrit declare) ; sinon `defaut`.
    fn action_de_depot(&mut self, mois: &str, quoi: &str, nic: &Value, defaut: Option<Action>) -> R<(Option<Action>, Option<String>)> {
        let Some((etape, fichier, refus)) = self.chemin_de_depot(mois, quoi, nic)? else { return Ok((defaut, None)) };
        let mut charge = json!({"quoi": quoi, "mois": mois});
        if quoi == DSN && vrai(nic) {
            charge["etablissement"] = nic.clone();
        }
        if quoi == DPAE {
            let a = crate::routes::depots::apercu_depot_dpae(self.ctx, &json!(fichier.chemin))?;
            let embauches: Vec<Value> = a["embauches"].as_array().into_iter().flatten().map(|e| json!({"salarie": e["salarie"], "numero": e["numero"]})).collect();
            charge["engagements"] = json!(embauches);
        }
        let nom = if quoi == DSN { "la DSN" } else { "la DPAE" };
        let deposer = |libelle: String| action(Route::DepotApercu, charge.clone(), Some(Route::Deposer), libelle);
        let action = match etape {
            // Le fichier ecrit ne declare plus la paie : la DSN se refait depuis les engagements valides, et repasse par
            // le test.
            "paie_changee" => deposer_dsn(mois, nic, format!("Réécrire {} et la tester…", nom)),
            "ecrite" => deposer(format!("Tester {}…", nom)),
            "en_test" => deposer("Voir le test en cours…".into()),
            "testee" => deposer(format!("Envoyer {} testée…", nom)),
            _ => deposer("Voir le refus du test…".into()),
        };
        Ok((Some(action), refus.map(|r| format!("test refusé : {}", r))))
    }

    /// `bilan_du_mois` : LES ETAPES DU MOIS, dans l'ordre de l'habitude (DPAE, Contrats, DSN, Documents), chacune
    /// avec les groupes que le panneau dessine (`Groupe`) : un groupe `fait` n'a pas de compte ; un groupe qui reste compte
    /// en nombre simple, avec son echeance (`texte`, et `court` sans l'annee, que la case ecrit) et le geste qu'ouvre un
    /// clic. `colonnes` : celles du mois (`colonnes`, G d'abord) ; chaque groupe nomme celles qu'il couvre, les memes d'une
    /// etape a l'autre. L'etape `suivante` est la premiere qui a quelque chose a faire de notre cote. Aucun montant : le
    /// brut n'est jamais lu.
    pub(crate) fn bilan_du_mois(&mut self, mois: &str) -> R<Bilan> {
        let le_mois = Mois::lire(mois).map_err(plain)?;
        // UN MOIS TENU AVANT CE LOGICIEL : ses etapes se sont faites ailleurs ; chaque case le dit (« mois importé »),
        // sans geste. Ce que l'import en dit (`import::mois::etapes`) se lit tel qu'il est.
        if let Some(vue) = crate::import::mois::vue(self.ctx, mois)? {
            let b = crate::import::mois::etapes(&vue);
            let colonnes = vec!["G".to_string()];
            let etapes = ORDRE
                .iter()
                .map(|cle| {
                    let x = &b[cle.cle()];
                    let (etat, quoi) = if est(&x["couleur"], "vert") {
                        (EtatCase::Fait, "mois importé".to_string())
                    } else {
                        (if est(&x["couleur"], "rouge") { EtatCase::Refuse } else { EtatCase::Information }, format!("mois importé · {}", valeur_en_texte(&x["courte"])))
                    };
                    let g = Groupe::de(etat, None, String::new(), quoi, None, None).sous(&colonnes);
                    Etape { phrase: Some(x["phrase"].clone()), ..Etape::de(*cle, vec![g]) }
                })
                .collect();
            let etapes = suivante(etapes);
            return Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(None, &etapes), colonnes, etapes });
        }
        let paies = self.etat_par_engagement(mois)?;
        let echeance_de = |ps: &[&Vue], dpae: bool| -> Option<Echeance> {
            ps.iter().filter_map(|p| p.echeances.as_ref()).map(|e| if dpae { &e.dpae } else { &e.contrat }).min_by_key(|e| e.jour).cloned()
        };

        // --- Les colonnes : chaque engagement a son point, la DSN celui de son etablissement.
        let classes = self.groupes_dsn(mois)?;
        let classe_de = |p: &Vue| -> usize { p.etablissement.as_deref().and_then(|nic| classes.iter().position(|(nics, _)| nics.iter().any(|n| n == nic))).unwrap_or(0) };
        let points: Vec<Point> = paies.iter().map(|l| Point::de(&l.vue, classe_de(&l.vue))).collect();
        let dsn_fait: Vec<bool> = classes.iter().map(|(_, g)| g.etat == EtatCase::Fait).collect();
        let cols = colonnes(&points, &dsn_fait);
        let de = |m: &[usize]| -> Vec<&Vue> { m.iter().map(|i| &paies[*i].vue).collect() };

        // --- DPAE : par embauche du mois. Partie, elle est faite tant qu'aucun retour ne revient KO.
        let mut dpae = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.dpae, &["a_faire", "refuse", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].dpae == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "refuse" => Groupe::de(EtatCase::Refuse, Some(Compte::Nombre(n)), n.to_string(), format!("refusée{} · l'éditeur est prévenu", plur(n, "", "s")), echeance_de(&siens, true), Some(historique(mois))),
                "a_faire" => {
                    let (a, refus) = self.action_de_depot(mois, DPAE, &Value::Null, Some(declarer_embauches(mois)))?;
                    // Le jour de l'embauche passe, la DPAE qui devait le preceder (L. 1221-10) est en retard ; elle se depose
                    // toujours.
                    let echeance = echeance_de(&siens, true);
                    let en_retard = siens.iter().filter(|p| p.echeances.as_ref().is_some_and(|e| e.dpae.jour < self.ctx.aujourdhui)).count();
                    let etat = match en_retard {
                        0 => "à déposer".to_string(),
                        r if r == n => "à déposer, en retard".to_string(),
                        r => format!("à déposer, dont {} en retard", r),
                    };
                    let quoi = refus.map_or_else(|| etat.clone(), |r| format!("{} · {}", etat, r));
                    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), quoi, echeance, a)
                }
                A_COMPLETER => a_completer(n),
                A_VALIDER => a_valider(n),
                RIEN_A_DECLARER => rien_ici("rien à déclarer"),
                _ => Groupe::fait(),
            };
            dpae.push(g.sous(&noms));
        }

        // --- Contrats : un par embauche dont la nature a un contrat ecrit (`engagement::contrat_a_signer`).
        let mut contrats = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.contrat, &["a_envoyer", "a_refaire", "en_attente", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].contrat == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "en_attente" => {
                    // CE QUI EST REVENU SUR CE QUI VIENT DE PARTIR, et sur cela seul : les contrats de la case partis le
                    // jour du dernier envoi encore attendu.
                    let jour = |p: &Vue| p.etats.signature.fait.as_deref().unwrap_or_default().chars().take(10).collect::<String>();
                    let dernier = siens.iter().map(|p| jour(p)).max().unwrap_or_default();
                    let partis: Vec<&Vue> = de(&m)
                        .into_iter()
                        .zip(m.iter())
                        .filter(|(p, i)| matches!(points[**i].contrat, "fait" | "en_attente") && p.etats.signature.parti() && jour(p) == dernier)
                        .map(|(p, _)| p)
                        .collect();
                    let recus = partis.iter().filter(|p| vrai(&p.etats.signature.recu) && !p.etats.signature.avenant_attendu).count();
                    // L'echeance est celle des contrats attendus (L. 1242-13) ; un avenant attendu n'en a pas (aucun texte
                    // depose ne date sa remise).
                    let contrats_attendus: Vec<&Vue> = siens.iter().copied().filter(|p| !p.etats.signature.avenant_attendu).collect();
                    Groupe::de(
                        EtatCase::EnAttente,
                        Some(Compte::Recus { recus, partis: partis.len() }),
                        format!("{}/{}", recus, partis.len()),
                        format!("reçu{}", plur(recus, "", "s")),
                        echeance_de(&contrats_attendus, false),
                        Some(contrats_en_attente(mois)),
                    )
                }
                // L'AVENANT n'a pas d'echeance : L. 1242-13 date la remise du contrat a l'embauche, aucun texte depose ne
                // date celle d'un avenant (regle 1 : rien ne se devine).
                "a_refaire" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à refaire signer".into(), None, Some(faire_signer(mois))),
                "a_envoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), echeance_de(&siens, false), Some(faire_signer(mois))),
                A_COMPLETER => a_completer(n),
                A_VALIDER => a_valider(n),
                RIEN_A_SIGNER => rien_ici("rien à signer"),
                _ => Groupe::fait(),
            };
            contrats.push(g.sous(&noms));
        }

        // --- DSN : par etablissement (`groupes_dsn`), chaque colonne sous la DSN de ses etablissements.
        let dsn: Vec<Groupe> = cases_de_la_ligne(&cols, |c| c.dsn.to_string()).into_iter().map(|(_, _, noms, k)| classes[k].1.clone().sous(&noms)).collect();

        // --- Documents : par salarie du mois, des que son engagement est valide, sans echeance : ils partent quand on veut.
        let mut documents = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.documents, &["a_renvoyer", "a_envoyer", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].documents == point).map(|(p, _)| p).collect();
            let n = siens.iter().map(|p| p.salarie.as_str()).collect::<BTreeSet<&str>>().len();
            let g = match point.as_str() {
                "a_renvoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à renvoyer".into(), None, Some(envoyer_documents(mois, format!("Renvoyer les documents corrigés ({})…", n)))),
                "a_envoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), None, Some(envoyer_documents(mois, "Envoyer les documents…".into()))),
                // Ce qui reste a completer se compte en engagements, comme a la DPAE.
                A_COMPLETER => a_completer(siens.len()),
                A_VALIDER => a_valider(siens.len()),
                _ => Groupe::fait(),
            };
            documents.push(g.sous(&noms));
        }
        // CE QUI PART CE MOIS-CI, NOMME PAR LE MOTEUR : les pieces des paies du mois, dans l'ordre de l'envoi.
        let noms_pieces = [("bulletin", "bulletin"), ("certificat_cs", "certificat Congés Spectacles"), ("aer", "AER")];
        let presentes: Vec<&str> = noms_pieces.iter().filter(|(c, _)| paies.iter().any(|p| p.vue.pieces.contains(c))).map(|(_, n)| *n).collect();

        // UN MOIS SANS ENGAGEMENT n'a rien de fait : ce qui n'a rien a faire s'y dit « rien ce mois-ci », en gris. Ce que le
        // moteur demande quand meme (une DSN due sans salarie) garde son etat.
        let vide = paies.is_empty();
        // La DSN d'un mois sans engagement, partie sans individu, est faite ; pas due, elle n'a rien a faire.
        let dsn_partie = vide && crate::compagnie::etat_dsn::du_mois(self.ctx, mois, &Value::Null)?.partie();
        let rien = |gs: Vec<Groupe>| -> Vec<Groupe> { gs.into_iter().map(|g| if vide && g.etat == EtatCase::Fait { Groupe::rien_ce_mois(g.colonnes) } else { g }).collect() };
        let dsn = if dsn_partie { dsn } else { rien(dsn) };
        // Un mois sans engagement se dit une fois : ce qui reste (sa DSN) se dit dans sa case.
        let debut = match (vide, dsn.iter().all(|g| g.etat == EtatCase::Rien)) {
            (false, _) => None,
            (true, rien_du_tout) => Some(format!(
                "Aucun engagement en {} : {}",
                Mois::en_clair(mois),
                if rien_du_tout { "rien à faire" } else { "ni DPAE, ni contrat, ni document" }
            )),
        };
        let etapes = vec![
            Etape::de(QuelleEtape::Dpae, rien(dpae)),
            Etape::de(QuelleEtape::Signature, rien(contrats)),
            Etape::de(QuelleEtape::Dsn, dsn),
            Etape { detail: Some(presentes.join(", ")), ..Etape::de(QuelleEtape::Documents, rien(documents)) },
        ];
        let noms: Vec<String> = cols.iter().map(|c| c.nom.clone()).collect();
        let etapes = suivante(etapes);
        Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(debut, &etapes), colonnes: noms, etapes })
    }

    /// LA DSN DU MOIS, par etablissement. Elle « est mensuelle et produite par établissement employeur » (cahier technique
    /// DSN 2026.1, § 2.4, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : chaque
    /// etablissement a la sienne (`groupes_dsn_de`). Les etablissements au meme point (meme etat, phrase et echeance)
    /// forment une classe, une case ; quand ils ne sont pas tous au meme point, la case d'une classe qui n'est pas faite
    /// nomme ses etablissements (« Lyon refusée · l'éditeur est prévenu »). Rend les classes (leurs NIC, leur case), le
    /// principal dans la premiere.
    fn groupes_dsn(&mut self, mois: &str) -> R<Vec<(Vec<String>, Groupe)>> {
        let ent = self.ctx.entreprise_du_mois(mois).map_err(plain)?;
        if ent.secondaires().is_empty() {
            let g = self.groupes_dsn_de(mois, &Value::Null)?;
            return Ok(vec![(vec![ent.nic_principal()], g)]);
        }
        let point = |g: &Groupe| (g.etat, g.phrase.clone(), g.echeance.as_ref().map(|e| e.jour));
        let mut classes: Vec<(Vec<String>, Groupe)> = Vec::new();
        for nic in ent.nics() {
            let g = self.groupes_dsn_de(mois, &json!(nic))?;
            match classes.iter_mut().find(|(_, x)| point(x) == point(&g)) {
                Some((nics, _)) => nics.push(nic),
                None => classes.push((vec![nic], g)),
            }
        }
        if classes.len() > 1 {
            for (nics, g) in classes.iter_mut().filter(|(_, g)| g.etat != EtatCase::Fait) {
                let noms: Vec<String> = nics.iter().map(|n| nom_de_l_etablissement(&ent, n)).collect();
                g.compte_texte = noms.join(", ");
                g.phrase = Some(format!("{} {}", noms.join(", "), g.quoi));
            }
        }
        Ok(classes)
    }

    /// LES GROUPES DE LA DSN de l'etablissement `nic` (`Null` : la compagnie n'en a qu'un). Une DSN partie ne se refait que
    /// si ce qu'elle a emporte est a corriger. L'echeance : celle du depot (`echeance_dsn`, le jour meme a midi) ; la veille a minuit pour un
    /// annule-et-remplace ; apres un refus, celle du depot qui la reprend.
    ///
    /// LA DSN REFUSEE n'est pas integree : celle qui la reprend est un depot initial (cahier technique DSN 2026.1,
    /// § 1.4.2, `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt` : le depot initial va jusqu'au jour de
    /// l'echeance a midi, l'annule-et-remplace jusqu'a la veille a minuit). INFERENCE : un depot refuse n'ouvre pas
    /// l'annule-et-remplace, puisqu'il n'y a rien a annuler ; l'echeance de la reprise est celle du depot initial.
    fn groupes_dsn_de(&mut self, mois: &str, nic: &Value) -> R<Groupe> {
        let echeance = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| erreur("ValueError", m))?;
        let au_depot = echeance_dsn(echeance.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 ; CSS R. 243-6 et R. 133-14 (declarations::echeance_dsn)");
        let e = crate::compagnie::etat_dsn::du_mois(self.ctx, mois, nic)?;
        let partie = e.deposee();
        if !partie {
            // Due chaque mois, sans salarie aussi (`etat_dsn::due`, ce que l'echeancier et les rappels lisent) ; un mois
            // sans rien a declarer porte encore la correction d'un mois passe (`dsn_a_declarer`).
            let a_declarer = self.dsn_a_declarer(mois, nic)?;
            if !a_declarer && !crate::compagnie::etat_dsn::due(self.ctx, mois, nic)? {
                return Ok(Groupe::fait());
            }
            // Avant son premier jour de depot, la case reste a deposer : c'est le depot qui dit le jour
            // (`compagnie::depart::fabriquer_dsn`).
            let (a, refus) = self.action_de_depot(mois, DSN, nic, Some(deposer_dsn(mois, nic, "Déposer la DSN…".into())))?;
            let etat = if a_declarer { "à déposer" } else { "à déposer, sans salarié" };
            let quoi = refus.map_or_else(|| etat.to_string(), |r| format!("{} · {}", etat, r));
            return Ok(Groupe::de(EtatCase::AFaire, None, String::new(), quoi, Some(au_depot), a));
        }
        if e.refusee() {
            return Ok(Groupe::de(EtatCase::Refuse, None, String::new(), "refusée · l'éditeur est prévenu".into(), Some(au_depot), Some(historique(mois))));
        }
        // Partie : faite, tant qu'aucun retour ne revient KO. Ce qui la rouvre : une paie modifiee depuis.
        let r = self.etat_remplacement(mois, nic)?;
        let libelle = format!("Refaire la DSN (annule-et-remplace, ordre {})…", r.prochain_ordre);
        let veille = echeance_dsn(r.limite, "à minuit", "Cahier technique DSN 2026.1, § 1.4.2 (atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt)");
        let modifiee = r.saisie_modifiee == Some(true);
        if modifiee && r.a_envoyer {
            // Un annule-et-remplace deja ecrit montre son chemin ; sinon, le refaire.
            let ecrit = self.chemin_de_depot(mois, DSN, nic)?.is_some();
            let (a, refus) = if ecrit { self.action_de_depot(mois, DSN, nic, None)? } else { (Some(deposer_dsn(mois, nic, libelle)), None) };
            let quoi = refus.map_or_else(|| "à refaire".to_string(), |x| format!("à refaire · {}", x));
            return Ok(Groupe::de(EtatCase::AFaire, None, String::new(), quoi, Some(veille), a));
        }
        if modifiee {
            // Apres la veille : la correction part dans la DSN qui porte (la premiere d'un mois suivant qui n'est pas
            // partie), celle que l'avertissement de « Modifier l'engagement » nomme aussi.
            let suivant = crate::regularisation::dsn_qui_porte(self.ctx, mois)?;
            let e2 = declarations::echeance_dsn::de_la_compagnie(self.ctx, &suivant).map_err(|m| erreur("ValueError", m))?;
            let a = deposer_dsn(&suivant, nic, format!("Déposer la DSN de {}…", Mois::en_clair(&suivant)));
            return Ok(Groupe::de(
                EtatCase::AFaire,
                None,
                String::new(),
                format!("à corriger dans la DSN de {}", Mois::en_clair(&suivant)),
                Some(echeance_dsn(e2.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 (declarations::echeance_dsn)")),
                Some(a),
            ));
        }
        Ok(Groupe::fait())
    }

    /// LA DSN DE `mois` A-T-ELLE QUELQUE CHOSE A DECLARER pour l'etablissement `nic` (`Null` : le seul) ? Elle est a
    /// deposer des qu'un engagement valide touche le mois (« le dépôt initial de la DSN "normale" peut
    /// être émis jusqu'à 1 mois à l'avance et 3 mois en retard », cahier technique DSN 2026.1, § 1.4.2,
    /// `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`), ou qu'elle porte la correction d'un mois passe
    /// dont l'annule-et-remplace n'est plus admis (§ 2.1.2.1, `chapitre-2-principes-de-constitution.txt` : « Une
    /// correction est un évènement connu après la date de clôture de la paie. Il donne lieu à la déclaration d'une
    /// correction dans la DSN suivante » ; `produire_regularisations::regularisations_pour`). Sinon, rien n'est a faire.
    /// Ne lit que ce qui est ecrit.
    fn dsn_a_declarer(&mut self, mois: &str, nic: &Value) -> R<bool> {
        let plusieurs = !self.ctx.entreprise_du_mois(mois).map_err(plain)?.secondaires().is_empty();
        let voulu = if vrai(nic) { valeur_en_texte(nic) } else { self.nic() };
        let de_l_etablissement = |ctx: &Contexte, m: &str, brut: &Value| !plusieurs || nic_de_l_engagement_ecrit(ctx, m, brut).as_deref() == Some(voulu.as_str());
        for e in self.etats_du_mois(mois)? {
            if e.valide().is_some() && de_l_etablissement(self.ctx, mois, &e.objet().brut) {
                return Ok(true);
            }
        }
        let departs = self.departs()?;
        for m in engagement::stock::mois_saisis(self.racine()).unwrap_or_default() {
            if m.as_str() >= mois {
                break;
            }
            if !declarations::produire_regularisations::se_corrige_ailleurs(self.ctx, &departs, &m)? {
                continue;
            }
            for e in self.etats_du_mois(&m)? {
                let Some(v) = e.valide() else { continue };
                if de_l_etablissement(self.ctx, &m, &e.objet().brut) && engagement::etats::correction_due(self.ctx, v, &departs, &m, mois).map_err(plain)?.is_some() {
                    return Ok(true);
                }
            }
            for n in engagement::etats::non_actifs_du_mois(self.ctx, &m).map_err(plain)? {
                if de_l_etablissement(self.ctx, &m, &n.objet().brut) && n.correction_due(self.ctx, &departs, &m, mois).map_err(plain)?.is_some() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // ------------------------------------------------------- les autres routes

    /// `exercice_clos` : la cloture de l'annee de ce mois, ou `None`. `config/exercices.json` est celui de la cloture
    /// (`compagnie::exercice_ecrit`), lu tel qu'il est.
    pub(crate) fn exercice_clos(&self, mois_ou_annee: &str) -> R<bool> {
        let p = self.racine().join(crate::compagnie::exercice_ecrit::EXERCICES);
        if !socle::fichiers::existe(&p) {
            return Ok(false);
        }
        // LU UNE FOIS TANT QU'IL NE CHANGE PAS : `etat` le demandait pour chaque mois. Le refus d'un fichier
        // illisible ne change pas (`OSError`, `JSONDecodeError`), et ne se garde pas.
        let v = socle::fichiers::json_en_cache_ou(&p, |p| {
            let t = socle::fichiers::lire_texte(p).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
            serde_json::from_str::<Value>(&t).map_err(|e| erreur("JSONDecodeError", e.to_string()))
        })?;
        let Value::Object(o) = &*v else {
            return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(&v))));
        };
        let annee: String = mois_ou_annee.chars().take(4).collect();
        Ok(o.iter().filter(|(k, _)| !k.starts_with('_')).find(|(k, _)| **k == annee).is_some_and(|(_, x)| vrai(x)))
    }

    /// `mois_a_ouvrir` : le plus ancien mois passe dont une etape n'est pas
    /// verte, sinon le mois du calendrier.
    fn mois_a_ouvrir(&mut self) -> R<MoisAOuvrir> {
        let courant = self.ctx.aujourdhui.mois_civil();
        let courant_texte = courant.to_string();
        // A L'OUVERTURE, RIEN NE SE CALCULE : les groupes d'un mois passe se tirent de ce qui est ecrit.
        self.ecrit = true;
        // Un mois passe sans engagement dont la DSN est due (ce que les rappels reclament) s'ouvre aussi.
        for m in crate::routes::rappels::mois_a_parcourir(self.ctx, self.ctx.aujourdhui)? {
            if m >= courant_texte || self.exercice_clos(&m)? {
                continue;
            }
            let reste = self.etapes_restantes(&m)?;
            if !reste.is_empty() {
                let noms: Vec<&str> = ORDRE.iter().filter(|c| reste.contains(c)).map(|c| c.nom()).collect();
                let phrase = format!("À ouvrir : {}, où il reste {}.", Mois::en_clair(&m), noms.join(", "));
                return Ok(MoisAOuvrir { phrase, mois: Mois::lire(&m).map_err(plain)?, courant, reste });
            }
        }
        let phrase = format!("À ouvrir : {}, le mois du jour ; rien ne reste aux mois d'avant.", Mois::en_clair(&courant_texte));
        Ok(MoisAOuvrir { phrase, mois: courant, courant, reste: Vec::new() })
    }

    /// LES ETAPES QUI RESTENT A UN MOIS PASSE : celles dont un groupe n'est pas fait (`bilan_du_mois`).
    pub(crate) fn etapes_restantes(&mut self, mois: &str) -> R<Vec<QuelleEtape>> {
        let b = self.bilan_du_mois(mois)?;
        Ok(b.etapes.iter().filter(|e| e.groupes.iter().any(|x| !matches!(x.etat, EtatCase::Fait | EtatCase::Rien))).map(|e| e.cle).collect())
    }

    /// `embauches_a_declarer` : les contrats des engagements valides dont la DPAE n'est pas encore partie, chacun avec
    /// l'empreinte de ce que sa DPAE declare. La seule liste des DPAE a faire : le panneau, les rappels, le filet,
    /// l'horloge la lisent.
    pub fn embauches_a_declarer(&mut self, mois: Option<&str>) -> R<Vec<DpaeDue>> {
        // UNE DERIVATION POUR TOUS LES MOIS : le bilan de chaque mois charge sa saisie, et les conges d'un
        // permanent y lisent les paies validees des mois d'avant (`conges::paies_du_mois`) une fois pour toute la route.
        // Rien ne s'ecrit entre deux mois de cette lecture : le memo vaut pour toute la route.
        let _derivation = engagement::conges::Derivation::ouvrir();
        let departs = self.departs()?;
        let mois: Vec<String> = match mois.filter(|m| !m.is_empty()) {
            Some(m) => vec![m.to_string()],
            None => {
                let mut v = engagement::stock::mois_saisis(self.racine()).unwrap_or_default();
                v.reverse();
                v
            }
        };
        let mut attente = Vec::new();
        for m in mois {
            if !engagement::stock::a_une_saisie(self.racine(), &m).unwrap_or(false) {
                continue;
            }
            let Ok(s) = self.charger_saisie(&m) else { continue };
            let le_mois = Mois::lire(&m).map_err(plain)?;
            let etats_m = engagement::etats::tels_qu_ecrits(self.ctx, &m, &departs).map_err(plain)?;
            // La DPAE part de l'engagement valide : un engagement en saisie n'a rien a declarer. Chacun retrouve son embauche
            // par son identite (salarie, numero du contrat).
            for e in &etats_m {
                let Some(v) = e.valide() else { continue };
                let brut = &*v.objet().brut;
                let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(self.ctx, &brut), brut.get("numero").and_then(Value::as_str)) else { continue };
                let Some(p) = s.paies.iter().find(|p| p.salarie.id == cle.as_str() && p.engagement.numero.as_deref() == Some(numero)) else {
                    continue;
                };
                // Seule la DPAE partie se compare (`debut_declare`) : les paies parties ne se relisent pas ici, sauf
                // quand le mois a deja compare tout ce qui est a refaire (`a_refaire_du_mois`).
                let refaire: Vec<ARefaire> = match self.refaire.get(&(m.clone(), cle.to_string(), numero.to_string())) {
                    Some(r) => r.iter().filter(|a| matches!(a, ARefaire::Dpae { .. })).cloned().collect(),
                    None => engagement::etats::a_refaire_parmi(self.ctx, v, &departs, &|s| s == engagement::departs::Envoi::Dpae),
                };
                let debut = p.engagement.debut.iso();
                // UNE DPAE PAR EMBAUCHE, reconnue a sa date : « L'embauche d'un salarié ne peut intervenir qu'après
                // déclaration nominative » (code du travail, L. 1221-10), et la DPAE porte la « Date et heure
                // d'embauche » (R. 1221-1, 3°) ; `atelier/reference/sources/code-du-travail/xml/L1221-10.xml` et
                // `R1221-1.xml`. INFERENCE : un cachet ajoute a une date nouvelle d'un engagement deja declare prolonge le
                // meme engagement (un avenant) ; l'embauche et sa date ne changent pas, aucune DPAE ne ressort -- aucun texte
                // depose n'en demande une nouvelle quand la duree (R. 1221-1, 4°) s'allonge. Une date d'embauche avancee,
                // elle, en appelle une (`dpae_d_une_date_changee`, plus bas).
                // La DPAE de cet engagement est partie (`departs`) ; sa date d'embauche n'a pas change depuis (`ARefaire::Dpae`).
                let partie = departs.de(&cle, numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Dpae);
                let ancienne = debut_declare(&refaire);
                if partie && ancienne.is_none() {
                    continue;
                }
                // Un contrat en cours se declare avec son premier mois.
                if !p.engagement.premier_mois().map_err(plain)? {
                    continue;
                }
                // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie : reculee, la DPAE de l'ancienne
                // date vaut ; avancee, une DPAE est a deposer, et la phrase dit pourquoi (`dpae_d_une_date_changee`). Seulement
                // quand une date d'embauche a change : le reste du temps, rien a dire.
                let pourquoi = match ancienne {
                    Some(a) => {
                        let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&a, &debut);
                        if vaut {
                            continue;
                        }
                        Some(phrase)
                    }
                    None => None,
                };
                let titre = if vrai(&p.spectacle) { p.spectacle.get("titre").filter(|t| vrai(t)).cloned() } else { None };
                let Some(numero) = p.engagement.numero.clone() else { continue };
                attente.push(DpaeDue {
                    mois: le_mois,
                    salarie: cle.to_string(),
                    numero,
                    nom: super::nom_a_l_ecran(&p.salarie),
                    spectacle: titre.unwrap_or_else(|| json!("salarié au mois")),
                    debut,
                    fin: p.engagement.fin.map(|f| f.iso()),
                    empreinte: empreinte_de_l_embauche(&p.engagement, &p.salarie),
                    pourquoi,
                });
            }
        }
        Ok(attente)
    }
}

/// « le 05/11/2026 » d'un instant du journal.
fn le_jour(v: &Value) -> String {
    let t = valeur_en_texte(v);
    Jour::lire(t.get(..10).unwrap_or(&t)).map(|j| format!("le {}", j.jj_mm_aaaa())).unwrap_or(t)
}

/// `le_jour` d'un instant qui peut manquer.
fn le_jour_de(t: &Option<String>) -> String {
    le_jour(&json!(t))
}

/// Un carre, son etat et sa phrase.
fn carre(cle: QuelleEtape, nom: impl Into<String>, etat: EtatCase, phrase: String, action: Option<Geste>, reste: &'static str) -> Carre {
    Carre { cle, nom: nom.into(), etat, phrase, action, pieces: None, couleur: etat.couleur(), coupee: None, reste, lien: None, titre: if cle == QuelleEtape::Dpae { "DPAE" } else { "Contrat" } }
}

/// LES TROIS CARRES DE LA LIGNE D'UN ENGAGEMENT VALIDE (DPAE, Contrat, Documents), lus comme les cases du panneau
/// (`Point::de`) : chacun son nom, son etat (`fait`, `a_faire`, `en_attente`, `refuse`), sa phrase (l'infobulle), le
/// geste qu'ouvre un clic, et le titre de sa boite. Le contrat ou l'avenant parti et pas revenu se note signe d'un clic
/// (`contrat-recu`).
fn carres(p: &Vue, mois: &str) -> Vec<Carre> {
    let pt = Point::de(p, 0);
    // A COMPLETER, OU A VALIDER : rien n'est parti de lui ; chaque carre qui l'attend dit ce qui reste, sans geste.
    if p.a_completer.is_some() || !p.valide() {
        let manque = p.a_completer.as_deref().unwrap_or_default();
        let attend = |cle: QuelleEtape, nom: &str, point: &str| match point {
            A_COMPLETER => carre(cle, nom, EtatCase::AFaire, format!("À compléter avant de valider : {}", manque), None, "à compléter"),
            A_VALIDER => carre(cle, nom, EtatCase::AFaire, format!("{} après la validation", nom), None, "à valider"),
            _ => carre(cle, nom, EtatCase::Fait, "Rien à faire ici pour cet engagement".into(), None, ""),
        };
        return vec![attend(QuelleEtape::Dpae, "DPAE", pt.dpae), attend(QuelleEtape::Signature, "Contrat", pt.contrat), attend(QuelleEtape::Documents, "Documents", pt.documents)];
    }
    let d = &p.etats.dpae;
    let s = &p.etats.signature;
    let o = &p.etats.documents;
    let embauche = p.contrat.premier_mois();
    let debut_dit = Jour::lire(&p.debut).map(|j| j.jj_mm_aaaa()).unwrap_or_default();
    let depuis = Mois::en_clair(p.contrat.depuis());
    let route = |a: Action| Some(Geste::Route(a));

    let (etat, phrase, geste) = match pt.dpae {
        "refuse" => {
            let motif = motif_dpae(d);
            (EtatCase::Refuse, format!("DPAE refusée {} : {}", le_jour(&d.fait), motif), route(historique(mois)))
        }
        "a_faire" => (EtatCase::AFaire, "DPAE à déposer".to_string(), route(declarer_embauches(mois))),
        RIEN_A_DECLARER => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à déclarer ici", debut_dit), None),
        _ if !embauche => (EtatCase::Fait, format!("DPAE faite en {}, à l'embauche", depuis), None),
        _ if d.passee => (EtatCase::Fait, format!("DPAE déposée {}, passée", le_jour(&d.fait)), None),
        _ => (EtatCase::Fait, format!("DPAE déposée {}, retour pas encore arrivé", le_jour(&d.fait)), None),
    };
    let pieces = Pieces { vide: Some("Aucune DPAE déposée pour cette embauche.".into()), encart: None, gestes: Vec::new() };
    let dpae = Carre { pieces: Some(pieces), ..carre(QuelleEtape::Dpae, "DPAE", etat, phrase, geste, "") };

    let cdi = &s.cdi_en_cours;
    let ailleurs = p.contrat.ailleurs();
    let dossier = p.contrat.dossier();
    let nom_contrat = match cdi {
        Some(c) if !ailleurs => format!("CDI, signé en {}", Mois::en_clair(&c.signe_en)),
        _ => "Contrat".to_string(),
    };
    let par_dit = if s.par.as_str() == Some("imprime") { "imprimé" } else { "envoyé" };
    let (etat, phrase, geste) = match pt.contrat {
        "a_envoyer" => (EtatCase::AFaire, "Contrat à envoyer".to_string(), route(faire_signer(mois))),
        "a_refaire" if !s.parti() => (EtatCase::AFaire, majuscule_initiale(s.pourquoi), route(faire_signer(mois))),
        "a_refaire" => (EtatCase::AFaire, format!("Contrat parti {} : {}", le_jour_de(&s.fait), s.pourquoi), route(faire_signer(mois))),
        // Un avenant parti, imprime ou par courriel, se note signe d'un clic, comme le contrat ; la relance est dans la
        // boite des contrats en attente. `par` est celui du dernier depart, l'avenant attendu.
        "en_attente" if s.avenant_attendu => (
            EtatCase::EnAttente,
            format!("Avenant n° {} {} {}", valeur_en_texte(&s.avenant["numero"]), par_dit, le_jour(&s.avenant["quand"])),
            route(noter_recu(p, mois, "Noter l'avenant signé", s.avenant["numero"].clone(), true)),
        ),
        "en_attente" => (EtatCase::EnAttente, format!("Contrat {} {}", par_dit, le_jour_de(&s.fait)), route(noter_recu(p, mois, "Noter le contrat signé", Value::Null, true))),
        RIEN_A_SIGNER if ailleurs && !p.contrat.anterieur() => (EtatCase::Fait, "Contrat signé hors de PuppetPaie : rien à signer ici".to_string(), geste_du_dossier(dossier)),
        RIEN_A_SIGNER => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à signer ici", debut_dit), geste_du_dossier(dossier)),
        _ if cdi.is_some() => (EtatCase::Fait, format!("CDI en cours depuis {}, sans terme : rien à envoyer", depuis), None),
        _ if !embauche => (EtatCase::Fait, format!("Contrat signé en {}", depuis), None),
        _ if !p.contrat.ecrit() => (EtatCase::Fait, "Pas de contrat à faire signer ici".to_string(), None),
        _ => (EtatCase::Fait, format!("Contrat signé {}", le_jour(&s.recu)), None),
    };
    let contrat = Carre { pieces: Some(pieces_du_contrat(p, mois, s, dossier)), ..carre(QuelleEtape::Signature, nom_contrat, etat, phrase, geste, "") };

    let (etat, phrase, geste) = match pt.documents {
        "a_renvoyer" => (EtatCase::AFaire, format!("Documents à renvoyer : {}", o.pourquoi), route(envoyer_documents(mois, "Renvoyer les documents corrigés…".into()))),
        "a_envoyer" => (EtatCase::AFaire, "Documents à envoyer".to_string(), route(envoyer_documents(mois, "Envoyer les documents…".into()))),
        _ => (EtatCase::Fait, format!("Documents partis {}", le_jour_de(&o.fait)), None),
    };
    let documents = carre(QuelleEtape::Documents, "Documents", etat, phrase, geste, "");
    // CE QUI RESTE, EN UN MOT, sur le carre meme (la phrase entiere est l'infobulle).
    let restes = [
        match pt.dpae { "refuse" => "refusée", "a_faire" => "à déposer", _ => "" },
        match pt.contrat { "a_envoyer" => "à envoyer", "a_refaire" => "à refaire signer", "en_attente" => "signature attendue", _ => "" },
        match pt.documents { "a_renvoyer" => "à renvoyer", "a_envoyer" => "à envoyer", _ => "" },
    ];
    // LE LIEN DU DETAIL vers ce qui est parti : seulement quand une piece est passee par le logiciel, ou vers le contrat
    // signe hors du logiciel, que son dossier garde.
    let liens = [
        vrai(&d.fait).then_some("La DPAE partie"),
        if ailleurs { Some("Le contrat signé") } else { s.parti().then_some("Le contrat parti") },
        None,
    ];
    let mut tous = vec![dpae, contrat, documents];
    for ((c, reste), lien) in tous.iter_mut().zip(restes).zip(liens) {
        c.coupee = Some(c.etat == EtatCase::EnAttente);
        c.reste = reste;
        c.lien = Some(lien);
    }
    tous
}

/// CE QUE MONTRE LE DETAIL DU CONTRAT d'un engagement (les pieces parties, ouvertes depuis la ligne) : la phrase quand
/// aucune piece n'est passee par le logiciel, l'encart d'un contrat qui n'est plus le bon, et les gestes, chacun son
/// libelle, son geste et s'il est le principal. Un contrat parti ou recu se note signe (ou ne l'est plus) ; un contrat
/// parti se modifie par un avenant, signe ou non, commence ou non ; un contrat jamais parti part a la signature. Un
/// contrat signe hors du logiciel (`dossier` : son dossier, `signature::contrat_signe_dit`) ne part pas : son PDF se range
/// dans son dossier, et il se modifie par un avenant.
fn pieces_du_contrat(p: &Vue, mois: &str, s: &Signature, dossier: &Value) -> Pieces {
    let (fait, recu, caduc) = (s.parti(), vrai(&s.recu), s.caduc);
    let ailleurs = vrai(dossier);
    // Un contrat ou un avenant parti (`fait`, imprime ou par courriel) est une piece : le detail la nomme, il n'a pas de
    // phrase « sans piece ».
    let vide = match (fait, recu) {
        _ if ailleurs => dossier["phrase"].as_str().map(String::from),
        (true, _) => None,
        (false, true) => Some("Le contrat a été noté signé à la main : aucun fichier n'est passé par le logiciel.".into()),
        (false, false) => Some("Aucun contrat n'est parti à la signature pour cet engagement.".into()),
    };
    let encart = caduc.then(|| {
        let pourquoi = majuscule_initiale(s.pourquoi);
        let pourquoi = if pourquoi.is_empty() { "L'engagement a changé depuis qu'il est parti.".to_string() } else { format!("{}.", pourquoi) };
        EncartDePiece { titre: "Ce contrat n'est plus le bon", texte: format!("{} Il se modifie par un avenant, que le salarié signe à son tour.", pourquoi) }
    });
    let route = |a: Action| Some(Geste::Route(a));
    let mut gestes = Vec::new();
    if ailleurs {
        gestes.push(GesteDePiece { libelle: "Ranger le contrat signé…", titre: None, action: geste_du_dossier(dossier), principal: !caduc });
        if caduc || fait {
            gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat signé hors de PuppetPaie : ce qui change, en clair"), action: route(avenant(p, mois)), principal: caduc });
        }
        return Pieces { vide, encart, gestes };
    }
    if fait || recu {
        let libelle = if recu { "Retirer la mention « signé »" } else { "Noter le contrat signé reçu" };
        gestes.push(GesteDePiece { libelle, titre: None, action: route(noter_recu(p, mois, libelle, Value::Null, !recu)), principal: false });
    }
    if fait {
        gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat parti : l'ancien texte et le nouveau, article par article"), action: route(avenant(p, mois)), principal: caduc });
    }
    if !fait {
        gestes.push(GesteDePiece { libelle: "Envoyer à la signature…", titre: None, action: route(faire_signer(mois)), principal: true });
    }
    Pieces { vide, encart, gestes }
}

/// Le geste qui ouvre le dossier d'un contrat signe hors du logiciel (`signature::contrat_signe_dit`) : la fenetre l'ouvre
/// et le cree ; la phrase dit ce qu'on y depose. Rien pour un autre contrat.
fn geste_du_dossier(dossier: &Value) -> Option<Geste> {
    if !vrai(dossier) {
        return None;
    }
    Some(Geste::Dossier(GesteDuDossier { libelle: "Ranger le contrat signé…", chemin: dossier["chemin"].clone(), phrase: dossier["phrase"].clone() }))
}

/// « Un terme… » d'« un terme… ».
fn majuscule_initiale(t: &str) -> String {
    let mut c = t.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT : s'il se coche (une embauche du mois), s'il est coche d'avance (pas de
/// DPAE, ou une DPAE refusee : c'est la seule raison de redeposer ; une DPAE passee ne se redeclare pas), et son etat
/// (`fait`, `a_faire`, `en_attente`, `refuse`) avec sa phrase.
fn choix_dpae(p: &Vue) -> Choix {
    let d = &p.etats.dpae;
    let choix = |cochable: bool, cochee: bool, etat: EtatCase, phrase: String| Choix { cochable, cochee, etat, couleur: etat.couleur(), phrase };
    // Il lui manque un fait : sa DPAE attend qu'il soit complet.
    if let Some(manque) = &p.a_completer {
        return choix(false, false, EtatCase::AFaire, format!("à compléter avant de valider : {}", manque));
    }
    if p.contrat.anterieur() {
        return choix(false, false, EtatCase::Fait, "contrat commencé avant PuppetPaie : rien à déclarer ici".into());
    }
    if !p.contrat.premier_mois() {
        let depuis = Mois::en_clair(p.contrat.depuis());
        return choix(false, false, EtatCase::Fait, format!("contrat en cours depuis {} — sa DPAE a été faite ce mois-là", depuis));
    }
    // RIEN NE PART D'UN ENGAGEMENT EN SAISIE : le depot le refuserait, et les autres avec lui.
    if !p.valide() {
        return choix(false, false, EtatCase::AFaire, "à valider avant sa DPAE".into());
    }
    // CE QUI SE COCHE EST CE QUE LE DEPOT PREND : la liste des DPAE a faire (`a_declarer`).
    if d.a_declarer && !d.refusee {
        return choix(true, true, EtatCase::AFaire, "à déclarer".into());
    }
    let le = le_jour(&d.fait);
    if d.refusee {
        let motif = motif_dpae(d);
        return choix(true, true, EtatCase::Refuse, format!("refusée {} — {} — à redéclarer", le, motif));
    }
    if d.passee {
        let dossier = if d.deja_existante {
            " (déjà existante à l'Urssaf, rien de créé)".to_string()
        } else if vrai(&d.dossier) {
            format!(" (dossier {})", valeur_en_texte(&d.dossier))
        } else {
            String::new()
        };
        return choix(true, false, EtatCase::Fait, format!("déclarée {} — passée{}", le, dossier));
    }
    choix(true, false, EtatCase::EnAttente, format!("déclarée {} — retour pas encore arrivé : on attend, on ne renvoie pas", le))
}

/// Les motifs d'un refus de DPAE, lus des anomalies du retour : « E99 : anomalie fictive », ou « motif non lu ».
fn motif_dpae(d: &EtatDpae) -> String {
    let motifs: Vec<String> = d
        .anomalies
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| match (a.get("code").map(valeur_en_texte).filter(|c| !c.is_empty()), a.get("message").map(valeur_en_texte).filter(|m| !m.is_empty())) {
            (Some(c), Some(m)) => format!("{} : {}", c, m),
            (None, Some(m)) => m,
            (Some(c), None) => c,
            (None, None) => valeur_en_texte(a),
        })
        .collect();
    if motifs.is_empty() { "motif non lu".to_string() } else { motifs.join(" ; ") }
}

/// Les documents d'un salarie partis pour un mois (`documents_partis`) : quand, et ce que leur envoi en a resume
/// (`envois::etats::resumes_des_documents`, lu tel qu'il est).
#[derive(Debug, Clone)]
pub(crate) struct DocumentsPartis {
    pub(crate) quand: String,
    pub(crate) resume: Value,
}

/// LES DOCUMENTS PARTIS DE `mois`, par salarie : le dernier envoi de ceux dont tous les engagements valides du mois sont
/// partis.
pub(crate) fn documents_partis(ctx: &Contexte, mois: &str, etats: &[engagement::etats::Etat], lignes: &[Value]) -> R<HashMap<String, DocumentsPartis>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let resumes = etats::resumes_des_documents(lignes, mois)?;
    let mut out: HashMap<String, DocumentsPartis> = HashMap::new();
    // Les documents d'un salarie partent ensemble, pour tous ses engagements valides du mois : un engagement valide dont
    // ils ne sont pas partis (valide apres leur envoi) les rend a envoyer, avec son bulletin.
    let a_envoyer: HashSet<String> = etats
        .iter()
        .filter(|e| matches!(e, engagement::etats::Etat::Valide(_)))
        .filter_map(|e| engagement::cle_de_l_engagement(ctx, &e.objet().brut).map(String::from))
        .collect();
    for e in etats {
        if !matches!(e, engagement::etats::Etat::Partis(_)) {
            continue;
        }
        let brut = &*e.objet().brut;
        let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &brut).map(String::from), brut.get("numero").filter(|n| vrai(n)).map(valeur_en_texte)) else { continue };
        let dernier = departs
            .de(&cle, &numero)
            .iter()
            .filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.as_deref() == Some(mois) && d.par.is_none())
            .map(|d| d.le.clone())
            .max();
        let Some(quand) = dernier else { continue };
        if a_envoyer.contains(&cle) {
            continue;
        }
        let plus_recent = out.get(&cle).is_some_and(|v| v.quand >= quand);
        if !plus_recent {
            out.insert(cle.clone(), DocumentsPartis { resume: resumes.get(&cle).cloned().unwrap_or(Value::Null), quand });
        }
    }
    Ok(out)
}
/// La phrase de la pastille Documents d'un salarie dont la paie a change depuis l'envoi.
pub(crate) const DOCUMENTS_CADUCS: &str = "la paie a changé depuis l'envoi";

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois. `parti_le` : le dernier envoi de ses documents ; `caduc` : ils
/// sont partis, et sa paie a change depuis la version sur laquelle ils sont partis (`engagement::etats::a_refaire`) --
/// ils ne sont plus les bons, ils sont a renvoyer.
fn etat_documents(parti_le: Option<String>, caduc: bool) -> EtatDocuments {
    let d = EtatDocuments { fait: parti_le, caduc: false, pourquoi: "" };
    let caduc = d.parti() && caduc;
    EtatDocuments { caduc, pourquoi: if caduc { DOCUMENTS_CADUCS } else { "" }, ..d }
}

/// `DSN_[^\W\d_]+\d{4}(_AR\d+)?\.dsn` : le nom d'une DSN mensuelle.
pub(crate) fn est_une_dsn_mensuelle(nom: &str) -> bool {
    let Some(reste) = nom.strip_prefix("DSN_") else { return false };
    let Some(reste) = reste.strip_suffix(".dsn") else { return false };
    let c: Vec<char> = reste.chars().collect();
    let lettres = c.iter().take_while(|x| etats::lettre_ou_chiffre(**x) && !x.is_ascii_digit() && **x != '_' && !x.is_numeric()).count();
    if lettres == 0 {
        return false;
    }
    let suite = &c[lettres..];
    if suite.len() < 4 || !suite[..4].iter().all(char::is_ascii_digit) {
        return false;
    }
    let fin = &suite[4..];
    fin.is_empty() || (fin.len() > 3 && fin[..3] == ['_', 'A', 'R'] && fin[3..].iter().all(char::is_ascii_digit))
}

/// La phrase de la pastille Contrat d'un engagement qui a change depuis la version sur laquelle son contrat est parti.
pub(crate) const CONTRAT_A_REFAIRE: &str = "l'engagement a changé depuis l'envoi du contrat : un avenant est à faire signer";

/// La meme, pour un contrat signe hors du logiciel (`engagement::signe_ailleurs`) dont un terme a change.
pub(crate) const CONTRAT_AILLEURS_A_MODIFIER: &str = "un terme du contrat signé hors de PuppetPaie a changé : un avenant est à faire signer";

/// La pastille Contrat d'un engagement.
///
/// `caduc` : le contrat parti -- signe ou non -- ne dit plus ce que dit l'engagement : ses clauses ont change depuis la
/// version sur laquelle il est parti (`a_refaire_signer`, `engagement::etats::a_refaire`), ou il porte un autre numero
/// que celui que la DSN declare. Un contrat renvoye attend une NOUVELLE signature : celle de l'ancien ne compte plus.
#[allow(clippy::too_many_arguments)]
pub(crate) fn etat_signature(
    fait: Option<String>, par: &Value, recu: &Value, renvoye: bool, avenants: &Value,
    a_refaire_signer: bool, ailleurs: bool,
) -> Signature {
    let recu = if renvoye { &Value::Null } else { recu };
    let nul = Value::Null;
    let s = Signature { fait, ..Signature::default() };
    let parti = s.parti();
    // Un contrat signe hors du logiciel (`ailleurs`) vaut un contrat parti : un terme qui change s'y dit par un avenant.
    let caduc = (parti || ailleurs) && a_refaire_signer;
    // L'AVENANT : le dernier parti. Pas encore revenu, il laisse la pastille orange.
    let dernier = avenants.as_array().and_then(|a| a.last()).cloned().unwrap_or(Value::Null);
    let attendu = vrai(&dernier) && !vrai(dernier.get("signe").unwrap_or(&nul));
    let pourquoi = match (caduc, ailleurs) {
        (false, _) => "",
        (true, false) => CONTRAT_A_REFAIRE,
        (true, true) => CONTRAT_AILLEURS_A_MODIFIER,
    };
    Signature {
        par: if parti { par.clone() } else { Value::Null },
        recu: recu.clone(),
        caduc,
        pourquoi,
        avenant: dernier,
        avenant_attendu: attendu,
        ..s
    }
}

// ----------------------------------------------------------- l'aiguillage

/// Le mois d'une charge `{"mois"}`, lu en sa forme (`ChargeMois`).
pub(crate) fn mois_de(charge: &Value) -> R<Mois> {
    lire_charge::<ChargeMois>(charge).map(|c| c.mois)
}

/// LES ENGAGEMENTS NON ACTIFS que `mois` montrerait (`engagement::etats::non_actifs_du_mois`), pour qu'ils se lisent :
/// le salarie, son nom, le contrat, depuis quand, et ce que leur desactivation rend a faire (`a_faire`). L'engagement se
/// lit tel qu'il est ecrit (`brut`).
fn non_actifs(ctx: &Contexte, mois: &str) -> R<Vec<NonActif>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let mut out = Vec::new();
    for n in engagement::etats::non_actifs_du_mois(ctx, mois).map_err(plain)? {
        let b = &*n.objet().brut;
        let cle = engagement::cle_de_l_engagement(ctx, &b).map(String::from).unwrap_or_else(|| b.get("salarie").map(valeur_en_texte).unwrap_or_default());
        let nom = ctx
            .salarie(&cle)
            .ok()
            .map(|f| super::nom_a_l_ecran(&f))
            .unwrap_or_else(|| cle.clone());
        let debut = engagement::debut_brut(&ctx.racine, &b);
        let fin_ecrite = b.get("fin").filter(|v| vrai(v)).cloned().unwrap_or(Value::Null);
        let fin = match &fin_ecrite {
            Value::Null => engagement::dates_engagement(&ctx.racine, &b).iter().map(|d| d.get("jour").map_or_else(|| valeur_en_texte(d), valeur_en_texte)).max(),
            f => Some(valeur_en_texte(f)),
        };
        let numero = b.get("numero").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| "sans numéro".to_string());
        out.push(NonActif {
            salarie: cle,
            nom: nom.trim().to_string(),
            numero: b.get("numero").and_then(Value::as_str).map(String::from),
            dit: format!("contrat {}, {}", numero, super::periode_en_clair(&json!(debut), &fin_ecrite)),
            debut,
            fin,
            non_actif: n.le().to_string(),
            non_actif_dit: format!("non actif depuis le {}", super::quand(n.le())),
            a_faire: crate::routes::engagement::a_faire_en_phrases(&n.a_refaire(ctx, &departs)),
        });
    }
    Ok(out)
}

/// Une route de ce module. `charge` : ce que la page envoie.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    lire(&mut Moteur::nouveau(ctx), nom, charge)
}

/// LES ROUTES `noms`, servies par une meme lecture (`Moteur`) : ce que l'etat garde d'un mois rend en une fois
/// (`etat_garde::refaire`). Chacune rend ce que sa route seule rendrait.
pub fn lire_ensemble(ctx: &Contexte, noms: &[Route], charge: &Value) -> R<Vec<Value>> {
    let mut m = Moteur::nouveau(ctx);
    noms.iter().map(|nom| lire(&mut m, *nom, charge)).collect()
}

fn lire(m: &mut Moteur, nom: Route, charge: &Value) -> R<Value> {
    let ctx = m.ctx;
    match nom {
        Route::EtatParEngagement => {
            let mois = mois_de(charge)?;
            let lignes = m.etat_par_engagement(&mois.to_string())?;
            en_json(EtatParEngagement { mois, engagements: lignes.iter().map(LigneDePage::de).collect(), non_actifs: non_actifs(ctx, &mois.to_string())? })
        }
        Route::EtatDuMois => {
            let mois = mois_de(charge)?;
            en_json(m.bilan_du_mois(&mois.to_string())?)
        }
        Route::MoisAOuvrir => {
            let SansCharge {} = lire_charge(charge)?;
            en_json(m.mois_a_ouvrir()?)
        }
        autre => Err(plain(format!("route inconnue : {}", autre))),
    }
}

/// Ce que la DPAE d'une embauche declare, en 16 caracteres : l'identite du salarie telle que l'engagement la lit, les
/// bornes du contrat, le spectacle, la nature ; pas les taux ni les cachets. Un test de DPAE fait sur une autre empreinte
/// ne vaut plus.
fn empreinte_de_l_embauche(e: &engagement::Engagement, sal: &droit::fiche::ChampsSalarie) -> String {
    let r = |t: &str| socle::fichiers::repr(t);
    // La derniere place est celle d'une case `nationalite` que la fiche n'a pas : vide, comme l'empreinte l'a toujours ecrite.
    let ident: Vec<String> = vec![
        r(sal.nir.texte()), r(sal.nom.texte()), r(sal.prenom.texte()), r(&sal.date_naissance.dite().map(|j| j.iso()).unwrap_or_default()),
        r(sal.lieu_naissance.texte()), r(sal.departement_naissance.texte()), r(sal.sexe.dite().map_or("", |s| s.as_str())), r(""),
    ];
    let fin = e.fin.map_or("None".to_string(), |f| f.iso());
    let spectacle = e.spectacle.clone().unwrap_or_else(|| "None".to_string());
    let brut = format!(
        "({}, {}, {}, ({}), {})",
        socle::fichiers::repr(&e.debut.iso()),
        socle::fichiers::repr(&fin),
        socle::fichiers::repr(&spectacle),
        ident.join(", "),
        socle::fichiers::repr(e.nature.as_deref().unwrap_or(""))
    );
    sha256_hex(brut.as_bytes())[..16].to_string()
}
