//! `importation_tri` (lot A du plan d'import des bulletins) : la boite aux lettres `import/` se trie par le
//! contenu, jamais par le nom, et chaque fichier a un sort.
//!
//! Le dossier est FICTIF, monte ici : une DSN de la copie figee des compagnies fictives (`fictive-3090-petite`), des AEM
//! ecrites ici, un PDF fabrique par la fabrique, un PDF ecrit a la main avec un seul trait, un PDF casse, un
//! `.txt`, un fichier vide, une archive zip, des doublons. Rien de `prive/`, aucun des PDF reels de `import/`.

use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::bulletins_fictifs::pdf_en_traces;
use crate::outils::{aem, copier, fictive, dossier_de_test, route};
use paie::importation_tri::{trier, Sorte};

/// Un enregistrement AEM marque `AEMEDIC2`.
fn aem2(nir: &str, debut: &str, fin: &str, objet: &str) -> String {
    aem(&[(0, "AEMEDIC2")], nir, debut, fin, objet)
}

/// Un PDF avec du texte, fabrique par la fabrique.
fn pdf_texte() -> Vec<u8> {
    let d = json!({
        "format": 1,
        "page": {"largeur": 400.0, "hauteur": 500.0, "marges": {"haut": 50.0, "droite": 50.0, "bas": 50.0, "gauche": 50.0}},
        "police": {"taille": 10.0, "interligne": 1.25, "couleur": "#000000"},
        "documents": [{"blocs": [{"type": "paragraphe", "morceaux": [{"texte": "Bulletin de paie fictif, Fanchon Brioche"}]}]}]
    });
    fabrique_pdf::fabriquer(&d.to_string()).unwrap_or_else(|r| panic!("la fabrique refuse : {}", r.0))
}

fn zip(entrees: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = ::zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (nom, octets) in entrees {
        z.start_file(*nom, ::zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(octets).unwrap();
    }
    z.finish().unwrap().into_inner()
}

/// Le dossier `import/` fictif, sous `base`.
fn monter(base: &Path) -> PathBuf {
    let import = base.join("import");
    let _ = std::fs::remove_dir_all(base);
    std::fs::create_dir_all(import.join("sous")).unwrap();
    let bulletin = pdf_texte();
    std::fs::write(import.join("a-bulletin.pdf"), &bulletin).unwrap();
    std::fs::write(import.join("b-trace.pdf"), pdf_en_traces()).unwrap();
    std::fs::write(import.join("c-casse.pdf"), b"%PDF-1.7\nceci n'est pas un PDF\n").unwrap();
    // Une DSN qui s'appelle « .pdf » : c'est le contenu qui decide.
    std::fs::copy(fictive("fictive-3090-petite").join("sorties/2025-03/DSN_Mars2025.dsn"), import.join("declaration.pdf")).unwrap();
    let deux = format!(
        "{}\r\n{}\r\n",
        aem2("2881175999001", "20250310", "20250314", "256Z00920022"),
        aem2("2881175999001", "20250401", "20250402", "256Z00920022")
    );
    std::fs::write(import.join("sous").join("AEM_EDIV5_1.txt"), deux).unwrap();
    std::fs::write(import.join("notes.txt"), "Les bulletins de l'an dernier.\n").unwrap();
    std::fs::write(import.join("sous").join("notes-copie.txt"), "Les bulletins de l'an dernier.\n").unwrap();
    std::fs::write(import.join("vide.dat"), b"").unwrap();
    // L'archive : une AEM, la copie exacte du bulletin, et un chemin qui voudrait sortir de l'archive.
    let une = format!("{}\r\n", aem2("2881175999001", "20250501", "20250503", "256Z00920022"));
    let archive = zip(&[("aem/AEM_EDIV5_2.txt", une.as_bytes()), ("copie-du-bulletin.pdf", &bulletin), ("../evasion.txt", b"dehors")]);
    std::fs::write(import.join("lot.zip"), archive).unwrap();
    import
}

fn sorte_de<'a>(t: &'a paie::importation_tri::Tri, chemin: &str) -> &'a paie::importation_tri::Fichier {
    t.fichiers.iter().find(|f| f.chemin == chemin).unwrap_or_else(|| {
        panic!("{} absent du tri : {:?}", chemin, t.fichiers.iter().map(|f| (&f.chemin, f.sorte)).collect::<Vec<_>>())
    })
}

