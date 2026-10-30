//! LE LECTEUR DES AEM EN PDF. L'attestation d'employeur mensuelle des annexes 8 et 10 que l'employeur remet a
//! l'intermittent pour chaque mois (« les attestations mensuelles nominatives afférentes à chaque salarié intermittent
//! rémunéré au cours du mois écoulé » : Unédic, circulaire 2016-25,
//! `atelier/reference/sources/unedic-circulaire-2016-25-annexes-8-10.txt`, l. 5094-5121) a la mise en page du
//! formulaire d'Unédic, quel que soit le logiciel qui l'edite.
//!
//! ELLE SE LIT PAR SES RUBRIQUES, PAS PAR DES POSITIONS SUR LA PAGE. Une rubrique (« N° SIRET », « Raison sociale »,
//! « Nom de famille », « NIR », « Date d'embauche »...) est une suite de mots sur une meme ligne ; sa valeur est ce qui
//! la suit a droite sur sa ligne, jusqu'a la rubrique suivante. Les cases cochees s'impriment par une marque (`MARQUES`)
//! juste a droite de leur libelle (« Artiste Ñ », « Non cadre Ñ », « AEM INITIALE Ñ »). LES NOMBRES DU MOIS se lisent
//! PAR COLONNE : les heures, les cachets isoles et groupes, les jours sont sous leurs en-tetes, sur deux rangs ; un
//! nombre va a l'en-tete dont il est le plus proche, a l'aplomb. Le brut est le nombre sous « SALAIRES BRUTS ».
//!
//! Ce que l'AEM porte et que l'import reprend :
//! - l'employeur : Siret, code APE, IDCC, raison sociale, code postal et commune, numero d'affiliation a la caisse des
//!   Congés Spectacles (dix chiffres et une lettre, `documents::conges_spectacles::faute_adherent`) ;
//! - le salarie : nom de famille, prenom, NIR et sa cle (verifiee), date de naissance, adresse, emploi, cadre ou non,
//!   artiste ou technicien (« Les salariés intermittents de l'annexe VIII sont les ouvriers et techniciens », l. 783 ;
//!   « Les salariés intermittents de l'annexe X sont les artistes du spectacle », l. 1846 : la case « Technicien » ou
//!   « Ouvrier » est l'annexe 8, la case « Artiste » l'annexe 10 ; la case « Réalisateur » ne dit pas de statut ici) ;
//! - le mois, le numero de l'attestation, le numero d'objet, la date d'embauche et la date de fin du contrat, les
//!   heures, les cachets, les jours travailles et le salaire brut du mois.
//!
//! UNE AEM DIT UN MOIS. Ses nombres et son brut sont ceux du mois de l'attestation (« RÉMUNERATION VERSÉES AU COURS DU
//! MOIS ») ; la periode qu'elle atteste est celle du contrat bornee a ce mois (un contrat « en cours » va jusqu'au
//! dernier jour du mois). C'est une inference, tiree du formulaire lui-meme.
//!
//! RIEN NE SE DEVINE (regle 1) : une AEM complementaire ou rectificative ne se reprend pas (elle corrige une attestation
//! que l'import ne voit pas) ; une AEM sans NIR, sans dates, sans ses colonnes de nombres ou sans brut n'est pas
//! reprise, et le refus dit ce qui manque. UN FORMULAIRE DONT LES LIBELLES NE SE LISENT PAS (polices sans table de
//! caracteres : les mots sortent en « � ») ne se lit pas : aucune rubrique n'ancre ses valeurs, et une grille de
//! positions relevee sur un exemplaire serait une position devinee ; l'import le dit et demande la piece du meme mois
//! qui se lit.

use std::path::Path;

use serde_json::Value;

use socle::date::Jour;
use socle::num::Decimal;

use crate::import::certificat_lu::{date_en_tete, jetons, nir_en_tete, Certificat};

/// Ce qui marque une case cochee, a droite de son libelle (« Ñ » : la coche d'une police sans table).
const MARQUES: [&str; 5] = ["Ñ", "✔", "✓", "☒", "X"];

/// Ce que dit une AEM qui ne se lit pas.
pub const ILLISIBLE: &str = "cette AEM ne se lit pas : ses libellés s'impriment en signes sans lettres (police sans table de caractères), rien ne dit quelle case porte quelle valeur ; déposez le bulletin ou le certificat Congés Spectacles du même mois";

