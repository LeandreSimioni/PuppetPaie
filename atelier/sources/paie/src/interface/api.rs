//! `/api/<route>` : ce que la page demande, rendu comme `web.py` le rendait (`{"ok", "resultat"}` ou
//! `{"ok": false, "erreur": {"type", "message", ...}}`).
//!
//! UNE ROUTE, UN PROCESSUS, comme du temps du Python (`paie route`, `paie_rust.route`) : une panique d'une route ne fait
//! pas tomber la fenetre, et l'etat d'une route (le mot de passe tape, les caches) meurt avec elle. Seuls restent ici
//! les gestes qui ne sont pas des routes du moteur : ouvrir un dossier dans l'explorateur, la veille, l'autorisation
//! Google (le navigateur s'ouvre sur la fenetre de consentement), et le filet qui repasse en fond.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};

use super::veille::Veille;
use crate::etat_garde::FileDesEtats;

/// Les refus qui ne sont pas des defauts du logiciel : une saisie en cours, un mot de passe mal tape, un mois sans
/// paie, et ceux que la page traite elle-meme (une fiche refusee, un depot deja fait...). On ne les compte pas en incidents.
const PAS_DES_INCIDENTS: [&str; 9] = [
    "SaisieIncomplete",
    "PreuveInsuffisante",
    "AucunDocument",
    "FicheRefusee",
    "DsnDejaDeposee",
    "MoisPasPret",
    "RepetitionManquante",
    "DpaeDejaDeclaree",
    "ModificationApresEcheance",
];

/// Un refus de route : sa classe (le `type` que la page lit), sa phrase, et ce qu'il porte pour la page.
pub struct Refus {
    pub classe: String,
    pub message: String,
    pub plus: Map<String, Value>,
}

impl Refus {
    pub fn simple(classe: &str, message: impl Into<String>) -> Refus {
        Refus { classe: classe.into(), message: message.into(), plus: Map::new() }
    }
}

/// Le programme qui sert les routes : ce binaire-ci (`paie`, ou la fenetre, qui sait aussi `route`).
fn programme() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("paie"))
}

/// Une route du moteur, dans son propre processus. Rend son `resultat`, ou son refus.
pub fn route(racine: &Path, nom: &str, charge: &Value) -> Result<Value, Refus> {
    let mut c = Command::new(programme());
    c.arg("route").arg("--racine").arg(racine).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Sans console : la fenetre ne fait pas clignoter une boite noire a chaque appel.
        c.creation_flags(0x0800_0000);
    }
    let mut p = c.spawn().map_err(|e| Refus::simple("ErreurOperation", format!("Le moteur ne se lance pas : {}.", e)))?;
    let demande = serde_json::to_vec(&json!({"route": nom, "charge": charge})).unwrap_or_default();
    if let Some(mut entree) = p.stdin.take() {
        let _ = entree.write_all(&demande);
    }
    let sortie = p.wait_with_output().map_err(|e| Refus::simple("ErreurOperation", format!("Le moteur ne repond pas : {}.", e)))?;
    let texte = String::from_utf8_lossy(&sortie.stdout);
    let Some(rep) = texte.lines().rev().find_map(|l| serde_json::from_str::<Value>(l.trim()).ok()) else {
        let err = String::from_utf8_lossy(&sortie.stderr);
        return Err(Refus::simple(
            "DefautDuLogiciel",
            format!("Defaut du logiciel : la route {} s'est arretee sans reponse ({}).", nom, err.trim().lines().last().unwrap_or("rien sur la sortie d'erreur")),
        ));
    };
    depuis_reponse(rep)
}

/// Une reponse de `paie route` (`{"ok", "resultat"}` ou `{"ok": false, "erreur", "classe", ...}`) : son resultat, ou son refus.
pub fn depuis_reponse(rep: Value) -> Result<Value, Refus> {
    if rep.get("ok").and_then(Value::as_bool) == Some(true) {
        return Ok(rep.get("resultat").cloned().unwrap_or(Value::Null));
    }
    let message = match rep.get("erreur") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "refus sans phrase".into(),
    };
    let classe = rep.get("classe").and_then(Value::as_str).unwrap_or("CalculRefuse").to_string();
    let mut plus = Map::new();
    if let Value::Object(m) = &rep {
        for (k, v) in m {
            if !["ok", "erreur", "classe"].contains(&k.as_str()) {
                plus.insert(k.clone(), v.clone());
            }
        }
    }
    Err(Refus { classe, message, plus })
}

