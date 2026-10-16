//! La paie calculee, a cote de la saisie : `paies/AAAA-MM.calcul.json`.
//!
//! Le fichier porte le resultat du calcul de `paies/AAAA-MM.json`
//! (la reponse telle quelle, sous `resultat`) et la liste de ses entrees avec
//! leur empreinte (`entrees`, puis `empreinte` : le SHA-256 de cette liste).
//! Les entrees sont tout ce que le calcul a lu (`fichiers::suivre`) : le
//! contenu de chaque fichier (saisie, fiches, baremes du socle et de la
//! compagnie, catalogues livres, effectifs, paies des mois precedents...),
//! les fichiers lus et absents, la liste des `.json` d'un dossier parcouru, les
//! fiches telles qu'elles etaient a `--date`, la date ou le jour courant
//! quand le calcul les a lus, et la version du moteur. Rien n'est devine :
//! un fichier de plus ou de moins, un octet change, et l'empreinte change.
//!
//! `commande` relit sans calculer tant que les entrees sont les
//! memes ; sinon, mois non parti : il recalcule et reecrit sans rien dire ;
//! mois parti (DSN deposee) : il garde les chiffres stockes.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde_json::{Map, Value};

use crate::contexte::Contexte;
use crate::fichiers::{self, dossier_du_code};
use crate::num::Resultat;
use crate::service::{calculer_avec, contexte, racine_de};

/// La version du moteur : celle du crate et la marque de ses sources
/// (`build.rs`). Tout changement du code recalcule les mois non partis.
pub const MOTEUR: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("PAIE_SOURCES"));

fn mois_valide(m: &str) -> bool {
    let b = m.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}

/// Les fiches telles qu'elles etaient a la date du contexte : c'est par elles
/// que `--date` entre dans le calcul.
fn empreinte_fiches(ctx: &Contexte) -> String {
    let v = Value::Array(vec![ctx.entreprise.clone(), Value::Object(ctx.salaries.clone()), Value::Object(ctx.spectacles.clone())]);
    fichiers::sha256_hex(v.to_string().as_bytes())
}

/// L'empreinte de la liste : une ligne `cle<TAB>valeur` par entree, triee.
fn empreinte(entrees: &BTreeMap<String, String>) -> String {
    let mut t = String::new();
    for (k, v) in entrees {
        t.push_str(k);
        t.push('\t');
        t.push_str(v);
        t.push('\n');
    }
    fichiers::sha256_hex(t.as_bytes())
}

/// `calculer` en notant tout ce qu'il lit : la reponse, et les entrees.
/// A lancer dans un processus neuf : un cache deja chaud ne relit pas.
pub fn calculer_suivi(o: &HashMap<String, String>, demande: &Value) -> Resultat<(Value, BTreeMap<String, String>)> {
    fichiers::suivre(&racine_de(o), &dossier_du_code());
    let r = contexte(o).and_then(|ctx| {
        let rep = calculer_avec(&ctx, demande)?;
        fichiers::noter_valeur("fiches", empreinte_fiches(&ctx));
        Ok(rep)
    });
    let mut entrees = fichiers::rendre();
    entrees.insert("moteur".into(), MOTEUR.into());
    Ok((r?, entrees))
}

fn fichier_calcul(racine: &Path, mois: &str) -> std::path::PathBuf {
    racine.join("paies").join(format!("{}.calcul.json", mois))
}

fn enregistrer(chemin: &Path, resultat: &Value, entrees: &BTreeMap<String, String>) -> Resultat<()> {
    let mut f = Map::new();
    f.insert("empreinte".into(), Value::String(empreinte(entrees)));
    f.insert("entrees".into(), Value::Object(entrees.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()));
    f.insert("resultat".into(), resultat.clone());
    crate::ecriture::ecrire_json(chemin, &Value::Object(f), 2)
}

/// Le fichier stocke : `(resultat, entrees, integre)`. `integre` : l'empreinte
/// dit bien ce que la liste contient.
fn stocke(chemin: &Path) -> Option<(Value, BTreeMap<String, String>, bool)> {
    let f = fichiers::lire_json(chemin).ok()?;
    let resultat = f.get("resultat").filter(|r| r.is_object())?.clone();
    let mut entrees = BTreeMap::new();
    let mut integre = true;
    match f.get("entrees") {
        Some(Value::Object(m)) => {
            for (k, v) in m {
                match v.as_str() {
                    Some(s) => {
                        entrees.insert(k.clone(), s.to_string());
                    }
                    None => integre = false,
                }
            }
        }
        _ => integre = false,
    }
    integre = integre && f.get("empreinte").and_then(Value::as_str) == Some(empreinte(&entrees).as_str());
    Some((resultat, entrees, integre))
}

