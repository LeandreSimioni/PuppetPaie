//! Ce qu'une DSN DEPOSEE a declare pour un contrat : son versement (bloc 50) et les blocs qui le suivent, relus tels
//! qu'ils sont partis.
//!
//! C'est ce que les blocs negatifs d'une regularisation annulent. La norme (§ 2.1.3.6, p. 30 et 49) annule « le
//! montant declare » : un premier rappel de -1000 (ce qui est parti), un second de 1200 (ce qui aurait du partir).
//! Refaire la paie d'avant avec le moteur du jour n'annule pas ce qui a ete declare des qu'un calcul, un bareme ou
//! une fiche a change depuis le depot : le cumul vu par l'organisme deviendrait « declare - recalcule + corrige », et
//! peut devenir negatif (p. 30). Comme le numero de contrat (regle 4), ce qui a ete declare se LIT.
//!
//! Le versement se reconnait au numero de contrat qu'il porte (51.010, 52.006, 54.005, 78.006) et a sa periode
//! (51.001, 54.003, 58.001, 78.002, toutes dans le mois corrige) : un versement de regularisation que la DSN du mois
//! porterait pour un mois plus ancien n'est pas celui-la. Tout ce qui ne se rapproche pas proprement REFUSE (regle 1) :
//! pas de versement, plusieurs, un versement qui porte aussi un autre contrat, une rubrique qu'on ne sait pas annuler.
//!
//! Le DERNIER etat declare fait foi (Leandre, 10/10/2026) : celui de la derniere DSN acceptee du mois (un
//! annule-et-remplace se substitue entierement au depot precedent ; `casc::dsn_deposee` la choisit), puis, si une
//! correction de ce contrat est deja partie dans la DSN d'un autre mois, le rappel que cette DSN a declare
//! (`apres_correction`).

use std::path::Path;
use std::str::FromStr;

use crate::num::Decimal;

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

/// Le versement d'un contrat, tel que la DSN deposee l'a declare.
#[derive(Clone, Debug)]
pub struct VersementDeclare {
    /// Le nom du fichier relu, pour les messages.
    pub fichier: String,
    /// S21.G00.50.002.
    pub net_imposable: Decimal,
    /// S21.G00.50.004 (le montant net verse de la norme : la RNF moins CSG/CRDS non deductible et sante patronale).
    pub net_verse: Decimal,
    /// S21.G00.58.004 du type 03 dont la periode de rattachement (58.001) est dans le mois corrige : le montant net social
    /// declare pour la periode d'origine (`dsn::net_social`). Depuis la vague 1b, ni
    /// defraiement ni acompte n'y entrent ; un depot fait par un moteur plus ancien y portait le net a payer. Ni lui
    /// ni les frais ne disent ce qui a ete paye : le net deja verse se lit sur le bulletin (`produire_regularisations`).
    pub net_social: Decimal,
    /// Vague 1o : les montants nets sociaux que des corrections deja portees ont declares aux dates du mois de leur
    /// versement (norme DSN 2026.1, S21.G00.58.003, p. 265 ; BOSS, montant net social, Q 35 et Q 41 :
    /// `dsn::blocs_58_de_la_correction`) : ils sont declares pour ce contrat, hors de la periode d'origine. Zero pour
    /// un depot du mois.
    pub rappels_net_social: Decimal,
    /// Les remboursements de frais declares : la somme des S21.G00.54.002 de type 07, 08 et 09 (`FRAIS_54`).
    pub frais: Decimal,
    /// S21.G00.51.013 du type 001.
    pub brut: Decimal,
    /// Les rubriques 51, 53, 54, 58, 78, 79 et 81 du versement, dans l'ordre du fichier, valeurs telles quelles ; des
    /// blocs 58, ceux de la periode d'origine seulement (les rappels sont dans `rappels_net_social`).
    pub lignes: Vec<(String, String)>,
    /// Les affiliations du contrat dans la DSN deposee : (S21.G00.70.012, S21.G00.70.013).
    pub affiliations: Vec<(String, String)>,
    /// S21.G00.50.006 du versement du mois d'origine : le taux de prelevement a la source applique ce mois-la, que le
    /// bloc 56 d'un indu cite (S21.G00.56.006, « le taux applique le mois de l'erreur ou de l'indu », cahier technique
    /// 2026.1, `prive/reference/norme-dsn/texte/bloc-S21.G00.56.txt`). Il se lit sur CE versement (reste R1 #3), pas
    /// sur un autre fichier du dossier. Apres une correction deja portee, il reste celui du mois d'origine : le
    /// versement de rappel porte le taux de SON mois, qui n'est pas le mois de l'erreur (inference : le texte ne parle
    /// pas d'une seconde correction). `None` : le versement n'en porte pas.
    pub taux_pas: Option<String>,
}

