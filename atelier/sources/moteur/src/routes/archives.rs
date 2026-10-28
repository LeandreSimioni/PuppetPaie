//! Les routes de l'Historique, des exports et de la cloture : ce qui reste d'une annee de paies (le dossier d'un
//! salarie, les exports et leur logiciel comptable, les corrections, le cout de l'annee, la revue des spectacles, la
//! cloture), et ce qui part d'elles (le dossier par courriel, la regularisation Conges Spectacles).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::depot_acte::{verifier_preuve, Preuve};
use crate::routes::documents::{client_courriel, expediteur_de, preuve_de, qui_de, refus, requis, vrai_ou, Ouvrir};
use crate::routes::engagement::{booleen, maintenant};
use crate::routes::ouverture::marquer_echeance;
use crate::routes::lire_json_ou_vide;
use crate::routes::etapes::Moteur;
use crate::routes::fiches::entier;
use crate::routes::ouverture::mois_connus;
use documents::plan_comptable::{controler_plan, tables};
use droit::contexte::Contexte;
use envois::envoi::{self, Message};
use envois::etats::{self, get, getd, items, ou, texte_vide};
use envois::ligne_d_envoi::{echec_d_envoi, noter, noter_echec_d_envoi};
use socle::date::jour_iso;
use socle::echec::{erreur, Echec, R};
use socle::ecriture::ecrire_json;
use socle::fichiers::lire_json_echec as lire_json;
use socle::fichiers::{nom_du, repr, vrai};
use socle::html::courriel_en_html;
use socle::valeur::{nom_type, valeur_en_texte, valeurs_egales};



/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "exercices", "dossier-formes", "exports-periodes", "regularisations", "couts",
    "revue-spectacles", "cloture", "export-compta", "retenir-logiciel", "casc-regularise", "exporter", "dossier-envoyer",
    "casc-envoyer",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &[
    "couts", "revue-spectacles", "cloture", "export-compta", "retenir-logiciel", "casc-regularise", "exporter",
    "dossier-envoyer", "casc-envoyer",
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "exercices" => exercices_route(ctx),
        "dossier-formes" => dossier_formes(ctx),
        "exports-periodes" => exports_periodes(ctx, charge),
        "regularisations" => regularisations(ctx),
        "couts" => couts(o, ctx, charge),
        "revue-spectacles" => revue_spectacles(o, ctx, charge),
        "cloture" => crate::exercice_ecrit::cloture_route(ctx, charge),
        "export-compta" => crate::exercice_ecrit::export_compta_route(ctx, charge),
        "retenir-logiciel" => retenir_logiciel(ctx, charge),
        "casc-regularise" => casc_regularise(ctx, charge),
        "exporter" => exporter(ctx, charge),
        "dossier-envoyer" => dossier_envoyer(ctx, charge, &mut || client_courriel(ctx)),
        "casc-envoyer" => casc_envoyer(ctx, charge, &mut || client_courriel(ctx)),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

/// `operations_etats.couts_de_l_annee(ctx, charge["annee"], ctx.aujourdhui)`.
fn couts(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let _ = o;
    let annee = valeur_en_texte(exiger(charge, "annee")?);
    let mut r = documents::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(annee), false);
    r.couts_de_l_annee().map_err(|e| Echec { classe: e.classe, message: e.message })
}

/// `ctx.racine / "sorties" / mois` : `mois` doit etre un texte (`TypeError` sinon,
/// avec la phrase de `PosixPath.__truediv__`).
fn sous_sorties(ctx: &Contexte, mois: &Value) -> R<std::path::PathBuf> {
    match mois {
        Value::String(m) => Ok(ctx.racine.join("sorties").join(m)),
        autre => Err(erreur(
            "TypeError",
            format!("'mois' : {}", socle::fichiers::attendu("un texte", nom_type(autre))),
        )),
    }
}

/// `sorted(d.glob("*.pdf"))` : les noms (sensible a la casse, fichiers caches compris),
/// en ordre de points de code.
fn pdf_de(d: &Path) -> R<Vec<String>> {
    let mut noms = Vec::new();
    // `Path.glob` ne leve rien pour ce qui n'est pas un dossier ni pour un dossier
    // interdit : elle ne rend rien.
    let lecture = match std::fs::read_dir(d) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(20) => return Ok(noms),
        Err(e) => return Err(erreur_io(&e, d)),
    };
    for e in lecture {
        let e = e.map_err(|e| erreur_io(&e, d))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".pdf") {
            noms.push(n);
        }
    }
    noms.sort();
    Ok(noms)
}

/// `documents_du_mois` : les fichiers du mois a ouvrir ou a remettre a la main.
pub(crate) fn documents_du_mois(ctx: &Contexte, mois: &Value, quoi: &Value) -> R<Value> {
    let base = sous_sorties(ctx, mois)?;
    let envoi = base.join("envoi");
    // `{...}.get(quoi)` : un texte seulement se trouve ; une liste ou un dictionnaire ne se hache pas.
    let prefixes: Option<&[&str]> = match quoi {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(q) if q == "contrats" => Some(&["Contrat_", "Contrats_"]),
        Value::String(q) if q == "documents" => Some(&["Bulletin_", "CertificatCS_", "AER_"]),
        _ => None,
    };
    let mut out = Vec::new();
    for (d, est_base) in [(&envoi, false), (&base, true)] {
        if !d.exists() {
            continue;
        }
        for nom in pdf_de(d)? {
            // Le PDF a imprimer reunit les pieces de plusieurs salaries : chacune se liste seule, lui jamais.
            if nom.starts_with(envois::envois_signature::PREFIXE_IMPRESSION) {
                continue;
            }
            if let Some(p) = prefixes {
                if !p.iter().any(|x| nom.starts_with(x)) {
                    continue;
                }
            }
            if est_base && vrai(quoi) {
                continue; // les fichiers groupes du mois : pas ici
            }
            let chemin = d.join(&nom);
            let st = std::fs::metadata(&chemin).map_err(|e| erreur_io(&e, &chemin))?;
            let stem = nom.strip_suffix(".pdf").filter(|s| !s.is_empty()).unwrap_or(&nom);
            let morceaux: Vec<&str> = stem.split('_').collect();
            let salarie = if morceaux.len() > 2 { morceaux[1] } else { "" };
            out.push(json!({"nom": nom, "octets": st.len(), "salarie": salarie}));
        }
    }
    Ok(Value::Array(out))
}

