//! Les corrections des mois deja deposes, telles que `produire` les embarque (lot H2) :
//! `operations_regularisation.corrections_a_porter`, `regularisations_pour`,
//! `resume_regularisations`, `versement_de_la_correction`, `numero_du_bulletin_rectifie` et
//! `ecrire_rectificatifs`.
//!
//! ECRIT, NON PROUVE. Ce qui se calcule vient des fonctions deja portees et prouvees de
//! `routes_regularisation` (`engagement_declare`, `paie_d_un_engagement`, `dsn_deposees`) ; ce module ajoute
//! la selection des corrections a porter, le versement de la difference de net, et les bulletins
//! rectificatifs.
//!
//! DEUX PAIES PAR COTE, COMME LE PYTHON. `avant` et `apres` sortent du calcul (`paie_de_json`) : ce sont elles
//! que les phrases, le versement et le numero lisent. Ce que la DSN et le bulletin IMPRIMENT, le Python le
//! faisait recalculer par le moteur depuis la saisie (`paie_rust.dsn`, `paie_rust.bulletin`) : `a_imprimer`
//! refait de meme (`charger_donnees`), puis pose les numeros de la paie du calcul.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{jours_du_mois, Jour};
use socle::valeur::valeur_en_texte;
use socle::echec::R;
use socle::echec::erreur;
use socle::fichiers::vrai;
use socle::num::{dec_avec_echelle, pct, Decimal};
use droit::pas::{self, ErreurPas, ZONE_DEFAUT};
use socle::echec::Echec;
use crate::routes_mois::Moteur;
use crate::routes_regularisation as regul;
use crate::versement_declare::{self, VersementDeclare};

/// Le prefixe de l'archive d'un bulletin rectificatif, a cote du bulletin rectifie.
const PREFIXE_ARCHIVE: &str = "Rectificatif";
/// Le prefixe du fichier par salarie, pour l'envoi (`envoi.PREFIXES["rectificatif"]`).
const PREFIXE_ENVOI: &str = "BulletinRectificatif";

/// Une correction embarquee : la correction du disque, et les deux paies (`Regularisation`).
pub(crate) struct Reg {
    pub(crate) correction: Map<String, Value>,
    pub(crate) mois: String,
    pub(crate) numero: i64,
    /// La paie d'avant refaite par le moteur du jour : elle ne donne plus que le contrat (bornes, nature, statut, pile de
    /// bareme), aucun montant calcule. Ce que la correction annule, au nominatif comme a l'agrege, se lit dans
    /// `declare` ; ce qui a ete paye, dans `net_deja_verse`.
    pub(crate) avant: Paie,
    /// Le dernier etat declare du contrat : le versement de la paie declaree, celle de la DSN du mois d'origine ou de la
    /// derniere correction deja portee (`versement_declare_de`).
    pub(crate) declare: VersementDeclare,
    /// Le net a payer avant impot deja verse au salarie pour le contrat, LU sur le bulletin qu'il a recu : celui du mois
    /// d'origine, ou le rectificatif de la derniere correction deja portee (`net_deja_verse_lu`).
    pub(crate) net_deja_verse: Decimal,
    pub(crate) apres: Paie,
    /// La saisie du mois d'origine, et les deux engagements (declare, corrige) : ce qui permet de refaire
    /// les paies a imprimer.
    pub(crate) donnees: Value,
    pub(crate) declare_saisi: Value,
    pub(crate) corrige: Value,
    pub(crate) taux_pas_origine: Option<String>,
}

fn en_texte_ou_vide(v: Option<&Value>) -> String {
    v.filter(|x| vrai(x)).map(valeur_en_texte).unwrap_or_default()
}

