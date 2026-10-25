//! La mise a jour d'une INSTALLATION, de bout en bout (`paie::maj` ; lot V1, refait par le lot U1 le 14/10/2026) :
//! l'editeur publie dans un dossier ; une installation (celle que pose `atelier/installeur/` : `logiciel/`, bridee faute
//! de `.edition.json`, des compagnies a cote) y trouve la version POUR ELLE-MEME, sans qu'aucune compagnie soit ouverte, la
//! telecharge, la controle, la range dans `<installation>/maj/`, l'applique, GARDE SON BRIDAGE (`EDITION.json`,
//! `.edition.json`), puis revient. Le poste de l'editeur recoit comme les autres (Leandre, 14/10/2026 : « mon logiciel
//! c'est le meme que les autres […] quand je fais une mise a jour, il faut que la mise a jour arrive aussi sur ma
//! machine »).
//!
//! La cle de signature est une cle d'essai (graine fixe) : le moteur livre ne connait que `maj::CLE_PUBLIQUE`, et ces
//! essais passent par les fonctions qui la recoivent en argument (`publier_signe`, `verifier_avec`). Le programme de
//! l'archive est le `paie` des essais, copie sous le nom de la fenetre de ce systeme (`linux/puppetpaie` ici) : c'est lui
//! que `controler` lance sur chaque compagnie de l'installation. Les programmes des autres systemes sont des fichiers
//! factices. Rien ne sort du dossier du test, rien ne touche le reseau.
//!
//! QUATRE ARCHIVES PAR VERSION (Leandre, 15/10/2026 : « Il y a les quatre : une par systeme et une tout-systeme,
//! ensemble ») : l'installation prend celle de son systeme, ou celle de tous si la sienne manque.

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

/// Un `logiciel/` : VERSION, les donnees livrees, une page, et le programme de ce systeme (le `paie` des essais, nomme
/// comme la fenetre : `linux/puppetpaie` ici).
fn logiciel(dossier: &Path, version: &str) -> PathBuf {
    copier(&code_livre(), &dossier.join("donnees"));
    std::fs::create_dir_all(dossier.join("interface")).unwrap();
    std::fs::write(dossier.join("interface").join("index.html"), format!("<p>{}</p>\n", version)).unwrap();
    std::fs::write(dossier.join("VERSION"), format!("{}\n", version)).unwrap();
    let exe = maj::programme(dossier, "puppetpaie");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_paie"), &exe).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dossier.to_path_buf()
}

/// Les quatre compagnies fictives livrees, a cote du logiciel `code` (`<code>/../fictives/`), comme dans le depot que
/// publie le flux : leur marqueur, leur fiche et leur configuration (la copie figee des essais). Sans elles, pas d'archive
/// d'installation, donc rien ne se publie (`maj::archiver_installation`).
const FICTIVES: [&str; 4] = ["fictive-1285-grande", "fictive-1285-petite", "fictive-3090-grande", "fictive-3090-petite"];

fn livrer_les_fictives(code: &Path) {
    let f = code.parent().unwrap().join("fictives");
    for nom in FICTIVES {
        let de = crate::outils::fictive(nom);
        copier(&de.join("config"), &f.join(nom).join("config"));
        for fichier in ["COPIE-DE-DEVELOPPEMENT.json", "FICTIVE.md"] {
            std::fs::copy(de.join(fichier), f.join(nom).join(fichier)).unwrap();
        }
    }
    std::fs::write(f.join("LISEZ-MOI.md"), "Les compagnies fictives.\n").unwrap();
}

fn lire(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

/// Les programmes des trois systemes, comme le flux « Publier » les pose dans `logiciel/` : ceux de ce systeme sont la
/// (`logiciel`), les autres sont des fichiers factices (un Linux ne lance ni un `.exe` ni un programme Mac).
fn tous_les_systemes(code: &Path) {
    let poser = |rel: &str, o: &[u8]| {
        let p = code.join(rel);
        if !p.exists() {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, o).unwrap();
        }
    };
    poser("puppetpaie.exe", b"MZ fenetre");
    poser("WebView2Loader.dll", b"MZ chargeur");
    for s in ["linux", "mac"] {
        poser(&format!("{}/puppetpaie", s), b"\x7fELF fenetre");
        poser(&format!("{}/paie", s), b"\x7fELF moteur");
    }
}

fn condensat(o: &[u8]) -> String {
    <sha2::Sha256 as sha2::Digest>::digest(o).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Les noms d'une archive, et le mode de chacun.
fn contenu(octets: &[u8]) -> Vec<(String, u32)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets.to_vec())).unwrap();
    (0..z.len())
        .map(|i| {
            let f = z.by_index(i).unwrap();
            (f.name().to_string(), f.unix_mode().unwrap_or(0) & 0o777)
        })
        .collect()
}

/// LA PUBLICATION FAIT QUATRE ARCHIVES, et `derniere.json` les nomme chacune avec son condensat et sa signature (valable,
/// par la cle de l'editeur, sur ses octets et pas sur ceux d'une autre). L'archive d'un systeme porte les fichiers
/// communs et ses programmes, pas ceux des autres ; celle de tous les systemes porte tout. Un zip ne garde pas le bit
/// d'execution : `archiver` le marque sur les programmes Linux et Mac, et sur eux seuls, pour que `deballer` les remette
/// executables. A cote, une archive D'INSTALLATION pour Linux, Mac et tous (`logiciel/` et les quatre fictives), que
/// `derniere.json` ne nomme pas.
#[test]
fn la_publication_fait_quatre_archives_signees_que_derniere_json_nomme() {
    let d = dossier_de_test("mise_a_jour", "quatre_archives");
    let code = logiciel(&d.join("logiciel"), "2026.11.03");
    livrer_les_fictives(&code);
    tous_les_systemes(&code);
    let depot = d.join("depot");
    let r = maj::publier_signe(&code, &depot, &GRAINE, &publique(), "", "", "").unwrap();
    let manifeste: Value = serde_json::from_str(&lire(&depot.join("derniere.json"))).unwrap();
    assert_eq!(manifeste["version"], "2026.11.03");
    assert_eq!(r["archives"], manifeste["archives"]);
    let archives = manifeste["archives"].as_object().unwrap();
    let mut cles: Vec<&str> = archives.keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["linux", "mac", "tout-systeme", "windows"]);
    assert_eq!(archives["tout-systeme"]["systemes"], json!(["windows", "linux", "mac"]), "celle de tous dit ce qu'elle porte");
    let mut octets = std::collections::BTreeMap::new();
    for (k, a) in archives {
        assert_eq!(a["archive"], format!("puppetpaie-2026.11.03-{}.zip", k));
        let o = std::fs::read(depot.join(a["archive"].as_str().unwrap())).unwrap();
        assert_eq!(a["sha256"], condensat(&o).as_str(), "{}", k);
        assert!(maj::signature_valide(&o, a["signature"].as_str().unwrap(), &publique()), "{} : signee par l'editeur", k);
        octets.insert(k.clone(), o);
    }
    assert!(!maj::signature_valide(&octets["linux"], archives["mac"]["signature"].as_str().unwrap(), &publique()), "chacune sa signature");
    assert_eq!(maj::derniere(&depot.display().to_string()).unwrap(), manifeste, "le recepteur relit ce qu'ecrit l'editeur");

    // LES ARCHIVES DE MISE A JOUR : le contenu de `logiciel/` a leur racine, et rien d'autre (le contrat de maj.rs).
    let programmes = |s: &str| -> Vec<&str> {
        match s {
            "windows" => vec!["puppetpaie.exe", "WebView2Loader.dll"],
            "linux" => vec!["linux/puppetpaie", "linux/paie"],
            _ => vec!["mac/puppetpaie", "mac/paie"],
        }
    };
    let un_systeme = |c: &[(String, u32)], k: &str, tete: &str| {
        let porte = |n: &str| c.iter().any(|(x, _)| x == &format!("{}{}", tete, n));
        for n in ["VERSION", "interface/index.html"] {
            assert!(porte(n), "{} porte {}{}", k, tete, n);
        }
        assert!(c.iter().any(|(x, _)| x.starts_with(&format!("{}donnees/", tete))), "{} porte les donnees", k);
        for s in maj::SYSTEMES {
            for n in programmes(s) {
                assert_eq!(porte(n), k == s || k == "tout-systeme", "{} et {}", k, n);
            }
        }
        for (n, mode) in c.iter().filter(|(n, _)| n.starts_with(tete)) {
            let unix = n.starts_with(&format!("{}linux/", tete)) || n.starts_with(&format!("{}mac/", tete));
            assert_eq!(*mode, if unix { 0o755 } else { 0o644 }, "{} : {}", k, n);
        }
    };
    for (k, o) in &octets {
        let c = contenu(o);
        un_systeme(&c, k, "");
        assert!(c.iter().all(|(x, _)| !x.starts_with("logiciel/") && !x.starts_with("fictives/")), "{} : ni logiciel/ ni fictives/ en tete", k);
    }

    // LES ARCHIVES D'INSTALLATION, a cote : Linux, Mac et tous (Windows a son installeur), ni signees ni dans derniere.json ;
    // `logiciel/` (le meme logiciel) et les quatre fictives livrees.
    let installations = r["installations"].as_object().unwrap();
    let mut cles: Vec<&str> = installations.keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["linux", "mac", "tout-systeme"]);
    assert!(manifeste.get("installations").is_none(), "derniere.json ne les nomme pas");
    for (k, a) in installations {
        assert_eq!(a["archive"], maj::nom_d_installation("2026.11.03", k));
        let o = std::fs::read(depot.join(a["archive"].as_str().unwrap())).unwrap();
        assert_eq!(a["sha256"], condensat(&o).as_str(), "{}", k);
        let c = contenu(&o);
        un_systeme(&c, k, "logiciel/");
        assert!(c.iter().any(|(x, _)| x == "fictives/LISEZ-MOI.md"), "{}", k);
        for nom in FICTIVES {
            assert!(c.iter().any(|(x, _)| x == &format!("fictives/{}/config/entreprise.json", nom)), "{} porte {}", k, nom);
        }
        assert!(c.iter().all(|(x, _)| x.starts_with("logiciel/") || x.starts_with("fictives/")), "{} : rien hors de logiciel/ et fictives/", k);
        let sans_tete: Vec<String> = c.iter().filter_map(|(x, _)| x.strip_prefix("logiciel/").map(str::to_string)).collect();
        let maj_du_systeme: Vec<String> = contenu(&octets[k.as_str()]).into_iter().map(|(x, _)| x).collect();
        assert_eq!(sans_tete, maj_du_systeme, "{} : le meme logiciel que l'archive de mise a jour", k);
    }

    // LES FICTIVES, a part : signees, nommees sous `fictives` avec leur empreinte ; leur archive porte exactement le
    // `fictives/` des archives d'installation, et rien d'autre.
    let f = &manifeste["fictives"];
    assert_eq!(f["archive"], maj::nom_des_fictives("2026.11.03"));
    let o = std::fs::read(depot.join(f["archive"].as_str().unwrap())).unwrap();
    assert_eq!(f["sha256"], condensat(&o).as_str());
    assert!(maj::signature_valide(&o, f["signature"].as_str().unwrap(), &publique()), "les fictives sont signees");
    assert_eq!(f["empreinte"].as_str().map(str::len), Some(64));
    let noms: Vec<String> = contenu(&o).into_iter().map(|(x, _)| x).collect();
    assert!(noms.iter().all(|x| x.starts_with("fictives/")), "{:?}", noms);
    for nom in FICTIVES {
        assert!(noms.contains(&format!("fictives/{}/.livree.json", nom)), "{} porte sa marque", nom);
    }
    let installation: Vec<String> = contenu(&std::fs::read(depot.join(maj::nom_d_installation("2026.11.03", "linux"))).unwrap())
        .into_iter()
        .map(|(x, _)| x)
        .filter(|x| x.starts_with("fictives/"))
        .collect();
    assert_eq!(noms, installation, "l'installation porte la meme livraison");

    // Sans aucune fenetre, rien n'est publie.
    let vide = d.join("sans-programme");
    std::fs::create_dir_all(vide.join("donnees")).unwrap();
    std::fs::write(vide.join("VERSION"), "2026.11.04\n").unwrap();
    let e = maj::publier_signe(&vide, &depot, &GRAINE, &publique(), "", "", "").unwrap_err();
    assert!(e.contains("aucune fenetre"), "{}", e);
    assert_eq!(maj::derniere(&depot.display().to_string()).unwrap()["version"], "2026.11.03");
}

