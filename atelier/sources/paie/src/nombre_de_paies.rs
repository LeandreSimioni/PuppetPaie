//! LE NOMBRE DE PAIES DE L'ANNEE, ENVOYE A L'EDITEUR LE 16 JANVIER (ticket #246 ; Leandre, 22/10/2026 : « le logiciel est
//! gratuit, sans abonnement ; c'est le seul moyen pour l'editeur de savoir combien il sert vraiment »).
//!
//! CE QUI PART : une fois par an et par compagnie, `{"annee": N-1, "paies": <nombre>}`, et le numero de poste que portent
//! deja les incidents. Rien d'autre : ni nom, ni SIREN, ni NIR, ni montant. Zero part aussi (zero est une information).
//!
//! LE COMPTE : les paies VALIDEES dont le mois de paie est dans l'annee N-1, intermittents et permanents confondus : une
//! par mois que chaque engagement valide couvre (`engagements/`, `valide.paies`). Un mois tenu avant ce logiciel (ticket
//! #215) n'a pas d'engagement : PuppetPaie n'a pas fait ces paies, elles ne comptent pas. Des engagements qui ne se
//! lisent pas font REFUSER le compte (regle 1 : rien ne se devine) : rien ne part, et le passage suivant recommence.
//!
//! QUAND : a partir du 16 janvier (l'horloge du moteur, `PUPPETPAIE_AUJOURDHUI` dans les essais), a l'ouverture ou au
//! passage de la veille (`ouverture::ouverture`, avec les incidents), le premier ce jour-la ou apres si le poste etait
//! eteint. Seule l'annee N-1 est due : une annee plus ancienne qui n'est jamais partie ne part plus.
//!
//! PAR OU : le canal des incidents (`incidents::canal`, l'identite du poste de `config/diagnostic.json`, la cle dans
//! l'en-tete), action `paies` du serveur de l'editeur ; SOUS LE MEME INTERRUPTEUR (`incidents::envoi_actif`) : coupe, rien ne
//! part.
//!
//! CE QUI EST RETENU : `config/diagnostic.json`, cle `paies_annuelles`, une entree par annee partie (`paies`, `poste`,
//! `parti_le`) ; une annee qui y est ne repart pas. Ce fichier est propre au poste (`compagnies::PROPRES_AU_POSTE`) : une
//! remise a zero de la compagnie ne fait pas renvoyer l'annee. Un envoi qui echoue (hors ligne, serveur en panne) n'ecrit
//! rien : le passage suivant le refait. L'ecran « Ce qui est parti chez l'editeur » le montre avec les incidents
//! (`routes_incidents::diagnostic_etat`).
//!
//! JAMAIS : depuis une compagnie fictive ni depuis la copie de developpement (le marqueur `COPIE-DE-DEVELOPPEMENT.json`,
//! `compagnies::est_copie`, que portent l'une et l'autre) ; depuis un logiciel bride (`bridage::est_bride`), ce qui couvre une
//! compagnie ajoutee sur un logiciel bride (#236) tant qu'il l'est. GARDEE AU DEBLOCAGE, elle est une compagnie reelle (son
//! marqueur est parti, `interface::suppression::garder`) : son nombre part, et ses paies validees en demo comptent -- le
//! logiciel les a faites ; qu'elles ne soient pas parties ne change pas la regle du compte.

use std::path::Path;

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;

use crate::incidents::{self, Canal};

/// La cle de `config/diagnostic.json` qui retient les annees parties.
pub const CLE: &str = "paies_annuelles";

/// Le jour de janvier a partir duquel l'annee passee part.
pub const JOUR_DE_JANVIER: u32 = 16;

/// L'annee dont le nombre est du `aujourdhui` : l'annee passee, a partir du 16 janvier ; aucune avant.
pub fn annee_due(aujourdhui: Jour) -> Option<i32> {
    (aujourdhui.mois > 1 || aujourdhui.jour >= JOUR_DE_JANVIER).then_some(aujourdhui.an - 1)
}

/// Le nombre de paies validees dont le mois de paie est dans `annee` (voir l'en-tete).
pub fn compter(ctx: &Contexte, annee: i32) -> R<usize> {
    let an = format!("{:04}-", annee);
    let mut n = 0;
    for o in engagement::stock::objets(&ctx.racine).map_err(socle::echec::Echec::from)? {
        let paies = o.brut.get("valide").and_then(|v| v.get("paies")).and_then(Value::as_object);
        n += paies.map_or(0, |p| p.keys().filter(|m| m.starts_with(&an)).count());
    }
    Ok(n)
}

/// Les annees deja parties, telles que `config/diagnostic.json` les retient.
pub fn parties(racine: &Path) -> Map<String, Value> {
    let p = racine.join("config").join("diagnostic.json");
    match crate::routes_fiches::lire_tolerant(&p) {
        Ok(Some(d)) => d.get(CLE).and_then(Value::as_object).cloned().unwrap_or_default(),
        _ => Map::new(),
    }
}

