//! Les etats de comptabilite de `exporter` (`moteur/operations_ecritures.py` et, pour la provision de conges et la
//! taxe sur les salaires, `moteur/operations_etats.py`) : les ecritures de paie dans les formats que les logiciels de
//! comptabilite importent (fec, ximport, oxygene, sage, quadra, ebp, pennylane), le grand livre, la balance, la
//! provision pour conges payes des permanents, la taxe sur les salaires.
//!
//! Les ecritures sont celles de `ecritures_comptables::ecritures_du_mois` (le meme plan, le meme refus d'un compte
//! devine) ; chaque mois fait une piece (`PAIE-AAAA-MM`) et un numero d'ecriture. Les fichiers de logiciel sont
//! ecrits en cp1252 avec remplacement (`encode("cp1252", errors="replace")`) quand le format est « Windows ».

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use droit::contexte::{archivee, Contexte};
use socle::valeur::valeur_en_texte;
use socle::echec::erreur;
use crate::etats_paie::{ecarter_declares, ecrire_fini, finir, lignes_livre, charger_mois, Demande, Periode};
use crate::ecritures_comptables::{ecritures_du_mois, Ecriture};
use socle::fichiers::vrai;
use socle::num::{add, div, dec_avec_echelle, eur, eur2, mul, sub, zero2, Decimal};
use crate::tableaux_export::{
    col_m, col_n, col_t, col_x, ecrire as ecrire_tableau, format_possible, jj, libelle_mois, nom_compagnie, refus, totaliser, Ligne, Section, Tableau, Val,
    FORMATS_TABLEAU, R, TOTAL_GENERAL,
};

// ------------------------------------------------------------------ les tables

/// `FORMATS_ECRITURES` : les six formats de la machinerie, puis les sept des logiciels.
pub const FORMATS_ECRITURES: [&str; 13] =
    ["csv", "tsv", "xlsx", "pdf", "html", "json", "fec", "ximport", "oxygene", "sage", "quadra", "ebp", "pennylane"];

const SOURCES: &str = "plan comptable : plan-comptable-base.json (plan comptable général, classes 64, 42, 43, 44), remplacé clé par clé par config/plan-comptable.json";
const SOURCE_FEC: &str = "art. A47 A-1 du Livre des procédures fiscales (Légifrance) et BOI-CF-IOR-60-40-20, relevés le 23/09/2026";
const SOURCE_XIMPORT: &str = "format XIMPORT de Ciel Compta : documentation Gestan (wiki.gestan.fr) et Compta-Online, relevée le 23/09/2026";
const SOURCE_OXYGENE: &str = "Oxygène : aide en ligne de Memsoft, « Structure des fichiers ASCII » (aides.memsoft.fr), relevée le 23/09/2026";
const SOURCE_SAGE: &str = "Sage 100 : « Description du format PNM », Sage, 2022 (fr-kb.sage.com), relevée le 23/09/2026";
const SOURCE_EBP: &str = "EBP : notice « Format d'importation des écritures en ASCII », version du 01/07/2006, reproduite par Sofakom (revendeur EBP), relevée le 23/09/2026";
const SOURCE_PENNYLANE: &str = "Pennylane : centre d'aide, « Importer des écritures » et « Importer un fichier de CEGID EXPERT au format TRA », relevés le 23/09/2026";
const SOURCE_QUADRA: &str = "Quadra : « Fichier d'entrée ASCII dans QuadraCOMPTA », QC_ASC.doc, mise à jour 2023 (Cegid, diffusé par support.sofakom.fr), relevé le 23/09/2026";

const ESSAI: &str = " Faire un premier import d'essai sur un dossier de test avant de s'y fier.";

/// `LOGICIELS[logiciel]` : (nom, format, pourquoi), lus dans `exports_tables.json`.
fn logiciel_connu(logiciel: &str) -> Option<(String, String, String)> {
    let t = crate::plan_comptable::tables();
    t["logiciels"].as_array()?.iter().find(|l| l["logiciel"].as_str() == Some(logiciel)).map(|l| {
        (
            l["nom"].as_str().unwrap_or("").to_string(),
            l["format"].as_str().unwrap_or("").to_string(),
            l["pourquoi"].as_str().unwrap_or("").to_string(),
        )
    })
}

/// Les logiciels connus, dans l'ordre de `LOGICIELS`.
fn logiciels_dits() -> Vec<String> {
    crate::plan_comptable::logiciels_connus().iter().map(|s| s.to_string()).collect()
}

// ------------------------------------------------------------------ petits outils de texte

/// `" ".join(s.split())`.
fn blancs(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `s[:n]`, en caracteres.
fn debut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `s[a:b]`, en caracteres.
fn tranche(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}

/// `s.ljust(n)` / `s.rjust(n)`.
fn caler(s: String, n: usize, a_droite: bool) -> String {
    let manque = n.saturating_sub(s.chars().count());
    let blanc = " ".repeat(manque);
    if a_droite {
        format!("{}{}", blanc, s)
    } else {
        format!("{}{}", s, blanc)
    }
}

/// `_zone` : espaces ramenees a une, coupe a `n`, cadre a gauche (`g`) ou a droite (`d`).
fn zone(v: &str, n: usize, cote: char) -> String {
    caler(debut(&blancs(v), n), n, cote == 'd')
}

/// `"%.2f" % v` d'un montant a deux decimales.
fn deux(v: Decimal) -> String {
    eur2(v).to_string()
}

/// `str(x or "")` d'une valeur JSON.
fn texte_ou_vide(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => valeur_en_texte(x),
        _ => String::new(),
    }
}

/// `str.isdigit()` : non vide, que des chiffres.
fn tout_chiffres(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// `quantize(Decimal(...))` au mode par defaut de Python (arrondi au pair).
fn quant(x: Decimal, dp: u32) -> Decimal {
    let mut r = x.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::MidpointNearestEven);
    r.rescale(dp);
    r
}

/// `"\r\n".join(lignes) + "\r\n"`, en cp1252 : ce que ce jeu ne porte pas devient « ? » (`errors="replace"`).
fn cp1252(lignes: &[String]) -> Vec<u8> {
    let texte = format!("{}\r\n", lignes.join("\r\n"));
    let mut out = Vec::with_capacity(texte.len());
    for c in texte.chars() {
        let n = c as u32;
        let octet = match n {
            0..=0x7F | 0xA0..=0xFF => n as u8,
            0x20AC => 0x80,
            0x201A => 0x82,
            0x0192 => 0x83,
            0x201E => 0x84,
            0x2026 => 0x85,
            0x2020 => 0x86,
            0x2021 => 0x87,
            0x02C6 => 0x88,
            0x2030 => 0x89,
            0x0160 => 0x8A,
            0x2039 => 0x8B,
            0x0152 => 0x8C,
            0x017D => 0x8E,
            0x2018 => 0x91,
            0x2019 => 0x92,
            0x201C => 0x93,
            0x201D => 0x94,
            0x2022 => 0x95,
            0x2013 => 0x96,
            0x2014 => 0x97,
            0x02DC => 0x98,
            0x2122 => 0x99,
            0x0161 => 0x9A,
            0x203A => 0x9B,
            0x0153 => 0x9C,
            0x017E => 0x9E,
            0x0178 => 0x9F,
            _ => b'?',
        };
        out.push(octet);
    }
    out
}

// ------------------------------------------------------------------ les ecritures

