//! CE QUE LES ORGANISMES REPONDENT A UNE COMPAGNIE FICTIVE, au-dela de l'accuse et du bilan conformes : l'effectif que
//! l'Urssaf calcule depuis les DSN deposees (compte rendu de nature 118), le taux de prelevement a la source que la DGFiP
//! communique (nature 94), et le bilan non conforme d'une DSN (nature 11, rapport `BAN`). Chaque piece est fabriquee dans la
//! forme ou l'organisme la publie, archivee dans `retours/` comme une vraie, et lue par le meme lecteur.
//!
//! LE MONDE N'EST PAS LA CONFIGURATION DE LA COMPAGNIE. Ce que la DGFiP sait d'un salarie (son taux) et l'anomalie que
//! net-entreprises relevera, une compagnie ne l'ecrit nulle part : c'est le scenario de la fictive qui l'annonce, a sa
//! date (`annoncer`), et le simulateur le garde de son cote, dans `organismes-simules/`, a cote de `courriels-simules/`.
//! Rien de ce dossier n'est lu par le calcul, ni par une route.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::{erreur, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

use crate::branchement::Branchement;

const DOSSIER: &str = "organismes-simules";
const FICHIER: &str = "annonces.json";

/// Le compte rendu de l'Urssaf qui porte les effectifs (table des types de parametres NEOReS).
pub const NATURE_EFFECTIFS: &str = "118";

fn chemin(racine: &Path) -> PathBuf {
    racine.join(DOSSIER).join(FICHIER)
}

