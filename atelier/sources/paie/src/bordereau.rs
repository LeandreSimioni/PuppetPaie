//! Le montant du bordereau Urssaf, CALCULE depuis ses lignes (blocs 22 et 23).
//!
//! En DSN, le bordereau EST le bloc 22, et son montant est la somme de ses
//! lignes 23, chacune valorisee au taux que la table des CTP de l'Urssaf donne
//! pour la periode. CoolPaie ne fait pas autre chose : rejouee sur ses 75 DSN
//! deposees (2021-2026), la regle ci-dessous retrouve 74 bordereaux au centime.
//! Le 75e, octobre 2025, donne 394 pour 393 declares : les lignes arrondies une
//! a une font 394 (0 + 113 + 109 + 14 + 126 + 1 + 23 + 8), la somme brute
//! 393,23 -- un euro que CoolPaie arrondissait autrement ce mois-la, et
//! qu'aucune variante n'explique sans en casser d'autres.
//!
//! La regle, ligne par ligne :
//!
//! - format A, C, D, E, K (une assiette, S21.G00.23.004) : assiette x taux, le
//!   taux PLAFONNE pour le qualifiant 921, DEPLAFONNE pour le 920, plus le taux
//!   AT/MP que la ligne porte (S21.G00.23.003) VALORISE A PART : assiette x
//!   taux arrondi, plus assiette x taux AT arrondi -- 74 bordereaux sur 75 de
//!   CoolPaie au centime ainsi, 60 seulement en additionnant les taux ;
//! - format Z (versement mobilite) : l'Urssaf n'a pas de taux national, c'est
//!   la ligne qui le porte (S21.G00.23.003, le taux de l'autorite) ;
//! - format F (reductions et exonerations, un montant S21.G00.23.005) : le
//!   montant, en DEDUCTION (mai 2025 : 367, et non 382 en l'ajoutant) ;
//! - chaque ligne arrondie a l'euro (guide Urssaf v5.1, §1.6 : << 22 et 23
//!   arrondis a l'euro >>), puis la somme.
//!
//! Un code absent de la table, ou d'un format que ce module ne connait pas,
//! fait REFUSER : un ordre de telepaiement ne se devine pas (regle 1).
//!
//! La table vit a cote du code (`donnees/urssaf-codes-types-personnel.csv`, la
//! copie de la source `urssaf-codes-types-personnel` de `sources.json`) : le
//! calcul d'un mois depend de la version du logiciel, pas de la veille du
//! poste -- c'est ce qui rend le rejeu possible. ATTENTION, ses dates sont en
//! AAAA-JJ-MM (l'AGS passe a 0,25 le << 2024-01-07 >>, le 1er juillet 2024).
//! Preuves : `tests/calcul/bordereau.rs`, dont le rejeu des 75 bordereaux
//! (`tests/donnees/bordereaux-reels.json`).

use crate::date::Jour;
use crate::fichiers::{dossier_du_code, signature};
use crate::num::{div, mul, Decimal, Resultat};
use rust_decimal::RoundingStrategy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

const NOM_TABLE: &str = "urssaf-codes-types-personnel.csv";

/// Une ligne 23, en donnee : ce que la DSN ecrit en 001 a 005. `assiette`
/// (formats A, C, D, E, K, Z) ou `montant` (format F), jamais les deux.
#[derive(Clone, Debug)]
pub struct LigneBordereau {
    pub code: String,
    pub qualifiant: String,
    /// Le taux AT/MP en texte, vide si la ligne n'en porte pas.
    pub taux_at: String,
    pub assiette: Option<Decimal>,
    pub montant: Option<Decimal>,
    pub commune: Option<String>,
}

/// Une ligne de la table : effet, fin, format, taux plafonne, taux deplafonne, libelle.
struct Ligne {
    effet: Jour,
    fin: Option<Jour>,
    format: String,
    plafonne: Option<Decimal>,
    deplafonne: Option<Decimal>,
    libelle: String,
}

type Table = HashMap<String, Vec<Ligne>>;

