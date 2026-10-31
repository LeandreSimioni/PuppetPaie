//! LES ECARTS DE CALCUL DE L'IMPORT : a la fin de chaque import, et a l'ouverture de la compagnie, chaque paie importee
//! pas encore comparee se compare a ce que ce moteur aurait calcule de la meme saisie (`engagement::ecarts`), et les
//! ecarts partent chez l'editeur, pour voir ou deux logiciels lisent les textes autrement. Les engagements importes
//! restent tels que l'autre logiciel les a ecrits ; rien ne s'ecrit dans la compagnie que la ligne du journal des
//! incidents et le registre des paies comparees (`journal/ecarts-import.json`, leurs empreintes).
//!
//! UN INCIDENT DE FONCTIONNEMENT COMME LES AUTRES : rien ne s'affiche ni ne se demande. Nature `logiciel`, `ou` =
//! `ecarts_import`, un incident par sorte d'ecart (la cotisation, la grandeur, le logiciel d'origine) pour que le
//! serveur les regroupe d'une compagnie a l'autre ; une paie qui ne se compare pas en fait un par raison, et les
//! libelles de cotisation que la table de lecture ne rattache a rien, un par logiciel. Puis l'envoi ordinaire
//! (`incidents::envoyer`), qui ne part que si l'envoi a l'editeur est actif sur ce poste.
//!
//! CE QUI PART NE DESIGNE PERSONNE : ni nom, prenom, NIR, naissance ou adresse, ni salaire, brut, net ou montant d'une
//! paie. Seulement la cotisation et les libelles du bulletin, la grandeur, notre taux et le leur, l'ecart RELATIF d'une
//! base ou d'un montant, le mois, l'IDCC et l'annexe de la convention, l'organisation patronale de la compagnie (elle
//! change le taux de la retraite complementaire), la nature, le statut et la qualite de cadre,
//! d'ou vient le taux AT/MP du calcul (la compagnie ou le bulletin), le logiciel reconnu.
//! Sans le fil ni les ecritures du diagnostic (`Note::sans_contexte`) : leurs charges de routes portent des montants.
//! Le texte passe quand meme par le caviardage (`caviardage`, les fiches de la compagnie connues), et un montant ecrit
//! dans une raison (un refus du calcul) devient `<MONTANT>`.
//!
//! UNE PAIE SE COMPARE UNE FOIS, sauf quand le calcul l'a refusee (une configuration a completer) : elle se compare de
//! nouveau a l'ouverture suivante. UN RAPPORT DEJA NOTE NE SE NOTE PAS DEUX FOIS (meme empreinte, meme resume, meme
//! detail) : un import rejoue ne renvoie rien.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::import::caviardage::{self, Connus};
use crate::compagnie::incidents;
use droit::contexte::Contexte;
use engagement::ecarts::{Comparaison, Grandeur};
use socle::date::en_texte;
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;

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

/// Le registre des paies importees deja comparees (leurs empreintes, `engagement::ecarts::empreinte`).
fn registre(racine: &Path) -> PathBuf {
    racine.join("journal").join("ecarts-import.json")
}

fn comparees(racine: &Path) -> BTreeSet<String> {
    socle::fichiers::lire_json(&registre(racine)).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
}

/// Les rapports des paies importees de la compagnie que le registre ne dit pas deja comparees, et les empreintes des
/// paies qui ne sont plus a comparer : comparees, ou que leurs pieces empechent de comparer (une paie que le calcul a
/// refusee se compare de nouveau la fois suivante).
pub fn rapports(ctx: &Contexte) -> (Vec<Rapport>, Vec<String>) {
    let table = match crate::import::bulletin_lu::Table::livree() {
        Ok(t) => t,
        Err(m) => return (vec![Rapport { genre: "table de lecture illisible".into(), resume: m, detail: json!({}) }], Vec::new()),
    };
    let rattacher = |l: &str| table.cotisation(l).map(str::to_string);
    let importes = engagement::importe::tous(&ctx.racine).unwrap_or_default();
    let deja = comparees(&ctx.racine);
    let comparaisons: Vec<Comparaison> = importes.iter().flat_map(|i| engagement::ecarts::comparer(&ctx.racine, i, &rattacher, &|e| deja.contains(e))).collect();
    let faites: Vec<String> = comparaisons.iter().filter(|c| !c.a_refaire).map(|c| c.empreinte.clone()).collect();
    (rapports_de(ctx, &comparaisons), faites)
}

