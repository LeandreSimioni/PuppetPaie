//! Les routes des IMPREVUS d'un salarie (`imprevus`, 15/10/2026) : la liste (`imprevus`), le maintien de salaire d'un
//! arret (`arret-maintien`), et la suspension du bloc 65 saisie une fois (`suspension-enregistrer`, `-modifier`,
//! `-oublier`). Les arrets se saisissent, se reprennent, se signalent et s'oublient par leurs routes de toujours
//! (`routes_ecritures_g2`, `routes_actes_h1`), dans le meme magasin.
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN. Les refus sont des `ErreurOperation` (la page les montre tels quels) ; un refus
//! n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (regle de Leandre, 15/10/2026 : rien de remplissable s'il n'y a pas a le
//! remplir). Le moteur dit, sur chaque imprevu resume (`resume`), ce que ses faits ouvrent : `maintien_ouvert` (le
//! contrat est paye au mois : un complement de salaire peut s'y verser, mois par mois, `mois_du_maintien`) et
//! `jours_deja_indemnises_attendus` (et le motif est une maladie ou un accident, L1226-1 : D1226-4 compte les jours deja
//! indemnises dans les douze mois). La page n'ouvre que cela.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::contexte::Contexte;
use crate::date::{d, Jour};
use crate::etats::{eq_py, erreur, py_str, R};
use crate::fichiers::vrai;
use crate::imprevus::{self, ARRET, SUSPENSION};
use crate::retours::Echec;
use crate::routes_listes::refus_magasin;

/// Les routes que ce module sert ; toutes sauf `imprevus` ecrivent.
pub const ROUTES: [&str; 5] = ["imprevus", "arret-maintien", "suspension-enregistrer", "suspension-modifier", "suspension-oublier"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "imprevus" => imprevus(ctx, charge),
        "arret-maintien" => arret_maintien(ctx, charge),
        "suspension-enregistrer" => suspension_enregistrer(ctx, charge),
        "suspension-modifier" => suspension_modifier(ctx, charge),
        "suspension-oublier" => suspension_oublier(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

pub(crate) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

fn exiger<'a>(charge: &'a Value, cle: &str) -> R<&'a Value> {
    charge.get(cle).ok_or_else(|| erreur("KeyError", crate::fichiers::absent(cle)))
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
    let fiche = crate::routes_dossiers::salarie(ctx, nom)?;
    Ok(py_str(fiche.get("_cle").ok_or_else(|| erreur("KeyError", crate::fichiers::absent("_cle")))?))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`routes_dossiers::contrats_au`), celui
