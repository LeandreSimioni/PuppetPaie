//! La contribution supplementaire a l'apprentissage (CSA, lot D2) : la declaration annuelle que la DSN de mars N+1
//! porte, sur l'etablissement principal, au titre de l'annee N.
//!
//! LES TEXTES (deposes ; les lignes citees sont celles des fichiers) :
//! - C. trav. L. 6242-1 (`atelier/reference/sources/code-du-travail/articles-20261013.md`) : due par les entreprises d'au
//!   moins 250 salaries redevables de la taxe d'apprentissage dont le quotient « de l'effectif annuel salarie » des
//!   alternants (apprentissage, professionnalisation, CDI a leur suite, CIFRE) « par l'effectif total » est inferieur a
//!   5 % (I) ; exoneree si ce quotient atteint 3 % et que cet effectif « a progressé d'au moins 10 % par rapport à
//!   l'année précédente » (II) ; assise sur l'assiette de la taxe d'apprentissage (III) ; taux 0,4 % sous 1 % (0,6 %
//!   « lorsque l'effectif salarié annuel excède deux mille salariés »), 0,2 % de 1 a 2 %, 0,1 % de 2 a 3 %, 0,05 % de 3 a
//!   5 % (IV) ; l'effectif de L. 130-1 CSS, apprecie sur l'annee au titre de laquelle elle est due, la regle de
//!   franchissement de son II comprise (V) ; 52 % de ces taux pour les etablissements d'Alsace-Moselle (VI) ; le montant
//!   et l'assiette arrondis selon L. 133-10 CSS (VII) ; un « versement unique » avec les cotisations de la periode de
//!   mars de l'annee suivante (VIII).
//! - Fiche net-entreprises 2588 et 2963 (`atelier/reference/sources/net-entreprises-base-connaissances/`) : la DSN de mars
//!   N+1, « sur un seul établissement », un bloc S21.G00.82 de code 079 (montant ; 82.003 et 82.004 du 01/01 au 31/12 de
//!   N ; 82.005 le SIRET de l'Urssaf) et le CTP au bloc 23 ; l'exoneration, un bloc 82 de code 080 a 0.00.
//! - Guide Urssaf des contributions de formation et d'apprentissage du 12/03/2026
//!   (`atelier/reference/sources/majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt`,
//!   l. 2441-2463) : « Le montant déclaré correspond au montant dû par l'entreprise pour l'ensemble de ses
//!   établissements » ; « Sur le CTP 998, en bloc « Cotisation agrégée - S21.G00.23.004 » » ; « Le CTP 998 a un taux à
//!   100% » (la table des CTP livree le dit aussi : `logiciel/donnees/urssaf-codes-types-personnel.csv`).
//! - Fiche net-entreprises 2624 (`net-entreprises-fiche2624-crm-effectifs-20250429.txt`) : les effectifs que l'Urssaf
//!   calcule et renvoie en compte rendu metier de nature 118 (« Début mars : pour la remontée des effectifs EMA CSA et
//!   CSA CFIP ») : A01 l'effectif moyen annuel standard, A05 celui des contrats favorisant l'insertion professionnelle
//!   (le numerateur), A07 celui « de détermination du taux d'alternants » (le denominateur). Le moteur les ecrit dans
//!   `config/effectifs.json` (`releve_retours::reconstruire_effectifs`) ; ils ne se saisissent pas (regle 1).
//!
//! CE QUI SE LIT, ET D'OU : l'effectif d'assujettissement (250) et celui des 2 000 salaries, A01 avec la regle des cinq
//! ans (`Effectif`), comme le bloc 44 de decembre (`assujettissement::blocs`) ; le quotient, A05 / A07 de l'annee N ; la
//! progression, A05 de N contre A05 de N-1 ; l'assiette, celle de la taxe d'apprentissage que les DSN de N declarent, tiree
//! des paies declarees (`dsn::assiettes_de_l_annee` ; CTP 992, et 993 pour l'Alsace-Moselle), etablissement par
//! etablissement, comme le solde de la taxe d'avril (`dsn::solde_taxe_apprentissage`). Ce qui manque refuse, en disant quoi.
//!
//! INFERENCES (passation du lot D2) : l'assiette des etablissements d'Alsace-Moselle est celle de leur CTP 993 ; la
//! « progression d'au moins 10 % » compare A05 de N a A05 de N-1 (le guide Urssaf, l. 2431-2433, la calcule ainsi :
//! « [(17-15) x 100] / 15 ») ; l'effectif qui « excède deux mille » suit, comme celui des 250, la regle de
//! franchissement de L. 130-1, II (L. 6242-1, V, le dit des deux seuils).