impl VersementDeclare {
    /// Les lignes des blocs dont le prefixe est donne (`"51"`, `"78"`...), dans l'ordre du fichier.
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

/// Le versement du contrat `numero` du salarie `nir` dans la DSN `chemin`, pour le mois `mois` : ses lignes brutes
/// (bloc 50 a part), et les affiliations du contrat dans ce fichier. Refus : aucun versement, plusieurs, un versement
/// qui cite aussi un autre contrat.
fn versement_du_contrat(chemin: &Path, mois: &str, nir: &str, numero: &str) -> Result<(String, Brouillon, Vec<(String, String)>), String> {
    let (fichier, siens, affiliations) = versements_du_contrat(chemin, mois, nir, numero)?;
    let b = match siens.len() {
        0 => return Err(format!("la DSN deposee {} ne porte aucun versement du contrat {} (salarie {}) pour ce mois", fichier, numero, nir)),
        1 => siens.into_iter().next().expect("un versement"),
        n => {
            return Err(format!(
                "la DSN deposee {} porte {} versements du contrat {} pour ce mois : lequel annuler ne se devine pas",
                fichier, n, numero
            ))
        }
    };
    if let Some(autre) = b.contrats.iter().find(|c| *c != numero) {
        return Err(format!(
            "dans la DSN deposee {}, le versement du contrat {} porte aussi le contrat {} : son net et ses cotisations ne se partagent pas entre les deux",
            fichier, numero, autre
        ));
    }
    Ok((fichier, b, affiliations))
}

/// Les versements du contrat `numero` du salarie `nir` dans la DSN `chemin` dont la periode est dans le mois `mois`
/// (tous, qu'il y en ait zero, un ou plusieurs), et les affiliations du contrat dans ce fichier.
fn versements_du_contrat(chemin: &Path, mois: &str, nir: &str, numero: &str) -> Result<(String, Vec<Brouillon>, Vec<(String, String)>), String> {
    let fichier = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let octets = std::fs::read(chemin).map_err(|e| format!("la DSN deposee {} ne se lit pas ({})", fichier, e))?;
    let texte = crate::apercu_dsn::texte_latin1(&octets);
    let mut individu = false;
    let mut contrat_courant: Option<String> = None;
    let mut affiliation: Option<String> = None;
    let mut affiliations: Vec<(String, String)> = Vec::new();
    let mut versements: Vec<Brouillon> = Vec::new();
    let mut ouvert = false;
    for ligne in crate::apercu_dsn::lignes_py(&texte) {
        let Some((k, v)) = ligne.split_once(',') else { continue };
        let v = v.trim_matches(crate::retours::blanc_py).trim_matches('\'').to_string();
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
            "S21.G00.40.001" => contrat_courant = None,
            "S21.G00.40.009" => contrat_courant = Some(v.clone()),
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
        // Un bloc 58 date du mois de versement d'une correction (vague 1o) ne date pas la periode du versement.
        if DATENT.contains(&k) && !(k == "S21.G00.58.001" && !dans_le_mois(&v, mois)) {
            b.dates.push(v.clone());
        }
        b.lignes.push((k.to_string(), v));
    }
    let siens: Vec<Brouillon> = versements
        .into_iter()
        .filter(|b| b.contrats.iter().any(|c| c == numero) && !b.dates.is_empty() && b.dates.iter().all(|d| dans_le_mois(d, mois)))
        .collect();
    Ok((fichier, siens, affiliations))
}

/// Le prelevement a la source qu'une DSN deposee a declare pour le versement d'un contrat (bloc S21.G00.50).
#[derive(Clone, Debug, PartialEq)]
pub struct PasDeclare {
    pub fichier: String,
    /// S21.G00.50.002, la remuneration nette fiscale.
    pub net_imposable: Decimal,
    /// S21.G00.50.006, tel qu'il est ecrit.
    pub taux: String,
    /// S21.G00.50.007.
    pub type_taux: String,
    /// S21.G00.50.008 (un taux transmis par la DGFiP seulement).
    pub identifiant: Option<String>,
    /// S21.G00.50.009.
    pub prelevement: Decimal,
    /// S21.G00.50.013.
    pub soumis: Decimal,
    /// S21.G00.50.012, quand le versement le porte.
    pub abattement: Option<Decimal>,
}

/// Le prelevement a la source que la DSN deposee `chemin` a declare pour le versement du contrat `numero` du salarie
/// `nir` dans le mois `mois` ; `None` : la DSN ne porte aucun versement de ce contrat pour ce mois (une paie ajoutee
/// apres le depot). Refus : plusieurs versements, ou un versement sans taux, sans montant ou sans assiette du
/// prelevement (le fichier n'est pas de ce moteur, ou il est abime) -- rien ne se devine (regle 1).
pub fn pas_declare(chemin: &Path, mois: &str, nir: &str, numero: &str) -> Result<Option<PasDeclare>, String> {
    let (fichier, siens, _) = versements_du_contrat(chemin, mois, nir, numero)?;
    let b = match siens.len() {
        0 => return Ok(None),
        1 => siens.into_iter().next().expect("un versement"),
        n => {
            return Err(format!(
                "la DSN deposee {} porte {} versements du contrat {} pour {} : lequel a porte le prelevement a la source ne se devine pas",
                fichier, n, numero, mois
            ))
        }
    };
    let texte_de = |r: &str| b.entete.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let montant_de = |r: &str| -> Result<Decimal, String> {
        let v = texte_de(r).ok_or_else(|| format!("la DSN deposee {} ne dit pas {} du versement du contrat {} pour {}", fichier, r, numero, mois))?;
        v.parse::<Decimal>().map_err(|_| format!("la DSN deposee {} porte un {} illisible ({}) pour le contrat {}", fichier, r, v, numero))
    };
    let taux = texte_de("S21.G00.50.006")
        .ok_or_else(|| format!("la DSN deposee {} ne dit pas le taux du prelevement a la source (S21.G00.50.006) du contrat {} pour {}", fichier, numero, mois))?;
    let type_taux = texte_de("S21.G00.50.007")
        .ok_or_else(|| format!("la DSN deposee {} ne dit pas le type du taux (S21.G00.50.007) du contrat {} pour {}", fichier, numero, mois))?;
    let abattement = match texte_de("S21.G00.50.012") {
        Some(_) => Some(montant_de("S21.G00.50.012")?),
        None => None,
    };
    Ok(Some(PasDeclare {
        net_imposable: montant_de("S21.G00.50.002")?,
        prelevement: montant_de("S21.G00.50.009")?,
        soumis: montant_de("S21.G00.50.013")?,
        identifiant: texte_de("S21.G00.50.008"),
        taux,
        type_taux,
        abattement,
        fichier,
    }))
}

/// Un montant de l'en-tete (bloc 50) du versement.
fn entete(b: &Brouillon, r: &str, numero: &str, fichier: &str) -> Result<Decimal, String> {
    let t = b.entete.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str()).ok_or_else(|| format!("le versement du contrat {} n'a pas de {} dans {}", numero, r, fichier))?;
    Decimal::from_str(t).map_err(|_| format!("{} illisible dans {} : {:?}", r, fichier, t))
}

