//! Les routes des IMPREVUS d'un salarie : la liste (`imprevus`, `arrets`, `fins-de-contrat`, `arret-contrats`), l'arret
//! saisi, repris, prolonge, signale ou oublie (`arret-enregistrer`, `arret-reprise`, `arret-prolonger`,
//! `arret-signaler`, `arret-oublier`), son maintien de salaire (`arret-maintien`), la suspension du bloc 65
//! (`suspension-enregistrer`, `-modifier`, `-oublier`), et la fin d'un contrat signalee, avec ses documents
//! (`fin-contrat-signaler`, `fin-contrat-documents`).
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN ; un signalement s'ecrit en fichier, il ne s'envoie pas. Les refus sont des
//! `ErreurOperation` (la page les montre tels quels) ; un refus n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (rien de remplissable s'il n'y a pas a le remplir). Le moteur dit, sur chaque
//! imprevu resume (`resume`), ce que ses faits ouvrent : `maintien_ouvert` (le contrat est paye au mois : un complement
//! de salaire peut s'y verser, mois par mois, `mois_du_maintien`) et `jours_deja_indemnises_attendus` (et le motif est
//! une maladie ou un accident, L1226-1 : D1226-4 compte les jours deja indemnises dans les douze mois). La page n'ouvre
//! que cela.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::routes::{brut, maintenant, qui_de, salarie};
use declarations::signalement::{self, ARRET};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use engagement::imprevus::{self, SUSPENSION};
use envois::etats::{get, getd, items, objet_vide, ou, Dico};
use socle::date::{d, en_texte, repr_texte, Jour};
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::num::{eur2, Decimal};
use socle::valeur::{nom_type, valeur_ecrite, valeur_en_texte, valeurs_egales};




/// Les routes de ce module.
pub const ROUTES: &[&str] = &[
    "imprevus", "arrets", "fins-de-contrat", "arret-contrats",
    "arret-enregistrer", "arret-reprise", "arret-oublier", "arret-maintien", "arret-prolonger", "arret-signaler",
    "suspension-enregistrer", "suspension-modifier", "suspension-oublier", "fin-contrat-signaler", "fin-contrat-documents",
];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = &[
    "arret-enregistrer", "arret-reprise", "arret-oublier", "arret-maintien", "arret-prolonger", "arret-signaler",
    "suspension-enregistrer", "suspension-modifier", "suspension-oublier", "fin-contrat-signaler", "fin-contrat-documents",
];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "imprevus" => imprevus(ctx, charge),
        "arrets" => arrets(ctx),
        "fins-de-contrat" => fins_de_contrat(ctx, charge),
        "arret-contrats" => arret_contrats(ctx, charge),
        "arret-enregistrer" => arret_enregistrer(ctx, charge),
        "arret-reprise" => arret_reprise(ctx, charge),
        "arret-oublier" => arret_oublier(ctx, charge),
        "arret-maintien" => arret_maintien(ctx, charge),
        "arret-prolonger" => arret_prolonger(ctx, charge),
        "arret-signaler" => arret_signaler(ctx, charge),
        "suspension-enregistrer" => suspension_enregistrer(ctx, charge),
        "suspension-modifier" => suspension_modifier(ctx, charge),
        "suspension-oublier" => suspension_oublier(ctx, charge),
        "fin-contrat-signaler" => fin_contrat_signaler(ctx, charge),
        "fin-contrat-documents" => fin_contrat_documents(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
pub(crate) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(cle)))
}

fn optionnel<'a>(charge: &'a Value, cle: &str) -> &'a Value {
    charge.get(cle).unwrap_or(&Value::Null)
}

pub(crate) fn jour(v: &Value) -> R<Jour> {
    d(v).map_err(|m| erreur("ValueError", m))
}

fn magasin<T>(r: Result<T, String>) -> R<T> {
    r.map_err(refus_magasin)
}

/// La cle de la fiche que `nom` designe (`KeyError` si aucune).
pub(crate) fn cle_du_salarie(ctx: &Contexte, nom: &Value) -> R<String> {
    let fiche = crate::routes::salarie(ctx, nom)?;
    Ok(valeur_en_texte(fiche.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`crate::routes::imprevus::contrats_au`), celui
/// que la charge nomme (`contrat`), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: &Value) -> R<Value> {
    let candidats = crate::routes::imprevus::contrats_au(ctx, &json!(cle), &jour.iso())?;
    let texte_de = |c: &Value, k: &str| valeur_en_texte(&c[k]);
    if vrai(voulu) {
        return match candidats.iter().find(|c| valeurs_egales(&c["numero"], voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| texte_de(c, "numero")).collect();
                Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    valeur_en_texte(voulu),
                    cle,
                    jour.jj_mm_aaaa(),
                    if liste.is_empty() { "aucun".to_string() } else { liste.join(", ") }
                )))
            }
        };
    }
    match candidats.len() {
        1 => Ok(candidats[0].clone()),
        0 => Err(refus(format!(
            "Aucun contrat de {} ne couvre le {}. Un arret ou une suspension designe le contrat suspendu par son numero et sa \
date de debut (S21.G00.40.009 et .001) : sans contrat declare, il n'y a rien a suspendre. Verifier la date, ou saisir le \
contrat.",
            cle,
            jour.jj_mm_aaaa()
        ))),
        _ => {
            let liste: Vec<String> =
                candidats.iter().map(|c| format!("{} du {} au {}", texte_de(c, "numero"), texte_de(c, "debut"), texte_de(c, "fin"))).collect();
            Err(refus(format!(
                "Deux contrats de {} couvrent le {} ({}). Le cahier technique laisse le choix au declarant : dire lequel.",
                cle,
                jour.jj_mm_aaaa(),
                liste.join(", ")
            )))
        }
    }
}

/// Le contrat de l'imprevu est-il paye au mois ? Lu dans les saisies (`contrats_au`, au jour de son debut) ; un contrat
/// que plus aucune saisie ne porte ne l'est pas.
fn mensuel(ctx: &Contexte, imp: &Value) -> R<bool> {
    let debut = valeur_en_texte(&imp["contrat_debut"]);
    let contrats = crate::routes::imprevus::contrats_au(ctx, &imp["salarie"], &debut)?;
    Ok(contrats.iter().any(|c| valeur_en_texte(&c["debut"]) == debut && c["mensuel"] == json!(true)))
}

/// UN IMPREVU NE CHEVAUCHE PAS UN AUTRE IMPREVU DU MEME CONTRAT : un jour d'absence ne se retire qu'une fois du salaire,
/// et deux periodes qui se recouvrent sur le meme contrat sont deux declarations qui se contredisent (un salarie ne
/// travaille pas le dernier jour travaille d'un arret qui court deja). `sauf` : l'imprevu lui-meme, quand il change.
pub(crate) fn sans_chevauchement(ctx: &Contexte, nouveau: &Value, sauf: Option<&Value>) -> R<()> {
    let (p, dj) = (magasin(imprevus::premier_jour(nouveau))?, magasin(imprevus::dernier_jour(nouveau))?);
    let cle = valeur_en_texte(&nouveau["salarie"]);
    let debut = jour(&nouveau["contrat_debut"])?;
    let numero = valeur_en_texte(&nouveau["contrat_numero"]);
    for autre in magasin(imprevus::tous(&ctx.racine))? {
        if sauf.is_some_and(|s| meme_imprevu(s, &autre)) || meme_imprevu(nouveau, &autre) {
            continue;
        }
        if !magasin(imprevus::du_contrat(&autre, &cle, Some(numero.as_str()), &debut))? {
            continue;
        }
        let (ap, ad) = (magasin(imprevus::premier_jour(&autre))?, magasin(imprevus::dernier_jour(&autre))?);
        if ap <= dj && ad >= p {
            return Err(refus(format!(
                "{} : du {} au {}, cet imprevu chevauche {} du {} au {} sur le meme contrat ({}). Un jour d'absence ne se \
declare et ne se retire qu'une fois : corriger les dates, ou reprendre l'autre imprevu.",
                cle,
                p.jj_mm_aaaa(),
                dj.jj_mm_aaaa(),
                if imprevus::est_un_arret(&autre) { "l'arret de travail" } else { "la suspension" },
                ap.jj_mm_aaaa(),
                ad.jj_mm_aaaa(),
                if numero.is_empty() { debut.jj_mm_aaaa() } else { numero.clone() }
            )));
        }
    }
    Ok(())
}

/// Le meme imprevu : meme genre, meme salarie, meme jour de rangement.
fn meme_imprevu(a: &Value, b: &Value) -> bool {
    imprevus::genre(a) == imprevus::genre(b)
        && valeurs_egales(&a["salarie"], &b["salarie"])
        && imprevus::jour_de_rangement(a).ok() == imprevus::jour_de_rangement(b).ok()
}

/// L'imprevu de ce genre, de ce salarie, a ce jour de rangement (le dernier jour travaille d'un arret, le debut d'une
/// suspension) : son indice et le fichier de son mois, tous genres compris.
pub(crate) fn trouver(ctx: &Contexte, genre: &str, cle: &str, jour: &Jour) -> R<Option<(usize, Vec<Value>)>> {
    let liste = magasin(imprevus::du_fichier(&ctx.racine, &jour.mois_iso()))?;
    let i = liste.iter().position(|x| {
        imprevus::genre(x) == genre && valeur_en_texte(&x["salarie"]) == cle && imprevus::jour_de_rangement(x).ok() == Some(*jour)
    });
    Ok(i.map(|i| (i, liste)))
}

/// Ecrit le fichier d'un mois du magasin.
pub(crate) fn ecrire(ctx: &Contexte, mois: &str, liste: Vec<Value>) -> R<()> {
    imprevus::ecrire(&ctx.racine, mois, liste).map_err(|m| erreur("OSError", m))
}

/// Les mois qu'un imprevu traverse, du premier au dernier jour d'absence.
fn mois_traverses(imp: &Value) -> R<Vec<String>> {
    let (p, dj) = (magasin(imprevus::premier_jour(imp))?, magasin(imprevus::dernier_jour(imp))?);
    Ok(if dj < p { Vec::new() } else { socle::date::mois_entre(&p, &dj) })
}

/// Les DSN mensuelles qui ont reporte l'imprevu (`envois` de nature 01).
fn reporte_par(imp: &Value) -> Vec<Value> {
    imp["envois"].as_array().map_or_else(Vec::new, |e| {
        e.iter().filter(|x| x.get("nature").and_then(Value::as_str) == Some("01")).map(|x| x.get("mois").cloned().unwrap_or(Value::Null)).collect()
    })
}

