//! Le signalement de fin de contrat de travail unique, nature 07.
//!
//! Il porte l'evenement ET la paie du solde de tout compte (cahier technique
//! 2026.1 p. 107) : un individu, un contrat, un bloc 62 et un bloc 63
//! obligatoires, ni bordereau ni ordre de telepaiement. Les controles qui lui
//! sont propres (CCH-14, CCH-15, CCH-11) sont tenus ici ; `Echec::Impossible` est la regle de la norme qui s'oppose.

use serde_json::Value;

use droit::contexte::{aujourdhui, Contexte};
use socle::date::{en_texte, Jour};
use crate::dsn::{
    autre_revenu, bases_et_cotisations, bloc_anciennete, bloc_contrat_fctu, g, liste, primes_et_indemnites,
    remunerations, req, usage_admet, Arret, Fichier, LOGICIEL, VERSION,
};
use engagement::Engagement;
use socle::fichiers::{repr, vrai};
use socle::num::Resultat;

pub const NATURE: &str = "07";
/// CCH-14 (cahier technique p. 293) : la base assujettie de prevoyance n'a
/// pas sa place dans un signalement de fin de contrat.
const BASES_INTERDITES: [&str; 1] = ["31"];
pub const NORMALE: &str = "01";
pub const ANNULE_ET_REMPLACE: &str = "03";
pub const ANNULE: &str = "04";
const TYPES: [(&str, &str); 3] = [(NORMALE, "normale"), (ANNULE_ET_REMPLACE, "annule et remplace integral"), (ANNULE, "annule")];

/// Un refus : `Impossible` est la regle de la norme qui s'oppose, `Autre` tout le reste.
#[derive(Debug, Clone)]
pub enum Echec {
    Impossible(String),
    Autre(String),
}

impl From<String> for Echec {
    fn from(m: String) -> Echec {
        Echec::Autre(m)
    }
}

impl From<&str> for Echec {
    fn from(m: &str) -> Echec {
        Echec::Autre(m.to_string())
    }
}

impl Echec {
    pub fn message(&self) -> &str {
        match self {
            Echec::Impossible(m) | Echec::Autre(m) => m,
        }
    }
}

fn type_de_declaration(code: &str) -> Option<&'static str> {
    TYPES.iter().find(|(c, _)| *c == code).map(|(_, l)| *l)
}

/// `fctu-base.json`, lu par sa forme (`droit::preavis::base`).
fn nomenclatures() -> Resultat<droit::preavis::BaseFctu> {
    droit::preavis::base()
}

/// `FinDeContrat.libelle_preavis` : le libelle du type de preavis, « ? » s'il est inconnu.
pub fn libelle_preavis(type_preavis: &str) -> Resultat<String> {
    Ok(nomenclatures()?.types_preavis.du_code(type_preavis).map_or_else(|| "?".to_string(), |c| c.libelle.clone()))
}

/// Le code par defaut d'une nomenclature.
fn defaut(n: &droit::preavis::Nomenclature, quoi: &str) -> Resultat<String> {
    n.defaut().map(str::to_string).ok_or_else(|| socle::echec::livree("fctu-base.json", quoi))
}

/// La fin d'un contrat, telle qu'elle se declare (dates AAAA-MM-JJ). Le dernier jour travaille et paye
/// (S21.G00.62.006) n'y est pas : il se lit sur le contrat, par `dernier_jour_paye`, comme dans la mensuelle.
#[derive(Debug, Clone)]
pub struct FinDeContrat {
    pub fin: String,
    pub motif: String,
    pub mois_rattachement: String,
    pub type_preavis: String,
    pub preavis_debut: Option<String>,
    pub preavis_fin: Option<String>,
    pub notification: Option<String>,
    pub signature_convention: Option<String>,
    pub engagement_licenciement: Option<String>,
    pub transaction: Option<String>,
}

