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
//! de juillet. Aucun reseau, aucun depot.

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

/// Le bareme livre change depuis le depot de juillet : la cotisation FNAL (S21.G00.81.001 = 049) passe a 0,5 %.
/// Refaite aujourd'hui, la paie de juillet ne declarerait plus le montant FNAL qui est parti.
fn changer_le_bareme(racine: &Path) {
    let vus = retoucher_le_bareme(racine, &|o| {
        if o.get("code81") == Some(&json!("049")) && o.contains_key("taux") {
            o.insert("taux".into(), json!(0.5));
            return true;
        }
        false
    });
    assert!(vus > 0, "le bareme 2026 porte la cotisation FNAL");
}

fn produire_le(racine: &Path, mois: &str, jour: &str) -> Value {
    route_le(racine, jour, "produire", json!({"mois": mois, "saisie": lire(racine, &format!("paies/{}.json", mois))}))
}

fn produire_octobre(racine: &Path) -> Value {
    produire_le(racine, "2026-10", JOUR)
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

/// Les defraiements (54.002 des blocs 54 de type 07 ou 09) de lignes de versement, en centimes. Depuis la vague 1b, ils
/// ne sont ni dans le montant net social (58.004) ni dans le net verse (50.004) : pour ce contrat, qui n'a ni acompte
/// ni indemnite, 58.004 = 50.004 d'une paie du moteur.
fn frais(v: &[(String, String)]) -> i64 {
    let (mut type_54, mut total) = ("", 0);
    for (k, x) in v {
        match k.as_str() {
            "S21.G00.54.001" => type_54 = x.as_str(),
            "S21.G00.54.002" if type_54 == "07" || type_54 == "09" => total += centimes(x),
            _ => {}
        }
    }
    total
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

/// Les 58.004 d'un versement, avec leur 58.001.
fn nets_sociaux(v: &[(String, String)]) -> Vec<(String, i64)> {
    let mut debut = String::new();
    let mut out = Vec::new();
    for (k, x) in v {
        if k == "S21.G00.58.001" {
            debut = x.clone();
        } else if k == "S21.G00.58.004" {
            out.push((debut.clone(), centimes(x)));
        }
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

#[test]
fn les_blocs_d_annulation_portent_les_montants_du_fichier_depose_pas_ceux_du_recalcul() {
    let r = copie("annulation");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    corriger_juillet(&r);
    changer_le_bareme(&r);
    octobre_pret(&r);
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement(&octobre, JUILLET);

    // Chaque groupe commence par l'annulation du declare, ligne pour ligne, puis declare l'apres.
    verifier_l_annulation(&depose, &regul);

    // 78.005 cite une affiliation que la DSN d'octobre declare pour ce contrat, celle de la meme reference (70.013)
    // qu'en juillet.
    let affiliations = |toutes: &[(String, String)]| -> Vec<(String, String)> {
        let (mut du_contrat, mut id, mut out) = (false, None, Vec::new());
        for (k, v) in toutes {
            match k.as_str() {
                "S21.G00.40.001" | "S21.G00.50.001" => du_contrat = false,
                "S21.G00.40.009" => du_contrat = v == CONTRAT,
                "S21.G00.70.012" if du_contrat => id = Some(v.clone()),
                "S21.G00.70.013" if du_contrat => out.push((id.take().unwrap(), v.clone())),
                _ => {}
            }
        }
        out
    };
    let (aff_juillet, aff_octobre) = (affiliations(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn"))), affiliations(&octobre));
    let reference = |aff: &[(String, String)], id: &str| aff.iter().find(|(i, _)| i == id).map(|(_, rf)| rf.clone());
    let deposees: Vec<&String> = depose.iter().filter(|(k, _)| k == "S21.G00.78.005").map(|(_, v)| v).collect();
    assert!(!deposees.is_empty(), "le depot de juillet rattache des bases 31");
    let ecrites: Vec<&String> = regul.iter().filter(|(k, _)| k == "S21.G00.78.005").map(|(_, v)| v).take(deposees.len()).collect();
    for (d, e) in deposees.iter().zip(&ecrites) {
        assert_eq!(reference(&aff_juillet, d), reference(&aff_octobre, e), "78.005 {} (juillet) -> {} (octobre)", d, e);
        assert!(reference(&aff_octobre, e).is_some());
    }

    // Le versement de la correction : la difference entre le net verse d'apres et ce qui a ete DECLARE (50.004 depose).
    // Le net verse d'apres est le montant net social corrige, sans defraiements (vagues 1a et 1b). Vague 1o : juillet
    // est parti avec un 58.004 d'avant la vague 1b (le net a payer) ; il s'annule, et le montant net social de ce qui a
    // ete paye en juillet se declare aux dates de juillet (BOSS Q 41) ; le rappel, aux dates d'octobre (norme p. 265).
    let nets = nets_sociaux(&regul);
    assert_eq!(nets.len(), 3, "{:?}", nets);
    assert_eq!(nets[0], (JUILLET.to_string(), -centimes(valeur(&depose, "S21.G00.58.004"))));
    assert_eq!(nets[1].0, JUILLET);
    assert_eq!(nets[2].0, "01102026");
    let frais_apres = frais(&rappel(&depose, &regul));
    assert!(frais_apres > 0, "le contrat corrige a des defraiements");
    assert_eq!(nets[1].1, centimes(valeur(&depose, "S21.G00.58.004")) - frais(&depose), "ce qui a ete paye, sans les defraiements");
    assert_eq!(centimes(valeur(&regul, "S21.G00.50.004")), nets[1].1 + nets[2].1 - centimes(valeur(&depose, "S21.G00.50.004")));
}

#[test]
fn le_105_depose_s_annule_tel_quel_et_le_rappel_se_declare_en_131() {
    // Vague 1g : juillet 2026 est parti en 105 (« Agirc-Arrco, y compris Apec ») ; la norme 2026.1 attend 131 et 132
    // (cahier technique 2026.1 p. 302). La correction annule ce qui a ete DECLARE -- le 105, a son montant -- et declare
    // le corrige en 131 ; le versement Agirc-Arrco d'octobre porte l'un et l'autre.
    let r = copie("agirc-arrco");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    let codes_81 = |v: &[(String, String)]| -> Vec<(String, i64)> {
        lignes_en_contexte(v)
            .into_iter()
            .filter(|(k, _, c)| k == "S21.G00.81.004" && ["105", "131", "132"].contains(&c.code81.as_str()))
            .map(|(_, x, c)| (c.code81, centimes(&x)))
            .collect()
    };
    let declare = codes_81(&depose);
    assert_eq!(declare.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["105"], "le depot de juillet porte un 105");
    corriger_juillet(&r);
    octobre_pret(&r);
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    let regul = versement(&octobre, JUILLET);
    verifier_l_annulation(&depose, &regul);
    let ecrit = codes_81(&regul);
    assert_eq!(ecrit.len(), 2, "{:?}", ecrit);
    assert_eq!(ecrit[0], ("105".to_string(), -declare[0].1), "l'annulation reprend le 105 depose, a son montant");
    assert_eq!(ecrit[1].0, "131", "le rappel se declare en 131");
    assert!(ecrit[1].1 > declare[0].1, "l'heure passe de 18 a 19 EUR");
    // Hors du versement corrige, octobre n'ecrit aucun 105.
    let tous: Vec<&str> = octobre.iter().filter(|(k, _)| k == "S21.G00.81.001").map(|(_, v)| v.as_str()).collect();
    assert_eq!(tous.iter().filter(|c| **c == "105").count(), 1, "un seul 105 dans octobre : l'annulation");
    // Le versement Agirc-Arrco d'octobre : 105, 131, 132 et 106 de tout le fichier.
    let attendu: i64 = lignes_en_contexte(&octobre)
        .iter()
        .filter(|(k, _, c)| k == "S21.G00.81.004" && ["105", "106", "131", "132"].contains(&c.code81.as_str()))
        .map(|(_, x, _)| centimes(x))
        .sum();
    assert_eq!(versements_20(&octobre, "01102026").get("44832375800038").copied(), Some(attendu));
}

#[test]
fn sans_la_dsn_deposee_la_correction_refuse_et_ne_recalcule_pas() {
    let r = copie("sans-depot");
    corriger_juillet(&r);
    octobre_pret(&r);
    std::fs::remove_file(r.join("sorties/2026-07/DSN_Juillet2026.dsn")).unwrap();
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(false), "{}", rep);
    let message = rep["erreur"].as_str().unwrap_or_default();
    assert!(message.contains("Correction de 2026-07") && message.contains(CONTRAT), "{}", message);
    assert!(message.contains("Remettre dans sorties/2026-07/"), "{}", message);
    assert!(!r.join("sorties/2026-10/DSN_Octobre2026.dsn").exists());
}

// ------------------------------------------------------------------ (b) un annule-et-remplace : la derniere fait foi

#[test]
fn deux_dsn_reelles_acceptees_pour_le_mois_d_origine_la_derniere_fait_foi() {
    // Un annule-et-remplace accepte de juillet (DSN_Juillet2026_AR.dsn), ou le versement de Servandoni differe du
    // premier depot : ses cotisations et son net ont chacun un euro de plus. Il s'est substitue au premier depot : c'est
    // lui que la correction annule (Leandre, 10/10/2026).
    let r = copie("deux-depots");
    let premier = lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn"));
    let mut dans_servandoni = false;
    let mut ar = String::new();
    let brut = std::fs::read(r.join("sorties/2026-07/DSN_Juillet2026.dsn")).unwrap();
    for l in String::from_utf8_lossy(&brut).split_inclusive('\n') {
        let (k, v) = l.trim_end().split_once(',').unwrap_or(("", ""));
        if k == "S21.G00.30.001" {
            dans_servandoni = v.trim_matches('\'') == NIR;
        }
        if dans_servandoni && ["S21.G00.81.004", "S21.G00.50.004", "S21.G00.58.004"].contains(&k) {
            let n = centimes(v.trim_matches('\'')) + 100;
            ar.push_str(&format!("{},'{}'{}", k, texte_centimes(n), if l.ends_with("\r\n") { "\r\n" } else { "\n" }));
        } else {
            ar.push_str(l);
        }
    }
    std::fs::write(r.join("sorties/2026-07/DSN_Juillet2026_AR.dsn"), ar.as_bytes()).unwrap();
    let remplace = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026_AR.dsn")), JUILLET);
    let depose = versement(&premier, JUILLET);
    assert_ne!(annulation_attendue(&depose, &["78", "79", "81"]), annulation_attendue(&remplace, &["78", "79", "81"]));

    corriger_juillet(&r);
    octobre_pret(&r);
    let mut j = lire(&r, "journal/depots.json");
    let l = j.as_array_mut().unwrap();
    let mut reel = l.iter().find(|x| x["quoi"] == json!("dsn") && x["mois"] == json!("2026-07") && x["mode"] == json!("reel")).unwrap().clone();
    reel["fichier"] = json!("DSN_Juillet2026_AR.dsn");
    reel["quand"] = json!("2026-08-20T10:00:00");
    reel["detail"]["type"] = json!("03");
    l.push(reel);
    ecrire_json(&r, "journal/depots.json", &j);

    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let regul = versement(&lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn")), JUILLET);
    verifier_l_annulation(&remplace, &regul);
    // Vague 1o : l'annulation du 58 de l'annule-et-remplace et le montant net social de ce qui a ete paye, aux dates de
    // juillet ; le rappel, aux dates d'octobre.
    let nets = nets_sociaux(&regul);
    assert_eq!(nets.len(), 3, "{:?}", nets);
    assert_eq!(nets[0].1, -centimes(valeur(&remplace, "S21.G00.58.004")));
    assert_eq!(nets[2].0, "01102026");
    let frais_apres = frais(&rappel(&remplace, &regul));
    assert!(frais_apres > 0, "le rappel declare ses defraiements en 54, hors du 58.004");
    assert_eq!(centimes(valeur(&regul, "S21.G00.50.004")), nets[1].1 + nets[2].1 - centimes(valeur(&remplace, "S21.G00.50.004")));
}

// ------------------------------------------------- (c) une seconde correction relit le rappel de la premiere

#[test]
fn une_seconde_correction_annule_le_rappel_porte_par_la_premiere_pas_le_depot_d_origine() {
    // Premiere correction (18 -> 19 EUR) portee par la DSN d'octobre, deposee et acceptee ; seconde (19 -> 20 EUR)
    // portee par celle de novembre. Ce qui a ete declare pour le contrat, quand la seconde se porte, c'est le rappel
    // d'octobre : c'est lui que novembre annule.
    let r = copie("seconde-correction");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    corriger_juillet(&r);
    octobre_pret(&r);
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let regul_octobre = versement(&lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn")), JUILLET);
    let rappel_octobre = rappel(&depose, &regul_octobre);

    // Octobre est depose en reel et accepte ; la correction est portee.
    outils::dsn_deposee(&r, "2026-10", "DSN_Octobre2026.dsn", "2026-11-12T10:00:00");
    let mut c = lire(&r, "regularisations/2026-07.json");
    let corrections = c["corrections"].as_array_mut().unwrap();
    assert_eq!(corrections.len(), 1);
    corrections[0]["portee_par"] = json!({"mois": "2026-10", "fichier": "DSN_Octobre2026.dsn", "quand": "2026-11-12T10:00:00"});
    ecrire_json(&r, "regularisations/2026-07.json", &c);

    let jour = "2026-12-10";
    corriger_juillet_a(&r, jour, 20.0);
    mois_pret(&r, "2026-11");
    let rep = produire_le(&r, "2026-11", jour);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let regul = versement(&lignes(&r.join("sorties/2026-11/DSN_Novembre2026.dsn")), JUILLET);

    // Novembre annule le rappel d'octobre, ligne pour ligne -- et non le depot de juillet.
    verifier_l_annulation(&rappel_octobre, &regul);
    assert_ne!(annulation_attendue(&depose, &["51", "53"]), annulation_attendue(&rappel_octobre, &["51", "53"]));
    // Le net : ce qui a ete declare jusqu'ici (juillet, plus le rappel d'octobre) s'annule, la difference se verse.
    // 50.004 cumule = le net verse corrige en octobre, c'est-a-dire son montant net social (vague 1b). Vague 1o : en
    // octobre, le montant net social de ce qui avait ete paye en juillet (aux dates de juillet) plus le rappel (aux dates
    // d'octobre) ; en novembre, juillet n'est plus faux, il ne se redeclare pas : un seul 58, le second rappel, aux dates
    // de novembre.
    let verse = centimes(valeur(&depose, "S21.G00.50.004")) + centimes(valeur(&regul_octobre, "S21.G00.50.004"));
    let octobre = nets_sociaux(&regul_octobre);
    assert_eq!(octobre.len(), 3, "{:?}", octobre);
    assert_eq!(octobre[1].1 + octobre[2].1, verse);
    let nets = nets_sociaux(&regul);
    assert_eq!(nets.len(), 1, "{:?}", nets);
    assert_eq!(nets[0].0, "01112026");
    let frais_apres = frais(&rappel(&rappel_octobre, &regul));
    assert!(frais_apres > 0, "le rappel declare ses defraiements en 54, hors du 58.004");
    assert_eq!(centimes(valeur(&regul, "S21.G00.50.004")), nets[0].1 + octobre[1].1 + octobre[2].1 - verse);
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
    outils::par_contrat_de_prevoyance(toutes).into_iter().map(|(c, m)| (c, i64::try_from(m * paie::num::Decimal::ONE_HUNDRED).unwrap())).collect()
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
    let rep = produire_octobre(&r);
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
    let rep = produire_octobre(&r);
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
    let mut saisie = lire(&r, "paies/2026-10.json");
    saisie["engagements"].as_array_mut().unwrap().push(json!({
        "salarie": "Servandoni", "spectacle": "266Z00910022", "debut": "2026-07-02", "fin": "2026-10-09",
        "date_paiement": "2026-10-09", "numero": CONTRAT,
        "lignes": [{"rubrique": "heure", "quantite": 32, "taux": 18.0, "lieu": "Theatre fictif de Saumur",
                    "dates": ["2026-10-06", "2026-10-07", "2026-10-08", "2026-10-09"]}]
    }));
    ecrire_json(&r, "paies/2026-10.json", &saisie);
    octobre_pret(&r);
    let rep = produire_octobre(&r);
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
    let rep = produire_octobre(&r);
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
    let rep = produire_octobre(&r);
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
    let rep = produire_octobre(&r);
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

// ------------------------------------------------- reste R1 #3 : le taux du bloc 56 se lit sur le versement declare

#[test]
fn le_taux_du_bloc_56_est_le_50006_du_versement_declare_pas_celui_d_un_autre_fichier_du_mois() {
    // « Taux declare le mois de l'erreur » (S21.G00.56.006) : « le taux applique le mois de l'erreur ou de l'indu »
    // (cahier technique 2026.1, bloc 56, atelier/reference/norme-dsn/texte/bloc-S21.G00.56.txt, l. 134-136). Il se lit sur
    // le versement que la correction annule (le dernier etat declare, vague 0b), pas sur n'importe quel .dsn du dossier :
    // avant le reste R1 #3, un fichier d'essai range a cote (ici avec 50.006 = 9.90) l'emportait sur le depot reel.
    let r = copie("taux-bloc-56");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    let taux_depose = valeur(&depose, "S21.G00.50.006").to_string();
    assert_eq!(taux_depose, "2.10");
    let reel = std::fs::read(r.join("sorties/2026-07/DSN_Juillet2026.dsn")).unwrap();
    let essai = String::from_utf8(reel).unwrap().replace("S21.G00.50.006,'2.10'", "S21.G00.50.006,'9.90'");
    std::fs::write(r.join("sorties/2026-07/DSN_Juillet2026_essai.dsn"), essai).unwrap();
    // Un indu : l'heure de Servandoni passe de 18 a 10 EUR, le net fiscal du rappel est negatif.
    corriger_juillet_a(&r, JOUR, 10.0);
    octobre_pret(&r);
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let regul = versement(&lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn")), JUILLET);
    assert!(centimes(valeur(&regul, "S21.G00.50.002")) < 0, "un indu");
    assert_eq!(valeur(&regul, "S21.G00.56.001"), "072026");
    assert_eq!(valeur(&regul, "S21.G00.56.002"), "03");
    assert_eq!(valeur(&regul, "S21.G00.56.006"), taux_depose, "le taux du versement declare");
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
    let mut saisie = lire(&r, "paies/2026-07.json");
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

    // « Corriger ce mois » le liste, sans rien de declare ; sa correction le declare en entier et lui donne son numero.
    let d = route_le(&r, JOUR, "regularisation-mois", json!({"mois": "2026-07"}));
    let item = d["resultat"]["engagements"].as_array().unwrap().iter().find(|e| e["ajoute"] == json!(true)).unwrap().clone();
    assert_eq!((item["debut"].as_str(), &item["numero"]), (Some("2026-07-27"), &Value::Null), "{}", item);
    let charge = json!({"mois": "2026-07", "salarie": item["salarie"], "debut": item["debut"], "engagement": {"lignes": item["engagement_saisi"]["lignes"]}});
    let a = route_le(&r, JOUR, "regularisation-apercu", charge.clone());
    assert_eq!((a["resultat"]["avant"]["brut"].as_str(), a["resultat"]["apres"]["brut"].as_str()), (Some("0.00"), Some("432.00")), "{}", a);
    let mut ecrite = charge;
    ecrite["motif"] = json!("contrat oublie");
    let w = route_le(&r, JOUR, "regularisation-ecrire", ecrite);
    assert_eq!(w["ok"], json!(true), "{}", w);
    let numero = w["resultat"]["correction"]["engagement"]["numero"].as_str().unwrap().to_string();
    assert!(numero.starts_with("SE9.26.") && numero != CONTRAT, "{}", numero);
    let fige = lire(&r, "paies/2026-07.json")["engagements"].as_array().unwrap().iter().any(|e| e["numero"] == json!(numero));
    assert!(fige, "le numero est pose dans la saisie de juillet");

    // L'etape DSN de juillet n'a plus rien a faire.
    let b = route_le(&r, JOUR, "bilan-du-mois", json!({"mois": "2026-07"}));
    assert_eq!(b["resultat"]["dsn"]["remplacement"]["correction_ecrite"], json!(true), "{}", b["resultat"]["dsn"]);

    octobre_pret(&r);
    let rep = produire_octobre(&r);
    assert_eq!(rep["ok"], json!(true), "{}", rep);
    let octobre = lignes(&r.join("sorties/2026-10/DSN_Octobre2026.dsn"));
    // Le bloc 40 du contrat oublie, a sa date de debut passee.
    let rang = octobre.iter().position(|(k, v)| k == "S21.G00.40.009" && *v == numero).expect("le contrat oublie est declare");
    let debut = octobre[..rang].iter().rev().find(|(k, _)| k == "S21.G00.40.001").map(|(_, v)| v.as_str());
    assert_eq!(debut, Some("27072026"));
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
/// La correction s'ecrit, et son apercu annule tout le declare. (Que la DSN d'octobre la porte depend de ce qu'octobre
/// verse par ailleurs aux memes organismes : un versement S21.G00.20.005 ne peut pas etre negatif, et le moteur refuse
/// alors ; ce n'est pas propre au contrat ajoute ou retire.)
#[test]
fn un_contrat_qui_n_a_pas_eu_lieu_se_corrige_a_zero() {
    let r = copie("contrat-a-zero");
    let depose = versement(&lignes(&r.join("sorties/2026-07/DSN_Juillet2026.dsn")), JUILLET);
    let mut saisie = lire(&r, "paies/2026-07.json");
    let rang = saisie["engagements"].as_array().unwrap().iter().position(|e| e["numero"] == json!(CONTRAT)).unwrap();
    for l in saisie["engagements"][rang]["lignes"].as_array_mut().unwrap() {
        l["quantite"] = json!(0);
    }
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
}