/// Un signalement de cette nature (04, 05) est-il parti pour cet arret (un `envois` que `marques_depot` a trace) ?
fn envoye(imp: &Value, nature: &str) -> bool {
    imp["envois"].as_array().is_some_and(|e| e.iter().any(|x| x.get("nature").and_then(Value::as_str) == Some(nature)))
}

/// UN ARRET EN COURS : celui dont on peut encore apprendre qu'il se prolonge ou que le salarie a repris plus
/// tot. Aucune reprise n'est saisie, et :
///
/// - sa fin previsionnelle n'est pas passee : le salarie est arrete aujourd'hui ;
/// - ou elle est passee, mais la DSN mensuelle du mois ou elle tombe n'est pas encore partie : l'avis de prolongation peut
///   arriver apres la fin prescrite, et une reprise anticipee se signale encore. Une fois cette DSN partie (`reporte_par`),
///   l'arret y est declare fini a sa fin previsionnelle -- « la reprise du travail de votre salarie a la date prevue doit
///   etre signalee dans la DSN mensuelle » (ameli, `atelier/reference/sources/ameli-arret-de-travail/`) --, et il n'est
///   plus en cours.
///
/// Les routes elles-memes ne s'en servent pas pour refuser : une prolongation se saisit tant qu'aucune reprise ne l'est
/// (`arret_prolonger`). C'est ce que la page offre, pas ce que le moteur admet.
pub(crate) fn en_cours(ctx: &Contexte, imp: &Value) -> R<bool> {
    if vrai(&imp["reprise"]) {
        return Ok(false);
    }
    let fin = jour(&imp["fin_previsionnelle"])?;
    if fin >= ctx.aujourdhui {
        return Ok(true);
    }
    Ok(!reporte_par(imp).iter().any(|m| valeur_en_texte(m) == fin.mois_iso()))
}

// ------------------------------------------------------------------ le resume

/// UN IMPREVU TEL QUE LA PAGE LE LIT. Un arret : le resume de toujours (`crate::routes::imprevus::resume_arret` : salarie, contrat,
/// motif et son libelle, les dates de la norme, signale, signalements, reporte_par, etat), plus `genre`, `premier_jour`,
/// `dernier_jour`, `jours_deja_indemnises`, `maintien`, `maintien_legal`, `origine`, `maintien_ouvert`,
/// `mois_du_maintien`, `jours_deja_indemnises_attendus`, `cachets`. Une suspension : `genre`, `salarie`, `contrat`,
/// `contrat_debut`, `motif`, `motif_libelle`, `remunere`, `debut`, `fin`, `premier_jour`, `dernier_jour`, `note`,
/// `saisi_le`, `reporte_par`, `cachets`.
pub(crate) fn resume(ctx: &Contexte, n: &Value, imp: &Value) -> R<Value> {
    let cachets = magasin(imprevus::cachets_dans(ctx, imp))?;
    let premier = magasin(imprevus::premier_jour(imp))?.iso();
    let dernier = magasin(imprevus::dernier_jour(imp))?.iso();
    if imprevus::est_un_arret(imp) {
        let mut r = crate::routes::imprevus::resume_arret(n, imp)?;
        let motif = magasin(imprevus::motif_d_arret(&valeur_en_texte(&imp["motif"])))?.unwrap_or(Value::Null);
        let maintien_legal = motif.get("maintien_legal").is_some_and(vrai);
        let ouvert = mensuel(ctx, imp)?;
        let o = r.as_object_mut().ok_or_else(|| erreur("TypeError", "resume"))?;
        o.insert("genre".into(), json!(ARRET));
        o.insert("premier_jour".into(), json!(premier));
        o.insert("dernier_jour".into(), json!(dernier));
        o.insert("jours_deja_indemnises".into(), imp["jours_deja_indemnises"].clone());
        o.insert("maintien".into(), imp["maintien"].clone());
        o.insert("maintien_legal".into(), json!(maintien_legal));
        o.insert("origine".into(), motif.get("origine").cloned().unwrap_or(Value::Null));
        o.insert("maintien_ouvert".into(), json!(ouvert));
        o.insert("mois_du_maintien".into(), json!(if ouvert { mois_traverses(imp)? } else { Vec::new() }));
        o.insert("jours_deja_indemnises_attendus".into(), json!(ouvert && maintien_legal));
        o.insert("cachets".into(), json!(cachets));
        // CE QUE L'ARRET OUVRE A LA PAGE (rien de remplissable s'il n'y a pas a le remplir) :
        // un arret en cours se prolonge ou se reprend plus tot ; une reprise saisie se corrige tant que son signalement
        // n'est pas parti.
        let cours = en_cours(ctx, imp)?;
        o.insert("prolongations".into(), imp["prolongations"].clone());
        o.insert("en_cours".into(), json!(cours));
        o.insert("peut_prolonger".into(), json!(cours));
        o.insert("peut_reprendre_plus_tot".into(), json!(cours));
        o.insert("reprise_corrigeable".into(), json!(vrai(&imp["reprise"]) && !envoye(imp, declarations::signalement::REPRISE)));
        return Ok(r);
    }
    let motif = magasin(imprevus::motif_de_suspension(&valeur_en_texte(&imp["motif"])))?.unwrap_or(Value::Null);
    Ok(json!({
        "genre": SUSPENSION,
        "salarie": imp["salarie"], "contrat": imp["contrat_numero"], "contrat_debut": imp["contrat_debut"],
        "motif": imp["motif"], "motif_libelle": motif.get("libelle").cloned().unwrap_or(json!("?")),
        "remunere": motif.get("remunere").is_some_and(vrai),
        "debut": imp["debut"], "fin": imp["fin"], "premier_jour": premier, "dernier_jour": dernier,
        "note": imp["note"], "saisi_le": imp["saisi_le"], "reporte_par": reporte_par(imp),
        "cachets": cachets,
    }))
}

/// Les motifs d'une suspension qui se DECLARENT (`permanents-base.json`, section `suspensions`, sans le 200).
fn motifs_de_suspension() -> R<Vec<Value>> {
    let regles = droit::permanents::regles().map_err(refus_magasin)?;
    Ok(regles["suspensions"]["motifs"]
        .as_array()
        .map_or(&[][..], |a| &a[..])
        .iter()
        .filter(|m| !m.get("_ne_pas_declarer").is_some_and(vrai))
        .map(|m| json!({"code": m["code"], "libelle": m["libelle"], "remunere": m.get("remunere").is_some_and(vrai)}))
        .collect())
}

// ------------------------------------------------------------------ imprevus

/// La route `imprevus` : `{salarie?, mois?}` -> les imprevus (d'un salarie, qui croisent un mois), resumes -- un arret
/// avec ses `obligations` --, et les nomenclatures pour en saisir.
fn imprevus(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = match charge.get("salarie").filter(|v| vrai(v)) {
        Some(nom) => Some(cle_du_salarie(ctx, nom)?),
        None => None,
    };
    let mois = charge.get("mois").filter(|v| vrai(v)).map(valeur_en_texte);
    let bornes = match &mois {
        Some(m) => Some((
            socle::date::premier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
            socle::date::dernier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
        )),
        None => None,
    };
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for imp in magasin(imprevus::tous(&ctx.racine))? {
        if cle.as_deref().is_some_and(|c| valeur_en_texte(&imp["salarie"]) != c) {
            continue;
        }
        if let Some((debut, fin)) = &bornes {
            if !magasin(imprevus::croise(&imp, debut, fin))? {
                continue;
            }
        }
        let mut r = resume(ctx, &n, &imp)?;
        // CE QU'UN ARRET FAIT NAITRE, SUR L'ARRET LUI-MEME : la page des imprevus (la carte d'un salarie, la
        // colonne du mois, son dossier) lit une seule liste, et chaque arret y porte ses obligations, calculees au meme
        // endroit que pour la route `arrets` (`obligations::de_l_arret`).
        if imprevus::est_un_arret(&imp) {
            r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, &imp, &suivi, ctx.aujourdhui)?);
        }
        out.push(r);
    }
    Ok(json!({
        "imprevus": out,
        "motifs_arret": crate::routes::imprevus::saisissables(&n, "motifs_arret")?,
        "motifs_reprise": crate::routes::imprevus::saisissables(&n, "motifs_reprise")?,
        "subrogation": n["subrogation"]["codes"],
        "motifs_suspension": motifs_de_suspension()?,
    }))
}

// ------------------------------------------------------------------ le maintien

/// Un entier positif ou nul, ou rien (`null`, `""`).
pub(crate) fn jours_ou_rien(v: &Value, quoi: &str) -> R<Value> {
    if v.is_null() || v.as_str().is_some_and(|t| t.trim().is_empty()) {
        return Ok(Value::Null);
    }
    let n = match v {
        Value::Number(x) => x.as_i64().filter(|_| x.as_f64().is_some_and(|f| f.fract() == 0.0)),
        Value::String(t) => t.trim().parse::<i64>().ok(),
        _ => None,
    };
    match n {
        Some(k) if k >= 0 => Ok(json!(k)),
        _ => Err(refus(format!("{} : un nombre entier de jours, positif ou nul, est attendu ({}).", quoi, valeur_en_texte(v)))),
    }
}

/// `arret-maintien` : `{salarie, dernier_jour, mois?, montant?, jours_deja_indemnises?}`. Le complement de salaire verse
/// au titre de l'arret pour un mois qu'il traverse (`montant` vide ou nul : retire), et les jours deja indemnises dans
/// les douze mois (D1226-4). Seulement pour un arret d'un contrat paye au mois : L1226-1 « ne s'appliquent pas [...] aux
/// salaries intermittents » (`atelier/reference/sources/code-du-travail/articles-20261015.md`), et un mois de CDDU n'a
/// pas de salaire du mois a completer.
fn arret_maintien(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let dernier = jour(exiger(charge, "dernier_jour")?)?;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let a = liste[i].clone();
    if !charge.get("mois").is_some() && !charge.get("jours_deja_indemnises").is_some() {
        return Err(refus("Dire le maintien d'un mois (`mois` et `montant`) ou les jours deja indemnises : rien n'est change."));
    }
    if !mensuel(ctx, &a)? {
        return Err(refus(format!(
            "L'arret de {} suspend le contrat {}, qui n'est pas paye au mois : le maintien de salaire de L1226-1 ne \
s'applique pas aux salaries intermittents, et un mois de CDDU n'a pas de salaire a completer. Rien n'est change.",
            cle,
            valeur_en_texte(&a["contrat_numero"])
        )));
    }
    let mut nouveau = a.clone();
    if let Some(m) = charge.get("mois") {
        let mois = valeur_en_texte(m);
        if !mois_traverses(&a)?.contains(&mois) {
            return Err(refus(format!(
                "Le maintien de {} : l'arret de {} (du {} au {}) ne traverse pas ce mois. Rien n'est change.",
                mois,
                cle,
                magasin(imprevus::premier_jour(&a))?.jj_mm_aaaa(),
                magasin(imprevus::dernier_jour(&a))?.jj_mm_aaaa()
            )));
        }
        let montant = optionnel(charge, "montant");
        let mut table = a["maintien"].as_object().cloned().unwrap_or_default();
        let vide = montant.is_null() || montant.as_str().is_some_and(|t| t.trim().is_empty());
        let valeur = if vide { None } else { Some(socle::num::dec_avec_echelle(montant).map_err(|e| refus(format!("Montant du maintien : {}", e)))?) };
        match valeur {
            Some(v) if v < socle::num::Decimal::ZERO => return Err(refus("Un maintien de salaire ne se verse pas en negatif.")),
            Some(v) if v > socle::num::Decimal::ZERO => {
                table.insert(mois, json!(socle::num::eur2(v).to_string().parse::<f64>().unwrap_or(0.0)));
            }
            _ => {
                table.remove(&mois);
            }
        }
        nouveau["maintien"] = Value::Object(table);
    }
    if let Some(j) = charge.get("jours_deja_indemnises") {
        nouveau["jours_deja_indemnises"] = jours_ou_rien(j, "Jours deja indemnises")?;
    }
    liste[i] = nouveau.clone();
    ecrire(ctx, &dernier.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"arret": resume(ctx, &n, &nouveau)?}))
}

