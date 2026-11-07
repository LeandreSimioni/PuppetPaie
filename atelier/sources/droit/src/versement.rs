//! LE JOUR DU VERSEMENT DES SALAIRES : le jour ou part l'argent d'une paie. La compagnie le dit dans sa configuration
//! (fiche entreprise, datee comme elle), une fois pour ses permanents (`date_de_versement_permanents` : un contrat paye au
//! mois, `Nature::mensuelle`) et une fois pour ses intermittents (`date_de_versement_intermittents` : les autres, au
//! cachet ou a l'heure) ; un engagement paye un autre jour le dit
//! sur sa carte (`date_paiement`). C'est la « Date à laquelle le débiteur effectue le versement » (S21.G00.50.001,
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, l. 50-55), la « date de paiement » du bulletin (C. trav.,
//! art. R. 3243-1, 11°, `atelier/reference/sources/code-du-travail/articles-20261030.md`) et le jour ou se lit le taux de
//! prelevement a la source (BOI-IR-PAS-30-10-20, § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`).
//! Le seul lecteur : `engagement::Engagement::date_du_versement`.
//!
//! Le choix de la compagnie :
//! - le dernier jour du mois (vide : le cas ordinaire) ;
//! - le dernier jour du contrat : chaque paie a la fin de sa periode, le dernier jour du contrat dans le mois, ou la fin
//!   du mois pour un contrat qui continue ; un permanent, paye au mois, l'est donc a la fin du mois ;
//! - un jour fixe, dans le mois ou le mois suivant (« le 5 du mois suivant ») : la paie apres la periode d'emploi, la
//!   « pratique dite du décalage de paie » (BOI-IR-PAS-30-10-30-20, § 50,
//!   `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-30-20-extrait-reversement.md`).
//!
//! Les intermittents ne sont pas sous le paiement mensuel (C. trav., art. L. 3242-1, al. 3,
//! `atelier/reference/sources/code-du-travail/xml/L3242-1.xml`) : aucun texte de la loi ne fixe leur jour de paie. Ce que
//! L. 3242-3 (deux paies par mois aux non-mensualises) dit d'eux, aucun texte depose ne le tranche : le moteur ne
//! l'applique pas. Un jour fixe dans le mois pour eux n'est pas refuse : la configuration demande seulement de le
//! confirmer (`a_confirmer_pour_les_intermittents`) ; la case « Payé le » d'une carte ne demande rien.
//!
//! Ce que les textes refusent (`refus`) :
//! - un versement apres le dernier jour du mois qui suit le mois principal declare (CCH-13 de S21.G00.50.001) ;
//! - en convention 1285, le salaire d'un artiste de cirque en contrat a duree determinee regle apres « la 1re semaine du
//!   mois suivant la fin dudit contrat » (CCNEAC, art. X.3.5.4, sous X.3.5 « Rémunération des artistes de cirque »,
//!   `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 324-333).
//! Rien d'autre : aucun texte n'ordonne le depot de la DSN et le versement, ni ne refuse un versement avant la fin du
//! contrat.
//!
//! INFERENCES, ecrites comme telles : un jour fixe que le mois n'a pas (« le 31 » en avril) est son dernier jour ; « la
//! 1re semaine du mois suivant » va du 1er au 7.

use socle::date::Jour;
use socle::mois::Mois;
use socle::num::Resultat;

use crate::fiche::Lue;
use crate::natures::Nature;

/// Les deux cases de la fiche entreprise : le jour des permanents, le jour des intermittents.
pub const CLE_PERMANENTS: &str = "date_de_versement_permanents";
pub const CLE_INTERMITTENTS: &str = "date_de_versement_intermittents";
pub const CLES: [&str; 2] = [CLE_PERMANENTS, CLE_INTERMITTENTS];

/// Les formes qu'elle lit, dans la phrase d'un refus.
const FORMES: &str = "vide (le dernier jour du mois), « fin du contrat », « le 25 » ou « le 5 du mois suivant »";

/// Le jour du versement que la compagnie a choisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Versement {
    #[default]
    FinDuMois,
    FinDuContrat,
    /// Le quantieme, dans le mois de la paie ou le suivant.
    Fixe { quantieme: u32, mois_suivant: bool },
}

