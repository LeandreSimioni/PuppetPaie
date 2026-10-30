//! Les jours qui comptent pour les conges payes et les absences d'un permanent : ouvrables (L3141-3, du lundi au
//! samedi), travailles (la fiche du salarie, `JoursTravailles`), et l'indemnite retenue entre le dixieme et le maintien
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

/// Les jours de la semaine, dans l'ordre (lundi = 0), tels que la fiche les ecrit.
pub const JOURS_DE_LA_SEMAINE: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/// LES JOURS TRAVAILLES D'UN PERMANENT : les jours de la semaine que son horaire programme, lus sur sa fiche
/// (`jours_travailles`, datee comme toute la fiche), du lundi au vendredi tant qu'elle n'en dit pas d'autres -- la valeur
/// que la case montre, cochee, sur la fiche (`champs-base.json`). C'est l'horaire de l'entreprise pour ce salarie : les
/// heures du mois s'y comptent (`jours_entre`, `periodes::part_du_mois`), et les heures d'un jour sont la duree
/// hebdomadaire sur leur nombre (`heures_par_jour`). Fiche DSN 2640 (`atelier/reference/sources/suspension-non-remuneree/
/// net-entreprises-fiche2640-volumes-activite-s21g00-53-20260929.txt`, « Le decompte et calcul des absences au reel ») :
/// « Nombre d'heures d'absence = Nb d'heures travaillees/jours oeuvres x Nb de jours theoriquement oeuvres sur la
/// periode ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoursTravailles {
    jours: [bool; 7],
    /// La fiche les dit (sinon : du lundi au vendredi, la valeur que la case montre).
    pub dits: bool,
}

impl JoursTravailles {
    /// Du lundi au vendredi, quand la fiche ne dit rien.
    pub fn du_lundi_au_vendredi() -> Self {
        JoursTravailles { jours: [true, true, true, true, true, false, false], dits: false }
    }

    /// Les jours que dit la fiche `fiche` (sa valeur au jour lu). Refus : une liste vide, ou un nom qui n'est pas un jour.
    pub fn de_la_fiche(fiche: &Value) -> Resultat<Self> {
        let v = match fiche.get("jours_travailles") {
            None | Some(Value::Null) => return Ok(Self::du_lundi_au_vendredi()),
            Some(v) => v,
        };
        let qui = fiche.get("_cle").and_then(Value::as_str).unwrap_or("le salarie");
        let liste = v.as_array().ok_or_else(|| {
            format!("Fiche de {} : « Jours travaillés » est une liste de jours de la semaine (lundi, mardi...).", qui)
        })?;
        let mut jours = [false; 7];
        for j in liste {
            let nom = j.as_str().unwrap_or_default().trim().to_lowercase();
            let rang = JOURS_DE_LA_SEMAINE.iter().position(|x| *x == nom).ok_or_else(|| {
                format!(
                    "Fiche de {} : « Jours travaillés » porte {}, qui n'est pas un jour de la semaine (lundi ... dimanche).",
                    qui, j
                )
            })?;
            jours[rang] = true;
        }
        if !jours.iter().any(|x| *x) {
            return Err(format!(
                "Fiche de {} : « Jours travaillés » ne coche aucun jour. Les heures du mois se comptent sur les jours que \
l'horaire programme (Cass. soc., 20/01/1999) : cocher ceux de son contrat.",
                qui
            ));
        }
        Ok(JoursTravailles { jours, dits: true })
    }

    /// Le jour `j` est-il travaille ?
    pub fn travaille(&self, j: &Jour) -> bool {
        self.jours[j.jour_de_la_semaine() as usize]
    }

    /// Combien de jours par semaine.
    pub fn par_semaine(&self) -> i64 {
        self.jours.iter().filter(|x| **x).count() as i64
    }

