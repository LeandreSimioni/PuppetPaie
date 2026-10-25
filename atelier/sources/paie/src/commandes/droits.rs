//! `paie droits [--detail] [--mois AAAA-MM] [--json]` : heures et cachets cumules, annexes 8 et 10 (`moteur/cli.py`,
//! `cmd_droits`). Le calcul est celui du moteur (`service::droits`, que le Python appelait par `paie_rust.droits`) ;
//! cette commande lit la position de chaque salarie et la met en forme.

use rust_decimal::RoundingStrategy;
use serde_json::{json, Map, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{debut, enveloppe, exiger, rendre};
use super::Cadre;
use socle::num::Decimal;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "droits",
    positionnels: &[],
    options: &[Opt::drapeau("detail"), Opt::valeur("mois"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(code) => code,
        Err(e) => cadre.plantage(e),
    }
}

fn refus(m: String) -> Echec {
    Echec { classe: None, message: m }
}

/// `int(v)` pour un nombre que le moteur rend en chaine (`moteur/operations_paies._valeur_droits`).
fn entier(v: &Value, cle: &str) -> i64 {
    match v.get(cle) {
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => 0,
    }
}

fn texte(v: &Value, cle: &str) -> String {
    v.get(cle).map(super::rendu::texte).unwrap_or_default()
}

fn vrai(v: &Value, cle: &str) -> bool {
    v.get(cle).is_some_and(socle::fichiers::vrai)
}

/// `Decimal(v).quantize(Decimal("0.01"))`, au pair le plus proche comme le contexte de Python.
fn deux_decimales(v: &str) -> String {
    match v.trim().parse::<Decimal>() {
        Ok(d) => {
            let mut r = d.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
            r.rescale(2);
            r.to_string()
        }
        Err(_) => v.to_string(),
    }
}

/// `_valeur_droits` : ce que la position du moteur dit, lu comme le Python le lisait (nombres entiers, `brut` a deux
/// decimales, textes et dates tels quels).
fn convertir(cle: &str, v: &Value) -> Value {
    const TEXTES: [&str; 6] = ["titre", "sous_titre", "libelle_fenetre", "salarie", "numero_objet", "mois"];
    const DATES: [&str; 4] = ["debut", "fin", "date_naissance", "echeance"];
    match v {
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => v.clone(),
        _ if TEXTES.contains(&cle) || DATES.contains(&cle) => v.clone(),
        Value::String(s) if cle == "brut" => Value::String(deux_decimales(s)),
        Value::String(s) => s.trim().parse::<i64>().map(Value::from).unwrap_or_else(|_| v.clone()),
        autre => autre.clone(),
    }
}

fn convertir_objet(o: &Value, sans: &[&str]) -> Map<String, Value> {
    o.as_object().map(|m| m.iter().filter(|(k, _)| !sans.contains(&k.as_str())).map(|(k, v)| (k.clone(), convertir(k, v))).collect()).unwrap_or_default()
}

/// `sortie_json.position_droits`.
fn position_json(cle: &str, salarie: &Value, x: &Value, p: &Value) -> Value {
    let prenom = match salarie.get("prenom_affichage") {
        Some(v) if socle::fichiers::vrai(v) => super::rendu::texte(v),
        _ => salarie.get("prenom").map(super::rendu::texte).unwrap_or_default(),
    };
    let colonnes: Vec<Value> = x.get("colonnes").and_then(Value::as_array).map(|a| a.iter().map(|c| Value::Object(convertir_objet(c, &[]))).collect()).unwrap_or_default();
    let c = |k: &str| convertir(k, p.get(k).unwrap_or(&Value::Null));
    json!({
        "salarie": {"cle": cle, "nom": salarie.get("nom").cloned().unwrap_or(Value::Null), "prenom": prenom},
        "fenetre": {
            "libelle": c("libelle_fenetre"), "debut": c("debut"), "fin": c("fin"),
            "anniversaire_configure": x.get("anniversaire").cloned().unwrap_or(Value::Null),
            "jours_restants": c("jours_restants"),
        },
        "cachets": c("cachets"), "heures": c("heures"),
        // Le plafond des 28 cachets par mois (annexe X, art. 3) : ce qui a ete declare, et ce que les annexes en retiennent.
        "cachets_retenus": c("cachets_retenus"), "cachets_perdus": c("cachets_perdus"),
        "mois_plafonnes": c("mois_plafonnes"), "heures_equivalentes": c("heures_equivalentes"),
        // Deux totaux, parce qu'ils repondent a deux questions : ai-je mes 507 heures, et sur combien d'heures l'allocation
        // sera-t-elle calculee.
        "heures_enseignement": c("heures_enseignement"), "plafond_enseignement": c("plafond_enseignement"),
        "heures_enseignement_retenues": c("heures_enseignement_retenues"), "heures_ouverture": c("heures_ouverture"),
        "heures_montant": c("heures_montant"),
        "seuil_atteint": c("atteint"), "manque": c("manque"),
        "cachets_manquants": c("cachets_manquants"),
        "colonnes": colonnes,
    })
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let mois = match args.texte("mois") {
        Some(m) => m.to_string(),
        None => format!("{:04}-{:02}", cadre.aujourdhui.an, cadre.aujourdhui.mois),
    };
    let demande = json!({"mois": mois, "aujourdhui": cadre.aujourdhui.iso()}).to_string();
    let rep = crate::service::droits(&cadre.o, &demande).map_err(refus)?;
    let constantes = rep.get("constantes").cloned().unwrap_or(Value::Null);
    let positions: Vec<Value> = rep.get("positions").and_then(Value::as_array).cloned().unwrap_or_default();
    // Chaque position avec la fiche du salarie : `ctx.salaries[x["cle"]]`.
    let mut lues: Vec<(String, Value, Value)> = Vec::new();
    for x in &positions {
        let cle = texte(x, "cle");
        let fiche = cadre.ctx.salaries.get(&cle).cloned().ok_or(Echec { classe: Some("KeyError"), message: socle::fichiers::repr(&cle) })?;
        lues.push((cle, fiche, x.clone()));
    }

    if args.drapeau("json") {
        let salaries: Vec<Value> = lues
            .iter()
            .map(|(cle, fiche, x)| {
                let p = Value::Object(convertir_objet(&x["position"], &["engagements"]));
                position_json(cle, fiche, x, &p)
            })
            .collect();
        return Ok(rendre(&enveloppe("droits", json!({"mois": mois, "salaries": salaries}))));
    }
    if lues.is_empty() {
        dire!("Aucun contrat connu : ni historique importe (`paie route importer`, ou la fenetre PuppetPaie), ni mois produit.");
        return Ok(1);
    }

    let k = |cle: &str| entier(&constantes, cle);
    for (cle, s, x) in &lues {
        let _ = cle;
        let p = Value::Object(convertir_objet(&x["position"], &["engagements"]));
        let n = |c: &str| entier(&p, c);
        dire!();
        dire!("{} {} - {}", exiger(s, "nom")?, exiger(s, "prenom")?, texte(&p, "libelle_fenetre"));
        dire!("{}", "-".repeat(62));
        dire!("  {:<34} {:>6}", "cachets declares", n("cachets"));
        if n("cachets_perdus") != 0 {
            // LE PLAFOND DES 28 CACHETS PAR MOIS a mordu (annexe X, art. 3). Le taire laisserait lire « 34 cachets » et
            // compter 408 h dans sa tete, quand 336 comptent.
            dire!("  {:<34} {:>6}", format!("  retenus ({} par mois au plus)", k("cachets_par_mois")), n("cachets_retenus"));
        }
        dire!("  {:<34} {:>6} h", format!("  convertis ({} h par cachet)", 12), n("cachets_retenus") * k("heures_par_cachet"));
        dire!("  {:<34} {:>6} h", "heures declarees", n("heures"));
        // SUR LE TOTAL RETENU, pas sur le declare : tout ce qui suit -- le seuil, la marge, ce qui manque -- se calcule apres
        // le plafond des 28 cachets.
        dire!("  {:<34} {:>6} h", "TOTAL heures equivalentes", n("heures_retenues"));
        if n("heures_enseignement") != 0 {
            // Deux decomptes des qu'il y a de l'enseignement : ces heures ouvrent les droits (plafonnees) mais n'augmentent pas l'ARE.
            dire!("  {:<34} {:>6} h", "  dont enseignement", n("heures_enseignement"));
            dire!("  {:<34} {:>6} h", format!("  retenues (plafond {} h)", n("plafond_enseignement")), n("heures_enseignement_retenues"));
            dire!("  {:<34} {:>6} h", "POUR L'OUVERTURE des droits", n("heures_ouverture"));
            dire!("  {:<34} {:>6} h", "POUR LE MONTANT de l'ARE", n("heures_montant"));
        }
        dire!("  {:<34} {:>6} h", "seuil annexes 8 et 10", k("seuil_heures"));
        if vrai(&p, "atteint") {
            dire!("  => seuil atteint, avec {} h de marge.", n("heures_ouverture") - k("seuil_heures"));
        } else {
            dire!("  => il manque {} h, soit environ {} cachets.", n("manque"), n("cachets_manquants"));
        }
        let anniversaire = x.get("anniversaire").is_some_and(socle::fichiers::vrai);
        if anniversaire && n("jours_restants") > 0 {
            dire!("  {:<34} {:>6} j", "avant le reexamen", n("jours_restants"));
        }
        if n("cachets_perdus") != 0 {
            let plafonnes: Vec<String> = p.get("mois_plafonnes").and_then(Value::as_array).map(|a| a.iter().map(super::rendu::texte).collect()).unwrap_or_default();
            dire!("  ({} cachet(s) depassent les {} d'un mois ({}) :", n("cachets_perdus"), k("cachets_par_mois"), plafonnes.join(", "));
            dire!("   joues, payes et declares, mais sans effet sur les droits.)");
        }
        if n("enseignement_perdu") != 0 {
            dire!("  ({} h d'enseignement depassent le plafond : payees et", n("enseignement_perdu"));
            dire!("   declarees, mais sans effet sur les droits.)");
        }
        if n("heures_enseignement") != 0 && p.get("date_naissance").map_or(true, |v| !socle::fichiers::vrai(v)) {
            dire!("  (date de naissance absente de config/salaries.json : le");
            dire!("   plafond bas est applique, ce qui sous-estime peut-etre.)");
        }
        if !anniversaire {
            dire!("  (fenetre deduite du dernier contrat declare. Renseigner");
            dire!("   'anniversaire_droits' dans config/salaries.json des que");
            dire!("   France Travail l'a notifiee : elle prime sur ce calcul.)");
        }
        let cols: Vec<Value> = x.get("colonnes").and_then(Value::as_array).map(|a| a.iter().map(|c| Value::Object(convertir_objet(c, &[]))).collect()).unwrap_or_default();
        dire!();
        dire!("  {:<22}{}", "", cols.iter().map(|c| format!("{:>14}", texte(c, "titre"))).collect::<String>());
        dire!("  {:<22}{}", "", cols.iter().map(|c| format!("{:>14}", texte(c, "sous_titre"))).collect::<String>());
        for (lib, attr, suf) in [("Cachets", "cachets", ""), ("Heures declarees", "heures", " h"), ("TOTAL heures", "heures_equivalentes", " h"), ("Salaire brut", "brut", "")] {
            let vals: String = cols.iter().map(|c| format!("{:>14}", format!("{}{}", texte(c, attr), suf))).collect();
            dire!("  {:<22}{}", lib, vals);
        }
        if args.drapeau("detail") {
            dire!();
            let mut engs: Vec<&Value> = x["position"].get("engagements").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
            engs.sort_by_key(|e| texte(e, "fin"));
            for e in engs {
                let jj_mm_aa = |iso: String| -> String {
                    // `strftime("%d/%m/%y")`
                    let c: Vec<char> = iso.chars().collect();
                    if c.len() >= 10 {
                        format!("{}{}/{}{}/{}{}", c[8], c[9], c[5], c[6], c[2], c[3])
                    } else {
                        iso
                    }
                };
                dire!(
                    "     {} -> {}  obj {:<12} h={:<3} cachets={:<2} = {:>3} h",
                    jj_mm_aa(texte(e, "debut")),
                    jj_mm_aa(texte(e, "fin")),
                    texte(e, "numero_objet"),
                    entier(e, "heures"),
                    entier(e, "cachets"),
                    entier(e, "heures_equivalentes")
                );
            }
        }
    }
    dire!("\nCe compte vaut ce que valent les AEM deposees : il ne remplace pas le releve de");
    dire!("France Travail, qui seul fait foi et qui peut retenir un autre contrat de");
    dire!("reference. Il sert a voir venir, pas a decider a la place de l'organisme.");
    dire!("\nEt ce ne sont QUE les AEM de la compagnie. Le seuil se compte sur tous les");
    dire!("employeurs : a chacun d'ajouter les siens. Ces totaux sont un plancher.");
    let _ = debut;
    Ok(0)
}
