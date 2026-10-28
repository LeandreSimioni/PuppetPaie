//! Le bloc « Changements contrat - S21.G00.41 » d'une DSN mensuelle : ce qui a change d'un contrat deja declare.
//!
//! Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/`) :
//! - le bloc porte l'ANCIENNE valeur, « Les rubriques doivent etre renseignees avec la valeur avant changement »
//!   (`bloc-S21.G00.41.txt`, p. 215), datee en 41.001, « date de survenance du changement » (p. 216) ;
//! - « Le changement d'une des modalites visees doit imperativement etre declare via un bloc « Changements Contrat -
//!   S21.G00.41 » », un bloc par modalite, « meme si plusieurs changements sont concomitants », dans « la DSN du mois
//!   ou le service prend connaissance de ces modifications » (`chapitre-2-principes-de-constitution.txt`, § 2.1.2.5.2,
//!   p. 25) ; CCH-13 et § 4.4.14.2 a (`chapitre-4-controles.txt`, p. 103) ;
//! - jamais un identifiant et une modalite dans le meme bloc (CCH-16, principe b) ; la profondeur de recalcul 41.028
//!   sur le bloc le plus ancien d'une modalite, au premier jour d'un mois civil ou a la date de debut du contrat
//!   (principe c, CCH-11 a 15 de 41.028, p. 226 ; § 2.1.2.5.2, p. 26), jamais pour un identifiant (principe d) ;
//! - un identifiant (date de debut 41.021, SIRET 41.012) se change « obligatoirement des la premiere DSN mensuelle dans
//!   laquelle est utilisee la nouvelle identite » (§ 2.1.2.3.1, p. 20) ; sa date « peut correspondre a toute date »,
//!   sauf celle du SIRET, « date d'effet du changement d'employeur » (p. 21) ;
//! - ce qui n'a pas de rubrique en bloc 41 (libelle de l'emploi, date de fin, taux AT) se change en reecrivant le bloc
//!   40 (§ 2.1.2.4, p. 21-22) : rien a faire ici.
//!
//! CE QUI A ETE DECLARE SE LIT, IL NE SE RECALCULE PAS (regle 4) : l'ancienne valeur est celle du bloc 40 du meme contrat
//! dans la DSN du mois le plus recent qui est partie pour lui, ecrit de la paie de ce mois telle que la version de
//! l'engagement sur laquelle elle est partie l'a ecrite (`dsn::bloc_40_declare`, `engagement::etats::paie_declaree`).
//! Un mois produit et pas encore depose n'est pas un declare.
//!
//! LA DATE SE LIT AUSSI, elle ne se pose jamais au 1er du mois par defaut : le lendemain de la fenetre de l'historique
//! de la fiche (salarie ou entreprise) qui a change le champ, la `date_modification` de la saisie du mois (un avenant),
//! le 1er janvier d'un bareme annuel (le code risque AT), ou le premier jour de la periode payee pour ce que le moteur
//! calcule periode par periode (les emplois multiples, 40.036). Un changement sans date refuse ; un changement anterieur
//! au mois (des mois deja declares avec l'ancienne valeur, que la paie n'a pas recalcules) refuse ; un changement en
//! cours de mois refuse sauf pour ce qui ne change pas la paie (statut BOETH), pour les identifiants, et pour ce que la
//! paie decoupee en periodes elementaires porte au jour du changement (lot R10, `periodes`).



use serde_json::Value;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::date::{dernier_du_mois, premier_du_mois, Jour};
use socle::num::Resultat;

