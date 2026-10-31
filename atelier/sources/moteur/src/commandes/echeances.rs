//! `moteur echeancier [--json]` et `moteur rappels [--json]`.
//! Tout passe par les routes du moteur (`echeancier`, `rappels`) : la fenetre fait les memes appels, et un rappel lu ici
//! est exactement celui qu'elle montre a l'ouverture.
//!
//! `echeancier --fait CLE` / `--defait CLE` (cocher une echeance) passe par la route `echeance-faite`.

use serde_json::{json, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, enveloppe, rendre, texte};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC_ECHEANCIER: Spec = Spec {
    commande: "echeancier",
    positionnels: &[],
    options: &[Opt::valeur("fait"), Opt::valeur("defait"), Opt::drapeau("json")],
};

pub static SPEC_RAPPELS: Spec = Spec { commande: "rappels", positionnels: &[], options: &[Opt::drapeau("json")] };

fn marque(etat: &str) -> &'static str {
    match etat {
        "fait" => "ok",
        "a_faire" => "->",
        "en_retard" => "!!",
        _ => "  ",
    }
}

pub fn echeancier(cadre: &Cadre, args: &Args) -> i32 {
    // `--fait` ou `--defait` : une valeur vide ne coche rien (la commande rend alors la liste).
    let cle = args.texte("fait").filter(|t| !t.is_empty()).or(args.texte("defait").filter(|t| !t.is_empty()));
    if let Some(cle) = cle {
        return cocher(cadre, cle, args.texte("fait").is_some_and(|t| !t.is_empty()));
    }
    match faire_echeancier(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// `echeancier --fait CLE` / `--defait CLE` : dire qu'une echeance que seul l'humain sait faite l'est (ou ne l'est plus). Une note,
/// pas un acte : rien ne part, pas de mot de passe. Ecrit `config/echeancier.json` avec `qui = "ligne de commande"`.
///
/// Une echeance inconnue, ou qui ne se coche pas : `! phrase`, code 2. (`--json` est sans effet sur ce chemin : la reponse
/// est toujours la phrase.)
fn cocher(cadre: &Cadre, cle: &str, fait: bool) -> i32 {
    match cadre.route("echeance-faite", json!({"cle": cle, "fait": fait, "qui": "ligne de commande"})) {
        Ok(r) => {
            dire!("{} : {}.", champ(&r, "cle"), if r.get("fait").is_some_and(socle::fichiers::vrai) { "faite" } else { "a faire" });
            0
        }
        Err(e) if super::actes::est_operation(e.classe) => {
            dire!("! {}", super::actes::phrase(&e));
            2
        }
        Err(e) => cadre.plantage(e),
    }
}

fn faire_echeancier(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let r = cadre.route("echeancier", json!({}))?;
    if args.drapeau("json") {
        // L'enveloppe JSON : le code de sortie est 0.
        rendre(&enveloppe("echeancier", r));
        return Ok(0);
    }
    let compagnie = r.get("compagnie").filter(|v| socle::fichiers::vrai(v)).map(texte);
    dire!("Nous sommes le {}. {}{}.", champ(&r, "date"), champ(&r, "bienvenue"), compagnie.map(|c| format!(" -- {}", c)).unwrap_or_default());
    dire!();
    dire!("CE MOIS-CI");
    let vide = vec![json!({"etat": "information", "titre": "rien a faire", "date": null, "cle": ""})];
    let ce_mois: Vec<Value> = r.get("ce_mois").and_then(Value::as_array).filter(|a| !a.is_empty()).cloned().unwrap_or(vide);
    for x in &ce_mois {
        let etat = champ(x, "etat");
        let avant = match x.get("date") {
            Some(d) if socle::fichiers::vrai(d) && etat != "fait" => format!(" (avant le {})", texte(d)),
            _ => String::new(),
        };
        dire!("  {} {}{}", marque(&etat), champ(x, "titre"), avant);
    }
    dire!();
    dire!("A VENIR");
    for x in r.get("a_venir").and_then(Value::as_array).cloned().unwrap_or_default() {
        dire!("  {}  {} {}", champ(&x, "mois"), marque(&champ(&x, "etat")), champ(&x, "titre"));
    }
    Ok(0)
}

pub fn rappels(cadre: &Cadre, args: &Args) -> i32 {
    match faire_rappels(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn faire_rappels(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let r = cadre.route("rappels", json!({}))?;
    let liste: Vec<Value> = r.get("rappels").and_then(Value::as_array).cloned().unwrap_or_default();
    if args.drapeau("json") {
        rendre(&enveloppe("rappels", Value::Array(liste)));
        return Ok(0);
    }
    if liste.is_empty() {
        dire!("Rien a rappeler.");
        return Ok(0);
    }
    for x in &liste {
        let en_retard = x.get("en_retard").is_some_and(socle::fichiers::vrai);
        dire!("{} {}", if en_retard { "!" } else { "-" }, champ(x, "titre"));
        dire!("    {}", champ(x, "question"));
        if x.get("detail").is_some_and(socle::fichiers::vrai) {
            dire!("    {}", champ(x, "detail"));
        }
        if x.get("fichier").is_some_and(socle::fichiers::vrai) {
            dire!("    a deposer : {}", champ(x, "fichier"));
        }
        for s in x.get("spectacles").and_then(Value::as_array).cloned().unwrap_or_default() {
            let objet = match s.get("numero_objet") {
                Some(o) if socle::fichiers::vrai(o) => texte(o),
                _ => "aucun numero".to_string(),
            };
            dire!("    {} : {}", champ(&s, "titre"), objet);
        }
    }
    Ok(0)
}
