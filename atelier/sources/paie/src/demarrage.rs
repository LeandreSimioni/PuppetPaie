//! POUR DEMARRER (Leandre, 09/10/2026 : « regarde si tu aurais aime des avertissements, et ou configurer, avec les
//! endroits ou les trouver pour les cies »).
//!
//! Une compagnie neuve decouvrait ce qui lui manquait un refus apres l'autre, au moment d'agir : le mois refuse (le taux
//! AT/MP), puis encore (l'effectif), la DPAE refusee (la medecine du travail), le bulletin (le compte Urssaf)... La fiche,
//! elle, ne disait rien. Ici, tout ce qui manque, d'un coup : ce que chaque manque BLOQUE, OU il se saisit, et ou la
//! compagnie TROUVE la valeur (l'aide du champ, `champs-base.json`).
//!
//! Les regles ne sont pas recopiees : le bareme de l'annee se demande au contexte (`Contexte::bareme` : le taux AT/MP,
//! l'effectif, la premiere embauche), l'employeur de la DPAE a la DPAE (`dpae::employeur_pret`). La table `FICHE` ne fait
//! que nommer les champs que la DSN et le bulletin lisent sans valeur par defaut (`dsn.rs`, en-tete S10/S20 et bloc 40 ;
//! `bulletin.rs`) : un champ qu'on y ajoute sans qu'un document le lise serait un faux manque.

use serde_json::{json, Value};

use crate::contexte::Contexte;
use crate::fichiers::vrai;
use crate::retours::Echec;

type R<T> = Result<T, Echec>;

pub const ROUTE: &str = "pour-demarrer";

const ENTREPRISE: &str = "Configuration › Entreprise";

/// (chemin dans la fiche entreprise, ce que son absence bloque).
const FICHE: [(&str, &str); 20] = [
    ("raison_sociale", "la DSN"),
    ("siren", "la DSN"),
    ("nic", "la DSN"),
    ("siret", "la DSN et la DPAE"),
    ("naf", "la DSN et la DPAE"),
    ("adresse", "la DSN et la DPAE"),
    ("code_postal", "la DSN et la DPAE"),
    ("commune", "la DSN et la DPAE"),
    ("idcc", "toute paie"),
    ("urssaf_compte", "les bulletins"),
    ("signataire.nom", "la DSN et les contrats"),
    ("signataire.prenom", "la DSN et les contrats"),
    ("declarant_dsn.civilite", "la DSN"),
    ("declarant_dsn.nom_prenom", "la DSN"),
    ("declarant_dsn.tel", "la DSN"),
    ("declarant_dsn.email", "la DSN"),
    ("declarant_dsn.qualite", "la DSN"),
    ("banque.iban", "la DSN (le paiement des cotisations)"),
    ("banque.bic", "la DSN (le paiement des cotisations)"),
    ("congesspectacles_affiliation", "la DSN des intermittents"),
];

/// Le nom en clair d'une case d'un champ a plusieurs cases.
fn en_clair(sous: &str) -> &str {
    match sous {
        "civilite" => "civilité",
        "nom_prenom" => "nom et prénom",
        "nom" => "nom",
        "prenom" => "prénom",
        "tel" => "téléphone",
        "email" => "courriel",
        "qualite" => "qualité",
        "iban" => "IBAN",
        "bic" => "BIC",
        autre => autre,
    }
}

fn valeur<'a>(ent: &'a Value, chemin: &str) -> Option<&'a Value> {
    chemin.split('.').try_fold(ent, |v, k| v.get(k))
}

/// Le libelle et l'aide du champ (`champs-base.json`), le premier segment du chemin.
fn champ(cle: &str) -> (String, String) {
    let racine = cle.split('.').next().unwrap_or(cle);
    let c = crate::champs::fiche("entreprise").ok().and_then(|v| v.into_iter().find(|c| c.cle == racine));
    match c {
        Some(c) => {
            let precision = match cle.split_once('.') {
                Some((_, sous)) => format!(" ({})", en_clair(sous)),
                None => String::new(),
            };
            (format!("{}{}", c.libelle, precision), c.aide)
        }
        None => (cle.to_string(), String::new()),
    }
}

