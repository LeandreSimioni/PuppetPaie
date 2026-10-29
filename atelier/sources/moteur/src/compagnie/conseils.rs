//! LES CONSEILS « Le saviez-vous ? ». Une option de la compagnie (fiche Entreprise, `conseils`, decochee
//! par defaut : sans elle, rien ne s'affiche nulle part) ouvre trois familles, qui parlent a l'employeur :
//!
//! - le conseil du demarrage (`conseil-du-jour`) : un conseil general de la banque (`quand` = « toujours »), a
//!   l'accueil. Il tourne sans rien ecrire : le rang du jour est `jour x pas mod N`, ou `pas` est premier avec `N` (le
//!   nombre de conseils affichables) ; N jours de suite parcourent donc toute la banque avant de repasser par le meme,
//!   et « un autre » (`decalage`) avance d'un cran a partir du conseil du jour. Rien a garder, rien a synchroniser entre
//!   postes, et le meme conseil pour tout le monde le meme jour ;
//! - les conseils de saisie (`de_saisie`) : sous la carte d'engagement de l'apercu, au plus un par carte, quand un fait
//!   de la saisie l'appelle (`saisie:<condition>`, dans l'ordre de `ordre_saisie`). Ils ne vivent QUE dans la route
//!   `apercu` (champ `conseil` de la vue d'une paie) : ni dans les avertissements de la paie, ni sur le bulletin, ni
//!   dans `produire`. Une phrase courte (`texte`), et le detail (`detail` : les deux faces, les chiffres) que l'ecran
//!   deplie derriere « pourquoi ? » avec la reference ;
//! - les conseils sur l'historique (`conseils-salarie`, `conseils-resume`) : ce que la compagnie a reellement fait pour
//!   un salarie, sur la fenetre de ses droits (depuis la date anniversaire que porte sa fiche, sinon 12 mois glissants).
//!
//! Le texte, ses chiffres dates et sa source vivent dans `donnees/conseils.json` : un conseil s'ajoute sans toucher au
//! moteur. Ici, seulement les conditions et les valeurs qu'elles fournissent. Ce qui est sur se chiffre : les heures
//! comptees chez nous, leur part des 507, ce que les conseils auraient change en heures et en jours d'allocation (annexe
//! X ou VIII, art. 30). JAMAIS d'euros d'allocation (Leandre) : on ne connait pas les autres employeurs. Le seul cas de
//! certitude : nos contrats font a eux seuls au moins 507 heures sur la fenetre ; alors ses droits sont ouverts, et le
//! conseil « plus d'heures » cede la place a « intense plutot que dilue ».
//!
//! Le salarie se nomme par son seul prenom (`{prenom}`), jamais par un pronom qui le genre.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::cumul::{self, Constantes, Declare, Fenetre};
use socle::date::{en_texte, Jour};
use droit::emplois;
use socle::fichiers::{dossier_du_code, json_en_cache, vrai};
use droit::minima::Par;

use socle::num::{div, Decimal, Resultat};
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

/// Les routes de lecture des conseils.
pub const ROUTES: [&str; 3] = ["conseil-du-jour", "conseils-salarie", "conseils-resume"];

/// Les conditions que le moteur sait evaluer : un conseil de la banque dont le `quand` n'est ni « toujours » ni l'une
/// d'elles ne s'afficherait jamais (le test de la banque le refuse).
pub const CONDITIONS: [&str; 16] = [
    "saisie:technicien-plus-de-208-heures",
    "saisie:plus-de-28-cachets",
    "saisie:mois-sature",
    "saisie:507-assurees",
    "saisie:repetition-en-cachet",
    "saisie:heures-pour-la-meme-somme",
    "saisie:enseignement",
    "saisie:suspension",
    "saisie:cachets-et-heures",
    "historique:droits-ouverts",
    "historique:heures-comptees",
    "historique:cachets-perdus",
    "historique:gain-des-conseils",
    "historique:intense",
    "historique:enseignement-perdu",
    "historique:resume",
];

/// Les conditions qui ne parlent que d'une annexe : (condition, annexe). Le cachet est propre a l'annexe X (art. 3 §1 :
/// « Lorsque l'activité des artistes est déclarée sous la forme de cachets » ; l'annexe VIII ne le connait pas) ; les
/// 208 heures retenues par mois, a l'annexe VIII. Les autres valent pour les deux annexes : leur conseil cite chaque
/// article dans l'une et l'autre (le test de la banque le verifie), et le moteur n'en montre que celles en jeu.
pub const D_UNE_ANNEXE: [(&str, &str); 5] = [
    ("saisie:technicien-plus-de-208-heures", "VIII"),
    ("saisie:plus-de-28-cachets", "X"),
    ("saisie:repetition-en-cachet", "X"),
    ("saisie:cachets-et-heures", "X"),
    ("historique:cachets-perdus", "X"),
];

/// Les deux annexes : X (artistes), VIII (techniciens).
const LES_DEUX: [&str; 2] = ["X", "VIII"];

/// Les annexes dont parle un conseil sur ce salarie : la sienne, lue de son metier ; les deux quand son metier ne la
/// dit pas (rien ne se devine : la regle citee vaut alors pour l'une ou l'autre).
fn annexes_en_jeu(annexe: Option<&'static str>) -> Vec<&'static str> {
    annexe.map_or(LES_DEUX.to_vec(), |a| vec![a])
}

/// L'option de la fiche Entreprise : absente vaut NON.
pub fn actifs(entreprise: &Value) -> bool {
    entreprise.get("conseils") == Some(&Value::Bool(true))
}

/// La banque, `donnees/conseils.json`.
pub fn banque() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("conseils.json"))
}

