//! L'ETAT D'UN MOIS ET L'ETAT DE L'OUVERTURE, tels que l'ecran les lit.
//!
//! L'etat d'un mois porte TOUT ce que l'ecran d'un mois lit : l'etat de chaque engagement (`etat-par-engagement`), celui
//! du mois (`etat-du-mois`), et les lectures que la page faisait une a une (`LECTURES` : la saisie, les contrats en
//! cours, les fins, les imprevus, l'apercu de la saisie enregistree). Il se fait dans le processus de la
//! fenetre, par UNE file (`FileDesEtats`) -- des le lancement pour les compagnies de l'installation (`Fil`) --, puis
//! apres chaque ecriture, a l'ouverture d'une compagnie et apres chaque tour de veille, le mois affiche et ses voisins
//! d'abord. Il ne s'ecrit pas : la file le garde en memoire, sous l'EMPREINTE de tout ce dont il est tire (`empreinte`),
//! et la fenetre le sert sans processus (`FileDesEtats::ecran`, route `mois` de `interface/api.rs`) : le dernier etat
//! connu, TOUT DE SUITE, meme perime -- marque `perime`, refait aussitot, et la page se repeint quand l'etat frais
//! arrive. Les paies des engagements valides sont ecrites dans l'engagement (`engagement::etats`) : un etat se refait
//! sans les recalculer.
//!
//! L'empreinte ne relit pas le contenu des sources : elle prend, pour chacune, sa date de modification et sa taille (une
//! modification a la main change la date). S'y ajoutent la date du jour (une echeance passee change ce qui se dit : un
//! etat se refait donc une fois par jour), le moteur lui-meme et l'emplacement du dossier (l'etat porte des chemins).
//! Tant que l'empreinte est la meme, il se sert sans rien refaire ; si quoi que ce soit a bouge, par le logiciel ou a la
//! main, il se refait une fois.
//!
//! L'OUVERTURE D'UNE COMPAGNIE, en une demi-seconde au plus : l'affichage d'abord ; une fois la page affichee, les petits
//! ajustements du moteur se font sans la faire bouger. Le meme mecanisme,
//! sous une autre cle (`OUVERTURE`) : `ouverture.etat.json`, a la racine de la compagnie, porte ce que la page lit en
//! s'ouvrant et qui n'est pas l'ecran d'un mois (`LECTURES_OUVERTURE` : la configuration et l'historique, le mois a
//! ouvrir, l'echeancier, les rappels, ce qui manque pour demarrer, les conseils) ; lui s'ecrit, pour qu'une compagnie
//! s'ouvre tout de suite au lancement suivant. Meme empreinte (dates et tailles), mais de TOUTE la compagnie ; meme
//! file, qui le refait a l'ouverture, apres chaque ecriture et chaque tour de veille ; meme service sans processus
//! (`FileDesEtats::ouverture`, route `ouverture` de `interface/api.rs`) : ses lectures justes (refaites si besoin), avec
//! l'ecran du mois a ouvrir, marque `perime` s'il l'est. Ce qui se refait, la fenetre le fait faire et dit ce que
//! fait le moteur (`phrase`), au lieu d'une roue muette.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use droit::contexte::Contexte;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

/// Le format du fichier : un autre se refait avant de se montrer. 3 : le moteur qui l'a ecrit (`moteur`) ; 4 : les
/// engagements non actifs (`non_actifs`).
const FORMAT: u64 = 4;

/// Les routes que l'etat d'un mois sert.
pub const ROUTES: [&str; 3] = ["etat-par-engagement", "etat-du-mois", "mois"];

/// Les lectures de l'ecran d'un mois qui ne sont pas les deux etats : la route, et si sa charge est `{"mois"}` (sinon
/// `{}`). La page les demandait une a une, chacune dans un processus ; elles sont faites ici, une fois, avec les memes
/// routes, et rendues telles quelles (resultat ou refus).
/// Les arrets n'y sont plus : la page lit les imprevus du mois, chaque arret avec ses obligations. La reprise
/// du journal non plus : elle se montre dans l'Historique, avec les envois et leurs retours.
const LECTURES: [(&str, bool); 4] = [
    ("saisie", true),
    ("contrats-en-cours", true),
    ("fins-de-contrat", true),
    ("imprevus", true),
];

/// La cle de l'etat garde de l'OUVERTURE dans la file : a cote des mois, jamais confondue avec eux.
pub const OUVERTURE: &str = "ouverture";

/// Ce que la page lit en s'ouvrant, hors de l'ecran d'un mois (`js/demarrage.js`) : la route et sa charge. Chacune est
/// rendue telle quelle (resultat ou refus), au format d'une reponse de la page. Ce qui vit dans la memoire de la fenetre
/// (la veille, la mise a jour) et ce que l'ecran ne garde pas (`journal/signalements.json`) se lit a chaque ouverture,
/// a cote (`interface/api.rs`).
const LECTURES_OUVERTURE: [(&str, &str); 7] = [
    ("etat", "{}"),
    ("mois-a-ouvrir", "{}"),
    ("echeancier", "{}"),
    ("rappels", "{}"),
    ("pour-demarrer", "{}"),
    ("conseils-resume", "{}"),
    ("conseil-du-jour", r#"{"decalage": 0}"#),
];

/// Le fichier de l'etat de l'OUVERTURE, a la racine de la compagnie : le seul etat qui s'ecrit.
pub fn chemin(racine: &Path) -> PathBuf {
    racine.join("ouverture.etat.json")
}


/// Le dossier, absolu et comparable (sous Windows, sans le `\\?\` de `canonicalize`).
fn absolu(p: &Path) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let t = abs.to_string_lossy().into_owned();
    t.strip_prefix(r"\\?\").map(String::from).unwrap_or(t)
}

/// Ajoute a `h` chaque fichier de `dossier` que `garder` retient (chemin relatif a `base`), avec sa date de modification
/// et sa taille, dans l'ordre des noms. Sous Windows, la date et la taille viennent de l'enumeration du dossier : pas
/// d'appel de plus par fichier.
fn signer_dossier(h: &mut Sha256, base: &Path, dossier: &Path, profond: bool, garder: &dyn Fn(&str) -> bool) {
    let mut entrees: Vec<std::fs::DirEntry> = std::fs::read_dir(dossier).into_iter().flatten().flatten().collect();
    entrees.sort_by_key(|e| e.file_name());
    for e in entrees {
        let Ok(t) = e.file_type() else { continue };
        let p = e.path();
        if t.is_dir() {
            if profond {
                signer_dossier(h, base, &p, true, garder);
            }
            continue;
        }
        let nom = p.strip_prefix(base).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        if !garder(&nom) {
            continue;
        }
        let (ns, taille) = e
            .metadata()
            .map(|m| (m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos()), m.len()))
            .unwrap_or((0, 0));
        h.update(nom.as_bytes());
        h.update([0]);
        h.update(ns.to_le_bytes());
        h.update(taille.to_le_bytes());
    }
}