/// Le brut (S21.G00.51.013 du type 001) de lignes de versement ; chaque rubrique y doit etre de celles que
/// l'annulation sait reprendre, chaque montant lisible.
fn verifier_et_brut(lignes: &[(String, String)], numero: &str, fichier: &str) -> Result<Decimal, String> {
    let mut brut: Option<Decimal> = None;
    let mut type_51: Option<&str> = None;
    for (k, v) in lignes {
        let montant = est_un_montant(k).ok_or_else(|| {
            format!("le versement du contrat {} porte {} dans {}, que l'annulation ne sait pas reprendre", numero, k, fichier)
        })?;
        if montant {
            Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, fichier, v))?;
        }
        if k == "S21.G00.51.011" {
            type_51 = Some(v);
        } else if k == "S21.G00.51.013" && type_51 == Some("001") {
            let m = Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, fichier, v))?;
            brut = Some(brut.unwrap_or(Decimal::ZERO) + m);
        }
    }
    brut.ok_or_else(|| format!("le versement du contrat {} ne declare pas de brut (S21.G00.51.011 = 001) dans {}", numero, fichier))
}

/// Le montant net social declare : le S21.G00.58.004 du seul bloc 58 de type 03 de ces lignes ; aucun ou plusieurs
/// refusent.
fn net_social_de(lignes: &[(String, String)], numero: &str, fichier: &str) -> Result<Decimal, String> {
    let mut type_58: Option<&str> = None;
    let mut nets = Vec::new();
    for (k, v) in lignes {
        if k == "S21.G00.58.003" {
            type_58 = Some(v);
        } else if k == "S21.G00.58.004" && type_58 == Some("03") {
            nets.push(Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, fichier, v))?);
        }
    }
    match nets[..] {
        [n] => Ok(n),
        _ => Err(format!(
            "le versement du contrat {} declare {} montant(s) net social (S21.G00.58.003 = 03) dans {} : le net verse au salarie ne se lit pas",
            numero,
            nets.len(),
            fichier
        )),
    }
}

