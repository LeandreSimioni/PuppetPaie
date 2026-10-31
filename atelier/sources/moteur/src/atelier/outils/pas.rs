//! `moteur-atelier pas [--amorcer] [--demander [--motif 07] [--salaries A,B] [--mois AAAA-MM] [--test] [--forcer]]` : les
//! taux de prelevement a la source connus (`config/pas.json`, `droit::pas`), l'amorcage depuis les DSN deposees
//! (`taux_pas::derniers_taux_des_dsn`), et les signalements d'amorcage a ecrire (`amorcage_demande::produire`, nature 08 :
//! ecrits, jamais deposes ici).

use serde_json::{Map, Value};

use crate::commandes::analyse::{Args, Opt, Spec};
use crate::commandes::rendu::champ;
use crate::commandes::Cadre;
use crate::compagnie::taux_pas::ko;
use socle::date::jj_mm_aaaa;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "pas",
    positionnels: &[],
    options: &[
        Opt::drapeau("amorcer"),
        Opt::drapeau("demander"),
        Opt::avec_defaut("motif", "07"),
        Opt::valeur("salaries"),
        Opt::valeur("mois"),
        Opt::drapeau("test"),
        Opt::drapeau("forcer"),
    ],
};

/// Les types de taux, cahier technique 2026.1, rubrique S21.G00.50.007 (`atelier/reference/sources/norme-dsn/texte/`).
const TYPES: [(&str, &str); 8] = [
    ("01", "Taux transmis par la DGFIP"),
    ("13", "Bareme mensuel metropole"),
    ("17", "Bareme mathematique sur base mensuelle metropole"),
    ("23", "Bareme mensuel Guadeloupe, Reunion et Martinique"),
    ("27", "Bareme mathematique sur base mensuelle Guadeloupe, Reunion, Martinique"),
    ("33", "Bareme mensuel Guyane et Mayotte"),
    ("37", "Bareme mathematique sur base mensuelle Guyane et Mayotte"),
    ("99", "Indu relatif a un exercice anterieur - pas de taux de PAS"),
];

pub fn libelle_type(t: &str) -> &'static str {
    TYPES.iter().find(|(c, _)| *c == t).map(|(_, l)| *l).unwrap_or("?")
}

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let r = if args.drapeau("demander") {
        demander(cadre, args)
    } else if args.drapeau("amorcer") {
        amorcer(cadre)
    } else {
        lire(cadre)
    };
    r.unwrap_or_else(|e| super::refus(&e))
}

fn fr(iso: Option<&str>) -> String {
    match iso {
        Some(i) if i.len() >= 10 => jj_mm_aaaa(i),
        _ => "?".into(),
    }
}

/// `--amorcer` : l'etat courant pose depuis la derniere DSN deposee de chaque salarie (celles de l'import).
fn amorcer(cadre: &Cadre) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    let export = crate::routes::fiches::pieces_importees(ctx);
    if !export.exists() {
        dire!("Dossier introuvable : {}", export.display());
        return Ok(1);
    }
    let releves = crate::compagnie::taux_pas::derniers_taux_des_dsn(ctx, &export);
    let nom = export.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let chgts = crate::compagnie::taux_pas::enregistrer(ctx, &releves, &format!("amorcage depuis les DSN deposees ({})", nom))?;
    dire!("Amorcage depuis les DSN deposees ({}) :", export.display());
    for c in &chgts {
        dire!("  {}", c);
    }
    if chgts.is_empty() {
        dire!("  rien a changer.");
    }
    dire!("\nCes valeurs sont celles REELLEMENT DECLAREES dans les dernieres DSN, pas des valeurs calculees. Elles tiennent jusqu'au\nprochain compte-rendu DGFiP (la releve des retours, route `ouverture`).");
    Ok(0)
}

