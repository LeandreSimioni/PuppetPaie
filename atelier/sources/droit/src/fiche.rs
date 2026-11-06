//! LES FICHES DE LA COMPAGNIE, DECLAREES UNE FOIS : celle d'un salarie (`config/salaries.json`) et celle de l'entreprise
//! avec ses autres etablissements (`config/entreprise.json`). La structure est la forme : chaque champ y a son type, une
//! cle qu'elle ne connait pas se refuse a la lecture (`lire_fichier`, la seule qui les lit), et
//! l'ecriture passe par elle (`socle::ecriture`). `donnees/champs-base.json` en garde la
//! presentation (libelles, aide, conditions d'affichage, ordre) : chacune de ses cles est un champ d'ici, et chaque champ
//! d'ici y a sa ligne (garde par `moteur/tests/fichiers/structure.rs`). La ligne `historique` (forme `histoire`) n'est
//! pas une donnee : la route de la fiche la compose des listes datees.
//!
//! Le fichier est date (`FicheDatee`) : chaque champ y est une liste `[{"depuis"?, "valeur", "note"?}]`
//! (`Datee`) ; la fiche a un jour (`ChampsSalarie`, `ChampsEntreprise`) en est la valeur en vigueur. Une valeur s'ecrit
//! nulle ou vide quand la fiche ne disait rien avant elle (`dater`) : `Case` garde ces deux formes, pour que le
//! fichier se reecrive tel qu'il est lu.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use socle::date::Jour;
use socle::mois::Mois;
use socle::num::{Decimal, Resultat};

use crate::contexte::IdSalarie;

// ------------------------------------------------------------------ une case

/// UNE CASE D'UNE FICHE : absente, nulle (une case a cocher que la fiche ne disait pas), vide (un texte qu'elle ne
/// disait pas, `""`), ou dite. Seule `Dite` porte une valeur ; les trois autres se relisent et se reecrivent telles
/// quelles.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Case<T> {
    #[default]
    Absente,
    Nulle,
    Vide,
    Dite(T),
}

impl<T> Case<T> {
    /// La valeur dite, `None` sinon.
    pub fn dite(&self) -> Option<&T> {
        match self {
            Case::Dite(v) => Some(v),
            _ => None,
        }
    }

    pub fn est_dite(&self) -> bool {
        matches!(self, Case::Dite(_))
    }

    /// Pour `skip_serializing_if` : une case absente ne s'ecrit pas.
    pub fn est_absente(&self) -> bool {
        matches!(self, Case::Absente)
    }
}

impl<T> Case<T> {
    /// La valeur dite, que l'acte exige : sinon le refus nomme la case (`cle`).
    pub fn exigee(&self, cle: &str) -> Resultat<&T> {
        self.dite().ok_or_else(|| socle::fichiers::absent(cle))
    }
}

impl<T: AsRef<str>> Case<T> {
    /// Le texte dit, `""` sinon.
    pub fn texte(&self) -> &str {
        self.dite().map_or("", AsRef::as_ref)
    }

    /// La meme case, son texte en `String` : pour une table de cases qui les ecrit toutes de meme.
    pub fn en_texte(&self) -> Case<String> {
        match self {
            Case::Absente => Case::Absente,
            Case::Nulle => Case::Nulle,
            Case::Vide => Case::Vide,
            Case::Dite(t) => Case::Dite(t.as_ref().to_string()),
        }
    }

    /// Le texte dit, s'il n'est pas vide.
    pub fn remplie(&self) -> Option<&str> {
        self.dite().map(AsRef::as_ref).filter(|t| !t.is_empty())
    }

    /// Le texte que la fiche porte, vide compris ; `None` : la case est absente ou nulle.
    pub fn portee(&self) -> Option<&str> {
        match self {
            Case::Absente | Case::Nulle => None,
            c => Some(c.texte()),
        }
    }

    /// Le texte d'une case que la fiche doit porter, vide ou non : absente, le refus la nomme (`cle`).
    pub fn ecrite(&self, cle: &str) -> Resultat<&str> {
        match self {
            Case::Absente => Err(socle::fichiers::absent(cle)),
            c => Ok(c.texte()),
        }
    }
}

impl<T: Choix> Case<T> {
    /// Un choix dit, tel qu'il s'ecrit ; `""` sinon.
    pub fn code(&self) -> &'static str {
        self.dite().map_or("", Choix::code)
    }
}

/// Un choix (`choix!`) : sa valeur ecrite.
pub trait Choix {
    fn code(&self) -> &'static str;
}

impl Case<bool> {
    /// Cochee : `true` dit ; ni nulle, ni absente, ni `false`.
    pub fn oui(&self) -> bool {
        matches!(self, Case::Dite(true))
    }
}

impl<T: Serialize> Serialize for Case<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Case::Absente | Case::Nulle => s.serialize_unit(),
            Case::Vide => s.serialize_str(""),
            Case::Dite(v) => v.serialize(s),
        }
    }
}

/// Lue par sa forme (`socle::forme::Remplie`) : le chemin d'un refus va jusqu'a la case fautive.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Case<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match socle::forme::Remplie::<T>::deserialize(d)? {
            socle::forme::Remplie::Nulle => Case::Nulle,
            socle::forme::Remplie::Vide => Case::Vide,
            socle::forme::Remplie::Dite(v) => Case::Dite(v),
        })
    }
}

/// UN MONTANT OU UN NOMBRE D'UNE FICHE : un nombre JSON, ou un texte qui se lit en decimal (`12,5` compris), garde tel
/// qu'il est ecrit (`socle::forme::NombreEcrit`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "socle::forme::NombreEcrit")]
pub enum Montant {
    Entier(i64),
    Naturel(u64),
    Flottant(f64),
    /// Un texte qui se lit en decimal (`socle::num::decimal_lu`).
    Texte(String),
}

impl Montant {
    pub fn decimal(&self) -> Decimal {
        match self {
            Montant::Entier(i) => Decimal::from(*i),
            Montant::Naturel(u) => Decimal::from(*u),
            Montant::Flottant(x) => socle::num::dec_f64(*x).unwrap_or_default(),
            Montant::Texte(s) => socle::num::decimal_lu(s).unwrap_or_default(),
        }
    }

    /// Le decimal, un flottant gardant son echelle telle qu'elle s'ecrit (`130.0`) ; un texte se lit sans virgule.
    pub fn avec_echelle(&self) -> Resultat<Decimal> {
        match self {
            Montant::Flottant(x) => socle::num::dec_texte(&socle::date::repr_flottant(*x)),
            Montant::Texte(s) => socle::num::dec_texte(s),
            autre => Ok(autre.decimal()),
        }
    }

    /// Le decimal, un flottant par sa forme la plus courte (`socle::num::dec_f64`) ; un texte se lit sans virgule.
    pub fn dec(&self) -> Resultat<Decimal> {
        match self {
            Montant::Flottant(x) => socle::num::dec_f64(*x),
            Montant::Texte(s) => socle::num::dec_texte(s),
            autre => Ok(autre.decimal()),
        }
    }

    /// Un nombre qui n'est pas zero, ou un texte : ce qu'une valeur JSON vraie dit.
    pub fn non_nul(&self) -> bool {
        match self {
            Montant::Entier(i) => *i != 0,
            Montant::Naturel(u) => *u != 0,
            Montant::Flottant(x) => *x != 0.0,
            Montant::Texte(s) => !s.is_empty(),
        }
    }

    /// Le flottant ; un texte se lit sans virgule.
    pub fn flottant(&self) -> Resultat<f64> {
        match self {
            Montant::Entier(i) => Ok(*i as f64),
            Montant::Naturel(u) => Ok(*u as f64),
            Montant::Flottant(x) => Ok(*x),
            Montant::Texte(s) => s.trim().parse::<f64>().map_err(|_| socle::num::illisible(s)),
        }
    }

    /// L'entier ecrit : un nombre entier, ou un texte qui se lit en entier ; `None` sinon.
    pub fn entier(&self) -> Option<i64> {
        match self {
            Montant::Entier(i) => Some(*i),
            Montant::Texte(s) => s.trim().parse().ok(),
            Montant::Naturel(_) | Montant::Flottant(_) => None,
        }
    }

    /// Tel qu'il est ecrit, pour un message : un texte tel quel, un nombre sous sa forme ecrite.
    pub fn ecrit(&self) -> String {
        match self {
            Montant::Entier(i) => i.to_string(),
            Montant::Naturel(u) => u.to_string(),
            Montant::Flottant(x) => socle::date::repr_flottant(*x),
            Montant::Texte(s) => s.clone(),
        }
    }
}