// ------------------------------------------------------------------ la prolongation

/// `arret-prolonger` : `{salarie, dernier_jour, fin?, subrogation_fin?}` -> `{"arret": resume, "subrogation_a_revoir",
/// "phrase", "avertissements"}`. L'AVIS DE PROLONGATION D'UN ARRET EN COURS.
///
/// CE QUI PART, ET CE QUI NE PART PAS. « En cas de prolongation d'arret, il ne faut pas envoyer de nouveau signalement de
/// nature " arret de travail ". Il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle, au niveau de la
/// rubrique " Date de fin previsionnelle - S21.G00.60.003 " » (fiche net-entreprises 1983, `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l.
/// 13) ; ameli dit de meme a l'employeur en DSN : « modifier la date de fin previsionnelle de l'arret de travail dans votre
/// logiciel de paie » (`atelier/reference/sources/ameli-arret-de-travail/`). La route ne change donc que
/// `fin_previsionnelle` : le bloc S21.G00.60 de chaque DSN mensuelle que l'arret traverse la porte (`dsn::bloc_60`,
/// `imprevus::du_mois`, qui suit la nouvelle fin), la paie d'un permanent retire les jours ajoutes et le maintien s'ouvre
/// aux mois ajoutes (`imprevus::lignes_du_mois`, `mois_du_maintien`), les cachets payes dans la prolongation s'avertissent
/// (`imprevus::cachets_dans`) -- par le mecanisme de toujours, sans second. Un signalement d'arret pas encore parti partira
/// avec la nouvelle fin.
///
/// LA SUBROGATION. Elle n'est portee que par le signalement d'arret (60.004 a 60.008 : C en 01, mais le moteur ne l'ecrit
/// qu'en 04 -- `dsn::bloc_60`, tableau des usages) ; sa fin « maximale prevue dans le cadre de la convention collective ou
/// de l'accord de branche » se declare d'emblee (fiche 1983, l. 25). Si elle est « insuffisante ou erronee », un nouveau
/// signalement d'arret de type annule et remplace la porte (l. 37), « le plus rapidement possible des connaissance de la
/// prolongation » (l. 66) ; si elle « court toujours », rien (l. 110). Le moteur ne connait pas le maximum de la convention
/// (regle 1) : quand l'arret subroge finit desormais apres la subrogation, il le dit (`subrogation_a_revoir`) et la page
/// demande la fin de subrogation -- c'est `subrogation_fin`, seule ou avec `fin`. Une fin de subrogation changee apres le
/// depot du signalement d'arret fait naitre le signalement rectificatif (`obligations::de_l_arret`,
/// `signalement_arret_rectificatif`), d'ou la trace de chaque avis dans `prolongations`.
///
/// CE QUI REFUSE, sans rien ecrire : un arret deja repris (« se prolonge sans interruption », ameli : un arret apres la
/// reprise est un nouvel arret) ; une fin qui n'est pas apres la fin prescrite (une fin plus tot est une reprise
/// anticipee, qui se signale : `arret-reprise`) ; une fin de subrogation sur un arret qui n'est pas subroge (CCH-11 de
/// S21.G00.60.006) ; ce que la norme refuse au signalement d'arret (`crate::routes::imprevus::verifier`) ; un chevauchement.
fn arret_prolonger(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let dernier = jour(exiger(charge, "dernier_jour")?)?;
    let Some((i, mut liste)) = trouver(ctx, ARRET, &cle, &dernier)? else {
        return Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}.", cle, dernier.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    if vrai(&avant["reprise"]) {
        return Err(refus(format!(
            "L'arrêt de {} a pris fin : le salarié a repris le {}. Une prolongation continue un arrêt en cours, sans \
interruption ; un arrêt qui commence après la reprise est un nouvel arrêt, qui se saisit et se signale comme tel. Rien \
n'est changé.",
            cle,
            jour(&avant["reprise"])?.jj_mm_aaaa()
        )));
    }
    let date_ou_rien = |k: &str| -> R<Option<Jour>> {
        match charge.get(k).filter(|v| vrai(v)) {
            Some(v) => Ok(Some(jour(v)?)),
            None => Ok(None),
        }
    };
    let (fin, sub_fin) = (date_ou_rien("fin")?, date_ou_rien("subrogation_fin")?);
    if fin.is_none() && sub_fin.is_none() {
        return Err(refus("Dire la nouvelle fin de l'arrêt (l'avis de prolongation), ou la fin de la subrogation : rien n'est changé."));
    }
    let fin_avant = jour(&avant["fin_previsionnelle"])?;
    let mut a = avant.clone();
    if let Some(f) = fin {
        if f < fin_avant {
            return Err(refus(format!(
                "Le {} est avant la fin prescrite de l'arrêt (le {}) : ce n'est pas une prolongation. Si le salarié a repris \
plus tôt, c'est une reprise anticipée, qui se saisit comme telle et se signale (nature 05). Rien n'est changé.",
                f.jj_mm_aaaa(),
                fin_avant.jj_mm_aaaa()
            )));
        }
        if f == fin_avant && sub_fin.is_none() {
            return Err(refus(format!("L'arrêt est déjà prescrit jusqu'au {} : rien n'est changé.", f.jj_mm_aaaa())));
        }
        a["fin_previsionnelle"] = json!(f.iso());
    }
    let subroge = signalement_subroge(&a)?;
    if let Some(sf) = sub_fin {
        if !subroge {
            return Err(refus(
                "Cet arrêt n'est pas subrogé : il n'a pas de fin de subrogation (S21.G00.60.006, interdite sans subrogation -- \
CCH-11). Rien n'est changé.",
            ));
        }
        a["subrogation_fin"] = json!(sf.iso());
    }
    crate::routes::imprevus::verifier(&a)?;
    sans_chevauchement(ctx, &a, Some(&avant))?;

    // La trace de l'avis : le jour ou la compagnie l'a su (le jour du moteur), l'heure de la saisie, et ce qui a change.
    let trace = json!({
        "le": ctx.aujourdhui.iso(),
        "saisi_le": crate::routes::maintenant(),
        "qui": crate::routes::qui_de(charge),
        "fin_avant": avant["fin_previsionnelle"], "fin": a["fin_previsionnelle"],
        "subrogation_fin_avant": avant["subrogation_fin"], "subrogation_fin": a["subrogation_fin"],
    });
    match a["prolongations"].as_array_mut() {
        Some(l) => l.push(trace),
        None => a["prolongations"] = json!([trace]),
    }
    liste[i] = a.clone();
    ecrire(ctx, &dernier.mois_iso(), liste)?;

    let nouvelle_fin = jour(&a["fin_previsionnelle"])?;
    let mut avertissements = Vec::new();
    if nouvelle_fin > fin_avant {
        avertissements.push(json!(format!(
            "Prolongé jusqu'au {}. Rien ne part : pas de nouveau signalement d'arrêt ; la DSN mensuelle de chaque mois que \
l'arrêt traverse porte la nouvelle fin prévisionnelle (S21.G00.60.003, fiche net-entreprises 1983).",
            nouvelle_fin.jj_mm_aaaa()
        )));
        // Les jours ajoutes qui tombent dans un mois dont la DSN reelle est deja partie : elle portait l'ancienne fin.
        let declares = reporte_par(&avant);
        for m in socle::date::mois_entre(&fin_avant.plus_jours(1).map_err(|e| erreur("ValueError", e))?, &nouvelle_fin) {
            if declares.iter().any(|x| valeur_en_texte(x) == m) {
                avertissements.push(json!(format!(
                    "La DSN de {} est déjà partie avec l'ancienne fin (le {}) : la paie de ce mois, où les jours ajoutés \
tombent, est à corriger.",
                    engagement::libelle_mois(&m),
                    fin_avant.jj_mm_aaaa()
                )));
            }
        }
    }
    let sub_change = sub_fin.is_some() && a["subrogation_fin"] != avant["subrogation_fin"];
    if sub_change && envoye(&avant, declarations::signalement::ARRET) {
        avertissements.push(json!(
            "Le signalement d'arrêt est déjà parti avec l'ancienne fin de subrogation : un signalement d'arrêt annule et \
remplace porte la nouvelle, le plus vite possible (fiche net-entreprises 1983). Il paraît dans ce qui est dû."
        ));
    }
    let (revoir, phrase) = subrogation_a_revoir(&a)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({
        "arret": resume(ctx, &n, &a)?,
        "subrogation_a_revoir": revoir,
        "phrase": phrase,
        "avertissements": avertissements,
    }))
}

/// L'arret est-il subroge (S21.G00.60.004 = oui) ?
fn signalement_subroge(a: &Value) -> R<bool> {
    declarations::signalement::avec_subrogation(&declarations::dsn::Arret { v: a.clone() }).map_err(|m| erreur("ValueError", m))
}

