//! `moteur/conges_spectacles.py` : le certificat d'emploi Conges Spectacles.
//!
//! Ce module ne met pas en page : il compose la DESCRIPTION (des blocs a position fixe, relevee
//! sur le certificat Frappaz) que la fabrique dessine. Les coordonnees, les libelles et les cases
//! sont ceux du Python, au point pres. `decouper` coupe un contrat au 31 mars, `certificats` dit
//! ce qui est du, `valeurs` ce qui va dans chaque case.

use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use crate::bulletin::{identite_de, FORMAT};
use engagement::calcul::Paie;
use socle::date::{repr_flottant, Jour};
use engagement::Engagement;
use socle::valeur::valeur_en_texte;
use socle::fichiers::vrai;
use socle::num::{dec, dec_f64, Decimal};
use crate::routes_ouverture::Refus;

type R<T> = Result<T, Refus>;

// ------------------------------------------------- le numero d'adherent, et le code que la DSN en tire

/// La cle de la fiche entreprise : le numero d'adherent a la Caisse des Conges Spectacles, le SEUL numero Conges
/// Spectacles que la compagnie saisit (#236). Il va dans la case COMPTE des certificats, dans les contrats, et la DSN
/// le declare precede de 97 (`code_dsn`).
pub const CLE_ADHERENT: &str = "congesspectacles_adherent";

/// LA FORME ECRITE DU NUMERO D'ADHERENT : la meme valeur, « accolés, sans espaces, ni tiret » et la lettre en capitale,
/// comme l'exemple de la fiche (970000000001A ; `audiens-fiche-specificites-conges-spectacles-20261023.txt`, l. 80-81).
/// Rien ne s'ajoute ni ne se devine : on ote les blancs et les tirets, on met en majuscules. Le seul endroit qui le fait ;
/// `routes_fiches::deriver` l'applique avant de controler et d'enregistrer la fiche, `adherent` avant de declarer.
pub fn forme_adherent(v: &str) -> String {
    v.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(char::to_uppercase).collect()
}

/// LA FORME DU NUMERO D'ADHERENT : « le numéro d'adhérent à la caisse Congés spectacles (10 chiffres + 1 lettre) »
/// (Audiens, fiche de specificites « Parametrage DSN Conges Spectacles »,
/// `atelier/reference/sources/audiens-dsn/audiens-fiche-specificites-conges-spectacles-20261023.txt`, l. 80-81). La
/// lettre s'ecrit en capitale (l'exemple de la fiche : 970000000001A). `None` : la forme est bonne.
pub fn faute_adherent(v: &str) -> Option<String> {
    let c: Vec<char> = v.chars().collect();
    let bon = c.len() == 11 && c[..10].iter().all(char::is_ascii_digit) && c[10].is_ascii_uppercase();
    (!bon).then(|| {
        "le numéro d'adhérent Congés Spectacles s'écrit en dix chiffres suivis d'une lettre \
         (par exemple 0123450001F), tel qu'il figure sur les courriers d'Audiens"
            .to_string()
    })
}

/// Le numero d'adherent de la fiche, de la forme que dit la fiche d'Audiens (`faute_adherent`) ; sinon, refus qui dit
/// quoi saisir.
pub fn adherent(ent: &Value) -> Result<String, String> {
    let v = match ent.get(CLE_ADHERENT) {
        Some(v) if vrai(v) => forme_adherent(&valeur_en_texte(v)),
        _ => {
            return Err("le numéro d'adhérent Congés Spectacles manque dans la fiche de l'entreprise (Entreprise, « Vos \
                        numéros chez les organismes ») : dix chiffres suivis d'une lettre, tel qu'il figure sur les \
                        courriers d'Audiens"
                .to_string())
        }
    };
    match faute_adherent(&v) {
        Some(faute) => Err(format!("fiche de l'entreprise : {}", faute)),
        None => Ok(v),
    }
}

