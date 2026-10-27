//! LES ORGANISMES ET LES CONTRATS DE LA DSN (09/10/2026). `config/organismes.json` n'avait aucun ecran : une compagnie
//! neuve ne pouvait pas faire sa premiere DSN, et le fichier recopiait des valeurs qui vivent ailleurs (le taux AT/MP,
//! fige quand il changeait ; le compte France Travail, deja dans la fiche Entreprise).
//!
//! Le fichier se COMPOSE ici, une fois, quand la compagnie enregistre ses contrats, et la DSN le lit tel quel :
//! - la partie normative (organismes, CTP, bloc 08) vient de `donnees/organismes-base.json`, la meme pour toutes ;
//! - les contrats attendus aussi (`contrats` : ceux de toutes les compagnies, puis ceux de la convention), plus la
//!   CASC-SVP (`casc-svp-base.json`) et les contributions de branche en DSN (`contributions_dsn` du bareme) : les regles
//!   ne sont pas recopiees ;
//! - la compagnie ne saisit que ce qui est a elle : la reference de ses contrats chez l'organisme, et l'organisme quand
//!   la branche n'en designe pas.
//!
//! Les identifiants d'adhesion deja declares (S21.G00.15.005) ne bougent jamais : relus dans le fichier en place. Un
//! contrat des permanents (`si_permanents`) n'est exige que le jour ou la compagnie en emploie un : sans lui, la DSN
//! refuse le permanent (`dsn::affiliations`), pas la compagnie.

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::ecriture::ecrire_json;
use socle::fichiers::{dossier_du_code, lire_json, vrai};
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE_LIRE: &str = "organismes-contrats";
pub const ROUTE_ECRIRE: &str = "organismes-enregistrer";

/// Audiens, delegataire des contrats de branche du spectacle (S21.G00.15.002).
const AUDIENS: &str = "P0983";
/// Le premier identifiant d'adhesion libre : 100 a 103 sont ceux que `affectations_81_059` nomme.
const PREMIER_LIBRE: u32 = 104;

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some("ErreurOperation"), m)
}

fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&Value::Null)
}

fn txt(v: &Value) -> String {
    if vrai(v) {
        en_texte(v).trim().to_string()
    } else {
        String::new()
    }
}

