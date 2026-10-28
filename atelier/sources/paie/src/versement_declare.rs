//! Ce qu'une DSN a declare pour un contrat : son versement (bloc 50) et les blocs qui le suivent, tels qu'elle les a
//! ecrits depuis la paie qu'elle a declaree.
//!
//! C'est ce que les blocs negatifs d'une regularisation annulent. La norme (§ 2.1.3.6, p. 30 et 49) annule « le
//! montant declare » : un premier rappel de -1000 (ce qui est parti), un second de 1200 (ce qui aurait du partir). Ce
//! qui a ete declare se lit dans la version de l'engagement sur laquelle la DSN est partie (regle 4 :
//! `engagement::etats::paies_declarees`) ; le versement s'ecrit depuis sa paie comme la mensuelle l'ecrit
//! (`dsn::versement_ecrit`), et se relit ici.
//!
//! Le versement se reconnait au numero de contrat qu'il porte (51.010, 52.006, 54.005, 78.006) et a sa periode
//! (51.001, 54.003, 58.001, 78.002, toutes dans le mois declare). Tout ce qui ne se rapproche pas proprement REFUSE
//! (regle 1) : pas de versement, plusieurs, un versement qui porte aussi un autre contrat, une rubrique qu'on ne sait pas
//! annuler.

use std::str::FromStr;

use droit::contexte::Contexte;
use engagement::calcul::Paie;

use socle::num::Decimal;

/// Les rubriques d'un versement que l'annulation reprend, et si elles portent un montant (a changer de signe).
/// Le bloc 52 (primes) n'y est pas : `bloc_regularisation` ne l'a jamais ecrit, ni avant ni apres (fiche N7,
/// § 2.1.3.6.1) ; le reprendre d'un seul cote annulerait une prime sans la redeclarer.
const REPRISES: [(&str, bool); 31] = [
    ("S21.G00.51.001", false),
    ("S21.G00.51.002", false),
    ("S21.G00.51.010", false),
    ("S21.G00.51.011", false),
    ("S21.G00.51.013", true),
    ("S21.G00.53.001", false),
    ("S21.G00.53.002", true),
    ("S21.G00.53.003", false),
    ("S21.G00.54.001", false),
    ("S21.G00.54.002", true),
    ("S21.G00.54.003", false),
    ("S21.G00.54.004", false),
    ("S21.G00.54.005", false),
    ("S21.G00.58.001", false),
    ("S21.G00.58.002", false),
    ("S21.G00.58.003", false),
    ("S21.G00.58.004", true),
    ("S21.G00.78.001", false),
    ("S21.G00.78.002", false),
    ("S21.G00.78.003", false),
    ("S21.G00.78.004", true),
    ("S21.G00.78.005", false),
    ("S21.G00.78.006", false),
    ("S21.G00.79.001", false),
    ("S21.G00.79.004", true),
    ("S21.G00.81.001", false),
    ("S21.G00.81.002", false),
    ("S21.G00.81.003", true),
    ("S21.G00.81.004", true),
    ("S21.G00.81.005", false),
    ("S21.G00.81.007", false),
];


/// Les rubriques qui citent le contrat du versement.
const CITENT_LE_CONTRAT: [&str; 4] = ["S21.G00.51.010", "S21.G00.52.006", "S21.G00.54.005", "S21.G00.78.006"];
/// Les rubriques qui datent le versement (debut de la periode de rattachement).
const DATENT: [&str; 4] = ["S21.G00.51.001", "S21.G00.54.003", "S21.G00.58.001", "S21.G00.78.002"];