/// LE CODE CONGES SPECTACLES QUE LA DSN DECLARE (S21.G00.40.022), et le seul endroit qui l'ecrit : « 97 puis le numéro
/// d'adhérent à la caisse Congés spectacles (10 chiffres + 1 lettre). Ces 2 identifiants doivent être accolés, sans
/// espaces, ni tiret, etc… EX : 970000000001A » (Audiens, fiche de specificites « Parametrage DSN Conges Spectacles »,
/// `atelier/reference/sources/audiens-dsn/audiens-fiche-specificites-conges-spectacles-20261023.txt`, l. 80-81). La
/// norme dit la meme forme (« Indiquer "97", le numero d'affiliation et la cle », CRE-11 : les deux premiers caracteres
/// dans la table CPCP, ou 97 est la « Caisse de conges spectacles » ; `atelier/reference/norme-dsn/texte/
/// bloc-S21.G00.40.txt`, l. 921-933). Une cle `congesspectacles_affiliation` qu'une fiche porterait encore n'est pas lue.
pub fn code_dsn(ent: &Value) -> Result<String, String> {
    Ok(format!("97{}", adherent(ent)?))
}

const TIMES: &str = "tiro";
const TIMES_GRAS: &str = "tibo";
const COURIER: &str = "cour";

/// Le cadre de la signature : x0, y0, x1, y1.
const SIGNATURE_CADRE: (f64, f64, f64, f64) = (441.84, 775.50, 521.84, 809.10);

/// (x, ligne de base, texte, police, corps) : le fond imprime.
#[rustfmt::skip]
const FOND: &[(f64, f64, &str, &str, f64)] = &[
    (48.96, 441.12, "CERTIFICAT D'EMPLOI", TIMES_GRAS, 10.08),
    (48.96, 453.12, "DESTINÉ AU SALARIÉ", TIMES_GRAS, 10.08),
    (212.16, 441.84, "LES CONGES SPECTACLES", TIMES_GRAS, 13.92),
    (219.36, 453.12, "Audiens TSA 30400 92177 Vanves Cedex", TIMES_GRAS, 10.08),
    (435.12, 441.36, "Information des Intermittents", TIMES, 9.12),
    (456.48, 453.36, "www.audiens.org", TIMES, 9.12),
    (450.48, 474.24, "Tél. : 0 173 173 434", TIMES, 9.12),
    (382.32, 484.32, "de 9h30 à 18 h : accueil sur place et téléphone", TIMES, 9.12),
    (49.92, 504.24,
     "Si vous n'êtes pas inscrit à notre Caisse, contactez-nous à l'adresse ci-dessus mentionnée afin que nous procédions à votre immatriculation.",
     TIMES_GRAS, 7.92),
    (48.00, 513.36,
     "Votre demande de congé doit être adressée chaque année, quinze jours au moins avant votre départ en congé, dûment complétée et signée. Votre formulaire de",
     TIMES, 7.92),
    (48.00, 522.48,
     "demande de congé vous sera adressé automatiquement chaque début d'année en même temps que les sommes à déclarer à l'administration fiscale. Si tel n'est",
     TIMES, 7.92),
    (48.00, 531.83,
     "pas le cas, vous pouvez le demander par courrier ou 24h/24h : sur notre site internet ou notre serveur vocal téléphonique.",
     TIMES, 7.92),
    (48.00, 540.95,
     "La loi n°2004-801 du 6 août 2004 relative à la protection des personnes physiques à l'égard des traitements  de données à caractère personnel s'applique aux",
     TIMES, 7.92),
    (48.00, 550.07,
     "informations portées sur ce certificat. Elle garantit un droit d'accès et de rectification auprès du service Information des Intermittents.",
     TIMES, 7.92),
    (48.00, 579.84, "N° SIRET", TIMES, 13.92),
    (253.92, 579.84, "COMPTE", TIMES, 13.92),
    (431.04, 579.84, "N° CE", TIMES, 13.92),
    (54.96, 614.87, "NIR", TIMES, 13.92),
    (58.08, 633.36, "NOM DE NAISSANCE", TIMES, 7.92),
    (58.08, 643.44, "NOM D'USAGE", TIMES, 7.92),
    (57.84, 653.28, "EMPLOI", TIMES, 7.92),
    (57.84, 663.36, "CADRE", TIMES, 7.92),
    (318.96, 633.36, "PRENOM", TIMES, 7.92),
    (318.96, 643.44, "PSEUDONYME", TIMES, 7.92),
    (157.44, 683.28, " DATES DE TRAVAIL", TIMES, 10.08),
    (396.96, 683.28, "    NB JOURS OU CACHETS", TIMES, 10.08),
    (51.84, 704.88, "DEBUT", TIMES, 13.92),
    (234.96, 704.88, "FIN", TIMES, 13.92),
    (70.80, 737.76, "BASE CONGE", TIMES, 13.92),
    (72.00, 747.36, "Arrondie à l'euro le plus proche", TIMES, 7.92),
    (318.96, 737.76, "SALAIRE BRUT", TIMES, 13.92),
    (318.96, 747.36, "Arrondi à l'euro le plus proche", TIMES, 7.92),
    (354.96, 759.12, "Date :", TIMES, 10.08),
    (354.00, 771.12, "Nom du signataire", TIMES, 10.08),
    (354.96, 782.16, "Signature", TIMES, 10.08),
];

