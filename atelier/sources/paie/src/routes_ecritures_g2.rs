//! Les routes qui ECRIVENT du lot G2 (`moteur/web.py`, `operations_regularisation`,
//! `operations_arrets`, `operations_echeancier`, `operations_etats`) :
//! `regularisation-ecrire`, `regularisation-annuler`, `arret-enregistrer`, `arret-reprise`,
//! `arret-oublier`, `echeance-faite` ; et `couts`, qui ne fait que lire.
//!
//! Memes fichiers ecrits, aux memes octets (`regularisations/AAAA-MM.json`, `arrets/AAAA-MM.json`,
//! `config/echeancier.json`), memes reponses que le Python. Un refus garde sa phrase quand elle est
//! utile a la page ; la classe de l'exception peut differer (`ContratIntrouvable`,
//! `ArretIntrouvable`, `MoisNonRegularisable` sont des `ErreurOperation` ; `SignalementImpossible`
//! une `ValueError`).
//!
//! - Une correction (`regularisation-ecrire`, `-annuler`) arme ou desarme la prochaine DSN d'un
//!   mois deja declare : le mot de passe du logiciel, s'il est pose, vient dans le corps
//!   (`motdepasse`) et se confronte au coffre (`filet::exiger_mot_de_passe`).
//! - Un arret se saisit, se reprend, s'oublie sans mot de passe : rien ne part. Les controles de la
//!   norme sont ceux de `signalement::verifier`, les memes que ceux du signalement.
//! - `couts` : le cout de l'annee d'un spectacle et d'un salarie, lu dans le livre de paie
//!   (`fiche_recapitulative::Recapitulatifs::couts_de_l_annee`).

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::date::{d, Jour};
use crate::ecriture::ecrire_json;
use crate::etats::{eq_py, erreur, py_str, R};
use crate::fichiers::vrai;
use crate::retours::Echec;
use crate::routes_dossiers::{brut, contrats_au, salarie};
use crate::routes_ecrites::{etat_correction, lire_corrections, marquer_echeance};
use crate::routes_listes::{lire_arrets, nomenclatures, resume_arret};
use crate::signalement::{self, ARRET};

/// Les routes que ce module sert.
pub const ROUTES: [&str; 7] = [
    "regularisation-ecrire",
    "regularisation-annuler",
    "arret-enregistrer",
    "arret-reprise",
    "arret-oublier",
    "echeance-faite",
    "couts",
];

/// Une route de ce module.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "regularisation-ecrire" => regularisation_ecrire(o, ctx, charge),
        "regularisation-annuler" => regularisation_annuler(ctx, charge),
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

fn en_os(m: String) -> Echec {
    erreur("OSError", m)
}

/// `charge["cle"]` : `KeyError` si elle manque.
fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle)))
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

/// `datetime.now().strftime("%Y-%m-%d %H:%M")`, a l'heure de Paris.
pub(crate) fn maintenant() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    crate::paris::iso_minutes(t).replacen('T', " ", 1)
}

fn ecrire_fichier(chemin: &Path, v: Value) -> R<()> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur("OSError", format!("{} : {}", parent.display(), e)))?;
    }
    ecrire_json(chemin, &v, 2).map_err(en_os)
}

/// `d(v)` : une date lisible, sinon `ValueError`.
fn jour(v: &Value) -> R<Jour> {
    d(v).map_err(|m| erreur("ValueError", m))
}

/// `jj_mm_aaaa(v)`.
fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(jour(v)?.jj_mm_aaaa())
}

/// QUI ECRIT. Les routes inscrivent `interface` : c'est la fenetre. La ligne de commande (`paie ...`) ajoute `qui` a la charge
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
        return Err(erreur("AttributeError", crate::fichiers::attendu("un texte", crate::etats::nom_type(cle))));
    };
    marquer_echeance(ctx, texte, fait, &qui_de(charge))?;
    Ok(json!({"cle": texte, "fait": fait}))
}

// ------------------------------------------------------------------------ couts

/// `operations_etats.couts_de_l_annee(ctx, charge["annee"], ctx.aujourdhui)`.
fn couts(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let _ = o;
    let annee = py_str(exiger(charge, "annee")?);
    let mut r = crate::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(annee), false);
    r.couts_de_l_annee().map_err(|e| Echec { classe: e.classe, message: e.message })
}

// --------------------------------------------------------------- les corrections