/// Les frais declares : la somme des S21.G00.54.002 dont le 54.001 est dans `FRAIS_54` (lignes deja verifiees lisibles).
fn frais_de(lignes: &[(String, String)], fichier: &str) -> Result<Decimal, String> {
    let mut type_54: Option<&str> = None;
    let mut total = Decimal::ZERO;
    for (k, v) in lignes {
        if k == "S21.G00.54.001" {
            type_54 = Some(v);
        } else if k == "S21.G00.54.002" && type_54.is_some_and(|t| FRAIS_54.contains(&t)) {
            total += Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, fichier, v))?;
        }
    }
    Ok(total)
}

/// Separe des lignes d'un versement les blocs 58 dont la periode de rattachement (58.001) n'est pas dans le mois corrige :
/// ceux qu'une correction date du mois de son versement (vague 1o, `dsn::blocs_58_de_la_correction`). Rend les autres
/// lignes, et la somme des 58.004 de type 03 ecartes.
fn separer_les_rappels_58(lignes: Vec<(String, String)>, mois: &str, fichier: &str) -> Result<(Vec<(String, String)>, Decimal), String> {
    let (mut gardees, mut rappels) = (Vec::new(), Decimal::ZERO);
    let mut bloc: Vec<(String, String)> = Vec::new();
    let mut vider = |bloc: &mut Vec<(String, String)>, gardees: &mut Vec<(String, String)>, rappels: &mut Decimal| -> Result<(), String> {
        let hors = bloc.iter().any(|(k, v)| k == "S21.G00.58.001" && !dans_le_mois(v, mois));
        if hors {
            let type_03 = bloc.iter().any(|(k, v)| k == "S21.G00.58.003" && v == "03");
            if let (true, Some((k, v))) = (type_03, bloc.iter().find(|(k, _)| k == "S21.G00.58.004")) {
                *rappels += Decimal::from_str(v).map_err(|_| format!("{} illisible dans {} : {:?}", k, fichier, v))?;
            }
        } else {
            gardees.append(bloc);
        }
        bloc.clear();
        Ok(())
    };
    for (k, v) in lignes {
        if k.starts_with("S21.G00.58.") {
            if k == "S21.G00.58.001" && !bloc.is_empty() {
                vider(&mut bloc, &mut gardees, &mut rappels)?;
            }
            bloc.push((k, v));
        } else {
            vider(&mut bloc, &mut gardees, &mut rappels)?;
            gardees.push((k, v));
        }
    }
    vider(&mut bloc, &mut gardees, &mut rappels)?;
    Ok((gardees, rappels))
}

