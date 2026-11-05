//! LES OBLIGATIONS QU'UN ARRET ET UNE FIN DE CONTRAT FONT NAITRE : ce qu'il faut faire, ou, dans quel delai,
//! a compter de quoi, jusqu'a quand, et ou on en est. UN SEUL ENDROIT les calcule ; les routes `arrets` et
//! `fins-de-contrat` (`routes::imprevus`) et le rappel de l'accueil (`routes::rappels`) ne font que les rendre, l'ecran ne
//! fait que les afficher (regle 2).
//!
//! Les regles sont des donnees : `logiciel/donnees/obligations-base.json` dit chaque obligation, son delai, son point de
//! depart et le texte qui la fonde -- tous deposes (son `_textes`) :
//!
//! - le signalement d'arret (04) et de reprise anticipee (05) : CSS R. 133-14, II
//!   (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`) et l'arrete du 2 juillet 2013, art. 1er
//!   (`atelier/reference/sources/arrete-2013-delai-evenements-dsn/xml/1.xml`) : cinq jours ouvres quand l'employeur
//!   n'est pas subroge ; avec la DSN mensuelle du mois quand il l'est. SAUF pour les motifs que la DSN ne couvre pas
//!   (`sauf_motifs` : le deuil d'enfant, 19 ; le temps partiel therapeutique, 15 a 18, non saisissable) : la page du compte
//!   entreprise de net-entreprises (`atelier/reference/sources/accident-du-travail/net-entreprises-compte-entreprise-dat-20260331.txt`)
//!   dit que l'Assurance Maladie n'en tire rien. La norme admet pourtant le motif 19 dans un signalement : `arret-signaler`
//!   ne le refuse pas, il avertit (`signalement::avertissement_hors_dsn`, appele par `signalement::verifier`) ;
//! - l'attestation de salaire pour les indemnites journalieres (DSIJ), HORS DSN, pour le deuil d'enfant (motif 19) : la
//!   meme page, « obligatoire pour tout arret de travail, quel qu'en soit le motif », « des reception de l'avis d'arret de
//!   travail », sur net-entreprises (ecran ou EDI) ; CSS L. 331-9 (`.../code-de-la-securite-sociale/xml/L331-9.xml`) pour les
//!   indemnites du conge de deuil de C. trav. L. 3142-1-1. « Des reception » n'est pas un delai chiffre : la date butoir est
//!   le jour meme de la connaissance de l'arret, et l'obligation le dit (`inferences`) ;
//! - le signalement de fin de contrat (07) : les memes textes, cinq jours ouvres apres la fin ;
//! - la declaration d'accident du travail ou de trajet (motifs 04 et 06), HORS DSN : CSS L. 441-2 et R. 441-3
//!   (`.../code-de-la-securite-sociale/xml/R441-3.xml`) : quarante-huit heures, dimanches et jours feries non compris, a
//!   compter de la connaissance de l'accident ; les reserves motivees, dix jours francs apres (R. 441-6) ;
//! - la maladie professionnelle (motif 05) n'est PAS declaree par l'employeur : par la victime, quinze jours apres la
//!   cessation du travail (L. 461-5, R. 461-5). Le moteur le dit, il n'en fait pas une obligation de la compagnie.
//!
//! CE QUI SE SAISIT, ET SE GARDE A COTE DES IMPREVUS (`imprevus/obligations.json`) : le jour (et l'heure) ou la compagnie a
//! connu l'arret ou l'accident, la declaration d'accident faite et l'attestation de salaire faite -- le jour, ou, la
//! reference. Rien de
//! cela ne se devine : sans connaissance saisie, le moteur prend le point de depart le plus exigeant et dit lequel
//! (`depart.presume`) ; sans declaration dite, l'accident n'est pas declare. Le fichier des arrets eux-memes
//! (`imprevus/AAAA-MM.json`, les imprevus de genre arret) n'est pas touche : ce module le lit par `crate::routes::imprevus::tous_les_arrets`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use droit::contexte::Contexte;
use socle::date::{en_texte, Jour};
use socle::ecran::Pastille;
use socle::echec::R;
pub use declarations::obligations::{CleDObligation, Ou, TexteDepose};
use declarations::obligations::{BaseDesObligations, DeBase};
use socle::echec::erreur;
use socle::fichiers::{repr, vrai};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