fn conseils_de(b: &Value) -> &[Value] {
    b.get("conseils").and_then(Value::as_array).map_or(&[][..], |a| &a[..])
}

fn texte(v: &Value, k: &str) -> String {
    v.get(k).map(en_texte).unwrap_or_default()
}

fn suspendu(c: &Value) -> bool {
    c.get("suspendu").map_or(false, vrai)
}

// ---------------------------------------------------------------- gabarits

/// Une valeur que fournit une condition : son texte, et le nombre qu'elle dit (pour accorder le mot qui suit).
#[derive(Debug, Clone)]
pub struct Valeur {
    pub texte: String,
    pub nombre: Option<Decimal>,
}

pub type Valeurs = BTreeMap<String, Valeur>;

fn val_texte(t: impl Into<String>) -> Valeur {
    Valeur { texte: t.into(), nombre: None }
}

fn val_nombre(n: Decimal) -> Valeur {
    Valeur { texte: nombre_fr(n, None), nombre: Some(n) }
}

fn val_entier(n: i64) -> Valeur {
    val_nombre(Decimal::from(n))
}

/// Un nombre en francais : milliers separes par une espace insecable, virgule decimale ; `decimales` fixe, sinon les
/// zeros de queue tombent (1,3 ; 4 005 ; 15,5).
pub fn nombre_fr(x: Decimal, decimales: Option<u32>) -> String {
    let x = match decimales {
        Some(d) => x.round_dp_with_strategy(d, rust_decimal::RoundingStrategy::MidpointAwayFromZero),
        None => x.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero).normalize(),
    };
    let s = match decimales {
        Some(d) => format!("{:.*}", d as usize, x),
        None => x.to_string(),
    };
    let (signe, s) = if let Some(r) = s.strip_prefix('-') { ("-", r.to_string()) } else { ("", s) };
    let (ent, fra) = match s.split_once('.') {
        Some((e, f)) => (e.to_string(), Some(f.to_string())),
        None => (s.clone(), None),
    };
    let mut groupes = Vec::new();
    let chiffres: Vec<char> = ent.chars().collect();
    let mut fin = chiffres.len();
    while fin > 3 {
        groupes.push(chiffres[fin - 3..fin].iter().collect::<String>());
        fin -= 3;
    }
    groupes.push(chiffres[..fin].iter().collect::<String>());
    groupes.reverse();
    let ent = groupes.join("\u{a0}");
    match fra {
        Some(f) if !f.is_empty() => format!("{}{},{}", signe, ent, f),
        _ => format!("{}{}", signe, ent),
    }
}

/// Une valeur des donnees livrees : `chemin` separe par « / », « id=x » choisit dans une liste, un entier indexe.
fn suivre<'a>(racine: &'a Value, chemin: &str) -> Option<&'a Value> {
    let mut v = racine;
    for seg in chemin.split('/') {
        v = if let Some(id) = seg.strip_prefix("id=") {
            v.as_array()?.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id))?
        } else if let (Some(a), Ok(i)) = (v.as_array(), seg.parse::<usize>()) {
            a.get(i)?
        } else {
            v.get(seg)?
        };
    }
    Some(v)
}

/// `{bareme:chemin*n/m,2}` : la valeur lue au chemin, puis les operations (elles commencent au premier « * » : un
/// « / » avant lui separe le chemin), avec ses decimales apres la virgule finale.
fn donnee(source: &Value, expr: &str) -> Option<String> {
    let (expr, decimales) = match expr.rsplit_once(',') {
        Some((e, d)) if !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()) => (e, d.parse().ok()),
        _ => (expr, None),
    };
    let (chemin, ops) = expr.split_at(expr.find('*').unwrap_or(expr.len()));
    let mut x = socle::num::dec(suivre(source, chemin)?).ok()?;
    let mut reste = ops;
    while let Some(op) = reste.chars().next() {
        let suite = &reste[1..];
        let n_fin = suite.find(['*', '/']).unwrap_or(suite.len());
        let n: Decimal = suite[..n_fin].parse().ok()?;
        x = match op {
            '*' => x * n,
            '/' if !n.is_zero() => div(x, n),
            _ => return None,
        };
        reste = &suite[n_fin..];
    }
    Some(nombre_fr(x, decimales))
}

/// Les donnees que les gabarits lisent : le bareme de l'annee, les minima.
pub struct Donnees {
    pub annee: i32,
    bareme: Option<Arc<Value>>,
    minima: Option<Arc<Value>>,
}

impl Donnees {
    pub fn de_l_annee(code: &Path, annee: i32) -> Donnees {
        Donnees {
            annee,
            bareme: json_en_cache(&code.join("baremes").join(format!("{}.json", annee))).ok(),
            minima: droit::minima::table().ok(),
        }
    }
}

/// Le texte d'un conseil, gabarits resolus ; `None` quand l'un d'eux ne se resout pas (le conseil se tait).
pub fn rendre(gabarit: &str, d: &Donnees, vals: &Valeurs) -> Option<String> {
    let mut out = String::new();
    let mut reste = gabarit;
    while let Some(i) = reste.find('{') {
        out.push_str(&reste[..i]);
        let apres = &reste[i + 1..];
        let j = apres.find('}')?;
        let dedans = &apres[..j];
        out.push_str(&resoudre(dedans, d, vals)?);
        reste = &apres[j + 1..];
    }
    out.push_str(reste);
    Some(out)
}

