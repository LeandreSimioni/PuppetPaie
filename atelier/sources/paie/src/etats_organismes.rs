//! Les etats des cotisations et des organismes de `moteur/operations_etats.py` : les cotisations par organisme,
//! les versements aux organismes (bloc 20 des DSN deposees), le bordereau Urssaf (blocs 22 et 23), le FCAP-SVP de
//! la 3090, les contributions Afdas, le paritarisme de branche.
//!
//! CE QUI SE LIT DANS UNE DSN DEPOSEE SE LIT DANS LA DSN DEPOSEE : les versements et le bordereau sont lus dans le
//! fichier parti (`casc::dsn_deposee`), jamais recomposes. Les montants annuels hors DSN (FCAP-SVP, Afdas,
//! paritarisme) se calculent sur le brut du livre de paie ou sur la tranche A declaree, avec les taux du bareme.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use crate::etats_paie::{ecarter_declares, finir, lignes_livre, Demande, Lecture, Periode};
use socle::fichiers::vrai;
use socle::num::{add, div, eur2, mul, zero2, Decimal};
use crate::tableaux_export::{col_m, col_n, col_t, col_x, libelle_mois, montant, qui, refus, sections_de, Ligne, Section, Tableau, R};

// ------------------------------------------------------------------ outils

/// `min(a, b)` de Python : le premier des deux quand ils sont egaux.
fn py_min(a: Decimal, b: Decimal) -> Decimal {
    if b < a {
        b
    } else {
        a
    }
}

/// `max(a, b)` de Python : le premier des deux quand ils sont egaux.
fn py_max(a: Decimal, b: Decimal) -> Decimal {
    if b > a {
        b
    } else {
        a
    }
}

/// `Decimal(str(v))` d'une valeur d'un fichier de baremes.
fn dec_de(v: &Value) -> R<Decimal> {
    socle::num::dec(v).map_err(|m| erreur("InvalidOperation", m))
}

/// `plan_comptable(ctx)` : le plan livre recouvert par `config/plan-comptable.json`, et d'ou il vient.
pub(crate) fn plan(ctx: &Contexte) -> R<(Map<String, Value>, &'static str)> {
    let p = crate::routes_dossiers::plan_comptable(ctx)?;
    let origine = if ctx.racine.join("config").join("plan-comptable.json").exists() { "config/plan-comptable.json" } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    Ok((p, origine))
}

/// `_organismes` : identifiant de ligne -> nom de l'organisme, lu dans le plan comptable (le meme endroit que
/// l'export compta : l'etat des cotisations et le credit des comptes 43 ne peuvent pas diverger).
fn organismes(ctx: &Contexte) -> R<Vec<(String, String)>> {
    let (plan, _) = plan(ctx)?;
    let noms = plan.get("tiers").filter(|t| vrai(t));
    let mut out = Vec::new();
    if let Some(Value::Object(lignes)) = plan.get("lignes") {
        for (k, v) in lignes {
            let tiers = v.get("tiers").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("tiers")))?;
            let nom = noms.and_then(|n| n.get(valeur_en_texte(tiers).as_str())).filter(|n| vrai(n)).map(valeur_en_texte);
            out.push((k.clone(), nom.unwrap_or_else(|| format!("Organisme {}", valeur_en_texte(tiers)))));
        }
    }
    Ok(out)
}