/// `Correction` -> son fichier : `{"_origine", "mois", "corrections": [asdict(c)]}`.
pub(crate) fn ecrire_corrections(ctx: &Contexte, mois: &str, corrections: Vec<Map<String, Value>>) -> R<()> {
    let chemin = ctx.racine.join("regularisations").join(format!("{}.json", mois));
    ecrire_fichier(
        &chemin,
        json!({
            "_origine": format!(
                "Corrections du mois {m} apres son echeance DSN : chacune part dans la DSN d'un mois suivant, datee de {m}. La saisie de {m} (paies/{m}.json) reste telle que deposee.",
                m = mois
            ),
            "mois": mois,
            "corrections": corrections.into_iter().map(Value::Object).collect::<Vec<_>>(),
        }),
    )
}

/// `{"correction": js.brut(c), "etat": c.etat()}`.
fn reponse_correction(c: &Map<String, Value>) -> Value {
    json!({"correction": brut(&Value::Object(c.clone())), "etat": etat_correction(c)})
}

/// `operations_regularisation.enregistrer_correction` : ecrit la correction. Une seule en cours par
/// engagement ; le mot de passe du logiciel est exige s'il est pose.
fn regularisation_ecrire(o: &HashMap<String, String>, ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = exiger(charge, "mois")?;
    let salarie_v = exiger(charge, "salarie")?;
    let debut = exiger(charge, "debut")?;
    let engagement = exiger(charge, "engagement")?;
    crate::filet::exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "corriger un mois deja declare")?;
    let motif = match optionnel(charge, "motif") {
        v if !vrai(v) => String::new(),
        Value::String(t) => t.clone(),
        autre => {
            return Err(erreur("AttributeError", crate::fichiers::attendu("un texte", crate::etats::nom_type(autre))));
        }
    };
    let motif = motif.trim_matches(crate::retours::blanc_py).to_string();
    if motif.is_empty() {
        return Err(refus("Une correction se motive : dire ce qui etait faux."));
    }
    let a = crate::routes_regularisation::apercu_correction(
        o,
        ctx,
        &json!({"mois": mois, "salarie": salarie_v, "debut": debut, "engagement": engagement}),
    )?;
    if a["rien_ne_change"].as_bool().unwrap_or(false) {
        return Err(refus("La correction ne change rien a la paie declaree : rien n'est ecrit."));
    }
    let nom_du_mois = py_str(mois);
    let mut corrections = lire_corrections(ctx, &nom_du_mois)?;
    for c in &corrections {
        let meme = eq_py(&c["salarie"], salarie_v) && eq_py(&c["debut"], debut);
        if meme && !vrai(&c["portee_par"]) && !vrai(&c["annulee_le"]) {
            return Err(refus(format!(
                "Une correction de cet engagement est deja en attente (n° {}, {}). L'annuler d'abord, ou attendre qu'elle soit portee.",
                py_str(&c["numero"]),
                py_str(&c["motif"])
            )));
        }
    }
    let mut c = Map::new();
    c.insert("mois".into(), mois.clone());
    c.insert("numero".into(), json!(corrections.len() + 1));
    c.insert("salarie".into(), salarie_v.clone());
    c.insert("debut".into(), debut.clone());
    c.insert("motif".into(), json!(motif));
    c.insert("engagement".into(), a["engagement"].clone());
    c.insert("decidee_le".into(), json!(maintenant()));
    c.insert("qui".into(), json!(qui_de(charge)));
    c.insert("preparee_par".into(), Value::Null);
    c.insert("portee_par".into(), Value::Null);
    c.insert("annulee_le".into(), Value::Null);
    let reponse = reponse_correction(&c);
    corrections.push(c);
    ecrire_corrections(ctx, &nom_du_mois, corrections)?;
    Ok(reponse)
}

/// `int(x)`, comme Python le lit d'une valeur JSON.
fn entier_py(v: &Value) -> R<i64> {
    match v {
        Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(i),
            None => Ok(n.as_f64().unwrap_or(0.0).trunc() as i64),
        },
        Value::Bool(b) => Ok(*b as i64),
        Value::String(t) => {
            let net = t.trim_matches(crate::retours::blanc_py);
            net.parse::<i64>()
                .map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", crate::fichiers::repr(t))))
        }
        autre => Err(erreur(
            "TypeError",
            crate::fichiers::attendu("un nombre entier", crate::etats::nom_type(autre)),
        )),
    }
}