/// Le versement d'un contrat, tel qu'une DSN l'a declare.
#[derive(Clone, Debug)]
pub struct VersementDeclare {
    /// Ce qui a ete declare, en mots, pour les messages (« la DSN de juillet 2026 »).
    pub fichier: String,
    /// S21.G00.50.002.
    pub net_imposable: Decimal,
    /// S21.G00.50.004 (le montant net verse de la norme : la RNF moins CSG/CRDS non deductible et sante patronale).
    pub net_verse: Decimal,
    /// S21.G00.58.004 du type 03 de la periode d'origine : le montant net social declare pour elle (`dsn::net_social`).
    /// Ni lui ni les frais ne disent ce qui a ete paye : le net deja verse se lit sur le bulletin
    /// (`produire_regularisations`).
    pub net_social: Decimal,
    /// Les montants nets sociaux que des corrections deja portees ont declares aux dates du mois de leur versement (norme
    /// DSN 2026.1, S21.G00.58.003, p. 265 ; BOSS, montant net social, Q 35 et Q 41 : `dsn::blocs_58_de_la_correction`) :
    /// ils sont declares pour ce contrat, hors de la periode d'origine. Zero sans correction portee.
    pub rappels_net_social: Decimal,
    /// Les remboursements de frais declares : la somme des S21.G00.54.002 de type 07, 08 et 09 (`FRAIS_54`).
    pub frais: Decimal,
    /// S21.G00.51.013 du type 001.
    pub brut: Decimal,
    /// Les rubriques 51, 53, 54, 58, 78, 79 et 81 du versement, dans l'ordre ecrit, valeurs telles quelles ; le bloc 58
    /// est celui de la periode d'origine (les rappels sont dans `rappels_net_social`).
    pub lignes: Vec<(String, String)>,
    /// Les affiliations du contrat : (S21.G00.70.012, S21.G00.70.013).
    pub affiliations: Vec<(String, String)>,
    /// S21.G00.50.006 du versement du mois d'origine : le taux de prelevement a la source applique ce mois-la, que le
    /// bloc 56 d'un indu cite (S21.G00.56.006, « le taux applique le mois de l'erreur ou de l'indu », cahier technique
    /// 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.56.txt`). Apres une correction deja portee, il reste
    /// celui du mois d'origine : le versement de rappel porte le taux de SON mois, qui n'est pas le mois de l'erreur
    /// (inference : le texte ne parle pas d'une seconde correction). `None` : le versement n'en porte pas.
    pub taux_pas: Option<String>,
    /// S21.G00.40.001 (JJMMAAAA) : la date de debut declaree, la DERNIERE (une correction deja portee a pu la changer).
    /// Une correction qui la change la declare en S21.G00.41.021 (fiche net-entreprises 1952). `None` : rien de declare
    /// (un contrat ajoute apres la limite).
    pub debut_contrat: Option<String>,
}

impl VersementDeclare {
    /// Les lignes des blocs dont le prefixe est donne (`"51"`, `"78"`...), dans l'ordre ecrit.
    pub fn blocs<'a>(&'a self, blocs: &'a [&str]) -> impl Iterator<Item = &'a (String, String)> + 'a {
        self.lignes.iter().filter(move |(r, _)| blocs.iter().any(|b| r.get(8..10) == Some(*b)))
    }
}

/// Les types de S21.G00.54.001 qui remboursent des frais professionnels (au forfait, pris en charge, au reel).
pub const FRAIS_54: [&str; 3] = ["07", "08", "09"];

/// La rubrique porte-t-elle un montant a changer de signe ; `None` : on ne sait pas l'annuler.
pub fn est_un_montant(rubrique: &str) -> Option<bool> {
    REPRISES.iter().find(|(r, _)| *r == rubrique).map(|(_, m)| *m)
}

/// Un versement en cours de lecture.
#[derive(Default)]
struct Brouillon {
    entete: Vec<(String, String)>,
    lignes: Vec<(String, String)>,
    contrats: Vec<String>,
    dates: Vec<String>,
}

fn dans_le_mois(jjmmaaaa: &str, mois: &str) -> bool {
    // `mois` : AAAA-MM ; la date : JJMMAAAA.
    jjmmaaaa.len() == 8 && jjmmaaaa.get(4..8) == mois.get(..4) && jjmmaaaa.get(2..4) == mois.get(5..7) && mois.len() == 7
}