/// La memoire du simulateur : `{"annonces": [...], "effectifs_rendus": ["2026", ...]}`.
fn charger(racine: &Path) -> R<Value> {
    let p = chemin(racine);
    if !p.exists() {
        return Ok(json!({"annonces": [], "effectifs_rendus": []}));
    }
    let t = std::fs::read_to_string(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
    let mut v: Value = serde_json::from_str(&t).map_err(|e| erreur("ValueError", format!("{} : {}", p.display(), e)))?;
    for cle in ["annonces", "effectifs_rendus"] {
        if !v.get(cle).is_some_and(Value::is_array) {
            v[cle] = json!([]);
        }
    }
    Ok(v)
}

fn ecrire(racine: &Path, v: &Value) -> R<()> {
    socle::ecriture::ecrire_json(&chemin(racine), v, 2).map_err(|m| erreur("OSError", m))
}

fn jour_iso(t: &str) -> Option<Jour> {
    Jour::lire(&t.chars().take(10).collect::<String>()).ok()
}

/// Ce que le scenario annonce aux organismes de la compagnie fictive, a la date ou il le joue (le jour de l'horloge
/// figee). Deux annonces :
/// - `{"taux_pas": {"salarie": "<cle de la fiche>", "taux": "7.50"}}` : la DGFiP connait ce taux pour ce salarie a
///   partir de ce jour ; elle le communique en retour de chaque DSN mensuelle reelle qui le declare ensuite ;
/// - `{"taux_atmp": {"code_risque": "923AD", "taux": "1.60", "date_effet": "AAAA-MM-JJ", "bureau": "B"?, "code_ctn": ..?,
///   "section": ..?}}` : la CARSAT notifie ce taux a l'etablissement ; la notification (nature 34) suit une fois la
///   prochaine DSN mensuelle reelle deposee a partir de ce jour (`crm_atmp`) ;
/// - `{"refus": {"code": "...", "message": "...", "rubrique": "S21.G00.xx.xxx"?, "mois": "AAAA-MM"?}}` : la prochaine DSN
///   reelle deposee a partir de ce jour (du mois dit, s'il l'est) recoit un bilan non conforme portant cette anomalie.
/// Hors d'une compagnie fictive, refus : le monde ne s'annonce pas.
pub fn annoncer(racine: &Path, branchement: Branchement, annonce: &Value) -> R<()> {
    if !branchement.joue() {
        return Err(erreur("ErreurOperation", "Seuls les organismes d'une compagnie fictive s'annoncent : ceux d'une vraie repondent d'eux-memes."));
    }
    let o = annonce.as_object().ok_or_else(|| erreur("ValueError", "Une annonce est un objet."))?;
    let texte = |v: &Value, k: &str| v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default();
    let mut propre = Map::new();
    propre.insert("le".into(), json!(socle::paris::maintenant().chars().take(10).collect::<String>()));
    match (o.get("taux_pas"), o.get("refus")) {
        (Some(t), None) if o.len() == 1 => {
            let salarie = texte(t, "salarie");
            let taux = texte(t, "taux").replace(',', ".");
            let n: f64 = taux.parse().map_err(|_| erreur("ValueError", format!("taux_pas : taux illisible ({:?}).", taux)))?;
            if salarie.is_empty() || !(0.0..=100.0).contains(&n) {
                return Err(erreur("ValueError", "taux_pas : un salarie (la cle de sa fiche) et un taux entre 0 et 100 sont attendus."));
            }
            propre.insert("taux_pas".into(), json!({"salarie": salarie, "taux": format!("{:.2}", n)}));
        }
        (None, None) if o.len() == 1 && o.contains_key("taux_atmp") => {
            let t = &o["taux_atmp"];
            let code = texte(t, "code_risque").replace(['.', ' '], "").to_uppercase();
            let taux = texte(t, "taux").replace(',', ".");
            let n: f64 = taux.parse().map_err(|_| erreur("ValueError", format!("taux_atmp : taux illisible ({:?}).", taux)))?;
            let effet = texte(t, "date_effet");
            if code.len() != 5 || !(0.0..20.0).contains(&n) || n == 0.0 || jour_iso(&effet).is_none() {
                return Err(erreur("ValueError", "taux_atmp : un code risque de 5 caractères, un taux en pourcentage et une date d'effet AAAA-MM-JJ sont attendus."));
            }
            let mut m = Map::new();
            m.insert("code_risque".into(), json!(code));
            m.insert("taux".into(), json!(format!("{:.2}", n)));
            m.insert("date_effet".into(), json!(effet));
            for k in ["bureau", "code_ctn", "section"] {
                let v = texte(t, k);
                if !v.is_empty() {
                    m.insert(k.into(), json!(v));
                }
            }
            propre.insert("taux_atmp".into(), Value::Object(m));
        }
        (None, Some(r)) if o.len() == 1 => {
            let (code, message) = (texte(r, "code"), texte(r, "message"));
            if code.is_empty() || message.is_empty() {
                return Err(erreur("ValueError", "refus : le code et le message de l'anomalie sont attendus."));
            }
            let mois = texte(r, "mois");
            if !mois.is_empty() && jour_iso(&format!("{}-01", mois)).is_none() {
                return Err(erreur("ValueError", format!("refus : mois illisible ({:?}), AAAA-MM attendu.", mois)));
            }
            let mut m = Map::new();
            m.insert("code".into(), json!(code));
            m.insert("message".into(), json!(message));
            for k in ["rubrique", "mois"] {
                let v = texte(r, k);
                if !v.is_empty() {
                    m.insert(k.into(), json!(v));
                }
            }
            propre.insert("refus".into(), Value::Object(m));
        }
        _ => return Err(erreur("ValueError", "Une annonce porte `taux_pas`, `taux_atmp` ou `refus`, et rien d'autre.")),
    }
    let mut v = charger(racine)?;
    v["annonces"].as_array_mut().expect("liste").push(Value::Object(propre));
    ecrire(racine, &v)
}

// ------------------------------------------------------------------ une DSN deposee, relue

/// Les rubriques d'un fichier DSN, dans l'ordre : `(code, valeur)`.
fn rubriques(octets: &[u8]) -> Vec<(String, String)> {
    let texte = socle::texte::texte_latin1(octets);
    socle::texte::lignes_du_texte(&texte)
        .into_iter()
        .filter_map(|l| {
            let (c, v) = l.trim().split_once(',')?;
            Some((c.to_string(), v.trim().trim_matches('\'').to_string()))
        })
        .collect()
}

/// JJMMAAAA.
fn jour_dsn(v: &str) -> Option<Jour> {
    let c: Vec<u32> = v.chars().map(|x| x.to_digit(10)).collect::<Option<_>>()?;
    if c.len() != 8 {
        return None;
    }
    let n = |a: usize, b: usize| c[a..b].iter().fold(0, |s, x| s * 10 + x);
    Jour::nouveau(n(4, 8) as i32, n(2, 4), n(0, 2))
}

/// Un contrat tel que la DSN le declare, pour le decompte de l'effectif.
struct Contrat {
    debut: Option<Jour>,
    fin: Option<Jour>,
    rupture: Option<Jour>,
    reference: f64,
    quotite: f64,
    modalite: String,
    motif_recours: String,
}

/// Par individu (son NIR), ses contrats.
fn contrats_par_individu(octets: &[u8]) -> BTreeMap<String, Vec<Contrat>> {
    let mut out: BTreeMap<String, Vec<Contrat>> = BTreeMap::new();
    let mut individu = String::new();
    for (c, v) in rubriques(octets) {
        match c.as_str() {
            "S20.G00.05.001" => individu.clear(),
            "S21.G00.30.001" => individu = v,
            "S21.G00.40.001" => out.entry(individu.clone()).or_default().push(Contrat {
                debut: jour_dsn(&v),
                fin: None,
                rupture: None,
                reference: 0.0,
                quotite: 0.0,
                modalite: String::new(),
                motif_recours: String::new(),
            }),
            _ => {
                let Some(k) = out.get_mut(&individu).and_then(|l| l.last_mut()) else { continue };
                match c.as_str() {
                    "S21.G00.40.010" => k.fin = jour_dsn(&v),
                    "S21.G00.40.012" => k.reference = v.parse().unwrap_or(0.0),
                    "S21.G00.40.013" => k.quotite = v.parse().unwrap_or(0.0),
                    "S21.G00.40.014" => k.modalite = v,
                    "S21.G00.40.021" => k.motif_recours = v,
                    "S21.G00.62.001" => k.rupture = jour_dsn(&v),
                    _ => {}
                }
            }
        }
    }
    out
}

/// L'effectif d'un mois, d'apres les DSN de ce mois (une par etablissement), comme R. 130-1 du code de la securite
/// sociale le compte (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261029.md`, « L'effectif au sens
/// de la securite sociale ») : chaque personne titulaire d'un contrat compte a due proportion des jours du mois ou elle
/// est employee (II, quatrieme alinea) ; a temps plein, entierement ; a temps partiel (S21.G00.40.014 = 20), au rapport
/// de sa duree contractuelle a la duree de reference (dernier alinea de L. 1111-2 du code du travail : la quotite du
/// contrat, S21.G00.40.013, sur celle de la categorie, S21.G00.40.012) ; un CDD qui remplace un salarie absent
/// (motif de recours 01, S21.G00.40.021) n'est pas compte (II, premier alinea, qui renvoie a la deuxieme phrase du 2° de
/// L. 1111-2). Une personne compte au plus une fois par jour, au plus fort de ses contrats de ce jour.
fn effectif_du_mois(an: i32, mois: u32, dsn: &[Vec<u8>]) -> f64 {
    let n = socle::date::jours_du_mois(an, mois);
    let premier = Jour::nouveau(an, mois, 1).expect("mois");
    let dernier = Jour::nouveau(an, mois, n).expect("mois");
    let mut par_jour: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for octets in dsn {
        for (nir, contrats) in contrats_par_individu(octets) {
            let jours = par_jour.entry(nir).or_insert_with(|| vec![0.0; n as usize]);
            for k in contrats {
                if k.motif_recours == "01" {
                    continue;
                }
                let Some(debut) = k.debut else { continue };
                let fin = k.rupture.or(k.fin).unwrap_or(dernier);
                if debut > dernier || fin < premier {
                    continue;
                }
                let poids = if k.modalite == "20" && k.reference > 0.0 { (k.quotite / k.reference).min(1.0) } else { 1.0 };
                let (a, b) = (if debut < premier { 1 } else { debut.jour }, if fin > dernier { n } else { fin.jour });
                for j in a..=b {
                    let p = &mut jours[(j - 1) as usize];
                    *p = p.max(poids);
                }
            }
        }
    }
    par_jour.values().map(|jours| jours.iter().sum::<f64>() / f64::from(n)).sum()
}

/// L'effectif salarie annuel de `an` : la moyenne des effectifs des mois de l'annee, les mois sans salarie ecartes, le
/// resultat tronque au centieme (L. 130-1, I, et R. 130-1, I, du code de la securite sociale, memes releves). `None` quand
/// aucun mois n'a de salarie.
pub fn effectif_annuel(an: i32, dsn_par_mois: &BTreeMap<u32, Vec<Vec<u8>>>) -> Option<f64> {
    let mois: Vec<f64> = (1..=12).map(|m| dsn_par_mois.get(&m).map_or(0.0, |d| effectif_du_mois(an, m, d))).filter(|e| *e > 0.0).collect();
    if mois.is_empty() {
        return None;
    }
    let moyenne = mois.iter().sum::<f64>() / mois.len() as f64;
    Some((moyenne * 100.0 + 1e-9).floor() / 100.0)
}

// ------------------------------------------------------------------ ce que le simulateur rend

/// Une piece fabriquee : le retour tel que la liste des retours le decrit, et ses octets.
pub struct Piece {
    pub retour: Value,
    pub contenu: Vec<u8>,
}

fn retour(nature: &str, flux: &str, publication: &str, libelle: &str) -> Value {
    json!({"flux": flux, "nature": nature, "publication": publication, "statut": "fictive", "libelle": libelle})
}

fn echappe(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// La ligne du journal : le fichier depose, relu la ou le depot l'a pris (`sorties/<mois>/<fichier>`).
fn fichier_depose(racine: &Path, l: &Value) -> Option<Vec<u8>> {
    let mois = l.get("mois").map(valeur_en_texte).unwrap_or_default();
    let fichier = l.get("fichier").filter(|f| vrai(f)).map(valeur_en_texte)?;
    let p = Path::new(&fichier);
    let chemin = if p.is_absolute() { p.to_path_buf() } else { racine.join("sorties").join(&mois).join(&fichier) };
    std::fs::read(chemin).ok()
}

fn nature_dsn(l: &Value) -> String {
    l.get("detail").and_then(|d| d.get("nature")).filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_else(|| "01".into())
}

fn reelle(l: &Value) -> bool {
    l.get("mode").and_then(Value::as_str) == Some("reel")
}

/// Le refus annonce qui attend ce depot : le premier non servi, annonce au plus tard le jour du depot, du mois du depot
/// s'il en dit un. Il est servi par ce depot (son flux).
fn refus_pour(memoire: &mut Value, l: &Value, flux: &str) -> Option<Value> {
    if !reelle(l) {
        return None;
    }
    let depot = l.get("quand").and_then(Value::as_str).and_then(jour_iso)?;
    let mois = l.get("mois").map(valeur_en_texte).unwrap_or_default();
    for a in memoire["annonces"].as_array_mut()? {
        let Some(r) = a.get("refus").cloned() else { continue };
        if a.get("servi_par").is_some() {
            continue;
        }
        let le = a.get("le").and_then(Value::as_str).and_then(jour_iso);
        let son_mois = r.get("mois").and_then(Value::as_str).unwrap_or("");
        if le.is_some_and(|le| le <= depot) && (son_mois.is_empty() || son_mois == mois) {
            a["servi_par"] = json!(flux);
            return Some(r);
        }
    }
    None
}

/// L'accuse de reception (nature 10, rapport `AEE`) et le bilan non conforme (nature 11, rapport `BAN`) d'une DSN que
/// net-entreprises refuse, dans le schema des bilans (`atelier/reference/sources/gip-mds-api-dsn/
/// gip-mds-specifications-schema-bilans-vt2.2-20201118.txt` : `envoi`, `idflux`, `essai_reel`, `envoi_etat` ;
/// `declaration`, sa `declaration_identification`, son bilan `etat` et `mois`, ses anomalies `code`, `categorie`,
/// `message`, `identifiant` de la rubrique).
fn bilans_refuses(flux: &str, publication: &str, l: &Value, refus: &Value) -> Vec<Piece> {
    let mois = l.get("mois").map(valeur_en_texte).unwrap_or_default();
    let mois_dsn = if mois.len() == 7 { format!("01{}{}", &mois[5..7], &mois[..4]) } else { String::new() };
    let t = |k: &str| echappe(refus.get(k).and_then(Value::as_str).unwrap_or(""));
    let envoi = format!("<envoi><idflux>{}</idflux><essai_reel>02</essai_reel><envoi_etat>OK</envoi_etat></envoi>", echappe(flux));
    let rapport = |type_rapport: &str, corps: &str| {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"{}\">{}</ns2:rapport>", type_rapport, corps)
            .into_bytes()
    };
    let ban = format!(
        "{}<declaration><declaration_identification><rang>1</rang><nature>{}</nature></declaration_identification>\
         <declaration_bilan><etat>KO</etat><mois>{}</mois><declaration_anomalie><code>{}</code><categorie>Bloquante</categorie>\
         <message>{}</message><identifiant>{}</identifiant></declaration_anomalie></declaration_bilan></declaration>",
        envoi,
        echappe(&nature_dsn(l)),
        mois_dsn,
        t("code"),
        t("message"),
        t("rubrique")
    );
    vec![
        Piece { retour: retour("10", flux, publication, "Accuse de reception (compagnie fictive)"), contenu: rapport("AEE", &envoi) },
        Piece { retour: retour("11", flux, publication, "Bilan d'anomalies (compagnie fictive)"), contenu: rapport("BAN", &ban) },
    ]
}

/// Le compte rendu DGFiP de nature 94 d'une DSN mensuelle reelle : pour chaque individu qu'elle declare et dont la DGFiP
/// connait un taux (la derniere annonce `taux_pas` faite au plus tard le jour du depot), son NIR, son matricule et son
/// taux, dans la forme du CRM nominatif recu (`envois/tests/envois/retours.rs`, `CRM_REEL` : `rapport/@identifiant`,
/// `salarie`, `NIR`, `matricule`, `taux_imposition_PAS`). `None` quand la DGFiP n'a rien a dire.
fn crm_pas(ctx: &Contexte, memoire: &Value, l: &Value, octets: &[u8], flux: &str, publication: &str) -> R<Option<Piece>> {
    let depot = l.get("quand").and_then(Value::as_str).and_then(jour_iso);
    let mut taux: BTreeMap<String, String> = BTreeMap::new();
    for a in memoire["annonces"].as_array().into_iter().flatten() {
        let Some(t) = a.get("taux_pas") else { continue };
        let le = a.get("le").and_then(Value::as_str).and_then(jour_iso);
        if le.is_some() && le <= depot {
            taux.insert(valeur_en_texte(&t["salarie"]), valeur_en_texte(&t["taux"]));
        }
    }
    let mut salaries = String::new();
    let mut vus = BTreeSet::new();
    let (mut nir, mut matricule) = (String::new(), String::new());
    let mut individus: Vec<(String, String)> = Vec::new();
    for (c, v) in rubriques(octets) {
        match c.as_str() {
            "S21.G00.30.001" => {
                if !nir.is_empty() {
                    individus.push((nir.clone(), matricule.clone()));
                }
                nir = v;
                matricule.clear();
            }
            "S21.G00.30.019" => matricule = v,
            _ => {}
        }
    }
    if !nir.is_empty() {
        individus.push((nir, matricule));
    }
    for (nir, matricule) in individus {
        let Some((cle, _)) = ctx.salaries.iter().find(|(_, s)| s.get("nir").is_some_and(|n| valeur_en_texte(n) == nir)) else { continue };
        let Some(t) = taux.get(cle.as_str()) else { continue };
        if !vus.insert(nir.clone()) {
            continue;
        }
        salaries.push_str(&format!(
            "<salarie><NIR>{}</NIR><matricule>{}</matricule><taux_imposition_PAS>{}</taux_imposition_PAS></salarie>",
            echappe(&nir),
            echappe(&matricule),
            echappe(t)
        ));
    }
    if salaries.is_empty() {
        return Ok(None);
    }
    let mut o = [0u8; 4];
    socle::temps::aleas(&mut o).map_err(|e| erreur("OSError", format!("pas d'alea du systeme : {}", e)))?;
    let identifiant = 100_000_000 + u32::from_le_bytes(o) % 900_000_000;
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" identifiant=\"{}\" type=\"CRM nominatif\" \
         profil=\"DSN\" version=\"v02r01\"><envoi><envoi_identification><idflux>{}</idflux></envoi_identification></envoi><declaration>\
         <declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        identifiant,
        echappe(flux),
        salaries
    );
    let mut r = retour(crate::retours::NATURE_TAUX_PAS, flux, publication, "Compte-rendu metier DGFiP, taux du PAS (compagnie fictive)");
    r["production"] = json!(l.get("mois").map(valeur_en_texte).unwrap_or_default().replace('-', ""));
    Ok(Some(Piece { retour: r, contenu: xml.into_bytes() }))
}

