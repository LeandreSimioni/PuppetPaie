//! `moteur pas [--amorcer]` : les taux de prelevement a la source connus, et l'amorcage depuis les DSN deposees ; le
//! magasin `config/pas.json` s'ecrit dans `compagnie::taux_pas`.
//!
//! `--demander` (ecrire les signalements d'amorcage, nature 08 ; rien n'est depose) : `demander`, sur `amorcage_demande`.

use serde_json::{json, Map, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::champ;
use crate::compagnie::taux_pas::ko;
use socle::date::jj_mm_aaaa;
use super::Cadre;
use droit::contexte::Contexte;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "pas",
    positionnels: &[],
    options: &[
        Opt::drapeau("amorcer"),
        Opt::drapeau("demander"),
        Opt::avec_defaut("motif", "07"),
        Opt::ajout("salarie"),
        Opt::valeur("mois"),
        Opt::drapeau("test"),
        Opt::drapeau("forcer"),
        Opt::drapeau("json"),
    ],
};

/// Cahier technique 2026.1, rubrique S21.G00.50.007.
pub const TYPES: [(&str, &str); 8] = [
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
    if args.drapeau("demander") {
        return demander(cadre, args);
    }
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn fr(iso: Option<&str>) -> String {
    match iso {
        Some(i) if !i.is_empty() => format!("{}/{}/{}", &i[8..10], &i[5..7], &i[..4]),
        _ => "?".into(),
    }
}


/// Pose l'etat courant a partir de la derniere DSN deposee de chaque salarie.
fn amorcer(ctx: &Contexte, dossier: &std::path::Path) -> Result<Vec<String>, Echec> {
    let releves = crate::compagnie::taux_pas::derniers_taux_des_dsn(ctx, dossier);
    let nom = dossier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    crate::compagnie::taux_pas::enregistrer(ctx, &releves, &format!("amorcage depuis les DSN deposees ({})", nom))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    if args.drapeau("amorcer") {
        let export = crate::routes::fiches::pieces_importees(ctx);
        if !export.exists() {
            dire!("Dossier introuvable : {}", export.display());
            return Ok(1);
        }
        let chgts = amorcer(ctx, &export)?;
        dire!("Amorcage depuis les DSN deposees ({}) :", export.display());
        for c in &chgts {
            dire!("  {}", c);
        }
        if chgts.is_empty() {
            dire!("  rien a changer.");
        }
        dire!("\nCes valeurs sont celles REELLEMENT DECLAREES dans les dernieres DSN, pas des\nvaleurs calculees. Elles tiennent jusqu'au prochain compte-rendu DGFiP :\n    moteur retours\n");
        return Ok(0);
    }

    let etat = droit::pas::charger(ctx).map_err(ko)?;
    let salaries: Map<String, Value> = etat.get("salaries").and_then(Value::as_object).cloned().unwrap_or_default();
    if salaries.is_empty() {
        dire!("Magasin vide. `moteur pas --amorcer` pour partir de l'historique, ou `moteur retours` pour lire un CRM.");
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

    // CE QU'IL FAUDRAIT DEMANDER. Un taux perime n'est plus le taux du salarie : deux mois apres sa mise a disposition,
    // c'est la grille de l'article 204 H qui s'applique, et le moteur ne sait pas encore la calculer.
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
        dire!("\nPour ecrire les signalements (un fichier par individu) :\n    moteur pas --demander\nReponse de la DGFiP sous cinq jours ouvres, par un CRM de nature 94 que\n`moteur retours` ramassera comme les autres.\n");
    }
    for s in diag.iter().filter(|s| s.get("situation").and_then(Value::as_str) == Some(droit::pas::SANS_NIR)) {
        dire!("  {:<12} {} -- rien a demander, c'est le bareme.", champ(s, "salarie"), champ(s, "libelle"));
    }
    dire!("S21.G00.50.006/.007/.008 sortent d'ici. Aucune n'est calculee : elles\nviennent du compte-rendu metier DGFiP de nature 94.");
    Ok(0)
}

/// `pas --demander` (`_demander_les_taux`) : ecrire les signalements d'amorcage (nature 08). RIEN N'EST DEPOSE ici : le depot est un
/// acte a part (`moteur depot-dsn`), avec sa preuve et son mot de passe -- et c'est lui qui fait courir les cinq jours ouvres, donc lui
/// qui note la demande. Un refus de la norme ou de la demande est une phrase (code 1), jamais une trace.
fn demander(cadre: &Cadre, args: &Args) -> i32 {
    use crate::compagnie::amorcage_demande::{produire, Demande};
    let ctx = &cadre.ctx;
    let motif = args.texte("motif").unwrap_or("07");
    // Le mois : celui qu'on dit, sinon le dernier saisi (c'est le contrat que la derniere DSN a declare, et ce que le motif 07 reprend).
    let mois = match args.texte("mois") {
        Some(m) => {
            if !socle::date::mois_valide(m) {
                dire!("--mois attend AAAA-MM, recu {}.", socle::fichiers::repr(m));
                return 1;
            }
            m.to_string()
        }
        None => match engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().into_iter().filter(|m| socle::date::mois_valide(m)).last() {
            Some(m) => m,
            None => {
                dire!("Aucun mois saisi : un signalement d'amorcage declare un CONTRAT (bloc 40, cardinalite (1,1)), et il n'y en a aucun a declarer.");
                return 1;
            }
        },
    };
    let paies = match envois::journal::lu(&ctx.racine).map(|j| j.departs).and_then(|departs| declarations::dsn::contrats_du_mois(ctx, &mois, &departs)) {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => {
            dire!("Aucun engagement valide ne touche {} : un signalement d'amorcage declare le contrat d'un engagement valide. Rien n'est ecrit.", mois);
            return 1;
        }
        Err(m) => {
            dire!("{}", m);
            return 1;
        }
    };
    let salaries = args.liste("salarie");
    let demande = Demande {
        motif,
        mois: &mois,
        salaries: if args.drapeau("salarie") { Some(&salaries) } else { None },
        envoi_test: args.drapeau("test"),
        forcer: args.drapeau("forcer"),
        constitution: cadre.aujourdhui,
    };
    let mut ecrits = Vec::new();
    let lot = match produire(ctx, &paies, &demande, &mut ecrits) {
        Ok(l) => l,
        Err(m) => {
            dire!("{}", m);
            return 1;
        }
    };
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
    let premier = lot.fichiers.first().map(|f| f.display().to_string()).unwrap_or_default();
    dire!(
        "\nUN SIGNALEMENT, UN INDIVIDU : chaque fichier se depose separement.\n    moteur depot-dsn {} --test\npuis, une fois le bilan lu, avec --reel. La DGFiP repond sous cinq jours\nouvres, par un CRM de nature 94 :\n    moteur retours\n",
        premier
    );
    if args.drapeau("test") {
        dire!("Ces fichiers sont des ESSAIS (S10.G00.00.005 = 01) : ils ne demandent rien.");
    }
    0
}
