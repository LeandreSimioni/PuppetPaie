//! Le document que l'import a accepte sans le lire : il part chez l'editeur, EN SILENCE, par le journal des incidents
//! (`compagnie::incidents`), pour que la table de lecture (`bulletins-lecture-base.json`) apprenne sa forme. Rien ne
//! s'en demande a la personne : l'encart de l'import dit seulement quels documents ne sont pas repris, et pourquoi.
//!
//! CE QUI EST ACCEPTE SANS ETRE LU : un PDF qui a du texte (`tri::Sorte::PdfTexte`) dont le compte rendu dit qu'il n'est
//! pas repris (`bulletins::non_repris`). Un scan (`PdfSansTexte`) et un fichier qui n'est ni PDF, ni DSN, ni AEM sont
//! refuses a l'ecran avec leur raison (`tri`) : ils ne font pas d'incident.
//!
//! UNE FAMILLE, UN INCIDENT : les documents d'un meme producteur et d'une meme mise en page portent les memes mots du
//! vocabulaire, dans le meme ordre, sur leur premiere page ; cette suite, avec le logiciel reconnu, fait la famille.
//! Un seul document de reference part par famille, et une famille deja au journal ne se note plus (un import rejoue
//! n'envoie rien de neuf).
//!
//! CE QUI PART EST MASQUE PAR LISTE BLANCHE (`masquer`) : le texte des pages, ligne a ligne, a sa place ; chaque mot du
//! vocabulaire de la paie et des pieces (les libelles de la table de lecture, ceux des AEM, des certificats Congés
//! Spectacles et des AER, les mots de liaison) reste, un code de rubrique DSN (`S21.G00.40.001`) reste, toute autre
//! lettre devient `X` et tout chiffre `9`. Un nom, un NIR, un montant, une adresse n'y survivent pas, qu'on les
//! connaisse ou non. LE PDF LUI-MEME NE PART JAMAIS, ni son nom (le fichier se dit par sa sorte et sa forme).

use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::compagnie::incidents;
use crate::import::bulletins::{self as ib, DossierLu};
use crate::import::tri::Sorte;
use droit::contexte::Contexte;
use socle::texte::sans_accent;

/// L'endroit, dans le journal des incidents.
const OU: &str = "importation";
/// La trace d'un incident tient en 4 000 caracteres chez le serveur.
const TRACE_MAX: usize = 4000;

/// Les mots des pieces que la table de lecture des bulletins ne porte pas (AEM, certificat Congés Spectacles, AER
/// Cinéma Spectacle), et les mots de liaison du francais : ils ne designent personne.
const MOTS_DES_PIECES: &[&str] = &[
    "A", "AU", "AUX", "DE", "DES", "DU", "D", "EN", "ET", "L", "LA", "LE", "LES", "OU", "PAR", "POUR", "SUR", "UN", "UNE",
    "N", "NO", "NUM", "NUMERO", "NE", "NEE", "OUI", "NON", "TOTAL", "PAGE", "SUR",
    "ATTESTATION", "ATTESTATIONS", "EMPLOYEUR", "MENSUELLE", "AEM", "AER", "CERTIFICAT", "EMPLOI", "CONGES", "SPECTACLES",
    "SPECTACLE", "CINEMA", "AUDIENS", "FRANCE", "TRAVAIL", "POLE", "ARTISTE", "ARTISTES", "TECHNICIEN", "TECHNICIENS",
    "INTERMITTENT", "INTERMITTENTS", "SALARIE", "SALARIEE", "NOM", "NOMS", "PRENOM", "PRENOMS", "NAISSANCE", "ADRESSE",
    "COMMUNE", "CODE", "POSTAL", "SECURITE", "SOCIALE", "IDENTIFICATION", "IDENTIFIANT", "ADHERENT", "SIRET", "SIREN",
    "APE", "NAF", "URSSAF", "CONVENTION", "COLLECTIVE", "IDCC", "LICENCE", "OBJET", "CONTRAT", "CONTRATS", "DATE",
    "DATES", "DEBUT", "FIN", "PERIODE", "MOIS", "ANNEE", "CACHET", "CACHETS", "HEURE", "HEURES", "JOUR", "JOURS",
    "SALAIRE", "SALAIRES", "BRUT", "BRUTS", "NET", "MONTANT", "CADRE", "NATURE", "MOTIF", "RUPTURE", "SIGNATURE",
    "CACHET", "RAISON", "SOCIALE", "ENTREPRISE", "ETABLISSEMENT", "ORGANISME", "CAISSE", "RETRAITE", "PREVOYANCE",
    "EMPLOYE", "EMPLOYEE", "QUALIFICATION", "FONCTION", "CATEGORIE", "PROFESSIONNELLE", "TRAVAILLES", "EFFECTUES",
    "VERSES", "VERSE", "CERTIFIE", "EXACTES", "INFORMATIONS", "FAIT", "LE", "CACHET",
];