/// Ouvre un fichier ou un dossier dans le programme du poste (l'explorateur, le lecteur PDF).
pub fn ouvrir(chemin: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let r = Command::new("explorer").arg(chemin).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(chemin).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(chemin).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    r.map(|_| ()).map_err(|e| format!("Impossible d'ouvrir {} : {}.", chemin.display(), e))
}

/// Ouvre une adresse dans le navigateur du poste.
pub fn ouvrir_adresse(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let r = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    r.map(|_| ()).map_err(|e| format!("Le navigateur ne s'ouvre pas : {}.", e))
}

/// Un texte pour une chaine PowerShell entre apostrophes : chaque apostrophe se double, et PowerShell en reconnait
/// cinq (l'apostrophe droite et les quatre typographiques U+2018 a U+201B) ; rien d'autre n'y est special.
pub fn entre_apostrophes_powershell(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            s.push(c);
        }
        s.push(c);
    }
    s
}

/// CHOISIR UN DOSSIER (Leandre, 08/10/2026 : « un bouton qui ouvre l'explorateur »). La boite du systeme, lancee par
/// le serveur du poste : Windows par PowerShell (`FolderBrowserDialog`, au premier plan), macOS par `osascript`, Linux
/// par `zenity` ou `kdialog`. Rend `{"dossier": chemin}`, ou `{"dossier": null}` si on a annule.
fn choisir_dossier(charge: &Value) -> Result<Value, Refus> {
    // Le titre garde son apostrophe (#141) : chaque boite l'echappe a sa facon -- PowerShell double l'apostrophe
    // dans sa chaine entre apostrophes (ou `$` et l'accent grave sont litteraux), AppleScript echappe `\` et `"`
    // dans sa chaine entre guillemets ; zenity le recoit en argument, tel quel.
    let titre: String = charge
        .get("titre")
        .and_then(Value::as_str)
        .unwrap_or("Choisir un dossier")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    #[cfg(windows)]
    let sortie = {
        use std::os::windows::process::CommandExt;
        let script = format!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms;              $f=New-Object System.Windows.Forms.Form -Property @{{TopMost=$true}};              $d=New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description='{}'; $d.ShowNewFolderButton=$false;              if($d.ShowDialog($f) -eq 'OK'){{ $d.SelectedPath }}",
            entre_apostrophes_powershell(&titre)
        );
        Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", &script])
            .creation_flags(0x0800_0000)
            .stdin(Stdio::null())
            .output()
    };
    #[cfg(target_os = "macos")]
    let sortie = Command::new("osascript")
        .args(["-e", &format!("POSIX path of (choose folder with prompt \"{}\")", titre.replace('\\', "\\\\").replace('"', "\\\""))])
        .stdin(Stdio::null())
        .output();
    #[cfg(all(unix, not(target_os = "macos")))]
    let sortie = Command::new("zenity")
        .args(["--file-selection", "--directory", &format!("--title={}", titre)])
        .stdin(Stdio::null())
        .output()
        .or_else(|_| Command::new("kdialog").args(["--getexistingdirectory", "."]).stdin(Stdio::null()).output());
    let sortie = sortie.map_err(|e| Refus::simple("ErreurOperation", format!("La boite de choix du dossier ne s'ouvre pas : {}. Tapez le chemin a la main.", e)))?;
    let chemin = String::from_utf8_lossy(&sortie.stdout).trim().trim_end_matches(['/', '\\']).to_string();
    Ok(json!({ "dossier": if chemin.is_empty() { Value::Null } else { Value::String(chemin) } }))
}