/// UNE SUBROGATION QUI S'ARRETE AVANT L'ARRET : `(true, la question)` quand l'arret est subroge et que sa fin de subrogation
/// (60.006) tombe avant sa fin previsionnelle. Le moteur ne connait pas le maximum que la convention permet (regle 1) : il
/// demande. Fiche net-entreprises 1983 (l. 25 et 110) : la fin de subrogation est la « date de fin de subrogation
/// maximale prevue dans le cadre de la convention collective ou de l'accord de branche » ; si elle « court toujours »,
/// il n'y a rien a faire.
fn subrogation_a_revoir(a: &Value) -> R<(bool, Value)> {
    if !signalement_subroge(a)? || !vrai(&a["subrogation_fin"]) {
        return Ok((false, Value::Null));
    }
    let (sf, fin) = (jour(&a["subrogation_fin"])?, jour(&a["fin_previsionnelle"])?);
    if sf >= fin {
        return Ok((false, Value::Null));
    }
    Ok((
        true,
        json!(format!(
            "La subrogation demandée s'arrête le {}, avant la nouvelle fin de l'arrêt (le {}). Elle se déclare jusqu'à son \
maximum prévu par la convention collective ou l'accord de branche : si ce maximum va plus loin, porter sa fin ici ; s'il est \
atteint, la garder (fiche net-entreprises 1983).",
            sf.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        )),
    ))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &Value, contrat_fin: &str) -> R<()> {
    imprevus::verifier_motif_de_suspension(&valeur_en_texte(&s["motif"]), &valeur_en_texte(&s["salarie"])).map_err(refus)?;
    let (debut, fin) = (jour(&s["debut"])?, jour(&s["fin"])?);
    if fin < debut {
        return Err(refus(format!(
            "Suspension du {} au {} : elle finit avant de commencer (S21.G00.65.002 et .003).",
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        )));
    }
    if !contrat_fin.is_empty() && fin.iso().as_str() > contrat_fin {
        return Err(refus(format!(
            "Suspension jusqu'au {}, mais le contrat {} finit le {} : une suspension ne survit pas a son contrat.",
            fin.jj_mm_aaaa(),
            valeur_en_texte(&s["contrat_numero"]),
            jour(&json!(contrat_fin))?.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// La fin du contrat d'une suspension deja ecrite, lue dans les saisies (vide : un contrat qui court).
fn fin_du_contrat(ctx: &Contexte, s: &Value) -> R<String> {
    let debut = valeur_en_texte(&s["contrat_debut"]);
    let contrats = crate::routes::imprevus::contrats_au(ctx, &s["salarie"], &debut)?;
    Ok(contrats.iter().find(|c| valeur_en_texte(&c["debut"]) == debut).map(|c| valeur_en_texte(&c["fin"])).unwrap_or_default())
}

fn note_de(charge: &Value) -> Value {
    let n = optionnel(charge, "note");
    if vrai(n) {
        n.clone()
    } else {
        json!("")
    }
}

/// `suspension-enregistrer` : `{salarie, debut, fin, motif, contrat?, note?}` -> `{"suspension": resume}`.
fn suspension_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let fin = jour(exiger(charge, "fin")?)?;
    let motif = valeur_en_texte(exiger(charge, "motif")?);
    let contrat = contrat_suspendu(ctx, &cle, &debut, optionnel(charge, "contrat"))?;
    let mut s = Map::new();
    s.insert("genre".into(), json!(SUSPENSION));
    s.insert("salarie".into(), json!(cle));
    s.insert("contrat_debut".into(), contrat["debut"].clone());
    s.insert("contrat_numero".into(), contrat["numero"].clone());
    s.insert("motif".into(), json!(motif));
    s.insert("debut".into(), json!(debut.iso()));
    s.insert("fin".into(), json!(fin.iso()));
    s.insert("saisi_le".into(), json!(crate::routes::maintenant()));
    s.insert("qui".into(), json!(crate::routes::qui_de(charge)));
    s.insert("note".into(), note_de(charge));
    s.insert("envois".into(), json!([]));
    let s = Value::Object(s);
    verifier_suspension(&s, &valeur_en_texte(&contrat["fin"]))?;
    if trouver(ctx, SUSPENSION, &cle, &debut)?.is_some() {
        return Err(refus(format!(
            "Une suspension de {} au {} est deja enregistree : la modifier plutot que d'en saisir une seconde.",
            cle,
            debut.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &s, None)?;
    let mois = debut.mois_iso();
    let mut liste = magasin(imprevus::du_fichier(&ctx.racine, &mois))?;
    liste.push(s.clone());
    ecrire(ctx, &mois, liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?}))
}

/// `suspension-modifier` : `{salarie, debut, fin?, motif?, note?}` -> `{"suspension": resume}`. Le debut designe la
/// suspension et ne change pas : une suspension qui commence un autre jour s'oublie et se saisit de nouveau.
fn suspension_modifier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let avant = liste[i].clone();
    let mut s = avant.clone();
    if let Some(f) = charge.get("fin") {
        s["fin"] = json!(jour(f)?.iso());
    }
    if let Some(m) = charge.get("motif") {
        s["motif"] = json!(valeur_en_texte(m));
    }
    if charge.get("note").is_some() {
        s["note"] = note_de(charge);
    }
    verifier_suspension(&s, &fin_du_contrat(ctx, &s)?)?;
    sans_chevauchement(ctx, &s, Some(&avant))?;
    liste[i] = s.clone();
    ecrire(ctx, &debut.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?}))
}

/// `suspension-oublier` : `{salarie, debut}` -> `{"suspension": resume}`. Une suspension qu'une DSN reelle a deja
/// declaree ne se retire pas : le magasin garderait moins que ce qui est parti.
fn suspension_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let cle = cle_du_salarie(ctx, exiger(charge, "salarie")?)?;
    let debut = jour(exiger(charge, "debut")?)?;
    let Some((i, mut liste)) = trouver(ctx, SUSPENSION, &cle, &debut)? else {
        return Err(refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())));
    };
    let envois = liste[i]["envois"].as_array().map_or(0, Vec::len);
    if envois > 0 {
        return Err(refus(format!(
            "Cette suspension a deja ete declaree ({} DSN). La retirer du magasin n'annule rien aupres des organismes : la \
reprendre (une fin plus tot), ou corriger les DSN deposees.",
            envois
        )));
    }
    let s = liste.remove(i);
    ecrire(ctx, &debut.mois_iso(), liste)?;
    let n = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?}))
}

/// `x or None`.
fn ou_rien(v: &Value) -> Value {
    if vrai(v) {
        v.clone()
    } else {
        Value::Null
    }
}

/// `jj_mm_aaaa(v)`.
fn jj_mm_aaaa(v: &Value) -> R<String> {
    Ok(jour(v)?.jj_mm_aaaa())
}

/// Ecrit le fichier d'un mois du magasin des imprevus (`imprevus/AAAA-MM.json`) : les arrets ET les suspensions qu'il
/// range, tels que la liste les donne.
pub(crate) fn ecrire_arrets(ctx: &Contexte, mois: &str, imprevus: Vec<Value>) -> R<()> {
    crate::routes::imprevus::ecrire(ctx, mois, imprevus)
}

/// Ce que la norme refuse (`SignalementImpossible`, une
/// `ValueError`) ; les avertissements ne servent pas ici.
pub(crate) fn verifier(arret: &Value) -> R<()> {
    match signalement::verifier(&declarations::dsn::Arret { v: arret.clone() }, ARRET, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(erreur("ValueError", m)),
        Err(signalement::Echec::Autre(m)) => Err(Echec { classe: None, message: m }),
    }
}

/// `ctx.salarie(salarie)["_cle"]`.
fn cle_en_json(ctx: &Contexte, nom: &Value) -> R<Value> {
    salarie(ctx, nom)?.get("_cle").cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'arret de ce salarie a ce dernier jour travaille : (son indice, le fichier de son mois, tous imprevus compris).
pub(crate) fn trouver_l_arret(ctx: &Contexte, nom: &Value, dernier_jour: &Value) -> R<(usize, Vec<Value>)> {
    let cle = cle_en_json(ctx, nom)?;
    if vrai(dernier_jour) {
        if let Some(t) = crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &jour(dernier_jour)?)? {
            return Ok(t);
        }
    }
    let quand = if vrai(dernier_jour) { jj_mm_aaaa(dernier_jour)? } else { "?".to_string() };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. `moteur arret` les liste.",
        valeur_en_texte(&cle),
        quand
    )))
}

/// `{"arret": resume}` : l'arret tel que la page le lit (`crate::routes::imprevus::resume`, avec les cachets qui tombent dedans).
fn reponse_arret(ctx: &Contexte, arret: &Value) -> R<Value> {
    let n = nomenclatures(ctx)?;
    Ok(json!({"arret": crate::routes::imprevus::resume(ctx, &n, arret)?}))
}