/// Les entrees valent-elles encore ce qu'elles valaient ? Chacune est relue
/// directement sur le disque ; aucun calcul.
fn a_jour(o: &HashMap<String, String>, entrees: &BTreeMap<String, String>) -> bool {
    match contexte(o) {
        Ok(ctx) => a_jour_avec(o, &ctx, entrees),
        Err(_) => false,
    }
}

/// `a_jour` avec le Contexte que l'appelant a deja lu (aucune relecture des fiches).
fn a_jour_avec(o: &HashMap<String, String>, ctx: &Contexte, entrees: &BTreeMap<String, String>) -> bool {
    let (racine, code) = (racine_de(o), dossier_du_code());
    entrees.iter().all(|(k, v)| match k.as_str() {
        "moteur" => v == MOTEUR,
        "fiches" => *v == empreinte_fiches(ctx),
        "date" => *v == ctx.date.iso(),
        "aujourdhui" => crate::contexte::aujourdhui_sans_note().is_ok_and(|j| *v == j.iso()),
        _ => match fichiers::chemin_de(k, &racine, &code) {
            Some((genre, p)) => match genre.as_str() {
                "fichier" => *v == fichiers::empreinte_fichier(&p),
                "existe" => *v == if p.exists() { "oui" } else { "non" },
                "dossier?" => *v == if p.is_dir() { "oui" } else { "non" },
                "dossier" => *v == fichiers::empreinte_dossier(&p),
                _ => false,
            },
            None => false,
        },
    })
}

fn demande(entree: &str) -> Resultat<(String, Value)> {
    let d: Value = serde_json::from_str(entree).map_err(|e| format!("demande illisible : {}", e))?;
    let mois = d.get("mois").and_then(Value::as_str).filter(|m| mois_valide(m)).ok_or("demande illisible : il faut `mois` (AAAA-MM)")?;
    Ok((mois.to_string(), d))
}

/// Le mois est « parti » quand le journal des depots porte une DSN REELLE
/// acceptee pour lui (`etats::dsn_du_mois`) : ses chiffres ne bougent plus.
pub fn mois_parti(ctx: &Contexte, mois: &str) -> Resultat<bool> {
    let lignes = crate::etats::charger(&ctx.racine).map_err(|e| e.message)?;
    let depot = crate::etats::dsn_du_mois(&lignes, &ctx.entreprise, mois).map_err(|e| e.message)?;
    Ok(fichiers::vrai(&depot))
}

/// Le calcul du mois, `{"mois": "AAAA-MM", "parti": bool}` : la meme reponse que
/// `calculer_suivi` de `{"fichier": paies/AAAA-MM.json}`. Le mois est
/// parti si le journal le dit ; `parti` de la demande ne peut que l'ajouter.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let (mois, d) = demande(entree)?;
    let ctx = contexte(o);
    let du_journal = match &ctx {
        Ok(c) => mois_parti(c, &mois)?,
        Err(_) => false,
    };
    let parti = d.get("parti").and_then(Value::as_bool).unwrap_or(false) || du_journal;
    calcul_avec(o, ctx.as_ref().ok(), &mois, parti)
}

/// Le calcul du mois pour un Contexte deja lu (`None` : il ne se lit pas, le
/// calcul le dira). Relu sans calcul si ses entrees n'ont pas change.
pub fn calcul_avec(o: &HashMap<String, String>, ctx: Option<&Contexte>, mois: &str, parti: bool) -> Resultat<Value> {
    Ok(calcul_et_entrees(o, ctx, mois, parti)?.0)
}

