//! `moteur-atelier fabriquer-bulletins <description.json> <dossier> [--gabarit coolpaie|spaiectacle]` (#236) : les
//! bulletins FICTIFS d'un logiciel precedent, un PDF par bulletin, pour qu'une compagnie fictive entre dans PuppetPaie
//! « D'un logiciel précédent » en important ses bulletins par la page.
//!
//! La page se dessine par la fabrique des tests (`bulletins_fictifs.rs`, a cote : un seul code), a la maniere du
//! logiciel choisi ; elle se relit par `bulletin_lu` comme un vrai bulletin. Ici, seulement ce qu'un
//! ancien logiciel aurait calcule et imprime, a partir d'une description courte (le format : `atelier/outils/banc/
//! LISEZ-MOI.md`, « Une compagnie qui vient d'un logiciel précédent ») :
//!
//! - le brut, somme des lignes soumises (quantite x taux, au centime) ;
//! - des cotisations VRAISEMBLABLES (`COTISATIONS` : des taux d'un faux bulletin, ni relevés ni recalcules par le
//!   moteur -- l'import ne recalcule rien, le bulletin fait foi) ; elles sont COHERENTES, comme le lecteur les controle :
//!   net avant impot = brut - cotisations salariales + lignes non soumises ; net a payer = net avant impot - impot ;
//! - le net imposable (le net avant impot hors non soumis, plus la CSG/CRDS non deductible), le PAS au taux donne ;
//! - les cumuls de l'annee par salarie (cachets, heures, jours, brut, plafond, cout employeur, net imposable, PAS), dans
//!   l'ordre du temps, tels que `bulletin_lu::compteurs` les confronte aux lignes : ils s'accordent.
//!
//! RIEN DE REEL (regle 9 a) : l'identite vient de la description, et le NIR (cle juste) comme le Siret (cle de Luhn)
//! sont verifies. Une description qui ferait une page que l'import ne reprendrait pas entiere est refusee, avec sa
//! raison : un libelle de ligne que le gabarit ne connait pas ou qui n'a pas de rubrique sure (la page n'aurait pas de
//! contrat), une ligne non soumise sur CoolPaie (son gabarit n'en a pas), des dates hors de la periode, une page qui
//! deborderait.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use rust_decimal::RoundingStrategy;
use serde_json::Value;

use super::bulletins_fictifs::{page_a, page_b, pdf_octets, Identite, RangA, RangB, A, B};
use crate::import::bulletin_lu::{jetons, Table};
use socle::date::Jour;
use socle::num::{eur2, Decimal, Resultat};

pub const USAGE: &str = "usage : moteur-atelier fabriquer-bulletins <description.json> <dossier> [--gabarit coolpaie|spaiectacle]";

// --------------------------------------------------------------------------------------------- la ligne de commande

pub fn commande(args: &[String]) -> i32 {
    let mut libres = Vec::new();
    let mut gabarit = None;
    let mut k = 0;
    while k < args.len() {
        if args[k] == "--gabarit" {
            let Some(g) = args.get(k + 1) else {
                eprintln!("{}", USAGE);
                return 2;
            };
            gabarit = Some(g.clone());
            k += 2;
        } else {
            libres.push(args[k].clone());
            k += 1;
        }
    }
    let [description, dossier] = libres.as_slice() else {
        eprintln!("{}", USAGE);
        return 2;
    };
    let r = std::fs::read_to_string(description)
        .map_err(|e| format!("{} illisible ({})", description, e))
        .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| format!("{} : JSON illisible ({})", description, e)))
        .and_then(|d| fabriquer(&d, gabarit.as_deref(), Path::new(dossier)));
    match r {
        Ok(faits) => {
            for f in &faits {
                println!("{}", f);
            }
            println!("{} bulletin(s) fabrique(s) dans {}", faits.len(), dossier);
            0
        }
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}

// --------------------------------------------------------------------------------------------- les taux du faux logiciel

#[derive(Clone, Copy, PartialEq)]
enum Base {
    Brut,
    Plafonnee,
    Csg,
}

/// Les cotisations d'un FAUX bulletin : (cle, base, taux employeur et salarie de l'intermittent, puis du permanent).
/// Des taux vraisemblables d'un logiciel de paie, en pourcentage ; « » : pas de part. Ils ne fondent aucune regle (le
/// moteur ne s'en sert pas, l'import ne les recalcule pas) : ils donnent des montants coherents a relire.
const COTISATIONS: [(&str, Base, &str, &str, &str, &str); 13] = [
    ("maladie", Base::Brut, "7.000", "", "7.000", ""),
    ("prevoyance", Base::Brut, "1.040", "0.120", "0.800", "0.800"),
    ("atmp", Base::Brut, "1.050", "", "1.050", ""),
    ("vieillesse_plafonnee", Base::Plafonnee, "8.550", "6.900", "8.550", "6.900"),
    ("vieillesse_deplafonnee", Base::Brut, "2.020", "0.400", "2.020", "0.400"),
    ("complementaire", Base::Plafonnee, "6.010", "4.010", "6.010", "4.010"),
    ("famille", Base::Brut, "3.450", "", "3.450", ""),
    ("chomage", Base::Brut, "9.050", "2.400", "4.300", ""),
    ("autres", Base::Brut, "2.836", "", "2.836", ""),
    ("conges_spectacles", Base::Brut, "15.500", "", "", ""),
    ("conventionnelles", Base::Brut, "0.300", "", "0.300", ""),
    ("csg_deductible", Base::Csg, "", "6.800", "", "6.800"),
    ("csg_non_deductible", Base::Csg, "", "2.900", "", "2.900"),
];

/// La part du brut soumise a la CSG (abattement de 1,75 % pour frais professionnels).
const ASSIETTE_CSG: &str = "0.9825";

/// La table des parametres du BOSS deposee : elle porte le PSS de 2021 a 2026 (codes PSS_M_METROP et PSS_H_METROP),
/// les annees qu'aucun bareme livre n'a (2024).
fn table_boss() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/boss-table-parametres-2026.csv")
}

