//! La mise a jour d'une INSTALLATION, de bout en bout (`paie::maj` ; lot V1, refait par le lot U1 le 14/10/2026) :
//! l'editeur publie dans un dossier ; une installation (celle que pose `atelier/installeur/` : `logiciel/` avec son
//! `EDITION.json`, des compagnies a cote) y trouve la version POUR ELLE-MEME, sans qu'aucune compagnie soit ouverte, la
//! telecharge, la controle, la range dans `<installation>/maj/`, l'applique, GARDE SON BRIDAGE (`EDITION.json`,
//! `.edition.json`), puis revient. Le poste de l'editeur recoit comme les autres (Leandre, 14/10/2026 : « mon logiciel
//! c'est le meme que les autres […] quand je fais une mise a jour, il faut que la mise a jour arrive aussi sur ma
//! machine »).
//!
//! La cle de signature est une cle d'essai (graine fixe) : le moteur livre ne connait que `maj::CLE_PUBLIQUE`, et ces
//! essais passent par les fonctions qui la recoivent en argument (`publier_signe`, `verifier_avec`). Le programme de
//! l'archive est le `paie` des essais, copie sous le nom `puppetpaie` : c'est lui que `controler` lance sur chaque
//! compagnie de l'installation. Rien ne sort du dossier du test, rien ne touche le reseau.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use paie::maj;
use serde_json::{json, Value};

use crate::outils::{code_livre, compagnie, copier, dossier_de_test};

const GRAINE: [u8; 32] = [42u8; 32];

fn publique() -> String {
    let k = ed25519_dalek::SigningKey::from_bytes(&GRAINE);
    paie::retours::base64_ecrire(k.verifying_key().as_bytes())
}

/// Un `logiciel/` : VERSION, les donnees livrees, une page, et le programme (le `paie` des essais, nomme `puppetpaie`).
fn logiciel(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("donnees"));
    std::fs::create_dir_all(dossier.join("interface")).unwrap();
    std::fs::write(dossier.join("interface").join("index.html"), format!("<p>{}</p>\n", version)).unwrap();
    std::fs::write(dossier.join("VERSION"), format!("{}\n", version)).unwrap();
    let exe = maj::programme(dossier, "puppetpaie");
    std::fs::copy(env!("CARGO_BIN_EXE_paie"), &exe).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dossier.to_path_buf()
}