/// `calcul_avec`, et les entrees du resultat rendu : celles du calcul stocke quand il se relit, celles du calcul frais
/// sinon.
fn calcul_et_entrees(o: &HashMap<String, String>, ctx: Option<&Contexte>, mois: &str, parti: bool) -> Resultat<(Value, BTreeMap<String, String>)> {
    let racine = racine_de(o);
    let calcul = fichier_calcul(&racine, mois);
    if let Some((resultat, entrees, integre)) = stocke(&calcul) {
        if parti || (integre && ctx.is_some_and(|c| a_jour_avec(o, c, &entrees))) {
            return Ok((resultat, entrees));
        }
    }
    let saisie = racine.join("paies").join(format!("{}.json", mois));
    let demande_calcul = serde_json::json!({ "fichier": saisie.to_string_lossy() });
    let (rep, entrees) = calculer_suivi(o, &demande_calcul)?;
    if !parti {
        // Un dossier qui refuse l'ecriture ne change pas le resultat.
        let _ = enregistrer(&calcul, &rep, &entrees);
    }
    Ok((rep, entrees))
}

/// Ecrit `paies/AAAA-MM.calcul.json` avec ce que `calculer_suivi` a rendu pour une saisie en main dont le fichier vient
/// d'etre ecrit tel quel : `true` si le fichier est ecrit. Le fichier du mois, qui n'existait pas ou pas encore comme cela
/// quand le calcul a lu le disque, prend sa valeur d'aujourd'hui. Si le calcul a lu l'ancien fichier du mois, rien n'est
/// ecrit : son resultat n'est pas celui du nouveau.
pub fn ecrire_calcul(o: &HashMap<String, String>, mois: &str, resultat: &Value, mut entrees: BTreeMap<String, String>) -> bool {
    let racine = racine_de(o);
    let saisie = racine.join("paies").join(format!("{}.json", mois));
    let cle = |genre: &str, rel: &str| format!("{} racine:{}", genre, rel);
    let rel = format!("paies/{}.json", mois);
    if entrees.contains_key(&cle("fichier", &rel)) || resultat.get("mois").and_then(Value::as_str) != Some(mois) {
        false
    } else {
        entrees.insert(cle("fichier", &rel), fichiers::empreinte_fichier(&saisie));
        entrees.insert(cle("existe", &rel), "oui".into());
        let dossier = cle("dossier", "paies");
        if entrees.contains_key(&dossier) {
            entrees.insert(dossier, fichiers::empreinte_dossier(&racine.join("paies")));
        }
        enregistrer(&fichier_calcul(&racine, mois), resultat, &entrees).is_ok()
    }
}

/// `{"mois", "resultat"}`, un calcul frais de
/// `paies/AAAA-MM.json` tel qu'il est sur le disque. `constat` : `absent` (pas
/// de fichier stocke ou illisible), `perime` (ses entrees ont change), `egal`,
/// ou `ecart` : memes entrees, autre resultat. Un `ecart` veut dire que le
/// fichier a ete modifie a la main, ou qu'une entree du calcul echappe a
/// l'empreinte.
pub fn comparer(o: &HashMap<String, String>, entree: &str) -> Resultat<Value> {
    let (mois, d) = demande(entree)?;
    let frais = d.get("resultat").filter(|r| r.is_object()).ok_or("demande illisible : il faut `resultat`")?;
    let constat = match stocke(&fichier_calcul(&racine_de(o), &mois)) {
        None => "absent",
        Some((_, entrees, integre)) if !integre || !a_jour(o, &entrees) => "perime",
        Some((resultat, _, _)) if &resultat == frais => "egal",
        Some(_) => "ecart",
    };
    Ok(serde_json::json!({ "ok": true, "constat": constat }))
}

// ------------------------------------------------------------------ la carte figee

/// LA CARTE FIGEE D'UNE PAIE VALIDEE (Leandre, 16/10/2026 : « quand on clique sur Valider, ca doit creer la carte simple qui,
/// elle, va etre affichee tout le temps sans passer par des calculs -- sauf dans les cas specifiques ou il y a des
/// changements de taux »).
///
/// `valider-paie` ecrit, dans le champ `valide` de l'engagement, `{"le", "carte", "empreinte"}` : `carte` est la vue de
/// l'apercu de cette paie (`routes_ouverture::vue_paie`, ce que la carte fermee et « Voir le detail » montrent), sans ses
/// numeros, qui se lisent en direct ; `empreinte` est ce dont ce calcul depend. La carte sert a l'AFFICHAGE, rien d'autre :
/// DSN, bordereau, bulletins, certificats et exports calculent depuis la saisie, comme avant.
///
/// L'empreinte, ce sont les entrees du calcul du mois (`calculer_suivi` : tout ce que le calcul a lu, fichier par fichier :
/// baremes du socle, couches de la convention et de la compagnie, fiches, effectifs, imprevus, paies des mois precedents,
/// moteur...), SAUF la saisie du mois, qui change a chaque frappe dans une autre carte. A sa place : la saisie telle que CE
/// calcul la lit -- le mois, et les engagements jusqu'a celui-ci (`calcul::calculer_apres` lit les paies d'avant dans le
/// mois), sans ce que le moteur tient seul (`valide`, `rouverte`) ni le numero (pose a la production, il ne change aucun
/// montant). Rouvrir retire `valide` : la carte part avec lui.
pub const CARTE: &str = "carte";

