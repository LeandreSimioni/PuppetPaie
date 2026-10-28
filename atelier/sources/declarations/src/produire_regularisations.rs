//! Les corrections des mois deja deposes, telles que la DSN d'un mois suivant les embarque : `corrections_dues`,
//! `regularisations_pour`, `resume_regularisations`, `versement_de_la_correction`, `numero_du_bulletin_rectifie` et
//! `ecrire_rectificatifs`.
//!
//! Corriger, c'est modifier l'engagement et le revalider. Ce qui est a corriger se tire de la comparaison entre la
//! version presente de chaque engagement valide et celle sur laquelle les DSN parties l'ont declare
//! (`engagement::etats::correction_due`), pour un mois dont l'annule-et-remplace n'est plus admis
//! (`se_corrige_ailleurs`). La paie d'avant (`Reg::avant`) est la derniere paie declaree du contrat, elle ne se recalcule
//! pas (regle 4) ; la paie d'apres est la paie ecrite de la version presente. La DSN et le bulletin rectificatif
//! impriment ces deux paies telles quelles.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::{q2, Decimal};
use socle::echec::Echec;
use crate::versement_declare::{self, VersementDeclare};

/// Le prefixe de l'archive d'un bulletin rectificatif, a cote du bulletin rectifie.
const PREFIXE_ARCHIVE: &str = "Rectificatif";
/// Le prefixe du fichier par salarie, pour l'envoi (`envoi.PREFIXES["rectificatif"]`).
const PREFIXE_ENVOI: &str = "BulletinRectificatif";

/// Une correction embarquee : un contrat d'un mois deja declare dont la paie validee presente n'est plus celle que les
/// DSN parties ont declaree, et les deux paies.
pub struct Reg {
    pub mois: String,
    /// La version presente de l'engagement, que la correction declare, et le jour ou elle a ete validee.
    pub version: u64,
    pub decidee_le: Option<Jour>,
    /// Un contrat qu'aucune DSN n'a declare pour son mois : ajoute apres elle.
    pub ajout: bool,
    /// La paie d'avant : la derniere paie declaree du contrat, telle que la version de l'engagement sur laquelle la DSN
    /// est partie la porte (`versement_declare::dernier_declare`) ; ce qui s'en imprime et le contrat qu'elle declare. Ce que la correction
    /// annule, au nominatif comme a l'agrege, se lit dans `declare` ; ce qui a ete paye, dans `net_deja_verse`.
    pub avant: Paie,
    /// Le dernier etat declare du contrat : le versement de la paie declaree, celle de la DSN du mois d'origine ou de la
    /// derniere correction deja portee (`versement_declare::dernier_declare`).
    pub declare: VersementDeclare,
    /// Le net a payer avant impot deja verse au salarie pour le contrat, lu dans la derniere paie declaree : celle du
    /// mois d'origine, ou celle de la derniere correction deja portee (`versement_declare::dernier_declare`).
    pub net_deja_verse: Decimal,
    /// La paie corrigee : celle que la version presente de l'engagement valide a ecrite pour le mois d'origine.
    pub apres: Paie,
    pub taux_pas_origine: Option<String>,
    /// Le numero du bulletin que la correction rectifie : le dernier qu'une paie declaree du contrat porte ; pour un
    /// contrat ajoute, celui de la paie corrigee s'il est parti.
    pub bulletin_rectifie: Option<String>,
}

/// UN MOIS SE CORRIGE DANS UNE DSN SUIVANTE quand sa DSN reelle est partie (un depart DSN du mois lui-meme, `departs`)
/// et que son annule-et-remplace n'est plus admis (la veille de l'echeance passee, cahier technique DSN §1.4.2) ; avant,
/// sa propre DSN se refait.
pub fn se_corrige_ailleurs(ctx: &Contexte, departs: &engagement::departs::Departs, mois: &str) -> R<bool> {
    use engagement::departs::Envoi;
    let partie = departs
        .engagements()
        .any(|(c, n)| departs.de(c, n).iter().any(|d| d.envoi == Envoi::Dsn && d.par.is_none() && d.mois.as_deref() == Some(mois)));
    if !partie {
        return Ok(false);
    }
    let limite = crate::echeance_dsn::de_la_compagnie(ctx, mois).and_then(|e| e.veille()).map_err(refus_valeur)?;
    Ok(ctx.aujourdhui > limite)
}