/// Les reperes d'angle, les traits de pliage et les filets : (x1, y1, x2, y2, epaisseur).
#[rustfmt::skip]
const TRAITS: &[(f64, f64, f64, f64, f64)] = &[
    (40.80, 419.28, 63.84, 419.28, 3.12),
    (42.48, 441.84, 42.48, 418.80, 3.12),
    (530.88, 419.28, 553.92, 419.28, 3.12),
    (552.48, 440.88, 552.48, 417.84, 3.12),
    (40.80, 800.40, 63.84, 800.40, 3.12),
    (42.48, 801.84, 42.48, 778.80, 3.12),
    (528.96, 800.40, 552.00, 800.40, 3.12),
    (551.28, 801.84, 551.28, 778.80, 3.12),
    (18.96, 279.84, 38.88, 279.84, 0.24),
    (554.88, 279.84, 574.80, 279.84, 0.24),
    (48.96, 678.96, 148.80, 678.96, 0.24),
    (258.96, 678.96, 358.80, 678.96, 0.24),
];

/// La position de chaque valeur remplie : (champ, x, ligne de base, police, corps).
#[rustfmt::skip]
const CASES: &[(&str, f64, f64, &str, f64)] = &[
    ("dest_nom",       322.80, 157.20, TIMES_GRAS, 10.08),
    ("dest_adresse",   322.80, 173.28, TIMES, 10.08),
    ("dest_commune",   322.80, 189.12, TIMES, 10.08),
    ("dest_pays",      322.80, 221.28, TIMES, 10.08),
    ("siret",          123.60, 579.60, COURIER, 13.92),
    ("compte",         338.40, 579.60, COURIER, 13.92),
    ("num_ce",         503.52, 579.60, COURIER, 13.92),
    ("nir",            112.80, 614.64, COURIER, 13.92),
    ("raison_sociale", 318.96, 614.64, COURIER, 13.92),
    ("nom_naissance",  174.00, 633.36, TIMES, 7.92),
    ("nom_usage",      174.00, 643.44, TIMES, 7.92),
    ("emploi",         174.00, 653.28, TIMES, 7.92),
    ("cadre",          174.00, 663.36, TIMES, 7.92),
    ("prenom",         390.00, 633.36, TIMES, 7.92),
    ("pseudonyme",     390.00, 643.44, TIMES, 7.92),
    ("debut",          126.00, 704.64, COURIER, 13.92),
    ("fin",            266.88, 704.64, COURIER, 13.92),
    ("nb_cachets",     457.44, 704.64, COURIER, 13.92),
    ("base_conge",     248.64, 737.52, COURIER, 13.92),
    ("salaire_brut",   470.64, 737.52, COURIER, 13.92),
    ("date",           441.84, 758.16, COURIER, 10.08),
    ("signataire",     441.84, 771.12, COURIER, 10.08),
    ("agrement",       105.12, 781.68, TIMES, 10.08),
];

