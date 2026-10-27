//! `paie dpae FICHIER [--heure HHMM] [--seulement SALARIE:AAAA-MM-JJ ...] [--force] [--json]` : produire la seule DPAE d'un mois,
//! avant l'embauche (`moteur/cli.py`, `cmd_dpae` ; `operations_declarations.produire_dpae`). Rien ne part : un fichier a plat
//! (990 caracteres par embauche), son archive et son recapitulatif, plus -- dans le meme geste -- les signalements d'amorcage
//! (nature 08) des embauches dont le taux de PAS manque. Le depot est une commande a part (`paie deposer-dpae`).
//!
//! Elle passe par la route `dpae` du moteur (`routes_actes_h1::dpae_detail`). La route lit les
//! engagements du mois (`AAAA-MM`, ou `engagements/AAAA-MM.json` des donnees) : un autre fichier est refuse en une phrase.
//!
//! Code de sortie 3 : au moins une embauche est deja passee, la DPAE est hors delai (elle s'ecrit quand meme, sans antidater).
//!
//! DEFAUT DU PYTHON, NE SE REPRODUIT PAS : une embauche deja declaree, un contrat en cours depuis un mois precedent, une
//! selection absente de la saisie, un fichier introuvable donnaient une trace d'appels. Ici : `! phrase`, code 2.

use serde_json::{json, Value};

use super::actes::{afficher_etapes, afficher_fichiers, est_operation, jj_mm_aaaa, reponse_metier};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, enveloppe, erreur, rendre};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "dpae",
    positionnels: &[Pos::requis("fichier")],
    options: &[Opt::avec_defaut("heure", "0800"), Opt::plusieurs("seulement"), Opt::drapeau("force"), Opt::drapeau("json")],
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
    // `--seulement FAVART:2026-08-23` : la DPAE d'une PARTIE du mois. Le fichier complet reste a cote, intact.
    let mut choix: Option<Vec<Value>> = None;
    let seulement = args.liste("seulement");
    if args.drapeau("seulement") {
        let mut liste = Vec::new();
        for x in &seulement {
            let Some((cle, debut)) = x.split_once(':') else {
                dire!("--seulement attend SALARIE:AAAA-MM-JJ, recu {}", socle::fichiers::repr(x));
                return Ok(2);
            };
            liste.push(json!({"salarie": cle.trim(), "debut": debut.trim()}));
        }
        choix = Some(liste);
    }
    let mois = match mois_de_la_saisie(cadre, args.texte("fichier").unwrap_or("")) {
        Ok(m) => m,
        Err(e) => return Ok(reponse_metier("dpae", json, &e, &[])),
    };
    let mut charge = json!({"mois": mois, "heure": args.texte("heure").unwrap_or("0800"), "forcer": args.drapeau("force")});
    if let Some(c) = choix {
        charge["paies"] = Value::Array(c);
    }
    let (r, amorcage, ecarte) = match crate::routes_actes_h1::dpae_detail(&cadre.ctx, &charge) {
        Ok(x) => x,
        Err(e) if est_operation(e.classe) => return Ok(reponse_metier("dpae", json, &e, &[])),
        Err(e) => {
            if json && matches!(e.classe, Some("FileNotFoundError") | Some("OSError")) {
                return Ok(rendre(&erreur("dpae", e.classe.unwrap_or("OSError"), &e.message, &[])));
            }
            return Err(e);
        }
    };
    let retard = r.get("retard").is_some_and(socle::fichiers::vrai);
    if json {
        rendre(&enveloppe("dpae", r));
        return Ok(if retard { 3 } else { 0 });
    }
    let lignes: Vec<Value> = r.get("lignes").and_then(Value::as_array).cloned().unwrap_or_default();
    dire!("\n{} - {} contrat(s) a declarer\n{}", champ(&r, "libelle"), lignes.len(), "-".repeat(58));
    for l in &lignes {
        let heure = champ(l, "heure");
        let (h, m) = (heure.chars().take(2).collect::<String>(), heure.chars().skip(2).collect::<String>());
        dire!("  {:<22} {} a {}h{}  {}", champ(l, "salarie"), jj_mm_aaaa(&champ(l, "debut")), h, m, champ(l, "libelle"));
    }
    dire!("{}", "-".repeat(58));

    // CE QUI PART AVEC LA DPAE : le signalement d'amorcage qui demande a la DGFiP le taux de PAS des embauches. Quand il ne part
    // pas, le dire -- << tous les taux sont valides >> est une reponse, pas un silence.
    if let Some(lignes_amorcage) = amorcage {
        dire!("\nTaux de prelevement a la source demandes (signalement d'amorcage, nature 08) :");
        for (salarie, libelle) in lignes_amorcage {
            dire!("  {:<22} {}", salarie, libelle);
        }
    } else if !ecarte.is_empty() {
        dire!("\nAucune demande de taux de PAS : {}", ecarte);
    }

    let dossier = champ(&r, "dossier");
    let fichiers: Vec<Value> = r.get("fichiers").and_then(Value::as_array).cloned().unwrap_or_default();
    afficher_fichiers(&fichiers, &dossier);
    let etapes: Vec<Value> = r.get("etapes").and_then(Value::as_array).cloned().unwrap_or_default();
    afficher_etapes(&etapes, "A faire :");
    if retard {
        dire!("\n>>> Au moins une embauche est deja passee : la DPAE est hors delai.");
        dire!(">>> La deposer quand meme, sans antidater.");
        return Ok(3);
    }
    Ok(0)
}
