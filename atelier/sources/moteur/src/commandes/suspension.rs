//! `moteur suspension [liste|motifs|saisir|modifier|oublier] [SALARIE] [options] [--json]` : les suspensions du contrat
//! (bloc S21.G00.65 : conge parental, sabbatique, sans solde...), saisies UNE fois comme imprevus du salarie. RIEN NE PART
//! D'ICI : la production de chaque mois traverse en tire la ligne de paie d'un permanent et le bloc 65 de sa DSN.
//!
//! Chaque verbe passe par une route du moteur : `imprevus` (la liste et les motifs), `suspension-enregistrer` (`saisir`),
//! `suspension-modifier` (`modifier`), `suspension-oublier` (`oublier`).

use serde_json::{json, Value};

use super::actes::{est_refus, jour_option, reponse_metier};
use socle::date::jj_mm_aaaa;
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{brut, champ, enveloppe, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "suspension",
    positionnels: &[
        Pos::parmi("quoi", &["liste", "motifs", "saisir", "modifier", "oublier"], Some("liste")),
        Pos::facultatif("salarie", None),
    ],
    options: &[
        Opt::valeur("debut"),
        Opt::valeur("fin"),
        Opt::valeur("motif"),
        Opt::valeur("contrat"),
        Opt::valeur("note"),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) => reponse_metier("suspension", args.drapeau("json"), &e, &[]),
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

fn exiger(args: &Args, verbe: &str, noms: &[&str]) -> Result<String, Echec> {
    let Some(salarie) = args.texte("salarie").filter(|t| !t.is_empty()) else {
        return Err(refus(format!("Dire quel salarie : `moteur suspension {} CLE ...` (la cle est celle de la fiche).", verbe)));
    };
    let manque: Vec<String> = noms.iter().filter(|n| opt(args, n).is_none()).map(|n| format!("--{}", n)).collect();
    if !manque.is_empty() {
        return Err(refus(format!("Il manque {} pour `moteur suspension {}`.", manque.join(", "), verbe)));
    }
    Ok(salarie.to_string())
}

fn date(args: &Args, nom: &str) -> Result<Option<String>, Echec> {
    match opt(args, nom) {
        Some(t) => Ok(Some(jour_option(nom, t)?.iso())),
        None => Ok(None),
    }
}

fn ligne(s: &Value) {
    dire!(
        "  {:<10} {} {}  {} -> {}   contrat {}",
        champ(s, "salarie"),
        champ(s, "motif"),
        champ(s, "motif_libelle"),
        jj_mm_aaaa(&champ(s, "debut")),
        jj_mm_aaaa(&champ(s, "fin")),
        champ(s, "contrat")
    );
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let quoi = args.texte("quoi").unwrap_or("liste");
    let charge_du_salarie = |verbe: &str, noms: &[&str]| -> Result<Value, Echec> {
        let salarie = exiger(args, verbe, noms)?;
        let mut c = json!({"salarie": salarie, "qui": "ligne de commande"});
        for k in ["debut", "fin"] {
            if let Some(d) = date(args, k)? {
                c[k] = json!(d);
            }
        }
        for k in ["motif", "contrat", "note"] {
            if let Some(v) = opt(args, k) {
                c[k] = json!(v);
            }
        }
        Ok(c)
    };
    let route = match quoi {
        "saisir" => Some(("suspension-enregistrer", charge_du_salarie("saisir", &["debut", "fin", "motif"])?, "Suspension enregistree.")),
        "modifier" => Some(("suspension-modifier", charge_du_salarie("modifier", &["debut"])?, "Suspension modifiee.")),
        "oublier" => Some(("suspension-oublier", charge_du_salarie("oublier", &["debut"])?, "Suspension retiree du magasin.")),
        _ => None,
    };
    if let Some((nom, charge, phrase)) = route {
        let r = cadre.route(nom, charge)?;
        let s = r.get("suspension").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre(&enveloppe("suspension", brut(&s))));
        }
        dire!("{} Rien n'est parti : chaque mois traverse la declare (bloc 65).", phrase);
        ligne(&s);
        return Ok(0);
    }
    let r = cadre.route("imprevus", json!({}))?;
    if quoi == "motifs" {
        let motifs = r.get("motifs_suspension").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre(&enveloppe("suspension", json!({"motifs": motifs}))));
        }
        dire!("Motifs de suspension (S21.G00.65.001) :");
        for m in motifs.as_array().map_or(&[][..], |a| &a[..]) {
            let paye = if m.get("remunere") == Some(&json!(true)) { "   (remunere : rien n'est retire du salaire)" } else { "" };
            dire!("  {}  {}{}", champ(m, "code"), champ(m, "libelle"), paye);
        }
        return Ok(0);
    }
    let toutes: Vec<Value> = r
        .get("imprevus")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |a| a.iter().filter(|i| i.get("genre") == Some(&json!("suspension"))).cloned().collect());
    if json {
        return Ok(rendre(&enveloppe("suspension", json!({"suspensions": brut(&json!(toutes))}))));
    }
    if toutes.is_empty() {
        dire!("Aucune suspension saisie. `moteur suspension saisir CLE --debut AAAA-MM-JJ --fin AAAA-MM-JJ --motif 632` en saisit une ; `moteur suspension motifs` les liste.");
        return Ok(0);
    }
    dire!("Suspensions ({}) :", toutes.len());
    for s in &toutes {
        ligne(s);
    }
    Ok(0)
}
