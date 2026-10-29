//! CE QUE LE MOTEUR LIVRE ET COMPILE EST CE QUE LES TEXTES DEPOSES DONNENT. Chaque referentiel de `logiciel/donnees/`
//! releve par `moteur extraire` (`src/atelier/extraire/`) est ce que son texte depose dans `atelier/reference/` donne, a
//! l'octet, hors le `_source` (la date du releve et la commande) ; les tables Unicode du moteur (`socle/src/tables_nfkc.rs`,
//! `socle/src/nfkd_ascii.rs`, `envois/src/tables_retours.rs`, `envois/src/tables_journal.rs`) sont celles que `moteur
//! generer-tables` (`src/atelier/generer_tables.rs`) ecrit depuis `atelier/reference/sources/` (`unicode/`, `python/`,
//! `w3c/` : chaque `LISEZ-MOI.md` dit ce qu'on en lit). Un texte qui change sans que ce qui en vient suive, ou un
//! referentiel edite a la main, fait echouer le test.
//!
//! Le cahier technique 2026.1 en PDF (382 pages) ne se relit pas : ses releves lisent ses transcriptions, dont l'empreinte
//! est verifiee contre celle du PDF depose. Pour regenerer les tables : `moteur generer-tables atelier/reference/sources
//! atelier/sources` (ou `PUPPETPAIE_REECRIRE_ORACLES=1` sur ce test), puis lire le `git diff`.

use std::path::{Path, PathBuf};

use moteur::atelier::extraire::{self, Releve};
use moteur::atelier::generer_tables::{generer, FICHIERS};
use serde_json::Value;
use socle::date::Jour;

use crate::outils::{code_livre, reecrire_oracles};

fn reference(chemin: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference").join(chemin)
}

/// Le releve redonne le fichier livre `nom`, octet pour octet, une fois son `_source` remplace par celui du livre ; et son
/// `_source` nomme la commande et chaque fichier depose qu'il a lu.
fn redonne_le_livre(releve: Releve, nom: &str, commande: &str, deposes: &[&str]) {
    let source = releve.donnees["_source"].as_str().unwrap_or("").to_string();
    assert!(source.contains(commande), "le _source de {} ne nomme pas `{}` : {}", nom, commande, source);
    for d in deposes {
        assert!(source.contains(d), "le _source de {} ne nomme pas {} : {}", nom, d, source);
    }
    let livre = std::fs::read(code_livre().join(nom)).unwrap();
    let lu: Value = serde_json::from_slice(&livre).unwrap();
    let mut donnees = releve.donnees;
    donnees["_source"] = lu["_source"].clone();
    let refait = Releve { donnees, indent: releve.indent, saut_final: releve.saut_final, bilan: Vec::new() }.octets();
    if refait != livre {
        let (a, b) = (String::from_utf8_lossy(&refait), String::from_utf8_lossy(&livre));
        let ligne =
            a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(a.lines().count().min(b.lines().count()));
        panic!(
            "{} : le releve differe du livre a la ligne {} :\n  releve : {:?}\n  livre  : {:?}",
            nom,
            ligne + 1,
            a.lines().nth(ligne),
            b.lines().nth(ligne)
        );
    }
}

/// Une transcription du cahier 2026.1, deposee a cote de lui (`moteur extraire transcrire`) : elle porte l'empreinte du
/// PDF dont elle vient, et c'est celle du PDF depose.
fn transcription_du_cahier_2026_1(quoi: &str) -> PathBuf {
    let pdf = reference("norme-dsn/dsn-cahier-technique-2026.1.pdf");
    let chemin = reference(&format!("norme-dsn/dsn-cahier-technique-2026.1-{}.txt", quoi));
    let t = extraire::Transcription::lire(&chemin).unwrap();
    let octets = std::fs::read(&pdf).unwrap();
    assert_eq!(t.sha256, socle::fichiers::sha256_hex(&octets), "{} : le PDF depose a change, la transcription se refait", chemin.display());
    chemin
}

#[test]
fn chaque_referentiel_livre_et_chaque_table_sont_ce_que_les_textes_deposes_donnent() {
    let jour = Jour::nouveau(2026, 10, 15).unwrap();

    // La NAF : les deux classeurs de l'Insee.
    let (s, c) = ("insee-naf-2025-structure-20241004.xlsx", "insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx");
    let r = extraire::naf::relever(&reference(&format!("sources/insee-naf-2025/{}", s)), &reference(&format!("sources/insee-naf-2025/{}", c)), &jour);
    redonne_le_livre(r.unwrap(), "naf-base.json", "moteur extraire naf", &[s, c]);

    // Les usages et les formats de la norme P27V01 : les classeurs 2027.1.2.
    let x = "dsn-tableau-des-usages-2027.1.2-20251224.xlsx";
    let r = extraire::usages::relever(&reference(&format!("norme-dsn/{}", x)), &jour);
    redonne_le_livre(r.unwrap(), "usages-dsn-p27v01-base.json", "moteur extraire usages", &[x]);
    let x = "dsn-tableaux-categories-controles-datatypes-usages-2027.1.2-20251224.xlsx";
    let r = extraire::formats::relever(&reference(&format!("norme-dsn/{}", x)), &jour);
    redonne_le_livre(r.unwrap(), "formats-dsn-p27v01-base.json", "moteur extraire formats", &[x]);

    // Les usages et les formats de la norme P26V01 : le cahier 2026.1. Le tableau des usages par rubrique occupe les
    // pages 363 a 371 ; celui des usages des controles qui le suit (p. 372-382) n'a aucune ligne de rubrique.
    let r = extraire::usages::relever(&transcription_du_cahier_2026_1("usages"), &jour).unwrap();
    assert!(r.donnees["_source"].as_str().unwrap().contains("(p. 363-371)"), "{}", r.donnees["_source"]);
    redonne_le_livre(r, "usages-dsn-base.json", "moteur extraire usages", &["dsn-cahier-technique-2026.1.pdf"]);
    let r = extraire::formats::relever(&transcription_du_cahier_2026_1("formats"), &jour);
    redonne_le_livre(r.unwrap(), "formats-dsn-base.json", "moteur extraire formats", &["dsn-cahier-technique-2026.1.pdf"]);

    // Les organismes Urssaf de la DPAE : les trois tables.
    let (table, sites, cog) =
        ("urssaf-table-des-urssaf-20260618.csv", "urssaf-liste-des-sites-et-des-activites.csv", "insee-cog-2015-departements.zip");
    let r = extraire::urssaf::relever(&reference(&format!("sources/{}", table)), &reference(&format!("sources/{}", sites)), &reference(&format!("sources/{}", cog)), &jour);
    redonne_le_livre(r.unwrap(), "urssaf-dpae-base.json", "moteur extraire urssaf", &[table, sites, cog]);

    // Les tables Unicode compilees dans le socle et les envois.
    let fichiers = generer(&reference("sources")).unwrap();
    assert_eq!(fichiers.iter().map(|(n, _)| *n).collect::<Vec<_>>(), FICHIERS);
    let mut ecarts = Vec::new();
    for (nom, contenu) in fichiers {
        let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(nom);
        if std::fs::read_to_string(&chemin).unwrap() != contenu {
            if reecrire_oracles() {
                std::fs::write(&chemin, contenu).unwrap();
            } else {
                ecarts.push(nom);
            }
        }
    }
    assert!(ecarts.is_empty(), "{:?} ne sont pas ce que les textes deposes donnent : moteur generer-tables atelier/reference/sources atelier/sources", ecarts);
}
