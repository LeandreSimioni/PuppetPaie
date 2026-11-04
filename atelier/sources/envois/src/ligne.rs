//! LA LIGNE DU JOURNAL DES ENVOIS, telle que `journal` l'ecrit et la relit : une structure par sorte d'envoi (`Ligne`,
//! etiquetee par `quoi`), qui porte ce que toutes partagent (`Envoi` : quand, mode, accepte, flux, versions...), le
//! detail de sa sorte et ce qui en revient (`retours`). Chaque forme se lit strictement (`deny_unknown_fields`) : une
//! ligne qui porte une cle inconnue ne se lit pas. Le numero d'un contrat y est un `NumeroContrat`, le mois un `Mois`,
//! le salarie un `IdSalarie`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use droit::contexte::IdSalarie;
use engagement::numeros::NumeroContrat;
use socle::mois::Mois;

/// Le mode d'un envoi : reel (il engage), ou test (un depot de test, qui ne conserve rien).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Reel,
    Test,
}

impl Mode {
    pub fn de(reel: bool) -> Mode {
        if reel {
            Mode::Reel
        } else {
            Mode::Test
        }
    }

    /// Son nom, tel que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Reel => "reel",
            Mode::Test => "test",
        }
    }
}

/// La sorte d'un envoi, telle que `quoi` l'ecrit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Acte {
    Dsn,
    Dpae,
    Documents,
    ContratSignature,
    ContratContresigne,
    RegularisationCasc,
    DossierSalarie,
    FicheDemande,
}

impl Acte {
    /// Son nom, tel que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Acte::Dsn => "dsn",
            Acte::Dpae => "dpae",
            Acte::Documents => "documents",
            Acte::ContratSignature => "contrat-signature",
            Acte::ContratContresigne => "contrat-contresigne",
            Acte::RegularisationCasc => "regularisation-casc",
            Acte::DossierSalarie => "dossier-salarie",
            Acte::FicheDemande => "fiche-demande",
        }
    }
}

/// Ce qui ne revient jamais d'un envoi (des documents, un contrat contresigne...) : sa liste `retours` reste vide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Aucun {}

/// LA VERSION D'UN ENGAGEMENT QU'UN ENVOI EMPORTE (`journal::versions_du_mois`) : la cle du salarie, le numero du contrat
/// et la version ; `mois`, pour la correction d'un mois passe que porte une DSN (`produire_regularisations`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub salarie: IdSalarie,
    pub numero: NumeroContrat,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
}

/// CE QUE TOUTE LIGNE PORTE, et le detail (`D`) et les retours (`R`) de sa sorte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envoi<D, R> {
    /// L'heure de l'envoi (ISO, a la seconde), ou celle qu'une reprise lit dans sa piece.
    pub quand: String,
    /// D'ou vient une heure qui n'est pas celle de l'acte (une reprise, l'horloge d'une fictive) ; vide sinon.
    pub origine: String,
    pub mois: Option<Mois>,
    /// Le nom du fichier parti, sans son dossier.
    pub fichier: Option<String>,
    pub empreinte: Option<String>,
    pub mode: Mode,
    pub qui: String,
    pub accepte: bool,
    pub destinataire: String,
    pub flux: String,
    pub retours: Vec<R>,
    pub reponse: String,
    /// La reponse de l'organisme archivee, relative a la compagnie.
    pub trace: Option<String>,
    pub detail: D,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<Version>>,
}

/// UNE LIGNE DU JOURNAL : une sorte d'envoi, son detail, ses retours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "quoi", rename_all = "kebab-case")]
pub enum Ligne {
    Dsn(Envoi<DetailDsn, RetourOrganisme>),
    Dpae(Envoi<DetailDpae, RetourOrganisme>),
    Documents(Envoi<DetailDocuments, Aucun>),
    ContratSignature(Envoi<DetailSignature, RetourContrat>),
    ContratContresigne(Envoi<DetailContresigne, Aucun>),
    RegularisationCasc(Envoi<DetailCasc, Aucun>),
    DossierSalarie(Envoi<DetailDossier, Aucun>),
    FicheDemande(Envoi<DetailFiche, RetourFiche>),
}

/// `$x` sur la part commune (`Envoi`) de la ligne `$l`, quelle que soit sa sorte.
macro_rules! commun {
    ($l:expr, $e:ident => $x:expr) => {
        match $l {
            Ligne::Dsn($e) => $x,
            Ligne::Dpae($e) => $x,
            Ligne::Documents($e) => $x,
            Ligne::ContratSignature($e) => $x,
            Ligne::ContratContresigne($e) => $x,
            Ligne::RegularisationCasc($e) => $x,
            Ligne::DossierSalarie($e) => $x,
            Ligne::FicheDemande($e) => $x,
        }
    };
}

