//! Le bloc S21.G00.44 « Assujettissement fiscal » de la DSN de decembre (corrections DSN, lot V3b ; point 11 de
//! `prive/journal/RELECTURE-NORME-DSN.md`, fiche N8 de `prive/journal/norme/fiches-N8.md`).
//!
//! LA NORME (cahier technique DSN 2026.1, `atelier/reference/norme-dsn/texte/`) :
//! - § 2.3.1, p. 58 (`chapitre-2-principes-de-constitution.txt`, lignes 1954-1967 ; taxe par taxe, p. 58-59, lignes
//!   1971-2059) : « L'assujettissement des entreprises est déclaré en DSN une fois par an, soit en fin d'exercice,
//!   soit en cours d'exercice pour les cas de cessation d'activité. Conformément au code général des impôts, cet
//!   assujettissement est déclaré par établissement de l'entreprise. [...] L'assujettissement ou le
//!   non-assujettissement doit obligatoirement être déclaré. [...] Hors cas de cessation d'activité de l'entreprise,
//!   le renseignement de ce bloc doit intervenir, au plus tard, dans la DSN mensuelle de décembre » ;
//! - le bloc, p. 240 (`bloc-S21.G00.44.txt`) et p. 241 (sa suite est en tete de `bloc-S21.G00.45.txt`) : les codes 001 a
//!   016 ; « Si une entreprise n'est pas assujettie à une taxe, le motif correspondant [...] doit être renseigné » ; la
//!   taxe sur les salaires porte en 009 « le montant de l'assiette de la taxe sur les salaires au taux normal pour
//!   l'ensemble des salariés », et « autant de blocs [...] que nécessaire » pour les assiettes aux taux majores ;
//!   CCH-11 de 44.001 : montant obligatoire pour 005, 009, 015, 016 ; CCH-12 : montant interdit pour 002, 004, 006, 008,
//!   010, 014 ; 44.003, le millesime ; 44.004 obligatoire pour 002, interdit sinon (CCH-11, p. 241), « Table MNATA »
//!   (livree depuis le lot R2 : `logiciel/donnees/mnata-base.json`).
//!
//! CE QUI SE DEDUIT ET CE QUI SE SAISIT (regle 1 : rien ne se devine ; ce que le moteur sait deja ne se saisit pas une
//! seconde fois). Les textes sont deposes sous `atelier/reference/sources/` (LISEZ-MOI de `bloc-44-assujettissement/`) :
//! - taxe d'apprentissage (001/002) et CFP (007/008) : elles se paient a l'Urssaf, au bordereau de chaque mois, et c'est
//!   le moteur qui les y met (CTP 992, ou 993 en Alsace-Moselle ; CTP 983, 959 ou 971 de `config/organismes.json`,
//!   compose depuis `donnees/organismes-base.json`). Le bloc 44 dit ce que le bordereau paie : 001 si le CTP 992 ou 993
//!   y est, 007 si un CTP de formation y est (008 sinon). Sans la taxe d'apprentissage au bordereau : 002, avec le motif
//!   de la table MNATA (44.004) que la fiche entreprise dit (`taxe_apprentissage_motif_non_assujettissement`), sinon
//!   refus (lot R2). Le motif 007 « Associations sans but lucratif » ne vaut que jusqu'au millesime 2025 : L. 6241-1,
//!   dans sa redaction du 21/02/2026, rend redevables « les associations [...] a activites non lucratives » ; celle du
//!   16/02/2025 ne les nommait pas (les deux deposees, `atelier/reference/sources/code-du-travail/`).
//! - contribution supplementaire a l'apprentissage (003/004) : code du travail L. 6242-1 (entreprises d'au moins 250
//!   salaries redevables de la taxe, effectif de l'annee meme, regle des cinq ans de L. 130-1 CSS). Sous 250 : 004.
//!   A 250 et plus, le quotient des contrats d'alternance ne se calcule pas ici : la fiche le dit
//!   (`contribution_supplementaire_apprentissage`, « due » ou « non_due »), sinon refus. (En decembre, l'Urssaf ne
//!   l'a pas encore calcule : il arrive debut mars en CRM 118, codes A05 et A07, et la DSN de mars N+1 declare la
//!   contribution elle-meme, calculee : `csa`, lot D2.)
//! - PEEC (005/006) : CCH L. 313-1 et norme p. 58 (« les employeurs occupant au minimum cinquante salariés »), quelle
//!   que soit la TVA (BOFiP BOI-TPS-PEEC-10, § 40). Sous 50 (effectif de l'annee du millesime, regle des cinq ans) :
//!   006 ; a 50 et plus, 005 avec l'assiette (CCH-11) : les remunerations de l'annee qui servent de base aux cotisations
//!   de securite sociale, la base 03 « assiette brute deplafonnee » de chaque contrat de l'etablissement.
//! - taxe sur les salaires (009/010, 015/016) : CGI art. 231. Elle depend de la TVA de la compagnie, que le moteur ne
//!   connait pas : la fiche dit sa TVA, le moteur conclut (`regime_taxe_sur_les_salaires`), sinon refus.
//!   Les assiettes sont celles de l'etat « taxe sur les salaires » (`etats_compta`), etablissement par etablissement.
//! - CPF-CDD (013/014) : code du travail L. 6331-6 (1 % des remunerations des titulaires d'un CDD), sauf le CDD
//!   saisonnier (L. 6331-6) et ceux de D. 6331-72 ; l'intermittent du spectacle releve de la participation unique de
//!   L. 6331-55 (« par dérogation [...] à l'article L. 6331-6 »). 013 si l'etablissement a eu dans l'annee un CDD de
//!   droit commun qui y est soumis, 014 sinon.
//!
//! INFERENCES, ecrites comme telles dans la passation du lot (`prive/journal/norme/corrections/PASSATION-V3b.md`) :
//! 015 et 016 sont les assiettes au premier et au second taux majore (8,50 % et 13,60 %) ; le montant du 005 est
//! l'assiette de l'annee du millesime, et l'effectif qui le declenche celui de cette annee-la ; la part assujettie a la
//! taxe sur les salaires s'applique aux montants 009, 015 et 016 ; un CDDU (motif 05) est un contrat d'intermittent au
//! sens de L. 6331-55 ; 013 se declare seulement si l'annee a eu un CDD soumis a la contribution.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::apprentissage::{champ_du_motif, motif_de_la_fiche, taxe_apprentissage_non_due, MILLESIME_ASSOCIATIONS_REDEVABLES, MNATA_ASSOCIATIONS};
use crate::dsn::{g, liste, txt};
use socle::num::{add, dec_avec_echelle, div, eur, mul, sub, Decimal, Resultat};