/// `arret-enregistrer` : `{salarie, dernier_jour, fin, motif, subrogation?, subrogation_debut?, subrogation_fin?,
/// date_accident?, jours_deja_indemnises?, contrat?, note?}` -> `{"arret": resume}`. Les controles de la norme sont ceux
/// du signalement (`signalement::verifier`, nature 04), avant toute ecriture ; un arret ne chevauche pas un autre imprevu
/// du meme contrat (`crate::routes::imprevus::sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let fin = exiger(charge, "fin")?;
    let motif = exiger(charge, "motif")?;
    let cle = cle_en_json(ctx, nom)?;
    let dernier = jour(dernier_jour)?;
    let choisi = crate::routes::imprevus::contrat_suspendu(ctx, &valeur_en_texte(&cle), &dernier, &ou_rien(optionnel(charge, "contrat")))?;

    let jour_ou_rien = |k: &str| -> R<Value> {
        let v = optionnel(charge, k);
        if vrai(v) {
            Ok(json!(jour(v)?.iso()))
        } else {
            Ok(Value::Null)
        }
    };
    let note = {
        let n = optionnel(charge, "note");
        if vrai(n) {
            n.clone()
        } else {
            json!("")
        }
    };
    // Les jours deja indemnises (D1226-4) ne se disent que pour un arret qu'un maintien de salaire peut suivre : un contrat
    // paye au mois, et un motif de maladie ou d'accident (L1226-1 ; `maintien_legal` dans arrets-base.json).
    let deja = crate::routes::imprevus::jours_ou_rien(optionnel(charge, "jours_deja_indemnises"), "Jours deja indemnises")?;
    if !deja.is_null() {
        let m = engagement::imprevus::motif_d_arret(&valeur_en_texte(motif)).map_err(crate::routes::imprevus::refus_magasin)?.unwrap_or(Value::Null);
        if choisi["mensuel"] != json!(true) || !m.get("maintien_legal").is_some_and(vrai) {
            return Err(refus(
                "Les jours deja indemnises (D1226-4) ne servent qu'au maintien de salaire d'un permanent arrete pour maladie \
ou accident (L1226-1) : cet arret n'en appelle pas. Rien n'est ecrit.",
            ));
        }
    }
    let mut a = Map::new();
    a.insert("genre".into(), json!(engagement::imprevus::ARRET));
    a.insert("salarie".into(), cle.clone());
    a.insert("contrat_debut".into(), choisi["debut"].clone());
    a.insert("contrat_numero".into(), choisi["numero"].clone());
    a.insert("motif".into(), json!(valeur_en_texte(motif)));
    a.insert("dernier_jour_travaille".into(), json!(dernier.iso()));
    a.insert("fin_previsionnelle".into(), json!(jour(fin)?.iso()));
    a.insert("subrogation".into(), optionnel(charge, "subrogation").clone());
    a.insert("subrogation_debut".into(), jour_ou_rien("subrogation_debut")?);
    a.insert("subrogation_fin".into(), jour_ou_rien("subrogation_fin")?);
    a.insert("reprise".into(), Value::Null);
    a.insert("reprise_motif".into(), Value::Null);
    a.insert("date_accident".into(), jour_ou_rien("date_accident")?);
    a.insert("jours_deja_indemnises".into(), deja);
    a.insert("maintien".into(), json!({}));
    a.insert("prolongations".into(), json!([]));
    a.insert("saisi_le".into(), json!(maintenant()));
    a.insert("qui".into(), json!(qui_de(charge)));
    a.insert("note".into(), note);
    a.insert("envois".into(), json!([]));
    let arret = Value::Object(a);
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;

    let mois = dernier.mois_iso();
    if crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &valeur_en_texte(&cle), &dernier)?.is_some() {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            valeur_en_texte(&cle),
            dernier.jj_mm_aaaa()
        )));
    }
    crate::routes::imprevus::sans_chevauchement(ctx, &arret, None)?;
    let mut liste = engagement::imprevus::du_fichier(&ctx.racine, &mois).map_err(crate::routes::imprevus::refus_magasin)?;
    liste.push(arret.clone());
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `arret-reprise` : `{salarie, dernier_jour, reprise, motif_reprise?}` -> `{"arret": resume}` ; une reprise vide l'efface.
fn arret_reprise(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let reprise = ou_rien(exiger(charge, "reprise")?);
    let motif = ou_rien(optionnel(charge, "motif_reprise"));
    let (i, mut liste) = trouver_l_arret(ctx, nom, dernier_jour)?;
    if let Value::Object(a) = &mut liste[i] {
        a.insert("reprise".into(), if vrai(&reprise) { json!(jour(&reprise)?.iso()) } else { reprise });
        a.insert("reprise_motif".into(), motif);
    }
    let arret = liste[i].clone();
    verifier(&arret)?;
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `arret-oublier` : `{salarie, dernier_jour}` -> `{"arret": resume}`. Un arret deja declare ne se retire pas.
fn arret_oublier(ctx: &Contexte, charge: &Value) -> R<Value> {
    let nom = exiger(charge, "salarie")?;
    let dernier_jour = exiger(charge, "dernier_jour")?;
    let (i, mut liste) = trouver_l_arret(ctx, nom, dernier_jour)?;
    let envois = match &liste[i]["envois"] {
        Value::Array(e) => e.len(),
        _ => 0,
    };
    if envois > 0 {
        return Err(refus(format!(
            "Cet arret a deja ete declare ({} envoi(s)). Le retirer du magasin n'annule rien aupres des organismes et ferait disparaitre la trace de ce qui est parti : emettre un annule-et-remplace.",
            envois
        )));
    }
    let arret = liste.remove(i);
    let mois: String = valeur_en_texte(&arret["dernier_jour_travaille"]).chars().take(7).collect();
    ecrire_arrets(ctx, &mois, liste)?;
    reponse_arret(ctx, &arret)
}

/// `charge.get(cle)` quand il est vrai (`or None`).
fn facultatif<'a>(charge: &'a Value, cle: &str) -> Option<&'a Value> {
    charge.get(cle).filter(|v| vrai(v))
}

/// `charge["le"]` : un jour de constitution, quand la ligne de commande en donne un (`--le AAAA-MM-JJ`). Absent : le jour du
/// contexte, comme la fenetre.
fn jour_le(charge: &Value) -> R<Option<Jour>> {
    match facultatif(charge, "le") {
        None => Ok(None),
        Some(v) => Jour::lire(&en_texte(v)).map(Some).map_err(|m| erreur("ValueError", m)),
    }
}

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn saisie_du_mois(ctx: &Contexte, mois: &str) -> R<(Value, Vec<Paie>)> {
    engagement::etats::validees_du_mois(ctx, mois).map_err(plain)?.ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
}

/// Le mois en toutes lettres (« mai 2026 ») pour une phrase ; tel quel s'il n'a pas la forme AAAA-MM.
fn mois_dit(mois: &str) -> String {
    match mois.get(..4) {
        Some(_) if mois.len() == 7 => engagement::libelle_mois(mois),
        _ => mois.to_string(),
    }
}

/// La cle de la fiche que `nom` designe, comme partout dans le moteur (`Contexte::salarie` : la casse, les accents, le
/// prenom, le NIR ne comptent pas). Un nom qui ne designe personne se refuse avec la phrase de `Contexte::salarie`.
fn cle_du_nom(ctx: &Contexte, nom: &str) -> R<String> {
    let fiche = crate::routes::salarie(ctx, &json!(nom))?;
    fiche.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, et son rang dans la saisie. Un engagement se rattache a
/// sa fiche comme sa paie (`engagement::cle_de_l_engagement`), quelle que soit la casse du nom saisi. `nom` : comme on
/// l'a dit, pour la phrase.
/// Un salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit<'a>(ctx: &Contexte, saisie: &'a Value, cle: &str, nom: &str, mois: &str) -> R<(usize, &'a Value)> {
    let eng = saisie.get("engagements").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let siens: Vec<(usize, &Value)> =
        eng.iter().enumerate().filter(|(_, x)| engagement::cle_de_l_engagement(ctx, x).as_deref() == Some(cle)).collect();
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucun engagement en {} : aucun contrat ne s'y termine.", nom, mois_dit(mois))));
    }
    siens.into_iter().find(|(_, x)| x.get("fin").map_or(false, vrai)).ok_or_else(|| {
        refus(format!("{} n'a aucun contrat qui se termine en {} (la saisie ne dit pas de `fin`).", nom, mois_dit(mois)))
    })
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `saisie_du_mois`, au meme rang que sa saisie),
/// numerotee (les numeros ne sont que reserves).
fn paie_de_l_engagement(ctx: &Contexte, paies: &[Paie], rang: usize, cle: &str, nom: &str, mois: &str) -> R<Paie> {
    let annee = mois.chars().take(4).collect::<String>().parse::<i32>().map_err(|_| erreur("ValueError", format!("mois illisible : {}", repr_texte(mois))))?;
    let mut paies = paies.to_vec();
    documents::ecrire_documents::numeroter(ctx, &mut paies, annee).map_err(en_echec)?;
    paies
        .into_iter()
        .nth(rang)
        .filter(|p| p.salarie.get("_cle").map(valeur_en_texte).as_deref() == Some(cle))
        .ok_or_else(|| refus(format!("{} n'a aucune paie validee en {} pour ce contrat.", nom, mois_dit(mois))))
}

/// Les mentions communes du certificat de travail et du recu (`documents_fin._marqueurs_communs`).
fn marqueurs_communs(date: &Jour, p: &Paie, ent: &Value) -> R<Map<String, Value>> {
    let s = &p.salarie;
    let e = &p.engagement;
    let req = |v: &Value, k: &str| -> R<String> { v.get(k).map(en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k))) };
    let ou_vide = |v: &Value, k: &str| -> String { v.get(k).map(en_texte).unwrap_or_default() };
    let sig = ent.get("signataire").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("signataire")))?;
    let aujourdhui = date.jj_mm_aaaa();
    let mut m = Map::new();
    let mut poser = |k: &str, v: String| {
        m.insert(k.to_string(), Value::String(v));
    };
    poser("employeur.raison_sociale", req(ent, "raison_sociale")?);
    poser("employeur.adresse", req(ent, "adresse")?);
    poser("employeur.code_postal", req(ent, "code_postal")?);
    poser("employeur.commune", req(ent, "commune")?);
    poser("employeur.siret", req(ent, "siret")?);
    poser("employeur.naf", req(ent, "naf")?);
    poser("employeur.signataire", format!("{} {}", req(sig, "prenom")?, req(sig, "nom")?));
    poser("employeur.signataire_fonction", ou_vide(sig, "fonction"));
    poser("salarie.nom_complet", format!("{} {}", req(s, "prenom")?, req(s, "nom")?));
    poser(
        "salarie.date_naissance",
        documents::bulletin::jj_mm_aaaa(s.get("date_naissance").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("date_naissance")))?).map_err(|m| erreur("ValueError", m))?,
    );
    poser("salarie.lieu_naissance", ou_vide(s, "lieu_naissance"));
    poser("salarie.adresse", req(s, "adresse")?);
    poser("salarie.code_postal", req(s, "code_postal")?);
    poser("salarie.commune", req(s, "commune")?);
    poser("contrat.numero", e.numero.clone().unwrap_or_default());
    poser("contrat.debut", e.debut.jj_mm_aaaa());
    poser("contrat.fin", e.fin.map(|f| f.jj_mm_aaaa()).unwrap_or_default());
    poser("contrat.emploi", ou_vide(s, "emploi"));
    poser("contrat.nature", e.nat().map_err(plain)?.libelle.clone());
    poser("certificat.date", aujourdhui.clone());
    poser("recu.date", aujourdhui);
    Ok(m)
}

/// Le detail des sommes, ligne a ligne -- pas un total.
fn inventaire(p: &Paie) -> String {
    let mut lignes = Vec::new();
    for l in &p.remuneration {
        lignes.push(format!("{} : {}", l.libelle, documents::bulletin::fmt_eur(eur2(l.montant))));
    }
    for l in &p.lignes {
        let m = eur2(l.montant_sal);
        if m != socle::num::Decimal::ZERO {
            lignes.push(format!("{} : -{}", l.libelle, documents::bulletin::fmt_eur(m)));
        }
    }
    lignes.join("\n")
}

/// Le papier de fin de contrat de `modele`, par la fabrique (style
/// « attestation »). `signe` : le certificat s'etablit SIGNE de l'employeur.
fn ecrire_papier(modele: &Path, valeurs: Map<String, Value>, chemin: &Path, ent: &Value, signe: bool) -> R<()> {
    let texte = documents::contrat::lire_modele_texte(modele).map_err(en_echec)?;
    let mut d = documents::contrat::enveloppe("attestation", ent);
    let mut un = Map::new();
    un.insert("texte".into(), json!(texte));
    un.insert("valeurs".into(), Value::Object(valeurs));
    if signe {
        if let Some(s) = documents::bulletin::identite_de(ent, "signature") {
            un.insert("signature_employeur".into(), json!(documents::conges_spectacles::chemin_texte(&s)));
        }
    }
    d.insert("documents".into(), json!([{ "modele": Value::Object(un) }]));
    documents::fabrique::ecrire(&Value::Object(d), chemin).map_err(|m| erreur("FabriqueRefus", m.replace('\n', " ").trim().to_string()))?;
    Ok(())
}

