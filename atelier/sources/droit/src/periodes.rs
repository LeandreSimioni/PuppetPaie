//! La paie d'un mois decoupee en periodes elementaires : un permanent dont le statut, la categorie ou la quotite
//! change en cours de mois, ou dont un avenant prend effet en cours de mois.
//!
//! Textes (`atelier/reference/`) :
//! - norme DSN 2026.1, § 2.1.3.2, p. 28 (`norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, l. 500-511) : « La
//!   periode elementaire de paie constitue une sous-partie de la periode de paie. Le decoupage [...] decoule de
//!   modalites differentes de calcul de la paie en cas d'evolution des parametres de calcul de la paie et notamment du
//!   statut du salarie (par exemple, passage de non cadre a cadre en cours de periode de paie) » ; « La periode a
//!   declarer dans le bloc Remuneration est cette periode elementaire de paie, lorsque le logiciel de paie produit
//!   effectivement un decoupage » ; § 2.1.3.3 et 2.1.3.5, p. 29 : les elements du bloc 51 et les mesures d'activite
//!   (53) « par periode elementaire de paie » ;
//! - FAQ Agirc-Arrco « Changement de situation en cours de mois », v2 (`sources/agirc-arrco/`) : « le salarie concerne
//!   par ce changement doit faire l'objet d'un decoupage de ses bases assujetties declarees en DSN » ; « Le salaire brut
//!   et le plafond de securite sociale doivent etre proratises selon la duree de chacune des periodes
//!   infra-mensuelles » (exemple 1 : non cadre -> cadre au 10 avril, 9/30 et 21/30) ; pour un passage a temps partiel,
//!   le plafond et le salaire de la periode « au prorata temporis de la duree du travail » (exemple 2 : 15/30 x
//!   70/151,67) ; les bases 02 et 03 datees de chaque periode (78.002, 78.003) ; cahier Agirc-Arrco CT2026, § 6.1 ;
//! - fiche net-entreprises 1265 (`sources/net-entreprises-base-connaissances/`, l. 52) : « Pour tout changement
//!   intervenant en cours de mois, il convient de ventiler les bases assujetties par periodes afin de permettre un
//!   calcul correspondant a la situation exacte du salarie » (tous les organismes, Urssaf compris) ;
//! - CSS R. 242-2, I (`sources/code-de-la-securite-sociale/articles-20261010.md`) : le plafond « reduit[s] a due
//!   proportion du nombre de jours de la periode pendant laquelle les personnes sont employees », et pour un temps
//!   partiel « a due proportion de la duree de travail inscrite a leur contrat de travail au titre de la periode ou ils
//!   sont presents » ; BOSS, assiette generale, § 730 (`sources/boss/`) : « les differentes regles d'ajustement du
//!   plafond sont cumulables ».
//!
//! Le droit dit ou la paie se coupe (`decouper`, `jour_de_l_avenant`) et quelle part du mois chaque periode porte
//! (`fraction_du_mois`) ; l'engagement calcule chaque periode et les assemble (`engagement::periodes`).

use serde_json::Value;

use socle::date::Jour;
use socle::num::{div, Decimal, Resultat};

/// Ce que `avant` (les termes du contrat avant un avenant en cours de mois) peut dire.
pub const TERMES_D_AVANT: [&str; 2] = ["salaire_mensuel", "quotite_contrat"];

/// Le jour ou un avenant decoupe la paie de la periode [`debut`, `fin`] : sa `date_modification`, quand le contrat dit
/// les termes d'avant (`avant`) et qu'elle tombe apres le premier jour de la periode payee. Sans `avant`, rien ; un
/// `avant` incomplet, sans date, ou qui ne decoupe rien refuse. `qui` nomme le contrat dans un refus.
pub fn jour_de_l_avenant(avant: Option<&Value>, date_modification: Option<&str>, debut: Jour, fin: Jour, qui: &str) -> Resultat<Option<Jour>> {
    let Some(avant) = avant else { return Ok(None) };
    let objet = avant.as_object().ok_or_else(|| format!("{} : `avant` (les termes du contrat avant l'avenant) : un objet est attendu, lu {}.", qui, avant))?;
    if let Some(k) = objet.keys().find(|k| !TERMES_D_AVANT.contains(&k.as_str())) {
        return Err(format!(
            "{} : `avant` dit `{}`, que le decoupage de la paie ne sait pas porter. Il dit les termes du contrat avant un \
avenant en cours de mois : {} ; un autre changement en cours de mois (nature, motif, duree de reference...) n'a pas de \
periode elementaire dans le logiciel. Refus.",
            qui,
            k,
            TERMES_D_AVANT.join(", ")
        ));
    }
    if let Some(k) = TERMES_D_AVANT.iter().find(|k| objet.get(**k).map_or(true, Value::is_null)) {
        return Err(format!(
            "{} : `avant` ne dit pas `{}`. La periode d'avant l'avenant se paie sur les termes du contrat d'alors, le \
salaire mensuel ET la duree du travail (S21.G00.40.013) ; le logiciel ne les devine pas. Les dire tous les deux.",
            qui, k
        ));
    }
    let Some(texte) = date_modification else {
        return Err(format!(
            "{} : `avant` dit les termes du contrat avant un avenant, et `date_modification` (« Modifier l'engagement », « A partir de ») \
ne dit pas quand il prend effet. Le dater.",
            qui
        ));
    };
    let j = Jour::lire(texte).map_err(|_| format!("{} : `date_modification` illisible : {}.", qui, texte))?;
    if j <= debut || j > fin {
        return Err(format!(
            "{} : `avant` dit les termes du contrat avant un avenant qui prend effet le {}, et la periode payee va du {} \
au {} : il ne la coupe pas, rien n'est a decouper. Retirer `avant` (un avenant au premier jour de la periode se paie sur \
les termes de la saisie, tout le mois).",
            qui,
            j.jj_mm_aaaa(),
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        ));
    }
    Ok(Some(j))
}

