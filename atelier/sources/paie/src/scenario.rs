//! LE SCENARIO D'UNE COMPAGNIE (#236, phase 2) : ce qu'on a fait dans l'interface, geste par geste, et son rejeu sans
//! navigateur.
//!
//! UN SCENARIO EST UN DOSSIER :
//! - `gestes.jsonl` : un geste par ligne, dans l'ordre ou ils ont eu lieu. Chacun porte son rang `n` et son instant
//!   `quand` (secondes UTC, le temps du moteur, `temps`), plus `heure` (le meme, lisible, a Paris). Les gestes :
//!   - `{"geste": "debut", "jour", "logiciel"}` : le banc commence (rien a rejouer) ;
//!   - `{"geste": "ajouter", "nom", "compagnie"}` : le « + » de l'accueil d'un logiciel bride (`compagnies::ajouter_dans`),
//!     le decor de depart ; `compagnie` est le dossier qu'il a cree, relatif a l'installation (`fictives/ajoutee-…`) ;
//!   - `{"geste": "simuler", "compagnie"}` : le geste d'atelier qui fait d'une fictive ajoutee une fictive aux envois
//!     simules (`rendre_simulee`) -- sans lui, ses envois sont refuses et elle ne peut pas vivre sa vie ;
//!   - `{"geste": "jour", "jour"}` : le calendrier avance (le banc) ; l'horloge de la fictive joue alors ce qui etait du,
//!     par la route `ouverture` qui suit (enregistree comme toute route) ;
//!   - `{"route", "charge", "compagnie", "ecrit", "ok"}` : une route du moteur, telle que la fenetre l'a servie (la page,
//!     la veille, le fil de fond : `interface::api::route`), avec la charge que le moteur a recue. `ecrit` : la route a
//!     change un fichier de la compagnie (son empreinte, avant et apres, hors caches) ; `ok` : elle a reussi. `pieces` :
//!     ce qu'on avait depose dans un dossier que la route lit (l'import), recopie dans le scenario.
//! - `pieces/<sha256>` : le contenu de chaque fichier depose, une fois (un PDF de bulletins, une DSN, un contrat...).
//!   Le scenario se rejoue ainsi sans rien d'exterieur : ni `/tmp`, ni le poste ou on l'a enregistre.
//!
//! L'ENREGISTREUR (`Enregistreur`) n'existe que dans le binaire de l'atelier (`paie-atelier`, `--features atelier`) :
//! `paie-atelier interface --enregistrer <dossier> --jour J` (le banc, `atelier/outils/banc/`). Le binaire livre n'en a
//! pas une ligne.
//!
//! LE REJEU (`rejouer`) est dans le moteur ordinaire : les essais (`tests/outils/rejeu.rs`) et l'exemple `vie_fictive`
//! l'appellent dans leur processus. Chaque geste se rejoue a son instant, fige (`temps::figer`), les aleas en suite fixe
//! (`temps::aleas_fixes`) : deux rejeux ecrivent les memes octets. Les routes qui n'ont rien ecrit s'ecartent par defaut
//! (une lecture ne change pas la compagnie ; les garder coute le temps de les recalculer) ; `OptionsRejeu::lectures` les
//! rejoue aussi.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// Le fichier des gestes, dans le dossier du scenario.
pub const GESTES: &str = "gestes.jsonl";
/// Le dossier des pieces deposees, par empreinte.
pub const PIECES: &str = "pieces";
/// La version du format.
pub const FORMAT: u64 = 1;

/// Les cles d'une charge qui nomment un dossier ou un fichier que la route LIT (une boite aux lettres d'import, un export
/// d'un autre logiciel) : ce qu'il contient entre dans le scenario.
#[cfg(feature = "atelier")]
const CLES_DE_DEPOT: [&str; 4] = ["dossier", "export", "fichier", "chemin"];

/// Les routes dont la cle `dossier` vide (ou absente) veut dire `import/` de la compagnie (`routes_importation::dossier_import`).
#[cfg(feature = "atelier")]
fn lit_la_boite_aux_lettres(route: &str) -> bool {
    route.starts_with("importation") || route == "importer"
}

