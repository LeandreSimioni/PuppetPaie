//! Ce que la page ecrit pour l'exercice (`moteur/operations_exercice.py`) : `cloture` (et sa
//! reouverture, `rouvrir`) et `export-compta`.
//!
//! - L'export compta : le journal de paie de l'annee (ou de `du` a `au`) en ecritures (`ecritures_du_mois`,
//!   le plan comptable livre recouvert par `config/plan-comptable.json`) et son CSV (`Compta_AAAA.csv`) :
//!   point-virgule, virgule decimale, UTF-8 avec BOM, fins de ligne CRLF. Tout part des bulletins recalcules
//!   depuis les saisies, les memes lignes, les memes arrondis.
//! - La cloture : fige l'annee (`config/exercices.json`) et reunit ce qu'il faut garder en un zip
//!   (`sorties/exercices/Exercice_AAAA.zip`) : saisies, sorties, retours des organismes, journal des depots,
//!   fiches, bareme, export compta, LISEZ-MOI. Le zip est ecrit par le zip de `routes_dossier_salarie` (deflate,
//!   CRC-32, entrees dans l'ordre du Python) : la liste des fichiers et chacun d'eux, octet pour octet, sont ceux
//!   de l'ancien Python ; les octets COMPRESSES (donc l'empreinte et la taille de l'archive) dependent du
//!   compresseur, et l'empreinte rendue est celle de l'archive que ce programme a reellement ecrite.
//!
//! Un refus de l'export pendant la cloture (une `ErreurOperation`, dont les refus du calcul) n'arrete pas la
//! cloture : il est rendu dans `manques`, et l'archive part sans export.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::catalogue::{self, HorsAssiette};
use crate::contexte::Contexte;
use crate::engagement;
use crate::etats::{self, erreur, py_str};
use crate::fichiers::vrai;
use crate::num::{eur2, zero2, Decimal};
use crate::retours::Echec;
use crate::routes_dossier_salarie::{entree_ecrite, entree_fichier, zip, Entree};
use crate::routes_dossiers::{compte_du_salarie, plan_comptable, Refus};
use crate::routes_ouverture::{mois_de_l_annee, nic_principal, refus_du_calcul};

type R<T> = Result<T, Echec>;

/// `util.MOIS`.
const MOIS: [&str; 12] = ["Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre"];

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    erreur("OSError", format!("{} : {}", p.display(), e))
}

/// Une `ErreurOperation` pour un `except ErreurOperation` de Python : celles que le moteur nomme, et
/// les refus du calcul (`CalculRefuse`, `SaisieIncomplete`), qui en sont.
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some("ErreurOperation") | Some("SaisieIncomplete"))
}

/// L'heure de Paris, a la seconde (`datetime.now().isoformat(timespec="seconds")`).
fn maintenant() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    crate::paris::iso_secondes(t)
}

/// `str(annee)` d'une valeur de la charge.
fn texte_de(v: &Value) -> String {
    py_str(v)
}

fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle)))
}

/// `int(x)` d'une valeur JSON.
fn entier(v: &Value) -> R<i64> {
    match v {
        Value::Number(n) => Ok(n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64)),
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::String(s) => crate::apercu_dsn::int_py(s)
            .and_then(|n| i64::try_from(n).ok())
            .ok_or_else(|| erreur("ValueError", format!("nombre entier illisible : {}", crate::fichiers::repr(s)))),
        autre => Err(erreur(
            "TypeError",
            crate::fichiers::attendu("un nombre entier", etats::nom_type(autre)),
        )),
    }
}

// ------------------------------------------------------------------ le journal des depots

/// Le journal des depots, relu une fois, quand il faut dire si la DSN d'un mois est partie.
struct Depots<'a> {
    ctx: &'a Contexte,
    lignes: Option<Vec<Value>>,
}

impl<'a> Depots<'a> {
    fn nouveau(ctx: &'a Contexte) -> Self {
        Depots { ctx, lignes: None }
    }

