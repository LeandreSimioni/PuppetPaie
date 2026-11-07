//! UNE DSN DEPOSEE, LUE UNE FOIS, EN SA FORME (`DsnLue`) : son en-tete (l'envoi S10, la declaration S20.G00.05), ses
//! montants en decimal, ses dates en jours, ses blocs tels que la norme les emboite (l'individu S21.G00.30, ses contrats
//! S21.G00.40, leur arret S21.G00.60 et leur fin S21.G00.62, ses versements S21.G00.50, et dans chacun ses remunerations
//! S21.G00.51, leurs activites S21.G00.53, ses bases S21.G00.78). Une seule fonction la lit (`lire`) ; l'import, le depot
//! et son apercu, les organismes simules, le controle des NTT et l'AER d'une fictive la prennent de la. Les DSN d'un
//! dossier se listent par leur contenu (`fichiers`). Lecture seule, sans reseau.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use socle::date::Jour;
use socle::echec::Echec;
use socle::mois::Mois;
use socle::num::Decimal;
use socle::texte::{blanc, lignes_du_texte, texte_latin1};

// ------------------------------------------------------------------ la forme

/// Le compte qui paie : le BIC et l'IBAN « du compte sur lequel doit etre preleve le montant de l'ordre de reglement »
/// (S21.G00.20.003, .004 ; cahier technique 2026.1, p. 156, `bloc-S21.G00.20.txt`), la `banque` de la fiche Entreprise
/// que le moteur y ecrit (`dsn.rs`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Banque {
    pub bic: String,
    pub iban: String,
}

/// Un contact chez le declare (S20.G00.07 : « nom et prenom » en une rubrique, .001 ; telephone .002 ; courriel .003 ;
/// type .004 ; cahier technique 2026.1, p. 139, `bloc-S20.G00.07.txt` l. 63-84), tel que la fiche Entreprise le porte
/// (`declarant_dsn`). Son courriel se garde tel que la DSN le declare, capitales comprises : la casse de ce qui precede
/// l'arobase est a l'adresse (« The local-part of a mailbox MUST BE treated as case sensitive », RFC 5321, § 2.4 :
/// `ietf/ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt`, l. 869).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Contact {
    pub nom_prenom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub civilite: Option<String>,
}

/// CE QUE LA DSN DECLARE DE L'ENTREPRISE, sous les cles de sa fiche (`champs-base.json`) : les blocs 06 (l'entreprise),
/// 11 (l'etablissement) et 40, PAS l'en-tete S10.G00.01, qui est l'EMETTEUR de l'envoi -- « dans le cas d'un tiers
/// declarant ou d'un concentrateur, c'est l'identifiant SIREN de ce tiers qui doit figurer ici » (cahier technique
/// 2026.1, p. 130, `atelier/reference/norme-dsn/texte/bloc-S10.G00.01.txt`) : un cabinet qui depose y met son nom et son
/// adresse. Le Siren et l'APEN se lisent en S21.G00.06 (p. 141-142, `bloc-S21.G00.06.txt`) ; le NIC, l'adresse et l'APET
/// de l'etablissement, en S21.G00.11 (p. 144-145, `bloc-S21.G00.11.txt`) -- la fiche ne porte qu'un etablissement, celui
/// de son NIC, que le moteur declare en 06.002 et en 11.001 (`dsn.rs`) ; l'adresse du siege (06.004-006) et son NIC
/// (06.002) ne servent qu'a defaut. La DSN ne declare pas la raison sociale de l'entreprise : celle de l'emetteur
/// (S10.G00.01.003) n'est reprise que si l'emetteur EST l'entreprise (meme Siren en S10.G00.01.001 et en S21.G00.06.001)
/// -- une inference : la norme ne dit pas que l'entreprise qui depose elle-meme y met son nom, mais l'emetteur « ayant
/// elabore le present envoi » est alors elle. L'IDCC de la fiche est la convention principale de l'etablissement
/// (S21.G00.11.022, p. 147, `bloc-S21.G00.11.txt`), qui vient avant tout contrat dans le fichier ; l'IDCC d'un contrat
/// (S21.G00.40.017) n'en tient lieu qu'a defaut : il en differe pour la 1734 (`droit::minima::idcc_du_contrat`). Le code
/// convention DSN de la fiche, le moteur le tire de l'IDCC (`routes::fiches::deriver`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct EntrepriseDeclaree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub siren: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idcc: Option<String>,
    /// La DSN declare « 97 puis le numero d'adherent » (S21.G00.40.022, `conges_spectacles::code_dsn`) : la fiche ne
    /// garde que le numero d'adherent, et seulement s'il a la forme que dit la fiche d'Audiens. La cle est
    /// `documents::conges_spectacles::CLE_ADHERENT`.
    #[serde(rename = "congesspectacles_adherent", skip_serializing_if = "Option::is_none")]
    pub adherent_conges_spectacles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raison_sociale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banque: Option<Banque>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub france_travail_dossier_recouvrement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declarant_dsn: Option<Contact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medecine_travail: Option<String>,
    /// La licence du premier contrat (S21.G00.40.049) : la fiche la date (`LectureDsn::licences` du moteur).
    #[serde(skip)]
    pub licence: Option<String>,
}

/// Ce que l'emploi d'un contrat (bloc S21.G00.40) dit du salarie, sous les cles de sa fiche.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EmploiDeclare {
    /// S21.G00.40.002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut_conventionnel: Option<String>,
    /// S21.G00.40.003.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut_categoriel_rc: Option<String>,
    /// S21.G00.40.004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcs_ese: Option<String>,
    /// S21.G00.40.005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_emploi_ft: Option<String>,
    /// S21.G00.40.006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emploi: Option<String>,
}

impl EmploiDeclare {
    /// Une valeur que la NATURE du contrat fixe (`contrats-base.json`, `codes` : « 999SPT » en 40.005 pour tout permanent,
    /// CCH-18 du bloc 40) ne dit rien du salarie : elle ne passe pas dans sa fiche. `999SPT` n'est pas un metier, et une
    /// DSN de PuppetPaie reimportee le donnerait comme `code_emploi_ft` d'un permanent.
    pub fn du_salarie(&self) -> EmploiDeclare {
        let garder = |rubrique: &str, v: &Option<String>| {
            v.clone().filter(|v| !droit::natures::natures().map_or(false, |n| n.iter().any(|nat| nat.code(rubrique).as_deref() == Some(v.as_str()))))
        };
        EmploiDeclare {
            statut_conventionnel: garder("S21.G00.40.002", &self.statut_conventionnel),
            statut_categoriel_rc: garder("S21.G00.40.003", &self.statut_categoriel_rc),
            pcs_ese: garder("S21.G00.40.004", &self.pcs_ese),
            code_emploi_ft: garder("S21.G00.40.005", &self.code_emploi_ft),
            emploi: garder("S21.G00.40.006", &self.emploi),
        }
    }
}

