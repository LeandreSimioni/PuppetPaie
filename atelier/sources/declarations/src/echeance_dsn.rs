//! L'ECHEANCE DE DEPOT D'UNE DSN MENSUELLE : UN SEUL ENDROIT la calcule. Tout ce qui dit « la DSN de ce mois
//! est due le... » (l'etat du mois, l'echeancier, les rappels, le filet, l'annule-et-remplace, le signalement d'arret
//! subroge, l'horloge des compagnies fictives) passe par `du_mois` ; aucun front ne la recompte (regle 2).
//!
//! LES TEXTES, tous deposes :
//!
//! - LE JOUR : CSS R. 243-6, II (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`) :
//!   « au plus tard aux echeances suivantes : 1° Le 5 de ce mois pour les employeurs dont l'effectif est d'au moins
//!   cinquante salaries et dont la paie est effectuee au cours du meme mois que la periode de travail ; 2° Le 15 de ce mois
//!   dans les autres cas. » Le choix du 5 ou du 15 est `Contexte::echeance_dsn` (l'effectif) ; R. 243-6 ne parle d'aucun
//!   report.
//! - LE REPORT : CSS R. 133-14, I (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`, version en
//!   vigueur depuis le 01/07/2026) : la DSN est adressee « au plus tard aux dates mentionnees a l'article R. 243-6 », et
//!   « Si le delai imparti pour effectuer la declaration sociale nominative expire un jour ferie ou non ouvre, il est
//!   proroge jusqu'au jour ouvrable suivant. »
//! - LA PRATIQUE DU POINT DE DEPOT : le calendrier des declarations de net-entreprises
//!   (`atelier/reference/sources/echeances-dsn/net-entreprises-calendrier-des-declarations-20261015.txt`) : le samedi
//!   05/12/2026 devient le lundi 07/12/2026, le dimanche 15/11/2026 le lundi 16/11/2026. Le samedi est donc non ouvre.
//! - L'HEURE : cahier technique DSN 2026.1, § 1.4.2 (`atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`) :
//!   « L'heure limite du depot de la DSN le jour d'echeance est 12h (midi) en France metropolitaine. » Le meme paragraphe :
//!   l'annule-et-remplace integral « au plus tard la veille a minuit » (`Echeance::veille`).
//!
//! LES INFERENCES (aucun texte ne les dit mot pour mot ; le raisonnement est dans
//! `atelier/reference/sources/echeances-dsn/RAPPORT-20261015.md`) :
//!
//! 1. « NON OUVRE » SE LIT SUR LE CALENDRIER DU POINT DE DEPOT, pas sur celui de la compagnie : le samedi, le dimanche et
//!    les onze fetes legales de L. 3133-1 (`feries::feries`), jamais les feries chomes de la compagnie (`feries::chomes`).
//!    La DSN est attendue par le point de depot, qui n'a qu'un calendrier, le national ; une compagnie qui chome un ferie
//!    de plus ne gagne pas un jour, une compagnie ouverte un ferie n'en perd pas.
//! 2. « JOUR OUVRABLE SUIVANT » SE LIT « PREMIER JOUR OUVRE SUIVANT » : un vendredi ferie reporte au lundi, pas au samedi.
//!    A la lettre, un jour ouvrable comprend le samedi ; mais le point de depot traite le samedi en non ouvre (le
//!    calendrier ci-dessus), et un report au samedi retomberait aussitot sur un jour non ouvre. Aucune piece officielle ne
//!    montre ce cas.
//! 3. LA VEILLE DE L'ANNULE-ET-REMPLACE est celle de l'echeance reportee : le cahier dit « la veille » de « la date butoir
//!    de la DSN (5 ou 15) », et R. 133-14, I, proroge cette date. Aucune piece ne tranche.

use serde_json::{json, Value};

use socle::date::Jour;
use socle::num::Resultat;

/// L'heure limite du depot le jour de l'echeance (cahier technique DSN 2026.1, § 1.4.2).
pub const HEURE_LIMITE: &str = "12:00";

/// Les minutes depuis minuit de `HEURE_LIMITE`.
pub const MINUTE_LIMITE: u32 = 12 * 60;

const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/// Ce que la reponse d'une route porte de ses textes et de ses inferences.
const TEXTES: [&str; 4] = [
    "Code de la sécurité sociale, art. R. 243-6, II : le 5 du mois suivant à partir de cinquante salariés quand la paie est faite dans le mois, le 15 sinon (atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml).",
    "Code de la sécurité sociale, art. R. 133-14, I : « Si le délai imparti pour effectuer la déclaration sociale nominative expire un jour férié ou non ouvré, il est prorogé jusqu'au jour ouvrable suivant. » (atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml).",
    "Calendrier des déclarations de net-entreprises, relevé le 15/10/2026 : samedi 05/12/2026 → lundi 07/12/2026, dimanche 15/11/2026 → lundi 16/11/2026 (atelier/reference/sources/echeances-dsn/net-entreprises-calendrier-des-declarations-20261015.txt).",
    "Cahier technique DSN 2026.1, § 1.4.2 : « L'heure limite du dépôt de la DSN le jour d'échéance est 12h (midi) » (atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt).",
];

const INFERENCE: &str = "Inférence : « non ouvré » se lit sur le calendrier du point de dépôt (samedi, dimanche et les onze fêtes légales), pas sur les fériés chômés de la compagnie ; « jour ouvrable suivant » se lit « premier jour ouvré suivant », si bien qu'un vendredi férié reporte au lundi (aucune pièce officielle ne montre ce cas). La veille de l'annule-et-remplace est celle de l'échéance reportée.";

