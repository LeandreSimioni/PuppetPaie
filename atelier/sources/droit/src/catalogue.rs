//! Le catalogue des rubriques : ce que le calcul lit.
//!
//! Deux fichiers : `donnees/rubriques-base.json` (le logiciel) et
//! `config/rubriques-compagnie.json` (les donnees, sous la racine de la
//! compagnie). Les rubriques de la compagnie sont des regles du calcul : datees
//! comme toute la configuration, lues a la date de la periode. Le catalogue se
//! range par libelle.

use serde::Deserialize;
use socle::echec::APart;
use socle::fichiers::{dossier_du_code, repr, signature};
use socle::num::Resultat;
use crate::fiche::{Case, ChampsRubrique, Datee, FicheDatee, RubriqueDatee, VoirListe};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use socle::date::{repr_texte, Jour};

/// Les unites que le moteur sait compter.
pub const UNITES: [&str; 8] =
    ["cachet", "heure", "jour", "semaine", "mois", "service", "forfait", "pourcentage"];

/// Les natures juridiques. Elles ne commandent pas l'assujettissement (`soumis`, `imposable` le disent, rubrique par
/// rubrique) ; d'une ligne HORS assiette, elles disent ce qu'elle est pour le montant net social
/// (`Rubrique::hors_assiette`).
pub const NATURES: [&str; 7] =
    ["salaire", "frais", "avantage_nature", "indemnite", "prime", "retenue", "absence"];

/// Ce qu'est une ligne hors assiette (`soumis` faux) pour le montant net social (S21.G00.58.004, type 03). Decret
/// n° 2023-1378 du 28/12/2023 : CSS R. 844-1 II (atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261010.md), C. trav. R. 3243-1, 9° bis ; FAQ du BOSS, Q 2.2, 2.11 et 2.12 (atelier/reference/sources/
/// montant-net-social/LISEZ-MOI.md, et la FAQ reproduite a cote).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorsAssiette {
    /// Un remboursement de frais professionnels dans ses limites d'exoneration : n'y entre pas (Q 2.12).
    Frais,
    /// Une somme versee a l'occasion du travail, exoneree ou non (indemnites de rupture, Q 2.2) : y entre.
    Revenu,
    /// Un mouvement de tresorerie (acompte, avance, saisie) : pas un revenu, n'y entre pas (Q 2.12).
    Mouvement,
}

/// Les cinq avantages en nature du bloc 54.
pub const AVANTAGES_EN_NATURE: [&str; 5] = ["02", "03", "04", "05", "06"];

pub const COMPTE_POUR: [&str; 2] = ["droits_et_montant", "ouverture_des_droits"];

/// Les valeurs de S21.G00.54.001 (cahier technique NEODeS 2026.1).
pub const AUTRES_BRUTS: &[(&str, &str)] = &[
    ("01", "Somme versee par un tiers"),
    ("02", "Avantage en nature : repas"),
    ("03", "Avantage en nature : logement"),
    ("04", "Avantage en nature : vehicule"),
    ("05", "Avantage en nature : NTIC"),
    ("06", "Avantage en nature : autres"),
    ("07", "Frais professionnels rembourses au forfait"),
    ("08", "Frais professionnels pris en charge par l'employeur"),
    ("09", "Frais professionnels rembourses au reel"),
    ("10", "Deduction forfaitaire specifique"),
    ("11", "Participation y compris supplement"),
    ("12", "Interessement y compris supplement"),
    ("14", "Abondement au plan d'epargne entreprise (PEE)"),
    ("15", "Abondement au plan d'epargne interentreprises (PEI)"),
    ("16", "Abondement a un plan d'epargne pour la retraite collectif"),
    ("17", "Participation patronale au financement des titres-restaurant"),
    ("18", "Participation patronale aux frais de transports publics"),
    ("19", "Participation patronale aux frais de transports personnels"),
    ("25", "Droit d'auteur"),
    ("26", "Droit de doublage"),
    ("27", "Droit de rediffusion"),
    ("31", "Avantages de preretraite verses par l'employeur"),
    ("33", "Sommes d'un CET reaffectees a un plan d'epargne retraite"),
    ("34", "[FP] Prestations familiales"),
    ("35", "Frais professionnels reintegres dans l'assiette (DFS)"),
    ("36", "Sommes provenant d'un CET converties en points RAFP"),
    ("37", "Participation et interessement verses immediatement"),
    ("50", "Potentiel nouveau type A"),
    ("51", "Potentiel nouveau type B"),
    ("90", "Participation au financement des services a la personne"),
    ("91", "Participation de l'employeur aux cheques vacances"),
    ("92", "Cotisation patronale frais de sante"),
    ("93", "Cotisation prevoyance et retraite supplementaire"),
    ("94", "Potentiel nouveau type C"),
    ("95", "Potentiel nouveau type D"),
    ("96", "Potentiel nouveau type E"),
];