/// Un bloc S21.G00.44 : le code taxe (44.001) et, s'il y en a un, le montant (44.002). Le millesime (44.003) est
/// l'annee de la DSN de decembre.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloc44 {
    pub code: &'static str,
    pub montant: Option<Decimal>,
    /// Le motif de non-assujettissement a la taxe d'apprentissage (44.004, table MNATA), pour un 002 seulement.
    pub motif: Option<String>,
}

/// La taxe d'apprentissage au bordereau (table des CTP de l'Urssaf) : la part principale hors Alsace-Moselle (992,
/// 0,59 %) et celle des etablissements du Bas-Rhin, du Haut-Rhin et de la Moselle (993, 0,44 % ; code du travail
/// L. 6241-1-1, II).
const CTP_TAXE_APPRENTISSAGE: [&str; 2] = ["992", "993"];
/// La formation professionnelle au bordereau : intermittents (983), moins de onze salaries (959), onze et plus (971).
const CTP_FORMATION: [&str; 3] = ["983", "959", "971"];
/// Le champ de la fiche entreprise qui dit la taxe sur les salaires.
pub const CHAMP_TAXE_SUR_LES_SALAIRES: &str = "taxe_sur_les_salaires";
/// Le champ de la fiche entreprise qui dit la contribution supplementaire, a 250 salaries et plus.
pub const CHAMP_CONTRIBUTION_SUPPLEMENTAIRE: &str = "contribution_supplementaire_apprentissage";

// ------------------------------------------------------------ la taxe sur les salaires, salarie par salarie

/// L'assiette de la taxe sur les salaires d'une paie : celle de la CSG, sans la reduction de 1,75 % (CGI art. 231, 1 :
/// « La réduction mentionnée au I de l'article L. 136-2 du code de la sécurité sociale n'est pas applicable »).
/// `abattement` : le facteur de la CSG du bareme (`abattement_csg`, 0,9825). Le calcul de l'etat « taxe sur les
/// salaires » (`etats_compta`), mis ici pour que la DSN lise le meme.
pub(crate) fn assiette_taxe_sur_les_salaires(p: &Paie, abattement: Decimal) -> Decimal {
    let csg = p.bases.get("csg").copied().unwrap_or(Decimal::ZERO);
    eur(add(sub(csg, eur(mul(p.brut, abattement))), p.brut))
}

/// Les fractions d'une remuneration individuelle annuelle soumises aux taux majores (CGI art. 231, 2 bis) : celle
/// comprise entre `seuil_1` et `seuil_2`, et celle qui excede `seuil_2`.
pub(crate) fn fractions_majorees(base: Decimal, seuil_1: Decimal, seuil_2: Decimal) -> (Decimal, Decimal) {
    let positif = |x: Decimal| if x > Decimal::ZERO { x } else { Decimal::ZERO };
    let premiere = positif(sub(if base < seuil_2 { base } else { seuil_2 }, seuil_1));
    (premiere, positif(sub(base, seuil_2)))
}

/// LA TAXE SUR LES SALAIRES SE DEDUIT DE LA TVA (ticket #216, 20/10/2026). La fiche disait « la compagnie la doit-elle ? »
/// et « part des salaires taxee » : on demandait a l'administratrice la conclusion du droit, pas le fait qu'elle connait.
/// Elle dit maintenant ce qu'elle sait de sa TVA, et le moteur conclut, une seule fois, ici -- pour l'ecran (la route
/// `taxe-sur-les-salaires`), pour la DSN de decembre (`blocs`) et pour l'etat « taxe sur les salaires » (`etats_compta`).
///
/// Le texte : CGI art. 231, 1 (version en vigueur depuis le 19/06/2025, deposee :
/// `atelier/reference/sources/code-general-des-impots/xml/231.xml`, lisible dans `articles-20261015.md`) :
/// - la taxe est a la charge des employeurs « lorsqu'ils ne sont pas assujettis a la taxe sur la valeur ajoutee ou ne
///   l'ont pas ete sur 90 % au moins de leur chiffre d'affaires au titre de l'annee civile precedant celle du paiement » ;
/// - son assiette : les remunerations multipliees par « le rapport [...] entre le chiffre d'affaires qui n'a pas ete
///   passible de la taxe sur la valeur ajoutee et le chiffre d'affaires total », ce total s'entendant « du total des
///   recettes et autres produits, y compris ceux [...] qui n'entrent pas dans le champ d'application de la [TVA] », le
///   numerateur « du total des recettes et autres produits qui n'ont pas ouvert droit a deduction » ;
/// - 2e alinea : « Les remunerations versees par les employeurs dont le chiffre d'affaires realise au cours de l'annee
///   civile precedant le versement de ces remunerations n'excede pas les plafonds definis aux I et I bis de l'article
///   293 B sont exonerees de la taxe sur les salaires. »
/// L'abattement des associations (art. 1679 A, depose a cote) ne change pas qui doit la taxe, seulement ce qu'il reste a
/// payer : l'etat le calcule.
///
/// La fiche (`taxe_sur_les_salaires` de config/entreprise.json) :
/// - `tva` : « assujettie » ou « exoneree » (exoneree de TVA, ou hors de son champ) ;
/// - `part_tva` : pour une compagnie assujettie, la part (en %) de ses recettes et produits de l'annee precedente,
///   subventions comprises, soumise a la TVA. INFERENCE : une recette soumise a la TVA est une recette qui a ouvert droit
///   a deduction ; le rapport de l'article 231 vaut donc 100 - `part_tva`. A 90 au moins, la taxe n'est pas due ;
/// - `sous_franchise` : quand la taxe serait due, le chiffre d'affaires de l'annee precedente n'excede-t-il pas les
///   plafonds de l'article 293 B ? (2e alinea) ;
/// - `association_1679A` : quand la taxe est due, l'abattement de l'article 1679 A s'applique-t-il ?
/// Chaque case ne se montre que si la precedente l'appelle (`champs`). Une case qui manque fait refuser la DSN de
/// decembre, avec ce qu'il faut dire (regle 1 : rien ne se devine).
#[derive(Debug, Clone, PartialEq)]
pub struct RegimeTaxeSalaires {
    /// `Ok(None)` : la taxe n'est pas due ; `Ok(Some(p))` : elle est due sur `p` % des remunerations (le rapport de
    /// l'article 231, 1) ; `Err` : ce que la fiche doit encore dire.
    pub decision: Result<Option<Decimal>, String>,
    /// L'abattement des associations (art. 1679 A), quand la taxe est due.
    pub association: bool,
    /// Les cases de la fiche que l'ecran montre, dans l'ordre : chacune parce que la precedente l'appelle.
    pub champs: Vec<&'static str>,
    /// La consequence, en clair, telle que l'ecran l'affiche sous les cases.
    pub consequence: String,
}