const SALARIE_ARRETE: Champ = Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé de la fiche du salarié arrêté", avance: false };
const DERNIER_JOUR: Champ = Champ { nom: "dernier_jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé de l'arrêt, AAAA-MM-JJ (il désigne l'arrêt)", avance: false };
const FAITE: &[Champ] = &[
    SALARIE_ARRETE,
    DERNIER_JOUR,
    Champ { nom: "le", libelle: Affiche::new("Fait le"), forme: Forme::Date, sens: "le jour où elle a été faite ; vide : la retirer", avance: false },
    Champ { nom: "depot", libelle: Affiche::new("Où"), forme: Forme::Texte, sens: "où elle a été faite (un des lieux que l'obligation propose)", avance: false },
    Champ { nom: "heure", libelle: Affiche::new("Heure"), forme: Forme::Texte, sens: "l'heure, HH:MM", avance: true },
    Champ { nom: "reference", libelle: Affiche::new("Référence de l'accusé"), forme: Forme::Texte, sens: "la référence de l'accusé", avance: true },
];
const OBLIGATIONS_REND: &str = "obligations (chacune son échéance, son état et ce qui reste à faire)";
const NON_ECRIT: Refus = Refus { classe: "ErreurOperation", corriger: "ce que la phrase dit (l'arrêt, le jour, le lieu) ; rien n'est écrit" };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ArretObligations,
        geste: "Dire ce qu'un arrêt en cours de saisie fera naître (signalement…).",
        charge: &[
            Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le motif de l'arrêt (code DSN)", avance: false },
            Champ { nom: "dernier_jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé", avance: false },
            Champ { nom: "debut", libelle: Affiche::new("Début de l'arrêt"), forme: Forme::Date, sens: "le premier jour que l'avis prescrit", avance: false },
            Champ { nom: "fin", libelle: Affiche::new("Fin prescrite"), forme: Forme::Date, sens: "la fin prescrite", avance: false },
            Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "le salarié : sa subrogation se tire de son contrat", avance: false },
            Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat suspendu, quand plusieurs couvrent le jour", avance: true },
            Champ { nom: "date_accident", libelle: Affiche::new("Jour de l'accident"), forme: Forme::Date, sens: "le jour de l'accident", avance: true },
        ],
        exemple: r#"{"motif":"01","dernier_jour":"2026-11-02","debut":"2026-11-03","fin":"2026-11-10","salarie":"SVOBODA"}"#,
        rend: OBLIGATIONS_REND,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ArretConnaissance,
        geste: "Dire le jour où la compagnie a connu un arrêt ou un accident.",
        charge: &[
            SALARIE_ARRETE,
            DERNIER_JOUR,
            Champ { nom: "de", libelle: Affiche::new("De"), forme: Forme::Texte, sens: "arret ou accident", avance: false },
            Champ { nom: "jour", libelle: Affiche::new("Jour"), forme: Forme::Date, sens: "le jour ; vide : le moteur reprend sa présomption", avance: false },
            Champ { nom: "heure", libelle: Affiche::new("Heure"), forme: Forme::Texte, sens: "l'heure, HH:MM", avance: true },
        ],
        exemple: r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","de":"arret","jour":"2026-11-02"}"#,
        rend: OBLIGATIONS_REND,
        refus: &[NON_ECRIT, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AccidentDeclare,
        geste: "Dire faite la déclaration d'un accident du travail ou de trajet.",
        charge: FAITE,
        exemple: r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","le":"2026-11-02","depot":"net-entreprises"}"#,
        rend: OBLIGATIONS_REND,
        refus: &[NON_ECRIT, MANQUE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AttestationDeclare,
        geste: "Dire faite l'attestation de salaire d'un arrêt que la DSN ne porte pas.",
        charge: FAITE,
        exemple: r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-16","le":"","depot":"net-entreprises-ecran"}"#,
        rend: OBLIGATIONS_REND,
        refus: &[NON_ECRIT, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

pub const A_FAIRE: &str = "a_faire";
pub const EN_RETARD: &str = "en_retard";
pub const FAIT: &str = "fait";
pub const A_VENIR: &str = "a_venir";
pub const INDETERMINE: &str = "indetermine";
pub const A_LA_CHARGE_DU_SALARIE: &str = "a_la_charge_du_salarie";

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

// ------------------------------------------------------------------ les donnees

/// `obligations-base.json`, lu par sa forme (`declarations::obligations::base`, son seul lecteur).
fn base() -> R<BaseDesObligations> {
    declarations::obligations::base().map_err(|m| erreur("ValueError", m))
}

/// Un nombre que le delai doit porter.
fn nombre(n: Option<i64>, quoi: &str) -> R<i64> {
    n.ok_or_else(|| erreur("KeyError", declarations::obligations::livree(quoi)))
}

/// Le motif d'arret appelle-t-il l'obligation `cle` (ses `motifs`) ?
fn motif_appelle(cle: CleDObligation, motif: &str) -> R<bool> {
    Ok(base()?.obligations.de(cle).motifs.iter().any(|m| m == motif))
}

/// Le motif d'arret appelle-t-il la declaration d'accident (`declaration_accident.motifs`) ?
pub fn motif_appelle_la_declaration(motif: &str) -> R<bool> {
    motif_appelle(CleDObligation::DeclarationAccident, motif)
}

/// Un signalement d'arret (04) est-il du pour ce motif ? Non pour ceux que la DSN ne couvre pas
/// (`signalement_arret.sauf_motifs`). La liste des arrets s'en sert pour ne pas dire « a signaler » ce qui ne se signale pas.
pub fn signalement_du(motif: &str) -> R<bool> {
    Ok(!base()?.obligations.signalement_arret.sauf().iter().any(|m| m == motif))
}

// --------------------------------------------------------------- le calendrier

fn jour(v: &Value) -> Option<Jour> {
    v.as_str().filter(|s| !s.is_empty()).and_then(|s| Jour::lire(s).ok())
}

fn jour_dit(t: &str) -> Option<Jour> {
    Some(t).filter(|s| !s.is_empty()).and_then(|s| Jour::lire(s).ok())
}

fn plus(j: Jour, n: i64) -> R<Jour> {
    j.plus_jours(n).map_err(|m| erreur("OverflowError", m))
}

/// Les feries chomes de la compagnie, annee par annee (`feries::chomes`, la fiche Entreprise).
struct Calendrier<'a> {
    dits: Option<&'a str>,
    chomes: HashMap<i32, Vec<Jour>>,
    legaux: HashMap<i32, Vec<Jour>>,
}

impl<'a> Calendrier<'a> {
    fn nouveau(dits: Option<&'a str>) -> Calendrier<'a> {
        Calendrier { dits, chomes: HashMap::new(), legaux: HashMap::new() }
    }

    fn chome(&mut self, j: Jour) -> R<bool> {
        if !self.chomes.contains_key(&j.an) {
            let f = socle::feries::chomes(j.an, self.dits).map_err(|m| erreur("ValueError", m))?;
            self.chomes.insert(j.an, f.into_iter().map(|(d, _)| d).collect());
        }
        Ok(self.chomes[&j.an].contains(&j))
    }

    fn ferie(&mut self, j: Jour) -> R<bool> {
        if !self.legaux.contains_key(&j.an) {
            let f = socle::feries::feries(j.an).map_err(|m| erreur("ValueError", m))?;
            self.legaux.insert(j.an, f.into_iter().map(|(d, _)| d).collect());
        }
        Ok(self.legaux[&j.an].contains(&j))
    }

    /// Le `n`-ieme jour ouvre qui suit `depart` : ni samedi, ni dimanche, ni ferie chome dans la compagnie ; le jour de
    /// depart ne compte pas (`obligations-base.json`, `jours_ouvres`).
    fn jours_ouvres_apres(&mut self, depart: Jour, n: i64) -> R<Jour> {
        let (mut j, mut reste) = (depart, n);
        while reste > 0 {
            j = plus(j, 1)?;
            if j.jour_de_la_semaine() < 5 && !self.chome(j)? {
                reste -= 1;
            }
        }
        Ok(j)
    }

    /// Un jour qui compte dans « quarante-huit heures non compris les dimanches et jours feries » (R. 441-3) : ni
    /// dimanche, ni l'une des onze fetes legales.
    fn compte_dans_les_heures(&mut self, j: Jour) -> R<bool> {
        Ok(j.jour_de_la_semaine() != 6 && !self.ferie(j)?)
    }

    /// `heures` heures a compter de `depart` a `minute` (minutes depuis minuit), dimanches et feries non compris. Rend le
    /// jour et la minute ou le delai expire (1440 : a la fin de ce jour-la). Un depart un jour qui ne compte pas court
    /// des minuit du premier jour qui compte.
    fn heures_apres(&mut self, depart: Jour, minute: u32, heures: i64) -> R<(Jour, u32)> {
        let (mut j, mut m) = (depart, minute as i64);
        while !self.compte_dans_les_heures(j)? {
            j = plus(j, 1)?;
            m = 0;
        }
        let mut reste = heures * 60;
        loop {
            let dans_ce_jour = 1440 - m;
            if reste <= dans_ce_jour {
                return Ok((j, (m + reste) as u32));
            }
            reste -= dans_ce_jour;
            j = plus(j, 1)?;
            while !self.compte_dans_les_heures(j)? {
                j = plus(j, 1)?;
            }
            m = 0;
        }
    }
}

// ---------------------------------------------------------------- les phrases

fn lisible(j: &Jour) -> String {
    format!("{} {}", JOURS[j.jour_de_la_semaine() as usize], j.jj_mm_aaaa())
}

fn heure_lisible(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

fn s(n: i64) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// `HH:MM` -> minutes depuis minuit.
fn lire_heure(h: &str) -> Option<u32> {
    let (a, b) = h.trim().split_once(':')?;
    let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
    (a < 24 && b < 60 && a.to_string().len() <= 2 && b.to_string().len() <= 2).then_some(a * 60 + b)
}

// ------------------------------------------------------------ ce qui est saisi

/// Ce que la compagnie a dit des obligations de ses arrets : `imprevus/obligations.json`, a cote du magasin des imprevus.
fn chemin(ctx: &Contexte) -> PathBuf {
    engagement::imprevus::dossier(&ctx.racine).join("obligations.json")
}

/// Ce que le fichier de suivi dit de lui-meme.
const QUOI_DU_SUIVI: &str = "Ce que la compagnie a dit des obligations de ses arrets de travail : le jour ou elle a connu l'arret ou l'accident, la declaration d'accident du travail faite et l'attestation de salaire faite (hors DSN, le deuil d'enfant). Ecrit par les routes `arret-enregistrer` (avec `connu`), `arret-connaissance`, `accident-declare` et `attestation-declare` (`atelier/sources/moteur/src/obligations.rs`) ; les arrets eux-memes sont dans `AAAA-MM.json`.";

/// `imprevus/obligations.json` : ce qu'il est, et une fiche par arret dont la compagnie a dit quelque chose.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FichierDuSuivi {
    _quoi: String,
    arrets: Vec<FicheDeSuivi>,
}

/// La fiche de suivi d'un arret (la cle du salarie et le dernier jour travaille le designent) : le jour ou la compagnie a
/// connu l'arret, l'accident, et les obligations hors DSN qu'elle dit faites.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FicheDeSuivi {
    salarie: String,
    dernier_jour_travaille: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    connaissance_arret: Option<ConnaissanceSaisie>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    connaissance_accident: Option<ConnaissanceSaisie>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accident_declare: Option<ObligationFaite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attestation_salaire: Option<ObligationFaite>,
}

impl FicheDeSuivi {
    /// Une fiche qui ne dit plus rien s'en va.
    fn dit_quelque_chose(&self) -> bool {
        self.connaissance_arret.is_some() || self.connaissance_accident.is_some() || self.accident_declare.is_some() || self.attestation_salaire.is_some()
    }

    fn connaissance(&self, de: DeQuoi) -> Option<&ConnaissanceSaisie> {
        match de {
            DeQuoi::Arret => self.connaissance_arret.as_ref(),
            DeQuoi::Accident => self.connaissance_accident.as_ref(),
        }
    }
}

/// Le jour (et l'heure) ou la compagnie a connu l'arret ou l'accident, et quand elle l'a dit.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnaissanceSaisie {
    jour: String,
    heure: Option<String>,
    saisi_le: String,
}

/// Ce que la compagnie a connu : l'arret, ou l'accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DeQuoi {
    Arret,
    Accident,
}

/// Les fiches de suivi, par (cle du salarie, dernier jour travaille).
pub struct Suivi {
    fiches: Vec<FicheDeSuivi>,
}

impl Suivi {
    /// `imprevus/obligations.json`, lu par sa forme ; aucune fiche s'il n'existe pas.
    pub fn lire(ctx: &Contexte) -> R<Suivi> {
        let p = chemin(ctx);
        if !p.exists() {
            return Ok(Suivi { fiches: Vec::new() });
        }
        let v = socle::fichiers::lire_json_echec(&p)?;
        let f: FichierDuSuivi = socle::forme::lire(&v, "Le fichier des obligations des imprévus (imprevus/obligations.json) ne se lit pas.").map_err(|m| erreur("ValueError", m))?;
        Ok(Suivi { fiches: f.arrets })
    }

    fn de(&self, salarie: &str, dernier_jour: &str) -> FicheDeSuivi {
        self.fiches.iter().find(|f| f.salarie == salarie && f.dernier_jour_travaille == dernier_jour).cloned().unwrap_or_default()
    }

    /// Change la fiche de l'arret (`changer`), puis ecrit le fichier ; une fiche qui ne dit plus rien s'en va.
    fn ecrire(&mut self, ctx: &Contexte, salarie: &str, dernier_jour: &str, changer: impl FnOnce(&mut FicheDeSuivi)) -> R<()> {
        let i = match self.fiches.iter().position(|f| f.salarie == salarie && f.dernier_jour_travaille == dernier_jour) {
            Some(i) => i,
            None => {
                self.fiches.push(FicheDeSuivi { salarie: salarie.to_string(), dernier_jour_travaille: dernier_jour.to_string(), ..FicheDeSuivi::default() });
                self.fiches.len() - 1
            }
        };
        changer(&mut self.fiches[i]);
        self.fiches.retain(FicheDeSuivi::dit_quelque_chose);
        let doc = FichierDuSuivi { _quoi: QUOI_DU_SUIVI.to_string(), arrets: std::mem::take(&mut self.fiches) };
        let ecrit = socle::ecriture::ecrire_json(&chemin(ctx), &crate::routes::en_json(&doc)?, 2).map_err(|m| erreur("OSError", m));
        self.fiches = doc.arrets;
        ecrit
    }
}