/// CE QUE L'ECRAN D'UN MOIS NE LIT PAS, et qui bouge sans lui. Etabli par mesure (`strace` de la
/// route `mois` sur cinq mois de la grande fictive 3090 : aucun de ces fichiers n'est ouvert ni teste) et par le code
/// (chacun n'a de lecteur que hors des lectures du mois : `LECTURES`, `etat-par-engagement`, `etat-du-mois`, l'apercu) :
/// - `config/.ouverture.json` : la trace de la veille (`ouverture.rs`, `chemin_trace`), reecrite a chaque tour ; lue par
///   `ouverture-derniere` seulement ;
/// - `config/.sauvegarde.json` : l'index de la sauvegarde (`sauvegarde::chemin_index`), reecrit a chaque sauvegarde ; lu
///   par la sauvegarde et la restauration seulement ;
/// - `config/diagnostic.json` : l'identite du poste chez l'editeur (`incidents.rs`, `POSTE_FICHIER`), ecrite par la
///   veille ; lue par l'envoi des incidents et des signalements, le filet et l'import ;
/// - `config/horloge.json` : la memoire de l'horloge d'une compagnie fictive (`horloge.rs`), reecrite a chaque tour de
///   veille ; lue par l'horloge seulement (le jour d'un `Contexte` ne la lit pas) ;
/// - `config/filet-dpae.json` et `config/.filet.verrou` : l'etat du filet DPAE (`filet.rs`), reecrit par
///   `filet-preparer`, que la fenetre lance en fond apres `produire` ; lu par le filet, `filet-etat` et l'ouverture ;
/// - `journal/signalements.json` : « Un probleme, une idee ? » (`problemes.rs`) ; lu par `signalements` et
///   `importation-etat` ;
/// - `journal/incidents.json` : le journal des incidents.
const HORS_DE_L_ECRAN: [&str; 9] = [
    "config/.ouverture.json",
    "config/.sauvegarde.json",
    "config/diagnostic.json",
    "config/horloge.json",
    "config/filet-dpae.json",
    "config/.filet.verrou",
    "journal/signalements.json",
    "journal/incidents.json",
    // Le tableau de l'administrateur (`administrateur.rs`) : le resume de la compagnie que l'accueil lit.
    "config/.tableau.json",
];

/// L'empreinte de tout ce dont l'etat `cle` est tire, au jour `jour` (AAAA-MM-JJ).
///
/// Pour un MOIS : ni le coffre (un `.bin` : ce qu'il dit se relit a chaque lecture), ni ce que l'ecran ne lit pas
/// (`HORS_DE_L_ECRAN`), ni les sorties des autres mois.
///
/// Pour l'OUVERTURE : toute la compagnie -- les engagements et les sorties de tous les mois (le mois a
/// ouvrir et l'historique les lisent), les images de l'identite (`gabarits/`), la date et la taille des fichiers du coffre
/// (la configuration dit si la boite courriel est liee, `boite_liee` : rien n'en est lu ici, ni ailleurs dans
/// l'empreinte), et l'adresse de la boite posee dans l'environnement (`COURRIEL_COMPTE`). Ce qui ne s'y lit pas reste
/// dehors (`HORS_DE_L_ECRAN` : les signalements se lisent a chaque ouverture, a cote).
pub fn empreinte_de(racine: &Path, cle: &str, jour: &str) -> String {
    let ouverture = cle == OUVERTURE;
    let mut h = Sha256::new();
    let mut marques = vec![env!("PAIE_SOURCES").to_string(), jour.to_string(), absolu(racine), absolu(&socle::fichiers::dossier_du_code())];
    if ouverture {
        marques.push(format!("ouverture {}", std::env::var("COURRIEL_COMPTE").unwrap_or_default()));
    }
    for marque in marques {
        h.update(marque.as_bytes());
        h.update([0]);
    }
    let sorties_du_mois = format!("sorties/{}/", cle);
    // La compagnie : ses engagements (et `engagements/avant/`), sa configuration, ses journaux, ses imprevus, ses
    // regularisations, son historique importe, ses retours, et ce qui est sorti pour ce mois-ci (pour l'ouverture : pour
    // tous les mois).
    signer_dossier(&mut h, racine, &racine.join(engagement::stock::DOSSIER), true, &|n| n.ends_with(".json"));
    signer_dossier(&mut h, racine, &racine.join("config"), true, &|n| {
        (ouverture || !n.ends_with(".bin")) && !n.ends_with(".tmp") && !HORS_DE_L_ECRAN.contains(&n)
    });
    signer_dossier(&mut h, racine, &racine.join("journal"), false, &|n| n.ends_with(".json") && !HORS_DE_L_ECRAN.contains(&n));
    for d in ["imprevus", "historique", "retours"] {
        signer_dossier(&mut h, racine, &racine.join(d), true, &|n| !n.ends_with(".tmp"));
    }
    if ouverture {
        signer_dossier(&mut h, racine, &racine.join("sorties"), true, &|n| !n.ends_with(".tmp"));
        signer_dossier(&mut h, racine, &racine.join("gabarits"), true, &|n| !n.ends_with(".tmp"));
    } else {
        signer_dossier(&mut h, racine, &racine.join("sorties").join(cle), true, &|n| n.starts_with(&sorties_du_mois));
    }
    // Le code livre : baremes, bases, modeles (une mise a jour les change sans changer le moteur).
    let code = socle::fichiers::dossier_du_code();
    signer_dossier(&mut h, &code, &code, true, &|_| true);
    h.finalize().iter().map(|o| format!("{:02x}", o)).collect()
}