use droit::contexte::Contexte;
use socle::fichiers::lire_json;
use socle::num::{add, div, mul, Decimal, Resultat};
use serde_json::Value;

const MOIS_DE_DECLARATION: &str = "03";
pub const CODE_CONTRIBUTION: &str = "079";
pub const CODE_EXONERATION: &str = "080";
pub const CTP: &str = "998";
const SEUIL: i64 = 250;
const SEUIL_DEUX_MILLE: f64 = 2000.0;
const CODE_ALTERNANTS: &str = "A05";
const CODE_DENOMINATEUR: &str = "A07";

/// Ce que la DSN de mars porte pour un millesime.
#[derive(Clone, Debug)]
pub struct Csa {
    pub millesime: i32,
    /// « 079 » (la contribution) ou « 080 » (l'exoneration, a 0.00).
    pub code: &'static str,
    /// Le montant, a l'euro (L. 6242-1, VII).
    pub montant: Decimal,
    /// Le quotient A05 / A07, en pourcentage.
    pub quotient: Decimal,
    /// Le taux applique, en pourcentage (zero pour l'exoneration).
    pub taux: Decimal,
    /// Les assiettes de la taxe d'apprentissage declarees pour N : hors et en Alsace-Moselle.
    pub assiette: Decimal,
    pub assiette_alsace_moselle: Decimal,
}

impl Csa {
    /// Le bloc S21.G00.82 : (code, montant).
    pub fn bloc_82(&self) -> (&'static str, Decimal) {
        (self.code, self.montant)
    }

    /// Ce qu'elle ajoute au bordereau Urssaf : le CTP 998 « en montant » (taux de 100 %), rien pour l'exoneration.
    pub fn bordereau(&self) -> Option<(&'static str, Decimal)> {
        (self.code == CODE_CONTRIBUTION && !self.montant.is_zero()).then_some((CTP, self.montant))
    }

    /// La phrase que `produire` dit au mois de mars.
    pub fn resume(&self) -> String {
        if self.code == CODE_EXONERATION {
            format!(
                "Contribution supplementaire a l'apprentissage {} : exoneree (alternants {} % de l'effectif), declaree en \
code 080 a 0.00.",
                self.millesime, self.quotient
            )
        } else {
            format!(
                "Contribution supplementaire a l'apprentissage {} : {} EUR (alternants {} % de l'effectif, taux {} % sur \
{} EUR d'assiette de la taxe d'apprentissage{}).",
                self.millesime,
                self.montant,
                self.quotient,
                self.taux,
                add(self.assiette, self.assiette_alsace_moselle),
                if self.assiette_alsace_moselle.is_zero() {
                    String::new()
                } else {
                    format!(", dont {} EUR en Alsace-Moselle a 52 % du taux", self.assiette_alsace_moselle)
                }
            )
        }
    }
}

/// Le taux de L. 6242-1, IV, en pourcentage, pour un quotient (en pourcentage) sous 5 % ; `None` a 5 % et plus.
pub fn taux(quotient: Decimal, plus_de_deux_mille: bool) -> Option<Decimal> {
    let p = |x: i64, e: u32| Decimal::new(x, e);
    if quotient < p(1, 0) {
        Some(if plus_de_deux_mille { p(6, 1) } else { p(4, 1) })
    } else if quotient < p(2, 0) {
        Some(p(2, 1))
    } else if quotient < p(3, 0) {
        Some(p(1, 1))
    } else if quotient < p(5, 0) {
        Some(p(5, 2))
    } else {
        None
    }
}

/// La valeur d'un code du CRM 118 pour une annee (`config/effectifs.json`), ou `None`.
fn effectif_du_118(effectifs: &Value, annee: i32, code: &str) -> Resultat<Option<Decimal>> {
    match effectifs.get("annees").and_then(|a| a.get(annee.to_string())).and_then(|a| a.get(code)).and_then(|c| c.get("valeur")) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => Ok(Some(socle::num::dec_avec_echelle(v)?)),
    }
}

