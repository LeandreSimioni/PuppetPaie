//! `moteur pas [--amorcer]` : les taux de prelevement a la source connus (`moteur/commandes_retours.py`, `cmd_pas`), et
//! l'ecriture du magasin `config/pas.json` que `moteur pas --amorcer` et `moteur retours --appliquer` font
//! (`moteur/pas.py` : `enregistrer`, `releves_historiques`, `amorcer`, `ecrire`).
//!
//! `--demander` (ecrire les signalements d'amorcage, nature 08 ; rien n'est depose) : `demander`, sur `amorcage_demande`.

use serde_json::{json, Map, Value};

use super::analyse::{Args, Opt, Spec};
use super::rendu::{champ, jj_mm_aaaa_de_iso};
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

/// `pas.TYPES` : cahier technique 2026.1, rubrique S21.G00.50.007.
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

fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

fn maintenant_court() -> String {
    // `datetime.now().strftime("%Y-%m-%d %H:%M")`, a l'heure de Paris.
    socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].replace('T', " ")
}

fn fr(iso: Option<&str>) -> String {
    match iso {
        Some(i) if !i.is_empty() => format!("{}/{}/{}", &i[8..10], &i[5..7], &i[..4]),
        _ => "?".into(),
    }
}

/// `pas.ecrire` : `json.dumps(data, ensure_ascii=False, indent=2)`, sans saut de ligne final.
fn ecrire(ctx: &Contexte, data: &Value) -> Result<(), Echec> {
    let p = ctx.racine.join("config").join("pas.json");
    std::fs::write(&p, socle::ecriture::json_indente(data, 2)).map_err(|e| Echec { classe: Some("OSError"), message: format!("{} : {}", p.display(), e) })
}

/// `pas.enregistrer` : applique des releves de CRM ; rend les changements lisibles. Un releve :
/// `{"salarie", "taux", "type", "identifiant", "periode", "mis_a_disposition"}`.
pub fn enregistrer(ctx: &Contexte, releves: &[Value], source: &str) -> Result<Vec<String>, Echec> {
    let mut data = droit::pas::charger(ctx).map_err(ko)?;
    let mut changements = Vec::new();
    let mut soldees = false;
    let horodatage = maintenant_court();
    for r in releves {
        let cle = r.get("salarie").and_then(Value::as_str).unwrap_or("").to_string();
        // LA REPONSE EST LA : la demande qui l'attendait n'a plus lieu d'etre. Elle se solde AVANT le test d'egalite.
        let demandes = data["demandes"].as_object_mut().ok_or_else(|| ko("demandes illisibles".into()))?;
        soldees = demandes.shift_remove(&cle).is_some() || soldees;
        let avant = data["salaries"].get(&cle).cloned();
        let get = |k: &str| r.get(k).cloned().unwrap_or(Value::Null);
        let neuf = json!({
            "taux": get("taux"), "type": get("type"), "identifiant": get("identifiant"),
            "periode": get("periode"), "mis_a_disposition": get("mis_a_disposition"),
            "source": source, "releve_le": horodatage,
        });
        if let Some(a) = avant.as_ref().filter(|a| socle::fichiers::vrai(a)) {
            if ["taux", "type", "identifiant"].iter().all(|k| a.get(*k) == neuf.get(*k)) {
                continue;
            }
        }
        let t = |v: &Value, k: &str| super::rendu::champ(v, k);
        data["salaries"].as_object_mut().ok_or_else(|| ko("salaries illisibles".into()))?.insert(cle.clone(), neuf.clone());
        match avant.filter(|a| socle::fichiers::vrai(a)) {
            Some(a) => changements.push(format!(
                "{} : taux {} -> {}, type {} -> {}, identifiant {} -> {}",
                cle, t(&a, "taux"), t(&neuf, "taux"), t(&a, "type"), t(&neuf, "type"), t(&a, "identifiant"), t(&neuf, "identifiant")
            )),
            None => changements.push(format!("{} : premier releve - taux {}, type {}, identifiant {}", cle, t(&neuf, "taux"), t(&neuf, "type"), t(&neuf, "identifiant"))),
        }
        let mut ligne = neuf.as_object().cloned().unwrap_or_default();
        ligne.insert("salarie".into(), json!(cle));
        data["journal"].as_array_mut().ok_or_else(|| ko("journal illisible".into()))?.push(Value::Object(ligne));
    }
    if !changements.is_empty() || soldees {
        ecrire(ctx, &data)?;
    }
    Ok(changements)
}