pub fn empreinte(ctx: &Contexte, mois: &str) -> String {
    empreinte_de(&ctx.racine, mois, &ctx.aujourdhui.iso())
}

/// L'etat de l'ouverture tel qu'il est ecrit, s'il est encore juste.
fn garde(racine: &Path, empreinte: &str) -> Option<Value> {
    let v: Value = serde_json::from_slice(&std::fs::read(chemin(racine)).ok()?).ok()?;
    (v.get("format").and_then(Value::as_u64) == Some(FORMAT) && v.get("empreinte").and_then(Value::as_str) == Some(empreinte)).then_some(v)
}

/// Une route du moteur, faite dans ce processus, rendue comme la page la recoit (`{"ok", "resultat"}` ou
/// `{"ok": false, "erreur": {"type", "message", ...}}`).
fn lecture(o: &HashMap<String, String>, route: &str, charge: Value) -> Value {
    let entree = json!({"route": route, "charge": charge}).to_string();
    let rep = crate::routes::commande(o, &entree).unwrap_or_else(|m| json!({"ok": false, "erreur": m}));
    crate::interface::api::en_page(crate::interface::api::depuis_reponse(rep))
}

/// Les lectures de l'ecran de `mois` (`LECTURES`, puis l'apercu de la saisie enregistree).
fn lectures(o: &HashMap<String, String>, mois: &str) -> Value {
    let mut l = Map::new();
    for (route, du_mois) in LECTURES {
        let charge = if du_mois { json!({ "mois": mois }) } else { json!({}) };
        l.insert(route.to_string(), lecture(o, route, charge));
    }
    // L'APERCU : ce que la page demande des qu'un mois s'affiche (`calculerApercu`), sur la saisie qu'elle vient de lire
    // (`{"mois": mois}`, le mois enregistre) ; la page ne le redemande que quand on modifie la saisie.
    if l.get("saisie").and_then(|s| s.pointer("/resultat/existe")).is_some_and(|e| e == &Value::Bool(true)) {
        l.insert("apercu".into(), lecture(o, "apercu", json!({"mois": mois, "lignes": false})));
    }
    Value::Object(l)
}

/// Refait l'etat de `cle` (un mois, ou l'ouverture, qui s'ecrit). Ce qui se lit peut ecrire (une paie validee qui se
/// recalcule d'elle-meme quand on la lit, `engagement::etats::du_mois`) : l'empreinte d'apres n'est alors
/// plus celle d'avant, et un second passage, sans recalcul cette fois, la retrouve.
fn refaire(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<Value> {
    let charge = json!({ "mois": mois });
    let mut derniere = None;
    for _ in 0..2 {
        let avant = empreinte(ctx, mois);
        let etat = if mois == OUVERTURE {
            // L'OUVERTURE : les lectures de la page qui s'ouvre (`LECTURES_OUVERTURE`), telles quelles.
            let mut l = Map::new();
            for (route, charge) in LECTURES_OUVERTURE {
                l.insert(route.to_string(), lecture(o, route, serde_json::from_str(charge).unwrap_or_else(|_| json!({}))));
            }
            json!({
                "format": FORMAT,
                "moteur": env!("PAIE_SOURCES"),
                "mois": OUVERTURE,
                "empreinte": avant,
                "lectures": l,
            })
        } else {
            let lus = lectures(o, mois);
            let par_engagement = crate::routes::etapes::route(o, ctx, "etat-par-engagement", &charge)?;
            let du_mois = crate::routes::etapes::route(o, ctx, "etat-du-mois", &charge)?;
            json!({
                "format": FORMAT,
                "moteur": env!("PAIE_SOURCES"),
                "mois": mois,
                "empreinte": avant,
                "engagements": par_engagement.get("engagements").cloned().unwrap_or(Value::Null),
                "non_actifs": par_engagement.get("non_actifs").cloned().unwrap_or(Value::Null),
                "du_mois": du_mois,
                "lectures": lus,
            })
        };
        // Le mois que l'ouverture montre se lit avec elle : ses paies validees se recalculent ici (`engagement::etats::du_mois`),
        // pas apres l'ecriture de l'etat, qui serait aussitot perime.
        if let Some(m) = etat.pointer("/lectures/mois-a-ouvrir/resultat/mois").and_then(Value::as_str).filter(|_| mois == OUVERTURE) {
            if let Ok(departs) = envois::journal::lu(&ctx.racine).map(|j| j.departs) {
                let _ = engagement::etats::du_mois(ctx, m, &departs);
            }
        }
        if empreinte(ctx, mois) == avant {
            if mois == OUVERTURE {
                ecrire(&ctx.racine, &etat);
            }
            return Ok(etat);
        }
        derniere = Some(etat);
    }
    Ok(derniere.unwrap_or(Value::Null))
}

/// Ecrit l'etat de l'ouverture d'un coup : un fichier a cote, puis le remplacement. Une lecture ne voit jamais un etat a
/// moitie ecrit. Une ecriture impossible ne change rien au resultat : la prochaine lecture refera.
fn ecrire(racine: &Path, etat: &Value) {
    let f = chemin(racine);
    let tmp = socle::ecriture::provisoire(&f);
    if std::fs::write(&tmp, etat.to_string()).is_ok() && std::fs::rename(&tmp, &f).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// L'etat de `cle` : celui de l'ouverture, ecrit, s'il est juste ; sinon refait.
pub fn lire(o: &HashMap<String, String>, ctx: &Contexte, cle: &str) -> R<Value> {
    if cle == OUVERTURE {
        if let Some(v) = garde(&ctx.racine, &empreinte(ctx, cle)) {
            return Ok(v);
        }
    }
    refaire(o, ctx, cle)
}

/// `etat-du-mois` tel que la page le recoit. LE COFFRE N'EST PAS DANS L'EMPREINTE (un `.bin`, qu'on ne lit pas) : ce
/// qu'il dit se relit a chaque fois.
fn du_mois(racine: &Path, etat: &Value) -> R<Value> {
    let mut du_mois = etat.get("du_mois").cloned().unwrap_or(Value::Null);
    if let Some(m) = du_mois.as_object_mut() {
        let pose = crate::compagnie::coffre::Coffre::du_poste(racine, "")
            .ouverture_exigee()
            .map_err(|x| Echec { classe: Some(x.classe), message: x.message })?;
        m.insert("mot_de_passe_pose".into(), json!(pose));
    }
    Ok(du_mois)
}

/// Tout l'ecran de `mois`, tel que la page le lit (`chargerMois`) : `lectures`, une entree par route, au format d'une
/// reponse de la page.
fn pour_la_page(racine: &Path, etat: &Value, brut: &Value) -> Value {
    use crate::interface::api::{en_page, Refus};
    let mut l = etat.get("lectures").and_then(Value::as_object).cloned().unwrap_or_default();
    l.insert("etat-par-engagement".into(), en_page(Ok(json!({ "mois": brut, "engagements": etat.get("engagements").cloned().unwrap_or(Value::Null), "non_actifs": etat.get("non_actifs").cloned().unwrap_or(Value::Null) }))));
    l.insert(
        "etat-du-mois".into(),
        en_page(du_mois(racine, etat).map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message))),
    );
    json!({ "mois": brut, "lectures": l })
}