/// Retient que `annee` est partie, sans toucher au reste du fichier (l'identite du poste, l'interrupteur).
fn retenir(racine: &Path, annee: i32, paies: usize, poste: &str) -> Result<(), String> {
    let p = racine.join("config").join("diagnostic.json");
    let mut d = match crate::routes_fiches::lire_tolerant(&p) {
        Ok(Some(Value::Object(o))) => o,
        _ => Map::new(),
    };
    let mut annees = d.get(CLE).and_then(Value::as_object).cloned().unwrap_or_default();
    annees.insert(annee.to_string(), json!({"paies": paies, "poste": poste, "parti_le": incidents::maintenant()}));
    d.insert(CLE.into(), Value::Object(annees));
    socle::ecriture::ecrire_json(&p, &Value::Object(d), 2)
}

/// Pourquoi rien ne part de cette compagnie, ou `None` si elle peut envoyer.
fn empechement(racine: &Path) -> Option<&'static str> {
    if crate::interface::compagnies::est_fictive(racine) {
        Some("compagnie fictive")
    } else if crate::interface::compagnies::est_copie(racine) {
        Some("copie de developpement")
    } else if crate::bridage::est_bride() {
        Some("logiciel bride")
    } else if !incidents::envoi_actif(racine) {
        Some("envoi coupe")
    } else {
        None
    }
}

/// A l'ouverture et a chaque passage de la veille : envoie le nombre de paies de l'annee passee s'il est du et pas encore
/// parti. Ne leve jamais (l'ouverture n'echoue pas) : rend ce qui s'est passe, `{"annee", "envoye", "paies", "raison",
/// "erreur"}` -- `erreur` non vide quand le passage suivant recommencera.
pub fn envoyer_si_du(ctx: &Contexte) -> Value {
    let racine = ctx.racine.as_path();
    let bilan = |annee: Option<i32>, envoye: bool, paies: Option<usize>, raison: &str, erreur: &str| {
        json!({"annee": annee, "envoye": envoye, "paies": paies, "raison": raison, "erreur": erreur})
    };
    if let Some(r) = empechement(racine) {
        return bilan(None, false, None, r, "");
    }
    let Some(annee) = annee_due(ctx.aujourdhui) else {
        return bilan(None, false, None, "pas avant le 16 janvier", "");
    };
    if parties(racine).contains_key(&annee.to_string()) {
        return bilan(Some(annee), false, None, "deja parti", "");
    }
    let client = match incidents::canal(racine) {
        Ok(Canal::Pret(c)) => c,
        Ok(Canal::Aucun) => return bilan(Some(annee), false, None, "aucun serveur", ""),
        Ok(Canal::Panne(e)) => return bilan(Some(annee), false, None, "", &e.message),
        Err(e) => return bilan(Some(annee), false, None, "", &e.message),
    };
    let paies = match compter(ctx, annee) {
        Ok(n) => n,
        Err(e) => return bilan(Some(annee), false, None, "", &format!("compte refuse : {}", e.message)),
    };
    if let Err(e) = client.compter_paies(annee, paies) {
        return bilan(Some(annee), false, Some(paies), "", &e.message);
    }
    let poste = client.poste.as_ref().map(|(n, _)| n.clone()).unwrap_or_default();
    match retenir(racine, annee, paies, &poste) {
        Ok(()) => bilan(Some(annee), true, Some(paies), "", ""),
        // Parti, mais pas retenu : le passage suivant le renverra, et le serveur remplace la meme annee.
        Err(m) => bilan(Some(annee), true, Some(paies), "", &m),
    }
}

/// Ce que l'ecran « Ce qui est parti chez l'editeur » montre de chaque annee partie, la plus recente en tete : exactement ce
/// qui a quitte le poste (`contenu`), et quand.
pub fn pour_l_ecran(racine: &Path) -> Vec<Value> {
    let mut v: Vec<(String, Value)> = parties(racine).into_iter().collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter()
        .filter_map(|(annee, e)| {
            let an: i64 = annee.parse().ok()?;
            let paies = e.get("paies").and_then(Value::as_u64)?;
            let poste = e.get("poste").cloned().unwrap_or(Value::Null);
            Some(json!({
                "annee": an,
                "paies": paies,
                "parti_le": e.get("parti_le").cloned().unwrap_or(Value::Null),
                "contenu": {"annee": an, "paies": paies, "poste": poste},
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jour(an: i32, mois: u32, jour: u32) -> Jour {
        Jour { an, mois, jour }
    }

    #[test]
    fn l_annee_passee_est_due_a_partir_du_16_janvier() {
        assert_eq!(annee_due(jour(2027, 1, 1)), None);
        assert_eq!(annee_due(jour(2027, 1, 15)), None);
        assert_eq!(annee_due(jour(2027, 1, 16)), Some(2026));
        assert_eq!(annee_due(jour(2027, 2, 1)), Some(2026));
        assert_eq!(annee_due(jour(2027, 12, 31)), Some(2026));
    }
}