fn resoudre(dedans: &str, d: &Donnees, vals: &Valeurs) -> Option<String> {
    if dedans == "annee" {
        return d.bareme.as_ref().map(|_| d.annee.to_string());
    }
    if let Some(expr) = dedans.strip_prefix("bareme:") {
        return donnee(d.bareme.as_deref()?, expr);
    }
    if let Some(expr) = dedans.strip_prefix("minima:") {
        return donnee(d.minima.as_deref()?, expr);
    }
    // {var|singulier|pluriel} : le nombre et le mot ; {var?singulier|pluriel} : le mot seul.
    let (nom, mots, avec_nombre) = match (dedans.find('|'), dedans.find('?')) {
        (_, Some(q)) => (&dedans[..q], Some(&dedans[q + 1..]), false),
        (Some(p), None) => (&dedans[..p], Some(&dedans[p + 1..]), true),
        (None, None) => (dedans, None, true),
    };
    let v = vals.get(nom)?;
    let Some(mots) = mots else { return Some(v.texte.clone()) };
    let (sing, plur) = mots.split_once('|')?;
    let pluriel = v.nombre.map_or(false, |n| n >= Decimal::from(2));
    let mot = if pluriel { plur } else { sing };
    Some(if avec_nombre { format!("{} {}", v.texte, mot) } else { mot.to_string() })
}

/// La reference lisible d'une source : le titre du document, l'article.
fn reference_de(b: &Value, s: &Value) -> String {
    let fichier = texte(s, "fichier");
    let cle = match s.get("code") {
        Some(c) => format!("{}#{}", fichier, en_texte(c)),
        None => fichier.clone(),
    };
    let titre = b.get("documents").and_then(|d| d.get(&cle)).map(en_texte).unwrap_or(fichier);
    let article = texte(s, "article");
    if article.is_empty() {
        titre
    } else if s.get("code").is_some() {
        format!("{}, art. {}", titre, article)
    } else {
        format!("{}, {}", titre, article)
    }
}

fn sources_de(c: &Value) -> Vec<&Value> {
    match c.get("source") {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(o @ Value::Object(_)) => vec![o],
        _ => vec![],
    }
}

/// Le conseil rendu pour l'ecran : son identifiant, son texte (la phrase courte d'un conseil de saisie), son detail
/// quand la banque en a un, sa reference (celle de chaque source) ; rien si l'un de ses gabarits ne se resout pas. Une
/// source tiree d'une annexe (`annexe` : « X » ou « VIII ») n'est citee que si cette annexe est en jeu (`annexes` :
/// celles des salaries dont le conseil parle) ; une source sans annexe l'est toujours.
fn vue(b: &Value, c: &Value, d: &Donnees, vals: &Valeurs, annexes: &[&str]) -> Option<Value> {
    let references: Vec<String> = sources_de(c)
        .iter()
        .filter(|s| s.get("annexe").and_then(Value::as_str).map_or(true, |a| annexes.contains(&a)))
        .map(|s| reference_de(b, s))
        .collect();
    let mut v = json!({
        "id": texte(c, "id"),
        "texte": rendre(&texte(c, "texte"), d, vals)?,
        "reference": references.join(" ; "),
    });
    if c.get("detail").is_some() {
        v["detail"] = Value::String(rendre(&texte(c, "detail"), d, vals)?);
    }
    Some(v)
}

/// Le premier conseil de la banque pour cette condition (`quand`), rendu avec ces valeurs : un conseil suspendu, ou
/// dont un gabarit ne se resout pas, cede la place au suivant ; rien s'il n'en reste aucun.
fn conseil(b: &Value, d: &Donnees, quand: &str, vals: &Valeurs, annexes: &[&str]) -> Option<Value> {
    conseils_de(b)
        .iter()
        .filter(|c| texte(c, "quand") == quand && !suspendu(c))
        .find_map(|c| vue(b, c, d, vals, annexes))
}

/// Le prenom du salarie, tel que sa fiche l'affiche (`prenom_affichage`, sinon `prenom`, en capitales initiales s'il
/// est tout en majuscules) ; sans prenom, son nom, sinon sa cle.
pub fn prenom(sal: &Value) -> String {
    let lu = |k: &str| sal.get(k).filter(|v| vrai(v)).map(en_texte).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(p) = lu("prenom_affichage") {
        return p;
    }
    if let Some(p) = lu("prenom").or_else(|| lu("nom_affichage")).or_else(|| lu("nom")) {
        if p.chars().any(char::is_lowercase) {
            return p;
        }
        // FANCHON -> Fanchon, JEAN-LOUP -> Jean-Loup.
        let mut out = String::with_capacity(p.len());
        let mut debut = true;
        for ch in p.chars() {
            if debut {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            debut = !ch.is_alphabetic();
        }
        return out;
    }
    texte(sal, "_cle")
}

// ------------------------------------------------------------ le demarrage

fn pgcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        pgcd(b, a % b)
    }
}

/// Le pas de la rotation : le premier entier a partir de N x 0,618 qui est premier avec N. Deux jours voisins ne
/// tombent pas sur deux conseils voisins de la banque (rangee par domaine), et N jours de suite les montrent tous.
fn pas(n: usize) -> usize {
    let mut s = ((n as f64) * 0.618).ceil().max(1.0) as usize;
    while pgcd(s, n) != 1 {
        s += 1;
    }
    s
}

/// Les conseils generaux affichables cette annee, dans l'ordre de la banque, deja rendus.
fn generaux(b: &Value, d: &Donnees) -> Vec<Value> {
    conseils_de(b)
        .iter()
        .filter(|c| texte(c, "quand") == "toujours" && !suspendu(c))
        .filter_map(|c| vue(b, c, d, &Valeurs::new(), &LES_DEUX))
        .collect()
}

