//! DPAE (ex-DUE) -- Declaration Prealable A l'Embauche (`moteur/dpae.py`).
//!
//! Format teletransmission (Format 120) : enregistrements a largeur fixe de
//! 990 caracteres, saut de ligne compris (989 + LF), encodage latin-1. Le
//! fichier se depose sur net-entreprises.fr (rubrique DPAE) des que l'engagement
//! est valide, et au plus tard avant le debut du travail.
//!
//! Comme pour l'AEM, on part d'un enregistrement reel
//! (`gabarits/due_tel_gabarit.txt`) et on ne remplace que les zones variables.
//!
//! Le Python (`moteur/dpae.py`) ne garde que ses portes : il envoie ce que la
//! DPAE lit de chaque embauche -- la fiche du salarie, le debut et la fin de
//! l'engagement, sa nature -- et recoit les octets. Les refus gardent les
//! phrases du Python ; une cle absente d'une fiche (un `KeyError` du Python)
//! est rendue comme lui, `'cle'`.
//!
//! Deux commandes (`service`) : `paie dpae` (enregistrements, contenu,
//! ecriture du fichier et de son .zip, recapitulatif) et `paie dpae-lire` (les
//! lecteurs : `relire`, `lire_certificat`, `meme_declaration`).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use droit::natures;
use socle::date::{d, en_texte};
use socle::fichiers::{dossier_du_code, json_en_cache, modele_de, repr, repr_valeur, vrai};
#[cfg(test)]
use socle::fichiers::MARQUEUR_COPIE;
use crate::ntt;
use socle::num::Resultat;
use crate::usages::strip;

// Ce que `unicodedata.normalize("NFKD", c).encode("ascii", "ignore")` laisse
// de chaque caractere (ecrit par `paie generer-tables`, `generer_tables.rs`, depuis la base Unicode deposee).
#[path = "nfkd_ascii.rs"]
mod nfkd_ascii;

pub const LONGUEUR: usize = 990;

/// L'heure d'embauche portee par la DPAE quand personne n'en dit d'autre.
/// 08h00 (<< l'heure d'embauche de base doit etre 0800, pas 1900, et pas
/// besoin de poser la question lors de l'envoi >>). L'interface ne la demande
/// plus ; la ligne de commande garde `--heure`.
pub const HEURE_EMBAUCHE: &str = "0800";

/// POSITIONS 0-BASED, [debut, fin). Le dessin d'enregistrement officiel
/// (Format 120, `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.pdf`) compte a
/// partir de 1 : sa position 122 est notre 121.
///
/// LE PIEGE DE L'HORODATAGE, trouve le 12/09/2026 apres quatre refus
/// << Code URSSAF invalide >>. Le gabarit de CoolPaie porte `28082026` `1512`
/// `11` `7...` puis le SIRET, et on y avait lu JJMMAAAAHHMMSS puis le SIRET.
/// Faux : le dessin dit D_CREATION (122-129, JJMMAAAA), H_CREATION (130-133,
/// HHMM), puis **C_URSSAF (134-136)** -- le code de l'Urssaf receptrice, 117
/// pour l'Ile-de-France -- puis le SIRET a 137. `151211` n'etait pas
/// 15h12m11s : c'etait 15h12 suivi de `11`, les deux premiers chiffres de
/// 117. En ecrivant les secondes, chaque DPAE portait un code Urssaf fait de
/// ses secondes d'envoi : `597`, `427`... Elle ne passait que si l'on
/// deposait a la onzieme seconde.
///
/// Et `912` en 65-72, ce n'est pas un code Urssaf : c'est N_LG_MESS, la
/// longueur du message hors entete (990 - 77 - 1). Il ne bouge jamais.
///
/// C_TYPE_CONTRAT : 1 CDD, 2 CDI, 3 CTT -- la DPAE n'en connait que trois, la
/// ou la DSN en a une vingtaine. Il valait `1` en dur dans le gabarit, et
/// c'etait juste tant que la compagnie n'embauchait que des intermittents :
/// un CDI serait parti declare a duree determinee, sans terme a mettre dans
/// la case suivante. La valeur vient maintenant de la nature du contrat
/// (`donnees/contrats-base.json`).
pub const CHAMPS: [(&str, usize, usize); 14] = [
    ("horodatage", 121, 133),      // JJMMAAAAHHMM, sans secondes
    ("code_urssaf", 133, 136),     // C_URSSAF, 3 chiffres, annexe II.1
    ("nom", 283, 315),
    ("prenom", 315, 347),
    ("nir", 347, 360),
    ("date_naissance", 360, 368),  // JJMMAAAA
    ("lieu_naissance", 368, 412),
    ("date_embauche", 412, 420),   // JJMMAAAA
    ("heure_embauche", 420, 424),  // HHMM
    ("sexe", 730, 731),            // M / F
    ("dept_naissance", 827, 829),
    ("type_contrat", 864, 865),
    ("date_fin", 865, 873),        // JJMMAAAA, fin prevue du CDD
    ("jours_essai", 884, 887),     // N_JJ_ESSAI, la periode d'essai en jours
];

/// L'EMPLOYEUR (24/09/2026). Ces zones restaient celles du gabarit -- un
/// enregistrement REEL de la compagnie du poste --, et toute autre compagnie
/// aurait declare ses embauches sous le SIRET, la raison sociale, l'adresse et
/// le telephone de celle-la. Releve par les compagnies fictives. Elles
/// viennent maintenant de la fiche entreprise (`employeur`), positions du
/// dessin Format 120 v5.3, categories II et III, comptees a partir de 0.
pub const EMPLOYEUR: [(&str, usize, usize); 10] = [
    ("siret", 136, 150),            // N_SIRET
    ("raison_sociale", 151, 183),   // L_RAISON_SOC_1
    ("adresse", 187, 219),          // L_ADR_EMP_1
    ("adresse_2", 219, 251),        // L_ADR_EMP_2
    ("code_postal", 251, 256),      // C_POSTAL_EMP
    ("commune", 256, 283),          // L_BUR_DIST_EMP
    ("raison_sociale_2", 509, 541), // L_RAISON_SOC_2
    ("tel", 541, 552),              // N_TEL_EMP
    ("naf", 660, 665),              // C_NAF_NN
    ("medecine_travail", 937, 947), // C_MT_DCL, cadre a droite comme le gabarit
];

fn champ(nom: &str) -> (usize, usize) {
    let (_, a, b) = CHAMPS.iter().find(|(n, _, _)| *n == nom).expect("zone de la DPAE");
    (*a, *b)
}

fn zone_employeur(nom: &str) -> (usize, usize) {
    let (_, a, b) = EMPLOYEUR.iter().find(|(n, _, _)| *n == nom).expect("zone employeur de la DPAE");
    (*a, *b)
}

// ------------------------------------------------------------- les textes

/// `str(v or "")`.
fn chaine(v: Option<&Value>) -> String {
    match v {
        Some(v) if vrai(v) => en_texte(v),
        _ => String::new(),
    }
}

/// `s[cle]` : le `KeyError` du Python s'ecrit `'cle'`.
fn cle<'a>(s: &'a Value, k: &str) -> Resultat<&'a Value> {
    s.get(k).ok_or_else(|| socle::fichiers::absent(k))
}

/// `s.ljust(n)` : complete a droite avec des espaces, ne tronque pas.
fn ljust(s: &str, n: usize) -> String {
    let mut o = s.to_string();
    for _ in s.chars().count()..n {
        o.push(' ');
    }
    o
}

/// `s.rjust(n)`.
fn rjust(s: &str, n: usize) -> String {
    let mut o = String::new();
    for _ in s.chars().count()..n {
        o.push(' ');
    }
    o.push_str(s);
    o
}

/// `s[:n]`, en caracteres.
fn debut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Les caracteres que le Format 120 admet : A-Z, 0-9, espace, - et &.
/// Le texte passe en NFKD, ne garde que l'ASCII, se met en majuscules ; le
/// reste devient une espace.
pub fn normaliser(v: &str) -> String {
    ascii_nfkd(v)
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '&' { c } else { ' ' })
        .collect()
}