/// Une date de la table : AAAA-JJ-MM, sauf quand le jour depasse 12 (alors
/// AAAA-MM-JJ) ; vide, rien.
fn date_table(texte: &str) -> Resultat<Option<Jour>> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Ok(None);
    }
    let p: Vec<&str> = texte.split('-').collect();
    if p.len() != 3 {
        return Err(format!("{} valeurs a decompacter, 3 attendues : {:?}", p.len(), texte));
    }
    let n = |s: &str| -> Resultat<i64> {
        s.trim().parse::<i64>().map_err(|_| format!("nombre entier illisible : {:?}", s))
    };
    let (an, x, y) = (n(p[0])?, n(p[1])?, n(p[2])?);
    let (mois, jour) = if y > 12 { (x, y) } else { (y, x) };
    let j = if (1..=9999).contains(&an) && (1..=12).contains(&mois) && (1..=31).contains(&jour) {
        Jour::nouveau(an as i32, mois as u32, jour as u32)
    } else {
        None
    };
    j.map(Some).ok_or_else(|| crate::date::date_impossible(an, mois, jour))
}

fn taux(texte: &str) -> Resultat<Option<Decimal>> {
    let texte = texte.trim();
    if texte.is_empty() {
        return Ok(None);
    }
    Decimal::from_str(texte).map(Some).map_err(|_| crate::num::illisible(texte))
}

/// Les enregistrements d'un CSV a point-virgule (guillemets doubles admis).
fn enregistrements(texte: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rec: Vec<String> = Vec::new();
    let mut champ = String::new();
    let mut guillemets = false;
    let mut vu = false;
    let mut it = texte.chars().peekable();
    while let Some(c) = it.next() {
        if guillemets {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    champ.push('"');
                    it.next();
                } else {
                    guillemets = false;
                }
            } else {
                champ.push(c);
            }
            continue;
        }
        match c {
            '"' if champ.is_empty() => {
                guillemets = true;
                vu = true;
            }
            ';' => {
                rec.push(std::mem::take(&mut champ));
                vu = true;
            }
            '\r' | '\n' => {
                if c == '\r' && it.peek() == Some(&'\n') {
                    it.next();
                }
                if vu || !champ.is_empty() {
                    rec.push(std::mem::take(&mut champ));
                    out.push(std::mem::take(&mut rec));
                }
                vu = false;
            }
            c => {
                champ.push(c);
                vu = true;
            }
        }
    }
    if vu || !champ.is_empty() {
        rec.push(champ);
        out.push(rec);
    }
    out
}

/// `_lire` : code -> lignes de la table.
fn lire(chemin: &Path) -> Resultat<Table> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    let texte = String::from_utf8(octets).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    let texte = texte.strip_prefix('\u{feff}').unwrap_or(&texte);
    let mut recs = enregistrements(texte).into_iter();
    let entete = recs.next().unwrap_or_default();
    let col = |nom: &str| entete.iter().position(|c| c == nom);
    let (c_code, c_effet, c_fin, c_fmt) = (col("Code"), col("Date d'effet"), col("Date de fin"), col("Format"));
    let (c_pl, c_dep, c_lib) = (col("Taux plafonné"), col("Taux déplafonné"), col("Libellé"));
    let mut table = Table::new();
    for r in recs {
        let v = |c: Option<usize>| c.and_then(|i| r.get(i)).map(String::as_str).unwrap_or("");
        let code = v(c_code).trim().trim_matches('\'').to_string();
        if code.is_empty() {
            continue;
        }
        let Some(effet) = date_table(v(c_effet))? else { continue };
        let ligne = Ligne {
            effet,
            fin: date_table(v(c_fin))?,
            format: v(c_fmt).trim().to_string(),
            plafonne: taux(v(c_pl))?,
            deplafonne: taux(v(c_dep))?,
            libelle: v(c_lib).to_string(),
        };
        table.entry(code).or_default().push(ligne);
    }
    Ok(table)
}

/// La table, lue une fois tant que le fichier ne change pas.
fn table() -> Resultat<Arc<Table>> {
    type Cache = Mutex<HashMap<PathBuf, ((u128, u64), Arc<Table>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let chemin = dossier_du_code().join(NOM_TABLE);
    let sig = signature(&chemin);
    if let Some(s) = sig {
        if let Some((vue, t)) = cache.lock().unwrap().get(&chemin) {
            if *vue == s {
                return Ok(t.clone());
            }
        }
    }
    let t = Arc::new(lire(&chemin)?);
    if let Some(s) = sig {
        cache.lock().unwrap().insert(chemin, (s, t.clone()));
    }
    Ok(t)
}

/// La ligne de la table en vigueur ce jour-la pour ce CTP : la date d'effet la
/// plus recente l'emporte, a egalite la ligne encore ouverte ; fermee a ce jour,
/// aucune.
fn ligne_de_table<'a>(t: &'a Table, code: &str, jour: &Jour) -> Option<&'a Ligne> {
    let mut meilleure: Option<&Ligne> = None;
    for l in t.get(code)?.iter().filter(|l| l.effet <= *jour) {
        let mieux = match meilleure {
            None => true,
            Some(m) => (l.effet, l.fin.is_none()) > (m.effet, m.fin.is_none()),
        };
        if mieux {
            meilleure = Some(l);
        }
    }
    meilleure.filter(|l| l.fin.map_or(true, |f| *jour < f))
}

