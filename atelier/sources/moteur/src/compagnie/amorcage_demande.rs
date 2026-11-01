//! Demander a la DGFiP le taux de PAS d'un salarie qui n'en a pas de valide : qui (`a_combler`), si une demande est deja
//! partie sans reponse (`en_cours`), et le signalement d'amorcage qui la porte (`ecrire`).
//!
//! Deux usages : le motif 01 (embauche effective), qui part avec la DPAE (`depot_acte::deposer_dpae` : les signalements
//! s'ecrivent et se deposent dans le geste du depot) ; le motif 07 (changement de logiciel declaratif), que l'outil de
//! l'atelier ecrit sans le deposer (`produire`). UN SIGNALEMENT, UN INDIVIDU, UN FICHIER.
//!
//! UNE DEMANDE, UNE FOIS : un salarie dont l'engagement n'a pas de taux valide au jour de son versement est demande, sauf
//! si une demande est partie depuis le dernier taux recu pour lui (sa reponse est attendue). Ce qui ne se demande pas se
//! dit : un salarie sans NIR, une demande deja partie.

use std::path::PathBuf;

use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::{erreur, R};
use socle::valeur::valeur_en_texte;
use socle::fichiers::vrai;

/// Ce qu'on demande de produire (l'outil de l'atelier).
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

/// Un salarie dont le taux manque, et le contrat qui le declarerait.
pub struct Ligne {
    pub salarie: String,
    pub libelle: String,
    pub contrat: String,
    /// Pourquoi ce salarie ne part pas ; vide : il part.
    pub pourquoi_pas: String,
    indice: usize,
}

/// Ce que `produire` vient d'ecrire : un fichier par individu.
pub struct Lot {
    pub motif: String,
    pub dossier: PathBuf,
    pub fichiers: Vec<PathBuf>,
    pub lignes: Vec<Ligne>,
    pub ecartes: Vec<Ligne>,
    pub recapitulatif: PathBuf,
}

/// Une demande partie, dont la reponse n'est pas encore la.
pub struct EnCours {
    pub quand: Jour,
    /// La DGFiP repond « dans un délai qui ne devrait pas excéder 5 jours ouvrés » (BOI-IR-PAS-30-10-20, § 30,
    /// `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`).
    pub avant: Jour,
}

/// `"%-Ns" % texte`.
fn gauche(t: &str, n: usize) -> String {
    format!("{}{}", t, " ".repeat(n.saturating_sub(t.chars().count())))
}

