//! Les corrections des mois deja declares : l'etat d'un mois depose (`etat_mois` : se corrige-t-il par regularisation),
//! la liste des corrections (`corrections_par_mois`, route `regularisations`) ; et ce que la saisie d'un mois declare
//! en dit (`modification_de_l_engagement`). Rien n'est ecrit.
//!
//! Corriger, c'est modifier l'engagement et le revalider. Ce qui est a corriger se tire de la comparaison entre la
//! version presente de l'engagement valide et celle sur laquelle la DSN est partie
//! (`produire_regularisations::corrections_dues`).

use std::str::FromStr;

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::valeur::valeur_en_texte;
use socle::valeur::nom_type;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::{q2, Decimal};
use socle::echec::Echec;
use crate::routes::etapes::Moteur;

#[track_caller]
fn non_regularisable(message: impl Into<String>) -> Echec {
    erreur("MoisNonRegularisable", message)
}

// ------------------------------------------------------------------ le mois d'origine

/// `etat_mois` : ce mois se corrige-t-il par regularisation, et sinon, quoi faire.
pub(crate) fn etat_mois(m: &mut Moteur, ctx: &Contexte, mois_v: &Value) -> R<Value> {
    let Some(mois) = mois_v.as_str() else {
        return Err(erreur("TypeError", format!("'mois' : {}", socle::fichiers::attendu("un texte", nom_type(mois_v)))));
    };
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|m| erreur("ValueError", m))?;
    let limite = echeance.jour;
    let passee = echeance.passee_au(&ctx.aujourdhui);
    let etat = crate::import::mois::dsn_avant(ctx, mois, &Value::Null)?["etat"].as_str().unwrap_or_default().to_string();
    let deposee = matches!(etat.as_str(), "passee" | "attente" | "refusee");
    let drapeau = |k: &str| etat == k;
    let mut out = Map::new();
    out.insert("mois".into(), json!(mois));
    out.insert("deposee".into(), json!(deposee));
    out.insert("passee".into(), json!(drapeau("passee")));
    out.insert("refusee".into(), json!(drapeau("refusee")));
    out.insert("echeance".into(), json!(limite.iso()));
    out.insert("echeance_depot".into(), echeance.en_json());
    out.insert("echeance_passee".into(), json!(passee));
    out.insert("regularisable".into(), json!(false));
    out.insert("raison".into(), json!(""));
    if !deposee {
        let suite = if passee { format!(" (en retard : `moteur depot-dsn {} --test`, puis --reel)", mois) } else { String::new() };
        out.insert(
            "raison".into(),
            json!(format!("Aucune DSN reelle n'est partie pour {} : ce mois se saisit et se depose normalement{}.", mois, suite)),
        );
    } else if drapeau("refusee") {
        out.insert(
            "raison".into(),
            json!(format!("La DSN de {} a ete REFUSEE : elle se corrige et se redepose, il n'y a rien a regulariser.", mois)),
        );
    } else {
        // LA FRONTIERE EST LA VEILLE A MINUIT, pas le jour de l'echeance.
        let veille = m.limite(mois)?;
        if ctx.aujourdhui <= veille {
            out.insert(
                "raison".into(),
                json!(format!(
                    "La DSN de {} est partie et l'annule-et-remplace est admis jusqu'au {} a minuit : modifier le mois et refaire la DSN, elle partira en annule-et-remplace.",
                    mois,
                    veille.jj_mm_aaaa()
                )),
            );
        } else {
            out.insert("regularisable".into(), json!(true));
            out.insert(
                "raison".into(),
                json!(format!(
                    "La DSN de {} est deposee et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance du {}) : une correction part dans la DSN du mois courant, datee de {}.",
                    mois,
                    veille.jj_mm_aaaa(),
                    limite.jj_mm_aaaa(),
                    mois
                )),
            );
        }
    }
    Ok(Value::Object(out))
}