/// `corrections_a_porter` : celles en attente, plus celles que ce meme mois a deja portees (un
/// annule-et-remplace de ce mois les reprend, un mois suivant non). Jamais une correction d'un mois d'origine
/// posterieur ou egal au mois courant.
pub(crate) fn corrections_a_porter(ctx: &Contexte, mois_courant: &str) -> R<Vec<Map<String, Value>>> {
    let mut out = Vec::new();
    for c in crate::routes_ecrites::toutes_les_corrections(ctx)? {
        if c.get("annulee_le").is_some_and(vrai) || valeur_en_texte(c.get("mois").unwrap_or(&Value::Null)).as_str() >= mois_courant {
            continue;
        }
        let portee = c.get("portee_par").filter(|p| vrai(p));
        if portee.is_none_or(|p| p.get("mois").and_then(Value::as_str) == Some(mois_courant)) {
            out.push(c);
        }
    }
    Ok(out)
}

/// `regularisations_pour` : les corrections de la DSN de `mois_courant`, chacune avec le versement DECLARE (lu dans les
/// versions de l'engagement du mois d'origine, `versement_declare_de`), sa paie d'avant refaite (telle que
/// declaree, ou telle que la correction portee precedente l'a laissee) et sa paie d'apres.
pub(crate) fn regularisations_pour(ctx: &Contexte, m: &mut Moteur, mois_courant: &str) -> R<Vec<Reg>> {
    let mut out = Vec::new();
    for c in corrections_a_porter(ctx, mois_courant)? {
        let get = |k: &str| c.get(k).cloned().ok_or_else(|| erreur("KeyError", socle::fichiers::absent(k)));
        let (mois, salarie, debut, corrige) = (valeur_en_texte(&get("mois")?), get("salarie")?, get("debut")?, get("engagement")?);
        let numero = c.get("numero").and_then(Value::as_i64).ok_or_else(|| erreur("KeyError", socle::fichiers::absent("numero")))?;
        if c.get("ajout").is_some_and(vrai) {
            out.push(ajout(ctx, m, c.clone(), mois, numero, corrige)?);
            continue;
        }
        let (donnees, declare, _fiche) = regul::engagement_declare(m, ctx, &mois, &salarie, &debut)?;
        let avant = regul::paie_d_un_engagement(m, ctx, &donnees, &declare)?;
        let apres = regul::paie_d_un_engagement(m, ctx, &donnees, &corrige)?;
        let (lu, derniere) = versement_declare_de(ctx, &c, &mois, mois_courant, &apres)?;
        // Le taux du bloc 56 : le 50.006 du versement relu (reste R1 #3), pas celui d'un fichier quelconque du mois.
        let taux_pas_origine = lu.taux_pas.clone();
        let net_deja_verse = net_deja_verse_lu(ctx, &mois, mois_courant, &avant, &apres, derniere.as_ref())?;
        out.push(Reg { correction: c, mois, numero, avant, declare: lu, net_deja_verse, apres, donnees, declare_saisi: declare, corrige, taux_pas_origine });
    }
    Ok(out)
}

