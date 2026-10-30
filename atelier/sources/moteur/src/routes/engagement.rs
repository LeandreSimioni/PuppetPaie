//! Les routes de l'engagement : le valider (`valider-engagement`), modifier un engagement valide d'un geste
//! (`modifier-engagement`), et le retirer (`retirer-engagement` : supprime s'il n'en est rien parti, sinon rendu non
//! actif). Un refus garde sa phrase quand elle est utile a la page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, Refus, R};
use socle::fichiers::vrai;
use socle::valeur::valeur_en_texte;



/// Les routes de ce module. `etat-par-engagement` se lit dans l'etat garde (`etat_garde`), que la fenetre refait apres
/// chaque ecriture.
pub const ROUTES: &[&str] = &["valider-engagement", "modifier-engagement", "retirer-engagement"];

/// Celles qui ecrivent.
pub const ECRITURES: &[&str] = ROUTES;

/// Une route de ce module.
pub fn route(_o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    match nom {
        "valider-engagement" => valider_engagement(ctx, charge),
        "modifier-engagement" => modifier_engagement(ctx, charge),
        "retirer-engagement" => retirer_engagement(ctx, charge),
        autre => Err(Echec::from(format!("route inconnue : {}", autre))),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur("ErreurOperation", m)
}

pub(super) fn en_echec(r: Refus) -> Echec {
    Echec { classe: r.classe, message: r.message }
}

/// `charge["cle"]` : un texte requis.
pub(super) fn texte_requis<'a>(charge: &'a Value, cle: &str) -> R<&'a str> {
    crate::routes::fiches::exiger(charge, cle)?.as_str().ok_or_else(|| erreur("TypeError", format!("`{}` : un texte est attendu", cle)))
}

/// `bool(charge.get(cle, defaut))`.
pub(super) fn booleen(charge: &Value, cle: &str, defaut: bool) -> bool {
    charge.get(cle).map_or(defaut, vrai)
}

/// `datetime.now().isoformat(timespec="seconds")`, a l'heure de Paris.
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// L'ENGAGEMENT QU'UNE CARTE MONTRE : celui de l'ecran de `mois` a la place `place` (`engagement::stock::objets_a_l_ecran`,
/// l'ordre dans lequel la route `saisie` rend le mois). Refus : une place que l'ecran ne montre pas.
fn a_la_place(ctx: &Contexte, mois: &str, place: usize) -> R<engagement::stock::Objet> {
    engagement::stock::objets_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?.into_iter().nth(place).ok_or_else(|| {
        refus(format!("{} : l'écran ne montre pas d'engagement à la place {} ; rechargez le mois. Rien n'est écrit.", engagement::libelle_mois(mois), place + 1))
    })
}

/// `place` de la charge : la place de la carte dans l'ecran du mois.
fn place_de(charge: &Value) -> R<usize> {
    Ok(charge.get("place").and_then(Value::as_u64).ok_or_else(|| erreur("TypeError", "`place` : un entier est attendu"))? as usize)
}

/// Le rang de l'engagement `o` parmi ceux que `mois` calcule (`engagement::stock::objets_du_mois`, l'ordre de
/// `vue_du_mois`) ; refus pour un engagement qui ne s'y calcule pas (il lui manque un fait).
fn rang_calcule(ctx: &Contexte, mois: &str, o: &engagement::stock::Objet) -> R<usize> {
    engagement::stock::objets_du_mois(&ctx.racine, mois)
        .map_err(Echec::from)?
        .iter()
        .position(|x| x.fichier == o.fichier && x.rang == o.rang)
        .ok_or_else(|| refus(format!("{} : cet engagement ne se calcule pas dans ce mois.", engagement::libelle_mois(mois))))
}

// ------------------------------------------------------------------ valider-engagement


