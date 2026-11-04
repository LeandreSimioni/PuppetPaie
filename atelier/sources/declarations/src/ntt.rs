//! Le numero technique temporaire : declarer quelqu'un qui n'a pas de NIR.
//!
//! Il se fabrique (sexe + SIREN + matricule), il ne se saisit pas. Les
//! regles de forme CSL-11, CSL-12, CCH-13 et la longueur [11,40] sont ici.

use socle::date::en_texte;
use crate::dsn::Fichier;
use socle::fichiers::{repr, vrai};
use crate::formats;
use socle::num::Resultat;
use crate::usages::{lignes, rubrique_valeur, strip};
use droit::fiche::{ChampsSalarie, FicheSalarie};
use serde_json::Value;

pub const RUBRIQUE:&str = "S21.G00.30.020";
pub const RUBRIQUE_NIR: &str = "S21.G00.30.001";
// Le marqueur sur lequel on decoupe un fichier en individus : un individu
// sous NTT n'a pas de NIR.
pub const RUBRIQUE_NOM: &str = "S21.G00.30.002";

/// La valeur de `cle` de la fiche en texte, vide si elle manque ou est vide.
fn chaine(fiche: &Value, cle: &str) -> String {
    match fiche.get(cle) {
        Some(v) if vrai(v) => en_texte(v),
        _ => String::new(),
    }
}

/// Le code sexe de la norme : "1", "2", ou "" si la fiche ne le dit pas.
pub fn sexe(salarie: &Value) -> String {
    sexe_de(&chaine(salarie, "nir"), &chaine(salarie, "sexe"))
}

/// Le code sexe d'un NIR (son premier chiffre), sinon de la case `sexe` ; "" si ni l'un ni l'autre ne le dit.
pub fn sexe_de(nir: &str, sexe: &str) -> String {
    let nir = strip(nir);
    if nir.starts_with('1') || nir.starts_with('2') {
        return nir[..1].to_string();
    }
    let v = strip(sexe);
    if v == "1" || v == "2" {
        v.to_string()
    } else {
        String::new()
    }
}

/// Le NTT de ce salarie dans l'entreprise de SIREN `siren`.
pub fn numero(siren: &str, salarie: &FicheSalarie) -> Resultat<String> {
    let s = sexe_de(salarie.nir.texte(), salarie.sexe.code());
    if s.is_empty() {
        let qui = if salarie.nom.texte().is_empty() { salarie.id.to_string() } else { salarie.nom.texte().to_string() };
        return Err(format!(
            "Le numero technique temporaire ({}) de {} ne peut pas etre fabrique : sa fiche ne dit pas le sexe, et le NTT commence par lui (1 ou 2). D'ordinaire il se lit sur le NIR ; sans NIR, il faut le renseigner dans la fiche du salarie.",
            RUBRIQUE, qui
        ));
    }
    let siren = strip(siren).to_string();
    if siren.chars().count() != 9 || !siren.chars().all(|c| c.is_ascii_digit()) {
        return Err(socle::echec::a_la_cle(
            &format!(
                "Le numero technique temporaire ({}) porte le SIREN de l'entreprise en 2e a 10e position (CCH-13), et celui de la fiche entreprise n'a pas neuf chiffres : le corriger dans Configuration › Entreprise.",
                RUBRIQUE
            ),
            "siren",
            repr(&siren),
        ));
    }
    let matricule = strip(salarie.code_salarie.texte());
    if matricule.is_empty() {
        return Err(format!(
            "Le numero technique temporaire ({}) finit par l'identifiant perenne du salarie dans l'entreprise -- son matricule --, et la fiche n'en porte pas.",
            RUBRIQUE
        ));
    }
    let v = format!("{}{}{}", s, siren, matricule);
    if let Some(pourquoi) = faute(&v, &siren)? {
        return Err(format!(
            "Le numero technique temporaire fabrique pour cette fiche, {}, n'est pas declarable : {}.",
            repr(&v),
            pourquoi
        ));
    }
    Ok(v)
}

/// Ce que la norme reproche a ce NTT -- None s'il passe.
pub fn faute(valeur: &str, siren: &str) -> Resultat<Option<String>> {
    let c: Vec<char> = valeur.chars().collect();
    if c.is_empty() {
        return Ok(Some("il est vide".into()));
    }
    if c[0] != '1' && c[0] != '2' {
        return Ok(Some(format!(
            "il doit commencer par 1 ou 2, le code sexe de la personne (CSL-11) ; celui-ci commence par \u{ab} {} \u{bb}",
            c[0]
        )));
    }
    if let Some(i) = c.iter().position(|&x| x == ' ') {
        return Ok(Some(format!(
            "le caractere espace y est interdit (CSL-12) ; il porte un espace en position {}",
            i + 1
        )));
    }
    if !siren.is_empty() {
        let milieu: String = c.iter().skip(1).take(9).collect();
        if milieu != siren {
            return Ok(Some(format!(
                "ses caracteres 2 a 10 doivent etre le SIREN declare, {} (CCH-13) ; il porte {}",
                siren,
                repr(&milieu)
            )));
        }
    }
    let m = formats::mauvaise(RUBRIQUE, valeur)?;
    Ok(if m.is_empty() { None } else { Some(m) })
}

