//! La version de la norme DSN (NEODeS) qu'un fichier porte, `S10.G00.00.006`.
//!
//! Elle ne vient plus d'une constante : `logiciel/donnees/norme-dsn-base.json` dit, par version, ce qui la declenche
//! (le mois principal declare d'une DSN mensuelle, le jour de constitution d'un signalement), ses tables d'usages et de
//! formats, les valeurs de liste qu'elle n'admet plus et le code de la CET patronale. Le moteur applique la donnee ;
//! ce qu'elle ne couvre pas refuse. LA REGLE DE BASCULE EST UNE INFERENCE (`_inference` de la donnee) : le cahier
//! technique 2027.1.1 ne la donne pas (p. 129, une seule valeur par cahier ; lot N9, `fiches-N9.md` § 4).
//!
//! Le fichier en cours de fabrication porte sa norme (`dsn::Fichier::en_norme`) : « ce fichier est-il en P27V01 ? »
//! se demande a lui, par la table de sa version (`Norme::admet`), en un seul endroit.

use crate::date::Jour;
use crate::fichiers::{dossier_du_code, json_en_cache};
use crate::num::Resultat;
use crate::usages::{lignes, rubrique_valeur};
use serde_json::Value;
use std::sync::Arc;

const BASE: &str = "norme-dsn-base.json";
pub const RUBRIQUE: &str = "S10.G00.00.006";

/// Une version de la norme que le moteur sait ecrire (`P26V01`, `P27V01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Norme {
    pub version: String,
}

fn base() -> Resultat<Arc<Value>> {
    json_en_cache(&dossier_du_code().join(BASE))
}

/// Les versions livrees, dans l'ordre de la donnee.
pub fn versions() -> Resultat<Vec<Norme>> {
    let b = base()?;
    let v = b["versions"].as_object().ok_or_else(|| format!("{} : pas de `versions`", BASE))?;
    Ok(v.keys().map(|k| Norme { version: k.clone() }).collect())
}

fn liste_des_versions() -> Resultat<String> {
    Ok(versions()?.iter().map(|n| n.version.clone()).collect::<Vec<_>>().join(", "))
}