/// Le plafond annuel d'heures d'une rubrique (l'enseignement, annexe X) : selon l'age du salarie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlafondAnnuelHeures {
    pub moins_de_50_ans: i64,
    #[serde(rename = "50_ans_et_plus")]
    pub cinquante_ans_et_plus: i64,
}

/// Une forme de paie : ce que le bulletin appelle une ligne.
#[derive(Debug, Clone, PartialEq)]
pub struct Rubrique {
    pub code: String,
    pub libelle: String,
    pub unite: String,
    pub nature: String,
    pub saisie: Vec<String>,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub source: String,
    pub sens: String,
    pub heures_forfait: Option<f64>,
    pub plafond: Option<String>,
    pub aem: Option<String>,
    pub dsn_type: Option<String>,
    pub dsn_autres_bruts: Option<String>,
    pub annexe: Option<String>,
    pub plafond_mensuel: Option<i64>,
    pub plafond_annuel_heures: Option<PlafondAnnuelHeures>,
    pub compte_pour: String,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub suspension: bool,
    pub arret_de_travail: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<String>,
    pub origine: String,
    /// Ce que la ligne fait, que l'ecran dit sous elle quand son nom ne le dit pas (l'acompte).
    pub dit: Option<String>,
}

fn joindre(l: &[&str]) -> String {
    l.join(", ")
}

