//! LE REJEU D'UN SCENARIO ET SES SORTIES.
//!
//! Un scenario enregistre au banc d'essai (`moteur/src/atelier/scenario.rs`, `atelier/outils/banc/`) est fige dans
//! `tests/donnees/scenarios/<nom>/` (`gestes.jsonl`, `pieces/`) ; `rejouer` le rejoue dans un `dossier_de_test`, dans le
//! processus du test, et `sorties` releve ce que la compagnie en a tire. `rejouer_et_comparer` compare ce releve a
//! l'oracle `tests/donnees/scenarios/<nom>/sorties.json` (reecrit par `PUPPETPAIE_REECRIRE_ORACLES=1`).
//! `rejouer_et_lire` rejoue une fois et lit la compagnie le soir de jours choisis (`Soir` : une ligne de bulletin, un bloc
//! de DSN, une case de l'ecran des etapes), sans rejouer pour chaque date.
//!
//! CE QUE LE RELEVE GARDE, fichier par fichier, sous la compagnie :
//! - les paies de chaque mois saisi, CALCULEES au dernier jour du scenario (`engagement::charger`, rien
//!   d'ecrit) : brut, net, cout, chaque ligne de cotisation, telles que le moteur les rend ;
//! - chaque `.dsn` (les mensuelles, les signalements), ligne par ligne ;
//! - chaque `.txt`, `.csv` et `.eml` (la DPAE a largeur fixe, les recapitulatifs, les traces de depot), ligne par ligne ;
//! - le texte de chaque PDF (bulletins, certificats Conges Spectacles, contrats : `fabrique_pdf::lecture::texte`), ligne par ligne ;
//! - chaque `.json` (les fiches, les compteurs, les saisies, le journal des depots, l'echeancier, la memoire de
//!   l'horloge...), tel quel ;
//! - de tout autre fichier (une archive), son empreinte.
//!
//! RIEN N'Y EST ECARTE POUR CAUSE DE HASARD : le rejeu est deterministe (le temps du moteur est fige a l'instant de
//! chaque geste et les aleas suivent une suite fixe, `socle/src/temps.rs`), deux rejeux ecrivent les memes octets. Ce
//! qui est ecarte, et pourquoi :
//! - le chemin du dossier de test, s'il apparait (un texte qui cite un chemin) : remplace par `<racine>` ;
//! - le cache que la fenetre refait (`ouverture.etat.json`) et les fichiers caches (`.ouverture.json`, la
//!   trace du dernier passage de la veille, `.origine/`) : ce ne sont pas des sorties ;
//! - les copies d'avant une ecriture (`avant/`) : seulement leur nom.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

/// Un rejeu a la fois par processus : le temps du moteur est celui du processus (`temps.rs`).
static UN_A_LA_FOIS: Mutex<()> = Mutex::new(());

/// Le dossier d'un scenario fige : `tests/donnees/scenarios/<nom>`.
pub fn scenario(nom: &str) -> PathBuf {
    super::chemin_oracle(&format!("scenarios/{}", nom))
}

/// Rejoue le scenario `nom` dans `dossier` (une installation neuve) ; rend le compte rendu et la racine de la compagnie
/// qu'il a ajoutee (la premiere). Un ecart avec l'enregistrement fait echouer le test.
pub fn rejouer(nom: &str, dossier: &Path) -> (moteur::atelier::scenario::Rejeu, PathBuf) {
    rejouer_dossier(&scenario(nom), dossier)
}

/// `rejouer`, pour un scenario pris ailleurs que dans `tests/donnees/scenarios/` (un scenario elague par le test).
pub fn rejouer_dossier(sc: &Path, dossier: &Path) -> (moteur::atelier::scenario::Rejeu, PathBuf) {
    let _un = UN_A_LA_FOIS.lock().unwrap_or_else(|p| p.into_inner());
    let nom = sc.display();
    let r = moteur::atelier::scenario::rejouer(sc, &dossier.join("installation"), &moteur::atelier::scenario::OptionsRejeu::default())
        .unwrap_or_else(|m| panic!("le scenario {} ne se rejoue pas : {}", nom, m));
    assert!(r.ecarts.is_empty(), "le rejeu de {} s'ecarte de l'enregistrement :\n{}", nom, r.ecarts.join("\n"));
    let racine = r.compagnies.first().map(|(_, p)| p.clone()).unwrap_or_else(|| panic!("le scenario {} n'ajoute aucune compagnie", nom));
    (r, racine)
}