/// La notification du taux AT/MP de nature 34 (la CNAM) d'une DSN mensuelle reelle : chaque annonce `taux_atmp` faite au
/// plus tard le jour du depot et pas encore rendue, rattachee a cette declaration (« la dernière déclaration mensuelle
/// disponible », guide de l'API DSN v6.8, §5.26.1), dans la forme du schema des bilans : `declaration_identification`
/// (`nature`, `type`, `mois`, `SIREN`, `nic_affectation`), puis `declaration_bilan` et un `taux_AT` par annonce
/// (`code_ctn`, `section`, `code_risque`, `temoin_bureau`, `taux`, `date_effet`, `date_calcul` ;
/// `atelier/reference/sources/gip-mds-api-dsn/`, LISEZ-MOI, « La notification du taux AT/MP »). Le `type` du rapport
/// n'est ecrit par aucun texte depose : la piece n'en porte pas. `None` quand la CARSAT n'a rien a notifier.
fn crm_atmp(memoire: &mut Value, l: &Value, octets: &[u8], flux: &str, publication: &str) -> Option<Piece> {
    let depot = l.get("quand").and_then(Value::as_str).and_then(jour_iso);
    let rubriques = rubriques(octets);
    let r = |code: &str| rubriques.iter().find(|(c, _)| c == code).map(|(_, v)| v.clone()).unwrap_or_default();
    let calcul = socle::paris::maintenant().chars().take(10).collect::<String>().replace('-', "");
    let mut taux = String::new();
    for a in memoire["annonces"].as_array_mut().into_iter().flatten() {
        let Some(t) = a.get("taux_atmp").cloned() else { continue };
        let le = a.get("le").and_then(Value::as_str).and_then(jour_iso);
        if a.get("rendue").is_some() || le.is_none() || le > depot {
            continue;
        }
        let x = |k: &str| t.get(k).map(valeur_en_texte).unwrap_or_default();
        let facultatif = |k: &str, balise: &str| if x(k).is_empty() { String::new() } else { format!("<{}>{}</{}>", balise, echappe(&x(k)), balise) };
        taux.push_str(&format!(
            "<taux_AT>{}{}<code_risque>{}</code_risque>{}<taux>{}</taux><date_effet>{}</date_effet><date_calcul>{}</date_calcul></taux_AT>",
            facultatif("code_ctn", "code_ctn"),
            facultatif("section", "section"),
            echappe(&x("code_risque")),
            facultatif("bureau", "temoin_bureau"),
            echappe(&x("taux")),
            x("date_effet").replace('-', ""),
            calcul
        ));
        a["rendue"] = json!(flux);
    }
    if taux.is_empty() {
        return None;
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" profil=\"DSN\" version=\"v02r02\">\
         <envoi><envoi_identification><idflux>{}</idflux></envoi_identification></envoi><declaration><declaration_identification>\
         <rang>1</rang><nature>{}</nature><type>{}</type><mois>{}</mois><SIREN>{}</SIREN><nic_affectation>{}</nic_affectation>\
         </declaration_identification><declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        echappe(flux),
        echappe(&r("S20.G00.05.001")),
        echappe(&r("S20.G00.05.002")),
        echappe(&r("S20.G00.05.005")),
        echappe(&r("S21.G00.06.001")),
        echappe(&r("S21.G00.11.001")),
        taux
    );
    Some(Piece { retour: retour(crate::retours::NATURE_TAUX_ATMP, flux, publication, "Compte-rendu metier taux AT/MP, notification de la Carsat (compagnie fictive)"), contenu: xml.into_bytes() })
}