/// « VALIDER L'ENGAGEMENT », UN GESTE : la carte s'ecrit (`saisie`, et `place`, sa place dans l'ecran du mois, absente
/// pour un engagement neuf : `crate::routes::mois::ecrire_une_carte`), puis l'engagement ecrit se valide : il ecrit ses
/// paies, une par mois qu'il couvre (`engagement::etats::valider`). Il lui manque un fait : la carte reste ecrite en
/// saisie, et la validation refuse en disant lequel. Un engagement valide ne se valide pas (il se modifie par
/// `modifier-engagement`).
fn valider_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    // Valider ne se defait pas : un engagement valide se modifie (`modifier-engagement`) ou se retire.
    if charge.get("valide").is_some() {
        return Err(erreur("TypeError", "`valide` : valider-engagement valide ; un engagement valide se modifie ou se retire. Rien n'est écrit."));
    }
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    // Ce qui manquait aux autres se relit d'abord : un fait a pu se dire depuis, sur une fiche.
    crate::routes::mois::relire_les_manques(ctx)?;
    let place = crate::routes::mois::ecrire_une_carte(ctx, mois, charge)?;
    let o = a_la_place(ctx, mois, place)?;
    if let Some(manque) = o.a_completer() {
        let data = engagement::stock::vue_a_l_ecran(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
        return Err(crate::routes::mois::refus_du_calcul(ctx, &data, manque));
    }
    let i = rang_calcule(ctx, mois, &o)?;
    let departs = envois::journal::departs(&ctx.racine).map_err(Echec::from)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    let le = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::routes::mois::refus_du_calcul(ctx, &data, m))?.le().to_string();
    let e = &data["engagements"][i];
    let (salarie, debut) = (e.get("salarie").cloned().unwrap_or(Value::Null), engagement::debut_brut(e));
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &debut)?;
    }
    let avis = avis_de_validation(ctx, mois, i)?;
    Ok(json!({"salarie": salarie, "debut": debut, "place": place, "valide": le, "avis": avis}))
}

// ------------------------------------------------------------------ modifier-engagement

/// Les fichiers des engagements tels qu'ils sont avant une modification : une modification qui n'aboutit pas (rien n'a
/// change, l'avertissement n'est pas encore confirme, un refus) les remet octet pour octet.
struct Photo {
    dossier: PathBuf,
    fichiers: Vec<(PathBuf, Vec<u8>)>,
}

impl Photo {
    fn prendre(racine: &Path) -> R<Photo> {
        let dossier = racine.join(engagement::stock::DOSSIER);
        let mut fichiers = Vec::new();
        for p in Photo::presents(&dossier) {
            let octets = std::fs::read(&p).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
            fichiers.push((p, octets));
        }
        Ok(Photo { dossier, fichiers })
    }

    fn presents(dossier: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dossier).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()
    }

    fn remettre(&self) -> R<()> {
        let io = |p: &Path, e: std::io::Error| erreur("OSError", format!("{} : {}", p.display(), e));
        for p in Photo::presents(&self.dossier) {
            if !self.fichiers.iter().any(|(q, _)| *q == p) {
                std::fs::remove_file(&p).map_err(|e| io(&p, e))?;
            }
        }
        for (p, octets) in &self.fichiers {
            if std::fs::read(p).ok().as_ref() != Some(octets) {
                std::fs::write(p, octets).map_err(|e| io(p, e))?;
            }
        }
        Ok(())
    }
}

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Value),
    /// Ecrite : la date de sa validation.
    Ecrite(String),
}

