//! `paie exporter [ETAT] [--mois AAAA-MM] [--annee AAAA] [--toutes-les-annees] [--par ...] [--format ...] [--logiciel ...]
//! [--analytique|--sans-analytique] [--coordonnees] [--apercu] [--json]` : les etats pour le comptable, les organismes, les
//! financeurs (`moteur/commandes_exports.py`, `cmd_exporter` ; `operations_exports.exporter`).
//!
//! Sans etat : la liste de ce qui existe (texte ou `--json`). Avec un etat : l'etat est calcule par le moteur
//! (`routes_ecritures_g4::exporter_objet`, la route `exporter`) et, sauf `--apercu`, ECRIT dans `exports/` -- les memes fichiers que le
//! Python. Le texte est rendu a partir de l'objet (le livre de paie, le tableau, la regularisation CASC-SVP), pas du JSON : le
//! chemin ecrit y est entier, comme au Python. Rien n'est lu au coffre, rien ne part : pas de mot de passe.
//!
//! Sans `--format` ni `--logiciel`, les ecritures comptables suivent le logiciel retenu (`paie compta-logiciel`) ; ailleurs, le CSV.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : toute exception qui n'etait pas une `ErreurOperation` (un plan comptable abime,
//! une saisie illisible) sortait en trace d'appels ; ici, un plantage propre (une ligne `Classe: phrase` sur la sortie d'erreur et au
//! journal des incidents, code 1), comme les autres commandes.
//!
//! POINT DOUTEUX : `--analytique` et `--sans-analytique` ensemble : argparse retient le dernier tape ; l'analyse des options ne
//! garde pas l'ordre, `--sans-analytique` l'emporte ici.

use serde_json::{json, Value};