/// Un pourcentage tel qu'on l'ecrit en francais : « 62,5 ».
fn pourcent(p: Decimal) -> String {
    p.normalize().to_string().replace('.', ",")
}

/// `taxe_sur_les_salaires` de la fiche entreprise `ent`, et ce que le moteur en conclut (CGI art. 231, 1). `plafonds` :
/// ceux de la franchise en base de TVA, lus au bareme de l'annee (`plafonds_franchise`) ; la conclusion les nomme quand
/// on les a (ticket #245), elle dit « les plafonds » sinon. Ils ne changent rien a la decision.
pub fn regime_taxe_sur_les_salaires(ent: &Value, plafonds: Option<(Decimal, Decimal)>) -> RegimeTaxeSalaires {
    // Les deux plafonds, en mots de compagnie, pour la conclusion (ticket #245).
    let les_plafonds = match plafonds {
        Some((services, total)) => format!(
            "les deux plafonds de la franchise en base de TVA, {} de spectacles vendus et {} de recettes en tout",
            euros(services),
            euros(total)
        ),
        None => "les plafonds de la franchise en base de TVA".to_string(),
    };
    let fiche = g(ent, CHAMP_TAXE_SUR_LES_SALAIRES);
    let mut champs = vec!["tva"];
    let manque = |champs: Vec<&'static str>, quoi: String, consequence: &str| RegimeTaxeSalaires {
        decision: Err(format!(
            "Configuration › Entreprise, « Taxe sur les salaires » : {} (CGI art. 231, 1). Sans cette reponse, la DSN de \
decembre ne se fait pas.",
            quoi
        )),
        association: false,
        champs,
        consequence: consequence.to_string(),
    };
    let non_due = |champs: Vec<&'static str>, pourquoi: &str| RegimeTaxeSalaires {
        decision: Ok(None),
        association: false,
        champs,
        consequence: format!(
            "La compagnie ne doit pas la taxe sur les salaires : {}. La DSN de décembre le déclare.",
            pourquoi
        ),
    };
    let part_taxee = match fiche.get("tva").and_then(Value::as_str).unwrap_or("") {
        "exoneree" => Decimal::ONE_HUNDRED,
        "assujettie" => {
            champs.push("part_tva");
            let part = match fiche.get("part_tva") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.trim().is_empty() => None,
                Some(v) => match dec_avec_echelle(v) {
                    Ok(p) if p >= Decimal::ZERO && p <= Decimal::ONE_HUNDRED => Some(p),
                    _ => {
                        return manque(
                            champs,
                            format!("la part des recettes soumise a la TVA vaut {}, qui n'est pas un pourcentage de 0 a 100", v),
                            "La part des recettes soumise à la TVA est un pourcentage, de 0 à 100.",
                        )
                    }
                },
            };
            let Some(part) = part else {
                return manque(
                    champs,
                    "dire quelle part de ses recettes de l'annee precedente etait soumise a la TVA".to_string(),
                    "Dites quelle part de vos recettes de l'an dernier était soumise à la TVA : à 90 % ou plus, la \
compagnie ne doit pas la taxe sur les salaires ; en dessous, elle la doit sur le reste.",
                );
            };
            if part >= Decimal::from(90) {
                return non_due(
                    champs,
                    &format!(
                        "elle est assujettie à la TVA sur {} % de ses recettes de l'an dernier, 90 % ou plus (code général \
des impôts, art. 231)",
                        pourcent(part)
                    ),
                );
            }
            sub(Decimal::ONE_HUNDRED, part)
        }
        "" => {
            return manque(
                champs,
                "dire si la compagnie est assujettie a la TVA ou exoneree".to_string(),
                "Dites si la compagnie est assujettie à la TVA ou exonérée : le logiciel en déduit si elle doit la taxe \
sur les salaires. Sans réponse, la DSN de décembre ne se fait pas.",
            )
        }
        autre => {
            return manque(
                champs,
                format!("`tva` vaut « {} » ; il vaut « assujettie » ou « exoneree »", autre),
                "Dites si la compagnie est assujettie à la TVA ou exonérée.",
            )
        }
    };
    // La taxe serait due : sauf sous les plafonds de la franchise en base (231, 1, 2e alinea).
    champs.push("sous_franchise");
    let sur = if part_taxee == Decimal::ONE_HUNDRED {
        "sur tous ses salaires".to_string()
    } else {
        format!("sur {} % de ses salaires, la part de ses recettes qui n'était pas soumise à la TVA", pourcent(part_taxee))
    };
    match fiche.get("sous_franchise") {
        Some(Value::Bool(true)) => {
            return non_due(
                champs,
                &format!(
                    "ses recettes de l'an dernier sont restées sous {} (code général des impôts, art. 231 et 293 B)",
                    les_plafonds
                ),
            )
        }
        Some(Value::Bool(false)) => {}
        _ => {
            return manque(
                champs,
                "dire si le chiffre d'affaires de l'annee precedente depasse les plafonds de la franchise en base de TVA \
(art. 293 B)"
                    .to_string(),
                &format!(
                    "La compagnie doit la taxe sur les salaires {}, sauf si ses recettes de l'an dernier sont restées sous \
{} (code général des impôts, art. 231 et 293 B) : dites-le.",
                    sur, les_plafonds
                ),
            )
        }
    }
    champs.push("association_1679A");
    let association = fiche.get("association_1679A") == Some(&Value::Bool(true));
    RegimeTaxeSalaires {
        decision: Ok(Some(part_taxee)),
        association,
        champs,
        consequence: format!(
            "La compagnie doit la taxe sur les salaires, {} (code général des impôts, art. 231). La DSN de décembre en \
déclare les assiettes ; l'état « Taxe sur les salaires » (Exports) calcule ce qui reste à payer après la franchise et la \
décote{}.",
            sur,
            if association { ", puis l'abattement des associations (art. 1679 A)" } else { "" }
        ),
    }
}

