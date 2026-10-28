//! Les ecritures comptables d'un mois : le journal de paie en ecritures (`ecritures_du_mois`, le plan comptable livre
//! recouvert par `config/plan-comptable.json`) et leur CSV (`Compta_AAAA.csv`) : point-virgule, virgule decimale,
//! UTF-8 avec BOM, fins de ligne CRLF. Tout part des paies ecrites des engagements valides (`etats_paie::charger_mois`).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use droit::catalogue::{self, HorsAssiette};
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::fichiers::vrai;
use socle::num::{eur2, zero2, Decimal};
use socle::valeur::valeur_en_texte;

use crate::plan_comptable::{compte_du_salarie, Refus};

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// Une ligne du journal de paie : date, journal, compte, libelle, debit, credit, analytique, auxiliaire.
pub struct Ecriture {
    pub journal: String,
    pub date: String,
    pub compte: String,
    pub libelle: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub analytique: String,
    pub mois: String,
    pub auxiliaire: String,
    pub auxiliaire_libelle: String,
    /// `e.piece` : posee par les etats de `operations_ecritures` (« PAIE-AAAA-MM »).
    pub piece: String,
    /// `e.numero` : le rang de l'ecriture (une par mois) dans l'etat, a partir de 1.
    pub numero: i64,
}

impl Ecriture {
    pub fn json(&self) -> Value {
        json!({
            "journal": self.journal, "date": self.date, "compte": self.compte, "libelle": self.libelle,
            "debit": self.debit.to_string(), "credit": self.credit.to_string(), "analytique": self.analytique,
            "mois": self.mois, "auxiliaire": self.auxiliaire, "auxiliaire_libelle": self.auxiliaire_libelle,
        })
    }
}

/// `operations_exercice._dernier_jour` : le dernier jour du mois, AAAA-MM-JJ.
fn dernier_jour(annee: i64, mois: i64) -> R<String> {
    let (a, m) = if mois == 12 { (annee + 1, 1) } else { (annee, mois + 1) };
    let premier = socle::date::Jour::lire(&format!("{:04}-{:02}-01", a, m)).map_err(|x| erreur("ValueError", x))?;
    Ok(premier.plus_jours(-1).map_err(|x| erreur("ValueError", x))?.iso())
}

/// La date de l'ecriture d'un mois (son dernier jour) et son libelle (« Decembre 2026 »). « 00 » n'est aucun mois et se
/// refuse (`date::numero_du_mois`).
fn date_et_libelle(mois: &str) -> R<(String, String)> {
    let annee: i64 = mois.chars().take(4).collect::<String>().parse().map_err(|_| erreur("ValueError", "mois illisible"))?;
    let mo = i64::from(socle::date::numero_du_mois(mois).map_err(|e| erreur("ValueError", e))?);
    let date = dernier_jour(annee, mo)?;
    let nom = socle::date::nom_du_mois(mo, socle::date::Graphie::Titre).map_err(|e| erreur("ValueError", e))?;
    Ok((date, format!("{} {}", nom, annee)))
}

/// `plan[cle]` : `KeyError` s'il manque.
fn cle_du_plan<'a>(plan: &'a Map<String, Value>, cle: &str) -> R<&'a Value> {
    plan.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

/// `v[cle]` sur un dictionnaire du plan : `KeyError` s'il manque.
fn dans<'a>(v: &'a Value, cle: &str) -> R<&'a Value> {
    match v {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    }
}

/// `x or defaut`.
fn ou_texte(v: Option<&Value>, defaut: &str) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => defaut.to_string(),
    }
}