/// `conseil-du-jour` : `{"conseil": {...}, "rang", "total"}`, ou `{"conseil": null}` quand l'option est decochee.
/// Charge : `decalage` (0 par defaut : « un autre » l'augmente d'un).
pub fn du_jour(ctx: &Contexte, decalage: i64) -> Resultat<Value> {
    if !actifs(&ctx.entreprise) {
        return Ok(json!({ "conseil": null }));
    }
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(&ctx.code, jour.an);
    let liste = generaux(&b, &d);
    let n = liste.len();
    if n == 0 {
        return Ok(json!({ "conseil": null }));
    }
    let s = pas(n) as i128;
    let rang = ((jour.ordinal() as i128) * s + decalage as i128).rem_euclid(n as i128) as usize;
    Ok(json!({ "conseil": liste[rang], "rang": rang, "total": n }))
}

// ------------------------------------------------------- les faits d'une paie

/// L'annexe du salarie de cette paie, lue du metier de sa fiche : X pour un artiste, VIII pour un technicien ; rien
/// sinon (rien ne se devine).
fn annexe_de(sal: &Value) -> Resultat<Option<&'static str>> {
    let Some(m) = emplois::metier(&texte(sal, "code_emploi_ft"))? else { return Ok(None) };
    Ok(match texte(&m, "statut").as_str() {
        "artiste" => Some("X"),
        "technicien" => Some("VIII"),
        _ => None,
    })
}

/// Les constantes de l'article 30 pour une annexe (`conseils.json`, `annexes`).
struct Art30 {
    heures_par_jour: Decimal,
    coefficient: Decimal,
    jours_sans_allocation: Decimal,
}

fn art30(b: &Value, annexe: &str) -> Option<Art30> {
    let a = b.get("annexes")?.get(annexe)?;
    let n = |k: &str| a.get(k).and_then(|v| socle::num::dec(v).ok());
    Some(Art30 { heures_par_jour: n("heures_par_jour")?, coefficient: n("coefficient")?, jours_sans_allocation: n("jours_sans_allocation")? })
}

/// Les jours d'allocation qu'un nombre d'heures retire un mois indemnise (art. 30 : heures / heures par jour x
/// coefficient), sans arrondi : une somme sur plusieurs paies s'arrondit une fois (`environ`).
fn jours_bruts(heures: Decimal, a: &Art30) -> Decimal {
    div(heures, a.heures_par_jour) * a.coefficient
}

/// « environ » : au demi-jour le plus proche (Leandre, 21/10/2026 : 8 heures en annexe X, 1,04 jour, font « environ
/// 1 jour »). Zero quand le cout est de moins d'un quart de jour : la condition ne fournit alors pas `jours_perdus`, et
/// la variante « -minime » de la banque dit « moins d'un demi-jour » au lieu d'une perte.
fn environ(jours: Decimal) -> Decimal {
    (jours * Decimal::from(2)).round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero) / Decimal::from(2)
}

/// `jours_perdus`, seulement quand le cout arrondi n'est pas nul.
fn avec_jours(v: &mut Vec<(&str, Valeur)>, jours: Decimal) {
    let j = environ(jours);
    if j > Decimal::ZERO {
        let t = if j == Decimal::new(5, 1) { "½".to_string() } else { nombre_fr(j, None) };
        v.push(("jours_perdus", Valeur { texte: t, nombre: Some(j) }));
    }
}

/// Ce qu'une paie gagnerait en heures a suivre le conseil « cachet de repetition » : (heures avant, gain, jours,
/// somme), ou rien. La convention 3090 fixe un « cachet de repetition pour 1 ou 2 services de repetition dans la meme
/// journee » (`minima::cachet_de_repetition`) ; un cachet compte 12 heures (annexe X, art. 3). Seulement les lignes
/// « Service de repetition » a dates connues, de 1 ou 2 services par jour, de moins de 12 heures par jour, dont la
/// somme par jour couvre le cachet de repetition : sous le minimum, rien.
struct GainRepetition {
    heures_avant: Decimal,
    gain: Decimal,
    jours: i64,
    montant: Decimal,
    minimum: droit::minima::Minimum,
}

fn gain_repetition(ctx: &Contexte, p: &Paie, heures_cachet: Decimal) -> Resultat<Option<GainRepetition>> {
    let Some(min) = p.cachet_de_repetition(ctx)? else { return Ok(None) };
    let saisies = p.saisies();
    let (mut heures_avant, mut gain, mut jours, mut montant) = (Decimal::ZERO, Decimal::ZERO, 0i64, Decimal::ZERO);
    for (l, s) in p.remuneration.iter().zip(saisies) {
        if l.code != "repetition" || l.heures_annexes.is_zero() {
            continue;
        }
        let dates: BTreeSet<String> = s.get("dates").and_then(Value::as_array).into_iter().flatten().map(|j| match j {
            Value::Object(m) => m.get("jour").map(en_texte).unwrap_or_default(),
            autre => en_texte(autre),
        }).filter(|j| !j.is_empty()).collect();
        if dates.is_empty() {
            continue;
        }
        let n = Decimal::from(dates.len() as i64);
        let services_par_jour = div(l.quantite, n);
        let heures_jour = div(l.heures_annexes, n);
        let montant_jour = div(l.montant, n);
        if services_par_jour > Decimal::from(2) || heures_jour >= heures_cachet || montant_jour < min.montant {
            continue;
        }
        heures_avant += l.heures_annexes;
        gain += (heures_cachet - heures_jour) * n;
        jours += dates.len() as i64;
        montant += l.montant;
    }
    if gain <= Decimal::ZERO {
        return Ok(None);
    }
    Ok(Some(GainRepetition { heures_avant, gain, jours, montant, minimum: min }))
}