fn liste(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

fn chemin(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("config").join("organismes.json")
}

fn base() -> R<Value> {
    lire_json(&dossier_du_code().join("organismes-base.json")).map_err(plain)
}

/// LE VERSEMENT NORMATIF D'UN ORGANISME (ticket #223) : l'entree de `donnees/organismes-base.json` dont `siret` est
/// `identifiant` (S21.G00.20.001), ou `None`. On y lit son `nom` propre, ce que couvre son versement (`objet`) et son
/// `jour_de_paiement` dans le mois qui suit, chacun avec sa source (`_source_*`). Lu dans la base livree, pas dans
/// `config/organismes.json` : c'est la norme, la meme pour toute compagnie.
pub fn versement_normatif(identifiant: &str) -> Result<Option<Value>, String> {
    let b = lire_json(&dossier_du_code().join("organismes-base.json"))?;
    Ok(b.get("versements")
        .and_then(Value::as_array)
        .and_then(|vs| vs.iter().find(|v| v.get("siret").map(en_texte).as_deref() == Some(identifiant)))
        .cloned())
}

/// Le jour du mois suivant ou l'organisme `identifiant` preleve (`jour_de_paiement` de la base), ou `None`.
pub fn jour_de_paiement(identifiant: &str) -> Result<Option<u32>, String> {
    Ok(versement_normatif(identifiant)?.and_then(|v| v.get("jour_de_paiement").and_then(Value::as_u64)).map(|j| j as u32))
}

/// Les contrats que cette compagnie declare, dans l'ordre ou ils se presentent.
fn attendus(ctx: &Contexte, base: &Value) -> R<Vec<Value>> {
    let ent = &ctx.entreprise;
    let idcc = txt(g(ent, "idcc"));
    let contrats = g(base, "contrats");
    let mut out = liste(g(contrats, "tous"));
    let casc = crate::casc::base().map_err(plain)?;
    if idcc == txt(g(&casc, "idcc")) && !droit::bareme::casc_exemptee(ent).map_err(plain)? {
        out.push(json!({
            "cle": "casc", "libelle": txt(g(g(&casc, "dsn"), "contrat")), "reference": "100", "type": "02",
            "beneficiaires": "tous les salariés", "quoi": "CASC-SVP (activités sociales et culturelles)",
            "organisme": AUDIENS, "organisme_fixe": true,
        }));
    }
    // Les contributions de branche en DSN : le bareme dit lesquelles. Un bareme illisible (taux AT/MP manquant...) se
    // signale ailleurs (`pour-demarrer`) ; ici, il ne fait que taire ces lignes.
    if let Ok(bar) = ctx.bareme(&ctx.aujourdhui) {
        for c in liste(g(&bar, "contributions_dsn")) {
            let nom = txt(g(&c, "contribution"));
            if nom.is_empty() {
                continue;
            }
            out.push(json!({
                "cle": format!("contribution_{}", nom.to_lowercase()), "contribution": nom, "libelle_defaut": nom,
                "type": "01", "beneficiaires": "selon le contrat",
                "quoi": format!("Contribution {} (en DSN depuis {})", nom, txt(g(&c, "en_dsn_depuis"))),
                "organisme": AUDIENS, "organisme_fixe": true,
            }));
        }
    }
    out.extend(liste(g(contrats, &idcc)));
    Ok(out)
}

/// L'adhesion du fichier en place qui tient ce contrat : par sa cle, son identifiant fixe, sa contribution, ou sa place
/// dans les affiliations des permanents (prevoyance puis sante) pour un fichier ecrit avant cet ecran.
fn en_place<'a>(spec: &Value, fichier: &'a Value) -> Option<&'a Value> {
    let adh = fichier.get("adhesions_prevoyance")?.as_array()?;
    let cle = txt(g(spec, "cle"));
    if let Some(a) = adh.iter().find(|a| txt(g(a, "cle")) == cle) {
        return Some(a);
    }
    let reference = txt(g(spec, "reference"));
    if !reference.is_empty() {
        return adh.iter().find(|a| txt(g(a, "reference")) == reference);
    }
    let contribution = txt(g(spec, "contribution")).to_uppercase();
    if !contribution.is_empty() {
        return adh.iter().find(|a| {
            txt(g(a, "contribution")).to_uppercase() == contribution || txt(g(a, "libelle")).to_uppercase() == contribution
        });
    }
    let place = match txt(g(spec, "role")).as_str() {
        "prevoyance" => 0,
        "sante" => 1,
        _ => return None,
    };
    let refs = liste(g(g(g(fichier, "affiliations_par_nature"), "permanent"), "references"));
    let r = refs.get(place).map(txt)?;
    adh.iter().find(|a| txt(g(a, "reference")) == r)
}

/// Une ligne de l'ecran : ce que le contrat couvre, et ce qui est saisi.
fn ligne(spec: &Value, fichier: &Value) -> Value {
    let place = en_place(spec, fichier);
    let libelle_fixe = vrai(g(spec, "libelle"));
    let libelle = if libelle_fixe {
        txt(g(spec, "libelle"))
    } else {
        place.map(|a| txt(g(a, "libelle"))).filter(|s| !s.is_empty()).unwrap_or_else(|| txt(g(spec, "libelle_defaut")))
    };
    let organisme_fixe = vrai(g(spec, "organisme_fixe"));
    let organisme = if organisme_fixe {
        txt(g(spec, "organisme"))
    } else {
        place.map(|a| txt(g(a, "delegataire"))).filter(|s| !s.is_empty()).unwrap_or_else(|| txt(g(spec, "organisme")))
    };
    json!({
        "cle": txt(g(spec, "cle")),
        "quoi": txt(g(spec, "quoi")),
        "beneficiaires": txt(g(spec, "beneficiaires")),
        "libelle": libelle,
        "libelle_fixe": libelle_fixe,
        "organisme": organisme,
        "organisme_fixe": organisme_fixe,
        "si_permanents": vrai(g(spec, "si_permanents")),
        "depuis": txt(g(spec, "depuis")),
        "jusqu_a": txt(g(spec, "jusqu_a")),
        "declare": place.is_some(),
        "note": txt(g(spec, "_organisme")),
    })
}

fn fichier_en_place(ctx: &Contexte) -> R<Value> {
    let p = chemin(ctx);
    if p.is_file() {
        lire_json(&p).map_err(plain)
    } else {
        Ok(json!({}))
    }
}