impl FinDeContrat {
    /// Depuis l'objet JSON de ses champs.
    pub fn lire(v: &Value) -> Resultat<FinDeContrat> {
        let t = |k: &str| -> Resultat<String> { req(v, k).map(en_texte) };
        let o = |k: &str| -> Option<String> {
            let x = g(v, k);
            if vrai(x) {
                Some(en_texte(x))
            } else {
                None
            }
        };
        Ok(FinDeContrat {
            fin: t("fin")?,
            motif: t("motif")?,
            mois_rattachement: t("mois_rattachement")?,
            type_preavis: t("type_preavis")?,
            preavis_debut: o("preavis_debut"),
            preavis_fin: o("preavis_fin"),
            notification: o("notification"),
            signature_convention: o("signature_convention"),
            engagement_licenciement: o("engagement_licenciement"),
            transaction: o("transaction"),
        })
    }
}

fn d(s: &str) -> Resultat<Jour> {
    Jour::lire(s)
}

// ------------------------------------------------ le dernier jour travaille et paye

/// Le type de preavis (S21.G00.63.001) est-il un preavis NON effectue ? Le drapeau `non_effectue` de
/// `fctu-base.json` : 02, 03, 10, 50, 51, 61, que nomme la CCH-12 de S21.G00.62.006 (cahier technique DSN 2026.1,
/// p. 275). Un type inconnu refuse.
pub fn preavis_non_effectue(type_preavis: &str) -> Resultat<bool> {
    match nomenclatures()?.types_preavis.du_code(type_preavis) {
        Some(c) => Ok(c.non_effectue),
        None => Err(format!(
            "Type de préavis {} inconnu de la norme (S21.G00.63.001) : le choisir dans la liste.",
            repr(type_preavis)
        )),
    }
}

// ------------------------------------------------ la date de notification de la rupture

/// La date de notification de la rupture (S21.G00.62.003) est-elle obligatoire pour ce motif de rupture ? La liste
/// `notification_rupture.motifs` de `fctu-base.json`, celle de la CCH-12 de S21.G00.62.003 (cahier technique DSN 2026.1,
/// p. 275, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`). Un signalement de ce motif qui ne la porte pas est
/// refuse par le point de depot. Un seul endroit : la liste des fins a signaler (`moteur::routes::imprevus::ce_qui_manque`) et
/// `verifier` la lisent ici.
pub fn notification_exigee(motif: &str) -> Resultat<bool> {
    Ok(nomenclatures()?.notification_rupture.motifs.iter().any(|m| m == motif))
}

// ------------------------------------------------ le type de preavis et ce que la paie verse

/// Les lignes d'une paie declarees en indemnite compensatrice de preavis (S21.G00.52.001), en (code, libelle).
pub fn indemnites_de_preavis_versees(paie: &engagement::calcul::Paie) -> Resultat<Vec<(String, String)>> {
    let codes = droit::preavis::codes_52_de_l_indemnite_de_preavis()?;
    Ok(paie
        .remuneration
        .iter()
        .filter(|l| !l.montant.is_zero())
        .filter_map(|l| l.dsn_bloc52.as_ref().filter(|c| codes.contains(c)).map(|c| (c.clone(), l.libelle.clone())))
        .collect())
}

/// Les controles de S21.G00.62.006 (cahier technique DSN 2026.1, p. 275-276) : CCH-15, le jour tombe dans le contrat
/// (`debut` = 40.001, `fin` = 62.001) ; CCH-12, sur un preavis non effectue, il precede le debut du preavis (63.002).
fn controler_62_006(
    jour_paye: &Jour, debut: &Jour, fin: &Jour, type_preavis: Option<&str>, debut_preavis: Option<&Jour>,
) -> Resultat<()> {
    if jour_paye > fin {
        return Err(format!(
            "Le dernier jour travaille et paye au salaire habituel ({}) est posterieur a la fin du contrat ({}).",
            jour_paye.iso(),
            fin.iso()
        ));
    }
    if jour_paye < debut {
        return Err(format!(
            "Le dernier jour travaille et paye au salaire habituel ({}) est anterieur au debut du contrat ({}) -- CCH-15 \
de S21.G00.62.006.",
            jour_paye.iso(),
            debut.iso()
        ));
    }
    if let (Some(t), Some(dp)) = (type_preavis, debut_preavis) {
        if preavis_non_effectue(t)? && jour_paye >= dp {
            return Err(format!(
                "Preavis « {} » : le dernier jour travaille et paye au salaire habituel ({}) doit preceder le debut du \
preavis ({}) -- CCH-12 de S21.G00.62.006.",
                libelle_preavis(t)?,
                jour_paye.iso(),
                dp.iso()
            ));
        }
    }
    Ok(())
}