/// Le resume d'une paie que la page compare : montants, cotisations, activite.
fn resume(p: &Paie) -> Value {
    let e = &p.engagement;
    let heures = match e.heures {
        Some(h) if h != 0.0 => socle::ecriture::repr_flottant(h),
        _ => "0".to_string(),
    };
    json!({
        "brut": q2(p.brut),
        "net_a_payer": q2(p.net_a_payer),
        "net_imposable": q2(p.net_imposable),
        "cotisations_salariales": q2(p.total_sal),
        "cotisations_patronales": q2(p.total_pat),
        "cout_employeur": q2(p.cout_employeur),
        "cachets": e.cachets,
        "heures": heures,
        "jours": e.jours,
        "remuneration": p.remuneration.iter().map(|l| l.designation()).collect::<Vec<_>>(),
    })
}

fn decimal(v: &Value, k: &str) -> R<Decimal> {
    Decimal::from_str(v[k].as_str().unwrap_or("0")).map_err(|e| erreur("InvalidOperation", e.to_string()))
}

/// La difference d'activite (cachets, heures) entre deux paies resumees (`resume`), dans `delta` : la page la montre a
/// cote de celle des montants, sans la calculer (regle 2).
fn difference_d_activite(delta: &mut Map<String, Value>, avant: &Value, apres: &Value) -> R<()> {
    let entier = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    delta.insert("cachets".into(), json!(entier(apres, "cachets") - entier(avant, "cachets")));
    let d = decimal(apres, "heures")? - decimal(avant, "heures")?;
    delta.insert("heures".into(), json!(d.normalize().to_string()));
    Ok(())
}

/// Ce que la page dit d'une correction qui change les dates du contrat : les dates declarees, les corrigees, ce que la
/// DSN suivante en declare (`les_dates_d_un_contrat_declare`), et, quand le contrat court sur d'autres mois, que chacun
/// se corrige pour lui-meme.
fn phrase_des_dates(mois: &str, declarees: &Bornes, corrigees: &Bornes) -> R<String> {
    let mut quoi = Vec::new();
    if declarees.0 != corrigees.0 {
        quoi.push("la nouvelle date de début avec l'ancienne, dans un bloc « Changements contrat » (S21.G00.41.021, fiche net-entreprises 1952)");
    }
    if declarees.1 != corrigees.1 {
        quoi.push("la nouvelle fin dans un bloc « Fin du contrat » (S21.G00.62, fiche net-entreprises 1722) quand le contrat a fini");
    }
    let mut phrase = format!(
        "Dates corrigées : {} (déclaré : {}). La DSN suivante déclare {}.",
        bornes_en_clair(corrigees),
        bornes_en_clair(declarees),
        quoi.join(", et ")
    );
    let autres: Vec<String> = mois_des_bornes(corrigees)?.into_iter().chain(mois_des_bornes(declarees)?).filter(|m| m != mois).fold(Vec::new(), |mut v, m| {
        if !v.contains(&m) {
            v.push(m);
        }
        v
    });
    if !autres.is_empty() {
        phrase.push_str(&format!(
            " Le contrat court aussi sur {} : chaque mois se corrige pour lui-même, et la DSN suivante porte un versement par mois civil (cahier technique DSN §2.2.2.5).",
            autres.iter().map(|m| mois_en_clair(m)).collect::<Vec<_>>().join(" et ")
        ));
    }
    Ok(phrase)
}

/// Les mois (AAAA-MM) que couvrent des bornes ; sans fin, le seul premier mois.
fn mois_des_bornes(b: &Bornes) -> R<Vec<String>> {
    let debut = socle::date::Jour::lire(&b.0).map_err(|m| erreur("ValueError", m))?;
    let fin = match &b.1 {
        Some(f) => socle::date::Jour::lire(f).map_err(|m| erreur("ValueError", m))?,
        None => debut,
    };
    Ok(socle::date::mois_entre(&debut, &fin))
}

// ------------------------------------------------- modifier la paie d'un mois deja declare

/// La classe du refus de ce qui ne se declare pas apres la limite (`refus_non_declarable`) : la page le dit et recharge
/// la saisie enregistree.
pub const CLASSE_NON_DECLARABLE: &str = "ModificationNonDeclarable";

/// `septembre 2026`, avec ses accents.
pub(crate) fn mois_en_clair(mois: &str) -> String {
    socle::date::libelle_du_mois_ou_tel_quel(mois, socle::date::Graphie::Accentuee)
}