    /// `journal.etat_dsn_du_mois(ctx, mois)` : une DSN reelle acceptee est-elle partie pour ce mois ?
    fn dsn_acceptee(&mut self, mois: &str) -> R<bool> {
        if self.lignes.is_none() {
            self.lignes = Some(etats::charger(&self.ctx.racine)?);
        }
        let l = self.lignes.as_ref().unwrap();
        Ok(vrai(&etats::etat_dsn_du_mois(l, mois, &Value::Null, &nic_principal(self.ctx))?))
    }
}

// ------------------------------------------------------------------ les ecritures

/// `operations_exercice.Ecriture`.
pub(crate) struct Ecriture {
    pub(crate) journal: String,
    pub(crate) date: String,
    pub(crate) compte: String,
    pub(crate) libelle: String,
    pub(crate) debit: Decimal,
    pub(crate) credit: Decimal,
    pub(crate) analytique: String,
    pub(crate) mois: String,
    pub(crate) auxiliaire: String,
    pub(crate) auxiliaire_libelle: String,
    /// `e.piece` : posee par les etats de `operations_ecritures` (« PAIE-AAAA-MM »).
    pub(crate) piece: String,
    /// `e.numero` : le rang de l'ecriture (une par mois) dans l'etat, a partir de 1.
    pub(crate) numero: i64,
}

impl Ecriture {
    fn json(&self) -> Value {
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
    let premier = crate::date::Jour::lire(&format!("{:04}-{:02}-01", a, m)).map_err(|x| erreur("ValueError", x))?;
    Ok(premier.plus_jours(-1).map_err(|x| erreur("ValueError", x))?.iso())
}

/// La date de l'ecriture d'un mois (son dernier jour) et son libelle (« Decembre 2026 »). « 00 » n'est aucun mois et se
/// refuse (`date::numero_du_mois`, lot F1 ; il se datait du 31 decembre de l'annee d'avant, libelle « Decembre »).
fn date_et_libelle(mois: &str) -> R<(String, String)> {
    let annee: i64 = mois.chars().take(4).collect::<String>().parse().map_err(|_| erreur("ValueError", "mois illisible"))?;
    let mo = i64::from(crate::date::numero_du_mois(mois).map_err(|e| erreur("ValueError", e))?);
    let date = dernier_jour(annee, mo)?;
    Ok((date, format!("{} {}", MOIS[(mo - 1) as usize], annee)))
}

/// `plan[cle]` : `KeyError` s'il manque.
fn cle_du_plan<'a>(plan: &'a Map<String, Value>, cle: &str) -> R<&'a Value> {
    plan.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle)))
}

/// `v[cle]` sur un dictionnaire du plan : `KeyError` s'il manque.
fn dans<'a>(v: &'a Value, cle: &str) -> R<&'a Value> {
    match v {
        Value::Object(o) => o.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle))),
        autre => Err(erreur("TypeError", crate::fichiers::attendu("un objet", etats::nom_type(autre)))),
    }
}

/// `x or defaut`.
fn ou_texte(v: Option<&Value>, defaut: &str) -> String {
    match v {
        Some(x) if vrai(x) => py_str(x),
        _ => defaut.to_string(),
    }
}

