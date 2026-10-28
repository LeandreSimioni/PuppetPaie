//! Les routes des IMPREVUS d'un salarie (`imprevus`, 15/10/2026) : la liste (`imprevus`), le maintien de salaire d'un
//! arret (`arret-maintien`), l'avis de prolongation d'un arret (`arret-prolonger`, 21/10/2026), et la suspension du
//! bloc 65 saisie une fois (`suspension-enregistrer`, `-modifier`, `-oublier`). Les arrets se saisissent, se
//! reprennent, se signalent et s'oublient par leurs routes de toujours (`routes_ecritures_g2`, `routes_actes_h1`), dans le
//! meme magasin.
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

use droit::contexte::Contexte;
use socle::date::{d, Jour};
use socle::valeur::valeurs_egales;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use engagement::imprevus::{self, ARRET, SUSPENSION};
use socle::echec::Echec;
use crate::routes_listes::refus_magasin;

/// Les routes que ce module sert ; toutes sauf `imprevus` ecrivent.
pub const ROUTES: [&str; 6] =
    ["imprevus", "arret-maintien", "arret-prolonger", "suspension-enregistrer", "suspension-modifier", "suspension-oublier"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "imprevus" => imprevus(ctx, charge),
        "arret-maintien" => arret_maintien(ctx, charge),
        "arret-prolonger" => arret_prolonger(ctx, charge),
        "suspension-enregistrer" => suspension_enregistrer(ctx, charge),
        "suspension-modifier" => suspension_modifier(ctx, charge),
        "suspension-oublier" => suspension_oublier(ctx, charge),
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
    let fiche = crate::routes_dossiers::salarie(ctx, nom)?;
    Ok(valeur_en_texte(fiche.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`routes_dossiers::contrats_au`), celui
/// que la charge nomme (`contrat`), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: &Value) -> R<Value> {
    let candidats = crate::routes_dossiers::contrats_au(ctx, &json!(cle), &jour.iso())?;
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
    let contrats = crate::routes_dossiers::contrats_au(ctx, &imp["salarie"], &debut)?;
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

/// UN ARRET EN COURS (21/10/2026) : celui dont on peut encore apprendre qu'il se prolonge ou que le salarie a repris plus
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
        // CE QUE L'ARRET OUVRE A LA PAGE (21/10/2026, regle de Leandre : rien de remplissable s'il n'y a pas a le remplir) :
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
    Ok(json!({"arret": resume(ctx, &n, &nouveau)?}))
}

// ------------------------------------------------------------------ la prolongation

/// `arret-prolonger` : `{salarie, dernier_jour, fin?, subrogation_fin?}` -> `{"arret": resume, "subrogation_a_revoir",
/// "phrase", "avertissements"}`. L'AVIS DE PROLONGATION D'UN ARRET EN COURS (21/10/2026).
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
/// S21.G00.60.006) ; ce que la norme refuse au signalement d'arret (`routes_ecritures_g2::verifier`) ; un chevauchement.
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
    crate::routes_ecritures_g2::verifier(&a)?;
    sans_chevauchement(ctx, &a, Some(&avant))?;

    // La trace de l'avis : le jour ou la compagnie l'a su (le jour du moteur), l'heure de la saisie, et ce qui a change.
    let trace = json!({
        "le": ctx.aujourdhui.iso(),
        "saisi_le": crate::routes_ecritures_g2::maintenant(),
        "qui": crate::routes_ecritures_g2::qui_de(charge),
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
    let n = crate::routes_listes::nomenclatures(ctx)?;
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
    let contrats = crate::routes_dossiers::contrats_au(ctx, &s["salarie"], &debut)?;
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
    s.insert("saisi_le".into(), json!(crate::routes_ecritures_g2::maintenant()));
    s.insert("qui".into(), json!(crate::routes_ecritures_g2::qui_de(charge)));
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
        s["motif"] = json!(valeur_en_texte(m));
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
