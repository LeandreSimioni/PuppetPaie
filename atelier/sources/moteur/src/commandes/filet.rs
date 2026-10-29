//! `moteur filet [etat|activer ADRESSE|verifier|desactiver|preparer|suivies]` : la DPAE preparee d'avance et son filet de securite.
//! Les routes `filet-etat`, `filet-suivies`, `filet-activer`, `filet-verifier`, `filet-desactiver` et
//! `filet-preparer` font le travail.
//!
//! GARDE-FOUS. `activer` et `desactiver` engagent (un compte chez le serveur du filet, des reveils) : la route exige le mot de passe
//! du logiciel (`filet::exiger_mot_de_passe`) et, sans `motdepasse` dans la charge, le MOTEUR le demande lui-meme a un humain
//! (fenetre, terminal sans echo), sinon il refuse -- jamais un mot de passe en option ni en variable. Dans `moteur-atelier`, hors
//! compagnie fictive, ils sont refuses. `verifier` ne lit que l'etat chez le serveur du filet (l'adresse vient de la configuration de la
//! compagnie, comme celle du serveur des incidents), sans secret. `preparer` teste les DPAE a venir EN TEST chez l'Urssaf (jamais un
//! depot reel, donc sans preuve, comme la veille) : dans `moteur-atelier`, il exige le faux Urssaf local. `qui` : `ligne de commande`.
//!
//! Une `ErreurOperation` rend `! phrase`, code 1 (le refus du mot de passe, de l'atelier, du serveur).
//! Une panne (reseau, disque) passe par `Cadre::plantage` : une ligne sur la sortie
//! d'erreur, une ligne au journal des incidents, code 1.

use serde_json::{json, Value};

use super::actes::{est_operation, refuser_dans_l_atelier, refuser_si_pas_redirige, Organisme};
use super::analyse::{Args, Opt, Pos, Spec};
use super::rendu::{champ, texte};
use super::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "filet",
    positionnels: &[
        Pos::parmi("quoi", &["etat", "activer", "verifier", "desactiver", "preparer", "suivies"], Some("etat")),
        Pos::facultatif("adresse", None),
    ],
    options: &[Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let quoi = args.texte("quoi").unwrap_or("etat");
    match faire(cadre, quoi, args) {
        Ok(c) => c,
        // `except ops.ErreurOperation as e: print("! %s" % e); return 1` : un refus, pas un plantage.
        Err(e) if est_operation(e.classe) => {
            dire!("! {}", e.message);
            1
        }
        Err(e) => cadre.plantage(e),
    }
}

fn afficher(e: &Value) {
    if !e.get("actif").is_some_and(socle::fichiers::vrai) {
        dire!("Filet de securite DPAE : non active.");
        dire!("  moteur filet activer adresse@exemple.fr");
        return;
    }
    dire!("Filet de securite DPAE : {}", champ(e, "adresse"));
    dire!("  {}", if e.get("confirme").is_some_and(socle::fichiers::vrai) { "adresse confirmee" } else { "adresse PAS ENCORE confirmee : ouvrir le lien recu par courriel" });
    dire!("  {} reveil(s) arme(s) -- serveur {}", champ(e, "reveils"), champ(e, "serveur"));
    if e.get("erreur").is_some_and(socle::fichiers::vrai) {
        dire!("  ! {}", champ(e, "erreur"));
    }
}

fn faire(cadre: &Cadre, quoi: &str, args: &Args) -> Result<i32, Echec> {
    match quoi {
        "suivies" => {
            let r = cadre.route("filet-suivies", json!({}))?;
            let liste: Vec<Value> = r.get("suivies").and_then(Value::as_array).cloned().unwrap_or_default();
            if liste.is_empty() {
                dire!("Aucune embauche sans trace de DPAE.");
            }
            for e in &liste {
                let parti = e.get("contrat_parti").is_some_and(socle::fichiers::vrai);
                dire!("  {}  {:<28} {}{}", texte(e.get("debut").unwrap_or(&Value::Null)), champ(e, "nom"), champ(e, "spectacle"), if parti { "" } else { "  (contrat pas encore parti)" });
            }
            Ok(0)
        }
        "activer" => {
            let Some(adresse) = args.texte("adresse").filter(|a| !a.is_empty()) else {
                dire!("L'adresse qui recevra le rappel : filet activer ADRESSE");
                return Ok(2);
            };
            refuser_dans_l_atelier(cadre, "d'activer le filet de securite DPAE")?;
            // Le mot de passe du logiciel : la route le fait demander par le moteur (un humain, ou un refus).
            afficher(&cadre.route("filet-activer", json!({"adresse": adresse}))?);
            dire!("Un courriel de confirmation est parti a {}.", adresse);
            Ok(0)
        }
        "desactiver" => {
            refuser_dans_l_atelier(cadre, "de desactiver le filet de securite DPAE")?;
            afficher(&cadre.route("filet-desactiver", json!({}))?);
            Ok(0)
        }
        "verifier" => {
            afficher(&cadre.route("filet-verifier", json!({}))?);
            Ok(0)
        }
        "preparer" => {
            // Le test d'une DPAE part chez l'Urssaf (en TEST) avec le mot de passe du coffre : jamais vers le vrai Urssaf depuis l'atelier.
            refuser_si_pas_redirige(cadre, Organisme::Dpae, "le test automatique des DPAE")?;
            // Le test lit le mot de passe de l'Urssaf au coffre : la cle du poste, ou une seance que le moteur demande lui-meme.
            cadre.ouvrir_le_coffre();
            let b = cadre.route("filet-preparer", json!({"qui": "ligne de commande"}))?;
            let n = |cle: &str| b.get(cle).and_then(Value::as_array).map_or(0, Vec::len);
            dire!("{} embauche(s) testee(s), {} reveil(s) arme(s), {} desarme(s).", n("testees"), n("armes"), n("desarmes"));
            let incidents: Vec<Value> = b.get("incidents").and_then(Value::as_array).cloned().unwrap_or_default();
            for i in &incidents {
                dire!("  ! {}", texte(i));
            }
            Ok(if incidents.is_empty() { 0 } else { 1 })
        }
        _ => {
            afficher(&cadre.route("filet-etat", json!({}))?);
            Ok(0)
        }
    }
}
