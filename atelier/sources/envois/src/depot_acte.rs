//! DEPOSER UNE DSN OU UNE DPAE, une fois que `moteur` a tout verifie (le fichier lu, la preuve d'un humain pour un reel,
//! la repetition, le mot de passe de l'organisme pris au coffre, les versions des engagements que le depot emporte) :
//! le depot lui-meme, selon le branchement (le monde, ou le simulateur qui rend un flux `FICTIF-…`), la reponse archivee
//! dans `retours/depots/`, et la ligne au journal des depots, quoi qu'il arrive. Ce que le depot laisse a faire a la
//! compagnie (l'incident d'un refus, les marques d'un depot reel) va dans les suites.

use std::path::{Path, PathBuf};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};
use socle::mois::Mois;

use crate::branchement::{reponse_fictive, Branchement};
use crate::depot;
use declarations::dsn_lue::DsnLue;
use crate::ligne::{Acte, DetailDpae, DetailDsn, EmbaucheDeclaree, Envoi, Ligne, Mode, Ouverture, Version};
use crate::releve_retours::{Client, Panne};
use crate::suites::{DepotRefuse, Suites};

/// Ce qui vient de partir, et ce que l'organisme a repondu : la reponse de la route `deposer`. `ligne` est la ligne
/// ecrite au journal ; `mois`, celui que le fichier declare.
#[derive(Debug, serde::Serialize)]
pub struct DepotFait {
    pub quoi: String,
    pub mois: Mois,
    pub reel: bool,
    pub reponse: String,
    pub flux: String,
    pub trace: Option<PathBuf>,
    pub ligne: Ligne,
    /// Le depot PRECEDENT du meme fichier, s'il y en a eu un.
    pub deja: Option<Ligne>,
    /// La repetition qui a ouvert ce depot reel (rien pour un test : il EST la repetition).
    pub repetition: Option<Ouverture>,
    /// Ce que le depot a entraine, en phrases : chaque demande de taux de PAS partie avec une DPAE, ce que les retours
    /// laissent a faire.
    pub suites: Vec<String>,
}

/// Ce que `moteur` a verifie et prepare avant le depot.
pub struct Depot<'a> {
    /// Le fichier depose, tel qu'il est sur le disque.
    pub chemin: &'a Path,
    pub octets: &'a [u8],
    /// Le mois que le fichier declare ; pour un signalement, celui de son evenement (`evenement`).
    pub mois: Mois,
    /// Le jour de l'evenement qu'un signalement declare (`DsnLue::evenement_lisible`).
    pub evenement: Option<String>,
    pub reel: bool,
    pub qui: &'a str,
    /// La forme de la preuve d'un depot reel (`""` pour un test).
    pub preuve: &'a str,
    /// La repetition qui ouvre un depot reel (rien pour un test).
    pub repetition: Option<&'a Ouverture>,
    /// Le mot de passe de l'organisme (le monde seulement).
    pub motdepasse: Option<String>,
    /// La version de chaque engagement que le depot emporte.
    pub versions: &'a [Version],
}

/// Garde la reponse de l'organisme a cote du journal, en clair, datee. Le nom doit tenir sur tous les systemes (un `?`
/// est impossible sous Windows), et une reponse n'en ecrase JAMAIS une autre : deux depots dans la meme seconde (un test
/// puis le reel) prennent `-2`, `-3`... avant `.txt`.
pub fn archiver_reponse(racine: &Path, quoi: &str, mois: &str, reponse: &str) -> R<PathBuf> {
    let dossier = racine.join(crate::retours::RETOURS).join("depots");
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", socle::echec::du_disque(&dossier, &e)))?;
    let etiquette: String = mois.replace('-', "").chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
    let etiquette = if etiquette.is_empty() { "sans-date".to_string() } else { etiquette };
    let iso = socle::paris::maintenant();
    let horo = format!("{}{}{}-{}{}{}", &iso[0..4], &iso[5..7], &iso[8..10], &iso[11..13], &iso[14..16], &iso[17..19]);
    let mut rang = 1;
    loop {
        let nom = if rang == 1 { format!("{}_{}_{}.txt", horo, quoi, etiquette) } else { format!("{}_{}_{}-{}.txt", horo, quoi, etiquette, rang) };
        let p = dossier.join(nom);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(reponse.as_bytes()).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
                return Ok(p);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => rang += 1,
            Err(e) => return Err(erreur("OSError", socle::echec::du_disque(&p, &e))),
        }
    }
}