/// L'editeur : sa version 2026.11.01, publiee dans `<base>/depot`. Rend le depot.
fn publiee(base: &Path, avec_edition: bool) -> PathBuf {
    let depot = base.join("depot");
    let editeur = logiciel(&base.join("editeur").join("logiciel"), "2026.11.01");
    livrer_les_fictives(&editeur);
    tous_les_systemes(&editeur);
    if avec_edition {
        std::fs::write(editeur.join("EDITION.json"), "{\"edition\": \"bride\"}\n").unwrap();
    }
    let r = maj::publier_signe(&editeur, &depot, &GRAINE, &publique(), "Essai de bout en bout", "", "").unwrap();
    assert_eq!(r["version"], "2026.11.01", "l'identifiant reste AAAA.MM.JJ dans derniere.json");
    for s in ["windows", "linux", "mac", "tout-systeme"] {
        assert!(depot.join(format!("puppetpaie-2026.11.01-{}.zip", s)).is_file(), "{}", s);
        assert_eq!(depot.join(maj::nom_d_installation("2026.11.01", s)).is_file(), s != "windows", "l'installation de {}", s);
    }
    assert!(depot.join("derniere.json").is_file());
    depot
}

/// Ce que l'installation a deballe de la version prete (`maj/<version>/logiciel/`).
fn deballee(inst: &Path, version: &str) -> PathBuf {
    inst.join("maj").join(version).join("logiciel")
}

/// Le logiciel deballe porte les programmes de CE systeme, executables, et ceux d'aucun autre (l'archive de son systeme).
fn seulement_ce_systeme(neuf: &Path) {
    for s in maj::SYSTEMES {
        assert_eq!(maj::programme_de(neuf, s, "puppetpaie").is_file(), s == maj::SYSTEME, "la fenetre de {}", s);
    }
    executables(neuf);
}

/// Les programmes de ce systeme sont executables apres le deballage, un fichier de donnees ne l'est pas.
fn executables(neuf: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o111;
        for p in ["puppetpaie", "paie"] {
            assert_eq!(mode(&maj::programme(neuf, p)), 0o111, "{} executable apres le deballage", p);
        }
        assert_eq!(mode(&neuf.join("VERSION")), 0, "un fichier de donnees ne l'est pas");
    }
}

/// L'installation de l'installeur : `logiciel/` en 2026.10.09, sans `EDITION.json` (l'installeur n'en pose plus : sans
/// rien qui dise « debloque », le logiciel est bride, ticket #246). Rend l'installation.
fn installee(base: &Path) -> PathBuf {
    let inst = base.join("installation");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    assert!(paie::bridage::est_bride_de(&inst), "une installation neuve est bridee");
    inst
}

/// Une compagnie telle que la cree « Ajouter une compagnie » (`interface::compagnies::ajouter`) : trois fiches.
fn ajoutee(racine: &Path) {
    crate::outils::ecrire(racine, "config/entreprise.json", &json!({"nom_affichage": "Essai", "raison_sociale": "ESSAI"}));
    crate::outils::ecrire(racine, "config/salaries.json", &json!({}));
    crate::outils::ecrire(racine, "config/spectacles.json", &json!({}));
}

/// L'installation bridee de l'installeur recoit la version publiee POUR ELLE-MEME (aucune compagnie ouverte : la
/// recherche ne prend que l'installation), la range a cote de `logiciel/`, l'applique en restant bridee, et
/// revient a la precedente. Rien ne se range dans une compagnie.
#[test]
fn une_installation_recoit_la_version_publiee_garde_son_bridage_et_revient() {
    let base = dossier_de_test("mise_a_jour", "bout_en_bout");
    let depot = publiee(&base, false);
    let inst = installee(&base);
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
    // L'archive de CE systeme (Linux, ici), pas celle de tous : rien d'un autre systeme ; et ses programmes executables.
    let neuf = deballee(&inst, "2026.11.01");
    assert_eq!(maj::prete(&inst).unwrap()["archive"], format!("puppetpaie-2026.11.01-{}.zip", maj::SYSTEME));
    seulement_ce_systeme(&neuf);
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"));

    // Appliquee : la neuve est en place, la precedente dans .avant/, et l'EDITION.json de l'installation est la.
    let r = maj::appliquer(&inst, false).unwrap();
    assert_eq!(r["version"], "2026.11.01");
    assert_eq!(maj::courante(&code), "2026.11.01");
    assert_eq!(lire(&code.join("interface").join("index.html")), "<p>2026.11.01</p>\n");
    assert!(!code.join("EDITION.json").exists() && paie::bridage::est_bride_de(&inst), "l'installation reste bridee");
    assert_eq!(maj::courante(&code.join(".avant")), "2026.10.09");
    assert!(maj::prete(&inst).is_none() && maj::applicable(&inst).is_none(), "plus rien a appliquer");
    let applique: Value = serde_json::from_str(&lire(&inst.join("maj").join("appliquee.json"))).unwrap();
    assert_eq!(applique["version"], "2026.11.01");
    assert_eq!(maj::verifier_avec(&inst, true, &[&publique()]), "version 01.11.2026, a jour");

    // La marche arriere : la precedente revient, et l'installation reste bridee.
    let r = maj::revenir(&inst).unwrap();
    assert_eq!(r["version"], "2026.10.09");
    assert_eq!(maj::courante(&code), "2026.10.09");
    assert!(!code.join("EDITION.json").exists() && paie::bridage::est_bride_de(&inst), "toujours bridee");
    assert!(!code.join(".avant").exists() && !code.join(".apres").exists());
}

/// Le `derniere.json` du depot, lu et reecrit tel quel (un depot compromis, ou une Release a qui il manque un fichier).
fn manifeste(depot: &Path) -> Value {
    serde_json::from_str(&lire(&depot.join("derniere.json"))).unwrap()
}

fn reecrire_manifeste(depot: &Path, m: &Value) {
    std::fs::write(depot.join("derniere.json"), serde_json::to_vec_pretty(m).unwrap()).unwrap();
}

/// Une installation de l'installeur, avec une compagnie, reglee sur `depot`, qui cherche : (installation, ce qui est dit).
fn recoit(base: &Path, depot: &Path) -> (PathBuf, String) {
    let inst = installee(base);
    ajoutee(&inst.join("compagnies").join("essai"));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    (inst, dit)
}

