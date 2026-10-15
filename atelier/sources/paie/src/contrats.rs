//! Les natures de contrat (`moteur/contrats.py`) : ce que la DSN ecrit de la
//! STRUCTURE d'un contrat, lu dans `donnees/contrats-base.json`.
//!
//! Ce module ne decide rien et ne calcule rien : il lit le fichier, verifie
//! qu'il se tient, et rend des valeurs. Les refus gardent les messages du
//! Python. Non porte : `TYPES_CONTRAT_DPAE` hors du controle de lecture.

use crate::fichiers::{dossier_du_code, json_en_cache, repr, repr_valeur, signature, vrai};
use crate::num::Resultat;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Les faces de paie que le moteur sait calculer.
pub const PAIES: [&str; 2] = ["activite", "mensuelle"];

/// Les types de contrat de la DPAE (Format 120 v5.3, C_TYPE_CONTRAT).
pub const TYPES_CONTRAT_DPAE: [(&str, &str); 3] = [("1", "CDD"), ("2", "CDI"), ("3", "CTT")];

/// Une date civile (an, mois, jour) : le minimum pour `contrat_court`.
pub type Jour = (i32, u32, u32);

/// Une nature de contrat, telle que le fichier la decrit.
#[derive(Debug, Clone, PartialEq)]
pub struct Nature {
    pub cle: String,
    pub libelle: String,
    /// `"activite"` (cachets, heures) ou `"mensuelle"`.
    pub paie: String,
    pub duree_indeterminee: bool,
    /// S21.G00.40.011.
    pub unite_quotite: String,
    /// rubrique -> valeur fixe.
    pub codes: Map<String, Value>,
    /// rubrique -> d'ou la valeur vient.
    pub champs: Map<String, Value>,
    pub absents: Map<String, Value>,
    pub interdits: Map<String, Value>,
    pub fin_de_contrat: Map<String, Value>,
    pub dpae: Map<String, Value>,
    pub remunerations: Map<String, Value>,
    pub activites: Map<String, Value>,
    /// Le plafond de 40.012 et 40.013 : CCH-12 du bloc 40, 31.00 en journee ; CCH-13, 250.00 en heure.
    pub plafond_quotite: Option<f64>,
    pub quotite_reference_defaut: Option<f64>,
    /// La section `par_nature` du bareme a appliquer par-dessus le statut.
    pub bareme: Option<String>,
}

fn texte(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        autre => Some(autre.to_string()),
    }
}

impl Nature {
    /// Paye au mois : un permanent.
    pub fn mensuelle(&self) -> bool {
        self.paie == "mensuelle"
    }

    pub fn permanent(&self) -> bool {
        self.mensuelle()
    }

    /// La valeur fixe d'une rubrique pour cette nature, ou `None` (absente
    /// comme interdite : `interdit` les distingue).
    pub fn code(&self, rubrique: &str) -> Option<String> {
        self.codes
            .get(rubrique)
            .filter(|v| vrai(v))
            .and_then(|v| v.get("valeur"))
            .and_then(texte)
    }

    /// Le controle qui refuse cette rubrique pour cette nature, ou `None`.
    pub fn interdit(&self, rubrique: &str) -> Option<String> {
        self.interdits.get(rubrique).and_then(texte)
    }

    /// Cette nature ecrit-elle cette rubrique ? (valeur fixe ou champ)
    pub fn ecrit(&self, rubrique: &str) -> bool {
        self.codes.contains_key(rubrique) || self.champs.contains_key(rubrique)
    }

    /// Le motif de rupture quand le contrat va a son terme (`None` pour un CDI).
    pub fn motif_rupture_par_defaut(&self) -> Option<String> {
        self.fin_de_contrat
            .get("motif_par_defaut")
            .filter(|v| vrai(v))
            .and_then(|m| m.get("valeur"))
            .and_then(texte)
    }

    pub fn motifs_rupture(&self) -> Vec<Value> {
        liste(self.fin_de_contrat.get("motifs"))
    }

    /// Le refus d'un motif de rupture (`S21.G00.62.002`) que cette nature ne declare pas, ou `None`. Les `motifs` de
    /// `contrats-base.json` sont ceux que la CCH-11 de la rubrique autorise pour la nature (cahier technique DSN
    /// 2026.1, p. 270-273, `norme-dsn/texte/bloc-S21.G00.62.txt`) : un motif hors de la liste serait rejete (091 et 092,
    /// l'inaptitude, ne valent que pour les natures 01, 07, 08, 09, 50, 53, 54, 82, 91 ; un CDD rompu pour inaptitude
    /// est en 033). Le CDDU, de nature 02, a la liste du CDD (lot C1). Une nature sans liste n'est pas controlee.
    pub fn motif_rupture_refuse(&self, motif: &str, numero: &str) -> Option<String> {
        let motifs = self.motifs_rupture();
        let code = |m: &Value| m.get("code").and_then(texte).unwrap_or_default();
        if motifs.is_empty() || motifs.iter().any(|m| code(m) == motif) {
            return None;
        }
        let connus: Vec<String> =
            motifs.iter().map(|m| format!("{} ({})", code(m), m.get("libelle").and_then(texte).unwrap_or_default())).collect();
        Some(format!(
            "Contrat {} ({}) : le motif de rupture {} n'est pas autorise pour cette nature de contrat (CCH-11 de S21.G00.62.002, cahier technique DSN p. 270-273) ; la declaration serait rejetee. Motifs : {}. Rien n'est ecrit.",
            numero,
            self.libelle,
            motif,
            connus.join(", ")
        ))
    }