/// Le certificat de travail et le recu pour solde de
/// tout compte. Deux PAPIERS : rien ne part nulle part.
fn fin_contrat_documents(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?);
    let (saisie, paies) = saisie_du_mois(ctx, mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let p = paie_de_l_engagement(ctx, &paies, rang, &cle, salarie, mois)?;
    let ent = ctx.entreprise_et_gabarits();
    let sortie = ctx.racine.join("sorties").join(mois);
    let date: Jour = jour_le(charge)?.unwrap_or(ctx.aujourdhui);
    let fin_jj = en_texte(&e["fin"]).replace('-', "");
    let poste = ctx.gabarits();

    let numero = p.engagement.numero.clone().unwrap_or_else(|| "?".to_string());
    if p.engagement.fin.is_none() {
        return Err(erreur(
            "ValueError",
            format!(
                "Certificat de travail : le contrat {} n'a pas de date de fin. Il se delivre A L'EXPIRATION du contrat (L1234-19).",
                numero
            ),
        ));
    }
    let nom_cert = format!("CertificatTravail_{}_{}.pdf", cle, fin_jj);
    let modele = socle::fichiers::modele_de("certificat-travail.modele", &ent, &poste);
    ecrire_papier(&modele, marqueurs_communs(&date, &p, &ent)?, &sortie.join(&nom_cert), &ent, true)?;

    let nom_recu = format!("RecuSoldeToutCompte_{}_{}.pdf", cle, fin_jj);
    let mut valeurs = marqueurs_communs(&date, &p, &ent)?;
    valeurs.insert("solde.inventaire".into(), json!(inventaire(&p)));
    valeurs.insert("solde.net".into(), json!(documents::bulletin::fmt_eur(eur2(p.net_a_payer))));
    let modele = socle::fichiers::modele_de("recu-solde-de-tout-compte.modele", &ent, &poste);
    ecrire_papier(&modele, valeurs, &sortie.join(&nom_recu), &ent, false)?;

    Ok(json!({
        "certificat": sortie.join(&nom_cert).to_string_lossy(),
        "recu": sortie.join(&nom_recu).to_string_lossy(),
        "noms": [nom_cert, nom_recu],
        "avertissements": [
            "Le recu pour solde de tout compte s'etablit en DOUBLE EXEMPLAIRE (D1234-7) : un pour le salarie, un pour la compagnie. Il se signe de sa main, avec la mention << pour solde de tout compte >>.",
            "Il peut etre denonce par lettre recommandee dans les SIX MOIS (L1234-20 et D1234-8) ; passe ce delai il est liberatoire pour les seules sommes qui y sont inscrites.",
            "Le certificat de travail ne contient QUE les mentions de D1234-6 : dates d'entree et de sortie, emplois occupes et leurs periodes. Y ajouter autre chose, c'est sortir de ce que la loi autorise."
        ],
    }))
}

/// `fctu.FinDeContrat` (en dictionnaire) que le signalement declarera, batie sur la saisie
/// (`fin-contrat-signaler`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Value, mois: &str) -> R<declarations::fctu::FinDeContrat> {
    let manque = crate::routes::imprevus::ce_qui_manque(ctx, e)?;
    if !manque.is_empty() {
        let numero = e.get("numero").filter(|n| vrai(n)).map(en_texte).unwrap_or_else(|| "?".to_string());
        let liste: Vec<String> = manque.iter().map(valeur_en_texte).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne peut pas etre produit, la saisie ne dit pas tout.\n  {}",
            numero,
            liste.join("\n  ")
        )));
    }
    let preavis = e.get("preavis").filter(|p| vrai(p)).cloned().unwrap_or_else(|| json!({}));
    let cle = |k: &str| e.get(k).cloned().unwrap_or(Value::Null);
    let fin = e.get("fin").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("fin")))?.clone();
    let motif = e.get("motif_rupture").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("motif_rupture")))?.clone();
    let type_preavis = preavis.get("type").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("type")))?.clone();
    let v = json!({
        "fin": fin, "motif": motif, "mois_rattachement": mois,
        "type_preavis": type_preavis,
        "preavis_debut": preavis.get("debut").cloned().unwrap_or(Value::Null),
        "preavis_fin": preavis.get("fin").cloned().unwrap_or(Value::Null),
        "notification": cle("notification_rupture"),
        "signature_convention": cle("signature_convention"),
        "engagement_licenciement": cle("engagement_licenciement"),
        "transaction": cle("transaction"),
    });
    declarations::fctu::FinDeContrat::lire(&v).map_err(plain)
}

/// L'echec d'un signalement de fin de contrat ou d'amorcage : une regle de la norme qui s'oppose
/// (`FctuImpossible`, `AmorcageImpossible`, des `ValueError`) ou autre chose.
fn echec_fctu(e: declarations::fctu::Echec) -> Echec {
    match e {
        declarations::fctu::Echec::Impossible(m) | declarations::fctu::Echec::Autre(m) => erreur("ValueError", m),
    }
}

/// Ecrit le signalement dans `sorties/AAAA-MM/`. Rien n'est
/// depose : le depot est un acte a part.
fn fin_contrat_signaler(ctx: &Contexte, charge: &Value) -> R<Value> {
    let (mois, salarie) = (texte_requis(charge, "mois")?, texte_requis(charge, "salarie")?);
    let remplace: Option<String> = facultatif(charge, "remplace").map(en_texte);
    // LE NUMERO D'ORDRE : celui qu'on donne, sinon (0) le suivant de tous les signalements deja partis (p. 136), lu une
    // fois la saisie verifiee : la plate-forme ne se consulte pas pour un refus.
    let ordre: i64 = match facultatif(charge, "ordre") {
        None => 0,
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(1),
        Some(v) => {
            let t = en_texte(v);
            t.trim().parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(&t))))?
        }
    };
    let envoi_test = facultatif(charge, "test").is_some();

    let (saisie, paies) = saisie_du_mois(ctx, mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(ctx, &saisie, &cle, salarie, mois)?;
    let fin = fin_de_contrat_declaree(ctx, e, mois)?;
    let paie = paie_de_l_engagement(ctx, &paies, rang, &cle, salarie, mois)?;
    let type_declaration = if remplace.is_some() { declarations::fctu::ANNULE_ET_REMPLACE } else { declarations::fctu::NORMALE };
    let mut avertissements = declarations::fctu::verifier(&fin, &paie.engagement, type_declaration).map_err(echec_fctu)?;
    let (ordre, plate_forme) = if ordre != 0 { (ordre, Vec::new()) } else { prochain_ordre(ctx)? };
    avertissements.extend(plate_forme);
    let nom = format!("SignalementFinContrat_{}_{}.dsn", cle, fin.fin.replace('-', ""));
    let cible = ctx.racine.join("sorties").join(mois).join(&nom);
    let options = declarations::fctu::Options {
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace: remplace.clone(),
        envoi_test,
        date_constitution: jour_le(charge)?,
    };
    // Les arrets du contrat qui croisent la periode du signalement : le FCTU les reporte (bloc 60), comme la mensuelle,
    // tires du magasin des imprevus ; ses suspensions (bloc 65) en viennent deja (`imprevus::suspensions_du_mois`).
    let (debut_p, fin_p) = (paie.engagement.debut_periode().map_err(plain)?, paie.engagement.fin_periode().map_err(plain)?);
    let arrets: Vec<declarations::dsn::Arret> = engagement::imprevus::arrets_du_contrat(ctx, &paie, &debut_p, &fin_p)
        .map_err(crate::routes::imprevus::refus_magasin)?
        .into_iter()
        .map(|v| declarations::dsn::Arret { v })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "nom": nom,
        "type": type_declaration,
        "ordre": ordre,
        "preavis": declarations::fctu::libelle_preavis(&fin.type_preavis).map_err(plain)?,
        "avertissements": avertissements,
    }))
}

/// L'arret de ce salarie dont c'est le dernier jour travaille, dans le magasin des imprevus.
fn trouver_arret(ctx: &Contexte, salarie: &str, dernier_jour: &Value) -> R<Value> {
    let cle = ctx.salarie(salarie).map_err(|m| erreur("KeyError", m))?;
    let cle = cle.get("_cle").map(valeur_en_texte).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?;
    if vrai(dernier_jour) {
        let jour = crate::routes::imprevus::jour(dernier_jour)?;
        if let Some((i, liste)) = crate::routes::imprevus::trouver(ctx, engagement::imprevus::ARRET, &cle, &jour)? {
            return Ok(liste[i].clone());
        }
    }
    let lisible = if vrai(dernier_jour) {
        documents::bulletin::jj_mm_aaaa(dernier_jour).map_err(|m| erreur("ValueError", m))?
    } else {
        "?".to_string()
    };
    Err(refus(format!(
        "Aucun arret de {} dont le dernier jour travaille soit le {}. `moteur arret` les liste.",
        cle, lisible
    )))
}

/// Le numero d'ordre (`S20.G00.05.004`) du prochain signalement : un de plus que le plus grand de tous ceux que
/// la compagnie a deja envoyes, arrets, reprises, fins de contrat et amorcages confondus. Cahier technique
/// DSN, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit unique quel que
/// soit le mois d'emission » (et non plus le rang parmi les envois de CET arret, qui repartait a 1). LA PLATE-FORME
/// FAIT FOI : `crate::compagnie::releve::dernier_ordre_connu` la consulte ; ce qu'il faut en dire (elle n'a pas
/// repondu, elle connaissait plus que le journal) part avec les avertissements.
pub(crate) fn prochain_ordre(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let connu = crate::compagnie::releve::dernier_ordre_connu(ctx, None)?;
    Ok((connu.dernier + 1, connu.avertissements))
}