/// OUVRIR LE DOSSIER D'IMPORT (lot G2, 10/10/2026 : « un dossier, tout dedans ») : la boite aux lettres dans
/// l'explorateur du poste, pour y deposer ce que l'ancien logiciel a laisse. Vide ou absent : `import/` de la
/// compagnie (la page le preremplit), cree s'il n'existe pas encore ; un autre dossier doit exister, il n'est jamais
/// cree. Rend `{"dossier": chemin}`.
fn ouvrir_import(racine: &Path, charge: &Value) -> Result<Value, Refus> {
    let choisi = charge.get("dossier").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let import = racine.join("import");
    let dossier = choisi.map_or_else(|| import.clone(), |s| PathBuf::from(crate::routes_apercus::normaliser(s)));
    if dossier == import {
        std::fs::create_dir_all(&import)
            .map_err(|e| Refus::simple("ErreurOperation", format!("Le dossier {} ne se cree pas : {}.", import.display(), e)))?;
    }
    if !dossier.is_dir() {
        return Err(Refus::simple("ErreurOperation", format!("Le dossier {} n'existe pas.", dossier.display())));
    }
    ouvrir(&dossier).map_err(|m| Refus::simple("ErreurOperation", m))?;
    Ok(json!({"dossier": dossier.display().to_string()}))
}

fn texte(charge: &Value, cle: &str) -> Result<String, Refus> {
    charge.get(cle).and_then(Value::as_str).map(String::from).ok_or_else(|| Refus::simple("KeyError", crate::fichiers::absent(cle)))
}

fn ouvrir_piece(racine: &Path, charge: &Value) -> Result<Value, Refus> {
    // UNE PIECE DE L'HISTORIQUE (20/10/2026) : un document du mois ou la piece d'un retour, que `historique-mois` a
    // listee pour ce mois -- et rien d'autre : le chemin vient de la page, il ne s'ouvre que s'il est dans la liste.
    if let Some(mois) = charge.get("historique").and_then(Value::as_str) {
        let chemin = texte(charge, "chemin")?;
        let listage = route(racine, crate::historique_mois::ROUTE, &json!({"mois": mois}))?;
        if !crate::historique_mois::piece_listee(&listage, &chemin) {
            return Err(Refus::simple("ErreurOperation", format!("{} n'est pas une pièce de {} dans l'Historique.", chemin, mois)));
        }
        let f = racine.join(&chemin);
        if !f.is_file() {
            return Err(Refus::simple("ErreurOperation", format!("{} a été déplacé ou effacé.", chemin)));
        }
        ouvrir(&f).map_err(|m| Refus::simple("ErreurOperation", m))?;
        return Ok(json!({"fichier": chemin}));
    }
    let fichier = texte(charge, "fichier")?;
    let parties = route(racine, "pieces-parties", charge)?;
    let pieces: Vec<&Value> = match &parties {
        Value::Object(m) => m.values().filter_map(Value::as_array).flatten().collect(),
        Value::Array(a) => a.iter().collect(),
        _ => Vec::new(),
    };
    for p in pieces {
        if p.get("fichier").and_then(Value::as_str) == Some(fichier.as_str()) {
            let mois = p.get("mois").cloned().unwrap_or(Value::Null);
            return match crate::avenant::chemin_piece(racine, &mois, &Value::String(fichier.clone())) {
                Some(f) => {
                    ouvrir(&f).map_err(|m| Refus::simple("ErreurOperation", m))?;
                    Ok(json!({"fichier": fichier}))
                }
                None => Err(Refus::simple(
                    "ErreurOperation",
                    format!(
                        "{} n'est plus dans sorties/{} : le journal dit qu'il est parti, le fichier a ete deplace ou efface.",
                        fichier,
                        mois.as_str().unwrap_or("")
                    ),
                )),
            };
        }
    }
    Err(Refus::simple("ErreurOperation", format!("{} n'est pas une piece partie pour cette paie.", fichier)))
}