// ---------------------------------------------------------- une obligation

/// Le point de depart d'un delai : le jour, l'heure s'il y en a une, ce que c'est, et s'il est saisi ou presume.
struct Depart {
    jour: Jour,
    minute: Option<u32>,
    quoi: String,
    presume: Option<String>,
}

/// Le point de depart d'un delai, tel que l'obligation le rend.
#[derive(Debug, Clone, Serialize)]
pub struct DepartDit {
    jour: String,
    heure: Option<String>,
    quoi: String,
    saisi: bool,
    presume: Option<String>,
}

impl Depart {
    fn dit(&self) -> DepartDit {
        DepartDit { jour: self.jour.iso(), heure: self.minute.map(heure_lisible), quoi: self.quoi.clone(), saisi: self.presume.is_none(), presume: self.presume.clone() }
    }
}

/// Ce qui est fait : le jour, comment, et ce que l'obligation en rend.
struct Fait {
    le: Jour,
    comment: String,
    dit: FaitDit,
}

/// Ce qui est fait, tel que l'obligation le rend : un depot que le journal porte, ou ce que la compagnie a dit fait (la
/// fiche de suivi).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum FaitDit {
    Depose(DepotFait),
    Dit(ObligationFaite),
}

/// Un depot du journal qui fait une obligation : quand, son fichier, son ordre ou son flux, et le jour.
#[derive(Debug, Clone, Serialize)]
pub struct DepotFait {
    quand: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordre: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flux: Option<String>,
    comment: &'static str,
    le: String,
}

impl FaitDit {
    pub fn le(&self) -> &str {
        match self {
            FaitDit::Depose(d) => &d.le,
            FaitDit::Dit(o) => &o.le,
        }
    }
}

/// Le jour ou la compagnie l'a su, la ou le delai en court : ce que la page demande (`arret-connaissance`, `de`).
#[derive(Debug, Clone, Serialize)]
pub struct ConnaissanceDemandee {
    pub de: &'static str,
    titre: &'static str,
}

/// Une suite de la declaration d'accident (les reserves, la feuille d'accident) : ce qu'elle est, son texte, jusqu'a
/// quand, et sa phrase.
#[derive(Debug, Clone, Serialize)]
pub struct Suite {
    cle: &'static str,
    libelle: String,
    texte: TexteDepose,
    #[serde(skip_serializing_if = "Option::is_none")]
    inference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jusqu_au: Option<String>,
    phrase: String,
}

/// UNE OBLIGATION, comptee au jour dit : ce qu'elle est (libelle, d'ou elle nait, qui, ou, a qui, ses textes, ses
/// exceptions, ses inferences, ce qu'il faut avoir), son delai et son point de depart, sa date butoir, ou on en est (etat,
/// si elle est due, sa pastille, ce qui est fait), sa phrase, et ce que l'ecran en dit (`butoir_dit`, `delai_dit`,
/// `connaissance`).
#[derive(Debug, Clone, Serialize)]
pub struct Obligation {
    pub cle: CleDObligation,
    pub libelle: String,
    nait_de: String,
    qui: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ou: Option<Ou>,
    destinataire: String,
    textes: Vec<TexteDepose>,
    exceptions: Vec<String>,
    inferences: Vec<String>,
    a_avoir: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subrogation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    echeance_dsn: Option<declarations::echeance_dsn::EcheanceDite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suites: Option<Vec<Suite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delai: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depart: Option<DepartDit>,
    pub butoir: Option<String>,
    butoir_heure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dans_les_temps: Option<bool>,
    pub etat: &'static str,
    pub du: bool,
    pub pastille: Pastille,
    priorite: u8,
    jours_restants: Option<i64>,
    pub fait: Option<FaitDit>,
    pub phrase: String,
    pub butoir_dit: Option<String>,
    delai_dit: Option<String>,
    pub connaissance: Option<ConnaissanceDemandee>,
}

impl Obligation {
    /// L'obligation `cle` telle que la base la dit, avant d'etre comptee : son delai ne se compte pas encore.
    fn squelette(cle: CleDObligation, o: &DeBase) -> Obligation {
        let mut ob = Obligation {
            cle,
            libelle: o.libelle.clone(),
            nait_de: o.nait_de.clone(),
            qui: o.qui.clone(),
            ou: o.ou.clone(),
            destinataire: o.destinataire.clone(),
            textes: o.textes.clone(),
            exceptions: o.exceptions.clone(),
            inferences: o.inferences.clone(),
            a_avoir: o.a_avoir.clone(),
            subrogation: None,
            echeance_dsn: None,
            suites: None,
            delai: None,
            depart: None,
            butoir: None,
            butoir_heure: None,
            dans_les_temps: None,
            etat: INDETERMINE,
            du: false,
            pastille: Pastille::de(A_FAIRE, ""),
            priorite: 1,
            jours_restants: None,
            fait: None,
            phrase: String::new(),
            butoir_dit: None,
            delai_dit: None,
            connaissance: None,
        };
        ob.poser_l_etat(INDETERMINE);
        ob
    }

    /// Ce que l'ecran dit d'une obligation dans l'etat `etat` : est-elle DUE par la compagnie (en retard, a faire, un
    /// delai qui ne se compte pas encore faute d'un fait), et sa pastille (`routes::pastille`).
    fn poser_l_etat(&mut self, etat: &'static str) {
        let (du, couleur, mots) = match etat {
            EN_RETARD => (true, EN_RETARD, "en retard"),
            A_FAIRE => (true, A_FAIRE, "à faire"),
            INDETERMINE => (true, A_FAIRE, "délai à préciser"),
            FAIT => (false, FAIT, "fait"),
            A_VENIR => (false, "information", "à venir"),
            _ => (false, "information", "à la charge du salarié"),
        };
        self.etat = etat;
        self.du = du;
        self.pastille = Pastille::de(couleur, mots);
        // Le plus presse d'abord, la ou on le montre : ce qui est en retard, puis le reste (par date butoir).
        self.priorite = if etat == EN_RETARD { 0 } else { 1 };
    }

    /// Une obligation dont le delai ne se compte pas : pourquoi.
    fn indeterminee(mut self, pourquoi: &str) -> Obligation {
        self.poser_l_etat(INDETERMINE);
        self.phrase = pourquoi.to_string();
        self
    }

    /// Pose l'etat, la date butoir et la phrase d'une obligation dont on connait le depart et la butoir.
    fn conclure(&mut self, aujourdhui: Jour, depart: Option<&Depart>, butoir: Jour, butoir_minute: Option<u32>, delai: &str, fait: Option<Fait>) {
        let quand_butoir = match butoir_minute {
            Some(1440) => format!("le {}, jusqu'à minuit", lisible(&butoir)),
            Some(720) => format!("le {} à midi", lisible(&butoir)),
            Some(mn) => format!("le {} à {}", lisible(&butoir), heure_lisible(mn)),
            None => format!("le {}", lisible(&butoir)),
        };
        self.delai = Some(delai.to_string());
        if let Some(d) = depart {
            self.depart = Some(d.dit());
        }
        self.butoir = Some(butoir.iso());
        self.butoir_heure = butoir_minute.map(|mn| if mn == 1440 { "24:00".to_string() } else { heure_lisible(mn) });
        let restants = butoir.moins(&aujourdhui);
        let (etat, phrase) = match &fait {
            Some(f) => {
                let dans_les_temps = f.le <= butoir;
                self.dans_les_temps = Some(dans_les_temps);
                (
                    FAIT,
                    format!(
                        "Fait le {} ({}){}.",
                        f.le.jj_mm_aaaa(),
                        f.comment,
                        if dans_les_temps { String::new() } else { format!(", après la date butoir du {}", butoir.jj_mm_aaaa()) }
                    ),
                )
            }
            None if depart.is_some_and(|d| d.jour > aujourdhui) => (A_VENIR, format!("À venir : {}, soit au plus tard {}.", delai, quand_butoir)),
            None if restants >= 0 => (
                A_FAIRE,
                format!(
                    "À faire au plus tard {}{}.",
                    quand_butoir,
                    match restants {
                        0 => " : c'est aujourd'hui".to_string(),
                        n => format!(" : il reste {} jour{}", n, s(n)),
                    }
                ),
            ),
            None => (EN_RETARD, format!("En retard de {} jour{} : c'était à faire au plus tard {}.", -restants, s(-restants), quand_butoir)),
        };
        self.poser_l_etat(etat);
        self.jours_restants = if fait.is_some() { None } else { Some(restants) };
        self.fait = fait.map(|f| f.dit);
        self.phrase = phrase;
    }

    /// La date butoir telle que l'ecran la lit (`butoir_dit`, `butoir_en_clair`) ; absente sans butoir. Et le delai et
    /// son point de depart, en une phrase (`delai_dit`).
    fn dire_le_butoir(mut self) -> Obligation {
        self.butoir_dit = self.butoir_en_clair();
        self.delai_dit = self.delai_en_clair();
        // LE JOUR OU LA COMPAGNIE L'A SU se dit la ou le delai en court (`arret-connaissance`) : l'accident, et le
        // signalement d'un arret sans subrogation -- subroge, il court du mois de l'arret.
        let connaissance = match self.cle {
            CleDObligation::SignalementArret => Some(("arret", "Connaissance de l'arrêt")),
            CleDObligation::DeclarationAccident => Some(("accident", "Connaissance de l'accident")),
            _ => None,
        };
        self.connaissance = match connaissance {
            Some((de, titre)) if self.depart.is_some() && self.subrogation != Some(true) => Some(ConnaissanceDemandee { de, titre }),
            _ => None,
        };
        self
    }