/// L'arrondi a l'euro des montants et des assiettes declares aux organismes de securite sociale (CSS L. 133-10,
/// `atelier/reference/sources/code-de-la-securite-sociale/articles-20261013.md` : « arrondi a l'euro le plus proche. La
/// fraction d'euro egale a 0,50 est comptee pour 1 ») : la moitie vers le haut. Le seul endroit qui arrondit a l'euro, pour
/// les lignes 23 du bordereau comme pour les versements 20 et les corrections de la DSN.
pub fn euro(x: Decimal) -> Decimal {
    x.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
}

/// Le montant d'une ligne 23, a l'euro.
pub fn montant_ligne(l: &LigneBordereau, jour: &Jour) -> Resultat<Decimal> {
    let t = table()?;
    let Some(t) = ligne_de_table(&t, &l.code, jour) else {
        return Err(format!(
            "CTP {} absent de la table de l'Urssaf au {} : le montant du \
bordereau ne peut pas se calculer. Mettre a jour la table (source \
urssaf-codes-types-personnel). Rien n'est ecrit.",
            l.code,
            jour.jj_mm_aaaa()
        ));
    };
    let at = if l.taux_at.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from_str(l.taux_at.trim()).map_err(|_| crate::num::illisible(&l.taux_at))?
    };
    let cent = Decimal::ONE_HUNDRED;
    match t.format.as_str() {
        "F" => {
            let m = l.montant.ok_or_else(|| crate::fichiers::absent("montant"))?;
            Ok(-euro(m))
        }
        "Z" => {
            let a = l.assiette.ok_or_else(|| crate::fichiers::absent("assiette"))?;
            Ok(euro(div(mul(a, at), cent)))
        }
        "A" | "C" | "D" | "E" | "K" => {
            let plafonne = l.qualifiant == "921";
            let Some(tx) = (if plafonne { t.plafonne } else { t.deplafonne }) else {
                return Err(format!(
                    "CTP {} ({}), qualifiant {} : la table de l'Urssaf ne donne pas de taux {}. \
Rien n'est ecrit.",
                    l.code,
                    t.libelle,
                    l.qualifiant,
                    if plafonne { "plafonne" } else { "deplafonne" }
                ));
            };
            let a = l.assiette.ok_or_else(|| crate::fichiers::absent("assiette"))?;
            Ok(euro(div(mul(a, tx), cent)) + euro(div(mul(a, at), cent)))
        }
        fmt => Err(format!(
            "CTP {} ({}) : format {} de la table de l'Urssaf, que le moteur ne sait pas \
valoriser. Rien n'est ecrit.",
            l.code, t.libelle, fmt
        )),
    }
}

/// Le montant du bordereau (S21.G00.22.005) : la somme de ses lignes, a deux decimales.
pub fn montant(lignes: &[LigneBordereau], jour: &Jour) -> Resultat<Decimal> {
    let mut s = Decimal::ZERO;
    for l in lignes {
        s += montant_ligne(l, jour)?;
    }
    // Un zero n'a pas de signe : `Decimal("0") + Decimal("-0")` vaut `0`.
    if s.is_zero() {
        s = Decimal::ZERO;
    }
    s.rescale(2);
    Ok(s)
}

