//! LES ECARTS DE CALCUL DE L'IMPORT : a la fin de chaque import, chaque paie importee se compare a ce que ce moteur
//! aurait calcule de la meme saisie (`engagement::ecarts`), et les ecarts partent chez l'editeur, pour voir ou deux
//! logiciels lisent les textes autrement. Les engagements importes restent tels que l'autre logiciel les a ecrits ; rien
//! ne s'ecrit dans la compagnie que la ligne du journal des incidents.
//!
//! UN INCIDENT DE FONCTIONNEMENT COMME LES AUTRES : rien ne s'affiche ni ne se demande. Nature `logiciel`, `ou` =
//! `ecarts_import`, un incident par sorte d'ecart (la cotisation, la grandeur, le logiciel d'origine) pour que le
//! serveur les regroupe d'une compagnie a l'autre ; une paie qui ne se compare pas en fait un par raison, et les
//! libelles de cotisation que la table de lecture ne rattache a rien, un par logiciel. Puis l'envoi ordinaire
//! (`incidents::envoyer`), qui ne part que si l'envoi a l'editeur est actif sur ce poste.
//!
//! CE QUI PART NE DESIGNE PERSONNE : ni nom, prenom, NIR, naissance ou adresse, ni salaire, brut, net ou montant d'une
//! paie. Seulement la cotisation et les libelles du bulletin, la grandeur, notre taux et le leur, l'ecart RELATIF d'une
//! base ou d'un montant, le mois, l'IDCC et l'annexe de la convention, la nature et le statut, le logiciel reconnu.
//! Sans le fil ni les ecritures du diagnostic (`Note::sans_contexte`) : leurs charges de routes portent des montants.
//! Le texte passe quand meme par le caviardage (`caviardage`, les fiches de la compagnie connues), et un montant ecrit
//! dans une raison (un refus du calcul) devient `<MONTANT>`.
//!
//! UN RAPPORT DEJA NOTE NE SE NOTE PAS DEUX FOIS (meme empreinte, meme resume, meme detail) : un import rejoue ne
//! renvoie rien.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::caviardage::{self, Connus};
use crate::incidents;
use droit::contexte::Contexte;
use engagement::ecarts::{Comparaison, Grandeur};
use socle::date::en_texte;
use socle::fichiers::vrai;

/// L'endroit, dans le journal des incidents.
pub const OU: &str = "ecarts_import";

/// Un incident a noter : son genre (ce qui le regroupe), son resume, son detail.
#[derive(Debug, Clone, PartialEq)]
pub struct Rapport {
    pub genre: String,
    pub resume: String,
    pub detail: Value,
}

fn montant() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"\d[\d\u{a0}\u{202f} .]*[,.]\d{2}\b").expect("motif d'un montant"))
}

/// Le texte tel qu'il part : caviarde.
fn propre(texte: &str, connus: &Connus) -> String {
    caviardage::caviarder(texte, connus).texte
}

fn propre_json(v: &Value, connus: &Connus) -> Value {
    match v {
        Value::String(s) => Value::String(propre(s, connus)),
        Value::Array(a) => Value::Array(a.iter().map(|x| propre_json(x, connus)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), propre_json(x, connus))).collect()),
        autre => autre.clone(),
    }
}