/// Ce qu'un engagement dit de sa validation : l'horodatage (`le`), ou rien.
pub fn valide_le(v: Option<&Value>) -> Value {
    v.and_then(|v| v.get("le")).cloned().unwrap_or(Value::Null)
}

/// L'empreinte de la saisie telle que le calcul de l'engagement `i` la lit.
fn empreinte_saisie(data: &Value, i: usize) -> String {
    let mut m = data.as_object().cloned().unwrap_or_default();
    m.remove("_origine");
    m.remove("numeros_figes");
    let engagements: Vec<Value> = data
        .get("engagements")
        .and_then(Value::as_array)
        .map_or(&[][..], |a| &a[..])
        .iter()
        .take(i + 1)
        .map(|e| {
            let mut e = e.as_object().cloned().unwrap_or_default();
            for k in ["valide", "rouverte", "numero"] {
                e.remove(k);
            }
            Value::Object(e)
        })
        .collect();
    m.insert("engagements".into(), Value::Array(engagements));
    fichiers::sha256_hex(canonique(&Value::Object(m)).as_bytes())
}

/// Une valeur ecrite sans ce qui ne change rien a ce qu'elle dit : les cles triees, un nombre ecrit comme un flottant
/// (`180` et `180.0` sont le meme taux : la page ecrit l'un, un fichier ancien l'autre).
fn canonique(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut cles: Vec<&String> = m.keys().collect();
            cles.sort();
            let corps: Vec<String> = cles.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), canonique(&m[*k]))).collect();
            format!("{{{}}}", corps.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonique).collect::<Vec<_>>().join(",")),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), |f| format!("{:?}", f)),
        // Un montant ecrit en texte (« 190.0 », « 190 ») : le meme nombre.
        Value::String(t) if !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit() || c == b'.' || c == b'-') && t.parse::<f64>().is_ok() => {
            format!("{:?}", t.parse::<f64>().unwrap_or_default())
        }
        autre => autre.to_string(),
    }
}

/// Les entrees d'un calcul, sans la saisie du mois.
fn entrees_de_la_carte(entrees: &BTreeMap<String, String>, mois: &str) -> Map<String, Value> {
    let saisie = format!("racine:paies/{}.json", mois);
    entrees
        .iter()
        .filter(|(k, _)| k.split_once(' ').map_or(true, |(_, c)| c != saisie))
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect()
}

/// Le `valide` d'une paie qu'on valide : `le`, sa carte, et l'empreinte de ce dont elle depend.
pub fn sceller(le: &str, carte: &Value, data: &Value, i: usize, entrees: &BTreeMap<String, String>, mois: &str) -> Value {
    let mut carte = carte.clone();
    if let Some(c) = carte.as_object_mut() {
        c.remove("numero_bulletin");
        c.remove("numero_contrat");
    }
    serde_json::json!({
        "le": le,
        CARTE: carte,
        "empreinte": { "saisie": empreinte_saisie(data, i), "entrees": entrees_de_la_carte(entrees, mois) },
    })
}

