//! LES ECARTS DE CALCUL D'UN ENGAGEMENT IMPORTE : ce que ce moteur aurait calcule de la saisie qu'un autre logiciel a
//! payee, compare ligne a ligne a ce que son bulletin imprime. Le but est de voir ou deux logiciels lisent les textes
//! autrement ; l'engagement importe ne change pas (il se lit, il ne se recalcule pas : `importe`), et la paie calculee
//! ici ne s'ecrit nulle part.
//!
//! LA SAISIE REFAITE : le salarie (sa fiche, a la date de la periode : son statut, son metier), le spectacle que le
//! bulletin nomme, la nature que la DSN ou l'AEM atteste (muette : un CDDU, regle 6), les lignes de remuneration que le
//! bulletin imprime, par leur rubrique du catalogue (quantite, taux, heures), aux jours qu'il dit travailles, payee le
//! jour du versement. Elle se calcule comme la validation la calculerait (`charger_donnees`), avec les baremes et la
//! configuration a la date de la periode ; une compagnie qui n'a pas encore pose son taux AT/MP de l'annee lit celui que
//! le bulletin applique (`Contexte::avec_le_taux_atmp_lu`).
//!
//! LA COMPARAISON, PAR COTISATION : chaque ligne de cotisation du bulletin importe porte la cotisation que la table de
//! lecture lui rattache (`rattacher`, par son libelle) ; chaque ligne de la paie calculee, la sienne (`cotisation_de`).
//! Plusieurs lignes d'une meme cotisation s'additionnent (les tranches de l'Apec, les contributions de « Autres charges
//! dues par l'employeur ») ; leur base et leurs taux ne se comparent que quand chaque cote n'en a qu'une. Un bulletin au
//! modele de l'arrete du 25/02/2016 (art. 3) range la caisse des Conges Spectacles dans « Autres charges dues par
//! l'employeur » et la CET dans « Retraite complementaire, CEG et CET T1 » : quand le bulletin importe ne les imprime
//! pas a part, les notres y entrent aussi (`REPLIS`). Un montant compare a l'arrondi de chaque ligne pres (1 centime par
//! ligne additionnee), un taux a la milliemes de point pres (le bulletin en imprime trois decimales).
//!
//! CE QUI NE SE COMPARE PAS LE DIT (regle 1) : une periode sans bareme, une paie sans bulletin (la DSN seule ne porte
//! pas les lignes du bulletin), une ligne de remuneration soumise sans rubrique, une saisie que le calcul refuse (une
//! convention sans couche, une fiche incomplete) ; et, dans une paie comparee, les lignes de cotisation que la table ne
//! rattache a aucune cotisation.
//!
//! CE QUI SE REND NE DESIGNE PERSONNE : la cotisation, les libelles, la grandeur, les deux taux et l'ecart RELATIF d'une
//! base ou d'un montant (jamais le montant), la periode, la nature et le statut.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::calcul::LigneCotisation;
use crate::importe::Importe;
use droit::contexte::Contexte;
use socle::date::{dernier_du_mois, en_texte, Jour};
use socle::fichiers::vrai;
use socle::num::{dec, dec_f64, Decimal};

/// Ce que l'on compare d'une ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grandeur {
    Base,
    TauxSalarie,
    TauxEmployeur,
    MontantSalarie,
    MontantEmployeur,
    /// Le bulletin importe n'imprime pas une cotisation que ce moteur calcule.
    AbsenteDuBulletin,
    /// Le bulletin importe imprime une cotisation que ce moteur ne calcule pas.
    AbsenteDuCalcul,
}

impl Grandeur {
    pub fn code(self) -> &'static str {
        match self {
            Grandeur::Base => "base",
            Grandeur::TauxSalarie => "taux_salarie",
            Grandeur::TauxEmployeur => "taux_employeur",
            Grandeur::MontantSalarie => "montant_salarie",
            Grandeur::MontantEmployeur => "montant_employeur",
            Grandeur::AbsenteDuBulletin => "absente_du_bulletin",
            Grandeur::AbsenteDuCalcul => "absente_du_calcul",
        }
    }

    /// En francais, pour un resume.
    pub fn dite(self) -> &'static str {
        match self {
            Grandeur::Base => "base",
            Grandeur::TauxSalarie => "taux salarial",
            Grandeur::TauxEmployeur => "taux employeur",
            Grandeur::MontantSalarie => "montant salarial",
            Grandeur::MontantEmployeur => "montant employeur",
            Grandeur::AbsenteDuBulletin => "calculée ici, absente du bulletin importé",
            Grandeur::AbsenteDuCalcul => "sur le bulletin importé, que ce moteur ne calcule pas",
        }
    }
}