/// Par mois d'origine, les corrections que la page montre (`crate::regularisation::corrections_du_mois`) : celles a
/// porter (`etat` « à porter ») et celles deja portees par la DSN d'un mois suivant (« portée »).
fn corrections_par_mois(ctx: &Contexte) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for mois in crate::routes::etapes::mois_saisis(&ctx.racine) {
        let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, &mois)?;
        let mut corrections: Vec<Value> = portees
            .into_iter()
            .map(|mut c| {
                c["etat"] = json!("portée");
                c
            })
            .collect();
        corrections.extend(a_porter.into_iter().map(|mut c| {
            c["etat"] = json!("à porter");
            c
        }));
        if !corrections.is_empty() {
            out.push(json!({"mois": mois, "corrections": corrections}));
        }
    }
    Ok(out)
}

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Value> {
    let e = m.dsn_du_mois(mois)?;
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let passee = ctx.aujourdhui > limite;
    let deposee = vrai(&e);
    let drapeau = |k: &str| -> R<bool> { Ok(deposee && vrai(get(&e, k)?)) };
    let (partie_passee, refusee) = (drapeau("passee")?, drapeau("refusee")?);
    // `_remplacement.limite` : la veille de l'echeance (reportee), a minuit.
    let veille = echeance.veille().map_err(plain)?;
    let mut regularisable = false;
    let raison = if !deposee {
        let retard = if passee { format!(" (en retard : `moteur depot-dsn {} --test`, puis --reel)", mois) } else { String::new() };
        format!("Aucune DSN reelle n'est partie pour {} : ce mois se saisit et se depose normalement{}.", mois, retard)
    } else if refusee {
        format!("La DSN de {} a ete REFUSEE : elle se corrige et se redepose, il n'y a rien a regulariser.", mois)
    } else if ctx.aujourdhui <= veille {
        format!(
            "La DSN de {} est partie et l'annule-et-remplace est admis jusqu'au {} a minuit : modifier le mois et refaire la DSN, \
             elle partira en annule-et-remplace.",
            mois,
            veille.jj_mm_aaaa()
        )
    } else {
        regularisable = true;
        format!(
            "La DSN de {} est deposee et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance du {}) : \
             une correction part dans la DSN du mois courant, datee de {}.",
            mois,
            veille.jj_mm_aaaa(),
            limite.jj_mm_aaaa(),
            mois
        )
    };
    Ok(json!({
        "mois": mois, "deposee": deposee, "passee": partie_passee, "refusee": refusee,
        "echeance": limite.iso(), "echeance_passee": passee, "echeance_depot": echeance.en_json(),
        "mois_courant": ctx.aujourdhui.mois_iso(),
        "regularisable": regularisable, "raison": raison,
    }))
}

/// `mois_a_corriger` : les mois qui appellent une decision.
fn mois_a_corriger(ctx: &Contexte) -> R<Value> {
    let mut m = Moteur::nouveau(ctx);
    let mut etats = Vec::new();
    for mois in crate::routes::etapes::mois_saisis(&ctx.racine) {
        etats.push(etat_mois(&mut m, ctx, &mois)?);
    }
    let choisis = |f: &dyn Fn(&Value) -> bool| -> Vec<Value> { etats.iter().filter(|e| f(e)).cloned().collect() };
    Ok(json!({
        "regularisables": choisis(&|e| e["regularisable"] == json!(true)),
        "en_retard": choisis(&|e| e["echeance_passee"] == json!(true) && e["deposee"] == json!(false)),
    }))
}

/// La route `regularisations`.
fn regularisations(ctx: &Contexte) -> R<Value> {
    let mois = mois_a_corriger(ctx)?;
    Ok(json!({"mois": mois, "corrections": corrections_par_mois(ctx)?}))
}

/// `charge["cle"]`.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

// -------------------------------------------------------------- arret-contrats





// -------------------------------------------------------------- dossier-formes