/// SANS L'ARCHIVE DE SON SYSTEME, L'INSTALLATION PREND CELLE DE TOUS : quand `derniere.json` ne la nomme pas, et quand il
/// la nomme mais qu'elle ne se telecharge pas. Celle de tous se controle, s'applique, et ses programmes sont executables.
/// Sans l'une ni l'autre, rien n'est prepare. L'ancien `derniere.json` (une seule `archive`, avant le 15/10/2026) est
/// refuse : pas de compatibilite, Leandre est seul a l'avoir recu.
#[test]
fn sans_l_archive_de_son_systeme_l_installation_prend_celle_de_tous() {
    let base = dossier_de_test("mise_a_jour", "repli_tout_systeme");
    let depot = publiee(&base, false);
    let complet = manifeste(&depot);
    let de_tous = format!("puppetpaie-2026.11.01-{}.zip", maj::TOUT_SYSTEME);

    // derniere.json ne nomme pas l'archive de ce systeme.
    let mut sans = complet.clone();
    sans["archives"].as_object_mut().unwrap().remove(maj::SYSTEME);
    reecrire_manifeste(&depot, &sans);
    let (inst, dit) = recoit(&base.join("non-nommee"), &depot);
    assert!(dit.contains("01.11.2026 est telechargee, controlee, prete") && dit.contains("(archive tout-systeme"), "{}", dit);
    assert_eq!(maj::prete(&inst).unwrap()["archive"], de_tous.as_str());
    let neuf = deballee(&inst, "2026.11.01");
    for s in maj::SYSTEMES {
        assert!(maj::programme_de(&neuf, s, "puppetpaie").is_file(), "celle de tous porte la fenetre de {}", s);
    }
    executables(&neuf);
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
    executables(&inst.join("logiciel"));

    // Il la nomme, mais elle n'est pas dans le depot.
    reecrire_manifeste(&depot, &complet);
    let propre = depot.join(format!("puppetpaie-2026.11.01-{}.zip", maj::SYSTEME));
    let gardee = std::fs::read(&propre).unwrap();
    std::fs::remove_file(&propre).unwrap();
    let (inst, dit) = recoit(&base.join("introuvable"), &depot);
    assert!(dit.contains("prete") && dit.contains("(archive tout-systeme"), "{}", dit);
    assert_eq!(maj::prete(&inst).unwrap()["archive"], de_tous.as_str());
    std::fs::write(&propre, gardee).unwrap();

    // Ni l'une ni l'autre : rien n'est telecharge, la version en place reste, et c'est dit.
    let mut aucune = complet.clone();
    aucune["archives"].as_object_mut().unwrap().retain(|k, _| k != maj::SYSTEME && k != maj::TOUT_SYSTEME);
    reecrire_manifeste(&depot, &aucune);
    let (inst, dit) = recoit(&base.join("aucune"), &depot);
    assert_eq!(dit, format!("version 09.10.2026 -- la 01.11.2026 est publiee sans ce systeme ({}) : cette installation garde sa version", maj::SYSTEME));
    assert!(maj::prete(&inst).is_none() && !deballee(&inst, "2026.11.01").exists());

    // L'ancien derniere.json : refuse.
    let t = &complet["archives"][maj::TOUT_SYSTEME];
    reecrire_manifeste(&depot, &json!({"version": "2026.11.01", "archive": t["archive"], "sha256": t["sha256"], "signature": t["signature"]}));
    let (inst, dit) = recoit(&base.join("ancien_format"), &depot);
    assert!(dit.contains("derniere.json sans « archives »"), "{}", dit);
    assert!(maj::prete(&inst).is_none());
}

/// « PUBLIER » A FABRIQUE SANS CE SYSTEME (Leandre, 15/10/2026 : on choisit Windows seul, Windows et Linux, ou les trois).
/// `derniere.json` ne nomme que les archives fabriquees, et celle de tous dit qu'elle ne porte pas ce systeme :
/// l'installation ne telecharge rien (pas meme celle de tous, ou son programme manque), garde sa version et le dit. Un
/// `derniere.json` dont celle de tous pretendrait le porter alors qu'il manque serait refuse par `controler`.
#[test]
fn une_version_publiee_sans_ce_systeme_ne_se_telecharge_pas() {
    let base = dossier_de_test("mise_a_jour", "sans_ce_systeme");
    let editeur = logiciel(&base.join("editeur").join("logiciel"), "2026.11.01");
    livrer_les_fictives(&editeur);
    std::fs::remove_dir_all(maj::dossier_programmes(&editeur)).unwrap();
    std::fs::write(editeur.join("puppetpaie.exe"), b"MZ fenetre").unwrap();
    std::fs::write(editeur.join("WebView2Loader.dll"), b"MZ chargeur").unwrap();
    let depot = base.join("depot");
    maj::publier_signe(&editeur, &depot, &GRAINE, &publique(), "Windows seul", "", "").unwrap();
    let m = manifeste(&depot);
    let mut cles: Vec<&str> = m["archives"].as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["tout-systeme", "windows"], "seules les archives fabriquees sont nommees");
    assert_eq!(m["archives"]["tout-systeme"]["systemes"], json!(["windows"]));
    assert!(!depot.join(format!("puppetpaie-2026.11.01-{}.zip", maj::SYSTEME)).exists());

    let (inst, dit) = recoit(&base, &depot);
    assert_eq!(dit, format!("version 09.10.2026 -- la 01.11.2026 est publiee sans ce systeme ({}) : cette installation garde sa version", maj::SYSTEME));
    assert!(maj::prete(&inst).is_none() && !deballee(&inst, "2026.11.01").exists(), "rien n'est telecharge");
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.10.09");

    // Celle de tous qui pretendrait porter ce systeme sans son programme : telechargee, mais refusee au controle.
    let mut menteur = m.clone();
    menteur["archives"]["tout-systeme"]["systemes"] = json!(["windows", maj::SYSTEME]);
    reecrire_manifeste(&depot, &menteur);
    let (inst, dit) = recoit(&base.join("pretendue"), &depot);
    assert!(dit.contains("n'a pas pu etre preparee") && dit.contains("le programme de ce systeme manque"), "{}", dit);
    assert!(maj::prete(&inst).is_none());
}

/// UNE ARCHIVE DONT LA SIGNATURE NE VAUT PAS EST REFUSEE, sans repli sur celle de tous les systemes : celle de son systeme
/// est la, elle n'est pas la notre. La signature d'une autre archive de la meme version (celle de tous) ne vaut pas pour
/// elle ; des octets changes ne passent pas le condensat ; un condensat refait sur ces octets ne passe pas la signature.
/// Rien n'est deballe.
#[test]
fn une_archive_dont_la_signature_ne_vaut_pas_est_refusee_sans_repli() {
    let base = dossier_de_test("mise_a_jour", "signature_refusee");
    let depot = publiee(&base, false);
    let complet = manifeste(&depot);
    let refusee = |inst: &Path, dit: &str, quoi: &str| {
        assert!(dit.contains("n'a pas pu etre preparee") && dit.contains(quoi), "{} : {}", quoi, dit);
        assert!(maj::prete(inst).is_none() && !deballee(inst, "2026.11.01").exists(), "rien n'est deballe : {}", dit);
    };

    let mut m = complet.clone();
    m["archives"][maj::SYSTEME]["signature"] = complet["archives"][maj::TOUT_SYSTEME]["signature"].clone();
    reecrire_manifeste(&depot, &m);
    let (inst, dit) = recoit(&base.join("signature_d_une_autre"), &depot);
    refusee(&inst, &dit, "signature");

    reecrire_manifeste(&depot, &complet);
    let propre = depot.join(format!("puppetpaie-2026.11.01-{}.zip", maj::SYSTEME));
    let mut o = std::fs::read(&propre).unwrap();
    let n = o.len() / 2;
    o[n] ^= 1;
    std::fs::write(&propre, &o).unwrap();
    let (inst, dit) = recoit(&base.join("octets_changes"), &depot);
    refusee(&inst, &dit, "alteree");

    let mut m = complet.clone();
    m["archives"][maj::SYSTEME]["sha256"] = json!(condensat(&o));
    reecrire_manifeste(&depot, &m);
    let (inst, dit) = recoit(&base.join("condensat_refait"), &depot);
    refusee(&inst, &dit, "signature");
}

/// Une installation DEBLOQUEE (le geste de l'accueil a ecrit `.edition.json` a cote de `logiciel/`)
/// reste debloquee : la mise a jour ne touche pas ce choix, meme si le logiciel de
/// l'editeur porte son propre `EDITION.json` (il ne part pas dans l'archive). Leandre, 14/10/2026 : « le petit curseur
/// ecrit en JSON lui il reste comme il etait. Si les gens ont debloque, elle reste debloquee ».
#[test]
fn une_installation_debloquee_reste_debloquee() {
    let base = dossier_de_test("mise_a_jour", "installation_debloquee");
    let depot = publiee(&base, true);
    let inst = installee(&base);
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
    assert!(!code.join("EDITION.json").exists(), "l'EDITION.json de l'editeur n'est pas arrive");
    assert!(!paie::bridage::est_bride_de(&inst), "l'installation reste debloquee");
}

/// L'ARCHIVE D'INSTALLATION, DECOMPRESSEE DANS UN DOSSIER NEUF, EST UNE INSTALLATION (ticket #246) : `logiciel/` et, a
/// cote, les quatre fictives livrees ; celle de ce systeme comme celle de tous (le dossier portable). Rien ne dit
/// « debloque » : elle est bridee, et l'accueil propose ses quatre fictives, aucune compagnie reelle. Le programme de ce
/// systeme y est, executable.
#[test]
fn une_archive_decompressee_dans_un_dossier_neuf_est_une_installation_bridee_avec_ses_fictives() {
    let base = dossier_de_test("mise_a_jour", "archive_decompressee");
    let depot = publiee(&base, false);
    for systeme in [maj::SYSTEME, maj::TOUT_SYSTEME] {
        let octets = std::fs::read(depot.join(maj::nom_d_installation("2026.11.01", systeme))).unwrap();
        let neuf = base.join(format!("dossier-neuf-{}", systeme));
        zip::ZipArchive::new(std::io::Cursor::new(octets)).unwrap().extract(&neuf).unwrap();
        let mut a_la_racine: Vec<String> = std::fs::read_dir(&neuf).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        a_la_racine.sort();
        assert_eq!(a_la_racine, ["fictives", "logiciel"], "{} : une installation, rien d'autre", systeme);
        assert_eq!(maj::courante(&neuf.join("logiciel")), "2026.11.01");
        executables(&neuf.join("logiciel"));
        assert!(paie::bridage::est_bride_de(&neuf), "rien ne dit « debloque » : bridee");
        let l = paie::interface::compagnies::liste_de(&neuf);
        assert_eq!(l.iter().filter(|c| c.reelle).count(), 0, "aucune compagnie reelle");
        let mut ids: Vec<String> = l.iter().map(|c| c.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, FICTIVES.map(|n| format!("fictive:{}", n)), "les quatre fictives");
        assert!(l.iter().all(|c| paie::interface::compagnies::est_fictive(&c.racine)));
    }
}