/// `_libelle_compte` : le libelle d'un compte, lu dans le plan : tiers, brut, net, PAS, retenues, puis les charges
/// (le premier libelle de ligne qui le porte).
fn libelle_compte(plan: &Map<String, Value>, compte: &str) -> R<String> {
    if let Some(Value::Object(tiers)) = plan.get("tiers").filter(|t| vrai(t)) {
        if let Some(v) = tiers.get(compte) {
            return Ok(valeur_en_texte(v));
        }
    }
    for k in ["brut", "net", "pas"] {
        let vide = Value::Object(Map::new());
        let x = plan.get(k).unwrap_or(&vide);
        let c = x.get("compte").map(valeur_en_texte).unwrap_or_else(|| "None".to_string());
        if c == compte {
            let l = x.get("libelle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("libelle")))?;
            return Ok(valeur_en_texte(l));
        }
    }
    if let Some(Value::Object(retenues)) = plan.get("retenues") {
        for regle in retenues.values() {
            if regle.get("compte").map(valeur_en_texte).as_deref() == Some(compte) {
                return Ok(texte_brut(regle.get("libelle")));
            }
        }
    }
    if let Some(Value::Object(lignes)) = plan.get("lignes") {
        for regle in lignes.values() {
            let charge = regle.get("charge").map(valeur_en_texte).unwrap_or_else(|| "None".to_string());
            if charge == compte {
                let nom = match compte {
                    "645100" => Some("Cotisations Urssaf"),
                    "645300" => Some("Cotisations retraite et prévoyance"),
                    "645400" => Some("Cotisations assurance chômage"),
                    "645500" => Some("Cotisations Congés Spectacles"),
                    "645800" => Some("Autres charges sociales"),
                    _ => None,
                };
                return Ok(match nom {
                    Some(n) => n.to_string(),
                    None => texte_brut(regle.get("libelle")),
                });
            }
        }
    }
    Ok(String::new())
}

/// `regle.get("libelle", "")`.
fn texte_brut(v: Option<&Value>) -> String {
    v.map(valeur_en_texte).unwrap_or_default()
}

/// `_compte` : le sous-compte individuel du salarie quand le plan en tient un, le compte general sinon.
fn compte_de(e: &Ecriture) -> String {
    if e.auxiliaire.is_empty() {
        e.compte.clone()
    } else {
        e.auxiliaire.clone()
    }
}

/// `_libelle`.
fn libelle_de(plan: &Map<String, Value>, e: &Ecriture) -> R<String> {
    let lib = libelle_compte(plan, &e.compte)?;
    Ok(if e.auxiliaire.is_empty() { lib } else { format!("{} — {}", lib, e.auxiliaire_libelle) })
}

/// `_ligne` : une ligne d'ecriture pour un tableau.
fn ligne_de(e: &Ecriture, plan: &Map<String, Value>) -> R<Ligne> {
    let mut l = Ligne::nouvelle();
    l.t("journal", e.journal.clone()).t("date", jj(&e.date)).t("piece", e.piece.clone()).t("compte", compte_de(e));
    l.t("libelle_compte", libelle_de(plan, e)?).t("libelle", e.libelle.clone());
    l.od("debit", if e.debit.is_zero() { None } else { Some(e.debit) });
    l.od("credit", if e.credit.is_zero() { None } else { Some(e.credit) });
    l.t("analytique", e.analytique.clone()).t("mois", e.mois.clone());
    Ok(l)
}

/// `_ecritures` : les ecritures de la periode, numerotees -- une ecriture (une piece) par mois, ses lignes dans
/// l'ordre de `ecritures_du_mois`.
fn ecritures_de(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>) -> R<(Vec<Ecriture>, Map<String, Value>, &'static str)> {
    let (plan, origine) = crate::plan_comptable::plan(ctx)?;
    let mut out = Vec::new();
    let mois = ecarter_declares(p, avert, "la répartition des cotisations — donc pas d'écriture équilibrée")?;
    for (n, m) in mois.iter().enumerate() {
        for mut e in ecritures_du_mois(ctx, m, &plan)? {
            e.piece = format!("PAIE-{}", m);
            e.numero = n as i64 + 1;
            out.push(e);
        }
    }
    if !ctx.racine.join("config").join("plan-comptable.json").exists() {
        avert.push("Plan comptable : celui livré avec le logiciel, à faire valider par le comptable (config/plan-comptable.json pour le remplacer).".to_string());
    }
    Ok((out, plan, origine))
}

// ------------------------------------------------------------------ ce que chaque ecrivain recoit

/// `Envoi` : le plan, le contexte, et les codes de section quand l'analytique est demandee.
struct Envoi<'a> {
    plan: &'a Map<String, Value>,
    ctx: &'a Contexte,
    sections: Option<BTreeMap<String, String>>,
}

impl Envoi<'_> {
    /// `Envoi.section` : le code de section d'une ligne -- seules les lignes de salaires bruts en ont une.
    fn section(&self, e: &Ecriture) -> String {
        let Some(sections) = &self.sections else { return String::new() };
        let brut = self.plan.get("brut").and_then(|b| b.get("compte")).map(valeur_en_texte).unwrap_or_default();
        if e.compte != brut {
            return String::new();
        }
        sections.get(&e.analytique).cloned().unwrap_or_default()
    }
}

/// `_code` : un titre en code -- lettres et chiffres, sans accent, en majuscules, coupe a la largeur de la zone.
fn code_du_titre(titre: &str, n: usize) -> String {
    let ascii = socle::texte::ascii_nfkd(titre);
    debut(&ascii.to_uppercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>(), n)
}

/// `codes_des_fiches` : titre -> code analytique, lu sur les fiches spectacle. La fiche active l'emporte, puis le
/// numero le plus recent.
fn codes_des_fiches(spectacles: &Map<String, Value>) -> BTreeMap<String, String> {
    let mut rangees: Vec<(&String, &Value)> = spectacles.iter().filter(|(_, v)| v.is_object()).collect();
    rangees.sort_by_cached_key(|(_, v)| (!archivee(v), texte_ou_vide(v.get("numero_objet"))));
    let mut out = BTreeMap::new();
    for (_, v) in rangees {
        let code = texte_ou_vide(v.get("code_analytique")).trim().to_string();
        if !code.is_empty() && v.get("titre").is_some_and(vrai) {
            out.insert(valeur_en_texte(&v["titre"]), code);
        }
    }
    out
}

/// `codes_sections` : le code de chaque spectacle -- celui de sa fiche, sinon celui du plan, sinon tire du titre.
fn codes_sections(plan: &Map<String, Value>, ecritures: &[Ecriture], largeur: usize, fiches: &BTreeMap<String, String>) -> R<BTreeMap<String, String>> {
    let brut = plan.get("brut").and_then(|b| b.get("compte")).map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("brut")))?;
    let mut propres: BTreeMap<String, String> = BTreeMap::new();
    if let Some(Value::Object(s)) = plan.get("sections") {
        for (k, v) in s {
            propres.insert(k.clone(), texte_ou_vide(Some(v)));
        }
    }
    for (k, v) in fiches {
        propres.insert(k.clone(), v.clone());
    }
    let mut out = BTreeMap::new();
    let mut qui: BTreeMap<String, String> = BTreeMap::new();
    let titres: BTreeSet<&String> = ecritures.iter().filter(|e| !e.analytique.is_empty() && e.compte == brut).map(|e| &e.analytique).collect();
    for titre in titres {
        let choisi = propres.get(titre).cloned().unwrap_or_default();
        let code = if choisi.is_empty() { code_du_titre(titre, largeur) } else { choisi.clone() };
        if code.is_empty() || code.chars().count() > largeur {
            return Err(refus(format!(
                "Analytique : le code « {} » du spectacle « {} » ne tient pas dans la zone de ce logiciel ({} caractères au plus). Le raccourcir dans Configuration › Spectacles, case « Code analytique ».",
                code, titre, largeur
            )));
        }
        if let Some(autre) = qui.get(&code) {
            let meme_choix = !choisi.is_empty() && propres.get(autre).cloned().unwrap_or_default() == code;
            if !meme_choix {
                return Err(refus(format!(
                    "Analytique : « {} » et « {} » donnent le même code « {} ». En donner un à chacun dans Configuration › Spectacles, case « Code analytique ».",
                    autre, titre, code
                )));
            }
        }
        out.insert(titre.clone(), code.clone());
        qui.insert(code, titre.clone());
    }
    Ok(out)
}

/// `comptes_utilises` : (compte, libelle) de chaque compte que les ecritures mouvementent, dans l'ordre des numeros.
fn comptes_utilises(ecritures: &[Ecriture], plan: &Map<String, Value>) -> R<Vec<(String, String)>> {
    let mut vus: BTreeMap<String, String> = BTreeMap::new();
    for e in ecritures {
        let c = compte_de(e);
        if !vus.contains_key(&c) {
            let lib = libelle_de(plan, e)?;
            let lib = if lib.is_empty() { e.libelle.split(" -- ").next().unwrap_or("").to_string() } else { lib };
            vus.insert(c, lib);
        }
    }
    Ok(vus.into_iter().collect())
}

// ------------------------------------------------------------------ FEC

const ZONES_FEC: [&str; 18] = [
    "JournalCode", "JournalLib", "EcritureNum", "EcritureDate", "CompteNum", "CompteLib", "CompAuxNum", "CompAuxLib", "PieceRef", "PieceDate", "EcritureLib", "Debit",
    "Credit", "EcritureLet", "DateLet", "ValidDate", "Montantdevise", "Idevise",
];

/// `_fec_propre` : ni tabulation ni fin de ligne dans une zone.
fn fec_propre(s: &str) -> String {
    blancs(&s.replace('\t', " "))
}

