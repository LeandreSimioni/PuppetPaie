//! DEPOSER UNE DSN OU UNE DPAE, une fois que `moteur` a tout verifie (le fichier lu, la preuve d'un humain pour un reel,
//! la repetition, le mot de passe de l'organisme pris au coffre, les versions des engagements que le depot emporte) :
//! le depot lui-meme, selon le branchement (le monde, ou le simulateur qui rend un flux `FICTIF-…`), la reponse archivee
//! dans `retours/depots/`, et la ligne au journal des depots, quoi qu'il arrive. Ce que le depot laisse a faire a la
//! compagnie (l'incident d'un refus, les marques d'un depot reel) va dans les suites.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};

use crate::branchement::{reponse_fictive, Branchement};
use crate::depot::{self, Apercu};
use crate::releve_retours::{Client, Panne};
use crate::suites::{DepotReel, DepotRefuse, Signalement, Suites};

/// Ce qui vient de partir, et ce que l'organisme a repondu.
pub struct DepotFait {
    pub quoi: String,
    pub mois: String,
    pub reel: bool,
    pub reponse: String,
    pub flux: String,
    pub trace: Option<PathBuf>,
    pub ligne: Value,
    /// Le depot PRECEDENT du meme fichier, s'il y en a eu un (`Null` sinon).
    pub deja: Value,
    /// La repetition qui a ouvert ce depot reel (`Null` pour un test : il EST la repetition).
    pub repetition: Value,
}

impl DepotFait {
    /// La reponse de la route `deposer`.
    pub fn en_json(&self) -> Value {
        json!({
            "quoi": self.quoi, "mois": self.mois, "reel": self.reel, "reponse": self.reponse, "flux": self.flux,
            "trace": self.trace.as_ref().map(|t| t.to_string_lossy().into_owned()),
            "ligne": self.ligne, "deja": self.deja, "repetition": self.repetition,
        })
    }
}

/// Le texte d'un refus qui porte une donnee pour la page (`{"message", <cle>}` dans le message) : sa phrase seule.
pub fn phrase_de(e: &Echec) -> String {
    serde_json::from_str::<Value>(&e.message).ok().and_then(|d| d.get("message").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| e.message.clone())
}

/// Ce que `moteur` a verifie et prepare avant le depot.
pub struct Depot<'a> {
    /// Le fichier depose, tel qu'il est sur le disque.
    pub chemin: &'a Path,
    pub octets: &'a [u8],
    pub mois: &'a str,
    pub reel: bool,
    pub qui: &'a str,
    /// La forme de la preuve d'un depot reel (`""` pour un test).
    pub preuve: &'a str,
    /// La repetition qui ouvre un depot reel (`Null` pour un test).
    pub repetition: &'a Value,
    /// Le mot de passe de l'organisme (le monde seulement).
    pub motdepasse: Option<String>,
    /// La version de chaque engagement que le depot emporte.
    pub versions: &'a Value,
}

/// Garde la reponse de l'organisme a cote du journal, en clair, datee. Le nom doit tenir sur tous les systemes (un `?`
/// est impossible sous Windows), et une reponse n'en ecrase JAMAIS une autre : deux depots dans la meme seconde (un test
/// puis le reel) prennent `-2`, `-3`... avant `.txt`.
pub fn archiver_reponse(racine: &Path, quoi: &str, mois: &str, reponse: &str) -> R<PathBuf> {
    let dossier = racine.join("retours").join("depots");
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", format!("{} : {}", dossier.display(), e)))?;
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
                f.write_all(reponse.as_bytes()).map_err(|e| erreur("OSError", format!("{} : {}", p.display(), e)))?;
                return Ok(p);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => rang += 1,
            Err(e) => return Err(erreur("OSError", format!("{} : {}", p.display(), e))),
        }
    }
}

/// Ajoute la ligne au journal des depots et la rend.
#[allow(clippy::too_many_arguments)]
fn noter(
    ctx: &Contexte,
    quoi: &str,
    d: &Depot,
    accepte: bool,
    destinataire: &str,
    flux: &str,
    reponse: &str,
    trace: Option<&Path>,
    detail: &Value,
    versions: Option<&Value>,
) -> R<Value> {
    let fichier = d.chemin.to_string_lossy().into_owned();
    // LA TRACE, RELATIVE A LA COMPAGNIE : une compagnie change de dossier, et un chemin absolu ne mene plus nulle part ;
    // `journal::trace_de` la relit, et lit tel quel un absolu.
    let trace = trace.map(|t| t.strip_prefix(&ctx.racine).unwrap_or(t).to_string_lossy().replace('\\', "/"));
    let n = crate::journal::Note {
        quoi,
        mois: Some(d.mois),
        fichier: Some(&fichier),
        mode: if d.reel { "reel" } else { "test" },
        qui: d.qui,
        accepte,
        destinataire,
        flux,
        reponse,
        trace: trace.as_deref(),
        detail: Some(detail),
        versions,
        quand: "",
        origine: "",
    };
    crate::journal::noter(&ctx.racine, &n, &socle::paris::maintenant()).map_err(|m| erreur("OSError", m))
}

/// Le depot a echoue : la ligne refusee au journal, l'incident dans les suites, et le refus rendu.
fn echec(ctx: &Contexte, quoi: &str, d: &Depot, destinataire: &str, detail: Value, e: Echec, suites: &mut Suites) -> Echec {
    let classe = e.classe.unwrap_or("Exception");
    let phrase = phrase_de(&e);
    let _ = noter(ctx, quoi, d, false, destinataire, "", &phrase.chars().take(600).collect::<String>(), None, &detail, None);
    suites.depot_refuse = Some(DepotRefuse { quoi: quoi.into(), reel: d.reel, classe: classe.into(), message: phrase, repetition: d.repetition.clone() });
    e
}

