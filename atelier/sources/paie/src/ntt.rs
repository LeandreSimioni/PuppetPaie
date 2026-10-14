//! Le numero technique temporaire : declarer quelqu'un qui n'a pas de NIR
//! (`moteur/ntt.py`).
//!
//! Il se fabrique (sexe + SIREN + matricule), il ne se saisit pas. Les
//! regles de forme CSL-11, CSL-12, CCH-13 et la longueur [11,40] sont ici.

use crate::date::{en_texte, Jour};
use crate::dsn::Fichier;
use crate::fichiers::{repr, vrai};
use crate::formats;
use crate::num::Resultat;
use crate::usages::{lignes, rubrique_valeur, strip};
use serde_json::Value;

pub const RUBRIQUE: &str = "S21.G00.30.020";
pub const RUBRIQUE_NIR: &str = "S21.G00.30.001";
// Le marqueur sur lequel on decoupe un fichier en individus : un individu
// sous NTT n'a pas de NIR.
pub const RUBRIQUE_NOM: &str = "S21.G00.30.002";

/// `str(fiche.get(cle) or "")`.
fn chaine(fiche: &Value, cle: &str) -> String {
    match fiche.get(cle) {
        Some(v) if vrai(v) => en_texte(v),
        _ => String::new(),
    }
}

/// Le code sexe de la norme : "1", "2", ou "" si la fiche ne le dit pas.
pub fn sexe(salarie: &Value) -> String {
    let nir = chaine(salarie, "nir");
    let nir = strip(&nir);
    if nir.starts_with('1') || nir.starts_with('2') {
        return nir[..1].to_string();
    }
    let v = chaine(salarie, "sexe");
    let v = strip(&v);
    if v == "1" || v == "2" {
        v.to_string()
    } else {
        String::new()
    }
}

/// Le NTT de ce salarie dans cette entreprise.
pub fn numero(entreprise: &Value, salarie: &Value) -> Resultat<String> {
    let s = sexe(salarie);
    if s.is_empty() {
        let qui = ["nom", "_cle"]
            .iter()
            .map(|c| chaine(salarie, c))
            .find(|x| !x.is_empty())
            .unwrap_or_else(|| "ce salarie".to_string());
        return Err(format!(
            "Le numero technique temporaire ({}) de {} ne peut pas etre fabrique : sa fiche ne dit pas le sexe, et le NTT commence par lui (1 ou 2). D'ordinaire il se lit sur le NIR ; sans NIR, il faut le renseigner dans la fiche du salarie.",
            RUBRIQUE, qui
        ));
    }
    let siren = chaine(entreprise, "siren");
    let siren = strip(&siren).to_string();
    if siren.chars().count() != 9 || !siren.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Le numero technique temporaire ({}) porte le SIREN de l'entreprise en 2e a 10e position (CCH-13), et config/entreprise.json en donne {}.",
            RUBRIQUE,
            repr(&siren)
        ));
    }
    let matricule = chaine(salarie, "code_salarie");
    let matricule = strip(&matricule);
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

pub fn nir_de(salarie: &Value) -> String {
    strip(&chaine(salarie, "nir")).to_string()
}

/// Le jour ou le NIR a pris effet, s'il a succede a une absence de NIR.
pub fn depuis_quand_le_nir(salarie: &Value) -> Resultat<Option<Jour>> {
    if let Some(h) = salarie.get("historique").and_then(|h| h.as_array()) {
        for e in h {
            if e.get("champ").and_then(|c| c.as_str()) == Some("nir") && strip(&chaine(e, "valeur")).is_empty() {
                let j = e.get("jusqu_au").ok_or_else(|| crate::fichiers::absent("jusqu_au"))?;
                return Ok(Some(crate::date::d(j)?.plus_jours(1)?));
            }
        }
    }
    Ok(None)
}

/// Cette declaration-ci doit-elle porter le NTT a cote du NIR ?
pub fn accompagne_le_nir(salarie: &Value, date_ref: &Jour) -> Resultat<bool> {
    Ok(match depuis_quand_le_nir(salarie)? {
        None => false,
        Some(depuis) => depuis >= Jour { an: date_ref.an, mois: date_ref.mois, jour: 1 },
    })
}

/// Faut-il ecrire S21.G00.30.020 dans cette declaration ?
pub fn besoin(salarie: &Value, date_ref: &Jour) -> Resultat<bool> {
    Ok(nir_de(salarie).is_empty() || accompagne_le_nir(salarie, date_ref)?)
}

/// S21.G00.30.001, quand il y en a un -- et rien du tout sinon.
pub fn poser_nir(f: &mut Fichier, salarie: &Value) -> Resultat<()> {
    let nir = nir_de(salarie);
    if !nir.is_empty() {
        f.add(RUBRIQUE_NIR, nir);
    }
    Ok(())
}

/// S21.G00.30.020, apres le matricule.
pub fn poser_ntt(f: &mut Fichier, ent: &Value, salarie: &Value, date_ref: &Jour) -> Resultat<()> {
    if besoin(salarie, date_ref)? {
        f.add(RUBRIQUE, numero(ent, salarie)?);
    }
    Ok(())
}

/// S21.G00.30.020 d'une DSN mensuelle, ou l'on sait ce qui a ete DECLARE pour ce salarie (vague 3d) : `ntt_declare`
/// est le NTT de sa derniere declaration quand elle ne portait pas de NIR. Si la fiche a maintenant un NIR, c'est la
/// premiere DSN qui le porte : le NTT declare l'accompagne, quel que soit le mois ou le NIR a pris effet (« Le NTT doit
/// etre renseigne sur la premiere DSN ou le NIR est attribue. Ceci permettra de faire le lien entre les deux
/// identites », p. 175 ; « le NTT le plus ancien en cours de validite », meme page). Sinon, `poser_ntt`.
pub fn poser_ntt_declare(f: &mut Fichier, ent: &Value, salarie: &Value, date_ref: &Jour, ntt_declare: Option<&str>) -> Resultat<()> {
    match ntt_declare.map(strip).filter(|n| !n.is_empty()) {
        Some(n) if !nir_de(salarie).is_empty() => {
            f.add(RUBRIQUE, n);
            Ok(())
        }
        _ => poser_ntt(f, ent, salarie, date_ref),
    }
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