/// Relit, dans la DSN deposee `chemin` du mois `mois`, le versement du contrat `numero` du salarie `nir`.
/// `Err` : une phrase qui dit ce qui manque ; l'appelant la complete de ce qu'il faut faire.
pub fn lire(chemin: &Path, mois: &str, nir: &str, numero: &str) -> Result<VersementDeclare, String> {
    let (fichier, b, affiliations) = versement_du_contrat(chemin, mois, nir, numero)?;
    let (net_imposable, net_verse) = (entete(&b, "S21.G00.50.002", numero, &fichier)?, entete(&b, "S21.G00.50.004", numero, &fichier)?);
    let lignes: Vec<(String, String)> = b.lignes.into_iter().filter(|(k, _)| !k.starts_with("S21.G00.52.")).collect();
    let (lignes, rappels_net_social) = separer_les_rappels_58(lignes, mois, &fichier)?;
    let brut = verifier_et_brut(&lignes, numero, &fichier)?;
    let net_social = net_social_de(&lignes, numero, &fichier)?;
    let frais = frais_de(&lignes, &fichier)?;
    let taux_pas = b.entete.iter().find(|(k, _)| k == "S21.G00.50.006").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    Ok(VersementDeclare { fichier, net_imposable, net_verse, net_social, rappels_net_social, frais, brut, lignes, affiliations, taux_pas })
}

/// Les groupes de lignes d'un versement, dans l'ordre ou `dsn::bloc_regularisation` les ecrit : chacun commence par
/// l'annulation du declare, puis declare la paie corrigee.
const GROUPES: [&[&str]; 4] = [&["51", "53"], &["54"], &["58"], &["78", "79", "81"]];

