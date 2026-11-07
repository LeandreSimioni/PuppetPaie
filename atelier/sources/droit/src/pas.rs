//! Le prelevement a la source : le taux et son identifiant, tels que la DGFiP les donne (les taux recus, dans la fiche du
//! salarie, `CHAMP`), et la grille de l'article 204 H du CGI quand aucun ne vaut. Ce que le calcul d'un engagement et la
//! DSN lisent.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize, Serializer};

use crate::contexte::Contexte;
use crate::fiche::{Lue, Montant, TauxPas};
use crate::forme_bareme::{GrillePas, PasParDefaut, Pile};
use socle::date::{jours_du_mois, Jour};
use socle::num::nombre;
use socle::fichiers::repr;
use socle::num::{eur, pct, sub, Decimal, Resultat};

/// LE TAUX RECU APPARTIENT AU SALARIE : chaque taux que la DGFiP transmet (un compte rendu de nature 94) s'ecrit dans
/// sa fiche, valeur datee du jour ou il commence de valoir (`date_d_effet`) : `{"taux", "type", "identifiant",
/// "mis_a_disposition", "en_retour_de"?}`, `mis_a_disposition` en AAAAMMJJHHMMSS, l'instant du compte rendu, et
/// `en_retour_de` (`EN_RETOUR_DE`) le mois de la DSN a laquelle il repond.
pub const CHAMP: &str = "taux_pas";

/// Un refus du module. `SansGrille` est `BaremeSansGrille` (un bareme sans la grille de l'article 204 H : le prelevement
/// ne se devine pas, le calcul refuse) ; `Inconnu` est `PasInconnu`.
#[derive(Debug, Clone, PartialEq)]
pub enum ErreurPas {
    SansGrille(String),
    Inconnu(String),
    Autre(String),
}

impl ErreurPas {
    pub fn message(&self) -> &str {
        match self {
            ErreurPas::SansGrille(m) | ErreurPas::Inconnu(m) | ErreurPas::Autre(m) => m,
        }
    }
}

impl From<String> for ErreurPas {
    fn from(m: String) -> ErreurPas {
        ErreurPas::Autre(m)
    }
}

type R<T> = Result<T, ErreurPas>;

/// Un taux qui vaut : taux, type, identifiant, tels que le compte rendu les donne.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Triplet {
    pub taux: Option<Montant>,
    pub type_taux: Option<String>,
    pub identifiant: Option<String>,
}

/// Le texte d'une case telle qu'ecrite ; `None` : vide.
fn ecrit(c: &Option<String>) -> Option<String> {
    c.portee().map(str::to_string)
}

// ------------------------------------------------------------- peremption

/// Un taux transmis par la DGFiP vaut jusqu'a la fin du deuxieme mois suivant
/// celui de sa mise a disposition : le debiteur « applique le taux calculé par
/// l'administration fiscale, au plus tard le deuxième mois suivant sa
/// transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) ; « le taux de prélèvement est
/// applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition » :
/// mis a disposition le 10 fevrier, il est « valide jusqu'au 30 avril »
/// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/
/// BOI-IR-PAS-30-10-20-20181203.html`).
pub const VALIDITE_MOIS: u32 = 2;

/// Les `n` premiers caracteres, s'ils sont tous des chiffres.
fn chiffres(s: &str, n: usize) -> Option<Vec<u32>> {
    let c: Vec<char> = s.chars().take(n).collect();
    if c.len() == n && c.iter().all(|x| x.is_ascii_digit()) {
        Some(c.iter().map(|x| x.to_digit(10).unwrap()).collect())
    } else {
        None
    }
}

fn nombre_de(c: &[u32]) -> u32 {
    c.iter().fold(0, |a, x| a * 10 + x)
}

/// Le jour de la mise a disposition d'un taux recu (`mis_a_disposition`, AAAAMMJJHHMMSS) : d'ou court sa validite.
pub fn mise_a_disposition(e: &TauxPas) -> Option<Jour> {
    let c = chiffres(e.mis_a_disposition.portee()?.trim(), 8)?;
    Jour::nouveau(nombre_de(&c[..4]) as i32, nombre_de(&c[4..6]), nombre_de(&c[6..8]))
}

