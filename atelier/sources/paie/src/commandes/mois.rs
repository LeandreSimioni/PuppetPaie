//! `paie mois FICHIER [--constitution JJ/MM/AAAA] [--forcer] [--test] [--json]` : produire un mois -- bulletins, certificats
//! Conges Spectacles et DSN (`moteur/cli.py`, `cmd_mois` ; `operations_paies.fermer_mois`). Rien n'est depose : le depot est une
//! commande a part (`paie depot-dsn`).
//!
//! Elle passe par la route `produire` du moteur (`produire_mois::produire_pour`), celle que la fenetre appelle : on ne ferme
//! qu'un mois pret (la DPAE de chaque embauche partie, son contrat parti a la signature), tout le mois ou rien, les compteurs
//! sous verrou. La saisie lue dans le fichier est celle qui se produit ; la route l'enregistre d'abord, comme pour la fenetre.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : un fichier introuvable, une date de constitution illisible, un mois dont la
//! DSN etait due (`MoisHorsDelai`), un exercice clos, une saisie incomplete donnaient une trace d'appels (seuls `MoisPasPret` et
//! `CompteursVerrouilles` etaient attrapes). Ici : une phrase (`! ...`) et le code 2 -- ou l'erreur `--json`. Le fichier de
//! constitution mal ecrit est refuse AVANT d'ecrire quoi que ce soit.

use serde_json::{json, Value};