/// Une correction due d'un mois (`corrections_dues`) : l'engagement valide, son rang dans l'ecran du mois, et ce qui a
/// ete declare pour lui (vide : rien).
pub struct Due {
    pub valide: engagement::etats::Valide,
    pub rang: usize,
    pub declarees: Vec<engagement::etats::Declaree>,
}

/// LES CORRECTIONS DUES DE `mois` dans la DSN de `porteur` : chaque engagement valide du mois dont la paie presente n'est
/// plus celle que les DSN anterieures a `porteur` ont declaree, ou qu'aucune n'a declare (`engagement::etats::
/// correction_due`). Corriger, c'est modifier l'engagement et le revalider : rien ne s'ecrit a cote de lui, et un
/// engagement en saisie n'est pas encore une correction (ce qui n'est pas valide n'est pas paye).
pub fn corrections_dues(ctx: &Contexte, mois: &str, porteur: &str, departs: &engagement::departs::Departs) -> R<Vec<Due>> {
    let refus = |m: String| erreur("MoisNonRegularisable", m);
    let mut out = Vec::new();
    for (rang, e) in engagement::etats::du_mois(ctx, mois)?.into_iter().enumerate() {
        let Some(v) = e.valide().cloned() else { continue };
        if let Some(declarees) = engagement::etats::correction_due(ctx, &v, departs, mois, porteur).map_err(refus)? {
            out.push(Due { valide: v, rang, declarees });
        }
    }
    Ok(out)
}