/// `fec` : l'extrait du journal de paie aux 18 champs, UTF-8, tabulations, CRLF.
fn fec(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let plan = env.plan;
    let mut lignes = vec![ZONES_FEC.join("\t")];
    let journal_lib = match plan.get("journal_libelle").filter(|v| vrai(v)) {
        Some(v) => valeur_en_texte(v),
        None => "Opérations diverses — paie".to_string(),
    };
    for e in ecritures {
        let date = e.date.replace('-', "");
        let lib = if e.analytique.is_empty() { e.libelle.clone() } else { format!("{} — {}", e.libelle, e.analytique) };
        let champs = [
            e.journal.clone(),
            journal_lib.clone(),
            format!("{}{:05}", debut(&e.date, 4), e.numero),
            date.clone(),
            e.compte.clone(),
            libelle_compte(plan, &e.compte)?,
            e.auxiliaire.clone(),
            e.auxiliaire_libelle.clone(),
            e.piece.clone(),
            date,
            lib,
            deux(e.debit).replace('.', ","),
            deux(e.credit).replace('.', ","),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        lignes.push(champs.iter().map(|c| fec_propre(c)).collect::<Vec<_>>().join("\t"));
    }
    Ok(format!("{}\r\n", lignes.join("\r\n")).into_bytes())
}

// ------------------------------------------------------------------ outils communs

/// `_libelle_court` : le libelle d'une ligne dans une zone etroite -- ce qui la distingue (le spectacle ou le
/// salarie, sinon la nature ; les deux si `nature`) et le mois, qui reste entier.
fn libelle_court(e: &Ecriture, n: usize, nature: bool) -> String {
    let mm = format!("{}/{}", tranche(&e.date, 5, 7), debut(&e.date, 4));
    let sans_mois = match e.libelle.rfind(" -- ") {
        Some(i) => &e.libelle[..i],
        None => e.libelle.as_str(),
    };
    let mut quoi = sans_mois.split(" -- ").collect::<Vec<_>>().join(" ");
    if nature && !e.analytique.is_empty() {
        quoi = format!("{} — {}", quoi, e.analytique);
    }
    let tete_brute = if !e.analytique.is_empty() && !nature { e.analytique.clone() } else { quoi };
    let lib = blancs(&format!("{} {}", tete_brute, mm));
    if lib.chars().count() <= n {
        return lib;
    }
    let place = (n as i64 - mm.chars().count() as i64 - 1).max(0) as usize;
    let tete: Vec<char> = lib.chars().take(place + 1).collect();
    let coupe = tete.iter().rposition(|c| *c == ' ');
    let tete: String = match coupe {
        Some(c) if c >= place / 2 => tete[..c].iter().collect(),
        _ => lib.chars().take(place).collect(),
    };
    format!("{} {}", tete.trim_end_matches([' ', '—', '-']), mm)
}

/// `_piece_courte` : PAIE-2026-08 fait 12 caracteres ; Oxygene et Quadra en prennent 10.
fn piece_courte(e: &Ecriture) -> String {
    debut(&format!("PAIE{}", e.mois.replace('-', "")), 10)
}

/// `_sens`.
fn sens(e: &Ecriture) -> (&'static str, Decimal) {
    if e.debit.is_zero() {
        ("C", e.credit)
    } else {
        ("D", e.debit)
    }
}

/// `_jjmmaa`.
fn jjmmaa(date: &str) -> String {
    format!("{}{}{}", tranche(date, 8, 10), tranche(date, 5, 7), tranche(date, 2, 4))
}

/// `_csv_propre` : ni point-virgule ni fin de ligne dans une zone.
fn csv_propre(s: &str) -> String {
    blancs(&s.replace(';', ","))
}

/// `_virgule`.
fn virgule(v: Decimal) -> String {
    if v.is_zero() {
        String::new()
    } else {
        deux(v).replace('.', ",")
    }
}

/// `LIMITES[format]` : (nom, largeur du compte, largeur du journal).
fn limites(format: &str) -> Option<(String, usize, usize)> {
    let l = crate::plan_comptable::tables()["limites"].get(format)?;
    Some((l["nom"].as_str()?.to_string(), l["compte"].as_u64()? as usize, l["journal"].as_u64()? as usize))
}

/// `_borne` : le compte et le journal tiennent dans leurs zones, ou on refuse.
fn borne(format: &str, compte: &str, journal: &str) -> R<()> {
    let (nom, lc, lj) = limites(format).ok_or_else(|| erreur("KeyError", format!("format inconnu : {}", socle::fichiers::repr(format))))?;
    if compte.chars().count() > lc || journal.chars().count() > lj {
        return Err(refus(format!("{} : le compte « {} » ou le journal « {} » dépasse sa zone ({} et {} caractères).", nom, compte, journal, lc, lj)));
    }
    Ok(())
}

// ------------------------------------------------------------------ XIMPORT

const LARGEURS_XIMPORT: [(&str, usize, char); 13] = [
    ("mouvement", 5, 'd'), ("journal", 2, 'g'), ("date", 8, 'g'), ("echeance", 8, 'g'), ("piece", 12, 'g'), ("compte", 11, 'g'), ("libelle", 25, 'g'), ("montant", 13, 'd'),
    ("sens", 1, 'g'), ("pointage", 12, 'g'), ("analytique", 6, 'g'), ("libelle_compte", 34, 'g'), ("euro", 1, 'g'),
];

/// `ximport` : une ligne par ligne d'ecriture, a largeurs fixes, cp1252.
fn ximport(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut out = Vec::new();
    for e in ecritures {
        let (s, montant) = sens(e);
        let champs: BTreeMap<&str, String> = [
            ("mouvement", e.numero.to_string()),
            ("journal", debut(&e.journal, 2)),
            ("date", e.date.replace('-', "")),
            ("echeance", String::new()),
            ("piece", e.piece.clone()),
            ("compte", compte_de(e)),
            ("libelle", libelle_court(e, 25, false)),
            ("montant", deux(montant).replace('.', ",")),
            ("sens", s.to_string()),
            ("pointage", String::new()),
            ("analytique", env.section(e)),
            ("libelle_compte", libelle_de(env.plan, e)?),
            ("euro", "O".to_string()),
        ]
        .into_iter()
        .collect();
        out.push(LARGEURS_XIMPORT.iter().map(|(k, n, c)| zone(&champs[k], *n, *c)).collect::<String>());
    }
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Oxygene

fn zones_oxygene() -> Vec<String> {
    let mut z: Vec<String> =
        ["CODSOC", "CODAGE", "NUMJL", "MONNAIE_IDENT", "NUMCP", "COURS", "CSECT", "DTOPE", "NPIEC", "LIBEC", "MTDEB", "MTCRE", "RELET", "NIPIE", "LIBAUTO"].iter().map(|s| s.to_string()).collect();
    for i in 1..=5 {
        for p in ["CB", "CR", "DE", "ME"] {
            z.push(format!("{}{}", p, i));
        }
    }
    z.push("NIDENT".to_string());
    z.push("NUMCP2".to_string());
    z
}

/// `oxygene` : une ligne par ligne d'ecriture ; NPIEC regroupe les lignes d'une meme piece, le mois.
fn oxygene(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let zones = zones_oxygene();
    let mut out = vec![zones.join(";")];
    for e in ecritures {
        borne("oxygene", &compte_de(e), &e.journal)?;
        let section = env.section(e);
        let mut z: BTreeMap<&str, String> = zones.iter().map(|k| (k.as_str(), String::new())).collect();
        let poste = if section.is_empty() { String::new() } else { texte_ou_vide(env.plan.get("poste_analytique")) };
        for (k, v) in [
            ("NUMJL", e.journal.clone()),
            ("NUMCP", compte_de(e)),
            ("DTOPE", e.date.replace('-', "")),
            ("NPIEC", piece_courte(e)),
            ("LIBEC", libelle_court(e, 40, true)),
            ("MTDEB", virgule(e.debit)),
            ("MTCRE", virgule(e.credit)),
            ("CSECT", section),
            ("CODAGE", poste),
        ] {
            z.insert(k, v);
        }
        out.push(zones.iter().map(|k| csv_propre(&z[k.as_str()])).collect::<Vec<_>>().join(";"));
    }
    Ok(cp1252(&out))
}

/// `oxygene_comptes` : « L'importation du plan comptable », fiche compte.
fn oxygene_comptes(comptes: &[(String, String)]) -> Vec<u8> {
    let mut out = vec!["NUMCP;CODCP;LIBCP".to_string()];
    for (compte, lib) in comptes {
        out.push([csv_propre(compte), csv_propre(&debut(lib, 15)), csv_propre(&debut(lib, 50))].join(";"));
    }
    cp1252(&out)
}

// ------------------------------------------------------------------ Sage (PNM)

const LARGEURS_PNM: [(&str, usize, char); 13] = [
    ("journal", 3, 'g'), ("date", 6, 'g'), ("type_piece", 2, 'g'), ("compte", 13, 'g'), ("type_compte", 1, 'g'), ("auxiliaire", 13, 'g'), ("reference", 13, 'g'),
    ("libelle", 25, 'g'), ("paiement", 1, 'g'), ("echeance", 6, 'g'), ("sens", 1, 'g'), ("montant", 20, 'd'), ("type_ecriture", 1, 'g'),
];

/// `sage_pnm` : le format 105 caracteres. Avec l'analytique, la ligne generale est suivie d'une ligne de type A.
fn sage_pnm(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let nom = nom_compagnie(env.ctx);
    let nom = if nom.is_empty() { "Paie".to_string() } else { nom };
    let mut out = vec![zone(&nom, 30, 'g').trim_end().to_string()];
    for e in ecritures {
        borne("sage", &compte_de(e), &e.journal)?;
        let (s, montant) = sens(e);
        let mut champs: BTreeMap<&str, String> = [
            ("journal", e.journal.clone()),
            ("date", jjmmaa(&e.date)),
            ("type_piece", "OD".to_string()),
            ("compte", compte_de(e)),
            ("type_compte", String::new()),
            ("auxiliaire", String::new()),
            ("reference", e.piece.clone()),
            ("libelle", libelle_court(e, 25, false)),
            ("paiement", String::new()),
            ("echeance", String::new()),
            ("sens", s.to_string()),
            ("montant", deux(montant)),
            ("type_ecriture", "N".to_string()),
        ]
        .into_iter()
        .collect();
        let ligne = |c: &BTreeMap<&str, String>| LARGEURS_PNM.iter().map(|(k, n, cote)| zone(&c[k], *n, *cote)).collect::<String>();
        out.push(ligne(&champs));
        let section = env.section(e);
        if !section.is_empty() {
            champs.insert("type_compte", "A".to_string());
            champs.insert("auxiliaire", section);
            out.push(ligne(&champs));
        }
    }
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Quadra

/// L'enregistrement de compte : (cle, longueur) -- les positions se suivent sans trou (QC_ASC.doc).
const POSITIONS_QUADRA: [(&str, usize); 23] = [
    ("type", 1), ("compte", 8), ("journal", 2), ("folio", 3), ("date", 6), ("code_libelle", 1), ("libelle", 20), ("sens", 1), ("montant", 13), ("contrepartie", 8),
    ("echeance", 6), ("lettrage", 2), ("statistiques", 3), ("piece5", 5), ("affaire", 10), ("quantite", 10), ("piece8", 8), ("devise", 3), ("journal3", 3), ("tva", 3),
    ("libelle30", 30), ("tva2", 2), ("piece10", 10),
];
const POSITIONS_QUADRA_COMPTE: [(&str, usize); 12] = [
    ("type", 1), ("compte", 8), ("libelle", 30), ("cle", 7), ("debit1", 13), ("credit1", 13), ("debit2", 13), ("credit2", 13), ("collectif", 8), ("adresse", 110), ("maj", 1),
    ("type_compte", 1),
];

/// `_compte_quadra` : huit caracteres, pas un de plus ; un compte de chiffres plus court est complete de zeros.
fn compte_quadra(compte: &str) -> R<String> {
    if compte.chars().count() > 8 {
        return Err(refus(format!(
            "Quadra : le compte « {} » a plus de 8 caractères, la largeur de la zone. Choisir dans config/plan-comptable.json un sous-compte qui tient (par exemple « 421{{code:03}} » plutôt que « 421{{cle}} »).",
            compte
        )));
    }
    Ok(if tout_chiffres(compte) { caler_zeros(compte, 8) } else { compte.to_string() })
}

/// `compte.ljust(8, "0")`.
fn caler_zeros(compte: &str, n: usize) -> String {
    let mut s = compte.to_string();
    while s.chars().count() < n {
        s.push('0');
    }
    s
}

/// `_quadra_ligne` : les zones a la suite, cadrees a gauche.
fn quadra_ligne(champs: &BTreeMap<&str, String>, positions: &[(&str, usize)]) -> String {
    positions.iter().map(|(k, n)| zone(champs.get(k).map(String::as_str).unwrap_or(""), *n, 'g')).collect()
}

/// `quadra` : les comptes d'abord (type C), puis les ecritures (enregistrements M).
fn quadra(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut sortie = Vec::new();
    for (c, lib) in comptes_utilises(ecritures, env.plan)? {
        let mut ch: BTreeMap<&str, String> = BTreeMap::new();
        ch.insert("type", "C".to_string());
        ch.insert("compte", compte_quadra(&c)?);
        ch.insert("libelle", lib);
        for k in ["debit1", "credit1", "debit2", "credit2"] {
            ch.insert(k, "+000000000000".to_string());
        }
        ch.insert("type_compte", "G".to_string());
        sortie.push(quadra_ligne(&ch, &POSITIONS_QUADRA_COMPTE));
    }
    for e in ecritures {
        let (s, montant) = sens(e);
        let centimes = eur2(montant).mantissa();
        let mut ch: BTreeMap<&str, String> = BTreeMap::new();
        ch.insert("type", "M".to_string());
        ch.insert("compte", compte_quadra(&compte_de(e))?);
        ch.insert("journal", debut(&e.journal, 2));
        ch.insert("folio", "000".to_string());
        ch.insert("date", jjmmaa(&e.date));
        ch.insert("libelle", libelle_court(e, 20, false));
        ch.insert("sens", s.to_string());
        ch.insert("montant", format!("+{:012}", centimes));
        ch.insert("affaire", env.section(e));
        ch.insert("devise", "EUR".to_string());
        ch.insert("journal3", debut(&e.journal, 3));
        ch.insert("libelle30", libelle_court(e, 30, false));
        ch.insert("piece10", piece_courte(e));
        sortie.push(quadra_ligne(&ch, &POSITIONS_QUADRA));
    }
    Ok(cp1252(&sortie))
}

// ------------------------------------------------------------------ EBP

/// `_ebp_texte` : entre guillemets, sans guillemet dedans.
fn ebp_texte(s: &str, n: usize) -> String {
    format!("\"{}\"", debut(&blancs(&s.replace('"', "'")), n))
}

/// `_ebp_zone` : une zone sans guillemets ne contient pas de virgule.
fn ebp_zone(s: &str) -> String {
    blancs(&s.replace(',', " ").replace('"', "'"))
}

/// `ebp` : toutes les zones, meme vides ; avec l'analytique, la ligne de ventilation suit l'ecriture.
fn ebp(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut out = Vec::new();
    for (i, e) in ecritures.iter().enumerate() {
        borne("ebp", &compte_de(e), &e.journal)?;
        let (s, montant) = sens(e);
        out.push(
            [
                (i + 1).to_string(),
                jjmmaa(&e.date),
                e.journal.clone(),
                compte_de(e),
                String::new(),
                ebp_texte(&libelle_court(e, 40, true), 40),
                ebp_texte(&e.piece, 15),
                deux(montant),
                s.to_string(),
                String::new(),
                String::new(),
            ]
            .join(","),
        );
        let section = env.section(e);
        if !section.is_empty() {
            out.push(format!(">{},100.00,{}", section, deux(montant)));
        }
    }
    Ok(cp1252(&out))
}

/// `ebp_comptes` : COMPTES.TXT.
fn ebp_comptes(comptes: &[(String, String)]) -> Vec<u8> {
    let out: Vec<String> = comptes
        .iter()
        .map(|(c, lib)| {
            let mut z = vec![debut(&ebp_zone(c), 15), debut(&ebp_zone(lib), 60)];
            z.extend(std::iter::repeat(String::new()).take(8));
            z.join(",")
        })
        .collect();
    cp1252(&out)
}

// ------------------------------------------------------------------ Pennylane

/// `pennylane` : le code journal n'a que des lettres (5 au plus). UTF-8, point-virgule.
fn pennylane(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut out = vec!["Date;Code journal;Numéro de compte;Libellé;Débit;Crédit;Code analytique".to_string()];
    for e in ecritures {
        if !(!e.journal.is_empty() && e.journal.chars().all(char::is_alphabetic) && e.journal.chars().count() <= 5) {
            return Err(refus(format!("Pennylane : le code journal « {} » doit tenir en 5 lettres.", e.journal)));
        }
        let champs = [
            jj(&e.date),
            e.journal.clone(),
            compte_de(e),
            format!("{} {}", e.piece, libelle_court(e, 80, true)),
            virgule(e.debit),
            virgule(e.credit),
            env.section(e),
        ];
        out.push(champs.iter().map(|c| csv_propre(c)).collect::<Vec<_>>().join(";"));
    }
    Ok(format!("{}\r\n", out.join("\r\n")).into_bytes())
}

// ------------------------------------------------------------------ les formats de logiciel

/// `Ecrivain` : le fichier, l'ecrivain, la source, ce qu'on dit a celui qui l'importe, la largeur du code de
/// section quand le format porte l'analytique, et le fichier des comptes quand le format en a un.
struct Ecrivain {
    fichier: &'static str,
    ecrire: fn(&[Ecriture], &Envoi) -> R<Vec<u8>>,
    source: &'static str,
    conseil: String,
    section: usize,
    /// (modele du nom, ecrivain, conseil)
    comptes: Option<(&'static str, fn(&[(String, String)]) -> Vec<u8>, &'static str)>,
}

/// `ECRIVAINS.get(format)`.
fn ecrivain(format: &str) -> Option<Ecrivain> {
    Some(match format {
        "fec" => Ecrivain {
            fichier: "extrait-fec-paie-%s.txt",
            ecrire: fec,
            source: SOURCE_FEC,
            conseil: "Extrait du journal de paie à la forme du FEC, à importer dans la comptabilité — ce n'est pas le FEC de l'entreprise, que seul le logiciel de comptabilité produit. ValidDate est vide : la validation se fait à l'import.".to_string(),
            section: 0,
            comptes: None,
        },
        "ximport" => Ecrivain {
            fichier: "XIMPORT-paie-%s.TXT",
            ecrire: ximport,
            source: SOURCE_XIMPORT,
            conseil: format!("XIMPORT : format relevé dans une documentation tierce, pas chez l'éditeur.{}", ESSAI),
            section: 6,
            comptes: None,
        },
        "oxygene" => Ecrivain {
            fichier: "oxygene-ecritures-paie-%s.csv",
            ecrire: oxygene,
            source: SOURCE_OXYGENE,
            conseil: format!(
                "Oxygène : importation de données, « Import écritures à partir d'1 seul fichier », format EXCEL2. L'aide de Memsoft ne dit ni le séparateur décimal ni le jeu de caractères : virgule et Windows ont été retenus.{}",
                ESSAI
            ),
            section: 20,
            comptes: Some(("oxygene-plan-comptable-paie-%s.csv", oxygene_comptes, "à importer d'abord, « Import plan comptable » ; un compte qui existe déjà n'a pas à y être.")),
        },
        "sage" => Ecrivain {
            fichier: "ecritures-paie-%s.pnm",
            ecrire: sage_pnm,
            source: SOURCE_SAGE,
            conseil: format!(
                "Sage 100 : Fichier > Importer, type « Écritures Sage (*.pnm) ». Journal et comptes doivent exister dans la société ; le montant est écrit au point décimal, cadré à droite.{}",
                ESSAI
            ),
            section: 13,
            comptes: None,
        },
        "quadra" => Ecrivain {
            fichier: "quadra-ecritures-paie-%s.txt",
            ecrire: quadra,
            source: SOURCE_QUADRA,
            conseil: format!(
                "Quadra : Suivi des dossiers > Import ASCII (Compta). Les comptes utilisés sont décrits en tête (sans toucher à un compte existant) ; les comptes de chiffres sont complétés à 8 (641100 → 64110000) ; montants en centimes signés, folio 000.{}",
                ESSAI
            ),
            section: 10,
            comptes: None,
        },
        "ebp" => Ecrivain {
            fichier: "ECRITURES-paie-%s.TXT",
            ecrire: ebp,
            source: SOURCE_EBP,
            conseil: format!("EBP : Outils > Import de données > Import écritures, format « EBP V3 pour Windows ».{}", ESSAI),
            section: 10,
            comptes: Some(("COMPTES.TXT", ebp_comptes, "EBP le lit seul s'il est dans le même dossier que le fichier des écritures, et crée les comptes absents avec leur intitulé.")),
        },
        "pennylane" => Ecrivain {
            fichier: "pennylane-ecritures-paie-%s.csv",
            ecrire: pennylane,
            source: SOURCE_PENNYLANE,
            conseil: format!(
                "Pennylane : Saisie > Imports > Importer un fichier > Écritures ; faire correspondre les colonnes au premier import et enregistrer le modèle. Le 421 individuel est accepté en alphanumérique.{}",
                ESSAI
            ),
            section: 20,
            comptes: None,
        },
        _ => return None,
    })
}

/// `choix_compta` : le logiciel du comptable et l'analytique, tels que retenus (`config/compta.json`).
fn choix_compta(ctx: &Contexte) -> R<Value> {
    let chemin = ctx.racine.join("config").join("compta.json");
    if chemin.exists() {
        socle::fichiers::lire_json_echec(&chemin)
    } else {
        Ok(Value::Object(Map::new()))
    }
}

// ------------------------------------------------------------------ le journal de paie

/// `ecritures_comptables` : le journal de paie, une piece par mois, equilibree, dans le format choisi -- tableau
/// (CSV, texte tabule, XLSX, PDF, HTML, JSON), ou celui d'un logiciel de comptabilite.
pub fn ecritures_comptables(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let retenu = choix_compta(ctx)?;
    let mut format = d.format.clone();
    let mut logiciel = d.logiciel.clone().filter(|l| !l.is_empty());
    if logiciel.is_none() && format.is_empty() {
        logiciel = retenu.get("logiciel").filter(|v| vrai(v)).map(valeur_en_texte);
    }
    let analytique = match d.analytique {
        Some(a) => a,
        None => retenu.get("analytique").is_some_and(vrai),
    };
    if let Some(l) = &logiciel {
        let Some((nom, fmt, pourquoi)) = logiciel_connu(l) else {
            return Err(refus(format!("Logiciel inconnu : « {} ». Les logiciels connus sont {}.", l, logiciels_dits().join(", "))));
        };
        avert.push(format!("{} : format « {} » — {}.", nom, fmt, pourquoi));
        format = fmt;
    }
    if format.is_empty() {
        format = "csv".to_string();
    }
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert)?;
    let mut t = Tableau::nouveau("ecritures-comptables", "Journal de paie — écritures comptables", p.libelle.clone());
    t.mois = p.saisis();
    t.avertissements = avert;
    let mois: BTreeSet<&String> = ecr.iter().map(|e| &e.mois).collect();
    for m in mois {
        let mut lignes = Vec::new();
        for e in ecr.iter().filter(|e| &e.mois == m) {
            lignes.push(ligne_de(e, &plan)?);
        }
        t.sections.push(Section::nouvelle(libelle_mois(m)?, lignes));
    }
    t.colonnes = vec![
        col_t("journal", "Journal"),
        col_t("date", "Date"),
        col_t("piece", "N° pièce"),
        col_t("compte", "N° compte"),
        col_t("libelle", "Libellé"),
        col_m("debit", "Débit"),
        col_m("credit", "Crédit"),
        col_t("analytique", "Analytique"),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    let ecr_format = ecrivain(&format);
    if let Some(ec) = &ecr_format {
        t.sources.push(ec.source.to_string());
        t.avertissements.push(ec.conseil.clone());
    }
    t.format = format_possible(&format, &FORMATS_ECRITURES)?;
    totaliser(&mut t, TOTAL_GENERAL)?;
    let total = t.total.clone().unwrap_or_default();
    let zero = zero2();
    let (debit, credit) = (total.decimal("debit").unwrap_or(zero), total.decimal("credit").unwrap_or(zero));
    let debit = if debit.is_zero() { zero } else { debit };
    let credit = if credit.is_zero() { zero } else { credit };
    if debit != credit {
        return Err(refus(format!("Le journal n'est pas équilibré (débit {}, crédit {}).", debit, credit)));
    }
    let mut sections = None;
    if analytique {
        if let Some(ec) = ecr_format.as_ref().filter(|e| e.section > 0) {
            let codes = codes_sections(&plan, &ecr, ec.section, &codes_des_fiches(&ctx.spectacles))?;
            let dits: Vec<String> = codes.iter().map(|(titre, code)| format!("{} = {}", code, titre)).collect();
            t.avertissements.push(format!(
                "Analytique : les salaires bruts sont ventilés par spectacle ({}). Les sections doivent exister dans le logiciel du comptable ; leurs codes se changent dans Configuration › Spectacles, case « Code analytique ».",
                dits.join(", ")
            ));
            sections = Some(codes);
        } else if ecr_format.is_some() {
            t.avertissements.push(format!("Analytique : le format « {} » ne la porte pas ; les écritures sortent sans ventilation.", t.format));
        }
    }
    if d.ecrire {
        let dossier = p.dossier(ctx);
        if let Some(ec) = &ecr_format {
            let io = |c: &std::path::Path, e: std::io::Error| erreur("OSError", format!("{} : {}", c.display(), e));
            std::fs::create_dir_all(&dossier).map_err(|e| io(&dossier, e))?;
            let env = Envoi { plan: &plan, ctx, sections };
            let chemin = dossier.join(ec.fichier.replace("%s", &p.nom()));
            std::fs::write(&chemin, (ec.ecrire)(&ecr, &env)?).map_err(|e| io(&chemin, e))?;
            t.fichier = Some(chemin);
            if let Some((modele, fabrique, conseil)) = ec.comptes {
                // COMPTES.TXT n'a pas de periode dans son nom : EBP le cherche sous ce nom-la, a cote des ecritures.
                let nom_comptes = if modele.contains("%s") { modele.replace("%s", &p.nom()) } else { modele.to_string() };
                let cible = dossier.join(&nom_comptes);
                std::fs::write(&cible, fabrique(&comptes_utilises(&ecr, &plan)?)).map_err(|e| io(&cible, e))?;
                t.avertissements.push(format!("Les comptes et leurs libellés sont dans « {} », à côté : {}", nom_comptes, conseil));
            }
        } else {
            ecrire_tableau(&mut t, &dossier, &format!("ecritures-comptables-{}", p.nom()), ctx)?;
        }
    }
    Ok(t)
}

// ------------------------------------------------------------------ le grand livre

/// `_solde`.
fn solde(d: &Ligne) -> String {
    let zero = zero2();
    let v = sub(d.decimal("debit").unwrap_or(zero), d.decimal("credit").unwrap_or(zero));
    let t = if v > Decimal::ZERO {
        format!("débiteur {}", v)
    } else if v < Decimal::ZERO {
        format!("créditeur {}", -v)
    } else {
        "nul".to_string()
    };
    t.replace('.', ",")
}

/// `grand_livre` : les ecritures de paie rangees par compte, un sous-total par compte.
pub fn grand_livre(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert)?;
    let mut par: BTreeMap<String, Vec<Ligne>> = BTreeMap::new();
    let mut libelles: BTreeMap<String, String> = BTreeMap::new();
    for e in &ecr {
        par.entry(compte_de(e)).or_default().push(ligne_de(e, &plan)?);
        libelles.insert(compte_de(e), libelle_de(&plan, e)?);
    }
    let mut t = Tableau::nouveau("grand-livre-paie", "Grand livre des comptes de paie", p.libelle.clone());
    t.mois = p.saisis();
    t.avertissements = avert;
    t.sections = par.into_iter().map(|(c, l)| Section::nouvelle(format!("{} {}", c, libelles[&c]), l)).collect();
    t.colonnes = vec![
        col_t("compte", "Compte"),
        col_t("date", "Date"),
        col_t("piece", "Pièce"),
        col_t("libelle", "Libellé"),
        col_t("analytique", "Analytique"),
        col_m("debit", "Débit"),
        col_m("credit", "Crédit"),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    t.format = format_possible(&d.format, &FORMATS_TABLEAU)?;
    totaliser(&mut t, TOTAL_GENERAL)?;
    for s in t.sections.iter_mut() {
        if let Some(tot) = s.total.as_mut() {
            let dit = format!("solde {}", solde(tot));
            tot.t("analytique", dit);
        }
    }
    if d.ecrire {
        ecrire_tableau(&mut t, &p.dossier(ctx), &format!("grand-livre-paie-{}", p.nom()), ctx)?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la balance

/// `balance` : une ligne par compte -- total debit, total credit, solde. Le total general tombe a zero.
pub fn balance(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert)?;
    let zero = zero2();
    // compte -> (libelle, debit, credit)
    let mut par: BTreeMap<String, (String, Decimal, Decimal)> = BTreeMap::new();
    for e in &ecr {
        let c = compte_de(e);
        if !par.contains_key(&c) {
            par.insert(c.clone(), (libelle_de(&plan, e)?, zero, zero));
        }
        let x = par.get_mut(&c).expect("insere");
        x.1 = add(x.1, e.debit);
        x.2 = add(x.2, e.credit);
    }
    let mut lignes = Vec::new();
    for (c, (lib, debit, credit)) in &par {
        let v = sub(*debit, *credit);
        let mut l = Ligne::nouvelle();
        l.t("compte", c.clone()).t("libelle", lib.clone()).d("debit", *debit).d("credit", *credit);
        l.od("solde_debiteur", if v > Decimal::ZERO { Some(v) } else { None });
        l.od("solde_crediteur", if v < Decimal::ZERO { Some(-v) } else { None });
        lignes.push(l);
    }
    let mut t = Tableau::nouveau("balance-paie", "Balance des comptes de paie", p.libelle.clone());
    t.mois = p.saisis();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        col_t("compte", "Compte"),
        col_t("libelle", "Libellé"),
        col_m("debit", "Total débit"),
        col_m("credit", "Total crédit"),
        col_m("solde_debiteur", "Solde débiteur"),
        col_m("solde_crediteur", "Solde créditeur"),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    t.format = format_possible(&d.format, &FORMATS_TABLEAU)?;
    totaliser(&mut t, TOTAL_GENERAL)?;
    if d.ecrire {
        ecrire_tableau(&mut t, &p.dossier(ctx), &format!("balance-paie-{}", p.nom()), ctx)?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la provision de conges payes

/// `_taux_patronal` : le taux REEL des cotisations patronales d'un salarie -- ses cotisations patronales sur son
/// brut, sur les douze mois qui finissent a `jusqu_a`. Rend (taux en %, nombre de bulletins lus).
fn taux_patronal(lignes: &[crate::etats_paie::LigneLivre], cle: &str, jusqu_a: &str) -> R<(Option<Decimal>, i64)> {
    let annee: i64 = debut(jusqu_a, 4).parse().map_err(|_| erreur("ValueError", "mois illisible"))?;
    let depart = format!("{:04}-{}", annee - 1, tranche(jusqu_a, 5, 7));
    let (mut brut, mut pat, mut n) = (zero2(), zero2(), 0i64);
    for l in lignes {
        let (Some(b), Some(c)) = (l.brut, l.cotisations_patronales) else { continue };
        if l.cle_salarie != cle || !(depart.as_str() < l.mois.as_str() && l.mois.as_str() <= jusqu_a) {
            continue;
        }
        brut = add(brut, b);
        pat = add(pat, c);
        n += 1;
    }
    if brut.is_zero() {
        return Ok((None, 0));
    }
    Ok((Some(quant(div(mul(pat, Decimal::ONE_HUNDRED), brut), 2)), n))
}

/// `provision_conges` : la dette de conges payes des salaries au mois a la fin de la periode -- ce que la compagnie
/// devrait s'ils partaient ce jour-la. Un salarie, une ligne ; les intermittents n'y sont pas.
pub fn provision_conges(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    use droit::permanents::arrondir_jours;
    if d.toutes {
        return Err(refus("La provision de congés se calcule à UNE date : demander un mois (la fin du mois) ou une année (la fin du dernier mois produit)."));
    }
    let p = Periode::de(ctx, d, true)?;
    let saisis = p.saisis();
    let Some(m) = saisis.last().cloned() else {
        return Err(refus(format!("Aucun mois saisi dans PuppetPaie sur la période ({}) : la provision se calcule sur les paies du logiciel.", p.libelle)));
    };
    let fin = socle::date::dernier_du_mois(&m).map_err(|x| erreur("ValueError", x))?;
    let mut avert: Vec<String> = Vec::new();
    // Ce qui a ete paye ce mois-la : les paies ecrites de ses engagements valides, la saisie au meme rang.
    let (saisie, paies) = charger_mois(ctx, &m)?;
    let toutes = Periode::nouvelle(ctx, None, None, true, true)?;
    let livre = lignes_livre(ctx, &toutes, &mut Vec::new())?;
    let zero = zero2();
    let mut lignes: Vec<Ligne> = Vec::new();
    let mut vus: BTreeSet<String> = BTreeSet::new();
    let vide = Vec::new();
    let engagements = saisie.get("engagements").and_then(Value::as_array).unwrap_or(&vide);
    let cle_de = |p: &engagement::calcul::Paie| p.salarie.get("_cle").map(valeur_en_texte).unwrap_or_else(|| p.engagement.salarie.clone());
    for (e, pa) in engagements.iter().zip(paies.iter()) {
        let nat = pa.engagement.nat().map_err(|x| refus(x))?;
        if !nat.mensuelle() {
            continue;
        }
        if pa.engagement.fin.is_some_and(|f| f <= fin) {
            // Un contrat fini ce mois-ci a eu son solde de tout compte : les conges non pris y sont payes, il n'y a plus de dette.
            continue;
        }
        let cle = cle_de(pa);
        if !vus.insert(cle.clone()) {
            continue;
        }
        let brut_du_mois = paies.iter().filter(|q| cle_de(q) == cle).fold(zero, |a, q| add(a, q.brut));
        let Some(cpt) = provision_conges_de(pa, &m, brut_du_mois, e.get("salaire_mensuel")).map_err(socle::echec::Echec::from)? else { continue };
        for a in &cpt.avertissements {
            avert.push(format!("{} : {}", cle, a.texte));
        }
        let (dixieme, maintien, retenu, regle) = match &cpt.indemnite {
            Some(i) => (i.dixieme, i.maintien, i.retenu, i.regle.to_string()),
            None => {
                if cpt.solde < Decimal::ZERO {
                    avert.push(format!("{} : compteur négatif ({} jours) — des congés pris par anticipation ; aucune dette, et une avance à régler.", cle, arrondir_jours(cpt.solde)));
                }
                (zero, zero, zero, String::new())
            }
        };
        let (taux, n) = taux_patronal(&livre, &cle, &m)?;
        if taux.is_none() {
            avert.push(format!("{} : aucun bulletin de PuppetPaie sur les douze derniers mois — le taux des charges patronales ne se lit pas, la colonne reste vide.", cle));
        }
        let charges = taux.map(|t| eur(div(mul(retenu, t), Decimal::ONE_HUNDRED)));
        let fiche = ctx.salaries.get(&cle).cloned().unwrap_or(Value::Object(Map::new()));
        let nom = match fiche.get("nom").filter(|v| vrai(v)) {
            Some(v) => valeur_en_texte(v),
            None => cle.clone(),
        };
        let prenom = texte_ou_vide(fiche.get("prenom"));
        let mut l = Ligne::nouvelle();
        l.t("salarie", format!("{} {}", nom, prenom));
        l.t("contrat", texte_ou_vide(e.get("numero"))).t("nature", nat.libelle.clone());
        l.d("report", arrondir_jours(cpt.report_restant)).d("acquis", arrondir_jours(cpt.acquis)).d("pris", arrondir_jours(cpt.pris)).d("solde", arrondir_jours(cpt.solde));
        l.d("dixieme", dixieme).d("maintien", maintien).d("indemnite", retenu).t("regle", regle);
        l.od("taux_patronal", taux).n("bulletins", n).od("charges", charges);
        l.od("total", charges.map(|c| add(retenu, c)));
        lignes.push(l);
    }
    if lignes.is_empty() {
        // RIEN A PROVISIONNER N'EST PAS UNE ERREUR : le comptable qui la demande a sa reponse -- zero, et pourquoi.
        avert.push(format!(
            "Aucun salarié au mois sous contrat au {} : pas de congés payés à provisionner (les intermittents relèvent de la Caisse des Congés Spectacles, à laquelle la compagnie cotise chaque mois).",
            jj(&fin.iso())
        ));
    }
    let (plan, _origine) = crate::plan_comptable::plan(ctx)?;
    let pc = plan.get("provision_conges").filter(|v| vrai(v)).cloned().unwrap_or(Value::Object(Map::new()));
    let indemnites = lignes.iter().fold(zero, |a, l| add(a, l.decimal("indemnite").unwrap_or(zero)));
    let charges = lignes.iter().fold(zero, |a, l| add(a, l.decimal("charges").filter(|c| !c.is_zero()).unwrap_or(zero)));
    if !pc.as_object().is_none_or(|o| o.is_empty()) {
        let cle = |a: &str, b: &str| -> R<String> {
            let x = pc.get(a).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(a)))?;
            Ok(valeur_en_texte(x.get(b).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(b)))?))
        };
        avert.push(format!(
            "Écriture au {} (journal OD) : {} {} au débit, {} {} au crédit, {} € ; {} {} au débit, {} {} au crédit, {} €. À extourner le premier jour de l'exercice suivant — la dette se paie au fil des congés pris, ou au solde de tout compte.",
            jj(&fin.iso()),
            cle("conges", "compte")?,
            cle("conges", "libelle")?,
            cle("dette", "compte")?,
            cle("dette", "libelle")?,
            indemnites,
            cle("charges", "compte")?,
            cle("charges", "libelle")?,
            cle("dette_charges", "compte")?,
            cle("dette_charges", "libelle")?,
            charges
        ));
    }
    let mut t = Tableau::nouveau("provision-conges", format!("Provision pour congés payés au {}", jj(&fin.iso())), p.libelle.clone());
    t.mois = vec![m.clone()];
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        col_t("salarie", "Salarié"),
        col_t("contrat", "Contrat"),
        col_t("nature", "Nature"),
        col_n("report", "Report N-1 (j)"),
        col_n("acquis", "Acquis N (j)"),
        col_n("pris", "Pris (j)"),
        col_n("solde", "Solde (j)"),
        col_m("dixieme", "Au dixième"),
        col_m("maintien", "Au maintien"),
        col_m("indemnite", "Indemnité retenue"),
        col_t("regle", "Règle"),
        col_x("taux_patronal", "Taux patronal réel"),
        col_m("charges", "Charges patronales"),
        col_m("total", "Provision"),
    ];
    t.sources = vec!["compteur de congés écrit dans la paie validée du mois, indemnité L3141-24 comme au solde de tout compte, taux patronal = cotisations patronales / brut des bulletins des douze derniers mois".to_string()];
    finir(ctx, &mut t, Some(&p), "provision-conges", &d.format, false)?;
    for s in t.sections.iter_mut() {
        if let Some(tot) = s.total.as_mut() {
            for c in ["report", "acquis", "pris", "solde", "taux_patronal", "bulletins"] {
                tot.rien(c);
            }
        }
    }
    if let Some(tot) = t.total.as_mut() {
        for c in ["report", "acquis", "pris", "solde", "taux_patronal", "bulletins"] {
            tot.rien(c);
        }
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "provision-conges")?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la taxe sur les salaires

/// `Decimal(str(v))` d'une valeur du bareme.
fn dec_du_bareme(v: &Value) -> R<Decimal> {
    dec_avec_echelle(v).map_err(|m| erreur("InvalidOperation", m))
}

/// `ts[cle]` : `KeyError` s'il manque.
fn du_bareme<'a>(ts: &'a Value, cle: &str) -> R<&'a Value> {
    ts.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

/// `taxe_sur_les_salaires` : la taxe sur les salaires d'une annee (CGI art. 231), salarie par salarie, et ce qui
/// reste a payer apres franchise, decote et abattement. Se calcule sur une ANNEE (ses seuils sont annuels).
pub fn taxe_sur_les_salaires(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let Some(annee) = d.annee.as_deref().filter(|a| !a.is_empty()) else {
        return Err(refus("La taxe sur les salaires se calcule sur une ANNÉE (ses seuils sont annuels) : --annee AAAA."));
    };
    // Qui doit la taxe, et sur quelle part : le meme regime que la DSN de decembre (`assujettissement`, CGI art. 231, 1).
    // NE PAS SAVOIR N'EST PAS UNE ERREUR : l'etat le dit, et dit quoi repondre.
    let jour = socle::date::Jour::lire(&format!("{}-01-01", annee)).map_err(|x| erreur("ValueError", x))?;
    // Les plafonds de la franchise, que la conclusion nomme (ticket #245) : ceux du bareme de l'annee.
    let plafonds = crate::taxe_sur_les_salaires::plafonds_franchise(ctx.bareme(&jour).ok().as_ref());
    let regime = crate::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ctx.entreprise, plafonds);
    let p = Periode::nouvelle(ctx, None, Some(annee), false, true)?;
    let mut avert: Vec<String> = Vec::new();
    let part = match &regime.decision {
        Ok(part) => *part,
        Err(m) => {
            avert.push(m.clone());
            None
        }
    };
    if regime.decision == Ok(None) {
        avert.push(format!("{} Rien à calculer.", regime.consequence));
    }
    let bar = ctx.bareme(&jour).map_err(socle::echec::Echec::from)?;
    let ts = match bar.get("taxe_sur_les_salaires").filter(|v| vrai(v)) {
        Some(t) => t.clone(),
        None => {
            return Err(refus(format!(
                "Le barème {} ne porte pas la taxe sur les salaires (seuils, franchise, abattement) : ils se lisent sur la notice 2501-SD de l'année.",
                annee
            )))
        }
    };
    let abatt = match bar.get("abattement_csg").filter(|v| vrai(v)) {
        Some(v) => dec_du_bareme(v)?,
        None => Decimal::ONE,
    };
    let zero = zero2();
    // cle -> (salarie, brut, base, mois) dans l'ordre d'insertion
    let mut ordre: Vec<String> = Vec::new();
    let mut par: BTreeMap<String, (String, Decimal, Decimal, BTreeSet<String>)> = BTreeMap::new();
    for m in p.saisis() {
        let paies = match charger_mois(ctx, &m) {
            Ok((_, paies)) => paies,
            Err(e) if matches!(e.classe, Some("ErreurOperation") | Some("ValueError") | Some("KeyError") | Some("FileNotFoundError")) => {
                avert.push(format!("{} : le mois ne se charge pas ({}) — écarté.", m, e.message));
                continue;
            }
            Err(e) => return Err(e),
        };
        for pa in &paies {
            if pa.engagement.sans_paie() {
                continue;
            }
            let cle = pa.salarie.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?;
            if !par.contains_key(&cle) {
                ordre.push(cle.clone());
                par.insert(cle.clone(), (crate::tableaux_export::qui_souple(&pa.salarie), zero, zero, BTreeSet::new()));
            }
            let x = par.get_mut(&cle).expect("insere");
            // L'assiette de la CSG, sans la reduction de 1,75 % (CGI 231) : la meme que la DSN de decembre declare
            // (`assujettissement`, bloc S21.G00.44).
            x.1 = add(x.1, pa.brut);
            x.2 = add(x.2, crate::taxe_sur_les_salaires::assiette_taxe_sur_les_salaires(pa, abatt));
            x.3.insert(m.clone());
        }
    }
    let s1 = dec_du_bareme(du_bareme(&ts, "seuil_1")?)?;
    let s2 = dec_du_bareme(du_bareme(&ts, "seuil_2")?)?;
    let taux_normal = dec_du_bareme(du_bareme(&ts, "taux_normal")?)?;
    let maj_1 = dec_du_bareme(du_bareme(&ts, "majoration_1")?)?;
    let maj_2 = dec_du_bareme(du_bareme(&ts, "majoration_2")?)?;
    let cent = Decimal::ONE_HUNDRED;
    let mut rangs: Vec<&String> = ordre.iter().collect();
    rangs.sort_by(|a, b| par[*a].0.cmp(&par[*b].0));
    let mut lignes = Vec::new();
    let mut brute = zero;
    for cle in rangs {
        let (nom, brut, base, mois) = &par[cle];
        let b = *base;
        let normal = eur(div(mul(b, taux_normal), cent));
        let (au_dela_1, au_dela_2) = crate::taxe_sur_les_salaires::fractions_majorees(b, s1, s2);
        let m1 = eur(div(mul(au_dela_1, maj_1), cent));
        let m2 = eur(div(mul(au_dela_2, maj_2), cent));
        let taxe = add(add(normal, m1), m2);
        let mut l = Ligne::nouvelle();
        l.t("salarie", nom.clone()).n("mois", mois.len() as i64).d("brut", *brut).d("base", b).d("taux_normal", normal).d("majoration_1", m1).d("majoration_2", m2).d("taxe", taxe);
        brute = add(brute, taxe);
        lignes.push(l);
    }
    let pct_a = part.unwrap_or(Decimal::ZERO);
    let due = if part.is_some() { quant(div(mul(brute, pct_a), cent), 0) } else { zero };
    let franchise = dec_du_bareme(du_bareme(&ts, "franchise")?)?;
    let limite = dec_du_bareme(du_bareme(&ts, "decote_jusqu_a")?)?;
    let mut etapes = vec![format!("taxe brute {} € × {} % taxés (CGI art. 231, 1) = {} €", brute, pct_a, due)];
    let mut net;
    if due <= franchise {
        net = zero;
        etapes.push(format!("franchise : {} € n'excède pas {} € — rien à payer", due, franchise));
    } else if due <= limite {
        let decote = quant(div(mul(sub(limite, due), Decimal::from(3)), Decimal::from(4)), 0);
        net = sub(due, decote);
        etapes.push(format!("décote : ({} − {}) × 3/4 = {} €, reste {} €", limite, due, decote, net));
    } else {
        net = due;
    }
    if regime.association && net > Decimal::ZERO {
        let ab = dec_du_bareme(du_bareme(&ts, "abattement_1679A")?)?;
        let reste = sub(net, ab);
        let reste = if reste > zero { reste } else { zero };
        etapes.push(format!("abattement des associations (art. 1679 A) : {} €, reste {} €", ab, reste));
        net = reste;
    }
    etapes.push(format!("TAXE NETTE DUE : {} €", net));
    avert.push(format!("Calcul {} : {}.", annee, etapes.join(" ; ")));
    if net.is_zero() {
        avert.push("Rien à payer : pas de relevé 2501-SD ni de déclaration 2502-SD à déposer (franchise ou abattement).".to_string());
    } else {
        let periodicite = if net < Decimal::from(4000) {
            "un seul versement, déclaration 2502-SD au 15 janvier"
        } else if net <= Decimal::from(10000) {
            "trimestrielle (2501-SD)"
        } else {
            "mensuelle (2501-SD)"
        };
        avert.push(format!("Périodicité l'année suivante : {}.", periodicite));
    }
    let mut t = Tableau::nouveau("taxe-sur-les-salaires", format!("Taxe sur les salaires {}", annee), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        col_t("salarie", "Salarié"),
        col_n("mois", "Mois"),
        col_m("brut", "Brut"),
        col_m("base", "Base (assiette CSG sans réduction)"),
        col_m("taux_normal", "4,25 %"),
        col_m("majoration_1", &format!("+4,25 % au-delà de {} €", valeur_en_texte(du_bareme(&ts, "seuil_1")?))),
        col_m("majoration_2", &format!("+9,35 % au-delà de {} €", valeur_en_texte(du_bareme(&ts, "seuil_2")?))),
        col_m("taxe", "Taxe brute"),
    ];
    t.sources = vec!["CGI art. 231 et 1679 A ; notices 2501-SD et 2502-SD de la DGFiP (seuils, franchise, décote, abattement de l'année)".to_string()];
    finir(ctx, &mut t, Some(&p), "taxe-sur-les-salaires", &d.format, false)?;
    for s in t.sections.iter_mut() {
        if let Some(tot) = s.total.as_mut() {
            tot.rien("mois");
        }
    }
    if let Some(tot) = t.total.as_mut() {
        tot.rien("mois");
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "taxe-sur-les-salaires")?;
    }
    Ok(t)
}