/// « MODIFIER L'ENGAGEMENT » : les valeurs que la carte ouverte d'un engagement valide montre, ecrites d'un geste.
/// La carte se designe par sa place dans l'ecran du mois (`place`, que le moteur a rendue) ; `saisie` est ce que
/// sa carte ecrit pour `mois`. Une carte porte un salarie : elle modifie son engagement, et lui seul.
///
/// - Rien n'a change : rien ne s'ecrit, la reponse le dit (`change` faux).
/// - Quelque chose a change, et rien de ce qui est parti ne le demande : la modification s'ecrit, l'engagement reste valide.
/// - Ce qui est parti le demande : la reponse porte l'avertissement, et rien n'est ecrit ; la page le montre et renvoie
///   la demande avec `confirme` (et `a_partir_du`, le mois choisi, `a_partir_du_jour`, son jour quand ce n'est pas le
///   1er). `avertissement` : `engagements`, l'engagement touche nomme (`qui`) avec une ligne par ce que le changement
///   entraine pour un envoi parti sur la version d'avant (`consequences`, `engagement::etats::a_refaire`) : un avenant a
///   faire signer, une DPAE a deposer pour une embauche avancee, un bulletin rectificatif, la DSN qui se corrige ; et,
///   pour un contrat au mois dont un mois est parti, `a_partir_du` : la question « a partir de quel mois ? » (`phrase`),
///   ce que chaque mois parti deviendrait (`si`) et ses choix.
///
/// Refus, et rien n'est ecrit : un engagement qui n'est pas valide (il se saisit sur sa carte), importe, d'un mois tenu
/// avant PuppetPaie, d'un exercice clos ; un changement que la norme ne permet plus de declarer
/// (`crate::regularisation::refus_non_declarable`) ; une saisie qui ne se calcule pas.
fn modifier_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let Some(Value::Object(saisie)) = charge.get("saisie") else {
        return Err(erreur("TypeError", "`saisie` : un objet est attendu"));
    };
    let confirme = booleen(charge, "confirme", false);
    let a_partir_du = charge.get("a_partir_du").filter(|v| vrai(v)).map(valeur_en_texte);
    // Le jour du mois choisi ou le changement prend effet, quand on le dit (le 1er sinon).
    let le = match charge.get("a_partir_du_jour").filter(|v| vrai(v)) {
        None => None,
        Some(v) => Some(valeur_en_texte(v).trim().parse::<u32>().map_err(|_| {
            refus(format!("« le {} » : un jour du mois est attendu, de 1 à 31. Rien n'est écrit.", valeur_en_texte(v)))
        })?),
    };
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let o = a_la_place(ctx, mois, place_de(charge)?)?;
    if !o.est_valide() {
        return Err(refus(format!(
            "{} : cet engagement n'est pas validé ; il se saisit sur sa carte, puis « Valider l'engagement ». Rien n'est écrit.",
            engagement::etats::qui_de(ctx, &o)
        )));
    }
    let i = rang_calcule(ctx, mois, &o)?;
    // Ce qu'une DSN deposee dont la limite est passee a declare : la base de ce que la norme permet encore.
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let declaree = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
    let base = if declaree.get("confirmer").is_some_and(vrai) {
        crate::regularisation::saisie_declaree(ctx, mois)?.or(engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?)
    } else {
        None
    };
    let photo = Photo::prendre(&ctx.racine)?;
    let issue = modifier_puis_revalider(ctx, mois, i, saisie, base.as_ref(), confirme, a_partir_du.as_deref(), le);
    let valide = match issue {
        Ok(Issue::Ecrite(valide)) => valide,
        Ok(Issue::Rien) => {
            photo.remettre()?;
            return Ok(json!({"change": false}));
        }
        Ok(Issue::Avertir(a)) => {
            photo.remettre()?;
            return Ok(json!({"change": true, "avertissement": a}));
        }
        Err(e) => {
            photo.remettre()?;
            return Err(e);
        }
    };
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
    let e = &data["engagements"][i];
    if let Some(cle) = engagement::cle_de_l_engagement(ctx, e) {
        crate::routes::fiches::poser_anciennete_a_l_embauche(ctx, &cle, &engagement::debut_brut(e))?;
    }
    let avis = avis_de_validation(ctx, mois, i)?;
    crate::interface::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| erreur("OSError", m))?;
    Ok(json!({"change": true, "valide": valide, "avis": avis}))
}