/// LE PLAFOND DE LA SECURITE SOCIALE D'UNE ANNEE (mensuel, horaire), d'un texte et non de memoire (#236, regle 1) :
/// le bareme livre du moteur (`logiciel/donnees/baremes/<annee>.json`, `plafond_securite_sociale`, qui cite sa
/// source), sinon la table des parametres du BOSS deposee (`table_boss`), sinon refus -- l'annee manque, on ne
/// l'invente pas.
fn pss(annee: i32) -> Resultat<(Decimal, Decimal)> {
    let bareme = socle::fichiers::dossier_du_code().join("baremes").join(format!("{}.json", annee));
    if let Ok(t) = std::fs::read_to_string(&bareme) {
        let v: Value = serde_json::from_str(&t).map_err(|e| format!("{} illisible ({})", bareme.display(), e))?;
        let p = &v["plafond_securite_sociale"];
        if let (Some(m), Some(h)) = (p.get("mensuel"), p.get("horaire")) {
            return Ok((socle::num::dec(m)?, socle::num::dec(h)?));
        }
    }
    let chemin = table_boss();
    let octets = std::fs::read(&chemin).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    let texte = String::from_utf8_lossy(&octets);
    let valeur = |code: &str| -> Option<Decimal> {
        texte.lines().find_map(|l| {
            let c: Vec<&str> = l.split(';').collect();
            let effet = format!("01/01/{}", annee);
            (c.len() >= 4 && c[0] == code && c[3].trim() == effet)
                .then(|| c[2].replace([' ', '\u{a0}'], "").replace(',', ".").parse::<Decimal>().ok())
                .flatten()
        })
    };
    match (valeur("PSS_M_METROP"), valeur("PSS_H_METROP")) {
        (Some(m), Some(h)) => Ok((m, h)),
        _ => Err(format!(
            "pas de plafond de la Sécurité sociale pour {} : ni bareme livré (logiciel/donnees/baremes/{}.json) ni ligne \
             PSS_M_METROP/PSS_H_METROP au 01/01/{} dans {} -- déposez le texte qui le donne",
            annee,
            annee,
            annee,
            chemin.display()
        )),
    }
}

// --------------------------------------------------------------------------------------------- la description lue

struct Salarie {
    code: String,
    nom: String,
    prenom: String,
    nir: String,
    naissance: Jour,
    adresse: [String; 2],
    conges_spectacles: Option<String>,
}

struct Ligne {
    libelle: String,
    quantite: Decimal,
    taux: Decimal,
    montant: Decimal,
    cachets: Decimal,
    heures: Decimal,
}

struct Bulletin {
    rang: usize,
    salarie: usize,
    emploi: String,
    statut: String,
    permanent: bool,
    debut: Jour,
    fin: Jour,
    dates: Vec<(Jour, Jour)>,
    objet: Option<(String, String)>,
    analytique: String,
    lignes: Vec<Ligne>,
    non_soumis: Vec<Ligne>,
    taux_pas: Decimal,
    regle_le: Jour,
    numero: Option<String>,
}

fn texte(v: &Value, cle: &str, ou: &str) -> Resultat<String> {
    match v.get(cle) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        _ => Err(format!("{} : `{}` manque", ou, cle)),
    }
}

fn texte_ou(v: &Value, cle: &str) -> Option<String> {
    texte(v, cle, "").ok()
}

fn jour(v: &Value, cle: &str, ou: &str) -> Resultat<Jour> {
    Jour::lire(&texte(v, cle, ou)?).map_err(|m| format!("{} : `{}` : {}", ou, cle, m))
}

fn nombre(v: &Value, cle: &str, ou: &str) -> Resultat<Decimal> {
    let x = v.get(cle).ok_or_else(|| format!("{} : `{}` manque", ou, cle))?;
    socle::num::dec(x).map_err(|m| format!("{} : `{}` : {}", ou, cle, m))
}

fn verifier_nir(nir: &str, ou: &str) -> Resultat<()> {
    let ok = nir.len() == 15 && nir.is_ascii() && declarations::champs::cle_nir(&nir[..13]).as_deref() == Some(&nir[13..]);
    if ok {
        Ok(())
    } else {
        Err(format!("{} : le NIR {} n'a pas quinze caractères ou sa clé est fausse (97 - NIR mod 97)", ou, nir))
    }
}

/// Un texte coupe en rangs d'au plus `largeur` caracteres, aux espaces.
fn rangs(texte: &str, largeur: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for mot in texte.split_whitespace() {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + mot.chars().count() <= largeur => {
                l.push(' ');
                l.push_str(mot);
            }
            _ => out.push(mot.to_string()),
        }
    }
    out
}

/// Une valeur sur au plus deux rangs, sinon le refus qui le dit.
fn deux_rangs(texte: &str, largeur: usize, quoi: &str) -> Resultat<[String; 2]> {
    let mut r = rangs(texte, largeur);
    if r.len() > 2 || r.iter().any(|l| l.chars().count() > largeur) {
        return Err(format!("{} « {} » ne tient pas sur deux rangs de {} caractères : abrégez-le", quoi, texte, largeur));
    }
    r.resize(2, String::new());
    Ok([r[0].clone(), r[1].clone()])
}

// --------------------------------------------------------------------------------------------- l'ecriture des nombres

/// « 1 234,56 », comme les bulletins l'impriment.
fn fr(x: Decimal) -> String {
    let x = eur2(x);
    let s = x.abs().to_string();
    let (ent, dec) = s.split_once('.').unwrap_or((&s, "00"));
    let mut groupes = Vec::new();
    let c: Vec<char> = ent.chars().collect();
    let mut fin = c.len();
    while fin > 3 {
        groupes.push(c[fin - 3..fin].iter().collect::<String>());
        fin -= 3;
    }
    groupes.push(c[..fin].iter().collect::<String>());
    groupes.reverse();
    format!("{}{},{}", if x.is_sign_negative() && !x.is_zero() { "-" } else { "" }, groupes.join(" "), dec)
}

/// Un taux a trois decimales, « 6,900 ».
fn taux3(x: Decimal) -> String {
    let mut t = x.round_dp_with_strategy(3, RoundingStrategy::MidpointAwayFromZero);
    t.rescale(3);
    t.to_string().replace('.', ",")
}

/// Un nombre sans zeros inutiles (une quantite : « 3 », « 7,5 »).
fn quantite(x: Decimal) -> String {
    x.normalize().to_string().replace('.', ",")
}

fn dmy(j: &Jour) -> String {
    j.jj_mm_aaaa()
}

fn dmy_court(j: &Jour) -> String {
    format!("{:02}/{:02}/{:02}", j.jour, j.mois, j.an % 100)
}