/// LES PLAFONDS DE LA FRANCHISE EN BASE DE TVA, EN CLAIR (ticket #239, 21/10/2026). L'ecran demandait « sous les plafonds
/// de l'article 293 B ? » sans dire lesquels : « on pourrait pas avoir les plafonds directement dans le logiciel ? ». Le
/// moteur les lit au bareme de l'annee (`taxe_sur_les_salaires.plafonds_franchise`) et ecrit la question et ses deux
/// reponses avec les montants ; la page n'en porte aucun.
///
/// Les textes (deposes, lus dans `atelier/reference/sources/code-general-des-impots/LISEZ-MOI.md`) :
/// - CGI art. 231, 1, 2e alinea (`xml/231.xml`) : sont exonerees les remunerations des employeurs « dont le chiffre
///   d'affaires realise au cours de l'annee civile precedant le versement de ces remunerations n'excede pas les plafonds
///   definis aux I et I bis de l'article 293 B » ;
/// - CGI art. 293 B, I (`xml/293_B.xml`, version en vigueur depuis le 01/03/2025) : ligne « Annee civile precedente »,
///   85 000 EUR de « chiffre d'affaires national total » et 37 500 EUR de chiffre d'affaires « afferent aux prestations de
///   services » ;
/// - la notice 2502-SD (`formulaires/impots-2502-sd-2025-notice.pdf`, C, 1) : ce chiffre d'affaires « doit s'apprecier au
///   regard de l'ensemble des recettes et autres produits, y compris [...] subventions non imposables a la TVA ».
/// INFERENCES, ecrites comme telles : un spectacle vendu (cession, billetterie) est une prestation de services, d'ou le
/// plafond de 37 500 EUR pour une compagnie ; les subventions comptent dans le total (85 000 EUR), pas dans les prestations ;
/// la colonne « annee civile precedente » est celle que vise 231 (le chiffre d'affaires de l'annee d'avant) ; le I bis
/// (avocats, auteurs, artistes-interpretes) ne vise pas une compagnie employeur.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionFranchise {
    /// L'aide de la question, avec les montants de l'annee : un paragraphe par ligne (`\n`).
    pub aide: String,
    /// La reponse « sous les plafonds » (`sous_franchise` vrai), et « au-dessus » (faux).
    pub sous: String,
    pub dessus: String,
    /// Les plafonds lus au bareme : prestations de services, total. `None` si le bareme de l'annee ne les porte pas.
    pub plafonds: Option<(Decimal, Decimal)>,
}

/// Un montant entier, ecrit comme sur l'ecran : « 37 500 € », en espaces insecables (ticket #245 : un montant ne se coupe
/// pas en fin de ligne, « 85 / 000 € »).
fn euros(x: Decimal) -> String {
    let s = crate::bulletin::fmt_eur(x);
    format!("{} €", s.strip_suffix(",00").unwrap_or(&s)).replace(' ', "\u{a0}")
}

/// Les plafonds de la franchise en base de TVA au bareme `bar` (`taxe_sur_les_salaires.plafonds_franchise`) : prestations
/// de services, total. `None` si le bareme ne les porte pas.
pub fn plafonds_franchise(bar: Option<&Value>) -> Option<(Decimal, Decimal)> {
    let pf = g(g(bar?, CHAMP_TAXE_SUR_LES_SALAIRES), "plafonds_franchise");
    let lire = |k: &str| pf.get(k).and_then(|v| dec_avec_echelle(v).ok());
    Some((lire("prestations_de_services")?, lire("total")?))
}

/// La question des plafonds pour les remunerations de `annee` (le chiffre d'affaires de `annee - 1`), depuis le bareme
/// de `annee` (`bar`, `None` s'il n'y en a pas). Un bareme sans plafonds ne fait pas refuser : la question se pose sans
/// montants, et le dit.
pub fn question_franchise(bar: Option<&Value>, annee: i32) -> QuestionFranchise {
    let avant = annee - 1;
    let pf = bar.map(|b| g(g(b, CHAMP_TAXE_SUR_LES_SALAIRES), "plafonds_franchise"));
    let lus = plafonds_franchise(bar);
    let Some((services, total)) = lus else {
        return QuestionFranchise {
            aide: format!(
                "Le barème {} du logiciel ne porte pas les plafonds de la franchise en base de TVA (code général des impôts, \
art. 293 B) : la mise à jour du logiciel les apportera. Sous ces plafonds, la compagnie ne doit pas la taxe sur les salaires.",
                annee
            ),
            sous: "Sous les deux plafonds".to_string(),
            dessus: "Au-dessus d'un des plafonds".to_string(),
            plafonds: None,
        };
    };
    // Une version de 293 B entree en vigueur en cours d'annee se dit (2025 : le 01/03).
    let depuis = pf
        .and_then(|pf| pf.get("en_vigueur_depuis"))
        .and_then(Value::as_str)
        .and_then(|s| socle::date::Jour::lire(s).ok())
        .filter(|j| j.an == annee && (j.mois, j.jour) != (1, 1))
        .map(|j| format!(", en vigueur depuis le {}", j.jj_mm_aaaa()))
        .unwrap_or_default();
    QuestionFranchise {
        aide: format!(
            "Les deux plafonds se comparent aux recettes de l'année civile {avant} (du 1er janvier au 31 décembre), pour \
les salaires de {annee}. Sous les deux, la compagnie ne doit pas la taxe sur les salaires ; au-dessus d'un seul, elle la \
doit.\n{s} : les spectacles vendus, c'est-à-dire les cessions, la billetterie, les ateliers et vos autres prestations. Les \
subventions n'y entrent pas.\n{t} : les recettes en tout, ces ventes plus les subventions et tous les autres produits.\n\
Ces montants sont dans le compte de résultat {avant} (ses produits), ou chez votre comptable. Code général des impôts, \
art. 231 et 293 B{depuis}.",
            avant = avant,
            s = euros(services),
            t = euros(total),
            annee = annee,
            depuis = depuis,
        ),
        sous: format!(
            "Sous les deux plafonds en {} : au plus {} de spectacles vendus (cessions, billetterie, ateliers…) et au plus \
{} de recettes en tout, subventions comprises",
            avant,
            euros(services),
            euros(total)
        ),
        dessus: format!(
            "Au-dessus d'un plafond en {} : plus de {} de spectacles vendus, ou plus de {} de recettes en tout",
            avant,
            euros(services),
            euros(total)
        ),
        plafonds: Some((services, total)),
    }
}