// ------------------------------------------------------------------ le trop-verse
//
// UN VERSEMENT N'EST JAMAIS NEGATIF. Une correction a la baisse (une regularisation qui annule plus qu'elle ne redeclare)
// peut faire ressortir negative la somme qu'un mois doit a un organisme. La norme ne l'admet pour aucun : S21.G00.20.005
// est « Montant du versement correspondant au montant des cotisations reglees. Avec decimale, non signe, valeur zero
// autorisee » (cahier technique DSN 2026.1, p. 156, `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`, l. 166-167 ;
// CSL 00 `[0]*(0|[1-9][0-9]*)\.[0-9]{2}`, l. 219). Le guide Urssaf le redit : « Un montant negatif est interdit en bloc
// Versement organisme de protection sociale » (`sources/editeur-dsn/guide-urssaf-regularisation.txt`, l. 733-734). Ce qui
// suit est le seul endroit qui dit, organisme par organisme, ce que devient la somme due et ce que la compagnie fait du
// credit ; `dsn.rs` ecrit le montant, `ordres_paiement.rs` dit le reste a l'ecran.
//
// - L'URSSAF : « Lors du paiement de votre declaration courante, vous pouvez deduire du montant du, celui de votre trop
//   verse dans la rubrique Montant du versement - S21.G00.20.005. Cette deduction pourra s'effectuer dans la limite du
//   montant du bloc S21.G00.22 courant. Il est conseille dans ce cas, notamment si vous vous reprenez d'un montant
//   important, de contacter votre Urssaf via le service en ligne « echange avec mon Urssaf » » (guide Urssaf, § 6.4, l.
//   1095-1103). Au-dela, le credit reste a l'Urssaf : « le declarant prendra contact avec son Urssaf de rattachement pour
//   une gestion de la situation en bilateral » (cahier technique, § 2.2.3.2, `texte/chapitre-2-principes-de-
//   constitution.txt`, l. 1741-1744).
// - UN ORGANISME COMPLEMENTAIRE (prevoyance, bloc 55) : le versement est la somme des composants « sauf si cette somme fait
//   ressortir un montant negatif. Auquel cas un montant nul doit etre renseigne » (p. 157, l. 191-193 ; CCH-12, l. 211-213 ;
//   redit par la fiche net-entreprises 1328, `sources/net-entreprises-base-connaissances/net-entreprises-fiche1328-
//   ventilation-du-versement-20250322.txt`). Les composants (55.001), eux, portent le negatif : l'organisme le voit.
// - LA DGFIP (prelevement a la source) : « Lorsque la somme totale [...] est negative, ces rubriques doivent etre
//   renseignees a 0.00 » ; une DSN initiale creditrice ne declare pas de bloc DGFiP, « si par commodite le logiciel en
//   produit un, il devra etre declare [...] a 0,00 » ; « une demande de remboursement pourra etre deposee aupres du SIE
//   [...] a partir de la messagerie securisee de son espace professionnel sur le site impots.gouv.fr » (fiche
//   net-entreprises 1931, `net-entreprises-fiche1931-situations-creditrices-versement-20250323.txt`, l. 4, 13, 19, 40-41).
// - TOUT AUTRE ORGANISME (Agirc-Arrco, Conges Spectacles, France Travail) : la norme seule. Le versement est ce que la
//   compagnie REGLE (« montant des cotisations reglees ») : elle ne regle rien, il vaut zero, valeur que la norme autorise
//   a chacun d'eux (modalite « montant », p. 156-157). Les cotisations individuelles (81.004) portent, elles, le negatif de
//   la correction. INFERENCE assumee : aucun texte releve ne dit comment ces organismes rendent un credit (remboursement,
//   ou deduction d'une echeance suivante) ; le logiciel ne l'invente pas, il dit a la compagnie de le leur demander.

/// A qui va un versement, pour ce que devient un credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destinataire {
    Urssaf,
    /// Un organisme complementaire, dont le versement porte des composants (bloc S21.G00.55).
    Complementaire,
    Dgfip,
    /// Agirc-Arrco, Conges Spectacles, France Travail : aucun texte particulier.
    Autre,
}

/// Ce qu'un versement ecrit (S21.G00.20.005, jamais negatif) et le credit que la somme due laisse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AVerser {
    pub montant: Decimal,
    /// Ce que la somme due avait de negatif (0 sinon).
    pub credit: Decimal,
}

/// Le versement d'une somme due `du` (deja arrondie comme l'organisme l'attend) : elle-meme, ou zero et le credit.
pub fn a_verser(du: Decimal) -> AVerser {
    let mut zero = Decimal::ZERO;
    zero.rescale(2);
    if du < Decimal::ZERO {
        AVerser { montant: zero, credit: -du }
    } else {
        AVerser { montant: du, credit: Decimal::ZERO }
    }
}

