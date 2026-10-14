//! Corrections DSN, vague 1c : le net deja verse au salarie pour un contrat corrige (le « net a payer avant impot du
//! bulletin rectifie ») se LIT sur le bulletin archive du mois d'origine (`sorties/<mois>/Bulletins_<Mois><Annee>.pdf`),
//! ou sur le bulletin rectificatif de la derniere correction deja portee ; il ne se reconstitue plus depuis la DSN
//! deposee (regle 4 : ce qui a ete paye se lit). Sans bulletin archive, sans le contrat dedans, ou avec deux bulletins
//! du contrat sur sa periode : refus, la DSN du mois ne se produit pas.
//!
//! Chaque test monte une copie de `demo-3090-grande` (compagnie inventee) dans un dossier a lui ; aucun reseau, aucun
//! depot. Le depot reel d'un mois produit par le test s'ecrit au journal de la copie.

use crate::outils::{self, compagnie, dsn_deposee, ecrire, lire, mois_pret};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const JOUR: &str = "2026-11-10";
const SERVANDONI: &str = "SE9.26.05";

/// La 3090-grande sans ses pieces, sauf juin et juillet 2026 (les bulletins archives que relit la correction).
fn copie(test: &str) -> PathBuf {
    compagnie::ses_dsn("demo-3090-grande", &["2026-06", "2026-07"], &outils::dossier_de_test("net_deja_verse", test))
}

fn produire_le(racine: &Path, mois: &str, jour: &str) -> Value {
    outils::route(racine, "produire", json!({"mois": mois, "saisie": lire(racine, &format!("paies/{}.json", mois))}), jour)
}

/// Une correction du contrat `numero` de `mois` : le taux de ses lignes `rubrique` passe a `taux`.
fn corriger(racine: &Path, jour: &str, mois: &str, numero: &str, rubrique: &str, taux: f64) {
    let d = outils::route(racine, "regularisation-mois", json!({"mois": mois}), jour);
    let item = outils::reussi(&d)["engagements"].as_array().unwrap().iter().find(|e| e["numero"] == json!(numero)).unwrap().clone();
    let mut e = item["engagement"].clone();
    for l in e["lignes"].as_array_mut().unwrap() {
        if l["rubrique"] == json!(rubrique) {
            l["taux"] = json!(taux);
        }
    }
    let charge = json!({"mois": mois, "salarie": item["salarie"], "debut": item["debut"], "engagement": e, "motif": "le taux etait faux"});
    outils::reussi(&outils::route(racine, "regularisation-ecrire", charge, jour));
}

/// Le texte d'un PDF ecrit par le moteur, pages jointes.
fn texte_pdf(chemin: &Path) -> String {
    fabrique_pdf::lecture::texte(chemin.to_str().unwrap()).unwrap_or_else(|r| panic!("{} : {}", chemin.display(), r.0)).0.join("\n")
}

/// Le bulletin du contrat `numero` dans un fichier groupe : du « Bulletin de paie n° » qui le commence au suivant.
fn bulletin_du_contrat<'a>(texte: &'a str, numero: &str) -> &'a str {
    let marque = format!("Contrat n° {} ", numero);
    let mut trouves = texte.split("Bulletin de paie n").filter(|b| b.contains(&marque));
    let b = trouves.next().unwrap_or_else(|| panic!("aucun bulletin du contrat {}", numero));
    assert!(trouves.next().is_none(), "un seul bulletin du contrat {}", numero);
    b
}

/// Le premier montant ecrit apres `libelle` (« 2 943,62 »), tel qu'imprime.
fn montant_apres(texte: &str, libelle: &str) -> String {
    let i = texte.find(libelle).unwrap_or_else(|| panic!("{:?} n'est pas imprime", libelle));
    let reste: Vec<char> = texte[i + libelle.len()..].trim_start().chars().collect();
    let n = reste.iter().take_while(|c| c.is_ascii_digit() || matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '.')).count();
    assert!(n > 0 && reste.get(n) == Some(&',') && reste.len() > n + 2, "pas de montant apres {:?}", libelle);
    let m: String = reste[..n + 3].iter().collect();
    m.replace(['\u{a0}', '\u{202f}'], " ").trim().to_string()
}