/// LE MARQUEUR D'UNE FICTIVE AUX ENVOIS SIMULES, pose sur une fictive ajoutee (le geste `simuler`) : `copie_de` dit
/// « compagnie fictive … » (`compagnies::est_fictive`) sans la marque `ajoutee` -- ses envois sont joues
/// (`routes_accueil::simulation_active`) et son horloge joue ce qui etait du (`horloge`). Le nom (« … sur le banc
/// d'essai ») n'est celui d'aucune fictive livree : la livraison des fictives (`maj.rs`) ne la prend jamais pour une
/// des siennes. Refuse hors d'une fictive ajoutee.
pub fn rendre_simulee(racine: &Path) -> Result<(), String> {
    if !crate::interface::compagnies::est_ajoutee(racine) {
        return Err(format!("{} n'est pas une compagnie ajoutee par le « + » : rien a changer.", racine.display()));
    }
    let nom = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    crate::ecriture::ecrire_json(
        &racine.join("COPIE-DE-DEVELOPPEMENT.json"),
        &json!({
            "copie_de": format!("{} {} sur le banc d'essai", crate::interface::compagnies::MARQUE_FICTIVE, nom),
            "quand": crate::incidents::maintenant(),
            "_quoi": "Une compagnie ajoutee par le « + », devenue fictive aux envois SIMULES sur le banc d'essai de l'atelier (scenario.rs, geste `simuler`) : ses DPAE, DSN et courriels sont joues, rien ne part.",
        }),
        2,
    )
}

/// Le chemin de `racine` relatif a l'installation `inst` (barres droites), ou le chemin entier s'il n'y est pas.
pub fn relatif(inst: &Path, racine: &Path) -> String {
    let (i, r) = (std::fs::canonicalize(inst).unwrap_or_else(|_| inst.to_path_buf()), std::fs::canonicalize(racine).unwrap_or_else(|_| racine.to_path_buf()));
    r.strip_prefix(&i).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| r.display().to_string())
}

/// L'heure lisible d'un instant, a Paris.
fn heure(t: f64) -> String {
    crate::paris::iso_secondes(t)
}

/// Les fichiers d'un dossier, relatifs a lui (barres droites), dans l'ordre des noms.
#[cfg(feature = "atelier")]
fn fichiers_de(dossier: &Path) -> Vec<(String, PathBuf)> {
    fn parcourir(base: &Path, d: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(it) = std::fs::read_dir(d) else { return };
        for e in it.flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(base, &p, out);
            } else if let Ok(r) = p.strip_prefix(base) {
                out.push((r.to_string_lossy().replace('\\', "/"), p.clone()));
            }
        }
    }
    let mut out = Vec::new();
    parcourir(dossier, dossier, &mut out);
    out.sort();
    out
}

// ------------------------------------------------------------------ le rejeu

/// Ce qu'on demande au rejeu.
#[derive(Debug, Clone, Default)]
pub struct OptionsRejeu {
    /// Rejouer aussi les routes qui n'avaient rien ecrit.
    pub lectures: bool,
    /// S'arreter au geste de ce rang (compris).
    pub jusqu_a: Option<u64>,
}

/// Ce que le rejeu a fait.
#[derive(Debug, Clone, Default)]
pub struct Rejeu {
    /// Les compagnies rejouees, dans l'ordre de leur « + » : (dossier relatif a l'installation, racine).
    pub compagnies: Vec<(String, PathBuf)>,
    pub routes_rejouees: usize,
    pub lectures_ecartees: usize,
    /// Les gestes sur une compagnie que le scenario n'a pas ajoutee (une fictive livree ouverte pour voir) : ecartes.
    pub autres_compagnies: usize,
    /// Ce que l'horloge des fictives a joue pendant le rejeu, en phrases, precedees de l'heure du geste.
    pub horloge: Vec<String>,
    /// L'instant du dernier geste rejoue (secondes UTC) : le jour ou l'on lit la compagnie qui en sort.
    pub fin: Option<f64>,
    /// Ce qui ne s'est pas passe comme a l'enregistrement : une route reussie qui refuse au rejeu, ou l'inverse.
    pub ecarts: Vec<String>,
}

/// Les gestes d'un scenario, lus.
pub fn lire(scenario: &Path) -> Result<Vec<Value>, String> {
    let f = scenario.join(GESTES);
    let texte = std::fs::read_to_string(&f).map_err(|e| format!("{} : {}", f.display(), e))?;
    texte
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("{} ligne {} : {}", f.display(), i + 1, e)))
        .collect()
}