/// Les DSN mensuelles reelles de l'annee `an` que l'Urssaf a recues et acceptees, par mois : pour chaque mois et chaque
/// etablissement, la derniere deposee ; une DSN dont le bilan est non conforme n'en est pas une.
fn dsn_de_l_annee(racine: &Path, lignes: &[Value], an: i32) -> BTreeMap<u32, Vec<Vec<u8>>> {
    let mut dernieres: BTreeMap<(String, String), &Value> = BTreeMap::new();
    for l in lignes {
        let mois = l.get("mois").map(valeur_en_texte).unwrap_or_default();
        if l.get("quoi").and_then(Value::as_str) != Some("dsn") || !reelle(l) || !l.get("accepte").is_some_and(vrai) || nature_dsn(l) != "01" {
            continue;
        }
        if !mois.starts_with(&format!("{}-", an)) {
            continue;
        }
        let etab = l.get("detail").and_then(|d| d.get("etablissement")).filter(|e| vrai(e)).map(valeur_en_texte).unwrap_or_default();
        dernieres.insert((mois, etab), l);
    }
    let mut out: BTreeMap<u32, Vec<Vec<u8>>> = BTreeMap::new();
    for ((mois, _), l) in dernieres {
        let refusee = l.get("retours").and_then(Value::as_array).into_iter().flatten().any(|r| r.get("verdict").and_then(Value::as_str) == Some("KO"));
        let Some(m) = mois.get(5..7).and_then(|m| m.parse::<u32>().ok()) else { continue };
        if refusee {
            continue;
        }
        if let Some(o) = fichier_depose(racine, l) {
            out.entry(m).or_default().push(o);
        }
    }
    out
}