/// Les periodes elementaires de [`debut`, `fin`] coupee aux `jours` ou quelque chose change (chacun est le premier jour
/// d'une periode) ; un jour hors de la periode ne coupe rien.
pub fn decouper(debut: Jour, fin: Jour, mut jours: Vec<Jour>) -> Resultat<Vec<(Jour, Jour)>> {
    jours.retain(|j| *j > debut && *j <= fin);
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    let mut d = debut;
    for j in jours {
        out.push((d, j.plus_jours(-1)?));
        d = j;
    }
    out.push((d, fin));
    Ok(out)
}

/// La regle qui paie un mois incomplet d'un contrat paye au mois (`part_du_mois`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegleDuMoisIncomplet {
    /// Le salaire mensuel au quotient des heures du mois dans l'entreprise, par les heures de la periode.
    HoraireReel,
    /// CCN 1285, art. X.3 : 1/21 du salaire mensuel par jour ouvre du contrat (artiste, premier ou dernier mois).
    Ccneac1285Artiste,
}

/// La regle d'un mois incomplet : celle de la convention qui en a une, sinon l'horaire reel. La CCNEAC (IDCC 1285),
/// art. X.3 (`atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 20), pour les emplois de la
/// filiere artistique : « Si le premier et/ou le dernier mois civil est incomplet, le salaire de celui-ci est calcule en
/// multipliant 1/21 du montant du salaire mensuel inscrit au contrat [...] par le nombre de jours ouvres figurant au
/// contrat ». `contrat_incomplet` : le contrat commence apres le premier jour du mois ou finit avant le dernier (un
/// avenant qui coupe un mois entier n'en fait pas un premier ni un dernier mois).
pub fn regle_du_mois_incomplet(idcc: &str, statut: &str, contrat_incomplet: bool) -> RegleDuMoisIncomplet {
    if idcc == "1285" && statut == "artiste" && contrat_incomplet {
        RegleDuMoisIncomplet::Ccneac1285Artiste
    } else {
        RegleDuMoisIncomplet::HoraireReel
    }
}

/// La part du mois qu'une periode payee porte de son salaire mensuel (`part_du_mois`).
#[derive(Debug, Clone, PartialEq)]
pub struct PartDuMois {
    /// La fraction du salaire mensuel payee : 1 pour un mois entier.
    pub fraction: Decimal,
    /// Les jours ouvres de la periode (`conges::jours_ouvres`).
    pub jours: i64,
    /// Ceux du mois entier (horaire reel), ou 21 (CCN 1285).
    pub jours_du_mois: i64,
    pub regle: RegleDuMoisIncomplet,
}

impl PartDuMois {
    /// Ce que la ligne du bulletin dit de la part payee, pour un contrat de `heures_par_jour` heures par jour ouvre ;
    /// `None` pour un mois entier.
    pub fn sur_le_bulletin(&self, heures_par_jour: Decimal) -> Option<String> {
        if self.fraction == Decimal::ONE {
            return None;
        }
        Some(match self.regle {
            RegleDuMoisIncomplet::HoraireReel => format!(
                "{} h sur les {} h du mois",
                heures_en_mots(heures_par_jour, self.jours),
                heures_en_mots(heures_par_jour, self.jours_du_mois)
            ),
            RegleDuMoisIncomplet::Ccneac1285Artiste => format!("{} jour(s) ouvré(s) à 1/21 du mois", self.jours),
        })
    }