/// Pose, avant la route, ce qu'on avait depose : un dossier de la compagnie se remet tel qu'il etait ; un dossier
/// hors d'elle se recree dans `travail/<rang>/<cle>` et la charge le nomme. Rend la charge a donner au moteur.
fn poser_les_pieces(scenario: &Path, racine: &Path, travail: &Path, geste: &Value) -> Result<Value, String> {
    let mut charge = geste.get("charge").cloned().unwrap_or_else(|| json!({}));
    let Some(pieces) = geste.get("pieces").and_then(Value::as_object) else { return Ok(charge) };
    let n = geste.get("n").and_then(Value::as_u64).unwrap_or(0);
    for (cle, p) in pieces {
        let fichiers = p.get("fichiers").and_then(Value::as_object).cloned().unwrap_or_default();
        let (cible, est_fichier) = match (p.get("dans_la_compagnie").and_then(Value::as_str), p.get("fichier").and_then(Value::as_bool) == Some(true)) {
            (Some(rel), f) => (racine.join(rel), f),
            (None, f) => (travail.join(n.to_string()).join(cle), f),
        };
        if est_fichier {
            // Un fichier seul : la cle `.` de `fichiers`.
            let sha = fichiers.get(".").and_then(Value::as_str).ok_or("piece sans contenu")?;
            let dest = if p.get("dans_la_compagnie").is_some() { cible.clone() } else { cible.join(p.get("nom").and_then(Value::as_str).unwrap_or("piece")) };
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::copy(scenario.join(PIECES).join(sha), &dest).map_err(|e| format!("piece {} : {}", sha, e))?;
            if p.get("dans_la_compagnie").is_none() {
                charge[cle.as_str()] = json!(dest.display().to_string());
            }
            continue;
        }
        let _ = std::fs::remove_dir_all(&cible);
        std::fs::create_dir_all(&cible).map_err(|e| format!("{} : {}", cible.display(), e))?;
        for (rel, sha) in &fichiers {
            let dest = cible.join(rel);
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let sha = sha.as_str().ok_or("piece sans empreinte")?;
            std::fs::copy(scenario.join(PIECES).join(sha), &dest).map_err(|e| format!("piece {} : {}", sha, e))?;
        }
        if p.get("dans_la_compagnie").is_none() {
            charge[cle.as_str()] = json!(cible.display().to_string());
        }
    }
    Ok(charge)
}

/// REJOUE le scenario `scenario` dans l'installation `inst` (un dossier vide, ou qui ne porte pas encore les compagnies
/// du scenario) : le decor du « + », chaque geste a son instant, et rien d'autre. Le temps du moteur est simule le temps
/// du rejeu, puis rendu a la montre. `travail` recoit les dossiers deposes hors de la compagnie.
pub fn rejouer(scenario: &Path, inst: &Path, options: &OptionsRejeu) -> Result<Rejeu, String> {
    let gestes = lire(scenario)?;
    std::fs::create_dir_all(inst).map_err(|e| format!("{} : {}", inst.display(), e))?;
    let travail = inst.join("pieces-deposees");
    crate::temps::liberer();
    crate::temps::aleas_fixes();
    let rendu = rejouer_les_gestes(scenario, inst, &travail, &gestes, options);
    crate::temps::liberer();
    rendu
}

