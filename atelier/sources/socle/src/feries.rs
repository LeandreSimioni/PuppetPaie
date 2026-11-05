//! Les jours feries et ceux qui sont chomes.
//!
//! Un ferie n'est pas forcement chome : la loi ne rend chome d'office que
//! le 1er mai. Le reste se lit dans la fiche entreprise
//! (`jours_feries_chomes`). Les dates mobiles se comptent depuis Paques.
//!
//! `Jours` : la date et le nom de chaque jour, dans l'ordre du fichier livre.

use crate::date::Jour;
use crate::fichiers::{forme_en_cache, FormeGardee};
use crate::num::Resultat;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};

/// La date et le nom de chaque jour, une date une fois, dans l'ordre ou elle vient.
pub type Jours = Vec<(Jour, String)>;

/// `feries-base.json` : les fetes legales de L3133-1, dans l'ordre de l'article.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseFeries {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "note")]
    _note: String,
    jours: Vec<FeteLue>,
}

/// Une fete du fichier : a date fixe (`mois`, `jour`), ou comptee depuis le dimanche de Paques (`paques`, en jours).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeteLue {
    nom: String,
    #[serde(rename = "article")]
    _article: String,
    #[serde(default)]
    mois: Option<i64>,
    #[serde(default)]
    jour: Option<i64>,
    #[serde(default)]
    paques: Option<i64>,
    /// Chome par la loi elle-meme (L3133-4 : le 1er mai).
    #[serde(default)]
    chome_de_droit: bool,
    #[serde(default, rename = "chome_article")]
    _chome_article: Option<String>,
}

/// Le jour d'une fete : fixe dans l'annee, ou a tant de jours du dimanche de Paques.
enum Quand {
    Fixe { mois: i64, jour: i64 },
    Paques(i64),
}

/// Une fete legale, telle que le fichier livre la dit.
struct Fete {
    nom: String,
    quand: Quand,
    chome_de_droit: bool,
}

const ILLISIBLE: &str = "Les jours fériés livrés avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE de `feries-base.json`, une fois tant qu'il ne change pas. Une fete dit sa date fixe ou son ecart a
/// Paques, l'un ou l'autre.
fn fetes() -> Resultat<Arc<Vec<Fete>>> {
    static CACHE: OnceLock<FormeGardee<Vec<Fete>>> = OnceLock::new();
    forme_en_cache(&CACHE, "feries-base.json", ILLISIBLE, |b: BaseFeries| {
        b.jours
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                let quand = match (f.mois, f.jour, f.paques) {
                    (Some(mois), Some(jour), None) => Quand::Fixe { mois, jour },
                    (None, None, Some(p)) => Quand::Paques(p),
                    _ => {
                        return Err(crate::echec::a_la_cle(
                            ILLISIBLE,
                            &format!("jours[{}]", i),
                            "une fête dit son mois et son jour, ou son écart à Pâques, l'un ou l'autre",
                        ))
                    }
                };
                Ok(Fete { nom: f.nom, quand, chome_de_droit: f.chome_de_droit })
            })
            .collect()
    })
}

/// Le jour `an`-`mois`-`jour`, ou le refus qui dit ce qui ne va pas (l'annee, le mois, le jour).
pub fn date_ou_refus(an: i64, mois: i64, jour: i64) -> Resultat<Jour> {
    if !(1..=9999).contains(&an) {
        return Err(crate::date::annee_impossible(an));
    }
    if !(1..=12).contains(&mois) {
        return Err(crate::mois::mois_impossible(mois));
    }
    Jour::nouveau(an as i32, mois as u32, jour.max(0) as u32)
        .ok_or_else(|| crate::date::jour_impossible(an, mois, jour))
}

fn inserer(out: &mut Jours, jour: Jour, nom: &str) {
    match out.iter_mut().find(|(j, _)| *j == jour) {
        Some(e) => e.1 = nom.to_string(),
        None => out.push((jour, nom.to_string())),
    }
}