/// L'envoi du depot `d` a `destinataire`, et ce que l'organisme a repondu.
fn envoi_du_depot<D, Rt>(d: &Depot, accepte: bool, destinataire: &str, flux: &str, reponse: &str, trace: Option<String>, detail: D, versions: bool) -> Envoi<D, Rt> {
    let mut e = Envoi::nouveau(Mode::de(d.reel), d.qui, accepte, destinataire, reponse, detail);
    e.mois = Some(d.mois);
    e.flux = flux.to_string();
    e.trace = trace;
    e.versions = versions.then(|| d.versions.to_vec());
    e
}

/// Ajoute la ligne au journal des depots et la rend.
fn noter(ctx: &Contexte, d: &Depot, ligne: Ligne) -> R<Ligne> {
    crate::journal::noter(&ctx.racine, ligne, Some(d.chemin), &socle::paris::maintenant()).map_err(|m| erreur("OSError", m))
}

/// LA TRACE, RELATIVE A LA COMPAGNIE : une compagnie change de dossier, et un chemin absolu ne menerait nulle part.
fn trace_relative(ctx: &Contexte, t: &Path) -> String {
    t.strip_prefix(&ctx.racine).unwrap_or(t).to_string_lossy().replace('\\', "/")
}

/// Le depot a echoue : la ligne refusee au journal, l'incident dans les suites, et le refus rendu.
fn echec(ctx: &Contexte, quoi: Acte, d: &Depot, ligne: Ligne, e: Echec, suites: &mut Suites) -> Echec {
    let classe = e.classe.unwrap_or("Exception");
    let _ = noter(ctx, d, ligne);
    suites.depot_refuse = Some(DepotRefuse { quoi: quoi.nom().into(), reel: d.reel, classe: classe.into(), message: e.phrase(), repetition: d.repetition.cloned() });
    e
}

/// LE SIMULATEUR REPOND DANS LE GESTE : rien ne voyage, rien n'oblige a attendre ; les retours d'un depot joue (accuse,
/// bilan, et ce que les organismes rendent de plus) sont la des la reponse du depot, et le reel peut suivre son test
/// aussitot. Rend la ligne du journal telle qu'elle est apres eux. Un depot du monde attend ses retours a la releve.
fn repondu_aussitot(ctx: &Contexte, joue: bool, ligne: Ligne, suites: &mut Suites) -> R<Ligne> {
    if !joue {
        return Ok(ligne);
    }
    crate::releve_retours::rattacher_simulation(ctx, suites)?;
    let meme = |l: &Ligne| l.flux() == ligne.flux() && l.quand() == ligne.quand();
    Ok(crate::journal::lu(&ctx.racine)?.lignes.iter().rev().find(|l| meme(l)).cloned().unwrap_or(ligne))
}