/// L'echeance de depot de la DSN d'un mois.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Echeance {
    /// Le 5 ou le 15 du mois suivant (R. 243-6, II).
    pub nominale: Jour,
    /// Le jour ou la DSN est due, a midi : la nominale, reportee au premier jour ouvre (R. 133-14, I).
    pub jour: Jour,
    /// Les jours sautes, et pourquoi (« samedi », « dimanche », « Assomption, fête légale »).
    pub sautes: Vec<(Jour, String)>,
}

/// Pourquoi `j` n'est pas un jour ouvre du point de depot, ou `None` s'il l'est.
pub fn non_ouvre(j: &Jour) -> Resultat<Option<String>> {
    match j.jour_de_la_semaine() {
        5 => return Ok(Some("samedi".into())),
        6 => return Ok(Some("dimanche".into())),
        _ => {}
    }
    Ok(socle::feries::feries(j.an)?.into_iter().find(|(d, _)| d == j).map(|(_, nom)| format!("{}, fête légale", nom)))
}

/// `nominale`, reportee au premier jour ouvre qui la suit si elle n'en est pas un (R. 133-14, I ; inferences 1 et 2).
pub fn reporter(nominale: Jour) -> Resultat<Echeance> {
    let (mut jour, mut sautes) = (nominale, Vec::new());
    while let Some(pourquoi) = non_ouvre(&jour)? {
        sautes.push((jour, pourquoi));
        jour = jour.plus_jours(1)?;
    }
    Ok(Echeance { nominale, jour, sautes })
}

/// `int(texte)` d'un morceau de nom de mois.
fn entier(t: &str) -> Resultat<i64> {
    t.trim().parse::<i64>().map_err(|_| format!("nombre entier illisible : {}", socle::fichiers::repr(t)))
}

/// L'echeance de la DSN du mois `mois` (`AAAA-MM`) : le `jour_du_mois` (5 ou 15, `Contexte::echeance_dsn`) du mois
/// suivant, reporte au premier jour ouvre.
pub fn du_mois(mois: &str, jour_du_mois: u32) -> Resultat<Echeance> {
    let (an, mo) = (entier(&mois.chars().take(4).collect::<String>())?, entier(&mois.chars().skip(5).take(2).collect::<String>())?);
    let (an, mo) = if mo == 12 { (an + 1, 1) } else { (an, mo + 1) };
    if !(1..=12).contains(&mo) {
        return Err(socle::date::mois_impossible(mo));
    }
    if !(1..=9999).contains(&an) {
        return Err(socle::date::annee_impossible(an));
    }
    let nominale = Jour::nouveau(an as i32, mo as u32, jour_du_mois).ok_or_else(|| socle::date::jour_impossible(an, mo, jour_du_mois))?;
    reporter(nominale)
}

/// L'echeance de la DSN du mois `mois` pour cette compagnie (son effectif choisit le 5 ou le 15).
pub fn de_la_compagnie(ctx: &droit::contexte::Contexte, mois: &str) -> Resultat<Echeance> {
    du_mois(mois, ctx.echeance_dsn()?)
}

fn lisible(j: &Jour) -> String {
    format!("{} {}", JOURS[j.jour_de_la_semaine() as usize], j.jj_mm_aaaa())
}

impl Echeance {
    /// La date a-t-elle bouge ?
    pub fn reportee(&self) -> bool {
        self.jour != self.nominale
    }

    /// Le dernier jour (jusqu'a minuit) ou un annule-et-remplace integral est admis : la veille de l'echeance (cahier
    /// technique § 1.4.2 ; inference 3 : la veille de l'echeance reportee).
    pub fn veille(&self) -> Resultat<Jour> {
        self.jour.plus_jours(-1)
    }

    /// Pourquoi la date a bouge, ou `None`.
    pub fn pourquoi(&self) -> Option<String> {
        if !self.reportee() {
            return None;
        }
        // « Le 15/11/2026 (dimanche) », « Le 05/04/2026 (dimanche), le 06/04/2026 (Lundi de Pâques, fête légale) ».
        let jours: Vec<String> = self.sautes.iter().map(|(j, p)| format!("le {} ({})", j.jj_mm_aaaa(), p)).collect();
        Some(format!(
            "L{} {} pas ouvré{} : l'échéance du {} est reportée au {} (CSS R. 133-14, I).",
            &jours.join(", ")[1..],
            if jours.len() > 1 { "ne sont" } else { "n'est" },
            if jours.len() > 1 { "s" } else { "" },
            self.nominale.jj_mm_aaaa(),
            lisible(&self.jour)
        ))
    }

    /// « lundi 16/11/2026 à midi ».
    pub fn phrase(&self) -> String {
        format!("{} à midi", lisible(&self.jour))
    }

    /// Ce que les routes rendent a cote de leur date (`echeance_depot`).
    pub fn en_json(&self) -> Value {
        json!({
            "jour": self.jour.iso(),
            "heure": HEURE_LIMITE,
            "nominale": self.nominale.iso(),
            "reportee": self.reportee(),
            "pourquoi": self.pourquoi(),
            "phrase": self.phrase(),
            "veille_annule_et_remplace": self.veille().ok().map(|j| j.iso()),
            "textes": TEXTES,
            "inference": INFERENCE,
        })
    }
}