fn pct(base: Decimal, taux: &str) -> Option<Decimal> {
    (!taux.is_empty()).then(|| socle::num::pct(base, taux.parse::<Decimal>().unwrap_or_default()))
}

// --------------------------------------------------------------------------------------------- le calcul du faux bulletin

#[derive(Default, Clone)]
struct Cumuls {
    cachets: Decimal,
    heures: Decimal,
    jours: Decimal,
    brut: Decimal,
    plafond: Decimal,
    cout: Decimal,
    net_imposable: Decimal,
    pas: Decimal,
}

/// Ce que le faux logiciel a calcule pour un bulletin.
struct Calcule {
    brut: Decimal,
    plafond: Decimal,
    assiette_csg: Decimal,
    /// (cle, base, taux employeur, montant employeur, taux salarie, montant salarie)
    cotisations: Vec<(&'static str, Decimal, &'static str, Option<Decimal>, &'static str, Option<Decimal>)>,
    total_employeur: Decimal,
    total_salarie: Decimal,
    net_social: Decimal,
    net_avant_impot: Decimal,
    net_imposable: Decimal,
    pas: Decimal,
    net_a_payer: Decimal,
    cout: Decimal,
    cachets: Decimal,
    heures: Decimal,
    jours: Decimal,
    periode: Cumuls,
    annee: Cumuls,
}

fn calculer(b: &Bulletin, avant: &Cumuls) -> Resultat<Calcule> {
    let brut: Decimal = b.lignes.iter().map(|l| l.montant).sum();
    let jours_travailles: BTreeSet<Jour> = b
        .dates
        .iter()
        .flat_map(|(d, f)| (0..=f.moins(d)).filter_map(move |k| d.plus_jours(k).ok()))
        .collect();
    let jours = Decimal::from(jours_travailles.len());
    // LE « PLAFOND URSSAF » IMPRIME, PAR LA REGLE DU MOTEUR (`plafond.rs`, qui cite ses textes) et le PSS d'un texte
    // (`pss`) : le prorata calendaire pour un permanent, les journees de l'artiste engage moins de cinq jours, le
    // prorata des jours remuneres pour les autres intermittents.
    let t = |j: &Jour| (j.an, j.mois, j.jour);
    let resoudre = |annee: i32| pss(annee);
    let plafond = if b.permanent {
        droit::plafond::plafond_permanent(t(&b.debut), t(&b.fin), None, &resoudre)
    } else {
        let dates: Vec<_> = jours_travailles.iter().map(t).collect();
        droit::plafond::plafond_urssaf(t(&b.debut), t(&b.fin), jetons(&b.statut).concat() == "artiste", &resoudre, &dates)
    }
    .map_err(|m| format!("bulletin {} : {}", b.rang, m))?;
    let assiette_csg = eur2(brut * ASSIETTE_CSG.parse::<Decimal>().unwrap_or_default());
    let mut cotisations = Vec::new();
    let (mut total_e, mut total_s, mut csg_non_deductible) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for (cle, base, ei, si, ep, sp) in COTISATIONS {
        let (te, ts) = if b.permanent { (ep, sp) } else { (ei, si) };
        if te.is_empty() && ts.is_empty() {
            continue;
        }
        let assiette = match base {
            Base::Brut => brut,
            Base::Plafonnee => brut.min(plafond),
            Base::Csg => assiette_csg,
        };
        let (me, ms) = (pct(assiette, te), pct(assiette, ts));
        total_e += me.unwrap_or_default();
        total_s += ms.unwrap_or_default();
        if cle == "csg_non_deductible" {
            csg_non_deductible = ms.unwrap_or_default();
        }
        cotisations.push((cle, assiette, te, me, ts, ms));
    }
    let non_soumis: Decimal = b.non_soumis.iter().map(|l| l.montant).sum();
    let net_social = brut - total_s;
    let net_avant_impot = net_social + non_soumis;
    let net_imposable = net_social + csg_non_deductible;
    let pas = socle::num::pct(net_imposable, b.taux_pas);
    let cout = brut + total_e + non_soumis;
    let cachets: Decimal = b.lignes.iter().map(|l| l.cachets).sum();
    let heures: Decimal = b.lignes.iter().map(|l| l.heures).sum();
    let periode = Cumuls { cachets, heures, jours, brut, plafond, cout, net_imposable, pas };
    let annee = Cumuls {
        cachets: avant.cachets + cachets,
        heures: avant.heures + heures,
        jours: avant.jours + jours,
        brut: avant.brut + brut,
        plafond: avant.plafond + plafond,
        cout: avant.cout + cout,
        net_imposable: avant.net_imposable + net_imposable,
        pas: avant.pas + pas,
    };
    Ok(Calcule {
        brut,
        plafond,
        assiette_csg,
        cotisations,
        total_employeur: total_e,
        total_salarie: total_s,
        net_social,
        net_avant_impot,
        net_imposable,
        pas,
        net_a_payer: net_avant_impot - pas,
        cout,
        cachets,
        heures,
        jours,
        periode,
        annee,
    })
}

// --------------------------------------------------------------------------------------------- la lecture de la description

struct Description {
    gabarit: String,
    entreprise: Value,
    salaries: Vec<Salarie>,
    bulletins: Vec<Bulletin>,
}

fn lire_description(d: &Value, gabarit: Option<&str>, racine_catalogue: &Path) -> Resultat<Description> {
    let gabarit = gabarit
        .map(str::to_string)
        .or_else(|| texte_ou(d, "gabarit"))
        .ok_or("le gabarit manque : `gabarit` dans la description, ou --gabarit coolpaie|spaiectacle")?;
    if !matches!(gabarit.as_str(), "coolpaie" | "spaiectacle") {
        return Err(format!("gabarit inconnu : {} (coolpaie ou spaiectacle)", gabarit));
    }
    let g = Table::livree()?;

    let e = d.get("entreprise").filter(|v| v.is_object()).ok_or("`entreprise` manque")?.clone();
    for cle in ["raison_sociale", "adresse", "code_postal", "commune", "siret", "naf"] {
        texte(&e, cle, "entreprise")?;
    }
    let siret = texte(&e, "siret", "entreprise")?;
    if siret.len() != 14 || !declarations::champs::luhn(&siret) {
        return Err(format!("entreprise : le Siret {} n'a pas quatorze chiffres ou sa clé de Luhn est fausse", siret));
    }
    let conv = e.get("convention").ok_or("entreprise : `convention` manque ({\"idcc\": …, \"libelle\": …})")?;
    let idcc = texte(conv, "idcc", "entreprise.convention")?;
    if idcc.len() != 4 || !idcc.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("entreprise.convention : l'IDCC {} n'a pas quatre chiffres", idcc));
    }
    texte(conv, "libelle", "entreprise.convention")?;

    let mut salaries = Vec::new();
    let mut codes: HashMap<String, usize> = HashMap::new();
    for (k, s) in d.get("salaries").and_then(Value::as_array).ok_or("`salaries` manque (une liste)")?.iter().enumerate() {
        let ou = format!("salarié {}", k + 1);
        let code = texte(s, "code", &ou)?;
        let nir = texte(s, "nir", &ou)?;
        verifier_nir(&nir, &ou)?;
        if codes.insert(code.clone(), k).is_some() {
            return Err(format!("{} : le code {} est déjà pris", ou, code));
        }
        salaries.push(Salarie {
            code,
            nom: texte(s, "nom", &ou)?,
            prenom: texte(s, "prenom", &ou)?,
            nir,
            naissance: jour(s, "naissance", &ou)?,
            adresse: [texte(s, "adresse", &ou)?, format!("{} {}", texte(s, "code_postal", &ou)?, texte(s, "commune", &ou)?)],
            conges_spectacles: texte_ou(s, "conges_spectacles"),
        });
    }

    let lire_ligne = |l: &Value, ou: &str, soumise: bool| -> Resultat<Ligne> {
        let libelle = texte(l, "libelle", ou)?;
        let canon = jetons(&libelle);
        let connu = g.lignes().iter().find(|x| jetons(&x.libelle) == canon).ok_or_else(|| {
            let connus: Vec<&str> = g.lignes().iter().map(|x| x.libelle.as_str()).collect();
            format!("{} : « {} » n'est pas un libellé de la table de lecture ({})", ou, libelle, connus.join(", "))
        })?;
        let code = connu.rubrique.clone().ok_or_else(|| {
            format!("{} : « {} » n'a pas de rubrique sûre dans la table : la page se lirait sans contrat", ou, connu.libelle)
        })?;
        // Le dossier de fabrication n'a pas de rubriques de la compagnie : le catalogue livre, le meme a toute date.
        let r = droit::catalogue::rubrique(racine_catalogue, &code, &droit::contexte::jour_reel())?;
        if r.soumis != soumise {
            return Err(format!(
                "{} : « {} » ({}) est une ligne {} : mettez-la dans `{}`",
                ou,
                connu.libelle,
                code,
                if r.soumis { "soumise" } else { "non soumise" },
                if r.soumis { "lignes" } else { "non_soumis" }
            ));
        }
        let q = nombre(l, "quantite", ou)?;
        let taux = nombre(l, "taux", ou)?;
        let (mut cachets, mut heures) = (Decimal::ZERO, Decimal::ZERO);
        if r.unite == "cachet" {
            cachets = q;
        } else if let Some(u) = r.heures_forfait.map(socle::num::dec_f64).transpose()?.or(connu.heures).or_else(|| (r.unite == "heure").then_some(Decimal::ONE)) {
            heures = q * u;
        }
        Ok(Ligne { libelle: connu.libelle.clone(), quantite: q, taux, montant: eur2(q * taux), cachets, heures })
    };

    let mut bulletins = Vec::new();
    for (k, b) in d.get("bulletins").and_then(Value::as_array).ok_or("`bulletins` manque (une liste)")?.iter().enumerate() {
        let ou = format!("bulletin {}", k + 1);
        let code = texte(b, "salarie", &ou)?;
        let salarie = *codes.get(&code).ok_or_else(|| format!("{} : aucun salarié de code {}", ou, code))?;
        let (debut, fin) = (jour(b, "debut", &ou)?, jour(b, "fin", &ou)?);
        if fin < debut {
            return Err(format!("{} : la fin précède le début", ou));
        }
        let mut dates = Vec::new();
        match b.get("dates").and_then(Value::as_array) {
            None => dates.push((debut, fin)),
            Some(l) => {
                for x in l {
                    let paire = x.as_array().filter(|p| p.len() == 2).ok_or_else(|| format!("{} : `dates` : des paires [début, fin]", ou))?;
                    let lire = |v: &Value| v.as_str().ok_or("une date en texte".to_string()).and_then(Jour::lire).map_err(|m| format!("{} : `dates` : {}", ou, m));
                    let (d0, d1) = (lire(&paire[0])?, lire(&paire[1])?);
                    if d1 < d0 || d0 < debut || d1 > fin {
                        return Err(format!("{} : `dates` : {} - {} hors de la période", ou, d0.iso(), d1.iso()));
                    }
                    dates.push((d0, d1));
                }
            }
        }
        let lignes = b
            .get("lignes")
            .and_then(Value::as_array)
            .filter(|l| !l.is_empty())
            .ok_or_else(|| format!("{} : `lignes` manque", ou))?
            .iter()
            .map(|l| lire_ligne(l, &ou, true))
            .collect::<Resultat<Vec<_>>>()?;
        let non_soumis = b.get("non_soumis").and_then(Value::as_array).map(|l| l.iter().map(|x| lire_ligne(x, &ou, false)).collect::<Resultat<Vec<_>>>()).transpose()?.unwrap_or_default();
        if gabarit == "coolpaie" && !non_soumis.is_empty() {
            return Err(format!("{} : le gabarit CoolPaie n'a pas de ligne non soumise", ou));
        }
        let regime = texte_ou(b, "regime").unwrap_or_else(|| "intermittent".into());
        if !matches!(regime.as_str(), "intermittent" | "permanent") {
            return Err(format!("{} : `regime` vaut intermittent ou permanent, lu {}", ou, regime));
        }
        let objet = match b.get("objet") {
            None | Some(Value::Null) => None,
            Some(o) => Some((texte(o, "numero", &format!("{}.objet", ou))?, texte_ou(o, "titre").unwrap_or_default())),
        };
        bulletins.push(Bulletin {
            rang: k + 1,
            salarie,
            emploi: texte(b, "emploi", &ou)?,
            statut: texte(b, "statut", &ou)?,
            permanent: regime == "permanent",
            debut,
            fin,
            dates,
            objet,
            analytique: texte_ou(b, "analytique").unwrap_or_default(),
            lignes,
            non_soumis,
            taux_pas: b.get("taux_pas").map(socle::num::dec).transpose().map_err(|m| format!("{} : `taux_pas` : {}", ou, m))?.unwrap_or_default(),
            regle_le: if b.get("regle_le").is_some() { jour(b, "regle_le", &ou)? } else { fin },
            numero: texte_ou(b, "numero"),
        });
    }

    if d.get("plafonds").is_some() {
        return Err("`plafonds` ne se donne plus : le plafond de la Sécurité sociale se lit dans le barème livré ou la table du BOSS déposée".into());
    }
    Ok(Description { gabarit, entreprise: e, salaries, bulletins })
}