    /// La methode, en mots, et son texte : ce que l'apercu du bulletin dit d'un mois incomplet.
    pub fn methode(&self, heures_par_jour: Decimal) -> String {
        match self.regle {
            RegleDuMoisIncomplet::HoraireReel => format!(
                "{} h de travail sur les {} h que l'horaire de l'entreprise compte dans le mois ({} jours ouvrés de {} h, \
du lundi au vendredi, jours fériés compris), payées au salaire mensuel divisé par les heures du mois (Cass. soc., \
20/01/1999, n° 96-45.042 : « la rémunération horaire » multipliée par « le nombre d'heures de travail réellement \
effectuées »). La fiche ne dit pas les jours travaillés : l'horaire est compté du lundi au vendredi",
                heures_en_mots(heures_par_jour, self.jours),
                heures_en_mots(heures_par_jour, self.jours_du_mois),
                self.jours_du_mois,
                socle::num::nombre(heures_par_jour).replace('.', ",")
            ),
            RegleDuMoisIncomplet::Ccneac1285Artiste => format!(
                "{} jour(s) ouvré(s) du contrat dans le mois, payés chacun 1/21 du salaire mensuel (CCN 1285, art. X.3 : \
le premier ou le dernier mois incomplet d'un artiste)",
                self.jours
            ),
        }
    }
}

/// `jours` jours de `heures_par_jour` heures, au centieme, ecrits a la francaise.
fn heures_en_mots(heures_par_jour: Decimal, jours: i64) -> String {
    socle::num::nombre(socle::num::eur2(socle::num::mul(heures_par_jour, Decimal::from(jours)))).replace('.', ",")
}

/// LA PART DU SALAIRE MENSUEL que porte la periode payee [`d`, `f`] du mois qui finit le `fin_mois`.
///
/// L'HORAIRE REEL : « la retenue par heure d'absence d'un salarie paye au mois doit etre en principe egale au quotient du
/// salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le mois considere », et le salaire d'un mois
/// incomplet se determine « en multipliant la remuneration horaire par le nombre d'heures de travail reellement
/// effectuees » : un salaire au 25/30e est casse (Cass. soc., 20/01/1999, n° 96-45.042, `atelier/reference/sources/
/// mois-incomplet/legifrance-cass-soc-19990120-96-45042-20261030.md` ; meme quotient : Cass. soc., 11/02/1982,
/// n° 80-40.359, et 14/05/1987, n° 84-43.854, une embauche au 9 fevrier payee 117 h sur les 156 h du mois,
/// `../suspension-non-remuneree/`). Les heures du mois dans l'entreprise et celles de la periode sont les jours ouvres
/// (`conges::jours_ouvres` : du lundi au vendredi, feries compris, fiche DSN 2693, exemple 4) fois les heures d'un jour,
/// les memes pour les deux : la part est le quotient des jours ouvres. INFERENCE NOMMEE : la saisie ne dit pas les jours
/// de la semaine travailles, et l'horaire est pris du lundi au vendredi (l'hypothese de `conges::jours_oeuvres`, fiche DSN
/// 2640).
///
/// LA CCN 1285, pour un artiste (`regle_du_mois_incomplet`) : les jours ouvres de la periode, a 1/21 chacun. INFERENCE
/// NOMMEE : « les jours ouvres figurant au contrat » se comptent du lundi au vendredi, feries compris, comme ci-dessus.
///
/// Une periode qui couvre le mois entier en porte tout le salaire. Le plafond de la securite sociale, lui, se proratise
/// aux jours calendaires (CSS R. 242-2, `plafond::prorata_calendaire`) : ce n'est pas le salaire.
pub fn part_du_mois(d: &Jour, f: &Jour, fin_mois: &Jour, regle: RegleDuMoisIncomplet) -> Resultat<PartDuMois> {
    let premier = Jour::nouveau(fin_mois.an, fin_mois.mois, 1).ok_or("part_du_mois : premier jour du mois")?;
    let jours = crate::conges::jours_ouvres(d, f)?;
    let du_mois = crate::conges::jours_ouvres(&premier, fin_mois)?;
    let entier = *d <= premier && *f >= *fin_mois;
    let (jours_du_mois, fraction) = match regle {
        _ if entier => (du_mois, Decimal::ONE),
        RegleDuMoisIncomplet::HoraireReel => (du_mois, div(Decimal::from(jours), Decimal::from(du_mois))),
        RegleDuMoisIncomplet::Ccneac1285Artiste => (21, div(Decimal::from(jours), Decimal::from(21))),
    };
    Ok(PartDuMois { fraction, jours, jours_du_mois, regle })
}