/// LE DEBALLAGE DU CODE D'AVANT, recopie tel quel de `paie/src/maj.rs` a 8fc646b (avant le ticket #246) : ce que fait
/// chaque installation deja posee de l'archive que `derniere.json` nomme, dans `maj/<version>/logiciel/`. On ne change
/// pas le code d'une installation posee : ceci ne doit JAMAIS suivre `maj::deballer`.
fn deballer_comme_le_code_d_avant(octets: &[u8], cible: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| format!("archive illisible : {}", e))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { return Err(format!("archive suspecte : {}", f.name())) };
        let chemin = cible.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&chemin).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let executable = f.unix_mode().is_some_and(|m| m & 0o111 != 0);
        let mut o = Vec::new();
        f.read_to_end(&mut o).map_err(|e| e.to_string())?;
        std::fs::write(&chemin, o).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("{} : {}", chemin.display(), e))?;
        }
        #[cfg(not(unix))]
        let _ = executable;
    }
    Ok(())
}

/// UNE INSTALLATION AU CODE D'AVANT SE MET A JOUR (le contrat de la tete de `maj.rs`) : ce que publie le code d'aujourd'hui,
/// une installation posee avant le ticket #246 le lit et l'applique avec SON code. On rejoue ici `telecharger` de 8fc646b,
/// pas a pas : `derniere.json` a la forme qu'il exige (`version`, `archives` : `archive`, `sha256`, `signature`, et
/// `systemes` pour la tout-systeme), l'archive de son systeme a le condensat et la signature annonces, et deballee
/// par son code dans `maj/<version>/logiciel/`, elle dit la version annoncee et porte le programme de ce systeme,
/// executable. Une archive d'installation, elle, deballee par ce meme code, dirait « 0.0.0 » : c'est ce qui coupait les
/// installations posees le 22/10/2026 (7ea89f6), et c'est pourquoi `derniere.json` ne la nomme jamais.
#[test]
fn une_installation_au_code_d_avant_deballe_et_lit_l_archive_de_mise_a_jour() {
    let base = dossier_de_test("mise_a_jour", "code_d_avant");
    let depot = publiee(&base, false);
    let d: Value = serde_json::from_str(&lire(&depot.join("derniere.json"))).unwrap();
    let version = d["version"].as_str().unwrap();
    let archives = d["archives"].as_object().unwrap();
    for (s, a) in archives {
        assert!(maj::SYSTEMES.contains(&s.as_str()) || s == maj::TOUT_SYSTEME, "{}", s);
        for k in ["archive", "sha256", "signature"] {
            assert!(a[k].as_str().is_some_and(|x| !x.is_empty()), "{} : {}", s, k);
        }
    }
    assert!(archives[maj::TOUT_SYSTEME]["systemes"].as_array().is_some_and(|l| !l.is_empty()));
    // Les fictives sont nommees a la racine, sous une cle que le code d'avant ne lit pas (il ne lit que `version` et
    // `archives`, et refuserait sous `archives` ce qui n'est pas un systeme).
    assert!(d["fictives"]["archive"].is_string() && !archives.contains_key("fictives"));
    // Et le code d'avant le 15/10/2026 (6a5c83f^ : une seule archive) lit a la racine `archive`, `sha256`, `signature` :
    // l'archive de Windows, la seule qu'il connaissait.
    for k in ["archive", "sha256", "signature"] {
        assert_eq!(d[k], archives["windows"][k], "{}", k);
    }
    for systeme in [maj::SYSTEME, maj::TOUT_SYSTEME] {
        let a = &archives[systeme];
        let octets = std::fs::read(depot.join(a["archive"].as_str().unwrap())).unwrap();
        assert_eq!(a["sha256"], condensat(&octets).as_str());
        assert!(maj::signee_par_l_une(&octets, a["signature"].as_str().unwrap(), &[&publique()]));
        let neuf = base.join("installation-d-avant").join("maj").join(version).join(format!("logiciel-{}", systeme));
        deballer_comme_le_code_d_avant(&octets, &neuf).unwrap();
        assert_eq!(maj::courante(&neuf), version, "{} : le code d'avant lit la version annoncee", systeme);
        assert!(maj::programme(&neuf, "puppetpaie").is_file(), "{} : le programme de ce systeme", systeme);
        executables(&neuf);
        assert!(!neuf.join("fictives").exists() && !neuf.join("logiciel").exists(), "{} : rien d'autre que le logiciel", systeme);
    }
    // Ce qui arrivait avec 7ea89f6 : l'archive d'installation, prise pour une mise a jour par le code d'avant.
    let installation = std::fs::read(depot.join(maj::nom_d_installation(version, maj::SYSTEME))).unwrap();
    let neuf = base.join("installation-d-avant").join("maj").join("installation").join("logiciel");
    deballer_comme_le_code_d_avant(&installation, &neuf).unwrap();
    assert_eq!(maj::courante(&neuf), "0.0.0", "le code d'avant ne lirait pas une archive d'installation");
}

/// LA MISE A JOUR DU LOGICIEL SEUL NE TOUCHE PAS LES FICTIVES DU POSTE : la livraison de fictives publiee est celle que
/// le poste retient (`fictives/.livraison.json`) ; rien n'en est telecharge, et une fictive jouee (sa fiche changee, une
/// paie de plus) et sa photo d'origine (`fictives/.origine/`) restent exactement comme elles etaient. L'archive de mise a
/// jour n'en porte aucune.
#[test]
fn la_mise_a_jour_en_place_n_ecrase_pas_les_fictives_du_poste() {
    let base = dossier_de_test("mise_a_jour", "fictives_du_poste");
    let depot = publiee(&base, false);
    let inst = installee(&base);
    let fictive = inst.join("fictives").join("fictive-3090-petite");
    compagnie::config_et_mois("fictive-3090-petite", &[], &fictive);
    std::fs::copy(crate::outils::fictive("fictive-3090-petite").join("COPIE-DE-DEVELOPPEMENT.json"), fictive.join("COPIE-DE-DEVELOPPEMENT.json")).unwrap();
    paie::interface::compagnies::figer_origine(&fictive).unwrap();
    let origine = inst.join("fictives").join(".origine").join("fictive-3090-petite");
    assert!(origine.join("config").join("entreprise.json").is_file(), "la photo d'origine");
    crate::outils::ecrire(&fictive, "config/entreprise.json", &json!({"nom_affichage": "Jouee sur ce poste"}));
    crate::outils::poser_saisie(&fictive, "2026-12", &json!({"joue": true}));
    let fiche = lire(&fictive.join("config").join("entreprise.json"));
    let photo = lire(&origine.join("config").join("entreprise.json"));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    // Le poste a deja la livraison de fictives publiee : seule la version du logiciel est neuve.
    let publiees: Value = serde_json::from_str(&lire(&depot.join("derniere.json"))).unwrap();
    crate::outils::ecrire(&inst, "fictives/.livraison.json", &json!({"empreinte": publiees["fictives"]["empreinte"], "posees": {}}));

    let dit = maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(dit.contains("prete"), "{}", dit);
    assert!(!inst.join("maj").join("2026.11.01").join("fictives").exists(), "rien des fictives de l'archive n'est range");
    assert!(!inst.join("maj").join("fictives-pretes").exists(), "la livraison de fictives est celle du poste : rien n'est telecharge");
    maj::appliquer(&inst, false).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
    assert_eq!(lire(&fictive.join("config").join("entreprise.json")), fiche, "la fiche jouee est intacte");
    assert!(fictive.join("paies").join("2026-12.json").is_file(), "la paie jouee aussi");
    assert_eq!(lire(&origine.join("config").join("entreprise.json")), photo, "la photo d'origine aussi");
    let mut fictives: Vec<String> = std::fs::read_dir(inst.join("fictives")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    fictives.sort();
    assert_eq!(fictives, [".livraison.json", ".origine", "fictive-3090-petite"], "aucune fictive de l'archive n'est arrivee");
    assert!(maj::poser_les_fictives_pretes(&inst).is_none(), "au lancement, rien a poser");
}

/// Tout ce que porte un dossier, chemin par chemin : pour dire qu'il est intact.
fn photo_de(d: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut pile = vec![d.to_path_buf()];
    while let Some(p) = pile.pop() {
        for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                pile.push(e.path());
            } else {
                out.insert(e.path().strip_prefix(d).unwrap().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap());
            }
        }
    }
    out
}

/// L'editeur publie la version `version` dans `depot`, ses fictives livrees puis changees par `changer` (le dossier
/// `fictives/` de l'editeur). Rend `derniere.json`.
fn publier_avec_fictives(base: &Path, depot: &Path, version: &str, changer: impl Fn(&Path)) -> Value {
    let editeur = logiciel(&base.join(format!("editeur-{}", version)).join("logiciel"), version);
    livrer_les_fictives(&editeur);
    changer(&editeur.parent().unwrap().join("fictives"));
    tous_les_systemes(&editeur);
    maj::publier_signe(&editeur, depot, &GRAINE, &publique(), "", "", "").unwrap();
    serde_json::from_str(&lire(&depot.join("derniere.json"))).unwrap()
}

/// Le marqueur d'une fictive livree, au nom `nom`.
fn marquer(dossier: &Path, nom: &str) {
    crate::outils::ecrire(dossier, "COPIE-DE-DEVELOPPEMENT.json", &json!({"copie_de": format!("compagnie fictive {}", nom)}));
}