fn rejouer_les_gestes(scenario: &Path, inst: &Path, travail: &Path, gestes: &[Value], options: &OptionsRejeu) -> Result<Rejeu, String> {
    let mut r = Rejeu::default();
    // Le dossier enregistre -> la racine du rejeu.
    let mut racines: BTreeMap<String, PathBuf> = BTreeMap::new();
    for g in gestes {
        let n = g.get("n").and_then(Value::as_u64).unwrap_or(0);
        if options.jusqu_a.is_some_and(|j| n > j) {
            break;
        }
        if let Some(t) = g.get("quand").and_then(Value::as_f64) {
            crate::temps::figer(t);
            r.fin = Some(t);
        }
        let compagnie = g.get("compagnie").and_then(Value::as_str).unwrap_or("");
        match g.get("geste").and_then(Value::as_str) {
            Some("ajouter") => {
                let nom = g.get("nom").and_then(Value::as_str).ok_or(format!("geste {} : `ajouter` sans nom", n))?;
                let c = crate::interface::compagnies::ajouter_dans(inst, nom).map_err(|m| format!("geste {} : « + » refuse : {}", n, m))?;
                let rel = relatif(inst, &c.racine);
                if rel != compagnie {
                    r.ecarts.push(format!("geste {} : le « + » a cree {} (enregistre : {})", n, rel, compagnie));
                }
                racines.insert(compagnie.to_string(), c.racine.clone());
                r.compagnies.push((compagnie.to_string(), c.racine));
            }
            Some("simuler") => {
                let racine = racines.get(compagnie).ok_or(format!("geste {} : `simuler` sur {}, que le scenario n'a pas ajoutee", n, compagnie))?;
                rendre_simulee(racine).map_err(|m| format!("geste {} : {}", n, m))?;
            }
            Some(_) => {}
            None => {
                let route = g.get("route").and_then(Value::as_str).ok_or(format!("geste {} : ni `geste` ni `route`", n))?;
                let Some(racine) = racines.get(compagnie) else {
                    r.autres_compagnies += 1;
                    continue;
                };
                if g.get("ecrit") != Some(&Value::Bool(true)) && !options.lectures {
                    r.lectures_ecartees += 1;
                    continue;
                }
                let charge = poser_les_pieces(scenario, racine, travail, g)?;
                let mut o = std::collections::HashMap::new();
                o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
                let rep = crate::routes::dans_la_fenetre(&o, route, &charge);
                r.routes_rejouees += 1;
                let ok = rep.get("ok") == Some(&Value::Bool(true));
                // Ce que l'horloge de la fictive a joue (la route `horloge-rattraper`, ou la veille par `ouverture`), date du geste.
                let jouees = match route {
                    "horloge-rattraper" => rep.pointer("/resultat"),
                    "ouverture" => rep.pointer("/resultat/horloge"),
                    _ => None,
                };
                for p in jouees.and_then(Value::as_array).into_iter().flatten() {
                    r.horloge.push(format!("{} {}", heure(g.get("quand").and_then(Value::as_f64).unwrap_or(0.0)), crate::etats::py_str(p)));
                }
                let attendu = g.get("ok") == Some(&Value::Bool(true));
                if ok != attendu {
                    r.ecarts.push(format!(
                        "geste {} ({} {}) : {} au rejeu, {} a l'enregistrement{}",
                        n,
                        heure(g.get("quand").and_then(Value::as_f64).unwrap_or(0.0)),
                        route,
                        if ok { "reussit" } else { "refuse" },
                        if attendu { "reussi" } else { "refuse" },
                        rep.get("erreur").map(|e| format!(" : {}", crate::etats::py_str(e))).unwrap_or_default()
                    ));
                }
            }
        }
    }
    Ok(r)
}

// ------------------------------------------------------------------ l'enregistreur (l'atelier seulement)

#[cfg(feature = "atelier")]
pub use enregistreur::Enregistreur;

#[cfg(feature = "atelier")]
mod enregistreur {
    use super::*;
    use serde_json::Map;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    /// L'enregistreur de la fenetre de l'atelier : un seul par processus, pose au lancement (`demarrer`).
    pub struct Enregistreur {
        dossier: PathBuf,
        /// L'installation : les compagnies se nomment relativement a elle.
        inst: PathBuf,
        /// Un geste a la fois, de bout en bout : la route, son empreinte avant et apres, sa ligne. Le temps est fige
        /// pendant ce temps-la (`temps::figer`) ; l'ordre du fichier est celui ou les gestes ont eu lieu.
        tour: Mutex<u64>,
    }

    static ENREGISTREUR: OnceLock<Enregistreur> = OnceLock::new();

