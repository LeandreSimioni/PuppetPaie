//! LE SCENARIO D'UNE COMPAGNIE : ce qu'on a fait dans l'interface, geste par geste, et son rejeu sans
//! navigateur.
//!
//! UN SCENARIO EST UN DOSSIER :
//! - `gestes.jsonl` : un geste par ligne, dans l'ordre ou ils ont lieu. Les gestes :
//!   - `{"geste": "debut", "jour", "logiciel"}` : le banc commence (rien a rejouer) ;
//!   - `{"geste": "ajouter", "nom"}` : le « + » de l'accueil d'un logiciel bride (`compagnies::ajouter_dans`), le decor
//!     de depart ; le banc y note `compagnie`, le dossier qu'il a cree, relatif a l'installation (`fictives/ajoutee-…`),
//!     et le rejeu verifie qu'il cree le meme ;
//!   - `{"geste": "simuler"}` : le geste d'atelier qui fait d'une fictive ajoutee une fictive aux envois simules
//!     (`rendre_simulee`) -- sans lui, ses envois sont refuses et elle ne peut pas vivre sa vie ;
//!   - `{"geste": "jour", "jour"}` : le calendrier avance, et chaque compagnie du scenario s'ouvre ce jour-la : son
//!     horloge joue ce qui etait du (`horloge::rattraper`, comme a l'ouverture). Au banc, la route `ouverture` qui suit
//!     ne trouve alors plus rien a jouer ;
//!   - `{"route", "charge"}` : une route du moteur, telle que la fenetre l'a servie (la page, la veille, le fil de fond :
//!     `interface::api::route`), avec la charge que le moteur a recue. `ecrit` (vrai par defaut) : la route a change un
//!     fichier de la compagnie -- le banc le note, et le rejeu ecarte les routes qui n'ont rien ecrit ; `ok` (vrai par
//!     defaut) : elle a reussi, et le rejeu le verifie. `pieces` : ce qu'on avait depose dans un dossier que la route lit
//!     (l'import), par cle de la charge : `{"chemin"}` (un fichier ou un dossier du scenario, nomme relativement a lui),
//!     ou `{"fichiers": {rel: empreinte}}` (ce que le banc a recopie dans `pieces/`) ; `dans_la_compagnie` pose la piece
//!     a cet endroit de la compagnie au lieu d'un dossier de travail.
//!   Chaque geste a son instant : `quand` (secondes UTC, ce que le banc ecrit), ou `heure`, a Paris (`AAAA-MM-JJ`, a 9 h,
//!   ou `AAAA-MM-JJTHH:MM[:SS]`) ; un geste `jour` sans l'un ni l'autre a lieu ce jour-la a 9 h, et tout autre geste sans
//!   date, une seconde apres celui qui le precede. Les instants ne reculent jamais. `compagnie` (le dossier relatif a
//!   l'installation) se tait quand le scenario n'en ajoute qu'une : c'est la derniere ajoutee. `n`, le rang que le banc
//!   ecrit, ne sert qu'a le relire.
//! - `pieces/` : le contenu de chaque fichier depose -- par empreinte (`pieces/<sha256>`, le banc : un PDF de bulletins,
//!   une DSN, un contrat...), ou sous le nom qu'un scenario ecrit a la main lui donne. Le scenario se rejoue ainsi sans
//!   rien d'exterieur : ni `/tmp`, ni le poste ou on l'a enregistre.
//!
//! L'ENREGISTREUR (`Enregistreur`) n'existe que dans le binaire de l'atelier (`moteur-atelier`, `--features atelier`) :
//! `moteur-atelier interface --enregistrer <dossier> --jour J` (le banc, `atelier/outils/banc/`). Le binaire livre n'en a
//! pas une ligne.
//!
//! LE REJEU (`rejouer`) est dans le moteur ordinaire : les essais (`tests/outils/rejeu.rs`) et l'exemple `vie_fictive`
//! l'appellent dans leur processus. Chaque geste se rejoue a son instant, fige (`temps::figer`), les aleas en suite fixe
//! (`temps::aleas_fixes`) : deux rejeux ecrivent les memes octets. Les routes qui n'ont rien ecrit s'ecartent par defaut
//! (une lecture ne change pas la compagnie ; les garder coute le temps de les recalculer) ; `OptionsRejeu::lectures` les
//! rejoue aussi. `rejouer_et_lire` s'arrete a des jours choisis pour qu'on lise la compagnie telle qu'elle est ce
//! soir-la, puis reprend : un seul rejeu pour plusieurs dates.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use socle::date::Jour;

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

