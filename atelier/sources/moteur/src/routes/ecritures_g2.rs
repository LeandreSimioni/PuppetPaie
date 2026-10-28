//! Les routes qui ECRIVENT du lot G2 (`moteur/web.py`, `operations_arrets`, `operations_echeancier`,
//! `operations_etats`) : `arret-enregistrer`, `arret-reprise`,
//! `arret-oublier`, `echeance-faite` ; et `couts`, qui ne fait que lire.
//!
//! Memes fichiers ecrits, aux memes octets (`imprevus/AAAA-MM.json` (le magasin des
//! imprevus, `imprevus`),
//! `config/echeancier.json`), memes reponses que le Python. Un refus garde sa phrase quand elle est
//! utile a la page ; la classe de l'exception peut differer (`ContratIntrouvable`,
//! `ArretIntrouvable` sont des `ErreurOperation` ; `SignalementImpossible`
//! une `ValueError`).
//!
//! - Un arret se saisit, se reprend, s'oublie sans mot de passe : rien ne part ; il vit dans le magasin des imprevus
//!   (`imprevus`), qui en tire la ligne de paie d'un permanent et le bloc 60 de chaque DSN. Les controles de la
//!   norme sont ceux de `signalement::verifier`, les memes que ceux du signalement.
//! - `couts` : le cout de l'annee d'un spectacle et d'un salarie, lu dans le livre de paie
//!   (`fiche_recapitulative::Recapitulatifs::couts_de_l_annee`).

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::{d, Jour};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::echec::Echec;
use crate::routes::dossiers::salarie;
use crate::routes::ecrites::marquer_echeance;
use crate::routes::listes::nomenclatures;
use declarations::signalement::{self, ARRET};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 5] = [
    "arret-enregistrer",
    "arret-reprise",
    "arret-oublier",
    "echeance-faite",
    "couts",
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "arret-enregistrer" => arret_enregistrer(ctx, charge),
        "arret-reprise" => arret_reprise(ctx, charge),
        "arret-oublier" => arret_oublier(ctx, charge),
        "echeance-faite" => echeance_faite(ctx, charge),
        "couts" => couts(o, ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

/// `charge["cle"]` : `KeyError` si elle manque.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

/// `charge.get(cle)`.
fn optionnel<'a>(charge: &'a Value, cle: &str) -> &'a Value {
    charge.get(cle).unwrap_or(&Value::Null)
}

/// `x or None`.
fn ou_rien(v: &Value) -> Value {
    if vrai(v) {
        v.clone()
    } else {
        Value::Null
    }
}

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// `d(v)` : une date lisible, sinon `ValueError`.
fn jour(v: &Value) -> R<Jour> {
    d(v).map_err(|m| erreur("ValueError", m))
}

/// `jj_mm_aaaa(v)`.
fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(jour(v)?.jj_mm_aaaa())
}

/// QUI ECRIT. Les routes inscrivent `interface` : c'est la fenetre. La ligne de commande (`moteur ...`) ajoute `qui` a la charge
/// (`cli`, ou `ligne de commande` selon la commande, comme le Python les ecrit) : une valeur ecrite reste identique d'une face a
/// l'autre. Absent, vide ou pas un texte : `interface` -- la fenetre ne change pas.
pub(crate) fn qui_de(charge: &Value) -> String {
    match charge.get("qui") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => "interface".to_string(),
    }
}

// --------------------------------------------------------------- echeance-faite

/// `operations_echeancier.marquer_echeance(ctx, charge["cle"], fait=..., qui="interface")`.
fn echeance_faite(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = exiger(charge, "cle")?;
    let fait = charge.get("fait") != Some(&Value::Bool(false));
    let Some(texte) = cle.as_str() else {
        return Err(erreur("AttributeError", socle::fichiers::attendu("un texte", socle::valeur::nom_type(cle))));
    };
    marquer_echeance(ctx, texte, fait, &qui_de(charge))?;
    Ok(json!({"cle": texte, "fait": fait}))
}

// ------------------------------------------------------------------------ couts

