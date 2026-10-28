//! `paie arret [liste|motifs|contrats|saisir|reprise|prolonger|maintien|signaler|oublier|connu|declare] [SALARIE] [options] [--json]` : les arrets de travail, de la
//! saisie au signalement (`moteur/commandes_arrets.py`). RIEN NE PART D'ICI : `signaler` ECRIT un fichier, c'est `paie depot-dsn`
//! qui le depose.
//!
//! Chaque verbe passe par une route du moteur : `arrets` (la liste), `arret-contrats`, `arret-enregistrer` (`saisir`),
//! `arret-reprise`, `arret-prolonger` (`prolonger` : l'avis de prolongation, `--fin`, et la fin de subrogation, `--subrogation-fin`),
//! `arret-maintien` (`maintien` : le complement de salaire d'un mois, les jours deja indemnises), `arret-signaler`,
//! `arret-oublier`. Les arrets vivent dans le magasin des imprevus (`imprevus`). `connu` et `declare` (15/10/2026) : `arret-connaissance`
//! (le jour ou la compagnie a su l'arret ou l'accident), `accident-declare` (la declaration d'accident du travail faite, hors DSN) et,
//! avec `--attestation`, `attestation-declare` (l'attestation de salaire faite hors DSN, pour un deuil d'enfant) ; les
//! obligations de chaque arret, leur delai et leur etat, sont celles que la route `arrets` rend (`obligations.rs`). Les motifs et le « oui/non » de la subrogation se lisent dans
//! `arrets-base.json`.
//!
//! `saisir` et `reprise` inscrivent `qui = "ligne de commande"` (la charge de la route le porte), comme le Python.
//!
//! DEFAUTS DU PYTHON, NE SE REPRODUISENT PAS : un verbe auquel il manque le salarie, un jour, une date ou un motif donnait une trace
//! d'appels (`None` jete dans la recherche) ; une date illisible aussi. Ici : `! phrase`, code 2. Le Python ignore `--json` pour
//! `oublier` : le texte seul, ici aussi.

use serde_json::{json, Value};

use super::actes::{est_refus, jj_mm_aaaa, jour_option, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{brut, champ, enveloppe, rendre, texte};
use super::Cadre;
use socle::fichiers::vrai;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "arret",
    positionnels: &[
        Pos::parmi("quoi", &["liste", "motifs", "contrats", "saisir", "reprise", "prolonger", "maintien", "signaler", "oublier", "connu", "declare"], Some("liste")),
        Pos::facultatif("salarie", None),
    ],
    options: &[
        Opt::valeur("dernier-jour"),
        Opt::valeur("fin"),
        Opt::valeur("motif"),
        Opt::valeur("subrogation"),
        Opt::valeur("subrogation-debut"),
        Opt::valeur("subrogation-fin"),
        Opt::valeur("accident"),
        Opt::valeur("contrat"),
        Opt::valeur("jours-deja-indemnises"),
        Opt::valeur("mois"),
        Opt::valeur("montant"),
        Opt::valeur("note"),
        Opt::valeur("le"),
        Opt::avec_defaut("motif-reprise", "01"),
        Opt::drapeau("reprise"),
        Opt::valeur("remplace"),
        Opt::entier("ordre"),
        Opt::drapeau("test"),
        Opt::drapeau("json"),
        Opt::valeur("de"),
        Opt::valeur("heure"),
        Opt::valeur("ou"),
        Opt::valeur("reference"),
        Opt::drapeau("retirer"),
        Opt::drapeau("attestation"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus(e.classe) => reponse_metier("arret", args.drapeau("json"), &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some("ErreurOperation"), m)
}

/// Une option texte, vide ou absente = absente (`if args.x`).
fn opt<'a>(args: &'a Args, nom: &str) -> Option<&'a str> {
    args.texte(nom).filter(|t| !t.is_empty())
}

/// Les arguments sans lesquels le verbe ne peut rien : le salarie, puis les options nommees.
fn exiger(args: &Args, verbe: &str, noms: &[&str]) -> Result<String, Echec> {
    if opt_pos(args).is_none() {
        return Err(refus(format!("Dire quel salarie : `paie arret {} CLE ...` (`paie arret` liste les arrets, la cle est celle de la fiche).", verbe)));
    }
    let manque: Vec<String> = noms.iter().filter(|n| opt(args, n).is_none()).map(|n| format!("--{}", n)).collect();
    if !manque.is_empty() {
        return Err(refus(format!("Il manque {} pour `paie arret {}`.", manque.join(", "), verbe)));
    }
    Ok(opt_pos(args).unwrap_or_default().to_string())
}