impl Versement {
    /// La case telle qu'elle s'ecrit ; `Err` : ce qu'elle attend.
    pub fn lire(texte: &str) -> Result<Versement, String> {
        let t = socle::texte::sans_accent(texte).to_lowercase();
        let mots: Vec<&str> = t.split_whitespace().collect();
        let t = mots.join(" ");
        let t = match t.strip_prefix("le ") {
            Some(reste) => reste,
            None => &t,
        };
        match t {
            "" | "fin du mois" | "dernier jour du mois" => return Ok(Versement::FinDuMois),
            "fin du contrat" | "dernier jour du contrat" => return Ok(Versement::FinDuContrat),
            _ => {}
        }
        let (n, mois_suivant) = match t.strip_suffix(" du mois suivant") {
            Some(n) => (n, true),
            None => (t, false),
        };
        let n = match n.strip_suffix("er") {
            Some(reste) => reste,
            None => n,
        };
        match n.parse::<u32>() {
            Ok(q) if (1..=31).contains(&q) => Ok(Versement::Fixe { quantieme: q, mois_suivant }),
            _ => Err(format!("{} attendu", FORMES)),
        }
    }

    /// La case de la fiche entreprise `ent` qui dit le jour d'un contrat de nature `nat` (`cle_de`) ; absente : le dernier
    /// jour du mois.
    pub fn de_la_fiche(ent: &crate::fiche::ChampsEntreprise, nat: &Nature) -> Resultat<Versement> {
        let dit = if nat.mensuelle() { &ent.date_de_versement_permanents } else { &ent.date_de_versement_intermittents };
        match dit.portee() {
            Some(t) => Versement::lire(t).map_err(|_| {
                format!(
                    "Le jour du versement des {} que dit la configuration, « {} », ne se lit pas : écrire {} \
                     (Configuration › Entreprise).",
                    if nat.mensuelle() { "permanents" } else { "intermittents" },
                    t.trim(),
                    FORMES
                )
            }),
            None => Ok(Versement::FinDuMois),
        }
    }

    /// Ce que la configuration demande de confirmer avant d'ecrire le jour des intermittents `texte`, quand il change : un
    /// jour fixe dans le mois paie avant leur fin les contrats qui finissent apres lui, ce qui n'est pas l'usage (un choix
    /// de la compagnie, que rien n'interdit : `None` pour les autres formes, et le 31, qui est la fin du mois).
    pub fn a_confirmer_pour_les_intermittents(texte: &str) -> Option<String> {
        match Versement::lire(texte) {
            Ok(Versement::Fixe { quantieme, mois_suivant: false }) if quantieme < 31 => Some(format!(
                "Payer le {} alors que des contrats peuvent finir jusqu'au 31 n'est pas l'usage. Continuer ?",
                if quantieme == 1 { "1er".to_string() } else { quantieme.to_string() }
            )),
            _ => None,
        }
    }

    /// Le jour du versement de la paie du mois `mois` (AAAA-MM) dont la periode finit le `fin_periode`.
    pub fn jour(&self, mois: &str, fin_periode: Jour) -> Resultat<Jour> {
        let mois = Mois::lire(mois)?;
        match *self {
            Versement::FinDuMois => Ok(mois.dernier_jour()),
            Versement::FinDuContrat => Ok(fin_periode),
            Versement::Fixe { quantieme, mois_suivant } => {
                let m = if mois_suivant { mois.suivant() } else { mois };
                let jour = quantieme.min(m.jours());
                m.jour(jour).ok_or_else(|| format!("Le {} du mois n'existe pas.", jour))
            }
        }
    }
}