/// S21.G00.62.006, « Dernier jour travaille paye selon le salaire defini par le contrat de travail ou la convention
/// collective » (cahier technique DSN 2026.1, p. 275, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`) : LA
/// SEULE SOURCE, que lisent la mensuelle (`dsn::bloc_contrat_fctu`, le mois ou le contrat finit) et le FCTU du meme
/// contrat. La mensuelle doit reporter « a l'identique » ce que porte le signalement (§ 2.5.2, p. 68).
///
/// Le jour que la saisie dit (`dernier_jour_paye`) ; sinon la fin du contrat, qui n'est ce jour-la que si le preavis
/// a ete effectue ou qu'il n'y en a pas. Sur un preavis NON effectue (02, 03, 10, 50, 51, 61), le dernier jour paye au
/// salaire habituel precede le debut du preavis (CCH-12) : sans saisie, le moteur refuse plutot que de le deviner.
pub fn dernier_jour_paye(e: &Engagement) -> Resultat<Jour> {
    let numero = e.numero.as_deref().unwrap_or("None").to_string();
    let fin = e.fin.ok_or_else(|| format!("S21.G00.62.006 : le contrat {} n'a pas de fin.", numero))?;
    let preavis = e.preavis.as_ref();
    let type_preavis = preavis.map(|p| p.genre.clone()).filter(|t| !t.is_empty());
    let debut_preavis = match preavis.and_then(|p| p.debut.as_deref()).filter(|v| !v.is_empty()) {
        Some(v) => Some(Jour::lire(v)?),
        None => None,
    };
    let jour_paye = match e.dernier_jour_paye.as_deref() {
        Some(s) => d(s)?,
        None => {
            if let Some(t) = type_preavis.as_deref() {
                if preavis_non_effectue(t)? {
                    return Err(format!(
                        "Contrat {} : preavis « {} », et la saisie ne dit pas le dernier jour travaille et paye au salaire \
habituel (« Dernier jour payé », S21.G00.62.006). Sur un preavis non effectue, ce n'est pas la fin du contrat ({}) : il \
precede le debut du preavis (CCH-12, cahier technique DSN p. 275). Il se saisit ; le moteur ne le devine pas.",
                        numero,
                        libelle_preavis(t)?,
                        fin.iso()
                    ));
                }
            }
            fin
        }
    };
    controler_62_006(&jour_paye, &e.debut, &fin, type_preavis.as_deref(), debut_preavis.as_ref())
        .map_err(|m| format!("Contrat {} : {}", numero, m))?;
    Ok(jour_paye)
}

// ------------------------------------------------------------- les controles