    /// « au plus tard le 16/10/2026 à 10:00 » : la date butoir (`butoir`, `butoir_heure`), telle que l'ecran la dit ;
    /// `None` sans butoir.
    pub(crate) fn butoir_en_clair(&self) -> Option<String> {
        let jour = self.butoir.as_deref().filter(|b| !b.is_empty())?;
        let heure = self.butoir_heure.as_deref().filter(|h| !h.is_empty() && *h != "24:00").map(|h| format!(" à {}", h)).unwrap_or_default();
        Some(format!("au plus tard le {}{}", jour_en_clair(jour), heure))
    }

    /// « Dans les 48 heures — à compter de la connaissance de l'arrêt, le 02/11/2026. » : le delai et son point de
    /// depart, tels que l'ecran les dit ; `None` sans delai.
    fn delai_en_clair(&self) -> Option<String> {
        let d = self.delai.as_deref().map(str::trim).filter(|d| !d.is_empty())?;
        let mut p: String = d.chars().take(1).flat_map(char::to_uppercase).chain(d.chars().skip(1)).collect();
        if let Some(dep) = self.depart.as_ref().filter(|dep| !dep.quoi.is_empty()) {
            // « à compter du jour », « de la reprise » : la preposition prend l'article du point de depart.
            let quoi = dep.quoi.strip_prefix("le ").map(|r| format!("du {}", r)).unwrap_or_else(|| format!("de {}", dep.quoi));
            p.push_str(&format!(" — à compter {}", quoi));
            if !dep.jour.is_empty() {
                p.push_str(&format!(", le {}", jour_en_clair(&dep.jour)));
            }
        }
        p.push('.');
        Some(p)
    }
}

/// « 02/11/2026 » : un jour ISO tel qu'une phrase le dit ; ce qui n'en est pas un, tel quel.
fn jour_en_clair(iso: &str) -> String {
    Jour::lire(iso).map(|j| j.jj_mm_aaaa()).unwrap_or_else(|_| iso.to_string())
}

/// CE QUI EST PARTI D'UN ARRET, tel que le journal des envois le porte : un signalement (04, 05) ou une DSN mensuelle
/// (01) qui l'a reporte -- sa nature, son type, son ordre, le mois de la DSN, son fichier, quand.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnvoiDeLArret {
    pub nature: String,
    #[serde(rename = "type")]
    pub type_declaration: String,
    ordre: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<String>,
    fichier: String,
    pub quand: String,
}

/// Le premier envoi de cette nature parti pour l'arret (`ce_qui_est_parti`).
fn envoi(envois: &[EnvoiDeLArret], nature: &str) -> Option<Fait> {
    let e = envois.iter().filter(|e| e.nature == nature && e.quand.len() >= 10).min_by(|a, b| a.quand.cmp(&b.quand))?;
    let le = Jour::lire(&e.quand[..10]).ok()?;
    Some(Fait {
        le,
        comment: "déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
        dit: FaitDit::Depose(DepotFait { quand: e.quand.clone(), fichier: Some(e.fichier.clone()), ordre: Some(e.ordre), flux: None, comment: "depot", le: le.iso() }),
    })
}

/// LE SIGNALEMENT RECTIFICATIF EST-IL DU ? Quand un avis de prolongation (`prolongations`, `crate::routes::imprevus::arret_prolonger`)
/// a change la fin de subrogation APRES le premier depot du signalement d'arret (04) : la subrogation partie etait
/// « insuffisante », et « l'employeur doit alors transmettre un nouveau signalement de nature " arret de travail " de type
/// annule et remplace, dans lequel une nouvelle date de fin de subrogation sera renseignee » (fiche net-entreprises 1983,
/// `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`,
/// l. 37). Le depart : le jour ou la derniere de ces prolongations a ete sue (`le`). Fait : un signalement d'arret de type
/// 03 (annule et remplace) depose apres sa saisie. L'ordre se lit sur les heures de saisie et de depot, toutes deux a
/// l'horloge (`saisi_le`, `quand`).
fn rectificatif_du(a: &Value, envois: &[EnvoiDeLArret]) -> Option<(Depart, Option<Fait>)> {
    let signalements: Vec<&EnvoiDeLArret> = envois.iter().filter(|e| e.nature == "04").collect();
    let premier = signalements.iter().map(|e| e.quand.clone()).filter(|q| q.len() >= 10).min()?;
    let texte = |v: &Value, k: &str| v.get(k).map(en_texte).unwrap_or_default();
    let p = a
        .get("prolongations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| p.get("subrogation_fin") != p.get("subrogation_fin_avant") && texte(p, "saisi_le") > premier)
        .last()?;
    let saisi = texte(p, "saisi_le");
    let depart = Depart { jour: jour(p.get("le")?)?, minute: None, quoi: "le jour où la prolongation de la subrogation a été saisie".into(), presume: None };
    let fait = signalements
        .iter()
        .filter(|e| e.type_declaration == declarations::signalement::ANNULE_ET_REMPLACE && e.quand >= saisi)
        .map(|e| e.quand.clone())
        .min()
        .and_then(|quand| {
            let le = Jour::lire(quand.get(..10)?).ok()?;
            Some(Fait {
                le,
                comment: "annule et remplace déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
                dit: FaitDit::Depose(DepotFait { quand, fichier: None, ordre: None, flux: None, comment: "depot", le: le.iso() }),
            })
        });
    Some((depart, fait))
}

/// Ce que la compagnie a dit fait d'une obligation hors DSN (`accident_declare`, `attestation_salaire` de sa fiche de
/// suivi) : le jour, et ou (le libelle court du code de `o.ou`).
fn dit_fait(faite: Option<&ObligationFaite>, o: &DeBase) -> Option<Fait> {
    let f = faite?;
    let le = jour_dit(&f.le)?;
    let libelle = o.lieux().iter().find(|x| x.code == f.depot).map(|x| x.court.clone()).unwrap_or_else(|| f.depot.clone());
    Some(Fait { le, comment: format!("dit par la compagnie : {}", libelle), dit: FaitDit::Dit(f.clone()) })
}

/// Un signalement DSN (04 ou 05) : cinq jours ouvres sans subrogation, avec la mensuelle du mois avec.
#[allow(clippy::too_many_arguments)]
fn signalement_dsn(
    ctx: &Contexte,
    cal: &mut Calendrier,
    aujourdhui: Jour,
    cle: CleDObligation,
    o: &DeBase,
    subrogation: &Value,
    depart: Depart,
    evenement: Jour,
    fait: Option<Fait>,
) -> R<Obligation> {
    let mut m = Obligation::squelette(cle, o);
    if !vrai(subrogation) {
        return Ok(m.indeterminee(
            "La subrogation de cet arrêt n'est pas dite : c'est elle qui fixe le délai (cinq jours ouvrés sans subrogation, avec la DSN du mois avec ; CSS R. 133-14, II).",
        ));
    }
    if en_texte(subrogation) == droit::arrets::bloc_60().map_err(|m| erreur("ValueError", m))?.subrogation_oui().map_err(|m| erreur("KeyError", m))? {
        let d = o.delai_de(&o.delai_avec_subrogation, "delai_avec_subrogation")?;
        // SUBROGE, la date butoir est l'echeance de la DSN mensuelle du mois de l'evenement (R. 133-14, II, premier
        // alinea), telle que `echeance_dsn` la donne : reportee au jour ouvre suivant un samedi, un dimanche ou un ferie
        // (R. 133-14, I), a midi (cahier technique § 1.4.2). INFERENCE : le report de la mensuelle vaut pour le signalement
        // qui part avec elle (le texte du report parle de « la declaration sociale nominative » sans distinguer).
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, &evenement.mois_civil().to_string()).map_err(|m| erreur("ValueError", m))?;
        m.subrogation = Some(true);
        m.echeance_dsn = Some(echeance.dite());
        // La connaissance ne compte pas : le delai court du mois de l'evenement.
        let ev = Depart { jour: evenement, minute: None, quoi: "le jour de l'événement (sa DSN mensuelle est celle de son mois)".into(), presume: None };
        m.conclure(aujourdhui, Some(&ev), echeance.jour, Some(declarations::echeance_dsn::MINUTE_LIMITE), &d.dit, fait);
    } else {
        let d = o.delai_de(&o.delai_sans_subrogation, "delai_sans_subrogation")?;
        let butoir = cal.jours_ouvres_apres(depart.jour, nombre(d.jours_ouvres, "delai_sans_subrogation.jours_ouvres")?)?;
        m.subrogation = Some(false);
        m.conclure(aujourdhui, Some(&depart), butoir, None, &d.dit, fait);
    }
    Ok(m)
}

/// La connaissance saisie de l'arret ou de l'accident, sinon `None`.
fn connaissance(fiche: &FicheDeSuivi, de: DeQuoi, quoi: &str) -> Option<Depart> {
    let c = fiche.connaissance(de)?;
    let j = jour_dit(&c.jour)?;
    Some(Depart { jour: j, minute: c.heure.as_deref().and_then(lire_heure), quoi: quoi.into(), presume: None })
}