impl Ligne {
    pub fn acte(&self) -> Acte {
        match self {
            Ligne::Dsn(_) => Acte::Dsn,
            Ligne::Dpae(_) => Acte::Dpae,
            Ligne::Documents(_) => Acte::Documents,
            Ligne::ContratSignature(_) => Acte::ContratSignature,
            Ligne::ContratContresigne(_) => Acte::ContratContresigne,
            Ligne::RegularisationCasc(_) => Acte::RegularisationCasc,
            Ligne::DossierSalarie(_) => Acte::DossierSalarie,
            Ligne::FicheDemande(_) => Acte::FicheDemande,
        }
    }

    pub fn quand(&self) -> &str {
        commun!(self, e => &e.quand)
    }

    pub fn origine(&self) -> &str {
        commun!(self, e => &e.origine)
    }

    pub fn mois(&self) -> Option<Mois> {
        commun!(self, e => e.mois)
    }

    pub fn fichier(&self) -> Option<&str> {
        commun!(self, e => e.fichier.as_deref())
    }

    pub fn empreinte(&self) -> Option<&str> {
        commun!(self, e => e.empreinte.as_deref())
    }

    pub fn mode(&self) -> Mode {
        commun!(self, e => e.mode)
    }

    pub fn reel(&self) -> bool {
        self.mode() == Mode::Reel
    }

    pub fn qui(&self) -> &str {
        commun!(self, e => &e.qui)
    }

    pub fn accepte(&self) -> bool {
        commun!(self, e => e.accepte)
    }

    pub fn destinataire(&self) -> &str {
        commun!(self, e => &e.destinataire)
    }

    pub fn flux(&self) -> &str {
        commun!(self, e => &e.flux)
    }

    pub fn reponse(&self) -> &str {
        commun!(self, e => &e.reponse)
    }

    pub fn trace(&self) -> Option<&str> {
        commun!(self, e => e.trace.as_deref())
    }

    pub fn versions(&self) -> Option<&[Version]> {
        commun!(self, e => e.versions.as_deref())
    }

    /// Le nombre de retours rattaches.
    pub fn nombre_de_retours(&self) -> usize {
        commun!(self, e => e.retours.len())
    }

    /// Les retours des organismes d'une DSN ou d'une DPAE ; aucun pour un autre envoi.
    pub fn retours_organisme(&self) -> &[RetourOrganisme] {
        match self {
            Ligne::Dsn(e) => &e.retours,
            Ligne::Dpae(e) => &e.retours,
            _ => &[],
        }
    }

    /// Le detail d'une DSN.
    pub fn dsn(&self) -> Option<&Envoi<DetailDsn, RetourOrganisme>> {
        match self {
            Ligne::Dsn(e) => Some(e),
            _ => None,
        }
    }

    /// Le detail d'une DPAE.
    pub fn dpae(&self) -> Option<&Envoi<DetailDpae, RetourOrganisme>> {
        match self {
            Ligne::Dpae(e) => Some(e),
            _ => None,
        }
    }

    pub fn signature(&self) -> Option<&Envoi<DetailSignature, RetourContrat>> {
        match self {
            Ligne::ContratSignature(e) => Some(e),
            _ => None,
        }
    }

    pub fn documents(&self) -> Option<&Envoi<DetailDocuments, Aucun>> {
        match self {
            Ligne::Documents(e) => Some(e),
            _ => None,
        }
    }

    pub fn fiche(&self) -> Option<&Envoi<DetailFiche, RetourFiche>> {
        match self {
            Ligne::FicheDemande(e) => Some(e),
            _ => None,
        }
    }

    /// Un envoi remis en main propre (des contrats ou des documents imprimes).
    pub fn main_propre(&self) -> bool {
        match self {
            Ligne::ContratSignature(e) => e.detail.main_propre,
            Ligne::Documents(e) => e.detail.main_propre,
            _ => false,
        }
    }

    /// L'etablissement (son NIC) qu'une DSN declare ; `None` : l'etablissement principal.
    pub fn etablissement(&self) -> Option<&str> {
        self.dsn().and_then(|e| e.detail.etablissement.as_deref()).filter(|n| !n.is_empty())
    }

    /// La nature d'une DSN (`01` mensuelle, `04` `05` `07` `08` un signalement) ; vide si la ligne ne la dit pas.
    pub fn nature_dsn(&self) -> &str {
        self.dsn().and_then(|e| e.detail.nature.as_deref()).unwrap_or("")
    }