/// `verifier` : refuse ce que la norme refuse ; rend les avertissements.
pub fn verifier(fin: &FinDeContrat, e: &Engagement, type_declaration: &str) -> Result<Vec<String>, Echec> {
    let mut avertissements = Vec::new();
    if type_de_declaration(type_declaration).is_none() {
        return Err(Echec::Impossible(format!(
            "Type de declaration {}. Un FCTU est {}.",
            repr(type_declaration),
            TYPES.iter().map(|(c, l)| format!("{} - {}", c, l)).collect::<Vec<_>>().join(", ")
        )));
    }
    let types = nomenclatures()?.types_preavis;
    let preavis = match types.du_code(&fin.type_preavis) {
        Some(p) => p,
        None => {
            return Err(Echec::Impossible(format!(
                "Type de preavis {} inconnu de la norme (S21.G00.63.001). Les types sont dans fctu-base.json.",
                repr(&fin.type_preavis)
            )))
        }
    };
    let libelle = preavis.libelle.clone();
    let dates = preavis.dates;
    let (debut_p, fin_p) = (fin.preavis_debut.as_deref(), fin.preavis_fin.as_deref());

    // Le bloc 63 est (1,*) : un FCTU en porte toujours un.
    if dates && !(debut_p.is_some() && fin_p.is_some()) {
        return Err(Echec::Impossible(format!(
            "Preavis « {} » : ses deux dates sont OBLIGATOIRES (CCH-11 de S21.G00.63.002 et CCH-14 de .003 -- elles \
sont presentes si et seulement si le type n'est pas 90). Si le contrat n'a pas de preavis, le type est « 90 - pas de \
clause de preavis applicable », et les dates sont alors interdites.",
            libelle
        )));
    }
    if !dates && (debut_p.is_some() || fin_p.is_some()) {
        return Err(Echec::Impossible(format!(
            "Preavis « {} » : les dates de preavis y sont INTERDITES (CCH-11 de S21.G00.63.002).",
            libelle
        )));
    }
    if dates {
        let (pd, pf) = (debut_p.unwrap_or(""), fin_p.unwrap_or(""));
        if d(pf)? < d(pd)? {
            return Err(Echec::Impossible(format!(
                "Preavis : la date de fin ({}) est anterieure a la date de debut ({}) -- CCH-13 de S21.G00.63.003.",
                pf, pd
            )));
        }
        // CCH-16 : le preavis commence apres le debut du contrat.
        if d(pd)? < e.debut {
            return Err(Echec::Impossible(format!(
                "Preavis : il commence le {}, avant le debut du contrat ({}) -- CCH-16 de S21.G00.63.002.",
                pd,
                e.debut.iso()
            )));
        }
        // CCH-13 : le preavis commence apres la notification de la rupture.
        if let Some(n) = fin.notification.as_deref() {
            if d(pd)? < d(n)? {
                return Err(Echec::Impossible(format!(
                    "Preavis : il commence le {}, avant la notification de la rupture ({}) -- CCH-13 de \
S21.G00.63.002.",
                    pd, n
                )));
            }
        }
    }

    // CCH-11 et CCH-12 du bloc 63 : deux motifs limitent le type de preavis.
    if let Some(limite) = types.permis_au_motif(&fin.motif) {
        let permis = &limite.types;
        if !permis.contains(&fin.type_preavis) {
            let mut noms = Vec::new();
            for c in permis {
                let lib = types.du_code(c).map(|x| x.libelle.clone());
                noms.push(format!("« {} - {} »", c, lib.ok_or_else(|| socle::fichiers::absent(c))?));
            }
            return Err(Echec::Impossible(format!(
                "Preavis « {} » avec le motif de rupture {} : {} n'y permet que {}. {}",
                libelle,
                fin.motif,
                limite.controle,
                noms.join(", "),
                if fin.motif == "043" || fin.motif == "110" {
                    "Une rupture conventionnelle n'a pas de preavis : sa convention FIXE la date de rupture (L1237-13)."
                } else {
                    "Une rupture en periode d'essai releve du delai de prevenance (L1221-25 et L1221-26), pas du \
preavis de L1234-1."
                }
            )));
        }
    }

    // CCH-11 de S21.G00.62.003 : la notification tombe dans le contrat.
    if let Some(nt) = fin.notification.as_deref() {
        let nt_j = d(nt)?;
        if !(e.debut <= nt_j && nt_j <= d(&fin.fin)?) {
            return Err(Echec::Impossible(format!(
                "Notifiée le {} : la rupture se notifie pendant le contrat, entre son premier jour, le {}, et sa fin, le \
{} -- CCH-11 de S21.G00.62.003.",
                nt_j.jj_mm_aaaa(),
                e.debut.jj_mm_aaaa(),
                d(&fin.fin)?.jj_mm_aaaa()
            )));
        }
    }

    // CCH-12 de S21.G00.62.003 : pour ces motifs, la date de notification est obligatoire.
    if fin.notification.is_none() && notification_exigee(&fin.motif)? {
        return Err(Echec::Impossible(format!(
            "Motif de rupture {} : la date de notification de la rupture (S21.G00.62.003, `notification_rupture` dans la \
saisie) est OBLIGATOIRE -- CCH-12 de S21.G00.62.003, cahier technique DSN p. 275. Le point de depot refuserait le \
signalement sans elle. Rien n'est ecrit.",
            fin.motif
        )));
    }

    // S21.G00.62.006 se lit sur le contrat, comme dans la mensuelle ; ses controles valent aussi contre la fin et le
    // preavis que ce signalement declare (62.001, 63.001, 63.002).
    let jour_paye = dernier_jour_paye(e).map_err(Echec::Impossible)?;
    let debut_preavis = match debut_p {
        Some(p) => Some(d(p)?),
        None => None,
    };
    controler_62_006(&jour_paye, &e.debut, &d(&fin.fin)?, Some(&fin.type_preavis), debut_preavis.as_ref())
        .map_err(Echec::Impossible)?;

    // CCH-11 de S21.G00.62.002 : le motif doit etre autorise pour la nature du contrat (091/092 pas pour un CDD).
    let nat = e.nat()?;
    if let Some(refus) = nat.motif_rupture_refuse(&fin.motif, &e.numero.as_deref().unwrap_or("None").to_string()) {
        return Err(Echec::Impossible(refus));
    }

    // La derogation du CDDU (circuit derogatoire, motif 031 seul : fiche 2062) : le moteur le DIT plutot que de refuser,
    // car le salarie peut demander le signalement (fiche 1902). Rompu avant son terme, le CDDU se signale.
    if !nat.signalement_fctu(Some(&fin.motif)) {
        avertissements.push(format!(
            "Contrat {} : un {} arrivé à son terme n'a pas besoin de signalement de fin de contrat ; sa fin part dans la \
DSN du mois, par le circuit dérogatoire. Ce signalement ne se fait que si le salarié le demande.",
            e.numero.as_deref().unwrap_or_default().to_string(),
            nat.libelle
        ));
    }
    avertissements.push(format!(
        "Fin du contrat le {} : le signalement est dû dans les cinq jours ouvrés qui suivent (cahier technique p. 107).",
        socle::date::jj_mm_aaaa(&fin.fin)
    ));
    Ok(avertissements)
}