fn lire(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// L'editeur : sa version 2026.11.01, publiee dans `<base>/depot`. Rend le depot.
fn publiee(base: &Path, avec_edition: bool) -> PathBuf {
    let depot = base.join("depot");
    let editeur = logiciel(&base.join("editeur").join("logiciel"), "2026.11.01");
    if avec_edition {
        std::fs::write(editeur.join("EDITION.json"), "{\"edition\": \"bride\"}\n").unwrap();
    }
    let r = maj::publier_signe(&editeur, &depot, &GRAINE, &publique(), "Essai de bout en bout", "", "").unwrap();
    assert_eq!(r["version"], "2026.11.01", "l'identifiant reste AAAA.MM.JJ dans derniere.json");
    assert!(depot.join("puppetpaie-2026.11.01.zip").is_file() && depot.join("derniere.json").is_file());
    depot
}

/// L'installation de l'installeur : `logiciel/` en 2026.10.09 et l'`EDITION.json` de l'installeur (bride). Rend
/// (installation, son EDITION.json).
fn installee(base: &Path) -> (PathBuf, Vec<u8>) {
    let inst = base.join("installation");
    let code = logiciel(&inst.join("logiciel"), "2026.10.09");
    let edition = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../installeur/EDITION.json")).unwrap();
    std::fs::write(code.join("EDITION.json"), &edition).unwrap();
    (inst, edition)
}

/// Une compagnie telle que la cree « Ajouter une compagnie » (`interface::compagnies::ajouter`) : trois fiches.
fn ajoutee(racine: &Path) {
    crate::outils::ecrire(racine, "config/entreprise.json", &json!({"nom_affichage": "Essai", "raison_sociale": "ESSAI"}));
    crate::outils::ecrire(racine, "config/salaries.json", &json!({}));
    crate::outils::ecrire(racine, "config/spectacles.json", &json!({}));
}

/// L'installation bridee de l'installeur recoit la version publiee POUR ELLE-MEME (aucune compagnie ouverte : la
/// recherche ne prend que l'installation), la range a cote de `logiciel/`, l'applique en gardant son `EDITION.json`, et
/// revient a la precedente. Rien ne se range dans une compagnie.
#[test]
fn une_installation_recoit_la_version_publiee_garde_son_bridage_et_revient() {
    let base = dossier_de_test("mise_a_jour", "bout_en_bout");
    let depot = publiee(&base, false);
    let (inst, edition) = installee(&base);
    let code = inst.join("logiciel");
    let racine = inst.join("compagnies").join("essai");
    ajoutee(&racine);
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));

    // Une cle qui n'est pas celle de l'editeur : rien n'est prepare.
    let refus = maj::verifier_avec(&inst, true, &maj::cles_de_l_editeur());
    assert!(refus.contains("n'a pas pu etre preparee") && refus.contains("signature"), "{}", refus);
    assert!(maj::prete(&inst).is_none());

    // La bonne : telechargee, controlee (le programme neuf lit la configuration de la compagnie), prete.
    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(dit.contains("01.11.2026 est telechargee, controlee, prete"), "{}", dit);
    assert!(dit.starts_with("version 09.10.2026"), "la version se lit JJ.MM.AAAA : {}", dit);
    assert_eq!(maj::prete(&inst).unwrap()["version"], "2026.11.01");
    assert!(inst.join("maj").join("2026.11.01").join("logiciel").join("VERSION").is_file(), "rangee a cote de logiciel/");
    assert!(!racine.join("maj").exists(), "rien dans la compagnie");
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"));

    // Appliquee : la neuve est en place, la precedente dans .avant/, et l'EDITION.json de l'installation est la.
    let r = maj::appliquer(&inst, false).unwrap();
    assert_eq!(r["version"], "2026.11.01");
    assert_eq!(maj::courante(&code), "2026.11.01");
    assert_eq!(lire(&code.join("interface").join("index.html")), "<p>2026.11.01</p>\n");
    assert_eq!(std::fs::read(code.join("EDITION.json")).ok().as_deref(), Some(edition.as_slice()), "l'installation reste bridee");
    assert_eq!(maj::courante(&code.join(".avant")), "2026.10.09");
    assert!(maj::prete(&inst).is_none() && maj::applicable(&inst).is_none(), "plus rien a appliquer");
    let applique: Value = serde_json::from_str(&lire(&inst.join("maj").join("appliquee.json"))).unwrap();
    assert_eq!(applique["version"], "2026.11.01");
    assert_eq!(maj::verifier_avec(&inst, true, &[&publique()]), "version 01.11.2026, a jour");

    // La marche arriere : la precedente revient, avec son EDITION.json.
    let r = maj::revenir(&inst).unwrap();
    assert_eq!(r["version"], "2026.10.09");
    assert_eq!(maj::courante(&code), "2026.10.09");
    assert_eq!(std::fs::read(code.join("EDITION.json")).ok().as_deref(), Some(edition.as_slice()));
    assert!(!code.join(".avant").exists() && !code.join(".apres").exists());
}

/// Une installation DEBLOQUEE (le geste de l'accueil a ecrit `.edition.json` a cote de `logiciel/` ; l'`EDITION.json` de
/// l'installeur est toujours la) reste debloquee : la mise a jour ne touche ni l'un ni l'autre, meme si le logiciel de
/// l'editeur porte son propre `EDITION.json` (il ne part pas dans l'archive). Leandre, 14/10/2026 : « le petit curseur
/// ecrit en JSON lui il reste comme il etait. Si les gens ont debloque, elle reste debloquee ».
#[test]
fn une_installation_debloquee_reste_debloquee() {
    let base = dossier_de_test("mise_a_jour", "installation_debloquee");
    let depot = publiee(&base, true);
    let (inst, edition) = installee(&base);
    let code = inst.join("logiciel");
    let curseur = "{\"edition\": \"debloque\", \"_quoi\": \"le geste de l'accueil\"}";
    std::fs::write(inst.join(".edition.json"), curseur).unwrap();
    ajoutee(&inst.join("compagnies").join("essai"));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(dit.contains("prete"), "{}", dit);
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&code), "2026.11.01");
    assert_eq!(lire(&inst.join(".edition.json")), curseur, "le choix du geste est intact");
    assert_eq!(std::fs::read(code.join("EDITION.json")).ok().as_deref(), Some(edition.as_slice()), "l'EDITION.json de l'installation aussi");
}