/// Un code de rubrique DSN, ou un mot (lettres et chiffres d'un seul tenant).
fn jeton() -> &'static Regex {
    static J: OnceLock<Regex> = OnceLock::new();
    J.get_or_init(|| Regex::new(r"S\d{2}\.G\d{2}\.\d{2}(?:\.\d{3})?|[\p{L}\p{N}]+").expect("motif d'un mot"))
}

/// Un mot tel que le vocabulaire le compare : en capitales, sans accent.
fn plie(mot: &str) -> String {
    sans_accent(mot).to_uppercase()
}

/// Le vocabulaire : chaque mot des valeurs de la table de lecture livree (hors des cles `_`), et `MOTS_DES_PIECES`.
fn vocabulaire() -> &'static HashSet<String> {
    static V: OnceLock<HashSet<String>> = OnceLock::new();
    V.get_or_init(|| {
        let mut v: HashSet<String> = MOTS_DES_PIECES.iter().map(|m| m.to_string()).collect();
        fn parcourir(x: &Value, v: &mut HashSet<String>) {
            match x {
                Value::String(s) => {
                    for m in jeton().find_iter(s) {
                        if m.as_str().chars().all(char::is_alphabetic) {
                            v.insert(plie(m.as_str()));
                        }
                    }
                }
                Value::Array(a) => a.iter().for_each(|y| parcourir(y, v)),
                Value::Object(o) => o.iter().filter(|(k, _)| !k.starts_with('_')).for_each(|(_, y)| parcourir(y, v)),
                _ => {}
            }
        }
        let chemin = socle::fichiers::dossier_du_code().join(crate::import::bulletin_lu::FICHIER_TABLE);
        if let Ok(t) = socle::fichiers::json_en_cache(&chemin) {
            parcourir(&t, &mut v);
        }
        v
    })
}

/// LE MASQUE : un mot du vocabulaire et un code de rubrique DSN restent ; toute autre lettre devient `X`, tout chiffre
/// `9` ; les espaces, la ponctuation et les retours a la ligne restent a leur place.
pub fn masquer(texte: &str) -> String {
    masquer_avec(texte, vocabulaire())
}

fn masquer_avec(texte: &str, vocabulaire: &HashSet<String>) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut fin = 0;
    for m in jeton().find_iter(texte) {
        sortie.push_str(&texte[fin..m.start()]);
        let mot = m.as_str();
        let garde = (mot.starts_with('S') && mot.contains(".G")) || (mot.chars().all(char::is_alphabetic) && vocabulaire.contains(&plie(mot)));
        if garde {
            sortie.push_str(mot);
        } else {
            sortie.extend(mot.chars().map(|c| if c.is_numeric() { '9' } else { 'X' }));
        }
        fin = m.end();
    }
    sortie.push_str(&texte[fin..]);
    sortie
}

/// La famille d'un document : la suite des mots du vocabulaire de sa premiere page, et le logiciel reconnu.
fn famille(premiere_page: &str, logiciel: &str) -> String {
    let voc = vocabulaire();
    let mots: Vec<String> = jeton().find_iter(premiere_page).map(|m| plie(m.as_str())).filter(|m| voc.contains(m)).collect();
    let mut h = Sha256::new();
    h.update(logiciel.as_bytes());
    h.update(b"\n");
    h.update(mots.join(" ").as_bytes());
    h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect()
}