fn texte_de(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

impl Norme {
    /// La version nommee, si la donnee la connait.
    pub fn connue(version: &str) -> Resultat<Norme> {
        if base()?["versions"].get(version).is_some() {
            return Ok(Norme { version: version.to_string() });
        }
        Err(format!(
            "Version de la norme DSN « {} » inconnue du logiciel (versions livrees : {} ; logiciel/donnees/{}). Rien n'est ecrit.",
            version,
            liste_des_versions()?,
            BASE
        ))
    }

    fn entree(&self) -> Resultat<Value> {
        Ok(base()?["versions"][&self.version].clone())
    }

    /// La table des usages de cette version (un fichier de `logiciel/donnees/`).
    pub fn table_usages(&self) -> Resultat<String> {
        texte_de(&self.entree()?["usages"]).ok_or_else(|| format!("{} : la version {} ne nomme pas sa table d'usages", BASE, self.version))
    }

    /// La table des formats de cette version.
    pub fn table_formats(&self) -> Resultat<String> {
        texte_de(&self.entree()?["formats"]).ok_or_else(|| format!("{} : la version {} ne nomme pas sa table de formats", BASE, self.version))
    }

    /// La rubrique est-elle admise (O ou C) pour cette nature dans le tableau des usages de CETTE version ? C'est la
    /// question que les generateurs posent avant d'ecrire une rubrique qu'une version a retiree (06.003, 11.002 en
    /// P27V01) ou ajoutee.
    pub fn admet(&self, rubrique: &str, nature: &str) -> Resultat<bool> {
        let u = crate::usages::usage_en(self, rubrique, nature)?;
        Ok(u == "O" || u == "C")
    }

    /// Pourquoi cette valeur n'est plus dans la liste de la rubrique pour cette version (le texte de la donnee), ou
    /// `None`.
    pub fn valeur_retiree(&self, rubrique: &str, valeur: &str) -> Resultat<Option<String>> {
        Ok(texte_de(&self.entree()?["valeurs_retirees"][rubrique][valeur]))
    }

    /// Pourquoi cette rubrique ne se renseigne pas dans cette version alors que son tableau des usages l'admet (le texte
    /// de la donnee, `rubriques_a_ne_pas_renseigner`), ou `None`.
    pub fn rubrique_a_ne_pas_renseigner(&self, rubrique: &str) -> Resultat<Option<String>> {
        Ok(texte_de(&self.entree()?["rubriques_a_ne_pas_renseigner"][rubrique]))
    }

    /// Le texte du cahier qui porte la regle d'ecriture `nom` (`regles` de la donnee) si cette version l'applique, `None`
    /// sinon : ce que la version ajoute a la precedente et qui ne se lit pas dans ses tables d'usages et de formats (une
    /// valeur a ne pas depasser, une datation, la definition d'un bloc).
    pub fn regle(&self, nom: &str) -> Resultat<Option<String>> {
        Ok(texte_de(&self.entree()?["regles"][nom]))
    }

    /// Le code de cotisation (S21.G00.81.001) sous lequel la CET patronale se declare dans cette version ; `Err` avec
    /// la raison si la donnee ne le dit pas.
    pub fn cotisation_cet_patronale(&self) -> Resultat<String> {
        let e = self.entree()?;
        texte_de(&e["cotisation_cet_patronale"]).ok_or_else(|| {
            format!(
                "Contribution d'equilibre technique (CET) patronale dans un fichier en norme {} : le code de cotisation \
(S21.G00.81.001) sous lequel elle se declare n'est pas tranche. {} (logiciel/donnees/{}, `cotisation_cet_patronale`.) \
Rien n'est ecrit.",
                self.version,
                texte_de(&e["_pourquoi_cet"]).unwrap_or_default(),
                BASE
            )
        })
    }
}

/// `"AAAA-MM"` dans [du, au] (bornes ouvertes si absentes) ; la comparaison de textes de meme forme suffit.
fn dans(v: &str, plage: &Value) -> bool {
    let du = plage["du"].as_str();
    let au = plage["au"].as_str();
    du.map_or(true, |d| d <= v) && au.map_or(true, |a| v <= a)
}

fn choisir(cle: &str, valeur: &str) -> Resultat<Option<Norme>> {
    let b = base()?;
    let v = b["versions"].as_object().ok_or_else(|| format!("{} : pas de `versions`", BASE))?;
    let siennes: Vec<&String> = v.iter().filter(|(_, e)| e.get(cle).map_or(false, |p| dans(valeur, p))).map(|(k, _)| k).collect();
    match siennes.as_slice() {
        [] => Ok(None),
        [k] => Ok(Some(Norme { version: (*k).clone() })),
        _ => Err(format!(
            "{} : {} versions couvrent {} ({}) -- la donnee se contredit, rien n'est ecrit.",
            BASE,
            siennes.len(),
            valeur,
            siennes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn plages(cle: &str) -> Resultat<String> {
    let b = base()?;
    let v = b["versions"].as_object().ok_or_else(|| format!("{} : pas de `versions`", BASE))?;
    Ok(v.iter()
        .map(|(k, e)| {
            let p = &e[cle];
            let borne = |x: &Value| x.as_str().unwrap_or("...").to_string();
            format!("{} du {} au {}", k, borne(&p["du"]), borne(&p["au"]))
        })
        .collect::<Vec<_>>()
        .join(" ; "))
}

/// La version d'une DSN mensuelle, d'apres son mois principal declare (`AAAA-MM`).
pub fn de_la_mensuelle(mois: &str) -> Resultat<Norme> {
    choisir("mensuelle", mois)?.ok_or_else(|| {
        format!(
            "DSN de {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce mois principal declare \
(logiciel/donnees/{} : {}). La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
            mois,
            BASE,
            plages("mensuelle").unwrap_or_default()
        )
    })
}

/// La version d'un signalement (arret, reprise, fin de contrat unique, amorcage), d'apres le jour ou il est constitue
/// (S20.G00.05.007) : un signalement n'a pas de mois principal declare.
pub fn du_signalement(constitution: &Jour) -> Resultat<Norme> {
    choisir("signalements", &constitution.iso())?.ok_or_else(|| {
        format!(
            "Signalement constitue le {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce jour \
(logiciel/donnees/{} : {}). La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
            constitution.jj_mm_aaaa(),
            BASE,
            plages("signalements").unwrap_or_default()
        )
    })
}

/// La version que le fichier porte lui-meme (S10.G00.00.006) ; refus si elle manque ou si le logiciel ne la connait pas
/// -- ses tables d'usages et de formats sont celles d'une version.
pub fn du_fichier(texte: &str) -> Resultat<Norme> {
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if r == RUBRIQUE {
            return Norme::connue(v).map_err(|e| {
                format!("{} = '{}' : le fichier ne se controle pas, ses tables d'usages et de formats sont celles d'une version. {}", RUBRIQUE, v, e)
            });
        }
    }
    Err(format!(
        "Le fichier ne porte pas {} (numero de version de la norme) : ses tables d'usages et de formats ne se choisissent \
pas. Rien n'est ecrit.",
        RUBRIQUE
    ))
}

/// Les valeurs que la version du fichier n'admet plus dans leur liste (`valeurs_retirees`), et les rubriques que son
/// tableau des usages admet mais que sa description dit « a ne pas renseigner » (`rubriques_a_ne_pas_renseigner`) :
/// refus, avec le texte de la donnee.
pub fn verifier_les_valeurs(texte: &str, norme: &Norme) -> Resultat<()> {
    let mut refus = Vec::new();
    let mut a_ne_pas = Vec::new();
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if let Some(pourquoi) = norme.valeur_retiree(r, v)? {
            let dit = format!("    {} = '{}' : {}", r, v, pourquoi);
            if !refus.contains(&dit) {
                refus.push(dit);
            }
        }
        if let Some(pourquoi) = norme.rubrique_a_ne_pas_renseigner(r)? {
            let dit = format!("    {} : {}", r, pourquoi);
            if !a_ne_pas.contains(&dit) {
                a_ne_pas.push(dit);
            }
        }
    }
    if !a_ne_pas.is_empty() {
        return Err(format!(
            "Rubrique(s) que la norme {} dit << a ne pas renseigner >> :\n{}\n  logiciel/donnees/{}, \
`rubriques_a_ne_pas_renseigner`. Rien n'est ecrit.",
            norme.version,
            a_ne_pas.join("\n"),
            BASE
        ));
    }
    if refus.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Valeur(s) que la norme {} n'admet plus dans leur liste -- la plate-forme refuserait le fichier :\n{}\n  \
logiciel/donnees/{}, `valeurs_retirees`. Rien n'est ecrit.",
        norme.version,
        refus.join("\n"),
        BASE
    ))
}