/// `operations_etats.couts_de_l_annee(ctx, charge["annee"], ctx.aujourdhui)`.
fn couts(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let _ = o;
    let annee = valeur_en_texte(exiger(charge, "annee")?);
    let mut r = documents::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(annee), false);
    r.couts_de_l_annee().map_err(|e| Echec { classe: e.classe, message: e.message })
}


// ------------------------------------------------------------------------ arrets

/// Ecrit le fichier d'un mois du magasin des imprevus (`imprevus/AAAA-MM.json`) : les arrets ET les suspensions qu'il
/// range, tels que la liste les donne.
pub(crate) fn ecrire_arrets(ctx: &Contexte, mois: &str, imprevus: Vec<Value>) -> R<()> {
    crate::routes::imprevus::ecrire(ctx, mois, imprevus)
}

/// `signalement.verifier(arret, ARRET)` : ce que la norme refuse (`SignalementImpossible`, une
/// `ValueError`) ; les avertissements ne servent pas ici.
pub(crate) fn verifier(arret: &Value) -> R<()> {
    match signalement::verifier(&declarations::dsn::Arret { v: arret.clone() }, ARRET, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(erreur("ValueError", m)),
        Err(signalement::Echec::Autre(m)) => Err(Echec { classe: None, message: m }),
    }
}

/// `ctx.salarie(salarie)["_cle"]`.
fn cle_du_salarie(ctx: &Contexte, nom: &Value) -> R<Value> {
    salarie(ctx, nom)?.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'arret de ce salarie a ce dernier jour travaille : (son indice, le fichier de son mois, tous imprevus compris).
pub(crate) fn trouver(ctx: &Contexte, nom: &Value, dernier_jour: &Value) -> R<(usize, Vec<Value>)> {
    let cle = cle_du_salarie(ctx, nom)?;
    if vrai(dernier_jour) {
        if let Some(t) = crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &jour(dernier_jour)?)? {
            return Ok(t);
        }
    }
    let quand = if vrai(dernier_jour) { jj_mm_aaaa(dernier_jour)? } else { "?".to_string() };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. `moteur arret` les liste.",
        valeur_en_texte(&cle),
        quand
    )))
}

/// `{"arret": resume}` : l'arret tel que la page le lit (`crate::routes::imprevus::resume`, avec les cachets qui tombent dedans).
fn reponse_arret(ctx: &Contexte, arret: &Value) -> R<Value> {
    let n = nomenclatures(ctx)?;
    Ok(json!({"arret": crate::routes::imprevus::resume(ctx, &n, arret)?}))
}