/// Depose une DSN. `ordre_test` : l'ordre auquel un essai de mensuelle part en annule-et-remplace (`None` : celui du
/// fichier). `environnement` : `production`, ou la plate-forme d'essai de l'editeur.
pub fn deposer_dsn(
    ctx: &Contexte,
    branchement: Branchement,
    d: &Depot,
    apercu: &DsnLue,
    environnement: &str,
    ordre_test: Option<i64>,
    suites: &mut Suites,
) -> R<DepotFait> {
    branchement.ouvrir()?;
    let deja = crate::etats::deja_depose(&crate::journal::lu(&ctx.racine)?.lignes, Acte::Dsn, &crate::journal::empreinte(d.octets));
    let joue = branchement.joue();
    let envoi = || -> R<String> {
        if joue {
            return reponse_fictive("dsn", d.reel);
        }
        let mut client = Client::dsn(ctx, environnement, d.motdepasse.clone()).map_err(Panne::en_echec)?;
        client.authentifier().map_err(Panne::en_echec)?;
        // La verification est refaite ici : la derniere porte avant le reseau.
        depot::verifier(&declarations::dsn_lue::lire(d.octets, "")?, d.reel, None, true)?;
        let mut contenu = d.octets.to_vec();
        if !d.reel {
            contenu = depot::basculer_dsn(&contenu)?;
            if let Some(n) = ordre_test.filter(|n| *n != 0) {
                contenu = depot::renumeroter(&contenu, &n.to_string())?;
            }
        }
        let url = client.adresses.depot.clone();
        let (reponse, _type) = client.deposer_octets(&url, &contenu, "dsn").map_err(Panne::en_echec)?;
        Ok(reponse)
    };
    let texte = |v: Option<&str>| Some(v.unwrap_or_default().to_string());
    let preuve = Some(d.preuve.to_string());
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let detail = DetailDsn { preuve, erreur: Some(e.classe.unwrap_or("Exception").into()), repetition: d.repetition.cloned(), nature: texte(apercu.nature.as_deref()), ordre: texte(apercu.ordre.as_deref()), evenement: d.evenement.clone(), ..Default::default() };
            let ligne = Ligne::Dsn(envoi_du_depot(d, false, "net-entreprises", "", &e.phrase(), None, detail, false));
            return Err(echec(ctx, Acte::Dsn, d, ligne, e, suites));
        }
    };

    let flux = crate::etats::flux_de(&reponse);
    let trace = archiver_reponse(&ctx.racine, "depot-dsn", &d.evenement.clone().unwrap_or_else(|| d.mois.to_string()), &reponse)?;
    let detail = DetailDsn {
        preuve,
        type_declaration: texte(apercu.type_declaration.as_deref()),
        // LA NATURE : un signalement se numerote a la suite de tous ceux deja partis (`journal::dernier_ordre_des_signalements`).
        nature: texte(apercu.nature.as_deref()),
        // LE RANG DECLARE : le prochain annule-et-remplace se numerote au-dessus de cet ordre.
        ordre: texte(apercu.ordre.as_deref()),
        versements: Some(apercu.versements_organismes.iter().map(|m| m.to_string()).collect()),
        salaries: Some(apercu.individus.len()),
        // CE QUI A ETE PRELEVE SUR LA GRILLE, et non sur le taux du salarie : la seule trace qui reste.
        pas_bareme: Some(apercu.pas_bareme()),
        repetition: d.repetition.cloned(),
        ordre_test,
        etablissement: texte(apercu.etablissement()),
        evenement: d.evenement.clone(),
        ..Default::default()
    };
    let ligne = Ligne::Dsn(envoi_du_depot(d, true, "net-entreprises", &flux, &reponse, Some(trace_relative(ctx, &trace)), detail, true));
    let ligne = noter(ctx, d, ligne)?;
    let ligne = repondu_aussitot(ctx, joue, ligne, suites)?;
    Ok(DepotFait { quoi: "dsn".into(), mois: d.mois, reel: d.reel, reponse, flux, trace: Some(trace), ligne, deja, repetition: d.repetition.cloned(), suites: Vec::new() })
}