/// Ce qu'une paie gagnerait en heures a couvrir, pour la meme somme, autant d'heures que le minimum horaire de la
/// convention le permet (`minima::minimum_de_la_ligne`, un minimum a l'heure) : la ligne au plus grand gain, ou rien.
/// Sous le minimum, rien.
///
/// LE SEUIL : un gain d'au moins `seuil` heures, le poids d'un cachet (12 heures, annexe X art. 3 §1 ; Leandre,
/// 21/10/2026 : « seulement si l'ecart est important »). Pourquoi lui : les deux simulations du ticket 224 (le modele
/// mensuel des annexes VIII et X, rejoue sur les exemples chiffres du guide France Travail) montrent que des heures de
/// plus pour la meme somme ne valent que pres du seuil des 507 heures, ou elles peuvent valoir d'un mois a une annee de
/// droits ; ailleurs, un mois indemnise, chaque heure coute 0,13 jour d'allocation (X ; 0,175 en VIII) et rapporte
/// moins l'annee suivante (2 heures de plus : de 0 a -60 EUR sur 24 mois ; dix fois 2 heures : -60 a -200 EUR). Le conseil ne
/// gagne que si le salarie est a moins de `gain` heures des 507 : sa chance de servir croit avec le gain, et sous le
/// poids d'un cachet elle est trop mince pour une ligne sous chaque carte.
struct GainHeures {
    heures: Decimal,
    montant: Decimal,
    minimum: Decimal,
    heures_max: Decimal,
    gain: Decimal,
}

fn gain_heures(ctx: &Contexte, p: &Paie, seuil: Decimal) -> Resultat<Option<GainHeures>> {
    let mut meilleur: Option<GainHeures> = None;
    for (i, l) in p.remuneration.iter().enumerate() {
        if l.heures_annexes <= Decimal::ZERO || l.montant <= Decimal::ZERO {
            continue;
        }
        let Some(m) = p.minimum_de_la_ligne(ctx, i)? else { continue };
        if m.par != Par::Heure || m.montant <= Decimal::ZERO {
            continue;
        }
        let heures_max = div(l.montant, m.montant).floor();
        let gain = (heures_max - l.heures_annexes).floor();
        if gain < seuil.max(Decimal::ONE) {
            continue;
        }
        if meilleur.as_ref().map_or(true, |g| gain > g.gain) {
            meilleur = Some(GainHeures { heures: l.heures_annexes, montant: l.montant, minimum: m.montant, heures_max, gain });
        }
    }
    Ok(meilleur)
}

/// Les engagements que nos fichiers declarent, ceux du mois en cours remplaces par les paies de l'ecran ; lus une
/// fois par apercu, et seulement si une condition en a besoin.
struct Declares<'a> {
    ctx: &'a Contexte,
    paies: &'a [Paie],
    lus: Option<Vec<Declare>>,
}

impl<'a> Declares<'a> {
    fn tous(&mut self) -> Resultat<&[Declare]> {
        if self.lus.is_none() {
            self.lus = Some(engagement::cumul::lire_engagements(self.ctx, Some(self.paies))?);
        }
        Ok(self.lus.as_deref().unwrap_or(&[]))
    }
}

/// La phrase de la fenetre (`conseils.json`, `fenetres`).
fn phrase_fenetre(b: &Value, f: &cumul::Position, anniversaire: bool) -> String {
    let g = b.get("fenetres").and_then(|x| x.get(if anniversaire { "anniversaire" } else { "glissante" })).map(en_texte).unwrap_or_default();
    g.replace("{debut}", &f.debut.jj_mm_aaaa()).replace("{fin}", &f.fin.jj_mm_aaaa())
}

/// La position du salarie sur la fenetre de ses droits : depuis sa date anniversaire si sa fiche la porte, sinon les
/// 12 mois jusqu'a `a_la_date`.
fn position(ctx: &Contexte, cle: &str, engs: &[Declare], a_la_date: Jour) -> Resultat<(cumul::Position, bool)> {
    let anniv = cumul::anniversaire_configure(ctx, cle)?;
    let p = cumul::position_droits(ctx, cle, engs, anniv, Some(a_la_date))?;
    Ok((p, anniv.is_some()))
}

// ------------------------------------------------------------- la saisie