/// Une modalite du contrat qui a sa rubrique en bloc 41 : d'ou son changement tient sa date.
struct Modalite {
    r40: &'static str,
    r41: &'static str,
    /// La saisie du mois la porte (`date_modification` de l'engagement la date).
    saisie: bool,
    /// Les champs de la fiche salarie dont l'historique la date.
    fiche: &'static [&'static str],
    /// Les champs de la fiche entreprise dont l'historique la date.
    entreprise: &'static [&'static str],
    /// Elle vient du bareme de l'annee : elle change au 1er janvier.
    annuelle: bool,
    /// Elle se lit periode de paie par periode de paie (le moteur la calcule chaque mois) : elle change au premier
    /// jour de la periode payee.
    periodique: bool,
    /// Elle peut changer en cours de mois : elle ne change pas la paie.
    en_cours_de_mois: bool,
    /// Elle peut changer en cours de mois quand la paie se decoupe au jour du changement (lot R10, `periodes`) : le
    /// statut, la categorie, la PCS et le code emploi que la fiche date, la duree du travail que l'avenant date avec ses
    /// termes d'avant (`avant`), et la proratisation du plafond qui la suit.
    decoupable: bool,
}

const fn m(r40: &'static str, r41: &'static str, saisie: bool, fiche: &'static [&'static str], entreprise: &'static [&'static str]) -> Modalite {
    Modalite { r40, r41, saisie, fiche, entreprise, annuelle: false, periodique: false, en_cours_de_mois: false, decoupable: false }
}

/// `m`, une modalite que le decoupage de la paie porte en cours de mois.
const fn d(r40: &'static str, r41: &'static str, saisie: bool, fiche: &'static [&'static str], entreprise: &'static [&'static str]) -> Modalite {
    Modalite { decoupable: true, ..m(r40, r41, saisie, fiche, entreprise) }
}

/// Les modalites que le modele fait changer (fiche N8, « Les rubriques 41.0xx, une par une »), chacune avec sa jumelle
/// du bloc 40 (la liste des types, § 4.4.14.1, p. 101-103). Le reste du bloc 41 (fonction publique, regimes speciaux,
/// dispositifs absents du modele, rubriques 40 non ecrites ou fixees par la nature) n'a pas de jumelle qui bouge : rien
/// ne peut en changer.
const MODALITES: [Modalite; 20] = [
    d("S21.G00.40.002", "S21.G00.41.002", true, &["statut_conventionnel", "statut_categoriel_rc", "code_emploi_ft"], &[]),
    d("S21.G00.40.003", "S21.G00.41.003", true, &["statut_categoriel_rc", "statut_conventionnel", "code_emploi_ft"], &[]),
    d("S21.G00.40.004", "S21.G00.41.019", true, &["pcs_ese", "code_emploi_ft"], &[]),
    d("S21.G00.40.005", "S21.G00.41.020", true, &["code_emploi_ft"], &[]),
    m("S21.G00.40.007", "S21.G00.41.004", true, &[], &[]),
    m("S21.G00.40.008", "S21.G00.41.005", true, &[], &[]),
    m("S21.G00.40.011", "S21.G00.41.006", true, &[], &[]),
    m("S21.G00.40.012", "S21.G00.41.022", true, &[], &["quotite_reference"]),
    d("S21.G00.40.013", "S21.G00.41.007", true, &[], &[]),
    d("S21.G00.40.014", "S21.G00.41.008", true, &[], &[]),
    m("S21.G00.40.016", "S21.G00.41.010", false, &["etablissement"], &["code_postal", "etablissements"]),
    m("S21.G00.40.017", "S21.G00.41.011", false, &[], &["idcc"]),
    // Le lieu de travail est le SIRET de l'etablissement du salarie (`dsn::bloc_contrat`) : il change avec lui, a la
    // date de la mutation (le SIRET, identifiant, va en 41.012 dans un autre bloc, CCH-16).
    m("S21.G00.40.019", "S21.G00.41.013", false, &["etablissement"], &[]),
    m("S21.G00.40.021", "S21.G00.41.016", true, &[], &[]),
    m("S21.G00.40.022", "S21.G00.41.023", false, &[], &["congesspectacles_adherent"]),
    // Le taux de la deduction forfaitaire specifique vient du bareme de l'annee (arrete du 4 septembre 2025, art. 9, II :
    // reduit « au 1er janvier de chaque annee ») et de la fiche (profession, accord du salarie, option de l'employeur).
    Modalite {
        annuelle: true,
        ..m("S21.G00.40.023", "S21.G00.41.017", false, &["deduction_forfaitaire", "deduction_forfaitaire_accord"], &["deduction_forfaitaire_option"])
    },
    // Le code risque vient du bareme de l'annee, celui de l'etablissement du salarie (`bareme::surcharger`) : il change
    // au 1er janvier, ou avec une mutation.
    Modalite { annuelle: true, ..m("S21.G00.40.040", "S21.G00.41.024", false, &["etablissement"], &[]) },
    // « emplois multiples », « au cours d'une meme periode de paie » (p. 199) : `dsn::emplois_multiples` le calcule
    // chaque mois ; 41.061 est une modalite (§ 4.4.14.1, p. 103).
    Modalite { periodique: true, ..m("S21.G00.40.036", "S21.G00.41.061", false, &[], &[]) },
    Modalite { en_cours_de_mois: true, ..m("S21.G00.40.072", "S21.G00.41.048", false, &["statut_boeth"], &[]) },
    // Norme 2027 (P27V01) : « Ancienne application de la proratisation du Plafond de Securite Sociale a hauteur de la
    // quotite de travail » (cahier 2027.1.1, p. 242, `texte-2027/bloc-S21.G00.41.txt` l. 1439-1450 ; modalite, § 4.4.14.1,
    // `chapitre-4-controles.txt` l. 1047). 40.084 change avec le temps partiel (un avenant, date par la saisie), avec
    // la renonciation de l'etablissement (la fiche entreprise) ou avec le statut d'artiste, que la loi exclut de la
    // proratisation (`calcul::plafond_temps_partiel`). Absente des DSN de 2026 : rien de declare, pas de bloc 41.
    d("S21.G00.40.084", "S21.G00.41.066", true, &["statut"], &["plafond_temps_partiel"]),
];