/// Le bord droit a ne pas depasser, quand la case a un voisin.
const LIMITES: &[(&str, f64)] = &[
    ("siret", 253.92),
    ("compte", 431.04),
    ("num_ce", 549.00),
    ("nir", 318.96),
    ("raison_sociale", 549.00),
    ("nom_naissance", 318.96),
    ("nom_usage", 318.96),
    ("emploi", 318.96),
    ("cadre", 318.96),
    ("prenom", 549.00),
    ("pseudonyme", 549.00),
    ("debut", 234.96),
    ("fin", 396.96),
    ("nb_cachets", 549.00),
    ("base_conge", 318.96),
    ("salaire_brut", 549.00),
    ("date", 549.00),
    ("signataire", 549.00),
];

const RESPIRATION: f64 = 3.0;
const NOIR: &str = "#000000";

// ---------------------------------------------------------------- chiffres

/// `arrondi_euro` : a l'euro le plus proche, moitie superieure.
pub(crate) fn arrondi_euro(x: Decimal) -> i64 {
    use rust_decimal::prelude::ToPrimitive;
    x.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64().unwrap_or(0)
}

/// `base_conge` : le brut, plafonne au triple du minimum conventionnel quand la fiche en porte un.
fn base_conge(brut: Decimal, salarie: &Value) -> R<Decimal> {
    match salarie.get("minimum_conventionnel_cachet") {
        Some(mini) if vrai(mini) => {
            let m = dec(&Value::String(valeur_en_texte(mini)))? * Decimal::from(3);
            Ok(brut.min(m))
        }
        _ => Ok(brut),
    }
}

// ----------------------------------------------------- decoupage au 31 mars

/// Ce que `decouper` et `certificats` lisent d'un engagement.
#[derive(Debug, Clone)]
pub(crate) struct ContratCs {
    pub debut: Jour,
    pub fin: Jour,
    pub dates_travail: Vec<Jour>,
    pub cachets: i64,
    pub jours: i64,
    pub heures: Option<f64>,
    pub taux_cachet: Option<f64>,
    pub numero: Option<String>,
}

impl ContratCs {
    pub fn de(e: &Engagement) -> R<ContratCs> {
        Ok(ContratCs {
            debut: e.debut,
            fin: e.fin.ok_or("un contrat sans fin n'a pas de certificat")?,
            dates_travail: e.dates_travail.clone(),
            cachets: e.cachets,
            jours: e.jours,
            heures: e.heures,
            taux_cachet: e.taux_cachet,
            numero: e.numero.clone(),
        })
    }

    fn numero_ou(&self, defaut: &str) -> String {
        match &self.numero {
            Some(n) if !n.is_empty() => n.clone(),
            _ => defaut.to_string(),
        }
    }
}

/// Ce que `certificats` lit d'une paie : son contrat, son brut, la fiche du salarie.
#[derive(Debug, Clone)]
pub(crate) struct PaieCs {
    pub engagement: ContratCs,
    pub brut: Decimal,
    pub salarie: Value,
}

impl PaieCs {
    pub fn de(p: &Paie) -> R<PaieCs> {
        Ok(PaieCs { engagement: ContratCs::de(&p.engagement)?, brut: p.brut, salarie: p.salarie.clone() })
    }
}

/// Les 31 mars strictement traverses par un contrat.
fn bornes_31_mars(debut: Jour, fin: Jour) -> Vec<Jour> {
    (debut.an..=fin.an)
        .filter_map(|an| Jour::nouveau(an, 3, 31))
        .filter(|b| debut <= *b && *b < fin)
        .collect()
}

/// Une periode de certificat : (debut, fin, dates, nb, jours).
pub(crate) type Periode = (Jour, Jour, Vec<Jour>, i64, i64);