/// LES PAIES VALIDEES AVANT LA CARTE FIGEE (Leandre, 16/10/2026 : tout ce qui est deja sur le poste profite du gain tout
/// de suite, sans rien rouvrir ni revalider). Leur `valide` n'etait qu'un horodatage : au premier passage du moteur sur le
/// mois (`routes_mois::etat_par_paie`, que la file des etats refait pour chaque mois a l'ouverture), chacune recoit sa
/// carte, une fois, depuis le calcul du mois -- celui que l'ecran montrait deja pour elle (`calcul_avec` : le calcul
/// garde s'il tient, et, mois parti, les chiffres stockes, comme l'apercu) --, et garde sa date de validation dans `le`.
///
/// SEUL `valide` CHANGE : ni le numero de contrat (regle 4 : il se lit, il ne se recalcule pas), ni rien d'autre de la
/// saisie ; un mois dont la DSN est partie se scelle comme les autres, ce qui a ete declare ne bouge pas. Le fichier se
/// relit juste avant d'etre remplace : s'il a change pendant le calcul (une ecriture passait), rien ne s'ecrit, et le
/// passage suivant scellera. L'ecriture est atomique (`ecriture::ecrire_json`). Un mois qui ne se calcule pas reste tel
/// quel : il n'y a pas de carte a figer.
pub fn sceller_les_anciennes(o: &HashMap<String, String>, ctx: &Contexte, mois: &str) {
    let chemin = ctx.racine.join("paies").join(format!("{}.json", mois));
    let Ok(octets) = std::fs::read(&chemin) else { return };
    let Ok(mut data) = serde_json::from_slice::<Value>(&octets) else { return };
    let anciennes: Vec<usize> = data
        .get("engagements")
        .and_then(Value::as_array)
        .map_or(&[][..], |a| &a[..])
        .iter()
        .enumerate()
        .filter(|(_, e)| e.get("valide").is_some_and(|v| fichiers::vrai(v) && !v.is_object()))
        .map(|(i, _)| i)
        .collect();
    if anciennes.is_empty() {
        return;
    }
    let parti = mois_parti(ctx, mois).unwrap_or(false);
    let Ok((rep, entrees)) = calcul_et_entrees(o, Some(ctx), mois, parti) else { return };
    let Ok((_, vues)) = crate::routes_ouverture::vues_du_calcul(ctx, &data, &rep, false) else { return };
    for i in anciennes {
        let (Some(vue), Some(ancien)) = (vues.get(i), data["engagements"][i].get("valide")) else { return };
        let le = ancien.as_str().map_or_else(|| ancien.to_string(), str::to_string);
        data["engagements"][i]["valide"] = sceller(&le, vue, &data, i, &entrees, mois);
    }
    if std::fs::read(&chemin).ok().as_deref() == Some(&octets[..]) {
        let _ = crate::ecriture::ecrire_json(&chemin, &data, 2);
    }
}

/// La carte figee de chaque engagement de `data` (`None` : pas validee), et si elle TIENT : son
/// empreinte est celle d'aujourd'hui. Aucun calcul : la saisie se hache, chaque entree se relit sur le disque.
pub fn cartes(o: &HashMap<String, String>, ctx: &Contexte, data: &Value) -> Vec<Option<(Value, bool)>> {
    let engagements = data.get("engagements").and_then(Value::as_array).map_or(&[][..], |a| &a[..]);
    engagements
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let v = e.get("valide")?.as_object()?;
            let carte = v.get(CARTE).filter(|c| c.is_object())?.clone();
            let emp = v.get("empreinte")?;
            let entrees: Option<BTreeMap<String, String>> = emp
                .get("entrees")?
                .as_object()?
                .iter()
                .map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
            let tient = emp.get("saisie").and_then(Value::as_str) == Some(empreinte_saisie(data, i).as_str())
                && entrees.is_some_and(|en| a_jour_avec(o, ctx, &en));
            Some((carte, tient))
        })
        .collect()
}

/// La carte d'une paie dont l'empreinte a bouge, devant le calcul d'aujourd'hui (`frais`, la meme vue) : la carte, telle
/// qu'elle a ete validee, et -- si le calcul ne donne plus la meme paie -- `revoir`, ce qu'il donne maintenant. Rien ne
/// change en silence : la paie n'est plus ce qu'on a valide, l'ecran le dit.
pub fn devant_le_calcul(carte: &Value, frais: &Value) -> Value {
    let sans = |v: &Value| -> Value {
        let mut m = v.as_object().cloned().unwrap_or_default();
        for k in ["numero_bulletin", "numero_contrat", "avertissements", "informations", "revoir"] {
            m.remove(k);
        }
        Value::Object(m)
    };
    let mut c = carte.clone();
    if canonique(&sans(carte)) != canonique(&sans(frais)) {
        c["revoir"] = serde_json::json!({
            "brut": frais.get("brut"), "net_a_payer": frais.get("net_a_payer"), "cout_employeur": frais.get("cout_employeur"),
        });
    }
    c
}