/// `etat-par-engagement`, `etat-du-mois` et `mois`, servis par l'etat du mois : pour les deux premiers, le meme JSON que
/// `routes::etapes`.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let (brut, mois) = crate::routes::etapes::mois_de(charge)?;
    let etat = lire(o, ctx, &mois)?;
    match nom {
        "etat-par-engagement" => Ok(json!({ "mois": brut, "engagements": etat.get("engagements").cloned().unwrap_or(Value::Null), "non_actifs": etat.get("non_actifs").cloned().unwrap_or(Value::Null) })),
        "mois" => Ok(pour_la_page(&ctx.racine, &etat, &brut)),
        _ => du_mois(&ctx.racine, &etat),
    }
}

/// L'etat de l'OUVERTURE tel qu'il est ecrit, JUSTE OU NON : `(etat, juste)`. `None` s'il n'y en a pas, s'il ne se lit
/// pas, ou s'il vient d'un autre format ou d'un autre moteur (sa forme peut n'etre pas celle que la page lit : il se
/// refait avant de se montrer). Rien ne se calcule ici : une lecture de fichier, des dates de fichiers. Un mois n'a pas
/// d'etat ecrit (`FileDesEtats` le garde en memoire).
pub fn servir(racine: &Path, cle: &str, jour: &str) -> Option<(Value, bool)> {
    if cle != OUVERTURE {
        return None;
    }
    let v: Value = serde_json::from_slice(&std::fs::read(chemin(racine)).ok()?).ok()?;
    let de_ce_moteur = v.get("format").and_then(Value::as_u64) == Some(FORMAT)
        && v.get("moteur").and_then(Value::as_str) == Some(env!("PAIE_SOURCES"))
        && v.get("mois").and_then(Value::as_str) == Some(cle);
    if !de_ce_moteur {
        return None;
    }
    let juste = v.get("empreinte").and_then(Value::as_str) == Some(empreinte_de(racine, cle, jour).as_str());
    Some((v, juste))
}

/// Les mois dont l'etat se tient a jour : ceux que la saisie a touches, et le mois du jour.
fn mois_suivis(racine: &Path, jour: &str) -> Vec<String> {
    let mut mois = engagement::stock::mois_saisis(racine).unwrap_or_default();
    if jour.len() >= 7 {
        mois.push(jour[..7].to_string());
    }
    mois.sort();
    mois.dedup();
    mois
}

/// Le mois qu'ouvre l'etat garde de l'ouverture : celui que rend `mois-a-ouvrir`, sinon (refus, ou rien) le mois du jour.
fn mois_a_ouvrir(ouverture: &Value, jour: &str) -> String {
    ouverture
        .pointer("/lectures/mois-a-ouvrir/resultat/mois")
        .and_then(Value::as_str)
        .filter(|m| socle::date::mois_valide(m))
        .map(String::from)
        .unwrap_or_else(|| jour.get(..7).unwrap_or_default().to_string())
}

/// Le mois `m` decale de `pas` mois.
fn voisin(m: &str, pas: i32) -> Option<String> {
    let (a, n): (i32, i32) = (m.get(..4)?.parse().ok()?, m.get(5..7)?.parse().ok()?);
    let t = a * 12 + n - 1 + pas;
    Some(format!("{:04}-{:02}", t.div_euclid(12), t.rem_euclid(12) + 1))
}

