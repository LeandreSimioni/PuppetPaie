//! `paie corriger [AAAA-MM] [--extraire CLE] [--engagement FICHIER.json] [--salarie CLE] [--debut AAAA-MM-JJ] [--motif TEXTE]
//! [--apercu] [--annuler N] [--fenetre] [--json]` : corriger un mois deja depose, apres son echeance
//! (`moteur/commandes_regularisation.py`).
//!
//! Rien ne part d'ici : la correction attend la prochaine DSN du mois courant, qui l'embarque datee du mois d'origine. La saisie du
//! mois d'origine n'est jamais retouchee. Routes : `regularisations` (toutes), `regularisation-mois` (l'etat d'un mois et ses
//! engagements), `regularisation-apercu`, `regularisation-ecrire`, `regularisation-annuler`.
//!
//! ECRIRE ou ANNULER une correction est un acte (mot de passe du logiciel, s'il est pose) : la route le demande ELLE-MEME
//! (`filet::exiger_mot_de_passe` -> `demande`), jamais la commande, jamais en option. Sans humain (ni ecran ni terminal), elle
//! refuse. `--fenetre` existe pour ne pas casser les habitudes : le moteur choisit seul son canal (fenetre s'il y a un ecran, sinon
//! le terminal), l'option ne change rien.
//!
//! `qui = "cli"` : la valeur que le Python ecrit dans la correction.
//!
//! DEFAUTS DU PYTHON, NE SE REPRODUISENT PAS : un mois mal ecrit donnait une trace d'appels ; ici, `! phrase`, code 2. Le message
//! d'un fichier d'engagement qui n'est pas du JSON est celui de l'analyseur Rust, pas celui de Python.

use serde_json::{json, Value};

use super::actes::{est_refus, mois_valide, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{brut, champ, enveloppe, rendre, texte};
use super::Cadre;
use socle::fichiers::vrai;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "corriger",
    positionnels: &[Pos::facultatif("mois", None)],
    options: &[
        Opt::valeur("extraire"),
        Opt::valeur("engagement"),
        Opt::valeur("salarie"),
        Opt::valeur("debut"),
        Opt::valeur("motif"),
        Opt::drapeau("apercu"),
        Opt::entier("annuler"),
        Opt::drapeau("fenetre"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) => reponse_metier("corriger", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some("ErreurOperation"), m)
}

fn opt<'a>(args: &'a Args, nom: &str) -> Option<&'a str> {
    args.texte(nom).filter(|t| !t.is_empty())
}

/// `_cle` : (salarie, debut) de l'engagement corrige.
fn cle(args: &Args, mois: &str) -> Result<(String, String), Echec> {
    let salarie = opt(args, "extraire").or(opt(args, "salarie"));
    match (salarie, opt(args, "debut")) {
        (Some(s), Some(d)) => Ok((s.to_string(), d.to_string())),
        _ => Err(refus(format!("Dire quel engagement : --salarie CLE --debut AAAA-MM-JJ (`paie corriger {}` les liste).", mois))),
    }
}

/// `_engagement` : l'engagement corrige, lu dans le fichier JSON.
fn engagement(args: &Args) -> Result<Value, Echec> {
    let nom = opt(args, "engagement").unwrap_or("");
    let chemin = std::path::Path::new(nom);
    if !chemin.exists() {
        return Err(refus(format!("{} n'existe pas.", nom)));
    }
    let contenu = std::fs::read_to_string(chemin).map_err(|e| refus(format!("{} ne se lit pas : {}", nom, e)))?;
    let d: Value = serde_json::from_str(&contenu).map_err(|e| refus(format!("{} ne se lit pas comme du JSON : {}", nom, e)))?;
    if !d.as_object().is_some_and(|o| o.contains_key("lignes")) {
        return Err(refus(format!("{} ne porte pas un engagement : un objet JSON avec ses `lignes`, comme dans la saisie du mois.", nom)));
    }
    Ok(d)
}

fn rendre_json(resultat: &Value) -> i32 {
    rendre(&enveloppe("corriger", brut(resultat)))
}

fn afficher_engagements(liste: &[Value]) {
    if liste.is_empty() {
        dire!("  (aucun engagement dans la saisie de ce mois)");
        return;
    }
    for e in liste {
        dire!(
            "  {:<30} {} -> {}  contrat {}  brut {}  net {}",
            format!("{} [{}]", champ(e, "nom"), champ(e, "salarie")),
            champ(e, "debut"),
            champ(e, "fin"),
            champ(e, "numero"),
            champ(e, "brut"),
            champ(e, "net_a_payer")
        );
        dire!("      {}", champ(e, "spectacle"));
        if e.get("deja_corrige").is_some_and(vrai) {
            dire!("      deja corrige par une DSN partie : l'engagement ci-dessus est celui de la derniere correction portee.");
        }
        if let Some(c) = e.get("correction_en_cours").filter(|c| vrai(c)) {
            dire!("      correction n° {} en attente : {}", champ(c, "numero"), champ(c, "motif"));
        }
    }
}