/// Une date `AAAA-MM-JJ` stricte.
fn depuis_iso(s: &str) -> Resultat<Jour> {
    let faux = || format!("date illisible : {}", repr(s));
    let c: Vec<char> = s.chars().collect();
    if c.len() != 10 || c[4] != '-' || c[7] != '-' {
        return Err(faux());
    }
    let tranche = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    match (chiffres(&tranche(0, 4), 4), chiffres(&tranche(5, 7), 2), chiffres(&tranche(8, 10), 2)) {
        (Some(a), Some(m), Some(j)) => Jour::nouveau(nombre_de(&a) as i32, nombre_de(&m), nombre_de(&j)).ok_or_else(faux),
        _ => Err(faux()),
    }
}

/// Dernier jour ou ce taux est celui du salarie ; `None` si on ne sait pas.
pub fn fin_de_validite(e: &TauxPas) -> Option<Jour> {
    let debut = mise_a_disposition(e)?;
    let mois = debut.mois + VALIDITE_MOIS;
    let (an, m) = (debut.an + ((mois - 1) / 12) as i32, (mois - 1) % 12 + 1);
    Jour::nouveau(an, m, jours_du_mois(an, m))
}

/// Un taux qu'on ne sait pas dater est traite comme perime ; le bareme (tout
/// type autre que 01) ne se perime pas.
pub fn est_perime(e: &TauxPas, le: Jour) -> bool {
    if e.r#type.portee() != Some("01") {
        return false;
    }
    fin_de_validite(e).is_none_or(|fin| le > fin)
}

// ------------------------------------------------------------------ lecture

/// Vrai seulement si la fiche est la ET qu'elle n'a pas de NIR.
pub fn sans_nir(ctx: &Contexte, cle: &str) -> bool {
    ctx.salaries.get(cle).is_some_and(|f| f.nir.portee().is_none_or(|n| n.trim().is_empty()))
}

/// Le jour auquel on demande si un taux vaut encore : celui du contexte.
pub fn le_jour(ctx: &Contexte, le: Option<Jour>) -> Jour {
    match le {
        Some(j) => j,
        None => {
            socle::fichiers::noter_valeur("date", ctx.date.iso());
            ctx.date
        }
    }
}

/// LES TAUX RECUS POUR LE SALARIE `cle`, du premier mis a disposition au dernier (a instant egal, dans l'ordre ou ils
/// se sont ecrits). Une valeur de la fiche qui n'est pas un taux (la case vide d'avant le premier) n'en est pas un.
pub fn recus(ctx: &Contexte, cle: &str) -> Resultat<Vec<TauxPas>> {
    let Some(fiche) = ctx.salarie_brut(cle) else { return Ok(Vec::new()) };
    let mut ecrits: Vec<_> = fiche.taux_pas.iter().flatten().collect();
    ecrits.sort_by_key(|e| e.depuis);
    let mut r: Vec<TauxPas> = ecrits.into_iter().filter_map(|e| e.valeur.dite()).cloned().collect();
    r.sort_by_key(|v| v.mis_a_disposition.portee().map(|m| m.trim().to_string()));
    Ok(r)
}

/// Dans un taux recu : le mois (AAAA-MM) de la DSN mensuelle a laquelle repond le compte rendu qui l'a transmis ; absent
/// pour la reponse a une demande de taux (signalement d'amorcage) et pour un taux repris d'une DSN deposee.
pub const EN_RETOUR_DE: &str = "en_retour_de";

/// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT, JAMAIS POUR CELUI QU'ELLE DECLARE. Le compte
/// rendu est « établi par l'administration fiscale en retour de chaque déclaration sociale nominative » (BOI-IR-PAS-30-10-20,
/// § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`) ; le taux de celui qui repond a la DSN de
/// janvier sert a « précompter la retenue à la source sur les revenus versés au titre des mois de février, mars et avril »
/// (§ 70, exemple 1). La DSN d'un mois a deja declare le taux « précompté appliqué » a chacun de ses versements
/// (S21.G00.50.006, `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`) ; le compte rendu qui lui repond porte « un
/// identifiant propre » (CSS, art. R. 133-13, V, 1°, `atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261030.md`) : s'il valait pour les versements qu'elle declare, chaque DSN deposee avant eux serait a
/// refaire par sa propre reponse, et son annule-et-remplace par la sienne. INFERENCE (les textes supposent la DSN deposee
/// apres le versement) : un taux en retour de la DSN du mois `M` ne vaut pas pour la paie de `M`, ni d'un mois d'avant.
fn en_retour_d_un_mois_qui_n_est_pas_avant(e: &TauxPas, mois: Option<&str>) -> bool {
    let retour = e.en_retour_de.dite().map(|m| m.to_string()).filter(|r| !r.is_empty());
    mois.zip(retour).is_some_and(|(m, retour)| retour.as_str() >= m)
}

/// Le jour ou un taux recu commence de valoir dans la fiche (`depuis` de sa valeur datee) : le jour de sa mise a
/// disposition `mad`, et, en retour de la DSN du mois `en_retour_de`, pas avant le premier jour du mois qui suit
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Deux comptes rendus du meme jour, l'un pour le mois, l'autre en retour
/// de sa DSN, ne s'ecrasent donc pas.
pub fn date_d_effet(mad: Jour, en_retour_de: Option<&str>) -> Resultat<Jour> {
    let premier_d_apres = en_retour_de.map(|m| socle::mois::Mois::lire(m).map(|m| m.suivant().premier_jour())).transpose()?;
    Ok(match premier_d_apres {
        Some(p) => p.max(mad),
        None => mad,
    })
}

/// LE TAUX DU SALARIE AU JOUR `le`, pour la paie du mois `mois` : de ceux qu'il a recus, le dernier mis a disposition au
/// plus tard ce jour-la, hors ceux qui repondent a la DSN de ce mois ou d'un mois d'apres
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Le debiteur « applique le taux calculé par l'administration fiscale, au
/// plus tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) : un taux vaut des sa mise a disposition (BOI-IR-PAS-30-10-20, § 70 et
/// 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`), pour toute retenue faite a partir de ce
/// jour ; la retenue est « effectuée par le débiteur lors du paiement » (CGI, art. 1671, 1, meme fichier) : un taux
/// transmis apres le paiement d'un salaire ne vaut pas pour lui. Aucun taux recu avant ce jour : aucun (la grille de
/// l'article 204 H s'applique).
fn en_vigueur<'a>(recus: &'a [TauxPas], le: Jour, mois: Option<&str>) -> Option<&'a TauxPas> {
    recus.iter().filter(|e| mise_a_disposition(e).is_some_and(|d| d <= le) && !en_retour_d_un_mois_qui_n_est_pas_avant(e, mois)).last()
}

// ------------------------------------------ la grille de l'article 204 H du CGI

pub const ZONE_DEFAUT: &str = "metropole";

/// La section des taux par defaut du bareme (`pas_taux_par_defaut`).
fn section(bareme: &Pile) -> R<&PasParDefaut> {
    bareme.pas_taux_par_defaut.as_ref().ok_or_else(|| {
        ErreurPas::SansGrille(
            "Le bareme ne porte pas de section `pas_taux_par_defaut` : sans \
             elle, le moteur ne sait pas quel taux appliquer a qui n'a pas de \
             taux DGFiP.\n\
             Elle se releve sur BOI-BAREME-000037 (grilles des taux par \
             defaut et abattement pour les contrats courts), et le socle \
             livre de l'annee en cours la porte."
                .to_string(),
        )
    })
}