fn opt_pos(args: &Args) -> Option<&str> {
    args.texte("salarie").filter(|t| !t.is_empty())
}

/// Une date d'option, lue avant de la confier a la route (une date illisible est un refus d'une phrase).
fn date(args: &Args, nom: &str) -> Result<Option<String>, Echec> {
    match opt(args, nom) {
        Some(t) => Ok(Some(jour_option(nom, t)?.iso())),
        None => Ok(None),
    }
}

/// `_oui_non` : « oui »/« non » -> le code de la norme, lu dans `arrets-base.json`.
fn oui_non(cadre: &Cadre, v: &str) -> Result<Value, Echec> {
    let n = crate::routes_listes::nomenclatures(&cadre.ctx)?;
    let codes: Vec<Value> = n.get("subrogation").and_then(|s| s.get("codes")).and_then(Value::as_array).cloned().unwrap_or_default();
    let table: Vec<(String, String)> = codes.iter().map(|c| (champ(c, "libelle"), champ(c, "code"))).collect();
    let v = v.trim_matches(socle::texte::blanc).to_lowercase();
    if let Some((_, code)) = table.iter().find(|(lib, _)| *lib == v) {
        return Ok(json!(code));
    }
    if table.iter().any(|(_, code)| *code == v) {
        return Ok(json!(v));
    }
    let liste: Vec<String> = table.iter().map(|(lib, code)| format!("{} = {}", lib, code)).collect();
    Err(refus(format!("--subrogation : dire « oui » ou « non » ({}).", liste.join(", "))))
}

/// `_lignes_arret` : un arret en trois ou quatre lignes.
fn lignes_arret(r: &Value) {
    let vrai_de = |k: &str| r.get(k).is_some_and(vrai);
    let fin = if vrai_de("reprise") { champ(r, "reprise") } else { champ(r, "fin_previsionnelle") };
    dire!(
        "  {:<10} {}  {} -> {}",
        champ(r, "salarie"),
        champ(r, "motif_libelle"),
        jj_mm_aaaa(&champ(r, "dernier_jour_travaille")),
        jj_mm_aaaa(&fin)
    );
    let mut detail = vec![format!("contrat {}", champ(r, "contrat"))];
    if vrai_de("reprise") {
        detail.push(format!("reprise le {}", jj_mm_aaaa(&champ(r, "reprise"))));
    }
    if vrai_de("subrogation") {
        detail.push(format!("subrogation {}", if champ(r, "subrogation") == "01" { "oui" } else { "non" }));
    }
    if vrai_de("date_accident") {
        detail.push(format!("accident le {}", jj_mm_aaaa(&champ(r, "date_accident"))));
    }
    dire!("             {}", detail.join(", "));
    let signalements: Vec<Value> = r.get("signalements").and_then(Value::as_array).cloned().unwrap_or_default();
    if signalements.is_empty() {
        dire!("             PAS ENCORE SIGNALE");
    } else {
        for e in &signalements {
            let quand = if e.get("quand").is_some() { champ(e, "quand") } else { "?".to_string() };
            dire!("             signale le {} (nature {}, ordre {})", quand, champ(e, "nature"), champ(e, "ordre"));
        }
    }
    let reporte: Vec<String> = r.get("reporte_par").and_then(Value::as_array).map(|a| a.iter().map(texte).collect()).unwrap_or_default();
    if !reporte.is_empty() {
        dire!("             reporte par la DSN de {}", reporte.join(", "));
    }
    if let Some(m) = r.get("maintien").and_then(Value::as_object).filter(|m| !m.is_empty()) {
        let l: Vec<String> = m.iter().map(|(mois, v)| format!("{} : {} EUR", mois, texte(v))).collect();
        dire!("             maintien de salaire {}", l.join(", "));
    }
    for c in r.get("cachets").and_then(Value::as_array).map_or(&[][..], |a| &a[..]) {
        let dates: Vec<String> = c.get("dates").and_then(Value::as_array).map_or_else(Vec::new, |a| a.iter().map(|d| jj_mm_aaaa(&texte(d))).collect());
        dire!("             ! {} paye(s) pendant l'arret ({}) : {}", champ(c, "libelle"), champ(c, "mois"), dates.join(", "));
    }
    // Les obligations, leur delai et ou on en est : ce que le moteur rend (`obligations.rs`), tel quel.
    for o in r.get("obligations").and_then(Value::as_array).into_iter().flatten() {
        dire!("             {} : {}", champ(o, "libelle"), champ(o, "phrase"));
    }
}