/// La modification elle-meme, sur les fichiers : l'engagement de rang `i` se rouvre, prend sa saisie et se revalide. Ce
/// qu'elle rend dit si elle reste ecrite (`Issue::Ecrite`) ; sinon la route remet les fichiers.
#[allow(clippy::too_many_arguments)]
fn modifier_puis_revalider(
    ctx: &Contexte,
    mois: &str,
    i: usize,
    saisie: &Map<String, Value>,
    base: Option<&Value>,
    confirme: bool,
    a_partir_du: Option<&str>,
    le: Option<u32>,
) -> R<Issue> {
    engagement::etats::modifier(ctx, mois, i).map_err(Echec::from)?;
    engagement::stock::poser_la_saisie(ctx, mois, i, saisie).map_err(refus)?;
    if !engagement::etats::a_revalider_rouvert(ctx, mois, i).map_err(Echec::from)? {
        return Ok(Issue::Rien);
    }
    if let Some(base) = base {
        let nouvelle = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)?.unwrap_or(Value::Null);
        if let Some(texte) = crate::regularisation::refus_non_declarable(ctx, mois, base, &nouvelle)? {
            return Err(erreur(crate::regularisation::CLASSE_NON_DECLARABLE, texte));
        }
    }
    let departs = envois::journal::departs(&ctx.racine).map_err(Echec::from)?;
    let en_valeur = |qui: String, c: Vec<String>| json!([{"qui": qui, "consequences": c}]);
    // « A PARTIR DE QUEL MOIS ? » : un contrat au mois dont un mois est parti et dont les termes changent. Sans reponse
    // parmi ses choix, la question vient dans l'avertissement ; un seul choix vaut reponse.
    if let Some(q) = engagement::etats::question_a_partir_du(ctx, mois, i, &departs).map_err(Echec::from)? {
        let depuis = a_partir_du.filter(|m| q.choix.iter().any(|c| c == m)).map(String::from).or_else(|| (q.choix.len() == 1).then(|| q.choix[0].clone()));
        match depuis {
            Some(d) => engagement::etats::poser_a_partir_du(ctx, mois, i, &d, le, &departs).map_err(refus)?,
            None => {
                let o = engagement::stock::objet_du_mois(&ctx.racine, mois, i).map_err(Echec::from)?;
                let avenant = o.as_ref().is_some_and(|o| contrat_parti(ctx, o, &departs));
                let choix: Vec<Value> =
                    q.choix.iter().map(|m| json!({"mois": m, "libelle": engagement::libelle_mois(m), "partis": q.partis.contains(m)})).collect();
                let question = json!({"phrase": q.phrase(), "si": ce_qui_arrive_aux_mois_partis(ctx, &q)?, "choix": choix});
                let lignes = if avenant { vec![UN_AVENANT.to_string()] } else { Vec::new() };
                return Ok(Issue::Avertir(json!({"engagements": en_valeur(q.qui.clone(), lignes), "a_partir_du": question})));
            }
        }
    }
    let v = engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| {
        let data = engagement::stock::vue_a_l_ecran(&ctx.racine, mois).ok().flatten().unwrap_or(Value::Null);
        crate::routes::mois::refus_du_calcul(ctx, &data, m)
    })?;
    // L'AVERTISSEMENT : l'engagement touche, nomme (`qui`), et une ligne par ce que le changement entraine pour lui.
    let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
    if !confirme && !lignes.is_empty() {
        return Ok(Issue::Avertir(json!({"engagements": en_valeur(engagement::etats::qui_de(ctx, v.objet()), lignes), "a_partir_du": Value::Null})));
    }
    Ok(Issue::Ecrite(v.le().to_string()))
}

/// Le contrat (ou un avenant) de l'engagement `o` est-il parti ?
fn contrat_parti(ctx: &Contexte, o: &engagement::stock::Objet, departs: &engagement::departs::Departs) -> bool {
    let b = Value::Object(o.brut.clone());
    let (Some(cle), Some(numero)) = (engagement::cle_de_l_engagement(ctx, &b), b.get("numero").filter(|n| vrai(n)).map(valeur_en_texte)) else {
        return false;
    };
    departs.de(&cle, &numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Contrat)
}

/// Ce qu'un contrat parti que le changement modifie entraine.
const UN_AVENANT: &str = "un avenant à faire signer";