/// Les taux connus, et ce qu'il faudrait demander a la DGFiP.
fn lire(cadre: &Cadre) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    let etat = droit::pas::charger(ctx).map_err(ko)?;
    let salaries: Map<String, Value> = etat.get("salaries").and_then(Value::as_object).cloned().unwrap_or_default();
    if salaries.is_empty() {
        dire!("Aucun taux connu. `pas --amorcer` part de l'historique importe ; la releve des retours lit les CRM de nature 94.");
        return Ok(0);
    }
    dire!("Taux de prelevement a la source connus\n{}", "-".repeat(62));
    for (cle, e) in &salaries {
        let s = droit::pas::situation(ctx, cle, None).map_err(ko)?;
        dire!("  {:<12} taux {:<6} type {} ({})", cle, champ(e, "taux"), champ(e, "type"), libelle_type(&champ(e, "type")));
        dire!("  {:<12} identifiant {}", "", champ(e, "identifiant"));
        let jusqu = match s.get("valable_jusqu_a") {
            Some(Value::String(j)) => format!(", jusqu'au {}", jj_mm_aaaa(j)),
            _ => String::new(),
        };
        dire!("  {:<12} {}{}", "", champ(&s, "libelle"), jusqu);
        let periode = if e.get("periode").is_some_and(socle::fichiers::vrai) { format!(" (periode {})", champ(e, "periode")) } else { String::new() };
        dire!("  {:<12} source : {}{}\n", "", champ(e, "source"), periode);
    }
    let diag = droit::pas::diagnostic(ctx, None, None).map_err(ko)?;
    let a_demander: Vec<&Value> = diag.iter().filter(|s| s.get("a_demander").is_some_and(socle::fichiers::vrai)).collect();
    if !a_demander.is_empty() {
        dire!("A demander a la DGFiP (signalement d'amorcage, nature 08)\n{}", "-".repeat(62));
        for s in a_demander {
            let attente = match s.get("demande").filter(|d| socle::fichiers::vrai(d)) {
                Some(d) => format!(
                    "  -- demande partie le {}, reponse attendue avant le {}",
                    fr(d.get("quand").and_then(Value::as_str)),
                    fr(d.get("reponse_attendue_avant").and_then(Value::as_str))
                ),
                None => String::new(),
            };
            dire!("  {:<12} {}{}", champ(s, "salarie"), champ(s, "libelle"), attente);
        }
        dire!("\nLes signalements s'ecrivent par `pas --demander` ; la DPAE les fait partir avec elle.");
    }
    for s in diag.iter().filter(|s| s.get("situation").and_then(Value::as_str) == Some(droit::pas::SANS_NIR)) {
        dire!("  {:<12} {} -- rien a demander, c'est le bareme.", champ(s, "salarie"), champ(s, "libelle"));
    }
    Ok(0)
}

/// `--demander` : ecrire les signalements d'amorcage (nature 08), un fichier par individu. RIEN N'EST DEPOSE : le depot est la
/// route `deposer` (avec `fichier`), sa preuve et son mot de passe.
fn demander(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    use crate::compagnie::amorcage_demande::{produire, Demande};
    let ctx = &cadre.ctx;
    let mois = match args.texte("mois") {
        Some(m) if socle::date::mois_valide(m) => m.to_string(),
        Some(m) => {
            dire!("--mois attend AAAA-MM, recu {}.", socle::fichiers::repr(m));
            return Ok(1);
        }
        None => match engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().filter(|m| socle::date::mois_valide(m)).last() {
            Some(m) => m,
            None => {
                dire!("Aucun mois saisi : un signalement d'amorcage declare un contrat (bloc 40), et il n'y en a aucun.");
                return Ok(1);
            }
        },
    };
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let paies = declarations::dsn::contrats_du_mois(ctx, &mois, &departs)?;
    if paies.is_empty() {
        dire!("Aucun engagement valide ne touche {} : rien n'est ecrit.", mois);
        return Ok(1);
    }
    let salaries: Vec<String> = args.texte("salaries").map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let demande = Demande {
        motif: args.texte("motif").unwrap_or("07"),
        mois: &mois,
        salaries: if salaries.is_empty() { None } else { Some(&salaries) },
        envoi_test: args.drapeau("test"),
        forcer: args.drapeau("forcer"),
        constitution: cadre.aujourdhui,
    };
    let mut ecrits = Vec::new();
    let lot = produire(ctx, &paies, &demande, &mut ecrits).map_err(Echec::from)?;
    let nom = |p: &std::path::Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dire!("Signalements d'amorcage des donnees variables (nature 08)");
    dire!("Motif {} - {}\n{}", lot.motif, declarations::amorcage::libelle_motif(&lot.motif).unwrap_or_else(|_| "?".into()), "-".repeat(62));
    for (ligne, f) in lot.lignes.iter().zip(&lot.fichiers) {
        dire!("  {:<12} {:<24} {}", ligne.salarie, ligne.libelle, nom(f));
    }
    for ligne in &lot.ecartes {
        dire!("  {:<12} ecarte : {}", ligne.salarie, ligne.pourquoi_pas);
    }
    dire!("\n  recapitulatif : {}", nom(&lot.recapitulatif));
    dire!("\nUN SIGNALEMENT, UN INDIVIDU : chaque fichier se depose separement, par la route `deposer` (avec `fichier`).");
    if args.drapeau("test") {
        dire!("Ces fichiers sont des ESSAIS (S10.G00.00.005 = 01) : ils ne demandent rien.");
    }
    Ok(0)
}