/// `de septembre 2026`, `d'octobre 2026`.
pub(crate) fn de_mois_en_clair(mois: &str) -> String {
    envois::envoi::de_mois(&mois_en_clair(mois))
}

/// Ce que « Modifier l'engagement » dit d'un mois dont la DSN reelle est deposee, quand une modification le touche
/// (`modifier-engagement`) ; `Null` quand aucune DSN reelle acceptee n'est partie.
///
/// * tant que l'annule-et-remplace est admis (jusqu'a la veille de l'echeance, minuit) : une phrase, rien a confirmer ;
/// * apres : `confirmer`, et l'avertissement que la page montre a la premiere modification. Il dit ce qui est VRAI : la DSN
///   deposee ne change plus ; la correction se declare dans la DSN qui porte (`dsn_qui_porte` : la premiere d'un mois
///   suivant qui n'est pas partie), des que l'engagement modifie est revalide
///   (`produire_regularisations::regularisations_pour`). `porteur` : son mois, que l'avertissement de « Modifier
///   l'engagement » et le panneau des etapes nomment aussi.
///
/// Un mois dont l'ancien logiciel a depose la DSN se dit avec la phrase du panneau et du rappel
/// (`import::mois::dsn_de_l_ancien_logiciel`) : cette DSN ne se remplace jamais ici (regle 4), ce que la saisie y ajoute
/// se declare dans la DSN qui porte (`dsn_qui_porte`). Se confirme seulement la modification d'un engagement de la
/// saisie qu'une DSN partie ici a deja declare pour ce mois (le journal des envois, `departs`).
pub(crate) fn modification_de_l_engagement(m: &mut Moteur, ctx: &Contexte, mois: &str) -> R<Value> {
    if let Some(d) = crate::import::mois::dsn_de_l_ancien_logiciel(ctx, mois)? {
        let mut r = json!({"confirmer": false, "porteur": d["porteur"], "phrase": d["phrase"]});
        let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
        let declare_ici = departs.engagements().any(|(c, n)| departs.de(c, n).iter().any(|x| x.envoi == engagement::departs::Envoi::Dsn && x.mois.as_deref() == Some(mois)));
        if declare_ici {
            r["confirmer"] = json!(true);
            r["avertissement"] = json!(format!(
                "La DSN {} a été déposée par l'ancien logiciel : elle ne se remplace pas. La correction se déclarera dans la DSN {}. Êtes-vous sûr de vouloir modifier cet engagement ?",
                de_mois_en_clair(mois),
                de_mois_en_clair(&valeur_en_texte(&d["porteur"]))
            ));
        }
        return Ok(r);
    }
    let etat = etat_mois(m, ctx, &json!(mois))?;
    if !vrai(&etat["deposee"]) || vrai(&etat["refusee"]) {
        return Ok(Value::Null);
    }
    let veille = m.limite(mois)?;
    let de = de_mois_en_clair(mois);
    if !vrai(&etat["regularisable"]) {
        return Ok(json!({
            "confirmer": false,
            "limite": veille.iso(),
            "phrase": format!(
                "La DSN {} est déposée : la modifier fera partir un annule-et-remplace, jusqu'au {} à minuit.",
                de,
                veille.jj_mm_aaaa()
            ),
        }));
    }
    let porteur_iso = dsn_qui_porte(ctx, mois)?;
    let porteur = de_mois_en_clair(&porteur_iso);
    Ok(json!({
        "confirmer": true,
        "limite": veille.iso(),
        "porteur": porteur_iso,
        "phrase": format!(
            "La DSN {} est déposée et ne peut plus être remplacée : modifier cet engagement se confirme, et la correction se déclare dans la DSN {}.",
            de, porteur
        ),
        "avertissement": format!(
            "La limite de la DSN {} est passée : l'annule-et-remplace n'est plus admis depuis le {} à minuit, on ne peut plus la remplacer. La DSN déposée ne changera pas : la correction se déclarera dans la DSN {}. Êtes-vous sûr de vouloir modifier cet engagement ?",
            de,
            veille.jj_mm_aaaa(),
            porteur
        ),
    }))
}