/// LA CORRECTION D'UN CONTRAT AJOUTE APRES LA LIMITE (un contrat oublie, 21/10/2026) : rien n'a ete declare pour lui --
/// la DSN deposee du mois ne le porte pas --, et rien n'a ete verse par ce logiciel -- le bulletin archive du mois ne le
/// porte pas non plus (le contrat n'existait pas quand il a ete ecrit). Le declare est donc vide et le net deja verse
/// nul : la DSN suivante declare le contrat en entier, ses montants rattaches au mois d'origine (cahier technique DSN
/// 2026.1, §2.1.2.1 et §2.2.2.5 ; fiche net-entreprises 1741 : `routes_regularisation::apercu_d_un_ajout`), et le
/// versement de la correction paie son net. Sa paie se calcule depuis la saisie du mois telle que declaree, son
/// engagement a la place des autres, comme toute correction.
fn ajout(ctx: &Contexte, m: &mut Moteur, c: Map<String, Value>, mois: String, numero: i64, corrige: Value) -> R<Reg> {
    let donnees = regul::saisie_d_origine(ctx, &mois)?;
    let apres = regul::paie_d_un_engagement(m, ctx, &donnees, &corrige)?;
    if !apres.engagement.numero.as_deref().is_some_and(|n| !n.is_empty()) {
        return Err(erreur(
            "MoisNonRegularisable",
            format!("Correction n° {} de {} : le contrat ajoute n'a pas de numero (S21.G00.40.009). Valider l'engagement, puis reecrire la correction.", numero, mois),
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
    Ok(Reg {
        correction: c,
        mois,
        numero,
        avant: apres.clone(),
        declare,
        net_deja_verse: Decimal::ZERO,
        apres,
        donnees,
        declare_saisi: corrige.clone(),
        corrige,
        taux_pas_origine: None,
    })
}

/// Le versement declare pour le contrat corrige, dans son DERNIER etat : ce que l'annulation retire (norme § 2.1.3.6,
/// p. 30 et 49). Il se LIT dans les versions de l'engagement (regle 4) : la version sur laquelle la DSN du mois d'origine
/// est partie, puis celle de chaque correction de ce contrat qu'une DSN d'un mois anterieur au mois courant a portee
/// (`engagement::etats::paies_declarees`) ; le versement de la derniere s'ecrit comme la mensuelle l'ecrit
/// (`versement_declare::de_la_paie`). Rien de declare pour ce contrat : la regularisation ne se produit pas.
///
/// Le montant net social (vague 1o, `dsn::blocs_58_de_la_correction`) : une correction qui a change le net paye en a
/// declare la difference aux dates du mois de son versement (le rappel), et laisse le montant d'origine ; une correction
/// qui ne l'a pas change a redeclare le montant d'origine. Les rappels deja declares se tirent donc de la suite des
/// paies declarees, et le montant d'origine est le dernier montant net social declare moins eux.
///
/// Rend aussi la derniere de ces corrections deja portees, (mois qui l'a portee, numero), s'il y en a une : son
/// bulletin rectificatif dit ce qui a ete paye depuis (`net_deja_verse_lu`).
fn versement_declare_de(ctx: &Contexte, c: &Map<String, Value>, mois: &str, mois_courant: &str, apres: &Paie) -> R<(VersementDeclare, Option<(String, String)>)> {
    let nom = valeur_en_texte(apres.salarie.get("nom").unwrap_or(&Value::Null));
    let numero = apres.engagement.numero.clone().unwrap_or_default();
    let refus = |quoi: String| {
        erreur(
            "MoisNonRegularisable",
            format!(
                "Correction de {} pour {} (contrat {}) : {}. Une regularisation annule ce qui a ete DECLARE : elle le lit dans la version de l'engagement sur laquelle la DSN est partie, elle ne le recalcule pas. La DSN de {} ne se produit pas tant que ce n'est pas regle.",
                mois, nom, if numero.is_empty() { "sans numero" } else { numero.as_str() }, quoi, mois_courant
            ),
        )
    };
    if numero.is_empty() {
        return Err(refus("le numero sous lequel le contrat a ete declare (S21.G00.40.009) n'est pas connu".into()));
    }
    let cle = c.get("salarie").map(valeur_en_texte).unwrap_or_default();
    let departs = crate::journal::departs(&ctx.racine)?;
    let objet = engagement::stock::objets_du_mois(&ctx.racine, mois)?.into_iter().find(|o| {
        let b = Value::Object(o.brut.clone());
        engagement::cle_de_l_engagement(ctx, &b).as_deref() == Some(cle.as_str()) && b.get("numero").map(valeur_en_texte).as_deref() == Some(numero.as_str())
    });
    let Some(objet) = objet else { return Err(refus(format!("aucun engagement de {} ne porte ce contrat", mois))) };
    let declarees = engagement::etats::paies_declarees(ctx, &objet, &departs, mois, mois_courant).map_err(refus)?;
    let (Some(origine), Some(derniere_paie)) = (declarees.first(), declarees.last()) else {
        return Err(refus(format!("aucune DSN partie (journal/depots.json) n'a declare ce contrat pour {}", mois)));
    };
    let mut etat = versement_declare::de_la_paie(ctx, &derniere_paie.paie, mois).map_err(refus)?;
    let mut rappels = Decimal::ZERO;
    for paire in declarees.windows(2) {
        let (avant, puis) = (&paire[0].paie, &paire[1].paie);
        if avant.net_a_payer != puis.net_a_payer {
            rappels += puis.net_social - avant.net_social;
        }
    }
    if !rappels.is_zero() {
        let origine_58 = etat.net_social - rappels;
        let mut type_03 = false;
        for (k, v) in etat.lignes.iter_mut() {
            match k.as_str() {
                "S21.G00.58.003" => type_03 = v == "03",
                "S21.G00.58.004" if type_03 => *v = crate::dsn::mt(origine_58),
                _ => {}
            }
        }
        etat.net_social = origine_58;
        etat.rappels_net_social = rappels;
    }
    etat.taux_pas = origine.paie.pas.as_ref().map(|p| valeur_en_texte(&p.taux)).filter(|t| !t.is_empty());
    if let Some(d) = engagement::etats::debut_declare(ctx, &objet, &departs, mois_courant) {
        etat.debut_contrat = Some(Jour::lire(&d).map_err(|e| refus(e))?.jjmmaaaa());
    }
    // La derniere correction deja portee : (mois de la DSN qui l'a portee, son numero dans les corrections du mois).
    let derniere = match &derniere_paie.par {
        Some(porteur) => {
            let meme = |k: &str| c.get(k).map(valeur_en_texte);
            let n = crate::routes_ecrites::toutes_les_corrections(ctx)?.into_iter().find(|autre| {
                !autre.get("annulee_le").is_some_and(vrai)
                    && autre.get("portee_par").and_then(|p| p.get("mois")).map(valeur_en_texte).as_deref() == Some(porteur.as_str())
                    && ["mois", "salarie", "debut"].iter().all(|k| autre.get(*k).map(valeur_en_texte) == meme(k))
            });
            let n = n.ok_or_else(|| refus(format!("la DSN de {} a declare une correction de ce contrat, et aucune correction portee par elle ne le dit", porteur)))?;
            Some((porteur.clone(), valeur_en_texte(n.get("numero").unwrap_or(&Value::Null))))
        }
        None => None,
    };
    Ok((etat, derniere))
}

fn prenom_de(sal: &Value) -> String {
    match sal.get("prenom_affichage") {
        Some(v) if vrai(v) => valeur_en_texte(v),
        _ => valeur_en_texte(sal.get("prenom").unwrap_or(&Value::Null)),
    }
}

/// `resume_regularisations` : une ligne lisible par correction embarquee.
pub(crate) fn resume_regularisations(regs: &[Reg]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for r in regs {
        let s = &r.apres.salarie;
        let nom = s.get("nom").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("nom")))?;
        out.push(format!(
            "{} {}, contrat {} ({}) : brut {} -> {}, net {} -> {} -- {}",
            valeur_en_texte(nom),
            prenom_de(s),
            r.apres.engagement.numero.as_deref().unwrap_or("None"),
            engagement::libelle_mois(&r.mois),
            regul::q2(r.declare.brut),
            regul::q2(r.apres.brut),
            regul::q2(r.net_deja_verse),
            regul::q2(r.apres.net_a_payer),
            en_texte_ou_vide(r.correction.get("motif")),
        ));
    }
    Ok(out)
}

/// Ce que la DSN et le bulletin IMPRIMENT : la paie d'un engagement, refaite depuis la saisie du mois
/// d'origine comme le moteur la refait (`paie_rust.bulletin`, `paie_rust.dsn`), aux numeros donnes.
pub(crate) fn a_imprimer(ctx: &Contexte, donnees: &Value, engagement: &Value, numero_contrat: Option<&String>, numero_bulletin: Option<String>) -> R<Paie> {
    let mut seule = donnees.clone();
    if let Some(obj) = seule.as_object_mut() {
        obj.insert("engagements".into(), Value::Array(vec![engagement.clone()]));
    }
    let s = engagement::charger_donnees(ctx, &seule).map_err(|msg| Echec::from(crate::routes_ouverture::refus_du_calcul(ctx, &seule, msg)))?;
    let mut p = s.paies.into_iter().next().ok_or_else(|| erreur("IndexError", socle::fichiers::trop_courte("la paie de l'engagement")))?;
    p.engagement.numero = numero_contrat.cloned();
    p.numero_bulletin = numero_bulletin;
    Ok(p)
}

/// Les regularisations de la DSN (`dsn::Regularisation`), avec les paies que le moteur imprime.
pub(crate) fn pour_la_dsn(ctx: &Contexte, regs: &[&Reg]) -> R<Vec<crate::dsn::Regularisation>> {
    let mut out = Vec::new();
    for r in regs {
        out.push(crate::dsn::Regularisation {
            mois: r.mois.clone(),
            avant: a_imprimer(ctx, &r.donnees, &r.declare_saisi, r.avant.engagement.numero.as_ref(), None)?,
            declare: r.declare.clone(),
            apres: a_imprimer(ctx, &r.donnees, &r.corrige, r.apres.engagement.numero.as_ref(), None)?,
            taux_pas_origine: r.taux_pas_origine.clone(),
            net_deja_verse: r.net_deja_verse,
            // « AAAA-MM-JJ HH:MM » (`routes_ecritures_g2::maintenant`) : le jour seul.
            decidee_le: r.correction.get("decidee_le").and_then(Value::as_str).and_then(|t| t.get(..10)).and_then(|t| Jour::lire(t).ok()),
        });
    }
    Ok(out)
}

fn refus_valeur(m: String) -> Echec {
    erreur("ValueError", m)
}

/// Le net a payer avant impot deja verse au salarie pour le contrat corrige (le « net a payer avant impot du bulletin
/// rectifie ») : il se LIT sur le bulletin que le salarie a recu (regle 4, Fable 10/10/2026), jamais dans la DSN. La DSN
/// ne declare pas le net a payer, et rien dans un fichier depose ne dit quel moteur l'a ecrit : un depot d'avant la
/// vague 1b porte en 58.004 le net a payer (frais compris, acompte deduit), un depot d'apres le montant net social.
///
/// Le bulletin qui fait foi : celui du mois d'origine, archive dans `sorties/<mois>/Bulletins_<Mois><Annee>.pdf` ; ou,
/// si une correction de ce contrat est deja partie dans la DSN d'un mois anterieur (`derniere`, celle que
/// `versement_declare_de` a suivie), son bulletin rectificatif `sorties/<mois>/Rectificatif_<cle>_<Mois><Annee>_c<n>.pdf`,
/// dont le net a payer avant impot est le net rectifie qu'elle a fait payer. Le bulletin du contrat s'y reconnait au
/// numero de contrat imprime et a sa periode (`apercu_dsn::numeros_archives`, le lecteur des bulletins du moteur) ;
/// deux contrats du meme salarie dans le mois (jumeaux) ont chacun leur numero, et ne se confondent pas.
///
/// REFUS (regle 1), la DSN du mois ne se produit pas : le fichier absent ou illisible, aucun bulletin du contrat sur sa
/// periode, plusieurs (lequel a ete paye ne se lit pas), ou un bulletin dont le net a payer ne se lit pas.
fn net_deja_verse_lu(ctx: &Contexte, mois: &str, mois_courant: &str, avant: &Paie, apres: &Paie, derniere: Option<&(String, String)>) -> R<Decimal> {
    let nom = valeur_en_texte(apres.salarie.get("nom").unwrap_or(&Value::Null));
    let numero = apres.engagement.numero.clone().unwrap_or_default();
    let etiq = crate::envoi::etiquette(mois).map_err(refus_valeur)?;
    let (fichier, lequel) = match derniere {
        None => (format!("Bulletins_{}.pdf", etiq), "le bulletin du mois d'origine, tel qu'il lui a ete remis".to_string()),
        Some((porteur, n)) => {
            let cle = valeur_en_texte(apres.salarie.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
            (
                format!("{}_{}_{}_c{}.pdf", PREFIXE_ARCHIVE, cle, etiq, n),
                format!("le bulletin rectificatif de la correction n° {}, partie dans la DSN de {}", n, porteur),
            )
        }
    };
    let rel = format!("sorties/{}/{}", mois, fichier);
    let refus = |quoi: String| {
        erreur(
            "MoisNonRegularisable",
            format!(
                "Correction de {} pour {} (contrat {}) : {}. Le net deja verse au salarie se lit sur le bulletin qu'il a recu ({}) ; il ne se reconstitue pas depuis la DSN. Remettre {} dans sorties/{}/, ou annuler la correction. La DSN de {} ne se produit pas tant que ce n'est pas regle.",
                mois, nom, numero, quoi, lequel, fichier, mois, mois_courant
            ),
        )
    };
    let chemin = ctx.racine.join("sorties").join(mois).join(&fichier);
    if !chemin.exists() {
        return Err(refus(format!("{} n'est pas sur le disque", rel)));
    }
    let lus = crate::apercu_dsn::numeros_archives(&chemin).map_err(|e| refus(format!("{} ne se lit pas ({})", rel, e.message.trim_end_matches('.'))))?;
    let e = &avant.engagement;
    let (debut, fin) = (e.debut_periode().map_err(refus_valeur)?.iso(), e.fin_periode().map_err(refus_valeur)?.iso());
    let siens: Vec<&crate::apercu_dsn::Lu> = lus.iter().filter(|lu| lu.contrat == numero && lu.debut <= fin && lu.fin >= debut).collect();
    match siens.as_slice() {
        [] => Err(refus(format!("{} ne porte aucun bulletin du contrat {} sur sa periode (du {} au {})", rel, numero, debut, fin))),
        [lu] => lu.net.ok_or_else(|| refus(format!("le bulletin n° {} de {} n'imprime pas de net a payer avant impot lisible", lu.bulletin, rel))),
        plusieurs => Err(refus(format!(
            "{} porte {} bulletins du contrat {} sur sa periode (n° {}) : lequel a ete paye ne se lit pas",
            rel,
            plusieurs.len(),
            numero,
            plusieurs.iter().map(|lu| lu.bulletin.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// `versement_de_la_correction` : ce que la correction verse -- ou retient -- CE mois-ci : la difference de net,
/// et le prelevement a la source sur cette difference, calcule par la meme porte que le bloc 50 de la DSN.
///
/// Due au salarie : le taux du mois ou l'on paie, sur le bareme de ce mois. Trop-verse (net imposable qui
/// baisse) : le taux du mois de l'erreur, lu dans la DSN deposee (bloc 56). Un bareme sans grille ne rend pas de
/// prelevement : on ne l'invente pas.
pub(crate) fn versement_de_la_correction(ctx: &Contexte, r: &Reg, mois_courant: &str) -> R<Value> {
    // Ce qui a ete verse et declare se lit dans la DSN deposee, pas dans un recalcul : le net imposable en 50.002 ; le
    // net deja verse se lit sur le bulletin recu (`net_deja_verse_lu`).
    let (avant, apres) = (&r.declare, &r.apres);
    let d_ni = apres.net_imposable - avant.net_imposable;
    let net_avant = r.net_deja_verse;
    let d_net = apres.net_a_payer - net_avant;
    let an: i32 = mois_courant.get(..4).and_then(|t| t.parse().ok()).ok_or_else(|| refus_valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois_courant))))?;
    let mo: u32 = mois_courant.get(5..7).and_then(|t| t.parse().ok()).ok_or_else(|| refus_valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois_courant))))?;
    let fin_mois = Jour::nouveau(an, mo, jours_du_mois(an, mo)).ok_or_else(|| refus_valeur(socle::date::date_impossible(an, mo, 1)))?;
    let debut_mois = Jour::nouveau(an, mo, 1).ok_or_else(|| refus_valeur(socle::date::date_impossible(an, mo, 1)))?;
    let indu = d_ni < Decimal::ZERO;
    let mut taux: Value = Value::Null;
    let mut prelevement: Option<Decimal> = None;
    if indu {
        if let Some(t) = r.taux_pas_origine.as_deref().filter(|t| !t.is_empty()) {
            taux = json!(t);
            prelevement = Some(pct(d_ni, dec_avec_echelle(&json!(t)).map_err(refus_valeur)?));
        }
    } else {
        let cle = valeur_en_texte(apres.salarie.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        let triplet = pas::taux_dgfip(ctx, &cle, None).map_err(refus_valeur)?;
        let bareme = ctx.bareme(&debut_mois).map_err(refus_valeur)?;
        let court = crate::dsn::court(apres, fin_mois).map_err(refus_valeur)?;
        match pas::du_versement(triplet.as_ref(), &bareme, d_ni, Some(fin_mois), court, ZONE_DEFAUT) {
            Ok(v) => {
                prelevement = Some(pct(v.montant_soumis, dec_avec_echelle(&v.taux).map_err(refus_valeur)?));
                taux = v.taux;
            }
            Err(ErreurPas::SansGrille(_)) => {}
            Err(e) => return Err(refus_valeur(e.message().to_string())),
        }
    }
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
        "motif": r.correction.get("motif").cloned().unwrap_or(Value::Null),
    }))
}