/// Depose une DSN. `ordre_test` : l'ordre auquel un essai de mensuelle part en annule-et-remplace (`None` : celui du
/// fichier). `environnement` : `production`, ou la plate-forme d'essai de l'editeur.
pub fn deposer_dsn(
    ctx: &Contexte,
    branchement: Branchement,
    d: &Depot,
    apercu: &Apercu,
    environnement: &str,
    ordre_test: Option<i64>,
    suites: &mut Suites,
) -> R<DepotFait> {
    branchement.ouvrir()?;
    let deja = crate::etats::deja_depose(&crate::journal::lu(&ctx.racine).map(|j| j.lignes)?, "dsn", &crate::journal::empreinte(d.octets))?;
    let joue = branchement.joue();
    let envoi = || -> R<String> {
        if joue {
            return reponse_fictive("dsn", d.reel);
        }
        let mut client = Client::dsn(ctx, environnement, d.motdepasse.clone()).map_err(Panne::en_echec)?;
        client.authentifier().map_err(Panne::en_echec)?;
        // La verification est refaite ici : la derniere porte avant le reseau.
        depot::verifier(&depot::lire(d.octets), d.reel, None, true)?;
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
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let detail = json!({"preuve": d.preuve, "erreur": e.classe.unwrap_or("Exception"), "repetition": d.repetition, "nature": apercu.champ("nature"), "ordre": apercu.champ("ordre")});
            return Err(echec(ctx, "dsn", d, "net-entreprises", detail, e, suites));
        }
    };

    let flux = crate::etats::flux_de(&reponse);
    let trace = archiver_reponse(&ctx.racine, "depot-dsn", d.mois, &reponse)?;
    let detail = json!({
        "preuve": d.preuve, "type": apercu.champ("type_declaration"),
        // LA NATURE : un signalement se numerote a la suite de tous ceux deja partis (`journal::dernier_ordre_des_signalements`).
        "nature": apercu.champ("nature"),
        // LE RANG DECLARE : le prochain annule-et-remplace se numerote au-dessus de cet ordre.
        "ordre": apercu.champ("ordre"),
        "versements": apercu.versements, "salaries": apercu.salaries,
        // CE QUI A ETE PRELEVE SUR LA GRILLE, et non sur le taux du salarie : la seule trace qui reste.
        "pas_bareme": apercu.pas_bareme,
        "repetition": d.repetition, "ordre_test": ordre_test,
        "etablissement": apercu.champ("etablissement"),
    });
    let ligne = noter(ctx, "dsn", d, true, "net-entreprises", &flux, &reponse, Some(&trace), &detail, Some(d.versions))?;
    if d.reel {
        let signalement = apercu.est_signalement().then(|| Signalement {
            nature: apercu.champ("nature").into(),
            ordre: apercu.champ("ordre").into(),
            type_declaration: apercu.champ("type_declaration").into(),
            evenement: apercu.evenement_lisible(),
        });
        suites.depot_reel = Some(DepotReel {
            quoi: "dsn".into(),
            mois: d.mois.into(),
            chemin: d.chemin.to_path_buf(),
            quand: ligne.get("quand").and_then(Value::as_str).unwrap_or("").into(),
            joue,
            signalement,
            embauches: Vec::new(),
        });
    }
    Ok(DepotFait { quoi: "dsn".into(), mois: d.mois.into(), reel: d.reel, reponse, flux, trace: Some(trace), ligne, deja, repetition: d.repetition.clone() })
}

/// Depose une DPAE. `contenu` : le fichier tel qu'il part (bascule en test faite) ; `embauches` : ce qu'elle declare
/// (salarie, premier jour, fin) ; `client` : le client Urssaf, identifiants deja verifies (le monde seulement) ; `deja` :
/// le depot precedent du meme fichier.
#[allow(clippy::too_many_arguments)]
pub fn deposer_dpae(
    ctx: &Contexte,
    branchement: Branchement,
    d: &Depot,
    contenu: &[u8],
    embauches: Vec<Value>,
    client: Option<&mut Client>,
    deja: Value,
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
    let reponse = match envoi() {
        Ok(r) => r,
        Err(e) => {
            let detail = json!({"preuve": d.preuve, "erreur": e.classe.unwrap_or("Exception"), "repetition": d.repetition});
            return Err(echec(ctx, "dpae", d, "urssaf", detail, e, suites));
        }
    };

    let trace = archiver_reponse(&ctx.racine, "depot-dpae", d.mois, &reponse)?;
    let flux = crate::etats::flux_de(&reponse);
    // Ce que ce depot a REELLEMENT declare, a la maille de l'embauche : c'est ce qui repondra ensuite a
    // « lesquelles restent a faire ».
    let detail = json!({"preuve": d.preuve, "repetition": d.repetition, "embauches": embauches});
    let ligne = noter(ctx, "dpae", d, true, "urssaf", &flux, &reponse, Some(&trace), &detail, Some(d.versions))?;
    if d.reel {
        suites.depot_reel = Some(DepotReel {
            quoi: "dpae".into(),
            mois: d.mois.into(),
            chemin: d.chemin.to_path_buf(),
            quand: ligne.get("quand").and_then(Value::as_str).unwrap_or("").into(),
            joue,
            signalement: None,
            embauches,
        });
    }
    Ok(DepotFait { quoi: "dpae".into(), mois: d.mois.into(), reel: d.reel, reponse, flux, trace: Some(trace), ligne, deja, repetition: d.repetition.clone() })
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