/// Les bornes d'un contrat : son premier jour et sa fin (ISO), telles que le moteur les lit.
pub(crate) type Bornes = (String, Option<String>);

fn bornes(p: &Paie) -> Bornes {
    (p.engagement.debut.iso(), p.engagement.fin.map(|f| f.iso()))
}

/// « du 02/07/2026 au 25/07/2026 ».
pub(crate) fn bornes_en_clair(b: &Bornes) -> String {
    let jour = |t: &str| socle::date::Jour::lire(t).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| t.to_string());
    match &b.1 {
        Some(f) => format!("du {} au {}", jour(&b.0), jour(f)),
        None => format!("à partir du {}, sans fin", jour(&b.0)),
    }
}

/// UN CONTRAT DECLARE NE SE RETIRE PAS D'UN MOIS DONT LA DSN NE SE REMPLACE PLUS. Passe la date d'exigibilite, une DSN
/// ne se remplace plus : « il convient de corriger les informations erronees dans la DSN du mois suivant » (fiche
/// net-entreprises 1722, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche1722-annulation-fin-de-contrat-transmise-a-tort-20250326.txt`). Ce qui s'y corrige : la paie,
/// par rappel, en approche differentielle ou par annulation et remplacement des montants (cahier technique DSN 2026.1,
/// §2.1.3.6, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, l. 607) ; et, d'un contrat,
/// sa fin, que le motif 099 annule (fiche 1722). INFERENCE : aucun de ces textes n'annule un contrat declare (son bloc
/// S21.G00.40) ; un contrat qui n'a pas eu lieu garde son bloc et voit ses montants annules (ses quantites a zero).
///
/// `avant` : l'engagement tel qu'il est ecrit ; `apres` : ce qu'il devient. Refus : un mois que sa DSN a declare pour
/// lui (`departs`) et dont la DSN ne se remplace plus (`modification_de_l_engagement`, `confirmer`), qu'il ne couvre plus --
/// ou tous, quand le contrat change de salarie. Le refus nomme la DSN qui a declare le contrat pour ce mois : la sienne,
/// ou celle d'un mois suivant qui a porte sa correction (`Depart::par`). `Ok(None)` : rien d'indeclarable.
pub(crate) fn refus_non_declarable(
    m: &mut Moteur,
    ctx: &Contexte,
    avant: &engagement::stock::Objet,
    apres: &engagement::stock::Objet,
    departs: &engagement::departs::Departs,
) -> R<Option<String>> {
    let brut = |o: &engagement::stock::Objet| Value::Object(o.brut.clone());
    let (b_avant, b_apres) = (brut(avant), brut(apres));
    let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &b_avant), b_avant.get("numero").filter(|n| vrai(n)).map(valeur_en_texte)) else {
        return Ok(None);
    };
    let meme_salarie = engagement::cle_de_l_engagement(ctx, &b_apres).as_deref() == Some(cle.as_str());
    for d in departs.de(&cle, &numero).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dsn) {
        let Some(mois) = d.mois.as_deref() else { continue };
        if meme_salarie && apres.couvre(mois).map_err(Echec::from)? {
            continue;
        }
        if !modification_de_l_engagement(m, ctx, mois)?.get("confirmer").is_some_and(vrai) {
            continue;
        }
        let nom = b_avant.get("salarie").map(valeur_en_texte).unwrap_or_else(|| cle.clone());
        let debut = socle::date::Jour::lire(&engagement::debut_brut(&b_avant)).map(|j| j.jj_mm_aaaa()).unwrap_or_default();
        let dsn = match departs.de(&cle, &numero).iter().find(|x| x.envoi == engagement::departs::Envoi::Dsn && x.mois.as_deref() == Some(mois)).and_then(|x| x.par.clone()) {
            Some(par) => format!("dans la DSN {}, qui a porté la correction {}", de_mois_en_clair(&par), de_mois_en_clair(mois)),
            None => format!("dans la DSN {}, qui ne se remplace plus", de_mois_en_clair(mois)),
        };
        return Ok(Some(format!(
            "Le contrat de {} du {} est déclaré {} : il ne se retire plus. La DSN corrige la paie d'un mois passé (rappel de paie, cahier technique DSN §2.1.3.6) et annule une fin de contrat (motif 099, fiche net-entreprises 1722), pas un contrat. S'il n'a pas eu lieu, gardez-le et mettez ses quantités à zéro : la DSN suivante déclarera l'annulation de ses montants. Rien n'est enregistré.",
            nom, debut, dsn
        )));
    }
    Ok(None)
}