/// `str.splitlines()` de Python.
fn lignes_py(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut c = t.chars().peekable();
    while let Some(x) = c.next() {
        match x {
            '\r' => {
                if c.peek() == Some(&'\n') {
                    c.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push(std::mem::take(&mut cur)),
            x => cur.push(x),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `pas.releves_historiques` : par salarie, l'historique des taux declares dans les DSN deja deposees (une lecture, pas un calcul).
fn releves_historiques(ctx: &Contexte, dossier: &std::path::Path) -> Vec<(String, Vec<Value>)> {
    let par_nir: std::collections::HashMap<String, String> = ctx
        .salaries
        .iter()
        .filter_map(|(cle, s)| s.get("nir").filter(|n| socle::fichiers::vrai(n)).map(|n| (super::rendu::texte(n), cle.clone())))
        .collect();
    let mut fichiers = Vec::new();
    crate::routes_fiches::fichiers_dsn(dossier, &mut fichiers);
    fichiers.sort();
    // salarie -> mois -> releve (dans l'ordre d'insertion des salaries, mois tries).
    let mut trouve: Vec<(String, std::collections::BTreeMap<String, Value>)> = Vec::new();
    for f in fichiers {
        let Ok(octets) = std::fs::read(&f) else { continue };
        let texte: String = octets.iter().map(|b| *b as char).collect();
        let (mut mois, mut nir): (Option<String>, Option<String>) = (None, None);
        let mut cur: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for ligne in lignes_py(&texte) {
            if !ligne.contains(',') {
                continue;
            }
            let Some((code, val)) = ligne.trim().split_once(',') else { continue };
            let val = val.trim().trim_matches('\'').to_string();
            if code == "S20.G00.05.005" {
                let c: Vec<char> = val.chars().collect();
                let tr = |a: usize, b: usize| -> String { c.iter().skip(a).take(b.saturating_sub(a)).collect() };
                mois = Some(format!("{}-{}", tr(4, 8), tr(2, 4)));
            } else if code == "S21.G00.30.001" {
                nir = Some(val);
            } else if code.starts_with("S21.G00.50.00") {
                cur.insert(code.to_string(), val.clone());
                if code == "S21.G00.50.008" {
                    if let (Some(cle), Some(m)) = (nir.as_ref().and_then(|n| par_nir.get(n)), mois.as_ref()) {
                        let g = |k: &str| cur.get(k).map_or(Value::Null, |v| json!(v));
                        let rel = json!({"taux": g("S21.G00.50.006"), "type": g("S21.G00.50.007"), "identifiant": val, "periode": m});
                        match trouve.iter_mut().find(|(k, _)| k == cle) {
                            Some((_, mm)) => {
                                mm.insert(m.clone(), rel);
                            }
                            None => trouve.push((cle.clone(), [(m.clone(), rel)].into_iter().collect())),
                        }
                    }
                    cur.clear();
                }
            }
        }
    }
    trouve.into_iter().map(|(cle, mm)| (cle, mm.into_values().collect())).collect()
}

/// `pas.amorcer` : pose l'etat courant a partir de la derniere DSN deposee de chaque salarie.
fn amorcer(ctx: &Contexte, dossier: &std::path::Path) -> Result<Vec<String>, Echec> {
    let mut releves = Vec::new();
    for (cle, entrees) in releves_historiques(ctx, dossier) {
        if let Some(dernier) = entrees.last() {
            let mut r = dernier.as_object().cloned().unwrap_or_default();
            r.insert("salarie".into(), json!(cle));
            releves.push(Value::Object(r));
        }
    }
    let nom = dossier.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    enregistrer(ctx, &releves, &format!("amorcage depuis les DSN deposees ({})", nom))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    if args.drapeau("amorcer") {
        let export = crate::routes_fiches::declarations_coolpaie(ctx).join("DSN");
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
            Some(Value::String(j)) => format!(", jusqu'au {}", jj_mm_aaaa_de_iso(j)),
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

/// Pour `moteur retours --appliquer` : `pas.enregistrer`.
pub fn appliquer(ctx: &Contexte, releves: &[Value], source: &str) -> Result<Vec<String>, Echec> {
    enregistrer(ctx, releves, source)
}

/// `pas --demander` (`_demander_les_taux`) : ecrire les signalements d'amorcage (nature 08). RIEN N'EST DEPOSE ici : le depot est un
/// acte a part (`moteur depot-dsn`), avec sa preuve et son mot de passe -- et c'est lui qui fait courir les cinq jours ouvres, donc lui
/// qui note la demande. Un refus de la norme ou de la demande est une phrase (code 1), jamais une trace.
fn demander(cadre: &Cadre, args: &Args) -> i32 {
    use engagement::embauches_du_mois as paies_du_mois;
    use crate::amorcage_demande::{produire, Demande};
    let ctx = &cadre.ctx;
    let motif = args.texte("motif").unwrap_or("07");
    // Le mois : celui qu'on dit, sinon le dernier saisi (c'est le contrat que la derniere DSN a declare, et ce que le motif 07 reprend).
    let mois = match args.texte("mois") {
        Some(m) => {
            if !super::actes::mois_valide(m) {
                dire!("--mois attend AAAA-MM, recu {}.", socle::fichiers::repr(m));
                return 1;
            }
            m.to_string()
        }
        None => match crate::routes_mois::mois_saisis(&ctx.racine).into_iter().filter(|m| super::actes::mois_valide(m)).last() {
            Some(m) => m,
            None => {
                dire!("Aucun mois saisi : un signalement d'amorcage declare un CONTRAT (bloc 40, cardinalite (1,1)), et il n'y en a aucun a declarer.");
                return 1;
            }
        },
    };
    let paies = match paies_du_mois(ctx, &mois) {
        Ok(Some(p)) => p,
        Ok(None) => {
            dire!("Aucun engagement ne touche {}. Rien n'est ecrit.", mois);
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
        "\nUN SIGNALEMENT, UN INDIVIDU : chaque fichier se depose separement.\n    moteur depot-dsn {} --test\npuis, une fois le bilan lu, avec --reel. La DGFiP repond sous cinq jours\nouvres, par un CRM de nature 94 :\n    moteur retours --appliquer\n",
        premier
    );
    if args.drapeau("test") {
        dire!("Ces fichiers sont des ESSAIS (S10.G00.00.005 = 01) : ils ne demandent rien.");
    }
    0
}
