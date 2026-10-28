//! `moteur dossier [CLE ...] [--mois AAAA-MM] [--annee AAAA] [--toutes-les-annees] [--seulement CATEGORIE ...] [--ensemble]
//! [--apercu] [--envoyer] [--fenetre]` : le dossier d'un salarie -- ou de chacun --, tous ses documents sur une periode
//! (`moteur/commandes_exports.py`, `cmd_dossier` ; `operations_dossier`). Pas de `--json` (le Python n'en a pas ici).
//!
//! Le dossier se construit par `crate::routes::dossier_salarie::construire` (celui de la route `dossier-salarie`) et s'ecrit dans
//! `exports/dossiers/` sauf `--apercu` (le compte des pages, rien sur le disque).
//!
//! `--envoyer` envoie a chaque salarie SON dossier : un envoi qui ne se rattrape pas. Il passe par
//! `crate::routes::envois::envoyer_dossier` (celui de la route `dossier-envoyer`), donc par les garde-fous du moteur : la boite est ouverte
//! (le coffre par la cle du poste ou une seance), puis le moteur demande LUI-MEME le mot de passe du logiciel
//! (`depot_acte::verifier_preuve` : fenetre, terminal sans echo, sinon refus) ; dans `moteur-atelier` tout envoi reel est refuse
//! (`ActeReelRefuse`). Aucun mot de passe en option ni dans une sortie ; `--fenetre` est accepte et sans effet. `qui` = `ligne de
//! commande`, comme le Python. Un dossier `--ensemble` ne part a personne.
//!
//! DEFAUTS DU PYTHON, QUI NE SE REPRODUISENT PAS : une panne de la boite en cours d'envoi sortait en trace d'appels. Ici :
//! `! phrase`, code 2.

use serde_json::{json, Map, Value};

use super::actes::{est_refus_d_envoi, ouvrir_la_boite, reponse_metier, textes_de};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::champ;
use super::Cadre;
use crate::depot_acte::Preuve;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "dossier",
    positionnels: &[Pos::plusieurs("salaries")],
    options: &[
        Opt::valeur("mois"),
        Opt::valeur("annee"),
        Opt::drapeau("toutes-les-annees"),
        Opt::ajout_parmi("seulement", &crate::routes::dossier_salarie::CATEGORIES),
        Opt::drapeau("ensemble"),
        Opt::drapeau("apercu"),
        Opt::drapeau("envoyer"),
        Opt::drapeau("fenetre"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) if est_refus_d_envoi(e.classe) => reponse_metier("dossier", false, &e, &[]),
        Err(e) => cadre.plantage(e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    // Les cles que le Python lit par `or None` / `bool` : une valeur vide ou fausse n'est pas passee.
    let mut demande = Map::new();
    let salaries = args.liste("salaries");
    if !salaries.is_empty() {
        demande.insert("salaries".into(), json!(salaries));
    }
    for cle in ["mois", "annee"] {
        if let Some(v) = args.texte(cle).filter(|v| !v.is_empty()) {
            demande.insert(cle.into(), json!(v));
        }
    }
    for (option, cle) in [("toutes-les-annees", "toutes_les_annees"), ("ensemble", "ensemble"), ("apercu", "apercu")] {
        if args.drapeau(option) {
            demande.insert(cle.into(), json!(true));
        }
    }
    let seulement = args.liste("seulement");
    if !seulement.is_empty() {
        demande.insert("categories".into(), json!(seulement));
    }
    let d = crate::routes::dossier_salarie::construire(&cadre.ctx, &Value::Object(demande)).map_err(Echec::from)?;
    let r = crate::routes::dossier_salarie::dossier_json(&d);

    let salaries_du_dossier = textes_de(&r, "salaries");
    dire!("Dossier{} -- {}", if salaries_du_dossier.len() > 1 { "s" } else { "" }, champ(&r, "periode"));
    for cle in &salaries_du_dossier {
        let pages: Vec<String> = r
            .get("pages")
            .and_then(|p| p.get(cle))
            .and_then(Value::as_object)
            .map(|p| p.iter().map(|(c, n)| format!("{} {} p.", c, n.as_i64().unwrap_or(0))).collect())
            .unwrap_or_default();
        dire!("  {:<14} {}", cle, pages.join(", "));
    }
    for x in textes_de(&r, "refus") {
        dire!("  refuse : {}", x);
    }
    for a in textes_de(&r, "avertissements") {
        dire!("  ! {}", a);
    }
    for f in textes_de(&r, "fichiers") {
        dire!("  -> {}", f);
    }
    if !args.drapeau("envoyer") {
        return Ok(0);
    }

    ouvrir_la_boite(cadre);
    let ctx = &cadre.ctx;
    let envoi = crate::routes::envois::envoyer_dossier(ctx, &d, &Preuve::default(), "ligne de commande", &mut || crate::routes::envois::client_courriel(ctx))?;
    dire!("{} dossier(s) envoye(s) depuis {}, a {}.", champ(&envoi, "messages"), champ(&envoi, "expediteur"), textes_de(&envoi, "partis").join(", "));
    Ok(0)
}
