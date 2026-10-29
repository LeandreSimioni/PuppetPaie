//! LES ECHEANCES D'UN ENGAGEMENT, tirees des textes deposes : la DPAE et la remise du contrat. Celle
//! de la DSN est dans `declarations::echeance_dsn`.
//!
//! - LA DPAE : « L'embauche d'un salarié ne peut intervenir qu'après déclaration nominative » (code du travail, L. 1221-10,
//!   `atelier/reference/sources/code-du-travail/articles-20261028.md`, `xml/L1221-10.xml`) : elle se fait avant le
//!   premier jour de l'engagement.
//! - LE CONTRAT : « Le contrat de travail est transmis au salarié, au plus tard, dans les deux jours ouvrables suivant
//!   l'embauche » (L. 1242-13, `atelier/reference/sources/code-du-travail/xml/L1242-13.xml`) ; une convention plus
//!   exigeante l'avance (`clauses-conventionnelles-base.json`, `remise_du_contrat` de sa couche : la 1285, titre XV).
//!
//! INFERENCE : un jour ouvrable est un jour du lundi au samedi qui n'est pas un ferie chome de la compagnie
//! (`conges::jours_ouvrables`, L. 3141-3) ; « dans les deux jours ouvrables suivant l'embauche » se compte a partir du
//! lendemain de l'embauche.

use serde_json::{json, Value};

use socle::date::Jour;
use socle::num::Resultat;

/// Une echeance : le jour, ce qui le precede (« avant le », « au plus tard le »), l'heure s'il y en a une (« à midi »),
/// et le texte depose qui la donne.
#[derive(Debug, Clone, PartialEq)]
pub struct Echeance {
    pub jour: Jour,
    pub mots: &'static str,
    pub heure: &'static str,
    pub source: String,
}

impl Echeance {
    /// « avant le 14/11/2026 », « avant le 15/12/2026 à midi ».
    pub fn texte(&self) -> String {
        Self::dire(self.mots, &self.jour.jj_mm_aaaa(), self.heure)
    }

    /// Sans l'annee, et « au plus tard le » dit « jusqu'au » (meme jour limite, inclus) : « avant le 14/11 »,
    /// « avant le 15/12 à midi », « jusqu'au 24/11 ». C'est ce qu'ecrit une case du panneau.
    pub fn court(&self) -> String {
        let mots = if self.mots == "au plus tard le" { "jusqu'au" } else { self.mots };
        Self::dire(mots, &self.jour.jj_mm(), self.heure)
    }

    fn dire(mots: &str, jour: &str, heure: &str) -> String {
        if heure.is_empty() {
            format!("{mots} {jour}")
        } else {
            format!("{mots} {jour} {heure}")
        }
    }

    pub fn en_json(&self) -> Value {
        json!({"jour": self.jour.iso(), "texte": self.texte(), "court": self.court(), "source": self.source})
    }
}

/// Le `n`-ieme jour ouvrable apres `depuis` (`n` > 0) ou avant lui (`n` < 0), `depuis` exclu.
pub fn jour_ouvrable(depuis: &Jour, n: i64, entreprise: Option<&Value>) -> Resultat<Jour> {
    let pas = if n < 0 { -1 } else { 1 };
    let (mut jour, mut reste) = (*depuis, n.abs());
    while reste > 0 {
        jour = jour.plus_jours(pas)?;
        if crate::conges::jours_ouvrables(&jour, &jour, entreprise)? == 1 {
            reste -= 1;
        }
    }
    Ok(jour)
}

/// La DPAE d'un engagement qui commence le `debut` : avant ce jour (L. 1221-10).
pub fn dpae(debut: &Jour) -> Echeance {
    Echeance {
        jour: *debut,
        mots: "avant le",
        heure: "",
        source: "Code du travail, art. L. 1221-10 (atelier/reference/sources/code-du-travail/xml/L1221-10.xml)".into(),
    }
}

/// La remise du contrat a duree determinee d'un engagement qui commence le `debut` : deux jours ouvrables apres
/// l'embauche (L. 1242-13), ou plus tot si la couche de la convention `idcc` le dit pour ce metier (`code_emploi`).
pub fn remise_du_contrat(debut: &Jour, idcc: &Value, code_emploi: &str, entreprise: Option<&Value>) -> Resultat<Echeance> {
    let loi = jour_ouvrable(debut, 2, entreprise)?;
    let mut e = Echeance {
        jour: loi,
        mots: "au plus tard le",
        heure: "",
        source: "Code du travail, art. L. 1242-13 (atelier/reference/sources/code-du-travail/xml/L1242-13.xml)".into(),
    };
    let Some(conv) = crate::clauses::releve(idcc)? else { return Ok(e) };
    let r = &conv["remise_du_contrat"];
    let code = code_emploi.trim().to_uppercase();
    let concerne = r["metiers"].as_array().is_some_and(|l| l.iter().any(|m| m.as_str() == Some(code.as_str())));
    if let (true, Some(n)) = (concerne, r["jours_ouvrables_avant"].as_i64()) {
        let jour = jour_ouvrable(debut, -n, entreprise)?;
        if jour < e.jour {
            e = Echeance {
                jour,
                mots: "au plus tard le",
                heure: "",
                source: format!(
                    "{}, art. {} ({})",
                    crate::clauses::nom(idcc),
                    r["article"].as_str().unwrap_or(""),
                    r["fichier"].as_str().unwrap_or("")
                ),
            };
        }
    }
    Ok(e)
}