/// `operations_exercice.ecritures_du_mois` : une ecriture pour le mois, depuis les bulletins recalcules.
/// Le brut par spectacle, la charge patronale et le tiers par ligne de bulletin, le prelevement a la source retenu
/// (une dette envers l'Etat, au compte `pas` du plan), le net par salarie -- le net PAYE, apres l'impot, celui du
/// bulletin. Un identifiant de ligne absent du plan fait refuser : on ne devine pas un compte.
pub(crate) fn ecritures_du_mois(ctx: &Contexte, mois: &str, plan: &Map<String, Value>) -> R<Vec<Ecriture>> {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let data = engagement::lire_saisie(&chemin).map_err(Echec::from)?;
    let saisie = engagement::charger_donnees(ctx, &data).map_err(|m| Echec::from(refus_du_calcul(ctx, &data, m)))?;
    let (date, lib_mois) = date_et_libelle(mois)?;
    let jrn = ou_texte(plan.get("journal"), "OD");
    let jrn = if plan.contains_key("journal") { py_str(&plan["journal"]) } else { jrn };
    let zero = zero2();

    let lignes_du_plan = cle_du_plan(plan, "lignes")?;
    let mut brut_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut net_par_salarie: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut compte_de: BTreeMap<String, String> = BTreeMap::new();
    let mut frais_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut charge: BTreeMap<(String, String), Decimal> = BTreeMap::new();
    let mut tiers: BTreeMap<String, Decimal> = BTreeMap::new();
    // LE PRELEVEMENT A LA SOURCE : celui du bulletin (`pas::des_paies`, le meme calcul, la meme position), retenu sur
    // le net et du a la DGFiP (compte 4421 « Prelevements a la source (Impot sur le revenu) » du plan
    // comptable general, prive/reference/sources/plan-comptable-general/LISEZ-MOI.md). Une paie dont le prelevement
    // ne se calcule pas (bareme sans grille) n'en a pas : le bulletin imprime 0,00 et paie le net entier. Le PAS d'un
    // mois DEPOSE est celui que sa DSN a declare (S21.G00.50.009), lu et non recalcule au taux du jour (lot D2 ; CGI
    // art. 1671 : `pas::des_paies`) ; jusque-la (limite du lot R3), il s'en ecartait quand la DGFiP avait transmis un
    // autre taux depuis.
    let pas_des_paies = crate::pas::des_paies(ctx, &saisie.paies).map_err(refus)?;
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
    for (p, pas) in saisie.paies.iter().zip(&pas_des_paies) {
        // UN PERMANENT N'A PAS DE SPECTACLE : son brut va a sa propre section.
        let titre = match p.spectacle.get("titre").filter(|t| vrai(t)) {
            Some(t) => py_str(t),
            None => match p.engagement.spectacle.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s.to_string(),
                None => "Salariés au mois".to_string(),
            },
        };
        let brut = eur2(p.brut);
        ajouter(&mut brut_par_spectacle, titre.clone(), brut);
        let prenom = match p.salarie.get("prenom_affichage").filter(|v| vrai(v)) {
            Some(v) => py_str(v),
            None => p.salarie.get("prenom").map(py_str).unwrap_or_default(),
        };
        let qui = format!("{} {}", py_str(p.salarie.get("nom").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("nom")))?), prenom);
        let mut retenu = zero;
        for l in p.remuneration.iter().filter(|l| !l.soumis) {
            if catalogue::rubrique(&ctx.racine, &l.code).and_then(|r| r.hors_assiette()).map_err(refus)? != HorsAssiette::Mouvement {
                continue;
            }
            let Some(regle) = plan.get("retenues").and_then(|t| t.get(&l.code)).filter(|r| vrai(r)) else {
                return Err(refus(format!(
                    "La retenue « {} » ({}) n'a pas de compte dans le plan comptable. L'ajouter dans config/plan-comptable.json sous « retenues » : un compte 425 pour un acompte ou une avance, 427 pour une saisie ou une opposition.",
                    l.code, l.libelle
                )));
            };
            let k = (py_str(dans(regle, "compte")?), ou_texte(regle.get("libelle"), &l.libelle), qui.clone());
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
        // a l'Etat. (Jusqu'au lot R3, l'export portait tout le net au 421 et rien au 4421 : l'ancien
        // `getattr(p, "pas", 0)` du Python valait toujours 0, et le Rust l'avait recopie.)
        let prelevement = eur2(pas.as_ref().map_or(zero, |v| v.prelevement));
        pas_du_mois += prelevement;
        ajouter(&mut net_par_salarie, qui.clone(), eur2(p.net_a_payer) - prelevement);
        let cle_salarie = p.salarie.get("_cle").map(py_str).unwrap_or_default();
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
                    let k = (py_str(dans(regle, "charge")?), libelle_de(regle));
                    *charge.entry(k).or_insert(zero) += eur2(l.montant_pat);
                    for (part, id_tiers) in [(rg.urssaf, "maladie"), (rg.chomage, "chomage"), (rg.agirc_arrco, "retraite_t1")] {
                        if !part.is_zero() {
                            let r = lignes_du_plan.as_object().and_then(|o| o.get(id_tiers)).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(id_tiers)))?;
                            let c = py_str(dans(r, "tiers")?);
                            *tiers.entry(c).or_insert(zero) -= part;
                        }
                    }
                    continue;
                }
            }
            let (pat, sal) = (eur2(l.montant_pat), eur2(l.montant_sal));
            if !pat.is_zero() {
                let k = (py_str(dans(regle, "charge")?), libelle_de(regle));
                *charge.entry(k).or_insert(zero) += pat;
            }
            if !pat.is_zero() || !sal.is_zero() {
                *tiers.entry(py_str(dans(regle, "tiers")?)).or_insert(zero) += pat + sal;
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
        ligne(&py_str(dans(brut_plan, "compte")?), &py_str(dans(brut_plan, "libelle")?), *m, zero, titre, "");
    }
    let frais_defaut = json!({"compte": "641400", "libelle": "Indemnites non soumises"});
    let frais = match plan.get("frais").filter(|f| vrai(f)) {
        Some(f) => f.clone(),
        None => frais_defaut,
    };
    for (titre, m) in &frais_par_spectacle {
        ligne(&py_str(dans(&frais, "compte")?), &py_str(dans(&frais, "libelle")?), *m, zero, titre, "");
    }
    // Les charges dans l'ordre du plan (un compte, un libelle), puis les tiers.
    for ((c, lib), m) in &charge {
        ligne(c, lib, *m, zero, "", "");
    }
    let noms_tiers = plan.get("tiers").filter(|t| vrai(t));
    for (c, m) in &tiers {
        let nom = match noms_tiers.and_then(|t| t.get(c.as_str())).filter(|n| vrai(n)) {
            Some(n) => py_str(n),
            None => format!("Organisme {}", c),
        };
        ligne(c, &nom, zero, *m, "", "");
    }
    // Au credit, une ligne pour le mois : la dette envers la DGFiP, qui la preleve sur la declaration du mois.
    if !pas_du_mois.is_zero() {
        let pas_plan = cle_du_plan(plan, "pas")?;
        ligne(&py_str(dans(pas_plan, "compte")?), &py_str(dans(pas_plan, "libelle")?), zero, pas_du_mois, "", "");
    }
    // Une retenue est negative sur le bulletin : elle passe au credit (un acompte retenu solde l'avance).
    for ((c, lib, qui), m) in &retenues {
        ligne(c, lib, *m, zero, qui, "");
    }
    let net_plan = cle_du_plan(plan, "net")?;
    for (qui, m) in &net_par_salarie {
        let aux = compte_de.get(qui).cloned().unwrap_or_default();
        ligne(&py_str(dans(net_plan, "compte")?), &py_str(dans(net_plan, "libelle")?), zero, *m, qui, &aux);
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
pub(crate) fn csv_compta(ecritures: &[Ecriture]) -> Vec<u8> {
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

// ------------------------------------------------------------------ l'export compta

/// `operations_exercice.ExportCompta`.
pub(crate) struct ExportCompta {
    pub(crate) annee: String,
    pub(crate) du: i64,
    pub(crate) au: i64,
    pub(crate) mois: Vec<String>,
    pub(crate) ecritures: Vec<Ecriture>,
    pub(crate) total_debit: Decimal,
    pub(crate) total_credit: Decimal,
    pub(crate) fichier: Option<PathBuf>,
    pub(crate) plan: String,
    pub(crate) avertissements: Vec<String>,
}

impl ExportCompta {
    /// `sortie_json.brut(export)` : un chemin ne rend que son nom.
    pub(crate) fn json(&self) -> Value {
        json!({
            "annee": self.annee, "du": self.du, "au": self.au, "mois": self.mois,
            "ecritures": self.ecritures.iter().map(Ecriture::json).collect::<Vec<_>>(),
            "total_debit": self.total_debit.to_string(), "total_credit": self.total_credit.to_string(),
            "fichier": self.fichier.as_ref().map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            "plan": self.plan, "avertissements": self.avertissements,
        })
    }
}

fn dossier_exercices(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("sorties").join("exercices")
}

/// `operations_exercice.export_compta` : le journal de paie de l'annee (ou de `du` a `au`), et son fichier CSV.
/// Les mois pris sont ceux qui ont une saisie.
fn export_compta(ctx: &Contexte, annee: &str, du: i64, au: i64, ecrire: bool) -> R<ExportCompta> {
    let plan = plan_comptable(ctx)?;
    let propre = ctx.racine.join("config").join("plan-comptable.json");
    let origine = if propre.exists() { "config/plan-comptable.json" } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    let mois: Vec<String> = mois_de_l_annee(ctx, annee)
        .into_iter()
        .filter(|m| {
            let n: i64 = m.chars().skip(5).take(2).collect::<String>().parse().unwrap_or(0);
            du <= n && n <= au && ctx.racine.join("paies").join(format!("{}.json", m)).exists()
        })
        .collect();
    if mois.is_empty() {
        return Err(refus(format!(
            "Aucune paie saisie en {}{}.",
            annee,
            if (du, au) == (1, 12) { String::new() } else { format!(" entre les mois {} et {}", du, au) }
        )));
    }
    let mut depots = Depots::nouveau(ctx);
    let mut avert: Vec<String> = Vec::new();
    let mut ecritures: Vec<Ecriture> = Vec::new();
    for m in &mois {
        if !ctx.racine.join("sorties").join(m).is_dir() {
            avert.push(format!("{} : saisi mais jamais écrit (pas de bulletin) -- pris quand même, depuis la saisie.", m));
        } else if !depots.dsn_acceptee(m)? {
            avert.push(format!("{} : pas de DSN réelle acceptée au journal.", m));
        }
        ecritures.extend(ecritures_du_mois(ctx, m, &plan)?);
    }
    if !propre.exists() {
        avert.push("Plan comptable : celui livré avec le logiciel, à faire valider par le comptable (config/plan-comptable.json pour le remplacer).".to_string());
    }
    let zero = zero2();
    let total_debit = ecritures.iter().fold(zero, |a, e| a + e.debit);
    let total_credit = ecritures.iter().fold(zero, |a, e| a + e.credit);
    let mut fichier = None;
    if ecrire {
        let dossier = dossier_exercices(ctx);
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        let nom = if (du, au) == (1, 12) { format!("Compta_{}.csv", annee) } else { format!("Compta_{}_{:02}-{:02}.csv", annee, du, au) };
        let f = dossier.join(nom);
        std::fs::write(&f, csv_compta(&ecritures)).map_err(|e| en_os(&e, &f))?;
        fichier = Some(f);
    }
    Ok(ExportCompta { annee: annee.to_string(), du, au, mois, ecritures, total_debit, total_credit, fichier, plan: origine.to_string(), avertissements: avert })
}

/// La route `export-compta` : `{"annee", "du"?, "au"?, "apercu"?}`.
pub(crate) fn export_compta_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(export_compta_charge(ctx, charge)?.json())
}

/// L'export compta de la charge de la route, avant sa mise en JSON (la ligne de commande en rend le chemin entier).
pub(crate) fn export_compta_charge(ctx: &Contexte, charge: &Value) -> R<ExportCompta> {
    let annee = texte_de(exiger(charge, "annee")?);
    let borne = |cle: &str, defaut: i64| -> R<i64> {
        match charge.get(cle).filter(|v| vrai(v)) {
            Some(v) => entier(v),
            None => Ok(defaut),
        }
    };
    let du = borne("du", 1)?;
    let au = borne("au", 12)?;
    let ecrire = !charge.get("apercu").is_some_and(vrai);
    export_compta(ctx, &annee, du, au, ecrire)
}

// ------------------------------------------------------------------ la cloture

/// `operations_socle.exercices` : `config/exercices.json`, sans les cles qui commencent par `_`.
fn lire_exercices(ctx: &Contexte) -> R<Map<String, Value>> {
    match crate::routes_etats::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => Ok(o),
        autre => Err(erreur("AttributeError", crate::fichiers::attendu("un objet", etats::nom_type(&autre)))),
    }
}

