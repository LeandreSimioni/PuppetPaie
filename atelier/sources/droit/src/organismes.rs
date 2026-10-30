//! LES ORGANISMES ET LES CONTRATS DE LA DSN d'une compagnie, composes a une date (`Contexte::organismes`) : ce que la
//! DSN, les signalements, l'amorcage et le contrat ecrit lisent des organismes.
//!
//! Tout se compose a la date lue, depuis les textes livres et la configuration de cette date :
//! - la partie normative (organismes payes, CTP, bloc 08) vient de `donnees/organismes-base.json`, la meme pour toutes ;
//! - les contrats attendus aussi (`contrats` : ceux de toutes les compagnies, puis ceux de la convention en vigueur a
//!   cette date), plus la CASC-SVP (`casc-svp-base.json`, `adhesions::casc_svp`) et les contributions de branche en DSN
//!   (`contributions_dsn` du bareme de cette date) ;
//! - de la compagnie, seulement ce qui est a elle (`config/organismes.json`, ecrit par Configuration › Organismes et
//!   contrats) : la reference de ses contrats chez l'organisme quand aucun texte ne la donne, l'organisme quand la
//!   branche n'en designe pas, et les identifiants d'adhesion deja attribues.
//!
//! Une compagnie qui n'a rien a saisir (les intermittents d'une 3090 : tous leurs contrats sont donnes par les textes)
//! n'a donc rien a enregistrer. Ce qui manque ne se devine pas : un contrat des permanents sans reference laisse
//! `affiliations_par_nature.permanent.references` a `null`, et la DSN refuse le permanent (`dsn::affiliations`), pas la
//! compagnie.
//!
//! `config/organismes.json` est DATE comme toute la configuration (`contexte::fiche_au`) : sa cle
//! `adhesions_prevoyance` est une liste `{depuis, valeur}`. Les identifiants d'adhesion (S21.G00.15.005) ne bougent
//! jamais : relus dans le fichier, et un identifiant nouveau n'en reprend aucun qu'une autre periode porte.

use serde_json::{json, Map, Value};

use socle::date::{en_texte, Jour};
use socle::fichiers::{lire_json, vrai};
use socle::num::Resultat;

use crate::contexte::{fiche_au, toutes_les_valeurs, Contexte, ORGANISMES};

/// Audiens, delegataire des contrats de branche du spectacle (S21.G00.15.002).
const AUDIENS: &str = "P0983";
/// Le premier identifiant d'adhesion libre : 100 a 103 sont ceux que `affectations_81_059` nomme.
const PREMIER_LIBRE: u32 = 104;

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

/// `donnees/organismes-base.json`.
pub fn base(ctx: &Contexte) -> Resultat<Value> {
    lire_json(&ctx.code.join("organismes-base.json"))
}

/// Ce que la compagnie a saisi, a la date du contexte (`{"adhesions_prevoyance": [...]}`, ou `{}` si rien), et les
/// identifiants d'adhesion qu'une periode quelconque du fichier porte. Le fichier ne porte que ce qu'aucun texte ne
/// donne : de lui ne se lisent que les adhesions, et une adhesion qui recopie un contrat qu'un texte donne entier
/// (`donnes_par_un_texte`) n'est pas une saisie, sous quelque convention qu'on lise. Son identifiant reste pris : il a
/// pu partir dans une DSN.
pub fn saisis(ctx: &Contexte) -> Resultat<(Value, Vec<u32>)> {
    let p = ctx.racine.join("config").join(ORGANISMES);
    if !p.is_file() {
        return Ok((json!({}), Vec::new()));
    }
    let brut = lire_json(&p)?;
    let prises = toutes_les_valeurs(brut.get("adhesions_prevoyance"))
        .into_iter()
        .flat_map(liste)
        .filter_map(|a| txt(g(&a, "reference")).parse::<u32>().ok())
        .collect();
    let au = fiche_au(&brut, &ctx.lue_au, &format!("config/{}", ORGANISMES))?;
    let textes = donnes_par_un_texte(ctx)?;
    let adhesions: Vec<Value> = liste(g(&au, "adhesions_prevoyance"))
        .into_iter()
        .filter(|a| {
            let libelle = txt(g(a, "libelle")).to_uppercase();
            let contribution = txt(g(a, "contribution")).to_uppercase();
            !textes.contains(&libelle) && !(!contribution.is_empty() && libelle == contribution)
        })
        .collect();
    if adhesions.is_empty() {
        return Ok((json!({}), prises));
    }
    Ok((json!({ "adhesions_prevoyance": adhesions }), prises))
}