/// `regularisations_pour` : les corrections que la DSN de `mois_courant` porte, de chaque mois anterieur qui se corrige
/// ailleurs que dans sa DSN (`se_corrige_ailleurs`), chacune avec le versement DECLARE (`versement_declare::dernier_declare`), sa paie
/// d'avant (telle que declaree, ou telle que la correction portee precedente l'a laissee) et sa paie d'apres (la paie
/// ecrite de la version presente).
pub fn regularisations_pour(ctx: &Contexte, mois_courant: &str) -> R<Vec<Reg>> {
    let departs = engagement::departs::lire(&ctx.racine)?;
    let mut out = Vec::new();
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(refus_valeur)? {
        if mois.as_str() >= mois_courant {
            break;
        }
        if !se_corrige_ailleurs(ctx, &departs, &mois)? {
            continue;
        }
        // Un engagement qui n'est plus actif et que la DSN de ce mois a declare : sa correction annule tout ce qui a ete
        // declare (cahier technique DSN 2027.1, §2.1.2.1, `norme-dsn/texte-2027/chapitre-2-principes-de-constitution.txt` :
        // « une correction ... donne lieu a la declaration d'une correction dans la DSN suivante »). Elle ne s'ecrit pas
        // encore : la DSN refuse plutot que de taire ce qui est a corriger (regle 1).
        for n in engagement::etats::non_actifs_du_mois(ctx, &mois).map_err(refus_valeur)? {
            if n.a_refaire(ctx, &departs).iter().any(|a| matches!(a, engagement::etats::ARefaire::Dsn { mois: m, .. } if *m == mois)) {
                return Err(erreur(
                    "MoisNonRegularisable",
                    format!(
                        "{} : l'engagement de {} (contrat {}) n'est plus actif, et la DSN de ce mois l'a declare. Ce qui a ete declare pour lui est a annuler dans la DSN de {}, et cette correction ne s'ecrit pas encore. La DSN de {} ne se produit pas tant que ce n'est pas regle.",
                        engagement::libelle_mois(&mois),
                        valeur_en_texte(n.objet().brut.get("salarie").unwrap_or(&Value::Null)),
                        valeur_en_texte(n.objet().brut.get("numero").unwrap_or(&Value::Null)),
                        mois_courant,
                        mois_courant
                    ),
                ));
            }
        }
        let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, &mois)? else { continue };
        for d in corrections_dues(ctx, &mois, mois_courant, &departs)? {
            let apres = d.valide.paie(ctx, &data, d.rang)?;
            let version = d.valide.version().unwrap_or_default();
            let decidee_le = d.valide.le().get(..10).and_then(|t| Jour::lire(t).ok());
            if d.declarees.is_empty() {
                out.push(ajout(mois.clone(), version, decidee_le, apres)?);
                continue;
            }
            let numero = apres.engagement.numero.clone().unwrap_or_default();
            let refus = |quoi: &str| {
                erreur(
                    "MoisNonRegularisable",
                    format!(
                        "Correction de {} pour {} (contrat {}) : {}. Une regularisation annule ce qui a ete DECLARE : elle le lit dans la version de l'engagement sur laquelle la DSN est partie, elle ne le recalcule pas. La DSN de {} ne se produit pas tant que ce n'est pas regle.",
                        mois,
                        valeur_en_texte(apres.salarie.get("nom").unwrap_or(&Value::Null)),
                        if numero.is_empty() { "sans numero" } else { numero.as_str() },
                        quoi,
                        mois_courant
                    ),
                )
            };
            if numero.is_empty() {
                return Err(refus("le numero sous lequel le contrat a ete declare (S21.G00.40.009) n'est pas connu"));
            }
            let lu = versement_declare::dernier_declare(ctx, d.valide.objet(), &departs, &mois, mois_courant)
                .map_err(|e| refus(&e))?
                .ok_or_else(|| refus(&format!("aucune DSN partie (journal/depots.json) n'a declare ce contrat pour {}", mois)))?;
            // Le taux du bloc 56 : celui de la paie declaree du mois d'origine.
            let taux_pas_origine = lu.versement.taux_pas.clone();
            out.push(Reg {
                mois: mois.clone(),
                version,
                decidee_le,
                ajout: false,
                avant: lu.avant,
                bulletin_rectifie: lu.bulletin,
                declare: lu.versement,
                net_deja_verse: lu.net_deja_verse,
                apres,
                taux_pas_origine,
            });
        }
    }
    Ok(out)
}

/// LES VERSIONS QUE PORTENT LES CORRECTIONS de la DSN de `mois` pour l'etablissement `nic` : pour chaque correction
/// qu'elle porte (`regularisations_pour`), la version presente de l'engagement du mois corrige, `{"salarie", "numero",
/// "version", "mois": <le mois corrige>}`. Ce que la ligne du journal de son depot emporte.
pub fn versions_portees(ctx: &Contexte, mois: &str, nic: &str) -> R<Vec<Value>> {
    let fiche = &ctx.entreprise;
    let plusieurs = !droit::bareme::secondaires(fiche).is_empty();
    let mut out = Vec::new();
    for r in regularisations_pour(ctx, mois)? {
        if plusieurs && droit::bareme::etablissement_du_salarie(fiche, &r.apres.salarie).map_err(refus_valeur)? != nic {
            continue;
        }
        let v = json!({
            "salarie": valeur_en_texte(r.apres.salarie.get("_cle").unwrap_or(&Value::Null)),
            "numero": r.apres.engagement.numero.clone().unwrap_or_default(),
            "version": r.version,
            "mois": r.mois,
        });
        if !out.contains(&v) {
            out.push(v);
        }
    }
    Ok(out)
}