    /// L'enregistreur de ce processus, s'il y en a un.
    pub fn actif() -> Option<&'static Enregistreur> {
        ENREGISTREUR.get()
    }

    /// UN MOT DE PASSE TAPE N'ENTRE JAMAIS DANS UN SCENARIO : la valeur de toute cle `motdepasse` d'une charge est
    /// retiree. Au rejeu, une fictive aux envois simules n'en a pas besoin (`depot_acte::verifier_preuve`).
    pub(super) fn sans_mot_de_passe(v: &Value) -> Value {
        match v {
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), if k.starts_with("motdepasse") && x.is_string() { json!("") } else { sans_mot_de_passe(x) }))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(sans_mot_de_passe).collect()),
            x => x.clone(),
        }
    }

    /// Ce qu'une empreinte de la compagnie ne regarde pas : les caches que la fenetre refait en fond, et son journal.
    fn hors_empreinte(rel: &str) -> bool {
        rel == "journal/interface.log"
            || rel.ends_with(".etat.json")
            || rel.ends_with(".calcul.json")
            || rel.ends_with(crate::ouverture::ETAPE)
            || rel.ends_with(".verrou")
    }

    /// L'empreinte d'une compagnie : chaque fichier, sa taille et sa date (hors caches).
    fn empreinte(racine: &Path) -> BTreeMap<String, (u64, f64)> {
        fichiers_de(racine)
            .into_iter()
            .filter(|(r, _)| !hors_empreinte(r))
            .filter_map(|(r, p)| std::fs::metadata(&p).ok().map(|m| (r, (m.len(), crate::paris::mtime(&m)))))
            .collect()
    }

    impl Enregistreur {
        /// Pose l'enregistreur de ce processus : `dossier` (cree s'il n'existe pas ; un scenario commence s'y poursuit),
        /// `inst` l'installation. Le banc commence a `jour`.
        pub fn demarrer(dossier: &Path, inst: &Path) -> Result<&'static Enregistreur, String> {
            ENREGISTREUR.set(Enregistreur::nouveau(dossier, inst)?).map_err(|_| "Un enregistreur est deja pose dans ce processus.".to_string())?;
            Ok(ENREGISTREUR.get().expect("pose"))
        }

        /// Un enregistreur qui n'est pas celui du processus (les essais) : `dossier` cree s'il n'existe pas, un scenario
        /// commence s'y poursuit.
        pub fn nouveau(dossier: &Path, inst: &Path) -> Result<Enregistreur, String> {
            std::fs::create_dir_all(dossier.join(PIECES)).map_err(|e| format!("{} : {}", dossier.display(), e))?;
            let deja = lire(dossier).map(|g| g.iter().filter_map(|x| x.get("n").and_then(Value::as_u64)).max().unwrap_or(0)).unwrap_or(0);
            Ok(Enregistreur { dossier: dossier.to_path_buf(), inst: inst.to_path_buf(), tour: Mutex::new(deja) })
        }

        pub fn dossier(&self) -> &Path {
            &self.dossier
        }

        /// Le dernier instant enregistre (le banc reprend apres lui).
        pub fn dernier_instant(&self) -> Option<f64> {
            lire(&self.dossier).ok()?.iter().filter_map(|g| g.get("quand").and_then(Value::as_f64)).fold(None, |m, t| Some(m.map_or(t, |x: f64| x.max(t))))
        }

        fn ecrire(&self, n: &mut u64, mut ligne: Map<String, Value>, quand: f64) {
            *n += 1;
            let mut m = Map::new();
            m.insert("n".into(), json!(*n));
            m.insert("quand".into(), json!(quand));
            m.insert("heure".into(), json!(heure(quand)));
            m.append(&mut ligne);
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dossier.join(GESTES)) {
                let _ = writeln!(f, "{}", Value::Object(m));
            }
        }

        /// Un geste qui n'est pas une route (`debut`, `ajouter`, `simuler`, `jour`), fait par `f` sous le meme tour et au
        /// meme instant fige ; `f` rend ce qu'on ajoute a la ligne, ou son refus (rien n'est alors enregistre).
        pub fn geste<T>(&self, nom: &str, f: impl FnOnce() -> Result<(T, Map<String, Value>), String>) -> Result<T, String> {
            let mut n = self.tour.lock().unwrap_or_else(|p| p.into_inner());
            let quand = crate::temps::secondes();
            crate::temps::figer(quand);
            let rendu = f();
            crate::temps::defiger();
            let (t, mut plus) = rendu?;
            let mut ligne = Map::new();
            ligne.insert("geste".into(), json!(nom));
            ligne.append(&mut plus);
            self.ecrire(&mut n, ligne, quand);
            Ok(t)
        }

        /// Le dossier d'une compagnie, tel que le scenario le nomme.
        pub fn compagnie(&self, racine: &Path) -> String {
            relatif(&self.inst, racine)
        }

        /// UNE ROUTE, servie par `servir` sous l'enregistreur : le temps fige a son debut, ce qu'on avait depose recopie,
        /// l'empreinte de la compagnie avant et apres, puis sa ligne.
        pub fn route(&self, racine: &Path, nom: &str, charge: &Value, servir: impl FnOnce() -> Value) -> Value {
            let mut n = self.tour.lock().unwrap_or_else(|p| p.into_inner());
            let quand = crate::temps::secondes();
            crate::temps::figer(quand);
            let pieces = self.recopier_les_pieces(racine, nom, charge);
            let avant = empreinte(racine);
            let rep = servir();
            let ecrit = empreinte(racine) != avant;
            crate::temps::defiger();
            let mut ligne = Map::new();
            ligne.insert("route".into(), json!(nom));
            ligne.insert("compagnie".into(), json!(self.compagnie(racine)));
            ligne.insert("charge".into(), sans_mot_de_passe(charge));
            if !pieces.is_empty() {
                ligne.insert("pieces".into(), Value::Object(pieces));
            }
            ligne.insert("ecrit".into(), json!(ecrit));
            let ok = rep.get("ok") == Some(&Value::Bool(true));
            ligne.insert("ok".into(), json!(ok));
            // Le refus se garde, pour qui relit le scenario (le rejeu ne compare que `ok`).
            if !ok {
                ligne.insert("refus".into(), json!(rep.get("erreur").map(crate::etats::py_str).unwrap_or_default()));
            }
            self.ecrire(&mut n, ligne, quand);
            rep
        }

        /// Recopie un fichier dans `pieces/` (une fois par contenu) ; rend son empreinte.
        fn recopier(&self, p: &Path) -> Option<String> {
            let octets = std::fs::read(p).ok()?;
            let sha = crate::fichiers::sha256_hex(&octets);
            let dest = self.dossier.join(PIECES).join(&sha);
            if !dest.exists() {
                std::fs::write(&dest, &octets).ok()?;
            }
            Some(sha)
        }

        /// Ce que la route lit hors du moteur : un dossier ou un fichier nomme par la charge (`CLES_DE_DEPOT`), ou la
        /// boite aux lettres `import/` de la compagnie. Chaque fichier entre dans `pieces/`.
        fn recopier_les_pieces(&self, racine: &Path, route: &str, charge: &Value) -> Map<String, Value> {
            let mut out = Map::new();
            for cle in CLES_DE_DEPOT {
                let valeur = charge.get(cle).and_then(Value::as_str).map(str::trim);
                let chemin = match valeur {
                    Some(s) if !s.is_empty() => {
                        let p = PathBuf::from(crate::routes_apercus::normaliser(s));
                        if p.is_absolute() {
                            p
                        } else {
                            continue;
                        }
                    }
                    _ if cle == "dossier" && lit_la_boite_aux_lettres(route) => racine.join("import"),
                    _ => continue,
                };
                let dans = chemin.strip_prefix(racine).ok().map(|r| r.to_string_lossy().replace('\\', "/"));
                let mut fichiers = Map::new();
                let mut piece = Map::new();
                if chemin.is_file() {
                    let Some(sha) = self.recopier(&chemin) else { continue };
                    fichiers.insert(".".into(), json!(sha));
                    piece.insert("fichier".into(), json!(true));
                    piece.insert("nom".into(), json!(chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
                } else if chemin.is_dir() {
                    for (rel, p) in fichiers_de(&chemin) {
                        if let Some(sha) = self.recopier(&p) {
                            fichiers.insert(rel, json!(sha));
                        }
                    }
                } else {
                    continue;
                }
                // Un fichier de la compagnie que le moteur a ecrit lui-meme (une DSN de `sorties/` a deposer) n'est pas
                // une piece : le rejeu le refait.
                if piece.contains_key("fichier") && dans.is_some() {
                    continue;
                }
                if let Some(d) = dans {
                    piece.insert("dans_la_compagnie".into(), json!(d));
                }
                piece.insert("fichiers".into(), Value::Object(fichiers));
                out.insert(cle.to_string(), Value::Object(piece));
            }
            out
        }
    }
}