fn afficher_correction(c: &Value) {
    dire!("  n° {:<3} {:<12} {}, debut {}", champ(c, "numero"), champ(c, "etat"), champ(c, "salarie"), champ(c, "debut"));
    dire!("        {} (decidee le {} par {})", champ(c, "motif"), champ(c, "decidee_le"), champ(c, "qui"));
    if let Some(p) = c.get("portee_par").filter(|p| vrai(p)) {
        let quand = if p.get("quand").is_some() { champ(p, "quand") } else { "?".to_string() };
        dire!("        portee par la DSN de {}, deposee le {}", champ(p, "mois"), quand);
    } else if let Some(p) = c.get("preparee_par").filter(|p| vrai(p)) {
        let fichier = if p.get("fichier").is_some() { champ(p, "fichier") } else { "?".to_string() };
        dire!("        preparee par la DSN de {} ({}) : elle partira a son depot", champ(p, "mois"), fichier);
    }
    if c.get("annulee_le").is_some_and(vrai) {
        dire!("        annulee le {}", champ(c, "annulee_le"));
    }
}

fn liste_de(v: &Value, cle: &str) -> Vec<Value> {
    v.get(cle).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Sans mois : toutes les corrections, par mois.
fn toutes(cadre: &Cadre, json: bool) -> Result<i32, Echec> {
    let r = cadre.route("regularisations", json!({}))?;
    let par_mois = liste_de(&r, "corrections");
    let mois = r.get("mois").cloned().unwrap_or(Value::Null);
    if json {
        return Ok(rendre_json(&json!({"corrections": par_mois, "mois": mois})));
    }
    let en_retard = liste_de(&mois, "en_retard");
    let regularisables = liste_de(&mois, "regularisables");
    for e in &en_retard {
        dire!("! {} : aucune DSN acceptee, et l'echeance du {} est passee.", champ(e, "mois"), champ(e, "echeance"));
        dire!("    `paie mois paies/{}.json --forcer` -- en retard, assume.", champ(e, "mois"));
    }
    if !regularisables.is_empty() {
        dire!("Les mois qui se corrigent (deposes, echeance passee) :");
        for e in &regularisables {
            dire!("  {}   echeance du {}   `paie corriger {}`", champ(e, "mois"), champ(e, "echeance"), champ(e, "mois"));
        }
    } else if en_retard.is_empty() {
        dire!("Aucun mois depose n'a son echeance passee : rien ne se regularise.");
    }
    if par_mois.is_empty() {
        dire!("Aucune correction ecrite.");
        return Ok(0);
    }
    dire!();
    dire!("Les corrections ecrites :");
    for m in &par_mois {
        dire!("{} :", champ(m, "mois"));
        for c in liste_de(m, "corrections") {
            afficher_correction(&c);
        }
    }
    Ok(0)
}

/// L'etat d'un mois : `regularisation-mois` (etat, corrections ecrites, engagements declares).
fn etat_du_mois(cadre: &Cadre, mois: &str) -> Result<Value, Echec> {
    cadre.route("regularisation-mois", json!({"mois": mois}))
}

fn etat_texte(r: &Value, mois: &str, json: bool) -> i32 {
    let etat = r.get("etat").cloned().unwrap_or(Value::Null);
    let ecrites = liste_de(r, "corrections");
    let engagements = liste_de(r, "engagements");
    let regularisable = etat.get("regularisable").is_some_and(vrai);
    if json {
        let mut a = etat.as_object().cloned().unwrap_or_default();
        a.insert("engagements".into(), Value::Array(engagements));
        a.insert("corrections".into(), Value::Array(ecrites));
        return rendre_json(&Value::Object(a));
    }
    dire!("{}", champ(&etat, "raison"));
    if !ecrites.is_empty() {
        dire!("\nCorrections deja ecrites :");
        for c in &ecrites {
            afficher_correction(c);
        }
    }
    if regularisable {
        dire!("\nEngagements declares en {} :", mois);
        afficher_engagements(&engagements);
        dire!("\n  Pour corriger l'un d'eux :");
        dire!("    paie corriger {} --extraire CLE --debut AAAA-MM-JJ > corrige.json", mois);
        dire!("    (modifier les lignes dans corrige.json, puis)");
        dire!("    paie corriger {} --engagement corrige.json --salarie CLE --debut AAAA-MM-JJ --apercu", mois);
        dire!("    ... et la meme commande avec --motif au lieu de --apercu pour l'ecrire.");
    }
    0
}

fn extraire(cadre: &Cadre, args: &Args, mois: &str) -> Result<i32, Echec> {
    let (salarie, debut) = cle(args, mois)?;
    let r = etat_du_mois(cadre, mois)?;
    let etat = r.get("etat").cloned().unwrap_or(Value::Null);
    if !etat.get("regularisable").is_some_and(vrai) {
        return Err(Echec { classe: Some("MoisNonRegularisable"), message: champ(&etat, "raison") });
    }
    for e in liste_de(&r, "engagements") {
        if champ(&e, "salarie") == salarie && champ(&e, "debut") == debut {
            dire!("{}", socle::ecriture::json_py(&e["engagement"], 2));
            return Ok(0);
        }
    }
    Err(refus(format!("Aucun engagement de {} commencant le {} en {}. `paie corriger {}` les liste.", salarie, debut, mois, mois)))
}

fn apercu(cadre: &Cadre, args: &Args, mois: &str, engagement: Value) -> Result<i32, Echec> {
    let (salarie, debut) = cle(args, mois)?;
    let a = cadre.route("regularisation-apercu", json!({"mois": mois, "salarie": salarie, "debut": debut, "engagement": engagement}))?;
    if args.drapeau("json") {
        return Ok(rendre_json(&a));
    }
    let eng = a.get("engagement").cloned().unwrap_or(Value::Null);
    dire!("Correction de {}, contrat {} ({} -> {})", mois, champ(&a, "numero"), champ(&eng, "debut"), champ(&eng, "fin"));
    dire!("  {:<22} {:>12} {:>12} {:>12}", "", "avant", "apres", "difference");
    for (cle, titre) in [
        ("cachets", "cachets"),
        ("heures", "heures"),
        ("brut", "brut"),
        ("cotisations_salariales", "cot. salariales"),
        ("cotisations_patronales", "cot. patronales"),
        ("net_imposable", "net imposable"),
        ("net_a_payer", "net a payer"),
        ("cout_employeur", "cout employeur"),
    ] {
        let delta = match a.get("delta").and_then(|d| d.get(cle)) {
            Some(d) if !d.is_null() => texte(d),
            _ => String::new(),
        };
        dire!(
            "  {:<22} {:>12} {:>12} {:>12}",
            titre,
            a.get("avant").map(|x| champ(x, cle)).unwrap_or_default(),
            a.get("apres").map(|x| champ(x, cle)).unwrap_or_default(),
            delta
        );
    }
    if a.get("rien_ne_change").is_some_and(vrai) {
        dire!("\n  Rien ne change : il n'y a pas de correction a ecrire.");
    } else {
        dire!("\n  Cette difference partira dans la DSN du mois courant, datee de {}.", mois);
    }
    Ok(0)
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let Some(mois) = opt(args, "mois") else {
        return toutes(cadre, json);
    };
    if !mois_valide(mois) {
        return Err(refus(format!("Le mois « {} » ne se lit pas : AAAA-MM attendu (par exemple 2026-07).", mois)));
    }
    if let Some(n) = args.entier("annuler").filter(|n| *n != 0) {
        // Le mot de passe du logiciel : la route le demande elle-meme, a un humain.
        let r = cadre.route("regularisation-annuler", json!({"mois": mois, "numero": n, "qui": "cli"}))?;
        let c = r.get("correction").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre_json(&c));
        }
        dire!("Correction n° {} de {} annulee : la prochaine DSN ne l'embarque plus.", champ(&c, "numero"), mois);
        return Ok(0);
    }
    if opt(args, "extraire").is_some() {
        return extraire(cadre, args, mois);
    }
    if opt(args, "engagement").is_none() {
        let r = etat_du_mois(cadre, mois)?;
        return Ok(etat_texte(&r, mois, json));
    }
    let eng = engagement(args)?;
    if args.drapeau("apercu") || opt(args, "motif").is_none() {
        if !(args.drapeau("apercu") || json) {
            dire!("(apercu : sans --motif, rien n'est ecrit)\n");
        }
        return apercu(cadre, args, mois, eng);
    }
    let (salarie, debut) = cle(args, mois)?;
    let motif = opt(args, "motif").unwrap_or("");
    let r = cadre.route(
        "regularisation-ecrire",
        json!({"mois": mois, "salarie": salarie, "debut": debut, "engagement": eng, "motif": motif, "qui": "cli"}),
    )?;
    let c = r.get("correction").cloned().unwrap_or(Value::Null);
    if json {
        return Ok(rendre_json(&c));
    }
    dire!("Correction n° {} ecrite dans regularisations/{}.json.", champ(&c, "numero"), mois);
    dire!("  {}", champ(&c, "motif"));
    dire!("  La saisie de {} n'a pas bouge : la prochaine DSN du mois courant portera cette correction, datee de {}.", mois, mois);
    Ok(0)
}
