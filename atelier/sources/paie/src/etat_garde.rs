//! L'ETAT GARDE D'UN MOIS (Leandre, 08/10/2026 : « la verite est dans les JSON, l'ecran ne fait que lire » ; « c'est a
//! l'ecriture que ca doit fonctionner » ; 09/10/2026 : « juste de la lecture »). Le plan : `PLAN-MOIS-INSTANTANE.md`.
//!
//! `paies/AAAA-MM.etat.json` porte TOUT ce que l'ecran d'un mois lit : l'etat de chaque paie (`etat-par-paie`), celui du
//! mois (`etat-du-mois`), et les lectures que la page faisait une a une (`LECTURES` : la saisie, les contrats en cours,
//! les embauches, les fins, les imprevus, la reprise du journal, l'apercu de la saisie enregistree). Il est ECRIT apres
//! chaque ecriture qui touche le mois (`apres_ecriture`, en arriere-plan : l'acte n'attend pas) et a l'ouverture d'une
//! compagnie (`prechauffer`), puis LU tel quel : la fenetre le sert elle-meme, sans lancer de processus
//! (`servir_si_juste`, route `mois` de `interface/api.rs`).
//!
//! Il n'est pas une seconde verite : la verite reste la saisie, le calcul, les journaux, les retours et les fiches. L'etat
//! garde en est le resume, sous l'EMPREINTE de tout ce dont il est tire (`empreinte`). Depuis le 09/10/2026 l'empreinte ne
//! relit plus le contenu des sources : elle prend, pour chacune, sa date de modification et sa taille (une modification a
//! la main change la date). S'y ajoutent la date du jour (une echeance passee change ce qui se dit : un etat se refait
//! donc une fois par jour, et `prechauffer` le refait des l'ouverture), le moteur lui-meme et l'emplacement du dossier
//! (l'etat porte des chemins : un dossier deplace se refait). Tant que l'empreinte est la meme, il se sert sans rien
//! recalculer ; si quoi que ce soit a bouge, par le logiciel ou a la main, il se refait une fois. Un etat garde ne peut
//! donc pas mentir : au pire, il se refait.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::contexte::Contexte;
use crate::retours::Echec;

type R<T> = Result<T, Echec>;

/// Le format du fichier : un autre se refait.
const FORMAT: u64 = 2;

/// Les routes que l'etat garde sert.
pub const ROUTES: [&str; 3] = ["etat-par-paie", "etat-du-mois", "mois"];

/// Les lectures de l'ecran d'un mois qui ne sont pas les deux etats : la route, et si sa charge est `{"mois"}` (sinon
/// `{}`). La page les demandait une a une, chacune dans un processus ; elles sont faites ici, une fois, avec les memes
/// routes, et rendues telles quelles (resultat ou refus).
/// Les arrets n'y sont plus (15/10/2026) : la page lit les imprevus du mois, chaque arret avec ses obligations.
const LECTURES: [(&str, bool); 6] = [
    ("saisie", true),
    ("contrats-en-cours", true),
    ("embauches-a-declarer", true),
    ("fins-de-contrat", true),
    ("imprevus", true),
    ("journal-reprise", false),
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

/// L'empreinte de tout ce dont l'etat de `mois` est tire, au jour `jour` (AAAA-MM-JJ). Les calculs des AUTRES mois n'y
/// sont pas : ils ne changent rien a celui-ci. Ni le coffre (un `.bin` : ce qu'il dit se relit a chaque lecture), ni les
/// journaux de la fenetre et des incidents (ils bougent sans rien changer a l'ecran).
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
    signer_dossier(&mut h, racine, &racine.join("config"), true, &|n| !n.ends_with(".bin") && !n.ends_with(".tmp"));
    signer_dossier(&mut h, racine, &racine.join("journal"), false, &|n| n.ends_with(".json") && n != "journal/incidents.json");
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
    let saisie = l.get("saisie").and_then(|s| s.pointer("/resultat/saisie")).cloned();
    if let Some(s) = saisie {
        l.insert("apercu".into(), lecture(o, "apercu", json!({"saisie": s, "lignes": false})));
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
    let tmp = f.with_extension(format!("json.{}.tmp", std::process::id()));
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

/// LA FENETRE, SANS PROCESSUS : l'ecran de `mois` si son etat garde est encore juste, `None` sinon (un processus le
/// refait alors, par la route `mois`). Rien ne se calcule ici : une lecture de fichier, des dates de fichiers, le coffre.
pub fn servir_si_juste(racine: &Path, mois: &str) -> Option<Value> {
    if !mois_valide(mois) {
        return None;
    }
    let jour = crate::contexte::aujourdhui_sans_note().ok()?.iso();
    let etat = garde(racine, mois, &empreinte_de(racine, mois, &jour))?;
    Some(pour_la_page(racine, &etat, &Value::String(mois.to_string())))
}

/// A L'OUVERTURE D'UNE COMPAGNIE : les etats des mois saisis et du mois en cours se refont en arriere-plan, l'un apres
/// l'autre, s'ils ne sont plus justes (un nouveau jour, une nouvelle version, une modification a la main). Quand on
/// arrive sur un mois, il est pret.
pub fn prechauffer(racine: &Path) {
    let racine = racine.to_path_buf();
    std::thread::spawn(move || {
        let mut mois: Vec<String> = std::fs::read_dir(racine.join("paies"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).filter(|m| mois_valide(m)).map(String::from))
            .collect();
        if let Ok(j) = crate::contexte::aujourdhui_sans_note() {
            mois.push(j.iso()[..7].to_string());
        }
        mois.sort();
        mois.dedup();
        // Le plus recent d'abord : c'est la que l'on arrive.
        for m in mois.iter().rev() {
            if servir_si_juste(&racine, m).is_none() {
                let _ = crate::interface::api::route(&racine, "etat-du-mois", &json!({ "mois": m }));
            }
        }
    });
}

/// Apres une ecriture qui touche `mois` : l'etat se refait EN ARRIERE-PLAN, dans un processus a part qui le redemande
/// (et le refait s'il n'est plus juste). L'acte n'attend pas ; l'ecran, quand il viendra, le trouvera pret.
pub fn apres_ecriture(o: &HashMap<String, String>, mois: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(programme) = std::env::current_exe() else { return };
    let mut c = Command::new(programme);
    c.arg("route");
    for cle in ["racine", "date"] {
        if let Some(v) = o.get(cle) {
            c.arg(format!("--{}", cle)).arg(v);
        }
    }
    c.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    if let Ok(mut p) = c.spawn() {
        if let Some(mut e) = p.stdin.take() {
            let _ = e.write_all(json!({"route": "etat-du-mois", "charge": {"mois": mois}}).to_string().as_bytes());
        }
    }
}