/// LES FICTIVES VOYAGENT A PART, ET UNE LIVRAISON NE TOUCHE QUE CE QU'ELLE A POSE (Leandre, 22/10/2026). Une installation
/// posee par l'archive d'installation retient sa livraison : une Release aux memes fictives ne telecharge rien. Une
/// livraison changee se prepare en fond sans rien toucher, puis, au lancement, REMPLACE les fictives qu'elle avait posees,
/// jouees ou non, photo d'origine comprise (la suivante se prend sur la fictive neuve), retire celle qu'elle ne porte
/// plus, pose la nouvelle. Une copie renommee sur le poste, et un dossier fait sur le poste au nom d'une fictive livree,
/// restent intacts (la livraison ne pose pas celle-ci, et le dit) ; ni `compagnies/` ni `prive/` ne bougent. Une Release
/// suivante aux memes fictives ne retelecharge rien et ne touche rien.
#[test]
fn une_livraison_de_fictives_changee_remplace_celles_qu_elle_a_posees_et_rien_d_autre() {
    let base = dossier_de_test("mise_a_jour", "livraison_de_fictives");
    let depot = base.join("depot");
    let v1 = publier_avec_fictives(&base, &depot, "2026.11.01", |_| {});

    // L'installation, telle que la pose l'archive d'installation ; une compagnie reelle a cote.
    let inst = installee(&base);
    let deballee = base.join("archive-d-installation");
    let octets = std::fs::read(depot.join(maj::nom_d_installation("2026.11.01", maj::SYSTEME))).unwrap();
    zip::ZipArchive::new(std::io::Cursor::new(octets)).unwrap().extract(&deballee).unwrap();
    std::fs::rename(deballee.join("fictives"), inst.join("fictives")).unwrap();
    assert_eq!(maj::empreinte_retenue(&inst.join("fictives")).as_deref(), v1["fictives"]["empreinte"].as_str(), "elle retient sa livraison");
    ajoutee(&inst.join("compagnies").join("essai"));
    let compagnies = photo_de(&inst.join("compagnies"));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    let f = inst.join("fictives");

    // Les memes fictives que la Release : rien a telecharger.
    maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(!inst.join("maj").join("fictives-pretes").exists(), "les memes fictives ne se telechargent pas");

    // Le poste joue une fictive livree, en copie une sous un autre nom, et fait la sienne au nom d'une fictive a venir.
    let jouee = f.join("fictive-3090-petite");
    paie::interface::compagnies::figer_origine(&jouee).unwrap();
    crate::outils::ecrire(&jouee, "config/entreprise.json", &json!({"nom_affichage": "Jouee sur ce poste"}));
    crate::outils::poser_saisie(&jouee, "2026-12", &json!({"joue": true}));
    let copie = f.join("ma-partie");
    copier(&jouee, &copie);
    paie::interface::compagnies::figer_origine(&copie).unwrap();
    let a_moi = f.join("fictive-nouvelle");
    copier(&f.join("fictive-1285-petite"), &a_moi);
    crate::outils::ecrire(&a_moi, "config/entreprise.json", &json!({"nom_affichage": "Faite sur ce poste"}));
    let (photo_copie, photo_origine_copie, photo_a_moi) = (photo_de(&copie), photo_de(&f.join(".origine").join("ma-partie")), photo_de(&a_moi));

    // La livraison suivante : une fictive changee, une retiree, une nouvelle (au nom du dossier fait sur le poste).
    let v2 = publier_avec_fictives(&base, &depot, "2026.11.02", |l| {
        crate::outils::ecrire(&l.join("fictive-3090-petite"), "config/entreprise.json", &json!({"nom_affichage": "Nouvelle histoire"}));
        std::fs::remove_dir_all(l.join("fictive-1285-grande")).unwrap();
        copier(&l.join("fictive-3090-grande"), &l.join("fictive-nouvelle"));
        marquer(&l.join("fictive-nouvelle"), "fictive-nouvelle");
    });
    assert_ne!(v2["fictives"]["empreinte"], v1["fictives"]["empreinte"]);
    maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(inst.join("maj").join("fictives-pretes").is_dir(), "la livraison changee se prepare");
    assert!(lire(&jouee.join("config").join("entreprise.json")).contains("Jouee"), "rien n'est pose avant le lancement");

    // Au lancement : posee.
    let dit = maj::poser_les_fictives_pretes(&inst).unwrap();
    assert!(dit.contains("fictive-nouvelle non posee"), "{}", dit);
    assert!(lire(&jouee.join("config").join("entreprise.json")).contains("Nouvelle histoire"), "la fictive jouee est remplacee");
    assert!(!jouee.join("paies").join("2026-12.json").exists(), "ce qui y etait joue est parti");
    assert!(!f.join(".origine").join("fictive-3090-petite").exists(), "sa photo d'origine aussi");
    paie::interface::compagnies::figer_origine(&jouee).unwrap();
    assert!(lire(&f.join(".origine/fictive-3090-petite/config/entreprise.json")).contains("Nouvelle histoire"), "la suivante est la neuve");
    assert!(!f.join("fictive-1285-grande").exists(), "la fictive retiree de la livraison disparait");
    assert!(f.join("fictive-1285-petite").join("config").join("entreprise.json").is_file(), "celles qui n'ont pas change restent");
    assert_eq!(photo_de(&copie), photo_copie, "la copie renommee est intacte");
    assert_eq!(photo_de(&f.join(".origine").join("ma-partie")), photo_origine_copie, "et sa photo d'origine");
    assert_eq!(photo_de(&a_moi), photo_a_moi, "le dossier fait sur le poste, au nom d'une fictive livree, est intact");
    assert_eq!(photo_de(&inst.join("compagnies")), compagnies, "rien hors de fictives/");
    assert!(!inst.join("prive").exists());
    assert!(!inst.join("maj").join("fictives-pretes").exists() && !inst.join("maj").join("fictives-avant").exists());
    let retenue: Value = serde_json::from_str(&lire(&f.join(".livraison.json"))).unwrap();
    assert_eq!(retenue["empreinte"], v2["fictives"]["empreinte"]);
    let mut posees: Vec<&str> = retenue["posees"].as_object().unwrap().keys().map(String::as_str).collect();
    posees.sort();
    assert_eq!(posees, ["fictive-1285-petite", "fictive-3090-grande", "fictive-3090-petite"], "seulement ce qu'elle a pose");

    // Une Release aux memes fictives : rien n'est telecharge (son archive des fictives n'est meme pas lue), rien n'est touche.
    let v3 = publier_avec_fictives(&base, &depot, "2026.11.03", |l| {
        crate::outils::ecrire(&l.join("fictive-3090-petite"), "config/entreprise.json", &json!({"nom_affichage": "Nouvelle histoire"}));
        std::fs::remove_dir_all(l.join("fictive-1285-grande")).unwrap();
        copier(&l.join("fictive-3090-grande"), &l.join("fictive-nouvelle"));
        marquer(&l.join("fictive-nouvelle"), "fictive-nouvelle");
    });
    assert_eq!(v3["fictives"]["empreinte"], v2["fictives"]["empreinte"], "memes fictives, meme empreinte");
    std::fs::remove_file(depot.join(maj::nom_des_fictives("2026.11.03"))).unwrap();
    crate::outils::ecrire(&jouee, "config/entreprise.json", &json!({"nom_affichage": "Rejouee"}));
    let avant = photo_de(&f);
    maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(!inst.join("maj").join("fictives-pretes").exists(), "rien n'est retelecharge");
    assert!(maj::poser_les_fictives_pretes(&inst).is_none());
    assert_eq!(photo_de(&f), avant, "rien n'est touche");
}

/// UNE INSTALLATION D'AVANT CE MECANISME (rien de retenu : l'installeur Windows, ou une installation posee avant le
/// 22/10/2026) : la premiere livraison ne remplace que les dossiers au nom d'une fictive livree QUI PORTENT LE MARQUEUR
/// D'UNE FICTIVE LIVREE (`COPIE-DE-DEVELOPPEMENT.json` : « compagnie fictive <ce nom> »). Un dossier fait sur le poste au
/// nom d'une fictive livree, sans ce marqueur, survit intact, et la fictive livree n'y est pas posee.
#[test]
fn sans_livraison_retenue_seules_les_fictives_marquees_livrees_sont_remplacees() {
    let base = dossier_de_test("mise_a_jour", "livraison_sans_retenue");
    let depot = base.join("depot");
    publier_avec_fictives(&base, &depot, "2026.11.01", |_| {});
    let inst = installee(&base);
    let f = inst.join("fictives");
    let livree = f.join("fictive-3090-petite");
    copier(&crate::outils::fictive("fictive-3090-petite"), &livree);
    crate::outils::ecrire(&livree, "config/entreprise.json", &json!({"nom_affichage": "Jouee sur ce poste"}));
    let a_moi = f.join("fictive-3090-grande");
    copier(&crate::outils::fictive("fictive-1285-petite"), &a_moi);
    let photo_a_moi = photo_de(&a_moi);
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));

    maj::verifier_avec(&inst, true, &[&publique()]);
    let dit = maj::poser_les_fictives_pretes(&inst).unwrap();
    assert!(dit.contains("fictive-3090-grande non posee"), "{}", dit);
    assert!(!lire(&livree.join("config").join("entreprise.json")).contains("Jouee"), "la fictive livree est remplacee");
    assert!(livree.join(".livree.json").is_file(), "et marquee");
    assert_eq!(photo_de(&a_moi), photo_a_moi, "le dossier fait sur le poste est intact");
    for nom in ["fictive-1285-grande", "fictive-1285-petite"] {
        assert!(f.join(nom).join("config").join("entreprise.json").is_file(), "{} est posee", nom);
    }
}