// --------------------------------------------------------------------------------------------- les pages

fn identite(d: &Description, s: &Salarie) -> Resultat<Identite<String>> {
    let e = &d.entreprise;
    let t = |k: &str| texte_ou(e, k).unwrap_or_default();
    let conv = &e["convention"];
    let (idcc, libelle) = (texte_ou(conv, "idcc").unwrap_or_default(), texte_ou(conv, "libelle").unwrap_or_default());
    let convention = if d.gabarit == "coolpaie" {
        let [a, b] = deux_rangs(&format!("IDCC n° {} - {}", idcc, libelle), 34, "la convention")?;
        [a, b]
    } else {
        deux_rangs(&format!("{} (IDCC {})", libelle, idcc), 46, "la convention")?
    };
    Ok(Identite {
        entreprise: [t("raison_sociale"), t("adresse"), format!("{} {}", t("code_postal"), t("commune"))],
        siret: t("siret"),
        naf: t("naf"),
        urssaf: t("urssaf"),
        convention,
        licence: t("licence"),
        nom: s.nom.clone(),
        prenom: s.prenom.clone(),
        adresse: s.adresse.clone(),
        naissance: dmy(&s.naissance),
        nir: s.nir.clone(),
        code: s.code.clone(),
        conges_spectacles: s.conges_spectacles.clone(),
    })
}