// ------------------------------------------------------------- les arrets

/// CE QUI EST PARTI DE L'IMPREVU `a`, lu au journal des envois : les signalements de l'arret (une ligne `dsn` de nature
/// 04 ou 05 dont l'evenement, `mois`, est le dernier jour travaille que l'arret declare, S21.G00.60.002, et qui couvre un
/// engagement du salarie, `versions`) et les DSN mensuelles des mois qu'il traverse deposees apres sa saisie (elles le
/// reportent). Seul compte un depot reel accepte : un signalement refuse au bilan n'est pas recu (cahier technique DSN,
/// `norme-dsn/texte/chapitre-4-controles.txt` §4.2) et reste du.
pub(crate) fn ce_qui_est_parti(ctx: &Contexte, a: &Value) -> R<Vec<EnvoiDeLArret>> {
    let lu = envois::journal::lu(&ctx.racine).map_err(|m| erreur("OSError", m))?;
    let (lignes, departs) = (lu.lignes, lu.departs);
    let arret = declarations::dsn::Arret { v: a.clone() };
    let texte = |k: &str| a.get(k).map(en_texte).unwrap_or_default();
    let salarie = texte("salarie");
    let dj = texte("dernier_jour_travaille");
    let declare = engagement::imprevus::dernier_jour_declare(a).map(|j| j.iso()).unwrap_or_default();
    let traverses: Vec<String> = match (engagement::imprevus::dernier_jour_declare(a), engagement::imprevus::dernier_jour(a)) {
        (Ok(p), Ok(d)) if p <= d => socle::mois::Mois::entre(&p, &d).iter().map(socle::mois::Mois::to_string).collect(),
        _ => Vec::new(),
    };
    let saisi = texte("saisi_le");
    // Une DSN mensuelle a reporte l'imprevu : celle d'un mois qu'il traverse, partie apres sa saisie ; ou, un arret, la DSN suivante
    // qui reporte l'arret saisi apres le depart de celles de ses mois (`Arret::a_reporter`).
    let reporte = |mois: &str, quand: &str| -> R<bool> {
        if traverses.iter().any(|m| m == mois) {
            return Ok(quand.replace('T', " ") >= saisi);
        }
        if !engagement::imprevus::est_un_arret(a) {
            return Ok(false);
        }
        arret.a_reporter(mois, &departs).map_err(|m| erreur("ValueError", m))
    };
    let mut partis = Vec::new();
    for e in envois::journal::filtrer(&lignes, Some(envois::journal::Acte::Dsn), None, true).filter(|l| l.accepte() && !envois::etats::refuse_par_un_retour(l)).filter_map(envois::journal::Ligne::dsn) {
        let (nom, mois, quand) = (e.fichier.clone().unwrap_or_default(), e.mois.map(|m| m.to_string()).unwrap_or_default(), e.quand.clone());
        let d = &e.detail;
        let (nature, type_declaration) = (d.nature.clone().unwrap_or_default(), d.type_declaration.clone().unwrap_or_default());
        let ordre = d.ordre.as_deref().unwrap_or("").trim().parse::<i64>().unwrap_or(0);
        let du_salarie = e.versions.iter().flatten().any(|x| x.salarie.as_str() == salarie);
        if matches!(nature.as_str(), "04" | "05") && dj.len() == 10 && d.evenement.as_deref() == Some(declare.as_str()) && du_salarie {
            partis.push(EnvoiDeLArret { fichier: format!("sorties/{}/{}", &dj[..7], nom), nature, type_declaration, ordre, mois: None, quand });
        } else if nature == "01" && reporte(&mois, &quand)? {
            partis.push(EnvoiDeLArret { fichier: format!("sorties/{}/{}", mois, nom), nature, type_declaration, ordre, mois: Some(mois), quand });
        }
    }
    Ok(partis)
}

/// Les obligations d'un arret (`a` : l'arret tel que `imprevus/AAAA-MM.json` le porte), dans l'ordre ou elles se font,
/// comptees au jour `aujourdhui` d'apres ce qui en est parti (`ce_qui_est_parti`).
pub fn de_l_arret(ctx: &Contexte, a: &Value, suivi: &Suivi, aujourdhui: Jour) -> R<Vec<Obligation>> {
    let envois = ce_qui_est_parti(ctx, a)?;
    obligations_de(ctx, a, &envois, suivi, aujourdhui)
}

/// Les obligations de l'arret `a`, sachant ce qui en est parti (`envois`).
fn obligations_de(ctx: &Contexte, a: &Value, envois: &[EnvoiDeLArret], suivi: &Suivi, aujourdhui: Jour) -> R<Vec<Obligation>> {
    let n = base()?.obligations;
    let mut cal = Calendrier::nouveau(ctx.entreprise.feries_chomes());
    let mut out = Vec::new();
    let texte = |k: &str| a.get(k).map(en_texte).unwrap_or_default();
    let dernier = texte("dernier_jour_travaille");
    let fiche = suivi.de(&texte("salarie"), &dernier);
    let motif = texte("motif");
    let subrogation = a.get("subrogation").cloned().unwrap_or(Value::Null);
    // L'evenement que le signalement declare est « le debut [...] de l'arret de travail » (CSS R. 133-13, I, 1°) : le
    // premier jour que l'avis prescrit (`imprevus::premier_jour`). La maladie professionnelle compte depuis « la
    // cessation du travail » (L. 461-5) : le lendemain du dernier jour travaille.
    let debut = engagement::imprevus::premier_jour(a).ok();
    let cessation = match jour_dit(&dernier) {
        Some(j) => Some(plus(j, 1)?),
        None => None,
    };
    let connu_de_l_arret = |o: &DeBase| -> R<Option<Depart>> {
        Ok(match (connaissance(&fiche, DeQuoi::Arret, "le jour où la compagnie a connu l'arrêt"), debut) {
            (Some(c), _) => Some(c),
            (None, Some(debut)) => Some(Depart { jour: debut, minute: None, quoi: "le jour où la compagnie a connu l'arrêt".into(), presume: Some(o.presume()?) }),
            (None, None) => None,
        })
    };

    // LE SIGNALEMENT D'ARRET (04) : pas pour un motif que la DSN ne couvre pas (`sauf_motifs`).
    let o = &n.signalement_arret;
    if !o.sauf().contains(&motif) {
        match (debut, connu_de_l_arret(o)?) {
            (Some(debut), Some(depart)) => out.push(signalement_dsn(ctx, &mut cal, aujourdhui, CleDObligation::SignalementArret, o, &subrogation, depart, debut, envoi(envois, "04"))?),
            _ => out.push(Obligation::squelette(CleDObligation::SignalementArret, o).indeterminee("Le début de cet arrêt est illisible : le délai ne se compte pas.")),
        }
    }

    // LE SIGNALEMENT D'ARRET RECTIFICATIF (04, annule et remplace) : une fin de subrogation prolongee APRES le depot du
    // signalement d'arret, qui la portait plus courte (fiche net-entreprises 1983 ; `rectificatif_du`).
    let o = &n.signalement_arret_rectificatif;
    if let Some((depart, fait)) = rectificatif_du(a, envois) {
        let mut m = Obligation::squelette(CleDObligation::SignalementArretRectificatif, o);
        let d = o.delai_de(&o.delai, "delai")?;
        let butoir = cal.jours_ouvres_apres(depart.jour, nombre(d.jours_ouvres, "delai.jours_ouvres")?)?;
        m.conclure(aujourdhui, Some(&depart), butoir, None, &d.dit, fait);
        out.push(m);
    }

    // L'ATTESTATION DE SALAIRE HORS DSN (le deuil d'enfant) : des reception de l'avis d'arret, soit -- inference dite --
    // le jour meme ou la compagnie connait l'arret.
    let o = &n.attestation_salaire;
    if o.motifs.contains(&motif) {
        let mut m = Obligation::squelette(CleDObligation::AttestationSalaire, o);
        match (debut, connu_de_l_arret(o)?) {
            (Some(_), Some(depart)) => {
                let d = o.delai_de(&o.delai, "delai")?;
                let butoir = plus(depart.jour, nombre(d.jours, "delai.jours")?)?;
                let fait = dit_fait(fiche.attestation_salaire.as_ref(), o);
                m.conclure(aujourdhui, Some(&depart), butoir, None, &d.dit, fait);
                out.push(m);
            }
            _ => out.push(m.indeterminee("Le début de cet arrêt est illisible : le jour de l'attestation ne se compte pas.")),
        }
    }

    // LA DECLARATION D'ACCIDENT DU TRAVAIL OU DE TRAJET (hors DSN).
    let o = &n.declaration_accident;
    if o.motifs.contains(&motif) {
        let mut m = Obligation::squelette(CleDObligation::DeclarationAccident, o);
        let d = o.delai_de(&o.delai, "delai")?;
        let quoi = "la connaissance de l'accident par la compagnie";
        let depart = match (connaissance(&fiche, DeQuoi::Accident, quoi), a.get("date_accident").and_then(jour)) {
            (Some(c), _) => Some(c),
            (None, Some(j)) => Some(Depart { jour: j, minute: None, quoi: quoi.into(), presume: Some(o.presume()?) }),
            (None, None) => None,
        };
        match depart {
            None => out.push(m.indeterminee("La date de l'accident n'est pas saisie : les quarante-huit heures ne se comptent pas.")),
            Some(depart) => {
                let (butoir, minute) = cal.heures_apres(depart.jour, depart.minute.unwrap_or(0), nombre(d.heures, "delai.heures")?)?;
                let fait = dit_fait(fiche.accident_declare.as_ref(), o);
                // LES SUITES : une fois declaree, les reserves motivees (R. 441-6) ; la feuille d'accident (L. 441-5).
                let mut suites = Vec::new();
                if let Some(r) = o.suites.as_ref().and_then(|s| s.reserves.as_ref()) {
                    let (jusqu_au, phrase) = match &fait {
                        Some(f) => {
                            let jusqu = plus(f.le, r.jours_francs)?;
                            let phrase = if jusqu >= aujourdhui { format!("possibles jusqu'au {}.", lisible(&jusqu)) } else { format!("le délai est passé (le {}).", jusqu.jj_mm_aaaa()) };
                            (Some(jusqu.iso()), phrase)
                        }
                        None => (None, format!("dans les {} jours francs qui suivent la déclaration.", r.jours_francs)),
                    };
                    suites.push(Suite { cle: "reserves", libelle: r.libelle.clone(), texte: r.texte.clone(), inference: Some(r.inference.clone()), jusqu_au, phrase });
                }
                if let Some(f) = o.suites.as_ref().and_then(|s| s.feuille_accident.as_ref()) {
                    suites.push(Suite { cle: "feuille_accident", libelle: f.libelle.clone(), texte: f.texte.clone(), inference: None, jusqu_au: None, phrase: f.libelle.clone() });
                }
                m.suites = Some(suites);
                m.conclure(aujourdhui, Some(&depart), butoir, Some(minute), &d.dit, fait);
                out.push(m);
            }
        }
    }

    // LA MALADIE PROFESSIONNELLE : la victime la declare, pas la compagnie.
    let o = &n.declaration_maladie_professionnelle;
    if o.motifs.contains(&motif) {
        let m = Obligation::squelette(CleDObligation::DeclarationMaladieProfessionnelle, o);
        match cessation {
            None => out.push(m.indeterminee("Le dernier jour travaillé de cet arrêt est illisible.")),
            Some(debut) => {
                let d = o.delai_de(&o.delai, "delai")?;
                let butoir = plus(debut, nombre(d.jours, "delai.jours")?)?;
                let mut m = m;
                m.delai = Some(d.dit.clone());
                m.depart = Some(Depart { jour: debut, minute: None, quoi: "la cessation du travail".into(), presume: None }.dit());
                m.butoir = Some(butoir.iso());
                m.poser_l_etat(A_LA_CHARGE_DU_SALARIE);
                m.phrase = format!(
                    "La maladie professionnelle se déclare par le salarié lui-même à sa caisse, au plus tard le {} (CSS L. 461-5 et R. 461-5) : la compagnie n'a pas de déclaration d'accident à faire.",
                    lisible(&butoir)
                );
                out.push(m);
            }
        }
    }

    // LE SIGNALEMENT DE REPRISE ANTICIPEE (05) : seulement une reprise avant la fin previsionnelle, et pas pour un motif que
    // la DSN ne couvre pas (`sauf_motifs`).
    let o = &n.signalement_reprise;
    if let (Some(rep), Some(fin)) = (a.get("reprise").and_then(jour), a.get("fin_previsionnelle").and_then(jour)) {
        if rep < fin && !o.sauf().contains(&motif) {
            let depart = Depart { jour: rep, minute: None, quoi: "la reprise".into(), presume: None };
            out.push(signalement_dsn(ctx, &mut cal, aujourdhui, CleDObligation::SignalementReprise, o, &subrogation, depart, rep, envoi(envois, "05"))?);
        }
    }
    Ok(out.into_iter().map(Obligation::dire_le_butoir).collect())
}