type Rendu = Result<Value, (Option<&'static str>, String)>;

/// La cle du TABLEAU de l'administrateur dans la file : `config/.tableau.json`, refait pour une compagnie
/// rattachee (`administrateur::tableau_compagnie`), sous l'empreinte de l'ouverture (toute la compagnie).
pub const TABLEAU: &str = "tableau";

#[derive(Default)]
struct EtatDeLaFile {
    /// Les mois a verifier, dans l'ordre ou ils se feront.
    attente: VecDeque<String>,
    /// Le dernier numero de demande de chaque mois, et le numero de la demande que son dernier passage a servie.
    voulu: HashMap<String, u64>,
    fait: HashMap<String, (u64, Rendu)>,
    numero: u64,
    affiche: Option<String>,
    /// Ce que le fil refait en ce moment (une cle) : la phrase d'attente le dit (`phrase`).
    en_cours: Option<String>,
    arretee: bool,
}

/// LE FIL DES CALCULS EN FOND : n'importe quelle compagnie s'ouvre instantanement, meme a la premiere ouverture du jour.
/// UN fil pour TOUTES les compagnies que la fenetre tient (`FileDesEtats`, une par compagnie) : un
/// calcul a la fois sur le poste, jamais deux qui se disputent le processeur pendant qu'on ouvre une compagnie. Il sert
/// d'abord la compagnie OUVERTE (`devant`), puis les autres, chacune son tour (une cle chacune : leurs ouvertures
/// d'abord, puis les mois a ouvrir, puis le reste), dans l'ordre ou la fenetre les a donnees (la derniere ouverte
/// d'abord). Un calcul commence se finit : la compagnie qu'on ouvre attend au plus celui qui tourne.
///
/// CE FIL NE SORT PAS ET N'OUVRE AUCUN COFFRE (`fil_de_fond`) : il ne fait que lire les fichiers de la compagnie et ecrire
/// ses etats gardes. Une sortie reseau ou l'usage de la cle du poste ou d'un mot de passe y est refuse, quoi qu'une
/// lecture demande -- c'est ce qui permet de le faire tourner AVANT le mot de passe, des le lancement.
///
/// LE JOUR CHANGE (la fenetre ouverte passe minuit) : tout se reverifie (l'empreinte d'un etat porte la date du jour).
pub struct Fil {
    etat: Mutex<EtatDuFil>,
    reveil: Condvar,
}

#[derive(Default)]
struct EtatDuFil {
    files: Vec<Arc<FileDesEtats>>,
    devant: Option<Arc<FileDesEtats>>,
    /// La prochaine compagnie a servir parmi les autres (chacune son tour).
    tour: usize,
    /// Le jour de la derniere verification complete.
    jour: String,
    arrete: bool,
}

impl Fil {
    /// Un fil, qui tourne aussitot (il attend du travail).
    pub fn nouveau() -> Arc<Fil> {
        let f = Arc::new(Fil { etat: Mutex::new(EtatDuFil::default()), reveil: Condvar::new() });
        let fil = f.clone();
        crate::routes::fil_de_routes(move || {
            socle::fil_de_fond::marquer();
            fil.tourner();
        });
        f
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, EtatDuFil> {
        self.etat.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Du travail est arrive (une cle inscrite) : le fil se reveille. Pris sous le verrou du fil : le fil qui regardait
    /// les files a ce moment l'a lache pour attendre, il ne manque pas le signal.
    fn reveiller(&self) {
        let _g = self.verrou();
        self.reveil.notify_all();
    }

    /// La file de la compagnie `racine`, sur ce fil. `date` : la date des calculs (les tests).
    pub fn file(self: &Arc<Self>, racine: &Path, date: Option<&str>) -> Arc<FileDesEtats> {
        let mut o = HashMap::new();
        o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
        if let Some(d) = date {
            o.insert("date".to_string(), d.to_string());
        }
        let f = Arc::new(FileDesEtats {
            o,
            racine: racine.to_path_buf(),
            etat: Mutex::new(EtatDeLaFile::default()),
            signal: Condvar::new(),
            fil: self.clone(),
        });
        let mut g = self.verrou();
        g.files.push(f.clone());
        if g.jour.is_empty() {
            g.jour = f.jour();
        }
        drop(g);
        f
    }

    /// `file` passe devant toutes les autres (la compagnie qu'on ouvre) ; `None` : aucune (retour a l'accueil).
    pub fn devant(&self, file: Option<&Arc<FileDesEtats>>) {
        let mut g = self.verrou();
        g.devant = file.cloned();
        if let Some(f) = file {
            if let Some(i) = g.files.iter().position(|x| Arc::ptr_eq(x, f)) {
                let f = g.files.remove(i);
                g.files.insert(0, f);
            }
        }
        self.reveil.notify_all();
    }

    /// La compagnie que le fil sert en premier, s'il y en a une.
    pub fn compagnie_devant(&self) -> Option<PathBuf> {
        self.verrou().devant.as_ref().map(|f| f.racine.clone())
    }

    /// Le prochain travail : la compagnie devant d'abord, puis les autres, chacune son tour.
    fn choisir(g: &mut EtatDuFil) -> Option<(Arc<FileDesEtats>, String, u64)> {
        if let Some(d) = g.devant.clone() {
            if let Some((cle, n)) = d.prendre_travail() {
                return Some((d, cle, n));
            }
        }
        let n = g.files.len();
        for i in 0..n {
            let k = (g.tour + i) % n;
            let f = g.files[k].clone();
            if g.devant.as_ref().is_some_and(|d| Arc::ptr_eq(d, &f)) {
                continue;
            }
            if let Some((cle, numero)) = f.prendre_travail() {
                g.tour = k + 1;
                return Some((f, cle, numero));
            }
        }
        None
    }

    fn tourner(&self) {
        loop {
            let mut g = self.verrou();
            let (file, cle, numero) = loop {
                if g.arrete {
                    return;
                }
                if let Some(x) = Self::choisir(&mut g) {
                    break x;
                }
                let (h, attente) = self.reveil.wait_timeout(g, Duration::from_secs(60)).unwrap_or_else(|p| p.into_inner());
                g = h;
                // LE JOUR A CHANGE : tout se reverifie (hors du verrou du fil : `tout_verifier` le reprend pour reveiller).
                if attente.timed_out() {
                    if let Some(premiere) = g.files.first().cloned() {
                        let jour = premiere.jour();
                        if !jour.is_empty() && jour != g.jour {
                            g.jour = jour;
                            let files = g.files.clone();
                            drop(g);
                            for f in &files {
                                f.tout_verifier();
                            }
                            g = self.verrou();
                        }
                    }
                }
            };
            drop(g);
            let rendu = file.faire(&cle);
            file.rendre(cle, numero, rendu);
        }
    }
}

/// LA FILE DES ETATS D'UNE COMPAGNIE : changer de mois est instantane. Dans le processus de la fenetre, le fil des calculs (`Fil`) refait ses etats, un a la fois : jamais
/// deux calculs du meme mois en meme temps, pas un processus par mois (sous Windows chaque processus coute, l'antivirus
/// l'inspecte). Le calcul est celui de `moteur route` (`lire`, qui fait les memes routes par `routes::commande`), garde
/// d'une panique (`catch_unwind`) : une route qui tombe rend un refus, la fenetre reste.
///
/// Trois entrees : `demander` (un mois, devant ou a la suite), `tout_verifier` (au lancement de la fenetre, apres une
/// ecriture, a l'ouverture, apres chaque tour de veille : le mois affiche et ses voisins d'abord, puis l'ouverture, puis
/// les autres, du plus recent au plus ancien), et `attendre` (l'etat frais d'un mois, quand la page le demande ou
/// qu'aucun etat n'est garde). Un mois deja juste ne se recalcule pas : son passage dans la file ne coute que son
/// empreinte.
pub struct FileDesEtats {
    o: HashMap<String, String>,
    racine: PathBuf,
    etat: Mutex<EtatDeLaFile>,
    signal: Condvar,
    fil: Arc<Fil>,
}

impl FileDesEtats {
    /// La file de la compagnie `racine`, sur un fil a elle seule. `date` : la date des calculs (`moteur route --date`, les
    /// tests). La fenetre, elle, met toutes ses compagnies sur le meme fil (`Fil::file`).
    pub fn nouvelle(racine: &Path, date: Option<&str>) -> Arc<FileDesEtats> {
        Fil::nouveau().file(racine, date)
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, EtatDeLaFile> {
        self.etat.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// La compagnie de cette file.
    pub fn racine(&self) -> &Path {
        &self.racine
    }

    /// Le jour de l'empreinte : celui d'un `Contexte` (`aujourdhui`, que `date` ne regle pas).
    fn jour(&self) -> String {
        droit::contexte::aujourdhui_sans_note().map(|j| j.iso()).unwrap_or_default()
    }

    /// Le dernier etat connu de `cle`, JUSTE OU NON : celui de l'ouverture, ecrit (`servir`) ; celui d'un mois, que la file
    /// garde en memoire depuis son dernier passage. `None` : aucun.
    pub fn connu(&self, cle: &str, jour: &str) -> Option<(Value, bool)> {
        if cle == OUVERTURE {
            return servir(&self.racine, cle, jour);
        }
        let v = match self.verrou().fait.get(cle) {
            Some((_, Ok(v))) => v.clone(),
            _ => return None,
        };
        let juste = v.get("empreinte").and_then(Value::as_str) == Some(empreinte_de(&self.racine, cle, jour).as_str());
        Some((v, juste))
    }

    /// La prochaine cle a refaire, si la file en a une (sous le verrou du fil, `Fil::choisir`).
    fn prendre_travail(&self) -> Option<(String, u64)> {
        let mut e = self.verrou();
        if e.arretee {
            return None;
        }
        let m = e.attente.pop_front()?;
        let n = e.voulu.get(&m).copied().unwrap_or(0);
        e.en_cours = Some(m.clone());
        Some((m, n))
    }

    /// Un passage fini : son rendu, et qui l'attendait est libere.
    fn rendre(&self, mois: String, numero: u64, rendu: Rendu) {
        let mut e = self.verrou();
        e.en_cours = None;
        // L'OUVERTURE FAITE, le mois qu'elle ouvre passe devant : la page le demande juste apres.
        if mois == OUVERTURE && e.affiche.is_none() {
            if let Ok(v) = &rendu {
                let m = mois_a_ouvrir(v, &self.jour());
                Self::inscrire(&mut e, &m, true);
            }
        }
        e.fait.insert(mois, (numero, rendu));
        self.signal.notify_all();
    }

    /// L'etat juste de `cle` : le garde s'il l'est, sinon refait (`lire`), ici, sans processus.
    fn faire(&self, mois: &str) -> Rendu {
        if mois == TABLEAU {
            return self.faire_le_tableau();
        }
        if let Some((v, true)) = self.connu(mois, &self.jour()) {
            return Ok(v);
        }
        let o = &self.o;
        let _code = socle::fichiers::code_fige();
        let refaire = || {
            let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let ctx = crate::service::contexte(o).map_err(|m| Echec { classe: None, message: m })?;
                lire(o, &ctx, mois)
            }));
            // Un calcul tombe a pu laisser son fil sale (un registre ouvert, les nombres « fideles ») : le mois suivant
            // repart d'un fil propre.
            crate::routes::remettre_le_fil();
            rendu
        };
        // LE BANC D'ESSAI : ce que l'etat refait ecrit entre dans le scenario (`scenario::Enregistreur::fond`).
        #[cfg(feature = "atelier")]
        let rendu = match crate::atelier::scenario::enregistreur_actif() {
            Some(e) => e.fond(&self.racine, mois, refaire),
            None => refaire(),
        };
        #[cfg(not(feature = "atelier"))]
        let rendu = refaire();
        match rendu {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err((e.classe, e.message)),
            Err(_) => Err((Some("DefautDuLogiciel"), format!("Defaut du logiciel : l'etat de {} ne s'est pas refait (le calcul s'est arrete).", mois))),
        }
    }

    /// LE TABLEAU DE L'ADMINISTRATEUR pour cette compagnie (`config/.tableau.json`) : le garde s'il porte l'empreinte
    /// d'aujourd'hui de toute la compagnie (celle de l'ouverture), sinon refait.
    fn faire_le_tableau(&self) -> Rendu {
        let empreinte = empreinte_de(&self.racine, OUVERTURE, &self.jour());
        let garde: Option<Value> = std::fs::read(self.racine.join(crate::compagnie::administrateur::TABLEAU)).ok().and_then(|o| serde_json::from_slice(&o).ok());
        if let Some(v) = garde.filter(|v| v.get("empreinte").and_then(Value::as_str) == Some(empreinte.as_str())) {
            return Ok(v);
        }
        let o = &self.o;
        let _code = socle::fichiers::code_fige();
        let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let ctx = crate::service::contexte(o).map_err(|m| Echec { classe: None, message: m })?;
            crate::compagnie::administrateur::tableau_compagnie(o, &ctx)
        }));
        // Comme apres un mois (`faire`) : le travail suivant, d'une autre compagnie peut-etre, repart d'un fil propre.
        crate::routes::remettre_le_fil();
        match rendu {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err((e.classe, e.message)),
            Err(_) => Err((Some("DefautDuLogiciel"), "Defaut du logiciel : le tableau de la compagnie ne s'est pas refait.".into())),
        }
    }

    /// `mois` a verifier : devant (il est attendu), ou a la suite s'il n'est pas deja dans la file. Rend le numero de la
    /// demande : un passage qui commence apres elle la sert.
    pub fn demander(&self, mois: &str, devant: bool) -> u64 {
        let n = {
            let mut e = self.verrou();
            Self::inscrire(&mut e, mois, devant)
        };
        self.fil.reveiller();
        n
    }

    fn inscrire(e: &mut EtatDeLaFile, mois: &str, devant: bool) -> u64 {
        e.numero += 1;
        let n = e.numero;
        e.voulu.insert(mois.to_string(), n);
        let deja = e.attente.iter().position(|m| m == mois);
        match (deja, devant) {
            (Some(i), true) => {
                e.attente.remove(i);
                e.attente.push_front(mois.to_string());
            }
            (Some(_), false) => {}
            (None, true) => e.attente.push_front(mois.to_string()),
            (None, false) => e.attente.push_back(mois.to_string()),
        }
        n
    }

    /// TOUS LES MOIS SUIVIS A VERIFIER (au lancement, apres une ecriture, a l'ouverture, apres un tour de veille) : le mois
    /// affiche et ses voisins d'abord, puis l'ouverture (qui fait passer devant le mois qu'elle ouvre), puis -- pour une
    /// compagnie de l'administrateur -- son tableau, puis les autres mois, du plus recent au plus ancien. Ceux qui sont
    /// deja dans la file y restent a leur place, sauf les premiers, qui passent devant.
    pub fn tout_verifier(&self) {
        let suivis = mois_suivis(&self.racine, &self.jour());
        let tableau = crate::compagnie::administrateur::est_rattachee(&crate::compagnie::administrateur::installation_de(&self.racine), &self.racine);
        {
            let mut e = self.verrou();
            let mut premiers: Vec<String> = Vec::new();
            if let Some(a) = e.affiche.clone() {
                premiers.push(a.clone());
                for pas in [-1, 1] {
                    if let Some(v) = voisin(&a, pas).filter(|v| suivis.contains(v)) {
                        premiers.push(v);
                    }
                }
            }
            for m in suivis.iter().rev().filter(|m| !premiers.contains(m)) {
                Self::inscrire(&mut e, m, false);
            }
            if tableau {
                Self::inscrire(&mut e, TABLEAU, true);
            }
            // L'OUVERTURE : juste apres le mois affiche et ses voisins ; la premiere quand aucun mois n'est
            // encore affiche (au lancement, a l'ouverture d'une compagnie).
            Self::inscrire(&mut e, OUVERTURE, true);
            for m in premiers.iter().rev() {
                Self::inscrire(&mut e, m, true);
            }
        }
        self.fil.reveiller();
    }

    /// L'etat frais de `cle` : un passage commence apres cet appel, et l'on attend sa fin.
    pub fn attendre(&self, mois: &str) -> R<Value> {
        let n = self.demander(mois, true);
        let mut e = self.verrou();
        loop {
            if let Some((fait, rendu)) = e.fait.get(mois) {
                if *fait >= n {
                    return rendu.clone().map_err(|(classe, message)| Echec { classe, message });
                }
            }
            if e.arretee {
                return Err(Echec { classe: Some("ErreurOperation"), message: "La compagnie a ete fermee.".into() });
            }
            e = self.signal.wait(e).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// LE TABLEAU DE CETTE COMPAGNIE, frais : refait par le fil (devant), ou le garde s'il est juste.
    pub fn tableau(&self) -> R<Value> {
        self.attendre(TABLEAU)
    }

    /// L'ECRAN DE `mois` (la route `mois` de la fenetre) : le dernier etat connu, tout de suite, meme perime -- marque
    /// `perime`, et refait aussitot, devant tous les autres. `attendre` (la page qui veut l'etat frais) ou aucun etat
    /// garde : l'etat frais, une fois refait.
    pub fn ecran(&self, mois: &str, attendre: bool) -> R<Value> {
        if !socle::date::mois_valide(mois) {
            return Err(Echec { classe: Some("ErreurOperation"), message: format!("Mois illisible : {}.", mois) });
        }
        self.verrou().affiche = Some(mois.to_string());
        let brut = Value::String(mois.to_string());
        match self.connu(mois, &self.jour()) {
            Some((etat, true)) => return Ok(Self::marquer(pour_la_page(&self.racine, &etat, &brut), false)),
            Some((etat, false)) if !attendre => {
                self.demander(mois, true);
                return Ok(Self::marquer(pour_la_page(&self.racine, &etat, &brut), true));
            }
            _ => {}
        }
        let etat = self.attendre(mois)?;
        Ok(Self::marquer(pour_la_page(&self.racine, &etat, &brut), false))
    }

    /// L'etat garde `cle`, s'il y en a un (juste, si `juste` l'exige) ; sinon il est demande devant et attendu jusqu'a
    /// `limite` -- `Ok(None)` si elle passe avant lui. Le passage rend l'etat (juste) ou son refus.
    fn obtenir(&self, cle: &str, jour: &str, limite: Instant, juste: bool) -> R<Option<(Value, bool)>> {
        if let Some(x) = self.connu(cle, jour).filter(|(_, j)| *j || !juste) {
            return Ok(Some(x));
        }
        // Deja en route (la page qui redemande) : on attend ce passage-la, sans en demander un de plus.
        let en_route = {
            let e = self.verrou();
            (e.en_cours.as_deref() == Some(cle) || e.attente.iter().any(|m| m == cle)).then(|| e.voulu.get(cle).copied()).flatten()
        };
        let n = en_route.unwrap_or_else(|| self.demander(cle, true));
        let mut e = self.verrou();
        loop {
            if let Some((fait, rendu)) = e.fait.get(cle) {
                if *fait >= n {
                    return rendu.clone().map(|v| Some((v, true))).map_err(|(classe, message)| Echec { classe, message });
                }
            }
            if e.arretee {
                return Err(Echec { classe: Some("ErreurOperation"), message: "La compagnie a ete fermee.".into() });
            }
            let reste = limite.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return Ok(None);
            }
            e = self.signal.wait_timeout(e, reste).map(|(g, _)| g).unwrap_or_else(|p| p.into_inner().0);
        }
    }

    /// CE QUE FAIT LE MOTEUR, en une phrase (jamais une roue qui tourne sans dire pourquoi) : ce que le fil refait en ce
    /// moment.
    pub fn phrase(&self) -> String {
        match self.verrou().en_cours.clone() {
            Some(c) if c == OUVERTURE => "Lecture de la compagnie : sa configuration, ses échéances, ses rappels…".into(),
            Some(c) if c == TABLEAU => "Tableau des déclarations de la compagnie…".into(),
            Some(m) => format!("Calcul de {}…", socle::date::libelle_du_mois_ou_tel_quel(&m, socle::date::Graphie::Accentuee)),
            None => "Préparation de l'écran…".into(),
        }
    }

    /// L'OUVERTURE D'UNE COMPAGNIE (la route `ouverture` de la fenetre) : d'un coup, ce que la page lit en
    /// s'ouvrant -- les lectures de l'ouverture (`lectures`, une entree par route, au format d'une reponse de la page), le
    /// mois a ouvrir (`mois`) et l'ecran de ce mois (`ecran`, celui de la route `mois`). Les lectures sont JUSTES : la
    /// page ne les lit qu'une fois. L'ecran du mois est le dernier connu, tout de suite, marque `perime` s'il l'est
    /// (`ecran.perime`), et refait aussitot. `pret: true`.
    ///
    /// Sans lectures justes ou sans etat connu du mois (compagnie neuve, etat illisible ou d'un autre moteur, une
    /// ecriture depuis) : il se fait, et l'on attend au plus `patienter` ; passe ce delai, `{"pret": false, "phrase"}` dit
    /// ce que fait le moteur, et la page redemande. `frais` : les lectures de l'ouverture une fois refaites, sans l'ecran
    /// du mois (l'Historique qui se relit).
    ///
    /// `aujourdhui` : LE JOUR DU MOTEUR, celui que la page prend pour tout ce qu'elle date ou presente au jour (une
    /// date d'effet proposee, l'annee du taux AT/MP, le mois en cours de la grille) -- jamais l'horloge du navigateur :
    /// au banc d'essai le moteur a son temps simule (`temps`), et c'est le seul qui vaille (regle 2).
    pub fn ouverture(&self, patienter: Duration, frais: bool) -> R<Value> {
        let jour = self.jour();
        if frais {
            let v = self.attendre(OUVERTURE)?;
            let mois = mois_a_ouvrir(&v, &jour);
            return Ok(json!({"pret": true, "mois": mois, "aujourdhui": jour, "lectures": v.get("lectures").cloned().unwrap_or(Value::Null), "ecran": Value::Null}));
        }
        let limite = Instant::now() + patienter;
        let attente = || json!({"pret": false, "phrase": self.phrase()});
        let Some((ouv, _)) = self.obtenir(OUVERTURE, &jour, limite, true)? else { return Ok(attente()) };
        let mois = mois_a_ouvrir(&ouv, &jour);
        self.verrou().affiche = Some(mois.clone());
        let Some((etat, mois_juste)) = self.obtenir(&mois, &jour, limite, false)? else { return Ok(attente()) };
        // Refait en fond, devant : ses cartes sont ce qui se voit le plus.
        if !mois_juste {
            self.demander(&mois, true);
        }
        let ecran = Self::marquer(pour_la_page(&self.racine, &etat, &Value::String(mois.clone())), !mois_juste);
        Ok(json!({
            "pret": true,
            "mois": mois,
            "aujourdhui": jour,
            "lectures": ouv.get("lectures").cloned().unwrap_or(Value::Null),
            "ecran": ecran,
        }))
    }

    fn marquer(mut page: Value, perime: bool) -> Value {
        page["perime"] = Value::Bool(perime);
        page
    }

    /// La compagnie n'est plus tenue : son travail s'arrete apres le calcul en cours, et qui attendait est libere. Le fil
    /// qui n'a plus aucune compagnie s'arrete aussi (celui d'une file seule, `nouvelle`).
    pub fn arreter(&self) {
        self.fermer();
        let mut g = self.fil.verrou();
        g.files.retain(|f| !std::ptr::eq(Arc::as_ptr(f), self));
        if g.devant.as_ref().is_some_and(|d| std::ptr::eq(Arc::as_ptr(d), self)) {
            g.devant = None;
        }
        if g.files.is_empty() {
            g.arrete = true;
        }
        self.fil.reveil.notify_all();
    }

    /// LA COMPAGNIE S'EN VA (supprimee, ou deplacee quand on la garde au deblocage : `interface::suppression`) : sa file
    /// quitte le fil, qui continue de servir les autres -- meme s'il n'en reste aucune, la fenetre lui en donnera d'autres --,
    /// et l'on attend la fin du calcul qui tourne pour elle : rien ne s'ecrit plus dans son dossier ensuite.
    pub fn quitter(&self) {
        self.fermer();
        {
            let mut g = self.fil.verrou();
            g.files.retain(|f| !std::ptr::eq(Arc::as_ptr(f), self));
            if g.devant.as_ref().is_some_and(|d| std::ptr::eq(Arc::as_ptr(d), self)) {
                g.devant = None;
            }
        }
        let mut e = self.verrou();
        while e.en_cours.is_some() {
            e = self.signal.wait(e).unwrap_or_else(|p| p.into_inner());
        }
    }

    fn fermer(&self) {
        self.verrou().arretee = true;
        self.signal.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un fil qui ne tourne pas : on lui demande son prochain travail a la main (`Fil::choisir`).
    fn immobile() -> Arc<Fil> {
        Arc::new(Fil { etat: Mutex::new(EtatDuFil::default()), reveil: Condvar::new() })
    }

    fn suite(fil: &Fil, n: usize) -> Vec<(String, String)> {
        let mut g = fil.verrou();
        (0..n)
            .filter_map(|_| Fil::choisir(&mut g).map(|(f, cle, _)| (f.racine.file_name().unwrap().to_string_lossy().into_owned(), cle)))
            .collect()
    }

    /// UN FIL POUR TOUTES LES COMPAGNIES : la compagnie devant d'abord, tout son travail ; puis les autres,
    /// une cle chacune a son tour, dans l'ordre ou la fenetre les a donnees. Rien ne se fait deux fois.
    #[test]
    fn la_compagnie_ouverte_passe_devant_et_les_autres_se_servent_chacune_son_tour() {
        let fil = immobile();
        let (a, b, c) = (fil.file(Path::new("/x/a"), None), fil.file(Path::new("/x/b"), None), fil.file(Path::new("/x/c"), None));
        for f in [&a, &b, &c] {
            f.demander(OUVERTURE, false);
            f.demander("2026-09", false);
        }
        let paire = |c: &str, k: &str| (c.to_string(), k.to_string());
        assert_eq!(
            suite(&fil, 3),
            vec![paire("a", OUVERTURE), paire("b", OUVERTURE), paire("c", OUVERTURE)],
            "les ouvertures de toutes d'abord, chacune son tour"
        );
        // On ouvre c : tout son travail passe devant celui des autres.
        c.demander("2026-08", false);
        fil.devant(Some(&c));
        assert_eq!(suite(&fil, 2), vec![paire("c", "2026-09"), paire("c", "2026-08")]);
        assert_eq!(suite(&fil, 3), vec![paire("a", "2026-09"), paire("b", "2026-09")], "puis les autres ; rien deux fois");
        // Une compagnie fermee (`arreter`) ne se sert plus.
        a.demander("2026-07", false);
        a.arreter();
        assert!(suite(&fil, 1).is_empty());
    }
}
