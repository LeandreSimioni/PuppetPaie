//! Les corrections des mois deja deposes, telles que la DSN d'un mois suivant les embarque : `corrections_dues`,
//! `regularisations_pour`, `resume_regularisations`.
//!
//! Corriger, c'est modifier l'engagement et le revalider. Ce qui est a corriger se tire de la comparaison entre la
//! version presente de chaque engagement valide et celle sur laquelle les DSN parties l'ont declare
//! (`engagement::etats::correction_due`), pour un mois dont l'annule-et-remplace n'est plus admis
//! (`se_corrige_ailleurs`). La paie d'avant (`Reg::avant`) est la derniere paie declaree du contrat, elle ne se recalcule
//! pas (regle 4) ; la paie d'apres est la paie ecrite de la version presente. La DSN declare ces deux paies telles
//! quelles ; elle ne fabrique aucun bulletin : celui de la paie corrigee part avec les documents de son mois.


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


/// Une correction embarquee : un contrat d'un mois deja declare dont la paie validee presente n'est plus celle que les
/// DSN parties ont declaree, et les deux paies.
pub struct Reg {
    pub mois: String,
    /// La version presente de l'engagement, que la correction declare, et le jour ou elle a ete validee.
    pub version: u64,
    pub decidee_le: Option<Jour>,
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
}