/// `decouper` : un contrat -> une ou plusieurs periodes de certificat.
pub(crate) fn decouper(eng: &ContratCs) -> R<Vec<Periode>> {
    let bornes = bornes_31_mars(eng.debut, eng.fin);
    if bornes.is_empty() {
        let nb = if eng.cachets != 0 { eng.cachets } else { eng.jours };
        return Ok(vec![(eng.debut, eng.fin, eng.dates_travail.clone(), nb, eng.jours)]);
    }
    if eng.heures.map_or(false, |h| h != 0.0) {
        return Err(Refus {
            classe: Some("ValueError"),
            message: format!(
                "Contrat {} a cheval sur le 31 mars ET porteur d'heures ({} h) : les heures ne sont pas datees, le moteur ne sait pas les repartir entre les deux periodes de conges. Scinder le contrat en deux engagements dans le fichier de paie.",
                eng.numero_ou("sans numero"),
                repr_flottant(eng.heures.unwrap_or(0.0))
            ),
        });
    }
    let n = eng.dates_travail.len() as i64;
    if eng.cachets != n || eng.jours != n {
        return Err(Refus {
            classe: Some("ValueError"),
            message: format!(
                "Contrat {} a cheval sur le 31 mars : {} cachet(s) et {} jour(s) declares pour {} date(s) travaillee(s). La repartition entre les deux periodes de conges serait arbitraire. Scinder le contrat en deux engagements dans le fichier de paie.",
                eng.numero_ou("sans numero"),
                eng.cachets,
                eng.jours,
                n
            ),
        });
    }
    let mut periodes = Vec::new();
    let mut debut = eng.debut;
    let mut tous = bornes;
    tous.push(eng.fin);
    for borne in tous {
        let dates: Vec<Jour> = eng.dates_travail.iter().copied().filter(|x| debut <= *x && *x <= borne).collect();
        let nb = dates.len() as i64;
        periodes.push((debut, borne, dates, nb, nb));
        debut = borne.plus_jours(1)?;
    }
    Ok(periodes)
}

// -------------------------------------------------------------- le certificat

#[derive(Debug, Clone)]
pub(crate) struct Certificat {
    pub numero: i64,
    pub debut: Jour,
    pub fin: Jour,
    pub nb_jours_ou_cachets: i64,
    pub brut: Decimal,
    pub base: Decimal,
    pub emission: Jour,
}

/// `certificats` : les certificats dus au titre d'une paie, dans l'ordre des periodes.
pub(crate) fn certificats(paie: &PaieCs, premier_numero: i64) -> R<Vec<Certificat>> {
    let e = &paie.engagement;
    let parts = decouper(e)?;
    let mut out = Vec::new();
    let mut numero = premier_numero;
    for (debut, fin, _dates, nb, _jours) in &parts {
        let brut = if parts.len() == 1 {
            paie.brut
        } else {
            match e.taux_cachet {
                Some(t) => dec_f64(t)? * Decimal::from(*nb),
                // Aucun cachet ni jour dans la periode : rien a repartir, quel que soit le taux.
                None if *nb == 0 => Decimal::ZERO,
                None => {
                    return Err(Refus {
                        classe: Some("ValueError"),
                        message: format!(
                            "Contrat {} a cheval sur le 31 mars : {} cachet(s) dans la periode du {} au {}, et aucun taux de cachet n'est ecrit -- le brut de chaque periode de conges ne se repartit pas sans lui, et le moteur ne l'invente pas. Ecrire le taux sur la ligne de cachet de l'engagement.",
                            e.numero_ou("sans numero"),
                            nb,
                            debut.jj_mm_aaaa(),
                            fin.jj_mm_aaaa()
                        ),
                    })
                }
            }
        };
        out.push(Certificat {
            numero,
            debut: *debut,
            fin: *fin,
            nb_jours_ou_cachets: *nb,
            brut,
            base: base_conge(brut, &paie.salarie)?,
            emission: *fin,
        });
        numero += 1;
    }
    if parts.len() > 1 {
        let total = out.iter().fold(Decimal::ZERO, |a, c| a + c.brut);
        if total != paie.brut {
            let qui = match &e.numero {
                Some(n) if !n.is_empty() => n.clone(),
                _ => valeur_en_texte(paie.salarie.get("nom").ok_or_else(|| cle_absente("nom"))?),
            };
            return Err(Refus {
                classe: Some("ValueError"),
                message: format!(
                    "Decoupage au 31 mars incoherent pour {} : {} EUR repartis pour un brut de {} EUR.",
                    qui, total, paie.brut
                ),
            });
        }
    }
    Ok(out)
}

