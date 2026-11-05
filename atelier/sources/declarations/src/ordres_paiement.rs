//! LES ORDRES DE PAIEMENT D'UNE DSN, EN CLAIR : jamais une liste de montants sans dire a qui. Chaque bloc S21.G00.20 du
//! fichier est un ordre de prelevement. Ici, un seul endroit dit, par ordre : A QUI (l'organisme), POUR QUOI (ce qu'il couvre, et sur quelle periode),
//! COMBIEN, et POUR QUAND. La page affiche ce que rend `decrire`, rien d'autre (regle 2).
//!
//! CE QUI SE LIT (regle 1 : rien ne se devine) :
//!
//! - L'ORGANISME : S21.G00.20.001 du fichier. Son nom propre et ce que couvre son versement (`nom`, `objet`) se lisent
//!   dans la base livree, `donnees/organismes-base.json` (`droit::organismes::Base`), qui cite pour chacun la
//!   table IVO des nomenclatures DSN et Audiens. « DGFIP » est la
//!   valeur que la norme donne a la DGFiP (cahier technique 2026.1, `norme-dsn/texte/bloc-S21.G00.20.txt`, l. 47). Un
//!   identifiant que la base ne connait pas se dit tel quel, sans nom invente.
//! - L'URSSAF : l'organisme du bordereau (S21.G00.22.001) du meme fichier ; une Urssaf par region, le nom est generique.
//! - LA RETRAITE COMPLEMENTAIRE d'une autre institution qu'Audiens : l'organisme dont `codes81` porte le 131 ou le 105,
//!   « Cotisation regime unifie Agirc-Arrco » (`texte/bloc-S21.G00.81.txt`, l. 269 et 287).
//! - LE MONTANT ET LA PERIODE : S21.G00.20.005, .006 et .007.
//!
//! POUR QUAND, toujours avec le texte qui le dit :
//!
//! 1. S21.G00.20.011, quand le fichier la porte : « Date a prendre en compte par l'organisme dans le cas d'un
//!    prelevement SEPA » (`bloc-S21.G00.20.txt`, rubrique 011). C'est le cas de France Travail et d'Audiens Sante
//!    Prevoyance (P0983), que `dsn.rs` ecrit.
//! 2. L'Urssaf : CSS R. 243-6, II, le versement est fait « le mois suivant la periode de travail [...] au plus tard » le 5
//!    ou le 15 (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`) : la date
//!    NOMINALE de `echeance_dsn::du_mois`, sans report (R. 133-14, I ne reporte que la declaration ;
//!    `echeances-dsn/LISEZ-MOI.md`, « Ce qui n'en depend pas »).
//! 3. La DGFiP : BOI-IR-PAS-30-10-30-20, § 50 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-30-20-20190329.html`,
//!    extrait `...-extrait-reversement.md`) : pour un employeur en DSN, le reversement « intervient le mois suivant » et
//!    « doit être effectué au plus tard : le 15 du mois pour les entreprises comptant au maximum quarante-neuf salariés ;
//!    le 5 du mois pour les autres » -- le meme 5 ou 15 que `Contexte::echeance_dsn`, sans report. L'option trimestrielle
//!    sous onze salariés (§ 60) n'est pas modelisee : le moteur reverse chaque mois.
//! 4. Les organismes dont la base donne `jour_de_paiement` (Audiens : retraite, Conges Spectacles) : « L'echeance de
//!    paiement pour Audiens est pour un mois M, le 25 du mois M+1 » (Audiens, dossier DSN,
//!    `atelier/reference/sources/audiens-dsn/audiens-dossier-dsn-20261020.md`), M etant le mois declare.
//! 5. Une autre institution Agirc-Arrco : « le prelevement sera execute au plus tard le dernier jour ouvre du mois
//!    d'exigibilite des cotisations » (cahier d'aide a la codification Agirc-Arrco CT2026, p. 47,
//!    `atelier/reference/sources/agirc-arrco/agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`, l. 2401-2405) :
//!    le dernier jour ouvre du mois qui suit la fin de periode (S21.G00.20.007). « Ouvre » : ni samedi, ni dimanche, ni
//!    fete legale (`echeance_dsn::non_ouvre`).
//! 6. Tout autre organisme sans S21.G00.20.011 : aucun texte depose ne dit son jour. Le logiciel le dit.
//!
//! LE TROP-VERSE : un ordre n'est jamais negatif ; quand une correction a la baisse laisse un credit, l'ordre
//! le dit, avec ce que la compagnie en fait (`credit`). Le credit se LIT dans le fichier, a la maille ou la norme le
//! porte : les bordereaux negatifs des mois corriges (22.005) pour l'Urssaf, les composants (55.001) pour un organisme
//! complementaire, le prelevement retenu (50.009 et 56.007) pour la DGFiP, les cotisations individuelles (81.004) de
//! l'organisme pour les autres -- par 81.002 quand la cotisation le nomme, par son code (`codes81` de la configuration)
//! quand la norme dit 81.002 « non concerne » (Agirc-Arrco, Conges Spectacles : `texte/bloc-S21.G00.81.txt`, l. 372 et
//! 379). Ce qu'il en advient est dit a un seul endroit, `bordereau::credit`.