/// `numero_du_bulletin_rectifie` : le numero du bulletin d'origine, LU dans le fichier groupe archive du mois
/// d'origine, par le numero de contrat declare. `None` s'il ne s'y lit pas : l'appelant le dit, il ne le devine
/// pas. Un fichier illisible ne se devine pas non plus.
fn numero_du_bulletin_rectifie(ctx: &Contexte, r: &Reg) -> Option<String> {
    let etiq = crate::envoi::etiquette(&r.mois).ok()?;
    let groupe = ctx.racine.join("sorties").join(&r.mois).join(format!("Bulletins_{}.pdf", etiq));
    if !groupe.exists() {
        return None;
    }
    let lus = crate::apercu_dsn::numeros_archives(&groupe).ok()?;
    let e = &r.avant.engagement;
    let numero = e.numero.as_deref()?;
    let (debut, fin) = (e.debut_periode().ok()?.iso(), e.fin_periode().ok()?.iso());
    lus.into_iter().find(|lu| lu.contrat == numero && lu.debut <= fin && lu.fin >= debut).map(|lu| lu.bulletin)
}

/// Ce que `ecrire_rectificatifs` rend : les fichiers ecrits, les avertissements (de simples phrases).
pub(crate) struct Rectificatifs {
    pub(crate) fichiers: Vec<PathBuf>,
    pub(crate) avertissements: Vec<String>,
}

