//! L'ETAT GARDE D'UN MOIS (Leandre, 08/10/2026 : « la verite est dans les JSON, l'ecran ne fait que lire » ; « c'est a
//! l'ecriture que ca doit fonctionner » ; 09/10/2026 : « juste de la lecture »). Le plan : `PLAN-MOIS-INSTANTANE.md`.
//!
//! `paies/AAAA-MM.etat.json` porte TOUT ce que l'ecran d'un mois lit : l'etat de chaque paie (`etat-par-paie`), celui du
//! mois (`etat-du-mois`), et les lectures que la page faisait une a une (`LECTURES` : la saisie, les contrats en cours,
//! les embauches, les fins, les imprevus, l'apercu de la saisie enregistree). Il est refait dans
//! le processus de la fenetre, par UNE file (`FileDesEtats`, ticket #212) : apres chaque ecriture, a l'ouverture d'une
//! compagnie et apres chaque tour de veille, le mois affiche et ses voisins d'abord. La fenetre le sert elle-meme, sans
//! processus (`FileDesEtats::ecran`, route `mois` de `interface/api.rs`) : le dernier etat connu, TOUT DE SUITE, meme
//! perime -- marque `perime`, refait aussitot, et la page se repeint quand l'etat frais arrive.
//!
//! Il n'est pas une seconde verite : la verite reste la saisie, le calcul, les journaux, les retours et les fiches. L'etat
//! garde en est le resume, sous l'EMPREINTE de tout ce dont il est tire (`empreinte`). Depuis le 09/10/2026 l'empreinte ne
//! relit plus le contenu des sources : elle prend, pour chacune, sa date de modification et sa taille (une modification a
//! la main change la date). S'y ajoutent la date du jour (une echeance passee change ce qui se dit : un etat se refait
//! donc une fois par jour, et la file le refait des l'ouverture), le moteur lui-meme et l'emplacement du dossier
//! (l'etat porte des chemins : un dossier deplace se refait). Tant que l'empreinte est la meme, il se sert sans rien
//! recalculer ; si quoi que ce soit a bouge, par le logiciel ou a la main, il se refait une fois. Un etat garde ne ment
//! donc pas longtemps : servi perime, il le dit, et il se refait.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::contexte::Contexte;
use crate::retours::Echec;

type R<T> = Result<T, Echec>;

/// Le format du fichier : un autre se refait avant de se montrer. 3 : le moteur qui l'a ecrit (`moteur`).
const FORMAT: u64 = 3;

/// Les routes que l'etat garde sert.
pub const ROUTES: [&str; 3] = ["etat-par-paie", "etat-du-mois", "mois"];

/// Les lectures de l'ecran d'un mois qui ne sont pas les deux etats : la route, et si sa charge est `{"mois"}` (sinon
/// `{}`). La page les demandait une a une, chacune dans un processus ; elles sont faites ici, une fois, avec les memes
/// routes, et rendues telles quelles (resultat ou refus).
/// Les arrets n'y sont plus (15/10/2026) : la page lit les imprevus du mois, chaque arret avec ses obligations. La reprise
/// du journal non plus (20/10/2026) : elle se montre dans l'Historique, avec les envois et leurs retours.
const LECTURES: [(&str, bool); 5] = [
    ("saisie", true),
    ("contrats-en-cours", true),
    ("embauches-a-declarer", true),
    ("fins-de-contrat", true),
    ("imprevus", true),
];

pub fn chemin(racine: &Path, mois: &str) -> PathBuf {
    racine.join("paies").join(format!("{}.etat.json", mois))
}