/// L'effectif « excède deux mille salariés » pour l'annee N : au-dessus les cinq annees civiles N-4 a N (L. 6242-1, V ;
/// L. 130-1, II, CSS). Une annee a 2 000 ou moins suffit a dire non ; une annee inconnue, sans annee sous le seuil,
/// refuse.
fn excede_deux_mille(ctx: &Contexte, n: i32) -> Resultat<bool> {
    let e = ctx.effectif()?;
    let mut inconnues = Vec::new();
    for a in n - 4..=n {
        match e.annee(a) {
            Some(v) if v > SEUIL_DEUX_MILLE => {}
            Some(_) => return Ok(false),
            None => inconnues.push(a.to_string()),
        }
    }
    if inconnues.is_empty() {
        return Ok(true);
    }
    Err(format!(
        "Contribution supplementaire a l'apprentissage {} : l'effectif depasse 250 salaries, mais le moteur ne sait pas s'il \
excede 2 000 (le taux passe de 0,4 a 0,6 %, L. 6242-1, IV) : l'effectif moyen annuel de {} manque. L'Urssaf le renvoie en \
compte rendu metier de nature 118 (code A01) : relever les retours. Rien n'est ecrit.",
        n,
        inconnues.join(", ")
    ))
}

/// Ce que la DSN de mars de l'annee N+1 porte au titre de N, ou `None` (un autre mois ; une compagnie qui ne doit pas la
/// taxe d'apprentissage pour N, ou qui n'atteint pas 250 salaries). Le moteur la porte sur l'etablissement principal.
pub fn declaration(ctx: &Contexte, mois: &str) -> Resultat<Option<Csa>> {
    if mois.len() != 7 || &mois[5..7] != MOIS_DE_DECLARATION {
        return Ok(None);
    }
    let n: i32 = mois[..4].parse::<i32>().map_err(|_| format!("mois illisible : {}", mois))? - 1;
    // Redevable de la taxe d'apprentissage (L. 6242-1, I) : le motif de non-assujettissement de la fiche (lot R2).
    if droit::apprentissage::taxe_apprentissage_non_due(&ctx.entreprise, n)? {
        return Ok(None);
    }
    // Au moins 250 salaries : l'effectif de l'annee N, la regle des cinq ans (L. 6242-1, V), comme au bloc 44 de decembre.
    let assujettie = ctx.effectif()?.atteint(SEUIL, n + 1).map_err(|m| {
        format!("DSN de mars {}, contribution supplementaire a l'apprentissage {} : {}", n + 1, n, m)
    })?;
    if !assujettie {
        return Ok(None);
    }
    let chemin = ctx.racine.join("config").join("effectifs.json");
    let effectifs = if socle::fichiers::existe(&chemin) { lire_json(&chemin)? } else { Value::Object(Default::default()) };
    let (alternants, denominateur) =
        (effectif_du_118(&effectifs, n, CODE_ALTERNANTS)?, effectif_du_118(&effectifs, n, CODE_DENOMINATEUR)?);
    let (Some(alternants), Some(denominateur)) = (alternants, denominateur) else {
        return Err(format!(
            "DSN de mars {} : la compagnie compte au moins 250 salaries et doit la taxe d'apprentissage ; la contribution \
supplementaire a l'apprentissage {} (code du travail L. 6242-1) se calcule sur le quotient de ses alternants, que l'Urssaf \
calcule et renvoie debut mars en compte rendu metier de nature 118 (fiche net-entreprises 2624) : l'effectif moyen annuel \
des contrats favorisant l'insertion professionnelle (code {}) et celui de determination du taux d'alternants (code {}). \
config/effectifs.json ne les a pas pour {} : relever les retours (`paie retours`), le moteur les y ecrit. Ils ne se \
saisissent pas. Rien n'est ecrit.",
            n + 1,
            n,
            CODE_ALTERNANTS,
            CODE_DENOMINATEUR,
            n
        ));
    };
    if denominateur <= Decimal::ZERO {
        return Err(format!(
            "DSN de mars {} : le compte rendu metier 118 donne un effectif de determination du taux d'alternants ({}) nul \
pour {} : le quotient de la contribution supplementaire a l'apprentissage ne se calcule pas. Rien n'est ecrit.",
            n + 1,
            CODE_DENOMINATEUR,
            n
        ));
    }
    let quotient = div(mul(alternants, Decimal::ONE_HUNDRED), denominateur);
    let quotient_lu = quotient.round_dp(3).normalize();
    let exoneree = |csa_quotient: Decimal| Csa {
        millesime: n,
        code: CODE_EXONERATION,
        montant: Decimal::new(0, 2),
        quotient: csa_quotient,
        taux: Decimal::ZERO,
        assiette: Decimal::ZERO,
        assiette_alsace_moselle: Decimal::ZERO,
    };
    let plus_de_deux_mille = quotient < Decimal::ONE && excede_deux_mille(ctx, n)?;
    let Some(t) = taux(quotient, plus_de_deux_mille) else {
        return Ok(Some(exoneree(quotient_lu)));
    };
    if quotient >= Decimal::new(3, 0) {
        // L. 6242-1, II, dernier alinea : 3 % et une progression d'au moins 10 % sur l'annee precedente.
        let Some(avant) = effectif_du_118(&effectifs, n - 1, CODE_ALTERNANTS)? else {
            return Err(format!(
                "DSN de mars {} : les alternants font {} % de l'effectif en {}, entre 3 et 5 % : la contribution \
supplementaire a l'apprentissage n'est pas due si leur effectif a progresse d'au moins 10 % sur {} (L. 6242-1, II). \
config/effectifs.json n'a pas l'effectif des alternants de {} (code {} du compte rendu metier 118) : relever les retours \
de {}. Rien n'est ecrit.",
                n + 1,
                quotient_lu,
                n,
                n - 1,
                n - 1,
                CODE_ALTERNANTS,
                n
            ));
        };
        if mul(alternants, Decimal::from(10)) >= mul(avant, Decimal::from(11)) {
            return Ok(Some(exoneree(quotient_lu)));
        }
    }
    // L'assiette : celle de la taxe d'apprentissage que les DSN de N declarent, etablissement par etablissement (le
    // montant « pour l'ensemble de ses établissements »).
    let (mut assiette, mut alsace) = (Decimal::ZERO, Decimal::ZERO);
    for nic in crate::dsn::nics(&ctx.entreprise) {
        let a = crate::dsn::assiettes_de_l_annee(ctx, n, &nic, &["992", "993"])?;
        assiette = add(assiette, a[0]);
        alsace = add(alsace, a[1]);
    }
    if add(assiette, alsace).is_zero() {
        return Err(format!(
            "DSN de mars {} : la contribution supplementaire a l'apprentissage {} est due (alternants {} % de l'effectif), \
et aucune paie validee de {} ne porte d'assiette de la taxe d'apprentissage (CTP 992 ou 993) : elle se calcule sur cette \
assiette (L. 6242-1, III). Rien n'est ecrit.",
            n + 1,
            n,
            quotient_lu,
            n
        ));
    }
    // VI : 52 % des taux en Alsace-Moselle ; VII : le montant a l'euro (L. 133-10 CSS).
    let cinquante_deux = Decimal::new(52, 2);
    let brut = div(add(mul(assiette, t), mul(mul(alsace, t), cinquante_deux)), Decimal::ONE_HUNDRED);
    let mut montant = documents::bordereau::euro(brut);
    montant.rescale(2);
    Ok(Some(Csa {
        millesime: n,
        code: CODE_CONTRIBUTION,
        montant,
        quotient: quotient_lu,
        taux: t,
        assiette,
        assiette_alsace_moselle: alsace,
    }))
}

#[cfg(test)]
mod tests {
    use super::taux;
    use rust_decimal::Decimal;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn les_taux_suivent_les_tranches_du_quotient() {
        // L. 6242-1, IV (`atelier/reference/sources/code-du-travail/xml/L6242-1.xml`) ; fiches net-entreprises 2588 et 2963.
        assert_eq!(taux(d("0.5"), false), Some(d("0.4")));
        assert_eq!(taux(d("0.5"), true), Some(d("0.6")));
        assert_eq!(taux(d("1"), true), Some(d("0.2")), "le 0,6 % ne vaut que sous 1 %");
        assert_eq!(taux(d("2.99"), false), Some(d("0.1")));
        assert_eq!(taux(d("3"), false), Some(d("0.05")));
        assert_eq!(taux(d("4.999"), false), Some(d("0.05")));
        assert_eq!(taux(d("5"), false), None, "a 5 % : exoneree");
    }
}