fn libelle_a(cle: &str) -> (Option<&'static str>, f64, &'static str, &'static str) {
    // (titre de famille sur son propre rang, x du libelle, libelle, famille)
    match cle {
        "maladie" => (Some("SANTE"), 33.9, "Sécurité sociale - Maladie Maternité Invalidité Décès", "sante"),
        "prevoyance" => (None, 33.9, "Complémentaire Incapacité Invalidité Décès", "sante"),
        "atmp" => (Some("ACCIDENTS DU TRAVAIL - MALADIES"), 40.0, "PROFESSIONNELLES", "accidents_du_travail"),
        "vieillesse_plafonnee" => (Some("RETRAITE"), 33.9, "Sécurité sociale plafonnée", "retraite"),
        "vieillesse_deplafonnee" => (None, 33.9, "Sécurité sociale déplafonnée", "retraite"),
        "complementaire" => (None, 33.9, "Complémentaire Tranche 1", "retraite"),
        "famille" => (None, 33.9, "FAMILLE", "famille"),
        "chomage" => (Some("ASSURANCE CHOMAGE"), 33.9, "Chômage", "assurance_chomage"),
        "autres" => (Some("AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR"), 33.9, "autres contributions dues par l'employeur", "autres_contributions"),
        "conges_spectacles" => (None, 33.9, "CONGES SPECTACLES", "conges_spectacles"),
        "conventionnelles" => (Some("COTISATIONS STATUTAIRES OU PREVUES PAR LA"), 40.0, "CONVENTION COLLECTIVE", "cotisations_conventionnelles"),
        "csg_deductible" => (None, 33.9, "CSG déductible à l'impôt sur le revenu", "csg"),
        _ => (None, 33.9, "CSG/CRDS non déductible à l'impôt sur le revenu", "csg"),
    }
}

fn libelle_b(cle: &str) -> (Option<&'static str>, f64, &'static str, &'static str) {
    match cle {
        "maladie" => (Some("Santé"), 74.0, "Sécurité Sociale - Maladie Maternité Invalidité Décès", "sante"),
        "prevoyance" => (None, 74.0, "Prévoyance, Incapacité, Invalidité, Décès, Autres", "sante"),
        "atmp" => (None, 39.0, "Accidents du travail & maladies professionnelles", "accidents_du_travail"),
        "vieillesse_plafonnee" => (Some("Retraite"), 74.0, "Sécurité Sociale Vieillesse plafonnée", "retraite"),
        "vieillesse_deplafonnee" => (None, 74.0, "Sécurité Sociale Vieillesse déplafonnée", "retraite"),
        "complementaire" => (None, 74.0, "Retraite complémentaire, CEG et CET T1", "retraite"),
        "famille" => (None, 39.0, "Famille", "famille"),
        "chomage" => (None, 39.0, "Assurance chômage", "assurance_chomage"),
        "autres" => (None, 39.0, "Autres charges dues par l\u{2019}employeur", "autres_contributions"),
        "conges_spectacles" => (None, 39.0, "Congés Spectacles", "conges_spectacles"),
        "conventionnelles" => (None, 39.0, "Cotisations statutaires ou prévues par la convention collective", "cotisations_conventionnelles"),
        "csg_deductible" => (None, 39.0, "CSG déductible de l'impôt sur le revenu", "csg"),
        _ => (None, 39.0, "CSG/CRDS non déductible de l'impôt sur le revenu", "csg"),
    }
}

fn dates_a(dates: &[(Jour, Jour)]) -> Resultat<Vec<String>> {
    let morceaux: Vec<String> = dates.iter().map(|(d, f)| format!("du {} au {}", dmy(d), dmy(f))).collect();
    let rangs: Vec<String> = morceaux
        .chunks(2)
        .enumerate()
        .map(|(k, c)| {
            let mut r = c.join(", ");
            if (k + 1) * 2 < morceaux.len() {
                r.push(',');
            }
            r
        })
        .collect();
    // Trois rangs tiennent entre « Dates travaillées » et le tableau.
    if rangs.len() > 3 {
        return Err(format!("{} intervalles de dates : CoolPaie n'en imprime que six, regroupez-les", morceaux.len()));
    }
    Ok(rangs)
}

