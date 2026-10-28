//! Ce qu'une DSN a declare pour un contrat : son versement (bloc 50) et les blocs qui le suivent, tels que les fonctions
//! de la mensuelle les ecrivent depuis la paie qu'elle a declaree.
//!
//! C'est ce que les blocs negatifs d'une regularisation annulent. La norme (§ 2.1.3.6, p. 30 et 49) annule « le
//! montant declare » : un premier rappel de -1000 (ce qui est parti), un second de 1200 (ce qui aurait du partir). Ce
//! qui a ete declare se lit dans la version de l'engagement sur laquelle la DSN est partie (regle 4 :
//! `engagement::etats::paies_declarees`) ; ses rubriques s'ecrivent depuis cette paie par les fonctions de la mensuelle
//! (`dsn::versement_ecrit`), ses montants sont ceux de la paie. Une rubrique que l'annulation ne sait pas reprendre
//! REFUSE (regle 1).

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

/// Le versement que la paie declaree `p` de `mois` a fait partir, tel que les fonctions de la mensuelle l'ecrivent
/// (`dsn::versement_ecrit`) : ses rubriques (bloc 50 a part, sans le bloc 52), les affiliations du contrat, et ses
/// montants, ceux de la paie. Refus : une rubrique que l'annulation ne sait pas reprendre, un montant net social absent.
pub fn de_la_paie(ctx: &Contexte, p: &Paie, mois: &str) -> Result<VersementDeclare, String> {
    let numero = p.engagement.numero.clone().unwrap_or_default();
    let quoi = format!("la paie declaree de {}", mois);
    let ecrit = crate::dsn::versement_ecrit(ctx, p, mois)?;
    let (entete, lignes): (Vec<(String, String)>, Vec<(String, String)>) =
        ecrit.rubriques.into_iter().filter(|(k, _)| !k.starts_with("S21.G00.52.")).partition(|(k, _)| k.starts_with("S21.G00.50."));
    let brut = verifier_et_brut(&lignes, &numero, &quoi)?;
    let net_social = net_social_de(&lignes, &numero, &quoi)?;
    let frais = frais_de(&lignes, &quoi)?;
    let taux_pas = entete.iter().find(|(k, _)| k == "S21.G00.50.006").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    Ok(VersementDeclare {
        fichier: quoi,
        net_imposable: p.net_imposable,
        net_verse: p.net_verse(),
        net_social,
        rappels_net_social: Decimal::ZERO,
        frais,
        brut,
        lignes,
        affiliations: ecrit.affiliations,
        taux_pas,
        debut_contrat: Some(p.engagement.debut.jjmmaaaa()),
    })
}

/// Le dernier etat declare d'un contrat avant la DSN d'un mois : ce qu'une correction de ce contrat annule.
pub struct DernierDeclare {
    /// Le versement de la derniere paie declaree, ses montants nets sociaux ramenes a la periode d'origine.
    pub versement: VersementDeclare,
    /// Le net a payer avant impot de la derniere paie declaree : ce qui a ete paye au salarie.
    pub net_deja_verse: Decimal,
    /// La derniere paie declaree, au numero du dernier bulletin qu'une paie declaree de ce contrat porte.
    pub avant: Paie,
}

/// LE DERNIER ETAT DECLARE de l'engagement `o` pour `mois`, par les DSN des mois anterieurs a `mois_courant` (`departs` :
/// ce qui etait parti) : la paie de la version sur laquelle la DSN de `mois` est partie, puis celle de chaque correction
/// deja portee (`engagement::etats::paies_declarees`) ; le versement de la derniere s'ecrit comme la mensuelle l'ecrit
/// (`de_la_paie`).
///
/// Le montant net social (`dsn::blocs_58_de_la_correction`) : une correction qui a change le net paye en a declare la
/// difference aux dates du mois de son versement (le rappel), et laisse le montant d'origine ; une correction qui ne l'a
/// pas change a redeclare le montant d'origine. Les rappels deja declares se tirent de la suite des paies declarees, et
/// le montant d'origine est le dernier montant net social declare moins eux. Le taux de prelevement est celui de la paie
/// du mois d'origine ; la date de debut, la derniere declaree (`engagement::etats::debut_declare`). `None` : aucune DSN
/// n'a declare ce contrat pour `mois`.
pub fn dernier_declare(
    ctx: &Contexte, o: &engagement::stock::Objet, departs: &engagement::departs::Departs, mois: &str, mois_courant: &str,
) -> Result<Option<DernierDeclare>, String> {
    let declarees = engagement::etats::paies_declarees(ctx, o, departs, mois, mois_courant)?;
    let (Some(origine), Some(derniere)) = (declarees.first(), declarees.last()) else { return Ok(None) };
    let mut etat = de_la_paie(ctx, &derniere.paie, mois)?;
    let mut rappels = Decimal::ZERO;
    for paire in declarees.windows(2) {
        let (avant, puis) = (&paire[0].paie, &paire[1].paie);
        if avant.net_a_payer != puis.net_a_payer {
            rappels += puis.net_social - avant.net_social;
        }
    }
    if !rappels.is_zero() {
        let origine_58 = etat.net_social - rappels;
        let mut type_03 = false;
        for (k, v) in etat.lignes.iter_mut() {
            match k.as_str() {
                "S21.G00.58.003" => type_03 = v == "03",
                "S21.G00.58.004" if type_03 => *v = crate::dsn::mt(origine_58),
                _ => {}
            }
        }
        etat.net_social = origine_58;
        etat.rappels_net_social = rappels;
    }
    etat.taux_pas = origine.paie.pas.as_ref().map(|p| socle::valeur::valeur_en_texte(&p.taux)).filter(|t| !t.is_empty());
    if let Some(d) = engagement::etats::debut_declare(ctx, o, departs, mois_courant)? {
        etat.debut_contrat = Some(socle::date::Jour::lire(&d)?.jjmmaaaa());
    }
    let mut avant = derniere.paie.clone();
    avant.numero_bulletin = declarees.iter().rev().find_map(|d| d.paie.numero_bulletin.clone());
    Ok(Some(DernierDeclare { versement: etat, net_deja_verse: derniere.paie.net_a_payer, avant }))
}