/// Les references de contrat (S21.G00.15.001) qu'un texte livre donne entieres, toutes conventions confondues : les
/// `libelle` des contrats de `organismes-base.json` et celui de la CASC-SVP (`casc-svp-base.json`). Une contribution de
/// branche se reconnait a son nom (`saisis`).
fn donnes_par_un_texte(ctx: &Contexte) -> Resultat<Vec<String>> {
    let base = base(ctx)?;
    let mut out: Vec<String> = g(&base, "contrats")
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .flat_map(liste)
        .map(|c| txt(g(&c, "libelle")).to_uppercase())
        .filter(|l| !l.is_empty())
        .collect();
    let casc = lire_json(&ctx.code.join("casc-svp-base.json"))?;
    let l = txt(g(g(&casc, "dsn"), "contrat")).to_uppercase();
    if !l.is_empty() {
        out.push(l);
    }
    Ok(out)
}

/// Les contrats que cette compagnie declare a la date du contexte, dans l'ordre ou ils se presentent.
pub fn attendus(ctx: &Contexte, base: &Value) -> Resultat<Vec<Value>> {
    let ent = &ctx.entreprise;
    let idcc = txt(g(ent, "idcc"));
    let contrats = g(base, "contrats");
    let mut out = liste(g(contrats, "tous"));
    if let Some(casc) = crate::adhesions::casc_svp(&ctx.code, ent)? {
        out.push(json!({
            "cle": "casc", "libelle": casc["libelle"], "reference": casc["reference"], "type": casc["type"],
            "beneficiaires": "tous les salariés", "quoi": "CASC-SVP (activités sociales et culturelles)",
            "organisme": casc["delegataire"], "organisme_fixe": true,
        }));
    }
    // Les contributions de branche en DSN : le bareme de la date dit lesquelles. Un bareme illisible (taux AT/MP
    // manquant...) se signale ailleurs (`pour-demarrer`) ; ici, il ne fait que taire ces lignes.
    if let Ok(bar) = ctx.bareme(&ctx.lue_au) {
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

/// L'adhesion saisie qui tient ce contrat : par sa cle, son identifiant fixe, ou sa contribution.
fn en_place<'a>(spec: &Value, saisi: &'a Value) -> Option<&'a Value> {
    let adh = saisi.get("adhesions_prevoyance")?.as_array()?;
    let cle = txt(g(spec, "cle"));
    if let Some(a) = adh.iter().find(|a| txt(g(a, "cle")) == cle) {
        return Some(a);
    }
    let reference = txt(g(spec, "reference"));
    if !reference.is_empty() {
        return adh.iter().find(|a| txt(g(a, "reference")) == reference);
    }
    let contribution = txt(g(spec, "contribution")).to_uppercase();
    if contribution.is_empty() {
        return None;
    }
    adh.iter().find(|a| txt(g(a, "contribution")).to_uppercase() == contribution || txt(g(a, "libelle")).to_uppercase() == contribution)
}

/// Une ligne de l'ecran : ce que le contrat couvre, et ce qui vaut (le texte quand il le donne, sinon la saisie).
pub fn ligne(spec: &Value, saisi: &Value) -> Value {
    let place = en_place(spec, saisi);
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
        "note": txt(g(spec, "_organisme")),
    })
}

