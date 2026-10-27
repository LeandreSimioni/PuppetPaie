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
            "{} : `avant` dit les termes du contrat avant un avenant, et `date_modification` (« Avenant a compter du ») \
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

/// La part du mois que porte la periode [`d`, `f`] : ses jours sur ceux du mois (FAQ Agirc-Arrco, exemples 1 et 2 ;
/// CSS R. 242-2, I).
pub fn fraction_du_mois(d: &Jour, f: &Jour, fin_mois: &Jour) -> Decimal {
    div(Decimal::from(f.moins(d) + 1), Decimal::from(fin_mois.jours_du_mois()))
}