/// Les rubriques que `dsn::bloc_contrat` ecrit telles que la nature les donne, quand elle en donne une.
const FIXEES_PAR_LA_NATURE: [&str; 5] = ["S21.G00.40.005", "S21.G00.40.007", "S21.G00.40.008", "S21.G00.40.014", "S21.G00.40.021"];

/// Le bloc 40 d'un contrat tel qu'une DSN partie l'a declare (`dsn::bloc_40_declare`).
pub struct Declare {
    /// Le mois de cette DSN (AAAA-MM).
    pub mois: String,
    /// Le NIC de l'etablissement qui l'a deposee, et son SIRET (06.001 + 11.001).
    pub nic: String,
    pub siret: String,
    pub bloc: Vec<(String, String)>,
}


fn valeur<'a>(bloc: &'a [(String, String)], r: &str) -> Option<&'a str> {
    bloc.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str())
}

fn mois_precedent(mois: &str) -> Resultat<String> {
    let veille = premier_du_mois(mois)?.plus_jours(-1)?;
    Ok(format!("{:04}-{:02}", veille.an, veille.mois))
}

fn ddmmaaaa(v: &str) -> Option<Jour> {
    let c: Vec<char> = v.chars().collect();
    if c.len() != 8 || !c.iter().all(char::is_ascii_digit) {
        return None;
    }
    let s = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    Jour::lire(&format!("{}-{}-{}", s(4, 8), s(2, 4), s(0, 2))).ok()
}

/// Les lendemains des fenetres de l'historique d'une fiche pour ces champs : les jours ou ils ont change.
fn changements_dates(fiche: &Value, champs: &[&str]) -> Resultat<Vec<(Jour, String)>> {
    let mut out = Vec::new();
    for e in fiche.get("historique").and_then(Value::as_array).into_iter().flatten() {
        let Some(c) = e.get("champ").and_then(Value::as_str).filter(|c| champs.contains(c)) else { continue };
        let Some(j) = e.get("jusqu_au").and_then(Value::as_str) else { continue };
        out.push((Jour::lire(j)?.plus_jours(1)?, c.to_string()));
    }
    Ok(out)
}