/// UN INDIVIDU (bloc S21.G00.30), sous les cles de la fiche salarie. Le sexe (30.005, « 01 - masculin », « 02 -
/// féminin ») devient le « 1 » ou « 2 » de la fiche (`sexe_de_la_fiche`) : la norme ne le fait porter que s'il differe
/// de celui du NIR ou si le NIR manque (cahier technique 2026.1, p. 171, `atelier/reference/norme-dsn/texte/
/// bloc-S21.G00.30.txt`, l. 154-163). Le nom d'usage (30.003, meme bloc, l. 138-144) n'a pas de case dans la fiche
/// salarie (`champs-base.json`) : il n'est pas lu. La date de naissance n'est pas un jour : la norme admet un jour ou un
/// mois inconnu (« 00MMAAAA ou 99MMAAAA », « JJ00AAAA ou JJ99AAAA », p. 172, l. 175-177) ; elle passe a la fiche en
/// AAAA-MM-JJ, que la fiche controle.
#[derive(Clone, Debug, Default, Serialize)]
pub struct IndividuDeclare {
    /// S21.G00.30.001.
    #[serde(skip)]
    pub nir: String,
    /// .002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
    /// .004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom: Option<String>,
    /// .005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sexe: Option<String>,
    /// .006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<String>,
    /// .007.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu_naissance: Option<String>,
    /// .008.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    /// .009.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<String>,
    /// .010.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    /// .013.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_nationalite_dsn: Option<String>,
    /// .014.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departement_naissance: Option<String>,
    /// .015.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_naissance: Option<String>,
    /// .018.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// .019.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_salarie: Option<String>,
    /// .020, le numero technique temporaire d'un individu declare sans NIR (`ntt`) : la fiche n'en a pas.
    #[serde(skip)]
    pub ntt: Option<String>,
}

/// Un contrat (bloc S21.G00.40) de l'individu qui le precede, son arret (bloc S21.G00.60) et sa fin (bloc S21.G00.62)
/// s'il les declare.
#[derive(Clone, Debug, Default)]
pub struct ContratDeclare {
    pub nir: String,
    /// Le rang de son individu dans `DsnLue::individus` : un individu sans NIR n'a que lui.
    pub individu: usize,
    /// .001.
    pub debut: Option<Jour>,
    pub emploi: EmploiDeclare,
    /// .007, la nature du contrat.
    pub nature_dsn: Option<String>,
    /// .009.
    pub numero: Option<String>,
    /// .010.
    pub fin: Option<Jour>,
    /// .012, la quotite de reference de la categorie ; .013, celle du contrat ; .014, la modalite du temps de travail.
    pub quotite_reference: Option<Decimal>,
    pub quotite: Option<Decimal>,
    pub modalite: Option<String>,
    /// .017, la convention du contrat.
    pub idcc: Option<String>,
    /// .021, le motif de recours.
    pub motif_recours: Option<String>,
    /// .022, la caisse de conges payes, telle que la DSN l'ecrit.
    pub caisse_conges: Option<String>,
    /// .040 et .043 : le code risque et le taux AT/MP (`bloc-S21.G00.40.txt` l. 1224-1315).
    pub code_risque: Option<String>,
    pub taux_at: Option<Decimal>,
    /// .049, la licence.
    pub licence: Option<String>,
    /// .050, le numero d'objet spectacle.
    pub objet: Option<String>,
    /// S21.G00.60.002 : le dernier jour travaille avant l'arret que la DSN declare.
    pub dernier_jour_travaille: Option<Jour>,
    /// S21.G00.62.001 et .002 : la fin que la DSN declare, et son motif.
    pub fin_declaree: Option<Jour>,
    pub motif_fin: Option<String>,
}

/// Une activite (bloc S21.G00.53) de la remuneration qui la precede : son type (.001), sa mesure (.002), son unite (.003).
#[derive(Clone, Debug, Default)]
pub struct Activite {
    pub genre: Option<String>,
    pub mesure: Option<Decimal>,
    pub unite: Option<String>,
}

/// UNE REMUNERATION (bloc S21.G00.51), « contrepartie de l'activite de l'individu dans le cadre du contrat »
/// (`norme-dsn/texte/bloc-S21.G00.51.txt` l. 22) : sa periode de paie (.001, « date de debut de periode a laquelle la
/// remuneration est rattachee », l. 38-40), LE CONTRAT QU'ELLE PAIE (.010, « identique a la valeur renseignee dans une
/// rubrique Numéro du contrat - S21.G00.40.009 », CCH-11, l. 91-98), son type (.011), son montant (.013), ses activites.
#[derive(Clone, Debug, Default)]
pub struct Remuneration {
    pub debut: Option<Jour>,
    pub contrat: Option<String>,
    pub genre: Option<String>,
    pub montant: Option<Decimal>,
    pub activites: Vec<Activite>,
}

impl Remuneration {
    /// Celle d'une periode anterieure au mois principal declare (S20.G00.05.005) est le rappel d'une paie deja declaree
    /// (chapitre 2, §2.1.3.6, `norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 607-616), pas la paie du mois.
    pub fn rappel(&self, mois: Mois) -> bool {
        self.debut.is_some_and(|d| d < mois.premier_jour())
    }
}

/// UN VERSEMENT (bloc 50) et ce qui s'y rattache. Ses montants sont ceux du VERSEMENT, pas d'un contrat : le bloc 50 n'a
/// pas de numero de contrat, seules ses remunerations en portent un (S21.G00.51.010), et ses bases (bloc 78) ne le
/// portent que « dans le cas de la declaration de plusieurs blocs Contrat » (S21.G00.78.006, `bloc-S21.G00.79.txt`
/// l. 9-14). 50.004 est le montant net verse de la norme (la remuneration nette fiscale moins CSG/CRDS non deductible et
/// sante patronale, p. 244), pas le net a payer. Le taux du prelevement a la source, son type et son identifiant (50.006
/// a 50.008) se lisent avec lui.
#[derive(Clone, Debug, Default)]
pub struct VersementLu {
    pub nir: String,
    /// .001.
    pub verse_le: Option<Jour>,
    /// .002.
    pub net_imposable: Option<Decimal>,
    /// .004.
    pub net_verse: Option<Decimal>,
    /// .006 a .008.
    pub taux_pas: Option<Decimal>,
    pub type_pas: Option<String>,
    pub identifiant_pas: Option<String>,
    /// .009.
    pub pas: Option<Decimal>,
    pub remunerations: Vec<Remuneration>,
    /// LES BASES ASSUJETTIES QUE LA DSN DECLARE (bloc 78 ; `norme-dsn/texte/bloc-S21.G00.78.txt`), dans l'ordre.
    pub bases: Vec<BaseLue>,
}

/// Une base assujettie (bloc S21.G00.78) : son code (.001), son montant (.004), et le contrat qu'elle nomme (.006, « dans
/// le cas de la declaration de plusieurs blocs Contrat », `bloc-S21.G00.79.txt` l. 9-14 ; « dans le cas de la declaration
/// d'un contrat unique, cette rubrique n'est pas a renseigner »).
#[derive(Clone, Debug, Default)]
pub struct BaseLue {
    pub code: String,
    pub montant: Decimal,
    pub contrat: Option<String>,
}

impl VersementLu {
    /// Les contrats que ses remunerations paient (S21.G00.51.010), dans l'ordre ou ils paraissent.
    pub fn contrats(&self) -> Vec<&str> {
        let mut v: Vec<&str> = Vec::new();
        for n in self.remunerations.iter().filter_map(|r| r.contrat.as_deref()) {
            if !v.contains(&n) {
                v.push(n);
            }
        }
        v
    }
}