/// « 2 943,62 » -> « 2943.62 ».
fn en_dsn(m: &str) -> String {
    m.replace([' ', '.'], "").replace(',', ".")
}

/// Le bulletin rectificatif archive de la correction `n` du mois `mois` (`Rectificatif_<cle>_<Mois>_c<n>.pdf`).
fn rectificatif(racine: &Path, mois: &str, n: usize) -> PathBuf {
    let dossier = racine.join("sorties").join(mois);
    let suffixe = format!("_c{}.pdf", n);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dossier)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("Rectificatif_") && p.to_string_lossy().ends_with(&suffixe))
        .collect();
    assert_eq!(v.len(), 1, "un rectificatif c{} dans {}", n, dossier.display());
    v.remove(0)
}

const NET_RECTIFIE: &str = "Net à payer avant impôt du bulletin rectifié";

// ------------------------------------------------------------------------------- un depot d'avant la vague 1b

#[test]
fn le_net_deja_verse_de_servandoni_est_celui_de_son_bulletin_de_juillet() {
    // SE9.26.05 (juillet 2026 de la demo figee) : defraye (21 x 20,70 = 434,70), deposee par un moteur d'avant la
    // vague 1b, dont le 58.004 portait le net a payer (2 943,62, defraiements compris). Le bulletin archive imprime
    // 2 943,62 : c'est ce qui a ete paye. La reconstitution depuis la DSN comptait les defraiements une seconde fois
    // (2 943,62 + 434,70 = 3 378,32). Le bulletin est d'avant le modele de l'arrete (vague V2g) : le lecteur des archives
    // (`apercu_dsn::numeros_archives`) le lit aussi.
    let r = copie("servandoni");
    let archive = texte_pdf(&r.join("sorties/2026-07/Bulletins_Juillet2026.pdf"));
    let ancien = bulletin_du_contrat(&archive, SERVANDONI);
    assert!(ancien.contains("SALAIRE BRUT") && !ancien.contains("MONTANT BRUT"), "un bulletin d'avant la vague V2g");
    let paye = montant_apres(ancien, "NET A PAYER AVANT IMPÔT");
    assert_eq!(paye, "2 943,62");
    corriger(&r, JOUR, "2026-07", SERVANDONI, "heure", 19.0);
    mois_pret(&r, "2026-10");
    let rep = produire_le(&r, "2026-10", JOUR);
    outils::reussi(&rep);
    let rect = texte_pdf(&rectificatif(&r, "2026-07", 1));
    assert_eq!(montant_apres(&rect, NET_RECTIFIE), paye, "le net deja verse est celui du bulletin archive");
    assert!(rep.to_string().contains(&format!("net {} -> ", en_dsn(&paye))), "le resume de la regularisation dit le meme net : {}", rep);
}

// --------------------------------- un contrat defraye avec acompte : la DSN deposee ne change rien au net deja verse

/// Chaminade, octobre 2026 : 32 h a 19 EUR, 4 defraiements de 20,70, et un acompte de 100 EUR. Octobre est produit par
/// le moteur du jour (bulletin archive a deux nets, au modele de l'arrete), depose ; `ancien_58004` reecrit le 58.004 de
/// son versement dans la DSN deposee au net a payer, comme l'aurait ecrit un moteur d'avant la vague 1b. Puis l'heure
/// passe a 20 EUR, et novembre porte la correction.
struct Chaminade {
    r: PathBuf,
    numero: String,
    /// Le bulletin d'octobre archive.
    bulletin: String,
    /// Le montant net social et le net a payer imprimes.
    social: String,
    paye: String,
    /// Le net deja verse que lit le rectificatif.
    verse: String,
}