/// Le dernier jour ou peut etre verse ce que declare la DSN du mois principal `mois` (AAAA-MM) : le dernier jour du
/// mois suivant (CCH-13 de S21.G00.50.001, `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, l. 61-62).
pub fn au_plus_tard_en_dsn(mois: &str) -> Resultat<Jour> {
    Ok(Mois::lire(mois)?.suivant().dernier_jour())
}

/// Le dernier jour ou se regle le salaire d'un artiste de cirque dont le contrat a duree determinee finit le `fin` :
/// « au plus tard, la 1re semaine du mois suivant la fin dudit contrat » (CCNEAC, art. X.3.5.4) ; le 7, par inference.
pub fn au_plus_tard_cirque_1285(fin: &Jour) -> Resultat<Jour> {
    fin.mois_civil().suivant().premier_jour().plus_jours(6)
}

/// Le refus d'un versement le `verse` pour `qui`, paye au titre du mois `mois` ; `fin_cirque` : la fin du contrat a duree
/// determinee d'un artiste de cirque en convention 1285, quand elle tombe dans ce mois. `None` : les textes l'admettent.
pub fn refus(qui: &str, verse: &Jour, mois: &str, fin_cirque: Option<&Jour>) -> Resultat<Option<String>> {
    let corriger = "Choisir un jour plus tôt : sur la carte (« Payé le »), ou dans la configuration (« Jour du versement \
                    des permanents » ou « des intermittents »).";
    let limite = au_plus_tard_en_dsn(mois)?;
    if *verse > limite {
        return Ok(Some(format!(
            "{} : versé le {}, après le {}, dernier jour où la DSN de ce mois peut déclarer un versement (norme DSN, \
             S21.G00.50.001, CCH-13). {}",
            qui,
            verse.jj_mm_aaaa(),
            limite.jj_mm_aaaa(),
            corriger
        )));
    }
    if let Some(fin) = fin_cirque {
        let limite = au_plus_tard_cirque_1285(fin)?;
        if *verse > limite {
            return Ok(Some(format!(
                "{} : versé le {}, après le {}. Le salaire d'un artiste de cirque en CDD se règle à la fin du contrat \
                 ou au plus tard la première semaine du mois qui suit sa fin (CCNEAC, art. X.3.5.4). {}",
                qui,
                verse.jj_mm_aaaa(),
                limite.jj_mm_aaaa(),
                corriger
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(t: &str) -> Jour {
        Jour::lire(t).unwrap()
    }

    /// CCH-13 de S21.G00.50.001 : au plus tard le dernier jour du mois qui suit le mois principal declare ; le 5 du mois
    /// suivant (le decalage de paie, BOI-IR-PAS-30-10-30-20, § 50) passe.
    #[test]
    fn un_versement_apres_le_mois_qui_suit_se_refuse() {
        let cinq = Versement::lire("le 5 du mois suivant").unwrap().jour("2026-12", j("2026-12-31")).unwrap();
        assert_eq!(cinq, j("2027-01-05"));
        assert_eq!(refus("X", &cinq, "2026-12", None).unwrap(), None);
        assert_eq!(refus("X", &j("2027-01-31"), "2026-12", None).unwrap(), None);
        assert!(refus("X", &j("2027-02-01"), "2026-12", None).unwrap().is_some_and(|m| m.contains("CCH-13")));
    }

    /// CCNEAC, art. X.3.5.4 : le salaire d'un artiste de cirque en CDD se regle au plus tard la premiere semaine du mois
    /// qui suit la fin du contrat.
    #[test]
    fn un_artiste_de_cirque_de_la_1285_est_paye_la_premiere_semaine_qui_suit_son_contrat() {
        let fin = j("2026-10-28");
        assert_eq!(refus("X", &j("2026-11-07"), "2026-10", Some(&fin)).unwrap(), None);
        assert!(refus("X", &j("2026-11-10"), "2026-10", Some(&fin)).unwrap().is_some_and(|m| m.contains("X.3.5.4")));
        // Hors du cirque de la 1285, le 10 du mois suivant passe.
        assert_eq!(refus("X", &j("2026-11-10"), "2026-10", None).unwrap(), None);
    }
}