/// Depose une DPAE. `contenu` : le fichier tel qu'il part (bascule en test faite) ; `embauches` : ce qu'elle declare
/// (salarie, numero du contrat, premier jour, fin) ; `client` : le client Urssaf, identifiants deja verifies (le monde
/// seulement) ; `deja` : le depot precedent du meme fichier.
#[allow(clippy::too_many_arguments)]
pub fn deposer_dpae(
    ctx: &Contexte,
    branchement: Branchement,
    d: &Depot,
    contenu: &[u8],
    embauches: Vec<EmbaucheDeclaree>,
    client: Option<&mut Client>,
    deja: Option<Ligne>,
    suites: &mut Suites,
) -> R<DepotFait> {
    branchement.ouvrir()?;
    let joue = branchement.joue();
    let mut client = client;
    let mut envoi = || -> R<String> {
        if joue {
            return reponse_fictive("dpae", d.reel);
        }
        let c: &mut Client = client.as_deref_mut().ok_or_else(|| erreur("ErreurOperation", "client absent"))?;
        c.authentifier().map_err(Panne::en_echec)?;
        let url = c.adresses.depot.clone();
        let (reponse, _type) = c.deposer_octets(&url, contenu, "dpae").map_err(Panne::en_echec)?;
        Ok(reponse)
    };
    let preuve = Some(d.preuve.to_string());
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let detail = DetailDpae { preuve, erreur: Some(e.classe.unwrap_or("Exception").into()), repetition: d.repetition.cloned(), ..Default::default() };
            let ligne = Ligne::Dpae(envoi_du_depot(d, false, "urssaf", "", &e.phrase(), None, detail, false));
            return Err(echec(ctx, Acte::Dpae, d, ligne, e, suites));
        }
    };

    let trace = archiver_reponse(&ctx.racine, "depot-dpae", &d.mois.to_string(), &reponse)?;
    let flux = crate::etats::flux_de(&reponse);
    // Ce que ce depot a REELLEMENT declare, a la maille de l'embauche : c'est ce qui repondra ensuite a
    // « lesquelles restent a faire ».
    let detail = DetailDpae { preuve, repetition: d.repetition.cloned(), embauches, ..Default::default() };
    let ligne = Ligne::Dpae(envoi_du_depot(d, true, "urssaf", &flux, &reponse, Some(trace_relative(ctx, &trace)), detail, true));
    let ligne = noter(ctx, d, ligne)?;
    let ligne = repondu_aussitot(ctx, joue, ligne, suites)?;
    Ok(DepotFait { quoi: "dpae".into(), mois: d.mois, reel: d.reel, reponse, flux, trace: Some(trace), ligne, deja, repetition: d.repetition.cloned(), suites: Vec::new() })
}

#[cfg(test)]
mod tests_archives {
    use super::*;

    #[test]
    fn la_reponse_d_un_depot_n_en_ecrase_jamais_une_autre() {
        // Un depot de test puis le reel, dans la meme seconde, portaient le meme nom `<horodatage>_depot-dsn_<mois>.txt`
        // et la reponse du test disparaissait.
        let r = std::env::temp_dir().join(format!("envois-depot-acte-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        let mut chemins = Vec::new();
        for i in 0..6 {
            chemins.push(archiver_reponse(&r, "depot-dsn", "2026-09", &format!("reponse {}", i)).unwrap());
        }
        let mut tous = chemins.clone();
        tous.sort();
        tous.dedup();
        assert_eq!(tous.len(), 6, "{:?}", chemins);
        for (i, c) in chemins.iter().enumerate() {
            assert_eq!(std::fs::read_to_string(c).unwrap(), format!("reponse {}", i), "{}", c.display());
            let nom = c.file_name().unwrap().to_string_lossy().into_owned();
            assert!(nom.ends_with("_depot-dsn_202609.txt") || nom.contains("_depot-dsn_202609-"), "{}", nom);
        }
        // Le nom ordinaire (premier de la seconde) ne change pas : `AAAAMMJJ-HHMMSS_depot-dsn_AAAAMM.txt`.
        let ordinaire = chemins.iter().filter(|c| c.to_string_lossy().ends_with("_depot-dsn_202609.txt")).count();
        assert!(ordinaire >= 1);
        let _ = std::fs::remove_dir_all(&r);
    }
}
