//! Lire ce que CoolPaie a depose (DSN et AEM), pour ne pas le ressaisir.
//!
//! Ce module LIT et rend des donnees: il n'ecrit rien. Les DSN se lisent ligne a
//! ligne en gardant l'individu courant ; les AEM par positions fixes (1000 caracteres, latin-1).
//! Une DSN se lit une fois, par `lire_une_dsn`, en sa forme (`DsnLue`) : ses montants en decimal, ses dates en jours ;
//! la fiche qu'elle remplit n'est ecrite qu'a la fin (`EntrepriseDeclaree`, `SalarieDeclare::fiche`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use socle::date::Jour;
use socle::echec::Echec;
use socle::mois::Mois;
use socle::num::Decimal;
use socle::texte::{blanc, lignes_du_texte, texte_latin1};

// ------------------------------------------------------------------ une DSN deposee

/// Le compte qui paie : le BIC et l'IBAN « du compte sur lequel doit etre preleve le montant de l'ordre de reglement »
/// (S21.G00.20.003, .004 ; cahier technique 2026.1, p. 156, `bloc-S21.G00.20.txt`), la `banque` de la fiche Entreprise
/// que le moteur y ecrit (`dsn.rs`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Banque {
    pub(crate) bic: String,
    pub(crate) iban: String,
}

/// Un contact chez le declare (S20.G00.07 : « nom et prenom » en une rubrique, .001 ; telephone .002 ; courriel .003 ;
/// type .004 ; cahier technique 2026.1, p. 139, `bloc-S20.G00.07.txt` l. 63-84), tel que la fiche Entreprise le porte
/// (`declarant_dsn`). Son courriel se garde tel que la DSN le declare, capitales comprises : la casse de ce qui precede
/// l'arobase est a l'adresse (« The local-part of a mailbox MUST BE treated as case sensitive », RFC 5321, § 2.4 :
/// `ietf/ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt`, l. 869).
#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Contact {
    pub(crate) nom_prenom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) qualite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) civilite: Option<String>,
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
pub(crate) struct EntrepriseDeclaree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) siren: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) naf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) nic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) idcc: Option<String>,
    /// La DSN declare « 97 puis le numero d'adherent » (S21.G00.40.022, `conges_spectacles::code_dsn`) : la fiche ne
    /// garde que le numero d'adherent, et seulement s'il a la forme que dit la fiche d'Audiens. La cle est
    /// `documents::conges_spectacles::CLE_ADHERENT`.
    #[serde(rename = "congesspectacles_adherent", skip_serializing_if = "Option::is_none")]
    pub(crate) adherent_conges_spectacles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) raison_sociale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) banque: Option<Banque>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) france_travail_dossier_recouvrement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declarant_dsn: Option<Contact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) medecine_travail: Option<String>,
    /// La licence du premier contrat (S21.G00.40.049) : la fiche la date (`LectureDsn::licences`).
    #[serde(skip)]
    pub(crate) licence: Option<String>,
}

/// Ce que l'emploi d'un contrat (bloc S21.G00.40) dit du salarie, sous les cles de sa fiche.
#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct EmploiDeclare {
    /// S21.G00.40.002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) statut_conventionnel: Option<String>,
    /// S21.G00.40.003.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) statut_categoriel_rc: Option<String>,
    /// S21.G00.40.004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pcs_ese: Option<String>,
    /// S21.G00.40.005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_emploi_ft: Option<String>,
    /// S21.G00.40.006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) emploi: Option<String>,
}

