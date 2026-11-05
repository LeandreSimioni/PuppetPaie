//! La preparation des depots, sans reseau.
//!
//! Le passage en test en memoire (`basculer_dsn`, `renumeroter`, `basculer_dpae`), les garde-fous du depot (`verifier`,
//! sur ce que le fichier declare, lu par `declarations::dsn_lue::lire`) et le controle des identifiants du declarant
//! DPAE. Un depot de test ne part jamais avec la nature `02` ni l'indicateur `120` : la bascule les remplace avant tout
//! envoi, et `verifier` refuse un test demande sur un fichier reel comme un reel demande sur un fichier de test. Le
//! transport est ailleurs (`transport`).

use declarations::dsn_lue::DsnLue;
use socle::echec::Echec;
use socle::texte::{ascii_nfkd, strip};

type Resultat<T> = Result<T, Echec>;

pub const NATURE_TEST: &str = "01";
pub use declarations::dsn_lue::{MENSUELLE, NATURE_REEL};
pub const RUB_NATURE: &str = "S10.G00.00.005";
pub use declarations::norme::SIGNALEMENTS;


pub const TYPES: [(&str, &str); 5] = [
    ("01", "déclaration normale"),
    ("02", "déclaration normale sans individu"),
    ("03", "annule et remplace intégral"),
    ("04", "déclaration annule"),
    ("05", "annule et remplace sans individu"),
];

/// Le champ C_VERSION de l'enregistrement DPAE a plat : positions 72 a 75.
pub const INDICATEUR: (usize, usize) = (72, 75);
pub const INDICATEUR_REEL: &str = "120";
pub const INDICATEUR_TEST: &str = "TST";
pub const LONGUEUR_DPAE: i64 = 990;

#[track_caller]
fn refus(classe: &'static str, message: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some(classe), message)
}

#[track_caller]
fn depot_refuse(message: impl Into<String>) -> Echec {
    refus("DepotRefuse", message)
}

// ------------------------------------------------ la bascule en test, en memoire

/// Position du premier `prefixe` que `ok` accepte, avec la longueur du motif.
fn premier(octets: &[u8], prefixe: &[u8], ok: impl Fn(&[u8]) -> Option<usize>) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + prefixe.len() <= octets.len() {
        if &octets[i..i + prefixe.len()] == prefixe {
            if let Some(n) = ok(&octets[i + prefixe.len()..]) {
                return Some((i, prefixe.len() + n));
            }
        }
        i += 1;
    }
    None
}

fn remplacer(octets: &[u8], debut: usize, longueur: usize, par: &[u8]) -> Vec<u8> {
    let mut s = Vec::with_capacity(octets.len() + par.len());
    s.extend_from_slice(&octets[..debut]);
    s.extend_from_slice(par);
    s.extend_from_slice(&octets[debut + longueur..]);
    s
}

/// `<rubrique>,'\d\d'` (chiffres ASCII, comme un motif d'octets) : la longueur de `\d\d'`.
fn deux_chiffres(reste: &[u8]) -> Option<usize> {
    (reste.len() >= 3 && reste[0].is_ascii_digit() && reste[1].is_ascii_digit() && reste[2] == b'\'').then_some(3)
}

/// `basculer_en_test` : S10.G00.00.005 repasse a `01`, jamais sur le disque.
pub fn basculer_dsn(octets: &[u8]) -> Resultat<Vec<u8>> {
    let prefixe = format!("{},'", RUB_NATURE);
    match premier(octets, prefixe.as_bytes(), deux_chiffres) {
        Some((i, n)) => Ok(remplacer(octets, i, n, format!("{},'{}'", RUB_NATURE, NATURE_TEST).as_bytes())),
        None => Err(depot_refuse(format!("Rubrique {} introuvable : ce fichier ne ressemble pas a une DSN.", RUB_NATURE))),
    }
}

/// `poser` : `<rubrique>,'[^']*'` devient `<rubrique>,'<valeur>'`, la premiere fois.
fn poser(octets: &[u8], rubrique: &str, valeur: &str) -> Resultat<Vec<u8>> {
    let prefixe = format!("{},'", rubrique);
    let fin = |reste: &[u8]| reste.iter().position(|&b| b == b'\'').map(|p| p + 1);
    match premier(octets, prefixe.as_bytes(), fin) {
        Some((i, n)) => Ok(remplacer(octets, i, n, format!("{},'{}'", rubrique, valeur).as_bytes())),
        None => Err(depot_refuse(format!(
            "Rubrique {} introuvable : ce fichier ne ressemble pas a une DSN mensuelle.",
            rubrique
        ))),
    }
}