/// La DPAE d'un contrat dont la date d'embauche a change (`crate::routes::etapes::debut_declare`), deposee pour l'ancienne date `avant` :
/// vaut-elle pour la nouvelle `apres`, et la phrase qui le dit. Code du travail, `atelier/reference/sources/code-du-travail/`
/// (`xml/L1221-10.xml`, `xml/R1221-1.xml`, `xml/R1221-4.xml`) : « L'embauche d'un salarie ne peut intervenir qu'apres
/// declaration nominative » (L. 1221-10) ; la DPAE porte la « Date et heure d'embauche » (R. 1221-1, 3°) et part au plus
/// tot huit jours avant « la date previsible de l'embauche » (R. 1221-4). INFERENCE : reculee, l'embauche a eu lieu apres
/// la declaration, qui portait sa date previsible -- elle vaut ; avancee, l'embauche a precede la declaration -- une DPAE
/// a la date vraie est a deposer. La DPAE n'a pas d'annule-et-remplace (guide de l'API DPAE de l'Urssaf,
/// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3) et ne s'annule pas : une correction est une
/// nouvelle declaration (fiche net-entreprises 3300, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche3300-dpae-en-dsn-20261015.txt`, l. 451).
pub(crate) fn dpae_d_une_date_changee(avant: &str, apres: &str) -> (bool, String) {
    let jour = |t: &str| socle::date::Jour::lire(t).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| t.to_string());
    if avant < apres {
        (
            true,
            format!(
                "Embauche reportée du {} au {} : la DPAE déposée pour le {} vaut — elle a précédé l'embauche (code du travail, art. L. 1221-10) et portait sa date prévisible (art. R. 1221-4). Rien à refaire.",
                jour(avant),
                jour(apres),
                jour(avant)
            ),
        )
    } else {
        (
            false,
            format!(
                "Embauche avancée du {} au {} : la DPAE déposée pour le {} ne l'a pas précédée (code du travail, art. L. 1221-10) et porte une autre date d'embauche (art. R. 1221-1, 3°). Une DPAE au {} est à déposer : une DPAE ne se remplace pas (pas d'annule-et-remplace, guide de l'API DPAE de l'Urssaf).",
                jour(avant),
                jour(apres),
                jour(avant),
                jour(apres)
            ),
        )
    }
}

/// Une paie a zero : ce qu'un contrat ajoute avait de declare (rien).
fn resume_nul() -> Value {
    json!({
        "brut": "0.00", "net_a_payer": "0.00", "net_imposable": "0.00", "cotisations_salariales": "0.00",
        "cotisations_patronales": "0.00", "cout_employeur": "0.00", "cachets": 0, "heures": "0", "jours": 0,
        "remuneration": [],
    })
}


// ------------------------------------------------------------- les corrections d'un mois

/// Le nom d'un salarie tel que la page le montre : « NOM Prenom ».
fn nom_de(p: &Paie) -> String {
    let sal = &p.salarie;
    let texte = |k: &str| sal.get(k).filter(|v| vrai(v)).map(valeur_en_texte);
    format!("{} {}", texte("nom").unwrap_or_default(), texte("prenom_affichage").or_else(|| texte("prenom")).unwrap_or_default())
}