/// Ce que le bloc 50 dit du versement : ses nets (.002, .004) et son prelevement a la source (.006 a .009). Le bloc n'a
/// pas de numero de contrat (`bloc-S21.G00.50.txt`) : ces montants ne sont ceux d'un contrat que si le versement ne paie
/// que lui, et lui seul (`PaieDuContrat::nets`).
#[derive(Clone, Debug, Default)]
pub struct Nets {
    pub net_imposable: Option<Decimal>,
    pub net_verse: Option<Decimal>,
    pub taux_pas: Option<Decimal>,
    pub type_pas: Option<String>,
    pub identifiant_pas: Option<String>,
    pub pas: Option<Decimal>,
}

/// CE QUE LA DSN ATTRIBUE A UN CONTRAT POUR LE MOIS, de tout ce que ses versements portent : ce que la norme rattache au
/// contrat par son numero -- ses remunerations et leurs activites (S21.G00.51.010, CCH-11, `bloc-S21.G00.51.txt`
/// l. 91-98 : `mesures`), ses bases (S21.G00.78.006, `bloc-S21.G00.79.txt` l. 9-14) -- et, quand un seul versement le
/// paie et ne paie que lui, ce que ce versement porte sans le nommer : ses nets, son prelevement (`nets`) et ses bases sans
/// 78.006. Un versement de plusieurs contrats n'attribue a aucun ses nets ni ses bases sans numero : rien ne se repartit.
#[derive(Clone, Debug, Default)]
pub struct PaieDuContrat {
    pub nir: String,
    /// Le contrat (S21.G00.51.010).
    pub numero: String,
    /// Le jour du dernier versement qui le paie (S21.G00.50.001).
    pub verse_le: Option<Jour>,
    pub mesures: Mesures,
    /// Ses bases, par code (78.001), sommees.
    pub bases: BTreeMap<String, Decimal>,
    pub nets: Option<Nets>,
    /// Les autres contrats que ses versements paient (vide : ils ne paient que lui).
    pub avec: Vec<String>,
}

/// Une adhesion prevoyance (S21.G00.15 : la reference .001, l'organisme .002, la reference de l'adhesion .005).
#[derive(Clone, Debug, Default)]
pub struct Adhesion {
    pub libelle: String,
    pub organisme: Option<String>,
    pub reference: Option<String>,
}

/// Ce qu'un contrat declare pour des periodes de paie : son brut (51 de type 001, « remuneration brute non plafonnee » ;
/// `None` : aucun), son salaire habituel (51 de type 010, « la remuneration brute habituelle du salarie »,
/// `bloc-S21.G00.51.txt` : le salaire du contrat d'un permanent) et son activite (53 de type 01).
#[derive(Clone, Debug, Default)]
pub struct Mesures {
    pub brut: Option<Decimal>,
    pub salaire_habituel: Option<Decimal>,
    pub jours: Decimal,
    pub cachets: Decimal,
    pub heures: Decimal,
    pub remunere: bool,
}

/// CE QUE DECLARE UNE DSN DEPOSEE.
#[derive(Clone, Debug)]
pub struct DsnLue {
    /// Le logiciel qui l'a ecrite (S10.G00.00.001), la nature de l'envoi (.005 : `01` test, `02` reel).
    pub logiciel: Option<String>,
    pub nature_envoi: Option<String>,
    /// Le Siren de l'emetteur (S10.G00.01.001) : celui du cabinet quand un tiers depose (`EntrepriseDeclaree`).
    pub emetteur: Option<String>,
    /// La nature de la declaration (S20.G00.05.001 : `01` mensuelle, un signalement sinon ; `NATURES`).
    pub nature: Option<String>,
    /// Le mois principal declare (S20.G00.05.005) ; un signalement n'en declare pas.
    pub mois: Option<Mois>,
    /// Le type, la fraction et le numero d'ordre de la declaration (S20.G00.05.002, .003, .004), et l'identifiant de
    /// celle qu'elle remplace (.006).
    pub type_declaration: Option<String>,
    pub fraction: Option<String>,
    pub ordre: Option<String>,
    pub remplace: Option<String>,
    /// Le jour de constitution de l'envoi (S20.G00.05.007).
    pub depot: Option<Jour>,
    pub fichier: String,
    /// Les lignes du fichier.
    pub lignes: usize,
    /// Le montant de chaque versement a un organisme (S21.G00.20.005), dans l'ordre.
    pub versements_organismes: Vec<Decimal>,
    pub entreprise: EntrepriseDeclaree,
    pub individus: Vec<IndividuDeclare>,
    pub contrats: Vec<ContratDeclare>,
    pub versements: Vec<VersementLu>,
    /// Les etablissements declares (S21.G00.11.001) et le NIC du siege (S21.G00.06.002).
    pub etablissements: Vec<String>,
    pub nic_siege: Option<String>,
    pub adhesions: Vec<Adhesion>,
    /// Les references d'adhesion auxquelles un contrat se rattache (S21.G00.70.013).
    pub rattachements: Vec<String>,
    /// Chaque rubrique lue et la valeur qu'elle declare (`declare`).
    rubriques: BTreeSet<(String, String)>,
}

/// La nature d'un envoi reel (S10.G00.00.005 ; `01` : un test).
pub const NATURE_REEL: &str = "02";
/// La nature d'une DSN mensuelle (S20.G00.05.001).
pub const MENSUELLE: &str = "01";
/// Les natures de declaration (S20.G00.05.001), en clair.
pub const NATURES: [(&str, &str); 7] = [
    ("01", "DSN mensuelle"),
    ("04", "signalement arrêt de travail"),
    ("05", "signalement reprise suite à arrêt de travail"),
    ("07", "signalement fin de contrat de travail unique"),
    ("08", "signalement amorçage des données variables"),
    ("09", "DSN de substitution"),
    ("10", "signalement déclaration préalable à l'embauche"),
];

impl DsnLue {
    /// Un envoi reel (S10.G00.00.005), pas un test.
    pub fn reel(&self) -> bool {
        self.nature_envoi.as_deref() == Some(NATURE_REEL)
    }

    /// La nature de la declaration ; une DSN qui ne la dit pas est une mensuelle.
    pub fn nature_ou_mensuelle(&self) -> &str {
        self.nature.as_deref().unwrap_or(MENSUELLE)
    }

    pub fn est_signalement(&self) -> bool {
        crate::norme::SIGNALEMENTS.contains(&self.nature_ou_mensuelle())
    }

    pub fn nature_lisible(&self) -> String {
        let n = self.nature_ou_mensuelle();
        match NATURES.iter().find(|(c, _)| *c == n) {
            Some((_, l)) => l.to_string(),
            None => format!("nature {}", n),
        }
    }

    /// L'etablissement declare (le premier S21.G00.11.001).
    pub fn etablissement(&self) -> Option<&str> {
        self.etablissements.first().map(String::as_str)
    }

    /// Le jour de l'evenement d'un signalement : l'arret par son dernier jour travaille (S21.G00.60.002), la fin de
    /// contrat par sa date de fin (S21.G00.62.001), l'amorcage par le debut du contrat (S21.G00.40.001), un amorcage de
    /// changement de logiciel (nature 08, sans aucun de ces jours) par sa constitution.
    pub fn evenement(&self) -> Option<Jour> {
        let premier = |f: fn(&ContratDeclare) -> Option<Jour>| self.contrats.iter().find_map(f);
        premier(|c| c.dernier_jour_travaille)
            .or_else(|| premier(|c| c.fin_declaree))
            .or_else(|| self.contrats.first().and_then(|c| c.debut))
            .or_else(|| self.depot.filter(|_| self.nature.as_deref() == Some(crate::amorcage::NATURE)))
    }