#[test]
fn importation_tri_chaque_fichier_a_un_sort() {
    let base = dossier_de_test("importation_tri", "fonction");
    let import = monter(&base);
    // Le dossier de travail SOUS le dossier trie : il n'est pas parcouru.
    let travail = import.join(".travail");
    let t = trier(&import, &travail).unwrap();
    let vus: Vec<(&str, Sorte)> = t.fichiers.iter().map(|f| (f.chemin.as_str(), f.sorte)).collect();
    assert_eq!(
        vus,
        vec![
            ("a-bulletin.pdf", Sorte::PdfTexte),
            ("b-trace.pdf", Sorte::PdfSansTexte),
            ("c-casse.pdf", Sorte::Illisible),
            ("declaration.pdf", Sorte::Dsn),
            ("lot.zip", Sorte::Archive),
            ("lot.zip/../evasion.txt", Sorte::Illisible),
            ("lot.zip/aem/AEM_EDIV5_2.txt", Sorte::Aem),
            ("lot.zip/copie-du-bulletin.pdf", Sorte::Doublon),
            ("notes.txt", Sorte::Autre),
            ("sous/AEM_EDIV5_1.txt", Sorte::Aem),
            ("sous/notes-copie.txt", Sorte::Doublon),
            ("vide.dat", Sorte::Autre),
        ],
        "{:#?}",
        t.fichiers
    );

    let f = sorte_de(&t, "a-bulletin.pdf");
    assert_eq!(f.pages.len(), 1);
    assert!(f.pages[0].mots > 0, "{:?}", f.pages);
    assert_eq!(f.empreinte.len(), 64);
    assert_eq!(f.taille, std::fs::metadata(import.join("a-bulletin.pdf")).unwrap().len());

    let f = sorte_de(&t, "b-trace.pdf");
    assert_eq!(f.pages.len(), 1);
    assert_eq!((f.pages[0].mots, f.pages[0].polices, f.pages[0].images), (0, 0, 0));
    assert!(f.pages[0].traits >= 1, "{:?}", f.pages);
    assert!(f.raison.as_deref().unwrap_or("").contains("aucun mot"), "{:?}", f.raison);

    assert!(sorte_de(&t, "c-casse.pdf").raison.as_deref().unwrap_or("").starts_with("PDF que la fabrique n'ouvre pas"));
    assert_eq!(sorte_de(&t, "declaration.pdf").mois.as_deref(), Some("2025-03"));
    assert_eq!(sorte_de(&t, "sous/AEM_EDIV5_1.txt").enregistrements, Some(2));
    assert_eq!(sorte_de(&t, "lot.zip/aem/AEM_EDIV5_2.txt").archive.as_deref(), Some("lot.zip"));
    assert_eq!(sorte_de(&t, "lot.zip/copie-du-bulletin.pdf").doublon_de.as_deref(), Some("a-bulletin.pdf"));
    assert_eq!(sorte_de(&t, "sous/notes-copie.txt").doublon_de.as_deref(), Some("notes.txt"));
    let notes = sorte_de(&t, "notes.txt");
    assert_eq!(notes.extension, "txt");
    assert!(notes.raison.as_deref().unwrap_or("").contains(".txt"), "{:?}", notes.raison);
    assert_eq!(sorte_de(&t, "vide.dat").raison.as_deref(), Some("fichier vide"));

    // Le zip s'est ouvert dans `travail/`, et nulle part ailleurs : le chemin « ../ » n'a rien ecrit.
    let sous: Vec<_> = std::fs::read_dir(&travail).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(sous.len(), 1, "{:?}", sous);
    assert!(sous[0].starts_with("lot.zip-"), "{:?}", sous);
    assert!(travail.join(&sous[0]).join("aem").join("AEM_EDIV5_2.txt").is_file());
    assert!(!travail.join("evasion.txt").exists() && !import.join("evasion.txt").exists() && !base.join("evasion.txt").exists());

    // Le JSON : chaque fichier, et les comptes par sorte (toutes les sortes).
    let j = t.json();
    assert_eq!(j["total"], json!(12));
    assert_eq!(
        j["comptes"],
        json!({"dsn": 1, "aem": 2, "pdf_texte": 1, "pdf_sans_texte": 1, "archive": 1, "doublon": 2, "illisible": 2, "autre": 2})
    );
    let b = &j["fichiers"][1];
    assert_eq!(b["chemin"], json!("b-trace.pdf"));
    assert_eq!(b["sorte"], json!("pdf_sans_texte"));
    assert_eq!(b["pages"][0]["texte"], json!(false));
    assert_eq!(j["fichiers"][0]["pages"][0]["texte"], json!(true));
    assert!(j["fichiers"][3].get("pages").is_none(), "une DSN n'a pas de pages : {}", j["fichiers"][3]);

    // Un dossier qui n'existe pas : refus, pas un tri vide.
    let e = trier(&base.join("absent"), &base.join("travail")).unwrap_err();
    assert_eq!(e.classe, Some("ErreurOperation"));
}

/// Toute l'arborescence d'un dossier, chemins relatifs.
fn arbre(d: &Path, base: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
        if p.is_dir() {
            arbre(&p, base, out);
        }
    }
}

#[test]
fn importation_tri_la_route_lit_import_de_la_compagnie_sans_rien_y_ecrire() {
    let base = dossier_de_test("importation_tri", "route");
    let import = monter(&base.join("compagnie"));
    let racine = base.join("compagnie");
    copier(&fictive("fictive-3090-petite").join("config"), &racine.join("config"));
    std::fs::create_dir_all(racine.join("paies")).unwrap();
    let mut avant = Vec::new();
    arbre(&racine, &racine, &mut avant);
    avant.sort();

    let r = route(&racine, "importation-tri", json!({}), "2026-10-03");
    assert_eq!(r["ok"], json!(true), "{}", r);
    let t = &r["resultat"];
    assert_eq!(t["dossier"], json!(import.display().to_string()));
    assert_eq!(t["total"], json!(12), "{}", t);
    assert_eq!(t["comptes"]["dsn"], json!(1));
    assert_eq!(t["comptes"]["aem"], json!(2));
    assert_eq!(t["comptes"]["doublon"], json!(2));

    // Le meme tri, le dossier nomme.
    let r2 = route(&racine, "importation-tri", json!({"dossier": import.display().to_string()}), "2026-10-03");
    assert_eq!(r2["resultat"]["comptes"], t["comptes"]);

    // Rien n'a ete ecrit dans la compagnie (le zip s'est ouvert dans un dossier temporaire, efface).
    let mut apres = Vec::new();
    arbre(&racine, &racine, &mut apres);
    apres.sort();
    assert_eq!(avant, apres);

    // Un dossier absent : refus dit.
    let r = route(&racine, "importation-tri", json!({"dossier": base.join("absent").display().to_string()}), "2026-10-03");
    assert_eq!(r["ok"], json!(false), "{}", r);
    assert_eq!(r["classe"], json!("ErreurOperation"));
}