/// `operations_exercice.ecritures_du_mois` : une ecriture pour le mois, depuis les paies validees.
/// Le brut par spectacle, la charge patronale et le tiers par ligne de bulletin, le prelevement a la source retenu
/// (une dette envers l'Etat, au compte `pas` du plan), le net par salarie -- le net PAYE, apres l'impot, celui du
/// bulletin. Un identifiant de ligne absent du plan fait refuser : on ne devine pas un compte.
pub fn ecritures_du_mois(ctx: &Contexte, mois: &str, plan: &Map<String, Value>) -> R<Vec<Ecriture>> {
    let (_, paies) = crate::etats_paie::charger_mois(ctx, mois)?;
    let (date, lib_mois) = date_et_libelle(mois)?;
    let jrn = ou_texte(plan.get("journal"), "OD");
    let jrn = if plan.contains_key("journal") { valeur_en_texte(&plan["journal"]) } else { jrn };
    let zero = zero2();

    let lignes_du_plan = cle_du_plan(plan, "lignes")?;
    let mut brut_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut net_par_salarie: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut compte_de: BTreeMap<String, String> = BTreeMap::new();
    let mut frais_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut charge: BTreeMap<(String, String), Decimal> = BTreeMap::new();
    let mut tiers: BTreeMap<String, Decimal> = BTreeMap::new();
    // LE PRELEVEMENT A LA SOURCE : celui du bulletin (`prelevement::des_paies`, la meme lecture, la meme position), retenu sur
    // le net et du a la DGFiP (compte 4421 « Prelevements a la source (Impot sur le revenu) » du plan
    // comptable general, atelier/reference/sources/plan-comptable-general/LISEZ-MOI.md), tel que la paie le porte. Le PAS d'un
    // mois declare est celui de la paie declaree (`engagement::etats::paie_declaree`), lu et non recalcule au taux du
    // jour (CGI art. 1671 : `prelevement::des_paies`).
    let pas_des_paies = crate::prelevement::des_paies(ctx, &paies).map_err(refus)?;
    let mut pas_du_mois = zero;
    // LES RETENUES (une ligne hors assiette de nature `retenue` : acompte, avance, saisie) ne sont pas une charge : un
    // acompte solde une avance deja versee (425 « Personnel - Avances et acomptes et autres comptes debiteurs »), une
    // saisie est due a un tiers (427 « Personnel - Oppositions »), plan de comptes de l'ANC (meme LISEZ-MOI). Le plan
    // dit le compte de chaque rubrique (`retenues`) ; une retenue qu'il ne nomme pas fait refuser. Par (compte,
    // libelle, salarie).
    let mut retenues: BTreeMap<(String, String, String), Decimal> = BTreeMap::new();
    let ajouter = |m: &mut BTreeMap<String, Decimal>, k: String, v: Decimal| {
        let e = m.entry(k).or_insert(zero);
        *e += v;
    };
    for (p, pas) in paies.iter().zip(&pas_des_paies) {
        // UN PERMANENT N'A PAS DE SPECTACLE : son brut va a sa propre section.
        let titre = match p.spectacle.get("titre").filter(|t| vrai(t)) {
            Some(t) => valeur_en_texte(t),
            None => match p.engagement.spectacle.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s.to_string(),
                None => "Salariés au mois".to_string(),
            },
        };
        let brut = eur2(p.brut);
        ajouter(&mut brut_par_spectacle, titre.clone(), brut);
        let prenom = match p.salarie.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => valeur_en_texte(v),
            None => p.salarie.get("prenom").map(valeur_en_texte).unwrap_or_default(),
        };
        let qui = format!("{} {}", valeur_en_texte(p.salarie.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?), prenom);
        let mut retenu = zero;
        for l in p.remuneration.iter().filter(|l| !l.soumis) {
            if catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au).and_then(|r| r.hors_assiette()).map_err(refus)? != HorsAssiette::Mouvement {
                continue;
            }
            let Some(regle) = plan.get("retenues").and_then(|t| t.get(&l.code)).filter(|r| vrai(r)) else {
                return Err(refus(format!(
                    "La retenue « {} » ({}) n'a pas de compte dans le plan comptable. L'ajouter dans config/plan-comptable.json sous « retenues » : un compte 425 pour un acompte ou une avance, 427 pour une saisie ou une opposition.",
                    l.code, l.libelle
                )));
            };
            let k = (valeur_en_texte(dans(regle, "compte")?), ou_texte(regle.get("libelle"), &l.libelle), qui.clone());
            *retenues.entry(k).or_insert(zero) += eur2(l.montant);
            retenu += eur2(l.montant);
        }
        // CE QUE LE NET PORTE EN PLUS DU BRUT : les indemnites non soumises (repas, defraiements) sont payees
        // sans etre du brut ; sans leur ligne au debit, l'ecriture ne s'equilibrerait pas. Les retenues en sont
        // otees : elles ont leur compte.
        let hors_brut = eur2(p.net_a_payer) - (brut - eur2(p.total_sal)) - retenu;
        if !hors_brut.is_zero() {
            ajouter(&mut frais_par_spectacle, titre.clone(), hors_brut);
        }
        // Le net paye est le net a payer moins l'impot retenu (le « MONTANT NET A PAYER » du bulletin) ; l'impot va
        // a l'Etat.
        let prelevement = eur2(pas.as_ref().map_or(zero, |v| v.prelevement));
        pas_du_mois += prelevement;
        ajouter(&mut net_par_salarie, qui.clone(), eur2(p.net_a_payer) - prelevement);
        let cle_salarie = p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_default();
        let compte = compte_du_salarie(plan, &p.salarie, &cle_salarie).map_err(|r| match r {
            Refus::Operation(m) => refus(m),
            Refus::Autre(e) => e,
        })?;
        compte_de.insert(qui, compte);
        for l in &p.lignes {
            let regle = lignes_du_plan.as_object().and_then(|o| o.get(&l.id)).filter(|r| vrai(r));
            let Some(regle) = regle else {
                return Err(refus(format!(
                    "La ligne de bulletin « {} » ({}) n'a pas de compte dans le plan comptable. L'ajouter dans config/plan-comptable.json sous « lignes », avec un compte de charge et un compte de tiers.",
                    l.id, l.libelle
                )));
            };
            let libelle_de = |r: &Value| ou_texte(r.get("libelle"), &l.libelle);
            if l.id == "reduction_generale" {
                if let Some(rg) = &p.reduction_generale {
                    // La RGDU : une charge negative, ventilee au tiers de chaque part telle que la paie l'a calculee.
                    let k = (valeur_en_texte(dans(regle, "charge")?), libelle_de(regle));
                    *charge.entry(k).or_insert(zero) += eur2(l.montant_pat);
                    for (part, id_tiers) in [(rg.urssaf, "maladie"), (rg.chomage, "chomage"), (rg.agirc_arrco, "retraite_t1")] {
                        if !part.is_zero() {
                            let r = lignes_du_plan.as_object().and_then(|o| o.get(id_tiers)).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(id_tiers)))?;
                            let c = valeur_en_texte(dans(r, "tiers")?);
                            *tiers.entry(c).or_insert(zero) -= part;
                        }
                    }
                    continue;
                }
            }
            let (pat, sal) = (eur2(l.montant_pat), eur2(l.montant_sal));
            if !pat.is_zero() {
                let k = (valeur_en_texte(dans(regle, "charge")?), libelle_de(regle));
                *charge.entry(k).or_insert(zero) += pat;
            }
            if !pat.is_zero() || !sal.is_zero() {
                *tiers.entry(valeur_en_texte(dans(regle, "tiers")?)).or_insert(zero) += pat + sal;
            }
        }
    }

    let mut out: Vec<Ecriture> = Vec::new();
    let mut ligne = |compte: &str, libelle: &str, debit: Decimal, credit: Decimal, analytique: &str, auxiliaire: &str| {
        if debit.is_zero() && credit.is_zero() {
            return;
        }
        // Un montant negatif change de colonne plutot que de signe : un logiciel de compta n'aime pas un debit negatif.
        let (mut debit, mut credit) = (debit, credit);
        if debit.is_sign_negative() && !debit.is_zero() {
            credit = -debit;
            debit = zero;
        }
        if credit.is_sign_negative() && !credit.is_zero() {
            debit = -credit;
            credit = zero;
        }
        out.push(Ecriture {
            journal: jrn.clone(),
            date: date.clone(),
            compte: compte.to_string(),
            libelle: format!("{} -- {}", libelle, lib_mois),
            debit: eur2(debit),
            credit: eur2(credit),
            analytique: analytique.to_string(),
            mois: mois.to_string(),
            auxiliaire: auxiliaire.to_string(),
            auxiliaire_libelle: if auxiliaire.is_empty() { String::new() } else { analytique.to_string() },
            piece: String::new(),
            numero: 0,
        });
    };
    let brut_plan = cle_du_plan(plan, "brut")?;
    for (titre, m) in &brut_par_spectacle {
        ligne(&valeur_en_texte(dans(brut_plan, "compte")?), &valeur_en_texte(dans(brut_plan, "libelle")?), *m, zero, titre, "");
    }
    let frais_defaut = json!({"compte": "641400", "libelle": "Indemnites non soumises"});
    let frais = match plan.get("frais").filter(|f| vrai(f)) {
        Some(f) => f.clone(),
        None => frais_defaut,
    };
    for (titre, m) in &frais_par_spectacle {
        ligne(&valeur_en_texte(dans(&frais, "compte")?), &valeur_en_texte(dans(&frais, "libelle")?), *m, zero, titre, "");
    }
    // Les charges dans l'ordre du plan (un compte, un libelle), puis les tiers.
    for ((c, lib), m) in &charge {
        ligne(c, lib, *m, zero, "", "");
    }
    let noms_tiers = plan.get("tiers").filter(|t| vrai(t));
    for (c, m) in &tiers {
        let nom = match noms_tiers.and_then(|t| t.get(c.as_str())).filter(|n| vrai(n)) {
            Some(n) => valeur_en_texte(n),
            None => format!("Organisme {}", c),
        };
        ligne(c, &nom, zero, *m, "", "");
    }
    // Au credit, une ligne pour le mois : la dette envers la DGFiP, qui la preleve sur la declaration du mois.
    if !pas_du_mois.is_zero() {
        let pas_plan = cle_du_plan(plan, "pas")?;
        ligne(&valeur_en_texte(dans(pas_plan, "compte")?), &valeur_en_texte(dans(pas_plan, "libelle")?), zero, pas_du_mois, "", "");
    }
    // Une retenue est negative sur le bulletin : elle passe au credit (un acompte retenu solde l'avance).
    for ((c, lib, qui), m) in &retenues {
        ligne(c, lib, *m, zero, qui, "");
    }
    let net_plan = cle_du_plan(plan, "net")?;
    for (qui, m) in &net_par_salarie {
        let aux = compte_de.get(qui).cloned().unwrap_or_default();
        ligne(&valeur_en_texte(dans(net_plan, "compte")?), &valeur_en_texte(dans(net_plan, "libelle")?), zero, *m, qui, &aux);
    }

    let d = out.iter().fold(zero, |a, e| a + e.debit);
    let c = out.iter().fold(zero, |a, e| a + e.credit);
    if d != c {
        // Par construction brut + charges = tiers + prelevement + retenues + net ; si ca ne tombe pas, une ligne du bulletin
        // manque au plan ou a change de nature. On ne livre pas une ecriture desequilibree.
        return Err(refus(format!(
            "L'écriture de {} n'est pas équilibrée (débit {}, crédit {}) : une ligne de bulletin n'est pas portée comme il faut par le plan comptable.",
            lib_mois, d, c
        )));
    }
    Ok(out)
}