/// UN SOIR DU REJEU : la compagnie telle qu'elle est a la fin du jour `jour`, avant le premier geste du lendemain
/// (`rejouer_et_lire`). Ce qu'un test y lit, il le lit comme l'ecran ou un salarie le lirait.
pub struct Soir<'a> {
    pub jour: String,
    pub racine: &'a Path,
}

impl Soir<'_> {
    /// Le texte du bulletin de `salarie` pour `mois` (`sorties/<mois>/envoi/Bulletin_<SALARIE>_…`), ligne a ligne ; vide
    /// s'il n'est pas encore fait.
    pub fn bulletin(&self, mois: &str, salarie: &str) -> Vec<String> {
        let envoi = self.racine.join("sorties").join(mois).join("envoi");
        let prefixe = format!("Bulletin_{}_", salarie);
        let mut noms: Vec<String> = std::fs::read_dir(&envoi)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(&prefixe) && n.ends_with(".pdf"))
            .collect();
        noms.sort();
        let Some(nom) = noms.last() else { return Vec::new() };
        let (pages, _) = fabrique_pdf::lecture::texte(&envoi.join(nom).to_string_lossy()).unwrap_or_else(|m| panic!("{} : {}", nom, m.0));
        pages.join("\n").lines().map(|l| l.trim_end().to_string()).collect()
    }

    /// La premiere ligne du bulletin de `salarie` pour `mois` qui contient `libelle`.
    pub fn ligne_de_bulletin(&self, mois: &str, salarie: &str, libelle: &str) -> Option<String> {
        self.bulletin(mois, salarie).into_iter().find(|l| l.contains(libelle))
    }

    /// Le bloc de l'individu `nom` dans la DSN mensuelle de `mois` (`sorties/<mois>/DSN_<Mois><Annee>.dsn`) : (rubrique,
    /// valeur), dans l'ordre ; vide si elle n'est pas encore fabriquee.
    pub fn bloc_dsn(&self, mois: &str, nom: &str) -> Vec<(String, String)> {
        let f = self.racine.join("sorties").join(mois).join(format!("DSN_{}.dsn", socle::date::etiquette_du_mois(mois).unwrap()));
        let Ok(octets) = std::fs::read(&f) else { return Vec::new() };
        super::individu(&octets.iter().map(|&o| o as char).collect::<String>(), nom)
    }

    /// L'etat de chaque groupe de la case `cle` du mois `mois`, tel que l'ecran des etapes le montre ce soir-la
    /// (`etat-du-mois`).
    pub fn case(&self, mois: &str, cle: &str) -> Vec<String> {
        let r = super::route(self.racine, "etat-du-mois", json!({"mois": mois}), &self.jour);
        let etapes = super::reussi(&r)["etapes"].as_array().cloned().unwrap_or_default();
        let e = etapes.iter().find(|e| e["cle"] == json!(cle)).unwrap_or_else(|| panic!("la case {} de {} manque", cle, mois)).clone();
        e["groupes"].as_array().into_iter().flatten().map(|g| g["etat"].as_str().unwrap_or_default().to_string()).collect()
    }
}

/// REJOUE le scenario `nom` UNE FOIS, et le lit le soir de chacun des `jours` (AAAA-MM-JJ, dans l'ordre) : `lire` recoit
/// la compagnie telle qu'elle est ce soir-la, puis le rejeu reprend. Un ecart avec l'enregistrement fait echouer le test.
pub fn rejouer_et_lire(nom: &str, dossier: &Path, jours: &[&str], mut lire: impl FnMut(&Soir)) -> (moteur::atelier::scenario::Rejeu, PathBuf) {
    let _un = UN_A_LA_FOIS.lock().unwrap_or_else(|p| p.into_inner());
    let arrets: Vec<socle::date::Jour> = jours.iter().map(|j| socle::date::Jour::lire(j).unwrap()).collect();
    let mut lus = 0;
    let r = moteur::atelier::scenario::rejouer_et_lire(&scenario(nom), &dossier.join("installation"), &Default::default(), &arrets, &mut |jour, r| {
        let (_, racine) = r.compagnies.first().unwrap_or_else(|| panic!("le scenario {} n'a pas encore de compagnie le {}", nom, jour.iso()));
        lire(&Soir { jour: jour.iso(), racine });
        lus += 1;
    })
    .unwrap_or_else(|m| panic!("le scenario {} ne se rejoue pas : {}", nom, m));
    assert_eq!(lus, jours.len(), "des soirs n'ont pas ete lus");
    assert!(r.ecarts.is_empty(), "le rejeu de {} s'ecarte de l'enregistrement :\n{}", nom, r.ecarts.join("\n"));
    let racine = r.compagnies.first().map(|(_, p)| p.clone()).unwrap_or_else(|| panic!("le scenario {} n'ajoute aucune compagnie", nom));
    (r, racine)
}