    /// Le jour de l'evenement (`evenement`), `AAAA-MM-JJ`, ou `?`.
    pub fn evenement_lisible(&self) -> String {
        self.evenement().map_or_else(|| "?".to_string(), |j| j.iso())
    }

    /// LE MOIS QUE LE FICHIER DECLARE (S20.G00.05.005) ; un signalement se date par son evenement, et se range dans le
    /// mois de ce jour.
    pub fn mois_declare(&self) -> Result<Mois, String> {
        match (self.mois, self.evenement()) {
            (Some(m), _) => Ok(m),
            (None, Some(j)) if self.est_signalement() => Ok(Mois::du_jour(&j)),
            _ => Err("il ne porte pas de mois principal déclaré (S20.G00.05.005)".to_string()),
        }
    }

    /// Ce qu'on fait recopier pour prouver qu'on a regarde : le jour de l'evenement d'un signalement, le mois sinon.
    pub fn confirmation_attendue(&self) -> String {
        if self.est_signalement() {
            self.evenement_lisible()
        } else {
            self.mois_declare().map_or_else(|_| "?".to_string(), |m| m.to_string())
        }
    }

    /// Les versements dont le prelevement a la source est declare (S21.G00.50.007), et ceux preleves sur la grille (un
    /// type autre que `01`, le taux transmis par la DGFiP).
    pub fn pas_versements(&self) -> usize {
        self.versements.iter().filter(|v| v.type_pas.is_some()).count()
    }

    pub fn pas_bareme(&self) -> usize {
        self.versements.iter().filter(|v| v.type_pas.as_deref().is_some_and(|t| t != "01")).count()
    }

    /// Les contrats de l'individu `nir`.
    pub fn contrats_de<'a>(&'a self, nir: &'a str) -> impl Iterator<Item = &'a ContratDeclare> + 'a {
        self.contrats.iter().filter(move |c| c.nir == nir)
    }

    /// Les versements de l'individu `nir`.
    pub fn versements_de<'a>(&'a self, nir: &'a str) -> impl Iterator<Item = &'a VersementLu> + 'a {
        self.versements.iter().filter(move |v| v.nir == nir)
    }

    /// CE QUE LE CONTRAT `numero` DE L'INDIVIDU `nir` DECLARE, pour le mois (`rappel` faux) ou en rappel d'un mois deja
    /// declare : ses remunerations (S21.G00.51.010 le nomme) dans tous ses versements, le brut de type 001, le salaire
    /// habituel de type 010, et le travail remunere (S21.G00.53 de type 01 : en heures, unites 10 et 35 ; en jours, 12 ;
    /// en cachets, 37).
    pub fn mesures(&self, nir: &str, numero: &str, rappel: bool) -> Mesures {
        let mut m = Mesures::default();
        for r in self.versements_de(nir).flat_map(|v| v.remunerations.iter()) {
            if r.contrat.as_deref() != Some(numero) || self.mois.is_some_and(|m| r.rappel(m)) != rappel {
                continue;
            }
            if let Some(montant) = r.montant {
                m.remunere = true;
                match r.genre.as_deref() {
                    Some("001") => m.brut = Some(m.brut.unwrap_or_default() + montant),
                    Some("010") => m.salaire_habituel = Some(montant),
                    _ => {}
                }
            }
            for a in r.activites.iter().filter(|a| a.genre.as_deref() == Some("01")) {
                let mesure = a.mesure.unwrap_or_default();
                match a.unite.as_deref() {
                    Some("10" | "35") => m.heures += mesure,
                    Some("12") => m.jours += mesure,
                    Some("37") => m.cachets += mesure,
                    _ => {}
                }
            }
        }
        m
    }

    /// LES PAIES DU MOIS QUE LA DSN DECLARE, une par contrat qu'un versement paie (S21.G00.51.010), dans l'ordre ou
    /// elles paraissent, chacune avec ce que la DSN lui attribue (`PaieDuContrat`).
    pub fn paies(&self) -> Vec<PaieDuContrat> {
        let mut out: Vec<PaieDuContrat> = Vec::new();
        for v in &self.versements {
            for c in v.contrats() {
                if !out.iter().any(|p| p.nir == v.nir && p.numero == c) {
                    out.push(PaieDuContrat { nir: v.nir.clone(), numero: c.to_string(), ..PaieDuContrat::default() });
                }
            }
        }
        for p in &mut out {
            let payeurs: Vec<&VersementLu> = self.versements_de(&p.nir).filter(|v| v.contrats().contains(&p.numero.as_str())).collect();
            let seul = match payeurs.as_slice() {
                [v] if v.contrats().len() == 1 => Some(*v),
                _ => None,
            };
            p.verse_le = payeurs.last().and_then(|v| v.verse_le);
            p.mesures = self.mesures(&p.nir, &p.numero, false);
            for c in payeurs.iter().flat_map(|v| v.contrats()) {
                if c != p.numero && !p.avec.iter().any(|x| x == c) {
                    p.avec.push(c.to_string());
                }
            }
            for v in self.versements_de(&p.nir) {
                let sans_numero = seul.is_some_and(|s| std::ptr::eq(s, v));
                for b in v.bases.iter().filter(|b| b.contrat.as_deref().map_or(sans_numero, |c| c == p.numero)) {
                    *p.bases.entry(b.code.clone()).or_default() += b.montant;
                }
            }
            p.nets = seul.map(|v| Nets {
                net_imposable: v.net_imposable,
                net_verse: v.net_verse,
                taux_pas: v.taux_pas,
                type_pas: v.type_pas.clone(),
                identifiant_pas: v.identifiant_pas.clone(),
                pas: v.pas,
            });
        }
        out
    }

    /// LA DSN DECLARE-T-ELLE `valeur` EN `rubrique` ? La rubrique est celle que la table des champs donne a une case
    /// (`champs-base.json`, `rubrique`) ; la valeur, telle que la DSN l'ecrit.
    pub fn declare(&self, rubrique: &str, valeur: &str) -> bool {
        self.rubriques.contains(&(rubrique.to_string(), valeur.to_string()))
    }
}

impl EntrepriseDeclaree {
    pub fn est_vide(&self) -> bool {
        self.fiche().is_empty()
    }

    /// Ce qu'une DSN plus recente declare remplace ce que disait la precedente.
    pub fn suivre(&mut self, r: &EntrepriseDeclaree) {
        fn prendre<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
            if b.is_some() {
                *a = b.clone();
            }
        }
        prendre(&mut self.siren, &r.siren);
        prendre(&mut self.naf, &r.naf);
        prendre(&mut self.nic, &r.nic);
        prendre(&mut self.adresse, &r.adresse);
        prendre(&mut self.code_postal, &r.code_postal);
        prendre(&mut self.commune, &r.commune);
        prendre(&mut self.idcc, &r.idcc);
        prendre(&mut self.adherent_conges_spectacles, &r.adherent_conges_spectacles);
        prendre(&mut self.raison_sociale, &r.raison_sociale);
        prendre(&mut self.banque, &r.banque);
        prendre(&mut self.france_travail_dossier_recouvrement, &r.france_travail_dossier_recouvrement);
        prendre(&mut self.declarant_dsn, &r.declarant_dsn);
        prendre(&mut self.medecine_travail, &r.medecine_travail);
    }

    /// La fiche Entreprise que la DSN remplit (`champs-base.json`), telle que les fiches la lisent.
    pub fn fiche(&self) -> Map<String, Value> {
        en_fiche(self)
    }
}

