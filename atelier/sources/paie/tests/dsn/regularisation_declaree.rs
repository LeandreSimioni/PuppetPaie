//! Corrections DSN, vagues 0 et 0b : une regularisation annule ce qui a ete DECLARE, relu dans la DSN deposee, et non
//! la paie d'avant refaite par le moteur du jour (norme § 2.1.3.6, p. 30 et 49 ; fiche N7 § 2.1.3.6 (b) ; point 23 de
//! la relecture). Vague 0b : le DERNIER etat declare fait foi -- la derniere DSN acceptee du mois (un
//! annule-et-remplace), le rappel porte par une correction precedente -- et l'agrege (bordereau Urssaf du mois
//! d'origine, versements 20, bloc 55) porte la meme difference que le nominatif.
//!
//! Chaque test monte une copie de `fictive-3090-grande` (compagnie inventee) dans un dossier a lui : juillet 2026 y est
//! depose pour de vrai (journal et `sorties/2026-07/DSN_Juillet2026.dsn`), l'echeance passee. On corrige le contrat de
//! Servandoni (technicien, SE9.26.05, pas paye en octobre), on change parfois un taux du bareme livre (dans une copie de
//! `logiciel/donnees/`, ou `PUPPETPAIE_CODE` pointe alors), et on produit octobre : la DSN d'octobre porte la correction
//! de juillet. Les contrats sur deux mois montent la `fictive-3090-petite`, ou Lemercier en a un. Aucun reseau, aucun
//! depot.

use crate::outils::{self, compagnie, ecrire as ecrire_json, lire, mois_pret};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-11-10";
const NIR: &str = "1910659999005";
const CONTRAT: &str = "SE9.26.05";
/// Le debut de la periode du contrat corrige (51.001), en JJMMAAAA.
const JUILLET: &str = "02072026";
const URSSAF: &str = "78861779300013";
const FRANCE_TRAVAIL: &str = "13000548100010";

/// La 3090-grande sans ses pieces, sauf juillet 2025 et juillet 2026 (les bulletins archives que relit la correction),
/// dans `<dossier du test>/compagnie` : le code livre, quand un cas le change, se copie a cote.
fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("fictive-3090-grande", &["2025-07", "2026-07"], &outils::dossier_de_test("regularisation_declaree", test).join("compagnie"))
}

fn route_le(racine: &Path, jour: &str, nom: &str, charge: Value) -> Value {
    outils::route(racine, nom, charge, jour)
}

fn octobre_pret(racine: &Path) {
    mois_pret(racine, "2026-10")
}

/// Une correction de juillet : l'heure de Servandoni passe a `taux` EUR (18 dans le depot).
fn corriger_juillet_a(racine: &Path, jour: &str, taux: f64) {
    let d = route_le(racine, jour, "regularisation-mois", json!({"mois": "2026-07"}));
    assert_eq!(d["ok"], json!(true), "{}", d);
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(CONTRAT)).unwrap().clone();
    let mut e = item["engagement"].clone();
    for l in e["lignes"].as_array_mut().unwrap() {
        if l["rubrique"] == json!("heure") {
            l["taux"] = json!(taux);
        }
    }
    let r = route_le(racine, jour, "regularisation-ecrire", json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": e, "motif": "le taux horaire etait faux"}));
    assert_eq!(r["ok"], json!(true), "{}", r);
}

/// La correction de juillet : l'heure de Servandoni passe de 18 a 19 EUR.
fn corriger_juillet(racine: &Path) {
    corriger_juillet_a(racine, JOUR, 19.0)
}

/// Change, dans une copie du bareme livre 2026, les objets que `quoi` designe : comme une version suivante du
/// logiciel le ferait depuis le depot de juillet.
fn retoucher_le_bareme(racine: &Path, quoi: &dyn Fn(&mut serde_json::Map<String, Value>) -> bool) -> usize {
    fn parcourir(v: &mut Value, quoi: &dyn Fn(&mut serde_json::Map<String, Value>) -> bool, vus: &mut usize) {
        match v {
            Value::Object(o) => {
                if quoi(o) {
                    *vus += 1;
                }
                o.values_mut().for_each(|x| parcourir(x, quoi, vus));
            }
            Value::Array(a) => a.iter_mut().for_each(|x| parcourir(x, quoi, vus)),
            _ => {}
        }
    }
    let chemin = compagnie::avec_le_code(racine, false).join("baremes/2026.json");
    let mut b: Value = serde_json::from_slice(&std::fs::read(&chemin).unwrap()).unwrap();
    let mut vus = 0;
    parcourir(&mut b, quoi, &mut vus);
    std::fs::write(&chemin, serde_json::to_vec_pretty(&b).unwrap()).unwrap();
    vus
}

fn dsn_le(racine: &Path, mois: &str, jour: &str) -> Value {
    route_le(racine, jour, "dsn-a-deposer", json!({"mois": mois}))
}

fn dsn_d_octobre(racine: &Path) -> Value {
    dsn_le(racine, "2026-10", JOUR)
}

/// (rubrique, valeur) d'un fichier DSN.
fn lignes(chemin: &Path) -> Vec<(String, String)> {
    outils::paires(&String::from_utf8_lossy(&std::fs::read(chemin).unwrap()))
}

fn bloc(k: &str) -> &str {
    &k[8..10]
}

/// Le versement (bloc 50 et ce qui le suit jusqu'au prochain bloc 50 ou au bloc 86) du salarie `NIR` dont les
/// remunerations citent `CONTRAT` et commencent le `periode` (JJMMAAAA).
fn versement(toutes: &[(String, String)], periode: &str) -> Vec<(String, String)> {
    versement_du(toutes, CONTRAT, periode)
}

/// `versement`, pour un autre contrat de Servandoni.
fn versement_du(toutes: &[(String, String)], contrat: &str, periode: &str) -> Vec<(String, String)> {
    versement_de(toutes, NIR, contrat, periode)
}

/// `versement`, pour un contrat `contrat` du salarie `nir`.
fn versement_de(toutes: &[(String, String)], nir: &str, contrat: &str, periode: &str) -> Vec<(String, String)> {
    let mut trouves = Vec::new();
    let mut individu = false;
    let mut courant: Option<Vec<(String, String)>> = None;
    let fermer = |c: Option<Vec<(String, String)>>, trouves: &mut Vec<Vec<(String, String)>>| {
        if let Some(c) = c {
            let cite = c.iter().any(|(k, v)| k == "S21.G00.51.010" && v == contrat);
            let date = c.iter().find(|(k, _)| k == "S21.G00.51.001").map(|(_, v)| v.as_str()) == Some(periode);
            if cite && date {
                trouves.push(c);
            }
        }
    };
    for (k, v) in toutes {
        if k == "S21.G00.30.001" {
            fermer(courant.take(), &mut trouves);
            individu = v == nir;
            continue;
        }
        if !individu {
            continue;
        }
        if k == "S21.G00.50.001" {
            fermer(courant.take(), &mut trouves);
            courant = Some(Vec::new());
        } else if !["50", "51", "52", "53", "54", "56", "58", "78", "79", "81"].contains(&bloc(k)) {
            fermer(courant.take(), &mut trouves);
        }
        if let Some(c) = courant.as_mut() {
            c.push((k.clone(), v.clone()));
        }
    }
    fermer(courant.take(), &mut trouves);
    assert_eq!(trouves.len(), 1, "un versement de {} pour la periode {}", contrat, periode);
    trouves.remove(0)
}

fn valeur<'a>(v: &'a [(String, String)], k: &str) -> &'a str {
    v.iter().find(|(r, _)| r == k).map(|(_, x)| x.as_str()).unwrap_or_else(|| panic!("pas de {}", k))
}

fn centimes(t: &str) -> i64 {
    let (e, c) = t.trim_start_matches('-').split_once('.').unwrap();
    let n = e.parse::<i64>().unwrap() * 100 + c.parse::<i64>().unwrap();
    if t.starts_with('-') {
        -n
    } else {
        n
    }
}

fn texte_centimes(n: i64) -> String {
    format!("{}{}.{:02}", if n < 0 { "-" } else { "" }, n.abs() / 100, n.abs() % 100)
}

const MONTANTS: [&str; 8] =
    ["S21.G00.51.013", "S21.G00.53.002", "S21.G00.54.002", "S21.G00.58.004", "S21.G00.78.004", "S21.G00.79.004", "S21.G00.81.003", "S21.G00.81.004"];

/// Les lignes `blocs` du versement depose, comme une annulation doit les ecrire : montants de signe change.
fn annulation_attendue(depose: &[(String, String)], blocs: &[&str]) -> Vec<(String, String)> {
    depose
        .iter()
        .filter(|(k, _)| blocs.contains(&bloc(k)))
        .map(|(k, v)| (k.clone(), if MONTANTS.contains(&k.as_str()) { texte_centimes(-centimes(v)) } else { v.clone() }))
        .collect()
}

fn du_groupe(v: &[(String, String)], blocs: &[&str]) -> Vec<(String, String)> {
    v.iter().filter(|(k, _)| blocs.contains(&bloc(k))).cloned().collect()
}

const GROUPES: [&[&str]; 4] = [&["51", "53"], &["54"], &["58"], &["78", "79", "81"]];

/// Le mois de la periode d'origine d'un versement (MMAAAA, de son premier 51.001).
fn mois_d_origine(v: &[(String, String)]) -> String {
    valeur(v, "S21.G00.51.001")[2..].to_string()
}

/// Vague 1o : un versement de correction sans ses blocs 58 dates du mois de son versement (norme DSN 2026.1,
/// S21.G00.58.003, p. 265 : le montant net social se date « en periode de versement ») ; restent ceux de la periode
/// d'origine `mmaaaa`.
fn sans_rappels_58(v: &[(String, String)], mmaaaa: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        if v[i].0 == "S21.G00.58.001" && !v[i].1.ends_with(mmaaaa) {
            i += 1;
            while i < v.len() && v[i].0.starts_with("S21.G00.58.") && v[i].0 != "S21.G00.58.001" {
                i += 1;
            }
            continue;
        }
        out.push(v[i].clone());
        i += 1;
    }
    out
}

/// Chaque groupe du versement de regularisation `regul` commence par l'annulation de `declare` (les lignes d'un
/// versement tel qu'il a ete declare), ligne pour ligne, montants de signe change ; 78.005 excepte (l'affiliation se
/// renumerote dans le fichier qui porte la regularisation).
fn verifier_l_annulation(declare: &[(String, String)], regul: &[(String, String)]) {
    let regul = &sans_rappels_58(regul, &mois_d_origine(declare));
    for blocs in GROUPES {
        let attendu = annulation_attendue(declare, blocs);
        assert!(!attendu.is_empty() || blocs == ["54"], "le declare porte des blocs {:?}", blocs);
        let ecrit = du_groupe(regul, blocs);
        if blocs == ["58"] && ecrit.is_empty() {
            continue; // vague 1o : le montant net social d'origine n'est pas redeclare, seul le rappel l'est
        }
        assert!(ecrit.len() >= attendu.len(), "blocs {:?} : {:?}", blocs, ecrit);
        let annulation = &ecrit[..attendu.len()];
        for (i, ((ka, va), (ke, ve))) in attendu.iter().zip(annulation).enumerate() {
            assert_eq!(ka, ke, "blocs {:?}, ligne {}", blocs, i);
            if ka == "S21.G00.78.005" {
                continue;
            }
            assert_eq!(va, ve, "blocs {:?}, ligne {} ({}) : l'annulation doit porter le montant declare", blocs, i, ka);
        }
    }
}