    /// Une DSN de signalement (arret, reprise, fin de contrat, amorcage).
    pub fn signalement(&self) -> bool {
        declarations::norme::SIGNALEMENTS.contains(&self.nature_dsn())
    }
}

// ------------------------------------------------------------------ la DSN et la DPAE

/// CE QUI A OUVERT UN DEPOT REEL (`moteur::compagnie::depot_acte::exiger_repetition`) : le test conforme de ce contenu
/// (son heure, son flux, son compte rendu), ou le motif ecrit d'un reel parti sans test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ouverture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux: Option<String>,
    pub empreinte: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compte_rendu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contournement: Option<String>,
}

/// LE DETAIL D'UNE DSN : ce que le fichier deposé declare (sa nature, son type, son ordre, son etablissement, ses
/// versements), ce qui l'a ouverte, ce qui l'a fait echouer (`erreur`) ; pour un depot repris des retours
/// (`reconstruit`), ce que son bilan en dit.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDsn {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erreur: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reconstruit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_declaration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fraction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versements: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salaries: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrats: Option<String>,
    /// Ce qui a ete preleve sur la grille, et non sur le taux du salarie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pas_bareme: Option<usize>,
    pub repetition: Option<Ouverture>,
    pub ordre_test: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etablissement: Option<String>,
    /// Le jour de l'evenement qu'un signalement declare (`AAAA-MM-JJ` : le dernier jour travaille d'un arret, la fin d'un
    /// contrat...) ; la ligne porte le mois de ce jour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evenement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etat_envoi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etat_declaration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict_connu: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<String>>,
}

/// Une embauche qu'une DPAE declare : le salarie, le numero du contrat de son engagement, son premier et son dernier
/// jour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbaucheDeclaree {
    pub salarie: IdSalarie,
    pub numero: Option<NumeroContrat>,
    pub debut: String,
    pub fin: Option<String>,
}

/// LE DETAIL D'UNE DPAE : ce qui l'a ouverte, ses embauches, ce qui l'a fait echouer ; pour une DPAE reprise des
/// certificats (`reconstruit`), les embauches qu'ils attestent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDpae {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erreur: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reconstruit: bool,
    pub repetition: Option<Ouverture>,
    #[serde(default)]
    pub embauches: Vec<EmbaucheDeclaree>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub certificat: bool,
}

/// Une personne du certificat de conformite d'une DPAE (nature 41, `declarations::dpae::lire_certificat`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonneCertifiee {
    pub nom: String,
    pub prenom: String,
    pub embauche: String,
    pub dossier: String,
    pub code: String,
    pub libelle: String,
    pub enregistree: bool,
    pub deja_existante: bool,
    pub formalites: Vec<String>,
    /// Ce que la page en montre (`moteur::routes::depots`) : pose sur ce qu'une route rend, jamais ecrit ni lu au journal.
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub pastille: Option<socle::ecran::Pastille>,
}

impl PersonneCertifiee {
    /// Sa pastille : enregistree, faite ; deja existante, une information ; refusee, refusee.
    pub fn pastille_due(&self) -> socle::ecran::Pastille {
        use socle::ecran::Pastille;
        if self.enregistree {
            Pastille::de("fait", &if self.dossier.is_empty() { "enregistrée".to_string() } else { format!("enregistrée — dossier {}", self.dossier) })
        } else if self.deja_existante {
            Pastille::de("information", "déjà existante")
        } else {
            let mots = [&self.libelle, &self.code].into_iter().find(|t| !t.is_empty()).cloned().unwrap_or_else(|| "refusée".into());
            Pastille::de("refuse", &mots)
        }
    }
}

/// CE QU'UN COMPTE RENDU DIT, lu dans sa piece (`etats::verdict_du_retour`) : son verdict, ses anomalies en phrases, et
/// pour le certificat d'une DPAE, chaque personne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
    pub verdict: String,
    pub anomalies: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpae: Option<Vec<PersonneCertifiee>>,
}

/// CE QU'UN ORGANISME A RENDU d'une DSN ou d'une DPAE : sa nature, son libelle, son statut, sa publication, et ce que sa
/// piece en dit quand elle s'est lue (`verdict`, `anomalies`, `dpae`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourOrganisme {
    pub nature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anomalies: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpae: Option<Vec<PersonneCertifiee>>,
    pub libelle: String,
    pub statut: Option<String>,
    pub publication: Option<String>,
}

impl RetourOrganisme {
    pub fn verdict(&self) -> &str {
        self.verdict.as_deref().unwrap_or("")
    }

    pub fn ko(&self) -> bool {
        self.verdict() == "KO"
    }

