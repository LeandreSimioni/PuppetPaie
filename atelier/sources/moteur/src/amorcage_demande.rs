//! Demander a la DGFiP les taux de PAS qui manquent : qui, et comment (`moteur/operations_amorcage.py` : `a_combler`,
//! `produire`, `recapitulatif`). Lot J2b, theme 1c.
//!
//! Deux usages : le motif 07 (changement de logiciel declaratif, une fois, pour tout le monde) et le motif 01 (embauche
//! effective, qui part avec la DPAE : `crate::routes::depots::dpae` y passe aussi). UN SIGNALEMENT, UN INDIVIDU, UN FICHIER.
//! RIEN N'EST DEPOSE ici : le depot est un acte a part (`depot-dsn`), et c'est lui qui note la demande.
//!
//! Ce qui ne se demande pas : un salarie sans NIR, et un salarie dont la demande est deja partie et dont la reponse n'est pas
//! echue (`forcer` : le signalement rejete par la plate-forme, pas l'ordinaire).

use std::path::PathBuf;

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::fichiers::vrai;

/// Ce qu'on demande de produire.
pub struct Demande<'a> {
    pub motif: &'a str,
    /// Le mois de paie ou lire le contrat a declarer (`AAAA-MM`) ; vide : le mois de la constitution.
    pub mois: &'a str,
    /// `--salarie` : seulement ceux-la (`None` : tous ceux qui ont un taux a demander).
    pub salaries: Option<&'a [String]>,
    pub envoi_test: bool,
    pub forcer: bool,
    pub constitution: Jour,
}

/// Un salarie dont le taux manque, et le contrat qui le declarerait (`ACombler`).
pub struct Ligne {
    pub salarie: String,
    pub libelle: String,
    pub contrat: String,
    /// Pourquoi ce salarie ne part pas ; vide : il part.
    pub pourquoi_pas: String,
    indice: usize,
}

/// Ce qui vient d'etre ecrit : un fichier par individu (`LotProduit`).
pub struct Lot {
    pub motif: String,
    pub dossier: PathBuf,
    pub fichiers: Vec<PathBuf>,
    pub lignes: Vec<Ligne>,
    pub ecartes: Vec<Ligne>,
    pub recapitulatif: PathBuf,
}

/// `"%-Ns" % texte`.
fn gauche(t: &str, n: usize) -> String {
    format!("{}{}", t, " ".repeat(n.saturating_sub(t.chars().count())))
}

/// `jj_mm_aaaa` d'une cle de la demande (`quand`, `reponse_attendue_avant`).
fn jj(demande: &Value, cle: &str) -> Result<String, String> {
    documents::bulletin::jj_mm_aaaa(demande.get(cle).unwrap_or(&Value::Null))
}

/// `a_combler` : qui a besoin d'un taux, et par quel contrat on le demanderait. Rend la liste ENTIERE, ecartes compris : chacun dit
/// pourquoi il part ou ne part pas. Le premier contrat de chaque individu, et un seul : le signalement en porte un (1,1).
fn a_combler(ctx: &Contexte, paies: &[Paie], forcer: bool) -> Result<Vec<Ligne>, String> {
    use declarations::amorcage as am;
    let mut par_salarie: Vec<(String, usize)> = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        let cle = p.salarie.get("_cle").map(valeur_en_texte).ok_or_else(|| socle::fichiers::absent("_cle"))?;
        if !par_salarie.iter().any(|(c, _)| *c == cle) {
            par_salarie.push((cle, i));
        }
    }
    par_salarie.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (cle, i) in &par_salarie {
        let p = &paies[*i];
        let s = droit::pas::situation(ctx, cle, None)?;
        if !s.get("a_demander").is_some_and(vrai) {
            continue;
        }
        let mut l = Ligne {
            salarie: cle.clone(),
            libelle: s.get("libelle").map(valeur_en_texte).unwrap_or_default(),
            contrat: p.engagement.numero.clone().unwrap_or_default(),
            pourquoi_pas: String::new(),
            indice: *i,
        };
        match s.get("demande").filter(|d| vrai(d)) {
            Some(demande) if !forcer => {
                l.pourquoi_pas = format!("demande deja partie le {}, reponse attendue avant le {}", jj(demande, "quand")?, jj(demande, "reponse_attendue_avant")?);
            }
            _ => {
                if let Err(e) = am::verifier(p, am::CHANGEMENT_DE_LOGICIEL, am::NORMALE, None) {
                    match e {
                        declarations::fctu::Echec::Impossible(m) => l.pourquoi_pas = m,
                        declarations::fctu::Echec::Autre(m) => return Err(m),
                    }
                }
            }
        }
        lignes.push(l);
    }
    Ok(lignes)
}