/// AVANT, APRES, DIFFERENCE : deux paies ecrites d'un contrat (`avant` : la derniere declaree, `None` pour un contrat
/// qu'aucune DSN n'a declare ; `apres` : celle de la version presente), resumees comme la DSN les declare, et ce que la
/// correction en declare.
fn avant_apres(mois: &str, avant: Option<&Paie>, apres: &Paie) -> R<Value> {
    let r_avant = avant.map_or_else(resume_nul, resume);
    let r_apres = resume(apres);
    let mut delta = Map::new();
    let mut tout_nul = true;
    for k in ["brut", "net_a_payer", "net_imposable", "cotisations_salariales", "cotisations_patronales", "cout_employeur"] {
        let d = decimal(&r_apres, k)? - decimal(&r_avant, k)?;
        tout_nul &= d.is_zero();
        delta.insert(k.to_string(), json!(d.to_string()));
    }
    difference_d_activite(&mut delta, &r_avant, &r_apres)?;
    let dates = match avant {
        Some(a) if bornes(a) != bornes(apres) => Some(phrase_des_dates(mois, &bornes(a), &bornes(apres))?),
        _ => None,
    };
    let rien_ne_change = tout_nul
        && dates.is_none()
        && r_avant["cachets"] == r_apres["cachets"]
        && r_avant["heures"] == r_apres["heures"]
        && r_avant["jours"] == r_apres["jours"];
    Ok(json!({
        "salarie": valeur_en_texte(apres.salarie.get("_cle").unwrap_or(&Value::Null)),
        "debut": apres.engagement.debut.iso(),
        "nom": nom_de(apres),
        "spectacle": apres.titre()?,
        "numero": apres.engagement.numero,
        "avant": r_avant,
        "apres": r_apres,
        "delta": delta,
        "rien_ne_change": rien_ne_change,
        "dates": dates,
        "ajout": avant.is_none(),
    }))
}

/// Ce que la page montre des corrections de `mois` : celles a porter (la paie presente de chaque contrat dont la DSN
/// partie ne declare plus la paie, `produire_regularisations::corrections_dues`, comparee a la derniere declaree) et
/// celles deja portees par la DSN d'un mois suivant (chaque paie declaree par une correction, comparee a celle qu'elle
/// corrigeait). Vide quand le mois ne se corrige pas ailleurs que dans sa DSN (`se_corrige_ailleurs`).
pub(crate) fn corrections_du_mois(ctx: &Contexte, mois: &str) -> R<(Vec<Value>, Vec<Value>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    if !declarations::produire_regularisations::se_corrige_ailleurs(ctx, &departs, mois)? {
        return Ok((Vec::new(), Vec::new()));
    }
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else { return Ok((Vec::new(), Vec::new())) };
    let refus = |m: String| non_regularisable(m);
    let mut a_porter = Vec::new();
    for d in declarations::produire_regularisations::corrections_dues(ctx, mois, "9999-12", &departs)? {
        let apres = d.valide.paie(ctx, &data, d.rang)?;
        a_porter.push(avant_apres(mois, d.declarees.last().map(|x| &x.paie), &apres)?);
    }
    let mut portees = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, &departs)? {
        let Some(v) = e.valide() else { continue };
        let declarees = engagement::etats::paies_declarees(ctx, v.objet(), &departs, mois, "9999-12").map_err(refus)?;
        for (i, x) in declarees.iter().enumerate() {
            let Some(par) = &x.par else { continue };
            let mut c = avant_apres(mois, i.checked_sub(1).map(|j| &declarees[j].paie), &x.paie)?;
            c["portee_par"] = json!(par);
            portees.push(c);
        }
    }
    Ok((a_porter, portees))
}


/// LA DSN QUI PORTE LES CORRECTIONS DE `mois` : la premiere DSN d'un mois suivant qui n'est pas partie (ni ici, ni par
/// l'ancien logiciel, `engagement::importe::dsn_deposee`). Une correction se declare dans la DSN suivante (cahier
/// technique DSN 2026.1, § 2.1.2.1, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`).
pub(crate) fn dsn_qui_porte(ctx: &Contexte, mois: &str) -> R<String> {
    let partie = |m: &str| -> R<bool> { Ok(matches!(crate::import::mois::dsn_avant(ctx, m, &Value::Null)?["etat"].as_str(), Some("passee" | "attente" | "importee"))) };
    let suivant = |m: &str| -> R<String> { Ok(socle::date::Jour::lire(&format!("{}-01", m)).map_err(Echec::from)?.mois_suivant()) };
    let mut porteur = suivant(mois)?;
    while partie(&porteur)? {
        porteur = suivant(&porteur)?;
    }
    Ok(porteur)
}