/// `s[:n]` sur des points de code.
fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A chaque import : chaque document accepte et pas lu, un par famille, note masque au journal des incidents, puis
/// l'envoi ordinaire (`incidents::envoyer`, qui ne part que si l'envoi a l'editeur est actif sur ce poste). Ne leve
/// jamais : un incident qui ne s'ecrit pas ne change rien a l'import. Rend le nombre d'incidents notes.
pub(crate) fn noter_les_non_lus(ctx: &Contexte, lu: &DossierLu) -> usize {
    let vide: Vec<Value> = Vec::new();
    let cr = lu.ap.reponse.get("tri").and_then(|t| t.get("fichiers")).and_then(Value::as_array).unwrap_or(&vide);
    let deja: BTreeSet<String> = incidents::lire(&ctx.racine, None, false)
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.get("empreinte").and_then(Value::as_str).map(str::to_string))
        .collect();
    let mut vues: HashSet<String> = HashSet::new();
    let mut notes = 0;
    for (f, c) in lu.tri.fichiers.iter().zip(cr.iter()) {
        let sort = c.get("sort").and_then(Value::as_str).unwrap_or("");
        if f.sorte != Sorte::PdfTexte || !ib::non_repris(sort) {
            continue;
        }
        let Ok((pages, _)) = fabrique_pdf::lecture::texte(&f.disque.to_string_lossy()) else { continue };
        let pages_cr: Vec<&Value> = c.get("pages").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
        let logiciel = pages_cr.iter().find_map(|p| p.get("logiciel").and_then(Value::as_str)).unwrap_or("").to_string();
        let genre = format!("document non lu : famille {}", famille(pages.first().map_or("", String::as_str), &logiciel));
        let empreinte = incidents::empreinte(incidents::LOGICIEL, OU, &genre);
        if deja.contains(&empreinte) || !vues.insert(empreinte) {
            continue;
        }
        let forme: Vec<Value> = pages_cr
            .iter()
            .map(|p| {
                let n = p.get("page").and_then(Value::as_u64).unwrap_or(0);
                let l = f.pages.get((n as usize).saturating_sub(1));
                json!({
                    "page": n,
                    "sort": p.get("sort").cloned().unwrap_or(Value::Null),
                    "raison": masquer(p.get("raison").and_then(Value::as_str).unwrap_or("")),
                    "mots": l.map(|x| x.mots), "polices": l.map(|x| x.polices), "images": l.map(|x| x.images), "traits": l.map(|x| x.traits),
                })
            })
            .collect();
        let detail = json!({"sort": sort, "logiciel": logiciel, "pages": pages.len(), "forme": forme,
            "raison": masquer(c.get("raison").and_then(Value::as_str).unwrap_or(""))});
        let texte: Vec<String> = pages.iter().enumerate().map(|(k, p)| format!("--- page {} ---\n{}", k + 1, masquer(p.trim_end()))).collect();
        let trace = tete(&texte.join("\n"), TRACE_MAX);
        let resume = format!("Import : un document accepté n'a pas été lu (« {} »). Sa forme, masquée.", sort.replace('_', " "));
        let note = incidents::Note {
            nature: incidents::LOGICIEL,
            resume: &resume,
            ou: OU,
            quoi: &genre,
            detail,
            trace: Some(&trace),
            sans_contexte: true,
            ..Default::default()
        };
        if incidents::noter(&ctx.racine, &note).is_some() {
            notes += 1;
        }
    }
    if notes > 0 && incidents::envoi_actif(&ctx.racine) {
        incidents::envoyer(&ctx.racine);
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LE MASQUE NE LAISSE PASSER AUCUN CHIFFRE NI AUCUN MOT HORS DU VOCABULAIRE : un nom, un NIR, un montant, une
    /// adresse deviennent leur forme ; un libelle et un code de rubrique DSN restent a leur place.
    #[test]
    fn le_masque_ne_laisse_passer_que_le_vocabulaire() {
        let voc: HashSet<String> = ["SALAIRE", "BRUT", "NOM", "CACHETS"].iter().map(|m| m.to_string()).collect();
        let page = "Nom : DUPONT Élodie  NIR 2 85 07 75 115 042 83\nSalaire brut   1 234,56 €  Cachets 3\n12 rue des Lilas 75011 Paris S21.G00.40.001";
        let m = masquer_avec(page, &voc);
        assert_eq!(
            m,
            "Nom : XXXXXX XXXXXX  XXX 9 99 99 99 999 999 99\nSalaire brut   9 999,99 €  Cachets 9\n99 XXX XXX XXXXX 99999 XXXXX S21.G00.40.001"
        );
        let sans_codes = m.replace("S21.G00.40.001", "");
        assert!(!sans_codes.chars().any(|c| c.is_numeric() && c != '9'), "{}", m);
        for mot in jeton().find_iter(&sans_codes) {
            let w = mot.as_str();
            assert!(voc.contains(&plie(w)) || w.chars().all(|c| c == 'X' || c == '9'), "{} a passe le masque", w);
        }
    }
}
