//! `moteur releve-missions <sources> <donnees>` : les deux tables de la DGFiP dont le moteur tire la limite des frais
//! d'un deplacement a l'etranger (`droit::missions`), relevees par l'API publique d'opendatasoft de
//! data.economie.gouv.fr (`/api/explore/v2.1/catalog/datasets/<jeu>/exports/csv`, sans cle) :
//! - `dgfip-frais-de-mission-etranger` : les indemnites journalieres de mission, par pays et ville, datees ;
//! - `dgfip-taux-de-change` : les taux de chancellerie, dates.
//!
//! Chaque table relevee se relit comme le moteur la lit (`droit::missions::verifier`) ; lue, elle se depose datee dans
//! `<sources>/dgfip-frais-de-mission/` et remplace la table livree (`<donnees>/missions/`). « Publier » l'appelle avant de
//! livrer (`.github/workflows/publier.yml`) : une installation recoit les taux du jour de la publication, et une paie
//! passee ne change pas, son empreinte ne retenant que les valeurs de ses jours.

use std::path::Path;

/// L'adresse d'un export CSV de data.economie.gouv.fr.
const EXPORT: &str = "https://data.economie.gouv.fr/api/explore/v2.1/catalog/datasets/{}/exports/csv?delimiter=%3B";

/// Les deux jeux : leur identifiant, le nom du depot date, le nom de la table livree.
const JEUX: [(&str, &str, &str); 2] = [
    ("dgfip-frais-de-mission-etranger", "dgfip-frais-de-mission-etranger", "indemnites.csv"),
    ("dgfip-taux-de-change", "dgfip-taux-de-change", "change.csv"),
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
    for (jeu, _, _) in JEUX {
        let url = EXPORT.replace("{}", jeu);
        let o = telecharger(&url).map_err(|e| format!("{} : {}", url, e))?;
        textes.push(String::from_utf8(o).map_err(|_| format!("{} : le CSV n'est pas en UTF-8.", url))?);
    }
    let (lieux, taux) = droit::missions::verifier(&textes[0], &textes[1])?;
    let jour = droit::contexte::aujourdhui_sans_note()?;
    let date = jour.iso().replace('-', "");
    let depot = sources.join(DOSSIER_DES_SOURCES);
    std::fs::create_dir_all(&depot).map_err(|e| socle::echec::du_disque(&depot, &e))?;
    std::fs::create_dir_all(donnees.join("missions")).map_err(|e| socle::echec::du_disque(donnees, &e))?;
    for ((_, nom, livre), texte) in JEUX.iter().zip(&textes) {
        let p = depot.join(format!("{}-{}.csv", nom, date));
        std::fs::write(&p, texte).map_err(|e| socle::echec::du_disque(&p, &e))?;
        let p = donnees.join("missions").join(livre);
        std::fs::write(&p, texte).map_err(|e| socle::echec::du_disque(&p, &e))?;
    }
    Ok(format!("Tables des missions relevées le {} : {} lieux, {} taux de chancellerie.", jour.jj_mm_aaaa(), lieux, taux))
}