/// Les chiffres d'un instant (`AAAA-MM-JJTHH:MM:SS` -> `AAAAMMJJHHMMSS`).
fn chiffres(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

/// Le jour ouvre qui tombe `combien` jours ouvres apres `depart` -- ni samedi, ni dimanche, ni fete legale.
fn jours_ouvres_apres(depart: Jour, combien: i64) -> R<Jour> {
    let mut jour = depart;
    let mut restants = combien;
    while restants > 0 {
        jour = jour.plus_jours(1).map_err(|m| erreur("OverflowError", m))?;
        if jour.jour_de_la_semaine() >= 5 {
            continue;
        }
        let feries = socle::feries::feries(jour.an).map_err(|m| erreur("ValueError", m))?;
        if feries.iter().any(|(j, _)| *j == jour) {
            continue;
        }
        restants -= 1;
    }
    Ok(jour)
}

/// LA DEMANDE DE TAUX DU SALARIE `cle` QUI ATTEND SA REPONSE : le dernier signalement d'amorcage reel parti pour lui
/// (accepte, et pas refuse au bilan : regle 5), s'il est parti apres le dernier taux recu pour lui. Il se lit au journal
/// de ce qui est parti ; rien d'autre ne le garde.
pub fn en_cours(ctx: &Contexte, cle: &str) -> R<Option<EnCours>> {
    let lignes = envois::journal::lu(&ctx.racine).map_err(|m| erreur("OSError", m))?.lignes;
    let texte = |l: &Value, k: &str| l.get(k).map(valeur_en_texte).unwrap_or_default();
    let pour_lui = |l: &Value| l["versions"].as_array().is_some_and(|v| v.iter().any(|x| texte(x, "salarie") == cle));
    let mut derniere: Option<&Value> = None;
    for l in lignes.iter() {
        if texte(l, "quoi") == "dsn"
            && texte(l, "mode") == "reel"
            && l.get("accepte").is_some_and(vrai)
            && texte(&l["detail"], "nature") == declarations::amorcage::NATURE
            && pour_lui(l)
            && !envois::etats::refuse_par_un_retour(l)?
        {
            derniere = Some(l);
        }
    }
    let Some(l) = derniere else { return Ok(None) };
    let quand = texte(l, "quand");
    let dernier_recu = droit::pas::recus(ctx, cle).map_err(|m| erreur("ValueError", m))?.last().map(|r| texte(r, "mis_a_disposition")).unwrap_or_default();
    if chiffres(&dernier_recu) >= chiffres(&quand) {
        return Ok(None);
    }
    let jour = Jour::lire(quand.get(..10).unwrap_or("")).map_err(|m| erreur("ValueError", m))?;
    Ok(Some(EnCours { quand: jour, avant: jours_ouvres_apres(jour, 5)? }))
}

/// QUI A BESOIN D'UN TAUX, et par quel contrat on le demanderait : un salarie dont un engagement de `paies` n'a pas de taux
/// DGFiP valide au jour de son versement (`droit::pas::situation`, `Engagement::date_du_versement`). Rend la liste
/// ENTIERE, ecartes compris : chacun dit pourquoi il part ou ne part pas. Un contrat par individu : le signalement en
/// porte un (1,1). `forcer` : demander encore, une demande deja partie (le signalement rejete par la plate-forme).
pub fn a_combler(ctx: &Contexte, paies: &[Paie], motif: &str, forcer: bool) -> R<Vec<Ligne>> {
    use declarations::amorcage as am;
    let ko = |m: String| erreur("ValueError", m);
    let cle_de = |p: &Paie| p.salarie.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")));
    let mut cles: Vec<String> = Vec::new();
    for p in paies {
        let c = cle_de(p)?;
        if !cles.contains(&c) {
            cles.push(c);
        }
    }
    cles.sort();
    let mut lignes = Vec::new();
    for cle in &cles {
        let mut besoin: Option<(usize, Value)> = None;
        for (i, p) in paies.iter().enumerate().filter(|(_, p)| cle_de(p).is_ok_and(|c| c == *cle)) {
            let s = droit::pas::situation(ctx, cle, Some(p.engagement.date_du_versement().map_err(ko)?)).map_err(ko)?;
            if s["situation"] == droit::pas::SANS_NIR || s.get("a_demander").is_some_and(vrai) {
                besoin = Some((i, s));
                break;
            }
        }
        let Some((i, s)) = besoin else { continue };
        // SANS NIR, LA DGFiP N'A PERSONNE A QUI REPONDRE : la demande ne part pas, et cela se dit.
        if s["situation"] == droit::pas::SANS_NIR {
            lignes.push(Ligne { salarie: cle.clone(), libelle: String::new(), contrat: String::new(), pourquoi_pas: "sans NIR dans sa fiche, la DGFiP n'a personne à qui répondre".into(), indice: i });
            continue;
        }
        let p = &paies[i];
        let mut l = Ligne {
            salarie: cle.clone(),
            libelle: s.get("libelle").map(valeur_en_texte).unwrap_or_default(),
            contrat: p.engagement.numero.clone().unwrap_or_default(),
            pourquoi_pas: String::new(),
            indice: i,
        };
        match en_cours(ctx, cle)? {
            Some(d) if !forcer => {
                l.pourquoi_pas = format!("demande déjà partie le {}, réponse attendue avant le {}", d.quand.jj_mm_aaaa(), d.avant.jj_mm_aaaa());
            }
            _ => {
                if let Err(e) = am::verifier(p, motif, am::NORMALE, None) {
                    match e {
                        declarations::fctu::Echec::Impossible(m) => l.pourquoi_pas = m,
                        declarations::fctu::Echec::Autre(m) => return Err(erreur("ErreurOperation", m)),
                    }
                }
            }
        }
        lignes.push(l);
    }
    Ok(lignes)
}

/// ECRIT LE SIGNALEMENT D'AMORCAGE de chaque ligne retenue, dans `sorties/<mois>/`, un fichier par individu
/// (`Amorcage<motif>_<salarie>_<AAAAMMJJ>.dsn`), constitue le jour `constitution`. UN NUMERO D'ORDRE PAR FICHIER, a la
/// suite de tous les signalements deja partis : la sequence ne se remet jamais a zero (cahier technique DSN, p. 136), et
/// deux signalements au meme ordre ne passent pas (p. 14) ; ecrits ensemble, ils se suivent.
pub fn ecrire(ctx: &Contexte, paies: &[Paie], retenues: &[Ligne], motif: &str, mois: &str, constitution: Jour, envoi_test: bool) -> R<Vec<PathBuf>> {
    use declarations::amorcage as am;
    let dossier = ctx.racine.join("sorties").join(mois);
    let premier = envois::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| erreur("ErreurOperation", m))? + 1;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(|m| erreur("OSError", m))?;
    let mut fichiers = Vec::new();
    for (rang, l) in retenues.iter().enumerate() {
        let options = am::Options {
            motif: motif.to_string(),
            type_declaration: am::NORMALE.to_string(),
            ordre: premier + rang as i64,
            remplace: None,
            envoi_test,
            date_constitution: Some(constitution),
        };
        let texte = am::generer(ctx, &paies[l.indice], &options, &departs).map_err(|e| erreur("ErreurOperation", e.message().to_string()))?;
        let cible = dossier.join(format!("Amorcage{}_{}_{}.dsn", motif, l.salarie, constitution.aaaammjj()));
        declarations::dsn::ecrire(&texte, &cible).map_err(|m| erreur("OSError", m))?;
        crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None)?;
        fichiers.push(cible);
    }
    Ok(fichiers)
}