    pub fn anomalies(&self) -> &[String] {
        self.anomalies.as_deref().unwrap_or(&[])
    }

    pub fn personnes(&self) -> &[PersonneCertifiee] {
        self.dpae.as_deref().unwrap_or(&[])
    }

    /// Le meme compte rendu (sa nature et sa publication).
    pub fn meme(&self, autre: &RetourOrganisme) -> bool {
        self.nature == autre.nature && self.publication == autre.publication
    }

    /// Ce que sa piece en dit, pose sur lui.
    pub fn poser(&mut self, v: Verdict) {
        self.verdict = Some(v.verdict);
        self.anomalies = Some(v.anomalies);
        if v.dpae.is_some() {
            self.dpae = v.dpae;
        }
    }

    /// Ce qu'un autre poste en savait de plus (un verdict), sans rien retirer.
    pub fn completer(&mut self, autre: &RetourOrganisme) {
        if self.verdict.is_none() {
            self.verdict = autre.verdict.clone();
        }
        if self.anomalies.is_none() {
            self.anomalies = autre.anomalies.clone();
        }
        if self.dpae.is_none() {
            self.dpae = autre.dpae.clone();
        }
    }
}

// ------------------------------------------------------------------ les envois au salarie

/// Les durees d'un envoi de documents, en millisecondes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Durees {
    pub preparation: u64,
    pub mot_de_passe: u64,
    pub envoi: u64,
}

/// Un salarie que des documents ont servi : ses pieces, le resume de sa paie, et si c'etait un renvoi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Servi {
    pub cle: IdSalarie,
    pub nom: Option<String>,
    pub pieces: Vec<String>,
    pub resume: Option<crate::envoi::ResumeDePaie>,
    pub renvoi: bool,
}

/// LE DETAIL D'UN ENVOI DE DOCUMENTS : les salaries servis, imprimes (`main_propre`) ou partis par courriel (de
/// `expediteur`) ; un envoi coupe en route dit ceux qui ont recu (`partis`) et l'adresse ou il a echoue (`echec_a`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDocuments {
    pub salaries: Vec<Servi>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main_propre: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durees_ms: Option<Durees>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<String>,
}

/// UN CONTRAT (OU UN AVENANT) PARTI A LA SIGNATURE : son engagement (salarie, bornes, numero), sa piece, ce que le
/// contrat dit (`clauses`), et ce qu'il faut pour reconnaitre la reponse (le code, l'adresse, le message). Un avenant
/// (`acte`) dit son numero, son effet et ses modifications ; un contrat refait, celui qu'il remplace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratParti {
    pub salarie: IdSalarie,
    pub debut: String,
    pub fin: Option<String>,
    pub numero: Option<NumeroContrat>,
    pub fichier: String,
    pub spectacle: Option<String>,
    pub mois: Mois,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acte: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avenant: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remplace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifie: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifications: Option<Vec<ArticleModifie>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clauses: Option<BTreeMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empreinte: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ailleurs: bool,
}

impl ContratParti {
    /// Un avenant au contrat, pas le contrat.
    pub fn est_avenant(&self) -> bool {
        self.acte.as_deref() == Some("avenant")
    }

    /// Le numero du contrat, tel qu'il sert de cle : son texte, ou `""`.
    pub fn numero_dit(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }
}

/// CE QU'UN AVENANT CHANGE, un article ou un terme : l'article du modele de contrat et son numero, l'ancien texte et le
/// nouveau (`documents::avenant::modifications`, les paragraphes `blocs` de l'article) ; ou, pour un contrat signe hors du
/// logiciel, le terme dit en clair (`phrase`), sans numero, et le jour ou il change (`documents::avenant::changements_en_clair`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleModifie {
    pub numero: Option<usize>,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocs: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jour: Option<String>,
}

impl ArticleModifie {
    /// Un terme dit en clair (un contrat signe hors du logiciel), pas un article cite.
    pub fn en_clair(&self) -> bool {
        self.phrase.as_deref().is_some_and(|p| !p.is_empty())
    }
}

/// LE DETAIL D'UN ENVOI A LA SIGNATURE : ses contrats (ou ses avenants, `acte`), imprimes (`main_propre`) ou partis par
/// courriel (de `expediteur`, une relance ou non) ; un envoi coupe en route dit ceux qui ont recu et ou il a echoue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailSignature {
    pub contrats: Vec<ContratParti>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main_propre: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acte: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<String>,
}

