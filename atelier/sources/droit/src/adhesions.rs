//! Les contrats d'organismes (les adhesions de prevoyance et de sante, bloc S21.G00.15 de la DSN) sont dates : un
//! contrat porte `depuis` (le premier jour ou il vaut) et, s'il y a lieu, `jusqu_a` (le dernier). Le contrat de
//! Thalie Sante des intermittents, par exemple, n'existe qu'a partir du 01/01/2026 : la fiche de parametrage Audiens
//! version 139 le dit (`ParametresContrats DateDebutValidite="01012026"`, contrat `TSINTERMITTENTS` ;
//! `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`, l. 305-307), et
//! `logiciel/donnees/organismes-base.json` le porte (`depuis`).
//!
//! UN SEUL CRITERE, partout : un contrat vaut pour une periode qu'il couvre, au moins un jour. Le calcul et la DSN
//! d'un mois ne lisent que ceux-la, une affiliation (bloc S21.G00.70) ne rattache qu'a eux, et la Configuration ne
//! propose un contrat que pour la periode qu'il couvre. Un contrat qui commence dans la periode la couvre : son
//! affiliation se date de son premier jour (S21.G00.70.014, `dsn::rattachements`).

use std::sync::{Arc, OnceLock};

use crate::fiche::Lue;
use socle::date::Jour;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::forme::Nombre;
use socle::num::Resultat;

use crate::organismes::Adhesion;
use crate::source::forme_notee;

/// Le contrat qui court de `depuis` a `jusqu_a` (l'un ou l'autre absent : sans borne) couvre-t-il au moins un jour de
/// la periode `debut`..=`fin` ?
pub fn en_vigueur(depuis: Option<Jour>, jusqu_a: Option<Jour>, debut: &Jour, fin: &Jour) -> bool {
    depuis.is_none_or(|d| d <= *fin) && jusqu_a.is_none_or(|j| j >= *debut)
}

forme_notee! {
    /// `casc-svp-base.json`, entier : la CASC-SVP de la 3090, ce que la DSN en declare et sa regularisation hors DSN.
    pub struct CascSvp {
        pub idcc: String,
        /// Le premier mois declare en DSN, `AAAA-MM`.
        pub depuis: String,
        pub taux: TauxCasc,
        pub reglement: Reglement,
        pub courriel_audiens: CourrielCasc,
        pub courriel_casc: CourrielCasc,
        /// La page employeurs du CASC-SVP.
        pub page: String,
        pub dsn: CascDsn,
    }
}

forme_notee! {
    /// Le taux de la cotisation, en pourcentage, par annee de salaires (`AAAA`).
    pub struct TauxCasc {
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub par_annee: Vec<(String, Nombre)>,
    }
}

impl TauxCasc {
    /// Le taux de l'annee `an`, s'il est releve.
    pub fn de(&self, an: &str) -> Option<socle::num::Decimal> {
        self.par_annee.iter().find(|(a, _)| a == an).map(|(_, t)| t.0)
    }
}

forme_notee! {
    /// Le virement de la regularisation : a qui, et son libelle (`{siret}`, `{annee}` s'y remplacent).
    pub struct Reglement {
        pub beneficiaire: String,
        pub libelle: String,
    }
}

/// Un courriel de la regularisation : a qui, et son objet (`{siret}` s'y remplace).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourrielCasc {
    pub a: String,
    pub objet: String,
}

forme_notee! {
    /// L'adhesion CASCSVP de la DSN : la reference du contrat (15.001), l'organisme (15.002), l'identifiant de
    /// l'adhesion (15.005), et les lignes du bulletin qui font sa cotisation.
    pub struct CascDsn {
        pub contrat: String,
        pub organisme: String,
        pub identifiant: String,
        pub lignes_bulletin: Vec<String>,
    }
}

/// LA LECTURE DE `casc-svp-base.json`, la seule, une fois tant que le fichier ne change pas.
pub fn casc_svp_base() -> Resultat<Arc<CascSvp>> {
    static CACHE: OnceLock<FormeGardee<CascSvp>> = OnceLock::new();
    forme_en_cache(&CACHE, "casc-svp-base.json", "La table livrée de la CASC-SVP ne se lit pas : le signaler au développeur.", Ok)
}

