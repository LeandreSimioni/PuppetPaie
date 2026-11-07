//! LES MARQUEURS D'UN MODELE DE DOCUMENT (`{{employeur.siret}}`), DECLARES UNE FOIS : chacun une variante de `Marqueur`,
//! et son nom texte, celui que le modele porte, ici seulement. Un modele -- livre (`donnees/modeles/`) ou pose par la
//! compagnie dans son dossier `gabarits/` -- se lit en morceaux texte et marqueur (`ModeleLu::lire`) : un marqueur que
//! le logiciel ne connait pas se refuse a la lecture, en le nommant. Les valeurs d'un document (`Valeurs`) se posent par
//! marqueur ; la fabrique de PDF, qui ne connait que des noms, les recoit par `pour_la_fabrique`.
//!
//! L'ordre de la declaration est celui dans lequel la description d'un document ecrit ses valeurs.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value};

use socle::echec::Refus;

macro_rules! marqueurs {
    ($($v:ident = $nom:literal),* $(,)?) => {
        /// UN MARQUEUR qu'un modele peut citer.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Marqueur { $($v),* }

        impl Marqueur {
            /// Son nom dans le modele, sans les accolades.
            pub fn nom(self) -> &'static str {
                match self { $(Marqueur::$v => $nom),* }
            }

            /// Le marqueur de ce nom ; `None` : le logiciel ne le connait pas.
            pub fn lire(nom: &str) -> Option<Marqueur> {
                match nom { $($nom => Some(Marqueur::$v),)* _ => None }
            }
        }
    };
}

marqueurs! {
    EmployeurRaisonSociale = "employeur.raison_sociale",
    EmployeurAdresse = "employeur.adresse",
    EmployeurCodePostal = "employeur.code_postal",
    EmployeurCommune = "employeur.commune",
    EmployeurSiret = "employeur.siret",
    EmployeurNaf = "employeur.naf",
    EmployeurLicence = "employeur.licence",
    EmployeurIdcc = "employeur.idcc",
    EmployeurConvention = "employeur.convention",
    EmployeurCongesspectacles = "employeur.congesspectacles",
    EmployeurSignataire = "employeur.signataire",
    EmployeurSignataireFonction = "employeur.signataire_fonction",
    EmployeurQualiteDe = "employeur.qualite_de",
    SalarieNom = "salarie.nom",
    SalariePrenom = "salarie.prenom",
    SalarieNomComplet = "salarie.nom_complet",
    SalarieDateNaissance = "salarie.date_naissance",
    SalarieLieuNaissance = "salarie.lieu_naissance",
    SalarieAdresse = "salarie.adresse",
    SalarieCodePostal = "salarie.code_postal",
    SalarieCommune = "salarie.commune",
    SalarieNir = "salarie.nir",
    SalarieEmploi = "salarie.emploi",
    SalarieQualiteDe = "salarie.qualite_de",
    ContratNumero = "contrat.numero",
    ContratDebut = "contrat.debut",
    ContratFin = "contrat.fin",
    ContratDatesTravail = "contrat.dates_travail",
    ContratLieu = "contrat.lieu",
    ContratDatePaiement = "contrat.date_paiement",
    RemunerationDetail = "remuneration.detail",
    RemunerationPeriode = "remuneration.periode",
    RemunerationCachets = "remuneration.cachets",
    RemunerationTauxCachet = "remuneration.taux_cachet",
    RemunerationBrut = "remuneration.brut",
    RemunerationNetEstime = "remuneration.net_estime",
    SpectacleTitre = "spectacle.titre",
    SpectacleObjet = "spectacle.objet",
    CertificatDate = "certificat.date",
    ContratClausesParticulieres = "contrat.clauses_particulieres",
    ContratDureeTravail = "contrat.duree_travail",
    ContratEmploi = "contrat.emploi",
    ContratMotif = "contrat.motif",
    ContratNature = "contrat.nature",
    ContratPaiement = "contrat.paiement",
    ContratPeriodeEssai = "contrat.periode_essai",
    ContratPersonneRemplacee = "contrat.personne_remplacee",
    MentionsConges = "mentions.conges",
    MentionsFormation = "mentions.formation",
    MentionsProtectionSociale = "mentions.protection_sociale",
    MentionsRupture = "mentions.rupture",
    ModificationApres = "modification.apres",
    ModificationArticle = "modification.article",
    ModificationAvant = "modification.avant",
    ModificationNumero = "modification.numero",
    ModificationPhrase = "modification.phrase",
    OrganismesPrevoyance = "organismes.prevoyance",
    OrganismesRetraite = "organismes.retraite",
    RecuDate = "recu.date",
    RemplaceDate = "remplace.date",
    RemplaceNumero = "remplace.numero",
    RemunerationHorsBrut = "remuneration.hors_brut",
    SalarieCaseSignature = "salarie.case_signature",
    SalarieCategorie = "salarie.categorie",
    SoldeInventaire = "solde.inventaire",
    SoldeNet = "solde.net",
    AvenantNumero = "avenant.numero",
    AvenantDateContrat = "avenant.date_contrat",
    AvenantReference = "avenant.reference",
    AvenantEffet = "avenant.effet",
    AvenantDate = "avenant.date",
}

