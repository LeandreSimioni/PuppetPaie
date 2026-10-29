//! `moteur ouverture [--sans-sauvegarde] [--sans-retours] [--sans-signatures] [--sans-sources] [--jours N] [--pas-avant N]
//! [--veille]` : tout ce qui se fait a l'ouverture (`moteur/commandes_ouverture.py`, `cmd_ouverture`). La route `ouverture` fait le
//! travail (retours DSN et DPAE, signatures, filet DPAE, incidents de l'editeur, sauvegarde, trace) et ne rend jamais d'erreur : une
//! panne est une phrase de `incidents`.
//!
//! La mise a jour se cherche au lancement du logiciel, pas ici ; la verification des sources non plus : `--sans-sources` est
//! accepte et sans effet, `sources` vaut `null` en `--json`, et le texte n'a pas de ligne `Sources`.
//!
//! LE COFFRE. Les retours et les signatures lisent des mots de passe au coffre : `Cadre::ouvrir_le_coffre` d'abord (la cle du poste,
//! ou une seance que le moteur demande lui-meme ; ses phrases vont sur la sortie d'erreur, donc pas dans un `--json`). Aucun mot de
//! passe ne passe par la commande.
//!
//! DANS `moteur-atelier`, HORS COMPAGNIE FICTIVE, la commande ne touche que ce que l'atelier redirige vers un faux serveur local : la boite
//! courriel n'est jamais redirigee (signatures et fiches coupees), la sauvegarde exige le faux Google, les retours exigent le faux
//! net-entreprises ET le faux Urssaf, le filet (le test des DPAE chez l'Urssaf) exige le faux Urssaf, et ce qui part chez l'editeur
//! (incidents) est coupe. Chaque coupure est dite sur la sortie d'erreur. Dans `moteur`, rien n'est coupe.

use serde_json::{json, Value};

#[cfg(feature = "atelier")]
use super::actes::{boite_interdite, pas_redirige, Organisme};
use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, enveloppe, rendre, texte};
use super::Cadre;

pub static SPEC: Spec = Spec {
    commande: "ouverture",
    positionnels: &[],
    options: &[
        Opt::drapeau("sans-sauvegarde"),
        Opt::drapeau("sans-retours"),
        Opt::drapeau("sans-signatures"),
        Opt::drapeau("sans-sources"),
        Opt::entier("jours"),
        Opt::entier("pas-avant"),
        Opt::drapeau("veille"),
        Opt::drapeau("json"),
    ],
};

#[cfg_attr(not(feature = "atelier"), allow(unused_mut))]
pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let mut sauvegarder = !args.drapeau("sans-sauvegarde");
    let mut retours = !args.drapeau("sans-retours");
    let mut signatures = !args.drapeau("sans-signatures");
    let mut filet = true;
    let mut incidents_editeur = true;
    // L'atelier hors compagnie fictive : on ne coupe que ce qui irait au vrai serveur. Ce bloc n'est pas compile dans `moteur`.
    #[cfg(feature = "atelier")]
    {
    let mut coupures: Vec<&str> = Vec::new();
    if signatures && boite_interdite(cadre) {
        signatures = false;
        coupures.push("les signatures et les fiches (la boite courriel n'est jamais redirigee)");
    }
    if sauvegarder && pas_redirige(cadre, Organisme::Google) {
        sauvegarder = false;
        coupures.push("la sauvegarde (pas de faux Google : PUPPETPAIE_ATELIER_GOOGLE)");
    }
    if retours && (pas_redirige(cadre, Organisme::Dsn) || pas_redirige(cadre, Organisme::Dpae)) {
        retours = false;
        coupures.push("les retours (il faut le faux net-entreprises ET le faux Urssaf : PUPPETPAIE_ATELIER_DSN, PUPPETPAIE_ATELIER_DPAE)");
    }
    if pas_redirige(cadre, Organisme::Dpae) {
        filet = false;
        coupures.push("le filet de securite DPAE (pas de faux Urssaf : PUPPETPAIE_ATELIER_DPAE)");
    }
    if boite_interdite(cadre) {
        incidents_editeur = false;
        coupures.push("l'envoi des incidents chez l'editeur");
    }
    for c in &coupures {
        eprintln!("moteur-atelier : coupe hors compagnie fictive : {}.", c);
    }
    }
    if retours || signatures || filet {
        cadre.ouvrir_le_coffre();
    }
    let jours = args.entier("jours").unwrap_or(30);
    let pas_avant = args.entier("pas-avant").unwrap_or(0);
    let r = match cadre.route(
        "ouverture",
        json!({
            "sauvegarder": sauvegarder, "retours": retours, "signatures": signatures, "filet": filet,
            "jours": jours, "pas_avant": pas_avant, "veille": args.drapeau("veille"), "incidents_editeur": incidents_editeur,
        }),
    ) {
        Ok(r) => r,
        Err(e) => return cadre.plantage(e),
    };
    let incidents = r.get("incidents").and_then(Value::as_array).map_or(0, Vec::len);
    if args.drapeau("json") {
        rendre(&enveloppe("ouverture", en_json(&r)));
        // Le Python rend 0 toujours avec `--json`.
        return 0;
    }
    dire_ouverture(&r);
    // Un manque de secret ou de compte Google n'est pas une panne : c'est une chose a faire (code 0). Un incident se signale.
    if incidents > 0 {
        1
    } else {
        0
    }
}