/// L'abattement contrats courts en vigueur : la moitie du SMIC net imposable (le bareme le porte a sa date).
pub fn abattement(bareme: &Pile) -> R<Decimal> {
    section(bareme)?.abattement_contrats_courts.map(|n| n.0).ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent("abattement_contrats_courts")))
}

/// La grille en vigueur a cette date pour cette zone (elles changent au 1er mai).
pub fn grille<'a>(bareme: &'a Pile, le: Option<Jour>, zone: &str) -> R<&'a GrillePas> {
    let grilles = &section(bareme)?.grilles;
    let mut meilleure: Option<&GrillePas> = None;
    for g in grilles.iter().filter(|g| g.zone == zone) {
        if let Some(l) = le {
            if depuis_iso(&g.depuis)? > l {
                continue;
            }
        }
        // `max` garde la premiere des egales.
        if meilleure.is_none_or(|m| g.depuis > m.depuis) {
            meilleure = Some(g);
        }
    }
    meilleure.ok_or_else(|| {
        let connues: BTreeSet<&str> = grilles.iter().map(|g| g.zone.as_str()).collect();
        ErreurPas::SansGrille(format!(
            "Aucune grille de taux par defaut pour la zone {} au {}. Zones \
             connues du bareme : {}.",
            repr(zone),
            match le {
                Some(j) => j.iso(),
                None => "?".to_string(),
            },
            connues.into_iter().collect::<Vec<_>>().join(", ")
        ))
    })
}

/// `(taux, type de taux)` pour un montant DEJA abattu.
pub fn taux_par_defaut(bareme: &Pile, montant: Decimal, le: Option<Jour>, zone: &str) -> R<(String, String)> {
    let g = grille(bareme, le, zone)?;
    let nombre = |x: f64| dec_avec_echelle_f64(x);
    for t in &g.tranches {
        let (bas, haut, taux) = match t.as_slice() {
            [Some(b), h, Some(x)] => (*b, *h, *x),
            _ => return Err(ErreurPas::Autre(format!("tranche de la grille du PAS illisible (bas, haut, taux attendus) : {:?}", t))),
        };
        // La derniere tranche n'a pas de haut.
        if montant >= nombre(bas)? && haut.map(nombre).transpose()?.is_none_or(|h| montant < h) {
            return Ok((format!("{:.2}", taux), g.genre.clone()));
        }
    }
    // Seul un montant negatif tombe ici : taux zero.
    Ok(("0.00".to_string(), g.genre.clone()))
}

/// Le decimal d'un flottant du bareme, son echelle gardee (sa forme ecrite, `socle::date::repr_flottant`).
fn dec_avec_echelle_f64(x: f64) -> Resultat<Decimal> {
    let s = socle::date::repr_flottant(x);
    s.parse::<Decimal>().or_else(|_| Decimal::from_scientific(&s)).map_err(|_| format!("nombre hors de portee : {}", s))
}

/// LE PRELEVEMENT D'UN VERSEMENT, par l'un des deux chemins : tel que la paie l'ecrit et le relit (les montants en
/// texte, `socle::num::nombre`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Versement {
    pub taux: Option<Montant>,
    #[serde(rename = "type")]
    pub type_taux: Option<String>,
    pub identifiant: Option<String>,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub abattement: Decimal,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub montant_soumis: Decimal,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub prelevement: Decimal,
}

fn en_nombre<S: Serializer>(x: &Decimal, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&nombre(*x))
}

/// Le taux d'un versement en decimal, son echelle gardee ; un versement sans taux ne se calcule pas.
pub fn taux_du_versement(taux: Option<&Montant>) -> Resultat<Decimal> {
    taux.ok_or_else(|| "nombre attendu, lu null".to_string())?.avec_echelle()
}

pub fn plancher(x: Decimal, zero: Decimal) -> Decimal {
    if zero > x {
        zero
    } else {
        x
    }
}