use droit::organismes::Versement;

use documents::bordereau::{self, Destinataire};
use socle::date::Jour;
use socle::num::Resultat;

const SOURCE_011: &str = "Date de paiement écrite dans la DSN (S21.G00.20.011) : « date à prendre en compte par l'organisme dans le cas d'un prélèvement SEPA » (cahier technique DSN 2026.1, bloc S21.G00.20).";
const SOURCE_URSSAF: &str = "Code de la sécurité sociale, art. R. 243-6, II : le versement est effectué le mois suivant la période de travail, au plus tard le 5 ou le 15 (sans report : seul le dépôt de la DSN se reporte un jour non ouvré, R. 133-14, I).";
const SOURCE_DGFIP: &str = "BOFiP, BOI-IR-PAS-30-10-30-20, § 50 : pour un employeur en DSN, le reversement de la retenue à la source intervient le mois suivant, au plus tard le 15 jusqu'à quarante-neuf salariés, le 5 au-delà (art. 1671 du CGI). L'option trimestrielle sous onze salariés (§ 60) n'est pas prise en compte.";
const SOURCE_AUDIENS: &str = "Audiens, dossier DSN : « L'échéance de paiement pour Audiens est pour un mois M, le 25 du mois M+1 », pour la retraite, les Congés Spectacles et la Santé Prévoyance.";
const SOURCE_AGIRC: &str = "Cahier d'aide à la codification Agirc-Arrco CT2026, p. 47 : « le prélèvement sera exécuté au plus tard le dernier jour ouvré du mois d'exigibilité des cotisations », le mois qui suit la période déclarée (ou le trimestre civil).";
const SOURCE_AUCUNE: &str = "La DSN ne porte pas de date de paiement pour cet organisme (S21.G00.20.011 absente), et aucun texte relevé par le logiciel ne dit quand il prélève : l'organisme prélève à sa propre échéance.";

/// « d'août 2026 », « de septembre 2026 », ou « du 01/07/2026 au 30/09/2026 ».
fn periode_lisible(debut: Option<Jour>, fin: Option<Jour>) -> String {
    match (debut, fin) {
        (Some(d), Some(f)) if d.mois_civil() == f.mois_civil() && d == d.mois_civil().premier_jour() && f == f.mois_civil().dernier_jour() => {
            socle::texte::de(&d.mois_civil().libelle(socle::mois::Graphie::Accentuee))
        }
        (Some(d), Some(f)) => format!("du {} au {}", d.jj_mm_aaaa(), f.jj_mm_aaaa()),
        _ => "d'une période illisible dans le fichier".to_string(),
    }
}

/// Le dernier jour ouvre (ni samedi, ni dimanche, ni fete legale) du mois qui suit `fin`.
fn dernier_ouvre_du_mois_suivant(fin: &Jour) -> Resultat<Jour> {
    let mut j = fin.mois_civil().suivant().dernier_jour();
    while crate::echeance_dsn::non_ouvre(&j)?.is_some() {
        j = j.plus_jours(-1)?;
    }
    Ok(j)
}

/// Le `jour` du mois qui suit le mois declare `mois` (`AAAA-MM`).
fn jour_du_mois_suivant(mois: &str, jour: u32) -> Resultat<Jour> {
    let m = socle::mois::Mois::lire(mois)?.suivant();
    m.jour(jour).ok_or_else(|| socle::date::jour_impossible(m.annee(), m.numero(), jour))
}

/// L'organisme est-il une caisse de retraite complementaire Agirc-Arrco (`codes81` porte le 131 ou le 105) ?
fn est_agirc_arrco(v: Option<&Versement>) -> bool {
    v.is_some_and(|v| v.codes81.iter().any(|x| x == "131" || x == "105"))
}