fn page_coolpaie(d: &Description, b: &Bulletin, c: &Calcule, numero: String) -> Resultat<Vec<Value>> {
    let s = &d.salaries[b.salarie];
    let mut cotisations: Vec<RangA<String>> = Vec::new();
    let vide = String::new;
    for (cle, base, te, me, ts, ms) in &c.cotisations {
        let (titre, x, libelle, famille) = libelle_a(cle);
        if let Some(t) = titre {
            cotisations.push((33.9, t.into(), "", vide(), vide(), vide(), vide(), vide()));
        }
        let taux = |t: &str| if t.is_empty() { String::new() } else { taux3(t.parse().unwrap_or_default()) };
        cotisations.push((x, libelle.into(), famille, fr(*base), taux(te), me.map(fr).unwrap_or_default(), taux(ts), ms.map(fr).unwrap_or_default()));
    }
    let cachets = c.periode.cachets > Decimal::ZERO;
    let (unite, p, a) = if cachets { ("Cachets:", c.periode.cachets, c.annee.cachets) } else { ("Heures:", c.periode.heures, c.annee.heures) };
    let lignes: Vec<(String, String, String, String)> = b.lignes.iter().map(|l| (quantite(l.quantite), l.libelle.clone(), fr(l.taux), fr(l.montant))).collect();
    // La page tient-elle ? Le pied finit a 72 points sous le net a payer, la signature est a 810.
    let bas = 308.0 + 11.2 * (lignes.len() + 1 + cotisations.len()) as f64 + 14.2 + 22.0 + 20.3 + 11.0 + 11.5 + 14.0 + 8.5 + 8.5 + 13.0 + 72.0;
    if bas > 800.0 {
        return Err(format!("bulletin {} : trop de lignes pour une page CoolPaie", b.rang));
    }
    let a_ = A {
        id: identite(d, s)?,
        numero,
        objet: b.objet.clone(),
        emploi: b.emploi.clone(),
        statut: b.statut.clone(),
        periode: format!("du {} au {}", dmy(&b.debut), dmy(&b.fin)),
        dates: dates_a(&b.dates)?,
        lignes,
        brut: fr(c.brut),
        csg: fr(c.assiette_csg),
        cotisations,
        total: [fr(c.total_employeur), fr(c.total_salarie)],
        net: fr(c.net_avant_impot),
        pas: [fr(c.net_imposable), format!("{:.3}", b.taux_pas.round_dp(3)), format!("{:.2}", c.pas), fr(c.annee.pas)],
        net_imposable: [fr(c.net_imposable), fr(c.annee.net_imposable)],
        net_a_payer: fr(c.net_a_payer),
        regle: dmy(&b.regle_le),
        mode: b.fin.an >= 2023,
        unite: unite.into(),
        cumuls: [
            [fr(p), fr(a)],
            [fr(c.brut), fr(c.annee.brut)],
            [fr(c.plafond), fr(c.annee.plafond)],
            [fr(c.cout), fr(c.annee.cout)],
        ],
        total_verse: fr(c.cout),
    };
    Ok(page_a(&a_))
}

fn page_spaiectacle(d: &Description, b: &Bulletin, c: &Calcule) -> Resultat<Vec<Value>> {
    let s = &d.salaries[b.salarie];
    let vide = String::new;
    let mut cotisations: Vec<RangB<String>> = Vec::new();
    for (cle, base, _, me, ts, ms) in &c.cotisations {
        let (titre, x, libelle, famille) = libelle_b(cle);
        if let Some(t) = titre {
            cotisations.push((39.0, t.into(), "", vide(), vide(), vide(), vide()));
        }
        let taux = if ts.is_empty() { String::new() } else { taux3(ts.parse().unwrap_or_default()) };
        cotisations.push((x, libelle.into(), famille, fr(*base), taux, ms.map(fr).unwrap_or_default(), me.map(fr).unwrap_or_default()));
    }
    let ligne = |l: &Ligne| (quantite(l.quantite), l.libelle.clone(), fr(l.taux), fr(l.montant));
    let lignes: Vec<_> = b.lignes.iter().map(ligne).collect();
    let non_soumis: Vec<_> = b.non_soumis.iter().map(ligne).collect();
    let titres = cotisations.iter().filter(|c| c.0 < 50.0).count() as f64;
    let bas = 308.5 + 11.0 * lignes.len() as f64 + 9.0 + 13.0 * titres + 10.0 * (cotisations.len() as f64 - titres) + 13.0 + 18.0 + 18.0 * non_soumis.len() as f64;
    if bas > 645.0 {
        return Err(format!("bulletin {} : trop de lignes pour une page sPAIEctacle", b.rang));
    }
    let mut cumuls = Vec::new();
    if c.annee.cachets > Decimal::ZERO {
        cumuls.push(("Cachets".to_string(), fr(c.periode.cachets), fr(c.annee.cachets)));
    }
    cumuls.push(("Jours".to_string(), fr(c.periode.jours), fr(c.annee.jours)));
    if c.heures > Decimal::ZERO {
        // sPAIEctacle compte les cachets dans ses heures, a 12 h (le forfait du catalogue) ; les cumuls de l'annee aussi.
        let h = |x: &Cumuls| x.heures + x.cachets * Decimal::from(12);
        cumuls.push(("Heures".to_string(), fr(h(&c.periode)), fr(h(&c.annee))));
    }
    cumuls.push(("Salaire brut".to_string(), fr(c.brut), fr(c.annee.brut)));
    let (d0, d1) = (&b.debut, &b.fin);
    let courte = if (d0.an, d0.mois) == (d1.an, d1.mois) {
        format!("du {} au {}/{:02}/{:02}", d0.jour, d1.jour, d1.mois, d1.an % 100)
    } else {
        format!("du {}/{:02} au {}/{:02}/{:02}", d0.jour, d0.mois, d1.jour, d1.mois, d1.an % 100)
    };
    let mut statut = b.statut.clone();
    if let Some(p) = statut.get(..1) {
        statut = format!("{}{}", p.to_uppercase(), &statut[1..]);
    }
    let p = B {
        id: identite(d, s)?,
        periode: format!("du {} au {}", dmy_court(d0), dmy_court(d1)),
        periode_courte: courte,
        reglement: format!("{} par virement", dmy_court(&b.regle_le)),
        objet: b.objet.as_ref().map(|o| o.0.clone()).unwrap_or_default(),
        analytique: b.analytique.clone(),
        emploi: b.emploi.clone(),
        statut,
        lignes,
        brut: ("MONTANT BRUT".into(), fr(c.brut)),
        cotisations,
        total: ("TOTAL COTISATIONS ET CONTRIB. OBLIGATOIRES".into(), fr(c.total_salarie), fr(c.total_employeur)),
        apres_total: Vec::new(),
        non_soumis,
        net_social: Some(fr(c.net_social)),
        net_avant_impot: fr(c.net_avant_impot),
        entete_pas: vec![(280.9, "base"), (339.0, "taux personnalisé"), (442.2, "montant"), (502.7, "cumul annuel")],
        pas: (
            "IMPOT SUR LE REVENU PRELEVE A LA SOURCE".into(),
            vec![(316.1, fr(c.net_imposable)), (412.2, fr(b.taux_pas)), (487.0, fr(c.pas)), (563.0, fr(c.annee.pas))],
        ),
        net_imposable: Some((fr(c.net_imposable), fr(c.annee.net_imposable))),
        cumuls,
        net_paye: fr(c.net_a_payer),
        allegement: false,
        total_verse: fr(c.cout),
    };
    Ok(page_b(&p))
}