/// `renumeroter_pour_test` : un annule-et-remplace d'ordre `ordre` (type 01 -> 03,
/// 02 -> 05), en memoire.
pub fn renumeroter(octets: &[u8], ordre: &str) -> Resultat<Vec<u8>> {
    let prefixe = b"S20.G00.05.002,'";
    let mut contenu = octets.to_vec();
    if let Some((i, _)) = premier(octets, prefixe, deux_chiffres) {
        let type_ar = match &octets[i + prefixe.len()..i + prefixe.len() + 2] {
            b"01" => Some("03"),
            b"02" => Some("05"),
            _ => None,
        };
        if let Some(t) = type_ar {
            contenu = poser(&contenu, "S20.G00.05.002", t)?;
        }
    }
    poser(&contenu, "S20.G00.05.004", ordre)
}

// ----------------------------------------------------------------- verifier

/// La confirmation recopiee, sans les blancs de part et d'autre.
fn nettoye(s: &str) -> &str {
    strip(s)
}

/// Le mode demande doit correspondre au fichier, et un reel
/// exige une preuve.
pub fn verifier(a: &DsnLue, reel: bool, mois_confirme: Option<&str>, presence_prouvee: bool) -> Resultat<()> {
    let nature_envoi = a.nature_envoi.as_deref().unwrap_or("");
    if reel && nature_envoi != NATURE_REEL {
        return Err(depot_refuse(format!(
            "Le fichier porte {}='{}' (envoi de test) et vous demandez un depot REEL. Un fichier de test depose reellement ne declare RIEN : les donnees ne sont pas conservees. Deposez le fichier du mois, que la route deposer ecrit elle-meme.",
            RUB_NATURE, nature_envoi
        )));
    }
    if !reel && nature_envoi != NATURE_REEL {
        return Ok(());
    }
    if reel {
        let attendu = a.confirmation_attendue();
        if let Some(m) = mois_confirme.filter(|m| !m.is_empty()) {
            if nettoye(m) != attendu {
                return Err(depot_refuse(format!(
                    "{} confirme '{}' alors que le fichier declare '{}'. Rien n'est parti.",
                    if a.est_signalement() { "Jour de l'evenement" } else { "Mois" },
                    nettoye(m),
                    attendu
                )));
            }
            return Ok(());
        }
        if !presence_prouvee {
            return Err(depot_refuse(format!(
                "Un depot REEL demande une preuve : reecrire {} ({}), ou retaper le mot de passe du logiciel.",
                if a.est_signalement() { "le jour de l'evenement signale" } else { "le mois declare" },
                attendu
            )));
        }
    }
    Ok(())
}

// --------------------------------------------------------------------- DPAE

/// C_VERSION du premier enregistrement.
pub fn indicateur(octets: &[u8]) -> String {
    let (a, b) = INDICATEUR;
    let fin = b.min(octets.len());
    if a >= fin {
        return String::new();
    }
    octets[a..fin].iter().map(|&c| c as char).collect()
}

pub fn est_reel(octets: &[u8]) -> bool {
    indicateur(octets) == INDICATEUR_REEL
}

/// Chaque enregistrement repasse en C_VERSION = TST,
/// avec la meme affectation de tranche qu'un `bytearray` (une tranche hors du
/// flux s'ajoute a la fin).
pub fn basculer_dpae(octets: &[u8], longueur: i64) -> Resultat<Vec<u8>> {
    if longueur == 0 {
        return Err(refus("ZeroDivisionError", "integer modulo by zero"));
    }
    let n = octets.len() as i64;
    if n % longueur != 0 {
        return Err(refus("ValueError", "flux DPAE de longueur inattendue"));
    }
    let mut sortie = octets.to_vec();
    let (a, b) = (INDICATEUR.0 as i64, INDICATEUR.1 as i64);
    for i in 0..(n / longueur).max(0) {
        let long = sortie.len() as i64;
        let debut = (i * longueur + a).min(long);
        let fin = (i * longueur + b).clamp(debut, long);
        sortie.splice(debut as usize..fin as usize, INDICATEUR_TEST.bytes());
    }
    Ok(sortie)
}

/// Ce que le controle des identifiants DPAE verifie sans rien lire d'ailleurs :
/// l'identite du declarant et ses accents. `arobase` : le mot de passe porte un
/// `@`.
pub fn identifiants(declarant: &crate::transport::Identite, arobase: bool) -> Resultat<Vec<String>> {
    let mut soucis = Vec::new();
    let manquants = declarant.manquants();
    if !manquants.is_empty() {
        soucis.push(format!(
            "Le compte des DPAE est incomplet (il manque : {}). Il se saisit dans Configuration › Mots de passe, ligne Urssaf (DPAE) : le nom et le prénom du compte ouvert sur urssaf.fr pour la DPAE.",
            manquants.join(", ")
        ));
    }
    for (c, v) in [("nom", declarant.nom.as_str()), ("prenom", declarant.prenom.as_str())] {
        if v != ascii_nfkd(v) {
            soucis.push(format!(
                "Le {} du declarant (« {} ») porte un accent. La DUE MtoM les refuse : corriger config/entreprise.json.",
                c,
                v
            ));
        }
    }
    if arobase {
        soucis.push(
            "Le mot de passe contient un caractere @, que la connexion MtoM refuse. Il doit etre change sur net-entreprises.fr."
                .to_string(),
        );
    }
    Ok(soucis)
}