/// CE QUE LE CHANGEMENT ENTRAINE, une ligne par consequence, pour l'engagement revalide `v` : chaque envoi parti sur une
/// version qui n'est plus la sienne (`engagement::etats::a_refaire`, sans l'avenant d'un contrat rompu :
/// `crate::routes::contrats::seule_la_rupture`). Une embauche reportee ne refait pas la DPAE
/// (`crate::regularisation::dpae_d_une_date_changee`).
fn ce_que_le_changement_entraine(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Vec<String>> {
    use engagement::etats::ARefaire;
    let saisie = |x: Option<&Value>| x.and_then(|x| x.get("saisie")).cloned().unwrap_or(Value::Null);
    let presente = saisie(v.objet().brut.get("valide"));
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in engagement::etats::a_refaire(ctx, v, departs) {
        match a {
            ARefaire::Avenant { depuis } => {
                if !crate::routes::contrats::seule_la_rupture(&saisie(v.version_numero(depuis)), &presente) {
                    out.push(UN_AVENANT.to_string());
                }
            }
            ARefaire::Dpae { debut_declare, .. } => {
                let (rien_a_refaire, phrase) = crate::regularisation::dpae_d_une_date_changee(&debut_declare, &engagement::debut_brut(&presente));
                if !rien_a_refaire {
                    out.push(phrase);
                }
            }
            ARefaire::Bulletin { mois, .. } => out.push(un_bulletin_rectificatif(&mois)),
            ARefaire::Dsn { mois, .. } => out.push(la_dsn_qui_change(&mut m, ctx, &mois)?),
        }
    }
    Ok(out)
}

/// « un bulletin rectificatif de janvier 2026 » : la paie partie de `mois` se refait.
fn un_bulletin_rectificatif(mois: &str) -> String {
    format!("un bulletin rectificatif {}", crate::regularisation::de_mois_en_clair(mois))
}

/// Ce qui arrive a la DSN deposee de `mois` quand sa paie change, comme toute modification d'un mois declare
/// (`crate::regularisation::modification_de_la_paie`) : un annule-et-remplace jusqu'a la limite, ensuite la correction
/// dans la DSN qui porte (`crate::regularisation::dsn_qui_porte`).
fn la_dsn_qui_change(m: &mut crate::routes::etapes::Moteur, ctx: &Contexte, mois: &str) -> R<String> {
    let de = crate::regularisation::de_mois_en_clair(mois);
    let d = crate::regularisation::modification_de_la_paie(m, ctx, mois)?;
    Ok(if d.get("confirmer").is_some_and(vrai) {
        format!(
            "la correction dans la DSN {} (celle {}, déposée, ne se remplace plus)",
            crate::regularisation::de_mois_en_clair(&valeur_en_texte(&d["porteur"])),
            de
        )
    } else if d.is_null() {
        format!("la DSN {} à corriger", de)
    } else {
        let limite = socle::date::Jour::lire(&valeur_en_texte(&d["limite"])).map(|j| j.jj_mm_aaaa()).unwrap_or_default();
        format!("un annule-et-remplace de la DSN {}, admis jusqu'au {} à minuit", de, limite)
    })
}

/// CE QUI ARRIVE A UN MOIS PARTI que le changement toucherait, pour chaque mois parti parmi les choix de la question
/// « a partir de quel mois ? » : sa paie se refait (bulletin rectificatif), et sa DSN, si elle est deposee, se corrige
/// (`la_dsn_qui_change`).
fn ce_qui_arrive_aux_mois_partis(ctx: &Contexte, q: &engagement::etats::APartirDu) -> R<Vec<String>> {
    let mut m = crate::routes::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for mois in q.choix.iter().filter(|c| q.partis.contains(c)) {
        let d = crate::regularisation::modification_de_la_paie(&mut m, ctx, mois)?;
        let dsn = if d.is_null() { String::new() } else { format!(" ; {}", la_dsn_qui_change(&mut m, ctx, mois)?) };
        out.push(format!("dès {} : {}{}", engagement::libelle_mois(mois), un_bulletin_rectificatif(mois), dsn));
    }
    Ok(out)
}

/// CE QUE LA VALIDATION DIT SANS REFUSER, pour l'engagement de rang `i` de la saisie de `mois` : un contrat que le
/// logiciel fait signer (`engagement::contrat_a_signer`) part a la signature par courriel, a l'adresse de la fiche ; sans
/// elle, il ne partira pas (`compagnie::signature`, qui le refuse au depart). Il s'imprime, ou le courriel se saisit.
fn avis_de_validation(ctx: &Contexte, mois: &str, i: usize) -> R<Vec<Value>> {
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois).map_err(Echec::from)? else { return Ok(Vec::new()) };
    let e = &data["engagements"][i];
    let Some(cle) = engagement::cle_de_l_engagement(ctx, e) else { return Ok(Vec::new()) };
    let nature = droit::natures::nature(e.get("nature").and_then(Value::as_str)).map_err(Echec::from)?;
    let numero = e.get("numero").filter(|n| vrai(n)).map(valeur_en_texte).unwrap_or_default();
    if nature.modele_contrat().is_none() || engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, &cle, &engagement::debut_brut(e), &numero) {
        return Ok(Vec::new());
    }
    let fiche = ctx.salarie(&cle).map_err(Echec::from)?;
    if fiche.get("email").map(valeur_en_texte).is_some_and(|m| !m.trim().is_empty()) {
        return Ok(Vec::new());
    }
    let texte = |c: &str| fiche.get(c).filter(|x| vrai(x)).map(valeur_en_texte);
    let qui = [texte("nom"), texte("prenom_affichage").or_else(|| texte("prenom"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
    // Orange : a faire, et la compagnie peut le faire.
    Ok(vec![json!({"couleur": "orange", "phrase": format!(
        "{} n'a pas de courriel dans sa fiche : son contrat ne pourra pas partir à la signature par courriel. Imprimez-le \
         pour le faire signer, ou saisissez son courriel (étape « Contrats » de la paie, ou sa fiche dans Configuration › \
         Salariés).",
        if qui.is_empty() { cle } else { qui }
    )})])
}

// ------------------------------------------------------------------ retirer-engagement

/// La classe du refus de supprimer un engagement dont quelque chose est parti : la page montre la phrase, et renvoie la
/// demande avec `non_actif` si l'utilisateur veut qu'il ne soit plus actif.
pub const CLASSE_ENGAGEMENT_PARTI: &str = "EngagementParti";

/// « Retirer cet engagement » : la carte se designe par sa place dans l'ecran du mois (`place`, que le moteur a rendue).
/// Tant que rien n'est parti de lui, l'engagement se supprime, meme valide, et rend son numero
/// (`engagement::etats::supprimer`). Des qu'un envoi est parti, le moteur refuse (`CLASSE_ENGAGEMENT_PARTI`, la phrase
/// dit ce qui est parti) ; avec `non_actif`, il devient non actif (`engagement::etats::desactiver`) et la reponse dit ce
/// que cela rend a faire (`a_faire`, en phrases : la DPAE a annuler, la DSN d'un mois a corriger). Rien ne part de
/// lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: &Value) -> R<Value> {
    let mois = texte_requis(charge, "mois")?;
    let _verrou = crate::routes::mois::avant_d_ecrire(ctx, mois)?;
    let o = a_la_place(ctx, mois, place_de(charge)?)?;
    let departs = envois::journal::departs(&ctx.racine).map_err(Echec::from)?;
    if !booleen(charge, "non_actif", false) {
        return match engagement::etats::supprimer(ctx, &o, &departs) {
            Ok(()) => Ok(json!({"retire": "supprime"})),
            Err(m) => Err(erreur(CLASSE_ENGAGEMENT_PARTI, m)),
        };
    }
    let i = rang_calcule(ctx, mois, &o)?;
    let n = engagement::etats::desactiver(ctx, mois, i, &departs).map_err(Echec::from)?;
    Ok(json!({"retire": "non_actif", "le": n.le(), "a_faire": a_faire_en_phrases(&n.a_refaire(ctx, &departs))}))
}

/// Ce qu'un engagement non actif rend a faire, en phrases pour l'ecran.
pub(crate) fn a_faire_en_phrases(a: &[engagement::etats::ARefaire]) -> Vec<String> {
    use engagement::etats::ARefaire;
    a.iter()
        .filter_map(|x| match x {
            ARefaire::Dsn { mois, .. } => Some(format!("DSN de {} à corriger", engagement::libelle_mois(mois))),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------ contrat-recu




// ------------------------------------------------------------------ retenir-logiciel


// ------------------------------------------------------------------ casc-regularise


// ------------------------------------------------------------------ exporter