// --------------------------------------------------------------------------------------------- la fabrique

/// Fabrique les bulletins de la description dans `dossier` (cree au besoin), un PDF par bulletin ; rend une ligne par
/// fichier ecrit. Rien n'est ecrit si un bulletin se refuse.
pub fn fabriquer(d: &Value, gabarit: Option<&str>, dossier: &Path) -> Resultat<Vec<String>> {
    let desc = lire_description(d, gabarit, dossier)?;
    let mut ordre: Vec<usize> = (0..desc.bulletins.len()).collect();
    ordre.sort_by_key(|&k| (desc.bulletins[k].fin, desc.bulletins[k].debut, k));
    let mut cumuls: HashMap<(usize, i32), Cumuls> = HashMap::new();
    let mut numeros: HashMap<i32, usize> = HashMap::new();
    let mut pages = Vec::new();
    for k in ordre {
        let b = &desc.bulletins[k];
        let cle = (b.salarie, b.fin.an);
        let avant = cumuls.get(&cle).cloned().unwrap_or_default();
        let c = calculer(b, &avant)?;
        cumuls.insert(cle, c.annee.clone());
        let n = numeros.entry(b.fin.an).or_insert(0);
        *n += 1;
        let numero = b.numero.clone().unwrap_or_else(|| format!("{}/{}", n, b.fin.an));
        let page = if desc.gabarit == "coolpaie" { page_coolpaie(&desc, b, &c, numero)? } else { page_spaiectacle(&desc, b, &c)? };
        let s = &desc.salaries[b.salarie];
        let nom = format!("{}-{:02}-{}-{:03}.pdf", b.fin.an, b.fin.mois, s.code, b.rang);
        let ligne = format!(
            "{}  {} {}  du {} au {}  brut {}  net à payer {}  ({} cachet(s), {} h, {} jour(s))",
            nom,
            s.prenom,
            s.nom,
            b.debut.iso(),
            b.fin.iso(),
            fr(c.brut),
            fr(c.net_a_payer),
            quantite(c.cachets),
            quantite(c.heures),
            quantite(c.jours)
        );
        pages.push((nom, pdf_octets(vec![page])?, ligne));
    }
    std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {}", dossier.display(), e))?;
    let mut faits = Vec::new();
    for (nom, octets, ligne) in pages {
        let chemin = dossier.join(&nom);
        std::fs::write(&chemin, octets).map_err(|e| format!("{} : {}", chemin.display(), e))?;
        faits.push(ligne);
    }
    Ok(faits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// La description de la notice du banc (`atelier/outils/banc/LISEZ-MOI.md`) : identite inventee, Siret de la famille
    /// `123456…` a cle de Luhn juste, NIR de la commune 999 a cle juste.
    fn description() -> Value {
        json!({
            "entreprise": {
                "raison_sociale": "La Fanfare des Toits", "adresse": "7 rue des Gouttières", "code_postal": "44000",
                "commune": "Nantes", "siret": "12345678200010", "naf": "9001Z", "urssaf": "527000000001",
                "licence": "PLATESV-R-2020-000123",
                "convention": {"idcc": "3252", "libelle": "Spectacle vivant, création et événement"}
            },
            "salaries": [
                {"code": "1", "nom": "Scaramouche", "prenom": "Tiberio", "nir": "185079999912388", "naissance": "1985-07-14",
                 "adresse": "3 quai de la Fosse", "code_postal": "44000", "commune": "Nantes", "conges_spectacles": "Z1234567"},
                {"code": "2", "nom": "Colombine", "prenom": "Isabella", "nir": "290039999900145", "naissance": "1990-03-02",
                 "adresse": "12 rue Kervégan", "code_postal": "44000", "commune": "Nantes"}
            ],
            "bulletins": [
                {"salarie": "1", "emploi": "MUSICIEN", "statut": "artiste", "debut": "2025-03-10", "fin": "2025-03-14",
                 "dates": [["2025-03-10", "2025-03-11"], ["2025-03-14", "2025-03-14"]],
                 "objet": {"numero": "256Z00920031", "titre": "Les Toits en Fanfare"},
                 "lignes": [{"libelle": "Cachets isolés", "quantite": 3, "taux": 146}], "taux_pas": 2.5},
                {"salarie": "2", "emploi": "CHARGEE DE DIFFUSION", "statut": "non cadre", "regime": "permanent",
                 "debut": "2025-03-01", "fin": "2025-03-31",
                 "lignes": [{"libelle": "Heures", "quantite": 86.67, "taux": 14.5}], "taux_pas": "1.4"},
                {"salarie": "1", "emploi": "MUSICIEN", "statut": "artiste", "debut": "2025-04-07", "fin": "2025-04-09",
                 "objet": {"numero": "256Z00920031", "titre": "Les Toits en Fanfare"},
                 "lignes": [{"libelle": "Service de répétition 4 heures", "quantite": 2, "taux": 62},
                            {"libelle": "Cachets isolés", "quantite": 1, "taux": 146}]}
            ]
        })
    }

    fn lire_tout(dossier: &Path) -> Vec<crate::import::bulletin_lu::BulletinLu> {
        let mut fichiers: Vec<_> = std::fs::read_dir(dossier).unwrap().map(|e| e.unwrap().path()).collect();
        fichiers.sort();
        fichiers
            .iter()
            .map(|f| {
                let mut l = crate::import::bulletin_lu::lire(f).unwrap();
                assert_eq!(l.len(), 1);
                match l.remove(0).resultat {
                    Ok(b) => b,
                    Err(r) => panic!("{} : {}", f.display(), r.message()),
                }
            })
            .collect()
    }

    fn dossier(nom: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("paie-fabriquer-bulletins-{}-{}", std::process::id(), nom));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// Chaque bulletin fabrique se relit par le lecteur de l'import, sans avertissement (les totaux retombent, chaque
    /// libelle a sa rubrique, les cumuls s'accordent aux lignes), et dit ce que la description a donne.
    #[test]
    fn les_bulletins_fabriques_se_relisent_sans_ecart() {
        for gabarit in ["coolpaie", "spaiectacle"] {
            let d = dossier(gabarit);
            let mut desc = description();
            if gabarit == "coolpaie" {
                desc["bulletins"][2]["lignes"][0]["libelle"] = json!("Service(s) répét. 4h (contrat<5j)");
            } else {
                desc["bulletins"][0]["non_soumis"] = json!([{"libelle": "Défraiement non soumis", "quantite": 2, "taux": 18.5}]);
            }
            let faits = fabriquer(&desc, Some(gabarit), &d).unwrap();
            assert_eq!(faits.len(), 3);
            let lus = lire_tout(&d);
            assert_eq!(lus.len(), 3);
            for b in &lus {
                assert_eq!(b.logiciel.as_deref(), Some(gabarit));
                assert!(b.avertissements.is_empty(), "{} : {:?}", gabarit, b.avertissements);
                assert_eq!(b.entreprise.siret.as_deref().map(|s| s.replace(' ', "")).as_deref(), Some("12345678200010"));
                let c = crate::import::bulletin_lu::compteurs(b, &d).unwrap();
                assert!(c.avertissements.is_empty(), "{} : {:?}", gabarit, c.avertissements);
                assert!(c.cachets.is_some() && c.heures.is_some() && c.jours.is_some(), "{} : {:?}", gabarit, c);
            }
            // Les fichiers sont dans l'ordre du temps : mars (1, puis 2), avril.
            let mars = &lus[0];
            assert_eq!(mars.salarie.nir.as_deref().map(|s| s.replace(' ', "")).as_deref(), Some("185079999912388"));
            assert_eq!(mars.brut, Decimal::new(43800, 2));
            assert_eq!(mars.numero_objet.as_deref(), Some("256Z00920031"));
            let c = crate::import::bulletin_lu::compteurs(mars, &d).unwrap();
            assert_eq!((c.cachets, c.jours), (Some(Decimal::from(3)), Some(Decimal::from(3))));
            let avril = &lus[2];
            let c = crate::import::bulletin_lu::compteurs(avril, &d).unwrap();
            assert_eq!((c.cachets, c.heures), (Some(Decimal::ONE), Some(Decimal::from(8))));
            // Le cumul de l'annee suit le salarie : brut de mars + brut d'avril.
            let brut = avril.cumuls.iter().find(|(n, _)| n == "brut").map(|(_, c)| c.cumul).unwrap();
            assert_eq!(brut, Some(Decimal::new(43800 + 27000, 2)));
            let _ = std::fs::remove_dir_all(&d);
        }
    }

    /// LE PLAFOND IMPRIME VIENT D'UN TEXTE (#236, regle 1) : un mois entier d'un permanent porte le PSS mensuel de son
    /// annee -- 2025 du bareme livre (3 925), 2024 de la table du BOSS deposee (3 864, ligne PSS_M_METROP au
    /// 01/01/2024 de `atelier/reference/boss-table-parametres-2026.csv`). Une annee qu'aucun texte depose ne porte
    /// (2019) se refuse, et rien n'est ecrit ; `plafonds` ne se donne plus a la main.
    #[test]
    fn le_plafond_imprime_vient_du_bareme_ou_de_la_table_du_boss() {
        let permanent = |an: i32| {
            let mut desc = description();
            let mut b = desc["bulletins"][1].clone();
            b["debut"] = json!(format!("{}-03-01", an));
            b["fin"] = json!(format!("{}-03-31", an));
            desc["bulletins"] = json!([b]);
            desc
        };
        for (an, pss) in [(2025, 3925), (2024, 3864)] {
            let d = dossier(&format!("plafond-{}", an));
            fabriquer(&permanent(an), Some("coolpaie"), &d).unwrap();
            let lu = lire_tout(&d).remove(0);
            let plafond = lu.cumuls.iter().find(|(n, _)| n == "plafond").map(|(_, c)| c.periode).unwrap();
            assert_eq!(plafond, Some(Decimal::from(pss)), "{}", an);
            let _ = std::fs::remove_dir_all(&d);
        }
        let d = dossier("plafond-2019");
        assert!(fabriquer(&permanent(2019), Some("coolpaie"), &d).is_err());
        assert!(!d.exists());
        let mut desc = permanent(2025);
        desc["plafonds"] = json!({"2025": {"mensuel": 1, "journalier": 1}});
        assert!(fabriquer(&desc, Some("coolpaie"), &d).is_err());
    }

    /// Ce que l'import ne reprendrait pas entier se refuse, et rien n'est ecrit.
    #[test]
    fn une_description_que_l_import_ne_reprendrait_pas_se_refuse() {
        let d = dossier("refus");
        let cas: Vec<(&str, Value)> = vec![
            ("/bulletins/0/lignes/0/libelle", json!("Cachets de répétition")),
            ("/bulletins/0/lignes/0/libelle", json!("Salaire de base")),
            ("/salaries/0/nir", json!("185079999912389")),
            ("/entreprise/siret", json!("12345678200011")),
            ("/bulletins/0/dates", json!([["2025-03-09", "2025-03-11"]])),
        ];
        for (chemin, valeur) in cas {
            let mut desc = description();
            *desc.pointer_mut(chemin).unwrap() = valeur;
            assert!(fabriquer(&desc, Some("coolpaie"), &d).is_err(), "{}", chemin);
        }
        let mut desc = description();
        desc["bulletins"][0]["non_soumis"] = json!([{"libelle": "Défraiement non soumis", "quantite": 2, "taux": 18.5}]);
        assert!(fabriquer(&desc, Some("coolpaie"), &d).is_err());
        assert!(fabriquer(&description(), None, &d).is_err(), "sans gabarit");
        assert!(!d.exists());
    }
}