/// LA CORRECTION D'UN CONTRAT AJOUTE APRES LA LIMITE (un contrat oublie) : rien n'a ete declare pour lui -- la DSN
/// deposee du mois ne le porte pas --, et rien n'a ete verse -- aucun bulletin de ce mois ne le porte. Le declare est
/// donc vide et le net deja verse nul : la DSN suivante declare le contrat en entier, ses montants rattaches au mois
/// d'origine (cahier technique DSN 2026.1, §2.1.2.1 : « une correction est un evenement connu apres la date de cloture
/// de la paie. Il donne lieu a la declaration d'une correction dans la DSN suivante », et §2.2.2.5 ; fiche
/// net-entreprises 1741 : un contrat que le systeme DSN ne connait pas encore se declare en entier dans la DSN mensuelle
/// qui le porte), et le versement de la correction paie son net.
fn ajout(mois: String, version: u64, decidee_le: Option<Jour>, apres: Paie) -> R<Reg> {
    if !apres.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) {
        return Err(erreur(
            "MoisNonRegularisable",
            format!("{} : le contrat ajoute apres la DSN de ce mois n'a pas de numero (S21.G00.40.009). Le revalider.", mois),
        ));
    }
    let declare = VersementDeclare {
        fichier: String::new(),
        net_imposable: Decimal::ZERO,
        net_verse: Decimal::ZERO,
        net_social: Decimal::ZERO,
        rappels_net_social: Decimal::ZERO,
        frais: Decimal::ZERO,
        brut: Decimal::ZERO,
        lignes: Vec::new(),
        affiliations: Vec::new(),
        taux_pas: None,
        debut_contrat: None,
    };
    let bulletin_rectifie = apres.numero_bulletin.clone();
    Ok(Reg { mois, version, decidee_le, ajout: true, avant: apres.clone(), declare, net_deja_verse: Decimal::ZERO, apres, taux_pas_origine: None, bulletin_rectifie })
}

fn prenom_de(sal: &Value) -> String {
    match sal.get("prenom_affichage") {
        Some(v) if vrai(v) => valeur_en_texte(v),
        _ => valeur_en_texte(sal.get("prenom").unwrap_or(&Value::Null)),
    }
}

/// `resume_regularisations` : une ligne lisible par correction embarquee.
pub fn resume_regularisations(regs: &[Reg]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for r in regs {
        let s = &r.apres.salarie;
        let nom = s.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
        out.push(format!(
            "{} {}, contrat {} ({}) : brut {} -> {}, net {} -> {}",
            valeur_en_texte(nom),
            prenom_de(s),
            r.apres.engagement.numero.as_deref().unwrap_or("None"),
            engagement::libelle_mois(&r.mois),
            q2(r.declare.brut),
            q2(r.apres.brut),
            q2(r.net_deja_verse),
            q2(r.apres.net_a_payer),
        ));
    }
    Ok(out)
}

/// Les regularisations de la DSN (`dsn::Regularisation`) : la paie declaree et la paie corrigee.
pub fn pour_la_dsn(regs: &[&Reg]) -> Vec<crate::dsn::Regularisation> {
    regs.iter()
        .map(|r| crate::dsn::Regularisation {
            mois: r.mois.clone(),
            avant: r.avant.clone(),
            declare: r.declare.clone(),
            apres: r.apres.clone(),
            taux_pas_origine: r.taux_pas_origine.clone(),
            net_deja_verse: r.net_deja_verse,
            decidee_le: r.decidee_le,
        })
        .collect()
}

fn refus_valeur(m: String) -> Echec {
    erreur("ValueError", m)
}