/// Les routes dont la cle `dossier` vide (ou absente) veut dire `import/` de la compagnie (`crate::routes::import::dossier_import`).
#[cfg(feature = "atelier")]
fn lit_la_boite_aux_lettres(route: &str) -> bool {
    route.starts_with("importation") || route == "importer"
}

/// LE MARQUEUR D'UNE FICTIVE AUX ENVOIS SIMULES, pose sur une fictive ajoutee (le geste `simuler`) : `copie_de` dit
/// « compagnie fictive … » (`compagnies::est_fictive`) sans la marque `ajoutee` -- ses envois sont joues
/// (`bridage::branchement`) et son horloge joue ce qui etait du (`horloge`). Le nom (« … sur le banc
/// d'essai ») n'est celui d'aucune fictive livree : la livraison des fictives (`compagnie::maj`) ne la prend jamais pour une
/// des siennes. Refuse hors d'une fictive ajoutee.
pub fn rendre_simulee(racine: &Path) -> Result<(), String> {
    if !crate::interface::compagnies::est_ajoutee(racine) {
        return Err(format!("{} n'est pas une compagnie ajoutee par le « + » : rien a changer.", racine.display()));
    }
    let nom = racine.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    socle::ecriture::ecrire_json(
        &racine.join("COPIE-DE-DEVELOPPEMENT.json"),
        &json!({
            "copie_de": format!("{} {} sur le banc d'essai", crate::interface::compagnies::MARQUE_FICTIVE, nom),
            "quand": socle::paris::maintenant(),
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
    socle::paris::iso_secondes(t)
}

/// Les fichiers d'un dossier, relatifs a lui (barres droites), dans l'ordre des noms.
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
    /// S'arreter a la fin de ce jour (a Paris) : les gestes d'apres ne se jouent pas.
    pub jusqu_au: Option<Jour>,
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

/// Les gestes d'un scenario, lus tels qu'ils sont ecrits.
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

/// L'instant qu'un geste ecrit : `quand`, sinon `heure` a Paris (`AAAA-MM-JJ` a 9 h, `AAAA-MM-JJTHH:MM[:SS]`), sinon le
/// jour d'un geste `jour`, a 9 h. `None` : le geste n'en ecrit pas.
fn instant_ecrit(g: &Value) -> Result<Option<f64>, String> {
    if let Some(t) = g.get("quand").and_then(Value::as_f64) {
        return Ok(Some(t));
    }
    let texte = match (g.get("heure").and_then(Value::as_str), g.get("geste").and_then(Value::as_str)) {
        (Some(h), _) => h,
        (None, Some("jour")) => g.get("jour").and_then(Value::as_str).ok_or("un geste `jour` sans jour")?,
        _ => return Ok(None),
    };
    let illisible = || format!("{} : une date (AAAA-MM-JJ) ou une heure de Paris (AAAA-MM-JJTHH:MM) est attendue", socle::fichiers::repr(texte));
    let (jour, hms) = texte.split_once('T').unwrap_or((texte, "09:00"));
    let jour = Jour::lire(jour).map_err(|_| illisible())?;
    let mut parts = hms.split(':').map(|x| x.parse::<i64>().ok());
    let (h, m, s) = match (parts.next().flatten(), parts.next().flatten(), parts.next()) {
        (Some(h), Some(m), None) => (h, m, 0),
        (Some(h), Some(m), Some(Some(s))) => (h, m, s),
        _ => return Err(illisible()),
    };
    if !(0..24).contains(&h) || !(0..60).contains(&m) || !(0..60).contains(&s) {
        return Err(illisible());
    }
    Ok(Some(socle::temps::instant_de(jour, h) + (m * 60 + s) as f64))
}

/// Les instants de chaque geste, dans l'ordre du fichier : celui qu'il ecrit, ou une seconde apres le precedent. Un
/// instant ecrit qui recule refuse le scenario ; un instant egal au precedent passe une seconde apres lui (deux gestes ne
/// partagent jamais un instant : chacun tire ses aleas depuis le sien).
fn instants(gestes: &[Value]) -> Result<Vec<f64>, String> {
    let mut out: Vec<f64> = Vec::with_capacity(gestes.len());
    let mut ecrit_avant: Option<f64> = None;
    for (i, g) in gestes.iter().enumerate() {
        let geste = |m: String| format!("geste {} : {}", i + 1, m);
        let precedent = out.last().copied();
        let t = match instant_ecrit(g).map_err(geste)? {
            Some(t) => {
                if ecrit_avant.is_some_and(|a| t < a) {
                    return Err(geste(format!("{} vient avant le geste qui le precede : les gestes vont dans l'ordre du temps", heure(t))));
                }
                ecrit_avant = Some(t);
                match precedent {
                    Some(p) if t <= p => p + 1.0,
                    _ => t,
                }
            }
            None => precedent.map(|p| p + 1.0).ok_or_else(|| geste("le premier geste n'a ni `quand` ni `heure`".to_string()))?,
        };
        out.push(t);
    }
    Ok(out)
}

/// Le contenu d'une piece : `chemin` (relatif au scenario) ou l'empreinte `sha` (dans `pieces/`).
fn source_de_piece(scenario: &Path, sha: &str) -> PathBuf {
    scenario.join(PIECES).join(sha)
}

/// Pose, avant la route, ce qu'on avait depose : un dossier de la compagnie se remet tel qu'il etait ; un dossier
/// hors d'elle se recree dans `travail/<rang>/<cle>` et la charge le nomme. Rend la charge a donner au moteur.
fn poser_les_pieces(scenario: &Path, racine: &Path, travail: &Path, rang: usize, geste: &Value) -> Result<Value, String> {
    let mut charge = geste.get("charge").cloned().unwrap_or_else(|| json!({}));
    let Some(pieces) = geste.get("pieces").and_then(Value::as_object) else { return Ok(charge) };
    for (cle, p) in pieces {
        // Les fichiers de la piece : (relatif a la piece, source). Un fichier seul a la cle `.`.
        let fichiers: Vec<(String, PathBuf)> = match p.get("chemin").and_then(Value::as_str) {
            Some(c) => {
                let source = scenario.join(c);
                if source.is_dir() {
                    fichiers_de(&source)
                } else if source.is_file() {
                    vec![(".".to_string(), source)]
                } else {
                    return Err(format!("piece {} : {} n'existe pas dans le scenario", cle, c));
                }
            }
            None => p
                .get("fichiers")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(|(rel, sha)| Ok((rel.clone(), source_de_piece(scenario, sha.as_str().ok_or("piece sans empreinte")?))))
                .collect::<Result<_, String>>()?,
        };
        let nom = p.get("nom").and_then(Value::as_str).map(String::from).or_else(|| {
            p.get("chemin").and_then(Value::as_str).and_then(|c| Path::new(c).file_name()).map(|n| n.to_string_lossy().into_owned())
        });
        let est_fichier = p.get("fichier").and_then(Value::as_bool) == Some(true) || (fichiers.len() == 1 && fichiers[0].0 == ".");
        let dans = p.get("dans_la_compagnie").and_then(Value::as_str);
        let cible = match dans {
            Some(rel) => racine.join(rel),
            None => travail.join(rang.to_string()).join(cle),
        };
        if est_fichier {
            let (_, source) = fichiers.first().ok_or("piece sans contenu")?;
            let dest = if dans.is_some() { cible.clone() } else { cible.join(nom.as_deref().unwrap_or("piece")) };
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::copy(source, &dest).map_err(|e| format!("piece {} : {}", source.display(), e))?;
            if dans.is_none() {
                charge[cle.as_str()] = json!(dest.display().to_string());
            }
            continue;
        }
        let _ = std::fs::remove_dir_all(&cible);
        std::fs::create_dir_all(&cible).map_err(|e| format!("{} : {}", cible.display(), e))?;
        for (rel, source) in &fichiers {
            let dest = cible.join(rel);
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            std::fs::copy(source, &dest).map_err(|e| format!("piece {} : {}", source.display(), e))?;
        }
        if dans.is_none() {
            charge[cle.as_str()] = json!(cible.display().to_string());
        }
    }
    Ok(charge)
}

/// REJOUE le scenario `scenario` dans l'installation `inst` (un dossier vide, ou qui ne porte pas encore les compagnies
/// du scenario) : le decor du « + », chaque geste a son instant, et rien d'autre. Le temps du moteur est simule le temps
/// du rejeu, puis rendu a la montre. `travail` recoit les dossiers deposes hors de la compagnie.
pub fn rejouer(scenario: &Path, inst: &Path, options: &OptionsRejeu) -> Result<Rejeu, String> {
    rejouer_et_lire(scenario, inst, options, &[], &mut |_, _| {})
}

/// `rejouer`, qui s'arrete le soir de chacun des jours `arrets` (dans l'ordre) -- apres le dernier geste de ce jour-la,
/// avant le premier du lendemain -- pour que `lire` regarde la compagnie telle qu'elle est (le jour, et ce que le rejeu a
/// fait jusque-la), puis reprend. Un jour d'arret apres le dernier geste se lit a la fin. Le temps du moteur reste
/// celui du dernier geste joue : `lire` lit comme un poste, par une route ou les fichiers, a la date qu'il dit.
pub fn rejouer_et_lire(scenario: &Path, inst: &Path, options: &OptionsRejeu, arrets: &[Jour], lire_au: &mut dyn FnMut(Jour, &Rejeu)) -> Result<Rejeu, String> {
    let gestes = lire(scenario)?;
    let instants = instants(&gestes)?;
    std::fs::create_dir_all(inst).map_err(|e| format!("{} : {}", inst.display(), e))?;
    // L'INSTALLATION DU REJEU, BRIDEE EN TOUTES LETTRES (le geste de l'accueil, `bridage.rs`) : comme celle du banc, et le
    // moteur ne lit qu'elle (`compagnies::base`) -- rien du depot ni du poste ou l'on rejoue.
    socle::ecriture::ecrire_json(&inst.join(".edition.json"), &json!({"edition": crate::compagnie::bridage::BRIDE}), 2)?;
    let inst = std::fs::canonicalize(inst).map_err(|e| format!("{} : {}", inst.display(), e))?;
    let travail = inst.join("pieces-deposees");
    socle::temps::liberer();
    socle::temps::aleas_fixes();
    crate::interface::compagnies::poser_l_installation_du_rejeu(Some(inst.clone()));
    let rendu = rejouer_les_gestes(scenario, &inst, &travail, &gestes, &instants, options, arrets, lire_au);
    crate::interface::compagnies::poser_l_installation_du_rejeu(None);
    socle::temps::liberer();
    rendu
}

/// Le jour a Paris d'un instant.
fn jour_de(t: f64) -> Jour {
    Jour::lire(&heure(t)[..10]).expect("un instant se lit en jour")
}

#[allow(clippy::too_many_arguments)]
fn rejouer_les_gestes(
    scenario: &Path,
    inst: &Path,
    travail: &Path,
    gestes: &[Value],
    instants: &[f64],
    options: &OptionsRejeu,
    arrets: &[Jour],
    lire_au: &mut dyn FnMut(Jour, &Rejeu),
) -> Result<Rejeu, String> {
    let mut r = Rejeu::default();
    let mut arrets = arrets.iter().copied().peekable();
    // LA LECTURE D'UN SOIR, avant le premier geste du lendemain. Ce qu'elle tire d'aleas ne change rien : chaque geste
    // repart des siens a son instant (`temps::figer`).
    let mut lire_les_soirs = |avant: Jour, r: &Rejeu| {
        while let Some(j) = arrets.next_if(|j| *j < avant) {
            lire_au(j, r);
        }
    };
    // Le dossier enregistre -> la racine du rejeu ; « » nomme la derniere ajoutee.
    let mut racines: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (i, (g, &t)) in gestes.iter().zip(instants).enumerate() {
        let n = g.get("n").and_then(Value::as_u64).map_or(i + 1, |n| n as usize);
        let jour = jour_de(t);
        if options.jusqu_au.is_some_and(|j| jour > j) {
            break;
        }
        lire_les_soirs(jour, &r);
        socle::temps::figer(t);
        r.fin = Some(t);
        let compagnie = g.get("compagnie").and_then(Value::as_str).unwrap_or("");
        match g.get("geste").and_then(Value::as_str) {
            Some("ajouter") => {
                let nom = g.get("nom").and_then(Value::as_str).ok_or(format!("geste {} : `ajouter` sans nom", n))?;
                let c = crate::interface::compagnies::ajouter_dans(inst, nom).map_err(|m| format!("geste {} : « + » refuse : {}", n, m))?;
                let rel = relatif(inst, &c.racine);
                if !compagnie.is_empty() && rel != compagnie {
                    r.ecarts.push(format!("geste {} : le « + » a cree {} (enregistre : {})", n, rel, compagnie));
                }
                let nomme = if compagnie.is_empty() { rel } else { compagnie.to_string() };
                racines.insert(nomme.clone(), c.racine.clone());
                racines.insert(String::new(), c.racine.clone());
                r.compagnies.push((nomme, c.racine));
            }
            Some("simuler") => {
                let racine = racines.get(compagnie).ok_or(format!("geste {} : `simuler` sur {}, que le scenario n'a pas ajoutee", n, compagnie))?;
                rendre_simulee(racine).map_err(|m| format!("geste {} : {}", n, m))?;
            }
            Some("jour") => {
                for (_, racine) in &r.compagnies {
                    match crate::horloge::rattraper(racine, Some(jour), true) {
                        Ok(fait) => r.horloge.extend(fait.iter().map(|p| format!("{} {}", heure(t), p))),
                        Err(e) => r.ecarts.push(format!("geste {} ({}) : l'horloge refuse : {}", n, heure(t), e.message)),
                    }
                }
            }
            Some(_) => {}
            None => {
                let route = g.get("route").and_then(Value::as_str).ok_or(format!("geste {} : ni `geste` ni `route`", n))?;
                let Some(racine) = racines.get(compagnie) else {
                    r.autres_compagnies += 1;
                    continue;
                };
                if g.get("ecrit") == Some(&Value::Bool(false)) && !options.lectures {
                    r.lectures_ecartees += 1;
                    continue;
                }
                let charge = poser_les_pieces(scenario, racine, travail, n, g)?;
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
                    r.horloge.push(format!("{} {}", heure(t), socle::valeur::valeur_en_texte(p)));
                }
                let attendu = g.get("ok") != Some(&Value::Bool(false));
                if ok != attendu {
                    r.ecarts.push(format!(
                        "geste {} ({} {}) : {} au rejeu, {} a l'enregistrement{}",
                        n,
                        heure(t),
                        route,
                        if ok { "reussit" } else { "refuse" },
                        if attendu { "reussi" } else { "refuse" },
                        rep.get("erreur").map(|e| format!(" : {}", socle::valeur::valeur_en_texte(e))).unwrap_or_default()
                    ));
                }
            }
        }
    }
    lire_les_soirs(Jour::nouveau(9999, 12, 31).expect("un jour"), &r);
    Ok(r)
}

// ------------------------------------------------------------------ elaguer, verifier, la ligne de commande

/// ELAGUER un scenario pour le figer (phase 4) : `vers` recoit les memes gestes sans les routes qui n'ont rien ecrit (le
/// rejeu les ecarte deja), et les seules pieces que les gestes gardes nomment. Rend (gestes gardes, lectures otees).
pub fn elaguer(scenario: &Path, vers: &Path) -> Result<(usize, usize), String> {
    let gestes = lire(scenario)?;
    std::fs::create_dir_all(vers.join(PIECES)).map_err(|e| format!("{} : {}", vers.display(), e))?;
    let copier = |rel: &Path| -> Result<(), String> {
        let dest = vers.join(rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{} : {}", d.display(), e))?;
        }
        std::fs::copy(scenario.join(rel), &dest).map(|_| ()).map_err(|e| format!("piece {} : {}", rel.display(), e))
    };
    let (mut gardes, mut otees) = (Vec::new(), 0);
    for g in gestes {
        if g.get("route").is_some() && g.get("ecrit") == Some(&Value::Bool(false)) {
            otees += 1;
            continue;
        }
        for p in g.get("pieces").and_then(Value::as_object).into_iter().flat_map(|m| m.values()) {
            for sha in p.get("fichiers").and_then(Value::as_object).into_iter().flat_map(|f| f.values()).filter_map(Value::as_str) {
                copier(&Path::new(PIECES).join(sha))?;
            }
            if let Some(c) = p.get("chemin").and_then(Value::as_str) {
                let source = scenario.join(c);
                if source.is_dir() {
                    for (rel, _) in fichiers_de(&source) {
                        copier(&Path::new(c).join(rel))?;
                    }
                } else {
                    copier(Path::new(c))?;
                }
            }
        }
        gardes.push(g.to_string());
    }
    let n = gardes.len();
    std::fs::write(vers.join(GESTES), gardes.join("\n") + "\n").map_err(|e| format!("{} : {}", vers.display(), e))?;
    Ok((n, otees))
}

/// Ce qu'une comparaison de deux compagnies ne regarde pas : les caches que la fenetre refait en fond, son journal, l'etape
/// en cours d'un passage de la veille.
fn hors_comparaison(rel: &str) -> bool {
    rel == "journal/interface.log" || rel == "ouverture.etat.json" || rel.ends_with(crate::compagnie::ouverture::ETAPE)
}

/// Les fichiers d'une compagnie, relatifs (barres droites), hors caches.
fn fichiers_compares(racine: &Path) -> BTreeMap<String, PathBuf> {
    fn parcourir(base: &Path, d: &Path, out: &mut BTreeMap<String, PathBuf>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(base, &p, out);
            } else if let Ok(r) = p.strip_prefix(base) {
                let r = r.to_string_lossy().replace('\\', "/");
                if !hors_comparaison(&r) {
                    out.insert(r, p.clone());
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    parcourir(racine, racine, &mut out);
    out
}

/// Le contenu d'un fichier, le chemin de sa compagnie remplace par `<racine>` (un journal note des chemins).
fn contenu(p: &Path, racine: &Path) -> Vec<u8> {
    let o = std::fs::read(p).unwrap_or_default();
    match String::from_utf8(o) {
        Ok(t) => t.replace(&racine.to_string_lossy().into_owned(), "<racine>").into_bytes(),
        Err(e) => e.into_bytes(),
    }
}

/// VERIFIER UN BANC : rejoue son scenario dans `travail` (vide d'abord) et compare, fichier par fichier, chaque
/// compagnie que le scenario a ajoutee a celle du banc (`inst`). Le banc tire les memes aleas que le rejeu
/// (`temps::aleas`) : toute difference est un geste qui a ecrit sans etre enregistre (un fichier touche hors de la page, une
/// ecriture de fond), ou un ecart du rejeu. Rend les differences, en phrases (vide : le scenario suffit).
pub fn verifier(scenario: &Path, inst: &Path, travail: &Path) -> Result<Vec<String>, String> {
    let _ = std::fs::remove_dir_all(travail);
    let r = rejouer(scenario, travail, &OptionsRejeu::default())?;
    let mut ecarts: Vec<String> = r.ecarts.iter().map(|e| format!("au rejeu, {}", e)).collect();
    for (rel, rejouee) in &r.compagnies {
        let banc = inst.join(rel);
        let (a, b) = (fichiers_compares(&banc), fichiers_compares(rejouee));
        for (f, p) in &a {
            match b.get(f) {
                None => ecarts.push(format!("{}/{} : au banc, pas au rejeu (ecrit sans etre enregistre ?)", rel, f)),
                Some(q) if contenu(p, &banc) != contenu(q, rejouee) => {
                    let (x, y) = (contenu(p, &banc), contenu(q, rejouee));
                    let ligne = String::from_utf8_lossy(&x).lines().zip(String::from_utf8_lossy(&y).lines()).position(|(u, v)| u != v);
                    ecarts.push(format!(
                        "{}/{} : differe du rejeu{}",
                        rel,
                        f,
                        ligne.map(|l| format!(" (des la ligne {})", l + 1)).unwrap_or_default()
                    ));
                }
                Some(_) => {}
            }
        }
        for f in b.keys().filter(|f| !a.contains_key(*f)) {
            ecarts.push(format!("{}/{} : au rejeu, pas au banc", rel, f));
        }
    }
    Ok(ecarts)
}

const USAGE: &str = "scenario rejouer <scenario> <installation neuve> [--lectures] [--jusqu-au AAAA-MM-JJ]\n\
                     scenario verifier <scenario> <installation du banc> <dossier de travail>\n\
                     scenario elaguer <scenario> <scenario sans lectures>";

/// LA LIGNE DE COMMANDE DES SCENARIOS : `moteur-atelier scenario ...` (le banc) et l'exemple `vie_fictive` (sans le binaire
/// de l'atelier). `rejouer`, `verifier`, `elaguer`. Rend le code de sortie : 1 a la moindre difference.
pub fn commande(args: &[String]) -> i32 {
    let mut options = OptionsRejeu::default();
    let mut a: Vec<&str> = Vec::new();
    let mut it = args.iter().map(String::as_str);
    while let Some(x) = it.next() {
        match x {
            "--lectures" => options.lectures = true,
            "--jusqu-au" => match it.next().map(Jour::lire) {
                Some(Ok(j)) => options.jusqu_au = Some(j),
                _ => {
                    eprintln!("--jusqu-au attend un jour, AAAA-MM-JJ.\n{}", USAGE);
                    return 2;
                }
            },
            _ => a.push(x),
        }
    }
    // Rien ne sort de la machine (`garde_reseau`) : lancee a la main, la commande pose la variable, avant tout fil.
    if std::env::var_os(socle::garde_reseau::VARIABLE).is_none() {
        std::env::set_var(socle::garde_reseau::VARIABLE, "1");
    }
    let debut = std::time::Instant::now();
    match a.as_slice() {
        ["rejouer", sc, inst] => {
            let inst = Path::new(inst);
            if std::fs::read_dir(inst).is_ok_and(|mut d| d.next().is_some()) {
                eprintln!("{} n'est pas vide : le rejeu part d'une installation neuve.", inst.display());
                return 2;
            }
            match rejouer(Path::new(sc), inst, &options) {
                Ok(r) => {
                    for p in &r.horloge {
                        println!("      {}", p);
                    }
                    for (nom, racine) in &r.compagnies {
                        println!("{} -> {}", nom, racine.display());
                    }
                    println!(
                        "{} route(s) rejouee(s), {} lecture(s) ecartee(s), {} geste(s) sur une autre compagnie, en {:.1} s.",
                        r.routes_rejouees,
                        r.lectures_ecartees,
                        r.autres_compagnies,
                        debut.elapsed().as_secs_f64()
                    );
                    for e in &r.ecarts {
                        println!("ECART  {}", e);
                    }
                    i32::from(!r.ecarts.is_empty())
                }
                Err(m) => {
                    eprintln!("{}", m);
                    1
                }
            }
        }
        ["verifier", sc, inst, travail] => match verifier(Path::new(sc), Path::new(inst), Path::new(travail)) {
            Ok(e) if e.is_empty() => {
                println!("Le rejeu refait la compagnie du banc, fichier pour fichier ({:.1} s).", debut.elapsed().as_secs_f64());
                0
            }
            Ok(e) => {
                for x in &e {
                    println!("DIFFERENCE  {}", x);
                }
                1
            }
            Err(m) => {
                eprintln!("{}", m);
                1
            }
        },
        ["elaguer", sc, vers] => match elaguer(Path::new(sc), Path::new(vers)) {
            Ok((n, o)) => {
                println!("{} geste(s) garde(s), {} lecture(s) otee(s) : {}", n, o, vers);
                0
            }
            Err(m) => {
                eprintln!("{}", m);
                1
            }
        },
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
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
            || rel == "ouverture.etat.json"
            || rel.ends_with(crate::compagnie::ouverture::ETAPE)
            || rel.ends_with(".verrou")
    }

    /// L'empreinte d'une compagnie : chaque fichier, sa taille et sa date (hors caches).
    fn empreinte(racine: &Path) -> BTreeMap<String, (u64, f64)> {
        fichiers_de(racine)
            .into_iter()
            .filter(|(r, _)| !hors_empreinte(r))
            .filter_map(|(r, p)| std::fs::metadata(&p).ok().map(|m| (r, (m.len(), socle::paris::mtime(&m)))))
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
            let quand = socle::temps::secondes();
            socle::temps::figer(quand);
            let rendu = f();
            socle::temps::defiger();
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
            let quand = socle::temps::secondes();
            socle::temps::figer(quand);
            let pieces = self.recopier_les_pieces(racine, nom, charge);
            let avant = empreinte(racine);
            let rep = servir();
            let ecrit = empreinte(racine) != avant;
            socle::temps::defiger();
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
                ligne.insert("refus".into(), json!(rep.get("erreur").map(socle::valeur::valeur_en_texte).unwrap_or_default()));
            }
            self.ecrire(&mut n, ligne, quand);
            rep
        }

        /// Recopie un fichier dans `pieces/` (une fois par contenu) ; rend son empreinte.
        fn recopier(&self, p: &Path) -> Option<String> {
            let octets = std::fs::read(p).ok()?;
            let sha = socle::fichiers::sha256_hex(&octets);
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
                        let p = PathBuf::from(crate::routes::depots::normaliser(s));
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