/// La reprise d'un credit Urssaf (guide Urssaf, § 6.4) : `courant`, le bordereau du mois (22.005) ; `credit`, ce que les
/// bordereaux des mois corriges ont de negatif (positif ici). Le versement du mois est le bordereau moins le credit, dans
/// la limite du bordereau ; `deduit` est ce qui a ete repris, le reste demeure a l'Urssaf.
pub fn reprise_urssaf(courant: Decimal, credit: Decimal) -> (AVerser, Decimal) {
    let deduit = credit.max(Decimal::ZERO).min(courant.max(Decimal::ZERO));
    let v = a_verser(courant - deduit);
    (AVerser { montant: v.montant, credit: credit.max(Decimal::ZERO) - deduit + v.credit }, deduit)
}

/// Le credit d'un ordre, dit a l'ecran : combien, ce que la compagnie fait, et le texte qui le dit.
#[derive(Clone, Debug, PartialEq)]
pub struct Credit {
    /// Ce que la compagnie a verse en trop et qui n'est pas repris par ce versement.
    pub reste: Decimal,
    /// Ce que ce versement reprend deja (l'Urssaf seulement).
    pub deduit: Decimal,
    /// Le titre de l'encart (« Trop versé : 65,72 € »).
    pub titre: String,
    pub que_faire: String,
    pub source: &'static str,
}

const SOURCE_URSSAF: &str = "Guide Urssaf « Comment déclarer et régulariser les cotisations Urssaf en DSN », § 6.4 « La reprise de crédit » : le trop versé se déduit du versement courant, dans la limite du bordereau du mois ; au-delà, cahier technique DSN 2026.1, § 2.2.3.2 : prendre contact avec son Urssaf.";
const SOURCE_COMPLEMENTAIRE: &str = "Cahier technique DSN 2026.1, S21.G00.20.005 (CCH-12) et fiche net-entreprises 1328 : quand la somme des composants de versement est négative, le versement est déclaré à zéro.";
const SOURCE_DGFIP: &str = "Fiche net-entreprises 1931 « Gérer les situations créditrices » : un prélèvement à la source négatif se déclare à 0,00, et le remboursement se demande au service des impôts des entreprises (messagerie sécurisée de l'espace professionnel impots.gouv.fr).";
const SOURCE_AUTRE: &str = "Cahier technique DSN 2026.1, S21.G00.20.005 : « montant des cotisations réglées », « non signé, valeur zéro autorisée ». Aucun texte relevé ne dit comment cet organisme rend un crédit.";

/// Un montant a l'ecran : « 1 295,00 € », milliers separes et euro colle par des espaces insecables (le montant ne se
/// coupe pas en fin de ligne), comme `euros` de la page (`toLocaleString("fr-FR")`).
fn euros(x: Decimal) -> String {
    let mut x = x.abs();
    x.rescale(2);
    let t = x.to_string();
    let (e, c) = t.split_once('.').unwrap_or((&t, "00"));
    let mut groupes: Vec<&str> = Vec::new();
    let mut fin = e.len();
    while fin > 3 {
        groupes.push(&e[fin - 3..fin]);
        fin -= 3;
    }
    groupes.push(&e[..fin]);
    groupes.reverse();
    format!("{},{}\u{a0}€", groupes.join("\u{202f}"), c)
}