/// Les annees dont le socle est livre (`baremes.annees()`).
fn annees_du_socle(ctx: &Contexte) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    if let Ok(l) = std::fs::read_dir(ctx.code.join("baremes")) {
        for e in l.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = nom.strip_suffix(".json") {
                if !stem.is_empty() && stem.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(n) = stem.parse() {
                        out.push(n);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// `baremes.socle(an) or baremes.socle(max(baremes.annees()))`.
fn socle_de(ctx: &Contexte, an: i32) -> R<Value> {
    if let Some(s) = ctx.socle_livre(an).map_err(|m| erreur("OSError", m))?.filter(vrai) {
        return Ok(s);
    }
    let derniere = annees_du_socle(ctx).into_iter().max().ok_or_else(|| erreur("ValueError", "max() arg is an empty sequence"))?;
    Ok(ctx.socle_livre(derniere).map_err(|m| erreur("OSError", m))?.unwrap_or(Value::Null))
}

fn annee_entiere(an: &str) -> R<i32> {
    an.parse::<i32>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", socle::fichiers::repr(an))))
}

fn idcc_de(ctx: &Contexte) -> String {
    ctx.entreprise.get("idcc").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_default()
}

// ===================================== 6. les cotisations par organisme

/// Une ligne de cumul : un organisme, une cotisation, ses taux -- dans l'ordre ou on l'a vue.
struct Cumul {
    cle: (String, String, String, String, Option<Decimal>, Option<Decimal>),
    mois: String,
    organisme: String,
    code: String,
    libelle: String,
    taux_sal: Option<Decimal>,
    taux_pat: Option<Decimal>,
    base: Decimal,
    part_sal: Decimal,
    part_pat: Decimal,
}

/// `operations_etats.cotisations_par_organisme` : ce qui est du a chaque organisme, rubrique par rubrique.
pub(crate) fn cotisations_par_organisme(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let regroupement = d.regroupement.as_str();
    if !["organisme", "mois"].contains(&regroupement) {
        return Err(refus(format!("Regroupement inconnu pour les cotisations : « {} » (organisme ou mois).", regroupement)));
    }
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let orgs = organismes(ctx)?;
    let org_de = |id: &str| orgs.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone());
    let mut lecture = Lecture::nouvelle(ctx);
    let mut cumul: Vec<Cumul> = Vec::new();
    let mut manque: BTreeSet<String> = BTreeSet::new();
    let zero = zero2();
    let mut ajouter = |m: &str, org: &str, code: &str, libelle: &str, base: Option<Decimal>, ts: Option<Decimal>, ms: Decimal, tp: Option<Decimal>, mp: Decimal| {
        let cle = (if regroupement == "mois" { m.to_string() } else { String::new() }, org.to_string(), code.to_string(), libelle.to_string(), ts, tp);
        let i = match cumul.iter().position(|c| c.cle == cle) {
            Some(i) => i,
            None => {
                cumul.push(Cumul {
                    cle,
                    mois: m.to_string(),
                    organisme: org.to_string(),
                    code: code.to_string(),
                    libelle: libelle.to_string(),
                    taux_sal: ts,
                    taux_pat: tp,
                    base: zero,
                    part_sal: zero,
                    part_pat: zero,
                });
                cumul.len() - 1
            }
        };
        let c = &mut cumul[i];
        c.base = add(c.base, base.filter(|b| !b.is_zero()).unwrap_or(zero));
        c.part_sal = add(c.part_sal, if ms.is_zero() { zero } else { ms });
        c.part_pat = add(c.part_pat, if mp.is_zero() { zero } else { mp });
    };
    for m in ecarter_declares(&p, &mut avert, "la répartition des cotisations")? {
        for paie in lecture.paies(&m)?.iter() {
            for c in &paie.lignes {
                let Some(org) = org_de(&c.id) else {
                    manque.insert(format!("{} ({})", c.id, c.libelle));
                    continue;
                };
                let ts = c.taux_sal.map(crate::etats_paie::dec_taux).transpose()?;
                let tp = c.taux_pat.map(crate::etats_paie::dec_taux).transpose()?;
                if c.id == "reduction_generale" {
                    if let Some(rg) = &paie.reduction_generale {
                        let mut reparti = zero;
                        for (part, id_org, quoi) in [(rg.urssaf, "maladie", "part Urssaf"), (rg.chomage, "chomage", "part chômage"), (rg.agirc_arrco, "retraite_t1", "part retraite")] {
                            if !part.is_zero() {
                                let o = org_de(id_org).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(id_org)))?;
                                ajouter(&m, &o, &c.id, &format!("{} — {}", c.libelle, quoi), None, None, zero, None, -part);
                                reparti = add(reparti, part);
                            }
                        }
                        if -reparti != c.montant_pat {
                            avert.push(format!(
                                "{}, {} : la réduction générale ({}) ne tombe pas sur la somme de ses parts ({}).",
                                libelle_mois(&m)?,
                                qui(&paie.salarie)?,
                                eur2(c.montant_pat),
                                -reparti
                            ));
                        }
                        continue;
                    }
                }
                ajouter(&m, &org, &c.id, &c.libelle, Some(eur2(c.base)), ts, eur2(c.montant_sal), tp, eur2(c.montant_pat));
            }
        }
    }
    if !manque.is_empty() {
        return Err(refus(format!(
            "Lignes de bulletin sans organisme dans le plan comptable : {}. Les ajouter dans config/plan-comptable.json sous « lignes » : on ne devine pas à qui une cotisation est due.",
            manque.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    let mut lignes = Vec::new();
    for c in cumul {
        let mut l = Ligne::nouvelle();
        l.t("mois", c.mois).t("organisme", c.organisme).t("code", c.code).t("libelle", c.libelle).od("taux_sal", c.taux_sal).od("taux_pat", c.taux_pat);
        l.od("base", if c.base == zero { None } else { Some(c.base) }).d("part_sal", c.part_sal).d("part_pat", c.part_pat).d("total", add(c.part_sal, c.part_pat));
        lignes.push(l);
    }
    let tri = |l: &Ligne| vec![l.texte("code"), l.texte("libelle")];
    let mut t = Tableau::nouveau("cotisations-par-organisme", "Cotisations par organisme", p.libelle.clone());
    t.mois = p.mois.clone();
    t.sections = if regroupement == "mois" {
        sections_de(lignes, |l| (l.texte("mois"), l.texte("organisme")), |k| format!("{} — {}", libelle_mois(&k.0).unwrap_or_else(|_| k.0.clone()), k.1), Some(&tri))
    } else {
        sections_de(lignes, |l| l.texte("organisme"), |k| k.clone(), Some(&tri))
    };
    // Par organisme, la section porte deja son nom : une colonne qui le repeterait ne dirait rien de plus.
    let mut cols = if regroupement == "mois" { vec![col_t("mois", "Mois"), col_t("organisme", "Organisme")] } else { Vec::new() };
    cols.extend([
        col_t("libelle", "Cotisation"),
        col_t("code", "Code"),
        // La base ne se totalise pas : deux assiettes differentes ne s'additionnent pas.
        col_x("base", "Base"),
        col_x("taux_sal", "Taux sal. %"),
        col_m("part_sal", "Part salariale"),
        col_x("taux_pat", "Taux pat. %"),
        col_m("part_pat", "Part patronale"),
        col_m("total", "Total dû"),
    ]);
    t.colonnes = cols;
    t.sources = vec![
        "lignes de cotisation du bulletin ; organisme : compte de tiers du plan comptable (plan-comptable-base.json ou config/plan-comptable.json)".to_string(),
        "une base additionnée sur plusieurs bulletins est la somme des assiettes, pas une assiette déclarée".to_string(),
    ];
    avert.push("Les « autres contributions dues par l'employeur » (FNAL, CSA, dialogue social, formation) sont versées à l'Urssaf sans ventilation, comme le bulletin les imprime.".to_string());
    t.avertissements = avert;
    finir(ctx, &mut t, Some(&p), &format!("cotisations-par-organisme-{}", regroupement), &d.format, d.ecrire)?;
    Ok(t)
}

// ===================================== 7. les versements aux organismes (bloc 20)

/// Un bloc d'une DSN : `{rubrique: valeur}`, et le bloc dont il vient.
struct Bloc {
    bloc: String,
    v: Vec<(String, String)>,
}

impl Bloc {
    fn get(&self, k: &str) -> Option<&str> {
        self.v.iter().rev().find(|(c, _)| c == k).map(|(_, x)| x.as_str())
    }

    fn texte(&self, k: &str) -> String {
        self.get(k).unwrap_or("").to_string()
    }
}

/// `_lire_blocs` : les blocs d'une DSN dont la rubrique commence par l'un des prefixes. Un bloc commence a sa
/// rubrique .001.
fn lire_blocs(chemin: &Path, prefixes: &[&str]) -> R<Vec<Bloc>> {
    let octets = std::fs::read(chemin).map_err(|e| erreur("OSError", format!("{} : {}", chemin.display(), e)))?;
    let texte = crate::apercu_dsn::texte_latin1(&octets);
    let mut blocs: Vec<Bloc> = Vec::new();
    let mut en_cours = false;
    for l in crate::apercu_dsn::lignes_py(&texte) {
        let Some((k, v)) = l.split_once(',') else { continue };
        let v = v.trim_matches(socle::texte::blanc).trim_matches('\'').to_string();
        let bloc: String = k.chars().take(10).collect();
        if !prefixes.contains(&bloc.as_str()) {
            en_cours = false;
            continue;
        }
        if k.ends_with(".001") || !en_cours || blocs.last().map_or(true, |b| b.bloc != bloc) {
            blocs.push(Bloc { bloc: bloc.clone(), v: Vec::new() });
            en_cours = true;
        }
        let rubrique: String = k.chars().skip(11).collect();
        let cur = blocs.last_mut().expect("un bloc");
        match cur.v.iter_mut().find(|(c, _)| *c == rubrique) {
            Some(x) => x.1 = v,
            None => cur.v.push((rubrique, v)),
        }
    }
    Ok(blocs)
}

/// `_date_dsn` : JJMMAAAA -> JJ/MM/AAAA.
fn date_dsn(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 {
        format!("{}/{}/{}", c[..2].iter().collect::<String>(), c[2..4].iter().collect::<String>(), c[4..8].iter().collect::<String>())
    } else {
        v.to_string()
    }
}

/// `_montant` d'un texte de la DSN.
fn montant_dsn(b: &Bloc, k: &str) -> R<Option<Decimal>> {
    montant(b.get(k).map(|t| Value::String(t.to_string())).as_ref())
}

/// `_dsn_des_mois` : [(mois, chemin)] des DSN deposees de la periode -- lues, jamais recomposees. Un mois saisi
/// sans DSN reelle acceptee est dit.
fn dsn_des_mois(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>) -> R<Vec<(String, PathBuf)>> {
    let (mut out, mut sans) = (Vec::new(), Vec::new());
    for m in &p.mois {
        match crate::casc::dsn_deposee(ctx, m, None) {
            Err(e) => avert.push(e),
            Ok(None) => sans.push(m.clone()),
            Ok(Some(c)) => out.push((m.clone(), c)),
        }
    }
    if !sans.is_empty() {
        avert.push(format!("Pas de DSN réelle acceptée au journal pour {} : ces mois ne sont pas dans l'état, qui ne lit que ce qui est parti.", sans.join(", ")));
    }
    if out.is_empty() {
        return Err(refus(format!("Aucune DSN déposée sur la période ({}) : cet état se lit dans les DSN parties, il n'y a rien à lire.", p.libelle)));
    }
    Ok(out)
}

/// Les paies declarees de chaque mois de la periode (`dsn::paies_declarees_du_mois` : la version sur laquelle la DSN est
/// partie, ou la paie ecrite si rien n'est parti), tous etablissements. Un mois sans paie validee est dit.
fn paies_des_mois(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>) -> R<Vec<(String, Vec<engagement::calcul::Paie>)>> {
    let (mut out, mut sans) = (Vec::new(), Vec::new());
    for m in &p.mois {
        let paies = crate::dsn::paies_declarees_du_mois(ctx, m, None).map_err(refus)?;
        if paies.is_empty() {
            sans.push(m.clone());
        } else {
            out.push((m.clone(), paies));
        }
    }
    if !sans.is_empty() {
        avert.push(format!("Aucune paie validée pour {} : ces mois ne sont pas dans l'état.", sans.join(", ")));
    }
    if out.is_empty() {
        return Err(refus(format!("Aucune paie validée sur la période ({}) : il n'y a rien à lire.", p.libelle)));
    }
    Ok(out)
}

/// `_libelles_organismes` : SIRET -> libelle, lu dans `config/organismes.json`.
fn libelles_organismes(ctx: &Contexte) -> R<Vec<(String, String)>> {
    let Some(d) = ctx.organismes()? else { return Ok(Vec::new()) };
    let mut out: Vec<(String, String)> = Vec::new();
    for v in d.get("versements").and_then(Value::as_array).into_iter().flatten() {
        if let Some(siret) = v.get("siret").filter(|s| vrai(s)) {
            let (k, lib) = (valeur_en_texte(siret), v.get("libelle").map(valeur_en_texte).unwrap_or_default());
            match out.iter_mut().find(|(c, _)| *c == k) {
                Some(x) => x.1 = lib,
                None => out.push((k, lib)),
            }
        }
    }
    Ok(out)
}

fn nom_du_fichier(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `operations_etats.versements_organismes` : chaque versement que les DSN deposees ont ordonne (bloc S21.G00.20).
pub(crate) fn versements_organismes(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let noms = libelles_organismes(ctx)?;
    let mut lignes = Vec::new();
    for (m, chemin) in dsn_des_mois(ctx, &p, &mut avert)? {
        for b in lire_blocs(&chemin, &["S21.G00.20"])? {
            let ident = b.texte("001");
            let organisme = match noms.iter().find(|(k, _)| *k == ident).map(|(_, v)| v.clone()).filter(|v| !v.is_empty()) {
                Some(n) => n,
                None => ident.clone(),
            };
            let mode = b.texte("010");
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("organisme", organisme).t("identifiant", ident).t("reference", b.texte("002"));
            l.t("du", date_dsn(&b.texte("006"))).t("au", date_dsn(&b.texte("007"))).od("montant", montant_dsn(&b, "005")?);
            l.t("mode", if mode == "05" { "prélèvement SEPA".to_string() } else { format!("code {}", mode) });
            l.t("date", date_dsn(&b.texte("011"))).t("fichier", nom_du_fichier(&chemin));
            lignes.push(l);
        }
    }
    if lignes.iter().any(|l| l.texte("date").is_empty()) {
        avert.push("Un versement sans date de paiement est prélevé à l'échéance légale de l'organisme.".to_string());
    }
    let mut t = Tableau::nouveau("versements-organismes", "Versements aux organismes (DSN)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), None);
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("organisme", "Organisme"),
        col_t("identifiant", "SIRET ou code"),
        col_t("reference", "Référence"),
        col_t("du", "Période du"),
        col_t("au", "au"),
        col_m("montant", "Montant"),
        col_t("mode", "Mode"),
        col_t("date", "Date de paiement"),
        col_t("fichier", "DSN"),
    ];
    t.sources = vec![
        "DSN déposée, bloc S21.G00.20 « Versement organisme de protection sociale » (cahier technique DSN 2026.1) ; libellés : config/organismes.json".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "versements-organismes", &d.format, d.ecrire)?;
    Ok(t)
}

// ===================================== 8. le bordereau Urssaf (blocs 22 et 23)

/// Un lecteur CSV a la maniere de Python (`csv.reader(delimiter=";")`) : guillemets doubles, fins de ligne CR LF.
fn lire_csv(texte: &str, delim: char) -> Vec<Vec<String>> {
    let (mut rangs, mut rang, mut champ): (Vec<Vec<String>>, Vec<String>, String) = (Vec::new(), Vec::new(), String::new());
    let (mut guillemets, mut commence) = (false, true);
    let mut it = texte.chars().peekable();
    while let Some(c) = it.next() {
        if guillemets {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    champ.push('"');
                    it.next();
                } else {
                    guillemets = false;
                }
            } else {
                champ.push(c);
            }
            continue;
        }
        match c {
            '"' if commence => {
                guillemets = true;
                commence = false;
            }
            c if c == delim => {
                rang.push(std::mem::take(&mut champ));
                commence = true;
            }
            '\r' | '\n' => {
                if c == '\r' && it.peek() == Some(&'\n') {
                    it.next();
                }
                if !(rang.is_empty() && champ.is_empty() && commence) {
                    rang.push(std::mem::take(&mut champ));
                    rangs.push(std::mem::take(&mut rang));
                }
                commence = true;
            }
            c => {
                champ.push(c);
                commence = false;
            }
        }
    }
    if !(rang.is_empty() && champ.is_empty()) {
        rang.push(champ);
        rangs.push(rang);
    }
    rangs
}

/// `_libelles_ctp` : code CTP -> libelle, lu dans la table de l'Urssaf livree avec le logiciel
/// (`donnees/urssaf-codes-types-personnel.csv`, celle de `bordereau.rs`). Avant le 15/10/2026, on la cherchait dans
/// `reference/sources/` de la compagnie, qui n'existait que chez l'editeur ; les textes de reference vivent desormais
/// dans le depot (`atelier/reference/`), et la table livree en est la copie conforme (`sources.json`).
fn libelles_ctp(ctx: &Contexte) -> R<Vec<(String, String)>> {
    let p = ctx.code.join("urssaf-codes-types-personnel.csv");
    if !p.exists() {
        return Ok(Vec::new());
    }
    let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
    let texte = String::from_utf8_lossy(&octets).into_owned();
    let texte = texte.strip_prefix('\u{feff}').unwrap_or(&texte).to_string();
    let rangs = lire_csv(&texte, ';');
    let Some((entete, corps)) = rangs.split_first() else { return Ok(Vec::new()) };
    let col = |nom: &str| entete.iter().position(|e| e == nom);
    let (i_code, i_lib, i_fin) = (col("Code"), col("Libellé"), col("Date de fin"));
    let mut out: Vec<(String, String)> = Vec::new();
    for r in corps {
        let cel = |i: Option<usize>| i.and_then(|i| r.get(i)).cloned().unwrap_or_default();
        let code = cel(i_code).trim_matches('\'').to_string();
        let lib = cel(i_lib);
        if code.is_empty() {
            continue;
        }
        match out.iter_mut().find(|(c, _)| *c == code) {
            Some(x) => {
                if cel(i_fin).is_empty() {
                    x.1 = lib;
                }
            }
            None => out.push((code, lib)),
        }
    }
    Ok(out)
}

/// `operations_etats.bordereau_urssaf` : le bordereau Urssaf de chaque mois, CTP par CTP, tel que la DSN le porte (`dsn::bordereau_du_mois`). Les taux ne sont
/// pas appliques ici : c'est l'Urssaf qui calcule depuis les assiettes, et un export ne calcule pas.
pub(crate) fn bordereau_urssaf(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let ctp = libelles_ctp(ctx)?;
    if ctp.is_empty() {
        avert.push("La table des codes de l'Urssaf manque au logiciel : les codes sont donnés sans libellé.".to_string());
    }
    let mut lignes = Vec::new();
    for (m, paies) in paies_des_mois(ctx, &p, &mut avert)? {
        let debut = socle::date::Jour::lire(&format!("{}-01", m)).map_err(refus)?;
        let fin = socle::date::dernier_du_mois(&m).map_err(refus)?;
        for nic in crate::dsn::nics(&ctx.entreprise) {
            let mut siennes = Vec::new();
            for q in &paies {
                if droit::bareme::etablissement_du_salarie(&ctx.entreprise, &q.salarie).map_err(refus)? == nic {
                    siennes.push(q.clone());
                }
            }
            if siennes.is_empty() {
                continue;
            }
            let (lignes_23, total) = crate::dsn::bordereau_du_mois(ctx, &siennes, &m, Some(&nic)).map_err(refus)?;
            let mut l = Ligne::nouvelle();
            l.t("mois", m.clone()).t("ctp", "").t("libelle", "Total dû à l'Urssaf (bloc 22)");
            l.t("periode", format!("{} au {}", debut.jj_mm_aaaa(), fin.jj_mm_aaaa())).od("total", Some(total));
            lignes.push(l);
            for b in lignes_23 {
                let mut l = Ligne::nouvelle();
                let code = b.code.clone();
                l.t("mois", m.clone()).t("ctp", code.clone()).t("libelle", ctp.iter().find(|(c, _)| *c == code).map(|(_, v)| v.clone()).unwrap_or_default());
                l.t("qualifiant", match b.qualifiant.as_str() {
                    "920" => "920 — assiette plafonnée".to_string(),
                    "921" => "921 — assiette déplafonnée".to_string(),
                    q => q.to_string(),
                });
                let taux_at = if b.taux_at.is_empty() { None } else { montant(Some(&Value::String(b.taux_at.clone())))? };
                l.od("taux_at", taux_at).od("assiette", if b.montant.is_some() { None } else { b.assiette }).od("montant", b.montant);
                lignes.push(l);
            }
        }
    }
    let mut t = Tableau::nouveau("bordereau-urssaf", "Bordereau Urssaf (DSN)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), |k| libelle_mois(k).unwrap_or_else(|_| k.clone()), None);
    t.colonnes = vec![
        col_t("mois", "Mois"),
        col_t("ctp", "CTP"),
        col_t("libelle", "Libellé"),
        col_t("qualifiant", "Qualifiant"),
        col_x("taux_at", "Taux AT"),
        col_x("assiette", "Assiette déclarée"),
        col_m("montant", "Montant forfaitaire"),
        col_t("periode", "Période"),
        col_m("total", "Total dû"),
    ];
    t.sources = vec![
        "blocs S21.G00.22 « Bordereau de cotisation due » et S21.G00.23 « Cotisation agrégée » (cahier technique DSN 2026.1), tirés des paies déclarées par les fonctions qui écrivent la DSN".to_string(),
        "libellés : table des codes types de personnel, open.urssaf.fr (histocodestypescsv), relevée le 18/09/2026".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "bordereau-urssaf", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================== le FCAP-SVP (3090, titre V)

fn refuse_un_mois(quoi: &str, d: &Demande) -> R<()> {
    if d.mois.as_deref().is_some_and(|m| !m.is_empty()) {
        return Err(refus(quoi));
    }
    Ok(())
}

/// `operations_etats.fcap_svp` : 0,1 % de la tranche A de l'annee, plancher 80 EUR, plafond 300 EUR, paye HORS
/// DSN en debut d'annee. La tranche A est la base 02 que la DSN declare (S21.G00.78), tiree des paies declarees.
pub(crate) fn fcap_svp(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Le FCAP-SVP est une contribution annuelle : il se demande pour une année, ou pour toutes les années.", d)?;
    let idcc = idcc_de(ctx);
    let table = crate::routes_listes::lire_json(&socle::fichiers::dossier_du_code().join("prevoyance-conventionnelle-base.json"))?;
    let fp = table.get("conventions").and_then(|c| c.get(idcc.as_str())).and_then(|r| r.get("financement_paritarisme")).filter(|v| vrai(v)).cloned();
    let Some(fp) = fp else {
        return Err(refus(format!(
            "Le FCAP-SVP est la contribution de la convention 3090 (titre V) ; la fiche entreprise dit IDCC {}. Il n'y a rien à calculer.",
            if idcc.is_empty() { "rien".to_string() } else { idcc }
        )));
    };
    let p = Periode::nouvelle(ctx, None, d.annee.as_deref(), d.toutes, true)?;
    let mut avert = Vec::new();
    let (taux, plancher, plafond) = (dec_de(&fp["taux"])?, dec_de(&fp["plancher"])?, dec_de(&fp["plafond"])?);
    let mut par_an: Vec<(String, i64, Decimal)> = Vec::new();
    let zero = zero2();
    let orgs = ctx.organismes_exiges().map_err(refus)?;
    for (m, paies) in paies_des_mois(ctx, &p, &mut avert)? {
        // La tranche A : la base 02 (« assiette brute plafonnee ») que la DSN declare pour chaque contrat (`dsn::cotisations`).
        let mut ta = zero;
        for q in &paies {
            let nic = droit::bareme::etablissement_du_salarie(&ctx.entreprise, &q.salarie).map_err(refus)?;
            let bar = crate::dsn::bar_du(&ctx.du_mois(&m).map_err(refus)?, &ctx.entreprise, &orgs, &nic, &m).map_err(refus)?;
            for e in q.elementaires() {
                if let Some(c) = crate::dsn::cotisations(e, &bar).map_err(refus)?.into_iter().find(|c| c.bloc78 == "02") {
                    ta = add(ta, c.base78);
                }
            }
        }
        let an: String = m.chars().take(4).collect();
        match par_an.iter_mut().find(|(a, _, _)| *a == an) {
            Some(x) => {
                x.1 += 1;
                x.2 = add(x.2, ta);
            }
            None => par_an.push((an, 1, add(zero, ta))),
        }
    }
    par_an.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (an, mois, tranche_a) in par_an {
        let calcule = eur2(div(mul(tranche_a, taux), Decimal::ONE_HUNDRED));
        let du = py_max(plancher, py_min(plafond, calcule));
        let regle = if du == plancher && calcule < plancher {
            "plancher"
        } else if du == plafond && calcule > plafond {
            "plafond"
        } else {
            "0,1 % de la tranche A"
        };
        let mut l = Ligne::nouvelle();
        l.t("annee", an.clone()).n("mois", mois).d("tranche_a", tranche_a).d("taux", taux).d("calcule", calcule).d("du", du).t("regle", regle);
        l.t("a_payer", format!("début {} (inférence : le texte ne dit pas l'année de masse)", annee_entiere(&an)? + 1));
        lignes.push(l);
    }
    avert.push("Le FCAP-SVP se paie hors DSN, au FCAP-SVP (ou à l'organisme à qui il délègue la collecte, art. 5.2), en début d'année civile (art. 5.3). Que la tranche A soit celle de l'année écoulée est une inférence, à confirmer sur un appel du FCAP-SVP.".to_string());
    let mut t = Tableau::nouveau("fcap-svp", "FCAP-SVP — financement du paritarisme (IDCC 3090)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        col_t("annee", "Année"),
        col_n("mois", "Mois lus"),
        col_m("tranche_a", "Tranche A déclarée"),
        col_x("taux", "Taux %"),
        col_m("calcule", "0,1 % de la tranche A"),
        col_m("du", "Contribution due"),
        col_t("regle", "Règle"),
        col_t("a_payer", "À payer"),
    ];
    t.sources = vec![
        "CCN 3090, titre V, articles 5.1 (0,1 % de la tranche A, plancher 80 €, plafond 300 € par an et par entreprise), 5.2 et 5.3 — base KALI de la DILA, relevée le 23/09/2026".to_string(),
        "tranche A : assiette brute plafonnée que la DSN déclare (S21.G00.78.001 = 02), tirée des paies déclarées".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "fcap-svp", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================================ l'Afdas

/// `operations_etats.afdas_2642` : ce que la production audiovisuelle (2642) paie une fois l'an a l'Afdas, hors DSN,
/// avant le 28 fevrier. La production cinematographique (3097) paie les memes, aux memes taux ; les espaces de
/// loisirs (1790) la formation conventionnelle de leurs permanents. La masse se lit dans le livre de paie,
/// bulletin par bulletin, avec la nature du contrat : un CDDU est un intermittent.
pub(crate) fn afdas_2642(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Les contributions Afdas de la 2642 sont annuelles : elles se demandent pour une année, ou pour toutes.", d)?;
    let idcc = idcc_de(ctx);
    let branche = match idcc.as_str() {
        "2642" => "la production audiovisuelle (IDCC 2642)",
        "3097" => "la production cinématographique (IDCC 3097)",
        "1790" => "les espaces de loisirs, d'attractions et culturels (IDCC 1790)",
        _ => {
            return Err(refus(format!(
                "Les contributions Afdas de cet état sont celles de la production audiovisuelle (IDCC 2642), de la production cinématographique (IDCC 3097) et des espaces de loisirs (IDCC 1790) ; la fiche entreprise dit IDCC {}.",
                if idcc.is_empty() { "rien".to_string() } else { idcc }
            )))
        }
    };
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut par_an: Vec<(String, Decimal, Decimal)> = Vec::new();
    let zero = zero2();
    // Un permanent est une nature MENSUELLE du catalogue des contrats.
    let mensuelles: Vec<String> = droit::natures::natures().map_err(|m| erreur("ErreurOperation", m))?.iter().filter(|n| n.mensuelle()).map(|n| n.libelle.clone()).collect();
    for l in lignes_livre(ctx, &p, &mut avert)? {
        let Some(brut) = l.brut else { continue };
        let an: String = l.mois.chars().take(4).collect();
        let i = match par_an.iter().position(|(a, _, _)| *a == an) {
            Some(i) => i,
            None => {
                par_an.push((an, zero, zero));
                par_an.len() - 1
            }
        };
        par_an[i].1 = add(par_an[i].1, brut);
        if mensuelles.contains(&l.nature) {
            par_an[i].2 = add(par_an[i].2, brut);
        }
    }
    par_an.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (an, masse, masse_permanents) in par_an {
        let socle = socle_de(ctx, annee_entiere(&an)?)?;
        let hb = socle.get("par_convention").and_then(|c| c.get(idcc.as_str())).and_then(|c| c.get("hors_bulletin")).filter(|v| vrai(v));
        let Some(hb) = hb else {
            return Err(refus(format!("Le barème {} ne porte pas les contributions Afdas de la {} (`par_convention.{}.hors_bulletin`).", an, idcc, idcc)));
        };
        let mut cpnef = zero;
        if let Some(c) = hb.get("cpnef_dialogue_social").filter(|v| vrai(v)) {
            let brut_cpnef = eur2(div(mul(masse, dec_de(&c["taux"])?), Decimal::ONE_HUNDRED));
            cpnef = py_max(dec_de(&c["plancher"])?, py_min(dec_de(&c["plafond"])?, brut_cpnef));
        }
        let effectif = ctx.effectif().ok().and_then(|e| e.valeur(annee_entiere(&an).unwrap_or(0)));
        if effectif.is_none() {
            avert.push(format!("{} : l'effectif de l'année n'est pas connu (retours de nature 118) ; le taux des permanents est celui de la plus petite tranche d'effectif.", an));
        }
        let tranches = hb
            .get("contribution_conventionnelle_permanents")
            .and_then(|c| c.get("par_effectif"))
            .and_then(Value::as_array)
            .ok_or_else(|| erreur("KeyError", socle::fichiers::absent("contribution_conventionnelle_permanents")))?;
        let eff = effectif.unwrap_or(0.0);
        let tranche = tranches
            .iter()
            .find(|x| x["jusqu_a"].is_null() || x["jusqu_a"].as_f64().is_some_and(|j| eff <= j))
            .ok_or_else(|| erreur("StopIteration", ""))?;
        let taux_p = dec_de(&tranche["taux"])?;
        let permanents = eur2(div(mul(masse_permanents, taux_p), Decimal::ONE_HUNDRED));
        let mut l = Ligne::nouvelle();
        l.t("annee", an.clone()).d("masse", masse).d("masse_permanents", masse_permanents).d("cpnef", cpnef).d("taux_permanents", taux_p).d("permanents", permanents);
        l.d("total_ht", add(cpnef, permanents));
        l.t("a_payer", format!("avant le 28/02/{}, à l'Afdas (Globaldec')", annee_entiere(&an)? + 1));
        lignes.push(l);
    }
    if idcc == "1790" {
        avert.push("Montants hors taxes : l'Afdas facture la TVA en plus. Taux de l'avenant n° 52 (article 1.5) : 0,35 % sous 10 salariés, 0,30 % à partir de 10, sur les salaires des CDI et des CDD. La plaquette Afdas « Loisirs 2026 » place le seuil à 11 : l'écart ne touche qu'un effectif d'exactement 10.".to_string());
    } else {
        avert.push("Montants hors taxes : l'Afdas facture la TVA en plus. Le taux de la CPNEF est celui de l'Afdas, qui collecte (0,004 %, 15 à 2 000 € HT) ; la web-conférence Audiens 2026 donne l'inverse — à confirmer sur le premier appel de l'Afdas.".to_string());
    }
    avert.push("La contribution conventionnelle des intermittents (0,10 % et un forfait) n'est pas ici : elle est traitée comme sous la 3090.".to_string());
    let mut t = Tableau::nouveau("afdas-2642", format!("Contributions Afdas de {}", branche), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    let mut cols = vec![col_t("annee", "Année"), col_m("masse", "Masse salariale")];
    if idcc != "1790" {
        cols.push(col_m("cpnef", "CPNEF (dialogue social)"));
    }
    cols.extend([
        col_m("masse_permanents", "Masse hors intermittents"),
        col_x("taux_permanents", "Taux %"),
        col_m("permanents", "Contribution conventionnelle"),
        col_m("total_ht", "Total HT"),
        col_t("a_payer", "À payer"),
    ]);
    t.colonnes = cols;
    t.sources = vec![
        if idcc == "1790" { "CCN 1790, avenant n° 52, article 1.5 ; plaquette Afdas « Loisirs 2026 »" } else { "plaquette Afdas « Contributions 2026 Audiovisuel » (janvier 2026)" }.to_string(),
        "masse salariale : le livre de paie, bulletin par bulletin".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "afdas-2642", &d.format, d.ecrire)?;
    Ok(t)
}

// ===================================================== le paritarisme de branche

/// `operations_etats.paritarisme_branche` : le financement du paritarisme d'une branche qui le collecte une fois
/// l'an, hors DSN, sur la masse salariale de l'annee ecoulee (3252, 3241, et toute couche qui porte
/// `hors_bulletin.financement_paritarisme`). La masse se lit dans le livre de paie.
pub(crate) fn paritarisme_branche(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Le paritarisme de branche est annuel : il se demande pour une année, ou pour toutes.", d)?;
    let idcc = idcc_de(ctx);
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let mut par_an: Vec<(String, Decimal)> = Vec::new();
    let zero = zero2();
    for l in lignes_livre(ctx, &p, &mut avert)? {
        let Some(brut) = l.brut else { continue };
        let an: String = l.mois.chars().take(4).collect();
        match par_an.iter_mut().find(|(a, _)| *a == an) {
            Some(x) => x.1 = add(x.1, brut),
            None => par_an.push((an, add(zero, brut))),
        }
    }
    par_an.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (an, masse) in par_an {
        let socle = socle_de(ctx, annee_entiere(&an)?)?;
        let f = socle.get("par_convention").and_then(|c| c.get(idcc.as_str())).and_then(|c| c.get("hors_bulletin")).and_then(|h| h.get("financement_paritarisme")).filter(|v| vrai(v));
        let Some(f) = f else {
            return Err(refus(format!(
                "La convention {} n'a pas de paritarisme annuel hors DSN dans le barème {} (`par_convention.{}.hors_bulletin.financement_paritarisme`).",
                if idcc.is_empty() { "(non renseignée)" } else { &idcc },
                an,
                idcc
            )));
        };
        let taux = dec_de(&f["taux"])?;
        let mut m = eur2(div(mul(masse, taux), Decimal::ONE_HUNDRED));
        if let Some(pl) = f.get("plancher").filter(|v| !v.is_null()) {
            m = py_max(m, dec_de(pl)?);
        }
        if let Some(pl) = f.get("plafond").filter(|v| !v.is_null()) {
            m = py_min(m, dec_de(pl)?);
        }
        let collecteur = f.get("collecteur").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| "l'organisme collecteur de la branche".to_string());
        if let Some(a) = f.get("avertissement").filter(|v| vrai(v)).map(valeur_en_texte) {
            if !avert.contains(&a) {
                avert.push(a);
            }
        }
        let mut l = Ligne::nouvelle();
        l.t("annee", an.clone()).d("masse", masse).d("taux", taux).d("montant", m).t("a_payer", format!("en {}, à {}", annee_entiere(&an)? + 1, collecteur));
        lignes.push(l);
    }
    avert.push("Assiette : le brut du livre de paie, tous salariés. Le texte parle de masse salariale ou de masse déclarée en retraite complémentaire : pour une compagnie du spectacle, c'est le même brut.".to_string());
    let mut t = Tableau::nouveau("paritarisme-branche", format!("Financement du paritarisme de la convention {}", idcc), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![col_t("annee", "Année de la masse"), col_m("masse", "Masse salariale"), col_x("taux", "Taux %"), col_m("montant", "Contribution"), col_t("a_payer", "À payer")];
    t.sources = vec!["la couche de la convention, `hors_bulletin`".to_string(), "masse salariale : le livre de paie, bulletin par bulletin".to_string()];
    finir(ctx, &mut t, Some(&p), "paritarisme-branche", &d.format, d.ecrire)?;
    Ok(t)
}