/// Le contrat vaut-il pour la periode (`adhesions::en_vigueur`) ?
pub fn couvre(spec: &Value, (debut, fin): &(Jour, Jour)) -> Resultat<bool> {
    crate::adhesions::en_vigueur(spec, debut, fin)
}

/// Compose les organismes a la date du contexte depuis la base, les contrats attendus, ce que la compagnie a saisi
/// (`saisi` : le fichier a cette date ; `prises` : les identifiants que le fichier porte) et ce qu'elle saisit
/// maintenant (`saisies` : cle -> {libelle, organisme}). Chaque contrat garde ses dates (`depuis`, `jusqu_a`) : la DSN
/// de chaque mois ne lit que ceux qui le couvrent.
///
/// `exiger` : la periode que l'ecran enregistre ; un contrat qu'il propose pour elle et dont la reference manque se
/// refuse. Sans elle (la lecture), un contrat sans reference ne se declare pas, et ce qui en depend se refuse plus loin.
pub fn composer(
    ctx: &Contexte,
    saisi: &Value,
    mut prises: Vec<u32>,
    saisies: &Map<String, Value>,
    exiger: Option<&(Jour, Jour)>,
) -> Resultat<Value> {
    let base = base(ctx)?;
    let mut retenus: Vec<(Value, Value)> = Vec::new();
    for spec in attendus(ctx, &base)? {
        let l = ligne(&spec, saisi);
        let maintenant = saisies.get(&txt(g(&spec, "cle")));
        let libelle = match maintenant {
            Some(s) if !vrai(g(&l, "libelle_fixe")) => txt(g(s, "libelle")).to_uppercase(),
            _ => txt(g(&l, "libelle")),
        };
        let organisme = match maintenant {
            Some(s) if !vrai(g(&l, "organisme_fixe")) => txt(g(s, "organisme")).to_uppercase(),
            _ => txt(g(&l, "organisme")),
        };
        let quoi = txt(g(&spec, "quoi"));
        if libelle.is_empty() {
            // Un contrat vide ne se reclame pas quand l'ecran ne l'a pas propose : un contrat des permanents a une
            // compagnie qui n'en emploie pas, un contrat que la periode regardee ne couvre pas.
            let Some(periode) = exiger else { continue };
            if (vrai(g(&spec, "si_permanents")) && !ctx.emploie_des_permanents()) || !couvre(&spec, periode)? {
                continue;
            }
            return Err(format!(
                "{} : la référence du contrat manque. Elle se lit sur la fiche de paramétrage DSN de l'organisme \
(« référence contrat »).",
                quoi
            ));
        }
        // Les longueurs du cahier technique : S21.G00.15.001 de 1 a 30, S21.G00.15.002 de 5 a 9.
        if libelle.chars().count() > 30 {
            return Err(format!("{} : « {} » dépasse 30 caractères, ce que la DSN n'admet pas.", quoi, libelle));
        }
        let n = organisme.chars().count();
        if !(5..=9).contains(&n) || organisme.contains(char::is_whitespace) {
            return Err(format!(
                "{} : le code de l'organisme ({}) a de 5 à 9 caractères sans espace, comme P0983 pour Audiens. Il est \
sur le certificat d'adhésion et la fiche de paramétrage DSN.",
                quoi,
                if organisme.is_empty() { "vide".to_string() } else { format!("« {} »", organisme) }
            ));
        }
        let reference = txt(g(&spec, "reference"));
        let reference = if reference.is_empty() { en_place(&spec, saisi).map(|a| txt(g(a, "reference"))).unwrap_or_default() } else { reference };
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
    // Les identifiants nouveaux, a la suite des pris : ceux deja attribues ne bougent pas.
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
        _ => return Err("donnees/organismes-base.json : un objet est attendu.".into()),
    };
    out.shift_remove("contrats");
    out.insert("adhesions_prevoyance".into(), Value::Array(adhesions));
    out.insert("affiliations_par_nature".into(), json!({"intermittent": {"references": intermittent}, "permanent": permanent}));
    Ok(Value::Object(out))
}