// ------------------------------------------------------------ les blocs 62/63

/// La fin du contrat et son preavis, dans l'ordre de l'arborescence. `e` : le contrat, d'ou se lit S21.G00.62.006
/// (`dernier_jour_paye`).
pub fn bloc_62_63(f: &mut Fichier, fin: &FinDeContrat, e: &Engagement) -> Resultat<()> {
    let j = |s: &str| -> Resultat<String> { Ok(d(s)?.jjmmaaaa()) };
    f.add("S21.G00.62.001", j(&fin.fin)?);
    f.add("S21.G00.62.002", &fin.motif);
    if let Some(n) = &fin.notification {
        f.add("S21.G00.62.003", j(n)?);
    }
    if let Some(s) = &fin.signature_convention {
        f.add("S21.G00.62.004", j(s)?);
    }
    if let Some(s) = &fin.engagement_licenciement {
        f.add("S21.G00.62.005", j(s)?);
    }
    f.add("S21.G00.62.006", dernier_jour_paye(e)?.jjmmaaaa());
    // << Transaction en cours >> est OBLIGATOIRE : elle se declare meme pour
    // dire qu'il n'y en a pas.
    match &fin.transaction {
        Some(t) => f.add("S21.G00.62.008", t),
        None => f.add("S21.G00.62.008", defaut(&nomenclatures()?.transaction, "transaction")?),
    }
    // S21.G00.62.020 : « cette date doit etre renseignee au premier jour du
    // mois (sous la forme 01mmaaaa) » -- cahier technique p. 278.
    f.add("S21.G00.62.020", j(&format!("{}-01", fin.mois_rattachement))?);
    f.add("S21.G00.63.001", &fin.type_preavis);
    if let Some(p) = &fin.preavis_debut {
        f.add("S21.G00.63.002", j(p)?);
    }
    if let Some(p) = &fin.preavis_fin {
        f.add("S21.G00.63.003", j(p)?);
    }
    Ok(())
}