    /// Les motifs de recours (S21.G00.40.021) que cette nature accepte.
    pub fn motifs_recours(&self) -> Vec<Value> {
        liste(self.champs.get("S21.G00.40.021").and_then(|c| c.get("valeurs")))
    }

    pub fn motif_recours_exige(&self) -> bool {
        self.champs
            .get("S21.G00.40.021")
            .and_then(|c| c.get("obligatoire"))
            .map_or(false, vrai)
    }

    /// Le circuit derogatoire de la fin du contrat (`fin_de_contrat.circuit_derogatoire` : le CDDU), s'il y en a un.
    fn circuit_derogatoire(&self) -> Option<&Value> {
        self.fin_de_contrat.get("circuit_derogatoire").filter(|c| vrai(c))
    }

    /// La fin de ce contrat, pour ce motif de rupture (S21.G00.62.002 ; `None` ou vide : le defaut de la nature),
    /// part-elle par le circuit derogatoire ? Pour un CDDU, le seul 031 (CCH-12 de S21.G00.62.017, fiche 2062).
    pub fn circuit_derogatoire_pour(&self, motif: Option<&str>) -> bool {
        let Some(c) = self.circuit_derogatoire() else { return false };
        let motif = motif.filter(|m| !m.is_empty()).map(str::to_string).or_else(|| self.motif_rupture_par_defaut());
        motif.is_some_and(|m| liste(c.get("motifs")).iter().any(|x| texte(x).as_deref() == Some(m.as_str())))
    }

    /// LA REGLE, ET SON SEUL ENDROIT : la fin de ce contrat, pour ce motif de rupture (`None` ou vide : le defaut de la
    /// nature), se declare-t-elle par un signalement de fin de contrat (nature 07) ? Toute fin de contrat donne lieu a
    /// l'attestation employeur (R. 1234-9), par le FCTU en DSN (fiche net-entreprises 119) ; le CDDU n'en est dispense
    /// que par le circuit derogatoire, pour le 031 : « Tous les autres motifs de rupture doivent etre declares par
    /// l'emission d'un signalement FCTU » (fiche 2062, `atelier/reference/sources/net-entreprises-base-connaissances/
    /// net-entreprises-fiche2062-derogations-fin-de-contrat-fctu-20250905.txt`). Les listes des fins a signaler (ligne de
    /// commande, page, rappels), le FCTU et l'ecran de saisie la lisent ici.
    pub fn signalement_fctu(&self, motif: Option<&str>) -> bool {
        self.fin_de_contrat.get("signalement_fctu").map_or(false, vrai) && !self.circuit_derogatoire_pour(motif)
    }

    /// Les rubriques du bloc 62 que la nature fixe, pour ce motif de rupture : rubrique -> valeur. La modalite de
    /// declaration de la fin d'un contrat d'usage (S21.G00.62.017) suit le motif : « 01 » au circuit derogatoire,
    /// « 02 » hors de lui (CCH-11 et CCH-12, cahier technique DSN 2026.1 p. 277, `atelier/reference/norme-dsn/texte/
    /// bloc-S21.G00.62.txt`).
    pub fn codes_fin(&self, motif: Option<&str>) -> Map<String, Value> {
        let mut out = Map::new();
        if let Some(Value::Object(c)) = self.fin_de_contrat.get("codes") {
            for (r, v) in c {
                out.insert(r.clone(), v.get("valeur").cloned().unwrap_or(Value::Null));
            }
        }
        if let Some(c) = self.circuit_derogatoire() {
            let cle = if self.circuit_derogatoire_pour(motif) { "applique" } else { "non_applique" };
            if let Some(r) = c.get("rubrique").and_then(texte) {
                out.insert(r, c.get(cle).cloned().unwrap_or(Value::Null));
            }
        }
        out
    }

    /// C_TYPE_CONTRAT du Format 120 : 1 CDD, 2 CDI, 3 CTT.
    pub fn type_contrat_dpae(&self) -> String {
        self.dpae
            .get("type_contrat")
            .filter(|v| vrai(v))
            .and_then(|t| t.get("valeur"))
            .filter(|v| vrai(v))
            .and_then(texte)
            .unwrap_or_default()
    }