/// que la charge nomme (`contrat`), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: &Value) -> R<Value> {
    let candidats = crate::routes_dossiers::contrats_au(ctx, &json!(cle), &jour.iso())?;
    let texte_de = |c: &Value, k: &str| py_str(&c[k]);
    if vrai(voulu) {
        return match candidats.iter().find(|c| eq_py(&c["numero"], voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| texte_de(c, "numero")).collect();
                Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    py_str(voulu),
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
    let debut = py_str(&imp["contrat_debut"]);
    let contrats = crate::routes_dossiers::contrats_au(ctx, &imp["salarie"], &debut)?;
    Ok(contrats.iter().any(|c| py_str(&c["debut"]) == debut && c["mensuel"] == json!(true)))
}

/// UN IMPREVU NE CHEVAUCHE PAS UN AUTRE IMPREVU DU MEME CONTRAT : un jour d'absence ne se retire qu'une fois du salaire,
/// et deux periodes qui se recouvrent sur le meme contrat sont deux declarations qui se contredisent (un salarie ne
/// travaille pas le dernier jour travaille d'un arret qui court deja). `sauf` : l'imprevu lui-meme, quand il change.
pub(crate) fn sans_chevauchement(ctx: &Contexte, nouveau: &Value, sauf: Option<&Value>) -> R<()> {
    let (p, dj) = (magasin(imprevus::premier_jour(nouveau))?, magasin(imprevus::dernier_jour(nouveau))?);
    let cle = py_str(&nouveau["salarie"]);
    let debut = jour(&nouveau["contrat_debut"])?;
    let numero = py_str(&nouveau["contrat_numero"]);
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
        && eq_py(&a["salarie"], &b["salarie"])
        && imprevus::jour_de_rangement(a).ok() == imprevus::jour_de_rangement(b).ok()
}

/// L'imprevu de ce genre, de ce salarie, a ce jour de rangement (le dernier jour travaille d'un arret, le debut d'une
/// suspension) : son indice et le fichier de son mois, tous genres compris.
pub(crate) fn trouver(ctx: &Contexte, genre: &str, cle: &str, jour: &Jour) -> R<Option<(usize, Vec<Value>)>> {
    let liste = magasin(imprevus::du_fichier(&ctx.racine, &jour.mois_iso()))?;
    let i = liste.iter().position(|x| {
        imprevus::genre(x) == genre && py_str(&x["salarie"]) == cle && imprevus::jour_de_rangement(x).ok() == Some(*jour)
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
    Ok(if dj < p { Vec::new() } else { crate::date::mois_entre(&p, &dj) })
}

/// Les DSN mensuelles qui ont reporte l'imprevu (`envois` de nature 01).
fn reporte_par(imp: &Value) -> Vec<Value> {
    imp["envois"].as_array().map_or_else(Vec::new, |e| {
        e.iter().filter(|x| x.get("nature").and_then(Value::as_str) == Some("01")).map(|x| x.get("mois").cloned().unwrap_or(Value::Null)).collect()
    })
}

// ------------------------------------------------------------------ le resume

/// UN IMPREVU TEL QUE LA PAGE LE LIT. Un arret : le resume de toujours (`routes_listes::resume_arret` : salarie, contrat,
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
        let mut r = crate::routes_listes::resume_arret(n, imp)?;
        let motif = magasin(imprevus::motif_d_arret(&py_str(&imp["motif"])))?.unwrap_or(Value::Null);
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
        return Ok(r);
    }
    let motif = magasin(imprevus::motif_de_suspension(&py_str(&imp["motif"])))?.unwrap_or(Value::Null);
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
    let regles = crate::permanents::regles().map_err(refus_magasin)?;
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
    let mois = charge.get("mois").filter(|v| vrai(v)).map(py_str);
    let bornes = match &mois {
        Some(m) => Some((
            crate::date::premier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
            crate::date::dernier_du_mois(m).map_err(|e| erreur("ValueError", e))?,
        )),
        None => None,
    };
    let n = crate::routes_listes::nomenclatures(ctx)?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for imp in magasin(imprevus::tous(&ctx.racine))? {
        if cle.as_deref().is_some_and(|c| py_str(&imp["salarie"]) != c) {
            continue;
        }
        if let Some((debut, fin)) = &bornes {
            if !magasin(imprevus::croise(&imp, debut, fin))? {
                continue;
            }
        }
        let mut r = resume(ctx, &n, &imp)?;
        // CE QU'UN ARRET FAIT NAITRE, SUR L'ARRET LUI-MEME (15/10/2026) : la page des imprevus (la carte d'un salarie, la
        // colonne du mois, son dossier) lit une seule liste, et chaque arret y porte ses obligations, calculees au meme
        // endroit que pour la route `arrets` (`obligations::de_l_arret`).
        if imprevus::est_un_arret(&imp) {
            r["obligations"] = Value::Array(crate::obligations::de_l_arret(ctx, &imp, &suivi, ctx.aujourdhui)?);
        }
        out.push(r);
    }
    Ok(json!({
        "imprevus": out,
        "motifs_arret": crate::routes_listes::saisissables(&n, "motifs_arret")?,
        "motifs_reprise": crate::routes_listes::saisissables(&n, "motifs_reprise")?,
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
        _ => Err(refus(format!("{} : un nombre entier de jours, positif ou nul, est attendu ({}).", quoi, py_str(v)))),
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
            py_str(&a["contrat_numero"])
        )));
    }
    let mut nouveau = a.clone();
    if let Some(m) = charge.get("mois") {
        let mois = py_str(m);
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
        let valeur = if vide { None } else { Some(crate::num::dec_py(montant).map_err(|e| refus(format!("Montant du maintien : {}", e)))?) };
        match valeur {
            Some(v) if v < crate::num::Decimal::ZERO => return Err(refus("Un maintien de salaire ne se verse pas en negatif.")),
            Some(v) if v > crate::num::Decimal::ZERO => {
                table.insert(mois, json!(crate::conges::flot(crate::num::eur2(v))));
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
    Ok(json!({"arret": resume(ctx, &n, &nouveau)?}))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &Value, contrat_fin: &str) -> R<()> {
    imprevus::verifier_motif_de_suspension(&py_str(&s["motif"]), &py_str(&s["salarie"])).map_err(refus)?;
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
            py_str(&s["contrat_numero"]),
            jour(&json!(contrat_fin))?.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// La fin du contrat d'une suspension deja ecrite, lue dans les saisies (vide : un contrat qui court).
fn fin_du_contrat(ctx: &Contexte, s: &Value) -> R<String> {
    let debut = py_str(&s["contrat_debut"]);
    let contrats = crate::routes_dossiers::contrats_au(ctx, &s["salarie"], &debut)?;
    Ok(contrats.iter().find(|c| py_str(&c["debut"]) == debut).map(|c| py_str(&c["fin"])).unwrap_or_default())
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
    let motif = py_str(exiger(charge, "motif")?);
    let contrat = contrat_suspendu(ctx, &cle, &debut, optionnel(charge, "contrat"))?;
    let mut s = Map::new();
    s.insert("genre".into(), json!(SUSPENSION));
    s.insert("salarie".into(), json!(cle));
    s.insert("contrat_debut".into(), contrat["debut"].clone());
    s.insert("contrat_numero".into(), contrat["numero"].clone());
    s.insert("motif".into(), json!(motif));
    s.insert("debut".into(), json!(debut.iso()));
    s.insert("fin".into(), json!(fin.iso()));
    s.insert("saisi_le".into(), json!(crate::routes_ecritures_g2::maintenant()));
    s.insert("qui".into(), json!(crate::routes_ecritures_g2::qui_de(charge)));
    s.insert("note".into(), note_de(charge));
    s.insert("envois".into(), json!([]));
    let s = Value::Object(s);
    verifier_suspension(&s, &py_str(&contrat["fin"]))?;
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
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
        s["motif"] = json!(py_str(m));
    }
    if charge.get("note").is_some() {
        s["note"] = note_de(charge);
    }
    verifier_suspension(&s, &fin_du_contrat(ctx, &s)?)?;
    sans_chevauchement(ctx, &s, Some(&avant))?;
    liste[i] = s.clone();
    ecrire(ctx, &debut.mois_iso(), liste)?;
    let n = crate::routes_listes::nomenclatures(ctx)?;
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
    Ok(json!({"suspension": resume(ctx, &n, &s)?}))
}
