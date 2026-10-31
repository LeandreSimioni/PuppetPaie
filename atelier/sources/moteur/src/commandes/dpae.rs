//! `moteur dpae FICHIER [--heure HHMM] [--seulement SALARIE:NUMERO ...] [--json]` : produire la seule DPAE d'un mois,
//! avant l'embauche. Rien ne part : un fichier a plat
//! (990 caracteres par embauche), son archive et son recapitulatif, plus -- dans le meme geste -- les signalements d'amorcage
//! (nature 08) des embauches dont le taux de PAS manque. Le depot est une commande a part (`moteur deposer-dpae`).
//!
//! Elle passe par la route `dpae` du moteur (`crate::routes::depots::dpae`). La route lit les
//! engagements du mois (`AAAA-MM`, ou `engagements/AAAA-MM.json` des donnees) : un autre fichier est refuse en une phrase.
//!
//! Une embauche deja declaree, un contrat en cours depuis un mois precedent, une selection absente de la saisie, un
//! fichier introuvable : `! phrase`, code 2.

use serde_json::{json, Value};

use super::actes::{afficher_fichiers, est_operation, reponse_metier};
use socle::date::jj_mm_aaaa;
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "dpae",
    positionnels: &[Pos::requis("fichier")],
    options: &[Opt::avec_defaut("heure", "0800"), Opt::plusieurs("seulement"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => cadre.plantage(e),
    }
}

/// Le mois que la commande vise (`actes::mois_du_fichier`).
fn mois_de_la_saisie(cadre: &Cadre, texte: &str) -> Result<String, Echec> {
    super::actes::mois_du_fichier(cadre, texte, "La DPAE se produit")
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let json = args.drapeau("json");
    // `--seulement FAVART:3` : la DPAE d'une PARTIE du mois (le salarie et le numero du contrat). Le fichier complet reste a cote, intact.
    let mut choix: Option<Vec<Value>> = None;
    let seulement = args.liste("seulement");
    if args.drapeau("seulement") {
        let mut liste = Vec::new();
        for x in &seulement {
            let Some((cle, numero)) = x.split_once(':') else {
                dire!("--seulement attend SALARIE:NUMERO, recu {}", socle::fichiers::repr(x));
                return Ok(2);
            };
            liste.push(json!({"salarie": cle.trim(), "numero": numero.trim()}));
        }
        choix = Some(liste);
    }
    let mois = match mois_de_la_saisie(cadre, args.texte("fichier").unwrap_or("")) {
        Ok(m) => m,
        Err(e) => return Ok(reponse_metier("dpae", json, &e, &[])),
    };
    let mut charge = json!({"mois": mois, "heure": args.texte("heure").unwrap_or("0800")});
    if let Some(c) = choix {
        charge["engagements"] = Value::Array(c);
    }
    let r = match crate::routes::depots::dpae(&cadre.ctx, &charge) {
        Ok(x) => x,
        Err(e) if est_operation(e.classe) => return Ok(reponse_metier("dpae", json, &e, &[])),
        Err(e) => {
            if json && matches!(e.classe, Some("FileNotFoundError") | Some("OSError")) {
                return Ok(rendre(&erreur("dpae", e.classe.unwrap_or("OSError"), &e.message, &[])));
            }
            return Err(e);
        }
    };
    if json {
        rendre(&enveloppe("dpae", r));
        return Ok(0);
    }
    let lignes: Vec<Value> = r.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    dire!("\n{} - {} contrat(s) a declarer\n{}", champ(&r, "libelle"), lignes.len(), "-".repeat(58));
    for l in &lignes {
        let heure = champ(l, "heure");
        let (h, m) = (heure.chars().take(2).collect::<String>(), heure.chars().skip(2).collect::<String>());
        dire!("  {:<22} {} a {}h{}", champ(l, "salarie"), jj_mm_aaaa(&champ(l, "debut")), h, m);
    }
    dire!("{}", "-".repeat(58));

    dire!("\n{}", champ(&r, "phrase"));
    let dossier = champ(&r, "dossier");
    let fichiers: Vec<Value> = r.get("fichiers").and_then(Value::as_array).cloned().unwrap_or_default();
    afficher_fichiers(&fichiers, &dossier);
    Ok(0)
}