use super::actes::{afficher_etapes, afficher_fichiers, est_operation, fmt_eur_texte, phrase, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "mois",
    positionnels: &[Pos::requis("fichier")],
    options: &[Opt::valeur("constitution"), Opt::drapeau("forcer"), Opt::drapeau("test"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

fn refus(m: String) -> Echec {
    Echec { classe: Some("ErreurOperation"), message: m }
}

/// Les trois nombres d'un `CompteursVerrouilles` (age, detenteur, chemin du verrou), relus de sa phrase.
fn details_du_verrou(message: &str) -> Vec<(&'static str, Value)> {
    let age = message.split("depuis ").nth(1).and_then(|r| r.split(" s").next()).and_then(|n| n.trim().parse::<i64>().ok());
    let detenteur = message.split(" s (").nth(1).and_then(|r| r.split(").").next()).map(str::to_string);
    let verrou = message.lines().last().map(|l| l.trim().to_string());
    vec![("detenteur", json!(detenteur)), ("age", json!(age)), ("verrou", json!(verrou))]
}

/// `_afficher_apercu` : le mois tel qu'il a ete calcule, une ligne par paie.
fn afficher_apercu(r: &Value, a_regarder: &[String], informations: &[String]) {
    let paies: Vec<Value> = r.get("paies").and_then(Value::as_array).cloned().unwrap_or_default();
    dire!("\n{} - {} paie(s)\n{}", champ(r, "libelle"), paies.len(), "-".repeat(58));
    for p in &paies {
        let salarie = p.get("salarie").cloned().unwrap_or(Value::Null);
        let designations: Vec<String> =
            p.get("remuneration").and_then(Value::as_array).map(|l| l.iter().map(|x| champ(x, "designation")).collect()).unwrap_or_default();
        let quoi = designations.join(", ");
        dire!(
            "  {:<9} {:<22} {:<30} brut {:>8}  net {:>8}  cout {:>8}",
            champ(p, "numero_bulletin"),
            format!("{} {}", champ(&salarie, "nom"), champ(&salarie, "prenom")),
            quoi.chars().take(30).collect::<String>(),
            fmt_eur_texte(&p["brut"]),
            fmt_eur_texte(&p["net_a_payer"]),
            fmt_eur_texte(&p["cout_employeur"])
        );
    }
    dire!(
        "{}\n  TOTAL                              brut {:>8}              cout {:>8}\n",
        "-".repeat(58),
        fmt_eur_texte(&r["total_brut"]),
        fmt_eur_texte(&r["total_cout"])
    );
    for a in a_regarder {
        dire!("  /!\\  {}", a);
    }
    for a in informations {
        dire!("       {}", a);
    }
    if !a_regarder.is_empty() || !informations.is_empty() {
        dire!("");
    }
}

fn textes(r: &Value, cle: &str) -> Vec<String> {
    r.get(cle).and_then(Value::as_array).map(|l| l.iter().map(super::rendu::texte).collect()).unwrap_or_default()
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    // Le fichier : tel quel s'il existe, sinon sous les donnees ; un mois nu (`2026-10`) designe sa saisie.
    let chemin = match crate::routes_apercus::resoudre(&cadre.racine, &json!(args.texte("fichier").unwrap_or(""))) {
        Ok(c) => c,
        Err(e) => return Ok(reponse_metier("mois", json, &Echec { classe: Some("ErreurOperation"), message: e.message }, &[])),
    };
    let saisie = match crate::routes_saisie::lire_saisie(std::path::Path::new(&chemin)) {
        Ok(s) => s,
        Err(e) => {
            if json {
                return Ok(rendre(&erreur("mois", e.classe.unwrap_or("OSError"), &e.message, &[])));
            }
            dire!("Fichier illisible : {}", e.message);
            return Ok(1);
        }
    };
    let Some(mois) = saisie.get("mois").and_then(Value::as_str).map(str::to_string) else {
        return Ok(reponse_metier("mois", json, &refus(format!("{} ne dit pas son mois (cle `mois`, AAAA-MM).", chemin)), &[]));
    };
    let constitution = match args.texte("constitution") {
        None => None,
        Some(t) => match socle::date::Jour::lire(t) {
            Ok(j) => Some(j),
            Err(_) => {
                return Ok(reponse_metier("mois", json, &refus(format!("La date de constitution « {} » ne se lit pas : JJ/MM/AAAA attendu.", t)), &[]));
            }
        },
    };

    let charge = json!({"mois": mois, "saisie": saisie, "test": args.drapeau("test"), "forcer": args.drapeau("forcer")});
    let (r, (apercu_a_regarder, apercu_informations)) = match crate::produire_mois::produire_pour(&cadre.o, &cadre.ctx, &charge, "cli", constitution) {
        Ok(x) => x,
        Err(e) => return mois_refuse(cadre, json, &saisie_de(&charge), e),
    };
    if json {
        return Ok(rendre(&enveloppe("mois", r)));
    }
    afficher_apercu(&r, &apercu_a_regarder, &apercu_informations);
    let (avertissements, informations) = (textes(&r, "avertissements"), textes(&r, "informations"));
    for a in &avertissements {
        dire!(">>> {}", a);
    }
    for a in &informations {
        dire!("    {}", a);
    }
    if !avertissements.is_empty() || !informations.is_empty() {
        dire!();
    }
    let dossier = champ(&r, "dossier");
    let fichiers: Vec<Value> = r.get("fichiers").and_then(Value::as_array).cloned().unwrap_or_default();
    afficher_fichiers(&fichiers, &dossier);
    let etapes: Vec<Value> = r.get("etapes").and_then(Value::as_array).cloned().unwrap_or_default();
    afficher_etapes(&etapes, "A faire ensuite :");
    dire!("\n  Les pieces individuelles sont deja ecrites dans {}.", std::path::Path::new(&dossier).join("envoi").display());
    Ok(0)
}

fn saisie_de(charge: &Value) -> Value {
    charge.get("saisie").cloned().unwrap_or(Value::Null)
}

/// Ce que le Python attrapait (`MoisPasPret`, `CompteursVerrouilles`) et, en plus, toute `ErreurOperation` : une phrase.
fn mois_refuse(cadre: &Cadre, json: bool, saisie: &Value, e: Echec) -> Result<i32, Echec> {
    match e.classe {
        Some("MoisPasPret") => {
            let donnee: Value = serde_json::from_str(&e.message).unwrap_or(Value::Null);
            if json {
                let manques = donnee.get("manques").cloned().unwrap_or_else(|| json!([]));
                let apercu = cadre.route("apercu", json!({"saisie": saisie}))?;
                return Ok(rendre(&erreur("mois", "MoisPasPret", &phrase(&e), &[("manques", manques), ("apercu", apercu)])));
            }
            dire!("{}", phrase(&e));
            Ok(2)
        }
        Some("CompteursVerrouilles") => {
            if json {
                return Ok(rendre(&erreur("mois", "CompteursVerrouilles", &e.message, &details_du_verrou(&e.message))));
            }
            dire!("{}", e.message);
            Ok(2)
        }
        c if est_operation(c) => Ok(reponse_metier("mois", json, &e, &[])),
        _ => Err(e),
    }
}
