//! Qui l'employeur declare a la caisse des Conges Spectacles.
//!
//! Les textes (deposes ; `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md` dit ou, et ce qu'on en lit) :
//! - code du travail D. 7121-40 : les employeurs du spectacle « s'affilient, pour le personnel artistique et technique
//!   qu'ils emploient, a la caisse de conges payes » ; D. 7121-41 : ils lui « declarent [...] le personnel artistique
//!   et technique qu'ils n'ont pas employe de facon continue pendant les douze mois precedant la demande de conge »
//!   (releve LEGI du 10/10/2026, `code-du-travail/articles-20261010.md`) ;
//! - la fiche pratique Audiens « Les Conges Spectacles » (fevrier 2023) : la caisse sert les artistes et techniciens
//!   « quelle que soit [...] la nature du contrat de travail (CDD de droit commun, CDD d'usage, etc) », et la cotisation
//!   se calcule « sur chaque bulletin de paie d'artiste ou de technicien embauche sous contrat a duree determinee de
//!   moins de 12 mois » (`majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt`, l. 15-31) ;
//! - CCN 3090, art. 10.1.1 (etendu) : le droit commun s'applique au CDD d'usage « occupe[] de maniere continue, dans
//!   le cadre d'un meme contrat de travail, pendant les 12 mois » ; CCNEAC (1285), art. IX.1 : D. 7121-41 repris.
//!
//! Ce que le moteur en fait : un CDDU de moins de douze mois (du debut au terme signes) est declare a la caisse, un
//! CDDU de douze mois ou plus ne l'est pas, des son premier bulletin ; un CDD de droit commun de moins de douze mois
//! d'un intermittent (un artiste, ou un technicien d'une fonction que la liste de l'annexe 8 nomme pour la convention de
//! l'employeur : `intermittent`) l'est ; un CDI ne l'est jamais. Ce qui n'est pas porte (des contrats successifs qui,
//! mis bout a bout, font douze mois continus ; un contrat prolonge au-dela de douze mois) : aucun texte depose ne le
//! tranche.

use crate::natures::Nature;
use socle::date::Jour;
use crate::emplois;
use socle::fichiers::vrai;
use socle::num::Resultat;
use serde_json::Value;

/// Ce que la pile du bareme decrit sans contrat : le CORPS est un intermittent affilie (le CDDU), la section
/// `par_nature.permanent` un salarie que l'employeur paie de ses conges (CDI, CDD de droit commun). C'est la valeur
/// que prennent les lectures qui n'ont pas de contrat.
pub fn par_defaut(nat: &Nature) -> bool {
    !nat.mensuelle()
}

/// Le jour ou un contrat commence a durer douze mois : son debut, douze mois plus tard (le dernier jour du mois quand
/// ce jour n'existe pas : un 29 fevrier). Un contrat dont le lendemain du terme l'atteint dure douze mois ou plus.
fn douze_mois_apres(debut: &Jour) -> Jour {
    let an = debut.an + 1;
    let jour = debut.jour.min(socle::date::jours_du_mois(an, debut.mois));
    Jour::nouveau(an, debut.mois, jour).unwrap_or(*debut)
}

/// Le contrat, du `debut` au `fin` signes, dure-t-il douze mois ou plus ?
pub fn douze_mois_ou_plus(debut: &Jour, fin: &Jour) -> Resultat<bool> {
    Ok(fin.plus_jours(1)? >= douze_mois_apres(debut))
}

/// La cle de la fiche (telle que la paie la lit) qui porte la convention collective de l'employeur (son IDCC) : le
/// domaine d'activite ou l'annexe 8 range le technicien (`intermittent`). La fiche la recoit de l'entreprise, a la
/// date ou la paie la lit (`engagement::calcul::fiche_de_l_engagement`).
pub const IDCC_DE_L_EMPLOYEUR: &str = "idcc_de_l_employeur";