/// Ce que devient le credit d'un versement : `reste`, le credit que le versement ne reprend pas ; `deduit`, ce qu'il
/// reprend (l'Urssaf seulement). `None` quand il n'y a rien a dire. (`_nom` : l'organisme, que l'ecran nomme deja.)
pub fn credit(dest: Destinataire, _nom: &str, reste: Decimal, deduit: Decimal) -> Option<Credit> {
    if reste <= Decimal::ZERO && deduit <= Decimal::ZERO {
        return None;
    }
    const DEMANDER: &str = "Aucun texte relevé ne dit comment l'organisme rend ce crédit : demandez-lui s'il vous le \
rembourse ou le déduit d'une prochaine échéance.";
    let (titre, que_faire, source) = match dest {
        Destinataire::Urssaf if reste <= Decimal::ZERO => (
            format!("Trop versé déduit : {}", euros(deduit)),
            "Ce qui a été payé en trop sur un mois corrigé est retiré de ce versement, dans la limite du bordereau du \
mois. Rien d'autre à faire ; si la somme est importante, le guide Urssaf conseille de l'expliquer à votre Urssaf par \
le service en ligne « échange avec mon Urssaf »."
                .to_string(),
            SOURCE_URSSAF,
        ),
        Destinataire::Urssaf => (
            if deduit > Decimal::ZERO {
                format!("Trop versé : {} déduits, {} à réclamer", euros(deduit), euros(reste))
            } else {
                format!("Trop versé : {} à réclamer", euros(reste))
            },
            format!(
                "Le trop versé dépasse le bordereau de ce mois : {}{} ne peuvent pas en être déduits. Contactez votre \
Urssaf (service en ligne « échange avec mon Urssaf ») pour qu'elle vous les rende ou les impute.",
                if deduit > Decimal::ZERO { "ce versement tombe à zéro, et les " } else { "les " },
                euros(reste)
            ),
            SOURCE_URSSAF,
        ),
        Destinataire::Complementaire => (
            format!("Trop versé : {}", euros(reste)),
            format!(
                "Rien à payer ce mois-ci : les corrections font plus de cotisations en moins que le mois n'en doit. Le \
versement est déclaré à zéro, le crédit dans les composants de versement. {}",
                DEMANDER
            ),
            SOURCE_COMPLEMENTAIRE,
        ),
        Destinataire::Dgfip => (
            format!("Prélèvement à la source en trop : {}", euros(reste)),
            "Rien à reverser ce mois-ci : les régularisations rendent plus que le mois ne retient. Demandez le \
remboursement au service des impôts des entreprises, par la messagerie sécurisée de votre espace professionnel sur \
impots.gouv.fr."
                .to_string(),
            SOURCE_DGFIP,
        ),
        Destinataire::Autre => (
            format!("Trop versé : {}", euros(reste)),
            format!(
                "Rien à payer ce mois-ci : les corrections font plus de cotisations en moins que le mois n'en doit. Un \
versement ne peut pas être négatif : il est déclaré à zéro, et le crédit reste dans les cotisations des salariés. {}",
                DEMANDER
            ),
            SOURCE_AUTRE,
        ),
    };
    // Les guillemets ne se separent pas de ce qu'ils citent en fin de ligne (« échange avec mon Urssaf »).
    let que_faire = que_faire.replace("« ", "«\u{a0}").replace(" »", "\u{a0}»");
    Some(Credit { reste: reste.max(Decimal::ZERO), deduit, titre, que_faire, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    /// Un versement n'est jamais negatif (S21.G00.20.005, « non signe, valeur zero autorisee ») : une somme due negative
    /// s'ecrit 0.00 et devient un credit.
    #[test]
    fn une_somme_due_negative_se_verse_a_zero_et_laisse_un_credit() {
        assert_eq!(a_verser(d("12.30")), AVerser { montant: d("12.30"), credit: Decimal::ZERO });
        let v = a_verser(d("-65.72"));
        assert_eq!((v.montant.to_string().as_str(), v.credit), ("0.00", d("65.72")));
    }

    /// Guide Urssaf, § 6.4 : 2000 dus en fevrier, 400 trop verses en janvier, 1600 verses ; la deduction s'arrete au
    /// bordereau courant, le reste demeure a l'Urssaf.
    #[test]
    fn la_reprise_urssaf_s_arrete_au_bordereau_du_mois() {
        let (v, deduit) = reprise_urssaf(d("2000.00"), d("400.00"));
        assert_eq!((v.montant, v.credit, deduit), (d("1600.00"), Decimal::ZERO, d("400.00")));
        let (v, deduit) = reprise_urssaf(d("300.00"), d("400.00"));
        assert_eq!((v.montant, v.credit, deduit), (d("0.00"), d("100.00"), d("300.00")));
        let c = credit(Destinataire::Urssaf, "Urssaf", v.credit, deduit).unwrap();
        assert!(c.titre.contains("300,00") && c.titre.contains("100,00"), "le titre dit le deduit et le reste a reclamer : {}", c.titre);
    }

    /// Une date dont le jour depasse 12 est en AAAA-MM-JJ, les autres en
    /// AAAA-JJ-MM.
    #[test]
    fn une_date_dont_le_jour_depasse_12_est_en_aaaa_mm_jj() {
        assert_eq!(date_table("2003-05-28").unwrap(), Jour::nouveau(2003, 5, 28));
        assert_eq!(date_table("2024-01-07").unwrap(), Jour::nouveau(2024, 7, 1));
    }
}