/// `unicodedata.normalize("NFKD", v).encode("ascii", "ignore").decode("ascii")`.
pub fn ascii_nfkd(v: &str) -> String {
    let mut ascii = String::new();
    for c in v.chars() {
        if c.is_ascii() {
            ascii.push(c);
            continue;
        }
        let cp = c as u32;
        if let Some((a, _, t, avance)) = nfkd_ascii::TABLE
            .binary_search_by(|(a, b, _, _)| {
                if cp < *a {
                    std::cmp::Ordering::Greater
                } else if cp > *b {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|i| nfkd_ascii::TABLE[i])
        {
            if avance {
                let base = t.chars().next().unwrap_or(' ') as u32;
                ascii.push(char::from_u32(base + (cp - a)).unwrap_or(' '));
            } else {
                ascii.push_str(t);
            }
        }
    }
    ascii
}

/// `(v or "").upper()[:largeur].ljust(largeur)`.
fn txt(v: &Value, largeur: usize) -> String {
    let t = if vrai(v) { en_texte(v).to_uppercase() } else { String::new() };
    ljust(&debut(&t, largeur), largeur)
}

/// `c.isdigit()` de Python.
fn est_chiffre(c: char) -> bool {
    let n = c as u32;
    nfkd_ascii::CHIFFRES.binary_search_by(|(a, b)| {
        if n < *a {
            std::cmp::Ordering::Greater
        } else if n > *b {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    }).is_ok()
}

/// `t.isdigit()` : non vide, et rien que des chiffres.
fn tout_chiffres(t: &str) -> bool {
    !t.is_empty() && t.chars().all(est_chiffre)
}

/// Les chiffres de `str(tel)`.
fn chiffres(t: &str) -> String {
    t.chars().filter(|&c| est_chiffre(c)).collect()
}

// ------------------------------------------------------- le gabarit

/// Ou chercher le gabarit : `defaut` quand l'appelant n'a pas de fiche
/// entreprise (le gabarit trouve a l'import par le Python), `poste` (le
/// dossier `gabarits/` du poste) sinon.
#[derive(Debug, Clone)]
pub struct Chemins {
    pub defaut: PathBuf,
    pub poste: PathBuf,
}

/// Le nom du gabarit livre.
const NOM_GABARIT: &str = "due_tel_gabarit.txt";

/// Les 990 premiers caracteres du gabarit, lus comme `Path.read_text(
/// encoding="latin-1")` (les fins de ligne universelles).
fn gabarit_lu(ent: Option<&Value>, chemins: &Chemins) -> Resultat<Vec<char>> {
    // Le gabarit de la compagnie s'il en a un, celui livre avec le logiciel
    // sinon (`donnees/modeles/`, un enregistrement neutre) : il ne donne plus
    // que l'entete constante, tout le reste est reecrit plus bas.
    let chemin = match ent {
        Some(e) => modele_de(NOM_GABARIT, e, &chemins.poste),
        None => chemins.defaut.clone(),
    };
    let octets = std::fs::read(&chemin).map_err(|e| socle::fichiers::panne_fichier(&e, &chemin))?;
    let texte: String = octets.iter().map(|&o| o as char).collect();
    let texte = texte.replace("\r\n", "\n").replace('\r', "\n");
    let gab: Vec<char> = texte.chars().take(LONGUEUR).collect();
    if gab.len() != LONGUEUR {
        return Err(format!("gabarit DUE corrompu ({} != {})", gab.len(), LONGUEUR));
    }
    Ok(gab)
}

/// Ce que la DPAE refuserait de l'employeur de cette fiche, sans rien ecrire : `(champ de la fiche, phrase du refus)`,
/// TOUS d'un coup (un SIRET faux n'empeche pas de dire que le code de medecine du travail manque aussi). La liste « Pour
/// demarrer » s'en sert, champ par champ : la meme regle que le depot (`employeur`), au meme endroit.
pub fn employeur_manques(ent: &Value, gabarits: &Path) -> Resultat<Vec<(&'static str, String)>> {
    let chemins = Chemins { defaut: dossier_du_code().join("modeles").join(NOM_GABARIT), poste: gabarits.to_path_buf() };
    let gabarit = gabarit_lu(Some(ent), &chemins)?;
    let mut out = Vec::new();
    if let Err(m) = siret_de(ent) {
        out.push(("siret", m));
    }
    if let Err(m) = medecine_de(ent, &gabarit) {
        out.push(("medecine_travail", m));
    }
    Ok(out)
}

/// Le SIRET de la fiche, 14 chiffres, ou le refus de la DPAE.
fn siret_de(ent: &Value) -> Resultat<String> {
    let siret = chaine(ent.get("siret"));
    let siret = strip(&siret).to_string();
    if siret.chars().count() != 14 || !tout_chiffres(&siret) {
        return Err(format!("DPAE : le SIRET de la fiche entreprise ({}) ne fait pas 14 chiffres.", repr(&siret)));
    }
    Ok(siret)
}

/// Le code du centre de medecine du travail (C_MT_DCL, obligatoire hors travail temporaire) : `medecine_travail` ; faute
/// de lui, il n'est repris du gabarit QUE si le gabarit est un enregistrement de la meme compagnie (meme SIRET) -- jamais
/// celui d'une autre.
fn medecine_de(ent: &Value, gabarit: &[char]) -> Resultat<String> {
    let mt = chaine(ent.get("medecine_travail"));
    let mt = strip(&mt).to_string();
    if !mt.is_empty() {
        return Ok(mt);
    }
    let (a, b) = zone_employeur("siret");
    let du_gabarit: String = gabarit[a..b].iter().collect();
    let siret = strip(&chaine(ent.get("siret"))).to_string();
    if du_gabarit != siret || siret.is_empty() {
        return Err("DPAE : la fiche entreprise ne dit pas le code du centre de medecine du travail (`medecine_travail`, C_MT_DCL, obligatoire). Il se lit sur l'adhesion au service de sante au travail ; il ne se devine pas, et celui du gabarit est celui d'une autre compagnie.".to_string());
    }
    let (ma, mb) = zone_employeur("medecine_travail");
    let du_gabarit: String = gabarit[ma..mb].iter().collect();
    Ok(strip(&du_gabarit).to_string())
}

// -------------------------------------------------------- l'employeur

/// Les zones employeur de la DPAE, lues dans la fiche `ent` (le SIRET : `siret_de` ; la medecine du travail :
/// `medecine_de`).
pub fn employeur(ent: &Value, gabarit: &[char]) -> Resultat<Vec<(&'static str, String)>> {
    let siret = siret_de(ent)?;
    let mt = medecine_de(ent, gabarit)?;
    // `(ent.get("declarant_dsn") or {}).get("tel") or ent.get("tel") or
    // (ent.get("signataire") or {}).get("tel") or ""`
    let tel_de = |k: &str| -> Option<&Value> { ent.get(k).filter(|v| vrai(v)).and_then(|v| v.get("tel")).filter(|v| vrai(v)) };
    let tel = tel_de("declarant_dsn")
        .or_else(|| ent.get("tel").filter(|v| vrai(v)))
        .or_else(|| tel_de("signataire"));
    let tel = chiffres(&chaine(tel));
    Ok(vec![
        ("siret", siret),
        ("raison_sociale", ljust(&debut(&normaliser(&chaine(ent.get("raison_sociale"))), 32), 32)),
        ("adresse", ljust(&debut(&normaliser(&chaine(ent.get("adresse"))), 32), 32)),
        ("adresse_2", ljust(&debut(&normaliser(&chaine(ent.get("complement_adresse"))), 32), 32)),
        ("code_postal", ljust(&debut(&chaine(ent.get("code_postal")), 5), 5)),
        ("commune", ljust(&debut(&normaliser(&chaine(ent.get("commune"))), 27), 27)),
        ("raison_sociale_2", " ".repeat(32)),
        ("tel", ljust(&debut(&tel, 11), 11)),
        ("naf", ljust(&debut(&chaine(ent.get("naf")), 5), 5)),
        ("medecine_travail", rjust(&debut(&mt, 10), 10)),
    ])
}

// ------------------------------------------------------- l'Urssaf

/// `urssaf.existe` : ce code est-il celui d'un organisme de recouvrement
/// connu (`urssaf-dpae-base.json`) ?
fn urssaf_existe(code: &str) -> Resultat<bool> {
    let base = json_en_cache(&dossier_du_code().join("urssaf-dpae-base.json"))?;
    let c = strip(code);
    Ok(base
        .get("organismes")
        .and_then(Value::as_array)
        .map_or(false, |o| o.iter().any(|x| x.get("code").map_or(false, |v| en_texte(v) == c))))
}

// -------------------------------------------- ce que la DPAE lit d'une embauche

/// Ce que la DPAE lit d'une embauche (`operations_paies.Embauche` ou `Paie`) :
/// la fiche du salarie, le debut et la fin de l'engagement, sa nature, et --
/// pour le recapitulatif seulement -- le spectacle.
#[derive(Debug, Clone)]
pub struct Embauche {
    pub salarie: Value,
    pub debut: Value,
    pub fin: Value,
    /// La cle de nature ; `None` : la nature par defaut.
    pub nature: Option<String>,
    pub spectacle: Value,
}

impl Embauche {
    pub fn lire(v: &Value) -> Resultat<Embauche> {
        let salarie = v.get("salarie").filter(|s| s.is_object()).cloned().ok_or("demande illisible : une embauche sans `salarie`")?;
        Ok(Embauche {
            salarie,
            debut: v.get("debut").cloned().unwrap_or(Value::Null),
            fin: v.get("fin").cloned().unwrap_or(Value::Null),
            nature: v.get("nature").filter(|n| !n.is_null()).map(en_texte),
            spectacle: v.get("spectacle").cloned().unwrap_or(Value::Null),
        })
    }
}

/// Les parametres communs d'une ecriture.
pub struct Parametres<'a> {
    /// JJMMAAAAHHMM : la minute de l'ecriture, donnee par le Python (heure de Paris).
    pub horodatage: &'a str,
    pub heure_embauche: String,
    /// Le code Urssaf voulu, tel que l'appelant l'a donne (`""` : celui du gabarit).
    pub code_urssaf: Value,
    pub ent: Option<&'a Value>,
    pub chemins: &'a Chemins,
}

fn poser(buf: &mut [char], nom: &str, valeur: &str) -> Resultat<()> {
    let (a, b) = champ(nom);
    let n = valeur.chars().count();
    if n != b - a {
        return Err(format!("{}: {} fait {} car., attendu {}", nom, repr(valeur), n, b - a));
    }
    for (i, c) in valeur.chars().enumerate() {
        buf[a + i] = c;
    }
    Ok(())
}

fn ecrire_zone(buf: &mut [char], a: usize, b: usize, valeur: &str) {
    let v: Vec<char> = valeur.chars().collect();
    buf[a..b].copy_from_slice(&v[..b - a]);
}

/// Un enregistrement de 990 caracteres.
pub fn enregistrement(e: &Embauche, p: &Parametres, gabarit: &[char]) -> Resultat<String> {
    let mut buf: Vec<char> = gabarit.to_vec();
    if let Some(ent) = p.ent {
        for (nom, v) in employeur(ent, gabarit)? {
            let (a, b) = zone_employeur(nom);
            ecrire_zone(&mut buf, a, b, &v);
        }
    }
    if vrai(&p.code_urssaf) {
        let (a, b) = champ("code_urssaf");
        let brut = en_texte(&p.code_urssaf);
        let v = strip(&brut);
        if v.chars().count() != b - a || !tout_chiffres(v) {
            return Err(format!(
                "code_urssaf: {}, attendu {} chiffres (annexe II.1)",
                repr_valeur(&p.code_urssaf),
                b - a
            ));
        }
        // ET LE CODE DOIT EXISTER (22/09/2026). Trois chiffres bien formes
        // suffisaient ; le point de depot, lui, repond << Code URSSAF invalide
        // -- n'appartenant pas a la liste des Codes Urssaf >>, des jours plus
        // tard, sur une embauche qui n'est alors pas declaree. La liste est
        // celle de la Table des Urssaf (voir `moteur/urssaf.py`), relevee et
        // datee.
        if !urssaf_existe(v)? {
            return Err(format!(
                "code_urssaf: {} n'est pas un organisme de recouvrement connu.\n  Le point de depot le refuserait (\u{ab} Code URSSAF invalide \u{bb}). Le code se lit sur les bordereaux Urssaf de la compagnie.",
                repr_valeur(&p.code_urssaf)
            ));
        }
        ecrire_zone(&mut buf, a, b, v);
    }

    let s = &e.salarie;
    // LE SEXE VIENT DU NIR QUAND IL Y EN A UN, de la fiche sinon : c'est la
    // meme lecture que pour le numero technique temporaire de la DSN
    // (`ntt.rs`), et il n'y en a qu'une dans le logiciel.
    let sexe = match ntt::sexe(s).as_str() {
        "1" => "M",
        "2" => "F",
        _ => "",
    };
    if sexe.is_empty() {
        return Err(format!(
            "La DPAE de {} {} ne peut pas partir : elle declare le sexe (zone SEXE, position 731), et la fiche ne le dit ni par le NIR ni par sa case `sexe`.",
            en_texte(cle(s, "nom")?),
            en_texte(cle(s, "prenom")?)
        ));
    }
    poser(&mut buf, "horodatage", p.horodatage)?;
    poser(&mut buf, "nom", &txt(cle(s, "nom")?, 32))?;
    poser(&mut buf, "prenom", &txt(cle(s, "prenom")?, 32))?;
    // LE NIR EST FACULTATIF ICI, et c'est tout l'interet : une DPAE sans
    // numero de securite sociale DEMANDE l'immatriculation du salarie
    // (formalite IMMA du Format 120, << demande d'immatriculation d'un
    // salarie depourvu de numero de securite sociale >>, transmise a la
    // CPAM). C'est par elle que le NIR arrive -- et, en attendant, la DSN
    // declare l'individu sous son numero technique temporaire. La zone reste
    // donc a blanc, sur ses 13 caracteres.
    poser(&mut buf, "nir", &ljust(&chaine(s.get("nir")), 13))?;
    poser(&mut buf, "date_naissance", &d(cle(s, "date_naissance")?)?.jjmmaaaa())?;
    poser(&mut buf, "lieu_naissance", &txt(cle(s, "lieu_naissance")?, 44))?;
    poser(&mut buf, "date_embauche", &d(&e.debut)?.jjmmaaaa())?;
    poser(&mut buf, "heure_embauche", &p.heure_embauche)?;
    poser(&mut buf, "sexe", sexe)?;
    poser(&mut buf, "dept_naissance", &en_texte(cle(s, "departement_naissance")?))?;
    let nat = nature_de(e)?;
    poser(&mut buf, "type_contrat", &nat.type_contrat_dpae())?;
    // UN CDI N'A PAS DE TERME, et la zone D_FIN_CDD reste a blanc : le
    // controle 46 (<< date de fin obligatoire >>) ne vise que les contrats de
    // type CDD. Un CDD sans fin, lui, est refuse ici plutot que par l'Urssaf.
    if nat.date_fin_dpae() {
        if !vrai(&e.fin) {
            return Err(format!(
                "{} : un contrat de nature '{}' doit dire sa date de fin -- la DPAE d'un CDD la porte (D_FIN_CDD), et l'Urssaf la refuse sans (code retour 46).",
                en_texte(cle(s, "nom")?),
                nat.cle
            ));
        }
        poser(&mut buf, "date_fin", &d(&e.fin)?.jjmmaaaa())?;
    } else {
        poser(&mut buf, "date_fin", &" ".repeat(8))?;
    }
    // LA PERIODE D'ESSAI, en chiffres ou rien : trois blancs valent le code
    // retour 99 (guide << API DPAE, principes de fonctionnement >>, p. 8-9).
    // Le contrat CDDU que le logiciel redige (`modeles/contrat-cddu.modele`)
    // n'en prevoit aucune : `000` est exact. Un CDI ou un CDD, le logiciel ne
    // redige pas le contrat et n'en connait pas l'essai : la zone reste a
    // blanc plutot que devinee, et la DPAE est enregistree en 99.
    let essai = if nat.cle == "cddu_spectacle" { "000".to_string() } else { " ".repeat(3) };
    poser(&mut buf, "jours_essai", &essai)?;
    Ok(buf.into_iter().collect())
}

/// Les enregistrements de ces embauches, dans l'ordre. Le gabarit n'est lu
/// qu'a la premiere (une liste vide ne lit rien).
pub fn enregistrements(embauches: &[Embauche], p: &Parametres) -> Resultat<Vec<String>> {
    let mut out = Vec::new();
    let mut gabarit: Option<Vec<char>> = None;
    for e in embauches {
        if gabarit.is_none() {
            gabarit = Some(gabarit_lu(p.ent, p.chemins)?);
        }
        out.push(enregistrement(e, p, gabarit.as_deref().unwrap_or(&[]))?);
    }
    Ok(out)
}

/// Le texte en latin-1, comme `str.encode("latin-1")` : un caractere hors de
/// la table donne la phrase de `UnicodeEncodeError`.
pub fn en_latin1(texte: &str) -> Resultat<Vec<u8>> {
    let c: Vec<char> = texte.chars().collect();
    if let Some(i) = c.iter().position(|&x| x as u32 > 255) {
        let mut j = i + 1;
        while j < c.len() && c[j] as u32 > 255 {
            j += 1;
        }
        let ech = |x: char| {
            let n = x as u32;
            if n <= 0xffff { format!("\\u{:04x}", n) } else { format!("\\U{:08x}", n) }
        };
        return Err(if j == i + 1 {
            format!(
                "'latin-1' codec can't encode character '{}' in position {}: ordinal not in range(256)",
                ech(c[i]),
                i
            )
        } else {
            format!("'latin-1' codec can't encode characters in position {}-{}: ordinal not in range(256)", i, j - 1)
        });
    }
    Ok(c.iter().map(|&x| x as u8).collect())
}

/// Les octets qu'`ecrire` poserait sur le disque, sans rien ecrire.
pub fn contenu(embauches: &[Embauche], p: &Parametres) -> Resultat<Vec<u8>> {
    en_latin1(&enregistrements(embauches, p)?.concat())
}

// ------------------------------------------------------------- le zip

/// Un .zip d'un seul fichier, compresse (deflate) : net-entreprises accepte
/// les deux, et CoolPaie deposait le zip. La date du fichier dans l'archive
/// est la minute de l'ecriture (JJMMAAAAHHMM) : le zip est le meme a chaque
/// ecriture du meme contenu.
pub fn zip(nom: &str, donnees: &[u8], horodatage: &str) -> Resultat<Vec<u8>> {
    use flate2::write::DeflateEncoder;
    use flate2::{Compression, Crc};
    use std::io::Write;

    let n = |a: usize, b: usize| horodatage.get(a..b).and_then(|t| t.parse::<u32>().ok());
    let (jour, mois, an, h, m) = (n(0, 2), n(2, 4), n(4, 8), n(8, 10), n(10, 12));
    let (date, heure) = match (jour, mois, an, h, m) {
        (Some(j), Some(mo), Some(a), Some(h), Some(m)) if a >= 1980 && a < 2108 => {
            (((a - 1980) << 9) | (mo << 5) | j, (h << 11) | (m << 5))
        }
        _ => ((0 << 9) | (1 << 5) | 1, 0),
    };
    let mut crc = Crc::new();
    crc.update(donnees);
    let somme = crc.sum();
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    enc.write_all(donnees).map_err(|e| e.to_string())?;
    let comprime = enc.finish().map_err(|e| e.to_string())?;
    let nom_octets = nom.as_bytes();
    let drapeau: u16 = if nom.is_ascii() { 0 } else { 0x800 };
    let u16_ = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let u32_ = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());

    let mut z = Vec::new();
    // l'en-tete local
    u32_(&mut z, 0x0403_4b50);
    u16_(&mut z, 20);
    u16_(&mut z, drapeau);
    u16_(&mut z, 8);
    u16_(&mut z, heure as u16);
    u16_(&mut z, date as u16);
    u32_(&mut z, somme);
    u32_(&mut z, comprime.len() as u32);
    u32_(&mut z, donnees.len() as u32);
    u16_(&mut z, nom_octets.len() as u16);
    u16_(&mut z, 0);
    z.extend_from_slice(nom_octets);
    z.extend_from_slice(&comprime);
    // le repertoire central
    let debut_repertoire = z.len() as u32;
    u32_(&mut z, 0x0201_4b50);
    u16_(&mut z, 20 | (3 << 8)); // cree sous Unix, comme Python
    u16_(&mut z, 20);
    u16_(&mut z, drapeau);
    u16_(&mut z, 8);
    u16_(&mut z, heure as u16);
    u16_(&mut z, date as u16);
    u32_(&mut z, somme);
    u32_(&mut z, comprime.len() as u32);
    u32_(&mut z, donnees.len() as u32);
    u16_(&mut z, nom_octets.len() as u16);
    u16_(&mut z, 0);
    u16_(&mut z, 0);
    u16_(&mut z, 0);
    u16_(&mut z, 0);
    u32_(&mut z, 0o100644 << 16);
    u32_(&mut z, 0);
    z.extend_from_slice(nom_octets);
    let taille_repertoire = z.len() as u32 - debut_repertoire;
    // la fin du repertoire
    u32_(&mut z, 0x0605_4b50);
    u16_(&mut z, 0);
    u16_(&mut z, 0);
    u16_(&mut z, 1);
    u16_(&mut z, 1);
    u32_(&mut z, taille_repertoire);
    u32_(&mut z, debut_repertoire);
    u16_(&mut z, 0);
    Ok(z)
}

/// Un enregistrement par contrat, groupes par salarie comme le faisait
/// CoolPaie. Ecrit aussi le .zip du meme nom : net-entreprises accepte les
/// deux, et CoolPaie deposait le zip.
pub fn ecrire(embauches: &[Embauche], chemin: &Path, p: &Parametres, zipper: bool) -> Resultat<()> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} : {}", parent.display(), e))?;
    }
    let octets = contenu(embauches, p)?;
    std::fs::write(chemin, &octets).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    if zipper {
        let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut cz = chemin.as_os_str().to_os_string();
        cz.push(".zip");
        std::fs::write(PathBuf::from(cz), zip(&nom, &octets, p.horodatage)?).map_err(|e| format!("{} : {}", chemin.display(), e))?;
    }
    Ok(())
}