/// Ce qu'un versement de regularisation declare APRES la correction : dans chaque groupe, les lignes qui suivent
/// l'annulation de `declare`, dans l'ordre d'un versement.
/// Vague 1o : sans 58 de la periode d'origine dans `regul`, le montant net social d'origine reste celui de `declare`.
fn rappel(declare: &[(String, String)], regul: &[(String, String)]) -> Vec<(String, String)> {
    let regul = &sans_rappels_58(regul, &mois_d_origine(declare));
    let mut out = Vec::new();
    for blocs in GROUPES {
        let n = du_groupe(declare, blocs).len();
        if blocs == ["58"] && du_groupe(regul, blocs).is_empty() {
            out.extend(du_groupe(declare, blocs));
            continue;
        }
        out.extend(du_groupe(regul, blocs).into_iter().skip(n));
    }
    out
}

// ------------------------------------------- (a) l'agrege porte la meme difference que le nominatif

/// La somme, dans `v`, des montants `rubrique` des lignes que `garder` retient. `garder` recoit, pour chaque ligne,
/// la ligne et le contexte (78.001 et 78.005 du bloc 78 en cours, 51.011 en cours, 81.001 et 81.002 en cours).
#[derive(Default, Clone)]
struct Contexte {
    b78: String,
    aff: String,
    contrat78: String,
    type51: String,
    code81: String,
    org81: String,
}

fn lignes_en_contexte(v: &[(String, String)]) -> Vec<(String, String, Contexte)> {
    let mut c = Contexte::default();
    let mut out = Vec::new();
    // Le contrat d'une base : 78.006, ou, quand l'individu n'a qu'un contrat (78.006 n'est alors pas a renseigner,
    // cahier technique 2026.1 p. 298 ; reste R1 #26), son 40.009.
    let mut contrat40 = String::new();
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.40.009" => contrat40 = x.clone(),
            "S21.G00.78.001" => c = Contexte { b78: x.clone(), contrat78: contrat40.clone(), ..Default::default() },
            "S21.G00.78.005" => c.aff = x.clone(),
            "S21.G00.78.006" => c.contrat78 = x.clone(),
            "S21.G00.51.011" => c.type51 = x.clone(),
            "S21.G00.81.001" => {
                c.code81 = x.clone();
                c.org81 = String::new();
            }
            "S21.G00.81.002" => c.org81 = x.clone(),
            _ => {}
        }
        out.push((k.clone(), x.clone(), c.clone()));
    }
    out
}

fn somme(v: &[(String, String)], rubrique: &str, garder: &dyn Fn(&Contexte) -> bool) -> i64 {
    lignes_en_contexte(v).iter().filter(|(k, _, c)| k == rubrique && garder(c)).map(|(_, x, _)| centimes(x)).sum()
}

/// L'euro le plus proche de centimes, la fraction egale a 0,50 comptee pour 1 (CSS L. 133-10,
/// `atelier/reference/sources/code-de-la-securite-sociale/articles-20261013.md`) ; rendu en centimes. Un montant negatif
/// s'arrondit comme son oppose (INFERENCE : le texte ne dit que « comptee pour 1 »).
fn a_l_euro(n: i64) -> i64 {
    n.signum() * ((n.abs() + 50) / 100) * 100
}

/// Les lignes 23 du bordereau Urssaf de la periode qui commence le `debut` (S21.G00.22.003) : (code, qualifiant,
/// assiette ou montant en centimes).
fn bordereau(toutes: &[(String, String)], debut: &str) -> Vec<(String, String, i64)> {
    let (mut dedans, mut out) = (false, Vec::new());
    for (k, v) in toutes {
        match k.as_str() {
            "S21.G00.22.001" => dedans = false,
            "S21.G00.22.003" => dedans = v == debut,
            "S21.G00.23.001" if dedans => out.push((v.clone(), String::new(), 0)),
            "S21.G00.23.002" if dedans => out.last_mut().unwrap().1 = v.clone(),
            "S21.G00.23.004" | "S21.G00.23.005" if dedans => out.last_mut().unwrap().2 = centimes(v),
            _ => {}
        }
    }
    out.sort();
    out
}

/// La cle de totalisation d'une ligne 81 (`codes81` des versements de `config/organismes.json`).
fn cle_81(code: &str, org: &str) -> String {
    match (code, org) {
        ("018", FRANCE_TRAVAIL) => "018:chomage".into(),
        ("018" | "040" | "048", URSSAF) => format!("{}:urssaf", code),
        _ => code.into(),
    }
}

/// Les versements 20 d'une periode (S21.G00.20.006) : SIRET -> montant en centimes.
fn versements_20(toutes: &[(String, String)], debut: &str) -> BTreeMap<String, i64> {
    let (mut siret, mut montant, mut out) = (String::new(), 0, BTreeMap::new());
    for (k, v) in toutes {
        match k.as_str() {
            "S21.G00.20.001" => siret = v.clone(),
            "S21.G00.20.005" => montant = centimes(v),
            "S21.G00.20.006" if v == debut => {
                out.insert(siret.clone(), montant);
            }
            _ => {}
        }
    }
    out
}

/// `outils::par_contrat_de_prevoyance`, en centimes.
fn par_contrat_de_prevoyance(toutes: &[(String, String)]) -> BTreeMap<String, i64> {
    outils::par_contrat_de_prevoyance(toutes).into_iter().map(|(c, m)| (c, i64::try_from(m * socle::num::Decimal::ONE_HUNDRED).unwrap())).collect()
}

/// La ligne 23 de la reduction generale d'un mois corrige, pour une difference `d` (en centimes, positive quand la
/// reduction monte) : le CTP de la reduction en montant (23.005) quand elle monte, son CTP de regularisation en assiette
/// (23.004), sans signe, quand elle baisse. « Ctp 668/671 : montant de cotisation (reduction) ; Ctp 669/801 :
/// [montant d'assiette] », « les montants ne doivent pas comporter de signe ; les CTP comprennent les signes
/// necessaires » (guide declaratif Urssaf 5.1, § 8.2, atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt,
/// lignes 1341-1362).
fn reduction_ou_sa_regularisation(ctp: &'static str, regularisation: &'static str, d: i64) -> (&'static str, &'static str, i64) {
    if d < 0 { (regularisation, "921", -d) } else { (ctp, "921", d) }
}

#[test]
fn l_agrege_du_mois_d_origine_porte_la_meme_difference_que_le_nominatif() {
    // Le bareme livre change entre le depot et la correction, sur ce qui fait l'agrege : les assiettes du bordereau
    // Urssaf (majorations conges 115 et 110, abattement CSG), le taux Conges Spectacles (066, versement 20) et la
    // contribution TSINTERMITTENTS (059, bloc 55). Refaite aujourd'hui, la paie de juillet donnerait d'autres
    // assiettes et d'autres montants que ceux qui sont partis.
    let r = copie("agrege");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    corriger_juillet(&r);
    let vus = retoucher_le_bareme(&r, &|o| {
        let mut touche = false;
        for (cle, nouveau) in [("majoration_conges_115", 1.2), ("majoration_conges_110", 1.15), ("abattement_csg", 0.98)] {
            if o.get(cle).and_then(Value::as_f64).is_some_and(|x| x > 1.05 || cle == "abattement_csg") {
                o.insert(cle.into(), json!(nouveau));
                touche = true;
            }
        }
        if o.get("code81") == Some(&json!("066")) && o.contains_key("taux") {
            o.insert("taux".into(), json!(16.0));
            touche = true;
        }
        if o.get("code81") == Some(&json!("059")) && o.get("affectation") == Some(&json!("202")) && o.get("taux") == Some(&json!(0.42)) {
            o.insert("taux".into(), json!(0.5));
            touche = true;
        }
        touche
    });
    assert!(vus >= 4, "{} objets du bareme retouches", vus);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement(&octobre, JUILLET);
    verifier_l_annulation(&depose, &regul);

    // Le nominatif : le versement de regularisation annule le declare et declare le corrige ; la somme de chaque
    // rubrique y est donc « corrige moins declare ».
    let d_brut = somme(&regul, "S21.G00.51.013", &|c| c.type51 == "001");
    let d_csg = somme(&regul, "S21.G00.78.004", &|c| c.b78 == "04");
    // Vague 1i (point 22) : la base 02 est l'assiette plafonnee. SE9.26.05 (3 312,00 en juillet) depasse son plafond
    // (3 100,65) : le corrige y porte 3 100,65 la ou le depose portait le brut ; le 100/921 suit la base 02, plus le brut.
    let d_02 = somme(&regul, "S21.G00.78.004", &|c| c.b78 == "02");
    let d_x115 = somme(&regul, "S21.G00.81.003", &|c| c.code81 == "049");
    let d_x110 = somme(&regul, "S21.G00.81.003", &|c| c.code81 == "130");
    let d_rg = -somme(&regul, "S21.G00.81.004", &|c| c.code81 == "018" && c.org81 == URSSAF);
    assert!(d_brut != 0 && d_csg != 0 && d_x115 != 0 && d_x110 != 0);
    assert!(d_rg < 0, "la reduction du technicien baisse : elle se regularise en 801 ({})", d_rg);

    // Le bordereau Urssaf de juillet que porte octobre, code par code (un technicien intermittent : la table des CTP
    // de config/organismes.json). Ses complements maladie et allocations familiales sont ceux du taux plein, 635 et 430
    // (son nominatif declare 907 a 6,00 et 102 a 1,80) : les 636 et 432 « ABATT. 30 % » sont ceux des artistes (« Les
    // techniciens ne sont pas concernes », atelier/reference/sources/urssaf-taux-reduits-artistes/, l. 24 du .txt). La copie
    // figee a un fichier des organismes compose avant : il se lit juste sans etre recompose.
    let mut attendu: Vec<(String, String, i64)> = [
        ("027", "920", d_brut),
        ("260", "920", d_csg),
        ("430", "920", d_brut),
        ("635", "920", d_brut),
        ("983", "920", d_brut),
        ("992", "920", d_x110),
        ("100", "920", d_brut),
        ("100", "921", d_02),
        ("332", "921", d_x115),
        reduction_ou_sa_regularisation("671", "801", d_rg),
    ]
    .iter()
    .map(|(c, q, v)| (c.to_string(), q.to_string(), a_l_euro(*v)))
    .filter(|(_, _, v)| *v != 0)
    .collect();
    attendu.sort();
    // Les ecarts se relevent tous avant de conclure : bordereau, versements 20, bloc 55.
    let mut ecarts: Vec<String> = Vec::new();
    let ecrit = bordereau(&octobre, "01072026");
    if ecrit != attendu {
        ecarts.push(format!("bordereau Urssaf de juillet : ecrit {:?}, nominatif {:?}", ecrit, attendu));
    }

    // Les versements 20 d'octobre (hors Urssaf, que le bordereau fait).
    ecarts.extend(ecarts_des_versements_20(&r, &octobre));

    // Le bloc 55 : par affectation, la somme des bases 31 du fichier (`bloc_55`).
    let (b55, periodes) = bloc_55(&octobre);
    assert!(
        periodes.get("TSINTERMITTENTS").is_some_and(|p| p.contains(&"2026T03".to_string())),
        "la difference de juillet sur TSINTERMITTENTS s'affecte au trimestre de juillet : {:?}",
        periodes
    );
    assert!(periodes.values().flatten().all(|p| p == "2026T03" || p == "2026T04"), "{:?}", periodes);
    let aff = par_contrat_de_prevoyance(&octobre);
    assert!(aff.contains_key("TSINTERMITTENTS") && aff.contains_key("INTERBRANCHE") && aff.contains_key("CASCSVP"), "{:?}", aff);
    if b55 != aff {
        ecarts.push(format!("bloc 55 : ecrit {:?}, nominatif {:?}", b55, aff));
    }
    assert!(ecarts.is_empty(), "l'agrege ne porte pas la difference du nominatif :\n{}", ecarts.join("\n"));
}