/// Une forme ecrite en fiche : des textes, qui s'ecrivent toujours.
pub fn en_fiche<T: Serialize>(t: &T) -> Map<String, Value> {
    match serde_json::to_value(t) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

// ------------------------------------------------------------------ quel fichier est une DSN

/// UNE DSN SE RECONNAIT A SON CONTENU, PAS A SON NOM : elle commence par la rubrique S10.G00.00.001, apres les marques
/// d'ordre d'octets UTF-8 qu'un editeur aurait posees.
pub fn est_une_dsn(tete: &[u8]) -> bool {
    let mut t = tete;
    while t.starts_with(b"\xef\xbb\xbf") {
        t = &t[3..];
    }
    t.starts_with(b"S10.G00.00.001,")
}

/// LES DSN D'UN DOSSIER ET DE SES SOUS-DOSSIERS (sans suivre un lien vers un dossier), par leur contenu
/// (`est_une_dsn`), dans l'ordre des noms.
pub fn fichiers(dossier: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    parcourir(dossier, &mut out);
    out
}

fn parcourir(dossier: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    let mut entrees: Vec<std::fs::DirEntry> = entrees.flatten().collect();
    entrees.sort_by_key(|e| e.file_name());
    for e in entrees {
        let chemin = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => parcourir(&chemin, out),
            Ok(_) if chemin.is_file() && tete(&chemin).is_some_and(|t| est_une_dsn(&t)) => out.push(chemin),
            _ => {}
        }
    }
}

/// Les premiers octets d'un fichier, assez pour reconnaitre l'en-tete d'une DSN derriere des marques d'ordre d'octets.
fn tete(chemin: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut t = Vec::with_capacity(64);
    std::fs::File::open(chemin).ok()?.take(64).read_to_end(&mut t).ok()?;
    Some(t)
}

// ------------------------------------------------------------------ la lecture

/// `01` -> `1`, `02` -> `2` (les choix du champ `sexe` de la fiche) ; une autre valeur reste telle quelle, et la fiche
/// la signalera.
fn sexe_de_la_fiche(v: &str) -> String {
    match v.trim() {
        "01" => "1".into(),
        "02" => "2".into(),
        autre => autre.to_string(),
    }
}

/// `23071990` -> `1990-07-23` ; une date qui n'a pas ses huit chiffres est rendue telle quelle.
fn jjmmaaaa(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 && c.iter().all(char::is_ascii_digit) {
        let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
    } else {
        v.to_string()
    }
}

/// Une date de la DSN, JJMMAAAA, qui est un jour.
fn jour_dsn(v: &str) -> Option<Jour> {
    if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| v[a..b].parse::<u32>().ok();
    Jour::nouveau(n(4, 8)? as i32, n(2, 4)?, n(0, 2)?)
}