/// Une installation BRIDEE dont les seules compagnies sont les fictives (et une copie de developpement) recoit et applique
/// comme les autres, et reste bridee : le bridage ne joue aucun role dans la mise a jour. Le programme neuf est controle
/// sur la fictive ; la copie de developpement n'est pas une compagnie de l'installation.
#[test]
fn une_installation_bridee_recoit_applique_et_reste_bridee() {
    let base = dossier_de_test("mise_a_jour", "installation_bridee");
    let depot = publiee(&base, false);
    let (inst, edition) = installee(&base);
    let fictive = inst.join(paie::interface::compagnies::DOSSIER_FICTIVES).join("fictive-3090-petite");
    compagnie::config_et_mois("fictive-3090-petite", &[], &fictive);
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), fictive.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    let dev = inst.join("compagnies").join("copie-dev");
    crate::outils::ecrire(&dev, "config/entreprise.json", &json!({"nom_affichage": "Copie"}));
    crate::outils::ecrire(&dev, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": "prive"}));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));

    assert_eq!(maj::compagnies_de(&inst), vec![fictive.clone()], "la fictive, pas la copie de developpement");
    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(dit.contains("01.11.2026 est telechargee, controlee, prete"), "{}", dit);
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
    assert_eq!(std::fs::read(inst.join("logiciel").join("EDITION.json")).ok().as_deref(), Some(edition.as_slice()), "l'installation reste bridee");
    assert!(!inst.join(".edition.json").exists(), "aucun choix bride/debloque n'a ete ecrit");
}

/// LE POSTE DE L'EDITEUR RECOIT une version plus recente que la sienne, comme les autres (Leandre, 14/10/2026). Il est
/// regle a l'ancienne : `prive/config/maj.json`, `"editeur": true` et son `depot`, lu en repli faute de
/// `maj/reglage.json`. Sans aucune compagnie, une installation recoit aussi (rien a controler que le programme).
#[test]
fn le_poste_de_l_editeur_recoit_une_version_plus_recente() {
    let base = dossier_de_test("mise_a_jour", "poste_editeur");
    let depot = publiee(&base, false);
    let inst = base.join("poste-editeur");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    crate::outils::ecrire(&inst, "prive/config/maj.json", &json!({"editeur": true, "depot": depot.display().to_string()}));
    assert_eq!(maj::reglage(&inst).unwrap(), (Some(depot.display().to_string()), true), "le reglage d'avant, en repli");
    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(dit.contains("01.11.2026 est telechargee, controlee, prete"), "{}", dit);
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"));
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
    // L'editeur qui publie depuis son propre `logiciel/` a deja la version publiee : rien a recevoir.
    assert_eq!(maj::verifier_avec(&inst, true, &[&publique()]), "version 01.11.2026, a jour");
    // Le reglage de l'installation, s'il est pose, passe avant le reglage d'avant.
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": "/ailleurs"}));
    assert_eq!(maj::reglage(&inst).unwrap(), (Some("/ailleurs".to_string()), false));
}