/// Le versement du contrat `numero` du salarie `nir` dans le texte `texte`, pour le mois `mois` : ses lignes (bloc 50 a
/// part), les affiliations du contrat, et la date de debut que son bloc 40 declare (S21.G00.40.001). Refus : aucun
/// versement, plusieurs, un versement qui cite aussi un autre contrat.
fn versement_du_contrat(texte: &str, quoi: &str, mois: &str, nir: &str, numero: &str) -> Result<(Brouillon, Vec<(String, String)>, Option<String>), String> {
    let mut individu = false;
    let mut contrat_courant: Option<String> = None;
    let mut affiliation: Option<String> = None;
    let mut affiliations: Vec<(String, String)> = Vec::new();
    let mut versements: Vec<Brouillon> = Vec::new();
    let mut ouvert = false;
    let (mut debut_courant, mut debut_du_contrat): (Option<String>, Option<String>) = (None, None);
    for ligne in crate::apercu_dsn::lignes_py(texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(socle::texte::blanc).trim_matches('\'').to_string();
        if k == "S21.G00.30.001" {
            individu = v == nir;
            contrat_courant = None;
            ouvert = false;
            continue;
        }
        if !individu {
            continue;
        }
        let bloc = k.get(..10).unwrap_or("");
        let dans_un_versement = matches!(
            bloc,
            "S21.G00.50" | "S21.G00.51" | "S21.G00.52" | "S21.G00.53" | "S21.G00.54" | "S21.G00.55" | "S21.G00.56" | "S21.G00.57" | "S21.G00.58"
                | "S21.G00.78" | "S21.G00.79" | "S21.G00.81"
        );
        if k == "S21.G00.50.001" {
            versements.push(Brouillon::default());
            ouvert = true;
        } else if !dans_un_versement {
            ouvert = false;
        }
        match k {
            "S21.G00.40.001" => {
                contrat_courant = None;
                debut_courant = Some(v.clone());
            }
            "S21.G00.40.009" => {
                if v == numero {
                    debut_du_contrat = debut_courant.clone();
                }
                contrat_courant = Some(v.clone());
            }
            "S21.G00.70.012" => affiliation = Some(v.clone()),
            "S21.G00.70.013" => {
                if contrat_courant.as_deref() == Some(numero) {
                    if let Some(id) = affiliation.take() {
                        affiliations.push((id, v.clone()));
                    }
                }
            }
            _ => {}
        }
        if !ouvert {
            continue;
        }
        let b = versements.last_mut().expect("un versement ouvert");
        if bloc == "S21.G00.50" {
            b.entete.push((k.to_string(), v));
            continue;
        }
        if CITENT_LE_CONTRAT.contains(&k) && !b.contrats.contains(&v) {
            b.contrats.push(v.clone());
        }
        if DATENT.contains(&k) {
            b.dates.push(v.clone());
        }
        b.lignes.push((k.to_string(), v));
    }
    let mut siens = versements
        .into_iter()
        .filter(|b| b.contrats.iter().any(|c| c == numero) && !b.dates.is_empty() && b.dates.iter().all(|d| dans_le_mois(d, mois)));
    let b = match (siens.next(), siens.next()) {
        (Some(b), None) => b,
        (None, _) => return Err(format!("{} ne porte aucun versement du contrat {} (salarie {}) pour ce mois", quoi, numero, nir)),
        (Some(_), Some(_)) => return Err(format!("{} porte plusieurs versements du contrat {} pour ce mois : lequel annuler ne se devine pas", quoi, numero)),
    };
    if let Some(autre) = b.contrats.iter().find(|c| *c != numero) {
        return Err(format!(
            "dans {}, le versement du contrat {} porte aussi le contrat {} : son net et ses cotisations ne se partagent pas entre les deux",
            quoi, numero, autre
        ));
    }
    Ok((b, affiliations, debut_du_contrat))
}

/// Un montant de l'en-tete (bloc 50) du versement.
fn entete(b: &Brouillon, r: &str, numero: &str, quoi: &str) -> Result<Decimal, String> {
    let t = b.entete.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str()).ok_or_else(|| format!("le versement du contrat {} n'a pas de {} dans {}", numero, r, quoi))?;
    Decimal::from_str(t).map_err(|_| format!("{} illisible dans {} : {:?}", r, quoi, t))
}