pub fn nir_de(salarie: &ChampsSalarie) -> String {
    strip(salarie.nir.texte()).to_string()
}

/// La fiche datee du salarie `salarie`, ou rien.
pub fn brute(ctx: &droit::contexte::Contexte, salarie: &droit::contexte::IdSalarie) -> Value {
    ctx.salarie_brut(salarie.as_str()).cloned().unwrap_or(Value::Null)
}

/// S21.G00.30.001, quand il y en a un -- et rien du tout sinon.
pub fn poser_nir(f: &mut Fichier, salarie: &ChampsSalarie) -> Resultat<()> {
    let nir = nir_de(salarie);
    if !nir.is_empty() {
        f.add(RUBRIQUE_NIR, nir);
    }
    Ok(())
}

/// S21.G00.30.020, apres le matricule, dans toute DSN (mensuelle, signalement, demande de taux). Sans NIR, le NTT du
/// salarie (« En l'absence de NIR ou de NIA, il est obligatoire de remplir la rubrique ») ; avec un NIR, le NTT sous lequel
/// la derniere DSN partie l'a declare quand elle ne portait pas de NIR, quel que soit le jour ou le NIR est arrive dans la
/// fiche (« Le NTT doit etre renseigne sur la premiere DSN ou le NIR est attribue. Ceci permettra de faire le lien entre
/// les deux identites declarees par l'employeur », et « le NTT le plus ancien en cours de validite » :
/// `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 348-354) ; sinon rien. `declaree` : l'identite que la derniere DSN a
/// declaree pour lui (`changements_individu::Declarations::derniere`, ou `derniere_declaree`).
pub fn poser_ntt(f: &mut Fichier, ent: &droit::fiche::ChampsEntreprise, salarie: &FicheSalarie, declaree:Option<&crate::changements_individu::Declaree>) -> Resultat<()> {
    if nir_de(salarie).is_empty() {
        f.add(RUBRIQUE, numero(ent.siren.texte(), salarie)?);
        return Ok(());
    }
    if let Some(n) = declaree.filter(|d| d.nir.is_empty()).map(|d| strip(&d.ntt)).filter(|n| !n.is_empty()) {
        f.add(RUBRIQUE, n);
    }
    Ok(())
}

/// CCH-12 et CCH-14, verifies sur le fichier seul.
pub fn verifier_fichier(texte: &str) -> Resultat<()> {
    refuser_les_doublons(&individus(texte))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Individu {
    pub nom: String,
    pub nir: String,
    pub ntt: String,
}

/// Les blocs Individu d'un fichier DSN.
pub fn individus(texte: &str) -> Vec<Individu> {
    let mut trouves: Vec<Individu> = Vec::new();
    let vide = |nir: &str| Individu { nom: String::new(), nir: nir.to_string(), ntt: String::new() };
    for ligne in lignes(texte) {
        let (rubrique, valeur) = rubrique_valeur(ligne);
        if rubrique == RUBRIQUE_NIR {
            trouves.push(vide(valeur));
        } else if rubrique == RUBRIQUE_NOM {
            if trouves.last().map_or(true, |c| !c.nom.is_empty()) {
                trouves.push(vide(""));
            }
            trouves.last_mut().unwrap().nom = valeur.to_string();
        } else if rubrique == RUBRIQUE {
            if let Some(c) = trouves.last_mut() {
                c.ntt = valeur.to_string();
            }
        }
    }
    trouves
}

fn refuser_les_doublons(individus: &[Individu]) -> Resultat<()> {
    let mut vus_nir: Vec<(&str, &str)> = Vec::new();
    let mut vus_ntt: Vec<(&str, &str)> = Vec::new();
    for i in individus {
        if i.nir.is_empty() && i.ntt.is_empty() {
            return Err(format!(
                "{} est declare sans numero d'inscription au repertoire et sans numero technique temporaire. La norme exige l'un ou l'autre (CCH-12, S21.G00.30.020) : sans cela l'individu n'est identifie par rien.",
                i.nom
            ));
        }
        for (cle, vus, quoi) in [(&i.nir, &mut vus_nir, "NIR"), (&i.ntt, &mut vus_ntt, "NTT")] {
            if cle.is_empty() {
                continue;
            }
            if let Some((_, autre)) = vus.iter().find(|(c, _)| *c == cle.as_str()) {
                return Err(format!(
                    "Le meme {} ({}) identifie deux individus de la meme declaration, {} et {}. La norme l'interdit (CCH-14) : un identifiant qui designe deux personnes rend la declaration intracable.",
                    quoi, cle, autre, i.nom
                ));
            }
            vus.push((cle.as_str(), i.nom.as_str()));
        }
    }
    Ok(())
}