    /// La DPAE de ce contrat porte-t-elle une date de fin ?
    pub fn date_fin_dpae(&self) -> bool {
        self.dpae
            .get("date_fin")
            .and_then(|d| d.get("obligatoire"))
            .map_or(false, vrai)
    }

    pub fn types_remuneration(&self) -> Vec<Value> {
        liste(self.remunerations.get("types"))
    }

    /// Les quatre blocs 51 valent-ils tous le brut ? (vrai sur un CDDU)
    pub fn remunerations_au_brut(&self) -> bool {
        self.remunerations.get("toutes_au_brut").map_or(false, vrai)
    }

    pub fn unites_activite(&self) -> Vec<Value> {
        liste(self.activites.get("unites"))
    }

    pub fn unite_absence(&self) -> Option<Value> {
        self.activites.get("absences").filter(|v| !v.is_null()).cloned()
    }
}

fn liste(v: Option<&Value>) -> Vec<Value> {
    v.and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

// ------------------------------------ l'abattement << contrats courts >> du PAS

pub const DUREE_CONTRAT_COURT_MOIS: u32 = 2;

fn jours_du_mois(an: i32, mois: u32) -> u32 {
    match mois {
        4 | 6 | 9 | 11 => 30,
        2 if (an % 4 == 0 && an % 100 != 0) || an % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// La meme quantieme, n mois plus tard, ramenee au dernier jour du mois.
fn dans_n_mois(depuis: Jour, mois: u32) -> Jour {
    let rang = depuis.1 - 1 + mois;
    let an = depuis.0 + (rang / 12) as i32;
    let m = rang % 12 + 1;
    (an, m, depuis.2.min(jours_du_mois(an, m)))
}

/// L'abattement contrats courts s'applique-t-il a ce versement ? `debut` et
/// `fin` sont les bornes du CONTRAT SIGNE (`fin` a `None` : terme imprecis),
/// `le` la date du versement.
pub fn contrat_court(nat: &Nature, debut: Jour, fin: Option<Jour>, le: Jour) -> bool {
    if nat.duree_indeterminee {
        return false;
    }
    let terme = dans_n_mois(debut, DUREE_CONTRAT_COURT_MOIS);
    if let Some(f) = fin {
        if f > terme {
            return false;
        }
    }
    le <= terme
}

// ------------------------------------------------------------------- lecture

fn construire(cle: &str, d: &Map<String, Value>) -> Resultat<Nature> {
    let manquants: Vec<&str> =
        ["libelle", "paie", "codes"].into_iter().filter(|c| !d.contains_key(*c)).collect();
    if !manquants.is_empty() {
        return Err(format!(
            "Nature de contrat {} : {} manque dans contrats-base.json.",
            repr(cle), manquants.join(", ")));
    }
    let paie = texte(&d["paie"]).unwrap_or_default();
    if !PAIES.contains(&paie.as_str()) {
        return Err(format!(
            "Nature de contrat {} : face de paie {} inconnue. Connues : {}.",
            repr(cle), repr_valeur(&d["paie"]), PAIES.join(", ")));
    }
    let objet = |c: &str| -> Map<String, Value> {
        d.get(c).and_then(|v| v.as_object()).cloned().unwrap_or_default()
    };
    let uq = objet("unite_quotite");
    let valeur_uq = uq.get("valeur").filter(|v| vrai(v));
    let Some(valeur_uq) = valeur_uq else {
        return Err(format!(
            "Nature de contrat {} : `unite_quotite` (S21.G00.40.011) manque. \
             L'unite de mesure de la quotite ne se devine pas -- 12 pour des \
             journees, 10 pour des heures.", repr(cle)));
    };
    let qref = objet("quotite_reference_defaut");
    let dp = objet("dpae");
    let tc = dp
        .get("type_contrat")
        .filter(|v| vrai(v))
        .and_then(|t| t.get("valeur"))
        .filter(|v| vrai(v))
        .and_then(texte)
        .unwrap_or_default();
    if !TYPES_CONTRAT_DPAE.iter().any(|(k, _)| *k == tc) {
        let connus: Vec<String> =
            TYPES_CONTRAT_DPAE.iter().map(|(k, v)| format!("{} {}", k, v)).collect();
        return Err(format!(
            "Nature de contrat {} : `dpae.type_contrat` vaut {}. Le Format \
             120 de la DPAE n'en connait que trois -- {}. Sans lui, \
             l'embauche partirait au type du gabarit, celui d'un CDD.",
            repr(cle), repr(&tc), connus.join(", ")));
    }
    let sans_commentaire = |m: Map<String, Value>| -> Map<String, Value> {
        m.into_iter().filter(|(r, _)| !r.starts_with('_')).collect()
    };
    let codes = d["codes"].as_object().cloned().unwrap_or_default();
    Ok(Nature {
        cle: cle.to_string(),
        libelle: texte(&d["libelle"]).unwrap_or_default(),
        paie,
        duree_indeterminee: d.get("duree_indeterminee").map_or(false, vrai),
        unite_quotite: texte(valeur_uq).unwrap_or_default(),
        plafond_quotite: uq.get("plafond").and_then(|v| v.as_f64()),
        quotite_reference_defaut: qref.get("valeur").and_then(|v| v.as_f64()),
        bareme: d.get("bareme").and_then(texte),
        codes: sans_commentaire(codes),
        champs: sans_commentaire(objet("champs")),
        absents: objet("absents"),
        interdits: objet("interdits"),
        fin_de_contrat: objet("fin_de_contrat"),
        remunerations: objet("remunerations"),
        activites: objet("activites"),
        dpae: dp,
    })
}

fn fichier() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join("contrats-base.json"))
}

/// La nature d'un engagement qui ne dit pas la sienne : le CDDU.
pub fn defaut() -> Resultat<String> {
    Ok(fichier()?
        .get("_defaut")
        .filter(|v| vrai(v))
        .and_then(texte)
        .unwrap_or_else(|| "cddu_spectacle".into()))
}

type Natures = Arc<Vec<Arc<Nature>>>;
static NATURES: OnceLock<Mutex<HashMap<String, ((u128, u64), Natures)>>> = OnceLock::new();

/// Toutes les natures, dans l'ordre du fichier.
pub fn natures() -> Resultat<Natures> {
    let chemin = dossier_du_code().join("contrats-base.json");
    let cle = chemin.display().to_string();
    let sig = signature(&chemin);
    let cache = NATURES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(s) = sig {
        if let Some((vue, n)) = cache.lock().unwrap().get(&cle) {
            if *vue == s {
                crate::fichiers::noter_si_besoin(&chemin);
                return Ok(n.clone());
            }
        }
    }
    let f = fichier()?;
    let mut out = Vec::new();
    let brutes = f.get("natures").and_then(|n| n.as_object()).ok_or_else(|| crate::fichiers::absent("natures"))?;
    for (c, d) in brutes {
        let d = d.as_object().cloned().unwrap_or_default();
        out.push(Arc::new(construire(c, &d)?));
    }
    let out = Arc::new(out);
    if let Some(s) = sig {
        cache.lock().unwrap().insert(cle, (s, out.clone()));
    }
    Ok(out)
}

/// Une nature par sa cle (`None` : la nature par defaut). Refuse une cle inconnue.
pub fn nature(cle: Option<&str>) -> Resultat<Arc<Nature>> {
    let cle = match cle {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => defaut()?,
    };
    let toutes = natures()?;
    if let Some(n) = toutes.iter().find(|n| n.cle == cle) {
        return Ok(n.clone());
    }
    let mut connues: Vec<&str> = toutes.iter().map(|n| n.cle.as_str()).collect();
    connues.sort();
    Err(format!(
        "Nature de contrat inconnue : {}. Connues : {}.", repr(&cle), connues.join(", ")))
}

/// Le manifeste de la source (titre, copie, date de releve).
pub fn source() -> Resultat<Value> {
    Ok(fichier()?.get("_source").filter(|v| vrai(v)).cloned().unwrap_or(Value::Object(Map::new())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_natures_utilisees_par_le_calcul() {
        let cddu = nature(None).unwrap();
        assert_eq!(cddu.cle, "cddu_spectacle");
        assert!(!cddu.mensuelle() && cddu.remunerations_au_brut() && cddu.bareme.is_none());
        let cdi = nature(Some("cdi")).unwrap();
        assert!(cdi.mensuelle() && cdi.duree_indeterminee && !cdi.remunerations_au_brut());
        assert_eq!(cdi.bareme.as_deref(), Some("permanent"));
        assert_eq!(cdi.quotite_reference_defaut, Some(151.67));
        assert_eq!(nature(Some("")).unwrap().cle, "cddu_spectacle");
    }

    /// CCH-18 du 40.005 (cahier technique 2026.1, p. 185, `bloc-S21.G00.40.txt` l. 325-329) : « 999SPT pour les emplois
    /// permanents du spectacle » ; un CDDU garde le code de la nomenclature.
    #[test]
    fn la_nature_fixe_999spt_pour_les_permanents_seulement() {
        for cle in ["cdi", "cdd"] {
            assert_eq!(nature(Some(cle)).unwrap().code("S21.G00.40.005").as_deref(), Some("999SPT"), "{}", cle);
        }
        assert_eq!(nature(Some("cddu_spectacle")).unwrap().code("S21.G00.40.005"), None);
    }
}