/// Tout le bloc 50 cote PAS quand aucun taux DGFiP ne s'applique ; pas
/// d'identifiant (S21.G00.50.008 est interdite hors taux DGFiP).
pub fn rubriques_par_defaut(
    bareme: &Pile,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> R<Versement> {
    let rnf = eur(remuneration_nette_fiscale);
    let zero = Decimal::new(0, 2);
    let abat = if contrat_court { abattement(bareme)? } else { zero };
    // L'abattement ne creuse pas : il se limite a ce qu'il y a a abattre.
    let abat = eur(abat.min(plancher(rnf, zero)));
    let soumis = eur(plancher(sub(rnf, abat), zero));
    let (taux, type_taux) = taux_par_defaut(bareme, soumis, le, zone)?;
    let prelevement = pct(soumis, socle::num::dec_texte(&taux)?);
    Ok(Versement {
        taux: Some(Montant::Texte(taux)),
        type_taux: Some(type_taux),
        identifiant: None,
        abattement: abat,
        montant_soumis: soumis,
        prelevement,
    })
}

// ------------------------------------------- lequel des deux chemins, et pour
//                                              quel versement

/// Le triplet a declarer pour un versement du jour `le` de la paie du mois `mois`, ou `None` : pas de taux DGFiP
/// applicable, c'est la grille. Un taux perime compte pour rien.
/// UN CALCUL DEPEND DU TAUX DE SON SALARIE A SON JOUR : il le note (`pas <cle> <jour> <mois>`, `empreinte`), et un taux
/// recu qui le change le fait recalculer.
pub fn taux_dgfip(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Option<Triplet>> {
    let t = triplet(ctx, cle, le, mois)?;
    if let Some(j) = le {
        socle::fichiers::noter_valeur(&cle_d_empreinte(cle, j, mois), empreinte_du_triplet(&t));
    }
    Ok(t)
}

/// La cle sous laquelle `taux_dgfip` note le taux du salarie `cle` au jour `le` pour la paie de `mois`.
fn cle_d_empreinte(cle: &str, le: Jour, mois: Option<&str>) -> String {
    match mois {
        Some(m) => format!("pas {} {} {}", cle, le.iso(), m),
        None => format!("pas {} {}", cle, le.iso()),
    }
}

fn triplet(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Option<Triplet>> {
    let s = situation(ctx, cle, le, mois)?;
    if s.situation != VALIDE {
        return Ok(None);
    }
    Ok(Some(Triplet { taux: ecrit(&s.taux).map(Montant::Texte), type_taux: ecrit(&s.type_taux), identifiant: ecrit(&s.identifiant) }))
}

fn empreinte_du_triplet(t: &Option<Triplet>) -> String {
    match t {
        Some(t) => socle::ecriture::json_en_ligne(&(&t.taux, &t.type_taux, &t.identifiant)),
        None => "aucun".into(),
    }
}

/// Ce que `taux_dgfip` a note sous `cle_notee` (`pas <cle> <jour> [<mois>]`), relu maintenant : l'empreinte tient tant
/// qu'il est le meme. `None` : la cle n'est pas une cle du PAS (`pas <cle> <jour>`).
pub fn empreinte(ctx: &Contexte, cle_notee: &str) -> Resultat<Option<String>> {
    let Some((cle, jour, mois)) = cle_notee.strip_prefix("pas ").and_then(|r| {
        let mut parts = r.split(' ');
        Some((parts.next()?, parts.next()?, parts.next()))
    }) else {
        return Ok(None);
    };
    Ok(Some(empreinte_du_triplet(&triplet(ctx, cle, Some(Jour::lire(jour)?), mois)?)))
}

/// La fiche d'un salarie sans ses taux recus : ce qu'une paie lit de la fiche hors du taux, qu'elle lit au jour de son
/// versement (`taux_dgfip`). Un taux recu ne fait recalculer que la paie dont il change le taux.
pub fn sans_taux_recus(fiche: &crate::fiche::ChampsSalarie) -> crate::fiche::ChampsSalarie {
    crate::fiche::ChampsSalarie { taux_pas: None, ..fiche.clone() }
}

/// Le prelevement d'UN versement. Un triplet s'applique tel quel, sur la
/// remuneration nette fiscale entiere, sans abattement.
pub fn du_versement(
    taux: Option<&Triplet>,
    bareme: &Pile,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> R<Versement> {
    let Some(t) = taux else {
        return rubriques_par_defaut(bareme, remuneration_nette_fiscale, le, contrat_court, zone);
    };
    // UNE REMUNERATION NETTE FISCALE NEGATIVE N'EST PAS UNE ASSIETTE NEGATIVE. Fiche DSN 1810 « Remuneration nette fiscale
    // negative » (`atelier/reference/sources/suspension-non-remuneree/net-entreprises-fiche1810-rnf-negative-pas-20250322.txt`) :
    // « La rubrique « Montant soumis au PAS - S21.G00.50.013 » ne peut pas etre renseignee avec une valeur negative. Il
    // convient alors de renseigner [...] a 0.00 » (l'exemple : 50.002 = -180.00, 50.009 = 0.00, 50.013 = 0.00) ; le cahier
    // technique refuse le signe moins en 50.009 et 50.013 et l'accepte en 50.002 (`norme-dsn/texte/bloc-S21.G00.50.txt`).
    // La grille (`rubriques_par_defaut`) plancher deja a zero ; le taux DGFiP aussi. Le cas : un mois sans salaire ou la
    // part patronale de sante reste due et se reintegre (CGI 83, 1° quater, meme dossier, `legi-extrait-code-general-des-
    // impots-20261015.md`).
    let soumis = plancher(eur(remuneration_nette_fiscale), Decimal::new(0, 2));
    Ok(Versement {
        taux: t.taux.clone(),
        type_taux: t.type_taux.clone(),
        identifiant: t.identifiant.clone(),
        abattement: Decimal::new(0, 2),
        montant_soumis: soumis,
        prelevement: pct(soumis, taux_du_versement(t.taux.as_ref())?),
    })
}

// ------------------------------------------------------------- diagnostic

pub const VALIDE: &str = "valide";
pub const PERIME: &str = "perime";
pub const BAREME: &str = "bareme";
pub const INCONNU: &str = "inconnu";
pub const SANS_NIR: &str = "sans_nir";

fn libelle(situation: &str) -> &'static str {
    match situation {
        VALIDE => "taux DGFiP en cours de validite",
        PERIME => "taux DGFiP perime -- ce n'est plus le taux du salarie",
        BAREME => "aucun taux personnalise : c'est le bareme qui s'applique",
        INCONNU => "aucun taux connu",
        _ => "pas de NIR : la DGFiP n'a personne a qui repondre",
    }
}

/// OU EN EST LE TAUX D'UN SALARIE : sa situation (`VALIDE`, `PERIME`...), le taux en vigueur tel qu'il est ecrit, et
/// ce qu'il faudrait demander.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Situation {
    pub salarie: String,
    pub situation: &'static str,
    pub libelle: &'static str,
    pub taux: Option<String>,
    #[serde(rename = "type")]
    pub type_taux: Option<String>,
    pub identifiant: Option<String>,
    pub valable_jusqu_a: Option<Jour>,
    pub a_demander: bool,
    pub dernier_recu: Option<String>,
}

/// Ou en est le taux d'un salarie au jour `le`, pour la paie du mois `mois` (`en_vigueur`). Ne refuse que si sa fiche est
/// illisible.
pub fn situation(ctx: &Contexte, cle: &str, le: Option<Jour>, mois: Option<&str>) -> Resultat<Situation> {
    let le = le_jour(ctx, le);
    let recus = recus(ctx, cle)?;
    let e = en_vigueur(&recus, le, mois);
    let quoi = match e {
        None if sans_nir(ctx, cle) => SANS_NIR,
        None => INCONNU,
        Some(e) if e.r#type.portee() != Some("01") => {
            if sans_nir(ctx, cle) {
                SANS_NIR
            } else {
                BAREME
            }
        }
        Some(e) => {
            if est_perime(e, le) {
                PERIME
            } else {
                VALIDE
            }
        }
    };
    // Sans taux, ses cases ne disent rien.
    let champ = |c: fn(&TauxPas) -> &Option<String>| e.and_then(|e| c(e).clone());
    Ok(Situation {
        salarie: cle.to_string(),
        situation: quoi,
        libelle: libelle(quoi),
        taux: champ(|e| &e.taux),
        type_taux: champ(|e| &e.r#type),
        identifiant: champ(|e| &e.identifiant),
        valable_jusqu_a: e.and_then(fin_de_validite),
        a_demander: matches!(quoi, PERIME | BAREME | INCONNU),
        dernier_recu: recus.last().and_then(|r| r.mis_a_disposition.clone()),
    })
}

fn archive(ctx: &Contexte, cle: &str) -> bool {
    ctx.salaries.get(cle).is_some_and(|f| f.archivee())
}

/// Le meme tableau pour plusieurs salaries, dans l'ordre alphabetique. Sans liste : ceux des fiches, moins les archives.
pub fn diagnostic(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>, mois: Option<&str>) -> Resultat<Vec<Situation>> {
    let cles: BTreeSet<String> = match cles {
        Some(c) => c.iter().cloned().collect(),
        None => ctx.salaries.keys().filter(|c| !archive(ctx, c)).map(|c| c.to_string()).collect(),
    };
    cles.iter().map(|c| situation(ctx, c, le, mois)).collect()
}

/// Les phrases a dire au depot de la DSN et a la production des bulletins (`depart::sans_taux_recu`), une par salarie
/// dont la paie prendrait la grille de l'article 204 H, III (CGI, art. 1671, 2 : « A défaut de taux transmis par
/// l'administration ») : ce que sa fiche sait des taux recus. Vide si tout va bien.
pub fn avertissements(ctx: &Contexte, cles: Option<&[String]>, le: Option<Jour>, mois: Option<&str>) -> Resultat<Vec<String>> {
    let mut lignes = Vec::new();
    for s in diagnostic(ctx, cles, le, mois)? {
        let mut ligne = format!("{} : ", ctx.nomme(&s.salarie));
        ligne += &match s.situation {
            VALIDE => continue,
            PERIME => format!(
                "le dernier taux transmis par la DGFiP ({} %) valait jusqu'au {}",
                match s.taux.portee() {
                    Some(t) => t.replace('.', ","),
                    None => "?".to_string(),
                },
                match s.valable_jusqu_a {
                    Some(j) => j.jj_mm_aaaa(),
                    None => "?".to_string(),
                }
            ),
            SANS_NIR => "sans NIR dans sa fiche, la DGFiP ne peut transmettre aucun taux".into(),
            _ => "aucun taux transmis par la DGFiP".into(),
        };
        lignes.push(ligne);
    }
    Ok(lignes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un taux recu, ecrit en texte et lu par la porte du socle.
    fn lu(texte: &str) -> TauxPas {
        socle::forme::lire_texte(texte, "le taux").unwrap()
    }

    fn recu(taux: &str, id: &str, le: &str, retour: Option<&str>) -> TauxPas {
        let retour = match retour {
            Some(m) => format!(r#", "{}": "{}""#, EN_RETOUR_DE, m),
            None => String::new(),
        };
        lu(&format!(r#"{{"taux": "{}", "type": "01", "identifiant": "{}", "mis_a_disposition": "{}"{}}}"#, taux, id, le, retour))
    }

    /// LA DATE D'EFFET D'UN TAUX TRANSMIS. Le debiteur « applique le taux calculé par l'administration fiscale, au plus
    /// tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/code-general-des-
    /// impots/articles-20261021.md`, l. 135) ; la retenue « est effectuée par le débiteur lors du paiement » (meme
    /// article, 1, l. 132) ; le taux est « applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition »
    /// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`, l. 23-24). Un
    /// taux vaut donc pour tout paiement fait a partir du jour de sa mise a disposition, et pour aucun avant : mis a
    /// disposition le 13/03, il vaut pour le salaire paye le 14/03 ; celui du 15/04 ne vaut pas pour lui, il vaut pour
    /// celui paye le 15/04. Sans taux recu avant le paiement : aucun, la grille s'applique.
    #[test]
    fn un_taux_vaut_pour_tout_paiement_a_partir_de_sa_mise_a_disposition() {
        let taux = |id: &str, le: &str| recu("9.40", id, le, None);
        let recus = vec![taux("480747943", "20260313070000"), taux("985249073", "20260415100000")];
        let id = |le: &str| en_vigueur(&recus, Jour::lire(le).unwrap(), None).and_then(|e| e.identifiant.en_texte());
        assert_eq!(id("2026-03-12"), None);
        assert_eq!(id("2026-03-13"), Some("480747943".to_string()));
        assert_eq!(id("2026-03-14"), Some("480747943".to_string()));
        assert_eq!(id("2026-04-14"), Some("480747943".to_string()));
        assert_eq!(id("2026-04-15"), Some("985249073".to_string()));
        assert_eq!(id("2024-12-31"), None);
    }

    /// BOI-IR-PAS-30-10-20, § 70, exemple 1 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) :
    /// un taux mis a disposition le 10 fevrier « est valide jusqu'au 30 avril ».
    #[test]
    fn un_taux_vaut_jusqu_a_la_fin_du_deuxieme_mois_qui_suit_sa_mise_a_disposition() {
        let e = recu("5.00", "1", "20260210120000", None);
        assert_eq!(fin_de_validite(&e), Jour::nouveau(2026, 4, 30));
        assert!(!est_perime(&e, Jour::nouveau(2026, 4, 30).unwrap()));
        assert!(est_perime(&e, Jour::nouveau(2026, 5, 1).unwrap()));
    }

    /// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT (BOI-IR-PAS-30-10-20, § 20 et § 70, exemple 1 :
    /// le compte rendu de la DSN de janvier sert aux revenus de « février, mars et avril », `atelier/reference/sources/
    /// bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`). La DSN de decembre, deposee le 21/12 pour un versement du 31/12 au
    /// taux du compte rendu du 05/10 (valide jusqu'au 31/12) : son compte rendu du 21/12 ne vaut pas pour la paie de
    /// decembre, il vaut pour celle de janvier, et s'ecrit dans la fiche a partir du 01/01. La reponse a une demande de
    /// taux (sans DSN mensuelle) vaut des sa mise a disposition.
    #[test]
    fn le_taux_en_retour_de_la_dsn_d_un_mois_vaut_pour_les_mois_qui_suivent() {
        let taux = |id: &str, le: &str, retour: Option<&str>| recu("2.30", id, le, retour);
        let recus = vec![taux("157352475", "20261005194311", Some("2026-09")), taux("713288315", "20261221184328", Some("2026-12"))];
        let id = |le: &str, mois: &str| en_vigueur(&recus, Jour::lire(le).unwrap(), Some(mois)).and_then(|e| e.identifiant.en_texte());
        assert_eq!(id("2026-12-31", "2026-12"), Some("157352475".to_string()));
        assert_eq!(id("2027-01-05", "2026-12"), Some("157352475".to_string()));
        assert_eq!(id("2027-01-31", "2027-01"), Some("713288315".to_string()));
        let demande = vec![taux("480747943", "20261221184328", None)];
        assert!(en_vigueur(&demande, Jour::lire("2026-12-31").unwrap(), Some("2026-12")).is_some());
        let mad = Jour::lire("2026-12-21").unwrap();
        assert_eq!(date_d_effet(mad, Some("2026-12")).unwrap(), Jour::lire("2027-01-01").unwrap());
        assert_eq!(date_d_effet(mad, Some("2026-11")).unwrap(), mad);
        assert_eq!(date_d_effet(mad, None).unwrap(), mad);
        assert!(date_d_effet(mad, Some("décembre")).is_err());
    }
}