impl std::fmt::Display for Marqueur {
    /// Tel que le modele l'ecrit : `{{employeur.siret}}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{{{}}}}}", self.nom())
    }
}

/// Ce que la fabrique de PDF pose elle-meme : le contrat remplace (bloc `ANNULE:`), chaque modification d'un avenant
/// (bloc `MODIFICATION:`).
pub const DU_REMPLACEMENT: [Marqueur; 2] = [Marqueur::RemplaceNumero, Marqueur::RemplaceDate];

/// Ce que l'avenant ajoute aux champs du contrat : les siens, et ceux de chaque modification.
pub const DE_L_AVENANT: [Marqueur; 10] = [
    Marqueur::AvenantNumero,
    Marqueur::AvenantDateContrat,
    Marqueur::AvenantReference,
    Marqueur::AvenantEffet,
    Marqueur::AvenantDate,
    Marqueur::ModificationNumero,
    Marqueur::ModificationArticle,
    Marqueur::ModificationAvant,
    Marqueur::ModificationApres,
    Marqueur::ModificationPhrase,
];

/// LES VALEURS D'UN DOCUMENT, par marqueur.
pub type Valeurs = BTreeMap<Marqueur, String>;

/// Les valeurs telles que la fabrique de PDF les lit, par leur nom.
pub fn pour_la_fabrique(v: &Valeurs) -> fabrique_pdf::modele::Valeurs {
    v.iter().map(|(k, x)| (k.nom().to_string(), x.clone())).collect()
}

/// Les valeurs telles que la description d'un document les ecrit, dans l'ordre des marqueurs.
pub fn dans_la_description(v: &Valeurs) -> Value {
    Value::Object(v.iter().map(|(k, x)| (k.nom().to_string(), Value::String(x.clone()))).collect::<Map<String, Value>>())
}

/// UN MORCEAU D'UN MODELE : du texte, ou un marqueur.
#[derive(Debug, Clone, PartialEq)]
pub enum Morceau {
    Texte(String),
    Marqueur(Marqueur),
}

/// UN MODELE LU : son texte, tel que la fabrique le met en page, et ses morceaux hors de ses commentaires (les lignes en
/// `#`, que la fabrique saute).
#[derive(Debug, Clone)]
pub struct ModeleLu {
    pub texte: String,
    pub morceaux: Vec<Morceau>,
}

impl ModeleLu {
    /// Le modele `texte` (`nom` : ce que le refus nomme), en morceaux ; un marqueur inconnu se refuse, et le refus les
    /// nomme tous.
    pub fn lire(texte: &str, nom: &str) -> Result<ModeleLu, String> {
        let mut morceaux = Vec::new();
        let mut inconnus: Vec<&str> = Vec::new();
        for l in texte.lines().filter(|l| !l.trim_start().starts_with('#')) {
            let (mut i, mut depuis) = (0, 0);
            while let Some(k) = l[i..].find("{{") {
                let a = i + k;
                match fabrique_pdf::modele::marqueur_en(l, a) {
                    Some((cle, fin)) => {
                        if a > depuis {
                            morceaux.push(Morceau::Texte(l[depuis..a].to_string()));
                        }
                        match Marqueur::lire(cle) {
                            Some(m) => morceaux.push(Morceau::Marqueur(m)),
                            None if !inconnus.contains(&cle) => inconnus.push(cle),
                            None => {}
                        }
                        (i, depuis) = (fin, fin);
                    }
                    None => i = a + 2,
                }
            }
            morceaux.push(Morceau::Texte(format!("{}\n", &l[depuis..])));
        }
        if !inconnus.is_empty() {
            return Err(format!(
                "Le modèle « {} » cite des champs que le logiciel ne connaît pas : {}. Les corriger d'après la liste des champs (Configuration › Modèles de contrat), ou les écrire en texte.",
                nom,
                inconnus.iter().map(|c| format!("{{{{{}}}}}", c)).collect::<Vec<_>>().join(", ")
            ));
        }
        Ok(ModeleLu { texte: texte.to_string(), morceaux })
    }

    /// Le fichier `chemin`, lu (UTF-8) puis `lire`.
    pub fn du_fichier(chemin: &Path) -> Result<ModeleLu, Refus> {
        let octets = std::fs::read(chemin).map_err(|e| {
            let classe = if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" };
            Refus { classe: Some(classe), message: socle::fichiers::panne_fichier(&e, chemin) }
        })?;
        let texte = String::from_utf8(octets).map_err(|e| Refus { classe: Some("UnicodeDecodeError"), message: format!("'utf-8' codec can't decode bytes: {}", e.utf8_error()) })?;
        let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        ModeleLu::lire(&texte, &nom).map_err(|m| Refus { classe: Some("ModeleIncomplet"), message: m })
    }

    /// Le modele cite-t-il `m` ?
    pub fn cite(&self, m: Marqueur) -> bool {
        self.morceaux.contains(&Morceau::Marqueur(m))
    }

    /// Son decoupage en titre, preambule, articles et signatures (`fabrique_pdf::modele::lire_modele`).
    pub fn decoupe(&self, nom: &str) -> Result<fabrique_pdf::modele::Modele, String> {
        fabrique_pdf::modele::lire_modele(&self.texte, nom).map_err(|r| r.0)
    }
}