/// `produire` (l'outil de l'atelier) : ecrit un signalement par individu qui a un taux a demander, et son
/// recapitulatif ; rien ne se depose. `ecrits` : les fichiers poses, au fur et a mesure.
pub fn produire(ctx: &Contexte, paies: &[Paie], d: &Demande, ecrits: &mut Vec<PathBuf>) -> R<Lot> {
    use declarations::amorcage as am;
    let mut lignes = a_combler(ctx, paies, d.motif, d.forcer)?;
    if let Some(voulus) = d.salaries {
        let mut inconnus: Vec<&String> = voulus.iter().filter(|v| !lignes.iter().any(|l| &l.salarie == *v)).collect();
        if !inconnus.is_empty() {
            inconnus.sort();
            inconnus.dedup();
            return Err(erreur(
                "ErreurOperation",
                format!(
                    "Ces salaries n'ont pas de taux a demander, ou n'ont pas de contrat a declarer dans le mois retenu : {}. Rien n'est ecrit.",
                    inconnus.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        lignes.retain(|l| voulus.contains(&l.salarie));
    }
    let (retenus, ecartes): (Vec<Ligne>, Vec<Ligne>) = lignes.into_iter().partition(|l| l.pourquoi_pas.is_empty());
    // Aucun taux a demander : rien ne s'ecrit, et il n'y a rien a dire.
    if retenus.is_empty() && ecartes.is_empty() {
        return Ok(Lot { motif: d.motif.to_string(), dossier: PathBuf::new(), fichiers: Vec::new(), lignes: Vec::new(), ecartes, recapitulatif: PathBuf::new() });
    }
    if retenus.is_empty() {
        let raisons: Vec<String> = ecartes.iter().map(|l| format!("{} ({})", l.salarie, l.pourquoi_pas)).collect();
        return Err(erreur("AmorcageImpossible", format!("Taux de PAS non demandé : {}.", raisons.join(" ; "))));
    }
    let mois = if d.mois.is_empty() { d.constitution.mois_iso() } else { d.mois.to_string() };
    let dossier = ctx.racine.join("sorties").join(&mois);
    let fichiers = ecrire(ctx, paies, &retenus, d.motif, &mois, d.constitution, d.envoi_test)?;
    ecrits.extend(fichiers.iter().cloned());
    // Le recapitulatif : ce qu'on lit avant de deposer -- et apres, pour savoir ce qu'on attend.
    let jour = d.constitution.aaaammjj();
    let mut out: Vec<String> = vec![
        "SIGNALEMENTS D'AMORCAGE DES DONNEES VARIABLES (nature 08)".into(),
        format!("Motif {} - {}", d.motif, am::libelle_motif(d.motif).map_err(|m| erreur("ValueError", m))?),
        format!("Constitues le {}", d.constitution.jj_mm_aaaa()),
        String::new(),
        format!("Un signalement par individu : {} fichier(s) a deposer.", retenus.len()),
        "La DGFiP repond sous cinq jours ouvres, par un compte-rendu metier de".into(),
        "nature 94 que la releve des retours ramassera comme les autres.".into(),
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
    std::fs::write(&recap, format!("{}\n", out.join("\n"))).map_err(|e| erreur("OSError", format!("{} : {}", recap.display(), e)))?;
    Ok(Lot { motif: d.motif.to_string(), dossier, fichiers, lignes: retenus, ecartes, recapitulatif: recap })
}