impl Serialize for Montant {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Montant::Entier(i) => s.serialize_i64(*i),
            Montant::Naturel(u) => s.serialize_u64(*u),
            Montant::Flottant(x) => s.serialize_f64(*x),
            Montant::Texte(t) => s.serialize_str(t),
        }
    }
}

impl From<socle::forme::NombreEcrit> for Montant {
    fn from(n: socle::forme::NombreEcrit) -> Montant {
        match n {
            socle::forme::NombreEcrit::Entier(i) => Montant::Entier(i),
            socle::forme::NombreEcrit::Naturel(u) => Montant::Naturel(u),
            socle::forme::NombreEcrit::Flottant(x) => Montant::Flottant(x),
            socle::forme::NombreEcrit::Texte(s) => Montant::Texte(s),
        }
    }
}

// ------------------------------------------------------- les textes que les normes bornent

/// La borne d'un texte declare : au plus, ou exactement, sa longueur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Borne {
    AuPlus,
    Exactement,
}

impl Borne {
    fn hors(self, n: usize, longueur: usize) -> bool {
        match self {
            Borne::AuPlus => n > longueur,
            Borne::Exactement => n != longueur,
        }
    }
}

/// Le nombre de caracteres d'un texte.
fn caracteres(t: &str) -> usize {
    t.chars().count()
}

/// Le nombre de chiffres d'un texte (`socle::texte::est_chiffre`).
fn chiffres(t: &str) -> usize {
    t.chars().filter(|c| socle::texte::est_chiffre(*c)).count()
}