fn cle_absente(k: &str) -> Refus {
    Refus { classe: Some("KeyError"), message: socle::fichiers::absent(k) }
}

// --------------------------------------------------------------------- rendu

fn cases_date(j: &Jour) -> String {
    format!("{:02} {:02} {:04}", j.jour, j.mois, j.an)
}

/// `_siret_espace` : 12345678900017 -> '123456789 00017'.
fn siret_espace(siret: &Value) -> String {
    let s = valeur_en_texte(siret);
    let c: Vec<char> = s.chars().collect();
    if c.len() == 14 {
        format!("{} {}", c[..9].iter().collect::<String>(), c[9..].iter().collect::<String>())
    } else {
        s
    }
}

fn requis<'a>(v: &'a Value, k: &str) -> R<&'a Value> {
    v.get(k).ok_or_else(|| cle_absente(k))
}

fn ou_vide(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// `valeurs` : ce qui va dans chaque case, dans l'ordre du formulaire.
fn valeurs(c: &Certificat, p: &PaieCs, ent: &Value) -> R<Vec<(&'static str, String)>> {
    let s = &p.salarie;
    let sig = requis(ent, "signataire")?;
    let agrement = ou_vide(ent, "congesspectacles_agrement");
    let adresse_defaut = requis(s, "adresse")?;
    let commune_defaut = requis(s, "commune")?;
    let mut out: Vec<(&'static str, String)> = Vec::new();
    out.push(("dest_nom", format!("{} {}", valeur_en_texte(requis(s, "prenom")?), valeur_en_texte(requis(s, "nom")?))));
    out.push(("dest_adresse", valeur_en_texte(s.get("adresse_affichage").unwrap_or(adresse_defaut))));
    out.push((
        "dest_commune",
        format!("{} {}", valeur_en_texte(requis(s, "code_postal")?), valeur_en_texte(s.get("commune_affichage").unwrap_or(commune_defaut))),
    ));
    out.push(("dest_pays", "France".to_string()));
    out.push(("siret", siret_espace(requis(ent, "siret")?)));
    out.push(("compte", ou_vide(ent, "congesspectacles_adherent")));
    out.push(("num_ce", c.numero.to_string()));
    out.push((
        "nir",
        format!("{} {}", ou_vide(s, "nir"), ou_vide(s, "nir_cle")).trim_matches(socle::texte::blanc).to_string(),
    ));
    out.push(("raison_sociale", valeur_en_texte(requis(ent, "raison_sociale")?)));
    out.push(("nom_naissance", valeur_en_texte(requis(s, "nom")?)));
    out.push(("nom_usage", ou_vide(s, "nom_usage")));
    out.push(("emploi", valeur_en_texte(requis(s, "emploi")?)));
    out.push(("cadre", if s.get("cadre").map_or(false, vrai) { "OUI" } else { "NON" }.to_string()));
    out.push(("prenom", valeur_en_texte(requis(s, "prenom")?)));
    out.push(("pseudonyme", ou_vide(s, "pseudonyme")));
    out.push(("debut", cases_date(&c.debut)));
    out.push(("fin", cases_date(&c.fin)));
    out.push(("nb_cachets", c.nb_jours_ou_cachets.to_string()));
    out.push(("base_conge", arrondi_euro(c.base).to_string()));
    out.push(("salaire_brut", arrondi_euro(c.brut).to_string()));
    let e = c.emission;
    out.push(("date", format!("{:02}/{:02}/{:02}", e.jour, e.mois, e.an % 100)));
    out.push(("signataire", valeur_en_texte(requis(sig, "nom")?)));
    out.push((
        "agrement",
        if agrement.is_empty() { "N° d'agrément :".to_string() } else { format!("N° d'agrément : {}", agrement) },
    ));
    Ok(out)
}

fn largeur_max(champ: &str, x: f64) -> Option<f64> {
    LIMITES.iter().find(|(c, _)| *c == champ).map(|(_, limite)| limite - RESPIRATION - x)
}

fn texte_fixe(x: f64, y: f64, texte: &str, police: &str, corps: f64, largeur: Option<f64>) -> Value {
    let (nom, gras) = match police {
        TIMES => ("times", false),
        TIMES_GRAS => ("times", true),
        _ => ("courier", false),
    };
    let mut b = Map::new();
    b.insert("type".into(), json!("texte_fixe"));
    b.insert("x".into(), json!(x));
    b.insert("y".into(), json!(y));
    b.insert("texte".into(), json!(texte));
    b.insert("police".into(), json!(nom));
    b.insert("taille".into(), json!(corps));
    b.insert("couleur".into(), json!(NOIR));
    if let Some(l) = largeur {
        b.insert("largeur_max".into(), json!(l));
    }
    if gras {
        b.insert("gras".into(), json!(true));
    }
    Value::Object(b)
}

fn blocs_certificat(c: &Certificat, p: &PaieCs, ent: &Value) -> R<Vec<Value>> {
    let mut blocs: Vec<Value> = TRAITS
        .iter()
        .map(|(x0, y0, x1, y1, ep)| json!({"type": "trait", "x1": x0, "y1": y0, "x2": x1, "y2": y1, "epaisseur": ep, "couleur": NOIR}))
        .collect();
    for (x, y, texte, police, corps) in FOND {
        blocs.push(texte_fixe(*x, *y, texte, police, *corps, None));
    }
    for (champ, texte) in valeurs(c, p, ent)? {
        if !texte.is_empty() {
            let (_, x, y, police, corps) = CASES.iter().find(|(n, ..)| *n == champ).expect("case connue");
            blocs.push(texte_fixe(*x, *y, &texte, police, *corps, largeur_max(champ, *x)));
        }
    }
    if let Some(signature) = identite_de(ent, "signature") {
        let (x0, y0, x1, y1) = SIGNATURE_CADRE;
        let svg = signature.extension().map_or(false, |e| e.to_string_lossy().eq_ignore_ascii_case("svg"));
        blocs.push(json!({"type": "image_fixe", "chemin": chemin_texte(&signature), "x": x0, "y": y0,
                          "largeur": x1 - x0, "hauteur": y1 - y0, "centree": !svg}));
    }
    Ok(blocs)
}

/// `str(Path(...))`.
pub(crate) fn chemin_texte(p: &std::path::Path) -> String {
    p.components().collect::<std::path::PathBuf>().to_string_lossy().into_owned()
}

/// `description` : un document par page, sans marge.
pub(crate) fn description(certs: &[(Certificat, &PaieCs)], ent: &Value) -> R<Value> {
    let mut documents = Vec::new();
    for (c, p) in certs {
        documents.push(json!({"blocs": blocs_certificat(c, p, ent)?}));
    }
    Ok(json!({
        "format": FORMAT,
        "page": {"largeur": 595.22, "hauteur": 842.0, "marges": {"haut": 0, "droite": 0, "bas": 0, "gauche": 0}},
        "titre": "Certificats d'emploi Conges Spectacles",
        "documents": documents
    }))
}

/// `ecrire_numerotes` : chaque paie vient avec le N CE qui EST le sien ; la description du PDF.
pub(crate) fn description_numerotee(paires: &[(&PaieCs, i64)], ent: &Value) -> R<Value> {
    if !ent.get("congesspectacles_adherent").map_or(false, vrai) {
        return Err(Refus {
            classe: Some("ValueError"),
            message: "`congesspectacles_adherent` manquant dans config/entreprise.json : c'est le numero qui va dans la case COMPTE du certificat.".to_string(),
        });
    }
    let mut tous: Vec<(Certificat, &PaieCs)> = Vec::new();
    for (p, premier) in paires {
        for c in certificats(p, *premier)? {
            tous.push((c, p));
        }
    }
    description(&tous, ent)
}

/// `recapitulatif` : le meme contenu en texte, ce qui va etre imprime et sous quels numeros (la trace des N CE
/// consommes, relue par `_premier_ce`).
pub(crate) fn recapitulatif(paires: &[(&PaieCs, i64)], ent: &Value) -> R<String> {
    let adherent = match ent.get("congesspectacles_adherent") {
        Some(v) if vrai(v) => valeur_en_texte(v),
        _ => "?".to_string(),
    };
    let mut lignes = vec!["Certificats d'emploi Conges Spectacles".to_string(), format!("Compte (adherent Audiens) : {}", adherent), "-".repeat(76)];
    for (p, premier) in paires {
        for c in certificats(p, *premier)? {
            let nom = format!(
                "{} {}",
                valeur_en_texte(p.salarie.get("nom").ok_or_else(|| cle_absente("nom"))?),
                valeur_en_texte(p.salarie.get("prenom_affichage").ok_or_else(|| cle_absente("prenom_affichage"))?)
            );
            lignes.push(format!(
                "  N CE {:<6} {:<22} {} -> {}  {:>2} jour(s)/cachet(s)  brut {:>5}  base {:>5}",
                c.numero,
                nom,
                c.debut.jj_mm_aaaa(),
                c.fin.jj_mm_aaaa(),
                c.nb_jours_ou_cachets,
                arrondi_euro(c.brut),
                arrondi_euro(c.base)
            ));
        }
    }
    lignes.push("-".repeat(76));
    lignes.push("A remettre au salarie a la fin de son contrat. Rien a envoyer a".to_string());
    lignes.push("Audiens : la cotisation part deja par la DSN (S21.G00.78 type 27).".to_string());
    Ok(lignes.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(an: i32, mo: u32, jo: u32) -> Jour {
        Jour::nouveau(an, mo, jo).unwrap()
    }

    fn contrat(debut: Jour, fin: Jour, dates: Vec<Jour>, cachets: i64, jours: i64, heures: Option<f64>) -> ContratCs {
        ContratCs { debut, fin, dates_travail: dates, cachets, jours, heures, taux_cachet: Some(130.0), numero: Some("FA1.26.01".into()) }
    }

    #[test]
    fn un_contrat_qui_ne_traverse_pas_le_31_mars_n_a_qu_une_periode() {
        let c = contrat(j(2026, 10, 5), j(2026, 10, 6), vec![j(2026, 10, 5), j(2026, 10, 6)], 2, 2, None);
        let p = decouper(&c).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].3, 2);
    }

    #[test]
    fn la_case_porte_les_jours_quand_il_n_y_a_pas_de_cachet() {
        let c = contrat(j(2026, 10, 5), j(2026, 10, 5), vec![j(2026, 10, 5)], 0, 1, Some(5.0));
        assert_eq!(decouper(&c).unwrap()[0].3, 1);
    }

    #[test]
    fn un_contrat_a_cheval_se_coupe_au_31_mars() {
        let dates = vec![j(2026, 3, 30), j(2026, 3, 31), j(2026, 4, 1)];
        let c = contrat(j(2026, 3, 30), j(2026, 4, 1), dates, 3, 3, None);
        let p = decouper(&c).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].1, p[1].0), (j(2026, 3, 31), j(2026, 4, 1)));
        assert_eq!((p[0].3, p[1].3), (2, 1));
    }

    #[test]
    fn des_heures_sur_le_31_mars_refusent() {
        let c = contrat(j(2026, 3, 30), j(2026, 4, 1), vec![j(2026, 3, 30)], 1, 1, Some(6.0));
        assert!(decouper(&c).unwrap_err().message.contains("porteur d'heures (6.0 h)"));
    }

    #[test]
    fn l_arrondi_est_celui_de_l_euro_moitie_superieure() {
        assert_eq!(arrondi_euro(Decimal::new(12050, 2)), 121);
        assert_eq!(arrondi_euro(Decimal::new(12049, 2)), 120);
    }
}
