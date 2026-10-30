//! Le bloc S21.G00.31 « Changements individu » (corrections DSN).
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
//! CE QUI A ETE DECLARE SE RELIT, IL NE SE RECALCULE PAS (regles 1 et 4) : l'identite de reference est celle de la fiche
//! que lit la paie declaree la plus recente du salarie, d'un mois anterieur au mois declare (`dsn::paies_declarees_du_mois`,
//! la version de l'engagement sur laquelle sa DSN est partie). La date du changement est celle que la fiche datee porte :
//! le `depuis` de la valeur qui suit la derniere qui porte l'ancienne valeur declaree (`contexte::dater`). Une fiche qui
//! ne date pas le changement refuse : la date ne se devine pas.

use std::collections::BTreeMap;

use serde_json::Value;

use droit::contexte::{fiche_au, valeurs_datees, Contexte};
use socle::date::{en_texte, Jour};
use crate::dsn::Fichier;
use socle::fichiers::vrai;
use socle::num::Resultat;
use crate::usages::strip;

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

/// Les identites declarees des mois anterieurs au mois declare, lues a la demande, du plus recent au plus ancien.
pub struct Declarations<'a> {
    ctx: &'a Contexte,
    departs: &'a engagement::departs::Departs,
    mois: Vec<String>,
    lus: BTreeMap<usize, Vec<Declaree>>,
}

impl<'a> Declarations<'a> {
    /// Les mois saisis avant `mois`.
    pub fn avant(ctx: &'a Contexte, mois: &str, departs: &'a engagement::departs::Departs) -> Resultat<Declarations<'a>> {
        let mut avant: Vec<String> = engagement::stock::mois_saisis(&ctx.racine)?.into_iter().filter(|m| m.as_str() < mois).collect();
        avant.sort();
        avant.reverse();
        Ok(Declarations { ctx, departs, mois: avant, lus: BTreeMap::new() })
    }

    /// Les individus que les paies declarees du `i`-ieme mois portent (`dsn::paies_declarees_du_mois`), avec la fiche
    /// que leur calcul a lue : l'identite que leur DSN a ecrite.
    fn lire(&mut self, i: usize) -> Resultat<&Vec<Declaree>> {
        if !self.lus.contains_key(&i) {
            let mois = &self.mois[i];
            let mut tous = Vec::new();
            for p in crate::dsn::paies_declarees_du_mois(self.ctx, mois, None, self.departs)? {
                let s = &p.salarie;
                let nir = crate::ntt::nir_de(s);
                tous.push(Declaree {
                    mois: mois.clone(),
                    fichier: "la paie declaree".to_string(),
                    ntt: if nir.is_empty() { crate::ntt::numero(&self.ctx.entreprise, s).unwrap_or_default() } else { String::new() },
                    nir,
                    nom: texte(s.get("nom")),
                    prenoms: texte(s.get("prenom")),
                    naissance: ecrite("date_naissance", s.get("date_naissance")).unwrap_or_default(),
                    matricule: texte(s.get("code_salarie")),
                });
            }
            self.lus.insert(i, tous);
        }
        Ok(&self.lus[&i])
    }

    /// L'identite que la derniere DSN qui le porte a declaree pour ce salarie, ou `None` s'il n'a jamais ete declare.
    /// Il se reconnait a un NIR que sa fiche (`brute`, datee) a porte, sinon a son matricule
    /// (30.019 ; il finit le NTT et ne change pas sans NIR, `moteur::routes::fiches::ecrire_fiche`).
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
    let matricule = texte(salarie.get("code_salarie"));
    format!("{} (matricule {})", socle::texte::salarie_nomme_ou(Some(salarie), "?"), if matricule.is_empty() { "?" } else { &matricule })
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
        if let Some(suivant) = suivant.filter(|s| s <= fin) {
            if ecrite(champ, Some(v)).as_deref() == Some(declaree) {
                meilleur = Some(suivant);
            }
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
        Declaree {
            mois: "2026-07".into(),
            fichier: "la paie declaree".into(),
            nir: "2901075999009".into(),
            nom: "CLAIRON".into(),
            prenoms: "LUCILE".into(),
            naissance: "02101990".into(),
            ntt: String::new(),
            matricule: "6".into(),
        }
    }

    fn fin() -> Jour {
        Jour::lire("2026-09-30").unwrap()
    }

    /// Une fiche datee : chaque valeur de tout temps, puis `apres` (champ, depuis, valeur).
    fn fiche(valeurs: Value, apres: &[(&str, &str, &str)]) -> Value {
        let mut f = droit::contexte::fiche_datee(valeurs.as_object().unwrap());
        for (champ, depuis, v) in apres {
            droit::contexte::dater(&mut f, champ, json!(v), Some(&Jour::lire(depuis).unwrap()), None);
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