/// `produire` : ecrit un fichier de signalement par individu, dans `sorties/AAAA-MM/`, et son recapitulatif. Un refus est une phrase
/// (`ErreurOperation` ou `AmorcageImpossible` du Python). `ecrits` : les fichiers poses, au fur et a mesure.
pub fn produire(ctx: &Contexte, paies: &[Paie], d: &Demande, ecrits: &mut Vec<PathBuf>) -> Result<Lot, String> {
    use declarations::amorcage as am;
    let mut lignes = a_combler(ctx, paies, d.forcer)?;
    if let Some(voulus) = d.salaries {
        let mut inconnus: Vec<&String> = voulus.iter().filter(|v| !lignes.iter().any(|l| &l.salarie == *v)).collect();
        if !inconnus.is_empty() {
            inconnus.sort();
            inconnus.dedup();
            return Err(format!(
                "Ces salaries n'ont pas de taux a demander, ou n'ont pas de contrat a declarer dans le mois retenu : {}. Rien n'est ecrit.",
                inconnus.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        lignes.retain(|l| voulus.contains(&l.salarie));
    }
    let (retenus, ecartes): (Vec<Ligne>, Vec<Ligne>) = lignes.into_iter().partition(|l| l.pourquoi_pas.is_empty());
    if retenus.is_empty() {
        let raisons: Vec<String> = ecartes.iter().map(|l| format!("{} ({})", l.salarie, l.pourquoi_pas)).collect();
        return Err(format!("Aucun signalement a produire : {}", if raisons.is_empty() { "tous les taux sont valides.".to_string() } else { raisons.join("; ") }));
    }
    let mois = if d.mois.is_empty() { d.constitution.mois_iso() } else { d.mois.to_string() };
    let dossier = ctx.racine.join("sorties").join(&mois);
    let jour = d.constitution.aaaammjj();
    // UN NUMERO D'ORDRE PAR FICHIER, a la suite de tous les signalements deja partis : la sequence ne se remet
    // jamais a zero (cahier technique DSN, p. 136), et deux signalements au meme ordre ne passent pas (p. 14).
    let premier = envois::journal::dernier_ordre_des_signalements(&ctx.racine)? + 1;
    let mut fichiers = Vec::new();
    for (rang, l) in retenus.iter().enumerate() {
        let nom = format!("Amorcage{}_{}_{}.dsn", d.motif, l.salarie, jour);
        let options = am::Options {
            motif: d.motif.to_string(),
            type_declaration: am::NORMALE.to_string(),
            ordre: premier + rang as i64,
            remplace: None,
            envoi_test: d.envoi_test,
            date_constitution: Some(d.constitution),
        };
        let texte = am::generer(ctx, &paies[l.indice], &options).map_err(|e| e.message().to_string())?;
        let cible = dossier.join(&nom);
        declarations::dsn::ecrire(&texte, &cible)?;
        crate::depart::noter_piece_ecrite(&ctx.racine, &crate::depart::cle_de_piece(&cible), None).map_err(|e| e.message)?;
        ecrits.push(cible.clone());
        fichiers.push(cible);
    }
    // Le recapitulatif : ce qu'on lit avant de deposer -- et apres, pour savoir ce qu'on attend.
    let mut out: Vec<String> = vec![
        "SIGNALEMENTS D'AMORCAGE DES DONNEES VARIABLES (nature 08)".into(),
        format!("Motif {} - {}", d.motif, am::libelle_motif(d.motif)?),
        format!("Constitues le {}", d.constitution.jj_mm_aaaa()),
        String::new(),
        format!("Un signalement par individu : {} fichier(s) a deposer.", retenus.len()),
        "La DGFiP repond sous cinq jours ouvres, par un compte-rendu metier de".into(),
        "nature 94 que `moteur retours` ramassera comme les autres.".into(),
        String::new(),
        "DEMANDES :".into(),
    ];
    for l in &retenus {
        let contrat = if l.contrat.is_empty() { "?" } else { &l.contrat };
        out.push(format!("  {} {} contrat {} ({})", gauche(&l.salarie, 14), gauche(&l.libelle, 22), contrat, mois));
    }
    if !ecartes.is_empty() {
        out.push(String::new());
        out.push("NON DEMANDES :".into());
        for l in &ecartes {
            out.push(format!("  {} {}", gauche(&l.salarie, 14), l.pourquoi_pas));
        }
    }
    let recap = dossier.join(format!("Amorcage{}_{}_recapitulatif.txt", d.motif, jour));
    std::fs::write(&recap, format!("{}\n", out.join("\n"))).map_err(|e| format!("{} : {}", recap.display(), e))?;
    Ok(Lot { motif: d.motif.to_string(), dossier, fichiers, lignes: retenus, ecartes, recapitulatif: recap })
}
