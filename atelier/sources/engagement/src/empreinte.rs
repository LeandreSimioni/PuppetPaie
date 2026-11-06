//! CE DONT UNE PAIE DEPEND : l'empreinte est celle d'un engagement, pas d'un mois. Le calcul d'un mois note tout ce qu'il
//! lit (`socle::fichiers::suivre`) : le contenu de chaque fichier (baremes du socle et de la compagnie, catalogues livres,
//! effectifs...), les fichiers absents, les dossiers parcourus, le jour s'il le lit, le taux de prelevement a
//! la source de son salarie au jour de son paiement (`pas <salarie> <jour> <mois>`, `droit::pas::taux_dgfip`), ses imprevus (`imprevus <magasin> <salarie>`, `imprevus::du_salarie`), et les paies
//! validees de son salarie dans les autres mois qu'il lit (`paies <mois> <salarie>`,
//! `etats::paies_validees`), l'indemnite de mission a l'etranger de chaque jour qu'il paie (`mission <lieu> <jour>`,
//! `droit::missions` : la valeur prise, pas la table), ou, pour le premier mois d'un permanent, ses contrats payes au mois d'avant, qu'il n'a pas
//! (`mensuels <mois> <salarie>`, `conges::avant_le_mois`). De la saisie de son mois, chaque paie ne garde que ce dont elle depend :
//! ce que le mois pose pour tous, les engagements valides de son salarie (`saisie <mois> <salarie>`,
//! `etats::calculer_les_rangs`) et les fiches de l'entreprise, de son salarie (hors de ses taux recus) et de son spectacle au dernier jour du mois
//! (`fiches`). S'y ajoutent la marque du code qui calcule (`MOTEUR`) et les regles datees qui
//! s'appliquent a la periode payee (`regles`).
//!
//! L'empreinte TIENT quand chacune de ses entrees vaut encore ce qu'elle valait (`tient`) : aucun calcul, une relecture
//! du disque. Elle ne tient plus : la paie se recalcule, et ce qui a change dit pourquoi (`etats::rafraichir`).

use std::collections::{BTreeMap, HashMap};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::fichiers::{self, dossier_du_code};
use socle::num::Resultat;

use crate::stock;

/// La marque du code qui calcule : la version du crate et celle de ses sources et de celles du socle et du droit
/// (`build.rs`). Un autre code recalcule ce qui n'est pas parti.
pub const MOTEUR: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("ENGAGEMENT_SOURCES"));

/// L'entree des fiches dont depend la paie de `salarie` pour `mois`, sur `spectacle` : l'entreprise, la fiche du salarie
/// et celle du spectacle, telles qu'elles valent au dernier jour du mois (`Contexte::du_mois`). La fiche d'un autre
/// salarie, une valeur datee apres le mois n'en sont pas : elles ne font pas recalculer cette paie.
pub(crate) fn fiches(ctx: &Contexte, mois: &str, salarie: &str, spectacle: Option<&str>) -> Resultat<(String, String)> {
    let cle = socle::ecriture::json_en_ligne(&(mois, salarie, spectacle));
    Ok((format!("fiches {}", cle), empreinte_fiches(&ctx.du_mois(mois)?, salarie, spectacle)))
}

fn empreinte_fiches(ctx: &Contexte, salarie: &str, spectacle: Option<&str>) -> String {
    // L'entreprise, la fiche au jour sans ses taux recus, et le spectacle, chacun dans l'ordre de sa forme
    // (`droit::fiche`) ; ce qui manque s'ecrit `null`.
    let sal = ctx.salaries.get(salarie).map(|f| droit::fiche::ChampsSalarie { taux_pas: droit::fiche::Case::Absente, ..f.champs.clone() });
    let spec = spectacle.and_then(|k| ctx.spectacles.get(k));
    fichiers::sha256_hex(socle::ecriture::json_en_ligne(&(&ctx.entreprise, &sal, &spec)).as_bytes())
}

/// L'entree `fiches [mois, salarie, spectacle]` tient-elle encore ?
fn fiches_tiennent(ctx: &Contexte, cle: &str, v: &str) -> Resultat<bool> {
    let (mois, salarie, spectacle) = socle::forme::lire_texte::<(String, String, Option<String>)>(cle, socle::echec::PAIE_ILLISIBLE)?;
    Ok(fiches(ctx, &mois, &salarie, spectacle.as_deref())?.1 == v)
}

/// Une entree de fichier qui est la saisie d'un mois : elle se compte par la saisie (`stock::cle_de_saisie`), pas par
/// les octets de son fichier.
fn est_un_fichier_d_engagements(cle: &str) -> bool {
    cle.split_once(' ').is_some_and(|(_, c)| c.starts_with(&format!("racine:{}/", stock::DOSSIER)) || c == format!("racine:{}", stock::DOSSIER))
}

/// `f`, et tout ce qu'il a lu. Les fiches s'y ajoutent paie par paie (`fiches`).
pub(crate) fn suivre<T>(ctx: &Contexte, f: impl FnOnce() -> Resultat<T>) -> Resultat<(T, BTreeMap<String, String>)> {
    fichiers::suivre(&ctx.racine, &dossier_du_code());
    let r = f();
    let mut entrees = fichiers::rendre();
    entrees.retain(|k, _| !est_un_fichier_d_engagements(k));
    entrees.insert("moteur".into(), MOTEUR.into());
    Ok((r?, entrees))
}

/// Ce qu'une passe de verification a deja relu : une entree qui revient d'une paie a l'autre (les baremes, les fiches, la
/// saisie du mois) ne se relit qu'une fois.
#[derive(Default)]
pub struct Relues(HashMap<(String, String), bool>);