fn chaminade_corrigee(test: &str, ancien_58004: bool) -> Chaminade {
    let r = copie(test);
    let mut s = lire(&r, "paies/2026-10.json");
    let e = s["engagements"].as_array_mut().unwrap().iter_mut().find(|e| e["salarie"] == json!("Chaminade")).unwrap();
    e["lignes"].as_array_mut().unwrap().push(json!({"rubrique": "acompte", "quantite": 1, "taux": 100.0}));
    ecrire(&r, "paies/2026-10.json", &s);
    mois_pret(&r, "2026-10");
    outils::reussi(&produire_le(&r, "2026-10", JOUR));
    let saisie = lire(&r, "paies/2026-10.json");
    let numero = saisie["engagements"].as_array().unwrap().iter().find(|e| e["salarie"] == json!("Chaminade")).unwrap()["numero"].as_str().unwrap().to_string();

    let archive = texte_pdf(&r.join("sorties/2026-10/Bulletins_Octobre2026.pdf"));
    let bulletin = bulletin_du_contrat(&archive, &numero).to_string();
    let (social, paye) = (montant_apres(&bulletin, "MONTANT NET SOCIAL"), montant_apres(&bulletin, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"));

    dsn_deposee(&r, "2026-10", "DSN_Octobre2026.dsn", "2026-11-12T10:00:00");
    let chemin = r.join("sorties/2026-10/DSN_Octobre2026.dsn");
    let brut = std::fs::read(&chemin).unwrap();
    let (mut du_contrat, mut vus, mut dsn) = (false, 0, String::new());
    for l in String::from_utf8_lossy(&brut).split_inclusive('\n') {
        let (k, v) = l.trim_end().split_once(',').unwrap_or(("", ""));
        match k {
            "S21.G00.50.001" => du_contrat = false,
            "S21.G00.51.010" if v.trim_matches('\'') == numero => du_contrat = true,
            _ => {}
        }
        if k == "S21.G00.58.004" && du_contrat {
            vus += 1;
            assert_eq!(v.trim_matches('\''), en_dsn(&social), "le moteur du jour declare le montant net social");
            if ancien_58004 {
                dsn.push_str(&format!("{},'{}'{}", k, en_dsn(&paye), if l.ends_with("\r\n") { "\r\n" } else { "\n" }));
                continue;
            }
        }
        dsn.push_str(l);
    }
    assert_eq!(vus, 1, "un 58.004 pour le contrat {}", numero);
    std::fs::write(&chemin, dsn.as_bytes()).unwrap();

    let jour = "2026-12-10";
    corriger(&r, jour, "2026-10", &numero, "heure", 20.0);
    mois_pret(&r, "2026-11");
    outils::reussi(&produire_le(&r, "2026-11", jour));
    let verse = montant_apres(&texte_pdf(&rectificatif(&r, "2026-10", 1)), NET_RECTIFIE);
    Chaminade { r, numero, bulletin, social, paye, verse }
}

#[test]
fn le_net_deja_verse_d_un_contrat_defraye_avec_acompte_est_celui_du_bulletin_et_son_rappel_se_date_au_versement() {
    // 58.004 depose = montant net social (le moteur du jour). Norme DSN 2026.1, S21.G00.58.003 (p. 265) : les dates du
    // montant net social « sont a renseigner en periode de versement » ; BOSS Q 35 et Q 41 : le rappel se prend dans le
    // montant net social du mois ou il est verse, celui d'octobre ne change pas. Un seul 58, du 01 au 30/11/2026, la
    // difference de montant net social.
    let c = chaminade_corrigee("chaminade", false);
    assert_ne!(c.social, c.paye, "defraiements (82,80) et acompte (100) separent les deux nets");
    assert_eq!(c.verse, c.paye, "le net deja verse est celui du bulletin");
    // Le bulletin du moteur du jour est au modele de l'arrete du 25/02/2016 (art. 1, I), et le lecteur le lit.
    for libelle in ["MONTANT BRUT", "COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES", "TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES", "TOTAL VERSÉ PAR L'EMPLOYEUR"] {
        assert!(c.bulletin.contains(libelle), "le bulletin neuf imprime {:?}", libelle);
    }
    assert!(!c.bulletin.contains("SALAIRE BRUT") && !c.bulletin.contains("Coût total employeur"), "plus les libelles d'avant");
    let (b58, nets) = blocs_58(&c.r.join("sorties/2026-11/DSN_Novembre2026.dsn"), &c.numero);
    assert_eq!(b58.len(), 1, "{:?}", b58);
    let (d, f, m) = &b58[0];
    assert_eq!((d.as_str(), f.as_str()), ("01112026", "30112026"), "en periode de versement");
    let rappel: f64 = m.parse().unwrap();
    assert!(rappel > 0.0, "32 h a 1 EUR de plus, moins les cotisations : {}", m);
    // Ni defraiement, ni acompte, ni indemnite ne bougent : le rappel de montant net social est le net verse du rappel.
    assert_eq!(nets.last().unwrap(), m, "50.004 du versement de la correction");
}

#[test]
fn sur_un_depot_d_avant_1b_le_net_deja_verse_ne_change_pas_et_le_montant_net_social_d_origine_se_corrige() {
    // Octobre depose par un moteur d'avant la vague 1b (58.004 = le net a payer, defraiements et acompte compris). Le net
    // deja verse reste celui du bulletin. BOSS Q 41 : la periode d'origine se corrige « en precisant quel etait le montant
    // net social effectivement verse au titre de cette periode » -- l'annulation du 58 depose et le montant net social de
    // ce qui a ete paye, aux dates d'octobre ; le rappel, lui, aux dates de novembre.
    let c = chaminade_corrigee("chaminade-ancien-58004", true);
    assert_eq!(c.verse, c.paye, "58.004 depose = net a payer : le meme net deja verse");
    let (b58, nets) = blocs_58(&c.r.join("sorties/2026-11/DSN_Novembre2026.dsn"), &c.numero);
    assert_eq!(b58.len(), 3, "{:?}", b58);
    assert_eq!(b58[0].2, format!("-{}", en_dsn(&c.paye)), "l'annulation du 58.004 depose");
    assert_eq!(b58[1].2, en_dsn(&c.social), "le montant net social de ce qui a ete paye : celui du bulletin d'octobre");
    assert_eq!((b58[0].0.as_str(), b58[1].0.as_str()), (b58[1].0.as_str(), b58[0].0.as_str()));
    assert!(b58[0].0.ends_with("102026") && b58[0].1.ends_with("102026"), "aux dates d'origine : {:?}", b58[0]);
    assert_eq!((b58[2].0.as_str(), b58[2].1.as_str()), ("01112026", "30112026"));
    assert_eq!(&b58[2].2, nets.last().unwrap());
}

// -------------------------------------------------------------------------------------------------- les refus

fn refus_pour_servandoni(test: &str, abimer: &dyn Fn(&Path)) -> String {
    let r = copie(test);
    corriger(&r, JOUR, "2026-07", SERVANDONI, "heure", 19.0);
    mois_pret(&r, "2026-10");
    abimer(&r);
    let (_, message) = outils::refus(&produire_le(&r, "2026-10", JOUR));
    assert!(!r.join("sorties/2026-10/DSN_Octobre2026.dsn").exists(), "la DSN d'octobre ne se produit pas");
    assert!(message.contains("Correction de 2026-07") && message.contains(SERVANDONI), "{}", message);
    assert!(message.contains("net deja verse"), "{}", message);
    message
}

#[test]
fn sans_bulletin_archive_la_correction_refuse() {
    let message = refus_pour_servandoni("sans-bulletin", &|r| std::fs::remove_file(r.join("sorties/2026-07/Bulletins_Juillet2026.pdf")).unwrap());
    assert!(message.contains("sorties/2026-07/Bulletins_Juillet2026.pdf"), "{}", message);
    assert!(message.contains("n'est pas sur le disque"), "{}", message);
}

#[test]
fn un_bulletin_archive_sans_le_contrat_refuse() {
    // Le fichier de juillet remplace par celui de juin : un fichier du moteur, lisible, ou SE9.26.05 n'est pas.
    let message = refus_pour_servandoni("sans-le-contrat", &|r| {
        std::fs::copy(r.join("sorties/2026-06/Bulletins_Juin2026.pdf"), r.join("sorties/2026-07/Bulletins_Juillet2026.pdf")).unwrap();
    });
    assert!(message.contains("ne porte aucun bulletin du contrat"), "{}", message);
}

// ------------------------------------------------------ une seconde correction : le rectificatif de la premiere

#[test]
fn une_seconde_correction_lit_le_net_sur_le_rectificatif_de_la_premiere() {
    // Premiere correction de SE9.26.05 (18 -> 19 EUR) portee par octobre, depose ; seconde (19 -> 20 EUR) portee par
    // novembre. Ce qui a ete paye quand la seconde se porte : le net a payer rectifie de la premiere.
    let r = copie("seconde");
    corriger(&r, JOUR, "2026-07", SERVANDONI, "heure", 19.0);
    mois_pret(&r, "2026-10");
    outils::reussi(&produire_le(&r, "2026-10", JOUR));
    dsn_deposee(&r, "2026-10", "DSN_Octobre2026.dsn", "2026-11-12T10:00:00");
    let mut c = lire(&r, "regularisations/2026-07.json");
    c["corrections"][0]["portee_par"] = json!({"mois": "2026-10", "fichier": "DSN_Octobre2026.dsn", "quand": "2026-11-12T10:00:00"});
    ecrire(&r, "regularisations/2026-07.json", &c);
    let premier = rectificatif(&r, "2026-07", 1);
    let rect1 = texte_pdf(&premier);
    let rectifie = montant_apres(&rect1, "Net à payer avant impôt rectifié");
    assert_eq!(montant_apres(&rect1, "MONTANT NET A PAYER AVANT IMPOT SUR LE REVENU"), rectifie, "le rectificatif imprime la paie corrigee");

    let jour = "2026-12-10";
    corriger(&r, jour, "2026-07", SERVANDONI, "heure", 20.0);
    mois_pret(&r, "2026-11");

    // Sans le rectificatif de la premiere : refus, il dit lequel manque.
    let garde = r.parent().unwrap().join("seconde-rectificatif-c1.pdf");
    std::fs::rename(&premier, &garde).unwrap();
    let (_, message) = outils::refus(&produire_le(&r, "2026-11", jour));
    assert!(message.contains(&premier.file_name().unwrap().to_string_lossy().to_string()) && message.contains("n'est pas sur le disque"), "{}", message);
    assert!(!r.join("sorties/2026-11/DSN_Novembre2026.dsn").exists());

    std::fs::rename(&garde, &premier).unwrap();
    outils::reussi(&produire_le(&r, "2026-11", jour));
    let rect2 = texte_pdf(&rectificatif(&r, "2026-07", 2));
    assert_eq!(montant_apres(&rect2, NET_RECTIFIE), rectifie);
}

// ------------------------------------- vague 1o : la date du montant net social d'une correction (BOSS Q 35 et Q 41)

/// Les blocs 58 (58.001, 58.002, 58.004) et le 50.004 des versements du contrat `numero` dans la DSN `chemin`.
fn blocs_58(chemin: &Path, numero: &str) -> (Vec<(String, String, String)>, Vec<String>) {
    let texte = String::from_utf8_lossy(&std::fs::read(chemin).unwrap()).into_owned();
    let (mut out, mut nets) = (Vec::new(), Vec::new());
    let mut bloc: Vec<(String, String)> = Vec::new();
    let mut fermer = |bloc: &mut Vec<(String, String)>| {
        if bloc.iter().any(|(k, v)| k == "S21.G00.51.010" && v == numero) {
            let mut courant: Option<(String, String)> = None;
            for (k, v) in bloc.iter() {
                match k.as_str() {
                    "S21.G00.50.004" => nets.push(v.clone()),
                    "S21.G00.58.001" => courant = Some((v.clone(), String::new())),
                    "S21.G00.58.002" => courant.as_mut().unwrap().1 = v.clone(),
                    "S21.G00.58.004" => {
                        let (d, f) = courant.take().unwrap();
                        out.push((d, f, v.clone()));
                    }
                    _ => {}
                }
            }
        }
        bloc.clear();
    };
    for l in texte.lines() {
        let (k, v) = l.split_once(',').unwrap_or((l, ""));
        let v = v.trim_matches('\'').to_string();
        if k == "S21.G00.50.001" || !(k.starts_with("S21.G00.5") || k.starts_with("S21.G00.7") || k.starts_with("S21.G00.8")) {
            fermer(&mut bloc);
        }
        if k.starts_with("S21.G00.5") || k.starts_with("S21.G00.7") || k.starts_with("S21.G00.8") {
            bloc.push((k.to_string(), v));
        }
    }
    fermer(&mut bloc);
    (out, nets)
}
