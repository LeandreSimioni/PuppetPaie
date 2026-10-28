//! LA RELEVE DES RETOURS, du cote de la compagnie : le branchement de la compagnie (`bridage::branchement`), le mot de
//! passe de consultation pris au coffre (regle 3 : la releve d'`envois` le recoit), puis les suites
//! (`suites::faire` : les incidents des retours, le rangement des attestations France Travail).

use serde_json::json;

use droit::contexte::Contexte;
use envois::branchement::Branchement;
use envois::ordre_signalements::OrdreConnu;
use envois::releve_retours::{Client, Releve};
use envois::suites::Suites;
use socle::echec::{Echec, R};

/// Le mot de passe d'un service, pour une CONSULTATION : l'environnement d'abord, puis le coffre ; `None`
/// quand rien n'est pose. Un coffre qui ne s'ouvre pas est un refus, pas une absence.
pub fn mot_de_passe_de_consultation(ctx: &Contexte, service: &str) -> R<Option<String>> {
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine, "");
    coffre.lire(&json!(service), &json!(crate::coffre::CONSULTATION), false, None).map_err(crate::routes_etats::vers_route)
}

/// Le client net-entreprises de la consultation, dans le monde seulement : `None` quand aucun mot de passe n'est pose.
pub fn client_dsn(ctx: &Contexte, branchement: Branchement, environnement: &str) -> R<Option<Client>> {
    if branchement != Branchement::Monde {
        return Ok(None);
    }
    let Some(mdp) = mot_de_passe_de_consultation(ctx, "net-entreprises")? else { return Ok(None) };
    Client::dsn(ctx, environnement, Some(mdp)).map(Some).map_err(|p| p.en_echec())
}

/// Le client Urssaf de la consultation, dans le monde seulement : `None` quand aucun mot de passe n'est pose.
pub fn client_dpae(ctx: &Contexte, branchement: Branchement) -> R<Option<Client>> {
    if branchement != Branchement::Monde {
        return Ok(None);
    }
    Ok(mot_de_passe_de_consultation(ctx, "urssaf")?.map(|mdp| Client::dpae(ctx, false, Some(mdp))))
}

/// Fait les suites d'une releve et les reporte sur elle.
fn avec_les_suites(ctx: &Contexte, r: R<Releve>, s: Suites) -> R<Releve> {
    let faites = crate::suites::faire(ctx, s);
    let mut releve = r?;
    let faites = faites?;
    releve.a_faire.extend(faites.a_faire);
    releve.incidents.extend(faites.incidents);
    Ok(releve)
}

/// Releve les retours DSN (ou, `canal = "dpae"` avec un client Urssaf donne, ceux de la DPAE) : `client` donne, ou celui
/// de la consultation net-entreprises.
pub fn relever_retours(ctx: &Contexte, debut: i64, fin: i64, tout: bool, environnement: &str, client: Option<Client>, canal: &str) -> R<Releve> {
    let b = crate::bridage::branchement(&ctx.racine);
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, environnement)?,
    };
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours(ctx, b, debut, fin, tout, client, canal, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Releve les retours de la DPAE : par flux s'il est connu, sinon par plage.
pub fn relever_retours_dpae(ctx: &Contexte, debut: i64, fin: i64, tout: bool, flux: &str) -> R<Releve> {
    let b = crate::bridage::branchement(&ctx.racine);
    let client = client_dpae(ctx, b)?;
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours_dpae(ctx, b, debut, fin, tout, flux, client, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Le simulateur rend les retours des depots joues qui les attendent depuis `delai` secondes : le nombre de depots
/// completes, et ce que le rangement des AER jouees laisse a faire (une attestation que la paie ne confirme pas, un nom
/// qu'aucune fiche ne porte). Un rangement qui echoue est un refus.
pub fn rattacher_simulation(ctx: &Contexte, delai: i64) -> R<(i64, Vec<String>)> {
    let mut s = Suites::default();
    let n = envois::releve_retours::rattacher_simulation(ctx, delai, &mut s);
    let faites = crate::suites::faire(ctx, s)?;
    if let Some(i) = faites.incidents.into_iter().next() {
        return Err(Echec::from(i));
    }
    Ok((n?, faites.a_faire))
}

/// Le plus grand ordre de signalement connu (`envois::ordre_signalements::dernier_ordre_connu`) : `client` donne, ou celui
/// de la consultation net-entreprises.
pub fn dernier_ordre_connu(ctx: &Contexte, client: Option<Client>) -> R<OrdreConnu> {
    let b = crate::bridage::branchement(&ctx.racine);
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, "production")?,
    };
    let mut s = Suites::default();
    let r = envois::ordre_signalements::dernier_ordre_connu(ctx, b, client, &mut s);
    crate::suites::faire(ctx, s)?;
    r
}