/// Ce qui est declare pour le contrat APRES une correction deja portee : `precedent` est l'etat declare avant elle
/// (le depot du mois, ou l'etat qu'une correction plus ancienne a laisse), `chemin` la DSN deposee qui l'a portee, ou
/// le versement de regularisation du contrat a sa periode dans le mois `mois`.
///
/// Dans chaque groupe (51+53, 54, 58, 78+79+81), ce versement annule `precedent` ligne pour ligne, puis declare la
/// paie corrigee : ce sont ces lignes-la, le rappel, qui font le nouvel etat. On le VERIFIE : si l'annulation n'est
/// pas exactement `precedent` (montants de signe change, 78.005 de meme reference d'adhesion), le fichier ne dit pas
/// ce qui a ete declare apres lui, et on refuse (regle 1). Les nets : ceux de `precedent` plus les differences que le
/// versement porte (50.002, 50.004) ; le montant net social et les frais, ceux du rappel (58.004, 54). Le bloc 56 (la
/// regularisation du prelevement a la source) et le bloc 52 ne font pas partie de l'etat repris.
pub fn apres_correction(precedent: &VersementDeclare, chemin: &Path, mois: &str, nir: &str, numero: &str) -> Result<VersementDeclare, String> {
    let (fichier, b, affiliations) = versement_du_contrat(chemin, mois, nir, numero)?;
    let reference = |aff: &[(String, String)], id: &str| aff.iter().find(|(i, _)| i == id).map(|(_, r)| r.clone());
    let du_groupe = |l: &[(String, String)], g: &[&str]| -> Vec<(String, String)> {
        l.iter().filter(|(k, _)| g.iter().any(|x| k.get(8..10) == Some(*x))).cloned().collect()
    };
    let ecart = |g: &[&str], quoi: String| {
        format!(
            "dans la DSN {}, le versement de regularisation du contrat {} n'annule pas ligne pour ligne ce qui avait ete declare avant lui ({}, blocs {}) : {}. Ce qu'il a laisse de declare ne se lit pas",
            fichier,
            numero,
            precedent.fichier,
            g.join("+"),
            quoi
        )
    };
    if let Some((k, _)) = b.lignes.iter().find(|(k, _)| !k.starts_with("S21.G00.52.") && !k.starts_with("S21.G00.56.") && est_un_montant(k).is_none()) {
        return Err(format!("le versement de regularisation du contrat {} porte {} dans {}, que l'annulation ne sait pas reprendre", numero, k, fichier));
    }
    // Vague 1o : les blocs 58 dates du mois du versement sont des rappels de montant net social ; ceux de la periode
    // d'origine, s'il y en a, annulent et redeclarent le montant net social d'origine. Sans eux, il reste celui d'avant.
    let (b_lignes, rappel_ici) = separer_les_rappels_58(b.lignes.clone(), mois, &fichier)?;
    let mut lignes = Vec::new();
    for g in GROUPES {
        let avant = du_groupe(&precedent.lignes, g);
        let lus = du_groupe(&b_lignes, g);
        if g == ["58"] && lus.is_empty() {
            lignes.extend(avant);
            continue;
        }
        if lus.len() < avant.len() {
            return Err(ecart(g, format!("{} lignes pour en annuler {}", lus.len(), avant.len())));
        }
        for (i, ((ka, va), (kl, vl))) in avant.iter().zip(&lus).enumerate() {
            let pareil = if ka != kl {
                false
            } else if ka == "S21.G00.78.005" {
                reference(&precedent.affiliations, va).is_some() && reference(&precedent.affiliations, va) == reference(&affiliations, vl)
            } else if est_un_montant(ka) == Some(true) {
                matches!((Decimal::from_str(va), Decimal::from_str(vl)), (Ok(a), Ok(l)) if l == -a)
            } else {
                va == vl
            };
            if !pareil {
                return Err(ecart(g, format!("ligne {}, {} = {:?} pour annuler {} = {:?}", i + 1, kl, vl, ka, va)));
            }
        }
        lignes.extend(lus.into_iter().skip(avant.len()));
    }
    let brut = verifier_et_brut(&lignes, numero, &fichier)?;
    let net_imposable = precedent.net_imposable + entete(&b, "S21.G00.50.002", numero, &fichier)?;
    let net_verse = precedent.net_verse + entete(&b, "S21.G00.50.004", numero, &fichier)?;
    // Le montant net social d'origine et les frais sont ceux du rappel : il redeclare la paie corrigee entiere (le
    // montant net social d'origine reste celui d'avant quand la correction ne le redeclare pas, vague 1o).
    let net_social = net_social_de(&lignes, numero, &fichier)?;
    let rappels_net_social = precedent.rappels_net_social + rappel_ici;
    let frais = frais_de(&lignes, &fichier)?;
    let taux_pas = precedent.taux_pas.clone();
    Ok(VersementDeclare { fichier, net_imposable, net_verse, net_social, rappels_net_social, frais, brut, lignes, affiliations, taux_pas })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dsn(lignes: &[&str]) -> std::path::PathBuf {
        // Un fichier par appel : les tests tournent en parallele.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dossier = std::env::temp_dir().join(format!("versement-declare-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let p = dossier.join(format!("DSN_{}.dsn", n));
        std::fs::write(&p, lignes.join("\r\n")).unwrap();
        p
    }

    const AOUT: [&str; 34] = [
        "S20.G00.05.005,'01082026'",
        "S21.G00.30.001,'1990101999001'",
        "S21.G00.40.001,'01012024'",
        "S21.G00.40.009,'AB.24.01'",
        "S21.G00.70.012,'202'",
        "S21.G00.70.013,'106'",
        "S21.G00.50.001,'31082026'",
        "S21.G00.50.002,'900.00'",
        "S21.G00.50.003,'01'",
        "S21.G00.50.004,'800.00'",
        "S21.G00.51.001,'01082026'",
        "S21.G00.51.002,'31082026'",
        "S21.G00.51.010,'AB.24.01'",
        "S21.G00.51.011,'001'",
        "S21.G00.51.013,'1000.00'",
        "S21.G00.52.001,'026'",
        "S21.G00.52.002,'10.00'",
        "S21.G00.52.006,'AB.24.01'",
        "S21.G00.58.001,'01082026'",
        "S21.G00.58.002,'31082026'",
        "S21.G00.58.003,'03'",
        "S21.G00.58.004,'850.00'",
        "S21.G00.78.001,'31'",
        "S21.G00.78.002,'01082026'",
        "S21.G00.78.003,'31082026'",
        "S21.G00.78.004,'0.00'",
        "S21.G00.78.005,'202'",
        "S21.G00.78.006,'AB.24.01'",
        "S21.G00.81.001,'059'",
        "S21.G00.81.004,'12.00'",
        // un rappel d'un mois plus ancien, au meme contrat : ce n'est pas le versement d'aout
        "S21.G00.50.001,'31082026'",
        "S21.G00.51.001,'01062026'",
        "S21.G00.51.010,'AB.24.01'",
        "S21.G00.86.001,'07'",
    ];

    #[test]
    fn le_versement_du_mois_se_lit_avec_ses_affiliations_et_sans_le_rappel() {
        let v = lire(&dsn(&AOUT), "2026-08", "1990101999001", "AB.24.01").unwrap();
        // 50.004 (net verse) et 58.004 (montant net social) se lisent chacun ; sans bloc 54, aucun frais declare.
        assert_eq!((v.net_imposable, v.net_verse, v.net_social, v.frais, v.brut), (Decimal::from(900), Decimal::from(800), Decimal::from(850), Decimal::ZERO, Decimal::from(1000)));
        assert_eq!(v.affiliations, vec![("202".to_string(), "106".to_string())]);
        assert!(v.lignes.iter().all(|(k, _)| !k.starts_with("S21.G00.52")));
        assert_eq!(v.lignes.iter().filter(|(k, _)| k == "S21.G00.51.001").count(), 1);
        assert_eq!(v.blocs(&["78", "79", "81"]).count(), 8);
    }

    #[test]
    fn ce_qui_ne_se_rapproche_pas_refuse() {
        assert!(lire(&dsn(&AOUT), "2026-08", "1990101999001", "AB.24.02").unwrap_err().contains("aucun versement"));
        assert!(lire(&dsn(&AOUT), "2026-08", "2000000000000", "AB.24.01").unwrap_err().contains("aucun versement"));
        let mut deux: Vec<&str> = AOUT[..30].to_vec();
        deux.extend_from_slice(&AOUT[6..30]);
        assert!(lire(&dsn(&deux), "2026-08", "1990101999001", "AB.24.01").unwrap_err().contains("2 versements"));
        let mut melange: Vec<&str> = AOUT[..30].to_vec();
        melange.push("S21.G00.54.001,'92'");
        melange.push("S21.G00.54.005,'AB.24.09'");
        melange.push("S21.G00.54.003,'01082026'");
        assert!(lire(&dsn(&melange), "2026-08", "1990101999001", "AB.24.01").unwrap_err().contains("porte aussi le contrat AB.24.09"));
        let mut inconnue: Vec<&str> = AOUT[..30].to_vec();
        inconnue.push("S21.G00.81.008,'1.00'");
        assert!(lire(&dsn(&inconnue), "2026-08", "1990101999001", "AB.24.01").unwrap_err().contains("S21.G00.81.008"));
    }

    /// Septembre porte une correction du contrat d'aout : l'annulation du depot d'aout, puis le rappel.
    fn septembre(annule_81: &str) -> Vec<String> {
        let mut l: Vec<String> = [
            "S20.G00.05.005,'01092026'",
            "S21.G00.30.001,'1990101999001'",
            "S21.G00.40.001,'01012024'",
            "S21.G00.40.009,'AB.24.01'",
            "S21.G00.70.012,'205'",
            "S21.G00.70.013,'106'",
            "S21.G00.50.001,'30092026'",
            "S21.G00.50.002,'90.00'",
            "S21.G00.50.003,'01'",
            "S21.G00.50.004,'80.00'",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let remu = |m: &str| vec![
            "S21.G00.51.001,'01082026'".to_string(),
            "S21.G00.51.002,'31082026'".into(),
            "S21.G00.51.010,'AB.24.01'".into(),
            "S21.G00.51.011,'001'".into(),
            format!("S21.G00.51.013,'{}'", m),
        ];
        let net = |m: &str| vec!["S21.G00.58.001,'01082026'".to_string(), "S21.G00.58.002,'31082026'".into(), "S21.G00.58.003,'03'".into(), format!("S21.G00.58.004,'{}'", m)];
        let base31 = |m: &str| vec![
            "S21.G00.78.001,'31'".to_string(),
            "S21.G00.78.002,'01082026'".into(),
            "S21.G00.78.003,'31082026'".into(),
            "S21.G00.78.004,'0.00'".into(),
            "S21.G00.78.005,'205'".into(),
            "S21.G00.78.006,'AB.24.01'".into(),
            "S21.G00.81.001,'059'".into(),
            format!("S21.G00.81.004,'{}'", m),
        ];
        l.extend(remu("-1000.00"));
        l.extend(remu("1100.00"));
        l.push("S21.G00.56.001,'082026'".into());
        l.extend(net("-850.00"));
        l.extend(net("930.00"));
        l.extend(base31(annule_81));
        l.extend(base31("13.20"));
        l
    }

    #[test]
    fn apres_une_correction_portee_l_etat_declare_est_le_rappel() {
        let aout = lire(&dsn(&AOUT[..30]), "2026-08", "1990101999001", "AB.24.01").unwrap();
        let s = septembre("-12.00");
        let v = apres_correction(&aout, &dsn(&s.iter().map(String::as_str).collect::<Vec<_>>()), "2026-08", "1990101999001", "AB.24.01").unwrap();
        // 50.004 cumule (800 + 80) ; le montant net social est le 58.004 du rappel (930).
        assert_eq!((v.net_imposable, v.net_verse, v.net_social, v.brut), (Decimal::from(990), Decimal::from(880), Decimal::from(930), Decimal::from(1100)));
        assert_eq!(v.affiliations, vec![("205".to_string(), "106".to_string())]);
        assert_eq!(v.blocs(&["81"]).map(|(_, m)| m.as_str()).collect::<Vec<_>>(), ["059", "13.20"]);
        assert!(v.lignes.iter().all(|(k, _)| !k.starts_with("S21.G00.56")));
        // Une annulation qui ne retire pas exactement le depot : on ne sait pas ce qui reste declare.
        let faux = septembre("-11.00");
        let e = apres_correction(&aout, &dsn(&faux.iter().map(String::as_str).collect::<Vec<_>>()), "2026-08", "1990101999001", "AB.24.01").unwrap_err();
        assert!(e.contains("n'annule pas ligne pour ligne") && e.contains("S21.G00.81.004"), "{}", e);
    }

    #[test]
    fn les_frais_declares_se_lisent_en_bloc_54_de_type_07_08_09() {
        // Un repas (06, avantage en nature) n'est pas un frais ; deux defraiements (07 et 09) le sont.
        let mut l: Vec<&str> = AOUT[..18].to_vec();
        l.extend([
            "S21.G00.54.001,'07'", "S21.G00.54.002,'41.40'", "S21.G00.54.003,'01082026'", "S21.G00.54.004,'31082026'", "S21.G00.54.005,'AB.24.01'",
            "S21.G00.54.001,'06'", "S21.G00.54.002,'5.10'", "S21.G00.54.003,'01082026'", "S21.G00.54.004,'31082026'", "S21.G00.54.005,'AB.24.01'",
            "S21.G00.54.001,'09'", "S21.G00.54.002,'8.60'", "S21.G00.54.003,'01082026'", "S21.G00.54.004,'31082026'", "S21.G00.54.005,'AB.24.01'",
        ]);
        l.extend_from_slice(&AOUT[18..30]);
        let v = lire(&dsn(&l), "2026-08", "1990101999001", "AB.24.01").unwrap();
        assert_eq!((v.net_social, v.frais), (Decimal::from(850), Decimal::from(50)));
    }
}
