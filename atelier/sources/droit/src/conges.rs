//! Les jours qui comptent pour les conges payes et les absences d'un permanent : ouvrables (L3141-3, du lundi au
//! samedi), oeuvres (l'hypothese classique de la fiche DSN 2640), et l'indemnite retenue entre le dixieme et le maintien
//! (L3141-24). Le compteur d'un salarie, qui se derive de ses paies, est l'affaire de l'engagement (`engagement::conges`).

use serde_json::Value;

use socle::date::Jour;
use socle::feries;
use socle::num::{div, eur2, mul, Decimal, Resultat};

/// Le nombre de jours ouvrables entre deux dates, bornes incluses : du lundi
/// au samedi, moins les feries que l'entreprise chome.
pub fn jours_ouvrables(debut: &Jour, fin: &Jour, entreprise: Option<&Value>) -> Resultat<i64> {
    jours_de_semaine(debut, fin, entreprise, 6)
}

/// LES JOURS OEUVRES entre deux dates, bornes incluses : du lundi au vendredi, moins les feries que l'entreprise chome
/// -- les jours ou un salarie a temps plein « doit travailler » dans la modalite classique (cinq jours de sept heures
/// pour 35 h). Fiche DSN 2640 (`atelier/reference/sources/suspension-non-remuneree/net-entreprises-fiche2640-volumes-
/// activite-s21g00-53-20260929.txt`, « Le decompte et calcul des absences au reel ») : quand le planning du salarie n'est
/// pas connu du logiciel, « Nombre d'heures d'absence = Nb d'heures travaillees/jours oeuvres x Nb de jours
/// theoriquement oeuvres sur la periode », sur « une ou plusieurs hypotheses de modalites classiques de travail » ;
/// fiche 2693, exemple 10 : un mois de septembre 2026 entier, « 154 heures (22 x 7 heures) ». INFERENCE NOMMEE : la
/// saisie ne dit pas les jours de la semaine d'un permanent, et le moteur prend l'hypothese classique, du lundi au
/// vendredi (la mesure des absences, `imprevus::lignes_du_mois`).
pub fn jours_oeuvres(debut: &Jour, fin: &Jour, entreprise: Option<&Value>) -> Resultat<i64> {
    jours_de_semaine(debut, fin, entreprise, 5)
}

/// LES JOURS OUVRES entre deux dates, bornes incluses : du lundi au vendredi, jours feries compris -- les jours que
/// l'horaire de l'entreprise programme dans le mois, un ferie chome etant paye comme un jour de travail (L3133-3 : « Le
/// chomage des jours feries ne peut entrainer aucune perte de salaire », `atelier/reference/sources/code-du-travail/
/// articles-20261029.md`, l. 113). Fiche DSN 2693, exemple 4 (`atelier/reference/sources/suspension-non-remuneree/
/// net-entreprises-fiche2693-volumes-activite-exemples-20260929.txt`, l. 509) : janvier 2024 entier, « nombre de jours
/// ouvres du mois [...] 161 heures (23 jours x 7 heures) », le lundi 1er janvier compte parmi les 23. L'hypothese des
/// cinq jours est celle de `jours_oeuvres`.
pub fn jours_ouvres(debut: &Jour, fin: &Jour) -> Resultat<i64> {
    let (mut n, mut jour) = (0, *debut);
    while jour <= *fin {
        if jour.jour_de_la_semaine() < 5 {
            n += 1;
        }
        jour = jour.plus_jours(1)?;
    }
    Ok(n)
}

/// Les jours de `debut` a `fin` dont le rang dans la semaine (lundi = 0) est sous `jusqu_a`, moins les feries chomes.
fn jours_de_semaine(debut: &Jour, fin: &Jour, entreprise: Option<&Value>, jusqu_a: u32) -> Resultat<i64> {
    let chomes = feries::chomes_entre(debut, fin, entreprise)?;
    let (mut n, mut jour) = (0, *debut);
    while jour <= *fin {
        if jour.jour_de_la_semaine() < jusqu_a && !chomes.iter().any(|(j, _)| *j == jour) {
            n += 1;
        }
        jour = jour.plus_jours(1)?;
    }
    Ok(n)
}

/// LES HEURES D'UN JOUR OEUVRE d'un contrat paye au mois de quotite `quotite` (heures par mois) : la duree hebdomadaire
/// (quotite x 12 / 52, au centieme : 151,67 h font 35,00 h) sur cinq jours (`jours_oeuvres`) -- 7 h pour un temps plein.
/// Fiche DSN 2640, « Nb d'heures travaillees/jours oeuvres » ; l'hypothese des cinq jours est l'inference de
/// `jours_oeuvres`.
pub fn heures_par_jour_oeuvre(quotite: Decimal) -> Decimal {
    div(eur2(div(mul(quotite, Decimal::from(12)), Decimal::from(52))), Decimal::from(5))
}

/// L'indemnite de conges : le dixieme, le maintien, et celui qui gagne.
#[derive(Debug, Clone)]
pub struct IndemniteDesJours {
    pub dixieme: Decimal,
    pub maintien: Decimal,
    pub retenu: Decimal,
    pub regle: &'static str,
}

/// L'indemnite retenue (L3141-24) : la plus forte du dixieme et du maintien de salaire ; a egalite, le maintien.
pub fn la_plus_forte(dixieme: Decimal, maintien: Decimal) -> IndemniteDesJours {
    let maintien = eur2(maintien);
    IndemniteDesJours {
        dixieme,
        maintien,
        retenu: if maintien > dixieme { maintien } else { dixieme },
        regle: if dixieme > maintien { "dixieme" } else { "maintien" },
    }
}