/// Le compte rendu NEOReS de nature 118 de l'Urssaf : l'effectif moyen annuel de l'annee `an` (type de parametre A01,
/// `atelier/reference/sources/neores/gip-mds-neores-table-types-de-parametres-20230703.csv`), dans le profil « parametre
/// utile en paie » du cahier technique NEOReS 2026.1.2 (`atelier/reference/sources/neores/`, p. 27 : `RO.B001`, `RO.B002`,
/// `RO.B004` de type 04, `RO.B008` ; dates AAAA-MM-JJ, date de calcul AAAA-MM-JJThh:mm:ss). La date d'effet et la date de
/// fin bornent l'annee mesuree, comme `releve_retours::reconstruire_effectifs` les lit (inference : le cahier ne dit pas
/// quelle annee les dates portent ; c'est la forme des 118 recus que le lecteur range).
fn crm_effectif(an: i32, valeur: f64, flux: &str, publication: &str) -> Piece {
    let calcule_le = socle::paris::maintenant().chars().take(19).collect::<String>();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><RO><RO_R001>P26V01</RO_R001><RO_B000><RO_B000_R001>{}</RO_B000_R001></RO_B000>\
         <RO_B001><RO_B001_R001>04</RO_B001_R001><RO_B002><RO_B002_R002>{}</RO_B002_R002><RO_B002_R003>Effectifs</RO_B002_R003>\
         <RO_B002_R005>URSSAF</RO_B002_R005></RO_B002><RO_B004><RO_B004_R003>04</RO_B004_R003><RO_B008><RO_B008_R001>A01</RO_B008_R001>\
         <RO_B008_R002>{:.2}</RO_B008_R002><RO_B008_R003>{}-01-01</RO_B008_R003><RO_B008_R004>{}-12-31</RO_B008_R004>\
         <RO_B008_R006>{}</RO_B008_R006></RO_B008></RO_B004></RO_B001></RO>",
        echappe(flux),
        NATURE_EFFECTIFS,
        valeur,
        an,
        an,
        calcule_le
    );
    Piece { retour: retour(NATURE_EFFECTIFS, flux, publication, "Effectifs (compagnie fictive)"), contenu: xml.into_bytes() }
}