fn ouvrir_dossier(racine: &Path, charge: &Value) -> Result<Value, Refus> {
    let mois = texte(charge, "mois")?;
    if mois.contains(['/', '\\']) || mois.contains("..") {
        return Err(Refus::simple("ErreurOperation", format!("Mois illisible : {}.", mois)));
    }
    let mut dossier = racine.join("sorties").join(&mois);
    let envoi = dossier.join("envoi");
    let quoi = charge.get("quoi").and_then(Value::as_str).unwrap_or("");
    let contrats = || {
        std::fs::read_dir(&envoi)
            .map(|it| it.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("Contrat") && n.ends_with(".pdf")
            }))
            .unwrap_or(false)
    };
    if envoi.exists() && (quoi == "documents" || (quoi == "contrats" && contrats())) {
        dossier = envoi;
    }
    if !dossier.exists() {
        return Err(Refus::simple("ErreurOperation", format!("Rien n'est encore ecrit pour {}.", mois)));
    }
    ouvrir(&dossier).map_err(|m| Refus::simple("ErreurOperation", m))?;
    Ok(json!({"dossier": dossier.display().to_string()}))
}

fn ouvrir_sous(dossier: PathBuf, absent: &str) -> Result<Value, Refus> {
    if !dossier.exists() {
        return Err(Refus::simple("ErreurOperation", absent));
    }
    ouvrir(&dossier).map_err(|m| Refus::simple("ErreurOperation", m))?;
    Ok(json!({"dossier": dossier.display().to_string()}))
}

fn google_autoriser(racine: &Path, charge: &Value) -> Result<Value, Refus> {
    let vers = |f: crate::retours::Echec| Refus::simple(f.classe.unwrap_or("ErreurOperation"), f.message);
    let mot = charge.get("motdepasse").cloned().unwrap_or(Value::Null);
    crate::filet::exiger_mot_de_passe(racine, &mot, "lier le compte Google").map_err(vers)?;
    let url = |u: &str| {
        let _ = ouvrir_adresse(u);
    };
    let compte = crate::google_autorisation::autoriser(racine, crate::google_autorisation::DELAI_AUTORISATION, &url, &|_| {}).map_err(vers)?;
    Ok(json!({"compte": compte}))
}

/// Le filet repasse en fond apres une ecriture de mois ou un envoi de contrats : l'ecran n'attend pas l'Urssaf.
fn filet_en_fond(racine: &Path) {
    let r = racine.to_path_buf();
    std::thread::spawn(move || {
        let _ = route(&r, "filet-preparer", &json!({"qui": "filet"}));
    });
}

/// « Un probleme, une idee ? » part en fond (Leandre, 14/10/2026) : `signalement-envoyer` a enregistre le message et
/// rendu la main ; `signalements-partir` le fait partir, le temps du serveur de l'editeur, sans que la page attende.
fn signalements_en_fond(racine: &Path) {
    let r = racine.to_path_buf();
    std::thread::spawn(move || {
        let _ = route(&r, "signalements-partir", &json!({}));
    });
}