/// UNE FICTIVE AJOUTEE (ticket #236 : le « + » d'un logiciel bride, `compagnies::ajouter_dans`) n'est jamais a la
/// livraison : une livraison posee sur le poste ne la remplace ni ne la retire, meme quand l'editeur livre une fictive du
/// meme nom ; et celle que le poste de l'editeur aurait ajoutee ne part dans aucune archive des fictives.
#[test]
fn la_livraison_des_fictives_ne_touche_jamais_une_fictive_ajoutee() {
    let base = dossier_de_test("mise_a_jour", "livraison_et_ajoutee");
    let depot = base.join("depot");
    let v1 = publier_avec_fictives(&base, &depot, "2026.11.01", |f| {
        // Le poste de l'editeur a ajoute une fictive : elle ne se livre pas.
        let c = paie::interface::compagnies::ajouter_dans(f.parent().unwrap(), "Chez l'editeur").unwrap();
        assert!(c.racine.starts_with(f));
    });
    assert!(!v1.to_string().contains("ajoutee-"), "{}", v1);
    let inst = installee(&base);
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    maj::verifier_avec(&inst, true, &[&publique()]);
    maj::poser_les_fictives_pretes(&inst).unwrap();
    let f = inst.join("fictives");
    assert!(!f.join("ajoutee-chez-l-editeur").exists(), "rien de l'editeur n'arrive");
    let c = paie::interface::compagnies::ajouter_dans(&inst, "Fictive 3090 petite").unwrap();
    crate::outils::poser_saisie(&c.racine, "2026-12", &json!({"saisi": "sur ce poste"}));
    let photo = photo_de(&c.racine);

    // La livraison suivante change tout, et livre une fictive au nom que porterait l'ajoutee sans son prefixe.
    publier_avec_fictives(&base, &depot, "2026.11.02", |l| {
        crate::outils::ecrire(&l.join("fictive-3090-petite"), "config/entreprise.json", &json!({"nom_affichage": "Nouvelle histoire"}));
        std::fs::remove_dir_all(l.join("fictive-1285-grande")).unwrap();
    });
    maj::verifier_avec(&inst, true, &[&publique()]);
    let dit = maj::poser_les_fictives_pretes(&inst).unwrap();
    assert!(!dit.contains("non posee"), "le nom de l'ajoutee ne prend celui d'aucune livree : {}", dit);
    assert!(lire(&f.join("fictive-3090-petite/config/entreprise.json")).contains("Nouvelle histoire"));
    assert_eq!(photo_de(&c.racine), photo, "la fictive ajoutee est intacte");
    assert!(!c.racine.join(".livree.json").exists());
    let retenue: Value = serde_json::from_str(&lire(&f.join(".livraison.json"))).unwrap();
    assert!(retenue["posees"].get(c.racine.file_name().unwrap().to_str().unwrap()).is_none(), "{}", retenue);
}

/// L'INSTALLEUR WINDOWS RETIENT SA LIVRAISON : le flux « Publier » marque les fictives livrees (`paie maj
/// --marquer-fictives`) avant de fabriquer l'installeur, qui pose ces marques avec elles. Une installation ainsi posee ne
/// retelecharge pas des fictives qui n'ont pas change. Les marques ne changent ni l'empreinte ni ce qui se livre.
#[test]
fn les_fictives_marquees_par_le_flux_ne_se_retelechargent_pas() {
    let base = dossier_de_test("mise_a_jour", "fictives_marquees");
    let depot = base.join("depot");
    let v1 = publier_avec_fictives(&base, &depot, "2026.11.01", |_| {});
    let editeur = base.join("editeur-2026.11.01").join("logiciel");
    let s = std::process::Command::new(env!("CARGO_BIN_EXE_paie")).args(["maj", "--marquer-fictives", "--logiciel", editeur.to_str().unwrap()]).output().unwrap();
    assert!(s.status.success(), "{}", String::from_utf8_lossy(&s.stdout));
    let fictives = editeur.parent().unwrap().join("fictives");
    assert_eq!(maj::empreinte_retenue(&fictives).as_deref(), v1["fictives"]["empreinte"].as_str());
    assert_eq!(maj::archiver_fictives(&editeur).unwrap().1, v1["fictives"]["empreinte"].as_str().unwrap(), "les marques ne se livrent pas");

    // Ce que pose l'installeur : les fichiers des fictives et leurs marques.
    let inst = installee(&base);
    copier(&fictives, &inst.join("fictives"));
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    maj::verifier_avec(&inst, true, &[&publique()]);
    assert!(!inst.join("maj").join("fictives-pretes").exists(), "rien n'est telecharge");
}