/// `operations_regularisation.annuler_correction` : une correction pas encore portee s'annule.
fn regularisation_annuler(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = exiger(charge, "mois")?;
    let numero = exiger(charge, "numero")?;
    crate::filet::exiger_mot_de_passe(&ctx.racine, optionnel(charge, "motdepasse"), "annuler une correction")?;
    let nom_du_mois = py_str(mois);
    let mut corrections = lire_corrections(ctx, &nom_du_mois)?;
    let mut trouve: Option<usize> = None;
    for (i, c) in corrections.iter().enumerate() {
        if eq_py(&c["numero"], &json!(entier_py(numero)?)) {
            trouve = Some(i);
            break;
        }
    }
    let Some(i) = trouve else {
        return Err(refus(format!("Pas de correction n° {} pour {}.", py_str(numero), nom_du_mois)));
    };
    let c = &mut corrections[i];
    if vrai(&c["portee_par"]) {
        let portee = c["portee_par"].clone();
        let mois_porteur = portee.get("mois").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("mois")))?;
        let quand = portee.get("quand").cloned().unwrap_or_else(|| json!("?"));
        return Err(refus(format!(
            "La correction n° {} de {} a ete PORTEE par la DSN de {} (deposee le {}) : elle ne s'annule plus. Une nouvelle correction peut la contredire.",
            py_str(&c["numero"]),
            nom_du_mois,
            py_str(mois_porteur),
            py_str(&quand)
        )));
    }
    if vrai(&c["annulee_le"]) {
        return Err(refus(format!("La correction n° {} de {} est deja annulee.", py_str(&c["numero"]), nom_du_mois)));
    }
    c.insert("annulee_le".into(), json!(maintenant()));
    c.insert("qui".into(), json!(qui_de(charge)));
    let reponse = reponse_correction(c);
    ecrire_corrections(ctx, &nom_du_mois, corrections)?;
    Ok(reponse)
}

// ------------------------------------------------------------------------ arrets

/// `Arret` -> son fichier : `{"_origine", "mois", "arrets": [asdict(a)]}`.
pub(crate) fn ecrire_arrets(ctx: &Contexte, mois: &str, arrets: Vec<Value>) -> R<()> {
    let chemin = ctx.racine.join("arrets").join(format!("{}.json", mois));
    ecrire_fichier(
        &chemin,
        json!({
            "_origine": format!(
                "Les arrets de travail commences en {}. Chacun part deux fois : en signalement d'evenement (nature 04, dans les cinq jours), puis reporte par chaque DSN mensuelle dont le mois croise l'arret.",
                mois
            ),
            "mois": mois,
            "arrets": arrets,
        }),
    )
}

/// `signalement.verifier(arret, ARRET)` : ce que la norme refuse (`SignalementImpossible`, une
/// `ValueError`) ; les avertissements ne servent pas ici.
fn verifier(arret: &Value) -> R<()> {
    match signalement::verifier(&crate::dsn::Arret { v: arret.clone() }, ARRET, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(erreur("ValueError", m)),
        Err(signalement::Echec::Autre(m)) => Err(Echec { classe: None, message: m }),
    }
}

/// `ctx.salarie(salarie)["_cle"]`.
fn cle_du_salarie(ctx: &Contexte, nom: &Value) -> R<Value> {
    salarie(ctx, nom)?.get("_cle").cloned().ok_or_else(|| erreur("KeyError", crate::fichiers::absent("_cle")))
}

/// Les `n` premiers caracteres du texte de `v` (`str(v)[:n]`).
fn debut_du_texte(v: &Value, n: usize) -> String {
    py_str(v).chars().take(n).collect()
}

/// `operations_arrets._trouver` : (l'indice de l'arret, la liste de son mois).
pub(crate) fn trouver(ctx: &Contexte, nom: &Value, dernier_jour: &Value) -> R<(usize, Vec<Value>)> {
    let mois = debut_du_texte(dernier_jour, 7);
    let liste = lire_arrets(ctx, &mois)?;
    let cle = cle_du_salarie(ctx, nom)?;
    let texte = py_str(dernier_jour);
    for (i, a) in liste.iter().enumerate() {
        if eq_py(&a["salarie"], &cle) && eq_py(&a["dernier_jour_travaille"], &json!(texte)) {
            return Ok((i, liste));
        }
    }
    let quand = if vrai(dernier_jour) { jj_mm_aaaa(dernier_jour)? } else { "?".to_string() };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. `paie arret` les liste.",
        py_str(&cle),
        quand
    )))
}

/// `operations_arrets.resume`, rendu tel que `web.py` le met dans sa reponse.
fn reponse_arret(ctx: &Contexte, arret: &Value) -> R<Value> {
    let n = nomenclatures(ctx)?;
    Ok(json!({"arret": resume_arret(&n, arret)?}))
}