fn traiter(racine: &Path, nom: &str, charge: &Value, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Result<Value, Refus> {
    match nom {
        "ouvrir-piece" => ouvrir_piece(racine, charge),
        "ouvrir-dossier" => ouvrir_dossier(racine, charge),
        "ouvrir-exercices" => ouvrir_sous(racine.join("sorties").join("exercices"), "Rien n'est encore ecrit pour un exercice."),
        "ouvrir-exports" => {
            let mut d = racine.join("exports");
            if let Some(s) = charge.get("sous").and_then(Value::as_str).filter(|s| ["casc-svp", "dossiers"].contains(s)) {
                d = d.join(s);
            }
            ouvrir_sous(d, "Aucun export n'a encore ete ecrit : le dossier n'existe pas.")
        }
        "veille" => Ok(veille.map(Veille::etat).unwrap_or_else(|| json!({"minutes": 0, "dernier": {}, "en_cours": false}))),
        // Ou en est le passage, le temps qu'il tourne : une lecture sans processus (`Veille::etape`).
        "veille-etape" => Ok(veille.map(Veille::etape).unwrap_or_else(|| json!({"en_cours": false, "sauvegarde_en_cours": false, "etape": null}))),
        "veille-maintenant" => match veille {
            Some(v) => Ok(v.tour(false, false)),
            None => Err(Refus::simple("ErreurOperation", "Pas de veille sur cette interface.")),
        },
        "google-autoriser" => google_autoriser(racine, charge),
        "choisir-dossier" => choisir_dossier(charge),
        "ouvrir-import" => ouvrir_import(racine, charge),
        // `xlsx` et `pdf` : deux faits du poste que le Python disait a la route. Le Rust les fait toujours.
        "exports-periodes" => {
            let mut c = charge.as_object().cloned().unwrap_or_default();
            c.insert("xlsx".into(), Value::Bool(true));
            c.insert("pdf".into(), Value::Bool(true));
            route(racine, nom, &Value::Object(c))
        }
        "produire" | "demander-signature" => {
            let r = route(racine, nom, charge);
            if r.is_ok() {
                filet_en_fond(racine);
            }
            r
        }
        "signalement-envoyer" => {
            let r = route(racine, nom, charge);
            if r.is_ok() {
                signalements_en_fond(racine);
            }
            r
        }
        "apercu" => route(racine, nom, &json!({"saisie": charge.get("saisie").cloned().unwrap_or(Value::Null), "lignes": charge.get("lignes").cloned().unwrap_or(Value::Bool(false)), "urssaf": charge.get("urssaf").cloned().unwrap_or(Value::Bool(false))})),
        "rappels" => route(racine, nom, &json!({})),
        // L'ECRAN D'UN MOIS SE LIT ICI, SANS PROCESSUS (PLAN-MOIS-INSTANTANE.md, ticket #212) : le dernier etat connu du
        // mois, tout de suite, meme perime (marque `perime`, et refait en fond) ; `attendre` : l'etat frais.
        "mois" => {
            let mois = texte(charge, "mois")?;
            match etats {
                Some(f) => f
                    .ecran(&mois, charge.get("attendre").and_then(Value::as_bool) == Some(true))
                    .map_err(|e| Refus::simple(e.classe.unwrap_or("ErreurOperation"), e.message)),
                None => route(racine, nom, charge),
            }
        }
        _ => route(racine, nom, charge),
    }
}

/// La reponse a `/api/<nom>`, au format que la page attend.
pub fn repondre(racine: &Path, nom: &str, charge: &Value, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Value {
    let rendu = traiter(racine, nom, charge, veille, etats);
    // APRES UNE ECRITURE (ou un tour de veille demande), les etats de tous les mois se verifient en fond, le mois affiche
    // d'abord : une ecriture d'un mois peut changer les suivants (un contrat en cours, une numerotation, un cumul). Meme
    // refusee, une ecriture a pu toucher un fichier ; un mois deja juste ne coute que son empreinte.
    if let Some(f) = etats {
        if crate::routes::est_une_ecriture(nom) || nom == "veille-maintenant" {
            f.tout_verifier();
        }
    }
    match rendu {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(r) => {
            if !PAS_DES_INCIDENTS.contains(&r.classe.as_str()) && nom != "incident-navigateur" {
                let _ = crate::incidents::depuis_exception(racine, &r.classe, &r.message, None, nom, json!({"route": nom}), None);
            }
            en_page(Err(r))
        }
    }
}

/// Un resultat ou un refus, au format que la page attend (`{"ok", "resultat"}` ou `{"ok": false, "erreur": {"type", "message", ...}}`).
pub fn en_page(r: Result<Value, Refus>) -> Value {
    match r {
        Ok(v) => json!({"ok": true, "resultat": v}),
        Err(r) => {
            let mut e = Map::new();
            e.insert("type".into(), Value::String(r.classe));
            e.insert("message".into(), Value::String(r.message));
            for (k, v) in r.plus {
                e.insert(k, v);
            }
            json!({"ok": false, "erreur": e})
        }
    }
}

#[cfg(test)]
mod tests {
    /// #141 : le titre de la boite « choisir un dossier » garde son apostrophe ; PowerShell la recoit doublee (les cinq
    /// apostrophes qu'il reconnait), et rien ne sort de la chaine.
    #[test]
    fn le_titre_de_la_boite_garde_son_apostrophe() {
        use super::entre_apostrophes_powershell as ps;
        assert_eq!(ps("Le dossier de l'export"), "Le dossier de l''export");
        assert_eq!(ps("l\u{2019}export"), "l\u{2019}\u{2019}export");
        assert_eq!(ps("$a `b"), "$a `b");
    }
}