/// Ecrit le fichier du signalement dans `sorties/AAAA-MM/`. Rien
/// n'est depose : le depot est un acte a part.
fn arret_signaler(ctx: &Contexte, charge: &Value) -> R<Value> {
    use declarations::signalement::{self, Echec as Ech};
    let salarie = en_texte(crate::routes::fiches::exiger(charge, "salarie")?);
    let dernier_jour = crate::routes::fiches::exiger(charge, "dernier_jour")?;
    let nature = facultatif(charge, "nature").map(en_texte).unwrap_or_else(|| signalement::ARRET.to_string());
    let remplace: Option<String> = facultatif(charge, "remplace").map(en_texte);
    let envoi_test = facultatif(charge, "test").is_some();

    let arret = trouver_arret(ctx, &salarie, dernier_jour)?;
    let type_declaration = if remplace.is_some() { signalement::ANNULE_ET_REMPLACE } else { signalement::NORMALE };
    // `ordre` (`--ordre N`) : le rang voulu ; sinon le suivant. `le` (`--le`) : la date de constitution ; sinon le jour du contexte.
    let ordre = match facultatif(charge, "ordre") {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(v) => {
            let t = en_texte(v);
            t.trim().parse().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr_texte(&t))))?
        }
        None => 0,
    };
    let constitution: Jour = jour_le(charge)?.unwrap_or(ctx.aujourdhui);
    let dsn_arret = declarations::dsn::Arret { v: arret.clone() };
    let echec = |e: Ech| match e {
        Ech::Impossible(m) | Ech::Autre(m) => erreur("ValueError", m),
    };
    let mut avertissements = signalement::verifier(&dsn_arret, &nature, Some(&constitution)).map_err(echec)?;
    let (ordre, plate_forme) = if ordre != 0 { (ordre, Vec::new()) } else { prochain_ordre(ctx)? };
    avertissements.extend(plate_forme);

    let jour = en_texte(&arret["dernier_jour_travaille"]);
    let mois: String = jour.chars().take(7).collect();
    let nom = format!(
        "Signalement{}_{}_{}.dsn",
        if nature == signalement::ARRET { "Arret" } else { "Reprise" },
        en_texte(&arret["salarie"]),
        jour.replace('-', "")
    );
    let cible = ctx.racine.join("sorties").join(&mois).join(&nom);
    let options = signalement::Options {
        nature: nature.clone(),
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace,
        envoi_test,
        date_constitution: Some(constitution),
    };
    let texte = signalement::generer(ctx, &dsn_arret, &options).map_err(echec)?;
    declarations::dsn::ecrire(&texte, &cible).map_err(plain)?;
    crate::compagnie::depart::noter_piece_ecrite(&ctx.racine, &crate::compagnie::depart::cle_de_piece(&cible), None)?;
    let nomenclatures = crate::routes::imprevus::nomenclatures(ctx)?;
    Ok(json!({
        "fichier": cible.to_string_lossy(),
        "nom": nom,
        "nature": nature,
        "type": type_declaration,
        "ordre": ordre,
        "avertissements": avertissements,
        "arret": crate::routes::imprevus::resume_arret(&nomenclatures, &arret)?,
    }))
}

#[track_caller]
fn plain(m: String) -> Echec {
    socle::echec::noter_et(None, m)
}

fn en_echec(r: Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

/// `charge["cle"]` : un texte requis.
fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

/// `int(texte)` pour un morceau de nom de mois.
fn entier(t: &str) -> R<i64> {
    t.trim().parse::<i64>().map_err(|_| erreur("ValueError", format!("nombre entier illisible : {}", repr(t))))
}

/// `dict.get(cle)` d'une dataclass construite : `None` pour un champ absent.
fn champ(v: &Value, k: &str, defaut: Value) -> Value {
    v.get(k).cloned().unwrap_or(defaut)
}

// -------------------------------------------------------------------- arrets

/// Un refus du magasin des imprevus (`imprevus`) : un fichier abime, un genre inconnu, le dossier `arrets/` d'avant.
#[track_caller]
pub(crate) fn refus_magasin(m: String) -> Echec {
    erreur("ErreurOperation", m)
}

/// Les arrets de travail du magasin des imprevus (`imprevus/AAAA-MM.json`, genre `arret`), dans l'ordre des mois du
/// magasin, chacun complet.
pub(crate) fn tous_les_arrets(ctx: &Contexte) -> R<Vec<Value>> {
    Ok(engagement::imprevus::tous(&ctx.racine).map_err(refus_magasin)?.into_iter().filter(engagement::imprevus::est_un_arret).collect())
}

/// `signalement.nomenclatures()`.
pub(crate) fn nomenclatures(ctx: &Contexte) -> R<std::sync::Arc<Value>> {
    let _ = ctx;
    json_en_cache(&dossier_du_code().join("arrets-base.json")).map_err(plain)
}

/// Les codes de `nomenclatures()[quoi]["codes"]`.
fn codes(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let section = n.get(quoi).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(quoi)))?;
    let liste = section.get("codes").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("codes")))?;
    Ok(items(liste)?.into_iter().map(|c| c.into_owned()).collect())
}

/// `signalement.motifs_arret()` et `motifs_reprise()` : les codes saisissables.
pub(crate) fn saisissables(n: &Value, quoi: &str) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for c in codes(n, quoi)? {
        if vrai(get(&c, "saisissable")?) {
            out.push(c);
        }
    }
    Ok(out)
}

/// `Arret.libelle_motif` : `_codes("motifs_arret").get(motif, {}).get("libelle", "?")`.
fn libelle_motif(n: &Value, motif: &Value) -> R<Value> {
    if matches!(motif, Value::Array(_) | Value::Object(_)) {
        return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(motif))));
    }
    let Some(m) = motif.as_str() else { return Ok(json!("?")) };
    let mut trouve: Option<Value> = None;
    for c in codes(n, "motifs_arret")? {
        if get(&c, "code")?.as_str() == Some(m) {
            trouve = Some(c);
        }
    }
    match trouve {
        Some(c) => Ok(getd(&c, "libelle", &json!("?"))?.clone()),
        None => Ok(json!("?")),
    }
}

/// Les envois d'un arret (`arret.envois`), a parcourir.
fn envois(a: &Value) -> R<Vec<std::borrow::Cow<'_, Value>>> {
    match a.get("envois") {
        None => Ok(Vec::new()),
        Some(v) => items(v),
    }
}

/// Un arret resume pour la liste des imprevus.
pub(crate) fn resume_arret(n: &Value, a: &Value) -> R<Value> {
    let mut signalements = Vec::new();
    for e in envois(a)? {
        if matches!(get(&e, "nature")?.as_str(), Some("04") | Some("05")) {
            signalements.push(e.into_owned());
        }
    }
    let mut reports = Vec::new();
    for e in envois(a)? {
        if get(&e, "nature")?.as_str() == Some("01") {
            reports.push(e.into_owned());
        }
    }
    let motif = champ(a, "motif", Value::Null);
    let libelle = libelle_motif(n, &motif)?;
    let mut mois = Vec::new();
    for r in &reports {
        mois.push(get(r, "mois")?.clone());
    }
    let signale = !signalements.is_empty();
    // UN SIGNALEMENT EST-IL DU ? Pas pour un motif que la DSN ne couvre pas (le deuil d'enfant : `obligations-base.json`,
    // `signalement_arret.sauf_motifs`) ; son obligation est alors l'attestation de salaire hors DSN.
    let signalement_du = crate::obligations::signalement_du(&socle::date::en_texte(&motif))?;
    Ok(json!({
        "salarie": champ(a, "salarie", Value::Null),
        "contrat": champ(a, "contrat_numero", Value::Null),
        "contrat_debut": champ(a, "contrat_debut", Value::Null),
        "motif": motif,
        "motif_libelle": libelle,
        "dernier_jour_travaille": champ(a, "dernier_jour_travaille", Value::Null),
        "fin_previsionnelle": champ(a, "fin_previsionnelle", Value::Null),
        "subrogation": champ(a, "subrogation", Value::Null),
        "subrogation_debut": champ(a, "subrogation_debut", Value::Null),
        "subrogation_fin": champ(a, "subrogation_fin", Value::Null),
        "date_accident": champ(a, "date_accident", Value::Null),
        "reprise": champ(a, "reprise", Value::Null),
        "reprise_motif": champ(a, "reprise_motif", Value::Null),
        "note": champ(a, "note", json!("")),
        "saisi_le": champ(a, "saisi_le", Value::Null),
        "signale": signale,
        "signalement_du": signalement_du,
        "signalements": signalements,
        "reporte_par": mois,
        "etat": if signale { "signale" } else if signalement_du { "a signaler" } else { "sans signalement" },
    }))
}

/// La route `arrets` : les arrets de travail du magasin des imprevus, chacun resume comme `imprevus` le resume
/// (`crate::routes::imprevus::resume`), ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<Value> {
    let tous = tous_les_arrets(ctx)?;
    let n = nomenclatures(ctx)?;
    // LES OBLIGATIONS de chaque arret (`obligations::de_l_arret`, 15/10/2026) : un champ de plus a son resume.
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut a_signaler = Vec::new();
    for a in &tous {
        let mut r = crate::routes::imprevus::resume(ctx, &n, a)?;
        r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?);
        if !r["signale"].as_bool().unwrap_or(false) && r["signalement_du"].as_bool().unwrap_or(true) {
            a_signaler.push(r.clone());
        }
        resumes.push(r);
    }
    let subrogation = n.get("subrogation").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("subrogation")))?;
    Ok(json!({
        "arrets": resumes,
        "a_signaler": a_signaler,
        "motifs": saisissables(&n, "motifs_arret")?,
        "motifs_reprise": saisissables(&n, "motifs_reprise")?,
        "subrogation": subrogation.get("codes").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("codes")))?,
    }))
}

// ----------------------------------------------------------- fins de contrat

/// `calendar.monthrange(an, mo)[1]` pour `_fin_mois`.
fn fin_du_mois(mois: &str) -> R<Jour> {
    let an = entier(&mois.chars().take(4).collect::<String>())?;
    let mo = entier(&mois.chars().skip(5).take(2).collect::<String>())?;
    if !(1..=12).contains(&mo) {
        return Err(erreur("ValueError", socle::date::mois_impossible(mo)));
    }
    if !(1..=9999).contains(&an) {
        return Err(erreur("ValueError", socle::date::annee_impossible(an)));
    }
    let dernier = socle::date::jours_du_mois(an as i32, mo as u32);
    Jour::nouveau(an as i32, mo as u32, dernier).ok_or_else(|| erreur("ValueError", socle::date::jour_impossible(an as i64, mo as i64, dernier)))
}

/// `KeyError` quand elle est inconnue.
fn nature_de(cle: &Value) -> R<std::sync::Arc<droit::natures::Nature>> {
    let texte = match cle {
        Value::String(s) => Some(s.clone()),
        v if !vrai(v) => None,
        Value::Array(_) | Value::Object(_) => {
            return Err(erreur("TypeError", socle::fichiers::pas_une_cle(nom_type(cle))));
        }
        autre => {
            // Une cle qui n'est pas un texte ne peut etre aucune nature.
            let mut connues: Vec<String> = droit::natures::natures().map_err(plain)?.iter().map(|n| n.cle.clone()).collect();
            connues.sort();
            return Err(erreur("KeyError", format!("Nature de contrat inconnue : {}. Connues : {}.", valeur_ecrite(autre), connues.join(", "))));
        }
    };
    droit::natures::nature(texte.as_deref()).map_err(|m| erreur("KeyError", m))
}