/// `operations_arrets.enregistrer_arret(..., qui="interface")`.
fn arret_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let fin = exiger(charge, "fin")?;
    let motif = exiger(charge, "motif")?;
    let contrat_numero = ou_rien(optionnel(charge, "contrat"));

    let cle = cle_du_salarie(ctx, nom)?;
    let dernier = jour(dernier_jour)?;
    let candidats = contrats_au(ctx, &cle, &dernier.iso())?;
    let texte_de = |c: &Value, k: &str| py_str(&c[k]);
    let choisi: Value = if vrai(&contrat_numero) {
        match candidats.iter().find(|c| eq_py(&c["numero"], &contrat_numero)) {
            Some(c) => c.clone(),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| texte_de(c, "numero")).collect();
                return Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    py_str(&contrat_numero),
                    py_str(&cle),
                    jj_mm_aaaa(dernier_jour)?,
                    if liste.is_empty() { "aucun".to_string() } else { liste.join(", ") }
                )));
            }
        }
    } else if candidats.len() == 1 {
        candidats[0].clone()
    } else if candidats.is_empty() {
        return Err(refus(format!(
            "Aucun contrat de {} ne couvre le {}. Un signalement d'arret designe le contrat suspendu par son numero et sa date de debut (S21.G00.40.009 et .001) : sans contrat declare, il n'y a rien a suspendre. Verifier la date, ou saisir le contrat.",
            py_str(&cle),
            jj_mm_aaaa(dernier_jour)?
        )));
    } else {
        let liste: Vec<String> =
            candidats.iter().map(|c| format!("{} du {} au {}", texte_de(c, "numero"), texte_de(c, "debut"), texte_de(c, "fin"))).collect();
        return Err(refus(format!(
            "Deux contrats de {} couvrent le {} ({}). Le cahier technique laisse le choix au declarant : dire lequel.",
            py_str(&cle),
            jj_mm_aaaa(dernier_jour)?,
            liste.join(", ")
        )));
    };

    let subrogation = optionnel(charge, "subrogation").clone();
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
    let mut a = Map::new();
    a.insert("salarie".into(), cle.clone());
    a.insert("contrat_debut".into(), choisi["debut"].clone());
    a.insert("contrat_numero".into(), choisi["numero"].clone());
    a.insert("motif".into(), json!(py_str(motif)));
    a.insert("dernier_jour_travaille".into(), json!(dernier.iso()));
    a.insert("fin_previsionnelle".into(), json!(jour(fin)?.iso()));
    a.insert("subrogation".into(), subrogation);
    a.insert("subrogation_debut".into(), jour_ou_rien("subrogation_debut")?);
    a.insert("subrogation_fin".into(), jour_ou_rien("subrogation_fin")?);
    a.insert("reprise".into(), Value::Null);
    a.insert("reprise_motif".into(), Value::Null);
    a.insert("date_accident".into(), jour_ou_rien("date_accident")?);
    a.insert("saisi_le".into(), json!(maintenant()));
    a.insert("qui".into(), json!(qui_de(charge)));
    a.insert("note".into(), note);
    a.insert("envois".into(), json!([]));
    let arret = Value::Object(a);
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;

    let mois: String = arret["dernier_jour_travaille"].as_str().unwrap_or("").chars().take(7).collect();
    let mut liste = lire_arrets(ctx, &mois)?;
    if liste.iter().any(|x| eq_py(&x["salarie"], &arret["salarie"]) && eq_py(&x["dernier_jour_travaille"], &arret["dernier_jour_travaille"])) {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            py_str(&cle),
            jj_mm_aaaa(&arret["dernier_jour_travaille"])?
        )));
    }
    liste.push(arret.clone());
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `operations_arrets.modifier_arret(ctx, salarie, dernier_jour, reprise=..., reprise_motif=...)`.
fn arret_reprise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let reprise = ou_rien(exiger(charge, "reprise")?);
    let motif = ou_rien(optionnel(charge, "motif_reprise"));
    let (i, mut liste) = trouver(ctx, nom, dernier_jour)?;
    if let Value::Object(a) = &mut liste[i] {
        a.insert("reprise".into(), reprise);
        a.insert("reprise_motif".into(), motif);
    }
    let arret = liste[i].clone();
    verifier(&arret)?;
    let mois: String = py_str(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `operations_arrets.supprimer_arret` : un arret deja declare ne se retire pas.
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
    let mois: String = py_str(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_entier_se_lit_comme_python() {
        assert_eq!(entier_py(&json!(3)).unwrap(), 3);
        assert_eq!(entier_py(&json!(" 4 ")).unwrap(), 4);
        assert_eq!(entier_py(&json!(2.9)).unwrap(), 2);
        assert!(entier_py(&json!("x")).is_err());
        assert!(entier_py(&Value::Null).is_err());
    }

    #[test]
    fn l_heure_a_la_forme_du_python() {
        let t = maintenant();
        assert_eq!(t.len(), 16);
        assert_eq!(&t[10..11], " ");
    }
}