fn rapports_de(ctx: &Contexte, comparaisons: &[Comparaison]) -> Vec<Rapport> {
    let texte = |k: &str| ctx.entreprise.get(k).filter(|v| vrai(v)).map(en_texte);
    let (idcc, annexe, organisation) = (texte("idcc"), texte("annexe_convention"), texte("organisation_patronale"));
    let mut connus = Connus::default();
    ajouter_les_fiches(ctx, &mut connus);
    let logiciel = |c: &Comparaison| c.logiciel.clone().unwrap_or_else(|| "autre".into());
    let cas = |c: &Comparaison| json!({"mois": c.mois, "nature": c.nature, "statut": c.statut, "cadre": c.cadre, "taux_atmp": c.taux_atmp});

    let mut ecarts: BTreeMap<(String, Grandeur, String), (Vec<String>, Vec<Value>)> = BTreeMap::new();
    let mut refus: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut seules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in comparaisons {
        match &c.ecarts {
            Err(raison) => refus.entry(montant().replace_all(&propre(raison, &connus), "<MONTANT>").into_owned()).or_default().push(json!({"mois": c.mois, "logiciel": c.logiciel, "nature": c.nature, "statut": c.statut, "cadre": c.cadre})),
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
            "idcc": idcc, "annexe": annexe, "organisation_patronale": organisation, "cas": cas,
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

/// A la fin d'un import : les rapports notes, puis l'envoi ordinaire. Ne leve jamais et ne dit rien a l'ecran : rend le
/// nombre de rapports notes.
pub fn rapporter(ctx: &Contexte) -> usize {
    let notes = noter(ctx);
    if notes > 0 {
        incidents::envoyer(&ctx.racine);
    }
    notes
}

/// Les paies importees pas encore comparees se comparent, leurs rapports se notent au journal des incidents (sauf ceux
/// qui y sont deja), et le registre retient celles qui ne sont plus a comparer. A l'ouverture de la compagnie comme a
/// la fin d'un import : une paie qu'une configuration incomplete empechait de calculer (l'effectif de l'entreprise...)
/// se compare des que la compagnie est complete. Ne leve jamais ; rend le nombre de rapports notes.
pub fn noter(ctx: &Contexte) -> usize {
    let racine = &ctx.racine;
    if !engagement::importe::tous(racine).is_ok_and(|v| !v.is_empty()) {
        return 0;
    }
    let (rapports, faites) = rapports(ctx);
    let deja = incidents::lire(racine, Some(incidents::LOGICIEL), false).unwrap_or_default();
    let mut notes = 0;
    for r in rapports {
        let empreinte = json!(incidents::empreinte(incidents::LOGICIEL, OU, &r.genre));
        // Ce qui reconnait un rapport deja note : le journal tronque les longs textes, le rapport se reconnait a la
        // signature de ce qu'il dit en entier.
        let signature = json!(socle::fichiers::sha256_hex(format!("{}|{}", r.resume, r.detail).as_bytes())[..16].to_string());
        if deja.iter().any(|l| l.get("empreinte") == Some(&empreinte) && l.pointer("/detail/signature") == Some(&signature)) {
            continue;
        }
        let mut detail = r.detail;
        detail["signature"] = signature;
        let note = incidents::Note { nature: incidents::LOGICIEL, resume: &r.resume, ou: OU, quoi: &r.genre, detail, sans_contexte: true, ..Default::default() };
        if incidents::noter(racine, &note).is_some() {
            notes += 1;
        }
    }
    if !faites.is_empty() {
        let mut toutes = comparees(racine);
        toutes.extend(faites);
        let _ = socle::ecriture::ecrire_json(&registre(racine), &json!(toutes), 0);
    }
    notes
}

/// Ce que les fiches de la compagnie disent de ses salaries, ajoute aux valeurs connues ; la compagnie, protegee.
fn ajouter_les_fiches(ctx: &Contexte, c: &mut Connus) {
    let pousser = |v: &mut Vec<String>, x: Option<String>| {
        if let Some(x) = x.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            if !v.contains(&x) {
                v.push(x);
            }
        }
    };
    let fiches = ctx
        .salaries
        .iter()
        .filter(|(k, v)| !k.starts_with('_') && v.is_object())
        .map(|(_, v)| v.clone());
    for f in fiches {
        let t = |k: &str| f.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
        for k in ["nom", "nom_affichage"] {
            pousser(&mut c.noms, t(k));
        }
        pousser(&mut c.noms, t("code_salarie").filter(|x| x.chars().filter(|c| c.is_alphabetic()).count() >= 2));
        for k in ["prenom", "prenom_affichage"] {
            pousser(&mut c.prenoms, t(k));
        }
        pousser(&mut c.nirs, t("nir"));
        if let (Some(n), Some(k)) = (t("nir"), t("nir_cle")) {
            pousser(&mut c.nirs, Some(format!("{}{}", n, k)));
        }
        pousser(&mut c.naissances, t("date_naissance"));
        for k in ["adresse", "adresse_affichage"] {
            pousser(&mut c.adresses, t(k));
        }
        pousser(&mut c.codes_postaux, t("code_postal"));
        for k in ["commune", "commune_affichage", "lieu_naissance", "lieu_naissance_affichage"] {
            pousser(&mut c.communes, t(k));
        }
        pousser(&mut c.courriels, t("email"));
        pousser(&mut c.conges_spectacles, t("num_conges_spectacles"));
    }
    let e = &ctx.entreprise;
    for k in ["raison_sociale", "nom_affichage", "adresse", "adresse_affichage", "code_postal", "commune", "commune_affichage"] {
        pousser(&mut c.entreprise, e.get(k).filter(|v| vrai(v)).map(valeur_en_texte));
    }
}