fn manque(cle: &str, libelle: String, bloque: &str, ou: &str, champ: Option<&str>, trouver: String) -> Value {
    json!({ "cle": cle, "libelle": libelle, "bloque": bloque, "ou": ou, "champ": champ, "trouver": trouver })
}

/// La compagnie emploie-t-elle des intermittents ? Un spectacle actif le dit : un CDDU se rattache a un spectacle.
fn a_des_spectacles(ctx: &Contexte) -> bool {
    std::fs::read(ctx.racine.join("config").join("spectacles.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().map(|o| o.iter().any(|(k, s)| !k.starts_with('_') && s.get("actif") != Some(&json!(false)))))
        .unwrap_or(false)
}

/// Une suspension saisie (un imprevu du bloc 65) dont le motif n'est pas remunere. Un magasin illisible ne dit rien ici :
/// la paie le refusera elle-meme.
fn une_suspension_non_remuneree(ctx: &Contexte) -> bool {
    crate::imprevus::tous(&ctx.racine).unwrap_or_default().iter().any(|imp| {
        crate::imprevus::est_une_suspension(imp)
            && imp
                .get("motif")
                .map(crate::date::en_texte)
                .and_then(|m| crate::imprevus::motif_de_suspension(&m).ok().flatten())
                .is_some_and(|m| !m.get("remunere").is_some_and(vrai))
    })
}

/// Tout ce qui manque a la compagnie pour ses premiers actes. Vide : rien ne manque.
pub fn manques(ctx: &Contexte) -> Vec<Value> {
    let ent = &ctx.entreprise;
    let mut out: Vec<Value> = Vec::new();
    let intermittents = a_des_spectacles(ctx);
    // Un champ a plusieurs cases (le declarant : nom, telephone, courriel, qualite) fait UNE ligne, qui nomme ce qui manque.
    let mut vus: Vec<String> = Vec::new();
    for (cle, bloque) in FICHE {
        if cle == "congesspectacles_affiliation" && !intermittents {
            continue;
        }
        if valeur(ent, cle).is_some_and(vrai) {
            continue;
        }
        let racine = cle.split('.').next().unwrap_or(cle);
        let sous = cle.split_once('.').map(|(_, s)| en_clair(s).to_string());
        if let Some(i) = vus.iter().position(|r| r == racine) {
            if let (Some(sous), Some(l)) = (sous, out[i]["libelle"].as_str().map(str::to_string)) {
                let l = match l.strip_suffix(')') {
                    Some(debut) => format!("{}, {})", debut, sous),
                    None => format!("{} ({})", l, sous),
                };
                out[i]["libelle"] = json!(l);
            }
            continue;
        }
        let (libelle, aide) = champ(cle);
        vus.push(racine.to_string());
        out.push(manque(cle, libelle, bloque, ENTREPRISE, Some(racine), aide));
    }
    // La licence d'entrepreneur de spectacles : la DSN l'ecrit dans chaque contrat (S21.G00.40.049) et refuse sans elle.
    // Elle peut etre datee (une par periode) : le contexte la resout au jour.
    match ctx.licence(&ctx.aujourdhui) {
        Ok(l) if !l.trim().is_empty() => {}
        autre => {
            let (libelle, aide) = champ("licence_spectacle");
            let trouver = match autre {
                Err(m) => m,
                Ok(_) => aide,
            };
            out.push(manque("licence_spectacle", libelle, "la DSN", ENTREPRISE, Some("licence_spectacle"), trouver));
        }
    }
    // L'effectif, puis le reste du bareme de l'annee : le contexte refuse lui-meme, avec la phrase qui dit ou trouver ce
    // qui manque. L'effectif a part : le bareme s'arrete au premier refus (le taux AT/MP), et tout doit sortir d'un coup.
    let an = ctx.aujourdhui.an;
    if let Err(m) = ctx.effectif().and_then(|e| e.atteint(11, an).map(|_| ())) {
        let (libelle, _) = champ("premiere_embauche");
        out.push(manque("premiere_embauche", libelle, "toute paie", ENTREPRISE, Some("premiere_embauche"), m));
    }
    if let Err(m) = ctx.bareme(&ctx.aujourdhui) {
        if m.contains("AT/MP") {
            out.push(manque("taux_atmp", format!("Le taux AT/MP {}", an), "toute paie", ENTREPRISE, Some("taux_atmp"), m));
        } else if !m.contains("Effectif") {
            out.push(manque("bareme", format!("Le barème {}", an), "toute paie", ENTREPRISE, None, m));
        }
    }
    // L'employeur de la DPAE : la DPAE elle-meme dit ce qui lui manque.
    if let Err(m) = crate::dpae::employeur_pret(ent) {
        let (libelle, aide) = champ("medecine_travail");
        if m.contains("medecine") {
            out.push(manque("medecine_travail", libelle, "la DPAE", ENTREPRISE, Some("medecine_travail"), aide));
        } else {
            out.push(manque("dpae", "L'employeur de la DPAE".into(), "la DPAE", ENTREPRISE, None, m));
        }
    }
    // La sante d'un permanent suspendu sans indemnisation tout un mois : hors de la 3090, aucun texte ne la dit, et le mois
    // refuse tant que la fiche ne repond pas (`calcul::sante_de_la_suspension`). Elle ne se demande que si une suspension
    // non remuneree est saisie : un champ n'apparait que parce qu'un fait l'exige.
    let idcc = ent.get("idcc").map(crate::date::en_texte).unwrap_or_default();
    if idcc != "3090" && !valeur(ent, crate::calcul::SANTE_SUSPENSION).is_some_and(vrai) && une_suspension_non_remuneree(ctx) {
        let cle = crate::calcul::SANTE_SUSPENSION;
        let (libelle, aide) = champ(cle);
        out.push(manque(cle, libelle, "le mois d'un permanent suspendu sans indemnisation tout le mois", ENTREPRISE, Some(cle), aide));
    }
    // Les organismes de la DSN : aucun ecran ne les ecrit encore.
    if !ctx.racine.join("config").join("organismes.json").is_file() {
        out.push(manque(
            "organismes",
            "Les organismes et les contrats de la DSN".into(),
            "la DSN, donc l'écriture du mois",
            "Configuration › Organismes et contrats",
            None,
            "Les comptes que la DSN paie (Urssaf, France Travail Spectacle, Congés Spectacles, retraite complémentaire) et vos contrats de prévoyance et de santé : l'organisme, la référence du contrat et la population couverte se lisent sur le certificat d'adhésion de l'assureur et sur sa fiche de paramétrage DSN.".into(),
        ));
    }
    out
}

/// `pour-demarrer` : `{"manques": [...]}`.
pub fn route(ctx: &Contexte) -> R<Value> {
    Ok(json!({ "manques": manques(ctx) }))
}

/// Ce qui suit le refus d'un acte quand la compagnie n'a pas fini de demarrer : toute la liste, d'un coup.
pub fn a_la_suite(ctx: &Contexte) -> Option<String> {
    let m = manques(ctx);
    if m.is_empty() {
        return None;
    }
    let lignes: Vec<String> = m
        .iter()
        .map(|x| format!("  - {} : bloque {} ({})", x["libelle"].as_str().unwrap_or(""), x["bloque"].as_str().unwrap_or(""), x["ou"].as_str().unwrap_or("")))
        .collect();
    Some(format!("\n\nPour démarrer, il manque encore :\n{}", lignes.join("\n")))
}