    /// LES JOURS TRAVAILLES entre deux dates, bornes incluses, jours feries compris -- les jours que l'horaire programme
    /// dans le mois, un ferie chome etant paye comme un jour de travail a qui a trois mois d'anciennete (L3133-3 ;
    /// `feries_non_payes` pour les autres). Fiche DSN 2693, exemple 4 (`atelier/reference/sources/suspension-non-remuneree/
    /// net-entreprises-fiche2693-volumes-activite-exemples-20260929.txt`, l. 509) : janvier 2024 entier, « nombre de jours
    /// ouvres du mois [...] 161 heures (23 jours x 7 heures) », le lundi 1er janvier compte parmi les 23.
    pub fn jours_entre(&self, debut: &Jour, fin: &Jour) -> Resultat<i64> {
        let (mut n, mut jour) = (0, *debut);
        while jour <= *fin {
            if self.travaille(&jour) {
                n += 1;
            }
            jour = jour.plus_jours(1)?;
        }
        Ok(n)
    }

    /// Les jours, en mots : « du lundi au vendredi » pour une suite, « le lundi, le mardi et le jeudi » sinon.
    pub fn en_mots(&self) -> String {
        let rangs: Vec<usize> = (0..7).filter(|i| self.jours[*i]).collect();
        let suite = rangs.windows(2).all(|w| w[1] == w[0] + 1);
        match rangs.as_slice() {
            [seul] => format!("le {}", JOURS_DE_LA_SEMAINE[*seul]),
            [premier, .., dernier] if suite => {
                format!("du {} au {}", JOURS_DE_LA_SEMAINE[*premier], JOURS_DE_LA_SEMAINE[*dernier])
            }
            _ => {
                let noms: Vec<String> = rangs.iter().map(|i| format!("le {}", JOURS_DE_LA_SEMAINE[*i])).collect();
                let (dernier, tete) = noms.split_last().expect("au moins un jour");
                format!("{} et {}", tete.join(", "), dernier)
            }
        }
    }
}

/// UN FERIE CHOME QUE LE SALARIE N'A PAS PAYE : « Le chomage des jours feries ne peut entrainer aucune perte de salaire
/// pour les salaries totalisant au moins trois mois d'anciennete dans l'entreprise » (L3133-3,
/// `atelier/reference/sources/code-du-travail/articles-20261029.md`, l. 113) ; a qui ne les totalise pas, le ferie chome
/// n'est pas du. Le 1er mai fait exception : « Le chomage du 1er mai ne peut etre une cause de reduction de salaire »
/// (L3133-5, meme fichier, l. 123), quelle que soit l'anciennete. Seuls comptent les feries que l'entreprise chome
/// (`feries::chomes_entre`) et qui tombent un jour travaille du salarie : un ferie travaille est un jour de travail.
/// L'anciennete se compte depuis `anciennete` (la fiche, « Dans la compagnie depuis » : les contrats successifs s'y
/// cumulent, L3133-3, al. 2), au jour du ferie (`permanents::anciennete_mois`).
pub fn feries_non_payes(
    debut: &Jour,
    fin: &Jour,
    entreprise: Option<&Value>,
    jours: &JoursTravailles,
    anciennete: &Jour,
) -> Resultat<Vec<(Jour, String)>> {
    let mut out = Vec::new();
    for (j, nom) in feries::chomes_entre(debut, fin, entreprise)? {
        let premier_mai = feries::chomes_de_droit(j.an)?.iter().any(|(x, _)| *x == j);
        if jours.travaille(&j) && !premier_mai && crate::permanents::anciennete_mois(anciennete, &j) < Decimal::from(3) {
            out.push((j, nom));
        }
    }
    Ok(out)
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

/// LES HEURES D'UN JOUR TRAVAILLE d'un contrat paye au mois de quotite `quotite` (heures par mois) : la duree
/// hebdomadaire (quotite x 12 / 52, au centieme : 151,67 h font 35,00 h) sur les jours travailles de la semaine -- 7 h
/// pour un temps plein sur cinq jours. Fiche DSN 2640, « Nb d'heures travaillees/jours oeuvres ». INFERENCE NOMMEE :
/// la duree se repartit egalement entre les jours travailles (la fiche ne dit pas les heures de chaque jour).
pub fn heures_par_jour(quotite: Decimal, jours: &JoursTravailles) -> Decimal {
    div(eur2(div(mul(quotite, Decimal::from(12)), Decimal::from(52))), Decimal::from(jours.par_semaine()))
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