use super::actes::est_refus;
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{debut, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;
use crate::routes_ecritures_g4::{exporter_objet, Exporte};
use documents::tableaux_export::{texte, Tableau};

const REGROUPEMENTS: &[&str] = &["salarie", "mois", "spectacle", "organisme"];
const FORMATS: &[&str] = &["csv", "tsv", "xlsx", "pdf", "html", "json", "fec", "ximport", "oxygene", "sage", "quadra", "ebp", "pennylane"];
pub(super) const LOGICIELS: &[&str] = &["oxygene", "sage", "quadra", "ciel", "ebp", "pennylane", "paheko", "acd", "loop", "isacompta", "autre"];

pub static SPEC: Spec = Spec {
    commande: "exporter",
    positionnels: &[Pos::facultatif("etat", None)],
    options: &[
        Opt::valeur("mois"),
        Opt::valeur("annee"),
        Opt::drapeau("toutes-les-annees"),
        Opt::choix("par", REGROUPEMENTS, None),
        Opt::choix("format", FORMATS, None),
        Opt::choix("logiciel", LOGICIELS, None),
        Opt::drapeau("analytique"),
        Opt::drapeau("sans-analytique"),
        Opt::drapeau("coordonnees"),
        Opt::drapeau("apercu"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) => super::actes::reponse_metier("exporter", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

/// `m` : un montant, ou deux tirets quand la source ne le declare pas.
fn m(v: &Option<socle::num::Decimal>) -> String {
    v.map_or_else(|| "--".to_string(), |d| d.to_string())
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let Some(etat) = args.texte("etat").filter(|t| !t.is_empty()) else {
        return Ok(liste(args.drapeau("json")));
    };
    let ecritures = etat == "ecritures-comptables";
    let analytique = if args.drapeau("sans-analytique") {
        Some(false)
    } else if args.drapeau("analytique") {
        Some(true)
    } else {
        None
    };
    let logiciel = args.texte("logiciel").filter(|t| !t.is_empty());
    let mut charge = json!({
        "etat": etat,
        "mois": args.texte("mois"),
        "annee": args.texte("annee"),
        "toutes_les_annees": args.drapeau("toutes-les-annees"),
        "regroupement": args.texte("par").filter(|t| !t.is_empty()).unwrap_or("salarie"),
        "coordonnees": args.drapeau("coordonnees"),
        "apercu": args.drapeau("apercu"),
    });
    if let Some(f) = args.texte("format").filter(|t| !t.is_empty()) {
        charge["format"] = json!(f);
    }
    if logiciel.is_some() || analytique.is_some() {
        if !ecritures {
            return Err(Echec { classe: Some("ErreurOperation"), message: "--logiciel et --analytique ne valent que pour ecritures-comptables.".to_string() });
        }
        if let Some(l) = logiciel {
            charge["logiciel"] = json!(l);
        }
        if let Some(a) = analytique {
            charge["analytique"] = json!(a);
        }
    }
    if ecritures {
        // Sans --format ni --logiciel, c'est le logiciel retenu (`compta-logiciel`) qui choisit.
        charge["format_du_logiciel_retenu"] = json!(true);
    } else if charge.get("format").is_none() {
        charge["format"] = json!("csv");
    }
    let r = exporter_objet(&cadre.ctx, &charge)?;
    if args.drapeau("json") {
        let v = match &r {
            Exporte::Casc(v) => v.clone(),
            Exporte::Livre(l) => l.json(),
            Exporte::Tableau(t) => documents::tableaux_export::brut_tableau(t),
        };
        return Ok(rendre(&enveloppe("exporter", v)));
    }
    match r {
        Exporte::Casc(v) => casc(&v),
        Exporte::Tableau(t) => tableau(&t),
        Exporte::Livre(l) => livre(&l),
    }
    Ok(0)
}

/// Sans etat : ce que le logiciel sait exporter.
fn liste(json_: bool) -> i32 {
    let t = crate::routes_dossiers::tables();
    let etats: Vec<Value> = t["etats"]
        .as_array()
        .map(|a| a.iter().map(|e| json!({"etat": e["etat"], "quoi": e["quoi"], "famille": e["famille"], "formats": e.get("formats").cloned().unwrap_or(json!([]))})).collect())
        .unwrap_or_default();
    if json_ {
        return rendre(&enveloppe("exporter", Value::Array(etats)));
    }
    dire!("Ce que le logiciel sait exporter :");
    let mut famille = String::new();
    for e in &etats {
        let f = super::rendu::champ(e, "famille");
        if f != famille {
            famille = f;
            dire!();
            dire!("  {}", famille);
        }
        dire!("    {:<26} {}", super::rendu::champ(e, "etat"), super::rendu::champ(e, "quoi"));
        let formats = super::actes::textes_de(e, "formats");
        if !formats.is_empty() {
            dire!("    {:<26} formats : {}", "", formats.join(", "));
        }
    }
    dire!();
    let formats: Vec<String> = t["formats"].as_array().map(|a| a.iter().map(|f| format!("{} ({})", super::rendu::champ(f, "format"), super::rendu::champ(f, "quoi"))).collect()).unwrap_or_default();
    dire!("Formats : {}", formats.join(", "));
    dire!();
    dire!("Ecritures comptables, par logiciel (--logiciel) :");
    for l in t["logiciels"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        dire!(
            "    {:<10} {:<28} {:<10} {}",
            super::rendu::champ(l, "logiciel"),
            super::rendu::champ(l, "nom"),
            super::rendu::champ(l, "format"),
            super::rendu::champ(l, "pourquoi")
        );
    }
    0
}

/// `_afficher_livre`.
fn livre(l: &documents::etats_paie::LivreDePaie) {
    let regroupement = match l.regroupement.as_str() {
        "salarie" => "par salarié",
        "mois" => "par mois",
        "spectacle" => "par spectacle",
        autre => autre,
    };
    dire!("Livre de paie -- {}, {}", l.periode, regroupement);
    for g in &l.groupes {
        dire!("");
        dire!("  {}", g.libelle);
        for x in &g.lignes {
            dire!(
                "    {}  {:<12} {:<28} {:>10} {:>10} {:>10} {:>10} {:>10}",
                x.mois,
                debut(&x.contrat, 12),
                debut(&x.salarie, 28),
                m(&x.brut),
                m(&x.cotisations_salariales),
                m(&x.cotisations_patronales),
                m(&x.net_a_payer),
                m(&x.pas)
            );
        }
        let t = &g.totaux;
        dire!(
            "    {:<51} {:>10} {:>10} {:>10} {:>10} {:>10}  ({} bulletins)",
            "total",
            t.somme("brut"),
            t.somme("cotisations_salariales"),
            t.somme("cotisations_patronales"),
            t.somme("net_a_payer"),
            t.somme("pas"),
            t.bulletins
        );
    }
    let t = &l.totaux;
    dire!("");
    dire!(
        "  {:<51} {:>10} {:>10} {:>10} {:>10} {:>10}  ({} bulletins)",
        "TOTAL GENERAL",
        t.somme("brut"),
        t.somme("cotisations_salariales"),
        t.somme("cotisations_patronales"),
        t.somme("net_a_payer"),
        t.somme("pas"),
        t.bulletins
    );
    dire!("  cout employeur {}, net imposable {}", t.somme("cout_employeur"), t.somme("net_imposable"));
    // UN TOTAL PARTIEL LE DIT.
    let mut inconnus = t.inconnus.clone();
    inconnus.sort();
    for (champ, n) in inconnus {
        dire!("  {} : total sur {} bulletin(s) sur {} -- {} ne le declarent pas", champ.replace('_', " "), t.bulletins - n, t.bulletins, n);
    }
    for a in &l.avertissements {
        dire!("  ! {}", a);
    }
    if let Some(f) = &l.fichier {
        dire!("Ecrit : {}", f.display());
    }
}

/// `_afficher_tableau` : l'en-tete, les lignes, les totaux -- colonnes tronquees a l'ecran ; le fichier, lui, porte tout.
fn tableau(t: &Tableau) {
    use documents::tableaux_export::{Ligne, Val};
    dire!("{} -- {}", t.titre, t.periode);
    let rangee = |d: &Ligne| -> String {
        let cellules: Vec<String> = t
            .colonnes
            .iter()
            .map(|c| {
                let largeur = if c.a_droite() { 12 } else { 18 };
                let s = debut(&texte(d.val(&c.cle), c), largeur);
                if c.a_droite() {
                    format!("{:>w$}", s, w = largeur)
                } else {
                    format!("{:<w$}", s, w = largeur)
                }
            })
            .collect();
        format!("  {}", cellules.join(" "))
    };
    let mut entete = Ligne::nouvelle();
    for c in &t.colonnes {
        entete.poser(&c.cle, Some(Val::txt(c.libelle.clone())));
    }
    dire!("{}", rangee(&entete));
    for sec in &t.sections {
        if !sec.libelle.is_empty() {
            dire!("");
            dire!("  {}", sec.libelle);
        }
        for l in &sec.lignes {
            dire!("{}", rangee(l));
        }
        if let Some(tot) = &sec.total {
            if t.sections.len() > 1 {
                dire!("{}", rangee(tot));
            }
        }
    }
    if let Some(tot) = &t.total {
        dire!("");
        dire!("{}", rangee(tot));
    }
    for a in &t.avertissements {
        dire!("  ! {}", a);
    }
    for x in &t.sources {
        dire!("  Source : {}", x);
    }
    if let Some(f) = &t.fichier {
        dire!("Ecrit : {}", f.display());
    }
}

/// La regularisation CASC-SVP.
fn casc(r: &Value) {
    use super::rendu::champ;
    dire!("Regularisation CASC-SVP -- {} -- total {} EUR", champ(r, "periode"), champ(r, "total"));
    for x in r.get("mois").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        dire!("  {:<16} masse {:>10}  {} %  {:>8}", champ(x, "libelle"), champ(x, "masse"), champ(x, "taux"), champ(x, "cotisation"));
    }
    dire!("");
    for x in super::actes::textes_de(r, "a_faire") {
        if x.starts_with(' ') {
            dire!("{}", x);
        } else {
            dire!("- {}", x);
        }
    }
    for a in super::actes::textes_de(r, "avertissements") {
        dire!("! {}", a);
    }
    if r.get("dossier").is_some_and(socle::fichiers::vrai) {
        dire!("");
        dire!("Ecrit dans {}", champ(r, "dossier"));
    }
}