impl Rubrique {
    /// Les refus d'une rubrique mal formee.
    fn verifier(&self) -> Resultat<()> {
        let code = &self.code;
        if !UNITES.contains(&self.unite.as_str()) {
            return Err(format!(
                "Rubrique {} : unite {} inconnue. Connues : {}.",
                repr(code), repr(&self.unite), joindre(&UNITES)));
        }
        if !NATURES.contains(&self.nature.as_str()) {
            return Err(format!(
                "Rubrique {} : nature {} inconnue. Connues : {}.",
                repr(code), repr(&self.nature), joindre(&NATURES)));
        }
        if !COMPTE_POUR.contains(&self.compte_pour.as_str()) {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : compte_pour {} inconnu. Connus : {}.",
                repr(code), repr(&self.compte_pour), joindre(&COMPTE_POUR))));
        }
        if self.source.is_empty() {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `source` est vide. Une rubrique dit d'ou vient \
                 sa regle, sinon personne ne pourra la verifier.", repr(code))));
        }
        if self.conges_spectacles && !self.soumis {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `conges_spectacles` sans `soumis`. Les Conges \
                 Spectacles sont une cotisation : leur assiette ne peut pas \
                 depasser ce qui entre dans l'assiette des cotisations.", repr(code))));
        }
        if self.heures_forfait.is_some() && self.unite == "heure" {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `heures_forfait` sur une rubrique dont l'unite \
                 est l'heure. Les heures y sont un fait, pas un forfait : \
                 elles compteraient deux fois.", repr(code))));
        }
        if let Some(c) = &self.dsn_autres_bruts {
            if !AUTRES_BRUTS.iter().any(|(k, _)| k == c) {
                return Err(mal_decrite(code, format!(
                    "Rubrique {} : `dsn_autres_bruts` vaut {}, qui n'est pas une \
                     valeur de S21.G00.54.001. Voir AUTRES_BRUTS, releve dans le \
                     cahier technique NEODeS 2026.1.", repr(code), repr(c))));
            }
        }
        Ok(())
    }

    /// Ce qu'une ligne de cette rubrique, hors assiette, est pour le montant net social : la nature le dit
    /// (`frais`, `indemnite`, `retenue`). Une autre nature ne le dit pas d'une ligne hors assiette (une prime, un
    /// salaire exoneres sont-ils un revenu ? un avantage en nature hors assiette, quoi ?) : refus, regle 1.
    pub fn hors_assiette(&self) -> Resultat<HorsAssiette> {
        match self.nature.as_str() {
            "frais" => Ok(HorsAssiette::Frais),
            "indemnite" => Ok(HorsAssiette::Revenu),
            "retenue" => Ok(HorsAssiette::Mouvement),
            autre => Err(mal_decrite(&self.code, format!(
                "Rubrique {} : hors assiette (`soumis` faux) et de nature {}. Le montant net social \
                 (S21.G00.58.004, bulletin) ne sait pas si cette ligne est un revenu, un remboursement de frais \
                 ou un mouvement de tresorerie, et ne le devine pas. Hors assiette, la nature doit etre `frais` \
                 (remboursement de frais professionnels : hors du montant net social), `indemnite` (une somme \
                 versee a l'occasion du travail, exoneree ou non : dans le montant net social) ou `retenue` \
                 (acompte, avance, saisie : hors du montant net social).",
                repr(&self.code), repr(autre)))),
        }
    }

    /// La forme de paie a laquelle la rubrique est reservee : `"mensuelle"`, `"activite"` ou rien.
    pub fn reservee_a(&self) -> Option<&'static str> {
        if self.salaire_de_base
            || self.retabli
            || self.suspension
            || self.arret_de_travail
            || self.conges_pris
            || self.dsn_bloc52.as_deref().is_some_and(|s| !s.is_empty())
        {
            return Some("mensuelle");
        }
        if self.unite == "cachet" {
            return Some("activite");
        }
        None
    }

    /// Le statut auquel la rubrique est reservee : le service et le cachet ne sont des unites de paie que pour un
    /// artiste. Les conventions payent au service les repetitions des artistes (1285, art. X.3.2 C et X.3.3,
    /// `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`) et au cachet leurs representations
    /// (3090, titre VII « Salaires minimaux », KALIARTI000049099195 de `kali-entrepot/` : « Artistes … Rémunération au
    /// cachet ») ; un technicien y est paye a l'heure, a la journee, a la semaine ou au mois (1285, titre VI,
    /// `kali-1285-titre-vi-duree-du-travail-20261102.md` ; 3090, titre III, art. 3.2, KALIARTI000028930755 : des services
    /// indivisibles comptes en heures, et le titre VII : « Techniciens … par heure, par mois » ; les grilles des
    /// techniciens de `minima-conventionnels.json`, aucune au service ni au cachet). L'assurance chomage ne connait le
    /// cachet que des artistes : annexe X, art. 3 (« Lorsque l'activité des artistes est déclarée sous la forme de
    /// cachets ») ; l'annexe VIII, celle des techniciens, ne compte que des heures (art. 3,
    /// `plafond-intermittents/unedic-annexe10-convention-20241115.txt`, `…annexe8…`).
    pub fn reservee_au_statut(&self) -> Option<&'static str> {
        matches!(self.unite.as_str(), "service" | "cachet").then_some("artiste")
    }

    /// Une ligne de cette rubrique doit dire ses heures : `heures` est dans
    /// sa `saisie` et aucun forfait n'en tient lieu (le service de repetition,
    /// la journee, la semaine, le mois, l'absence a la journee). Le catalogue
    /// le dit (`_heures` de `rubriques-base.json`) : « partout ailleurs les
    /// heures sont saisies, parce qu'elles sont un fait elles aussi ». Une
    /// ligne qui les tait se refuse (`engagement::engagements`) :
    /// sans elle, [`Rubrique::heures`] compterait zero en silence.
    pub fn heures_a_saisir(&self) -> bool {
        self.heures_forfait.is_none() && self.saisie.iter().any(|s| s == "heures")
    }

    /// Les heures que cette ligne fait compter, au total : le forfait
    /// l'emporte, sinon les heures saisies, sinon la quantite si l'unite est
    /// l'heure. Zero pour une rubrique qui demande ses heures et ne les a pas
    /// recues : la saisie qui la porte a deja ete refusee
    /// ([`Rubrique::heures_a_saisir`]).
    ///
    /// UNE convention : le `heures` d'une ligne de paie, comme
    /// `heures_forfait`, compte les heures PAR UNITE, et la quantite les
    /// multiplie. 14 services de 4 h : `quantite` 14, `heures` 4, soit 56 h.
    /// Un mois de permanent : `quantite` = la fraction du mois payee,
    /// `heures` = la quotite du contrat (`conges::lignes_du_mois`). Le produit
    /// se compte au centieme d'heure, comme la DSN l'ecrit.
    pub fn heures(&self, quantite: f64, heures_saisies: Option<f64>) -> f64 {
        let centieme = |x: f64| (x * 100.0).round() / 100.0;
        if let Some(f) = self.heures_forfait {
            return centieme(f * quantite);
        }
        if let Some(h) = heures_saisies {
            return centieme(h * quantite);
        }
        if self.unite == "heure" {
            return quantite;
        }
        0.0
    }
}