/// Le brut (S21.G00.51.013 du type 001) de lignes de versement ; chaque rubrique y doit etre de celles que
/// l'annulation sait reprendre, chaque montant lisible.
fn verifier_et_brut(lignes: &[(String, String)], numero: &str, quoi: &str) -> Result<Decimal, String> {
    let mut brut: Option<Decimal> = None;
    let mut type_51: Option<&str> = None;
    for (k, v) in lignes {
        let montant = est_un_montant(k).ok_or_else(|| format!("le versement du contrat {} porte {} dans {}, que l'annulation ne sait pas reprendre", numero, k, quoi))?;
        if montant {
            Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, quoi, v))?;
        }
        if k == "S21.G00.51.011" {
            type_51 = Some(v);
        } else if k == "S21.G00.51.013" && type_51 == Some("001") {
            let m = Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, quoi, v))?;
            brut = Some(brut.unwrap_or(Decimal::ZERO) + m);
        }
    }
    brut.ok_or_else(|| format!("le versement du contrat {} ne declare pas de brut (S21.G00.51.011 = 001) dans {}", numero, quoi))
}

/// Le montant net social declare : le S21.G00.58.004 du seul bloc 58 de type 03 de ces lignes ; aucun ou plusieurs
/// refusent.
fn net_social_de(lignes: &[(String, String)], numero: &str, quoi: &str) -> Result<Decimal, String> {
    let mut type_58: Option<&str> = None;
    let mut nets = Vec::new();
    for (k, v) in lignes {
        if k == "S21.G00.58.003" {
            type_58 = Some(v);
        } else if k == "S21.G00.58.004" && type_58 == Some("03") {
            nets.push(Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, quoi, v))?);
        }
    }
    match nets[..] {
        [n] => Ok(n),
        _ => Err(format!(
            "le versement du contrat {} declare {} montant(s) net social (S21.G00.58.003 = 03) dans {} : le net verse au salarie ne se lit pas",
            numero,
            nets.len(),
            quoi
        )),
    }
}

/// Les frais declares : la somme des S21.G00.54.002 dont le 54.001 est dans `FRAIS_54` (lignes deja verifiees lisibles).
fn frais_de(lignes: &[(String, String)], quoi: &str) -> Result<Decimal, String> {
    let mut type_54: Option<&str> = None;
    let mut total = Decimal::ZERO;
    for (k, v) in lignes {
        if k == "S21.G00.54.001" {
            type_54 = Some(v);
        } else if k == "S21.G00.54.002" && type_54.is_some_and(|t| FRAIS_54.contains(&t)) {
            total += Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, quoi, v))?;
        }
    }
    Ok(total)
}

/// Relit, dans le texte DSN `texte` (`quoi` le nomme), le versement du contrat `numero` du salarie `nir` pour le mois
/// `mois`. `Err` : une phrase qui dit ce qui manque ; l'appelant la complete de ce qu'il faut faire.
pub fn lire(texte: &str, quoi: &str, mois: &str, nir: &str, numero: &str) -> Result<VersementDeclare, String> {
    let (b, affiliations, debut_contrat) = versement_du_contrat(texte, quoi, mois, nir, numero)?;
    let (net_imposable, net_verse) = (entete(&b, "S21.G00.50.002", numero, quoi)?, entete(&b, "S21.G00.50.004", numero, quoi)?);
    let lignes: Vec<(String, String)> = b.lignes.into_iter().filter(|(k, _)| !k.starts_with("S21.G00.52.")).collect();
    let brut = verifier_et_brut(&lignes, numero, quoi)?;
    let net_social = net_social_de(&lignes, numero, quoi)?;
    let frais = frais_de(&lignes, quoi)?;
    let taux_pas = b.entete.iter().find(|(k, _)| k == "S21.G00.50.006").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    Ok(VersementDeclare {
        fichier: quoi.to_string(),
        net_imposable,
        net_verse,
        net_social,
        rappels_net_social: Decimal::ZERO,
        frais,
        brut,
        lignes,
        affiliations,
        taux_pas,
        debut_contrat,
    })
}

/// Le versement que la paie declaree `p` de `mois` a fait partir, ecrit comme la mensuelle l'ecrit
/// (`dsn::versement_ecrit`) puis relu (`lire`).
pub fn de_la_paie(ctx: &Contexte, p: &Paie, mois: &str) -> Result<VersementDeclare, String> {
    let nir = p.salarie.get("nir").and_then(serde_json::Value::as_str).unwrap_or("");
    let numero = p.engagement.numero.clone().unwrap_or_default();
    let texte = crate::dsn::versement_ecrit(ctx, p, mois)?;
    lire(&texte, &format!("la paie declaree de {}", mois), mois, nir, &numero)
}