/// LES CONSEILS DE SAISIE : pour chaque paie de l'apercu, le premier conseil (dans l'ordre `ordre_saisie`) qu'un fait
/// de la saisie appelle, ou rien. Rien du tout quand l'option est decochee. Un permanent (paye au mois), un mois sans
/// paie, une paie dont le metier ne dit pas l'annexe : rien.
pub fn de_saisie(ctx: &Contexte, paies: &[Paie]) -> Resultat<Vec<Option<Value>>> {
    let mut out = vec![None; paies.len()];
    if !actifs(&ctx.entreprise) {
        return Ok(out);
    }
    let b = banque()?;
    let d = Donnees::de_l_annee(&ctx.code, ctx.aujourdhui.an);
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let ordre: Vec<String> = b.get("ordre_saisie").and_then(Value::as_array).map(|a| a.iter().map(en_texte).collect()).unwrap_or_default();
    // Le mois de chaque salarie, chez nous : ses heures (cachets a 12 h) et ses cachets, toutes ses paies de l'ecran.
    let mut du_mois: HashMap<String, (i64, i64)> = HashMap::new();
    for p in paies {
        let e = &p.engagement;
        if e.sans_paie() || e.nat()?.mensuelle() {
            continue;
        }
        let x = du_mois.entry(texte(&p.salarie, "_cle")).or_insert((0, 0));
        x.0 += e.heures.unwrap_or(0.0).round_ties_even() as i64 + e.cachets * c.heures_par_cachet;
        x.1 += e.cachets;
    }
    let mut declares = Declares { ctx, paies, lus: None };
    let mut assurees: HashMap<String, Option<(i64, String)>> = HashMap::new();
    for (i, p) in paies.iter().enumerate() {
        let e = &p.engagement;
        if e.sans_paie() || e.nat()?.mensuelle() {
            continue;
        }
        let Some(annexe) = annexe_de(&p.salarie)? else { continue };
        let Some(a30) = art30(&b, annexe) else { continue };
        let cle = texte(&p.salarie, "_cle");
        let (heures_mois, cachets_mois) = du_mois.get(&cle).copied().unwrap_or((0, 0));
        let qui = prenom(&p.salarie);
        let mois = socle::date::libelle_du_mois_ou_tel_quel(&e.mois, socle::date::Graphie::Accentuee);
        let base = |extra: Vec<(&str, Valeur)>| -> Valeurs {
            let mut v = Valeurs::new();
            v.insert("prenom".into(), val_texte(qui.clone()));
            v.insert("mois".into(), val_texte(mois.clone()));
            v.insert("heures_cachet".into(), val_entier(c.heures_par_cachet));
            for (k, x) in extra {
                v.insert(k.into(), x);
            }
            v
        };
        let seuil_mois = a30.jours_sans_allocation * a30.heures_par_jour;
        let sature = Decimal::from(heures_mois) >= seuil_mois;
        // Les deux conseils « plus d'heures », et ce qui les remplace quand nos contrats assurent les 507 heures.
        let rep = if annexe == "X" { gain_repetition(ctx, p, Decimal::from(c.heures_par_cachet))? } else { None };
        let hrs = gain_heures(ctx, p, Decimal::from(c.heures_par_cachet))?;
        let assure = if (rep.is_some() || hrs.is_some()) && !sature {
            if !assurees.contains_key(&cle) {
                let fin = e.fin_periode()?;
                let (pos, anniv) = position(ctx, &cle, declares.tous()?, fin)?;
                let h = pos.heures_ouverture()?;
                let r = (h >= c.seuil_heures).then(|| (h, phrase_fenetre(&b, &pos, anniv)));
                assurees.insert(cle.clone(), r);
            }
            assurees.get(&cle).cloned().flatten()
        } else {
            None
        };
        for id in &ordre {
            let vals: Option<Valeurs> = match id.as_str() {
                "saisie:technicien-plus-de-208-heures" => {
                    let a = b.get("annexes").and_then(|x| x.get("VIII"));
                    let plafond = a.and_then(|x| x.get("heures_retenues_par_mois")).and_then(Value::as_i64);
                    let majore = a.and_then(|x| x.get("heures_retenues_plusieurs_employeurs")).and_then(Value::as_i64);
                    match (annexe, plafond, majore) {
                        ("VIII", Some(pl), Some(mj)) if heures_mois > pl => Some(base(vec![
                            ("heures", val_entier(heures_mois)),
                            ("plafond", val_entier(pl)),
                            ("plafond_majore", val_entier(mj)),
                        ])),
                        _ => None,
                    }
                }
                "saisie:mois-sature" if sature => Some(base(vec![
                    ("heures", val_entier(heures_mois)),
                    ("seuil", val_nombre(seuil_mois)),
                    ("jours", val_nombre(a30.jours_sans_allocation)),
                    ("heures_par_jour", val_nombre(a30.heures_par_jour)),
                ])),
                "saisie:plus-de-28-cachets" if annexe == "X" && cachets_mois > c.cachets_par_mois => Some(base(vec![
                    ("cachets", val_entier(cachets_mois)),
                    ("plafond", val_entier(c.cachets_par_mois)),
                    ("perdus", val_entier(cachets_mois - c.cachets_par_mois)),
                ])),
                "saisie:507-assurees" => assure.as_ref().map(|(h, fenetre)| {
                    base(vec![
                        ("heures", val_entier(*h)),
                        ("fenetre", val_texte(fenetre.clone())),
                        ("heures_par_jour", val_nombre(a30.heures_par_jour)),
                        ("coefficient", val_nombre(a30.coefficient)),
                    ])
                }),
                "saisie:repetition-en-cachet" if assure.is_none() && !sature => rep.as_ref().map(|g| {
                    let mut v = vec![
                        ("heures_avant", val_nombre(g.heures_avant)),
                        ("nb_jours", val_entier(g.jours)),
                        ("montant_jour", val_texte(nombre_fr(div(g.montant, Decimal::from(g.jours)), Some(2)))),
                        ("minimum", val_texte(nombre_fr(g.minimum.montant, Some(2)))),
                        ("gain", val_nombre(g.gain)),
                    ];
                    avec_jours(&mut v, jours_bruts(g.gain, &a30));
                    base(v)
                }),
                "saisie:heures-pour-la-meme-somme" if assure.is_none() && !sature => hrs.as_ref().map(|g| {
                    let mut v = vec![
                        ("heures", val_nombre(g.heures)),
                        ("montant", val_texte(nombre_fr(g.montant, Some(2)))),
                        ("taux", val_texte(nombre_fr(div(g.montant, g.heures), Some(2)))),
                        ("minimum", val_texte(nombre_fr(g.minimum, Some(2)))),
                        ("heures_max", val_nombre(g.heures_max)),
                        ("gain", val_nombre(g.gain)),
                    ];
                    avec_jours(&mut v, jours_bruts(g.gain, &a30));
                    base(v)
                }),
                "saisie:enseignement" => {
                    let mut trouve = None;
                    for l in &p.remuneration {
                        let r = droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au)?;
                        if r.compte_pour != "ouverture_des_droits" {
                            continue;
                        }
                        let pl = r.plafond_annuel_heures.clone().unwrap_or(Value::Null);
                        let bas = pl.get("moins_de_50_ans").and_then(Value::as_i64);
                        let haut = pl.get("50_ans_et_plus").and_then(Value::as_i64);
                        if let (Some(bas), Some(haut)) = (bas, haut) {
                            trouve = Some(base(vec![("plafond", val_entier(bas)), ("plafond_50", val_entier(haut))]));
                        }
                        break;
                    }
                    trouve
                }
                "saisie:suspension" if !e.suspensions.is_empty() => Some(base(vec![])),
                "saisie:cachets-et-heures" if annexe == "X" && e.cachets > 0 && e.heures.unwrap_or(0.0) > 0.0 => Some(base(vec![])),
                _ => None,
            };
            if let Some(v) = vals {
                if let Some(x) = conseil(&b, &d, id, &v, &[annexe]) {
                    out[i] = Some(x);
                    break;
                }
            }
        }
    }
    Ok(out)
}