// ------------------------------------------------------------------- lecture

/// Le refus d'une rubrique qui ne se construit pas : la phrase, et ce qui ne va pas a part.
fn mal_decrite(code: &str, detail: String) -> String {
    socle::echec::avec(format!("La rubrique {} est mal décrite : le signaler au développeur.", repr(code)), APart { detail: Some(detail), ..APart::default() })
}

/// `rubriques-base.json`, entier : ses notes, les annexes VIII et X, les rubriques livrees (chacune une fiche que la
/// compagnie peut ajuster case par case, `ChampsRubrique`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Base {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    annexes: Annexes,
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    rubriques: Vec<(String, ChampsRubrique)>,
}

/// Le seuil et la fenetre des annexes VIII et X (`rubriques-base.json`, `annexes`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annexes {
    /// Les heures qui ouvrent les droits (507).
    pub seuil_heures: i64,
    /// La fenetre ou elles se comptent, en jours (365).
    pub fenetre_jours: i64,
    #[serde(rename = "note")]
    _note: String,
}

/// LA SEULE LECTURE DE `rubriques-base.json` : sa forme.
fn base() -> Resultat<Base> {
    socle::forme::lire_fichier(
        &dossier_du_code().join("rubriques-base.json"),
        "Les rubriques livrées avec le logiciel ne se lisent pas : le signaler au développeur.",
    )
}

/// Le seuil et la fenetre des annexes VIII et X.
pub fn annexes() -> Resultat<Annexes> {
    Ok(base()?.annexes)
}

/// `config/rubriques-compagnie.json` : les rubriques propres a la compagnie, chacune une fiche datee (`RubriqueDatee`).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RubriquesDeLaCompagnie {
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    rubriques: Vec<(String, RubriqueDatee)>,
}

/// Le fichier des rubriques de la compagnie, s'il existe ; absent, il n'en dit aucune.
fn de_la_compagnie(racine: &Path) -> Resultat<RubriquesDeLaCompagnie> {
    let p = racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE);
    if !socle::fichiers::existe(&p) {
        return Ok(RubriquesDeLaCompagnie::default());
    }
    crate::fiche::lire_fichier(&p, &format!("config/{}", RUBRIQUES_COMPAGNIE))
}

/// LA RUBRIQUE DU CALCUL, construite de ses cases (`ChampsRubrique`), sa seule construction : une case que la rubrique
/// doit dire, ou qui ne dit pas ce que sa forme attend, se refuse en la nommant.
fn construire(code: &str, f: ChampsRubrique, origine: &str) -> Resultat<Rubrique> {
    let faute = |cle: &str, quoi: &str| mal_decrite(code, format!("Rubrique {} : `{}` {}.", repr(code), cle, quoi));
    fn dite<T>(c: Case<T>, cle: &str, faute: &dyn Fn(&str, &str) -> String) -> Resultat<T> {
        match c {
            Case::Dite(v) => Ok(v),
            Case::Absente => Err(faute(cle, "manque")),
            _ => Err(faute(cle, "est vide")),
        }
    }
    fn ou_rien<T>(c: Case<T>, cle: &str, faute: &dyn Fn(&str, &str) -> String) -> Resultat<Option<T>> {
        match c {
            Case::Dite(v) => Ok(Some(v)),
            Case::Absente | Case::Nulle => Ok(None),
            Case::Vide => Err(faute(cle, "est vide")),
        }
    }
    fn texte(c: Case<String>) -> Option<String> {
        match c {
            Case::Dite(t) => Some(t),
            Case::Vide => Some(String::new()),
            Case::Absente | Case::Nulle => None,
        }
    }
    let texte_dit = |c: Case<String>, cle: &str| match c {
        Case::Absente | Case::Nulle => Err(faute(cle, "manque")),
        c => Ok(c.texte().to_string()),
    };
    let oui = |c: Case<bool>, cle: &str| ou_rien(c, cle, &faute).map(|b| b.unwrap_or(false));
    let r = Rubrique {
        code: code.to_string(),
        libelle: texte_dit(f.libelle, "libelle")?,
        unite: texte_dit(f.unite, "unite")?,
        nature: texte_dit(f.nature, "nature")?,
        saisie: ou_rien(f.saisie, "saisie", &faute)?.unwrap_or_default(),
        soumis: dite(f.soumis, "soumis", &faute)?,
        imposable: dite(f.imposable, "imposable", &faute)?,
        conges_spectacles: dite(f.conges_spectacles, "conges_spectacles", &faute)?,
        source: texte_dit(f.source, "source")?,
        sens: texte(f.sens).unwrap_or_else(|| "+".into()),
        heures_forfait: ou_rien(f.heures_forfait, "heures_forfait", &faute)?.map(|m| m.flottant()).transpose()?,
        plafond: texte(f.plafond),
        aem: texte(f.aem),
        dsn_type: texte(f.dsn_type),
        dsn_autres_bruts: texte(f.dsn_autres_bruts),
        annexe: texte(f.annexe),
        plafond_mensuel: ou_rien(f.plafond_mensuel, "plafond_mensuel", &faute)?,
        plafond_annuel_heures: ou_rien(f.plafond_annuel_heures, "plafond_annuel_heures", &faute)?,
        compte_pour: texte(f.compte_pour).unwrap_or_else(|| "droits_et_montant".into()),
        salaire_de_base: oui(f.salaire_de_base, "salaire_de_base")?,
        retabli: oui(f.retabli, "retabli")?,
        suspension: oui(f.suspension, "suspension")?,
        arret_de_travail: oui(f.arret_de_travail, "arret_de_travail")?,
        conges_pris: oui(f.conges_pris, "conges_pris")?,
        dsn_bloc52: texte(f.dsn_bloc52),
        origine: origine.to_string(),
        dit: texte(f.dit),
    };
    r.verifier()?;
    Ok(r)
}