/// LE GARDE-FOU DU CALCUL, pour l'installation : une version qui change un calcul attend la DSN des mois saisis de ses
/// compagnies REELLES ; une fictive ne la retient pas (elle ne depose jamais) ; « appliquer quand meme » passe outre.
#[test]
fn une_version_qui_change_un_calcul_attend_les_compagnies_reelles_pas_les_fictives() {
    let base = dossier_de_test("mise_a_jour", "garde_fou_calcul");
    let inst = base.join("installation");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    let neuf = inst.join("maj").join("2026.11.01").join("logiciel");
    logiciel(&neuf, "2026.11.01");
    crate::outils::ecrire(&inst, "maj/prete.json", &json!({"version": "2026.11.01", "calcul": "la CSG"}));
    // Une fictive avec un mois saisi sans DSN : elle ne retient rien.
    let fictive = inst.join(paie::interface::compagnies::DOSSIER_FICTIVES).join("fictive-3090-petite");
    compagnie::config_et_mois("fictive-3090-petite", &[], &fictive);
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), fictive.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    crate::outils::ecrire(&fictive, "paies/2030-01.json", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"), "une fictive ne retient pas la version");
    // Une compagnie reelle avec le meme mois : elle la retient.
    let reelle = inst.join("compagnies").join("reelle");
    compagnie::config_et_mois("fictive-3090-petite", &[], &reelle);
    crate::outils::ecrire(&reelle, "paies/2030-01.json", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    assert!(maj::applicable(&inst).is_none(), "le mois 2030-01 de la compagnie reelle attend sa DSN");
    let refus = maj::appliquer(&inst, false).unwrap_err();
    assert!(refus.contains("2030-01 (reelle)") && refus.contains("--malgre-calcul"), "{}", refus);
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.10.09");
    maj::appliquer(&inst, true).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
}

// ------------------------------------------------------------------ la publication par GitHub (lot P1, 14/10/2026)

fn publique_de(graine: &[u8; 32]) -> String {
    paie::retours::base64_ecrire(ed25519_dalek::SigningKey::from_bytes(graine).verifying_key().as_bytes())
}

/// Une Release : la version de `logiciel/` signee par `graine` (comme le flux de GitHub, par la graine en base64), dans
/// `<base>/releases/<etiquette>/`. Rend le dossier.
fn release(base: &Path, version: &str, graine: &[u8; 32]) -> PathBuf {
    let editeur = logiciel(&base.join(format!("editeur-{}", version)).join("logiciel"), version);
    let dossier = base.join("releases").join(version);
    maj::publier_par_graine(&editeur, &dossier, &paie::retours::base64_ecrire(graine), &publique_de(graine), "Essai", "", "").unwrap();
    dossier
}

/// Un faux GitHub : `/releases/latest/download/<f>` redirige (302) vers `/releases/download/<etiquette>/<f>`, qui
/// redirige vers `/stockage/<etiquette>/<f>`, servi depuis `<base>/releases/<etiquette>/` (le releve du 14/10/2026 :
/// `atelier/reference/sources/github/LISEZ-MOI.md`, « La derniere Release »). `derniere` dit l'etiquette de la derniere.
fn faux_github(base: &Path, derniere: Arc<Mutex<String>>) -> (String, crate::outils::Recu) {
    let releases = base.join("releases");
    crate::outils::serveur(move |q| {
        let vers = |url: String| format!("HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", url).into_bytes();
        let morceaux: Vec<&str> = q.chemin.trim_start_matches('/').split('/').collect();
        Some(match morceaux.as_slice() {
            ["releases", "latest", "download", f] => vers(format!("{}/releases/download/{}/{}", q.base, derniere.lock().unwrap(), f)),
            ["releases", "download", e, f] => vers(format!("{}/stockage/{}/{}", q.base, e, f)),
            ["stockage", e, f] => match std::fs::read(releases.join(e).join(f)) {
                Ok(o) => crate::outils::http(200, "application/octet-stream", &o),
                Err(_) => crate::outils::http(404, "text/plain", b"Not Found"),
            },
            _ => crate::outils::http(404, "text/plain", b"Not Found"),
        })
    })
}

/// LES DEUX CLES DE L'EDITEUR (celle du poste, celle de GitHub) sont acceptees, UNE TROISIEME est refusee, et
/// l'installation recoit d'un depot qui redirige comme les Releases de GitHub, sans autre reglage que l'adresse. Le
/// moteur livre lit, lui, la derniere Release du depot public (`DEPOT_PAR_DEFAUT`) avec ses deux cles.
#[test]
fn deux_cles_acceptees_une_troisieme_refusee_par_un_depot_qui_redirige_comme_github() {
    assert_eq!(maj::depot(None), "https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download");
    assert!(maj::cles_de_l_editeur().contains(&maj::CLE_PUBLIQUE), "la cle du poste de l'editeur reste acceptee");
    assert!(maj::cles_de_l_editeur().iter().all(|c| !c.is_empty()), "une cle non reportee (vide) n'est pas une cle");

    let base = dossier_de_test("mise_a_jour", "github");
    let (poste, github, autre) = ([42u8; 32], [7u8; 32], [9u8; 32]);
    let cles = [publique_de(&poste), publique_de(&github)];
    let cles: Vec<&str> = cles.iter().map(String::as_str).collect();
    release(&base, "2026.11.01", &autre);
    release(&base, "2026.11.02", &github);
    release(&base, "2026.12.01", &poste);
    let derniere = Arc::new(Mutex::new("2026.11.01".to_string()));
    let (url, recu) = faux_github(&base, derniere.clone());
    let (inst, _) = installee(&base);
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": format!("{}/releases/latest/download", url)}));

    // Signee par une troisieme cle : refusee, rien n'est prepare.
    let dit = maj::verifier_avec(&inst, true, &cles);
    assert!(dit.contains("n'a pas pu etre preparee") && dit.contains("signature"), "{}", dit);
    assert!(maj::prete(&inst).is_none());

    // Signee par la cle de GitHub : recue en suivant les redirections, appliquee.
    *derniere.lock().unwrap() = "2026.11.02".into();
    let dit = maj::verifier_avec(&inst, true, &cles);
    assert!(dit.contains("02.11.2026 est telechargee, controlee, prete"), "{}", dit);
    let chemins: Vec<String> = recu.lock().unwrap().iter().map(|q| q.chemin.clone()).collect();
    assert!(chemins.contains(&"/releases/latest/download/puppetpaie-2026.11.02.zip".to_string()), "{:?}", chemins);
    assert!(chemins.contains(&"/stockage/2026.11.02/puppetpaie-2026.11.02.zip".to_string()), "les redirections sont suivies : {:?}", chemins);
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.02");

    // Signee par la cle du poste : recue aussi.
    *derniere.lock().unwrap() = "2026.12.01".into();
    let dit = maj::verifier_avec(&inst, true, &cles);
    assert!(dit.contains("01.12.2026 est telechargee, controlee, prete"), "{}", dit);
}

/// LA PUBLICATION PAR GITHUB (`publier_par_graine`, le coeur de `paie maj --publier-github`) refuse quand la cle
/// publique n'est pas reportee, quand la graine n'est pas la sienne, et quand la version n'est pas plus recente que la
/// derniere Release ; elle n'ecrit rien alors, et aucun message ne porte la graine.
#[test]
fn la_publication_par_github_refuse_sans_cle_reportee_mauvaise_graine_ou_version_pas_plus_recente() {
    let base = dossier_de_test("mise_a_jour", "publier_github_refus");
    let editeur = logiciel(&base.join("editeur").join("logiciel"), "2026.11.01");
    let (graine, autre) = ([7u8; 32], [9u8; 32]);
    let g64 = paie::retours::base64_ecrire(&graine);
    let sortie = base.join("sortie");
    let sans_rien = |e: &str| {
        assert!(!e.contains(&g64), "la graine n'apparait jamais : {}", e);
        assert!(!sortie.join("derniere.json").exists());
    };
    let e = maj::publier_par_graine(&editeur, &sortie, &g64, "", "", "", "").unwrap_err();
    assert!(e.contains("CLE_PUBLIQUE_GITHUB"), "{}", e);
    sans_rien(&e);
    let e = maj::publier_par_graine(&editeur, &sortie, &g64, &publique_de(&autre), "", "", "").unwrap_err();
    assert!(e.contains("n'est pas celle"), "{}", e);
    sans_rien(&e);
    let e = maj::publier_par_graine(&editeur, &sortie, "pas une graine", &publique_de(&graine), "", "", "").unwrap_err();
    sans_rien(&e);

    // La derniere Release (son derniere.json, que le flux telecharge dans la sortie) est deja en 2026.11.01.
    let r = maj::publier_par_graine(&editeur, &sortie, &g64, &publique_de(&graine), "", "", "").unwrap();
    assert_eq!(r["version"], "2026.11.01");
    let e = maj::publier_par_graine(&editeur, &base.join("sortie"), &g64, &publique_de(&graine), "", "", "").unwrap_err();
    assert!(e.contains("deja en 01.11.2026"), "{}", e);
}

/// LA CLE DE GITHUB, de bout en bout par le programme : `paie maj --cle-github DOSSIER` ecrit la graine dans un fichier
/// et la cle publique a cote, sans jamais montrer la graine ; il refuse d'en refaire une tant que la graine est la.
/// `paie maj --publier-github` lit la graine dans `PUPPETPAIE_CLE_SIGNATURE` et ne la montre jamais (le moteur livre la
/// verifie contre `CLE_PUBLIQUE_GITHUB` ; reportee ou non, la graine d'un autre ne publie rien). La graine du fichier,
/// avec la cle publique ecrite a cote, signe une archive qu'une installation accepte.
#[test]
fn la_cle_de_github_se_fabrique_dans_un_fichier_et_ne_s_affiche_jamais() {
    let base = dossier_de_test("mise_a_jour", "cle_github");
    let dossier = base.join("maj");
    let paie_exe = |args: &[&str], env: &[(&str, &str)]| {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_paie"));
        c.arg("maj").args(args).env_remove(maj::VARIABLE_GRAINE);
        for (k, v) in env {
            c.env(k, v);
        }
        let s = c.output().unwrap();
        (s.status.code(), format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr)))
    };
    let (code, dit) = paie_exe(&["--cle-github", dossier.to_str().unwrap()], &[]);
    assert_eq!(code, Some(0), "{}", dit);
    let g64 = std::fs::read_to_string(dossier.join("cle-github-graine.txt")).unwrap();
    let publique = std::fs::read_to_string(dossier.join("cle-github-publique.txt")).unwrap().trim().to_string();
    let graine: [u8; 32] = paie::retours::base64_lire(&g64).unwrap().try_into().unwrap();
    assert_eq!(publique_de(&graine), publique, "la cle publique ecrite est celle de la graine");
    assert!(dit.contains(&publique) && !dit.contains(g64.trim()), "la publique se montre, la graine jamais : {}", dit);
    let (code, dit) = paie_exe(&["--cle-github", dossier.to_str().unwrap()], &[]);
    assert_eq!(code, Some(2), "{}", dit);
    assert_eq!(std::fs::read_to_string(dossier.join("cle-github-graine.txt")).unwrap(), g64, "rien n'est ecrase");

    // Le flux, avec le moteur livre : la graine vient de l'environnement et ne se montre pas.
    let editeur = logiciel(&base.join("editeur").join("logiciel"), "2026.11.01");
    let sortie = base.join("sortie");
    let (code, dit) = paie_exe(
        &["--publier-github", "--logiciel", editeur.to_str().unwrap(), "--sortie", sortie.to_str().unwrap()],
        &[(maj::VARIABLE_GRAINE, g64.trim())],
    );
    assert_eq!(code, Some(2), "cette graine n'est pas celle de CLE_PUBLIQUE_GITHUB : {}", dit);
    assert!(!dit.contains(g64.trim()), "{}", dit);
    assert!(!sortie.join("derniere.json").exists());
    let (code, dit) = paie_exe(&["--publier-github", "--logiciel", editeur.to_str().unwrap(), "--sortie", sortie.to_str().unwrap()], &[]);
    assert_eq!(code, Some(2), "sans la variable : {}", dit);

    // La meme graine, avec SA cle publique : l'installation qui la connait recoit.
    let depot = base.join("depot");
    maj::publier_par_graine(&editeur, &depot, &g64, &publique, "", "", "").unwrap();
    let (inst, _) = installee(&base);
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    let dit = maj::verifier_avec(&inst, true, &[maj::CLE_PUBLIQUE, &publique]);
    assert!(dit.contains("01.11.2026 est telechargee, controlee, prete"), "{}", dit);
}

/// `--cle-github` SANS DOSSIER, lance depuis une installation (le programme dans `<installation>/logiciel/`) : la graine
/// va dans `<installation>/maj/`, pas dans le dossier ou le programme a ete compile. Le 14/10/2026, la commande prenait
/// le chemin de compilation (`installation()` avant `fixer_le_code`) et Windows a ecrit la graine dans
/// `D:\home\user\puppetpaieprivate\maj\`.
#[test]
fn la_cle_de_github_va_dans_le_dossier_maj_de_l_installation() {
    let inst = dossier_de_test("mise_a_jour", "cle_github_installation");
    let code = logiciel(&inst.join("logiciel"), "2026.10.14");
    let s = std::process::Command::new(maj::programme(&code, "puppetpaie"))
        .args(["maj", "--cle-github"])
        .env_remove("PUPPETPAIE_CODE")
        .env_remove(maj::VARIABLE_GRAINE)
        .current_dir(&inst)
        .output()
        .unwrap();
    let dit = format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
    assert_eq!(s.status.code(), Some(0), "{}", dit);
    assert!(inst.join("maj").join("cle-github-graine.txt").is_file(), "{}", dit);
    assert!(inst.join("maj").join("cle-github-publique.txt").is_file(), "{}", dit);
}