/// Un mot de la page et sa boite.
#[derive(Clone, Debug)]
struct Mot {
    texte: String,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Mot {
    fn cy(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
    fn cx(&self) -> f64 {
        (self.x0 + self.x1) / 2.0
    }
}

/// Une rubrique trouvee : les mots qui la font (rangs dans la page) et sa boite.
#[derive(Clone, Debug)]
struct Rubrique {
    cle: &'static str,
    mots: Vec<usize>,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Rubrique {
    fn cy(&self) -> f64 {
        (self.y0 + self.y1) / 2.0
    }
}

/// Les rubriques du formulaire : (cle, jetons). Les plus longues d'abord : « N° d'affiliation à la caisse » n'est pas
/// « N° d'affiliation ». Celles qui ne se lisent pas ici bornent la valeur de leur voisine de ligne.
const RUBRIQUES: [(&str, &str); 37] = [
    ("fin", "date de fin du contrat de travail"),
    ("adherent", "n d affiliation a la caisse"),
    ("licence", "licence de spectacle n"),
    ("prestation", "4 prestation de travail"),
    ("authentification", "5 authentification"),
    ("idcc_prestation", "si different n idcc"),
    ("embauche", "date d embauche"),
    ("naissance", "date de naissance"),
    ("attestation_initiale", "n de l attestation initiale"),
    ("organisateur", "organisateur occasionnel"),
    ("ape", "code ape naf"),
    ("affiliation", "n d affiliation"),
    ("attestation", "attestation n"),
    ("nom", "nom de famille"),
    ("usage", "nom d usage"),
    ("emploi", "emploi occupe"),
    ("objet", "numero objet"),
    ("en_cours", "contrat en cours"),
    ("raison", "raison sociale"),
    ("code_postal", "code postal"),
    ("employeur", "2 employeur"),
    ("salarie", "3 salarie"),
    ("siret", "n siret"),
    ("idcc", "n idcc"),
    ("brut", "salaires bruts"),
    ("retraite", "regime de"),
    ("motif", "motif de"),
    ("prenom", "prenom"),
    ("nir", "nir"),
    ("adresse", "adresse"),
    ("commune", "commune"),
    ("telephone", "telephone"),
    ("courriel", "courriel"),
    ("mois", "mois"),
    ("sinon", "sinon"),
    ("cadre", "cadre"),
    ("heures", "heures"),
];

/// Les en-tetes des colonnes de nombres du mois.
const COLONNES: [(&str, &str); 4] = [("heures", "heures"), ("isoles", "isoles"), ("groupes", "groupes"), ("jours", "jours")];

/// Deux mots sont sur la meme ligne quand leurs milieux verticaux sont a moins de cela.
const LIGNE: f64 = 3.5;

/// Ce qu'une AEM en PDF dit, page par page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aem {
    pub page: usize,
    /// Le mois de l'attestation (`AAAA-MM`).
    pub mois: Option<String>,
    pub numero: Option<String>,
    pub siret: Option<String>,
    pub ape: Option<String>,
    pub idcc: Option<String>,
    pub employeur: Option<String>,
    pub employeur_code_postal: Option<String>,
    pub employeur_commune: Option<String>,
    /// Le numero d'adherent a la caisse des Congés Spectacles, ecrit comme la fiche le garde.
    pub adherent: Option<String>,
    pub nir: Option<String>,
    pub cle_nir: Option<String>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
    pub nom_usage: Option<String>,
    pub naissance: Option<Jour>,
    pub rue: Option<String>,
    pub code_postal: Option<String>,
    pub commune: Option<String>,
    pub emploi: Option<String>,
    pub numero_objet: Option<String>,
    pub cadre: Option<bool>,
    /// « artiste » (annexe 10) ou « technicien » (annexe 8).
    pub statut: Option<&'static str>,
    /// La date d'embauche et la date de fin du contrat, telles qu'imprimees.
    pub embauche: Option<Jour>,
    pub fin_contrat: Option<Jour>,
    pub en_cours: bool,
    pub heures: Option<i64>,
    pub cachets_isoles: Option<i64>,
    pub cachets_groupes: Option<i64>,
    pub jours: Option<i64>,
    pub brut: Option<Decimal>,
    /// « complementaire » ou « rectificative » quand la case est cochee.
    pub corrige: Option<&'static str>,
    /// Ce que la page dit d'elle-meme qui ne tient pas (une cle de NIR fausse, un nombre hors colonne).
    pub faute: Option<String>,
}

impl Aem {
    /// La periode que l'AEM atteste : le contrat borne au mois de l'attestation.
    pub fn periode(&self) -> Option<(Jour, Jour)> {
        let (an, m) = self.mois.as_deref().and_then(|t| Some((t.get(..4)?.parse::<i32>().ok()?, t.get(5..7)?.parse::<u32>().ok()?)))?;
        let premier = Jour::nouveau(an, m, 1)?;
        let dernier = Jour::nouveau(an, m, socle::date::jours_du_mois(an, m))?;
        let debut = self.embauche?.max(premier);
        let fin = match (self.fin_contrat, self.en_cours) {
            (Some(f), _) => f.min(dernier),
            (None, true) => dernier,
            (None, false) => return None,
        };
        (debut <= fin).then_some((debut, fin))
    }

    /// Les cachets du mois : isoles et groupes.
    pub fn cachets(&self) -> i64 {
        self.cachets_isoles.unwrap_or(0) + self.cachets_groupes.unwrap_or(0)
    }

    /// Ce qui manque pour que l'AEM atteste un contrat ; `None` : rien.
    pub fn manque(&self) -> Option<String> {
        if let Some(c) = self.corrige {
            return Some(format!(
                "AEM {} : elle corrige une attestation initiale que l'import ne lit pas, elle ne se reprend pas seule ; déposez le bulletin du même mois",
                c
            ));
        }
        if let Some(f) = &self.faute {
            return Some(format!("AEM : {}", f));
        }
        let mut m: Vec<&str> = Vec::new();
        if self.nir.is_none() {
            m.push("le NIR");
        }
        if self.mois.is_none() {
            m.push("le mois");
        }
        if self.embauche.is_none() || (self.fin_contrat.is_none() && !self.en_cours) {
            m.push("les dates du contrat");
        }
        if self.heures.is_none() && self.cachets_isoles.is_none() && self.cachets_groupes.is_none() && self.jours.is_none() {
            m.push("les heures, cachets et jours");
        }
        if self.brut.is_none() {
            m.push("le salaire brut");
        }
        if m.is_empty() && self.periode().is_none() {
            m.push("une période dans le mois de l'attestation");
        }
        (!m.is_empty()).then(|| format!("AEM dont {} ne se lit pas", m.join(", ")))
    }

    /// Ce que l'AEM dit en commun avec un certificat d'emploi : l'employeur, le salarie, le contrat du mois.
    pub fn commun(&self) -> Certificat {
        let periode = self.periode();
        let mut adresse: Vec<String> = self.rue.iter().cloned().collect();
        if let (Some(cp), Some(c)) = (&self.code_postal, &self.commune) {
            adresse.push(format!("{} {}", cp, c));
        }
        Certificat {
            page: self.page,
            siret: self.siret.clone(),
            compte: self.adherent.clone(),
            employeur: self.employeur.clone(),
            numero: self.numero.clone(),
            nir: self.nir.clone(),
            cle_nir: self.cle_nir.clone(),
            nom: self.nom.clone(),
            prenom: self.prenom.clone(),
            nom_usage: self.nom_usage.clone(),
            pseudonyme: None,
            nom_complet: match (&self.prenom, &self.nom) {
                (Some(p), Some(n)) => Some(format!("{} {}", p, n)),
                _ => None,
            },
            adresse,
            emploi: self.emploi.clone(),
            cadre: self.cadre,
            debut: periode.map(|p| p.0),
            fin: periode.map(|p| p.1),
            nombre: None,
            base_conge: None,
            brut: self.brut,
            signe_le: None,
        }
    }
}

/// Une page d'AEM : lue, refusee (et ce qui s'en est lu), ou la notice du formulaire.
#[derive(Clone, Debug)]
pub enum Lecture {
    Lue(Aem),
    Refusee { sort: &'static str, raison: String, partiel: Option<Aem> },
    Notice,
}

#[derive(Clone, Debug)]
pub struct PageAem {
    pub page: usize,
    pub lecture: Lecture,
}

/// Les pages du PDF qui sont des AEM (ou leur notice), lues. Les autres pages n'y sont pas.
pub fn lire(chemin: &Path) -> Result<Vec<PageAem>, String> {
    let (texte, _) = fabrique_pdf::inspection::inspecter(&chemin.to_string_lossy()).map_err(|r| format!("PDF que la fabrique n'ouvre pas : {}", r.0))?;
    let releve: Value = serde_json::from_str(&texte).map_err(|e| format!("releve du PDF illisible ({})", e))?;
    let pages: Vec<Vec<Mot>> = releve.get("pages").and_then(Value::as_array).map(|ps| ps.iter().map(mots_de).collect()).unwrap_or_default();
    let notices: Vec<bool> = pages.iter().map(|m| est_notice(m)).collect();
    let mut out = Vec::new();
    for (i, mots) in pages.iter().enumerate() {
        let page = i + 1;
        if notices[i] {
            out.push(PageAem { page, lecture: Lecture::Notice });
        } else if notices.iter().any(|n| *n) && illisible(mots) {
            out.push(PageAem { page, lecture: Lecture::Refusee { sort: "sans_texte", raison: ILLISIBLE.into(), partiel: None } });
        } else if let Some(a) = lire_page(mots, page) {
            let lecture = match a.manque() {
                Some(raison) => Lecture::Refusee { sort: "incomplet", raison, partiel: Some(a) },
                None => Lecture::Lue(a),
            };
            out.push(PageAem { page, lecture });
        }
    }
    Ok(out)
}

fn mots_de(p: &Value) -> Vec<Mot> {
    let mut mots = Vec::new();
    for m in p.get("mots").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        let texte = m.get("texte").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let r: Vec<f64> = m.get("rect").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if texte.is_empty() || r.len() != 4 {
            continue;
        }
        mots.push(Mot { texte, x0: r[0], y0: r[1], x1: r[2], y1: r[3] });
    }
    mots
}

/// Les jetons de la page, a la suite (dans l'ordre de lecture), pour reconnaitre ce qu'elle est.
fn texte_en_jetons(mots: &[Mot]) -> String {
    let mut m: Vec<&Mot> = mots.iter().collect();
    m.sort_by(|a, b| a.y0.total_cmp(&b.y0).then(a.x0.total_cmp(&b.x0)));
    format!(" {} ", m.iter().flat_map(|w| jetons(&w.texte)).collect::<Vec<_>>().join(" "))
}

/// La notice du formulaire (« RAPPEL DES OBLIGATIONS DU SALARIE », « l'attestation mensuelle ») : rien a reprendre.
fn est_notice(mots: &[Mot]) -> bool {
    let t = texte_en_jetons(mots);
    t.contains(" rappel des obligations du salarie ") && t.contains(" attestation mensuelle ")
}

/// Une page dont la moitie des mots au moins sortent en signes sans lettres.
fn illisible(mots: &[Mot]) -> bool {
    !mots.is_empty() && mots.iter().filter(|m| m.texte.contains('\u{FFFD}')).count() * 2 >= mots.len()
}

/// Les mots sur la ligne de `m`, a sa droite, de gauche a droite (lui compris).
fn ligne_depuis(mots: &[Mot], m: usize) -> Vec<usize> {
    let cy = mots[m].cy();
    let mut v: Vec<usize> = (0..mots.len()).filter(|&k| (mots[k].cy() - cy).abs() <= LIGNE && mots[k].x0 >= mots[m].x0).collect();
    v.sort_by(|a, b| mots[*a].x0.total_cmp(&mots[*b].x0));
    v
}

/// Les rubriques de la page. Un mot ne sert qu'a une rubrique ; les plus longues se cherchent d'abord.
fn rubriques(mots: &[Mot]) -> Vec<Rubrique> {
    let mut prises = vec![false; mots.len()];
    let mut out: Vec<Rubrique> = Vec::new();
    let mut ordre: Vec<usize> = (0..mots.len()).collect();
    ordre.sort_by(|a, b| mots[*a].y0.total_cmp(&mots[*b].y0).then(mots[*a].x0.total_cmp(&mots[*b].x0)));
    for (cle, phrase) in RUBRIQUES {
        let p: Vec<&str> = phrase.split(' ').collect();
        for &m in &ordre {
            if prises[m] || jetons(&mots[m].texte).first().map(String::as_str) != Some(p[0]) {
                continue;
            }
            // Les jetons de la ligne, chacun avec son mot ; la rubrique commence au premier jeton du mot.
            let ligne = ligne_depuis(mots, m);
            let js: Vec<(String, usize)> = ligne.iter().flat_map(|&k| jetons(&mots[k].texte).into_iter().map(move |j| (j, k))).collect();
            if js.len() < p.len() || !p.iter().enumerate().all(|(i, x)| js[i].0 == *x) {
                continue;
            }
            let mut faits: Vec<usize> = js[..p.len()].iter().map(|j| j.1).collect();
            faits.dedup();
            // Les mots d'une rubrique se suivent (pas de valeur entre eux), et ne finissent pas au milieu d'un mot.
            let fin_dans_le_mot = js.get(p.len()).is_some_and(|j| j.1 == *faits.last().unwrap());
            let serres = faits.windows(2).all(|w| mots[w[1]].x0 - mots[w[0]].x1 < 15.0);
            if fin_dans_le_mot || !serres || faits.iter().any(|k| prises[*k]) {
                continue;
            }
            for k in &faits {
                prises[*k] = true;
            }
            let b = |f: fn(&Mot) -> f64, min: bool| faits.iter().map(|k| f(&mots[*k])).fold(if min { f64::MAX } else { f64::MIN }, |a, x| if min { a.min(x) } else { a.max(x) });
            out.push(Rubrique { cle, x0: b(|w| w.x0, true), x1: b(|w| w.x1, false), y0: b(|w| w.y0, true), y1: b(|w| w.y1, false), mots: faits });
        }
    }
    out
}

/// La page : `None` si ce n'est pas une AEM (son titre, les annexes 8 et 10, ses rubriques de contrat).
fn lire_page(mots: &[Mot], page: usize) -> Option<Aem> {
    let t = texte_en_jetons(mots);
    let titre = t.contains(" attestation aem ") || t.contains(" attestation d employeur mensuelle ") || t.contains(" attestation employeur mensuelle ");
    let annexes = t.contains(" annexes 8 et 10 ") || t.contains(" annexes viii et x ");
    if !titre || !annexes || !t.contains(" salaires bruts ") || !t.contains(" date d embauche ") {
        return None;
    }
    let rs = rubriques(mots);
    let pris: std::collections::HashSet<usize> = rs.iter().flat_map(|r| r.mots.iter().copied()).collect();
    let marques: Vec<usize> = (0..mots.len()).filter(|&k| MARQUES.contains(&mots[k].texte.as_str())).collect();
    // Les sections du formulaire, par leur titre : 2 EMPLOYEUR, 3 SALARIE, 4 PRESTATION DE TRAVAIL, 5 AUTHENTIFICATION.
    let haut_de = |cle: &str| rs.iter().find(|r| r.cle == cle).map(|r| r.y0);
    let bornes = [haut_de("employeur"), haut_de("salarie"), haut_de("prestation"), haut_de("authentification")];
    let section = |y: f64| -> usize { bornes.iter().filter(|b| b.is_some_and(|b| y >= b - 1.0)).count() };

    // La valeur d'une rubrique : les mots a sa droite, sur sa ligne, jusqu'a la rubrique suivante de la ligne.
    let valeur_de = |r: &Rubrique| -> Vec<String> {
        let tol = (r.y1 - r.y0).max(4.0) * 0.6;
        let suivante = rs.iter().filter(|o| o.x0 > r.x1 && (o.cy() - r.cy()).abs() <= tol).map(|o| o.x0).fold(f64::MAX, f64::min);
        let mut v: Vec<usize> = (0..mots.len())
            .filter(|&k| !pris.contains(&k) && !marques.contains(&k) && mots[k].x0 >= r.x1 - 1.0 && mots[k].x0 < suivante && (mots[k].cy() - r.cy()).abs() <= tol)
            .collect();
        v.sort_by(|a, b| mots[*a].x0.total_cmp(&mots[*b].x0));
        v.into_iter().map(|k| mots[k].texte.clone()).collect()
    };
    // La premiere rubrique `cle` de la section `s` (toutes si `None`) qui a une valeur.
    let lu = |cle: &str, s: Option<usize>| -> Vec<String> {
        rs.iter().filter(|r| r.cle == cle && s.is_none_or(|s| section(r.y0) == s)).map(&valeur_de).find(|v| !v.is_empty()).unwrap_or_default()
    };
    let texte = |v: Vec<String>| -> Option<String> {
        let t = v.join(" ").trim().to_string();
        (!t.is_empty()).then_some(t)
    };

    let mut a = Aem { page, ..Default::default() };
    // Le mois : ses chiffres, imprimes case a case (« 0 4 20 2 3 ») : MM puis AAAA.
    let chiffres: String = lu("mois", Some(0)).iter().take_while(|m| m.chars().all(|c| c.is_ascii_digit())).map(String::as_str).collect();
    if let (6, Some(m), Some(an)) = (chiffres.len(), chiffres.get(..2).and_then(|x| x.parse::<u32>().ok()), chiffres.get(2..).and_then(|x| x.parse::<i32>().ok())) {
        if (1..=12).contains(&m) {
            a.mois = Some(format!("{:04}-{:02}", an, m));
        }
    }
    a.numero = lu("attestation", Some(0)).first().cloned();
    let siret: String = lu("siret", Some(1)).concat().chars().filter(char::is_ascii_digit).collect();
    a.siret = (siret.len() == 14).then_some(siret);
    a.ape = lu("ape", Some(1)).first().cloned();
    a.idcc = lu("idcc", Some(1)).first().filter(|x| x.len() == 4 && x.chars().all(|c| c.is_ascii_digit())).cloned();
    a.employeur = texte(lu("raison", Some(1)));
    a.employeur_code_postal = lu("code_postal", Some(1)).first().filter(|x| x.len() == 5 && x.chars().all(|c| c.is_ascii_digit())).cloned();
    a.employeur_commune = texte(lu("commune", Some(1)));
    a.adherent = lu("adherent", Some(1))
        .iter()
        .map(|x| documents::conges_spectacles::forme_adherent(x))
        .find(|x| documents::conges_spectacles::faute_adherent(x).is_none());

    a.nom = texte(lu("nom", Some(2)));
    a.prenom = texte(lu("prenom", Some(2)));
    a.nom_usage = texte(lu("usage", Some(2)));
    let mut fautes: Vec<&str> = Vec::new();
    if let Some((nir, cle, _)) = nir_en_tete(&lu("nir", Some(2))) {
        match &cle {
            Some(k) if declarations::champs::cle_nir(&nir).as_deref() != Some(k.as_str()) => fautes.push("la clé du NIR imprimé ne correspond pas à son numéro"),
            _ => {
                a.nir = Some(nir);
                a.cle_nir = cle;
            }
        }
    }
    a.naissance = date_en_tete(&lu("naissance", Some(2))).map(|d| d.0);
    a.rue = texte(lu("adresse", Some(2)));
    a.code_postal = lu("code_postal", Some(2)).first().filter(|x| x.len() == 5 && x.chars().all(|c| c.is_ascii_digit())).cloned();
    a.commune = texte(lu("commune", Some(2)));

    a.emploi = texte(lu("emploi", Some(3)));
    a.numero_objet = lu("objet", Some(3)).first().cloned();
    a.embauche = date_en_tete(&lu("embauche", Some(3))).map(|d| d.0);
    a.fin_contrat = date_en_tete(&lu("fin", Some(3))).map(|d| d.0);

    // Les cases cochees : le mot dont la marque suit le bord droit.
    for &k in &marques {
        let m = &mots[k];
        let Some(l) = (0..mots.len())
            .filter(|&j| j != k && mots[j].x1 <= m.x0 + 1.0 && m.x0 - mots[j].x1 <= 12.0 && (mots[j].cy() - m.cy()).abs() <= 5.0)
            .min_by(|a, b| (m.x0 - mots[*a].x1).total_cmp(&(m.x0 - mots[*b].x1)))
        else {
            continue;
        };
        let lib = jetons(&mots[l].texte).last().cloned().unwrap_or_default();
        let s = section(mots[l].y0);
        match (s, lib.as_str()) {
            (0, "complementaire") => a.corrige = Some("complémentaire"),
            (0, "positive" | "negative") => a.corrige = Some("rectificative"),
            (3, "artiste") => a.statut = Some("artiste"),
            (3, "technicien" | "ouvrier") => a.statut = Some("technicien"),
            (3, "cours") => a.en_cours = true,
            (3, "cadre") => {
                // « Non cadre » : « Non » juste avant, ou juste au-dessus (le libelle sur deux rangs).
                let w = &mots[l];
                let non = mots.iter().any(|o| {
                    jetons(&o.texte) == ["non"]
                        && (((o.cy() - w.cy()).abs() <= LIGNE && w.x0 - o.x1 >= 0.0 && w.x0 - o.x1 < 8.0)
                            || ((o.x0 - w.x0).abs() <= 3.0 && w.cy() - o.cy() > 2.0 && w.cy() - o.cy() < 12.0))
                });
                a.cadre = Some(!non);
            }
            _ => {}
        }
    }

    // Les nombres du mois, par colonne : sous les en-tetes, au-dessus de « SALAIRES BRUTS ».
    let en_tetes: Vec<(&str, &Mot)> = COLONNES
        .iter()
        .filter_map(|(c, j)| mots.iter().find(|m| section(m.y0) == 3 && jetons(&m.texte).first().map(String::as_str) == Some(*j)).map(|m| (*c, m)))
        .collect();
    let brut_tete = rs.iter().filter(|r| r.cle == "brut" && section(r.y0) == 3).min_by(|a, b| a.x0.total_cmp(&b.x0));
    if !en_tetes.is_empty() {
        let haut = en_tetes.iter().map(|(_, m)| m.y1).fold(f64::MIN, f64::max);
        let bas = brut_tete.map_or(f64::MAX, |r| r.y0);
        let mut par_colonne: std::collections::BTreeMap<&str, Vec<i64>> = std::collections::BTreeMap::new();
        let mut ambigu = false;
        for (k, m) in mots.iter().enumerate() {
            if pris.contains(&k) || m.y0 < haut || m.y1 > bas {
                continue;
            }
            let Some(n) = entier(&m.texte) else { continue };
            match en_tetes.iter().map(|(c, t)| (*c, (t.cx() - m.cx()).abs())).filter(|x| x.1 <= 45.0).min_by(|a, b| a.1.total_cmp(&b.1)) {
                Some((c, _)) => par_colonne.entry(c).or_default().push(n),
                None => ambigu = true,
            }
        }
        if ambigu || par_colonne.values().any(|v| v.len() > 1) {
            fautes.push("un nombre du mois ne tombe sous aucune colonne, ou une colonne en porte deux");
        } else {
            let v = |c: &str| par_colonne.get(c).map_or(0, |x| x[0]);
            let a_l_en_tete = |c: &str| en_tetes.iter().any(|x| x.0 == c);
            if a_l_en_tete("heures") && a_l_en_tete("jours") && (a_l_en_tete("isoles") || a_l_en_tete("groupes")) {
                a.heures = Some(v("heures"));
                a.cachets_isoles = Some(v("isoles"));
                a.cachets_groupes = Some(v("groupes"));
                a.jours = Some(v("jours"));
            }
        }
    }
    // Le brut : le nombre sous « SALAIRES BRUTS », a son aplomb.
    if let Some(r) = brut_tete {
        let cx = (r.x0 + r.x1) / 2.0;
        a.brut = mots
            .iter()
            .filter(|m| m.y0 > r.y1 && m.y0 < r.y1 + 40.0 && (m.cx() - cx).abs() <= (r.x1 - r.x0) / 2.0 + 10.0)
            .filter_map(|m| crate::import::bulletin_lu::lire_nombre(&m.texte).filter(|_| m.texte.chars().any(|c| c.is_ascii_digit())).map(|n| (m.y0, n)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1);
    }
    a.faute = fautes.first().map(|f| f.to_string());
    Some(a)
}

/// Un nombre entier du mois (« 1 », « 12 », « 8,00 ») ; `None` pour ce qui n'en est pas un.
fn entier(t: &str) -> Option<i64> {
    if !t.chars().all(|c| c.is_ascii_digit() || c == ',') || !t.chars().next()?.is_ascii_digit() {
        return None;
    }
    let (e, d) = t.split_once(',').unwrap_or((t, ""));
    (d.chars().all(|c| c == '0')).then(|| e.parse().ok()).flatten()
}