/// Un mois `AAAA-MM`, rien d'autre : il nomme un fichier.
fn mois_valide(mois: &str) -> bool {
    let b = mois.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
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

/// CE QUE L'ECRAN D'UN MOIS NE LIT PAS, et qui bouge sans lui (ticket #212, 16/10/2026). Etabli par mesure (`strace` de la
/// route `mois` sur cinq mois de la grande fictive 3090 : aucun de ces fichiers n'est ouvert ni teste) et par le code
/// (chacun n'a de lecteur que hors des lectures du mois : `LECTURES`, `etat-par-paie`, `etat-du-mois`, l'apercu) :
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
const HORS_DE_L_ECRAN: [&str; 8] = [
    "config/.ouverture.json",
    "config/.sauvegarde.json",
    "config/diagnostic.json",
    "config/horloge.json",
    "config/filet-dpae.json",
    "config/.filet.verrou",
    "journal/signalements.json",
    "journal/incidents.json",
];

/// L'empreinte de tout ce dont l'etat de `mois` est tire, au jour `jour` (AAAA-MM-JJ). Les calculs des AUTRES mois n'y
/// sont pas : ils ne changent rien a celui-ci. Ni le coffre (un `.bin` : ce qu'il dit se relit a chaque lecture), ni ce
/// que l'ecran ne lit pas (`HORS_DE_L_ECRAN`).
pub fn empreinte_de(racine: &Path, mois: &str, jour: &str) -> String {
    let mut h = Sha256::new();
    for marque in [env!("PAIE_SOURCES").to_string(), jour.to_string(), absolu(racine), absolu(&crate::fichiers::dossier_du_code())] {
        h.update(marque.as_bytes());
        h.update([0]);
    }
    let calcul_du_mois = format!("paies/{}.calcul.json", mois);
    let sorties_du_mois = format!("sorties/{}/", mois);
    // La compagnie : ses saisies (et `paies/avant/`), sa configuration, ses journaux, ses imprevus, ses regularisations, son
    // historique importe, ses retours, et ce qui est sorti pour ce mois-ci.
    signer_dossier(&mut h, racine, &racine.join("paies"), true, &|n| {
        n.ends_with(".json") && !n.ends_with(".etat.json") && (!n.ends_with(".calcul.json") || n == calcul_du_mois)
    });
    signer_dossier(&mut h, racine, &racine.join("config"), true, &|n| {
        !n.ends_with(".bin") && !n.ends_with(".tmp") && !HORS_DE_L_ECRAN.contains(&n)
    });
    signer_dossier(&mut h, racine, &racine.join("journal"), false, &|n| n.ends_with(".json") && !HORS_DE_L_ECRAN.contains(&n));
    for d in ["imprevus", "regularisations", "historique", "retours"] {
        signer_dossier(&mut h, racine, &racine.join(d), true, &|n| !n.ends_with(".tmp"));
    }
    signer_dossier(&mut h, racine, &racine.join("sorties").join(mois), true, &|n| n.starts_with(&sorties_du_mois));
    // Le code livre : baremes, bases, modeles (une mise a jour les change sans changer le moteur).
    let code = crate::fichiers::dossier_du_code();
    signer_dossier(&mut h, &code, &code, true, &|_| true);
    h.finalize().iter().map(|o| format!("{:02x}", o)).collect()
}

pub fn empreinte(ctx: &Contexte, mois: &str) -> String {
    empreinte_de(&ctx.racine, mois, &ctx.aujourdhui.iso())
}

/// L'etat garde de `mois`, s'il est encore juste.
fn garde(racine: &Path, mois: &str, empreinte: &str) -> Option<Value> {
    let v: Value = serde_json::from_slice(&std::fs::read(chemin(racine, mois)).ok()?).ok()?;
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
    // L'APERCU : ce que la page demandait des qu'un mois s'affichait (`calculerApercu`), sur la saisie qu'elle venait de
    // lire. Le meme calcul, sur la saisie enregistree ; la page ne le redemande que quand on modifie la saisie.
    // La saisie telle qu'elle est sur le disque (`{"saisie": mois}`) : celle dont les cartes figees ont pris l'empreinte, et
    // dont le calcul du mois est garde (`paies/AAAA-MM.calcul.json`).
    if l.get("saisie").and_then(|s| s.pointer("/resultat/existe")).is_some_and(|e| e == &Value::Bool(true)) {
        l.insert("apercu".into(), lecture(o, "apercu", json!({"saisie": mois, "lignes": false})));
    }
    Value::Object(l)
}

/// Refait l'etat de `mois` et l'ecrit. Le calcul du mois peut se refaire pendant la lecture (le brut de chaque paie) :
/// l'empreinte d'apres n'est alors plus celle d'avant, et un second passage, sans calcul cette fois, la retrouve. Si elle
/// bouge encore (une autre ecriture passait), rien ne s'ecrit : la prochaine lecture refera.
fn refaire(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<Value> {
    let charge = json!({ "mois": mois });
    let mut derniere = None;
    for _ in 0..2 {
        let avant = empreinte(ctx, mois);
        let par_paie = crate::routes_mois::route(o, ctx, "etat-par-paie", &charge)?;
        let du_mois = crate::routes_mois::route(o, ctx, "etat-du-mois", &charge)?;
        let etat = json!({
            "format": FORMAT,
            "moteur": env!("PAIE_SOURCES"),
            "mois": mois,
            "empreinte": avant,
            "paies": par_paie.get("paies").cloned().unwrap_or(Value::Null),
            "du_mois": du_mois,
            "lectures": lectures(o, mois),
        });
        if empreinte(ctx, mois) == avant {
            ecrire(&ctx.racine, mois, &etat);
            return Ok(etat);
        }
        derniere = Some(etat);
    }
    Ok(derniere.unwrap_or(Value::Null))
}

/// Ecrit d'un coup : un fichier a cote, puis le remplacement. Une lecture ne voit jamais un etat a moitie ecrit. Une
/// ecriture impossible ne change rien au resultat : la prochaine lecture refera.
fn ecrire(racine: &Path, mois: &str, etat: &Value) {
    let f = chemin(racine, mois);
    let tmp = crate::ecriture::provisoire(&f);
    if std::fs::write(&tmp, etat.to_string()).is_ok() && std::fs::rename(&tmp, &f).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// L'etat de `mois` : le garde s'il est juste, sinon refait.
pub fn lire(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) -> R<Value> {
    match garde(&ctx.racine, mois, &empreinte(ctx, mois)) {
        Some(v) => Ok(v),
        None => refaire(o, ctx, mois),
    }
}

/// `etat-du-mois` tel que la page le recoit. LE COFFRE N'EST PAS DANS L'EMPREINTE (un `.bin`, qu'on ne lit pas) : ce
/// qu'il dit se relit a chaque fois.
fn du_mois(racine: &Path, etat: &Value) -> R<Value> {
    let mut du_mois = etat.get("du_mois").cloned().unwrap_or(Value::Null);
    if let Some(m) = du_mois.as_object_mut() {
        let pose = crate::coffre::Coffre::du_poste(racine, "")
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
    l.insert("etat-par-paie".into(), en_page(Ok(json!({ "mois": brut, "paies": etat.get("paies").cloned().unwrap_or(Value::Null) }))));
    l.insert(
        "etat-du-mois".into(),
        en_page(du_mois(racine, etat).map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message))),
    );
    json!({ "mois": brut, "lectures": l })
}

/// `etat-par-paie`, `etat-du-mois` et `mois`, servis par l'etat garde : pour les deux premiers, le meme JSON que
/// `routes_mois`.
pub fn route(o: &HashMap<String, String>, ctx: &Contexte, nom: &str, charge: &Value) -> R<Value> {
    let (brut, mois) = crate::routes_mois::mois_de(charge)?;
    let etat = lire(o, ctx, &mois)?;
    match nom {
        "etat-par-paie" => Ok(json!({ "mois": brut, "paies": etat.get("paies").cloned().unwrap_or(Value::Null) })),
        "mois" => Ok(pour_la_page(&ctx.racine, &etat, &brut)),
        _ => du_mois(&ctx.racine, &etat),
    }
}

/// L'etat garde de `mois` tel qu'il est sur le disque, JUSTE OU NON (ticket #212) : `(etat, juste)`. `None` s'il n'y en a
/// pas, s'il ne se lit pas, ou s'il vient d'un autre format ou d'un autre moteur (sa forme peut n'etre pas celle que la
/// page lit : il se refait avant de se montrer). Rien ne se calcule ici : une lecture de fichier, des dates de fichiers.
pub fn servir(racine: &Path, mois: &str, jour: &str) -> Option<(Value, bool)> {
    if !mois_valide(mois) {
        return None;
    }
    let v: Value = serde_json::from_slice(&std::fs::read(chemin(racine, mois)).ok()?).ok()?;
    let de_ce_moteur = v.get("format").and_then(Value::as_u64) == Some(FORMAT)
        && v.get("moteur").and_then(Value::as_str) == Some(env!("PAIE_SOURCES"))
        && v.get("mois").and_then(Value::as_str) == Some(mois);
    if !de_ce_moteur {
        return None;
    }
    let juste = v.get("empreinte").and_then(Value::as_str) == Some(empreinte_de(racine, mois, jour).as_str());
    Some((v, juste))
}

/// Les mois dont l'etat se tient a jour : ceux qui ont une saisie ou un etat garde, et le mois du jour.
fn mois_suivis(racine: &Path, jour: &str) -> Vec<String> {
    let mut mois: Vec<String> = std::fs::read_dir(racine.join("paies"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_str()?.to_string();
            let m = n.strip_suffix(".etat.json").or_else(|| n.strip_suffix(".json"))?;
            mois_valide(m).then(|| m.to_string())
        })
        .collect();
    if jour.len() >= 7 {
        mois.push(jour[..7].to_string());
    }
    mois.sort();
    mois.dedup();
    mois
}

/// Le mois `m` decale de `pas` mois.
fn voisin(m: &str, pas: i32) -> Option<String> {
    let (a, n): (i32, i32) = (m.get(..4)?.parse().ok()?, m.get(5..7)?.parse().ok()?);
    let t = a * 12 + n - 1 + pas;
    Some(format!("{:04}-{:02}", t.div_euclid(12), t.rem_euclid(12) + 1))
}

type Rendu = Result<Value, (Option<&'static str>, String)>;

#[derive(Default)]
struct EtatDeLaFile {
    /// Les mois a verifier, dans l'ordre ou ils se feront.
    attente: VecDeque<String>,
    /// Le dernier numero de demande de chaque mois, et le numero de la demande que son dernier passage a servie.
    voulu: HashMap<String, u64>,
    fait: HashMap<String, (u64, Rendu)>,
    numero: u64,
    affiche: Option<String>,
    arretee: bool,
}

/// LA FILE DES ETATS (ticket #212, Leandre, 16/10/2026 : « quand je change de mois c'est pas instantane »). Dans le
/// processus de la fenetre, UN fil refait les etats des mois, un a la fois : jamais deux calculs du meme mois en meme
/// temps, pas un processus par mois (sous Windows chaque processus coute, l'antivirus l'inspecte). Le calcul est celui
/// de `paie route` (`lire`, qui fait les memes routes par `routes::commande`), garde d'une panique (`catch_unwind`) :
/// une route qui tombe rend un refus, la fenetre reste.
///
/// Trois entrees : `demander` (un mois, devant ou a la suite), `tout_verifier` (apres une ecriture, a l'ouverture, apres
/// chaque tour de veille : le mois affiche et ses voisins d'abord, puis les autres, du plus recent au plus ancien), et
/// `attendre` (l'etat frais d'un mois, quand la page le demande ou qu'aucun etat n'est garde). Un mois deja juste ne se
/// recalcule pas : son passage dans la file ne coute que son empreinte.
pub struct FileDesEtats {
    o: HashMap<String, String>,
    racine: PathBuf,
    etat: Mutex<EtatDeLaFile>,
    signal: Condvar,
}

impl FileDesEtats {
    /// La file de la compagnie `racine`, et son fil. `date` : la date des calculs (`paie route --date`, les tests).
    pub fn nouvelle(racine: &Path, date: Option<&str>) -> Arc<FileDesEtats> {
        let mut o = HashMap::new();
        o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
        if let Some(d) = date {
            o.insert("date".to_string(), d.to_string());
        }
        let f = Arc::new(FileDesEtats { o, racine: racine.to_path_buf(), etat: Mutex::new(EtatDeLaFile::default()), signal: Condvar::new() });
        let fil = f.clone();
        crate::routes::fil_de_routes(move || fil.tourner());
        f
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, EtatDeLaFile> {
        self.etat.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Le jour de l'empreinte : celui d'un `Contexte` (`aujourdhui`, que `date` ne regle pas).
    fn jour(&self) -> String {
        crate::contexte::aujourdhui_sans_note().map(|j| j.iso()).unwrap_or_default()
    }

    /// Le fil : un mois a la fois, jusqu'a l'arret.
    fn tourner(&self) {
        loop {
            let (mois, numero) = {
                let mut e = self.verrou();
                loop {
                    if e.arretee {
                        return;
                    }
                    if let Some(m) = e.attente.pop_front() {
                        let n = e.voulu.get(&m).copied().unwrap_or(0);
                        break (m, n);
                    }
                    e = self.signal.wait(e).unwrap_or_else(|p| p.into_inner());
                }
            };
            let rendu = self.faire(&mois);
            let mut e = self.verrou();
            e.fait.insert(mois, (numero, rendu));
            self.signal.notify_all();
        }
    }

    /// L'etat juste de `mois` : le garde s'il l'est, sinon refait (`lire`), ici, sans processus.
    fn faire(&self, mois: &str) -> Rendu {
        if let Some((v, true)) = servir(&self.racine, mois, &self.jour()) {
            return Ok(v);
        }
        let o = &self.o;
        let _code = crate::fichiers::code_fige();
        let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let ctx = crate::service::contexte(o).map_err(|m| Echec { classe: None, message: m })?;
            lire(o, &ctx, mois)
        }));
        // Un calcul tombe a pu laisser son fil sale (un registre ouvert, les nombres « fideles ») : le mois suivant
        // repart d'un fil propre.
        crate::routes::remettre_le_fil();
        match rendu {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err((e.classe, e.message)),
            Err(_) => Err((Some("DefautDuLogiciel"), format!("Defaut du logiciel : l'etat de {} ne s'est pas refait (le calcul s'est arrete).", mois))),
        }
    }

    /// `mois` a verifier : devant (il est attendu), ou a la suite s'il n'est pas deja dans la file. Rend le numero de la
    /// demande : un passage qui commence apres elle la sert.
    pub fn demander(&self, mois: &str, devant: bool) -> u64 {
        let mut e = self.verrou();
        let n = Self::inscrire(&mut e, mois, devant);
        self.signal.notify_all();
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

    /// TOUS LES MOIS SUIVIS A VERIFIER (apres une ecriture, a l'ouverture, apres un tour de veille) : le mois affiche et
    /// ses voisins d'abord, puis les autres, du plus recent au plus ancien. Ceux qui sont deja dans la file y restent a
    /// leur place, sauf les premiers, qui passent devant.
    pub fn tout_verifier(&self) {
        let suivis = mois_suivis(&self.racine, &self.jour());
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
        for m in premiers.iter().rev() {
            Self::inscrire(&mut e, m, true);
        }
        self.signal.notify_all();
    }

    /// L'etat frais de `mois` : un passage commence apres cet appel, et l'on attend sa fin.
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

    /// L'ECRAN DE `mois` (la route `mois` de la fenetre) : le dernier etat connu, tout de suite, meme perime -- marque
    /// `perime`, et refait aussitot, devant tous les autres. `attendre` (la page qui veut l'etat frais) ou aucun etat
    /// garde : l'etat frais, une fois refait.
    pub fn ecran(&self, mois: &str, attendre: bool) -> R<Value> {
        if !mois_valide(mois) {
            return Err(Echec { classe: Some("ErreurOperation"), message: format!("Mois illisible : {}.", mois) });
        }
        self.verrou().affiche = Some(mois.to_string());
        let brut = Value::String(mois.to_string());
        match servir(&self.racine, mois, &self.jour()) {
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

    fn marquer(mut page: Value, perime: bool) -> Value {
        page["perime"] = Value::Bool(perime);
        page
    }

    /// La compagnie se ferme : le fil s'arrete apres le mois en cours, et qui attendait est libere.
    pub fn arreter(&self) {
        self.verrou().arretee = true;
        self.signal.notify_all();
    }
}