/// LE CONTRAT D'UN INTERMITTENT. Un CDD d'usage (la nature payee a l'activite) l'est toujours ; un CDI jamais ; un CDD
/// de droit commun l'est quand l'annexe de l'assurance chomage range son salarie, dans les memes termes que pour le
/// CDDU :
/// - l'annexe 10 : « les artistes [...] engages au titre d'un contrat de travail a duree determinee » (art. 1er, § 2,
///   `atelier/reference/sources/plafond-intermittents/unedic-annexe10-convention-20241115.txt`, l. 15-17) : un metier
///   de l'annexe 10 de la nomenclature des emplois du spectacle ;
/// - l'annexe 8 : « les ouvriers et techniciens engages dans le cadre d'un contrat a duree determinee, pour assurer
///   l'une des fonctions enumerees dans la liste figurant au titre XIV, [...] dans les domaines d'activite ou dans les
///   entreprises enumeres dans la meme liste », domaines « definis par les numeros d'identifiant des conventions
///   collectives (IDCC) » (art. 1er, § 2, et art. 71, `unedic-annexe8-convention-20241115.txt`, l. 16-22 et 1530 s.) :
///   un metier du catalogue dont la liste de la convention de l'employeur nomme la fonction (`conventions` de
///   `emplois-base.json`, relevee de la liste ; `IDCC_DE_L_EMPLOYEUR` sur la fiche). Un metier hors catalogue ne dit
///   pas sur quelle liste il est : non.
/// La caisse des Conges Spectacles ne distingue pas davantage : elle sert « le personnel artistique et technique
/// employe de facon intermittente » (C. trav. D. 7121-38 ; D. 7121-40, D. 7121-41,
/// `atelier/reference/sources/code-du-travail/articles-20261102.md`, l. 596-607).
/// INFERENCE (ecrite) : l'employeur est dans le domaine de sa convention. Pour le spectacle vivant (1285, 3090), la
/// liste n'ajoute que des categories que remplit tout employeur de la convention qui fait jouer un spectacle (licencie,
/// ou organisateur occasionnel) ; pour les autres domaines, elle nomme aussi des codes NAF, que le moteur ne confronte
/// pas.
/// Ce qui en suit, et le lit ici : la caisse des Conges Spectacles (`affilie`), le code emploi de la nomenclature du
/// spectacle en DSN (S21.G00.40.005, CCH-18 : « 999SPT » n'est que pour les emplois permanents), l'attestation employeur
/// Cinema Spectacle (`documents::aer::due`), le certificat de la caisse.
pub fn intermittent(nat: &Nature, sal: &Value) -> Resultat<bool> {
    if nat.duree_indeterminee {
        return Ok(false);
    }
    if !nat.mensuelle() {
        return Ok(true);
    }
    let code = sal.get("code_emploi_ft").filter(|c| vrai(c)).and_then(Value::as_str).unwrap_or("").trim().to_string();
    if code.is_empty() || code.eq_ignore_ascii_case("999SPT") {
        return Ok(false);
    }
    let Some(m) = emplois::metier(&code)? else {
        return Ok(false);
    };
    match m.get("annexe").and_then(|a| a.as_i64().or_else(|| a.as_f64().map(|x| x as i64))) {
        Some(10) => Ok(true),
        Some(8) => {
            let idcc = sal.get(IDCC_DE_L_EMPLOYEUR).filter(|c| vrai(c)).map(socle::date::en_texte).unwrap_or_default();
            let listes = m.get("conventions").and_then(Value::as_array).cloned().unwrap_or_default();
            Ok(!idcc.trim().is_empty() && listes.iter().any(|c| socle::date::en_texte(c).trim() == idcc.trim()))
        }
        _ => Ok(false),
    }
}

/// L'employeur declare-t-il ce contrat a la caisse des Conges Spectacles ? `sal` est la fiche telle que la paie la lit
/// (`emplois::emploi_de` a la date du contrat).
/// - un CDI : non ;
/// - un CDD d'usage (CDDU) : oui, sauf s'il dure douze mois ou plus du debut au terme signes (le chargeur des
///   engagements pose toujours le terme d'un CDDU : sa derniere date ; sans terme, il est de moins de douze mois) ;
/// - un CDD de droit commun d'un intermittent (`intermittent`) : oui, sauf s'il dure douze mois ou plus ; sans terme,
///   refus -- sa duree decide, et le moteur ne la devine pas ;
/// - un CDD de droit commun d'un autre salarie : non.
pub fn affilie(nat: &Nature, sal: &Value, debut: &Jour, fin: Option<&Jour>) -> Resultat<bool> {
    if !intermittent(nat, sal)? {
        return Ok(false);
    }
    let fin = match fin {
        Some(f) => f,
        None if !nat.mensuelle() => return Ok(true),
        None => {
            return Err(format!(
                "{} d'un intermittent commence le {} sans terme : la duree d'un contrat a duree determinee dit s'il est \
declare a la caisse des Conges Spectacles (moins de douze mois : code du travail D. 7121-41, fiche Audiens), et le \
moteur ne la devine pas.",
                nat.libelle,
                debut.jj_mm_aaaa()
            ))
        }
    };
    Ok(!douze_mois_ou_plus(debut, fin)?)
}