/// Ce que la DSN de decembre lit : `None` si la taxe n'est pas due, sinon la part taxee des remunerations en
/// pourcentage. Une fiche qui ne dit pas tout : refus, avec ce qu'il faut dire (`regime_taxe_sur_les_salaires`).
pub(crate) fn taxe_sur_les_salaires_de_la_fiche(ent: &Value) -> Resultat<Option<Decimal>> {
    regime_taxe_sur_les_salaires(ent, None).decision
}

// ------------------------------------------------------------ les mois de l'annee

/// Une paie de l'annee : son salarie (`_cle`), et la paie calculee ou seulement son contrat.
enum MoisDeLAnnee {
    /// Les paies calculees du mois (le mois de la DSN, ou un mois saisi quand les montants sont necessaires).
    Calcules(String, Vec<Paie>),
    /// Les contrats saisis du mois, sans montants (`dsn::contrats_du_mois`).
    Contrats(Vec<Paie>),
    /// Un mois d'avant PuppetPaie (ou sans saisie) que seule une DSN deposee decrit.
    Depose(String, String),
}

fn lire_latin1(chemin: &Path) -> Resultat<String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{} illisible ({})", chemin.display(), e))?;
    Ok(octets.iter().map(|&b| b as char).collect())
}

/// Les douze mois de l'annee de la DSN, pour l'etablissement `nic`. Le mois de la DSN est `paies` (celles qu'elle
/// declare) ; un autre mois se lit dans sa saisie (`paies/AAAA-MM.json`), calcule si `montants`, sinon ses seuls
/// contrats ; sans saisie, dans la DSN deposee du mois (`casc::dsn_deposee` : celle du journal, ou celle de l'ancien
/// logiciel). Un mois sans rien n'a pas eu de salarie.
fn mois_de_l_annee(ctx: &Contexte, ent: &Value, nic: &str, mois: &str, paies: &[Paie], montants: bool) -> Resultat<Vec<MoisDeLAnnee>> {
    let an = &mois[..4];
    let mut out = Vec::new();
    let de_l_etablissement = |ps: Vec<Paie>| -> Resultat<Vec<Paie>> {
        let mut garde = Vec::new();
        for p in ps {
            if droit::bareme::etablissement_du_salarie(ent, &p.salarie)? == nic {
                garde.push(p);
            }
        }
        Ok(garde)
    };
    for m in 1..=12u32 {
        let m = format!("{}-{:02}", an, m);
        if m == mois {
            out.push(MoisDeLAnnee::Calcules(m, paies.to_vec()));
            continue;
        }
        let saisie = ctx.racine.join("paies").join(format!("{}.json", m));
        if saisie.is_file() {
            if montants {
                let s = engagement::charger(ctx, &saisie).map_err(|e| format!("{} : {}", m, e))?;
                out.push(MoisDeLAnnee::Calcules(m, de_l_etablissement(s.paies)?));
            } else {
                out.push(MoisDeLAnnee::Contrats(de_l_etablissement(crate::dsn::contrats_du_mois(ctx, &m)?)?));
            }
        } else if let Some(chemin) = crate::casc::dsn_deposee(ctx, &m, Some(nic))? {
            out.push(MoisDeLAnnee::Depose(m, lire_latin1(&chemin)?));
        }
    }
    Ok(out)
}

// ------------------------------------------------------------ la contribution CPF-CDD

/// Ce que la DSN ecrit d'un contrat calcule ou saisi : nature, dispositif, motif.
fn codes_du_contrat(p: &Paie) -> Resultat<(String, String, String)> {
    Ok(droit::cpf_cdd::codes_de(&*p.engagement.nat()?, p.engagement.motif_recours.as_deref()))
}

fn qui(p: &Paie) -> String {
    format!(
        "Contrat {} de {}",
        p.engagement.numero.clone().unwrap_or_else(|| "sans numero".into()),
        txt(g(&p.salarie, "_cle"))
    )
}