/// Le dimanche de Paques (Meeus, comput gregorien).
pub fn paques(annee: i32) -> Resultat<Jour> {
    let an = annee as i64;
    let a = an.rem_euclid(19);
    let (b, c) = (an.div_euclid(100), an.rem_euclid(100));
    let (d_, e) = (b.div_euclid(4), b.rem_euclid(4));
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d_ - g + 15).rem_euclid(30);
    let (i, k) = (c.div_euclid(4), c.rem_euclid(4));
    let l = (32 + 2 * e + 2 * i - h - k).rem_euclid(7);
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let t = h + l - 7 * m + 114;
    date_ou_refus(an, t.div_euclid(31), t.rem_euclid(31) + 1)
}

/// Les onze fetes legales de l'annee.
pub fn feries(annee: i32) -> Resultat<Jours> {
    let mut out = Jours::new();
    let dimanche = paques(annee)?;
    for f in fetes()?.iter() {
        let jour = match f.quand {
            Quand::Paques(ecart) => dimanche.plus_jours(ecart)?,
            Quand::Fixe { mois, jour } => date_ou_refus(annee as i64, mois, jour)?,
        };
        inserer(&mut out, jour, &f.nom);
    }
    Ok(out)
}

/// Ceux que la loi rend chomes : le 1er mai, et lui seul.
pub fn chomes_de_droit(annee: i32) -> Resultat<Jours> {
    let fetes = fetes()?;
    let noms: Vec<&str> = fetes.iter().filter(|f| f.chome_de_droit).map(|f| f.nom.as_str()).collect();
    Ok(feries(annee)?.into_iter().filter(|(_, n)| noms.contains(&n.as_str())).collect())
}

/// Ce que la fiche entreprise chome : `tous` les feries, sinon le 1er mai seulement.
pub const TOUS: &str = "tous";

/// Les feries chomes dans cette entreprise, cette annee. `dits` : ce que sa fiche en dit (`jours_feries_chomes` : `tous`,
/// `1er mai seulement`) ; rien de dit, le plancher legal.
pub fn chomes(annee: i32, dits: Option<&str>) -> Resultat<Jours> {
    match dits {
        Some(TOUS) => feries(annee),
        _ => chomes_de_droit(annee),
    }
}

/// Les feries chomes d'une periode, annees a cheval comprises.
pub fn chomes_entre(debut: &Jour, fin: &Jour, dits: Option<&str>) -> Resultat<Jours> {
    let mut out = Jours::new();
    for annee in debut.an..=fin.an {
        for (jour, nom) in chomes(annee, dits)? {
            if *debut <= jour && jour <= *fin {
                inserer(&mut out, jour, &nom);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les onze fetes legales de C. trav. L3133-1 (`atelier/reference/sources/code-du-travail/articles-20261021.md`,
    /// l. 92-105), Paques au comput gregorien ; le 1er mai seul chome de droit (L3133-4).
    #[test]
    fn les_feries_de_l_annee() {
        for (an, m, j) in [(2024, 3, 31), (2025, 4, 20), (2026, 4, 5), (2027, 3, 28)] {
            assert_eq!(paques(an).unwrap(), Jour::nouveau(an, m, j).unwrap());
        }
        let jours: Vec<String> = feries(2026).unwrap().into_iter().map(|(j, _)| j.iso()).collect();
        assert_eq!(
            jours,
            ["2026-01-01", "2026-04-06", "2026-05-01", "2026-05-08", "2026-05-14", "2026-05-25", "2026-07-14", "2026-08-15", "2026-11-01", "2026-11-11", "2026-12-25"]
        );
        let chomes: Vec<String> = chomes_de_droit(2026).unwrap().into_iter().map(|(j, _)| j.iso()).collect();
        assert_eq!(chomes, ["2026-05-01"]);
    }
}
