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
//!   connait pas : la fiche dit sa TVA, le moteur conclut (`documents::taxe_sur_les_salaires`), sinon refus.
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

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::apprentissage::{champ_du_motif, motif_de_la_fiche, taxe_apprentissage_non_due, MILLESIME_ASSOCIATIONS_REDEVABLES, MNATA_ASSOCIATIONS};
use crate::dsn::{g, liste, txt};
use documents::taxe_sur_les_salaires::{assiette_taxe_sur_les_salaires, fractions_majorees, taxe_sur_les_salaires_de_la_fiche};
use socle::num::{add, dec_avec_echelle, div, eur, mul, Decimal, Resultat};

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
/// Le champ de la fiche entreprise qui dit la contribution supplementaire, a 250 salaries et plus.
pub const CHAMP_CONTRIBUTION_SUPPLEMENTAIRE: &str = "contribution_supplementaire_apprentissage";

// ------------------------------------------------------------ les mois de l'annee

/// Une paie de l'annee : son salarie (`_cle`), et la paie calculee ou seulement son contrat.
enum MoisDeLAnnee {
    /// Les paies du mois : celles de la DSN, ou les paies declarees d'un autre mois quand les montants sont necessaires.
    Calcules(String, Vec<Paie>),
    /// Les contrats saisis du mois, sans montants (`dsn::contrats_du_mois`).
    Contrats(Vec<Paie>),
}

/// Les douze mois de l'annee de la DSN, pour l'etablissement `nic`. Le mois de la DSN est `paies` (celles qu'elle
/// declare) ; un autre mois se lit dans ses engagements : leurs paies declarees (`dsn::paies_declarees_du_mois`) si
/// `montants`, sinon leurs seuls contrats. Un mois sans saisie n'a pas eu de salarie.
fn mois_de_l_annee(ctx: &Contexte, ent: &Value, nic: &str, mois: &str, paies: &[Paie], montants: bool) -> Resultat<Vec<MoisDeLAnnee>> {
    let an = &mois[..4];
    let mut out = Vec::new();
    for m in 1..=12u32 {
        let m = format!("{}-{:02}", an, m);
        if m == mois {
            out.push(MoisDeLAnnee::Calcules(m, paies.to_vec()));
            continue;
        }
        if !engagement::stock::a_une_saisie(&ctx.racine, &m)? {
            continue;
        }
        if montants {
            let declarees = crate::dsn::paies_declarees_du_mois(ctx, &m, Some(nic)).map_err(|e| format!("{} : {}", m, e))?;
            out.push(MoisDeLAnnee::Calcules(m, declarees));
        } else {
            let mut garde = Vec::new();
            for p in crate::dsn::contrats_du_mois(ctx, &m)? {
                if droit::bareme::etablissement_du_salarie(ent, &p.salarie)? == nic {
                    garde.push(p);
                }
            }
            out.push(MoisDeLAnnee::Contrats(garde));
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
    for m in annee {
        match m {
            MoisDeLAnnee::Calcules(_, ps) => {
                for p in ps.iter().filter(|p| !p.engagement.sans_paie()) {
                    let b = par_salarie.entry(txt(g(&p.salarie, "_cle"))).or_insert(Decimal::ZERO);
                    *b = add(*b, assiette_taxe_sur_les_salaires(p, abattement));
                }
            }
            MoisDeLAnnee::Contrats(_) => return Err("assiettes de la taxe sur les salaires : un mois sans montants".into()),
        }
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

/// L'assiette de la PEEC de l'etablissement : les remunerations de l'annee qui servent de base aux cotisations de
/// securite sociale, la base 03 de chaque contrat (celle que la DSN de chaque mois a declaree ou declare).
fn assiette_peec(ctx: &Contexte, ent: &Value, orgs: &Value, nic: &str, annee: &[MoisDeLAnnee]) -> Resultat<Decimal> {
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
        Bloc44 { code: "005", montant: Some(assiette_peec(ctx, ent, orgs, nic, &annee)?), motif: None }
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
}