/// `exercice_clos(ctx, annee)` : la cloture de l'annee (ou `None`).
fn exercice_clos(ctx: &Contexte, annee: &str) -> R<Option<Value>> {
    let a: String = annee.chars().take(4).collect();
    Ok(lire_exercices(ctx)?.into_iter().find(|(k, _)| !k.starts_with('_') && *k == a).map(|(_, v)| v))
}

fn ecrire_exercices(ctx: &Contexte, tous: &Map<String, Value>) -> R<()> {
    let chemin = ctx.racine.join("config").join("exercices.json");
    crate::ecriture::ecrire_json(&chemin, &Value::Object(tous.clone()), 1).map_err(|m| erreur("OSError", m))
}

/// Les fichiers sous `d`, dans l'ordre de `sorted(d.rglob("*"))` (les chemins se comparent composant par composant).
fn fichiers_sous(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            fichiers_sous(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

/// `operations_exercice.cloturer` : fige l'annee et reunit ce qu'il faut garder en un zip. Refuse si un mois
/// de l'annee qui a des paies n'a pas de DSN reelle acceptee (`forcer` passe outre, et c'est ecrit dans
/// `exercices.json`), une annee deja close, et l'annee en cours.
pub(crate) fn cloturer(ctx: &Contexte, annee: &str, forcer: bool, qui: &str) -> R<Value> {
    if let Some(clos) = exercice_clos(ctx, annee)? {
        let le: String = match clos.get("le") {
            Some(v) => py_str(v),
            None => "?".to_string(),
        };
        return Err(refus(format!("L'exercice {} est déjà clos (le {}).", annee, le.chars().take(10).collect::<String>())));
    }
    let an = entier(&Value::String(annee.to_string()))?;
    if an >= i64::from(ctx.aujourdhui.an) {
        return Err(refus(format!("L'exercice {} n'est pas fini : on clôture une année passée.", annee)));
    }
    let mois = mois_de_l_annee(ctx, annee);
    let mut depots = Depots::nouveau(ctx);
    let mut sans_dsn: Vec<String> = Vec::new();
    for m in &mois {
        if ctx.racine.join("paies").join(format!("{}.json", m)).exists() && !depots.dsn_acceptee(m)? {
            sans_dsn.push(m.clone());
        }
    }
    if !sans_dsn.is_empty() && !forcer {
        return Err(refus(format!(
            "Pas de DSN réelle acceptée pour {}. La clôture vient APRÈS la dernière DSN ; déposer d'abord, ou forcer en le sachant.",
            sans_dsn.join(", ")
        )));
    }

    // L'export compta fait partie de ce qu'on garde : on l'ecrit si absent.
    let (fichier_export, manques) = match export_compta(ctx, annee, 1, 12, true) {
        Ok(e) => (e.fichier, Vec::new()),
        Err(e) if est_une_erreur_d_operation(&e) => (None, vec![format!("export compta : {}", e.message)]),
        Err(e) => return Err(e),
    };

    let dossier = dossier_exercices(ctx);
    std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
    let archive = dossier.join(format!("Exercice_{}.zip", annee));
    let mut entrees: Vec<Entree> = Vec::new();
    for m in &mois {
        let s = ctx.racine.join("paies").join(format!("{}.json", m));
        if s.exists() {
            entrees.push(entree_fichier(&s, format!("paies/{}.json", m))?);
        }
        let d = ctx.racine.join("sorties").join(m);
        if d.is_dir() {
            let mut fichiers = Vec::new();
            fichiers_sous(&d, &mut fichiers);
            fichiers.sort();
            for p in fichiers {
                let rel: Vec<String> = p.strip_prefix(&d).unwrap_or(&p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                entrees.push(entree_fichier(&p, format!("sorties/{}/{}", m, rel.join("/")))?);
            }
        }
    }
    // Les retours des organismes : les fichiers dont le nom commence par l'annee (AAAAMMJJ-HHMMSS_nature..). Ceux de
    // janvier suivant qui repondent a la DSN de decembre sont pris aussi : ils portent sur l'annee close.
    let suivante = format!("{}01", an + 1);
    for sous in ["", "dpae"] {
        let d = if sous.is_empty() { ctx.racine.join("retours") } else { ctx.racine.join("retours").join(sous) };
        if d.is_dir() {
            let mut noms: Vec<PathBuf> = std::fs::read_dir(&d).map_err(|e| en_os(&e, &d))?.flatten().map(|e| e.path()).collect();
            noms.sort();
            for p in noms {
                let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if p.is_file() && (nom.starts_with(annee) || nom.starts_with(&suivante)) {
                    let prefixe = if sous.is_empty() { String::new() } else { format!("{}/", sous) };
                    entrees.push(entree_fichier(&p, format!("retours/{}{}", prefixe, nom))?);
                }
            }
        }
    }
    let j = ctx.racine.join("journal").join("depots.json");
    if j.exists() {
        entrees.push(entree_fichier(&j, "journal/depots.json".to_string())?);
    }
    let cfg = ctx.racine.join("config");
    for nom in ["entreprise.json", "salaries.json", "spectacles.json", "organismes.json", "rubriques.json", "compteurs.json", "pas.json"] {
        if cfg.join(nom).exists() {
            entrees.push(entree_fichier(&cfg.join(nom), format!("config/{}", nom))?);
        }
    }
    let bar = cfg.join("baremes").join(format!("{}.json", annee));
    if bar.exists() {
        entrees.push(entree_fichier(&bar, format!("config/baremes/{}.json", annee))?);
    }
    if let Some(f) = &fichier_export {
        entrees.push(entree_fichier(f, format!("compta/{}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?);
    }
    let aujourd_hui = ctx.aujourdhui;
    let lisez_moi = format!(
        "Exercice {} -- archive de cloture, PuppetPaie, le {:02}/{:02}/{:04}.\n\n\
         paies/      les saisies de chaque mois\n\
         sorties/    bulletins, contrats (signes dans envoi/), DPAE, DSN, certificats, courriels\n\
         retours/    les reponses des organismes (net-entreprises, Urssaf DPAE)\n\
         journal/    le journal des depots\n\
         config/     les fiches telles qu'elles etaient a la cloture\n\
         compta/     le journal des ecritures de paie\n",
        annee, aujourd_hui.jour, aujourd_hui.mois, aujourd_hui.an
    );
    entrees.push(entree_ecrite("LISEZ-MOI.txt".to_string(), lisez_moi.into_bytes()));
    let n = entrees.len();
    let octets = zip(&entrees)?;
    std::fs::write(&archive, &octets).map_err(|e| en_os(&e, &archive))?;

    let empreinte: String = Sha256::digest(&octets).iter().map(|b| format!("{:02x}", b)).collect();
    let le = maintenant();
    let mut tous = match crate::routes_etats::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => o,
        autre => return Err(erreur("TypeError", crate::fichiers::non_modifiable(etats::nom_type(&autre)))),
    };
    let relatif = archive.strip_prefix(&ctx.racine).unwrap_or(&archive).to_string_lossy().replace('\\', "/");
    tous.insert(
        annee.to_string(),
        json!({
            "le": le, "qui": qui, "archive": relatif, "octets": octets.len(), "empreinte": empreinte,
            "fichiers": n, "forcee": forcer && !sans_dsn.is_empty(),
            "sans_dsn": if forcer { json!(sans_dsn) } else { json!([]) },
        }),
    );
    ecrire_exercices(ctx, &tous)?;
    Ok(json!({
        "annee": annee, "le": le, "archive": archive.to_string_lossy(), "octets": octets.len(),
        "empreinte": empreinte, "fichiers": n, "manques": manques,
    }))
}

/// `operations_exercice.rouvrir_exercice` : retire la cloture. L'archive reste sur le disque : elle dit ce
/// qu'etait l'annee au moment ou on l'a fermee.
fn rouvrir_exercice(ctx: &Contexte, annee: &str, qui: &str) -> R<Value> {
    let mut tous = match crate::routes_etats::lire_json_ou_vide(&ctx.racine.join("config").join("exercices.json"))? {
        Value::Object(o) => o,
        autre => return Err(erreur("TypeError", format!("config/exercices.json : {}", crate::fichiers::attendu("un objet", etats::nom_type(&autre))))),
    };
    let Some(clos) = tous.shift_remove(annee) else {
        return Err(refus(format!("L'exercice {} n'est pas clos.", annee)));
    };
    let mut rouvert = match &clos {
        Value::Object(o) => o.clone(),
        autre => return Err(erreur("TypeError", crate::fichiers::attendu("un objet", etats::nom_type(autre)))),
    };
    rouvert.insert("annee".into(), json!(annee));
    rouvert.insert("rouvert_le".into(), json!(maintenant()));
    rouvert.insert("rouvert_par".into(), json!(qui));
    let liste = tous.entry("_rouverts").or_insert_with(|| json!([]));
    match liste {
        Value::Array(a) => a.push(Value::Object(rouvert)),
        autre => return Err(erreur("AttributeError", crate::fichiers::attendu("une liste", etats::nom_type(autre)))),
    }
    ecrire_exercices(ctx, &tous)?;
    Ok(json!({
        "annee": annee,
        "etait_clos_depuis": clos.get("le").cloned().unwrap_or(Value::Null),
        "archive": clos.get("archive").cloned().unwrap_or(Value::Null),
    }))
}

/// La route `cloture` : `{"annee", "forcer"?, "rouvrir"?, "qui"?}`.
pub(crate) fn cloture_route(ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = texte_de(exiger(charge, "annee")?);
    // `qui` : la ligne de commande ecrit `cli`, la page `interface`.
    let qui = charge.get("qui").filter(|v| vrai(v)).map(py_str).unwrap_or_else(|| "interface".to_string());
    if charge.get("rouvrir").is_some_and(vrai) {
        return rouvrir_exercice(ctx, &annee, &qui);
    }
    cloturer(ctx, &annee, charge.get("forcer").is_some_and(vrai), &qui)
}

#[cfg(test)]
mod tests_mois {
    /// Lot F1 : l'ecriture du mois « 00 » se refuse ; elle se datait du 31 decembre de l'annee d'avant, libellee
    /// « Decembre » de l'annee dite.
    #[test]
    fn le_mois_00_d_une_ecriture_se_refuse() {
        let refus = super::date_et_libelle("2026-00").unwrap_err();
        assert!(refus.message.contains(&crate::date::mois_impossible(0)), "{}", refus.message);
        assert_eq!(super::date_et_libelle("2026-12").unwrap(), ("2026-12-31".to_string(), "Decembre 2026".to_string()));
    }
}