/// Les contrats d'une DSN deposee : (numero 40.009, nature 40.007, dispositif 40.008, motif 40.021).
fn contrats_deposes(texte: &str) -> Vec<(String, String, String, String)> {
    let mut out: Vec<(String, String, String, String)> = Vec::new();
    for l in crate::usages::lignes(texte) {
        let (r, v) = crate::usages::rubrique_valeur(l);
        match r {
            "S21.G00.40.001" => out.push(Default::default()),
            "S21.G00.40.007" | "S21.G00.40.008" | "S21.G00.40.009" | "S21.G00.40.021" => {
                if let Some(c) = out.last_mut() {
                    match r {
                        "S21.G00.40.007" => c.1 = v.to_string(),
                        "S21.G00.40.008" => c.2 = v.to_string(),
                        "S21.G00.40.009" => c.0 = v.to_string(),
                        _ => c.3 = v.to_string(),
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn cpf_cdd(annee: &[MoisDeLAnnee]) -> Resultat<bool> {
    let mut soumis = false;
    for m in annee {
        match m {
            MoisDeLAnnee::Calcules(_, ps) | MoisDeLAnnee::Contrats(ps) => {
                for p in ps {
                    let (n, d, mo) = codes_du_contrat(p)?;
                    soumis |= droit::cpf_cdd::ouvre_cpf_cdd(&n, &d, &mo, &qui(p))?;
                }
            }
            MoisDeLAnnee::Depose(mois, texte) => {
                for (numero, n, d, mo) in contrats_deposes(texte) {
                    soumis |= droit::cpf_cdd::ouvre_cpf_cdd(&n, &d, &mo, &format!("Contrat {} de la DSN deposee de {}", numero, mois))?;
                }
            }
        }
    }
    Ok(soumis)
}

// ------------------------------------------------------------ les assiettes de l'annee

/// La taxe sur les salaires de l'etablissement : les assiettes au taux normal, au premier et au second taux majore,
/// la part assujettie appliquee a chacune (CGI art. 231, 1 et 2 bis ; BOFiP BOI-TPS-TS-30, § 240 : « en appliquant le
/// rapport d'assujettissement [...] à chacune des fractions des rémunérations individuelles passibles des taux
/// majorés »).
fn assiettes_taxe_sur_les_salaires(annee: &[MoisDeLAnnee], bar: &Value, pct: Decimal) -> Resultat<(Decimal, Decimal, Decimal)> {
    let ts = g(bar, "taxe_sur_les_salaires");
    let seuil = |k: &str| -> Resultat<Decimal> {
        match ts.get(k) {
            Some(v) => dec_avec_echelle(v),
            None => Err(format!(
                "Le bareme de l'annee ne porte pas la taxe sur les salaires (`taxe_sur_les_salaires.{}`) : les seuils des taux \
majores se lisent sur la notice 2501-SD de l'annee.",
                k
            )),
        }
    };
    let (s1, s2) = (seuil("seuil_1")?, seuil("seuil_2")?);
    let abattement = match bar.get("abattement_csg").filter(|v| socle::fichiers::vrai(v)) {
        Some(v) => dec_avec_echelle(v)?,
        None => Decimal::ONE,
    };
    let mut par_salarie: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut deposes = Vec::new();
    for m in annee {
        match m {
            MoisDeLAnnee::Calcules(_, ps) => {
                for p in ps.iter().filter(|p| !p.engagement.sans_paie()) {
                    let b = par_salarie.entry(txt(g(&p.salarie, "_cle"))).or_insert(Decimal::ZERO);
                    *b = add(*b, assiette_taxe_sur_les_salaires(p, abattement));
                }
            }
            MoisDeLAnnee::Depose(mois, _) => deposes.push(mois.clone()),
            MoisDeLAnnee::Contrats(_) => return Err("assiettes de la taxe sur les salaires : un mois sans montants".into()),
        }
    }
    if !deposes.is_empty() {
        return Err(format!(
            "La taxe sur les salaires de l'annee se declare dans la DSN de decembre (S21.G00.44, codes 009, 015, 016), avec \
son assiette salarie par salarie, et {} ne {} que par une DSN deposee hors de PuppetPaie, qui ne donne pas l'assiette de \
la CSG ligne a ligne (la meme limite que l'etat « taxe sur les salaires »). Le moteur n'ecrit pas une assiette \
incomplete : saisir ces mois dans PuppetPaie.",
            deposes.join(", "),
            if deposes.len() > 1 { "se connaissent" } else { "se connait" }
        ));
    }
    let (mut normal, mut premier, mut second) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for b in par_salarie.values() {
        let (f1, f2) = fractions_majorees(*b, s1, s2);
        normal = add(normal, *b);
        premier = add(premier, f1);
        second = add(second, f2);
    }
    let part = |x: Decimal| eur(div(mul(x, pct), Decimal::ONE_HUNDRED));
    Ok((part(normal), part(premier), part(second)))
}

/// Les bases 03 (« assiette brute deplafonnee ») d'une DSN deposee dont la periode (78.002) est dans l'annee `an`.
fn bases_03_deposees(texte: &str, an: &str) -> Resultat<Decimal> {
    let mut total = Decimal::ZERO;
    let mut code = String::new();
    let mut dans_l_annee = false;
    for l in crate::usages::lignes(texte) {
        let (r, v) = crate::usages::rubrique_valeur(l);
        match r {
            "S21.G00.78.001" => {
                code = v.to_string();
                dans_l_annee = false;
            }
            "S21.G00.78.002" => dans_l_annee = v.len() == 8 && &v[4..] == an,
            "S21.G00.78.004" if code == "03" && dans_l_annee => {
                total = add(total, v.parse::<Decimal>().map_err(|_| format!("S21.G00.78.004 illisible : {}", v))?);
            }
            _ => {}
        }
    }
    Ok(total)
}

/// L'assiette de la PEEC de l'etablissement : les remunerations de l'annee qui servent de base aux cotisations de
/// securite sociale, la base 03 de chaque contrat (celle que la DSN de chaque mois a declaree ou declare).
fn assiette_peec(ctx: &Contexte, ent: &Value, orgs: &Value, nic: &str, an: &str, annee: &[MoisDeLAnnee]) -> Resultat<Decimal> {
    let mut total = Decimal::ZERO;
    for m in annee {
        match m {
            MoisDeLAnnee::Calcules(mois, ps) => {
                if ps.is_empty() {
                    continue;
                }
                let bar = crate::dsn::bar_du(ctx, ent, orgs, nic, mois)?;
                // Les periodes elementaires d'un mois decoupe (lot R10) : chacune a sa base 03.
                for p in ps.iter().flat_map(|p| p.elementaires()) {
                    if let Some(c) = crate::dsn::cotisations(p, &bar)?.into_iter().find(|c| c.bloc78 == "03") {
                        total = add(total, c.base78);
                    }
                }
            }
            MoisDeLAnnee::Depose(_, texte) => total = add(total, bases_03_deposees(texte, an)?),
            MoisDeLAnnee::Contrats(_) => return Err("assiette de la PEEC : un mois sans montants".into()),
        }
    }
    Ok(eur(total))
}

// ------------------------------------------------------------ le bloc

fn effectif_atteint(ctx: &Contexte, seuil: i64, annee_paie: i32, quoi: &str) -> Resultat<bool> {
    ctx.effectif()?.atteint(seuil, annee_paie).map_err(|m| format!("Bloc S21.G00.44 (assujettissement fiscal), {} : {}", quoi, m))
}

/// Les CTP du bordereau (`ctp_urssaf` du fichier des organismes), sans ceux de la taxe d'apprentissage (992, 993) quand
/// elle n'est pas due (`non_due` : `taxe_apprentissage_non_due_au_mois` pour le bordereau d'un mois,
/// `taxe_apprentissage_non_due` pour le bloc 44 d'un millesime).
pub(crate) fn ctp_sans_la_taxe_non_due(ctps: Vec<Value>, non_due: bool) -> Vec<Value> {
    if !non_due {
        return ctps;
    }
    ctps.into_iter().filter(|c| !CTP_TAXE_APPRENTISSAGE.contains(&txt(g(c, "code")).as_str())).collect()
}

/// Le bloc de la taxe d'apprentissage : 001 si le bordereau la paie (CTP 992 ou 993), 002 et son motif MNATA (44.004)
/// sinon, le motif lu dans la fiche entreprise. `ctp` : les codes du bordereau, deja sans la taxe quand le motif vaut
/// (`ctp_sans_la_taxe_non_due`). Un motif hors de la table, le motif 007 a partir du millesime 2026, une fiche qui dit
/// un motif quand le bordereau paie la taxe (garde-fou : `blocs` lui donne un bordereau qui ne la paie plus) : refus.
fn taxe_d_apprentissage(ent: &Value, ctp: &[String], an: i32) -> Resultat<Bloc44> {
    let paye = ctp.iter().any(|c| CTP_TAXE_APPRENTISSAGE.contains(&c.as_str()));
    let champ = champ_du_motif();
    let motif = motif_de_la_fiche(ent).map_err(|e| format!("DSN de decembre {} : {}", an, e))?;
    let Some((code, libelle)) = motif else {
        if paye {
            return Ok(Bloc44 { code: "001", montant: None, motif: None });
        }
        return Err(format!(
            "DSN de decembre {} : config/organismes.json ne paie pas la taxe d'apprentissage (ni CTP {} ni CTP {} dans \
`ctp_urssaf`), et le bloc S21.G00.44 declare donc « 002 - Non assujettissement », qui exige son motif (S21.G00.44.004, \
table MNATA de la norme). La fiche ne le dit pas : {}. Si la compagnie doit la taxe (code du travail L. 6241-1), \
recomposer les organismes (Configuration › Organismes et contrats). Rien n'est ecrit.",
            an, CTP_TAXE_APPRENTISSAGE[0], CTP_TAXE_APPRENTISSAGE[1], champ
        ));
    };
    if code == MNATA_ASSOCIATIONS && an >= MILLESIME_ASSOCIATIONS_REDEVABLES {
        return Err(format!(
            "DSN de decembre {} : la fiche dit la compagnie non assujettie a la taxe d'apprentissage au motif {} « {} ». \
Depuis le 21/02/2026, le code du travail (L. 6241-1, I) dit qu'« en sont également redevables les associations [...] à \
activités non lucratives » : pour le millesime {}, l'association la doit. Effacer le motif ({}) et recomposer les \
organismes (Configuration › Organismes et contrats) pour que le bordereau la paie. Rien n'est ecrit.",
            an, code, libelle, an, champ
        ));
    }
    if paye {
        return Err(format!(
            "DSN de decembre {} : la fiche dit la compagnie non assujettie a la taxe d'apprentissage (motif MNATA {}), et le \
bordereau la paie (CTP {} ou {} de config/organismes.json). L'un des deux est faux : {}, ou les organismes. Rien \
n'est ecrit.",
            an, code, CTP_TAXE_APPRENTISSAGE[0], CTP_TAXE_APPRENTISSAGE[1], champ
        ));
    }
    // Le code vient de la table livree : `Bloc44.code` est statique, le motif ne l'est pas.
    Ok(Bloc44 { code: "002", montant: None, motif: Some(code) })
}

/// Les blocs S21.G00.44 de la DSN mensuelle de `mois` pour l'etablissement `nic`, dans l'ordre des codes : aucun hors de
/// decembre (la norme ne les demande qu'une fois l'an, « au plus tard » en decembre ; le moteur ne connait pas la
/// cessation d'activite, seul autre cas de la p. 58). `ent` : la fiche entreprise ; `ent_e` : vue depuis
/// l'etablissement ; `bar` : le bareme du mois pour l'etablissement ; `paies` : celles que la DSN declare.
pub(crate) fn blocs(
    ctx: &Contexte, ent: &Value, ent_e: &Value, orgs: &Value, bar: &Value, nic: &str, mois: &str, paies: &[Paie],
) -> Resultat<Vec<Bloc44>> {
    if mois.len() != 7 || &mois[5..7] != "12" {
        return Ok(Vec::new());
    }
    let an_txt = &mois[..4];
    let an: i32 = an_txt.parse().map_err(|_| format!("mois illisible : {}", mois))?;
    // Le bordereau que la DSN ecrit : sans la taxe d'apprentissage quand la fiche dit un motif qui vaut (R12).
    let ctp: Vec<String> =
        ctp_sans_la_taxe_non_due(liste(g(orgs, "ctp_urssaf")).to_vec(), taxe_apprentissage_non_due(ent, an)?)
            .iter()
            .map(|c| txt(g(c, "code")))
            .collect();
    let mut out = Vec::new();

    // La taxe d'apprentissage : ce que le bordereau paie ; sinon 002 et le motif que la fiche dit.
    out.push(taxe_d_apprentissage(ent, &ctp, an)?);

    // La contribution supplementaire : L. 6242-1, l'effectif de l'annee du millesime (V), la regle des cinq ans.
    let csa = if effectif_atteint(ctx, 250, an + 1, "contribution supplementaire a l'apprentissage")? {
        match g(ent, CHAMP_CONTRIBUTION_SUPPLEMENTAIRE).as_str() {
            Some("due") => "003",
            Some("non_due") => "004",
            _ => {
                return Err(format!(
                    "DSN de decembre {} : la compagnie compte au moins 250 salaries depuis cinq ans, et la contribution \
supplementaire a l'apprentissage (code du travail L. 6242-1) est due si moins de 5 % de son effectif annuel est en \
contrat d'apprentissage, de professionnalisation, en CIFRE ou en CDI a leur suite. Le moteur ne compte pas ces contrats : \
le dire dans Configuration › Entreprise, « Contribution supplementaire a l'apprentissage » (`{}` : \"due\" ou \
\"non_due\"). Rien n'est ecrit.",
                    an, CHAMP_CONTRIBUTION_SUPPLEMENTAIRE
                ))
            }
        }
    } else {
        "004"
    };
    out.push(Bloc44 { code: csa, montant: None, motif: None });

    // Ce qui demande les montants de l'annee : la taxe sur les salaires et la PEEC.
    let ts = taxe_sur_les_salaires_de_la_fiche(ent).map_err(|m| format!("DSN de decembre {}, bloc S21.G00.44 : {} Rien n'est ecrit.", an, m))?;
    if ts.is_some() {
        let cp = txt(g(ent_e, "code_postal"));
        if cp.starts_with("97") || cp.starts_with("98") {
            return Err(format!(
                "DSN de decembre {} : l'etablissement {} est outre-mer (code postal {}), ou la taxe sur les salaires a un taux \
unique et pas de taux majores (CGI art. 231, 2 bis et 5). Le moteur ne connait que le bareme de metropole et n'ecrit \
pas son assiette. Rien n'est ecrit.",
                an, nic, cp
            ));
        }
    }
    // L'effectif est celui de l'annee du millesime, comme pour la contribution supplementaire : la PEEC declaree ici est
    // assise sur les salaires de cette annee-la et s'investit l'annee d'apres (BOFiP BOI-TPS-PEEC-20, § 10), et L. 313-1
    // renvoie a L. 130-1 CSS, qui retient l'effectif de l'annee civile precedant l'obligation (INFERENCE, passation).
    let peec = effectif_atteint(ctx, 50, an + 1, "participation des employeurs a l'effort de construction")?;
    let annee = mois_de_l_annee(ctx, ent, nic, mois, paies, ts.is_some() || peec)?;

    // La PEEC : CCH L. 313-1, cinquante salaries.
    out.push(if peec {
        Bloc44 { code: "005", montant: Some(assiette_peec(ctx, ent, orgs, nic, an_txt, &annee)?), motif: None }
    } else {
        Bloc44 { code: "006", montant: None, motif: None }
    });

    // La formation professionnelle : ce que le bordereau paie.
    out.push(Bloc44 { code: if ctp.iter().any(|c| CTP_FORMATION.contains(&c.as_str())) { "007" } else { "008" }, montant: None, motif: None });

    // La taxe sur les salaires : ce que la fiche dit, les assiettes de l'annee.
    let mut majorees = Vec::new();
    match ts {
        None => out.push(Bloc44 { code: "010", montant: None, motif: None }),
        Some(pct) => {
            let (normal, premier, second) = assiettes_taxe_sur_les_salaires(&annee, bar, pct)?;
            out.push(Bloc44 { code: "009", montant: Some(normal), motif: None });
            if premier > Decimal::ZERO {
                majorees.push(Bloc44 { code: "015", montant: Some(premier), motif: None });
            }
            if second > Decimal::ZERO {
                majorees.push(Bloc44 { code: "016", montant: Some(second), motif: None });
            }
        }
    }

    // La contribution CPF-CDD : les contrats de l'annee.
    out.push(Bloc44 { code: if cpf_cdd(&annee)? { "013" } else { "014" }, montant: None, motif: None });
    out.extend(majorees);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn la_fiche_propose_les_motifs_de_la_table_mnata_livree() {
        let motifs = droit::apprentissage::motifs_mnata().unwrap();
        assert_eq!(motifs.len(), 19);
        assert_eq!(motifs.iter().find(|(c, _)| c == "007").map(|(_, l)| l.as_str()), Some("Associations sans but lucratif"));
        let champs = socle::fichiers::json_en_cache(&socle::fichiers::dossier_du_code().join("champs-base.json")).unwrap();
        let champ = liste(g(&champs, "entreprise")).iter().find(|c| txt(g(c, "cle")) == droit::apprentissage::CHAMP_MOTIF_TAXE_APPRENTISSAGE).cloned().unwrap();
        let choix: Vec<String> = liste(g(&champ, "choix")).iter().map(|c| c.as_str().unwrap().to_string()).filter(|c| !c.is_empty()).collect();
        assert_eq!(choix, motifs.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>());
    }

    #[test]
    fn les_fractions_majorees_suivent_le_2_bis() {
        let (s1, s2) = (d("9147"), d("18259"));
        assert_eq!(fractions_majorees(d("5000"), s1, s2), (Decimal::ZERO, Decimal::ZERO));
        assert_eq!(fractions_majorees(d("10000"), s1, s2), (d("853"), Decimal::ZERO));
        // L'exemple du BOFiP (BOI-TPS-TS-30, § 240), seuils 2020 : 180 000 -> 7 977 et 164 019.
        assert_eq!(fractions_majorees(d("180000"), d("8004"), d("15981")), (d("7977"), d("164019")));
    }

    #[test]
    fn une_dsn_deposee_donne_ses_contrats_et_ses_bases_03_de_l_annee() {
        let t = "S21.G00.40.001,'01122025'\r\nS21.G00.40.007,'02'\r\nS21.G00.40.008,'99'\r\nS21.G00.40.009,'C1'\r\n\
S21.G00.40.021,'05'\r\nS21.G00.78.001,'03'\r\nS21.G00.78.002,'01122025'\r\nS21.G00.78.003,'31122025'\r\n\
S21.G00.78.004,'1000.00'\r\nS21.G00.78.001,'03'\r\nS21.G00.78.002,'01112024'\r\nS21.G00.78.003,'30112024'\r\n\
S21.G00.78.004,'-50.00'\r\nS21.G00.78.001,'02'\r\nS21.G00.78.002,'01122025'\r\nS21.G00.78.004,'900.00'\r\n";
        assert_eq!(contrats_deposes(t), vec![("C1".into(), "02".into(), "99".into(), "05".into())]);
        assert_eq!(bases_03_deposees(t, "2025").unwrap(), d("1000.00"), "la regularisation de 2024 et la base 02 n'en sont pas");
    }
}
