//! `moteur releve-missions <sources> <donnees>` : les deux tables de la DGFiP dont le moteur tire la limite des frais
//! d'un deplacement a l'etranger (`droit::missions`), relevees par l'API publique d'opendatasoft de
//! data.economie.gouv.fr (`/api/explore/v2.1/catalog/datasets/<jeu>/exports/csv`, sans cle) :
//! - `dgfip-frais-de-mission-etranger` : les indemnites journalieres de mission, par pays et ville, datees ;
//! - `dgfip-taux-de-change` : les taux de chancellerie, dates.
//!
//! Chaque table relevee se depose entiere et datee dans `<sources>/dgfip-frais-de-mission/` ; la table livree
//! (`<donnees>/missions/`) n'en garde que ce qu'une paie peut lire (`droit::missions::pour_les_paies`, depuis la plus
//! ancienne annee dont un bareme est livre), relue comme le moteur la lit (`droit::missions::verifier`). « Publier » l'appelle avant de
//! livrer (`.github/workflows/publier.yml`) : une installation recoit les taux du jour de la publication, et une paie
//! passee ne change pas, son empreinte ne retenant que les valeurs de ses jours.

use std::path::Path;

/// L'adresse d'un export CSV de data.economie.gouv.fr.
const EXPORT: &str = "https://data.economie.gouv.fr/api/explore/v2.1/catalog/datasets/{}/exports/csv?delimiter=%3B";

/// Les deux jeux : leur identifiant, le nom du depot date, le nom de la table livree, les colonnes qui designent un lieu.
const JEUX: [(&str, &str, &str, &[&str]); 2] = [
    ("dgfip-frais-de-mission-etranger", "dgfip-frais-de-mission-etranger", "indemnites.csv", &["code_pays"]),
    ("dgfip-taux-de-change", "dgfip-taux-de-change", "change.csv", &["monnaie_source", "code_pays"]),
];

const DOSSIER_DES_SOURCES: &str = "dgfip-frais-de-mission";

const USAGE: &str = "usage : moteur releve-missions <sources> <donnees>

Releve chez data.economie.gouv.fr les indemnites journalieres de mission a l'etranger et les taux de chancellerie de la
DGFiP, les relit comme le moteur, les depose datees dans <sources>/dgfip-frais-de-mission/ et remplace les tables livrees
de <donnees>/missions/. <sources> : atelier/reference/sources ; <donnees> : logiciel/donnees.";

/// La commande.
pub fn commande(args: &[String]) -> i32 {
    if args.len() != 2 || args.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        eprintln!("{}", USAGE);
        return 2;
    }
    match relever(Path::new(&args[0]), Path::new(&args[1]), &|url| crate::atelier::sources::telecharger(url, 120).map(|(o, _)| o)) {
        Ok(t) => {
            println!("{}", t);
            0
        }
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}

/// Releve, relit et depose les deux tables ; `telecharger` rend les octets d'une adresse. Rien ne s'ecrit si l'une des
/// deux ne se telecharge ou ne se lit pas.
pub fn relever(sources: &Path, donnees: &Path, telecharger: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Result<String, String> {
    let mut textes = Vec::new();
    for (jeu, _, _, _) in JEUX {
        let url = EXPORT.replace("{}", jeu);
        let o = telecharger(&url).map_err(|e| format!("{} : {}", url, e))?;
        textes.push(String::from_utf8(o).map_err(|_| format!("{} : le CSV n'est pas en UTF-8.", url))?);
    }
    // Ce qui se livre : ce qu'une paie peut lire, depuis le premier jour de la plus ancienne annee dont un bareme est
    // livre (un mois sans bareme ne se calcule pas).
    let depuis = premier_jour_calculable(donnees)?;
    let livres: Vec<String> =
        JEUX.iter().zip(&textes).map(|((_, _, _, cles), t)| droit::missions::pour_les_paies(t, cles, &depuis)).collect::<Result<_, _>>()?;
    let (lieux, taux) = droit::missions::verifier(&livres[0], &livres[1])?;
    let jour = droit::contexte::aujourdhui_sans_note()?;
    let date = jour.iso().replace('-', "");
    let depot = sources.join(DOSSIER_DES_SOURCES);
    std::fs::create_dir_all(&depot).map_err(|e| socle::echec::du_disque(&depot, &e))?;
    std::fs::create_dir_all(donnees.join("missions")).map_err(|e| socle::echec::du_disque(donnees, &e))?;
    for (((_, nom, livre, _), texte), livre_texte) in JEUX.iter().zip(&textes).zip(&livres) {
        let p = depot.join(format!("{}-{}.csv", nom, date));
        std::fs::write(&p, texte).map_err(|e| socle::echec::du_disque(&p, &e))?;
        let p = donnees.join("missions").join(livre);
        std::fs::write(&p, livre_texte).map_err(|e| socle::echec::du_disque(&p, &e))?;
    }
    Ok(format!("Tables des missions relevées le {} : {} lieux, {} taux de chancellerie.", jour.jj_mm_aaaa(), lieux, taux))
}

/// Le 1er janvier de la plus ancienne annee dont `<donnees>/baremes/` livre le bareme (`AAAA.json`) : avant, aucune paie
/// ne se calcule (`droit::contexte`, le socle du bareme de l'annee).
fn premier_jour_calculable(donnees: &Path) -> Result<socle::date::Jour, String> {
    let dossier = donnees.join("baremes");
    let lus = std::fs::read_dir(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e))?;
    let an = lus
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".json")?.parse::<i32>().ok())
        .min()
        .ok_or_else(|| format!("{} : aucun barème livré, la première année calculable ne se lit pas.", dossier.display()))?;
    socle::date::Jour::nouveau(an, 1, 1).ok_or_else(|| format!("année de barème illisible : {}", an))
}