/// Ce que les organismes rendent a la DSN que porte la ligne `l` du journal (deja acceptee au transport), au-dela de
/// l'accuse et du bilan conformes : `Err(pieces)` quand net-entreprises la refuse (ses deux bilans, rien d'autre), sinon
/// `Ok(pieces)` a ajouter (le taux du PAS, l'effectif de l'annee precedente a la premiere DSN mensuelle d'une annee).
pub fn reponses_dsn(ctx: &Contexte, memoire: &mut Value, lignes: &[Value], l: &Value, flux: &str, publication: &str) -> R<Result<Vec<Piece>, Vec<Piece>>> {
    if let Some(r) = refus_pour(memoire, l, flux) {
        return Ok(Err(bilans_refuses(flux, publication, l, &r)));
    }
    let mut pieces = Vec::new();
    if !reelle(l) || nature_dsn(l) != "01" {
        return Ok(Ok(pieces));
    }
    if let Some(octets) = fichier_depose(&ctx.racine, l) {
        pieces.extend(crm_pas(ctx, memoire, l, &octets, flux, publication)?);
        pieces.extend(crm_atmp(memoire, l, &octets, flux, publication));
    }
    let an: Option<i32> = l.get("mois").map(valeur_en_texte).and_then(|m| m.get(..4).and_then(|a| a.parse().ok()));
    if let Some(an) = an {
        let precedente = (an - 1).to_string();
        let rendus = memoire["effectifs_rendus"].as_array().into_iter().flatten().any(|a| valeur_en_texte(a) == precedente);
        if !rendus {
            if let Some(e) = effectif_annuel(an - 1, &dsn_de_l_annee(&ctx.racine, lignes, an - 1)) {
                pieces.push(crm_effectif(an - 1, e, flux, publication));
                memoire["effectifs_rendus"].as_array_mut().expect("liste").push(json!(precedente));
            }
        }
    }
    Ok(Ok(pieces))
}