/// `arret-enregistrer` : `{salarie, dernier_jour, fin, motif, subrogation?, subrogation_debut?, subrogation_fin?,
/// date_accident?, jours_deja_indemnises?, contrat?, note?}` -> `{"arret": resume}`. Les controles de la norme sont ceux
/// du signalement (`signalement::verifier`, nature 04), avant toute ecriture ; un arret ne chevauche pas un autre imprevu
/// du meme contrat (`crate::routes::imprevus::sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let fin = exiger(charge, "fin")?;
    let motif = exiger(charge, "motif")?;
    let cle = cle_du_salarie(ctx, nom)?;
    let dernier = jour(dernier_jour)?;
    let choisi = crate::routes::imprevus::contrat_suspendu(ctx, &valeur_en_texte(&cle), &dernier, &ou_rien(optionnel(charge, "contrat")))?;

    let jour_ou_rien = |k: &str| -> R<Value> {
        let v = optionnel(charge, k);
        if vrai(v) {
            Ok(json!(jour(v)?.iso()))
        } else {
            Ok(Value::Null)
        }
    };
    let note = {
        let n = optionnel(charge, "note");
        if vrai(n) {
            n.clone()
        } else {
            json!("")
        }
    };
    // Les jours deja indemnises (D1226-4) ne se disent que pour un arret qu'un maintien de salaire peut suivre : un contrat
    // paye au mois, et un motif de maladie ou d'accident (L1226-1 ; `maintien_legal` dans arrets-base.json).
    let deja = crate::routes::imprevus::jours_ou_rien(optionnel(charge, "jours_deja_indemnises"), "Jours deja indemnises")?;
    if !deja.is_null() {
        let m = engagement::imprevus::motif_d_arret(&valeur_en_texte(motif)).map_err(crate::routes::listes::refus_magasin)?.unwrap_or(Value::Null);
        if choisi["mensuel"] != json!(true) || !m.get("maintien_legal").is_some_and(vrai) {
            return Err(refus(
                "Les jours deja indemnises (D1226-4) ne servent qu'au maintien de salaire d'un permanent arrete pour maladie \
ou accident (L1226-1) : cet arret n'en appelle pas. Rien n'est ecrit.",
            ));
        }
    }
    let mut a = Map::new();
    a.insert("genre".into(), json!(engagement::imprevus::ARRET));
    a.insert("salarie".into(), cle.clone());
    a.insert("contrat_debut".into(), choisi["debut"].clone());
    a.insert("contrat_numero".into(), choisi["numero"].clone());
    a.insert("motif".into(), json!(valeur_en_texte(motif)));
    a.insert("dernier_jour_travaille".into(), json!(dernier.iso()));
    a.insert("fin_previsionnelle".into(), json!(jour(fin)?.iso()));
    a.insert("subrogation".into(), optionnel(charge, "subrogation").clone());
    a.insert("subrogation_debut".into(), jour_ou_rien("subrogation_debut")?);
    a.insert("subrogation_fin".into(), jour_ou_rien("subrogation_fin")?);
    a.insert("reprise".into(), Value::Null);
    a.insert("reprise_motif".into(), Value::Null);
    a.insert("date_accident".into(), jour_ou_rien("date_accident")?);
    a.insert("jours_deja_indemnises".into(), deja);
    a.insert("maintien".into(), json!({}));
    a.insert("prolongations".into(), json!([]));
    a.insert("saisi_le".into(), json!(maintenant()));
    a.insert("qui".into(), json!(qui_de(charge)));
    a.insert("note".into(), note);
    a.insert("envois".into(), json!([]));
    let arret = Value::Object(a);
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;

    let mois = dernier.mois_iso();
    if crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &dernier)?.is_some() {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            valeur_en_texte(&cle),
            dernier.jj_mm_aaaa()
        )));
    }
    crate::routes::imprevus::sans_chevauchement(ctx, &arret, None)?;
    let mut liste = engagement::imprevus::du_fichier(&ctx.racine, &mois).map_err(crate::routes::listes::refus_magasin)?;
    liste.push(arret.clone());
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `arret-reprise` : `{salarie, dernier_jour, reprise, motif_reprise?}` -> `{"arret": resume}` ; une reprise vide l'efface.
fn arret_reprise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let reprise = ou_rien(exiger(charge, "reprise")?);
    let motif = ou_rien(optionnel(charge, "motif_reprise"));
    let (i, mut liste) = trouver(ctx, nom, dernier_jour)?;
    if let Value::Object(a) = &mut liste[i] {
        a.insert("reprise".into(), if vrai(&reprise) { json!(jour(&reprise)?.iso()) } else { reprise });
        a.insert("reprise_motif".into(), motif);
    }
    let arret = liste[i].clone();
    verifier(&arret)?;
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `arret-oublier` : `{salarie, dernier_jour}` -> `{"arret": resume}`. Un arret deja declare ne se retire pas.
fn arret_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let (i, mut liste) = trouver(ctx, nom, dernier_jour)?;
    let envois = match &liste[i]["envois"] {
        Value::Array(e) => e.len(),
        _ => 0,
    };
    if envois > 0 {
        return Err(refus(format!(
            "Cet arret a deja ete declare ({} envoi(s)). Le retirer du magasin n'annule rien aupres des organismes et ferait disparaitre la trace de ce qui est parti : emettre un annule-et-remplace.",
            envois
        )));
    }
    let arret = liste.remove(i);
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_heure_a_la_forme_du_python() {
        let t = maintenant();
        assert_eq!(t.len(), 16);
        assert_eq!(&t[10..11], " ");
    }
}
