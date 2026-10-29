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

#[derive(Default)]
pub struct Contrat {
    pub numero: String,
    pub debut: String,
    pub fin: String,
    pub emploi: String,
    pub nature: String,
    pub objet: String,
    pub brut: Decimal,
    pub jours: Decimal,
    pub cachets: Decimal,
    pub heures: Decimal,
    pub fin_declaree: String,
    pub motif_fin: String,
    pub remunere: bool,
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

/// Les montants d'un contrat en texte, sous la forme que le recapitulatif complete.
impl Contrat {
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

/// Les salaries du fichier, leurs contrats, leurs versements.
pub fn lire_recap(octets: &[u8]) -> Result<Vec<Individu>, String> {
    let texte = texte_latin1(octets);
    let mut sals: Vec<Individu> = Vec::new();
    let mut contrat: Option<(usize, usize)> = None;
    let mut versement: Option<(usize, usize)> = None;
    let mut fin: Option<(usize, usize)> = None;
    // (numero de contrat, type) de la remuneration en cours
    let mut remu: Option<(String, String)> = None;
    // (type, mesure) de l'activite en cours
    let mut activite: Option<(String, Decimal)> = None;
    for ligne in lignes_du_texte(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(blanc).trim_matches('\'');
        let kc: Vec<char> = k.chars().collect();
        let bloc: String = kc.iter().take(10).collect();
        let champ: String = kc.iter().skip(11).collect();
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
                    remu = Some((String::new(), String::new()));
                } else if let Some(r) = remu.as_mut() {
                    match champ.as_str() {
                        "010" => r.0 = v.to_string(),
                        "011" => r.1 = v.to_string(),
                        "013" => {
                            let montant = decimal_lu(v).ok_or_else(|| socle::num::illisible(v))?;
                            let (numero, genre) = (r.0.clone(), r.1.clone());
                            if let Some(&(cs, cc)) = sals[si].par_numero.get(&numero) {
                                let c = &mut sals[cs].contrats[cc];
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
                    let c = remu.as_ref().and_then(|r| sals[si].par_numero.get(&r.0).copied());
                    let unite = match v {
                        "10" | "35" => Some("heures"),
                        "12" => Some("jours"),
                        "37" => Some("cachets"),
                        _ => None,
                    };
                    if let (Some((cs, cc)), Some(u)) = (c, unite) {
                        if genre == "01" {
                            let c = &mut sals[cs].contrats[cc];
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
        let f = "S21.G00.30.001,'1850'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.5'\nS21.G00.51.013,'49.5'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\nS21.G00.62.001,'31082026'\n";
        let s = lire_recap(f.as_bytes()).unwrap();
        assert_eq!(s.len(), 1);
        let c = &s[0].contrats[0];
        assert_eq!((c.brut_texte().as_str(), c.mesure_texte("cachets").as_str()), ("200.00", "2"));
        assert_eq!((c.debut.as_str(), c.fin_declaree.as_str(), c.remunere), ("2026-08-01", "2026-08-31", true));
        assert_eq!(lire_recap(b"S21.G00.30.001,'1'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'A'\nS21.G00.51.001,'x'\nS21.G00.51.010,'A'\nS21.G00.51.013,'12,5'\n").err(), Some(socle::num::illisible("12,5")));
    }
}