/// L'ADHESION CASC-SVP d'une compagnie de la 3090 qui n'en est pas exemptee (`bareme::casc_exemptee`), ou `None`.
/// Elle est entiere dans `casc-svp-base.json` (`dsn` : `contrat`, `organisme`, `identifiant`) : la fiche de
/// codification la donne a l'identique a toute compagnie (S21.G00.15.001 CASCSVP, S21.G00.15.002 P0983 ;
/// `atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt`), et l'identifiant technique (15.005) est
/// interne au message DSN (`atelier/reference/norme-dsn/texte/bloc-S21.G00.15.txt`, l. 112-117). La compagnie n'en
/// saisit donc rien : `Contexte::organismes` la porte meme quand `config/organismes.json` ne la nomme pas.
pub fn casc_svp(ent: &crate::fiche::ChampsEntreprise) -> Resultat<Option<Adhesion>> {
    let base = casc_svp_base()?;
    if ent.idcc.texte().trim() != base.idcc.trim() || crate::bareme::casc_exemptee(ent)? {
        return Ok(None);
    }
    let d = &base.dsn;
    Ok(Some(Adhesion {
        cle: "casc".into(),
        libelle: d.contrat.clone(),
        delegataire: d.organisme.clone(),
        r#type: "02".into(),
        reference: d.identifiant.clone(),
        contribution: None,
        depuis: None,
        jusqu_a: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(s: &str) -> Jour {
        Jour::lire(s).unwrap()
    }

    /// Thalie Sante des intermittents n'existe qu'a partir du 01/01/2026 (`DateDebutValidite="01012026"`,
    /// `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`, l. 305) : decembre 2025 ne la
    /// connait pas, janvier 2026 si ; un contrat clos le 30/06 ne couvre plus juillet ; un contrat ouvert le 15 couvre
    /// le mois ou il s'ouvre.
    #[test]
    fn un_contrat_ne_vaut_que_pour_la_periode_qu_il_couvre() {
        let thalie = Some(j("2026-01-01"));
        assert!(!en_vigueur(thalie, None, &j("2025-12-01"), &j("2025-12-31")));
        assert!(en_vigueur(thalie, None, &j("2026-01-01"), &j("2026-01-31")));
        let clos = (Some(j("2025-01-01")), Some(j("2026-06-30")));
        assert!(en_vigueur(clos.0, clos.1, &j("2026-06-01"), &j("2026-06-30")));
        assert!(!en_vigueur(clos.0, clos.1, &j("2026-07-01"), &j("2026-07-31")));
        assert!(en_vigueur(Some(j("2026-03-15")), None, &j("2026-03-01"), &j("2026-03-31")));
        assert!(en_vigueur(None, None, &j("2025-01-01"), &j("2025-01-31")));
    }

    /// L'adhesion CASC-SVP d'une 3090 est celle de la fiche de codification, sans rien de la compagnie
    /// (`atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt` : 15.001 CASCSVP, 15.002 P0983) ; une
    /// autre convention, ou une 3090 exemptee (CCN 3090, art. 4.22 B), n'en a pas.
    #[test]
    fn la_casc_svp_d_une_3090_ne_se_saisit_pas() {
        let fiche = |texte: &str| socle::forme::lire_texte::<crate::fiche::ChampsEntreprise>(texte, "la fiche").unwrap();
        let a = casc_svp(&fiche(r#"{"idcc": "3090"}"#)).unwrap().unwrap();
        assert_eq!((a.libelle.as_str(), a.delegataire.as_str()), ("CASCSVP", "P0983"));
        assert!(casc_svp(&fiche(r#"{"idcc": "1285"}"#)).unwrap().is_none());
        let exemptee = fiche(r#"{"idcc": "3090", "casc_svp_exemptee": true, "representation_personnel": "cse"}"#);
        assert!(casc_svp(&exemptee).unwrap().is_none());
    }
}