/// UN MOIS SE CORRIGE DANS UNE DSN SUIVANTE quand sa DSN reelle est partie (un depart DSN du mois lui-meme, `departs`)
/// et que son annule-et-remplace n'est plus admis (la veille de l'echeance passee, cahier technique DSN §1.4.2) ; tant qu'il l'est,
/// sa propre DSN se refait. Un mois dont la DSN est celle de l'ancien logiciel (`engagement::importe::dsn_deposee`) se
/// corrige toujours ailleurs : sa DSN n'a pas ete faite ici et ne se refait pas ici (regle 4).
pub fn se_corrige_ailleurs(ctx: &Contexte, departs: &engagement::departs::Departs, mois: &str) -> R<bool> {
    use engagement::departs::Envoi;
    if engagement::importe::dsn_deposee(&ctx.racine, mois).map_err(refus_valeur)?.is_some() {
        return Ok(true);
    }
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
    for (rang, e) in engagement::etats::du_mois(ctx, mois, departs)?.into_iter().enumerate() {
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
pub fn regularisations_pour(ctx: &Contexte, mois_courant: &str, departs: &engagement::departs::Departs) -> R<Vec<Reg>> {
    let mut out = Vec::new();
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(refus_valeur)? {
        if mois.as_str() >= mois_courant {
            break;
        }
        if !se_corrige_ailleurs(ctx, departs, &mois)? {
            continue;
        }
        out.extend(annulations(ctx, departs, &mois, mois_courant)?);
        let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, &mois)? else { continue };
        for d in corrections_dues(ctx, &mois, mois_courant, departs)? {
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
            let lu = versement_declare::dernier_declare(ctx, d.valide.objet(), departs, &mois, mois_courant)
                .map_err(|e| refus(&e))?
                .ok_or_else(|| refus(&format!("aucune DSN partie (journal/depots.json) n'a declare ce contrat pour {}", mois)))?;
            // Le taux du bloc 56 : celui de la paie declaree du mois d'origine.
            let taux_pas_origine = lu.versement.taux_pas.clone();
            out.push(Reg {
                mois: mois.clone(),
                version,
                decidee_le,
                avant: lu.avant,
                declare: lu.versement,
                net_deja_verse: lu.net_deja_verse,
                apres,
                taux_pas_origine,
            });
        }
    }
    Ok(out)
}

/// LES ANNULATIONS DE `mois` que la DSN de `mois_courant` porte : chaque engagement non actif que la derniere DSN qui a
/// declare ce mois a declare actif (`NonActif::correction_due`). Ce qui a ete declare pour lui, lu dans la version sur
/// laquelle elle est partie (`versement_declare::dernier_declare`, regle 4), s'annule en entier, et rien ne le remplace :
/// la paie d'apres est la meme, annulee (`Paie::annulee`), et le redacteur n'ecrit d'elle que l'annulation. Cahier
/// technique DSN 2027.1, `atelier/reference/norme-dsn/texte-2027/chapitre-2-principes-de-constitution.txt` : §2.1.2.1
/// (p. 19, « une correction est un evenement connu apres la date de cloture de la paie. Il donne lieu a la declaration
/// d'une correction dans la DSN suivante ») et §2.1.3.6 (p. 30, l'approche « par annulation et remplacement » : une
/// « annulation de la declaration initiale accompagnee si opportun d'une declaration corrective » ; ici, aucune).
fn annulations(ctx: &Contexte, departs: &engagement::departs::Departs, mois: &str, mois_courant: &str) -> R<Vec<Reg>> {
    let mut out = Vec::new();
    for n in engagement::etats::non_actifs_du_mois(ctx, mois).map_err(refus_valeur)? {
        if n.correction_due(ctx, departs, mois, mois_courant).map_err(refus_valeur)?.is_none() {
            continue;
        }
        let lu = versement_declare::dernier_declare(ctx, n.objet(), departs, mois, mois_courant)
            .map_err(refus_valeur)?
            .ok_or_else(|| refus_valeur(format!("{} : aucune DSN partie n'a declare le contrat {} de ce mois", mois, valeur_en_texte(n.objet().brut.get("numero").unwrap_or(&Value::Null)))))?;
        let apres = lu.avant.annulee();
        out.push(Reg {
            mois: mois.to_string(),
            version: n.version(),
            decidee_le: n.le().get(..10).and_then(|t| Jour::lire(t).ok()),
            taux_pas_origine: lu.versement.taux_pas.clone(),
            avant: lu.avant,
            declare: lu.versement,
            net_deja_verse: lu.net_deja_verse,
            apres,
        });
    }
    Ok(out)
}

/// LES VERSIONS QUE PORTENT LES CORRECTIONS de la DSN de `mois` pour l'etablissement `nic` : pour chaque correction
/// qu'elle porte (`regularisations_pour`), la version presente de l'engagement du mois corrige, `{"salarie", "numero",
/// "version", "mois": <le mois corrige>}`. Ce que la ligne du journal de son depot emporte.
pub fn versions_portees(ctx: &Contexte, mois: &str, nic: &str, departs: &engagement::departs::Departs) -> R<Vec<Value>> {
    let fiche = &ctx.entreprise;
    let plusieurs = !droit::bareme::secondaires(fiche).is_empty();
    let mut out = Vec::new();
    for r in regularisations_pour(ctx, mois, departs)? {
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
/// deposee du mois ne le porte pas. Le declare est vide et le net deja declare verse nul : la DSN suivante declare le
/// contrat en entier, ses montants rattaches au mois
/// d'origine (cahier technique DSN 2026.1, §2.1.2.1 : « une correction est un evenement connu apres la date de cloture
/// de la paie. Il donne lieu a la declaration d'une correction dans la DSN suivante », et §2.2.2.5 ; fiche
/// net-entreprises 1741 : un contrat que le systeme DSN ne connait pas encore se declare en entier dans la DSN mensuelle
/// qui le porte), et le versement de la correction declare son net. Son bulletin est celui de son mois, qui part avec
/// les documents de ce mois (`moteur::compagnie::depart::fabriquer_documents`).
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
    Ok(Reg { mois, version, decidee_le, avant: apres.clone(), declare, net_deja_verse: Decimal::ZERO, apres, taux_pas_origine: None })
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
            "{} {}, contrat {} ({}) : brut {}\u{a0}€ → {}\u{a0}€, net {}\u{a0}€ → {}\u{a0}€",
            valeur_en_texte(nom),
            prenom_de(s),
            r.apres.engagement.numero.as_deref().unwrap_or("None"),
            engagement::libelle_mois(&r.mois),
            q2(r.declare.brut).replace('.', ","),
            q2(r.apres.brut).replace('.', ","),
            q2(r.net_deja_verse).replace('.', ","),
            q2(r.apres.net_a_payer).replace('.', ","),
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