impl EmploiDeclare {
    /// Une valeur que la NATURE du contrat fixe (`contrats-base.json`, `codes` : « 999SPT » en 40.005 pour tout permanent,
    /// CCH-18 du bloc 40) ne dit rien du salarie : elle ne passe pas dans sa fiche. `999SPT` n'est pas un metier, et une
    /// DSN de PuppetPaie reimportee le donnerait comme `code_emploi_ft` d'un permanent.
    fn du_salarie(&self) -> EmploiDeclare {
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
pub(crate) struct IndividuDeclare {
    /// S21.G00.30.001.
    #[serde(skip)]
    pub(crate) nir: String,
    /// .002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) nom: Option<String>,
    /// .004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prenom: Option<String>,
    /// .005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sexe: Option<String>,
    /// .006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) date_naissance: Option<String>,
    /// .007.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lieu_naissance: Option<String>,
    /// .008.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) adresse: Option<String>,
    /// .009.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_postal: Option<String>,
    /// .010.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) commune: Option<String>,
    /// .013.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_nationalite_dsn: Option<String>,
    /// .014.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) departement_naissance: Option<String>,
    /// .015.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pays_naissance: Option<String>,
    /// .018.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
    /// .019.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_salarie: Option<String>,
}

/// Un contrat (bloc S21.G00.40) de l'individu qui le precede.
#[derive(Clone, Debug, Default)]
pub(crate) struct ContratDeclare {
    pub(crate) nir: String,
    /// .001.
    pub(crate) debut: Option<Jour>,
    pub(crate) emploi: EmploiDeclare,
    /// .007, la nature du contrat.
    pub(crate) nature_dsn: Option<String>,
    /// .009.
    pub(crate) numero: Option<String>,
    /// .010.
    pub(crate) fin: Option<Jour>,
    /// .013, la quotite.
    pub(crate) quotite: Option<Decimal>,
    /// .040 et .043 : le code risque et le taux AT/MP (`bloc-S21.G00.40.txt` l. 1224-1315).
    pub(crate) code_risque: Option<String>,
    pub(crate) taux_at: Option<Decimal>,
    /// .050, le numero d'objet spectacle.
    pub(crate) objet: Option<String>,
}

/// UN VERSEMENT (bloc 50) et ce qui s'y rattache. 50.004 est le montant net verse de la norme (la remuneration nette
/// fiscale moins CSG/CRDS non deductible et sante patronale, p. 244), pas le net a payer : CoolPaie y ecrivait son net a
/// payer, PuppetPaie y ecrit le net verse. La DSN ne declare pas le net a payer ; une paie importee d'une DSN seule n'en a
/// pas, un bulletin le donne (`import::bulletins::completer`). Le taux du prelevement a la source, son type et son
/// identifiant (50.006 a 50.008) se lisent avec lui.
#[derive(Clone, Debug, Default)]
pub(crate) struct VersementLu {
    pub(crate) nir: String,
    /// .001.
    pub(crate) verse_le: Option<Jour>,
    /// .002.
    pub(crate) net_imposable: Option<Decimal>,
    /// .004.
    pub(crate) net_verse: Option<Decimal>,
    /// .006 a .008.
    pub(crate) taux_pas: Option<Decimal>,
    pub(crate) type_pas: Option<String>,
    pub(crate) identifiant_pas: Option<String>,
    /// .009.
    pub(crate) pas: Option<Decimal>,
    /// Le contrat de ses remunerations (S21.G00.51.010). UN VERSEMENT DE DEUX CONTRATS ne dit pas a lequel ses bases (78)
    /// se rattachent : elles ne se gardent pas.
    pub(crate) contrat: Option<String>,
    pub(crate) plusieurs_contrats: bool,
    /// S21.G00.51.013 de type 001 (le brut), et de type 010 : « la remuneration brute habituelle du salarie »
    /// (`bloc-S21.G00.51.txt`), le salaire du contrat d'un permanent.
    pub(crate) brut: Option<Decimal>,
    pub(crate) salaire_base: Option<Decimal>,
    /// LES BASES ASSUJETTIES QUE LA DSN DECLARE (bloc 78 : son code .001, son montant .004 ;
    /// `norme-dsn/texte/bloc-S21.G00.78.txt`), par code, sommees sur les periodes de rattachement du versement.
    pub(crate) bases: BTreeMap<String, Decimal>,
}

/// Une adhesion prevoyance (S21.G00.15 : la reference .001, l'organisme .002, la reference de l'adhesion .005).
#[derive(Clone, Debug, Default)]
pub(crate) struct Adhesion {
    pub(crate) libelle: String,
    pub(crate) organisme: Option<String>,
    pub(crate) reference: Option<String>,
}

/// CE QUE L'IMPORT LIT D'UNE DSN DEPOSEE.
#[derive(Clone, Debug)]
pub(crate) struct DsnLue {
    /// Le mois principal declare (S20.G00.05.005).
    pub(crate) mois: Mois,
    /// Le jour de constitution de l'envoi (S20.G00.05.007).
    pub(crate) depot: Option<Jour>,
    pub(crate) fichier: String,
    pub(crate) entreprise: EntrepriseDeclaree,
    pub(crate) individus: Vec<IndividuDeclare>,
    pub(crate) contrats: Vec<ContratDeclare>,
    pub(crate) versements: Vec<VersementLu>,
    /// Les etablissements declares (S21.G00.11.001) et le NIC du siege (S21.G00.06.002).
    pub(crate) etablissements: Vec<String>,
    pub(crate) nic_siege: Option<String>,
    pub(crate) adhesions: Vec<Adhesion>,
    /// Les references d'adhesion auxquelles un contrat se rattache (S21.G00.70.013).
    pub(crate) rattachements: Vec<String>,
}

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
pub(crate) fn jjmmaaaa(v: &str) -> String {
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

/// Ce qu'une DSN porte et que l'import ne lit pas : une valeur qui n'est pas ce que sa rubrique dit, ou une ligne qui
/// n'est pas une rubrique de la norme.
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
    fn refus(self, mois: &Mois) -> Option<Echec> {
        let de = mois.libelle(socle::mois::Graphie::Accentuee);
        let ce = match self.0? {
            Faute::Valeur { rubrique, valeur, quoi } => format!("porte, en {}, « {} », qui n'est pas {}", rubrique, valeur, quoi),
            Faute::Rubrique { ligne, version: Some(v) } => format!("porte la rubrique {}, que la norme DSN {} ne connaît pas", ligne, v),
            Faute::Rubrique { ligne, version: None } => format!("porte la ligne « {} », qui n'est pas une rubrique de la norme DSN", ligne),
        };
        Some(socle::echec::erreur(
            "ErreurOperation",
            format!("La DSN de {} du dossier {} : corrigez-la ou retirez-la du dossier, puis relancez l'import.", de, ce),
        ))
    }
}

/// Le premier qui le dit.
fn premier(a: &mut Option<String>, v: &Option<String>) {
    if a.is_none() {
        *a = v.clone();
    }
}

/// UNE DSN DEPOSEE, LUE. `Ok(None)` si le fichier ne se lit pas ou ne dit pas son mois (une DSN illisible, que l'import
/// ignore en le disant). Une valeur qui n'est pas ce que sa rubrique dit (un montant, une date), ou une ligne qui n'est
/// pas une rubrique, refuse : rien ne se devine (regle 1). La rubrique se controle au tableau des usages de la version
/// que le fichier declare (S10.G00.00.006, `usages-dsn-base.json`) ; une version que le logiciel ne livre pas (une DSN
/// d'avant 2026) ne se controle que par la forme de ses rubriques (`usages::est_rubrique`).
pub(crate) fn lire_une_dsn(chemin: &Path) -> Result<Option<DsnLue>, Echec> {
    let Ok(octets) = std::fs::read(chemin) else { return Ok(None) };
    let contenu = texte_latin1(&octets);
    // L'indicateur d'ordre des octets d'UTF-8, lu en latin-1, n'est pas une rubrique.
    let lignes = lignes_du_texte(contenu.trim_start_matches("\u{ef}\u{bb}\u{bf}"));
    let valeur = |v: &str| v.trim_matches(blanc).trim_matches('\'').to_string();
    let norme = lignes
        .iter()
        .find_map(|l| l.split_once(',').filter(|(k, _)| *k == "S10.G00.00.006").map(|(_, v)| valeur(v)))
        .and_then(|v| declarations::norme::Norme::connue(&v).ok());
    let mut fautes = Fautes::default();
    let (mut mois, mut depot): (Option<Mois>, Option<Jour>) = (None, None);
    let mut e = EntrepriseDeclaree::default();
    let mut individus: Vec<IndividuDeclare> = Vec::new();
    let mut contrats: Vec<ContratDeclare> = Vec::new();
    let mut versements: Vec<VersementLu> = Vec::new();
    // Le code de la base (78.001) et le type de la remuneration (51.011) en cours, dans le versement en cours.
    let (mut base, mut type_remuneration): (Option<String>, Option<String>) = (None, None);
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
    let mut spst = String::new();
    for ligne in &lignes {
        if ligne.trim_matches(blanc).is_empty() {
            continue;
        }
        let Some((k, v)) = ligne.split_once(',').filter(|(k, _)| declarations::usages::est_rubrique(k)) else {
            fautes.noter(Faute::Rubrique { ligne: ligne.to_string(), version: None });
            continue;
        };
        if let Some(n) = &norme {
            if !declarations::usages::rubrique_connue(n, k).map_err(Echec::from)? {
                fautes.noter(Faute::Rubrique { ligne: k.to_string(), version: Some(n.version.clone()) });
                continue;
            }
        }
        let v = valeur(v);
        let plein = Some(v.clone()).filter(|x| !x.is_empty());
        let individu = individus.len().checked_sub(1);
        let versement = versements.last_mut();
        let contrat = contrats.last_mut();
        match k {
            "S10.G00.01.001" => premier(&mut emetteur_siren, &plein),
            "S10.G00.01.003" => premier(&mut emetteur_nom, &plein),
            "S10.G00.02.001" => civilite_emetteur = plein,
            "S10.G00.02.002" => nom_emetteur = plein,
            "S20.G00.05.005" => mois = Mois::de_la_dsn(&v).ok(),
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
            "S21.G00.11.022" | "S21.G00.40.017" => premier(&mut e.idcc, &plein),
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
                contrats.push(ContratDeclare { nir: individus[i].nir.clone(), debut, ..ContratDeclare::default() });
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
            "S21.G00.40.022" => {
                if let Some(n) = v.strip_prefix("97").filter(|n| documents::conges_spectacles::faute_adherent(n).is_none()) {
                    premier(&mut e.adherent_conges_spectacles, &Some(n.to_string()));
                }
            }
            "S21.G00.40.040" => contrat.into_iter().for_each(|c| c.code_risque = plein.clone()),
            "S21.G00.40.043" => {
                let t = fautes.nombre(k, &v);
                contrat.into_iter().for_each(|c| c.taux_at = t);
            }
            "S21.G00.40.049" => premier(&mut e.licence, &plein),
            "S21.G00.40.050" => contrat.into_iter().for_each(|c| c.objet = plein.clone()),
            "S21.G00.50.001" => {
                let Some(i) = individu else { continue };
                let verse_le = fautes.jour(k, &v);
                versements.push(VersementLu { nir: individus[i].nir.clone(), verse_le, ..VersementLu::default() });
                (base, type_remuneration) = (None, None);
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
            "S21.G00.51.010" => {
                let Some(x) = versement else { continue };
                if x.contrat.as_ref().is_some_and(|c| *c != v) {
                    x.plusieurs_contrats = true;
                }
                x.contrat = plein;
            }
            "S21.G00.51.011" if versement.is_some() => type_remuneration = plein,
            "S21.G00.51.013" => {
                let Some(x) = versement else { continue };
                match type_remuneration.take().as_deref() {
                    Some("001") => x.brut = fautes.montant(k, &v),
                    Some("010") => x.salaire_base = fautes.montant(k, &v),
                    _ => {}
                }
            }
            "S21.G00.70.013" => rattachements.push(v),
            "S21.G00.78.001" if versement.is_some() => base = plein,
            "S21.G00.78.004" => {
                let (Some(x), Some(code)) = (versement, base.clone()) else { continue };
                if let Some(m) = fautes.montant(k, &v) {
                    *x.bases.entry(code).or_default() += m;
                }
            }
            _ => {}
        }
    }
    let Some(mois) = mois else { return Ok(None) };
    if let Some(refus) = fautes.refus(&mois) {
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
        blocs20.iter().find(|b| b[0] == declarations::dsn::SIRET_FRANCE_TRAVAIL && !b[1].is_empty()).map(|b| b[1].clone());
    // QUI LES ORGANISMES CONTACTENT : le contact chez le declare de type 04 (« recouvrant, entre autres, des cotisations de
    // Securite sociale ») s'il y en a plusieurs, sinon le premier : le `declarant_dsn` de la fiche, que le moteur ecrit la
    // (`dsn.rs`, `type_du_contact`). Sa civilite n'est pas dans ce bloc : elle se lit dans celle du contact de l'emetteur
    // (S10.G00.02.001, `bloc-S10.G00.02.txt` l. 33-40) quand l'emetteur est l'entreprise (meme Siren, comme pour la raison
    // sociale) et que son contact porte le meme nom et prenom (S10.G00.02.002) -- une inference : c'est alors la meme
    // personne.
    let contact = contacts.iter().find(|c| c.qualite.as_deref() == Some(declarations::dsn::TYPE_CONTACT_ORDINAIRE)).or(contacts.first());
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
    for x in versements.iter_mut().filter(|x| x.plusieurs_contrats) {
        x.bases.clear();
    }
    Ok(Some(DsnLue {
        mois,
        depot,
        fichier: chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        entreprise: e,
        individus,
        contrats,
        versements,
        etablissements,
        nic_siege,
        adhesions,
        rattachements,
    }))
}

impl EntrepriseDeclaree {
    pub(crate) fn est_vide(&self) -> bool {
        self.fiche().is_empty()
    }

    /// Ce qu'une DSN plus recente declare remplace ce que disait la precedente.
    fn suivre(&mut self, r: &EntrepriseDeclaree) {
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
    pub(crate) fn fiche(&self) -> Map<String, Value> {
        en_fiche(self)
    }
}

/// Une forme ecrite en fiche : des textes, qui s'ecrivent toujours.
fn en_fiche<T: Serialize>(t: &T) -> Map<String, Value> {
    match serde_json::to_value(t) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

// ------------------------------------------------------------------ toutes les DSN d'un dossier

/// Ce qu'une DSN dit d'un salarie.
pub(crate) struct SalarieDeclare {
    pub(crate) nir: String,
    pub(crate) individu: IndividuDeclare,
    /// L'emploi de son dernier contrat de la DSN.
    pub(crate) emploi: EmploiDeclare,
    pub(crate) mois: String,
    pub(crate) initiales: String,
    pub(crate) premier_contrat: Option<Jour>,
}

impl SalarieDeclare {
    /// La fiche salarie que la DSN remplit : son identite, puis son emploi.
    pub(crate) fn fiche(&self) -> Map<String, Value> {
        #[derive(Serialize)]
        struct Fiche<'a> {
            #[serde(flatten)]
            individu: &'a IndividuDeclare,
            #[serde(flatten)]
            emploi: &'a EmploiDeclare,
        }
        en_fiche(&Fiche { individu: &self.individu, emploi: &self.emploi })
    }
}

/// Un numero d'objet spectacle, vu dans une DSN ou dans une AEM.
pub(crate) struct ObjetDeclare {
    pub(crate) numero: String,
    pub(crate) premier_mois: String,
    pub(crate) dernier_mois: String,
    pub(crate) contrats: i64,
    pub(crate) source: &'static str,
}

/// Un versement qu'une DSN declare, avec le contrat de ses remunerations (le dernier de ce numero dans la DSN).
#[derive(Clone, Debug)]
pub(crate) struct PaieDeclaree {
    pub(crate) versement: VersementLu,
    pub(crate) contrat: Option<ContratDeclare>,
    pub(crate) depot: Option<Jour>,
    pub(crate) fichier: String,
}

/// Les listes gardent l'ordre ou les cles sont apparues.
#[derive(Default)]
pub(crate) struct LectureDsn {
    pub(crate) fichiers: usize,
    pub(crate) salaries: Vec<SalarieDeclare>,
    pub(crate) objets: Vec<ObjetDeclare>,
    pub(crate) entreprise: EntrepriseDeclaree,
    pub(crate) licences: Vec<(String, String)>,
    pub(crate) premier_mois: String,
    pub(crate) dernier_mois: String,
    pub(crate) illisibles: Vec<String>,
    /// Les etablissements declares (S21.G00.11.001), et le NIC du siege (S21.G00.06.002), d'une DSN a l'autre.
    pub(crate) etablissements: BTreeSet<String>,
    pub(crate) nics_siege: BTreeSet<String>,
    /// Par (mois, NIR, numero du contrat).
    pub(crate) paies: BTreeMap<(String, String, String), PaieDeclaree>,
    /// Le code risque et le taux AT/MP que les contrats declarent (S21.G00.40.040, .043), par annee : ceux de la derniere DSN
    /// de l'annee (son mois, le code, le taux ; aucun taux pour 999ZZ), quand tous ses contrats disent le meme.
    pub(crate) atmp: BTreeMap<i32, (String, String, Option<Decimal>)>,
    /// Les adhesions prevoyance de la derniere DSN qui en declare.
    pub(crate) adhesions: Vec<Adhesion>,
}

/// L'extension d'un nom de fichier, en minuscules.
fn suffixe_minuscule(nom: &str) -> String {
    match nom.rfind('.') {
        Some(i) if i > 0 && i < nom.len() - 1 => nom[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn noms_tries(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = match std::fs::read_dir(dossier) {
        Ok(l) => l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

/// Une DSN se reconnait a son contenu, pas a son nom.
pub(crate) fn fichiers_dsn(dossier: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for nom in noms_tries(dossier) {
        let f = dossier.join(&nom);
        if !f.is_file() {
            continue;
        }
        if suffixe_minuscule(&nom) == ".dsn" {
            out.push(f);
            continue;
        }
        let Ok(mut h) = std::fs::File::open(&f) else { continue };
        let mut tete = [0u8; 64];
        let mut lu = 0;
        {
            use std::io::Read;
            while lu < tete.len() {
                match h.read(&mut tete[lu..]) {
                    Ok(0) => break,
                    Ok(n) => lu += n,
                    Err(_) => break,
                }
            }
        }
        let mut t = &tete[..lu];
        while t.starts_with(b"\xef\xbb\xbf") {
            t = &t[3..];
        }
        if t.starts_with(b"S10.G00.00.001,") {
            out.push(f);
        }
    }
    out
}

/// Toutes les DSN d'un dossier, la plus recente faisant foi par salarie. Une DSN fautive refuse (`lire_une_dsn`).
pub(crate) fn lire_dsn(dossier: &Path) -> Result<LectureDsn, Echec> {
    let mut out = LectureDsn::default();
    let mut lues: Vec<DsnLue> = Vec::new();
    for f in fichiers_dsn(dossier) {
        match lire_une_dsn(&f)? {
            Some(info) => lues.push(info),
            None => out.illisibles.push(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
        }
    }
    // Par mois declare, puis par date de depot : un annule-et-remplace depose apres l'original fait foi.
    lues.sort_by(|a, b| (a.mois, a.depot, &a.fichier).cmp(&(b.mois, b.depot, &b.fichier)));
    out.fichiers = lues.len();
    if let (Some(p), Some(d)) = (lues.first(), lues.last()) {
        out.premier_mois = p.mois.to_string();
        out.dernier_mois = d.mois.to_string();
    }
    let mut index_salaries: HashMap<String, usize> = HashMap::new();
    let mut index_objets: HashMap<String, usize> = HashMap::new();
    for info in &lues {
        let mois = info.mois.to_string();
        out.etablissements.extend(info.etablissements.iter().cloned());
        out.nics_siege.extend(info.nic_siege.clone());
        out.entreprise.suivre(&info.entreprise);
        if let Some(l) = &info.entreprise.licence {
            if out.licences.last().map_or(true, |x| x.1 != *l) {
                out.licences.push((mois.clone(), l.clone()));
            }
        }
        // Le code risque de la DSN : un seul pour tous ses contrats, sinon rien (on ne choisit pas).
        let risques: BTreeSet<(String, Option<Decimal>)> =
            info.contrats.iter().filter_map(|c| c.code_risque.clone().map(|r| (r, c.taux_at))).collect();
        if risques.len() == 1 {
            if let Some((code, taux)) = risques.into_iter().next() {
                out.atmp.insert(info.mois.annee(), (mois.clone(), code, taux));
            }
        }
        if !info.adhesions.is_empty() {
            out.adhesions = info.adhesions.clone();
        }
        let mut dernier_contrat: HashMap<&str, &ContratDeclare> = HashMap::new();
        let mut par_numero: HashMap<&str, &ContratDeclare> = HashMap::new();
        for c in &info.contrats {
            dernier_contrat.insert(c.nir.as_str(), c);
            if let Some(n) = &c.numero {
                par_numero.insert(n.as_str(), c);
            }
            if let Some(objet) = &c.objet {
                let i = *index_objets.entry(objet.clone()).or_insert_with(|| {
                    out.objets.push(ObjetDeclare {
                        numero: objet.clone(),
                        premier_mois: mois.clone(),
                        dernier_mois: mois.clone(),
                        contrats: 0,
                        source: "DSN",
                    });
                    out.objets.len() - 1
                });
                out.objets[i].dernier_mois = mois.clone();
                out.objets[i].contrats += 1;
            }
        }
        for v in &info.versements {
            let Some(numero) = &v.contrat else { continue };
            let paie = PaieDeclaree {
                versement: v.clone(),
                contrat: par_numero.get(numero.as_str()).map(|c| (*c).clone()),
                depot: info.depot,
                fichier: info.fichier.clone(),
            };
            out.paies.insert((mois.clone(), v.nir.clone(), numero.clone()), paie);
        }
        for ind in &info.individus {
            let nir = ind.nir.clone();
            let c = dernier_contrat.get(nir.as_str()).copied();
            let numero = c.and_then(|c| c.numero.as_deref()).unwrap_or("");
            let initiales: String = numero.split('.').next().unwrap_or("").chars().filter(|ch| ch.is_alphabetic()).take(2).collect();
            let avant = index_salaries.get(&nir).and_then(|&i| out.salaries[i].premier_contrat);
            let premier = info.contrats.iter().filter(|x| x.nir == nir).filter_map(|x| x.debut).chain(avant).min();
            let decl = SalarieDeclare {
                nir: nir.clone(),
                individu: ind.clone(),
                emploi: c.map(|c| c.emploi.du_salarie()).unwrap_or_default(),
                mois: mois.clone(),
                initiales,
                premier_contrat: premier,
            };
            match index_salaries.get(&nir) {
                Some(&i) => out.salaries[i] = decl,
                None => {
                    out.salaries.push(decl);
                    index_salaries.insert(nir, out.salaries.len() - 1);
                }
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ les AEM

/// Nom -> (debut, fin), en caracteres.
/// `ligne[a:b]` : une ligne plus courte rend ce qu'elle a.
fn tranche(ligne: &[char], a: usize, b: usize) -> String {
    ligne.iter().skip(a).take(b.saturating_sub(a)).collect()
}

/// `19900723` -> `1990-07-23` (les dates de l'AEM), le reste tel quel.
fn aaaammjj(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    // Des chiffres : les chiffres ASCII et les exposants de Latin-1 (qu'une AEM lue en latin-1 peut porter).
    if c.len() == 8 && c.iter().all(|x| x.is_ascii_digit() || matches!(x, '\u{b2}' | '\u{b3}' | '\u{b9}')) {
        return format!("{}-{}-{}", c[0..4].iter().collect::<String>(), c[4..6].iter().collect::<String>(), c[6..8].iter().collect::<String>());
    }
    v.to_string()
}

fn champ(ligne: &[char], a: usize, b: usize) -> String {
    ligne[a..b].iter().collect()
}

/// Un enregistrement AEM, un contrat tel qu'il a ete atteste.
pub(crate) struct ContratAtteste {
    pub(crate) nir: String,
    pub(crate) debut: socle::date::Jour,
    pub(crate) fin: socle::date::Jour,
    pub(crate) numero_objet: String,
    pub(crate) heures: i128,
    /// Les cachets, et les cachets « groupes » (la case que le gabarit garde a 00).
    pub(crate) cachets: i128,
    pub(crate) jours: i128,
    /// Le brut en euros, deux décimales.
    pub(crate) brut: socle::num::Decimal,
    /// Le numero de l'attestation (zone 36..42), et sa date de signature (858..866, AAAA-MM-JJ si c'en est une).
    pub(crate) numero_attestation: String,
    pub(crate) date_attestation: String,
    pub(crate) fichier: String,
}

/// Le contrat atteste, tel qu'un engagement importe le porte (`salarie` : la cle de sa fiche).
pub(crate) fn ligne_du_contrat(c: &ContratAtteste, salarie: &str) -> engagement::importe::ContratLu {
    use engagement::importe::{Attestation, ContratAtteste as Atteste, ContratLu};
    let plein = |s: &str| Some(s.to_string()).filter(|s| !s.trim().is_empty());
    let mut a = Atteste::vide(Attestation::Aem);
    a.numero_objet = plein(&c.numero_objet);
    (a.heures, a.cachets, a.jours) = (Some(c.heures as i64), Some(c.cachets as i64), Some(c.jours as i64));
    a.brut = Some(c.brut);
    a.numero_attestation = plein(&c.numero_attestation);
    a.atteste_le = plein(&c.date_attestation);
    a.fichier = plein(&c.fichier);
    ContratLu { salarie: salarie.to_string(), debut: c.debut.iso(), fin: c.fin.iso(), numero: None, contrat: a }
}

/// Tous les contrats attestes d'un dossier AEM_EDI, dedoublonnes, et les
/// fichiers ou enregistrements illisibles. Un `InvalidOperation` (un brut qui n'est pas un nombre)
/// n'est pas rattrape.
pub(crate) fn lire_aem(dossier: &Path) -> Result<(Vec<ContratAtteste>, Vec<String>), socle::echec::Echec> {
    use socle::num::{decimal_lu, entier_lu};
    let mut vus: Vec<(Vec<String>, ContratAtteste)> = Vec::new();
    let mut connus: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
    let mut illisibles = Vec::new();
    // `glob.glob(".../*.txt")` : pas de fichier cache, ordre des chaines.
    let mut noms: Vec<String> = noms_tries(dossier).into_iter().filter(|n| !n.starts_with('.') && n.ends_with(".txt")).collect();
    noms.sort();
    for nom in noms {
        let f = dossier.join(&nom);
        let Ok(octets) = std::fs::read(&f) else {
            illisibles.push(nom);
            continue;
        };
        for ligne in lignes_du_texte(&texte_latin1(&octets)) {
            let l: Vec<char> = ligne.chars().collect();
            if l.len() < 700 {
                continue;
            }
            let illisible = |illisibles: &mut Vec<String>| illisibles.push(format!("{} : un enregistrement illisible", nom));
            let nir: String = champ(&l, 375, 390).chars().take(13).collect();
            let (Some(debut), Some(fin)) =
                (socle::date::lire_aaaammjj(&champ(&l, 618, 626)), socle::date::lire_aaaammjj(&champ(&l, 627, 635)))
            else {
                illisible(&mut illisibles);
                continue;
            };
            let numero_objet = champ(&l, 638, 650).trim_matches(blanc).to_string();
            let entiers = [(650, 653), (653, 655), (655, 657), (657, 659)].map(|(a, b)| entier_lu(&champ(&l, a, b)));
            let [Some(heures), Some(c1), Some(c2), Some(jours)] = entiers else {
                illisible(&mut illisibles);
                continue;
            };
            let brut_texte = champ(&l, 659, 668);
            let Some(brut_cts) = decimal_lu(&brut_texte) else {
                return Err(socle::echec::Echec { classe: Some("InvalidOperation"), message: socle::num::illisible(&brut_texte) });
            };
            let mut brut = brut_cts / socle::num::Decimal::from(100);
            brut = brut.round_dp(2);
            brut.rescale(2);
            let cle = vec![
                nir.clone(),
                debut.iso(),
                fin.iso(),
                numero_objet.clone(),
                heures.to_string(),
                (c1 + c2).to_string(),
                jours.to_string(),
                brut.to_string(),
            ];
            if connus.insert(cle.clone()) {
                vus.push((cle, ContratAtteste {
                    nir,
                    debut,
                    fin,
                    numero_objet,
                    heures,
                    cachets: c1 + c2,
                    jours,
                    brut,
                    numero_attestation: tranche(&l, 36, 42).trim_matches(blanc).to_string(),
                    date_attestation: aaaammjj(&tranche(&l, 858, 866)),
                    fichier: nom.clone(),
                }));
            }
        }
    }
    let mut contrats: Vec<ContratAtteste> = vus.into_iter().map(|(_, c)| c).collect();
    contrats.sort_by(|a, b| (a.debut, a.fin, &a.nir).cmp(&(b.debut, b.fin, &b.nir)));
    Ok((contrats, illisibles))
}

// ------------------------------------------------------------------ l'entreprise de chaque piece

/// Ou l'AEM porte le Siret de l'employeur : `ligne[50:64]`, 14 chiffres. `CHAMPS` ne
/// nomme pas cette zone (elle est figee dans le gabarit, comme tout ce que l'AEM ne change pas d'un contrat a
/// l'autre) : elle a ete relevee sur 190 enregistrements AEM produits par CoolPaie de 2021 a 2026, ou le Siret de la
/// fiche de la compagnie se lit a cette place dans tous. Un Siret ne se tient pour lu que s'il a ses 14 chiffres et sa
/// cle de Luhn ; sinon l'AEM passe, avec un avertissement nomme (`AEM_SANS_SIRET`) -- on ne refuse pas au hasard.
pub(crate) const AEM_SIRET: (usize, usize) = (50, 64);

/// Ce qu'une piece (DSN, AEM) dit de son employeur.
#[derive(Default, Clone)]
pub(crate) struct Employeur {
    /// Les Siren qu'elle porte (un, d'ordinaire ; une AEM en porte un par enregistrement).
    pub(crate) sirens: std::collections::BTreeSet<String>,
    /// Les enregistrements (AEM) ou l'identifiant de l'employeur ne se lit pas.
    pub(crate) illisibles: usize,
}

/// Une DSN : le Siren de l'ENTREPRISE (`S21.G00.06.001`), pas celui de l'emetteur (`S10.G00.01.001`), qui est celui
/// du cabinet quand un tiers depose (voir `ENTREPRISE`).
pub(crate) fn employeur_dsn(chemin: &Path) -> Employeur {
    let mut e = Employeur::default();
    let Ok(octets) = std::fs::read(chemin) else { return e };
    for l in lignes_du_texte(&texte_latin1(&octets)) {
        if let Some(v) = l.strip_prefix("S21.G00.06.001,") {
            let v = v.trim_matches(blanc).trim_matches('\'').trim();
            if !v.is_empty() {
                e.sirens.insert(v.to_string());
            }
        }
    }
    e
}

/// Une AEM : le Siret de chaque enregistrement (`AEM_SIRET`), ramene a son Siren. Un fichier qui ne se lit pas ne dit
/// rien ici : `lire_aem` le dit illisible.
pub(crate) fn employeur_aem(chemin: &Path) -> Employeur {
    let mut e = Employeur::default();
    let Ok(octets) = std::fs::read(chemin) else { return e };
    for ligne in lignes_du_texte(&texte_latin1(&octets)) {
        let l: Vec<char> = ligne.chars().collect();
        // Les enregistrements que `lire_aem` lit.
        if l.len() < 700 {
            continue;
        }
        let s = tranche(&l, AEM_SIRET.0, AEM_SIRET.1);
        if s.len() == 14 && s.chars().all(|c| c.is_ascii_digit()) && declarations::champs::luhn(&s) {
            e.sirens.insert(s[..9].to_string());
        } else {
            e.illisibles += 1;
        }
    }
    e
}

/// LE SIREN DE LA COMPAGNIE, pour tout ce que l'import lit (DSN, AEM, bulletins) : celui de la fiche Entreprise ;
/// sinon celui des DSN, si elles n'en portent qu'un (plusieurs : aucune reference) ; sinon, sans DSN qui le dise, celui
/// des autres pieces (bulletins, AEM) si elles n'en portent qu'un. `None` : rien ne dit laquelle est la compagnie.
pub(crate) fn siren_de_reference(
    fiche: &droit::fiche::ChampsEntreprise,
    dsn: &std::collections::BTreeSet<String>,
    autres: &std::collections::BTreeSet<String>,
) -> Option<String> {
    let s = fiche.siren.texte().trim();
    if !s.is_empty() {
        return Some(s.to_string());
    }
    let seul = |e: &std::collections::BTreeSet<String>| (e.len() == 1).then(|| e.iter().next().cloned()).flatten();
    if !dsn.is_empty() {
        return seul(dsn);
    }
    seul(autres)
}

/// Une piece dont l'entreprise ne se rapporte a rien : le dossier porte plusieurs entreprises, aucune ne se tranche.
pub(crate) const PLUSIEURS_ENTREPRISES: &str = "le dossier porte plusieurs entreprises (Siren des DSN, Siret des bulletins ou des AEM), et ni la fiche Entreprise ni une seule DSN ne dit laquelle est la compagnie : non repris";
pub(crate) const DSN_AUTRE: &str = "DSN d'une autre entreprise : son Siren n'est pas celui de la compagnie, non reprise";
pub(crate) const DSN_SANS_SIREN: &str = "DSN sans Siren lisible (S21.G00.06.001) : rien ne dit de quelle entreprise elle vient, non reprise";
pub(crate) const AEM_AUTRE: &str = "AEM d'une autre entreprise : son Siret n'est pas celui de la compagnie, non reprise";
pub(crate) const AEM_SANS_SIRET: &str =
    "son Siret ne se lit pas (caractères 51 à 64 de l'enregistrement) : l'entreprise n'est pas contrôlée";

/// Le sort d'une DSN (`aem` faux) ou d'une AEM devant la reference : `Err(raison)` -- elle n'est pas reprise et RIEN
/// n'en est lu --, `Ok(Some(avertissement))` -- elle passe, mais son entreprise ne se controle pas --, ou `Ok(None)`.
pub(crate) fn verdict_entreprise(aem: bool, e: &Employeur, reference: Option<&str>) -> Result<Option<String>, String> {
    if e.sirens.is_empty() {
        // Une AEM sans enregistrement lisible ne rend rien a `lire_aem` : rien a controler.
        return if aem { Ok((e.illisibles > 0).then(|| AEM_SANS_SIRET.to_string())) } else { Err(DSN_SANS_SIREN.to_string()) };
    }
    let Some(r) = reference else { return Err(PLUSIEURS_ENTREPRISES.to_string()) };
    if e.sirens.iter().any(|s| s != r) {
        return Err((if aem { AEM_AUTRE } else { DSN_AUTRE }).to_string());
    }
    Ok((e.illisibles > 0).then(|| AEM_SANS_SIRET.to_string()))
}


#[cfg(test)]
mod tests_parametrage {
    /// Ce que l'import lit d'une DSN pour la configuration : le contact chez le declare, « nom et prenom » en une rubrique
    /// (S20.G00.07.001 a .004, `norme-dsn/texte/bloc-S20.G00.07.txt` l. 63-84), sa civilite par le contact de l'emetteur
    /// quand l'emetteur est l'entreprise et le nom le meme (S10.G00.02.001-002) ; le code risque et le taux AT/MP du contrat
    /// (S21.G00.40.040, .043, `bloc-S21.G00.40.txt` l. 1224-1315) ; les adhesions prevoyance (S21.G00.15.001, .002, .005).
    #[test]
    fn la_dsn_dit_son_contact_son_code_risque_et_ses_adhesions() {
        let d = std::env::temp_dir().join(format!("puppetpaie-dsn-parametrage-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("essai.dsn");
        let lignes = [
            "S10.G00.00.001,'PuppetPaie'", "S10.G00.01.001,'123456873'", "S10.G00.01.003,'COMPAGNIE DU FIL DE FER'",
            "S10.G00.02.001,'02'", "S10.G00.02.002,'FONTAINE CLAIRE'",
            "S20.G00.05.005,'01122025'", "S20.G00.07.001,'FONTAINE CLAIRE'", "S20.G00.07.002,'0478000000'",
            "S20.G00.07.003,'contact@exemple.invalid'", "S20.G00.07.004,'04'",
            "S21.G00.06.001,'123456873'", "S21.G00.11.001,'00017'",
            "S21.G00.15.001,'PREV SPECT VIVANT PRIV'", "S21.G00.15.002,'P0983'", "S21.G00.15.005,'104'",
            "S21.G00.30.001,'2930899999999'", "S21.G00.40.001,'01012025'", "S21.G00.40.040,'923AD'", "S21.G00.40.043,'1.65'",
        ];
        std::fs::write(&f, lignes.join("\n")).unwrap();
        let lu = super::lire_dsn(&d).unwrap();
        std::fs::remove_dir_all(&d).ok();
        let c = lu.entreprise.declarant_dsn.clone().unwrap_or_default();
        assert_eq!((c.nom_prenom.as_str(), c.tel.as_deref(), c.civilite.as_deref()), ("FONTAINE CLAIRE", Some("0478000000"), Some("02")));
        assert_eq!(lu.atmp.get(&2025), Some(&("2025-12".to_string(), "923AD".to_string(), socle::num::decimal_lu("1.65"))));
        assert_eq!(lu.adhesions[0].libelle, "PREV SPECT VIVANT PRIV");
        assert_eq!(lu.adhesions[0].organisme.as_deref(), Some("P0983"));
    }
}