/// Les rapports des comparaisons de tous les engagements importes de la compagnie.
pub fn rapports(ctx: &Contexte) -> Vec<Rapport> {
    let table = match crate::bulletin_lu::Table::livree() {
        Ok(t) => t,
        Err(m) => return vec![Rapport { genre: "table de lecture illisible".into(), resume: m, detail: json!({}) }],
    };
    let rattacher = |l: &str| table.cotisation(l).map(str::to_string);
    let importes = engagement::importe::tous(&ctx.racine).unwrap_or_default();
    let comparaisons: Vec<Comparaison> = importes.iter().flat_map(|i| engagement::ecarts::comparer(&ctx.racine, i, &rattacher)).collect();
    let texte = |k: &str| ctx.entreprise.get(k).filter(|v| vrai(v)).map(en_texte);
    let (idcc, annexe) = (texte("idcc"), texte("annexe_convention"));
    let mut connus = Connus::default();
    crate::importation_incident::ajouter_les_fiches(ctx, &mut connus, std::iter::empty());
    let logiciel = |c: &Comparaison| c.logiciel.clone().unwrap_or_else(|| "autre".into());
    let cas = |c: &Comparaison| json!({"mois": c.mois, "nature": c.nature, "statut": c.statut});

    let mut ecarts: BTreeMap<(String, Grandeur, String), (Vec<String>, Vec<Value>)> = BTreeMap::new();
    let mut refus: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut seules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in &comparaisons {
        match &c.ecarts {
            Err(raison) => refus.entry(montant().replace_all(&propre(raison, &connus), "<MONTANT>").into_owned()).or_default().push(json!({"mois": c.mois, "logiciel": c.logiciel, "nature": c.nature, "statut": c.statut})),
            Ok(liste) => {
                for e in liste {
                    let (libelles, v) = ecarts.entry((e.cotisation.clone(), e.grandeur, logiciel(c))).or_default();
                    for l in &e.libelles {
                        if !libelles.contains(l) {
                            libelles.push(l.clone());
                        }
                    }
                    let mut x = cas(c);
                    let j = e.to_json();
                    for k in ["notre_taux", "leur_taux", "ecart_relatif"] {
                        x[k] = j[k].clone();
                    }
                    v.push(x);
                }
            }
        }
        for l in &c.non_rattachees {
            let v = seules.entry(logiciel(c)).or_default();
            if !v.contains(l) {
                v.push(l.clone());
            }
        }
    }
    let mut sortie = Vec::new();
    for ((cotisation, grandeur, logiciel), (libelles, cas)) in ecarts {
        let resume = format!(
            "Écart de calcul à l'import : {} ({}), {}, bulletin {} ; {} paie(s).",
            cotisation,
            if libelles.is_empty() { "total du bulletin".to_string() } else { libelles.join(" ; ") },
            grandeur.dite(),
            logiciel,
            cas.len()
        );
        let detail = json!({
            "cotisation": cotisation, "libelles": libelles, "grandeur": grandeur.code(), "logiciel": logiciel,
            "idcc": idcc, "annexe": annexe, "cas": cas,
        });
        sortie.push(Rapport { genre: format!("{} · {} · {}", cotisation, grandeur.code(), logiciel), resume, detail });
    }
    for (raison, cas) in refus {
        let resume = format!("Paie importée non comparée : {} ; {} paie(s).", raison, cas.len());
        let genre: String = format!("non comparée : {}", raison).chars().take(120).collect();
        sortie.push(Rapport { genre, resume, detail: json!({"raison": raison, "idcc": idcc, "annexe": annexe, "cas": cas}) });
    }
    for (logiciel, libelles) in seules {
        let resume = format!("Lignes de cotisation non comparées (bulletin {}) : la table de lecture ne les rattache à aucune cotisation : {}.", logiciel, libelles.join(" ; "));
        sortie.push(Rapport { genre: format!("cotisation non rattachée · {}", logiciel), resume, detail: json!({"logiciel": logiciel, "libelles": libelles, "idcc": idcc}) });
    }
    sortie.into_iter().map(|r| Rapport { genre: propre(&r.genre, &connus), resume: propre(&r.resume, &connus), detail: propre_json(&r.detail, &connus) }).collect()
}

/// A la fin d'un import : les rapports notes au journal des incidents (sauf ceux qui y sont deja), puis l'envoi
/// ordinaire. Ne leve jamais et ne dit rien a l'ecran : rend le nombre de rapports notes.
pub fn rapporter(ctx: &Contexte) -> usize {
    let racine = &ctx.racine;
    let deja = incidents::lire(racine, Some(incidents::LOGICIEL), false).unwrap_or_default();
    let ecrit = |v: Value| incidents::nettoyer(&v, &mut Vec::new(), 600);
    let mut notes = 0;
    for r in rapports(ctx) {
        let empreinte = json!(incidents::empreinte(incidents::LOGICIEL, OU, &r.genre));
        let (resume, detail) = (ecrit(json!(r.resume)), ecrit(r.detail.clone()));
        if deja.iter().any(|l| l.get("empreinte") == Some(&empreinte) && l.get("resume") == Some(&resume) && l.get("detail") == Some(&detail)) {
            continue;
        }
        let note = incidents::Note { nature: incidents::LOGICIEL, resume: &r.resume, ou: OU, quoi: &r.genre, detail: r.detail, sans_contexte: true, ..Default::default() };
        if incidents::noter(racine, &note).is_some() {
            notes += 1;
        }
    }
    if notes > 0 {
        incidents::envoyer(racine);
    }
    notes
}
