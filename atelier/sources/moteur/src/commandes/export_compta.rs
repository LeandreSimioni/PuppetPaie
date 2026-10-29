//! `moteur export-compta ANNEE [--du N] [--au N] [--apercu] [--json]` : le journal de paie de l'annee, pour la comptabilite
//! (`moteur/commandes_exercice.py`, `cmd_export_compta` ; `operations_exercice.export_compta`). Route `export-compta`
//! (`exercice_ecrit::export_compta_charge`). Ecrit `exports/` ... `Compta_AAAA.csv` sauf `--apercu` ; rien n'est lu au coffre.
//!
//! Le texte rend le chemin ECRIT en entier (le JSON n'en rend que le nom), comme le Python.
//!
//! DEFAUT DU PYTHON, QUI NE SE REPRODUIT PAS : une annee mal ecrite, un plan comptable abime, une saisie illisible sortaient en trace
//! d'appels (seule `ErreurOperation` etait attrapee) ; ici, un plantage propre au journal des incidents.
//!
//! POINT DOUTEUX : `--du 0` / `--au 0` : la route lit `0` comme « absent » (les bornes 1 et 12) ; le Python filtrait vraiment sur 0.

use serde_json::json;

use super::actes::{est_refus, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{debut, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "export-compta",
    positionnels: &[Pos::requis("annee")],
    options: &[Opt::entier("du"), Opt::entier("au"), Opt::drapeau("apercu"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) || e.classe == Some("CalculRefuse") => reponse_metier("export-compta", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let charge = json!({
        "annee": args.texte("annee").unwrap_or(""),
        "du": args.entier("du").unwrap_or(1),
        "au": args.entier("au").unwrap_or(12),
        "apercu": args.drapeau("apercu"),
    });
    let r = crate::compagnie::exercice_ecrit::export_compta_charge(&cadre.ctx, &charge)?;
    if args.drapeau("json") {
        return Ok(rendre(&enveloppe("export-compta", r.json())));
    }
    let mois: Vec<String> = r.mois.iter().map(|m| m.chars().skip(5).collect()).collect();
    dire!("Export compta {}, mois {} : {} ecritures, debit {} = credit {}", r.annee, mois.join(", "), r.ecritures.len(), r.total_debit, r.total_credit);
    for e in &r.ecritures {
        // `e.debit or ""` : un zero ne s'ecrit pas.
        let (debit, credit) = (if e.debit.is_zero() { String::new() } else { e.debit.to_string() }, if e.credit.is_zero() { String::new() } else { e.credit.to_string() });
        dire!("  {} {} {:<8} {:<60} {:>10} {:>10}  {}", e.journal, e.date, e.compte, debut(&e.libelle, 60), debit, credit, e.analytique);
    }
    for a in &r.avertissements {
        dire!("  ! {}", a);
    }
    if let Some(f) = &r.fichier {
        dire!("Ecrit : {}", f.display());
    }
    Ok(0)
}