/// Les obligations d'un arret, telles que `arret-connaissance`, `accident-declare` et `attestation-declare` les rendent.
fn obligations_rendues(r: &Value, json: bool) -> i32 {
    if json {
        return rendre_json(r);
    }
    for o in r.get("obligations").and_then(Value::as_array).into_iter().flatten() {
        dire!("  {} : {}", champ(o, "libelle"), champ(o, "phrase"));
    }
    0
}

/// La reponse `--json` : `sortie_json.enveloppe("arret", brut(resultat))`.
fn rendre_json(resultat: &Value) -> i32 {
    rendre(&enveloppe("arret", brut(resultat)))
}

/// Les codes saisissables d'une nomenclature (`signalement.motifs_arret()`, `motifs_reprise()`).
fn saisissables(cadre: &Cadre, quoi: &str) -> Result<Vec<Value>, Echec> {
    let n = crate::routes_listes::nomenclatures(&cadre.ctx)?;
    let codes = n.get(quoi).and_then(|s| s.get("codes")).and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(codes.into_iter().filter(|c| c.get("saisissable").is_some_and(vrai)).collect())
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    let quoi = args.texte("quoi").unwrap_or("liste");

    if quoi == "motifs" {
        let arret = saisissables(cadre, "motifs_arret")?;
        let reprise = saisissables(cadre, "motifs_reprise")?;
        if json {
            let n = crate::routes_listes::nomenclatures(&cadre.ctx)?;
            let subrogation = n.get("subrogation").and_then(|s| s.get("codes")).cloned().unwrap_or(Value::Null);
            return Ok(rendre_json(&json!({"motifs_arret": arret, "motifs_reprise": reprise, "subrogation": subrogation})));
        }
        dire!("Motifs d'arret (S21.G00.60.001) :");
        for c in &arret {
            let accident = if c.get("date_accident").is_some_and(vrai) { "   (date de l'accident obligatoire)" } else { "" };
            dire!("  {}  {}{}", champ(c, "code"), champ(c, "libelle"), accident);
        }
        dire!("\nMotifs de reprise (S21.G00.60.011) :");
        for c in &reprise {
            dire!("  {}  {}", champ(c, "code"), champ(c, "libelle"));
        }
        return Ok(0);
    }

    if quoi == "contrats" {
        let salarie = exiger(args, "contrats", &["le"])?;
        let jour = date(args, "le")?.unwrap_or_default();
        let r = cadre.route("arret-contrats", json!({"salarie": salarie, "jour": jour}))?;
        let contrats: Vec<Value> = r.get("contrats").and_then(Value::as_array).cloned().unwrap_or_default();
        if json {
            return Ok(rendre_json(&json!({"contrats": contrats})));
        }
        if contrats.is_empty() {
            dire!("Aucun contrat de {} ne couvre le {}.", salarie, jj_mm_aaaa(&jour));
            return Ok(1);
        }
        for c in &contrats {
            dire!(
                "  {:<12} du {} au {}   (saisi dans {})",
                champ(c, "numero"),
                jj_mm_aaaa(&champ(c, "debut")),
                jj_mm_aaaa(&champ(c, "fin")),
                champ(c, "mois")
            );
        }
        return Ok(0);
    }

    if quoi == "saisir" {
        let salarie = exiger(args, "saisir", &["dernier-jour", "fin", "motif"])?;
        let mut charge = json!({
            "salarie": salarie,
            "dernier_jour": date(args, "dernier-jour")?,
            "fin": date(args, "fin")?,
            "motif": opt(args, "motif"),
            "qui": "ligne de commande",
        });
        if let Some(v) = args.texte("subrogation") {
            charge["subrogation"] = oui_non(cadre, v)?;
        }
        for (option, cle) in [("subrogation-debut", "subrogation_debut"), ("subrogation-fin", "subrogation_fin"), ("accident", "date_accident")] {
            if let Some(d) = date(args, option)? {
                charge[cle] = json!(d);
            }
        }
        if let Some(c) = opt(args, "contrat") {
            charge["contrat"] = json!(c);
        }
        if let Some(j) = opt(args, "jours-deja-indemnises") {
            charge["jours_deja_indemnises"] = json!(j);
        }
        if let Some(n) = opt(args, "note") {
            charge["note"] = json!(n);
        }
        let r = cadre.route("arret-enregistrer", charge)?;
        let resume = r.get("arret").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre_json(&resume));
        }
        dire!("Arret enregistre. Rien n'est parti.");
        lignes_arret(&resume);
        dire!(
            "\nEnsuite : `paie arret signaler {} --dernier-jour {}` produit le signalement, `paie depot-dsn` le depose.",
            champ(&resume, "salarie"),
            champ(&resume, "dernier_jour_travaille")
        );
        return Ok(0);
    }

    if quoi == "reprise" {
        let salarie = exiger(args, "reprise", &["dernier-jour", "le"])?;
        let charge = json!({
            "salarie": salarie,
            "dernier_jour": date(args, "dernier-jour")?,
            "reprise": date(args, "le")?,
            "motif_reprise": args.texte("motif-reprise").unwrap_or("01"),
        });
        let r = cadre.route("arret-reprise", charge)?;
        let resume = r.get("arret").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre_json(&resume));
        }
        dire!("Reprise notee le {}.", jj_mm_aaaa(&champ(&resume, "reprise")));
        lignes_arret(&resume);
        dire!(
            "\nUne reprise ANTICIPEE se signale (nature 05) : `paie arret signaler {} --dernier-jour {} --reprise`.",
            champ(&resume, "salarie"),
            champ(&resume, "dernier_jour_travaille")
        );
        return Ok(0);
    }

    if quoi == "prolonger" {
        let salarie = exiger(args, "prolonger", &["dernier-jour"])?;
        let mut charge = json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?, "qui": "ligne de commande"});
        if let Some(f) = date(args, "fin")? {
            charge["fin"] = json!(f);
        }
        if let Some(f) = date(args, "subrogation-fin")? {
            charge["subrogation_fin"] = json!(f);
        }
        let r = cadre.route("arret-prolonger", charge)?;
        if json {
            return Ok(rendre_json(&r));
        }
        let resume = r.get("arret").cloned().unwrap_or(Value::Null);
        dire!("Arrêt prescrit jusqu'au {}.", jj_mm_aaaa(&champ(&resume, "fin_previsionnelle")));
        lignes_arret(&resume);
        for a in r.get("avertissements").and_then(Value::as_array).into_iter().flatten() {
            dire!("  {}", texte(a));
        }
        if r.get("subrogation_a_revoir").is_some_and(vrai) {
            dire!(
                "\n{}\n`paie arret prolonger {} --dernier-jour {} --subrogation-fin AAAA-MM-JJ` la porte.",
                texte(&r["phrase"]),
                champ(&resume, "salarie"),
                champ(&resume, "dernier_jour_travaille")
            );
        }
        return Ok(0);
    }

    if quoi == "maintien" {
        let salarie = exiger(args, "maintien", &["dernier-jour"])?;
        let mut charge = json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?});
        if let Some(m) = opt(args, "mois") {
            charge["mois"] = json!(m);
            charge["montant"] = opt(args, "montant").map_or(Value::Null, |t| json!(t));
        }
        if let Some(j) = args.texte("jours-deja-indemnises") {
            charge["jours_deja_indemnises"] = json!(j);
        }
        let r = cadre.route("arret-maintien", charge)?;
        let resume = r.get("arret").cloned().unwrap_or(Value::Null);
        if json {
            return Ok(rendre_json(&resume));
        }
        dire!("Maintien note. Rien n'est parti : la paie du mois le portera.");
        lignes_arret(&resume);
        return Ok(0);
    }

    if quoi == "signaler" {
        let salarie = exiger(args, "signaler", &["dernier-jour"])?;
        let nature = if args.drapeau("reprise") { declarations::signalement::REPRISE } else { declarations::signalement::ARRET };
        let mut charge = json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?, "nature": nature});
        if let Some(r) = opt(args, "remplace") {
            charge["remplace"] = json!(r);
        }
        if let Some(o) = args.entier("ordre").filter(|o| *o != 0) {
            charge["ordre"] = json!(o);
        }
        if args.drapeau("test") {
            charge["test"] = json!(true);
        }
        if let Some(l) = date(args, "le")? {
            charge["le"] = json!(l);
        }
        let r = cadre.route("arret-signaler", charge)?;
        let avertissements: Vec<Value> = r.get("avertissements").and_then(Value::as_array).cloned().unwrap_or_default();
        if json {
            return Ok(rendre_json(&json!({
                "fichier": r["fichier"], "nature": r["nature"], "type": r["type"], "ordre": r["ordre"], "avertissements": avertissements,
            })));
        }
        dire!("Ecrit : {}", champ(&r, "fichier"));
        let nom_nature = if nature == declarations::signalement::ARRET { "signalement arret de travail" } else { "signalement reprise suite a arret de travail" };
        dire!(
            "Nature {} ({}), type {}, ordre {}{}.",
            nature,
            nom_nature,
            champ(&r, "type"),
            champ(&r, "ordre"),
            if args.drapeau("test") { " -- ENVOI DE TEST" } else { "" }
        );
        for a in &avertissements {
            dire!("  ! {}", texte(a));
        }
        dire!("\nRien n'est parti. Deposer : `paie depot-dsn {} --test` d'abord, puis `--reel`.", champ(&r, "fichier"));
        return Ok(0);
    }

    if quoi == "connu" {
        let salarie = exiger(args, "connu", &["dernier-jour", "le"])?;
        let de = opt(args, "de").unwrap_or("arret");
        let mut charge = json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?, "de": de, "jour": date(args, "le")?});
        if let Some(h) = opt(args, "heure") {
            charge["heure"] = json!(h);
        }
        let r = cadre.route("arret-connaissance", charge)?;
        return Ok(obligations_rendues(&r, json));
    }

    if quoi == "declare" {
        let salarie = exiger(args, "declare", &["dernier-jour"])?;
        let mut charge = json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?});
        if args.drapeau("retirer") {
            charge["le"] = json!("");
        } else {
            let le = date(args, "le")?.ok_or_else(|| {
                refus(if args.drapeau("attestation") {
                    "Il manque --le (le jour de l'attestation) et --ou (net-entreprises-ecran ou net-entreprises-edi) pour `paie arret declare --attestation`."
                } else {
                    "Il manque --le (le jour de la declaration) et --ou (net-entreprises ou courrier) pour `paie arret declare`."
                })
            })?;
            charge["le"] = json!(le);
            charge["depot"] = json!(opt(args, "ou").unwrap_or_default());
            if let Some(r) = opt(args, "reference") {
                charge["reference"] = json!(r);
            }
            if let Some(h) = opt(args, "heure") {
                charge["heure"] = json!(h);
            }
        }
        let r = cadre.route(if args.drapeau("attestation") { "attestation-declare" } else { "accident-declare" }, charge)?;
        return Ok(obligations_rendues(&r, json));
    }

    if quoi == "oublier" {
        let salarie = exiger(args, "oublier", &["dernier-jour"])?;
        let r = cadre.route("arret-oublier", json!({"salarie": salarie, "dernier_jour": date(args, "dernier-jour")?}))?;
        let resume = r.get("arret").cloned().unwrap_or(Value::Null);
        dire!(
            "Arret de {} au {} retire du magasin. Il n'avait jamais ete declare.",
            champ(&resume, "salarie"),
            jj_mm_aaaa(&champ(&resume, "dernier_jour_travaille"))
        );
        return Ok(0);
    }

    // ------------------------------------------------------------ la liste
    let r = cadre.route("arrets", json!({}))?;
    let tous: Vec<Value> = r.get("arrets").and_then(Value::as_array).cloned().unwrap_or_default();
    let attente: Vec<Value> = r.get("a_signaler").and_then(Value::as_array).cloned().unwrap_or_default();
    if json {
        return Ok(rendre_json(&json!({"arrets": tous, "a_signaler": attente})));
    }
    if tous.is_empty() {
        dire!("Aucun arret de travail saisi.");
        dire!("`paie arret saisir CLE --dernier-jour AAAA-MM-JJ --fin AAAA-MM-JJ --motif 01` en ouvre un ; `paie arret motifs` liste les motifs.");
        return Ok(0);
    }
    dire!("Arrets de travail ({}) :", tous.len());
    for r in &tous {
        lignes_arret(r);
    }
    if !attente.is_empty() {
        dire!("\n{} arret(s) attendent leur signalement : son delai est dit sous chacun.", attente.len());
    }
    Ok(0)
}
