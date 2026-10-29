//! La taxe sur les salaires (CGI art. 231) : ce que la fiche entreprise en dit et ce que le moteur en conclut, les
//! plafonds de la franchise en base de TVA, l'assiette d'une paie et les fractions aux taux majores. L'etat « taxe sur
//! les salaires » (`etats_compta`) et le bloc S21.G00.44 de la DSN de decembre (`declarations::assujettissement`) lisent
//! les memes fonctions.

use serde_json::Value;

use engagement::calcul::Paie;
use socle::num::{add, dec_avec_echelle, eur, mul, sub, Decimal, Resultat};

/// Le champ de la fiche entreprise qui dit la taxe sur les salaires.
pub const CHAMP_TAXE_SUR_LES_SALAIRES: &str = "taxe_sur_les_salaires";

const NUL: Value = Value::Null;

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NUL)
}

// ------------------------------------------------------------ la taxe sur les salaires, salarie par salarie

/// L'assiette de la taxe sur les salaires d'une paie : celle de la CSG, sans la reduction de 1,75 % (CGI art. 231, 1 :
/// « La réduction mentionnée au I de l'article L. 136-2 du code de la sécurité sociale n'est pas applicable »).
/// `abattement` : le facteur de la CSG du bareme (`abattement_csg`, 0,9825). Le calcul de l'etat « taxe sur les
/// salaires » (`etats_compta`), mis ici pour que la DSN lise le meme.
pub fn assiette_taxe_sur_les_salaires(p: &Paie, abattement: Decimal) -> Decimal {
    let csg = p.bases.get("csg").copied().unwrap_or(Decimal::ZERO);
    eur(add(sub(csg, eur(mul(p.brut, abattement))), p.brut))
}

/// Les fractions d'une remuneration individuelle annuelle soumises aux taux majores (CGI art. 231, 2 bis) : celle
/// comprise entre `seuil_1` et `seuil_2`, et celle qui excede `seuil_2`.
pub fn fractions_majorees(base: Decimal, seuil_1: Decimal, seuil_2: Decimal) -> (Decimal, Decimal) {
    let positif = |x: Decimal| if x > Decimal::ZERO { x } else { Decimal::ZERO };
    let premiere = positif(sub(if base < seuil_2 { base } else { seuil_2 }, seuil_1));
    (premiere, positif(sub(base, seuil_2)))
}

/// LA TAXE SUR LES SALAIRES SE DEDUIT DE LA TVA. La fiche disait « la compagnie la doit-elle ? »
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
/// on les a, elle dit « les plafonds » sinon. Ils ne changent rien a la decision.
pub fn regime_taxe_sur_les_salaires(ent: &Value, plafonds: Option<(Decimal, Decimal)>) -> RegimeTaxeSalaires {
    // Les deux plafonds, en mots de compagnie, pour la conclusion.
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

/// LES PLAFONDS DE LA FRANCHISE EN BASE DE TVA, EN CLAIR. L'ecran demandait « sous les plafonds
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

/// Un montant entier, ecrit comme sur l'ecran : « 37 500 € », en espaces insecables (un montant ne se coupe
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
pub fn taxe_sur_les_salaires_de_la_fiche(ent: &Value) -> Resultat<Option<Decimal>> {
    regime_taxe_sur_les_salaires(ent, None).decision
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn les_fractions_majorees_suivent_le_2_bis() {
        let (s1, s2) = (d("9147"), d("18259"));
        assert_eq!(fractions_majorees(d("5000"), s1, s2), (Decimal::ZERO, Decimal::ZERO));
        assert_eq!(fractions_majorees(d("10000"), s1, s2), (d("853"), Decimal::ZERO));
        // L'exemple du BOFiP (BOI-TPS-TS-30, § 240, `atelier/reference/sources/bofip/BOI-TPS-TS-30-20200624.html`), seuils
        // 2020 : 180 000 -> 7 977 et 164 019.
        assert_eq!(fractions_majorees(d("180000"), d("8004"), d("15981")), (d("7977"), d("164019")));
    }
}
