//! L'ETAT DE LA DSN D'UN MOIS, composé ici et nulle part ailleurs (l'accueil, le mois tenu avant, les rappels, la
//! clôture, l'échéancier, le panneau, le suivi d'un dépôt et le dépôt réel le lisent) : le journal des dépôts d'abord
//! (un dépôt que les retours attestent y entre de lui-même : `reprise_journal::reprendre_les_depots`) ; à défaut, la DSN
//! que l'import a lue pour ce mois (le fichier de l'ancien logiciel) -- le mois est déclaré, rien ne propose de la
//! refaire (règle 4 : une déclaration déposée ne se régénère pas, et PuppetPaie n'a pas calculé ce mois). Un mois tenu
//! avant sans aucune trace le dit, sans le dire « en retard » ni proposer de la produire.
//!
//! Ce que l'état veut dire (partie, déposée, à faire) se lit ici, par les méthodes d'[`EtatDsn`].

use serde_json::{json, Value};

use droit::contexte::Contexte;
use envois::etats;
use socle::echec::R;
use socle::fichiers::vrai;

/// L'état de la DSN d'un mois : `etat` (« passee », « attente », « refusee » : le journal, et `anomalies` pour une
/// refusée, `aer_recue` ce que France Travail en a renvoyé ; « importee » : la DSN de l'ancien logiciel ;
/// « sans_trace » : un mois tenu avant dont rien ne garde la DSN, qui ne se réclame pas ; « a_deposer » : un mois de ce
/// logiciel dont la DSN n'est pas partie, sans mots ici), `courte`, `phrase`, `quand`.
pub(crate) struct EtatDsn(pub(crate) Value);

impl EtatDsn {
    fn est(&self, etats: &[&str]) -> bool {
        self.0["etat"].as_str().is_some_and(|e| etats.contains(&e))
    }

    /// Une DSN est partie pour ce mois et n'est pas refusée : passée, retour attendu, ou déposée par l'ancien logiciel.
    pub(crate) fn partie(&self) -> bool {
        self.est(&["passee", "attente", "importee"])
    }

    /// Une DSN de ce logiciel est sur la plate-forme, refusée comprise.
    pub(crate) fn deposee(&self) -> bool {
        self.est(&["passee", "attente", "refusee"])
    }

    /// Le bilan l'a refusée.
    pub(crate) fn refusee(&self) -> bool {
        self.est(&["refusee"])
    }

    /// La DSN reste à faire : refusée, ou jamais partie d'un mois de ce logiciel.
    pub(crate) fn a_faire(&self) -> bool {
        self.est(&["refusee", "a_deposer"])
    }
}