/// Une ligne se montre-t-elle a cette compagnie ? Un contrat des permanents (`role` : prevoyance ou sante, les
/// affiliations d'un salarie paye au mois) ne se montre qu'a une compagnie qui en emploie (ticket #214 : « j'emploie pas
/// de permanent pourtant on me propose de remplir ») : le curseur est `Contexte::emploie_des_permanents`, le meme que
/// partout. Ce qui ne se montre pas ne change pas pour autant : `composer` garde un contrat fixe (la sante des
/// permanents de la 3090, « declaree meme sans permanent ») et un contrat deja saisi (lu dans le fichier en place), et ne
/// reclame pas un `si_permanents` vide.
fn se_montre(ctx: &Contexte, spec: &Value) -> bool {
    txt(g(spec, "role")).is_empty() || ctx.emploie_des_permanents()
}

/// La periode que l'ecran regarde : le mois `mois` de la charge (celui de l'onglet Paie), sinon celui du jour.
fn periode(ctx: &Contexte, charge: &Value) -> R<(Jour, Jour)> {
    let mois = match txt(g(charge, "mois")) {
        m if m.is_empty() => ctx.aujourdhui.iso()[..7].to_string(),
        m => m,
    };
    let premier = socle::date::premier_du_mois(&mois).map_err(plain)?;
    let dernier = socle::date::dernier_du_mois(&mois).map_err(plain)?;
    Ok((premier, dernier))
}

/// Le contrat vaut-il pour la periode (`droit::adhesions::en_vigueur`) ? La Configuration ne propose que ceux-la.
fn couvre(spec: &Value, (debut, fin): &(Jour, Jour)) -> R<bool> {
    droit::adhesions::en_vigueur(spec, debut, fin).map_err(plain)
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`couvre`).
pub fn lire(ctx: &Contexte, charge: &Value) -> R<Value> {
    let base = base()?;
    let fichier = fichier_en_place(ctx)?;
    let periode = periode(ctx, charge)?;
    let mut lignes: Vec<Value> = Vec::new();
    for s in attendus(ctx, &base)? {
        if se_montre(ctx, &s) && couvre(&s, &periode)? {
            lignes.push(ligne(&s, &fichier));
        }
    }
    Ok(json!({
        "idcc": txt(g(&ctx.entreprise, "idcc")),
        "compose": chemin(ctx).is_file(),
        "mois": periode.0.mois_iso(),
        "contrats": lignes,
    }))
}