/// Un ecart sur une cotisation (ou un total : `total_salarie`, `total_employeur`, `net_social`, `net_imposable`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ecart {
    pub cotisation: String,
    /// Les libelles du bulletin importe pour cette cotisation (les notres quand il ne l'imprime pas).
    pub libelles: Vec<String>,
    pub grandeur: Grandeur,
    pub notre_taux: Option<Decimal>,
    pub leur_taux: Option<Decimal>,
    /// (le leur - le notre) / le notre, a 4 decimales, pour une base ou un montant ; `None` pour un taux, ou quand le
    /// notre est nul.
    pub ecart_relatif: Option<Decimal>,
}

impl Ecart {
    pub fn to_json(&self) -> Value {
        let d = |x: &Option<Decimal>| x.map_or(Value::Null, |x| json!(socle::num::nombre(x)));
        json!({
            "cotisation": self.cotisation,
            "libelles": self.libelles,
            "grandeur": self.grandeur.code(),
            "notre_taux": d(&self.notre_taux),
            "leur_taux": d(&self.leur_taux),
            "ecart_relatif": d(&self.ecart_relatif),
        })
    }
}

/// La comparaison d'une paie importee (un mois de l'engagement).
#[derive(Clone, Debug)]
pub struct Comparaison {
    pub mois: String,
    /// Le logiciel que le lecteur a reconnu au bulletin ; `None` : un autre.
    pub logiciel: Option<String>,
    pub nature: Option<String>,
    pub statut: Option<String>,
    /// Les ecarts ; `Err` : la paie ne se compare pas, et pourquoi.
    pub ecarts: Result<Vec<Ecart>, String>,
    /// Les libelles de cotisation du bulletin que la table ne rattache a aucune cotisation : non compares.
    pub non_rattachees: Vec<String>,
}

/// Quand le bulletin importe n'imprime pas la premiere a part, elle se compare dans la seconde (le modele de l'arrete
/// du 25/02/2016, art. 3 : « Autres charges dues par l'employeur » ; la ligne « Retraite complementaire, CEG et CET »).
const REPLIS: [(&str, &str); 2] = [("conges_spec", "autres"), ("cet", "retraite_t1")];

/// La cotisation d'une ligne de la paie calculee, au grain ou un bulletin l'imprime : les contributions que
/// l'employeur paie seul et que le modele agrege (`autres`), les cotisations de la convention (`convention`), les
/// tranches de l'Apec et de la prevoyance ; les autres gardent leur identifiant du bareme.
pub fn cotisation_de(l: &LigneCotisation) -> String {
    let seule_employeur = l.taux_sal.is_none() && l.montant_sal.is_zero();
    match l.id.as_str() {
        "autres_1" | "autres_2" | "autres_3" | "autres_4" | "versement_mobilite" if seule_employeur => "autres".into(),
        "ccn" | "fcap" | "paritarisme" => "convention".into(),
        "apec_t1" | "apec_t2" => "apec".into(),
        id if id.starts_with("prevoyance") => "prevoyance".into(),
        id => id.into(),
    }
}

/// Une ligne de cotisation du bulletin importe.
#[derive(Clone, Debug, Default)]
struct Lue {
    libelle: String,
    base: Option<Decimal>,
    taux_salarie: Option<Decimal>,
    taux_employeur: Option<Decimal>,
    montant_salarie: Option<Decimal>,
    montant_employeur: Option<Decimal>,
}

fn decimal(v: Option<&Value>) -> Option<Decimal> {
    v.filter(|x| vrai(x)).and_then(|x| dec(x).ok())
}

fn texte(o: &Map<String, Value>, k: &str) -> Option<String> {
    o.get(k).filter(|x| vrai(x)).map(en_texte)
}

/// Les comparaisons de chaque paie de l'engagement importe `imp`, mois par mois. `rattacher` : la cotisation que la
/// table de lecture rattache a un libelle de cotisation imprime.
pub fn comparer(racine: &Path, imp: &Importe, rattacher: &dyn Fn(&str) -> Option<String>) -> Vec<Comparaison> {
    let brut = &imp.objet().brut;
    let contrat = imp.contrat();
    let nature = contrat.and_then(|c| texte(c, "nature")).or_else(|| texte(brut, "nature"));
    imp.paies()
        .into_iter()
        .map(|(mois, p)| {
            let logiciel = p.get("bulletin").and_then(|b| b.get("logiciel")).filter(|x| vrai(x)).map(en_texte);
            let mut c = Comparaison { mois: mois.clone(), logiciel, nature: nature.clone(), statut: None, ecarts: Ok(Vec::new()), non_rattachees: Vec::new() };
            c.ecarts = comparer_une(racine, brut, &mois, p, nature.as_deref(), rattacher, &mut c);
            c
        })
        .collect()
}