// --------------------------------------------------------------- l'historique

/// Ce que les conseils de saisie auraient change aux paies de ce salarie sur la fenetre : (heures, jours d'allocation,
/// avant arrondi). Chaque mois saisi de la fenetre se recharge (`engagement::charger`) ; un mois qui ne se charge pas
/// est laisse de cote.
fn gains_sur_la_fenetre(ctx: &Contexte, cle: &str, f: &Fenetre, a30: &Art30, annexe: &str) -> Resultat<(Decimal, Decimal)> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let (debut, fin, _) = f;
    let mut gain = Decimal::ZERO;
    let mut jours = Decimal::ZERO;
    for mois in socle::date::mois_entre(debut, fin) {
        let Ok(saisie) = engagement::mois_pour_l_ecran(ctx, &mois) else { continue };
        for p in &saisie.paies {
            let e = &p.engagement;
            if texte(&p.salarie, "_cle") != cle || e.sans_paie() || e.nat()?.mensuelle() {
                continue;
            }
            let fp = e.fin_periode()?;
            if fp < *debut || fp > *fin {
                continue;
            }
            let g = match (annexe == "X").then(|| gain_repetition(ctx, p, Decimal::from(c.heures_par_cachet))).transpose()?.flatten() {
                Some(r) => r.gain,
                None => gain_heures(ctx, p, Decimal::from(c.heures_par_cachet))?.map_or(Decimal::ZERO, |h| h.gain),
            };
            if g > Decimal::ZERO {
                gain += g;
                jours += jours_bruts(g, a30);
            }
        }
    }
    Ok((gain, jours))
}

/// `conseils-salarie` : ce que la compagnie a fait pour ce salarie, sur la fenetre de ses droits. Charge : `salarie`.
pub fn du_salarie(ctx: &Contexte, cle: &str) -> Resultat<Vec<Value>> {
    if !actifs(&ctx.entreprise) {
        return Ok(Vec::new());
    }
    let Some(fiche) = ctx.salaries.get(cle) else { return Ok(Vec::new()) };
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(&ctx.code, jour.an);
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let engs = engagement::cumul::lire_engagements(ctx, None)?;
    let (pos, anniv) = position(ctx, cle, &engs, jour)?;
    if pos.engagements.is_empty() {
        return Ok(Vec::new());
    }
    let mut f = fiche.clone();
    if let Value::Object(m) = &mut f {
        m.insert("_cle".into(), Value::String(cle.into()));
    }
    let qui = prenom(&f);
    let fenetre = phrase_fenetre(&b, &pos, anniv);
    let annexe = annexe_de(&f)?;
    let a30 = annexe.and_then(|a| art30(&b, a));
    let heures = pos.heures_ouverture()?;
    let base = |extra: Vec<(&str, Valeur)>| -> Valeurs {
        let mut v = Valeurs::new();
        v.insert("prenom".into(), val_texte(qui.clone()));
        v.insert("fenetre".into(), val_texte(fenetre.clone()));
        v.insert("seuil".into(), val_entier(c.seuil_heures));
        for (k, x) in extra {
            v.insert(k.into(), x);
        }
        v
    };
    let en_jeu = annexes_en_jeu(annexe);
    let mut out = Vec::new();
    let mut pousser = |quand: &str, v: Valeurs| {
        if let Some(x) = conseil(&b, &d, quand, &v, &en_jeu) {
            out.push(x);
        }
    };
    if heures >= c.seuil_heures {
        pousser("historique:droits-ouverts", base(vec![("heures", val_entier(heures))]));
        if let Some(a) = &a30 {
            pousser(
                "historique:intense",
                base(vec![("heures_par_jour", val_nombre(a.heures_par_jour)), ("coefficient", val_nombre(a.coefficient))]),
            );
        }
    } else {
        let pct = (Decimal::from(heures * 100) / Decimal::from(c.seuil_heures)).round();
        pousser("historique:heures-comptees", base(vec![("heures", val_entier(heures)), ("pourcentage", val_nombre(pct))]));
        if let (Some(a), Some(ax)) = (&a30, annexe) {
            let f: Fenetre = (pos.debut, pos.fin, pos.libelle_fenetre.clone());
            let (gain, jours) = gains_sur_la_fenetre(ctx, cle, &f, a, ax)?;
            if gain > Decimal::ZERO {
                let pct = (gain * Decimal::from(100) / Decimal::from(c.seuil_heures)).round_dp(1).normalize();
                let mut v = vec![("gain", val_nombre(gain)), ("pourcentage", val_nombre(pct))];
                avec_jours(&mut v, jours);
                pousser("historique:gain-des-conseils", base(v));
            }
        }
    }
    // Le plafond de 28 cachets est de l'annexe X (art. 3 §1) : rien a dire d'un technicien.
    let par_mois = pos.cachets_par_mois();
    for m in pos.mois_plafonnes().into_iter().filter(|_| annexe != Some("VIII")) {
        let n = par_mois.get(&m).copied().unwrap_or(0);
        pousser(
            "historique:cachets-perdus",
            base(vec![
                ("mois", val_texte(socle::date::libelle_du_mois_ou_tel_quel(&m, socle::date::Graphie::Accentuee))),
                ("cachets", val_entier(n)),
                ("plafond", val_entier(c.cachets_par_mois)),
                ("perdus", val_entier(n - c.cachets_par_mois)),
            ]),
        );
    }
    let perdu = pos.enseignement_perdu()?;
    if perdu > 0 {
        pousser(
            "historique:enseignement-perdu",
            base(vec![
                ("heures", val_entier(pos.heures_enseignement())),
                ("plafond", val_entier(pos.plafond_enseignement()?)),
                ("perdus", val_entier(perdu)),
            ]),
        );
    }
    Ok(out)
}