// -------------------------------------------------------- les fins de contrat

/// Le signalement de fin de contrat (07) d'un contrat qui finit le `fin`. `cle` : la cle de la fiche du salarie (le nom
/// du fichier que `fin-contrat-signaler` ecrit) ; `journal` : les lignes du journal des depots.
pub fn de_la_fin(ctx: &Contexte, cle: Option<&str>, fin: Jour, journal: &[envois::journal::Ligne], aujourdhui: Jour) -> R<Obligation> {
    let n = base()?.obligations;
    let o = &n.signalement_fin_contrat;
    let mut cal = Calendrier::nouveau(ctx.entreprise.feries_chomes());
    let mut m = Obligation::squelette(CleDObligation::SignalementFinContrat, o);
    let d = o.delai_de(&o.delai, "delai")?;
    let butoir = cal.jours_ouvres_apres(fin, nombre(d.jours_ouvres, "delai.jours_ouvres")?)?;
    let fait = cle.and_then(|c| fctu_depose(journal, c, &fin));
    let depart = Depart { jour: fin, minute: None, quoi: "la fin du contrat".into(), presume: None };
    m.conclure(aujourdhui, Some(&depart), butoir, None, &d.dit, fait);
    Ok(m.dire_le_butoir())
}

/// Le jour ou le signalement de fin de ce contrat est parti (`fctu_depose`), s'il l'est.
pub(crate) fn fin_signalee_le(journal: &[envois::journal::Ligne], cle: &str, fin: &Jour) -> Option<Jour> {
    fctu_depose(journal, cle, fin).map(|f| f.le)
}

/// Le premier depot REEL, non refuse au bilan, du signalement de fin de ce contrat : une ligne `dsn` du journal, au jour de la fin
/// (`DsnLue::evenement_lisible`), dont le fichier est celui que `fin-contrat-signaler` ecrit pour ce salarie.
fn fctu_depose(journal: &[envois::journal::Ligne], cle: &str, fin: &Jour) -> Option<Fait> {
    let nom = format!("SignalementFinContrat_{}_{}.dsn", cle.to_uppercase(), fin.aaaammjj());
    envois::journal::filtrer(journal, Some(envois::journal::Acte::Dsn), None, true)
        .filter(|l| l.accepte())
        // Un signalement refuse au bilan n'est pas recu (cahier technique DSN, `norme-dsn/texte/chapitre-4-controles.txt` §4.2) : il reste du.
        .filter(|l| !envois::etats::refuse_par_un_retour(l))
        .filter(|l| evenement_de(l) == fin.iso() && l.fichier().unwrap_or("").to_uppercase() == nom.to_uppercase())
        .filter_map(|l| {
            let quand = l.quand().to_string();
            let le = Jour::lire(quand.get(..10)?).ok()?;
            Some((quand, le, l))
        })
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(quand, le, l)| Fait {
            le,
            comment: "déposé sur net-entreprises ; le bilan dit s'il est accepté".into(),
            dit: FaitDit::Depose(DepotFait { quand, fichier: l.fichier().map(str::to_string), ordre: None, flux: Some(l.flux().to_string()), comment: "depot", le: le.iso() }),
        })
}

/// Le jour de l'evenement qu'un signalement declare (`AAAA-MM-JJ`), ou rien.
fn evenement_de(l: &envois::journal::Ligne) -> &str {
    l.dsn().and_then(|e| e.detail.evenement.as_deref()).unwrap_or("")
}

// ------------------------------------------------------------------ les routes

/// Une route de ce module.
pub fn route(ctx: &Contexte, nom: Route, charge: &Value) -> R<Value> {
    use crate::routes::{en_json, lire_charge};
    match nom {
        Route::ArretConnaissance => en_json(arret_connaissance(ctx, lire_charge(charge)?)?),
        Route::AccidentDeclare => en_json(accident_declare(ctx, lire_charge(charge)?)?),
        Route::AttestationDeclare => en_json(attestation_declare(ctx, lire_charge(charge)?)?),
        Route::ArretObligations => en_json(arret_obligations(ctx, lire_charge(charge)?)?),
        autre => Err(erreur("ErreurOperation", format!("route inconnue : {}", autre))),
    }
}

/// CE QUE RENDENT LES ROUTES DE CE MODULE : leur phrase, les obligations de l'arret, une ligne par obligation et par
/// suite (`phrases`, `en_phrases`) et, pour un arret pas encore ecrit, ce qu'il fera naitre en une ligne (`ensuite`).
#[derive(Debug, Serialize)]
pub(crate) struct ObligationsDeLArret {
    phrase: String,
    obligations: Vec<Obligation>,
    phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ensuite: Option<String>,
}

impl ObligationsDeLArret {
    /// Les obligations, et leur phrase (`phrase(n)`, `n` : combien il y en a).
    fn de(obligations: Vec<Obligation>, phrase: impl FnOnce(usize) -> String) -> ObligationsDeLArret {
        ObligationsDeLArret { phrase: phrase(obligations.len()), phrases: en_phrases(&obligations), obligations, ensuite: None }
    }
}

/// LES OBLIGATIONS D'UN ARRET EN PHRASES, telles que l'ecran les dit : chacune, son libelle et ce qu'elle dit d'elle-meme,
/// puis ses suites (les reserves, la feuille d'accident). Ce que les routes qui en rendent mettent dans `phrases`.
pub(crate) fn en_phrases(obligations: &[Obligation]) -> Vec<String> {
    let mut out = Vec::new();
    for o in obligations {
        out.push(format!("{} : {}", o.libelle, o.phrase));
        for s in o.suites.iter().flatten() {
            out.push(if s.libelle.is_empty() || s.libelle == s.phrase { s.phrase.clone() } else { format!("{} : {}", s.libelle, s.phrase) });
        }
    }
    out
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur("ErreurOperation", m)
}

