//! Ce que declare un fichier DSN, relu salarie par salarie : ses contrats (blocs S21.G00.40 et S21.G00.62), les
//! remunerations et activites qui s'y rattachent (S21.G00.51, S21.G00.53) et ses versements (S21.G00.50). Lecture
//! seule, sans reseau.

use std::collections::HashMap;

use socle::num::{decimal_lu, q2, Decimal};
use socle::texte::{blanc, lignes_du_texte, texte_latin1};

/// `format(d.normalize(), "f")`, avec `0` pour zero.
pub fn norm(d: Decimal) -> String {
    if d.is_zero() {
        "0".to_string()
    } else {
        d.normalize().to_string()
    }
}

/// JJMMAAAA -> AAAA-MM-JJ.
fn date_dsn(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    if c.len() == 8 {
        let t = |a: usize, b: usize| c[a..b].iter().collect::<String>();
        format!("{}-{}-{}", t(4, 8), t(2, 4), t(0, 2))
    } else {
        v.to_string()
    }
}

/// Ce qu'un contrat declare pour des periodes de paie : son brut (51 de type 001) et son activite (53 de type 01).
#[derive(Default)]
pub struct Mesures {
    pub brut: Decimal,
    pub jours: Decimal,
    pub cachets: Decimal,
    pub heures: Decimal,
    pub remunere: bool,
}

/// Un contrat du fichier. Un bloc 51 se rattache a sa periode de paie (S21.G00.51.001, « date de debut de periode a
/// laquelle la remuneration est rattachee », `norme-dsn/texte/bloc-S21.G00.51.txt` l. 38-40) : celui d'une periode
/// anterieure au mois principal declare (S20.G00.05.005) est le rappel d'une paie deja declaree (chapitre 2, §2.1.3.6,
/// `norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 607-616), pas la paie du mois.
#[derive(Default)]
pub struct Contrat {
    pub numero: String,
    pub debut: String,
    pub fin: String,
    pub emploi: String,
    pub nature: String,
    pub objet: String,
    /// La paie du mois principal declare.
    pub mois: Mesures,
    /// Les rappels des mois anterieurs.
    pub rappel: Mesures,
    pub fin_declaree: String,
    pub motif_fin: String,
}

#[derive(Default)]
pub struct Versement {
    pub date: String,
    pub net_verse: String,
    pub net_fiscal: String,
    pub taux_pas: String,
    pub montant_pas: String,
}

#[derive(Default)]
pub struct Individu {
    pub nir: String,
    pub nom: String,
    pub prenoms: String,
    pub contrats: Vec<Contrat>,
    pub versements: Vec<Versement>,
    pub par_numero: HashMap<String, (usize, usize)>,
}

/// Les montants en texte, sous la forme que le recapitulatif complete.
impl Mesures {
    pub fn brut_texte(&self) -> String {
        q2(self.brut)
    }
    pub fn mesure_texte(&self, cle: &str) -> String {
        norm(match cle {
            "jours" => self.jours,
            "cachets" => self.cachets,
            _ => self.heures,
        })
    }
}

impl Contrat {
    fn mesures(&mut self, rappel: bool) -> &mut Mesures {
        if rappel { &mut self.rappel } else { &mut self.mois }
    }
}