/// `versement_de_la_correction` : ce que la correction verse -- ou retient -- CE mois-ci : la difference de net,
/// et le prelevement a la source sur cette difference, tel que l'engagement le rend
/// (`engagement::etats::prelevement_de_la_correction`, le meme que le bloc 50 de la DSN).
pub fn versement_de_la_correction(ctx: &Contexte, r: &Reg, mois_courant: &str) -> R<Value> {
    // Ce qui a ete verse et declare se lit dans la paie declaree, pas dans un recalcul : le net imposable en 50.002, le
    // net deja verse.
    let apres = &r.apres;
    let net_avant = r.net_deja_verse;
    let d_net = apres.net_a_payer - net_avant;
    let pas_c = engagement::etats::prelevement_de_la_correction(
        ctx,
        apres,
        r.declare.net_imposable,
        r.taux_pas_origine.as_deref(),
        mois_courant,
    )
    .map_err(refus_valeur)?;
    let (d_ni, indu, prelevement) = (pas_c.difference, pas_c.indu(), pas_c.prelevement);
    // Le taux que le bulletin montre : celui du versement, ou, pour un trop-verse, le taux d'origine.
    let taux = match (indu, r.taux_pas_origine.as_deref().filter(|t| !t.is_empty())) {
        (false, _) => pas_c.versement.taux.clone(),
        (true, Some(t)) => json!(t),
        (true, None) => Value::Null,
    };
    let a_verser = match prelevement {
        Some(p) => d_net - p,
        None => d_net,
    };
    Ok(json!({
        "net_avant": net_avant.to_string(),
        "net_apres": apres.net_a_payer.to_string(),
        "difference_net": d_net.to_string(),
        "difference_net_imposable": d_ni.to_string(),
        "indu": indu,
        "taux": taux,
        "prelevement": prelevement.map_or(Value::Null, |p| json!(p.to_string())),
        "a_verser": a_verser.to_string(),
        "mois_versement": engagement::libelle_mois(mois_courant),
        "mois_origine": engagement::libelle_mois(&r.mois),
    }))
}

/// `numero_du_bulletin_rectifie` : le numero du bulletin d'origine, tel que les paies declarees le portent (`Reg::bulletin_rectifie`).
/// `None` s'il n'y est pas : l'appelant le dit, il ne le devine pas.
fn numero_du_bulletin_rectifie(r: &Reg) -> Option<String> {
    r.bulletin_rectifie.clone().filter(|n| !n.is_empty())
}

/// Ce que `ecrire_rectificatifs` rend : les fichiers ecrits, les avertissements (de simples phrases).
pub struct Rectificatifs {
    pub fichiers: Vec<PathBuf>,
    pub avertissements: Vec<String>,
}

fn ecrire_le_pdf(description: &Value, chemin: &Path) -> R<()> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur("OSError", format!("{} : {}", parent.display(), e)))?;
    }
    documents::fabrique::ecrire_pdf(description, chemin)
}