#[cfg(feature = "atelier")]
pub use enregistreur::actif as enregistreur_actif;

/// L'enregistreur, puis le rejeu de ce qu'il a enregistre (la suite de l'atelier : `--features atelier`).
#[cfg(all(test, feature = "atelier"))]
mod atelier {
    use super::*;
    use serde_json::Map;

    fn route(e: &Enregistreur, racine: &Path, nom: &str, charge: Value) -> Value {
        let mut o = std::collections::HashMap::new();
        o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
        e.route(racine, nom, &charge, || crate::routes::dans_la_fenetre(&o, nom, &charge))
    }

    /// Le « + », une ecriture, une lecture, un dossier depose hors de la compagnie : le scenario dit ce qui a ecrit, recopie
    /// ce qu'on avait depose ; rejoue ailleurs, sans le dossier depose, il refait la meme compagnie.
    #[test]
    fn ce_que_l_enregistreur_consigne_se_rejoue_sans_rien_d_exterieur() {
        let d = std::env::temp_dir().join(format!("puppetpaie-scenario-atelier-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (inst, sc, ailleurs) = (d.join("installation"), d.join("scenario"), d.join("ailleurs"));
        std::fs::create_dir_all(ailleurs.join("sous")).unwrap();
        std::fs::write(ailleurs.join("sous/note.txt"), "une piece deposee\n").unwrap();
        crate::temps::couler_depuis(crate::temps::instant_de(crate::date::Jour::nouveau(2026, 11, 2).unwrap(), 9));
        let e = Enregistreur::nouveau(&sc, &inst).unwrap();
        let c = e
            .geste("ajouter", || {
                let c = crate::interface::compagnies::ajouter_dans(&inst, "Essai du rejeu")?;
                let mut m = Map::new();
                m.insert("nom".into(), json!("Essai du rejeu"));
                m.insert("compagnie".into(), json!(e.compagnie(&c.racine)));
                Ok((c, m))
            })
            .unwrap();
        let fiche = json!({"nom": "spectacle", "cle": null, "donnees": {"titre": "Le Rejeu", "numero_objet": "266Z00952011"}});
        assert_eq!(route(&e, &c.racine, "enregistrer-fiche", fiche)["ok"], json!(true));
        route(&e, &c.racine, "etat", json!({"motdepasse": "pas-dans-le-scenario"}));
        route(&e, &c.racine, "importation-apercu", json!({"dossier": ailleurs.to_string_lossy()}));
        crate::temps::liberer();

        let gestes = lire(&sc).unwrap();
        let de = |r: &str| gestes.iter().find(|g| g["route"] == json!(r)).unwrap_or_else(|| panic!("{} absent du scenario", r)).clone();
        assert_eq!(gestes[0]["geste"], json!("ajouter"));
        assert_eq!(gestes[0]["compagnie"], json!("fictives/ajoutee-essai-du-rejeu"));
        assert_eq!(de("enregistrer-fiche")["ecrit"], json!(true));
        assert_eq!(de("etat")["ecrit"], json!(false), "une lecture n'ecrit rien");
        assert!(!std::fs::read_to_string(sc.join(GESTES)).unwrap().contains("pas-dans-le-scenario"), "un mot de passe tape n'entre pas dans le scenario");
        let sha = de("importation-apercu")["pieces"]["dossier"]["fichiers"]["sous/note.txt"].as_str().unwrap().to_string();
        assert!(sc.join(PIECES).join(&sha).is_file(), "la piece deposee est recopiee dans le scenario");

        // Le rejeu, ailleurs, le dossier depose efface : la meme compagnie, et la piece recreee a cote d'elle.
        std::fs::remove_dir_all(&ailleurs).unwrap();
        let inst2 = d.join("rejeu");
        let r = rejouer(&sc, &inst2, &OptionsRejeu { lectures: true, ..Default::default() }).unwrap();
        assert!(r.ecarts.is_empty(), "{:?}", r.ecarts);
        let lire_fiche = |i: &Path| std::fs::read_to_string(i.join("fictives/ajoutee-essai-du-rejeu/config/spectacles.json")).unwrap();
        assert_eq!(lire_fiche(&inst2), lire_fiche(&inst));
        assert_eq!(std::fs::read_to_string(inst2.join("pieces-deposees/4/dossier/sous/note.txt")).unwrap(), "une piece deposee\n");
        assert!(!crate::temps::simule(), "le rejeu rend le temps a la montre");
        let _ = std::fs::remove_dir_all(&d);
    }
}