/// Ce qu'une DSN porte et qui ne se lit pas : une valeur qui n'est pas ce que sa rubrique dit, ou une ligne qui n'est
/// pas une rubrique de la norme.
enum Faute {
    Valeur { rubrique: String, valeur: String, quoi: &'static str },
    Rubrique { ligne: String, version: Option<String> },
}

/// La premiere faute de la lecture.
#[derive(Default)]
struct Fautes(Option<Faute>);

impl Fautes {
    fn noter(&mut self, f: Faute) {
        self.0.get_or_insert(f);
    }
    fn valeur<T>(&mut self, k: &str, v: &str, quoi: &'static str, lu: Option<T>) -> Option<T> {
        if lu.is_none() && !v.is_empty() {
            self.noter(Faute::Valeur { rubrique: k.to_string(), valeur: v.to_string(), quoi });
        }
        lu
    }
    fn montant(&mut self, k: &str, v: &str) -> Option<Decimal> {
        self.valeur(k, v, "un montant", socle::num::decimal_lu(v))
    }
    fn nombre(&mut self, k: &str, v: &str) -> Option<Decimal> {
        self.valeur(k, v, "un nombre", socle::num::decimal_lu(v))
    }
    fn jour(&mut self, k: &str, v: &str) -> Option<Jour> {
        self.valeur(k, v, "une date (JJMMAAAA)", jour_dsn(v))
    }
    /// Un refus dit quoi corriger et n'ecrit rien.
    fn refus(self, mois: Option<Mois>) -> Option<Echec> {
        let de = mois.map(|m| format!(" de {}", m.libelle(socle::mois::Graphie::Accentuee))).unwrap_or_default();
        let ce = match self.0? {
            Faute::Valeur { rubrique, valeur, quoi } => format!("porte, en {}, « {} », qui n'est pas {}", rubrique, valeur, quoi),
            Faute::Rubrique { ligne, version: Some(v) } => format!("porte la rubrique {}, que la norme DSN {} ne connaît pas", ligne, v),
            Faute::Rubrique { ligne, version: None } => format!("porte la ligne « {} », qui n'est pas une rubrique de la norme DSN", ligne),
        };
        Some(socle::echec::erreur(
            "ErreurOperation",
            format!("La DSN{} {} : corrigez-la ou retirez-la, puis recommencez.", de, ce),
        ))
    }
}

/// Le premier qui le dit.
fn premier(a: &mut Option<String>, v: &Option<String>) {
    if a.is_none() {
        *a = v.clone();
    }
}

/// La DSN du fichier `chemin` (`lire`) ; `Ok(None)` si le fichier ne se lit pas ou n'est pas une DSN (`est_une_dsn`).
pub fn lire_une_dsn(chemin: &Path) -> Result<Option<DsnLue>, Echec> {
    let Some(octets) = std::fs::read(chemin).ok().filter(|o| est_une_dsn(o)) else { return Ok(None) };
    lire(&octets, &chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).map(Some)
}

/// UNE DSN, LUE (`fichier` : son nom), un signalement sans mois principal compris. Une valeur qui n'est pas ce que sa
/// rubrique dit (un montant, une date), ou une ligne qui n'est pas une
/// rubrique, refuse : rien ne se devine (regle 1). La rubrique se controle au tableau des usages de la version que le
/// fichier declare (S10.G00.00.006, `usages-dsn-base.json`) ; une version que le logiciel ne livre pas (une DSN d'avant
/// 2026) ne se controle que par la forme de ses rubriques (`usages::est_rubrique`).
pub fn lire(octets: &[u8], fichier: &str) -> Result<DsnLue, Echec> {
    let contenu = texte_latin1(octets);
    // L'indicateur d'ordre des octets d'UTF-8, lu en latin-1, n'est pas une rubrique.
    let lignes = lignes_du_texte(contenu.trim_start_matches("\u{ef}\u{bb}\u{bf}"));
    let valeur = |v: &str| v.trim_matches(blanc).trim_matches('\'').to_string();
    let norme = lignes
        .iter()
        .find_map(|l| l.split_once(',').filter(|(k, _)| *k == "S10.G00.00.006").map(|(_, v)| valeur(v)))
        .and_then(|v| crate::norme::Norme::connue(&v).ok());
    let mut fautes = Fautes::default();
    let (mut mois, mut depot): (Option<Mois>, Option<Jour>) = (None, None);
    let (mut type_declaration, mut fraction, mut ordre, mut remplace): (Option<String>, Option<String>, Option<String>, Option<String>) = (None, None, None, None);
    let (mut logiciel, mut nature_envoi, mut nature): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    let mut versements_organismes: Vec<Decimal> = Vec::new();
    let mut e = EntrepriseDeclaree::default();
    let mut individus: Vec<IndividuDeclare> = Vec::new();
    let mut contrats: Vec<ContratDeclare> = Vec::new();
    let mut versements: Vec<VersementLu> = Vec::new();
    // Une base (78.001) est ouverte dans le versement en cours.
    let mut base = false;
    // Le contrat dont la DSN declare la fin (S21.G00.62.001) : le dernier contrat lu.
    let mut fin_de: Option<usize> = None;
    // L'emetteur, son contact, le siege et l'APET : ce qui ne sert qu'a defaut, ou sous condition (voir
    // `EntrepriseDeclaree`).
    let (mut emetteur_siren, mut emetteur_nom, mut civilite_emetteur, mut nom_emetteur) = (None, None, None, None);
    let (mut nic_siege, mut adresse_siege, mut code_postal_siege, mut commune_siege, mut apet) = (None, None, None, None, None);
    let mut etablissements: Vec<String> = Vec::new();
    // Les versements aux organismes (bloc 20) : identifiant, entite d'affectation, BIC, IBAN.
    let mut blocs20: Vec<[String; 4]> = Vec::new();
    let mut contacts: Vec<Contact> = Vec::new();
    let mut adhesions: Vec<Adhesion> = Vec::new();
    let mut rattachements: Vec<String> = Vec::new();
    let mut rubriques: BTreeSet<(String, String)> = BTreeSet::new();
    let mut spst = String::new();
    for ligne in &lignes {
        if ligne.trim_matches(blanc).is_empty() {
            continue;
        }
        let Some((k, v)) = ligne.split_once(',').filter(|(k, _)| crate::usages::est_rubrique(k)) else {
            fautes.noter(Faute::Rubrique { ligne: ligne.to_string(), version: None });
            continue;
        };
        if let Some(n) = &norme {
            if !crate::usages::rubrique_connue(n, k).map_err(Echec::from)? {
                fautes.noter(Faute::Rubrique { ligne: k.to_string(), version: Some(n.version.clone()) });
                continue;
            }
        }
        let v = valeur(v);
        rubriques.insert((k.to_string(), v.clone()));
        let plein = Some(v.clone()).filter(|x| !x.is_empty());
        // UN INDIVIDU SANS NIR (S21.G00.30.001 absent, « En l'absence de NIR ou de NIA », `bloc-S21.G00.30.txt`
        // l. 338-354) commence a son nom (.002).
        if k == "S21.G00.30.002" && individus.last().map_or(true, |i| i.nom.is_some()) {
            individus.push(IndividuDeclare::default());
        }
        let individu = individus.len().checked_sub(1);
        let versement = versements.last_mut();
        let contrat = contrats.last_mut();
        match k {
            "S10.G00.00.001" => premier(&mut logiciel, &plein),
            "S10.G00.00.005" => premier(&mut nature_envoi, &plein),
            "S20.G00.05.001" => premier(&mut nature, &plein),
            "S20.G00.05.003" => premier(&mut fraction, &plein),
            "S20.G00.05.006" => premier(&mut remplace, &plein),
            "S21.G00.20.005" => versements_organismes.extend(fautes.montant(k, &v)),
            "S21.G00.30.020" => {
                if let Some(i) = individu {
                    individus[i].ntt = plein;
                }
            }
            "S21.G00.40.012" => {
                let q = fautes.nombre(k, &v);
                contrat.into_iter().for_each(|c| c.quotite_reference = q);
            }
            "S21.G00.40.014" => contrat.into_iter().for_each(|c| c.modalite = plein.clone()),
            "S21.G00.40.021" => contrat.into_iter().for_each(|c| c.motif_recours = plein.clone()),
            "S21.G00.60.002" => {
                let jour = fautes.jour(k, &v);
                contrat.into_iter().for_each(|c| c.dernier_jour_travaille = jour);
            }
            "S10.G00.01.001" => premier(&mut emetteur_siren, &plein),
            "S10.G00.01.003" => premier(&mut emetteur_nom, &plein),
            "S10.G00.02.001" => civilite_emetteur = plein,
            "S10.G00.02.002" => nom_emetteur = plein,
            "S20.G00.05.002" => type_declaration = plein,
            "S20.G00.05.004" => ordre = plein,
            "S20.G00.05.005" => mois = fautes.valeur(k, &v, "un mois (01MMAAAA)", Mois::de_la_dsn(&v).ok()),
            "S20.G00.05.007" => depot = fautes.jour(k, &v),
            "S20.G00.07.001" => contacts.push(Contact { nom_prenom: v, ..Contact::default() }),
            "S20.G00.07.002" | "S20.G00.07.003" | "S20.G00.07.004" => {
                if let Some(c) = contacts.last_mut() {
                    *match k { "S20.G00.07.002" => &mut c.tel, "S20.G00.07.003" => &mut c.email, _ => &mut c.qualite } = plein;
                }
            }
            "S21.G00.06.001" => premier(&mut e.siren, &plein),
            "S21.G00.06.002" => premier(&mut nic_siege, &plein),
            "S21.G00.06.003" => premier(&mut e.naf, &plein),
            "S21.G00.06.004" => premier(&mut adresse_siege, &plein),
            "S21.G00.06.005" => premier(&mut code_postal_siege, &plein),
            "S21.G00.06.006" => premier(&mut commune_siege, &plein),
            "S21.G00.11.001" => {
                premier(&mut e.nic, &plein);
                if !etablissements.contains(&v) {
                    etablissements.push(v);
                }
            }
            "S21.G00.11.002" => premier(&mut apet, &plein),
            "S21.G00.11.003" => premier(&mut e.adresse, &plein),
            "S21.G00.11.004" => premier(&mut e.code_postal, &plein),
            "S21.G00.11.005" => premier(&mut e.commune, &plein),
            "S21.G00.11.022" => premier(&mut e.idcc, &plein),
            "S21.G00.11.025" => spst = v,
            "S21.G00.15.001" => adhesions.push(Adhesion { libelle: v, ..Adhesion::default() }),
            "S21.G00.15.002" | "S21.G00.15.005" => {
                if let Some(a) = adhesions.last_mut() {
                    *if k == "S21.G00.15.002" { &mut a.organisme } else { &mut a.reference } = plein;
                }
            }
            "S21.G00.20.001" => blocs20.push([v, String::new(), String::new(), String::new()]),
            "S21.G00.20.002" | "S21.G00.20.003" | "S21.G00.20.004" => {
                if let Some(b) = blocs20.last_mut() {
                    b[match k { "S21.G00.20.002" => 1, "S21.G00.20.003" => 2, _ => 3 }] = v;
                }
            }
            "S21.G00.30.001" => individus.push(IndividuDeclare { nir: v, ..IndividuDeclare::default() }),
            "S21.G00.30.002" | "S21.G00.30.004" | "S21.G00.30.005" | "S21.G00.30.006" | "S21.G00.30.007" | "S21.G00.30.008"
            | "S21.G00.30.009" | "S21.G00.30.010" | "S21.G00.30.013" | "S21.G00.30.014" | "S21.G00.30.015" | "S21.G00.30.018"
            | "S21.G00.30.019" => {
                let Some(i) = individu else { continue };
                let s = &mut individus[i];
                let case = match k {
                    "S21.G00.30.002" => &mut s.nom,
                    "S21.G00.30.004" => &mut s.prenom,
                    "S21.G00.30.005" => &mut s.sexe,
                    "S21.G00.30.006" => &mut s.date_naissance,
                    "S21.G00.30.007" => &mut s.lieu_naissance,
                    "S21.G00.30.008" => &mut s.adresse,
                    "S21.G00.30.009" => &mut s.code_postal,
                    "S21.G00.30.010" => &mut s.commune,
                    "S21.G00.30.013" => &mut s.code_nationalite_dsn,
                    "S21.G00.30.014" => &mut s.departement_naissance,
                    "S21.G00.30.015" => &mut s.pays_naissance,
                    "S21.G00.30.018" => &mut s.email,
                    _ => &mut s.code_salarie,
                };
                *case = Some(match k {
                    "S21.G00.30.005" => sexe_de_la_fiche(&v),
                    "S21.G00.30.006" => jjmmaaaa(&v),
                    _ => v,
                });
            }
            "S21.G00.40.001" => {
                let Some(i) = individu else { continue };
                let debut = fautes.jour(k, &v);
                contrats.push(ContratDeclare { nir: individus[i].nir.clone(), individu: i, debut, ..ContratDeclare::default() });
                fin_de = Some(contrats.len() - 1);
            }
            "S21.G00.40.002" | "S21.G00.40.003" | "S21.G00.40.004" | "S21.G00.40.005" | "S21.G00.40.006" => {
                let Some(c) = contrat else { continue };
                *match k {
                    "S21.G00.40.002" => &mut c.emploi.statut_conventionnel,
                    "S21.G00.40.003" => &mut c.emploi.statut_categoriel_rc,
                    "S21.G00.40.004" => &mut c.emploi.pcs_ese,
                    "S21.G00.40.005" => &mut c.emploi.code_emploi_ft,
                    _ => &mut c.emploi.emploi,
                } = plein;
            }
            "S21.G00.40.007" => contrat.into_iter().for_each(|c| c.nature_dsn = plein.clone()),
            "S21.G00.40.009" => contrat.into_iter().for_each(|c| c.numero = plein.clone()),
            "S21.G00.40.010" => {
                let fin = fautes.jour(k, &v);
                contrat.into_iter().for_each(|c| c.fin = fin);
            }
            "S21.G00.40.013" => {
                let q = fautes.nombre(k, &v);
                contrat.into_iter().for_each(|c| c.quotite = q);
            }
            "S21.G00.40.017" => {
                premier(&mut e.idcc, &plein);
                contrat.into_iter().for_each(|c| c.idcc = plein.clone());
            }
            "S21.G00.40.022" => {
                if let Some(n) = v.strip_prefix("97").filter(|n| documents::conges_spectacles::faute_adherent(n).is_none()) {
                    premier(&mut e.adherent_conges_spectacles, &Some(n.to_string()));
                }
                contrat.into_iter().for_each(|c| c.caisse_conges = plein.clone());
            }
            "S21.G00.40.040" => contrat.into_iter().for_each(|c| c.code_risque = plein.clone()),
            "S21.G00.40.043" => {
                let t = fautes.nombre(k, &v);
                contrat.into_iter().for_each(|c| c.taux_at = t);
            }
            "S21.G00.40.049" => {
                premier(&mut e.licence, &plein);
                contrat.into_iter().for_each(|c| c.licence = plein.clone());
            }
            "S21.G00.40.050" => contrat.into_iter().for_each(|c| c.objet = plein.clone()),
            "S21.G00.62.001" => {
                let fin = fautes.jour(k, &v);
                if let Some(c) = fin_de.and_then(|i| contrats.get_mut(i)) {
                    c.fin_declaree = fin;
                }
            }
            "S21.G00.62.002" => {
                if let Some(c) = fin_de.and_then(|i| contrats.get_mut(i)) {
                    c.motif_fin = plein;
                }
            }
            "S21.G00.50.001" => {
                let Some(i) = individu else { continue };
                let verse_le = fautes.jour(k, &v);
                versements.push(VersementLu { nir: individus[i].nir.clone(), verse_le, ..VersementLu::default() });
                base = false;
            }
            "S21.G00.50.002" => {
                let m = fautes.montant(k, &v);
                versement.into_iter().for_each(|x| x.net_imposable = m);
            }
            "S21.G00.50.004" => {
                let m = fautes.montant(k, &v);
                versement.into_iter().for_each(|x| x.net_verse = m);
            }
            "S21.G00.50.006" => {
                let t = fautes.nombre(k, &v);
                versement.into_iter().for_each(|x| x.taux_pas = t);
            }
            "S21.G00.50.007" => versement.into_iter().for_each(|x| x.type_pas = plein.clone()),
            "S21.G00.50.008" => versement.into_iter().for_each(|x| x.identifiant_pas = plein.clone()),
            "S21.G00.50.009" => {
                let m = fautes.montant(k, &v);
                versement.into_iter().for_each(|x| x.pas = m);
            }
            "S21.G00.51.001" => {
                let Some(x) = versement else { continue };
                let debut = fautes.jour(k, &v);
                x.remunerations.push(Remuneration { debut, ..Remuneration::default() });
            }
            "S21.G00.51.010" | "S21.G00.51.011" | "S21.G00.51.013" => {
                let Some(r) = versement.and_then(|x| x.remunerations.last_mut()) else { continue };
                match k {
                    "S21.G00.51.010" => r.contrat = plein,
                    "S21.G00.51.011" => r.genre = plein,
                    _ => r.montant = fautes.montant(k, &v),
                }
            }
            "S21.G00.53.001" => {
                let Some(r) = versement.and_then(|x| x.remunerations.last_mut()) else { continue };
                r.activites.push(Activite { genre: plein, ..Activite::default() });
            }
            "S21.G00.53.002" | "S21.G00.53.003" => {
                let mesure = if k == "S21.G00.53.002" { fautes.nombre(k, &v) } else { None };
                let Some(a) = versement.and_then(|x| x.remunerations.last_mut()).and_then(|r| r.activites.last_mut()) else { continue };
                match k {
                    "S21.G00.53.002" => a.mesure = mesure,
                    _ => a.unite = plein,
                }
            }
            "S21.G00.70.013" => rattachements.push(v),
            "S21.G00.78.001" => {
                let Some(x) = versement else { continue };
                base = plein.is_some();
                if let Some(code) = plein {
                    x.bases.push(BaseLue { code, ..BaseLue::default() });
                }
            }
            "S21.G00.78.004" | "S21.G00.78.006" => {
                let m = if k == "S21.G00.78.004" { fautes.montant(k, &v) } else { None };
                let Some(b) = versement.filter(|_| base).and_then(|x| x.bases.last_mut()) else { continue };
                match k {
                    "S21.G00.78.004" => b.montant += m.unwrap_or_default(),
                    _ => b.contrat = plein,
                }
            }
            _ => {}
        }
    }
    if let Some(refus) = fautes.refus(mois) {
        return Err(refus);
    }
    let emetteur_est_l_entreprise = emetteur_siren.is_some() && emetteur_siren == e.siren;
    if emetteur_est_l_entreprise && emetteur_nom.is_some() {
        e.raison_sociale = emetteur_nom;
    }
    premier(&mut e.nic, &nic_siege);
    premier(&mut e.naf, &apet);
    if e.adresse.is_none() && e.code_postal.is_none() {
        (e.adresse, e.code_postal, e.commune) = (adresse_siege, code_postal_siege, commune_siege.or(e.commune));
    }
    // LE COMPTE QUI PAIE (`Banque`), quand tous les versements de la DSN disent le meme compte ; deux comptes, rien (on ne
    // choisit pas). Le dossier de recouvrement France Travail : l'entite d'affectation (20.002) du versement a France
    // Travail, « identifiant attribue par le centre de recouvrement Spectacle (CS suivi de 8 caracteres numeriques) »
    // (p. 155), que le moteur ecrit depuis `france_travail_dossier_recouvrement`.
    let comptes: BTreeSet<(&str, &str)> =
        blocs20.iter().filter(|b| !b[2].is_empty() && !b[3].is_empty()).map(|b| (b[2].as_str(), b[3].as_str())).collect();
    if let (1, Some((bic, iban))) = (comptes.len(), comptes.iter().next()) {
        e.banque = Some(Banque { bic: bic.to_string(), iban: iban.to_string() });
    }
    e.france_travail_dossier_recouvrement =
        blocs20.iter().find(|b| b[0] == crate::dsn::SIRET_FRANCE_TRAVAIL && !b[1].is_empty()).map(|b| b[1].clone());
    // QUI LES ORGANISMES CONTACTENT : le contact chez le declare de type 04 (« recouvrant, entre autres, des cotisations de
    // Securite sociale ») s'il y en a plusieurs, sinon le premier : le `declarant_dsn` de la fiche, que le moteur ecrit la
    // (`dsn.rs`, `type_du_contact`). Sa civilite n'est pas dans ce bloc : elle se lit dans celle du contact de l'emetteur
    // (S10.G00.02.001, `bloc-S10.G00.02.txt` l. 33-40) quand l'emetteur est l'entreprise (meme Siren, comme pour la raison
    // sociale) et que son contact porte le meme nom et prenom (S10.G00.02.002) -- une inference : c'est alors la meme
    // personne.
    let contact = contacts.iter().find(|c| c.qualite.as_deref() == Some(crate::dsn::TYPE_CONTACT_ORDINAIRE)).or(contacts.first());
    if let Some(c) = contact.filter(|c| !c.nom_prenom.trim().is_empty()) {
        let mut d = c.clone();
        if emetteur_est_l_entreprise && nom_emetteur.as_deref() == Some(c.nom_prenom.as_str()) {
            if let Some(civ) = civilite_emetteur.filter(|x| x == "01" || x == "02") {
                d.civilite = Some(civ);
            }
        }
        e.declarant_dsn = Some(d);
    }
    // LA MEDECINE DU TRAVAIL INTERNE : « MT01 - MEDECINE INTERNE ETABLISSEMENT » en S21.G00.11.025 (norme 2027,
    // `dsn::spst_ecrit`) : la fiche garde le code de la DPAE, ou la medecine interne s'ecrit MT999
    // (`CodeMedecine::INTERNE`). L'identifiant d'un service externe ne se reprend pas : sa definition est dans une fiche
    // consigne non deposee, et qu'il soit le code de la DPAE serait une inference.
    if spst == "MT01" {
        e.medecine_travail = Some(droit::fiche::CodeMedecine::INTERNE.to_string());
    }
    Ok(DsnLue {
        logiciel,
        nature_envoi,
        emetteur: emetteur_siren,
        nature,
        mois,
        type_declaration,
        fraction,
        ordre,
        remplace,
        depot,
        fichier: fichier.to_string(),
        lignes: lignes.len(),
        versements_organismes,
        entreprise: e,
        individus,
        contrats,
        versements,
        etablissements,
        nic_siege,
        adhesions,
        rattachements,
        rubriques,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce qu'un contrat declare pour le mois et ce qu'il rappelle d'un mois deja declare : un bloc 51 d'une periode
    /// anterieure au mois principal est un rappel (`bloc-S21.G00.51.txt` l. 38-40 ; §2.1.3.6) ; un montant qui n'en est
    /// pas un refuse.
    #[test]
    fn un_contrat_dit_son_mois_et_ses_rappels() {
        let f = "S20.G00.05.005,'01082026'\nS21.G00.30.001,'1850'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.62.001,'31082026'\nS21.G00.50.001,'31082026'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.50'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'49.50'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\n\
                 S21.G00.51.001,'01072026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'30.00'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'1'\nS21.G00.53.003,'37'\n";
        let d = lire(f.as_bytes(), "essai.dsn").unwrap();
        let (mois, rappel) = (d.mesures("1850", "FA1.26.13", false), d.mesures("1850", "FA1.26.13", true));
        assert_eq!((socle::num::q2(mois.brut.unwrap()), mois.cachets.normalize().to_string(), mois.remunere), ("200.00".to_string(), "2".to_string(), true));
        assert_eq!((socle::num::q2(rappel.brut.unwrap()), rappel.cachets.normalize().to_string()), ("30.00".to_string(), "1".to_string()));
        assert_eq!(d.contrats[0].fin_declaree.map(|j| j.iso()).as_deref(), Some("2026-08-31"));
        let faux = "S20.G00.05.005,'01082026'\nS21.G00.30.001,'1'\nS21.G00.50.001,'31082026'\nS21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.013,'12,5'\n";
        assert!(lire(faux.as_bytes(), "essai.dsn").unwrap_err().message.contains("« 12,5 », qui n'est pas un montant"));
    }

    /// UN VERSEMENT DE DEUX CONTRATS attribue a chacun ce que la norme rattache a son numero : ses remunerations
    /// (S21.G00.51.010, `bloc-S21.G00.51.txt` l. 91-98) et ses bases (S21.G00.78.006, `bloc-S21.G00.79.txt` l. 9-14) ;
    /// ses nets (bloc 50) et une base sans numero ne sont a aucun.
    #[test]
    fn un_versement_de_deux_contrats_attribue_a_chacun_ce_qui_le_nomme() {
        let f = "S20.G00.05.005,'01122025'\nS21.G00.30.001,'1800'\nS21.G00.40.001,'05122025'\nS21.G00.40.009,'MO4.25.16'\n\
                 S21.G00.40.001,'19122025'\nS21.G00.40.009,'MO4.25.17'\nS21.G00.50.001,'31122025'\nS21.G00.50.002,'700.00'\n\
                 S21.G00.51.001,'05122025'\nS21.G00.51.010,'MO4.25.16'\nS21.G00.51.011,'001'\nS21.G00.51.013,'525.00'\n\
                 S21.G00.51.001,'19122025'\nS21.G00.51.010,'MO4.25.17'\nS21.G00.51.011,'001'\nS21.G00.51.013,'350.00'\n\
                 S21.G00.78.001,'02'\nS21.G00.78.004,'525.00'\nS21.G00.78.006,'MO4.25.16'\n\
                 S21.G00.78.001,'02'\nS21.G00.78.004,'350.00'\nS21.G00.78.006,'MO4.25.17'\n\
                 S21.G00.78.001,'31'\nS21.G00.78.004,'875.00'\n";
        let p = lire(f.as_bytes(), "essai.dsn").unwrap().paies();
        let dit = |i: usize| (p[i].numero.as_str(), p[i].mesures.brut.map(socle::num::q2), p[i].bases.get("02").copied().map(socle::num::q2), p[i].bases.len(), p[i].nets.is_some(), p[i].avec.clone());
        assert_eq!(dit(0), ("MO4.25.16", Some("525.00".into()), Some("525.00".into()), 1, false, vec!["MO4.25.17".to_string()]));
        assert_eq!(dit(1), ("MO4.25.17", Some("350.00".into()), Some("350.00".into()), 1, false, vec!["MO4.25.16".to_string()]));
    }
}