/// Le nom du type Python d'une valeur JSON, pour la phrase d'un `TypeError`.
fn type_py(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// La nature de contrat de cette embauche. Une nature inconnue est un
/// `KeyError` en Python, dont `str()` rend le message entre guillemets.
fn nature_de(e: &Embauche) -> Resultat<std::sync::Arc<natures::Nature>> {
    natures::nature(e.nature.as_deref()).map_err(|m| repr(&m))
}

/// Texte a recopier si l'on prefere la saisie en ligne sur net-entreprises.
/// `heure_embauche` se coupe comme un texte (`heure_embauche[:2]`) : toute
/// autre valeur est le `TypeError` du Python.
pub fn recapitulatif(embauches: &[Embauche], heure_embauche: &Value) -> Resultat<String> {
    let heure_embauche = match heure_embauche {
        Value::String(t) => t.as_str(),
        autre => return Err(format!("heure d'embauche : {}", socle::fichiers::attendu("un texte", type_py(autre)))),
    };
    let h: Vec<char> = heure_embauche.chars().collect();
    let heure = format!(
        "{}h{}",
        h.iter().take(2).collect::<String>(),
        h.iter().skip(2).collect::<String>()
    );
    let mut lignes: Vec<String> = vec!["DPAE - a declarer avant le debut de chaque contrat".into(), String::new()];
    for e in embauches {
        let s = &e.salarie;
        let nat = nature_de(e)?;
        // Un permanent n'a ni terme ni spectacle : la nature du contrat le
        // situe, comme sur le bulletin (`calcul.Paie.__str__`). Ce texte se
        // recopie a la main dans la saisie en ligne -- il ne doit rien dire
        // que le fichier ne dise.
        let terme = if vrai(&e.fin) {
            format!(", fin prevue le {}", d(&e.fin)?.jj_mm_aaaa())
        } else {
            ", sans terme".to_string()
        };
        let ou = if vrai(&e.spectacle) {
            format!(" - {}", en_texte(cle(&e.spectacle, "titre")?))
        } else {
            String::new()
        };
        lignes.push(format!(
            "{} {} - ne(e) le {} a {} ({})",
            en_texte(cle(s, "nom")?),
            en_texte(cle(s, "prenom")?),
            d(cle(s, "date_naissance")?)?.jj_mm_aaaa(),
            en_texte(cle(s, "lieu_naissance")?),
            en_texte(cle(s, "departement_naissance")?)
        ));
        lignes.push(if s.get("nir").map_or(false, vrai) {
            format!("  NIR {}{}", en_texte(cle(s, "nir")?), en_texte(cle(s, "nir_cle")?))
        } else {
            "  NIR : aucun -- cette DPAE vaut demande d'immatriculation (formalite IMMA)".to_string()
        });
        lignes.push(format!("  Embauche le {} a {}{}", d(&e.debut)?.jj_mm_aaaa(), heure, terme));
        lignes.push(format!("  Emploi : {} - {}{}", en_texte(cle(s, "emploi")?), nat.libelle, ou));
        lignes.push(String::new());
    }
    Ok(lignes.join("\n"))
}

// ---------------------------------------------------------- les lecteurs

/// Les enregistrements d'un flux DPAE, 990 caracteres chacun.
///
/// Le Format 120 dit << 990 en incluant le saut de ligne >> : 989 caracteres
/// puis LF. C'est ce que porte le gabarit de CoolPaie, ce qu'on emet, et ce
/// que l'Urssaf renvoie dans le certificat de conformite (nature 41). On a
/// longtemps decrit ce fichier comme << colle bout a bout sans separateur >> :
/// le separateur est la, c'est le 990e caractere. Un CRLF ou un flux
/// reellement colle se lisent aussi ; le decoupage se fait sur le LF quand
/// chaque morceau fait 989 ou 990, en largeur fixe sinon.
pub fn lire_enregistrements(contenu: &str) -> Vec<Vec<char>> {
    let b = contenu.replace("\r\n", "\n");
    let morceaux: Vec<&str> = b.split('\n').filter(|l| !strip(l).is_empty()).collect();
    if !morceaux.is_empty() && morceaux.iter().all(|l| matches!(l.chars().count(), 989 | 990)) {
        return morceaux
            .iter()
            .map(|l| {
                let mut v: Vec<char> = l.chars().collect();
                v.resize(LONGUEUR, ' ');
                v
            })
            .collect();
    }
    let c: Vec<char> = b.chars().collect();
    (0..c.len() / LONGUEUR).map(|i| c[i * LONGUEUR..(i + 1) * LONGUEUR].to_vec()).collect()
}

fn tranche(r: &[char], a: usize, b: usize) -> String {
    r[a..b].iter().collect()
}

/// `dpae.relire` : chaque enregistrement par ses zones (`CHAMPS`).
pub fn relire(contenu: &str) -> Vec<Vec<(&'static str, String)>> {
    lire_enregistrements(contenu)
        .iter()
        .map(|r| CHAMPS.iter().map(|(k, a, b)| (*k, tranche(r, *a, *b))).collect())
        .collect()
}

/// Deux flux DPAE qui declarent la meme chose, a l'horodatage pres.
///
/// L'horodatage (D_CREATION, H_CREATION) est la minute ou le fichier a ete
/// ecrit : deux ecritures des memes embauches a deux minutes d'ecart ne
/// different que par lui. C'est ce qui permet de reprendre le fichier deja
/// ecrit -- et deja REPETE en test -- au lieu d'en poser un neuf, que la
/// repetition ne couvrirait plus (25/09/2026 : on ecrivait, testait, revenait,
/// reecrivait, et le reel redemandait un test sans fin).
pub fn meme_declaration(a: &str, b: &str) -> bool {
    let (x, y) = (lire_enregistrements(a), lire_enregistrements(b));
    if x.len() != y.len() || x.is_empty() {
        return false;
    }
    let (d0, f0) = champ("horodatage");
    x.iter().zip(&y).all(|(u, v)| u[..d0] == v[..d0] && u[f0..] == v[f0..])
}

// ----------------------------------------------- ce que l'Urssaf repond
// Les codes retour du Format 120 (onglet << Codes retour >>, v5.3 du
// 13/06/2013), recopies du PDF le 13/09/2026. L'Urssaf les rend a deux
// endroits : dans le bilan de conformite (nature 11, XML, <code>02</code>),
// et dans le certificat de conformite (nature 41), qui est NOTRE
// enregistrement renvoye avec ses zones de retour remplies.

/// C_RET_AR_LOT, 76-77 : le traitement du message.
pub const CODES_RET_AR_LOT: [(&str, &str); 3] =
    [("00", "Correct"), ("11", "Code version incorrect"), ("15", "Longueur du message incorrecte")];

/// C_RETOUR_AR, 430-431 : chaque DPAE.
pub const CODES_RETOUR_AR: [(&str, &str); 26] = [
    ("00", "DPAE enregistrée"),
    ("01", "SIRET invalide - syntaxiquement incorrect"),
    ("02", "Code URSSAF invalide - n'appartenant pas à la liste des codes Urssaf"),
    ("03", "Code APE invalide - n'appartenant pas à la liste des codes APE (NAF)"),
    ("05", "Nom ou raison sociale invalide - non renseignée ou caractères interdits"),
    ("06", "Adresse invalide - non renseignée"),
    ("07", "Code postal invalide - ne comportant pas 5 caractères numériques"),
    ("08", "Commune invalide - non renseignée"),
    ("11", "Code origine application incorrect"),
    ("12", "Code origine saisie invalide - non renseigné"),
    ("31", "Nom salarié invalide - non renseigné"),
    ("32", "Prénom salarié invalide - non renseigné"),
    ("33", "Date de naissance salarié invalide - invraisemblable ou non renseignée"),
    ("34", "Lieu de naissance invalide - non renseigné"),
    ("35", "Date et/ou heure d'embauche invalide - invraisemblable ou non renseignée"),
    ("38", "Numéro de sécurité sociale invalide - invraisemblable, département inexistant…"),
    ("39", "Date de déclaration invalide - invraisemblable ou non renseignée"),
    ("40", "Heure de déclaration invalide - invraisemblable ou non renseignée"),
    ("43", "Sexe du salarié invalide - différent de M ou F, ou non renseigné"),
    ("44", "Type de contrat invalide - différent de 1, 2 ou 3, ou non renseigné"),
    ("45", "Code NAF invalide pour le type de contrat CTT"),
    ("46", "Date de fin de CDD obligatoire pour un CDD, ou invalide (antérieure à l'embauche…)"),
    ("47", "Département de naissance invalide - non renseigné"),
    ("50", "Code médecine du travail invalide - non renseigné"),
    ("98", "DPAE déjà existante (nom, prénom, date de naissance, date et heure d'embauche, SIRET)"),
    ("99", "Autre erreur (données facultatives)"),
];

// ENREGISTREE, EN PRATIQUE : `00` ou `99` avec une reference de dossier. Le
// `99` (<< autre erreur, donnees facultatives >>) enregistre la DPAE mais
// signale une donnee facultative mal formee. Celui que portaient tous les
// certificats depuis juin 2026, CoolPaie compris, venait de la periode
// d'essai (N_JJ_ESSAI) laissee a blanc : un CDDU l'ecrit `000` et rend `00`.
// Un CDI ou un CDD, dont l'essai n'est pas connu, reste en 99.
// Le `98` dit qu'une DPAE identique existait deja : rien n'a ete cree, et les
// formalites restent a N -- vu le 12/09/2026 sur le second TEST des memes
// embauches. L'embauche est declaree quand meme, par la premiere.
pub const CODES_ENREGISTREE: [&str; 2] = ["00", "99"];
pub const CODE_DEJA_EXISTANTE: &str = "98";

/// 0-based, [debut, fin). Positions du Format 120 moins un.
pub const ZONES_RETOUR: [(&str, usize, usize); 7] = [
    ("ret_ar_lot", 75, 77), // C_RET_AR_LOT
    ("dossier", 424, 429),  // R_DOSSIER, la reference sous laquelle la DPAE est enregistree
    ("retour_ar", 429, 431), // C_RETOUR_AR
    ("certif", 970, 971),   // I_CERTIF : donnees transmises pour la DADS (CRAM)
    ("imma", 973, 974),     // I_IMMA : demande d'immatriculation (CPAM)
    ("mt", 974, 975),       // I_MT : medecine du travail informee
    ("pmf5", 975, 976),     // I_PMF5 : premiere embauche dans l'etablissement (CFE-Urssaf)
];
pub const FORMALITES: [(&str, &str); 4] = [("certif", "CERTIF"), ("imma", "IMMA"), ("mt", "MT"), ("pmf5", "PMF5")];

fn zone_retour(nom: &str) -> (usize, usize) {
    let (_, a, b) = ZONES_RETOUR.iter().find(|(n, _, _)| *n == nom).expect("zone de retour de la DPAE");
    (*a, *b)
}

/// Le certificat de conformite (nature 41), DPAE par DPAE.
///
/// Rend, par enregistrement : `nom`, `prenom`, `nir`, `embauche`
/// (AAAA-MM-JJ), `dossier` (R_DOSSIER), `code` et `libelle` (C_RETOUR_AR,
/// Format 120), `enregistree` (00 ou 99 avec un dossier), `deja_existante`
/// (98), `formalites` (celles a O : CERTIF, IMMA, MT, PMF5), `lot`
/// (C_RET_AR_LOT).
pub fn lire_certificat(contenu: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for r in lire_enregistrements(contenu) {
        let z = |k: &str| {
            let (a, b) = zone_retour(k);
            strip(&tranche(&r, a, b)).to_string()
        };
        let c = |k: &str| {
            let (a, b) = champ(k);
            strip(&tranche(&r, a, b)).to_string()
        };
        let emb = c("date_embauche");
        let code = if z("retour_ar").is_empty() { "00".to_string() } else { z("retour_ar") };
        let libelle = CODES_RETOUR_AR
            .iter()
            .find(|(k, _)| *k == code)
            .map_or_else(|| format!("code {} (inconnu du Format 120)", code), |(_, l)| l.to_string());
        let e: Vec<char> = emb.chars().collect();
        let embauche = if e.len() == 8 {
            format!(
                "{}-{}-{}",
                e[4..].iter().collect::<String>(),
                e[2..4].iter().collect::<String>(),
                e[..2].iter().collect::<String>()
            )
        } else {
            String::new()
        };
        let dossier = z("dossier");
        let lot = z("ret_ar_lot");
        let mut m = Map::new();
        m.insert("nom".into(), json!(c("nom")));
        m.insert("prenom".into(), json!(c("prenom")));
        m.insert("nir".into(), json!(c("nir")));
        m.insert("embauche".into(), json!(embauche));
        m.insert("dossier".into(), json!(dossier));
        m.insert("code".into(), json!(code));
        m.insert("libelle".into(), json!(libelle));
        let enregistree = CODES_ENREGISTREE.contains(&code.as_str()) && !dossier.is_empty();
        m.insert("enregistree".into(), json!(enregistree));
        m.insert("deja_existante".into(), json!(code == CODE_DEJA_EXISTANTE));
        let formalites: Vec<&str> = FORMALITES.iter().filter(|(k, _)| z(k) == "O").map(|(_, n)| *n).collect();
        m.insert("formalites".into(), json!(formalites));
        m.insert("lot".into(), json!(if lot.is_empty() { "00".to_string() } else { lot }));
        out.push(Value::Object(m));
    }
    out
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Le gabarit neutre livre avec le logiciel.
    fn neutre() -> Vec<char> {
        let octets = std::fs::read(dossier_du_code().join("modeles").join(NOM_GABARIT)).unwrap();
        octets.iter().map(|&o| o as char).collect::<String>().chars().take(LONGUEUR).collect()
    }

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("puppetpaie-dpae-{}-{}", std::process::id(), nom));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Un salarie inventé (le NIR porte `999`, la marque des numeros inventes).
    fn salarie() -> Value {
        json!({"nom": "Dupont", "prenom": "Alice", "nir": "2850375999001", "nir_cle": "42",
               "date_naissance": "1985-03-07", "lieu_naissance": "Tours", "departement_naissance": "37",
               "emploi": "Comedienne"})
    }

    fn embauche(s: Value, nature: &str, fin: Value) -> Embauche {
        Embauche {
            salarie: s,
            debut: json!("2026-10-01"),
            fin,
            nature: Some(nature.to_string()),
            spectacle: Value::Null,
        }
    }

    fn entreprise() -> Value {
        json!({"siret": "12345691500016", "raison_sociale": "Cie Fictive", "adresse": "1 rue Fictive",
               "code_postal": "45000", "commune": "Orléans", "naf": "9001Z", "medecine_travail": "MT42",
               "tel": "01 02 03 04 05"})
    }

    fn chemins() -> Chemins {
        Chemins {
            defaut: dossier_du_code().join("modeles").join(NOM_GABARIT),
            poste: std::env::temp_dir().join("puppetpaie-dpae-sans-poste"),
        }
    }

    fn parametres<'a>(ent: Option<&'a Value>, code: Value, chemins: &'a Chemins) -> Parametres<'a> {
        Parametres { horodatage: "031020261407", heure_embauche: "0800".into(), code_urssaf: code, ent, chemins }
    }

    fn enr(e: &Embauche, ent: Option<&Value>, code: Value) -> Resultat<String> {
        let c = chemins();
        enregistrements(std::slice::from_ref(e), &parametres(ent, code, &c)).map(|mut v| v.remove(0))
    }

    fn zone(texte: &str, nom: &str) -> String {
        let (a, b) = champ(nom);
        texte.chars().skip(a).take(b - a).collect()
    }

    // ---- ce que le Python testait de ses internes

    #[test]
    fn une_dpae_sans_medecine_du_travail_ne_reprend_pas_celle_d_un_autre() {
        let gab = neutre();
        let mut autre: Vec<char> = gab.clone();
        let (a, b) = zone_employeur("siret");
        autre.splice(a..b, "12345678900017".chars());
        let (ma, mb) = zone_employeur("medecine_travail");
        autre.splice(ma..mb, "     MT176".chars());
        let mut ent = json!({"siret": "12345691500016", "raison_sociale": "CIE"});
        let e = employeur(&ent, &autre).unwrap_err();
        assert!(e.contains("medecine du travail"), "{}", e);
        // Le gabarit de la MEME compagnie, lui, le prete.
        ent["siret"] = json!("12345678900017");
        let z = employeur(&ent, &autre).unwrap();
        assert_eq!(z.iter().find(|(k, _)| *k == "medecine_travail").unwrap().1.trim(), "MT176");
    }

    #[test]
    fn un_cdd_sans_terme_est_refuse_ici_et_non_par_l_urssaf() {
        let e = embauche(salarie(), "cdd", Value::Null);
        let m = enr(&e, None, json!("")).unwrap_err();
        assert!(m.contains("date de fin"), "{}", m);
        assert!(m.contains("'cdd'"), "{}", m);
        // Avec son terme, il part, type 1 (CDD) et date de fin dans la case.
        let e = embauche(salarie(), "cdd", json!("2026-12-31"));
        let t = enr(&e, None, json!("")).unwrap();
        assert_eq!(zone(&t, "type_contrat"), "1");
        assert_eq!(zone(&t, "date_fin"), "31122026");
    }

    #[test]
    fn un_cdi_est_declare_a_duree_indeterminee_sans_terme() {
        let e = embauche(salarie(), "cdi", Value::Null);
        let t = enr(&e, None, json!("")).unwrap();
        assert_eq!(zone(&t, "type_contrat"), "2");
        assert_eq!(zone(&t, "date_fin"), " ".repeat(8));
        // Une fin portee par un CDI n'est pas declaree : il n'a pas de terme.
        let e = embauche(salarie(), "cdi", json!("2026-12-31"));
        assert_eq!(zone(&enr(&e, None, json!("")).unwrap(), "date_fin"), " ".repeat(8));
    }

    #[test]
    fn un_cdd_d_usage_est_un_cdd_avec_terme() {
        let e = embauche(salarie(), "cddu_spectacle", json!("2026-10-05"));
        let t = enr(&e, None, json!("")).unwrap();
        assert_eq!(zone(&t, "type_contrat"), "1");
        assert_eq!(zone(&t, "date_fin"), "05102026");
        assert_eq!(t.chars().count(), LONGUEUR);
    }

    #[test]
    fn les_zones_du_salarie() {
        let e = embauche(salarie(), "cdi", Value::Null);
        let t = enr(&e, None, json!("117")).unwrap();
        assert_eq!(zone(&t, "horodatage"), "031020261407");
        assert_eq!(zone(&t, "code_urssaf"), "117");
        assert_eq!(zone(&t, "nom").trim_end(), "DUPONT");
        assert_eq!(zone(&t, "prenom").trim_end(), "ALICE");
        assert_eq!(zone(&t, "nir"), "2850375999001");
        assert_eq!(zone(&t, "date_naissance"), "07031985");
        assert_eq!(zone(&t, "lieu_naissance").trim_end(), "TOURS");
        assert_eq!(zone(&t, "date_embauche"), "01102026");
        assert_eq!(zone(&t, "heure_embauche"), "0800");
        assert_eq!(zone(&t, "sexe"), "F");
        assert_eq!(zone(&t, "dept_naissance"), "37");
    }

    #[test]
    fn sans_nir_la_zone_reste_a_blanc_et_le_sexe_vient_de_la_fiche() {
        let mut s = salarie();
        s["nir"] = json!("");
        s["sexe"] = json!("1");
        let t = enr(&embauche(s.clone(), "cdi", Value::Null), None, json!("")).unwrap();
        assert_eq!(zone(&t, "nir"), " ".repeat(13));
        assert_eq!(zone(&t, "sexe"), "M");
        // Ni NIR ni sexe : la DPAE ne part pas.
        s["sexe"] = json!("");
        let m = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap_err();
        assert!(m.contains("zone SEXE, position 731"), "{}", m);
    }

    #[test]
    fn le_code_urssaf_doit_etre_bien_forme_et_exister() {
        let e = embauche(salarie(), "cdi", Value::Null);
        let m = enr(&e, None, json!("11")).unwrap_err();
        assert!(m.contains("code_urssaf") && m.contains("'11'"), "{}", m);
        let m = enr(&e, None, json!("abc")).unwrap_err();
        assert!(m.contains("attendu 3 chiffres"), "{}", m);
        let m = enr(&e, None, json!("999")).unwrap_err();
        assert!(m.contains("code_urssaf") && m.contains("'999'"), "{}", m);
        assert!(enr(&e, None, json!(" 117 ")).is_ok());
        // Sans code, celui du gabarit reste.
        let t = enr(&e, None, json!("")).unwrap();
        assert_eq!(zone(&t, "code_urssaf"), neutre()[133..136].iter().collect::<String>());
    }

    #[test]
    fn l_employeur_vient_de_la_fiche_entreprise() {
        let ent = entreprise();
        let e = embauche(salarie(), "cdi", Value::Null);
        let t = enr(&e, Some(&ent), json!("117")).unwrap();
        let ch = |nom: &str| {
            let (a, b) = zone_employeur(nom);
            t.chars().skip(a).take(b - a).collect::<String>()
        };
        assert_eq!(ch("siret"), "12345691500016");
        assert_eq!(ch("raison_sociale").trim_end(), "CIE FICTIVE");
        assert_eq!(ch("adresse").trim_end(), "1 RUE FICTIVE");
        assert_eq!(ch("code_postal"), "45000");
        assert_eq!(ch("commune").trim_end(), "ORLEANS");
        assert_eq!(ch("tel"), "0102030405 ");
        assert_eq!(ch("naf"), "9001Z");
        assert_eq!(ch("medecine_travail"), "      MT42");
        // Un SIRET qui ne fait pas 14 chiffres : refus.
        let mut mauvais = entreprise();
        mauvais["siret"] = json!("1234");
        let m = enr(&e, Some(&mauvais), json!("117")).unwrap_err();
        assert!(m.contains("SIRET") && m.contains("'1234'"), "{}", m);
    }

    #[test]
    fn le_telephone_se_cherche_dans_le_declarant_puis_la_fiche_puis_le_signataire() {
        let g = neutre();
        let tel = |ent: Value| {
            let z = employeur(&ent, &g).unwrap();
            z.iter().find(|(k, _)| *k == "tel").unwrap().1.clone()
        };
        let base = json!({"siret": "12345691500016", "medecine_travail": "MT1"});
        let mut e = base.clone();
        e["declarant_dsn"] = json!({"tel": "+33 1 23 45 67 89"});
        e["tel"] = json!("0999999999");
        assert_eq!(tel(e), "33123456789");
        let mut e = base.clone();
        e["declarant_dsn"] = json!({});
        e["tel"] = json!("01.23.45.67.89");
        assert_eq!(tel(e), "0123456789 ");
        let mut e = base.clone();
        e["signataire"] = json!({"tel": "06 12 34 56 78 90 12"});
        assert_eq!(tel(e), "06123456789");
        assert_eq!(tel(base), " ".repeat(11));
    }

    #[test]
    fn normaliser_ne_garde_que_ce_que_le_format_120_admet() {
        assert_eq!(normaliser("Théâtre de l'Œuvre & Cie - Ça"), "THEATRE DE L UVRE & CIE - CA");
        assert_eq!(normaliser("Strauß Łukasz"), "STRAU UKASZ");
        assert_eq!(normaliser("Nguyễn Thị Hồng"), "NGUYEN THI HONG");
        assert_eq!(normaliser("ﬁn ½ ™ Ａｂ² \u{a0}x"), "FIN 12 TM AB2  X");
        assert_eq!(normaliser("a.b,c/d"), "A B C D");
        assert_eq!(normaliser(""), "");
    }

    #[test]
    fn un_cddu_ecrit_000_en_periode_d_essai_les_autres_la_laissent_a_blanc() {
        let e = embauche(salarie(), "cddu_spectacle", json!("2026-10-05"));
        assert_eq!(zone(&enr(&e, None, json!("")).unwrap(), "jours_essai"), "000");
        let e = embauche(salarie(), "cdi", Value::Null);
        assert_eq!(zone(&enr(&e, None, json!("")).unwrap(), "jours_essai"), "   ");
        let e = embauche(salarie(), "cdd", json!("2026-12-31"));
        assert_eq!(zone(&enr(&e, None, json!("")).unwrap(), "jours_essai"), "   ");
    }

    #[test]
    fn les_noms_accentues_et_trop_longs_se_coupent_a_la_largeur_de_la_zone() {
        let mut s = salarie();
        s["nom"] = json!("Éloïse-Ænéas");
        s["prenom"] = json!("x".repeat(40));
        let t = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap();
        assert_eq!(zone(&t, "nom").trim_end(), "ÉLOÏSE-ÆNÉAS");
        assert_eq!(zone(&t, "prenom"), "X".repeat(32));
        assert_eq!(t.chars().count(), LONGUEUR);
    }

    #[test]
    fn les_refus_de_forme_nomment_le_champ() {
        let mut s = salarie();
        s["departement_naissance"] = json!("974");
        let m = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap_err();
        assert!(m.contains("dept_naissance") && m.contains("'974'"), "{}", m);
        let mut s = salarie();
        s["nir"] = json!("28503759990011234");
        let m = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap_err();
        assert!(m.contains("nir") && m.contains("'28503759990011234'"), "{}", m);
        let mut s = salarie();
        s["date_naissance"] = json!("pas une date");
        let m = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap_err();
        assert!(m.contains("'pas une date'"), "{}", m);
        let mut s = salarie();
        s.as_object_mut().unwrap().remove("lieu_naissance");
        let m = enr(&embauche(s, "cdi", Value::Null), None, json!("")).unwrap_err();
        assert!(m.contains("lieu_naissance"), "{}", m);
        let mut e = embauche(salarie(), "cdi", Value::Null);
        e.nature = Some("inexistante".into());
        let m = enr(&e, None, json!("")).unwrap_err();
        assert!(m.contains("'inexistante'"), "{}", m);
    }

    #[test]
    fn l_heure_d_embauche_se_pose_sur_quatre_caracteres() {
        let e = embauche(salarie(), "cdi", Value::Null);
        let c = chemins();
        let mut p = parametres(None, json!(""), &c);
        p.heure_embauche = "1900".into();
        assert_eq!(zone(&enregistrements(&[e.clone()], &p).unwrap()[0], "heure_embauche"), "1900");
        p.heure_embauche = "800".into();
        let m = enregistrements(&[e], &p).unwrap_err();
        assert!(m.contains("heure_embauche") && m.contains("'800'"), "{}", m);
    }

    #[test]
    fn le_gabarit_de_la_compagnie_passe_devant_celui_du_logiciel() {
        let d = dossier("gabarits");
        let gab = d.join("compagnie").join("gabarits");
        std::fs::create_dir_all(&gab).unwrap();
        let mut t: Vec<char> = neutre();
        t.splice(0..4, "ZZZZ".chars());
        let octets: Vec<u8> = t.iter().map(|&c| c as u8).collect();
        std::fs::write(gab.join(NOM_GABARIT), octets).unwrap();
        let ent = json!({"_gabarits": gab.to_string_lossy()});
        let poste = d.join("poste");
        assert_eq!(modele_de(NOM_GABARIT, &ent, &poste), gab.join(NOM_GABARIT));
        // Une copie de developpement n'emprunte rien au poste, et prend le modele livre.
        std::fs::create_dir_all(poste.join("gabarits")).unwrap();
        std::fs::write(poste.join("gabarits").join(NOM_GABARIT), b"x").unwrap();
        let vide = d.join("copie").join("gabarits");
        std::fs::create_dir_all(&vide).unwrap();
        let ent = json!({"_gabarits": vide.to_string_lossy()});
        assert_eq!(modele_de(NOM_GABARIT, &ent, &poste.join("gabarits")), poste.join("gabarits").join(NOM_GABARIT));
        std::fs::write(d.join("copie").join(MARQUEUR_COPIE), "{}").unwrap();
        assert_eq!(modele_de(NOM_GABARIT, &ent, &poste.join("gabarits")), dossier_du_code().join("modeles").join(NOM_GABARIT));
        // Un gabarit trop court est refuse.
        std::fs::write(gab.join(NOM_GABARIT), b"court\n").unwrap();
        let ent = json!({"_gabarits": gab.to_string_lossy()});
        let c = Chemins { defaut: PathBuf::new(), poste };
        let m = gabarit_lu(Some(&ent), &c).unwrap_err();
        assert!(m.contains("gabarit DUE"), "{}", m);
        let _ = std::fs::remove_dir_all(&d);
    }

    // ---- le latin-1 et le .zip

    #[test]
    fn un_caractere_hors_latin_1_se_refuse() {
        // Le fichier DPAE s'ecrit en latin-1 : ce qui n'y entre pas se refuse, rien ne se perd en route.
        assert_eq!(en_latin1("caf\u{e9}").unwrap(), b"caf\xe9");
        for t in ["ab\u{178}c", "a\u{178}\u{178}\u{1f600}", "\u{1f600}"] {
            assert!(en_latin1(t).is_err(), "{:?}", t);
        }
    }

    /// Relit un .zip d'un fichier : (nom, octets decompresses), CRC verifie.
    fn depaqueter(z: &[u8]) -> (String, Vec<u8>) {
        use flate2::read::DeflateDecoder;
        use std::io::Read;
        let u16_ = |i: usize| u16::from_le_bytes([z[i], z[i + 1]]) as usize;
        let u32_ = |i: usize| u32::from_le_bytes([z[i], z[i + 1], z[i + 2], z[i + 3]]);
        assert_eq!(u32_(0), 0x0403_4b50);
        let (compresse, taille, nom, extra) = (u32_(18) as usize, u32_(22) as usize, u16_(26), u16_(28));
        let debut = 30 + nom + extra;
        let mut sortie = Vec::new();
        DeflateDecoder::new(&z[debut..debut + compresse]).read_to_end(&mut sortie).unwrap();
        assert_eq!(sortie.len(), taille);
        let mut crc = flate2::Crc::new();
        crc.update(&sortie);
        assert_eq!(crc.sum(), u32_(14));
        // le repertoire central pointe sur lui, et finit l'archive
        let rep = debut + compresse;
        assert_eq!(u32_(rep), 0x0201_4b50);
        let fin = z.len() - 22;
        assert_eq!(u32_(fin), 0x0605_4b50);
        assert_eq!(u32_(fin + 16) as usize, rep);
        (String::from_utf8(z[30..30 + nom].to_vec()).unwrap(), sortie)
    }

    #[test]
    fn le_zip_contient_les_memes_octets_sous_le_meme_nom() {
        let donnees: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        let z = zip("DPAE_Octobre2026.txt", &donnees, "031020261407").unwrap();
        let (nom, octets) = depaqueter(&z);
        assert_eq!(nom, "DPAE_Octobre2026.txt");
        assert_eq!(octets, donnees);
        // la minute de l'ecriture est la date de l'entree : 2026-10-03 14:07
        let date = u16::from_le_bytes([z[12], z[13]]);
        let heure = u16::from_le_bytes([z[10], z[11]]);
        assert_eq!((date >> 9) + 1980, 2026);
        assert_eq!((date >> 5) & 15, 10);
        assert_eq!(date & 31, 3);
        assert_eq!((heure >> 11, (heure >> 5) & 63), (14, 7));
        // un fichier vide se zippe aussi
        let (_, vide) = depaqueter(&zip("v.txt", b"", "031020261407").unwrap());
        assert!(vide.is_empty());
    }

    #[test]
    fn ecrire_pose_le_fichier_et_son_zip_et_cree_le_dossier() {
        let d = dossier("ecrire");
        let chemin = d.join("sorties").join("2026-10").join("DPAE_Octobre2026.txt");
        let c = chemins();
        let ent = entreprise();
        let e = embauche(salarie(), "cdi", Value::Null);
        let p = parametres(Some(&ent), json!("117"), &c);
        ecrire(&[e.clone(), e.clone()], &chemin, &p, true).unwrap();
        let octets = std::fs::read(&chemin).unwrap();
        assert_eq!(octets.len(), 2 * LONGUEUR);
        let (nom, dans_le_zip) = depaqueter(&std::fs::read(d.join("sorties/2026-10/DPAE_Octobre2026.txt.zip")).unwrap());
        assert_eq!(nom, "DPAE_Octobre2026.txt");
        assert_eq!(dans_le_zip, octets);
        assert_eq!(octets, contenu(&[e.clone(), e.clone()], &p).unwrap());
        // sans zip
        let seul = d.join("sans-zip.txt");
        ecrire(&[e.clone()], &seul, &p, false).unwrap();
        assert!(seul.exists() && !d.join("sans-zip.txt.zip").exists());
        // un refus laisse le dossier cree, et rien d'ecrit
        let mut mauvais = salarie();
        mauvais["nir"] = json!("");
        mauvais["sexe"] = json!("");
        let refuse = d.join("autre").join("x.txt");
        assert!(ecrire(&[embauche(mauvais, "cdi", Value::Null)], &refuse, &p, true).is_err());
        assert!(d.join("autre").is_dir() && !refuse.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn une_liste_vide_ne_lit_aucun_gabarit() {
        let c = Chemins { defaut: PathBuf::from("/n/existe/pas"), poste: PathBuf::from("/n/existe/pas") };
        let ent = json!({});
        assert!(contenu(&[], &parametres(Some(&ent), json!(""), &c)).unwrap().is_empty());
        let e = embauche(salarie(), "cdi", Value::Null);
        let m = contenu(&[e], &parametres(None, json!(""), &c)).unwrap_err();
        assert!(m.contains("/n/existe/pas"), "{}", m);
    }

    // ---- le recapitulatif

    #[test]
    fn le_recapitulatif_d_un_permanent_ne_nomme_aucun_spectacle() {
        let e = embauche(salarie(), "cdi", Value::Null);
        let t = recapitulatif(&[e], &json!("0800")).unwrap();
        assert!(t.contains("sans terme"), "{}", t);
        assert!(t.contains("CDI"), "{}", t);
        assert!(!t.contains("CDD d'usage"), "{}", t);
        // Ce qu'il faut pour declarer : qui, sa naissance, son NIR, le jour et l'heure de l'embauche, l'emploi.
        for fait in ["Dupont Alice", "07/03/1985", "Tours (37)", "285037599900142", "01/10/2026", "08h00", "Comedienne"] {
            assert!(t.contains(fait), "{} : {}", fait, t);
        }
    }

    #[test]
    fn le_recapitulatif_d_un_cdd_d_usage_dit_son_terme_et_son_spectacle() {
        let mut e = embauche(salarie(), "cddu_spectacle", json!("2026-10-05"));
        e.spectacle = json!({"titre": "Ubu roi"});
        let t = recapitulatif(&[e], &json!("1900")).unwrap();
        assert!(t.contains("Embauche le 01/10/2026 a 19h00, fin prevue le 05/10/2026"), "{}", t);
        assert!(t.contains(" - Ubu roi\n"), "{}", t);
        // un spectacle sans titre : le KeyError du Python
        let mut e = embauche(salarie(), "cddu_spectacle", json!("2026-10-05"));
        e.spectacle = json!({"autre": 1});
        assert!(recapitulatif(&[e], &json!("1900")).unwrap_err().contains("titre"));
    }

    #[test]
    fn le_recapitulatif_sans_nir_demande_l_immatriculation() {
        let mut s = salarie();
        s["nir"] = json!("");
        s["sexe"] = json!("2");
        let t = recapitulatif(&[embauche(s, "cdi", Value::Null)], &json!("0800")).unwrap();
        assert!(t.contains("immatriculation"), "{}", t);
        assert!(!recapitulatif(&[], &json!("0800")).unwrap().contains("Dupont"));
        assert!(recapitulatif(&[], &Value::Null).is_err(), "une heure d'embauche qui n'est pas un texte se refuse");
    }

    // ---- les lecteurs

    fn flux(n: usize, f: impl Fn(&mut Vec<char>, usize)) -> String {
        let mut out = String::new();
        for i in 0..n {
            let mut r = vec![' '; LONGUEUR - 1];
            f(&mut r, i);
            out.extend(r);
            out.push('\n');
        }
        out
    }

    fn poser_texte(r: &mut Vec<char>, a: usize, t: &str) {
        for (i, c) in t.chars().enumerate() {
            r[a + i] = c;
        }
    }

    #[test]
    fn un_flux_se_decoupe_a_989_ou_990_caracteres_ou_en_largeur_fixe() {
        let deux = flux(2, |r, _| r[0] = 'X');
        assert_eq!(deux.chars().count(), 2 * LONGUEUR);
        assert_eq!(lire_enregistrements(&deux).len(), 2);
        // CRLF
        assert_eq!(lire_enregistrements(&deux.replace('\n', "\r\n")).len(), 2);
        // colle bout a bout, sans separateur
        let colle: String = deux.replace('\n', " ");
        assert_eq!(lire_enregistrements(&colle).len(), 2);
        // 989 caracteres seuls
        let sans_lf = deux.trim_end_matches('\n').to_string();
        assert_eq!(lire_enregistrements(&sans_lf).len(), 2);
        assert!(lire_enregistrements(&sans_lf).iter().all(|r| r.len() == LONGUEUR));
        // rien, ou des blancs : rien
        assert!(lire_enregistrements("").is_empty());
        assert!(lire_enregistrements("\n  \n").is_empty());
        // un morceau de longueur quelconque : le decoupage fixe
        assert_eq!(lire_enregistrements(&format!("{}x", colle)).len(), 2);
    }

    #[test]
    fn relire_rend_les_zones_dans_l_ordre_des_champs() {
        let f = flux(1, |r, _| {
            poser_texte(r, 283, "DUPONT");
            poser_texte(r, 347, "2850375999001");
            poser_texte(r, 412, "01102026");
        });
        let l = relire(&f);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].iter().map(|(k, _)| *k).collect::<Vec<_>>(), CHAMPS.iter().map(|c| c.0).collect::<Vec<_>>());
        let v = |k: &str| l[0].iter().find(|(n, _)| *n == k).unwrap().1.clone();
        assert_eq!(v("nom").trim_end(), "DUPONT");
        assert_eq!(v("nir"), "2850375999001");
        assert_eq!(v("date_embauche"), "01102026");
        assert_eq!(v("nom").chars().count(), 32);
    }

    #[test]
    fn le_certificat_rend_le_dossier_le_code_et_les_formalites() {
        let f = flux(4, |r, i| {
            poser_texte(r, 283, "DUPONT");
            poser_texte(r, 412, "01102026");
            poser_texte(r, 75, "00");
            match i {
                0 => {
                    poser_texte(r, 424, "D1234");
                    poser_texte(r, 429, "99");
                    poser_texte(r, 970, "O");
                    poser_texte(r, 974, "O");
                }
                1 => {
                    poser_texte(r, 429, "98");
                    poser_texte(r, 973, "O");
                }
                2 => poser_texte(r, 429, "77"),
                _ => {
                    poser_texte(r, 75, "11");
                    poser_texte(r, 429, "00");
                }
            }
        });
        let c = lire_certificat(&f);
        assert_eq!(c.len(), 4);
        assert_eq!(c[0]["embauche"], "2026-10-01");
        assert_eq!(c[0]["dossier"], "D1234");
        assert_eq!(c[0]["code"], "99");
        assert_eq!(c[0]["libelle"], "Autre erreur (données facultatives)");
        assert_eq!(c[0]["enregistree"], true);
        assert_eq!(c[0]["formalites"], json!(["CERTIF", "MT"]));
        // 98 : deja existante, et pas enregistree
        assert_eq!(c[1]["deja_existante"], true);
        assert_eq!(c[1]["enregistree"], false);
        assert_eq!(c[1]["formalites"], json!(["IMMA"]));
        // un code inconnu du Format 120
        assert_eq!(c[2]["libelle"], "code 77 (inconnu du Format 120)");
        assert_eq!(c[2]["enregistree"], false);
        // un lot en erreur ; le code `00` sans dossier n'est pas enregistre
        assert_eq!(c[3]["lot"], "11");
        assert_eq!(c[3]["enregistree"], false);
        // sans code de retour, `00`
        assert_eq!(lire_certificat(&flux(1, |_, _| {}))[0]["code"], "00");
    }

    #[test]
    fn deux_flux_qui_ne_different_que_par_l_horodatage_declarent_la_meme_chose() {
        let a = flux(2, |r, _| poser_texte(r, 283, "DUPONT"));
        let b = flux(2, |r, _| {
            poser_texte(r, 283, "DUPONT");
            poser_texte(r, 121, "031020261407");
        });
        assert!(meme_declaration(&a, &b));
        let autre = flux(2, |r, i| poser_texte(r, 283, if i == 1 { "DURAND" } else { "DUPONT" }));
        assert!(!meme_declaration(&a, &autre));
        assert!(!meme_declaration(&a, &flux(1, |r, _| poser_texte(r, 283, "DUPONT"))));
        assert!(!meme_declaration("", ""));
    }

    // ---- le service

    #[test]
    fn les_tables_n_ont_ni_trou_ni_recouvrement() {
        let mut zones: Vec<(usize, usize, &str)> = CHAMPS.iter().map(|(n, a, b)| (*a, *b, *n)).collect();
        zones.extend(EMPLOYEUR.iter().map(|(n, a, b)| (*a, *b, *n)));
        zones.sort();
        for w in zones.windows(2) {
            assert!(w[0].1 <= w[1].0, "{} recouvre {}", w[0].2, w[1].2);
        }
        assert!(zones.iter().all(|z| z.1 <= LONGUEUR - 1));
        assert_eq!(CODES_RETOUR_AR.len(), 26);
        assert!(ZONES_RETOUR.iter().all(|z| z.2 < LONGUEUR));
    }
}
