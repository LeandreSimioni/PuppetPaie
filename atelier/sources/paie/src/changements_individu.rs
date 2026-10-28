//! Le bloc S21.G00.31 « Changements individu » (corrections DSN, vague 3d).
//!
//! Le NIR, le nom de famille, les prenoms et la date de naissance sont des donnees IDENTIFIANTES : elles se reportent
//! a l'identique d'une DSN mensuelle a la suivante, et tout changement se signale par le bloc changement « des la
//! premiere DSN portant le changement » (cahier technique 2026.1, § 2.1.1, p. 18 ; § 2.1.2.3.1, p. 20 :
//! « obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee la nouvelle identite »). Le bloc 30 porte
//! la nouvelle valeur, le bloc 31 l'ANCIENNE (« Les rubriques doivent etre renseignees avec la valeur avant
//! changement », p. 177), datee en 31.001 ; plusieurs changements a la meme date font un seul bloc, des dates
//! differentes autant de blocs (p. 177) ; un bloc qui ne porte que sa date est interdit (CCH-12, p. 177). Textes :
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.31.txt`, `bloc-S21.G00.34.txt` (la fin de 31.011, p. 179),
//! `chapitre-2-principes-de-constitution.txt` (p. 18-21), `chapitre-4-controles.txt` (§ 4.4.12, p. 99 : 31.001 ne
//! depasse pas le dernier jour du mois principal declare).
//!
//! CE QUI A ETE DECLARE SE RELIT, IL NE SE RECALCULE PAS (regles 1 et 4) : l'identite de reference est le bloc 30 de
//! l'individu dans la derniere DSN mensuelle reelle acceptee qui le porte, d'un mois anterieur au mois declare (le
//! fichier que `casc::dsn_deposee` designe, comme pour une regularisation, vague 0b). La date du changement est celle
//! que la fiche datee porte : le `depuis` de la valeur qui suit la derniere qui porte l'ancienne valeur declaree
//! (`contexte::dater`). Une fiche qui ne date pas le changement refuse : la date ne se devine pas.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::Value;

use droit::contexte::{fiche_au, valeurs_datees, Contexte};
use socle::date::{en_texte, Jour};
use crate::dsn::Fichier;
use socle::fichiers::{lire_json, vrai};
use socle::num::Resultat;
use crate::usages::{lignes, rubrique_valeur, strip};

/// Les champs de la fiche que le bloc 30 ecrit et que le bloc 31 peut porter : (champ, rubrique 30, rubrique 31,
/// libelle). L'ordre est celui du bloc 31 (p. 177).
pub const IDENTIFIANTS: [(&str, &str, &str, &str); 4] = [
    ("nir", "S21.G00.30.001", "S21.G00.31.008", "NIR"),
    ("nom", "S21.G00.30.002", "S21.G00.31.009", "nom de famille"),
    ("prenom", "S21.G00.30.004", "S21.G00.31.010", "prenoms"),
    ("date_naissance", "S21.G00.30.006", "S21.G00.31.011", "date de naissance"),
];

/// L'identite d'un individu telle qu'une DSN l'a declaree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaree {
    /// Le mois principal declare (AAAA-MM) et le nom du fichier qui l'a portee.
    pub mois: String,
    pub fichier: String,
    pub nir: String,
    pub nom: String,
    pub prenoms: String,
    /// JJMMAAAA, comme 30.006.
    pub naissance: String,
    /// Le numero technique temporaire (30.020), s'il y en avait un.
    pub ntt: String,
    pub matricule: String,
}

impl Declaree {
    fn valeur(&self, champ: &str) -> &str {
        match champ {
            "nir" => &self.nir,
            "nom" => &self.nom,
            "prenom" => &self.prenoms,
            _ => &self.naissance,
        }
    }

    /// L'identite seule (sans le mois ni le fichier) : deux etablissements d'un meme mois doivent dire la meme.
    fn identite(&self) -> (&str, &str, &str, &str, &str) {
        (&self.nir, &self.nom, &self.prenoms, &self.naissance, &self.ntt)
    }
}

/// Les individus (bloc 30) d'un fichier DSN : un individu commence a 30.001, ou a 30.002 s'il n'a pas de NIR (comme
/// `ntt::individus`).
pub fn individus(texte: &str, mois: &str, fichier: &str) -> Vec<Declaree> {
    let vide = || Declaree {
        mois: mois.to_string(),
        fichier: fichier.to_string(),
        nir: String::new(),
        nom: String::new(),
        prenoms: String::new(),
        naissance: String::new(),
        ntt: String::new(),
        matricule: String::new(),
    };
    let mut out: Vec<Declaree> = Vec::new();
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        let v = strip(v).to_string();
        match r {
            "S21.G00.30.001" => {
                let mut d = vide();
                d.nir = v;
                out.push(d);
            }
            "S21.G00.30.002" => {
                if out.last().map_or(true, |d| !d.nom.is_empty()) {
                    out.push(vide());
                }
                out.last_mut().unwrap().nom = v;
            }
            "S21.G00.30.004" | "S21.G00.30.006" | "S21.G00.30.019" | "S21.G00.30.020" => {
                if let Some(d) = out.last_mut() {
                    match r {
                        "S21.G00.30.004" => d.prenoms = v,
                        "S21.G00.30.006" => d.naissance = v,
                        "S21.G00.30.019" => d.matricule = v,
                        _ => d.ntt = v,
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn texte(v: Option<&Value>) -> String {
    match v {
        Some(x) if vrai(x) => strip(&en_texte(x)).to_string(),
        _ => String::new(),
    }
}

/// La valeur d'un identifiant de la fiche telle que le bloc 30 l'ecrit (la date de naissance en JJMMAAAA). `None` :
/// une date illisible, qui ne ressemble donc a aucune valeur declaree.
fn ecrite(champ: &str, v: Option<&Value>) -> Option<String> {
    if champ == "date_naissance" {
        let v = v.filter(|x| vrai(x))?;
        return socle::date::d(v).ok().map(|j| j.jjmmaaaa());
    }
    Some(texte(v))
}

/// La fiche telle que le bloc 30 doit la lire pour le mois declare : ses identifiants tels qu'ils valaient le dernier
/// jour du mois (`fin`), lus dans la fiche datee (`brute`), le reste tel quel. Un changement d'identite date APRES le
/// mois declare n'entre pas dans sa DSN, sans quoi 31.001 depasserait le dernier jour du mois principal declare
/// (§ 4.4.12, p. 99) -- il ira dans la DSN du mois ou il prend effet.
pub fn identite_du_mois(salarie: &Value, brute: &Value, fin: &Jour) -> Resultat<Value> {
    let au = fiche_au(brute, fin, "config/salaries.json")?;
    let mut sortie = salarie.clone();
    if let Some(o) = sortie.as_object_mut() {
        for (champ, ..) in IDENTIFIANTS {
            match au.get(champ) {
                Some(v) => {
                    o.insert(champ.to_string(), v.clone());
                }
                None => {
                    o.shift_remove(champ);
                }
            }
        }
    }
    Ok(sortie)
}

/// Les DSN mensuelles reelles acceptees des mois anterieurs au mois declare, lues a la demande, du plus recent au plus
/// ancien.
pub struct Declarations<'a> {
    ctx: &'a Contexte,
    /// (mois, etablissements dont le journal a une DSN reelle acceptee ; vide : celle que l'import CoolPaie a retenue).
    mois: Vec<(String, BTreeSet<String>)>,
    lus: BTreeMap<usize, Vec<Declaree>>,
}

impl<'a> Declarations<'a> {
    /// Les mois declares avant `mois` : le journal des depots (`journal/depots.json`, DSN reelles acceptees) et
    /// l'historique importe de CoolPaie (`historique/paies.json`).
    pub fn avant(ctx: &'a Contexte, mois: &str) -> Resultat<Declarations<'a>> {
        let mut par_mois: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let principal = texte(ctx.entreprise.get("nic"));
        let journal = ctx.racine.join("journal").join("depots.json");
        if journal.exists() {
            for l in lire_json(&journal)?.as_array().into_iter().flatten() {
                let m = texte(l.get("mois"));
                if l.get("quoi").and_then(Value::as_str) != Some("dsn")
                    || l.get("mode").and_then(Value::as_str) != Some("reel")
                    || !l.get("accepte").is_some_and(vrai)
                    || m.as_str() >= mois
                    || m.len() != 7
                {
                    continue;
                }
                let e = texte(l.get("detail").and_then(|d| d.get("etablissement")));
                par_mois.entry(m).or_default().insert(if e.is_empty() { principal.clone() } else { e });
            }
        }
        let historique = ctx.racine.join("historique").join("paies.json");
        if historique.exists() {
            for p in lire_json(&historique)?.get("paies").and_then(Value::as_array).into_iter().flatten() {
                let m = texte(p.get("mois"));
                if m.len() == 7 && m.as_str() < mois && !texte(p.get("fichier")).is_empty() {
                    par_mois.entry(m).or_default();
                }
            }
        }
        Ok(Declarations { ctx, mois: par_mois.into_iter().rev().collect(), lus: BTreeMap::new() })
    }

    /// Les individus des DSN qui font foi pour le `i`-ieme mois.
    fn lire(&mut self, i: usize) -> Resultat<&Vec<Declaree>> {
        if !self.lus.contains_key(&i) {
            let (mois, etabs) = &self.mois[i];
            let mut chemins: Vec<PathBuf> = Vec::new();
            let nics: Vec<Option<&str>> = if etabs.is_empty() { vec![None] } else { etabs.iter().map(|e| Some(e.as_str())).collect() };
            for nic in nics {
                if let Some(p) = crate::casc::dsn_deposee(self.ctx, mois, nic).map_err(|m| {
                    format!(
                        "{} (L'identite des salaries se relit aussi dans les DSN deposees : le bloc S21.G00.31 compare a ce qui a ete declare.)",
                        m
                    )
                })? {
                    if !chemins.contains(&p) {
                        chemins.push(p);
                    }
                }
            }
            let mut tous = Vec::new();
            for p in chemins {
                let octets = std::fs::read(&p).map_err(|e| format!("{} : {}", p.display(), e))?;
                // Latin-1, comme la DSN s'ecrit (`dsn::ecrire`).
                let t: String = octets.iter().map(|b| *b as char).collect();
                let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                tous.extend(individus(&t, mois, &nom));
            }
            self.lus.insert(i, tous);
        }
        Ok(&self.lus[&i])
    }

    /// L'identite que la derniere DSN qui le porte a declaree pour ce salarie, ou `None` s'il n'a jamais ete declare.
    /// Il se reconnait a un NIR que sa fiche (`brute`, datee) a porte, sinon a son matricule
    /// (30.019 ; il finit le NTT et ne change pas sans NIR, `routes_ecritures::ecrire_fiche`).
    pub fn derniere(&mut self, salarie: &Value, brute: &Value) -> Resultat<Option<Declaree>> {
        let mut nirs: Vec<String> = vec![texte(salarie.get("nir"))];
        nirs.extend(valeurs_datees(brute, "nir")?.into_iter().map(|(_, v)| texte(Some(v))));
        nirs.retain(|n| !n.is_empty());
        let matricule = texte(salarie.get("code_salarie"));
        for i in 0..self.mois.len() {
            let tous = self.lire(i)?;
            let mut siens: Vec<&Declaree> = tous.iter().filter(|d| !d.nir.is_empty() && nirs.contains(&d.nir)).collect();
            if siens.is_empty() && !matricule.is_empty() {
                siens = tous.iter().filter(|d| d.matricule == matricule).collect();
            }
            let Some(premier) = siens.first() else { continue };
            if let Some(autre) = siens.iter().find(|d| d.identite() != premier.identite()) {
                return Err(format!(
                    "{} : la DSN de {} le declare deux fois, sous deux identites ({} et {}) : laquelle a ete declaree ne se \
devine pas. Rien n'est ecrit.",
                    qui(salarie),
                    premier.mois,
                    decrire(premier),
                    decrire(autre)
                ));
            }
            return Ok(Some((*premier).clone()));
        }
        Ok(None)
    }
}

fn qui(salarie: &Value) -> String {
    let prenom = texte(salarie.get("prenom_affichage").or(salarie.get("prenom")));
    let nom = texte(salarie.get("nom"));
    let matricule = texte(salarie.get("code_salarie"));
    format!("{} {} (matricule {})", prenom, nom, if matricule.is_empty() { "?" } else { &matricule })
}

fn decrire(d: &Declaree) -> String {
    format!(
        "{} {}, ne(e) le {}, {}",
        d.prenoms,
        d.nom,
        d.naissance,
        if d.nir.is_empty() { format!("NTT {}", d.ntt) } else { format!("NIR {}", d.nir) }
    )
}

/// Le jour d'effet que la fiche datee `brute` donne au changement de `champ` depuis la valeur `declaree` : le `depuis`
/// de la valeur qui suit la derniere qui la porte, au plus tard la fin du mois declare (`fin`).
fn date_d_effet(brute: &Value, champ: &str, declaree: &str, fin: &Jour) -> Resultat<Option<Jour>> {
    let valeurs = valeurs_datees(brute, champ)?;
    let mut meilleur: Option<Jour> = None;
    for paire in valeurs.windows(2) {
        let ((_, v), (suivant, _)) = (&paire[0], &paire[1]);
        if ecrite(champ, Some(v)).as_deref() == Some(declaree) && suivant <= fin {
            meilleur = Some(*suivant);
        }
    }
    Ok(meilleur)
}

/// Ecrit, apres le bloc 30 de `salarie` (sa fiche telle que le bloc 30 l'a lue, `identite_du_mois`), les blocs 31 de
/// ce qui differe de l'identite `declaree` ; rien s'il n'a jamais ete declare. `mois` (AAAA-MM) et `fin` : le mois
/// declare et son dernier jour.
///
/// Le NIR qui ARRIVE (declare sans NIR, sous un NTT) n'a pas d'« ancien NIR » : la norme fait le lien par le NTT porte
/// a cote du NIR (30.020, p. 175 ; `ntt.rs`), pas par le bloc 31.
pub fn ecrire(f: &mut Fichier, salarie: &Value, brute: &Value, declaree: Option<&Declaree>, mois: &str, fin: &Jour) -> Resultat<()> {
    let Some(d) = declaree else { return Ok(()) };
    let mut par_date: BTreeMap<Jour, Vec<(&str, String)>> = BTreeMap::new();
    let mut sans_date: Vec<String> = Vec::new();
    for (champ, _, r31, libelle) in IDENTIFIANTS {
        let avant = d.valeur(champ);
        let maintenant = ecrite(champ, salarie.get(champ)).unwrap_or_default();
        if avant == maintenant {
            continue;
        }
        if champ == "nir" {
            if avant.is_empty() {
                continue;
            }
            if maintenant.is_empty() {
                return Err(format!(
                    "{} a ete declare(e) sous le NIR {} (DSN de {}, {}), et sa fiche n'a plus de NIR : un NIR declare ne se \
retire pas, il se reporte a l'identique d'une DSN a la suivante (cahier technique, § 2.1.1, p. 18). Remettre le NIR \
dans la fiche, ou le remplacer par le NIR corrige, date. Rien n'est ecrit.",
                    qui(salarie),
                    avant,
                    d.mois,
                    d.fichier
                ));
            }
        }
        match date_d_effet(brute, champ, avant, fin)? {
            Some(j) => par_date.entry(j).or_default().push((r31, avant.to_string())),
            None => sans_date.push(format!(
                "  - {} : declare(e) \u{ab} {} \u{bb} dans la DSN de {} ({}), \u{ab} {} \u{bb} dans la fiche",
                libelle, avant, d.mois, d.fichier, maintenant
            )),
        }
    }
    if !sans_date.is_empty() {
        return Err(format!(
            "La DSN de {} ne se fait pas : l'identite de {} a change depuis sa derniere declaration, et la fiche ne dit pas \
depuis quand.\n{}\nLa DSN declare l'ancienne valeur et la date du changement (bloc \u{ab} Changements individu \u{bb} \
S21.G00.31, cahier technique p. 177 ; \u{ab} obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee \
la nouvelle identite \u{bb}, p. 20) : dater ce changement dans la fiche du salarie (y remettre l'ancienne valeur, \
enregistrer, puis saisir la nouvelle : l'ecran demande alors la date d'effet). Rien n'est ecrit.",
            mois,
            qui(salarie),
            sans_date.join("\n")
        ));
    }
    for (jour, rubriques) in par_date {
        f.add("S21.G00.31.001", jour.jjmmaaaa());
        for (r, v) in rubriques {
            f.add(r, v);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn declaree() -> Declaree {
        individus(
            "S21.G00.30.001,'2901075999009'\r\nS21.G00.30.002,'CLAIRON'\r\nS21.G00.30.004,'LUCILE'\r\nS21.G00.30.006,'02101990'\r\nS21.G00.30.019,'6'\r\n",
            "2026-07",
            "DSN_Juillet2026.dsn",
        )
        .remove(0)
    }

    fn fin() -> Jour {
        Jour::lire("2026-09-30").unwrap()
    }

    /// Une fiche datee : chaque valeur depuis le 01/01/2026, puis `apres` (champ, depuis, valeur).
    fn fiche(valeurs: Value, apres: &[(&str, &str, &str)]) -> Value {
        let mut f = droit::contexte::fiche_datee(valeurs.as_object().unwrap(), &Jour::lire("2026-01-01").unwrap());
        for (champ, depuis, v) in apres {
            droit::contexte::dater(&mut f, champ, json!(v), &Jour::lire(depuis).unwrap(), None);
        }
        Value::Object(f)
    }

    #[test]
    fn rien_ne_change_rien_ne_s_ecrit() {
        let s = json!({"nir": "2901075999009", "nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let b = fiche(s.clone(), &[]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &s, &b, Some(&declaree()), "2026-09", &fin()).unwrap();
        assert!(f.lignes.is_empty());
        ecrire(&mut f, &s, &b, None, "2026-09", &fin()).unwrap();
        assert!(f.lignes.is_empty());
    }

    #[test]
    fn un_changement_date_porte_l_ancienne_valeur_et_un_non_date_refuse() {
        let s = json!({"nir": "2901075999009", "nom": "DUVAL", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let b = fiche(json!({"nom": "CLAIRON"}), &[("nom", "2026-09-15", "DUVAL")]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &s, &b, Some(&declaree()), "2026-09", &fin()).unwrap();
        assert_eq!(f.lignes, vec!["S21.G00.31.001,'15092026'", "S21.G00.31.009,'CLAIRON'"]);
        // Une fiche qui n'a jamais porte la valeur declaree ne date rien.
        let b = fiche(json!({"nom": "AUTRE"}), &[("nom", "2026-09-15", "DUVAL")]);
        let e = ecrire(&mut Fichier::nouveau(), &s, &b, Some(&declaree()), "2026-09", &fin()).unwrap_err();
        assert!(e.contains("nom de famille") && e.contains("CLAIRON") && e.contains("DUVAL"), "{}", e);
    }

    #[test]
    fn un_nir_declare_ne_disparait_pas() {
        let s = json!({"nir": "", "nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"});
        let e = ecrire(&mut Fichier::nouveau(), &s, &fiche(s.clone(), &[]), Some(&declaree()), "2026-09", &fin()).unwrap_err();
        assert!(e.contains("n'a plus de NIR"), "{}", e);
    }

    #[test]
    fn l_identite_du_mois_ignore_un_changement_posterieur() {
        let s = json!({"nom": "DUVAL", "adresse": "NOUVELLE"});
        let b = fiche(json!({"nom": "CLAIRON", "adresse": "ANCIENNE"}), &[("nom", "2026-10-02", "DUVAL"), ("adresse", "2026-10-02", "NOUVELLE")]);
        let m = identite_du_mois(&s, &b, &fin()).unwrap();
        assert_eq!(m["nom"], json!("CLAIRON"));
        // Ce qui n'identifie pas se lit tel quel (la derniere valeur connue, § 2.1.2.4.2 par analogie).
        assert_eq!(m["adresse"], json!("NOUVELLE"));
    }
}