/// Le fichier des rubriques propres a la compagnie, sous `config/`.
pub const RUBRIQUES_COMPAGNIE: &str = "rubriques-compagnie.json";

/// Une rubrique de la compagnie a-t-elle une valeur datee ? Sinon le catalogue est le meme a toute date.
fn rubriques_datees(racine: &Path) -> Resultat<bool> {
    struct UneDatee(bool);
    impl VoirListe for UneDatee {
        fn liste<T: serde::Serialize>(&mut self, _: &'static str, liste: &[Datee<T>]) {
            self.0 |= liste.iter().any(|e| e.depuis.is_some());
        }
    }
    let mut datee = UneDatee(false);
    for (_, r) in de_la_compagnie(racine)?.rubriques {
        r.voir_listes(&mut datee);
    }
    Ok(datee.0)
}

/// LE CATALOGUE A `jour` : les rubriques livrees, chacune sous les cases que la compagnie en dit a cette date, puis les
/// siennes (`config/rubriques-compagnie.json`, datees comme toute la configuration : une rubrique changee a une date ne
/// change pas les paies d'avant ; une rubrique qui ne dit encore rien a `jour` n'existe pas).
fn construire_catalogue(racine: &Path, jour: &Jour) -> Resultat<Vec<Rubrique>> {
    // code -> (cases, propre a la compagnie)
    let mut brut: Vec<(String, ChampsRubrique, bool)> = base()?.rubriques.into_iter().map(|(c, v)| (c, v, false)).collect();
    if socle::fichiers::est_dossier(racine) {
        for (c, datee) in de_la_compagnie(racine)?.rubriques {
            let v = datee.au(jour);
            if v.est_vide() {
                continue;
            }
            match brut.iter_mut().find(|e| e.0 == c) {
                Some(e) => e.1.par_dessus(&v),
                None => brut.push((c, v, true)),
            }
        }
    }
    let mut out = Vec::new();
    for (code, d, propre) in brut {
        out.push(construire(&code, d, if propre { "compagnie" } else { "base" })?);
    }
    out.sort_by_key(|r| r.libelle.to_lowercase());
    Ok(out)
}

fn empreinte(p: &Path) -> String {
    format!("{}:{:?}", p.display(), signature(p))
}

/// Un catalogue construit, et si les rubriques de la compagnie y sont datees (il depend alors du jour lu).
type EnCache = (bool, Arc<Vec<Rubrique>>);
static CATALOGUES: OnceLock<Mutex<HashMap<String, EnCache>>> = OnceLock::new();