/// CE QUI REVIENT D'UN CONTRAT PARTI (`moteur::routes::contrats::contrat_recu_avec`) : le contrat (salarie, debut,
/// numero, avenant), s'il est revenu signe (`recu`, ou la mention retiree), et, pour une reponse « Je signe » relevee
/// par courriel (`origine`), ce qui la prouve. `quand`, `qui`, `reponse` : la note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourContrat {
    pub salarie: IdSalarie,
    pub debut: String,
    pub recu: bool,
    pub numero: Option<NumeroContrat>,
    pub nom: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avenant: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    pub quand: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fichier_signe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acte: Option<String>,
    pub qui: String,
    pub reponse: String,
}

impl RetourContrat {
    pub fn numero_dit(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }

    /// Une reponse « Je signe » relevee par courriel.
    pub fn par_courriel(&self) -> bool {
        self.origine.as_deref() == Some("courriel")
    }
}

/// UN AVENANT PARTI (`etats::avenants_par_contrat`) : son numero, quand et dans quel mois il est parti, sa piece, son code
/// de signature, le contrat qu'il modifie, son effet, ses modifications, et le jour ou il est revenu signe.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AvenantParti {
    pub numero: Option<i64>,
    pub quand: String,
    pub mois: Mois,
    pub fichier: String,
    pub code: Option<String>,
    pub contrat: Option<NumeroContrat>,
    pub effet: Option<String>,
    pub modifications: Vec<ArticleModifie>,
    pub signe: Option<String>,
}

/// UN CONTRAT QUE LA SIGNATURE ATTEND (`etats::signatures_attendues`), et que le contrat contresigne renvoie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratAttendu {
    pub salarie: IdSalarie,
    pub debut: String,
    pub fin: Option<String>,
    pub numero: Option<NumeroContrat>,
    pub spectacle: String,
    pub fichier: String,
    pub avenant: Option<i64>,
    pub mois: Mois,
}

impl ContratAttendu {
    pub fn numero_dit(&self) -> &str {
        self.numero.as_deref().unwrap_or("")
    }
}

/// LE DETAIL DU CONTRAT CONTRESIGNE RENVOYE AU SALARIE : qui, sous quel code, ses contrats, les PDF contresignes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailContresigne {
    pub salarie: IdSalarie,
    pub nom: String,
    pub code: String,
    pub contrats: Vec<ContratAttendu>,
    pub fichiers: Vec<String>,
}

/// Une annee de la regularisation CASC-SVP : son montant, et le jour du virement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnneeCasc {
    pub annee: String,
    pub vire_le: String,
    pub montant: String,
}

/// LE DETAIL DE LA REGULARISATION CASC-SVP envoyee a Audiens et au CASC-SVP : ses annees, ses mois, son total, les
/// messages gardes (`eml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailCasc {
    pub annees: Vec<AnneeCasc>,
    pub mois: Vec<String>,
    pub total: String,
    pub expediteur: String,
    #[serde(default)]
    pub eml: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<String>,
}

/// Le dossier qu'un salarie a recu : sa piece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DossierServi {
    pub cle: IdSalarie,
    pub piece: Option<String>,
}

/// LE DETAIL DE L'ENVOI DU DOSSIER AUX SALARIES : la periode, ce qu'il porte, qui l'a recu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDossier {
    pub periode: String,
    pub categories: Vec<String>,
    pub salaries: Vec<DossierServi>,
    pub expediteur: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<String>,
}

/// LE DETAIL D'UNE DEMANDE DE FICHE : son code, a qui, et l'identifiant du message parti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailFiche {
    pub code: String,
    pub adresse: String,
    pub prenom: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<String>,
}

/// CE QUI REVIENT D'UNE DEMANDE DE FICHE : la fiche recue (la cle de la fiche, illisible ou non, ses erreurs, le
/// message), ou la demande abandonnee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourFiche {
    pub code: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub abandon: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salarie: Option<IdSalarie>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub illisible: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erreurs: Option<usize>,
    pub quand: String,
    pub qui: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub de: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub reponse: String,
}

/// UN ENVOI COUPE EN ROUTE (`ligne_d_envoi::noter_echec_d_envoi`) : son detail dit ceux qui ont recu et l'adresse ou il
/// a echoue.
pub trait EnRoute {
    fn coupe(&mut self, partis: Vec<String>, echec_a: String);
}

macro_rules! en_route {
    ($($t:ty),*) => {
        $(impl EnRoute for $t {
            fn coupe(&mut self, partis: Vec<String>, echec_a: String) {
                self.partis = Some(partis);
                self.echec_a = Some(echec_a);
            }
        })*
    };
}

en_route!(DetailDocuments, DetailSignature, DetailCasc, DetailDossier);