/// Les salaries du fichier, leurs contrats, leurs versements.
pub fn lire_recap(octets: &[u8]) -> Result<Vec<Individu>, String> {
    let texte = texte_latin1(octets);
    let mut sals: Vec<Individu> = Vec::new();
    let mut contrat: Option<(usize, usize)> = None;
    let mut versement: Option<(usize, usize)> = None;
    let mut fin: Option<(usize, usize)> = None;
    // Le premier jour du mois principal declare (S20.G00.05.005), AAAA-MM-JJ
    let mut principal = String::new();
    // (numero de contrat, type, rappel d'une periode anterieure) de la remuneration en cours
    let mut remu: Option<(String, String, bool)> = None;
    // (type, mesure) de l'activite en cours
    let mut activite: Option<(String, Decimal)> = None;
    for ligne in lignes_du_texte(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(blanc).trim_matches('\'');
        let kc: Vec<char> = k.chars().collect();
        let bloc: String = kc.iter().take(10).collect();
        let champ: String = kc.iter().skip(11).collect();
        if k == "S20.G00.05.005" {
            principal = date_dsn(v);
        }
        if bloc == "S21.G00.30" {
            if champ == "001" || sals.is_empty() {
                sals.push(Individu::default());
            }
            let s = sals.last_mut().unwrap();
            match champ.as_str() {
                "001" => s.nir = v.to_string(),
                "002" => s.nom = v.to_string(),
                "004" => s.prenoms = v.to_string(),
                _ => {}
            }
            continue;
        }
        if sals.is_empty() {
            continue;
        }
        let si = sals.len() - 1;
        match bloc.as_str() {
            "S21.G00.40" => {
                if champ == "001" {
                    sals[si].contrats.push(Contrat { debut: date_dsn(v), ..Contrat::default() });
                    contrat = Some((si, sals[si].contrats.len() - 1));
                } else if let Some((cs, cc)) = contrat {
                    match champ.as_str() {
                        "006" => sals[cs].contrats[cc].emploi = v.to_string(),
                        "007" => sals[cs].contrats[cc].nature = v.to_string(),
                        "009" => {
                            sals[cs].contrats[cc].numero = v.to_string();
                            sals[si].par_numero.insert(v.to_string(), (cs, cc));
                        }
                        "010" => sals[cs].contrats[cc].fin = date_dsn(v),
                        "050" => sals[cs].contrats[cc].objet = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.62" => {
                if champ == "001" && contrat.is_some() {
                    fin = contrat;
                    let (fs, fc) = fin.unwrap();
                    sals[fs].contrats[fc].fin_declaree = date_dsn(v);
                } else if champ == "002" {
                    if let Some((fs, fc)) = fin {
                        sals[fs].contrats[fc].motif_fin = v.to_string();
                    }
                }
            }
            "S21.G00.50" => {
                if champ == "001" {
                    sals[si].versements.push(Versement { date: date_dsn(v), ..Versement::default() });
                    versement = Some((si, sals[si].versements.len() - 1));
                } else if let Some((vs, vi)) = versement {
                    let x = &mut sals[vs].versements[vi];
                    match champ.as_str() {
                        "002" => x.net_fiscal = v.to_string(),
                        "004" => x.net_verse = v.to_string(),
                        "006" => x.taux_pas = v.to_string(),
                        "009" => x.montant_pas = v.to_string(),
                        _ => {}
                    }
                }
            }
            "S21.G00.51" => {
                if champ == "001" {
                    remu = Some((String::new(), String::new(), !principal.is_empty() && date_dsn(v) < principal));
                } else if let Some(r) = remu.as_mut() {
                    match champ.as_str() {
                        "010" => r.0 = v.to_string(),
                        "011" => r.1 = v.to_string(),
                        "013" => {
                            let montant = decimal_lu(v).ok_or_else(|| socle::num::illisible(v))?;
                            let (numero, genre, rappel) = (r.0.clone(), r.1.clone(), r.2);
                            if let Some(&(cs, cc)) = sals[si].par_numero.get(&numero) {
                                let c = sals[cs].contrats[cc].mesures(rappel);
                                c.remunere = true;
                                if genre == "001" {
                                    c.brut += montant;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "S21.G00.53" => {
                if champ == "001" {
                    activite = Some((v.to_string(), Decimal::ZERO));
                } else if activite.is_some() && champ == "002" {
                    activite.as_mut().unwrap().1 = decimal_lu(v).ok_or_else(|| socle::num::illisible(v))?;
                } else if activite.is_some() && champ == "003" {
                    let (genre, mesure) = activite.clone().unwrap();
                    let c = remu.as_ref().and_then(|r| sals[si].par_numero.get(&r.0).map(|&(cs, cc)| (cs, cc, r.2)));
                    let unite = match v {
                        "10" | "35" => Some("heures"),
                        "12" => Some("jours"),
                        "37" => Some("cachets"),
                        _ => None,
                    };
                    if let (Some((cs, cc, rappel)), Some(u)) = (c, unite) {
                        if genre == "01" {
                            let c = sals[cs].contrats[cc].mesures(rappel);
                            match u {
                                "jours" => c.jours += mesure,
                                "cachets" => c.cachets += mesure,
                                _ => c.heures += mesure,
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(sals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recapitulatif_lu_dans_le_fichier() {
        let f = "S20.G00.05.005,'01082026'\nS21.G00.30.001,'1850'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.5'\nS21.G00.51.013,'49.5'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\nS21.G00.62.001,'31082026'\n\
                 S21.G00.51.001,'01072026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'30.00'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'1'\nS21.G00.53.003,'37'\n";
        let s = lire_recap(f.as_bytes()).unwrap();
        // Un bloc 51 d'une periode anterieure au mois principal est un rappel (bloc-S21.G00.51.txt l. 38-40 ; §2.1.3.6) :
        // il ne s'ajoute pas a la paie du mois.
        assert_eq!(s.len(), 1);
        let c = &s[0].contrats[0];
        assert_eq!((c.mois.brut_texte().as_str(), c.mois.mesure_texte("cachets").as_str()), ("200.00", "2"));
        assert_eq!((c.debut.as_str(), c.fin_declaree.as_str(), c.mois.remunere), ("2026-08-01", "2026-08-31", true));
        assert_eq!((c.rappel.brut_texte().as_str(), c.rappel.mesure_texte("cachets").as_str()), ("30.00", "1"));
        assert_eq!(lire_recap(b"S21.G00.30.001,'1'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'A'\nS21.G00.51.001,'x'\nS21.G00.51.010,'A'\nS21.G00.51.013,'12,5'\n").err(), Some(socle::num::illisible("12,5")));
    }
}
