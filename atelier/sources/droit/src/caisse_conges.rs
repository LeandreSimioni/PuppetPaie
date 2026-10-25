//! Qui l'employeur declare a la caisse des Conges Spectacles (lot V2f).
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
//! d'un ARTISTE l'est ; un CDI ne l'est jamais. Ce qui n'est pas porte (le technicien en CDD de droit commun ; des
//! contrats successifs qui, mis bout a bout, font douze mois continus ; un contrat prolonge au-dela de douze mois) est
//! dans `prive/journal/norme/corrections/PASSATION-V2f.md` : ce sont des decisions, pas des lectures.

use crate::natures::Nature;
use socle::date::Jour;
use crate::emplois;
use socle::fichiers::vrai;
use socle::num::Resultat;
use serde_json::Value;

/// Ce que la pile du bareme decrit sans contrat : le CORPS est un intermittent affilie (le CDDU), la section
/// `par_nature.permanent` un salarie que l'employeur paie de ses conges (CDI, CDD de droit commun). C'est la valeur
/// que prennent les lectures qui n'ont pas de contrat (`oracle bareme`, `service bareme-applique`).
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

/// Le salarie tient-il un emploi ARTISTIQUE du spectacle ? Son metier (`code_emploi_ft`, tel que la paie le lit) est
/// un emploi de l'annexe 10 de la nomenclature des emplois du spectacle (les artistes). Un metier hors nomenclature, ou
/// pas de metier : non.
///
/// Les TECHNICIENS (annexe 8) ne sont pas lus ici, et c'est voulu : D. 7121-40 dit « le personnel artistique et
/// technique » sans le definir, et l'annexe 8 -- la seule liste que le moteur ait -- range parmi les « ouvriers et
/// techniciens » l'administrateur, le charge et l'attache de production (liste de l'Unedic,
/// `unedic-annexe8-liste-champ-application-20161216.txt`, p. 21-22), que la compagnie fictive `fictive-3090-grande` paie en
/// CDD de droit commun avec ses conges payes par l'employeur. Quel technicien en CDD de droit commun est declare a la
/// caisse est une decision (PASSATION-V2f.md) ; en attendant, il reste ce qu'il etait : non declare.
pub fn artiste_du_spectacle(sal: &Value) -> Resultat<bool> {
    let code = sal.get("code_emploi_ft").filter(|c| vrai(c)).and_then(Value::as_str).unwrap_or("").trim().to_string();
    if code.is_empty() || code.eq_ignore_ascii_case("999SPT") {
        return Ok(false);
    }
    let Some(m) = emplois::metier(&code)? else {
        return Ok(false);
    };
    let annexe = m.get("annexe").and_then(|a| a.as_i64().or_else(|| a.as_f64().map(|x| x as i64)));
    Ok(annexe == Some(10))
}

/// L'employeur declare-t-il ce contrat a la caisse des Conges Spectacles ? `sal` est la fiche telle que la paie la lit
/// (`emplois::emploi_de` a la date du contrat).
/// - un CDI : non ;
/// - un CDD d'usage (CDDU) : oui, sauf s'il dure douze mois ou plus du debut au terme signes (le chargeur des
///   engagements pose toujours le terme d'un CDDU : sa derniere date ; sans terme, il est de moins de douze mois) ;
/// - un CDD de droit commun d'un artiste (`artiste_du_spectacle`) : oui, sauf s'il dure douze mois ou plus ; sans
///   terme, refus -- sa duree decide, et le moteur ne la devine pas ;
/// - un CDD de droit commun d'un autre salarie : non (voir `artiste_du_spectacle` pour les techniciens).
pub fn affilie(nat: &Nature, sal: &Value, debut: &Jour, fin: Option<&Jour>) -> Resultat<bool> {
    if nat.duree_indeterminee {
        return Ok(false);
    }
    if nat.mensuelle() && !artiste_du_spectacle(sal)? {
        return Ok(false);
    }
    let fin = match fin {
        Some(f) => f,
        None if !nat.mensuelle() => return Ok(true),
        None => {
            return Err(format!(
                "{} d'un artiste commence le {} sans terme : la duree d'un contrat a duree determinee dit s'il est \
declare a la caisse des Conges Spectacles (moins de douze mois : code du travail D. 7121-41, fiche Audiens), et le \
moteur ne la devine pas.",
                nat.libelle,
                debut.jj_mm_aaaa()
            ))
        }
    };
    Ok(!douze_mois_ou_plus(debut, fin)?)
}
