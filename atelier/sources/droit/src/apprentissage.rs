//! La taxe d'apprentissage : la fiche entreprise dit-elle un motif de non-assujettissement (table MNATA), et vaut-il pour
//! la periode payee ? Le bareme du contrat, le bordereau et le bloc 44 de la DSN lisent cette meme decision.

use std::sync::OnceLock;

use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::num::Resultat;

use crate::fiche::ChampsEntreprise;
use crate::source::forme_notee;

forme_notee! {
    /// `donnees/mnata-base.json` : la table MNATA de la DSN (S21.G00.44.004), relevee dans les nomenclatures de
    /// net-entreprises.
    pub struct TableMnata {
        pub motifs: Vec<MotifMnata>,
    }
}

/// Un motif de non-assujettissement : son code et son libelle.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotifMnata {
    pub code: String,
    pub libelle: String,
}

/// Le champ de la fiche entreprise qui dit pourquoi la compagnie ne doit pas la taxe d'apprentissage (un code MNATA).
pub const CHAMP_MOTIF_TAXE_APPRENTISSAGE: &str = "taxe_apprentissage_motif_non_assujettissement";
/// Le motif MNATA 007, « Associations sans but lucratif ».
pub const MNATA_ASSOCIATIONS: &str = "007";
/// Le premier millesime ou une association a activites non lucratives est redevable de la taxe d'apprentissage : C. trav.
/// L. 6241-1, I, redaction en vigueur depuis le 21/02/2026 (loi n° 2026-103 de finances pour 2026, art. 135, XIII) :
/// « En sont également redevables les associations [...] à activités non lucratives » (deposee :
/// `atelier/reference/sources/code-du-travail/xml/L6241-1.xml`). La redaction precedente (16/02/2025 - 21/02/2026,
/// `code-du-travail/xml-anterieur/L6241-1-v20250216.xml`) ne les nommait pas. Le millesime 2026 couvre des salaires d'avant
/// le 21/02/2026 : l'association y est redevable pour le reste de l'annee, elle declare 001 (INFERENCE).
pub const MILLESIME_ASSOCIATIONS_REDEVABLES: i32 = 2026;
/// La premiere periode d'emploi ou une association a activites non lucratives paie la taxe d'apprentissage : mars 2026.
/// Guide Urssaf des contributions de formation et d'apprentissage, version du 12/03/2026
/// (`atelier/reference/sources/majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt`,
/// l. 1166-1176 et 1192-1196) : « À compter de la période d'emploi de mars 2026, conformément au XIII de l'article 135
/// de la loi n° 2026-103 du 19 février 2026 de finances pour 2026 modifiant l'article L6241-1 du code du travail, la taxe
/// d'apprentissage est également due par les associations [...] à activités non lucratives ». Les salaires de
/// janvier et fevrier 2026 n'en portent pas ; le millesime 2026 la porte de mars a decembre, et se declare 001.
pub const PREMIERE_PERIODE_ASSOCIATIONS_REDEVABLES: (i32, u32) = (2026, 3);

/// La table MNATA livree : (code, libelle).
pub fn motifs_mnata() -> Resultat<Vec<(String, String)>> {
    static CACHE: OnceLock<FormeGardee<TableMnata>> = OnceLock::new();
    let t = forme_en_cache(&CACHE, "mnata-base.json", "La table MNATA livrée ne se lit pas : le signaler au développeur.", Ok)?;
    Ok(t.motifs.iter().map(|m| (m.code.trim().to_string(), m.libelle.trim().to_string())).collect())
}

/// Ou la fiche entreprise dit le motif de non-assujettissement a la taxe d'apprentissage.
pub fn champ_du_motif() -> String {
    format!("Configuration › Entreprise, « Taxe d'apprentissage : motif de non-assujettissement » (`{}`)", CHAMP_MOTIF_TAXE_APPRENTISSAGE)
}

/// Le motif MNATA que dit la fiche entreprise, avec son libelle, lu dans la table livree : `None` quand la fiche n'en dit
/// pas ; refus quand il n'est pas dans la table (la DSN n'admet que ses valeurs, CRE-11 de 44.004).
pub fn motif_de_la_fiche(ent: &ChampsEntreprise) -> Resultat<Option<(String, String)>> {
    let motif = ent.taxe_apprentissage_motif_non_assujettissement.code();
    if motif.is_empty() {
        return Ok(None);
    }
    let motifs = motifs_mnata()?;
    match motifs.iter().find(|(c, _)| *c == motif) {
        Some(m) => Ok(Some(m.clone())),
        None => Err(format!(
            "Le motif de non-assujettissement a la taxe d'apprentissage de la fiche (« {} ») n'est pas dans la table MNATA \
de la DSN (S21.G00.44.004 : {}). {}. Rien n'est ecrit.",
            motif,
            motifs.iter().map(|(c, l)| format!("{} {}", c, l)).collect::<Vec<_>>().join(" ; "),
            champ_du_motif()
        )),
    }
}

/// La taxe d'apprentissage des salaires de l'annee `an` est-elle hors du bordereau Urssaf ? Oui quand la fiche entreprise
/// dit un motif de non-assujettissement (table MNATA) qui vaut pour cette annee : un motif dit que la taxe n'est pas due.
/// Le 007 (associations) ne vaut que jusqu'au millesime 2025
/// (`MILLESIME_ASSOCIATIONS_REDEVABLES`, INFERENCE ecrite la-bas) : une association la paie pour 2026. Le
/// bordereau de chaque mois (`dsn::lignes_bordereau`, `dsn::lignes_regularisees`), le solde d'avril
/// (`dsn::solde_taxe_apprentissage`) et le bloc 44 de decembre (`blocs`) lisent cette meme decision : le fichier des
/// organismes garde ses lignes 992/993, comme ses lignes qui dependent du statut ou de l'effectif.
pub fn taxe_apprentissage_non_due(ent: &ChampsEntreprise, an: i32) -> Resultat<bool> {
    Ok(match motif_de_la_fiche(ent)? {
        Some((code, _)) => !(code == MNATA_ASSOCIATIONS && an >= MILLESIME_ASSOCIATIONS_REDEVABLES),
        None => false,
    })
}

/// La taxe d'apprentissage des salaires de la periode d'emploi `an`-`mois` est-elle due ? Non quand la fiche dit un
/// motif de non-assujettissement (table MNATA) qui vaut pour ce mois. Le 007 (associations) vaut jusqu'a la periode de
/// fevrier 2026 (`PREMIERE_PERIODE_ASSOCIATIONS_REDEVABLES`, guide Urssaf du 12/03/2026). C'est la decision du MOIS,
/// et la seule : le bareme du contrat (`bareme::bareme_taxe_apprentissage` : la ligne du bulletin et la cotisation
/// individuelle 130 du bloc 81) et le bordereau (`dsn::lignes_bordereau`, `dsn::lignes_regularisees` : CTP 992 et 993)
/// la lisent ici, pour que les trois faces disent la meme chose.
pub fn taxe_apprentissage_non_due_au_mois(ent: &ChampsEntreprise, an: i32, mois: u32) -> Resultat<bool> {
    Ok(match motif_de_la_fiche(ent)? {
        Some((code, _)) => !(code == MNATA_ASSOCIATIONS && (an, mois) >= PREMIERE_PERIODE_ASSOCIATIONS_REDEVABLES),
        None => false,
    })
}