fn masquer(v: Value, racine: &str) -> Value {
    match v {
        Value::String(s) => Value::String(s.replace(racine, "<racine>")),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| masquer(x, racine)).collect()),
        // L'empreinte du code du moteur change a chaque compilation qui touche le moteur : elle dit quel calcul a ecrit
        // la paie, pas ce que la paie vaut.
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, x)| if k == "moteur" && x.is_string() { (k, json!("<moteur>")) } else { (k, masquer(x, racine)) })
                .collect(),
        ),
        x => x,
    }
}

fn lignes(texte: &str, racine: &str) -> Value {
    Value::Array(texte.replace(racine, "<racine>").lines().map(|l| json!(l.trim_end())).collect())
}

/// Les fichiers d'un dossier, relatifs (barres droites), dans l'ordre des noms.
fn fichiers(base: &Path) -> Vec<String> {
    fn parcourir(base: &Path, d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if p.is_dir() {
                parcourir(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    parcourir(base, base, &mut out);
    out.sort();
    out
}

/// LE RELEVE des sorties de la compagnie `racine`, lue au jour `jour` (AAAA-MM-JJ, le dernier du scenario).
pub fn sorties(racine: &Path, jour: &str) -> Value {
    let r = racine.to_string_lossy().into_owned();
    let mut par_fichier = Map::new();
    let mut avant = Vec::new();
    for rel in fichiers(racine) {
        if rel == "ouverture.etat.json" {
            continue;
        }
        if rel.contains("/avant/") || rel.starts_with("avant/") {
            avant.push(json!(rel));
            continue;
        }
        let p = racine.join(&rel);
        let ext = Path::new(&rel).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let octets = std::fs::read(&p).unwrap();
        let v = match ext.as_str() {
            "json" => masquer(serde_json::from_slice(&octets).unwrap_or_else(|e| panic!("{} illisible : {}", rel, e)), &r),
            // Les fichiers a largeur fixe sont en latin-1 : un octet, un caractere.
            "dsn" | "txt" | "csv" | "eml" => lignes(&octets.iter().map(|&o| o as char).collect::<String>(), &r),
            "pdf" => {
                let (pages, _) = fabrique_pdf::lecture::texte(&p.to_string_lossy()).unwrap_or_else(|m| panic!("{} : {}", rel, m.0));
                lignes(&documents::avenant::normaliser(&pages.join("\n")), &r)
            }
            _ => json!({"empreinte": socle::fichiers::sha256_hex(&octets)}),
        };
        par_fichier.insert(rel, v);
    }
    // Les paies de chaque mois saisi, calculees au dernier jour du scenario.
    let mut paies = Map::new();
    let ctx = droit::contexte::Contexte::nouveau(&racine, Some(socle::date::Jour::lire(jour).unwrap())).unwrap();
    for mois in engagement::stock::mois_saisis(&racine).unwrap() {
        let calcul = engagement::mois_pour_l_ecran(&ctx, &mois)
            .and_then(|s| s.paies.iter().map(|p| p.canon(&ctx)).collect::<Result<Vec<_>, _>>())
            .map_or_else(|m| json!({"refus": m}), |v| json!({"paies": v}));
        paies.insert(mois.to_string(), masquer(calcul, &r));
    }
    json!({"paies": paies, "fichiers": par_fichier, "copies_avant": avant})
}

/// LE TEST D'UNE FICTIVE (le modele des tests de la phase 4) : rejoue le scenario `nom` dans `dossier`, releve ses
/// sorties, et les compare a l'oracle `scenarios/<nom>/sorties.json`. Rend le releve.
pub fn rejouer_et_comparer(nom: &str, dossier: &Path) -> Value {
    let (r, racine) = rejouer(nom, dossier);
    let jour = socle::paris::iso_secondes(r.fin.expect("un scenario sans geste"))[..10].to_string();
    let releve = sorties(&racine, &jour);
    super::oracle(&format!("scenarios/{}/sorties.json", nom), "", &releve);
    releve
}