/// `csv.writer(delimiter=";", lineterminator="\r\n")` : un champ qui porte le delimiteur, un guillemet ou un
/// saut de ligne se met entre guillemets, les guillemets doublés.
fn champ_csv(t: &str) -> String {
    if t.contains([';', '"', '\r', '\n']) {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t.to_string()
    }
}

/// `operations_exercice.csv_compta` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub fn csv_compta(ecritures: &[Ecriture]) -> Vec<u8> {
    let mut t = String::from("\u{feff}");
    let mut rang = |champs: &[String]| {
        t.push_str(&champs.iter().map(|c| champ_csv(c)).collect::<Vec<_>>().join(";"));
        t.push_str("\r\n");
    };
    rang(&["Journal", "Date", "Compte", "Libelle", "Debit", "Credit", "Analytique"].map(String::from));
    for e in ecritures {
        let date: String = format!("{}/{}/{}", e.date.chars().skip(8).take(2).collect::<String>(), e.date.chars().skip(5).take(2).collect::<String>(), e.date.chars().take(4).collect::<String>());
        let compte = if e.auxiliaire.is_empty() { e.compte.clone() } else { e.auxiliaire.clone() };
        let montant = |m: Decimal| if m.is_zero() { String::new() } else { m.to_string().replace('.', ",") };
        rang(&[e.journal.clone(), date, compte, e.libelle.clone(), montant(e.debit), montant(e.credit), e.analytique.clone()]);
    }
    t.into_bytes()
}