/// Les mois qu'il y a matiere a exporter : ceux que des engagements touchent, du plus ancien au plus recent, chacun
/// marque `saisie` (ses paies se lisent dans ses engagements valides).
pub(crate) fn mois_disponibles(ctx: &Contexte) -> R<Vec<(String, &'static str)>> {
    let mut mois: Vec<(String, &'static str)> = crate::routes::mois::mois_saisis(ctx)?.into_iter().map(|m| (m, "saisie")).collect();
    mois.sort();
    mois.dedup();
    Ok(mois)
}

/// `sorted({m[:4] for m in mois}, reverse=True)`.
fn annees_de(dispo: &[(String, &'static str)]) -> Vec<String> {
    let mut annees: Vec<String> = dispo.iter().map(|(m, _)| m.chars().take(4).collect::<String>()).collect::<HashSet<_>>().into_iter().collect();
    annees.sort();
    annees.reverse();
    annees
}

/// Les salaries que la page propose.
fn salaries_de_la_page(ctx: &Contexte) -> Vec<Value> {
    let mut fiches: Vec<(&String, &Value)> = ctx.salaries.iter().filter(|(k, v)| !k.starts_with('_') && v.is_object()).collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    fiches
        .into_iter()
        .map(|(k, v)| {
            let nom = format!(
                "{} {}",
                valeur_en_texte(v.get("nom").unwrap_or(texte_vide())),
                valeur_en_texte(ou(v.get("prenom_affichage").unwrap_or(&Value::Null), v.get("prenom").unwrap_or(texte_vide())))
            );
            json!({"cle": k, "nom": nom, "actif": v.get("actif").map_or(true, vrai), "email": v.get("email").is_some_and(vrai)})
        })
        .collect()
}

/// `operations_dossier.formes_dossier` : ce que la page propose -- les salaries,
/// les categories a cocher, les periodes, et si l'envoi est possible.
fn dossier_formes(ctx: &Contexte) -> R<Value> {
    let salaries = salaries_de_la_page(ctx);
    // `periodes_exportables` : les mois, puis (sans rien en dire) le logiciel du comptable.
    let dispo = mois_disponibles(ctx)?;
    let mois: Vec<&String> = dispo.iter().map(|(m, _)| m).collect();
    let annees = annees_de(&dispo);
    etat_logiciel(ctx)?;
    let boite = crate::routes::ouverture::boite_liee(ctx)?;
    Ok(json!({
        "salaries": salaries,
        "categories": [
            {"categorie": "bulletins", "quoi": "Bulletins de paie"},
            {"categorie": "contrats", "quoi": "Contrats"},
            {"categorie": "contrats-signes", "quoi": "Contrats signés"},
            {"categorie": "certificats-cs", "quoi": "Certificats Congés Spectacles"},
            {"categorie": "attestations", "quoi": "Attestations employeur (AER, AEM)"},
            {"categorie": "documents-fin", "quoi": "Documents de fin de contrat"},
            {"categorie": "recapitulatif", "quoi": "Fiche récapitulative"},
        ],
        "mois": mois,
        "annees": annees,
        "boite_liee": boite,
        "dossier": ctx.racine.join("exports").join("dossiers").display().to_string(),
    }))
}

// ------------------------------------------------- le logiciel du comptable

/// `operations_ecritures.etat_logiciel` : ce que la page montre -- les logiciels,
/// celui retenu (`config/compta.json`), et ce que le plan comptable ne tiendrait
/// pas dans son format.
pub(crate) fn etat_logiciel(ctx: &Contexte) -> R<Value> {
    let chemin = ctx.racine.join("config").join("compta.json");
    let choix = if chemin.exists() { lire_json(&chemin)? } else { json!({}) };
    let retenu = getd(&choix, "logiciel", &Value::Null)?;
    let retenu = ou(retenu, texte_vide());
    let analytique = vrai(getd(&choix, "analytique", &Value::Null)?);
    let controle = match retenu {
        Value::Array(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => return Err(erreur("TypeError", socle::fichiers::pas_une_cle("dict"))),
        Value::String(nom) => {
            let logiciels = tables()["logiciels"].as_array().expect("logiciels");
            match logiciels.iter().find(|l| l["logiciel"].as_str() == Some(nom)) {
                Some(l) => controler_plan(ctx, l["format"].as_str().expect("format"))?,
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    Ok(json!({"logiciels": tables()["logiciels"], "logiciel": retenu, "analytique": analytique, "controle": controle}))
}

/// `operations_exports._etat_concerne` : la page ne propose pas la CASC-SVP de la
/// 3090 a une compagnie de la 1285, ni la dette de conges a qui n'emploie pas de
/// permanents.
fn etat_concerne(ctx: &Contexte, etat: &str) -> bool {
    let idcc = valeur_en_texte(ou(ctx.entreprise.get("idcc").unwrap_or(&Value::Null), texte_vide()));
    let t = tables();
    if let Some(liste) = t["si_convention"].get(etat) {
        if !idcc.is_empty() && !liste.as_array().expect("liste").iter().any(|c| c.as_str() == Some(idcc.as_str())) {
            return false;
        }
    }
    if t["si_permanents"].as_array().expect("permanents").iter().any(|e| e.as_str() == Some(etat)) {
        return ctx.emploie_des_permanents();
    }
    true
}

/// `operations_exports.periodes_exportables` : les mois et les annees qu'il y a
/// matiere a exporter, les etats, les formats, et le logiciel du comptable. `xlsx`
/// (`import openpyxl` reussit-il ?) et `pdf` (la fabrique est-elle installee ?)
/// sont deux faits de l'interprete et du poste, que `web.py` dit dans la charge.
fn exports_periodes(ctx: &Contexte, charge: &Value) -> R<Value> {
    let dispo = mois_disponibles(ctx)?;
    let t = tables();
    let etats: Vec<&Value> = t["etats"].as_array().expect("etats").iter().filter(|e| etat_concerne(ctx, e["etat"].as_str().expect("etat"))).collect();
    let annees = annees_de(&dispo);
    let compta = etat_logiciel(ctx)?;
    let fait = |k: &str| charge.get(k).is_some_and(vrai);
    Ok(json!({
        "mois": dispo.iter().map(|(m, _)| m).collect::<Vec<_>>(),
        "annees": annees,
        "etats": etats,
        "formats": t["formats"],
        "regroupements": t["regroupements"],
        "dossier": ctx.racine.join("exports").display().to_string(),
        "xlsx": fait("xlsx"),
        "pdf": fait("pdf"),
        "compta": compta,
    }))
}

/// `operations_exercice.revue_spectacles` : archive les spectacles actifs qui ne sont pas dans
/// `continuent`, note la revue de l'annee faite, rend la configuration.
fn revue_spectacles(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = entier(exiger(charge, "annee")?)?;
    let continuent = etats::ou(getd(charge, "continuent", &etats::LISTE_VIDE)?, &etats::LISTE_VIDE);
    // `set(continuent)` : l'ordre ne compte pas, les doublons non plus.
    let mut garder: Vec<Value> = Vec::new();
    for c in items(continuent)? {
        if matches!(c.as_ref(), Value::Array(_) | Value::Object(_)) {
            return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(&c))));
        }
        if !garder.iter().any(|g| valeurs_egales(g, &c)) {
            garder.push(c.into_owned());
        }
    }
    let actifs = ctx.spectacles_actifs();
    let est_actif = |v: &Value| v.as_str().is_some_and(|s| actifs.contains(&s));
    let mut inconnus: Vec<&Value> = garder.iter().filter(|g| !est_actif(g)).collect();
    if !inconnus.is_empty() {
        if let Some((i, autre)) = inconnus.iter().enumerate().find(|(_, c)| !c.is_string()) {
            return Err(erreur("TypeError", socle::fichiers::element(i, &socle::fichiers::attendu("un texte", nom_type(autre)))));
        }
        inconnus.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        let noms: Vec<&str> = inconnus.iter().filter_map(|c| c.as_str()).collect();
        return Err(erreur("ErreurOperation", format!("Spectacle actif inconnu : {}.", noms.join(", "))));
    }
    let archives: Vec<String> = actifs.iter().filter(|c| !garder.iter().any(|g| g.as_str() == Some(c))).map(|c| c.to_string()).collect();
    for cle in &archives {
        crate::routes::fiches::activer_fiche(ctx, "spectacle", cle, false)?;
    }
    marquer_echeance(ctx, &format!("revue-spectacles:{}", annee), true, "interface")?;
    let mut continuent: Vec<String> = garder.iter().filter_map(|g| g.as_str().map(String::from)).collect();
    continuent.sort();
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    let configuration = crate::routes::ouverture::route(o, &neuf, "configuration", &json!({}))?;
    Ok(json!({"annee": annee, "archives": archives, "continuent": continuent, "configuration": configuration}))
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

/// `operations_ecritures.retenir_logiciel` : le logiciel du comptable, et l'analytique, retenus une fois pour
/// toutes. Rend le controle du plan pour ce logiciel. Ce n'est pas un export : un reglage, pas une donnee de paie.
fn retenir_logiciel(ctx: &Contexte, charge: &Value) -> R<Value> {
    let logiciel = charge.get("logiciel").filter(|v| vrai(v));
    let analytique = booleen(charge, "analytique", false);
    let nom = logiciel.map(valeur_en_texte).unwrap_or_default();
    if logiciel.is_some() {
        let connus = documents::plan_comptable::logiciels_connus();
        if !connus.iter().any(|l| *l == nom) {
            return Err(refus(format!("Logiciel inconnu : « {} ».", nom)));
        }
    }
    let chemin = ctx.racine.join("config").join("compta.json");
    let contenu = json!({
        "_quoi": "Le logiciel du comptable, retenu pour les écritures comptables (Exports, ou moteur compta-logiciel).",
        "logiciel": nom,
        "analytique": analytique,
    });
    ecrire_json(&chemin, &contenu, 2).map_err(|m| erreur("OSError", m))?;
    crate::routes::archives::etat_logiciel(ctx).map_err(Echec::from)
}

/// `operations_casc.marquer_regularise` : note qu'une annee de CASC-SVP est regularisee (apres le virement).
fn casc_regularise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let annee = crate::routes::fiches::exiger(charge, "annee")?;
    let vire_le = crate::routes::fiches::exiger(charge, "vire_le")?;
    let montant = crate::routes::fiches::exiger(charge, "montant")?;
    let aujourd_hui = maintenant();
    let qui = charge.get("qui").filter(|v| vrai(v)).map(valeur_en_texte).unwrap_or_else(|| "interface".to_string());
    declarations::casc_regularisation::marquer_regularise(ctx, annee, vire_le, montant, &qui, &aujourd_hui[..10])
}

/// `charge[cle] or None` : un texte, ou rien.
fn texte_ou_rien(charge: &Value, cle: &str) -> Option<String> {
    charge.get(cle).filter(|v| vrai(v)).map(valeur_en_texte)
}

/// `operations_exports.exporter` : le verbe unique, `exporter <etat>`. Lit la charge (etat, mois, annee,
/// toutes_les_annees, regroupement, format, coordonnees, apercu, logiciel, analytique), refuse un etat inconnu,
/// ne passe a l'etat que ce que sa forme demande, puis rend l'etat tel que `js.brut` le rend.
fn exporter(ctx: &Contexte, charge: &Value) -> R<Value> {
    Ok(match exporter_objet(ctx, charge)? {
        Exporte::Casc(v) => v,
        Exporte::Livre(l) => l.json(),
        Exporte::Tableau(t) => documents::tableaux_export::brut_tableau(&t),
    })
}

/// Ce qu'un export rend, avant d'etre mis en JSON : la ligne de commande (`moteur exporter`) le rend en texte a partir de l'objet, comme le
/// Python le faisait (le chemin ecrit en entier, les montants en `Decimal`).
pub(crate) enum Exporte {
    Casc(Value),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON. La charge peut porter `format_du_logiciel_retenu` : sans `format`, les ecritures comptables laissent
/// le logiciel retenu (`compta-logiciel`) choisir leur format (la ligne de commande), au lieu du CSV (la page, qui envoie son format).
pub(crate) fn exporter_objet(ctx: &Contexte, charge: &Value) -> R<Exporte> {
    use documents::etats_paie::Demande;
    let etat = texte_ou_rien(charge, "etat").unwrap_or_else(|| "livre-de-paie".to_string());
    let tables = documents::plan_comptable::tables();
    let etats = tables["etats"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let Some(forme) = etats.iter().find(|e| e["etat"].as_str() == Some(etat.as_str())) else {
        let mut noms: Vec<&str> = etats.iter().filter_map(|e| e["etat"].as_str()).collect();
        noms.sort_unstable();
        return Err(refus(format!("État inconnu : « {} ». Les états possibles sont {}.", etat, noms.join(", "))));
    };
    let mut d = Demande {
        mois: texte_ou_rien(charge, "mois"),
        annee: texte_ou_rien(charge, "annee"),
        toutes: charge.get("toutes_les_annees").is_some_and(vrai),
        regroupement: texte_ou_rien(charge, "regroupement").unwrap_or_else(|| "salarie".to_string()),
        format: texte_ou_rien(charge, "format").unwrap_or_else(|| {
            if etat == "ecritures-comptables" && charge.get("format_du_logiciel_retenu").is_some_and(vrai) { String::new() } else { "csv".to_string() }
        }),
        coordonnees: charge.get("coordonnees").is_some_and(vrai),
        ecrire: !charge.get("apercu").is_some_and(vrai),
        logiciel: texte_ou_rien(charge, "logiciel"),
        analytique: charge.get("analytique").map(vrai),
    };
    if etat == "regularisation-casc-svp" {
        let demande = declarations::casc_regularisation::Demande { mois: d.mois.clone(), annee: d.annee.clone(), toutes_les_annees: d.toutes, boite_liee: crate::routes::ouverture::boite_liee(ctx).unwrap_or(false), ..Default::default() };
        let mut r = declarations::casc_regularisation::regularisation(ctx, &demande)?;
        if d.ecrire {
            declarations::casc_regularisation::ecrire(ctx, &mut r)?;
        }
        return Ok(Exporte::Casc(declarations::casc_regularisation::vers_json(&r)));
    }
    if etat == "livre-de-paie" {
        return Ok(Exporte::Livre(documents::etats_paie::livre_de_paie(ctx, &d)?));
    }
    // Ce que l'etat ne demande pas ne lui est pas passe : un regroupement par defaut (« salarie ») n'a pas de
    // sens pour les cotisations.
    let questions: Vec<&str> = forme["questions"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !questions.contains(&"regroupement") {
        d.regroupement = String::new();
    } else {
        let permis: Vec<&str> = forme["regroupements"].as_array().map(|q| q.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if !permis.contains(&d.regroupement.as_str()) {
            d.regroupement = permis.first().map(|s| s.to_string()).unwrap_or_default();
        }
    }
    if !questions.contains(&"coordonnees") {
        d.coordonnees = false;
    }
    let tableau = match etat.as_str() {
        "livre-de-paie-detaille" => documents::etats_paie::livre_detaille(ctx, &d),
        "recapitulatif-annuel" => documents::etats_paie::recapitulatif_annuel(ctx, &d),
        "masse-salariale" => documents::etats_paie::masse_salariale(ctx, &d),
        "etat-des-virements" => documents::etats_paie::etat_des_virements(ctx, &d),
        "prelevement-a-la-source" => documents::etats_paie::prelevement_a_la_source(ctx, &d),
        "cotisations-par-organisme" => declarations::etats_organismes::cotisations_par_organisme(ctx, &d),
        "versements-organismes" => declarations::etats_organismes::versements_organismes(ctx, &d, &crate::depart::dsn_du_mois),
        "bordereau-urssaf" => declarations::etats_organismes::bordereau_urssaf(ctx, &d, &crate::depart::dsn_du_mois),
        "fcap-svp" => declarations::etats_organismes::fcap_svp(ctx, &d),
        "afdas-2642" => declarations::etats_organismes::afdas_2642(ctx, &d),
        "paritarisme-branche" => declarations::etats_organismes::paritarisme_branche(ctx, &d),
        "reduction-generale" => documents::etats_paie::reduction_generale(ctx, &d),
        "assiettes" => documents::etats_paie::assiettes(ctx, &d),
        "heures-et-cachets" => documents::etats_paie::heures_et_cachets(ctx, &d),
        "cout-par-spectacle" => documents::etats_paie::cout_par_spectacle(ctx, &d),
        "contrats" => documents::etats_personnel::liste_des_contrats(ctx, &d),
        "registre-du-personnel" => documents::etats_personnel::registre_du_personnel(ctx, &d),
        "fichier-du-personnel" => documents::etats_personnel::fichier_du_personnel(ctx, &d),
        "journal-des-declarations" => documents::etats_personnel::journal_des_declarations(ctx, &d, &envois::journal::lire(&ctx.racine, None, None, false).map_err(|m| erreur("OSError", m))?),
        "ecritures-comptables" => documents::etats_compta::ecritures_comptables(ctx, &d),
        "grand-livre-paie" => documents::etats_compta::grand_livre(ctx, &d),
        "balance-paie" => documents::etats_compta::balance(ctx, &d),
        "provision-conges" => documents::etats_compta::provision_conges(ctx, &d),
        "taxe-sur-les-salaires" => documents::etats_compta::taxe_sur_les_salaires(ctx, &d),
        autre => Err(refus(format!("État inconnu : « {} ».", autre))),
    }?;
    Ok(Exporte::Tableau(tableau))
}

const CORPS_DOSSIER: &str = "Bonjour {prenom},\n\nTu trouveras ci-joint ton dossier de salarié{periode} : {liste}.\n\n{reedition}Bien à toi,\n{signataire}\n{raison_sociale}\n";

/// Ce que le courriel dit des contrats, quand le dossier en porte : eux seuls se reeditent (les bulletins et les
/// certificats sont les pages memes de l'archive, lot V1p ; le reste se prend tel quel).
const REEDITION_CONTRATS: &str = "Les contrats ont été réédités par notre logiciel de paie depuis les données\nd'origine : les numéros et les montants sont ceux des contrats qui t'ont été\nremis ; seule la mise en page peut avoir changé depuis.\n\n";

/// La route `dossier-envoyer` : le dossier se refait ici (`dossier_salarie`, un fichier par salarie, ecrit dans
/// `exports/dossiers/`), puis part a chaque salarie par la boite de la compagnie. La reponse est celle du dossier
/// (`dossier_json`) et, sous `envoi`, ce que l'envoi a fait.
fn dossier_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    // `ops.dossier_salarie(salaries=..., mois=..., annee=..., toutes_les_annees=..., categories=..., ensemble=False, ecrire=True)`.
    let mut demande = Map::new();
    for cle in ["salaries", "mois", "annee", "toutes_les_annees", "categories"] {
        if let Some(v) = charge.get(cle) {
            demande.insert(cle.to_string(), v.clone());
        }
    }
    let d = crate::routes::dossier_salarie::construire(ctx, &Value::Object(demande))?;
    let mut r = crate::routes::dossier_salarie::dossier_json(&d);
    let envoi = envoyer_dossier(ctx, &d, &preuve_de(charge), &qui, ouvrir)?;
    if let Value::Object(o) = &mut r {
        o.insert("envoi".into(), envoi);
    }
    Ok(r)
}

/// `operations_dossier.envoyer_dossier` : envoie a chaque salarie SON dossier, par la boite de la compagnie.
/// Irreversible : la meme preuve que les pieces du mois, le meme journal. Un dossier `ensemble` ne part a personne.
pub(crate) fn envoyer_dossier(ctx: &Contexte, d: &crate::routes::dossier_salarie::Dossier, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<Value> {
    if d.fichiers.iter().any(|f| nom_du(f).starts_with("Dossiers_")) {
        return Err(refus(
            "Un dossier qui réunit tous les salariés ne s'envoie à personne : il porte la paie de chacun. Refaire un fichier par salarié.",
        ));
    }
    if d.fichiers.len() != d.salaries.len() {
        return Err(refus("Le dossier n'a pas été écrit : rien à envoyer."));
    }
    let mut messages: Vec<Message> = Vec::new();
    for (cle, f) in d.salaries.iter().zip(&d.fichiers) {
        let s = ctx.salaries.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))?;
        let Some(email) = s.get("email").filter(|v| vrai(v)).map(valeur_en_texte) else {
            return Err(refus(format!(
                "Pas d'adresse e-mail pour {} {} : compléter sa fiche avant d'envoyer. Rien n'est parti.",
                valeur_en_texte(requis(s, "nom")?),
                valeur_en_texte(requis(s, "prenom")?)
            )));
        };
        let prenom = match vrai_ou(s, "prenom_affichage") {
            Some(p) => p,
            None => valeur_en_texte(requis(s, "prenom")?),
        };
        let signataire = ctx.entreprise.get("signataire").filter(|v| vrai(v)).cloned().unwrap_or_else(|| json!({}));
        let ses_pages = d.pages.get(cle).and_then(Value::as_object);
        let liste = d
            .categories
            .iter()
            .filter(|c| ses_pages.is_some_and(|p| p.contains_key(**c)))
            .map(|c| crate::routes::dossier_salarie::libelle_de(c).to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        let corps = envoi::formater(
            CORPS_DOSSIER,
            &[
                ("prenom", prenom.clone()),
                ("periode", if d.periode == "toute la période" { String::new() } else { format!(" ({})", d.periode) }),
                ("liste", liste),
                ("reedition", if ses_pages.is_some_and(|p| p.contains_key("contrats")) { REEDITION_CONTRATS.to_string() } else { String::new() }),
                ("signataire", vrai_ou(&signataire, "prenom_affichage").or_else(|| signataire.get("prenom").map(valeur_en_texte)).unwrap_or_default()),
                ("raison_sociale", vrai_ou(&ctx.entreprise, "nom_affichage").or_else(|| ctx.entreprise.get("raison_sociale").map(valeur_en_texte)).unwrap_or_default()),
            ],
        );
        messages.push(Message {
            cle: cle.clone(),
            nom: format!("{} {}", prenom, valeur_en_texte(requis(s, "nom")?)),
            destinataire: email.to_lowercase(),
            sujet: format!("Ton dossier de salarié — {}", d.periode),
            corps,
            pieces: vec![f.clone()],
            html: String::new(),
        });
    }
    let mut client = ouvrir()?;
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    verifier_preuve(ctx, Some(preuve), &format!("{:04}-{:02}", aujourdhui.an, aujourdhui.mois), "envoi du dossier aux salaries", &a_qui)?;
    let expediteur = expediteur_de(client.as_ref());
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "dossier-salarie", None, qui, &e.partis, &e.echoue, &e.message, None, None)?;
            return Err(echec_d_envoi(e.message));
        }
    };
    let detail = json!({
        "periode": d.periode,
        "categories": d.categories,
        "salaries": messages.iter().map(|m| json!({"cle": m.cle, "piece": m.pieces.first().map(|p| nom_du(p))})).collect::<Vec<_>>(),
        "expediteur": expediteur,
    });
    let ligne = noter(ctx, "dossier-salarie", None, qui, true, &partis.join(", "), "", Some(&detail))?;
    Ok(json!({"partis": partis, "messages": messages.len(), "expediteur": expediteur, "ligne": ligne}))
}

/// `operations_casc._jjmmaaaa`.
fn jjmmaaaa(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// `operations_casc.envoyer_regularisation` : envoie les deux courriels de la regularisation, APRES les virements.
/// Irreversible. `vire_le` : {annee: AAAA-MM-JJ}, une date par virement fait ; le dossier envoye est celui de ces
/// annees-la, exactement.
///
/// Dans cet ordre : le dossier (ecrit, pour que la liste nominative jointe soit celle du jour), la boite, la
/// preuve, l'envoi, puis ce qui s'en garde -- les deux `.eml` dans `exports/casc-svp/envoyes/`, la ligne de
/// journal, et les annees notees regularisees. Rien de tout cela ne s'ecrit si l'envoi casse.
fn casc_envoyer(ctx: &Contexte, charge: &Value, ouvrir: Ouvrir) -> R<Value> {
    let qui = qui_de(charge);
    use declarations::casc_regularisation::{ecrire, marquer_regularise, regularisation, Demande};
    // `charge.get("vire_le") or {}`.
    let vire_le: Vec<(String, Value)> = match charge.get("vire_le").filter(|v| vrai(v)) {
        None => Vec::new(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(autre) => return Err(erreur("AttributeError", socle::fichiers::attendu("un objet", socle::valeur::nom_type(autre)))),
    };
    let aujourdhui = crate::routes::dossier_salarie::aujourdhui_reel();
    let mut dates: Vec<(String, String)> = Vec::new();
    for (a, d) in &vire_le {
        let texte = valeur_en_texte(d);
        let Some(jour) = jour_iso(&texte) else {
            return Err(refus(format!("Date du virement {} illisible : {} (attendu AAAA-MM-JJ). Rien n'est parti.", a, repr(&d_en_repr(d)))));
        };
        if jour > aujourdhui {
            return Err(refus(format!(
                "Le virement {} est date du {}, qui n'est pas encore arrive : le courriel annonce un virement FAIT. Rien n'est parti.",
                a,
                jjmmaaaa(&jour.iso())
            )));
        }
        match dates.iter_mut().find(|(k, _)| k == a) {
            Some(e) => e.1 = jour.iso(),
            None => dates.push((a.clone(), jour.iso())),
        }
    }
    if dates.is_empty() {
        return Err(refus("Aucune date de virement : les courriels annoncent des virements faits, ils partent apres. Rien n'est parti."));
    }
    let mut annees_voulues: Vec<String> = dates.iter().map(|(a, _)| a.clone()).collect();
    annees_voulues.sort();

    // Le dossier, ecrit : la liste nominative jointe est celle du jour.
    let mut r = regularisation(ctx, &Demande { toutes_les_annees: true, annees: annees_voulues.clone(), vire_le: dates.clone(), ..Default::default() })?;
    ecrire(ctx, &mut r)?;
    let mut manquent: Vec<String> = annees_voulues.iter().filter(|a| !r.annees.iter().any(|x| &x.annee == *a)).cloned().collect();
    manquent.sort();
    if !manquent.is_empty() {
        return Err(refus(format!("Rien a regulariser pour {} (deja fait, ou aucune DSN deposee). Rien n'est parti.", manquent.join(", "))));
    }
    let classeur = std::path::Path::new(&r.dossier).join("CASC-SVP_liste-nominative.xlsx");
    if !classeur.exists() {
        return Err(refus(format!(
            "La liste nominative n'a pas pu etre remplie ({}) : le courriel au CASC-SVP la promet en piece jointe. Rien n'est parti.",
            r.avertissements.last().cloned().unwrap_or_default()
        )));
    }

    let mut client = ouvrir()?;
    let dernier_mois = r.mois.last().map(|m| m.mois.clone()).ok_or_else(|| erreur("IndexError", socle::fichiers::trop_courte("les mois de la régularisation")))?;
    verifier_preuve(ctx, Some(&preuve_de(charge)), &dernier_mois, "envoi a Audiens et au CASC-SVP (liste nominative)", "Audiens et CASC-SVP")?;

    let messages = vec![
        Message {
            cle: "audiens".into(),
            nom: "Audiens".into(),
            destinataire: r.courriel_audiens.a.clone(),
            sujet: r.courriel_audiens.objet.clone(),
            corps: r.courriel_audiens.corps.clone(),
            pieces: Vec::new(),
            html: courriel_en_html(&r.courriel_audiens.corps),
        },
        Message {
            cle: "casc".into(),
            nom: "CASC-SVP".into(),
            destinataire: r.courriel_casc.a.clone(),
            sujet: r.courriel_casc.objet.clone(),
            corps: r.courriel_casc.corps.clone(),
            pieces: vec![classeur],
            html: String::new(),
        },
    ];
    let compte = client.compte().to_string();
    let mut copies: Vec<Vec<u8>> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &compte, client.as_mut(), None, Some(&mut copies)) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, "regularisation-casc", None, &qui, &e.partis, &e.echoue, &e.message, Some(json!({"annees": annees_voulues})), None)?;
            return Err(echec_d_envoi(e.message));
        }
    };

    // Ce que l'envoi garde : les deux messages tels qu'ils sont partis.
    let garde = std::path::Path::new(&r.dossier).join("envoyes");
    std::fs::create_dir_all(&garde).map_err(|e| erreur("OSError", format!("{} : {}", garde.display(), e)))?;
    let iso = socle::paris::maintenant();
    let horodate = format!("{}_{}{}{}", &iso[..10], &iso[11..13], &iso[14..16], &iso[17..19]);
    let mut emls: Vec<String> = Vec::new();
    for (m, brut) in messages.iter().zip(&copies) {
        let p = garde.join(format!("{}_CASC-SVP_{}.eml", horodate, m.cle));
        std::fs::write(&p, brut).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
        emls.push(nom_du(&p));
    }
    let date_de = |a: &str| dates.iter().find(|(k, _)| k == a).map(|(_, d)| d.clone()).unwrap_or_default();
    let detail = json!({
        "annees": r.annees.iter().map(|a| json!({"annee": a.annee, "vire_le": date_de(&a.annee), "montant": a.cotisation.to_string()})).collect::<Vec<_>>(),
        "mois": r.mois.iter().map(|m| m.mois.clone()).collect::<Vec<_>>(),
        "total": r.total.to_string(),
        "expediteur": compte,
        "eml": emls,
    });
    let ligne = noter(ctx, "regularisation-casc", None, &qui, true, &partis.join(", "), &format!("{} courriel(s) envoye(s)", partis.len()), Some(&detail))?;
    // Les annees regularisees : leurs mois sortent des dossiers suivants. S'ecrit APRES le virement, jamais avant.
    let auj = socle::paris::maintenant();
    let mut notees: Vec<Value> = Vec::new();
    for a in &r.annees {
        notees.push(marquer_regularise(ctx, &json!(a.annee), &json!(date_de(&a.annee)), &json!(a.cotisation.to_string()), &qui, &auj[..10])?);
    }
    Ok(json!({
        "partis": partis,
        "expediteur": compte,
        "periode": r.periode,
        "total": r.total.to_string(),
        "annees": notees,
        "eml": emls,
        "dossier": garde.to_string_lossy(),
        "ligne": ligne,
    }))
}

/// Le texte d'une valeur de `vire_le` comme `%r` l'ecrit : ce qui n'est pas un texte s'ecrit sans guillemets.
fn d_en_repr(d: &Value) -> String {
    valeur_en_texte(d)
}

/// `operations_socle.exercices` : les clotures, sans les cles `_...`.
fn exercices_clos(racine: &Path) -> R<Map<String, Value>> {
    let v = lire_json_ou_vide(&racine.join("config").join("exercices.json"))?;
    match v {
        Value::Object(o) => Ok(o.into_iter().filter(|(k, _)| !k.starts_with('_')).collect()),
        autre => Err(erreur("AttributeError", socle::fichiers::attendu("un objet", nom_type(&autre)))),
    }
}

/// La route `exercices` : `{"exercices": etat_exercices, "dossier": ...}`.
fn exercices_route(ctx: &Contexte) -> R<Value> {
    let liste = etat_exercices(ctx)?;
    Ok(json!({"exercices": liste, "dossier": ctx.racine.join("sorties").join("exercices").display().to_string()}))
}

/// `etat_exercices` : chaque annee connue du dossier -- ses mois, sa cloture,
/// son export.
fn etat_exercices(ctx: &Contexte) -> R<Value> {
    let clos = exercices_clos(&ctx.racine)?;
    let mois_tous: Vec<String> = mois_connus(&ctx.racine).into_iter().map(|(m, _)| m).collect();
    let mut annees: BTreeSet<String> = mois_tous.iter().map(|m| m.chars().take(4).collect()).collect();
    annees.extend(clos.keys().cloned());
    let mut m = Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in annees.iter().rev() {
        let mut mois: Vec<&String> = mois_tous.iter().filter(|x| x.starts_with(a.as_str())).collect();
        mois.sort();
        let export = ctx.racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", a));
        let export = export.exists().then_some(export);
        let mut acceptees = Vec::new();
        for x in &mois {
            if vrai(&m.dsn_du_mois(x)?) {
                acceptees.push(json!(x));
            }
        }
        let export_json = match &export {
            Some(p) => {
                json!({
                    "fichier": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    "ecrit": crate::depart::ecrit_le(&ctx.racine, p),
                })
            }
            None => Value::Null,
        };
        out.push(json!({
            "annee": a, "mois": mois, "dsn_acceptees": acceptees,
            "clos": clos.get(a).cloned().unwrap_or(Value::Null),
            "export": export_json,
        }));
    }
    Ok(Value::Array(out))
}

/// L'erreur du disque, sous le nom que Python lui donne.
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    let classe = match (e.kind(), e.raw_os_error()) {
        (std::io::ErrorKind::NotFound, _) => "FileNotFoundError",
        (std::io::ErrorKind::PermissionDenied, _) => "PermissionError",
        (_, Some(20)) => "NotADirectoryError",
        (_, Some(21)) => "IsADirectoryError",
        _ => "OSError",
    };
    erreur(classe, socle::fichiers::panne_fichier(e, p))
}