/// UN TEXTE DECLARE QUE LES NORMES BORNENT : il en a `LONGUEUR` (`Borne`), comptes par sa mesure, la longueur de la plus
/// courte des normes qui l'ecrivent. Hors de la borne, il se refuse a la lecture (`lire`, que la lecture du fichier
/// appelle) et a la saisie (`FormeDeCase::FAUTE`, par `declarations::champs::fautes_des_types`), en une phrase qui dit
/// quoi faire (`{texte}`, `{n}` sa mesure, `{max}` la borne) : rien ne le couperait. Un texte vide n'est pas le sien :
/// c'est une case vide (`Case::Vide`).
macro_rules! texte_borne {
    (@commun $nom:ident, $unite:literal) => {
        impl $nom {
            /// Le texte `texte`, ou ce qui le refuse.
            pub fn lire(texte: &str) -> Result<$nom, String> {
                match Self::faute(texte) {
                    Some(f) => Err(f),
                    None => Ok($nom(texte.to_string())),
                }
            }
        }

        impl AsRef<str> for $nom {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::ops::Deref for $nom {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $nom {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $nom::lire(&String::deserialize(d)?).map_err(serde::de::Error::custom)
            }
        }

        impl FormeDeCase for $nom {
            const FAUTE: Option<fn(&str) -> Option<String>> = Some($nom::faute);
            const LONGUEUR: Option<(usize, &'static str)> = Some(($nom::LONGUEUR, $unite));
        }
    };
    ($(#[$m:meta])* pub struct $nom:ident : $borne:ident $max:literal $unite:literal par $mesure:ident, $phrase:literal;) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Default, Serialize)]
        #[serde(transparent)]
        pub struct $nom(String);

        impl $nom {
            pub const LONGUEUR: usize = $max;

            /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
            pub fn faute(texte: &str) -> Option<String> {
                let n = $mesure(texte);
                (!texte.is_empty() && Borne::$borne.hors(n, Self::LONGUEUR)).then(|| format!($phrase, texte = texte, n = n, max = Self::LONGUEUR))
            }
        }

        texte_borne!(@commun $nom, $unite);
    };
}

texte_borne! {
    /// UNE LIGNE D'ADRESSE DECLAREE de l'entreprise ou d'un etablissement (la voie, puis son complement) : 32 caracteres au
    /// plus. La DPAE en ecrit deux de 32 (L_ADR_EMP_1, positions 188-219, et L_ADR_EMP_2, 220-251 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 262-276) ; la DSN, 50 (S21.G00.06.004 et .007,
    /// S21.G00.11.003 et .006, S10.G00.01.004 et .009, X [1,50] : `atelier/reference/norme-dsn/texte/bloc-S21.G00.06.txt`,
    /// `bloc-S21.G00.11.txt`, `bloc-S10.G00.01.txt`).
    pub struct LigneAdresse : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : une ligne d'adresse en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Adresse de l'établissement »). Passez la suite à la ligne suivante, « Complément d'adresse », ou abrégez";
}

texte_borne! {
    /// LA COMMUNE DECLAREE de l'entreprise ou d'un etablissement : 27 caracteres au plus. La DPAE en ecrit 27
    /// (L_BUR_DIST_EMP, positions 257-283 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 284-292) ; la DSN,
    /// 50 (S10.G00.01.006, S21.G00.06.006, S21.G00.11.005, X [1,50] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CommuneEmployeur : AuPlus 27 "caractères" par caracteres,
        "« {texte} » a {n} caractères : la commune d'un établissement en a {max} au plus, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Commune de l'établissement »). Abrégez-la";
}

texte_borne! {
    /// LE CODE POSTAL de l'entreprise ou d'un etablissement : 5 caracteres. La DPAE en ecrit 5 (C_POSTAL_EMP, positions
    /// 252-256 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 277-283) ; la DSN aussi (S10.G00.01.005,
    /// S21.G00.06.005, S21.G00.11.004, X [5,5] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CodePostal : Exactement 5 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un code postal en a {max}, ce que la DPAE et la DSN écrivent (norme DPAE v5.3, \
zone « Code postal de l'établissement » ; DSN, S21.G00.11.004). Corrigez-le";
}

texte_borne! {
    /// LE CODE APE (NAF) de l'entreprise : 5 caracteres. La DPAE en ecrit 5 (C_NAF_NN, positions 661-665 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 397-403) ; la DSN aussi (S21.G00.06.003, S21.G00.11.002,
    /// X [5,5] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CodeNaf : Exactement 5 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un code APE en a {max}, quatre chiffres et une lettre, sans point, ce que la DPAE et \
la DSN écrivent (norme DPAE v5.3, zone « Code NAF » ; DSN, S21.G00.06.003). Corrigez-le";
}

/// LE CODE DU SERVICE DE SANTE AU TRAVAIL, celui de la DPAE : « MT » et trois chiffres, 5 caracteres. La norme DPAE
/// (C_MT_DCL, positions 938-947 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 485-497) : « Il est
/// attendu ici un code du type MT+ 3 positions numériques. Ex: MT542 [...] Si vous disposez d'un centre de médecine
/// interne à l'établissement, renseignez MT999 » ; la zone en a 10, cadree a droite. La DSN 2027 ecrit la medecine
/// interne « MT01 - MEDECINE INTERNE ETABLISSEMENT » en S21.G00.11.025 (`declarations::dsn::spst_ecrit`, qui la tire de
/// `MT999`, `INTERNE`) ; l'identifiant d'un service externe y est defini par une fiche consigne qui n'est pas deposee.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct CodeMedecine(String);

impl CodeMedecine {
    pub const LONGUEUR: usize = 5;
    /// La medecine interne a l'etablissement, dans le code de la DPAE (l. 496-497).
    pub const INTERNE: &'static str = "MT999";

    /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
    pub fn faute(texte: &str) -> Option<String> {
        let c: Vec<char> = texte.chars().collect();
        let bon = c.len() == Self::LONGUEUR && c[0] == 'M' && c[1] == 'T' && c[2..].iter().all(|x| x.is_ascii_digit());
        (!texte.is_empty() && !bon).then(|| {
            format!(
                "« {} » n'est pas un code de service de santé au travail : la DPAE l'attend en MT suivi de trois chiffres, \
MT542 par exemple, et MT999 pour une médecine interne à l'établissement (norme DPAE v5.3, zone « Code centre de médecine \
du travail »). Il se lit sur l'attestation d'adhésion à ce service",
                texte
            )
        })
    }

    /// La compagnie a-t-elle sa propre medecine du travail (`INTERNE`) ?
    pub fn interne(&self) -> bool {
        self.0 == Self::INTERNE
    }
}

texte_borne!(@commun CodeMedecine, "caractères");

texte_borne! {
    /// LE TELEPHONE de l'employeur (le contact DSN, le signataire) : 11 chiffres au plus. La DPAE en ecrit les chiffres, 11
    /// au plus (N_TEL_EMP, positions 542-552 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 380-385) ; la
    /// DSN, le texte, 20 caracteres au plus (S10.G00.02.005, S20.G00.07.002, X [10,20] :
    /// `logiciel/donnees/formats-dsn-base.json`), que son controle des formats tient.
    pub struct Telephone : AuPlus 11 "chiffres" par chiffres,
        "« {texte} » a {n} chiffres : un téléphone en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, zone « Numéro de téléphone »). \
Écrivez-le sans l'indicatif international, en dix chiffres (0102030405)";
}

texte_borne! {
    /// LE NOM DECLARE d'un salarie : 32 caracteres au plus. La DPAE en ecrit 32 (L_NOM_PATRO_SAL, positions 284-315 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 293-300) ; la DSN, 80 (S21.G00.30.002, X [1,80] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct NomDeclare : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un nom déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Nom du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE PRENOM DECLARE d'un salarie : 32 caracteres au plus. La DPAE en ecrit 32 (L_PRENOMS_SAL, positions 316-347 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 301-308) ; la DSN, 80 (S21.G00.30.004, X [1,80] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct PrenomDeclare : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un prénom déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Prénom du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE LIEU DE NAISSANCE DECLARE d'un salarie (la commune ou le pays) : 24 caracteres au plus. La DPAE en ecrit 24
    /// (L_LIEU_NAISS_SAL, positions 369-392 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 323-332) ; la
    /// DSN, 30 (S21.G00.30.007, X [1,30] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct LieuNaissance : AuPlus 24 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un lieu de naissance déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Lieu de naissance du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE DEPARTEMENT DE NAISSANCE d'un salarie : 2 caracteres. La DPAE en ecrit 2 (C_DEPT_NAISS_SAL, positions 828-829 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 427-433) ; la DSN aussi (S21.G00.30.014, X [2,2] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct DepartementNaissance : Exactement 2 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un département de naissance en a {max} (75, 2A, 99 pour une naissance à \
l'étranger), ce que la DPAE et la DSN écrivent (norme DPAE v5.3, zone « Code du département de naissance » ; DSN, S21.G00.30.014). Corrigez-le";
}

texte_borne! {
    /// LE NUMERO DE SECURITE SOCIALE d'un salarie, sans sa cle : 13 caracteres. La DPAE en ecrit 13 (N_SECU_SOC, positions
    /// 348-360 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 309-315) ; la DSN aussi (S21.G00.30.001,
    /// X [13,13] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct Nir : Exactement 13 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un numéro de sécurité sociale en a {max}, sans sa clé, ce que la DPAE et la DSN \
écrivent (norme DPAE v5.3, zone « Numéro de sécurité sociale » ; DSN, S21.G00.30.001). Les deux derniers chiffres vont dans la case « Clé »";
}

/// LA RAISON SOCIALE DECLAREE : 60 caracteres au plus, en deux lignes de 32 coupees entre deux mots. La DSN en ecrit 60
/// (S10.G00.01.003, X [1,60] : `logiciel/donnees/formats-dsn-base.json`) ; la DPAE, deux lignes de 32 (L_RAISON_SOC_1,
/// positions 152-183, et L_RAISON_SOC_2, « Nom ou raison sociale (ligne 2) », 510-541 :
/// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 249-256 et 374-379).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct RaisonSociale(String);

impl RaisonSociale {
    pub const LONGUEUR: usize = 60;
    /// Une ligne de la DPAE.
    pub const LIGNE: usize = 32;

    /// Ses deux lignes de la DPAE (`LIGNE` chacune au plus) : la premiere aussi longue que les mots le permettent, la
    /// seconde le reste, vide quand tout tient sur la premiere ; sinon, la phrase qui dit quoi faire.
    pub fn en_deux_lignes(texte: &str) -> Result<(String, String), String> {
        if caracteres(texte) <= Self::LIGNE {
            return Ok((texte.to_string(), String::new()));
        }
        let c: Vec<char> = texte.chars().collect();
        let deux = (1..=Self::LIGNE).rev().find(|&i| c[i] == ' ').map(|i| (c[..i].iter().collect::<String>(), c[i + 1..].iter().collect::<String>()));
        match deux {
            Some((premiere, seconde)) if caracteres(&seconde) <= Self::LIGNE => Ok((premiere, seconde)),
            _ => Err(format!(
                "« {} » ne tient pas en deux lignes de {} caractères coupées entre deux mots, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Nom ou raison sociale », lignes 1 et 2). Abrégez le mot le plus long, ou la fin",
                texte,
                Self::LIGNE
            )),
        }
    }

    /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
    pub fn faute(texte: &str) -> Option<String> {
        let n = caracteres(texte);
        if n > Self::LONGUEUR {
            return Some(format!(
                "« {} » a {} caractères : une raison sociale déclarée en a {} au plus, ce que la DSN écrit (S10.G00.01.003). \
Abrégez-la",
                texte,
                n,
                Self::LONGUEUR
            ));
        }
        Self::en_deux_lignes(texte).err()
    }
}

texte_borne!(@commun RaisonSociale, "caractères");

/// L'adresse d'un document en une ligne : la voie, puis son complement quand la fiche le dit (`LigneAdresse`).
pub fn avec_complement(adresse: String, complement: Option<&str>) -> String {
    match complement.filter(|c| !c.trim().is_empty()) {
        Some(c) => format!("{}, {}", adresse, c),
        None => adresse,
    }
}

// ------------------------------------------------------------ la forme declaree

/// CE QU'UNE CASE DECLARE, pour le test qui la confronte a `champs-base.json` : ses valeurs (un choix) et ses cases (une
/// structure) ; pour la saisie, la faute que son type refuse (`FormeDeCase::FAUTE`).
#[derive(Debug, Clone)]
pub struct Declaree {
    pub cle: &'static str,
    pub valeurs: &'static [&'static str],
    pub cases: Vec<Declaree>,
    pub faute: Option<fn(&str) -> Option<String>>,
    /// La borne de son texte et ce qu'elle compte (`texte_borne!`), que l'aide de sa ligne de `champs-base.json` dit.
    pub longueur: Option<(usize, &'static str)>,
}

/// La forme d'un type de case.
pub trait FormeDeCase {
    /// Les valeurs d'un choix ; aucune hors d'un choix.
    const VALEURS: &'static [&'static str] = &[];
    /// Ce que le type refuse d'un texte saisi, en une phrase qui dit quoi corriger (`LigneAdresse`) ; rien d'ordinaire.
    const FAUTE: Option<fn(&str) -> Option<String>> = None;
    /// La borne d'un texte declare (`texte_borne!`) et ce qu'elle compte ; aucune d'ordinaire.
    const LONGUEUR: Option<(usize, &'static str)> = None;
    /// Les cases d'une structure ; aucune hors d'une structure.
    fn cases() -> Vec<Declaree> {
        Vec::new()
    }
}

pub fn declaree<T: FormeDeCase>(cle: &'static str) -> Declaree {
    Declaree { cle: cle.strip_prefix("r#").unwrap_or(cle), valeurs: T::VALEURS, cases: T::cases(), faute: T::FAUTE, longueur: T::LONGUEUR }
}

impl FormeDeCase for String {}
impl FormeDeCase for bool {}
impl FormeDeCase for i64 {}
impl FormeDeCase for Jour {}
impl FormeDeCase for Mois {}
impl FormeDeCase for Montant {}
impl<T: FormeDeCase> FormeDeCase for Vec<T> {
    const VALEURS: &'static [&'static str] = T::VALEURS;
    fn cases() -> Vec<Declaree> {
        T::cases()
    }
}

/// UN CHOIX (`forme: "choix"` dans `champs-base.json`) : une valeur de la liste, ecrite telle quelle. La ligne vide d'une
/// liste est une case vide (`Case::Vide`), pas une valeur.
macro_rules! choix {
    ($(#[$m:meta])* pub enum $nom:ident { $( $(#[$vm:meta])* $var:ident = $txt:literal ),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $nom { $( $(#[$vm])* #[serde(rename = $txt)] $var ),* }
        impl $nom {
            pub fn as_str(&self) -> &'static str {
                match self { $( $nom::$var => $txt ),* }
            }
        }
        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl FormeDeCase for $nom {
            const VALEURS: &'static [&'static str] = &[$($txt),*];
        }
        impl Choix for $nom {
            fn code(&self) -> &'static str {
                self.as_str()
            }
        }
    };
}