// ---------------------------------------------------------------- generation

/// Les parametres de `generer`, avec leurs defauts.
pub struct Options {
    pub type_declaration: String,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub envoi_test: bool,
    pub date_constitution: Option<Jour>,
}

impl Default for Options {
    fn default() -> Options {
        Options { type_declaration: NORMALE.into(), ordre: 1, remplace: None, envoi_test: false, date_constitution: None }
    }
}

/// Le fichier d'un FCTU, pret a deposer : les suspensions de l'engagement de la paie (bloc 65) ;
/// `arrets` : les arrets a reporter (bloc 60) ; `departs` : ce qui est parti, pour lire les engagements valides du mois.
pub fn generer(
    ctx: &Contexte, paie: &engagement::calcul::Paie, fin: &FinDeContrat, o: &Options, arrets: &[Arret], departs: &engagement::departs::Departs,
) -> Result<String, Echec> {
    let type_declaration = o.type_declaration.as_str();
    let libelle_type = match type_de_declaration(type_declaration) {
        Some(l) => l,
        None => return Err(Echec::Impossible(format!("Type de declaration {}.", repr(type_declaration)))),
    };
    let remplace = o.remplace.as_deref().filter(|r| !r.is_empty());
    if (type_declaration == ANNULE_ET_REMPLACE || type_declaration == ANNULE) && remplace.is_none() {
        return Err(Echec::Impossible(format!(
            "Un signalement « {} » doit viser la declaration qu'il reprend (S20.G00.05.006) : sans elle, la \
plate-forme rend B1-208-08.",
            libelle_type
        )));
    }
    if type_declaration == NORMALE && remplace.is_some() {
        return Err(Echec::Impossible(
            "S20.G00.05.006 est interdite sur un signalement normal : elle ne s'ecrit que sur un annule-et-remplace \
ou un annule."
                .into(),
        ));
    }
    verifier(fin, &paie.engagement, type_declaration)?;
    // Le type de preavis (63.001) et l'indemnite que la paie du solde verse (52.001) disent la meme chose.
    if let Some(pourquoi) = droit::preavis::desaccord_du_preavis(&fin.type_preavis, &indemnites_de_preavis_versees(paie)?)? {
        return Err(Echec::Impossible(format!(
            "Contrat {} : {}",
            paie.engagement.numero.as_deref().unwrap_or("None").to_string(),
            pourquoi
        )));
    }

    let e = &paie.engagement;

    let fin_jour = d(&fin.fin)?;
    let ctx = &ctx.au(&fin_jour)?;
    let ent0 = &ctx.entreprise;
    let s = &paie.salarie;
    let bar = crate::dsn::BaremeDuMois::nouveau((*ctx.bareme(&fin_jour)?).clone(), ctx.entreprise_lue(&fin_jour)?);
    let constitution = match o.date_constitution {
        Some(j) => j,
        None => aujourdhui()?,
    };
    let (debut_p, fin_p) = (e.debut_periode()?, e.fin_periode()?);

    // La version de la norme d'un signalement suit le jour ou il est constitue (`norme-dsn-base.json`, une inference :
    // `_inference`) ; un jour que la donnee ne couvre pas refuse.
    let norme = crate::norme::du_signalement(&constitution)?;
    crate::naf::verifier_pour(&norme, ent0)?;
    let mut f = Fichier::en_norme(norme);
    // ------------------------------------------------------------ S10 envoi
    f.add("S10.G00.00.001", LOGICIEL);
    f.add("S10.G00.00.002", LOGICIEL);
    f.add("S10.G00.00.003", VERSION);
    f.add("S10.G00.00.005", if o.envoi_test { "01" } else { "02" });
    f.ecrire_la_version()?;
    f.add("S10.G00.00.007", "01");
    f.add("S10.G00.00.008", "01");
    f.add_case("S10.G00.01.001", &ent0.siren, "siren")?;
    f.add_case("S10.G00.01.002", &ent0.nic, "nic")?;
    f.add_case("S10.G00.01.003", &ent0.raison_sociale, "raison_sociale")?;
    f.add_case("S10.G00.01.004", &ent0.adresse, "adresse")?;
    f.add_case("S10.G00.01.005", &ent0.code_postal, "code_postal")?;
    f.add_case("S10.G00.01.006", &ent0.commune, "commune")?;
    f.add_complement("S10.G00.01.009", &ent0.complement_adresse);
    crate::dsn::contact_emetteur(&mut f, ent0)?;

    // ---------------------------------------------------------- S20 declaration
    f.add("S20.G00.05.001", NATURE);
    f.add("S20.G00.05.002", type_declaration);
    f.add("S20.G00.05.003", "11");
    f.add("S20.G00.05.004", o.ordre);
    // S20.G00.05.005 (mois principal declare) est INTERDITE : un signalement
    // ne declare pas un mois.
    if let Some(r) = remplace {
        f.add("S20.G00.05.006", r);
    }
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    f.add("S20.G00.05.010", "01");
    crate::dsn::contact_chez_le_declare(&mut f, ent0)?;

    // ------------------------------------------------------------ S21 entreprise
    f.add_case("S21.G00.06.001", &ent0.siren, "siren")?;
    f.add_case("S21.G00.06.002", &ent0.nic, "nic")?;
    // Code APEN et code APET : retires de la norme P27V01 (cahier 2027.1.1, p. 142 et 145) ; la version en decide.
    if usage_admet(&f, "S21.G00.06.003", NATURE)? {
        f.add_case("S21.G00.06.003", &ent0.naf, "naf")?;
    }
    f.add_case("S21.G00.06.004", &ent0.adresse, "adresse")?;
    f.add_case("S21.G00.06.005", &ent0.code_postal, "code_postal")?;
    f.add_case("S21.G00.06.006", &ent0.commune, "commune")?;
    f.add_complement("S21.G00.06.007", &ent0.complement_adresse);
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`droit::bareme::entreprise_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = droit::bareme::entreprise_du_salarie(ent0, &paie.salarie)?;
    for (rub, case, cle) in [
        ("S21.G00.11.001", &etab.nic, "nic"),
        ("S21.G00.11.002", &etab.naf.en_texte(), "naf"),
        ("S21.G00.11.003", &etab.adresse.en_texte(), "adresse"),
        ("S21.G00.11.004", &etab.code_postal.en_texte(), "code_postal"),
        ("S21.G00.11.005", &etab.commune.en_texte(), "commune"),
        ("S21.G00.11.022", &etab.idcc, "idcc"),
    ] {
        if rub == "S21.G00.11.002" && !usage_admet(&f, rub, NATURE)? {
            continue;
        }
        f.add_case(rub, case, cle)?;
        if rub == "S21.G00.11.005" {
            f.add_complement("S21.G00.11.006", &etab.complement_adresse);
        }
    }
    // Les adhesions prevoyance, sans S21.G00.15.004 : la rubrique « personnel
    // couvert » est interdite dans un signalement.
    let orgs = ctx.organismes()?;
    // Le numero de licence EN VIGUEUR a la fin du contrat, pas la liste.
    let mut ent_l = etab.clone();
    ent_l.licence_spectacle = droit::fiche::Case::Dite(ctx.licence(&fin_jour));
    for a in liste(req(&orgs, "adhesions_prevoyance")?) {
        f.add_v("S21.G00.15.001", req(a, "libelle")?)?;
        f.add_v("S21.G00.15.002", req(a, "delegataire")?)?;
        f.add_v("S21.G00.15.005", req(a, "reference")?)?;
    }

    // ------------------------------------------------------------ S21 individu
    crate::dsn::bloc_individu(&mut f, s, false)?;
    crate::ntt::poser_ntt(&mut f, &ent_l, s, crate::changements_individu::derniere_declaree(ctx, &paie.salarie, &fin_jour, departs)?.as_ref())?;

    // ------------------------------------------------------------ S21 contrat
    let siens: Vec<&Arret> = arrets.iter().collect();
    // Les emplois multiples (`40.036`) : les autres contrats du salarie dans la periode de paie du signalement.
    let du_mois = crate::dsn::contrats_du_mois(ctx, &debut_p.mois_civil().to_string(), departs)?;
    let emplois = crate::dsn::emplois_multiples(paie, &du_mois.iter().collect::<Vec<_>>());
    bloc_contrat_fctu(&mut f, paie, &ent_l, &orgs, 1, &bar, true, &siens, NATURE, Some(fin), emplois)?;

    // ------------------------------------------------------------ S21 versement
    // La date de versement : la fin de la periode de paie.
    f.add("S21.G00.50.001", fin_p.jjmmaaaa());
    f.add("S21.G00.50.003", "01");
    f.add("S21.G00.50.020", defaut(&nomenclatures()?.rattachement_versement, "rattachement_versement")?);
    // Un mois decoupe en periodes elementaires : les remunerations et les bases de chacune, datees de ses bornes.
    let bornes = |q: &engagement::calcul::Paie| -> Resultat<(Jour, Jour)> {
        if paie.periodes.is_empty() { Ok((debut_p, fin_p)) } else { Ok((q.engagement.debut_periode()?, q.engagement.fin_periode()?)) }
    };
    for q in paie.elementaires() {
        let (d, f_) = bornes(q)?;
        remunerations(&mut f, q, &d, &f_, 1)?;
    }
    primes_et_indemnites(&mut f, paie, &debut_p, &fin_p, 1)?;
    autre_revenu(&mut f, paie, &bar, &debut_p, &fin_p, 1)?;
    // Les bases de prevoyance ne partent pas dans un FCTU (CCH-14, p. 293).
    for q in paie.elementaires() {
        let (d, f_) = bornes(q)?;
        bases_et_cotisations(&mut f, q, &bar, 1, &d, &f_, 1, NATURE, &BASES_INTERDITES)?;
    }

    // ------------------------------------------------------------ S21 anciennete
    let depuis = match &s.date_anciennete {
        droit::fiche::Case::Absente => e.debut,
        c => *c.exigee("date_anciennete")?,
    };
    bloc_anciennete(&mut f, paie, fin_jour.moins(&depuis) + 1)?;

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    verifier_controles_07(&texte)?;
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    Ok(texte)
}

/// Les deux controles que la nature 07 ajoute : CCH-14 (la base 31 est
/// interdite en S21.G00.78.001) et CCH-15 (au moins trois blocs 51, de types
/// 001, 002 et 010).
pub fn verifier_controles_07(texte: &str) -> Result<(), Echec> {
    let lignes: Vec<&str> = texte.lines().map(str::trim).collect();
    let valeur = |l: &str| -> String { l.split(',').nth(1).unwrap_or("").trim_matches('\'').to_string() };
    let bases: Vec<String> = lignes.iter().filter(|l| l.starts_with("S21.G00.78.001,")).map(|l| valeur(l)).collect();
    if bases.iter().any(|b| b == "31") {
        return Err(Echec::Impossible(
            "CCH-14 : la valeur « 31 » est interdite en S21.G00.78.001 dans un signalement de fin de contrat. Le \
bareme declare une base assujettie de prevoyance, qui n'a pas sa place ici -- l'identifiant d'affiliation qui \
l'accompagne (78.005) y est interdit lui aussi."
                .into(),
        ));
    }
    let types: Vec<String> = lignes.iter().filter(|l| l.starts_with("S21.G00.51.011,")).map(|l| valeur(l)).collect();
    let manquants: Vec<&str> = ["001", "002", "010"].into_iter().filter(|t| !types.iter().any(|x| x == t)).collect();
    if !manquants.is_empty() {
        return Err(Echec::Impossible(format!(
            "CCH-15 : un signalement de fin de contrat porte au moins trois blocs 51 par versement -- « 001 » \
(remuneration brute non plafonnee), « 002 » (salaire brut soumis a contributions chomage) et « 010 » (salaire de \
base). Manque{} : {}.",
            if manquants.len() > 1 { "nt" } else { "" },
            manquants.join(", ")
        )));
    }
    Ok(())
}