/// L'arret du salarie `nom` (sa cle, ou son nom) dont le dernier jour travaille est `dernier` : (sa cle, l'arret).
fn l_arret_de(ctx: &Contexte, nom: &str, dernier: &Jour) -> R<(String, Value)> {
    let cle = ctx.cle_du_salarie(nom).map_err(|m| erreur("KeyError", m))?.to_string();
    for a in crate::routes::imprevus::tous_les_arrets(ctx)? {
        if a.get("salarie").map(en_texte).as_deref() == Some(cle.as_str()) && a.get("dernier_jour_travaille").map(en_texte) == Some(dernier.iso()) {
            return Ok((cle, a));
        }
    }
    Err(refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}. Rien n'est ecrit.", cle, dernier.iso())))
}

/// Un jour saisi : vide pour effacer, sinon une date lisible qui n'est pas dans le futur.
fn jour_saisi(ctx: &Contexte, t: &str, quoi: &str) -> R<Option<Jour>> {
    if t.trim().is_empty() {
        return Ok(None);
    }
    let j = Jour::lire(t.trim()).map_err(|m| refus(format!("{} : {}. Rien n'est ecrit.", quoi, m)))?;
    if j > ctx.aujourdhui {
        return Err(refus(format!(
            "{} au {} : c'est apres aujourd'hui ({}). Un fait deja survenu ne se date pas dans le futur. Rien n'est ecrit.",
            quoi,
            j.jj_mm_aaaa(),
            ctx.aujourdhui.jj_mm_aaaa()
        )));
    }
    Ok(Some(j))
}

fn heure_saisie(heure: Option<&str>) -> R<Option<String>> {
    match heure.filter(|h| !h.trim().is_empty()) {
        None => Ok(None),
        Some(h) => match lire_heure(h) {
            Some(m) => Ok(Some(heure_lisible(m))),
            None => Err(refus(format!("Heure {} illisible : HH:MM, de 00:00 a 23:59. Rien n'est ecrit.", repr(&h)))),
        },
    }
}

/// Les obligations de l'arret `a`, comptees aujourd'hui, et leur phrase.
fn reponse(ctx: &Contexte, a: &Value, phrase: impl FnOnce(usize) -> String) -> R<ObligationsDeLArret> {
    let suivi = Suivi::lire(ctx)?;
    Ok(ObligationsDeLArret::de(de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?, phrase))
}

/// La charge de `arret-connaissance` : l'arret (le salarie et son dernier jour travaille), ce que la compagnie a connu
/// (`de`), le jour (vide : le moteur reprend sa presomption) et l'heure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeConnaissance {
    salarie: String,
    dernier_jour: Jour,
    de: DeQuoi,
    jour: String,
    #[serde(default)]
    heure: Option<String>,
}

/// Le jour (et l'heure) ou la compagnie a connu l'arret ou l'accident, tel qu'on le dit : `arret-connaissance`, et
/// `connu` de `arret-enregistrer`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConnaissanceDite {
    jour: String,
    #[serde(default)]
    heure: Option<String>,
}

/// `arret-connaissance` : le jour (et l'heure) ou la compagnie a connu l'arret ou l'accident ; un `jour` vide efface ce
/// qui etait dit (le moteur reprend sa presomption).
fn arret_connaissance(ctx: &Contexte, c: ChargeConnaissance) -> R<ObligationsDeLArret> {
    let (_, a) = l_arret_de(ctx, &c.salarie, &c.dernier_jour)?;
    let saisie = connaissance_saisie(ctx, &a, c.de, &ConnaissanceDite { jour: c.jour, heure: c.heure })?;
    connaissance_ecrire(ctx, &a, c.de, saisie)?;
    reponse(ctx, &a, |n| format!("Jour de connaissance noté. Obligations de l'arrêt : {}.", n))
}

/// Ce que la compagnie dit avoir connu de l'arret `a` (`de`), d'apres le jour et l'heure dits : la connaissance a
/// ecrire dans la fiche de suivi, controlee sans rien ecrire (`None` : un jour vide efface).
pub(crate) fn connaissance_saisie(ctx: &Contexte, a: &Value, de: DeQuoi, dite: &ConnaissanceDite) -> R<Option<ConnaissanceSaisie>> {
    let motif = a.get("motif").map(en_texte).unwrap_or_default();
    if de == DeQuoi::Accident && !motif_appelle_la_declaration(&motif)? {
        return Err(refus(format!(
            "L'arret de {} (motif {}) n'est pas un accident du travail ou de trajet : il n'y a pas de declaration d'accident, ni de connaissance de l'accident a dire. Rien n'est ecrit.",
            a.get("salarie").map(en_texte).unwrap_or_default(),
            motif
        )));
    }
    let j = jour_saisi(ctx, &dite.jour, "La connaissance")?;
    let heure = heure_saisie(dite.heure.as_deref())?;
    if let (Some(j), DeQuoi::Accident, Some(acc)) = (j, de, a.get("date_accident").and_then(jour)) {
        if j < acc {
            return Err(refus(format!(
                "Connu le {}, avant l'accident lui-meme (le {}) : ce n'est pas possible. Rien n'est ecrit.",
                j.jj_mm_aaaa(),
                acc.jj_mm_aaaa()
            )));
        }
    }
    Ok(j.map(|j| ConnaissanceSaisie { jour: j.iso(), heure, saisi_le: crate::routes::maintenant() }))
}

/// Ecrit dans la fiche de suivi de l'arret `a` ce que `connaissance_saisie` a controle.
pub(crate) fn connaissance_ecrire(ctx: &Contexte, a: &Value, de: DeQuoi, saisie: Option<ConnaissanceSaisie>) -> R<()> {
    let texte = |k: &str| a.get(k).map(en_texte).unwrap_or_default();
    Suivi::lire(ctx)?.ecrire(ctx, &texte("salarie"), &texte("dernier_jour_travaille"), |f| match de {
        DeQuoi::Arret => f.connaissance_arret = saisie,
        DeQuoi::Accident => f.connaissance_accident = saisie,
    })
}

/// `accident-declare` : la declaration d'accident du travail faite, dite par la compagnie : le jour, ou (un code de
/// `declaration_accident.ou`), et la reference qu'elle en a (le numero de l'accuse de net-entreprises, celui du
/// recommande). Un `le` vide la retire.
fn accident_declare(ctx: &Contexte, charge: ChargeFaite) -> R<ObligationsDeLArret> {
    dire_faite(ctx, &charge, &Faite {
        obligation: CleDObligation::DeclarationAccident,
        connaissance: DeQuoi::Accident,
        hors_motif: "n'est pas un accident du travail ou de trajet : il n'y a pas de declaration d'accident a dire faite",
        quoi: "La declaration",
        dite: "Déclaration d'accident",
        avant: "que la compagnie ait connu l'accident",
        accident: true,
    })
}

/// `attestation-declare` : l'attestation de salaire pour les indemnites journalieres faite hors DSN (le deuil d'enfant,
/// `attestation_salaire`), dite par la compagnie : le jour, ou (un code de `attestation_salaire.ou` :
/// `net-entreprises-ecran`, `net-entreprises-edi`), et la reference de l'accuse. Un `le` vide la retire. Refuse un arret
/// dont le motif ne l'appelle pas, un jour futur, un lieu inconnu, un jour avant la connaissance de l'arret saisie : rien
/// n'est alors ecrit.
fn attestation_declare(ctx: &Contexte, charge: ChargeFaite) -> R<ObligationsDeLArret> {
    dire_faite(ctx, &charge, &Faite {
        obligation: CleDObligation::AttestationSalaire,
        connaissance: DeQuoi::Arret,
        hors_motif: "est couvert par la DSN : l'attestation de salaire passe par le signalement d'arret, il n'y a pas d'attestation hors DSN a dire faite",
        quoi: "L'attestation",
        dite: "Attestation de salaire",
        avant: "que la compagnie ait connu l'arret",
        accident: false,
    })
}

/// Une obligation hors DSN que la compagnie dit faite : laquelle (sa cle, qui est aussi celle de sa fiche de suivi), ce
/// qui la borne, et comment les phrases la nomment.
struct Faite {
    obligation: CleDObligation,
    connaissance: DeQuoi,
    hors_motif: &'static str,
    quoi: &'static str,
    dite: &'static str,
    avant: &'static str,
    /// Bornee aussi par la date de l'accident (la declaration d'accident).
    accident: bool,
}

/// La charge de `accident-declare` et de `attestation-declare` : l'arret (le salarie et son dernier jour travaille, comme
/// `etapes::ChargeObligationDArret` les ecrit), le jour ou l'obligation a ete faite (vide : la note se retire), ou, a
/// quelle heure, et la reference de l'accuse.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeFaite {
    salarie: String,
    dernier_jour: Jour,
    le: String,
    #[serde(default)]
    depot: Option<String>,
    #[serde(default)]
    heure: Option<String>,
    #[serde(default)]
    reference: Option<String>,
}

/// Une obligation hors DSN dite faite, telle que la fiche de suivi de l'arret la garde.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationFaite {
    le: String,
    heure: Option<String>,
    depot: String,
    reference: String,
    saisi_le: String,
}