/// CE QUI REGARDE LES CASES D'UNE FICHE PAR LEUR NOM, celui que `champs-base.json` donne a chacune (une table qui les
/// nomme, un bareme, l'import) : chacune lui est donnee typee (`Cases::voir`), qu'il ecrit comme il l'entend.
pub trait VoirCase {
    fn case<T: Serialize>(&mut self, cle: &'static str, case: &Case<T>);
}

/// UNE FICHE QUI SE MONTRE CASE PAR CASE (`cases!`) a qui la regarde (`VoirCase`).
pub trait Cases {
    fn voir<V: VoirCase>(&self, v: &mut V);
}

/// LA CASE `cle` DE `fiche`, ecrite par `s` (une table qui la confronte a ses valeurs l'ecrit dans sa forme) ; `None` :
/// elle est absente, ou la forme ne la declare pas.
pub fn case_en<F: Cases + ?Sized, S: Serializer>(fiche: &F, cle: &str, s: S) -> Option<Result<S::Ok, S::Error>> {
    struct Une<'a, S: Serializer> {
        cle: &'a str,
        s: Option<S>,
        ecrite: Option<Result<S::Ok, S::Error>>,
    }
    impl<S: Serializer> VoirCase for Une<'_, S> {
        fn case<T: Serialize>(&mut self, cle: &'static str, case: &Case<T>) {
            if cle == self.cle && !case.est_absente() {
                if let Some(s) = self.s.take() {
                    self.ecrite = Some(case.serialize(s));
                }
            }
        }
    }
    let mut une = Une { cle, s: Some(s), ecrite: None };
    fiche.voir(&mut une);
    une.ecrite
}