/// `_ce_qui_manque_au_signalement` : ce qu'un signalement exige et que la
/// saisie ne dit pas encore.
pub(crate) fn ce_qui_manque(ctx: &Contexte, e: &Value) -> R<Vec<Value>> {
    let mut manque = Vec::new();
    let motif = get(e, "motif_rupture")?;
    if !vrai(motif) {
        manque.push(json!(
            "`motif_rupture` : le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par defaut."
        ));
    }
    // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`fctu::notification_exigee`).
    if vrai(motif) && !vrai(get(e, "notification_rupture")?) && declarations::fctu::notification_exigee(&valeur_en_texte(motif)).map_err(plain)? {
        manque.push(json!(format!(
            "`notification_rupture` : la date de notification de la rupture (S21.G00.62.003) est obligatoire pour le motif {} \
(CCH-12, cahier technique DSN p. 275) : l'envoi de la lettre de licenciement ou de rupture, la remise de la demission, la \
notification de la fin de la periode d'essai. Le signalement ne s'ecrit pas sans elle.",
            valeur_en_texte(motif)
        )));
    }
    let preavis = ou(get(e, "preavis")?, objet_vide());
    if !vrai(get(preavis, "type")?) {
        let requis = |k: &str| e.get(k).ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
        let (debut, fin) = (d(requis("debut")?).map_err(|m| erreur("ValueError", m))?, d(requis("fin")?).map_err(|m| erreur("ValueError", m))?);
        let anciennete = droit::permanents::anciennete_mois(&debut, &fin);
        let motif_texte = if vrai(motif) { Some(valeur_en_texte(motif)) } else { None };
        let cdd = !nature_de(get(e, "nature")?)?.duree_indeterminee;
        let duree = droit::permanents::preavis_mois(anciennete, motif_texte.as_deref(), cdd).map_err(plain)?;
        let regle = droit::permanents::regle_du_preavis(motif_texte.as_deref(), cdd).map_err(plain)?;
        let mut dit = match (duree, &regle) {
            (Some(m), _) if m != Decimal::ZERO => format!("La loi en donne {} mois (L1234-1).", m),
            (Some(_), _) => format!(
                "La loi ne donne aucun preavis pour ce motif : {}",
                droit::permanents::raison_sans_preavis(motif_texte.as_deref(), cdd).map_err(plain)?.unwrap_or_default()
            ),
            (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
            (None, None) => "Sous six mois d'anciennete, la loi renvoie a la convention ou aux usages (L1234-1) : elle ne dit pas de duree.".to_string(),
        };
        // Ce que dit la convention, a cote, par le moteur (le calcul de `moteur preavis`) ; pas pour un motif sans preavis
        // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
        if duree != Some(Decimal::ZERO) && regle.is_none() {
            let tout = engagement::preavis(ctx, e, anciennete, e.get("motif_rupture").and_then(Value::as_str)).map_err(plain)?;
            if let (Some(conv), true) = (tout.convention, tout.qui != "la loi (L1234-1)") {
                dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", tout.qui, conv));
            } else if tout.mois.is_none() {
                if let Some(pourquoi) = tout.pourquoi.as_ref().filter(|p| !p.is_empty()) {
                    dit.push_str(&format!(" Et {}.", pourquoi));
                }
            }
        }
        manque.push(json!(format!(
            "`preavis` : {{\"type\": \"...\", \"debut\": \"...\", \"fin\": \"...\"}}. Le bloc S21.G00.63 est OBLIGATOIRE dans un \
             signalement de fin de contrat (cardinalite 1,*) : meme sans preavis, il se declare, avec le type << 90 - pas de clause de \
             preavis applicable >>. {}",
            dit
        )));
    } else if declarations::fctu::preavis_non_effectue(&valeur_en_texte(get(preavis, "type")?)).map_err(plain)? && !vrai(get(e, "dernier_jour_paye")?) {
        manque.push(json!(
            "`dernier_jour_paye` : sur un preavis non effectue, le dernier jour travaille et paye au salaire habituel \
(S21.G00.62.006) n'est pas la fin du contrat, il precede le debut du preavis (CCH-12, cahier technique DSN p. 275). La \
mensuelle et le signalement le declarent tous deux."
        ));
    }
    Ok(manque)
}

/// Les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`rappels`) la lisent.
pub(crate) fn fins_a_signaler(ctx: &Contexte, mois: &str, aujourdhui: Jour) -> R<Vec<Value>> {
    let Some(saisie) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else {
        return Ok(Vec::new());
    };
    let fin_mois = fin_du_mois(mois)?;
    let engagements = getd(&saisie, "engagements", &Value::Null)?;
    let liste = if vrai(engagements) { items(engagements)? } else { Vec::new() };
    let mut out = Vec::new();
    let journal = envois::etats::charger(&ctx.racine)?;
    for e in liste {
        let e: &Value = &e;
        let nat = nature_de(get(e, "nature")?)?;
        // La regle est celle de la nature et du motif (`Nature::signalement_fctu`) : un CDDU rompu avant son terme se signale.
        let motif = e.get("motif_rupture").filter(|m| vrai(m)).map(valeur_en_texte);
        if !nat.signalement_fctu(motif.as_deref()) || !vrai(get(e, "fin")?) {
            continue;
        }
        let debut_du_mois = d(&json!(format!("{}-01", mois))).map_err(|m| erreur("ValueError", m))?;
        let fin = d(&e["fin"]).map_err(|m| erreur("ValueError", m))?;
        if !(debut_du_mois <= fin && fin <= fin_mois) {
            continue;
        }
        let salarie = e.get("salarie").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("salarie")))?;
        // La cle de la fiche : celle que porte le nom du fichier du signalement (`fin-contrat-signaler`).
        let cle = ctx.salarie(&valeur_en_texte(salarie)).ok().and_then(|f| f.get("_cle").map(valeur_en_texte));
        let a_saisir = ce_qui_manque(ctx, e)?;
        // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`).
        let obligation = crate::obligations::de_la_fin(ctx, cle.as_deref(), fin, &journal, aujourdhui)?;
        // Due : il manque de quoi signaler, aucun delai ne se lit, ou le delai court. Sa pastille : ce qui manque, sinon
        // celle du delai.
        let incomplet = !a_saisir.is_empty();
        let du = incomplet || obligation.is_null() || obligation.get("du").is_some_and(vrai);
        let pastille = if incomplet { super::pastille("a_faire", "à compléter") } else { obligation.get("pastille").cloned().unwrap_or(Value::Null) };
        out.push(json!({
            "salarie": salarie,
            "numero": get(e, "numero")?,
            "fin": e["fin"],
            "nature": nat.libelle,
            "motif_rupture": get(e, "motif_rupture")?,
            "preavis": get(e, "preavis")?,
            "a_saisir": a_saisir,
            "du": du,
            "pastille": pastille,
            "obligation": obligation,
        }));
    }
    Ok(out)
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, charge: &Value) -> R<Value> {
    let brut = charge.get("mois").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("mois")))?;
    let fins = fins_a_signaler(ctx, &valeur_en_texte(brut), ctx.aujourdhui)?;
    Ok(json!({"mois": brut, "fins": fins}))
}

// ------------------------------------------------------------ regularisations





/// `{"contrats": [...]}` : les
/// contrats d'un salarie qui couvrent un jour, lus dans les saisies.
fn arret_contrats(ctx: &Contexte, charge: &Value) -> R<Value> {
    let sal = exiger(charge, "salarie")?;
    let jour = exiger(charge, "jour")?;
    let cle = match salarie(ctx, sal)?.get("_cle") {
        Some(c) => c.clone(),
        None => return Err(erreur("KeyError", socle::fichiers::absent("_cle"))),
    };
    let jour = d(jour).map_err(|m| erreur("ValueError", m))?.iso();
    Ok(json!({"contrats": contrats_au(ctx, &cle, &jour)?}))
}

/// Les contrats de la fiche `cle` qui couvrent `jour` (AAAA-MM-JJ),
/// lus dans les saisies, du plus ancien debut au plus recent. Chacun : salarie, nom, numero, debut,
/// fin, mois, spectacle.
pub(crate) fn contrats_au(ctx: &Contexte, cle: &Value, jour: &str) -> R<Vec<Value>> {
    let cle_texte = valeur_en_texte(cle);
    let mut sorties: Vec<(String, Value)> = Vec::new();
    let mut vus: Dico<()> = Dico::nouveau();
    let mut mois = engagement::stock::mois_saisis(&ctx.racine).map_err(socle::echec::Echec::from)?;
    mois.reverse();
    for m in mois {
        for (c, e) in crate::routes::mois::engagements_du_mois(ctx, &m)? {
            if c != cle_texte {
                continue;
            }
            let (debut, fin) = crate::routes::mois::bornes_brutes(&e)?;
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Le mois le plus recent qui le saisit fait foi (une fin ecrite plus
            // tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map(|n| n.mensuelle()).unwrap_or(false);
            let numero = e.get("numero").cloned().unwrap_or(Value::Null);
            if debut.is_empty() || (fin.is_empty() && !mensuel) {
                continue;
            }
            if vus.contient(&[json!(c), json!(debut), numero.clone()])? {
                continue;
            }
            vus.poser(vec![json!(c), json!(debut), numero.clone()], ())?;
            if !(debut.as_str() <= jour && (fin.is_empty() || jour <= fin.as_str())) {
                continue;
            }
            let s = salarie(ctx, cle)?;
            let champ = |k: &str| -> R<Value> { s.get(k).cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k))) };
            let prenom = match s.get("prenom_affichage").filter(|x| vrai(x)) {
                Some(p) => p.clone(),
                None => champ("prenom")?,
            };
            let nom = format!("{} {}", valeur_en_texte(&champ("nom")?), valeur_en_texte(&prenom));
            let ou_vide = |k: &str| e.get(k).filter(|x| vrai(x)).cloned().unwrap_or_else(|| json!(""));
            let mut o = Map::new();
            o.insert("salarie".into(), cle.clone());
            o.insert("nom".into(), json!(nom));
            o.insert("numero".into(), brut(&ou_vide("numero")));
            o.insert("debut".into(), json!(debut));
            o.insert("fin".into(), json!(fin));
            o.insert("mois".into(), json!(m));
            o.insert("spectacle".into(), brut(&ou_vide("spectacle")));
            // Paye au mois (`Nature::mensuelle`) : son mois de paie se tire du moteur, et un arret y appelle le maintien de
            // salaire (L1226-1 : pas pour un intermittent).
            o.insert("mensuel".into(), json!(mensuel));
            sorties.push((debut, Value::Object(o)));
        }
    }
    sorties.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sorties.into_iter().map(|(_, v)| v).collect())
}