/// `ecrire_rectificatifs` : les bulletins rectificatifs des corrections que la DSN de `mois_courant` embarque.
///
/// Les numeros se RESERVENT (`reserves`, comme `ctx.enregistrer_compteur`) dans la suite de l'annee du mois
/// courant, apres ceux des bulletins du mois : ils ne s'ecrivent qu'a la validation, une fois le mois entier
/// produit. Deux fichiers par correction, jamais a la place d'un autre :
/// `sorties/<mois d'origine>/Rectificatif_<cle>_<Mois>_v<version>.pdf`, a cote du bulletin rectifie ; et, pour
/// l'envoi, un fichier par salarie dans `sorties/<mois courant>/envoi/`.
pub fn ecrire_rectificatifs(
    ctx: &Contexte,
    regs: &[Reg],
    mois_courant: &str,
    sortie_courante: &Path,
    reserves: &mut documents::ecrire_documents::Reserves,
) -> R<Rectificatifs> {
    let mut out = Rectificatifs { fichiers: Vec::new(), avertissements: Vec::new() };
    let payes: Vec<&Reg> = regs.iter().filter(|r| !r.apres.engagement.sans_paie()).collect();
    for r in regs {
        if r.apres.engagement.sans_paie() {
            out.avertissements.push(format!(
                "{}, contrat {} ({}) : la correction retire toute la paie -- pas de bulletin rectificatif a imprimer ; le net deja verse ({}) se retient, et le salarie doit en etre prevenu par ecrit.",
                valeur_en_texte(r.apres.salarie.get("nom").unwrap_or(&Value::Null)),
                r.apres.engagement.numero.as_deref().unwrap_or("None"),
                engagement::libelle_mois(&r.mois),
                q2(r.net_deja_verse)
            ));
        }
    }
    if payes.is_empty() {
        return Ok(out);
    }
    let an: i64 = mois_courant.get(..4).and_then(|t| t.parse().ok()).ok_or_else(|| refus_valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois_courant))))?;
    let mut n = reserves.prochain(ctx, "bulletin", an)?;
    // Par salarie, dans l'ordre ou on les rencontre : (cle, [(paie imprimee, numero de son bulletin, rectificatif, ent)]).
    let mut par_salarie: Vec<(String, Vec<(&Paie, String, Value, Value)>)> = Vec::new();
    for r in &payes {
        let numero_bulletin = format!("{}/{}", n, an);
        n += 1;
        let mut rectif = versement_de_la_correction(ctx, r, mois_courant)?;
        let rectifie = numero_du_bulletin_rectifie(r);
        rectif["rectifie"] = rectifie.clone().map_or(Value::Null, Value::String);
        let contrat = r.apres.engagement.numero.as_deref().unwrap_or("None");
        // Un contrat ajoute n'a pas de bulletin d'origine : son bulletin est celui-ci.
        if rectifie.is_none() && !r.ajout {
            out.avertissements.push(format!(
                "Bulletin rectificatif {} ({}, contrat {}) : la paie declaree ne porte pas le numero du bulletin d'origine de {} -- le rectificatif cite le contrat a la place.",
                numero_bulletin,
                engagement::libelle_mois(&r.mois),
                contrat,
                r.mois
            ));
        }
        if rectif["prelevement"].is_null() {
            out.avertissements.push(format!(
                "Bulletin rectificatif {} : le taux de prelevement a la source applique au trop-verse est introuvable dans la paie declaree -- le bulletin ne montre pas de prelevement sur la difference.",
                numero_bulletin
            ));
        }
        let ent = ctx.entreprise_du_mois(&r.mois).map_err(Echec::from)?;
        let cle = valeur_en_texte(r.apres.salarie.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        let etiq = socle::date::etiquette_du_mois(&r.mois).map_err(refus_valeur)?;
        let archive = ctx.racine.join("sorties").join(&r.mois).join(format!("{}_{}_{}_v{}.pdf", PREFIXE_ARCHIVE, cle, etiq, r.version));
        let rectifs: HashMap<usize, Value> = HashMap::from([(0, rectif.clone())]);
        let numeros = [Some(numero_bulletin.clone())];
        let description = documents::bulletin::jeux_et_description(ctx, std::slice::from_ref(&r.apres), &numeros, &ent, "", None, false, &rectifs).map_err(refus_valeur)?;
        ecrire_le_pdf(&description, &archive)?;
        out.fichiers.push(archive);
        match par_salarie.iter_mut().find(|(c, _)| *c == cle) {
            Some((_, v)) => v.push((&r.apres, numero_bulletin, rectif, ent)),
            None => par_salarie.push((cle, vec![(&r.apres, numero_bulletin, rectif, ent)])),
        }
    }
    let dossier = sortie_courante.join("envoi");
    let etiq = socle::date::etiquette_du_mois(mois_courant).map_err(refus_valeur)?;
    for (cle, siens) in &par_salarie {
        // Un fichier par salarie, tous ses rectificatifs dedans, comme ses bulletins du mois. Un seul `ent` par
        // fichier : la licence du premier mois d'origine (les archives, elles, ont chacune la leur).
        let chemin = dossier.join(format!("{}_{}_{}.pdf", PREFIXE_ENVOI, cle, etiq));
        let paies: Vec<Paie> = siens.iter().map(|(p, _, _, _)| (*p).clone()).collect();
        let numeros: Vec<Option<String>> = siens.iter().map(|(_, n, _, _)| Some(n.clone())).collect();
        let rectifs: HashMap<usize, Value> = siens.iter().enumerate().map(|(i, (_, _, x, _))| (i, x.clone())).collect();
        let description = documents::bulletin::jeux_et_description(ctx, &paies, &numeros, &siens[0].3, "", None, false, &rectifs).map_err(refus_valeur)?;
        ecrire_le_pdf(&description, &chemin)?;
    }
    reserves.enregistrer(an, "bulletin", n - 1);
    Ok(out)
}