/// UNE STRUCTURE DE CASES : chacune une `Case`, une cle inconnue refusee. Ses `notes` (d'une fiche datee) ne sont pas des
/// cases : des textes qui ne se datent pas, que la fiche porte tels qu'ecrits.
macro_rules! cases {
    ($(#[$m:meta])* pub struct $nom:ident { $( $(#[$fm:meta])* $champ:ident : $t:ty ),* $(,)? } $( notes { $( $(#[$nm:meta])* $note:ident ),* $(,)? } )?) => {
        $(#[$m])*
        #[allow(non_snake_case)] // `association_1679A`, la cle ecrite
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $( $(#[$fm])* #[serde(default, skip_serializing_if = "Case::est_absente")] pub $champ: Case<$t>, )*
            $( $( $(#[$nm])* #[serde(default, skip_serializing_if = "Option::is_none")] pub $note: Option<String>, )* )?
        }
        impl FormeDeCase for $nom {
            fn cases() -> Vec<Declaree> {
                vec![$(declaree::<$t>(stringify!($champ))),*]
            }
        }
        impl $nom {
            /// Les cles de ses notes : des textes qui ne sont pas des cases.
            pub const NOTES: &'static [&'static str] = &[$($( stringify!($note) ),*)?];

            /// Aucune case n'y est ecrite.
            pub fn est_vide(&self) -> bool {
                true $(&& self.$champ.est_absente())*
            }

            /// Les cases `cles` (leurs noms) sont-elles les memes dans `autre` ?
            pub fn memes_cases(&self, autre: &$nom, cles: &[&str]) -> bool {
                $( if cles.contains(&stringify!($champ).trim_start_matches("r#")) && self.$champ != autre.$champ {
                    return false;
                } )*
                true
            }

            /// Les cases que `autre` dit (ni absentes) remplacent les siennes ; ses notes restent.
            pub fn par_dessus(&mut self, autre: &$nom) {
                $( if !autre.$champ.est_absente() {
                    self.$champ = autre.$champ.clone();
                } )*
            }
        }
        impl Cases for $nom {
            fn voir<V: VoirCase>(&self, v: &mut V) {
                $( v.case(stringify!($champ).trim_start_matches("r#"), &self.$champ); )*
            }
        }
    };
}

/// UNE FICHE DATEE : ses champs a un jour (`$plate`, des `cases!`), et la fiche telle qu'ecrite (`$datee`), chaque champ
/// une liste datee ; `au` lit l'une dans l'autre, la valeur en vigueur de chaque champ (`valeur_au`).
macro_rules! fiche {
    ($(#[$m:meta])* pub struct $plate:ident, datee $datee:ident { $( $(#[$fm:meta])* $champ:ident : $t:ty ),* $(,)? } $( notes { $( $(#[$nm:meta])* $note:ident ),* $(,)? } )?) => {
        cases! { $(#[$m])* pub struct $plate { $( $(#[$fm])* $champ : $t ),* } $( notes { $( $(#[$nm])* $note ),* } )? }

        /// La fiche telle qu'ecrite : chaque champ une liste datee.
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $datee {
            $( #[serde(default, skip_serializing_if = "Vec::is_empty")] pub $champ: Vec<Datee<$t>>, )*
            $( $( $(#[$nm])* #[serde(default, skip_serializing_if = "Option::is_none")] pub $note: Option<String>, )* )?
        }

        impl $datee {
            /// Ses champs en vigueur a `jour`, ses notes telles qu'ecrites.
            pub fn au(&self, jour: &Jour) -> $plate {
                $plate { $( $champ: valeur_au(&self.$champ, jour), )* $( $( $note: self.$note.clone(), )* )? }
            }

            /// Les jours ou le champ `cle` prend une valeur, du plus ancien au plus recent (`None` : la valeur de tout
            /// temps, d'abord) ; aucun pour un champ que la fiche ne dit pas, ou que la forme ne declare pas.
            pub fn depuis(&self, cle: &str) -> Vec<Option<Jour>> {
                $( if stringify!($champ).trim_start_matches("r#") == cle {
                    let mut j: Vec<Option<Jour>> = self.$champ.iter().map(|e| e.depuis).collect();
                    j.sort();
                    return j;
                } )*
                Vec::new()
            }

            /// Le `depuis` de la valeur en vigueur a `jour` de chaque champ ; un champ dont seule la valeur de tout temps
            /// s'applique n'en donne pas.
            pub fn dates_en_vigueur(&self, jour: &Jour) -> Vec<Jour> {
                let mut out = Vec::new();
                $( out.extend(self.$champ.iter().filter_map(|e| e.depuis).filter(|j| j <= jour).max()); )*
                out
            }

            /// Les listes et les notes que `autre` porte remplacent les siennes, entieres ; les autres restent.
            pub fn par_dessus(&mut self, autre: &$datee) {
                $( if !autre.$champ.is_empty() {
                    self.$champ = autre.$champ.clone();
                } )*
                $( $( if autre.$note.is_some() {
                    self.$note = autre.$note.clone();
                } )* )?
            }
        }

        impl FicheDatee for $datee {
            type Plate = $plate;

            fn au(&self, jour: &Jour) -> $plate {
                $datee::au(self, jour)
            }

            fn depuis(&self, cle: &str) -> Vec<Option<Jour>> {
                $datee::depuis(self, cle)
            }

            fn dater(&mut self, cle: &str, plate: &$plate, depuis: Option<&Jour>, note: Option<&str>) -> bool {
                $( if stringify!($champ).trim_start_matches("r#") == cle {
                    if plate.$champ.est_absente() {
                        return false;
                    }
                    dater(&mut self.$champ, plate.$champ.clone(), depuis, note);
                    return true;
                } )*
                false
            }

            fn noter(&mut self, plate: &$plate) {
                $( $( if plate.$note.is_some() {
                    self.$note = plate.$note.clone();
                } )* )?
                let _ = plate;
            }

            fn neuve(plate: &$plate) -> $datee {
                let mut f = $datee::default();
                $( if !plate.$champ.est_absente() {
                    dater(&mut f.$champ, plate.$champ.clone(), None, None);
                } )*
                f.noter(plate);
                f
            }

            fn voir_listes<V: VoirListe>(&self, v: &mut V) {
                $( v.liste(stringify!($champ).trim_start_matches("r#"), &self.$champ); )*
            }
        }
    };
}

/// UNE FICHE DATEE (`fiche!`) : la fiche telle qu'ecrite, chaque champ une liste datee, et sa fiche a un jour
/// (`Plate`). Elle s'ecrit par `dater` (une case de `Plate`, qui vaut a partir de `depuis`), ses notes par `noter`.
pub trait FicheDatee: Clone + Default + PartialEq + Serialize + DeserializeOwned {
    type Plate: Cases + Clone + Default + Serialize + DeserializeOwned;

    /// Ses champs en vigueur a `jour`, ses notes telles qu'ecrites.
    fn au(&self, jour: &Jour) -> Self::Plate;

    /// Les jours ou le champ `cle` prend une valeur, du plus ancien au plus recent (`None` d'abord) ; aucun pour un champ
    /// que la fiche ne dit pas, ou que la forme ne declare pas.
    fn depuis(&self, cle: &str) -> Vec<Option<Jour>>;

    /// LA CASE `cle` DE `plate` S'ECRIT DATEE (`dater`) ; faux : la forme ne la declare pas, ou `plate` ne la dit pas.
    fn dater(&mut self, cle: &str, plate: &Self::Plate, depuis: Option<&Jour>, note: Option<&str>) -> bool;

    /// Les notes que `plate` porte remplacent les siennes ; les autres restent.
    fn noter(&mut self, plate: &Self::Plate);

    /// Une fiche neuve : chaque case dite de `plate` vaut de tout temps, ses notes telles quelles.
    fn neuve(plate: &Self::Plate) -> Self;

    /// Chaque liste datee, sous son nom, a qui la regarde.
    fn voir_listes<V: VoirListe>(&self, v: &mut V);
}

/// CE QUI REGARDE LES LISTES DATEES D'UNE FICHE PAR LEUR NOM (l'historique que l'ecran montre, les gardes qui cherchent
/// ce qui nomme une personne) : chacune lui est donnee typee (`FicheDatee::voir_listes`).
pub trait VoirListe {
    fn liste<T: Serialize>(&mut self, cle: &'static str, liste: &[Datee<T>]);
}

/// UNE VALEUR DATEE : de tout temps sans `depuis`, ou a partir de ce jour ; `note` dit d'ou elle vient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: DeserializeOwned"))]
pub struct Datee<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depuis: Option<Jour>,
    pub valeur: Case<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// La valeur en vigueur a `jour` : celle dont le `depuis` est le plus recent sans le depasser (a meme date, la derniere
/// ecrite ; sans `depuis`, de tout temps). Avant la premiere, la case est absente.
pub fn valeur_au<T: Clone>(liste: &[Datee<T>], jour: &Jour) -> Case<T> {
    let mut entrees: Vec<&Datee<T>> = liste.iter().collect();
    entrees.sort_by_key(|e| e.depuis);
    entrees.into_iter().filter(|e| e.depuis.map_or(true, |j| j <= *jour)).last().map_or(Case::Absente, |e| e.valeur.clone())
}

/// LA CONFIGURATION DE LA COMPAGNIE EST DATEE, comme les baremes : chaque valeur d'une fiche (l'entreprise et sa
/// convention, ses etablissements, un salarie, un spectacle, les organismes, les rubriques de la compagnie) s'ecrit en
/// liste datee (`Datee`) et se lit a la date de la periode payee (`valeur_au`), jamais « telle qu'aujourd'hui » : une
/// valeur changee a une date ne touche pas ce qui precede. La premiere valeur d'un champ s'ecrit sans `depuis` : elle
/// vaut DE TOUT TEMPS, jusqu'a la suivante (un contrat commence avant que la fiche soit saisie, un mois repris d'un
/// autre logiciel trouvent la fiche). Un champ dont toutes les valeurs sont datees ne dit rien avant la premiere.
///
/// ECRIRE UNE VALEUR DATEE, la seule fonction qui le fait : `valeur` vaut a partir de `depuis` (`None` : de tout temps) ;
/// ce qui precede reste ce qu'il etait. La premiere valeur d'une liste vaut de tout temps, quel que soit `depuis` : avant
/// elle, la fiche ne disait rien d'autre. Une valeur deja ecrite au meme jour est remplacee (une correction du meme jour
/// d'effet). `note` dit pourquoi.
pub fn dater<T>(liste: &mut Vec<Datee<T>>, valeur: Case<T>, depuis: Option<&Jour>, note: Option<&str>) {
    let depuis = if liste.is_empty() { None } else { depuis.copied() };
    liste.retain(|e| e.depuis != depuis);
    liste.push(Datee { depuis, valeur, note: note.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string) });
    liste.sort_by_key(|e| e.depuis); // stable : a meme date, la derniere ecrite gagne
}

/// LES VALEURS D'UNE LISTE DATEE, de la plus ancienne a la plus recente : `None` pour celle de tout temps (a meme date,
/// dans l'ordre ecrit).
pub fn valeurs_datees<T>(liste: &[Datee<T>]) -> Vec<&Datee<T>> {
    let mut out: Vec<&Datee<T>> = liste.iter().collect();
    out.sort_by_key(|e| e.depuis);
    out
}

// ------------------------------------------------------------------ les choix

choix! {
    /// Le sexe du NIR (S21.G00.30.005).
    pub enum Sexe { Masculin = "1", Feminin = "2" }
}

choix! {
    /// S21.G00.30.007.
    pub enum CodeNationalite { Francaise = "01", UnionEuropeenne = "02", HorsUnionEuropeenne = "03", Inconnue = "04" }
}

choix! {
    pub enum Statut { Artiste = "artiste", Technicien = "technicien" }
}

choix! {
    /// S21.G00.40.002.
    pub enum StatutConventionnel { Cadre03 = "03", Cadre04 = "04", AgentDeMaitrise = "05", Employe06 = "06", Employe07 = "07" }
}

choix! {
    /// S21.G00.40.003.
    pub enum StatutCategorielRc { Cadre = "01", Extension = "02", NonCadre = "04" }
}

choix! {
    /// La profession de la deduction forfaitaire specifique.
    pub enum ProfessionDfs {
        Musicien = "musicien",
        Choriste = "choriste",
        ChefOrchestre = "chef_orchestre",
        RegisseurTheatre = "regisseur_theatre",
        ArtisteDramatique = "artiste_dramatique",
        ArtisteLyrique = "artiste_lyrique",
        ArtisteChoregraphique = "artiste_choregraphique",
        ArtisteCinematographique = "artiste_cinematographique",
    }
}

choix! {
    pub enum AccordDfs { Contrat = "contrat", ConsentementAnnuel = "consentement_annuel", Refus = "refus" }
}

choix! {
    /// S21.G00.30.026 (le statut BOETH).
    pub enum StatutBoeth {
        S01 = "01", S02 = "02", S03 = "03", S04 = "04", S05 = "05", S06 = "06",
        S07 = "07", S08 = "08", S09 = "09", S10 = "10", S11 = "11", S12 = "12",
    }
}

choix! {
    pub enum StagiaireCcneac { Premiere = "1", Deuxieme = "2" }
}

choix! {
    pub enum CategorieConventionnelle {
        Artiste = "artiste",
        Employe = "employe",
        Technicien = "technicien",
        AgentDeMaitrise = "agent_de_maitrise",
        Cadre = "cadre",
        CadreDeDirection = "cadre_de_direction",
    }
}

choix! {
    pub enum ModeleBulletin { Simplifie = "simplifié", Complet = "complet" }
}

choix! {
    pub enum JourDeLaSemaine {
        Lundi = "lundi", Mardi = "mardi", Mercredi = "mercredi", Jeudi = "jeudi", Vendredi = "vendredi", Samedi = "samedi",
        Dimanche = "dimanche",
    }
}

choix! {
    pub enum AnnexeConvention { A1 = "1", A2 = "2", A3 = "3", A4 = "4", A5 = "5", A6 = "6" }
}

choix! {
    pub enum Secteur2121 { Phonographique = "phonographique", Livre = "livre", Musique = "musique" }
}

choix! {
    pub enum TypeRadio1922 { T1 = "1", T2 = "2", T3 = "3" }
}

choix! {
    pub enum JaugeAnnexe1 { JusquA200 = "<=200", De201A500 = "201-500", PlusDe500 = ">500" }
}

choix! {
    pub enum Subrogation { Oui = "oui" }
}

choix! {
    pub enum JoursFeriesChomes { Tous = "tous", PremierMaiSeulement = "1er mai seulement" }
}

choix! {
    pub enum PlafondTempsPartiel { Proratise = "proratisé", Entier = "entier" }
}

choix! {
    pub enum OptionDfs { AccordCollectif = "accord_collectif", AccordCse = "accord_cse", AccordIndividuel = "accord_individuel" }
}

choix! {
    pub enum RepresentationPersonnel { Aucune = "aucune", Csec = "csec", CseAdherentFnas = "cse_adherent_fnas", Cse = "cse" }
}

choix! {
    pub enum SanteSuspension { Suspendue = "suspendue", Maintenue = "maintenue" }
}

choix! {
    /// S21.G00.44 : le motif de non-assujettissement a la taxe d'apprentissage.
    pub enum MotifNonAssujettissement {
        M001 = "001", M002 = "002", M003 = "003", M004 = "004", M005 = "005", M006 = "006", M007 = "007", M008 = "008",
        M009 = "009", M010 = "010", M011 = "011", M012 = "012", M013 = "013", M014 = "014", M015 = "015", M016 = "016",
        M017 = "017", M018 = "018", M019 = "019",
    }
}

choix! {
    pub enum Tva { Assujettie = "assujettie", Exoneree = "exoneree" }
}

choix! {
    pub enum RegimePrevoyance { Contrat = "contrat" }
}

choix! {
    pub enum ExtensionT2 { Tous = "tous", Cadres = "cadres" }
}

choix! {
    /// S20.G00.07.004.
    pub enum Civilite { Monsieur = "01", Madame = "02" }
}

// ------------------------------------------------------------- les structures

cases! {
    /// Le compte bancaire (IBAN, BIC).
    pub struct Banque { iban: String, bic: String }
}

cases! {
    /// La complementaire sante d'un permanent : le niveau souscrit et la composition couverte.
    pub struct SanteSvp { niveau: String, conjoint: bool, enfants: Montant }
}

cases! {
    /// Les conges a la reprise, lus sur le dernier bulletin de l'ancien logiciel.
    pub struct CongesReprise {
        date: Jour,
        report: Montant,
        acquis_report: Montant,
        remuneration_report: Montant,
        acquis: Montant,
        pris: Montant,
        remuneration_periode: Montant,
    }
}

cases! {
    /// UN TAUX DE PAS RECU (`droit::pas`) : le taux, son type, l'identifiant et l'instant de sa mise a disposition
    /// (AAAAMMJJHHMMSS), et le mois de la DSN a laquelle il repond.
    pub struct TauxPas { taux: String, r#type: String, identifiant: String, mis_a_disposition: String, en_retour_de: Mois }
}

cases! {
    /// Le contact de la medecine du travail interne (S20.G00.07).
    pub struct ContactSanteTravail { nom_prenom_affichage: String, nom_prenom: String, tel: String, email: String }
}

cases! {
    pub struct TauxPrevoyance { t1_non_cadre: Montant, t1_cadre: Montant, part_patronale: Montant }
}

cases! {
    /// La prevoyance des permanents, quand un contrat d'entreprise la fixe.
    pub struct PrevoyancePermanents { regime: RegimePrevoyance, taux: TauxPrevoyance, extension_t2: ExtensionT2, adhesion: String }
}

cases! {
    pub struct SantePermanents { taux_contrat: Montant, part_patronale: Montant }
}

cases! {
    /// Le bloc S21.G00.44 : la compagnie doit-elle la taxe sur les salaires.
    pub struct TaxeSurLesSalaires { tva: Tva, part_tva: Montant, sous_franchise: bool, association_1679A: bool }
}

cases! {
    /// Le taux AT/MP d'un etablissement, et l'annee qu'il vaut (D. 242-6-15 : celle de sa date d'effet), que l'ecran pose
    /// (`routes::etablissements`).
    pub struct TauxAtmp { code_risque: String, valeur: Montant, annee: i64 }
}

cases! {
    pub struct VersementMobilite { taux: Montant, code_insee: String }
}

cases! {
    /// UN AUTRE ETABLISSEMENT que le siege : son SIRET (et le NIC qui s'en lit), son adresse (et la forme declaree qui se
    /// fabrique de la lisible), son ouverture, son taux AT/MP et son versement mobilite.
    pub struct Etablissement {
        libelle: String,
        siret: String,
        nic: String,
        adresse_affichage: String,
        adresse: LigneAdresse,
        complement_adresse_affichage: String,
        complement_adresse: LigneAdresse,
        code_postal: CodePostal,
        commune_affichage: String,
        commune: CommuneEmployeur,
        ouverture: Jour,
        taux_atmp: TauxAtmp,
        versement_mobilite: VersementMobilite,
    }
}

cases! {
    pub struct Signataire {
        prenom_affichage: String,
        nom_affichage: String,
        prenom: String,
        nom: String,
        fonction: String,
        tel: Telephone,
        email: String,
    }
}

cases! {
    /// Le contact DSN (S20.G00.07).
    pub struct DeclarantDsn { civilite: Civilite, nom_prenom_affichage: String, nom_prenom: String, tel: Telephone, email: String, qualite: String }
}

cases! {
    /// Le declarant inscrit au service DSN de net-entreprises, tel qu'inscrit (`envois::transport::declarant`).
    pub struct Declarant { siret: String, nom: String, prenom: String }
}

cases! {
    /// Le compte DPAE d'urssaf.fr (`envois::releve_retours`) : son titulaire, son SIRET (celui de l'entreprise sinon), son
    /// service.
    pub struct ApiDpae { nom: String, prenom: String, siret: String, service: String }
}

// ------------------------------------------------------------------ les fiches

fiche! {
    /// LA FICHE D'UN SALARIE a un jour.
    pub struct ChampsSalarie, datee FicheSalarieDatee {
        code_salarie: String,
        initiales_contrat: String,
        nom_affichage: String,
        nom: NomDeclare,
        prenom_affichage: String,
        prenom: PrenomDeclare,
        pseudonyme: String,
        nir: Nir,
        nir_cle: String,
        sexe: Sexe,
        date_naissance: Jour,
        lieu_naissance: LieuNaissance,
        lieu_naissance_affichage: String,
        departement_naissance: DepartementNaissance,
        pays_naissance: String,
        code_nationalite_dsn: CodeNationalite,
        pays_nationalite: String,
        pays_nationalite_hors_liste: String,
        adresse: String,
        adresse_affichage: String,
        code_postal: String,
        commune: String,
        commune_affichage: String,
        email: String,
        banque: Banque,
        num_conges_spectacles: String,
        emploi_affichage: String,
        emploi: String,
        pcs_ese: String,
        code_emploi_ft: String,
        statut: Statut,
        statut_conventionnel: StatutConventionnel,
        statut_categoriel_rc: StatutCategorielRc,
        deduction_forfaitaire: ProfessionDfs,
        deduction_forfaitaire_accord: AccordDfs,
        statut_boeth: StatutBoeth,
        sante_svp: SanteSvp,
        echelon_ccneac: String,
        classement_conventionnel: String,
        poste_depuis: Jour,
        stagiaire_ccneac: StagiaireCcneac,
        experimente_3241: bool,
        jours_travailles: Vec<JourDeLaSemaine>,
        forfait_jours: bool,
        sante_dispense: String,
        sante_dispense_depuis: Jour,
        sante_dispense_jusqu_au: Jour,
        prevoyance_branche_depuis: Jour,
        categorie_conventionnelle: CategorieConventionnelle,
        conges_reprise: CongesReprise,
        date_anciennete: Jour,
        actif: bool,
        anniversaire_droits: Jour,
        modele_bulletin: ModeleBulletin,
        taux_pas: TauxPas,
    }
}

fiche! {
    /// LA FICHE DE L'ENTREPRISE a un jour (son siege et, en `etablissements`, les autres).
    pub struct ChampsEntreprise, datee FicheEntrepriseDatee {
        nom_affichage: String,
        raison_sociale: RaisonSociale,
        siren: String,
        nic: String,
        siret: String,
        naf: CodeNaf,
        code_urssaf: String,
        adresse: LigneAdresse,
        adresse_affichage: String,
        complement_adresse: LigneAdresse,
        complement_adresse_affichage: String,
        code_postal: CodePostal,
        commune: CommuneEmployeur,
        commune_affichage: String,
        idcc: String,
        annexe_convention: AnnexeConvention,
        secteur_2121: Secteur2121,
        type_radio_1922: TypeRadio1922,
        cinquante_salaries_3241: bool,
        jauge_annexe_1: JaugeAnnexe1,
        subrogation_ij: Subrogation,
        urssaf_compte: String,
        medecine_travail: CodeMedecine,
        contact_sante_travail: ContactSanteTravail,
        licence_spectacle: String,
        france_travail_dossier_recouvrement: String,
        congesspectacles_adherent: String,
        affiliation_france_travail: String,
        retraite_complementaire: String,
        organisation_patronale: String,
        organisation_patronale_depuis: Jour,
        emploie_des_permanents: bool,
        date_de_versement_permanents: String,
        date_de_versement_intermittents: String,
        prevoyance_permanents: PrevoyancePermanents,
        jours_feries_chomes: JoursFeriesChomes,
        plafond_temps_partiel: PlafondTempsPartiel,
        deduction_forfaitaire_option: OptionDfs,
        representation_personnel: RepresentationPersonnel,
        casc_svp_exemptee: bool,
        sante_permanents: SantePermanents,
        sante_suspension_non_indemnisee: SanteSuspension,
        premiere_embauche: Jour,
        taxe_sur_les_salaires: TaxeSurLesSalaires,
        taxe_apprentissage_motif_non_assujettissement: MotifNonAssujettissement,
        cinquante_salaries_et_plus: bool,
        etablissements: Vec<Etablissement>,
        signataire: Signataire,
        declarant_dsn: DeclarantDsn,
        declarant: Declarant,
        api_dpae: ApiDpae,
        banque: Banque,
        courriel_nom: String,
        courriel_signature: String,
        modele_bulletin: ModeleBulletin,
        clauses_particulieres: String,
        conseils: bool,
    }
    notes {
        /// D'ou vient le fichier (une compagnie d'essai le dit).
        note,
    }
}

choix! {
    /// Le genre d'un programme audiovisuel (2642) : les minima de ses techniciens en dependent.
    pub enum GenreAudiovisuel { Fiction = "fiction", Flux = "flux", Autre = "autre" }
}

choix! {
    /// Le format d'un film (3097) : les minima de ses comediens en dependent.
    pub enum FormatCinema { Long = "long", Court = "court" }
}

fiche! {
    /// LA FICHE D'UN SPECTACLE a un jour (`config/spectacles.json`). Ses notes, ecrites par l'import, disent d'ou il
    /// vient et le titre que les bulletins de l'ancien logiciel proposent (`declarations::demarrage`).
    pub struct ChampsSpectacle, datee FicheSpectacleDatee {
        titre: String,
        numero_objet: String,
        code_analytique: String,
        genre_audiovisuel: GenreAudiovisuel,
        emission_tv_1734: bool,
        format_cinema: FormatCinema,
        derogation_annexe_iii_3097: bool,
        alias: Vec<String>,
        actif: bool,
    }
    notes {
        /// D'ou vient la fiche (ce que l'import a lu).
        note,
        titre_propose,
        titre_lu_dans,
    }
}

impl FormeDeCase for crate::catalogue::PlafondAnnuelHeures {}

fiche! {
    /// UNE RUBRIQUE TELLE QU'UN FICHIER L'ECRIT, case par case : livree (`rubriques-base.json`, de tout temps), ou la
    /// compagnie par-dessus, case par case, datee comme toute la configuration (`config/rubriques-compagnie.json`).
    /// `catalogue::construire` en fait la `Rubrique` du calcul.
    pub struct ChampsRubrique, datee RubriqueDatee {
        libelle: String,
        unite: String,
        nature: String,
        saisie: Vec<String>,
        soumis: bool,
        imposable: bool,
        conges_spectacles: bool,
        source: String,
        sens: String,
        heures_forfait: Montant,
        plafond: String,
        aem: String,
        dsn_type: String,
        dsn_autres_bruts: String,
        annexe: String,
        plafond_mensuel: i64,
        plafond_annuel_heures: crate::catalogue::PlafondAnnuelHeures,
        compte_pour: String,
        salaire_de_base: bool,
        retabli: bool,
        suspension: bool,
        arret_de_travail: bool,
        conges_pris: bool,
        dsn_bloc52: String,
        dit: String,
    }
}

impl ChampsSpectacle {
    /// Archivee : `actif: false`. L'absence vaut actif.
    pub fn archivee(&self) -> bool {
        self.actif.dite() == Some(&false)
    }
}

/// UN SPECTACLE TROUVE (`contexte::Contexte::spectacle`) : sa cle dans `config/spectacles.json`, et sa fiche au jour.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectacle {
    pub cle: String,
    pub champs: ChampsSpectacle,
}

impl std::ops::Deref for Spectacle {
    type Target = ChampsSpectacle;
    fn deref(&self) -> &ChampsSpectacle {
        &self.champs
    }
}

impl ChampsSalarie {
    /// « NOM Prénom », tel que l'ecran le nomme (`socle::texte::salarie_nomme`) : le nom et le prenom d'usage, sinon ceux de
    /// l'etat civil. `None` : la fiche ne nomme personne.
    pub fn nomme(&self) -> Option<String> {
        socle::texte::salarie_nomme(self.nom.texte(), self.nom_affichage.texte(), self.prenom.texte(), self.prenom_affichage.texte())
    }

    /// Ce que la fiche dit de la nationalite (`socle::codification_ue`, S21.G00.30.013).
    pub fn nationalite(&self) -> socle::codification_ue::Nationalite {
        socle::codification_ue::Nationalite::nouvelle(
            self.code_nationalite_dsn.code(),
            self.pays_nationalite.texte(),
            self.pays_nationalite_hors_liste.texte(),
            self.nomme(),
        )
    }

    /// Le pays de naissance tel que la DSN le declare (S21.G00.30.015, `socle::pays::de_naissance`).
    pub fn pays_de_naissance(&self) -> Resultat<Case<String>> {
        let qui = format!("{} {}", self.prenom.texte(), self.nom.texte()).trim().to_string();
        Ok(match &self.pays_naissance {
            Case::Dite(code) => Case::Dite(socle::pays::de_naissance(code, &qui)?),
            autre => autre.clone(),
        })
    }

    /// Archivee : `actif: false`. L'absence vaut actif.
    pub fn archivee(&self) -> bool {
        self.actif.dite() == Some(&false)
    }
}

impl ChampsEntreprise {
    /// Le NIC du siege, ou `""`.
    pub fn nic_principal(&self) -> String {
        self.nic.texte().to_string()
    }

    /// Les etablissements autres que le siege.
    pub fn secondaires(&self) -> Vec<&Etablissement> {
        let nic = self.nic.texte();
        self.etablissements.dite().into_iter().flatten().filter(|e| e.nic.texte() != nic).collect()
    }

    /// L'autre etablissement de ce NIC, `None` : le siege, ou un NIC que la fiche ne porte pas.
    pub fn etablissement(&self, nic: &str) -> Option<&Etablissement> {
        self.secondaires().into_iter().find(|e| e.nic.texte() == nic)
    }

    /// Le NIC du siege, puis celui de chaque autre etablissement : une DSN par etablissement.
    pub fn nics(&self) -> Vec<String> {
        std::iter::once(self.nic_principal()).chain(self.secondaires().iter().map(|e| e.nic.texte().to_string())).collect()
    }

    /// Les feries qu'elle chome, tels que `socle::feries::chomes` les lit ; `None` : la fiche n'en dit rien.
    pub fn feries_chomes(&self) -> Option<&'static str> {
        self.jours_feries_chomes.dite().map(Choix::code)
    }

    /// Le nom de la compagnie tel que l'ecran le dit : le nom d'usage, sinon la raison sociale.
    pub fn nommee(&self) -> String {
        match self.nom_affichage.texte() {
            "" => self.raison_sociale.texte().to_string(),
            n => n.to_string(),
        }
    }
}

/// LA FICHE D'UN SALARIE A UN JOUR, avec sa cle : ce que lit tout ce qui le paie ou le declare.
#[derive(Debug, Clone, PartialEq)]
pub struct FicheSalarie {
    pub id: IdSalarie,
    pub champs: ChampsSalarie,
}

impl std::ops::Deref for FicheSalarie {
    type Target = ChampsSalarie;
    fn deref(&self) -> &ChampsSalarie {
        &self.champs
    }
}

impl std::ops::DerefMut for FicheSalarie {
    fn deref_mut(&mut self) -> &mut ChampsSalarie {
        &mut self.champs
    }
}

/// LA FICHE QUE LA PAIE D'UN CONTRAT LIT : celle du salarie a la date de la periode, telle que l'engagement la voit (son
/// emploi l'emporte, `emplois::emploi_de`), avec ce que le contrat y ajoute : la convention de l'employeur
/// (`caisse_conges::intermittent`), l'etablissement de rattachement (S21.G00.11.001), et la sante suspendue d'un mois
/// sans indemnisation (`bareme::sans_sante`). Elle s'ecrit a cote de la paie (`engagement::relire::FichesDuCalcul`)
/// par sa forme (`Figee`) et se relit telle quelle (`lire`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Figee", into = "Figee")]
pub struct SalarieDuContrat {
    pub fiche: FicheSalarie,
    pub idcc_de_l_employeur: Option<String>,
    pub etablissement: Option<String>,
    pub sante_suspendue: Option<String>,
}

/// La forme ecrite de `SalarieDuContrat` : la cle du salarie, ses champs, et ce que le contrat y ajoute.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Figee {
    cle: IdSalarie,
    champs: ChampsSalarie,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    idcc_de_l_employeur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etablissement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sante_suspendue: Option<String>,
}

impl From<Figee> for SalarieDuContrat {
    fn from(f: Figee) -> SalarieDuContrat {
        SalarieDuContrat { fiche: FicheSalarie { id: f.cle, champs: f.champs }, idcc_de_l_employeur: f.idcc_de_l_employeur, etablissement: f.etablissement, sante_suspendue: f.sante_suspendue }
    }
}

impl From<SalarieDuContrat> for Figee {
    fn from(s: SalarieDuContrat) -> Figee {
        Figee { cle: s.fiche.id, champs: s.fiche.champs, idcc_de_l_employeur: s.idcc_de_l_employeur, etablissement: s.etablissement, sante_suspendue: s.sante_suspendue }
    }
}

impl std::ops::Deref for SalarieDuContrat {
    type Target = FicheSalarie;
    fn deref(&self) -> &FicheSalarie {
        &self.fiche
    }
}

impl std::ops::DerefMut for SalarieDuContrat {
    fn deref_mut(&mut self) -> &mut FicheSalarie {
        &mut self.fiche
    }
}

impl SalarieDuContrat {
    /// La fiche, sans rien que le contrat ajoute.
    pub fn de(fiche: FicheSalarie) -> SalarieDuContrat {
        SalarieDuContrat { fiche, idcc_de_l_employeur: None, etablissement: None, sante_suspendue: None }
    }
}

// ------------------------------------------------------------- les fichiers

/// La note d'un fichier de fiches.
pub const NOTE_DU_FICHIER: &str = "note";

/// Cette cle est-elle une note d'une fiche de la configuration (`notes` de `ChampsEntreprise` et de `ChampsSpectacle`) :
/// un texte qui n'est pas une case, que l'ecran ne montre pas et qu'une ecriture garde.
pub fn est_une_note(cle: &str) -> bool {
    ChampsEntreprise::NOTES.contains(&cle) || ChampsSpectacle::NOTES.contains(&cle)
}

/// UN FICHIER DE FICHES (`config/salaries.json`, `config/spectacles.json`) : sa `note`, et ses fiches par cle (`K`), dans
/// l'ordre du fichier. Une cle repetee se refuse (`socle::forme::fiches_et_note`).
#[derive(Debug, Clone, PartialEq)]
pub struct FichierDeFiches<K: std::hash::Hash + Eq, F> {
    pub source: Option<String>,
    pub fiches: IndexMap<K, F>,
}

impl<K: std::hash::Hash + Eq, F> Default for FichierDeFiches<K, F> {
    fn default() -> Self {
        FichierDeFiches { source: None, fiches: IndexMap::new() }
    }
}

impl<'de, K: std::hash::Hash + Eq + Deserialize<'de>, F: Deserialize<'de>> Deserialize<'de> for FichierDeFiches<K, F> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (source, fiches) = socle::forme::fiches_et_note(d, NOTE_DU_FICHIER)?;
        Ok(FichierDeFiches { source, fiches: fiches.into_iter().collect() })
    }
}

impl<K: std::hash::Hash + Eq + Serialize, F: Serialize> Serialize for FichierDeFiches<K, F> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.fiches.len() + usize::from(self.source.is_some())))?;
        if let Some(n) = &self.source {
            m.serialize_entry(NOTE_DU_FICHIER, n)?;
        }
        for (k, f) in &self.fiches {
            m.serialize_entry(k, f)?;
        }
        m.end()
    }
}

/// `config/salaries.json`.
pub type Salaries = FichierDeFiches<IdSalarie, FicheSalarieDatee>;

/// `config/spectacles.json`.
pub type Spectacles = FichierDeFiches<String, FicheSpectacleDatee>;

/// LA LECTURE D'UN FICHIER DE FICHES (`config/entreprise.json` en `FicheEntrepriseDatee`, `config/salaries.json` en
/// `Salaries`, `config/spectacles.json` en `Spectacles`), la seule : `fichier` le nomme dans les refus, qui disent la cle
/// fautive jusqu'a sa case et quoi faire.
pub fn lire_fichier<T: DeserializeOwned>(chemin: &std::path::Path, fichier: &str) -> Resultat<T> {
    socle::forme::lire_fichier_en_phrase(chemin, |cle, detail| refus_de_fiche(fichier, cle, detail))
}

/// Le refus d'une fiche qui ne se lit pas : le fichier, la cle, et quoi faire.
fn refus_de_fiche(fichier: &str, chemin: &str, detail: &str) -> String {
    let ou = if chemin.is_empty() { String::new() } else { format!(", à « {} »", chemin) };
    format!(
        "{}{} ne se lit pas : {}. Ce fichier a été écrit par une version précédente du logiciel ou à la main : ajouter la \
compagnie à nouveau (« Ajouter une compagnie », d'un logiciel précédent) ou corriger cette clé dans le fichier.",
        fichier, ou, detail
    )
}

/// Le refus d'une ecriture de fiche que sa forme ne lit pas : rien ne s'ecrit, la valeur se corrige.
fn refus_d_ecriture(fichier: &str, chemin: &str, detail: &str) -> String {
    let ou = if chemin.is_empty() { String::new() } else { format!(", à « {} »", chemin) };
    format!("{}{} ne s'écrirait pas : {}. Rien n'est écrit : corriger cette valeur.", fichier, ou, detail)
}

/// Les champs d'une fiche a un jour (ou toute forme d'une fiche), lus de `v` (une charge, une fiche composee) : le
/// refus nomme la cle jusqu'a sa case.
pub fn lire_champs<T: DeserializeOwned>(v: &(impl Serialize + ?Sized), quoi: &str) -> Resultat<T> {
    socle::forme::lire_en_phrase(&socle::ecriture::en_valeur(v), |chemin, detail| refus_de_fiche(quoi, chemin, detail))
}

/// CE QU'UNE ECRITURE VA POSER DANS LE FICHIER `fichier`, lu par sa forme avant de s'ecrire : ce qui ne se lit pas se
/// refuse en nommant la cle jusqu'a sa case, et rien n'est ecrit.
pub fn lire_saisie<T: DeserializeOwned>(v: &(impl Serialize + ?Sized), fichier: &str) -> Resultat<T> {
    socle::forme::lire_en_phrase(&socle::ecriture::en_valeur(v), |chemin, detail| refus_d_ecriture(fichier, chemin, detail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Une fiche se relit et se reecrit telle quelle : les cases nulles et vides comprises.
    #[test]
    fn une_fiche_se_reecrit_telle_qu_elle_est_lue() {
        let v = json!({
            "note": "note",
            "FULLER": {
                "nom": [{"valeur": "FULLER", "note": "importe"}],
                "actif": [{"valeur": null}, {"depuis": "2026-11-02", "valeur": true}],
                "taux_pas": [{"valeur": ""}, {"depuis": "2026-11-20", "valeur": {"taux": "1.90", "type": "01", "identifiant": "1", "mis_a_disposition": "20261120000000"}}],
                "conges_reprise": [{"valeur": {"date": "2026-01-01", "report": 30, "remuneration_report": 47321.04}}]
            }
        });
        let s = lire_champs::<Salaries>(&v, "config/salaries.json").unwrap();
        assert_eq!(socle::ecriture::en_valeur(&s), v);
        let f = &s.fiches[&IdSalarie::lire("FULLER").unwrap()];
        assert_eq!(f.au(&Jour::lire("2026-11-01").unwrap()).actif, Case::Nulle);
        assert!(f.au(&Jour::lire("2026-11-02").unwrap()).actif.oui());
    }

    /// Une cle inconnue se refuse, et la phrase nomme le fichier et la cle.
    #[test]
    fn une_cle_inconnue_se_refuse_en_la_nommant() {
        let e = lire_champs::<Salaries>(&json!({"FULLER": {"prevoyance_techniciens_3097": [{"valeur": true}]}}), "config/salaries.json").unwrap_err();
        assert!(e.contains("config/salaries.json") && e.contains("prevoyance_techniciens_3097"), "{}", e);
    }
}