/// `conseils-resume` : a l'accueil, les salaries dont nos seuls contrats font au moins 507 heures sur la fenetre de
/// leurs droits, la meme que leur fiche (`position`) : les 12 mois avant leur date anniversaire si la fiche la porte,
/// sinon les 12 derniers mois. La phrase dit l'une, l'autre, ou les deux quand la liste les mele : la banque essaie
/// `historique-resume-melange`, puis `historique-resume-anniversaire`, puis `historique-resume`, et le premier dont
/// les deux groupes de noms sont la parle. `{"conseils": [...]}`, vide quand il n'y en a pas.
pub fn resume(ctx: &Contexte) -> Resultat<Vec<Value>> {
    if !actifs(&ctx.entreprise) {
        return Ok(Vec::new());
    }
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(&ctx.code, jour.an);
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let engs = engagement::cumul::lire_engagements(ctx, None)?;
    let cles: BTreeSet<&str> = engs.iter().map(|e| e.salarie.as_str()).collect();
    // Les salaries par leur prenom ; deux du meme prenom, par leur prenom et leur nom (sinon on ne saurait lequel).
    let mut fiches = Vec::new();
    for cle in cles {
        let (p, anniv) = position(ctx, cle, &engs, jour)?;
        if p.heures_ouverture()? >= c.seuil_heures {
            let mut fiche = ctx.salaries.get(cle).cloned().unwrap_or(Value::Null);
            if !fiche.is_object() {
                fiche = Value::Object(Map::new());
            }
            fiche["_cle"] = Value::String(cle.to_string());
            fiches.push((prenom(&fiche), fiche, anniv));
        }
    }
    if fiches.is_empty() {
        return Ok(Vec::new());
    }
    // Les annexes de ceux que la phrase nomme : la reference cite X pour un artiste, VIII pour un technicien, les deux
    // quand la liste les mele.
    let mut en_jeu: Vec<&str> = Vec::new();
    for (_, fiche, _) in &fiches {
        for a in annexes_en_jeu(annexe_de(fiche)?) {
            if !en_jeu.contains(&a) {
                en_jeu.push(a);
            }
        }
    }
    let noms: Vec<(String, bool)> = fiches
        .iter()
        .map(|(p, fiche, anniv)| {
            let n = if fiches.iter().filter(|(q, _, _)| q == p).count() > 1 {
                let nom = Some(texte(fiche, "nom_affichage")).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| texte(fiche, "nom"));
                if nom.trim().is_empty() { p.clone() } else { format!("{} {}", p, nom.trim()) }
            } else {
                p.clone()
            };
            (n, *anniv)
        })
        .collect();
    let mut v = Valeurs::new();
    v.insert("seuil".into(), val_entier(c.seuil_heures));
    // `{noms_anniversaire}` (et `{n_anniversaire}`) : ceux dont la fiche porte la date anniversaire ;
    // `{noms_glissants}` : les autres. Un groupe vide ne se pose pas : les conseils qui le nomment se taisent.
    for (groupe, anniv) in [("anniversaire", true), ("glissants", false)] {
        let qui: Vec<String> = noms.iter().filter(|(_, a)| *a == anniv).map(|(n, _)| n.clone()).collect();
        if !qui.is_empty() {
            v.insert(format!("n_{}", groupe), val_entier(qui.len() as i64));
            v.insert(format!("noms_{}", groupe), val_texte(et(&qui)));
        }
    }
    Ok(conseil(&b, &d, "historique:resume", &v, &en_jeu).into_iter().collect())
}

/// « Anastasia », « Anastasia et Léandre », « Anastasia, Fanchon et Léandre ».
fn et(noms: &[String]) -> String {
    match noms {
        [] => String::new(),
        [seul] => seul.clone(),
        [tete @ .., dernier] => format!("{} et {}", tete.join(", "), dernier),
    }
}

// ------------------------------------------------------------------ routes

/// L'aiguillage des trois routes de lecture.
pub fn route(ctx: &Contexte, route: &str, charge: &Value) -> R<Value> {
    match route {
        "conseil-du-jour" => {
            let decalage = charge.get("decalage").and_then(Value::as_i64).unwrap_or(0);
            Ok(du_jour(ctx, decalage)?)
        }
        "conseils-salarie" => {
            let cle = charge.get("salarie").map(en_texte).unwrap_or_default();
            Ok(json!({ "conseils": du_salarie(ctx, &cle)? }))
        }
        "conseils-resume" => Ok(json!({ "conseils": resume(ctx)? })),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}