/// L'état de la DSN de `mois` ; `nic` : l'établissement (`Null` : le principal ; les états du journal sont les siens).
pub(crate) fn du_mois(ctx: &Contexte, mois: &str, nic: &Value) -> R<EtatDsn> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let e = etats::etat_dsn_du_mois(&lignes, mois, nic, &droit::bareme::nic_principal(&ctx.entreprise))?;
    let quand = |e: &Value| etats::quand_lisible(e.get("quand").unwrap_or(&Value::Null));
    if vrai(&e) {
        let refusee = e.get("refusee").is_some_and(vrai);
        let passee = e.get("passee").is_some_and(vrai);
        let (etat, courte, phrase) = if refusee {
            ("refusee", "refusée".to_string(), format!("DSN déposée le {} — REFUSÉE par la plate-forme", quand(&e)))
        } else if passee {
            ("passee", "déposée — passée".to_string(), format!("DSN déposée le {} — passée", quand(&e)))
        } else {
            ("attente", "déposée — retour attendu".to_string(), format!("DSN déposée le {} — retour attendu", quand(&e)))
        };
        return Ok(EtatDsn(json!({
            "etat": etat, "courte": courte, "phrase": phrase, "quand": e.get("quand").cloned().unwrap_or(Value::Null),
            "anomalies": e.get("anomalies").cloned().unwrap_or(Value::Null),
            "aer_recue": e.get("aer_recue").cloned().unwrap_or(Value::Null),
        })));
    }
    if let Some(d) = crate::import::mois::dsn_importee(&ctx.racine, mois)? {
        let etablie = d["etablie_le"].as_str().unwrap_or_default();
        let jour = if etablie.len() == 10 { format!(" (établie le {}/{}/{})", &etablie[8..10], &etablie[5..7], &etablie[..4]) } else { String::new() };
        return Ok(EtatDsn(json!({
            "etat": "importee",
            "courte": "déclarée avant PuppetPaie",
            "phrase": format!(
                "DSN déclarée avant PuppetPaie : celle de l'ancien logiciel, importée{} — aucun bilan de la plate-forme n'est lu pour elle. Rien n'est à refaire ici.",
                jour
            ),
            "quand": Value::Null,
        })));
    }
    if !crate::import::mois::importe(&ctx.racine, mois)? {
        return Ok(EtatDsn(json!({"etat": "a_deposer", "quand": Value::Null})));
    }
    Ok(EtatDsn(json!({
        "etat": "sans_trace",
        "courte": "aucune trace de sa DSN",
        "phrase": "Mois tenu avant PuppetPaie : aucune trace de sa DSN — ni au journal, ni dans les retours relevés, ni parmi les DSN importées. Si elle est partie, le relevé des retours ou l'import de la DSN de l'ancien logiciel la retrouvera ; si elle n'est jamais partie, elle se fait dans le logiciel qui a calculé ces paies.",
        "quand": Value::Null,
    })))
}

/// LE PREMIER MOIS DONT LA COMPAGNIE DOIT UNE DSN, quand on le sait. Une compagnie neuve n'est en retard de rien : le
/// premier mois du est celui de sa premiere embauche (`premiere_embauche` de la fiche Entreprise) ; sans cette date, une
/// compagnie qui n'a ni paie saisie ni DSN au journal n'a encore rien a declarer avant le mois courant (des fiches ne
/// sont pas un passe). `None` : rien ne borne -- une compagnie qui a deja des paies ou des declarations.
pub(crate) fn premier_mois_du(ctx: &Contexte) -> R<Option<String>> {
    let embauche: String = ctx.entreprise.get("premiere_embauche").filter(|v| vrai(v)).map(socle::date::en_texte).unwrap_or_default().chars().take(7).collect();
    if !embauche.is_empty() {
        return Ok(Some(embauche));
    }
    if !engagement::stock::mois_saisis(&ctx.racine).unwrap_or_default().is_empty() {
        return Ok(None);
    }
    let journal = envois::journal::lu(&ctx.racine)?;
    if journal.lignes.iter().any(|l| l.get("quoi").and_then(Value::as_str) == Some(etats::DSN)) {
        return Ok(None);
    }
    Ok(Some(ctx.aujourdhui.mois_civil().to_string()))
}

/// LA DSN DE `mois` EST-ELLE DUE pour l'etablissement `nic` (`Null` : le principal) ? « La DSN est mensuelle et
/// produite par établissement employeur. Par principe, elle est générée par l'acte de paie et liée à la présence de
/// salariés [...]. A défaut, une DSN Sans individu devra être produite » (cahier technique DSN 2026.1, § 2.4,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2291-2309) : elle est due chaque mois,
/// avec ou sans salarie, a partir du premier mois du (`premier_mois_du`), tant qu'elle reste a faire
/// (`EtatDsn::a_faire`). LE SEUL ENDROIT QUI LE DIT : l'echeancier, les rappels et le panneau des etapes le lisent.
pub(crate) fn due(ctx: &Contexte, mois: &str, nic: &Value) -> R<bool> {
    if premier_mois_du(ctx)?.is_some_and(|p| mois < p.as_str()) {
        return Ok(false);
    }
    Ok(du_mois(ctx, mois, nic)?.a_faire())
}