fn liste(v: &Value, cle: &str) -> Vec<Value> {
    v.get(cle).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn objet(v: &Value, cle: &str) -> Option<Value> {
    v.get(cle).filter(|x| x.is_object()).cloned()
}

/// Ce que `--json` rend : `sources` est `null`.
fn en_json(r: &Value) -> Value {
    let get = |v: &Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let sauvegarde = objet(r, "sauvegarde").map(|s| json!({"resume": get(&s, "resume"), "deposes": get(&s, "deposes"), "identiques": get(&s, "identiques"), "octets": get(&s, "octets")}));
    let retours = objet(r, "retours").map(|t| json!({"resume": get(&t, "resume"), "trouves": get(&t, "trouves"), "nouveaux": get(&t, "nouveaux"), "archives": get(&t, "archives")}));
    let signatures = objet(r, "signatures").map(|s| json!({"quand": get(&s, "quand"), "attendues": get(&s, "attendues"), "recues": get(&s, "recues"), "incidents": get(&s, "incidents")}));
    json!({
        "quand": get(r, "quand"),
        "secondes": get(r, "secondes"),
        "sauvegarde": sauvegarde,
        "retours": retours,
        "signatures": signatures,
        "incidents": get(r, "incidents"),
        "manquants": get(r, "manquants"),
        "taux_pas": get(r, "taux_pas"),
    })
}

/// `_dire_ouverture`, sans les lignes `Sources` et `Version` (non portees).
fn dire_ouverture(r: &Value) {
    let secondes = r.get("secondes").and_then(Value::as_f64).unwrap_or(0.0);
    dire!("Ouverture du {} ({:.1} s)", champ(r, "quand"), secondes);
    if let Some(s) = objet(r, "sauvegarde") {
        dire!("  Drive     : {}", champ(&s, "resume"));
        if let Some(souci) = s.get("souci").and_then(Value::as_str) {
            dire!("  ! {}", souci);
        }
        let detail = liste(&s, "deposes_detail");
        for f in detail.iter().take(12) {
            dire!("      {:<6} {}", champ(f, "etat"), champ(f, "relatif"));
        }
        if detail.len() > 12 {
            dire!("      ... et {} autre(s)", detail.len() - 12);
        }
    }
    let retours = objet(r, "retours");
    if let Some(t) = &retours {
        dire!("  Retours   : {}", champ(t, "resume"));
        for a in liste(t, "archives_detail") {
            dire!("      {}  {}", champ(&a, "fichier"), champ(&a, "libelle"));
        }
    }
    if let Some(s) = objet(r, "signatures") {
        let attendues = s.get("attendues").and_then(Value::as_i64).unwrap_or(0);
        let recues = liste(&s, "recues");
        if attendues > 0 || !recues.is_empty() {
            dire!("  Signatures: {}", champ(&s, "resume"));
            for x in &recues {
                let quand: String = texte(x.get("quand").unwrap_or(&Value::Null)).replace('T', " ").chars().take(16).collect();
                let renvoye = if x.get("renvoye").is_some_and(socle::fichiers::vrai) { " -- contrat contresigne renvoye" } else { "" };
                dire!("      {}  signe le {} depuis {} ({}){}", champ(x, "nom"), quand, champ(x, "adresse"), champ(x, "code"), renvoye);
            }
        }
    }
    let taux = liste(r, "taux_pas");
    if !taux.is_empty() {
        dire!("  Taux PAS  : {} salarie(s) sans taux DGFiP valide", taux.len());
        for ligne in &taux {
            dire!("      {}", texte(ligne));
        }
        dire!("      Leur paie sera prelevee sur la GRILLE par defaut (art. 204 H du CGI),");
        dire!("      qui preleve le plus souvent PLUS que le taux reel.");
    }
    for m in liste(r, "manquants") {
        dire!("  ! {}", texte(&m));
    }
    for i in liste(r, "incidents") {
        dire!("  ! {}", texte(&i));
    }
    if let Some(t) = &retours {
        let nouveaux = t.get("nouveaux").and_then(Value::as_i64).unwrap_or(0);
        if nouveaux > 0 {
            dire!("\n{} retour(s) nouveau(x). Les lire en detail :\n  moteur retours", nouveaux);
        }
    }
}