fn ecrire_le_pdf(description: &Value, chemin: &Path) -> R<()> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur("OSError", format!("{} : {}", parent.display(), e)))?;
    }
    crate::routes_dossier_salarie::ecrire_pdf(description, chemin).map_err(Echec::from)
}

/// `ecrire_rectificatifs` : les bulletins rectificatifs des corrections que la DSN de `mois_courant` embarque.
///
/// Les numeros se RESERVENT (`reserves`, comme `ctx.enregistrer_compteur`) dans la suite de l'annee du mois
/// courant, apres ceux des bulletins du mois : ils ne s'ecrivent qu'a la validation, une fois le mois entier
/// produit. Deux fichiers par correction, jamais a la place d'un autre :
/// `sorties/<mois d'origine>/Rectificatif_<cle>_<Mois>_c<n>.pdf`, a cote du bulletin rectifie ; et, pour
/// l'envoi, un fichier par salarie dans `sorties/<mois courant>/envoi/`.
pub(crate) fn ecrire_rectificatifs(
    ctx: &Contexte,
    regs: &[Reg],
    mois_courant: &str,
    sortie_courante: &Path,
    reserves: &mut crate::depart::Reserves,
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
                regul::q2(r.net_deja_verse)
            ));
        }
    }
    if payes.is_empty() {
        return Ok(out);
    }
    let an: i64 = mois_courant.get(..4).and_then(|t| t.parse().ok()).ok_or_else(|| refus_valeur(format!("nombre entier illisible : {}", socle::fichiers::repr(mois_courant))))?;
    let mut n = reserves.prochain(ctx, "bulletin", an)?;
    // Par salarie, dans l'ordre ou on les rencontre : (cle, [(paie imprimee, rectificatif, ent)]).
    let mut par_salarie: Vec<(String, Vec<(Paie, Value, Value)>)> = Vec::new();
    for r in &payes {
        let numero_bulletin = format!("{}/{}", n, an);
        n += 1;
        let mut rectif = versement_de_la_correction(ctx, r, mois_courant)?;
        let rectifie = numero_du_bulletin_rectifie(ctx, r);
        rectif["rectifie"] = rectifie.clone().map_or(Value::Null, Value::String);
        let contrat = r.apres.engagement.numero.as_deref().unwrap_or("None");
        // Un contrat ajoute n'a pas de bulletin d'origine : son bulletin est celui-ci.
        if rectifie.is_none() && !r.correction.get("ajout").is_some_and(vrai) {
            out.avertissements.push(format!(
                "Bulletin rectificatif {} ({}, contrat {}) : le numero du bulletin d'origine ne se lit pas dans sorties/{}/ -- le rectificatif cite le contrat a la place.",
                numero_bulletin,
                engagement::libelle_mois(&r.mois),
                contrat,
                r.mois
            ));
        }
        if rectif["prelevement"].is_null() {
            out.avertissements.push(format!(
                "Bulletin rectificatif {} : le prelevement a la source sur la difference n'a pas pu etre calcule -- le bulletin le dit.",
                numero_bulletin
            ));
        }
        let ent = ctx.entreprise_du_mois(&r.mois).map_err(Echec::from)?;
        let cle = valeur_en_texte(r.apres.salarie.get("_cle").ok_or_else(|| erreur("KeyError", socle::fichiers::absent("_cle")))?);
        let etiq = crate::envoi::etiquette(&r.mois).map_err(refus_valeur)?;
        let archive = ctx.racine.join("sorties").join(&r.mois).join(format!("{}_{}_{}_c{}.pdf", PREFIXE_ARCHIVE, cle, etiq, r.numero));
        let imprimee = a_imprimer(ctx, &r.donnees, &r.corrige, r.apres.engagement.numero.as_ref(), Some(numero_bulletin))?;
        let rectifs: HashMap<usize, Value> = HashMap::from([(0, rectif.clone())]);
        let description = crate::bulletin::jeux_et_description(ctx, std::slice::from_ref(&imprimee), &ent, "", None, false, &rectifs).map_err(refus_valeur)?;
        ecrire_le_pdf(&description, &archive)?;
        out.fichiers.push(archive);
        match par_salarie.iter_mut().find(|(c, _)| *c == cle) {
            Some((_, v)) => v.push((imprimee, rectif, ent)),
            None => par_salarie.push((cle, vec![(imprimee, rectif, ent)])),
        }
    }
    let dossier = sortie_courante.join("envoi");
    let etiq = crate::envoi::etiquette(mois_courant).map_err(refus_valeur)?;
    for (cle, siens) in &par_salarie {
        // Un fichier par salarie, tous ses rectificatifs dedans, comme ses bulletins du mois. Un seul `ent` par
        // fichier : la licence du premier mois d'origine (les archives, elles, ont chacune la leur).
        let chemin = dossier.join(format!("{}_{}_{}.pdf", PREFIXE_ENVOI, cle, etiq));
        let paies: Vec<Paie> = siens.iter().map(|(p, _, _)| p.clone()).collect();
        let rectifs: HashMap<usize, Value> = siens.iter().enumerate().map(|(i, (_, x, _))| (i, x.clone())).collect();
        let description = crate::bulletin::jeux_et_description(ctx, &paies, &siens[0].2, "", None, false, &rectifs).map_err(refus_valeur)?;
        ecrire_le_pdf(&description, &chemin)?;
    }
    reserves.enregistrer(an, "bulletin", n - 1);
    Ok(out)
}
