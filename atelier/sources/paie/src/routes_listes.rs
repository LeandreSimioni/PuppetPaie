//! Les listes que la page lit en s'ouvrant (`paie route`) : `arrets`,
//! `fins-de-contrat`, `regularisations`.
//!
//! C'est le Python de `moteur/web.py` (`_traiter`) et ce qu'il appelait :
//! `operations_arrets` (`tous_les_arrets`, `a_signaler`, `resume`),
//! `signalement` (`motifs_arret`, `motifs_reprise`, `nomenclatures`),
//! `operations_permanents.fins_a_signaler` (+ `_ce_qui_manque_au_signalement`),
//! `operations_regularisation` (`mois_a_corriger`, `etat_mois`,
//! `corrections_par_mois`) et `coffre.ouverture_exigee` -- portes a
//! l'identique : meme JSON (l'enveloppe de `web.py` comprise), memes refus sous
//! la meme classe.
//!
//! LECTURE SEULE, AUCUN CALCUL DE PAIE, AUCUN RESEAU, AUCUN SOUS-PROCESSUS.
//! `fins-de-contrat` demandait au Python un sous-processus `paie preavis` par
//! contrat sans motif de preavis : ici c'est `solde::duree_du_preavis`, appelee
//! directement. Le journal des depots n'est lu qu'une fois par mois de saisie
//! (`regularisations`), par les etats de `etats.rs`.
//!
//! Ecarts assumes : le texte d'un `JSONDecodeError` est celui de serde, pas celui
//! de Python (la classe est la meme) ; un `mois` de correction qui n'est pas un
//! texte (fichier abime a la main) n'est range que pour les cas simples.

use std::collections::HashMap;

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::{d, Jour};
use envois::etats::{get, getd, items, objet_vide, ou};
use socle::valeur::valeur_ecrite;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::{dossier_du_code, json_en_cache, repr, vrai};
use socle::num::Decimal;
use socle::echec::Echec;
use crate::routes_mois::Moteur;

/// Les routes que ce module sert.
pub const ROUTES: [&str; 3] = ["arrets", "fins-de-contrat", "regularisations"];

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "arrets" => arrets(ctx),
        "fins-de-contrat" => fins_de_contrat(ctx, charge),
        "regularisations" => regularisations(ctx),
        autre => Err(Echec { classe: None, message: format!("route inconnue : {}", autre) }),
    }
}

// ------------------------------------------------------------------ les outils

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
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

/// `operations_arrets.resume`.
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
/// (`routes_imprevus::resume`), ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<Value> {
    let tous = tous_les_arrets(ctx)?;
    let n = nomenclatures(ctx)?;
    // LES OBLIGATIONS de chaque arret (`obligations::de_l_arret`, 15/10/2026) : un champ de plus a son resume.
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut a_signaler = Vec::new();
    for a in &tous {
        let mut r = crate::routes_imprevus::resume(ctx, &n, a)?;
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

/// `contrats.nature(cle)` : `KeyError` quand elle est inconnue.
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
        // Ce que dit la convention, a cote, par le moteur (le calcul de `paie preavis`) ; pas pour un motif sans preavis
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

/// `operations_permanents.fins_a_signaler` : les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`routes_ouverture`) la lisent.
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
        out.push(json!({
            "salarie": salarie,
            "numero": get(e, "numero")?,
            "fin": e["fin"],
            "nature": nat.libelle,
            "motif_rupture": get(e, "motif_rupture")?,
            "preavis": get(e, "preavis")?,
            "a_saisir": ce_qui_manque(ctx, e)?,
            // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`, 15/10/2026) : un champ de plus.
            "obligation": crate::obligations::de_la_fin(ctx, cle.as_deref(), fin, &journal, aujourdhui)?,
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

/// Par mois d'origine, les corrections que la page montre (`routes_regularisation::corrections_du_mois`) : celles a
/// porter (`etat` « à porter ») et celles deja portees par la DSN d'un mois suivant (« portée »).
fn corrections_par_mois(ctx: &Contexte) -> R<Vec<Value>> {
    let mut out = Vec::new();
    for mois in crate::routes_mois::mois_saisis(&ctx.racine) {
        let (a_porter, portees) = crate::routes_regularisation::corrections_du_mois(ctx, &mois)?;
        let mut corrections: Vec<Value> = portees
            .into_iter()
            .map(|mut c| {
                c["etat"] = json!("portée");
                c
            })
            .collect();
        corrections.extend(a_porter.into_iter().map(|mut c| {
            c["etat"] = json!("à porter");
            c
        }));
        if !corrections.is_empty() {
            out.push(json!({"mois": mois, "corrections": corrections}));
        }
    }
    Ok(out)
}

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Value> {
    let e = m.dsn_du_mois(mois)?;
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let passee = ctx.aujourdhui > limite;
    let deposee = vrai(&e);
    let drapeau = |k: &str| -> R<bool> { Ok(deposee && vrai(get(&e, k)?)) };
    let (partie_passee, refusee) = (drapeau("passee")?, drapeau("refusee")?);
    // `_remplacement.limite` : la veille de l'echeance (reportee), a minuit.
    let veille = echeance.veille().map_err(plain)?;
    let mut regularisable = false;
    let raison = if !deposee {
        let retard = if passee { format!(" (en retard : `paie depot-dsn {} --test`, puis --reel)", mois) } else { String::new() };
        format!("Aucune DSN reelle n'est partie pour {} : ce mois se saisit et se depose normalement{}.", mois, retard)
    } else if refusee {
        format!("La DSN de {} a ete REFUSEE : elle se corrige et se redepose, il n'y a rien a regulariser.", mois)
    } else if ctx.aujourdhui <= veille {
        format!(
            "La DSN de {} est partie et l'annule-et-remplace est admis jusqu'au {} a minuit : modifier le mois et refaire la DSN, \
             elle partira en annule-et-remplace.",
            mois,
            veille.jj_mm_aaaa()
        )
    } else {
        regularisable = true;
        format!(
            "La DSN de {} est deposee et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance du {}) : \
             une correction part dans la DSN du mois courant, datee de {}.",
            mois,
            veille.jj_mm_aaaa(),
            limite.jj_mm_aaaa(),
            mois
        )
    };
    Ok(json!({
        "mois": mois, "deposee": deposee, "passee": partie_passee, "refusee": refusee,
        "echeance": limite.iso(), "echeance_passee": passee, "echeance_depot": echeance.en_json(),
        "mois_courant": ctx.aujourdhui.mois_iso(),
        "regularisable": regularisable, "raison": raison,
    }))
}

/// `mois_a_corriger` : les mois qui appellent une decision.
fn mois_a_corriger(ctx: &Contexte) -> R<Value> {
    let mut m = Moteur::nouveau(ctx);
    let mut etats = Vec::new();
    for mois in crate::routes_mois::mois_saisis(&ctx.racine) {
        etats.push(etat_mois(&mut m, ctx, &mois)?);
    }
    let choisis = |f: &dyn Fn(&Value) -> bool| -> Vec<Value> { etats.iter().filter(|e| f(e)).cloned().collect() };
    Ok(json!({
        "regularisables": choisis(&|e| e["regularisable"] == json!(true)),
        "en_retard": choisis(&|e| e["echeance_passee"] == json!(true) && e["deposee"] == json!(false)),
    }))
}

/// La route `regularisations`.
fn regularisations(ctx: &Contexte) -> R<Value> {
    let mois = mois_a_corriger(ctx)?;
    Ok(json!({"mois": mois, "corrections": corrections_par_mois(ctx)?}))
}