/// Compose `config/organismes.json` depuis la base, les contrats attendus et ce que la compagnie a saisi
/// (`saisies` : cle -> {libelle, organisme}). Chaque contrat garde ses dates (`depuis`, `jusqu_a`) : la DSN de chaque
/// mois ne lit que ceux qui le couvrent. Un contrat a saisir que la periode regardee ne couvre pas ne se reclame pas :
/// l'ecran ne l'a pas propose.
pub fn composer(ctx: &Contexte, saisies: &Map<String, Value>, periode: &(Jour, Jour)) -> R<Value> {
    let base = base()?;
    let fichier = fichier_en_place(ctx)?;
    let mut prises: Vec<u32> = Vec::new();
    let mut retenus: Vec<(Value, Value)> = Vec::new();
    for spec in attendus(ctx, &base)? {
        let l = ligne(&spec, &fichier);
        let saisi = saisies.get(&txt(g(&spec, "cle")));
        let libelle = match saisi {
            Some(s) if !vrai(g(&l, "libelle_fixe")) => txt(g(s, "libelle")).to_uppercase(),
            _ => txt(g(&l, "libelle")),
        };
        let organisme = match saisi {
            Some(s) if !vrai(g(&l, "organisme_fixe")) => txt(g(s, "organisme")).to_uppercase(),
            _ => txt(g(&l, "organisme")),
        };
        let quoi = txt(g(&spec, "quoi"));
        if libelle.is_empty() {
            // Un contrat vide ne se reclame pas quand l'ecran ne l'a pas propose : un contrat des permanents a une
            // compagnie qui n'en emploie pas, un contrat que la periode regardee ne couvre pas.
            if (vrai(g(&spec, "si_permanents")) && !ctx.emploie_des_permanents()) || !couvre(&spec, periode)? {
                continue;
            }
            return Err(refus(format!(
                "{} : la référence du contrat manque. Elle se lit sur la fiche de paramétrage DSN de l'organisme \
(« référence contrat »).",
                quoi
            )));
        }
        // Les longueurs du cahier technique : S21.G00.15.001 de 1 a 30, S21.G00.15.002 de 5 a 9.
        if libelle.chars().count() > 30 {
            return Err(refus(format!("{} : « {} » dépasse 30 caractères, ce que la DSN n'admet pas.", quoi, libelle)));
        }
        let n = organisme.chars().count();
        if !(5..=9).contains(&n) || organisme.contains(char::is_whitespace) {
            return Err(refus(format!(
                "{} : le code de l'organisme ({}) a de 5 à 9 caractères sans espace, comme P0983 pour Audiens. Il est \
sur le certificat d'adhésion et la fiche de paramétrage DSN.",
                quoi,
                if organisme.is_empty() { "vide".to_string() } else { format!("« {} »", organisme) }
            )));
        }
        let reference = txt(g(&spec, "reference"));
        let reference = if reference.is_empty() { en_place(&spec, &fichier).map(|a| txt(g(a, "reference"))).unwrap_or_default() } else { reference };
        if let Ok(r) = reference.parse::<u32>() {
            prises.push(r);
        }
        let mut a = Map::new();
        a.insert("cle".into(), json!(txt(g(&spec, "cle"))));
        a.insert("libelle".into(), json!(libelle));
        a.insert("delegataire".into(), json!(organisme));
        a.insert("type".into(), json!(txt(g(&spec, "type"))));
        a.insert("reference".into(), json!(reference));
        for k in ["contribution", "depuis", "jusqu_a"] {
            if vrai(g(&spec, k)) {
                a.insert(k.into(), json!(txt(g(&spec, k))));
            }
        }
        retenus.push((spec, Value::Object(a)));
    }
    // Les identifiants nouveaux, a la suite des pris : ceux deja declares ne bougent pas.
    let mut libre = PREMIER_LIBRE;
    for (_, a) in retenus.iter_mut() {
        if txt(g(a, "reference")).is_empty() {
            while prises.contains(&libre) {
                libre += 1;
            }
            prises.push(libre);
            a["reference"] = json!(libre.to_string());
        }
    }
    let role = |r: &str| retenus.iter().find(|(s, _)| txt(g(s, "role")) == r).map(|(_, a)| txt(g(a, "reference")));
    let permanent = match (role("prevoyance"), role("sante")) {
        (Some(p), Some(s)) => json!({"references": [p, s]}),
        _ => json!({
            "references": null,
            "_a_confirmer": "La prévoyance et la santé des permanents ne sont pas toutes deux saisies (Configuration › \
Organismes et contrats) : un permanent est refusé tant qu'elles manquent.",
        }),
    };
    let intermittent: Vec<String> = ["tsintermittents", "interbranche"]
        .iter()
        .filter_map(|c| retenus.iter().find(|(s, _)| txt(g(s, "cle")) == *c).map(|(_, a)| txt(g(a, "reference"))))
        .collect();
    let mut adhesions: Vec<Value> = retenus.into_iter().map(|(_, a)| a).collect();
    adhesions.sort_by_key(|a| txt(g(a, "reference")).parse::<u32>().unwrap_or(u32::MAX));

    let mut out = match base {
        Value::Object(m) => m,
        _ => return Err(plain("donnees/organismes-base.json : un objet est attendu.")),
    };
    out.shift_remove("contrats");
    out.insert(
        "_quoi".into(),
        json!("Compose par PuppetPaie (Configuration › Organismes et contrats) depuis donnees/organismes-base.json et les \
contrats saisis. Ne pas modifier a la main : un nouvel enregistrement le recompose."),
    );
    // Une reference de versement deja posee (le compte France Travail d'un fichier d'avant) est gardee : la DSN refuse
    // si elle contredit la fiche Entreprise (`dsn::reference_versement`).
    if let Some(Value::Array(vs)) = out.get_mut("versements") {
        for v in vs.iter_mut() {
            let siret = txt(g(v, "siret"));
            let ancienne = liste(g(&fichier, "versements")).into_iter().find(|x| txt(g(x, "siret")) == siret);
            if let Some(r) = ancienne.map(|x| txt(g(&x, "reference"))).filter(|r| !r.is_empty()) {
                v["reference"] = json!(r);
            }
        }
    }
    out.insert("adhesions_prevoyance".into(), Value::Array(adhesions));
    out.insert("affiliations_par_nature".into(), json!({"intermittent": {"references": intermittent}, "permanent": permanent}));
    Ok(Value::Object(out))
}

/// `organismes-enregistrer` : `{"contrats": {cle: {libelle, organisme}}, "mois": "AAAA-MM"}` ; compose, ecrit, et rend
/// la lecture.
pub fn enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let saisies = g(charge, "contrats").as_object().cloned().unwrap_or_default();
    let compose = composer(ctx, &saisies, &periode(ctx, charge)?)?;
    let p = chemin(ctx);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| plain(e.to_string()))?;
    }
    crate::routes_contrats::sauver_avant(&p)?;
    ecrire_json(&p, &compose, 2).map_err(plain)?;
    lire(ctx, charge)
}