/// Les versements 20 d'octobre (hors Urssaf, que le bordereau fait) dont le montant n'est pas la somme des lignes 81 de
/// leurs codes dans tout le fichier, mois courant et regularisation : un ecart par versement.
fn ecarts_des_versements_20(r: &Path, octobre: &[(String, String)]) -> Vec<String> {
    let mut ecarts = Vec::new();
    let orgs = lire(r, "config/organismes.json");
    let vers = versements_20(octobre, "01102026");
    let toutes_81: Vec<(String, i64)> = lignes_en_contexte(octobre)
        .into_iter()
        .filter(|(k, _, _)| k == "S21.G00.81.004")
        .map(|(_, x, c)| (cle_81(&c.code81, &c.org81), centimes(&x)))
        .collect();
    for v in orgs["versements"].as_array().unwrap() {
        let siret = v["siret"].as_str().unwrap();
        if siret == URSSAF {
            continue;
        }
        let mut codes: Vec<&str> = v["codes81"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
        // Vague 1g : la copie figee nomme Agirc-Arrco par le seul 105 (fichier compose avant ce lot) ; ce 105 « y
        // compris Apec » se declare desormais en 131 et 132 (cahier technique 2026.1 p. 302), que le meme versement
        // porte. Le 105 depose en juillet s'y annule, le rappel s'y declare en 131.
        if codes.contains(&"105") {
            codes.extend(["131", "132"]);
        }
        let attendu: i64 = toutes_81.iter().filter(|(k, _)| codes.contains(&k.as_str())).map(|(_, m)| m).sum();
        let ecrit = vers.get(siret).copied().unwrap_or(0);
        if ecrit != attendu {
            ecarts.push(format!("versement 20 de {} ({:?}) : ecrit {}, nominatif {}", siret, codes, ecrit, attendu));
        }
    }
    ecarts
}

/// Le bloc 55 d'un fichier : par code d'affectation (55.003), la somme des montants (55.001), et ses periodes (55.004).
fn bloc_55(octobre: &[(String, String)]) -> (BTreeMap<String, i64>, BTreeMap<String, Vec<String>>) {
    // Depuis la vague 1f, un composant par contrat ET par periode d'affectation (« une periode civile complete [...]
    // contenant la ou les periodes cotisees », norme p. 260) : la difference de juillet part sous 2026T03, le mois
    // d'octobre sous 2026T04 ; on somme donc les periodes par contrat.
    let mut b55: BTreeMap<String, i64> = BTreeMap::new();
    let mut periodes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let (mut montant, mut code) = (0, String::new());
    for (k, v) in octobre {
        match k.as_str() {
            "S21.G00.55.001" => montant = centimes(v),
            "S21.G00.55.003" => {
                *b55.entry(v.clone()).or_insert(0) += montant;
                code = v.clone();
            }
            "S21.G00.55.004" => periodes.entry(code.clone()).or_default().push(v.clone()),
            _ => {}
        }
    }
    (b55, periodes)
}

/// Les 78.004 du bloc `code` de lignes de versement.
fn bases_78(v: &[(String, String)], code: &str) -> Vec<String> {
    let (mut courant, mut out) = (String::new(), Vec::new());
    for (k, x) in v {
        if k == "S21.G00.78.001" {
            courant = x.clone();
        } else if k == "S21.G00.78.004" && courant == code {
            out.push(x.clone());
        }
    }
    out
}

#[test]
fn une_correction_de_juillet_2025_annule_le_declare_et_plafonne_le_rappel() {
    // Vague 1l : le bareme 2025 assied desormais la vieillesse plafonnee, le FNAL et le chomage d'un intermittent sur
    // son plafond. Juillet 2025 est parti avant (base 02 = le brut, 3 220,00 pour SE9.25.04, technicien du 03 au
    // 26/07/2025). Une correction (l'heure de 17,50 a 18,50 : 3 404,00) annule ce qui a ete DECLARE -- base 02 a
    // 3 220,00, relue dans DSN_Juillet2025.dsn --, et le rappel declare la base 02 sur le plafond du contrat :
    // 3 925 x 24 / 31 = 3 038,71. Vague 1o : le plafond des jours remuneres (BOSS, assiette generale, § 1090), 23 jours
    // dates sur les 24 du contrat : 3 925 x 23 / 31 = 2 912,10.
    let r = copie("juillet-2025");
    const SE: &str = "SE9.25.04";
    const PERIODE: &str = "03072025";
    let depose = versement_du(&lignes(&r.join("sorties/2025-07/DSN_Juillet2025.dsn")), SE, PERIODE);
    assert_eq!(bases_78(&depose, "02"), vec!["3220.00"], "le depot de juillet 2025 porte le brut en base 02");
    let d = route_le(&r, JOUR, "regularisation-mois", json!({"mois": "2025-07"}));
    assert_eq!(d["ok"], json!(true), "{}", d);
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(SE)).unwrap().clone();
    let mut e = item["engagement"].clone();
    for l in e["lignes"].as_array_mut().unwrap() {
        if l["rubrique"] == json!("heure") {
            l["taux"] = json!(18.5);
        }
    }
    let w = route_le(&r, JOUR, "regularisation-ecrire", json!({"mois": "2025-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": e, "motif": "le taux horaire etait faux"}));
    assert_eq!(w["ok"], json!(true), "{}", w);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let regul = versement_du(&lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn")), SE, PERIODE);
    verifier_l_annulation(&depose, &regul);
    let apres = rappel(&depose, &regul);
    assert_eq!(bases_78(&apres, "02"), vec!["2912.10"], "le rappel plafonne la base 02");
    assert_eq!(bases_78(&apres, "03"), vec!["3404.00"]);
    assert_eq!(bases_78(&apres, "07"), vec!["3404.00"], "le chomage dans la limite de quatre plafonds");
}

// ------------------------------------------------- vague 2c : le numero de versement, le bordereau d'un etablissement

/// Les rubriques de l'individu `nir` d'un fichier DSN (de son 30.001 au 30.001 suivant, ou a la fin des blocs S21).
fn individu(toutes: &[(String, String)], nir: &str) -> Vec<(String, String)> {
    let mut dedans = false;
    let mut out = Vec::new();
    for (k, v) in toutes {
        if k == "S21.G00.30.001" {
            dedans = v == nir;
        } else if !k.starts_with("S21.G00.") {
            dedans = false;
        }
        if dedans {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

#[test]
fn un_contrat_paye_ce_mois_et_corrige_porte_des_numeros_de_versement_distincts() {
    // Point 18 de la relecture (fiche N2, S21.G00.50.003 ; N7 § 2.1.3.6 (c)). Norme DSN 2026.1, bloc 50, p. 243-244
    // (`atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`) : CCH-11, « Pour un même bloc "Individu - S21.G00.30", il
    // est interdit de déclarer plusieurs blocs « Versement individu - S21.G00.50 » avec un même "Numéro de versement" » ;
    // format N [2,2], CSL 00 [0]*[1-9][0-9]*.
    //
    // Le contrat de Servandoni (SE9.26.05, du 02/07) est prolonge jusqu'au 09/10 : octobre le paie, et la DSN d'octobre
    // porte aussi la correction de juillet du MEME contrat. Avant, la regularisation reprenait le rang du versement
    // courant : deux blocs 50 numerotes 01 sous le meme individu.
    let r = copie("numero-de-versement");
    corriger_juillet(&r);
    // Le contrat se prolonge : sa fin passe au 09/10, et octobre porte ses jours.
    let mut juillet = crate::outils::saisie(&r, "2026-07");
    let e = juillet["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == json!(CONTRAT)).unwrap();
    e["fin"] = json!("2026-10-09");
    crate::outils::poser_saisie(&r, "2026-07", &juillet);
    let mut saisie = crate::outils::saisie(&r, "2026-10");
    let e = saisie["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == json!(CONTRAT)).expect("octobre montre le contrat prolonge");
    e["date_paiement"] = json!("2026-10-09");
    e["lignes"] = json!([{"rubrique": "heure", "quantite": 32, "taux": 18.0, "lieu": "Theatre fictif de Saumur",
                          "dates": ["2026-10-06", "2026-10-07", "2026-10-08", "2026-10-09"]}]);
    crate::outils::poser_saisie(&r, "2026-10", &saisie);
    crate::outils::tout_valider(&r, "2026-10", JOUR);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let lui = individu(&octobre, NIR);

    // Le contrat ne se declare qu'une fois ; il porte deux versements : celui d'octobre et la correction de juillet.
    assert_eq!(lui.iter().filter(|(k, v)| k == "S21.G00.40.009" && v == CONTRAT).count(), 1, "un seul bloc 40 pour {}", CONTRAT);
    let courant = versement(&octobre, "01102026");
    let regul = versement(&octobre, JUILLET);
    let numeros: Vec<&str> = lui.iter().filter(|(k, _)| k == "S21.G00.50.003").map(|(_, v)| v.as_str()).collect();
    assert_eq!(numeros.len(), 2, "{:?}", numeros);
    let conforme = |n: &str| n.len() == 2 && n.chars().all(|c| c.is_ascii_digit()) && n != "00";
    assert!(numeros.iter().all(|n| conforme(n)), "N [2,2], [0]*[1-9][0-9]* : {:?}", numeros);
    assert_ne!(numeros[0], numeros[1], "CCH-11 : deux versements du meme individu, deux numeros");
    assert_eq!(valeur(&courant, "S21.G00.50.003"), "01", "le versement du mois garde son numero");
    assert_eq!(valeur(&regul, "S21.G00.50.003"), "02", "la correction prend le numero suivant");

    // Le numero de versement n'est pas le rang du contrat : les 78.005 de la correction citent toujours une affiliation
    // (70.012) que le bloc 40 du contrat declare.
    let mut affiliations = Vec::new();
    let mut du_contrat = false;
    for (k, v) in &lui {
        match k.as_str() {
            "S21.G00.40.009" => du_contrat = v == CONTRAT,
            "S21.G00.50.001" => du_contrat = false,
            "S21.G00.70.012" if du_contrat => affiliations.push(v.clone()),
            _ => {}
        }
    }
    let citees: Vec<&String> = regul.iter().filter(|(k, _)| k == "S21.G00.78.005").map(|(_, v)| v).collect();
    assert!(!citees.is_empty());
    for a in citees {
        assert!(affiliations.contains(a), "78.005 = {} hors des affiliations du contrat {:?}", a, affiliations);
    }
}

#[test]
fn la_correction_d_un_etablissement_secondaire_porte_son_taux_at_dans_le_bordereau() {
    // Point 25 de la relecture (fiche N7 § 2.2.3.1-2.2.3.3). Le bordereau Urssaf du mois d'origine que porte la DSN d'un
    // etablissement secondaire (22/23) se valorise au taux AT/MP de CET etablissement (regle 8 : chaque etablissement a
    // le sien, `bareme::surcharger`), comme le bordereau courant ; avant, il prenait celui du principal (1,25).
    //
    // Servandoni passe a l'etablissement 00099 (taux AT/MP 3,10) ; juillet 2026 de cet etablissement est depose (une
    // copie du depot du principal, 11.001 = 00099). La correction passe l'heure a 25 EUR : avec 19 EUR, le rappel de net
    // imposable serait negatif une fois les defraiements sortis de la RNF (vague 1a), et le PAS du seul etablissement
    // negatif refuse.
    let r = copie("etablissement-secondaire");
    let mut ent = lire(&r, "config/entreprise.json");
    let naf = ent["naf"].clone();
    ent["etablissements"] = json!([{
        "nic": "00099", "naf": naf, "adresse": "1 RUE DE L'ESSAI", "code_postal": "69001", "commune": "LYON",
        "taux_atmp": {"valeur": 3.1, "code_risque": "923AD"}
    }]);
    ecrire_json(&r, "config/entreprise.json", &ent);
    let mut sal = lire(&r, "config/salaries.json");
    sal["SERVANDONI"]["etablissement"] = json!("00099");
    ecrire_json(&r, "config/salaries.json", &sal);
    let depot = std::fs::read(r.join("sorties/2026-07/DSN_Juillet2026.dsn")).unwrap();
    let texte = String::from_utf8_lossy(&depot).replace("S21.G00.11.001,'00016'", "S21.G00.11.001,'00099'");
    std::fs::write(r.join("sorties/2026-07/DSN_Juillet2026_00099.dsn"), texte.as_bytes()).unwrap();
    let mut j = lire(&r, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let mut reel = l.iter().find(|x| x["quoi"] == json!("dsn") && x["mois"] == json!("2026-07") && x["mode"] == json!("reel")).unwrap().clone();
    reel["fichier"] = json!("DSN_Juillet2026_00099.dsn");
    reel["detail"]["etablissement"] = json!("00099");
    l.push(reel);
    ecrire_json(&r, "journal/depots.json", &j);

    corriger_juillet_a(&r, JOUR, 25.0);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let secondaire = lignes(&r.join("sorties/2026-10/DSN_Octobre2026_00099.dsn"));
    assert_eq!(valeur(&secondaire, "S21.G00.11.001"), "00099");
    // Le versement de la correction est bien dans cette DSN-ci.
    let _ = versement(&secondaire, JUILLET);

    // Les lignes 23 du bordereau de juillet qui portent un taux AT (23.003) : celui de l'etablissement, 3,10 pour un
    // technicien (2,17 = 70 % pour un artiste, `taux_at_du_ctp`), jamais celui du principal.
    let (mut dedans, mut taux) = (false, Vec::new());
    for (k, v) in &secondaire {
        match k.as_str() {
            "S21.G00.22.001" => dedans = false,
            "S21.G00.22.003" => dedans = v == "01072026",
            "S21.G00.23.003" if dedans => taux.push(v.clone()),
            _ => {}
        }
    }
    assert!(!taux.is_empty(), "le bordereau de juillet porte une ligne au taux AT");
    for t in &taux {
        assert!(t == "03.10" || t == "02.17", "23.003 = {} : le taux AT/MP de l'etablissement 00099, pas celui du principal", t);
    }
}

// ------------------------------------------------- vague 3a : la correction d'un permanent (CDI)

/// Montansier, dans la copie figee : CDI du 04/09/2023 (MO.23.01), payee au mois -- 3 120,00 et une prime de tournee de
/// 250,00 en juillet 2026 --, technicienne, avec reduction generale. Juillet 2026 depose porte pour elle deux 018 chez
/// l'Urssaf (part securite sociale et part chomage, que l'Urssaf recouvre pour un permanent). Elle est aussi payee en
/// octobre, comme Nicolet (CDI, NI.24.01).
const NIR_MO: &str = "2820731999006";
const NIR_NI: &str = "1930935999007";
const CDI: &str = "MO.23.01";
/// Le debut de la periode d'un mois paye au mois (51.001), en JJMMAAAA.
const JUILLET_MOIS: &str = "01072026";
const OCTOBRE_MOIS: &str = "01102026";

/// Corrige juillet 2026 de Montansier par les routes de la page : ses lignes du mois deviennent `lignes` (un contrat paye
/// au mois qui ecrit ses lignes les garde, `conges::lignes_du_mois`). Rend la reponse de l'apercu et celle de l'ecriture.
fn corriger_le_cdi(racine: &Path, lignes: Value) -> (Value, Value) {
    let d = route_le(racine, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    assert_eq!(d["ok"], json!(true), "{}", d);
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(CDI)).unwrap().clone();
    let mut charge = json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": {"lignes": lignes}});
    let a = route_le(racine, JOUR, "regularisation-apercu", charge.clone());
    charge["motif"] = json!("la paie de juillet etait fausse");
    let w = route_le(racine, JOUR, "regularisation-ecrire", charge);
    (a, w)
}

/// Juillet sans la prime de tournee : le mois seul, comme `conges::lignes_du_mois` l'ecrit (3 120,00 pour 151,67 h).
fn juillet_sans_la_prime() -> Value {
    json!([{"rubrique": "mois", "quantite": 1, "taux": 3120.0, "heures": 151.67}])
}

/// Les 81.004 (en centimes) des lignes 018 de l'Urssaf d'un versement : la reduction generale que l'Urssaf recouvre.
fn reduction_a_l_urssaf(v: &[(String, String)]) -> i64 {
    -somme(v, "S21.G00.81.004", &|c| c.code81 == "018" && c.org81 == URSSAF)
}

#[test]
fn la_correction_d_un_cdi_passe_l_apercu_et_s_ecrit_sans_date_de_fin() {
    // Avant la vague 3a, l'apercu ecrivait `"fin": "None"` (texte) pour un contrat sans fin, et la paie corrigee refusait
    // « date illisible : 'None' » : aucun permanent ne se corrigeait. Le contrat declare garde ses dates et son numero
    // (regle 4) ; sans fin, il reste sans fin.
    let r = copie("cdi-sans-fin");
    let (a, w) = corriger_le_cdi(&r, juillet_sans_la_prime());
    assert_eq!(a["ok"], json!(true), "{}", a);
    let e = &a["resultat"]["engagement"];
    assert_eq!(e["fin"], Value::Null, "un CDI n'a pas de fin : null, pas le texte « None »");
    assert_eq!(e["debut"], json!("2023-09-04"));
    assert_eq!(e["numero"], json!(CDI));
    assert_eq!(e["nature"], json!("cdi"));
    assert_eq!(a["resultat"]["avant"]["brut"], json!("3370.00"));
    assert_eq!(a["resultat"]["apres"]["brut"], json!("3120.00"));
    assert_eq!(a["resultat"]["rien_ne_change"], json!(false));
    assert_eq!(w["ok"], json!(true), "{}", w);

    // Ce qui est ecrit se relit tel quel : la correction en cours de la page, et un apercu de ce meme engagement.
    let ecrite = lire(&r, "regularisations/2026-07.json")["corrections"][0]["engagement"].clone();
    assert_eq!(&ecrite, e, "l'ecriture garde l'engagement de l'apercu");
    let d = route_le(&r, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|x| x["numero"] == json!(CDI)).unwrap().clone();
    assert_eq!(item["fin"], Value::Null, "regularisation-mois : un CDI n'a pas de fin (null, pas « None »)");
    assert_eq!(item["correction_en_cours"]["engagement"], ecrite);
    let relu = route_le(&r, JOUR, "regularisation-apercu", json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": ecrite}));
    assert_eq!(relu["ok"], json!(true), "{}", relu);
    assert_eq!(relu["resultat"]["apres"]["brut"], json!("3120.00"));
    assert_eq!(relu["resultat"]["engagement"]["fin"], Value::Null);
}

#[test]
fn la_correction_d_un_cdi_annule_le_declare_et_son_bordereau_porte_la_reduction_comme_le_nominatif() {
    // Le declare de juillet est relu (vagues 0 et 0b) ; le rappel est calcule par le moteur du jour ; le bordereau Urssaf
    // de juillet que porte octobre est, CTP par CTP, la difference du nominatif. Le CTP 668 « reduction generale
    // etendue » d'un permanent porte la somme de SES 018 de l'Urssaf, part securite sociale et part chomage : « La somme
    // des donnees individuelles (S21.G00.81.004 : code 018) doit etre egale aux donnees agregees (S21.G00.23.005) »,
    // « le montant agrege contient uniquement les montants Urssaf et assurance chomage » (guide declaratif Urssaf, § 8.2,
    // atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes 1289-1290, 1300-1311 et 1355-1356).
    let r = copie("cdi-bordereau");
    let depose = versement_de(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), NIR_MO, CDI, JUILLET_MOIS);
    let deux = lignes_en_contexte(&depose).into_iter().filter(|(k, _, c)| k == "S21.G00.81.004" && c.code81 == "018" && c.org81 == URSSAF).count();
    assert_eq!(deux, 2, "juillet porte deux 018 de l'Urssaf pour le CDI");
    let (_, w) = corriger_le_cdi(&r, juillet_sans_la_prime());
    assert_eq!(w["ok"], json!(true), "{}", w);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement_de(&octobre, NIR_MO, CDI, JUILLET_MOIS);

    // Le nominatif : l'annulation de ce qui a ete declare, ligne pour ligne, puis le rappel du moteur du jour.
    verifier_l_annulation(&depose, &regul);
    let apres = rappel(&depose, &regul);
    for b in ["02", "03", "07"] {
        assert_eq!(bases_78(&apres, b), vec!["3120.00"], "base {} du rappel", b);
    }
    // Comme pour un CDDU paye le mois de sa correction (vague 2c) : un seul bloc 40, deux numeros de versement
    // (S21.G00.50.003, CCH-11), et le rappel cite des affiliations (78.005) que ce bloc 40 declare (70.012).
    let elle = individu(&octobre, NIR_MO);
    assert_eq!(elle.iter().filter(|(k, v)| k == "S21.G00.40.009" && v == CDI).count(), 1);
    assert_eq!(valeur(&versement_de(&octobre, NIR_MO, CDI, OCTOBRE_MOIS), "S21.G00.50.003"), "01");
    assert_eq!(valeur(&regul, "S21.G00.50.003"), "02");
    let declarees: Vec<&String> = elle.iter().filter(|(k, _)| k == "S21.G00.70.012").map(|(_, v)| v).collect();
    let citees: Vec<&String> = regul.iter().filter(|(k, _)| k == "S21.G00.78.005").map(|(_, v)| v).collect();
    assert!(!citees.is_empty() && citees.iter().all(|a| declarees.contains(a)), "78.005 {:?} parmi {:?}", citees, declarees);

    // Le bordereau de juillet porte par octobre : la table des CTP d'un permanent (config/organismes.json de la copie).
    let d_brut = somme(&regul, "S21.G00.51.013", &|c| c.type51 == "001");
    let d_base = |b: &str| somme(&regul, "S21.G00.78.004", &|c| c.b78 == b);
    let d_rg = reduction_a_l_urssaf(&regul);
    assert!(d_brut < 0 && d_rg > 0, "sans la prime, le brut baisse et la reduction monte : {} / {}", d_brut, d_rg);
    let mut attendu: Vec<(String, String, i64)> = [
        ("027", "920", d_brut),
        ("260", "920", d_base("04")),
        ("992", "920", d_brut),
        ("100", "920", d_brut),
        ("100", "921", d_base("02")),
        ("635", "920", d_brut),
        ("430", "920", d_brut),
        ("332", "921", d_base("02")),
        ("772", "920", d_base("07")),
        ("937", "920", d_base("07")),
        ("959", "920", d_brut),
        reduction_ou_sa_regularisation("668", "669", d_rg),
    ]
    .iter()
    .map(|(c, q, v)| (c.to_string(), q.to_string(), a_l_euro(*v)))
    .filter(|(_, _, v)| *v != 0)
    .collect();
    attendu.sort();
    assert_eq!(bordereau(&octobre, "01072026"), attendu);

    // Les versements 20 hors Urssaf et le bloc 55 portent aussi la difference du nominatif (vague 0b) : les deux 018 de
    // l'Urssaf vont a la meme cle de totalisation (`018:urssaf`), la prevoyance et la sante du permanent a ses adhesions.
    let mut ecarts = ecarts_des_versements_20(&r, &octobre);
    let (b55, _) = bloc_55(&octobre);
    let aff = par_contrat_de_prevoyance(&octobre);
    if b55 != aff {
        ecarts.push(format!("bloc 55 : ecrit {:?}, nominatif {:?}", b55, aff));
    }
    assert!(ecarts.is_empty(), "l'agrege ne porte pas la difference du nominatif :\n{}", ecarts.join("\n"));

    // Le 668 du mois courant suit la meme regle : la somme des 018 de l'Urssaf des permanents payes en octobre.
    let courant: i64 = [(NIR_MO, CDI), (NIR_NI, "NI.24.01")].iter().map(|(n, c)| reduction_a_l_urssaf(&versement_de(&octobre, n, c, OCTOBRE_MOIS))).sum();
    let ecrit = bordereau(&octobre, OCTOBRE_MOIS).into_iter().find(|(c, _, _)| c == "668").map(|(_, _, v)| v);
    assert_eq!(ecrit, Some(a_l_euro(courant)), "CTP 668 d'octobre");
}

#[test]
fn un_permanent_corrige_au_dessus_de_quatre_plafonds_garde_une_base_07_a_quatre_plafonds() {
    // Vague 2e : le chomage et l'AGS d'un permanent s'assoient dans la limite de quatre plafonds (reglement general
    // d'assurance chomage, art. 49 ; atelier/reference/sources/chomage-permanents/LISEZ-MOI.md). Juillet est parti a
    // 3 370,00 (base 07 = brut) ; corrige a 20 000,00, le rappel declare une base 07 de 4 x 4 005 = 16 020,00, et le
    // bordereau de juillet porte 16 020 - 3 370 = 12 650 aux CTP 772 et 937 (la base 07 relue, `assiette_declaree`).
    let r = copie("cdi-quatre-plafonds");
    // Le fichier des organismes recompose depuis donnees/organismes-base.json : 772 et 937 sur la base 07.
    let mut o = lire(&r, "config/organismes.json");
    for c in o["ctp_urssaf"].as_array_mut().unwrap() {
        if c["code"] == json!("772") || c["code"] == json!("937") {
            c["assiette"] = json!("chomage_permanent");
        }
    }
    ecrire_json(&r, "config/organismes.json", &o);
    let depose = versement_de(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), NIR_MO, CDI, JUILLET_MOIS);
    assert_eq!(bases_78(&depose, "07"), vec!["3370.00"]);
    let (_, w) = corriger_le_cdi(&r, json!([{"rubrique": "mois", "quantite": 1, "taux": 20000.0, "heures": 151.67}]));
    assert_eq!(w["ok"], json!(true), "{}", w);
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement_de(&octobre, NIR_MO, CDI, JUILLET_MOIS);
    verifier_l_annulation(&depose, &regul);
    let apres = rappel(&depose, &regul);
    assert_eq!(bases_78(&apres, "03"), vec!["20000.00"]);
    assert_eq!(bases_78(&apres, "02"), vec!["4005.00"]);
    assert_eq!(bases_78(&apres, "07"), vec!["16020.00"], "la base 07 du rappel, a quatre plafonds");
    for code in ["040", "048"] {
        let assiettes: Vec<String> = lignes_en_contexte(&apres).into_iter().filter(|(k, _, c)| k == "S21.G00.81.003" && c.code81 == code).map(|(_, x, _)| x).collect();
        assert_eq!(assiettes, vec!["16020.00"], "81.003 de la cotisation {}", code);
    }
    let b = bordereau(&octobre, "01072026");
    let ligne = |code: &str, q: &str| b.iter().find(|(c, x, _)| c == code && x == q).map(|(_, _, v)| *v);
    assert_eq!(ligne("772", "920"), Some(1_265_000));
    assert_eq!(ligne("937", "920"), Some(1_265_000));
    assert_eq!(ligne("100", "920"), Some(1_663_000), "le deplafonne sur le brut entier");
    assert_eq!(ligne("100", "921"), Some(a_l_euro(400_500 - 337_000)), "le plafonne sur un plafond");
    // A 20 000,00 la reduction generale tombe a zero : elle BAISSE. Sa regularisation passe au CTP 669, en assiette
    // (23.004) et sans signe, pas au 668 en montant negatif (guide declaratif Urssaf 5.1, § 8.2, schema de l'attendu
    // declaratif, atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes 1341-1362).
    let d_rg = reduction_a_l_urssaf(&regul);
    assert!(d_rg < 0, "la reduction baisse : {}", d_rg);
    assert_eq!(ligne("668", "921"), None, "pas de 668 negatif");
    assert_eq!(ligne("669", "921"), Some(a_l_euro(-d_rg)), "la baisse de la reduction en 669, sans signe");
    let i = octobre.iter().position(|(k, v)| k == "S21.G00.23.001" && v == "669").expect("une ligne 669");
    let rubriques: Vec<&str> = octobre[i + 1..].iter().take_while(|(k, _)| k != "S21.G00.23.001" && k.starts_with("S21.G00.23.")).map(|(k, _)| k.as_str()).collect();
    assert_eq!(rubriques, vec!["S21.G00.23.002", "S21.G00.23.004"], "le 669 porte une assiette, pas un montant");
}

/// UN CONTRAT OUBLIE, AJOUTE APRES LA LIMITE (21/10/2026). Servandoni a aussi travaille du 27 au 29 juillet 2026, et ce
/// contrat n'a jamais ete declare. Juillet est depose, son annule-et-remplace est passe : le contrat s'ajoute a la saisie
/// de juillet (en le confirmant), sa correction s'ecrit (« Corriger ce mois », qui lui donne son numero), et la DSN
/// d'octobre le declare en entier -- cahier technique DSN 2026.1, §2.1.2.1 (« une correction est un evenement connu apres
/// la date de cloture de la paie. Il donne lieu a la declaration d'une correction dans la DSN suivante ») et §2.2.2.5
/// (« les corrections doivent etre portees par une DSN mensuelle ulterieure [...] valorisees mois civil par mois civil »),
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` ; fiche net-entreprises 1741 (un contrat
/// que le systeme DSN ne connait pas encore se declare en entier dans la DSN mensuelle qui le porte). Son bloc 40 porte sa
/// date de debut passee (S21.G00.40.001) ; son versement, des remunerations (S21.G00.51.001) et des bases
/// (S21.G00.78.002) rattachees a juillet, toutes positives : rien n'a ete declare, rien ne s'annule.
#[test]
fn un_contrat_oublie_ajoute_apres_la_limite_se_declare_en_entier_dans_la_dsn_suivante() {
    let r = copie("contrat-oublie");
    let mut saisie = crate::outils::saisie(&r, "2026-07");
    let mut e = saisie["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(CONTRAT)).unwrap().clone();
    let o = e.as_object_mut().unwrap();
    o.remove("numero");
    o.insert("debut".into(), json!("2026-07-27"));
    o.insert("fin".into(), json!("2026-07-29"));
    o.insert("date_paiement".into(), json!("2026-07-29"));
    o.insert(
        "lignes".into(),
        json!([{"rubrique": "heure", "quantite": 24, "taux": 18.0, "lieu": "Festival fictif d'Avignon, Gymnase",
                "dates": ["2026-07-27", "2026-07-28", "2026-07-29"]},
               {"rubrique": "defraiement_forfait", "quantite": 3, "taux": 20.7}]),
    );
    saisie["engagements"].as_array_mut().unwrap().push(e);
    let ecrire = |confirme: bool| route_le(&r, JOUR, "enregistrer-saisie", json!({"mois": "2026-07", "saisie": saisie, "modifier_apres_echeance": confirme}));
    assert_eq!(ecrire(false)["classe"], json!("ModificationApresEcheance"));
    assert_eq!(ecrire(true)["ok"], json!(true));
    // Valide, il recoit son numero.
    let v = route_le(&r, JOUR, "valider-engagement", json!({"mois": "2026-07", "salarie": "SERVANDONI", "debut": "2026-07-27"}));
    assert_eq!(v["ok"], json!(true), "{}", v);

    // La regularisation du mois le liste, sans rien de declare ; sa correction le declare en entier, sous son numero.
    let d = route_le(&r, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["ajoute"] == json!(true)).unwrap().clone();
    assert_eq!(item["debut"].as_str(), Some("2026-07-27"), "{}", item);
    let charge = json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": {"lignes": item["engagement_saisi"]["lignes"]}});
    let a = route_le(&r, JOUR, "regularisation-apercu", charge.clone());
    assert_eq!((a["resultat"]["avant"]["brut"].as_str(), a["resultat"]["apres"]["brut"].as_str()), (Some("0.00"), Some("432.00")), "{}", a);
    let mut ecrite = charge;
    ecrite["motif"] = json!("contrat oublie");
    let w = route_le(&r, JOUR, "regularisation-ecrire", ecrite);
    assert_eq!(w["ok"], json!(true), "{}", w);
    let numero = w["resultat"]["correction"]["engagement"]["numero"].as_str().unwrap().to_string();
    assert!(numero.starts_with("SE9.26.") && numero != CONTRAT, "{}", numero);
    let pose = crate::outils::saisie(&r, "2026-07")["engagements"].as_array().unwrap().iter().any(|e| e["numero"] == json!(numero));
    assert!(pose, "le numero est celui de la saisie de juillet");

    // L'etape DSN de juillet n'a plus rien a faire.
    let b = route_le(&r, JOUR, "bilan-du-mois", json!({"mois": "2026-07"}));
    assert_eq!(b["resultat"]["dsn"]["remplacement"]["correction_ecrite"], json!(true), "{}", b["resultat"]["dsn"]);

    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    // Le bloc 40 du contrat oublie, a sa date de debut passee.
    let rang = octobre.iter().position(|(k, v)| k == "S21.G00.40.009" && *v == numero).expect("le contrat oublie est declare");
    let debut = octobre[..rang].iter().rev().find(|(k, _)| k == "S21.G00.40.001").map(|(_, v)| v.as_str());
    assert_eq!(debut, Some("27072026"));
    // Et sa fin, comme pour tout contrat termine : le bloc 62 (« Evenement de fin du contrat de travail », cahier technique ;
    // fiche 1722 : une fin se declare, ou se corrige, par le bloc 62 de la DSN mensuelle suivante).
    let c = le_contrat(&octobre, &numero);
    assert_eq!(valeur(&c, "S21.G00.62.001"), "29072026");
    assert_eq!(valeur(&c, "S21.G00.62.002"), "031");
    // Son versement : rattache a juillet, sans rien d'annule.
    let v = versement_du(&octobre, &numero, "27072026");
    assert_eq!(valeur(&v, "S21.G00.78.002"), "27072026");
    // Aucune annulation : une seule remuneration de chaque type, et ni remuneration, ni activite, ni base, ni montant net
    // social negatifs (les cotisations 81.004 negatives sont des reductions, pas des annulations).
    assert_eq!(v.iter().filter(|(k, x)| k == "S21.G00.51.011" && x == "001").count(), 1, "{:?}", v);
    let sans_signe = ["S21.G00.51.013", "S21.G00.53.002", "S21.G00.54.002", "S21.G00.58.004", "S21.G00.78.004"];
    assert!(v.iter().filter(|(k, _)| sans_signe.contains(&k.as_str())).all(|(_, x)| !x.starts_with('-')), "{:?}", v);
    assert_eq!(valeur(&v, "S21.G00.51.013"), "432.00");
}

/// UN CONTRAT DECLARE QUI N'A PAS EU LIEU (21/10/2026). Il ne se retire pas de la saisie apres la limite (la norme n'annule
/// pas un contrat declare : seule une fin de contrat s'annule, S21.G00.62.002 = 099, fiche net-entreprises 1722) ; ses
/// quantites passent a zero, et sa correction annule ses montants -- un rappel par annulation (cahier technique DSN
/// 2026.1, §2.1.3.6 : « un premier rappel de -1000€ et un second rappel de 1200€ » ; ici le second est nul).
/// La correction s'ecrit, son apercu annule tout le declare, et la DSN d'octobre la porte : quand octobre verse trop peu
/// a un organisme pour couvrir l'annulation, son versement S21.G00.20.005 (jamais negatif) est a zero, et l'ordre de
/// paiement dit le credit et ce que la compagnie en fait (`bordereau.rs`, « le trop-verse »).
#[test]
fn un_contrat_qui_n_a_pas_eu_lieu_se_corrige_a_zero() {
    let r = copie("contrat-a-zero");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    let mut saisie = crate::outils::saisie(&r, "2026-07");
    let rang = saisie["engagements"].as_array().unwrap().iter().position(|e| e["numero"] == json!(CONTRAT)).unwrap();
    for l in saisie["engagements"][rang]["lignes"].as_array_mut().unwrap() {
        l["quantite"] = json!(0);
    }
    crate::outils::remettre_en_saisie(&r, "2026-07", Some(CONTRAT));
    let w = route_le(&r, JOUR, "enregistrer-saisie", json!({"mois": "2026-07", "saisie": saisie, "modifier_apres_echeance": true}));
    assert_eq!(w["ok"], json!(true), "{}", w);
    let d = route_le(&r, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(CONTRAT)).unwrap().clone();
    let c = json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": {"lignes": item["engagement_saisi"]["lignes"]}});
    let a = route_le(&r, JOUR, "regularisation-apercu", c.clone());
    assert_eq!(a["resultat"]["apres"]["brut"], json!("0.00"), "{}", a);
    assert_eq!(centimes(a["resultat"]["delta"]["brut"].as_str().unwrap()), -centimes(valeur(&depose, "S21.G00.51.013")));
    let mut c = c;
    c["motif"] = json!("le contrat n'a pas eu lieu");
    let w = route_le(&r, JOUR, "regularisation-ecrire", c);
    assert_eq!(w["ok"], json!(true), "{}", w);
    let b = route_le(&r, JOUR, "bilan-du-mois", json!({"mois": "2026-07"}));
    assert_eq!(b["resultat"]["dsn"]["remplacement"]["correction_ecrite"], json!(true), "{}", b["resultat"]["dsn"]);

    // LA DSN D'OCTOBRE porte la correction, et se produit : elle a passe les controles d'usages et de formats du moteur
    // (`dsn::generer`, `usages::verifier_fichier` et `formats::verifier_fichier`), qui refusaient « S21.G00.20.005 =
    // '-65.72' ». Octobre verse trop peu aux Conges Spectacles et a France Travail pour couvrir l'annulation de juillet :
    // leur versement est a zero -- « non signe, valeur zero autorisee » (cahier technique DSN 2026.1, p. 156,
    // `norme-dsn/texte/bloc-S21.G00.20.txt`) -- et le credit reste declare dans les cotisations individuelles (81.004).
    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement(&octobre, JUILLET);
    assert!(!regul.is_empty(), "octobre porte la correction de juillet");
    // Aucun versement negatif.
    let versements: Vec<(String, String)> = {
        let mut out = Vec::new();
        let mut org = String::new();
        for (k, v) in &octobre {
            if k == "S21.G00.20.001" {
                org = v.clone();
            } else if k == "S21.G00.20.005" {
                out.push((org.clone(), v.clone()));
            }
        }
        out
    };
    assert!(versements.iter().all(|(_, m)| !m.starts_with('-')), "{:?}", versements);
    // Les cotisations individuelles d'octobre dues a chacun (81.004 : par 81.002, ou par le code 066 des Conges
    // Spectacles, « non concerne » en 81.002) : negatives, et leur versement est a zero.
    let du = |org: &str, code_sans_81_002: Option<&str>| -> i64 {
        let mut s = 0;
        let mut courant: Option<(String, Option<String>)> = None;
        for (k, v) in &octobre {
            match k.as_str() {
                "S21.G00.81.001" => courant = Some((v.clone(), None)),
                "S21.G00.81.002" => {
                    if let Some(c) = courant.as_mut() {
                        c.1 = Some(v.clone())
                    }
                }
                "S21.G00.81.004" => {
                    if let Some((code, o)) = &courant {
                        let a_lui = match o {
                            Some(o) => o == org,
                            None => Some(code.as_str()) == code_sans_81_002,
                        };
                        if a_lui {
                            s += centimes(v);
                        }
                    }
                }
                _ => {}
            }
        }
        s
    };
    const CONGES_SPECTACLES: &str = "77567608300030";
    let credits = [(CONGES_SPECTACLES, du(CONGES_SPECTACLES, Some("066"))), (FRANCE_TRAVAIL, du(FRANCE_TRAVAIL, None))];
    for (org, d) in credits {
        assert!(d < 0, "{} : {}", org, d);
        assert_eq!(versements.iter().filter(|(o, _)| o == org).map(|(_, m)| m.as_str()).collect::<Vec<_>>(), vec!["0.00"], "{}", org);
    }

    // L'ORDRE DE PAIEMENT DIT LE CREDIT, et ce que la compagnie en fait (`ordres_paiement::decrire`, `bordereau::credit`).
    let a = route_le(&r, JOUR, "depot-apercu", json!({"quoi": "dsn", "fichier": "sorties/2026-10/DSN_Octobre2026.dsn"}));
    let ordres = a["resultat"]["ordres"].as_array().unwrap_or_else(|| panic!("{}", a));
    // Le recapitulatif de la meme boite : le contrat de juillet n'est dans le fichier que pour sa correction, ce n'est
    // pas un ecart avec la paie d'octobre (`apercu_dsn::recapitulatif_dsn`).
    let recap = &a["resultat"]["recapitulatif"];
    assert_eq!((recap["corrections"].as_u64(), recap["conforme"].as_bool()), (Some(1), Some(true)), "{}", recap);
    let corrige = recap["salaries"].as_array().unwrap().iter().flat_map(|s| s["contrats"].as_array().unwrap()).find(|c| c["numero"] == json!(CONTRAT)).unwrap();
    assert_eq!((&corrige["correction"], &corrige["ecarts"]), (&json!(true), &json!([])), "{}", corrige);
    for (org, d) in credits {
        let o = ordres.iter().find(|o| o["identifiant"] == json!(org)).unwrap_or_else(|| panic!("{} : {:?}", org, ordres));
        assert_eq!(o["montant"], json!("0.00"));
        assert_eq!(centimes(o["credit"]["reste"].as_str().unwrap()), -d, "{}", o);
        let dit = o["credit"]["que_faire"].as_str().unwrap();
        assert!(dit.contains("Rien à payer") && dit.contains("demandez"), "{}", dit);
    }
    // L'Urssaf : le bordereau de juillet est negatif (22.005, admis pour une periode passee : CCH-11 de 22.005), et son
    // trop-verse se deduit du versement d'octobre, dans la limite du bordereau d'octobre (guide Urssaf, § 6.4).
    let bordereaux: Vec<(String, i64)> = {
        let mut out: Vec<(String, i64)> = Vec::new();
        for (k, v) in &octobre {
            match k.as_str() {
                "S21.G00.22.003" => out.push((v.clone(), 0)),
                "S21.G00.22.005" => out.last_mut().unwrap().1 = centimes(v),
                _ => {}
            }
        }
        out
    };
    let juillet = bordereaux.iter().find(|(d, _)| d == "01072026").map(|(_, m)| *m).unwrap();
    let courant = bordereaux.iter().find(|(d, _)| d == "01102026").map(|(_, m)| *m).unwrap();
    assert!(juillet < 0 && courant > -juillet, "{:?}", bordereaux);
    let u = ordres.iter().find(|o| o["identifiant"] == json!(URSSAF)).unwrap();
    assert_eq!(centimes(u["montant"].as_str().unwrap()), courant + juillet, "{}", u);
    assert_eq!((centimes(u["credit"]["deduit"].as_str().unwrap()), u["credit"]["reste"].as_str()), (-juillet, Some("0.00")), "{}", u);
    // Les autres ordres, payes en entier, ne portent pas de credit.
    let autres = ordres.iter().filter(|o| o["montant"] != json!("0.00") && o["identifiant"] != json!(URSSAF));
    assert!(autres.clone().count() > 0 && autres.into_iter().all(|o| o["credit"].is_null()), "{:?}", ordres);
}

/// Les dates de Servandoni changees dans la saisie de juillet apres la limite (confirme), puis sa correction ecrite
/// depuis « Corriger ce mois », et octobre produit. Rend la DSN d'octobre et la correction ecrite.
fn changer_les_dates_de_juillet(r: &Path, debut: &str, fin: &str, jours: &[&str]) -> (Vec<(String, String)>, Value) {
    let mut saisie = crate::outils::saisie(r, "2026-07");
    let rang = saisie["engagements"].as_array().unwrap().iter().position(|e| e["numero"] == json!(CONTRAT)).unwrap();
    let e = &mut saisie["engagements"][rang];
    e["debut"] = json!(debut);
    e["fin"] = json!(fin);
    e["date_paiement"] = json!(fin);
    e["lignes"][0]["dates"] = json!(jours);
    e["lignes"][0]["quantite"] = json!(8 * jours.len());
    crate::outils::remettre_en_saisie(r, "2026-07", Some(CONTRAT));
    let ecrire = |confirme: bool| route_le(r, JOUR, "enregistrer-saisie", json!({"mois": "2026-07", "saisie": saisie, "modifier_apres_echeance": confirme}));
    assert_eq!(ecrire(false)["classe"], json!("ModificationApresEcheance"));
    let w = ecrire(true);
    assert_eq!(w["ok"], json!(true), "{}", w);

    // « Corriger ce mois » : le contrat declare, a son premier jour declare, avec les dates de la saisie.
    let d = route_le(r, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(CONTRAT)).unwrap().clone();
    assert_eq!((item["debut"].as_str(), item["ajoute"].as_bool()), (Some("2026-07-02"), Some(false)), "{}", item);
    assert!(item["dates_saisies"].is_string(), "{}", item);
    let charge = json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": {"lignes": item["engagement_saisi"]["lignes"]}});
    let a = route_le(r, JOUR, "regularisation-apercu", charge.clone());
    assert_eq!(a["ok"], json!(true), "{}", a);
    assert_eq!((a["resultat"]["engagement"]["debut"].as_str(), a["resultat"]["engagement"]["fin"].as_str()), (Some(debut), Some(fin)), "{}", a);
    assert!(a["resultat"]["dates"].as_str().unwrap().starts_with("Dates corrigées : "), "{}", a);
    let mut ecrite = charge;
    ecrite["motif"] = json!("les dates du contrat etaient fausses");
    let w = route_le(r, JOUR, "regularisation-ecrire", ecrite);
    assert_eq!(w["ok"], json!(true), "{}", w);

    // L'etape DSN de juillet passe au vert : la correction est ecrite.
    let b = route_le(r, JOUR, "bilan-du-mois", json!({"mois": "2026-07"}));
    let dsn = &b["resultat"]["dsn"];
    assert_eq!((dsn["couleur"].as_str(), dsn["remplacement"]["correction_ecrite"].as_bool()), (Some("bien"), Some(true)), "{}", dsn);

    octobre_pret(r);
    let rep = dsn_d_octobre(r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    (lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn")), w["resultat"]["correction"].clone())
}

/// Le bloc 40 du contrat `CONTRAT` dans la DSN, et ce qui le suit jusqu'au contrat ou au versement suivant (41, 62, 70...).
fn contrat_et_ses_blocs(toutes: &[(String, String)]) -> Vec<(String, String)> {
    let rang = toutes.iter().position(|(k, v)| k == "S21.G00.40.009" && v == CONTRAT).expect("le contrat est declare");
    let debut = toutes[..rang].iter().rposition(|(k, _)| k == "S21.G00.40.001").unwrap();
    let fin = toutes[rang..].iter().position(|(k, _)| k == "S21.G00.40.001" || k == "S21.G00.50.001").map_or(toutes.len(), |i| rang + i);
    toutes[debut..fin].to_vec()
}

/// S21.G00.41.001 attendu : le jour ou la correction a ete ecrite, au plus tard le dernier jour d'octobre (CCH-15).
fn jour_de_la_correction(c: &Value) -> String {
    let iso = &c["decidee_le"].as_str().unwrap()[..10];
    let iso = if iso > "2026-10-31" { "2026-10-31" } else { iso };
    format!("{}{}{}", &iso[8..10], &iso[5..7], &iso[..4])
}

/// UNE DATE DE DEBUT DECLAREE, AVANCEE APRES LA LIMITE (21/10/2026). Servandoni a commence le 1er juillet, pas le 2 :
/// la saisie de juillet change apres la limite, sa correction s'ecrit, et la DSN d'octobre porte -- fiche net-entreprises
/// 1952 (`atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1952-correction-date-debut-contrat-20250326.txt`) --
/// le bloc 40 a la nouvelle date (S21.G00.40.001 = 01072026), et sous lui un bloc « Changements contrat » avec la date de
/// la modification (S21.G00.41.001, le jour de la correction) et l'ancienne date de debut, celle de la DSN deposee
/// (S21.G00.41.021 = 02072026), sans profondeur de recalcul (41.028 : interdite avec 41.021, fiche 1952). Le versement
/// annule ce que juillet a declare, a ses dates (51.001 = 02072026), et declare la paie corrigee a la periode corrigee
/// (51.001 = 01072026 ; cahier technique DSN 2026.1, §2.1.3.6). La fin n'a pas change : pas de bloc 62. La DSN passe les
/// controles d'usages et de formats du moteur (`produire` les joue avant d'ecrire).
#[test]
fn une_date_de_debut_avancee_apres_la_limite_se_declare_par_le_bloc_41() {
    let r = copie("debut-avance");
    let jours: Vec<String> = (1..=25).filter(|j| *j != 12).map(|j| format!("2026-07-{:02}", j)).collect();
    let jours: Vec<&str> = jours.iter().map(String::as_str).collect();
    let (octobre, c) = changer_les_dates_de_juillet(&r, "2026-07-01", "2026-07-25", &jours);
    let contrat = contrat_et_ses_blocs(&octobre);
    assert_eq!(contrat[0], ("S21.G00.40.001".to_string(), "01072026".to_string()));
    assert_eq!(valeur(&contrat, "S21.G00.40.010"), "25072026");
    let premier_41 = contrat.iter().position(|(k, _)| k.starts_with("S21.G00.41.")).expect("un bloc 41");
    assert!(contrat[..premier_41].iter().all(|(k, _)| k.starts_with("S21.G00.40.")), "le bloc 41 suit le bloc 40 : {:?}", contrat);
    let b41: Vec<(String, String)> = contrat.iter().filter(|(k, _)| k.starts_with("S21.G00.41.")).cloned().collect();
    assert_eq!(
        b41,
        vec![("S21.G00.41.001".to_string(), jour_de_la_correction(&c)), ("S21.G00.41.021".to_string(), JUILLET.to_string())]
    );
    assert!(!contrat.iter().any(|(k, _)| k.starts_with("S21.G00.62.")), "la fin n'a pas change : {:?}", contrat);

    let v = versement(&octobre, JUILLET);
    let periodes: Vec<&str> = v.iter().filter(|(k, _)| k == "S21.G00.51.001").map(|(_, x)| x.as_str()).collect();
    assert!(periodes.contains(&JUILLET) && periodes.contains(&"01072026"), "{:?}", periodes);
    // Le brut : -3312.00 annule, puis 24 jours de 8 heures a 18 EUR.
    let bruts: Vec<i64> = v
        .windows(3)
        .filter(|w| w[1].0 == "S21.G00.51.011" && w[1].1 == "001" && w[2].0 == "S21.G00.51.013")
        .map(|w| centimes(&w[2].1))
        .collect();
    assert_eq!(bruts, vec![-331200, 24 * 8 * 1800], "{:?}", v);
    assert!(v.iter().any(|(k, x)| k == "S21.G00.78.002" && x == "01072026"), "les bases corrigees, a la periode corrigee");
}

/// UNE DATE DE DEBUT RECULEE ET UNE FIN CORRIGEE APRES LA LIMITE (21/10/2026). Servandoni a commence le 3 juillet et fini
/// le 26 : le bloc 40 de la DSN d'octobre porte les deux nouvelles dates (S21.G00.40.001 = 03072026, S21.G00.40.010 =
/// 26072026), le bloc 41 l'ancienne date de debut (S21.G00.41.021 = 02072026, fiche net-entreprises 1952), et un bloc
/// « Fin du contrat » la fin corrigee (S21.G00.62.001 = 26072026) : « une correction de fin de contrat s'opere par
/// l'emission d'un bloc " Fin du contrat - S21.G00.62 " dans la DSN mensuelle suivante portant les informations correctes
/// qui ecraseront celles precedemment declarees » (fiche net-entreprises 1722,
/// `net-entreprises-fiche1722-annulation-fin-de-contrat-transmise-a-tort-20250326.txt`). La date de fin prevue n'a pas de
/// rubrique en bloc 41 : elle se redeclare en bloc 40 (cahier technique DSN 2026.1, §2.1.2.4.2).
#[test]
fn une_date_de_debut_reculee_et_une_fin_corrigee_apres_la_limite_se_declarent() {
    let r = copie("debut-recule");
    let jours: Vec<String> = (3..=26).filter(|j| *j != 12).map(|j| format!("2026-07-{:02}", j)).collect();
    let jours: Vec<&str> = jours.iter().map(String::as_str).collect();
    let (octobre, c) = changer_les_dates_de_juillet(&r, "2026-07-03", "2026-07-26", &jours);
    let contrat = contrat_et_ses_blocs(&octobre);
    assert_eq!(contrat[0], ("S21.G00.40.001".to_string(), "03072026".to_string()));
    assert_eq!(valeur(&contrat, "S21.G00.40.010"), "26072026");
    assert_eq!(valeur(&contrat, "S21.G00.41.001"), jour_de_la_correction(&c));
    assert_eq!(valeur(&contrat, "S21.G00.41.021"), JUILLET);
    assert!(!contrat.iter().any(|(k, _)| k == "S21.G00.41.028"));
    assert_eq!(valeur(&contrat, "S21.G00.62.001"), "26072026");
    let v = versement(&octobre, JUILLET);
    assert!(v.iter().any(|(k, x)| k == "S21.G00.51.001" && x == "03072026"), "{:?}", v);
    assert!(v.iter().any(|(k, x)| k == "S21.G00.51.002" && x == "26072026"), "{:?}", v);
}

// ------------------------------------------------------------ un contrat sur deux mois (21/10/2026)

/// Les depots reels de la DSN de `mois` au journal, retires (pour reproduire le mois), rendus pour etre remis.
fn retirer_les_depots_dsn(r: &Path, mois: &str) -> Vec<Value> {
    let mut j = lire(r, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let siens: Vec<Value> = l.iter().filter(|x| x["quoi"] == json!("dsn") && x["mois"] == json!(mois)).cloned().collect();
    l.retain(|x| !(x["quoi"] == json!("dsn") && x["mois"] == json!(mois)));
    ecrire_json(r, "journal/depots.json", &j);
    siens
}

fn remettre_les_depots(r: &Path, mut depots: Vec<Value>) {
    // L'empreinte de la saisie notee au depot est celle d'avant le contrat ajoute : le mois reproduit n'en a pas.
    for d in &mut depots {
        if let Some(detail) = d.get_mut("detail").and_then(Value::as_object_mut) {
            detail.remove("empreinte_saisie");
        }
    }
    let mut j = lire(r, "journal/depots.json");
    j.as_array_mut().unwrap().extend(depots);
    ecrire_json(r, "journal/depots.json", &j);
}

/// Un engagement de Lemercier (regisseuse) sur Ubu roi, du 28 juillet au 3 aout 2026, 8 heures par jour.
fn lemercier_a_cheval(numero: Option<&str>, jours: &[&str]) -> Value {
    let mut e = json!({
        "salarie": "Lemercier", "spectacle": "256Z00920011", "debut": "2026-07-28", "fin": "2026-08-03",
        "date_paiement": jours.last().unwrap(),
        "lignes": [{"rubrique": "heure", "quantite": 8 * jours.len(), "taux": 18.0, "lieu": "Festival fictif de Charleville, Gymnase", "dates": jours}],
    });
    if let Some(n) = numero {
        e["numero"] = json!(n);
    }
    e
}

const JUILLET_A_CHEVAL: [&str; 4] = ["2026-07-28", "2026-07-29", "2026-07-30", "2026-07-31"];
const AOUT_A_CHEVAL: [&str; 3] = ["2026-08-01", "2026-08-02", "2026-08-03"];
/// Le NIR (invente) de Lemercier, dans la 3090-petite.
const NIR_LEMERCIER: &str = "2950803999012";

/// La 3090-petite ou Lemercier a aussi un contrat du 28 juillet au 3 aout 2026, declare dans les DSN de juillet et
/// d'aout (reproduites avec lui, deposees et acceptees au journal comme les autres). Rend la racine et son numero.
fn contrat_a_cheval(test: &str) -> (PathBuf, String) {
    let r = compagnie::ses_dsn("fictive-3090-petite", &["2026-07", "2026-08"], &outils::dossier_de_test("regularisation_declaree", test).join("compagnie"));
    let depots_juillet = retirer_les_depots_dsn(&r, "2026-07");
    let depots_aout = retirer_les_depots_dsn(&r, "2026-08");
    let mut juillet = crate::outils::saisie(&r, "2026-07");
    juillet["engagements"].as_array_mut().unwrap().push(lemercier_a_cheval(None, &JUILLET_A_CHEVAL));
    crate::outils::poser_saisie(&r, "2026-07", &juillet);
    crate::outils::tout_valider(&r, "2026-07", "2026-08-05");
    mois_pret(&r, "2026-07");
    let rep = dsn_le(&r, "2026-07", "2026-08-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let numero = crate::outils::saisie(&r, "2026-07")["engagements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["debut"] == json!("2026-07-28"))
        .and_then(|e| e["numero"].as_str())
        .expect("le numero est ecrit dans la saisie de juillet a sa validation")
        .to_string();
    remettre_les_depots(&r, depots_juillet);
    outils::reussi(&route_le(&r, "2026-08-05", "envoyer-documents", json!({"mois": "2026-07", "motdepasse": "x"})));
    let mut aout = crate::outils::saisie(&r, "2026-08");
    // Le contrat de juillet court en aout : l'ecran d'aout le montre, ses jours d'aout s'y posent.
    let e = aout["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == json!(numero)).expect("aout montre le contrat de juillet");
    let a_cheval = lemercier_a_cheval(Some(&numero), &AOUT_A_CHEVAL);
    e["lignes"] = a_cheval["lignes"].clone();
    e["date_paiement"] = a_cheval["date_paiement"].clone();
    crate::outils::poser_saisie(&r, "2026-08", &aout);
    crate::outils::tout_valider(&r, "2026-08", "2026-09-05");
    let rep = dsn_le(&r, "2026-08", "2026-09-05");
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    remettre_les_depots(&r, depots_aout);
    outils::reussi(&route_le(&r, "2026-09-05", "envoyer-documents", json!({"mois": "2026-08", "motdepasse": "x"})));
    let aout = lignes(&r.join("sorties/2026-08/DSN_Aout2026.dsn"));
    assert!(aout.iter().any(|(k, v)| k == "S21.G00.40.009" && *v == numero), "la DSN d'aout declare le contrat");
    (r, numero)
}

/// La saisie de `mois`, le contrat `numero` change par `changer`, ecrite en confirmant.
fn changer_le_contrat(r: &Path, mois: &str, numero: &str, changer: &dyn Fn(&mut Value)) {
    let mut s = crate::outils::saisie(r, &mois);
    let e = s["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == json!(numero)).unwrap();
    changer(e);
    crate::outils::remettre_en_saisie(r, mois, Some(numero));
    let w = route_le(r, JOUR, "enregistrer-saisie", json!({"mois": mois, "saisie": s, "modifier_apres_echeance": true}));
    assert_eq!(w["ok"], json!(true), "{}", w);
}

/// Le bloc 40 du contrat `numero` (unique dans la DSN) et ce qui le suit jusqu'au contrat ou au versement suivant.
fn le_contrat(toutes: &[(String, String)], numero: &str) -> Vec<(String, String)> {
    let rangs: Vec<usize> = toutes.iter().enumerate().filter(|(_, (k, v))| k == "S21.G00.40.009" && v == numero).map(|(i, _)| i).collect();
    assert_eq!(rangs.len(), 1, "un seul bloc 40 du contrat {}", numero);
    let rang = rangs[0];
    let debut = toutes[..rang].iter().rposition(|(k, _)| k == "S21.G00.40.001").unwrap();
    let fin = toutes[rang..].iter().position(|(k, _)| k == "S21.G00.40.001" || k == "S21.G00.50.001").map_or(toutes.len(), |i| rang + i);
    toutes[debut..fin].to_vec()
}

/// UN CONTRAT SUR DEUX MOIS DONT LE DEBUT EST AVANCE APRES LA LIMITE. Le contrat de Lemercier du 28 juillet
/// au 3 aout commencait en fait le 27 : la saisie de juillet change (le 27 en plus), celle d'aout recoit la meme date de
/// debut (« La date de debut du contrat est un des identifiants du contrat », fiche net-entreprises 3344), et la DSN
/// d'octobre porte d'elle-meme la correction de chaque mois : UN bloc 40 du contrat a sa nouvelle date (S21.G00.40.001 =
/// 27072026), un bloc « Changements contrat » avec l'ancienne (S21.G00.41.021 = 28072026, fiche 1952), et un versement par
/// mois civil (cahier technique DSN 2026.1, §2.2.2.5) : juillet (l'annulation a 28072026, la paie corrigee a 27072026) et
/// aout (l'annulation et la redeclaration a 01082026). Pas de bloc 62 : la fin n'a pas change.
#[test]
fn un_contrat_sur_deux_mois_dont_le_debut_est_avance_se_corrige_mois_par_mois() {
    let (r, numero) = contrat_a_cheval("a-cheval-debut");
    changer_le_contrat(&r, "2026-07", &numero, &|e| {
        e["debut"] = json!("2026-07-27");
        e["lignes"][0]["dates"] = json!(["2026-07-27", "2026-07-28", "2026-07-29", "2026-07-30", "2026-07-31"]);
        e["lignes"][0]["quantite"] = json!(40);
    });
    let aout = crate::outils::saisie(&r, "2026-08");
    let e = aout["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(numero)).unwrap();
    assert_eq!((e["debut"].as_str(), e["fin"].as_str()), (Some("2026-07-27"), Some("2026-08-03")), "aout suit : {}", e);
    assert!(r.join("regularisations/2026-08.saisie-declaree.json").exists(), "la saisie d'aout telle que declaree est gardee");

    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let c = le_contrat(&octobre, &numero);
    assert_eq!(valeur(&c, "S21.G00.40.001"), "27072026");
    assert_eq!(valeur(&c, "S21.G00.40.010"), "03082026");
    assert_eq!(valeur(&c, "S21.G00.41.021"), "28072026");
    assert_eq!(c.iter().filter(|(k, _)| k == "S21.G00.41.021").count(), 1);
    assert!(!c.iter().any(|(k, _)| k.starts_with("S21.G00.62.")), "{:?}", c);
    let juillet = versement_de(&octobre, NIR_LEMERCIER, &numero, "28072026");
    assert!(juillet.iter().any(|(k, v)| k == "S21.G00.51.001" && v == "27072026"), "{:?}", juillet);
    let aout = versement_de(&octobre, NIR_LEMERCIER, &numero, "01082026");
    assert!(aout.iter().all(|(k, v)| k != "S21.G00.51.001" || v == "01082026"), "{:?}", aout);
}

/// UN CONTRAT SUR DEUX MOIS DONT LA FIN EST RECULEE DANS LE MOIS SUIVANT, APRES LA LIMITE. Le contrat de
/// Lemercier finissait le 5 aout, pas le 3 : la saisie d'aout change (le 4 et le 5 en plus), celle de juillet recoit la
/// meme fin. La DSN d'octobre porte un bloc 40 a la nouvelle fin (S21.G00.40.010 = 05082026) et un bloc « Fin du
/// contrat » corrige (S21.G00.62.001 = 05082026 : « une correction de fin de contrat s'opere par l'emission d'un bloc
/// " Fin du contrat - S21.G00.62 " dans la DSN mensuelle suivante portant les informations correctes », fiche
/// net-entreprises 1722) ; pas de bloc 41, le debut n'a pas change ; un versement par mois.
#[test]
fn un_contrat_sur_deux_mois_dont_la_fin_est_reculee_se_corrige_mois_par_mois() {
    let (r, numero) = contrat_a_cheval("a-cheval-fin");
    changer_le_contrat(&r, "2026-08", &numero, &|e| {
        e["fin"] = json!("2026-08-05");
        e["date_paiement"] = json!("2026-08-05");
        e["lignes"][0]["dates"] = json!(["2026-08-01", "2026-08-02", "2026-08-03", "2026-08-04", "2026-08-05"]);
        e["lignes"][0]["quantite"] = json!(40);
    });
    let juillet = crate::outils::saisie(&r, "2026-07");
    let e = juillet["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(numero)).unwrap();
    assert_eq!(e["fin"], json!("2026-08-05"), "juillet suit : {}", e);

    octobre_pret(&r);
    let rep = dsn_d_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let c = le_contrat(&octobre, &numero);
    assert_eq!(valeur(&c, "S21.G00.40.001"), "28072026");
    assert_eq!(valeur(&c, "S21.G00.40.010"), "05082026");
    assert_eq!(valeur(&c, "S21.G00.62.001"), "05082026");
    assert!(!c.iter().any(|(k, _)| k.starts_with("S21.G00.41.")), "{:?}", c);
    versement_de(&octobre, NIR_LEMERCIER, &numero, "28072026");
    let aout = versement_de(&octobre, NIR_LEMERCIER, &numero, "01082026");
    assert!(aout.iter().any(|(k, v)| k == "S21.G00.51.002" && v == "05082026"), "{:?}", aout);
}

/// Des dates qui sortiraient le contrat d'un mois ou il est saisi avec ses jours se refusent, et disent quoi faire.
#[test]
fn des_dates_qui_sortent_le_contrat_d_un_mois_saisi_se_refusent() {
    let (r, numero) = contrat_a_cheval("a-cheval-sorti");
    let mut s = crate::outils::saisie(&r, "2026-07");
    let e = s["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["numero"] == json!(numero)).unwrap();
    e["fin"] = json!("2026-07-31");
    crate::outils::remettre_en_saisie(&r, "2026-07", Some(&numero));
    let w = route_le(&r, JOUR, "enregistrer-saisie", json!({"mois": "2026-07", "saisie": s, "modifier_apres_echeance": true}));
    assert_eq!(w["ok"], json!(false), "{}", w);
    let message = w["erreur"].as_str().unwrap();
    assert!(message.contains("août 2026") || message.contains("aout 2026"), "le refus nomme le mois qui porte des jours : {}", message);
    assert_eq!(crate::outils::saisie(&r, "2026-07")["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(numero)).unwrap()["fin"], json!("2026-08-03"));
}