/// « Prenom NOM, contrat <numero> » : qui la phrase de refus designe.
fn qui(p: &Paie) -> String {
    let t = |k: &str| p.salarie.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let nom = [t("prenom"), t("nom")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
    format!(
        "{}, contrat {}",
        if nom.is_empty() { t("_cle") } else { nom },
        p.engagement.numero.clone().unwrap_or_else(|| "sans numero".into())
    )
}

fn libelle_mois(mois: &str) -> String {
    engagement::libelle_mois(mois)
}

/// La date de la modification (41.001) du changement de `md`, declaree `avant` dans la DSN de `decl.mois`. Refus
/// quand elle ne se lit pas, ou qu'elle ne tombe pas dans le mois declare (voir l'en-tete du module).
#[allow(clippy::too_many_arguments)]
fn date_du_changement(ctx: &Contexte, p: &Paie, md: &Modalite, decl: &Declare, mois: &str, avant: &str, apres: &str) -> Resultat<Jour> {
    let (debut_m, fin_b) = (premier_du_mois(mois)?, dernier_du_mois(&decl.mois)?);
    let fin_p = p.engagement.fin_periode()?;
    let mut candidats: Vec<(Jour, String)> = Vec::new();
    if md.saisie {
        if let Some(t) = &p.engagement.date_modification {
            let j = Jour::lire(t).map_err(|_| format!("{} : `date_modification` illisible dans la saisie de {} : {}.", qui(p), mois, t))?;
            candidats.push((j, "la date de modification de la saisie du mois (`date_modification`)".into()));
        }
    }
    for (j, c) in changements_dates(&p.salarie, md.fiche)? {
        candidats.push((j, format!("la fiche du salarie (`{}`)", c)));
    }
    for (j, c) in changements_dates(&ctx.entreprise, md.entreprise)? {
        candidats.push((j, format!("la fiche entreprise (`{}`)", c)));
    }
    if md.annuelle && decl.mois[..4] < mois[..4] {
        candidats.push((Jour { an: debut_m.an, mois: 1, jour: 1 }, "le bareme de l'annee (il vaut du 1er janvier)".into()));
    }
    if md.periodique {
        // INFERENCE : une valeur qui se lit periode de paie par periode de paie change au premier jour de la periode
        // ou elle differe. Elle n'a pu changer qu'au mois declare suivant : sinon, le mois intermediaire, non depose,
        // l'a peut-etre deja changee, et le jour ne se lit plus.
        if decl.mois != mois_precedent(mois)? {
            return Err(format!(
                "{} : {} passe de '{}' (declare en {}) a '{}'. Cette valeur se lit periode de paie par periode de paie : le changement date du premier jour de la periode ou il a eu lieu, et les mois entre {} et {}, non deposes, ne le disent pas. Deposer d'abord la DSN du mois precedent.",
                qui(p),
                md.r40,
                avant,
                libelle_mois(&decl.mois),
                apres,
                libelle_mois(&decl.mois),
                libelle_mois(mois)
            ));
        }
        candidats.push((p.engagement.debut_periode()?, "la periode de paie (la valeur se lit periode par periode, p. 199)".into()));
    }
    candidats.retain(|(j, _)| *j <= fin_p);
    let mut apres_declare: Vec<&(Jour, String)> = candidats.iter().filter(|(j, _)| *j > fin_b).collect();
    apres_declare.sort_by_key(|(j, _)| *j);
    apres_declare.dedup_by_key(|(j, _)| *j);
    let quoi = format!("{} passe de '{}' (declare en {}) a '{}'", md.r40, avant, libelle_mois(&decl.mois), if apres.is_empty() { "(rien)" } else { apres });
    let ou_dater = {
        let mut o = Vec::new();
        if md.saisie {
            o.push("la `date_modification` de l'engagement dans la saisie du mois (a l'ecran : « Avenant a compter du », le jour ou \
l'avenant prend effet)"
                .to_string());
        }
        if !md.fiche.is_empty() || !md.entreprise.is_empty() {
            o.push("la date du changement sur la fiche (« a partir du » de l'ecran)".to_string());
        }
        o.join(", ou ")
    };
    match apres_declare.as_slice() {
        [] => {
            if let Some((j, d)) = candidats.iter().filter(|(j, _)| *j <= fin_b).max_by_key(|(j, _)| *j) {
                return Err(format!(
                    "{} : {}. Le changement est date du {} ({}), donc deja en vigueur au mois de {}, que la DSN deposee a \
declare avec l'ancienne valeur. Ce n'est plus un changement du mois mais une correction de mois deja declares (cahier \
technique DSN, § 2.1.2.5.2 p. 25-26 : bloc S21.G00.41 et profondeur de recalcul), avec la regularisation de leurs paies, \
que le logiciel ne fait pas. Refus : verifier la date du changement.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, libelle_mois(&decl.mois)
                ));
            }
            Err(format!(
                "{} : {}, et rien ne dit depuis quand. La DSN doit declarer l'ancienne valeur dans un bloc Changements \
contrat (S21.G00.41) date du jour du changement (41.001, cahier technique DSN p. 216 et § 2.1.2.5.2 p. 25), et ce jour ne \
se devine pas. Le dater : {}.",
                qui(p), quoi, ou_dater
            ))
        }
        [(j, d)] => {
            if *j < debut_m {
                return Err(format!(
                    "{} : {}. Le changement est date du {} ({}), avant {} : il se declare dans la DSN de {}, qui n'est pas \
deposee. La deposer d'abord (elle porte le changement) ; si ces mois ont ete declares avec l'ancienne valeur, c'est une \
correction de mois deja declares, que le logiciel ne fait pas.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, libelle_mois(mois), libelle_mois(&format!("{:04}-{:02}", j.an, j.mois))
                ));
            }
            // En cours de mois : la paie decoupee au jour du changement le porte (lot R10 ; norme, § 2.1.2.5 p. 22-23 :
            // 41.001 au jour, 41.028 au premier du mois), pour ce que le decoupage sait calculer.
            if *j > debut_m && !md.en_cours_de_mois && !(md.decoupable && p.decoupee_le(j)) {
                let suivant = Jour { jour: 1, ..debut_m.plus_jours(31)? };
                let comment = if md.decoupable && md.saisie && md.fiche.is_empty() {
                    " La paie se decoupe au jour d'un avenant quand la saisie dit les termes du contrat d'avant (`avant` : \
`salaire_mensuel` et `quotite_contrat`) ; sinon, le"
                } else if md.decoupable {
                    " Ce changement ne decoupe pas la paie : il vient d'ailleurs que de la fiche ou d'un avenant dont la saisie \
dit les termes d'avant (`avant`). Le"
                } else {
                    " Le logiciel ne decoupe la paie qu'au jour d'un changement de statut, de categorie, d'emploi ou de duree du \
travail. Le"
                };
                return Err(format!(
                    "{} : {}, le {} ({}), en cours de mois.{} dater au premier jour d'un mois : le {} ou le {}.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, comment, debut_m.jj_mm_aaaa(), suivant.jj_mm_aaaa()
                ));
            }
            Ok(*j)
        }
        plusieurs => Err(format!(
            "{} : {}, et plusieurs changements sont dates depuis la DSN deposee de {} ({}). La DSN les declarerait en \
plusieurs blocs, chacun avec la valeur d'avant lui, que la fiche ne garde pas toutes. Refus : deposer les mois \
intermediaires, ou ne garder qu'un changement.",
            qui(p),
            quoi,
            libelle_mois(&decl.mois),
            plusieurs.iter().map(|(j, d)| format!("{} par {}", j.jj_mm_aaaa(), d)).collect::<Vec<_>>().join(" ; ")
        )),
    }
}

/// Les blocs S21.G00.41 du contrat de `p`, dont le bloc 40 vient d'etre ecrit (`ecrit`), dans la DSN mensuelle de
/// `mois` de l'etablissement `nics[0]` ; `declare` : son bloc 40 declare (`dsn::bloc_40_declare`). Vide au premier mois du
/// contrat ou quand rien n'a encore ete declare.
pub fn blocs_41(ctx: &Contexte, p: &Paie, mois: &str, nics: &[String], ecrit: &[(String, String)], declare: Option<&Declare>) -> Resultat<Vec<Vec<(String, String)>>> {
    let e = &p.engagement;
    if e.numero.as_deref().is_none_or(str::is_empty) {
        return Ok(Vec::new());
    }
    if e.premier_mois()? {
        return Ok(Vec::new());
    }
    let Some(decl) = declare else { return Ok(Vec::new()) };
    let fin_m = dernier_du_mois(mois)?;
    let mut identifiants = Vec::new();
    // L'IDENTIFIANT : la date de debut. Sans date explicite, il ne change pas en silence (les organismes verraient un
    // nouveau contrat, § 2.1.2.3).
    let (avant, apres) = (valeur(&decl.bloc, "S21.G00.40.001").unwrap_or(""), valeur(ecrit, "S21.G00.40.001").unwrap_or(""));
    if avant != apres {
        let date = e.date_modification.as_deref().map(|t| Jour::lire(t).map_err(|_| format!("{} : `date_modification` illisible : {}.", qui(p), t)));
        let Some(date) = date.transpose()? else {
            return Err(format!(
                "{} : la date de debut du contrat (S21.G00.40.001, donnee identifiante) passe du {} (declare en {}) au {}. \
Un identifiant change ne se declare qu'avec l'ancien dans un bloc Changements contrat (S21.G00.41.021), sans quoi les \
organismes voient un nouveau contrat (cahier technique DSN, § 2.1.2.3, p. 20-21) ; ce bloc se date, et la date ne se \
devine pas. Poser `date_modification` sur l'engagement dans la saisie du mois (a l'ecran : « Avenant a compter du », le \
jour de la correction), ou remettre la date de debut declaree.",
                qui(p),
                ddmmaaaa(avant).map_or_else(|| avant.to_string(), |j| j.jj_mm_aaaa()),
                libelle_mois(&decl.mois),
                ddmmaaaa(apres).map_or_else(|| apres.to_string(), |j| j.jj_mm_aaaa())
            ));
        };
        if date > fin_m {
            return Err(format!(
                "{} : `date_modification` ({}) apres la fin du mois declare ; la norme l'interdit (CCH-15 de S21.G00.41.001, \
p. 217).",
                qui(p),
                date.jj_mm_aaaa()
            ));
        }
        identifiants.push(vec![("S21.G00.41.001".to_string(), date.jjmmaaaa()), ("S21.G00.41.021".to_string(), avant.to_string())]);
    }
    // L'IDENTIFIANT : l'etablissement. Le contrat etait declare par un autre etablissement : son SIRET, date du jour de
    // la mutation (« la datation precise du changement de SIRET est impactante », p. 21).
    if decl.nic != nics[0] {
        let fin_b = dernier_du_mois(&decl.mois)?;
        let mut dates: Vec<Jour> = changements_dates(&p.salarie, &["etablissement"])?
            .into_iter()
            .map(|(j, _)| j)
            .filter(|j| *j > fin_b && *j <= fin_m)
            .collect();
        dates.sort();
        dates.dedup();
        let [date] = dates.as_slice() else {
            return Err(format!(
                "{} : le contrat etait declare par l'etablissement {} (DSN de {}) et l'est maintenant par l'etablissement {}. \
Le SIRET de l'ancien etablissement se declare dans un bloc Changements contrat (S21.G00.41.012) date du jour de la \
mutation (cahier technique DSN, p. 21) ; la fiche du salarie {} . Dater le changement d'etablissement sur la fiche.",
                qui(p),
                decl.siret,
                libelle_mois(&decl.mois),
                nics[0],
                if dates.is_empty() { "ne le date pas" } else { "date plusieurs changements depuis" }
            ));
        };
        identifiants.push(vec![("S21.G00.41.001".to_string(), date.jjmmaaaa()), ("S21.G00.41.012".to_string(), decl.siret.clone())]);
    }
    // LES MODALITES : une par bloc, l'ancienne valeur, sa date, la profondeur de recalcul.
    let nat = e.nat()?;
    let meme_nature = valeur(&decl.bloc, "S21.G00.40.007") == valeur(ecrit, "S21.G00.40.007");
    let mut modalites = Vec::new();
    for md in &MODALITES {
        let Some(avant) = valeur(&decl.bloc, md.r40) else {
            // Rien de declare : il n'y a pas d'ancienne valeur (41.048 : un statut BOETH « precedemment declare »,
            // p. 234). INFERENCE, la meme pour toutes : une rubrique qui apparait ne fait pas de bloc 41.
            continue;
        };
        let apres = valeur(ecrit, md.r40).unwrap_or("");
        // A la casse pres : « 353b » (le catalogue, la nomenclature de l'Insee) et « 353B » (une fiche) sont un meme code.
        if avant.eq_ignore_ascii_case(apres) {
            continue;
        }
        // Une valeur que la nature fixe (`contrats-base.json`) ne change pas tant que la nature reste : un declare
        // different vient d'une version anterieure du logiciel (40.005 = 999SPT des permanents depuis la vague V2b),
        // pas d'un changement du contrat. INFERENCE : la norme ne dit rien d'une valeur corrigee par le logiciel.
        // Pas 40.016 : la nature y met 99, que le regime local d'Alsace-Moselle remplace (`dsn::bloc_contrat`), et une
        // mutation qui en sort est un vrai changement.
        if meme_nature && FIXEES_PAR_LA_NATURE.contains(&md.r40) && nat.code(md.r40).filter(|c| !c.is_empty()).as_deref() == Some(apres) {
            continue;
        }
        let date = date_du_changement(ctx, p, md, &decl, mois, avant, apres)?;
        let debut = ddmmaaaa(valeur(ecrit, "S21.G00.40.001").unwrap_or("")).unwrap_or(e.debut);
        let profondeur = Jour { jour: 1, ..date }.max(debut);
        modalites.push((
            md.r41,
            vec![
                ("S21.G00.41.001".to_string(), date.jjmmaaaa()),
                (md.r41.to_string(), avant.to_string()),
                ("S21.G00.41.028".to_string(), profondeur.jjmmaaaa()),
            ],
        ));
    }
    modalites.sort_by_key(|(r, _)| *r);
    identifiants.extend(modalites.into_iter().map(|(_, b)| b));
    Ok(identifiants)
}

/// Pose les blocs 41 du contrat de `p` juste apres son bloc 40, qui commence a la ligne `debut` de `lignes`.
pub fn poser(lignes: &mut Vec<String>, debut: usize, ctx: &Contexte, p: &Paie, mois: &str, nics: &[String], declare: Option<&Declare>) -> Resultat<()> {
    let mut fin = debut;
    let mut ecrit = Vec::new();
    while let Some(l) = lignes.get(fin).filter(|l| l.starts_with("S21.G00.40.")) {
        if let Some((k, v)) = l.split_once(',') {
            ecrit.push((k.to_string(), v.trim_matches('\'').to_string()));
        }
        fin += 1;
    }
    let blocs = blocs_41(ctx, p, mois, nics, &ecrit, declare)?;
    let neuves: Vec<String> = blocs.into_iter().flatten().map(|(k, v)| format!("{},'{}'", k, v)).collect();
    lignes.splice(fin..fin, neuves);
    Ok(())
}

/// LA DATE DE DEBUT D'UN CONTRAT DECLARE, CORRIGEE APRES LA LIMITE DE L'ANNULE-ET-REMPLACE (21/10/2026), par une
/// regularisation (`dsn::Regularisation`) : fiche net-entreprises 1952 (`atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1952-correction-date-debut-contrat-20250326.txt`), « Cette
/// donnee peut etre corrigee via un bloc " Changements Contrat - S21.G00.41 " en remplissant les rubriques " Date de la
/// modification - S21.G00.41.001 " et " Ancienne date de debut du contrat - S21.G00.41.021 ". La nouvelle valeur est
/// renseignee au niveau du bloc " Contrat - S21.G00.40 " ». Le bloc 40 vient d'etre ecrit a partir de la ligne `debut`
/// de `lignes`, a la date corrigee ; le bloc 41 se pose juste apres lui.
///
/// - 41.021 : la date declaree, LUE dans la version declaree (`versement_declare`, `debut_contrat` : regle 4), pas celle
///   d'une saisie ;
/// - 41.001 : le jour ou la correction a ete ecrite (« Cette erreur a ete corrigee en paie le 7 fevrier 2018 » ->
///   41.001 = 07022018, l'exemple de la fiche), au plus tard le dernier jour du mois principal declare (CCH-15 de
///   41.001, `bloc-S21.G00.41.txt` p. 217) ; pour un identifiant, « la date de modification du bloc changement peut
///   correspondre a toute date » (§ 2.1.2.3.2, `chapitre-2-principes-de-constitution.txt` p. 21) ;
/// - un identifiant seul dans son bloc, sans profondeur de recalcul (CCH-16, principe d, p. 23 ; fiche 1952 : 41.021
///   « Il n'est pas possible de la declarer avec la rubrique " Profondeur de recalcul de la paie - S21.G00.41.028 " »).
///
/// `r` : la correction du dernier mois qui a declare le contrat (pour un contrat sur plusieurs mois, celle du dernier) ;
/// `nouveau` : la date de debut du bloc 40 ecrit.
///
/// Rien a poser quand la date n'a pas change, ou pour un contrat jamais declare (un contrat ajoute). Refus quand la
/// date declaree ne se lit pas, ou que le jour de la correction n'est pas connu : le bloc ne se devine pas (regle 1).
pub fn ancienne_date_de_debut(lignes: &mut Vec<String>, debut: usize, r: &crate::dsn::Regularisation, nouveau: &Jour, fin_mois: Jour) -> Resultat<()> {
    let e = &r.apres.engagement;
    let numero = e.numero.clone().unwrap_or_default();
    if r.declare.lignes.is_empty() && r.declare.fichier.is_empty() {
        return Ok(());
    }
    let Some(ancienne) = r.declare.debut_contrat.as_deref() else {
        if *nouveau == r.avant.engagement.debut {
            return Ok(());
        }
        return Err(format!(
            "Correction de {} (contrat {}) : la date de debut passe du {} au {}, et la DSN deposee {} ne dit pas la date \
declaree (S21.G00.40.001 du contrat). Le bloc Changements contrat (S21.G00.41.021, fiche net-entreprises 1952) porte la \
date declaree : elle se lit, elle ne se devine pas. Rien n'est ecrit.",
            r.mois,
            numero,
            r.avant.engagement.debut.jj_mm_aaaa(),
            nouveau.jj_mm_aaaa(),
            r.declare.fichier
        ));
    };
    if ancienne == nouveau.jjmmaaaa() {
        return Ok(());
    }
    let Some(decidee) = r.decidee_le else {
        return Err(format!(
            "Correction de {} (contrat {}) : la date de debut passe du {} au {} ; le bloc Changements contrat se date du \
jour de la correction (S21.G00.41.001, fiche net-entreprises 1952), et la correction ne le dit pas (`decidee_le`). Rien \
n'est ecrit.",
            r.mois,
            numero,
            ddmmaaaa(ancienne).map_or_else(|| ancienne.to_string(), |j| j.jj_mm_aaaa()),
            nouveau.jj_mm_aaaa()
        ));
    };
    let date = decidee.min(fin_mois);
    let mut fin = debut;
    while lignes.get(fin).is_some_and(|l| l.starts_with("S21.G00.40.")) {
        fin += 1;
    }
    let bloc = [format!("S21.G00.41.001,'{}'", date.jjmmaaaa()), format!("S21.G00.41.021,'{}'", ancienne)];
    lignes.splice(fin..fin, bloc);
    Ok(())
}