/// Les ordres d'une DSN fabriquee (`dsn::fabriquer`), mois principal declare `mois` (`AAAA-MM`), en clair : `organisme`
/// (le nom propre, ou l'identifiant quand rien ne le connait), `identifiant`, `objet`, `montant`, `periode` (du, au),
/// `date` (ISO, ou `null` quand aucun texte ne la dit), `quand` (la phrase de la page), `source` (d'ou vient la date) et
/// `credit` (le trop-verse que l'ordre ne reprend pas ou reprend, et ce que la compagnie en fait ; `null` sans
/// trop-verse). `jour_echeance` : 5 ou 15 (`Contexte::echeance_dsn`).
pub fn decrire(ordres: &[crate::dsn::Ordre], mois: &str, jour_echeance: u32) -> Resultat<Vec<OrdreEnClair>> {
    let base = droit::organismes::base()?;
    let mut out = Vec::new();
    for o in ordres {
        let norme = base.versement(&o.organisme);
        let dgfip = o.destinataire == Destinataire::Dgfip;
        let est_urssaf = o.destinataire == Destinataire::Urssaf;
        let organisme = if dgfip {
            "DGFiP".to_string()
        } else if let Some(n) = norme {
            n.nom.clone()
        } else if est_urssaf {
            "Urssaf".to_string()
        } else {
            format!("Organisme {} (inconnu de la configuration)", o.organisme)
        };
        let (debut, fin) = (Some(o.debut), Some(o.fin));
        let periode = periode_lisible(debut, fin);
        let regularisation = debut.is_some_and(|d| d.mois_civil().to_string() != mois);
        let quoi = if dgfip {
            "l'impôt sur le revenu prélevé à la source sur les salaires"
        } else if let Some(n) = norme {
            n.objet.as_str()
        } else if est_urssaf {
            "les cotisations et contributions sociales du bordereau Urssaf"
        } else if est_agirc_arrco(norme) {
            "la retraite complémentaire Agirc-Arrco"
        } else {
            "les cotisations déclarées"
        };
        let objet = format!("{}{} {}", if regularisation { "Régularisation : " } else { "" }, quoi, periode);
        let objet = {
            let mut c = objet.chars();
            c.next().map(|p| p.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        };
        let jour_norme = norme.and_then(|v| v.jour_de_paiement);
        let (date, quand, source): (Option<Jour>, String, &str) = if let Some(j) = o.date_paiement {
            (Some(j), format!("le {}", j.jj_mm_aaaa()), SOURCE_011)
        } else if est_urssaf || dgfip {
            let e = crate::echeance_dsn::du_mois(mois, jour_echeance)?;
            (Some(e.nominale), format!("au plus tard le {}", e.nominale.jj_mm_aaaa()), if dgfip { SOURCE_DGFIP } else { SOURCE_URSSAF })
        } else if let Some(jour) = jour_norme {
            let j = jour_du_mois_suivant(mois, jour as u32)?;
            (Some(j), format!("le {}", j.jj_mm_aaaa()), SOURCE_AUDIENS)
        } else if est_agirc_arrco(norme) && fin.is_some() {
            let j = dernier_ouvre_du_mois_suivant(&fin.expect("fin lue"))?;
            (Some(j), format!("au plus tard le {}", j.jj_mm_aaaa()), SOURCE_AGIRC)
        } else {
            (None, "date non dite par la DSN".to_string(), SOURCE_AUCUNE)
        };
        let credit = bordereau::credit(o.destinataire, &organisme, o.reste, o.deduit)
            .map(|c| CreditEnClair { reste: crate::dsn::mt(c.reste), deduit: crate::dsn::mt(c.deduit), titre: c.titre, que_faire: c.que_faire, source: c.source });
        out.push(OrdreEnClair {
            organisme,
            identifiant: o.organisme.clone(),
            objet,
            montant: crate::dsn::mt(o.montant),
            periode: PeriodeEnClair { du: debut.map(|j| j.iso()), au: fin.map(|j| j.iso()) },
            date: date.map(|j| j.iso()),
            quand,
            source,
            credit,
        });
    }
    Ok(out)
}

/// UN ORDRE DE TELEPAIEMENT EN CLAIR (`decrire`), tel que la boite du depot le montre.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrdreEnClair {
    pub organisme: String,
    pub identifiant: String,
    pub objet: String,
    pub montant: String,
    pub periode: PeriodeEnClair,
    pub date: Option<String>,
    pub quand: String,
    pub source: &'static str,
    pub credit: Option<CreditEnClair>,
}

/// La periode d'un ordre : du, au (`AAAA-MM-JJ`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PeriodeEnClair {
    pub du: Option<String>,
    pub au: Option<String>,
}

/// Le trop-verse d'un ordre (`bordereau::credit`) : ce qu'il ne reprend pas, ce qu'il reprend, et ce que la compagnie en
/// fait.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CreditEnClair {
    pub reste: String,
    pub deduit: String,
    pub titre: String,
    pub que_faire: String,
    pub source: &'static str,
}

/// Le jour du mois suivant ou l'organisme `identifiant` (S21.G00.20.001) preleve, quand la base livree le dit
/// (`jour_de_paiement` de `donnees/organismes-base.json`, la norme, la meme pour toute compagnie), ou `None`.
pub fn jour_de_paiement(identifiant: &str) -> Resultat<Option<u32>> {
    Ok(droit::organismes::base()?.versement(identifiant).and_then(|v| v.jour_de_paiement))
}