fn dire_faite(ctx: &Contexte, charge: &ChargeFaite, f: &Faite) -> R<ObligationsDeLArret> {
    let (cle, a) = l_arret_de(ctx, &charge.salarie, &charge.dernier_jour)?;
    let motif = a.get("motif").map(en_texte).unwrap_or_default();
    if !motif_appelle(f.obligation, &motif)? {
        return Err(refus(format!("L'arret de {} (motif {}) {}. Rien n'est ecrit.", cle, motif, f.hors_motif)));
    }
    let dernier = a.get("dernier_jour_travaille").map(en_texte).unwrap_or_default();
    let le = jour_saisi(ctx, &charge.le, f.quoi)?;
    let faite = match le {
        None => None,
        Some(le) => {
            let depot = charge.depot.clone().ok_or_else(|| erreur("KeyError", socle::fichiers::absent("depot")))?;
            let codes: Vec<String> = base()?.obligations.de(f.obligation).lieux().iter().map(|x| x.code.clone()).collect();
            if !codes.contains(&depot) {
                return Err(refus(format!("Ou {} a-t-elle ete faite ? {} n'est pas connu ; les lieux sont : {}. Rien n'est ecrit.", f.quoi.to_lowercase(), repr(&depot), codes.join(", "))));
            }
            let fiche = Suivi::lire(ctx)?.de(&cle, &dernier);
            if let Some(c) = connaissance(&fiche, f.connaissance, "") {
                if le < c.jour {
                    return Err(refus(format!(
                        "{} faite le {}, avant {} (le {}, saisi) : ce n'est pas possible. Rien n'est ecrit.",
                        f.quoi,
                        le.jj_mm_aaaa(),
                        f.avant,
                        c.jour.jj_mm_aaaa()
                    )));
                }
            }
            if f.accident {
                if let Some(acc) = a.get("date_accident").and_then(jour) {
                    if le < acc {
                        return Err(refus(format!(
                            "Declaree le {}, avant l'accident lui-meme (le {}) : ce n'est pas possible. Rien n'est ecrit.",
                            le.jj_mm_aaaa(),
                            acc.jj_mm_aaaa()
                        )));
                    }
                }
            }
            Some(ObligationFaite {
                le: le.iso(),
                heure: heure_saisie(charge.heure.as_deref())?,
                depot,
                reference: charge.reference.clone().unwrap_or_default(),
                saisi_le: crate::routes::maintenant(),
            })
        }
    };
    let accident = f.accident;
    Suivi::lire(ctx)?.ecrire(ctx, &cle, &dernier, |fiche| {
        if accident {
            fiche.accident_declare = faite;
        } else {
            fiche.attestation_salaire = faite;
        }
    })?;
    let (quoi, retiree) = (f.dite, charge.le.trim().is_empty());
    let le = charge.le.clone();
    reponse(ctx, &a, |n| {
        if retiree {
            format!("{} : la note « faite » est retirée. Obligations de l'arrêt : {}.", quoi, n)
        } else {
            format!("{} notée faite le {}. Obligations de l'arrêt : {}.", quoi, crate::routes::quand(&le), n)
        }
    })
}

/// La charge de `arret-obligations` : l'arret tel qu'il se saisit, chaque champ vide tant qu'il n'est pas tape -- le
/// motif, le dernier jour travaille, le debut, la fin prescrite, la reprise, le salarie et le contrat suspendu (d'ou se
/// tire la subrogation), le jour de l'accident, et ce que la compagnie a connu (l'arret, l'accident et son heure).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct ChargeArretObligations {
    motif: String,
    dernier_jour: String,
    debut: String,
    fin: String,
    reprise: String,
    salarie: String,
    contrat: Option<String>,
    date_accident: String,
    connaissance_arret: String,
    connaissance_accident: String,
    connaissance_heure: String,
}

/// `arret-obligations` : les obligations qu'un arret AINSI SAISI ferait naitre, avant d'etre enregistre (la boite de
/// saisie les montre). Rien n'est ecrit, rien n'est verifie de l'arret lui-meme (c'est `arret-enregistrer` qui refuse) :
/// un champ vide ou illisible donne une obligation `indetermine`, qui dit pourquoi. Rien n'en est parti.
fn arret_obligations(ctx: &Contexte, c: ChargeArretObligations) -> R<ObligationsDeLArret> {
    // L'arret tel que le magasin des imprevus l'ecrirait (`engagement::imprevus`, une `Value`).
    let mut a = json!({
        "genre": engagement::imprevus::ARRET, "salarie": "", "motif": c.motif, "dernier_jour_travaille": c.dernier_jour, "debut": c.debut,
        "fin_previsionnelle": c.fin, "subrogation": "", "date_accident": c.date_accident,
        "reprise": c.reprise,
    });
    // La subrogation se tire du contrat suspendu et du droit (`routes::imprevus::subrogation_de`), comme a l'enregistrement.
    if let (false, Some(dj)) = (c.salarie.is_empty(), socle::date::jour_iso(&c.dernier_jour)) {
        if let Ok(cle) = ctx.cle_du_salarie(&c.salarie) {
            if let Ok(contrat) = crate::routes::imprevus::contrat_suspendu(ctx, &cle, &dj, c.contrat.as_deref()) {
                let imp = json!({"genre": engagement::imprevus::ARRET, "salarie": cle.to_string(), "contrat_debut": contrat.debut, "motif": c.motif,
                    "dernier_jour_travaille": dj.iso(), "debut": c.debut, "fin_previsionnelle": c.fin, "reprise": Value::Null});
                if socle::date::jour_iso(&c.fin).is_some() && socle::date::jour_iso(&c.debut).is_some() {
                    let (sub, sub_debut, sub_fin) = crate::routes::imprevus::subrogation_de(ctx, &imp)?;
                    a["salarie"] = json!(cle.to_string());
                    a["contrat_debut"] = json!(contrat.debut);
                    a["contrat_numero"] = contrat.numero.clone();
                    if sub == json!("01") {
                        a["subrogation_debut"] = sub_debut;
                        a["subrogation_fin"] = sub_fin;
                    }
                    a["subrogation"] = sub;
                }
            }
        }
    }
    let connue = |jour: &str, heure: &str| {
        (!jour.trim().is_empty()).then(|| ConnaissanceSaisie { jour: jour.to_string(), heure: Some(heure.to_string()).filter(|h| !h.is_empty()), saisi_le: String::new() })
    };
    let fiche = FicheDeSuivi {
        salarie: String::new(),
        dernier_jour_travaille: c.dernier_jour.clone(),
        connaissance_arret: connue(&c.connaissance_arret, ""),
        connaissance_accident: connue(&c.connaissance_accident, &c.connaissance_heure),
        accident_declare: None,
        attestation_salaire: None,
    };
    let suivi = Suivi { fiches: vec![fiche] };
    let obligations = obligations_de(ctx, &a, &[], &suivi, ctx.aujourdhui)?;
    // Ce que la boite d'un arret pas encore ecrit dit de ce qu'il fera naitre, en une ligne.
    let suites: Vec<String> = obligations
        .iter()
        .map(|o| {
            let libelle = o.libelle.chars().next().map(|c| c.to_lowercase().collect::<String>() + &o.libelle[c.len_utf8()..]).unwrap_or_default();
            let quand = if o.etat == A_LA_CHARGE_DU_SALARIE { " (par le salarié)".to_string() } else { o.butoir_en_clair().map(|b| format!(" ({})", b)).unwrap_or_default() };
            libelle + &quand
        })
        .collect();
    let mut r = ObligationsDeLArret::de(obligations, |n| format!("Obligations de l'arrêt : {}.", n));
    r.ensuite = (!suites.is_empty()).then(|| format!("Il faudra ensuite : {}.", suites.join(" ; ")));
    Ok(r)
}

#[cfg(test)]
mod essais {
    use super::*;

    #[test]
    fn une_heure_se_lit_en_minutes() {
        assert_eq!(lire_heure("10:30"), Some(630));
        assert_eq!(lire_heure("24:00"), None);
        assert_eq!(lire_heure("9h"), None);
    }

    /// Bug reel : un signalement de fin de contrat accepte au transport puis refuse au bilan se disait fait. Le bilan
    /// tranche (cahier technique DSN, `norme-dsn/texte/chapitre-4-controles.txt` §4.2) : refuse, il reste du.
    #[test]
    fn un_signalement_de_fin_refuse_au_bilan_reste_du() {
        let fin = Jour::lire("2026-03-13").unwrap();
        let ligne = |verdict: &str| -> Vec<envois::journal::Ligne> {
            vec![serde_json::from_value(json!({"quoi": "dsn", "mode": "reel", "accepte": true, "mois": "2026-03", "quand": "2026-03-14T10:00:00", "origine": "",
                    "fichier": "SignalementFinContrat_FANCHON_20260313.dsn", "empreinte": null, "qui": "t", "destinataire": "", "flux": "", "reponse": "", "trace": null,
                    "detail": {"nature": "07", "evenement": "2026-03-13", "repetition": null, "ordre_test": null},
                    "retours": [{"nature": "11", "verdict": verdict, "libelle": "", "statut": null, "publication": null}]}))
            .unwrap()]
        };
        assert!(fctu_depose(&ligne("KO"), "fanchon", &fin).is_none());
        assert!(fctu_depose(&ligne("OK"), "fanchon", &fin).is_some());
    }
}