/// Les rubriques disponibles a `jour` (la date de la periode payee : les rubriques de la compagnie sont datees), par
/// libelle. `racine` est la racine des donnees de la
/// compagnie (`ctx.racine`). Construit une fois par etat des deux fichiers (et par jour, si la compagnie a date une
/// de ses rubriques).
pub fn catalogue(racine: &Path, jour: &Jour) -> Resultat<Arc<Vec<Rubrique>>> {
    let fichiers = format!(
        "{}|{}",
        empreinte(&dossier_du_code().join("rubriques-base.json")),
        empreinte(&racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE)),
    );
    let cache = CATALOGUES.get_or_init(|| Mutex::new(HashMap::new()));
    let connue = cache.lock().unwrap().get(&fichiers).map(|(datee, _)| *datee);
    let datee = match connue {
        Some(d) => d,
        None => rubriques_datees(racine)?,
    };
    let cle = if datee { format!("{}|{}", fichiers, jour.iso()) } else { fichiers.clone() };
    if let Some((_, c)) = cache.lock().unwrap().get(&cle) {
        // Un catalogue servi par le cache a lu ses fichiers quand meme, pour l'empreinte du calcul -- seulement quand un
        // calcul suivi les note (sinon, autant de lectures du disque pour rien a chaque rubrique).
        if !socle::fichiers::registre_ouvert() {
            return Ok(c.clone());
        }
        socle::fichiers::est_dossier(racine);
        for p in [
            dossier_du_code().join("rubriques-base.json"),
            racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE),
        ] {
            if socle::fichiers::existe(&p) {
                socle::fichiers::noter_si_besoin(&p);
            }
        }
        return Ok(c.clone());
    }
    let c = Arc::new(construire_catalogue(racine, jour)?);
    let mut g = cache.lock().unwrap();
    if g.len() > 64 {
        g.clear();
    }
    g.insert(fichiers, (datee, c.clone()));
    g.insert(cle, (datee, c.clone()));
    Ok(c)
}

/// LA RUBRIQUE D'UNE LIGNE DE PAIE NEUVE, par forme de paie (`"activite"` ou `"mensuelle"`, celles de
/// [`Rubrique::reservee_a`]) et par statut du salarie (`"artiste"` ou `"technicien"`, celui de
/// [`Rubrique::reservee_au_statut`]) : une ligne neuve paie un travail, elle ne s'ouvre pas sur la premiere rubrique du
/// deroulant, qui se range par libelle (« Absence » en tete) : la premiere rubrique de salaire (nature `salaire`) que la
/// forme et le statut proposent, dans l'ordre du catalogue. `None` : aucune.
pub fn par_defaut<'a>(cat: &'a [Rubrique], forme: &str, statut: &str) -> Option<&'a Rubrique> {
    let proposee = |r: &&Rubrique| r.reservee_a().is_none_or(|f| f == forme) && r.reservee_au_statut().is_none_or(|s| s == statut);
    cat.iter().filter(proposee).find(|r| r.nature == "salaire")
}

/// Une rubrique par son code, telle qu'elle vaut a `jour`. Refuse un code inconnu : mieux vaut s'arreter que payer une
/// ligne dont on ignore le regime.
pub fn rubrique(racine: &Path, code: &str, jour: &Jour) -> Resultat<Rubrique> {
    let cat = catalogue(racine, jour)?;
    if let Some(r) = cat.iter().find(|r| r.code == code) {
        return Ok(r.clone());
    }
    let mut connues: Vec<&str> = cat.iter().map(|r| r.code.as_str()).collect();
    connues.sort();
    Err(format!(
        "Rubrique inconnue : {}. Connues : {}.", repr(code), connues.join(", ")))
}

/// Le code S21.G00.54.001 de cette ligne de paie, ou `None` : `genre` de la
/// saisie precise lequel des cinq avantages en nature.
pub fn code_54(r: &Rubrique, genre: Option<&str>) -> Resultat<Option<String>> {
    let Some(code) = &r.dsn_autres_bruts else { return Ok(None) };
    let genre = genre.unwrap_or("").trim().to_string();
    if genre.is_empty() {
        return Ok(Some(code.clone()));
    }
    if !AVANTAGES_EN_NATURE.contains(&code.as_str()) {
        return Err(socle::echec::a_la_cle(&format!("La ligne « {} » dit le genre d'un avantage en nature, et ce n'en est pas un : retirer ce genre.", r.libelle), "genre", repr_texte(&genre)));
    }
    if !AVANTAGES_EN_NATURE.contains(&genre.as_str()) {
        return Err(socle::echec::a_la_cle(&format!("La ligne « {} » porte un genre d'avantage en nature inconnu : le choisir dans la liste.", r.libelle), "genre", format!("{} ; connus : {}", repr_texte(&genre), AVANTAGES_EN_NATURE.join(", "))));
    }
    Ok(Some(genre))
}