fn comparer_une(
    racine: &Path,
    brut: &Map<String, Value>,
    mois: &str,
    p: &Map<String, Value>,
    nature: Option<&str>,
    rattacher: &dyn Fn(&str) -> Option<String>,
    c: &mut Comparaison,
) -> Result<Vec<Ecart>, String> {
    let annee = mois.get(..4).unwrap_or("");
    let fin = dernier_du_mois(mois)?;
    let mut ctx = Contexte::nouveau(racine, Some(fin))?;
    if !ctx.code.join("baremes").join(format!("{}.json", annee)).exists() {
        return Err(format!("aucun barème livré pour {} : la période ne se calcule pas ici", annee));
    }
    let salarie = texte(brut, "salarie").ok_or("l'engagement importé ne nomme pas son salarié")?;
    let fiche = ctx.salarie(&salarie)?;
    c.statut = fiche.get("statut").filter(|x| vrai(x)).map(en_texte);
    let Some(Value::Array(cotisations)) = p.get("cotisations") else {
        return Err("la paie importée n'a pas de bulletin : la DSN seule ne porte pas ses lignes de cotisation".into());
    };
    // LE TAUX AT/MP est celui de l'etablissement, notifie par l'Assurance Maladie. Une compagnie qui n'a pas encore pose
    // le sien pour l'annee lit celui que le bulletin applique (son taux employeur, sinon son montant sur sa base), ramene
    // au taux notifie par le facteur que le bareme applique a ce contrat (`droit::bareme::bareme_atmp`), au centieme de
    // point comme un taux notifie. L'AT/MP n'est alors pas compare : il est l'entree.
    let an: i32 = annee.parse().map_err(|_| format!("mois illisible : {}", mois))?;
    let mut atmp_applique: Option<Decimal> = None;
    if !racine.join("config").join("baremes").join(format!("{}.json", annee)).exists() {
        let ligne = cotisations.iter().find(|l| l.get("libelle").map(en_texte).is_some_and(|x| rattacher(&x).as_deref() == Some("atmp")));
        atmp_applique = ligne.and_then(|l| {
            decimal(l.get("taux_employeur")).or_else(|| match (decimal(l.get("montant_employeur")), decimal(l.get("base"))) {
                (Some(m), Some(b)) if !b.is_zero() => Some((m / b * Decimal::from(100)).round_dp(2)),
                _ => None,
            })
        });
        if atmp_applique.is_some() {
            ctx = ctx.avec_le_taux_atmp_lu(an, json!(1.0));
        }
    }
    let atmp_lu = atmp_applique.is_some();
    let lignes = p.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    // Les jours travailles que le bulletin dit, chacun sur chaque ligne (les jours de l'engagement en sont la reunion).
    let mut jours: Vec<Value> = Vec::new();
    for d in p.get("dates").and_then(Value::as_array).into_iter().flatten() {
        let (Some(a), Some(b)) = (d.get("debut").and_then(Value::as_str), d.get("fin").and_then(Value::as_str)) else { continue };
        let (mut j, b) = (Jour::lire(a)?, Jour::lire(b)?);
        while j <= b {
            jours.push(json!(j.iso()));
            j = j.plus_jours(1)?;
        }
    }
    let mut saisies = Vec::new();
    for l in &lignes {
        let libelle = l.get("libelle").map(en_texte).unwrap_or_default();
        let Some(r) = l.get("rubrique").filter(|x| vrai(x)) else {
            if l.get("soumis") == Some(&Value::Bool(false)) {
                continue;
            }
            return Err(format!("la ligne de rémunération « {} » ne se rattache à aucune rubrique du catalogue", libelle));
        };
        let mut s = Map::new();
        s.insert("rubrique".into(), r.clone());
        let quantite = l.get("quantite").filter(|x| vrai(x)).cloned().ok_or_else(|| format!("la ligne « {} » n'imprime pas sa quantité", libelle))?;
        let taux = l.get("taux").filter(|x| vrai(x)).cloned().ok_or_else(|| format!("la ligne « {} » n'imprime pas son taux", libelle))?;
        s.insert("quantite".into(), json!(dec(&quantite)?.to_string().parse::<f64>().map_err(|e| e.to_string())?));
        s.insert("taux".into(), json!(dec(&taux)?.to_string().parse::<f64>().map_err(|e| e.to_string())?));
        if let Some(h) = l.get("heures").filter(|x| vrai(x)) {
            s.insert("heures".into(), json!(dec(h)?.to_string().parse::<f64>().map_err(|e| e.to_string())?));
        }
        if !jours.is_empty() {
            s.insert("dates".into(), Value::Array(jours.clone()));
        }
        saisies.push(Value::Object(s));
    }
    let mensuelle = droit::natures::nature(nature).is_ok_and(|n| n.mensuelle());
    let mut e = Map::new();
    e.insert("salarie".into(), json!(salarie));
    if !mensuelle {
        // Le spectacle que le bulletin nomme ; sans titre, celui dont la fiche porte son numero d'objet.
        let par_objet = texte(p, "numero_objet").and_then(|n| {
            ctx.spectacles.iter().find(|(_, f)| f.get("numero_objet").map(en_texte).as_deref() == Some(n.as_str())).map(|(k, _)| json!(k))
        });
        if let Some(sp) = p.get("spectacle").filter(|x| vrai(x)).cloned().or(par_objet) {
            e.insert("spectacle".into(), sp);
        }
    }
    if let Some(n) = nature {
        e.insert("nature".into(), json!(n));
    }
    for k in ["debut", "fin"] {
        if let Some(v) = p.get(k).filter(|x| vrai(x)) {
            e.insert(k.into(), v.clone());
        }
    }
    e.insert("lignes".into(), Value::Array(saisies));
    let mut data = json!({"mois": mois, "engagements": [Value::Object(e)]});
    if let Some(v) = p.get("verse_le").filter(|x| vrai(x)) {
        data["date_paiement"] = v.clone();
    }
    let mut saisie = crate::charger_donnees(&ctx, &data)?;
    if let Some(applique) = atmp_applique {
        // Calcule au taux notifie de 1 %, la ligne AT/MP porte le facteur du contrat.
        let facteur = saisie.paies.first().and_then(|p| p.lignes.iter().find(|l| l.id == "atmp")).and_then(|l| l.taux_pat).filter(|f| *f > 0.0);
        if let Some(f) = facteur {
            let notifie = (applique / dec_f64(f)?).round_dp(2);
            ctx = ctx.avec_le_taux_atmp_lu(an, json!(notifie.to_string().parse::<f64>().map_err(|e| e.to_string())?));
            saisie = crate::charger_donnees(&ctx, &data)?;
        }
    }
    let paie = saisie.paies.first().ok_or("le calcul ne rend aucune paie")?;

    // Les deux cotes, par cotisation.
    let mut leurs: BTreeMap<String, Vec<Lue>> = BTreeMap::new();
    // Les cotisations que le bulletin imprime apres son total (les facultatives, les exonerations) : hors de ce total.
    let mut hors_total: Vec<String> = Vec::new();
    for l in cotisations {
        let lue = Lue {
            libelle: l.get("libelle").map(en_texte).unwrap_or_default(),
            base: decimal(l.get("base")),
            taux_salarie: decimal(l.get("taux_salarie")),
            taux_employeur: decimal(l.get("taux_employeur")),
            montant_salarie: decimal(l.get("montant_salarie")),
            montant_employeur: decimal(l.get("montant_employeur")),
        };
        if lue.montant_salarie.is_none() && lue.montant_employeur.is_none() {
            continue;
        }
        match rattacher(&lue.libelle) {
            Some(code) => {
                if l.get("hors_total") == Some(&Value::Bool(true)) && !hors_total.contains(&code) {
                    hors_total.push(code.clone());
                }
                leurs.entry(code).or_default().push(lue)
            }
            None => {
                if !c.non_rattachees.contains(&lue.libelle) {
                    c.non_rattachees.push(lue.libelle);
                }
            }
        }
    }
    let mut nos: BTreeMap<String, Vec<&LigneCotisation>> = BTreeMap::new();
    for l in &paie.lignes {
        nos.entry(cotisation_de(l)).or_default().push(l);
    }
    for (fin, gros) in REPLIS {
        if !leurs.contains_key(fin) {
            if let Some(v) = nos.remove(fin) {
                nos.entry(gros.into()).or_default().extend(v);
            }
        }
    }
    let mut ecarts = Vec::new();
    let mut codes: Vec<&String> = leurs.keys().chain(nos.keys()).collect();
    codes.sort();
    codes.dedup();
    let zero = Decimal::ZERO;
    for code in codes {
        if atmp_lu && code == "atmp" {
            continue;
        }
        let (eux, nous) = (leurs.get(code).map_or(&[][..], |v| &v[..]), nos.get(code).map_or(&[][..], |v| &v[..]));
        let leur_sal: Decimal = eux.iter().filter_map(|l| l.montant_salarie).map(|x| x.abs()).sum();
        let leur_pat: Decimal = eux.iter().filter_map(|l| l.montant_employeur).map(|x| x.abs()).sum();
        let notre_sal: Decimal = nous.iter().map(|l| l.montant_sal.abs()).sum();
        let notre_pat: Decimal = nous.iter().map(|l| l.montant_pat.abs()).sum();
        let mut libelles: Vec<String> = Vec::new();
        for l in eux.iter().map(|l| l.libelle.clone()).chain(eux.is_empty().then(|| nous.iter().map(|l| l.libelle.clone())).into_iter().flatten()) {
            if !libelles.contains(&l) {
                libelles.push(l);
            }
        }
        let ecart = |grandeur, notre_taux, leur_taux, ecart_relatif| Ecart { cotisation: code.clone(), libelles: libelles.clone(), grandeur, notre_taux, leur_taux, ecart_relatif };
        let (chez_eux, chez_nous) = (leur_sal + leur_pat > zero, notre_sal + notre_pat > zero);
        if chez_nous && !chez_eux {
            ecarts.push(ecart(Grandeur::AbsenteDuBulletin, None, None, None));
            continue;
        }
        if chez_eux && !chez_nous {
            ecarts.push(ecart(Grandeur::AbsenteDuCalcul, None, None, None));
            continue;
        }
        if !chez_eux {
            continue;
        }
        let tolerance = Decimal::new(1, 2) * Decimal::from(eux.len().max(nous.len()) as i64);
        for (grandeur, leur, notre) in [(Grandeur::MontantSalarie, leur_sal, notre_sal), (Grandeur::MontantEmployeur, leur_pat, notre_pat)] {
            if (leur - notre).abs() > tolerance {
                ecarts.push(ecart(grandeur, None, None, relatif(leur, notre)));
            }
        }
        let ([l], [n]) = (eux, nous) else { continue };
        if let Some(b) = l.base.filter(|_| !n.masquer_base_et_taux) {
            if (b - n.base).abs() > Decimal::new(1, 2) {
                ecarts.push(ecart(Grandeur::Base, None, None, relatif(b, n.base)));
            }
        }
        for (grandeur, leur, notre) in [(Grandeur::TauxSalarie, l.taux_salarie, n.taux_sal), (Grandeur::TauxEmployeur, l.taux_employeur, n.taux_pat)] {
            let Some(leur) = leur else { continue };
            let notre = notre.map(dec_f64).transpose()?;
            if (leur - notre.unwrap_or(zero)).abs() > Decimal::new(5, 4) {
                ecarts.push(ecart(grandeur, notre, Some(leur), None));
            }
        }
    }
    // Les totaux du bulletin, sans ce qu'il imprime apres eux : l'ecart relatif seulement.
    let hors = |f: &dyn Fn(&LigneCotisation) -> Decimal| -> Decimal { paie.lignes.iter().filter(|l| hors_total.contains(&cotisation_de(l))).map(f).sum() };
    for (cle, notre) in [
        ("cotisations_salariales", paie.total_sal - hors(&|l| l.montant_sal)),
        ("cotisations_patronales", paie.total_pat - hors(&|l| l.montant_pat)),
        ("net_social", paie.net_social),
        ("net_imposable", paie.net_imposable),
    ] {
        let Some(leur) = decimal(p.get(cle)) else { continue };
        if (leur - notre).abs() > Decimal::new(1, 2) {
            let grandeur = if cle == "cotisations_patronales" { Grandeur::MontantEmployeur } else { Grandeur::MontantSalarie };
            ecarts.push(Ecart { cotisation: cle.into(), libelles: Vec::new(), grandeur, notre_taux: None, leur_taux: None, ecart_relatif: relatif(leur, notre) });
        }
    }
    Ok(ecarts)
}

/// (leur - notre) / notre, a 4 decimales ; `None` quand le notre est nul.
fn relatif(leur: Decimal, notre: Decimal) -> Option<Decimal> {
    (!notre.is_zero()).then(|| ((leur - notre) / notre).round_dp(4))
}