/// Une installation BRIDEE dont les seules compagnies sont les fictives (et une copie de developpement) recoit et applique
/// comme les autres, et reste bridee : le bridage ne joue aucun role dans la mise a jour. Le programme neuf est controle
/// sur la fictive ; la copie de developpement n'est pas une compagnie de l'installation.
#[test]
fn une_installation_bridee_recoit_applique_et_reste_bridee() {
    let base = dossier_de_test("mise_a_jour", "installation_bridee");
    let depot = publiee(&base, false);
    let inst = installee(&base);
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
    assert!(!inst.join("logiciel").join("EDITION.json").exists() && paie::bridage::est_bride_de(&inst), "l'installation reste bridee");
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
    crate::outils::poser_saisie(&fictive, "2030-01", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"), "une fictive ne retient pas la version");
    // Une compagnie reelle avec le meme mois : elle la retient.
    let reelle = inst.join("compagnies").join("reelle");
    compagnie::config_et_mois("fictive-3090-petite", &[], &reelle);
    crate::outils::poser_saisie(&reelle, "2030-01", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    assert!(maj::applicable(&inst).is_none(), "le mois 2030-01 de la compagnie reelle attend sa DSN");
    let refus = maj::appliquer(&inst, false).unwrap_err();
    assert!(refus.contains("2030-01 (reelle)") && refus.contains("--malgre-calcul"), "{}", refus);
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.10.09");
    maj::appliquer(&inst, true).unwrap();
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.11.01");
}

/// LES MOIS SAISIS EN DEMO d'une compagnie gardee au deblocage (ticket #236) ne retiennent pas une version qui change un
/// calcul : le logiciel ne les a jamais declares. Un mois saisi apres qu'elle a ete gardee retient comme toute reelle.
#[test]
fn les_mois_faits_en_demo_d_une_compagnie_gardee_ne_retiennent_pas_la_version() {
    let base = dossier_de_test("mise_a_jour", "garde_fou_demo");
    let inst = base.join("installation");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    logiciel(&inst.join("maj").join("2026.11.01").join("logiciel"), "2026.11.01");
    crate::outils::ecrire(&inst, "maj/prete.json", &json!({"version": "2026.11.01", "calcul": "la CSG"}));
    // Bride : le « + » ajoute une fictive ; on y saisit un mois, jamais declare.
    let ajoutee = paie::interface::compagnies::ajouter_dans(&inst, "Les Pupazzi").unwrap();
    compagnie::config_et_mois("fictive-3090-petite", &[], &ajoutee.racine);
    let mut e = crate::outils::lire(&ajoutee.racine, "config/entreprise.json");
    e["nom_affichage"] = json!("Les Pupazzi");
    crate::outils::ecrire(&ajoutee.racine, "config/entreprise.json", &e);
    crate::outils::poser_saisie(&ajoutee.racine, "2030-01", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    // Debloque, on la garde : une compagnie reelle.
    crate::outils::debloquer(&inst);
    let poste = paie::administrateur::Poste { dossier_cle: base.join("cle"), maintenant: "2026-10-23T10:00".into() };
    let v = paie::interface::suppression::repondre(&inst, std::slice::from_ref(&ajoutee.id), &[], &poste, &|_| {}).unwrap();
    assert_eq!(v["erreurs"], json!([]), "{}", v);
    let gardee = inst.join("compagnies").join("les-pupazzi");
    assert!(gardee.join("paies/2030-01.json").is_file());
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"), "le mois fait en demo ne retient rien");
    // Un mois saisi apres : il attend sa DSN.
    crate::outils::poser_saisie(&gardee, "2030-02", &json!({"mois": "2030-02", "engagements": [{"salarie": "INVENTE"}]}));
    assert!(maj::applicable(&inst).is_none(), "le mois 2030-02, saisi apres, attend sa DSN");
    let refus = maj::appliquer(&inst, false).unwrap_err();
    assert!(refus.contains("2030-02 (les-pupazzi)") && !refus.contains("2030-01"), "{}", refus);
}

/// UN MOIS DE DEMO RETRAVAILLE apres que la compagnie a ete gardee (sa saisie enregistree) est un mois qu'on entend
/// declarer : il sort de la mention de demo et retient la version comme tout mois reel.
#[test]
fn un_mois_de_demo_retravaille_apres_avoir_ete_garde_retient_la_version() {
    let base = dossier_de_test("mise_a_jour", "garde_fou_demo_retravaille");
    let inst = base.join("installation");
    logiciel(&inst.join("logiciel"), "2026.10.09");
    logiciel(&inst.join("maj").join("2026.11.01").join("logiciel"), "2026.11.01");
    crate::outils::ecrire(&inst, "maj/prete.json", &json!({"version": "2026.11.01", "calcul": "la CSG"}));
    let ajoutee = paie::interface::compagnies::ajouter_dans(&inst, "Les Pupazzi").unwrap();
    compagnie::config_et_mois("fictive-3090-petite", &["2026-10"], &ajoutee.racine);
    // En demo, rien n'a ete ecrit au journal des depots.
    std::fs::remove_dir_all(ajoutee.racine.join("journal")).unwrap();
    let mut e = crate::outils::lire(&ajoutee.racine, "config/entreprise.json");
    e["nom_affichage"] = json!("Les Pupazzi");
    crate::outils::ecrire(&ajoutee.racine, "config/entreprise.json", &e);
    crate::outils::debloquer(&inst);
    let poste = paie::administrateur::Poste { dossier_cle: base.join("cle"), maintenant: "2026-10-23T10:00".into() };
    paie::interface::suppression::repondre(&inst, std::slice::from_ref(&ajoutee.id), &[], &poste, &|_| {}).unwrap();
    let gardee = inst.join("compagnies").join("les-pupazzi");
    assert_eq!(paie::interface::suppression::mois_de_demo(&gardee), vec!["2026-10".to_string()]);
    assert_eq!(maj::applicable(&inst).as_deref(), Some("2026.11.01"), "fait en demo, il ne retient rien");
    // On le retravaille : sa saisie s'enregistre.
    let saisie = crate::outils::saisie(&gardee, "2026-10");
    crate::outils::reussi(&crate::outils::route(&gardee, "enregistrer-saisie", json!({"mois": "2026-10", "saisie": saisie}), "2026-10-22"));
    assert!(paie::interface::suppression::mois_de_demo(&gardee).is_empty(), "il n'est plus de demo");
    assert!(maj::applicable(&inst).is_none(), "il attend sa DSN comme tout mois reel");
    assert!(maj::appliquer(&inst, false).unwrap_err().contains("2026-10 (les-pupazzi)"));
}

// ------------------------------------------------------------------ la publication par GitHub (lot P1, 14/10/2026)

fn publique_de(graine: &[u8; 32]) -> String {
    paie::retours::base64_ecrire(ed25519_dalek::SigningKey::from_bytes(graine).verifying_key().as_bytes())
}

/// Une Release : la version de `logiciel/` signee par `graine` (comme le flux de GitHub, par la graine en base64), dans
/// `<base>/releases/<etiquette>/`. Rend le dossier.
fn release(base: &Path, version: &str, graine: &[u8; 32]) -> PathBuf {
    let editeur = logiciel(&base.join(format!("editeur-{}", version)).join("logiciel"), version);
    livrer_les_fictives(&editeur);
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
    let inst = installee(&base);
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
    let nom = format!("puppetpaie-2026.11.02-{}.zip", maj::SYSTEME);
    assert!(chemins.contains(&format!("/releases/latest/download/{}", nom)), "l'archive de ce systeme : {:?}", chemins);
    assert!(chemins.contains(&format!("/stockage/2026.11.02/{}", nom)), "les redirections sont suivies : {:?}", chemins);
    assert!(!chemins.iter().any(|c| c.contains("tout-systeme")), "celle de tous les systemes n'est pas telechargee : {:?}", chemins);
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
    livrer_les_fictives(&editeur);
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

    // La derniere Release est d'avant les quatre archives (un derniere.json a une seule `archive`) : sa version se lit
    // quand meme, pour refuser la meme et accepter une plus recente.
    let ancienne = base.join("sortie-ancienne");
    crate::outils::ecrire(&ancienne, "derniere.json", &json!({"version": "2026.11.01", "archive": "puppetpaie-2026.11.01.zip", "sha256": "0", "signature": "0"}));
    let e = maj::publier_par_graine(&editeur, &ancienne, &g64, &publique_de(&graine), "", "", "").unwrap_err();
    assert!(e.contains("deja en 01.11.2026"), "{}", e);
    std::fs::write(editeur.join("VERSION"), "2026.11.02\n").unwrap();
    let r = maj::publier_par_graine(&editeur, &ancienne, &g64, &publique_de(&graine), "", "", "").unwrap();
    assert_eq!(r["version"], "2026.11.02");
    assert!(r["archives"].get(maj::TOUT_SYSTEME).is_some(), "{}", r);
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
    livrer_les_fictives(&editeur);
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
    let inst = installee(&base);
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

// ------------------------------------------------------------------ l'indicateur de la recherche (ticket #210, 16/10/2026)

/// LA RECHERCHE DIT OU ELLE EN EST, ET SE TAIT QUAND ELLE A FINI (ticket #210 : « Recherche de mise a jour / Aucune mise
/// a jour / Mise a jour en cours / Redemarrer pour mettre a jour »). Ce que l'accueil et la page d'une compagnie montrent
/// est `Recherche::etat` : sans recherche, rien a montrer ; une recherche finie n'est jamais `en_cours` (la page cesse de
/// relire) ; le clic « redemarrer » n'est offert que pour une version prete que rien ne retient, et le geste refuse sans
/// rien lancer dans tous les autres cas. Un echec (depot injoignable, signature refusee) se dit sans rien preparer.
#[test]
fn la_recherche_dit_ou_elle_en_est_et_se_tait_quand_elle_a_fini() {
    let fini = |e: &Value, etape: &str| {
        assert_eq!(e["etape"], etape, "{}", e);
        assert_eq!(e["en_cours"], false, "une recherche finie n'est pas en cours : {}", e);
        if matches!(etape, "a_jour" | "retenue") {
            assert!(e["relire_ms"].as_u64().is_some(), "la recherche de fond continue : la page relit : {}", e);
        } else {
            assert!(e["relire_ms"].is_null(), "une recherche finie pour de bon ne se relit plus : {}", e);
        }
        assert!(e["texte"].as_str().is_some_and(|t| !t.is_empty()), "elle se dit : {}", e);
    };
    let relais = |inst: &Path| std::fs::read_dir(inst.join("maj")).map(|d| d.flatten().any(|f| f.file_name().to_string_lossy().starts_with("relais-"))).unwrap_or(false);

    let rien = maj::Recherche::nouvelle();
    let e = rien.etat();
    assert_eq!((e["texte"].as_str(), e["en_cours"].as_bool(), e["redemarrer"].as_bool()), (Some(""), Some(false), Some(false)), "sans recherche, rien : {}", e);

    let base = dossier_de_test("mise_a_jour", "indicateur");
    let depot = publiee(&base, false);
    let inst = installee(&base);
    ajoutee(&inst.join("compagnies").join("essai"));

    // Le depot ne repond pas : un echec, rien de pret, rien a redemarrer.
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": base.join("pas-de-depot").display().to_string()}));
    let suivi = maj::Recherche::nouvelle();
    maj::verifier_suivi(&inst, true, &[&publique()], &suivi);
    fini(&suivi.etat(), "echec");
    assert_eq!(suivi.etat()["redemarrer"], false);

    // Une signature qui n'est pas celle de l'editeur : un echec, rien de deballe.
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": depot.display().to_string()}));
    maj::verifier_suivi(&inst, true, &maj::cles_de_l_editeur(), &suivi);
    fini(&suivi.etat(), "echec");
    assert!(maj::prete(&inst).is_none() && !deballee(&inst, "2026.11.01").exists());
    assert!(maj::redemarrer_pour_mettre_a_jour(&inst, &suivi).is_err() && !relais(&inst), "rien de pret : rien ne part");

    // La bonne : telechargee, prete, le clic est offert.
    maj::verifier_suivi(&inst, true, &[&publique()], &suivi);
    let e = suivi.etat();
    fini(&e, "prete");
    assert_eq!((e["redemarrer"].as_bool(), e["version"].as_str()), (Some(true), Some("01.11.2026")), "{}", e);
    // Une autre recherche (une page d'essai, un autre processus) n'a rien de pret : le geste refuse sans rien lancer.
    assert!(maj::redemarrer_pour_mettre_a_jour(&inst, &rien).is_err() && !relais(&inst));

    // Prete, mais elle change un calcul et une compagnie reelle a un mois saisi sans DSN : retenue, pas de clic.
    let mut p: Value = serde_json::from_str(&lire(&inst.join("maj").join("prete.json"))).unwrap();
    p["calcul"] = json!("la CSG");
    crate::outils::ecrire(&inst, "maj/prete.json", &p);
    let reelle = inst.join("compagnies").join("reelle");
    compagnie::config_et_mois("fictive-3090-petite", &[], &reelle);
    crate::outils::poser_saisie(&reelle, "2030-01", &json!({"mois": "2030-01", "engagements": [{"salarie": "INVENTE"}]}));
    maj::verifier_suivi(&inst, true, &[&publique()], &suivi);
    let e = suivi.etat();
    fini(&e, "retenue");
    assert_eq!(e["redemarrer"], false, "{}", e);
    assert!(e["detail"].as_str().unwrap().contains("2030-01 (reelle)"), "elle dit ce qui la retient : {}", e);
    assert!(maj::redemarrer_pour_mettre_a_jour(&inst, &suivi).is_err() && !relais(&inst), "retenue : rien ne part");

    // Installee : plus rien de plus recent.
    std::fs::remove_dir_all(&reelle).unwrap();
    maj::appliquer(&inst, false).unwrap();
    maj::verifier_suivi(&inst, true, &[&publique()], &suivi);
    let e = suivi.etat();
    fini(&e, "a_jour");
    assert_eq!(e["redemarrer"], false);
    assert_eq!(e["texte"], "Logiciel à jour", "ticket #217 : on dit ce qui est vrai, positivement : {}", e);
    assert_eq!(e["detail"], "PuppetPaie 01.11.2026 est la dernière version.", "{}", e);
}

/// LANCEE SANS RESEAU, LA RECHERCHE REPREND QUAND IL REVIENT (ticket #217, Leandre, 20/10/2026 : « Mise a jour : hors
/// ligne » restait affiche alors que le poste etait connecte, le logiciel ayant demarre sans reseau). Le depot coupe
/// toute connexion (le poste hors ligne) : la recherche echoue `hors ligne`, la bulle dit le prochain essai, la page
/// relit (`relire_ms`), et elle reessaie d'elle-meme ; le reseau revient : la version publiee est recue, prete, et la
/// boucle s'arrete (plus aucune demande). Un echec qu'un nouvel essai ne leverait pas (un depot introuvable) ne se
/// reessaie pas. Le rythme livre est raisonnable : jamais plus d'un essai par 30 secondes, un par 5 minutes au plus lent.
#[test]
fn lancee_sans_reseau_la_recherche_reprend_quand_il_revient() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    assert!(maj::ATTENTES_HORS_LIGNE.iter().all(|a| *a >= Duration::from_secs(30)), "aucune boucle rapide");
    assert!(maj::ATTENTES_HORS_LIGNE.last().is_some_and(|a| *a <= Duration::from_secs(300)), "le reseau revenu se voit en 5 minutes au plus");

    let base = dossier_de_test("mise_a_jour", "hors_ligne_puis_en_ligne");
    let depot = publiee(&base, false);
    let inst = installee(&base);
    ajoutee(&inst.join("compagnies").join("essai"));
    let en_ligne = Arc::new(AtomicBool::new(false));
    let (url, recu) = {
        let en_ligne = en_ligne.clone();
        crate::outils::serveur(move |q| {
            if !en_ligne.load(Ordering::SeqCst) {
                return None;
            }
            Some(match std::fs::read(depot.join(q.chemin.trim_start_matches('/'))) {
                Ok(o) => crate::outils::http(200, "application/octet-stream", &o),
                Err(_) => crate::outils::http(404, "text/plain", b"Not Found"),
            })
        })
    };
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": url}));

    let suivi = Arc::new(maj::Recherche::nouvelle());
    let fil = {
        let (inst, suivi) = (inst.clone(), suivi.clone());
        std::thread::spawn(move || maj::chercher_tant_qu_il_tourne(&inst, &[&publique()], &suivi, &[Duration::from_millis(100), Duration::from_millis(200)], Duration::from_secs(3600)))
    };
    let attendre = |quoi: &dyn Fn(&Value) -> bool, dit: &str| {
        let fin = Instant::now() + Duration::from_secs(120);
        loop {
            let e = suivi.etat();
            if quoi(&e) {
                return e;
            }
            assert!(Instant::now() < fin, "{} : {}", dit, e);
            std::thread::sleep(Duration::from_millis(20));
        }
    };

    // Hors ligne : l'echec se dit, le prochain essai aussi, et la page relit.
    let e = attendre(&|e| e["etape"] == "echec", "la recherche sans reseau echoue");
    assert_eq!(e["texte"], "Mise à jour : hors ligne", "{}", e);
    assert!(e["detail"].as_str().unwrap().contains("dès que le réseau revient"), "{}", e);
    assert_eq!(e["en_cours"], false, "{}", e);
    assert!(e["relire_ms"].as_u64().is_some(), "un nouvel essai est prevu : la page relit : {}", e);
    // Elle reessaie d'elle-meme.
    attendre(&|_| recu.lock().unwrap().len() >= 3, "elle reessaie");
    assert!(!fil.is_finished(), "hors ligne, la recherche continue");
    assert!(maj::prete(&inst).is_none());

    // Le reseau revient : la version est recue, prete, et la boucle s'arrete.
    en_ligne.store(true, Ordering::SeqCst);
    let e = attendre(&|e| e["etape"] == "prete", "le reseau revenu, la version se prepare");
    assert_eq!((e["texte"].as_str(), e["redemarrer"].as_bool()), (Some("Redémarrer pour mettre à jour"), Some(true)), "{}", e);
    assert!(e["relire_ms"].is_null(), "la recherche finie, la page se tait : {}", e);
    fil.join().unwrap();
    let demandes = recu.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(recu.lock().unwrap().len(), demandes, "plus aucune demande une fois prete");
    assert!(maj::prete(&inst).is_some_and(|p| p["version"] == "2026.11.01"));

    // Un depot qui repond « introuvable » : un nouvel essai n'y changerait rien, la boucle rend aussitot.
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": format!("{}/rien-ici", base.join("pas-de-depot").display())}));
    let autre = maj::Recherche::nouvelle();
    let debut = Instant::now();
    maj::chercher_tant_qu_il_tourne(&inst, &[&publique()], &autre, &[Duration::from_secs(3600)], Duration::from_secs(3600));
    assert!(debut.elapsed() < Duration::from_secs(60), "aucune attente");
    let e = autre.etat();
    assert_eq!(e["etape"], "echec", "{}", e);
    assert!(e["relire_ms"].is_null() && e["detail"].as_str().unwrap().contains("prochain lancement"), "{}", e);
}

/// A JOUR, LA RECHERCHE CONTINUE TANT QUE LE LOGICIEL TOURNE (Leandre, 21/10/2026 : la 2026.10.21, publiee a 11 h 40, ne
/// se voyait pas dans un logiciel lance avant, qui disait encore « Logiciel a jour » ; « toutes les 5 minutes »). Le
/// depot n'a rien de plus recent : « Logiciel a jour », et la page relit. Les tours suivants se font sans que
/// l'indicateur repasse par « Recherche de mise a jour… » ; une coupure le temps d'un tour le laisse « a jour ». Une
/// version publiee APRES est trouvee par un tour suivant, telechargee, preparee : « Redemarrer pour mettre a jour », la
/// boucle s'arrete, et rien n'est installe (le logiciel en place reste le meme). Le rythme livre est de 5 minutes.
#[test]
fn a_jour_la_recherche_continue_et_trouve_la_version_publiee_apres() {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::time::{Duration, Instant};

    assert_eq!(maj::RYTHME_A_JOUR, Duration::from_secs(300), "Leandre, 21/10/2026 : toutes les 5 minutes");

    let base = dossier_de_test("mise_a_jour", "a_jour_puis_publiee");
    let depot = publiee(&base, false);
    let inst = installee(&base);
    ajoutee(&inst.join("compagnies").join("essai"));
    // 0 : le depot dit la version en place ; 1 : il ne repond pas (un tour coupe) ; 2 : la 2026.11.01 est publiee.
    let publication = Arc::new(AtomicU8::new(0));
    let (url, recu) = {
        let (publication, depot) = (publication.clone(), depot.clone());
        crate::outils::serveur(move |q| {
            let nom = q.chemin.trim_start_matches('/');
            let o = std::fs::read(depot.join(nom));
            Some(match (publication.load(Ordering::SeqCst), o) {
                (1, _) => return None,
                (0, Ok(o)) if nom == "derniere.json" => {
                    let mut d: Value = serde_json::from_slice(&o).unwrap();
                    d["version"] = json!("2026.10.09");
                    crate::outils::http(200, "application/json", d.to_string().as_bytes())
                }
                (2, Ok(o)) => crate::outils::http(200, "application/octet-stream", &o),
                _ => crate::outils::http(404, "text/plain", b"Not Found"),
            })
        })
    };
    crate::outils::ecrire(&inst, "maj/reglage.json", &json!({"depot": url}));
    let demandes = || recu.lock().unwrap().iter().filter(|q| q.chemin.ends_with("derniere.json")).count();

    let suivi = Arc::new(maj::Recherche::nouvelle());
    let fil = {
        let (inst, suivi) = (inst.clone(), suivi.clone());
        std::thread::spawn(move || maj::chercher_tant_qu_il_tourne(&inst, &[&publique()], &suivi, &[Duration::from_secs(3600)], Duration::from_millis(100)))
    };
    let vues = Mutex::new(Vec::<String>::new());
    let attendre = |quoi: &dyn Fn(&Value) -> bool, dit: &str| {
        let fin = Instant::now() + Duration::from_secs(120);
        loop {
            let e = suivi.etat();
            let etape = e["etape"].as_str().unwrap_or_default().to_string();
            let mut v = vues.lock().unwrap();
            if v.last() != Some(&etape) {
                v.push(etape);
            }
            drop(v);
            if quoi(&e) {
                return e;
            }
            assert!(Instant::now() < fin, "{} : {}", dit, e);
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    // A jour : il le dit, et la page relit (la recherche de fond continue).
    let e = attendre(&|e| e["etape"] == "a_jour", "le depot n'a rien de plus recent");
    assert_eq!(e["texte"], "Logiciel à jour", "{}", e);
    assert!(e["relire_ms"].as_u64().is_some(), "a jour, la page relit : {}", e);
    vues.lock().unwrap().clear();
    // Des tours se font, sans que l'indicateur bouge ; un tour coupe le laisse « a jour ».
    let d = demandes();
    attendre(&|_| demandes() >= d + 3, "la recherche continue, a jour");
    publication.store(1, Ordering::SeqCst);
    let d = recu.lock().unwrap().len();
    attendre(&|_| recu.lock().unwrap().len() >= d + 2, "un tour sans reponse");
    assert!(!fil.is_finished(), "a jour, la recherche ne s'arrete pas");
    assert_eq!(*vues.lock().unwrap(), vec!["a_jour".to_string()], "un tour de fond ne fait pas clignoter l'indicateur");
    assert!(maj::prete(&inst).is_none());

    // La version est publiee apres : un tour la trouve, la prepare ; prete, la boucle s'arrete.
    publication.store(2, Ordering::SeqCst);
    let e = attendre(&|e| e["etape"] == "prete", "la version publiee apres se prepare");
    assert_eq!((e["texte"].as_str(), e["redemarrer"].as_bool(), e["version"].as_str()), (Some("Redémarrer pour mettre à jour"), Some(true), Some("01.11.2026")), "{}", e);
    assert!(e["relire_ms"].is_null(), "prete, plus rien ne change : {}", e);
    fil.join().unwrap();
    assert!(!vues.lock().unwrap().iter().any(|v| v == "recherche" || v == "echec"), "rien d'autre que ce qui change : {:?}", vues.lock().unwrap());
    assert!(maj::prete(&inst).is_some_and(|p| p["version"] == "2026.11.01"));
    assert!(deballee(&inst, "2026.11.01").join("VERSION").is_file(), "preparee a cote");
    // Rien n'est installe pendant que le logiciel tourne : la version en place reste.
    assert_eq!(maj::courante(&inst.join("logiciel")), "2026.10.09", "rien ne s'installe avant le redemarrage");
    let n = recu.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(recu.lock().unwrap().len(), n, "prete : plus aucune demande");
}