/// La memoire du simulateur, lue pour une releve jouee ; `garder` la reecrit si elle a change.
pub fn memoire(racine: &Path) -> R<Value> {
    charger(racine)
}

pub fn garder(racine: &Path, avant: &Value, apres: &Value) -> R<()> {
    if avant != apres {
        ecrire(racine, apres)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une DSN reduite a ce que le decompte lit : un individu et ses contrats `(debut, fin, modalite, reference, quotite,
    /// motif de recours)`, dates JJMMAAAA.
    fn dsn(individus: &[(&str, &[(&str, &str, &str, &str, &str, &str)])]) -> Vec<u8> {
        let mut t = String::from("S20.G00.05.001,'01'\n");
        for (nir, contrats) in individus {
            t.push_str(&format!("S21.G00.30.001,'{}'\n", nir));
            for (debut, fin, modalite, reference, quotite, motif) in contrats.iter() {
                t.push_str(&format!("S21.G00.40.001,'{}'\n", debut));
                if !fin.is_empty() {
                    t.push_str(&format!("S21.G00.40.010,'{}'\n", fin));
                }
                t.push_str(&format!("S21.G00.40.012,'{}'\nS21.G00.40.013,'{}'\nS21.G00.40.014,'{}'\n", reference, quotite, modalite));
                if !motif.is_empty() {
                    t.push_str(&format!("S21.G00.40.021,'{}'\n", motif));
                }
            }
        }
        t.into_bytes()
    }

    /// R. 130-1 du code de la securite sociale (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261029.md`) :
    /// chacun a due proportion des jours du mois ou il est employe (II), un temps partiel au rapport de ses horaires a la
    /// duree de reference (dernier alinea de L. 1111-2), le CDD de remplacement ecarte (2° de L. 1111-2), les mois sans
    /// salarie hors de la moyenne et le resultat tronque au centieme (I).
    #[test]
    fn l_effectif_annuel_se_compte_comme_r_130_1_le_dit() {
        let mut par_mois: BTreeMap<u32, Vec<Vec<u8>>> = BTreeMap::new();
        // Janvier (31 jours) : un CDI a temps plein tout le mois (1), un mi-temps (75,84 h sur 151,67 : 0,5) du 16 au 31
        // (16 jours : 0,258...), un CDDU de deux jours (2/31), un CDD de remplacement (ecarte).
        par_mois.insert(
            1,
            vec![dsn(&[
                ("1800101999991", &[("01012020", "", "10", "151.67", "151.67", "")]),
                ("2800101999992", &[("16012025", "", "20", "151.67", "75.84", "")]),
                ("1800101999993", &[("20012025", "21012025", "99", "2.00", "2.00", "05")]),
                ("2800101999994", &[("01012025", "31012025", "10", "151.67", "151.67", "01")]),
            ])],
        );
        // Fevrier (28 jours) : le seul CDI ; mars a decembre sans salarie, hors de la moyenne.
        par_mois.insert(2, vec![dsn(&[("1800101999991", &[("01012020", "", "10", "151.67", "151.67", "")])])]);
        let janvier: f64 = 1.0 + 0.5 * 16.0 / 31.0 + 2.0 / 31.0;
        let attendu = ((janvier + 1.0) / 2.0 * 100.0).floor() / 100.0;
        assert_eq!(effectif_annuel(2025, &par_mois), Some(attendu));
        assert_eq!(attendu, 1.16);
        assert_eq!(effectif_annuel(2025, &BTreeMap::new()), None);
    }

    /// Le compte rendu 118 que le simulateur rend se lit par le lecteur des vrais (`releve_retours::reconstruire_effectifs`,
    /// cahier technique NEOReS 2026.1.2) : `config/effectifs.json` porte l'effectif A01 de l'annee mesuree.
    #[test]
    fn le_118_rendu_s_ecrit_dans_les_effectifs_de_la_compagnie() {
        let racine = std::env::temp_dir().join(format!("envois-118-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        std::fs::create_dir_all(racine.join("retours")).unwrap();
        std::fs::create_dir_all(racine.join("config")).unwrap();
        let p = crm_effectif(2026, 11.42, "FICTIF-000000000001", "20270215100000");
        let chemin = crate::transport::archiver(&racine, "", &p.retour, &p.contenu, "application/xml", "20270215-100000").unwrap();
        assert!(chemin.file_name().unwrap().to_string_lossy().contains("_nature118_"));
        crate::releve_retours::reconstruire_effectifs(&racine).unwrap();
        let e: Value = serde_json::from_str(&std::fs::read_to_string(racine.join("config/effectifs.json")).unwrap()).unwrap();
        assert_eq!(e["annees"]["2026"]["A01"]["valeur"], json!(11.42));
        let _ = std::fs::remove_dir_all(&racine);
    }
}