/// Les entrees valent-elles encore ce qu'elles valaient ? Chacune est relue sur le disque ; la saisie d'un mois se relit
/// et se hache ; rien ne se calcule. Une entree qui ne se relit pas (une fiche, une paie, un imprevu illisibles) refuse,
/// avec sa raison : rien ne dit qu'elle tient, ni qu'elle a change.
pub fn tient(ctx: &Contexte, entrees: &BTreeMap<String, String>, relues: &mut Relues) -> Resultat<bool> {
    for (k, v) in entrees {
        let t = match relues.0.get(&(k.clone(), v.clone())) {
            Some(t) => *t,
            None => {
                let t = entree_tient(ctx, k, v)?;
                relues.0.insert((k.clone(), v.clone()), t);
                t
            }
        };
        if !t {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Une decision de l'editeur rend-elle l'empreinte ecrite `ecrite` (d'une paie de `mois`) fausse ? Le code qui calcule
/// a change (`MOTEUR`, ou un fichier livre avec le code), ou une regle datee s'applique au mois qu'elle n'avait pas.
/// Rien ne se calcule.
pub fn code_ou_regle_change(ctx: &Contexte, ecrite: &crate::paie_ecrite::Empreinte, mois: &str, relues: &mut Relues) -> Resultat<bool> {
    let code = |k: &str| k == "moteur" || k.split_once(' ').is_some_and(|(_, c)| c.starts_with("code:"));
    let entrees: BTreeMap<String, String> = ecrite.entrees.iter().filter(|(k, _)| code(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    if !tient(ctx, &entrees, relues)? {
        return Ok(true);
    }
    let fin = socle::mois::Mois::lire(mois)?.dernier_jour();
    Ok(regles(ctx, &fin)?.iter().any(|x| !ecrite.regles.contains(x)))
}

fn entree_tient(ctx: &Contexte, k: &str, v: &str) -> Resultat<bool> {
    let (racine, code) = (&ctx.racine, dossier_du_code());
    if let Some(c) = k.strip_prefix("fiches ") {
        return fiches_tiennent(ctx, c, v);
    }
    match k {
        "moteur" => return Ok(v == MOTEUR),
        "date" => return Ok(v == ctx.date.iso()),
        "aujourdhui" => return Ok(v == droit::contexte::aujourdhui_sans_note()?.iso()),
        _ => {}
    }
    // L'indemnite de mission d'un jour a l'etranger : la valeur prise, pas la table (`droit::missions`).
    if let Some((code, jour)) = k.strip_prefix("mission ").and_then(|r| r.split_once(' ')) {
        return Ok(v == droit::missions::du_jour(code, &Jour::lire(jour)?)?.empreinte());
    }
    if k.starts_with("pas ") {
        return Ok(droit::pas::empreinte(ctx, k).is_some_and(|e| v == e));
    }
    if let Some((mois, salarie)) = k.strip_prefix(crate::conges::MENSUELS).and_then(|r| r.split_once(' ')) {
        return Ok(v == crate::conges::empreinte_des_mensuels_d_avant(ctx, mois, salarie)?);
    }
    if let Some((mois, salarie)) = k.strip_prefix("paies ").and_then(|r| r.split_once(' ')) {
        return Ok(v == crate::etats::empreinte_des_paies_validees(ctx, mois, Some(salarie))?);
    }
    if let Some(salarie) = crate::imprevus::salarie_de_l_entree(k) {
        return Ok(v == crate::imprevus::empreinte_du_salarie(racine, salarie)?);
    }
    if let Some(reste) = k.strip_prefix("saisie ") {
        let (mois, salarie) = match reste.split_once(' ') {
            Some((m, s)) => (m, Some(s)),
            None => (reste, None),
        };
        return Ok(stock::empreinte_de_la_saisie_du_mois(racine, mois, salarie)?.is_some_and(|e| v == e));
    }
    Ok(fichiers::chemin_de(k, racine, &code).is_some_and(|(genre, p)| match genre.as_str() {
        "fichier" => v == fichiers::empreinte_fichier(&p),
        "existe" => v == if p.exists() { "oui" } else { "non" },
        "dossier?" => v == if p.is_dir() { "oui" } else { "non" },
        "dossier" => v == fichiers::empreinte_dossier(&p),
        _ => false,
    }))
}

/// Les regles datees qui s'appliquent au jour `jour` (`Contexte::regles_datees`), en AAAA-MM-JJ.
pub fn regles(ctx: &Contexte, jour: &Jour) -> Resultat<Vec<String>> {
    Ok(ctx.regles_datees(jour)?.into_iter().map(|j| j.iso()).collect())
}

/// Les valeurs datees de la compagnie qui s'appliquent au jour `jour` a l'engagement de `salarie` sur `spectacle`
/// (`Contexte::configuration_datee`), en AAAA-MM-JJ. Elles ne font pas recalculer (les fiches sont deja dans les
/// entrees) : elles disent a quelle date s'applique une valeur nouvelle.
pub fn configuration(ctx: &Contexte, jour: &Jour, salarie: &str, spectacle: Option<&str>) -> Resultat<Vec<String>> {
    Ok(ctx.configuration_datee(jour, salarie, spectacle)?.into_iter().map(|j| j.iso()).collect())
}

/// L'empreinte telle qu'elle s'ecrit : les entrees, les regles datees, et les valeurs datees de la compagnie.
pub fn ecrite(entrees: &BTreeMap<String, String>, regles: &[String], configuration: &[String]) -> crate::paie_ecrite::Empreinte {
    crate::paie_ecrite::Empreinte { entrees: entrees.clone(), regles: regles.to_vec(), configuration: configuration.to_vec() }
}