#[allow(dead_code)]
fn _inutilises() {
    let _ = Val::txt("");
}

/// Le compteur d'un permanent a la fin du mois et l'indemnite de son solde.
pub struct Provision {
    pub report_restant: socle::num::Decimal,
    pub acquis: socle::num::Decimal,
    pub pris: socle::num::Decimal,
    pub solde: socle::num::Decimal,
    pub avertissements: Vec<engagement::Avis>,
    pub indemnite: Option<droit::conges::IndemniteDesJours>,
}

/// La provision de conges d'un permanent a la fin de `mois`, lue dans la paie ecrite `paie` de ce mois : le compteur
/// que son bulletin montre (`Paie::conges`, ecrit a la validation), dont l'assiette du dixieme compte aussi le brut du
/// mois (`brut_du_mois`, celui des paies ecrites du salarie), et l'indemnite de son solde (le dixieme ou le maintien,
/// L3141-24) quand il est positif. `salaire_mensuel` : la valeur de l'engagement (`None` ou fausse = 0). `None` : un
/// contrat sans compteur de l'employeur (declare a la caisse des Conges Spectacles). Refus : une paie validee qui ne
/// porte pas le compteur qu'elle devrait.
pub fn provision_conges_de(paie: &engagement::calcul::Paie, mois: &str, brut_du_mois: socle::num::Decimal, salaire_mensuel: Option<&Value>) -> socle::num::Resultat<Option<Provision>> {
    use droit::conges::jours_ouvrables;

    use socle::num::{add, dec, div, eur, mul, Decimal};
    let Some(ecrit) = paie.conges.as_ref() else {
        if paie.declaree_a_la_caisse_des_conges()? {
            return Ok(None);
        }
        return Err(format!(
            "{} : la paie validee de {} ne porte pas le compteur de conges que son bulletin montre. Revalider l'engagement.",
            paie.engagement.salarie, mois
        ));
    };
    let fin = socle::date::dernier_du_mois(mois)?;
    let mut cpt = ecrit.clone();
    cpt.remuneration_periode = add(cpt.remuneration_periode, brut_du_mois);
    let solde = cpt.solde();
    let salaire = match salaire_mensuel {
        Some(v) if socle::fichiers::vrai(v) => dec(v)?,
        _ => Decimal::ZERO,
    };
    let ouvrables = match jours_ouvrables(&socle::date::premier_du_mois(mois)?, &fin, None)? {
        0 => Decimal::ONE,
        n => Decimal::from(n),
    };
    let indemnite = if solde > Decimal::ZERO {
        Some(cpt.indemnite_des_jours(solde, eur(mul(div(salaire, ouvrables), solde)))?)
    } else {
        None
    };
    Ok(Some(Provision { report_restant: cpt.report_restant(), acquis: cpt.acquis, pris: cpt.pris, solde, avertissements: cpt.avertissements.clone(), indemnite }))
}
